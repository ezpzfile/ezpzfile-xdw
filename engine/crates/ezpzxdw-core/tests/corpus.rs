//! Whole-corpus checks. Uses the local sample files when `EZPZXDW_CORPUS`
//! points at a folder of .xdw / .xbd files (see corpus/manifest.tsv).

use ezpzxdw_core::container::Container;
use ezpzxdw_core::doc::Document;
use ezpzxdw_core::edit::Shape;
use ezpzxdw_core::{props, tlv, write};

/// The records a save writes: renumbered for a file written anew, or as
/// `to_write` gives them when it appends (documents with a signature).
fn written(d: &Document) -> Vec<props::Record> {
    if d.is_signed() {
        d.to_write().0
    } else {
        d.fresh_parts().unwrap().records
    }
}

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
            // every signature reads back its module from the properties
            for (oi, o) in d.pages[k].objects.iter().enumerate() {
                if o.kind_name == "signature" {
                    let s = d.signature_of(k, oi).unwrap_or_else(|| panic!("{name}: signature info"));
                    assert!(!s.module.is_empty(), "{name}: signature module");
                }
            }
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
        // pictures: DocuWorks' bitmap annotation and our see-through one;
        // then switch the first and resize the second
        let mut px = vec![255u8; 8 * 8 * 4];
        for k in 0..8 {
            px[(k * 8 + k) * 4 + 1] = 0;
            px[(k * 8 + k) * 4 + 2] = 0;
        }
        px[7] = 0; // a transparent pixel becomes white
        let pa = d.add_picture(0, 2000.0, 3000.0, 3000.0, 3000.0, &px, 8, 8, false).unwrap();
        let pb = d.add_picture(0, 6000.0, 3000.0, 3000.0, 3000.0, &px, 8, 8, true).unwrap();
        assert_eq!((d.picture_see_through(0, pa), d.picture_see_through(0, pb)), (Some(false), Some(true)), "{name}");
        d.set_picture(0, pa, 2000.0, 3000.0, 3000.0, 3000.0, true).unwrap();
        d.set_picture(0, pb, 6000.0, 3000.0, 1500.0, 4500.0, false).unwrap();
        assert_eq!((d.picture_see_through(0, pa), d.picture_see_through(0, pb)), (Some(true), Some(false)), "{name}");
        assert_eq!((d.pages[0].objects[pb].w, d.pages[0].objects[pb].h), (1500, 4500), "{name}");
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
        assert_eq!(back.records, written(&d), "{name}");
        assert_eq!(back.pages.len(), n, "{name}");
        let first_with_annots = if moved { 1 } else { 0 };
        let shapes_back: Vec<_> = (0..back.pages[first_with_annots].objects.len()).filter_map(|o| back.shape_of(first_with_annots, o)).collect();
        assert!(shapes_back.len() >= shapes.len(), "{name}: annotations read back");
        let pg = &back.pages[first_with_annots];
        let pics: Vec<_> = pg.objects.iter().filter(|o| o.kind_name == "picture").rev().take(2).collect();
        assert_eq!(pics.len(), 2, "{name}: pictures read back");
        for o in pics {
            let img = back.picture_image(o).unwrap_or_else(|| panic!("{name}: picture pixels"));
            let ezpzxdw_core::gfx::ImageData::Rgba(p) = &img.data else { panic!("{name}: rgba") };
            assert_eq!((img.w, img.h, &p[..8]), (8, 8, &[255, 0, 0, 255, 255, 255, 255, 255][..]), "{name}");
        }
        let disp = back.render(first_with_annots).unwrap();
        assert!(disp.items.iter().any(|it| matches!(it, ezpzxdw_core::gfx::Item::Image { mul: true, .. })), "{name}: see-through picture multiplies");
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
        let sticky = Shape::Sticky { text: "付箋\n2行目".into(), size: 12.0, color: 0, background: 0xffff64 };
        d.add_annotation(0, 1500.0, 1500.0, 0.0, 0.0, &sticky).unwrap();
        let boxed = Shape::Text { text: "枠つき".into(), size: 10.0, color: 0, bold: false, background: Some(0xfff59d), frame: Some(0xc7bf7a) };
        d.add_annotation(0, 1500.0, 5000.0, 0.0, 0.0, &boxed).unwrap();
        let stamp4 = Shape::Stamp { top: "EZPZ".into(), date: "2026.10.01".into(), bottom: "総務".into(), color: 0x131a2e };
        d.add_annotation(0, 12000.0, 1500.0, 0.0, 0.0, &stamp4).unwrap();
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
        assert_eq!(back.records, written(&d), "{name}");
        assert_eq!(back.pages.len(), expect, "{name}");
        // the stamp and the sticky note read back as drawn
        let shapes: Vec<Shape> = d.pages.iter().enumerate().flat_map(|(p, pg)| (0..pg.objects.len()).filter_map(|o| back.shape_of(p, o)).collect::<Vec<_>>()).collect();
        assert!(shapes.contains(&stamp), "{name}: stamp");
        assert!(shapes.contains(&sticky), "{name}: sticky");
        assert!(shapes.contains(&boxed), "{name}: text with background");
        assert!(shapes.contains(&stamp4), "{name}: stamp with a four-digit year");
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

