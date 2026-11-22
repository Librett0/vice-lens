//! Vice Lens: a local web app for researching the game's files: the map
//! with every placement, models in 3D with their textures, archives byte
//! by byte, and the text tables. Vice Lens serves it on 127.0.0.1; it reads
//! the user's own copy of the game and Vice Lens's caches and changes
//! neither.

mod anims;
mod audio;
mod bink;
mod creatures;
mod export;
mod http;
mod media;
mod search;
mod skin;
mod vehicles;
mod wardrobe;

use std::collections::{HashMap, VecDeque};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use http::{Reply, Request};
use rage::Archive;

use crate::{map, world};

const INDEX: &str = include_str!("../../lens/index.html");
const CSS: &str = include_str!("../../lens/lens.css");
const JS: &str = include_str!("../../lens/lens.js");

pub use http::quote;

/// Serve Vice Lens on 127.0.0.1:`port` until the process ends.
pub fn serve(game: PathBuf, port: u16) {
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            log::error!("cannot listen on 127.0.0.1:{port}: {e}");
            return;
        }
    };
    log::info!("Vice Lens: http://127.0.0.1:{port}");
    serve_on(listener, game);
}

/// Serve Vice Lens on a listener already bound (the desktop app's).
pub fn serve_on(listener: TcpListener, game: PathBuf) {
    let lens = Arc::new(Lens::new(game));
    for stream in listener.incoming().flatten() {
        let lens = lens.clone();
        std::thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));
            let Some(req) = http::read(&stream) else { return };
            let t = std::time::Instant::now();
            let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| lens.route(&req))).unwrap_or_else(|_| Reply::bad("the request failed; see the log"));
            log::debug!("{} {} {} in {:.1?}", req.method, req.path, reply.status, t.elapsed());
            http::write(stream, reply);
        });
    }
}

/// One file of an archive as a survey lists it.
struct Row {
    offset: u64,
    size: u64,
    kind: String,
    name: String,
}

/// A file read whole for the byte view: the file as stored after
/// inflation, and, for resources, the virtual and physical segments.
struct Blob {
    file: Vec<u8>,
    virt: Vec<u8>,
    phys: Vec<u8>,
}

/// The map's placements and what is known of their models.
struct World {
    index: world::ModelIndex,
    /// Entity records as the `entities` tool writes them (60 bytes).
    ents: Vec<u8>,
    /// 256 m cells → entities.
    grid: HashMap<(i32, i32), Vec<u32>>,
    by_model: HashMap<u32, Vec<u32>>,
    archetypes: HashMap<u32, (f32, f32)>,
    /// (archive, offset) → model name.
    names_at: HashMap<(String, u64), String>,
}

const CELL: f32 = 256.0;

struct Ent {
    model: u32,
    pos: [f32; 3],
    rot: [f32; 4],
    scale: [f32; 2],
    parent: i32,
    flags: u32,
    lod: f32,
    child: f32,
    children: u32,
}

impl World {
    fn len(&self) -> usize {
        self.ents.len() / 60
    }
    fn ent(&self, i: usize) -> Ent {
        let r = &self.ents[i * 60..i * 60 + 60];
        let u = |o: usize| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let f = |o: usize| f32::from_bits(u(o));
        Ent { model: u(0), pos: [f(4), f(8), f(12)], rot: [f(16), f(20), f(24), f(28)], scale: [f(32), f(36)], parent: u(40) as i32, flags: u(44), lod: f(48), child: f(52), children: u(56) }
    }
    fn name(&self, h: u32) -> String {
        self.index.names.get(&h).cloned().unwrap_or_else(|| format!("{h:08x}"))
    }
    /// 0 detailed, 1 LOD stand-in, 2 super-LOD, 3 water.
    fn kind(&self, e: &Ent) -> u8 {
        if self.index.water.contains(&e.model) {
            return 3;
        }
        if e.children > 0 {
            return if e.child < 250.0 { 1 } else { 2 };
        }
        let n = self.index.names.get(&e.model).map(String::as_str).unwrap_or("");
        if n.contains("slod") || self.index.slod.contains(&e.model) {
            2
        } else if n.ends_with("_lod") || self.index.lod.contains(&e.model) {
            1
        } else {
            0
        }
    }
}

struct Lens {
    game: PathBuf,
    archives: Mutex<HashMap<String, Arc<Archive>>>,
    /// Loaded once their caches exist (tried again until then).
    /// The map style last asked for: its name, the file's time, and the
    /// image with its halvings (level k is 2^k times smaller).
    map: Mutex<Option<(String, std::time::SystemTime, Arc<Vec<map::MapImage>>)>>,
    tiles: Mutex<(HashMap<(String, u32, u32, u32), Arc<Vec<u8>>>, VecDeque<(String, u32, u32, u32)>)>,
    world: Mutex<Option<Arc<World>>>,
    textures: OnceLock<world::Textures>,
    models: Mutex<VecDeque<(u32, Arc<world::Model>)>>,
    text: OnceLock<search::Text>,
    surveys: Mutex<HashMap<String, Arc<Vec<Row>>>>,
    blobs: Mutex<VecDeque<((String, u64), Arc<Blob>)>>,
    /// Background work the UI shows: (what, done, total, finished).
    jobs: Mutex<Vec<(String, u64, u64, bool)>>,
    texdicts: OnceLock<Arc<Vec<media::TexDict>>>,
    movies: OnceLock<Arc<Vec<media::Movie>>>,
    banks: OnceLock<Arc<Vec<audio::Bank>>>,
    /// The FFmpeg found, and whether it decodes Bink 2.
    ffmpeg: Mutex<Option<media::Ffmpeg>>,
}

fn cache() -> PathBuf {
    crate::dirs_cache()
}

impl Lens {
    fn new(game: PathBuf) -> Lens {
        Lens {
            game,
            archives: Default::default(),
            map: Default::default(),
            tiles: Default::default(),
            world: Default::default(),
            textures: OnceLock::new(),
            models: Default::default(),
            text: OnceLock::new(),
            surveys: Default::default(),
            blobs: Default::default(),
            jobs: Default::default(),
            texdicts: OnceLock::new(),
            movies: OnceLock::new(),
            banks: OnceLock::new(),
            ffmpeg: Default::default(),
        }
    }

