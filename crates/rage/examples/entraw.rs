//! Print raw 144-byte entity records of the 0DRPz resources at the listed
//! offsets, as 32-bit words with their float reading, to study the layout.
//!
//! `entraw <archive> <offsets.txt> <max records> [model-hash-hex]`
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let max: usize = a[2].parse().unwrap();
    let want = a.get(3).map(|h| u32::from_str_radix(h, 16).unwrap());
    let mut shown = 0;
    for line in std::fs::read_to_string(&a[1]).unwrap().lines() {
        let Some(off) = line.split_whitespace().next().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()) else { continue };
        // Skip resources whose block list (first stream) has no entities.
        let Ok((first, _)) = ar.inflate_at(off, 1 << 20) else { continue };
        let list = rage::le_u64(&first, 0x38).wrapping_sub(0x5000_0000) as usize;
        let n = if first.len() > 0x4e { u16::from_le_bytes([first[0x4c], first[0x4d]]) as usize } else { 0 };
        let has = (0..n).any(|i| first.get(list + i * 16..list + i * 16 + 4).is_some_and(|b| u32::from_le_bytes(b.try_into().unwrap()) == rage::mapdata::ENTITY));
        if !has {
            continue;
        }
        let Ok(r) = ar.resource_at(off) else { continue };
        for (t, b) in rage::mapdata::blocks(&r.virt, &r.phys) {
            if t != rage::mapdata::ENTITY {
                continue;
            }
            for e in b.as_chunks::<144>().0 {
                let w = |o: usize| u32::from_le_bytes(e[o..o + 4].try_into().unwrap());
                if want.is_some_and(|h| w(0x44) != h) {
                    continue;
                }
                println!("res {off:#x} model {:08x}", w(0x44));
                for row in 0..9 {
                    let ws: Vec<String> = (0..4).map(|k| format!("{:08x}", w(row * 16 + k * 4))).collect();
                    let fs: Vec<String> = (0..4).map(|k| format!("{:>12.4}", f32::from_bits(w(row * 16 + k * 4)))).collect();
                    println!("  +{:02x} {}  {}", row * 16, ws.join(" "), fs.join(" "));
                }
                shown += 1;
                if shown >= max {
                    return;
                }
            }
        }
    }
}
