//! AnyVector — document model, geometry and text engine.

pub mod codes;
pub mod color;
pub mod document;
pub mod fontsub;
pub mod geom;
pub mod history;
pub mod object;
pub mod ops;
pub mod path;
pub mod style;
pub mod text;
pub mod units;

pub use color::{Color, NamedColor, Palette};
pub use document::{Document, Guide, GuideKind, Layer, Page};
pub use kurbo;
pub use object::{ObjId, ObjKind, Object};
pub use style::{Fill, Outline, Style, Transparency};
pub use units::Unit;

pub const APP_NAME: &str = "AnyVector";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const FILE_EXT: &str = "vct";
