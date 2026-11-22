//! Count the textures of an archive's texture dictionaries by format and by
//! the byte at +0x40 of the texture struct (the tiling), with examples.
//!
//! `texmodes <archive>...`

use std::collections::BTreeMap;

fn main() {
    let mut counts: BTreeMap<(u8, u8), (usize, Vec<String>)> = BTreeMap::new();
    for path in std::env::args().skip(1) {
        let ar = rage::Archive::open(&path).unwrap();
        let mut streams = Vec::new();
        ar.scan(|s, _| {
            streams.push(s);
            true
        });
        for f in rage::Archive::files(&streams) {
            let Ok(d) = ar.read_file(&f) else { continue };
            if !(rage::rsc::looks_like_resource(&d) && rage::txd::probe(&d).is_some()) {
                continue;
            }
            let Ok(r) = ar.resource_at(f.offset()) else { continue };
            for t in rage::txd::textures(&r.virt) {
                let mode = r.virt.get(t.at + 0x40).copied().unwrap_or(0xff);
                let e = counts.entry((t.dxgi, mode)).or_default();
                e.0 += 1;
                if e.1.len() < 3 {
                    e.1.push(format!("{} {:#x} {} {}x{} mips {}", path.rsplit('/').next().unwrap_or(&path), f.offset(), t.name, t.width, t.height, t.mips));
                }
            }
        }
    }
    for ((dxgi, mode), (n, ex)) in &counts {
        println!("dxgi {dxgi:3} mode {mode:3}: {n:5}   {}", ex.join(" | "));
    }
}
