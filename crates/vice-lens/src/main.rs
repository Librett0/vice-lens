//! Vice Lens: explores the Xbox Series data of the game (1.0.2.1) from the
//! user's own copy.
//!
//! vice-lens [--game DIR]                 the desktop app
//! vice-lens --serve [--port N]           the same in a browser
//! vice-lens --build-caches               every cache, from the command line
//! vice-lens --make-map                   the map only
//! vice-lens --world X,Y[,R] [--window]   the 3D world around a point
//!     [--flight FRAMES] [--bench FRAMES] [--capture OUT] [--nav]
//! vice-lens --menu [--lang CODE] [--capture OUT.png] [--size WxH]
//!     [--focus N] [--input up,down,accept,back] [--dump-layout]
//!     [--style NAME]...                  the pause menu
//! vice-lens --model OFFSET [--capture OUT.png]   one model
//!
//! Languages: en pt zh fr it ko es mx (the ones present in the data).
//! Pause menu keys: Up/Down or W/S move the focus, Enter/Space accept,
//! Escape/Backspace go back (and quit at the top).

mod app;
mod assets;
mod caches;
mod fly;
mod lens;
mod map;
mod menu;
mod render;
mod view3d;
mod world;

use std::path::PathBuf;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use assets::Game;
use menu::Menu;
use render::Renderer;

struct Args {
    game: PathBuf,
    lang: String,
    capture: Option<PathBuf>,
    size: [u32; 2],
    focus: usize,
    dump_layout: bool,
    styles: Vec<String>,
    input: String,
    model: Option<u64>,
    window: bool,
    world: Option<(f32, f32, f32)>,
    nav: bool,
    /// Frames of a scripted flight to render (with --world and --capture DIR).
    flight: usize,
    bench: usize,
    make_map: bool,
    build_caches: bool,
    /// Serve Vice Lens on this port; or open it as a desktop app.
    lens: Option<u16>,
    app: bool,
}

/// Arguments that only adjust a mode, not choose one.
const SETTINGS: [&str; 3] = ["--game", "--lang", "--port"];

/// Where Vice Lens keeps the game folder it was given.
fn game_dir_file() -> PathBuf {
    dirs_config().join("game")
}

/// The game folder: VICE_LENS_GAME, else the one saved by Vice Lens (`--game`
/// overrides both).
fn default_game_dir() -> PathBuf {
    std::env::var_os("VICE_LENS_GAME").map(PathBuf::from).or_else(|| std::fs::read_to_string(game_dir_file()).ok().map(|s| PathBuf::from(s.trim()))).unwrap_or_default()
}

/// Remember the game folder for later runs.
pub(crate) fn save_game_dir(p: &std::path::Path) -> std::io::Result<()> {
    let f = game_dir_file();
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(f, p.to_string_lossy().as_bytes())
}

