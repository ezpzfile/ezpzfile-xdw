//! `ezpzxdw`: inspect DocuWorks files.

use ezpzxdw_core::container::{Body, Container};
use ezpzxdw_core::{props, tlv};
use std::io::Write;
use std::process::ExitCode;

const HELP: &str = r#"ezpzxdw: DocuWorks (.xdw) reader

USAGE:
  ezpzxdw info   <file>            segments, trailer, entries
  ezpzxdw tree   <file>            the object tree in the properties block
  ezpzxdw props  <file> [out]      expanded properties block (raw bytes)
  ezpzxdw entry  <file> <n> [out]  data of entry n (expanded when compressed)
  ezpzxdw check  <file>            check values and properties round trip
  ezpzxdw pages  <file>            pages and the objects on them
  ezpzxdw render <file> [page]     what drawing a page produces (summary)
  ezpzxdw text   <file>            text of every page
  ezpzxdw set-attr <in> <record> <tag> <value> <out>
                                 change one attribute (value: hex:…, u16:text, int:a,b, del)
  ezpzxdw edit   <in> <ops.json> <out>
                                 apply edits: [{"op":"add","page":0,"x":…,"y":…,"w":…,"h":…,"shape":{…}},
                                 {"op":"rotate","page":0,"q":1}, {"op":"delete_page","page":0},
                                 {"op":"move_page","from":0,"to":2}, {"op":"delete","page":0,"obj":1},
                                 {"op":"move","page":0,"obj":1,"x":…,"y":…}]
  ezpzxdw resave <in> <out>        save again without changes (appends a segment)
  ezpzxdw set-content <in> <entry> <data> <out>
                                 replace what entry <entry> holds (expanded data)
  ezpzxdw lzh-c  <in> <out>        compress (LHA -lh5-)
  ezpzxdw lzh-d  <in> <size> <out> expand (LHA -lh5-)
"#;

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprint!("{HELP}");
        return ExitCode::from(2);
    }
    match run(&a) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn show_value(v: &[u8]) -> String {
    let n = props::ints(v);
    let back = props::ints_value(&n);
    if !v.is_empty() && back == v && n.len() <= 8 {
        return format!("{n:?}");
    }
    let printable = v.iter().filter(|&&c| (0x20..0x7f).contains(&c) || c == 0).count();
    if v.len() > 1 && printable == v.len() {
        return format!("{:?}", String::from_utf8_lossy(v).trim_end_matches('\0'));
    }
    if v.len() >= 4 && v.len() % 2 == 0 && v.iter().skip(1).step_by(2).filter(|&&c| c == 0).count() * 3 >= v.len() {
        let u: Vec<u16> = v.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return format!("u{:?}", String::from_utf16_lossy(&u).trim_end_matches('\0'));
    }
    let h: String = v.iter().take(24).map(|x| format!("{x:02x}")).collect();
    if v.len() > 24 {
        format!("<{h}… {} bytes>", v.len())
    } else {
        format!("<{h}>")
    }
}

