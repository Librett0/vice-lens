//! Print every shader's texture parameters (hash, texture key or embedded
//! name) of a drawable, to find which parameter holds the albedo.
//!
//! `shaders <archive> <offset-hex>`
use rage::{Archive, drawable, le, rsc::vptr};
use std::collections::BTreeMap;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let mut hist: BTreeMap<u32, usize> = BTreeMap::new();
    for off in &a[1..] {
        let off = u64::from_str_radix(off.trim_start_matches("0x"), 16).unwrap();
        let res = ar.resource_at(off).unwrap();
        let v = &res.virt;
        let b = drawable::base(v).unwrap();
        println!("{}", drawable::name(v, b));
        let Some(sg) = vptr(le::u64(v, b + 0x10)) else { continue };
        let sa = vptr(le::u64(v, sg + 0x10)).unwrap();
        let n = le::u16(v, sg + 0x18) as usize;
        for si in 0..n {
            let Some(sh) = vptr(le::u64(v, sa + si * 8)) else { continue };
            let refs = vptr(le::u64(v, sh + 0x10));
            let Some(info) = vptr(le::u64(v, sh + 0x20)) else { continue };
            let (nt, np) = (le::u8(v, info + 1) as usize, le::u8(v, info + 4) as usize);
            let name = le::u32(v, sh + 0x28);
            let mut line = format!("  shader {si:>2} [{name:08x} {:08x}] tex {nt} params {np}:", le::u32(v, sh + 0x2c));
            for i in 0..np {
                let (hash, data) = (le::u32(v, info + 8 + i * 8), le::u32(v, info + 12 + i * 8));
                if data & 3 != 0 {
                    continue;
                }
                let ti = ((data >> 2) & 255) as usize;
                *hist.entry(hash).or_default() += 1;
                let what = refs
                    .and_then(|r| vptr(le::u64(v, r + ti * 8)))
                    .map(|tp| {
                        if le::u64(v, tp + 0x18) >> 28 == 6 {
                            "emb".to_string()
                        } else if let Some(r) = vptr(le::u64(v, tp + 0x30)) {
                            format!("{:016x}", le::u64(v, r + 8))
                        } else {
                            "?".into()
                        }
                    })
                    .unwrap_or_else(|| "-".into());
                line += &format!(" {hash:08x}={what}");
            }
            println!("{line}");
        }
    }
    println!("texture parameter hashes:");
    for (h, n) in hist {
        println!("  {h:08x} {n}");
    }
}
