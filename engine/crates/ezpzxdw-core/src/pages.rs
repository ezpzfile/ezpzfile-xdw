//! New pages: blank pages, pictures (JPEG) as pages, and pages copied from
//! another DocuWorks document; and the documents of a binder (`.xbd`):
//! list, rename, reorder, remove, add.
//!
//! A page is a record (1301) under its document's page list (1303), with
//! a number (attribute 3) taken from the list's "lastmid". Its content
//! (8010) refers to entries: the drawing (7), a small picture of the page
//! (58) and the pictures the drawing uses (300 = how many, 301…). Copying
//! a page copies those entries too and renumbers the references.

use crate::doc::{Document, K_CONTENT, K_DOCUMENT, K_PAGE, K_ROOT};
use crate::edit::{defs_attr, int_attr, named, place_record, sjisz, subtree_end, utf16z};
use crate::emfw::{self, Emf};
use crate::error::{Error, Result};
use crate::props::{self, Attr, Record};
use crate::tlv;
use crate::write::{self, element};
use std::collections::HashMap;

pub const K_BINDER: i64 = 0xc014;
/// A binder document's name as UTF-16 (DocuWorks 10; older ones: 4, Shift_JIS).
const A_NAME_W: u32 = 70;
const K_BINDER_LIST: i64 = 0x1401;
const K_BINDER_ITEM: i64 = 0x1402;

/// One document of a binder.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BinderDoc {
    /// Index of the document record (c013).
    pub record: usize,
    pub name: String,
    /// First page (in the binder's page order) and how many.
    pub first_page: usize,
    pub pages: usize,
}

/// A picture for a new page: JPEG bytes and its size in pixels.
pub struct Jpeg<'a> {
    pub data: &'a [u8],
    pub w: u32,
    pub h: u32,
}

/// The small picture of a page (58): RGBA pixels, top row first.
pub struct Thumb<'a> {
    pub rgba: &'a [u8],
    pub w: u32,
    pub h: u32,
}

fn parent(r: &[Record], i: usize) -> Option<usize> {
    let d = r.get(i)?.depth;
    (0..i).rev().find(|&j| r[j].depth < d)
}

/// The picture's place on a `pw` × `ph` page: as large as fits, centred.
pub fn fit(pw: f64, ph: f64, iw: f64, ih: f64) -> [f64; 4] {
    let k = (pw / iw).min(ph / ih);
    let (w, h) = (iw * k, ih * k);
    [(pw - w) / 2.0, (ph - h) / 2.0, w, h]
}

/// Width and height of a JPEG picture (from its frame header).
pub fn jpeg_size(d: &[u8]) -> Option<(u32, u32)> {
    if d.len() < 4 || d[0] != 0xff || d[1] != 0xd8 {
        return None;
    }
    let mut i = 2;
    while i + 4 <= d.len() {
        if d[i] != 0xff {
            i += 1;
            continue;
        }
        let m = d[i + 1];
        if m == 0xff {
            i += 1;
            continue;
        }
        if m == 0xd8 || (0xd0..=0xd7).contains(&m) || m == 0x01 {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([d[i + 2], d[i + 3]]) as usize;
        if (0xc0..=0xcf).contains(&m) && !matches!(m, 0xc4 | 0xc8 | 0xcc) {
            let h = u16::from_be_bytes([*d.get(i + 5)?, *d.get(i + 6)?]) as u32;
            let w = u16::from_be_bytes([*d.get(i + 7)?, *d.get(i + 8)?]) as u32;
            return (w > 0 && h > 0).then_some((w, h));
        }
        i += 2 + len;
    }
    None
}

/// Page size for a picture: A4, turned to match the picture.
pub fn a4_for(iw: u32, ih: u32) -> (f64, f64) {
    if iw > ih {
        (29700.0, 21000.0)
    } else {
        (21000.0, 29700.0)
    }
}

/// The name stored in a document or binder record (attribute 4). Seen:
/// document names in a binder as Shift_JIS + NUL, the binder's own name
/// as UTF-16 + one zero byte.
pub fn decode_name(v: &[u8]) -> String {
    let utf16 = |v: &[u8]| {
        let u: Vec<u16> = v.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&u| u != 0).collect();
        String::from_utf16_lossy(&u)
    };
    // UTF-16 starting with an ASCII character
    if v.len() >= 2 && v[0] != 0 && v[1] == 0 {
        return utf16(v);
    }
    let end = v.iter().position(|&b| b == 0).unwrap_or(v.len());
    let (text, _, bad) = encoding_rs::SHIFT_JIS.decode(&v[..end]);
    // half-width katakana in a name almost always means UTF-16 bytes read as
    // Shift_JIS (kana and kanji have 0x30… high bytes)
    let halfwidth = text.chars().any(|c| ('\u{ff61}'..='\u{ff9f}').contains(&c));
    if !bad && !halfwidth {
        return text.into_owned();
    }
    utf16(v)
}

