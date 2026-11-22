//! How interiors hang on the world. Map data resources whose entities sit
//! around the origin hold interiors in their own coordinates; world
//! entities that name no drawable are the candidates for their placements.
//! For every local resource this looks for a 32-bit word anywhere in its
//! blocks (entities included) that is the model of such a world entity, and
//! reports where in the resource the matches sit.
//!
//! `interiors <levels.rpf> <survey-levels.tsv> <threads> <model lists...>`

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use rage::{le, mapdata};

/// A local resource: offset, entity count, block types, and every word in
/// its blocks with where it was seen (block type, offset in the block).
struct Local {
    off: u64,
    entities: usize,
    types: Vec<(u32, usize)>,
    words: HashMap<u32, (u32, usize)>,
}

/// A world entity naming no drawable.
#[derive(Clone)]
struct Unnamed {
    off: u64,
    pos: [f32; 3],
    rot: [f32; 4],
    flags: u32,
    raw: Vec<u8>,
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let rows = rage::index::read_survey(&std::fs::read_to_string(&a[1]).unwrap());
    let threads: usize = a[2].parse().unwrap();
    let mut drawables: HashSet<u32> = HashSet::new();
    let mut names: HashMap<u32, String> = HashMap::new();
    for list in &a[3..] {
        for l in std::fs::read_to_string(list).unwrap().lines() {
            let n = l.rsplit('\t').next().unwrap().trim_end_matches(".#dr").trim_end_matches(".#ft");
            drawables.insert(rage::joaat(n));
            names.insert(rage::joaat(n), n.to_string());
        }
    }
    let offs = rage::index::map_data(&rows);
    let locals = Mutex::new(Vec::<Local>::new());
    let unnamed = Mutex::new(HashMap::<u32, Vec<Unnamed>>::new());
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                while let Some(&off) = offs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    // Cheap check on the first stream: an entity block?
                    let Ok((first, _)) = ar.inflate_at(off, 1 << 20) else { continue };
                    let list = rage::le_u64(&first, 0x38).wrapping_sub(0x5000_0000) as usize;
                    let n = if first.len() > 0x4e { u16::from_le_bytes([first[0x4c], first[0x4d]]) as usize } else { 0 };
                    let has = list + n * 16 > first.len() || (0..n).any(|i| first.get(list + i * 16..list + i * 16 + 4).is_some_and(|b| u32::from_le_bytes(b.try_into().unwrap()) == mapdata::ENTITY));
                    if !has {
                        continue;
                    }
                    let Ok(r) = ar.resource_at(off) else { continue };
                    let es = mapdata::entities(&r.virt, &r.phys);
                    if es.is_empty() {
                        continue;
                    }
                    let mut d: Vec<f32> = es.iter().map(|e| e.pos[0].hypot(e.pos[1])).collect();
                    let mid = d.len() / 2;
                    d.select_nth_unstable_by(mid, f32::total_cmp);
                    let blocks = mapdata::blocks(&r.virt, &r.phys);
                    if d[mid] < 32.0 {
                        let mut words = HashMap::new();
                        for (t, b) in &blocks {
                            for (k, w) in b.as_chunks::<4>().0.iter().enumerate() {
                                words.entry(u32::from_le_bytes(*w)).or_insert((*t, k * 4));
                            }
                        }
                        let types = blocks.iter().map(|(t, b)| (*t, b.len())).collect();
                        locals.lock().unwrap().push(Local { off, entities: es.len(), types, words });
                    } else {
                        let mut u = unnamed.lock().unwrap();
                        for (t, b) in &blocks {
                            if *t != mapdata::ENTITY {
                                continue;
                            }
                            for e in b.as_chunks::<{ mapdata::ENTITY_SIZE }>().0 {
                                let model = le::u32(e, 0x44);
                                if drawables.contains(&model) {
                                    continue;
                                }
                                let f = |o: usize| f32::from_bits(le::u32(e, o));
                                u.entry(model).or_default().push(Unnamed {
                                    off,
                                    pos: [f(0x38), f(0x3c), f(0x40)],
                                    rot: [f(0x10), f(0x14), f(0x18), f(0x1c)],
                                    flags: le::u32(e, 0x48),
                                    raw: e.to_vec(),
                                });
                            }
                        }
                    }
                }
            });
        }
    });
    let locals = locals.into_inner().unwrap();
    let unnamed = unnamed.into_inner().unwrap();
    println!(
        "{} local resources ({} entities); {} unnamed world models ({} entities)",
        locals.len(),
        locals.iter().map(|l| l.entities).sum::<usize>(),
        unnamed.len(),
        unnamed.values().map(Vec::len).sum::<usize>()
    );
    let mut types: BTreeMap<u32, usize> = BTreeMap::new();
    for l in &locals {
        for t in l.types.iter().map(|t| t.0).collect::<HashSet<_>>() {
            *types.entry(t).or_default() += 1;
        }
    }
    println!("block types in local resources (resources holding each): {types:x?}");
    let (mut matched, mut wheres): (usize, BTreeMap<(u32, usize), usize>) = (0, BTreeMap::new());
    let mut shown = 0;
    for l in &locals {
        let hits: Vec<(u32, (u32, usize))> = l.words.iter().filter(|(w, _)| **w != 0 && unnamed.contains_key(*w)).map(|(w, at)| (*w, *at)).collect();
        if hits.is_empty() {
            continue;
        }
        matched += 1;
        for (_, at) in &hits {
            *wheres.entry(*at).or_default() += 1;
        }
        if shown < 5 {
            shown += 1;
            println!("{:#x}: {} entities, blocks {:x?}", l.off, l.entities, l.types);
            for (w, at) in hits.iter().take(4) {
                let u = &unnamed[w];
                println!("    {w:08x} at block {:08x} +{:#x}: {} world entities, first at {:?} rot {:?} flags {:#x} in {:#x}", at.0, at.1, u.len(), u[0].pos, u[0].rot, u[0].flags, u[0].off);
                let raw: Vec<String> = u[0].raw.as_chunks::<4>().0.iter().map(|b| format!("{:08x}", u32::from_le_bytes(*b))).collect();
                println!("      raw {}", raw.join(" "));
            }
        }
    }
    println!("{matched} of {} local resources hold the model of an unnamed world entity", locals.len());
    // Every local resource, for later tools.
    if let Ok(path) = std::env::var("LOCAL_LIST") {
        let mut v: Vec<String> = locals.iter().map(|l| format!("{:#x}\t{}\t{}", l.off, l.entities, l.types.iter().map(|t| format!("{:08x}:{}", t.0, t.1)).collect::<Vec<_>>().join(","))).collect();
        v.sort();
        std::fs::write(path, v.join("\n")).unwrap();
    }
    let mut w: Vec<_> = wheres.into_iter().collect();
    w.sort_by_key(|x| std::cmp::Reverse(x.1));
    println!("where in them (block, offset in block): {:x?}", &w[..w.len().min(12)]);
    let _ = names;
}
