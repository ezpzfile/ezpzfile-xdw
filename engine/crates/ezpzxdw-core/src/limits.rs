//! Resource limits for untrusted files.
//!
//! A `.xdw` carries its own sizes and counts: how large a compressed block
//! becomes once expanded, how many pixels a picture has, how many points a
//! line has. A crafted file can claim far more than it holds (a few hundred
//! bytes that say "4 GB once expanded", a 10,000 × 10,000 picture made of one
//! byte), and a reader that trusts those numbers can be made to allocate
//! gigabytes or run for minutes.
//!
//! The reader therefore checks every count against the bytes that are really
//! there, and stops at the limits below. The defaults are far above anything
//! a real DocuWorks document needs (the public samples use a few percent of
//! them) while keeping a browser tab safe. Embedders can tighten them with
//! [`crate::doc::Document::open_with`] or the wasm `XdwDoc.openWith`.
//!
//! Work is counted per scope: opening a document is one, drawing a page is
//! another. Past a limit the call fails with [`Error::TooLarge`]
//! ("too large or damaged").

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Limits {
    /// Working memory for opening the document, or for drawing one page:
    /// expanded blocks and decoded pictures together, in bytes.
    pub max_bytes: usize,
    /// One compressed block (properties, drawing, picture) expanded, in bytes.
    pub max_expand: usize,
    /// A compressed block may expand to at most this many times its stored
    /// size (plus 64 KiB).
    pub max_ratio: usize,
    /// Most pages read; a document with more is refused.
    pub max_pages: usize,
    /// Most pixels in one picture.
    pub max_image_pixels: usize,
    /// Most drawing records played for one page.
    pub max_records: usize,
    /// Most path points (and text characters) drawn on one page.
    pub max_points: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_bytes: 768 << 20,
            max_expand: 256 << 20,
            max_ratio: 1000,
            max_pages: 20_000,
            max_image_pixels: 100_000_000,
            max_records: 1_000_000,
            max_points: 5_000_000,
        }
    }
}

impl Limits {
    /// Parse limits from JSON (`{"maxPages": 500, …}`); missing keys keep
    /// their defaults, zero means "use the default".
    pub fn from_json(s: &str) -> Limits {
        let d = Limits::default();
        let mut l = d;
        let body = s.trim().trim_start_matches('{').trim_end_matches('}');
        for pair in body.split(',') {
            let Some((k, v)) = pair.split_once(':') else { continue };
            let k = k.trim().trim_matches('"');
            let Ok(v) = v.trim().parse::<f64>() else { continue };
            if !(v.is_finite() && v >= 1.0) {
                continue;
            }
            let v = v.min(usize::MAX as f64) as usize;
            match k {
                "maxBytes" => l.max_bytes = v,
                "maxExpand" => l.max_expand = v,
                "maxRatio" => l.max_ratio = v,
                "maxPages" => l.max_pages = v,
                "maxImagePixels" => l.max_image_pixels = v,
                "maxRecords" => l.max_records = v,
                "maxPoints" => l.max_points = v,
                _ => {}
            }
        }
        l
    }

    /// Largest expanded size accepted for `stored` compressed bytes.
    pub fn expand_cap(&self, stored: usize) -> usize {
        stored.saturating_mul(self.max_ratio).saturating_add(64 << 10).min(self.max_expand)
    }
}

/// What one scope has used so far.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub bytes: usize,
    pub largest_expand: usize,
    pub largest_ratio: usize,
    pub largest_image: usize,
    pub records: usize,
    pub points: usize,
}

struct Scope {
    limits: Limits,
    used: Usage,
    over: Option<String>,
}

thread_local! {
    static SCOPES: RefCell<Vec<Scope>> = const { RefCell::new(Vec::new()) };
    static LAST: RefCell<Usage> = RefCell::new(Usage::default());
}

/// Run `f` with `limits` counted from zero. If anything inside went past a
/// limit, the result is [`Error::TooLarge`] even when `f` itself carried on.
pub fn scope<T>(limits: &Limits, f: impl FnOnce() -> Result<T>) -> Result<T> {
    SCOPES.with(|s| s.borrow_mut().push(Scope { limits: *limits, used: Usage::default(), over: None }));
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            SCOPES.with(|s| {
                if let Some(sc) = s.borrow_mut().pop() {
                    LAST.with(|l| *l.borrow_mut() = sc.used);
                }
            });
        }
    }
    let pop = Pop;
    let r = f();
    let over = SCOPES.with(|s| s.borrow().last().and_then(|sc| sc.over.clone()));
    drop(pop);
    match over {
        Some(why) => Err(Error::TooLarge(why)),
        None => r,
    }
}

/// What the last finished scope on this thread used (for measuring).
pub fn last_usage() -> Usage {
    LAST.with(|l| *l.borrow())
}

/// The limits in effect (the defaults outside any scope).
pub fn current() -> Limits {
    SCOPES.with(|s| s.borrow().last().map(|sc| sc.limits)).unwrap_or_default()
}

