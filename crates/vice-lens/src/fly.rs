//! A window to fly around the world: WASD moves, Q/E go down/up, the arrow
//! keys or dragging with the left mouse button look around, Shift is fast,
//! Escape quits. F12 saves the current frame to `captures/`.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use glam::Vec3;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::view3d::{Camera, MapView, View3d};

/// What to show. `update` runs before every frame: it uploads models and
/// places instances as they become ready, and may return a status line for
/// the window title.
pub trait Scene {
    fn update(&mut self, view: &mut View3d, cam: &Camera) -> Option<String>;
}

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    view: View3d,
}

pub struct Fly {
    gpu: Option<Gpu>,
    scene: Box<dyn Scene>,
    title_at: Instant,
    cam: Camera,
    keys: HashSet<KeyCode>,
    drag: Option<(f64, f64)>,
    last: Instant,
    /// The island map, until it is uploaded.
    map_image: Option<crate::map::MapImage>,
    /// The full map, when open: the world point at the window's centre and
    /// metres per pixel.
    map: Option<(glam::Vec2, f32)>,
    cursor: glam::Vec2,
    /// Where the left button went down on the full map, and how far the
    /// pointer has moved since (a click teleports, a drag pans).
    press: Option<(glam::Vec2, f32)>,
}

/// Minimap: edge (pixels) and metres per pixel.
const MINIMAP: f32 = 230.0;
const MINIMAP_SCALE: f32 = 2.2;

pub fn run(cam: Camera, scene: Box<dyn Scene>) {
    let el = EventLoop::new().expect("event loop");
    let map_image = crate::map::MapImage::load();
    if map_image.is_none() {
        log::info!("no map yet: `vice-lens --make-map` draws it (minimap and M)");
    }
    let mut app = Fly { gpu: None, scene, title_at: Instant::now(), cam, keys: HashSet::new(), drag: None, last: Instant::now(), map_image, map: None, cursor: glam::Vec2::ZERO, press: None };
    el.run_app(&mut app).expect("run");
}

/// What the overlay draws: the full map (centre, metres per pixel) when
/// open, with a cross at the cursor; else the minimap, heading up.
pub fn map_views(cam: &Camera, size: glam::Vec2, map: Option<(glam::Vec2, f32)>, cursor: Option<glam::Vec2>) -> Vec<MapView> {
    let player = glam::Vec3::new(cam.eye.x, cam.eye.y, cam.yaw);
    match map {
        Some((centre, per_px)) => vec![MapView { rect: [0.0, 0.0, size.x, size.y], round: false, centre, per_px, turn: 0.0, player, cross: cursor }],
        None => vec![MapView {
            rect: [24.0, size.y - 24.0 - MINIMAP, MINIMAP, MINIMAP],
            round: true,
            centre: cam.eye.truncate(),
            per_px: MINIMAP_SCALE,
            turn: cam.yaw - std::f32::consts::FRAC_PI_2,
            player,
            cross: None,
        }],
    }
}

impl Fly {
    fn size(&self) -> glam::Vec2 {
        self.gpu.as_ref().map_or(glam::Vec2::ONE, |g| glam::Vec2::new(g.config.width as f32, g.config.height as f32))
    }

    /// The world point under a screen point of the full map.
    fn map_world(&self, at: glam::Vec2) -> glam::Vec2 {
        let (centre, per_px) = self.map.unwrap_or((glam::Vec2::ZERO, 1.0));
        let d = at - self.size() * 0.5;
        centre + glam::Vec2::new(d.x, -d.y) * per_px
    }

    fn map_views(&self) -> Vec<MapView> {
        map_views(&self.cam, self.size(), self.map, Some(self.cursor))
    }
}

