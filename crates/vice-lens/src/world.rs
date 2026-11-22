//! The world: drawables and their textures, loaded from the archives and
//! uploaded for the 3D view, streamed around the camera by worker threads.

use crate::view3d::{FADE_SECS, Fade};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use rage::drawable::{self, Drawable, TexRef};
use rage::{Archive, swizzle, ttpf};

/// All TTPF packs and an index from texture key to (pack, entry).
pub struct Textures {
    packs: Vec<ttpf::Pack>,
    by_key: HashMap<u64, (usize, usize)>,
    /// Keys a load found to be normal maps.
    not_colour_maps: Mutex<HashSet<u64>>,
}

impl Textures {
    pub fn open(game: &Path) -> Textures {
        let mut paths = Vec::new();
        collect(game, &mut paths);
        paths.sort();
        let mut packs = Vec::new();
        let mut by_key = HashMap::new();
        for p in paths {
            match ttpf::Pack::open(&p) {
                Ok(pack) => {
                    let pi = packs.len();
                    for (i, e) in pack.entries.iter().enumerate() {
                        by_key.entry(e.key).or_insert((pi, i));
                    }
                    packs.push(pack);
                }
                Err(e) => log::warn!("{}: {e}", p.display()),
            }
        }
        log::info!("{} texture packs, {} keys", packs.len(), by_key.len());
        Textures { packs, by_key, not_colour_maps: Mutex::new(HashSet::new()) }
    }

    /// Whether the texture with `key` exists and its top level can be
    /// decoded (not BCPack; the small mips may be).
    pub fn decodable(&self, key: u64) -> bool {
        self.by_key.get(&key).is_some_and(|&(p, i)| self.packs[p].top_plain(i))
    }

    /// Whether the texture with `key` cannot be a colour map: one or two
    /// channels (BC4, BC5), or found to be a normal map.
    pub fn not_colour(&self, key: u64) -> bool {
        self.by_key.get(&key).is_some_and(|&(p, i)| matches!(self.packs[p].entries[i].dxgi, 79..=84)) || self.not_colour_maps.lock().unwrap().contains(&key)
    }

    /// What the pack says of the texture with `key`: width, height, DXGI
    /// format, and whether its top level decodes.
    pub fn info(&self, key: u64) -> Option<(u32, u32, u8, bool)> {
        let &(p, i) = self.by_key.get(&key)?;
        let e = &self.packs[p].entries[i];
        Some((e.width, e.height, e.dxgi, self.packs[p].top_plain(i)))
    }

    /// Top mip of the texture with `key`: (dxgi, width, height, linear blocks).
    pub fn get(&self, key: u64) -> Option<(u8, u32, u32, Vec<u8>)> {
        let &(p, i) = self.by_key.get(&key)?;
        let pack = &self.packs[p];
        let e = &pack.entries[i];
        match pack.top_mip(i) {
            Ok(data) => Some((e.dxgi, e.width, e.height, data)),
            Err(err) => {
                log::debug!("texture {key:016x}: {err}");
                None
            }
        }
    }
}

fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "ttpf") {
            out.push(p);
        }
    }
}

/// Decoded pixels of a texture: (dxgi, width, height, linear blocks).
pub type Pixels = (u8, u32, u32, Vec<u8>);

/// A small RGBA8 copy of a texture with its whole mip chain, for the
/// distance (the stored lower mips are not read reliably yet): width,
/// height, and the levels one after another.
pub type Small = (u32, u32, Vec<u8>);

/// A shader's albedo map: its TTPF key (shared textures are uploaded once)
/// and its pixels, if this load decoded them.
#[derive(Default)]
pub struct ShaderTex {
    pub key: Option<u64>,
    pub pixels: Option<Pixels>,
    pub small: Option<Small>,
}

/// Largest edge of a texture's small copy: for models that may come near,
/// and for LOD stand-ins, which never do.
const SMALL: u32 = 128;
const SMALL_FAR: u32 = 64;

/// The small mipmapped copy of decoded pixels, at most `edge` wide.
pub fn small_copy(p: &Pixels, edge: u32) -> Option<Small> {
    let (dxgi, w, h, data) = p;
    let rgba = crate::assets::decode(data, *dxgi, *w, *h)?;
    let (mut w, mut h, mut img) = (*w, *h, rgba);
    while w.max(h) > edge {
        (w, h, img) = halve(w, h, &img);
    }
    Some(mip_chain(w, h, img))
}

/// An RGBA8 image followed by its mips down to 1×1.
pub fn mip_chain(w: u32, h: u32, img: Vec<u8>) -> Small {
    let mut out = img.clone();
    let (mut cw, mut ch, mut cur) = (w, h, img);
    while cw > 1 || ch > 1 {
        (cw, ch, cur) = halve(cw, ch, &cur);
        out.extend_from_slice(&cur);
    }
    (w, h, out)
}

/// Box-filter an RGBA8 image to half size (at least 1×1).
fn halve(w: u32, h: u32, img: &[u8]) -> (u32, u32, Vec<u8>) {
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        for x in 0..nw {
            for c in 0..4 {
                let mut sum = 0u32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                    sum += img[((sy * w + sx) * 4 + c) as usize] as u32;
                }
                out[((y * nw + x) * 4 + c) as usize] = (sum / 4) as u8;
            }
        }
    }
    (nw, nh, out)
}

/// A drawable with its vertices built and its albedo maps decoded, ready for
/// upload; built on a worker thread.
pub struct Model {
    pub drawable: Drawable,
    /// Per mesh, the vertices the 3D view draws.
    pub vertices: Vec<Vec<crate::view3d::Vertex>>,
    /// Per shader.
    pub textures: Vec<ShaderTex>,
    /// A sphere holding every vertex: centre and radius.
    pub sphere: [f32; 4],
}

