//! Lengths in the user's units: outline widths (in the outline unit, `Settings::outline_unit`)
//! and text sizes (in the text unit, `Settings::text_unit`). Values are always kept in points;
//! only what the fields show and read is in the unit. Every outline width and text size field,
//! list and summary formats and parses through here, so they all agree (#109, #127, #137, #138).
//!
//! An object has an outline when its outline has a color (`Outline::is_visible`); a width of 0
//! with a color is a hairline, never "no outline".

use crate::app::AnyVectorApp;
use vector_core::style::Outline;
use vector_core::Unit;

/// The unit and resolution outline widths are shown in.
pub fn outline_unit(app: &AnyVectorApp) -> (Unit, f64) {
    (app.settings.outline_unit, dpi(app))
}

/// The unit and resolution text sizes are shown in.
pub fn text_unit(app: &AnyVectorApp) -> (Unit, f64) {
    (app.settings.text_unit, dpi(app))
}

/// The document's resolution (pixels), else the default one.
fn dpi(app: &AnyVectorApp) -> f64 {
    app.doc().map(|d| d.doc.settings.dpi).unwrap_or(300.0)
}

// ----------------------------------------------------------------------------- outline width

/// The decimals an outline width shows in `u` (0.0025", 0.567 pt, 0.2 mm).
pub fn width_decimals(u: Unit) -> usize {
    match u {
        Unit::Feet | Unit::Yards | Unit::Meters => 6,
        Unit::Inches | Unit::Centimeters | Unit::Picas | Unit::Ciceros => 4,
        Unit::Millimeters | Unit::Points | Unit::Didots => 3,
        Unit::Pixels => 2,
    }
}

/// One click of a width field's arrows, in points: 0.01" in imperial units, 0.1 mm in metric
/// ones, else half a point.
pub fn width_step(u: Unit, dpi: f64) -> f64 {
    match u {
        Unit::Inches | Unit::Feet | Unit::Yards => Unit::Inches.from_unit(0.01, dpi),
        Unit::Millimeters | Unit::Centimeters | Unit::Meters => Unit::Millimeters.from_unit(0.1, dpi),
        _ => 0.5,
    }
}

/// An outline width in `u`: "0.5 pt", "0.01 \"". 0 is "Hairline".
pub fn width_text(w: f64, u: Unit, dpi: f64) -> String {
    if w <= 0.0 {
        "Hairline".to_string()
    } else {
        u.format_trim(w, dpi, width_decimals(u))
    }
}

/// What a field or a summary says about an outline: "None" without a color, "Hairline" for
/// a width of 0, else its width in `u`.
pub fn outline_text(ol: &Outline, u: Unit, dpi: f64) -> String {
    if ol.is_visible() {
        width_text(ol.width, u, dpi)
    } else {
        "None".to_string()
    }
}

/// A typed outline width: `Some(None)` for "None", `Some(Some(0.0))` for "Hairline" (in English
/// or in the interface language), else the width in points (the number in `u`, or in the unit
/// written after it). `None` when it can't be read.
pub fn parse_width(text: &str, u: Unit, dpi: f64) -> Option<Option<f64>> {
    let t = text.trim().to_lowercase();
    let is = |word: &str| t == word.to_lowercase() || t == crate::i18n::t(word).to_lowercase();
    if is("None") {
        return Some(None);
    }
    if is("Hairline") {
        return Some(Some(0.0));
    }
    Unit::parse_value(&t, u, dpi).map(|v| Some(v.max(0.0)))
}

/// The widths the outline width lists offer in `u`, in points, Hairline (0) first: round
/// values of the unit (0.01", 0.25 mm), so a pick is exactly the value it shows.
pub fn width_choices(u: Unit, dpi: f64) -> Vec<f64> {
    const POINTS: [f64; 14] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 8.0, 10.0, 12.0, 16.0, 24.0, 36.0];
    const INCHES: [f64; 14] = [0.001, 0.0025, 0.005, 0.0075, 0.01, 0.015, 0.02, 0.03, 0.05, 0.08, 0.1, 0.125, 0.25, 0.5];
    const MM: [f64; 14] = [0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.5, 0.8, 1.0, 1.5, 2.0, 3.0, 5.0, 10.0];
    let list: Vec<f64> = match u {
        Unit::Inches | Unit::Feet | Unit::Yards => INCHES.iter().map(|v| Unit::Inches.from_unit(*v, dpi)).collect(),
        Unit::Millimeters | Unit::Centimeters | Unit::Meters => MM.iter().map(|v| Unit::Millimeters.from_unit(*v, dpi)).collect(),
        Unit::Points | Unit::Picas | Unit::Ciceros | Unit::Didots | Unit::Pixels => POINTS.to_vec(),
    };
    std::iter::once(0.0).chain(list).collect()
}

