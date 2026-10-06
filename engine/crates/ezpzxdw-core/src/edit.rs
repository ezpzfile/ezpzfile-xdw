//! Changes to a document: annotations (add, move, resize, change, delete)
//! and pages (rotate, delete, reorder), then saving as `.xdw`.
//!
//! Everything here changes only the object tree in the properties block;
//! saving appends one segment with the new tree (see `write`). Every
//! annotation we add or change gets a fresh drawing (`emfw`), because
//! DocuWorks shows the stored drawing, not the settings.

use crate::doc::{self, Document, K_CONTENT, K_ELLIPSE, K_LINE, K_PLACE, K_RECT, K_TEXT};
use crate::emfw::{self, Emf, UNIT};
use crate::error::{Error, Result};
use crate::lzh;
use crate::props::{self, Attr, Record};
use crate::tlv;
use crate::write::{self, element};
use serde::{Deserialize, Serialize};

/// What to draw for a new or changed annotation.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Shape {
    Text {
        text: String,
        /// Points.
        size: f64,
        /// 0xRRGGBB
        color: u32,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        background: Option<u32>,
        #[serde(default)]
        frame: Option<u32>,
    },
    Rect {
        #[serde(default)]
        stroke: Option<u32>,
        #[serde(default = "one")]
        width: f64,
        #[serde(default)]
        fill: Option<u32>,
        /// Fill like a highlighter (colour multiplies what is under it).
        #[serde(default)]
        highlight: bool,
    },
    Ellipse {
        #[serde(default)]
        stroke: Option<u32>,
        #[serde(default = "one")]
        width: f64,
        #[serde(default)]
        fill: Option<u32>,
    },
    /// Points relative to the box (1/100 mm).
    Line {
        points: Vec<(f64, f64)>,
        color: u32,
        #[serde(default = "one")]
        width: f64,
    },
    /// A date stamp (日付印): a circle split in three by two lines, with
    /// `top` above, `date` in the middle and `bottom` below.
    Stamp {
        #[serde(default)]
        top: String,
        date: String,
        #[serde(default)]
        bottom: String,
        #[serde(default = "stamp_red")]
        color: u32,
    },
    /// A sticky note (付箋): a coloured note with text on it, stored as
    /// DocuWorks stores one (the note, and a text annotation inside it).
    Sticky {
        text: String,
        /// Points.
        #[serde(default = "twelve")]
        size: f64,
        /// Text colour, 0xRRGGBB.
        #[serde(default)]
        color: u32,
        /// The note's colour, 0xRRGGBB.
        #[serde(default = "sticky_yellow")]
        background: u32,
    },
}

/// What a signature (8043) says about itself in the stored properties.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SignatureInfo {
    /// The signing module, as stored (e.g. "DocuWorks電子印鑑 (SHA1 …").
    pub module: String,
    /// The signature format version (`%sigver`), if present.
    pub version: Option<String>,
    /// A DocuWorks electronic seal (電子印鑑) rather than a PKI certificate.
    pub stamp: bool,
}

fn twelve() -> f64 {
    12.0
}

fn sticky_yellow() -> u32 {
    0xffff64
}

/// Where the text sits on a sticky note (1/100 mm from its corner), and the
/// note's shadow, as DocuWorks makes them.
const STICKY_PAD: f64 = 300.0;
const STICKY_SHADOW: f64 = 50.0;

/// Size of a sticky note for `text` at `size` points (1/100 mm).
pub fn sticky_box(text: &str, size: f64) -> (f64, f64) {
    let (tw, th) = text_box(text, size);
    ((tw + 2.0 * STICKY_PAD + STICKY_SHADOW).max(3000.0), (th + 2.0 * STICKY_PAD + STICKY_SHADOW).max(1500.0))
}

fn one() -> f64 {
    1.0
}

fn stamp_red() -> u32 {
    0xe60012
}

/// Default size of a date stamp (1/100 mm): 18 mm across.
pub const STAMP_SIZE: f64 = 1800.0;

/// Text font: what DocuWorks text annotations use by default.
pub const FACE: &str = "ＭＳ ゴシック";
const FACE_W: &str = "MS Gothic";

/// Line height for text annotations, relative to the font size.
const LINE: f64 = 1.25;
/// Text annotation margin (1/100 mm), as DocuWorks writes it.
const MARGIN: f64 = 170.0;

/// Character advance in em (estimate the viewer and we both follow, since
/// the drawing stores each advance).
pub fn advance(c: char) -> f64 {
    let u = c as u32;
    if u < 0x20 {
        0.0
    } else if u < 0x2000 || (0xff61..=0xff9f).contains(&u) {
        0.5
    } else {
        1.0
    }
}

/// Size of a text annotation box for `text` at `size` points (1/100 mm).
pub fn text_box(text: &str, size: f64) -> (f64, f64) {
    let em = size * 2540.0 / 72.0;
    let lines: Vec<&str> = text.split('\n').collect();
    let w = lines.iter().map(|l| l.chars().map(advance).sum::<f64>()).fold(0.0, f64::max) * em;
    let h = lines.len().max(1) as f64 * em * LINE;
    (w + 2.0 * MARGIN, h + 2.0 * MARGIN)
}

/// The drawing (EMF) for a shape in a `w` × `h` box (1/100 mm).
pub fn draw(shape: &Shape, w: f64, h: f64) -> Vec<u8> {
    let (uw, uh) = (emfw::units(w), emfw::units(h));
    let mut e = Emf::new(uw, uh);
    e.clip_to_box();
    match shape {
        Shape::Text { text, size, color, bold, background, frame } => {
            if background.is_some() || frame.is_some() {
                let fw = emfw::units(25.0).max(1);
                e.pen_brush(frame.map(|c| (c, fw)), *background);
                // the frame inside the box (GDI leaves out the right and
                // bottom edge, and the box clips)
                let i = if frame.is_some() { (fw + 1) / 2 } else { 0 };
                e.rectangle(i, i, uw - i + 1, uh - i + 1);
            }
            text_lines(&mut e, emfw::units(MARGIN), emfw::units(MARGIN), text, *size, *color, *bold);
        }
        Shape::Sticky { text, size, color, background } => {
            // shadow, the note with a grey edge, then the text
            let s = emfw::units(STICKY_SHADOW).max(1);
            e.pen_brush(None, Some(0x999999));
            e.rectangle(s, s, uw + 1, uh + 1);
            e.pen_brush(Some((0x666666, 1)), Some(*background));
            e.rectangle(0, 0, uw - s, uh - s);
            let m = emfw::units(STICKY_PAD + MARGIN);
            text_lines(&mut e, m, m, text, *size, *color, false);
        }
        Shape::Rect { stroke, width, fill, highlight } => {
            let wu = emfw::units(width * 100.0 / 2.835).max(1);
            // a highlighter multiplies its colour with the page
            // (R2_MASKPEN): text under it stays readable in DocuWorks
            if *highlight {
                e.mask_mode(true);
            }
            e.pen_brush(stroke.map(|c| (c, wu)), *fill);
            let i = if stroke.is_some() { wu / 2 } else { 0 };
            e.rectangle(i, i, uw - i, uh - i);
            if *highlight {
                e.mask_mode(false);
            }
        }
        Shape::Ellipse { stroke, width, fill } => {
            let wu = emfw::units(width * 100.0 / 2.835).max(1);
            e.pen_brush(stroke.map(|c| (c, wu)), *fill);
            let i = if stroke.is_some() { wu / 2 } else { 0 };
            e.ellipse(i, i, uw - i, uh - i);
        }
        Shape::Line { points, color, width } => {
            let wu = emfw::units(width * 100.0 / 2.835).max(1);
            e.pen_brush(Some((*color, wu)), None);
            let pts: Vec<(i32, i32)> = points.iter().map(|&(x, y)| (emfw::units(x), emfw::units(y))).collect();
            e.polyline(&pts);
        }
        Shape::Stamp { top, date, bottom, color } => draw_stamp(&mut e, uw, uh, top, date, bottom, *color),
    }
    e.finish(w, h)
}