    fn route(self: &Arc<Self>, r: &Request) -> Reply {
        match (r.method.as_str(), r.path.as_str()) {
            (_, "/") | (_, "/index.html") => Reply::bytes("text/html; charset=utf-8", INDEX.as_bytes().to_vec(), false),
            (_, "/lens.css") => Reply::bytes("text/css; charset=utf-8", CSS.as_bytes().to_vec(), false),
            (_, "/lens.js") => Reply::bytes("text/javascript; charset=utf-8", JS.as_bytes().to_vec(), false),
            (_, "/api/status") => self.status(),
            (_, "/api/map/meta") => self.map_meta(r),
            (_, "/api/map/tile") => self.map_tile(r),
            (_, "/api/map/placements") => self.placements(r),
            (_, "/api/placement") => self.placement(r),
            (_, "/api/models") => self.models_search(r),
            (_, "/api/model") => self.model_info(r),
            (_, "/api/model/mesh") => self.model_mesh(r),
            (_, "/api/skeleton") => self.skeleton(r),
            (_, "/api/anims") => self.anims_list(r),
            (_, "/api/anim/clips") => self.anim_clips(r),
            (_, "/api/anim/clip") => self.anim_clip(r),
            (_, "/api/model/places") => self.model_places(r),
            (_, "/api/model/texture") => self.model_texture(r),
            (_, "/api/model/slots") => self.model_slots(r),
            (_, "/api/model/slot") => self.model_slot(r),
            (_, "/api/model/family") => self.model_family(r),
            (_, "/api/model/export") => self.model_export(r),
            (_, "/api/vehicles") => self.vehicles_list(),
            (_, "/api/vehicle") => self.vehicle(r),
            (_, "/api/wheels") => self.wheels_list(),
            (_, "/api/wardrobe") => self.wardrobe(),
            (_, "/api/creatures") => self.creatures(),
            (_, "/api/archives") => self.archives_list(),
            (_, "/api/archive/rows") => self.archive_rows(r),
            ("POST", "/api/archive/index") => self.archive_index(r),
            (_, "/api/jobs") => self.jobs(),
            (_, "/api/blob") => self.blob_info(r),
            (_, "/api/bytes") => self.bytes(r),
            (_, "/api/file") => self.file_part(r),
            (_, "/api/text") => self.text_search(r),
            ("POST", "/api/setup") => self.setup(r),
            ("POST", "/api/capture") => capture(r),
            ("POST", "/api/explore") => self.explore(r),
            ("POST", "/api/caches/build") => self.build_caches(),
            (_, "/api/settings") => self.settings(r),
            (_, "/api/textures") => self.textures_list(r),
            (_, "/api/texdict") => self.texdict(r),
            (_, "/api/texture") => self.texture(r),
            (_, "/api/movies") => self.movies_list(r),
            ("POST", "/api/movie/prepare") => self.movie_prepare(r),
            (_, "/api/movie/poster") => self.movie_poster(r),
            (_, "/api/movie/frame") => self.movie_frame(r),
            (_, "/api/movie/raw") => self.movie_raw(r),
            (_, "/api/media") => self.media(r),
            (_, "/api/banks") => self.banks_list(r),
            (_, "/api/bank") => self.bank(r),
            (_, "/api/sound") => self.sound(r),
            // The page reports measurements (frame rate) to the log.
            (_, "/api/probe") => {
                let mut q: Vec<String> = r.query.iter().map(|(k, v)| format!("{k}={v}")).collect();
                q.sort();
                log::info!("lens probe: {}", q.join(" "));
                Reply::json("{}".into())
            }
            _ => Reply::missing("no such page"),
        }
    }

    /// An archive of the game folder by name: one at its top, or a content
    /// pack's (`pack/story000/story000.rpf`); no other path.
    fn archive(&self, name: &str) -> Option<Arc<Archive>> {
        if name.contains("..") || (name.contains('/') && !crate::caches::model_archives(&self.game).iter().any(|a| a == name)) {
            return None;
        }
        let mut a = self.archives.lock().unwrap();
        if let Some(x) = a.get(name) {
            return Some(x.clone());
        }
        let x = Arc::new(Archive::open(self.game.join(name)).ok()?);
        a.insert(name.to_string(), x.clone());
        Some(x)
    }

