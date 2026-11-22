//! Research dumps to PNG.
//!
//! font <archive> <file-offset> <font-name> <text> <out.png>
//! txd  <archive> <file-offset> <out-dir>
//! ttpf <pack.ttpf> <index> <out.png>   (TILE=4, 64, 4s or 64s: the top level
//!                                    in that tile mode, at the end or the start)
//! raw  <archive> <file-offset> <out.bin>
//! rsc  <archive> <file-offset> <out-prefix>   (writes .virt and .phys)

use rage::{Archive, gfx, rsc, swizzle, ttpf, txd};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a[0].as_str() {
        "font" => font(&a[1], parse_off(&a[2]), &a[3], &a[4], &a[5]),
        "txd" => dict(&a[1], parse_off(&a[2]), &a[3]),
        "ttpf" => pack(&a[1], a[2].parse().unwrap(), &a[3]),
        "key" => by_key(&a[1], u64::from_str_radix(a[2].trim_start_matches("0x"), 16).unwrap(), &a[3]),
        "rsc" => {
            let archive = Archive::open(&a[1]).unwrap();
            let r = archive.resource_at(parse_off(&a[2])).unwrap();
            std::fs::write(format!("{}.virt", a[3]), &r.virt).unwrap();
            std::fs::write(format!("{}.phys", a[3]), &r.phys).unwrap();
            println!("virtual {} physical {}", r.virt.len(), r.phys.len());
        }
        "raw" => {
            let archive = Archive::open(&a[1]).unwrap();
            std::fs::write(&a[3], file_at(&archive, parse_off(&a[2]))).unwrap();
        }
        c => panic!("unknown command {c}"),
    }
}

fn parse_off(s: &str) -> u64 {
    u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
}

fn file_at(archive: &Archive, off: u64) -> Vec<u8> {
    archive.file_at(off).unwrap()
}

fn font(path: &str, off: u64, name: &str, text: &str, out: &str) {
    let archive = Archive::open(path).unwrap();
    let data = file_at(&archive, off);
    let fonts = gfx::fonts(&data).unwrap();
    let f = fonts.iter().find(|f| f.name == name).unwrap_or_else(|| {
        panic!("fonts: {:?}", fonts.iter().map(|f| &f.name).collect::<Vec<_>>());
    });
    println!("{}: {} glyphs, ascent {} descent {} leading {}", f.name, f.glyphs.len(), f.ascent, f.descent, f.leading);
    let px = 96.0;
    let scale = px / gfx::EM;
    let mut pm = Pixmap::new(1800, 200).unwrap();
    pm.fill(tiny_skia::Color::from_rgba8(20, 20, 28, 255));
    let mut paint = Paint::default();
    paint.set_color_rgba8(240, 240, 240, 255);
    paint.anti_alias = true;
    let (mut pen, base) = (20.0f32, 140.0f32);
    let mut prev = None;
    for c in text.chars() {
        let Some(g) = f.glyph(c) else {
            pen += px * 0.3;
            continue;
        };
        if let Some(p) = prev {
            pen += f.kerning.get(&(p, c)).copied().unwrap_or(0.0) * scale;
        }
        let mut pb = PathBuilder::new();
        for s in &g.path {
            match *s {
                gfx::Seg::Move([x, y]) => pb.move_to(pen + x * scale, base + y * scale),
                gfx::Seg::Line([x, y]) => pb.line_to(pen + x * scale, base + y * scale),
                gfx::Seg::Quad([cx, cy], [x, y]) => pb.quad_to(pen + cx * scale, base + cy * scale, pen + x * scale, base + y * scale),
            }
        }
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint, FillRule::EvenOdd, Transform::identity(), None);
        }
        pen += g.advance * scale;
        prev = Some(c);
    }
    pm.save_png(out).unwrap();
}

