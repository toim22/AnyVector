//! Fill, outline and transparency attributes.

use crate::color::Color;
use kurbo::Point;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GradientKind {
    Linear,
    Elliptical,
    Conical,
    Rectangular,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpreadMode {
    Pad,
    Repeat,
    Reflect,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub pos: f64,
    pub color: Color,
}

/// A gradient. `start`/`end` are in normalized bounding-box space of the object
/// (0,0 = top-left, 1,1 = bottom-right) so the fill follows the object when it moves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    pub kind: GradientKind,
    pub stops: Vec<GradientStop>,
    pub start: Point,
    pub end: Point,
    pub spread: SpreadMode,
    /// Number of rendering steps (0 = smooth).
    #[serde(default)]
    pub steps: u32,
}

impl Gradient {
    pub fn linear(a: Color, b: Color) -> Gradient {
        Gradient {
            kind: GradientKind::Linear,
            stops: vec![GradientStop { pos: 0.0, color: a }, GradientStop { pos: 1.0, color: b }],
            start: Point::new(0.0, 0.5),
            end: Point::new(1.0, 0.5),
            spread: SpreadMode::Pad,
            steps: 0,
        }
    }
    pub fn radial(a: Color, b: Color) -> Gradient {
        Gradient {
            kind: GradientKind::Elliptical,
            stops: vec![GradientStop { pos: 0.0, color: a }, GradientStop { pos: 1.0, color: b }],
            start: Point::new(0.5, 0.5),
            end: Point::new(1.0, 0.5),
            spread: SpreadMode::Pad,
            steps: 0,
        }
    }
    pub fn sorted_stops(&self) -> Vec<GradientStop> {
        let mut s = self.stops.clone();
        s.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
        if s.is_empty() {
            s.push(GradientStop { pos: 0.0, color: Color::BLACK });
            s.push(GradientStop { pos: 1.0, color: Color::WHITE });
        }
        s
    }
    pub fn color_at(&self, t: f64) -> Color {
        let s = self.sorted_stops();
        if t <= s[0].pos {
            return s[0].color;
        }
        for w in s.windows(2) {
            if t >= w[0].pos && t <= w[1].pos {
                let span = (w[1].pos - w[0].pos).max(1e-9);
                return Color::lerp(w[0].color, w[1].color, (t - w[0].pos) / span);
            }
        }
        s[s.len() - 1].color
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternKind {
    Checker,
    HorizontalLines,
    VerticalLines,
    DiagonalLines,
    CrossHatch,
    Dots,
    Bricks,
    Waves,
    Grid,
    Diamonds,
}

impl PatternKind {
    pub const ALL: [PatternKind; 10] = [
        PatternKind::Checker,
        PatternKind::HorizontalLines,
        PatternKind::VerticalLines,
        PatternKind::DiagonalLines,
        PatternKind::CrossHatch,
        PatternKind::Dots,
        PatternKind::Bricks,
        PatternKind::Waves,
        PatternKind::Grid,
        PatternKind::Diamonds,
    ];
    pub fn name(self) -> &'static str {
        match self {
            PatternKind::Checker => "Checkerboard",
            PatternKind::HorizontalLines => "Horizontal lines",
            PatternKind::VerticalLines => "Vertical lines",
            PatternKind::DiagonalLines => "Diagonal lines",
            PatternKind::CrossHatch => "Cross hatch",
            PatternKind::Dots => "Dots",
            PatternKind::Bricks => "Bricks",
            PatternKind::Waves => "Waves",
            PatternKind::Grid => "Grid",
            PatternKind::Diamonds => "Diamonds",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternFill {
    pub kind: PatternKind,
    pub fg: Color,
    pub bg: Color,
    /// Tile size in points.
    pub tile: f64,
    pub angle: f64,
    /// Optional bitmap pattern (image id in the document's image store).
    #[serde(default)]
    pub image: Option<u64>,
    #[serde(default = "default_true")]
    pub transform_with_object: bool,
}

fn default_true() -> bool {
    true
}

/// Mesh fill: a grid of colors over the object's bounding box, bilinearly interpolated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshFill {
    pub cols: usize,
    pub rows: usize,
    /// (cols+1) * (rows+1) node colors, row-major.
    pub colors: Vec<Color>,
}

impl MeshFill {
    pub fn new(cols: usize, rows: usize, base: Color) -> MeshFill {
        MeshFill { cols, rows, colors: vec![base; (cols + 1) * (rows + 1)] }
    }
    pub fn color_at(&self, u: f64, v: f64) -> Color {
        let u = u.clamp(0.0, 1.0) * self.cols as f64;
        let v = v.clamp(0.0, 1.0) * self.rows as f64;
        let i = (u.floor() as usize).min(self.cols.saturating_sub(1));
        let j = (v.floor() as usize).min(self.rows.saturating_sub(1));
        let fu = u - i as f64;
        let fv = v - j as f64;
        let idx = |i: usize, j: usize| self.colors.get(j * (self.cols + 1) + i).copied().unwrap_or(Color::WHITE);
        let top = Color::lerp(idx(i, j), idx(i + 1, j), fu);
        let bot = Color::lerp(idx(i, j + 1), idx(i + 1, j + 1), fu);
        Color::lerp(top, bot, fv)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum Fill {
    #[default]
    None,
    Solid(Color),
    Gradient(Gradient),
    Pattern(PatternFill),
    Mesh(MeshFill),
}

impl Fill {
    pub fn is_none(&self) -> bool {
        matches!(self, Fill::None)
    }
    /// A representative color (for swatches/status bar).
    pub fn main_color(&self) -> Option<Color> {
        match self {
            Fill::None => None,
            Fill::Solid(c) => Some(*c),
            Fill::Gradient(g) => g.sorted_stops().first().map(|s| s.color),
            Fill::Pattern(p) => Some(p.fg),
            Fill::Mesh(m) => m.colors.first().copied(),
        }
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Fill::None => "None",
            Fill::Solid(_) => "Uniform Fill",
            Fill::Gradient(_) => "Fountain Fill",
            Fill::Pattern(_) => "Pattern Fill",
            Fill::Mesh(_) => "Mesh Fill",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OutlinePosition {
    #[default]
    Center,
    Inside,
    Outside,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArrowHead {
    Triangle,
    OpenTriangle,
    Circle,
    Square,
    Diamond,
    Bar,
}

impl ArrowHead {
    pub const ALL: [ArrowHead; 6] =
        [ArrowHead::Triangle, ArrowHead::OpenTriangle, ArrowHead::Circle, ArrowHead::Square, ArrowHead::Diamond, ArrowHead::Bar];
    pub fn name(self) -> &'static str {
        match self {
            ArrowHead::Triangle => "Triangle",
            ArrowHead::OpenTriangle => "Open triangle",
            ArrowHead::Circle => "Circle",
            ArrowHead::Square => "Square",
            ArrowHead::Diamond => "Diamond",
            ArrowHead::Bar => "Bar",
        }
    }
}

/// Width profile for artistic-media / variable outlines.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WidthProfile {
    /// Thick in the middle, tapering to both ends.
    Taper,
    /// Starts thick, ends thin.
    TaperEnd,
    /// Starts thin, ends thick.
    TaperStart,
    /// Calligraphic nib at a fixed angle (degrees), with a minimum width ratio.
    Calligraphic { angle: f64, min_ratio: f64 },
    /// Pressure samples (0..1) along the stroke, evenly spaced.
    Pressure(Vec<f32>),
    /// Painterly brush: stamped, textured dabs.
    Painterly { bristles: u32, jitter: f64, seed: u32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Outline {
    /// Width in points; 0 = hairline.
    pub width: f64,
    /// `None` = no outline.
    pub color: Option<Color>,
    pub join: LineJoin,
    pub cap: LineCap,
    /// Dash pattern in multiples of the line width; empty = solid.
    pub dash: Vec<f64>,
    pub miter_limit: f64,
    pub position: OutlinePosition,
    pub behind_fill: bool,
    pub scale_with_object: bool,
    pub start_arrow: Option<ArrowHead>,
    pub end_arrow: Option<ArrowHead>,
    #[serde(default)]
    pub profile: Option<WidthProfile>,
}

impl Default for Outline {
    fn default() -> Self {
        Outline {
            width: 0.5669, // 0.2 mm
            color: Some(Color::BLACK),
            join: LineJoin::Miter,
            cap: LineCap::Butt,
            dash: vec![],
            miter_limit: 5.0,
            position: OutlinePosition::Center,
            behind_fill: false,
            scale_with_object: false,
            start_arrow: None,
            end_arrow: None,
            profile: None,
        }
    }
}

impl Outline {
    pub fn none() -> Outline {
        Outline { color: None, ..Default::default() }
    }
    pub fn is_visible(&self) -> bool {
        self.color.is_some()
    }
}

/// Dash presets (multiples of width).
pub fn dash_presets() -> Vec<(&'static str, Vec<f64>)> {
    vec![
        ("Solid", vec![]),
        ("Dotted", vec![1.0, 2.0]),
        ("Dashed", vec![4.0, 3.0]),
        ("Long dash", vec![8.0, 4.0]),
        ("Dash dot", vec![6.0, 3.0, 1.0, 3.0]),
        ("Dash dot dot", vec![6.0, 3.0, 1.0, 3.0, 1.0, 3.0]),
        ("Fine dots", vec![0.5, 1.5]),
    ]
}

/// Standard outline widths (points) offered in the property bar.
pub fn outline_width_presets() -> Vec<(&'static str, f64)> {
    vec![
        ("Hairline", 0.0),
        ("0.1 mm", 0.2835),
        ("0.2 mm", 0.5669),
        ("0.25 pt", 0.25),
        ("0.5 pt", 0.5),
        ("0.75 pt", 0.75),
        ("1.0 pt", 1.0),
        ("1.5 pt", 1.5),
        ("2.0 pt", 2.0),
        ("3.0 pt", 3.0),
        ("4.0 pt", 4.0),
        ("6.0 pt", 6.0),
        ("8.0 pt", 8.0),
        ("10.0 pt", 10.0),
        ("12.0 pt", 12.0),
        ("16.0 pt", 16.0),
        ("24.0 pt", 24.0),
        ("36.0 pt", 36.0),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MergeMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl MergeMode {
    pub const ALL: [MergeMode; 16] = [
        MergeMode::Normal,
        MergeMode::Multiply,
        MergeMode::Screen,
        MergeMode::Overlay,
        MergeMode::Darken,
        MergeMode::Lighten,
        MergeMode::ColorDodge,
        MergeMode::ColorBurn,
        MergeMode::HardLight,
        MergeMode::SoftLight,
        MergeMode::Difference,
        MergeMode::Exclusion,
        MergeMode::Hue,
        MergeMode::Saturation,
        MergeMode::Color,
        MergeMode::Luminosity,
    ];
    pub fn name(self) -> &'static str {
        match self {
            MergeMode::Normal => "Normal",
            MergeMode::Multiply => "Multiply",
            MergeMode::Screen => "Screen",
            MergeMode::Overlay => "Overlay",
            MergeMode::Darken => "Darken",
            MergeMode::Lighten => "Lighten",
            MergeMode::ColorDodge => "Color dodge",
            MergeMode::ColorBurn => "Color burn",
            MergeMode::HardLight => "Hard light",
            MergeMode::SoftLight => "Soft light",
            MergeMode::Difference => "Difference",
            MergeMode::Exclusion => "Exclusion",
            MergeMode::Hue => "Hue",
            MergeMode::Saturation => "Saturation",
            MergeMode::Color => "Color",
            MergeMode::Luminosity => "Luminosity",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum Transparency {
    #[default]
    None,
    /// 0 = opaque, 100 = fully transparent.
    Uniform(f64),
    /// Gradient whose stop colors' luminance encodes opacity (black = transparent, white = opaque),
    /// matching the original's transparency ramp convention.
    Gradient(Gradient),
}

impl Transparency {
    pub fn uniform_opacity(&self) -> f32 {
        match self {
            Transparency::Uniform(t) => (1.0 - t / 100.0).clamp(0.0, 1.0) as f32,
            _ => 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Style {
    pub fill: Fill,
    pub outline: Outline,
    pub transparency: Transparency,
    #[serde(default)]
    pub merge: MergeMode,
    #[serde(default)]
    pub overprint_fill: bool,
    #[serde(default)]
    pub overprint_outline: bool,
    /// Even-odd fill rule (combined curves with holes) vs non-zero.
    #[serde(default = "default_true")]
    pub even_odd: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            fill: Fill::None,
            outline: Outline::default(),
            transparency: Transparency::None,
            merge: MergeMode::Normal,
            overprint_fill: false,
            overprint_outline: false,
            even_odd: true,
        }
    }
}

impl Style {
    pub fn filled(c: Color) -> Style {
        Style { fill: Fill::Solid(c), outline: Outline::none(), ..Default::default() }
    }
    pub fn text_default() -> Style {
        Style { fill: Fill::Solid(Color::BLACK), outline: Outline::none(), ..Default::default() }
    }
}
