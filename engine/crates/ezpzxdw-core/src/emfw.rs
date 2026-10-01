//! Writing small EMF drawings: the ready-made pictures annotations carry.
//!
//! DocuWorks shows an annotation by playing the drawing stored with it
//! (it does not redraw from the annotation's settings), so every
//! annotation we add or change gets a drawing made here. Like DocuWorks'
//! own, the drawing uses 300 units per inch and sets its window to the
//! annotation's size; the viewer fits that window to the annotation box.

/// 1/100 mm per drawing unit (1/300 inch).
pub const UNIT: f64 = 2540.0 / 300.0;

pub fn units(v_100mm: f64) -> i32 {
    (v_100mm / UNIT).round() as i32
}

/// COLORREF from 0xRRGGBB.
pub fn colorref(rgb: u32) -> u32 {
    (rgb & 0xff) << 16 | (rgb & 0xff00) | (rgb >> 16) & 0xff
}

pub struct Emf {
    recs: Vec<u8>,
    count: u32,
    handles: u32,
    w: i32,
    h: i32,
    /// 1/100 mm per unit
    unit: f64,
    /// drawn area in units (for the header's bounds), if not the whole
    bounds: Option<[i32; 4]>,
}

/// 1/100 mm per device pixel of a page drawing (600 dpi, as the DocuWorks
/// printer driver writes them).
pub const PAGE_UNIT: f64 = 2540.0 / 600.0;

impl Emf {
    /// A drawing `w` × `h` units.
    pub fn new(w: i32, h: i32) -> Emf {
        let mut e = Emf { recs: Vec::new(), count: 0, handles: 1, w: w.max(1), h: h.max(1), unit: UNIT, bounds: None };
        e.rec(9, &[e.w, e.h]); // SETWINDOWEXTEX
        e
    }

    /// A page drawing `w` × `h` device pixels at 600 dpi, like the
    /// DocuWorks printer driver's: no window, units are pixels.
    pub fn page(w: i32, h: i32) -> Emf {
        Emf { recs: Vec::new(), count: 0, handles: 1, w: w.max(1), h: h.max(1), unit: PAGE_UNIT, bounds: None }
    }

    /// SETSTRETCHBLTMODE (3 = COLORONCOLOR, 4 = HALFTONE).
    pub fn stretch_mode(&mut self, m: i32) {
        self.rec(21, &[m]);
    }

    fn comment(&mut self, data: &[u8]) {
        let mut p = (data.len() as u32).to_le_bytes().to_vec();
        p.extend_from_slice(data);
        self.raw(70, &p);
    }

    /// Draw the next picture of the page's picture list (DocuWorks' own
    /// records: `DWb` takes the next picture, `DWc` draws it like
    /// STRETCHDIBITS) from the whole `pw` × `ph` source into `dst`
    /// (x, y, w, h in units).
    pub fn next_picture(&mut self, pw: i32, ph: i32, dst: [i32; 4]) {
        self.comment(b"DWb\0");
        let [x, y, w, h] = dst;
        let mut d = b"DWc\0".to_vec();
        for v in [x, y, x + w - 1, y + h - 1, x, y, 0, 0, pw, ph, 0, 0x00cc_0020, w, h] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        self.comment(&d);
        let b = self.bounds.get_or_insert([x, y, x + w - 1, y + h - 1]);
        *b = [b[0].min(x), b[1].min(y), b[2].max(x + w - 1), b[3].max(y + h - 1)];
    }

    fn rec(&mut self, t: u32, ints: &[i32]) {
        self.raw(t, &ints.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>());
    }

    fn raw(&mut self, t: u32, payload: &[u8]) {
        let mut p = payload.to_vec();
        while p.len() % 4 != 0 {
            p.push(0);
        }
        self.recs.extend_from_slice(&t.to_le_bytes());
        self.recs.extend_from_slice(&((8 + p.len()) as u32).to_le_bytes());
        self.recs.extend_from_slice(&p);
        self.count += 1;
    }

    pub fn clip_to_box(&mut self) {
        self.rec(30, &[0, 0, self.w, self.h]); // INTERSECTCLIPRECT
    }

