//! AnyVector — document model, geometry and text engine.

pub mod blend_path;
pub mod cms;
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
pub mod sysfonts;
pub mod text;
pub mod units;

pub use color::{Color, NamedColor, Palette};
pub use document::{Document, Guide, GuideKind, Layer, Page};
pub use kurbo;
pub use object::{ObjId, ObjKind, Object};
pub use style::{Fill, Outline, Style, Transparency};
pub use units::Unit;

pub const APP_NAME: &str = "AnyVector";
/// The version on this system; each system has its own (Cargo.toml, build.rs).
pub const APP_VERSION: &str = env!("ANYVECTOR_VERSION");
/// The system this AnyVector is for, in words (issues, release notes).
pub const APP_SYSTEM: &str = if cfg!(windows) {
    "Windows"
} else if cfg!(target_os = "linux") {
    "Linux"
} else if cfg!(target_os = "macos") {
    "macOS"
} else {
    std::env::consts::OS
};
pub const FILE_EXT: &str = "vct";
