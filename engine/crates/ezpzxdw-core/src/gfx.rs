//! What a page looks like, as a list of drawing operations in page units
//! (1/100 mm, origin top-left, y down). The web viewer draws this on a
//! canvas; PDF export uses it too.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum Seg {
    M(f32, f32),
    L(f32, f32),
    C(f32, f32, f32, f32, f32, f32),
    Z,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Path(pub Vec<Seg>);

impl Path {
    pub fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Path {
        Path(vec![Seg::M(x0, y0), Seg::L(x1, y0), Seg::L(x1, y1), Seg::L(x0, y1), Seg::Z])
    }
    pub fn is_empty(&self) -> bool {
        !self.0.iter().any(|s| matches!(s, Seg::L(..) | Seg::C(..)))
    }
    pub fn bounds(&self) -> Option<[f32; 4]> {
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        let mut any = false;
        let mut add = |x: f32, y: f32| {
            b[0] = b[0].min(x);
            b[1] = b[1].min(y);
            b[2] = b[2].max(x);
            b[3] = b[3].max(y);
            any = true;
        };
        for s in &self.0 {
            match *s {
                Seg::M(x, y) | Seg::L(x, y) => add(x, y),
                Seg::C(a, b2, c, d, e, f) => {
                    add(a, b2);
                    add(c, d);
                    add(e, f);
                }
                Seg::Z => {}
            }
        }
        any.then_some(b)
    }
}

/// A colour as 0xRRGGBB.
pub type Rgb = u32;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Item {
    Fill {
        path: Path,
        color: Rgb,
        evenodd: bool,
        clip: u32,
        /// Multiply with what is under it (a highlighter).
        mul: bool,
    },
    Stroke {
        path: Path,
        color: Rgb,
        width: f32,
        dash: Vec<f32>,
        /// 0 round, 1 square, 2 flat
        cap: u8,
        /// 0 round, 1 bevel, 2 miter
        join: u8,
        clip: u32,
    },
    /// One run of text. `x`, `y` is where the first character sits on the
    /// baseline; `xs` are the offsets of each character along the baseline
    /// (empty: let the font decide).
    Text {
        x: f32,
        y: f32,
        /// Degrees, counter-clockwise.
        angle: f32,
        /// Em size.
        size: f32,
        /// Horizontal scale of the glyphs (1 = normal).
        sx: f32,
        face: String,
        weight: u16,
        italic: bool,
        underline: bool,
        strike: bool,
        vertical: bool,
        color: Rgb,
        text: String,
        xs: Vec<f32>,
        clip: u32,
    },
    /// Picture `image` stretched so that its unit square (top-left origin)
    /// lands on the page through `m` = [a b c d e f]:
    /// page = (a*u + c*v + e, b*u + d*v + f).
    Image {
        image: u32,
        m: [f32; 6],
        clip: u32,
        /// Opacity 0-1.
        alpha: f32,
        /// Multiply with what is under it (SRCAND). Near-white pixels are
        /// also made transparent, for painters that cannot multiply.
        mul: bool,
    },
}

/// A clip: the intersection of the paths (each filled non-zero).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Clip(pub Vec<Path>);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ImageData {
    Jpeg(#[serde(skip)] Vec<u8>),
    /// 4 bytes per pixel, top row first, alpha 0 = transparent.
    Rgba(#[serde(skip)] Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub data: ImageData,
}

/// A page's drawing.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Display {
    /// Page size in 1/100 mm.
    pub w: f32,
    pub h: f32,
    pub items: Vec<Item>,
    /// Clip 0 is "no clip".
    pub clips: Vec<Clip>,
    pub images: Vec<Image>,
    /// Things the renderer could not draw (record names), for diagnostics.
    pub skipped: Vec<String>,
}

impl Display {
    pub fn new(w: f32, h: f32) -> Display {
        Display { w, h, clips: vec![Clip::default()], ..Default::default() }
    }
    /// All text of the page in drawing order: a new line when the baseline
    /// moves, a space where runs on a line are apart.
    pub fn text(&self) -> String {
        let mut out = String::new();
        let mut last: Option<(f32, f32)> = None; // (baseline y, end x)
        for it in &self.items {
            if let Item::Text { x, y, text, size, xs, angle, .. } = it {
                if let Some((ly, lx)) = last {
                    if (ly - y).abs() > size * 0.5 || angle.abs() > 1.0 {
                        out.push('\n');
                    } else if x - lx > size * 0.6 {
                        out.push(' ');
                    }
                }
                out.push_str(text);
                let n = text.chars().count();
                let end = x + xs.last().copied().unwrap_or(0.0) + if n > 0 { size * if text.chars().last().map(|c| (c as u32) < 0x2000).unwrap_or(false) { 0.5 } else { 1.0 } } else { 0.0 };
                last = Some((*y, end));
            }
        }
        out
    }
}

/// A 2D affine transform [a b c d e f]: (x, y) → (a x + c y + e, b x + d y + f).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xf(pub [f64; 6]);

impl Xf {
    pub const ID: Xf = Xf([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let m = &self.0;
        (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
    }
    /// self after other: first `other`, then `self`.
    pub fn after(&self, o: &Xf) -> Xf {
        let a = &self.0;
        let b = &o.0;
        Xf([
            a[0] * b[0] + a[2] * b[1],
            a[1] * b[0] + a[3] * b[1],
            a[0] * b[2] + a[2] * b[3],
            a[1] * b[2] + a[3] * b[3],
            a[0] * b[4] + a[2] * b[5] + a[4],
            a[1] * b[4] + a[3] * b[5] + a[5],
        ])
    }
    /// How much lengths grow (geometric mean of the axes).
    pub fn scale(&self) -> f64 {
        let m = &self.0;
        (m[0] * m[3] - m[1] * m[2]).abs().sqrt()
    }
    pub fn scale_y(&self) -> f64 {
        (self.0[2] * self.0[2] + self.0[3] * self.0[3]).sqrt()
    }
}
