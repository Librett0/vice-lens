//! Vice Lens: texture dictionaries, movies with their sound, opening the 3D
//! world, and the settings these need.

use std::io::{BufRead, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

use super::http::{Reply, Request, quote};
use super::{Lens, cache, encode_png};

/// A texture dictionary: where it is and what it holds.
pub(super) struct TexDict {
    archive: String,
    offset: u64,
    names: Vec<String>,
}

/// A Bink 2 movie inside `movies.rpf`.
pub(super) struct Movie {
    offset: u64,
    bytes: u64,
    frames: u32,
    width: u32,
    height: u32,
    fps: f32,
    tracks: Vec<super::bink::Track>,
}

/// At most three FFmpeg runs for stills at once: a gallery of posters asks
/// for dozens together.
static STILLS: (Mutex<u32>, Condvar) = (Mutex::new(0), Condvar::new());

fn with_still_slot<T>(f: impl FnOnce() -> T) -> T {
    let (m, c) = &STILLS;
    let mut n = m.lock().unwrap();
    while *n >= 3 {
        n = c.wait(n).unwrap();
    }
    *n += 1;
    drop(n);
    let r = f();
    *m.lock().unwrap() -= 1;
    c.notify_one();
    r
}

fn config(name: &str) -> PathBuf {
    crate::dirs_config().join(name)
}

fn media_dir() -> PathBuf {
    let d = cache().join("media");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// The FFmpeg Vice Lens runs, what it was found to do, and whether it can
/// make WebM (VP9 and Opus).
#[derive(Clone, Default)]
pub(super) struct Ffmpeg {
    path: Option<PathBuf>,
    bink2: bool,
    webm: bool,
}

/// What a movie is converted to for the app's web view: MP4 (H.264, AAC)
/// where it plays that, else WebM (VP9, Opus). WebKitGTK plays H.264 and
/// AAC only with GStreamer's libav plugin (gst-libav), which a fresh Arch
/// or CachyOS leaves out, while VP9 and Opus come with its base and good
/// plugins. The page asks its web view and says which (`f=webm`).
#[derive(Clone, Copy, PartialEq)]
enum Format {
    Mp4,
    Webm,
}

impl Format {
    fn of(r: &Request) -> Format {
        if r.get("f") == Some("webm") { Format::Webm } else { Format::Mp4 }
    }

    /// The picture of the movie at `o`, or with sound track `t`: the WebM
    /// names differ from the MP4 ones in their stem too, so the notes kept
    /// beside them (`.err`, `.damaged`, `.silent`) do not mix.
    fn file(self, o: u64, t: Option<usize>) -> String {
        let (tag, ext) = match self {
            Format::Mp4 => ("", "mp4"),
            Format::Webm => ("-vp9", "webm"),
        };
        match t {
            None => format!("movie-{o:x}{tag}.{ext}"),
            Some(t) => format!("movie-{o:x}{tag}-a{t}.{ext}"),
        }
    }

    fn ext(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::Webm => "webm",
        }
    }

    /// The sound alone, beside the movie's file: its extension, and the
    /// encoder's arguments.
    fn sound(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Format::Mp4 => ("m4a", &["-c:a", "aac", "-b:a", "192k"]),
            Format::Webm => ("sound.webm", &["-c:a", "libopus", "-b:a", "160k"]),
        }
    }

    /// The picture encoder's arguments: both about as fast (a 13-second
    /// movie at 1280×720 in one to two seconds).
    fn picture(self) -> &'static [&'static str] {
        match self {
            Format::Mp4 => &["-c:v", "libx264", "-preset", "veryfast", "-crf", "22", "-pix_fmt", "yuv420p", "-movflags", "+faststart"],
            Format::Webm => &["-c:v", "libvpx-vp9", "-deadline", "realtime", "-cpu-used", "8", "-row-mt", "1", "-b:v", "0", "-crf", "32", "-pix_fmt", "yuv420p"],
        }
    }
}

/// The file a movie plays from: with sound track `t`, or the picture alone
/// when it has no sound.
fn movie_file(m: &Movie, t: usize, f: Format) -> String {
    f.file(m.offset, (!m.tracks.is_empty()).then_some(t))
}