impl ApplicationHandler for Fly {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let window = Arc::new(el.create_window(Window::default_attributes().with_title("Vice Lens 3D")).expect("window"));
        let mut desc = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(window.clone()));
        desc.backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(desc);
        let surface = instance.create_surface(window.clone()).expect("surface");
        let (adapter, device, queue) = crate::adapter(&instance, Some(&surface));
        let size = window.inner_size();
        let mut config = surface.get_default_config(&adapter, size.width.max(1), size.height.max(1)).expect("surface config");
        // The 3D view writes linear light; the surface encodes it.
        if let Some(f) = surface.get_capabilities(&adapter).formats.iter().find(|f| f.is_srgb()) {
            config.format = *f;
        }
        config.usage |= wgpu::TextureUsages::COPY_SRC;
        surface.configure(&device, &config);
        let mut view = View3d::new(device, queue, config.format);
        if let Some(m) = self.map_image.take() {
            view.set_map(&m, config.format);
        }
        window.request_redraw();
        self.gpu = Some(Gpu { window, surface, config, view });
        self.last = Instant::now();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => {
                gpu.config.width = s.width.max(1);
                gpu.config.height = s.height.max(1);
                gpu.surface.configure(gpu.view.device(), &gpu.config);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(k) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        if k == KeyCode::Escape {
                            if self.map.take().is_none() {
                                el.exit();
                            }
                            return;
                        }
                        if k == KeyCode::KeyM && !event.repeat && gpu.view.has_map() {
                            self.map = match self.map {
                                Some(_) => None,
                                None => Some((self.cam.eye.truncate(), 6.0)),
                            };
                            self.press = None;
                            return;
                        }
                        if k == KeyCode::F12 {
                            let size = [gpu.config.width, gpu.config.height];
                            let px = gpu.view.capture(&self.cam, size);
                            let name = format!("captures/fly-{}.png", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
                            crate::save_png(std::path::Path::new(&name), &px, size);
                        }
                        self.keys.insert(k);
                    } else {
                        self.keys.remove(&k);
                    }
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } if self.map.is_some() => {
                if state == ElementState::Pressed {
                    self.press = Some((self.cursor, 0.0));
                } else if let Some((_, moved)) = self.press.take()
                    && moved < 5.0
                {
                    // Teleport to the point clicked, at least 80 m up.
                    let to = self.map_world(self.cursor);
                    self.cam.eye = Vec3::new(to.x, to.y, self.cam.eye.z.max(80.0));
                    self.map = None;
                    log::info!("teleported to ({:.0}, {:.0})", to.x, to.y);
                }
            }
            WindowEvent::MouseWheel { delta, .. } if self.map.is_some() => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                // Zoom about the cursor: the point under it stays.
                let under = self.map_world(self.cursor);
                let (_, per_px) = self.map.unwrap();
                let per_px = (per_px * 0.85f32.powf(lines)).clamp(0.5, 40.0);
                let d = self.cursor - self.size() * 0.5;
                self.map = Some((under - glam::Vec2::new(d.x, -d.y) * per_px, per_px));
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                if state == ElementState::Released {
                    self.drag = None;
                } else {
                    self.drag = Some((f64::NAN, f64::NAN));
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let now = glam::Vec2::new(position.x as f32, position.y as f32);
                let was = std::mem::replace(&mut self.cursor, now);
                if let (Some((centre, per_px)), Some((_, moved))) = (self.map, self.press.as_mut()) {
                    // Pan: the map follows the pointer.
                    let d = now - was;
                    *moved += d.length();
                    self.map = Some((centre - glam::Vec2::new(d.x, -d.y) * per_px, per_px));
                    return;
                }
                if self.map.is_some() {
                    return;
                }
                if let Some((x, y)) = self.drag {
                    if x.is_finite() {
                        self.cam.yaw -= ((position.x - x) * 0.004) as f32;
                        self.cam.pitch = (self.cam.pitch - ((position.y - y) * 0.004) as f32).clamp(-1.5, 1.5);
                    }
                    self.drag = Some((position.x, position.y));
                }
            }
            WindowEvent::RedrawRequested => {
                let dt = self.last.elapsed().as_secs_f32().min(0.1);
                self.last = Instant::now();
                self.step(dt);
                let gpu = self.gpu.as_mut().unwrap();
                let status = self.scene.update(&mut gpu.view, &self.cam);
                if let Some(status) = status.filter(|_| self.title_at.elapsed().as_secs_f32() > 0.5) {
                    let e = self.cam.eye;
                    gpu.window.set_title(&format!("Vice Lens 3D — {status} — ({:.0}, {:.0}, {:.0}) {:.0} fps", e.x, e.y, e.z, 1.0 / dt.max(1e-3)));
                    self.title_at = Instant::now();
                }
                let frame = match gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    _ => {
                        gpu.surface.configure(gpu.view.device(), &gpu.config);
                        gpu.window.request_redraw();
                        return;
                    }
                };
                let target = frame.texture.create_view(&Default::default());
                gpu.view.draw(&self.cam, &target, [gpu.config.width, gpu.config.height]);
                let views = self.map_views();
                let gpu = self.gpu.as_mut().unwrap();
                gpu.view.draw_overlay(&target, [gpu.config.width, gpu.config.height], &views);
                gpu.view.present(frame);
                gpu.window.request_redraw();
            }
            _ => {}
        }
    }
}

impl Fly {
    fn step(&mut self, dt: f32) {
        let k = |c: KeyCode| self.keys.contains(&c);
        let turn = 1.6 * dt;
        if k(KeyCode::ArrowLeft) {
            self.cam.yaw += turn;
        }
        if k(KeyCode::ArrowRight) {
            self.cam.yaw -= turn;
        }
        if k(KeyCode::ArrowUp) {
            self.cam.pitch = (self.cam.pitch + turn).min(1.5);
        }
        if k(KeyCode::ArrowDown) {
            self.cam.pitch = (self.cam.pitch - turn).max(-1.5);
        }
        let speed = if k(KeyCode::ShiftLeft) || k(KeyCode::ShiftRight) { 120.0 } else { 25.0 } * dt;
        let f = self.cam.forward();
        let right = f.cross(Vec3::Z).normalize_or_zero();
        let mut m = Vec3::ZERO;
        if k(KeyCode::KeyW) {
            m += f;
        }
        if k(KeyCode::KeyS) {
            m -= f;
        }
        if k(KeyCode::KeyD) {
            m += right;
        }
        if k(KeyCode::KeyA) {
            m -= right;
        }
        if k(KeyCode::KeyE) {
            m += Vec3::Z;
        }
        if k(KeyCode::KeyQ) {
            m -= Vec3::Z;
        }
        self.cam.eye += m.normalize_or_zero() * speed;
    }
}
