//! Text that something drawn later covers completely.
//!
//! People black out names and numbers by putting an opaque box (a rectangle
//! annotation, a pasted picture) over them. The page then looks redacted,
//! but the characters are still in the drawing underneath, and a reader that
//! collects every text item would hand them back in the extracted text and
//! in the PDF text layer. So after a page is drawn, each character is
//! checked against everything drawn after it:
//!
//! - an opaque fill (any colour, white included), within its clip;
//! - an opaque picture (JPEG, or pixels that are all fully opaque) at full
//!   opacity, within its clip.
//!
//! A highlighter (a fill or picture that multiplies) and a see-through
//! picture are not covers. Strokes are not covers either. A picture in the
//! same drawing as the text is not a cover: that is how a scanned page keeps
//! its recognised text under the scan, and how a page taken from a PDF keeps
//! its text under the page picture (the picture shows that very text).
//!
//! A character counts as covered when the inner part of its box (inset from
//! the cell its advance gives it: 15% from each side, from 0.7 em above the
//! baseline to the baseline) lies inside the cover: its four corners and
//! centre are inside, and no corner of the cover lies inside the box (so a
//! cover with a hole or a notch over the character does not count). Covered
//! characters are removed from the run; a run with none left is removed.

use crate::gfx::{Display, ImageData, Item, Path, Seg};

/// A shape drawn on top: polygons (with their fill rule) and the clip it
/// is drawn through.
struct Cover {
    /// Drawing order: only text drawn before it can be covered.
    at: usize,
    /// A picture: does not cover text of its own object.
    picture: bool,
    polys: Vec<Vec<(f32, f32)>>,
    evenodd: bool,
    clip: u32,
    bounds: [f32; 4],
}

/// Flatten a path into closed polygons (curves as 8 straight pieces).
fn polygons(p: &Path) -> Vec<Vec<(f32, f32)>> {
    let mut out: Vec<Vec<(f32, f32)>> = Vec::new();
    let mut cur: Vec<(f32, f32)> = Vec::new();
    for s in &p.0 {
        match *s {
            Seg::M(x, y) => {
                if cur.len() > 2 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
                cur.push((x, y));
            }
            Seg::L(x, y) => cur.push((x, y)),
            Seg::C(a, b, c, d, e, f) => {
                let (x0, y0) = cur.last().copied().unwrap_or((a, b));
                for k in 1..=8 {
                    let t = k as f32 / 8.0;
                    let u = 1.0 - t;
                    let x = u * u * u * x0 + 3.0 * u * u * t * a + 3.0 * u * t * t * c + t * t * t * e;
                    let y = u * u * u * y0 + 3.0 * u * u * t * b + 3.0 * u * t * t * d + t * t * t * f;
                    cur.push((x, y));
                }
            }
            Seg::Z => {
                if cur.len() > 2 {
                    let first = cur[0];
                    out.push(std::mem::take(&mut cur));
                    cur.push(first);
                } else {
                    cur.clear();
                }
            }
        }
    }
    if cur.len() > 2 {
        out.push(cur);
    }
    out
}

fn bounds(polys: &[Vec<(f32, f32)>]) -> Option<[f32; 4]> {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for &(x, y) in polys.iter().flatten() {
        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
    }
    (b[0] <= b[2] && b[0].is_finite() && b[2].is_finite() && b[1].is_finite() && b[3].is_finite()).then_some(b)
}

/// Winding number of the polygons around (x, y).
fn winding(polys: &[Vec<(f32, f32)>], x: f32, y: f32) -> i32 {
    let mut w = 0;
    for poly in polys {
        let n = poly.len();
        for i in 0..n {
            let (x0, y0) = poly[i];
            let (x1, y1) = poly[(i + 1) % n];
            if y0 <= y {
                if y1 > y && (x1 - x0) * (y - y0) - (x - x0) * (y1 - y0) > 0.0 {
                    w += 1;
                }
            } else if y1 <= y && (x1 - x0) * (y - y0) - (x - x0) * (y1 - y0) < 0.0 {
                w -= 1;
            }
        }
    }
    w
}

fn inside(polys: &[Vec<(f32, f32)>], evenodd: bool, x: f32, y: f32) -> bool {
    let w = winding(polys, x, y);
    if evenodd {
        w % 2 != 0
    } else {
        w != 0
    }
}

