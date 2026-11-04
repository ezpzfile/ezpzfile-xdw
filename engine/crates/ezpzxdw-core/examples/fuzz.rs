//! Field fuzzer: changes numbers and bytes in real documents and checks that
//! reading, drawing and saving never panic, and how long and how much memory
//! the worst case takes.
//!
//! `cargo run --release --example fuzz -- <corpus folder> <runs> [seed]`
//! `FUZZ_RUN=<n>`: build the runs the same way but run only run n, uncaught
//! (to see where a reported panic happens; the file goes to /tmp/fuzz-case.xdw).
//!
//! Each run takes a sample file and one of:
//! - raw: bytes or 32-bit fields of the file itself changed
//! - drawing: 32-bit fields of one page drawing (EMF / WMF / DIB) changed,
//!   stored back uncompressed so the change reaches the drawing code
//! - packed: bytes of a compressed drawing changed (the -lh5- decoder)
//! - props: bytes of an attribute value in the object tree changed

use ezpzxdw_core::doc::{Document, Drawing};
use ezpzxdw_core::props::Attr;
use ezpzxdw_core::{tlv, write};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

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

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
    fn value(&mut self) -> u32 {
        const SPECIAL: [u32; 12] = [0, 1, 2, 0x7f, 0xff, 0xffff, 0x10000, 0x7fff_ffff, 0x8000_0000, 0xffff_fff0, 0xffff_ffff, 0x0100_0000];
        match self.below(3) {
            0 => SPECIAL[self.below(SPECIAL.len())],
            1 => self.next() as u32,
            _ => (self.next() % 4096) as u32,
        }
    }
}

fn mutate_fields(b: &mut [u8], r: &mut Rng, from: usize) {
    if b.len() < from + 4 {
        return;
    }
    for _ in 0..1 + r.below(6) {
        let at = from + r.below((b.len() - from - 4) / 4 + 1) * 4;
        let at = at.min(b.len() - 4);
        if r.below(4) == 0 {
            b[at + r.below(4)] = r.next() as u8;
        } else {
            b[at..at + 4].copy_from_slice(&r.value().to_le_bytes());
        }
    }
}

fn drawing_data(d: &Document, record: usize) -> Option<(u8, Vec<u8>)> {
    let fields = match d.drawing(record)? {
        Drawing::Entry(i) => d.entry_body(i)?.to_vec(),
        Drawing::Inline(v) => v,
    };
    let c = ezpzxdw_core::doc::Content::of_fields(&fields)?;
    Some((c.kind as u8, c.data))
}

fn fields(kind: u8, stored: &[u8], expanded: Option<usize>) -> Vec<u8> {
    let mut v = write::element(0x80, &[kind]);
    if let Some(n) = expanded {
        v.extend(write::element(0x81, &tlv::uint_bytes(n as u64)));
        v.extend(write::element(0x8a, &[1]));
    }
    v.extend(write::element(0x89, &tlv::uint_bytes(stored.len() as u64)));
    v.extend(write::element(0x86, stored));
    v
}

