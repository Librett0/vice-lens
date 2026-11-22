//! Find the tile layout of an uncompressed texture by trying every way of
//! interleaving the x and y bits inside a 4 KiB tile, each in order, and
//! scoring how smooth the result is (mean difference of neighbouring
//! elements, byte by byte).
//!
//! `swzsearch <raw.bin> <width> <height> <bytes per element> [out.pgm]`:
//! raw is `texraw`'s output; the top mip is tried at the start and at the
//! end of it; the image written is each element's last byte.

/// Tile size in elements for 4 KiB tiles.
fn tile(bpe: usize) -> (usize, usize) {
    match bpe {
        1 => (64, 64),
        2 => (64, 32),
        4 => (32, 32),
        8 => (32, 16),
        _ => (16, 16),
    }
}

fn untile(src: &[u8], w: usize, h: usize, bpe: usize, bits: &[(bool, u32)]) -> Vec<u8> {
    let (tw, th) = tile(bpe);
    let mut out = vec![0u8; w * h * bpe];
    let tiles_x = w.div_ceil(tw);
    for y in 0..h {
        for x in 0..w {
            let (lx, ly) = (x % tw, y % th);
            let mut idx = 0usize;
            for (i, &(is_x, n)) in bits.iter().enumerate() {
                let v = if is_x { lx >> n & 1 } else { ly >> n & 1 };
                idx |= v << i;
            }
            let t = (y / th) * tiles_x + x / tw;
            let s = (t * tw * th + idx) * bpe;
            if let Some(px) = src.get(s..s + bpe) {
                out[(y * w + x) * bpe..(y * w + x + 1) * bpe].copy_from_slice(px);
            }
        }
    }
    out
}

fn roughness(img: &[u8], w: usize, h: usize, bpe: usize) -> f64 {
    let mut s = 0u64;
    let at = |x: usize, y: usize, b: usize| img[(y * w + x) * bpe + b] as i32;
    for y in 0..h {
        for x in 0..w {
            for b in 0..bpe {
                let v = at(x, y, b);
                if x + 1 < w {
                    s += (v - at(x + 1, y, b)).unsigned_abs() as u64;
                }
                if y + 1 < h {
                    s += (v - at(x, y + 1, b)).unsigned_abs() as u64;
                }
            }
        }
    }
    s as f64 / (w * h * bpe) as f64
}

fn name(bits: &[(bool, u32)]) -> String {
    bits.iter().map(|&(x, n)| format!("{}{n}", if x { 'X' } else { 'Y' })).collect::<Vec<_>>().join(" ")
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let raw = std::fs::read(&a[0]).unwrap();
    let (w, h, bpe): (usize, usize, usize) = (a[1].parse().unwrap(), a[2].parse().unwrap(), a[3].parse().unwrap());
    let (tw, th) = tile(bpe);
    let (xb, yb) = (tw.trailing_zeros(), th.trailing_zeros());
    // Whole tiles, as stored.
    let need = w.div_ceil(tw) * tw * h.div_ceil(th) * th * bpe;
    let mut results: Vec<(f64, String, String, Vec<(bool, u32)>, usize)> = Vec::new();
    for (region, off) in [("start", 0usize), ("end", raw.len() - need)] {
        let src = &raw[off..off + need];
        results.push((roughness(&src[..w * h * bpe], w, h, bpe), region.into(), "linear".into(), Vec::new(), off));
        // Every choice of which index bits are x bits.
        for mask in 0u32..1 << (xb + yb) {
            if mask.count_ones() != xb {
                continue;
            }
            let (mut xi, mut yi) = (0, 0);
            let bits: Vec<(bool, u32)> = (0..xb + yb)
                .map(|i| {
                    if mask >> i & 1 == 1 {
                        xi += 1;
                        (true, xi - 1)
                    } else {
                        yi += 1;
                        (false, yi - 1)
                    }
                })
                .collect();
            let img = untile(src, w, h, bpe, &bits);
            results.push((roughness(&img, w, h, bpe), region.into(), name(&bits), bits, off));
        }
    }
    results.sort_by(|a, b| a.0.total_cmp(&b.0));
    for r in results.iter().take(8) {
        println!("{:7.2}  {:5}  {}", r.0, r.1, r.2);
    }
    for r in results.iter().filter(|r| r.2 == "linear") {
        println!("{:7.2}  {:5}  {}", r.0, r.1, r.2);
    }
    if let Some(out) = a.get(4) {
        let best = &results[0];
        let img = if best.3.is_empty() { raw[best.4..best.4 + w * h * bpe].to_vec() } else { untile(&raw[best.4..best.4 + need], w, h, bpe, &best.3) };
        let mut pgm = format!("P5 {w} {h} 255\n").into_bytes();
        pgm.extend(img.chunks(bpe).map(|e| e[bpe - 1]));
        std::fs::write(out, pgm).unwrap();
    }
}