/// Lines of text from (x, y) (drawing units), the way text annotations
/// lay them out.
fn text_lines(e: &mut Emf, x: i32, y: i32, text: &str, size: f64, color: u32, bold: bool) {
    let em_u = (size * 300.0 / 72.0).round() as i32;
    e.font(em_u, if bold { 700 } else { 400 }, false, FACE);
    e.text_color(color);
    let lh = (em_u as f64 * LINE).round() as i32;
    let top_pad = ((lh - em_u) / 2).max(0);
    for (k, line) in text.split('\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let adv: Vec<i32> = line.chars().map(|c| (advance(c) * em_u as f64).round() as i32).collect();
        e.text(x, y + k as i32 * lh + top_pad, line, &adv);
    }
}

/// The date stamp picture: an oval filling the box, two lines across it,
/// and three centred texts sized to fit their part of the oval.
fn draw_stamp(e: &mut Emf, uw: i32, uh: i32, top: &str, date: &str, bottom: &str, color: u32) {
    let (w, h) = (uw as f64, uh as f64);
    let pen = (w.min(h) * 0.035).max(2.0);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let (rx, ry) = (w / 2.0 - pen / 2.0, h / 2.0 - pen / 2.0);
    // half the width of the oval at height dy from the centre
    let chord = |dy: f64| rx * (1.0 - (dy / ry).powi(2)).max(0.0).sqrt();
    e.pen_brush(Some((color, pen.round() as i32)), None);
    let p = pen / 2.0;
    e.ellipse(p.round() as i32, p.round() as i32, (w - p).round() as i32, (h - p).round() as i32);
    let d = ry * 0.34; // the lines, above and below the centre
    for y in [cy - d, cy + d] {
        let c = chord(d) - pen * 0.3;
        e.polyline(&[((cx - c).round() as i32, y.round() as i32), ((cx + c).round() as i32, y.round() as i32)]);
    }
    e.text_color(color);
    // (text, centre y, tallest em, the y where the width is narrowest)
    let band = ry - d;
    let parts = [
        (top, cy - d - band * 0.45, band * 0.56, d + band * 0.70),
        (date, cy, 2.0 * d * 0.66, d * 0.66),
        (bottom, cy + d + band * 0.45, band * 0.56, d + band * 0.70),
    ];
    for (text, yc, em_max, dy_narrow) in parts {
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let ems: f64 = text.chars().map(advance).sum::<f64>().max(0.5);
        let room = 2.0 * chord(dy_narrow) * 0.92;
        let em = em_max.min(room / ems).max(4.0);
        let em_u = em.round() as i32;
        e.font(em_u, 400, false, FACE);
        let adv: Vec<i32> = text.chars().map(|c| (advance(c) * em).round() as i32).collect();
        let tw: i32 = adv.iter().sum();
        e.text((cx - tw as f64 / 2.0).round() as i32, (yc - em / 2.0).round() as i32, text, &adv);
    }
}

/// Today's date the way DocuWorks date stamps show it: `'26.10.01`.
pub fn stamp_date(year: i32, month: u32, day: u32) -> String {
    format!("'{:02}.{:02}.{:02}", year.rem_euclid(100), month, day)
}

/// A stamp date as DocuWorks keeps it: the character before the year,
/// year, month, day (`'26.10.01` → `'`, 26, 10, 01; `2026.10.01` → none,
/// 2026, 10, 01). Other text goes into the year as it is.
fn stamp_parts(date: &str) -> (String, String, String, String) {
    let t = date.trim();
    let (prefix, rest) = match t.chars().next() {
        Some(c) if !c.is_ascii_digit() => (c.to_string(), &t[c.len_utf8()..]),
        _ => (String::new(), t),
    };
    let parts: Vec<&str> = rest.split(['.', '/', '-']).map(|x| x.trim()).collect();
    if parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) {
        return (prefix, parts[0].into(), parts[1].into(), parts[2].into());
    }
    (String::new(), t.into(), String::new(), String::new())
}

/// Attribute 7 holding a drawing: the body fields DocuWorks writes for
/// annotation drawings (compressed, no 0x8a flag).
pub fn drawing_attr(emf: &[u8], w: f64, h: f64) -> Attr {
    let stored = lzh::compress(emf);
    let mut v = element(0x80, &[4]);
    v.extend(element(0x81, &tlv::uint_bytes(emf.len() as u64)));
    v.extend(element(0x84, &tlv::uint_bytes(w.round() as u64)));
    v.extend(element(0x85, &tlv::uint_bytes(h.round() as u64)));
    v.extend(element(0x8d, &[1]));
    v.extend(element(0x90, &tlv::uint_bytes(emfw::units(w) as u64)));
    v.extend(element(0x91, &tlv::uint_bytes(emfw::units(h) as u64)));
    v.extend(element(0x89, &tlv::uint_bytes(stored.len() as u64)));
    v.extend(element(0x86, &stored));
    Attr { class: 0x80, tag: 7, value: v }
}

pub(crate) fn int_attr(tag: u32, vs: &[i64]) -> Attr {
    Attr { class: 0x80, tag, value: props::ints_value(vs) }
}

pub(crate) fn defs_attr(defs: &[(u32, i64, &str)]) -> Attr {
    let mut v = Vec::new();
    for &(tag, ty, name) in defs {
        let mut nm = name.as_bytes().to_vec();
        nm.push(0);
        v.extend(props::ints_value(&[tag as i64, ty, -1, nm.len() as i64]));
        v.push(nm.len() as u8);
        v.extend(nm);
    }
    Attr { class: 0x80, tag: props::A_DEFS, value: v }
}

