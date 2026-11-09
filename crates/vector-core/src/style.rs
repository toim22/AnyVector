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
    /// Node transparency 0..100 (empty = opaque), same order as `colors`.
    #[serde(default)]
    pub alpha: Vec<f64>,
    /// Smooth Mesh Color: smooth color transitions between patches.
    #[serde(default)]
    pub smooth: bool,
    /// Curve smoothness 0..100: how far the color of a node spreads into its patches.
    #[serde(default)]
    pub smoothness: f64,
}

impl MeshFill {
    pub fn new(cols: usize, rows: usize, base: Color) -> MeshFill {
        MeshFill { cols, rows, colors: vec![base; (cols + 1) * (rows + 1)], alpha: vec![], smooth: false, smoothness: 0.0 }
    }
    pub fn color_at(&self, u: f64, v: f64) -> Color {
        let u = u.clamp(0.0, 1.0) * self.cols as f64;
        let v = v.clamp(0.0, 1.0) * self.rows as f64;
        let i = (u.floor() as usize).min(self.cols.saturating_sub(1));
        let j = (v.floor() as usize).min(self.rows.saturating_sub(1));
        let mut fu = u - i as f64;
        let mut fv = v - j as f64;
        if self.smooth {
            fu = fu * fu * (3.0 - 2.0 * fu);
            fv = fv * fv * (3.0 - 2.0 * fv);
        }
        if self.smoothness > 0.0 {
            // a node's color spreads further into its patches: push the factors toward the nearest node
            let k = 1.0 + self.smoothness.clamp(0.0, 100.0) / 25.0;
            let push = |f: f64| if f < 0.5 { 0.5 * (2.0 * f).powf(k) } else { 1.0 - 0.5 * (2.0 * (1.0 - f)).powf(k) };
            fu = push(fu);
            fv = push(fv);
        }
        let n = self.cols + 1;
        let idx = |i: usize, j: usize| {
            let mut c = self.colors.get(j * n + i).copied().unwrap_or(Color::WHITE);
            if let Some(a) = self.alpha.get(j * n + i) {
                c.a = ((1.0 - a.clamp(0.0, 100.0) / 100.0) * c.a as f64).round() as u8;
            }
            c
        };
        let top = Color::lerp(idx(i, j), idx(i + 1, j), fu);
        let bot = Color::lerp(idx(i, j + 1), idx(i + 1, j + 1), fu);
        Color::lerp(top, bot, fv)
    }
    /// Add Intersection: a new column and row of nodes through (u, v), colored from the mesh.
    pub fn add_intersection(&mut self, u: f64, v: f64) {
        let (u, v) = (u.clamp(0.001, 0.999), v.clamp(0.001, 0.999));
        let old = self.clone();
        let (cols, rows) = (self.cols + 1, self.rows + 1);
        // the node positions, evenly spaced; the new ones are inserted at the nearest cell
        let ci = ((u * self.cols as f64).floor() as usize + 1).min(cols);
        let ri = ((v * self.rows as f64).floor() as usize + 1).min(rows);
        let us: Vec<f64> = (0..=cols).map(|c| if c < ci { c as f64 / self.cols as f64 } else if c == ci { u } else { (c - 1) as f64 / self.cols as f64 }).collect();
        let vs: Vec<f64> = (0..=rows).map(|r| if r < ri { r as f64 / self.rows as f64 } else if r == ri { v } else { (r - 1) as f64 / self.rows as f64 }).collect();
        let mut colors = Vec::with_capacity((cols + 1) * (rows + 1));
        let mut alpha = vec![];
        let mut plain = old.clone();
        plain.alpha.clear();
        for vv in &vs {
            for uu in &us {
                colors.push(plain.color_at(*uu, *vv));
                if !old.alpha.is_empty() {
                    let c = old.color_at(*uu, *vv);
                    let p = plain.color_at(*uu, *vv);
                    alpha.push(if p.a == 0 { 0.0 } else { (1.0 - c.a as f64 / p.a as f64) * 100.0 });
                }
            }
        }
        self.cols = cols;
        self.rows = rows;
        self.colors = colors;
        self.alpha = alpha;
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
    /// Object ▸ Create ▸ Arrowhead: an id in `Document::arrowheads`.
    Custom(u64),
}

/// An arrowhead made from an object. The path has its tip at (0, 0), points along +x and is
/// one unit long; `width` is its width in the same unit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomArrow {
    pub id: u64,
    pub name: String,
    /// SVG path data.
    pub path: String,
    pub width: f64,
    /// Filled (closed shapes) or stroked.
    pub filled: bool,
}

