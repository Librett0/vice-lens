//! Find drawable dictionaries: resources whose root is a pgDictionary
//! (`+0x20` hashes, `+0x28` count, `+0x30` items, `+0x38` count) of
//! drawables. Prints `offset<TAB>hash<TAB>index<TAB>first entry's name`
//! for every entry.
//!
//! `dicts <archive> <survey.tsv> <out.tsv> [threads]`
//!
//! Every parsable resource is checked, those in the model list too: the
//! model index pass reads a dictionary as one drawable named after its
//! first entry, which hid 21,545 dictionaries of LOD stand-ins.
use rage::{Archive, drawable, rsc::vptr};
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

fn u64at(b: &[u8], o: usize) -> u64 {
    b.get(o..o + 8).map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap()))
}
fn u16at(b: &[u8], o: usize) -> usize {
    b.get(o..o + 2).map_or(0, |s| u16::from_le_bytes(s.try_into().unwrap()) as usize)
}
fn u32at(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4).map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let offs: Vec<u64> = std::fs::read_to_string(&a[1])
        .unwrap()
        .lines()
        .filter(|l| l.split('\t').nth(3).is_some_and(|k| k.starts_with("RSC")))
        .filter_map(|l| u64::from_str_radix(l.split('\t').next()?.trim_start_matches("0x"), 16).ok())
        .collect();
    eprintln!("{} resources to check", offs.len());
    let threads: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(8);
    let next = AtomicUsize::new(0);
    let out = Mutex::new(std::io::BufWriter::new(std::fs::File::create(&a[2]).unwrap()));
    let found = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&off) = offs.get(i) else { break };
                    let Ok((first, _)) = ar.inflate_at(off, 1 << 20) else { continue };
                    let (hp, hn, ip, inn) = (vptr(u64at(&first, 0x20)), u16at(&first, 0x28), vptr(u64at(&first, 0x30)), u16at(&first, 0x38));
                    let (Some(hp), Some(_)) = (hp, ip) else { continue };
                    if hn == 0 || hn != inn || hn > 4096 || hp + hn * 4 > first.len() {
                        continue;
                    }
                    // Confirm: the first item parses as a drawable.
                    let Ok(res) = ar.resource_at(off) else { continue };
                    let items = vptr(u64at(&res.virt, 0x30)).unwrap();
                    let Some(d0) = vptr(u64at(&res.virt, items)) else { continue };
                    let name = drawable::name(&res.virt, d0);
                    let mut o = out.lock().unwrap();
                    for k in 0..hn {
                        writeln!(o, "{off:#x}\t{:08x}\t{k}\t{}", u32at(&res.virt, hp + k * 4), if k == 0 { name.as_str() } else { "" }).unwrap();
                    }
                    found.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
    eprintln!("{} dictionaries", found.load(Ordering::Relaxed));
}
