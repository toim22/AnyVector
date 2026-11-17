//! Colors, color models and palettes.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
    /// The CMYK inks (each 0..100) of a color defined in CMYK, kept exactly as entered; `r`,
    /// `g` and `b` are then its RGB equivalent through the document's color profiles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmyk: Option<[u8; 4]>,
}

impl Default for Color {
    fn default() -> Self {
        Color::BLACK
    }
}

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const RED: Color = Color::rgb(237, 27, 35);
    pub const BLUE: Color = Color::rgb(46, 48, 146);
    pub const TRANSPARENT: Color = Color { r: 0, g: 0, b: 0, a: 0, cmyk: None };

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255, cmyk: None }
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a, cmyk: None }
    }

    /// The color with its CMYK inks (each 0..100) recorded, its RGB unchanged.
    pub fn with_cmyk(self, cmyk: [f64; 4]) -> Color {
        Color { cmyk: Some(cmyk.map(|v| v.round().clamp(0.0, 100.0) as u8)), ..self }
    }

    /// The same RGB color without its CMYK definition.
    pub fn plain(self) -> Color {
        Color { cmyk: None, ..self }
    }

    /// Defined in CMYK.
    pub fn is_cmyk(&self) -> bool {
        self.cmyk.is_some()
    }

    pub fn from_hex(s: &str) -> Option<Color> {
        let s = s.trim().trim_start_matches('#');
        let v = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        match s.len() {
            6 => Some(Color::rgb(v(0)?, v(2)?, v(4)?)),
            8 => Some(Color::rgba(v(0)?, v(2)?, v(4)?, v(6)?)),
            3 => {
                let c = |i: usize| u8::from_str_radix(s.get(i..i + 1)?, 16).ok().map(|x| x * 17);
                Some(Color::rgb(c(0)?, c(1)?, c(2)?))
            }
            _ => None,
        }
    }

    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }

    pub fn with_alpha(self, a: u8) -> Color {
        Color { a, ..self }
    }

    pub fn to_f32(&self) -> [f32; 4] {
        [self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0, self.a as f32 / 255.0]
    }

    pub fn lerp(a: Color, b: Color, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0);
        let l = |x: u8, y: u8| ((x as f64) + (y as f64 - x as f64) * t).round().clamp(0.0, 255.0) as u8;
        let mut c = Color::rgba(l(a.r, b.r), l(a.g, b.g), l(a.b, b.b), l(a.a, b.a));
        // between two CMYK colors: the inks blend too
        if let (Some(x), Some(y)) = (a.cmyk, b.cmyk) {
            c.cmyk = Some([0, 1, 2, 3].map(|i| l(x[i], y[i])));
        }
        c
    }

    /// CMYK, values 0..100: the inks of a color defined in CMYK, otherwise the RGB converted
    /// with the color profiles in force (see [`crate::cms`]).
    pub fn to_cmyk(&self) -> [f64; 4] {
        match self.cmyk {
            Some(k) => k.map(|v| v as f64),
            None => crate::cms::current().rgb_to_cmyk(*self),
        }
    }

    /// A color defined in CMYK (inks 0..100): the inks are kept, and the RGB is their
    /// equivalent with the color profiles in force.
    pub fn from_cmyk(c: f64, m: f64, y: f64, k: f64) -> Color {
        let inks = [c, m, y, k].map(|v| v.clamp(0.0, 100.0));
        crate::cms::current().cmyk_to_rgb(inks).with_cmyk(inks)
    }

    /// Plain (device) CMYK of the RGB, values 0..100, without color management.
    pub fn to_cmyk_uncalibrated(&self) -> [f64; 4] {
        crate::cms::naive_rgb_to_cmyk(*self)
    }

    /// HSB with h in 0..360, s/b in 0..100.
    pub fn to_hsb(&self) -> [f64; 3] {
        let r = self.r as f64 / 255.0;
        let g = self.g as f64 / 255.0;
        let b = self.b as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let h = if d == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / d).rem_euclid(6.0))
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        let s = if max == 0.0 { 0.0 } else { d / max };
        [h, s * 100.0, max * 100.0]
    }

    pub fn from_hsb(h: f64, s: f64, v: f64) -> Color {
        let s = (s / 100.0).clamp(0.0, 1.0);
        let v = (v / 100.0).clamp(0.0, 1.0);
        let h = h.rem_euclid(360.0) / 60.0;
        let c = v * s;
        let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
        let (r, g, b) = match h as i32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = v - c;
        let f = |x: f64| ((x + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        Color::rgb(f(r), f(g), f(b))
    }

    pub fn luminance(&self) -> f64 {
        0.2126 * self.r as f64 + 0.7152 * self.g as f64 + 0.0722 * self.b as f64
    }

    /// Approximate color name for tooltips and the status bar.
    pub fn describe(&self) -> String {
        let [c, m, y, k] = self.to_cmyk();
        format!("C:{:.0} M:{:.0} Y:{:.0} K:{:.0}", c, m, y, k)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedColor {
    pub name: String,
    pub color: Color,
    #[serde(default)]
    pub spot: bool,
}

impl NamedColor {
    pub fn new(name: impl Into<String>, color: Color) -> Self {
        NamedColor { name: name.into(), color, spot: false }
    }

    /// The spot color print-and-cut workflows use for the cut line: 100% magenta, named
    /// "CutContour" (the name RIP software looks for).
    pub fn cut_contour() -> Self {
        NamedColor { name: CUT_CONTOUR_NAME.into(), color: CUT_CONTOUR, spot: true }
    }
}

pub const CUT_CONTOUR_NAME: &str = "CutContour";
/// The screen color of the CutContour spot color (100% magenta as CorelDRAW shows it). Writers
/// that know spot colors recognize CutContour by this value.
pub const CUT_CONTOUR: Color = Color::rgb(236, 0, 140);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub name: String,
    pub colors: Vec<NamedColor>,
}

impl Palette {
    pub fn from_hex_list(name: &str, list: &[(&str, &str)]) -> Palette {
        Palette {
            name: name.to_string(),
            colors: list
                .iter()
                .filter_map(|(n, h)| Color::from_hex(h).map(|c| NamedColor::new(*n, c)))
                .collect(),
        }
    }

    /// The default palette: CorelDRAW 2026's CMYK Palette (defcmyk.xml), its colors in the same
    /// order and defined by the same CMYK inks, so they are shown through the color management in
    /// force (bright with color management off).
    pub fn default_palette() -> Palette {
        const INKS: &[(&str, [u8; 4])] = &[
            ("Black", [0, 0, 0, 100]),
            ("90% Black", [0, 0, 0, 90]),
            ("80% Black", [0, 0, 0, 80]),
            ("70% Black", [0, 0, 0, 70]),
            ("60% Black", [0, 0, 0, 60]),
            ("50% Black", [0, 0, 0, 50]),
            ("40% Black", [0, 0, 0, 40]),
            ("30% Black", [0, 0, 0, 30]),
            ("20% Black", [0, 0, 0, 20]),
            ("10% Black", [0, 0, 0, 10]),
            ("White", [0, 0, 0, 0]),
            ("Blue", [100, 100, 0, 0]),
            ("Cyan", [100, 0, 0, 0]),
            ("Green", [100, 0, 100, 0]),
            ("Yellow", [0, 0, 100, 0]),
            ("Red", [0, 100, 100, 0]),
            ("Magenta", [0, 100, 0, 0]),
            ("Purple", [20, 80, 0, 20]),
            ("Orange", [0, 60, 100, 0]),
            ("Pink", [0, 40, 20, 0]),
            ("Dark Brown", [0, 20, 20, 60]),
            ("Powder Blue", [20, 20, 0, 0]),
            ("Pastel Blue", [40, 40, 0, 0]),
            ("Baby Blue", [60, 40, 0, 0]),
            ("Electric Blue", [60, 60, 0, 0]),
            ("Twilight Blue", [40, 40, 0, 20]),
            ("Navy Blue", [60, 40, 0, 40]),
            ("Deep Navy Blue", [40, 40, 0, 60]),
            ("Desert Blue", [40, 20, 0, 40]),
            ("Sky Blue", [100, 20, 0, 0]),
            ("Ice Blue", [40, 0, 0, 0]),
            ("Light BlueGreen", [20, 0, 0, 20]),
            ("Ocean Green", [20, 0, 0, 40]),
            ("Moss Green", [20, 0, 0, 60]),
            ("Dark Green", [20, 0, 0, 80]),
            ("Forest Green", [40, 0, 20, 60]),
            ("Grass Green", [60, 0, 40, 40]),
            ("Kentucky Green", [40, 0, 20, 40]),
            ("Light Green", [60, 0, 40, 20]),
            ("Spring Green", [60, 0, 60, 20]),
            ("Turquoise", [60, 0, 20, 0]),
            ("Sea Green", [60, 0, 20, 20]),
            ("Faded Green", [20, 0, 20, 20]),
            ("Ghost Green", [20, 0, 20, 0]),
            ("Mint Green", [40, 0, 40, 0]),
            ("Army Green", [20, 0, 20, 40]),
            ("Avocado Green", [20, 0, 40, 40]),
            ("Martian Green", [20, 0, 60, 20]),
            ("Dull Green", [20, 0, 40, 20]),
            ("Chartreuse", [40, 0, 100, 0]),
            ("Moon Green", [20, 0, 60, 0]),
            ("Murky Green", [0, 0, 20, 80]),
            ("Olive Drab", [0, 0, 20, 60]),
            ("Khaki", [0, 0, 20, 40]),
            ("Olive", [0, 0, 40, 40]),
            ("Banana Yellow", [0, 0, 60, 20]),
            ("Light Yellow", [0, 0, 60, 0]),
            ("Chalk", [0, 0, 40, 0]),
            ("Pale Yellow", [0, 0, 20, 0]),
            ("Brown", [0, 20, 40, 40]),
            ("Red Brown", [0, 40, 60, 20]),
            ("Gold", [0, 20, 60, 20]),
            ("Autumn Orange", [0, 60, 80, 0]),
            ("Light Orange", [0, 40, 80, 0]),
            ("Peach", [0, 40, 60, 0]),
            ("Deep Yellow", [0, 20, 100, 0]),
            ("Sand", [0, 20, 40, 0]),
            ("Walnut", [0, 20, 40, 60]),
            ("Ruby Red", [0, 60, 60, 40]),
            ("Brick Red", [0, 60, 80, 20]),
            ("Tropical Pink", [0, 60, 60, 0]),
            ("Soft Pink", [0, 40, 40, 0]),
            ("Faded Pink", [0, 20, 20, 0]),
            ("Crimson", [0, 40, 20, 40]),
            ("Regal Red", [0, 60, 40, 20]),
            ("Deep Rose", [0, 60, 20, 20]),
            ("Neon Red", [0, 100, 60, 0]),
            ("Deep Pink", [0, 60, 40, 0]),
            ("Hot Pink", [0, 80, 40, 0]),
            ("Dusty Rose", [0, 40, 20, 20]),
            ("Plum", [0, 40, 0, 60]),
            ("Deep Violet", [0, 60, 0, 40]),
            ("Light Violet", [0, 40, 0, 0]),
            ("Violet", [0, 40, 0, 20]),
            ("Dusty Plum", [0, 20, 0, 40]),
            ("Pale Purple", [0, 20, 0, 20]),
            ("Majestic Purple", [20, 60, 0, 20]),
            ("Neon Purple", [20, 80, 0, 0]),
            ("Light Purple", [20, 60, 0, 0]),
            ("Twilight Violet", [20, 40, 0, 20]),
            ("Easter Purple", [20, 40, 0, 0]),
            ("Deep Purple", [20, 40, 0, 60]),
            ("Grape", [20, 40, 0, 40]),
            ("Blue Violet", [40, 60, 0, 0]),
            ("Blue Purple", [40, 100, 0, 0]),
            ("Deep River", [40, 80, 0, 20]),
            ("Deep Azure", [60, 80, 0, 0]),
            ("Storm Blue", [40, 60, 0, 40]),
            ("Deep Blue", [60, 80, 0, 20]),("C100 M100 Y100 K100", [100, 100, 100, 100]),
        ];
        Palette {
            name: "Default Palette".to_string(),
            colors: INKS.iter().map(|(n, k)| NamedColor::new(*n, Color::from_cmyk(k[0] as f64, k[1] as f64, k[2] as f64, k[3] as f64))).collect(),
        }
    }

    pub fn grayscale() -> Palette {
        let mut p = Palette { name: "Grayscale".into(), colors: vec![] };
        for i in 0..=20 {
            let v = 255 - (i * 255 / 20) as u8;
            p.colors.push(NamedColor::new(format!("{}% Black", i * 5), Color::rgb(v, v, v)));
        }
        p
    }

    pub fn rgb_palette() -> Palette {
        let mut p = Palette { name: "Default RGB Palette".into(), colors: vec![] };
        let steps = [0u8, 51, 102, 153, 204, 255];
        for r in steps {
            for g in steps {
                for b in steps {
                    p.colors.push(NamedColor::new(format!("R{} G{} B{}", r, g, b), Color::rgb(r, g, b)));
                }
            }
        }
        p
    }

    pub fn web_palette() -> Palette {
        Palette::from_hex_list(
            "Web Safe Colors",
            &[
                ("Black", "#000000"),
                ("Navy", "#000080"),
                ("Blue", "#0000ff"),
                ("Green", "#008000"),
                ("Teal", "#008080"),
                ("Lime", "#00ff00"),
                ("Aqua", "#00ffff"),
                ("Maroon", "#800000"),
                ("Purple", "#800080"),
                ("Olive", "#808000"),
                ("Gray", "#808080"),
                ("Silver", "#c0c0c0"),
                ("Red", "#ff0000"),
                ("Fuchsia", "#ff00ff"),
                ("Yellow", "#ffff00"),
                ("White", "#ffffff"),
            ],
        )
    }

    /// Palette files: custom palettes (*.xml), a simple text format with one `name;#RRGGBB` per
    /// line, and GIMP .gpl.
    pub fn parse(name: &str, text: &str) -> Palette {
        if let Some(p) = Palette::parse_xml(text) {
            return p;
        }
        let mut p = Palette { name: name.to_string(), colors: vec![] };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') && line.len() != 7 && line.len() != 9 {
                continue;
            }
            if line.starts_with("GIMP") || line.starts_with("Name:") || line.starts_with("Columns:") {
                if let Some(n) = line.strip_prefix("Name:") {
                    p.name = n.trim().to_string();
                }
                continue;
            }
            if let Some((n, h)) = line.split_once(';') {
                if let Some(c) = Color::from_hex(h) {
                    p.colors.push(NamedColor::new(n.trim(), c));
                }
                continue;
            }
            // GIMP: "r g b name"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                if let (Ok(r), Ok(g), Ok(b)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>(), parts[2].parse::<u8>()) {
                    let n = if parts.len() > 3 { parts[3..].join(" ") } else { format!("{} {} {}", r, g, b) };
                    p.colors.push(NamedColor::new(n, Color::rgb(r, g, b)));
                    continue;
                }
            }
            if let Some(c) = Color::from_hex(line) {
                p.colors.push(NamedColor::new(c.to_hex(), c));
            }
        }
        p
    }

    /// An Adobe Swatch Exchange file (*.ase), as Adobe programs export color books and swatches:
    /// named RGB, CMYK, Lab and gray colors; colors of the spot type stay spot colors (#95).
    pub fn parse_ase(name: &str, data: &[u8]) -> Option<Palette> {
        let be16 = |i: usize| data.get(i..i + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
        let be32 = |i: usize| data.get(i..i + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
        let f32at = |i: usize| be32(i).map(|v| f32::from_bits(v) as f64);
        if data.get(..4)? != b"ASEF" {
            return None;
        }
        let count = be32(8)? as usize;
        let mut p = Palette { name: name.to_string(), colors: vec![] };
        let mut at = 12;
        for _ in 0..count {
            let kind = be16(at)?;
            let len = be32(at + 2)? as usize;
            let body = at + 6;
            let end = body.checked_add(len)?;
            if end > data.len() {
                break;
            }
            if kind == 0x0001 {
                let chars = be16(body)? as usize;
                let units: Vec<u16> = (0..chars).filter_map(|k| be16(body + 2 + 2 * k)).take_while(|&u| u != 0).collect();
                let cname = String::from_utf16_lossy(&units);
                let m = body + 2 + 2 * chars;
                let model = data.get(m..m + 4)?;
                let v = m + 4;
                let (color, n) = match model {
                    b"RGB " => {
                        let c = |k: usize| (f32at(v + 4 * k).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                        (Color::rgb(c(0), c(1), c(2)), 3)
                    }
                    b"CMYK" => {
                        let c = |k: usize| f32at(v + 4 * k).unwrap_or(0.0) * 100.0;
                        (Color::from_cmyk(c(0), c(1), c(2), c(3)), 4)
                    }
                    b"LAB " => {
                        let c = |k: usize| f32at(v + 4 * k).unwrap_or(0.0);
                        (lab_to_rgb(c(0) * 100.0, c(1), c(2)), 3)
                    }
                    b"Gray" => {
                        let g = (f32at(v).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                        (Color::rgb(g, g, g), 1)
                    }
                    _ => {
                        at = end;
                        continue;
                    }
                };
                let spot = be16(v + 4 * n) == Some(1);
                let label = if cname.trim().is_empty() { color.to_hex() } else { cname.trim().to_string() };
                p.colors.push(NamedColor { name: label, color, spot });
            }
            at = end;
        }
        (!p.colors.is_empty()).then_some(p)
    }

    /// A custom palette (*.xml): `<palette name=..>` with `<colors><page><color cs=.. tints=..
    /// name=../>`; spot colors are named color spaces in `<colorspaces>`.
    pub fn parse_xml(text: &str) -> Option<Palette> {
        let t = text.trim_start_matches('\u{feff}').trim_start();
        if !(t.starts_with("<?") || t.starts_with("<palette")) {
            return None;
        }
        let start = t.find("<palette")?;
        let head_end = t[start..].find('>')? + start;
        let name = xml_attr(&t[start..=head_end], "name").unwrap_or_default();
        // spot colors: named color spaces
        let mut spaces: Vec<(String, Color)> = vec![];
        let mut body = t[head_end + 1..].to_string();
        if let (Some(a), Some(b)) = (body.find("<colorspaces"), body.find("</colorspaces>")) {
            let block = body[a..b].to_string();
            let mut rest = block.as_str();
            while let Some(i) = rest.find("<cs ") {
                let Some(tag_end) = rest[i..].find('>').map(|e| e + i) else { break };
                let cs_name = xml_attr(&rest[i..=tag_end], "name").unwrap_or_default();
                let close = rest[tag_end..].find("</cs>").map(|e| e + tag_end).unwrap_or(rest.len());
                let inner = &rest[tag_end + 1..close];
                if let Some(ci) = inner.find("<color") {
                    if let Some(ce) = inner[ci..].find('>').map(|e| e + ci) {
                        if let Some(c) = xml_color(&inner[ci..=ce], &[]) {
                            spaces.push((cs_name, c));
                        }
                    }
                }
                rest = &rest[close..];
            }
            body.replace_range(a..b + "</colorspaces>".len(), "");
        }
        let mut colors = vec![];
        let mut rest = body.as_str();
        while let Some(i) = rest.find("<color ") {
            let e = rest[i..].find('>').map(|e| e + i)?;
            let tag = &rest[i..=e];
            let cs = xml_attr(tag, "cs").unwrap_or_default();
            if let Some(c) = xml_color(tag, &spaces) {
                let spot = spaces.iter().any(|(n, _)| n.eq_ignore_ascii_case(&cs));
                let n = xml_attr(tag, "name").unwrap_or_else(|| if spot { cs.clone() } else { c.to_hex() });
                colors.push(NamedColor { name: n, color: c, spot });
            }
            rest = &rest[e + 1..];
        }
        Some(Palette { name, colors })
    }

    /// The palette as a custom palette (*.xml).
    pub fn to_xml(&self) -> String {
        let num = |v: f64| {
            let s = format!("{v:.6}");
            let s = s.trim_end_matches('0').trim_end_matches('.');
            if s.is_empty() || s == "-0" {
                "0".to_string()
            } else {
                s.to_string()
            }
        };
        let rgb = |c: &Color| format!("{},{},{}", num(c.r as f64 / 255.0), num(c.g as f64 / 255.0), num(c.b as f64 / 255.0));
        // a CMYK color keeps its inks
        let space = |c: &Color| match c.cmyk {
            Some(k) => ("CMYK", k.iter().map(|v| num(*v as f64 / 100.0)).collect::<Vec<_>>().join(",")),
            None => ("RGB", rgb(c)),
        };
        let mut h: u64 = 0xcbf29ce484222325;
        for b in self.name.bytes().chain(self.colors.iter().flat_map(|c| [c.color.r, c.color.g, c.color.b])) {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
        let g2 = h.rotate_left(29) ^ 0x9e3779b97f4a7c15;
        let guid = format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", h >> 32, (h >> 16) & 0xffff, h & 0xffff, g2 >> 48, g2 & 0xffff_ffff_ffff);
        let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<palette guid=\"{guid}\" name=\"{}\">\n", xml_escape(&self.name));
        let spots: Vec<&NamedColor> = self.colors.iter().filter(|c| c.spot).collect();
        if !spots.is_empty() {
            s.push_str("<colorspaces>\n");
            for (i, c) in spots.iter().enumerate() {
                let (cs, tints) = space(&c.color);
                s.push_str(&format!("<cs name=\"{}\" fixedID=\"{}\">\n<color cs=\"{cs}\" tints=\"{tints}\"/>\n</cs>\n", xml_escape(&c.name), i + 1));
            }
            s.push_str("</colorspaces>\n");
        }
        s.push_str("<colors>\n<page>\n");
        for c in &self.colors {
            if c.spot {
                s.push_str(&format!("<color cs=\"{0}\" tints=\"1\" name=\"{0}\"/>\n", xml_escape(&c.name)));
            } else {
                let (cs, tints) = space(&c.color);
                s.push_str(&format!("<color cs=\"{cs}\" tints=\"{tints}\" name=\"{}\"/>\n", xml_escape(&c.name)));
            }
        }
        s.push_str("</page>\n</colors>\n</palette>\n");
        s
    }

    pub fn serialize(&self) -> String {
        let mut s = format!("# AnyVector palette\nName: {}\n", self.name);
        for c in &self.colors {
            s.push_str(&format!("{};{}\n", c.name, c.color.to_hex()));
        }
        s
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn xml_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(e) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        let ent = &tail[1..e];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent.strip_prefix("#x").and_then(|h| u32::from_str_radix(h, 16).ok()).or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok())).and_then(char::from_u32),
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=e]),
        }
        rest = &tail[e + 1..];
    }
    out.push_str(rest);
    out
}

