//! Print archetype records (block type 0x82d6fc83, 184 bytes) whose words
//! include the joaat of one of the given model names, as floats and words,
//! and where in the record the name sits.
//!
//! `archraw <archive> <offsets.txt> <models.tsv> <step> <name>...`
use std::collections::HashMap;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let all: HashMap<u32, String> = std::fs::read_to_string(&a[2])
        .unwrap()
        .lines()
        .filter_map(|l| l.split('\t').nth(2))
        .map(|n| {
            let n = n.trim_end_matches(".#dr").trim_end_matches(".#ft").to_string();
            (rage::joaat(&n), n)
        })
        .collect();
    let step: usize = a[3].parse().unwrap();
    let want: HashMap<u32, &str> = a[4..].iter().map(|n| (rage::joaat(n), n.as_str())).collect();
    let mut offsets: HashMap<usize, usize> = HashMap::new();
    let (mut shown, mut recs) = (0, 0);
    for line in std::fs::read_to_string(&a[1]).unwrap().lines().step_by(step) {
        let Some(off) = line.split_whitespace().next().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()) else { continue };
        let Ok((first, _)) = ar.inflate_at(off, 1 << 20) else { continue };
        let list = rage::le_u64(&first, 0x38).wrapping_sub(0x5000_0000) as usize;
        let n = if first.len() > 0x4e { u16::from_le_bytes([first[0x4c], first[0x4d]]) as usize } else { 0 };
        let has = (0..n.min(4096)).any(|i| first.get(list + i * 16..list + i * 16 + 4).is_some_and(|b| u32::from_le_bytes(b.try_into().unwrap()) == 0x82d6fc83));
        if !has {
            continue;
        }
        let Ok(r) = ar.resource_at(off) else { continue };
        for (t, b) in rage::mapdata::blocks(&r.virt, &r.phys) {
            if t != 0x82d6fc83 {
                continue;
            }
            for rec in b.chunks_exact(184) {
                recs += 1;
                let words: Vec<u32> = rec.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
                for (i, w) in words.iter().enumerate() {
                    if all.contains_key(w) {
                        *offsets.entry(i * 4).or_default() += 1;
                    }
                }
                let Some((i, name)) = words.iter().enumerate().find_map(|(i, w)| want.get(w).map(|n| (i, *n))) else { continue };
                if shown < 12 {
                    shown += 1;
                    println!("{name} (name at +{:#x}) in {off:#x}", i * 4);
                    for (k, row) in words.chunks(4).enumerate() {
                        let f: Vec<String> = row.iter().map(|w| format!("{:>10.3}", f32::from_bits(*w))).collect();
                        let h: Vec<String> = row.iter().map(|w| format!("{w:08x}")).collect();
                        println!("  +{:03x} {}  {}", k * 16, h.join(" "), f.join(" "));
                    }
                }
            }
        }
    }
    let mut o: Vec<_> = offsets.into_iter().collect();
    o.sort_by_key(|x| std::cmp::Reverse(x.1));
    println!("{recs} records; model names found at {:?}", &o[..o.len().min(8)]);
}
