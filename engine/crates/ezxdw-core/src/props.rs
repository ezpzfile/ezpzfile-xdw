//! The properties block: the document's object tree.
//!
//! Once expanded, the block is a flat list of records written in tree order
//! (each record says how deep it sits):
//!
//! ```text
//! 62 len {
//!   80 01 depth            0 root, 1 document, 2 page, 3 placement, 4 object
//!   62 len { type }        what the record is (a number, e.g. 0x8010 page body)
//!   attributes …           one element each
//! }
//! ```
//!
//! Attribute tags use the usual one-byte form (`83` = tag 3) or the long
//! form (`9f 8f 51` = tag 2001). Tags from 2001 up are named per record by
//! attribute 51, a list of definitions (number, type, …, name).
//!
//! Numbers inside values are stored as a length byte and that many
//! big-endian two's-complement bytes; a value can hold several (a size is
//! `02 52 08 02 74 04` = 21000, 29700). Text values are stored as they are.

use crate::error::{Error, Result};
use serde::Serialize;

/// Attribute 51: names for this record's attributes 2001…
pub const A_DEFS: u32 = 51;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Attr {
    /// The class bits of the tag byte (0x80 context, 0xc0 private), and the
    /// "constructed" bit 0x20 as stored.
    pub class: u8,
    pub tag: u32,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Record {
    pub depth: u8,
    /// The record's type as stored (big-endian bytes, sign-preserving).
    pub kind: Vec<u8>,
    pub attrs: Vec<Attr>,
}

/// One attribute definition from attribute 51.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AttrDef {
    pub tag: u32,
    pub ty: i64,
    /// -1, or a position in a list ("#pd" 101 = the 101st picture).
    pub index: i64,
    pub name: String,
}

/// Read definitions: groups of five items (number, type, index, name
/// length, name). Large lists are kept in an entry of their own and the
/// attribute then holds a reference to it.
pub fn parse_defs(v: &[u8]) -> Vec<AttrDef> {
    items(v)
        .chunks(5)
        .filter(|c| c.len() == 5)
        .map(|c| AttrDef {
            tag: num(c[0]) as u32,
            ty: num(c[1]),
            index: num(c[2]),
            name: String::from_utf8_lossy(c[4]).trim_end_matches('\0').to_string(),
        })
        .collect()
}

impl Record {
    pub fn kind_num(&self) -> i64 {
        num(&self.kind)
    }
    pub fn get(&self, tag: u32) -> Option<&Attr> {
        self.attrs.iter().find(|a| a.tag == tag)
    }
    pub fn get_mut(&mut self, tag: u32) -> Option<&mut Attr> {
        self.attrs.iter_mut().find(|a| a.tag == tag)
    }
    /// The definitions from attribute 51 (when stored in the record).
    pub fn defs(&self) -> Vec<AttrDef> {
        match self.get(A_DEFS) {
            Some(a) if a.class & 0xc0 != 0xc0 => parse_defs(&a.value),
            _ => Vec::new(),
        }
    }
    /// The attribute named `name` (through the definitions).
    pub fn named(&self, name: &str) -> Option<&Attr> {
        let d = self.defs().into_iter().find(|d| d.name == name)?;
        self.get(d.tag)
    }
    pub fn named_tag(&self, name: &str) -> Option<u32> {
        self.defs().into_iter().find(|d| d.name == name).map(|d| d.tag)
    }
    /// The numbers stored in attribute `tag`.
    pub fn ints(&self, tag: u32) -> Vec<i64> {
        self.get(tag).map(|a| ints(&a.value)).unwrap_or_default()
    }
    pub fn int(&self, tag: u32) -> Option<i64> {
        self.ints(tag).first().copied()
    }
    pub fn named_ints(&self, name: &str) -> Vec<i64> {
        self.named(name).map(|a| ints(&a.value)).unwrap_or_default()
    }
    pub fn set(&mut self, class: u8, tag: u32, value: Vec<u8>) {
        match self.get_mut(tag) {
            Some(a) => a.value = value,
            None => self.attrs.push(Attr { class, tag, value }),
        }
    }
}

/// A big-endian two's-complement number.
pub fn num(b: &[u8]) -> i64 {
    if b.is_empty() || b.len() > 8 {
        return 0;
    }
    let neg = b[0] & 0x80 != 0;
    b.iter().fold(if neg { -1i64 } else { 0 }, |a, &x| (a << 8) | x as i64)
}

/// The shortest two's-complement bytes of `v`.
pub fn num_bytes(v: i64) -> Vec<u8> {
    let bytes = v.to_be_bytes();
    let mut k = 0;
    while k < 7 {
        let (a, b) = (bytes[k], bytes[k + 1]);
        if (a == 0 && b & 0x80 == 0) || (a == 0xff && b & 0x80 != 0) {
            k += 1;
        } else {
            break;
        }
    }
    bytes[k..].to_vec()
}

/// Split a value into its length-prefixed items.
pub fn items(v: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let n = v[i] as usize;
        if i + 1 + n > v.len() {
            break;
        }
        out.push(&v[i + 1..i + 1 + n]);
        i += 1 + n;
    }
    out
}

/// The numbers of a value made of length-prefixed items.
pub fn ints(v: &[u8]) -> Vec<i64> {
    items(v).into_iter().map(num).collect()
}

/// Encode numbers as length-prefixed items.
pub fn ints_value(vs: &[i64]) -> Vec<u8> {
    let mut out = Vec::new();
    for &v in vs {
        let b = num_bytes(v);
        out.push(b.len() as u8);
        out.extend_from_slice(&b);
    }
    out
}

