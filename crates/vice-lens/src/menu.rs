//! The front menu: the game's pause menu screen (`SCRN_PauseMenu`), driven by
//! Vice Lens's stand-ins for the engine's data providers and focus behaviour.

use fwui::draw::{self, Command, State};
use fwui::layout::{self, Measure, Rect};
use fwui::style;
use fwui::style::TextStyle;
use fwui::{Action, Element, Kind};

use crate::assets::Game;
use crate::render::{CANVAS, measure};

/// One level of the menu: a container that is showing and its list.
#[derive(Clone)]
struct View {
    container: String,
    items: Vec<String>,
    focus: usize,
}

pub struct Menu {
    pristine: Element,
    root: Element,
    views: Vec<View>,
}

/// List items of the first stack panel under `e` that holds any.
fn items_in(e: &Element) -> Vec<String> {
    let mine: Vec<String> = e.children.iter().filter(|c| c.name.starts_with("PREFAB_List_Item_")).map(|c| c.name.clone()).collect();
    if e.kind == Kind::Stack && !mine.is_empty() {
        return mine;
    }
    e.children.iter().map(items_in).find(|v| !v.is_empty()).unwrap_or_default()
}

struct Ctx<'a> {
    game: &'a Game,
    focused: &'a str,
}

impl Ctx<'_> {
    fn resolve(&self, e: &Element) -> Option<String> {
        let label = e.text.as_deref()?;
        Some(self.game.text.get(label).unwrap_or(label).to_string())
    }
}

impl Ctx<'_> {
    fn text_style(&self, e: &Element) -> TextStyle {
        draw::style_of(e, None).and_then(|s| self.game.styles.look(s).text).unwrap_or_else(|| draw::fallback_text(&self.game.styles))
    }
}

impl Measure for Ctx<'_> {
    fn text(&self, e: &Element, _max_w: f32) -> [f32; 2] {
        let Some(s) = self.resolve(e) else { return [0.0, 0.0] };
        let st = self.text_style(e);
        let s = if st.upper { s.to_uppercase() } else { s };
        measure(self.game, &st, &s)
    }
}

impl State for Ctx<'_> {
    fn string(&self, e: &Element) -> Option<String> {
        self.resolve(e)
    }
    fn focused(&self, e: &Element) -> bool {
        e.name == self.focused
    }
    fn visible(&self, e: &Element, focused: bool) -> Option<bool> {
        match e.name.as_str() {
            "IMG_Icon_Focused" => Some(focused),
            "IMG_Icon" => Some(!focused),
            // Alerts are raised by data providers; nothing is pending at boot.
            "Cir_Alert" | "IMG_Alert_Halo" => Some(false),
            _ => None,
        }
    }
}

impl Menu {
    pub fn new(game: &Game) -> Option<Menu> {
        let mut root = Element::from_node(game.doc("SCRN_PauseMenu")?);
        style::apply_layout(&mut root);
        let open = root.find("PAN_Menu_Open")?;
        let view = View { container: open.name.clone(), items: items_in(open), focus: 0 };
        Some(Menu { pristine: root.clone(), root, views: vec![view] })
    }

    fn view(&self) -> &View {
        self.views.last().unwrap()
    }

    pub fn items(&self) -> &[String] {
        &self.view().items
    }

    pub fn set_focus(&mut self, i: usize) {
        let v = self.views.last_mut().unwrap();
        v.focus = i.min(v.items.len().saturating_sub(1));
    }

    pub fn move_focus(&mut self, d: isize) {
        let v = self.views.last_mut().unwrap();
        if !v.items.is_empty() {
            v.focus = (v.focus as isize + d).rem_euclid(v.items.len() as isize) as usize;
        }
    }

    /// Accept on the focused item. Returns what Vice Lens could not do yet.
    pub fn accept(&mut self) -> Option<String> {
        let name = self.view().items.get(self.view().focus)?.clone();
        let action = self.root.find(&name)?.accept.clone()?;
        match action {
            Action::Subscreen(sub) => {
                let e = self.pristine.find(&sub)?;
                let view = View { container: sub.clone(), items: items_in(e), focus: 0 };
                self.views.push(view);
                self.rebuild();
                None
            }
            Action::ExitSubscreen => {
                self.back();
                None
            }
            Action::Flow(link) => Some(format!("flow link {link} (the app it opens is not in Vice Lens yet)")),
        }
    }

    /// Back out of a sub-screen; false at the top level.
    pub fn back(&mut self) -> bool {
        if self.views.len() < 2 {
            return false;
        }
        self.views.pop();
        self.rebuild();
        true
    }

    /// Show only the innermost view's container: the engine shows a
    /// sub-screen in place of the panel it was opened from.
    fn rebuild(&mut self) {
        self.root = self.pristine.clone();
        let active = self.view().container.clone();
        for v in &self.views[..self.views.len() - 1] {
            if let Some(e) = self.root.find_mut(&v.container) {
                e.visible = false;
            }
        }
        if let Some(e) = self.root.find_mut(&active) {
            if e.kind == Kind::SubScreen {
                e.kind = Kind::Panel;
            }
            e.visible = true;
        }
    }

    pub fn commands(&self, game: &Game) -> Vec<Command> {
        let mut out = backdrop();
        let v = self.view();
        let focused = v.items.get(v.focus).map(String::as_str).unwrap_or("");
        let ctx = Ctx { game, focused };
        let laid = layout::layout(&self.root, Rect::new(0.0, 0.0, CANVAS[0], CANVAS[1]), &ctx);
        draw::draw(&self.root, &laid, &ctx, &game.styles, &mut out);
        out
    }

    /// Element names and rectangles, for checking layout.
    pub fn dump(&self, game: &Game) -> String {
        let ctx = Ctx { game, focused: "" };
        let laid = layout::layout(&self.root, Rect::new(0.0, 0.0, CANVAS[0], CANVAS[1]), &ctx);
        let mut s = String::new();
        fn walk(e: &Element, l: &layout::Laid, d: usize, s: &mut String) {
            if e.kind == Kind::SubScreen || !e.visible {
                return;
            }
            let r = l.rect;
            s.push_str(&format!("{}{:?} {} [{:.0},{:.0} {:.0}x{:.0}] x{:.2}\n", "  ".repeat(d), e.kind, e.name, r.x, r.y, r.w, r.h, l.scale));
            for (c, cl) in e.children.iter().zip(&l.children) {
                walk(c, cl, d + 1, s);
            }
        }
        walk(&self.root, &laid, 0, &mut s);
        s
    }
}

/// Until the loading-screen movies play, the menu sits on two layers of the
/// first loading screen: its sky, and its skyline along the bottom. The
/// arrangement is Vice Lens's.
fn backdrop() -> Vec<Command> {
    let img = |name: &str, rect: Rect| Command::Image { rect, dict: "loading_screen_00".into(), name: name.into(), tint: fwui::style::WHITE, flip: false };
    vec![img("loading_screen_00_i9", Rect::new(0.0, 0.0, CANVAS[0], CANVAS[1])), img("loading_screen_00_ie", Rect::new(0.0, CANVAS[1] - 1920.0, 3840.0, 1920.0))]
}
