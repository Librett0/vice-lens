//! Formats in texture packs: per pack, how many entries of each DXGI format,
//! with typical sizes, to spot data that is not pictures (buffers, strands).
//!
//! `ttpfformats <pack.ttpf>...`
use std::collections::BTreeMap;

/// Per format: entries, entries by size, whether any has mips.
type ByFormat = BTreeMap<u8, (usize, BTreeMap<(u32, u32), usize>, bool)>;

fn main() {
    for path in std::env::args().skip(1) {
        let Ok(p) = rage::ttpf::Pack::open(&path) else { continue };
        let mut by: ByFormat = BTreeMap::new();
        for e in &p.entries {
            let b = by.entry(e.dxgi).or_default();
            b.0 += 1;
            *b.1.entry((e.width, e.height)).or_default() += 1;
            b.2 |= e.mips;
        }
        println!("{path}: {} entries", p.entries.len());
        for (d, (n, sizes, mips)) in by {
            let mut s: Vec<_> = sizes.into_iter().collect();
            s.sort_by_key(|x| std::cmp::Reverse(x.1));
            let top: Vec<String> = s.iter().take(5).map(|((w, h), c)| format!("{w}x{h}:{c}")).collect();
            println!("   dxgi {d:3}: {n:6} (mips {mips}) {}", top.join(" "));
        }
    }
}
