//! 3D view of the world (wgpu, Vulkan): textured meshes under a sky with a
//! sun, sun shadows in two cascades, water with waves and reflections,
//! height haze and filmic tone mapping. Z is up, as in the game data.
//! Depth is reversed (1 at the eye, 0 at infinity): float depth then keeps
//! its precision far away, where the city meets the sea.

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use rage::drawable::Bucket;

use crate::world::{self, Model};

pub const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Shadow map edge, texels, per cascade.
const SHADOW: u32 = 4096;
/// Where the two shadow cascades end, metres from the eye.
const CASCADES: [f32; 3] = [0.5, 650.0, 3200.0];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pos: [f32; 3],
    /// Unit normal as signed bytes (w unused).
    normal: [i8; 4],
    uv: [f32; 2],
}

fn pack_normal(n: [f32; 3]) -> [i8; 4] {
    let q = |v: f32| (v.clamp(-1.0, 1.0) * 127.0).round() as i8;
    [q(n[0]), q(n[1]), q(n[2]), 0]
}

/// Index buffer contents: 16-bit when every index fits (padded to an even
/// count for 4-byte copies).
fn index_bytes(idx: &[u32]) -> (Vec<u8>, wgpu::IndexFormat) {
    if idx.iter().all(|&i| i < 0xffff) {
        let mut v: Vec<u16> = idx.iter().map(|&i| i as u16).collect();
        if v.len() % 2 == 1 {
            v.push(0);
        }
        (bytemuck::cast_slice(&v).to_vec(), wgpu::IndexFormat::Uint16)
    } else {
        (bytemuck::cast_slice(idx).to_vec(), wgpu::IndexFormat::Uint32)
    }
}

/// The vertices a drawable mesh draws with (smooth normals from its
/// triangles), texture coordinates from the UV set its albedo uses.
pub fn vertices(mesh: &rage::drawable::Mesh, uv1: bool) -> Vec<Vertex> {
    let normals = normals(&mesh.positions, &mesh.indices);
    let uvs = if uv1 && mesh.uvs1.len() == mesh.positions.len() { &mesh.uvs1 } else { &mesh.uvs };
    mesh.positions.iter().zip(&normals).zip(uvs).map(|((p, n), uv)| Vertex { pos: *p, normal: pack_normal(*n), uv: *uv }).collect()
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CamU {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    shadow: [[[f32; 4]; 4]; 2],
    /// Towards the sun; w: seconds, for the waves.
    sun: [f32; 4],
    /// w: 1 when the shadow maps are drawn.
    eye: [f32; 4],
    /// Depth bias of cascades 0 and 1, then their texel size in metres.
    shadow_k: [f32; 4],
    /// x: seconds an instance takes to dissolve in or out.
    fade: [f32; 4],
}

/// A texture meshes draw with: its small mipmapped copy always, its
/// full-size level while it is near enough and fits the budget.
struct Tex {
    group: wgpu::BindGroup,
    small: wgpu::TextureView,
    /// The full-size level and its bytes.
    full: Option<(wgpu::Texture, u64)>,
    /// Came from game data and can take a full-size level.
    upgradable: bool,
    /// The instance rebuild (`View3d::stamp`) that last placed it.
    last: u64,
    /// Meshes drawing with it; at 0 (when their models are freed) it goes.
    users: u32,
    small_bytes: u64,
    key: Option<u64>,
}

/// Geometry shares a few large buffers: chunks of `CHUNK` bytes in which
/// each mesh takes a range, found first-fit in a free list, given back when
/// its model is freed. Draws then bind buffers once per chunk instead of
/// once per mesh.
struct Arena {
    usage: wgpu::BufferUsages,
    /// Bytes per unit (a vertex, an index; 16-bit indices go in pairs).
    unit: u64,
    /// Per chunk: the buffer and its free ranges (start, units), sorted.
    chunks: Vec<(wgpu::Buffer, Vec<(u64, u64)>)>,
}

const CHUNK: u64 = 128 << 20;

impl Arena {
    fn new(usage: wgpu::BufferUsages, unit: u64) -> Arena {
        Arena { usage: usage | wgpu::BufferUsages::COPY_DST, unit, chunks: Vec::new() }
    }

    /// Room for `data` (a whole number of units, 4-byte aligned): (chunk,
    /// first unit).
    fn put(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) -> (u32, u64) {
        let units = (data.len() as u64).div_ceil(self.unit).max(1);
        let found = self.chunks.iter().enumerate().find_map(|(c, (_, free))| free.iter().position(|r| r.1 >= units).map(|i| (c, i)));
        let (c, i) = match found {
            Some(f) => f,
            None => {
                let cap = (CHUNK / self.unit).max(units);
                let buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("geometry"), size: cap * self.unit, usage: self.usage, mapped_at_creation: false });
                self.chunks.push((buf, vec![(0, cap)]));
                (self.chunks.len() - 1, 0)
            }
        };
        let free = &mut self.chunks[c].1;
        let at = free[i].0;
        if free[i].1 == units {
            free.remove(i);
        } else {
            free[i] = (at + units, free[i].1 - units);
        }
        queue.write_buffer(&self.chunks[c].0, at * self.unit, data);
        (c as u32, at)
    }

    fn give_back(&mut self, chunk: u32, at: u64, units: u64) {
        let free = &mut self.chunks[chunk as usize].1;
        let i = free.partition_point(|r| r.0 < at);
        free.insert(i, (at, units));
        // Merge with the neighbours.
        if i + 1 < free.len() && free[i].0 + free[i].1 == free[i + 1].0 {
            free[i].1 += free[i + 1].1;
            free.remove(i + 1);
        }
        if i > 0 && free[i - 1].0 + free[i - 1].1 == free[i].0 {
            free[i - 1].1 += free[i].1;
            free.remove(i);
        }
    }
}

/// Which pass a mesh draws in, from its shader's render bucket: solid
/// surfaces first, then cut-out ones, water, and decals and see-through
/// surfaces blended over them. Only the cut-out bucket's alpha is
/// coverage: opaque shaders keep other data there (a roof's texture has
/// holes over 40% of it), and cutting by it opened roofs to the floors
/// below.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    Solid,
    Cutout,
    Water,
    Decal,
    Alpha,
}

struct GpuMesh {
    pass: Pass,
    /// Vertex chunk and first vertex; index chunk (in the arena of its
    /// format), first index and count.
    vchunk: u32,
    base: u32,
    vunits: u64,
    ichunk: u32,
    first: u32,
    iunits: u64,
    format: wgpu::IndexFormat,
    count: u32,
    tex: usize,
    bytes: u64,
}

/// Things smaller than this on screen (pixels across) are not drawn.
const MIN_PIXELS: f32 = 2.0;

/// The bind groups over the instance buffer, remade when it grows.
struct Xforms {
    frame: wgpu::BindGroup,
    shadow: [wgpu::BindGroup; 2],
}

pub struct Camera {
    pub eye: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    /// Orthographic when above 0: half the view's height in metres.
    pub ortho: f32,
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(cy * cp, sy * cp, sp)
    }
    /// View and reversed projection (infinite when in perspective).
    pub fn matrix(&self, aspect: f32) -> Mat4 {
        let f = self.forward();
        let up = if f.z.abs() > 0.9999 { Vec3::Y } else { Vec3::Z };
        let proj = if self.ortho > 0.0 {
            let (h, w) = (self.ortho, self.ortho * aspect);
            // Depth 1 at 1 m, 0 at 20 km.
            let (n, fa) = (1.0, 20000.0);
            Mat4::from_cols(
                glam::Vec4::new(1.0 / w, 0.0, 0.0, 0.0),
                glam::Vec4::new(0.0, 1.0 / h, 0.0, 0.0),
                glam::Vec4::new(0.0, 0.0, 1.0 / (fa - n), 0.0),
                glam::Vec4::new(0.0, 0.0, fa / (fa - n), 1.0),
            )
        } else {
            reverse_perspective(self.fov.to_radians(), aspect, 0.5)
        };
        proj * look_to(self.eye, f, up)
    }
}

/// Right-handed view matrix looking along `dir`.
fn look_to(eye: Vec3, dir: Vec3, up: Vec3) -> Mat4 {
    let f = dir.normalize();
    let s = f.cross(up).normalize();
    let u = s.cross(f);
    Mat4::from_cols(glam::Vec4::new(s.x, u.x, -f.x, 0.0), glam::Vec4::new(s.y, u.y, -f.y, 0.0), glam::Vec4::new(s.z, u.z, -f.z, 0.0), glam::Vec4::new(-eye.dot(s), -eye.dot(u), eye.dot(f), 1.0))
}