impl CustomArrow {
    /// The arrowhead at the end of a line: `tip`, pointing along `dir`, for outline width `w`.
    pub fn placed(&self, tip: kurbo::Point, dir: kurbo::Vec2, w: f64) -> kurbo::BezPath {
        let s = (w * 4.0).max(4.0);
        let a = kurbo::Affine::translate(tip.to_vec2()) * kurbo::Affine::rotate(dir.atan2()) * kurbo::Affine::scale(s);
        a * kurbo::BezPath::from_svg(&self.path).unwrap_or_default()
    }
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
            ArrowHead::Custom(_) => "Custom",
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
    Painterly {
        bristles: u32,
        jitter: f64,
        seed: u32,
        #[serde(default)]
        opts: PaintOpts,
    },
    /// Variable Outline: widths of the two sides at positions along the outline.
    Variable { nodes: Vec<VarWidth>, scale: bool },
}

/// The settings of a Painterly Brush stroke (property bar).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaintOpts {
    /// Brush style (index into the brush catalog of the app).
    pub brush: u32,
    /// Brush transparency, 0..100.
    pub alpha: f64,
    /// Glow: brightness builds up to white along the stroke.
    pub glow: bool,
    /// Simulate pen pressure: the stroke tapers.
    pub taper: bool,
    /// Density of droplets and particles, 0..100.
    pub density: f64,
    /// Tilt angle (90 = upright) and direction (degrees, 0 = pointing left).
    pub tilt: f64,
    pub tilt_dir: f64,
    /// Texture strength of the Basic Paper texture, 0..100.
    pub texture_strength: f64,
    /// Kind of mark: 0 bristles, 1 airbrush droplets, 2 particles, 3 solid ink.
    pub mark: u8,
}

impl Default for PaintOpts {
    fn default() -> Self {
        PaintOpts { brush: 0, alpha: 0.0, glow: false, taper: false, density: 50.0, tilt: 90.0, tilt_dir: 0.0, texture_strength: 50.0, mark: 0 }
    }
}

/// A node of a variable outline: position along the outline (0..1) and the width on each side
/// of the path, in points.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VarWidth {
    pub t: f64,
    pub side1: f64,
    pub side2: f64,
}

/// The widths of a variable outline at `t` (0..1): (side 1, side 2), interpolated between nodes.
pub fn var_width_at(nodes: &[VarWidth], t: f64) -> (f64, f64) {
    if nodes.is_empty() {
        return (0.0, 0.0);
    }
    let mut v: Vec<&VarWidth> = nodes.iter().collect();
    v.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    if t <= v[0].t {
        return (v[0].side1, v[0].side2);
    }
    for w in v.windows(2) {
        if t <= w[1].t {
            let f = ((t - w[0].t) / (w[1].t - w[0].t).max(1e-9)).clamp(0.0, 1.0);
            let f = f * f * (3.0 - 2.0 * f);
            return (w[0].side1 + (w[1].side1 - w[0].side1) * f, w[0].side2 + (w[1].side2 - w[0].side2) * f);
        }
    }
    let l = v[v.len() - 1];
    (l.side1, l.side2)
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
    #[serde(default)]
    pub overprint_bitmap: bool,
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
            overprint_bitmap: false,
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
