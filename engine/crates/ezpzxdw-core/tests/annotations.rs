//! Arrowheads, markers, polygons, original data (原本) and new binders,
//! checked against files DocuWorks 10 made itself (its API, under Wine; see
//! tools/dwapi) and by writing, saving and reading back.

use ezpzxdw_core::doc::Document;
use ezpzxdw_core::edit::Shape;

fn open(b: &[u8]) -> Document {
    Document::open(b.to_vec()).unwrap()
}

/// The first annotation of page 1 as a shape.
fn first_shape(d: &Document) -> Shape {
    let k = d.pages[0].objects.iter().position(|o| o.kind_name != "page").unwrap();
    d.shape_of(0, k).unwrap()
}

#[test]
fn docuworks_marker_reads_as_a_marker() {
    let d = open(include_bytes!("data/dw-marker.bin"));
    let Shape::Marker { points, color, width, translucent } = first_shape(&d) else { panic!("not a marker") };
    assert_eq!(points.len(), 5);
    assert_eq!(points[0], (250.0, 250.0));
    assert_eq!((color, width, translucent), (0, 1.0, false));
}

#[test]
fn docuworks_polygons_read_as_polygons() {
    let d = open(include_bytes!("data/dw-polygon.bin"));
    let Shape::Polygon { points, closed, fill, stroke, .. } = first_shape(&d) else { panic!("not a polygon") };
    assert_eq!(points, vec![(200.0, 200.0), (3200.0, 200.0), (3700.0, 2200.0), (1200.0, 3200.0)]);
    assert!(closed);
    assert_eq!(fill, Some(0xffffff));
    assert_eq!(stroke, Some(0));

    // a polyline DocuWorks gave an arrowhead at the end (type 2, style 1)
    let d = open(include_bytes!("data/dw-polyline-arrow.bin"));
    let Shape::Polygon { closed, arrow, arrow_style, fill, .. } = first_shape(&d) else { panic!("not a polygon") };
    assert!(!closed);
    assert_eq!((arrow, arrow_style, fill), (2, 1, None));
}

#[test]
fn docuworks_line_arrowheads_read_back() {
    // our line, given arrowheads at both ends (style 1) by DocuWorks
    let d = open(include_bytes!("data/dw-line-arrows.bin"));
    let Shape::Line { arrow, arrow_style, .. } = first_shape(&d) else { panic!("not a line") };
    assert_eq!((arrow, arrow_style), (3, 1));
}

#[test]
fn arrowheads_are_where_docuworks_puts_them() {
    // DocuWorks 10: a line (500,500)→(5500,3500), 2 pt, style 1 or 2: the
    // end head is (5236,3227) (5500,3500) (5135,3396); style 0 (wide) is
    // (5258,2530) (5500,3500) (4530,3742)
    let pts = [(500.0, 500.0), (5500.0, 3500.0)];
    let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 25.0 && (a.1 - b.1).abs() < 25.0;
    let (_, end) = ezpzxdw_core::edit::arrow_heads(&pts, 2, 1, 2.0);
    let end = end.unwrap();
    assert!(near(end[0], (5236.0, 3227.0)) && end[1] == (5500.0, 3500.0) && near(end[2], (5135.0, 3396.0)), "{end:?}");
    let (start, end) = ezpzxdw_core::edit::arrow_heads(&pts, 3, 0, 2.0);
    let end = end.unwrap();
    assert!(near(end[0], (5258.0, 2530.0)) && near(end[2], (4530.0, 3742.0)), "{end:?}");
    // the start head: DocuWorks (764,773) (500,500) (865,604) for style 1
    let (start1, _) = ezpzxdw_core::edit::arrow_heads(&pts, 1, 1, 2.0);
    let s = start1.unwrap();
    assert!(near(s[0], (764.0, 773.0)) && s[1] == (500.0, 500.0) && near(s[2], (865.0, 604.0)), "{s:?}");
    assert!(start.is_some());
}