/// Load the drawable at `offset`. `claim(key)` says whether this load
/// should decode a shared texture (true for the first loader of each key);
/// a shader takes the first of its albedo maps that can be decoded.
/// With `full` false (LOD stand-ins, seen from afar) only the small copies
/// of textures are kept.
/// Load model `hash` from the file at `offset`: entry `entry` of a
/// dictionary, or the file's own drawable (or its dictionary entry named
/// `hash`).
pub fn load_model(archive: &Archive, offset: u64, entry: Option<u16>, hash: u32, full: bool, far: bool, textures: &Textures, claim: &dyn Fn(u64) -> bool) -> Option<Model> {
    let edge = if far { SMALL_FAR } else { SMALL };
    let res = archive.resource_at(offset).ok()?;
    let d = match entry {
        Some(k) => drawable::parse_at(&res.virt, &res.phys, drawable::dictionary_entry(&res.virt, k as usize)?)?,
        None => drawable::parse_model(&res.virt, &res.phys, hash)?,
    };
    let allocs = rage::txd::allocations(&d.embedded, res.phys.len());
    let tex = d
        .shaders
        .iter()
        .map(|cands| {
            for c in cands {
                match c {
                    TexRef::Key(k) if textures.decodable(*k) && !textures.not_colour(*k) => {
                        let pixels = if claim(*k) { textures.get(*k) } else { None };
                        let small = pixels.as_ref().and_then(|p| small_copy(p, edge));
                        if small.as_ref().is_some_and(normal_map) {
                            textures.not_colour_maps.lock().unwrap().insert(*k);
                            continue;
                        }
                        return ShaderTex { key: Some(*k), pixels: pixels.filter(|_| full), small };
                    }
                    TexRef::Key(_) => {}
                    TexRef::Embedded(at) => {
                        let Some((i, t)) = d.embedded.iter().enumerate().find(|(_, t)| t.at == *at) else { continue };
                        if matches!(t.dxgi, 79..=84) {
                            continue;
                        }
                        if let Some(data) = rage::txd::top_mip(&res.phys, t, allocs[i]) {
                            let pixels = Some((t.dxgi, t.width, t.height, data));
                            let small = pixels.as_ref().and_then(|p| small_copy(p, edge));
                            if small.as_ref().is_some_and(normal_map) {
                                continue;
                            }
                            return ShaderTex { key: None, pixels: pixels.filter(|_| full), small };
                        }
                    }
                }
            }
            ShaderTex::default()
        })
        .collect();
    let vertices = d.meshes.iter().map(|m| crate::view3d::vertices(m, d.layered.get(m.shader) == Some(&true))).collect();
    let sphere = bounding_sphere(&d);
    Some(Model { drawable: d, vertices, textures: tex, sphere })
}

