//! UI elements built from RBF0 documents.

use rage::rbf::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Panel,
    Grid,
    Stack,
    Canvas,
    Viewbox,
    Image,
    Rect,
    Circle,
    Text,
    SubScreen,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HAlign {
    Stretch,
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VAlign {
    Stretch,
    Top,
    Center,
    Bottom,
}

/// A grid row or column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    Pixel(f32),
    Star(f32),
    Auto,
}

/// What accepting a focused element does (its `uiScreenSelectableBehavior`).
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Open a sub-screen of this screen.
    Subscreen(String),
    /// Leave the current sub-screen.
    ExitSubscreen,
    /// Follow a link of the hosting flow block (`to_map`, `to_settings`, ...).
    Flow(String),
}

#[derive(Clone, Debug)]
pub struct Element {
    pub kind: Kind,
    pub type_name: String,
    pub name: String,
    /// Fixed size when the content mode is RAW.
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min: [f32; 2],
    pub max: [f32; 2],
    /// left, top, right, bottom
    pub margin: [f32; 4],
    pub padding: [f32; 4],
    pub halign: HAlign,
    pub valign: VAlign,
    pub visible: bool,
    /// 0..1
    pub alpha: f32,
    pub style: Option<String>,
    pub color: Option<String>,
    pub text: Option<String>,
    pub texture: Option<(String, String)>,
    pub columns: Vec<Track>,
    pub rows: Vec<Track>,
    pub row: usize,
    pub column: usize,
    pub row_span: usize,
    pub column_span: usize,
    /// Canvas child position.
    pub left: f32,
    pub top: f32,
    pub horizontal: bool,
    pub flip_horizontal: bool,
    /// `uiBehaviorStyledState` behaviours: a path (child names from this
    /// element) and the style conversion that styles that target by state.
    pub styled: Vec<(Vec<String>, String)>,
    /// What Accept does on this element.
    pub accept: Option<Action>,
    pub children: Vec<Element>,
}

