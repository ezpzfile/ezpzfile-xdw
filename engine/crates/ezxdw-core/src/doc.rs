//! The document: pages and what sits on them, read from the object tree.
//!
//! ```text
//! [0] c013 root                     (a binder .xbd has c014 → 1401 → 1402 → c013 …)
//!   [1] 1303 document               lastmid
//!     [2] 1301 page                 5 = size (1/100 mm), 3 = number
//!       [3] 1302 placement          52 = position, childdim = size
//!         [4] 8010 page content     7 = drawing (entry or inline), 61 = rotation,
//!                                   58 = thumbnail, 300… = pictures for DWb
//!       [3] 1302 placement
//!         [4] 8011 text annotation  (and other annotation kinds)
//!   [1] 1304, 1306 (attached original files), 1305
//! ```
//!
//! An object's attribute 7 is its drawing: either a reference to an entry
//! (private class: `81` entry number, `82` length) or the entry body itself
//! (context class). Annotations keep a ready-made drawing of themselves
//! this way, so every object can be drawn the same way.

use crate::container::{Body, Container, Entry};
use crate::emf;
use crate::error::{Error, Result};
use crate::gfx::{Display, Image, ImageData, Item, Path};
use crate::lzh;
use crate::props::{self, Record};
use crate::tlv;
use crate::write;
use serde::Serialize;

pub const K_ROOT: i64 = 0xc013;
pub const K_DOCUMENT: i64 = 0x1303;
pub const K_PAGE: i64 = 0x1301;
pub const K_PLACE: i64 = 0x1302;
pub const K_CONTENT: i64 = 0x8010;
pub const K_TEXT: i64 = 0x8011;
pub const K_LINE: i64 = 0x803c;
pub const K_RECT: i64 = 0x803d;
pub const K_ELLIPSE: i64 = 0x803e;
pub const K_PICTURE: i64 = 0x803f;
pub const K_CUSTOM: i64 = 0x8045;
pub const K_OLE: i64 = 0x800f;
pub const K_PAGEFORM: i64 = 0x802e;
pub const K_STAMP: i64 = 0x8033;
pub const K_FUSEN: i64 = 0x801a;
pub const K_MARKER: i64 = 0x801b;
pub const K_POLYGON: i64 = 0x8042;
pub const K_RECEIVED: i64 = 0x8040;
pub const K_LINK: i64 = 0xc02f;