    fn rpf_files(&self) -> Vec<(String, u64)> {
        let mut v: Vec<(String, u64)> = std::fs::read_dir(&self.game)
            .map(|rd| rd.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "rpf")).map(|e| (e.file_name().to_string_lossy().into_owned(), e.metadata().map_or(0, |m| m.len()))).collect())
            .unwrap_or_default();
        v.sort();
        v
    }

    // --- status and setup ---

    fn status(&self) -> Reply {
        let archives: Vec<String> = self.rpf_files().iter().map(|(n, b)| format!("{{\"name\":{},\"bytes\":{b}}}", quote(n))).collect();
        let caches = [
            ("Model index", "models.tsv", "names and places of drawables"),
            ("Missed models", "models-missed.tsv", "drawables the first pass skipped"),
            ("Dictionaries", "dicts-levels.tsv", "LOD and super-LOD models in dictionaries"),
            ("Entities", "entities.bin", "every placement on the map"),
            ("Batch instances", "instances.bin", "trees, grass and small props"),
            ("Archetypes", "archetypes.bin", "LOD distance and size of each model"),
            ("Interiors", "interiors.bin", "shops, flats and offices where the world places them"),
            ("Map", "map-relief.png", "the island seen from above: satellite, clay and relief, 2 m a pixel"),
            ("Pack models", "models-packs.tsv", "characters, animals and props in the packs"),
        ];
        let caches: Vec<String> = caches
            .iter()
            .map(|(what, file, about)| {
                let m = std::fs::metadata(cache().join(file)).ok();
                format!("{{\"what\":{},\"file\":{},\"about\":{},\"present\":{},\"bytes\":{}}}", quote(what), quote(file), quote(about), m.is_some(), m.map_or(0, |m| m.len()))
            })
            .collect();
        let ok = self.game.join("levels.rpf").exists();
        Reply::json(format!(
            "{{\"game\":{},\"ok\":{ok},\"cache\":{},\"archives\":[{}],\"caches\":[{}],\"version\":{},\"os\":\"{}\"}}",
            quote(&self.game.to_string_lossy()),
            quote(&cache().to_string_lossy()),
            archives.join(","),
            caches.join(","),
            quote(env!("CARGO_PKG_VERSION")),
            std::env::consts::OS
        ))
    }

    fn setup(&self, r: &Request) -> Reply {
        let Some(game) = r.get("game").map(str::trim).filter(|g| !g.is_empty()) else { return Reply::bad("no folder given") };
        let p = PathBuf::from(game);
        if !p.join("levels.rpf").exists() {
            return Reply::json(format!("{{\"ok\":false,\"error\":{}}}", quote("levels.rpf is not in that folder")));
        }
        match crate::save_game_dir(&p) {
            Ok(()) => Reply::json("{\"ok\":true,\"restart\":true}".into()),
            Err(e) => Reply::json(format!("{{\"ok\":false,\"error\":{}}}", quote(&e.to_string()))),
        }
    }

    fn jobs(&self) -> Reply {
        let j = self.jobs.lock().unwrap();
        let items: Vec<String> = j.iter().map(|(w, d, t, f)| format!("{{\"what\":{},\"done\":{d},\"total\":{t},\"finished\":{f}}}", quote(w))).collect();
        Reply::json(format!("{{\"items\":[{}]}}", items.join(",")))
    }

    // --- the map ---

    /// One style of the map and its halvings, loaded again when the
    /// file changes (the map made anew); only the last style asked for is
    /// kept (a whole style is some 450 MB).
    fn the_map(&self, style: &str) -> Option<Arc<Vec<map::MapImage>>> {
        let file = map::STYLES.iter().find(|s| s.0 == style)?.1;
        let time = std::fs::metadata(cache().join(file)).and_then(|m| m.modified()).ok()?;
        let mut m = self.map.lock().unwrap();
        if let Some((st, t, levels)) = m.as_ref()
            && st == style
            && *t == time
        {
            return Some(levels.clone());
        }
        *m = None;
        let mut levels = vec![map::MapImage::load_style(style)?];
        while levels.len() < 7 && levels.last().is_some_and(|l| l.w.max(l.h) > 256) {
            let next = levels.last().unwrap().halved();
            levels.push(next);
        }
        let levels = Arc::new(levels);
        self.tiles.lock().unwrap().0.retain(|k, _| k.0 != style);
        *m = Some((style.to_string(), time, levels.clone()));
        Some(levels)
    }

    fn map_style<'a>(&self, r: &'a Request) -> &'a str {
        r.get("st").filter(|s| map::STYLES.iter().any(|x| x.0 == *s)).unwrap_or("satellite")
    }

    fn map_meta(&self, r: &Request) -> Reply {
        let styles: Vec<String> = map::MapImage::styles().iter().map(|s| quote(s)).collect();
        let levels = self.the_map(self.map_style(r));
        // The image's version, so a browser fetches the tiles of a map
        // made anew instead of those it kept.
        let v = self.map.lock().unwrap().as_ref().and_then(|m| m.1.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
        match levels.as_ref().and_then(|l| l.first()) {
            Some(m) => {
                Reply::json(format!("{{\"ok\":true,\"west\":{},\"north\":{},\"per_px\":{},\"w\":{},\"h\":{},\"styles\":[{}],\"v\":{v}}}", m.west, m.north, m.per_px, m.w, m.h, styles.join(",")))
            }
            None => Reply::json(format!("{{\"ok\":false,\"styles\":[{}],\"command\":{}}}", styles.join(","), quote("./vice-lens.sh --make-map"))),
        }
    }

    /// A 256-pixel tile of the map shrunk `s` times (a power of two), in
    /// style `st`, cut from the halving nearest `s`.
    fn map_tile(&self, r: &Request) -> Reply {
        let (Some(s), Some(x), Some(y)) = (r.num::<u32>("s"), r.num::<u32>("x"), r.num::<u32>("y")) else { return Reply::bad("s, x, y") };
        if !s.is_power_of_two() || s > 64 {
            return Reply::bad("s must be 1, 2, 4 … 64");
        }
        let style = self.map_style(r);
        let key = (style.to_string(), s, x, y);
        if let Some(t) = self.tiles.lock().unwrap().0.get(&key) {
            return Reply::bytes("image/png", t.to_vec(), true);
        }
        let Some(levels) = self.the_map(style) else { return Reply::missing("no map yet") };
        let k = (s.trailing_zeros() as usize).min(levels.len() - 1);
        let m = &levels[k];
        // What is left of the shrinking after the halving.
        let s = s >> k;
        const T: u32 = 256;
        let mut px = vec![0u8; (T * T * 4) as usize];
        for ty in 0..T {
            for tx in 0..T {
                let (bx, by) = ((x * T + tx) * s, (y * T + ty) * s);
                if bx >= m.w || by >= m.h {
                    continue;
                }
                let mut acc = [0u32; 4];
                let mut n = 0;
                for dy in 0..s.min(m.h - by) {
                    let row = ((by + dy) * m.w + bx) as usize * 4;
                    for dx in 0..s.min(m.w - bx) {
                        let p = row + dx as usize * 4;
                        for (c, a) in acc.iter_mut().enumerate() {
                            *a += m.rgba[p + c] as u32;
                        }
                        n += 1;
                    }
                }
                let o = ((ty * T + tx) * 4) as usize;
                for c in 0..4 {
                    px[o + c] = (acc[c] / n.max(1)) as u8;
                }
            }
        }
        let png = encode_png(&px, T, T);
        let mut t = self.tiles.lock().unwrap();
        t.0.insert(key.clone(), Arc::new(png.clone()));
        t.1.push_back(key);
        while t.1.len() > 1024 {
            if let Some(k) = t.1.pop_front() {
                t.0.remove(&k);
            }
        }
        Reply::bytes("image/png", png, true)
    }

    fn the_world(&self) -> Option<Arc<World>> {
        let mut slot = self.world.lock().unwrap();
        if slot.is_none() {
            *slot = (|| {
                let t = std::time::Instant::now();
                let mut ents = std::fs::read(cache().join("entities.bin")).ok()?;
                let local = world::local_records(&cache().join("entities.bin"), 60);
                // Interiors where the world places them follow the map's own
                // placements (whose interior records, local, are left out).
                ents.extend(std::fs::read(cache().join("interiors.bin")).unwrap_or_default());
                let mut index = world::ModelIndex::load(&crate::caches::MODEL_LISTS.map(|f| cache().join(f)));
                index.add_dicts(&cache().join("dicts-levels.tsv"), "levels.rpf");
                let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
                let mut by_model: HashMap<u32, Vec<u32>> = HashMap::new();
                for (i, r) in ents.as_chunks::<60>().0.iter().enumerate() {
                    let f = |o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
                    let (x, y) = (f(4), f(8));
                    // Interiors in their own coordinates would pile up at the origin.
                    if !(x.is_finite() && y.is_finite()) || local.contains(&(i as u32)) {
                        continue;
                    }
                    grid.entry(((x / CELL).floor() as i32, (y / CELL).floor() as i32)).or_default().push(i as u32);
                    by_model.entry(u32::from_le_bytes(r[0..4].try_into().unwrap())).or_default().push(i as u32);
                }
                let mut names_at = HashMap::new();
                for f in crate::caches::MODEL_LISTS {
                    for line in std::fs::read_to_string(cache().join(f)).unwrap_or_default().lines() {
                        let mut it = line.split('\t');
                        if let (Some(a), Some(o), Some(n)) = (it.next(), it.next(), it.next())
                            && let Ok(o) = u64::from_str_radix(o.trim_start_matches("0x"), 16)
                        {
                            names_at.insert((a.to_string(), o), n.to_string());
                        }
                    }
                }
                let archetypes = world::load_archetypes(&cache().join("archetypes.bin"));
                log::info!("lens: {} placements, {} models known, in {:.1?}", ents.len() / 60, index.names.len(), t.elapsed());
                Some(World { index, ents, grid, by_model, archetypes, names_at })
            })()
            .map(Arc::new);
        }
        slot.clone()
    }

    /// Placements in a box, at most `limit`, spread evenly when there are
    /// more: [entity, x, y, kind, name].
    fn placements(&self, r: &Request) -> Reply {
        let Some(w) = self.the_world() else { return Reply::missing("no entity cache") };
        let (Some(x0), Some(y0), Some(x1), Some(y1)) = (r.num::<f32>("x0"), r.num::<f32>("y0"), r.num::<f32>("x1"), r.num::<f32>("y1")) else { return Reply::bad("x0, y0, x1, y1") };
        let kinds: Vec<u8> = r.get("kinds").unwrap_or("0,1,2,3").split(',').filter_map(|k| k.parse().ok()).collect();
        let limit = r.num::<usize>("limit").unwrap_or(3000).min(20000);
        let mut hits = Vec::new();
        let c = |v: f32| (v / CELL).floor() as i32;
        if (x1 - x0) * (y1 - y0) > 4.0e8 {
            return Reply::json("{\"total\":0,\"items\":[],\"too_wide\":true}".into());
        }
        for cx in c(x0)..=c(x1) {
            for cy in c(y0)..=c(y1) {
                for &i in w.grid.get(&(cx, cy)).map(Vec::as_slice).unwrap_or(&[]) {
                    let e = w.ent(i as usize);
                    if e.pos[0] < x0 || e.pos[0] > x1 || e.pos[1] < y0 || e.pos[1] > y1 {
                        continue;
                    }
                    let k = w.kind(&e);
                    if kinds.contains(&k) {
                        hits.push((i, e.pos, k, e.model));
                    }
                }
            }
        }
        let total = hits.len();
        let step = total.div_ceil(limit).max(1);
        let items: Vec<String> = hits.iter().step_by(step).map(|(i, p, k, m)| format!("[{i},{:.1},{:.1},{k},{}]", p[0], p[1], quote(&w.name(*m)))).collect();
        Reply::json(format!("{{\"total\":{total},\"items\":[{}]}}", items.join(",")))
    }

    fn placement(&self, r: &Request) -> Reply {
        let Some(w) = self.the_world() else { return Reply::missing("no entity cache") };
        let Some(i) = r.num::<usize>("i").filter(|i| *i < w.len()) else { return Reply::bad("i") };
        let e = w.ent(i);
        let arch = w.archetypes.get(&e.model).map_or("null".into(), |(l, rad)| format!("{{\"lod\":{l},\"radius\":{rad}}}"));
        Reply::json(format!(
            "{{\"i\":{i},\"model\":\"{:08x}\",\"name\":{},\"kind\":{},\"pos\":[{},{},{}],\"rot\":[{},{},{},{}],\"scale\":[{},{}],\"parent\":{},\"flags\":\"{:08x}\",\"lod\":{},\"child_lod\":{},\"children\":{},\"archetype\":{arch},\"file\":{}}}",
            e.model,
            quote(&w.name(e.model)),
            w.kind(&e),
            e.pos[0],
            e.pos[1],
            e.pos[2],
            e.rot[0],
            e.rot[1],
            e.rot[2],
            e.rot[3],
            e.scale[0],
            e.scale[1],
            e.parent,
            e.flags,
            e.lod,
            e.child,
            e.children,
            self.file_of(&w, e.model)
        ))
    }

    /// Where a model's drawable is: {archive, offset, entry} or null.
    fn file_of(&self, w: &World, h: u32) -> String {
        match w.index.by_hash.get(&h) {
            Some(&(a, o, entry)) => format!("{{\"archive\":{},\"offset\":\"{o:#x}\",\"entry\":{}}}", quote(&w.index.archives[a]), entry.map_or("null".into(), |k| k.to_string())),
            None => "null".into(),
        }
    }

    // --- models ---

    /// Models by name (as the search options say), hash, or the name a
    /// hash was made from; `placed` (1 placed only, 0 unplaced only),
    /// `kind` (see `model_kind`), `lods=0` leaves out LOD stand-ins, `sort`
    /// by best match, name or placements.
    fn models_search(&self, r: &Request) -> Reply {
        let Some(w) = self.the_world() else { return Reply::missing("no model index") };
        let m = match search::Matcher::from(r) {
            Ok(m) => m,
            Err(e) => return Reply::bad(&e),
        };
        let q = r.get("q").unwrap_or("").trim().to_lowercase();
        let limit = r.num::<usize>("limit").unwrap_or(200).min(5000);
        let hash = u32::from_str_radix(q.trim_start_matches("0x"), 16).ok().filter(|_| q.len() >= 6);
        let by_name = rage::joaat(&q);
        let placed = r.get("placed").filter(|p| !p.is_empty());
        let kind = r.get("kind").unwrap_or("");
        let lods = r.get("lods") != Some("0");
        let mut items: Vec<(u32, &String, usize)> = w
            .index
            .names
            .iter()
            .filter_map(|(h, n)| {
                if !(m.is_match(n) || Some(*h) == hash || (!q.is_empty() && by_name == *h)) {
                    return None;
                }
                let count = w.by_model.get(h).map_or(0, Vec::len);
                let keep = match placed {
                    Some("1") => count > 0,
                    Some("0") => count == 0,
                    _ => true,
                } && (lods || !(is_lod(n) || w.index.lod.contains(h) || w.index.slod.contains(h)))
                    && (kind.is_empty() || model_kind(n) == kind);
                keep.then_some((*h, n, count))
            })
            .collect();
        let total = items.len();
        // Best: exact names first, then those the match starts, shorter
        // before longer.
        let best = |x: &(u32, &String, usize)| (x.1.as_str() != q, m.start(x.1) != Some(0), x.1.len());
        let order = |a: &(u32, &String, usize), b: &(u32, &String, usize)| match r.get("sort").unwrap_or("best") {
            "name" => a.1.cmp(b.1),
            "placed" => b.2.cmp(&a.2).then_with(|| a.1.cmp(b.1)),
            _ => best(a).cmp(&best(b)).then_with(|| a.1.cmp(b.1)),
        };
        if items.len() > limit {
            items.select_nth_unstable_by(limit, order);
            items.truncate(limit);
        }
        items.sort_by(order);
        let rows: Vec<String> = items.iter().map(|(h, n, c)| format!("{{\"h\":\"{h:08x}\",\"name\":{},\"placed\":{c}}}", quote(n))).collect();
        Reply::json(format!("{{\"total\":{total},\"items\":[{}]}}", rows.join(",")))
    }

    fn load(&self, h: u32) -> Option<Arc<world::Model>> {
        if let Some((_, m)) = self.models.lock().unwrap().iter().find(|(k, _)| *k == h) {
            return Some(m.clone());
        }
        let w = self.the_world()?;
        let &(a, off, entry) = w.index.by_hash.get(&h)?;
        let archive = self.archive(&w.index.archives[a])?;
        let textures = self.textures.get_or_init(|| world::Textures::open(&self.game));
        let m = Arc::new(world::load_model(&archive, off, entry, h, true, false, textures, &|_| true)?);
        let mut c = self.models.lock().unwrap();
        c.push_back((h, m.clone()));
        if c.len() > 6 {
            c.pop_front();
        }
        Some(m)
    }

    fn model_info(&self, r: &Request) -> Reply {
        let Some(w) = self.the_world() else { return Reply::missing("no model index") };
        let Some(h) = r.hex("h").map(|h| h as u32) else { return Reply::bad("h") };
        let Some(m) = self.load(h) else { return Reply::missing("the model could not be read") };
        let d = &m.drawable;
        let meshes: Vec<String> = d.meshes.iter().map(|me| format!("{{\"verts\":{},\"tris\":{},\"shader\":{}}}", me.positions.len(), me.indices.len() / 3, me.shader)).collect();
        let shaders: Vec<String> = m
            .textures
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let size = t.pixels.as_ref().map(|p| format!("[{},{}]", p.1, p.2)).or(t.small.as_ref().map(|s| format!("[{},{}]", s.0, s.1))).unwrap_or("null".into());
                let cands: Vec<String> = d
                    .shaders
                    .get(i)
                    .into_iter()
                    .flatten()
                    .map(|c| {
                        quote(&match c {
                            rage::drawable::TexRef::Key(k) => format!("texture pack key {k:016x}"),
                            rage::drawable::TexRef::Embedded(at) => format!("embedded at {at:#x}"),
                        })
                    })
                    .collect();
                let bucket = match d.buckets.get(i).copied().unwrap_or_default() {
                    rage::drawable::Bucket::Opaque => "opaque",
                    rage::drawable::Bucket::Alpha => "glass",
                    rage::drawable::Bucket::Decal => "decal",
                    rage::drawable::Bucket::Cutout => "cut-out",
                    rage::drawable::Bucket::Water => "water",
                };
                format!(
                    "{{\"key\":{},\"size\":{size},\"full\":{},\"bucket\":\"{bucket}\",\"candidates\":[{}]}}",
                    t.key.map_or("null".into(), |k| format!("\"{k:016x}\"")),
                    t.pixels.is_some(),
                    cands.join(",")
                )
            })
            .collect();
        let placed = w.by_model.get(&h).map(Vec::as_slice).unwrap_or(&[]);
        let places: Vec<String> = placed
            .iter()
            .take(500)
            .map(|&i| {
                let e = w.ent(i as usize);
                format!("[{i},{:.1},{:.1},{:.1}]", e.pos[0], e.pos[1], e.pos[2])
            })
            .collect();
        let arch = w.archetypes.get(&h).map_or("null".into(), |(l, rad)| format!("{{\"lod\":{l},\"radius\":{rad}}}"));
        Reply::json(format!(
            "{{\"h\":\"{h:08x}\",\"name\":{},\"file\":{},\"meshes\":[{}],\"shaders\":[{}],\"min\":[{},{},{}],\"max\":[{},{},{}],\"radius\":{},\"archetype\":{arch},\"placed\":{},\"places\":[{}]}}",
            quote(&w.name(h)),
            self.file_of(&w, h),
            meshes.join(","),
            shaders.join(","),
            d.min[0],
            d.min[1],
            d.min[2],
            d.max[0],
            d.max[1],
            d.max[2],
            m.sphere[3],
            placed.len(),
            places.join(",")
        ))
    }

    /// Where a model is placed, without reading it: [entity, x, y, z].
    fn model_places(&self, r: &Request) -> Reply {
        let Some(w) = self.the_world() else { return Reply::missing("no entity cache") };
        let Some(h) = r.hex("h").map(|h| h as u32) else { return Reply::bad("h") };
        let items: Vec<String> = w
            .by_model
            .get(&h)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .take(20000)
            .map(|&i| {
                let e = w.ent(i as usize);
                format!("[{i},{:.1},{:.1},{:.1}]", e.pos[0], e.pos[1], e.pos[2])
            })
            .collect();
        Reply::json(format!("{{\"items\":[{}]}}", items.join(",")))
    }

    /// The model's meshes: "VLM1", count, then per mesh: shader, vertex
    /// count, index count, positions (3 f32), UVs (2 f32), indices (u32).
    fn model_mesh(&self, r: &Request) -> Reply {
        let Some(h) = r.hex("h").map(|h| h as u32) else { return Reply::bad("h") };
        let Some(m) = self.load(h) else { return Reply::missing("the model could not be read") };
        // On a skeleton (its model's hash): bones per vertex, "VLM3".
        if let Some(skel) = r.hex("skel") {
            return self.skinned_mesh(h, &m, skel as u32, r.get("apart") == Some("1"));
        }
        if r.get("apart") == Some("1") {
            return model_apart(&m);
        }
        let mut b = Vec::new();
        b.extend_from_slice(b"VLM1");
        b.extend_from_slice(&(m.drawable.meshes.len() as u32).to_le_bytes());
        for me in &m.drawable.meshes {
            for v in [me.shader as u32, me.positions.len() as u32, me.indices.len() as u32] {
                b.extend_from_slice(&v.to_le_bytes());
            }
            for p in &me.positions {
                for c in p {
                    b.extend_from_slice(&c.to_le_bytes());
                }
            }
            // A terrain blend shader tiles its layers by the second UV set.
            let layered = m.drawable.layered.get(me.shader) == Some(&true) && me.uvs1.len() == me.positions.len();
            for i in 0..me.positions.len() {
                let uv = if layered { me.uvs1[i] } else { me.uvs.get(i).copied().unwrap_or([0.0, 0.0]) };
                b.extend_from_slice(&uv[0].to_le_bytes());
                b.extend_from_slice(&uv[1].to_le_bytes());
            }
            for i in &me.indices {
                b.extend_from_slice(&i.to_le_bytes());
            }
        }
        Reply::bytes("application/octet-stream", b, false)
    }

    /// A shader's albedo map as PNG: full size when it decodes, else its
    /// small copy.
    fn model_texture(&self, r: &Request) -> Reply {
        let (Some(h), Some(s)) = (r.hex("h").map(|h| h as u32), r.num::<usize>("s")) else { return Reply::bad("h, s") };
        let Some(m) = self.load(h) else { return Reply::missing("the model could not be read") };
        let Some(t) = m.textures.get(s) else { return Reply::missing("no such shader") };
        if let Some((dxgi, w, ht, data)) = &t.pixels
            && let Some(rgba) = crate::assets::decode(data, *dxgi, *w, *ht)
        {
            return Reply::bytes("image/png", encode_png(&rgba, *w, *ht), true);
        }
        match &t.small {
            Some((w, ht, data)) => Reply::bytes("image/png", encode_png(&data[..(w * ht * 4) as usize], *w, *ht), true),
            None => Reply::missing("no texture this reader can decode (BCPack?)"),
        }
    }

    // --- archives ---

    fn survey_path(name: &str) -> PathBuf {
        let stem = name.trim_end_matches(".rpf");
        cache().join(format!("survey-{stem}.tsv"))
    }

    fn survey(&self, name: &str) -> Option<Arc<Vec<Row>>> {
        if let Some(s) = self.surveys.lock().unwrap().get(name) {
            return Some(s.clone());
        }
        let text = std::fs::read_to_string(Self::survey_path(name)).ok()?;
        let world = self.the_world();
        let names = world.as_ref().map(|w| &w.names_at);
        let rows: Vec<Row> = text
            .lines()
            .filter_map(|l| {
                let mut it = l.splitn(4, '\t');
                let offset = u64::from_str_radix(it.next()?.trim_start_matches("0x"), 16).ok()?;
                let size = it.next()?.parse().ok()?;
                let _pieces = it.next();
                let kind = it.next().unwrap_or("").to_string();
                let name = names.and_then(|n| n.get(&(name.to_string(), offset))).cloned().unwrap_or_default();
                Some(Row { offset, size, kind, name })
            })
            .collect();
        let rows = Arc::new(rows);
        self.surveys.lock().unwrap().insert(name.to_string(), rows.clone());
        Some(rows)
    }

    fn archives_list(&self) -> Reply {
        let items: Vec<String> = self
            .rpf_files()
            .iter()
            .map(|(n, b)| {
                let indexed = Self::survey_path(n).exists();
                format!("{{\"name\":{},\"bytes\":{b},\"indexed\":{indexed}}}", quote(n))
            })
            .collect();
        Reply::json(format!("{{\"items\":[{}]}}", items.join(",")))
    }

    /// An archive's files matching the search (by name, kind or offset),
    /// of one `kind` if asked (the kind's first word: PSO, RSC, raw…); the
    /// first page also counts the kinds.
    fn archive_rows(&self, r: &Request) -> Reply {
        let Some(a) = r.get("a") else { return Reply::bad("a") };
        let Some(rows) = self.survey(a) else { return Reply::json("{\"indexed\":false,\"total\":0,\"items\":[]}".into()) };
        let m = match search::Matcher::from(r) {
            Ok(m) => m,
            Err(e) => return Reply::bad(&e),
        };
        let kind = r.get("kind").filter(|k| !k.is_empty());
        let off = r.get("q").unwrap_or("").trim().trim_start_matches("0x").to_lowercase();
        let (from, limit) = (r.num::<usize>("from").unwrap_or(0), r.num::<usize>("limit").unwrap_or(200).min(2000));
        let hit = |row: &&Row| {
            kind.is_none_or(|k| row.kind.split(' ').next() == Some(k))
                && (m.all() || m.is_match(&row.name) || m.is_match(&row.kind) || (off.len() >= 3 && off.bytes().all(|b| b.is_ascii_hexdigit()) && format!("{:x}", row.offset).contains(&off)))
        };
        let total = rows.iter().filter(hit).count();
        let items: Vec<String> = rows.iter().filter(hit).skip(from).take(limit).map(|row| format!("[\"{:#x}\",{},{},{}]", row.offset, row.size, quote(&row.kind), quote(&row.name))).collect();
        let kinds = if from == 0 {
            let mut c: HashMap<&str, usize> = HashMap::new();
            for row in rows.iter() {
                *c.entry(row.kind.split(' ').next().unwrap_or("")).or_default() += 1;
            }
            let mut v: Vec<(&str, usize)> = c.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            format!(",\"kinds\":[{}]", v.iter().map(|(k, n)| format!("[{},{n}]", quote(k))).collect::<Vec<_>>().join(","))
        } else {
            String::new()
        };
        Reply::json(format!("{{\"indexed\":true,\"total\":{total}{kinds},\"items\":[{}]}}", items.join(",")))
    }

    /// List an archive's files in the background (the `survey` format).
    fn archive_index(self: &Arc<Self>, r: &Request) -> Reply {
        let Some(name) = r.get("a").map(str::to_string) else { return Reply::bad("a") };
        let Some(archive) = self.archive(&name) else { return Reply::missing("no such archive") };
        let this = self.clone();
        {
            let mut j = self.jobs.lock().unwrap();
            if j.iter().any(|x| x.0.ends_with(&name) && !x.3) {
                return Reply::json("{\"started\":false}".into());
            }
            j.push((format!("Listing {name}"), 0, archive.len(), false));
        }
        std::thread::spawn(move || {
            let mut streams = Vec::new();
            archive.scan(|s, _| {
                streams.push(s);
                if streams.len() % 4096 == 0 {
                    this.progress(&name, s.offset, false);
                }
                true
            });
            let mut out = String::new();
            for f in rage::Archive::files(&streams) {
                let Ok(d) = archive.read_file(&f) else { continue };
                out.push_str(&rage::index::Row { offset: f.offset(), size: d.len(), pieces: f.streams.len(), kind: rage::index::kind(&d) }.line());
            }
            let _ = std::fs::write(Self::survey_path(&name), out);
            this.surveys.lock().unwrap().remove(&name);
            this.progress(&name, archive.len(), true);
        });
        Reply::json("{\"started\":true}".into())
    }

    /// Make the missing caches in the background (the jobs show it).
    fn build_caches(self: &Arc<Self>) -> Reply {
        {
            let mut j = self.jobs.lock().unwrap();
            if j.iter().any(|x| x.0.starts_with("Caches") && !x.3) {
                return Reply::json("{\"started\":false}".into());
            }
            j.push(("Caches: starting".into(), 0, 1, false));
        }
        let this = self.clone();
        std::thread::spawn(move || {
            let report = |step: usize, d: u64, t: u64| {
                if let Some(j) = this.jobs.lock().unwrap().iter_mut().find(|x| x.0.starts_with("Caches") && !x.3) {
                    *j = (format!("Caches {}/{}: {}", step + 1, crate::caches::STEPS.len(), crate::caches::STEPS[step].1), d, t.max(1), false);
                }
            };
            let r = crate::caches::build(&this.game, &report);
            if let Some(j) = this.jobs.lock().unwrap().iter_mut().find(|x| x.0.starts_with("Caches") && !x.3) {
                *j = (
                    match r {
                        Ok(()) => "Caches ready".into(),
                        Err(e) => format!("Caches failed: {e}"),
                    },
                    1,
                    1,
                    true,
                );
            }
        });
        Reply::json("{\"started\":true}".into())
    }

    fn progress(&self, name: &str, done: u64, finished: bool) {
        if let Some(j) = self.jobs.lock().unwrap().iter_mut().find(|x| x.0.ends_with(name) && !x.3) {
            j.1 = done;
            j.3 = finished;
        }
    }

    fn blob(&self, a: &str, o: u64) -> Option<Arc<Blob>> {
        let key = (a.to_string(), o);
        if let Some((_, b)) = self.blobs.lock().unwrap().iter().find(|(k, _)| *k == key) {
            return Some(b.clone());
        }
        let archive = self.archive(a)?;
        let file = archive.file_at(o).ok()?;
        let (virt, phys) = if rage::rsc::looks_like_resource(&file) { archive.resource_at(o).map(|r| (r.virt, r.phys)).unwrap_or_default() } else { (Vec::new(), Vec::new()) };
        let b = Arc::new(Blob { file, virt, phys });
        let mut c = self.blobs.lock().unwrap();
        c.push_back((key, b.clone()));
        if c.len() > 4 {
            c.pop_front();
        }
        Some(b)
    }

    fn blob_info(&self, r: &Request) -> Reply {
        let (Some(a), Some(o)) = (r.get("a"), r.hex("o")) else { return Reply::bad("a, o") };
        let Some(b) = self.blob(a, o) else { return Reply::missing("nothing readable at that offset") };
        Reply::json(format!("{{\"file\":{},\"virt\":{},\"phys\":{},\"kind\":{}}}", b.file.len(), b.virt.len(), b.phys.len(), quote(&kind_of(&b.file))))
    }

    fn bytes(&self, r: &Request) -> Reply {
        let (Some(a), Some(o)) = (r.get("a"), r.hex("o")) else { return Reply::bad("a, o") };
        let Some(b) = self.blob(a, o) else { return Reply::missing("nothing readable at that offset") };
        let part = match r.get("part").unwrap_or("file") {
            "virt" => &b.virt,
            "phys" => &b.phys,
            _ => &b.file,
        };
        let at = r.num::<usize>("at").unwrap_or(0).min(part.len());
        let len = r.num::<usize>("len").unwrap_or(65536).min(1 << 20);
        Reply::bytes("application/octet-stream", part[at..(at + len).min(part.len())].to_vec(), false)
    }

    /// A whole file, or one segment of a resource, to save.
    fn file_part(&self, r: &Request) -> Reply {
        let (Some(a), Some(o)) = (r.get("a"), r.hex("o")) else { return Reply::bad("a, o") };
        let Some(b) = self.blob(a, o) else { return Reply::missing("nothing readable at that offset") };
        let part = match r.get("part").unwrap_or("file") {
            "virt" => &b.virt,
            "phys" => &b.phys,
            _ => &b.file,
        };
        Reply::bytes("application/octet-stream", part.clone(), false)
    }

    // --- text ---

    /// The game's text by content (as the search options say) or label
    /// (its name or hash): `src` ui, subs or all; `table` one subtitle
    /// table whole; `format=tsv` all of it as a file.
    fn text_search(&self, r: &Request) -> Reply {
        let text = self.text.get_or_init(|| search::load(self));
        let m = match search::Matcher::from(r) {
            Ok(m) => m,
            Err(e) => return Reply::bad(&e),
        };
        let q = r.get("q").unwrap_or("").trim();
        let label = rage::joaat(&q.to_lowercase());
        let hex = u32::from_str_radix(q.trim_start_matches("0x"), 16).ok().filter(|_| q.len() >= 6 && !q.contains(' '));
        let src = r.get("src").unwrap_or("all");
        let table = r.num::<u32>("table");
        let tsv = r.get("format") == Some("tsv");
        let limit = if tsv { usize::MAX } else { r.num::<usize>("limit").unwrap_or(300).min(5000) };
        let hits: Vec<&(u32, String, u32)> = text
            .lines
            .iter()
            .filter(|(h, s, t)| {
                let from = match src {
                    "ui" => *t == 0,
                    "subs" => *t > 0,
                    _ => true,
                };
                from && table.is_none_or(|k| *t == k) && (m.is_match(s) || (!q.is_empty() && (*h == label || Some(*h) == hex)))
            })
            .collect();
        if tsv {
            let mut out = String::from("label\tsource\ttext\n");
            for (h, s, t) in &hits {
                let from = if *t == 0 { "interface".to_string() } else { format!("table {t}") };
                out.push_str(&format!("{h:08x}\t{from}\t{}\n", s.replace(['\t', '\n'], " ")));
            }
            return Reply::bytes("text/tab-separated-values; charset=utf-8", out.into_bytes(), false);
        }
        let items: Vec<String> = hits.iter().take(limit).map(|(h, s, t)| format!("[\"{h:08x}\",{},{t}]", quote(s))).collect();
        let ui = text.lines.iter().take_while(|x| x.2 == 0).count();
        Reply::json(format!(
            "{{\"total\":{},\"loaded\":{},\"ui\":{ui},\"subs\":{},\"tables\":{},\"items\":[{}]}}",
            hits.len(),
            text.lines.len(),
            text.lines.len() - ui,
            text.tables.len(),
            items.join(",")
        ))
    }
}