// ----------------------------------------------------------------------------- text size

/// The decimals a text size shows in `u` (12 pt, 0.167", 4.23 mm).
pub fn size_decimals(u: Unit) -> usize {
    match u {
        Unit::Inches | Unit::Feet | Unit::Yards | Unit::Meters | Unit::Centimeters => 3,
        Unit::Pixels => 1,
        _ => 2,
    }
}

/// A text size (points) in `u`: "12 pt", "0.167 \"".
pub fn size_text(pt: f64, u: Unit, dpi: f64) -> String {
    u.format_trim(pt, dpi, size_decimals(u))
}

/// A typed text size: the number in `u`, or in the unit written after it; in points.
pub fn parse_size(text: &str, u: Unit, dpi: f64) -> Option<f64> {
    Unit::parse_value(text, u, dpi)
}

/// The common font sizes (points), listed in the text unit (12 pt is 0.167" in inches).
pub const FONT_SIZES: [f64; 17] = [6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 24.0, 36.0, 48.0, 72.0, 100.0, 150.0, 200.0];

// ----------------------------------------------------------------------------- widgets

/// A number field for a length kept in points, shown in `u` with its suffix; a value typed with
/// another unit ("6 mm", "1 pt", `0.25"`) is converted. `range` and `speed` are in points.
/// True when the value changed.
pub fn drag(ui: &mut egui::Ui, v: &mut f64, u: Unit, dpi: f64, range: std::ops::RangeInclusive<f64>, decimals: usize) -> bool {
    let k = u.points_per_unit(dpi).max(1e-12);
    let mut x = *v / k;
    let typed = std::cell::Cell::new(None);
    let parse = |s: &str| typed_value(s, u, dpi, k, &typed);
    let r = ui.add(egui::DragValue::new(&mut x).range(range.start() / k..=range.end() / k).speed(0.2 / k).max_decimals(decimals).suffix(format!(" {}", u.suffix())).custom_parser(parse));
    if r.changed() {
        *v = exact(x, k, decimals, typed.get()).clamp(*range.start(), *range.end());
        return true;
    }
    false
}

/// Parses a typed length for a field in `u` (`k` points a unit) and remembers it in points.
fn typed_value(s: &str, u: Unit, dpi: f64, k: f64, typed: &std::cell::Cell<Option<f64>>) -> Option<f64> {
    let pt = Unit::parse_value(s, u, dpi)?;
    typed.set(Some(pt));
    Some(pt / k)
}

/// The value of a field in points: what was typed, exactly, when the field only rounded it to
/// its decimals (24 pt typed in an inch field is 24 pt, not 0.333"), else `x` units.
fn exact(x: f64, k: f64, decimals: usize, typed: Option<f64>) -> f64 {
    match typed {
        Some(pt) if (pt / k - x).abs() <= 0.5 * 10f64.powi(-(decimals.min(12) as i32)) + 1e-12 => pt,
        _ => x * k,
    }
}

