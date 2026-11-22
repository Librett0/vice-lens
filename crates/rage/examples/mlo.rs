//! One interior resource against the world: its archetypes (name, flags,
//! LOD distance, bounds), which of them the world places (from the
//! placement cache) and where, and the extent of its local entities.
//!
//! `mlo <levels.rpf> <offset-hex> <entities.bin>`

use std::collections::HashMap;

use rage::le;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let cache = std::fs::read(&a[2]).unwrap();
    let mut placed: HashMap<u32, Vec<([f32; 3], [f32; 4], u32)>> = HashMap::new();
    for rec in cache.as_chunks::<60>().0 {
        let f = |o: usize| f32::from_le_bytes(rec[o..o + 4].try_into().unwrap());
        if f(4).hypot(f(8)) < 200.0 {
            continue;
        }
        placed.entry(u32::from_le_bytes(rec[0..4].try_into().unwrap())).or_default().push(([f(4), f(8), f(12)], [f(16), f(20), f(24), f(28)], u32::from_le_bytes(rec[44..48].try_into().unwrap())));
    }
    let es = rage::mapdata::entities(&r.virt, &r.phys);
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for e in &es {
        for c in 0..3 {
            lo[c] = lo[c].min(e.pos[c]);
            hi[c] = hi[c].max(e.pos[c]);
        }
    }
    println!("{} local entities spanning {lo:?} .. {hi:?}", es.len());
    for (t, b) in rage::mapdata::blocks(&r.virt, &r.phys) {
        if t != rage::mapdata::ARCHETYPE {
            continue;
        }
        for rec in b.as_chunks::<184>().0 {
            let f = |o: usize| f32::from_bits(le::u32(rec, o));
            let name = le::u32(rec, 0x74);
            let Some(p) = placed.get(&name) else { continue };
            println!(
                "archetype {name:08x} flags {:08x} lod {:.0} box {:?}..{:?} radius {:.1}: placed {} times, first at {:?} rot {:?} flags {:#x}",
                le::u32(rec, 0x30),
                f(0x38),
                [f(0x40), f(0x44), f(0x48)],
                [f(0x4c), f(0x50), f(0x54)],
                f(0x64),
                p.len(),
                p[0].0,
                p[0].1,
                p[0].2
            );
        }
    }
}