/// An attribute of an XML start tag.
fn xml_attr(tag: &str, name: &str) -> Option<String> {
    let mut from = 0;
    while let Some(i) = tag[from..].find(name).map(|i| i + from) {
        let before_ok = i > 0 && tag.as_bytes()[i - 1].is_ascii_whitespace();
        let after = tag[i + name.len()..].trim_start();
        if before_ok && after.starts_with('=') {
            let v = after[1..].trim_start();
            let q = v.chars().next()?;
            if q == '"' || q == '\'' {
                let end = v[1..].find(q)?;
                return Some(xml_unescape(&v[1..1 + end]));
            }
        }
        from = i + name.len();
    }
    None
}

/// The color of a `<color cs=.. tints=..>` tag; `spaces` are the named (spot) color spaces.
fn xml_color(tag: &str, spaces: &[(String, Color)]) -> Option<Color> {
    let cs = xml_attr(tag, "cs")?.to_uppercase();
    let t: Vec<f64> = xml_attr(tag, "tints").unwrap_or_default().split(',').filter_map(|v| v.trim().parse().ok()).collect();
    let u8c = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let get = |i: usize| t.get(i).copied().unwrap_or(0.0);
    Some(match cs.as_str() {
        "RGB" => Color::rgb(u8c(get(0)), u8c(get(1)), u8c(get(2))),
        "CMYK" => Color::from_cmyk(get(0) * 100.0, get(1) * 100.0, get(2) * 100.0, get(3) * 100.0),
        "CMY" => Color::from_cmyk(get(0) * 100.0, get(1) * 100.0, get(2) * 100.0, 0.0),
        "GRAY" | "GREY" => {
            let v = u8c(get(0));
            Color::rgb(v, v, v)
        }
        "HSB" | "HSV" => Color::from_hsb(get(0) * if get(0) <= 1.0 { 360.0 } else { 1.0 }, get(1) * 100.0, get(2) * 100.0),
        "LAB" => lab_to_rgb(get(0), get(1), get(2)),
        _ => {
            // a spot color: its color space, tinted
            let (_, base) = spaces.iter().find(|(n, _)| n.to_uppercase() == cs)?;
            let k = if t.is_empty() { 1.0 } else { get(0).clamp(0.0, 1.0) };
            Color::lerp(Color::WHITE, *base, k)
        }
    })
}