fn args() -> Args {
    let mut a = Args {
        game: default_game_dir(),
        lang: "en".into(),
        capture: None,
        size: [1920, 1080],
        focus: 0,
        dump_layout: false,
        styles: Vec::new(),
        input: String::new(),
        model: None,
        window: false,
        world: None,
        nav: false,
        flight: 0,
        bench: 0,
        make_map: false,
        build_caches: false,
        lens: None,
        app: false,
    };
    // Without a mode, the binary is the desktop app.
    let mut mode = false;
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        mode |= !SETTINGS.contains(&k.as_str());
        match k.as_str() {
            "--game" => a.game = it.next().expect("--game DIR").into(),
            "--lang" => a.lang = it.next().expect("--lang CODE"),
            "--capture" => a.capture = Some(it.next().expect("--capture OUT.png").into()),
            "--size" => {
                let s = it.next().expect("--size WxH");
                let (w, h) = s.split_once('x').expect("WxH");
                a.size = [w.parse().unwrap(), h.parse().unwrap()];
            }
            "--focus" => a.focus = it.next().expect("--focus N").parse().unwrap(),
            "--dump-layout" => a.dump_layout = true,
            "--model" => a.model = Some(u64::from_str_radix(it.next().expect("--model OFFSET").trim_start_matches("0x"), 16).unwrap()),
            "--window" => a.window = true,
            "--nav" => a.nav = true,
            "--flight" => a.flight = it.next().expect("--flight FRAMES").parse().unwrap(),
            "--bench" => a.bench = it.next().expect("--bench FRAMES").parse().unwrap(),
            "--make-map" => a.make_map = true,
            "--build-caches" => a.build_caches = true,
            "--serve" => a.lens = Some(a.lens.unwrap_or(7878)),
            "--menu" => {}
            "--port" => a.lens = Some(it.next().expect("--port N").parse().expect("a port number")),
            "--world" => {
                let v: Vec<f32> = it.next().expect("--world X,Y[,RADIUS]").split(',').map(|x| x.parse().unwrap()).collect();
                a.world = Some((v[0], v[1], v.get(2).copied().unwrap_or(400.0)));
            }
            "--input" => a.input = it.next().expect("--input up,down,accept,back"),
            "--style" => a.styles.push(it.next().expect("--style NAME or conversion")),
            other => panic!("unknown argument {other}"),
        }
    }
    a.app |= !mode && a.lens.is_none();
    a
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn")).init();
    let args = args();
    if args.app {
        app::run(args.game.clone());
        return;
    }
    if let Some(port) = args.lens {
        lens::serve(args.game.clone(), port);
        return;
    }
    if !args.game.join("levels.rpf").exists() {
        eprintln!(
            "No game files found{}.\nVice Lens reads your own copy of the game (the Xbox Series data, 1.0.2.1): \
             pass --game DIR, set VICE_LENS_GAME, or choose the folder in the app (run vice-lens with no arguments).",
            if args.game.as_os_str().is_empty() { String::new() } else { format!(" in {}", args.game.display()) }
        );
        std::process::exit(2);
    }
    if args.build_caches {
        let last = std::sync::Mutex::new((usize::MAX, 0u64));
        let r = caches::build(&args.game, &|step, d, t| {
            let pct = if t > 0 { d * 100 / t } else { 0 };
            let mut l = last.lock().unwrap();
            if l.0 != step || pct >= l.1 + 5 || d == t {
                *l = (step, pct);
                eprintln!("[{}/{}] {}: {pct}%", step + 1, caches::STEPS.len(), caches::STEPS[step].1);
            }
        });
        if let Err(e) = r {
            eprintln!("{e}");
            std::process::exit(1);
        }
        eprintln!("Caches are ready in {}.", dirs_cache().display());
        return;
    }
    if args.make_map {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).saturating_sub(4).clamp(2, 12);
        map::make(&args.game, threads, &|_, _| {});
        return;
    }
    if let Some((x, y, r)) = args.world {
        world_view(&args, x, y, r);
        return;
    }
    if let Some(off) = args.model {
        model_view(&args, off);
        return;
    }
    let game = Game::load(&args.game, &args.lang).expect("game data");
    let mut menu = Menu::new(&game).expect("SCRN_PauseMenu not found in the data");
    menu.set_focus(args.focus);
    for key in args.input.split(',').filter(|k| !k.is_empty()) {
        press(&mut menu, key);
    }
    log::info!("menu items: {:?}", menu.items());
    if !args.styles.is_empty() {
        for name in &args.styles {
            if let Some(c) = game.styles.store.conversions.get(&rage::joaat(name)) {
                println!("{name}: conversion {c:08x?}");
                for (slot, h) in c.iter().enumerate() {
                    println!("  slot {slot}: {:?}", game.styles.look(*h));
                }
            } else {
                let states = game.styles.store.styles.get(&rage::joaat(name));
                println!("{name}: {:?}\n  look: {:?}", states, game.styles.look_named(name));
            }
        }
        return;
    }
    if args.dump_layout {
        print!("{}", menu.dump(&game));
        return;
    }
    if let Some(out) = &args.capture {
        capture(&game, &menu, out, args.size);
        return;
    }
    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App { game, menu, gpu: None };
    event_loop.run_app(&mut app).expect("run");
}

/// Menu input by name: up, down, accept, back. Returns false for back at the
/// top level (leave).
fn press(menu: &mut Menu, key: &str) -> bool {
    match key {
        "up" => menu.move_focus(-1),
        "down" => menu.move_focus(1),
        "accept" => {
            if let Some(todo) = menu.accept() {
                log::warn!("{todo}");
            }
        }
        "back" => return menu.back(),
        other => log::warn!("unknown input {other}"),
    }
    true
}

pub(crate) fn adapter(instance: &wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: surface,
        apply_limit_buckets: false,
    }))
    .expect("no Vulkan adapter");
    log::info!("GPU: {}", adapter.get_info().name);
    // The game's textures are BC-compressed; upload them as they are.
    let required_features = adapter.features() & wgpu::Features::TEXTURE_COMPRESSION_BC;
    let desc = wgpu::DeviceDescriptor { required_features, ..Default::default() };
    let (device, queue) = pollster::block_on(adapter.request_device(&desc)).expect("device");
    (adapter, device, queue)
}

