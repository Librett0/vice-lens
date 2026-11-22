//! The albedo map keys (`diffusetex`, 0xc8e8c282) of shaders of one effect,
//! from every `step`th model of a list: to see what that effect keeps there.
//!
//! `effectkeys <game dir> <models.tsv> <step> <effect hex> [max]`

use rage::rsc::vptr;
use rage::{Archive, drawable, le};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let game = std::path::Path::new(&a[0]);
    let step: usize = a[2].parse().unwrap();
    let effect = u32::from_str_radix(&a[3], 16).unwrap();
    let max: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(20);
    let mut archives = std::collections::HashMap::new();
    let mut found = 0;
    for line in std::fs::read_to_string(&a[1]).unwrap().lines().step_by(step) {
        let f: Vec<&str> = line.split('\t').collect();
        let (arch, off, name) = if f.len() == 2 { ("levels.rpf", f[0], f[1]) } else { (f[0], f[1], f[2]) };
        let ar = archives.entry(arch.to_string()).or_insert_with(|| Archive::open(game.join(arch)).unwrap());
        let Ok(r) = ar.resource_at(u64::from_str_radix(off.trim_start_matches("0x"), 16).unwrap()) else { continue };
        let v = &r.virt;
        let Some(b) = drawable::base(v) else { continue };
        let Some(sg) = vptr(le::u64(v, b + 0x10)) else { continue };
        let Some(sa) = vptr(le::u64(v, sg + 0x10)) else { continue };
        for si in 0..(le::u16(v, sg + 0x18) as usize).min(1024) {
            let Some(sh) = vptr(le::u64(v, sa + si * 8)) else { continue };
            if le::u32(v, sh) != effect {
                continue;
            }
            let (Some(refs), Some(info)) = (vptr(le::u64(v, sh + 0x10)), vptr(le::u64(v, sh + 0x20))) else { continue };
            for i in 0..le::u8(v, info + 4) as usize {
                let (hash, data) = (le::u32(v, info + 8 + i * 8), le::u32(v, info + 12 + i * 8));
                if hash != 0xc8e8_c282 || data & 3 != 0 {
                    continue;
                }
                let key = vptr(le::u64(v, refs + ((data >> 2) & 255) as usize * 8)).and_then(|tp| vptr(le::u64(v, tp + 0x30))).map(|r| le::u64(v, r + 8));
                if let Some(k) = key.filter(|&k| k != 0) {
                    println!("{k:016x} {name} shader {si} bucket {:02x}", le::u8(v, sh + 0x39));
                    found += 1;
                }
            }
            if found >= max {
                return;
            }
        }
    }
}
