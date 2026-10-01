//! Windows metafiles (WMF): older DocuWorks pages (entry kind 1) and some
//! annotation drawings. Drawn with the same machinery as EMF; the window
//! (SETWINDOWORG / SETWINDOWEXT) is fitted to the destination box.

use crate::dib;
use crate::emf::{rgb, Brush, Font, Obj, Pen, Pictures, R};
use crate::gfx::{Display, Path, Xf};

fn u16le(b: &[u8], i: usize) -> u16 {
    b.get(i..i + 2).map(|x| u16::from_le_bytes([x[0], x[1]])).unwrap_or(0)
}
fn i16le(b: &[u8], i: usize) -> i16 {
    u16le(b, i) as i16
}
fn u32le(b: &[u8], i: usize) -> u32 {
    b.get(i..i + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]])).unwrap_or(0)
}

pub fn is_wmf(b: &[u8]) -> bool {
    b.len() >= 18 && ((u16le(b, 0) == 1 || u16le(b, 0) == 2) && u16le(b, 2) == 9 || u32le(b, 0) == 0x9AC6_CDD7)
}

/// Draw the WMF `b` into `out`, fitted to `dst` = [x, y, w, h].
pub fn render(b: &[u8], dst: [f64; 4], pics: Pictures, out: &mut Display) {
    let mut start = 0;
    let mut placeable: Option<[f64; 4]> = None;
    if u32le(b, 0) == 0x9AC6_CDD7 {
        placeable = Some([i16le(b, 6) as f64, i16le(b, 8) as f64, i16le(b, 10) as f64, i16le(b, 12) as f64]);
        start = 22;
    }
    if b.len() < start + 18 {
        out.skipped.push("not a WMF".into());
        return;
    }
    let hdr_words = u16le(b, start + 2) as usize;
    let mut i = start + hdr_words * 2;
    // first pass: the window, so drawing before SETWINDOWEXT still lands
    let mut win: Option<([f64; 2], [f64; 2])> = placeable.map(|p| ([p[0], p[1]], [p[2] - p[0], p[3] - p[1]]));
    {
        let mut j = i;
        let mut org = [0.0, 0.0];
        while j + 6 <= b.len() {
            let n = u32le(b, j) as usize;
            let f = u16le(b, j + 4);
            if n < 3 || j + 2 * n > b.len() {
                break;
            }
            match f {
                0x020B => org = [i16le(b, j + 8) as f64, i16le(b, j + 6) as f64],
                0x020C => {
                    let ext = [i16le(b, j + 8) as f64, i16le(b, j + 6) as f64];
                    if win.is_none() {
                        win = Some((org, ext));
                    }
                }
                0 => break,
                _ => {}
            }
            j += 2 * n;
        }
    }
    let base_clip = vec![Path::rect(dst[0] as f32, dst[1] as f32, (dst[0] + dst[2]) as f32, (dst[1] + dst[3]) as f32)];
    // logical units: anisotropic, viewport = the destination box
    let dev = Xf([1.0, 0.0, 0.0, 1.0, dst[0], dst[1]]);
    let ext = win.map(|w| w.1).unwrap_or([dst[2], dst[3]]);
    let px = (dst[2] / ext[0].abs().max(1.0)).abs().max(1e-3);
    let mut r = R::new(out, pics, dev, px, (1.0, 1.0), base_clip);
    r.dc.map_mode = 8;
    r.dc.vorg = (0.0, 0.0);
    r.dc.vext = (dst[2], dst[3]);
    if let Some((o, e)) = win {
        r.dc.worg = (o[0], o[1]);
        r.dc.wext = (e[0], e[1]);
    }
    let mut slots: Vec<bool> = Vec::new();
    while i + 6 <= b.len() {
        let n = u32le(b, i) as usize;
        let f = u16le(b, i + 4);
        if n < 3 || i + 2 * n > b.len() || f == 0 {
            break;
        }
        let rec = &b[i + 6..i + 2 * n];
        record(&mut r, f, rec, &mut slots);
        i += 2 * n;
    }
}

fn new_slot(slots: &mut Vec<bool>) -> u32 {
    if let Some(k) = slots.iter().position(|&u| !u) {
        slots[k] = true;
        k as u32
    } else {
        slots.push(true);
        (slots.len() - 1) as u32
    }
}

fn pts(rec: &[u8], at: usize, n: usize) -> Vec<(f64, f64)> {
    (0..n).map(|k| (i16le(rec, at + 4 * k) as f64, i16le(rec, at + 4 * k + 2) as f64)).collect()
}