fn media_url(p: &std::path::Path) -> String {
    format!("/api/media?f={}", p.file_name().map(|n| n.to_string_lossy()).unwrap_or_default())
}

impl Lens {
    // --- 3D world ---

    /// Open the 3D window at a point: a separate process, so the
    /// app stays responsive while the world streams in.
    pub(super) fn explore(&self, r: &Request) -> Reply {
        let (Some(x), Some(y)) = (r.num::<f32>("x"), r.num::<f32>("y")) else { return Reply::bad("x, y") };
        let z = r.num::<f32>("z").unwrap_or(120.0).max(20.0);
        let yaw = r.num::<f32>("yaw").unwrap_or(0.8);
        let Ok(exe) = std::env::current_exe() else { return Reply::bad("cannot find the Vice Lens binary") };
        let spawned = std::process::Command::new(exe)
            .arg("--game")
            .arg(&self.game)
            .arg("--world")
            .arg(format!("{x},{y},400"))
            .arg("--window")
            .env("VICE_LENS_CAM", format!("{x},{y},{z},{yaw},-0.3"))
            .stdin(std::process::Stdio::null())
            .spawn();
        match spawned {
            Ok(_) => Reply::json("{\"started\":true}".into()),
            Err(e) => Reply::json(format!("{{\"started\":false,\"error\":{}}}", quote(&e.to_string()))),
        }
    }

    // --- settings ---

    /// The FFmpeg Vice Lens uses, and whether it decodes Bink 2 (stock
    /// builds do not): of the one chosen in Setup, the one its releases
    /// carry next to it and `ffmpeg` on the PATH, the first that decodes
    /// Bink 2, else the first there is. A stock FFmpeg chosen in Setup
    /// before the releases carried one does not hide the one beside.
    pub(super) fn ffmpeg(&self) -> (Option<PathBuf>, bool) {
        let f = self.ffmpeg_found();
        (f.path, f.bink2)
    }