/// Is the point strictly inside the convex quadrilateral `q`?
fn in_quad(q: &[(f32, f32); 4], x: f32, y: f32) -> bool {
    let mut sign = 0.0f32;
    for i in 0..4 {
        let (x0, y0) = q[i];
        let (x1, y1) = q[(i + 1) % 4];
        let c = (x1 - x0) * (y - y0) - (x - x0) * (y1 - y0);
        if c.abs() < 1e-3 {
            return false;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

type Clips = std::collections::HashMap<u32, Vec<Vec<Vec<(f32, f32)>>>>;

impl Cover {
    fn covers(&self, clips: &Clips, q: &[(f32, f32); 4]) -> bool {
        let centre = ((q[0].0 + q[2].0) / 2.0, (q[0].1 + q[2].1) / 2.0);
        let pts = [q[0], q[1], q[2], q[3], centre];
        if pts.iter().any(|&(x, y)| x < self.bounds[0] || x > self.bounds[2] || y < self.bounds[1] || y > self.bounds[3]) {
            return false;
        }
        if !pts.iter().all(|&(x, y)| inside(&self.polys, self.evenodd, x, y)) {
            return false;
        }
        // a corner of the cover inside the box: a notch or a hole
        if self.polys.iter().flatten().any(|&(x, y)| in_quad(q, x, y)) {
            return false;
        }
        // drawn through a clip: the box must be inside every clip path too
        if let Some(c) = clips.get(&self.clip) {
            for polys in c {
                if !pts.iter().all(|&(x, y)| inside(polys, false, x, y)) {
                    return false;
                }
            }
        }
        true
    }
}

fn opaque(d: &Display, image: u32) -> bool {
    match d.images.get(image as usize).map(|i| &i.data) {
        Some(ImageData::Jpeg(_)) => true,
        Some(ImageData::Rgba(px)) => px.chunks_exact(4).all(|p| p[3] == 255),
        None => false,
    }
}

fn is_half(c: char) -> bool {
    (c as u32) < 0x2000 || ('｡'..='ﾟ').contains(&c)
}

/// Most cover checks made for one page; past this the rest of the page's
/// text is kept as it is (a page with that many shapes over its text is
/// not a redacted page).
const MAX_CHECKS: usize = 20_000_000;

/// Remove covered characters from `d`. Returns how many were removed and
/// adds them to `d.hidden_chars`.
///
/// `objects` gives the first item of each object drawn on the page (page
/// content, annotations), in order: a picture does not cover text of its
/// own object. `None`: every item counts as its own object.
pub fn remove_covered(d: &mut Display, objects: Option<&[usize]>) -> usize {
    let object_of = |i: usize| -> usize {
        match objects {
            Some(starts) => starts.partition_point(|&s| s <= i),
            None => i + 1,
        }
    };
    // covers, in drawing order
    let mut covers: Vec<Cover> = Vec::new();
    let mut opaque_cache: std::collections::HashMap<u32, bool> = Default::default();
    for (at, it) in d.items.iter().enumerate() {
        let (polys, evenodd, clip, picture) = match it {
            Item::Fill { path, evenodd, clip, mul: false, .. } => (polygons(path), *evenodd, *clip, false),
            Item::Image { image, m, clip, alpha, mul: false } if *alpha >= 1.0 => {
                let ok = *opaque_cache.entry(*image).or_insert_with(|| opaque(d, *image));
                if !ok {
                    continue;
                }
                let p = |u: f32, v: f32| (m[0] * u + m[2] * v + m[4], m[1] * u + m[3] * v + m[5]);
                (vec![vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)]], false, *clip, true)
            }
            _ => continue,
        };
        let Some(b) = bounds(&polys) else { continue };
        covers.push(Cover { at, picture, polys, evenodd, clip, bounds: b });
    }
    if covers.is_empty() {
        return 0;
    }
    let mut clips: Clips = Default::default();
    for c in &covers {
        if c.clip != 0 && !clips.contains_key(&c.clip) {
            let v = d.clips.get(c.clip as usize).map(|cl| cl.0.iter().map(polygons).collect()).unwrap_or_default();
            clips.insert(c.clip, v);
        }
    }
    let mut removed = 0;
    let mut checks = 0usize;
    let mut drop_items = Vec::new();
    for t in 0..d.items.len() {
        let Item::Text { x, y, angle, size, sx, text, xs, .. } = &d.items[t] else { continue };
        let later = &covers[covers.partition_point(|c| c.at <= t)..];
        let own = object_of(t);
        let later: Vec<&Cover> = later.iter().filter(|c| !(c.picture && object_of(c.at) == own)).collect();
        if later.is_empty() {
            continue;
        }
        let chars: Vec<char> = text.chars().collect();
        let (x, y, size, sx) = (*x, *y, *size, if *sx > 0.0 { *sx } else { 1.0 });
        let a = -angle.to_radians();
        let (ca, sa) = (a.cos(), a.sin());
        let page = |lx: f32, ly: f32| -> (f32, f32) {
            let lx = lx * sx;
            (x + lx * ca - ly * sa, y + lx * sa + ly * ca)
        };
        let pos: Vec<f32> = (0..chars.len()).map(|k| xs.get(k).copied().unwrap_or(k as f32 * size)).collect();
        let mut keep = vec![true; chars.len()];
        for (k, &c) in chars.iter().enumerate() {
            if checks > MAX_CHECKS {
                break;
            }
            let x0 = pos[k];
            let adv = match pos.get(k + 1) {
                Some(&n) if n > x0 => n - x0,
                _ => size * if is_half(c) { 0.5 } else { 1.0 },
            };
            let (l, r) = (x0 + adv * 0.15, x0 + adv * 0.85);
            let (top, bottom) = (-size * 0.7, 0.0);
            let q = [page(l, top), page(r, top), page(r, bottom), page(l, bottom)];
            checks += later.len();
            if later.iter().any(|cv| cv.covers(&clips, &q)) {
                keep[k] = false;
            }
        }
        let gone = keep.iter().filter(|&&k| !k).count();
        if gone == 0 {
            continue;
        }
        removed += chars.iter().zip(&keep).filter(|(c, &k)| !k && !c.is_whitespace()).count();
        if gone == chars.len() {
            drop_items.push(t);
            continue;
        }
        // keep the rest where it was: positions relative to the first kept one
        let first = keep.iter().position(|&k| k).unwrap_or(0);
        let shift = pos[first];
        let (nx, ny) = page(shift, 0.0);
        let new_text: String = chars.iter().zip(&keep).filter(|(_, &k)| k).map(|(c, _)| *c).collect();
        let new_xs: Vec<f32> = pos.iter().zip(&keep).filter(|(_, &k)| k).map(|(p, _)| p - shift).collect();
        if let Item::Text { x, y, text, xs, .. } = &mut d.items[t] {
            *x = nx;
            *y = ny;
            *text = new_text;
            *xs = new_xs;
        }
    }
    for t in drop_items.into_iter().rev() {
        d.items.remove(t);
    }
    d.hidden_chars += removed;
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(x: f32, y: f32, s: &str) -> Item {
        Item::Text {
            x,
            y,
            angle: 0.0,
            size: 400.0,
            sx: 1.0,
            face: String::new(),
            weight: 400,
            italic: false,
            underline: false,
            strike: false,
            vertical: false,
            color: 0,
            text: s.into(),
            xs: (0..s.chars().count()).map(|k| k as f32 * 400.0).collect(),
            clip: 0,
        }
    }

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32, mul: bool) -> Item {
        Item::Fill { path: Path::rect(x0, y0, x1, y1), color: 0, evenodd: false, clip: 0, mul }
    }

    #[test]
    fn a_box_over_part_of_a_run() {
        let mut d = Display::new(21000.0, 29700.0);
        d.items.push(text(1000.0, 2000.0, "氏名山田太郎"));
        // covers the 3rd to 6th characters (x 1800..3400)
        d.items.push(rect(1790.0, 1500.0, 3410.0, 2100.0, false));
        assert_eq!(remove_covered(&mut d, None), 4);
        assert_eq!(d.text(), "氏名");
        assert_eq!(d.hidden_chars, 4);
    }

    #[test]
    fn earlier_boxes_and_highlighters_do_not_hide() {
        let mut d = Display::new(21000.0, 29700.0);
        d.items.push(rect(0.0, 0.0, 9000.0, 9000.0, false));
        d.items.push(text(1000.0, 2000.0, "見える"));
        d.items.push(rect(0.0, 0.0, 9000.0, 9000.0, true));
        assert_eq!(remove_covered(&mut d, None), 0);
        assert_eq!(d.text(), "見える");
    }

    #[test]
    fn a_box_with_a_hole_does_not_hide() {
        let mut d = Display::new(21000.0, 29700.0);
        d.items.push(text(1000.0, 2000.0, "穴"));
        let mut p = Path::rect(0.0, 0.0, 9000.0, 9000.0);
        p.0.extend(Path::rect(900.0, 1000.0, 1600.0, 2200.0).0);
        d.items.push(Item::Fill { path: p, color: 0, evenodd: true, clip: 0, mul: false });
        assert_eq!(remove_covered(&mut d, None), 0);
    }

    #[test]
    fn a_partly_transparent_picture_does_not_hide() {
        use crate::gfx::Image;
        let mut d = Display::new(21000.0, 29700.0);
        d.items.push(text(1000.0, 2000.0, "AB"));
        d.images.push(Image { w: 1, h: 1, data: ImageData::Rgba(vec![0, 0, 0, 128]) });
        d.items.push(Item::Image { image: 0, m: [5000.0, 0.0, 0.0, 5000.0, 0.0, 0.0], clip: 0, alpha: 1.0, mul: false });
        assert_eq!(remove_covered(&mut d, None), 0);
        d.images[0] = Image { w: 1, h: 1, data: ImageData::Rgba(vec![0, 0, 0, 255]) };
        assert_eq!(remove_covered(&mut d, None), 2);
        assert!(d.items.iter().all(|i| !matches!(i, Item::Text { .. })));
    }
}
