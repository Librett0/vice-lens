//! The island map: drawn once by Vice Lens from the game's LOD stand-ins
//! seen from straight above, in three styles (`STYLES`), kept in the cache
//! (`map.png`, `map-clay.png`, `map-relief.png` and `map.txt`), and shown in
//! the window as a minimap and a full map.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::{view3d, world};

/// The map's north-west corner (metres), tile edge, tiles east and south,
/// and metres per pixel.
const WEST: f32 = -10240.0;
const NORTH: f32 = 10240.0;
const TILE: f32 = 2048.0;
const TILES: [u32; 2] = [8, 10];
const PER_PX: f32 = 2.0;

/// The map's styles: name, file in the cache, and the 3D view's `look`.
/// Satellite shows the textures; clay one pale colour, so buildings read
/// by their shape and shade; relief colours the ground by its height, with
/// contour lines every 10 m.
pub const STYLES: [(&str, &str, u32); 3] = [("satellite", "map.png", 0), ("clay", "map-clay.png", 1), ("relief", "map-relief.png", 2)];
/// Stand-ins this near a tile's centre are loaded for it.
const REACH: f32 = 1500.0;

/// The map image and where it lies: its north-west corner, metres per
/// pixel, size, and RGBA pixels (sRGB), north up.
pub struct MapImage {
    pub west: f32,
    pub north: f32,
    pub per_px: f32,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl MapImage {
    /// The satellite map from the cache, if it was made, at most 8192
    /// pixels across (the largest texture every GPU takes), for the 3D
    /// window.
    pub fn load() -> Option<MapImage> {
        let mut m = Self::load_style("satellite")?;
        while m.w.max(m.h) > 8192 {
            m = m.halved();
        }
        Some(m)
    }

    /// One style of the map, whole, if it was made.
    pub fn load_style(style: &str) -> Option<MapImage> {
        let dir = crate::dirs_cache();
        let file = STYLES.iter().find(|s| s.0 == style)?.1;
        let meta = std::fs::read_to_string(dir.join("map.txt")).ok()?;
        let v: Vec<f32> = meta.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        let [west, north, per_px, ..] = v[..] else { return None };
        let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(dir.join(file)).ok()?));
        let mut r = dec.read_info().ok()?;
        let mut rgba = vec![0; r.output_buffer_size()?];
        let info = r.next_frame(&mut rgba).ok()?;
        rgba.truncate(info.buffer_size());
        Some(MapImage { west, north, per_px, w: info.width, h: info.height, rgba })
    }

    /// The styles made so far.
    pub fn styles() -> Vec<&'static str> {
        let dir = crate::dirs_cache();
        STYLES.iter().filter(|s| dir.join(s.1).exists()).map(|s| s.0).collect()
    }

    /// The map at half the size (box filter).
    pub fn halved(&self) -> MapImage {
        let (w, h) = ((self.w / 2).max(1), (self.h / 2).max(1));
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                for c in 0..4 {
                    let s = |dx: u32, dy: u32| self.rgba[(((y * 2 + dy).min(self.h - 1) * self.w + (x * 2 + dx).min(self.w - 1)) * 4 + c) as usize] as u32;
                    rgba[((y * w + x) * 4 + c) as usize] = ((s(0, 0) + s(1, 0) + s(0, 1) + s(1, 1)) / 4) as u8;
                }
            }
        }
        MapImage { west: self.west, north: self.north, per_px: self.per_px * 2.0, w, h, rgba }
    }
}

