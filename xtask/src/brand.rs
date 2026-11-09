//! Brand assets drawn from the two SVGs in `crates/vector-app/assets/brand/`:
//!
//! - `vector-icon.svg` (the airship on its rounded dark tile) is the program's icon: the exe and
//!   window icon, shortcuts, file associations and the installer's icon.
//! - `vector-logo.svg` (the airship alone) is the logo: splash screen, About box, Welcome screen
//!   and the installer's side picture.

use crate::{root, Result};
use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::{ExtendedColorType, RgbaImage};
use resvg::{tiny_skia, usvg};
use std::path::Path;

/// Sizes stored in the .ico (Windows picks the closest one for each place it shows the icon).
const ICO_SIZES: [u32; 10] = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];
/// Display scales the installer has a picture for (Setup picks the one that fits the screen).
const SCALES: [u32; 6] = [100, 125, 150, 175, 200, 250];

fn options() -> usvg::Options<'static> {
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    opt
}

/// Renders an SVG to straight-alpha RGBA at exactly `w`×`h` pixels.
fn render(svg: &str, w: u32, h: u32, opt: &usvg::Options) -> Result<RgbaImage> {
    let tree = usvg::Tree::from_str(svg, opt)?;
    let size = tree.size();
    let mut pix = tiny_skia::Pixmap::new(w, h).ok_or("empty picture")?;
    let ts = tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, ts, &mut pix.as_mut());
    let mut img = RgbaImage::new(w, h);
    for (px, out) in pix.pixels().iter().zip(img.pixels_mut()) {
        let c = px.demultiply();
        *out = image::Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(img)
}

/// The `viewBox` of an SVG document and its markup without the outer `<svg>` element.
fn split_svg(svg: &str) -> Result<(String, String)> {
    let start = svg.find("<svg").ok_or("no <svg> element")?;
    let open_end = start + svg[start..].find('>').ok_or("unterminated <svg>")?;
    let open = &svg[start..=open_end];
    let vb = open.split("viewBox=\"").nth(1).and_then(|s| s.split('"').next()).ok_or("no viewBox")?.to_string();
    let close = svg.rfind("</svg>").ok_or("no </svg>")?;
    Ok((vb, svg[open_end + 1..close].to_string()))
}