    /// Select a solid pen (width in units, 0 = none) and brush
    /// (None = hollow). Returns nothing; objects are freed at the end.
    pub fn pen_brush(&mut self, pen: Option<(u32, i32)>, brush: Option<u32>) {
        match pen {
            Some((rgb, width)) => {
                let h = self.handles;
                self.handles += 1;
                // EXTCREATEPEN: geometric solid pen, round caps
                let mut p = Vec::new();
                for v in [h, 0, 0, 0, 0] {
                    p.extend_from_slice(&v.to_le_bytes());
                }
                for v in [0x0001_0000u32, width.max(1) as u32, 0, colorref(rgb), 0, 0] {
                    p.extend_from_slice(&v.to_le_bytes());
                }
                self.raw(95, &p);
                self.rec(37, &[h as i32]);
            }
            None => self.rec(37, &[0x8000_0008u32 as i32]), // NULL_PEN
        }
        match brush {
            Some(rgb) => {
                let h = self.handles;
                self.handles += 1;
                self.rec(39, &[h as i32, 0, colorref(rgb) as i32, 0]);
                self.rec(37, &[h as i32]);
            }
            None => self.rec(37, &[0x8000_0005u32 as i32]), // NULL_BRUSH
        }
    }

    /// R2_MASKPEN (like a highlighter: the colour multiplies) or copy.
    pub fn mask_mode(&mut self, on: bool) {
        self.rec(20, &[if on { 9 } else { 13 }]);
    }

    /// A see-through coloured rectangle (ALPHABLEND of a one-pixel picture
    /// with a constant opacity 0-255).
    pub fn alpha_rect(&mut self, l: i32, t: i32, r: i32, b: i32, rgb: u32, alpha: u8) {
        let mut p = Vec::new();
        let put = |p: &mut Vec<u8>, v: i32| p.extend_from_slice(&v.to_le_bytes());
        for v in [l, t, r - 1, b - 1] {
            put(&mut p, v); // bounds
        }
        for v in [l, t, r - l, b - t] {
            put(&mut p, v); // destination
        }
        p.extend_from_slice(&[0, 0, alpha, 0]); // BLENDFUNCTION
        put(&mut p, 0); // xSrc
        put(&mut p, 0); // ySrc
        for f in [1f32, 0.0, 0.0, 1.0, 0.0, 0.0] {
            p.extend_from_slice(&f.to_le_bytes());
        }
        put(&mut p, 0); // BkColorSrc
        put(&mut p, 0); // UsageSrc (DIB_RGB_COLORS)
        let off_bmi = 8 + p.len() as i32 + 6 * 4;
        put(&mut p, off_bmi);
        put(&mut p, 40);
        put(&mut p, off_bmi + 40);
        put(&mut p, 4);
        put(&mut p, 1); // cxSrc
        put(&mut p, 1); // cySrc
        // BITMAPINFOHEADER 1x1, 32 bpp
        for v in [40i32, 1, 1] {
            put(&mut p, v);
        }
        p.extend_from_slice(&1u16.to_le_bytes());
        p.extend_from_slice(&32u16.to_le_bytes());
        for _ in 0..6 {
            put(&mut p, 0);
        }
        p.extend_from_slice(&[(rgb & 0xff) as u8, (rgb >> 8) as u8, (rgb >> 16) as u8, 0]);
        self.raw(114, &p);
    }

    pub fn rectangle(&mut self, l: i32, t: i32, r: i32, b: i32) {
        self.rec(43, &[l, t, r, b]);
    }

    pub fn ellipse(&mut self, l: i32, t: i32, r: i32, b: i32) {
        self.rec(42, &[l, t, r, b]);
    }

    pub fn polyline(&mut self, pts: &[(i32, i32)]) {
        let (mut l, mut t, mut r, mut b) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for &(x, y) in pts {
            l = l.min(x);
            t = t.min(y);
            r = r.max(x);
            b = b.max(y);
        }
        let mut v = vec![l, t, r, b, pts.len() as i32];
        for &(x, y) in pts {
            v.push(x);
            v.push(y);
        }
        self.rec(4, &v);
    }

    pub fn polygon(&mut self, pts: &[(i32, i32)]) {
        let (mut l, mut t, mut r, mut b) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for &(x, y) in pts {
            l = l.min(x);
            t = t.min(y);
            r = r.max(x);
            b = b.max(y);
        }
        let mut v = vec![l, t, r, b, pts.len() as i32];
        for &(x, y) in pts {
            v.push(x);
            v.push(y);
        }
        self.rec(3, &v);
    }

    /// Select a font: em height in units, weight (400 / 700), face.
    pub fn font(&mut self, em: i32, weight: i32, italic: bool, face: &str) {
        let h = self.handles;
        self.handles += 1;
        let mut p = Vec::new();
        p.extend_from_slice(&h.to_le_bytes());
        for v in [-em, 0, 0, 0, weight] {
            p.extend_from_slice(&v.to_le_bytes());
        }
        p.extend_from_slice(&[italic as u8, 0, 0, 128, 0, 0, 0, 0x31]);
        let mut name: Vec<u16> = face.encode_utf16().take(31).collect();
        name.resize(32, 0);
        for u in name {
            p.extend_from_slice(&u.to_le_bytes());
        }
        // elfFullName, elfStyle, elfScript (unused)
        p.resize(4 + 92 + 64 * 2 + 32 * 2 + 32 * 2, 0);
        self.raw(82, &p);
        self.rec(37, &[h as i32]);
    }

