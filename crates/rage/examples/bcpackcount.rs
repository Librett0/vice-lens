//! Count the textures of every TTPF pack by how much of them is in BCPack:
//! none, only the small mips (the top level decodes), or the top level.
//!
//! `bcpackcount <game dir>`

fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let mut stack = vec![std::path::PathBuf::from(dir)];
    let (mut n, mut bytes) = ([0usize; 3], [0u64; 3]);
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !p.extension().is_some_and(|x| x == "ttpf") {
                continue;
            }
            let Ok(pk) = rage::ttpf::Pack::open(&p) else { continue };
            for (i, e) in pk.entries.iter().enumerate() {
                let k = if pk.plain(i) {
                    0
                } else if pk.top_plain(i) {
                    1
                } else {
                    2
                };
                n[k] += 1;
                bytes[k] += e.alloc;
            }
        }
    }
    let (tn, tb) = (n.iter().sum::<usize>() as f64, bytes.iter().sum::<u64>() as f64);
    for (k, label) in ["plain", "small mips in BCPack, top decodes", "top level in BCPack"].iter().enumerate() {
        println!("{label:36} {:7} textures ({:4.1}%)  {:6.2} GiB ({:4.1}%)", n[k], 100.0 * n[k] as f64 / tn, bytes[k] as f64 / (1u64 << 30) as f64, 100.0 * bytes[k] as f64 / tb);
    }
}