/// A sphere around a drawable's vertices (its declared bounds are not
/// always met: some vertices reach past them).
fn bounding_sphere(d: &Drawable) -> [f32; 4] {
    let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
    for p in d.meshes.iter().flat_map(|m| &m.positions) {
        lo = lo.min(glam::Vec3::from(*p));
        hi = hi.max(glam::Vec3::from(*p));
    }
    if lo.x > hi.x {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let c = (lo + hi) * 0.5;
    let r = d.meshes.iter().flat_map(|m| &m.positions).map(|p| c.distance(glam::Vec3::from(*p))).fold(0.0f32, f32::max);
    [c.x, c.y, c.z, r]
}

/// Whether a small copy reads as a tangent-space normal map: lavender on
/// average, red and green centred on half, blue high. Some shaders list a
/// normal map where Vice Lens looks for colour; drawn as colour, it paints
/// whole buildings violet.
fn normal_map(s: &Small) -> bool {
    let (w, h, px) = s;
    let n = (*w * *h) as usize;
    let mut sum = [0u64; 3];
    for p in px[..n * 4].as_chunks::<4>().0 {
        for c in 0..3 {
            sum[c] += p[c] as u64;
        }
    }
    let [r, g, b] = sum.map(|v| v as f32 / n as f32);
    (r - 128.0).abs() < 20.0 && (g - 128.0).abs() < 20.0 && b > 200.0
}

/// The wgpu format a diffuse map uploads as (colour data, so sRGB).
pub fn gpu_format(dxgi: u8) -> Option<wgpu::TextureFormat> {
    use wgpu::TextureFormat as F;
    Some(match dxgi {
        71 | 72 => F::Bc1RgbaUnormSrgb,
        74 | 75 => F::Bc2RgbaUnormSrgb,
        77 | 78 => F::Bc3RgbaUnormSrgb,
        80 => F::Bc4RUnorm,
        83 => F::Bc5RgUnorm,
        98 | 99 => F::Bc7RgbaUnormSrgb,
        28 | 29 => F::Rgba8UnormSrgb,
        87 | 91 => F::Bgra8UnormSrgb,
        _ => return None,
    })
}

/// Bytes per row of a linear top mip.
pub fn row_bytes(dxgi: u8, w: u32) -> Option<(u32, u32)> {
    let f = swizzle::format(dxgi)?;
    let (ew, _) = f.elements(w, 1);
    Some(((ew * f.bytes) as u32, if f.block { 4 } else { 1 }))
}

/// Model name hash → (archive, offset, entry in a drawable dictionary) of
/// its drawable.
#[derive(Default)]
pub struct ModelIndex {
    pub archives: Vec<String>,
    pub by_hash: HashMap<u32, (usize, u64, Option<u16>)>,
    pub names: HashMap<u32, String>,
    /// Models from dictionaries of LOD stand-ins, of super-LODs, and of
    /// water map tiles.
    pub lod: HashSet<u32>,
    pub slod: HashSet<u32>,
    pub water: HashSet<u32>,
}

impl ModelIndex {
    /// Read `archive<TAB>offset<TAB>name` lines (names with or without
    /// `.#dr`) from each file that exists; earlier files win.
    pub fn load(paths: &[std::path::PathBuf]) -> ModelIndex {
        let mut m = ModelIndex::default();
        let text: String = paths.iter().filter_map(|p| std::fs::read_to_string(p).ok()).collect::<Vec<_>>().join("\n");
        for line in text.lines() {
            let mut it = line.split('\t');
            let (Some(ar), Some(off), Some(name)) = (it.next(), it.next(), it.next()) else { continue };
            let Ok(off) = u64::from_str_radix(off.trim_start_matches("0x"), 16) else { continue };
            let ai = match m.archives.iter().position(|a| a == ar) {
                Some(i) => i,
                None => {
                    m.archives.push(ar.to_string());
                    m.archives.len() - 1
                }
            };
            let stem = name.split(".#").next().unwrap_or(name);
            let h = rage::joaat(stem);
            m.by_hash.entry(h).or_insert((ai, off, None));
            m.names.entry(h).or_insert_with(|| stem.to_string());
        }
        m
    }

    fn archive(&mut self, name: &str) -> usize {
        self.archives.iter().position(|a| a == name).unwrap_or_else(|| {
            self.archives.push(name.to_string());
            self.archives.len() - 1
        })
    }

    /// Add the entries of the drawable dictionaries the `dicts` example
    /// lists (`offset<TAB>hash<TAB>entry<TAB>first entry's name`). Their
    /// entries are only known by hash; the first entry's name tells the
    /// set: LOD stand-ins, super-LODs (left out) or water map tiles.
    pub fn add_dicts(&mut self, path: &Path, archive: &str) {
        let Ok(text) = std::fs::read_to_string(path) else { return };
        let ai = self.archive(archive);
        let mut kind = "";
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            let (Some(off), Some(h), Some(k)) = (f.first(), f.get(1), f.get(2)) else { continue };
            let (Ok(off), Ok(h), Ok(k)) = (u64::from_str_radix(off.trim_start_matches("0x"), 16), u32::from_str_radix(h, 16), k.parse::<u16>()) else { continue };
            if k == 0 {
                let first = f.get(3).copied().unwrap_or("");
                kind = if first.contains("slod") {
                    "slod"
                } else if first.contains("water") {
                    "water"
                } else {
                    "lod"
                };
            }
            // The model list holds each dictionary once, as if it were one
            // drawable named after its first entry: that is entry 0.
            match self.by_hash.get(&h) {
                Some(&(a, o, None)) if a == ai && o == off => {}
                Some(_) => continue,
                None => {}
            }
            self.by_hash.insert(h, (ai, off, Some(k)));
            match kind {
                "water" => self.water.insert(h),
                "slod" => self.slod.insert(h),
                _ => self.lod.insert(h),
            };
        }
    }
}

/// One placed object.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub model: u32,
    pub pos: glam::Vec3,
    pub rot: glam::Quat,
    pub scale: glam::Vec3,
    /// Drawn while the eye is between `near` and `far` (XY metres) from it.
    pub near: f32,
    pub far: f32,
    /// The model's number in the streamer (set there).
    pub id: u32,
}

impl Placement {
    pub fn matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(self.scale, self.rot, self.pos)
    }
}

/// Edge of a streaming cell, metres.
const CELL: f32 = 64.0;

/// Geometry and small texture copies kept on the GPU before models out of
/// range are freed (their textures go with the last model using them), MiB.
const MESH_BUDGET_MIB: u64 = 2000;

fn model_mib(view: &crate::view3d::View3d) -> u64 {
    let m = view.memory();
    m[1] + m[2]
}

/// What a streamer knows about one model.
enum Slot {
    /// Not asked for.
    Idle,
    /// Queued or being loaded.
    Loading,
    Ready(usize),
    Missing,
}

/// What the worker threads share.
struct Shared {
    archives: Vec<Archive>,
    index: ModelIndex,
    textures: Textures,
    /// Texture keys some load has decoded (or is decoding): at full size,
    /// and as small copies only. A full-size load decodes a key whatever
    /// the small loads did, so near models need no second pass.
    claimed: Mutex<HashSet<u64>>,
    claimed_small: Mutex<HashSet<u64>>,
    queue: Mutex<VecDeque<Job>>,
    wake: Condvar,
    quit: AtomicBool,
}

/// Work for the loader threads.
#[derive(Clone, Copy)]
enum Job {
    /// Load a model: with its full-size textures, and only seen from
    /// afar (LOD stand-ins: smaller texture copies).
    Load(u32, bool, bool),
    /// Decode an uploaded model's full-size textures again (the camera
    /// came near).
    Sharpen(u32),
}

/// What a loader thread brings back: the model, its load (if it could be
/// read), and whether it was a `Job::Sharpen`.
type Done = (u32, Option<Model>, bool);

/// Detailed models this far past the radius are loaded before they come
/// into range, metres.
const PREFETCH: f32 = 120.0;

/// Within this distance of the eye (metres) a model's textures go up at
/// full size; farther, the small copies are sharp enough.
const FULL_DIST: f32 = 320.0;