fn record(r: &mut R, f: u16, p: &[u8], slots: &mut Vec<bool>) {
    let s = |k: usize| i16le(p, 2 * k) as f64;
    match f {
        0x020B => r.dc.worg = (s(1), s(0)),
        0x020C => r.dc.wext = (s(1), s(0)),
        0x020D | 0x020E | 0x0211 | 0x0412 | 0x0410 => {} // viewport: the box decides
        0x0103 => {}
        0x0102 => r.dc.bk_mode = u16le(p, 0) as u32,
        0x0201 => r.dc.bk_color = u32le(p, 0),
        0x0209 => r.dc.text_color = u32le(p, 0),
        0x012E => r.dc.text_align = u16le(p, 0) as u32,
        0x0106 => r.dc.fill_mode = u16le(p, 0) as u32,
        0x001E => r.saved.push(r.dc.clone()),
        0x0127 => {
            let n = i16le(p, 0);
            let keep = if n < 0 { r.saved.len().saturating_sub((-n) as usize) } else { (n as usize).saturating_sub(1) };
            if keep < r.saved.len() {
                r.saved.truncate(keep + 1);
                if let Some(d) = r.saved.pop() {
                    r.dc = d;
                    r.last_clip = None;
                }
            }
        }
        0x0214 => r.dc.cur = (s(1), s(0)),
        0x0213 => {
            let to = (s(1), s(0));
            let path = r.poly_path(&[r.dc.cur, to], false);
            r.stroke(path);
            r.dc.cur = to;
        }
        0x0325 | 0x0324 => {
            let n = u16le(p, 0) as usize;
            let v = pts(p, 2, n);
            r.draw_poly(&v, f == 0x0324);
        }
        0x0538 => {
            let polys = u16le(p, 0) as usize;
            let counts: Vec<usize> = (0..polys).map(|k| u16le(p, 2 + 2 * k) as usize).collect();
            let mut at = 2 + 2 * polys;
            let mut segs = Vec::new();
            for c in counts {
                let v = pts(p, at, c);
                segs.extend(r.poly_path(&v, true).0);
                at += 4 * c;
            }
            r.shape(segs, true);
        }
        0x041B => {
            let rect = [s(3), s(2), s(1), s(0)];
            let path = r.poly_path(&R::rect_points(rect), true);
            r.shape(path.0, true);
        }
        0x0418 => {
            let rect = [s(3), s(2), s(1), s(0)];
            let segs = r.ellipse_segs(rect);
            r.shape(segs, true);
        }
        0x061C => {
            // ROUNDRECT h w b r t l
            let rect = [s(5), s(4), s(3), s(2)];
            let segs = r.ellipse_segs(rect);
            let _ = segs;
            let path = r.poly_path(&R::rect_points(rect), true);
            r.shape(path.0, true);
        }
        0x0817 | 0x081A | 0x0830 => {
            // ARC / PIE / CHORD: yend xend ystart xstart b r t l
            let rect = [s(7), s(6), s(5), s(4)];
            let (cx, cy, rx, ry, a0, sweep) = r.arc_angles(rect, (s(3), s(2)), (s(1), s(0)));
            let v = R::arc_pts(cx, cy, rx, ry, a0, sweep);
            let mut segs = Vec::new();
            if f == 0x081A {
                let (x, y) = r.pt(cx, cy);
                segs.push(crate::gfx::Seg::M(x, y));
                let (a, b2) = r.pt(v[0].0, v[0].1);
                segs.push(crate::gfx::Seg::L(a, b2));
                segs.extend(r.bezier_path(None, &v[1..]));
                segs.push(crate::gfx::Seg::Z);
            } else {
                segs.extend(r.bezier_path(Some(v[0]), &v[1..]));
                if f == 0x0830 {
                    segs.push(crate::gfx::Seg::Z);
                }
            }
            r.shape(segs, f != 0x0817);
        }
        0x02FA => {
            let h = new_slot(slots);
            let style = u16le(p, 0) as u32;
            let width = i16le(p, 2) as f64;
            let color = u32le(p, 6);
            r.objs.insert(h, Obj::Pen(Pen { style, width, color, geometric: width > 1.0, dash: vec![] }));
        }
        0x02FC => {
            let h = new_slot(slots);
            r.objs.insert(h, Obj::Brush(Brush { style: u16le(p, 0) as u32, color: u32le(p, 2) }));
        }
        0x0142 | 0x01F9 => {
            let h = new_slot(slots);
            r.objs.insert(h, Obj::Brush(Brush { style: 0, color: 0x808080 }));
        }
        0x02FB => {
            let h = new_slot(slots);
            let face_raw: Vec<u8> = p.get(18..50).unwrap_or(&[]).iter().copied().take_while(|&c| c != 0).collect();
            r.objs.insert(
                h,
                Obj::Font(Font {
                    height: i16le(p, 0) as i32,
                    width: i16le(p, 2) as i32,
                    escapement: i16le(p, 4) as i32,
                    weight: i16le(p, 8) as i32,
                    italic: p.get(10).copied().unwrap_or(0) != 0,
                    underline: p.get(11).copied().unwrap_or(0) != 0,
                    strike: p.get(12).copied().unwrap_or(0) != 0,
                    charset: p.get(13).copied().unwrap_or(0),
                    face: crate::sjis::decode(&face_raw),
                }),
            );
        }
        0x00F7 | 0x06FF => {
            let h = new_slot(slots);
            r.objs.insert(h, Obj::Other);
        }
        0x012D => r.select(u16le(p, 0) as u32),
        0x01F0 => {
            let h = u16le(p, 0) as usize;
            if h < slots.len() {
                slots[h] = false;
            }
            r.objs.remove(&(h as u32));
        }
        0x0416 => {
            let rect = [s(3), s(2), s(1), s(0)];
            let path = r.poly_path(&R::rect_points(rect), true);
            let mut c = r.dc.clip.clone();
            c.push(path);
            r.set_clip(c);
        }
        0x0415 => {}
        0x012C => r.set_clip(Vec::new()),
        0x0521 => {
            // TEXTOUT: count, string (padded to even), y, x
            let n = u16le(p, 0) as usize;
            let str_words = n.div_ceil(2);
            let raw = p.get(2..2 + n).unwrap_or(&[]);
            let y = i16le(p, 2 + 2 * str_words) as f64;
            let x = i16le(p, 4 + 2 * str_words) as f64;
            let (text, _) = crate::emf::sjis_chars(raw);
            r.draw_text(x, y, text, None, 0, [0.0; 4]);
        }
        0x0A32 => {
            // EXTTEXTOUT: y, x, count, options, [rect], string, [dx]
            let y = s(0);
            let x = s(1);
            let n = u16le(p, 4) as usize;
            let opts = u16le(p, 6) as u32;
            let mut at = 8;
            let mut rcl = [0.0; 4];
            if opts & 0x6 != 0 {
                rcl = [i16le(p, 8) as f64, i16le(p, 10) as f64, i16le(p, 12) as f64, i16le(p, 14) as f64];
                at += 8;
            }
            let raw = p.get(at..at + n).unwrap_or(&[]);
            let (text, per_char) = crate::emf::sjis_chars(raw);
            let dx_at = at + n + (n & 1);
            let dx: Vec<f64> = if p.len() >= dx_at + 2 * n { (0..n).map(|k| i16le(p, dx_at + 2 * k) as f64).collect() } else { Vec::new() };
            let adv = crate::emf::group_advances(&dx, &per_char);
            r.draw_text(x, y, text, adv, opts, rcl);
        }
        0x061D => {
            // PATBLT: rop, h, w, y, x
            let rop = u32le(p, 0);
            let dst = [s(5), s(4), s(3), s(2)];
            r.pattern_rect(dst, rop);
        }
        0x0B41 | 0x0F43 | 0x0940 | 0x0D33 => blit(r, f, p),
        0x0626 => {}
        0x0104 | 0x0107 | 0x0109 | 0x0231 | 0x0234 | 0x0035 | 0x0037 | 0x0139 | 0x0436 | 0x0220 | 0x0228 => {}
        _ => r.out.skipped.push(format!("WMF record {f:#06x}")),
    }
}

