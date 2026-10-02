//! Browser bindings: open a `.xdw`, draw its pages, edit annotations and
//! pages, save as `.xdw` or PDF.

use ezpzxdw_core::doc::Document;
use ezpzxdw_core::edit::Shape;
use ezpzxdw_core::gfx::{Display, ImageData, Item, Path, Seg};
use ezpzxdw_core::props::Record;
use std::collections::HashMap;
use std::fmt::Write as _;
use wasm_bindgen::prelude::*;

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

fn n(v: f32) -> String {
    if v.is_finite() {
        let s = format!("{:.1}", v);
        s.strip_suffix(".0").map(|x| x.to_string()).unwrap_or(s)
    } else {
        "0".into()
    }
}

/// A path as numbers: 0 x y = move, 1 x y = line, 2 x1 y1 x2 y2 x y = curve, 3 = close.
fn path_json(p: &Path, out: &mut String) {
    out.push('[');
    let mut first = true;
    let mut put = |out: &mut String, v: String| {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&v);
    };
    for s in &p.0 {
        match *s {
            Seg::M(x, y) => {
                put(out, "0".into());
                put(out, n(x));
                put(out, n(y));
            }
            Seg::L(x, y) => {
                put(out, "1".into());
                put(out, n(x));
                put(out, n(y));
            }
            Seg::C(a, b, c, d, e, f) => {
                put(out, "2".into());
                for v in [a, b, c, d, e, f] {
                    put(out, n(v));
                }
            }
            Seg::Z => put(out, "3".into()),
        }
    }
    out.push(']');
}

fn display_json(d: &Display) -> String {
    let mut o = String::with_capacity(d.items.len() * 64);
    let _ = write!(o, "{{\"w\":{},\"h\":{},\"clips\":[", n(d.w), n(d.h));
    for (k, c) in d.clips.iter().enumerate() {
        if k > 0 {
            o.push(',');
        }
        o.push('[');
        for (j, p) in c.0.iter().enumerate() {
            if j > 0 {
                o.push(',');
            }
            path_json(p, &mut o);
        }
        o.push(']');
    }
    o.push_str("],\"images\":[");
    for (k, im) in d.images.iter().enumerate() {
        if k > 0 {
            o.push(',');
        }
        let kind = match im.data {
            ImageData::Jpeg(_) => "jpeg",
            ImageData::Rgba(_) => "rgba",
        };
        let _ = write!(o, "{{\"w\":{},\"h\":{},\"k\":\"{kind}\"}}", im.w, im.h);
    }
    o.push_str("],\"items\":[");
    for (k, it) in d.items.iter().enumerate() {
        if k > 0 {
            o.push(',');
        }
        match it {
            Item::Fill { path, color, evenodd, clip, mul } => {
                o.push_str("[\"f\",");
                path_json(path, &mut o);
                let _ = write!(o, ",{color},{},{clip},{}]", *evenodd as u8, *mul as u8);
            }
            Item::Stroke { path, color, width, dash, cap, join, clip } => {
                o.push_str("[\"s\",");
                path_json(path, &mut o);
                let dash: Vec<String> = dash.iter().map(|v| n(*v)).collect();
                let _ = write!(o, ",{color},{},[{}],{cap},{join},{clip}]", n(*width), dash.join(","));
            }
            Item::Text { x, y, angle, size, sx, face, weight, italic, underline, strike, vertical, color, text, xs, clip } => {
                let xs: Vec<String> = xs.iter().map(|v| n(*v)).collect();
                let _ = write!(
                    o,
                    "[\"t\",{},{},{},{},{},{},{weight},{},{},{},{},{color},{},[{}],{clip}]",
                    n(*x),
                    n(*y),
                    n(*angle),
                    n(*size),
                    sx,
                    serde_json::to_string(face).unwrap_or_default(),
                    *italic as u8,
                    *underline as u8,
                    *strike as u8,
                    *vertical as u8,
                    serde_json::to_string(text).unwrap_or_default(),
                    xs.join(",")
                );
            }
            Item::Image { image, m, clip, alpha, mul } => {
                let m: Vec<String> = m.iter().map(|v| format!("{v:.2}")).collect();
                let _ = write!(o, "[\"i\",{image},[{}],{clip},{alpha},{}]", m.join(","), *mul as u8);
            }
        }
    }
    o.push_str("]}");
    o
}

