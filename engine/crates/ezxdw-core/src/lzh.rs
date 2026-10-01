//! LHA `-lh5-` decompression (and a minimal compressor for writing).
//!
//! DocuWorks stores its properties block and its vector page bodies as raw
//! `-lh5-` bit streams (no LHA archive header): blocks of static Huffman
//! codes over an 8 KiB sliding window. Bits are read most significant first.
//!
//! Block layout:
//!
//! ```text
//! u16 count of codes in the block
//! code lengths of the "pt" helper alphabet (19 symbols, 5-bit count)
//! code lengths of the literal/length alphabet (510 symbols, 9-bit count),
//!     run-length coded with the pt alphabet
//! code lengths of the position alphabet (14 symbols, 4-bit count)
//! `count` codes: a literal byte (< 256) or a match of length c - 253
//!     followed by a position code
//! ```

use crate::error::{Error, Result};

const NC: usize = 510; // 256 literals + lengths 3..=256
const NT: usize = 19;
const NP: usize = 14; // 8 KiB window: positions up to 2^13
const CBIT: u32 = 9;
const TBIT: u32 = 5;
const PBIT: u32 = 4;

struct Bits<'a> {
    b: &'a [u8],
    pos: usize, // bit position
}

impl<'a> Bits<'a> {
    fn bit(&mut self) -> u32 {
        let byte = self.b.get(self.pos / 8).copied().unwrap_or(0);
        let v = (byte >> (7 - (self.pos % 8))) & 1;
        self.pos += 1;
        v as u32
    }
    fn get(&mut self, n: u32) -> u32 {
        let mut v = 0;
        for _ in 0..n {
            v = (v << 1) | self.bit();
        }
        v
    }
    /// The next `n` bits without consuming them.
    fn peek(&self, n: u32) -> u32 {
        let mut t = Bits { b: self.b, pos: self.pos };
        t.get(n)
    }
    fn exhausted(&self) -> bool {
        self.pos > self.b.len() * 8 + 16
    }
}

/// A canonical Huffman code (codes assigned by length, then symbol order).
struct Huff {
    /// Only symbol when every length is zero.
    single: Option<u16>,
    first: [u32; 17],
    count: [u32; 17],
    offset: [u32; 17],
    symbols: Vec<u16>,
}

impl Huff {
    fn single(sym: u16) -> Huff {
        Huff {
            single: Some(sym),
            first: [0; 17],
            count: [0; 17],
            offset: [0; 17],
            symbols: Vec::new(),
        }
    }
    fn new(lens: &[u8]) -> Result<Huff> {
        let mut count = [0u32; 17];
        for &l in lens {
            if l > 16 {
                return Err(Error::Corrupt("Huffman code longer than 16 bits".into()));
            }
            if l > 0 {
                count[l as usize] += 1;
            }
        }
        let mut first = [0u32; 17];
        let mut offset = [0u32; 17];
        let mut code = 0u32;
        let mut idx = 0u32;
        for l in 1..=16 {
            code = (code + count[l - 1]) << 1;
            if l == 1 {
                code = 0;
            }
            first[l] = code;
            offset[l] = idx;
            idx += count[l];
        }
        let mut symbols = vec![0u16; idx as usize];
        let mut next = offset;
        for (s, &l) in lens.iter().enumerate() {
            if l > 0 {
                symbols[next[l as usize] as usize] = s as u16;
                next[l as usize] += 1;
            }
        }
        Ok(Huff {
            single: None,
            first,
            count,
            offset,
            symbols,
        })
    }
    fn decode(&self, r: &mut Bits) -> Result<u16> {
        if let Some(s) = self.single {
            return Ok(s);
        }
        let mut code = 0u32;
        for l in 1..=16 {
            code = (code << 1) | r.bit();
            let c = self.count[l];
            if c > 0 && code >= self.first[l] && code - self.first[l] < c {
                return Ok(self.symbols[(self.offset[l] + code - self.first[l]) as usize]);
            }
        }
        Err(Error::Corrupt("invalid Huffman code".into()))
    }
}

