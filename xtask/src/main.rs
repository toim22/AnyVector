//! Build helpers for Vector, run from the repository root:
//!
//! ```text
//! cargo run -p xtask -- brand      # app icon, logo and installer pictures from the brand SVGs
//! cargo run -p xtask -- coverage   # docs/COVERAGE.md from the CorelDRAW reference (MAPEAMENTO)
//! cargo run -p xtask -- compare    # target/visual/compare.html: Vector renders next to the reference
//! cargo run -p xtask -- licenses   # the Legal Notices text (open-source components and licenses)
//! cargo run -p xtask -- ui         # menus, toolbars and property bars from the reference definition
//! ```
//!
//! The reference folder defaults to `C:\Users\anton\Documents\Claude\corel-screenshots\MAPEAMENTO`
//! and can be changed with the `VECTOR_REFERENCE` environment variable.

mod brand;
mod compare;
mod coverage;
mod licenses;
mod ui;

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The repository root (the parent of this crate).
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives in the repository").to_path_buf()
}

/// The CorelDRAW reference material (screenshots, inventory and coverage reports).
pub fn reference() -> PathBuf {
    std::env::var_os("VECTOR_REFERENCE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Users\anton\Documents\Claude\corel-screenshots\MAPEAMENTO"))
}

fn main() {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    let result = match cmd.as_str() {
        "brand" => brand::run(),
        "coverage" => coverage::run(),
        "compare" => compare::run(),
        "licenses" => licenses::run(),
        "ui" => ui::run(),
        _ => {
            eprintln!("usage: cargo run -p xtask -- <brand|coverage|compare|licenses|ui>");
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