/// A model taken apart (see `vehicles::take_apart`): "VLM2", the part
/// count, per part its label (u16 length, UTF-8, zeros to 4 bytes), centre
/// (3 f32) and
/// triangles (u32); then the meshes as in "VLM1", each also with its part
/// (u32), one mesh per shader and part.
fn model_apart(m: &world::Model) -> Reply {
    let d = &m.drawable;
    let (of, parts) = vehicles::take_apart(d);
    let mut b = Vec::new();
    b.extend_from_slice(b"VLM2");
    b.extend_from_slice(&(parts.len() as u32).to_le_bytes());
    for (label, c, n) in &parts {
        b.extend_from_slice(&(label.len() as u16).to_le_bytes());
        b.extend_from_slice(label.as_bytes());
        // Padded to 4 bytes, so the floats that follow are aligned.
        while !b.len().is_multiple_of(4) {
            b.push(0);
        }
        for x in c {
            b.extend_from_slice(&x.to_le_bytes());
        }
        b.extend_from_slice(&(*n as u32).to_le_bytes());
    }
    // (shader, part, positions, UVs, indices); per part while splitting, the
    // mesh's vertices already taken.
    type Piece = (u32, u32, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>);
    type Split = (std::collections::HashMap<u32, u32>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>);
    let mut meshes: Vec<Piece> = Vec::new();
    for (me, tri_part) in d.meshes.iter().zip(&of) {
        let layered = d.layered.get(me.shader) == Some(&true) && me.uvs1.len() == me.positions.len();
        let mut by_part: std::collections::BTreeMap<u16, Split> = Default::default();
        for (t, &p) in me.indices.as_chunks::<3>().0.iter().zip(tri_part) {
            let e = by_part.entry(p).or_default();
            for &v in t {
                let n = e.1.len() as u32;
                let k = *e.0.entry(v).or_insert_with(|| {
                    e.1.push(me.positions[v as usize]);
                    e.2.push(if layered { me.uvs1[v as usize] } else { me.uvs.get(v as usize).copied().unwrap_or([0.0, 0.0]) });
                    n
                });
                e.3.push(k);
            }
        }
        for (p, (_, pos, uv, idx)) in by_part {
            meshes.push((me.shader as u32, p as u32, pos, uv, idx));
        }
    }
    b.extend_from_slice(&(meshes.len() as u32).to_le_bytes());
    for (shader, part, pos, uv, idx) in &meshes {
        for v in [*shader, pos.len() as u32, idx.len() as u32, *part] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for p in pos.iter().flatten().chain(uv.iter().flatten()) {
            b.extend_from_slice(&p.to_le_bytes());
        }
        for i in idx {
            b.extend_from_slice(&i.to_le_bytes());
        }
    }
    Reply::bytes("application/octet-stream", b, false)
}