fn rgba(lin: &[u8], dxgi: u8, w: u32, h: u32) -> Option<Vec<u8>> {
    let f = swizzle::format(dxgi)?;
    let (w, h) = (w as usize, h as usize);
    let mut px = vec![0u32; w * h];
    let ok = match dxgi {
        71 | 72 => texture2ddecoder::decode_bc1(lin, w, h, &mut px).is_ok(),
        74 | 75 | 77 | 78 => texture2ddecoder::decode_bc3(lin, w, h, &mut px).is_ok(),
        80 | 81 => texture2ddecoder::decode_bc4(lin, w, h, &mut px).is_ok(),
        83 | 84 => texture2ddecoder::decode_bc5(lin, w, h, &mut px).is_ok(),
        95 => texture2ddecoder::decode_bc6_unsigned(lin, w, h, &mut px).is_ok(),
        96 => texture2ddecoder::decode_bc6_signed(lin, w, h, &mut px).is_ok(),
        98 | 99 => texture2ddecoder::decode_bc7(lin, w, h, &mut px).is_ok(),
        28 | 29 => {
            return Some(lin[..w * h * 4].to_vec());
        }
        87 | 91 => {
            return Some(lin[..w * h * 4].chunks(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect());
        }
        61 => return Some(lin[..w * h].iter().flat_map(|&v| [v, v, v, 255]).collect()),
        65 => return Some(lin[..w * h].iter().flat_map(|&v| [255, 255, 255, v]).collect()),
        _ => false,
    };
    let _ = f;
    // texture2ddecoder yields BGRA in a u32.
    ok.then(|| {
        px.iter()
            .flat_map(|p| {
                let b = p.to_le_bytes();
                [b[2], b[1], b[0], b[3]]
            })
            .collect()
    })
}

fn save(rgba: &[u8], w: u32, h: u32, out: &str) {
    let mut pm = Pixmap::new(w, h).unwrap();
    for (d, s) in pm.pixels_mut().iter_mut().zip(rgba.chunks(4)) {
        let c = tiny_skia::ColorU8::from_rgba(s[0], s[1], s[2], s[3]);
        *d = c.premultiply();
    }
    pm.save_png(out).unwrap();
}

fn dict(path: &str, off: u64, dir: &str) {
    let archive = Archive::open(path).unwrap();
    let mut streams = Vec::new();
    archive.scan(|s, _| {
        streams.push(s);
        true
    });
    let i = streams.iter().position(|s| s.offset == off).expect("no stream at offset");
    let next = |cursor: u64| -> Option<rage::Stream> {
        let at = (cursor + 15) & !15;
        streams.iter().find(|s| s.offset >= at && s.offset < at + 0x2000).copied()
    };
    let res = rsc::read(&archive, streams[i], next).unwrap();
    println!("virtual {} physical {} streams {}", res.virt.len(), res.phys.len(), res.streams.len());
    let texs = txd::textures(&res.virt);
    let allocs = txd::allocations(&texs, res.phys.len());
    std::fs::create_dir_all(dir).unwrap();
    for (t, alloc) in texs.iter().zip(allocs) {
        let raw = txd::top_mip(&res.phys, t, alloc);
        let img = raw.as_deref().and_then(|l| rgba(l, t.dxgi, t.width, t.height));
        println!(
            "{:40} {:5}x{:<5} dxgi {:3} mips {:2} tiled {} data {:?} alloc {alloc} -> {}",
            t.name,
            t.width,
            t.height,
            t.dxgi,
            t.mips,
            t.tiled,
            t.data,
            if img.is_some() { "ok" } else { "no pixels" }
        );
        if let Some(img) = img {
            save(&img, t.width, t.height, &format!("{dir}/{}.png", t.name.replace('/', "_")));
        }
    }
}

/// Find a TTPF key in every pack under the game directory and save it.
fn by_key(game: &str, key: u64, out: &str) {
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "ttpf") {
                out.push(p);
            }
        }
    }
    let mut paths = Vec::new();
    walk(std::path::Path::new(game), &mut paths);
    for path in paths {
        let Ok(p) = ttpf::Pack::open(&path) else { continue };
        if let Some(i) = p.find(key) {
            println!("{}", path.display());
            return pack(path.to_str().unwrap(), i, out);
        }
    }
    println!("key {key:016x} not found");
}

fn pack(path: &str, index: usize, out: &str) {
    let p = ttpf::Pack::open(path).unwrap();
    let e = &p.entries[index];
    println!("{} entries; #{index}: key {:016x} {}x{} dxgi {} mips {}", p.entries.len(), e.key, e.width, e.height, e.dxgi, e.mips);
    let level: u32 = std::env::var("MIP").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let (w, h, lin) = match std::env::var("TILE").as_deref() {
        Ok("64") => (e.width, e.height, p.top_mip_tiled(index, rage::swizzle::Tile::K64).unwrap()),
        Ok("4") => (e.width, e.height, p.top_mip_tiled(index, rage::swizzle::Tile::K4).unwrap()),
        // The top level read from the start of the allocation instead.
        Ok(t @ ("4s" | "64s")) => {
            let tile = if t == "4s" { rage::swizzle::Tile::K4 } else { rage::swizzle::Tile::K64 };
            let f = rage::swizzle::format(e.dxgi).unwrap();
            let raw = p.read_virtual(e.offset, rage::swizzle::tiled_size(f, e.width, e.height, tile)).unwrap();
            (e.width, e.height, rage::swizzle::untile(&raw, f, e.width, e.height, tile))
        }
        _ => p.mip(index, level).unwrap(),
    };
    let img = rgba(&lin, e.dxgi, w, h).expect("decode");
    save(&img, w, h, out);
}
