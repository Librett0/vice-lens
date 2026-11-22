//! For every texture parameter hash used by terrain tiles, the formats and
//! mean colour of the textures it references: tells albedo (colour BC1/BC7)
//! from normal maps (BC5) and masks (BC4).
//!
//! `texroles <game> <hd-tiles.tsv> <count>`
use rage::{Archive, drawable, le, rsc::vptr, ttpf};
use std::collections::{BTreeMap, HashMap};
fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "ttpf") {
            out.push(p);
        }
    }
}
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let game = std::path::Path::new(&a[0]);
    let mut paths = Vec::new();
    collect(game, &mut paths);
    let packs: Vec<ttpf::Pack> = paths.iter().filter_map(|p| ttpf::Pack::open(p).ok()).collect();
    let mut by_key: HashMap<u64, (usize, usize)> = HashMap::new();
    for (pi, p) in packs.iter().enumerate() {
        for (i, e) in p.entries.iter().enumerate() {
            by_key.entry(e.key).or_insert((pi, i));
        }
    }
    let ar = Archive::open(game.join("levels.rpf")).unwrap();
    let list = std::fs::read_to_string(&a[1]).unwrap();
    let count: usize = a[2].parse().unwrap();
    // hash -> (dxgi -> count), shader texture-count families
    let mut roles: BTreeMap<u32, BTreeMap<u8, usize>> = BTreeMap::new();
    let mut fam: BTreeMap<u32, BTreeMap<usize, usize>> = BTreeMap::new();
    let mut missing: BTreeMap<u32, usize> = BTreeMap::new();
    for line in list.lines().filter(|l| l.contains("_hd") && !l.contains("road")).step_by(37).take(count) {
        let f: Vec<&str> = line.split('\t').collect();
        let off = u64::from_str_radix(f[0].trim_start_matches("0x"), 16).unwrap();
        let Ok(res) = ar.resource_at(off) else { continue };
        let v = &res.virt;
        let Some(b) = drawable::base(v) else { continue };
        let Some(sg) = vptr(le::u64(v, b + 0x10)) else { continue };
        let Some(sa) = vptr(le::u64(v, sg + 0x10)) else { continue };
        let n = le::u16(v, sg + 0x18) as usize;
        for si in 0..n.min(512) {
            let Some(sh) = vptr(le::u64(v, sa + si * 8)) else { continue };
            let Some(refs) = vptr(le::u64(v, sh + 0x10)) else { continue };
            let Some(info) = vptr(le::u64(v, sh + 0x20)) else { continue };
            let (nt, np) = (le::u8(v, info + 1) as usize, le::u8(v, info + 4) as usize);
            for i in 0..np {
                let (hash, data) = (le::u32(v, info + 8 + i * 8), le::u32(v, info + 12 + i * 8));
                if data & 3 != 0 {
                    continue;
                }
                *fam.entry(hash).or_default().entry(nt).or_default() += 1;
                let ti = ((data >> 2) & 255) as usize;
                let key = vptr(le::u64(v, refs + ti * 8)).and_then(|tp| vptr(le::u64(v, tp + 0x30))).map(|r| le::u64(v, r + 8));
                match key.and_then(|k| by_key.get(&k)) {
                    Some(&(pi, ei)) => *roles.entry(hash).or_default().entry(packs[pi].entries[ei].dxgi).or_default() += 1,
                    None => *missing.entry(hash).or_default() += 1,
                }
            }
        }
    }
    for (h, d) in &roles {
        println!("{h:08x} families {:?} missing {} dxgi {:?}", fam[h], missing.get(h).unwrap_or(&0), d);
    }
}
