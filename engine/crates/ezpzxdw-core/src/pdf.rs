//! PDF export: each page is the picture the viewer draws (so it looks the
//! same as on screen) with an invisible text layer on top, so the text can
//! still be searched, selected and copied.
//!
//! No fonts are embedded: the text layer uses render mode 3 (invisible),
//! `Identity-H` codes and a `ToUnicode` map.

use crate::gfx::{Display, Item};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// One rendered page: JPEG bytes and the pixel size.
pub struct PageImage {
    pub jpeg: Vec<u8>,
    pub px_w: u32,
    pub px_h: u32,
}

/// 1/100 mm → points
fn pt(v: f32) -> f32 {
    v * 72.0 / 2540.0
}

fn num(v: f32) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn pdf_text_utf16(s: &str) -> String {
    let mut h = String::from("<FEFF");
    for u in s.encode_utf16() {
        let _ = write!(h, "{u:04X}");
    }
    h.push('>');
    h
}

fn is_half(c: char) -> bool {
    (c as u32) < 0x2000 || ('｡'..='ﾟ').contains(&c)
}

fn stream(dict: String, data: &[u8]) -> Vec<u8> {
    let mut o = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    o.extend_from_slice(data);
    o.extend_from_slice(b"\nendstream");
    o
}

pub fn to_pdf(pages: &[Display], images: &[PageImage], title: &str) -> Vec<u8> {
    let mut codes: BTreeMap<char, u16> = BTreeMap::new();
    for p in pages {
        for it in &p.items {
            if let Item::Text { text, .. } = it {
                for c in text.chars() {
                    let n = codes.len() as u16 + 1;
                    if n < 0xffff {
                        codes.entry(c).or_insert(n);
                    }
                }
            }
        }
    }
    let mut objs: Vec<Vec<u8>> = Vec::new();
    let mut add = |o: Vec<u8>| -> usize {
        objs.push(o);
        objs.len()
    };
    for _ in 0..7 {
        add(Vec::new());
    }
    let mut kids = Vec::new();
    for (i, p) in pages.iter().enumerate() {
        let (w, h) = (pt(p.w), pt(p.h));
        let img = images.get(i);
        let mut content = String::new();
        if img.is_some() {
            let _ = writeln!(content, "q {} 0 0 {} 0 0 cm /Im0 Do Q", num(w), num(h));
        }
        content.push_str("BT 3 Tr\n");
        let mut cur = -1.0f32;
        for it in &p.items {
            let Item::Text { x, y, angle, size, text, xs, vertical, .. } = it else { continue };
            let size = pt(*size).max(0.5);
            if (size - cur).abs() > 0.001 {
                let _ = writeln!(content, "/F1 {} Tf", num(size));
                cur = size;
            }
            let a = angle.to_radians();
            let (ca, sa) = (a.cos(), a.sin());
            for (k, c) in text.chars().enumerate() {
                let Some(code) = codes.get(&c) else { continue };
                let off = pt(xs.get(k).copied().unwrap_or(0.0));
                // along the baseline (page y grows downwards, PDF upwards)
                let (gx, gy) = if *vertical {
                    (pt(*x) - size * 0.5, h - (pt(*y) + off))
                } else {
                    (pt(*x) + off * ca, h - (pt(*y) - off * sa))
                };
                let _ = writeln!(
                    content,
                    "{} {} {} {} {} {} Tm <{:04X}> Tj",
                    num(if *vertical { 1.0 } else { ca }),
                    num(if *vertical { 0.0 } else { sa }),
                    num(if *vertical { 0.0 } else { -sa }),
                    num(if *vertical { 1.0 } else { ca }),
                    num(gx),
                    num(gy),
                    code
                );
            }
        }
        content.push_str("ET\n");
        let content_obj = add(stream(String::new(), content.as_bytes()));
        let img_ref = img.map(|im| {
            add(stream(
                format!(
                    "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode",
                    im.px_w, im.px_h
                ),
                &im.jpeg,
            ))
        });
        let xobj = img_ref.map(|r| format!("/XObject << /Im0 {r} 0 R >> ")).unwrap_or_default();
        kids.push(add(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << {xobj}/Font << /F1 3 0 R >> >> /Contents {content_obj} 0 R >>",
                num(w),
                num(h)
            )
            .into_bytes(),
        ));
    }
    if pages.is_empty() {
        let c = add(stream(String::new(), b""));
        kids.push(add(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.28 841.89] /Contents {c} 0 R >>").into_bytes()));
    }
    objs[0] = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();
    objs[1] = format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids.iter().map(|k| format!("{k} 0 R")).collect::<Vec<_>>().join(" "),
        kids.len()
    )
    .into_bytes();
    objs[2] = b"<< /Type /Font /Subtype /Type0 /BaseFont /EZPZ-TextLayer /Encoding /Identity-H /DescendantFonts [4 0 R] /ToUnicode 5 0 R >>".to_vec();
    let mut w = String::from("[");
    for (c, code) in &codes {
        if is_half(*c) {
            let _ = write!(w, " {code} [500]");
        }
    }
    w.push_str(" ]");
    objs[3] = format!(
        "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /EZPZ-TextLayer /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor 6 0 R /DW 1000 /W {w} /CIDToGIDMap /Identity >>"
    )
    .into_bytes();
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /EZPZ-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let pairs: Vec<(&char, &u16)> = codes.iter().collect();
    for chunk in pairs.chunks(100) {
        let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
        for (c, code) in chunk {
            let mut u = String::new();
            let mut buf = [0u16; 2];
            for x in c.encode_utf16(&mut buf) {
                let _ = write!(u, "{x:04X}");
            }
            let _ = writeln!(cmap, "<{code:04X}> <{u}>");
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    objs[4] = stream(String::new(), cmap.as_bytes());
    objs[5] = b"<< /Type /FontDescriptor /FontName /EZPZ-TextLayer /Flags 4 /FontBBox [0 -120 1000 880] /ItalicAngle 0 /Ascent 880 /Descent -120 /CapHeight 700 /StemV 80 >>".to_vec();
    objs[6] = format!("<< /Title {} /Producer (EZPZ File XDW) /Creator (EZPZ File XDW) >>", pdf_text_utf16(title)).into_bytes();

    let mut out: Vec<u8> = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R /Info 7 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_pdf_is_well_formed() {
        let mut d = Display::new(21000.0, 29700.0);
        d.items.push(Item::Text {
            x: 2000.0,
            y: 3000.0,
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
            text: "文書a".into(),
            xs: vec![0.0, 400.0, 800.0],
            clip: 0,
        });
        let pdf = to_pdf(&[d], &[], "テスト");
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.contains("/Count 1"));
        let xref_at: usize = s.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(pdf[xref_at..].starts_with(b"xref"));
    }
}
