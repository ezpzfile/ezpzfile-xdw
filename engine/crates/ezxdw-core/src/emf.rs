//! Enhanced metafiles (EMF): the pictures DocuWorks pages are made of.
//!
//! Besides ordinary GDI records, the DocuWorks printer driver writes most
//! shapes and pictures as its own comment records (`GDICOMMENT` starting
//! with "DW"). What each does was worked out by drawing test pages and
//! opening them in DocuWorks Viewer Light:
//!
//! ```text
//! DW02            begin a path
//! DW <op> <enc>   points: u32 count, first point as two i16, the rest as
//!                 enc 0x20: two i16 each, 0x40: two i8 steps, 0x80: one
//!                 byte, high nibble x step, low nibble y step (signed)
//!   op 0x01 bit   the first point starts a new figure
//!   op 0x02 bit   close the figure after these points
//!   op 0x10 bit   the points (after a starting point) are Bézier triples
//!   op 0x20       a polygon on its own, filled with the brush
//!   op 0x40       a polyline on its own, drawn with the pen
//!                 (in the nibble form, a nibble of 8 means the step is in
//!                 the next byte, as an i8: x's byte comes before y's)
//! DW03            clip to the path     DW04  fill the path (brush)
//! DW05            stroke the path (pen)
//! DW06 l t r b    clip to a rectangle (replaces the clip)
//! DWa             a picture (a DIB stored in the record)
//! DWb             the next picture from the page's picture list
//! DWc             draw the current picture (like STRETCHDIBITS)
//! ```
//!
//! Everything is turned into a `gfx::Display`.

use crate::dib;
use crate::gfx::{Clip, Display, Image, ImageData, Item, Path, Seg, Xf};
use std::collections::HashMap;

fn i32le(b: &[u8], i: usize) -> i32 {
    b.get(i..i + 4).map(|x| i32::from_le_bytes([x[0], x[1], x[2], x[3]])).unwrap_or(0)
}
fn u32le(b: &[u8], i: usize) -> u32 {
    i32le(b, i) as u32
}
fn i16le(b: &[u8], i: usize) -> i16 {
    b.get(i..i + 2).map(|x| i16::from_le_bytes([x[0], x[1]])).unwrap_or(0)
}
fn f32le(b: &[u8], i: usize) -> f32 {
    f32::from_bits(u32le(b, i))
}

/// COLORREF (0x00BBGGRR) → 0xRRGGBB.
pub(crate) fn rgb(c: u32) -> u32 {
    (c & 0xff) << 16 | (c & 0xff00) | (c >> 16) & 0xff
}

#[derive(Debug, Clone)]
pub(crate) struct Pen {
    pub(crate) style: u32,
    /// Width in logical units (0 = one device pixel).
    pub(crate) width: f64,
    pub(crate) color: u32,
    pub(crate) geometric: bool,
    pub(crate) dash: Vec<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct Brush {
    /// 0 solid, 1 null, 2 hatched, 3 pattern …
    pub(crate) style: u32,
    pub(crate) color: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct Font {
    pub(crate) height: i32,
    pub(crate) width: i32,
    pub(crate) escapement: i32,
    pub(crate) weight: i32,
    pub(crate) italic: bool,
    pub(crate) underline: bool,
    pub(crate) strike: bool,
    pub(crate) charset: u8,
    pub(crate) face: String,
}

#[derive(Debug, Clone)]
pub(crate) enum Obj {
    Pen(Pen),
    Brush(Brush),
    Font(Font),
    /// A WMF region (bounding rectangle, logical units): selecting it sets
    /// the clip.
    Region([f64; 4]),
    Other,
}

#[derive(Debug, Clone)]
pub(crate) struct Dc {
    pub(crate) map_mode: u32,
    pub(crate) worg: (f64, f64),
    pub(crate) wext: (f64, f64),
    pub(crate) vorg: (f64, f64),
    pub(crate) vext: (f64, f64),
    pub(crate) world: Xf,
    pub(crate) pen: Pen,
    pub(crate) brush: Brush,
    pub(crate) font: Font,
    pub(crate) text_color: u32,
    pub(crate) bk_color: u32,
    pub(crate) bk_mode: u32,
    pub(crate) text_align: u32,
    pub(crate) fill_mode: u32,
    pub(crate) cur: (f64, f64),
    pub(crate) clip: Vec<Path>,
    pub(crate) arc_ccw: bool,
    pub(crate) rop2: u32,
}

pub(crate) fn stock(i: u32) -> Obj {
    let pen = |c: u32, style: u32| Obj::Pen(Pen { style, width: 0.0, color: c, geometric: false, dash: vec![] });
    let brush = |c: u32, style: u32| Obj::Brush(Brush { style, color: c });
    match i & 0x7fff_ffff {
        0 => brush(0xffffff, 0),
        1 => brush(0xc0c0c0, 0),
        2 => brush(0x808080, 0),
        3 => brush(0x404040, 0),
        4 => brush(0, 0),
        5 => brush(0, 1),
        6 => pen(0xffffff, 0),
        7 => pen(0, 0),
        8 => pen(0, 5),
        18 => brush(0xffffff, 0),
        19 => pen(0, 0),
        10..=17 => Obj::Font(default_font()),
        _ => Obj::Other,
    }
}

pub(crate) fn default_font() -> Font {
    Font {
        height: 12,
        width: 0,
        escapement: 0,
        weight: 400,
        italic: false,
        underline: false,
        strike: false,
        charset: 128,
        face: String::new(),
    }
}

/// The external pictures a page's drawing may use (DWb), in order.
pub type Pictures<'a> = &'a [Vec<u8>];

pub(crate) struct R<'a> {
    pub(crate) out: &'a mut Display,
    pub(crate) pics: Pictures<'a>,
    pub(crate) next_pic: usize,
    /// device → page
    pub(crate) dev: Xf,
    /// one device pixel in page units
    pub(crate) px: f64,
    /// device pixels per millimetre
    pub(crate) dpmm: (f64, f64),
    pub(crate) dc: Dc,
    pub(crate) saved: Vec<Dc>,
    pub(crate) objs: HashMap<u32, Obj>,
    /// GDI path bracket
    pub(crate) in_path: bool,
    pub(crate) path: Vec<Seg>,
    /// DocuWorks path (DW02 …)
    pub(crate) dw_path: Vec<Seg>,
    pub(crate) dw_image: Option<(Image, bool)>,
    pub(crate) clip_cache: HashMap<Vec<u8>, u32>,
    pub(crate) last_clip: Option<(usize, u32)>,
    pub(crate) base_clip: Vec<Path>,
    /// The window is fitted to the box: ignore map mode and viewport records.
    pub(crate) fitted: bool,
}

/// Draw the EMF `b` into `out`, its frame filling `dst` = [x, y, w, h]
/// (page units). `pics` are the pictures for DWb records.
pub fn render(b: &[u8], dst: [f64; 4], pics: Pictures, out: &mut Display) {
    render_opts(b, dst, pics, out, false)
}

/// The window set by the first SETWINDOWORGEX / SETWINDOWEXTEX records.
fn first_window(b: &[u8]) -> Option<((f64, f64), (f64, f64))> {
    let (mut org, mut ext) = ((0.0, 0.0), None);
    let mut i = 0usize;
    while i + 8 <= b.len() {
        let t = u32le(b, i);
        let n = u32le(b, i + 4) as usize;
        if n < 8 || i + n > b.len() {
            break;
        }
        match t {
            9 if ext.is_none() => ext = Some((i32le(b, i + 8) as f64, i32le(b, i + 12) as f64)),
            10 => org = (i32le(b, i + 8) as f64, i32le(b, i + 12) as f64),
            84 | 83 | 86 | 87 | 70 | 81 | 76 => {
                if ext.is_some() {
                    break;
                }
            }
            14 => break,
            _ => {}
        }
        i += n;
    }
    ext.filter(|e| e.0 != 0.0 && e.1 != 0.0).map(|e| (org, e))
}

/// Like `render`. With `fit_window`, the window the drawing sets up is
/// fitted to `dst` instead of the frame: annotation drawings are made
/// that way (their frame is not their size).
pub fn render_opts(b: &[u8], dst: [f64; 4], pics: Pictures, out: &mut Display, fit_window: bool) {
    if b.len() < 88 || u32le(b, 0) != 1 || &b[40..44] != b" EMF" {
        out.skipped.push("not an EMF".into());
        return;
    }
    let frame = [i32le(b, 24) as f64, i32le(b, 28) as f64, i32le(b, 32) as f64, i32le(b, 36) as f64];
    let dev_px = (i32le(b, 72).max(1) as f64, i32le(b, 76).max(1) as f64);
    let mm = (i32le(b, 80).max(1) as f64, i32le(b, 84).max(1) as f64);
    // device pixel → 0.01 mm
    let sx = mm.0 * 100.0 / dev_px.0;
    let sy = mm.1 * 100.0 / dev_px.1;
    let fw = (frame[2] - frame[0]).max(1.0);
    let fh = (frame[3] - frame[1]).max(1.0);
    let kx = dst[2] / fw;
    let ky = dst[3] / fh;
    let dev = Xf([sx * kx, 0.0, 0.0, sy * ky, dst[0] - frame[0] * kx, dst[1] - frame[1] * ky]);
    let px = (sx * kx).abs().max((sy * ky).abs());
    let base_clip = vec![Path::rect(dst[0] as f32, dst[1] as f32, (dst[0] + dst[2]) as f32, (dst[1] + dst[3]) as f32)];
    let win = if fit_window { first_window(b) } else { None };
    let (dev, px) = match win {
        Some((_, e)) => (Xf([1.0, 0.0, 0.0, 1.0, dst[0], dst[1]]), (dst[2] / e.0.abs()).abs().max(1e-3)),
        None => (dev, px),
    };
    let mut r = R::new(out, pics, dev, px, (dev_px.0 / mm.0, dev_px.1 / mm.1), base_clip);
    if let Some((o, e)) = win {
        r.fitted = true;
        r.dc.map_mode = 8;
        r.dc.worg = o;
        r.dc.wext = e;
        r.dc.vext = (dst[2], dst[3]);
    }
    let mut i = 0usize;
    while i + 8 <= b.len() {
        let t = u32le(b, i);
        let n = u32le(b, i + 4) as usize;
        if n < 8 || i + n > b.len() {
            break;
        }
        let rec = &b[i..i + n];
        r.record(t, rec);
        if t == 14 {
            break;
        }
        i += n;
    }
}

impl<'a> R<'a> {
    pub(crate) fn new(out: &'a mut Display, pics: Pictures<'a>, dev: Xf, px: f64, dpmm: (f64, f64), base_clip: Vec<Path>) -> R<'a> {
        let dc = Dc {
            map_mode: 1,
            worg: (0.0, 0.0),
            wext: (1.0, 1.0),
            vorg: (0.0, 0.0),
            vext: (1.0, 1.0),
            world: Xf::ID,
            pen: Pen { style: 0, width: 0.0, color: 0, geometric: false, dash: vec![] },
            brush: Brush { style: 0, color: 0xffffff },
            font: default_font(),
            text_color: 0,
            bk_color: 0xffffff,
            bk_mode: 2,
            text_align: 0,
            fill_mode: 1,
            cur: (0.0, 0.0),
            clip: Vec::new(),
            arc_ccw: true,
            rop2: 13,
        };
        R {
            out,
            pics,
            next_pic: 0,
            dev,
            px,
            dpmm,
            dc,
            saved: Vec::new(),
            objs: HashMap::new(),
            in_path: false,
            path: Vec::new(),
            dw_path: Vec::new(),
            dw_image: None,
            clip_cache: HashMap::new(),
            last_clip: None,
            base_clip,
            fitted: false,
        }
    }