/// Right-handed perspective with reversed depth and no far plane: depth 1
/// at `near`, falling towards 0 at infinity.
fn reverse_perspective(fov_y: f32, aspect: f32, near: f32) -> Mat4 {
    let h = 1.0 / (fov_y * 0.5).tan();
    Mat4::from_cols(glam::Vec4::new(h / aspect, 0.0, 0.0, 0.0), glam::Vec4::new(0.0, h, 0.0, 0.0), glam::Vec4::new(0.0, 0.0, 0.0, -1.0), glam::Vec4::new(0.0, 0.0, near, 0.0))
}

/// Right-handed orthographic projection with depth 0 at `near`, 1 at `far`.
fn orthographic(l: f32, r: f32, b: f32, t: f32, near: f32, far: f32) -> Mat4 {
    Mat4::from_cols(
        glam::Vec4::new(2.0 / (r - l), 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, 2.0 / (t - b), 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, -1.0 / (far - near), 0.0),
        glam::Vec4::new(-(r + l) / (r - l), -(t + b) / (t - b), -near / (far - near), 1.0),
    )
}

/// The sun's orthographic view of the slice `near..far` of the camera's
/// frustum: (matrix, texel size in metres, depth range in metres). The box
/// moves in whole texels so shadow edges do not crawl as the camera flies.
fn cascade(cam: &Camera, aspect: f32, near: f32, far: f32, sun: Vec3) -> (Mat4, f32, f32) {
    let f = cam.forward();
    let s = f.cross(Vec3::Z).normalize_or(Vec3::X);
    let u = s.cross(f);
    let th = (cam.fov.to_radians() * 0.5).tan();
    let tw = th * aspect;
    let mut corners = Vec::with_capacity(8);
    for d in [near, far] {
        for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            corners.push(cam.eye + f * d + s * (a * tw * d) + u * (b * th * d));
        }
    }
    let c = corners.iter().fold(Vec3::ZERO, |a, p| a + *p) / 8.0;
    let r = corners.iter().map(|p| p.distance(c)).fold(0.0f32, f32::max);
    let r = (r / 32.0).ceil() * 32.0;
    let up = if sun.z.abs() > 0.99 { Vec3::Y } else { Vec3::Z };
    let view = look_to(Vec3::ZERO, -sun, up);
    let lc = view.transform_point3(c);
    let texel = 2.0 * r / SHADOW as f32;
    let (cx, cy) = ((lc.x / texel).floor() * texel, (lc.y / texel).floor() * texel);
    // Towers up-sun of the box still cast into it.
    let reach = 900.0;
    let t = -lc.z;
    (orthographic(cx - r, cx + r, cy - r, cy + r, t - r - reach, t + r), texel, 2.0 * r + reach)
}

/// Direction towards the sun from VICE_LENS_SUN=azimuth,elevation (degrees,
/// azimuth counter-clockwise from east); a late-afternoon sun in the
/// west-north-west by default.
pub fn sun_from_env() -> Vec3 {
    let v: Vec<f32> = std::env::var("VICE_LENS_SUN").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    let (az, el) = if v.len() == 2 { (v[0], v[1]) } else { (150.0, 32.0) };
    let (az, el) = (az.to_radians(), el.to_radians());
    Vec3::new(az.cos() * el.cos(), az.sin() * el.cos(), el.sin())
}

pub struct View3d {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    water_pipeline: wgpu::RenderPipeline,
    decal_pipeline: wgpu::RenderPipeline,
    alpha_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    shadow_cutout_pipeline: wgpu::RenderPipeline,
    cutout_pipeline: wgpu::RenderPipeline,
    cam: wgpu::Buffer,
    lights: [wgpu::Buffer; 2],
    frame_layout: wgpu::BindGroupLayout,
    shadow_layout: wgpu::BindGroupLayout,
    tex_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    shadow_sampler: wgpu::Sampler,
    /// The shadow maps: one array view to sample, one view per cascade
    /// to draw into.
    shadow_view: wgpu::TextureView,
    shadow_layers: [wgpu::TextureView; 2],
    textures: Vec<Tex>,
    /// Shared geometry: vertices, 16-bit and 32-bit indices.
    vertices: Arena,
    indices16: Arena,
    indices32: Arena,
    /// Texture slots free for reuse, and keys of textures freed since the
    /// streamer last asked.
    free_tex: Vec<usize>,
    pub released: Vec<u64>,
    /// Counts instance rebuilds; textures remember the last one that
    /// placed something drawing with them.
    stamp: u64,
    /// Full-size levels freed for the budget so far.
    pub dropped: usize,
    by_key: std::collections::HashMap<u64, usize>,
    models: Vec<Vec<GpuMesh>>,
    /// Models drawn as water.
    water: Vec<bool>,
    /// (model, transform)
    instances: Vec<(usize, Mat4)>,
    /// The instances changed since the last frame.
    dirty: bool,
    /// Per model, its bounding sphere (centre, radius) in its own space;
    /// per instance, in the world.
    spheres: Vec<glam::Vec4>,
    inst_spheres: Vec<glam::Vec4>,
    /// Visible instances of each pass, written each frame.
    inst_buf: Option<wgpu::Buffer>,
    inst_cap: usize,
    stage: Vec<[[f32; 4]; 4]>,
    xforms: Option<Xforms>,
    frame_no: u64,
    /// The far cascade as last drawn: (matrix, texel, depth range).
    far_cascade: Option<(Mat4, f32, f32)>,
    /// Instances drawn last frame: in view, in the near and far cascades;
    /// and draw calls.
    pub drawn: [usize; 3],
    pub calls: std::cell::Cell<usize>,
    /// Meshes (model, mesh) waiting for a shared texture another model
    /// is bringing.
    pending: Vec<(usize, usize, u64)>,
    /// Bytes of full-size textures uploaded; past `FULL_BUDGET` only the
    /// small copies go up.
    full_bytes: u64,
    /// Bytes of small texture copies and of geometry uploaded.
    small_bytes: std::cell::Cell<u64>,
    mesh_bytes: u64,
    depth: Option<(wgpu::Texture, [u32; 2])>,
    /// Towards the sun.
    pub sun: Vec3,
    /// Seconds since the start, for the waves.
    pub time: f32,
    pub shadows: bool,
    /// Height haze (off for the map).
    pub haze: bool,
    /// How surfaces look (the map's styles): 0 their textures, 1 clay
    /// (one pale colour, foliage green), 2 relief (coloured by height,
    /// with contour lines every 10 m).
    pub look: u32,
    overlay: Option<Overlay>,
}

/// Seconds an instance takes to dissolve in or out.
pub const FADE_SECS: f32 = 0.5;

/// How an instance comes and goes: whole, or dissolving in or out since a
/// time (`View3d::time`).
#[derive(Clone, Copy)]
pub enum Fade {
    Whole,
    In(f32),
    Out(f32),
}

/// Video memory for full-size textures (the GPU has 8 GB; geometry and the
/// small copies need the rest).
const FULL_BUDGET: u64 = 5 << 28;

const SHADER: &str = r#"
struct Cam {
  view_proj: mat4x4<f32>,
  inv_view_proj: mat4x4<f32>,
  shadow: array<mat4x4<f32>, 2>,
  sun: vec4<f32>,
  eye: vec4<f32>,
  shadow_k: vec4<f32>,
  fade: vec4<f32>,
};
@group(0) @binding(0) var<uniform> cam: Cam;
@group(0) @binding(1) var<storage, read> xforms: array<mat4x4<f32>>;
@group(0) @binding(2) var shadow_map: texture_depth_2d_array;
@group(0) @binding(3) var shadow_samp: sampler_comparison;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;
@group(1) @binding(2) var small: texture_2d<f32>;

// Sunlight and the sky's light on an upward face (linear, before exposure).
const SUN: vec3<f32> = vec3<f32>(3.3, 2.95, 2.45);
const SKY_UP: vec3<f32> = vec3<f32>(0.40, 0.52, 0.72);
const BOUNCE: vec3<f32> = vec3<f32>(0.22, 0.20, 0.16);

fn sky(d: vec3<f32>, disk: bool) -> vec3<f32> {
  let s = normalize(cam.sun.xyz);
  let up = d.z;
  let h = 1.0 - clamp(up, 0.0, 1.0);
  let zenith = vec3<f32>(0.13, 0.30, 0.72);
  let horizon = vec3<f32>(0.70, 0.80, 0.90);
  var c = mix(zenith, horizon, h * h * h);
  // Below the horizon: the haze over the far sea.
  c = mix(c, horizon * 0.88, clamp(-up * 10.0, 0.0, 1.0));
  let mu = max(dot(d, s), 0.0);
  c += vec3<f32>(1.0, 0.80, 0.55) * (0.22 * pow(mu, 6.0) + 0.6 * pow(mu, 160.0));
  if (disk) {
    c += SUN * 30.0 * smoothstep(0.99990, 0.99996, mu);
  }
  return c;
}

