//! Which archives define the interiors the world places: the interior
//! definitions and placements of each archive's map data, and how many
//! placements find their interior in the same archive, in another, or
//! nowhere.
//!
//! `mlorefs <game dir> <threads> <archive>...`

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let game = std::path::Path::new(&a[0]);
    let threads: usize = a[1].parse().unwrap();
    let mut defined: HashMap<u64, String> = HashMap::new();
    let mut placed: Vec<(u64, String)> = Vec::new();
    for name in &a[2..] {
        let ar = rage::Archive::open(game.join(name)).unwrap();
        let rows = rage::index::survey(&ar, &|_, _| {});
        let offs = rage::index::map_data(&rows);
        let next = AtomicUsize::new(0);
        let (defs, insts) = (Mutex::new(Vec::new()), Mutex::new(Vec::new()));
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    while let Some(&off) = offs.get(next.fetch_add(1, Ordering::Relaxed)) {
                        let Ok(r) = ar.resource_at(off) else { continue };
                        defs.lock().unwrap().extend(rage::mapdata::mlos(&r.virt, &r.phys).into_iter().map(|m| m.hash));
                        insts.lock().unwrap().extend(rage::mapdata::mlo_instances(&r.virt, &r.phys).into_iter().map(|i| i.hash));
                    }
                });
            }
        });
        let (defs, insts) = (defs.into_inner().unwrap(), insts.into_inner().unwrap());
        println!("{name}: {} map data resources, {} interiors defined, {} placed", offs.len(), defs.len(), insts.len());
        for d in defs {
            defined.entry(d).or_insert(name.clone());
        }
        placed.extend(insts.into_iter().map(|h| (h, name.clone())));
    }
    let (mut same, mut other, mut none) = (0, 0, 0);
    let mut missing = HashSet::new();
    for (h, from) in &placed {
        match defined.get(h) {
            Some(d) if d == from => same += 1,
            Some(_) => other += 1,
            None => {
                none += 1;
                missing.insert(*h);
            }
        }
    }
    println!("placements: {same} defined in the same archive, {other} in another, {none} nowhere ({} distinct)", missing.len());
}
