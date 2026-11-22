//! Tally the block types of the 0DRPz resources at the listed offsets:
//! how many resources have each, total bytes, and a sample resource.
//!
//! `blocktypes <archive> <offsets.txt> [max resources]`
use std::collections::HashMap;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let max: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let mut tally: HashMap<u32, (usize, usize, u64, Vec<usize>)> = HashMap::new();
    let mut seen = 0;
    for line in std::fs::read_to_string(&a[1]).unwrap().lines().step_by(7) {
        let Some(off) = line.split_whitespace().next().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()) else { continue };
        // The block list sits in the first stream.
        let Ok((first, _)) = ar.inflate_at(off, 1 << 20) else { continue };
        seen += 1;
        if seen > max {
            break;
        }
        let list = rage::le_u64(&first, 0x38).wrapping_sub(0x5000_0000) as usize;
        let n = if first.len() > 0x4e { u16::from_le_bytes([first[0x4c], first[0x4d]]) as usize } else { 0 };
        for i in 0..n.min(4096) {
            let r = list + i * 16;
            let Some(rec) = first.get(r..r + 8) else { break };
            let t = u32::from_le_bytes(rec[0..4].try_into().unwrap());
            let size = u32::from_le_bytes(rec[4..8].try_into().unwrap()) as usize;
            let e = tally.entry(t).or_insert((0, 0, off, Vec::new()));
            e.0 += 1;
            e.1 += size;
            if e.3.len() < 6 {
                e.3.push(size);
            }
        }
    }
    let mut v: Vec<_> = tally.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1.0));
    for (t, (n, bytes, off, sizes)) in v.iter().take(40) {
        println!("{t:08x}\t{n}\t{bytes}\t{off:#x}\t{sizes:?}");
    }
    eprintln!("{seen} resources");
}
