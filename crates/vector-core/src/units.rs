//! Measurement units. The document's internal unit is the PostScript point (1/72 inch).

use serde::{Deserialize, Serialize};

pub const PT_PER_INCH: f64 = 72.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Unit {
    Inches,
    Millimeters,
    Centimeters,
    Meters,
    Points,
    Picas,
    Pixels,
    Feet,
    Yards,
    Ciceros,
    Didots,
}

impl Default for Unit {
    fn default() -> Self {
        Unit::Inches
    }
}

impl Unit {
    pub const ALL: [Unit; 11] = [
        Unit::Inches,
        Unit::Millimeters,
        Unit::Centimeters,
        Unit::Meters,
        Unit::Points,
        Unit::Picas,
        Unit::Pixels,
        Unit::Feet,
        Unit::Yards,
        Unit::Ciceros,
        Unit::Didots,
    ];

    /// Number of points in one of this unit (pixels use the document resolution, 96 dpi default).
    pub fn points_per_unit(self, dpi: f64) -> f64 {
        match self {
            Unit::Inches => 72.0,
            Unit::Millimeters => 72.0 / 25.4,
            Unit::Centimeters => 72.0 / 2.54,
            Unit::Meters => 72.0 / 0.0254,
            Unit::Points => 1.0,
            Unit::Picas => 12.0,
            Unit::Pixels => 72.0 / dpi.max(1.0),
            Unit::Feet => 72.0 * 12.0,
            Unit::Yards => 72.0 * 36.0,
            Unit::Ciceros => 12.7872,
            Unit::Didots => 1.0656,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Unit::Inches => "inches",
            Unit::Millimeters => "millimeters",
            Unit::Centimeters => "centimeters",
            Unit::Meters => "meters",
            Unit::Points => "points",
            Unit::Picas => "picas, points",
            Unit::Pixels => "pixels",
            Unit::Feet => "feet",
            Unit::Yards => "yards",
            Unit::Ciceros => "ciceros",
            Unit::Didots => "didots",
        }
    }