/// The logo placed at (`x`, `y`) with width `w`, as a nested `<svg>` (keeps its own coordinates).
fn nested_logo(logo: &str, x: f32, y: f32, w: f32) -> Result<String> {
    let (vb, inner) = split_svg(logo)?;
    let v: Vec<f32> = vb.split_whitespace().filter_map(|n| n.parse().ok()).collect();
    let h = w * v[3] / v[2];
    Ok(format!(r#"<svg x="{x}" y="{y}" width="{w}" height="{h}" viewBox="{vb}" overflow="visible">{inner}</svg>"#))
}

/// The installer's side picture (164×314 at 100%): the logo, the name and a short line on the
/// icon's dark tile (or a light version for Windows' light mode).
fn wizard_large(logo: &str, dark: bool) -> Result<String> {
    let (top, mid, bottom, name, line) = if dark {
        ("#2b3242", "#151a26", "#0b0d16", "#ffffff", "#9aa3b2")
    } else {
        ("#f7f9fc", "#eaeef5", "#d9e0ea", "#151a26", "#5a6474")
    };
    let glow = if dark { 1.0 } else { 0.55 };
    let logo = nested_logo(logo, 14.0, 96.0, 136.0)?;
    Ok(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 164 314" width="164" height="314">
  <defs>
    <linearGradient id="wz_bg" x1="0" y1="0" x2="0.25" y2="1">
      <stop offset="0" stop-color="{top}"/><stop offset="0.4" stop-color="{mid}"/><stop offset="1" stop-color="{bottom}"/>
    </linearGradient>
    <radialGradient id="wz_blue" cx="96" cy="112" r="120" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#0a9cff" stop-opacity="{b}"/><stop offset="1" stop-color="#0a9cff" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="wz_orange" cx="150" cy="300" r="120" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#ff6a00" stop-opacity="{o}"/><stop offset="1" stop-color="#ff6a00" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="wz_red" cx="10" cy="300" r="110" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#d0175e" stop-opacity="{r}"/><stop offset="1" stop-color="#d0175e" stop-opacity="0"/>
    </radialGradient>
    <filter id="wz_shadow" x="-20%" y="-20%" width="140%" height="160%">
      <feDropShadow dx="0" dy="4" stdDeviation="5" flood-color="#000000" flood-opacity="{s}"/>
    </filter>
  </defs>
  <rect width="164" height="314" fill="url(#wz_bg)"/>
  <rect width="164" height="314" fill="url(#wz_blue)"/>
  <rect width="164" height="314" fill="url(#wz_orange)"/>
  <rect width="164" height="314" fill="url(#wz_red)"/>
  <g filter="url(#wz_shadow)">{logo}</g>
  <text x="82" y="204" text-anchor="middle" font-family="Segoe UI" font-weight="600" font-size="26" fill="{name}">AnyVector</text>
  <text x="82" y="224" text-anchor="middle" font-family="Segoe UI" font-size="10.5" fill="{line}">Vector graphics editor</text>
</svg>"##,
        b = 0.45 * glow,
        o = 0.40 * glow,
        r = 0.35 * glow,
        s = if dark { 0.55 } else { 0.25 },
    ))
}

fn save_png(img: &RgbaImage, path: &Path) -> Result<()> {
    img.save(path)?;
    println!("  {}", path.strip_prefix(root()).unwrap_or(path).display());
    Ok(())
}

pub fn run() -> Result<()> {
    let root = root();
    let assets = root.join("crates/vector-app/assets");
    let installer = root.join("installer");
    let icon = std::fs::read_to_string(assets.join("brand/vector-icon.svg"))?;
    let logo = std::fs::read_to_string(assets.join("brand/vector-logo.svg"))?;
    let opt = options();

    println!("program icon:");
    let mut frames = vec![];
    for n in ICO_SIZES {
        let img = render(&icon, n, n, &opt)?;
        frames.push(IcoFrame::as_png(img.as_raw(), n, n, ExtendedColorType::Rgba8)?);
    }
    let ico = assets.join("vector.ico");
    IcoEncoder::new(std::fs::File::create(&ico)?).encode_images(&frames)?;
    println!("  {} ({} sizes)", ico.strip_prefix(&root).unwrap_or(&ico).display(), frames.len());
    save_png(&render(&icon, 256, 256, &opt)?, &assets.join("icon-256.png"))?;

    println!("logo:");
    let (vb, _) = split_svg(&logo)?;
    let v: Vec<f32> = vb.split_whitespace().filter_map(|n| n.parse().ok()).collect();
    let w = 1400;
    save_png(&render(&logo, w, (w as f32 * v[3] / v[2]).round() as u32, &opt)?, &assets.join("logo.png"))?;

    println!("installer pictures:");
    for old in std::fs::read_dir(&installer)? {
        let p = old?.path();
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name.starts_with("Wizard") || name.starts_with("wizard-") {
            std::fs::remove_file(&p)?;
        }
    }
    let light = wizard_large(&logo, false)?;
    let dark = wizard_large(&logo, true)?;
    for sc in SCALES {
        let (w, h) = ((164 * sc + 50) / 100, (314 * sc + 50) / 100);
        save_png(&render(&light, w, h, &opt)?, &installer.join(format!("WizardLarge-{sc}.png")))?;
        save_png(&render(&dark, w, h, &opt)?, &installer.join(format!("WizardLarge-dark-{sc}.png")))?;
        let n = (55 * sc + 50) / 100;
        save_png(&render(&icon, n, n, &opt)?, &installer.join(format!("WizardSmall-{sc}.png")))?;
    }
    Ok(())
}
