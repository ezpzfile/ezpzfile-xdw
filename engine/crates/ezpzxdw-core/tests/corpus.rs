//! Whole-corpus checks. Uses the local sample files when `EZPZXDW_CORPUS`
//! points at a folder of .xdw / .xbd files (see corpus/manifest.tsv).

use ezpzxdw_core::container::Container;
use ezpzxdw_core::doc::Document;
use ezpzxdw_core::edit::Shape;
use ezpzxdw_core::{props, tlv};

fn corpus() -> Vec<(String, Vec<u8>)> {
    let Ok(dir) = std::env::var("EZPZXDW_CORPUS") else {
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
        let mut moved = false;
        if n > 1 {
            // in a binder the last page of a document may not leave it
            let r = d.move_page(n - 1, 0);
            assert!(r.is_ok() || d.is_binder(), "{name}: {r:?}");
            moved = r.is_ok();
        }
        let saved = d.save().unwrap_or_else(|e| panic!("{name}: {e}"));
        let back = Document::open(saved).unwrap();
        assert_eq!(back.records, d.records, "{name}");
        assert_eq!(back.pages.len(), n, "{name}");
        let first_with_annots = if moved { 1 } else { 0 };
        let shapes_back: Vec<_> = (0..back.pages[first_with_annots].objects.len()).filter_map(|o| back.shape_of(first_with_annots, o)).collect();
        assert!(shapes_back.len() >= shapes.len(), "{name}: annotations read back");
        for k in 0..back.pages.len() {
            let disp = back.render(k).unwrap();
            assert!(disp.skipped.is_empty(), "{name} page {}: {:?}", k + 1, disp.skipped);
        }
    }
}

#[test]
fn new_pages_stamps_and_binder_documents() {
    use ezpzxdw_core::pages::{a4_for, jpeg_size, Jpeg, Thumb};
    let all = corpus();
    let Some((_, small)) = all.iter().find(|(n, _)| n.contains("codelibs-test_test_xdw_ver10")) else { return };
    let other = Document::open(small.clone()).unwrap();
    let jpeg = include_bytes!("data/tiny.jpg");
    let (jw, jh) = jpeg_size(jpeg).unwrap();
    let thumb = vec![200u8; 4 * 10 * 14];
    for (name, b) in &all {
        if b.len() > 4_000_000 {
            continue;
        }
        let mut d = Document::open(b.clone()).unwrap();
        let n = d.pages.len();
        let stamp = Shape::Stamp { top: "受付".into(), date: "'26.10.01".into(), bottom: "EZPZ".into(), color: 0xe60012 };
        d.add_annotation(0, 15000.0, 1500.0, 0.0, 0.0, &stamp).unwrap();
        let sticky = Shape::Text { text: "付箋".into(), size: 12.0, color: 0, bold: false, background: Some(0xfff59d), frame: Some(0xc7bf7a) };
        d.add_annotation(0, 1500.0, 1500.0, 0.0, 0.0, &sticky).unwrap();
        d.insert_blank_page(1.min(n), 21000.0, 29700.0).unwrap();
        let (pw, ph) = a4_for(jw, jh);
        d.insert_image_page(d.pages.len(), pw, ph, &Jpeg { data: jpeg, w: jw, h: jh }, None, Some(&Thumb { rgba: &thumb, w: 10, h: 14 })).unwrap();
        d.insert_pages_from(0, &other, &(0..other.pages.len()).collect::<Vec<_>>()).unwrap();
        let mut expect = n + 2 + other.pages.len();
        if d.is_binder() {
            let docs = d.binder_docs().len();
            d.add_binder_docs(1, &other, "追加").unwrap();
            d.rename_binder_doc(0, "一番目").unwrap();
            d.move_binder_doc(docs, 0).unwrap();
            assert_eq!(d.binder_docs()[1].name, "一番目", "{name}");
            assert_eq!(d.binder_docs()[2].name, "追加", "{name}");
            assert_eq!(d.binder_docs().len(), docs + 1, "{name}");
            expect += other.pages.len();
        }
        let saved = d.save().unwrap_or_else(|e| panic!("{name}: {e}"));
        let back = Document::open(saved).unwrap();
        assert_eq!(back.records, d.records, "{name}");
        assert_eq!(back.pages.len(), expect, "{name}");
        // the stamp and the sticky note read back as drawn
        let shapes: Vec<Shape> = d.pages.iter().enumerate().flat_map(|(p, pg)| (0..pg.objects.len()).filter_map(|o| back.shape_of(p, o)).collect::<Vec<_>>()).collect();
        assert!(shapes.contains(&stamp), "{name}: stamp");
        assert!(shapes.contains(&sticky), "{name}: sticky");
        for k in 0..back.pages.len() {
            let disp = back.render(k).unwrap();
            assert!(disp.skipped.is_empty(), "{name} page {}: {:?}", k + 1, disp.skipped);
        }
        // the picture page draws its picture
        let last = back.render(back.pages.len() - 1).unwrap();
        if !back.is_binder() {
            assert_eq!(last.images.len(), 1, "{name}: picture page");
        }
    }
}