    /// logical → device
    pub(crate) fn l2d(&self) -> Xf {
        let d = &self.dc;
        let (sx, sy) = match d.map_mode {
            2 => (self.dpmm.0 * 0.1, -self.dpmm.1 * 0.1),
            3 => (self.dpmm.0 * 0.01, -self.dpmm.1 * 0.01),
            4 => (self.dpmm.0 * 0.254, -self.dpmm.1 * 0.254),
            5 => (self.dpmm.0 * 0.0254, -self.dpmm.1 * 0.0254),
            6 => (self.dpmm.0 * 25.4 / 1440.0, -self.dpmm.1 * 25.4 / 1440.0),
            7 | 8 => {
                let mut sx = if d.wext.0 != 0.0 { d.vext.0 / d.wext.0 } else { 1.0 };
                let mut sy = if d.wext.1 != 0.0 { d.vext.1 / d.wext.1 } else { 1.0 };
                if d.map_mode == 7 {
                    let m = sx.abs().min(sy.abs());
                    sx = m * sx.signum();
                    sy = m * sy.signum();
                }
                (sx, sy)
            }
            _ => (1.0, 1.0),
        };
        if d.map_mode == 1 {
            Xf([1.0, 0.0, 0.0, 1.0, d.vorg.0 - d.worg.0, d.vorg.1 - d.worg.1])
        } else {
            Xf([sx, 0.0, 0.0, sy, d.vorg.0 - d.worg.0 * sx, d.vorg.1 - d.worg.1 * sy])
        }
    }
    /// logical (world) → page
    pub(crate) fn xf(&self) -> Xf {
        self.dev.after(&self.l2d().after(&self.dc.world))
    }
    pub(crate) fn pt(&self, x: f64, y: f64) -> (f32, f32) {
        let (a, b) = self.xf().apply(x, y);
        (a as f32, b as f32)
    }

    pub(crate) fn clip_index(&mut self) -> u32 {
        // cheap cache keyed by the clip's identity in this state
        let key = self.dc.clip.as_ptr() as usize ^ self.dc.clip.len();
        if let Some((k, v)) = self.last_clip {
            if k == key {
                return v;
            }
        }
        let paths: Vec<Path> = self.base_clip.iter().chain(self.dc.clip.iter()).cloned().collect();
        let sig = format!("{paths:?}").into_bytes();
        let idx = if let Some(&v) = self.clip_cache.get(&sig) {
            v
        } else {
            self.out.clips.push(Clip(paths));
            let v = (self.out.clips.len() - 1) as u32;
            self.clip_cache.insert(sig, v);
            v
        };
        self.last_clip = Some((key, idx));
        idx
    }
    pub(crate) fn set_clip(&mut self, c: Vec<Path>) {
        self.dc.clip = c;
        self.last_clip = None;
    }