/// Placements bucketed into cells.
struct Layer {
    placements: Vec<Placement>,
    cells: HashMap<(i32, i32), Vec<u32>>,
    /// Cell edge, metres; the farthest any placement here is drawn.
    cell: f32,
    reach: f32,
}

impl Layer {
    fn new(placements: Vec<Placement>, cell: f32) -> Layer {
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, p) in placements.iter().enumerate() {
            cells.entry(((p.pos.x / cell).floor() as i32, (p.pos.y / cell).floor() as i32)).or_default().push(i as u32);
        }
        let reach = placements.iter().map(|p| p.far).fold(0.0, f32::max);
        Layer { placements, cells, cell, reach }
    }

    /// Placements within `reach` (XY) of `at`: their number in the layer,
    /// the placement and its distance.
    fn within(&self, at: glam::Vec2, reach: f32, mut f: impl FnMut(u32, &Placement, f32)) {
        let reach = reach.min(self.reach);
        let c = |v: f32| (v / self.cell).floor() as i32;
        for cx in c(at.x - reach)..=c(at.x + reach) {
            for cy in c(at.y - reach)..=c(at.y + reach) {
                for &pi in self.cells.get(&(cx, cy)).map(Vec::as_slice).unwrap_or(&[]) {
                    let p = &self.placements[pi as usize];
                    let d = p.pos.truncate().distance(at);
                    if d <= reach {
                        f(pi, p, d);
                    }
                }
            }
        }
    }
}

/// Loads the models of the placements around the camera on worker threads,
/// nearest first, and keeps the 3D view's instances to the placements
/// whose window (`Placement::near..far`) holds the eye: detailed models
/// within `radius`, then the game's LOD and super-LOD stand-ins, each in
/// the band its data gives. Models are numbered densely and placements
/// globally, so a rebuild walks arrays rather than hash maps; a model that
/// arrives places its own instances, and the full rebuild runs only when
/// the eye has moved `REBUILD_STEP`.
pub struct Streamer {
    /// Placements by how far they reach: detailed, to 1 km, to 4 km,
    /// beyond, and batch instances (replaced as the camera travels); and
    /// the global number of each layer's first placement.
    layers: Vec<Layer>,
    base: [u32; 5],
    /// Per model (dense number): its name hash, slot, the rebuild that
    /// last had it in range, its placements (global numbers), and the
    /// rebuild it was last sent to be sharpened (0: never).
    hashes: Vec<u32>,
    dense: HashMap<u32, u32>,
    slots: Vec<Slot>,
    used: Vec<u64>,
    of_model: Vec<Vec<u32>>,
    sharpened: Vec<u64>,
    sharpening: Vec<bool>,
    /// Per placement: when it began dissolving in (or out, when leaving;
    /// NaN when not on screen), and the rebuild that last saw it in range.
    start: Vec<f32>,
    leaving: Vec<bool>,
    seen: Vec<u64>,
    /// Placements on screen.
    shown: Vec<u32>,
    shared: Arc<Shared>,
    results: mpsc::Receiver<Done>,
    workers: Vec<std::thread::JoinHandle<()>>,
    pub radius: f32,
    /// Nothing farther is drawn (pulled in when geometry is over budget).
    pub view: f32,
    /// Every model gets the smallest texture copies (the map).
    pub thumbnails: bool,
    /// Instances dissolve in and out; without, they come and go at once
    /// (the map, whose clock stands still).
    pub fades: bool,
    /// Instances placed whatever the camera does (the sea, the navmesh).
    fixed: Vec<(usize, glam::Mat4)>,
    generation: u64,
    /// Where the instances were last rebuilt.
    built_at: Option<glam::Vec2>,
    loading: usize,
    pub ready: usize,
    pub missing: usize,
    /// What the last rebuild placed: (model, transform).
    pub drawn: Vec<(u32, glam::Mat4)>,
    /// The view time of the first rebuild.
    first: Option<f32>,
}

/// The eye moves this far (metres, XY) before the instances are rebuilt.
const REBUILD_STEP: f32 = 16.0;

/// Whether a placement at XY distance `d` is drawn, and whether it is a
/// detailed model just past its window, loaded ahead.
fn in_window(p: &Placement, d: f32, radius: f32, view: f32) -> (bool, bool) {
    let detail = p.far <= radius + 1.0;
    (d >= p.near && d <= p.far && d <= view, detail && d > p.far && d <= p.far + PREFETCH)
}