fn read_pt_len(r: &mut Bits, nn: usize, nbit: u32, special: Option<usize>) -> Result<Huff> {
    let n = r.get(nbit) as usize;
    if n == 0 {
        return Ok(Huff::single(r.get(nbit) as u16));
    }
    if n > nn {
        return Err(Error::Corrupt("pt table too long".into()));
    }
    let mut lens = vec![0u8; nn];
    let mut i = 0;
    while i < n {
        let mut c = r.get(3);
        if c == 7 {
            while r.bit() == 1 {
                c += 1;
                if c > 16 {
                    return Err(Error::Corrupt("pt length overflow".into()));
                }
            }
        }
        lens[i] = c as u8;
        i += 1;
        if Some(i) == special {
            let z = r.get(2) as usize;
            for _ in 0..z {
                if i < nn {
                    lens[i] = 0;
                    i += 1;
                }
            }
        }
    }
    Huff::new(&lens)
}

fn read_c_len(r: &mut Bits, pt: &Huff) -> Result<Huff> {
    let n = r.get(CBIT) as usize;
    if n == 0 {
        return Ok(Huff::single(r.get(CBIT) as u16));
    }
    if n > NC {
        return Err(Error::Corrupt("c table too long".into()));
    }
    let mut lens = vec![0u8; NC];
    let mut i = 0;
    while i < n {
        let c = pt.decode(r)? as usize;
        if c <= 2 {
            let run = match c {
                0 => 1,
                1 => r.get(4) as usize + 3,
                _ => r.get(CBIT) as usize + 20,
            };
            for _ in 0..run {
                if i < NC {
                    lens[i] = 0;
                    i += 1;
                }
            }
        } else {
            lens[i] = (c - 2) as u8;
            i += 1;
        }
    }
    Huff::new(&lens)
}

/// Decompress a raw `-lh5-` stream into exactly `out_len` bytes.
pub fn decompress(src: &[u8], out_len: usize) -> Result<Vec<u8>> {
    const WINDOW: usize = 1 << 13;
    let mut out = Vec::with_capacity(out_len);
    let mut r = Bits { b: src, pos: 0 };
    while out.len() < out_len {
        if r.exhausted() {
            return Err(Error::Corrupt("compressed data ends early".into()));
        }
        let mut blocksize = r.get(16);
        if blocksize == 0 {
            return Err(Error::Corrupt("empty LZH block".into()));
        }
        let pt = read_pt_len(&mut r, NT, TBIT, Some(3))?;
        let c = read_c_len(&mut r, &pt)?;
        let p = read_pt_len(&mut r, NP, PBIT, None)?;
        while blocksize > 0 && out.len() < out_len {
            blocksize -= 1;
            let sym = c.decode(&mut r)? as usize;
            if sym < 256 {
                out.push(sym as u8);
            } else {
                let len = sym - 256 + 3;
                let mut d = p.decode(&mut r)? as usize;
                if d > 1 {
                    d = (1 << (d - 1)) + r.get(d as u32 - 1) as usize;
                }
                let dist = d + 1;
                if dist > out.len() || dist > WINDOW {
                    return Err(Error::Corrupt("LZH match points before the start".into()));
                }
                let from = out.len() - dist;
                for k in 0..len {
                    if out.len() >= out_len {
                        break;
                    }
                    let b = out[from + k];
                    out.push(b);
                }
            }
            if r.exhausted() {
                return Err(Error::Corrupt("compressed data ends early".into()));
            }
        }
    }
    let _ = r.peek(0);
    Ok(out)
}

// ---------------------------------------------------------------- writer

struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, v: u32, bits: u32) {
        for i in (0..bits).rev() {
            self.acc = (self.acc << 1) | ((v >> i) & 1);
            self.n += 1;
            if self.n == 8 {
                self.out.push(self.acc as u8);
                self.acc = 0;
                self.n = 0;
            }
        }
    }
    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push((self.acc << (8 - self.n)) as u8);
        }
        self.out
    }
}

/// Canonical codes for given lengths (same assignment as the decoder).
fn canonical(lens: &[u8]) -> Vec<(u32, u8)> {
    let mut count = [0u32; 17];
    for &l in lens {
        if l > 0 {
            count[l as usize] += 1;
        }
    }
    let mut next = [0u32; 17];
    let mut code = 0u32;
    for l in 1..=16 {
        code = (code + count[l - 1]) << 1;
        if l == 1 {
            code = 0;
        }
        next[l] = code;
    }
    lens.iter()
        .map(|&l| {
            if l == 0 {
                (0, 0)
            } else {
                let c = next[l as usize];
                next[l as usize] += 1;
                (c, l)
            }
        })
        .collect()
}

