//! Original data (原本): files a DocuWorks document carries along, such as
//! the Word or Excel file it was printed from ("原本添付").
//!
//! Each one is a record 0x1000 under the document's 0x1306 with three named
//! settings, as DocuWorks 10 writes them (made with its API and read back):
//! `nd` the data, `nf` the file name (Shift_JIS), `ut` when it was attached
//! (Unix time). Small data sits in `nd` itself; larger data is in an entry
//! that `nd` points to, LHA-compressed when that makes it smaller (then the
//! reference also says the stored length).

use serde::Serialize;

use crate::doc::Document;
use crate::edit::{defs_attr, int_attr, named, sjisz, subtree_end};
use crate::error::{Error, Result};
use crate::lzh;
use crate::props::{self, Attr, Record};
use crate::tlv;
use crate::write;

const K_ORIGINALS: i64 = 0x1306;
const K_ORIGINAL: i64 = 0x1000;
/// Data up to this size stays in the record (DocuWorks keeps a few bytes
/// there and puts 200 kB in an entry; where it draws the line is not known).
const INLINE_MAX: usize = 4096;

/// One file attached to a document as original data.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Original {
    pub name: String,
    pub size: usize,
    /// When it was attached (Unix time), if recorded.
    pub time: Option<i64>,
}

impl Document {
    /// Records of the original data, in order (a binder: every document's).
    fn original_records(&self) -> Vec<usize> {
        let r = &self.records;
        let mut out = Vec::new();
        for (i, rec) in r.iter().enumerate() {
            if rec.kind_num() != K_ORIGINALS {
                continue;
            }
            let end = subtree_end(r, i);
            for k in i + 1..end {
                if r[k].depth == rec.depth + 1 && r[k].kind_num() == K_ORIGINAL {
                    out.push(k);
                }
            }
        }
        out
    }

    /// The original data this document carries.
    pub fn originals(&self) -> Vec<Original> {
        self.original_records()
            .into_iter()
            .map(|k| {
                let name = self.named(k, "nf").map(|a| crate::sjis::decode(a.value.split(|&b| b == 0).next().unwrap_or(&[]))).unwrap_or_default();
                let size = self.named(k, "nd").map(|a| self.original_len(a)).unwrap_or(0);
                let time = self.named(k, "ut").and_then(|a| props::ints(&a.value).first().copied());
                Original { name, size, time }
            })
            .collect()
    }

    fn original_len(&self, a: &Attr) -> usize {
        if a.class & 0xc0 == 0xc0 {
            write::read_ref(&a.value).map(|(_, n)| n).unwrap_or(0)
        } else {
            a.value.len()
        }
    }

    /// The bytes of original data `k`.
    pub fn original_data(&self, k: usize) -> Result<Vec<u8>> {
        let rec = *self.original_records().get(k).ok_or_else(|| Error::Unsupported(format!("no original data {}", k + 1)))?;
        let a = self.named(rec, "nd").ok_or_else(|| Error::Unsupported("original data without its data".into()))?;
        if a.class & 0xc0 != 0xc0 {
            return Ok(a.value.clone());
        }
        let (n, expanded) = write::read_ref(&a.value).ok_or_else(|| Error::Unsupported("original data: bad reference".into()))?;
        let body = self.entry_body(n as usize).ok_or_else(|| Error::Unsupported(format!("original data: no entry {n}")))?;
        let items = tlv::list(&a.value, 0, a.value.len()).map_err(|_| Error::Unsupported("original data: bad reference".into()))?;
        let data = if tlv::find_uint(&items, &a.value, 0x83).is_some() { lzh::decompress(body, expanded)? } else { body.to_vec() };
        if data.len() != expanded {
            return Err(Error::Unsupported(format!("original data is {} bytes, the file says {expanded}", data.len())));
        }
        Ok(data)
    }

    /// Attach `data` as original data named `name` (to the document; in a
    /// binder, to its first document). `time` is Unix time.
    pub fn add_original(&mut self, name: &str, data: &[u8], time: i64) -> Result<usize> {
        let list = self.records.iter().position(|r| r.kind_num() == K_ORIGINALS).ok_or_else(|| Error::Unsupported("document without a place for original data".into()))?;
        let id = self.bump_lastmid(list, 1);
        let nd = if data.len() <= INLINE_MAX {
            named(2001, data.to_vec())
        } else {
            let packed = lzh::compress(data);
            if packed.len() < data.len() {
                let stored = packed.len();
                let (n, _) = self.add_entry(packed, true);
                Attr { class: 0xc0, tag: 2001, value: write::packed_ref(n, data.len(), stored) }
            } else {
                let (_, r) = self.add_entry(data.to_vec(), true);
                Attr { class: 0xc0, tag: 2001, value: r }
            }
        };
        let rec = Record {
            depth: self.records[list].depth + 1,
            kind: props::num_bytes(K_ORIGINAL),
            attrs: vec![
                int_attr(3, &[id]),
                defs_attr(&[(2001, 4, "nd"), (2002, 4, "nf"), (2003, 2, "ut")]),
                nd,
                named(2002, sjisz(name)),
                int_attr(2003, &[time]),
            ],
        };
        let at = subtree_end(&self.records, list);
        self.records.insert(at, rec);
        self.refresh();
        Ok(self.originals().len() - 1)
    }

    /// Take original data `k` off the document.
    pub fn delete_original(&mut self, k: usize) -> Result<()> {
        let rec = *self.original_records().get(k).ok_or_else(|| Error::Unsupported(format!("no original data {}", k + 1)))?;
        let end = subtree_end(&self.records, rec);
        self.records.drain(rec..end);
        self.refresh();
        Ok(())
    }
}
