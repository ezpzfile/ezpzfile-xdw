//! Windows device-independent bitmaps (DIB): BITMAPINFO + pixel rows.
//!
//! Decodes 1/4/8/16/24/32-bit pictures, RLE4/RLE8, and passes JPEG through.

use crate::gfx::{Image, ImageData};

fn u16le(b: &[u8], i: usize) -> u32 {
    b.get(i..i + 2).map(|x| u16::from_le_bytes([x[0], x[1]]) as u32).unwrap_or(0)
}
fn u32le(b: &[u8], i: usize) -> u32 {
    b.get(i..i + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]])).unwrap_or(0)
}

/// What a BITMAPINFO says.
#[derive(Debug, Clone)]
pub struct Info {
    pub w: u32,
    pub h: u32,
    pub top_down: bool,
    pub bpp: u32,
    pub compression: u32,
    /// Colour table as 0xRRGGBB.
    pub palette: Vec<u32>,
    pub masks: Option<[u32; 3]>,
}

pub fn info(bmi: &[u8]) -> Option<Info> {
    let size = u32le(bmi, 0) as usize;
    if size == 12 {
        let w = u16le(bmi, 4);
        let h = u16le(bmi, 6);
        let bpp = u16le(bmi, 10);
        let n = if bpp <= 8 { 1usize << bpp } else { 0 };
        let palette = (0..n)
            .map(|k| {
                let p = 12 + 3 * k;
                bmi.get(p..p + 3).map(|c| (c[2] as u32) << 16 | (c[1] as u32) << 8 | c[0] as u32).unwrap_or(0)
            })
            .collect();
        return Some(Info { w, h, top_down: false, bpp, compression: 0, palette, masks: None });
    }
    if size < 40 || bmi.len() < 40 {
        return None;
    }
    let w = u32le(bmi, 4) as i32;
    let h = u32le(bmi, 8) as i32;
    let bpp = u16le(bmi, 14);
    let compression = u32le(bmi, 16);
    let used = u32le(bmi, 32) as usize;
    let mut at = size;
    let mut masks = None;
    if compression == 3 || compression == 6 {
        if size >= 52 {
            masks = Some([u32le(bmi, 40), u32le(bmi, 44), u32le(bmi, 48)]);
        } else {
            masks = Some([u32le(bmi, at), u32le(bmi, at + 4), u32le(bmi, at + 8)]);
            at += 12;
        }
    }
    let n = if bpp <= 8 { if used > 0 { used.min(256) } else { 1usize << bpp } } else { used.min(256) };
    let palette = (0..n)
        .map(|k| {
            let p = at + 4 * k;
            bmi.get(p..p + 3).map(|c| (c[2] as u32) << 16 | (c[1] as u32) << 8 | c[0] as u32).unwrap_or(0)
        })
        .collect();
    if w <= 0 || h == 0 {
        return None;
    }
    Some(Info { w: w as u32, h: h.unsigned_abs(), top_down: h < 0, bpp, compression, palette, masks })
}

/// Decode to RGBA (top row first). `usage` 1 (palette indices) is treated
/// as plain colours.
pub fn decode(bmi: &[u8], bits: &[u8]) -> Option<Image> {
    let inf = info(bmi)?;
    let (w, h) = (inf.w as usize, inf.h as usize);
    if w == 0 || h == 0 || w.saturating_mul(h) > 120_000_000 {
        return None;
    }
    match inf.compression {
        4 => return Some(Image { w: inf.w, h: inf.h, data: ImageData::Jpeg(bits.to_vec()) }),
        5 => return None, // PNG inside a DIB: not seen in DocuWorks files
        _ => {}
    }
    let mut out = vec![255u8; w * h * 4];
    let put = |out: &mut Vec<u8>, x: usize, row: usize, c: u32| {
        let y = if inf.top_down { row } else { h - 1 - row };
        let p = (y * w + x) * 4;
        out[p] = (c >> 16) as u8;
        out[p + 1] = (c >> 8) as u8;
        out[p + 2] = c as u8;
        out[p + 3] = 255;
    };
    // colours missing from a short palette are black
    let pal = |i: usize| inf.palette.get(i).copied().unwrap_or(0);
    if inf.compression == 1 || inf.compression == 2 {
        rle(&inf, bits, |x, row, i| {
            if x < w && row < h {
                put(&mut out, x, row, pal(i));
            }
        });
        return Some(Image { w: inf.w, h: inf.h, data: ImageData::Rgba(out) });
    }
    let stride = ((w * inf.bpp as usize + 31) / 32) * 4;
    for row in 0..h {
        let line = match bits.get(row * stride..row * stride + stride) {
            Some(l) => l,
            None => break,
        };
        for x in 0..w {
            let c = match inf.bpp {
                1 => pal(((line[x / 8] >> (7 - x % 8)) & 1) as usize),
                4 => pal(((line[x / 2] >> if x % 2 == 0 { 4 } else { 0 }) & 15) as usize),
                8 => pal(line[x] as usize),
                16 => {
                    let v = u16le(line, x * 2);
                    match inf.masks {
                        Some(m) => from_masks(v, m),
                        None => {
                            let r = (v >> 10) & 31;
                            let g = (v >> 5) & 31;
                            let b = v & 31;
                            (r * 255 / 31) << 16 | (g * 255 / 31) << 8 | b * 255 / 31
                        }
                    }
                }
                24 => (line[x * 3 + 2] as u32) << 16 | (line[x * 3 + 1] as u32) << 8 | line[x * 3] as u32,
                32 => {
                    let v = u32le(line, x * 4);
                    match inf.masks {
                        Some(m) => from_masks(v, m),
                        None => v & 0xffffff,
                    }
                }
                _ => 0,
            };
            put(&mut out, x, row, c);
        }
    }
    Some(Image { w: inf.w, h: inf.h, data: ImageData::Rgba(out) })
}