fn run(a: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let b = std::fs::read(&a[2])?;
    match a[1].as_str() {
        "lzh-c" => {
            std::fs::write(a.get(3).ok_or("missing out")?, ezpzxdw_core::lzh::compress(&b))?;
            return Ok(());
        }
        "lzh-d" => {
            let n: usize = a.get(3).ok_or("missing size")?.parse()?;
            std::fs::write(a.get(4).ok_or("missing out")?, ezpzxdw_core::lzh::decompress(&b, n)?)?;
            return Ok(());
        }
        _ => {}
    }
    let c = Container::parse(&b)?;
    let mut out = std::io::stdout().lock();
    match a[1].as_str() {
        "info" => {
            writeln!(out, "size {}  generation {:?}  segments {}", c.size, c.generation, c.segments.len())?;
            for s in &c.segments {
                writeln!(out, "segment @{}..{} entries {} props {:?} trailer {:?}", s.at, s.end, s.entries.len(), s.props, s.trailer)?;
            }
            let t = &c.trailer;
            writeln!(
                out,
                "trailer @{} tag {:#04x} count {} props {}→{} check {:08x} 0x82 {:02x?} checks {}",
                t.at, t.tag, t.count, t.props_stored, t.props_expanded, t.check, t.unknown82, t.checks.len()
            )?;
            for (k, e) in c.entries(&b)?.iter().enumerate() {
                let desc = match &e.body {
                    Body::Fields { fields, .. } => fields
                        .iter()
                        .map(|(t, v, (_, l))| match v {
                            Some(v) => format!("{t:#04x}={v}"),
                            None => format!("{t:#04x}[{l}]"),
                        })
                        .collect::<Vec<_>>()
                        .join(" "),
                    Body::Raw(_, l) => format!("raw[{l}] {:02x?}", &e.data(&b)[..e.data(&b).len().min(16)]),
                };
                writeln!(out, "entry {k} @{} check {:08x?} {desc}", e.at, e.check)?;
            }
        }
        "tree" => {
            let p = c.properties(&b)?;
            for r in props::parse(&p)? {
                let defs = r.defs();
                let name = |t: u32| defs.iter().find(|d| d.tag == t).map(|d| d.name.clone());
                let kind: String = r.kind.iter().map(|x| format!("{x:02x}")).collect();
                let pad = "  ".repeat(r.depth as usize);
                writeln!(out, "{pad}[{}] {kind}", r.depth)?;
                for at in &r.attrs {
                    if at.tag == props::A_DEFS {
                        let names: Vec<String> = defs.iter().map(|d| format!("{}:{}({})", d.tag, d.name, d.ty)).collect();
                        writeln!(out, "{pad}    defs {}", names.join(" "))?;
                        continue;
                    }
                    let label = match name(at.tag) {
                        Some(n) => format!("{}·{n}", at.tag),
                        None => format!("{}", at.tag),
                    };
                    let mark = if at.class & 0x40 != 0 { "ᵖ" } else { "" };
                    if std::env::var_os("EZPZXDW_RAW").is_some() {
                        let h: String = at.value.iter().map(|x| format!("{x:02x}")).collect();
                        writeln!(out, "{pad}    {label}{mark} = {} <{h}>", show_value(&at.value))?;
                    } else {
                        writeln!(out, "{pad}    {label}{mark} = {}", show_value(&at.value))?;
                    }
                }
            }
        }
        "props" => {
            let p = c.properties(&b)?;
            match a.get(3) {
                Some(o) => std::fs::write(o, &p)?,
                None => out.write_all(&p)?,
            }
        }
        "entry" => {
            let n: usize = a.get(3).ok_or("missing entry number")?.parse()?;
            let es = c.entries(&b)?;
            let e = es.get(n).ok_or("no such entry")?;
            let data = e.expanded(&b)?;
            match a.get(4) {
                Some(o) => std::fs::write(o, data)?,
                None => out.write_all(&data)?,
            }
        }
        "check" => {
            let stored = c.properties_stored(&b)?;
            let ok_props = tlv::check(stored) == c.trailer.check;
            let es = c.entries(&b)?;
            let bad: Vec<usize> = es
                .iter()
                .enumerate()
                .filter(|(_, e)| e.check.is_some_and(|k| k != tlv::check(&b[e.body_range.0..e.body_range.0 + e.body_range.1])))
                .map(|(k, _)| k)
                .collect();
            let p = c.properties(&b)?;
            let recs = props::parse(&p)?;
            let rt = props::write(&recs) == p;
            writeln!(
                out,
                "props check {} | entries {} bad {:?} | props round trip {}",
                if ok_props { "ok" } else { "BAD" },
                es.len(),
                bad,
                if rt { "ok" } else { "DIFF" }
            )?;
        }
        "pages" => {
            let d = ezpzxdw_core::doc::Document::open(b.clone())?;
            if let Some(n) = d.binder_name() {
                writeln!(out, "binder {n:?}")?;
                for (k, bd) in d.binder_docs().iter().enumerate() {
                    writeln!(out, "  document {} {:?}: pages {}..{}", k + 1, bd.name, bd.first_page + 1, bd.first_page + bd.pages)?;
                }
            }
            for (k, p) in d.pages.iter().enumerate() {
                writeln!(out, "page {} {}x{}", k + 1, p.w, p.h)?;
                for o in &p.objects {
                    writeln!(out, "  {} {:#x} at {},{} size {}x{} rot {} {}", o.kind_name, o.kind, o.x, o.y, o.w, o.h, o.rotation, o.text.clone().unwrap_or_default())?;
                }
            }
        }
        "render" => {
            let d = ezpzxdw_core::doc::Document::open(b.clone())?;
            let pages: Vec<usize> = match a.get(3) {
                Some(n) => vec![n.parse::<usize>()?.saturating_sub(1)],
                None => (0..d.pages.len()).collect(),
            };
            for k in pages {
                let disp = d.render(k)?;
                let mut counts = std::collections::BTreeMap::new();
                for it in &disp.items {
                    let n = match it {
                        ezpzxdw_core::gfx::Item::Fill { .. } => "fill",
                        ezpzxdw_core::gfx::Item::Stroke { .. } => "stroke",
                        ezpzxdw_core::gfx::Item::Text { .. } => "text",
                        ezpzxdw_core::gfx::Item::Image { .. } => "image",
                    };
                    *counts.entry(n).or_insert(0) += 1;
                }
                let mut sk = std::collections::BTreeMap::new();
                for s in &disp.skipped {
                    *sk.entry(s.clone()).or_insert(0) += 1;
                }
                writeln!(out, "page {}: {:?} clips {} images {} skipped {:?}", k + 1, counts, disp.clips.len(), disp.images.len(), sk)?;
            }
        }
        "images" => {
            let d = ezpzxdw_core::doc::Document::open(b.clone())?;
            let n: usize = a.get(3).ok_or("page")?.parse::<usize>()?.saturating_sub(1);
            let disp = d.render(n)?;
            for (k, im) in disp.images.iter().enumerate() {
                match &im.data {
                    ezpzxdw_core::gfx::ImageData::Jpeg(j) => writeln!(out, "{k}: jpeg {}x{} {} bytes", im.w, im.h, j.len())?,
                    ezpzxdw_core::gfx::ImageData::Rgba(px) => {
                        let dark = px.chunks_exact(4).filter(|p| p[3] > 0 && (p[0] as u32 + p[1] as u32 + p[2] as u32) < 600).count();
                        writeln!(out, "{k}: rgba {}x{} non-white {dark}", im.w, im.h)?;
                        if let Some(dir) = a.get(4) {
                            let mut ppm = format!("P6\n{} {}\n255\n", im.w, im.h).into_bytes();
                            for p in px.chunks_exact(4) {
                                ppm.extend_from_slice(&p[..3]);
                            }
                            std::fs::write(format!("{dir}/img{k}.ppm"), ppm)?;
                        }
                    }
                }
            }
        }
        "json" => {
            let d = ezpzxdw_core::doc::Document::open(b.clone())?;
            let n: usize = a.get(3).ok_or("page")?.parse::<usize>()?.saturating_sub(1);
            writeln!(out, "{}", serde_json::to_string(&d.render(n)?)?)?;
        }
        "text" => {
            let d = ezpzxdw_core::doc::Document::open(b.clone())?;
            for (k, t) in d.text().iter().enumerate() {
                writeln!(out, "--- page {}\n{t}", k + 1)?;
            }
        }
        "add-named" => {
            // add a named number attribute (with its definition) to a record
            let mut recs = props::parse(&c.properties(&b)?)?;
            let ri: usize = a.get(3).ok_or("record")?.parse()?;
            let name = a.get(4).ok_or("name")?;
            let v: i64 = a.get(5).ok_or("value")?.parse()?;
            let r = recs.get_mut(ri).ok_or("no such record")?;
            let mut defs = r.defs();
            let tag = defs.iter().map(|d| d.tag).max().unwrap_or(2000).max(2000) + 1;
            defs.push(props::AttrDef { tag, ty: 2, index: -1, name: name.clone() });
            let mut dv = Vec::new();
            for d in &defs {
                let mut nm = d.name.as_bytes().to_vec();
                nm.push(0);
                dv.extend(props::ints_value(&[d.tag as i64, d.ty, d.index, nm.len() as i64]));
                dv.push(nm.len() as u8);
                dv.extend(nm);
            }
            r.set(0x80, props::A_DEFS, dv);
            r.set(0x80, tag, props::ints_value(&[v]));
            let (o, _) = ezpzxdw_core::write::append(&b, &c, &[], &recs)?;
            std::fs::write(a.get(6).ok_or("missing out")?, o)?;
        }
        "set-kind" => {
            let mut recs = props::parse(&c.properties(&b)?)?;
            let ri: usize = a.get(3).ok_or("record")?.parse()?;
            let k = i64::from_str_radix(a.get(4).ok_or("kind (hex)")?.trim_start_matches("0x"), 16)?;
            recs.get_mut(ri).ok_or("no such record")?.kind = props::num_bytes(k);
            let (o, _) = ezpzxdw_core::write::append(&b, &c, &[], &recs)?;
            std::fs::write(a.get(5).ok_or("missing out")?, o)?;
        }
        "set-attr" => {
            let mut recs = props::parse(&c.properties(&b)?)?;
            let ri: usize = a.get(3).ok_or("record")?.parse()?;
            let tag: u32 = a.get(4).ok_or("tag")?.parse()?;
            let v = a.get(5).ok_or("value")?;
            let r = recs.get_mut(ri).ok_or("no such record")?;
            if v == "del" {
                r.attrs.retain(|x| x.tag != tag);
            } else {
                let value: Vec<u8> = if let Some(h) = v.strip_prefix("hex:") {
                    (0..h.len() / 2).map(|k| u8::from_str_radix(&h[2 * k..2 * k + 2], 16).unwrap()).collect()
                } else if let Some(t) = v.strip_prefix("u16:") {
                    t.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
                } else if let Some(t) = v.strip_prefix("int:") {
                    props::ints_value(&t.split(',').map(|x| x.parse().unwrap()).collect::<Vec<i64>>())
                } else {
                    return Err("value must start with hex:, u16: or int:".into());
                };
                let class = r.get(tag).map(|x| x.class).unwrap_or(0x80);
                r.set(class, tag, value);
            }
            let (o, _) = ezpzxdw_core::write::append(&b, &c, &[], &recs)?;
            std::fs::write(a.get(6).ok_or("missing out")?, o)?;
        }
        "edit" => {
            let mut d = ezpzxdw_core::doc::Document::open(b.clone())?;
            let ops: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(a.get(3).ok_or("ops")?)?)?;
            for op in ops {
                let g = |k: &str| op.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let page = g("page") as usize;
                match op["op"].as_str().unwrap_or("") {
                    "add" => {
                        let shape: ezpzxdw_core::edit::Shape = serde_json::from_value(op["shape"].clone())?;
                        d.add_annotation(page, g("x"), g("y"), g("w"), g("h"), &shape)?;
                    }
                    "change" => {
                        let shape: ezpzxdw_core::edit::Shape = serde_json::from_value(op["shape"].clone())?;
                        d.change_annotation(page, g("obj") as usize, g("x"), g("y"), g("w"), g("h"), &shape)?;
                    }
                    "rotate" => d.rotate_page(page, g("q") as i32)?,
                    "picture" => {
                        // a test picture: a red ring on white, pw × ph pixels
                        let (pw, ph) = (g("pw") as u32, g("ph") as u32);
                        let mut px = vec![255u8; (pw * ph * 4) as usize];
                        for yy in 0..ph {
                            for xx in 0..pw {
                                let (dx, dy) = (xx as f64 - pw as f64 / 2.0, yy as f64 - ph as f64 / 2.0);
                                let r = (dx * dx + dy * dy).sqrt();
                                if (r - pw as f64 * 0.42).abs() < pw as f64 * 0.04 || (dy.abs() < ph as f64 * 0.02 && r < pw as f64 * 0.42) {
                                    let k = ((yy * pw + xx) * 4) as usize;
                                    px[k] = 220;
                                    px[k + 1] = 30;
                                    px[k + 2] = 30;
                                }
                            }
                        }
                        let see = op.get("see_through").and_then(|v| v.as_bool()).unwrap_or(false);
                        d.add_picture(page, g("x"), g("y"), g("w"), g("h"), &px, pw, ph, see)?;
                    }
                    "set_picture" => {
                        let see = op.get("see_through").and_then(|v| v.as_bool()).unwrap_or(false);
                        d.set_picture(page, g("obj") as usize, g("x"), g("y"), g("w"), g("h"), see)?;
                    }
                    "blank_page" => {
                        d.insert_blank_page(g("at") as usize, op.get("w").and_then(|v| v.as_f64()).unwrap_or(21000.0), op.get("h").and_then(|v| v.as_f64()).unwrap_or(29700.0))?;
                    }
                    "image_page" => {
                        let jpeg = std::fs::read(op["file"].as_str().ok_or("file")?)?;
                        let (iw, ih) = ezpzxdw_core::pages::jpeg_size(&jpeg).ok_or("not a JPEG")?;
                        let (pw, ph) = ezpzxdw_core::pages::a4_for(iw, ih);
                        let j = ezpzxdw_core::pages::Jpeg { data: &jpeg, w: iw, h: ih };
                        d.insert_image_page(g("at") as usize, pw, ph, &j, None, None)?;
                    }
                    "copy_pages" => {
                        let other = ezpzxdw_core::doc::Document::open(std::fs::read(op["file"].as_str().ok_or("file")?)?)?;
                        let which: Vec<usize> = match op.get("pages").and_then(|v| v.as_array()) {
                            Some(v) => v.iter().filter_map(|x| x.as_u64()).map(|x| x as usize).collect(),
                            None => (0..other.pages.len()).collect(),
                        };
                        d.insert_pages_from(g("at") as usize, &other, &which)?;
                    }
                    "binder_add" => {
                        let other = ezpzxdw_core::doc::Document::open(std::fs::read(op["file"].as_str().ok_or("file")?)?)?;
                        d.add_binder_docs(g("at") as usize, &other, op["name"].as_str().unwrap_or(""))?;
                    }
                    "binder_rename" => d.rename_binder_doc(g("doc") as usize, op["name"].as_str().unwrap_or(""))?,
                    "binder_delete" => d.delete_binder_doc(g("doc") as usize)?,
                    "binder_move" => d.move_binder_doc(g("from") as usize, g("to") as usize)?,
                    "delete_page" => d.delete_page(page)?,
                    "move_page" => d.move_page(g("from") as usize, g("to") as usize)?,
                    "delete" => d.delete_object(page, g("obj") as usize)?,
                    "move" => d.move_object(page, g("obj") as usize, g("x"), g("y"))?,
                    o => return Err(format!("unknown op {o}").into()),
                }
            }
            std::fs::write(a.get(4).ok_or("missing out")?, d.save()?)?;
        }
        "resave" => {
            let recs = props::parse(&c.properties(&b)?)?;
            let (o, _) = ezpzxdw_core::write::append(&b, &c, &[], &recs)?;
            std::fs::write(a.get(3).ok_or("missing out")?, o)?;
        }
        "set-content" => {
            use ezpzxdw_core::write::{self, NewEntry};
            let idx: u32 = a.get(3).ok_or("missing entry")?.parse()?;
            let data = std::fs::read(a.get(4).ok_or("missing data")?)?;
            let es = c.entries(&b)?;
            let e = es.get(idx as usize).ok_or("no such entry")?;
            let Body::Fields { kind, fields, .. } = &e.body else { return Err("raw entry".into()) };
            let keep: Vec<(u8, i64)> = fields
                .iter()
                .filter(|f| !matches!(f.0, 0x80 | 0x81 | 0x89 | 0x8a | 0x86))
                .map(|f| (f.0, f.1.unwrap_or(0) as i64))
                .collect();
            let compressed = e.field(0x8a) == Some(1);
            let body = write::content_body(kind.unwrap_or(4) as i64, &keep, &data, compressed);
            let new_len = body.len();
            let mut recs = props::parse(&c.properties(&b)?)?;
            let next = c.trailer.offsets.len() as u32;
            let mut changed = 0;
            for r in &mut recs {
                for at in &mut r.attrs {
                    if at.class & 0xc0 == 0xc0 {
                        if let Some((i, _)) = write::read_ref(&at.value) {
                            if i == idx {
                                at.value = write::entry_ref(next, new_len);
                                changed += 1;
                            }
                        }
                    }
                }
            }
            if changed == 0 {
                return Err("no reference to that entry".into());
            }
            let (o, _) = write::append(&b, &c, &[NewEntry { body, picture: false }], &recs)?;
            std::fs::write(a.get(5).ok_or("missing out")?, o)?;
            eprintln!("references changed: {changed}");
        }
        _ => {
            eprint!("{HELP}");
            return Err("unknown command".into());
        }
    }
    Ok(())
}