impl Streamer {
    pub fn new(game: &Path, index: ModelIndex, placements: Vec<Placement>, radius: f32, view: f32, threads: usize) -> Streamer {
        let mut dense: HashMap<u32, u32> = HashMap::new();
        let mut hashes = Vec::new();
        let mut tiers: [Vec<Placement>; 4] = Default::default();
        for mut p in placements {
            p.id = *dense.entry(p.model).or_insert_with(|| {
                hashes.push(p.model);
                hashes.len() as u32 - 1
            });
            let t = if p.far <= radius + 1.0 {
                0
            } else if p.far <= 1000.0 {
                1
            } else if p.far <= 4000.0 {
                2
            } else {
                3
            };
            tiers[t].push(p);
        }
        log::info!("placements by reach (detailed, 1 km, 4 km, farther): {:?}; {} models", tiers.iter().map(Vec::len).collect::<Vec<_>>(), hashes.len());
        let mut base = [0u32; 5];
        let mut of_model: Vec<Vec<u32>> = vec![Vec::new(); hashes.len()];
        let mut g = 0u32;
        for (i, t) in tiers.iter().enumerate() {
            base[i] = g;
            for p in t {
                of_model[p.id as usize].push(g);
                g += 1;
            }
        }
        let n = g as usize;
        base[4] = g;
        let mut layers: Vec<Layer> = tiers.into_iter().zip([CELL, 128.0, 512.0, 2048.0]).map(|(p, c)| Layer::new(p, c)).collect();
        layers.push(Layer::new(Vec::new(), CELL));
        let archives = index.archives.iter().map(|a| Archive::open(game.join(a)).expect("archive")).collect();
        let shared = Arc::new(Shared {
            archives,
            index,
            textures: Textures::open(game),
            claimed: Mutex::new(HashSet::new()),
            claimed_small: Mutex::new(HashSet::new()),
            queue: Mutex::new(VecDeque::new()),
            wake: Condvar::new(),
            quit: AtomicBool::new(false),
        });
        let (tx, results) = mpsc::channel();
        let workers = (0..threads.max(1))
            .map(|_| {
                let (shared, tx) = (shared.clone(), tx.clone());
                std::thread::spawn(move || worker(&shared, &tx))
            })
            .collect();
        let models = hashes.len();
        Streamer {
            layers,
            base,
            hashes,
            dense,
            slots: (0..models).map(|_| Slot::Idle).collect(),
            used: vec![0; models],
            of_model,
            sharpened: vec![0; models],
            sharpening: vec![false; models],
            start: vec![f32::NAN; n],
            leaving: vec![false; n],
            seen: vec![0; n],
            shown: Vec::new(),
            shared,
            results,
            workers,
            radius,
            view,
            thumbnails: false,
            fades: true,
            fixed: Vec::new(),
            generation: 0,
            built_at: None,
            loading: 0,
            ready: 0,
            missing: 0,
            drawn: Vec::new(),
            first: None,
        }
    }

    /// Replace the batch instances.
    pub fn set_batches(&mut self, batches: Vec<Placement>) {
        let b = self.base[4];
        for p in &self.layers[4].placements {
            self.of_model[p.id as usize].retain(|g| *g < b);
        }
        self.shown.retain(|g| *g < b);
        {
            let v = &mut self.start;
            v.truncate(b as usize);
        }
        self.leaving.truncate(b as usize);
        self.seen.truncate(b as usize);
        let mut batches = batches;
        for (k, p) in batches.iter_mut().enumerate() {
            p.id = match self.dense.get(&p.model) {
                Some(&i) => i,
                None => {
                    self.hashes.push(p.model);
                    self.slots.push(Slot::Idle);
                    self.used.push(0);
                    self.of_model.push(Vec::new());
                    self.sharpened.push(0);
                    self.sharpening.push(false);
                    let i = self.hashes.len() as u32 - 1;
                    self.dense.insert(p.model, i);
                    i
                }
            };
            self.of_model[p.id as usize].push(b + k as u32);
        }
        let n = b as usize + batches.len();
        self.start.resize(n, f32::NAN);
        self.leaving.resize(n, false);
        self.seen.resize(n, 0);
        log::debug!("{} batch instances", batches.len());
        self.layers[4] = Layer::new(batches, CELL);
        self.built_at = None;
    }

    fn placement(&self, g: u32) -> &Placement {
        let l = self.base.partition_point(|b| *b <= g) - 1;
        &self.layers[l].placements[(g - self.base[l]) as usize]
    }

    /// Keep an instance of an uploaded model whatever the camera does.
    pub fn keep(&mut self, model: usize, at: glam::Mat4) {
        self.fixed.push((model, at));
        self.built_at = None;
    }

    /// The model's name, when the index knows it.
    pub fn name(&self, model: u32) -> String {
        self.shared.index.names.get(&model).cloned().unwrap_or_else(|| format!("{model:08x}"))
    }

    /// Models queued or loading.
    pub fn loading(&self) -> usize {
        self.loading
    }

    /// Upload the models that arrived (within `budget`), placing their
    /// instances that are in range, and rebuild the view's instances when
    /// the camera has moved.
    pub fn update(&mut self, view: &mut crate::view3d::View3d, eye: glam::Vec3, budget: Duration) {
        let t = Instant::now();
        let mut uploaded = 0;
        let now = view.time;
        let settle = !self.fades || self.first.is_none_or(|f| now <= f);
        let at = self.built_at.unwrap_or(eye.truncate());
        while t.elapsed() < budget {
            if uploaded >= 64 {
                view.flush();
                uploaded = 0;
            }
            let Ok((hash, m, sharpen)) = self.results.try_recv() else { break };
            uploaded += 1;
            self.loading -= 1;
            let Some(&mi) = self.dense.get(&hash) else { continue };
            let mi = mi as usize;
            if sharpen {
                self.sharpening[mi] = false;
                if let (Some(m), Slot::Ready(id)) = (m, &self.slots[mi]) {
                    view.sharpen_model(*id, &m);
                }
                continue;
            }
            let Some(m) = m else {
                self.slots[mi] = Slot::Missing;
                self.missing += 1;
                continue;
            };
            let id = view.add_model(&m, self.shared.index.water.contains(&hash));
            self.slots[mi] = Slot::Ready(id);
            self.ready += 1;
            if self.ready.is_multiple_of(250) {
                log::debug!("{} models; MiB (textures, small, meshes) {:?}", self.ready, view.memory());
            }
            // Its placements in range come in now.
            for k in 0..self.of_model[mi].len() {
                let g = self.of_model[mi][k];
                let p = *self.placement(g);
                let (draw, _) = in_window(&p, p.pos.truncate().distance(at), self.radius, self.view);
                let gi = g as usize;
                if !draw || !self.start[gi].is_nan() {
                    continue;
                }
                let s = if settle { -1e9 } else { now };
                view.add_instance(id, p.matrix(), Fade::In(s));
                self.drawn.push((p.model, p.matrix()));
                (self.start[gi], self.leaving[gi], self.seen[gi]) = (s, false, self.generation);
                self.shown.push(g);
                self.used[mi] = self.generation;
            }
        }
        if uploaded > 0 {
            view.flush();
        }
        if self.built_at.is_none_or(|p| p.distance(eye.truncate()) > REBUILD_STEP) {
            self.rebuild(view, eye);
        }
    }