/// Render one drawable from levels.rpf (at `off`) to a PNG.
fn model_view(args: &Args, off: u64) {
    let out = args.capture.clone().unwrap_or_else(|| "model.png".into());
    let textures = world::Textures::open(&args.game);
    let ar_name = std::env::var("VICE_LENS_ARCHIVE").unwrap_or_else(|_| "levels.rpf".into());
    let levels = rage::Archive::open(args.game.join(ar_name)).expect("archive");
    let entry = std::env::var("VICE_LENS_ENTRY").ok().and_then(|e| e.parse().ok());
    let model = world::load_model(&levels, off, entry, 0, true, false, &textures, &|_| true).expect("no drawable there");
    let d = &model.drawable;
    let found = model.textures.iter().filter(|t| t.pixels.is_some()).count();
    log::info!("{}: {} meshes, albedo maps {found}/{}", d.name, d.meshes.len(), model.textures.len());
    let c = glam::Vec3::new(d.sphere[0], d.sphere[1], d.sphere[2]);
    let r = d.sphere[3].max(1.0);
    let yaw: f32 = std::env::var("VICE_LENS_YAW").ok().and_then(|v| v.parse().ok()).unwrap_or(-2.3);
    let pitch: f32 = std::env::var("VICE_LENS_PITCH").ok().and_then(|v| v.parse().ok()).unwrap_or(-0.35);
    let mut cam = view3d::Camera { eye: c, yaw, pitch, fov: 50.0, ortho: 0.0 };
    cam.eye = c - cam.forward() * r * 2.4;
    /// One model, uploaded on the first frame.
    struct One(Option<world::Model>);
    impl fly::Scene for One {
        fn update(&mut self, v: &mut view3d::View3d, _: &view3d::Camera) -> Option<String> {
            if let Some(m) = self.0.take() {
                let id = v.add_model(&m, false);
                v.add_instance(id, glam::Mat4::IDENTITY, view3d::Fade::Whole);
                return Some(m.drawable.name);
            }
            None
        }
    }
    if args.window {
        fly::run(cam, Box::new(One(Some(model))));
        return;
    }
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(desc);
    let (_, device, queue) = adapter(&instance, None);
    let mut v = view3d::View3d::new(device, queue, view3d::COLOR);
    let mut one = One(Some(model));
    fly::Scene::update(&mut one, &mut v, &cam);
    let px = v.capture(&cam, args.size);
    save_png(&out, &px, args.size);
}

