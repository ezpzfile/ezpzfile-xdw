//! `ezxdw` — inspect DocuWorks files.

use ezxdw_core::container::{Body, Container};
use ezxdw_core::{props, tlv};
use std::io::Write;
use std::process::ExitCode;

const HELP: &str = r#"ezxdw — DocuWorks (.xdw) reader

USAGE:
  ezxdw info   <file>            segments, trailer, entries
  ezxdw tree   <file>            the object tree in the properties block
  ezxdw props  <file> [out]      expanded properties block (raw bytes)
  ezxdw entry  <file> <n> [out]  data of entry n (expanded when compressed)
  ezxdw check  <file>            check values and properties round trip
  ezxdw pages  <file>            pages and the objects on them
  ezxdw render <file> [page]     what drawing a page produces (summary)
  ezxdw text   <file>            text of every page
  ezxdw set-attr <in> <record> <tag> <value> <out>
                                 change one attribute (value: hex:…, u16:text, int:a,b, del)
  ezxdw edit   <in> <ops.json> <out>
                                 apply edits: [{"op":"add","page":0,"x":…,"y":…,"w":…,"h":…,"shape":{…}},
                                 {"op":"rotate","page":0,"q":1}, {"op":"delete_page","page":0},
                                 {"op":"move_page","from":0,"to":2}, {"op":"delete","page":0,"obj":1},
                                 {"op":"move","page":0,"obj":1,"x":…,"y":…}]
  ezxdw resave <in> <out>        save again without changes (appends a segment)
  ezxdw set-content <in> <entry> <data> <out>
                                 replace what entry <entry> holds (expanded data)
  ezxdw lzh-c  <in> <out>        compress (LHA -lh5-)
  ezxdw lzh-d  <in> <size> <out> expand (LHA -lh5-)
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
            std::fs::write(a.get(3).ok_or("missing out")?, ezxdw_core::lzh::compress(&b))?;
            return Ok(());
        }
        "lzh-d" => {
            let n: usize = a.get(3).ok_or("missing size")?.parse()?;
            std::fs::write(a.get(4).ok_or("missing out")?, ezxdw_core::lzh::decompress(&b, n)?)?;
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
                    writeln!(out, "{pad}    {label}{mark} = {}", show_value(&at.value))?;
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
            let d = ezxdw_core::doc::Document::open(b.clone())?;
            for (k, p) in d.pages.iter().enumerate() {
                writeln!(out, "page {} {}x{}", k + 1, p.w, p.h)?;
                for o in &p.objects {
                    writeln!(out, "  {} {:#x} at {},{} size {}x{} rot {} {}", o.kind_name, o.kind, o.x, o.y, o.w, o.h, o.rotation, o.text.clone().unwrap_or_default())?;
                }
            }
        }
        "render" => {
            let d = ezxdw_core::doc::Document::open(b.clone())?;
            let pages: Vec<usize> = match a.get(3) {
                Some(n) => vec![n.parse::<usize>()?.saturating_sub(1)],
                None => (0..d.pages.len()).collect(),
            };
            for k in pages {
                let disp = d.render(k)?;
                let mut counts = std::collections::BTreeMap::new();
                for it in &disp.items {
                    let n = match it {
                        ezxdw_core::gfx::Item::Fill { .. } => "fill",
                        ezxdw_core::gfx::Item::Stroke { .. } => "stroke",
                        ezxdw_core::gfx::Item::Text { .. } => "text",
                        ezxdw_core::gfx::Item::Image { .. } => "image",
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
            let d = ezxdw_core::doc::Document::open(b.clone())?;
            let n: usize = a.get(3).ok_or("page")?.parse::<usize>()?.saturating_sub(1);
            let disp = d.render(n)?;
            for (k, im) in disp.images.iter().enumerate() {
                match &im.data {
                    ezxdw_core::gfx::ImageData::Jpeg(j) => writeln!(out, "{k}: jpeg {}x{} {} bytes", im.w, im.h, j.len())?,
                    ezxdw_core::gfx::ImageData::Rgba(px) => {
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
            let d = ezxdw_core::doc::Document::open(b.clone())?;
            let n: usize = a.get(3).ok_or("page")?.parse::<usize>()?.saturating_sub(1);
            writeln!(out, "{}", serde_json::to_string(&d.render(n)?)?)?;
        }
        "text" => {
            let d = ezxdw_core::doc::Document::open(b.clone())?;
            for (k, t) in d.text().iter().enumerate() {
                writeln!(out, "--- page {}\n{t}", k + 1)?;
            }
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
            let (o, _) = ezxdw_core::write::append(&b, &c, &[], &recs)?;
            std::fs::write(a.get(6).ok_or("missing out")?, o)?;
        }
        "edit" => {
            let mut d = ezxdw_core::doc::Document::open(b.clone())?;
            let ops: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(a.get(3).ok_or("ops")?)?)?;
            for op in ops {
                let g = |k: &str| op.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let page = g("page") as usize;
                match op["op"].as_str().unwrap_or("") {
                    "add" => {
                        let shape: ezxdw_core::edit::Shape = serde_json::from_value(op["shape"].clone())?;
                        d.add_annotation(page, g("x"), g("y"), g("w"), g("h"), &shape)?;
                    }
                    "change" => {
                        let shape: ezxdw_core::edit::Shape = serde_json::from_value(op["shape"].clone())?;
                        d.change_annotation(page, g("obj") as usize, g("x"), g("y"), g("w"), g("h"), &shape)?;
                    }
                    "rotate" => d.rotate_page(page, g("q") as i32)?,
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
            let (o, _) = ezxdw_core::write::append(&b, &c, &[], &recs)?;
            std::fs::write(a.get(3).ok_or("missing out")?, o)?;
        }
        "set-content" => {
            use ezxdw_core::write::{self, NewEntry};
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