    fn ffmpeg_found(&self) -> Ffmpeg {
        let mut cached = self.ffmpeg.lock().unwrap();
        if let Some(c) = cached.as_ref() {
            return c.clone();
        }
        let name = format!("ffmpeg{}", std::env::consts::EXE_SUFFIX);
        let chosen = std::fs::read_to_string(config("ffmpeg")).ok().map(|s| PathBuf::from(s.trim()));
        let beside = std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join(&name)));
        let on_path = std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(&name)).find(|f| f.exists()));
        let mut there: Vec<PathBuf> = Vec::new();
        for p in [chosen, beside, on_path].into_iter().flatten() {
            if p.exists() && !there.contains(&p) {
                there.push(p);
            }
        }
        let lists = |p: &PathBuf, what: &str| std::process::Command::new(p).args(["-hide_banner", what]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        let path = there.iter().find(|p| lists(p, "-decoders").contains("bink2video")).or(there.first()).cloned();
        let bink2 = path.as_ref().is_some_and(|p| lists(p, "-decoders").contains("bink2video"));
        let webm = path.as_ref().is_some_and(|p| {
            let e = lists(p, "-encoders");
            e.contains("libvpx-vp9") && e.contains("libopus")
        });
        let f = Ffmpeg { path, bink2, webm };
        *cached = Some(f.clone());
        f
    }

    pub(super) fn settings(&self, r: &Request) -> Reply {
        if let Some(f) = r.get("ffmpeg") {
            let f = f.trim();
            if !f.is_empty() && !PathBuf::from(f).exists() {
                return Reply::json(format!("{{\"ok\":false,\"error\":{}}}", quote("no file there")));
            }
            let c = config("ffmpeg");
            let _ = std::fs::create_dir_all(c.parent().unwrap());
            let _ = if f.is_empty() { std::fs::remove_file(&c) } else { std::fs::write(&c, f) };
            *self.ffmpeg.lock().unwrap() = None;
            // Another FFmpeg may convert what this one could not.
            for e in std::fs::read_dir(media_dir()).into_iter().flatten().flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "err") {
                    let _ = std::fs::remove_file(&p);
                }
                // A damaged picture and everything made from it.
                if p.extension().is_some_and(|x| x == "damaged") {
                    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    for f in std::fs::read_dir(media_dir()).into_iter().flatten().flatten() {
                        if f.file_name().to_string_lossy().starts_with(&format!("{stem}.")) || f.file_name().to_string_lossy().starts_with(&format!("{stem}-")) {
                            let _ = std::fs::remove_file(f.path());
                        }
                    }
                }
            }
        }
        // How the app's web view draws (Linux): on the GPU, or through
        // shared memory for drivers where the GPU path closes the window.
        if let Some(d) = r.get("renderer").filter(|d| ["gpu", "compatible"].contains(d)) {
            let c = config("renderer");
            let _ = std::fs::create_dir_all(c.parent().unwrap());
            let _ = std::fs::write(&c, d);
        }
        let renderer = std::fs::read_to_string(config("renderer")).map(|s| s.trim().to_string()).ok().filter(|s| s == "gpu").unwrap_or_else(|| "compatible".into());
        let drawing = if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_some_and(|v| v != "0") { "compatible" } else { "gpu" };
        let (path, bink2) = self.ffmpeg();
        Reply::json(format!("{{\"ok\":true,\"ffmpeg\":{},\"bink2\":{bink2},\"renderer\":{},\"drawing\":\"{drawing}\"}}", path.map_or("null".into(), |p| quote(&p.to_string_lossy())), quote(&renderer)))
    }

    // --- textures ---

    /// Every texture dictionary in the UI archives, found once.
    fn tex_dicts(&self) -> Arc<Vec<TexDict>> {
        self.texdicts
            .get_or_init(|| {
                let mut out = Vec::new();
                for name in ["textures.rpf", "data.rpf"] {
                    let Some(a) = self.archive(name) else { continue };
                    let mut streams = Vec::new();
                    a.scan(|s, _| {
                        streams.push(s);
                        true
                    });
                    for f in rage::Archive::files(&streams) {
                        let Ok(d) = a.read_file(&f) else { continue };
                        if rage::rsc::looks_like_resource(&d) && rage::txd::probe(&d).is_some() {
                            let names = rage::txd::textures(&d).into_iter().map(|t| t.name).collect();
                            out.push(TexDict { archive: name.to_string(), offset: f.offset(), names });
                        }
                    }
                }
                log::info!("lens: {} texture dictionaries", out.len());
                Arc::new(out)
            })
            .clone()
    }

    /// Dictionaries, or with `q` the textures whose names match it (as the
    /// search options say), at most `limit`.
    pub(super) fn textures_list(&self, r: &Request) -> Reply {
        let dicts = self.tex_dicts();
        let m = match super::search::Matcher::from(r) {
            Ok(m) => m,
            Err(e) => return Reply::bad(&e),
        };
        if m.all() {
            let items: Vec<String> = dicts
                .iter()
                .map(|d| format!("{{\"a\":{},\"o\":\"{:#x}\",\"count\":{},\"first\":{}}}", quote(&d.archive), d.offset, d.names.len(), quote(d.names.first().map_or("", String::as_str))))
                .collect();
            return Reply::json(format!("{{\"dicts\":[{}]}}", items.join(",")));
        }
        let limit = r.num::<usize>("limit").unwrap_or(1000).min(5000);
        let mut hits = Vec::new();
        let mut more = false;
        'all: for d in dicts.iter() {
            for (i, n) in d.names.iter().enumerate() {
                if m.is_match(n) {
                    if hits.len() == limit {
                        more = true;
                        break 'all;
                    }
                    hits.push(format!("{{\"a\":{},\"o\":\"{:#x}\",\"i\":{i},\"name\":{}}}", quote(&d.archive), d.offset, quote(n)));
                }
            }
        }
        Reply::json(format!("{{\"more\":{more},\"hits\":[{}]}}", hits.join(",")))
    }

    /// One dictionary's textures: name, size, format, mips.
    pub(super) fn texdict(&self, r: &Request) -> Reply {
        let (Some(a), Some(o)) = (r.get("a"), r.hex("o")) else { return Reply::bad("a, o") };
        let Some(archive) = self.archive(a) else { return Reply::missing("no such archive") };
        let Ok(res) = archive.resource_at(o) else { return Reply::missing("no resource there") };
        let items: Vec<String> = rage::txd::textures(&res.virt)
            .iter()
            .enumerate()
            .map(|(i, t)| format!("{{\"i\":{i},\"name\":{},\"w\":{},\"h\":{},\"format\":{},\"mips\":{}}}", quote(&t.name), t.width, t.height, quote(dxgi_name(t.dxgi)), t.mips))
            .collect();
        Reply::json(format!("{{\"items\":[{}]}}", items.join(",")))
    }

    /// A texture as PNG, shrunk to at most `max` pixels across if asked.
    pub(super) fn texture(&self, r: &Request) -> Reply {
        let (Some(a), Some(o), Some(i)) = (r.get("a"), r.hex("o"), r.num::<usize>("i")) else { return Reply::bad("a, o, i") };
        let max = r.num::<u32>("max").unwrap_or(u32::MAX);
        let Some(archive) = self.archive(a) else { return Reply::missing("no such archive") };
        let Ok(res) = archive.resource_at(o) else { return Reply::missing("no resource there") };
        let texs = rage::txd::textures(&res.virt);
        let allocs = rage::txd::allocations(&texs, res.phys.len());
        let Some(t) = texs.get(i) else { return Reply::missing("no such texture") };
        let Some(rgba) = rage::txd::top_mip(&res.phys, t, allocs[i]).and_then(|lin| crate::assets::decode(&lin, t.dxgi, t.width, t.height)) else {
            return Reply::missing("this format does not decode yet");
        };
        let (px, w, h) = super::export::shrink(rgba, t.width, t.height, max);
        Reply::bytes("image/png", encode_png(&px, w, h), true)
    }

    // --- movies ---

    /// The Bink 2 movies in `movies.rpf`: headers on 4 KiB boundaries; a
    /// movie found is stepped over whole.
    fn movies(&self) -> Arc<Vec<Movie>> {
        self.movies
            .get_or_init(|| {
                let mut out = Vec::new();
                let Some(a) = self.archive("movies.rpf") else { return Arc::new(out) };
                let b = a.bytes();
                let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
                let mut p = 0usize;
                while p + 44 <= b.len() {
                    if &b[p..p + 3] == b"KB2" && b[p + 3].is_ascii_lowercase() {
                        let bytes = u(p + 4) as u64 + 8;
                        let (frames, width, height, num, den) = (u(p + 8), u(p + 20), u(p + 24), u(p + 28), u(p + 32).max(1));
                        if bytes > 64 && p as u64 + bytes <= b.len() as u64 && (16..=8192).contains(&width) && (16..=8192).contains(&height) {
                            let tracks = super::bink::header(&b[p..p + bytes as usize]).map(|h| h.tracks).unwrap_or_default();
                            out.push(Movie { offset: p as u64, bytes, frames, width, height, fps: num as f32 / den as f32, tracks });
                            p = (p + bytes as usize).div_ceil(4096) * 4096;
                            continue;
                        }
                    }
                    p += 4096;
                }
                log::info!("lens: {} movies", out.len());
                Arc::new(out)
            })
            .clone()
    }

    pub(super) fn movies_list(&self, r: &Request) -> Reply {
        let (path, bink2) = self.ffmpeg();
        let f = Format::of(r);
        let items: Vec<String> = self
            .movies()
            .iter()
            .enumerate()
            .map(|(k, m)| {
                let ready = media_dir().join(movie_file(m, 0, f)).exists();
                let tracks: Vec<String> = m.tracks.iter().map(|t| t.id.to_string()).collect();
                format!(
                    "{{\"k\":{k},\"o\":\"{:#x}\",\"bytes\":{},\"frames\":{},\"w\":{},\"h\":{},\"fps\":{},\"seconds\":{:.2},\"audio\":{},\"tracks\":[{}],\"ready\":{ready}}}",
                    m.offset,
                    m.bytes,
                    m.frames,
                    m.width,
                    m.height,
                    m.fps,
                    m.frames as f32 / m.fps.max(1.0),
                    m.tracks.len(),
                    tracks.join(",")
                )
            })
            .collect();
        Reply::json(format!("{{\"ffmpeg\":{},\"bink2\":{bink2},\"items\":[{}]}}", path.map_or("null".into(), |p| quote(&p.to_string_lossy())), items.join(",")))
    }

    /// Make a movie playable in the background: its picture (at most 1280
    /// wide) as MP4, or as WebM for a web view that plays no H.264 (`f`),
    /// once, then for the chosen sound track (`t`, the first by default)
    /// that picture with the track (AAC, or Opus), joined without
    /// re-encoding, so switching tracks takes seconds. The media URL serves
    /// the result once done.
    pub(super) fn movie_prepare(self: &Arc<Self>, r: &Request) -> Reply {
        let Some(o) = r.hex("o") else { return Reply::bad("o") };
        let Some(m) = self.movies().iter().find(|m| m.offset == o).map(|m| (m.bytes, m.frames, m.tracks.len())) else { return Reply::missing("no movie there") };
        let t = r.num::<usize>("t").unwrap_or(0).min(m.2.saturating_sub(1));
        let fmt = Format::of(r);
        let video = media_dir().join(fmt.file(o, None));
        let out = if m.2 == 0 { video.clone() } else { media_dir().join(fmt.file(o, Some(t))) };
        // When the picture does not decode, the sound alone.
        let sound_only = out.with_extension(fmt.sound().0);
        let failed = out.with_extension("err");
        let picture_failed = std::fs::read_to_string(video.with_extension("err")).ok().map(|w| format!("the picture does not decode ({})", w.trim()));
        let damaged = std::fs::read_to_string(video.with_extension("damaged")).ok();
        let ready = |url: &std::path::Path, sound: bool, picture: bool, note: Option<&str>| {
            let silent = out.with_extension("silent").exists();
            let note = note.or(damaged.as_deref().filter(|_| picture));
            Reply::json(format!("{{\"ready\":true,\"url\":{},\"sound\":{sound},\"silent\":{silent},\"picture\":{picture},\"note\":{}}}", quote(&media_url(url)), note.map_or("null".into(), quote)))
        };
        if out.exists() {
            return ready(&out, m.2 > 0, true, None);
        }
        let sound_failed = std::fs::read_to_string(&failed).ok().map(|w| w.trim().to_string());
        match (&picture_failed, &sound_failed) {
            (Some(p), _) if m.2 == 0 => return Reply::json(format!("{{\"ready\":false,\"error\":{}}}", quote(p))),
            (Some(p), Some(s)) => return Reply::json(format!("{{\"ready\":false,\"error\":{}}}", quote(&format!("{p}; {s}")))),
            (Some(p), None) if sound_only.exists() => return ready(&sound_only, true, false, Some(p)),
            (None, Some(s)) if video.exists() => return ready(&video, false, true, Some(s)),
            _ => {}
        }
        let found = self.ffmpeg_found();
        let (Some(ffmpeg), true) = (found.path, found.bink2) else { return Reply::json(format!("{{\"ready\":false,\"error\":{}}}", quote("needs an FFmpeg that decodes Bink 2 (see Setup)"))) };
        if fmt == Format::Webm && !found.webm {
            return Reply::json(format!("{{\"ready\":false,\"error\":{}}}", quote("this window plays no H.264, and this FFmpeg makes no WebM (it needs libvpx and libopus): use the FFmpeg Vice Lens comes with, or open Vice Lens in your browser")));
        }
        let what = format!("Converting movie {o:#x}");
        {
            let mut j = self.jobs.lock().unwrap();
            if j.iter().any(|x| x.0 == what && !x.3) {
                return Reply::json("{\"ready\":false,\"started\":true}".into());
            }
            j.retain(|x| x.0 != what);
            j.push((what.clone(), 0, m.1 as u64, false));
        }
        let (game, this) = (self.game.join("movies.rpf"), self.clone());
        std::thread::spawn(move || {
            let input = format!("subfile,,start,{o},end,{},,:{}", o + m.0, game.display());
            let picture = video.exists() || (picture_failed.is_none() && this.movie_picture(&ffmpeg, &input, &video, &what, m.1 as u64, fmt));
            if m.2 > 0 {
                let (pic, target) = if picture { (Some(video.as_path()), &out) } else { (None, &sound_only) };
                if let Err(why) = this.movie_sound(&ffmpeg, o, m.0, t, pic, target, fmt) {
                    log::warn!("lens: movie {o:#x} track {t}: {why}");
                    let _ = std::fs::write(&failed, why);
                }
            }
            if !picture {
                log::warn!("lens: movie {o:#x}: the picture does not decode");
            }
            this.set_job(&what, m.1 as u64, true);
        });
        Reply::json("{\"ready\":false,\"started\":true}".into())
    }

    /// The picture of a movie as MP4 or WebM, with progress in the job `what`; on
    /// failure, FFmpeg's last complaint is kept beside it (`.err`). FFmpeg
    /// drops frames its Bink 2 decoder cannot read and carries on while
    /// fewer than two in three fail; a picture short of frames is marked
    /// `.damaged`, since what does decode of such a movie is damaged too.
    fn movie_picture(&self, ffmpeg: &std::path::Path, input: &str, video: &std::path::Path, what: &str, frames: u64, fmt: Format) -> bool {
        let part = video.with_extension(format!("part.{}", fmt.ext()));
        let log = video.with_extension("log");
        let child = std::process::Command::new(ffmpeg)
            .args(["-v", "error", "-nostdin", "-protocol_whitelist", "file,subfile", "-i", input, "-map", "0:v:0", "-an"])
            .args(["-vf", "scale='min(1280,iw)':-2"])
            .args(fmt.picture())
            .args(["-progress", "pipe:1", "-y"])
            .arg(&part)
            .stdout(std::process::Stdio::piped())
            .stderr(std::fs::File::create(&log).map_or(std::process::Stdio::null(), std::process::Stdio::from))
            .spawn();
        let mut made = 0;
        let ok = match child {
            Ok(mut c) => {
                if let Some(so) = c.stdout.take() {
                    for line in std::io::BufReader::new(so).lines().map_while(Result::ok) {
                        if let Some(f) = line.strip_prefix("frame=").and_then(|v| v.trim().parse::<u64>().ok()) {
                            made = f;
                            self.set_job(what, f, false);
                        }
                    }
                }
                c.wait().is_ok_and(|s| s.success())
            }
            Err(_) => false,
        };
        let complaint = std::fs::read_to_string(&log).unwrap_or_default();
        let _ = std::fs::remove_file(&log);
        if ok {
            if made * 100 < frames * 98 {
                let _ = std::fs::write(video.with_extension("damaged"), format!("the picture is damaged: only {made} of {frames} frames decode"));
            }
            std::fs::rename(&part, video).is_ok()
        } else {
            let _ = std::fs::remove_file(&part);
            // The decoder's own words, without FFmpeg's stream prefixes.
            let lines = || complaint.lines().filter(|l| !l.contains("Last message repeated"));
            let why = lines().find(|l| l.contains("error rate")).or_else(|| lines().find(|l| l.contains("Error"))).or(complaint.lines().last()).unwrap_or("FFmpeg failed");
            let why = why.rsplit("] ").next().unwrap_or(why).trim();
            let _ = std::fs::write(video.with_extension("err"), why);
            false
        }
    }

    /// Sound track `t` of the movie at `o` (`bytes` long) as AAC (Opus for
    /// WebM), joined with its picture into `out`, or alone without one. A
    /// track that holds only silence (some movies carry one) is marked
    /// beside it, so the app can say so.
    #[allow(clippy::too_many_arguments)]
    fn movie_sound(&self, ffmpeg: &std::path::Path, o: u64, bytes: u64, t: usize, video: Option<&std::path::Path>, out: &std::path::Path, fmt: Format) -> Result<(), String> {
        let a = self.archive("movies.rpf").ok_or("no movies.rpf")?;
        let b = a.bytes().get(o as usize..(o + bytes) as usize).ok_or("the movie lies past the archive's end")?;
        let stream = super::bink::ueba(b, t).ok_or("its sound track does not read")?;
        let ueba = out.with_extension("ueba");
        let (sound_ext, sound_codec) = fmt.sound();
        let track = out.with_extension(format!("track.{sound_ext}"));
        let part = out.with_extension(format!("part.{}", if video.is_some() { fmt.ext() } else { sound_ext }));
        std::fs::write(&ueba, stream).map_err(|e| e.to_string())?;
        let run = |args: &[&std::ffi::OsStr]| {
            std::process::Command::new(ffmpeg).args(["-v", "error", "-nostdin", "-y"]).args(args).output().map_err(|e| e.to_string()).and_then(|o| {
                let log = String::from_utf8_lossy(&o.stderr).into_owned();
                if o.status.success() { Ok(log) } else { Err(log.lines().last().unwrap_or("FFmpeg failed").to_string()) }
            })
        };
        let s = |v: &str| std::ffi::OsString::from(v);
        // volumedetect reports the peak at the info level.
        let mut decode = vec![s("-v"), s("info"), s("-f"), s("binka"), s("-i"), ueba.clone().into_os_string(), s("-af"), s("volumedetect")];
        decode.extend(sound_codec.iter().map(|v| s(v)));
        decode.push(track.clone().into_os_string());
        let decode: Vec<&std::ffi::OsStr> = decode.iter().map(|v| v.as_os_str()).collect();
        let r = run(&decode)
            .map_err(|e| format!("the sound does not decode ({e}); it needs an FFmpeg that reads Bink Audio 2 (UEBA)"))
            .and_then(|log| {
                let peak = log.lines().find_map(|l| l.split("max_volume:").nth(1)).map(|v| v.trim().trim_end_matches("dB").trim().parse::<f32>().unwrap_or(f32::NEG_INFINITY));
                if peak.is_some_and(|db| db <= -90.0) {
                    let _ = std::fs::write(out.with_extension("silent"), "");
                }
                let Some(video) = video else { return std::fs::rename(&track, &part).map(|_| String::new()).map_err(|e| e.to_string()) };
                let mut join = vec![s("-i"), video.as_os_str().to_os_string(), s("-i"), track.clone().into_os_string(), s("-map"), s("0:v:0"), s("-map"), s("1:a:0"), s("-c"), s("copy")];
                if fmt == Format::Mp4 {
                    join.extend([s("-movflags"), s("+faststart")]);
                }
                join.push(part.clone().into_os_string());
                let join: Vec<&std::ffi::OsStr> = join.iter().map(|v| v.as_os_str()).collect();
                run(&join)
            })
            .and_then(|_| std::fs::rename(&part, out).map_err(|e| e.to_string()));
        for f in [&ueba, &track, &part] {
            let _ = std::fs::remove_file(f);
        }
        r
    }

    /// A first frame of a movie, as a poster.
    pub(super) fn movie_poster(&self, r: &Request) -> Reply {
        let Some(o) = r.hex("o") else { return Reply::bad("o") };
        let Some(bytes) = self.movies().iter().find(|m| m.offset == o).map(|m| m.bytes) else { return Reply::missing("no movie there") };
        let png = media_dir().join(format!("poster-{o:x}.png"));
        if !png.exists() {
            let (Some(ffmpeg), true) = self.ffmpeg() else { return Reply::missing("needs an FFmpeg that decodes Bink 2") };
            let input = format!("subfile,,start,{o},end,{},,:{}", o + bytes, self.game.join("movies.rpf").display());
            let part = png.with_extension("part.png");
            with_still_slot(|| {
                std::process::Command::new(ffmpeg)
                    .args(["-v", "error", "-nostdin", "-protocol_whitelist", "file,subfile", "-ss", "1", "-i", &input, "-frames:v", "1", "-vf", "scale=480:-2", "-f", "image2", "-y"])
                    .arg(&part)
                    .output()
            })
            .ok()
            .filter(|o| o.status.success())
            .and_then(|_| std::fs::rename(&part, &png).ok());
        }
        match std::fs::read(&png) {
            Ok(b) => Reply::bytes("image/png", b, true),
            Err(_) => Reply::missing("no frame decoded"),
        }
    }

    /// The frame of a movie at `t` seconds, at full size, as PNG.
    pub(super) fn movie_frame(&self, r: &Request) -> Reply {
        let (Some(o), Some(t)) = (r.hex("o"), r.num::<f64>("t")) else { return Reply::bad("o, t") };
        let Some(bytes) = self.movies().iter().find(|m| m.offset == o).map(|m| m.bytes) else { return Reply::missing("no movie there") };
        let (Some(ffmpeg), true) = self.ffmpeg() else { return Reply::missing("needs an FFmpeg that decodes Bink 2") };
        let input = format!("subfile,,start,{o},end,{},,:{}", o + bytes, self.game.join("movies.rpf").display());
        let out = with_still_slot(|| {
            std::process::Command::new(ffmpeg)
                .args(["-v", "error", "-nostdin", "-protocol_whitelist", "file,subfile", "-ss", &format!("{:.3}", t.max(0.0)), "-i", &input, "-frames:v", "1", "-f", "image2pipe", "-c:v", "png", "-"])
                .output()
        });
        match out {
            Ok(o) if o.status.success() && o.stdout.starts_with(b"\x89PNG") => Reply::bytes("image/png", o.stdout, true),
            _ => Reply::missing("no frame decoded there"),
        }
    }

    /// A movie as stored: the Bink 2 file itself.
    pub(super) fn movie_raw(&self, r: &Request) -> Reply {
        let Some(o) = r.hex("o") else { return Reply::bad("o") };
        let Some(bytes) = self.movies().iter().find(|m| m.offset == o).map(|m| m.bytes) else { return Reply::missing("no movie there") };
        let Some(a) = self.archive("movies.rpf") else { return Reply::missing("no movies.rpf") };
        match a.bytes().get(o as usize..(o + bytes) as usize) {
            Some(b) => Reply::bytes("video/x-bink", b.to_vec(), false),
            None => Reply::missing("the movie lies past the archive's end"),
        }
    }

    /// A file Lens made (in the media cache), with byte ranges for seeking.
    pub(super) fn media(&self, r: &Request) -> Reply {
        let Some(f) = r.get("f").filter(|f| !f.contains('/') && !f.contains("..")) else { return Reply::bad("f") };
        let path = media_dir().join(f);
        let Ok(mut file) = std::fs::File::open(&path) else { return Reply::missing("not made yet") };
        let len = file.metadata().map_or(0, |m| m.len());
        let ctype = if f.ends_with(".mp4") {
            "video/mp4"
        } else if f.ends_with(".webm") {
            "video/webm"
        } else if f.ends_with(".m4a") {
            "audio/mp4"
        } else if f.ends_with(".png") {
            "image/png"
        } else {
            "application/octet-stream"
        };
        let (start, end) = match r.range.as_deref().and_then(|v| v.strip_prefix("bytes=")).and_then(|v| v.split_once('-')) {
            Some((a, b)) => {
                let s = a.parse::<u64>().unwrap_or(0).min(len.saturating_sub(1));
                let e = b.parse::<u64>().ok().unwrap_or(len.saturating_sub(1)).min(len.saturating_sub(1)).min(s + (8 << 20));
                (s, e)
            }
            None => (0, len.saturating_sub(1)),
        };
        let mut body = vec![0u8; (end + 1 - start) as usize];
        if file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut body).is_err() {
            return Reply::missing("read failed");
        }
        let mut reply = Reply::bytes(ctype, body, true);
        if r.range.is_some() {
            reply.status = 206;
            reply.extra.push(("Content-Range", format!("bytes {start}-{end}/{len}")));
        }
        reply.extra.push(("Accept-Ranges", "bytes".into()));
        reply
    }

    pub(super) fn set_job(&self, what: &str, done: u64, finished: bool) {
        if let Some(j) = self.jobs.lock().unwrap().iter_mut().find(|x| x.0 == what && !x.3) {
            j.1 = done;
            j.3 = finished;
        }
    }
}

pub(super) fn dxgi_name(d: u8) -> &'static str {
    match d {
        71 => "BC1",
        72 => "BC1 sRGB",
        74 | 75 => "BC2",
        77 => "BC3",
        78 => "BC3 sRGB",
        80 | 81 => "BC4",
        83 | 84 => "BC5",
        98 => "BC7",
        99 => "BC7 sRGB",
        28 => "RGBA8",
        29 => "RGBA8 sRGB",
        87 => "BGRA8",
        91 => "BGRA8 sRGB",
        61 => "R8",
        65 => "A8",
        2 => "RGBA32F",
        10 => "RGBA16F",
        11 => "RGBA16",
        24 => "RGB10A2",
        34 => "RG16F",
        35 => "RG16",
        41 => "R32F",
        49 => "RG8",
        54 => "R16F",
        56 => "R16",
        95 => "BC6H",
        96 => "BC6H signed",
        _ => "unknown",
    }
}