/// Huffman code lengths (limited to `max` bits) for symbol frequencies.
fn code_lengths(freq: &[u32], max: u8) -> Vec<u8> {
    let used: Vec<usize> = (0..freq.len()).filter(|&i| freq[i] > 0).collect();
    let mut lens = vec![0u8; freq.len()];
    if used.len() <= 1 {
        return lens; // caller writes a single-symbol table
    }
    // package-merge would be optimal; a simple approach: build a Huffman tree,
    // then flatten lengths that exceed `max` by rescaling frequencies.
    let mut f: Vec<u64> = freq.iter().map(|&x| x as u64).collect();
    loop {
        // heap of (weight, node)
        let mut nodes: Vec<(u64, Option<usize>, Option<(usize, usize)>)> =
            used.iter().map(|&s| (f[s], Some(s), None)).collect();
        let mut alive: Vec<usize> = (0..nodes.len()).collect();
        while alive.len() > 1 {
            alive.sort_by_key(|&i| std::cmp::Reverse(nodes[i].0));
            let a = alive.pop().unwrap();
            let b = alive.pop().unwrap();
            nodes.push((nodes[a].0 + nodes[b].0, None, Some((a, b))));
            alive.push(nodes.len() - 1);
        }
        let root = alive[0];
        let mut stack = vec![(root, 0u8)];
        let mut too_long = false;
        while let Some((i, d)) = stack.pop() {
            match nodes[i] {
                (_, Some(s), _) => {
                    lens[s] = d.max(1);
                    if d > max {
                        too_long = true;
                    }
                }
                (_, None, Some((a, b))) => {
                    stack.push((a, d + 1));
                    stack.push((b, d + 1));
                }
                _ => {}
            }
        }
        if !too_long {
            return lens;
        }
        for x in f.iter_mut() {
            if *x > 0 {
                *x = (*x).div_ceil(2);
            }
        }
    }
}

/// Compress into a raw `-lh5-` stream that [`decompress`] (and DocuWorks)
/// can read. Uses greedy matching with a small hash chain.
pub fn compress(src: &[u8]) -> Vec<u8> {
    const WINDOW: usize = 1 << 13;
    // tokens: literal byte or (length, distance)
    let mut toks: Vec<(u16, u16)> = Vec::new(); // (c symbol, distance-1) ; dist unused for literals
    let mut head: std::collections::HashMap<[u8; 3], Vec<usize>> = Default::default();
    let mut i = 0;
    while i < src.len() {
        let mut best = (0usize, 0usize);
        if i + 3 <= src.len() {
            let key = [src[i], src[i + 1], src[i + 2]];
            if let Some(list) = head.get(&key) {
                for &j in list.iter().rev().take(64) {
                    if i - j > WINDOW - 1 {
                        break;
                    }
                    let mut l = 0;
                    while l < 256 && i + l < src.len() && src[j + l] == src[i + l] {
                        l += 1;
                    }
                    if l > best.0 {
                        best = (l, i - j);
                        if l == 256 {
                            break;
                        }
                    }
                }
            }
        }
        let step = if best.0 >= 3 {
            toks.push(((best.0 - 3 + 256) as u16, (best.1 - 1) as u16));
            best.0
        } else {
            toks.push((src[i] as u16, 0));
            1
        };
        for k in i..(i + step).min(src.len().saturating_sub(2)) {
            head.entry([src[k], src[k + 1], src[k + 2]]).or_default().push(k);
        }
        i += step;
    }
    let mut w = BitWriter { out: Vec::new(), acc: 0, n: 0 };
    for block in toks.chunks(0xffff) {
        write_block(&mut w, block);
    }
    w.finish()
}

fn pos_code(d: u16) -> (u16, u32, u32) {
    // distance-1 → (position symbol, extra bits value, extra bit count)
    let d = d as u32;
    if d == 0 {
        (0, 0, 0)
    } else {
        let nb = 32 - d.leading_zeros(); // d in [2^(nb-1), 2^nb)
        let sym = nb as u16;
        if sym == 1 {
            (1, 0, 0)
        } else {
            (sym, d - (1 << (nb - 1)), nb - 1)
        }
    }
}