#[test]
fn new_shapes_save_and_read_back() {
    let shapes = vec![
        Shape::Line { points: vec![(1000.0, 800.0), (5800.0, 3200.0)], color: 0x0000ff, width: 2.0, arrow: 3, arrow_style: 2 },
        Shape::Marker { points: vec![(250.0, 250.0), (1250.0, 750.0), (4000.0, 2000.0)], color: 0x00ff00, width: 3.0, translucent: true },
        Shape::Polygon { points: vec![(200.0, 200.0), (3200.0, 200.0), (1200.0, 3200.0)], stroke: Some(0), width: 1.0, fill: Some(0xffcc99), closed: true, arrow: 0, arrow_style: 2 },
        Shape::Polygon { points: vec![(800.0, 800.0), (3000.0, 3000.0), (6000.0, 800.0)], stroke: Some(0xff0000), width: 2.0, fill: None, closed: false, arrow: 1, arrow_style: 0 },
    ];
    let mut d = Document::blank(21000.0, 29700.0).unwrap();
    for (k, s) in shapes.iter().enumerate() {
        d.add_annotation(0, 1500.0, 1500.0 + 5000.0 * k as f64, 7000.0, 4500.0, s).unwrap();
    }
    let b = d.save().unwrap();
    let d = open(&b);
    let got: Vec<Shape> = (0..d.pages[0].objects.len()).filter_map(|k| d.shape_of(0, k)).collect();
    assert_eq!(got, shapes);
    // every page still draws
    d.render(0).unwrap();
}

#[test]
fn original_data_from_docuworks_and_ours() {
    // DocuWorks keeps 204000 bytes of text in an entry, LHA-compressed
    let d = open(include_bytes!("data/dw-original-packed.bin"));
    let o = d.originals();
    assert_eq!(o.len(), 1);
    assert_eq!((o[0].name.as_str(), o[0].size), ("big.txt", 204000));
    assert_eq!(d.original_data(0).unwrap(), "hello world line\n".repeat(12000).into_bytes());

    // ours: small (in the record), compressible, not compressible
    let small = b"hello original\n".to_vec();
    let text = "原本のテキスト\n".repeat(3000).into_bytes();
    let noise: Vec<u8> = (0..50_000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
    let mut d = Document::blank(21000.0, 29700.0).unwrap();
    d.add_original("memo.txt", &small, 1_795_000_000).unwrap();
    d.add_original("見積書.xlsx", &text, 1_795_000_001).unwrap();
    d.add_original("noise.bin", &noise, 1_795_000_002).unwrap();
    let d = open(&d.save().unwrap());
    let names: Vec<String> = d.originals().into_iter().map(|o| o.name).collect();
    assert_eq!(names, ["memo.txt", "見積書.xlsx", "noise.bin"]);
    assert_eq!(d.original_data(0).unwrap(), small);
    assert_eq!(d.original_data(1).unwrap(), text);
    assert_eq!(d.original_data(2).unwrap(), noise);
    assert_eq!(d.originals()[1].time, Some(1_795_000_001));

    let mut d = d;
    d.delete_original(1).unwrap();
    let d = open(&d.save().unwrap());
    assert_eq!(d.originals().len(), 2);
    assert_eq!(d.original_data(1).unwrap(), noise);
}

#[test]
fn new_binders() {
    // DocuWorks' own binder of two documents
    let dw = open(include_bytes!("data/dw-binder.bin"));
    assert_eq!(dw.binder_docs().len(), 2);

    let mut b = Document::blank_binder("見積一式").unwrap();
    assert!(b.is_binder());
    assert_eq!(b.binder_name().as_deref(), Some("見積一式"));
    let page = Document::blank(21000.0, 29700.0).unwrap();
    b.add_binder_docs(0, &page, "表紙").unwrap();
    b.add_binder_docs(1, &dw, "").unwrap();
    let b = open(&b.save().unwrap());
    let names: Vec<String> = b.binder_docs().into_iter().map(|d| d.name).collect();
    assert_eq!(names, ["表紙", "blank", "marker"]);
    assert_eq!(b.pages.len(), 3);

    // a document into a binder of its own
    let mut d = Document::blank(21000.0, 29700.0).unwrap();
    d.add_annotation(0, 1000.0, 1000.0, 0.0, 0.0, &Shape::Text { text: "A".into(), size: 12.0, color: 0, bold: false, background: None, frame: None }).unwrap();
    let b = d.into_binder("まとめ", "第1章").unwrap();
    let b = open(&b.save().unwrap());
    assert_eq!(b.binder_docs()[0].name, "第1章");
    assert_eq!(b.pages[0].objects.iter().filter(|o| o.kind_name == "text").count(), 1);
}
