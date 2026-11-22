//! Layout: measure, then arrange, in the manner of WPF which fwui follows
//! (panels, grids with pixel/auto/star tracks, stack panels, canvases,
//! viewboxes, margins, alignment, RAW vs AUTO content sizing).

use crate::element::{Element, HAlign, Kind, Track, VAlign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w: w.max(0.0), h: h.max(0.0) }
    }
}

/// What layout needs from the outside world.
pub trait Measure {
    /// Size of `e`'s text at scale 1 with at most `max_w` width.
    fn text(&self, e: &Element, max_w: f32) -> [f32; 2];
}

/// A laid-out element: its rectangle in canvas pixels and the scale its
/// content is drawn at (viewboxes scale their subtree).
#[derive(Clone, Debug, Default)]
pub struct Laid {
    pub rect: Rect,
    pub scale: f32,
    pub children: Vec<Laid>,
}

struct Sized {
    /// Desired size including margins.
    size: [f32; 2],
    children: Vec<Sized>,
}

pub fn layout(root: &Element, area: Rect, m: &dyn Measure) -> Laid {
    let s = measure(root, [area.w, area.h], m);
    arrange(root, &s, area, m)
}

fn shown(e: &Element) -> bool {
    e.visible && e.kind != Kind::SubScreen
}

fn measure(e: &Element, avail: [f32; 2], m: &dyn Measure) -> Sized {
    if !shown(e) {
        return Sized { size: [0.0, 0.0], children: e.children.iter().map(|_| Sized { size: [0.0; 2], children: vec![] }).collect() };
    }
    let mx = e.margin[0] + e.margin[2];
    let my = e.margin[1] + e.margin[3];
    let px = e.padding[0] + e.padding[2];
    let py = e.padding[1] + e.padding[3];
    let outer = [e.width.unwrap_or(avail[0] - mx).min(e.max[0]), e.height.unwrap_or(avail[1] - my).min(e.max[1])];
    let inner = [(outer[0] - px).max(0.0), (outer[1] - py).max(0.0)];
    let (content, children) = match e.kind {
        Kind::Grid => measure_grid(e, inner, m),
        Kind::Stack => {
            let mut size = [0.0f32, 0.0f32];
            let kids: Vec<Sized> = e
                .children
                .iter()
                .map(|c| {
                    let a = if e.horizontal { [f32::INFINITY, inner[1]] } else { [inner[0], f32::INFINITY] };
                    let s = measure(c, a, m);
                    if e.horizontal {
                        size[0] += s.size[0];
                        size[1] = size[1].max(s.size[1]);
                    } else {
                        size[0] = size[0].max(s.size[0]);
                        size[1] += s.size[1];
                    }
                    s
                })
                .collect();
            (size, kids)
        }
        Kind::Viewbox => {
            let kids: Vec<Sized> = e.children.iter().map(|c| measure(c, [f32::INFINITY; 2], m)).collect();
            let d = kids.iter().fold([0.0f32; 2], |a, k| [a[0].max(k.size[0]), a[1].max(k.size[1])]);
            let s = fit(d, inner);
            ([d[0] * s, d[1] * s], kids)
        }
        Kind::Text => (m.text(e, inner[0]), Vec::new()),
        Kind::Image | Kind::Rect | Kind::Circle => ([0.0, 0.0], Vec::new()),
        _ => {
            let kids: Vec<Sized> = e.children.iter().map(|c| measure(c, inner, m)).collect();
            let d = kids.iter().fold([0.0f32; 2], |a, k| [a[0].max(k.size[0]), a[1].max(k.size[1])]);
            (d, kids)
        }
    };
    let w = e.width.unwrap_or(content[0] + px).clamp(e.min[0], e.max[0]);
    let h = e.height.unwrap_or(content[1] + py).clamp(e.min[1], e.max[1]);
    Sized { size: [w + mx, h + my], children }
}

/// Uniform scale that fits `d` into `avail` (1 when unconstrained).
fn fit(d: [f32; 2], avail: [f32; 2]) -> f32 {
    let sx = if d[0] > 0.0 && avail[0].is_finite() { avail[0] / d[0] } else { f32::INFINITY };
    let sy = if d[1] > 0.0 && avail[1].is_finite() { avail[1] / d[1] } else { f32::INFINITY };
    let s = sx.min(sy);
    if s.is_finite() { s } else { 1.0 }
}

fn tracks_of(e: &Element) -> (Vec<Track>, Vec<Track>) {
    let c = if e.columns.is_empty() { vec![Track::Star(1.0)] } else { e.columns.clone() };
    let r = if e.rows.is_empty() { vec![Track::Star(1.0)] } else { e.rows.clone() };
    (c, r)
}

/// Resolve track sizes for `total` space given children's desired sizes.
fn resolve(tracks: &[Track], total: f32, desired: &[(usize, usize, f32)]) -> Vec<f32> {
    let mut out: Vec<f32> = tracks.iter().map(|t| if let Track::Pixel(p) = t { *p } else { 0.0 }).collect();
    for &(i, span, d) in desired {
        if span == 1 && matches!(tracks.get(i), Some(Track::Auto)) {
            out[i] = out[i].max(d);
        }
    }
    let used: f32 = out.iter().sum();
    let stars: f32 = tracks.iter().map(|t| if let Track::Star(s) = t { *s } else { 0.0 }).sum();
    if stars > 0.0 {
        let free = if total.is_finite() { (total - used).max(0.0) } else { 0.0 };
        if total.is_finite() {
            for (i, t) in tracks.iter().enumerate() {
                if let Track::Star(s) = t {
                    out[i] = free * s / stars;
                }
            }
        } else {
            // Unconstrained: stars take their children's desired size.
            for &(i, span, d) in desired {
                if span == 1 && matches!(tracks.get(i), Some(Track::Star(_))) {
                    out[i] = out[i].max(d);
                }
            }
        }
    }
    out
}

