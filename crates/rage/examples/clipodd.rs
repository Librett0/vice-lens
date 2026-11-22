//! A clip's samples (30 a second) that are not unit turns or lie farther
//! than 50 m from their parent: bone hash, sample and value.
//!
//! `clipodd <archive> <dictionary offset> <clip index>`
use rage::{Archive, anim};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let v = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap().virt;
    let (_, clips) = anim::dictionary(&v).unwrap();
    let c = &clips[a[2].parse::<usize>().unwrap()];
    println!("{} {:.3} s", c.name, c.length);
    for p in &c.parts {
        let an = anim::animation(&v, p.animation).unwrap();
        println!("  {} frames {} ({:.3}–{:.3} s)", an.name, an.frames, p.start, p.end);
    }
    let (n, poses) = anim::sample(&v, c, 30.0);
    for (h, ps) in &poses {
        let bad: Vec<String> = ps
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                p.rotation.is_some_and(|q| (q.iter().map(|x| x * x).sum::<f32>() - 1.0).abs() > 0.01 || !q.iter().all(|x| x.is_finite()))
                    || p.position.is_some_and(|t| !t.iter().all(|x| x.is_finite() && x.abs() < 50.0))
            })
            .map(|(i, p)| format!("{i}: {:?} {:?}", p.rotation, p.position))
            .collect();
        if !bad.is_empty() {
            println!("{h:08x}: {} of {n} samples, e.g. {}", bad.len(), bad[..bad.len().min(3)].join(" | "));
        }
    }
}