pub(crate) fn named(tag: u32, value: Vec<u8>) -> Attr {
    Attr { class: 0x80, tag, value }
}

pub(crate) fn utf16z(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    v.extend([0, 0]);
    v
}

pub(crate) fn sjisz(s: &str) -> Vec<u8> {
    let mut v = crate::sjis::encode(s);
    v.push(0);
    v
}

/// The records for a shape: the object, and for a sticky note the text on it
/// (a placement of the note's kind, then the text annotation).
fn object_records(depth: u8, id: i64, shape: &Shape, w: f64, h: f64) -> Vec<Record> {
    let Shape::Sticky { text, size, color, background } = shape else {
        return vec![object_record(depth, id, shape, w, h)];
    };
    let inner = Shape::Text { text: text.clone(), size: *size, color: *color, bold: false, background: None, frame: None };
    let (tw, th) = text_box(text, *size);
    let note = Record {
        depth,
        kind: props::num_bytes(doc::K_FUSEN),
        attrs: vec![
            int_attr(5, &[w.round() as i64, h.round() as i64]),
            defs_attr(&[(2001, 2, "FSN_COLOR"), (2002, 2, "%AutoResize"), (2004, 2, "lastmid")]),
            int_attr(2001, &[emfw::colorref(*background) as i64]),
            int_attr(2002, &[0]),
            int_attr(3, &[id]),
            int_attr(2004, &[1]),
            drawing_attr(&draw(shape, w, h), w, h),
        ],
    };
    // inside a note the text's placement has the note's kind
    let mut place = place_record(depth + 1, STICKY_PAD, STICKY_PAD, tw, th);
    place.kind = props::num_bytes(doc::K_FUSEN);
    vec![note, place, object_record(depth + 2, 1, &inner, tw, th)]
}

/// The object record (kind and settings) for a shape.
fn object_record(depth: u8, id: i64, shape: &Shape, w: f64, h: f64) -> Record {
    let size = [w.round() as i64, h.round() as i64];
    let mut attrs: Vec<Attr> = Vec::new();
    let kind: i64;
    match shape {
        Shape::Text { text, size: pt, color, bold, background, frame } => {
            kind = K_TEXT;
            attrs.push(defs_attr(&[
                (2002, 4, "%Face"),
                (2003, 2, "%PitchAndFamily"),
                (2004, 2, "%CharSet"),
                (2006, 2, "%hidden"),
                (2007, 2, "%Style"),
                (2008, 2, "%Size"),
                (2009, 2, "%Color"),
                (2010, 2, "%ATTR_BKGND_COLOR"),
                (2011, 2, "%Turnover"),
                (2012, 2, "%TextBetweenLine"),
                (2013, 2, "%TextVerticalWriting"),
                (2014, 2, "%TextRotationAngle"),
                (2015, 107, "%NoRotateTextDim"),
                (2016, 2, "%AutoResizeHeight"),
                (2017, 2, "%Spacing"),
                (2018, 2, "%LeftMargin"),
                (2019, 2, "%TopMargin"),
                (2020, 2, "%RightMargin"),
                (2021, 2, "%BottomMargin"),
                (2022, 2, "%Align"),
                (2023, 2, "%BkGndPermeable"),
                (2024, 2, "%FrameOnOff"),
                (2025, 4, "%Face(w"),
                (2026, 2, "%CCP_%Face"),
                (2027, 2, "%CCP_#name"),
                (2028, 4, "%Text(w"),
                (2029, 4, "%Text"),
                (2030, 2, "%FrameColor"),
            ]));
            attrs.push(named(2002, sjisz(FACE)));
            attrs.push(int_attr(2003, &[17]));
            attrs.push(int_attr(2004, &[128]));
            attrs.push(int_attr(2006, &[0]));
            attrs.push(int_attr(2007, &[if *bold { 1 } else { 0 }]));
            attrs.push(int_attr(2008, &[(pt * 10.0).round() as i64]));
            attrs.push(int_attr(2009, &[emfw::colorref(*color) as i64]));
            attrs.push(int_attr(2010, &[background.map(|c| emfw::colorref(c) as i64).unwrap_or(65793)]));
            attrs.push(int_attr(2011, &[0]));
            attrs.push(int_attr(2012, &[100]));
            attrs.push(int_attr(2013, &[0]));
            attrs.push(int_attr(2014, &[0]));
            attrs.push(int_attr(2015, &size));
            attrs.push(int_attr(2016, &[0]));
            attrs.push(int_attr(2017, &[0]));
            for t in 2018..=2021 {
                attrs.push(int_attr(t, &[MARGIN as i64]));
            }
            attrs.push(int_attr(2022, &[0]));
            attrs.push(int_attr(2023, &[0]));
            attrs.push(int_attr(2024, &[if frame.is_some() { 1 } else { 0 }]));
            attrs.push(named(2025, utf16z(FACE_W)));
            attrs.push(int_attr(2026, &[932]));
            attrs.push(int_attr(2027, &[932]));
            attrs.push(named(2028, utf16z(text)));
            attrs.push(named(2029, sjisz(text)));
            attrs.push(int_attr(2030, &[emfw::colorref(frame.unwrap_or(0)) as i64]));
            attrs.push(int_attr(68, &[0]));
        }
        Shape::Rect { stroke, width, fill, highlight } => {
            kind = K_RECT;
            attrs.push(defs_attr(&[
                (2001, 2, "RECTATT_LINE"),
                (2002, 2, "RECTATT_THICK"),
                (2003, 2, "RECTATT_LINECOLOR"),
                (2004, 2, "RECTATT_DRAW"),
                (2006, 2, "RECTATT_RECTCOLOR"),
                (2007, 2, "ATTR_FRAMETRANSPARENT"),
                (2008, 2, "ATTR_FILLTRANSPARENT"),
            ]));
            attrs.push(int_attr(2001, &[0]));
            attrs.push(int_attr(2002, &[width.round().max(1.0) as i64]));
            attrs.push(int_attr(2003, &[emfw::colorref(stroke.unwrap_or(0)) as i64]));
            attrs.push(int_attr(2004, &[draw_mode(stroke.is_some(), fill.is_some())]));
            attrs.push(int_attr(2006, &[emfw::colorref(fill.unwrap_or(0xffffff)) as i64]));
            attrs.push(int_attr(2007, &[0]));
            attrs.push(int_attr(2008, &[if *highlight { 1 } else { 0 }]));
        }
        Shape::Ellipse { stroke, width, fill } => {
            kind = K_ELLIPSE;
            attrs.push(defs_attr(&[
                (2001, 2, "ARCATT_LINE"),
                (2002, 2, "ARCATT_THICK"),
                (2003, 2, "ARCATT_LINECOLOR"),
                (2004, 2, "ARCATT_DRAW"),
                (2006, 2, "ARCATT_RECTCOLOR"),
                (2007, 2, "ATTR_FRAMETRANSPARENT"),
                (2008, 2, "ATTR_FILLTRANSPARENT"),
            ]));
            attrs.push(int_attr(2001, &[0]));
            attrs.push(int_attr(2002, &[width.round().max(1.0) as i64]));
            attrs.push(int_attr(2003, &[emfw::colorref(stroke.unwrap_or(0)) as i64]));
            attrs.push(int_attr(2004, &[draw_mode(stroke.is_some(), fill.is_some())]));
            attrs.push(int_attr(2006, &[emfw::colorref(fill.unwrap_or(0xffffff)) as i64]));
            attrs.push(int_attr(2007, &[0]));
            attrs.push(int_attr(2008, &[0]));
        }
        Shape::Line { points, color, width } => {
            kind = K_LINE;
            attrs.push(defs_attr(&[
                (2003, 2, "LINE_COLOR"),
                (2004, 2, "ATTR_TRANSPARENT"),
                (2005, 2, "LINE_WIDTH"),
                (2006, 2, "LINE_STYLE"),
                (2007, 2, "ARROW_SORT"),
                (2010, 4, "LINE_DATA"),
            ]));
            attrs.push(int_attr(2003, &[emfw::colorref(*color) as i64]));
            attrs.push(int_attr(2004, &[0]));
            attrs.push(int_attr(2005, &[width.round().max(1.0) as i64]));
            attrs.push(int_attr(2006, &[0]));
            attrs.push(int_attr(2007, &[0]));
            let mut d = Vec::new();
            for &(x, y) in points {
                d.extend_from_slice(&(x.round() as i32).to_le_bytes());
                d.extend_from_slice(&(y.round() as i32).to_le_bytes());
            }
            attrs.push(named(2010, d));
        }
        Shape::Stamp { top, date, bottom, color } => {
            // what DocuWorks 10 itself writes for a date stamp (made with
            // its API and read back): POST is the upper text, NAME the lower
            kind = doc::K_STAMP;
            let (prefix, y, m, dd) = stamp_parts(date);
            attrs.push(defs_attr(&[
                (2001, 2, "STAMPATT_COLOR"),
                (2002, 2, "STAMPATT_TRANSPARENT"),
                (2003, 4, "STAMPATT_POST"),
                (2004, 4, "STAMPATT_NAME"),
                (2005, 2, "STAMPATT_DATEFLAG"),
                (2009, 2, "STAMPATT_ERA"),
                (2010, 2, "STAMPATT_BASEYEAR"),
                (2011, 4, "STAMPATT_PREFIX"),
                (2012, 4, "STAMPATT_DATEFORMAT"),
                (2013, 2, "STAMPATT_DATEORDER"),
                (2015, 2, "%CCP_STAMPATT_POST"),
                (2016, 4, "STAMPATT_POST(w"),
                (2017, 2, "%CCP_STAMPATT_NAME"),
                (2018, 4, "STAMPATT_NAME(w"),
                (2019, 4, "STAMPATT_YEAR"),
                (2020, 4, "STAMPATT_MONTH"),
                (2021, 4, "STAMPATT_DAY"),
            ]));
            attrs.push(int_attr(2001, &[emfw::colorref(*color) as i64]));
            attrs.push(int_attr(2002, &[0]));
            attrs.push(int_attr(2009, &[0]));
            attrs.push(int_attr(2010, &[1]));
            attrs.push(named(2011, sjisz(&prefix)));
            attrs.push(named(2012, sjisz("yy.mm.dd")));
            attrs.push(int_attr(2013, &[0])); // year, month, day
            attrs.push(named(2003, sjisz(top)));
            attrs.push(int_attr(2015, &[932]));
            attrs.push(named(2016, utf16z(top)));
            attrs.push(named(2004, sjisz(bottom)));
            attrs.push(int_attr(2017, &[932]));
            attrs.push(named(2018, utf16z(bottom)));
            attrs.push(int_attr(2005, &[1])); // the date as given, not today's
            attrs.push(named(2019, sjisz(&y)));
            attrs.push(named(2020, sjisz(&m)));
            attrs.push(named(2021, sjisz(&dd)));
            attrs.push(int_attr(68, &[0]));
        }
        Shape::Sticky { .. } => unreachable!("sticky notes are made by object_records"),
    }
    attrs.push(int_attr(3, &[id]));
    attrs.push(int_attr(5, &size));
    attrs.push(drawing_attr(&draw(shape, w, h), w, h));
    Record { depth, kind: props::num_bytes(kind), attrs }
}

