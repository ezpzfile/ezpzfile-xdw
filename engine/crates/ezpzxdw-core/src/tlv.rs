//! The tag-length-value elements every part of a `.xdw` is made of.
//!
//! One tag byte, then a length in the short form (`< 0x80`: the length
//! itself) or the long form (`0x80 | k`, then `k` big-endian bytes), then the
//! value. Values of the context tags (`0x80`…) are often lists of elements
//! again.

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Tlv {
    pub tag: u8,
    /// Offset of the tag byte.
    pub start: usize,
    /// Offset of the first value byte.
    pub value: usize,
    pub len: usize,
}

impl Tlv {
    pub fn end(&self) -> usize {
        self.value + self.len
    }
    pub fn bytes<'a>(&self, b: &'a [u8]) -> &'a [u8] {
        &b[self.value..self.end()]
    }
    /// The value as a number. Numbers are stored like ASN.1 INTEGERs: the
    /// fewest big-endian bytes in two's complement, so `00 c0 13` is 49171
    /// and `a1 b1 ae` is negative (as a 32-bit value: `ffa1b1ae`). Returns the
    /// value sign-extended to 64 bits and reinterpreted as unsigned; use
    /// `as u32` for 32-bit fields such as check values.
    pub fn uint(&self, b: &[u8]) -> Option<u64> {
        if self.len == 0 || self.len > 9 {
            return None;
        }
        let v = self.bytes(b);
        if self.len == 9 && v[0] != 0 {
            return None;
        }
        let neg = v[0] & 0x80 != 0;
        Some(v.iter().fold(if neg { u64::MAX } else { 0 }, |a, &x| (a << 8) | x as u64))
    }
    /// The value read as a list of elements.
    pub fn children(&self, b: &[u8]) -> Result<Vec<Tlv>> {
        list(b, self.value, self.end())
    }
}

/// One element at `at`, which must end at or before `limit`.
pub fn read(b: &[u8], at: usize, limit: usize) -> Result<Tlv> {
    let bad = |what: &str| Error::Corrupt(format!("{what} at offset {at}"));
    let tag = *b.get(at).ok_or_else(|| bad("missing tag"))?;
    let n = *b.get(at + 1).ok_or_else(|| bad("missing length"))? as usize;
    let (len, value) = if n < 0x80 {
        (n, at + 2)
    } else {
        let k = n & 0x7f;
        if k == 0 || k > 4 {
            return Err(bad("bad length form"));
        }
        let bytes = b.get(at + 2..at + 2 + k).ok_or_else(|| bad("short length"))?;
        (bytes.iter().fold(0usize, |a, &x| (a << 8) | x as usize), at + 2 + k)
    };
    if value + len > limit.min(b.len()) {
        return Err(bad("element runs past its parent"));
    }
    Ok(Tlv { tag, start: at, value, len })
}

/// All elements packed in `b[start..end]`.
pub fn list(b: &[u8], start: usize, end: usize) -> Result<Vec<Tlv>> {
    let mut out = Vec::new();
    let mut i = start;
    while i < end {
        let t = read(b, i, end)?;
        i = t.end();
        out.push(t);
    }
    Ok(out)
}

pub fn find(items: &[Tlv], tag: u8) -> Option<Tlv> {
    items.iter().copied().find(|t| t.tag == tag)
}

pub fn find_uint(items: &[Tlv], b: &[u8], tag: u8) -> Option<u64> {
    find(items, tag).and_then(|t| t.uint(b))
}

/// Encode an element (short or long length form).
pub fn encode(tag: u8, value: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let n = value.len();
    if n < 0x80 {
        out.push(n as u8);
    } else {
        let bytes = (n as u32).to_be_bytes();
        let skip = bytes.iter().take_while(|&&x| x == 0).count();
        out.push(0x80 | (4 - skip) as u8);
        out.extend_from_slice(&bytes[skip..]);
    }
    out.extend_from_slice(value);
    out
}

/// A non-negative number as stored: the fewest big-endian bytes whose top
/// bit is clear (a leading `00` is added when it would be set).
pub fn uint_bytes(v: u64) -> Vec<u8> {
    let bytes = v.to_be_bytes();
    let mut skip = bytes.iter().take_while(|&&x| x == 0).count().min(7);
    if bytes[skip] & 0x80 != 0 && skip > 0 {
        skip -= 1;
    }
    let mut out = bytes[skip..].to_vec();
    if out[0] & 0x80 != 0 {
        out.insert(0, 0);
    }
    out
}

/// A 32-bit value (a check value) as stored: read as a signed number, so
/// values with the top bit set are written in the fewest bytes that
/// sign-extend back to them.
pub fn i32_bytes(v: u32) -> Vec<u8> {
    let v = v as i32 as i64;
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

/// The check value DocuWorks keeps next to a stored block (0x81 of a page
/// entry, 0x85 of a trailer): every whole 4-byte little-endian word of the
/// block XORed together. Bytes after the last whole word are not counted.
pub fn check(block: &[u8]) -> u32 {
    block
        .chunks_exact(4)
        .fold(0u32, |a, w| a ^ u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_round_trip() {
        for v in [0u64, 1, 0x7f, 0x80, 0xff, 0x100, 0xc013, 29700, 0x7fff_ffff, 0x1_0000_0000] {
            let e = encode(0x80, &uint_bytes(v));
            let t = read(&e, 0, e.len()).unwrap();
            assert_eq!(t.uint(&e), Some(v), "{v:#x}");
        }
        for v in [0u32, 1, 0x80, 0xffa1_b1ae, 0x8fb2_2ccd, 0x035f_ce57, 0xffff_ffff, 0x0080_0000] {
            let e = encode(0x81, &i32_bytes(v));
            let t = read(&e, 0, e.len()).unwrap();
            assert_eq!(t.uint(&e).map(|x| x as u32), Some(v), "{v:#x}");
        }
        assert_eq!(i32_bytes(0xffa1_b1ae), vec![0xa1, 0xb1, 0xae]);
        assert_eq!(uint_bytes(0xc013), vec![0, 0xc0, 0x13]);
    }

    #[test]
    fn check_ignores_the_tail() {
        assert_eq!(check(&[1, 0, 0, 0, 2, 0, 0, 0, 9]), 3);
    }
}
