//! Writing the whole file anew, and new documents.
//!
//! Saving by appending (`Document::save_append`) keeps everything the file
//! ever held: a page deleted in the editor is gone from the document, but
//! its drawing is still in the file for anyone who reads the bytes. Writing
//! the file anew keeps only what the document uses now:
//!
//! ```text
//! 0x60 header      as it was (generation, document or binder)
//! 0x61 segment     the entries the properties refer to, in their old order,
//!                  numbered from 0; the properties (references renumbered);
//!                  the trailer (same tag and 0x82 as before)
//! ```
//!
//! That is the shape of a document DocuWorks has saved once. A document with
//! a signature is still saved by appending, so the signed state stays in the
//! file the way DocuWorks keeps it.
//!
//! A new document is written the way DocuWorks 10 writes one: a generation 10
//! header, and properties with the root, the page list and one blank page
//! (a page record with a size and no content).

use crate::container::{T_TRAILER, T_HEADER};
use crate::doc::{Document, K_DOCUMENT, K_PAGE, K_ROOT};
use crate::edit::{defs_attr, int_attr};
use crate::error::{Error, Result};
use crate::props::{self, Record};
use crate::tlv;
use crate::write;

/// How a save was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveMode {
    /// The whole file written anew: only what the document uses.
    Fresh,
    /// One segment appended to the file as it was (documents with a signature).
    Append,
}

/// What a file written anew holds.
pub struct FreshParts {
    /// The records, with references renumbered.
    pub records: Vec<Record>,
    /// The entries in their new order: complete 0x64 elements.
    pub entries: Vec<Vec<u8>>,
    /// The entries' bodies (the 0x82 values), to check the result against.
    pub bodies: Vec<Vec<u8>>,
    /// (new number, check value) of raw pictures, for the trailer's 0x8d.
    pub checks: Vec<(u32, u32)>,
    /// Old entry number → new one (`None`: no longer used, left out).
    pub map: Vec<Option<u32>>,
}

/// The kinds of the records that sit under a document root, in the order
/// DocuWorks 10 writes them: the page list (1303), then 1304, 1306 (attached
/// original files) and 1305.
const K_1304: i64 = 0x1304;
const K_ORIGINALS: i64 = 0x1306;
const K_1305: i64 = 0x1305;

/// The header DocuWorks 10 writes: generation 10, a document (c013), and
/// 0x83 = 01 0d 0a 01, which every sample has.
fn header(kind: i64) -> Vec<u8> {
    let mut v = write::element(0x82, &[10]);
    v.extend(write::element(0x80, &tlv::uint_bytes(kind as u64)));
    v.extend(write::element(0x83, &[0x01, 0x0d, 0x0a, 0x01]));
    write::element(T_HEADER, &v)
}

/// Every entry reference in `records`: (record, attribute, entry number).
/// References are private-class attributes holding `81` number `82` length
/// (drawings, small pictures, pictures 301…, definitions kept in an entry,
/// embedded files, signature data).
fn references(records: &[Record]) -> Vec<(usize, usize, u32)> {
    let mut out = Vec::new();
    for (ri, r) in records.iter().enumerate() {
        for (ai, a) in r.attrs.iter().enumerate() {
            if a.class & 0xc0 != 0xc0 {
                continue;
            }
            if let Some((n, _)) = write::read_ref(&a.value) {
                out.push((ri, ai, n));
            }
        }
    }
    out
}

impl Document {
    /// A new document with one blank page `w` × `h` (1/100 mm; A4 portrait is
    /// 21000 × 29700), the way DocuWorks 10 writes a new document.
    pub fn blank(w: f64, h: f64) -> Result<Document> {
        if !(w >= 1000.0 && h >= 1000.0 && w <= 500_000.0 && h <= 500_000.0) {
            return Err(Error::Unsupported(format!("page size {w} × {h}")));
        }
        let lastmid = |n: i64| [defs_attr(&[(2001, 2, "lastmid")]), int_attr(2001, &[n])];
        let rec = |depth: u8, kind: i64, attrs: Vec<props::Attr>| Record { depth, kind: props::num_bytes(kind), attrs };
        let mut page = lastmid(0).to_vec();
        page.push(int_attr(5, &[w.round() as i64, h.round() as i64]));
        page.push(int_attr(3, &[1]));
        let records = vec![
            rec(0, K_ROOT, Vec::new()),
            rec(1, K_DOCUMENT, lastmid(1).to_vec()),
            rec(2, K_PAGE, page),
            rec(1, K_1304, Vec::new()),
            rec(1, K_ORIGINALS, Vec::new()),
            rec(1, K_1305, Vec::new()),
        ];
        let bytes = write::fresh(&header(K_ROOT), &[], &[], &records, T_TRAILER, &[0])?;
        Document::open(bytes)
    }

