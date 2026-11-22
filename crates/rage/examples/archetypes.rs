//! Extract archetypes (block type 0x82d6fc83 of 0DRPz resources, 184-byte
//! records) into a binary cache: per archetype 16 bytes (u32 model name
//! joaat from +0x74, f32 LOD distance from +0x38, f32 bounding sphere
//! radius from +0x64, f32 full-texture distance from +0x70).
//!
//! `archetypes <archive> <offsets.txt> <out.bin>`
use std::io::Write;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let mut out = std::io::BufWriter::new(std::fs::File::create(&a[2]).unwrap());
    let (mut files, mut total) = (0usize, 0usize);
    let t = std::time::Instant::now();
    for line in std::fs::read_to_string(&a[1]).unwrap().lines() {
        let Some(off) = line.split_whitespace().next().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()) else { continue };
        let Ok((first, _)) = ar.inflate_at(off, 1 << 20) else { continue };
        let list = rage::le_u64(&first, 0x38).wrapping_sub(0x5000_0000) as usize;
        let n = if first.len() > 0x4e { u16::from_le_bytes([first[0x4c], first[0x4d]]) as usize } else { 0 };
        let has = (0..n.min(4096)).any(|i| first.get(list + i * 16..list + i * 16 + 4).is_some_and(|b| u32::from_le_bytes(b.try_into().unwrap()) == rage::mapdata::ARCHETYPE));
        if !has {
            continue;
        }
        let Ok(r) = ar.resource_at(off) else { continue };
        let found = rage::mapdata::archetypes(&r.virt, &r.phys);
        if found.is_empty() {
            continue;
        }
        files += 1;
        total += found.len();
        for x in found {
            for w in [x.name, x.lod_dist.to_bits(), x.radius.to_bits(), x.hd_texture_dist.to_bits()] {
                out.write_all(&w.to_le_bytes()).unwrap();
            }
        }
    }
    eprintln!("{files} files, {total} archetypes in {:.0?}", t.elapsed());
}