// Exponential height haze between the eye and w, in the sky's colour.
fn haze(c: vec3<f32>, w: vec3<f32>) -> vec3<f32> {
  if (cam.fade.y < 0.5) { return c; }
  let e = cam.eye.xyz;
  let d = distance(w, e);
  let dir = (w - e) / max(d, 1e-3);
  let b = 0.0021;
  let k = 0.00020 * exp(-b * max(e.z, 0.0));
  var f = k * d;
  if (abs(dir.z) > 1e-4) {
    f = k * (1.0 - exp(-b * dir.z * d)) / (b * dir.z);
  }
  return mix(c, sky(dir, false), 1.0 - exp(-max(f, 0.0)));
}

fn tonemap(x: vec3<f32>) -> vec3<f32> {
  let e = x * 0.5;
  return clamp((e * (2.51 * e + 0.03)) / (e * (2.43 * e + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

// 1 in sunlight, 0 in shadow; 3x3 filtered taps of the nearest cascade.
fn sun_shadow(w: vec3<f32>, n: vec3<f32>) -> f32 {
  if (cam.eye.w < 0.5) { return 1.0; }
  let size = vec2<f32>(textureDimensions(shadow_map));
  for (var i = 0; i < 2; i++) {
    let texel = select(cam.shadow_k.w, cam.shadow_k.z, i == 0);
    let p = cam.shadow[i] * vec4<f32>(w + n * texel * 1.5, 1.0);
    let uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    if (all(uv > vec2<f32>(0.01)) && all(uv < vec2<f32>(0.99)) && p.z < 1.0 && p.z > 0.0) {
      let bias = select(cam.shadow_k.y, cam.shadow_k.x, i == 0);
      var s = 0.0;
      for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
          s += textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2<f32>(f32(x), f32(y)) / size, i, p.z - bias);
        }
      }
      // Fade out at the far edge of the last cascade.
      let edge = max(abs(uv.x - 0.5), abs(uv.y - 0.5)) * 2.0;
      let fade = select(0.0, smoothstep(0.85, 0.98, edge), i == 1);
      return mix(s / 9.0, 1.0, fade);
    }
  }
  return 1.0;
}

struct V {
  @builtin(position) pos: vec4<f32>,
  @location(0) n: vec3<f32>,
  @location(1) uv: vec2<f32>,
  @location(2) w: vec3<f32>,
  // How much of the instance is drawn, and its kind: 0 whole, 1 fading
  // in, 2 fading out.
  @location(3) @interpolate(flat) f: vec2<f32>,
};
@vertex fn vs(@location(0) p: vec3<f32>, @location(1) n4: vec4<f32>, @location(2) uv: vec2<f32>, @builtin(instance_index) i: u32) -> V {
  let n = n4.xyz;
  var m = xforms[i];
  let kind = m[0].w;
  let start = m[1].w;
  m[0].w = 0.0;
  m[1].w = 0.0;
  let w = m * vec4<f32>(p, 1.0);
  var o: V;
  o.pos = cam.view_proj * w;
  o.n = (m * vec4<f32>(n, 0.0)).xyz;
  o.uv = uv;
  o.w = w.xyz;
  let a = clamp((cam.sun.w - start) / cam.fade.x, 0.0, 1.0);
  o.f = vec2<f32>(select(1.0 - a, a, kind < 1.5), kind);
  return o;
}

// Screen-door dissolve: what comes and what goes at the same moment (a
// detailed model and its LOD stand-in) cover complementary pixels.
fn dissolve(v: V) {
  if (v.f.y > 0.5) {
    let b = fract(52.9829189 * fract(dot(floor(v.pos.xy), vec2<f32>(0.06711056, 0.00583715))));
    if (v.f.y < 1.5) {
      if (b >= v.f.x) { discard; }
    } else if (b < 1.0 - v.f.x) {
      discard;
    }
  }
}

// The albedo: the full texture has no mips; past 2:1 minification it
// blends into the small mipmapped copy.
fn albedo(v: V) -> vec4<f32> {
  let size = vec2<f32>(textureDimensions(tex));
  let dx = dpdx(v.uv * size);
  let dy = dpdy(v.uv * size);
  let lod = 0.5 * log2(max(max(dot(dx, dx), dot(dy, dy)), 1e-8));
  let full = textureSampleLevel(tex, samp, v.uv, 0.0);
  let ssize = vec2<f32>(textureDimensions(small));
  let far = textureSampleLevel(small, samp, v.uv, max(lod - log2(size.x / ssize.x), 0.0));
  return mix(full, far, clamp(lod - 0.5, 0.0, 1.0));
}

// Colour by height for the relief look: shore greens, lowland tan,
// hill browns, high grey (the island rises to some 300 m).
fn hypso(z: f32) -> vec3<f32> {
  let c0 = vec3<f32>(0.56, 0.66, 0.50);
  let c1 = vec3<f32>(0.70, 0.76, 0.55);
  let c2 = vec3<f32>(0.84, 0.80, 0.60);
  let c3 = vec3<f32>(0.80, 0.66, 0.48);
  let c4 = vec3<f32>(0.64, 0.53, 0.44);
  let c5 = vec3<f32>(0.90, 0.88, 0.86);
  var c = mix(c0, c1, smoothstep(0.0, 8.0, z));
  c = mix(c, c2, smoothstep(8.0, 35.0, z));
  c = mix(c, c3, smoothstep(35.0, 110.0, z));
  c = mix(c, c4, smoothstep(110.0, 220.0, z));
  return mix(c, c5, smoothstep(220.0, 330.0, z));
}

// The albedo a look gives a surface (`foliage`: a cut-out one).
fn looks(v: V, c: vec3<f32>, foliage: bool) -> vec3<f32> {
  let k = cam.fade.z;
  if (k < 0.5) { return c; }
  // Albedos as dark as the textures' (the sun is bright).
  if (k < 1.5) { return select(vec3<f32>(0.36, 0.355, 0.34), vec3<f32>(0.17, 0.22, 0.13), foliage); }
  var col = hypso(v.w.z) * 0.42;
  if (foliage) { col *= vec3<f32>(0.72, 0.84, 0.66); }
  // Contour lines every 10 m, a pixel wide whatever the slope.
  let h = v.w.z / 10.0;
  let d = abs(fract(h + 0.5) - 0.5) / max(fwidth(h), 1e-4);
  return col * (1.0 - 0.22 * (1.0 - smoothstep(0.5, 1.5, d)));
}

// Solid surfaces: the texture's alpha is not coverage.
@fragment fn fs(v: V) -> @location(0) vec4<f32> {
  let c = albedo(v);
  dissolve(v);
  return vec4<f32>(shade(v, looks(v, c.rgb, false)), 1.0);
}

// Cut-out surfaces (leaves, fences), cut where the texture's alpha is low.
@fragment fn fs_cutout(v: V) -> @location(0) vec4<f32> {
  let c = albedo(v);
  if (c.a < 0.3) { discard; }
  dissolve(v);
  return vec4<f32>(shade(v, looks(v, c.rgb, true)), 1.0);
}

// Decals and see-through surfaces, blended by the texture's alpha; the
// plain looks leave them out.
@fragment fn fs_blend(v: V) -> @location(0) vec4<f32> {
  let c = albedo(v);
  if (c.a < 0.01 || cam.fade.z > 0.5) { discard; }
  dissolve(v);
  return vec4<f32>(shade(v, c.rgb), c.a);
}

// Sun, sky and haze on an albedo.
fn shade(v: V, albedo: vec3<f32>) -> vec3<f32> {
  // The face normal, towards the eye, decides which side is seen; it
  // stands in for vertex normals that disagree with the face.
  let to_eye = cam.eye.xyz - v.w;
  var f = normalize(cross(dpdx(v.w), dpdy(v.w)));
  if (dot(f, to_eye) < 0.0) { f = -f; }
  var n = normalize(v.n);
  if (dot(n, f) < 0.0) { n = -n; }
  if (dot(n, f) < 0.25) { n = f; }
  let s = normalize(cam.sun.xyz);
  let ndl = max(dot(n, s), 0.0);
  var sh = 1.0;
  if (ndl > 0.0) { sh = sun_shadow(v.w, n); }
  let amb = mix(BOUNCE, SKY_UP, n.z * 0.5 + 0.5);
  let lit = albedo * (amb + SUN * ndl * sh);
  return tonemap(haze(lit, v.w));
}

fn wave(p: vec2<f32>, dir: vec2<f32>, k: f32, a: f32, t: f32, fade: f32) -> vec2<f32> {
  let kv = normalize(dir) * k;
  let ph = dot(kv, p) - sqrt(9.81 * k) * t;
  return kv * (a * cos(ph) * fade);
}

@fragment fn fs_water(v: V) -> @location(0) vec4<f32> {
  // The plain looks draw water flat.
  if (cam.fade.z > 1.5) { return vec4<f32>(0.53, 0.70, 0.80, 1.0); }
  if (cam.fade.z > 0.5) { return vec4<f32>(0.66, 0.74, 0.78, 1.0); }
  let e = cam.eye.xyz;
  let d = distance(v.w, e);
  let view = (v.w - e) / max(d, 1e-3);
  let t = cam.sun.w;
  let p = v.w.xy;
  // Swell to ripples from many directions (golden-angle spread), equal
  // slopes; the short ones fade with distance before they alias.
  var g = vec2<f32>(0.0);
  var k = 0.045;
  for (var j = 0; j < 9; j++) {
    let a = 0.4 + f32(j) * 2.39996;
    let dir = vec2<f32>(cos(a), sin(a) * 0.8 + 0.3);
    g += wave(p, dir, k, 0.03 / k, t, exp(-d * k * 0.0035));
    k *= 1.62;
  }
  let n = normalize(vec3<f32>(-g, 1.0));
  let s = normalize(cam.sun.xyz);
  let sh = sun_shadow(v.w, vec3<f32>(0.0, 0.0, 1.0));
  var r = reflect(view, n);
  r.z = abs(r.z);
  let cosv = clamp(dot(-view, n), 0.0, 1.0);
  let fres = 0.02 + 0.98 * pow(1.0 - cosv, 5.0);
  let refl = sky(r, false) + SUN * 45.0 * pow(max(dot(r, s), 0.0), 900.0) * sh;
  let body = vec3<f32>(0.03, 0.12, 0.13) * (SKY_UP + SUN * max(s.z, 0.0) * sh * 0.45);
  return vec4<f32>(tonemap(haze(mix(body, refl, fres), v.w)), 1.0);
}

struct SkyV { @builtin(position) pos: vec4<f32>, @location(0) ndc: vec2<f32> };
@vertex fn vs_sky(@builtin(vertex_index) i: u32) -> SkyV {
  let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
  var o: SkyV;
  o.pos = vec4<f32>(p, 0.0, 1.0);
  o.ndc = p;
  return o;
}
@fragment fn fs_sky(v: SkyV) -> @location(0) vec4<f32> {
  let a = cam.inv_view_proj * vec4<f32>(v.ndc, 0.5, 1.0);
  let d = normalize(a.xyz / a.w - cam.eye.xyz);
  return vec4<f32>(tonemap(sky(d, true)), 1.0);
}
"#;

/// Depth from the sun; cut-out leaves and fences cast cut-out shadows.
const SHADOW_SHADER: &str = r#"
struct Light { m: mat4x4<f32>, fade: vec4<f32> };
@group(0) @binding(0) var<uniform> light: Light;
@group(0) @binding(1) var<storage, read> xforms: array<mat4x4<f32>>;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;
@group(1) @binding(2) var small: texture_2d<f32>;
struct V { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@location(0) p: vec3<f32>, @location(2) uv: vec2<f32>, @builtin(instance_index) i: u32) -> V {
  var o: V;
  var m = xforms[i];
  let kind = m[0].w;
  let a = clamp((light.fade.y - m[1].w) / light.fade.x, 0.0, 1.0);
  m[0].w = 0.0;
  m[1].w = 0.0;
  o.pos = light.m * (m * vec4<f32>(p, 1.0));
  // Instances less than half there cast nothing.
  if ((kind > 0.5 && kind < 1.5 && a < 0.5) || (kind > 1.5 && a > 0.5)) {
    o.pos = vec4<f32>(0.0, 0.0, -1.0, 1.0);
  }
  o.uv = uv;
  return o;
}
@fragment fn fs_cutout(v: V) {
  if (textureSampleLevel(small, samp, v.uv, 2.0).a < 0.3) { discard; }
}
"#;

fn layout_entry(binding: u32, visibility: wgpu::ShaderStages, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None }
}