    pub(crate) fn fill(&mut self, path: Path) {
        if self.dc.brush.style == 1 || path.is_empty() {
            return;
        }
        let clip = self.clip_index();
        let evenodd = self.dc.fill_mode == 1;
        let mul = self.dc.rop2 == 9;
        self.out.items.push(Item::Fill { path, color: rgb(self.dc.brush.color), evenodd, clip, mul });
    }
    pub(crate) fn stroke(&mut self, path: Path) {
        let p = self.dc.pen.clone();
        if p.style & 0xf == 5 || path.is_empty() {
            return;
        }
        let width = if p.width <= 0.0 || !p.geometric && p.width <= 1.0 {
            self.px
        } else {
            p.width * self.xf().scale()
        };
        let width = width.max(self.px * 0.5) as f32;
        let unit = width.max(self.px as f32);
        let dash: Vec<f32> = if !p.dash.is_empty() {
            p.dash.iter().map(|d| (*d * self.xf().scale()) as f32).collect()
        } else {
            let pat: &[f32] = match p.style & 0xf {
                1 => &[6.0, 2.0],
                2 => &[1.0, 1.0],
                3 => &[6.0, 2.0, 1.0, 2.0],
                4 => &[6.0, 2.0, 1.0, 2.0, 1.0, 2.0],
                _ => &[],
            };
            pat.iter().map(|d| d * unit * 3.0).collect()
        };
        let cap = match (p.style >> 8) & 0xf {
            1 => 1,
            2 => 2,
            _ => 0,
        };
        let join = match (p.style >> 12) & 0xf {
            1 => 1,
            2 => 2,
            _ => 0,
        };
        let color = rgb(p.color);
        let clip = self.clip_index();
        self.out.items.push(Item::Stroke { path, color, width, dash, cap, join, clip });
    }

    /// In a path bracket, figures start at the current position.
    pub(crate) fn path_start(&mut self) {
        if !matches!(self.path.last(), Some(_)) || matches!(self.path.last(), Some(Seg::Z)) {
            let (a, b) = self.pt(self.dc.cur.0, self.dc.cur.1);
            self.path.push(Seg::M(a, b));
        }
    }

    pub(crate) fn poly_path(&self, pts: &[(f64, f64)], close: bool) -> Path {
        let mut v = Vec::with_capacity(pts.len() + 1);
        for (k, &(x, y)) in pts.iter().enumerate() {
            let (a, b) = self.pt(x, y);
            v.push(if k == 0 { Seg::M(a, b) } else { Seg::L(a, b) });
        }
        if close && !v.is_empty() {
            v.push(Seg::Z);
        }
        Path(v)
    }

    /// polyline / polygon (outside or inside a path bracket)
    pub(crate) fn draw_poly(&mut self, pts: &[(f64, f64)], polygon: bool) {
        if pts.is_empty() {
            return;
        }
        let p = self.poly_path(pts, polygon);
        if self.in_path {
            self.path.extend(p.0);
            return;
        }
        if polygon {
            self.fill(p.clone());
        }
        self.stroke(p);
        if let Some(&last) = pts.last() {
            self.dc.cur = last;
        }
    }

    pub(crate) fn bezier_path(&self, start: Option<(f64, f64)>, pts: &[(f64, f64)]) -> Vec<Seg> {
        let mut v = Vec::new();
        if let Some((x, y)) = start {
            let (a, b) = self.pt(x, y);
            v.push(Seg::M(a, b));
        }
        for c in pts.chunks_exact(3) {
            let (a, b) = self.pt(c[0].0, c[0].1);
            let (cc, d) = self.pt(c[1].0, c[1].1);
            let (e, f) = self.pt(c[2].0, c[2].1);
            v.push(Seg::C(a, b, cc, d, e, f));
        }
        v
    }

    /// An ellipse arc as Bézier pieces (logical coordinates), from angle
    /// a0 sweeping `sweep` radians (positive = counter-clockwise on screen).
    pub(crate) fn arc_pts(cx: f64, cy: f64, rx: f64, ry: f64, a0: f64, sweep: f64) -> Vec<(f64, f64)> {
        let n = ((sweep.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).max(1);
        let step = sweep / n as f64;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let mut out = vec![(cx + rx * a0.cos(), cy - ry * a0.sin())];
        let mut a = a0;
        for _ in 0..n {
            let b = a + step;
            let (c0, s0, c1, s1) = (a.cos(), a.sin(), b.cos(), b.sin());
            out.push((cx + rx * (c0 - k * s0), cy - ry * (s0 + k * c0)));
            out.push((cx + rx * (c1 + k * s1), cy - ry * (s1 - k * c1)));
            out.push((cx + rx * c1, cy - ry * s1));
            a = b;
        }
        out
    }

    pub(crate) fn arc_angles(&self, rect: [f64; 4], start: (f64, f64), end: (f64, f64)) -> (f64, f64, f64, f64, f64, f64) {
        let cx = (rect[0] + rect[2]) / 2.0;
        let cy = (rect[1] + rect[3]) / 2.0;
        let rx = (rect[2] - rect[0]).abs() / 2.0;
        let ry = (rect[3] - rect[1]).abs() / 2.0;
        let ang = |p: (f64, f64)| (-(p.1 - cy) / ry.max(1e-9)).atan2((p.0 - cx) / rx.max(1e-9));
        let a0 = ang(start);
        let a1 = ang(end);
        let mut sweep = a1 - a0;
        if self.dc.arc_ccw {
            if sweep <= 0.0 {
                sweep += std::f64::consts::TAU;
            }
        } else if sweep >= 0.0 {
            sweep -= std::f64::consts::TAU;
        }
        (cx, cy, rx, ry, a0, sweep)
    }

    pub(crate) fn shape(&mut self, segs: Vec<Seg>, closed: bool) {
        if self.in_path {
            self.path.extend(segs);
            return;
        }
        let p = Path(segs);
        if closed {
            self.fill(p.clone());
        }
        self.stroke(p);
    }

    pub(crate) fn rect_points(r: [f64; 4]) -> Vec<(f64, f64)> {
        vec![(r[0], r[1]), (r[2], r[1]), (r[2], r[3]), (r[0], r[3])]
    }

    pub(crate) fn ellipse_segs(&self, r: [f64; 4]) -> Vec<Seg> {
        let cx = (r[0] + r[2]) / 2.0;
        let cy = (r[1] + r[3]) / 2.0;
        let pts = Self::arc_pts(cx, cy, (r[2] - r[0]).abs() / 2.0, (r[3] - r[1]).abs() / 2.0, 0.0, std::f64::consts::TAU);
        let mut v = self.bezier_path(Some(pts[0]), &pts[1..]);
        v.push(Seg::Z);
        v
    }

    pub(crate) fn select(&mut self, h: u32) {
        let o = if h & 0x8000_0000 != 0 { stock(h) } else { self.objs.get(&h).cloned().unwrap_or(Obj::Other) };
        match o {
            Obj::Pen(p) => self.dc.pen = p,
            Obj::Brush(b) => self.dc.brush = b,
            Obj::Font(f) => self.dc.font = f,
            Obj::Region(r) => {
                let path = self.poly_path(&Self::rect_points(r), true);
                self.set_clip(vec![path]);
            }
            Obj::Other => {}
        }
    }

    pub(crate) fn picture(&self, bmi: &[u8], bits: &[u8]) -> Option<Image> {
        dib::decode(bmi, bits)
    }

    /// Draw a bitmap: destination rectangle in logical units, source
    /// rectangle in bitmap pixels, raster operation.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn blit(&mut self, img: Image, dst: [f64; 4], src: [i64; 4], rop: u32, bottom_up: bool) {
        let mut img = img;
        // crop to the source rectangle (y counted from the bottom for
        // bottom-up pictures)
        let (sx, sy, sw, sh) = (src[0].max(0) as u32, src[1].max(0) as u32, src[2].unsigned_abs() as u32, src[3].unsigned_abs() as u32);
        if sw > 0 && sh > 0 && (sw != img.w || sh != img.h || sx != 0 || sy != 0) {
            let y_top = if bottom_up { img.h.saturating_sub(sy + sh) } else { sy };
            if let Some(c) = dib::crop(&img, sx, y_top, sw, sh) {
                img = c;
            }
        }
        if let ImageData::Rgba(px) = &mut img.data {
            let brush = rgb(self.dc.brush.color);
            match rop {
                0x8800C6 => {
                    for p in px.chunks_exact_mut(4) {
                        if p[0] > 250 && p[1] > 250 && p[2] > 250 {
                            p[3] = 0;
                        }
                    }
                }
                0xEE0086 | 0x660046 => {
                    for p in px.chunks_exact_mut(4) {
                        if p[0] < 5 && p[1] < 5 && p[2] < 5 {
                            p[3] = 0;
                        }
                    }
                }
                0xB8074A => {
                    for p in px.chunks_exact_mut(4) {
                        if p[0] < 128 {
                            p[0] = (brush >> 16) as u8;
                            p[1] = (brush >> 8) as u8;
                            p[2] = brush as u8;
                        } else {
                            p[3] = 0;
                        }
                    }
                }
                0xE20746 => {
                    for p in px.chunks_exact_mut(4) {
                        if p[0] >= 128 {
                            p[0] = (brush >> 16) as u8;
                            p[1] = (brush >> 8) as u8;
                            p[2] = brush as u8;
                        } else {
                            p[3] = 0;
                        }
                    }
                }
                0x330008 => {
                    for p in px.chunks_exact_mut(4) {
                        p[0] = 255 - p[0];
                        p[1] = 255 - p[1];
                        p[2] = 255 - p[2];
                    }
                }
                _ => {}
            }
        }
        let (x0, y0) = self.xf().apply(dst[0], dst[1]);
        let (x1, y1) = self.xf().apply(dst[0] + dst[2], dst[1]);
        let (x2, y2) = self.xf().apply(dst[0], dst[1] + dst[3]);
        let m = [(x1 - x0) as f32, (y1 - y0) as f32, (x2 - x0) as f32, (y2 - y0) as f32, x0 as f32, y0 as f32];
        let clip = self.clip_index();
        self.out.images.push(img);
        let image = (self.out.images.len() - 1) as u32;
        self.out.items.push(Item::Image { image, m, clip, alpha: 1.0 });
    }

