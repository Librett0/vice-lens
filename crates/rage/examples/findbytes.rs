//! Find values in every file of archives: the files as stored, and
//! resources' virtual and physical segments. A pattern is a 64-bit value
//! in hex, `w:` and a 32-bit one, or `s:` and a string.
//!
//! `findbytes <threads> <archive>... -- <pattern>...`
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use rage::Archive;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let threads: usize = a[0].parse().unwrap();
    let split = a.iter().position(|x| x == "--").expect("--");
    let keys: Vec<(String, Vec<u8>)> = a[split + 1..]
        .iter()
        .map(|k| {
            if let Some(w) = k.strip_prefix("w:") {
                (k.clone(), u32::from_str_radix(w.trim_start_matches("0x"), 16).unwrap().to_le_bytes().to_vec())
            } else if let Some(t) = k.strip_prefix("s:") {
                (k.clone(), t.as_bytes().to_vec())
            } else {
                (k.clone(), u64::from_str_radix(k.trim_start_matches("0x"), 16).unwrap().to_le_bytes().to_vec())
            }
        })
        .collect();
    for path in &a[1..split] {
        let ar = Archive::open(path).unwrap();
        let mut streams = Vec::new();
        ar.scan(|s, _| {
            streams.push(s);
            true
        });
        let files = Archive::files(&streams);
        let next = AtomicUsize::new(0);
        let hits = Mutex::new(Vec::<String>::new());
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    while let Some(f) = files.get(next.fetch_add(1, Ordering::Relaxed)) {
                        let Ok(d) = ar.read_file(f) else { continue };
                        let mut parts: Vec<(&str, Vec<u8>)> = Vec::new();
                        if rage::rsc::looks_like_resource(&d)
                            && let Ok(r) = ar.resource_at(f.offset())
                        {
                            parts.push(("virt", r.virt));
                            parts.push(("phys", r.phys));
                        }
                        parts.push(("file", d));
                        for (part, b) in &parts {
                            for (k, pat) in &keys {
                                for (at, w) in b.windows(pat.len()).enumerate() {
                                    if w == pat.as_slice() {
                                        hits.lock().unwrap().push(format!("{path} {:#x} {part} +{at:#x}: {k} ({} bytes)", f.offset(), b.len()));
                                    }
                                }
                            }
                        }
                    }
                });
            }
        });
        let mut h = hits.into_inner().unwrap();
        h.sort();
        println!("{path}: {} files, {} hits", files.len(), h.len());
        for l in h.iter().take(60) {
            println!("  {l}");
        }
    }
}