/// What kind of thing an object is, in words.
pub fn kind_name(k: i64) -> &'static str {
    match k {
        K_CONTENT => "page",
        K_TEXT => "text",
        K_LINE => "line",
        K_RECT => "rectangle",
        K_ELLIPSE => "ellipse",
        K_PICTURE => "picture",
        K_CUSTOM => "shape",
        K_OLE => "object",
        K_PAGEFORM => "header/footer",
        K_STAMP => "date stamp",
        K_FUSEN => "sticky note",
        K_MARKER => "marker",
        K_POLYGON => "polygon",
        K_RECEIVED => "received stamp",
        K_LINK => "link",
        _ => "annotation",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Object {
    /// Index of the placement record (1302).
    pub place: usize,
    /// Index of the object record.
    pub record: usize,
    pub kind: i64,
    pub kind_name: &'static str,
    /// Position and size on the page (1/100 mm).
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    /// Content rotation in degrees (page content only).
    pub rotation: i64,
    /// Text of a text annotation.
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Page {
    /// Index of the page record (1301).
    pub record: usize,
    pub w: i64,
    pub h: i64,
    pub objects: Vec<Object>,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub bytes: Vec<u8>,
    pub container: Container,
    pub records: Vec<Record>,
    pub entries: Vec<Entry>,
    pub pages: Vec<Page>,
    /// Attribute definitions kept in entries, by record index.
    pub ext_defs: std::collections::HashMap<usize, Vec<props::AttrDef>>,
    /// Entries added since opening (numbered after the file's own); they
    /// are written by the next save, if still referenced.
    pub pending: Vec<write::NewEntry>,
}

/// An entry's body: field list (starts with 0x80) or raw data.
#[derive(Debug, Clone, Copy)]
pub enum BodyRef<'a> {
    Fields(&'a [u8]),
    Raw(&'a [u8]),
}

impl<'a> BodyRef<'a> {
    fn of(b: &'a [u8]) -> BodyRef<'a> {
        if b.first() == Some(&0x80) && tlv::list(b, 0, b.len()).is_ok() {
            BodyRef::Fields(b)
        } else {
            BodyRef::Raw(b)
        }
    }
    pub fn bytes(&self) -> &'a [u8] {
        match self {
            BodyRef::Fields(b) | BodyRef::Raw(b) => b,
        }
    }
    /// The data as used: expanded drawing data, or the raw bytes.
    pub fn data(&self) -> Option<Vec<u8>> {
        match self {
            BodyRef::Fields(b) => Content::of_fields(b).map(|c| c.data),
            BodyRef::Raw(b) => Some(b.to_vec()),
        }
    }
}

/// Where an object's drawing is.
#[derive(Debug, Clone)]
pub enum Drawing {
    /// Entry number.
    Entry(usize),
    /// The body stored in the attribute itself.
    Inline(Vec<u8>),
}

impl Document {
    pub fn open(bytes: Vec<u8>) -> Result<Document> {
        let container = Container::parse(&bytes)?;
        let records = props::parse(&container.properties(&bytes)?)?;
        let entries = container.entries(&bytes)?;
        let mut d = Document { bytes, container, records, entries, pages: Vec::new(), ext_defs: Default::default(), pending: Vec::new() };
        d.refresh();
        Ok(d)
    }

    /// Body of entry `n` (the file's own or one added since opening).
    pub fn body(&self, n: usize) -> Option<BodyRef<'_>> {
        if let Some(e) = self.entries.get(n) {
            let b = &self.bytes[e.body_range.0..e.body_range.0 + e.body_range.1];
            return Some(match e.body {
                Body::Fields { .. } => BodyRef::Fields(b),
                Body::Raw(..) => BodyRef::Raw(b),
            });
        }
        self.pending.get(n - self.entries.len()).map(|e| BodyRef::of(&e.body))
    }

    /// Add an entry; returns its number and the reference value to store.
    pub fn add_entry(&mut self, body: Vec<u8>, picture: bool) -> (u32, Vec<u8>) {
        let n = (self.entries.len() + self.pending.len()) as u32;
        let r = write::entry_ref(n, body.len());
        self.pending.push(write::NewEntry { body, picture });
        (n, r)
    }

    /// Re-read pages and definitions after the records changed.
    pub fn refresh(&mut self) {
        self.ext_defs.clear();
        for i in 0..self.records.len() {
            let Some(a) = self.records[i].get(props::A_DEFS) else { continue };
            if a.class & 0xc0 != 0xc0 {
                continue;
            }
            let Some((n, _)) = write::read_ref(&a.value) else { continue };
            if let Some(v) = self.body(n as usize).and_then(|b| b.data()) {
                self.ext_defs.insert(i, props::parse_defs(&v));
            }
        }
        self.pages = self.find_pages();
    }

    /// A record's attribute definitions (stored in it or in an entry).
    pub fn defs(&self, record: usize) -> Vec<props::AttrDef> {
        match self.ext_defs.get(&record) {
            Some(v) => v.clone(),
            None => self.records.get(record).map(|r| r.defs()).unwrap_or_default(),
        }
    }

    /// The attribute of `record` named `name`.
    pub fn named(&self, record: usize, name: &str) -> Option<&props::Attr> {
        let tag = self.defs(record).into_iter().find(|d| d.name == name)?.tag;
        self.records.get(record)?.get(tag)
    }

    /// Pages in order (walks every document of a binder too).
    pub fn find_pages(&self) -> Vec<Page> {
        let r = &self.records;
        let mut pages = Vec::new();
        for (i, rec) in r.iter().enumerate() {
            if rec.kind_num() != K_PAGE {
                continue;
            }
            let d = rec.depth;
            let size = rec.ints(5);
            let mut objects = Vec::new();
            let mut j = i + 1;
            while j < r.len() && r[j].depth > d {
                if r[j].depth == d + 1 && r[j].kind_num() == K_PLACE {
                    let pl = &r[j];
                    let pos = pl.ints(52);
                    let dim = self.named(j, "childdim").map(|a| props::ints(&a.value)).unwrap_or_default();
                    if let Some(k) = (j + 1..r.len()).take_while(|&k| r[k].depth > pl.depth).find(|&k| r[k].depth == pl.depth + 1) {
                        let o = &r[k];
                        let kind = o.kind_num();
                        let osize = o.ints(5);
                        let text = if kind == K_TEXT {
                            self.named(k, "%Text(w").map(|a| utf16(&a.value)).or_else(|| self.named(k, "%Text").map(|a| crate::sjis::decode(&a.value)))
                        } else {
                            None
                        };
                        objects.push(Object {
                            place: j,
                            record: k,
                            kind,
                            kind_name: kind_name(kind),
                            x: pos.first().copied().unwrap_or(0),
                            y: pos.get(1).copied().unwrap_or(0),
                            w: dim.first().copied().or(osize.first().copied()).unwrap_or(0),
                            h: dim.get(1).copied().or(osize.get(1).copied()).unwrap_or(0),
                            rotation: o.int(61).unwrap_or(0),
                            text,
                        });
                    }
                }
                j += 1;
            }
            pages.push(Page {
                record: i,
                w: size.first().copied().unwrap_or(21000),
                h: size.get(1).copied().unwrap_or(29700),
                objects,
            });
        }
        pages
    }

    pub fn entry_body(&self, n: usize) -> Option<&[u8]> {
        self.body(n).map(|b| b.bytes())
    }

    /// An object's drawing reference.
    pub fn drawing(&self, record: usize) -> Option<Drawing> {
        let a = self.records.get(record)?.get(7)?;
        if a.class & 0xc0 == 0xc0 {
            write::read_ref(&a.value).map(|(i, _)| Drawing::Entry(i as usize))
        } else {
            Some(Drawing::Inline(a.value.clone()))
        }
    }

    /// The pictures (attributes 301…) an object's drawing uses, as stored.
    pub fn pictures(&self, record: usize) -> Vec<Vec<u8>> {
        let Some(r) = self.records.get(record) else { return Vec::new() };
        // 300: how many, as a little-endian number; the first hundred are
        // attributes 301…400, the rest are named "#pd" with their position
        let n = r.get(300).map(|a| a.value.iter().rev().fold(0u32, |acc, &x| (acc << 8) | x as u32)).unwrap_or(0);
        let defs = self.defs(record);
        (1..=n)
            .filter_map(|k| {
                if k <= 100 {
                    r.get(300 + k)
                } else {
                    let d = defs.iter().find(|d| d.name == "#pd" && d.index == k as i64)?;
                    r.get(d.tag)
                }
            })
            .filter_map(|a| write::read_ref(&a.value))
            .map(|(i, _)| self.body(i as usize).and_then(|b| b.data()).unwrap_or_default())
            .collect()
    }

    /// Draw page `n`.
    pub fn render(&self, n: usize) -> Result<Display> {
        let page = self.pages.get(n).ok_or_else(|| Error::Unsupported(format!("no page {n}")))?;
        let mut out = Display::new(page.w as f32, page.h as f32);
        for o in &page.objects {
            self.render_object(o, &mut out);
        }
        Ok(out)
    }

    pub fn render_object(&self, o: &Object, out: &mut Display) {
        let Some(dr) = self.drawing(o.record) else {
            out.skipped.push(format!("{} without drawing", o.kind_name));
            return;
        };
        let body = match &dr {
            Drawing::Entry(i) => match self.body(*i) {
                Some(BodyRef::Fields(b)) => Content::of_fields(b),
                _ => None,
            },
            Drawing::Inline(v) => Content::of_fields(v),
        };
        let Some(c) = body else {
            out.skipped.push(format!("{}: drawing not readable", o.kind_name));
            return;
        };
        let pics = self.pictures(o.record);
        let (x, y, w, h) = (o.x as f64, o.y as f64, o.w as f64, o.h as f64);
        let first = out.items.len();
        let rot = ((o.rotation % 360) + 360) % 360;
        // the drawing's own box before rotation
        let (bw, bh) = if rot == 90 || rot == 270 { (h, w) } else { (w, h) };
        match c.kind {
            4 | 1 if crate::wmf::is_wmf(&c.data) => {
                crate::wmf::render(&c.data, [0.0, 0.0, bw, bh], &pics, out);
            }
            4 | 1 if c.data.get(40..44) == Some(b" EMF") => {
                emf::render_opts(&c.data, [0.0, 0.0, bw, bh], &pics, out, o.kind != K_CONTENT);
            }
            1 | 7 => {
                // a bitmap page (DIB)
                if let Some((img, _)) = emf::external_picture(&c.data) {
                    out.images.push(img);
                    let image = (out.images.len() - 1) as u32;
                    out.items.push(Item::Image { image, m: [bw as f32, 0.0, 0.0, bh as f32, 0.0, 0.0], clip: 0, alpha: 1.0 });
                } else {
                    out.skipped.push(format!("{}: bitmap not readable", o.kind_name));
                }
            }
            k => out.skipped.push(format!("{}: drawing kind {k}", o.kind_name)),
        }
        // place: rotate within the box, then move to the object's position
        let m: [f64; 6] = match rot {
            // 61 = 90 turns the drawing clockwise (checked in Viewer Light)
            90 => [0.0, 1.0, -1.0, 0.0, x + w, y],
            180 => [-1.0, 0.0, 0.0, -1.0, x + w, y + h],
            270 => [0.0, -1.0, 1.0, 0.0, x, y + h],
            _ => [1.0, 0.0, 0.0, 1.0, x, y],
        };
        if m != [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] {
            transform_items(out, first, m);
        }
    }

    /// Text of every page (from the drawings and text annotations).
    pub fn text(&self) -> Vec<String> {
        (0..self.pages.len())
            .map(|k| self.render(k).map(|d| d.text()).unwrap_or_default())
            .collect()
    }
}

/// A drawing ready to render: kind (4 EMF, 1/7 bitmap) and expanded data.
pub struct Content {
    pub kind: i64,
    pub data: Vec<u8>,
}

impl Content {
    pub fn of_entry(b: &[u8], e: &Entry) -> Option<Content> {
        match &e.body {
            Body::Fields { kind, .. } => Some(Content { kind: kind.unwrap_or(0) as i64, data: e.expanded(b).ok()? }),
            Body::Raw(..) => None,
        }
    }
    /// From an inline body (the fields of an entry body).
    pub fn of_fields(v: &[u8]) -> Option<Content> {
        let items = tlv::list(v, 0, v.len()).ok()?;
        let kind = tlv::find_uint(&items, v, 0x80)? as i64;
        let data = tlv::find(&items, 0x86)?.bytes(v);
        let data = match crate::container::expanded_size(Some(kind as u64), tlv::find_uint(&items, v, 0x8a), tlv::find_uint(&items, v, 0x81), data.len()) {
            Some(n) => lzh::decompress(data, n).ok()?,
            None => data.to_vec(),
        };
        Some(Content { kind, data })
    }
}

/// The data of an entry as used (expanded; raw bodies as they are).
pub fn expanded_body(b: &[u8], e: &Entry) -> Option<Vec<u8>> {
    match &e.body {
        Body::Fields { .. } => e.expanded(b).ok(),
        Body::Raw(at, len) => Some(b[*at..at + len].to_vec()),
    }
}

pub fn utf16(v: &[u8]) -> String {
    let u: Vec<u16> = v.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&u).trim_end_matches('\0').to_string()
}

fn tp(m: &[f64; 6], x: f32, y: f32) -> (f32, f32) {
    let (x, y) = (x as f64, y as f64);
    ((m[0] * x + m[2] * y + m[4]) as f32, (m[1] * x + m[3] * y + m[5]) as f32)
}

fn tpath(m: &[f64; 6], p: &mut Path) {
    use crate::gfx::Seg;
    for s in p.0.iter_mut() {
        *s = match *s {
            Seg::M(x, y) => {
                let (a, b) = tp(m, x, y);
                Seg::M(a, b)
            }
            Seg::L(x, y) => {
                let (a, b) = tp(m, x, y);
                Seg::L(a, b)
            }
            Seg::C(a, b, c, d, e, f) => {
                let (a, b) = tp(m, a, b);
                let (c, d) = tp(m, c, d);
                let (e, f) = tp(m, e, f);
                Seg::C(a, b, c, d, e, f)
            }
            Seg::Z => Seg::Z,
        }
    }
}

/// Move/rotate items drawn from `first` on (and the clips they use).
fn transform_items(out: &mut Display, first: usize, m: [f64; 6]) {
    let scale = (m[0] * m[3] - m[1] * m[2]).abs().sqrt() as f32;
    let angle = (-m[1]).atan2(m[0]).to_degrees() as f32;
    let mut clips_done = std::collections::HashSet::new();
    let mut new_clips: std::collections::HashMap<u32, u32> = Default::default();
    for it in out.items[first..].iter_mut() {
        let clip = match it {
            Item::Fill { path, clip, .. } => {
                tpath(&m, path);
                clip
            }
            Item::Stroke { path, width, dash, clip, .. } => {
                tpath(&m, path);
                *width *= scale;
                for d in dash.iter_mut() {
                    *d *= scale;
                }
                clip
            }
            Item::Text { x, y, angle: a, size, xs, clip, .. } => {
                let (nx, ny) = tp(&m, *x, *y);
                *x = nx;
                *y = ny;
                *a += angle;
                *size *= scale;
                for v in xs.iter_mut() {
                    *v *= scale;
                }
                clip
            }
            Item::Image { m: im, clip, .. } => {
                let a = [im[0] as f64, im[1] as f64, im[2] as f64, im[3] as f64, im[4] as f64, im[5] as f64];
                let r = [
                    m[0] * a[0] + m[2] * a[1],
                    m[1] * a[0] + m[3] * a[1],
                    m[0] * a[2] + m[2] * a[3],
                    m[1] * a[2] + m[3] * a[3],
                    m[0] * a[4] + m[2] * a[5] + m[4],
                    m[1] * a[4] + m[3] * a[5] + m[5],
                ];
                *im = r.map(|v| v as f32);
                clip
            }
        };
        if *clip != 0 {
            // clips made while drawing this object are moved too (they are
            // not shared with earlier objects: copy if needed)
            let id = *clip;
            if let Some(&n) = new_clips.get(&id) {
                *clip = n;
            } else if clips_done.insert(id) {
                let mut c = out.clips[id as usize].clone();
                for p in c.0.iter_mut() {
                    tpath(&m, p);
                }
                out.clips.push(c);
                let n = (out.clips.len() - 1) as u32;
                new_clips.insert(id, n);
                *clip = n;
            }
        }
    }
}

/// A picture as an image (for the viewer): JPEG pass-through or RGBA.
pub fn image_bytes(img: &Image) -> (&'static str, &[u8]) {
    match &img.data {
        ImageData::Jpeg(d) => ("jpeg", d),
        ImageData::Rgba(d) => ("rgba", d),
    }
}
