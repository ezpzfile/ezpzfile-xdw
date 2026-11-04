//! Crafted files that claim far more than they hold, for measuring how the
//! reader copes. Nothing here is a real document: every file is built in
//! code (no attack files are kept in the repository).
//!
//! ```text
//! cargo run --release --example hostile -- list
//! cargo run --release --example hostile -- run <case>        open, draw every page, print time and peak memory
//! cargo run --release --example hostile -- write <case> <out.xdw>
//! ```

use ezpzxdw_core::doc::Document;
use ezpzxdw_core::props::Attr;
use ezpzxdw_core::{tlv, write};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------- memory

struct Count;
static NOW: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Count {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let n = NOW.fetch_add(l.size(), Ordering::Relaxed) + l.size();
            PEAK.fetch_max(n, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        NOW.fetch_sub(l.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = System.realloc(p, l, n);
        if !q.is_null() {
            if n > l.size() {
                let now = NOW.fetch_add(n - l.size(), Ordering::Relaxed) + n - l.size();
                PEAK.fetch_max(now, Ordering::Relaxed);
            } else {
                NOW.fetch_sub(l.size() - n, Ordering::Relaxed);
            }
        }
        q
    }
}

#[global_allocator]
static A: Count = Count;

// ---------------------------------------------------------------- -lh5- streams

struct Bits {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits {
    fn new() -> Bits {
        Bits { out: Vec::new(), acc: 0, n: 0 }
    }
    fn put(&mut self, v: u32, bits: u32) {
        for i in (0..bits).rev() {
            self.acc = (self.acc << 1) | ((v >> i) & 1);
            self.n += 1;
            if self.n == 8 {
                self.out.push(self.acc as u8);
                self.acc = 0;
                self.n = 0;
            }
        }
    }
    fn finish(mut self) -> Vec<u8> {
        // padding the decoder may read past the last code
        self.put(0, 32);
        if self.n > 0 {
            self.out.push((self.acc << (8 - self.n)) as u8);
        }
        self.out
    }
}

/// A block whose three tables each hold one symbol, so every code takes
/// no bits at all: `count` copies of `c` from 52 bits.
fn single_block(w: &mut Bits, count: u32, c: u32, p: u32) {
    w.put(count, 16);
    w.put(0, 5); // pt: one symbol
    w.put(0, 5);
    w.put(0, 9); // c: one symbol
    w.put(c, 9);
    w.put(0, 4); // p: one symbol
    w.put(p, 4);
}

/// Expands to at least `total` zero bytes from a few hundred bytes.
fn bomb(total: u64) -> Vec<u8> {
    let mut w = Bits::new();
    single_block(&mut w, 1, 0, 0); // one literal 0
    let per = 0xffffu64 * 256;
    for _ in 0..total.div_ceil(per) {
        single_block(&mut w, 0xffff, 509, 0); // length 256, distance 1
    }
    w.finish()
}

enum Tok {
    Lit(u8),
    /// Copy `len` (3..=256) bytes from `dist` back.
    Match(u32, u32),
}

/// A plain block writer: every table full with equal code lengths, so a
/// code is its symbol number.
fn lh5(toks: &[Tok]) -> Vec<u8> {
    let mut w = Bits::new();
    for block in toks.chunks(0xffff) {
        w.put(block.len() as u32, 16);
        // pt: 19 symbols of length 5
        w.put(19, 5);
        for i in 0..19 {
            w.put(5, 3);
            if i == 2 {
                w.put(0, 2);
            }
        }
        // c: 510 symbols of length 9 (pt symbol 11 each)
        w.put(510, 9);
        for _ in 0..510 {
            w.put(11, 5);
        }
        // p: 14 symbols of length 4
        w.put(14, 4);
        for _ in 0..14 {
            w.put(4, 3);
        }
        for t in block {
            match *t {
                Tok::Lit(b) => w.put(b as u32, 9),
                Tok::Match(len, dist) => {
                    w.put(len - 3 + 256, 9);
                    let d = dist - 1;
                    if d == 0 {
                        w.put(0, 4);
                    } else {
                        let nb = 32 - d.leading_zeros();
                        w.put(nb, 4);
                        if nb > 1 {
                            w.put(d - (1 << (nb - 1)), nb - 1);
                        }
                    }
                }
            }
        }
    }
    w.finish()
}

/// `prefix`, then the last `period` bytes repeated until `fill` bytes in
/// all, then `suffix`.
fn repeat_stream(prefix: &[u8], period: u32, fill: usize, suffix: &[u8]) -> Vec<u8> {
    let mut t: Vec<Tok> = prefix.iter().map(|&b| Tok::Lit(b)).collect();
    let mut have = prefix.len();
    while have < fill {
        let n = (fill - have).min(256);
        if n < 3 {
            // the last few bytes repeat the period byte for byte
            for _ in 0..n {
                t.push(Tok::Lit(0));
            }
        } else {
            t.push(Tok::Match(n as u32, period));
        }
        have += n;
    }
    t.extend(suffix.iter().map(|&b| Tok::Lit(b)));
    lh5(&t)
}

// ---------------------------------------------------------------- EMF

fn emf_header(records: u32, bytes: u32) -> Vec<u8> {
    let mut h = vec![0u8; 88];
    let put = |h: &mut Vec<u8>, at: usize, v: u32| h[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut h, 0, 1);
    put(&mut h, 4, 88);
    for (k, v) in [0u32, 0, 21000, 29700].iter().enumerate() {
        put(&mut h, 24 + 4 * k, *v);
    }
    h[40..44].copy_from_slice(b" EMF");
    put(&mut h, 44, 0x10000);
    put(&mut h, 48, bytes);
    put(&mut h, 52, records);
    put(&mut h, 72, 4960);
    put(&mut h, 76, 7016);
    put(&mut h, 80, 210);
    put(&mut h, 84, 297);
    h
}

fn rec(t: u32, body: &[u32]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&t.to_le_bytes());
    v.extend_from_slice(&((8 + 4 * body.len()) as u32).to_le_bytes());
    for x in body {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v
}

fn eof() -> Vec<u8> {
    rec(14, &[0, 16, 20])
}

// ---------------------------------------------------------------- documents

fn el(tag: u8, v: &[u8]) -> Vec<u8> {
    write::element(tag, v)
}

/// The fields of a drawing: kind, claimed expanded size, stored data.
fn drawing_fields(kind: u8, expanded: Option<u64>, stored: &[u8]) -> Vec<u8> {
    let mut v = el(0x80, &[kind]);
    if let Some(n) = expanded {
        v.extend(el(0x81, &tlv::uint_bytes(n)));
    }
    v.extend(el(0x89, &tlv::uint_bytes(stored.len() as u64)));
    v.extend(el(0x86, stored));
    v
}

/// A blank A4 page carrying `n` rectangles whose drawings are replaced by
/// `fields` (inline in the record, or kept in an entry).
fn page_with(fields: &[u8], n: usize, in_entry: bool) -> Vec<u8> {
    let mut d = Document::blank(21000.0, 29700.0).unwrap();
    let shape: ezpzxdw_core::edit::Shape = serde_json::from_str(r#"{"type":"rect","stroke":0,"width":1}"#).unwrap();
    let ref_value = in_entry.then(|| d.add_entry(fields.to_vec(), false).1);
    for _ in 0..n {
        d.add_annotation(0, 1000.0, 1000.0, 5000.0, 5000.0, &shape).unwrap();
    }
    let recs: Vec<usize> = d.pages[0].objects.iter().map(|o| o.record).collect();
    for k in recs {
        let attrs = &mut d.records[k].attrs;
        attrs.retain(|a| a.tag != 7);
        match &ref_value {
            Some(v) => attrs.push(Attr { class: 0xc0, tag: 7, value: v.clone() }),
            None => attrs.push(Attr { class: 0x80, tag: 7, value: fields.to_vec() }),
        }
    }
    d.save_fresh().unwrap()
}

/// A blank document whose properties block is replaced by `stored`, said
/// to expand to `expanded` bytes.
fn with_properties(stored: &[u8], expanded: u64) -> Vec<u8> {
    let blank = Document::blank(21000.0, 29700.0).unwrap();
    let (hv, hl) = blank.container.header;
    let header = &blank.bytes[..hv + hl];
    let mut t = Vec::new();
    t.extend(el(0x80, &[0]));
    t.extend(el(0x82, &[0]));
    t.extend(el(0x83, &tlv::uint_bytes(expanded)));
    t.extend(el(0x84, &tlv::uint_bytes(stored.len() as u64)));
    t.extend(el(0x85, &tlv::i32_bytes(tlv::check(stored))));
    let self_len = (t.len() + 6) as u32;
    t.extend(el(0x86, &self_len.to_le_bytes()));
    let mut body = el(0x63, stored);
    body.extend(el(0x68, &t));
    let mut out = header.to_vec();
    out.extend(el(0x61, &body));
    out
}

/// A DIB header (BITMAPINFOHEADER + one palette colour).
fn bmi(w: i32, h: i32, bpp: u16, compression: u32) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&40u32.to_le_bytes());
    v.extend_from_slice(&w.to_le_bytes());
    v.extend_from_slice(&h.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&bpp.to_le_bytes());
    v.extend_from_slice(&compression.to_le_bytes());
    v.extend_from_slice(&[0; 12]);
    v.extend_from_slice(&1u32.to_le_bytes()); // colours used
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&[0, 0, 0, 0]);
    v
}

