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
    /// Merged cells: the top-left cell spans `rspan` rows and `cspan` columns; the cells it
    /// covers keep a span of 1 and no text.
    #[serde(default = "one_usize")]
    pub rspan: usize,
    #[serde(default = "one_usize")]
    pub cspan: usize,
}

fn one_usize() -> usize {
    1
}

impl TableCell {
    pub fn empty() -> TableCell {
        TableCell { text: String::new(), fill: None, rspan: 1, cspan: 1 }
    }
}

/// A block of cells: first row, first column, last row, last column (inclusive).
pub type CellRange = [usize; 4];

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
    /// Cell margins: top, bottom, left, right (points).
    #[serde(default = "cell_margins")]
    pub margins: [f64; 4],
    /// Rows grow to fit their text while typing.
    #[serde(default)]
    pub auto_resize: bool,
    /// Separated cell borders: horizontal and vertical spacing between the cells.
    #[serde(default)]
    pub spacing: Option<(f64, f64)>,
}

fn cell_margins() -> [f64; 4] {
    [2.0; 4]
}

impl TableObj {
    /// The area of the cell anchored at (r, c) inside its margins and spacing.
    pub fn text_rect(&self, r: usize, c: usize) -> Rect {
        let rc = self.cell_box(r, c);
        let [t, b, l, rr] = self.margins;
        Rect::new(rc.x0 + l, rc.y0 + t, (rc.x1 - rr).max(rc.x0 + l + 1.0), (rc.y1 - b).max(rc.y0 + t + 1.0))
    }
    /// The box of the cell anchored at (r, c): its span, inset by half the spacing when the
    /// cell borders are separated.
    pub fn cell_box(&self, r: usize, c: usize) -> Rect {
        let rc = self.span_rect(r, c);
        match self.spacing {
            Some((h, v)) => Rect::new(rc.x0 + h / 2.0, rc.y0 + v / 2.0, (rc.x1 - h / 2.0).max(rc.x0 + h / 2.0), (rc.y1 - v / 2.0).max(rc.y0 + v / 2.0)),
            None => rc,
        }
    }
    /// Automatically resize cells while typing: each row is made tall enough for its text.
    pub fn fit_rows(&mut self) {
        for r in 0..self.rows {
            let mut need: f64 = 0.0;
            for c in 0..self.cols {
                let cell = self.cell(r, c);
                if cell.text.is_empty() || cell.rspan != 1 {
                    continue;
                }
                let tr = self.text_rect(r, c);
                let mut t = text::TextObj::paragraph(&cell.text, tr.width().max(1.0), 1e6, self.text_format.clone());
                t.align = text::TextAlign::Left;
                let lay = text::layout(&t);
                let h = lay.to_path().bounding_box().y1.max(self.text_format.size * 1.2);
                need = need.max(h + self.margins[0] + self.margins[1] + self.spacing.map(|s| s.1).unwrap_or(0.0));
            }
            if need > self.row_heights[r] {
                self.row_heights[r] = need;
            }
        }
    }
    pub fn new(rows: usize, cols: usize, w: f64, h: f64) -> TableObj {
        TableObj {
            rows,
            cols,
            col_widths: vec![w / cols as f64; cols],
            row_heights: vec![h / rows as f64; rows],
            cells: vec![TableCell::empty(); rows * cols],
            border: Color::BLACK,
            border_width: 0.5669,
            text_format: text::CharFormat { size: 10.0, ..Default::default() },
            margins: cell_margins(),
            auto_resize: false,
            spacing: None,
        }
    }
    pub fn width(&self) -> f64 {
        self.col_widths.iter().sum()
    }
    pub fn height(&self) -> f64 {
        self.row_heights.iter().sum()
    }
    pub fn cell(&self, r: usize, c: usize) -> &TableCell {
        &self.cells[r * self.cols + c]
    }
    pub fn cell_mut(&mut self, r: usize, c: usize) -> &mut TableCell {
        &mut self.cells[r * self.cols + c]
    }
    /// The rectangle of the single grid cell (r, c).
    pub fn cell_rect(&self, r: usize, c: usize) -> Rect {
        let x0: f64 = self.col_widths[..c].iter().sum();
        let y0: f64 = self.row_heights[..r].iter().sum();
        Rect::new(x0, y0, x0 + self.col_widths[c], y0 + self.row_heights[r])
    }
    /// The rectangle of the cell anchored at (r, c), merged cells included.
    pub fn span_rect(&self, r: usize, c: usize) -> Rect {
        let cl = self.cell(r, c);
        let (r1, c1) = ((r + cl.rspan).min(self.rows), (c + cl.cspan).min(self.cols));
        let x0: f64 = self.col_widths[..c].iter().sum();
        let y0: f64 = self.row_heights[..r].iter().sum();
        let x1: f64 = x0 + self.col_widths[c..c1].iter().sum::<f64>();
        let y1: f64 = y0 + self.row_heights[r..r1].iter().sum::<f64>();
        Rect::new(x0, y0, x1, y1)
    }
    /// The cell whose (merged) area holds grid cell (r, c).
    pub fn anchor_of(&self, r: usize, c: usize) -> (usize, usize) {
        for ar in 0..=r.min(self.rows.saturating_sub(1)) {
            for ac in 0..=c.min(self.cols.saturating_sub(1)) {
                let cl = self.cell(ar, ac);
                if (cl.rspan > 1 || cl.cspan > 1) && ar + cl.rspan > r && ac + cl.cspan > c {
                    return (ar, ac);
                }
            }
        }
        (r, c)
    }
    pub fn is_covered(&self, r: usize, c: usize) -> bool {
        self.anchor_of(r, c) != (r, c)
    }
    /// The cells that are drawn: every cell not covered by a merged cell.
    pub fn anchors(&self) -> Vec<(usize, usize)> {
        let mut covered = vec![false; self.rows * self.cols];
        let mut out = vec![];
        for r in 0..self.rows {
            for c in 0..self.cols {
                if covered[r * self.cols + c] {
                    continue;
                }
                out.push((r, c));
                let cl = self.cell(r, c);
                for rr in r..(r + cl.rspan).min(self.rows) {
                    for cc in c..(c + cl.cspan).min(self.cols) {
                        covered[rr * self.cols + cc] = true;
                    }
                }
            }
        }
        out
    }
    /// Cell borders: the outline of every drawn cell.
    pub fn grid_path(&self) -> BezPath {
        let mut bp = BezPath::new();
        for (r, c) in self.anchors() {
            let rc = self.span_rect(r, c);
            bp.move_to((rc.x0, rc.y0));
            bp.line_to((rc.x1, rc.y0));
            bp.line_to((rc.x1, rc.y1));
            bp.line_to((rc.x0, rc.y1));
            bp.close_path();
        }
        bp
    }
    pub fn cell_at(&self, p: Point) -> Option<(usize, usize)> {
        let mut y = 0.0;
        for r in 0..self.rows {
            let mut x = 0.0;
            for c in 0..self.cols {
                let rc = Rect::new(x, y, x + self.col_widths[c], y + self.row_heights[r]);
                if rc.contains(p) {
                    return Some(self.anchor_of(r, c));
                }
                x += self.col_widths[c];
            }
            y += self.row_heights[r];
        }
        None
    }
    /// The block from cell `a` to cell `b`, grown until no merged cell sticks out of it.
    pub fn range(&self, a: (usize, usize), b: (usize, usize)) -> CellRange {
        let lim = |v: usize, n: usize| v.min(n.saturating_sub(1));
        let mut g = [lim(a.0.min(b.0), self.rows), lim(a.1.min(b.1), self.cols), lim(a.0.max(b.0), self.rows), lim(a.1.max(b.1), self.cols)];
        loop {
            let before = g;
            for r in g[0]..=g[2] {
                for c in g[1]..=g[3] {
                    let (ar, ac) = self.anchor_of(r, c);
                    let cl = self.cell(ar, ac);
                    g[0] = g[0].min(ar);
                    g[1] = g[1].min(ac);
                    g[2] = g[2].max(ar + cl.rspan - 1);
                    g[3] = g[3].max(ac + cl.cspan - 1);
                }
            }
            if g == before {
                return g;
            }
        }
    }
    pub fn range_rect(&self, g: CellRange) -> Rect {
        self.cell_rect(g[0], g[1]).union(self.cell_rect(g[2], g[3]))
    }
    /// The drawn cells inside a block.
    pub fn anchors_in(&self, g: CellRange) -> Vec<(usize, usize)> {
        self.anchors().into_iter().filter(|(r, c)| *r >= g[0] && *r <= g[2] && *c >= g[1] && *c <= g[3]).collect()
    }
    pub fn scale(&mut self, sx: f64, sy: f64) {
        for w in self.col_widths.iter_mut() {
            *w *= sx;
        }
        for h in self.row_heights.iter_mut() {
            *h *= sy;
        }
    }
    /// Rows become columns (used to write the column operations once, as row operations).
    pub fn transposed(&self) -> TableObj {
        let mut cells = Vec::with_capacity(self.cells.len());
        for c in 0..self.cols {
            for r in 0..self.rows {
                let mut cl = self.cell(r, c).clone();
                std::mem::swap(&mut cl.rspan, &mut cl.cspan);
                cells.push(cl);
            }
        }
        TableObj { rows: self.cols, cols: self.rows, col_widths: self.row_heights.clone(), row_heights: self.col_widths.clone(), cells, border: self.border, border_width: self.border_width, text_format: self.text_format.clone(), margins: self.margins, auto_resize: self.auto_resize, spacing: self.spacing }
    }
    fn by_columns(&mut self, f: impl FnOnce(&mut TableObj)) {
        let mut t = self.transposed();
        f(&mut t);
        *self = t.transposed();
    }
    pub fn insert_row(&mut self, at: usize) {
        let at = at.min(self.rows);
        // merged cells that cross the new row grow over it
        for (r, c) in self.anchors() {
            let cl = self.cell_mut(r, c);
            if r < at && r + cl.rspan > at {
                cl.rspan += 1;
            }
        }
        let h = self.row_heights.get(at.min(self.rows.saturating_sub(1))).copied().unwrap_or(20.0);
        self.row_heights.insert(at, h);
        for _ in 0..self.cols {
            self.cells.insert(at * self.cols, TableCell::empty());
        }
        self.rows += 1;
    }
    pub fn insert_col(&mut self, at: usize) {
        self.by_columns(|t| t.insert_row(at));
    }
    pub fn delete_row(&mut self, at: usize) {
        if self.rows <= 1 || at >= self.rows {
            return;
        }
        for (r, c) in self.anchors() {
            let cl = self.cell(r, c).clone();
            if r < at && r + cl.rspan > at {
                self.cell_mut(r, c).rspan -= 1;
            } else if r == at && cl.rspan > 1 {
                // the merged cell moves down to the next row
                let below = self.cell_mut(r + 1, c);
                *below = TableCell { rspan: cl.rspan - 1, ..cl };
            }
        }
        self.row_heights.remove(at);
        self.cells.drain(at * self.cols..(at + 1) * self.cols);
        self.rows -= 1;
    }
    pub fn delete_col(&mut self, at: usize) {
        if self.cols <= 1 || at >= self.cols {
            return;
        }
        self.by_columns(|t| t.delete_row(at));
    }
    /// Table ▸ Merge Cells: the block becomes one cell holding the text of all of them.
    pub fn merge(&mut self, g: CellRange) {
        let g = self.range((g[0], g[1]), (g[2], g[3]));
        let texts: Vec<String> = self.anchors_in(g).iter().map(|(r, c)| self.cell(*r, *c).text.clone()).filter(|t| !t.is_empty()).collect();
        for r in g[0]..=g[2] {
            for c in g[1]..=g[3] {
                let cl = self.cell_mut(r, c);
                cl.rspan = 1;
                cl.cspan = 1;
                if (r, c) != (g[0], g[1]) {
                    cl.text.clear();
                }
            }
        }
        let a = self.cell_mut(g[0], g[1]);
        a.rspan = g[2] - g[0] + 1;
        a.cspan = g[3] - g[1] + 1;
        a.text = texts.join(" ");
    }
    /// Table ▸ Unmerge Cells.
    pub fn unmerge(&mut self, g: CellRange) {
        for (r, c) in self.anchors_in(g) {
            let cl = self.cell_mut(r, c);
            cl.rspan = 1;
            cl.cspan = 1;
        }
    }
    /// Table ▸ Split into Rows: the cell at (r, c) becomes `n` cells stacked; rows are added
    /// when it spans fewer than `n` rows, and the other cells of those rows stay whole.
    pub fn split_rows(&mut self, r: usize, c: usize, n: usize) {
        let (r, c) = self.anchor_of(r, c);
        let n = n.max(1);
        let (mut span, cspan) = (self.cell(r, c).rspan, self.cell(r, c).cspan);
        if span < n {
            let extra = n - span;
            let last = r + span - 1;
            let others: Vec<(usize, usize)> = self.anchors().into_iter().filter(|&(ar, ac)| (ar, ac) != (r, c) && ar <= last && ar + self.cell(ar, ac).rspan > last).collect();
            let h = self.row_heights[last] / (extra + 1) as f64;
            self.row_heights[last] = h;
            for _ in 0..extra {
                self.row_heights.insert(last + 1, h);
                for _ in 0..self.cols {
                    self.cells.insert((last + 1) * self.cols, TableCell::empty());
                }
                self.rows += 1;
            }
            for (ar, ac) in others {
                self.cell_mut(ar, ac).rspan += extra;
            }
            span = n;
        }
        let (base, rem) = (span / n, span % n);
        let mut pos = r;
        for k in 0..n {
            let size = base + usize::from(k < rem);
            let cl = self.cell_mut(pos, c);
            cl.rspan = size;
            cl.cspan = cspan;
            pos += size;
        }
    }
    /// Table ▸ Split into Columns.
    pub fn split_cols(&mut self, r: usize, c: usize, n: usize) {
        self.by_columns(|t| t.split_rows(c, r, n));
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
    /// Show Leading Zero: 0.5 instead of .5.
    #[serde(default = "yes")]
    pub leading_zero: bool,
    /// Dynamic Dimensioning: the value follows the measured objects (off = frozen text).
    #[serde(default = "yes")]
    pub dynamic: bool,
    /// Extension lines: gap from the object and extension past the dimension line, points.
    #[serde(default)]
    pub ext_gap: f64,
    #[serde(default)]
    pub ext_len: Option<f64>,
    /// Arrowheads of the dimension line (`None` = no arrowheads).
    #[serde(default = "dim_arrow")]
    pub arrow: Option<crate::style::ArrowHead>,
    /// Callout shape around the text: 0 none, 1 rectangle, 2 rounded rectangle, 3 ellipse.
    #[serde(default)]
    pub callout_shape: u8,
    /// Gap between the callout text and its line or shape, points.
    #[serde(default = "callout_gap")]
    pub callout_gap: f64,
}

fn dim_arrow() -> Option<crate::style::ArrowHead> {
    Some(crate::style::ArrowHead::OpenTriangle)
}

fn callout_gap() -> f64 {
    2.0
}

impl Default for DimensionObj {
    fn default() -> Self {
        DimensionObj {
            kind: DimensionKind::Parallel,
            points: vec![],
            text: String::new(),
            precision: 2,
            prefix: String::new(),
            suffix: String::new(),
            show_units: true,
            font_size: 10.0,
            unit: crate::units::Unit::Millimeters,
            leading_zero: true,
            dynamic: true,
            ext_gap: 0.0,
            ext_len: None,
            arrow: dim_arrow(),
            callout_shape: 0,
            callout_gap: callout_gap()
        }
    }
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
    /// Direction the connector leaves the anchor in (degrees: 0 right, 90 up), from the anchor's
    /// Adjust Anchor Direction (right-angle connectors).
    #[serde(default)]
    pub dir: Option<f64>,
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
    /// Color blend direction: 0 direct, 1 clockwise, 2 counterclockwise (around the color wheel).
    #[serde(default)]
    pub color_dir: u8,
    /// Loop: the rotation turns the steps around the center between the two ends.
    #[serde(default)]
    pub looped: bool,
    /// Color acceleration when it is not linked to the object acceleration.
    #[serde(default)]
    pub color_accel: f64,
    #[serde(default = "yes")]
    pub link_accel: bool,
    /// Accelerate the sizes too.
    #[serde(default)]
    pub size_accel: bool,
}

impl Default for BlendObj {
    fn default() -> Self {
        let o = Object::new(0, ObjKind::Group(vec![]), Affine::IDENTITY, crate::style::Style::default());
        BlendObj { start: Box::new(o.clone()), end: Box::new(o), steps: 20, rotation: 0.0, acceleration: 0.0, color_dir: 0, looped: false, color_accel: 0.0, link_accel: true, size_accel: false }
    }
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
    /// Perspective shadow (dragged from the top or bottom of the object): angle, fade and stretch apply.
    #[serde(default)]
    pub perspective: bool,
    /// Perspective shadow angle, degrees.
    #[serde(default)]
    pub angle: f64,
    /// Fade toward the far end, 0..100.
    #[serde(default)]
    pub fade: f64,
    /// Stretch of the perspective shadow, 0..100 (50 = as tall as the object).
    #[serde(default = "fifty")]
    pub stretch: f64,
    #[serde(default)]
    pub merge: crate::style::MergeMode,
}

fn fifty() -> f64 {
    50.0
}

impl Default for DropShadow {
    fn default() -> Self {
        DropShadow { offset: Vec2::new(6.0, 6.0), feather: 8.0, color: Color::BLACK, opacity: 50.0, inner: false, perspective: false, angle: 0.0, fade: 0.0, stretch: 50.0, merge: crate::style::MergeMode::Multiply }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockShadow {
    pub offset: Vec2,
    pub color: Color,
    /// Expand the shadow beyond the object, points.
    #[serde(default)]
    pub expand: f64,
    /// Include the outline in the shadow shape.
    #[serde(default)]
    pub from_outline: bool,
    #[serde(default)]
    pub overprint: bool,
    #[serde(default)]
    pub remove_holes: bool,
    #[serde(default)]
    pub simplify: bool,
}

impl Default for BlockShadow {
    fn default() -> Self {
        BlockShadow { offset: Vec2::new(9.0, 9.0), color: Color::BLACK, expand: 0.0, from_outline: false, overprint: false, remove_holes: false, simplify: false }
    }
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
    /// Object acceleration (-100..100): steps crowd toward the object (positive) or the end.
    #[serde(default)]
    pub accel: f64,
    /// Color acceleration when not linked to the object acceleration.
    #[serde(default)]
    pub color_accel: f64,
    #[serde(default = "yes")]
    pub link_accel: bool,
    /// Last Fill Color: the end color of the last step of a fountain-filled object.
    #[serde(default)]
    pub last_fill: Option<Color>,
    /// The corners of the steps: 0 mitered, 1 round, 2 beveled.
    #[serde(default)]
    pub corner: u8,
    /// Color blend: 0 linear, 1 clockwise, 2 counterclockwise (around the color wheel).
    #[serde(default)]
    pub color_dir: u8,
}

impl Default for ContourEffect {
    fn default() -> Self {
        ContourEffect { dir: ContourDir::Outside, steps: 3, offset: 6.0, end_fill: Color::WHITE, end_outline: Color::BLACK, accel: 0.0, color_accel: 0.0, link_accel: true, last_fill: None, corner: 0, color_dir: 0 }
    }
}

impl ContourEffect {
    /// The join of the steps' corners.
    pub fn join(&self) -> kurbo::Join {
        match self.corner {
            1 => kurbo::Join::Round,
            2 => kurbo::Join::Bevel,
            _ => kurbo::Join::Miter,
        }
    }
}

/// The steps of a contour in drawing order (the farthest from the object first): each one's
/// path in document space, fill and outline color. To center adds steps of `offset` until the
/// object is filled; Inside and Outside spread `steps` steps with the object acceleration.
pub fn contour_rings(o: &Object, c: &ContourEffect) -> Vec<(BezPath, crate::style::Fill, Option<Color>)> {
    use crate::style::Fill;
    let base = o.world_path();
    let join = c.join();
    let to_center = c.dir == ContourDir::ToCenter;
    let n = if to_center { 64 } else { c.steps.clamp(1, 64) };
    let mut paths = vec![];
    for k in 1..=n {
        let lin = k as f64 / n as f64;
        let d = if to_center { c.offset.max(0.01) * k as f64 } else { c.offset * n as f64 * accel(lin, c.accel) };
        let p = geom::offset_path(&base, if c.dir == ContourDir::Outside { d } else { -d }, join);
        if p.elements().is_empty() || p.area().abs() < 1e-3 {
            break;
        }
        paths.push(p);
    }
    let count = if to_center { paths.len().max(1) } else { n as usize };
    let start_fill = o.style.fill.main_color().unwrap_or(Color::WHITE);
    let start_ol = o.style.outline.color.unwrap_or(Color::BLACK);
    let mut v: Vec<_> = paths
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let lin = (i + 1) as f64 / count as f64;
            let t = accel(lin, if c.link_accel { c.accel } else { c.color_accel });
            // fountain fills blend their first color to the fill color and their last one
            // to the last fill color
            let fill = match (&o.style.fill, c.last_fill) {
                (Fill::Gradient(g), Some(lf)) if g.stops.len() >= 2 => {
                    let mut g = g.clone();
                    let m = g.stops.len();
                    g.stops[0].color = blend_color(g.stops[0].color, c.end_fill, t, c.color_dir);
                    g.stops[m - 1].color = blend_color(g.stops[m - 1].color, lf, t, c.color_dir);
                    Fill::Gradient(g)
                }
                _ => Fill::Solid(blend_color(start_fill, c.end_fill, t, c.color_dir)),
            };
            (p, fill, Some(blend_color(start_ol, c.end_outline, t, c.color_dir)))
        })
        .collect();
    if c.dir == ContourDir::Outside {
        v.reverse();
    }
    v
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtrudeEffect {
    /// Extrusion vector in points (towards the vanishing point).
    pub depth: Vec2,
    pub color: Color,
    pub shade: Color,
    pub steps: u32,
    /// Extrusion type: 0 small back, 1 small front, 2 big back, 3 big front, 4 back parallel, 5 front parallel.
    #[serde(default)]
    pub kind: u8,
    /// The vanishing point is locked to the page (true) or to the object.
    #[serde(default)]
    pub vp_page: bool,
    /// 3D rotation of the extrusion (degrees around x, y, z).
    #[serde(default)]
    pub rotation: [f64; 3],
    /// Color: 0 use object fill, 1 solid fill (`color`), 2 color shading (`color` to `shade`).
    #[serde(default = "two")]
    pub color_mode: u8,
    /// Bevel: depth (points) and angle (degrees).
    #[serde(default)]
    pub bevel: Option<(f64, f64)>,
    #[serde(default)]
    pub bevel_only: bool,
    /// Lights 1, 2 and 3: position and intensity 0..100. A position is where the lines of the
    /// 3 × 3 × 3 light cube cross: layer from the front × 9 + row × 3 + column; [`LIGHT_OFF`] is a
    /// light turned off.
    #[serde(default)]
    pub lights: Vec<(u8, f64)>,
    /// Drape fills: the object's fill runs over the extrusion (Use object fill).
    #[serde(default)]
    pub drape: bool,
    /// The bevel takes the extrusion's color.
    #[serde(default)]
    pub bevel_fill: bool,
    /// The bevel's color (None: the object's fill).
    #[serde(default)]
    pub bevel_color: Option<Color>,
}

/// The position of an extrusion light that is turned off.
pub const LIGHT_OFF: u8 = 255;

fn two() -> u8 {
    2
}

impl Default for ExtrudeEffect {
    fn default() -> Self {
        ExtrudeEffect { depth: Vec2::new(18.0, -18.0), color: Color::rgb(128, 128, 128), shade: Color::BLACK, steps: 24, kind: 0, vp_page: false, rotation: [0.0; 3], color_mode: 2, bevel: None, bevel_only: false, lights: vec![], drape: false, bevel_fill: false, bevel_color: None }
    }
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
    /// Edges: 0 straight line, 1 single arc, 2 double arc, 3 unconstrained.
    #[serde(default = "three_u8")]
    pub mode: u8,
    /// Mapping mode: 0 horizontal, 1 original, 2 putty, 3 vertical.
    #[serde(default = "one_u8")]
    pub mapping: u8,
    /// Keep lines: straight segments stay straight.
    #[serde(default)]
    pub keep_lines: bool,
}

fn one_u8() -> u8 {
    1
}

fn three_u8() -> u8 {
    3
}

impl Default for EnvelopeEffect {
    fn default() -> Self {
        EnvelopeEffect { handles: [Vec2::ZERO; 8], mode: 3, mapping: 1, keep_lines: false }
    }
}

/// Wrap Paragraph Text: how paragraph text flows around an object.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextWrap {
    /// 0 contour left, 1 contour right, 2 contour straddle, 3 square left, 4 square right,
    /// 5 square straddle, 6 square above/below.
    pub style: u8,
    /// Text wrap offset, points.
    pub offset: f64,
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

/// Effects ▸ Bevel: a soft edge or an emboss lit from a direction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BevelEffect {
    /// Emboss (else Soft edge).
    pub emboss: bool,
    /// The bevel goes to the center of the object (else `distance` in from the edges).
    pub to_center: bool,
    pub distance: f64,
    /// The shadow color (None: a darker shade of the object).
    pub shadow: Option<Color>,
    pub light: Color,
    /// 0..100.
    pub intensity: f64,
    /// Degrees, counterclockwise from the right.
    pub direction: f64,
    /// Degrees above the surface, 0..90.
    pub altitude: f64,
}

impl Default for BevelEffect {
    fn default() -> Self {
        BevelEffect { emboss: false, to_center: false, distance: 5.7, shadow: None, light: Color::WHITE, intensity: 80.0, direction: 70.0, altitude: 54.0 }
    }
}

/// Bevels used to be (depth, light angle).
fn de_bevel<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<BevelEffect>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        New(BevelEffect),
        Old((f64, f64)),
    }
    Ok(Option::<Any>::deserialize(d)?.map(|a| match a {
        Any::New(b) => b,
        Any::Old((distance, direction)) => BevelEffect { distance, direction, ..Default::default() },
    }))
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
    /// Bevel (Effects ▸ Bevel).
    #[serde(default, deserialize_with = "de_bevel")]
    pub bevel: Option<BevelEffect>,
    /// Lens: magnify factor (1 = none), or color filter.
    #[serde(default)]
    #[serde(deserialize_with = "de_lens")]
    pub lens: Option<LensEffect>,
    /// Wrap Paragraph Text around this object.
    #[serde(default)]
    pub wrap: Option<TextWrap>,
    /// Connector anchors set with the Anchor Editing tool (empty = the geometric anchors).
    #[serde(default)]
    pub anchors: Vec<ObjAnchor>,
}