/// A binder document's name: 70 (UTF-16) when there, else 4.
fn doc_name(rec: &Record) -> String {
    match rec.get(A_NAME_W) {
        Some(a) => {
            let u: Vec<u16> = a.value.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&u| u != 0).collect();
            String::from_utf16_lossy(&u)
        }
        None => rec.get(4).map(|a| decode_name(&a.value)).unwrap_or_default(),
    }
}

impl Document {
    /// Is this a binder (.xbd)?
    pub fn is_binder(&self) -> bool {
        self.records.first().map(|r| r.kind_num() == K_BINDER).unwrap_or(false)
    }

    /// Where a new page at position `at` (0 = before the first page,
    /// pages.len() = after the last) goes: (page list record, insert index).
    fn insert_point(&self, at: usize) -> Result<(usize, usize)> {
        if self.pages.is_empty() {
            return Err(Error::Unsupported("the document has no pages".into()));
        }
        let (pr, before) = if at < self.pages.len() { (self.pages[at].record, true) } else { (self.pages.last().unwrap().record, false) };
        let list = parent(&self.records, pr).filter(|&l| self.records[l].kind_num() == K_DOCUMENT).ok_or_else(|| Error::Unsupported("page without page list".into()))?;
        Ok((list, if before { pr } else { subtree_end(&self.records, pr) }))
    }

    fn new_page_records(&mut self, at: usize, mut recs: Vec<Record>) -> Result<usize> {
        let (list, idx) = self.insert_point(at)?;
        let id = self.bump_lastmid(list, 1);
        let shift = self.records[list].depth as i32 + 1 - recs[0].depth as i32;
        for r in &mut recs {
            r.depth = (r.depth as i32 + shift) as u8;
        }
        recs[0].set(0x80, 3, props::ints_value(&[id]));
        let n = recs.len();
        for (k, r) in recs.into_iter().enumerate() {
            self.records.insert(idx + k, r);
        }
        let _ = n;
        self.refresh();
        Ok(at.min(self.pages.len() - 1))
    }

    /// Insert a blank page `w` × `h` (1/100 mm) at position `at`.
    pub fn insert_blank_page(&mut self, at: usize, w: f64, h: f64) -> Result<usize> {
        let page = Record {
            depth: 0,
            kind: props::num_bytes(K_PAGE),
            attrs: vec![int_attr(5, &[w.round() as i64, h.round() as i64]), int_attr(3, &[0]), defs_attr(&[(2001, 2, "lastmid")]), int_attr(2001, &[0])],
        };
        self.new_page_records(at, vec![page])
    }

