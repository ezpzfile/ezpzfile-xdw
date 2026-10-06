//! Saving, two ways.
//!
//! `append` adds one segment, the way DocuWorks itself saves. Nothing
//! already in the file is rewritten. The new segment holds the entries that
//! are new (page content, pictures), the complete new properties block, and
//! a trailer listing every entry in effect. Entries keep their numbers: the
//! old table is copied as it is and new entries are added at the end, so
//! references in the properties stay valid. Everything the file held before
//! stays in it, deleted pages included.
//!
//! `fresh` writes the whole file anew: the header, then one segment with
//! only the entries given (numbered from 0), the properties and the trailer.
//! This is also how a document DocuWorks has saved only once looks.
//!
//! What the viewer checks (found by testing DocuWorks Viewer Light):
//! - the trailer's last field gives the trailer's length, counted from the
//!   end of the file;
//! - 0x85 must be the check value of the stored properties block (all whole
//!   4-byte little-endian words XORed);
//! - entry check values (0x81) are not checked when opening, but are kept
//!   right anyway.

use crate::container::Container;
use crate::error::Result;
use crate::lzh;
use crate::props::{self, Record};
use crate::tlv;

/// Encode an element the way DocuWorks does: lengths below 0x80 in one
/// byte, `81 nn` up to 0xfe, and the two-byte form from 0xff on.
pub fn element(tag: u8, value: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let n = value.len();
    if n < 0x80 {
        out.push(n as u8);
    } else if n < 0xff {
        out.push(0x81);
        out.push(n as u8);
    } else if n <= 0xffff {
        out.push(0x82);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n <= 0xff_ffff {
        out.push(0x83);
        out.extend_from_slice(&(n as u32).to_be_bytes()[1..]);
    } else {
        out.push(0x84);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    }
    out.extend_from_slice(value);
    out
}

/// A new entry: the value of its `0x82` body, and whether it is a picture
/// stored raw (those are also listed in the trailer's 0x8d table).
#[derive(Debug, Clone, PartialEq)]
pub struct NewEntry {
    pub body: Vec<u8>,
    pub picture: bool,
}

/// Build the body of a page-content entry from fields, compressing the
/// data. `fields` are the fields before the data, without 0x81 / 0x89 /
/// 0x8a / 0x86 (those are added here).
pub fn content_body(kind: i64, fields: &[(u8, i64)], data: &[u8], compress: bool) -> Vec<u8> {
    let mut out = element(0x80, &props::num_bytes(kind));
    let stored = if compress { lzh::compress(data) } else { data.to_vec() };
    if compress {
        out.extend(element(0x81, &tlv::uint_bytes(data.len() as u64)));
    }
    for &(t, v) in fields {
        out.extend(element(t, &props::num_bytes(v)));
    }
    if compress {
        out.extend(element(0x8a, &[1]));
    }
    out.extend(element(0x89, &tlv::uint_bytes(stored.len() as u64)));
    out.extend(element(0x86, &stored));
    out
}

/// A reference to an entry, as the properties store it: `81` index, `82`
/// body length.
pub fn entry_ref(index: u32, body_len: usize) -> Vec<u8> {
    let mut v = element(0x81, &tlv::uint_bytes(index as u64));
    v.extend(element(0x82, &tlv::uint_bytes(body_len as u64)));
    v
}

/// A reference to a raw entry holding LHA-compressed data (an embedded OLE
/// object's file): `81` index, `82` expanded length, `83` stored length.
pub fn packed_ref(index: u32, expanded: usize, stored: usize) -> Vec<u8> {
    let mut v = entry_ref(index, expanded);
    v.extend(element(0x83, &tlv::uint_bytes(stored as u64)));
    v
}

/// The same reference pointing at entry `index` (other fields kept).
pub fn renumber_ref(v: &[u8], index: u32) -> Vec<u8> {
    let Ok(items) = tlv::list(v, 0, v.len()) else { return v.to_vec() };
    let mut out = Vec::with_capacity(v.len() + 2);
    for t in items {
        if t.tag == 0x81 {
            out.extend(element(0x81, &tlv::uint_bytes(index as u64)));
        } else {
            out.extend_from_slice(&v[t.start..t.end()]);
        }
    }
    out
}

/// Read a reference written by `entry_ref` (extra fields are ignored).
pub fn read_ref(v: &[u8]) -> Option<(u32, usize)> {
    let items = tlv::list(v, 0, v.len()).ok()?;
    let idx = tlv::find_uint(&items, v, 0x81)? as u32;
    let len = tlv::find_uint(&items, v, 0x82)? as usize;
    Some((idx, len))
}

/// A complete entry element: `0x64 { 0x81 check, 0x82 body }`.
pub fn entry_element(body: &[u8]) -> Vec<u8> {
    let mut v = element(0x81, &tlv::i32_bytes(tlv::check(body)));
    v.extend(element(0x82, body));
    element(crate::container::T_ENTRY, &v)
}

/// Append a segment to `orig` holding `new_entries` and `records`.
/// Returns the new file and the numbers given to the new entries.
pub fn append(orig: &[u8], c: &Container, new_entries: &[NewEntry], records: &[Record]) -> Result<(Vec<u8>, Vec<u32>)> {
    let first_new = c.trailer.offsets.len() as u32;
    let numbers: Vec<u32> = (0..new_entries.len() as u32).map(|k| first_new + k).collect();
    let entries: Vec<Vec<u8>> = new_entries.iter().map(|e| entry_element(&e.body)).collect();
    let mut checks = c.trailer.checks.clone();
    for (k, e) in new_entries.iter().enumerate() {
        if e.picture {
            checks.push((numbers[k], tlv::check(&e.body)));
        }
    }
    let seg = segment(orig.len(), &c.trailer.offsets, &entries, &checks, records, c.trailer.tag, &c.trailer.unknown82)?;
    let mut out = orig.to_vec();
    out.extend_from_slice(&seg);
    Ok((out, numbers))
}

/// A whole file: `header` (the complete 0x60 element), then one segment
/// holding `entries` (complete 0x64 elements, numbered from 0 in this
/// order), the properties and a trailer tagged `tag`. `checks` are the
/// (number, check value) pairs of raw pictures for the trailer's 0x8d.
pub fn fresh(header: &[u8], entries: &[Vec<u8>], checks: &[(u32, u32)], records: &[Record], tag: u8, unknown82: &[u8]) -> Result<Vec<u8>> {
    let seg = segment(header.len(), &[], entries, checks, records, tag, unknown82)?;
    let mut out = header.to_vec();
    out.extend_from_slice(&seg);
    Ok(out)
}

/// One segment starting at file offset `start`: `entries`, the properties
/// and a trailer whose table is `prior` followed by these entries.
fn segment(start: usize, prior: &[u32], entries: &[Vec<u8>], checks: &[(u32, u32)], records: &[Record], tag: u8, unknown82: &[u8]) -> Result<Vec<u8>> {
    let expanded = props::write(records);
    let stored = lzh::compress(&expanded);
    let props_el = element(crate::container::T_PROPERTIES, &stored);
    // The segment header's size depends on the segment's length, which
    // depends on the entry offsets in the trailer; settle it by trying.
    for hdr in [4usize, 5, 6, 2, 3] {
        let mut offsets: Vec<u32> = prior.to_vec();
        let mut at = start + hdr;
        for e in entries {
            offsets.push(at as u32);
            at += e.len();
        }
        let trailer = trailer_element(tag, unknown82, &offsets, checks, expanded.len(), &stored);
        let mut body = Vec::new();
        for e in entries {
            body.extend_from_slice(e);
        }
        body.extend_from_slice(&props_el);
        body.extend_from_slice(&trailer);
        let seg = element(crate::container::T_SEGMENT, &body);
        if seg.len() - body.len() == hdr {
            return Ok(seg);
        }
    }
    Err(crate::error::Error::Unsupported("segment too large".into()))
}

fn trailer_element(tag: u8, unknown82: &[u8], offsets: &[u32], checks: &[(u32, u32)], expanded: usize, stored: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend(element(0x80, &tlv::uint_bytes(offsets.len() as u64)));
    if !offsets.is_empty() {
        let mut o = Vec::new();
        for x in offsets {
            o.extend_from_slice(&x.to_le_bytes());
        }
        v.extend(element(0x81, &o));
    }
    if !checks.is_empty() {
        let mut o = Vec::new();
        for (i, k) in checks {
            o.extend_from_slice(&i.to_le_bytes());
            o.extend_from_slice(&k.to_le_bytes());
        }
        v.extend(element(0x8d, &o));
    }
    let u82: &[u8] = if unknown82.is_empty() { &[0] } else { unknown82 };
    v.extend(element(0x82, u82));
    v.extend(element(0x83, &tlv::uint_bytes(expanded as u64)));
    v.extend(element(0x84, &tlv::uint_bytes(stored.len() as u64)));
    v.extend(element(0x85, &tlv::i32_bytes(tlv::check(stored))));
    // the last field: the length of the trailer's value, this field included
    let self_len = (v.len() + 6) as u32;
    v.extend(element(0x86, &self_len.to_le_bytes()));
    element(tag, &v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_like_docuworks() {
        assert_eq!(element(0x63, &[0; 0x7f])[..2], [0x63, 0x7f]);
        assert_eq!(element(0x63, &[0; 0xfe])[..3], [0x63, 0x81, 0xfe]);
        assert_eq!(element(0x63, &[0; 0xff])[..4], [0x63, 0x82, 0x00, 0xff]);
        assert_eq!(element(0x63, &[0; 0x1_0000])[..5], [0x63, 0x83, 0x01, 0x00, 0x00]);
    }

    #[test]
    fn refs_round_trip() {
        let v = entry_ref(3, 6041);
        assert_eq!(v, vec![0x81, 1, 3, 0x82, 2, 0x17, 0x99]);
        let p = packed_ref(303, 1283094, 598778);
        assert_eq!(p, vec![0x81, 2, 1, 0x2f, 0x82, 3, 0x13, 0x94, 0x16, 0x83, 3, 0x09, 0x22, 0xfa]);
        assert_eq!(renumber_ref(&p, 2), vec![0x81, 1, 2, 0x82, 3, 0x13, 0x94, 0x16, 0x83, 3, 0x09, 0x22, 0xfa]);
        assert_eq!(read_ref(&v), Some((3, 6041)));
    }
}