/// A connector anchor of an object: a normalized point of its bounding box, the direction
/// connectors leave it in (degrees, `None` = automatic) and whether it is a snap point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjAnchor {
    pub pos: Point,
    pub dir: Option<f64>,
    pub auto: bool,
}

/// The geometric anchors every object has: the middle of each side and the center.
pub const DEFAULT_ANCHORS: [(f64, f64); 5] = [(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5), (0.5, 0.5)];

impl Effects {
    /// The anchors connectors attach to: the edited ones, else the geometric ones.
    pub fn anchor_list(&self) -> Vec<ObjAnchor> {
        if self.anchors.is_empty() {
            DEFAULT_ANCHORS.iter().map(|(x, y)| ObjAnchor { pos: Point::new(*x, *y), dir: None, auto: true }).collect()
        } else {
            self.anchors.clone()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OldLens {
    Magnify(f64),
    Brighten(f64),
    Invert,
    ColorAdd(Color),
    Grayscale,
}

/// The lens types of Effects ▸ Lens, in the order of its list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LensKind {
    Brighten,
    ColorAdd,
    ColorLimit,
    CustomColorMap,
    FishEye,
    HeatMap,
    Invert,
    Magnify,
    TintedGrayscale,
    Transparency,
    Wireframe,
    BitmapEffect,
}

impl LensKind {
    pub const ALL: [LensKind; 12] = [
        LensKind::Brighten,
        LensKind::ColorAdd,
        LensKind::ColorLimit,
        LensKind::CustomColorMap,
        LensKind::FishEye,
        LensKind::HeatMap,
        LensKind::Invert,
        LensKind::Magnify,
        LensKind::TintedGrayscale,
        LensKind::Transparency,
        LensKind::Wireframe,
        LensKind::BitmapEffect,
    ];
    pub fn name(self) -> &'static str {
        match self {
            LensKind::Brighten => "Brighten",
            LensKind::ColorAdd => "Color Add",
            LensKind::ColorLimit => "Color Limit",
            LensKind::CustomColorMap => "Custom Color Map",
            LensKind::FishEye => "Fish Eye",
            LensKind::HeatMap => "Heat Map",
            LensKind::Invert => "Invert",
            LensKind::Magnify => "Magnify",
            LensKind::TintedGrayscale => "Tinted Grayscale",
            LensKind::Transparency => "Transparency",
            LensKind::Wireframe => "Wireframe",
            LensKind::BitmapEffect => "Bitmap Effect",
        }
    }
}

/// Effects ▸ Lens: how the objects under the lens object look through it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LensEffect {
    pub kind: LensKind,
    /// Rate (%): Brighten -100..100; Color Add, Color Limit, Transparency 0..100; Fish Eye -1000..1000.
    pub rate: f64,
    /// Magnify: amount (×).
    pub amount: f64,
    /// The color of Color Add, Color Limit, Tinted Grayscale and Transparency; the From color of a
    /// Custom Color Map.
    pub color: Color,
    /// The To color of a Custom Color Map.
    pub color2: Color,
    /// Custom Color Map: 0 direct palette, 1 forward rainbow, 2 reverse rainbow.
    pub palette: u8,
    /// Heat Map: palette rotation (%).
    pub rotation: f64,
    /// Wireframe: the outline and fill the objects take (None: their own).
    pub outline: Option<Color>,
    pub fill: Option<Color>,
    /// Frozen: the lens keeps what it showed when frozen.
    pub frozen: bool,
    /// Remove Face: the lens shows only over objects, not over the empty page.
    pub remove_face: bool,
    /// Viewpoint: the lens shows what is at this point instead of what is under it.
    pub viewpoint: Option<Point>,
}