    pub(crate) fn pattern_rect(&mut self, dst: [f64; 4], rop: u32) {
        let color = match rop {
            0x000042 => Some(0),
            0xFF0062 => Some(0xffffff),
            0xF00021 | 0xA000C9 | 0xFA0089 | 0x5A0049 => {
                if self.dc.brush.style == 1 {
                    None
                } else {
                    Some(rgb(self.dc.brush.color))
                }
            }
            _ => None,
        };
        let Some(color) = color else { return };
        if rop == 0xA000C9 && color == 0xffffff {
            return;
        }
        let p = self.poly_path(&Self::rect_points([dst[0], dst[1], dst[0] + dst[2], dst[1] + dst[3]]), true);
        let clip = self.clip_index();
        self.out.items.push(Item::Fill { path: p, color, evenodd: false, clip, mul: false });
    }

    pub(crate) fn text(&mut self, rec: &[u8], wide: bool) {
        // EMREXTTEXTOUTW: bounds(8) mode(24) exScale eyScale, then EMRTEXT at 36
        let rx = i32le(rec, 36) as f64;
        let ry = i32le(rec, 40) as f64;
        let n = u32le(rec, 44) as usize;
        let off = u32le(rec, 48) as usize;
        let opts = u32le(rec, 52);
        let rcl = [i32le(rec, 56) as f64, i32le(rec, 60) as f64, i32le(rec, 64) as f64, i32le(rec, 68) as f64];
        let off_dx = u32le(rec, 72) as usize;
        let step = if opts & 0x2000 != 0 { 2 } else { 1 };
        let dx: Vec<f64> = if off_dx > 0 { (0..n).map(|k| i32le(rec, off_dx + 4 * k * step) as f64).collect() } else { Vec::new() };
        let (text, per_char) = if wide {
            let units: Vec<u16> = (0..n).map(|k| i16le(rec, off + 2 * k) as u16).collect();
            let text = String::from_utf16_lossy(&units);
            let lens: Vec<usize> = text.chars().map(|c| c.len_utf16()).collect();
            (text, lens)
        } else {
            let raw = rec.get(off..off + n).unwrap_or(&[]);
            sjis_chars(raw)
        };
        let adv = group_advances(&dx, &per_char);
        self.draw_text(rx, ry, text, adv, opts, rcl);
    }

    /// Draw a run of text at logical (rx, ry). `adv` are the advances per
    /// character in logical units (None: estimate from the font).
    pub(crate) fn draw_text(&mut self, rx: f64, ry: f64, text: String, adv: Option<Vec<f64>>, opts: u32, rcl: [f64; 4]) {
        if opts & 0x2 != 0 && rcl[2] > rcl[0] && rcl[3] > rcl[1] {
            let p = self.poly_path(&Self::rect_points(rcl), true);
            let clip = self.clip_index();
            self.out.items.push(Item::Fill { path: p, color: rgb(self.dc.bk_color), evenodd: false, clip, mul: false });
        }
        if text.is_empty() {
            return;
        }
        if opts & 0x10 != 0 {
            // glyph numbers instead of characters: nothing readable
            self.out.skipped.push("glyph-index text".into());
            return;
        }
        let f = self.dc.font.clone();
        let xf = self.xf();
        let ys = xf.scale_y();
        let cjk = matches!(f.charset, 128 | 129 | 134 | 136) || f.face.chars().any(|c| c as u32 > 0x2e80);
        let h = f.height as f64;
        let em_l = if h < 0.0 {
            -h
        } else if h > 0.0 {
            if cjk { h } else { h / 1.15 }
        } else {
            12.0
        };
        let em = em_l * ys;
        let vertical = f.face.starts_with('@');
        let angle = f.escapement as f64 / 10.0;
        let adv: Vec<f64> = match adv {
            Some(a) if a.len() == text.chars().count() => a,
            _ => text
                .chars()
                .map(|c| if (c as u32) < 0x2000 || ('\u{ff61}'..='\u{ff9f}').contains(&c) { em_l * 0.5 } else { em_l })
                .collect(),
        };
        let total: f64 = adv.iter().sum();
        let (mut x, mut y) = (rx, ry);
        let ta = self.dc.text_align;
        if ta & 1 != 0 {
            x = self.dc.cur.0;
            y = self.dc.cur.1;
        }
        let shift_x = match ta & 6 {
            2 => -total,
            6 => -total / 2.0,
            _ => 0.0,
        };
        let ascent = if cjk { 0.88 } else { 0.9 } * em_l;
        let descent = em_l - ascent;
        let shift_y = match ta & 24 {
            24 => 0.0,
            8 => -descent,
            _ => ascent,
        };
        // baseline direction in logical space: escapement turns
        // counter-clockwise as seen on the page
        let flip = xf.0[3] < 0.0;
        let a = angle.to_radians();
        let (ca, sa) = (a.cos(), if flip { a.sin() } else { -a.sin() });
        // "down" (towards the descent) relative to the baseline
        let (dnx, dny) = if flip { (sa, -ca) } else { (-sa, ca) };
        let bx = x + ca * shift_x + dnx * shift_y;
        let by = y + sa * shift_x + dny * shift_y;
        let (px0, py0) = xf.apply(bx, by);
        let scale_x = (xf.0[0] * xf.0[0] + xf.0[1] * xf.0[1]).sqrt();
        let mut xs = Vec::with_capacity(adv.len());
        let mut acc = 0.0;
        for a in &adv {
            xs.push((acc * scale_x) as f32);
            acc += a;
        }
        if ta & 1 != 0 {
            self.dc.cur = (x + ca * total, y + sa * total);
        }
        if text.trim().is_empty() {
            return;
        }
        let sx = if f.width != 0 && h != 0.0 { (f.width.unsigned_abs() as f64 / (em_l / 2.0)) as f32 } else { 1.0 };
        let clip = self.clip_index();
        let face = f.face.trim_start_matches('@').to_string();
        self.out.items.push(Item::Text {
            x: px0 as f32,
            y: py0 as f32,
            angle: angle as f32,
            size: em as f32,
            sx,
            face,
            weight: f.weight.clamp(0, 1000) as u16,
            italic: f.italic,
            underline: f.underline,
            strike: f.strike,
            vertical,
            color: rgb(self.dc.text_color),
            text,
            xs,
            clip,
        });
    }

