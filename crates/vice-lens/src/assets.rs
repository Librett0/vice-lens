//! The game data Vice Lens runs from, located by scanning the archives.

use std::collections::HashMap;
use std::path::Path;

use fwui::Styles;
use fwui::store::StyleStore;
use rage::pso::Pso;
use rage::rbf::Node;
use rage::{Archive, Stream, gfx, rbf, rsc, text, txd};

pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

struct Dict {
    archive: usize,
    first: Stream,
    names: Vec<String>,
}

pub struct Game {
    pub docs: Vec<Node>,
    pub text: text::TextTable,
    pub fonts: Vec<gfx::Font>,
    pub styles: Styles,
    archives: Vec<(Archive, Vec<Stream>)>,
    dicts: Vec<Dict>,
    by_texture: HashMap<String, Vec<usize>>,
    /// Which dictionary a game dictionary name refers to, decided from the
    /// texture names the UI asks for under that name.
    aliases: HashMap<String, usize>,
}

fn scan(path: &Path) -> std::io::Result<(Archive, Vec<Stream>)> {
    let a = Archive::open(path).map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut streams = Vec::new();
    a.scan(|s, _| {
        streams.push(s);
        true
    });
    Ok((a, streams))
}

impl Game {
    pub fn load(dir: &Path, lang: &str) -> std::io::Result<Game> {
        let t = std::time::Instant::now();
        let mut archives = vec![scan(&dir.join("data.rpf"))?, scan(&dir.join("textures.rpf"))?];
        let mut docs = Vec::new();
        let mut tables = Vec::new();
        let mut fonts: Vec<gfx::Font> = Vec::new();
        let mut dicts = Vec::new();
        let (mut style_pso, mut conversion_pso) = (None, None);
        for (ai, (a, streams)) in archives.iter_mut().enumerate() {
            for f in Archive::files(streams) {
                let Ok(d) = a.read_file(&f) else { continue };
                if rbf::is_rbf(&d) {
                    if let Ok(n) = rbf::parse(&d) {
                        docs.push(n);
                    }
                } else if text::is_text_table(&d) {
                    if d.len() > 1 << 20
                        && let Ok(t) = text::parse(&d)
                    {
                        tables.push(t);
                    }
                } else if d.starts_with(b"PSIN") {
                    if let Ok(p) = Pso::parse(d) {
                        if p.block_of_type(0xebbc_845f).is_some_and(|b| b.size > 0x1000) {
                            style_pso = Some(p);
                        } else if p.block_of_type(0x9013_0782).is_some() {
                            conversion_pso = Some(p);
                        }
                    }
                } else if gfx::is_gfx(&d) {
                    for font in gfx::fonts(&d).unwrap_or_default() {
                        if !fonts.iter().any(|f| f.name == font.name) {
                            fonts.push(font);
                        }
                    }
                } else if rsc::looks_like_resource(&d) && txd::probe(&d).is_some() {
                    let names = txd::textures(&d).into_iter().map(|t| t.name).collect();
                    dicts.push(Dict { archive: ai, first: f.streams[0], names });
                }
            }
        }
        // Each language's table, told apart by how it spells "Settings". The
        // copy holds eight; Russian, German and Polish text is not in it.
        let settings = match lang {
            "en" => "Settings",
            "pt" => "Configurações",
            "zh" => "设置",
            "fr" => "Paramètres",
            "it" => "Impostazioni",
            "ko" => "설정",
            "es" => "Ajustes",
            "mx" => "Configuración",
            other => {
                log::warn!("no {other} text in these files; using English (have: en pt zh fr it ko es mx)");
                "Settings"
            }
        };
        let pick = tables.iter().position(|t| t.get("PM_INDX_SET") == Some(settings)).unwrap_or(0);
        let text = if tables.is_empty() { text::TextTable::default() } else { tables.swap_remove(pick) };
        let mut by_texture: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, d) in dicts.iter().enumerate() {
            for n in &d.names {
                by_texture.entry(n.clone()).or_default().push(i);
            }
        }
        let styles = match (&style_pso, &conversion_pso) {
            (Some(s), Some(c)) => Styles::new(StyleStore::load(s, c)),
            _ => {
                log::warn!("style store not found; drawing without styles");
                Styles::default()
            }
        };
        let mut g = Game { docs, text, fonts, styles, archives, dicts, by_texture, aliases: HashMap::new() };
        g.resolve_aliases();
        log::info!(
            "game data: {} UI documents, {} labels, {} fonts, {} texture dictionaries, {} styles, {} conversions in {:.1?}",
            g.docs.len(),
            g.text.len(),
            g.fonts.len(),
            g.dicts.len(),
            g.styles.store.styles.len(),
            g.styles.store.conversions.len(),
            t.elapsed()
        );
        Ok(g)
    }

    fn resolve_aliases(&mut self) {
        let mut wanted: HashMap<String, Vec<String>> = HashMap::new();
        fn collect(n: &Node, out: &mut HashMap<String, Vec<String>>) {
            if let (Some(d), Some(t)) = (n.text_of("TextureDictionary"), n.text_of("Texture")) {
                out.entry(d.trim().to_string()).or_default().push(t.trim().to_string());
            }
            for c in &n.children {
                collect(c, out);
            }
        }
        for d in &self.docs {
            collect(d, &mut wanted);
        }
        for (dict, names) in wanted {
            let mut score: HashMap<usize, usize> = HashMap::new();
            for n in &names {
                for &i in self.by_texture.get(n).map(Vec::as_slice).unwrap_or(&[]) {
                    *score.entry(i).or_default() += 1;
                }
            }
            if let Some((&best, _)) = score.iter().max_by_key(|(i, s)| (**s, usize::MAX - **i)) {
                self.aliases.insert(dict, best);
            }
        }
    }

    /// A UI document by its `Name` (screens, prefabs) or `ID` (flow blocks).
    pub fn doc(&self, name: &str) -> Option<&Node> {
        self.docs.iter().find(|d| d.text_of("Name").is_some_and(|n| n.trim() == name) || d.text_of("ID").is_some_and(|n| n.trim() == name))
    }

    pub fn font(&self, name: &str) -> Option<&gfx::Font> {
        self.fonts.iter().find(|f| f.name == name)
    }

    /// Decode a texture from a dictionary (by the game's dictionary name,
    /// or any dictionary holding a texture of that name).
    pub fn image(&self, dict: &str, name: &str) -> Option<Image> {
        let di = self.aliases.get(dict).copied().filter(|&i| self.dicts[i].names.iter().any(|n| n == name)).or_else(|| self.by_texture.get(name)?.first().copied())?;
        let d = &self.dicts[di];
        let (a, streams) = &self.archives[d.archive];
        let next = |cursor: u64| {
            let at = (cursor + 15) & !15;
            let i = streams.partition_point(|s| s.offset < at);
            streams.get(i).copied()
        };
        let res = rsc::read(a, d.first, next).ok()?;
        let texs = txd::textures(&res.virt);
        let allocs = txd::allocations(&texs, res.phys.len());
        let (i, t) = texs.iter().enumerate().find(|(_, t)| t.name == name)?;
        let lin = txd::top_mip(&res.phys, t, allocs[i])?;
        let rgba = decode(&lin, t.dxgi, t.width, t.height)?;
        Some(Image { width: t.width, height: t.height, rgba })
    }
}