impl LensEffect {
    /// A lens of `kind` with its settings at their defaults.
    pub fn new(kind: LensKind) -> Self {
        let rate = match kind {
            LensKind::FishEye => 100.0,
            _ => 50.0,
        };
        LensEffect {
            kind,
            rate,
            amount: 2.0,
            color: Color::rgb(0x46, 0x82, 0xdc),
            color2: Color::WHITE,
            palette: 0,
            rotation: 0.0,
            outline: Some(Color::rgb(0x46, 0x82, 0xdc)),
            fill: Some(Color::WHITE),
            frozen: false,
            remove_face: false,
            viewpoint: None,
        }
    }
}

/// Lenses used to be Magnify(amount), Brighten(rate), Invert, ColorAdd(color) or Grayscale.
fn de_lens<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<LensEffect>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        New(LensEffect),
        Old(OldLens),
    }
    Ok(Option::<Any>::deserialize(d)?.map(|a| match a {
        Any::New(l) => l,
        Any::Old(OldLens::Magnify(f)) => LensEffect { amount: f, ..LensEffect::new(LensKind::Magnify) },
        Any::Old(OldLens::Brighten(b)) => LensEffect { rate: b, ..LensEffect::new(LensKind::Brighten) },
        Any::Old(OldLens::Invert) => LensEffect::new(LensKind::Invert),
        Any::Old(OldLens::ColorAdd(c)) => LensEffect { color: c, ..LensEffect::new(LensKind::ColorAdd) },
        Any::Old(OldLens::Grayscale) => LensEffect { color: Color::WHITE, ..LensEffect::new(LensKind::TintedGrayscale) },
    }))
}