/// [`drag`] with the up and down arrows of the docker fields (`dockers::spin`), `step` points
/// a click, `w` wide.
pub fn spin(ui: &mut egui::Ui, v: &mut f64, u: Unit, dpi: f64, range: std::ops::RangeInclusive<f64>, step: f64, decimals: usize, w: f32) -> bool {
    let k = u.points_per_unit(dpi).max(1e-12);
    let mut x = *v / k;
    let typed = std::cell::Cell::new(None);
    let parse = |s: &str| typed_value(s, u, dpi, k, &typed);
    let suffix = format!(" {}", u.suffix());
    if crate::dockers::spin_parsed(ui, &mut x, range.start() / k..=range.end() / k, step / k, decimals, &suffix, w, Some(&parse)) {
        *v = exact(x, k, decimals, typed.get()).clamp(*range.start(), *range.end());
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hairline_is_an_outline_and_only_no_color_is_none() {
        let mut ol = Outline { width: 0.0, ..Outline::default() };
        assert!(ol.is_visible());
        assert_eq!(outline_text(&ol, Unit::Points, 300.0), "Hairline");
        assert_eq!(outline_text(&ol, Unit::Inches, 300.0), "Hairline");
        ol.width = 0.5;
        assert_eq!(outline_text(&ol, Unit::Points, 300.0), "0.5 pt");
        assert_eq!(outline_text(&ol, Unit::Inches, 300.0), "0.0069 \"");
        ol.color = None;
        assert_eq!(outline_text(&ol, Unit::Points, 300.0), "None");
        ol.width = 0.0;
        assert_eq!(outline_text(&ol, Unit::Points, 300.0), "None");
    }

    #[test]
    fn widths_parse_in_the_unit_or_with_their_own() {
        assert_eq!(parse_width("None", Unit::Inches, 300.0), Some(None));
        assert_eq!(parse_width(" hairline ", Unit::Inches, 300.0), Some(Some(0.0)));
        let pt = |s: &str, u: Unit| parse_width(s, u, 300.0).flatten().unwrap();
        assert!((pt("0.01", Unit::Inches) - 0.72).abs() < 1e-9);
        assert!((pt("0.25\"", Unit::Points) - 18.0).abs() < 1e-9);
        assert!((pt("6 mm", Unit::Inches) - 6.0 * 72.0 / 25.4).abs() < 1e-9);
        assert!((pt("1 pt", Unit::Millimeters) - 1.0).abs() < 1e-9);
        assert_eq!(parse_width("-2", Unit::Points, 300.0), Some(Some(0.0)));
        assert_eq!(parse_width("abc", Unit::Points, 300.0), None);
    }

    #[test]
    fn width_lists_follow_the_unit() {
        let names = |u: Unit| width_choices(u, 300.0).into_iter().map(|w| width_text(w, u, 300.0)).collect::<Vec<_>>();
        let inches = names(Unit::Inches);
        assert_eq!(inches[0], "Hairline");
        assert!(inches[1..].iter().all(|n| n.ends_with('"')), "{inches:?}");
        assert!(inches.contains(&"0.01 \"".to_string()));
        let mm = names(Unit::Millimeters);
        assert!(mm[1..].iter().all(|n| n.ends_with("mm")) && mm.contains(&"0.25 mm".to_string()), "{mm:?}");
        let pts = names(Unit::Points);
        assert!(pts[1..].iter().all(|n| n.ends_with("pt")) && pts.contains(&"0.5 pt".to_string()), "{pts:?}");
        // a pick is stored exactly: 0.01" is 0.72 pt and reads back as 0.01"
        let w = width_choices(Unit::Inches, 300.0).into_iter().find(|w| width_text(*w, Unit::Inches, 300.0) == "0.01 \"").unwrap();
        assert!((w - 0.72).abs() < 1e-12);
        assert_eq!(parse_width(&width_text(w, Unit::Inches, 300.0), Unit::Inches, 300.0), Some(Some(w)));
        // every listed width reads back as itself in its unit
        for u in Unit::ALL {
            for w in width_choices(u, 300.0).into_iter().skip(1) {
                let back = parse_width(&width_text(w, u, 300.0), u, 300.0).flatten().unwrap();
                assert!((back - w).abs() / w < 0.01, "{u:?} {w} -> {back}");
            }
        }
    }

    #[test]
    fn a_typed_length_is_kept_exactly() {
        let k = Unit::Inches.points_per_unit(300.0);
        // 24 pt typed in an inch field that rounds to 3 decimals (0.333")
        assert_eq!(exact(0.333, k, 3, Some(24.0)), 24.0);
        // a value dragged or stepped elsewhere is taken as shown
        assert!((exact(0.5, k, 3, Some(24.0)) - 36.0).abs() < 1e-9);
        assert!((exact(0.25, k, 3, None) - 18.0).abs() < 1e-9);
    }

    #[test]
    fn text_sizes_use_the_text_unit() {
        assert_eq!(size_text(12.0, Unit::Points, 300.0), "12 pt");
        assert_eq!(size_text(10.5, Unit::Points, 300.0), "10.5 pt");
        assert_eq!(size_text(12.0, Unit::Inches, 300.0), "0.167 \"");
        assert_eq!(size_text(72.0, Unit::Inches, 300.0), "1 \"");
        assert!((parse_size("0.25\"", Unit::Points, 300.0).unwrap() - 18.0).abs() < 1e-9);
        assert!((parse_size("0.25", Unit::Inches, 300.0).unwrap() - 18.0).abs() < 1e-9);
        assert!((parse_size("12 pt", Unit::Inches, 300.0).unwrap() - 12.0).abs() < 1e-9);
        assert!((parse_size("12", Unit::Points, 300.0).unwrap() - 12.0).abs() < 1e-9);
        for u in [Unit::Points, Unit::Millimeters, Unit::Centimeters, Unit::Inches] {
            let back = parse_size(&size_text(14.0, u, 300.0), u, 300.0).unwrap();
            assert!((back - 14.0).abs() < 0.05, "{u:?}: {back}");
        }
    }
}