/// The world around (x, y): the map's placed entities within `r`, streamed
/// in by worker threads (in the window, around the camera as it flies);
/// with `--nav`, the navigation meshes coloured by surface underneath.
fn world_view(args: &Args, x: f32, y: f32, r: f32) {
    let (parts, ground) = navmesh_ground(args, x, y, r.min(256.0));
    // Full-detail entities within `r` and the game's LOD stand-ins out to
    // `far`; the window reads a wider area to fly over.
    // Stand-ins are drawn out to `far` (their own LOD distances decide
    // which level where); the window and flights read a wider area.
    let far = 8000.0;
    // The window reads the whole island, so a teleport lands somewhere.
    let area = if args.window {
        1e7
    } else if args.flight > 0 || args.bench > 0 {
        far + 1500.0
    } else {
        far
    };
    // models-missed.tsv: drawables the first index pass skipped (the
    // `unnamed` example finds them).
    let mut index = world::ModelIndex::load(&caches::MODEL_LISTS.map(|f| dirs_cache().join(f)));
    index.add_dicts(&dirs_cache().join("dicts-levels.tsv"), "levels.rpf");
    let rules = std::sync::Arc::new(Rules::new(&index, r, far));
    let mut all = world::load_entities(&dirs_cache().join("entities.bin"), x, y, area, 60).unwrap_or_default();
    // Interiors where the world places them (their own records in
    // entities.bin are in local coordinates and left out).
    all.extend(world::load_entities(&dirs_cache().join("interiors.bin"), x, y, area, 60).unwrap_or_default());
    let placements: Vec<world::Placement> = all.into_iter().filter_map(|e| rules.place(e)).collect();
    let batches = if std::env::var("VICE_LENS_NO_INSTANCES").is_err() { load_batches(&rules, glam::Vec2::new(x, y)) } else { Vec::new() };
    log::info!("{} placements, {} batch instances within reach; {} archetypes, LOD distances scaled by {}", placements.len(), batches.len(), rules.archetypes.len(), rules.k);
    let mut cam = view3d::Camera { eye: glam::Vec3::new(x - r * 0.6, y - r * 0.6, ground + r * 0.45), yaw: 0.785, pitch: -0.45, fov: 55.0, ortho: 0.0 };
    cam.eye.z = cam.eye.z.max(ground + 40.0);
    if std::env::var("VICE_LENS_TOPDOWN").is_ok() {
        cam = view3d::Camera { eye: glam::Vec3::new(x, y, ground + r * 1.9), yaw: std::f32::consts::FRAC_PI_2, pitch: -1.5697, fov: 55.0, ortho: 0.0 };
    }
    // VICE_LENS_CAM=x,y,z,yaw,pitch places the camera.
    if let Some(c) = std::env::var("VICE_LENS_CAM").ok().map(|s| s.split(',').filter_map(|v| v.parse::<f32>().ok()).collect::<Vec<_>>()).filter(|c| c.len() == 5) {
        cam = view3d::Camera { eye: glam::Vec3::new(c[0], c[1], c[2]), yaw: c[3], pitch: c[4], fov: 55.0, ortho: 0.0 };
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).saturating_sub(4).clamp(2, 12);
    let mut streamer = world::Streamer::new(&args.game, index, placements, r, far, threads);
    streamer.set_batches(batches);
    // The sea at height 0 out to the horizon, under everything else; far
    // away, where only LOD stand-ins are drawn, it is the ground.
    let nav = Some(if args.nav { parts } else { Vec::new() });
    let sea = (x, y, -0.4, 30000.0);
    if args.window {
        /// The streamed world around the camera.
        /// The streamed world around the camera; batch instances (trees,
        /// small props) are read again on a thread once the camera is
        /// 600 m from where they were last read.
        struct Stream {
            streamer: world::Streamer,
            parts: Option<Vec<(Vec<[f32; 3]>, Vec<u32>, [u8; 4])>>,
            sea: (f32, f32, f32, f32),
            start: std::time::Instant,
            rules: std::sync::Arc<Rules>,
            batches_at: glam::Vec2,
            reading: Option<std::sync::mpsc::Receiver<Vec<world::Placement>>>,
        }
        impl fly::Scene for Stream {
            fn update(&mut self, v: &mut view3d::View3d, cam: &view3d::Camera) -> Option<String> {
                if let Some(parts) = self.parts.take() {
                    let id = v.add_colored(&parts);
                    self.streamer.keep(id, glam::Mat4::IDENTITY);
                    let (x, y, z, e) = self.sea;
                    let id = v.add_sea(x, y, z, e);
                    self.streamer.keep(id, glam::Mat4::IDENTITY);
                }
                let at = cam.eye.truncate();
                if self.reading.is_none() && at.distance(self.batches_at) > 600.0 && std::env::var("VICE_LENS_NO_INSTANCES").is_err() {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let rules = self.rules.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(load_batches(&rules, at));
                    });
                    self.reading = Some(rx);
                    self.batches_at = at;
                }
                if let Some(b) = self.reading.as_ref().and_then(|r| r.try_recv().ok()) {
                    self.streamer.set_batches(b);
                    self.reading = None;
                }
                v.time = self.start.elapsed().as_secs_f32();
                self.streamer.update(v, cam.eye, std::time::Duration::from_millis(8));
                Some(format!("{} models, {} loading, {} objects, M: map", self.streamer.ready, self.streamer.loading(), v.instance_count()))
            }
        }
        let scene = Stream { streamer, parts: nav, sea, start: std::time::Instant::now(), rules, batches_at: glam::Vec2::new(x, y), reading: None };
        fly::run(cam, Box::new(scene));
        return;
    }
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(desc);
    let (_, device, queue) = adapter(&instance, None);
    let mut v = view3d::View3d::new(device, queue, view3d::COLOR);
    if let Some(parts) = nav {
        let id = v.add_colored(&parts);
        streamer.keep(id, glam::Mat4::IDENTITY);
    }
    let id = v.add_sea(sea.0, sea.1, sea.2, sea.3);
    streamer.keep(id, glam::Mat4::IDENTITY);
    if args.flight > 0 {
        flight(args, &mut v, &mut streamer, cam);
        return;
    }
    if args.bench > 0 {
        bench(args, &mut v, &mut streamer, cam);
        return;
    }
    let t = std::time::Instant::now();
    let centre = glam::Vec3::new(x, y, ground);
    loop {
        streamer.update(&mut v, centre, std::time::Duration::from_millis(50));
        if streamer.loading() == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    log::info!("{} objects from {} models ({} without a model) in {:.1?}; MiB (textures, small, meshes) {:?}", v.instance_count(), streamer.ready, streamer.missing, t.elapsed(), v.memory());
    let out = args.capture.clone().unwrap_or_else(|| "world.png".into());
    // VICE_LENS_OVERLAY=minimap or map[:metres per pixel] draws the map views
    // the window would over the frame.
    let overlay = std::env::var("VICE_LENS_OVERLAY").ok().zip(map::MapImage::load());
    let px = match overlay {
        Some((what, m)) => {
            v.set_map(&m, view3d::COLOR);
            let size = glam::Vec2::new(args.size[0] as f32, args.size[1] as f32);
            let full = what.strip_prefix("map").map(|z| (cam.eye.truncate(), z.trim_start_matches(':').parse().unwrap_or(6.0)));
            let views = fly::map_views(&cam, size, full, Some(size * glam::Vec2::new(0.62, 0.4)));
            v.capture_with(&cam, args.size, &views)
        }
        None => v.capture(&cam, args.size),
    };
    save_png(&out, &px, args.size);
}

