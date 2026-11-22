//! Every interior placement in an archive's map data, one line each:
//! resource offset, interior hash, position, rotation, then the placement
//! record's words from `+0x48` on, to study which of several placements at
//! one spot the game shows.
//!
//! `mloplaces <levels.rpf> <survey-levels.tsv> <threads> <out.tsv>`

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let rows = rage::index::read_survey(&std::fs::read_to_string(&a[1]).unwrap());
    let threads: usize = a[2].parse().unwrap();
    let offs = rage::index::map_data(&rows);
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                while let Some(&off) = offs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let Ok(r) = ar.resource_at(off) else { continue };
                    for (t, b) in rage::mapdata::blocks(&r.virt, &r.phys) {
                        if t != rage::mapdata::MLO_INSTANCE {
                            continue;
                        }
                        for rec in b.as_chunks::<592>().0 {
                            let w = |o: usize| u32::from_le_bytes(rec[o..o + 4].try_into().unwrap());
                            let f = |o: usize| f32::from_bits(w(o));
                            let rest: Vec<String> = (0x48..0x100).step_by(4).map(|o| format!("{:08x}", w(o))).collect();
                            out.lock().unwrap().push(format!(
                                "{off:#x}\t{:016x}\t{:.2}\t{:.2}\t{:.2}\t{:.4},{:.4},{:.4},{:.4}\t{}",
                                u64::from_le_bytes(rec[0x20..0x28].try_into().unwrap()),
                                f(0x38),
                                f(0x3c),
                                f(0x40),
                                f(0x10),
                                f(0x14),
                                f(0x18),
                                f(0x1c),
                                rest.join(" ")
                            ));
                        }
                    }
                }
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort();
    std::fs::write(&a[3], v.join("\n") + "\n").unwrap();
}