/// Read the file, draw `page` (or every page up to 4), get the text, and
/// sometimes save.
fn exercise(bytes: Vec<u8>, page: Option<usize>, save: bool) -> (bool, usize) {
    match Document::open(bytes) {
        Ok(d) => {
            let pages: Vec<usize> = match page {
                Some(p) if p < d.pages.len() => vec![p],
                _ => (0..d.pages.len().min(4)).collect(),
            };
            let mut errs = 0;
            let mut shown = Vec::new();
            for k in pages {
                match d.render(k) {
                    Ok(x) => shown.push(x),
                    Err(_) => errs += 1,
                }
            }
            let _ = ezpzxdw_core::pdf::to_pdf(&shown, &[], "f");
            if save {
                if let Ok(b) = d.save() {
                    let _ = Document::open(b);
                }
            }
            (true, errs)
        }
        Err(_) => (false, 1),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = &a[1];
    let runs: usize = a[2].parse().unwrap();
    let mut r = Rng(a.get(3).and_then(|s| s.parse().ok()).unwrap_or(0x9e37_79b9_7f4a_7c15));
    let mut seeds: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| std::fs::metadata(p).map(|m| m.len() < 3_000_000).unwrap_or(false))
        .map(|p| (p.file_name().unwrap().to_string_lossy().to_string(), std::fs::read(&p).unwrap()))
        .filter(|(_, b)| Document::open(b.clone()).is_ok())
        .collect();
    seeds.sort();
    let docs: Vec<Document> = seeds.iter().map(|(_, b)| Document::open(b.clone()).unwrap()).collect();
    std::panic::set_hook(Box::new(|_| {}));
    let (mut panics, mut errors, mut worst_ms, mut worst_mb) = (0usize, 0usize, 0u128, 0usize);
    let mut worst_what = String::new();
    let mut by_kind = [0usize; 4];
    let t_all = std::time::Instant::now();
    for run in 0..runs {
        let s = r.below(seeds.len());
        let (name, orig) = &seeds[s];
        let kind = r.below(4);
        by_kind[kind] += 1;
        let save = run % 10 == 0;
        // build the changed file
        let (bytes, page) = match kind {
            0 => {
                let mut b = orig.clone();
                mutate_fields(&mut b, &mut r, 0);
                (b, None)
            }
            _ => {
                let mut d = docs[s].clone();
                let objs: Vec<(usize, usize)> = d.pages.iter().enumerate().flat_map(|(p, pg)| pg.objects.iter().map(move |o| (p, o.record))).collect();
                if objs.is_empty() {
                    continue;
                }
                let (p, rec) = objs[r.below(objs.len())];
                match kind {
                    1 | 2 => {
                        let Some((k, mut data)) = drawing_data(&d, rec) else { continue };
                        let body = if kind == 1 {
                            mutate_fields(&mut data, &mut r, 0);
                            fields(k, &data, None)
                        } else {
                            let n = data.len();
                            let mut packed = ezpzxdw_core::lzh::compress(&data);
                            for _ in 0..1 + r.below(4) {
                                let i = r.below(packed.len());
                                packed[i] = r.next() as u8;
                            }
                            fields(k, &packed, Some(if r.below(4) == 0 { r.value() as usize } else { n }))
                        };
                        let attrs = &mut d.records[rec].attrs;
                        attrs.retain(|a| a.tag != 7);
                        attrs.push(Attr { class: 0x80, tag: 7, value: body });
                    }
                    _ => {
                        // one attribute anywhere in the tree
                        let ri = r.below(d.records.len());
                        let n = d.records[ri].attrs.len();
                        if n == 0 {
                            continue;
                        }
                        let ai = r.below(n);
                        let v = &mut d.records[ri].attrs[ai].value;
                        if v.is_empty() {
                            continue;
                        }
                        for _ in 0..1 + r.below(4) {
                            let i = r.below(v.len());
                            v[i] = if r.below(2) == 0 { r.next() as u8 } else { [0, 0x7f, 0x80, 0xff][r.below(4)] };
                        }
                    }
                }
                d.refresh();
                match d.save_fresh().or_else(|_| d.save_append()) {
                    Ok(b) => (b, Some(p)),
                    Err(_) => continue,
                }
            }
        };
        // FUZZ_RUN=n: build every run the same way, but only run run n, without catching its panic
        if let Some(only) = std::env::var("FUZZ_RUN").ok().and_then(|v| v.parse::<usize>().ok()) {
            if run == only {
                let _ = std::panic::take_hook();
                std::fs::write("/tmp/fuzz-case.xdw", &bytes).ok();
                exercise(bytes, page, save);
                println!("run {run}: no panic");
                return;
            }
            continue;
        }
        let base = NOW.load(Ordering::Relaxed);
        PEAK.store(base, Ordering::Relaxed);
        let t = std::time::Instant::now();
        let res = std::panic::catch_unwind(move || exercise(bytes, page, save));
        let ms = t.elapsed().as_millis();
        let mb = (PEAK.load(Ordering::Relaxed) - base) >> 20;
        match res {
            Ok((_, e)) => errors += (e > 0) as usize,
            Err(p) => {
                panics += 1;
                let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                eprintln!("PANIC run {run} kind {kind} {name}: {msg}");
            }
        }
        if ms > worst_ms {
            worst_ms = ms;
            worst_what = format!("run {run} kind {kind} {name}");
        }
        worst_mb = worst_mb.max(mb);
        if run > 0 && run % 5000 == 0 {
            eprintln!("{run} runs, {panics} panics, worst {worst_ms} ms ({worst_what}), {worst_mb} MB, {:.0} s", t_all.elapsed().as_secs_f64());
        }
    }
    println!(
        "runs {runs} (raw {} drawing {} packed {} props {}), panics {panics}, runs with errors {errors}, worst time {worst_ms} ms ({worst_what}), worst memory {worst_mb} MB, total {:.0} s",
        by_kind[0], by_kind[1], by_kind[2], by_kind[3],
        t_all.elapsed().as_secs_f64()
    );
}
