//! Where a texture's bytes sit in its TTPF pack's chunks and which codec
//! each chunk uses: whether the small mips of a BCPack texture (stored
//! first) are in plain chunks.
//!
//! `bcpackmips <game dir> <texture key (decimal or 0x hex)>...`

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut packs = Vec::new();
    let mut stack = vec![std::path::PathBuf::from(&a[0])];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "ttpf")
                && let Ok(pk) = rage::ttpf::Pack::open(&p)
            {
                packs.push((p, pk));
            }
        }
    }
    for k in &a[1..] {
        let key = k.strip_prefix("0x").map_or_else(|| k.parse::<u64>().unwrap(), |h| u64::from_str_radix(h, 16).unwrap());
        let Some((path, pack, i)) = packs.iter().find_map(|(p, pk)| pk.find(key).map(|i| (p, pk, i))) else {
            println!("{key:016x}: not found");
            continue;
        };
        let e = &pack.entries[i];
        println!("{key:016x}: {} {}x{} dxgi {} mips {} at {:#x} alloc {:#x}", path.file_name().unwrap().to_string_lossy(), e.width, e.height, e.dxgi, e.mips, e.offset, e.alloc);
        for (codec, from, to) in pack.chunks_over(e.offset, e.offset + e.alloc) {
            println!("    codec {codec} virtual {:#x}..{:#x} (+{:#x})", from, to, from.saturating_sub(e.offset));
        }
    }
}