impl View3d {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, color: wgpu::TextureFormat) -> View3d {
        use wgpu::ShaderStages as St;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("world"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let shadow_module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("shadow"), source: wgpu::ShaderSource::Wgsl(SHADOW_SHADER.into()) });
        let uniform = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
        let storage = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None };
        let color_tex = wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false };
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                layout_entry(0, St::VERTEX_FRAGMENT, uniform),
                layout_entry(1, St::VERTEX, storage),
                layout_entry(2, St::FRAGMENT, wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false }),
                layout_entry(3, St::FRAGMENT, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison)),
            ],
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[layout_entry(0, St::VERTEX, uniform), layout_entry(1, St::VERTEX, storage)] });
        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                layout_entry(0, St::FRAGMENT, color_tex),
                layout_entry(1, St::FRAGMENT, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
                layout_entry(2, St::FRAGMENT, color_tex),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&frame_layout), Some(&tex_layout)], immediate_size: 0 });
        let sky_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&frame_layout)], immediate_size: 0 });
        let shadow_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&shadow_layout), Some(&tex_layout)], immediate_size: 0 });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Snorm8x4, 2 => Float32x2];
        let buffers = [Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs })];
        let depth = |write: bool, compare: wgpu::CompareFunction, bias: wgpu::DepthBiasState| {
            Some(wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: Some(write), depth_compare: Some(compare), stencil: Default::default(), bias })
        };
        let targets = [Some(wgpu::ColorTargetState { format: color, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        let blended = [Some(wgpu::ColorTargetState { format: color, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::COLOR })];
        // Blended surfaces test depth but leave it; decals are pulled
        // towards the eye (depth is reversed: larger is nearer) so they
        // win over the surface they lie on.
        let surface = |label: &str, fs: &str, blend: bool, bias: wgpu::DepthBiasState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &buffers },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: depth(!blend, wgpu::CompareFunction::GreaterEqual, bias),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(fs), compilation_options: Default::default(), targets: if blend { &blended } else { &targets } }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline = surface("world", "fs", false, Default::default());
        let cutout_pipeline = surface("cut-out", "fs_cutout", false, Default::default());
        let water_pipeline = surface("water", "fs_water", false, Default::default());
        let decal_pipeline = surface("decals", "fs_blend", true, wgpu::DepthBiasState { constant: 8, slope_scale: 1.0, clamp: 0.0 });
        let alpha_pipeline = surface("see-through", "fs_blend", true, Default::default());
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&sky_pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_sky"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: depth(false, wgpu::CompareFunction::Always, Default::default()),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_sky"), compilation_options: Default::default(), targets: &targets }),
            multiview_mask: None,
            cache: None,
        });
        // Shadows: solid casters write depth only; cut-out ones are cut.
        let shadow = |label: &str, cut: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&shadow_pl),
                vertex: wgpu::VertexState { module: &shadow_module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &buffers },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: depth(true, wgpu::CompareFunction::LessEqual, wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 }),
                multisample: wgpu::MultisampleState::default(),
                fragment: cut.then(|| wgpu::FragmentState { module: &shadow_module, entry_point: Some("fs_cutout"), compilation_options: Default::default(), targets: &[] }),
                multiview_mask: None,
                cache: None,
            })
        };
        let shadow_pipeline = shadow("shadow", false);
        let shadow_cutout_pipeline = shadow("shadow, cut-out", true);
        let cam = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cam"),
            size: std::mem::size_of::<CamU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light = || device.create_buffer(&wgpu::BufferDescriptor { label: Some("light"), size: 80, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let lights = [light(), light()];
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow"),
            size: wgpu::Extent3d { width: SHADOW, height: SHADOW, depth_or_array_layers: 2 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        let layer =
            |i: u32| shadow_tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2), base_array_layer: i, array_layer_count: Some(1), ..Default::default() });
        let shadow_layers = [layer(0), layer(1)];
        let mut v = View3d {
            device,
            queue,
            pipeline,
            water_pipeline,
            decal_pipeline,
            alpha_pipeline,
            sky_pipeline,
            shadow_pipeline,
            shadow_cutout_pipeline,
            cutout_pipeline,
            cam,
            lights,
            frame_layout,
            shadow_layout,
            tex_layout,
            sampler,
            shadow_sampler,
            shadow_view,
            shadow_layers,
            textures: Vec::new(),
            vertices: Arena::new(wgpu::BufferUsages::VERTEX, std::mem::size_of::<Vertex>() as u64),
            indices16: Arena::new(wgpu::BufferUsages::INDEX, 4),
            indices32: Arena::new(wgpu::BufferUsages::INDEX, 4),
            stamp: 0,
            dropped: 0,
            free_tex: Vec::new(),
            released: Vec::new(),
            by_key: Default::default(),
            models: Vec::new(),
            water: Vec::new(),
            instances: Vec::new(),
            dirty: true,
            spheres: Vec::new(),
            inst_spheres: Vec::new(),
            inst_buf: None,
            inst_cap: 0,
            stage: Vec::new(),
            xforms: None,
            frame_no: 0,
            far_cascade: None,
            drawn: [0; 3],
            calls: Default::default(),
            pending: Vec::new(),
            full_bytes: 0,
            small_bytes: Default::default(),
            mesh_bytes: 0,
            depth: None,
            sun: sun_from_env(),
            time: 0.0,
            shadows: std::env::var_os("VICE_LENS_NO_SHADOWS").is_none(),
            haze: true,
            look: 0,
            overlay: None,
        };
        // Texture 0: plain grey for surfaces without a diffuse map.
        v.plain_tex(&world::mip_chain(4, 4, vec![150u8; 64]), false);
        v
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn present(&self, frame: wgpu::SurfaceTexture) {
        self.queue.present(frame);
    }

    fn upload_small(&self, small: &world::Small) -> wgpu::TextureView {
        let (sw, sh, sdata) = small;
        self.small_bytes.set(self.small_bytes.get() + sdata.len() as u64);
        let levels = 32 - sw.max(sh).leading_zeros();
        let stex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: *sw, height: *sh, depth_or_array_layers: 1 },
                mip_level_count: levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            sdata,
        );
        stex.create_view(&Default::default())
    }

    fn group(&self, full: &wgpu::TextureView, small: &wgpu::TextureView) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.tex_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(full) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(small) },
            ],
        })
    }

    /// A texture from its small copy alone (plain colours, LOD stand-ins).
    fn plain_tex(&mut self, small: &world::Small, upgradable: bool) -> usize {
        let view = self.upload_small(small);
        let group = self.group(&view, &view);
        let t = Tex { group, small: view, full: None, upgradable, last: self.stamp, users: 0, small_bytes: small.2.len() as u64, key: None };
        match self.free_tex.pop() {
            Some(i) => {
                self.textures[i] = t;
                i
            }
            None => {
                self.textures.push(t);
                self.textures.len() - 1
            }
        }
    }

    /// A mesh stops drawing with texture `i`; the last one frees it, and
    /// its key goes to `released` (so a later load decodes it again).
    fn unuse(&mut self, i: usize) {
        if i == 0 {
            return;
        }
        let t = &mut self.textures[i];
        t.users = t.users.saturating_sub(1);
        if t.users > 0 {
            return;
        }
        if let Some((_, b)) = t.full.take() {
            self.full_bytes -= b;
        }
        self.small_bytes.set(self.small_bytes.get().saturating_sub(t.small_bytes));
        t.small_bytes = 0;
        if let Some(k) = t.key.take() {
            if self.by_key.get(&k) == Some(&i) {
                self.by_key.remove(&k);
            }
            self.released.push(k);
        }
        let (group, small) = (self.textures[0].group.clone(), self.textures[0].small.clone());
        let t = &mut self.textures[i];
        (t.group, t.small, t.upgradable) = (group, small, false);
        self.free_tex.push(i);
    }

    /// Upload the full-size level of texture `i` when its format allows and
    /// the budget has room (after freeing what has been out of view
    /// longest); a format that cannot go up stays small for good.
    fn sharpen(&mut self, i: usize, pixels: &world::Pixels) {
        let &(dxgi, w, h, ref data) = pixels;
        let fit = world::gpu_format(dxgi).zip(world::row_bytes(dxgi, w)).filter(|(_, (row, bh))| {
            let ok_dims = *bh == 1 || (w % 4 == 0 && h % 4 == 0);
            ok_dims && data.len() as u32 >= row * h.div_ceil(*bh)
        });
        let Some((format, (row, bh))) = fit else {
            self.textures[i].upgradable = false;
            return;
        };
        let rows = h.div_ceil(bh);
        let bytes = (row * rows) as u64;
        if !self.make_room(bytes) {
            return;
        }
        if w.max(h) > 4096 {
            log::warn!("texture {w}x{h} {format:?}");
        }
        let tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &data[..bytes as usize],
        );
        let group = self.group(&tex.create_view(&Default::default()), &self.textures[i].small);
        self.full_bytes += bytes;
        let t = &mut self.textures[i];
        t.group = group;
        t.full = Some((tex, bytes));
    }

    /// Free full-size levels not drawn since the last instance rebuild,
    /// longest unseen first, until `bytes` more fit the budget.
    fn make_room(&mut self, bytes: u64) -> bool {
        if self.full_bytes + bytes <= FULL_BUDGET {
            return true;
        }
        let mut old: Vec<(u64, usize)> = self.textures.iter().enumerate().filter(|(_, t)| t.full.is_some() && t.last < self.stamp).map(|(i, t)| (t.last, i)).collect();
        old.sort_unstable();
        for (_, i) in old {
            if self.full_bytes + bytes <= FULL_BUDGET * 9 / 10 {
                break;
            }
            let group = self.group(&self.textures[i].small, &self.textures[i].small);
            let t = &mut self.textures[i];
            if let Some((_, b)) = t.full.take() {
                self.full_bytes -= b;
            }
            t.group = group;
            self.dropped += 1;
        }
        self.full_bytes + bytes <= FULL_BUDGET
    }

    /// Whether model `id` draws with textures that could be sharper.
    pub fn blurry(&self, id: usize) -> bool {
        self.models[id].iter().any(|m| {
            let t = &self.textures[m.tex];
            t.upgradable && t.full.is_none()
        })
    }

    /// Give model `id` the full-size textures a fresh load of it decoded.
    pub fn sharpen_model(&mut self, id: usize, m: &Model) {
        for k in 0..self.models[id].len().min(m.drawable.meshes.len()) {
            let tex = self.models[id][k].tex;
            let Some(p) = m.textures.get(m.drawable.meshes[k].shader).and_then(|t| t.pixels.as_ref()) else { continue };
            if self.textures[tex].upgradable && self.textures[tex].full.is_none() {
                self.sharpen(tex, p);
            }
        }
    }

    /// Upload plain-coloured triangles (sRGB colour) as a model.
    pub fn add_colored(&mut self, parts: &[(Vec<[f32; 3]>, Vec<u32>, [u8; 4])]) -> usize {
        let mut meshes = Vec::new();
        for (pos, idx, rgba) in parts {
            if idx.is_empty() {
                continue;
            }
            let px: Vec<u8> = (0..16).flat_map(|_| *rgba).collect();
            let tex = self.plain_tex(&world::mip_chain(4, 4, px), false);
            let normals = normals(pos, idx);
            let verts: Vec<Vertex> = pos.iter().zip(&normals).map(|(p, n)| Vertex { pos: *p, normal: pack_normal(*n), uv: [0.5, 0.5] }).collect();
            meshes.push(self.upload_mesh(&verts, idx, tex));
        }
        self.models.push(meshes);
        self.water.push(false);
        self.spheres.push(glam::Vec4::new(0.0, 0.0, 0.0, f32::INFINITY));
        self.models.len() - 1
    }

    /// The sea at height `z` around (x, y), out to `e` metres, in tiles so
    /// no triangle is kilometres across.
    pub fn add_sea(&mut self, x: f32, y: f32, z: f32, e: f32) -> usize {
        const N: usize = 48;
        let mut verts = Vec::new();
        for j in 0..=N {
            for i in 0..=N {
                let (u, v) = (i as f32 / N as f32 * 2.0 - 1.0, j as f32 / N as f32 * 2.0 - 1.0);
                verts.push(Vertex { pos: [x + u * e, y + v * e, z], normal: [0, 0, 127, 0], uv: [0.5, 0.5] });
            }
        }
        let mut idx = Vec::new();
        for j in 0..N as u32 {
            for i in 0..N as u32 {
                let a = j * (N as u32 + 1) + i;
                let b = a + N as u32 + 1;
                idx.extend_from_slice(&[a, a + 1, b + 1, a, b + 1, b]);
            }
        }
        let mut mesh = self.upload_mesh(&verts, &idx, 0);
        mesh.pass = Pass::Water;
        self.models.push(vec![mesh]);
        self.water.push(true);
        self.spheres.push(glam::Vec4::new(0.0, 0.0, 0.0, f32::INFINITY));
        self.models.len() - 1
    }

    /// Upload a model (drawn as water when `water`); returns its index
    /// for instances.
    pub fn add_model(&mut self, m: &Model, water: bool) -> usize {
        let model = self.models.len();
        let mut tex_of_shader = Vec::new();
        for t in &m.textures {
            if let Some(&i) = t.key.and_then(|k| self.by_key.get(&k)) {
                // Already up; this load may bring its full-size level.
                if let (Some(p), None) = (&t.pixels, &self.textures[i].full)
                    && self.textures[i].upgradable
                {
                    self.sharpen(i, p);
                }
                tex_of_shader.push((i, None));
                continue;
            }
            if t.pixels.is_none() && t.small.is_none() {
                // A shared texture another load is decoding: bind it later.
                tex_of_shader.push((0, t.key));
                continue;
            }
            let grey;
            let small = match &t.small {
                Some(s) => s,
                None => {
                    grey = world::mip_chain(1, 1, vec![150, 150, 150, 255]);
                    &grey
                }
            };
            let i = self.plain_tex(small, true);
            if let Some(p) = &t.pixels {
                self.sharpen(i, p);
            }
            if let Some(k) = t.key {
                self.by_key.insert(k, i);
                self.textures[i].key = Some(k);
            }
            tex_of_shader.push((i, None));
        }
        let mut meshes = Vec::new();
        for (mesh, verts) in m.drawable.meshes.iter().zip(&m.vertices) {
            let (tex, later) = tex_of_shader.get(mesh.shader).copied().unwrap_or((0, None));
            if let Some(k) = later {
                self.pending.push((model, meshes.len(), k));
            }
            let mut g = self.upload_mesh(verts, &mesh.indices, tex);
            g.pass = match m.drawable.buckets.get(mesh.shader).copied().unwrap_or_default() {
                _ if water => Pass::Water,
                Bucket::Water => Pass::Water,
                Bucket::Decal => Pass::Decal,
                Bucket::Alpha => Pass::Alpha,
                Bucket::Cutout => Pass::Cutout,
                Bucket::Opaque => Pass::Solid,
            };
            self.mesh_bytes += g.bytes;
            if tex != 0 {
                self.textures[tex].users += 1;
            }
            meshes.push(g);
        }
        self.models.push(meshes);
        self.water.push(water);
        self.spheres.push(glam::Vec4::from(m.sphere));
        model
    }

    /// Place an instance of a model.
    pub fn add_instance(&mut self, model: usize, at: Mat4, fade: Fade) {
        let (models, textures) = (&self.models, &mut self.textures);
        for m in &models[model] {
            textures[m.tex].last = self.stamp;
        }
        let mut at = at;
        (at.x_axis.w, at.y_axis.w) = match fade {
            Fade::Whole => (0.0, 0.0),
            Fade::In(t) => (1.0, t),
            Fade::Out(t) => (2.0, t),
        };
        self.instances.push((model, at));
        self.dirty = true;
    }

    /// Free a model's geometry (its instances must be gone); its textures
    /// stay, shared by key.
    pub fn free_model(&mut self, id: usize) {
        for m in std::mem::take(&mut self.models[id]) {
            self.mesh_bytes -= m.bytes;
            self.unuse(m.tex);
            self.vertices.give_back(m.vchunk, m.base as u64, m.vunits);
            let at = m.first as u64 / self.index_per_unit(m.format);
            self.index_arena(m.format).give_back(m.ichunk, at, m.iunits);
        }
    }

    fn index_arena(&mut self, f: wgpu::IndexFormat) -> &mut Arena {
        if f == wgpu::IndexFormat::Uint16 { &mut self.indices16 } else { &mut self.indices32 }
    }

    /// Indices per arena unit: 16-bit ones go in pairs.
    fn index_per_unit(&self, f: wgpu::IndexFormat) -> u64 {
        if f == wgpu::IndexFormat::Uint16 { 2 } else { 1 }
    }

    /// Put a mesh's vertices and indices in the shared geometry buffers.
    fn upload_mesh(&mut self, verts: &[Vertex], idx: &[u32], tex: usize) -> GpuMesh {
        let (ibytes, format) = index_bytes(idx);
        let (vchunk, base) = self.vertices.put(&self.device, &self.queue, bytemuck::cast_slice(verts));
        let per = self.index_per_unit(format);
        let (device, queue) = (self.device.clone(), self.queue.clone());
        let arena = self.index_arena(format);
        let (ichunk, at) = arena.put(&device, &queue, &ibytes);
        let iunits = (ibytes.len() as u64).div_ceil(arena.unit).max(1);
        GpuMesh {
            pass: Pass::Solid,
            vchunk,
            base: base as u32,
            vunits: verts.len().max(1) as u64,
            ichunk,
            first: (at * per) as u32,
            iunits,
            format,
            count: idx.len() as u32,
            tex,
            bytes: (std::mem::size_of_val(verts) + ibytes.len()) as u64,
        }
    }

    /// Submit pending uploads so their staging memory is released.
    pub fn flush(&self) {
        self.queue.submit(std::iter::empty());
        let _ = self.device.poll(wgpu::PollType::Poll);
    }

    /// Remove every instance (the models stay uploaded).
    pub fn clear_instances(&mut self) {
        self.stamp += 1;
        self.instances.clear();
        self.dirty = true;
    }

    /// Video memory in use, MiB: full textures, small copies, geometry.
    /// Freed geometry leaves room in the shared buffers that new models
    /// take, so the buffers stay near the most ever in use at once.
    pub fn memory(&self) -> [u64; 3] {
        [self.full_bytes >> 20, self.small_bytes.get() >> 20, self.mesh_bytes >> 20]
    }

    /// The shared geometry buffers' whole size, MiB.
    pub fn geometry_reserved(&self) -> u64 {
        [&self.vertices, &self.indices16, &self.indices32].iter().map(|a| a.chunks.iter().map(|c| c.0.size()).sum::<u64>()).sum::<u64>() >> 20
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    pub fn draw(&mut self, cam: &Camera, target: &wgpu::TextureView, size: [u32; 2]) {
        self.render(cam, target, size, false);
    }

    /// Draw a frame. `exact` refreshes every shadow cascade (captures);
    /// otherwise the far cascade is redrawn every third frame.
    fn render(&mut self, cam: &Camera, target: &wgpu::TextureView, size: [u32; 2], exact: bool) {
        if self.depth.as_ref().is_none_or(|(_, s)| *s != size) {
            let t = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.depth = Some((t, size));
        }
        let depth = self.depth.as_ref().unwrap().0.create_view(&Default::default());
        let aspect = size[0] as f32 / size[1] as f32;
        let view_proj = cam.matrix(aspect);
        let sun = self.sun.normalize();
        self.frame_no += 1;
        let c0 = cascade(cam, aspect, CASCADES[0], CASCADES[1], sun);
        let refresh_far = exact || self.far_cascade.is_none() || self.frame_no.is_multiple_of(3);
        if refresh_far {
            self.far_cascade = Some(cascade(cam, aspect, CASCADES[1], CASCADES[2], sun));
        }
        let c1 = self.far_cascade.unwrap();
        let ((m0, t0, r0), (m1, t1, r1)) = (c0, c1);
        let u = CamU {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            shadow: [m0.to_cols_array_2d(), m1.to_cols_array_2d()],
            sun: [sun.x, sun.y, sun.z, self.time],
            eye: [cam.eye.x, cam.eye.y, cam.eye.z, if self.shadows { 1.0 } else { 0.0 }],
            shadow_k: [t0 * 0.6 / r0, t1 * 0.6 / r1, t0, t1],
            fade: [FADE_SECS, if self.haze { 1.0 } else { 0.0 }, self.look as f32, 0.0],
        };
        self.queue.write_buffer(&self.cam, 0, bytemuck::bytes_of(&u));
        for (i, m) in [m0, m1].iter().enumerate() {
            if i == 1 && !refresh_far {
                continue;
            }
            let mut light = [0f32; 20];
            light[..16].copy_from_slice(&m.to_cols_array());
            light[16..18].copy_from_slice(&[FADE_SECS, self.time]);
            self.queue.write_buffer(&self.lights[i], 0, bytemuck::cast_slice(&light));
        }
        // Shared textures that arrived since.
        if !self.pending.is_empty() {
            let (by_key, models, textures) = (&self.by_key, &mut self.models, &mut self.textures);
            self.pending.retain(|&(model, mesh, key)| match by_key.get(&key) {
                Some(&t) => {
                    if let Some(m) = models[model].get_mut(mesh) {
                        m.tex = t;
                        textures[t].users += 1;
                    }
                    false
                }
                None => true,
            });
        }
        // Instances changed: group them by model (water last) and find
        // their bounding spheres in the world.
        if self.dirty {
            self.dirty = false;
            let water = &self.water;
            self.instances.sort_by_key(|(m, _)| (water[*m], *m));
            let spheres = &self.spheres;
            self.inst_spheres = self
                .instances
                .iter()
                .map(|(m, at)| {
                    let s = spheres[*m];
                    let mut at = *at;
                    (at.x_axis.w, at.y_axis.w) = (0.0, 0.0);
                    let c = at.transform_point3(Vec3::new(s[0], s[1], s[2]));
                    let k = at.x_axis.truncate().length().max(at.y_axis.truncate().length()).max(at.z_axis.truncate().length());
                    glam::Vec4::new(c.x, c.y, c.z, s[3] * k)
                })
                .collect();
        }
        // What each pass draws this frame: the camera's frustum without
        // what would be under MIN_PIXELS across, and each cascade's box
        // without casters under a texel.
        let rows = [view_proj.row(0), view_proj.row(1), view_proj.row(2), view_proj.row(3)];
        let planes = [rows[3] + rows[0], rows[3] - rows[0], rows[3] + rows[1], rows[3] - rows[1], rows[3] - rows[2]].map(|p| p / p.truncate().length());
        let px = size[1] as f32 / (2.0 * (cam.fov.to_radians() * 0.5).tan());
        let (eye, ortho) = (cam.eye, cam.ortho);
        let (spheres, water) = (&self.inst_spheres, &self.water);
        let seen = |i: usize| {
            let s = spheres[i];
            if s.w.is_infinite() {
                return true;
            }
            let c = s.truncate().extend(1.0);
            if planes.iter().any(|p| p.dot(c) < -s.w) {
                return false;
            }
            if ortho > 0.0 {
                return s.w * size[1] as f32 * 0.5 / ortho >= MIN_PIXELS * 0.5;
            }
            let d = eye.distance(s.truncate());
            d <= s.w || s.w * px / d >= MIN_PIXELS
        };
        // A caster's shadow is about its size: one that would be under a
        // few pixels on screen, or a couple of texels in the map, is left
        // out.
        let casts = |i: usize, m: &Mat4, texel: f32, range: f32| {
            let s = spheres[i];
            if s.w.is_infinite() || s.w < texel * 2.0 {
                return false;
            }
            let d = eye.distance(s.truncate());
            if d > s.w && s.w * px / d < MIN_PIXELS * 3.0 {
                return false;
            }
            let p = m.transform_point3(s.truncate());
            let (rxy, rz) = (s.w / (texel * SHADOW as f32 * 0.5), s.w / range);
            p.x.abs() <= 1.0 + rxy && p.y.abs() <= 1.0 + rxy && p.z >= -rz && p.z <= 1.0 + rz
        };
        let mut stage = std::mem::take(&mut self.stage);
        stage.clear();
        let instances = &self.instances;
        let collect = |stage: &mut Vec<[[f32; 4]; 4]>, keep: &dyn Fn(usize) -> bool| {
            let mut runs: Vec<(usize, u32, u32)> = Vec::new();
            for (i, (m, at)) in instances.iter().enumerate() {
                if !keep(i) {
                    continue;
                }
                let k = stage.len() as u32;
                stage.push(at.to_cols_array_2d());
                match runs.last_mut() {
                    Some(r) if r.0 == *m && r.2 == k => r.2 = k + 1,
                    _ => runs.push((*m, k, k + 1)),
                }
            }
            runs
        };
        let main = collect(&mut stage, &seen);
        let (near, far) = if self.shadows {
            let near = collect(&mut stage, &|i| !water[instances[i].0] && casts(i, &m0, t0, r0));
            let far = if refresh_far { collect(&mut stage, &|i| !water[instances[i].0] && casts(i, &m1, t1, r1)) } else { Vec::new() };
            (near, far)
        } else {
            (Vec::new(), Vec::new())
        };
        let count = |runs: &[(usize, u32, u32)]| runs.iter().map(|r| (r.2 - r.1) as usize).sum::<usize>();
        self.drawn = [count(&main), count(&near), count(&far)];
        // One instance buffer, grown when it is too small.
        if self.inst_cap < stage.len().max(1) {
            self.inst_cap = (stage.len() * 3 / 2).max(1 << 14);
            self.inst_buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instances"),
                size: (self.inst_cap * 64) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.xforms = None;
        }
        let buf = self.inst_buf.as_ref().unwrap();
        if !stage.is_empty() {
            self.queue.write_buffer(buf, 0, bytemuck::cast_slice(&stage));
        }
        self.stage = stage;
        if self.xforms.is_none() {
            let frame = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.frame_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.cam.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.shadow_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.shadow_sampler) },
                ],
            });
            let shadow = |i: usize| {
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &self.shadow_layout,
                    entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.lights[i].as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: buf.as_entire_binding() }],
                })
            };
            self.xforms = Some(Xforms { frame, shadow: [shadow(0), shadow(1)] });
        }
        let xf = self.xforms.as_ref().unwrap();
        self.calls.set(0);
        let mut enc = self.device.create_command_encoder(&Default::default());
        for (i, runs) in [&near, &far].into_iter().enumerate() {
            if !self.shadows || (i == 1 && !refresh_far) {
                continue;
            }
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_layers[i],
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &xf.shadow[i], &[]);
            pass.set_pipeline(&self.shadow_pipeline);
            self.encode(&mut pass, runs, Pass::Solid);
            pass.set_pipeline(&self.shadow_cutout_pipeline);
            self.encode(&mut pass, runs, Pass::Cutout);
        }
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &xf.frame, &[]);
            pass.set_pipeline(&self.sky_pipeline);
            pass.draw(0..3, 0..1);
            for (pipeline, which) in
                [(&self.pipeline, Pass::Solid), (&self.cutout_pipeline, Pass::Cutout), (&self.water_pipeline, Pass::Water), (&self.decal_pipeline, Pass::Decal), (&self.alpha_pipeline, Pass::Alpha)]
            {
                pass.set_pipeline(pipeline);
                self.encode(&mut pass, &main, which);
            }
        }
        self.queue.submit([enc.finish()]);
    }

    /// Draw runs of instances, one draw per mesh of pass `which`, ordered
    /// by buffers and texture so each is bound once. A blended mesh
    /// without a texture of its own (a decal that only changes normals or
    /// shine, a pane with none) is left out: the plain grey would cover
    /// what it lies on.
    fn encode(&self, pass: &mut wgpu::RenderPass, runs: &[(usize, u32, u32)], which: Pass) {
        let mut draws: Vec<(u32, bool, u32, usize, &GpuMesh, u32, u32)> = Vec::new();
        let blended = matches!(which, Pass::Decal | Pass::Alpha);
        for &(model, a, b) in runs {
            for m in self.models[model].iter().filter(|m| m.pass == which && !(blended && m.tex == 0)) {
                draws.push((m.vchunk, m.format == wgpu::IndexFormat::Uint32, m.ichunk, m.tex, m, a, b));
            }
        }
        draws.sort_unstable_by_key(|d| (d.0, d.1, d.2, d.3));
        self.calls.set(self.calls.get() + draws.len());
        let (mut vb, mut ib, mut tex) = (u32::MAX, (false, u32::MAX), usize::MAX);
        for (vc, wide, ic, t, m, a, b) in draws {
            if vc != vb {
                pass.set_vertex_buffer(0, self.vertices.chunks[vc as usize].0.slice(..));
                vb = vc;
            }
            if (wide, ic) != ib {
                let arena = if wide { &self.indices32 } else { &self.indices16 };
                pass.set_index_buffer(arena.chunks[ic as usize].0.slice(..), m.format);
                ib = (wide, ic);
            }
            if t != tex {
                pass.set_bind_group(1, &self.textures[t].group, &[]);
                tex = t;
            }
            pass.draw_indexed(m.first..m.first + m.count, m.base as i32, a..b);
        }
    }

    /// A frame with map views drawn over it.
    pub fn capture_with(&mut self, cam: &Camera, size: [u32; 2], views: &[MapView]) -> Vec<u8> {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        self.render(cam, &view, size, true);
        self.draw_overlay(&view, size, views);
        crate::render::read_back(&self.device, &self.queue, &tex, size)
    }

    pub fn capture(&mut self, cam: &Camera, size: [u32; 2]) -> Vec<u8> {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.render(cam, &tex.create_view(&Default::default()), size, true);
        crate::render::read_back(&self.device, &self.queue, &tex, size)
    }
}

