//! Try every Nth model of the model index as the loader would, and count
//! what goes wrong, by archive: unreadable resource, parsed with no meshes,
//! a dictionary where a drawable was expected, or no drawable at all.
//!
//! `modelcheck <game dir> <models.tsv> [step] [--old]`: `--old` reads each
//! file as a plain drawable (`drawable::parse`), as the loader once did.

use std::collections::BTreeMap;

use rage::rsc::vptr;
use rage::{Archive, drawable};

fn u64at(v: &[u8], o: usize) -> u64 {
    v.get(o..o + 8).map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap()))
}

fn u16at(v: &[u8], o: usize) -> u16 {
    v.get(o..o + 2).map_or(0, |s| u16::from_le_bytes(s.try_into().unwrap()))
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let game = std::path::PathBuf::from(&a[0]);
    let step: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(40);
    let old = a.iter().any(|x| x == "--old");
    let text = std::fs::read_to_string(&a[1]).unwrap();
    let mut archives: BTreeMap<String, Option<Archive>> = BTreeMap::new();
    let mut counts: BTreeMap<(String, &str), usize> = BTreeMap::new();
    let mut examples: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for line in text.lines().step_by(step) {
        let mut it = line.split('\t');
        let (Some(ar), Some(off), Some(name)) = (it.next(), it.next(), it.next()) else { continue };
        let Ok(off) = u64::from_str_radix(off.trim_start_matches("0x"), 16) else { continue };
        let archive = archives.entry(ar.to_string()).or_insert_with(|| Archive::open(game.join(ar)).ok());
        let what: &str = match archive.as_ref().map(|x| x.resource_at(off)) {
            None => "archive missing",
            Some(Err(_)) => "resource unreadable",
            Some(Ok(r)) => match if old { drawable::parse(&r.virt, &r.phys) } else { drawable::parse_model(&r.virt, &r.phys, rage::joaat(name.split(".#").next().unwrap_or(name))) } {
                Some(d) if d.meshes.is_empty() => "drawable, no meshes",
                Some(d) if d.meshes.iter().all(|m| m.positions.is_empty()) => "drawable, empty meshes",
                Some(_) => "ok",
                None => {
                    let v = &r.virt;
                    let (hn, inn) = (u16at(v, 0x28), u16at(v, 0x38));
                    if hn > 0 && hn == inn && vptr(u64at(v, 0x20)).is_some() && vptr(u64at(v, 0x30)).is_some() {
                        let parsed = (0..inn as usize).filter_map(|k| drawable::dictionary_entry(v, k)).filter_map(|b| drawable::parse_at(v, &r.phys, b)).filter(|d| !d.meshes.is_empty()).count();
                        if parsed == inn as usize {
                            "dictionary, entries parse"
                        } else if parsed > 0 {
                            "dictionary, some entries parse"
                        } else {
                            "dictionary, no entry parses"
                        }
                    } else {
                        "not a drawable"
                    }
                }
            },
        };
        *counts.entry((ar.to_string(), what)).or_default() += 1;
        let ex = examples.entry(what).or_default();
        if ex.len() < 4 {
            ex.push(format!("{ar} {off:#x} {name}"));
        }
    }
    for ((ar, what), n) in &counts {
        println!("{n:7}  {what:32} {ar}");
    }
    for (what, ex) in &examples {
        if *what != "ok" {
            println!("{what}:");
            for e in ex {
                println!("    {e}");
            }
        }
    }
}
