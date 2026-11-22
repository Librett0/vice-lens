//! How much of a model's bone palette each skeleton covers: the palette's
//! name hashes found among a skeleton's bone names. `palcover <archive>
//! <offset> <name> <skeleton offset:name>...`
use rage::{Archive, drawable};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let load = |o: &str, n: &str| {
        let r = ar.resource_at(u64::from_str_radix(o.trim_start_matches("0x"), 16).unwrap()).unwrap();
        drawable::parse_model(&r.virt, &r.phys, rage::joaat(n)).unwrap()
    };
    let d = load(&a[1], &a[2]);
    let pal: Vec<u32> = d.meshes.iter().flat_map(|m| m.palette.iter().copied()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    for s in &a[3..] {
        let (o, n) = s.split_once(':').unwrap();
        let sk = load(o, n);
        let names: std::collections::HashMap<u32, &str> = sk.bones.iter().map(|b| (rage::joaat(&b.name), b.name.as_str())).collect();
        let found = pal.iter().filter(|h| names.contains_key(h)).count();
        println!("{n}: {} bones, covers {found} of {}", sk.bones.len(), pal.len());
    }
    // The bones of every skeleton given, by name hash, to name what is missing.
    let all: std::collections::HashMap<u32, String> = a[3..]
        .iter()
        .flat_map(|s| {
            let (o, n) = s.split_once(':').unwrap();
            load(o, n).bones.into_iter().map(|b| (rage::joaat(&b.name), b.name))
        })
        .collect();
    let own: std::collections::HashSet<u32> = d.bones.iter().chain(&d.more_bones).map(|b| rage::joaat(&b.name)).collect();
    println!("the model's own skeletons: {} bones, cover {} of {}", own.len(), pal.iter().filter(|h| own.contains(h)).count(), pal.len());
    let missing: Vec<String> = pal.iter().filter(|h| !all.contains_key(h) && !own.contains(h)).map(|h| format!("{h:08x}")).collect();
    println!("in none: {}", missing.join(" "));
}
