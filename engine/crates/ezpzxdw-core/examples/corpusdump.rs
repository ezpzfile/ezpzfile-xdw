//! For regression checks: one line per file and page with hashes of what
//! the engine produces (pages, drawing, text, PDF text layer, save).
//! `cargo run --release --example corpusdump -- <folder> > out.tsv`

use ezpzxdw_core::doc::Document;
use ezpzxdw_core::gfx::ImageData;
use std::hash::{DefaultHasher, Hash, Hasher};

fn h(b: &[u8]) -> String {
    let mut s = DefaultHasher::new();
    b.hash(&mut s);
    format!("{:016x}", s.finish())
}

fn main() {
    let dir = std::env::args().nth(1).expect("folder");
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();
    for p in files {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let b = std::fs::read(&p).unwrap();
        let t = std::time::Instant::now();
        let d = match Document::open(b) {
            // INCLUDE_HIDDEN=1: keep covered text, as before 0.3
            Ok(mut d) => {
                d.include_hidden = std::env::var("INCLUDE_HIDDEN").is_ok();
                d
            }
            Err(e) => {
                println!("{name}\topen-error\t{e}");
                continue;
            }
        };
        let pages = serde_json::to_string(&d.pages).unwrap();
        if std::env::var("USAGE").is_ok() {
            eprintln!("USAGE\t{name}\topen\t{}", serde_json::to_string(&ezpzxdw_core::limits::last_usage()).unwrap());
        }
        println!("{name}\tpages\t{}\t{}", d.pages.len(), h(pages.as_bytes()));
        let mut all = Vec::new();
        for k in 0..d.pages.len() {
            match d.render(k) {
                Ok(disp) => {
                    if std::env::var("USAGE").is_ok() {
                        eprintln!("USAGE\t{name}\tpage {k}\t{}\thidden {}", serde_json::to_string(&ezpzxdw_core::limits::last_usage()).unwrap(), disp.hidden_chars);
                    }
                    // without the field added in 0.3, so old and new runs compare
                    let js = serde_json::to_string(&disp).unwrap();
                    let js = js.replace(&format!(",\"hidden_chars\":{}", disp.hidden_chars), "");
                    let imgs: String = disp.images.iter().map(|i| match &i.data {
                        ImageData::Jpeg(v) | ImageData::Rgba(v) => h(v),
                    }).collect::<Vec<_>>().join(",");
                    println!("{name}\tpage {k}\t{}\t{}\t{}", h(js.as_bytes()), h(imgs.as_bytes()), h(disp.text().as_bytes()));
                    all.push(disp);
                }
                Err(e) => println!("{name}\tpage {k}\terror\t{e}"),
            }
        }
        let text: String = d.text().join("\u{c}");
        println!("{name}\ttext\t{}", h(text.as_bytes()));
        let pdf = ezpzxdw_core::pdf::to_pdf(&all, &[], "t");
        println!("{name}\tpdf\t{}", h(&pdf));
        let mut d2 = d.clone();
        match d2.save() {
            Ok(s) => println!("{name}\tsave\t{}", h(&s)),
            Err(e) => println!("{name}\tsave-error\t{e}"),
        }
        eprintln!("{name}\t{} ms", t.elapsed().as_millis());
    }
}
