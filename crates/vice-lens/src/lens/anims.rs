//! Vice Lens: the game's animations, played on a skeleton. Clip
//! dictionaries are listed from the cache (`anim-dicts.tsv`, made by
//! Setup), in `anim.rpf` and the content packs; a clip is decoded and
//! sampled 30 times a second, each animated bone matched by its name to the
//! skeleton the viewer shows. A dictionary without clips holds motions
//! (see `rage::anim::Motion`): the clips of other dictionaries the game
//! plays together, listed here with those clips found by their names'
//! hashes.
//!
//! A clip whose animation has a block missing says so; its bones without
//! data stay at rest.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

use super::Lens;
use super::http::{Reply, Request, quote};
use super::search::Matcher;

/// A dictionary as listed: its archive, offset, name and clips.
struct Dict {
    archive: String,
    offset: u64,
    name: String,
    clips: u32,
}

/// The dictionaries, and their positions by name hash (motions name
/// dictionaries by it).
struct Index {
    dicts: Vec<Dict>,
    by_hash: HashMap<u32, usize>,
}

const ANIM: &str = "anim.rpf";

fn index() -> Option<&'static Index> {
    static I: OnceLock<Option<Index>> = OnceLock::new();
    I.get_or_init(|| {
        let text = std::fs::read_to_string(super::cache().join("anim-dicts.tsv")).ok()?;
        // offset, name, clips, whether every clip decodes, and (since the
        // content packs are listed too) the archive.
        let dicts: Vec<Dict> = text
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                let offset = u64::from_str_radix(f.first()?.trim_start_matches("0x"), 16).ok()?;
                let archive = f.get(4).filter(|a| !a.is_empty()).unwrap_or(&ANIM).to_string();
                Some(Dict { archive, offset, name: f.get(1)?.to_string(), clips: f.get(2)?.parse().ok()? })
            })
            .collect();
        let by_hash = dicts.iter().enumerate().map(|(i, d)| (rage::joaat(&d.name), i)).collect();
        Some(Index { dicts, by_hash })
    })
    .as_ref()
}

/// A dictionary's clips by key (`rage::anim::clip_key`): index, name,
/// length, whether it decodes whole.
type Keys = HashMap<u32, (usize, String, f32, bool)>;

/// Dictionaries read lately: archive, offset and virtual segment.
type Recent = Mutex<VecDeque<(String, u64, Arc<Vec<u8>>)>>;

/// A dictionary's virtual segment, the last few kept whole (a clip reads
/// several animations; a motion reads its clips' dictionaries).
fn dictionary(lens: &Lens, archive: &str, o: u64) -> Option<Arc<Vec<u8>>> {
    static CACHE: OnceLock<Recent> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some((_, _, v)) = cache.lock().unwrap().iter().find(|(a, k, _)| a == archive && *k == o) {
        return Some(v.clone());
    }
    let a = lens.archive(archive)?;
    let v = Arc::new(a.resource_at(o).ok()?.virt);
    let mut c = cache.lock().unwrap();
    c.push_back((archive.to_string(), o, v.clone()));
    if c.len() > 8 {
        c.pop_front();
    }
    Some(v)
}

/// The archive a request names (`a`), `anim.rpf` when none.
fn archive_of(r: &Request) -> &str {
    r.get("a").filter(|a| !a.is_empty()).unwrap_or(ANIM)
}

/// `,"a":"…"` for a dictionary outside `anim.rpf`, else nothing.
fn archive_field(a: &str) -> String {
    if a == ANIM { String::new() } else { format!(",\"a\":{}", quote(a)) }
}

impl Lens {
    /// Dictionaries whose names match `q`.
    pub(super) fn anims_list(&self, r: &Request) -> Reply {
        let Some(ix) = index() else { return Reply::missing("the animations are not listed yet: build the caches in Setup") };
        let m = match Matcher::from(r) {
            Ok(m) => m,
            Err(e) => return Reply::bad(&e),
        };
        let limit = r.num::<usize>("limit").unwrap_or(300);
        let hits: Vec<&Dict> = ix.dicts.iter().filter(|d| m.is_match(&d.name)).collect();
        let items: Vec<String> = hits.iter().take(limit).map(|d| format!("{{\"o\":\"{:x}\",\"name\":{},\"clips\":{}{}}}", d.offset, quote(&d.name), d.clips, archive_field(&d.archive))).collect();
        let with_clips = ix.dicts.iter().filter(|d| d.clips > 0).count();
        Reply::json(format!("{{\"total\":{},\"listed\":{},\"with_clips\":{with_clips},\"items\":[{}]}}", hits.len(), ix.dicts.len(), items.join(",")))
    }