    /// The parts of the file written anew: the entries the records refer
    /// to, renumbered from 0 in their old order, and the records with their
    /// references changed to match.
    pub fn fresh_parts(&self) -> Result<FreshParts> {
        let own = self.entries.len();
        let total = own + self.pending.len();
        let refs = references(&self.records);
        let mut used = vec![false; total];
        for &(_, _, n) in &refs {
            let n = n as usize;
            if n >= total {
                return Err(Error::Corrupt(format!("a reference to entry {n}, but there are {total}")));
            }
            used[n] = true;
        }
        let mut map = vec![None; total];
        let (mut entries, mut bodies, mut checks) = (Vec::new(), Vec::new(), Vec::new());
        for i in (0..total).filter(|&i| used[i]) {
            let n = entries.len() as u32;
            map[i] = Some(n);
            if i < own {
                // the file's own entry, byte for byte as stored
                let e = &self.entries[i];
                entries.push(self.bytes[e.at..e.at + e.len].to_vec());
                bodies.push(self.bytes[e.body_range.0..e.body_range.0 + e.body_range.1].to_vec());
                if let Some(&(_, k)) = self.container.trailer.checks.iter().find(|&&(j, _)| j as usize == i) {
                    checks.push((n, k));
                }
            } else {
                let e = &self.pending[i - own];
                entries.push(write::entry_element(&e.body));
                bodies.push(e.body.clone());
                if e.picture {
                    checks.push((n, tlv::check(&e.body)));
                }
            }
        }
        let mut records = self.records.clone();
        for (ri, ai, n) in refs {
            let new = map[n as usize].expect("every reference is kept");
            if new != n {
                let a = &mut records[ri].attrs[ai];
                a.value = write::renumber_ref(&a.value, new);
            }
        }
        Ok(FreshParts { records, entries, bodies, checks, map })
    }

    /// The document written anew: only what it uses now (a deleted page or
    /// annotation leaves nothing behind). The result is read back and
    /// checked before it is returned.
    pub fn save_fresh(&self) -> Result<Vec<u8>> {
        let p = self.fresh_parts()?;
        let head = self.container.header;
        // the header element as stored: its tag, its length, its value
        let header = &self.bytes[..head.0 + head.1];
        if header.first() != Some(&T_HEADER) {
            return Err(Error::Corrupt("the header is not at the start".into()));
        }
        let bytes = write::fresh(header, &p.entries, &p.checks, &p.records, self.container.trailer.tag, &self.container.trailer.unknown82)?;
        let back = Document::open(bytes.clone())?;
        if back.records != p.records {
            return Err(Error::Corrupt("the saved file does not read back the same".into()));
        }
        if back.pages.len() != self.pages.len() {
            return Err(Error::Corrupt("page count changed while saving".into()));
        }
        if back.container.segments.len() != 1 || back.entries.len() != p.bodies.len() {
            return Err(Error::Corrupt("the saved file is not one segment with every entry".into()));
        }
        for (k, b) in p.bodies.iter().enumerate() {
            if back.entry_body(k) != Some(&b[..]) {
                return Err(Error::Corrupt("an entry does not read back the same".into()));
            }
        }
        Ok(bytes)
    }

    /// Save as `.xdw` / `.xbd`: written anew, or appended for a document with
    /// a signature. Returns the file and how it was written.
    pub fn save_with_mode(&self) -> Result<(Vec<u8>, SaveMode)> {
        if self.is_signed() {
            return Ok((self.save_append()?, SaveMode::Append));
        }
        Ok((self.save_fresh()?, SaveMode::Fresh))
    }