/// The box an annotation gets: text follows its text when w or h is 0,
/// a date stamp gets its default size.
fn box_size(shape: &Shape, w: f64, h: f64) -> (f64, f64) {
    match shape {
        Shape::Text { text, size, .. } if w <= 0.0 || h <= 0.0 => text_box(text, *size),
        Shape::Stamp { .. } if w <= 0.0 || h <= 0.0 => (STAMP_SIZE, STAMP_SIZE),
        Shape::Sticky { text, size, .. } if w <= 0.0 || h <= 0.0 => sticky_box(text, *size),
        _ => (w.max(100.0), h.max(100.0)),
    }
}

/// RECTATT_DRAW / ARCATT_DRAW: 0 frame, 1 fill, 2 both (DocuWorks files
/// with a filled, frameless box use 1).
fn draw_mode(frame: bool, fill: bool) -> i64 {
    match (frame, fill) {
        (true, true) => 2,
        (false, true) => 1,
        _ => 0,
    }
}

/// A placement; sticky notes' placements also carry 54 = 1 (as DocuWorks
/// writes them).
fn placement_for(shape: &Shape, depth: u8, x: f64, y: f64, w: f64, h: f64) -> Record {
    let mut p = place_record(depth, x, y, w, h);
    if matches!(shape, Shape::Sticky { .. }) {
        p.attrs.insert(0, int_attr(54, &[1]));
    }
    p
}

pub(crate) fn place_record(depth: u8, x: f64, y: f64, w: f64, h: f64) -> Record {
    Record {
        depth,
        kind: props::num_bytes(K_PLACE),
        attrs: vec![
            int_attr(52, &[x.round() as i64, y.round() as i64]),
            defs_attr(&[(2001, 107, "childdim")]),
            int_attr(2001, &[w.round() as i64, h.round() as i64]),
        ],
    }
}

/// Where a record's subtree ends (exclusive).
pub(crate) fn subtree_end(r: &[Record], i: usize) -> usize {
    let d = r[i].depth;
    let mut j = i + 1;
    while j < r.len() && r[j].depth > d {
        j += 1;
    }
    j
}

impl Document {
    pub(crate) fn page(&self, n: usize) -> Result<&doc::Page> {
        self.pages.get(n).ok_or_else(|| Error::Unsupported(format!("no page {}", n + 1)))
    }

