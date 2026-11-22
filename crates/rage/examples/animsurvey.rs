//! Every clip dictionary in an archive: its name, clips, and how many of
//! its animations use the incremental curves (type 10) and how many move
//! with the earlier types alone (raw, quantised, indirect, linear floats).
//!
//! `animsurvey <archive> > out.tsv`
use rage::{Archive, le, rsc::vptr};

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let ar = Archive::open(&path).unwrap();
    let mut streams = Vec::new();
    ar.scan(|s, _| {
        streams.push(s);
        true
    });
    let (mut dicts, mut readable) = (0, 0);
    for f in Archive::files(&streams) {
        let Ok(d) = ar.read_file(&f) else { continue };
        if !rage::rsc::looks_like_resource(&d) {
            continue;
        }
        let v = ar.resource_at(f.offset()).map(|r| r.virt).unwrap_or(d);
        if v.len() < 0x80 {
            continue;
        }
        let (Some(name), Some(map)) = (vptr(le::u64(&v, 0x38)), vptr(le::u64(&v, 0x48))) else { continue };
        let (buckets, clips) = (le::u16(&v, 0x40) as usize, le::u16(&v, 0x42) as usize);
        if buckets == 0 || clips == 0 || map + buckets * 8 > v.len() || name >= v.len() {
            continue;
        }
        let dict = String::from_utf8_lossy(le::cstr(&v, name)).into_owned();
        if !dict.is_ascii() || dict.len() < 3 {
            continue;
        }
        dicts += 1;
        // Animations: blocks' channel counts by type.
        let (mut with10, mut known, mut still, mut anims) = (0, 0, 0, std::collections::HashSet::new());
        let mut first_known = String::new();
        for b in 0..buckets {
            let mut e = vptr(le::u64(&v, map + b * 8));
            let mut guard = 0;
            while let Some(eo) = e {
                guard += 1;
                if guard > 4096 || eo + 0x18 > v.len() {
                    break;
                }
                e = vptr(le::u64(&v, eo + 0x10));
                let Some(c) = vptr(le::u64(&v, eo + 8)).filter(|&c| c + 0x80 <= v.len()) else { continue };
                let Some(lists) = vptr(le::u64(&v, c + 0x68)).filter(|&l| l + 0x18 <= v.len()) else { continue };
                let Some(entries) = vptr(le::u64(&v, lists)) else { continue };
                for k in 0..(le::u16(&v, lists + 8) as usize).min(16) {
                    let en = entries + k * 0x38;
                    let Some(a) = vptr(le::u64(&v, en)).filter(|&a| a + 0x80 <= v.len()) else { continue };
                    if !anims.insert(a) {
                        continue;
                    }
                    let Some(list) = vptr(le::u64(&v, a + 0x50)) else { continue };
                    let (mut t10, mut moving) = (0u32, 0u32);
                    for bi in 0..(le::u16(&v, a + 0x58) as usize).min(64) {
                        let Some(bl) = vptr(le::u64(&v, list + bi * 8)).filter(|&x| x + 0x20 <= v.len()) else { continue };
                        let (len, fo) = (le::u32(&v, bl + 4) as usize, le::u32(&v, bl + 12) as usize);
                        let (nf, fl) = (le::u16(&v, bl + 0x16) as usize, le::u16(&v, bl + 0x18) as usize);
                        let at = bl + 0x20 + fo + fl * nf;
                        if fo > len || at + 22 > v.len() {
                            continue;
                        }
                        let n = |t: usize| le::u16(&v, at + t * 2) as u32;
                        t10 += n(10) + n(9);
                        moving += n(3) + n(4) + n(5) + n(6);
                    }
                    if t10 > 0 {
                        with10 += 1;
                    } else if moving > 0 {
                        known += 1;
                        if first_known.is_empty() {
                            let cn = vptr(le::u64(&v, c + 0x28)).map(|o| String::from_utf8_lossy(le::cstr(&v, o)).into_owned()).unwrap_or_default();
                            first_known = format!("{cn} anim {a:#x}");
                        }
                    } else {
                        still += 1;
                    }
                }
            }
        }
        if known > 0 {
            readable += 1;
        }
        println!("{:#x}\t{dict}\t{clips}\t{}\t{with10}\t{known}\t{still}\t{first_known}", f.offset(), anims.len());
    }
    eprintln!("{dicts} dictionaries, {readable} with animations that move without type 10");
}
