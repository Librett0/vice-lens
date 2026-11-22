//! A bone's channels in each block of a clip's animations: type, track,
//! component, and for curves (6, 10) their header (words, widths, quantum,
//! offset); then the track's values at a few frames.
//!
//! `chanof <archive> <dictionary offset> <clip index> <bone hash>`
use rage::{Archive, anim, le};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let hex = |s: &str| u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
    let v = ar.resource_at(hex(&a[1])).unwrap().virt;
    let (_, clips) = anim::dictionary(&v).unwrap();
    let c = &clips[a[2].parse::<usize>().unwrap()];
    let bone = hex(&a[3]) as u32;
    let f32at = |d: &[u8], p: usize| f32::from_bits(le::u32(d, p));
    for p in &c.parts {
        let an = anim::animation(&v, p.animation).unwrap();
        let tracks: Vec<usize> = an.tracks.iter().enumerate().filter(|(_, t)| t.bone == bone).map(|(i, _)| i).collect();
        if tracks.is_empty() {
            continue;
        }
        println!("{} frames {} per block {}: tracks {:?} {:?}", an.name, an.frames, an.per_block, tracks, tracks.iter().map(|&i| (an.tracks[i].kind, an.tracks[i].format)).collect::<Vec<_>>());
        for (bi, &b) in an.blocks.iter().enumerate() {
            let len = le::u32(&v, b + 4) as usize;
            let (frame_off, frames, flen) = (le::u32(&v, b + 12) as usize, le::u16(&v, b + 0x16) as usize, le::u16(&v, b + 0x18) as usize);
            let d = &v[b + 0x20..(b + 0x20 + len).min(v.len())];
            let list = frame_off + flen * frames;
            let counts: Vec<usize> = (0..11).map(|t| le::u16(d, list + t * 2) as usize).collect();
            let (mut item, mut pos) = (list + 22, 0usize);
            for (t, &count) in counts.iter().enumerate() {
                for _ in 0..count {
                    let it = le::u16(d, item) as usize;
                    item += 2;
                    let (seq, idx) = (it >> 2, it & 3);
                    let here = pos;
                    pos += match t {
                        0 | 1 => 12,
                        2 => 4,
                        4 => 12,
                        5 => 20 + le::u32(d, pos + 8) as usize * 4,
                        6 | 10 => le::u32(d, pos) as usize * 4,
                        _ => 0,
                    };
                    if tracks.contains(&seq) {
                        let extra = if t == 6 || t == 10 {
                            let cnt = le::u32(d, here + 4);
                            format!(" words {} widths {} {} {} q {} o {}", le::u32(d, here), cnt & 0xff, (cnt >> 8) & 0xff, (cnt >> 16) & 0xff, f32at(d, here + 8), f32at(d, here + 12))
                        } else {
                            String::new()
                        };
                        println!("  block {bi} ({frames} frames): type {t} track {seq} comp {idx}{extra}");
                    }
                }
                if count % 4 != 0 {
                    item += (4 - count % 4) * 2;
                }
            }
            let blk = anim::block(&v, b, an.tracks.len()).unwrap();
            for &ti in &tracks {
                let every = std::env::var("ALL").is_ok();
                let at: Vec<usize> = if every { (0..frames).collect() } else { [0, 1, 2, 3, 8, 16, 32, 63, 64, 65, 100, frames.saturating_sub(1)].into_iter().filter(|&f| f < frames).collect() };
                let vals: Vec<String> = at
                    .iter()
                    .map(|&f| match blk.value(ti, f) {
                        Some(anim::Value::Rotation(q)) => format!("{f}:[{:.3},{:.3},{:.3},{:.3}]", q[0], q[1], q[2], q[3]),
                        Some(anim::Value::Values(x, n)) => format!("{f}:{:.3?}", &x[..n]),
                        None => format!("{f}:-"),
                    })
                    .collect();
                println!("    track {ti}: {}", vals.join(" "));
            }
        }
    }
}
