//! Turn laid-out elements into draw commands.

use rage::joaat;

use crate::element::{Element, Kind};
use crate::layout::{Laid, Rect};
use crate::style::{Color, Look, Paint, Styles, TextStyle, WHITE};

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Rect { rect: Rect, paint: Paint },
    Circle { rect: Rect, color: Color },
    Image { rect: Rect, dict: String, name: String, tint: Color, flip: bool },
    Text { rect: Rect, text: String, style: TextStyle, scale: f32 },
}

/// Per-frame UI state the engine supplies (what its data providers and focus
/// behaviours decided).
pub trait State {
    /// Resolved string for a text element (label lookup).
    fn string(&self, e: &Element) -> Option<String>;
    /// Whether `e` is a focused item (its subtree draws in focused states).
    fn focused(&self, e: &Element) -> bool;
    /// Data-bound visibility override.
    fn visible(&self, e: &Element, focused: bool) -> Option<bool>;
}

/// A styled-state target still to be reached: remaining path, conversion,
/// and the focus of the element that owns the behaviour.
#[derive(Clone)]
struct Pending<'a> {
    path: &'a [String],
    conversion: &'a str,
    focused: bool,
}

/// The style an element is drawn with: a conversion target's state style,
/// or its own `Style`.
pub fn style_of(e: &Element, conversion: Option<u32>) -> Option<u32> {
    conversion.or_else(|| e.style.as_deref().map(joaat))
}

/// Text look for an element without a resolvable style.
pub fn fallback_text(styles: &Styles) -> TextStyle {
    styles.look_named("MENU_TEXT_NEUTRAL_ENABLED").text.unwrap_or(TextStyle { font: "gta_art_deco_regular", size: 52.0, color: WHITE, upper: false, tracking: 0.0 })
}

pub fn draw(e: &Element, l: &Laid, st: &dyn State, styles: &Styles, out: &mut Vec<Command>) {
    walk(e, l, st, styles, 1.0, false, None, &[], out);
}

#[allow(clippy::too_many_arguments)]
fn walk(e: &Element, l: &Laid, st: &dyn State, styles: &Styles, alpha: f32, focused: bool, conversion: Option<u32>, pending: &[Pending<'_>], out: &mut Vec<Command>) {
    let focused = focused || st.focused(e);
    if !st.visible(e, focused).unwrap_or(e.visible) || e.kind == Kind::SubScreen {
        return;
    }
    let alpha = alpha * e.alpha;
    if alpha <= 0.0 {
        return;
    }
    let look = style_of(e, conversion).map(|s| styles.look(s)).unwrap_or_default();
    let fade = |c: Color| [c[0], c[1], c[2], c[3] * alpha];
    match e.kind {
        Kind::Rect => {
            if let Some(p) = look.fill.as_ref().or(None) {
                out.push(Command::Rect { rect: l.rect, paint: fade_paint(p, alpha) });
            }
        }
        Kind::Circle => {
            if let Some(Paint::Solid(c)) = &look.fill {
                out.push(Command::Circle { rect: l.rect, color: fade(*c) });
            }
        }
        Kind::Image => {
            if let Some((dict, name)) = &e.texture {
                out.push(Command::Image { rect: l.rect, dict: dict.clone(), name: name.clone(), tint: fade(tint(&look)), flip: e.flip_horizontal });
            }
        }
        Kind::Text => {
            if let Some(text) = st.string(e) {
                let mut ts = look.text.unwrap_or_else(|| fallback_text(styles));
                ts.color = fade(ts.color);
                out.push(Command::Text { rect: l.rect, text, style: ts, scale: l.scale });
            }
        }
        _ => {}
    }
    // Behaviours on this element style targets below it.
    let mut next: Vec<Pending<'_>> = pending.to_vec();
    for (path, conv) in &e.styled {
        next.push(Pending { path, conversion: conv, focused });
    }
    for (c, cl) in e.children.iter().zip(&l.children) {
        let mut target = None;
        let mut below = Vec::new();
        for p in &next {
            if p.path.first() == Some(&c.name) {
                if p.path.len() == 1 {
                    target = styles.converted(p.conversion, p.focused, true);
                } else {
                    below.push(Pending { path: &p.path[1..], ..p.clone() });
                }
            }
        }
        walk(c, cl, st, styles, alpha, focused, target, &below, out);
    }
}

/// Images styled by a text conversion take the text colour.
fn tint(look: &Look) -> Color {
    match (&look.text, &look.fill) {
        (Some(t), _) => t.color,
        (None, Some(Paint::Solid(c))) => *c,
        _ => WHITE,
    }
}

fn fade_paint(p: &Paint, a: f32) -> Paint {
    let f = |c: &Color| [c[0], c[1], c[2], c[3] * a];
    match p {
        Paint::Solid(c) => Paint::Solid(f(c)),
        Paint::Corners(cs) => Paint::Corners(cs.each_ref().map(f)),
        Paint::Linear { stops, angle } => Paint::Linear { stops: stops.iter().map(|(t, c)| (*t, f(c))).collect(), angle: *angle },
    }
}

/// Colour at `t` along gradient stops.
pub fn sample(stops: &[(f32, Color)], t: f32) -> Color {
    let Some(first) = stops.first() else { return [0.0; 4] };
    if t <= first.0 {
        return first.1;
    }
    for w in stops.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if t <= b {
            let k = if b > a { (t - a) / (b - a) } else { 1.0 };
            return [0, 1, 2, 3].map(|i| ca[i] + (cb[i] - ca[i]) * k);
        }
    }
    stops.last().unwrap().1
}

/// Corner colours of a linear gradient over a w×h box (angle in degrees,
/// measured from +x with y up).
pub fn corners(stops: &[(f32, Color)], angle: f32, w: f32, h: f32) -> [Color; 4] {
    let (s, c) = angle.to_radians().sin_cos();
    let dir = [c, -s]; // screen space, y down
    let pts = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
    let proj = pts.map(|p| p[0] * dir[0] + p[1] * dir[1]);
    let (lo, hi) = proj.iter().fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
    proj.map(|v| sample(stops, if hi > lo { (v - lo) / (hi - lo) } else { 0.0 }))
}