const GIB4: u64 = 0xf000_0000;

pub fn case(name: &str) -> Option<Vec<u8>> {
    Some(match name {
        // the properties block claims 3.75 GiB and expands to it
        "props-bomb" => with_properties(&bomb(GIB4), GIB4),
        // the properties block claims 3.75 GiB and holds a few bytes
        "props-claim" => {
            let d = Document::blank(21000.0, 29700.0).unwrap();
            let stored = d.container.properties_stored(&d.bytes).unwrap().to_vec();
            with_properties(&stored, GIB4)
        }
        // a drawing that claims 3.75 GiB and expands to it (inline / entry)
        "drawing-bomb" => page_with(&drawing_fields(4, Some(GIB4), &bomb(GIB4)), 1, false),
        "entry-bomb" => page_with(&drawing_fields(4, Some(GIB4), &bomb(GIB4)), 1, true),
        // 16 pictures of 10950 × 10950 pixels from 60 bytes each
        "image-size" => {
            let mut d = bmi(10950, 10950, 8, 1);
            d.extend_from_slice(&[0, 1]); // RLE8: end of bitmap
            page_with(&drawing_fields(7, None, &d), 16, false)
        }
        // a stored DIB whose compressed bits claim 3.75 GiB (dib.rs)
        "dib-bits" => {
            let mut d = bmi(1000, 1000, 24, 0);
            let b = bomb(GIB4);
            for v in [1u32, b.len() as u32, GIB4 as u32, 1000] {
                d.extend_from_slice(&v.to_le_bytes());
            }
            d.extend_from_slice(&b);
            page_with(&drawing_fields(7, None, &d), 1, false)
        }
        // counts not backed by data: POLYPOLYGON16 and EXTTEXTOUTW
        "counts" => {
            let mut e = Vec::new();
            e.extend(rec(91, &[0, 0, 100, 100, 0x1000_0000, 0x1000_0000, 3]));
            let mut t = vec![0u32; 17];
            t[9] = 0x1000_0000; // characters
            t[10] = 76;
            t[16] = 76; // dx
            e.extend(rec(84, &t));
            e.extend(eof());
            let mut all = emf_header(4, 88 + e.len() as u32);
            all.extend(e);
            page_with(&drawing_fields(4, None, &all), 1, false)
        }
        // 6 million LINETO records, 96 MB once expanded, from 1.3 MB
        "records" => {
            let n = 6_000_000u32;
            let line = rec(54, &[100, 100]);
            let mut prefix = emf_header(n + 2, 88 + 16 * n + 20);
            prefix.extend(&line);
            let fill = 88 + 16 * n as usize;
            let stream = repeat_stream(&prefix, 16, fill, &eof());
            page_with(&drawing_fields(4, Some((fill + 20) as u64), &stream), 1, false)
        }
        // one POLYLINE16 of 40 million points, 160 MB once expanded
        "points" => {
            let n = 40_000_000u32;
            let size = 28 + 4 * n;
            let mut prefix = emf_header(3, 88 + size + 20);
            let mut r = rec(87, &[0, 0, 100, 100, n]);
            r[4..8].copy_from_slice(&size.to_le_bytes());
            prefix.extend(r);
            prefix.push(0); // first point byte; the rest copy it
            let fill = 88 + size as usize;
            let stream = repeat_stream(&prefix, 1, fill, &eof());
            page_with(&drawing_fields(4, Some((fill + 20) as u64), &stream), 1, false)
        }
        _ => return None,
    })
}

