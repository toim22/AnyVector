//! Drawing objects and their geometry.

use crate::color::Color;
use crate::geom;
use crate::path::NodePath;
use crate::style::Style;
use crate::text::{self, TextObj};
use kurbo::{Affine, BezPath, ParamCurve, Point, Rect, Shape, Vec2};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

pub type ObjId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CornerType {
    #[default]
    Round,
    Scallop,
    Chamfer,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RectShape {
    pub w: f64,
    pub h: f64,
    /// Corner sizes: top-left, top-right, bottom-right, bottom-left.
    pub radii: [f64; 4],
    pub corner: CornerType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EllipseKind {
    #[default]
    Ellipse,
    Pie,
    Arc,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EllipseShape {
    pub w: f64,
    pub h: f64,
    pub kind: EllipseKind,
    /// Degrees, counter-clockwise from 3 o'clock (as displayed in the property bar).
    pub start: f64,
    pub end: f64,
    pub clockwise: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PolygonKind {
    #[default]
    Polygon,
    Star,
    ComplexStar,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolygonShape {
    pub w: f64,
    pub h: f64,
    pub sides: u32,
    pub kind: PolygonKind,
    /// Star sharpness 0..100 (%).
    pub sharpness: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpiralShape {
    pub w: f64,
    pub h: f64,
    pub revolutions: f64,
    pub logarithmic: bool,
    /// Expansion factor 1..100 for logarithmic spirals.
    pub expansion: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageObj {
    pub image: u64,
    pub w: f64,
    pub h: f64,
    /// Crop rectangle in normalized source coordinates (0..1).
    #[serde(default = "full_crop")]
    pub crop: [f64; 4],
    #[serde(default)]
    pub adjust: ImageAdjust,
}

fn full_crop() -> [f64; 4] {
    [0.0, 0.0, 1.0, 1.0]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct ImageAdjust {
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub grayscale: bool,
    pub invert: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    pub text: String,
    pub fill: Option<Color>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableObj {
    pub rows: usize,
    pub cols: usize,
    pub col_widths: Vec<f64>,
    pub row_heights: Vec<f64>,
    pub cells: Vec<TableCell>,
    pub border: Color,
    pub border_width: f64,
    pub text_format: text::CharFormat,
}

impl TableObj {
    pub fn new(rows: usize, cols: usize, w: f64, h: f64) -> TableObj {
        TableObj {
            rows,
            cols,
            col_widths: vec![w / cols as f64; cols],
            row_heights: vec![h / rows as f64; rows],
            cells: vec![TableCell { text: String::new(), fill: None }; rows * cols],
            border: Color::BLACK,
            border_width: 0.5669,
            text_format: text::CharFormat { size: 10.0, ..Default::default() },
        }
    }
    pub fn width(&self) -> f64 {
        self.col_widths.iter().sum()
    }
    pub fn height(&self) -> f64 {
        self.row_heights.iter().sum()
    }
    pub fn cell_rect(&self, r: usize, c: usize) -> Rect {
        let x0: f64 = self.col_widths[..c].iter().sum();
        let y0: f64 = self.row_heights[..r].iter().sum();
        Rect::new(x0, y0, x0 + self.col_widths[c], y0 + self.row_heights[r])
    }
    pub fn cell_at(&self, p: Point) -> Option<(usize, usize)> {
        let mut y = 0.0;
        for r in 0..self.rows {
            let mut x = 0.0;
            for c in 0..self.cols {
                let rc = Rect::new(x, y, x + self.col_widths[c], y + self.row_heights[r]);
                if rc.contains(p) {
                    return Some((r, c));
                }
                x += self.col_widths[c];
            }
            y += self.row_heights[r];
        }
        None
    }
    pub fn scale(&mut self, sx: f64, sy: f64) {
        for w in self.col_widths.iter_mut() {
            *w *= sx;
        }
        for h in self.row_heights.iter_mut() {
            *h *= sy;
        }
    }
    pub fn insert_row(&mut self, at: usize) {
        let h = self.row_heights.get(at.min(self.rows.saturating_sub(1))).copied().unwrap_or(20.0);
        self.row_heights.insert(at, h);
        for _ in 0..self.cols {
            self.cells.insert(at * self.cols, TableCell { text: String::new(), fill: None });
        }
        self.rows += 1;
    }
    pub fn insert_col(&mut self, at: usize) {
        let w = self.col_widths.get(at.min(self.cols.saturating_sub(1))).copied().unwrap_or(40.0);
        self.col_widths.insert(at, w);
        for r in (0..self.rows).rev() {
            self.cells.insert(r * self.cols + at, TableCell { text: String::new(), fill: None });
        }
        self.cols += 1;
    }
    pub fn delete_row(&mut self, at: usize) {
        if self.rows <= 1 || at >= self.rows {
            return;
        }
        self.row_heights.remove(at);
        self.cells.drain(at * self.cols..(at + 1) * self.cols);
        self.rows -= 1;
    }
    pub fn delete_col(&mut self, at: usize) {
        if self.cols <= 1 || at >= self.cols {
            return;
        }
        self.col_widths.remove(at);
        for r in (0..self.rows).rev() {
            self.cells.remove(r * self.cols + at);
        }
        self.cols -= 1;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimensionKind {
    Parallel,
    HorizontalVertical,
    Angular,
    Segment,
    Callout,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DimensionObj {
    pub kind: DimensionKind,
    /// Parallel/HV/Segment: [p0, p1, label point]. Angular: [vertex, a, b, label]. Callout: [target, elbow, text].
    pub points: Vec<Point>,
    pub text: String,
    pub precision: usize,
    pub prefix: String,
    pub suffix: String,
    pub show_units: bool,
    pub font_size: f64,
    pub unit: crate::units::Unit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectorKind {
    Straight,
    RightAngle,
    RoundedRightAngle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectorEnd {
    pub point: Point,
    /// Attached object and the anchor (normalized point in its bbox).
    pub attached: Option<(ObjId, Point)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectorObj {
    pub kind: ConnectorKind,
    pub start: ConnectorEnd,
    pub end: ConnectorEnd,
    pub corner_radius: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendObj {
    pub start: Box<Object>,
    pub end: Box<Object>,
    pub steps: u32,
    /// Rotation applied progressively across the blend, degrees.
    pub rotation: f64,
    /// Accelerate objects/colors (-100..100).
    pub acceleration: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ObjKind {
    Rect(RectShape),
    Ellipse(EllipseShape),
    Polygon(PolygonShape),
    Spiral(SpiralShape),
    Path(NodePath),
    Text(TextObj),
    Image(ImageObj),
    Group(Vec<Object>),
    /// PowerClip: a frame object with contents clipped to it.
    PowerClip { frame: Box<Object>, contents: Vec<Object> },
    Table(TableObj),
    Dimension(DimensionObj),
    Connector(ConnectorObj),
    Blend(BlendObj),
    /// A symbol instance referencing a document symbol definition.
    Symbol(u64),
}

impl ObjKind {
    pub fn type_name(&self) -> &'static str {
        match self {
            ObjKind::Rect(_) => "Rectangle",
            ObjKind::Ellipse(e) => match e.kind {
                EllipseKind::Ellipse => "Ellipse",
                EllipseKind::Pie => "Pie",
                EllipseKind::Arc => "Arc",
            },
            ObjKind::Polygon(p) => match p.kind {
                PolygonKind::Polygon => "Polygon",
                _ => "Star",
            },
            ObjKind::Spiral(_) => "Spiral",
            ObjKind::Path(_) => "Curve",
            ObjKind::Text(t) => match t.kind {
                text::TextKind::Artistic => "Artistic Text",
                text::TextKind::Paragraph { .. } => "Paragraph Text",
            },
            ObjKind::Image(_) => "Bitmap",
            ObjKind::Group(_) => "Group",
            ObjKind::PowerClip { .. } => "PowerClip",
            ObjKind::Table(_) => "Table",
            ObjKind::Dimension(_) => "Dimension",
            ObjKind::Connector(_) => "Connector",
            ObjKind::Blend(_) => "Blend Group",
            ObjKind::Symbol(_) => "Symbol",
        }
    }
}

// ------------------------------------------------------------------------------------------
// Effects
// ------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DropShadow {
    pub offset: Vec2,
    pub feather: f64,
    pub color: Color,
    /// 0..100 opacity.
    pub opacity: f64,
    #[serde(default)]
    pub inner: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockShadow {
    pub offset: Vec2,
    pub color: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContourDir {
    Inside,
    Outside,
    ToCenter,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContourEffect {
    pub dir: ContourDir,
    pub steps: u32,
    pub offset: f64,
    pub end_fill: Color,
    pub end_outline: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtrudeEffect {
    /// Extrusion vector in points (towards the vanishing point).
    pub depth: Vec2,
    pub color: Color,
    pub shade: Color,
    pub steps: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistortKind {
    PushPull,
    Zipper,
    Twister,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DistortEffect {
    pub kind: DistortKind,
    pub amount: f64,
    pub frequency: f64,
    /// Center in normalized bbox coordinates.
    pub center: Point,
}

/// Envelope: 8 control handles (corners + edge midpoints), as offsets from the bbox positions,
/// in normalized coordinates. Order: TL, T, TR, R, BR, B, BL, L.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvelopeEffect {
    pub handles: [Vec2; 8],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BitmapEffect {
    GaussianBlur { radius: f64 },
    Glow { radius: f64, color: Color },
    Emboss { depth: f64 },
    Noise { amount: f64 },
    Pixelate { size: f64 },
    Grayscale,
    Sepia,
    Invert,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Effects {
    pub drop_shadow: Option<DropShadow>,
    pub inner_shadow: Option<DropShadow>,
    pub block_shadow: Option<BlockShadow>,
    pub contour: Option<ContourEffect>,
    pub extrude: Option<ExtrudeEffect>,
    pub distort: Option<DistortEffect>,
    pub envelope: Option<EnvelopeEffect>,
    #[serde(default)]
    pub bitmap: Vec<BitmapEffect>,
    /// Bevel: depth (points), light angle (deg).
    #[serde(default)]
    pub bevel: Option<(f64, f64)>,
    /// Lens: magnify factor (1 = none), or color filter.
    #[serde(default)]
    pub lens: Option<LensEffect>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LensEffect {
    Magnify(f64),
    Brighten(f64),
    Invert,
    ColorAdd(Color),
    Grayscale,
}

impl Effects {
    pub fn is_empty(&self) -> bool {
        self == &Effects::default()
    }
    pub fn warps_geometry(&self) -> bool {
        self.distort.is_some() || self.envelope.is_some()
    }
}

// ------------------------------------------------------------------------------------------
// Object
// ------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub id: ObjId,
    #[serde(default)]
    pub name: String,
    pub kind: ObjKind,
    pub transform: Affine,
    pub style: Style,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default = "yes")]
    pub printable: bool,
    #[serde(default)]
    pub effects: Effects,
    /// Object Data fields (name, value).
    #[serde(default)]
    pub data: Vec<(String, String)>,
    /// Internet link (Links and Rollovers).
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub object_style: Option<String>,
    #[serde(default)]
    pub comments: Vec<String>,
}

fn yes() -> bool {
    true
}

impl Object {
    pub fn new(id: ObjId, kind: ObjKind, transform: Affine, style: Style) -> Object {
        Object {
            id,
            name: String::new(),
            kind,
            transform,
            style,
            locked: false,
            visible: true,
            printable: true,
            effects: Effects::default(),
            data: vec![],
            url: None,
            object_style: None,
            comments: vec![],
        }
    }

    pub fn display_name(&self) -> String {
        if self.name.is_empty() {
            self.kind.type_name().to_string()
        } else {
            self.name.clone()
        }
    }

    pub fn is_container(&self) -> bool {
        matches!(self.kind, ObjKind::Group(_) | ObjKind::PowerClip { .. } | ObjKind::Blend(_))
    }

    pub fn children(&self) -> Vec<&Object> {
        match &self.kind {
            ObjKind::Group(c) => c.iter().collect(),
            ObjKind::PowerClip { frame, contents } => {
                let mut v = vec![frame.as_ref()];
                v.extend(contents.iter());
                v
            }
            ObjKind::Blend(b) => vec![b.start.as_ref(), b.end.as_ref()],
            _ => vec![],
        }
    }

    pub fn children_mut(&mut self) -> Vec<&mut Object> {
        match &mut self.kind {
            ObjKind::Group(c) => c.iter_mut().collect(),
            ObjKind::PowerClip { frame, contents } => {
                let mut v = vec![frame.as_mut()];
                v.extend(contents.iter_mut());
                v
            }
            ObjKind::Blend(b) => vec![b.start.as_mut(), b.end.as_mut()],
            _ => vec![],
        }
    }

    /// Visit this object and all descendants.
    pub fn walk<'a>(&'a self, f: &mut dyn FnMut(&'a Object)) {
        f(self);
        for c in self.children() {
            c.walk(f);
        }
    }

    pub fn walk_mut(&mut self, f: &mut dyn FnMut(&mut Object)) {
        f(self);
        for c in self.children_mut() {
            c.walk_mut(f);
        }
    }

    pub fn find(&self, id: ObjId) -> Option<&Object> {
        if self.id == id {
            return Some(self);
        }
        for c in self.children() {
            if let Some(o) = c.find(id) {
                return Some(o);
            }
        }
        None
    }

    pub fn find_mut(&mut self, id: ObjId) -> Option<&mut Object> {
        if self.id == id {
            return Some(self);
        }
        for c in self.children_mut() {
            if let Some(o) = c.find_mut(id) {
                return Some(o);
            }
        }
        None
    }

    /// Geometry of the object in its local coordinate space (before `transform`).
    /// Containers return an empty path (use `outline_path` for them).
    pub fn local_path(&self) -> BezPath {
        match &self.kind {
            ObjKind::Rect(r) => rect_path(r),
            ObjKind::Ellipse(e) => ellipse_path(e),
            ObjKind::Polygon(p) => polygon_path(p),
            ObjKind::Spiral(s) => spiral_path(s),
            ObjKind::Path(np) => np.to_bez(),
            ObjKind::Text(t) => text::layout(t).to_path(),
            ObjKind::Image(i) => Rect::new(0.0, 0.0, i.w, i.h).to_path(0.1),
            ObjKind::Table(t) => Rect::new(0.0, 0.0, t.width(), t.height()).to_path(0.1),
            ObjKind::Dimension(d) => dimension_path(d),
            ObjKind::Connector(c) => connector_path(c),
            ObjKind::Group(_) | ObjKind::PowerClip { .. } | ObjKind::Blend(_) | ObjKind::Symbol(_) => BezPath::new(),
        }
    }

    /// Final geometry in document coordinates (transform + geometry-warping effects applied).
    pub fn world_path(&self) -> BezPath {
        let p = self.transform * self.local_path();
        self.apply_warps(p)
    }

    pub fn apply_warps(&self, p: BezPath) -> BezPath {
        if !self.effects.warps_geometry() || p.elements().is_empty() {
            return p;
        }
        let bb = p.bounding_box();
        let mut out = p;
        if let Some(env) = &self.effects.envelope {
            out = envelope_warp(&out, bb, env);
        }
        if let Some(d) = &self.effects.distort {
            out = distort_warp(&out, bb, d);
        }
        out
    }

    /// Bounding box in document coordinates (geometry only, no outline width).
    pub fn bounds(&self) -> Rect {
        self.bounds_with(&|_| None)
    }

    /// Bounds with a resolver for symbol instances.
    pub fn bounds_with(&self, sym: &dyn Fn(u64) -> Option<Rect>) -> Rect {
        match &self.kind {
            ObjKind::Group(c) => union_bounds(c.iter().map(|o| o.bounds_with(sym))),
            ObjKind::PowerClip { frame, .. } => frame.bounds_with(sym),
            ObjKind::Blend(b) => b.start.bounds_with(sym).union(b.end.bounds_with(sym)),
            ObjKind::Symbol(id) => sym(*id).map(|r| self.transform.transform_rect_bbox(r)).unwrap_or(Rect::ZERO),
            ObjKind::Text(t) => {
                if self.effects.warps_geometry() {
                    self.world_path().bounding_box()
                } else {
                    let l = text::layout(t);
                    self.transform.transform_rect_bbox(l.bounds)
                }
            }
            _ => {
                let p = self.world_path();
                if p.elements().is_empty() {
                    self.transform.transform_rect_bbox(Rect::ZERO)
                } else {
                    p.bounding_box()
                }
            }
        }
    }

    /// Bounds including outline width and effects (for rendering invalidation and export).
    pub fn visual_bounds(&self) -> Rect {
        let mut b = self.bounds();
        if self.style.outline.is_visible() {
            let w = self.style.outline.width.max(1.0);
            b = b.inflate(w, w);
        }
        if let Some(s) = &self.effects.drop_shadow {
            let sb = b + s.offset;
            b = b.union(sb.inflate(s.feather * 2.0, s.feather * 2.0));
        }
        if let Some(s) = &self.effects.block_shadow {
            b = b.union(b + s.offset);
        }
        if let Some(e) = &self.effects.extrude {
            b = b.union(b + e.depth);
        }
        if let Some(c) = &self.effects.contour {
            if c.dir == ContourDir::Outside {
                let d = c.offset * c.steps as f64;
                b = b.inflate(d, d);
            }
        }
        b
    }

    /// Apply an affine transform in document space.
    pub fn apply_transform(&mut self, a: Affine) {
        match &mut self.kind {
            ObjKind::Group(c) => {
                for o in c.iter_mut() {
                    o.apply_transform(a);
                }
            }
            ObjKind::PowerClip { frame, contents } => {
                frame.apply_transform(a);
                for o in contents.iter_mut() {
                    o.apply_transform(a);
                }
            }
            ObjKind::Blend(b) => {
                b.start.apply_transform(a);
                b.end.apply_transform(a);
            }
            ObjKind::Path(np) => {
                // paths keep their geometry in document space
                let t = a * self.transform;
                np.transform(t);
                self.transform = Affine::IDENTITY;
            }
            ObjKind::Dimension(d) => {
                let t = a * self.transform;
                for p in d.points.iter_mut() {
                    *p = t * *p;
                }
                self.transform = Affine::IDENTITY;
            }
            ObjKind::Connector(c) => {
                let t = a * self.transform;
                c.start.point = t * c.start.point;
                c.end.point = t * c.end.point;
                self.transform = Affine::IDENTITY;
            }
            _ => {
                self.transform = a * self.transform;
                self.absorb_scale();
            }
        }
        if self.style.outline.scale_with_object {
            let (_, _, _, sx, sy) = geom::decompose(a);
            let s = (sx.abs() * sy.abs()).sqrt();
            if s.is_finite() && s > 0.0 {
                self.style.outline.width *= s;
            }
        }
        if let Some(sh) = &mut self.effects.drop_shadow {
            let lin = Affine::new([a.as_coeffs()[0], a.as_coeffs()[1], a.as_coeffs()[2], a.as_coeffs()[3], 0.0, 0.0]);
            let v = lin * Point::new(sh.offset.x, sh.offset.y);
            if a.as_coeffs()[0].abs() > 0.0 {
                sh.offset = v.to_vec2();
            }
        }
    }

    /// Move scale out of the transform into the shape parameters, so rounded corners, outlines and
    /// text frames keep their proportions (as the original does for live shapes).
    pub fn absorb_scale(&mut self) {
        let (rest, sx, sy) = geom::split_scale(self.transform);
        if (sx - 1.0).abs() < 1e-12 && (sy - 1.0).abs() < 1e-12 {
            return;
        }
        if sx < 1e-9 || sy < 1e-9 {
            return;
        }
        match &mut self.kind {
            ObjKind::Rect(r) => {
                r.w *= sx;
                r.h *= sy;
            }
            ObjKind::Ellipse(e) => {
                e.w *= sx;
                e.h *= sy;
            }
            ObjKind::Polygon(p) => {
                p.w *= sx;
                p.h *= sy;
            }
            ObjKind::Spiral(s) => {
                s.w *= sx;
                s.h *= sy;
            }
            ObjKind::Image(i) => {
                i.w *= sx;
                i.h *= sy;
            }
            ObjKind::Table(t) => t.scale(sx, sy),
            ObjKind::Text(t) => match &mut t.kind {
                text::TextKind::Paragraph { width, height } => {
                    *width *= sx;
                    *height *= sy;
                    if let Some(tp) = &mut t.on_path {
                        tp.path.transform(Affine::scale_non_uniform(sx, sy));
                    }
                }
                text::TextKind::Artistic => return, // artistic text scales its glyphs
            },
            _ => return,
        }
        self.transform = rest;
    }

    /// Convert to an editable curve (Convert to Curves).
    pub fn to_curve(&self) -> Option<NodePath> {
        match &self.kind {
            ObjKind::Group(_) | ObjKind::PowerClip { .. } | ObjKind::Symbol(_) | ObjKind::Image(_) | ObjKind::Blend(_) => None,
            _ => Some(NodePath::from_bez(&self.world_path())),
        }
    }

    pub fn is_closed_shape(&self) -> bool {
        match &self.kind {
            ObjKind::Path(np) => np.is_closed(),
            ObjKind::Ellipse(e) => e.kind != EllipseKind::Arc,
            ObjKind::Spiral(_) | ObjKind::Dimension(_) | ObjKind::Connector(_) => false,
            _ => true,
        }
    }
}

pub fn union_bounds(it: impl Iterator<Item = Rect>) -> Rect {
    let mut acc: Option<Rect> = None;
    for r in it {
        if r.width() == 0.0 && r.height() == 0.0 && r.x0 == 0.0 && r.y0 == 0.0 {
            continue;
        }
        acc = Some(acc.map_or(r, |a| a.union(r)));
    }
    acc.unwrap_or(Rect::ZERO)
}

// ------------------------------------------------------------------------------------------
// Shape geometry builders
// ------------------------------------------------------------------------------------------

pub fn rect_path(r: &RectShape) -> BezPath {
    let (w, h) = (r.w, r.h);
    let maxr = (w.abs().min(h.abs())) / 2.0;
    let rad: Vec<f64> = r.radii.iter().map(|x| x.clamp(0.0, maxr)).collect();
    let mut bp = BezPath::new();
    if rad.iter().all(|x| *x <= 1e-9) {
        bp.move_to((0.0, 0.0));
        bp.line_to((w, 0.0));
        bp.line_to((w, h));
        bp.line_to((0.0, h));
        bp.close_path();
        return bp;
    }
    // corners: TL(0,0) TR(w,0) BR(w,h) BL(0,h)
    let corners = [Point::new(0.0, 0.0), Point::new(w, 0.0), Point::new(w, h), Point::new(0.0, h)];
    // direction from corner to next corner and from previous
    let k = 0.5522847498;
    let mut started = false;
    for i in 0..4 {
        let c = corners[i];
        let prev = corners[(i + 3) % 4];
        let next = corners[(i + 1) % 4];
        let rr = rad[i];
        let din = (c - prev).normalize();
        let dout = (next - c).normalize();
        let a = c - din * rr; // end of incoming edge
        let b = c + dout * rr; // start of outgoing edge
        if !started {
            bp.move_to(a);
            started = true;
        } else {
            bp.line_to(a);
        }
        if rr > 1e-9 {
            match r.corner {
                CornerType::Round => {
                    bp.curve_to(a + din * (rr * k), b - dout * (rr * k), b);
                }
                CornerType::Chamfer => {
                    bp.line_to(b);
                }
                CornerType::Scallop => {
                    // concave quarter circle centered at the corner
                    let ctr = c;
                    let a2 = a - ctr;
                    let b2 = b - ctr;
                    bp.curve_to(ctr + a2 * (1.0 - k) + (b2 * 0.0), ctr + b2 * (1.0 - k), b);
                }
            }
        }
    }
    bp.close_path();
    bp
}

pub fn ellipse_path(e: &EllipseShape) -> BezPath {
    let c = Point::new(e.w / 2.0, e.h / 2.0);
    let rx = e.w / 2.0;
    let ry = e.h / 2.0;
    match e.kind {
        EllipseKind::Ellipse => kurbo::Ellipse::new(c, (rx, ry), 0.0).to_path(0.01),
        _ => {
            // angles are counter-clockwise in the y-up convention → negate for y-down
            let mut a0 = -e.start.to_radians();
            let mut a1 = -e.end.to_radians();
            if e.clockwise {
                std::mem::swap(&mut a0, &mut a1);
            }
            let mut sweep = a1 - a0;
            while sweep >= 0.0 {
                sweep -= 2.0 * PI;
            }
            if sweep.abs() < 1e-9 {
                sweep = -2.0 * PI;
            }
            let arc = kurbo::Arc::new(c, (rx, ry), a0, sweep, 0.0);
            let mut bp = BezPath::new();
            let p0 = Point::new(c.x + rx * a0.cos(), c.y + ry * a0.sin());
            if e.kind == EllipseKind::Pie {
                bp.move_to(c);
                bp.line_to(p0);
            } else {
                bp.move_to(p0);
            }
            arc.to_cubic_beziers(0.01, |p1, p2, p3| bp.curve_to(p1, p2, p3));
            if e.kind == EllipseKind::Pie {
                bp.close_path();
            }
            bp
        }
    }
}

pub fn polygon_path(p: &PolygonShape) -> BezPath {
    let n = p.sides.max(3) as usize;
    let c = Point::new(p.w / 2.0, p.h / 2.0);
    let rx = p.w / 2.0;
    let ry = p.h / 2.0;
    let mut bp = BezPath::new();
    let start = -PI / 2.0;
    match p.kind {
        PolygonKind::Polygon => {
            for i in 0..n {
                let a = start + 2.0 * PI * i as f64 / n as f64;
                let q = Point::new(c.x + rx * a.cos(), c.y + ry * a.sin());
                if i == 0 {
                    bp.move_to(q)
                } else {
                    bp.line_to(q)
                }
            }
            bp.close_path();
        }
        PolygonKind::Star => {
            // inner radius from sharpness: 0% → regular polygon midpoints, 100% → very thin
            let base_inner = (PI / n as f64).cos();
            let sharp = (p.sharpness / 100.0).clamp(0.0, 0.99);
            let inner = base_inner * (1.0 - sharp);
            for i in 0..(2 * n) {
                let a = start + PI * i as f64 / n as f64;
                let r = if i % 2 == 0 { 1.0 } else { inner };
                let q = Point::new(c.x + rx * r * a.cos(), c.y + ry * r * a.sin());
                if i == 0 {
                    bp.move_to(q)
                } else {
                    bp.line_to(q)
                }
            }
            bp.close_path();
        }
        PolygonKind::ComplexStar => {
            let n = n.max(5);
            let skip = (((p.sharpness / 100.0) * ((n - 1) / 2) as f64).round() as usize).clamp(2, (n - 1) / 2);
            let pts: Vec<Point> = (0..n)
                .map(|i| {
                    let a = start + 2.0 * PI * i as f64 / n as f64;
                    Point::new(c.x + rx * a.cos(), c.y + ry * a.sin())
                })
                .collect();
            let g = gcd(n, skip);
            for s in 0..g {
                let mut idx = s;
                bp.move_to(pts[idx]);
                for _ in 0..(n / g) {
                    idx = (idx + skip) % n;
                    bp.line_to(pts[idx]);
                }
                bp.close_path();
            }
        }
    }
    bp
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

pub fn spiral_path(s: &SpiralShape) -> BezPath {
    let c = Point::new(s.w / 2.0, s.h / 2.0);
    let rx = s.w / 2.0;
    let ry = s.h / 2.0;
    let total = s.revolutions.max(0.25) * 2.0 * PI;
    let steps = (s.revolutions.max(0.25) * 64.0) as usize + 8;
    let mut pts = vec![];
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let ang = t * total;
        let r = if s.logarithmic {
            let k = (s.expansion.clamp(1.0, 100.0) / 100.0) * 4.0 + 0.2;
            ((k * t).exp() - 1.0) / (k.exp() - 1.0)
        } else {
            t
        };
        pts.push(Point::new(c.x + rx * r * ang.cos(), c.y + ry * r * ang.sin()));
    }
    let np = crate::path::fit_freehand(&pts, 0.0, false);
    np.to_bez()
}

fn fmt_measure(d: &DimensionObj, v_pt: f64, angle: bool) -> String {
    if !d.text.is_empty() {
        return d.text.clone();
    }
    let s = if angle {
        format!("{:.*}°", d.precision, v_pt)
    } else {
        let v = d.unit.to_unit(v_pt, 96.0);
        if d.show_units {
            format!("{:.*} {}", d.precision, v, d.unit.suffix())
        } else {
            format!("{:.*}", d.precision, v)
        }
    };
    format!("{}{}{}", d.prefix, s, d.suffix)
}

pub fn dimension_label(d: &DimensionObj) -> (String, Point, f64) {
    let pts = &d.points;
    match d.kind {
        DimensionKind::Parallel | DimensionKind::Segment => {
            if pts.len() < 2 {
                return (String::new(), Point::ZERO, 0.0);
            }
            let (a, b) = (pts[0], pts[1]);
            let lp = pts.get(2).copied().unwrap_or(a.midpoint(b));
            let dir = (b - a).normalize();
            let normal = Vec2::new(-dir.y, dir.x);
            let off = (lp - a).dot(normal);
            let mid = a.midpoint(b) + normal * off;
            (fmt_measure(d, (b - a).hypot(), false), mid, dir.y.atan2(dir.x))
        }
        DimensionKind::HorizontalVertical => {
            if pts.len() < 2 {
                return (String::new(), Point::ZERO, 0.0);
            }
            let (a, b) = (pts[0], pts[1]);
            let lp = pts.get(2).copied().unwrap_or(a.midpoint(b));
            let horizontal = (lp.y - a.y.max(b.y)).abs() > (lp.x - a.x.max(b.x)).abs() || (lp.y < a.y.min(b.y) || lp.y > a.y.max(b.y));
            if horizontal {
                (fmt_measure(d, (b.x - a.x).abs(), false), Point::new((a.x + b.x) / 2.0, lp.y), 0.0)
            } else {
                (fmt_measure(d, (b.y - a.y).abs(), false), Point::new(lp.x, (a.y + b.y) / 2.0), -PI / 2.0)
            }
        }
        DimensionKind::Angular => {
            if pts.len() < 3 {
                return (String::new(), Point::ZERO, 0.0);
            }
            let (v, a, b) = (pts[0], pts[1], pts[2]);
            let a1 = (a - v).atan2();
            let a2 = (b - v).atan2();
            let mut ang = (a2 - a1).to_degrees().abs();
            if ang > 180.0 {
                ang = 360.0 - ang;
            }
            let lp = pts.get(3).copied().unwrap_or(v + ((a - v).normalize() + (b - v).normalize()) * 30.0);
            (fmt_measure(d, ang, true), lp, 0.0)
        }
        DimensionKind::Callout => {
            let lp = pts.last().copied().unwrap_or(Point::ZERO);
            (if d.text.is_empty() { "Callout".to_string() } else { d.text.clone() }, lp, 0.0)
        }
    }
}

pub fn dimension_path(d: &DimensionObj) -> BezPath {
    let mut bp = BezPath::new();
    let pts = &d.points;
    let arrow = |bp: &mut BezPath, tip: Point, dir: Vec2, size: f64| {
        let dir = dir.normalize();
        let n = Vec2::new(-dir.y, dir.x);
        bp.move_to(tip);
        bp.line_to(tip - dir * size + n * size * 0.35);
        bp.move_to(tip);
        bp.line_to(tip - dir * size - n * size * 0.35);
    };
    let sz = (d.font_size * 0.5).max(3.0);
    match d.kind {
        DimensionKind::Parallel | DimensionKind::Segment if pts.len() >= 2 => {
            let (a, b) = (pts[0], pts[1]);
            let lp = pts.get(2).copied().unwrap_or(a.midpoint(b));
            let dir = (b - a).normalize();
            let normal = Vec2::new(-dir.y, dir.x);
            let off = (lp - a).dot(normal);
            let a2 = a + normal * off;
            let b2 = b + normal * off;
            let ext = normal * (2.0 * off.signum());
            bp.move_to(a);
            bp.line_to(a2 + ext);
            bp.move_to(b);
            bp.line_to(b2 + ext);
            bp.move_to(a2);
            bp.line_to(b2);
            arrow(&mut bp, a2, a2 - b2, sz);
            arrow(&mut bp, b2, b2 - a2, sz);
        }
        DimensionKind::HorizontalVertical if pts.len() >= 2 => {
            let (a, b) = (pts[0], pts[1]);
            let (_, mid, ang) = dimension_label(d);
            if ang == 0.0 {
                let y = mid.y;
                bp.move_to(a);
                bp.line_to((a.x, y));
                bp.move_to(b);
                bp.line_to((b.x, y));
                bp.move_to((a.x, y));
                bp.line_to((b.x, y));
                arrow(&mut bp, Point::new(a.x, y), Vec2::new(a.x - b.x, 0.0), sz);
                arrow(&mut bp, Point::new(b.x, y), Vec2::new(b.x - a.x, 0.0), sz);
            } else {
                let x = mid.x;
                bp.move_to(a);
                bp.line_to((x, a.y));
                bp.move_to(b);
                bp.line_to((x, b.y));
                bp.move_to((x, a.y));
                bp.line_to((x, b.y));
                arrow(&mut bp, Point::new(x, a.y), Vec2::new(0.0, a.y - b.y), sz);
                arrow(&mut bp, Point::new(x, b.y), Vec2::new(0.0, b.y - a.y), sz);
            }
        }
        DimensionKind::Angular if pts.len() >= 3 => {
            let (v, a, b) = (pts[0], pts[1], pts[2]);
            bp.move_to(a);
            bp.line_to(v);
            bp.line_to(b);
            let r = pts.get(3).map(|lp| (*lp - v).hypot() * 0.8).unwrap_or(30.0);
            let a1 = (a - v).atan2();
            let mut a2 = (b - v).atan2();
            let mut sweep = a2 - a1;
            if sweep > PI {
                sweep -= 2.0 * PI;
            }
            if sweep < -PI {
                sweep += 2.0 * PI;
            }
            a2 = a1 + sweep;
            let arc = kurbo::Arc::new(v, (r, r), a1, a2 - a1, 0.0);
            let p0 = Point::new(v.x + r * a1.cos(), v.y + r * a1.sin());
            bp.move_to(p0);
            arc.to_cubic_beziers(0.01, |p1, p2, p3| bp.curve_to(p1, p2, p3));
        }
        DimensionKind::Callout if pts.len() >= 2 => {
            bp.move_to(pts[0]);
            for p in &pts[1..] {
                bp.line_to(*p);
            }
            let circle = kurbo::Circle::new(pts[0], 1.5).to_path(0.05);
            bp.extend(circle.elements().iter().copied());
        }
        _ => {}
    }
    bp
}

pub fn connector_path(c: &ConnectorObj) -> BezPath {
    let a = c.start.point;
    let b = c.end.point;
    let mut bp = BezPath::new();
    match c.kind {
        ConnectorKind::Straight => {
            bp.move_to(a);
            bp.line_to(b);
        }
        ConnectorKind::RightAngle | ConnectorKind::RoundedRightAngle => {
            let midx = (a.x + b.x) / 2.0;
            let pts = [a, Point::new(midx, a.y), Point::new(midx, b.y), b];
            if c.kind == ConnectorKind::RightAngle || c.corner_radius <= 0.0 {
                bp.move_to(pts[0]);
                for p in &pts[1..] {
                    bp.line_to(*p);
                }
            } else {
                let r = c.corner_radius;
                bp.move_to(pts[0]);
                for i in 1..pts.len() - 1 {
                    let (p0, p1, p2) = (pts[i - 1], pts[i], pts[i + 1]);
                    let d0 = (p1 - p0).hypot();
                    let d1 = (p2 - p1).hypot();
                    let rr = r.min(d0 / 2.0).min(d1 / 2.0);
                    if rr < 1e-6 {
                        bp.line_to(p1);
                        continue;
                    }
                    let a1 = p1 - (p1 - p0).normalize() * rr;
                    let b1 = p1 + (p2 - p1).normalize() * rr;
                    bp.line_to(a1);
                    bp.quad_to(p1, b1);
                }
                bp.line_to(*pts.last().unwrap());
            }
        }
    }
    bp
}

// ------------------------------------------------------------------------------------------
// Warps
// ------------------------------------------------------------------------------------------

fn map_points(path: &BezPath, f: impl Fn(Point) -> Point) -> BezPath {
    // subdivide curves first so warps look smooth
    let mut fine = BezPath::new();
    for el in path.elements() {
        match *el {
            kurbo::PathEl::MoveTo(p) => fine.move_to(p),
            kurbo::PathEl::LineTo(p) => {
                let last = fine.elements().last().and_then(|e| e.end_point()).unwrap_or(p);
                let n = (((p - last).hypot()) / 6.0).ceil().clamp(1.0, 64.0) as usize;
                for i in 1..=n {
                    fine.line_to(last.lerp(p, i as f64 / n as f64));
                }
            }
            kurbo::PathEl::QuadTo(c, p) => fine.quad_to(c, p),
            kurbo::PathEl::CurveTo(c1, c2, p) => {
                let last = fine.elements().last().and_then(|e| e.end_point()).unwrap_or(p);
                let cb = kurbo::CubicBez::new(last, c1, c2, p);
                let n = 4;
                for i in 0..n {
                    let s = cb.subsegment((i as f64 / n as f64)..((i + 1) as f64 / n as f64));
                    fine.curve_to(s.p1, s.p2, s.p3);
                }
            }
            kurbo::PathEl::ClosePath => fine.close_path(),
        }
    }
    let mut out = BezPath::new();
    for el in fine.elements() {
        out.push(match *el {
            kurbo::PathEl::MoveTo(p) => kurbo::PathEl::MoveTo(f(p)),
            kurbo::PathEl::LineTo(p) => kurbo::PathEl::LineTo(f(p)),
            kurbo::PathEl::QuadTo(c, p) => kurbo::PathEl::QuadTo(f(c), f(p)),
            kurbo::PathEl::CurveTo(a, b, p) => kurbo::PathEl::CurveTo(f(a), f(b), f(p)),
            kurbo::PathEl::ClosePath => kurbo::PathEl::ClosePath,
        });
    }
    out
}

pub fn envelope_warp(path: &BezPath, bb: Rect, env: &EnvelopeEffect) -> BezPath {
    if bb.width() <= 0.0 || bb.height() <= 0.0 {
        return path.clone();
    }
    let w = bb.width();
    let h = bb.height();
    let base = [
        Point::new(0.0, 0.0),
        Point::new(0.5, 0.0),
        Point::new(1.0, 0.0),
        Point::new(1.0, 0.5),
        Point::new(1.0, 1.0),
        Point::new(0.5, 1.0),
        Point::new(0.0, 1.0),
        Point::new(0.0, 0.5),
    ];
    let hp: Vec<Point> = (0..8).map(|i| base[i] + env.handles[i]).collect();
    // quadratic edge curves through corner, midpoint (as interpolating point), corner
    let quad = |p0: Point, pm: Point, p1: Point, t: f64| -> Point {
        let c = Point::new(2.0 * pm.x - 0.5 * (p0.x + p1.x), 2.0 * pm.y - 0.5 * (p0.y + p1.y));
        let mt = 1.0 - t;
        Point::new(mt * mt * p0.x + 2.0 * mt * t * c.x + t * t * p1.x, mt * mt * p0.y + 2.0 * mt * t * c.y + t * t * p1.y)
    };
    map_points(path, |p| {
        let u = (p.x - bb.x0) / w;
        let v = (p.y - bb.y0) / h;
        let top = quad(hp[0], hp[1], hp[2], u);
        let bottom = quad(hp[6], hp[5], hp[4], u);
        let left = quad(hp[0], hp[7], hp[6], v);
        let right = quad(hp[2], hp[3], hp[4], v);
        // Coons patch
        let c00 = hp[0];
        let c10 = hp[2];
        let c01 = hp[6];
        let c11 = hp[4];
        let x = (1.0 - v) * top.x + v * bottom.x + (1.0 - u) * left.x + u * right.x
            - ((1.0 - u) * (1.0 - v) * c00.x + u * (1.0 - v) * c10.x + (1.0 - u) * v * c01.x + u * v * c11.x);
        let y = (1.0 - v) * top.y + v * bottom.y + (1.0 - u) * left.y + u * right.y
            - ((1.0 - u) * (1.0 - v) * c00.y + u * (1.0 - v) * c10.y + (1.0 - u) * v * c01.y + u * v * c11.y);
        Point::new(bb.x0 + x * w, bb.y0 + y * h)
    })
}

pub fn distort_warp(path: &BezPath, bb: Rect, d: &DistortEffect) -> BezPath {
    let c = Point::new(bb.x0 + d.center.x * bb.width(), bb.y0 + d.center.y * bb.height());
    let rmax = (bb.width().max(bb.height()) / 2.0).max(1.0);
    match d.kind {
        DistortKind::PushPull => {
            let amt = d.amount / 100.0;
            map_points(path, |p| {
                let v = p - c;
                let r = v.hypot() / rmax;
                let f = 1.0 + amt * (1.0 - r.min(1.0)) * 0.8;
                c + v * f
            })
        }
        DistortKind::Zipper => {
            let amp = d.amount / 100.0 * rmax * 0.15;
            let freq = d.frequency.max(1.0);
            map_points(path, |p| {
                let v = p - c;
                let ang = v.y.atan2(v.x);
                let r = v.hypot();
                let wav = (ang * freq).sin() * amp;
                let nr = (r + wav).max(0.0);
                c + Vec2::new(ang.cos(), ang.sin()) * nr
            })
        }
        DistortKind::Twister => {
            let rot = d.amount.to_radians();
            map_points(path, |p| {
                let v = p - c;
                let r = v.hypot() / rmax;
                let a = rot * (1.0 - r.min(1.0));
                let (s, co) = a.sin_cos();
                c + Vec2::new(v.x * co - v.y * s, v.x * s + v.y * co)
            })
        }
    }
}

/// Interpolate two objects for a blend step (t in 0..1). Geometry is matched by flattening both
/// to the same number of points per subpath.
pub fn blend_step(a: &Object, b: &Object, t: f64, id: ObjId, rotation: f64) -> Object {
    let pa = a.world_path();
    let pb = b.world_path();
    let ra = resample(&pa, 96);
    let rb = resample(&pb, 96);
    let mut np = crate::path::NodePath::new();
    let n = ra.len().min(rb.len());
    for k in 0..n {
        let (ca, closed) = &ra[k];
        let (cb, _) = &rb[k];
        let pts: Vec<Point> = ca.iter().zip(cb.iter()).map(|(p, q)| p.lerp(*q, t)).collect();
        np.subpaths.push(crate::path::polyline(&pts, *closed).subpaths.remove(0));
    }
    let mut bp = np.to_bez();
    if rotation != 0.0 {
        let c = bp.bounding_box().center();
        bp = Affine::rotate_about((rotation * t).to_radians(), c) * bp;
    }
    let mut style = a.style.clone();
    style.fill = lerp_fill(&a.style.fill, &b.style.fill, t);
    if let (Some(ca), Some(cb)) = (a.style.outline.color, b.style.outline.color) {
        style.outline.color = Some(Color::lerp(ca, cb, t));
    }
    style.outline.width = a.style.outline.width + (b.style.outline.width - a.style.outline.width) * t;
    let mut o = Object::new(id, ObjKind::Path(NodePath::from_bez(&bp)), Affine::IDENTITY, style);
    o.name = "Blend step".into();
    o
}

pub fn lerp_fill(a: &crate::style::Fill, b: &crate::style::Fill, t: f64) -> crate::style::Fill {
    use crate::style::Fill;
    match (a, b) {
        (Fill::Solid(x), Fill::Solid(y)) => Fill::Solid(Color::lerp(*x, *y, t)),
        (Fill::Solid(x), Fill::None) => Fill::Solid(x.with_alpha(((1.0 - t) * x.a as f64) as u8)),
        (Fill::None, Fill::Solid(y)) => Fill::Solid(y.with_alpha((t * y.a as f64) as u8)),
        _ => {
            if t < 0.5 {
                a.clone()
            } else {
                b.clone()
            }
        }
    }
}

/// Resample each subpath to `n` evenly spaced points.
pub fn resample(path: &BezPath, n: usize) -> Vec<(Vec<Point>, bool)> {
    let polys = geom::flatten_polylines(path, 0.2);
    polys
        .into_iter()
        .map(|(pts, closed)| {
            let mut pts = pts;
            if closed {
                pts.push(pts[0]);
            }
            let mut acc = vec![0.0];
            for w in pts.windows(2) {
                acc.push(acc.last().unwrap() + (w[1] - w[0]).hypot());
            }
            let total = *acc.last().unwrap();
            let mut out = vec![];
            let m = if closed { n } else { n + 1 };
            for i in 0..m {
                let s = total * i as f64 / n as f64;
                let mut k = 0;
                while k + 1 < acc.len() - 1 && acc[k + 1] < s {
                    k += 1;
                }
                let seg = (acc[k + 1] - acc[k]).max(1e-12);
                let tt = ((s - acc[k]) / seg).clamp(0.0, 1.0);
                out.push(pts[k].lerp(pts[(k + 1).min(pts.len() - 1)], tt));
            }
            (out, closed)
        })
        .collect()
}
