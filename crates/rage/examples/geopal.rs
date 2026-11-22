//! A model's LOD 0 model and geometry structs, word by word, and every
//! pointer in them tried as the geometry's bone palette: vertex bone
//! indices (`INDICES=<hex stream offset>`) mapped through it to the
//! skeleton (`SKEL=<offset>:<name>`), weighted by `WEIGHTS=<hex>`, and how
//! far each vertex lies from its heaviest bone.
//!
//! `geopal <archive> <offset> <name>`
use rage::{Archive, drawable, le, rsc::vptr};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let (v, p) = (&r.virt, &r.phys);
    let hash = rage::joaat(&a[2]);
    let b = drawable::base(v)
        .or_else(|| {
            let n = le::u16(v, 0x38) as usize;
            let hashes = vptr(le::u64(v, 0x20))?;
            let k = (0..n).find(|&k| le::u32(v, hashes + k * 4) == hash).unwrap_or(0);
            drawable::dictionary_entry(v, k)
        })
        .unwrap();
    let d = drawable::parse_at(v, p, b).unwrap();
    let skel = std::env::var("SKEL")
        .ok()
        .map(|s| {
            let (o, n) = s.split_once(':').unwrap();
            let r = ar.resource_at(u64::from_str_radix(o.trim_start_matches("0x"), 16).unwrap()).unwrap();
            drawable::parse_model(&r.virt, &r.phys, rage::joaat(n)).unwrap().bones
        })
        .unwrap_or_else(|| d.bones.clone());
    let bpos: Vec<[f32; 3]> = skel.iter().map(|b| [b.matrix[12], b.matrix[13], b.matrix[14]]).collect();
    let hex = |k: &str| std::env::var(k).ok().and_then(|w| usize::from_str_radix(w.trim_start_matches("0x"), 16).ok());
    let (idx, wts) = (hex("INDICES"), hex("WEIGHTS"));
    let word = |o: usize| {
        let x = le::u64(v, o);
        if vptr(x).is_some() {
            format!("V+{:x}", vptr(x).unwrap())
        } else if x >> 28 == 6 {
            format!("P+{:x}", x & 0xfff_ffff)
        } else {
            format!("{x:016x}")
        }
    };
    let l = vptr(le::u64(v, b + 0x50)).unwrap();
    let models = vptr(le::u64(v, l)).unwrap();
    let mesh0 = &d.meshes[0];
    for mi in 0..le::u16(v, l + 8) as usize {
        let m = vptr(le::u64(v, models + mi * 8)).unwrap();
        println!("model {mi} at {m:#x}: {}", (0..8).map(|k| word(m + k * 8)).collect::<Vec<_>>().join(" "));
        let ga = vptr(le::u64(v, m + 8)).unwrap();
        for gi in 0..(le::u16(v, m + 0x10) as usize).min(2) {
            let g = vptr(le::u64(v, ga + gi * 8)).unwrap();
            println!(" geometry {gi} at {g:#x}");
            for row in 0..6 {
                println!("   +{:03x} {}", row * 32, (0..4).map(|k| word(g + row * 32 + k * 8)).collect::<Vec<_>>().join(" "));
            }
            let mut ptrs: Vec<(String, usize)> = (0..0xc0).step_by(8).filter_map(|f| vptr(le::u64(v, g + f)).map(|q| (format!("geom+{f:#x}"), q))).collect();
            ptrs.extend((0..0x40).step_by(8).filter_map(|f| vptr(le::u64(v, m + f)).map(|q| (format!("model+{f:#x}"), q))));
            for (what, q) in ptrs {
                let vals: Vec<u16> = (0..12).map(|k| le::u16(v, q + k * 2)).collect();
                let mut line = format!("   {what} -> {q:#x}: {vals:?}");
                if let (Some(io), Some(wo)) = (idx, wts) {
                    let (mut dsum, mut n) = (0.0f64, 0usize);
                    for vi in 0..mesh0.positions.len() {
                        let ws: Vec<u8> = (0..4).map(|c| p[mesh0.vb + wo + vi * 4 + c]).collect();
                        let c = (0..4).max_by_key(|&c| ws[c]).unwrap();
                        let local = p[mesh0.vb + io + vi * 4 + c] as usize;
                        let bone = le::u16(v, q + local * 2) as usize;
                        if let Some(bp) = bpos.get(bone) {
                            let qv = mesh0.positions[vi];
                            dsum += (((qv[0] - bp[0]).powi(2) + (qv[1] - bp[1]).powi(2) + (qv[2] - bp[2]).powi(2)) as f64).sqrt();
                            n += 1;
                        }
                    }
                    line += &format!("  palette test: {:.3} over {}", dsum / n.max(1) as f64, n);
                }
                println!("{line}");
            }
        }
    }
}