    /// Insert a page `w` × `h` (1/100 mm) showing a JPEG picture in `dst`
    /// (x, y, w, h; 1/100 mm; None = as large as fits, centred), stored the
    /// way DocuWorks stores scanned pages: the picture as its own entry,
    /// drawn by the page's drawing with DocuWorks' DWb / DWc records.
    pub fn insert_image_page(&mut self, at: usize, w: f64, h: f64, jpeg: &Jpeg, dst: Option<[f64; 4]>, thumb: Option<&Thumb>) -> Result<usize> {
        if jpeg.data.len() < 4 || jpeg.data[0] != 0xff || jpeg.data[1] != 0xd8 || jpeg.w == 0 || jpeg.h == 0 {
            return Err(Error::Unsupported("not a JPEG picture".into()));
        }
        let dst = dst.unwrap_or_else(|| fit(w, h, jpeg.w as f64, jpeg.h as f64));
        // the picture entry: total size, 0, width, height, then the JPEG
        let mut pic = Vec::with_capacity(16 + jpeg.data.len());
        for v in [16 + jpeg.data.len() as u32, 0, jpeg.w, jpeg.h] {
            pic.extend_from_slice(&v.to_le_bytes());
        }
        pic.extend_from_slice(jpeg.data);
        let (_, pic_ref) = self.add_entry(pic, true);
        // the page drawing, 600 dpi like the DocuWorks printer driver
        let px = |v: f64| (v / emfw::PAGE_UNIT).round() as i32;
        let (dw, dh) = (px(w), px(h));
        let mut e = Emf::page(dw, dh);
        e.stretch_mode(3);
        e.next_picture(jpeg.w as i32, jpeg.h as i32, [px(dst[0]), px(dst[1]), px(dst[2]).max(1), px(dst[3]).max(1)]);
        let emf = e.finish(w, h);
        let fields = [(0x84, w.round() as i64), (0x85, h.round() as i64), (0x8d, 3), (0x90, dw as i64), (0x91, dh as i64)];
        let body = write::content_body(4, &fields, &emf, true);
        let (_, draw_ref) = self.add_entry(body, false);
        let mut content = vec![int_attr(57, &[1]), int_attr(3, &[1]), Attr { class: 0xc0, tag: 7, value: draw_ref }];
        if let Some(t) = thumb {
            if let Some(r) = self.thumb_entry(t, w, h) {
                content.push(Attr { class: 0xc0, tag: 58, value: r });
            }
        }
        content.push(int_attr(5, &[w.round() as i64, h.round() as i64]));
        content.push(Attr { class: 0x80, tag: 300, value: vec![1, 0] });
        content.push(Attr { class: 0xc0, tag: 301, value: pic_ref });
        let recs = vec![
            Record {
                depth: 0,
                kind: props::num_bytes(K_PAGE),
                attrs: vec![int_attr(5, &[w.round() as i64, h.round() as i64]), int_attr(3, &[0]), defs_attr(&[(2001, 2, "lastmid")]), int_attr(2001, &[1])],
            },
            place_record(1, 0.0, 0.0, w, h),
            Record { depth: 2, kind: props::num_bytes(K_CONTENT), attrs: content },
        ];
        self.new_page_records(at, recs)
    }

    /// A page's small picture as an entry (kind 7, DocuWorks' stored DIB).
    fn thumb_entry(&mut self, t: &Thumb, w: f64, h: f64) -> Option<Vec<u8>> {
        if t.w == 0 || t.h == 0 || t.rgba.len() < (t.w * t.h * 4) as usize {
            return None;
        }
        let raw = crate::edit::dib_24(t.rgba, t.w, t.h);
        let dib = crate::dib::encode_stored(&raw[..40], &raw[40..], t.h);
        let mut body = element(0x80, &[7]);
        body.extend(element(0x81, &[40]));
        body.extend(element(0x84, &tlv::uint_bytes(w.round() as u64)));
        body.extend(element(0x85, &tlv::uint_bytes(h.round() as u64)));
        body.extend(element(0x89, &tlv::uint_bytes(dib.len() as u64)));
        body.extend(element(0x86, &dib));
        Some(self.add_entry(body, false).1)
    }

    /// Is entry `n` one of the pictures listed in the trailer (raw pictures)?
    fn is_picture_entry(&self, n: u32) -> bool {
        let own = self.entries.len() as u32;
        if n < own {
            self.container.trailer.checks.iter().any(|&(i, _)| i == n)
        } else {
            self.pending.get((n - own) as usize).map(|e| e.picture).unwrap_or(false)
        }
    }