/// CIE L*a*b* (D65) to sRGB.
fn lab_to_rgb(l: f64, a: f64, b: f64) -> Color {
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let f = |t: f64| if t > 6.0 / 29.0 { t * t * t } else { 3.0 * (6.0f64 / 29.0).powi(2) * (t - 4.0 / 29.0) };
    let (x, y, z) = (0.95047 * f(fx), f(fy), 1.08883 * f(fz));
    let r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
    let g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
    let bl = 0.0557 * x - 0.2040 * y + 1.0570 * z;
    let gam = |c: f64| {
        let c = if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
        (c.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    Color::rgb(gam(r), gam(g), gam(bl))
}

#[cfg(test)]
mod xml_tests {
    use super::{Color, NamedColor, Palette};

    /// The default palette is CorelDRAW's CMYK Palette: its colors keep their inks, so they show
    /// through the color management in force (bright with it off, issue #90).
    #[test]
    fn default_palette_is_corels_cmyk_palette() {
        let p = Palette::default_palette();
        assert_eq!(p.colors.len(), 100);
        assert!(p.colors.iter().all(|c| c.color.is_cmyk()), "every color is defined in CMYK");
        let inks = |n: &str| p.colors.iter().find(|c| c.name == n).and_then(|c| c.color.cmyk);
        assert_eq!(inks("Black"), Some([0, 0, 0, 100]));
        assert_eq!(inks("Cyan"), Some([100, 0, 0, 0]));
        assert_eq!(inks("Red"), Some([0, 100, 100, 0]));
        assert_eq!(inks("Deep Blue"), Some([60, 80, 0, 20]));
        assert_eq!(p.colors[11].name, "Blue");
        // it survives saving as a custom palette
        let back = Palette::parse_xml(&p.to_xml()).unwrap();
        assert_eq!(back.colors.iter().map(|c| c.color.cmyk).collect::<Vec<_>>(), p.colors.iter().map(|c| c.color.cmyk).collect::<Vec<_>>());
    }

    #[test]
    fn xml_palette_round_trip() {
        let p = Palette { name: "Mine & <yours>".into(), colors: vec![NamedColor::new("Red", Color::rgb(255, 0, 0)), NamedColor { name: "Spot One".into(), color: Color::rgb(0, 128, 255), spot: true }] };
        let x = p.to_xml();
        let q = Palette::parse("ignored", &x);
        assert_eq!(q.name, p.name);
        assert_eq!(q.colors.len(), 2);
        assert_eq!(q.colors[0].color, Color::rgb(255, 0, 0));
        assert!(q.colors[1].spot && q.colors[1].name == "Spot One");
        assert_eq!(q.colors[1].color, Color::rgb(0, 128, 255));
        let cmyk = Palette::parse("x", "<?xml version=\"1.0\"?><palette name=\"P\"><colors><page><color cs=\"CMYK\" tints=\"0,0,0,1\" name=\"Black\"/><color cs=\"GRAY\" tints=\"0.5\" name=\"Mid\"/></page></colors></palette>");
        assert_eq!(cmyk.colors[0].color.cmyk, Some([0, 0, 0, 100]));
        // CMYK colors are written with their inks
        let back = Palette::parse("x", &cmyk.to_xml());
        assert_eq!(back.colors[0].color.cmyk, Some([0, 0, 0, 100]));
        assert_eq!(cmyk.colors[1].color, Color::rgb(128, 128, 128));
    }
}