    /// Short suffix used inside numeric fields (`0.25 "`).
    pub fn suffix(self) -> &'static str {
        match self {
            Unit::Inches => "\"",
            Unit::Millimeters => "mm",
            Unit::Centimeters => "cm",
            Unit::Meters => "m",
            Unit::Points => "pt",
            Unit::Picas => "pc",
            Unit::Pixels => "px",
            Unit::Feet => "ft",
            Unit::Yards => "yd",
            Unit::Ciceros => "c",
            Unit::Didots => "didot",
        }
    }

    pub fn decimals(self) -> usize {
        match self {
            Unit::Inches | Unit::Feet | Unit::Yards | Unit::Meters => 3,
            Unit::Millimeters | Unit::Centimeters => 2,
            Unit::Points | Unit::Picas | Unit::Ciceros | Unit::Didots => 1,
            Unit::Pixels => 0,
        }
    }

    pub fn to_unit(self, pt: f64, dpi: f64) -> f64 {
        pt / self.points_per_unit(dpi)
    }

    pub fn from_unit(self, v: f64, dpi: f64) -> f64 {
        v * self.points_per_unit(dpi)
    }

    /// `pt` in this unit with at most `decimals` decimals and no trailing zeros, then the suffix:
    /// "12 pt", "0.167 \"", "0.2 mm". The form every length field and summary shows.
    pub fn format_trim(self, pt: f64, dpi: f64, decimals: usize) -> String {
        format!("{} {}", trim_number(self.to_unit(pt, dpi), decimals), self.suffix())
    }

    /// The ruler of this unit at a zoom (screen px per point): the major step, its subdivisions,
    /// and whether the labels are inch fractions. Inches are divided in halves, quarters, eighths
    /// and so on (1/2", 1/4" ... 1/64"), as CorelDRAW's imperial rulers are; the other units in
    /// tenths, fifths and halves.
    pub fn ruler(self, px_per_pt: f64, dpi: f64) -> RulerStep {
        if self != Unit::Inches {
            let (step, subdiv) = self.ruler_step(px_per_pt, dpi);
            return RulerStep { step, subdiv, fraction: false, binary: false };
        }
        let ppu = self.points_per_unit(dpi) * px_per_pt; // pixels per inch
        // the labels stay at least 60 px apart; the ticks at least 5 px
        let binary = [1.0 / 64.0, 1.0 / 32.0, 1.0 / 16.0, 1.0 / 8.0, 1.0 / 4.0, 1.0 / 2.0, 1.0];
        let whole = [2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0];
        let step = binary.iter().chain(whole.iter()).copied().find(|c| c * ppu >= 60.0).unwrap_or(20000.0);
        // the subdivisions: binary under a whole inch, decimal-like over it
        let subdivs: &[u32] = if step <= 1.0 {
            &[16, 8, 4, 2]
        } else {
            match leading_digit(step) {
                2 => &[4, 2],
                5 => &[5],
                _ => &[10, 5, 2],
            }
        };
        let subdiv = subdivs.iter().copied().find(|n| step * ppu / *n as f64 >= 5.0).unwrap_or(*subdivs.last().unwrap());
        RulerStep { step, subdiv, fraction: step < 1.0, binary: step <= 1.0 }
    }

    /// Ruler tick spacing (major ticks) in this unit for a given zoom (screen px per point).
    pub fn ruler_step(self, px_per_pt: f64, dpi: f64) -> (f64, u32) {
        let ppu = self.points_per_unit(dpi) * px_per_pt; // pixels per unit
        let candidates = [
            0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0,
            200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0, 50000.0,
        ];
        for c in candidates {
            if c * ppu >= 60.0 {
                let subdiv = if (c * ppu) > 160.0 { 10 } else { 5 };
                return (c, subdiv);
            }
        }
        (100000.0, 10)
    }

    /// Parse a user-entered value. Accepts an explicit unit suffix (`2 mm`, `1.5"`, `12pt`).
    pub fn parse_value(text: &str, default_unit: Unit, dpi: f64) -> Option<f64> {
        let t = text.trim().replace(',', ".");
        if t.is_empty() {
            return None;
        }
        let mut num_end = 0;
        for (i, ch) in t.char_indices() {
            if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' || ch == 'e' && i > 0 {
                num_end = i + ch.len_utf8();
            } else {
                break;
            }
        }
        let num: f64 = t[..num_end].trim().parse().ok()?;
        let rest = t[num_end..].trim().to_lowercase();
        let unit = match rest.as_str() {
            "" => default_unit,
            "\"" | "in" | "inch" | "inches" => Unit::Inches,
            "mm" => Unit::Millimeters,
            "cm" => Unit::Centimeters,
            "m" => Unit::Meters,
            "pt" | "points" => Unit::Points,
            "pc" | "picas" => Unit::Picas,
            "px" | "pixels" => Unit::Pixels,
            "ft" | "'" => Unit::Feet,
            "yd" => Unit::Yards,
            _ => default_unit,
        };
        Some(unit.from_unit(num, dpi))
    }

    pub fn format(self, pt: f64, dpi: f64) -> String {
        let v = self.to_unit(pt, dpi);
        let d = self.decimals();
        let mut s = format!("{:.*}", d, v);
        if s.contains('.') {
            while s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.push('0');
            }
        }
        if s == "-0.0" {
            s = "0.0".into();
        }
        format!("{} {}", s, self.suffix())
    }
}

/// `v` with at most `decimals` decimals, without trailing zeros ("12", "0.5", "-0.25").
pub fn trim_number(v: f64, decimals: usize) -> String {
    let mut s = format!("{:.*}", decimals, v);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

/// The first digit of a positive number (2 for 20, 5 for 0.05).
fn leading_digit(v: f64) -> u32 {
    let e = v.log10().floor();
    (v / 10f64.powf(e)).round() as u32
}

/// The divisions of a ruler: labelled major ticks `step` units apart, cut in `subdiv` minor
/// steps; `fraction`: the labels are inch fractions ("1/4", "1 1/2").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RulerStep {
    pub step: f64,
    pub subdiv: u32,
    pub fraction: bool,
    /// The minor ticks are halves, quarters, eighths... and get shorter the finer they are.
    pub binary: bool,
}

impl RulerStep {
    /// The label of the major tick at `v` units from the origin.
    pub fn label(&self, v: f64) -> String {
        let v = if v.abs() < 1e-9 { 0.0 } else { v };
        if self.fraction {
            let denom = (1.0 / self.step).round().max(1.0) as u32;
            return format_fraction(v, denom);
        }
        let decimals = if self.step < 1.0 { ((-self.step.log10()).ceil() as usize).min(3) } else { 0 };
        format!("{:.*}", decimals, v)
    }