    fn rebuild(&mut self, view: &mut crate::view3d::View3d, eye: glam::Vec3) {
        let at = eye.truncate();
        self.built_at = Some(at);
        // Requests not started yet are redone in the new order.
        for job in self.shared.queue.lock().unwrap().drain(..) {
            match job {
                Job::Load(h, ..) => {
                    if let Some(&mi) = self.dense.get(&h) {
                        self.slots[mi as usize] = Slot::Idle;
                    }
                }
                Job::Sharpen(h) => {
                    if let Some(&mi) = self.dense.get(&h) {
                        self.sharpening[mi as usize] = false;
                    }
                }
            }
            self.loading -= 1;
        }
        view.clear_instances();
        for &(id, m) in &self.fixed {
            view.add_instance(id, m, Fade::Whole);
        }
        // What comes into view dissolves in, what leaves dissolves out;
        // the first frame comes up whole.
        let now = view.time;
        let settle = now <= *self.first.get_or_insert(now) || !self.fades;
        self.generation += 1;
        let generation = self.generation;
        // Model → (nearest distance, wants full-size textures, only far).
        let mut want: HashMap<u32, (f32, bool, bool)> = HashMap::new();
        let mut sharpen: Vec<u32> = Vec::new();
        let mut next: Vec<u32> = Vec::with_capacity(self.shown.len() + 1024);
        let index = &self.shared.index;
        let (radius, view_far, thumbnails) = (self.radius, self.view, self.thumbnails);
        let (slots, used, start, leaving, seen) = (&mut self.slots, &mut self.used, &mut self.start, &mut self.leaving, &mut self.seen);
        let (hashes, sharpening, sharpened, missing, drawn) = (&self.hashes, &self.sharpening, &self.sharpened, &mut self.missing, &mut self.drawn);
        drawn.clear();
        let added = std::cell::Cell::new(0usize);
        for (li, layer) in self.layers.iter().enumerate() {
            let base = self.base[li];
            let reach = if li == 0 || li == 4 { radius + PREFETCH } else { view_far };
            layer.within(at, reach, |k, p, d| {
                let (draw, ahead) = in_window(p, d, radius, view_far);
                if !draw && !ahead {
                    return;
                }
                let (mi, g) = (p.id as usize, (base + k) as usize);
                match slots[mi] {
                    Slot::Ready(_) if !draw => used[mi] = generation,
                    Slot::Ready(id) => {
                        let detail = p.far <= radius + 1.0;
                        let near = detail && (d * d + (eye.z - p.pos.z).powi(2)).sqrt() < FULL_DIST;
                        if near && !sharpening[mi] && (sharpened[mi] == 0 || generation > sharpened[mi] + 40) && view.blurry(id) {
                            sharpen.push(mi as u32);
                        }
                        let s = if start[g].is_nan() {
                            if settle { -1e9 } else { now }
                        } else if leaving[g] {
                            // Coming back while leaving: from where it is.
                            now - (1.0 - ((now - start[g]) / FADE_SECS).clamp(0.0, 1.0)) * FADE_SECS
                        } else {
                            start[g]
                        };
                        if seen[g] != generation {
                            let m = p.matrix();
                            view.add_instance(id, m, Fade::In(s));
                            drawn.push((p.model, m));
                            added.set(added.get() + 1);
                            next.push(g as u32);
                        }
                        (start[g], leaving[g], seen[g]) = (s, false, generation);
                        used[mi] = generation;
                    }
                    Slot::Loading | Slot::Missing => {}
                    Slot::Idle if index.by_hash.contains_key(&hashes[mi]) => {
                        let full = draw && p.far <= radius + 1.0 && (d * d + (eye.z - p.pos.z).powi(2)).sqrt() < FULL_DIST;
                        // Stand-ins seen only from 400 m on keep the
                        // smallest texture copies.
                        let distant = thumbnails || p.near >= 400.0;
                        let w = want.entry(mi as u32).or_insert((d, full, distant));
                        *w = (w.0.min(d), w.1 || full, w.2 && distant);
                    }
                    Slot::Idle => {
                        slots[mi] = Slot::Missing;
                        *missing += 1;
                    }
                }
            });
        }
        log::debug!("rebuild at ({:.0}, {:.0}): {} placed, {} were shown, view {:.0} m", at.x, at.y, added.get(), self.shown.len(), self.view);
        // What was on screen and is out of range now dissolves out.
        for &g in &std::mem::take(&mut self.shown) {
            let gi = g as usize;
            if self.seen[gi] == generation || self.start[gi].is_nan() {
                continue;
            }
            let p = *self.placement(g);
            let mi = p.id as usize;
            let s = if self.leaving[gi] { self.start[gi] } else { now - (1.0 - ((now - self.start[gi]) / FADE_SECS).clamp(0.0, 1.0)) * FADE_SECS };
            match self.slots[mi] {
                Slot::Ready(id) if self.fades && now - s < FADE_SECS => {
                    view.add_instance(id, p.matrix(), Fade::Out(s));
                    (self.start[gi], self.leaving[gi], self.seen[gi]) = (s, true, generation);
                    self.used[mi] = generation;
                    next.push(g);
                }
                _ => (self.start[gi], self.leaving[gi]) = (f32::NAN, false),
            }
        }
        self.shown = next;
        // Over the geometry budget: free the models out of range longest;
        // when all of it is in range, draw less far.
        if model_mib(view) > MESH_BUDGET_MIB {
            let mut idle: Vec<(u64, usize)> = self.slots.iter().enumerate().filter(|(i, s)| matches!(s, Slot::Ready(_)) && self.used[*i] < generation).map(|(i, _)| (self.used[i], i)).collect();
            idle.sort_unstable();
            for (_, mi) in idle {
                if model_mib(view) < MESH_BUDGET_MIB * 4 / 5 {
                    break;
                }
                if let Slot::Ready(id) = std::mem::replace(&mut self.slots[mi], Slot::Idle) {
                    view.free_model(id);
                    self.sharpened[mi] = 0;
                    self.ready -= 1;
                }
            }
            // Freed textures are decoded again when next wanted.
            let (mut full, mut small) = (self.shared.claimed.lock().unwrap(), self.shared.claimed_small.lock().unwrap());
            for k in view.released.drain(..) {
                full.remove(&k);
                small.remove(&k);
            }
        }
        if model_mib(view) > MESH_BUDGET_MIB {
            let least = self.radius + 600.0;
            if self.view > least {
                self.view = (self.view * 0.85).max(least);
                log::warn!("models over {MESH_BUDGET_MIB} MiB with all of them in range: drawing out to {:.0} m", self.view);
            }
        }
        let mut want: Vec<(u32, (f32, bool, bool))> = want.into_iter().collect();
        want.sort_by(|a, b| a.1.0.total_cmp(&b.1.0));
        sharpen.sort_unstable();
        sharpen.dedup();
        if !want.is_empty() || !sharpen.is_empty() {
            for &(mi, _) in &want {
                self.slots[mi as usize] = Slot::Loading;
            }
            for &mi in &sharpen {
                self.sharpening[mi as usize] = true;
                self.sharpened[mi as usize] = generation;
            }
            self.loading += want.len() + sharpen.len();
            let mut q = self.shared.queue.lock().unwrap();
            // Sharpening what is near comes before loading what is far.
            q.extend(sharpen.iter().map(|&mi| Job::Sharpen(self.hashes[mi as usize])));
            q.extend(want.iter().map(|&(mi, (_, full, far))| Job::Load(self.hashes[mi as usize], full, far)));
            drop(q);
            self.shared.wake.notify_all();
        }
    }
}

