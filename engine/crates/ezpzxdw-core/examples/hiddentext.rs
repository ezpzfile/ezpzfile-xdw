//! Which text a page leaves out because something covers it.
//! `cargo run --release --example hiddentext -- <file> [page]`

use ezpzxdw_core::doc::Document;
use ezpzxdw_core::gfx::Item;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut d = Document::open(std::fs::read(&a[1]).unwrap()).unwrap();
    let pages: Vec<usize> = match a.get(2) {
        Some(p) => vec![p.parse().unwrap()],
        None => (0..d.pages.len()).collect(),
    };
    for k in pages {
        d.include_hidden = true;
        let all = d.render(k).unwrap();
        d.include_hidden = false;
        let kept = d.render(k).unwrap();
        if kept.hidden_chars == 0 {
            continue;
        }
        println!("page {}: {} characters covered", k + 1, kept.hidden_chars);
        let runs = |disp: &ezpzxdw_core::gfx::Display| -> Vec<(f32, f32, String)> {
            disp.items.iter().filter_map(|i| match i {
                Item::Text { x, y, text, .. } => Some((*x, *y, text.clone())),
                _ => None,
            }).collect()
        };
        let after = runs(&kept);
        for (ti, it) in all.items.iter().enumerate() {
            let Item::Text { x, y, text: t, size, .. } = it else { continue };
            if after.iter().any(|(_, _, u)| u == t) {
                continue;
            }
            println!("  at ({x:.0}, {y:.0}) size {size:.0}: {t:?}");
            if std::env::var("WHY").is_err() {
                continue;
            }
            for (ci, c) in all.items.iter().enumerate().skip(ti + 1) {
                let (what, b) = match c {
                    Item::Fill { path, color, clip, .. } => (format!("fill #{color:06x} clip {clip}"), path.bounds()),
                    Item::Image { m, image, .. } => (format!("image {image} {:?}", all.images[*image as usize].data_kind()), Some([m[4], m[5], m[4] + m[0], m[5] + m[3]])),
                    _ => continue,
                };
                if let Some(b) = b {
                    if b[0] <= *x && b[2] >= *x && b[1] <= *y - size * 0.3 && b[3] >= *y - size * 0.3 {
                        println!("     covered by item {ci}: {what} {b:?}");
                    }
                }
            }
        }
    }
}
