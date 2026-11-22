//! Dump a drawable's shader structs: their first bytes, parameters (hash,
//! data) and the triangles each draws, to find what tells materials apart
//! (decals, glass, terrain).
//!
//! `matdump <archive> <offset-hex> [dictionary entry]`

use rage::Archive;
use rage::le;
use rage::rsc::vptr;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let archive = Archive::open(&a[0]).unwrap();
    let off = u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap();
    let r = archive.resource_at(off).unwrap();
    let v = &r.virt;
    let b = match a.get(2) {
        Some(k) => rage::drawable::dictionary_entry(v, k.parse().unwrap()).unwrap(),
        None => rage::drawable::base(v).unwrap_or(0),
    };
    let d = rage::drawable::parse_at(v, &r.phys, b);
    println!("drawable at {b:#x}: {}", rage::drawable::name(v, b));
    let Some(sg) = vptr(le::u64(v, b + 0x10)) else { return };
    println!("shader group at {sg:#x}: {:02x?}", &v[sg..sg + 0x30]);
    let Some(sa) = vptr(le::u64(v, sg + 0x10)) else { return };
    let n = le::u16(v, sg + 0x18) as usize;
    for si in 0..n {
        let Some(sh) = vptr(le::u64(v, sa + si * 8)) else { continue };
        let tris: usize = d.as_ref().map_or(0, |d| d.meshes.iter().filter(|m| m.shader == si).map(|m| m.indices.len() / 3).sum());
        println!("shader {si} at {sh:#x}, {tris} triangles");
        for row in 0..4 {
            let o = sh + row * 16;
            println!("  +{:#04x}: {:02x?}", row * 16, &v[o..o + 16]);
        }
        if let Some(x) = vptr(le::u64(v, sh + 8)) {
            println!("  +0x08 -> {:02x?}", &v[x..(x + 48).min(v.len())]);
        }
        if let Some(info) = vptr(le::u64(v, sh + 0x20)) {
            let (nt, np) = (le::u8(v, info + 1) as usize, le::u8(v, info + 4) as usize);
            println!("  info {:02x?} ({nt} textures, {np} parameters)", &v[info..info + 8]);
            let p: Vec<String> = (0..np).map(|i| format!("{:08x}:{:x}", le::u32(v, info + 8 + i * 8), le::u32(v, info + 12 + i * 8))).collect();
            println!("    {}", p.join(" "));
            // Texture parameters with their TTPF keys.
            let refs = vptr(le::u64(v, sh + 0x10));
            for i in 0..np {
                let (hash, data) = (le::u32(v, info + 8 + i * 8), le::u32(v, info + 12 + i * 8));
                let ti = ((data >> 2) & 255) as usize;
                if data & 3 != 0 || ti >= nt {
                    continue;
                }
                let key = refs.and_then(|r| vptr(le::u64(v, r + ti * 8))).and_then(|tp| vptr(le::u64(v, tp + 0x30))).map(|r| le::u64(v, r + 8));
                println!("    texture {hash:08x} -> {}", key.map_or("embedded".into(), |k| format!("{k:016x}")));
            }
        }
    }
}