impl Element {
    pub fn from_node(n: &Node) -> Element {
        let type_name = n.type_name().unwrap_or(&n.name).to_string();
        let kind = match type_name.as_str() {
            "rage__fwuiPanel" | "UIScreen" | "UIMovieView" => Kind::Panel,
            "rage__fwuiGrid" => Kind::Grid,
            "rage__fwuiStackPanel" => Kind::Stack,
            "rage__fwuiCanvas" => Kind::Canvas,
            "rage__fwuiViewbox" => Kind::Viewbox,
            "UIImage" => Kind::Image,
            "UIRectangle" => Kind::Rect,
            "UICircle" => Kind::Circle,
            "UIText" => Kind::Text,
            "rage__fwuiSubScreen" => Kind::SubScreen,
            _ => Kind::Other,
        };
        let lp = n.child("LayoutParams");
        let f = |name: &str| lp.and_then(|l| l.f64_of(name)).map(|v| v as f32);
        let s = |name: &str| lp.and_then(|l| l.text_of(name)).map(|v| v.trim().to_string());
        let raw = |mode: Option<String>| mode.as_deref() == Some("CONTENT_MODE_RAW");
        let width = if raw(s("widthContentMode")) { f("width") } else { None };
        let height = if raw(s("heightContentMode")) { f("height") } else { None };
        let halign = match s("horizontalAlignment").as_deref() {
            Some("HALIGN_LEFT") => HAlign::Left,
            Some("HALIGN_CENTER") => HAlign::Center,
            Some("HALIGN_RIGHT") => HAlign::Right,
            _ => HAlign::Stretch,
        };
        let valign = match s("verticalAlignment").as_deref() {
            Some("VALIGN_TOP") => VAlign::Top,
            Some("VALIGN_CENTER") => VAlign::Center,
            Some("VALIGN_BOTTOM") => VAlign::Bottom,
            _ => VAlign::Stretch,
        };
        let margin = [f("leftMargin"), f("topMargin"), f("rightMargin"), f("bottomMargin")].map(|v| v.unwrap_or(0.0));
        let pad = lp.and_then(|l| l.child("padding"));
        let padding = match pad {
            Some(p) => ["left", "top", "right", "bottom"].map(|k| p.f64_of(k).unwrap_or(0.0) as f32),
            None => [0.0; 4],
        };
        let spec = n.child("SpecializedLayoutParams");
        let tracks = |name: &str| -> Vec<Track> {
            spec.and_then(|s| s.child(name))
                .map(|list| {
                    list.children_named("Item")
                        .map(|it| {
                            let size = it.f64_of("size").map(|v| v as f32);
                            match it.text_of("type").as_deref().map(str::trim) {
                                Some("SIZING_TYPE_PIXEL") => Track::Pixel(size.unwrap_or(0.0)),
                                Some("SIZING_TYPE_AUTO") => Track::Auto,
                                _ => Track::Star(size.unwrap_or(1.0)),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let dep = n.child("DependencyLayoutParams");
        let d = |name: &str| dep.and_then(|x| x.f64_of(name));
        let horizontal = spec.and_then(|s| s.text_of("orientation")).is_some_and(|o| o.contains("HORIZONTAL"));
        let texture = match (n.text_of("TextureDictionary"), n.text_of("Texture")) {
            (Some(d), Some(t)) => Some((d.trim().to_string(), t.trim().to_string())),
            _ => None,
        };
        let styled = n
            .child("BehaviorSet")
            .and_then(|b| b.child("Behaviors"))
            .map(|b| {
                b.children_named("Item")
                    .filter(|i| i.type_name() == Some("uiBehaviorStyledState"))
                    .filter_map(|i| {
                        let conv = i.child("StyleConversion")?.text_of("SourceID")?.trim().to_string();
                        let path = i.child("Context").and_then(|c| c.text_of("Path")).unwrap_or_default();
                        let path = path.trim().split('.').filter(|p| !p.is_empty()).map(String::from).collect();
                        Some((path, conv))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let accept = n.child("BehaviorSet").and_then(|b| b.child("Behaviors")).and_then(|b| {
            b.children_named("Item").find_map(|i| {
                if i.type_name() != Some("uiScreenSelectableBehavior") || i.bool_of("Enabled") == Some(false) {
                    return None;
                }
                if i.text_of("Message").is_some_and(|m| m.trim() != "rage__fwuiInputMessageAccept") {
                    return None;
                }
                let link = |n: &Node| n.text_of("SubscreenLink").map(|s| s.trim().to_string());
                if let Some(s) = link(i).or_else(|| i.child("GenericOp").and_then(link)) {
                    return Some(if s == "fwuiExitSubscreen" { Action::ExitSubscreen } else { Action::Subscreen(s) });
                }
                i.child("FlowOp").and_then(|f| f.text_of("Link")).map(|l| Action::Flow(l.trim().to_string()))
            })
        });
        let children = n.child("Children").map(|c| c.children_named("Item").map(Element::from_node).collect()).unwrap_or_default();
        Element {
            kind,
            type_name,
            name: n.text_of("Name").unwrap_or_default().trim().to_string(),
            width,
            height,
            min: [f("minWidth").unwrap_or(0.0), f("minHeight").or(f("minSize")).unwrap_or(0.0)],
            max: [f("maxWidth").unwrap_or(f32::INFINITY), f("maxHeight").unwrap_or(f32::INFINITY)],
            margin,
            padding,
            halign,
            valign,
            visible: n.bool_of("Visible").unwrap_or(true),
            alpha: n.f64_of("Alpha").map_or(1.0, |a| (a / 100.0) as f32),
            style: n.text_of("Style").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            color: n.text_of("Color").map(|s| s.trim().to_string()),
            text: n.text_of("Text").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            texture,
            columns: tracks("columns"),
            rows: tracks("rows"),
            row: d("row").unwrap_or(0.0) as usize,
            column: d("column").unwrap_or(0.0) as usize,
            row_span: d("rowSpan").unwrap_or(1.0).max(1.0) as usize,
            column_span: d("columnSpan").unwrap_or(1.0).max(1.0) as usize,
            left: d("Left").unwrap_or(0.0) as f32,
            top: d("Top").unwrap_or(0.0) as f32,
            horizontal,
            flip_horizontal: n.bool_of("FlipHorizontal").unwrap_or(false),
            styled,
            accept,
            children,
        }
    }

    pub fn find(&self, name: &str) -> Option<&Element> {
        if self.name == name {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(name))
    }

    pub fn find_mut(&mut self, name: &str) -> Option<&mut Element> {
        if self.name == name {
            return Some(self);
        }
        self.children.iter_mut().find_map(|c| c.find_mut(name))
    }

    pub fn walk_mut(&mut self, f: &mut impl FnMut(&mut Element)) {
        f(self);
        for c in &mut self.children {
            c.walk_mut(f);
        }
    }
}
