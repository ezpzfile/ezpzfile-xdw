//! Text that a later opaque box or picture covers is left out of the text
//! and of the PDF text layer; a highlighter or a see-through picture does
//! not hide it.

use ezpzxdw_core::doc::Document;
use ezpzxdw_core::edit::Shape;

const NAME: &str = "山田太郎";

/// A page with a text annotation, and something placed over it.
fn page(cover: impl FnOnce(&mut Document)) -> Document {
    let mut d = Document::blank(21000.0, 29700.0).unwrap();
    let text = Shape::Text { text: format!("氏名 {NAME}"), size: 12.0, color: 0, bold: false, background: None, frame: None };
    d.add_annotation(0, 2000.0, 3000.0, 0.0, 0.0, &text).unwrap();
    cover(&mut d);
    // read back from the saved file, as a user would get it
    Document::open(d.save().unwrap()).unwrap()
}

fn text_box(d: &Document) -> (f64, f64, f64, f64) {
    let o = &d.pages[0].objects[0];
    (o.x as f64, o.y as f64, o.w as f64, o.h as f64)
}

fn black_box(d: &mut Document) {
    let (x, y, w, h) = text_box(d);
    let shape = Shape::Rect { stroke: None, width: 1.0, fill: Some(0), highlight: false };
    d.add_annotation(0, x - 100.0, y - 100.0, w + 200.0, h + 200.0, &shape).unwrap();
}

/// The text layer of a PDF: which characters its ToUnicode map knows.
fn pdf_chars(d: &Document) -> String {
    let pages: Vec<_> = (0..d.pages.len()).map(|k| d.render(k).unwrap()).collect();
    let pdf = ezpzxdw_core::pdf::to_pdf(&pages, &[], "t");
    let s = String::from_utf8_lossy(&pdf);
    let mut out = String::new();
    for line in s.lines() {
        // "<0001> <5C71>"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2 && parts[0].len() == 6 && parts[0].starts_with('<') && parts[1].starts_with('<') {
            let hex = parts[1].trim_matches(|c| c == '<' || c == '>');
            let units: Vec<u16> = (0..hex.len() / 4).filter_map(|i| u16::from_str_radix(&hex[4 * i..4 * i + 4], 16).ok()).collect();
            out.push_str(&String::from_utf16_lossy(&units));
        }
    }
    out
}

#[test]
fn nothing_over_the_text() {
    let d = page(|_| {});
    assert!(d.text()[0].contains(NAME));
    assert!(pdf_chars(&d).contains('郎'));
    assert_eq!(d.render(0).unwrap().hidden_chars, 0);
}

#[test]
fn a_black_box_annotation_hides_it() {
    let mut d = page(black_box);
    let t = d.text()[0].clone();
    assert!(!t.contains('山') && !t.contains('郎'), "{t:?}");
    let disp = d.render(0).unwrap();
    assert!(disp.hidden_chars >= NAME.chars().count(), "{}", disp.hidden_chars);
    let p = pdf_chars(&d);
    assert!(!p.contains('郎') && !p.contains('氏'), "{p:?}");
    // asked for: kept
    d.include_hidden = true;
    assert!(d.text()[0].contains(NAME));
    assert!(pdf_chars(&d).contains('郎'));
    assert_eq!(d.render(0).unwrap().hidden_chars, 0);
}

#[test]
fn a_box_over_part_of_the_line_hides_only_that_part() {
    let d = page(|d| {
        let (x, y, w, h) = text_box(d);
        // the right half of the box: the name, not 氏名
        let shape = Shape::Rect { stroke: None, width: 1.0, fill: Some(0), highlight: false };
        d.add_annotation(0, x + w * 0.45, y - 100.0, w * 0.55 + 100.0, h + 200.0, &shape).unwrap();
    });
    let t = d.text()[0].clone();
    assert!(t.contains("氏名"), "{t:?}");
    assert!(!t.contains('郎'), "{t:?}");
}

#[test]
fn an_opaque_picture_hides_it() {
    let d = page(|d| {
        let (x, y, w, h) = text_box(d);
        let px = vec![255u8; 8 * 8 * 4]; // white, fully opaque
        d.add_picture(0, x - 100.0, y - 100.0, w + 200.0, h + 200.0, &px, 8, 8, false).unwrap();
    });
    let t = d.text()[0].clone();
    assert!(!t.contains('郎'), "{t:?}");
    assert!(!pdf_chars(&d).contains('郎'));
}

#[test]
fn a_highlighter_or_a_see_through_picture_does_not() {
    let d = page(|d| {
        let (x, y, w, h) = text_box(d);
        let marker = Shape::Rect { stroke: None, width: 1.0, fill: Some(0xffe14d), highlight: true };
        d.add_annotation(0, x - 100.0, y - 100.0, w + 200.0, h + 200.0, &marker).unwrap();
        let px = vec![0u8; 8 * 8 * 4].into_iter().enumerate().map(|(i, v)| if i % 4 == 3 { 255 } else { v }).collect::<Vec<_>>();
        d.add_picture(0, x - 100.0, y - 100.0, w + 200.0, h + 200.0, &px, 8, 8, true).unwrap();
    });
    assert!(d.text()[0].contains(NAME));
    assert!(pdf_chars(&d).contains('郎'));
}

#[test]
fn a_box_drawn_before_the_text_does_not() {
    let mut d = Document::blank(21000.0, 29700.0).unwrap();
    let shape = Shape::Rect { stroke: None, width: 1.0, fill: Some(0xdddddd), highlight: false };
    d.add_annotation(0, 1000.0, 2000.0, 9000.0, 3000.0, &shape).unwrap();
    let text = Shape::Text { text: NAME.into(), size: 12.0, color: 0, bold: false, background: None, frame: None };
    d.add_annotation(0, 2000.0, 3000.0, 0.0, 0.0, &text).unwrap();
    assert!(d.text()[0].contains(NAME));
}

/// A page taken from a PDF keeps its text under the page picture, like a
/// scanned page with recognised text: the picture shows that very text.
#[test]
fn text_under_its_own_page_picture_is_kept() {
    use ezpzxdw_core::pages::{jpeg_size, Jpeg, TextRun};
    let mut d = Document::blank(21000.0, 29700.0).unwrap();
    let jpeg = include_bytes!("data/tiny.jpg");
    let (jw, jh) = jpeg_size(jpeg).unwrap();
    let run = TextRun { x: 2000.0, y: 3000.0, size: 500.0, width: 2000.0, text: NAME.into() };
    d.insert_image_page_text(0, 21000.0, 29700.0, &Jpeg { data: jpeg, w: jw, h: jh }, Some([0.0, 0.0, 21000.0, 29700.0]), None, &[run]).unwrap();
    assert!(d.text()[0].contains(NAME), "{:?}", d.text()[0]);
    // a black box annotation over it still hides it
    let shape = Shape::Rect { stroke: None, width: 1.0, fill: Some(0), highlight: false };
    d.add_annotation(0, 1500.0, 2500.0, 4000.0, 1500.0, &shape).unwrap();
    assert!(!d.text()[0].contains('郎'), "{:?}", d.text()[0]);
}
