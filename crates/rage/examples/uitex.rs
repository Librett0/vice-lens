//! List textures in an archive's texture dictionaries whose name contains
//! one of the given words: dictionary offset, name, size, format.
//!
//! `uitex <archive> <word>...`
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let words: Vec<String> = a[1..].iter().map(|w| w.to_lowercase()).collect();
    let mut streams = Vec::new();
    ar.scan(|s, _| {
        streams.push(s);
        true
    });
    for f in rage::Archive::files(&streams) {
        let Ok(d) = ar.read_file(&f) else { continue };
        if !(rage::rsc::looks_like_resource(&d) && rage::txd::probe(&d).is_some()) {
            continue;
        }
        let ts = rage::txd::textures(&d);
        for t in &ts {
            let n = t.name.to_lowercase();
            if words.iter().any(|w| n.contains(w.as_str())) {
                println!("{:#x}\t{}\t{}x{}\tdxgi {}\t({} in dict)", f.streams[0].offset, t.name, t.width, t.height, t.dxgi, ts.len());
            }
        }
    }
}
