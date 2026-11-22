//! Looks: what a style resolves to (fill and text), from the game's style
//! store plus two tables the data does not have:
//!
//! - the RGB of each named colour (the game asked its engine colour store,
//!   `fwuiColorStore.GetRGB`, for these; the data only names them and gives
//!   an alpha), and
//! - which face each `$fontN` alias means (the engine mapped them).
//!
//! Both tables are Vice Lens's choices, made from the names and from where the
//! data uses each alias; everything else (which colour, alpha, font alias,
//! size, tracking, gradient stops and angle) is the game's.

use rage::joaat;

use crate::element::{Element, Kind, Track};
use crate::store::{Fill, StyleStore};

pub type Color = [f32; 4];

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub font: &'static str,
    /// Pixels on the 3840×2160 canvas.
    pub size: f32,
    pub color: Color,
    pub upper: bool,
    /// Extra advance per glyph, in thousandths of an em.
    pub tracking: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid(Color),
    /// Corner colours: top-left, top-right, bottom-right, bottom-left.
    Corners([Color; 4]),
    /// A linear gradient across the shape (angle in degrees, y up).
    Linear {
        stops: Vec<(f32, Color)>,
        angle: f32,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Look {
    pub fill: Option<Paint>,
    pub text: Option<TextStyle>,
}

const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0]
}

pub const WHITE: Color = rgba(255, 255, 255, 255);

/// Vice Lens's RGB for the game's named colours.
const PALETTE: &[(&str, Color)] = &[
    ("COLOR_TEXT_LIGHT_EMPHASIS_XHIGH", rgba(255, 255, 255, 255)),
    ("COLOR_TEXT_LIGHT_EMPHASIS_HIGH", rgba(255, 255, 255, 222)),
    ("COLOR_TEXT_LIGHT_EMPHASIS_MEDIUM", rgba(255, 255, 255, 166)),
    ("COLOR_TEXT_LIGHT_EMPHASIS_LOW", rgba(255, 255, 255, 128)),
    ("COLOR_TEXT_DARK_EMPHASIS_XHIGH", rgba(8, 8, 12, 255)),
    ("COLOR_TEXT_DARK_EMPHASIS_HIGH", rgba(8, 8, 12, 222)),
    ("COLOR_TEXT_DARK_EMPHASIS_MEDIUM", rgba(8, 8, 12, 166)),
    ("COLOR_TEXT_DARK_EMPHASIS_LOW", rgba(8, 8, 12, 128)),
    ("COLOR_PURE_WHITE", rgba(255, 255, 255, 255)),
    ("COLOR_WHITE", rgba(250, 250, 250, 255)),
    ("COLOR_BLACK", rgba(0, 0, 0, 255)),
    ("COLOR_BACKGROUND_DARK", rgba(10, 10, 14, 255)),
    ("COLOR_BACKGROUND_LIGHT", rgba(244, 244, 246, 255)),
    ("COLOR_STROKE_DARK", rgba(0, 0, 0, 160)),
    ("COLOR_STROKE_LIGHT", rgba(255, 255, 255, 64)),
    ("COLOR_EFFECT_OUTLINE", rgba(0, 0, 0, 200)),
    ("COLOR_VALUE_NEUTRAL", rgba(255, 255, 255, 255)),
    ("COLOR_VALUE_WARNING", rgba(255, 196, 64, 255)),
    ("COLOR_VALUE_NEGATIVE", rgba(232, 72, 72, 255)),
    ("COLOR_STATE_FOCUS", rgba(255, 255, 255, 255)),
    ("COLOR_STATE_DISABLED", rgba(255, 255, 255, 96)),
    ("COLOR_STATE_WARNING", rgba(255, 196, 64, 255)),
    ("COLOR_THEME_ACCENT_PRIMARY", rgba(255, 140, 180, 255)),
    ("COLOR_THEME_ACCENT_PRIMARY_DISABLED", rgba(255, 140, 180, 110)),
    ("COLOR_THEME_ACCENT_SECONDARY", rgba(120, 200, 255, 255)),
    ("COLOR_RED_BRIGHT", rgba(240, 60, 60, 255)),
    ("COLOR_GRAY_XDARK", rgba(28, 28, 32, 255)),
    ("COLOR_GRAY_XLIGHT", rgba(222, 222, 226, 255)),
];

/// Colours the data uses that no known name hashes to; looked at in context.
const UNNAMED: &[(u32, Color)] = &[
    // The pause menu background gradient's colour.
    (0x52c9_ac9c, rgba(6, 6, 10, 255)),
];

pub fn color(hash: u32) -> Color {
    PALETTE.iter().find(|(n, _)| joaat(n) == hash).map(|(_, c)| *c).or_else(|| UNNAMED.iter().find(|(h, _)| *h == hash).map(|(_, c)| *c)).unwrap_or(rgba(40, 40, 48, 255))
}

