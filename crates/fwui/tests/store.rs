//! The game's style store, read from the real files when present.

use std::path::PathBuf;

use fwui::Styles;
use fwui::store::StyleStore;
use rage::Archive;
use rage::pso::Pso;

fn file_at(a: &Archive, offset: u64) -> Vec<u8> {
    let mut streams = Vec::new();
    a.scan(|s, _| {
        if s.offset >= offset {
            streams.push(s);
        }
        streams.len() < 256
    });
    a.read_file(&Archive::files(&streams)[0]).unwrap()
}

#[test]
fn menu_styles_resolve() {
    // The game files are the user's own: skipped without VICE_LENS_GAME.
    let Some(dir) = std::env::var_os("VICE_LENS_GAME").map(PathBuf::from) else { return };
    let Ok(a) = Archive::open(dir.join("data.rpf")) else { return };
    let styles = Pso::parse(file_at(&a, 0x1c33f000)).unwrap();
    let conversions = Pso::parse(file_at(&a, 0x1c330000)).unwrap();
    let s = Styles::new(StyleStore::load(&styles, &conversions));
    // Menu entries: $font12 at 52 px (normal text size), tracking 9.
    let t = s.look_named("MENU_TEXT_NEUTRAL_ENABLED").text.unwrap();
    assert_eq!((t.size, t.tracking), (52.0, 9.0));
    assert_eq!(t.font, "gta_art_deco_regular");
    // The header: $font11 at 96, upper case, tracking -21.
    let h = s.look_named("MENU_HEADER").text.unwrap();
    assert_eq!((h.size, h.upper, h.tracking), (96.0, true, -21.0));
    // A list item's label: enabled uses MENU_TEXT_NEUTRAL_ENABLED.
    let c = s.converted("common_controls+list_item_text_body_conversion", false, true).unwrap();
    assert_eq!(c, rage::joaat("MENU_TEXT_NEUTRAL_ENABLED"));
    // The pause menu background is a gradient at -79 degrees.
    match s.look_named("BACKGROUND_PAUSE_MENU").fill.unwrap() {
        fwui::Paint::Linear { angle, stops } => {
            assert_eq!(angle, -79.0);
            assert_eq!(stops.len(), 2);
        }
        other => panic!("{other:?}"),
    }
}