    /// The next free object number on a page (and remember it in lastmid).
    fn next_id(&mut self, page: usize) -> Result<i64> {
        let pr = self.page(page)?.record;
        Ok(self.bump_lastmid(pr, 2))
    }

    /// The next free number among the records `below` levels under
    /// `pr` (attribute 3), remembered in `pr`'s "lastmid".
    pub(crate) fn bump_lastmid(&mut self, pr: usize, below: u8) -> i64 {
        let tag = self.records[pr].named_tag("lastmid");
        let mut max = tag.and_then(|t| self.records[pr].int(t)).unwrap_or(0);
        let end = subtree_end(&self.records, pr);
        for r in &self.records[pr + 1..end] {
            if r.depth == self.records[pr].depth + below {
                max = max.max(r.int(3).unwrap_or(0));
            }
        }
        let id = max + 1;
        match tag {
            Some(t) => {
                if let Some(a) = self.records[pr].get_mut(t) {
                    a.value = props::ints_value(&[id]);
                }
            }
            None => {
                let rec = &mut self.records[pr];
                let mut defs = rec.defs().into_iter().map(|d| (d.tag, d.ty, d.name)).collect::<Vec<_>>();
                let t = defs.iter().map(|d| d.0).max().unwrap_or(2000) + 1;
                defs.push((t, 2, "lastmid".into()));
                let refs: Vec<(u32, i64, &str)> = defs.iter().map(|(a, b, c)| (*a, *b, c.as_str())).collect();
                let da = defs_attr(&refs);
                rec.set(da.class, da.tag, da.value);
                rec.set(0x80, t, props::ints_value(&[id]));
            }
        }
        id
    }

    /// Add an annotation at (x, y), size w × h (1/100 mm). A text
    /// annotation's size follows its text when w or h is 0.
    pub fn add_annotation(&mut self, page: usize, x: f64, y: f64, w: f64, h: f64, shape: &Shape) -> Result<usize> {
        let (w, h) = box_size(shape, w, h);
        let id = self.next_id(page)?;
        let pr = self.page(page)?.record;
        let d = self.records[pr].depth;
        let at = subtree_end(&self.records, pr);
        let mut recs = vec![placement_for(shape, d + 1, x, y, w, h)];
        recs.extend(object_records(d + 2, id, shape, w, h));
        for (k, r) in recs.into_iter().enumerate() {
            self.records.insert(at + k, r);
        }
        self.refresh();
        Ok(self.page(page)?.objects.len() - 1)
    }

    fn object(&self, page: usize, obj: usize) -> Result<doc::Object> {
        self.page(page)?
            .objects
            .get(obj)
            .cloned()
            .ok_or_else(|| Error::Unsupported(format!("no object {obj} on page {}", page + 1)))
    }

    pub fn delete_object(&mut self, page: usize, obj: usize) -> Result<()> {
        let o = self.object(page, obj)?;
        if o.kind == K_CONTENT {
            return Err(Error::Unsupported("the page content itself cannot be deleted; delete the page".into()));
        }
        if o.kind == doc::K_SIGNATURE {
            return Err(Error::Unsupported("a signature cannot be removed here".into()));
        }
        let end = subtree_end(&self.records, o.place);
        self.records.drain(o.place..end);
        self.refresh();
        Ok(())
    }

    pub fn move_object(&mut self, page: usize, obj: usize, x: f64, y: f64) -> Result<()> {
        let o = self.object(page, obj)?;
        if o.kind == K_CONTENT || o.kind == doc::K_SIGNATURE {
            return Err(Error::Unsupported("the page content and signatures cannot be moved".into()));
        }
        self.records[o.place].set(0x80, 52, props::ints_value(&[x.round() as i64, y.round() as i64]));
        self.refresh();
        Ok(())
    }

    /// Replace an annotation's look (and size): used for resizing and for
    /// changing text, colours, …
    pub fn change_annotation(&mut self, page: usize, obj: usize, x: f64, y: f64, w: f64, h: f64, shape: &Shape) -> Result<()> {
        let o = self.object(page, obj)?;
        if o.kind == K_CONTENT {
            return Err(Error::Unsupported("the page content cannot be changed".into()));
        }
        let (w, h) = box_size(shape, w, h);
        let id = self.records[o.record].int(3).unwrap_or(1);
        let d = self.records[o.record].depth;
        let end = subtree_end(&self.records, o.place);
        let mut recs = vec![placement_for(shape, d - 1, x, y, w, h)];
        recs.extend(object_records(d, id, shape, w, h));
        self.records.splice(o.place..end, recs);
        self.refresh();
        Ok(())
    }

    /// Add a picture annotation: `rgba` is `pw` × `ph` pixels, top row
    /// first, placed at (x, y) with size w × h (1/100 mm). Transparent
    /// pixels become white. With `see_through` the white parts show the page
    /// (an embedded OLE picture, see `doc::PICTURE_MARK`); without, it is
    /// DocuWorks' bitmap annotation (803f), which DocuWorks shows opaque.
    #[allow(clippy::too_many_arguments)]
    pub fn add_picture(&mut self, page: usize, x: f64, y: f64, w: f64, h: f64, rgba: &[u8], pw: u32, ph: u32, see_through: bool) -> Result<usize> {
        if pw == 0 || ph == 0 || rgba.len() < (pw * ph * 4) as usize {
            return Err(Error::Unsupported("picture size does not match its pixels".into()));
        }
        let id = self.next_id(page)?;
        let pr = self.page(page)?.record;
        let d = self.records[pr].depth;
        let at = subtree_end(&self.records, pr);
        let obj = self.picture_record(d + 2, id, w, h, 0, &dib_24(rgba, pw, ph), see_through);
        self.records.insert(at, place_record(d + 1, x, y, w, h));
        self.records.insert(at + 1, obj);
        self.refresh();
        Ok(self.page(page)?.objects.len() - 1)
    }

    /// Redo a picture annotation at (x, y), size w × h, see-through or
    /// not: for resizing, and for switching between the two kinds. Its
    /// pixels are read back from the file.
    #[allow(clippy::too_many_arguments)]
    pub fn set_picture(&mut self, page: usize, obj: usize, x: f64, y: f64, w: f64, h: f64, see_through: bool) -> Result<()> {
        let o = self.object(page, obj)?;
        let img = self.picture_image(&o).ok_or_else(|| Error::Unsupported("this picture cannot be read back".into()))?;
        let crate::gfx::ImageData::Rgba(px) = &img.data else {
            return Err(Error::Unsupported("a JPEG picture cannot be changed here".into()));
        };
        let raw = dib_24(px, img.w, img.h);
        let r = &self.records[o.record];
        let (id, d, rot) = (r.int(3).unwrap_or(1), r.depth, r.int(61).unwrap_or(0).rem_euclid(360));
        let end = subtree_end(&self.records, o.place);
        let rec = self.picture_record(d, id, w.max(1.0), h.max(1.0), rot, &raw, see_through);
        self.records.splice(o.place..end, [place_record(d - 1, x, y, w.max(1.0), h.max(1.0)), rec]);
        self.refresh();
        Ok(())
    }