/// Linear texel data to RGBA8.
pub fn decode(lin: &[u8], dxgi: u8, w: u32, h: u32) -> Option<Vec<u8>> {
    let (w, h) = (w as usize, h as usize);
    let mut px = vec![0u32; w * h];
    let ok = match dxgi {
        71 | 72 => texture2ddecoder::decode_bc1(lin, w, h, &mut px).is_ok(),
        // BC2 keeps plain 4-bit alpha; BC3 interpolates it.
        74 | 75 => texture2ddecoder::decode_bc2(lin, w, h, &mut px).is_ok(),
        77 | 78 => texture2ddecoder::decode_bc3(lin, w, h, &mut px).is_ok(),
        80 | 81 => texture2ddecoder::decode_bc4(lin, w, h, &mut px).is_ok(),
        83 | 84 => texture2ddecoder::decode_bc5(lin, w, h, &mut px).is_ok(),
        98 | 99 => texture2ddecoder::decode_bc7(lin, w, h, &mut px).is_ok(),
        28 | 29 => return lin.get(..w * h * 4).map(<[u8]>::to_vec),
        87 | 91 => return Some(lin.get(..w * h * 4)?.chunks(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()),
        61 => return Some(lin.get(..w * h)?.iter().flat_map(|&v| [v, v, v, 255]).collect()),
        65 => return Some(lin.get(..w * h)?.iter().flat_map(|&v| [255, 255, 255, v]).collect()),
        95 => texture2ddecoder::decode_bc6_unsigned(lin, w, h, &mut px).is_ok(),
        96 => texture2ddecoder::decode_bc6_signed(lin, w, h, &mut px).is_ok(),
        // Wider formats, brought down to 8 bits (floats clamped to 0..1,
        // as a viewer shows them; sRGB-encoded for display).
        2 => return floats(lin, w * h, 4, 4, |b| f32::from_le_bytes(b.try_into().unwrap())),
        10 => return floats(lin, w * h, 4, 2, half),
        34 => return floats(lin, w * h, 2, 2, half),
        54 => return floats(lin, w * h, 1, 2, half),
        41 => return floats(lin, w * h, 1, 4, |b| f32::from_le_bytes(b.try_into().unwrap())),
        11 => return Some(lin.get(..w * h * 8)?.chunks(2).map(|c| c[1]).collect()),
        35 => return Some(lin.get(..w * h * 4)?.chunks(4).flat_map(|c| [c[1], c[3], 0, 255]).collect()),
        56 => return Some(lin.get(..w * h * 2)?.chunks(2).flat_map(|c| [c[1], c[1], c[1], 255]).collect()),
        49 => return Some(lin.get(..w * h * 2)?.chunks(2).flat_map(|c| [c[0], c[1], 0, 255]).collect()),
        24 => {
            return Some(
                lin.get(..w * h * 4)?
                    .chunks(4)
                    .flat_map(|c| {
                        let v = u32::from_le_bytes(c.try_into().unwrap());
                        [(v & 1023) >> 2, (v >> 10 & 1023) >> 2, (v >> 20 & 1023) >> 2, (v >> 30) * 85].map(|x| x as u8)
                    })
                    .collect(),
            );
        }
        _ => false,
    };
    // The decoder yields BGRA packed in a u32.
    ok.then(|| {
        px.iter()
            .flat_map(|p| {
                let b = p.to_le_bytes();
                [b[2], b[1], b[0], b[3]]
            })
            .collect()
    })
}

/// An IEEE half float.
fn half(b: &[u8]) -> f32 {
    let h = u16::from_le_bytes([b[0], b[1]]) as u32;
    let (sign, exp, man) = (h >> 15, (h >> 10) & 31, h & 1023);
    let v = match exp {
        0 => (man as f32) * 2f32.powi(-24),
        31 => f32::INFINITY,
        e => (1.0 + man as f32 / 1024.0) * 2f32.powi(e as i32 - 15),
    };
    if sign == 1 { -v } else { v }
}

/// Float texels (`channels` of `size` bytes each) to RGBA8: clamped to
/// 0..1 and sRGB-encoded; missing channels are 0, missing alpha opaque.
fn floats(lin: &[u8], n: usize, channels: usize, size: usize, read: impl Fn(&[u8]) -> f32) -> Option<Vec<u8>> {
    let px = lin.get(..n * channels * size)?;
    let srgb = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        ((if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }) * 255.0 + 0.5) as u8
    };
    Some(
        px.chunks(channels * size)
            .flat_map(|t| {
                let c = |k: usize| if k < channels { read(&t[k * size..k * size + size]) } else { 0.0 };
                let a = if channels == 4 { (c(3).clamp(0.0, 1.0) * 255.0 + 0.5) as u8 } else { 255 };
                if channels == 1 {
                    let g = srgb(c(0));
                    [g, g, g, a]
                } else {
                    [srgb(c(0)), srgb(c(1)), srgb(c(2)), a]
                }
            })
            .collect(),
    )
}
