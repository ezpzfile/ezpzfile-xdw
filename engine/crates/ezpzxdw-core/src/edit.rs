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
            let em_u = (size * 300.0 / 72.0).round() as i32;
            e.font(em_u, if *bold { 700 } else { 400 }, false, FACE);
            e.text_color(*color);
            let m = emfw::units(MARGIN);
            let lh = (em_u as f64 * LINE).round() as i32;
            let top_pad = ((lh - em_u) / 2).max(0);
            for (k, line) in text.split('\n').enumerate() {
                if line.is_empty() {
                    continue;
                }
                let adv: Vec<i32> = line.chars().map(|c| (advance(c) * em_u as f64).round() as i32).collect();
                e.text(m, m + k as i32 * lh + top_pad, line, &adv);
            }
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

/// Year, month and day in a stamp date written like `'26.10.01`,
/// `2026.10.01`, `2026/10/1` or `2026-10-01`.
fn stamp_ymd(date: &str) -> Option<(String, String, String)> {
    let t = date.trim().trim_start_matches(['\'', '’']);
    let parts: Vec<&str> = t.split(['.', '/', '-']).map(|x| x.trim()).collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    Some((parts[0].to_string(), parts[1].to_string(), parts[2].to_string()))
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
            // The names follow DocuWorks' published API names for date
            // stamps; the viewer draws the stamp from the stored picture.
            kind = doc::K_STAMP;
            let (y, m, dd) = stamp_ymd(date).unwrap_or_default();
            attrs.push(defs_attr(&[
                (2001, 4, "%TopField"),
                (2002, 4, "%BottomField"),
                (2003, 2, "%DateStyle"),
                (2004, 4, "%YearField"),
                (2005, 4, "%MonthField"),
                (2006, 4, "%DayField"),
                (2007, 2, "%DateOrder"),
                (2008, 2, "%BorderColor"),
                (2009, 4, "%Text(w"),
            ]));
            attrs.push(named(2001, sjisz(top)));
            attrs.push(named(2002, sjisz(bottom)));
            attrs.push(int_attr(2003, &[1])); // manual date
            attrs.push(named(2004, sjisz(&y)));
            attrs.push(named(2005, sjisz(&m)));
            attrs.push(named(2006, sjisz(&dd)));
            attrs.push(int_attr(2007, &[0])); // year, month, day
            attrs.push(int_attr(2008, &[emfw::colorref(*color) as i64]));
            // the three lines as shown, so this editor can read them back
            attrs.push(named(2009, utf16z(&format!("{top}\n{date}\n{bottom}"))));
        }
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
        self.records.insert(at, place_record(d + 1, x, y, w, h));
        self.records.insert(at + 1, object_record(d + 2, id, shape, w, h));
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
        let end = subtree_end(&self.records, o.place);
        self.records.drain(o.place..end);
        self.refresh();
        Ok(())
    }

    pub fn move_object(&mut self, page: usize, obj: usize, x: f64, y: f64) -> Result<()> {
        let o = self.object(page, obj)?;
        if o.kind == K_CONTENT {
            return Err(Error::Unsupported("the page content cannot be moved".into()));
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
        let new_place = place_record(d - 1, x, y, w, h);
        let new_obj = object_record(d, id, shape, w, h);
        self.records.splice(o.place..end, [new_place, new_obj]);
        self.refresh();
        Ok(())
    }

    /// Add a picture annotation (DocuWorks "bitmap" annotation, kind
    /// 0x803f): `rgba` is `pw` × `ph` pixels, top row first; it is placed
    /// at (x, y) with size w × h (1/100 mm). Transparent pixels become white.
    pub fn add_picture(&mut self, page: usize, x: f64, y: f64, w: f64, h: f64, rgba: &[u8], pw: u32, ph: u32) -> Result<usize> {
        if pw == 0 || ph == 0 || rgba.len() < (pw * ph * 4) as usize {
            return Err(Error::Unsupported("picture size does not match its pixels".into()));
        }
        let raw = dib_24(rgba, pw, ph);
        let dib = crate::dib::encode_stored(&raw[..40], &raw[40..], ph);
        let mut body = element(0x80, &[7]);
        body.extend(element(0x81, &[40]));
        body.extend(element(0x84, &tlv::uint_bytes(w.round() as u64)));
        body.extend(element(0x85, &tlv::uint_bytes(h.round() as u64)));
        body.extend(element(0x89, &tlv::uint_bytes(dib.len() as u64)));
        body.extend(element(0x86, &dib));
        let id = self.next_id(page)?;
        let (_, reference) = self.add_entry(body, false);
        let pr = self.page(page)?.record;
        let d = self.records[pr].depth;
        let at = subtree_end(&self.records, pr);
        let obj = Record {
            depth: d + 2,
            kind: props::num_bytes(doc::K_PICTURE),
            attrs: vec![
                int_attr(57, &[1]),
                int_attr(5, &[w.round() as i64, h.round() as i64]),
                int_attr(61, &[0]),
                Attr { class: 0xc0, tag: 7, value: reference },
                int_attr(3, &[id]),
            ],
        };
        self.records.insert(at, place_record(d + 1, x, y, w, h));
        self.records.insert(at + 1, obj);
        self.refresh();
        Ok(self.page(page)?.objects.len() - 1)
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
                    if let Some((i, len)) = write::read_ref(&a.value) {
                        if let Some(&n) = map.get(&i) {
                            a.value = write::entry_ref(n, len);
                        }
                    }
                }
            }
        }
        (records, kept)
    }

    /// The document as a `.xdw` file (original bytes + one new segment).
    /// The result is read back and checked before it is returned.
    pub fn save(&self) -> Result<Vec<u8>> {
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
                let bg = ni("%ATTR_BKGND_COLOR").filter(|&c| c != 65793 && ni("%BkGndPermeable").unwrap_or(1) == 0).map(rgb);
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
                // only stamps made here keep their lines in %Text(w
                let t = utf16_str(&self.named(o.record, "%Text(w")?.value);
                let mut it = t.splitn(3, '\n');
                let (top, date, bottom) = (it.next()?.to_string(), it.next()?.to_string(), it.next()?.to_string());
                let color = rgb(ni("%BorderColor").unwrap_or(0x1200e6));
                Some(Shape::Stamp { top, date, bottom, color })
            }
            _ => None,
        }
    }

    /// Is `obj` one of the annotation kinds this editor can redraw?
    pub fn editable(&self, page: usize, obj: usize) -> bool {
        self.shape_of(page, obj).is_some()
    }
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