/// A frame the page drew, kept for making a video of it: only when the
/// server was started with `VICE_LENS_CAPTURE` naming a folder (frames go
/// to `<folder>/<name>/<n>.jpg`); otherwise refused.
fn capture(r: &Request) -> Reply {
    let Some(dir) = std::env::var_os("VICE_LENS_CAPTURE") else { return Reply::missing("capture is off") };
    let (Some(name), Some(n)) = (r.get("name"), r.num::<u32>("n")) else { return Reply::bad("name, n") };
    if name.is_empty() || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') || !r.body.starts_with(&[0xff, 0xd8]) {
        return Reply::bad("a plain name and a JPEG");
    }
    let dir = PathBuf::from(dir).join(name);
    match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(format!("{n:05}.jpg")), &r.body)) {
        Ok(()) => Reply::json("{\"ok\":true}".into()),
        Err(e) => Reply::missing(&e.to_string()),
    }
}

/// LOD stand-ins and super-LODs, by name.
fn is_lod(n: &str) -> bool {
    n.ends_with("_lod") || n.contains("_lod_") || n.contains("slod") || n.contains("batchlod") || n.contains("batch_lod") || n.ends_with("_imposter")
}

/// What a model is, by the prefixes of its name: weapon, prop, interior,
/// nature, character, terrain, effect, or other (buildings and the rest).
fn model_kind(n: &str) -> &'static str {
    let starts = |p: &[&str]| p.iter().any(|x| n.starts_with(x));
    let tile = n.strip_prefix('s').and_then(|r| r.split_once("_w")).is_some_and(|(a, b)| !a.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && b.starts_with(|c: char| c.is_ascii_digit()));
    if starts(&["w_"]) {
        "weapon"
    } else if starts(&["prop_", "prefab_"]) {
        "prop"
    } else if starts(&["int_", "mint_"]) {
        "interior"
    } else if starts(&["veg_", "rock_"]) {
        "nature"
    } else if starts(&["char_", "head_", "cs_", "hc_", "player_", "crm_"]) || ["_mr1_", "_fr1_", "_mo1_", "_fo1_"].iter().any(|x| n.contains(x)) {
        "character"
    } else if tile {
        "terrain"
    } else if starts(&["ptfx_"]) {
        "effect"
    } else {
        "other"
    }
}

