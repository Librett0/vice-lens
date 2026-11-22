//! The game's UI framework (fwui), reimplemented.
//!
//! Screens, prefabs and flow blocks come from the game data (RBF0). Their
//! behaviour (layout rules, states, data providers) was engine code; this crate
//! is Vice Lens's version of it.

pub mod draw;
pub mod element;
pub mod layout;
pub mod store;
pub mod style;

pub use draw::Command;
pub use element::{Action, Element, Kind};
pub use layout::Rect;
pub use style::{Look, Paint, Styles};
