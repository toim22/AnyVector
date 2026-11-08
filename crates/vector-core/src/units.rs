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