    /// Is `obj` a picture, and is it see-through? `None`: not a picture.
    pub fn picture_see_through(&self, page: usize, obj: usize) -> Option<bool> {
        let o = self.object(page, obj).ok()?;
        (o.kind_name == "picture").then_some(o.kind == doc::K_OLE)
    }

    /// The object record of a picture from a 24-bit DIB (`raw` =
    /// BITMAPINFOHEADER + bits). `w` × `h` is the box on the page; `rot`
    /// (0, 90, 180, 270) is the turn page rotation gave it: the drawing is
    /// made for the box before the turn, like rotate_page leaves it.
    #[allow(clippy::too_many_arguments)]
    fn picture_record(&mut self, depth: u8, id: i64, w: f64, h: f64, rot: i64, raw: &[u8], see_through: bool) -> Record {
        let (bw, bh) = if rot == 90 || rot == 270 { (h, w) } else { (w, h) };
        let pw = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
        let ph = u32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]);
        let size = [w.round() as i64, h.round() as i64];
        if see_through {
            // one STRETCHDIBITS with SRCAND: the picture multiplies the page.
            // Kept as an embedded OLE picture (Enhanced Metafile), which
            // DocuWorks moves and sizes by its stored drawing.
            let (uw, uh) = (emfw::units(bw), emfw::units(bh));
            let mut e = Emf::new(uw, uh);
            e.stretch_mode(4);
            e.stretch_dib([0, 0, uw, uh], pw as i32, ph as i32, &raw[..40], &raw[40..], 0x0088_00C6);
            let emf = e.finish(bw, bh);
            let (_, draw_ref) = self.add_entry(drawing_attr(&emf, bw, bh).value, false);
            let file = ole_picture_file(&emf);
            let stored = lzh::compress(&file);
            let stored_len = stored.len();
            let (n, _) = self.add_entry(stored, true);
            let mut attrs = vec![
                int_attr(57, &[1]),
                int_attr(55, &[1]),
                int_attr(56, &[1]),
                defs_attr(&[
                    (2001, 4, "%OLE_CONTENT_FILE"),
                    (2002, 107, "%OLE_SIZES"),
                    (2003, 4, "%OLE_SERVER_NAME"),
                    (2004, 4, "%OLE_CLSID"),
                    (2005, 2, "%OLE_DWASPECT"),
                    (2006, 2, "%OLE_ITEMTYPE"),
                    (2007, 2, doc::PICTURE_MARK),
                ]),
                int_attr(2002, &[bw.round() as i64, bh.round() as i64]),
                named(2003, sjisz(OLE_EMF_NAME)),
                named(2004, CLSID_PICTURE_EMF.to_vec()),
                int_attr(2005, &[1]),
                int_attr(2006, &[3]),
                int_attr(2007, &[1]),
            ];
            if rot != 0 {
                attrs.push(int_attr(61, &[rot]));
            }
            attrs.push(int_attr(3, &[id]));
            attrs.push(int_attr(5, &size));
            attrs.push(Attr { class: 0xc0, tag: 7, value: draw_ref });
            attrs.push(Attr { class: 0xc0, tag: 2001, value: write::packed_ref(n, file.len(), stored_len) });
            return Record { depth, kind: props::num_bytes(doc::K_OLE), attrs };
        }
        let dib = crate::dib::encode_stored(&raw[..40], &raw[40..], ph);
        let mut body = element(0x80, &[7]);
        body.extend(element(0x81, &[40]));
        body.extend(element(0x84, &tlv::uint_bytes(bw.round() as u64)));
        body.extend(element(0x85, &tlv::uint_bytes(bh.round() as u64)));
        body.extend(element(0x89, &tlv::uint_bytes(dib.len() as u64)));
        body.extend(element(0x86, &dib));
        let (_, reference) = self.add_entry(body, false);
        Record {
            depth,
            kind: props::num_bytes(doc::K_PICTURE),
            attrs: vec![
                int_attr(57, &[1]),
                int_attr(5, &size),
                int_attr(61, &[rot]),
                Attr { class: 0xc0, tag: 7, value: reference },
                int_attr(3, &[id]),
            ],
        }
    }

    /// Turn a page clockwise by `quarters` × 90°.
    pub fn rotate_page(&mut self, page: usize, quarters: i32) -> Result<()> {
        let q = quarters.rem_euclid(4);
        for _ in 0..q {
            self.rotate_once(page)?;
        }
        self.refresh();
        Ok(())
    }

    fn rotate_once(&mut self, page: usize) -> Result<()> {
        let p = self.page(page)?.clone();
        let (pw, ph) = (p.w, p.h);
        self.records[p.record].set(0x80, 5, props::ints_value(&[ph, pw]));
        for o in &p.objects {
            // the box turns: top-left goes to the right
            let nx = ph - (o.y + o.h);
            let ny = o.x;
            self.records[o.place].set(0x80, 52, props::ints_value(&[nx, ny]));
            let dim_tag = self.named(o.place, "childdim").map(|a| a.tag).unwrap_or(2001);
            self.records[o.place].set(0x80, dim_tag, props::ints_value(&[o.h, o.w]));
            let rec = &mut self.records[o.record];
            let size = rec.ints(5);
            if size.len() == 2 {
                rec.set(0x80, 5, props::ints_value(&[size[1], size[0]]));
            }
            let rot = (rec.int(61).unwrap_or(0) + 90).rem_euclid(360);
            rec.set(0x80, 61, props::ints_value(&[rot]));
        }
        self.pages = self.find_pages();
        Ok(())
    }

    pub fn delete_page(&mut self, page: usize) -> Result<()> {
        if self.pages.len() <= 1 {
            return Err(Error::Unsupported("a document needs at least one page".into()));
        }
        let pr = self.page(page)?.record;
        if self.pages_in_list(pr) <= 1 {
            return Err(Error::Unsupported("the last page of a binder document (remove the document instead)".into()));
        }
        let end = subtree_end(&self.records, pr);
        self.records.drain(pr..end);
        self.refresh();
        Ok(())
    }

    /// Move page `from` so that it becomes page `to` (0-based).
    pub fn move_page(&mut self, from: usize, to: usize) -> Result<()> {
        if from == to {
            return Ok(());
        }
        let a = self.page(from)?.record;
        let _ = self.page(to)?;
        if self.records[a].depth != self.records[self.page(to)?.record].depth {
            return Err(Error::Unsupported("pages of different documents in a binder".into()));
        }
        let from_list = self.list_of(a);
        let to_list = self.list_of(self.page(to)?.record);
        if from_list != to_list && self.pages_in_list(a) <= 1 {
            return Err(Error::Unsupported("the last page of a binder document cannot leave it".into()));
        }
        let end = subtree_end(&self.records, a);
        let block: Vec<Record> = self.records.drain(a..end).collect();
        self.pages = self.find_pages();
        // after the target when moving down, before it when moving up
        let at = if to >= self.pages.len() {
            let last = self.pages.last().map(|p| p.record).unwrap_or(0);
            subtree_end(&self.records, last)
        } else if to > from {
            // pages shifted by one: the target is now at to - 1
            subtree_end(&self.records, self.pages[to - 1].record)
        } else {
            self.pages[to].record
        };
        for (k, r) in block.into_iter().enumerate() {
            self.records.insert(at + k, r);
        }
        if from_list != to_list {
            // a page joining another document of a binder gets a number there
            if let Some(l) = self.list_of(at) {
                let id = self.bump_lastmid(l, 1);
                self.records[at].set(0x80, 3, props::ints_value(&[id]));
            }
        }
        self.refresh();
        Ok(())
    }

    /// The page list (1303) a page record belongs to.
    pub(crate) fn list_of(&self, pr: usize) -> Option<usize> {
        let d = self.records.get(pr)?.depth;
        (0..pr).rev().find(|&j| self.records[j].depth < d).filter(|&j| self.records[j].kind_num() == doc::K_DOCUMENT)
    }

    /// How many pages the page list of page record `pr` holds.
    fn pages_in_list(&self, pr: usize) -> usize {
        let l = self.list_of(pr);
        self.pages.iter().filter(|p| self.list_of(p.record) == l).count()
    }

    /// The records and new entries a save writes: entries added since
    /// opening that nothing refers to any more are left out, and the rest
    /// are numbered after the file's own.
    pub fn to_write(&self) -> (Vec<Record>, Vec<write::NewEntry>) {
        let base = self.entries.len() as u32;
        let mut used = vec![false; self.pending.len()];
        for r in &self.records {
            for a in &r.attrs {
                if a.class & 0xc0 == 0xc0 {
                    if let Some((i, _)) = write::read_ref(&a.value) {
                        if i >= base && ((i - base) as usize) < used.len() {
                            used[(i - base) as usize] = true;
                        }
                    }
                }
            }
        }
        let mut map = std::collections::HashMap::new();
        let mut kept = Vec::new();
        for (k, e) in self.pending.iter().enumerate() {
            if used[k] {
                map.insert(base + k as u32, base + kept.len() as u32);
                kept.push(e.clone());
            }
        }
        let mut records = self.records.clone();
        for r in &mut records {
            for a in &mut r.attrs {
                if a.class & 0xc0 == 0xc0 {
                    if let Some((i, _)) = write::read_ref(&a.value) {
                        if let Some(&n) = map.get(&i) {
                            a.value = write::renumber_ref(&a.value, n);
                        }
                    }
                }
            }
        }
        (records, kept)
    }

    /// The document as a `.xdw` file (original bytes + one new segment),
    /// the way DocuWorks saves: everything the file held stays in it. The
    /// result is read back and checked before it is returned. `save` writes
    /// the file anew instead (see `fresh`).
    pub fn save_append(&self) -> Result<Vec<u8>> {
        let (records, entries) = self.to_write();
        let (bytes, _) = write::append(&self.bytes, &self.container, &entries, &records)?;
        let back = Document::open(bytes.clone())?;
        if back.records != records {
            return Err(Error::Corrupt("the saved file does not read back the same".into()));
        }
        if back.pages.len() != self.pages.len() {
            return Err(Error::Corrupt("page count changed while saving".into()));
        }
        for (k, e) in entries.iter().enumerate() {
            let n = self.entries.len() + k;
            if back.entry_body(n) != Some(&e.body[..]) {
                return Err(Error::Corrupt("a new entry does not read back the same".into()));
            }
        }
        Ok(bytes)
    }

    /// The settings of an annotation, as a shape this editor can redraw.
    pub fn shape_of(&self, page: usize, obj: usize) -> Option<Shape> {
        let o = self.object(page, obj).ok()?;
        let r = &self.records[o.record];
        let ni = |name: &str| self.named(o.record, name).and_then(|a| props::ints(&a.value).first().copied());
        let rgb = |c: i64| emfw::colorref(c as u32); // COLORREF ↔ RGB is the same swap
        match o.kind {
            K_TEXT => {
                let text = o.text.clone().unwrap_or_default();
                let size = ni("%Size").unwrap_or(120) as f64 / 10.0;
                let color = rgb(ni("%Color").unwrap_or(0));
                let bold = ni("%Style").unwrap_or(0) & 1 != 0;
                let bg = ni("%ATTR_BKGND_COLOR").filter(|&c| c != 65793 && ni("%BkGndPermeable").unwrap_or(0) == 0).map(rgb);
                let frame = (ni("%FrameOnOff").unwrap_or(0) != 0).then(|| rgb(ni("%FrameColor").unwrap_or(0)));
                Some(Shape::Text { text, size, color, bold, background: bg, frame })
            }
            K_RECT | K_ELLIPSE => {
                let p = if o.kind == K_RECT { "RECTATT_" } else { "ARCATT_" };
                let mode = ni(&format!("{p}DRAW")).unwrap_or(0);
                let stroke = (mode != 1).then(|| rgb(ni(&format!("{p}LINECOLOR")).unwrap_or(0)));
                let fill = (mode >= 1).then(|| rgb(ni(&format!("{p}RECTCOLOR")).unwrap_or(0xffffff)));
                let width = ni(&format!("{p}THICK")).unwrap_or(1) as f64;
                if o.kind == K_RECT {
                    let highlight = ni("ATTR_FILLTRANSPARENT").unwrap_or(0) == 1;
                    Some(Shape::Rect { stroke, width, fill, highlight })
                } else {
                    Some(Shape::Ellipse { stroke, width, fill })
                }
            }
            K_LINE => {
                let data = self.named(o.record, "LINE_DATA")?.value.clone();
                let points = data
                    .chunks_exact(8)
                    .map(|c| (i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64, i32::from_le_bytes([c[4], c[5], c[6], c[7]]) as f64))
                    .collect();
                let color = rgb(ni("LINE_COLOR").unwrap_or(0));
                let width = ni("LINE_WIDTH").unwrap_or(1) as f64;
                let _ = r;
                Some(Shape::Line { points, color, width })
            }
            doc::K_STAMP => {
                let text = |name: &str| -> String {
                    match self.named(o.record, &format!("{name}(w")) {
                        Some(a) => utf16_str(&a.value),
                        None => self.named(o.record, name).map(|a| crate::sjis::decode(a.value.split(|&b| b == 0).next().unwrap_or(&[]))).unwrap_or_default(),
                    }
                };
                // a stamp that shows today's date (DATEFLAG 0) is redrawn
                // with the date it was stamped: its year, month and day
                let (y, m, d) = (text("STAMPATT_YEAR"), text("STAMPATT_MONTH"), text("STAMPATT_DAY"));
                let date = if m.is_empty() && d.is_empty() { y } else { format!("{}{y}.{m}.{d}", text("STAMPATT_PREFIX")) };
                if date.is_empty() {
                    return None; // a stamp with today's date and no stored date: move / delete only
                }
                let color = rgb(ni("STAMPATT_COLOR").unwrap_or(0x1200e6));
                Some(Shape::Stamp { top: text("STAMPATT_POST"), date, bottom: text("STAMPATT_NAME"), color })
            }
            doc::K_FUSEN => {
                // the text annotation on the note (its first one)
                let end = subtree_end(&self.records, o.record);
                let t = (o.record + 1..end).find(|&k| self.records[k].kind_num() == K_TEXT)?;
                let ti = |name: &str| self.named(t, name).and_then(|a| props::ints(&a.value).first().copied());
                let text = self.named(t, "%Text(w").map(|a| utf16_str(&a.value)).or_else(|| self.named(t, "%Text").map(|a| crate::sjis::decode(a.value.split(|&b| b == 0).next().unwrap_or(&[]))))?;
                Some(Shape::Sticky {
                    text,
                    size: ti("%Size").unwrap_or(120) as f64 / 10.0,
                    color: rgb(ti("%Color").unwrap_or(0)),
                    background: rgb(ni("FSN_COLOR").unwrap_or(0x64ffff)),
                })
            }
            _ => None,
        }
    }

    /// Does the document carry a signature (editing makes it invalid)?
    pub fn is_signed(&self) -> bool {
        self.records.iter().any(|r| r.kind_num() == doc::K_SIGNATURE)
    }

    /// What is readable about a signature (8043) from the stored properties,
    /// without the DocuWorks API: the signing module and the version. The
    /// module string tells a DocuWorks electronic seal (電子印鑑) from a PKI
    /// certificate signature. Whether the signature still holds needs the
    /// DocuWorks API or verifying the certificate, which this does not do.
    pub fn signature_of(&self, page: usize, obj: usize) -> Option<SignatureInfo> {
        let o = self.object(page, obj).ok()?;
        if o.kind != doc::K_SIGNATURE {
            return None;
        }
        // %smin: the module name (Shift_JIS), e.g. "DocuWorks電子印鑑 (SHA1 …"
        let module = self
            .named(o.record, "%smin")
            .map(|a| crate::sjis::decode(a.value.split(|&b| b == 0).next().unwrap_or(&a.value)))
            .unwrap_or_default();
        let version = self
            .named(o.record, "%sigver")
            .map(|a| crate::sjis::decode(a.value.split(|&b| b == 0).next().unwrap_or(&a.value)))
            .filter(|s| !s.is_empty());
        let stamp = module.contains("電子印鑑") || module.contains("Stamp");
        Some(SignatureInfo { module, version, stamp })
    }

    /// Is `obj` one of the annotation kinds this editor can redraw
    /// (pictures: resize and switch see-through)?
    pub fn editable(&self, page: usize, obj: usize) -> bool {
        self.shape_of(page, obj).is_some() || self.picture_see_through(page, obj).is_some()
    }
}