fn measure_grid(e: &Element, inner: [f32; 2], m: &dyn Measure) -> ([f32; 2], Vec<Sized>) {
    let (cols, rows) = tracks_of(e);
    let kids: Vec<Sized> = e.children.iter().map(|c| measure(c, inner, m)).collect();
    let dw: Vec<_> = e.children.iter().zip(&kids).map(|(c, k)| (c.column.min(cols.len() - 1), c.column_span, k.size[0])).collect();
    let dh: Vec<_> = e.children.iter().zip(&kids).map(|(c, k)| (c.row.min(rows.len() - 1), c.row_span, k.size[1])).collect();
    let w: f32 = resolve(&cols, inner[0], &dw).iter().sum();
    let h: f32 = resolve(&rows, inner[1], &dh).iter().sum();
    ([w, h], kids)
}

fn arrange(e: &Element, s: &Sized, slot: Rect, m: &dyn Measure) -> Laid {
    let r = Rect::new(slot.x + e.margin[0], slot.y + e.margin[1], slot.w - e.margin[0] - e.margin[2], slot.h - e.margin[1] - e.margin[3]);
    let want = [s.size[0] - e.margin[0] - e.margin[2], s.size[1] - e.margin[1] - e.margin[3]];
    let w = e.width.unwrap_or(if e.halign == HAlign::Stretch { r.w } else { want[0].min(r.w) }).min(e.max[0]);
    let h = e.height.unwrap_or(if e.valign == VAlign::Stretch { r.h } else { want[1].min(r.h) }).min(e.max[1]);
    let x = match e.halign {
        HAlign::Left => r.x,
        HAlign::Right => r.x + r.w - w,
        HAlign::Center | HAlign::Stretch => r.x + (r.w - w) / 2.0,
    };
    let y = match e.valign {
        VAlign::Top => r.y,
        VAlign::Bottom => r.y + r.h - h,
        VAlign::Center | VAlign::Stretch => r.y + (r.h - h) / 2.0,
    };
    let rect = Rect::new(x, y, w, h);
    let content = Rect::new(x + e.padding[0], y + e.padding[1], w - e.padding[0] - e.padding[2], h - e.padding[1] - e.padding[3]);
    let children = if !shown(e) {
        e.children.iter().map(|_| Laid::default()).collect()
    } else {
        match e.kind {
            Kind::Grid => {
                let (cols, rows) = tracks_of(e);
                let dw: Vec<_> = e.children.iter().zip(&s.children).map(|(c, k)| (c.column.min(cols.len() - 1), c.column_span, k.size[0])).collect();
                let dh: Vec<_> = e.children.iter().zip(&s.children).map(|(c, k)| (c.row.min(rows.len() - 1), c.row_span, k.size[1])).collect();
                let cw = resolve(&cols, content.w, &dw);
                let rh = resolve(&rows, content.h, &dh);
                let pos = |sizes: &[f32], i: usize| sizes[..i.min(sizes.len())].iter().sum::<f32>();
                e.children
                    .iter()
                    .zip(&s.children)
                    .map(|(c, k)| {
                        let (ci, ri) = (c.column.min(cw.len() - 1), c.row.min(rh.len() - 1));
                        let cx = content.x + pos(&cw, ci);
                        let cy = content.y + pos(&rh, ri);
                        let cwid: f32 = cw[ci..(ci + c.column_span).min(cw.len())].iter().sum();
                        let chei: f32 = rh[ri..(ri + c.row_span).min(rh.len())].iter().sum();
                        arrange(c, k, Rect::new(cx, cy, cwid, chei), m)
                    })
                    .collect()
            }
            Kind::Stack => {
                let mut at = if e.horizontal { content.x } else { content.y };
                e.children
                    .iter()
                    .zip(&s.children)
                    .map(|(c, k)| {
                        let slot = if e.horizontal { Rect::new(at, content.y, k.size[0], content.h) } else { Rect::new(content.x, at, content.w, k.size[1]) };
                        at += if e.horizontal { k.size[0] } else { k.size[1] };
                        arrange(c, k, slot, m)
                    })
                    .collect()
            }
            Kind::Canvas => e.children.iter().zip(&s.children).map(|(c, k)| arrange(c, k, Rect::new(content.x + c.left, content.y + c.top, k.size[0], k.size[1]), m)).collect(),
            Kind::Viewbox => e
                .children
                .iter()
                .zip(&s.children)
                .map(|(c, k)| {
                    let sc = fit(k.size, [content.w, content.h]);
                    let (dw, dh) = (k.size[0] * sc, k.size[1] * sc);
                    let ox = content.x + (content.w - dw) / 2.0;
                    let oy = content.y + (content.h - dh) / 2.0;
                    let mut laid = arrange(c, k, Rect::new(0.0, 0.0, k.size[0], k.size[1]), m);
                    transform(&mut laid, ox, oy, sc);
                    laid
                })
                .collect(),
            _ => e.children.iter().zip(&s.children).map(|(c, k)| arrange(c, k, content, m)).collect(),
        }
    };
    Laid { rect, scale: 1.0, children }
}

fn transform(l: &mut Laid, ox: f32, oy: f32, s: f32) {
    l.rect = Rect::new(ox + l.rect.x * s, oy + l.rect.y * s, l.rect.w * s, l.rect.h * s);
    l.scale *= s;
    for c in &mut l.children {
        transform(c, ox, oy, s);
    }
}