impl Drop for Streamer {
    fn drop(&mut self) {
        self.shared.quit.store(true, Ordering::Relaxed);
        self.shared.queue.lock().unwrap().clear();
        self.shared.wake.notify_all();
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
    }
}

fn worker(shared: &Shared, tx: &mpsc::Sender<Done>) {
    loop {
        let model = {
            let mut q = shared.queue.lock().unwrap();
            loop {
                if shared.quit.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(m) = q.pop_front() {
                    break m;
                }
                q = shared.wake.wait(q).unwrap();
            }
        };
        let (model, full, far, sharpen) = match model {
            Job::Load(m, full, far) => (m, full, far, false),
            Job::Sharpen(m) => (m, true, false, true),
        };
        let m = shared.index.by_hash.get(&model).and_then(|&(ai, off, entry)| {
            let claim = |k| {
                if sharpen {
                    true
                } else if full {
                    shared.claimed.lock().unwrap().insert(k)
                } else {
                    !shared.claimed.lock().unwrap().contains(&k) && shared.claimed_small.lock().unwrap().insert(k)
                }
            };
            load_model(&shared.archives[ai], off, entry, model, full, far, &shared.textures, &claim)
        });
        if tx.send((model, m, sharpen)).is_err() {
            return;
        }
    }
}

/// A placed entity and its place in the LOD tree: how many children it
/// stands in for (0 on detailed models), its LOD distance (-1: the
/// archetype's) and the distance within which its children take over.
pub struct Entity {
    pub place: Placement,
    /// -1 when no stand-in is over it.
    pub parent: i32,
    pub children: u32,
    pub lod_dist: f32,
    pub child_lod_dist: f32,
}

/// Archetypes from the cache the `archetypes` tool writes (16 bytes each):
/// model → (LOD distance, bounding radius).
pub fn load_archetypes(path: &Path) -> HashMap<u32, (f32, f32)> {
    let data = std::fs::read(path).unwrap_or_default();
    let mut out = HashMap::with_capacity(data.len() / 16);
    for r in data.as_chunks::<16>().0 {
        let w = |o: usize| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let (lod, radius) = (f32::from_bits(w(4)), f32::from_bits(w(8)));
        if lod.is_finite() && lod > 0.0 {
            out.entry(w(0)).or_insert((lod, radius));
        }
    }
    out
}