fn read_len(b: &[u8], i: &mut usize) -> Result<usize> {
    let n = *b.get(*i).ok_or_else(|| bad(*i))? as usize;
    *i += 1;
    if n < 0x80 {
        return Ok(n);
    }
    let k = n & 0x7f;
    if k == 0 || k > 4 || *i + k > b.len() {
        return Err(bad(*i));
    }
    let v = b[*i..*i + k].iter().fold(0usize, |a, &x| (a << 8) | x as usize);
    *i += k;
    Ok(v)
}

fn bad(at: usize) -> Error {
    Error::Corrupt(format!("properties: bad element at {at}"))
}

/// One element: (class byte, tag number, value range, end).
fn element(b: &[u8], at: usize, end: usize) -> Result<(u8, u32, usize, usize, usize)> {
    let mut i = at;
    let first = *b.get(i).ok_or_else(|| bad(at))?;
    i += 1;
    let mut tag = (first & 0x1f) as u32;
    if tag == 0x1f {
        tag = 0;
        loop {
            let x = *b.get(i).ok_or_else(|| bad(at))?;
            i += 1;
            tag = (tag << 7) | (x & 0x7f) as u32;
            if x & 0x80 == 0 {
                break;
            }
        }
    }
    let len = read_len(b, &mut i)?;
    if i + len > end {
        return Err(bad(at));
    }
    Ok((first & 0xe0, tag, i, len, i + len))
}

pub fn parse(b: &[u8]) -> Result<Vec<Record>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let (class, tag, v, _len, end) = element(b, i, b.len())?;
        if tag != 2 || class != 0x60 {
            return Err(bad(i));
        }
        // depth
        let (_, t0, v0, l0, e0) = element(b, v, end)?;
        if t0 != 0 || l0 != 1 {
            return Err(bad(v));
        }
        let depth = b[v0];
        // type
        let (c1, t1, v1, l1, e1) = element(b, e0, end)?;
        if t1 != 2 || c1 != 0x60 {
            return Err(bad(e0));
        }
        let kind = b[v1..v1 + l1].to_vec();
        let mut attrs = Vec::new();
        let mut j = e1;
        while j < end {
            let (c, t, va, la, ea) = element(b, j, end)?;
            attrs.push(Attr { class: c, tag: t, value: b[va..va + la].to_vec() });
            j = ea;
        }
        out.push(Record { depth, kind, attrs });
        i = end;
    }
    Ok(out)
}

/// Lengths as DocuWorks writes them: one byte below 0x80, `81 nn` up to
/// 0xfe, and from 0xff on the two-byte form (`82 00 ff`), even where one
/// byte would do.
fn put_len(out: &mut Vec<u8>, n: usize) {
    if n < 0x80 {
        out.push(n as u8);
    } else if n < 0xff {
        out.push(0x81);
        out.push(n as u8);
    } else if n <= 0xffff {
        out.push(0x82);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        let bytes = (n as u32).to_be_bytes();
        let skip = bytes.iter().take_while(|&&x| x == 0).count();
        out.push(0x80 | (4 - skip) as u8);
        out.extend_from_slice(&bytes[skip..]);
    }
}

fn put_element(out: &mut Vec<u8>, class: u8, tag: u32, value: &[u8]) {
    if tag < 0x1f {
        out.push(class | tag as u8);
    } else {
        out.push(class | 0x1f);
        let mut groups = Vec::new();
        let mut t = tag;
        loop {
            groups.push((t & 0x7f) as u8);
            t >>= 7;
            if t == 0 {
                break;
            }
        }
        for (k, g) in groups.iter().rev().enumerate() {
            out.push(if k + 1 < groups.len() { g | 0x80 } else { *g });
        }
    }
    put_len(out, value.len());
    out.extend_from_slice(value);
}

/// Write records back (the exact inverse of `parse`).
pub fn write(records: &[Record]) -> Vec<u8> {
    let mut out = Vec::new();
    for r in records {
        let mut inner = Vec::new();
        put_element(&mut inner, 0x80, 0, &[r.depth]);
        put_element(&mut inner, 0x60, 2, &r.kind);
        for a in &r.attrs {
            put_element(&mut inner, a.class, a.tag, &a.value);
        }
        put_element(&mut out, 0x60, 2, &inner);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(ints(&[2, 0x52, 0x08, 2, 0x74, 0x04]), vec![21000, 29700]);
        assert_eq!(ints(&[1, 0xff]), vec![-1]);
        assert_eq!(ints_value(&[21000, 29700, -1, 600, 128]), vec![2, 0x52, 0x08, 2, 0x74, 0x04, 1, 0xff, 2, 2, 0x58, 2, 0, 0x80]);
    }

    #[test]
    fn records_round_trip() {
        let b: Vec<u8> = vec![
            0x62, 0x08, 0x80, 0x01, 0x00, 0x62, 0x03, 0x00, 0xc0, 0x13, // root
            0x62, 0x10, 0x80, 0x01, 0x02, 0x62, 0x02, 0x13, 0x01, 0x83, 0x02, 0x01, 0x01, 0x9f, 0x8f, 0x51, 0x01, 0x05,
        ];
        let r = parse(&b).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].depth, 2);
        assert_eq!(r[1].int(3), Some(1));
        assert_eq!(r[1].get(2001).unwrap().value, vec![5]);
        assert_eq!(write(&r), b);
    }
}