pub fn color_named(name: &str) -> Color {
    color(joaat(name))
}

/// Vice Lens's reading of the `$fontN` aliases, from the styles that use them.
const FACES: &[(&str, &str)] = &[
    ("$font1", "gta_art_deco_regular"),
    ("$font3", "gta_art_deco_bold"),
    // Headers, tabs, central HUD messages.
    ("$font11", "gta_art_deco_condensed_heavy"),
    // Body and menu text, subtitles, descriptions.
    ("$font12", "gta_art_deco_regular"),
    // The bold of body text (MENU_TEXT_BOLD_*, objectives).
    ("$font13", "gta_art_deco_bold"),
    // App titles and large values.
    ("$font14", "gta_art_deco_condensed_bold"),
    // Phone OS text, notifications, the radar clock.
    ("$font15", "robin_pro_regular"),
    // Button prompt labels.
    ("$font16", "gta_art_deco_bold"),
    ("$font17", "robin_pro_bold"),
    // Timers, ammo counts: numerals that must not jitter.
    ("$font18", "gta_art_deco_condensed_bold_monospaced"),
    // Cash and prices.
    ("$font19", "gta_art_deco_currency_condensed_bold"),
    // The arcade game's own faces.
    ("$font20", "machine_pro"),
    ("$font21", "futura_book"),
    ("$font22", "bookman_oldstyle_bold"),
    ("$font23", "rage_italic"),
    ("$font25", "gta_art_deco_condensed_heavy"),
    // Credits.
    ("$font26", "pricedown"),
    ("$font28", "gta_art_deco_condensed_bold"),
    ("$font29", "futura_book"),
    ("$font30", "gta_art_deco_condensed_heavy"),
];

pub fn face(alias: u32) -> &'static str {
    FACES.iter().find(|(a, _)| joaat(a) == alias).map(|(_, f)| *f).unwrap_or("gta_art_deco_regular")
}

/// The style store and the reader's settings.
#[derive(Default)]
pub struct Styles {
    pub store: StyleStore,
    /// Text size level: 1 normal, 3 and 4 larger.
    pub level: u32,
}

/// Which of a conversion's four styles applies.
pub fn conversion_slot(focused: bool, enabled: bool) -> usize {
    match (focused, enabled) {
        (true, false) => 0,
        (true, true) => 1,
        (false, false) => 2,
        (false, true) => 3,
    }
}

impl Styles {
    pub fn new(store: StyleStore) -> Styles {
        Styles { store, level: 1 }
    }

    pub fn look(&self, style: u32) -> Look {
        let Some(st) = self.store.state(style, self.level) else { return Look::default() };
        // Slot 0 of a brush is used; the other two are variants Vice Lens has
        // not identified yet.
        let fill = st.brush.as_ref().and_then(|b| b[0].as_ref()).map(paint);
        let text = st.text.as_ref().map(|t| TextStyle {
            font: face(t.font),
            size: t.size,
            color: match &fill {
                Some(Paint::Solid(c)) => *c,
                _ => WHITE,
            },
            upper: t.flags[3] == 1,
            tracking: t.tracking,
        });
        Look { fill: if text.is_some() { None } else { fill }, text }
    }

    pub fn look_named(&self, name: &str) -> Look {
        self.look(joaat(name))
    }

    /// Style from a conversion (`common_controls+...`) for an item state.
    pub fn converted(&self, conversion: &str, focused: bool, enabled: bool) -> Option<u32> {
        let c = self.store.conversions.get(&joaat(conversion))?;
        Some(c[conversion_slot(focused, enabled)])
    }
}

fn paint(f: &Fill) -> Paint {
    match f {
        Fill::Solid(h, a) => {
            let c = color(*h);
            Paint::Solid([c[0], c[1], c[2], c[3] * a])
        }
        // The value at +0x20 is not applied: as an opacity it would leave a
        // focused item's light background too faint for its dark text.
        Fill::Gradient { stops, angle, .. } => Paint::Linear {
            stops: stops
                .iter()
                .map(|(p, h, a)| {
                    let c = color(*h);
                    (*p, [c[0], c[1], c[2], c[3] * a])
                })
                .collect(),
            angle: *angle,
        },
    }
}

/// Layout that styles carry. Some styles set layout parameters (e.g.
/// `MENU_LISTITEM_LAYOUT_PARAMS_AUTO_STAR`); Vice Lens does not read those yet.
/// Stand-in: a blade grid's first column hugs its content.
pub fn apply_layout(e: &mut Element) {
    e.walk_mut(&mut |e| {
        if e.kind == Kind::Grid && e.columns.is_empty() && e.style.as_deref() == Some("MENU_BLADE") {
            e.columns = vec![Track::Auto, Track::Star(1.0)];
        }
    });
}