    pub(crate) fn dw(&mut self, rec: &[u8]) {
        // GDICOMMENT: type, size, cbData, data
        let data = &rec[12..];
        if data.len() < 4 || &data[..2] != b"DW" {
            return;
        }
        let (a, b2) = (data[2], data[3]);
        let p = &data[4..];
        match (a, b2) {
            (b'0', b'2') => self.dw_path.clear(),
            (b'0', b'3') => {
                let path = Path(std::mem::take(&mut self.dw_path));
                self.set_clip(vec![path]);
            }
            (b'0', b'4') => {
                let path = Path(std::mem::take(&mut self.dw_path));
                self.fill(path);
            }
            (b'0', b'5') => {
                let path = Path(std::mem::take(&mut self.dw_path));
                self.stroke(path);
            }
            (b'0', b'6') => {
                let r = [i32le(p, 0) as f64, i32le(p, 4) as f64, i32le(p, 8) as f64, i32le(p, 12) as f64];
                let path = self.poly_path(&Self::rect_points(r), true);
                self.set_clip(vec![path]);
            }
            (b'0', _) => {}
            (b'a', 0) => {
                let off_bmi = u32le(p, 0) as usize;
                let cb_bmi = u32le(p, 4) as usize;
                let off_bits = u32le(p, 8) as usize;
                let cb_bits = u32le(p, 12) as usize;
                let bmi = rec.get(off_bmi..off_bmi + cb_bmi).unwrap_or(&[]);
                let bits = rec.get(off_bits..off_bits + cb_bits).unwrap_or(&[]);
                let bottom_up = dib::info(bmi).map(|i| !i.top_down).unwrap_or(true);
                self.dw_image = self.picture(bmi, bits).map(|i| (i, bottom_up));
            }
            (b'b', 0) => {
                let pic = self.pics.get(self.next_pic);
                self.next_pic += 1;
                self.dw_image = pic.and_then(|d| external_picture(d));
            }
            (b'c', 0) => {
                let v: Vec<i64> = (0..14).map(|k| i32le(p, 4 * k) as i64).collect();
                let Some((img, bottom_up)) = self.dw_image.clone() else {
                    self.out.skipped.push("DWc without picture".into());
                    return;
                };
                let dst = [v[4] as f64, v[5] as f64, v[12] as f64, v[13] as f64];
                let src = [v[6], v[7], v[8], v[9]];
                self.blit(img, dst, src, v[11] as u32, bottom_up);
            }
            (op, enc) if matches!(enc, 0x20 | 0x40 | 0x80) => {
                let pts = dw_points(p, enc);
                if pts.is_empty() {
                    return;
                }
                match op {
                    0x20 => {
                        let path = self.poly_path(&pts, true);
                        self.fill(path);
                    }
                    0x40 => {
                        let path = self.poly_path(&pts, false);
                        self.stroke(path);
                    }
                    _ => {
                        let start = op & 1 != 0;
                        let bez = op & 0x10 != 0;
                        let mut segs = Vec::new();
                        let rest: &[(f64, f64)] = if start {
                            let (x, y) = self.pt(pts[0].0, pts[0].1);
                            segs.push(Seg::M(x, y));
                            &pts[1..]
                        } else {
                            &pts[..]
                        };
                        if bez {
                            segs.extend(self.bezier_path(None, rest));
                        } else {
                            for &(x, y) in rest {
                                let (a, b) = self.pt(x, y);
                                segs.push(Seg::L(a, b));
                            }
                        }
                        if op & 2 != 0 {
                            segs.push(Seg::Z);
                        }
                        // a figure that does not start with a move continues
                        // from the last point; start one if there is none
                        if !start && self.dw_path.is_empty() {
                            if let Some(&(x, y)) = pts.first() {
                                let (a, b) = self.pt(x, y);
                                self.dw_path.push(Seg::M(a, b));
                            }
                        }
                        self.dw_path.extend(segs);
                    }
                }
            }
            _ => {
                self.out.skipped.push(format!("DW {:02x}{:02x}", a, b2));
            }
        }
    }