#[wasm_bindgen]
pub struct XdwDoc {
    doc: Document,
    cache: HashMap<usize, Display>,
    undo: Vec<Vec<Record>>,
    redo: Vec<Vec<Record>>,
    saved_state: Vec<Record>,
}

#[wasm_bindgen]
impl XdwDoc {
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: Vec<u8>) -> Result<XdwDoc, JsError> {
        let doc = Document::open(bytes).map_err(err)?;
        let saved_state = doc.records.clone();
        Ok(XdwDoc { doc, cache: HashMap::new(), undo: Vec::new(), redo: Vec::new(), saved_state })
    }

    #[wasm_bindgen(js_name = pageCount)]
    pub fn page_count(&self) -> usize {
        self.doc.pages.len()
    }

    /// Pages and the objects on them (JSON).
    pub fn pages(&self) -> String {
        let mut v = Vec::new();
        for (pi, p) in self.doc.pages.iter().enumerate() {
            let objs: Vec<serde_json::Value> = p
                .objects
                .iter()
                .enumerate()
                .map(|(oi, o)| {
                    serde_json::json!({
                        "kind": o.kind_name, "x": o.x, "y": o.y, "w": o.w, "h": o.h,
                        "rotation": o.rotation, "text": o.text,
                        "editable": self.doc.editable(pi, oi),
                        "seeThrough": self.doc.picture_see_through(pi, oi),
                        "signature": self.doc.signature_of(pi, oi),
                        "shape": self.doc.shape_of(pi, oi),
                    })
                })
                .collect();
            v.push(serde_json::json!({"w": p.w, "h": p.h, "objects": objs}));
        }
        serde_json::to_string(&v).unwrap_or_default()
    }

    /// What page `n` looks like (JSON display list; pictures via `image`).
    pub fn render(&mut self, page: usize) -> Result<String, JsError> {
        if !self.cache.contains_key(&page) {
            let d = self.doc.render(page).map_err(err)?;
            self.cache.insert(page, d);
        }
        Ok(display_json(&self.cache[&page]))
    }

    /// Picture `idx` of page `n` as JPEG bytes or RGBA pixels (see `render`).
    pub fn image(&self, page: usize, idx: usize) -> Vec<u8> {
        self.cache
            .get(&page)
            .and_then(|d| d.images.get(idx))
            .map(|im| match &im.data {
                ImageData::Jpeg(b) | ImageData::Rgba(b) => b.clone(),
            })
            .unwrap_or_default()
    }

    /// Drop a page's drawing from memory (after it has been painted).
    #[wasm_bindgen(js_name = forget)]
    pub fn forget(&mut self, page: usize) {
        self.cache.remove(&page);
    }

    pub fn text(&mut self, page: usize) -> Result<String, JsError> {
        if !self.cache.contains_key(&page) {
            let d = self.doc.render(page).map_err(err)?;
            self.cache.insert(page, d);
        }
        Ok(self.cache[&page].text())
    }

    fn before(&mut self) {
        self.undo.push(self.doc.records.clone());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn after(&mut self) {
        self.cache.clear();
    }

    fn fail(&mut self, e: impl std::fmt::Display) -> JsError {
        if let Some(r) = self.undo.pop() {
            self.doc.records = r;
            self.doc.refresh();
        }
        err(e)
    }

    /// Add an annotation; `shape` is JSON (see ezpzxdw_core::edit::Shape).
    /// Returns its object number on the page.
    #[wasm_bindgen(js_name = addAnnotation)]
    pub fn add_annotation(&mut self, page: usize, x: f64, y: f64, w: f64, h: f64, shape: &str) -> Result<usize, JsError> {
        let s: Shape = serde_json::from_str(shape).map_err(err)?;
        self.before();
        let r = self.doc.add_annotation(page, x, y, w, h, &s).map_err(|e| self.fail(e))?;
        self.after();
        Ok(r)
    }

    #[wasm_bindgen(js_name = changeAnnotation)]
    pub fn change_annotation(&mut self, page: usize, obj: usize, x: f64, y: f64, w: f64, h: f64, shape: &str) -> Result<(), JsError> {
        let s: Shape = serde_json::from_str(shape).map_err(err)?;
        self.before();
        self.doc.change_annotation(page, obj, x, y, w, h, &s).map_err(|e| self.fail(e))?;
        self.after();
        Ok(())
    }

    #[wasm_bindgen(js_name = moveObject)]
    pub fn move_object(&mut self, page: usize, obj: usize, x: f64, y: f64) -> Result<(), JsError> {
        self.before();
        self.doc.move_object(page, obj, x, y).map_err(|e| self.fail(e))?;
        self.after();
        Ok(())
    }

    #[wasm_bindgen(js_name = deleteObject)]
    pub fn delete_object(&mut self, page: usize, obj: usize) -> Result<(), JsError> {
        self.before();
        self.doc.delete_object(page, obj).map_err(|e| self.fail(e))?;
        self.after();
        Ok(())
    }

    #[wasm_bindgen(js_name = rotatePage)]
    pub fn rotate_page(&mut self, page: usize, quarters: i32) -> Result<(), JsError> {
        self.before();
        self.doc.rotate_page(page, quarters).map_err(|e| self.fail(e))?;
        self.after();
        Ok(())
    }

    #[wasm_bindgen(js_name = deletePage)]
    pub fn delete_page(&mut self, page: usize) -> Result<(), JsError> {
        self.before();
        self.doc.delete_page(page).map_err(|e| self.fail(e))?;
        self.after();
        Ok(())
    }

    #[wasm_bindgen(js_name = movePage)]
    pub fn move_page(&mut self, from: usize, to: usize) -> Result<(), JsError> {
        self.before();
        self.doc.move_page(from, to).map_err(|e| self.fail(e))?;
        self.after();
        Ok(())
    }

    /// Add a picture annotation (RGBA pixels, `pw` × `ph`). `see_through`:
    /// the white parts show the page, also in DocuWorks.
    #[wasm_bindgen(js_name = addPicture)]
    #[allow(clippy::too_many_arguments)]
    pub fn add_picture(&mut self, page: usize, x: f64, y: f64, w: f64, h: f64, rgba: &[u8], pw: u32, ph: u32, see_through: bool) -> Result<usize, JsError> {
        self.before();
        let r = self.doc.add_picture(page, x, y, w, h, rgba, pw, ph, see_through).map_err(|e| self.fail(e))?;
        self.after();
        Ok(r)
    }

    /// Resize a picture annotation, or switch it to see-through or back.
    #[wasm_bindgen(js_name = setPicture)]
    #[allow(clippy::too_many_arguments)]
    pub fn set_picture(&mut self, page: usize, obj: usize, x: f64, y: f64, w: f64, h: f64, see_through: bool) -> Result<(), JsError> {
        self.before();
        self.doc.set_picture(page, obj, x, y, w, h, see_through).map_err(|e| self.fail(e))?;
        self.after();
        Ok(())
    }

    /// Insert a blank page `w` × `h` (1/100 mm) at position `at`.
    #[wasm_bindgen(js_name = insertBlankPage)]
    pub fn insert_blank_page(&mut self, at: usize, w: f64, h: f64) -> Result<usize, JsError> {
        self.before();
        let r = self.doc.insert_blank_page(at, w, h).map_err(|e| self.fail(e))?;
        self.after();
        Ok(r)
    }

    /// Insert a page `w` × `h` showing a JPEG (`pxw` × `pxh` pixels), as
    /// large as fits; `thumb` is a small RGBA picture of it (`tw` × `th`,
    /// may be empty).
    #[wasm_bindgen(js_name = insertImagePage)]
    pub fn insert_image_page(&mut self, at: usize, w: f64, h: f64, jpeg: &[u8], pxw: u32, pxh: u32, thumb: &[u8], tw: u32, th: u32) -> Result<usize, JsError> {
        self.before();
        let j = ezpzxdw_core::pages::Jpeg { data: jpeg, w: pxw, h: pxh };
        let t = ezpzxdw_core::pages::Thumb { rgba: thumb, w: tw, h: th };
        let r = self.doc.insert_image_page(at, w, h, &j, None, (!thumb.is_empty()).then_some(&t)).map_err(|e| self.fail(e))?;
        self.after();
        Ok(r)
    }

    /// Copy pages of another DocuWorks file (`pages`: JSON list of page
    /// numbers from 0, or empty for all) to position `at`.
    #[wasm_bindgen(js_name = insertPagesFrom)]
    pub fn insert_pages_from(&mut self, at: usize, bytes: Vec<u8>, pages: &str) -> Result<usize, JsError> {
        let other = Document::open(bytes).map_err(err)?;
        let which: Vec<usize> = if pages.trim().is_empty() { (0..other.pages.len()).collect() } else { serde_json::from_str(pages).map_err(err)? };
        self.before();
        let r = self.doc.insert_pages_from(at, &other, &which).map_err(|e| self.fail(e))?;
        self.after();
        Ok(r)
    }

    /// The binder's documents (JSON {name, docs: [{name, first_page,
    /// pages}]}), or "null" for a plain document.
    pub fn binder(&self) -> String {
        match self.doc.binder_name() {
            Some(name) => serde_json::json!({"name": name, "docs": self.doc.binder_docs()}).to_string(),
            None => "null".into(),
        }
    }

    #[wasm_bindgen(js_name = renameBinderDoc)]
    pub fn rename_binder_doc(&mut self, k: usize, name: &str) -> Result<(), JsError> {
        self.before();
        self.doc.rename_binder_doc(k, name).map_err(|e| self.fail(e))?;
        self.after();
        Ok(())
    }

    #[wasm_bindgen(js_name = deleteBinderDoc)]
    pub fn delete_binder_doc(&mut self, k: usize) -> Result<(), JsError> {
        self.before();
        self.doc.delete_binder_doc(k).map_err(|e| self.fail(e))?;
        self.after();
        Ok(())
    }

    #[wasm_bindgen(js_name = moveBinderDoc)]
    pub fn move_binder_doc(&mut self, from: usize, to: usize) -> Result<(), JsError> {
        self.before();
        self.doc.move_binder_doc(from, to).map_err(|e| self.fail(e))?;
        self.after();
        Ok(())
    }

    /// Add another file's documents to this binder at position `at`.
    #[wasm_bindgen(js_name = addBinderDocs)]
    pub fn add_binder_docs(&mut self, at: usize, bytes: Vec<u8>, name: &str) -> Result<usize, JsError> {
        let other = Document::open(bytes).map_err(err)?;
        self.before();
        let r = self.doc.add_binder_docs(at, &other, name).map_err(|e| self.fail(e))?;
        self.after();
        Ok(r)
    }

    /// Does the document carry a signature (editing makes it invalid)?
    #[wasm_bindgen(js_name = isSigned)]
    pub fn is_signed(&self) -> bool {
        self.doc.is_signed()
    }

    /// Today's date as a date stamp shows it ('26.10.01).
    #[wasm_bindgen(js_name = stampDate)]
    pub fn stamp_date(year: i32, month: u32, day: u32) -> String {
        ezpzxdw_core::edit::stamp_date(year, month, day)
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(r) => {
                self.redo.push(std::mem::replace(&mut self.doc.records, r));
                self.doc.refresh();
                self.after();
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(r) => {
                self.undo.push(std::mem::replace(&mut self.doc.records, r));
                self.doc.refresh();
                self.after();
                true
            }
            None => false,
        }
    }

    #[wasm_bindgen(js_name = canUndo)]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[wasm_bindgen(js_name = canRedo)]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Changed since opening or the last save?
    pub fn dirty(&self) -> bool {
        self.doc.records != self.saved_state
    }

    /// The document as `.xdw` (checked by reading it back).
    pub fn save(&mut self) -> Result<Vec<u8>, JsError> {
        let b = self.doc.save().map_err(err)?;
        self.saved_state = self.doc.records.clone();
        Ok(b)
    }

    /// PDF from page pictures: `jpegs` concatenated, `lens` their sizes,
    /// `dims` [w, h] in pixels per page.
    #[wasm_bindgen(js_name = toPdf)]
    pub fn to_pdf(&mut self, jpegs: &[u8], lens: &[u32], dims: &[u32], title: &str) -> Result<Vec<u8>, JsError> {
        let mut pages = Vec::new();
        for k in 0..self.doc.pages.len() {
            pages.push(self.doc.render(k).map_err(err)?);
        }
        let mut imgs = Vec::new();
        let mut at = 0usize;
        for (k, &l) in lens.iter().enumerate() {
            let l = l as usize;
            imgs.push(ezpzxdw_core::pdf::PageImage {
                jpeg: jpegs.get(at..at + l).unwrap_or(&[]).to_vec(),
                px_w: dims.get(2 * k).copied().unwrap_or(1),
                px_h: dims.get(2 * k + 1).copied().unwrap_or(1),
            });
            at += l;
        }
        Ok(ezpzxdw_core::pdf::to_pdf(&pages, &imgs, title))
    }

    /// Box size a text annotation needs (JSON [w, h], 1/100 mm).
    #[wasm_bindgen(js_name = textBox)]
    pub fn text_box(text: &str, size: f64) -> String {
        let (w, h) = ezpzxdw_core::edit::text_box(text, size);
        format!("[{w},{h}]")
    }
}