fn write_block(w: &mut BitWriter, toks: &[(u16, u16)]) {
    let mut cf = vec![0u32; NC];
    let mut pf = vec![0u32; NP];
    for &(c, d) in toks {
        cf[c as usize] += 1;
        if c >= 256 {
            pf[pos_code(d).0 as usize] += 1;
        }
    }
    let clen = code_lengths(&cf, 16);
    let plen = code_lengths(&pf, 16);
    let csingle = cf.iter().filter(|&&x| x > 0).count() == 1;
    let psingle = pf.iter().filter(|&&x| x > 0).count() <= 1;
    // run-length coding of the c lengths with the pt alphabet
    let mut tsyms: Vec<(u16, u32, u32)> = Vec::new(); // (symbol, extra, extra bits)
    let n = if csingle { 0 } else { (0..NC).rev().find(|&i| clen[i] > 0).map(|i| i + 1).unwrap_or(0) };
    let mut i = 0;
    while i < n {
        if clen[i] == 0 {
            let mut run = 0;
            while i + run < n && clen[i + run] == 0 {
                run += 1;
            }
            let mut left = run;
            while left > 0 {
                if left <= 2 {
                    tsyms.push((0, 0, 0));
                    left -= 1;
                } else if left <= 18 {
                    tsyms.push((1, (left - 3) as u32, 4));
                    left = 0;
                } else if left == 19 {
                    tsyms.push((0, 0, 0));
                    left -= 1;
                } else {
                    let k = left.min(20 + 511);
                    tsyms.push((2, (k - 20) as u32, CBIT));
                    left -= k;
                }
            }
            i += run;
        } else {
            tsyms.push((clen[i] as u16 + 2, 0, 0));
            i += 1;
        }
    }
    let mut tf = vec![0u32; NT];
    for &(s, _, _) in &tsyms {
        tf[s as usize] += 1;
    }
    let tlen = code_lengths(&tf, 16);
    let tsingle = tf.iter().filter(|&&x| x > 0).count() <= 1;

    w.put(toks.len() as u32, 16);
    // pt table for c lengths
    if csingle || tsingle {
        w.put(0, TBIT);
        let s = tsyms.first().map(|t| t.0).unwrap_or(0);
        w.put(s as u32, TBIT);
    } else {
        write_pt(w, &tlen, TBIT, Some(3));
    }
    // c lengths
    if csingle {
        w.put(0, CBIT);
        let s = cf.iter().position(|&x| x > 0).unwrap_or(0);
        w.put(s as u32, CBIT);
    } else {
        w.put(n as u32, CBIT);
        let tc = canonical(&tlen);
        for &(s, extra, nb) in &tsyms {
            if !tsingle {
                let (code, l) = tc[s as usize];
                w.put(code, l as u32);
            }
            if nb > 0 {
                w.put(extra, nb);
            }
        }
    }
    // p lengths
    if psingle {
        w.put(0, PBIT);
        let s = pf.iter().position(|&x| x > 0).unwrap_or(0);
        w.put(s as u32, PBIT);
    } else {
        write_pt(w, &plen, PBIT, None);
    }
    let cc = canonical(&clen);
    let pc = canonical(&plen);
    for &(c, d) in toks {
        if !csingle {
            let (code, l) = cc[c as usize];
            w.put(code, l as u32);
        }
        if c >= 256 {
            let (ps, extra, nb) = pos_code(d);
            if !psingle {
                let (code, l) = pc[ps as usize];
                w.put(code, l as u32);
            }
            if nb > 0 {
                w.put(extra, nb);
            }
        }
    }
}

fn write_pt(w: &mut BitWriter, lens: &[u8], nbit: u32, special: Option<usize>) {
    let n = (0..lens.len()).rev().find(|&i| lens[i] > 0).map(|i| i + 1).unwrap_or(0);
    let n = match special {
        Some(sp) => n.max(sp),
        None => n,
    };
    w.put(n as u32, nbit);
    let mut i = 0;
    while i < n {
        let c = lens[i] as u32;
        if c < 7 {
            w.put(c, 3);
        } else {
            w.put(7, 3);
            for _ in 7..c {
                w.put(1, 1);
            }
            w.put(0, 1);
        }
        i += 1;
        if Some(i) == special {
            // zeros that follow (up to 3)
            let mut z = 0;
            while z < 3 && i + z < n && lens[i + z] == 0 {
                z += 1;
            }
            w.put(z as u32, 2);
            i += z;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        for i in 0..20000u32 {
            data.extend_from_slice(format!("line {} {}\n", i % 97, i * 7 % 13).as_bytes());
        }
        data.extend((0..5000u32).map(|i| (i.wrapping_mul(2654435761u32) >> 24) as u8));
        let c = compress(&data);
        assert!(c.len() < data.len());
        assert_eq!(decompress(&c, data.len()).unwrap(), data);
        for small in [&b"a"[..], b"ab", b"aaaaaaaaaaaaaaaaaaaaaaaaaaaa", b""] {
            let c = compress(small);
            assert_eq!(decompress(&c, small.len()).unwrap(), small);
        }
    }
}
