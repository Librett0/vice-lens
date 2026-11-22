//! Scan an archive and list what its files are.
//!
//! `cargo run -p rage --release --example catalog -- <archive> [--names]`

use std::collections::BTreeMap;

use rage::{Archive, gfx, rbf, rsc, text, txd};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("archive path");
    let names = args.any(|a| a == "--names");
    let archive = Archive::open(&path).expect("open");
    let t = std::time::Instant::now();
    let mut streams = Vec::new();
    archive.scan(|s, _| {
        streams.push(s);
        true
    });
    let files = Archive::files(&streams);
    eprintln!("{} streams, {} files in {:.1?}", streams.len(), files.len(), t.elapsed());
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for f in &files {
        let data = archive.read_file(f).expect("read");
        let at = f.offset();
        let kind = if rbf::is_rbf(&data) {
            match rbf::parse(&data) {
                Ok(root) => {
                    if names {
                        let n = root.text_of("Name").or_else(|| root.text_of("ID")).unwrap_or_default();
                        println!("{at:#012x} RBF0 {} {}", root.name, n.trim());
                    }
                    format!("RBF0 {}", root.name)
                }
                Err(e) => {
                    println!("{at:#012x} RBF0 error {e} size {}", data.len());
                    format!("RBF0 error: {e}")
                }
            }
        } else if gfx::is_gfx(&data) {
            if names {
                let list: Vec<_> = gfx::fonts(&data).unwrap_or_default().into_iter().map(|f| f.name).collect();
                if !list.is_empty() {
                    println!("{at:#012x} GFX fonts {list:?}");
                }
            }
            "GFX".into()
        } else if text::is_text_table(&data) {
            if names {
                let n = text::parse(&data).map(|t| t.len()).unwrap_or(0);
                if n > 0 {
                    println!("{at:#012x} TEXT size {} labels {n}", data.len());
                }
            }
            "TEXT".into()
        } else if rsc::looks_like_resource(&data) {
            if let Some(n) = txd::probe(&data) {
                if names {
                    let first: Vec<_> = txd::textures(&data).iter().take(4).map(|t| t.name.clone()).collect();
                    println!("{at:#012x} TXD {n} {first:?}");
                }
                "TXD".into()
            } else {
                "RSC".into()
            }
        } else if data.starts_with(b"PSIN") {
            "PSIN".into()
        } else {
            "other".into()
        };
        *kinds.entry(kind).or_default() += 1;
    }
    eprintln!("classified in {:.1?}", t.elapsed());
    for (k, n) in kinds {
        eprintln!("{n:8} {k}");
    }
}