/// A scripted flight: the camera glides forward and turns slowly, the
/// world streaming in around it; each frame waits for the models in range
/// and is saved as `<capture dir>/NNNN.png`.
fn flight(args: &Args, v: &mut view3d::View3d, streamer: &mut world::Streamer, mut cam: view3d::Camera) {
    let dir = args.capture.clone().unwrap_or_else(|| "flight".into());
    std::fs::create_dir_all(&dir).expect("frames dir");
    let speed: f32 = std::env::var("VICE_LENS_SPEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1.2);
    let turn: f32 = std::env::var("VICE_LENS_TURN").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0015);
    let t = std::time::Instant::now();
    let trace = std::env::var_os("VICE_LENS_TRACE").is_some();
    let mut before: std::collections::HashMap<u32, usize> = Default::default();
    for f in 0..args.flight {
        v.time = f as f32 / 30.0;
        loop {
            streamer.update(v, cam.eye, std::time::Duration::from_millis(50));
            if streamer.loading() == 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        if f == 0
            && let Some(path) = std::env::var_os("VICE_LENS_DUMP")
        {
            let text: String = streamer
                .drawn
                .iter()
                .map(|(m, at)| {
                    let p = at.w_axis;
                    format!("{}\t{:.1}\t{:.1}\t{:.1}\n", streamer.name(*m), p.x, p.y, p.z)
                })
                .collect();
            let _ = std::fs::write(path, text);
        }
        if trace {
            let mut now: std::collections::HashMap<u32, usize> = Default::default();
            for (m, _) in &streamer.drawn {
                *now.entry(*m).or_default() += 1;
            }
            let gone: Vec<String> = before.keys().filter(|m| !now.contains_key(m)).map(|m| streamer.name(*m)).collect();
            let came: Vec<String> = now
                .keys()
                .filter(|m| !before.contains_key(m))
                .map(|m| {
                    let at = streamer.drawn.iter().find(|(x, _)| x == m).unwrap().1;
                    let (s, _, p) = at.to_scale_rotation_translation();
                    format!("{} @{:.0},{:.0},{:.1} x{:.2}", streamer.name(*m), p.x, p.y, p.z, s.x)
                })
                .collect();
            log::info!(
                "frame {f}: {} instances, {} ready; +{} {:?} -{} {:?}",
                v.instance_count(),
                streamer.ready,
                came.len(),
                came.iter().take(12).collect::<Vec<_>>(),
                gone.len(),
                gone.iter().take(12).collect::<Vec<_>>()
            );
            before = now;
        }
        let px = v.capture(&cam, args.size);
        save_png(&dir.join(format!("{f:04}.png")), &px, args.size);
        let fwd = cam.forward();
        cam.eye += glam::Vec3::new(fwd.x, fwd.y, 0.0).normalize_or_zero() * speed;
        cam.yaw += turn;
    }
    log::info!("{} frames, {} objects from {} models, in {:.1?}; MiB (textures, small, meshes) {:?}", args.flight, v.instance_count(), streamer.ready, t.elapsed(), v.memory());
}

/// Which entities are drawn where: each one's window from the LOD tree,
/// as the game draws it. A model is drawn out to its LOD distance (the
/// entity's, else its archetype's), a stand-in from where its children
/// give way. Every distance is scaled by VICE_LENS_LOD_SCALE (1 by default,
/// as on the console; like the PC games' extended distance slider); where
/// a detailed model hands over to its stand-in, at most `r`. A stand-in
/// without a LOD distance of its own or its archetype's ends where the
/// next rung starts. Batch instances carry no LOD fields: a name that
/// marks a stand-in sends them to the stand-ins, from where the rung
/// below gives way (see `batch_near`).
struct Rules {
    archetypes: std::collections::HashMap<u32, (f32, f32)>,
    k: f32,
    r: f32,
    far: f32,
    /// Models named as stand-ins; models never drawn (biped bounds, and
    /// dictionary stand-ins placed as if detailed); water map tiles.
    named_lod: std::collections::HashSet<u32>,
    skip: std::collections::HashSet<u32>,
    water: std::collections::HashSet<u32>,
    /// Batch stand-ins: where each starts, unscaled.
    batch_near: std::collections::HashMap<u32, f32>,
}

/// Where a batch stand-in starts: every rung of a batch is placed at the
/// same spots (a sign, its `_batchlod`, a shared `_batch_slod` box), so
/// each rung starts where the one below ends, by the archetypes' LOD
/// distances: `x_batchlod` (or `x_batch_lod`) where `x` ends (60 m for
/// most props), `x_batch_slod` where `x_batchlod` ends; a shared
/// stand-in (`prop_box_regiontint_batch_slod`) at 250 m, past where the
/// props' `_batchlod`s end (150 to 250 m). Plants work the same way: a
/// `veg_..._imposter` (a few cards) stands at its plant's spot from where
/// the plant ends (75 m for a sapling, 160 m for a small palm).
fn batch_near(name: &str, lod: &dyn Fn(&str) -> Option<f32>) -> Option<f32> {
    if let Some(base) = name.strip_suffix("_batchlod").or_else(|| name.strip_suffix("_batch_lod")).or_else(|| name.strip_suffix("_imposter")) {
        return Some(lod(base).unwrap_or(60.0));
    }
    let base = name.strip_suffix("_batch_slod")?;
    Some(lod(&format!("{base}_batchlod")).or_else(|| lod(&format!("{base}_batch_lod"))).unwrap_or(250.0))
}

impl Rules {
    fn new(index: &world::ModelIndex, r: f32, far: f32) -> Rules {
        let archetypes = world::load_archetypes(&dirs_cache().join("archetypes.bin"));
        let lod = |n: &str| archetypes.get(&rage::joaat(n)).map(|a| a.0);
        let mut named_lod = std::collections::HashSet::new();
        let mut batch = std::collections::HashMap::new();
        let mut skip: std::collections::HashSet<u32> = index.lod.union(&index.slod).copied().collect();
        for (h, n) in &index.names {
            if n.contains("bipedbounds") {
                skip.insert(*h);
            } else if n.ends_with("_lod") || n.contains("slod") || n.contains("batchlod") || n.contains("_lod_") || n.ends_with("_imposter") {
                named_lod.insert(*h);
                if let Some(near) = batch_near(n, &lod) {
                    batch.insert(*h, near);
                }
            }
        }
        Rules { archetypes, k: std::env::var("VICE_LENS_LOD_SCALE").ok().and_then(|s| s.parse().ok()).unwrap_or(1.0), r, far, named_lod, skip, water: index.water.clone(), batch_near: batch }
    }

    /// The entity with its window, or None when it is not drawn.
    fn place(&self, e: world::Entity) -> Option<world::Placement> {
        let mut p = e.place;
        let own = if e.lod_dist > 0.0 { Some(e.lod_dist) } else { self.archetypes.get(&p.model).map(|a| a.0) };
        if e.children > 0 || self.named_lod.contains(&p.model) {
            // A batch instance (no LOD fields), or a map entity with no LOD
            // links (no children), starts where the rung below it ends.
            let batch = if e.lod_dist < 0.0 || e.children == 0 { self.batch_near.get(&p.model).copied() } else { None };
            let c = batch.unwrap_or(e.child_lod_dist.max(0.0));
            let to = own.unwrap_or(match c {
                c if c < 200.0 => 450.0,
                c if c < 600.0 => 1000.0,
                c if c < 1100.0 => 2000.0,
                c if c < 2100.0 => 4000.0,
                _ => 20000.0,
            });
            (p.near, p.far) = ((self.k * c).min(self.r), self.k * to);
            if p.near >= p.far {
                return None;
            }
        } else if self.water.contains(&p.model) {
            (p.near, p.far) = (0.0, self.far);
        } else if self.skip.contains(&p.model) {
            return None;
        } else {
            let to = own.map_or(self.r, |d| self.k * d);
            // Under a stand-in the hand-over is at most `r`; a large model
            // with nothing over it (a bridge, a landmark) keeps its reach.
            let large = self.archetypes.get(&p.model).is_some_and(|a| a.1 >= 40.0);
            (p.near, p.far) = (0.0, if e.parent == -1 && large { to.min(self.far) } else { to.min(self.r) });
        }
        Some(p)
    }
}

/// Batch instances (trees, grass, small props) around `at`, with windows.
fn load_batches(rules: &Rules, at: glam::Vec2) -> Vec<world::Placement> {
    world::load_entities(&dirs_cache().join("instances.bin"), at.x, at.y, rules.r + 1000.0, 48).unwrap_or_default().into_iter().filter_map(|e| rules.place(e)).collect()
}

/// The window's loop without a window: the camera flies forward at
/// VICE_LENS_SPEED m/s (25, the window's walking pace) at 60 steps a second,
/// models stream in without being waited for, and each frame is drawn
/// offscreen at `--size`. Logs where each frame's time went.
fn bench(args: &Args, v: &mut view3d::View3d, streamer: &mut world::Streamer, mut cam: view3d::Camera) {
    use std::time::{Duration, Instant};
    let speed: f32 = std::env::var("VICE_LENS_SPEED").ok().and_then(|s| s.parse().ok()).unwrap_or(25.0);
    let turn: f32 = std::env::var("VICE_LENS_TURN").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let tex = v.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("bench"),
        size: wgpu::Extent3d { width: args.size[0], height: args.size[1], depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: view3d::COLOR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let target = tex.create_view(&Default::default());
    // The starting area loads first, as behind a loading screen.
    let t = Instant::now();
    loop {
        streamer.update(v, cam.eye, Duration::from_millis(50));
        if streamer.loading() == 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    log::info!("start loaded in {:.1?}: {} models", t.elapsed(), streamer.ready);
    let start = Instant::now();
    let mut rows: Vec<[f32; 4]> = Vec::new();
    for f in 0..args.bench {
        let t0 = Instant::now();
        v.time = start.elapsed().as_secs_f32();
        streamer.update(v, cam.eye, Duration::from_millis(8));
        let t1 = Instant::now();
        v.draw(&cam, &target, args.size);
        let t2 = Instant::now();
        let _ = v.device().poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        let t3 = Instant::now();
        let ms = |a: Instant, b: Instant| (b - a).as_secs_f32() * 1000.0;
        let row = [ms(t0, t1), ms(t1, t2), ms(t2, t3), ms(t0, t3)];
        if row[3] > 50.0 {
            log::info!(
                "frame {f}: {:.1} ms (stream {:.1}, draw {:.1}, gpu {:.1}); {} instances, drawn {:?}, {} calls, {} loading",
                row[3],
                row[0],
                row[1],
                row[2],
                v.instance_count(),
                v.drawn,
                v.calls.get(),
                streamer.loading()
            );
        }
        rows.push(row);
        let fwd = cam.forward();
        cam.eye += glam::Vec3::new(fwd.x, fwd.y, 0.0).normalize_or_zero() * speed / 60.0;
        cam.yaw += turn;
    }
    let pct = |k: usize, p: f32| {
        let mut v: Vec<f32> = rows.iter().map(|r| r[k]).collect();
        v.sort_by(f32::total_cmp);
        v[((v.len() - 1) as f32 * p) as usize]
    };
    for (k, what) in ["stream", "draw (cpu)", "gpu wait", "frame"].iter().enumerate() {
        log::info!("{what:>11}: median {:.1} ms, p95 {:.1}, max {:.1}", pct(k, 0.5), pct(k, 0.95), pct(k, 1.0));
    }
    let slow = rows.iter().filter(|r| r[3] > 33.3).count();
    log::info!("{} frames in {:.1?}: {slow} over 33 ms; {} instances, {} models", rows.len(), start.elapsed(), v.instance_count(), streamer.ready);
}

/// Navigation mesh tiles around (x, y), coloured by surface, and their mean
/// height.
fn navmesh_ground(args: &Args, x: f32, y: f32, r: f32) -> (Vec<(Vec<[f32; 3]>, Vec<u32>, [u8; 4])>, f32) {
    use rage::navmesh::{self, Surface};
    let nav = rage::Archive::open(args.game.join("navmeshes.rpf")).expect("navmeshes.rpf");
    // The navmesh list is optional: without it the ground height is 0.
    let Ok(list) = std::fs::read_to_string(dirs_cache().join("navmesh.tsv")) else { return (Vec::new(), 0.0) };
    const COLORS: [[u8; 4]; 5] = [[120, 132, 96, 255], [60, 110, 150, 255], [70, 70, 76, 255], [168, 162, 150, 255], [150, 110, 90, 255]];
    let mut parts: Vec<(Vec<[f32; 3]>, Vec<u32>, [u8; 4])> = Vec::new();
    let (mut tiles, mut zsum, mut zn) = (0, 0.0f64, 0usize);
    for line in list.lines() {
        let Some((off, name)) = line.split_once('\t') else { continue };
        // nav_<gx>_<gy>_(<x>,<y>).ynv
        let Some(c) = name.split('(').nth(1).and_then(|s| s.split(')').next()) else { continue };
        let Some((tx, ty)) = c.split_once(',') else { continue };
        let (tx, ty): (f32, f32) = (tx.parse().unwrap_or(1e9), ty.parse().unwrap_or(1e9));
        if (tx + 64.0 - x).abs() > r + 64.0 || (ty + 64.0 - y).abs() > r + 64.0 {
            continue;
        }
        let off = u64::from_str_radix(off.trim_start_matches("0x"), 16).unwrap();
        let Ok(res) = nav.resource_at(off) else { continue };
        let Some(m) = navmesh::parse(&res.virt) else { continue };
        tiles += 1;
        for (k, color) in COLORS.iter().enumerate() {
            let want = [Surface::Land, Surface::Water, Surface::Road, Surface::Pavement, Surface::Interior][k];
            let tris: Vec<&[u32; 3]> = m.triangles.iter().zip(&m.surfaces).filter(|(_, s)| **s == want).map(|(t, _)| t).collect();
            if tris.is_empty() {
                continue;
            }
            let mut pos = Vec::new();
            let mut idx = Vec::new();
            let mut remap = std::collections::HashMap::new();
            for t in tris {
                for &i in t {
                    let n = *remap.entry(i).or_insert_with(|| {
                        let p = m.positions[i as usize];
                        zsum += p[2] as f64;
                        zn += 1;
                        pos.push(p);
                        pos.len() as u32 - 1
                    });
                    idx.push(n);
                }
            }
            parts.push((pos, idx, *color));
        }
    }
    let ground = if zn > 0 { (zsum / zn as f64) as f32 } else { 0.0 };
    log::info!("{tiles} navmesh tiles around ({x}, {y}); mean height {ground:.1} m");
    (parts, ground)
}

/// Vice Lens's cache directory.
pub(crate) fn dirs_cache() -> PathBuf {
    if let Some(d) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(d).join("vice-lens");
    }
    match std::env::var_os("HOME") {
        Some(h) => PathBuf::from(h).join(".cache/vice-lens"),
        // Windows.
        None => PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("Vice Lens/cache"),
    }
}

/// Where Vice Lens keeps its settings.
pub(crate) fn dirs_config() -> PathBuf {
    if let Some(d) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(d).join("vice-lens");
    }
    match std::env::var_os("HOME") {
        Some(h) => PathBuf::from(h).join(".config/vice-lens"),
        None => PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("Vice Lens"),
    }
}

pub(crate) fn save_png(out: &std::path::Path, px: &[u8], size: [u32; 2]) {
    let file = std::fs::File::create(out).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), size[0], size[1]);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(px).unwrap();
    log::info!("wrote {}", out.display());
}

