//! Building every cache Vice Lens needs from the user's copy of the game, in
//! order, each step skipped when its file is already there. The steps are
//! `rage::index`'s; the last draws the map (on the GPU).

use std::path::Path;

/// The files that make up the model index, earlier ones first.
pub const MODEL_LISTS: [&str; 3] = ["models.tsv", "models-missed.tsv", "models-packs.tsv"];

/// The archives besides levels.rpf that hold models: the packs (characters,
/// animals, props, story content) and data.rpf; not their `_bvh4`
/// (ray-tracing), script or licence archives.
pub fn model_archives(game: &Path) -> Vec<String> {
    let mut out: Vec<String> = ["packs.rpf", "data.rpf"].into_iter().filter(|n| game.join(n).exists()).map(String::from).collect();
    let mut packs: Vec<String> = std::fs::read_dir(game.join("pack"))
        .into_iter()
        .flatten()
        .flatten()
        .flat_map(|d| std::fs::read_dir(d.path()).into_iter().flatten().flatten())
        .filter_map(|f| {
            let name = f.file_name().to_string_lossy().into_owned();
            let parent = f.path().parent()?.file_name()?.to_string_lossy().into_owned();
            (name.ends_with(".rpf") && !["_bvh4", "_script", "licenses"].iter().any(|s| name.contains(s))).then(|| format!("pack/{parent}/{name}"))
        })
        .collect();
    packs.sort();
    out.extend(packs);
    out
}

/// The caches in the order they are made: (file, what it is).
pub const STEPS: [(&str, &str); 11] = [
    ("survey-levels.tsv", "Listing levels.rpf"),
    ("models.tsv", "Finding every model"),
    ("dicts-levels.tsv", "Finding model dictionaries"),
    ("levels-drpz.txt", "Finding map data"),
    ("entities.bin", "Reading placements"),
    ("instances.bin", "Reading batch instances"),
    ("archetypes.bin", "Reading archetypes"),
    ("interiors.bin", "Placing interiors"),
    // The last style the map draws: a map made before the styles (and
    // before its textures read right) is made again.
    ("map-relief.png", "Drawing the map"),
    ("models-packs.tsv", "Finding models in the packs"),
    // Clip dictionaries: offset, name, clips, whether all of it decodes.
    ("anim-dicts.tsv", "Listing the animations"),
];

fn write(name: &str, data: &[u8]) -> Result<(), String> {
    let dir = crate::dirs_cache();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let part = dir.join(format!("{name}.part"));
    std::fs::write(&part, data).map_err(|e| format!("{name}: {e}"))?;
    std::fs::rename(&part, dir.join(name)).map_err(|e| format!("{name}: {e}"))
}

/// Make the caches that are missing. `progress(step, done, total)`.
pub fn build(game: &Path, progress: &dyn Fn(usize, u64, u64)) -> Result<(), String> {
    let cache = crate::dirs_cache();
    let have = |n: &str| std::fs::metadata(cache.join(n)).is_ok_and(|m| m.len() > 0);
    let levels = rage::Archive::open(game.join("levels.rpf")).map_err(|e| format!("levels.rpf: {e}"))?;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).saturating_sub(2).clamp(2, 12);
    let step = |i: usize| move |d: u64, t: u64| progress(i, d, t);
    let survey = if have(STEPS[0].0) {
        rage::index::read_survey(&std::fs::read_to_string(cache.join(STEPS[0].0)).map_err(|e| e.to_string())?)
    } else {
        let rows = rage::index::survey(&levels, &step(0));
        write(STEPS[0].0, rows.iter().map(|r| r.line()).collect::<String>().as_bytes())?;
        rows
    };
    progress(0, 1, 1);
    if !have(STEPS[1].0) {
        let found = rage::index::drawables(&levels, &survey, threads, &step(1));
        write(STEPS[1].0, found.iter().map(|(o, n)| format!("levels.rpf\t{o:#x}\t{n}\n")).collect::<String>().as_bytes())?;
    }
    progress(1, 1, 1);
    if !have(STEPS[2].0) {
        let found = rage::index::dictionaries(&levels, &survey, threads, &step(2));
        write(STEPS[2].0, found.iter().map(|(o, h, k, n)| format!("{o:#x}\t{h:08x}\t{k}\t{n}\n")).collect::<String>().as_bytes())?;
    }
    progress(2, 1, 1);
    let map_data = rage::index::map_data(&survey);
    if !have(STEPS[3].0) {
        write(STEPS[3].0, map_data.iter().map(|o| format!("{o:#x}\n")).collect::<String>().as_bytes())?;
    }
    progress(3, 1, 1);
    if !have(STEPS[4].0) {
        write(STEPS[4].0, &rage::index::entities(&levels, &map_data, false, threads, &step(4)))?;
    }
    if !have(STEPS[5].0) {
        write(STEPS[5].0, &rage::index::entities(&levels, &map_data, true, threads, &step(5)))?;
    }
    if !have(STEPS[6].0) {
        write(STEPS[6].0, &rage::index::archetypes(&levels, &map_data, threads, &step(6)))?;
    }
    if !have(STEPS[7].0) {
        let (records, [defined, empty, placed, missing]) = rage::index::interiors(&levels, &map_data, threads, &step(7));
        log::info!("interiors: {defined} defined ({empty} without entities), {placed} placed ({missing} not found), {} entities", records.len() / 60);
        write(STEPS[7].0, &records)?;
    }
    drop(levels);
    if !have(STEPS[8].0) {
        crate::map::make(game, threads, &|d, t| progress(8, d, t));
    }
    progress(8, 1, 1);
    if !have(STEPS[9].0) {
        let archives = model_archives(game);
        let n = archives.len() as u64;
        let mut lines = String::new();
        for (i, name) in archives.iter().enumerate() {
            let a = rage::Archive::open(game.join(name)).map_err(|e| format!("{name}: {e}"))?;
            // Each archive is a share of the step: half listing, half reading.
            let at = |d: u64, t: u64, half: u64| progress(9, i as u64 * 1000 + half * 500 + d * 500 / t.max(1), n * 1000);
            let rows = rage::index::survey(&a, &|d, t| at(d, t, 0));
            for (o, m) in rage::index::models(&a, &rows, threads, &|d, t| at(d, t, 1)) {
                lines.push_str(&format!("{name}\t{o:#x}\t{m}\n"));
            }
        }
        write(STEPS[9].0, lines.as_bytes())?;
    }
    progress(9, 1, 1);
    if !have(STEPS[10].0) {
        // anim.rpf, then the content packs' archives (story scenes, more
        // scenarios), named in a fifth column.
        let mut archives = vec!["anim.rpf".to_string()];
        archives.extend(model_archives(game).into_iter().filter(|a| a.starts_with("pack/")));
        let n = archives.len() as u64;
        let mut lines = String::new();
        for (i, name) in archives.iter().enumerate() {
            let Ok(a) = rage::Archive::open(game.join(name)) else { continue };
            let rows = rage::anim::survey(&a, &|d, t| progress(10, i as u64 * 1000 + d * 1000 / t.max(1), n * 1000));
            let tail = if i == 0 { String::new() } else { format!("\t{name}") };
            lines.extend(rows.iter().map(|(o, nm, c, ok)| format!("{o:#x}\t{nm}\t{c}\t{}{tail}\n", *ok as u8)));
        }
        write(STEPS[10].0, lines.as_bytes())?;
    }
    progress(10, 1, 1);
    Ok(())
}