    /// How long the tick `k` minor steps from the origin is: 0 a major (labelled) tick, then 1
    /// for the halves, 2 for the quarters (binary steps) or the other minor ticks, and so on.
    pub fn tick_level(&self, k: i64) -> u32 {
        let n = self.subdiv.max(1) as i64;
        let m = k.rem_euclid(n);
        if m == 0 {
            return 0;
        }
        if self.binary && n.count_ones() == 1 {
            // halves 1, quarters 2, eighths 3...
            return n.trailing_zeros() - m.trailing_zeros();
        }
        if n % 2 == 0 && m == n / 2 {
            1
        } else {
            2
        }
    }
}

/// `v` as a whole number and a binary fraction of `denom` (a power of two), reduced: "0",
/// "1/2", "3/4", "1 1/4", "-2 1/8". `v` is rounded to the nearest 1/`denom`.
pub fn format_fraction(v: f64, denom: u32) -> String {
    let denom = denom.max(1) as i64;
    let n = (v * denom as f64).round() as i64;
    let sign = if n < 0 { "-" } else { "" };
    let n = n.abs();
    let (whole, mut num, mut den) = (n / denom, n % denom, denom);
    if num == 0 {
        return format!("{sign}{whole}");
    }
    while num % 2 == 0 && den % 2 == 0 {
        num /= 2;
        den /= 2;
    }
    if whole == 0 {
        format!("{sign}{num}/{den}")
    } else {
        format!("{sign}{whole} {num}/{den}")
    }
}

/// The unit and page size of a new install from the Windows regional settings: `measure` is
/// the locale's measurement system (LOCALE_IMEASURE: 0 metric, 1 U.S.), `paper` its paper size
/// (LOCALE_IPAPERSIZE: 1 Letter, 5 Legal, 8 A3, 9 A4). Metric gives millimeters, U.S.
/// inches; an unknown paper falls back to A4 (metric) or Letter (U.S.).
pub fn regional_defaults(measure: u32, paper: u32) -> (Unit, &'static str) {
    let metric = measure == 0;
    let unit = if metric { Unit::Millimeters } else { Unit::Inches };
    let page = match paper {
        1 => "Letter",
        5 => "Legal",
        8 => "A3",
        9 => "A4",
        _ if metric => "A4",
        _ => "Letter",
    };
    (unit, page)
}

