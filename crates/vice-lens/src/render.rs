//! 2D renderer for fwui draw commands (wgpu, Vulkan).
//!
//! Everything samples one RGBA atlas: decoded UI textures, glyphs rasterised
//! from the game's vector fonts at the size they are drawn, and a white texel
//! for flat fills. Circles are drawn analytically in the shader.

use std::collections::HashMap;

use fwui::draw::{Command, corners};
use fwui::style::{Paint, TextStyle};
use rage::gfx;
use tiny_skia::{FillRule, Paint as SkPaint, PathBuilder, Pixmap, Transform};

use crate::assets::Game;

pub const CANVAS: [f32; 2] = [3840.0, 2160.0];
const ATLAS: u32 = 4096;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    mode: f32,
}

#[derive(Clone, Copy)]
struct Slot {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[derive(Clone, Copy)]
struct Glyph {
    slot: Option<Slot>,
    /// Offset of the bitmap's top-left from the pen position, in pixels.
    off: [f32; 2],
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    atlas: wgpu::Texture,
    // shelf packer
    shelf: (u32, u32, u32),
    white: Slot,
    images: HashMap<(String, String), Option<Slot>>,
    glyphs: HashMap<(usize, u32, char), Glyph>,
    pub format: wgpu::TextureFormat,
}

const SHADER: &str = r#"
struct U { size: vec2<f32> };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
struct V { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32>, @location(2) mode: f32 };
@vertex fn vs(@location(0) p: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) c: vec4<f32>, @location(3) m: f32) -> V {
  var o: V;
  o.pos = vec4<f32>(p.x / u.size.x * 2.0 - 1.0, 1.0 - p.y / u.size.y * 2.0, 0.0, 1.0);
  o.uv = uv; o.color = c; o.mode = m; return o;
}
@fragment fn fs(v: V) -> @location(0) vec4<f32> {
  if (v.mode > 0.5) {
    let d = length(v.uv);
    let aa = fwidth(d);
    let a = 1.0 - smoothstep(1.0 - aa, 1.0, d);
    return vec4<f32>(v.color.rgb, v.color.a * a);
  }
  return textureSample(atlas, samp, v.uv) * v.color;
}
"#;

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Renderer {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("ui"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas"),
            size: wgpu::Extent3d { width: ATLAS, height: ATLAS, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor { label: Some("u"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let view = atlas.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs })],
            },
            primitive: wgpu::PrimitiveState::default(),
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
        let mut r = Renderer { device, queue, pipeline, bind, uniform, atlas, shelf: (0, 0, 0), white: Slot { x: 0, y: 0, w: 0, h: 0 }, images: HashMap::new(), glyphs: HashMap::new(), format };
        r.white = r.upload(4, 4, &[255u8; 64]).expect("atlas");
        r
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn present(&self, frame: wgpu::SurfaceTexture) {
        self.queue.present(frame);
    }

    /// Place an RGBA bitmap in the atlas (with a 1-texel gap).
    fn upload(&mut self, w: u32, h: u32, rgba: &[u8]) -> Option<Slot> {
        let (mut x, mut y, mut row) = self.shelf;
        if x + w + 1 > ATLAS {
            x = 0;
            y += row + 1;
            row = 0;
        }
        if y + h + 1 > ATLAS || w + 1 > ATLAS {
            log::warn!("UI atlas full");
            return None;
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.atlas, mip_level: 0, origin: wgpu::Origin3d { x, y, z: 0 }, aspect: wgpu::TextureAspect::All },
            rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.shelf = (x + w + 1, y, row.max(h));
        Some(Slot { x, y, w, h })
    }

    fn image(&mut self, game: &Game, dict: &str, name: &str) -> Option<Slot> {
        let key = (dict.to_string(), name.to_string());
        if let Some(s) = self.images.get(&key) {
            return *s;
        }
        let slot = game.image(dict, name).and_then(|img| self.upload(img.width, img.height, &img.rgba));
        if slot.is_none() {
            log::warn!("texture {dict}/{name} unavailable");
        }
        self.images.insert(key, slot);
        slot
    }

    fn glyph(&mut self, game: &Game, font: usize, px: u32, c: char) -> Glyph {
        if let Some(g) = self.glyphs.get(&(font, px, c)) {
            return *g;
        }
        let f = &game.fonts[font];
        let g = match f.glyph(c) {
            Some(gl) if !gl.path.is_empty() => {
                let s = px as f32 / gfx::EM;
                let pts = gl.path.iter().flat_map(|seg| match seg {
                    gfx::Seg::Move(p) | gfx::Seg::Line(p) => vec![*p],
                    gfx::Seg::Quad(a, b) => vec![*a, *b],
                });
                let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for p in pts {
                    x0 = x0.min(p[0] * s);
                    y0 = y0.min(p[1] * s);
                    x1 = x1.max(p[0] * s);
                    y1 = y1.max(p[1] * s);
                }
                let (ox, oy) = (x0.floor() - 1.0, y0.floor() - 1.0);
                let (w, h) = ((x1 - ox).ceil() as u32 + 2, (y1 - oy).ceil() as u32 + 2);
                let mut pb = PathBuilder::new();
                for seg in &gl.path {
                    match *seg {
                        gfx::Seg::Move([x, y]) => pb.move_to(x * s - ox, y * s - oy),
                        gfx::Seg::Line([x, y]) => pb.line_to(x * s - ox, y * s - oy),
                        gfx::Seg::Quad([cx, cy], [x, y]) => pb.quad_to(cx * s - ox, cy * s - oy, x * s - ox, y * s - oy),
                    }
                }
                let slot = pb.finish().and_then(|path| {
                    let mut pm = Pixmap::new(w.max(1), h.max(1))?;
                    let mut paint = SkPaint::default();
                    paint.set_color_rgba8(255, 255, 255, 255);
                    paint.anti_alias = true;
                    pm.fill_path(&path, &paint, FillRule::EvenOdd, Transform::identity(), None);
                    // Premultiplied white: store as straight white + alpha.
                    let rgba: Vec<u8> = pm.data().chunks(4).flat_map(|p| [255, 255, 255, p[3]]).collect();
                    self.upload(pm.width(), pm.height(), &rgba)
                });
                Glyph { slot, off: [ox, oy] }
            }
            _ => Glyph { slot: None, off: [0.0, 0.0] },
        };
        self.glyphs.insert((font, px, c), g);
        g
    }

    /// A quad with corner colours top-left, top-right, bottom-right, bottom-left.
    fn quad(&self, v: &mut Vec<Vertex>, r: [f32; 4], uv: [f32; 4], c: [[f32; 4]; 4], mode: f32) {
        let [x, y, w, h] = r;
        let [u0, v0, u1, v1] = uv;
        let p = |px, py, u, vv, c| Vertex { pos: [px, py], uv: [u, vv], color: c, mode };
        let (a, b, cc, d) = (p(x, y, u0, v0, c[0]), p(x + w, y, u1, v0, c[1]), p(x + w, y + h, u1, v1, c[2]), p(x, y + h, u0, v1, c[3]));
        v.extend_from_slice(&[a, b, cc, a, cc, d]);
    }

    fn uv(s: Slot) -> [f32; 4] {
        let a = ATLAS as f32;
        [(s.x as f32 + 0.5) / a, (s.y as f32 + 0.5) / a, (s.x + s.w) as f32 / a - 0.5 / a, (s.y + s.h) as f32 / a - 0.5 / a]
    }

    /// Draw `commands` (canvas coordinates) into `target` of `size` pixels.
    pub fn draw(&mut self, game: &Game, commands: &[Command], background: [f64; 4], target: &wgpu::TextureView, size: [u32; 2]) {
        let s = (size[0] as f32 / CANVAS[0]).min(size[1] as f32 / CANVAS[1]);
        let off = [(size[0] as f32 - CANVAS[0] * s) / 2.0, (size[1] as f32 - CANVAS[1] * s) / 2.0];
        let tr = |r: fwui::Rect| [off[0] + r.x * s, off[1] + r.y * s, r.w * s, r.h * s];
        let mut v: Vec<Vertex> = Vec::new();
        let white = Self::uv(self.white);
        let mid = [(white[0] + white[2]) / 2.0, (white[1] + white[3]) / 2.0];
        let flat = [mid[0], mid[1], mid[0], mid[1]];
        for c in commands {
            match c {
                Command::Rect { rect, paint } => {
                    let cs = match paint {
                        Paint::Solid(c) => [*c; 4],
                        Paint::Corners(cs) => *cs,
                        Paint::Linear { stops, angle } => corners(stops, *angle, rect.w, rect.h),
                    };
                    self.quad(&mut v, tr(*rect), flat, cs, 0.0);
                }
                Command::Circle { rect, color } => self.quad(&mut v, tr(*rect), [-1.0, -1.0, 1.0, 1.0], [*color; 4], 1.0),
                Command::Image { rect, dict, name, tint, flip } => {
                    if let Some(slot) = self.image(game, dict, name) {
                        let mut uv = Self::uv(slot);
                        if *flip {
                            uv.swap(0, 2);
                        }
                        self.quad(&mut v, tr(*rect), uv, [*tint; 4], 0.0);
                    }
                }
                Command::Text { rect, text, style, scale } => {
                    let Some(fi) = game.fonts.iter().position(|f| f.name == style.font) else { continue };
                    let px_size = (style.size * scale * s).round().max(1.0);
                    let f = &game.fonts[fi];
                    let em = px_size / gfx::EM;
                    let [x, y, _, _] = tr(*rect);
                    let baseline = y + f.ascent * em;
                    let mut pen = x;
                    let mut prev = None;
                    let shown: String = if style.upper { text.to_uppercase() } else { text.clone() };
                    for ch in shown.chars() {
                        let f = &game.fonts[fi];
                        let Some(gl) = f.glyph(ch) else {
                            pen += px_size * 0.3;
                            continue;
                        };
                        let adv = gl.advance * em;
                        if let Some(p) = prev {
                            pen += f.kerning.get(&(p, ch)).copied().unwrap_or(0.0) * em;
                        }
                        let g = self.glyph(game, fi, px_size as u32, ch);
                        if let Some(slot) = g.slot {
                            let r = [pen.round() + g.off[0], baseline.round() + g.off[1], slot.w as f32, slot.h as f32];
                            self.quad(&mut v, r, Self::uv(slot), [style.color; 4], 0.0);
                        }
                        pen += adv + style.tracking / 1000.0 * px_size;
                        prev = Some(ch);
                    }
                }
            }
        }
        self.queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&[size[0] as f32, size[1] as f32, 0.0, 0.0]));
        let vb = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vertices"),
            size: (v.len().max(1) * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&vb, 0, bytemuck::cast_slice(&v));
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: background[0], g: background[1], b: background[2], a: background[3] }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.draw(0..v.len() as u32, 0..1);
        }
        self.queue.submit([enc.finish()]);
    }

    /// Render into an offscreen texture and return its RGBA pixels.
    pub fn capture(&mut self, game: &Game, commands: &[Command], size: [u32; 2]) -> Vec<u8> {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        self.draw(game, commands, [0.0, 0.0, 0.0, 1.0], &view, size);
        read_back(&self.device, &self.queue, &tex, size)
    }
}

/// Copy a 4-byte-per-pixel texture to memory.
pub fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, tex: &wgpu::Texture, size: [u32; 2]) -> Vec<u8> {
    let row = (size[0] * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (row * size[1]) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(size[1]) } },
        wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    let data = buf.slice(..).get_mapped_range().expect("mapped readback");
    let mut out = Vec::with_capacity((size[0] * size[1] * 4) as usize);
    for y in 0..size[1] {
        out.extend_from_slice(&data[(y * row) as usize..(y * row + size[0] * 4) as usize]);
    }
    out
}

/// Text size at scale 1 on the canvas, for layout.
pub fn measure(game: &Game, style: &TextStyle, text: &str) -> [f32; 2] {
    let size = style.size;
    let Some(f) = game.font(style.font) else { return [0.0, size] };
    let em = size / gfx::EM;
    let mut w = 0.0;
    let mut prev = None;
    for ch in text.chars() {
        if let Some(p) = prev {
            w += f.kerning.get(&(p, ch)).copied().unwrap_or(0.0) * em;
        }
        w += f.glyph(ch).map_or(size * 0.3, |g| g.advance * em) + style.tracking / 1000.0 * size;
        prev = Some(ch);
    }
    [w, (f.ascent + f.descent) * em]
}
