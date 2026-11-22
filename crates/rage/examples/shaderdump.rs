//! Every texture parameter of a model's shaders, with the textures as PNG,
//! and the textures stored next to each in its pack (by stream offset):
//! looks for variants (other colours of the same item).
//!
//! `shaderdump <game> <archive> <offset> <name> <out dir> [neighbours]`
use rage::{Archive, drawable, le, rsc::vptr, swizzle, ttpf};
use tiny_skia::Pixmap;
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
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let game = std::path::Path::new(&a[0]);
    let ar = Archive::open(game.join(&a[1])).unwrap();
    let off = u64::from_str_radix(a[2].trim_start_matches("0x"), 16).unwrap();
    let out = std::path::Path::new(&a[4]);
    let around: usize = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
    std::fs::create_dir_all(out).unwrap();
    let mut paths = Vec::new();
    walk(game, &mut paths);
    let packs: Vec<(std::path::PathBuf, ttpf::Pack)> = paths.into_iter().filter_map(|p| ttpf::Pack::open(&p).ok().map(|k| (p, k))).collect();
    let r = ar.resource_at(off).unwrap();
    let v = &r.virt;
    let d = drawable::parse_model(v, &r.phys, rage::joaat(&a[3])).unwrap();
    let b = (0..le::u16(v, 0x38) as usize).find_map(|k| drawable::dictionary_entry(v, k).filter(|&b| drawable::name(v, b) == d.name)).or(drawable::base(v)).unwrap_or(0);
    let sg = vptr(le::u64(v, b + 0x10)).unwrap();
    let sa = vptr(le::u64(v, sg + 0x10)).unwrap();
    let n = le::u16(v, sg + 0x18) as usize;
    println!("{}: {n} shaders, meshes {}", d.name, d.meshes.len());
    for si in 0..n {
        let Some(sh) = vptr(le::u64(v, sa + si * 8)) else { continue };
        let (Some(refs), Some(info)) = (vptr(le::u64(v, sh + 0x10)), vptr(le::u64(v, sh + 0x20))) else { continue };
        let (nt, np) = (le::u8(v, info + 1) as usize, le::u8(v, info + 4) as usize);
        println!("shader {si}: effect {:08x} bucket {:02x} {nt} textures {np} params", le::u32(v, sh), le::u8(v, sh + 0x39));
        for i in 0..np {
            let (hash, data) = (le::u32(v, info + 8 + i * 8), le::u32(v, info + 12 + i * 8));
            let ti = ((data >> 2) & 255) as usize;
            if data & 3 != 0 || ti >= nt {
                println!("   {hash:08x} data {data:08x}");
                continue;
            }
            let Some(tp) = vptr(le::u64(v, refs + ti * 8)) else { continue };
            let key = vptr(le::u64(v, tp + 0x30)).map(|r| le::u64(v, r + 8)).unwrap_or(0);
            let Some((path, p, i)) = packs.iter().find_map(|(path, p)| p.find(key).map(|i| (path, p, i))) else {
                println!("   {hash:08x} tex {ti} key {key:016x} (not in a pack)");
                continue;
            };
            let e = &p.entries[i];
            let name = format!("s{si}-{hash:08x}");
            let ok = dumpone(p, i, &out.join(format!("{name}.png")));
            println!(
                "   {hash:08x} tex {ti} key {key:016x} {}x{} dxgi {} {} {} #{i} at {:#x}",
                e.width,
                e.height,
                e.dxgi,
                if ok { "ok" } else { "-" },
                path.file_name().unwrap().to_string_lossy(),
                e.offset
            );
            if around > 0 {
                let mut by_off: Vec<usize> = (0..p.entries.len()).collect();
                by_off.sort_by_key(|&k| p.entries[k].offset);
                let at = by_off.iter().position(|&k| k == i).unwrap();
                let from = at.saturating_sub(around);
                for (k, &j) in by_off.iter().enumerate().skip(from).take(at + around + 1 - from) {
                    let f = &p.entries[j];
                    let ok = dumpone(p, j, &out.join(format!("{name}-n{:+}.png", k as i64 - at as i64)));
                    println!("        {:+3} key {:016x} {}x{} dxgi {} {}", k as i64 - at as i64, f.key, f.width, f.height, f.dxgi, if ok { "ok" } else { "-" });
                }
            }
        }
    }
}
fn dumpone(p: &ttpf::Pack, i: usize, out: &std::path::Path) -> bool {
    let e = &p.entries[i];
    let Ok(lin) = p.top_mip(i) else { return false };
    let Some(img) = rgba(&lin, e.dxgi, e.width, e.height) else { return false };
    save(&img, e.width, e.height, out.to_str().unwrap());
    true
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