    pub(crate) fn record(&mut self, t: u32, rec: &[u8]) {
        let p = |k: usize| i32le(rec, 8 + 4 * k) as f64;
        let pts32 = |at: usize, n: usize| -> Vec<(f64, f64)> {
            (0..n).map(|k| (i32le(rec, at + 8 * k) as f64, i32le(rec, at + 8 * k + 4) as f64)).collect()
        };
        let pts16 = |at: usize, n: usize| -> Vec<(f64, f64)> {
            (0..n).map(|k| (i16le(rec, at + 4 * k) as f64, i16le(rec, at + 4 * k + 2) as f64)).collect()
        };
        match t {
            1 | 14 => {}
            // polybezier / polygon / polyline (32 and 16 bit)
            2 | 3 | 4 | 5 | 6 | 85 | 86 | 87 | 88 | 89 => {
                let n = u32le(rec, 24) as usize;
                let small = t >= 85;
                if rec.len() < 28 + n * if small { 4 } else { 8 } {
                    return;
                }
                let pts = if small { pts16(28, n) } else { pts32(28, n) };
                let kind = if small { t - 83 } else { t };
                match kind {
                    2 => {
                        if let Some(&first) = pts.first() {
                            let segs = self.bezier_path(Some(first), &pts[1..]);
                            self.shape(segs, false);
                        }
                    }
                    3 => self.draw_poly(&pts, true),
                    4 => self.draw_poly(&pts, false),
                    5 => {
                        let mut segs = Vec::new();
                        if self.in_path {
                            self.path_start();
                        } else {
                            let (a, b) = self.pt(self.dc.cur.0, self.dc.cur.1);
                            segs.push(Seg::M(a, b));
                        }
                        segs.extend(self.bezier_path(None, &pts));
                        if let Some(&l) = pts.last() {
                            self.dc.cur = l;
                        }
                        self.shape(segs, false);
                    }
                    6 => {
                        let mut all = vec![self.dc.cur];
                        all.extend_from_slice(&pts);
                        if self.in_path {
                            self.path_start();
                            for &(x, y) in &pts {
                                let (a, b) = self.pt(x, y);
                                self.path.push(Seg::L(a, b));
                            }
                        } else {
                            let path = self.poly_path(&all, false);
                            self.stroke(path);
                        }
                        if let Some(&l) = pts.last() {
                            self.dc.cur = l;
                        }
                    }
                    _ => {}
                }
            }
            // polypolyline / polypolygon (32 and 16 bit)
            7 | 8 | 90 | 91 => {
                let polys = u32le(rec, 24) as usize;
                let total = u32le(rec, 28) as usize;
                let small = t >= 90;
                let counts: Vec<usize> = (0..polys).map(|k| u32le(rec, 32 + 4 * k) as usize).collect();
                let at = 32 + 4 * polys;
                let all = if small { pts16(at, total) } else { pts32(at, total) };
                let polygon = t == 8 || t == 91;
                let mut segs = Vec::new();
                let mut k = 0;
                for c in counts {
                    let part = &all[k.min(all.len())..(k + c).min(all.len())];
                    segs.extend(self.poly_path(part, polygon).0);
                    k += c;
                }
                self.shape(segs, polygon);
            }
            9 => self.dc.wext = (p(0), p(1)),
            10 => self.dc.worg = (p(0), p(1)),
            11 | 12 | 17 if self.fitted => {}
            11 => self.dc.vext = (p(0), p(1)),
            12 => self.dc.vorg = (p(0), p(1)),
            17 => self.dc.map_mode = u32le(rec, 8),
            18 => self.dc.bk_mode = u32le(rec, 8),
            19 => self.dc.fill_mode = u32le(rec, 8),
            20 => self.dc.rop2 = u32le(rec, 8),
            22 => self.dc.text_align = u32le(rec, 8),
            24 => self.dc.text_color = u32le(rec, 8),
            25 => self.dc.bk_color = u32le(rec, 8),
            27 => {
                self.dc.cur = (p(0), p(1));
                if self.in_path {
                    let (a, b) = self.pt(p(0), p(1));
                    self.path.push(Seg::M(a, b));
                }
            }
            29 => {}
            30 => {
                let r = [p(0), p(1), p(2), p(3)];
                let path = self.poly_path(&Self::rect_points(r), true);
                let mut c = self.dc.clip.clone();
                c.push(path);
                self.set_clip(c);
            }
            31 => {
                let (xn, xd, yn, yd) = (p(0), p(1), p(2), p(3));
                if xd != 0.0 && yd != 0.0 {
                    self.dc.vext = (self.dc.vext.0 * xn / xd, self.dc.vext.1 * yn / yd);
                }
            }
            32 => {
                let (xn, xd, yn, yd) = (p(0), p(1), p(2), p(3));
                if xd != 0.0 && yd != 0.0 {
                    self.dc.wext = (self.dc.wext.0 * xn / xd, self.dc.wext.1 * yn / yd);
                }
            }
            33 => self.saved.push(self.dc.clone()),
            34 => {
                let n = i32le(rec, 8);
                let keep = if n < 0 { self.saved.len().saturating_sub((-n) as usize) } else { (n as usize).saturating_sub(1) };
                if keep < self.saved.len() {
                    self.saved.truncate(keep + 1);
                    if let Some(d) = self.saved.pop() {
                        self.dc = d;
                        self.last_clip = None;
                    }
                }
            }
            35 => self.dc.world = xform(rec, 8),
            36 => {
                let x = xform(rec, 8);
                match u32le(rec, 32) {
                    1 => self.dc.world = Xf::ID,
                    2 => self.dc.world = self.dc.world.after(&x),
                    3 => self.dc.world = x.after(&self.dc.world),
                    4 => self.dc.world = x,
                    _ => {}
                }
            }
            37 => self.select(u32le(rec, 8)),
            38 => {
                let h = u32le(rec, 8);
                let style = u32le(rec, 12);
                let width = i32le(rec, 16) as f64;
                let color = u32le(rec, 24);
                self.objs.insert(h, Obj::Pen(Pen { style, width, color, geometric: false, dash: vec![] }));
            }
            39 => {
                let h = u32le(rec, 8);
                self.objs.insert(h, Obj::Brush(Brush { style: u32le(rec, 12), color: u32le(rec, 16) }));
            }
            40 => {
                self.objs.remove(&u32le(rec, 8));
            }
            42 => {
                let r = [p(0), p(1), p(2), p(3)];
                let segs = self.ellipse_segs(r);
                self.shape(segs, true);
            }
            43 => {
                let r = [p(0), p(1), p(2), p(3)];
                let path = self.poly_path(&Self::rect_points(r), true);
                self.shape(path.0, true);
            }
            44 => {
                let r = [p(0), p(1), p(2), p(3)];
                let (cw, ch) = (p(4).abs() / 2.0, p(5).abs() / 2.0);
                let (x0, y0, x1, y1) = (r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3]));
                let mut pts = Vec::new();
                let h = std::f64::consts::FRAC_PI_2;
                for (cx, cy, a0) in [(x1 - cw, y0 + ch, 0.0), (x0 + cw, y0 + ch, h), (x0 + cw, y1 - ch, 2.0 * h), (x1 - cw, y1 - ch, 3.0 * h)] {
                    pts.push(Self::arc_pts(cx, cy, cw, ch, a0, h));
                }
                let mut segs = Vec::new();
                for (k, a) in pts.iter().enumerate() {
                    let (x, y) = self.pt(a[0].0, a[0].1);
                    segs.push(if k == 0 { Seg::M(x, y) } else { Seg::L(x, y) });
                    segs.extend(self.bezier_path(None, &a[1..]));
                }
                segs.push(Seg::Z);
                self.shape(segs, true);
            }
            41 => {
                // ANGLEARC: center, radius, start angle, sweep (degrees)
                let (cx, cy, r) = (p(0), p(1), u32le(rec, 16) as f64);
                let a0 = (f32le(rec, 20) as f64).to_radians();
                let sw = (f32le(rec, 24) as f64).to_radians();
                let pts = Self::arc_pts(cx, cy, r, r, a0, sw);
                let mut segs = vec![];
                let (a, b) = self.pt(self.dc.cur.0, self.dc.cur.1);
                if !self.in_path {
                    segs.push(Seg::M(a, b));
                }
                let (x, y) = self.pt(pts[0].0, pts[0].1);
                segs.push(Seg::L(x, y));
                segs.extend(self.bezier_path(None, &pts[1..]));
                if let Some(&l) = pts.last() {
                    self.dc.cur = l;
                }
                self.shape(segs, false);
            }
            45..=47 | 55 => {
                let r = [p(0), p(1), p(2), p(3)];
                let (cx, cy, rx, ry, a0, sweep) = self.arc_angles(r, (p(4), p(5)), (p(6), p(7)));
                let pts = Self::arc_pts(cx, cy, rx, ry, a0, sweep);
                let mut segs = Vec::new();
                if t == 55 {
                    let (a, b) = self.pt(self.dc.cur.0, self.dc.cur.1);
                    if !self.in_path {
                        segs.push(Seg::M(a, b));
                    }
                    let (x, y) = self.pt(pts[0].0, pts[0].1);
                    segs.push(Seg::L(x, y));
                    segs.extend(self.bezier_path(None, &pts[1..]));
                } else if t == 47 {
                    let (x, y) = self.pt(cx, cy);
                    segs.push(Seg::M(x, y));
                    let (a, b) = self.pt(pts[0].0, pts[0].1);
                    segs.push(Seg::L(a, b));
                    segs.extend(self.bezier_path(None, &pts[1..]));
                    segs.push(Seg::Z);
                } else {
                    segs.extend(self.bezier_path(Some(pts[0]), &pts[1..]));
                    if t == 46 {
                        segs.push(Seg::Z);
                    }
                }
                if t == 55 {
                    if let Some(&l) = pts.last() {
                        self.dc.cur = l;
                    }
                }
                self.shape(segs, t == 46 || t == 47);
            }
            54 => {
                let (x, y) = (p(0), p(1));
                if self.in_path {
                    self.path_start();
                    let (a, b) = self.pt(x, y);
                    self.path.push(Seg::L(a, b));
                } else {
                    let path = self.poly_path(&[self.dc.cur, (x, y)], false);
                    self.stroke(path);
                }
                self.dc.cur = (x, y);
            }
            56 | 92 => {
                let n = u32le(rec, 24) as usize;
                let small = t == 92;
                let pts = if small { pts16(28, n) } else { pts32(28, n) };
                let types_at = 28 + n * if small { 4 } else { 8 };
                let mut segs = Vec::new();
                let mut k = 0;
                while k < n {
                    let ty = rec.get(types_at + k).copied().unwrap_or(2);
                    let (x, y) = self.pt(pts[k].0, pts[k].1);
                    match ty & 6 {
                        6 => segs.push(Seg::M(x, y)),
                        4 if k + 2 < n => {
                            let (c, d) = self.pt(pts[k + 1].0, pts[k + 1].1);
                            let (e, f) = self.pt(pts[k + 2].0, pts[k + 2].1);
                            segs.push(Seg::C(x, y, c, d, e, f));
                            k += 2;
                        }
                        _ => segs.push(Seg::L(x, y)),
                    }
                    if rec.get(types_at + k).copied().unwrap_or(0) & 1 != 0 {
                        segs.push(Seg::Z);
                    }
                    k += 1;
                }
                if let Some(&l) = pts.last() {
                    self.dc.cur = l;
                }
                self.shape(segs, false);
            }
            57 => self.dc.arc_ccw = u32le(rec, 8) != 2,
            59 => {
                self.in_path = true;
                self.path.clear();
            }
            60 => self.in_path = false,
            61 => {
                if self.in_path {
                    self.path.push(Seg::Z);
                }
            }
            62 => {
                let path = Path(std::mem::take(&mut self.path));
                self.fill(path);
            }
            63 => {
                let path = Path(std::mem::take(&mut self.path));
                self.fill(path.clone());
                self.stroke(path);
            }
            64 => {
                let path = Path(std::mem::take(&mut self.path));
                self.stroke(path);
            }
            67 => {
                let path = Path(std::mem::take(&mut self.path));
                let mode = u32le(rec, 8);
                let mut c = if mode == 5 { Vec::new() } else { self.dc.clip.clone() };
                if mode == 1 || mode == 5 {
                    c.push(path);
                }
                self.set_clip(c);
            }
            68 => {
                self.in_path = false;
                self.path.clear();
            }
            70 => self.dw(rec),
            75 => {
                let cb = u32le(rec, 8) as usize;
                let mode = u32le(rec, 12);
                if cb == 0 {
                    if mode == 5 {
                        self.set_clip(Vec::new());
                    }
                    return;
                }
                let d = &rec[16..];
                let n = u32le(d, 8) as usize;
                let mut segs = Vec::new();
                for k in 0..n {
                    let at = 32 + 16 * k;
                    let r = [i32le(d, at) as f64, i32le(d, at + 4) as f64, i32le(d, at + 8) as f64, i32le(d, at + 12) as f64];
                    let (x0, y0) = self.dev.apply(r[0], r[1]);
                    let (x1, y1) = self.dev.apply(r[2], r[3]);
                    segs.extend(Path::rect(x0 as f32, y0 as f32, x1 as f32, y1 as f32).0);
                }
                let path = Path(segs);
                match mode {
                    5 => self.set_clip(vec![path]),
                    1 => {
                        let mut c = self.dc.clip.clone();
                        c.push(path);
                        self.set_clip(c);
                    }
                    _ => {}
                }
            }
            76 | 77 => {
                // BITBLT / STRETCHBLT
                let dst = [p(4), p(5), p(6), p(7)];
                let rop = u32le(rec, 40);
                let (xs, ys) = (i32le(rec, 44) as i64, i32le(rec, 48) as i64);
                let off_bmi = u32le(rec, 84) as usize;
                let cb_bmi = u32le(rec, 88) as usize;
                let off_bits = u32le(rec, 92) as usize;
                let cb_bits = u32le(rec, 96) as usize;
                if cb_bmi == 0 {
                    self.pattern_rect(dst, rop);
                    return;
                }
                let bmi = rec.get(off_bmi..off_bmi + cb_bmi).unwrap_or(&[]);
                let bits = rec.get(off_bits..off_bits + cb_bits).unwrap_or(&[]);
                let (cxs, cys) = if t == 77 { (i32le(rec, 100) as i64, i32le(rec, 104) as i64) } else { (dst[2] as i64, dst[3] as i64) };
                if let Some(img) = self.picture(bmi, bits) {
                    let bu = dib::info(bmi).map(|i| !i.top_down).unwrap_or(true);
                    self.blit(img, dst, [xs, ys, cxs, cys], rop, bu);
                }
            }
            80 | 81 => {
                // SETDIBITSTODEVICE / STRETCHDIBITS
                let (xd, yd) = (p(4), p(5));
                let (xs, ys, cxs, cys) = (i32le(rec, 32) as i64, i32le(rec, 36) as i64, i32le(rec, 40) as i64, i32le(rec, 44) as i64);
                let off_bmi = u32le(rec, 48) as usize;
                let cb_bmi = u32le(rec, 52) as usize;
                let off_bits = u32le(rec, 56) as usize;
                let cb_bits = u32le(rec, 60) as usize;
                let (rop, cxd, cyd) = if t == 81 {
                    (u32le(rec, 68), i32le(rec, 72) as f64, i32le(rec, 76) as f64)
                } else {
                    (0xCC0020, cxs as f64, cys as f64)
                };
                if cb_bmi == 0 {
                    self.pattern_rect([xd, yd, cxd, cyd], rop);
                    return;
                }
                let bmi = rec.get(off_bmi..off_bmi + cb_bmi).unwrap_or(&[]);
                let bits = rec.get(off_bits..off_bits + cb_bits).unwrap_or(&[]);
                if let Some(img) = self.picture(bmi, bits) {
                    let bu = dib::info(bmi).map(|i| !i.top_down).unwrap_or(true);
                    self.blit(img, [xd, yd, cxd, cyd], [xs, ys, cxs, cys], rop, bu);
                }
            }
            114 | 116 => {
                // ALPHABLEND / TRANSPARENTBLT (drawn as plain copies)
                let dst = [p(4), p(5), p(6), p(7)];
                let (xs, ys) = (i32le(rec, 44) as i64, i32le(rec, 48) as i64);
                let off_bmi = u32le(rec, 84) as usize;
                let cb_bmi = u32le(rec, 88) as usize;
                let off_bits = u32le(rec, 92) as usize;
                let cb_bits = u32le(rec, 96) as usize;
                let (cxs, cys) = (i32le(rec, 100) as i64, i32le(rec, 104) as i64);
                let bmi = rec.get(off_bmi..off_bmi + cb_bmi).unwrap_or(&[]);
                let bits = rec.get(off_bits..off_bits + cb_bits).unwrap_or(&[]);
                if let Some(img) = self.picture(bmi, bits) {
                    let bu = dib::info(bmi).map(|i| !i.top_down).unwrap_or(true);
                    self.blit(img, dst, [xs, ys, cxs, cys], 0xCC0020, bu);
                    if t == 114 {
                        // BLENDFUNCTION: constant opacity in its third byte
                        let a = rec.get(42).copied().unwrap_or(255) as f32 / 255.0;
                        if let Some(Item::Image { alpha, .. }) = self.out.items.last_mut() {
                            *alpha = a;
                        }
                    }
                }
            }
            82 => {
                let h = u32le(rec, 8);
                let lf = &rec[12..];
                let face_units: Vec<u16> = (0..32).map(|k| i16le(lf, 28 + 2 * k) as u16).take_while(|&c| c != 0).collect();
                self.objs.insert(
                    h,
                    Obj::Font(Font {
                        height: i32le(lf, 0),
                        width: i32le(lf, 4),
                        escapement: i32le(lf, 8),
                        weight: i32le(lf, 16),
                        italic: lf.get(20).copied().unwrap_or(0) != 0,
                        underline: lf.get(21).copied().unwrap_or(0) != 0,
                        strike: lf.get(22).copied().unwrap_or(0) != 0,
                        charset: lf.get(23).copied().unwrap_or(0),
                        face: String::from_utf16_lossy(&face_units),
                    }),
                );
            }
            83 => self.text(rec, false),
            84 => self.text(rec, true),
            95 => {
                let h = u32le(rec, 8);
                let style = u32le(rec, 28);
                let width = u32le(rec, 32) as f64;
                let color = u32le(rec, 40);
                let n = u32le(rec, 48) as usize;
                let dash = if style & 0xf == 7 { (0..n).map(|k| u32le(rec, 52 + 4 * k) as f64).collect() } else { vec![] };
                self.objs.insert(h, Obj::Pen(Pen { style, width, color, geometric: style & 0x10000 != 0, dash }));
            }
            93 | 94 => {
                // pattern brushes: use a mid grey stand-in
                let h = u32le(rec, 8);
                self.objs.insert(h, Obj::Brush(Brush { style: 0, color: 0x808080 }));
            }
            13 | 15 | 16 | 21 | 23 | 26 | 28 | 48..=53 | 58 | 65 | 66 | 98..=113 | 115 | 117 | 119..=122 => {}
            _ => self.out.skipped.push(format!("EMF record {t}")),
        }
    }
}