    /// A dictionary's clips (name, length, whether it decodes whole) and
    /// motions: per motion its clips, found in their dictionaries by name
    /// hash (null where the dictionary is not listed), with each clip's
    /// parameters as first and last key (value, time).
    pub(super) fn anim_clips(&self, r: &Request) -> Reply {
        let Some(o) = r.hex("o") else { return Reply::bad("o") };
        let a = archive_of(r);
        let Some(v) = dictionary(self, a, o) else { return Reply::missing("not a clip dictionary") };
        let Some((name, clips)) = rage::anim::dictionary(&v) else { return Reply::missing("not a clip dictionary") };
        let items: Vec<String> = clips
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{{\"i\":{i},\"name\":{},\"length\":{:.3},\"whole\":{}}}", quote(c.name.trim_start_matches("pack:/")), c.length, rage::anim::readable(&v, c)))
            .collect();
        // The dictionaries motions name, each read once: clip key → (index,
        // name, length, whole).
        let mut found: HashMap<u32, Option<(&Dict, Keys)>> = HashMap::new();
        let motions: Vec<String> = rage::anim::motions(&v)
            .iter()
            .map(|m| {
                let sources: Vec<String> = m
                    .sources
                    .iter()
                    .map(|s| {
                        let d = found.entry(s.dictionary).or_insert_with(|| {
                            let d = &index()?.dicts[*index()?.by_hash.get(&s.dictionary)?];
                            let w = dictionary(self, &d.archive, d.offset)?;
                            let (_, cs) = rage::anim::dictionary(&w)?;
                            let keys = cs
                                .iter()
                                .enumerate()
                                .map(|(i, c)| (rage::anim::clip_key(&c.name), (i, c.name.trim_start_matches("pack:/").to_string(), c.length, rage::anim::readable(&w, c))))
                                .collect();
                            Some((d, keys))
                        });
                        let params: Vec<String> = s.params.iter().filter_map(|k| Some(format!("[{:.3},{:.3},{:.3}]", k.first()?.0, k.last()?.0, k.last()?.1))).collect();
                        let at = match d {
                            Some((d, keys)) => {
                                let clip = keys.get(&s.clip).map_or("null".into(), |(i, n, l, w)| format!("{{\"c\":{i},\"name\":{},\"length\":{l:.3},\"whole\":{w}}}", quote(n)));
                                format!("\"dict\":{},\"o\":\"{:x}\"{},\"clip\":{clip}", quote(&d.name), d.offset, archive_field(&d.archive))
                            }
                            None => "\"dict\":null,\"clip\":null".into(),
                        };
                        format!("{{\"key\":\"{:08x}\",\"dkey\":\"{:08x}\",{at},\"params\":[{}]}}", s.clip, s.dictionary, params.join(","))
                    })
                    .collect();
                format!("{{\"name\":{},\"sources\":[{}]}}", quote(m.name.trim_start_matches("pack:/")), sources.join(","))
            })
            .collect();
        Reply::json(format!("{{\"name\":{},\"items\":[{}],\"motions\":[{}]}}", quote(&name), items.join(","), motions.join(",")))
    }

    /// Clip `c` of dictionary `o` (in archive `a`) on the skeleton of model
    /// `skel`: per bone it moves (by index), its local rotation (x, y, z,
    /// w) and position at 30 samples a second, null where the clip leaves
    /// it.
    pub(super) fn anim_clip(&self, r: &Request) -> Reply {
        let (Some(o), Some(ci), Some(skel)) = (r.hex("o"), r.num::<usize>("c"), r.hex("skel")) else { return Reply::bad("o, c, skel") };
        let Some(v) = dictionary(self, archive_of(r), o) else { return Reply::missing("not a clip dictionary") };
        let Some((_, clips)) = rage::anim::dictionary(&v) else { return Reply::missing("not a clip dictionary") };
        let Some(c) = clips.get(ci) else { return Reply::missing("no such clip") };
        let Some(bones) = self.skeleton_bones(skel as u32) else { return Reply::missing("no such skeleton") };
        let by_hash: HashMap<u32, usize> = bones.iter().enumerate().map(|(i, b)| (rage::joaat(&b.name), i)).collect();
        let fps = 30.0;
        let (n, poses) = rage::anim::sample(&v, c, fps);
        let fmt = |x: f32| format!("{x:.4}");
        let mut bones: Vec<(usize, String)> = poses
            .iter()
            .filter_map(|(h, p)| {
                let i = *by_hash.get(h)?;
                let rot = p.iter().all(|x| x.rotation.is_some()).then(|| p.iter().flat_map(|x| x.rotation.unwrap()).map(fmt).collect::<Vec<_>>().join(","));
                let pos = p.iter().all(|x| x.position.is_some()).then(|| p.iter().flat_map(|x| x.position.unwrap()).map(fmt).collect::<Vec<_>>().join(","));
                if rot.is_none() && pos.is_none() {
                    return None;
                }
                Some((i, format!("{{\"i\":{i},\"r\":{},\"t\":{}}}", rot.map_or("null".into(), |s| format!("[{s}]")), pos.map_or("null".into(), |s| format!("[{s}]")))))
            })
            .collect();
        bones.sort_by_key(|b| b.0);
        let list: Vec<String> = bones.into_iter().map(|b| b.1).collect();
        Reply::json(format!(
            "{{\"name\":{},\"length\":{:.3},\"fps\":{fps},\"samples\":{n},\"whole\":{},\"bones\":[{}]}}",
            quote(c.name.trim_start_matches("pack:/")),
            c.length,
            rage::anim::readable(&v, c),
            list.join(",")
        ))
    }
}