fn blit(r: &mut R, f: u16, p: &[u8]) {
    let s = |k: usize| i16le(p, 2 * k) as f64;
    let rop = u32le(p, 0);
    // parameter layouts (after the 4-byte raster operation)
    let (src, dst, dib_at) = match f {
        0x0B41 => ([s(5), s(4), s(3), s(2)], [s(9), s(8), s(7), s(6)], 20),
        0x0F43 => ([s(6), s(5), s(4), s(3)], [s(10), s(9), s(8), s(7)], 22),
        0x0940 => {
            // DIBBITBLT: rop, ySrc, xSrc, h, w, yDst, xDst, DIB
            let (ys, xs, h, w, yd, xd) = (s(2), s(3), s(4), s(5), s(6), s(7));
            ([xs, ys, w, h], [xd, yd, w, h], 16)
        }
        _ => {
            // SETDIBTODEV: usage, scans, start, ySrc, xSrc, h, w, yDst, xDst, DIB
            let u = |k: usize| i16le(p, 2 * k) as f64;
            ([u(4), u(3), u(6), u(5)], [u(8), u(7), u(6), u(5)], 18)
        }
    };
    let dibd = p.get(dib_at..).unwrap_or(&[]);
    let Some(inf) = dib::info(dibd) else {
        r.pattern_rect(dst, rop);
        return;
    };
    let size = u32le(dibd, 0) as usize;
    let pal = inf.palette.len() * 4 + if inf.masks.is_some() && size == 40 { 12 } else { 0 };
    let bits = dibd.get(size + pal..).unwrap_or(&[]);
    if let Some(img) = dib::decode(dibd, bits) {
        let src_i = [src[0] as i64, src[1] as i64, src[2] as i64, src[3] as i64];
        r.blit(img, dst, src_i, if f == 0x0D33 { 0xCC0020 } else { rop }, !inf.top_down);
    }
    let _ = rgb;
}
