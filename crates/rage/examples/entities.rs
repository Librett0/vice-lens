//! Extract every placed entity from the 0DRPz resources at the listed
//! offsets into a binary cache: per entity 60 bytes (u32 model, 3 f32
//! position, 4 f32 rotation, f32 scale xy, f32 scale z, i32 parent, u32
//! flags, f32 LOD distance, f32 children's LOD distance, u32 children).
//!
//! `entities <archive> <offsets.txt> <out.bin> [--instances]`
//!
//! With `--instances`, batched instances are written instead (decomposed
//! into position, rotation and scale; parent -2), 48 bytes each: the
//! record above without its last three fields.

use std::io::Write;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let mut out = std::io::BufWriter::new(std::fs::File::create(&a[2]).unwrap());
    let (mut maps, mut total) = (0usize, 0usize);
    let t = std::time::Instant::now();
    for line in std::fs::read_to_string(&a[1]).unwrap().lines() {
        let Some(off) = line.split_whitespace().next().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()) else { continue };
        // The block list is in the first stream: skip resources without
        // entity or batch blocks before reading the rest.
        let Ok((first, _)) = ar.inflate_at(off, 1 << 20) else { continue };
        let list = rage::le_u64(&first, 0x38).wrapping_sub(0x5000_0000) as usize;
        let n = if first.len() > 0x4e { u16::from_le_bytes([first[0x4c], first[0x4d]]) as usize } else { 0 };
        let want = (0..n).any(|i| {
            let r = list + i * 16;
            r + 4 <= first.len() && matches!(u32::from_le_bytes(first[r..r + 4].try_into().unwrap()), rage::mapdata::ENTITY | rage::mapdata::BATCH)
        });
        if !want && list + n * 16 <= first.len() {
            continue;
        }
        let Ok(r) = ar.resource_at(off) else { continue };
        let instances = a.get(3).is_some_and(|x| x == "--instances");
        let es = if instances {
            rage::mapdata::instances(&r.virt, &r.phys).into_iter().filter_map(|(model, m)| rage::mapdata::instance_entity(model, m)).collect()
        } else {
            rage::mapdata::entities(&r.virt, &r.phys)
        };
        if es.is_empty() {
            continue;
        }
        maps += 1;
        total += es.len();
        for e in es {
            let mut rec = Vec::with_capacity(60);
            rec.extend_from_slice(&e.model.to_le_bytes());
            for v in e.pos.iter().chain(&e.rot).chain([&e.scale_xy, &e.scale_z]) {
                rec.extend_from_slice(&v.to_le_bytes());
            }
            rec.extend_from_slice(&e.parent.to_le_bytes());
            rec.extend_from_slice(&e.flags.to_le_bytes());
            if !instances {
                rec.extend_from_slice(&e.lod_dist.to_le_bytes());
                rec.extend_from_slice(&e.child_lod_dist.to_le_bytes());
                rec.extend_from_slice(&e.children.to_le_bytes());
            }
            out.write_all(&rec).unwrap();
        }
    }
    eprintln!("{maps} maps, {total} entities in {:.0?}", t.elapsed());
}
