//! The file as a whole: a header and one or more segments.
//!
//! ```text
//! 0x60  header      { 0x82 generation, 0x80 …, 0x83 … }
//! 0x61  segment     { 0x64 entry …, 0x63 properties, 0x68 trailer }
//! 0x61  segment     (each later save appends one)
//! ```
//!
//! A save does not rewrite the file: it appends a segment with the entries
//! that are new, a complete new properties block, and a trailer whose offset
//! table lists every entry in effect (old ones included). The last trailer
//! is the current state of the document, and it is found from the end of
//! the file: the last 4 bytes give the length of the trailer's value.
//!
//! Older files (generation 7) use tag 0x65 for the trailer. A few files have
//! a segment length that stops short of its trailer; the reader does not
//! depend on segment lengths.

use crate::error::{Error, Result};
use crate::lzh;
use crate::tlv::{self, Tlv};
use serde::Serialize;

pub const T_HEADER: u8 = 0x60;
pub const T_SEGMENT: u8 = 0x61;
pub const T_PROPERTIES: u8 = 0x63;
pub const T_ENTRY: u8 = 0x64;
pub const T_TRAILER: u8 = 0x68;
pub const T_TRAILER_OLD: u8 = 0x65;

#[derive(Debug, Clone, Serialize)]
pub struct Trailer {
    /// Offset of the trailer's tag byte.
    pub at: usize,
    pub tag: u8,
    /// Number of entries in effect (0x80).
    pub count: u32,
    /// Absolute offsets of the entries in effect (0x81, u32 LE each).
    pub offsets: Vec<u32>,
    /// 0x8d: (entry index, check value) pairs kept by some writers.
    pub checks: Vec<(u32, u32)>,
    /// 0x82, meaning unknown; kept as stored.
    pub unknown82: Vec<u8>,
    /// Size of the properties block once expanded (0x83).
    pub props_expanded: u32,
    /// Stored size of the properties block (0x84).
    pub props_stored: u32,
    /// 0x85: check value of the stored properties block.
    pub check: u32,
    /// Every field as stored: (tag, value bytes).
    #[serde(skip)]
    pub fields: Vec<(u8, Vec<u8>)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    pub at: usize,
    /// Declared end of the segment.
    pub end: usize,
    /// Entries stored in this segment (offsets of their tags).
    pub entries: Vec<usize>,
    /// The properties block: (value offset, length).
    pub props: Option<(usize, usize)>,
    pub trailer: Option<usize>,
}

/// How an entry stores its data.
#[derive(Debug, Clone, Serialize)]
pub enum Body {
    /// `0x82 { 0x80 kind, …, 0x86 data }`.
    Fields {
        kind: Option<u64>,
        /// Every field: (tag, number if it reads as one, value range).
        fields: Vec<(u8, Option<u64>, (usize, usize))>,
        /// The data field 0x86: (offset, length).
        data: Option<(usize, usize)>,
    },
    /// The value of 0x82 is the data itself: (offset, length).
    Raw(usize, usize),
}

/// One entry (0x64): `0x81` check value and the `0x82` body.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub at: usize,
    pub len: usize,
    pub check: Option<u32>,
    /// The `0x82` value: (offset, length). The check value is computed
    /// over these bytes.
    pub body_range: (usize, usize),
    pub body: Body,
}