/// Written anew, every file reads back as one segment that draws exactly the
/// same, keeps its check values, and writing it anew again changes nothing.
#[test]
fn every_file_writes_anew_and_draws_the_same() {
    for (name, b) in corpus() {
        let d = Document::open(b.clone()).unwrap();
        let fresh = d.save_fresh().unwrap_or_else(|e| panic!("{name}: {e}"));
        let back = Document::open(fresh.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(back.container.segments.len(), 1, "{name}");
        assert_eq!(back.container.generation, d.container.generation, "{name}");
        assert_eq!(fresh[..d.container.header.0 + d.container.header.1], b[..d.container.header.0 + d.container.header.1], "{name}: header");
        assert_eq!(back.pages.len(), d.pages.len(), "{name}");
        assert_eq!(back.binder_docs().len(), d.binder_docs().len(), "{name}");
        for k in 0..d.pages.len() {
            let (a, z) = (d.render(k).unwrap(), back.render(k).unwrap());
            assert_eq!(format!("{a:?}"), format!("{z:?}"), "{name} page {}", k + 1);
        }
        let c = Container::parse(&fresh).unwrap();
        assert_eq!(tlv::check(c.properties_stored(&fresh).unwrap()), c.trailer.check, "{name}: properties check value");
        for e in c.entries(&fresh).unwrap() {
            assert_eq!(e.check, Some(tlv::check(&fresh[e.body_range.0..e.body_range.0 + e.body_range.1])), "{name}: entry check value");
        }
        assert_eq!(back.save_fresh().unwrap(), fresh, "{name}: writing anew again");
    }
}

/// A deleted page leaves nothing behind when the file is written anew: the
/// entries only it used are gone (appending keeps them).
#[test]
fn a_deleted_page_is_gone_from_the_file() {
    let mut checked = 0;
    for (name, b) in corpus() {
        if b.len() > 4_000_000 {
            continue;
        }
        let mut d = Document::open(b.clone()).unwrap();
        if d.pages.len() < 2 || d.is_signed() || d.is_binder() {
            continue;
        }
        // entries page 1 refers to, and that nothing else refers to
        let p = d.pages[0].record;
        let end = (p + 1..d.records.len()).find(|&j| d.records[j].depth <= d.records[p].depth).unwrap_or(d.records.len());
        let refs = |recs: &[props::Record]| -> Vec<u32> {
            recs.iter().flat_map(|r| r.attrs.iter()).filter(|a| a.class & 0xc0 == 0xc0).filter_map(|a| write::read_ref(&a.value)).map(|(n, _)| n).collect()
        };
        let mine = refs(&d.records[p..end]);
        let others: Vec<u32> = refs(&d.records[..p]).into_iter().chain(refs(&d.records[end..])).collect();
        let only: Vec<u32> = mine.into_iter().filter(|n| !others.contains(n)).collect();
        // a body long enough to be found only where it is stored
        let Some(body) = only.iter().filter_map(|&n| d.entry_body(n as usize)).filter(|b| b.len() >= 64).map(|b| b.to_vec()).next() else { continue };
        d.delete_page(0).unwrap();
        let has = |hay: &[u8]| hay.windows(body.len()).any(|w| w == &body[..]);
        assert!(has(&d.save_append().unwrap()), "{name}: appending keeps the deleted page");
        let fresh = d.save_fresh().unwrap();
        assert!(!has(&fresh), "{name}: the deleted page is still in the file written anew");
        assert_eq!(Document::open(fresh).unwrap().pages.len(), d.pages.len(), "{name}");
        checked += 1;
    }
    if !corpus().is_empty() {
        assert!(checked > 5, "only {checked} files checked");
    }
}

/// A new document takes everything the editor does and saves as one segment.
#[test]
fn a_new_document_from_scratch() {
    use ezpzxdw_core::pages::{jpeg_size, Jpeg};
    let mut d = Document::blank(21000.0, 29700.0).unwrap();
    let shapes = [
        Shape::Text { text: "新規文書\n二行目".into(), size: 12.0, color: 0xd40000, bold: true, background: None, frame: None },
        Shape::Rect { stroke: None, width: 1.0, fill: Some(0xffe14d), highlight: true },
        Shape::Ellipse { stroke: Some(0x00a36c), width: 3.0, fill: None },
        Shape::Line { points: vec![(100.0, 100.0), (3000.0, 1500.0)], color: 0, width: 1.0 },
        Shape::Stamp { top: "受付".into(), date: "'26.10.07".into(), bottom: "総務".into(), color: 0xe60012 },
        Shape::Sticky { text: "付箋".into(), size: 12.0, color: 0, background: 0xffff64 },
    ];
    for (k, s) in shapes.iter().enumerate() {
        d.add_annotation(0, 1500.0 + 300.0 * k as f64, 1500.0 + 3500.0 * k as f64, 4000.0, 1600.0, s).unwrap();
    }
    let jpeg = include_bytes!("data/tiny.jpg");
    let (jw, jh) = jpeg_size(jpeg).unwrap();
    d.insert_image_page(1, 29700.0, 21000.0, &Jpeg { data: jpeg, w: jw, h: jh }, None, None).unwrap();
    d.insert_blank_page(2, 25700.0, 36400.0).unwrap();
    d.rotate_page(2, 1).unwrap();
    let saved = d.save().unwrap();
    let back = Document::open(saved).unwrap();
    assert_eq!(back.container.segments.len(), 1);
    assert_eq!(back.pages.len(), 3);
    assert_eq!((back.pages[2].w, back.pages[2].h), (36400, 25700));
    for s in &shapes {
        assert!((0..back.pages[0].objects.len()).any(|o| back.shape_of(0, o).as_ref() == Some(s)), "{s:?}");
    }
    for k in 0..3 {
        assert!(back.render(k).unwrap().skipped.is_empty());
    }
    assert_eq!(back.render(1).unwrap().images.len(), 1);
}
