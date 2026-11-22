//! Building Vice Lens's caches from a copy of the game: what each archive
//! holds (the survey), where every drawable is, the drawable dictionaries,
//! the map's entities and batch instances, and the archetypes. The
//! examples, `vice-lens --build-caches` and the app's Setup all call these, so
//! the files they write agree.
//!
//! Each function reports progress as (done, total) through `progress`.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::archive::Archive;
use crate::{drawable, mapdata, pso, rbf, rsc, text, txd};

/// One line of a survey: a file's offset, stored size, pieces, and what it
/// looks like (`PSO region=… types`, `RBF0`, `TEXT`, `TXD`,
/// `RSC sig=… name=…`, `raw [..]`).
pub struct Row {
    pub offset: u64,
    pub size: usize,
    pub pieces: usize,
    pub kind: String,
}

impl Row {
    pub fn line(&self) -> String {
        format!("{:#x}\t{}\t{}\t{}\n", self.offset, self.size, self.pieces, self.kind)
    }
}

/// What a file looks like, in the survey's words.
pub fn kind(d: &[u8]) -> String {
    if d.starts_with(b"PSIN") {
        match pso::Pso::parse(d.to_vec()) {
            Ok(p) => {
                let types: Vec<String> = p.blocks.iter().skip(1).take(8).map(|b| format!("{:08x}:{}", b.type_hash, b.size)).collect();
                format!("PSO region={} {}", p.blocks[0].size, types.join(","))
            }
            Err(e) => format!("PSIN? {e}"),
        }
    } else if rbf::is_rbf(d) {
        "RBF0".into()
    } else if text::is_text_table(d) {
        "TEXT".into()
    } else if rsc::looks_like_resource(d) {
        if txd::probe(d).is_some() {
            "TXD".into()
        } else {
            let name = rsc::vptr(crate::le_u64(d, 0xa8))
                .filter(|&o| o < d.len())
                .map(|o| String::from_utf8_lossy(&d[o..d.len().min(o + 64)]).split('\0').next().unwrap_or("").to_string())
                .filter(|n| n.len() > 2 && n.is_ascii())
                .unwrap_or_default();
            format!("RSC sig={:016x} name={name}", crate::le_u64(d, 0x10))
        }
    } else {
        format!("raw {:02x?}", &d[..d.len().min(8)])
    }
}

/// Every file of an archive and what it looks like.
pub fn survey(a: &Archive, progress: &dyn Fn(u64, u64)) -> Vec<Row> {
    let mut streams = Vec::new();
    a.scan(|s, _| {
        if streams.len() % 8192 == 0 {
            progress(s.offset / 2, a.len());
        }
        streams.push(s);
        true
    });
    let files = Archive::files(&streams);
    let mut out = Vec::with_capacity(files.len());
    for (i, f) in files.iter().enumerate() {
        if i % 4096 == 0 {
            progress(a.len() / 2 + f.offset() / 2, a.len());
        }
        let Ok(d) = a.read_file(f) else { continue };
        out.push(Row { offset: f.offset(), size: d.len(), pieces: f.streams.len(), kind: kind(&d) });
    }
    out
}

/// Read a survey file back.
pub fn read_survey(text: &str) -> Vec<Row> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.splitn(4, '\t');
            let offset = u64::from_str_radix(it.next()?.trim_start_matches("0x"), 16).ok()?;
            let size = it.next()?.parse().ok()?;
            let pieces = it.next()?.parse().ok()?;
            Some(Row { offset, size, pieces, kind: it.next().unwrap_or("").to_string() })
        })
        .collect()
}

/// Run `work` over `items` on `threads` threads; this thread reports
/// progress until every item is done.
fn parallel<T: Sync>(items: &[T], threads: usize, progress: &dyn Fn(u64, u64), work: impl Fn(&T) + Sync) {
    let next = AtomicUsize::new(0);
    let done = AtomicU64::new(0);
    let total = items.len() as u64;
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                while let Some(it) = items.get(next.fetch_add(1, Ordering::Relaxed)) {
                    work(it);
                    done.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
        while done.load(Ordering::Relaxed) < total {
            progress(done.load(Ordering::Relaxed), total);
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    });
    progress(total, total);
}