/// CLSID_Picture_EnhMetafile {00000319-0000-0000-C000-000000000046}.
const CLSID_PICTURE_EMF: [u8; 16] = [0x19, 0x03, 0, 0, 0, 0, 0, 0, 0xc0, 0, 0, 0, 0, 0, 0, 0x46];
const OLE_EMF_NAME: &str = "Picture (Enhanced Metafile)";

/// The file DocuWorks keeps for an embedded OLE picture: a short header
/// (the file's length last), then an OLE compound file holding `\1Ole`,
/// `\1CompObj` and `CONTENTS` (as OLE stores a static metafile: the size
/// of its header, the header, then the whole metafile).
fn ole_picture_file(emf: &[u8]) -> Vec<u8> {
    let mut comp = vec![1, 0, 0xfe, 0xff, 3, 0x0a, 0, 0, 0xff, 0xff, 0xff, 0xff];
    comp.extend_from_slice(&CLSID_PICTURE_EMF);
    let name = format!("{OLE_EMF_NAME}\0");
    comp.extend_from_slice(&(name.len() as u32).to_le_bytes());
    comp.extend_from_slice(name.as_bytes());
    comp.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0x0e, 0, 0, 0]); // CF_ENHMETAFILE
    comp.extend_from_slice(&[0; 4]);
    comp.extend_from_slice(&0x71b2_39f4u32.to_le_bytes()); // Unicode marker
    comp.extend_from_slice(&[0; 12]);
    let mut ole = [0u8; 20];
    ole[0] = 1;
    ole[3] = 2;
    // CONTENTS: the header size, a copy of the header, then the metafile
    let mut contents = 108u32.to_le_bytes().to_vec();
    contents.extend_from_slice(&emf[..108]);
    contents.extend_from_slice(emf);
    let cfb = crate::cfb::write(CLSID_PICTURE_EMF, &[("\u{1}Ole", &ole), ("\u{1}CompObj", &comp), ("CONTENTS", &contents)]);
    let mut out = vec![0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0];
    out.extend_from_slice(&(cfb.len() as u32).to_le_bytes());
    out.extend(cfb);
    out
}

