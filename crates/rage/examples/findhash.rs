//! For block types of 0DRPz resources, find where known model hashes sit:
//! per type, how many words match a model name's joaat and at which
//! offsets modulo candidate record sizes.
//!
//! `findhash <archive> <offsets.txt> <models.tsv> <max resources> <type-hex>...`
use std::collections::{HashMap, HashSet};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let names: HashSet<u32> = std::fs::read_to_string(&a[2]).unwrap().lines().filter_map(|l| l.split('\t').nth(2)).map(|n| rage::joaat(n.trim_end_matches(".#dr").trim_end_matches(".#ft"))).collect();
    let max: usize = a[3].parse().unwrap();
    let want: HashSet<u32> = a[4..].iter().map(|t| u32::from_str_radix(t, 16).unwrap()).collect();
    let mut found: HashMap<u32, (usize, usize, Vec<usize>, Option<(u64, Vec<u8>)>)> = HashMap::new();
    let mut seen = 0;
    for line in std::fs::read_to_string(&a[1]).unwrap().lines().step_by(std::env::var("STEP").ok().and_then(|s| s.parse().ok()).unwrap_or(13)) {
        let Some(off) = line.split_whitespace().next().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()) else { continue };
        let Ok((first, _)) = ar.inflate_at(off, 1 << 20) else { continue };
        let list = rage::le_u64(&first, 0x38).wrapping_sub(0x5000_0000) as usize;
        let n = if first.len() > 0x4e { u16::from_le_bytes([first[0x4c], first[0x4d]]) as usize } else { 0 };
        let has = (0..n.min(4096)).any(|i| first.get(list + i * 16..list + i * 16 + 4).is_some_and(|b| want.contains(&u32::from_le_bytes(b.try_into().unwrap()))));
        if !has {
            continue;
        }
        let Ok(r) = ar.resource_at(off) else { continue };
        seen += 1;
        if seen > max {
            break;
        }
        for (t, b) in rage::mapdata::blocks(&r.virt, &r.phys) {
            if !want.contains(&t) {
                continue;
            }
            let e = found.entry(t).or_insert((0, 0, Vec::new(), None));
            e.0 += b.len() / 4;
            for (i, w) in b.chunks_exact(4).enumerate() {
                if names.contains(&u32::from_le_bytes(w.try_into().unwrap())) {
                    e.1 += 1;
                    if e.2.len() < 4000 {
                        e.2.push(i * 4);
                    }
                }
            }
            if e.3.is_none() && b.len() >= 256 {
                e.3 = Some((off, b[..b.len().min(1024)].to_vec()));
            }
        }
    }
    for (t, (words, hits, pos, sample)) in &found {
        println!("type {t:08x}: {hits} name hits in {words} words");
        for size in (40usize..=256).step_by(8) {
            let mut m: HashMap<usize, usize> = HashMap::new();
            for p in pos {
                *m.entry(p % size).or_default() += 1;
            }
            let best = m.iter().max_by_key(|x| x.1).map(|(k, v)| (*k, *v)).unwrap_or((0, 0));
            if best.1 * 100 >= pos.len() * 97 && !pos.is_empty() {
                println!("  record {size}: name at +{:#x} ({} of {})", best.0, best.1, pos.len());
            }
        }
        if let Some((off, s)) = sample {
            println!("  sample from {off:#x}:");
            for (i, row) in s.chunks(16).take(24).enumerate() {
                let w: Vec<String> = row.chunks(4).map(|c| format!("{:08x}", u32::from_le_bytes(c.try_into().unwrap_or([0; 4])))).collect();
                let f: Vec<String> = row.chunks(4).map(|c| format!("{:>12.3}", f32::from_le_bytes(c.try_into().unwrap_or([0; 4])))).collect();
                println!("  +{:03x} {}  {}", i * 16, w.join(" "), f.join(" "));
            }
        }
    }
}