/// Draw the map: for each 2 km tile, stream in the LOD stand-ins and water
/// around it, render it from above in each style (orthographic, 1 m a
/// pixel, halved to 2), and stitch the tiles into the styles' images.
/// `VICE_LENS_MAP_TILES=x,y;x,y` draws only those tiles, into
/// `map-test-<style>.png`, to try a change.
pub fn make(game: &Path, threads: usize, progress: &dyn Fn(u64, u64)) {
    let t = Instant::now();
    let cache = crate::dirs_cache();
    let mut index = world::ModelIndex::load(&crate::caches::MODEL_LISTS.map(|f| cache.join(f)));
    index.add_dicts(&cache.join("dicts-levels.tsv"), "levels.rpf");
    let all = world::load_entities(&cache.join("entities.bin"), 0.0, 0.0, 1e7, 60).unwrap_or_default();
    let mut placements = Vec::new();
    for e in all {
        let mut p = e.place;
        let n = index.names.get(&p.model).map(String::as_str).unwrap_or("");
        let lod = (e.children > 0 && e.child_lod_dist < 250.0) || (e.children == 0 && n.ends_with("_lod") && !n.contains("slod"));
        if lod || index.water.contains(&p.model) {
            (p.near, p.far) = (0.0, REACH);
            placements.push(p);
        }
    }
    log::info!("map: {} stand-ins and water tiles", placements.len());
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(desc);
    let (_, device, queue) = crate::adapter(&instance, None);
    let mut v = view3d::View3d::new(device, queue, view3d::COLOR);
    v.shadows = false;
    v.haze = false;
    // Lit from the north-west, as relief maps are.
    v.sun = glam::Vec3::new(-0.4, 0.35, 0.85).normalize();
    let (w, h) = (TILES[0] * (TILE / PER_PX) as u32, TILES[1] * (TILE / PER_PX) as u32);
    let sea = v.add_sea(WEST + w as f32 * PER_PX * 0.5, NORTH - h as f32 * PER_PX * 0.5, -0.4, 30000.0);
    let mut streamer = world::Streamer::new(game, index, placements, 0.0, REACH, threads);
    streamer.keep(sea, glam::Mat4::IDENTITY);
    streamer.thumbnails = true;
    streamer.fades = false;
    let side = (TILE / PER_PX) as usize;
    let only: Option<Vec<(u32, u32)>> =
        std::env::var("VICE_LENS_MAP_TILES").ok().map(|v| v.split(';').filter_map(|p| p.split_once(',')).filter_map(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?))).collect());
    let mut imgs: Vec<Vec<u8>> = STYLES.iter().map(|_| vec![0u8; (w * h * 4) as usize]).collect();
    let tiles: Vec<(u32, u32)> = only.clone().unwrap_or_else(|| (0..TILES[1]).flat_map(|ty| (0..TILES[0]).map(move |tx| (tx, ty))).collect());
    for (done, &(tx, ty)) in tiles.iter().enumerate() {
        {
            let (cx, cy) = (WEST + (tx as f32 + 0.5) * TILE, NORTH - (ty as f32 + 0.5) * TILE);
            let cam = view3d::Camera { eye: glam::Vec3::new(cx, cy, 3000.0), yaw: std::f32::consts::FRAC_PI_2, pitch: -std::f32::consts::FRAC_PI_2, fov: 55.0, ortho: TILE * 0.5 };
            loop {
                streamer.update(&mut v, cam.eye, Duration::from_millis(50));
                if streamer.loading() == 0 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            for (img, style) in imgs.iter_mut().zip(STYLES) {
                v.look = style.2;
                // The plain looks have the sun lower, for the relief to
                // read by its shading.
                v.sun = if style.2 == 0 { glam::Vec3::new(-0.4, 0.35, 0.85) } else { glam::Vec3::new(-0.55, 0.45, 0.62) }.normalize();
                let px = v.capture(&cam, [side as u32 * 2; 2]);
                // Halve (box filter) into the map.
                for y in 0..side {
                    for x in 0..side {
                        let o = ((ty as usize * side + y) * w as usize + tx as usize * side + x) * 4;
                        for c in 0..4 {
                            let s = |dx: usize, dy: usize| px[((y * 2 + dy) * side * 2 + x * 2 + dx) * 4 + c] as u32;
                            img[o + c] = ((s(0, 0) + s(1, 0) + s(0, 1) + s(1, 1)) / 4) as u8;
                        }
                    }
                }
            }
            v.look = 0;
            let n = done as u64 + 1;
            progress(n, tiles.len() as u64);
            log::info!("map tile {n}/{} ({tx}, {ty}): {} models, MiB {:?} (geometry buffers {}), {:.0?}", tiles.len(), streamer.ready, v.memory(), v.geometry_reserved(), t.elapsed());
        }
    }
    if let Some(only) = only {
        // Each tile tried, cut out of the map, side by side.
        for (img, style) in imgs.iter().zip(STYLES) {
            let mut out = vec![0u8; side * side * only.len() * 4];
            for (k, &(tx, ty)) in only.iter().enumerate() {
                for y in 0..side {
                    let from = ((ty as usize * side + y) * w as usize + tx as usize * side) * 4;
                    let to = (y * side * only.len() + k * side) * 4;
                    out[to..to + side * 4].copy_from_slice(&img[from..from + side * 4]);
                }
            }
            crate::save_png(&cache.join(format!("map-test-{}.png", style.0)), &out, [(side * only.len()) as u32, side as u32]);
        }
        log::info!("map tiles {only:?} drawn in {:.1?}", t.elapsed());
        return;
    }
    for (img, style) in imgs.iter().zip(STYLES) {
        crate::save_png(&cache.join(style.1), img, [w, h]);
    }
    std::fs::write(cache.join("map.txt"), format!("{WEST} {NORTH} {PER_PX} {w} {h}\n")).expect("map.txt");
    log::info!("map made in {:.1?}", t.elapsed());
}