/// Has the current scope gone past a limit? Long loops stop early then.
pub fn over() -> bool {
    SCOPES.with(|s| s.borrow().last().map(|sc| sc.over.is_some()).unwrap_or(false))
}

fn with<R>(f: impl FnOnce(&mut Scope) -> R) -> Option<R> {
    SCOPES.with(|s| s.borrow_mut().last_mut().map(f))
}

fn fail(sc: &mut Scope, why: String) -> bool {
    if sc.over.is_none() {
        sc.over = Some(why);
    }
    false
}

/// May `stored` compressed bytes be expanded to `n`? Counts `n` against the
/// scope's memory when so.
pub(crate) fn expand(stored: usize, n: usize) -> Result<()> {
    let l = current();
    let cap = l.expand_cap(stored);
    if n > cap {
        let why = if n > l.max_expand {
            format!("a compressed block says it expands to {n} bytes (limit maxExpand {})", l.max_expand)
        } else {
            format!("a compressed block of {stored} bytes says it expands to {n} bytes (limit maxRatio {})", l.max_ratio)
        };
        with(|sc| fail(sc, why.clone()));
        return Err(Error::TooLarge(why));
    }
    let ok = with(|sc| {
        sc.used.largest_expand = sc.used.largest_expand.max(n);
        sc.used.largest_ratio = sc.used.largest_ratio.max(n / stored.max(1));
        charge_in(sc, n, "expanded blocks")
    })
    .unwrap_or(true);
    if ok {
        Ok(())
    } else {
        Err(Error::TooLarge("expanded data past maxBytes".into()))
    }
}

fn charge_in(sc: &mut Scope, n: usize, what: &str) -> bool {
    sc.used.bytes = sc.used.bytes.saturating_add(n);
    if sc.used.bytes > sc.limits.max_bytes {
        let why = format!("{what} need more than maxBytes ({} bytes)", sc.limits.max_bytes);
        return fail(sc, why);
    }
    true
}

/// Is a picture of `w` × `h` pixels within `max_image_pixels`? (Pictures
/// the browser decodes, such as JPEG, are checked this way too.)
pub(crate) fn image_size(w: usize, h: usize) -> bool {
    let px = w.saturating_mul(h);
    let l = current();
    if px > l.max_image_pixels {
        with(|sc| fail(sc, format!("a picture of {w} × {h} pixels (limit maxImagePixels {})", l.max_image_pixels)));
        // outside a scope the picture is still refused
        return false;
    }
    with(|sc| sc.used.largest_image = sc.used.largest_image.max(px));
    true
}

/// May a picture of `w` × `h` pixels be decoded here (4 bytes each)?
pub(crate) fn image(w: usize, h: usize) -> bool {
    image_size(w, h) && charge(w.saturating_mul(h).saturating_mul(4))
}

/// Count `n` more bytes of working memory.
pub(crate) fn charge(n: usize) -> bool {
    with(|sc| charge_in(sc, n, "the page")).unwrap_or(true)
}

/// One more drawing record; false once past `max_records`.
pub(crate) fn record() -> bool {
    with(|sc| {
        sc.used.records += 1;
        if sc.used.records > sc.limits.max_records {
            let why = format!("more than {} drawing records on one page (limit maxRecords)", sc.limits.max_records);
            return fail(sc, why);
        }
        sc.over.is_none()
    })
    .unwrap_or(true)
}

/// `n` more points or characters; false once past `max_points`.
pub(crate) fn points(n: usize) -> bool {
    with(|sc| {
        sc.used.points = sc.used.points.saturating_add(n);
        if sc.used.points > sc.limits.max_points {
            let why = format!("more than {} points on one page (limit maxPoints)", sc.limits.max_points);
            return fail(sc, why);
        }
        sc.over.is_none()
    })
    .unwrap_or(true)
}

/// One more drawn item with `n` points or characters: counts the points
/// and the memory it takes. False once past a limit (the item is dropped).
pub(crate) fn item(n: usize) -> bool {
    let bytes = std::mem::size_of::<crate::gfx::Item>() + n * std::mem::size_of::<crate::gfx::Seg>();
    points(n) && charge(bytes)
}

/// How many items of `each` bytes fit in `avail` bytes, at most `n`: a count
/// read from a file never asks for more than the data can hold.
pub(crate) fn fit(n: usize, avail: usize, each: usize) -> usize {
    n.min(avail / each.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json() {
        let l = Limits::from_json(r#"{"maxPages": 5, "maxBytes": 0, "nope": 3, "maxPoints": -1}"#);
        assert_eq!(l.max_pages, 5);
        assert_eq!(l.max_bytes, Limits::default().max_bytes);
        assert_eq!(l.max_points, Limits::default().max_points);
    }

    #[test]
    fn scope_reports_going_over() {
        let l = Limits { max_records: 3, ..Default::default() };
        let r = scope(&l, || {
            for _ in 0..10 {
                if !record() {
                    break;
                }
            }
            Ok(())
        });
        assert!(matches!(r, Err(Error::TooLarge(_))));
        assert!(scope(&l, || Ok(record())).unwrap());
    }
}