/// Whether two model names name variants of one item: the same words but
/// one, which differs only in its last letter (`bin_04a`, `bin_04b`).
fn variants(a: &str, b: &str) -> bool {
    let (ta, tb): (Vec<&str>, Vec<&str>) = (a.split('_').collect(), b.split('_').collect());
    if a == b || ta.len() != tb.len() {
        return false;
    }
    let mut diff = ta.iter().zip(&tb).filter(|(x, y)| x != y);
    let Some((x, y)) = diff.next() else { return false };
    let letter = |t: &str| !t.is_empty() && t.as_bytes()[t.len() - 1].is_ascii_lowercase() && t[..t.len() - 1].bytes().all(|c| c.is_ascii_digit());
    diff.next().is_none() && letter(x) && letter(y) && x[..x.len() - 1] == y[..y.len() - 1]
}

/// What a file looks like, in a word or two.
fn kind_of(d: &[u8]) -> String {
    if d.starts_with(b"PSIN") {
        "PSO".into()
    } else if rage::rbf::is_rbf(d) {
        "RBF0 (UI)".into()
    } else if rage::text::is_text_table(d) {
        "text table".into()
    } else if rage::gfx::is_gfx(d) {
        "GFX (Scaleform)".into()
    } else if rage::rsc::looks_like_resource(d) {
        if rage::txd::probe(d).is_some() { "resource: textures".into() } else { "resource".into() }
    } else if d.len() >= 4 {
        format!("raw {:02x}{:02x}{:02x}{:02x}", d[0], d[1], d[2], d[3])
    } else {
        "raw".into()
    }
}

fn encode_png(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        if let Ok(mut wr) = enc.write_header() {
            let _ = wr.write_image_data(rgba);
        }
    }
    out
}