    /// Copies of `src`'s records with every entry they refer to copied
    /// into this document (`map`: source entry → our entry).
    fn import(&mut self, src: &Document, recs: &[Record], map: &mut HashMap<u32, u32>) -> Result<Vec<Record>> {
        let mut out = recs.to_vec();
        for r in &mut out {
            for a in &mut r.attrs {
                if a.class & 0xc0 != 0xc0 {
                    continue;
                }
                let Some((i, len)) = write::read_ref(&a.value) else { continue };
                let n = match map.get(&i) {
                    Some(&n) => n,
                    None => {
                        let body = src.body(i as usize).ok_or_else(|| Error::Corrupt(format!("entry {i} missing")))?.bytes().to_vec();
                        let (n, _) = self.add_entry(body, src.is_picture_entry(i));
                        map.insert(i, n);
                        n
                    }
                };
                a.value = write::entry_ref(n, len);
            }
        }
        Ok(out)
    }

    /// Copy pages `which` of `src` (in that order) to position `at`.
    /// Returns how many were inserted.
    pub fn insert_pages_from(&mut self, at: usize, src: &Document, which: &[usize]) -> Result<usize> {
        let mut map = HashMap::new();
        let mut pos = at.min(self.pages.len());
        for &k in which {
            let p = src.pages.get(k).ok_or_else(|| Error::Unsupported(format!("no page {} in the other document", k + 1)))?;
            let end = subtree_end(&src.records, p.record);
            let recs = self.import(src, &src.records[p.record..end], &mut map)?;
            self.new_page_records(pos, recs)?;
            pos += 1;
        }
        Ok(which.len())
    }

    /// The documents of a binder, in order (empty for a plain document).
    pub fn binder_docs(&self) -> Vec<BinderDoc> {
        if !self.is_binder() {
            return Vec::new();
        }
        let r = &self.records;
        let mut out = Vec::new();
        for (i, rec) in r.iter().enumerate() {
            if rec.kind_num() != K_ROOT || i == 0 {
                continue;
            }
            let end = subtree_end(r, i);
            let first = self.pages.iter().position(|p| p.record > i && p.record < end);
            let n = self.pages.iter().filter(|p| p.record > i && p.record < end).count();
            out.push(BinderDoc {
                record: i,
                name: doc_name(rec),
                first_page: first.unwrap_or_else(|| out.last().map(|d: &BinderDoc| d.first_page + d.pages).unwrap_or(0)),
                pages: n,
            });
        }
        out
    }

    /// The binder's own name.
    pub fn binder_name(&self) -> Option<String> {
        self.is_binder().then(|| self.records[0].get(4).map(|a| decode_name(&a.value)).unwrap_or_default())
    }

    fn binder_item(&self, k: usize) -> Result<usize> {
        let d = self.binder_docs();
        let doc = d.get(k).ok_or_else(|| Error::Unsupported(format!("no document {} in the binder", k + 1)))?;
        parent(&self.records, doc.record).filter(|&p| self.records[p].kind_num() == K_BINDER_ITEM).ok_or_else(|| Error::Unsupported("binder document without its holder".into()))
    }

    pub fn rename_binder_doc(&mut self, k: usize, name: &str) -> Result<()> {
        let rec = self.binder_docs().get(k).map(|d| d.record).ok_or_else(|| Error::Unsupported(format!("no document {} in the binder", k + 1)))?;
        self.records[rec].set(0x80, 4, sjisz(name.trim()));
        if self.records[rec].get(A_NAME_W).is_some() {
            self.records[rec].set(0x80, A_NAME_W, utf16z(name.trim()));
        }
        self.refresh();
        Ok(())
    }

    pub fn delete_binder_doc(&mut self, k: usize) -> Result<()> {
        if self.binder_docs().len() <= 1 {
            return Err(Error::Unsupported("a binder needs at least one document".into()));
        }
        let it = self.binder_item(k)?;
        let end = subtree_end(&self.records, it);
        self.records.drain(it..end);
        self.refresh();
        Ok(())
    }

    /// Move binder document `from` so that it becomes document `to`.
    pub fn move_binder_doc(&mut self, from: usize, to: usize) -> Result<()> {
        let n = self.binder_docs().len();
        if from == to || to >= n {
            return Ok(());
        }
        let a = self.binder_item(from)?;
        let end = subtree_end(&self.records, a);
        let block: Vec<Record> = self.records.drain(a..end).collect();
        self.refresh();
        let at = if to >= n - 1 {
            let last = self.binder_item(n - 2)?;
            subtree_end(&self.records, last)
        } else {
            self.binder_item(to)?
        };
        for (k, r) in block.into_iter().enumerate() {
            self.records.insert(at + k, r);
        }
        self.refresh();
        Ok(())
    }

