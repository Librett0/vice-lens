//! One line per file in an archive: offset, size, and what it looks like.
//! PSO files list their block types; resources their first-page signature.
//!
//! `survey <archive> > out.tsv`

use std::io::Write;

use rage::{Archive, pso, rbf, rsc, text, txd};

fn main() {
    let path = std::env::args().nth(1).expect("archive");
    let a = Archive::open(&path).expect("open");
    let mut streams = Vec::new();
    let t = std::time::Instant::now();
    a.scan(|s, _| {
        streams.push(s);
        true
    });
    eprintln!("{} streams in {:.0?}", streams.len(), t.elapsed());
    let files = Archive::files(&streams);
    let out = std::io::stdout();
    let mut out = std::io::BufWriter::new(out.lock());
    for f in &files {
        let Ok(d) = a.read_file(f) else { continue };
        let off = f.offset();
        let kind = if d.starts_with(b"PSIN") {
            match pso::Pso::parse(d.clone()) {
                Ok(p) => {
                    let types: Vec<String> = p.blocks.iter().skip(1).take(8).map(|b| format!("{:08x}:{}", b.type_hash, b.size)).collect();
                    format!("PSO region={} {}", p.blocks[0].size, types.join(","))
                }
                Err(e) => format!("PSIN? {e}"),
            }
        } else if rbf::is_rbf(&d) {
            "RBF0".into()
        } else if text::is_text_table(&d) {
            "TEXT".into()
        } else if rsc::looks_like_resource(&d) {
            if txd::probe(&d).is_some() {
                "TXD".into()
            } else {
                let name = rsc::vptr(rage::le_u64(&d, 0xa8))
                    .filter(|&o| o < d.len())
                    .map(|o| String::from_utf8_lossy(&d[o..d.len().min(o + 64)]).split('\0').next().unwrap_or("").to_string())
                    .filter(|n| n.len() > 2 && n.is_ascii())
                    .unwrap_or_default();
                format!("RSC sig={:016x} name={name}", rage::le_u64(&d, 0x10))
            }
        } else {
            format!("raw {:02x?}", &d[..d.len().min(8)])
        };
        let _ = writeln!(out, "{off:#x}\t{}\t{}\t{kind}", d.len(), f.streams.len());
    }
    eprintln!("{} files in {:.0?}", files.len(), t.elapsed());
}