    pub fn text_color(&mut self, rgb: u32) {
        self.rec(24, &[colorref(rgb) as i32]);
        self.rec(18, &[1]); // transparent background
        self.rec(22, &[0]); // TA_TOP | TA_LEFT
    }

    /// One line of text at (x, y) (top-left), with each character's advance.
    pub fn text(&mut self, x: i32, y: i32, s: &str, adv: &[i32]) {
        let units: Vec<u16> = s.encode_utf16().collect();
        let n = units.len();
        // per UTF-16 unit advances (a surrogate pair: all on the first)
        let mut dx = Vec::with_capacity(n);
        for (k, ch) in s.chars().enumerate() {
            dx.push(adv.get(k).copied().unwrap_or(0));
            if ch.len_utf16() == 2 {
                dx.push(0);
            }
        }
        let width: i32 = dx.iter().sum();
        let off_string = 8 + 16 + 12 + 40; // record header, bounds, mode/scales, EMRTEXT
        let str_bytes = n * 2;
        let str_pad = (str_bytes + 3) / 4 * 4;
        let off_dx = off_string + str_pad;
        let mut p = Vec::new();
        for v in [x, y, x + width, y + adv.first().copied().unwrap_or(0)] {
            p.extend_from_slice(&v.to_le_bytes());
        }
        p.extend_from_slice(&1u32.to_le_bytes()); // GM_COMPATIBLE
        p.extend_from_slice(&0f32.to_le_bytes());
        p.extend_from_slice(&0f32.to_le_bytes());
        for v in [x, y] {
            p.extend_from_slice(&v.to_le_bytes());
        }
        p.extend_from_slice(&(n as u32).to_le_bytes());
        p.extend_from_slice(&(off_string as u32).to_le_bytes());
        p.extend_from_slice(&0u32.to_le_bytes()); // options
        for v in [0i32, 0, -1, -1] {
            p.extend_from_slice(&v.to_le_bytes());
        }
        p.extend_from_slice(&(off_dx as u32).to_le_bytes());
        for u in &units {
            p.extend_from_slice(&u.to_le_bytes());
        }
        p.resize(off_dx - 8, 0);
        for d in dx {
            p.extend_from_slice(&d.to_le_bytes());
        }
        self.raw(84, &p);
    }

    /// The finished EMF (`w_100mm` × `h_100mm` is the frame).
    pub fn finish(mut self, w_100mm: f64, h_100mm: f64) -> Vec<u8> {
        self.rec(14, &[0, 16, 20]); // EOF
        let header_len = 108usize;
        let total = header_len + self.recs.len();
        let mut h = Vec::with_capacity(total);
        let put = |h: &mut Vec<u8>, v: i32| h.extend_from_slice(&v.to_le_bytes());
        put(&mut h, 1);
        put(&mut h, header_len as i32);
        // bounds (device = drawing units)
        for v in self.bounds.unwrap_or([0, 0, self.w, self.h]) {
            put(&mut h, v);
        }
        // frame (0.01 mm)
        for v in [0, 0, w_100mm.round() as i32, h_100mm.round() as i32] {
            put(&mut h, v);
        }
        h.extend_from_slice(b" EMF");
        put(&mut h, 0x10000);
        put(&mut h, total as i32);
        put(&mut h, self.count as i32 + 1);
        h.extend_from_slice(&(self.handles as u16).to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes());
        put(&mut h, 0); // nDescription
        put(&mut h, 0); // offDescription
        put(&mut h, 0); // nPalEntries
        put(&mut h, self.w); // device size in units
        put(&mut h, self.h);
        put(&mut h, ((self.w as f64) * self.unit / 100.0).round().max(1.0) as i32); // millimetres
        put(&mut h, ((self.h as f64) * self.unit / 100.0).round().max(1.0) as i32);
        put(&mut h, 0); // cbPixelFormat
        put(&mut h, 0); // offPixelFormat
        put(&mut h, 0); // bOpenGL
        put(&mut h, ((self.w as f64) * self.unit * 10.0).round() as i32); // micrometres
        put(&mut h, ((self.h as f64) * self.unit * 10.0).round() as i32);
        h.extend_from_slice(&self.recs);
        h
    }
}