/// Shift_JIS bytes → text and how many bytes each character took.
pub(crate) fn sjis_chars(raw: &[u8]) -> (String, Vec<usize>) {
    let mut text = String::new();
    let mut lens = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let c = raw[i];
        let n = if (0x81..=0x9f).contains(&c) || (0xe0..=0xfc).contains(&c) { 2 } else { 1 };
        let n = n.min(raw.len() - i);
        let s = crate::sjis::decode(&raw[i..i + n]);
        let s = if s.is_empty() && c == 0 { "\0".to_string() } else { s };
        for ch in s.chars() {
            text.push(ch);
            lens.push(n);
        }
        i += n;
    }
    (text, lens)
}

/// Sum per-unit advances into per-character advances.
pub(crate) fn group_advances(dx: &[f64], per_char: &[usize]) -> Option<Vec<f64>> {
    if dx.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(per_char.len());
    let mut k = 0;
    for &n in per_char {
        out.push(dx.get(k..k + n).map(|v| v.iter().sum()).unwrap_or(0.0));
        k += n;
    }
    Some(out)
}

fn xform(rec: &[u8], at: usize) -> Xf {
    Xf([
        f32le(rec, at) as f64,
        f32le(rec, at + 4) as f64,
        f32le(rec, at + 8) as f64,
        f32le(rec, at + 12) as f64,
        f32le(rec, at + 16) as f64,
        f32le(rec, at + 20) as f64,
    ])
}

