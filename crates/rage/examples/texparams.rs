//! Survey shader parameters across models: per parameter hash, how often it
//! holds a texture, the sizes of those textures, and the models it shows in.
//! Looks for palette textures (tints) and alternative albedo maps.
//!
//! `texparams <game> <step> <list.tsv>...`
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
const NAMES: &str = "diffusetex diffusesampler diffusetex2 diffusesampler2 bumptex bumpsampler normaltex normalsampler speculartex specularsampler specsampler detailsampler detailtex tintpalettetex tintpalettesampler tintpaletteselector tintpalette palettetex palettesampler paletteselector palette tint tintindex tintcolor tintcolour colortint colourtint dirtsampler dirttex damagesampler damagetex platebgsampler platebgtex environmentsampler envtex distancemapsampler texturesamp masktex masksampler maskmap lookuptexture emissivetex emissivesampler decaltex decalsampler wrinklesampler wrinklemasksampler variationtex variation variant colorvariation colourvariation colorpalette colourpalette albedotex albedo albedosampler basecolor basecolortex basecolortexture paintcolor paintcolour livery liverytex liverysampler diffusecolor diffusecolour materialcolor materialcolour primarycolor secondarycolor tint0 tint1 tint2 tintmask tintmasktex tintmasksampler colormask colourmask colormasktex";
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let game = std::path::Path::new(&a[0]);
    let step: usize = a[1].parse().unwrap();
    let mut known: HashMap<u32, &str> = HashMap::new();
    for n in NAMES.split_whitespace() {
        known.insert(rage::joaat(n), n);
    }
    let mut paths = Vec::new();
    collect(game, &mut paths);
    let packs: Vec<ttpf::Pack> = paths.iter().filter_map(|p| ttpf::Pack::open(p).ok()).collect();
    let mut by_key: HashMap<u64, (usize, usize)> = HashMap::new();
    for (pi, p) in packs.iter().enumerate() {
        for (i, e) in p.entries.iter().enumerate() {
            by_key.entry(e.key).or_insert((pi, i));
        }
    }
    let mut archives: HashMap<String, Archive> = HashMap::new();
    // param hash -> (tex uses, other uses, sizes, examples, shader counts with it)
    let mut stats: BTreeMap<u32, (usize, usize, BTreeMap<(u32, u32), usize>, Vec<String>)> = BTreeMap::new();
    let mut multi_albedo = 0usize;
    // tintpalettetex keys: key -> (uses, found in a pack)
    let mut palettes: BTreeMap<u64, (usize, bool, String)> = BTreeMap::new();
    let mut shaders_seen = 0usize;
    for list in &a[2..] {
        let rows = std::fs::read_to_string(list).unwrap();
        for line in rows.lines().step_by(step) {
            let f: Vec<&str> = line.split('\t').collect();
            let (arch, off, name) = if f.len() == 2 { ("levels.rpf", f[0], f[1]) } else { (f[0], f[1], f[2]) };
            let ar = archives.entry(arch.into()).or_insert_with(|| Archive::open(game.join(arch)).unwrap());
            let off = u64::from_str_radix(off.trim_start_matches("0x"), 16).unwrap();
            let Ok(r) = ar.resource_at(off) else { continue };
            let v = &r.virt;
            let stem = name.trim_end_matches(".#dr").trim_end_matches(".#ft");
            let Some(d) = drawable::parse_model(v, &r.phys, rage::joaat(stem)) else { continue };
            let b = (0..le::u16(v, 0x38) as usize).find_map(|k| drawable::dictionary_entry(v, k).filter(|&b| drawable::name(v, b) == d.name)).or(drawable::base(v)).unwrap_or(0);
            let Some(sg) = vptr(le::u64(v, b + 0x10)) else { continue };
            let Some(sa) = vptr(le::u64(v, sg + 0x10)) else { continue };
            let n = le::u16(v, sg + 0x18) as usize;
            for si in 0..n.min(512) {
                let Some(sh) = vptr(le::u64(v, sa + si * 8)) else { continue };
                let Some(refs) = vptr(le::u64(v, sh + 0x10)) else { continue };
                let Some(info) = vptr(le::u64(v, sh + 0x20)) else { continue };
                shaders_seen += 1;
                if d.shaders.get(si).is_some_and(|c| c.len() > 1) {
                    multi_albedo += 1;
                }
                let (nt, np) = (le::u8(v, info + 1) as usize, le::u8(v, info + 4) as usize);
                for i in 0..np {
                    let (hash, data) = (le::u32(v, info + 8 + i * 8), le::u32(v, info + 12 + i * 8));
                    let s = stats.entry(hash).or_default();
                    if s.3.len() < 3 && !s.3.contains(&d.name) {
                        s.3.push(d.name.clone());
                    }
                    let ti = ((data >> 2) & 255) as usize;
                    if data & 3 != 0 || ti >= nt {
                        s.1 += 1;
                        continue;
                    }
                    s.0 += 1;
                    let Some(tp) = vptr(le::u64(v, refs + ti * 8)) else { continue };
                    if hash == 0x2c8d_7182 {
                        let key = vptr(le::u64(v, tp + 0x30)).map(|r| le::u64(v, r + 8)).unwrap_or(0);
                        let emb = le::u64(v, tp + 0x18) >> 28 == 6;
                        let e = palettes.entry(if emb { 1 } else { key }).or_insert((0, by_key.contains_key(&key), d.name.clone()));
                        e.0 += 1;
                    }
                    let size = if le::u64(v, tp + 0x18) >> 28 == 6 {
                        (le::u16(v, tp + 0x50) as u32, 0)
                    } else {
                        match vptr(le::u64(v, tp + 0x30)).map(|r| le::u64(v, r + 8)).and_then(|k| by_key.get(&k)) {
                            Some(&(pi, ei)) => (packs[pi].entries[ei].width, packs[pi].entries[ei].height),
                            None => (0, 1),
                        }
                    };
                    *s.2.entry(size).or_default() += 1;
                }
            }
        }
    }
    println!("{shaders_seen} shaders, {multi_albedo} with more than one albedo candidate");
    let mut pv: Vec<_> = palettes.iter().collect();
    pv.sort_by_key(|x| std::cmp::Reverse(x.1.0));
    println!("tint palettes: {} distinct keys, {} uses, {} found in packs", pv.len(), pv.iter().map(|x| x.1.0).sum::<usize>(), pv.iter().filter(|x| x.1.1).count());
    for (k, (n, found, ex)) in pv.iter().take(12).chain(pv.iter().filter(|x| x.1.1)) {
        let size = by_key.get(k).map(|&(pi, ei)| format!("{}x{} dxgi {}", packs[pi].entries[ei].width, packs[pi].entries[ei].height, packs[pi].entries[ei].dxgi)).unwrap_or_default();
        println!("  {k:016x} uses {n} found {found} {size} e.g. {ex}");
    }
    let mut v: Vec<_> = stats.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1.0 + x.1.1));
    for (h, (t, o, sizes, ex)) in v.iter().take(90) {
        let mut sz: Vec<_> = sizes.iter().collect();
        sz.sort_by_key(|x| std::cmp::Reverse(*x.1));
        let szs: Vec<String> = sz.iter().take(5).map(|((w, hh), c)| format!("{w}x{hh}:{c}")).collect();
        println!("{h:08x} {:>18} tex {t:6} other {o:6}  {}  e.g. {}", known.get(h).copied().unwrap_or("?"), szs.join(" "), ex.join(" "));
    }
}