fn capture(game: &Game, menu: &Menu, out: &std::path::Path, size: [u32; 2]) {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(desc);
    let (_, device, queue) = adapter(&instance, None);
    let mut r = Renderer::new(device, queue, wgpu::TextureFormat::Rgba8Unorm);
    let commands = menu.commands(game);
    let px = r.capture(game, &commands, size);
    save_png(out, &px, size);
}

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
}

struct App {
    game: Game,
    menu: Menu,
    gpu: Option<Gpu>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let window = Arc::new(el.create_window(Window::default_attributes().with_title("Vice Lens — pause menu")).expect("window"));
        let mut desc = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(window.clone()));
        desc.backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(desc);
        let surface = instance.create_surface(window.clone()).expect("surface");
        let (adapter, device, queue) = adapter(&instance, Some(&surface));
        let size = window.inner_size();
        let mut config = surface.get_default_config(&adapter, size.width.max(1), size.height.max(1)).expect("surface config");
        // Colours in the data are display-referred: write them unchanged.
        let caps = surface.get_capabilities(&adapter);
        if let Some(f) = caps.formats.iter().find(|f| !f.is_srgb()) {
            config.format = *f;
        }
        surface.configure(&device, &config);
        let renderer = Renderer::new(device, queue, config.format);
        window.request_redraw();
        self.gpu = Some(Gpu { window, surface, config, renderer });
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => {
                gpu.config.width = s.width.max(1);
                gpu.config.height = s.height.max(1);
                gpu.surface.configure(gpu.renderer.device(), &gpu.config);
                gpu.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let key = match event.physical_key {
                    PhysicalKey::Code(KeyCode::ArrowUp | KeyCode::KeyW) => "up",
                    PhysicalKey::Code(KeyCode::ArrowDown | KeyCode::KeyS) => "down",
                    PhysicalKey::Code(KeyCode::Enter | KeyCode::Space) => "accept",
                    PhysicalKey::Code(KeyCode::Escape | KeyCode::Backspace) => "back",
                    _ => "",
                };
                if !key.is_empty() && !press(&mut self.menu, key) {
                    el.exit();
                }
                gpu.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let frame = match gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    _ => {
                        gpu.surface.configure(gpu.renderer.device(), &gpu.config);
                        return;
                    }
                };
                let view = frame.texture.create_view(&Default::default());
                let commands = self.menu.commands(&self.game);
                gpu.renderer.draw(&self.game, &commands, [0.0, 0.0, 0.0, 1.0], &view, [gpu.config.width, gpu.config.height]);
                gpu.renderer.present(frame);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    /// A batch prop's rungs follow on: its `_batchlod` from where the prop
    /// ends, its `_batch_slod` from where the `_batchlod` ends, a shared
    /// stand-in from 250 m; anything else is not a batch stand-in.
    #[test]
    fn batch_rungs_follow_on() {
        let lod = |n: &str| match n {
            "prop_sign" => Some(60.0),
            "prop_sign_batchlod" => Some(250.0),
            _ => None,
        };
        assert_eq!(super::batch_near("prop_sign_batchlod", &lod), Some(60.0));
        assert_eq!(super::batch_near("prop_sign_batch_slod", &lod), Some(250.0));
        assert_eq!(super::batch_near("prop_box_regiontint_batch_slod", &lod), Some(250.0));
        assert_eq!(super::batch_near("prop_other_batch_lod", &lod), Some(60.0));
        assert_eq!(super::batch_near("s03_w04_009_lod", &lod), None);
        assert_eq!(super::batch_near("prop_sign_imposter", &lod), Some(60.0));
    }
}