/// The limited palette of the Heat Map lens, from the darkest colors (cool) to the lightest (warm).
pub fn heat_color(t: f64) -> Color {
    const P: [(u8, u8, u8); 7] = [(0, 255, 255), (128, 0, 255), (0, 0, 255), (255, 0, 0), (255, 128, 0), (255, 255, 0), (255, 255, 255)];
    let i = ((t.rem_euclid(1.0) * 7.0) as usize).min(6);
    Color::rgb(P[i].0, P[i].1, P[i].2)
}

/// A color seen through a lens that changes colors (0..255 RGB); other lenses leave it alone.
pub fn lens_color(l: &LensEffect, rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb;
    let rate = (l.rate / 100.0) as f32;
    let c = [l.color.r as f32, l.color.g as f32, l.color.b as f32];
    let lum = 0.299 * r + 0.587 * g + 0.114 * b;
    let out = match l.kind {
        LensKind::Brighten if rate >= 0.0 => rgb.map(|v| v + (255.0 - v) * rate),
        LensKind::Brighten => rgb.map(|v| v * (1.0 + rate)),
        LensKind::ColorAdd => [r + c[0] * rate, g + c[1] * rate, b + c[2] * rate],
        LensKind::ColorLimit => {
            let k = |v: f32, cv: f32| v * (1.0 - rate + rate * cv / 255.0);
            [k(r, c[0]), k(g, c[1]), k(b, c[2])]
        }
        LensKind::CustomColorMap => {
            let m = blend_color(l.color, l.color2, (lum / 255.0) as f64, l.palette);
            [m.r as f32, m.g as f32, m.b as f32]
        }
        LensKind::HeatMap => {
            let m = heat_color(lum as f64 / 255.0 * 0.999 + l.rotation / 100.0);
            [m.r as f32, m.g as f32, m.b as f32]
        }
        LensKind::Invert => rgb.map(|v| 255.0 - v),
        LensKind::TintedGrayscale => c.map(|cv| cv * lum / 255.0),
        LensKind::Transparency => [c[0] + (r - c[0]) * rate, c[1] + (g - c[1]) * rate, c[2] + (b - c[2]) * rate],
        _ => rgb,
    };
    out.map(|v| v.clamp(0.0, 255.0))
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
    /// PowerClip ▸ Lock Contents to PowerClip turned off: moving the frame leaves the contents.
    #[serde(default)]
    pub powerclip_unlocked: bool,
    /// Object ▸ Object Hinting: edges drawn on whole pixels.
    #[serde(default)]
    pub hinting: bool,
    /// QR code, barcode or rollover data.
    #[serde(default)]
    pub special: Option<crate::codes::Special>,
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
            powerclip_unlocked: false,
            hinting: false,
            special: None,
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
        let unlocked = self.powerclip_unlocked;
        match &mut self.kind {
            ObjKind::Group(c) => {
                for o in c.iter_mut() {
                    o.apply_transform(a);
                }
            }
            ObjKind::PowerClip { frame, contents } => {
                frame.apply_transform(a);
                if !unlocked {
                    for o in contents.iter_mut() {
                        o.apply_transform(a);
                    }
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
        if let Some(crate::style::WidthProfile::Variable { nodes, scale: true }) = &mut self.style.outline.profile {
            let (_, _, _, sx, sy) = geom::decompose(a);
            let s = (sx.abs() * sy.abs()).sqrt();
            if s.is_finite() && s > 0.0 {
                for n in nodes.iter_mut() {
                    n.side1 *= s;
                    n.side2 *= s;
                }
            }
        }
        // symmetry and perspective groups keep their mirror lines and fields with them
        match &mut self.special {
            Some(crate::codes::Special::Symmetry { center, angle, .. }) => {
                let c = a * Point::new(center[0], center[1]);
                let lin = Affine::new([a.as_coeffs()[0], a.as_coeffs()[1], a.as_coeffs()[2], a.as_coeffs()[3], 0.0, 0.0]);
                let r = angle.to_radians();
                let d = lin * Point::new(r.cos(), r.sin());
                if d.to_vec2().hypot() > 1e-12 {
                    *angle = d.y.atan2(d.x).to_degrees();
                }
                *center = [c.x, c.y];
            }
            Some(crate::codes::Special::Perspective { field, horizon, flats, .. }) => {
                let f = a.transform_rect_bbox(kurbo::Rect::new(field[0], field[1], field[2], field[3]));
                *horizon = (a * Point::new(field[0], *horizon)).y;
                *field = [f.x0, f.y0, f.x1, f.y1];
                for (_, o) in flats.iter_mut() {
                    o.apply_transform(a);
                }
            }
            _ => {}
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
        let mut n = format!("{:.*}", d.precision, v);
        if !d.leading_zero && (n.starts_with("0.") || n.starts_with("-0.")) {
            n = n.replacen("0.", ".", 1);
        }
        if d.show_units {
            format!("{} {}", n, d.unit.suffix())
        } else {
            n
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
    use crate::style::ArrowHead;
    let mut bp = BezPath::new();
    let pts = &d.points;
    let kind = d.arrow;
    let arrow = |bp: &mut BezPath, tip: Point, dir: Vec2, size: f64| {
        let dir = dir.normalize();
        let n = Vec2::new(-dir.y, dir.x);
        match kind {
            None => {}
            Some(ArrowHead::OpenTriangle) | Some(ArrowHead::Custom(_)) => {
                bp.move_to(tip);
                bp.line_to(tip - dir * size + n * size * 0.35);
                bp.move_to(tip);
                bp.line_to(tip - dir * size - n * size * 0.35);
            }
            Some(ArrowHead::Triangle) => {
                bp.move_to(tip);
                bp.line_to(tip - dir * size + n * size * 0.35);
                bp.line_to(tip - dir * size - n * size * 0.35);
                bp.close_path();
            }
            Some(ArrowHead::Circle) => {
                let c = kurbo::Circle::new(tip, size * 0.3).to_path(0.05);
                bp.extend(c.elements().iter().copied());
            }
            Some(ArrowHead::Square) => {
                let h = size * 0.3;
                bp.move_to(tip + n * h);
                bp.line_to(tip + n * h - dir * h * 2.0);
                bp.line_to(tip - n * h - dir * h * 2.0);
                bp.line_to(tip - n * h);
                bp.close_path();
            }
            Some(ArrowHead::Diamond) => {
                let h = size * 0.35;
                bp.move_to(tip);
                bp.line_to(tip - dir * h + n * h);
                bp.line_to(tip - dir * h * 2.0);
                bp.line_to(tip - dir * h - n * h);
                bp.close_path();
            }
            Some(ArrowHead::Bar) => {
                bp.move_to(tip + n * size * 0.4);
                bp.line_to(tip - n * size * 0.4);
            }
        }
    };
    let gap = d.ext_gap.max(0.0);
    let past = d.ext_len.unwrap_or(2.0);
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
            let ext = normal * (past * off.signum());
            let g = normal * (gap * off.signum());
            bp.move_to(a + g);
            bp.line_to(a2 + ext);
            bp.move_to(b + g);
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
                let sa = (y - a.y).signum();
                let sb = (y - b.y).signum();
                bp.move_to((a.x, a.y + gap * sa));
                bp.line_to((a.x, y + past * sa));
                bp.move_to((b.x, b.y + gap * sb));
                bp.line_to((b.x, y + past * sb));
                bp.move_to((a.x, y));
                bp.line_to((b.x, y));
                arrow(&mut bp, Point::new(a.x, y), Vec2::new(a.x - b.x, 0.0), sz);
                arrow(&mut bp, Point::new(b.x, y), Vec2::new(b.x - a.x, 0.0), sz);
            } else {
                let x = mid.x;
                let sa = (x - a.x).signum();
                let sb = (x - b.x).signum();
                bp.move_to((a.x + gap * sa, a.y));
                bp.line_to((x + past * sa, a.y));
                bp.move_to((b.x + gap * sb, b.y));
                bp.line_to((x + past * sb, b.y));
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
            // the legs leave the anchors in their directions (horizontal unless set vertical)
            let vertical = |d: Option<f64>| d.is_some_and(|d| (d.rem_euclid(180.0) - 90.0).abs() < 45.0);
            let (sv, ev) = (vertical(c.start.dir), vertical(c.end.dir));
            let pts: Vec<Point> = match (sv, ev) {
                (false, false) => {
                    let midx = (a.x + b.x) / 2.0;
                    vec![a, Point::new(midx, a.y), Point::new(midx, b.y), b]
                }
                (true, true) => {
                    let midy = (a.y + b.y) / 2.0;
                    vec![a, Point::new(a.x, midy), Point::new(b.x, midy), b]
                }
                (false, true) => vec![a, Point::new(b.x, a.y), b],
                (true, false) => vec![a, Point::new(a.x, b.y), b],
            };
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
    map_points_ex(path, f, false)
}

/// Map every point of a path; `keep_lines` leaves straight segments straight (Keep Lines).
fn map_points_ex(path: &BezPath, f: impl Fn(Point) -> Point, keep_lines: bool) -> BezPath {
    // subdivide curves first so warps look smooth
    let mut fine = BezPath::new();
    for el in path.elements() {
        match *el {
            kurbo::PathEl::MoveTo(p) => fine.move_to(p),
            kurbo::PathEl::LineTo(p) if keep_lines => fine.line_to(p),
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
    let mode = env.mode;
    // the edges of the envelope for its mode
    let edge = |p0: Point, pm: Point, p1: Point, t: f64| -> Point {
        match mode {
            // straight line: the midpoint handle is not used
            0 => p0.lerp(p1, t),
            // double arc: an S through the middle of the edge, as deep as the midpoint handle
            2 => {
                let d = pm - p0.midpoint(p1);
                let c1 = p0.lerp(p1, 1.0 / 3.0) + d * 2.0;
                let c2 = p0.lerp(p1, 2.0 / 3.0) - d * 2.0;
                kurbo::CubicBez::new(p0, c1, c2, p1).eval(t)
            }
            _ => quad(p0, pm, p1, t),
        }
    };
    let mapping = env.mapping;
    map_points_ex(
        path,
        |p| {
            let u = (p.x - bb.x0) / w;
            let v = (p.y - bb.y0) / h;
            let c00 = hp[0];
            let c10 = hp[2];
            let c01 = hp[6];
            let c11 = hp[4];
            let bilinear = |u: f64, v: f64| -> Point {
                Point::new(
                    (1.0 - u) * (1.0 - v) * c00.x + u * (1.0 - v) * c10.x + (1.0 - u) * v * c01.x + u * v * c11.x,
                    (1.0 - u) * (1.0 - v) * c00.y + u * (1.0 - v) * c10.y + (1.0 - u) * v * c01.y + u * v * c11.y,
                )
            };
            let top = edge(hp[0], hp[1], hp[2], u);
            let bottom = edge(hp[6], hp[5], hp[4], u);
            let left = edge(hp[0], hp[7], hp[6], v);
            let right = edge(hp[2], hp[3], hp[4], v);
            // Coons patch (Original)
            let b = bilinear(u, v);
            let coons = Point::new(
                (1.0 - v) * top.x + v * bottom.x + (1.0 - u) * left.x + u * right.x - b.x,
                (1.0 - v) * top.y + v * bottom.y + (1.0 - u) * left.y + u * right.y - b.y,
            );
            let q = match mapping {
                // Horizontal: each row is fitted between the left and right edges
                0 => Point::new(left.x + (right.x - left.x) * u, coons.y),
                // Putty: only the corner handles
                2 => b,
                // Vertical: each column is fitted between the top and bottom edges
                3 => Point::new(coons.x, top.y + (bottom.y - top.y) * v),
                _ => coons,
            };
            Point::new(bb.x0 + q.x * w, bb.y0 + q.y * h)
        },
        env.keep_lines,
    )
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

/// Acceleration curve (-100..100) of blends and contours.
pub fn accel(t: f64, a: f64) -> f64 {
    if a.abs() < 1e-6 {
        return t;
    }
    let k = 1.0 + a.abs() / 25.0;
    if a > 0.0 {
        t.powf(k)
    } else {
        1.0 - (1.0 - t).powf(k)
    }
}

/// Color of a blend or contour step: direct, or clockwise / counterclockwise around the color wheel.
pub fn blend_color(a: Color, b: Color, t: f64, dir: u8) -> Color {
    if dir == 0 {
        return Color::lerp(a, b, t);
    }
    let [h1, s1, v1] = a.to_hsb();
    let [h2, s2, v2] = b.to_hsb();
    let mut dh = h2 - h1;
    if dir == 1 {
        if dh <= 0.0 {
            dh += 360.0;
        }
    } else if dh >= 0.0 {
        dh -= 360.0;
    }
    let h = (h1 + dh * t).rem_euclid(360.0);
    let mut c = Color::from_hsb(h, s1 + (s2 - s1) * t, v1 + (v2 - v1) * t);
    c.a = (a.a as f64 + (b.a as f64 - a.a as f64) * t).round() as u8;
    c
}

/// The intermediate objects of a blend, with its accelerations, color direction and loop.
pub fn blend_steps(b: &BlendObj, mut new_id: impl FnMut() -> ObjId) -> Vec<Object> {
    let ca = b.start.bounds().center();
    let cb = b.end.bounds().center();
    let pivot = ca.midpoint(cb);
    (1..=b.steps)
        .map(|k| {
            let t = k as f64 / (b.steps + 1) as f64;
            let to = accel(t, b.acceleration);
            let tc = if b.link_accel { to } else { accel(t, b.color_accel) };
            let ts = if b.size_accel { to } else { t };
            let mut o = blend_step(&b.start, &b.end, ts, new_id(), if b.looped { 0.0 } else { b.rotation });
            // position follows the object acceleration
            let want = ca.lerp(cb, to);
            let have = o.bounds().center();
            o.apply_transform(Affine::translate(want - have));
            if b.looped && b.rotation != 0.0 {
                o.apply_transform(Affine::rotate_about((b.rotation * to).to_radians(), pivot));
            }
            // colors follow the color acceleration and direction
            use crate::style::Fill;
            if let (Fill::Solid(x), Fill::Solid(y)) = (&b.start.style.fill, &b.end.style.fill) {
                o.style.fill = Fill::Solid(blend_color(*x, *y, tc, b.color_dir));
            }
            if let (Some(x), Some(y)) = (b.start.style.outline.color, b.end.style.outline.color) {
                o.style.outline.color = Some(blend_color(x, y, tc, b.color_dir));
            }
            o
        })
        .collect()
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

#[cfg(test)]
mod table_tests {
    use super::*;

    #[test]
    fn merge_split_and_spans() {
        let mut t = TableObj::new(3, 4, 400.0, 300.0);
        t.cell_mut(0, 0).text = "a".into();
        t.cell_mut(1, 1).text = "b".into();
        t.merge([0, 0, 1, 1]);
        assert_eq!((t.cell(0, 0).rspan, t.cell(0, 0).cspan), (2, 2));
        assert_eq!(t.cell(0, 0).text, "a b");
        assert!(t.is_covered(1, 1));
        assert_eq!(t.anchors().len(), 12 - 3);
        assert_eq!(t.cell_at(Point::new(150.0, 150.0)), Some((0, 0)));
        // a block touching the merged cell grows to hold all of it
        assert_eq!(t.range((1, 1), (1, 2)), [0, 0, 1, 2]);
        // inserting a row through the merged cell makes it taller
        t.insert_row(1);
        assert_eq!(t.cell(0, 0).rspan, 3);
        t.delete_row(0);
        assert_eq!((t.cell(0, 0).rspan, t.cell(0, 0).cspan, t.cell(0, 0).text.as_str()), (2, 2, "a b"));
        t.insert_col(1);
        assert_eq!(t.cell(0, 0).cspan, 3);
        t.unmerge([0, 0, 0, 0]);
        assert_eq!(t.anchors().len(), t.rows * t.cols);
        // splitting one cell into rows adds rows; the neighbours stay whole
        let mut t = TableObj::new(2, 2, 200.0, 100.0);
        t.split_rows(0, 1, 3);
        assert_eq!(t.rows, 4);
        assert_eq!(t.cell(0, 0).rspan, 3);
        assert_eq!((t.cell(0, 1).rspan, t.cell(1, 1).rspan, t.cell(2, 1).rspan), (1, 1, 1));
        assert!((t.height() - 100.0).abs() < 1e-9);
        // splitting a merged cell uses its rows
        let mut t = TableObj::new(4, 2, 200.0, 100.0);
        t.merge([0, 0, 3, 0]);
        t.split_rows(0, 0, 2);
        assert_eq!((t.rows, t.cell(0, 0).rspan, t.cell(2, 0).rspan), (4, 2, 2));
        t.split_cols(0, 1, 2);
        assert_eq!(t.cols, 3);
        assert_eq!(t.cell(0, 0).cspan, 1);
        assert_eq!(t.cell(1, 1).cspan, 2);
    }
}