/// The map drawn over the world: a round minimap or the full map.
pub struct MapView {
    /// Where on screen: x, y, width, height (pixels); round or square.
    pub rect: [f32; 4],
    pub round: bool,
    /// The world point at the rect's centre, metres per screen pixel, and
    /// the angle the map is turned by (0: north up).
    pub centre: glam::Vec2,
    pub per_px: f32,
    pub turn: f32,
    /// The player: x, y and heading (radians from east).
    pub player: glam::Vec3,
    /// A cross at this screen point (the cursor on the full map).
    pub cross: Option<glam::Vec2>,
}

const OVERLAY_SHADER: &str = r#"
struct O {
  rect: vec4<f32>,
  screen: vec4<f32>,
  view: vec4<f32>,
  map: vec4<f32>,
  player: vec4<f32>,
  cross: vec4<f32>,
};
@group(0) @binding(0) var<uniform> o: O;
@group(0) @binding(1) var maptex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VO { @builtin(position) pos: vec4<f32>, @location(0) px: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VO {
  let c = vec2<f32>(f32(i & 1u), f32((i >> 1u) & 1u));
  let px = o.rect.xy + c * o.rect.zw;
  var out: VO;
  out.pos = vec4<f32>(px.x / o.screen.x * 2.0 - 1.0, 1.0 - px.y / o.screen.y * 2.0, 0.0, 1.0);
  out.px = px;
  return out;
}

fn turn(v: vec2<f32>, a: f32) -> vec2<f32> {
  let s = sin(a);
  let c = cos(a);
  return vec2<f32>(v.x * c - v.y * s, v.x * s + v.y * c);
}

// Coverage of the player arrow at `p` (pixels from its centre), pointing
// along `dir` (screen, y down): a dart with a notched tail.
fn dart(p: vec2<f32>, dir: vec2<f32>, size: f32) -> f32 {
  let f = dot(p, dir) / size;
  let r = abs(dot(p, vec2<f32>(-dir.y, dir.x))) / size;
  // Inside the triangle tip (1, 0) .. tail (-0.7, ±0.75), minus the notch.
  let edge = (1.0 - f) * 0.44 - r;
  let notch = f + 0.35 - r * 0.5;
  return clamp(min(edge, notch) * size + 0.5, 0.0, 1.0) * step(-0.75, f);
}

@fragment fn fs(v: VO) -> @location(0) vec4<f32> {
  let c = o.rect.xy + o.rect.zw * 0.5;
  let d = v.px - c;
  let round = o.screen.z > 0.5;
  let radius = o.rect.z * 0.5;
  var a = 1.0;
  if (round) {
    a = clamp(radius - length(d), 0.0, 1.0);
    if (a <= 0.0) { discard; }
  }
  // The world under this pixel, and the map texel there.
  let w = o.view.xy + turn(vec2<f32>(d.x, -d.y) * o.view.z, o.view.w);
  let size = vec2<f32>(textureDimensions(maptex));
  let uv = vec2<f32>((w.x - o.map.x) / (size.x * o.map.z), (o.map.y - w.y) / (size.y * o.map.z));
  var col = textureSample(maptex, samp, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
  if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
    col = vec3<f32>(0.012, 0.03, 0.045);
  }
  if (round) {
    // Rim, and a tick where north is.
    let rim = smoothstep(radius - 5.0, radius - 3.5, length(d));
    col = mix(col, vec3<f32>(0.85, 0.85, 0.82), rim);
    let north = turn(vec2<f32>(0.0, 1.0), -o.view.w);
    let tip = c + vec2<f32>(north.x, -north.y) * (radius - 10.0);
    col = mix(col, vec3<f32>(0.85, 0.08, 0.06), clamp(6.0 - length(v.px - tip), 0.0, 1.0));
  }
  // The player's dart: outline, then fill.
  let pw = turn(o.player.xy - o.view.xy, -o.view.w) / o.view.z;
  let ps = c + vec2<f32>(pw.x, -pw.y);
  let h = turn(vec2<f32>(cos(o.player.z), sin(o.player.z)), -o.view.w);
  let dir = normalize(vec2<f32>(h.x, -h.y));
  col = mix(col, vec3<f32>(0.0), dart(v.px - ps, dir, 15.0));
  col = mix(col, vec3<f32>(1.0, 0.97, 0.9), dart(v.px - ps - dir * 1.0, dir, 11.0));
  if (o.cross.z > 0.5) {
    let q = abs(v.px - o.cross.xy);
    let line = (q.x < 1.0 && q.y < 12.0) || (q.y < 1.0 && q.x < 12.0);
    if (line) { col = vec3<f32>(1.0, 0.85, 0.2); }
  }
  return vec4<f32>(col, a);
}
"#;

/// What the overlay pass needs, made when the map is set.
struct Overlay {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    group: wgpu::BindGroup,
    /// The map's north-west corner and metres per pixel.
    map: [f32; 3],
}

/// Room per map view in the uniform buffer (dynamic offsets align to 256).
const OVERLAY_SLOT: u64 = 256;

impl View3d {
    /// Upload the map the overlay draws from.
    pub fn set_map(&mut self, m: &crate::map::MapImage, format: wgpu::TextureFormat) {
        let chain = world::mip_chain(m.w, m.h, m.rgba.clone());
        let levels = 32 - m.w.max(m.h).leading_zeros();
        let tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("map"),
                size: wgpu::Extent3d { width: m.w, height: m.h, depth_or_array_layers: 1 },
                mip_level_count: levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &chain.2,
        );
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("overlay"), source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()) });
        let layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                layout_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: None }),
                layout_entry(
                    1,
                    wgpu::ShaderStages::FRAGMENT,
                    wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                ),
                layout_entry(2, wgpu::ShaderStages::FRAGMENT, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let pl = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overlay"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let uniforms = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("overlay"),
            size: OVERLAY_SLOT * 4,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &uniforms, offset: 0, size: std::num::NonZeroU64::new(96) }) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&tex.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        self.overlay = Some(Overlay { pipeline, uniforms, group, map: [m.west, m.north, m.per_px] });
    }

    /// Whether a map is there to draw.
    pub fn has_map(&self) -> bool {
        self.overlay.is_some()
    }

    /// Draw map views over what `target` holds.
    pub fn draw_overlay(&self, target: &wgpu::TextureView, size: [u32; 2], views: &[MapView]) {
        let Some(ov) = &self.overlay else { return };
        for (i, v) in views.iter().take(4).enumerate() {
            let cross = v.cross.map_or([0.0; 4], |c| [c.x, c.y, 1.0, 0.0]);
            let u: [f32; 24] = [
                v.rect[0],
                v.rect[1],
                v.rect[2],
                v.rect[3],
                size[0] as f32,
                size[1] as f32,
                if v.round { 1.0 } else { 0.0 },
                0.0,
                v.centre.x,
                v.centre.y,
                v.per_px,
                v.turn,
                ov.map[0],
                ov.map[1],
                ov.map[2],
                0.0,
                v.player.x,
                v.player.y,
                v.player.z,
                0.0,
                cross[0],
                cross[1],
                cross[2],
                cross[3],
            ];
            self.queue.write_buffer(&ov.uniforms, i as u64 * OVERLAY_SLOT, bytemuck::cast_slice(&u));
        }
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&ov.pipeline);
            for i in 0..views.len().min(4) {
                pass.set_bind_group(0, &ov.group, &[(i as u64 * OVERLAY_SLOT) as u32]);
                pass.draw(0..4, 0..1);
            }
        }
        self.queue.submit([enc.finish()]);
    }
}

/// Smooth vertex normals from triangles.
fn normals(p: &[[f32; 3]], idx: &[u32]) -> Vec<[f32; 3]> {
    let mut n = vec![Vec3::ZERO; p.len()];
    for t in idx.as_chunks::<3>().0 {
        let (a, b, c) = (Vec3::from(p[t[0] as usize]), Vec3::from(p[t[1] as usize]), Vec3::from(p[t[2] as usize]));
        let f = (b - a).cross(c - a);
        for &i in t {
            n[i as usize] += f;
        }
    }
    n.iter().map(|v| v.normalize_or(Vec3::Z).to_array()).collect()
}