fn from_masks(v: u32, m: [u32; 3]) -> u32 {
    let ch = |mask: u32| -> u32 {
        if mask == 0 {
            return 0;
        }
        let shift = mask.trailing_zeros();
        let bits = (mask >> shift).count_ones();
        let x = (v & mask) >> shift;
        if bits >= 8 {
            x >> (bits - 8)
        } else {
            x * 255 / ((1 << bits) - 1)
        }
    };
    ch(m[0]) << 16 | ch(m[1]) << 8 | ch(m[2])
}

fn rle(inf: &Info, b: &[u8], mut set: impl FnMut(usize, usize, usize)) {
    let four = inf.compression == 2;
    let (mut x, mut row) = (0usize, 0usize);
    let mut i = 0;
    while i + 1 < b.len() {
        let (n, c) = (b[i] as usize, b[i + 1]);
        i += 2;
        if n > 0 {
            for k in 0..n {
                let idx = if four { if k % 2 == 0 { c >> 4 } else { c & 15 } } else { c };
                set(x, row, idx as usize);
                x += 1;
            }
            continue;
        }
        match c {
            0 => {
                x = 0;
                row += 1;
            }
            1 => break,
            2 => {
                if i + 1 >= b.len() {
                    break;
                }
                x += b[i] as usize;
                row += b[i + 1] as usize;
                i += 2;
            }
            m => {
                let m = m as usize;
                let bytes = if four { (m + 1) / 2 } else { m };
                for k in 0..m {
                    let idx = if four {
                        let byte = b.get(i + k / 2).copied().unwrap_or(0);
                        if k % 2 == 0 { byte >> 4 } else { byte & 15 }
                    } else {
                        b.get(i + k).copied().unwrap_or(0)
                    };
                    set(x, row, idx as usize);
                    x += 1;
                }
                i += bytes + (bytes & 1);
            }
        }
    }
}

/// Crop `img` to the source rectangle given in DIB terms (y measured from
/// the bottom for bottom-up pictures is already handled by the caller).
pub fn crop(img: &Image, x: u32, y: u32, w: u32, h: u32) -> Option<Image> {
    let ImageData::Rgba(px) = &img.data else { return None };
    if x == 0 && y == 0 && w == img.w && h == img.h {
        return None;
    }
    let w = w.min(img.w.saturating_sub(x));
    let h = h.min(img.h.saturating_sub(y));
    if w == 0 || h == 0 {
        return None;
    }
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for r in y..y + h {
        let s = ((r * img.w + x) * 4) as usize;
        out.extend_from_slice(&px[s..s + (w * 4) as usize]);
    }
    Some(Image { w, h, data: ImageData::Rgba(out) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_bit_bottom_up() {
        // 2x2, 1 bpp, palette black/white; bottom row first
        let mut bmi = vec![0u8; 40 + 8];
        bmi[0] = 40;
        bmi[4] = 2;
        bmi[8] = 2;
        bmi[12] = 1;
        bmi[14] = 1;
        bmi[44] = 255;
        bmi[45] = 255;
        bmi[46] = 255;
        let bits = [0b1000_0000, 0, 0, 0, 0b0100_0000, 0, 0, 0];
        let img = decode(&bmi, &bits).unwrap();
        let ImageData::Rgba(p) = img.data else { panic!() };
        // top row = second stored row: black, white
        assert_eq!(&p[0..3], &[0, 0, 0]);
        assert_eq!(&p[4..7], &[255, 255, 255]);
        // bottom row = first stored row: white, black
        assert_eq!(&p[8..11], &[255, 255, 255]);
        assert_eq!(&p[12..15], &[0, 0, 0]);
    }
}
