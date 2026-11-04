//! Crafted files that claim far more than they hold must fail quickly with
//! "too large or damaged", without allocating what they claim. The files are
//! built here in code; no attack files are kept in the repository.

use ezpzxdw_core::doc::Document;
use ezpzxdw_core::edit::Shape;
use ezpzxdw_core::error::Error;
use ezpzxdw_core::limits::Limits;
use ezpzxdw_core::props::Attr;
use ezpzxdw_core::{tlv, write};
use std::time::{Duration, Instant};

/// Generous for a debug build on a slow machine; the release build takes
/// well under a millisecond for each of these.
const QUICK: Duration = Duration::from_millis(500);

struct Bits {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits {
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
}

/// A `-lh5-` stream of blocks whose tables hold one symbol each, so codes
/// take no bits: about 16 MB of zeros per 7 bytes.
fn bomb(total: u64) -> Vec<u8> {
    let mut w = Bits { out: Vec::new(), acc: 0, n: 0 };
    let block = |w: &mut Bits, count: u32, c: u32| {
        w.put(count, 16);
        w.put(0, 5);
        w.put(0, 5);
        w.put(0, 9);
        w.put(c, 9);
        w.put(0, 4);
        w.put(0, 4);
    };
    block(&mut w, 1, 0);
    for _ in 0..total.div_ceil(0xffff * 256) {
        block(&mut w, 0xffff, 509);
    }
    w.put(0, 32);
    if w.n > 0 {
        let last = (w.acc << (8 - w.n)) as u8;
        w.out.push(last);
    }
    w.out
}

fn el(tag: u8, v: &[u8]) -> Vec<u8> {
    write::element(tag, v)
}

fn fields(kind: u8, expanded: Option<u64>, stored: &[u8]) -> Vec<u8> {
    let mut v = el(0x80, &[kind]);
    if let Some(n) = expanded {
        v.extend(el(0x81, &tlv::uint_bytes(n)));
    }
    v.extend(el(0x89, &tlv::uint_bytes(stored.len() as u64)));
    v.extend(el(0x86, stored));
    v
}

/// A blank page with `n` rectangles whose drawings are `body` (inline in
/// the record, or kept in an entry).
fn page_with(body: &[u8], n: usize, in_entry: bool) -> Vec<u8> {
    let mut d = Document::blank(21000.0, 29700.0).unwrap();
    let shape = Shape::Rect { stroke: Some(0), width: 1.0, fill: None, highlight: false };
    let r = in_entry.then(|| d.add_entry(body.to_vec(), false).1);
    for _ in 0..n {
        d.add_annotation(0, 1000.0, 1000.0, 5000.0, 5000.0, &shape).unwrap();
    }
    let recs: Vec<usize> = d.pages[0].objects.iter().map(|o| o.record).collect();
    for k in recs {
        let attrs = &mut d.records[k].attrs;
        attrs.retain(|a| a.tag != 7);
        attrs.push(match &r {
            Some(v) => Attr { class: 0xc0, tag: 7, value: v.clone() },
            None => Attr { class: 0x80, tag: 7, value: body.to_vec() },
        });
    }
    d.save_fresh().unwrap()
}

/// A blank document whose properties block is `stored`, said to expand to
/// `expanded` bytes.
fn with_properties(stored: &[u8], expanded: u64) -> Vec<u8> {
    let blank = Document::blank(21000.0, 29700.0).unwrap();
    let (hv, hl) = blank.container.header;
    let mut t = el(0x80, &[0]);
    t.extend(el(0x82, &[0]));
    t.extend(el(0x83, &tlv::uint_bytes(expanded)));
    t.extend(el(0x84, &tlv::uint_bytes(stored.len() as u64)));
    t.extend(el(0x85, &tlv::i32_bytes(tlv::check(stored))));
    let self_len = (t.len() + 6) as u32;
    t.extend(el(0x86, &self_len.to_le_bytes()));
    let mut body = el(0x63, stored);
    body.extend(el(0x68, &t));
    let mut out = blank.bytes[..hv + hl].to_vec();
    out.extend(el(0x61, &body));
    out
}

fn bmi(w: i32, h: i32, bpp: u16, compression: u32) -> Vec<u8> {
    let mut v = 40u32.to_le_bytes().to_vec();
    v.extend_from_slice(&w.to_le_bytes());
    v.extend_from_slice(&h.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&bpp.to_le_bytes());
    v.extend_from_slice(&compression.to_le_bytes());
    v.extend_from_slice(&[0; 12]);
    v.extend_from_slice(&1u32.to_le_bytes());
    v.extend_from_slice(&[0; 8]);
    v
}

fn emf(records: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = records.concat();
    let mut h = vec![0u8; 88];
    let put = |h: &mut Vec<u8>, at: usize, v: u32| h[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut h, 0, 1);
    put(&mut h, 4, 88);
    put(&mut h, 32, 21000);
    put(&mut h, 36, 29700);
    h[40..44].copy_from_slice(b" EMF");
    put(&mut h, 48, 88 + body.len() as u32);
    put(&mut h, 52, records.len() as u32 + 1);
    put(&mut h, 72, 4960);
    put(&mut h, 76, 7016);
    put(&mut h, 80, 210);
    put(&mut h, 84, 297);
    h.extend(body);
    h
}

fn rec(t: u32, body: &[u32]) -> Vec<u8> {
    let mut v = t.to_le_bytes().to_vec();
    v.extend_from_slice(&((8 + 4 * body.len()) as u32).to_le_bytes());
    for x in body {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v
}

/// A record of 10 bytes: shorter than its fixed fields.
fn short(t: u32) -> Vec<u8> {
    let mut v = t.to_le_bytes().to_vec();
    v.extend_from_slice(&12u32.to_le_bytes());
    v.extend_from_slice(&[0, 0, 0, 0]);
    v[4..8].copy_from_slice(&10u32.to_le_bytes());
    v.truncate(10);
    v
}

fn too_large<T: std::fmt::Debug>(r: Result<T, Error>) -> String {
    match r {
        Err(Error::TooLarge(why)) => why,
        other => panic!("expected TooLarge, got {other:?}"),
    }
}

const BIG: u64 = 0xf000_0000; // 3.75 GiB

/// (a) a properties block that claims 3.75 GiB (and really expands to it)
#[test]
fn a_huge_properties_block_is_refused() {
    for stored in [bomb(BIG), vec![0u8; 16]] {
        let b = with_properties(&stored, BIG);
        let t = Instant::now();
        let why = too_large(Document::open(b));
        assert!(t.elapsed() < QUICK, "{:?}", t.elapsed());
        assert!(why.contains("maxExpand"), "{why}");
    }
}

/// (b) a drawing that claims 3.75 GiB, inline or in an entry
#[test]
fn a_huge_drawing_is_refused() {
    for in_entry in [false, true] {
        let b = page_with(&fields(4, Some(BIG), &bomb(BIG)), 1, in_entry);
        let d = Document::open(b).unwrap();
        let t = Instant::now();
        let why = too_large(d.render(0));
        assert!(t.elapsed() < QUICK, "{:?}", t.elapsed());
        assert!(why.contains("maxExpand"), "{why}");
    }
}

/// A block that expands far more than real data does is refused even when
/// the size itself is allowed.
#[test]
fn an_unlikely_ratio_is_refused() {
    let b = page_with(&fields(4, Some(64 << 20), &bomb(64 << 20)), 1, false);
    let d = Document::open(b).unwrap();
    assert!(too_large(d.render(0)).contains("maxRatio"));
}

/// (c) pictures that claim far more pixels than they hold
#[test]
fn huge_pictures_are_refused() {
    // 16 pictures of 10950 × 10950 from a two-byte RLE stream each
    let mut dib = bmi(10950, 10950, 8, 1);
    dib.extend_from_slice(&[0, 1]);
    let d = Document::open(page_with(&fields(7, None, &dib), 16, false)).unwrap();
    let t = Instant::now();
    let why = too_large(d.render(0));
    assert!(t.elapsed() < QUICK, "{:?}", t.elapsed());
    assert!(why.contains("maxImagePixels"), "{why}");

    // a stored picture whose compressed bits claim 3.75 GiB
    let mut dib = bmi(1000, 1000, 24, 0);
    let b = bomb(BIG);
    for v in [1u32, b.len() as u32, BIG as u32, 1000] {
        dib.extend_from_slice(&v.to_le_bytes());
    }
    dib.extend_from_slice(&b);
    let d = Document::open(page_with(&fields(7, None, &dib), 1, false)).unwrap();
    let t = Instant::now();
    assert!(too_large(d.render(0)).contains("maxExpand"));
    assert!(t.elapsed() < QUICK);

    // pictures that each fit, but not together
    let mut dib = bmi(3000, 3000, 8, 1);
    dib.extend_from_slice(&[0, 1]);
    let d = Document::open_with(page_with(&fields(7, None, &dib), 8, false), &Limits { max_bytes: 100 << 20, ..Default::default() }).unwrap();
    assert!(too_large(d.render(0)).contains("maxBytes"));
}

/// Counts that the record does not hold are cut to what it holds.
#[test]
fn counts_without_data_are_cut() {
    let mut t = vec![0u32; 17];
    t[9] = 0x1000_0000;
    t[10] = 76;
    t[16] = 76;
    let e = emf(&[
        rec(91, &[0, 0, 100, 100, 0x1000_0000, 0x1000_0000, 3]), // POLYPOLYGON16
        rec(84, &t),                                              // EXTTEXTOUTW
        rec(87, &[0, 0, 100, 100, 0x4000_0000]),                  // POLYLINE16
        rec(75, &[]),                                             // EXTSELECTCLIPRGN, short
        short(70),                                                // GDICOMMENT, short
        short(82),                                                // EXTCREATEFONTINDIRECTW, short
        rec(14, &[0, 16, 20]),
    ]);
    let d = Document::open(page_with(&fields(4, None, &e), 1, false)).unwrap();
    let t = Instant::now();
    d.render(0).unwrap();
    assert!(t.elapsed() < QUICK, "{:?}", t.elapsed());
}

/// Records and points per page, and pages per document.
#[test]
fn records_points_and_pages() {
    let line = rec(54, &[100, 100]); // LINETO
    let e = emf(&vec![line; 5000]);
    let b = page_with(&fields(4, None, &e), 1, false);
    let d = Document::open_with(b.clone(), &Limits { max_records: 1000, ..Default::default() }).unwrap();
    assert!(too_large(d.render(0)).contains("maxRecords"));
    let d = Document::open_with(b, &Limits { max_points: 2000, ..Default::default() }).unwrap();
    assert!(too_large(d.render(0)).contains("maxPoints"));
    assert!(Document::open(page_with(&fields(4, None, &e), 1, false)).unwrap().render(0).is_ok());

    let mut d = Document::blank(21000.0, 29700.0).unwrap();
    for k in 0..4 {
        d.insert_blank_page(k, 21000.0, 29700.0).unwrap();
    }
    let b = d.save().unwrap();
    assert!(too_large(Document::open_with(b.clone(), &Limits { max_pages: 3, ..Default::default() })).contains("maxPages"));
    assert_eq!(Document::open(b).unwrap().pages.len(), 5);
}

#[test]
fn limits_from_json() {
    let l = Limits::from_json(r#"{"maxPages":300,"maxImagePixels":1e6}"#);
    assert_eq!(l.max_pages, 300);
    assert_eq!(l.max_image_pixels, 1_000_000);
    assert_eq!(l.max_bytes, Limits::default().max_bytes);
}
