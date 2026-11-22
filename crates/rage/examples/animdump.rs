//! A clip dictionary of `anim.rpf`, as far as it is read (docs/formats.md,
//! "Animations"): its clips (name, length, the animations each plays and
//! the stretch of each), and per animation its frames, length, tracks
//! (bone name hash, track hash, type) and data blocks with their channel
//! counts by type.
//!
//! `animdump <archive> <offset> [clips]`
use rage::{Archive, le, rsc::vptr};

fn cstr(v: &[u8], o: usize) -> String {
    String::from_utf8_lossy(le::cstr(v, o)).into_owned()
}

fn f32at(v: &[u8], o: usize) -> f32 {
    f32::from_bits(le::u32(v, o))
}

/// An animation: frames, frames per block, length, name, tracks, blocks.
fn animation(v: &[u8], a: usize, tracks: bool) {
    let (frames, per_block) = (le::u16(v, a + 0x24), le::u16(v, a + 0x26));
    let name = vptr(le::u64(v, a + 0x30)).map(|o| cstr(v, o)).unwrap_or_default();
    let nt = le::u16(v, a + 0x68) as usize;
    println!("    animation {a:#x} {name}: {frames} frames ({per_block} a block), {:.2} s, {nt} tracks", f32at(v, a + 0x28));
    if let Some(t) = vptr(le::u64(v, a + 0x60)).filter(|_| tracks) {
        let mut kinds: std::collections::BTreeMap<(u32, u32), usize> = Default::default();
        for i in 0..nt {
            *kinds.entry((le::u32(v, t + i * 12 + 4), le::u32(v, t + i * 12 + 8))).or_default() += 1;
        }
        println!("      tracks by (track hash, type): {}", kinds.iter().map(|((h, k), n)| format!("{h:08x}/{k}×{n}")).collect::<Vec<_>>().join(" "));
    }
    let Some(list) = vptr(le::u64(v, a + 0x50)) else { return };
    for k in 0..le::u16(v, a + 0x58) as usize {
        let Some(b) = vptr(le::u64(v, list + k * 8)) else { continue };
        let (len, frame_off) = (le::u32(v, b + 4) as usize, le::u32(v, b + 12) as usize);
        let (bframes, flen) = (le::u16(v, b + 0x16) as usize, le::u16(v, b + 0x18) as usize);
        let data = b + 0x20;
        let at = data + frame_off + flen * bframes;
        if at + 22 > v.len() || frame_off > len {
            continue;
        }
        let counts: Vec<u16> = (0..11).map(|i| le::u16(v, at + i * 2)).collect();
        println!("      block {k}: {len} bytes, {bframes} frames × {flen} bytes, {} chunks; channels by type {counts:?}", v[b + 0x1e]);
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let v = &r.virt;
    let want: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(3);
    let dict = vptr(le::u64(v, 0x38)).map(|o| cstr(v, o)).unwrap_or_default();
    let (buckets, clips) = (le::u16(v, 0x40) as usize, le::u16(v, 0x42));
    println!("{dict}: {buckets} buckets ({clips} in the header)");
    let Some(map) = vptr(le::u64(v, 0x48)) else { return };
    let mut shown = 0;
    for b in 0..buckets {
        let mut e = vptr(le::u64(v, map + b * 8));
        while let Some(eo) = e {
            e = vptr(le::u64(v, eo + 0x10));
            let Some(c) = vptr(le::u64(v, eo + 8)) else { continue };
            if shown >= want {
                continue;
            }
            shown += 1;
            let name = vptr(le::u64(v, c + 0x28)).map(|o| cstr(v, o)).unwrap_or_default();
            println!("clip {c:#x} {name} (type {}, {:.3} s)", le::u32(v, c + 0x20), f32at(v, c + 0x78));
            // The animations it plays: a list of (entries, count, length).
            let Some(lists) = vptr(le::u64(v, c + 0x68)) else { continue };
            let n = le::u16(v, lists + 8) as usize;
            let Some(entries) = vptr(le::u64(v, lists)) else { continue };
            for k in 0..n.min(8) {
                let en = entries + k * 0x38;
                let Some(anim) = vptr(le::u64(v, en)) else { continue };
                println!("  plays {:.3}..{:.3} s at rate {:.4}:", f32at(v, en + 0x28), f32at(v, en + 0x2c), f32at(v, en + 0x30));
                animation(v, anim, k == 0);
            }
        }
    }
}