/// A 24-bit bottom-up DIB (BITMAPINFOHEADER + rows) from RGBA pixels
/// (transparent pixels become white).
pub fn dib_24(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let stride = ((w * 3 + 3) / 4 * 4) as usize;
    let mut out = Vec::with_capacity(40 + stride * h as usize);
    for v in [40u32, w, h] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&((stride * h as usize) as u32).to_le_bytes());
    for v in [11811u32, 11811, 0, 0] {
        out.extend_from_slice(&v.to_le_bytes()); // 300 dpi, no palette
    }
    for row in (0..h as usize).rev() {
        let start = out.len();
        for x in 0..w as usize {
            let p = &rgba[(row * w as usize + x) * 4..][..4];
            let a = p[3] as u32;
            let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
            out.extend_from_slice(&[mix(p[2]), mix(p[1]), mix(p[0])]);
        }
        while out.len() - start < stride {
            out.push(0);
        }
    }
    out
}

fn utf16_str(b: &[u8]) -> String {
    let u: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&x| x != 0).collect();
    String::from_utf16_lossy(&u)
}

/// Text annotation size for a box in page units (for the UI).
pub fn em_100mm(pt: f64) -> f64 {
    pt * 2540.0 / 72.0
}

#[allow(dead_code)]
fn _unit() -> f64 {
    UNIT
}