/// Points of a DocuWorks path record.
pub fn dw_points(p: &[u8], enc: u8) -> Vec<(f64, f64)> {
    let n = u32le(p, 0) as usize;
    if n == 0 || p.len() < 8 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n);
    let (mut x, mut y) = (i16le(p, 4) as f64, i16le(p, 6) as f64);
    out.push((x, y));
    let mut i = 8;
    for _ in 1..n {
        match enc {
            0x20 => {
                if i + 4 > p.len() {
                    break;
                }
                x = i16le(p, i) as f64;
                y = i16le(p, i + 2) as f64;
                i += 4;
            }
            0x40 => {
                if i + 2 > p.len() {
                    break;
                }
                x += p[i] as i8 as f64;
                y += p[i + 1] as i8 as f64;
                i += 2;
            }
            _ => {
                // one byte: x step in the high nibble, y step in the low
                // nibble; a nibble of 8 means "the step is in the next byte"
                if i >= p.len() {
                    break;
                }
                let b = p[i];
                i += 1;
                let (hx, lx) = (b >> 4, b & 15);
                let step = |nib: u8, i: &mut usize| -> f64 {
                    if nib == 8 {
                        let v = p.get(*i).copied().unwrap_or(0) as i8 as f64;
                        *i += 1;
                        v
                    } else {
                        ((nib as i8) << 4 >> 4) as f64
                    }
                };
                x += step(hx, &mut i);
                y += step(lx, &mut i);
            }
        }
        out.push((x, y));
    }
    out
}

/// A picture stored in its own entry: 16-byte header (size, 0, width,
/// height) and then JPEG data, or a DIB.
pub fn external_picture(d: &[u8]) -> Option<(Image, bool)> {
    if d.len() > 18 && d[16] == 0xff && d[17] == 0xd8 {
        let w = u32le(d, 8);
        let h = u32le(d, 12);
        return Some((Image { w, h, data: ImageData::Jpeg(d[16..].to_vec()) }, false));
    }
    if d.len() > 2 && d[0] == 0xff && d[1] == 0xd8 {
        return Some((Image { w: 0, h: 0, data: ImageData::Jpeg(d.to_vec()) }, false));
    }
    // a DIB: BITMAPINFOHEADER then bits (possibly compressed, see dib)
    let inf = dib::info(d)?;
    let img = dib::decode_stored(d)?;
    Some((img, !inf.top_down))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibble_and_byte_steps() {
        let mut p = vec![4, 0, 0, 0, 10, 0, 20, 0];
        p.extend([0x7f, 0x91, 0x28, 0xf7]); // +7,-1 ; -7,+1 ; +2, then y in the next byte (-9)
        let v = dw_points(&p, 0x80);
        assert_eq!(v, vec![(10.0, 20.0), (17.0, 19.0), (10.0, 20.0), (12.0, 11.0)]);
        let mut q = vec![2, 0, 0, 0, 10, 0, 20, 0];
        q.extend([0xfe, 0x05]);
        assert_eq!(dw_points(&q, 0x40), vec![(10.0, 20.0), (8.0, 25.0)]);
    }
}