/// Every resource of the survey whose root is a named drawable:
/// (offset, name).
pub fn drawables(a: &Archive, rows: &[Row], threads: usize, progress: &dyn Fn(u64, u64)) -> Vec<(u64, String)> {
    let offs: Vec<u64> = rows.iter().filter(|r| r.kind.starts_with("RSC")).map(|r| r.offset).collect();
    let out = Mutex::new(Vec::new());
    parallel(&offs, threads, progress, |&off| {
        // Cheap check on the first stream before reading it all.
        let Ok((first, _)) = a.inflate_at(off, 1 << 20) else { return };
        if first.len() < 0x100 || crate::le_u64(&first, 0) != 0 {
            return;
        }
        // Not a drawable (or fragment) when its LOD table lies in the first
        // stream and does not parse.
        let lod_at = |b: usize| rsc::vptr(crate::le_u64(&first, b + 0x50));
        let frag = rsc::vptr(crate::le_u64(&first, 0x18)).filter(|&f| f + 0x60 < first.len());
        let in_first = |b: usize| lod_at(b).is_some_and(|l| l + 16 < first.len());
        if drawable::base(&first).is_none() && in_first(0) && frag.is_none_or(in_first) {
            return;
        }
        let Ok(r) = a.resource_at(off) else { return };
        let Some(b) = drawable::base(&r.virt) else { return };
        let n = drawable::name(&r.virt, b);
        if !n.is_empty() {
            out.lock().unwrap().push((off, n));
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort();
    v
}

/// Every entry of every drawable dictionary (pgDictionary of drawables):
/// (offset, name hash, entry, the first entry's name on entry 0).
pub fn dictionaries(a: &Archive, rows: &[Row], threads: usize, progress: &dyn Fn(u64, u64)) -> Vec<(u64, u32, usize, String)> {
    let offs: Vec<u64> = rows.iter().filter(|r| r.kind.starts_with("RSC")).map(|r| r.offset).collect();
    let out = Mutex::new(Vec::new());
    let u64at = |b: &[u8], o: usize| b.get(o..o + 8).map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap()));
    let u16at = |b: &[u8], o: usize| b.get(o..o + 2).map_or(0, |s| u16::from_le_bytes(s.try_into().unwrap()) as usize);
    parallel(&offs, threads, progress, |&off| {
        let Ok((first, _)) = a.inflate_at(off, 1 << 20) else { return };
        let (hp, hn, ip, inn) = (rsc::vptr(u64at(&first, 0x20)), u16at(&first, 0x28), rsc::vptr(u64at(&first, 0x30)), u16at(&first, 0x38));
        let (Some(hp), Some(_)) = (hp, ip) else { return };
        if hn == 0 || hn != inn || hn > 4096 || hp + hn * 4 > first.len() {
            return;
        }
        let Ok(res) = a.resource_at(off) else { return };
        let Some(items) = rsc::vptr(u64at(&res.virt, 0x30)) else { return };
        let Some(d0) = rsc::vptr(u64at(&res.virt, items)) else { return };
        let name = drawable::name(&res.virt, d0);
        let mut o = out.lock().unwrap();
        for k in 0..hn {
            let h = res.virt.get(hp + k * 4..hp + k * 4 + 4).map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()));
            o.push((off, h, k, if k == 0 { name.clone() } else { String::new() }));
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort_by_key(|x| (x.0, x.2));
    v
}

/// Every named model of an archive: each drawable (or fragment) and each
/// entry of a drawable dictionary, as (offset, name). A dictionary appears
/// once per entry; `drawable::parse_model` finds the entry by its name's
/// hash. The packs keep most character, animal and prop models this way.
pub fn models(a: &Archive, rows: &[Row], threads: usize, progress: &dyn Fn(u64, u64)) -> Vec<(u64, String)> {
    let offs: Vec<u64> = rows.iter().filter(|r| r.kind.starts_with("RSC")).map(|r| r.offset).collect();
    let out = Mutex::new(Vec::new());
    let u16at = |b: &[u8], o: usize| b.get(o..o + 2).map_or(0, |s| u16::from_le_bytes(s.try_into().unwrap()) as usize);
    parallel(&offs, threads, progress, |&off| {
        let Ok(r) = a.resource_at(off) else { return };
        let v = &r.virt;
        let mut found = Vec::new();
        if let Some(b) = drawable::base(v) {
            found.push(drawable::name(v, b));
        } else {
            let n = u16at(v, 0x38);
            if n > 0 && n <= 4096 && n == u16at(v, 0x28) {
                found
                    .extend((0..n).filter_map(|k| drawable::dictionary_entry(v, k)).filter(|&b| drawable::parse_at(v, &r.phys, b).is_some_and(|d| !d.meshes.is_empty())).map(|b| drawable::name(v, b)));
            }
        }
        let found: Vec<(u64, String)> = found.into_iter().filter(|n| n.len() > 2 && n.is_ascii()).map(|n| (off, n)).collect();
        if !found.is_empty() {
            out.lock().unwrap().extend(found);
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort();
    v
}

/// Whether a parsable-data resource lists a block of one of `types` (read
/// from its first stream only).
fn has_block(a: &Archive, off: u64, types: &[u32]) -> bool {
    let Ok((first, _)) = a.inflate_at(off, 1 << 20) else { return false };
    let list = crate::le_u64(&first, 0x38).wrapping_sub(0x5000_0000) as usize;
    let n = if first.len() > 0x4e { u16::from_le_bytes([first[0x4c], first[0x4d]]) as usize } else { 0 };
    // A block list past the first stream: read the resource to know.
    if list + n * 16 > first.len() {
        return true;
    }
    (0..n).any(|i| first.get(list + i * 16..list + i * 16 + 4).is_some_and(|b| types.contains(&u32::from_le_bytes(b.try_into().unwrap()))))
}

/// Offsets of the parsable-data resources (`TEXT` in the survey).
pub fn map_data(rows: &[Row]) -> Vec<u64> {
    rows.iter().filter(|r| r.kind == "TEXT").map(|r| r.offset).collect()
}

/// Every placed entity, as the cache's 60-byte records (model, position,
/// rotation, scale XY and Z, parent, flags, LOD distance, children's LOD
/// distance, children); or, with `instances`, every batch instance as
/// 48-byte records (the same without the last three).
pub fn entities(a: &Archive, offsets: &[u64], instances: bool, threads: usize, progress: &dyn Fn(u64, u64)) -> Vec<u8> {
    let out = Mutex::new(Vec::new());
    let wanted: &[u32] = if instances { &[mapdata::BATCH] } else { &[mapdata::ENTITY, mapdata::BATCH] };
    parallel(offsets, threads, progress, |&off| {
        if !has_block(a, off, wanted) {
            return;
        }
        let Ok(r) = a.resource_at(off) else { return };
        let es = if instances { mapdata::instances(&r.virt, &r.phys).into_iter().filter_map(|(model, m)| mapdata::instance_entity(model, m)).collect() } else { mapdata::entities(&r.virt, &r.phys) };
        if es.is_empty() {
            return;
        }
        let mut buf = Vec::with_capacity(es.len() * 60);
        for e in &es {
            record(&mut buf, e, !instances);
        }
        out.lock().unwrap().push((off, buf));
    });
    // In archive order, as the tools always wrote them.
    let mut parts = out.into_inner().unwrap();
    parts.sort_by_key(|p| p.0);
    parts.into_iter().flat_map(|p| p.1).collect()
}

/// An entity as a cache record: 60 bytes, or 48 without the LOD fields.
fn record(buf: &mut Vec<u8>, e: &mapdata::Entity, lod: bool) {
    buf.extend_from_slice(&e.model.to_le_bytes());
    for v in e.pos.iter().chain(&e.rot).chain([&e.scale_xy, &e.scale_z]) {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    buf.extend_from_slice(&e.parent.to_le_bytes());
    buf.extend_from_slice(&e.flags.to_le_bytes());
    if lod {
        buf.extend_from_slice(&e.lod_dist.to_le_bytes());
        buf.extend_from_slice(&e.child_lod_dist.to_le_bytes());
        buf.extend_from_slice(&e.children.to_le_bytes());
    }
}

/// Every interior's entities where the world places it, as the cache's
/// 60-byte records (see `mapdata`: interiors are defined in their own
/// coordinates and placed by instances in other resources). Every resource
/// is read: a quick look at the first stream missed the large ones that
/// define interiors. With counts:
/// (records, [interiors defined, of them with no entities, placements,
/// placements whose interior was not found]).
pub fn interiors(a: &Archive, offsets: &[u64], threads: usize, progress: &dyn Fn(u64, u64)) -> (Vec<u8>, [usize; 4]) {
    let defs = Mutex::new(std::collections::HashMap::<u64, Vec<mapdata::Entity>>::new());
    let placed = Mutex::new(Vec::<(u64, mapdata::MloInstance)>::new());
    parallel(offsets, threads, progress, |&off| {
        let Ok(r) = a.resource_at(off) else { return };
        for m in mapdata::mlos(&r.virt, &r.phys) {
            defs.lock().unwrap().insert(m.hash, m.entities);
        }
        placed.lock().unwrap().extend(mapdata::mlo_instances(&r.virt, &r.phys).into_iter().map(|i| (off, i)));
    });
    let defs = defs.into_inner().unwrap();
    let mut placed = placed.into_inner().unwrap();
    // In archive order, so the cache comes out the same each time.
    placed.sort_by_key(|p| (p.0, p.1.place.id));
    let (mut buf, mut missing) = (Vec::new(), 0);
    for (_, inst) in &placed {
        let Some(es) = defs.get(&inst.hash) else {
            missing += 1;
            continue;
        };
        for e in es {
            record(&mut buf, &mapdata::place_in(&inst.place, e), true);
        }
    }
    let empty = defs.values().filter(|e| e.is_empty()).count();
    (buf, [defs.len(), empty, placed.len(), missing])
}

/// Every archetype as the cache's 16-byte records (model name joaat, LOD
/// distance, bounding radius, full-texture distance).
pub fn archetypes(a: &Archive, offsets: &[u64], threads: usize, progress: &dyn Fn(u64, u64)) -> Vec<u8> {
    let out = Mutex::new(Vec::new());
    parallel(offsets, threads, progress, |&off| {
        if !has_block(a, off, &[mapdata::ARCHETYPE]) {
            return;
        }
        let Ok(r) = a.resource_at(off) else { return };
        let found = mapdata::archetypes(&r.virt, &r.phys);
        if found.is_empty() {
            return;
        }
        let mut buf = Vec::with_capacity(found.len() * 16);
        for x in found {
            for w in [x.name, x.lod_dist.to_bits(), x.radius.to_bits(), x.hd_texture_dist.to_bits()] {
                buf.extend_from_slice(&w.to_le_bytes());
            }
        }
        out.lock().unwrap().push((off, buf));
    });
    let mut parts = out.into_inner().unwrap();
    parts.sort_by_key(|p| p.0);
    parts.into_iter().flat_map(|p| p.1).collect()
}