impl Entry {
    pub fn kind(&self) -> Option<u64> {
        match &self.body {
            Body::Fields { kind, .. } => *kind,
            Body::Raw(..) => None,
        }
    }
    pub fn field(&self, tag: u8) -> Option<u64> {
        match &self.body {
            Body::Fields { fields, .. } => fields.iter().find(|f| f.0 == tag).and_then(|f| f.1),
            Body::Raw(..) => None,
        }
    }
    /// The stored data (0x86, or the whole raw body).
    pub fn data<'a>(&self, b: &'a [u8]) -> &'a [u8] {
        match &self.body {
            Body::Fields { data: Some((at, len)), .. } => &b[*at..at + len],
            Body::Fields { data: None, .. } => &[],
            Body::Raw(at, len) => &b[*at..at + len],
        }
    }
    /// The data as the program uses it: expanded when compressed.
    pub fn expanded(&self, b: &[u8]) -> Result<Vec<u8>> {
        let d = self.data(b);
        match expanded_size(self.kind(), self.field(0x8a), self.field(0x81), d.len()) {
            Some(n) => lzh::decompress(d, n),
            None => Ok(d.to_vec()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Container {
    pub size: usize,
    pub generation: Option<u64>,
    /// The header's raw value.
    pub header: (usize, usize),
    pub segments: Vec<Segment>,
    pub trailer: Trailer,
}

impl Container {
    pub fn parse(b: &[u8]) -> Result<Container> {
        if b.len() < 24 || b[0] != T_HEADER {
            return Err(Error::NotXdw("does not start with the DocuWorks header".into()));
        }
        let header = tlv::read(b, 0, b.len())?;
        let hf = header.children(b)?;
        let generation = tlv::find_uint(&hf, b, 0x82);
        let trailer = find_trailer(b)?;
        let segments = scan_segments(b, header.end());
        Ok(Container {
            size: b.len(),
            generation,
            header: (header.value, header.len),
            segments,
            trailer,
        })
    }

    /// Where the current properties block is stored: (value offset, length).
    pub fn properties_range(&self, b: &[u8]) -> Result<(usize, usize)> {
        let stored = self.trailer.props_stored as usize;
        // the block sits right before the trailer: 0x63, its length, value
        for hdr in [2usize, 3, 4, 5, 6] {
            let Some(start) = self.trailer.at.checked_sub(stored + hdr) else {
                continue;
            };
            if b[start] != T_PROPERTIES {
                continue;
            }
            if let Ok(t) = tlv::read(b, start, b.len()) {
                if t.len == stored && t.end() == self.trailer.at {
                    return Ok((t.value, t.len));
                }
            }
        }
        Err(Error::Corrupt("properties block not found before the trailer".into()))
    }

    /// The current properties block as stored (compressed).
    pub fn properties_stored<'a>(&self, b: &'a [u8]) -> Result<&'a [u8]> {
        let (at, len) = self.properties_range(b)?;
        Ok(&b[at..at + len])
    }

    /// The current properties block, expanded.
    pub fn properties(&self, b: &[u8]) -> Result<Vec<u8>> {
        lzh::decompress(self.properties_stored(b)?, self.trailer.props_expanded as usize)
    }

    /// Entries in effect, in table order.
    pub fn entries(&self, b: &[u8]) -> Result<Vec<Entry>> {
        self.trailer
            .offsets
            .iter()
            .map(|&o| parse_entry(b, o as usize))
            .collect()
    }
}

/// Whether stored data is compressed, and to what size. Entries say so
/// with 0x8a = 1; drawings stored inside the properties leave 0x8a out, so
/// an expanded size (0x81) larger than the data also means compressed.
/// Thumbnails (kind 7) use 0x81 for something else.
pub fn expanded_size(kind: Option<u64>, flag: Option<u64>, size: Option<u64>, stored: usize) -> Option<usize> {
    let n = size? as usize;
    if flag == Some(1) || (kind != Some(7) && flag.is_none() && n > stored) {
        Some(n)
    } else {
        None
    }
}

/// The trailer, found from the end of the file.
pub fn find_trailer(b: &[u8]) -> Result<Trailer> {
    let n = b.len();
    let self_len = u32::from_le_bytes([b[n - 4], b[n - 3], b[n - 2], b[n - 1]]) as usize;
    let value = n
        .checked_sub(self_len)
        .ok_or_else(|| Error::Corrupt("trailer length past the start of the file".into()))?;
    for hdr in 2..=6usize {
        let Some(at) = value.checked_sub(hdr) else { break };
        if !matches!(b[at], T_TRAILER | T_TRAILER_OLD) {
            continue;
        }
        if let Ok(t) = tlv::read(b, at, n) {
            if t.value == value && t.end() == n {
                return parse_trailer(b, &t);
            }
        }
    }
    Err(Error::NotXdw("no trailer at the end of the file".into()))
}

fn parse_trailer(b: &[u8], t: &Tlv) -> Result<Trailer> {
    let f = t.children(b)?;
    let le32s = |x: Tlv| -> Vec<u32> {
        x.bytes(b)
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    };
    let checks = tlv::find(&f, 0x8d)
        .map(le32s)
        .unwrap_or_default()
        .chunks_exact(2)
        .map(|p| (p[0], p[1]))
        .collect();
    Ok(Trailer {
        at: t.start,
        tag: t.tag,
        count: tlv::find_uint(&f, b, 0x80).unwrap_or(0) as u32,
        offsets: tlv::find(&f, 0x81).map(le32s).unwrap_or_default(),
        checks,
        unknown82: tlv::find(&f, 0x82).map(|x| x.bytes(b).to_vec()).unwrap_or_default(),
        props_expanded: tlv::find_uint(&f, b, 0x83).unwrap_or(0) as u32,
        props_stored: tlv::find_uint(&f, b, 0x84).unwrap_or(0) as u32,
        check: tlv::find_uint(&f, b, 0x85).unwrap_or(0) as u32,
        fields: f.iter().map(|x| (x.tag, x.bytes(b).to_vec())).collect(),
    })
}

/// The segments, read leniently (for inspection only).
fn scan_segments(b: &[u8], mut i: usize) -> Vec<Segment> {
    let mut out = Vec::new();
    while i + 2 <= b.len() {
        let Ok(t) = tlv::read(b, i, usize::MAX).or_else(|_| read_clamped(b, i)) else {
            break;
        };
        if t.tag != T_SEGMENT {
            if t.len == 0 && t.tag == 0 {
                break;
            }
            i = t.end();
            continue;
        }
        let mut seg = Segment {
            at: t.start,
            end: t.end(),
            entries: Vec::new(),
            props: None,
            trailer: None,
        };
        let mut j = t.value;
        let mut last_end = j;
        while j < b.len() {
            let Ok(c) = tlv::read(b, j, b.len()) else { break };
            match c.tag {
                T_ENTRY => seg.entries.push(c.start),
                T_PROPERTIES => seg.props = Some((c.value, c.len)),
                T_TRAILER | T_TRAILER_OLD => seg.trailer = Some(c.start),
                _ => break,
            }
            j = c.end();
            last_end = j;
            if matches!(c.tag, T_TRAILER | T_TRAILER_OLD) || j >= t.end() {
                break;
            }
        }
        i = last_end.max(t.end().min(b.len()));
        seg.end = seg.end.max(last_end);
        out.push(seg);
    }
    out
}

fn read_clamped(b: &[u8], at: usize) -> Result<Tlv> {
    let mut t = tlv::read(b, at, usize::MAX)?;
    t.len = t.len.min(b.len() - t.value);
    Ok(t)
}

pub fn parse_entry(b: &[u8], at: usize) -> Result<Entry> {
    let t = tlv::read(b, at, b.len())?;
    if t.tag != T_ENTRY {
        return Err(Error::Corrupt(format!("no entry at {at}")));
    }
    let f = t.children(b)?;
    let check = tlv::find_uint(&f, b, 0x81).map(|v| v as u32);
    let body = tlv::find(&f, 0x82).ok_or_else(|| Error::Corrupt("entry without body".into()))?;
    let as_fields = if b.get(body.value) == Some(&0x80) {
        body.children(b).ok()
    } else {
        None
    };
    let parsed = match as_fields {
        Some(bf) => Body::Fields {
            kind: tlv::find_uint(&bf, b, 0x80),
            fields: bf
                .iter()
                .map(|x| (x.tag, if x.tag == 0x86 { None } else { x.uint(b) }, (x.value, x.len)))
                .collect(),
            data: tlv::find(&bf, 0x86).map(|x| (x.value, x.len)),
        },
        None => Body::Raw(body.value, body.len),
    };
    Ok(Entry {
        at,
        len: t.end() - at,
        check,
        body_range: (body.value, body.len),
        body: parsed,
    })
}
