//! The bitmap effects of the Effects menu: what each effect's page in the Properties docker holds
//! (the table is generated into `fxdata.rs`), and the starting settings of a new effect.

use vector_core::object::{BitmapEffect, FxValue};
use vector_core::Color;

pub use crate::fxdata::FX;

/// The colors of a slider's track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Grad {
    None,
    /// The hues of the color wheel.
    Hue,
    Two([u8; 3], [u8; 3]),
    Three([u8; 3], [u8; 3], [u8; 3]),
}

/// One control of an effect's page. A `key` with `{Name}` in it names a setting per value of the
/// setting `Name` (the channel shown, the color range shown...).
#[derive(Debug)]
pub enum Ctl {
    /// The label above, the slider and the value field with arrows at its right.
    Slider { key: &'static str, label: &'static str, min: f64, max: f64, step: f64, dec: u8, log: bool, def: f64, unit: &'static str, grad: Grad },
    Spin { key: &'static str, label: &'static str, min: f64, max: f64, step: f64, dec: u8, def: f64, unit: &'static str },
    /// The direction dial and its field (`quarter`: 0 to 90 degrees).
    Angle { key: &'static str, label: &'static str, def: f64, quarter: bool },
    Radio { key: &'static str, label: &'static str, opts: &'static [&'static str], def: usize },
    /// Icon buttons of which one is down.
    Icons { key: &'static str, label: &'static str, icons: &'static [(&'static str, &'static str)], def: usize },
    Check { key: &'static str, label: &'static str, def: bool },
    Combo { key: &'static str, label: &'static str, opts: &'static [&'static str], def: usize, inline: bool },
    /// A color button (`def` None: no color yet) with the eyedropper beside it.
    Color { key: &'static str, label: &'static str, def: Option<[u8; 3]>, dropper: bool },
    /// A check box with a color swatch (or an icon) after it.
    Toggle { key: &'static str, def: bool, swatch: [u8; 3], icon: &'static str },
    /// The tool that picks the center of the effect on the object (CenterX, CenterY).
    Center { x: f64, y: f64 },
    /// The lock that keeps the sliders before and after it equal.
    Lock { key: &'static str, def: bool },
    /// The Randomize button and the number of the random pattern.
    Seed { key: &'static str, label: &'static str },
    /// Tune pages: each thumbnail adds its step to the effect (the "Ops" list).
    Thumbs { ops: &'static [&'static str] },
    /// Thumbnails of which one is chosen.
    Styles { key: &'static str, opts: &'static [&'static str], def: usize },
    /// Color swatches choosing which color range the sliders below change (`None`: all colors).
    Swatches { key: &'static str, label: &'static str, opts: &'static [(&'static str, Option<[u8; 3]>)], def: usize, split: usize },
    Heading(&'static str),
    /// The effect drawn on a small sample picture.
    Preview,
    Presets { opts: &'static [&'static str] },
    /// A drawn texture chosen from a list.
    Texture { key: &'static str, label: &'static str },
    /// A curve editor (Tone Curve, Shear); `hist`: over the histogram.
    Curve { key: &'static str, hist: bool },
    /// The histogram of the object (`markers`: with the threshold markers).
    Histogram { markers: bool },
    /// The histogram with the input and output levels.
    Levels,
    /// The 5 × 5 matrix of User Defined.
    Kernel,
    /// The mesh of Mesh Warp.
    Mesh,
    /// The box of 3D Rotate.
    Rotate3d,
    /// The color wheel of Replace Colors.
    Wheel,
    /// The frame list of Frame.
    Frames,
    /// Shown while `key` has the value `val`.
    When { key: &'static str, val: f64, ctls: &'static [Ctl] },
    /// Enabled when `key` is on (`!key`: off; `key=v`: has the value v; `Never`).
    Enable { key: &'static str, ctls: &'static [Ctl] },
    /// Pages chosen by icon buttons.
    Tabs { key: &'static str, tabs: &'static [(&'static str, &'static str, &'static [Ctl])] },
    Row(&'static [Ctl]),
}

pub struct FxDef {
    pub id: &'static str,
    /// The Effects menu item.
    pub guid: &'static str,
    pub name: &'static str,
    /// Starting values of settings that depend on another one.
    pub init: &'static [(&'static str, f64)],
    pub ctls: &'static [Ctl],
}

impl FxDef {
    /// A new effect with the reference's starting settings.
    pub fn create(&self) -> BitmapEffect {
        let mut e = BitmapEffect::new(self.id);
        defaults(self.ctls, &mut e);
        for (k, v) in self.init {
            e.set(k, FxValue::Num(*v));
        }
        e
    }
}

fn defaults(ctls: &[Ctl], e: &mut BitmapEffect) {
    let num = |e: &mut BitmapEffect, k: &str, v: f64| {
        if !k.contains('{') {
            e.set(k, FxValue::Num(v));
        }
    };
    for c in ctls {
        match c {
            Ctl::Slider { key, def, .. } | Ctl::Spin { key, def, .. } | Ctl::Angle { key, def, .. } => num(e, key, *def),
            Ctl::Radio { key, def, .. } | Ctl::Icons { key, def, .. } | Ctl::Combo { key, def, .. } | Ctl::Styles { key, def, .. } | Ctl::Swatches { key, def, .. } => num(e, key, *def as f64),
            Ctl::Check { key, def, .. } | Ctl::Toggle { key, def, .. } | Ctl::Lock { key, def } => num(e, key, if *def { 1.0 } else { 0.0 }),
            Ctl::Color { key, def: Some(c), .. } => e.set(key, FxValue::Color(Color::rgb(c[0], c[1], c[2]))),
            Ctl::Center { x, y } => {
                num(e, "CenterX", *x);
                num(e, "CenterY", *y);
            }
            Ctl::Seed { key, .. } => num(e, key, 1.0),
            Ctl::Texture { key, .. } => num(e, key, 0.0),
            Ctl::Mesh => num(e, "Gridlines", 4.0),
            Ctl::When { ctls, .. } | Ctl::Enable { ctls, .. } | Ctl::Row(ctls) => defaults(ctls, e),
            Ctl::Tabs { key, tabs } => {
                num(e, key, 0.0);
                for (_, _, c) in tabs.iter() {
                    defaults(c, e);
                }
            }
            _ => {}
        }
    }
}

/// The effect with this id.
pub fn by_id(id: &str) -> Option<&'static FxDef> {
    FX.iter().find(|f| f.id == id)
}

/// The effect of an Effects menu item (by the first 8 characters of its id, any case).
pub fn by_guid(g: &str) -> Option<(usize, &'static FxDef)> {
    let k = g.get(..8).unwrap_or(g).to_ascii_lowercase();
    FX.iter().enumerate().find(|(_, f)| f.guid.get(..8) == Some(k.as_str()))
}

/// The name an effect shows in the list (the old Glow effect has no menu item).
pub fn name(e: &BitmapEffect) -> &str {
    match by_id(&e.id) {
        Some(f) => f.name,
        None if e.id == "Glow" => "Glow",
        None => &e.id,
    }
}

/// A key with its `{Name}` parts replaced by the values of those settings.
pub fn resolve(e: &BitmapEffect, key: &str) -> String {
    let mut out = String::new();
    let mut rest = key;
    while let Some(a) = rest.find('{') {
        let Some(b) = rest[a..].find('}') else { break };
        out.push_str(&rest[..a]);
        let name = &rest[a + 1..a + b];
        out.push_str(&(e.num(name, 0.0).round() as i64).to_string());
        rest = &rest[a + b + 1..];
    }
    out.push_str(rest);
    out
}

/// Whether an [`Ctl::Enable`] condition holds.
pub fn enabled(e: &BitmapEffect, cond: &str) -> bool {
    if cond == "Never" {
        return false;
    }
    if let Some(k) = cond.strip_prefix('!') {
        return !e.flag(k, false);
    }
    if let Some((k, v)) = cond.split_once('=') {
        return (e.num(k, 0.0) - v.parse::<f64>().unwrap_or(0.0)).abs() < 1e-9;
    }
    match e.params.get(cond) {
        Some(FxValue::Color(_)) => true,
        _ => e.flag(cond, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_effect_has_a_page_and_runs() {
        assert_eq!(FX.len(), 122);
        let mut ids = std::collections::HashSet::new();
        for f in FX {
            assert!(ids.insert(f.id), "{} twice", f.id);
            assert_eq!(f.guid.len(), 36, "{}", f.id);
            let e = f.create();
            let mut pm = tiny_skia::Pixmap::new(40, 30).unwrap();
            pm.fill(tiny_skia::Color::from_rgba8(70, 130, 200, 255));
            let cx = vector_render::bitfx::FxCtx { s: 1.0, frame: [0.0, 0.0, 40.0, 30.0], seed: 7 };
            vector_render::bitfx::apply(&e, &mut pm, &cx);
        }
    }

    #[test]
    fn placeholders_follow_the_shown_channel() {
        let f = by_id("HueSaturationLightness").unwrap();
        let mut e = f.create();
        assert_eq!(resolve(&e, "Hue{Channel}"), "Hue0");
        e.set("Channel", FxValue::Num(3.0));
        assert_eq!(resolve(&e, "Hue{Channel}"), "Hue3");
        let cm = by_id("ChannelMixer").unwrap().create();
        assert_eq!(cm.num("RedChannel0", 0.0), 100.0);
    }

    #[test]
    fn defaults_are_the_reference_values() {
        let g = by_id("GaussianBlur").unwrap().create();
        assert_eq!(g.num("Radius", 0.0), 5.0);
        let b = by_id("Grayscale").unwrap().create();
        assert_eq!(b.num("Magentas", 0.0), 80.0);
        assert!(by_guid("A0F20985").is_some());
    }
}