const CASES: &[&str] = &["props-bomb", "props-claim", "drawing-bomb", "entry-bomb", "image-size", "dib-bits", "counts", "records", "points"];

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(|s| s.as_str()) {
        Some("list") => {
            for c in CASES {
                let b = case(c).unwrap();
                println!("{c}\t{} bytes", b.len());
            }
        }
        Some("write") => {
            let b = case(&a[2]).expect("case");
            std::fs::write(&a[3], b).unwrap();
        }
        Some("run") => {
            let b = case(&a[2]).expect("case");
            let size = b.len();
            PEAK.store(NOW.load(Ordering::Relaxed), Ordering::Relaxed);
            let base = NOW.load(Ordering::Relaxed);
            let t = std::time::Instant::now();
            let r = Document::open(b).and_then(|d| {
                let mut items = 0;
                for k in 0..d.pages.len() {
                    items += d.render(k)?.items.len();
                }
                Ok((d.pages.len(), items))
            });
            let ms = t.elapsed().as_millis();
            let peak = (PEAK.load(Ordering::Relaxed) - base) as f64 / 1048576.0;
            let what = match r {
                Ok((p, i)) => format!("ok: {p} page(s), {i} drawing items"),
                Err(e) => format!("error: {e}"),
            };
            println!("{}\t{size} bytes\t{ms} ms\t{peak:.0} MB\t{what}", a[2]);
        }
        _ => eprintln!("hostile list | run <case> | write <case> <out>"),
    }
}
