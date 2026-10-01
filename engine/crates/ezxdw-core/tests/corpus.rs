//! Whole-corpus checks. Uses the local sample files when `EZXDW_CORPUS`
//! points at a folder of .xdw / .xbd files (see corpus/manifest.tsv).

use ezxdw_core::container::Container;
use ezxdw_core::doc::Document;
use ezxdw_core::edit::Shape;
use ezxdw_core::{props, tlv};

fn corpus() -> Vec<(String, Vec<u8>)> {
    let Ok(dir) = std::env::var("EZXDW_CORPUS") else {
        return Vec::new();
    };
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map(|x| x.eq_ignore_ascii_case("xdw") || x.eq_ignore_ascii_case("xbd")).unwrap_or(false))
        .collect();
    v.sort();
    v.into_iter().map(|p| (p.display().to_string(), std::fs::read(&p).unwrap())).collect()
}

#[test]
fn every_file_reads_and_its_check_values_hold() {
    for (name, b) in corpus() {
        let c = Container::parse(&b).unwrap_or_else(|e| panic!("{name}: {e}"));
        let stored = c.properties_stored(&b).unwrap();
        assert_eq!(tlv::check(stored), c.trailer.check, "{name}: properties check value");
        for e in c.entries(&b).unwrap() {
            if let Some(k) = e.check {
                assert_eq!(k, tlv::check(&b[e.body_range.0..e.body_range.0 + e.body_range.1]), "{name}: entry check value");
            }
        }
        // the object tree is written back byte for byte
        let p = c.properties(&b).unwrap();
        assert_eq!(props::write(&props::parse(&p).unwrap()), p, "{name}: properties round trip");
    }
}

#[test]
fn every_page_draws() {
    for (name, b) in corpus() {
        let d = Document::open(b).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!d.pages.is_empty(), "{name}: no pages");
        for k in 0..d.pages.len() {
            let disp = d.render(k).unwrap_or_else(|e| panic!("{name} page {}: {e}", k + 1));
            assert!(disp.skipped.is_empty(), "{name} page {}: {:?}", k + 1, disp.skipped);
        }
    }
}

#[test]
fn edits_save_and_read_back() {
    for (name, b) in corpus() {
        if b.len() > 4_000_000 {
            continue;
        }
        let mut d = Document::open(b.clone()).unwrap();
        // unchanged: the saved file has the same tree
        let same = d.save().unwrap();
        assert_eq!(Document::open(same).unwrap().records, d.records, "{name}");
        // add one of each, move, rotate, reorder
        let n = d.pages.len();
        let shapes = [
            Shape::Text { text: "EZPZ テスト\n2行目".into(), size: 12.0, color: 0xd40000, bold: false, background: None, frame: None },
            Shape::Rect { stroke: Some(0x0155ff), width: 2.0, fill: None, highlight: false },
            Shape::Rect { stroke: None, width: 1.0, fill: Some(0xffe14d), highlight: true },
            Shape::Ellipse { stroke: Some(0x00a36c), width: 3.0, fill: None },
            Shape::Line { points: vec![(100.0, 100.0), (3000.0, 1500.0)], color: 0, width: 1.0 },
        ];
        for (k, s) in shapes.iter().enumerate() {
            d.add_annotation(0, 1000.0 + 500.0 * k as f64, 2000.0 + 3000.0 * k as f64, 4000.0, 1600.0, s).unwrap();
        }
        let objs = d.pages[0].objects.len();
        d.move_object(0, objs - 1, 5000.0, 5000.0).unwrap();
        d.rotate_page(n - 1, 1).unwrap();
        if n > 1 {
            d.move_page(n - 1, 0).unwrap();
        }
        let saved = d.save().unwrap_or_else(|e| panic!("{name}: {e}"));
        let back = Document::open(saved).unwrap();
        assert_eq!(back.records, d.records, "{name}");
        assert_eq!(back.pages.len(), n, "{name}");
        let first_with_annots = if n > 1 { 1 } else { 0 };
        let shapes_back: Vec<_> = (0..back.pages[first_with_annots].objects.len()).filter_map(|o| back.shape_of(first_with_annots, o)).collect();
        assert!(shapes_back.len() >= shapes.len(), "{name}: annotations read back");
        for k in 0..back.pages.len() {
            let disp = back.render(k).unwrap();
            assert!(disp.skipped.is_empty(), "{name} page {}: {:?}", k + 1, disp.skipped);
        }
    }
}