/// Standard page sizes (width x height in points, portrait).
pub fn page_presets() -> Vec<(&'static str, f64, f64)> {
    let mm = 72.0 / 25.4;
    vec![
        ("Letter", 612.0, 792.0),
        ("Legal", 612.0, 1008.0),
        ("Tabloid", 792.0, 1224.0),
        ("Executive", 522.0, 756.0),
        ("Statement", 396.0, 612.0),
        ("A0", 841.0 * mm, 1189.0 * mm),
        ("A1", 594.0 * mm, 841.0 * mm),
        ("A2", 420.0 * mm, 594.0 * mm),
        ("A3", 297.0 * mm, 420.0 * mm),
        ("A4", 210.0 * mm, 297.0 * mm),
        ("A5", 148.0 * mm, 210.0 * mm),
        ("A6", 105.0 * mm, 148.0 * mm),
        ("B4 (ISO)", 250.0 * mm, 353.0 * mm),
        ("B5 (ISO)", 176.0 * mm, 250.0 * mm),
        ("Envelope #10", 297.0, 684.0),
        ("Envelope DL", 110.0 * mm, 220.0 * mm),
        ("Business Card", 252.0, 144.0),
        ("Postcard", 288.0, 432.0),
        ("Web 1920 x 1080", 1440.0, 810.0),
        ("Web 1280 x 720", 960.0, 540.0),
        ("Instagram Post", 810.0, 810.0),
        ("Custom", 612.0, 792.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_trim_their_zeros_and_parse_any_suffix() {
        assert_eq!(Unit::Points.format_trim(12.0, 300.0, 2), "12 pt");
        assert_eq!(Unit::Points.format_trim(0.5669, 300.0, 3), "0.567 pt");
        assert_eq!(Unit::Inches.format_trim(12.0, 300.0, 3), "0.167 \"");
        assert_eq!(Unit::Inches.format_trim(0.72, 300.0, 4), "0.01 \"");
        assert_eq!(Unit::Millimeters.format_trim(0.5669, 300.0, 3), "0.2 mm");
        assert_eq!(Unit::Points.format_trim(-0.0001, 300.0, 2), "0 pt");
        // typed values: the field's unit, or the one written after the number
        assert!((Unit::parse_value("0.25\"", Unit::Points, 300.0).unwrap() - 18.0).abs() < 1e-9);
        assert!((Unit::parse_value("6 mm", Unit::Inches, 300.0).unwrap() - 6.0 * 72.0 / 25.4).abs() < 1e-9);
        assert!((Unit::parse_value("1 pt", Unit::Inches, 300.0).unwrap() - 1.0).abs() < 1e-9);
        assert!((Unit::parse_value("0.01", Unit::Inches, 300.0).unwrap() - 0.72).abs() < 1e-9);
        // a value shown in one unit reads back in another without drift
        for u in Unit::ALL {
            let pt = 0.72;
            let back = u.from_unit(u.to_unit(pt, 300.0), 300.0);
            assert!((back - pt).abs() < 1e-9, "{u:?}");
        }
    }

    #[test]
    fn inch_rulers_divide_into_binary_fractions() {
        let r = |px_per_pt: f64| Unit::Inches.ruler(px_per_pt, 300.0);
        // the page at about 100%: whole inches in sixteenths
        let a = r(96.0 / 72.0);
        assert_eq!((a.step, a.subdiv, a.fraction, a.binary), (1.0, 16, false, true));
        // zoomed in: halves, quarters, eighths and sixteenths are labelled as fractions
        for (zoom, step) in [(2.0, 0.5), (4.0, 0.25), (8.0, 0.125), (16.0, 0.0625)] {
            let s = r(96.0 / 72.0 * zoom);
            assert_eq!(s.step, step, "zoom {zoom}");
            assert!(s.fraction && s.binary && s.subdiv.is_power_of_two() && s.subdiv >= 2);
            assert!(s.step * 96.0 * zoom >= 60.0);
        }
        // zoomed out: whole inches several at a time
        let s = r(96.0 / 72.0 / 4.0);
        assert!(s.step >= 2.0 && !s.fraction && !s.binary);
        // labels
        let q = RulerStep { step: 0.25, subdiv: 4, fraction: true, binary: true };
        let labels: Vec<String> = [0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 2.5, -0.25, -1.75].iter().map(|v| q.label(*v)).collect();
        assert_eq!(labels, ["0", "1/4", "1/2", "3/4", "1", "1 1/4", "2 1/2", "-1/4", "-1 3/4"]);
        assert_eq!(format_fraction(3.0 / 16.0, 16), "3/16");
        assert_eq!(format_fraction(0.999, 8), "1");
        // the ticks get shorter at each halving
        let s = RulerStep { step: 1.0, subdiv: 16, fraction: false, binary: false };
        let f = RulerStep { binary: true, ..s };
        assert_eq!([0, 8, 4, 12, 2, 1, 15, 16].map(|k| f.tick_level(k)), [0, 1, 2, 2, 3, 4, 4, 0]);
        assert_eq!(s.tick_level(8), 1);
        assert_eq!(s.tick_level(3), 2);
    }

    #[test]
    fn metric_rulers_are_unchanged() {
        for z in [0.1, 0.5, 1.0, 2.0, 8.0, 40.0] {
            for u in [Unit::Millimeters, Unit::Centimeters, Unit::Points, Unit::Pixels, Unit::Feet] {
                let (step, subdiv) = u.ruler_step(z, 300.0);
                assert_eq!(u.ruler(z, 300.0), RulerStep { step, subdiv, fraction: false, binary: false });
            }
        }
        let m = Unit::Millimeters.ruler(1.0, 300.0);
        assert_eq!(m.label(10.0), "10");
        assert_eq!(RulerStep { step: 0.5, subdiv: 5, fraction: false, binary: false }.label(1.5), "1.5");
        assert_eq!(RulerStep { step: 0.5, subdiv: 5, fraction: false, binary: false }.label(-0.0), "0.0");
        assert_eq!(RulerStep { step: 0.25, subdiv: 4, fraction: true, binary: true }.label(-0.0), "0");
    }

    #[test]
    fn regional_settings_choose_the_unit_and_page() {
        assert_eq!(regional_defaults(0, 9), (Unit::Millimeters, "A4"));
        assert_eq!(regional_defaults(1, 1), (Unit::Inches, "Letter"));
        assert_eq!(regional_defaults(1, 5), (Unit::Inches, "Legal"));
        assert_eq!(regional_defaults(0, 0), (Unit::Millimeters, "A4"));
        assert_eq!(regional_defaults(1, 0), (Unit::Inches, "Letter"));
        assert!(page_presets().iter().any(|p| p.0 == "A3"));
    }
}