    /// Add the documents of `src` (a .xdw, or every document of a .xbd) to
    /// this binder at position `at`, the first named `name`.
    pub fn add_binder_docs(&mut self, at: usize, src: &Document, name: &str) -> Result<usize> {
        if !self.is_binder() {
            return Err(Error::Unsupported("not a binder".into()));
        }
        let list = self.records.iter().position(|r| r.kind_num() == K_BINDER_LIST).ok_or_else(|| Error::Unsupported("binder without document list".into()))?;
        // the documents to add: (records of c013 subtree, name)
        let mut parts: Vec<(usize, usize, String)> = Vec::new();
        if src.is_binder() {
            for d in src.binder_docs() {
                parts.push((d.record, subtree_end(&src.records, d.record), d.name));
            }
        } else if src.records.first().map(|r| r.kind_num()) == Some(K_ROOT) {
            parts.push((0, src.records.len(), name.to_string()));
        } else {
            return Err(Error::Unsupported("the other file is not a DocuWorks document".into()));
        }
        let docs = self.binder_docs();
        // DocuWorks 10 also keeps the name as UTF-16 (70); do the same when
        // the binder already does
        let wide = docs.iter().any(|d| self.records[d.record].get(A_NAME_W).is_some());
        let mut at_idx = if at < docs.len() { self.binder_item(at)? } else { subtree_end(&self.records, list) };
        let base = self.records[list].depth;
        let mut map = HashMap::new();
        for (k, (s, e, nm)) in parts.iter().enumerate() {
            let mut recs = self.import(src, &src.records[*s..*e], &mut map)?;
            let shift = base as i32 + 2 - recs[0].depth as i32;
            for r in &mut recs {
                r.depth = (r.depth as i32 + shift) as u8;
            }
            let id = self.bump_lastmid(list, 2);
            let nm = if k == 0 && !name.trim().is_empty() { name.trim() } else { nm.as_str() };
            recs[0].set(0x80, 4, sjisz(nm));
            if wide {
                recs[0].set(0x80, A_NAME_W, utf16z(nm));
            }
            recs[0].set(0x80, 3, props::ints_value(&[id]));
            let mut block = vec![Record { depth: base + 1, kind: props::num_bytes(K_BINDER_ITEM), attrs: Vec::new() }];
            block.extend(recs);
            let n = block.len();
            for (j, r) in block.into_iter().enumerate() {
                self.records.insert(at_idx + j, r);
            }
            at_idx += n;
        }
        self.refresh();
        Ok(parts.len())
    }
}

/// UTF-16 + NUL form some names use (kept for completeness).
#[allow(dead_code)]
pub(crate) fn name_utf16(s: &str) -> Vec<u8> {
    utf16z(s)
}

#[allow(dead_code)]
fn _named(tag: u32, v: Vec<u8>) -> Attr {
    named(tag, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        // the binder's own name: UTF-16 and one zero byte
        assert_eq!(decode_name(&[0xd0, 0x30, 0xa4, 0x30, 0xf3, 0x30, 0xc0, 0x30, 0xfc, 0x30, 0x00]), "バインダー");
        assert_eq!(decode_name(b"test_xdw_ver10-1\0"), "test_xdw_ver10-1");
        assert_eq!(decode_name(&sjisz("資料A（表紙）")), "資料A（表紙）");
        assert_eq!(decode_name(&utf16z("Ab")), "Ab");
    }

    #[test]
    fn jpeg_and_fit() {
        let j = include_bytes!("../tests/data/tiny.jpg");
        assert_eq!(jpeg_size(j), Some((24, 16)));
        assert_eq!(jpeg_size(b"not a jpeg"), None);
        assert_eq!(fit(200.0, 100.0, 50.0, 50.0), [50.0, 0.0, 100.0, 100.0]);
        assert_eq!(a4_for(3, 2), (29700.0, 21000.0));
    }
}