    /// Save as `.xdw` / `.xbd` (see `save_with_mode`).
    pub fn save(&self) -> Result<Vec<u8>> {
        self.save_with_mode().map(|(b, _)| b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::Shape;

    #[test]
    fn a_new_document_is_one_blank_page() {
        let d = Document::blank(21000.0, 29700.0).unwrap();
        assert_eq!(d.pages.len(), 1);
        assert_eq!((d.pages[0].w, d.pages[0].h), (21000, 29700));
        assert!(d.pages[0].objects.is_empty());
        assert_eq!(d.container.generation, Some(10));
        assert_eq!(d.container.segments.len(), 1);
        assert_eq!(d.entries.len(), 0);
        // the header DocuWorks 10 writes
        assert_eq!(d.bytes[..16], [0x60, 0x0e, 0x82, 0x01, 0x0a, 0x80, 0x03, 0x00, 0xc0, 0x13, 0x83, 0x04, 0x01, 0x0d, 0x0a, 0x01]);
        // a page list and a page that remember their last number
        assert_eq!(d.records[1].named_ints("lastmid"), vec![1]);
        assert_eq!(d.records[2].named_ints("lastmid"), vec![0]);
        assert_eq!(d.records[2].int(3), Some(1));
        let kinds: Vec<i64> = d.records.iter().map(|r| r.kind_num()).collect();
        assert_eq!(kinds, vec![K_ROOT, K_DOCUMENT, K_PAGE, K_1304, K_ORIGINALS, K_1305]);
        // saving it unchanged gives the same file
        assert_eq!(d.save().unwrap(), d.bytes);
        assert!(Document::blank(0.0, 29700.0).is_err());
    }

    #[test]
    fn a_new_document_takes_annotations_pages_and_pictures() {
        let mut d = Document::blank(29700.0, 21000.0).unwrap();
        let text = Shape::Text { text: "新しい文書".into(), size: 12.0, color: 0, bold: false, background: None, frame: None };
        d.add_annotation(0, 2000.0, 2000.0, 0.0, 0.0, &text).unwrap();
        d.insert_blank_page(1, 21000.0, 29700.0).unwrap();
        let px = vec![200u8; 4 * 4 * 4];
        d.add_picture(1, 1000.0, 1000.0, 3000.0, 3000.0, &px, 4, 4, false).unwrap();
        let (saved, mode) = d.save_with_mode().unwrap();
        assert_eq!(mode, SaveMode::Fresh);
        let back = Document::open(saved).unwrap();
        assert_eq!(back.pages.len(), 2);
        assert_eq!(back.shape_of(0, 0), Some(text));
        assert_eq!(back.pages[1].objects.len(), 1);
        assert_eq!(back.container.segments.len(), 1);
        assert_eq!(back.records[1].named_ints("lastmid"), vec![2]);
    }

    #[test]
    fn writing_anew_leaves_out_what_was_deleted() {
        let mut d = Document::blank(21000.0, 29700.0).unwrap();
        let mut px = vec![255u8; 8 * 8 * 4];
        px[0] = 0;
        // a picture page and a picture, saved once (both are entries)
        let jpeg = include_bytes!("../tests/data/tiny.jpg");
        let j = crate::pages::Jpeg { data: jpeg, w: 24, h: 16 };
        d.insert_image_page(1, 21000.0, 29700.0, &j, None, None).unwrap();
        d.add_picture(0, 1000.0, 1000.0, 3000.0, 3000.0, &px, 8, 8, true).unwrap();
        let once = Document::open(d.save().unwrap()).unwrap();
        assert!(once.entries.len() >= 3, "picture, page drawing, see-through picture");
        assert!(find(&once.bytes, jpeg));
        // delete the picture page: appending keeps its JPEG, writing anew does not
        let mut e = once.clone();
        e.delete_page(1).unwrap();
        let appended = e.save_append().unwrap();
        assert!(find(&appended, jpeg), "appending keeps the deleted page's picture");
        let fresh = e.save_fresh().unwrap();
        assert!(!find(&fresh, jpeg), "writing anew leaves it out");
        let back = Document::open(fresh.clone()).unwrap();
        assert_eq!(back.pages.len(), 1);
        assert!(fresh.len() < appended.len());
        // the see-through picture's entries were renumbered and still draw
        let o = &back.pages[0].objects[0];
        assert!(back.picture_image(o).is_some());
        assert_eq!(back.render(0).unwrap().skipped, Vec::<String>::new());
    }

    fn find(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }
}