/// The records of a placement cache that are in local coordinates rather
/// than the world's: interiors (shops, flats, their shells and contents) and
/// entity sets, stored around the origin to be placed relative to the
/// interior that holds them (`interiors.bin` has them where the world puts
/// them). Drawn where they are, they pile up in the bay at the origin into
/// one heap whose models do not fit in video memory. The cache keeps each resource's
/// records together, so a run of records near the origin whose median lies
/// within 32 m of it is one or more interiors (369 070 of the 2 039 513 map
/// entities, in three runs; larger ones reach 150 m out); batch instances
/// are interleaved, so any record within 32 m counts too (density there is
/// seventeen times the bay's). Found once per cache file, by record index.
pub fn local_records(path: &Path, rec: usize) -> Arc<HashSet<u32>> {
    static CACHE: Mutex<Option<HashMap<std::path::PathBuf, Arc<HashSet<u32>>>>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap();
    let map = cache.get_or_insert_with(HashMap::new);
    if let Some(s) = map.get(path) {
        return s.clone();
    }
    use std::io::Read;
    let mut out = HashSet::new();
    let mut run: Vec<(u32, f32)> = Vec::new();
    let close = |run: &mut Vec<(u32, f32)>, out: &mut HashSet<u32>| {
        if !run.is_empty() {
            let mut d: Vec<f32> = run.iter().map(|r| r.1).collect();
            let mid = d.len() / 2;
            d.select_nth_unstable_by(mid, f32::total_cmp);
            if d[mid] < 32.0 {
                out.extend(run.iter().map(|r| r.0));
            }
            run.clear();
        }
    };
    if let Ok(f) = std::fs::File::open(path) {
        let mut file = std::io::BufReader::with_capacity(1 << 20, f);
        let mut buf = vec![0u8; rec];
        let mut i = 0u32;
        while file.read_exact(&mut buf).is_ok() {
            let f = |o: usize| f32::from_le_bytes(buf[o..o + 4].try_into().unwrap());
            let (r, z) = (f(4).hypot(f(8)), f(12).abs());
            if r < 32.0 && z < 64.0 {
                out.insert(i);
            }
            if r < 160.0 && z < 160.0 {
                run.push((i, r));
            } else {
                close(&mut run, &mut out);
            }
            i += 1;
        }
        close(&mut run, &mut out);
    }
    log::info!("{}: {} records in local coordinates left out", path.display(), out.len());
    let out = Arc::new(out);
    map.insert(path.to_path_buf(), out.clone());
    out
}

/// Entities from a cache the `entities` tool writes (`rec` bytes each: 60
/// for map entities, 48 for batch instances, which have no LOD fields),
/// keeping those within `r` of (x, y), without those in local coordinates
/// (`local_records`). Windows are left open.
pub fn load_entities(path: &Path, x: f32, y: f32, r: f32, rec: usize) -> std::io::Result<Vec<Entity>> {
    use std::io::Read;
    let mut file = std::io::BufReader::with_capacity(1 << 20, std::fs::File::open(path)?);
    let local = local_records(path, rec);
    let mut rec = vec![0u8; rec];
    let mut out = Vec::new();
    let mut i = 0u32;
    while file.read_exact(&mut rec).is_ok() {
        let f = |o: usize| f32::from_le_bytes(rec[o..o + 4].try_into().unwrap());
        let pos = glam::Vec3::new(f(4), f(8), f(12));
        i += 1;
        if (pos.x - x).abs() > r || (pos.y - y).abs() > r || local.contains(&(i - 1)) {
            continue;
        }
        let model = u32::from_le_bytes(rec[0..4].try_into().unwrap());
        let rot = glam::Quat::from_xyzw(f(16), f(20), f(24), f(28));
        let (sxy, sz) = (f(32), f(36));
        let g = |o: usize| rec.get(o..o + 4).map(|b| <[u8; 4]>::try_from(b).unwrap());
        let (lod_dist, child_lod_dist) = (g(48).map_or(-1.0, f32::from_le_bytes), g(52).map_or(0.0, f32::from_le_bytes));
        let children = g(56).map_or(0, u32::from_le_bytes);
        let parent = g(40).map_or(-1, i32::from_le_bytes);
        let rot = if rot.is_finite() && rot.length_squared() > 0.5 { rot.normalize() } else { glam::Quat::IDENTITY };
        // About 1% of batch instances decode to absurd scales (a record
        // layout Vice Lens misreads); leave them out.
        let scale = glam::Vec3::new(sxy, sxy, sz);
        if !(scale.is_finite() && scale.min_element() > 0.02 && scale.max_element() < 20.0) {
            continue;
        }
        out.push(Entity { place: Placement { model, pos, rot, scale, near: 0.0, far: f32::INFINITY, id: 0 }, parent, children, lod_dist, child_lod_dist });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    /// Interiors are found as runs of records near the origin, whatever
    /// world records come before and after them.
    #[test]
    fn interiors_found_by_runs_near_the_origin() {
        let rec = |x: f32, y: f32, z: f32| {
            let mut r = vec![0u8; 60];
            for (o, v) in [(4, x), (8, y), (12, z)] {
                r[o..o + 4].copy_from_slice(&v.to_le_bytes());
            }
            r
        };
        let mut file = Vec::new();
        file.extend(rec(-560.0, 44.0, 10.0)); // 0: downtown
        for k in 0..9 {
            file.extend(rec(k as f32 * 3.0 - 12.0, 5.0, 2.0)); // 1..=9: a shop
        }
        file.extend(rec(120.0, 40.0, 0.0)); // 10: the shop's far corner
        file.extend(rec(3000.0, -200.0, 5.0)); // 11: elsewhere
        file.extend(rec(140.0, 20.0, 1.0)); // 12: a pier near the origin, alone
        let path = std::env::temp_dir().join(format!("vice-lens-local-{}.bin", std::process::id()));
        std::fs::write(&path, &file).unwrap();
        let local = super::local_records(&path, 60);
        let _ = std::fs::remove_file(&path);
        let mut got: Vec<u32> = local.iter().copied().collect();
        got.sort();
        assert_eq!(got, (1..=10).collect::<Vec<u32>>());
    }
}
