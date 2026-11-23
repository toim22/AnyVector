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
    /// Nodes moved with the Shape tool (#104); the shape stays a polygon.
    #[serde(default, skip_serializing_if = "PolygonNodes::is_regular")]
    pub nodes: PolygonNodes,
}

/// A polygon or star reshaped with the Shape tool, as in CorelDRAW: dragging one node moves all
/// its matching nodes the same way. The outer nodes (corners, star tips) and the inner nodes (the
/// middles of the sides, star valleys) each get a radius scale and a turn (degrees, clockwise)
/// over the regular shape.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolygonNodes {
    pub outer_scale: f64,
    pub outer_turn: f64,
    pub inner_scale: f64,
    pub inner_turn: f64,
}

impl Default for PolygonNodes {
    fn default() -> Self {
        PolygonNodes { outer_scale: 1.0, outer_turn: 0.0, inner_scale: 1.0, inner_turn: 0.0 }
    }
}

impl PolygonNodes {
    pub fn is_regular(&self) -> bool {
        *self == PolygonNodes::default()
    }
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
    /// Text position: above, through or below the dimension line (CorelDRAW's `Placement`).
    #[serde(default)]
    pub placement: TextPlacement,
    /// Center Text Between Extension Lines (`TextCentered`); off, the text sits at the label
    /// point along the line.
    #[serde(default = "yes")]
    pub text_centered: bool,
    /// Force Text Horizontal (`HorizontalText`).
    #[serde(default)]
    pub horizontal_text: bool,
    /// Draw Box Around Text (`BoxedText`): a box stroked with the dimension's outline.
    #[serde(default)]
    pub boxed_text: bool,
}

/// Where the value of a dimension sits against its dimension line, CorelDRAW's
/// `cdrDimensionPlacement` (`cdrDimensionAboveLine`, `cdrDimensionWithinLine`,
/// `cdrDimensionBelowLine`). Above and below follow the text's own up: with the text forced
/// horizontal they are up and down on the page, on an angular dimension out from and in towards
/// the vertex.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextPlacement {
    /// The value stands on the line: the only placement before 0.6.2, so files without the
    /// field keep their look.
    #[default]
    Above,
    /// The value is centred on the line, which is broken around it.
    Through,
    Below,
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
            callout_gap: callout_gap(),
            placement: TextPlacement::Above,
            text_centered: true,
            horizontal_text: false,
            boxed_text: false,
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
    /// The path the steps follow (Path Properties ▸ New Path), when the blend is on a path.
    #[serde(default)]
    pub path: Option<BlendPath>,
    /// Fixed spacing: the distance between the steps along the path (a blend on a path only);
    /// `None` spreads `steps` steps.
    #[serde(default)]
    pub spacing: Option<f64>,
    /// Split points (Options ▸ Split): intermediate objects the blend passes through, in order
    /// from the start. Each is the start of the next part of the blend.
    #[serde(default)]
    pub splits: Vec<BlendSplit>,
    /// Map Nodes: (subpath, start fraction, end fraction): that closed subpath of the start and
    /// of the end object is matched from those points (fractions of its length) on.
    #[serde(default)]
    pub map: Option<(usize, f64, f64)>,
}

/// The path a blend follows. It is embedded like the start and end objects, so the blend stays
/// self-contained (save, undo, copy and paste); it is drawn with its own style, under the steps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendPath {
    /// The path object, in document coordinates.
    pub obj: Box<Object>,
    /// Blend Along Full Path: the start sits at the beginning of the path and the end at its end.
    #[serde(default)]
    pub full: bool,
    /// Rotate All Objects: every object turns with the direction of the path.
    #[serde(default)]
    pub rotate: bool,
}

/// A split point of a blend: the object, and where it sits in the blend (0..1, the step it was).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendSplit {
    pub obj: Box<Object>,
    pub at: f64,
}

/// A part of a blend that can be picked on its own (Show Start, Show End, Show Path, a split).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendPart {
    Start,
    End,
    Path,
    Split(usize),
}

impl BlendObj {
    /// The object of a part, if the blend has it.
    pub fn part(&self, p: BlendPart) -> Option<&Object> {
        match p {
            BlendPart::Start => Some(&self.start),
            BlendPart::End => Some(&self.end),
            BlendPart::Path => self.path.as_ref().map(|p| p.obj.as_ref()),
            BlendPart::Split(i) => self.splits.get(i).map(|s| s.obj.as_ref()),
        }
    }

    /// The object of a part, to change it.
    pub fn part_mut(&mut self, p: BlendPart) -> Option<&mut Object> {
        match p {
            BlendPart::Start => Some(&mut self.start),
            BlendPart::End => Some(&mut self.end),
            BlendPart::Path => self.path.as_mut().map(|p| p.obj.as_mut()),
            BlendPart::Split(i) => self.splits.get_mut(i).map(|s| s.obj.as_mut()),
        }
    }

    /// The part whose object has the id `id`.
    pub fn part_with(&self, id: ObjId) -> Option<BlendPart> {
        if self.start.id == id {
            return Some(BlendPart::Start);
        }
        if self.end.id == id {
            return Some(BlendPart::End);
        }
        if self.path.as_ref().is_some_and(|p| p.obj.id == id) {
            return Some(BlendPart::Path);
        }
        self.splits.iter().position(|s| s.obj.id == id).map(BlendPart::Split)
    }
}

impl Default for BlendObj {
    fn default() -> Self {
        let o = Object::new(0, ObjKind::Group(vec![]), Affine::IDENTITY, crate::style::Style::default());
        BlendObj {
            start: Box::new(o.clone()),
            end: Box::new(o),
            steps: 20,
            rotation: 0.0,
            acceleration: 0.0,
            color_dir: 0,
            looped: false,
            color_accel: 0.0,
            link_accel: true,
            size_accel: false,
            path: None,
            spacing: None,
            splits: vec![],
            map: None,
        }
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

/// One of the shadows of an object: the one clicked with the Pick tool (#106), or one Break Apart
/// separates (#105).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShadowPart {
    Drop,
    Inner,
    Block,
}

impl DropShadow {
    /// The ground a perspective shadow lies on, for an object with bounds `bb`: flattened by the
    /// stretch and slanted by the angle, from the base of the object.
    pub fn ground(&self, bb: Rect) -> Affine {
        let base = Point::new(bb.center().x, bb.y1);
        let k = (self.stretch / 100.0).clamp(0.0, 2.0);
        let skew = self.angle.clamp(-89.0, 89.0).to_radians().tan();
        Affine::translate(base.to_vec2()) * Affine::new([1.0, 0.0, -skew * k, k, 0.0, 0.0]) * Affine::translate(-base.to_vec2())
    }

    /// Where the shadow of an object with bounds `bb` lies, from where the object is: laid on
    /// the ground (a perspective shadow), then moved by the offset.
    pub fn placement(&self, bb: Rect) -> Affine {
        let t = Affine::translate(self.offset);
        if self.perspective {
            t * self.ground(bb)
        } else {
            t
        }
    }
}

impl BlockShadow {
    /// The outline the block shadow is cast from, in document space: the object's geometry,
    /// without its holes (Remove Holes), grown by Expand and by half the outline (Generate from
    /// Object Outline), and simplified (Simplify).
    pub fn base_path(&self, o: &Object) -> BezPath {
        let mut path = o.world_path();
        if path.elements().is_empty() {
            return path;
        }
        if self.remove_holes {
            path = geom::outer_contours(&path);
        }
        let mut grow = self.expand;
        if self.from_outline && o.style.outline.is_visible() {
            grow += o.style.outline.width / 2.0;
        }
        if grow.abs() > 1e-6 {
            path = geom::offset_path(&path, grow, kurbo::Join::Miter);
        }
        if self.simplify {
            path = geom::simplify_path(&path, 0.5);
        }
        path
    }

    /// Is `p` on the block shadow of `o` (or within `tol` of its ends)? The shadow is the base
    /// swept along the offset, so `p` is on it when the segment from `p - offset` to `p` touches
    /// the base.
    pub fn hit(&self, o: &Object, p: Point, tol: f64) -> bool {
        let base = self.base_path(o);
        if base.elements().is_empty() {
            return false;
        }
        if geom::segment_touches_fill(&base, p - self.offset, p) {
            return true;
        }
        tol > 0.0 && (geom::distance_to_outline(&base, p) <= tol || geom::distance_to_outline(&base, p - self.offset) <= tol)
    }

    /// The block shadow of `o` as one path, as it is drawn: the base, the base moved by the
    /// offset, and the band each of its edges sweeps between them, welded.
    pub fn swept_path(&self, o: &Object) -> BezPath {
        let base = self.base_path(o);
        if base.elements().is_empty() {
            return base;
        }
        // drawn with the non-zero rule, whatever the object's own fill rule
        let region = geom::simplify_self(&base);
        let off = self.offset;
        if off.hypot() < 1e-9 {
            return region;
        }
        let mut sides = BezPath::new();
        for (pts, _) in geom::flatten_polylines(&base, 0.1) {
            let n = pts.len();
            if n < 2 {
                continue;
            }
            for k in 0..n {
                let (a, b) = (pts[k], pts[(k + 1) % n]);
                let mut q = [a, b, b + off, a + off];
                let area: f64 = (0..4).map(|i| q[i].x * q[(i + 1) % 4].y - q[(i + 1) % 4].x * q[i].y).sum();
                if area.abs() < 1e-9 {
                    continue;
                }
                // all turning the same way, so the non-zero rule welds them
                if area < 0.0 {
                    q.reverse();
                }
                sides.move_to(q[0]);
                for v in &q[1..] {
                    sides.line_to(*v);
                }
                sides.close_path();
            }
        }
        let ends = geom::boolean(&region, &(Affine::translate(off) * region.clone()), geom::BoolOp::Union, false);
        if sides.elements().is_empty() {
            return ends;
        }
        geom::boolean(&ends, &geom::simplify_self(&sides), geom::BoolOp::Union, false)
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
    /// Size of the far face against the object (None: from the kind). Imported CorelDRAW
    /// extrusions scale it toward their vanishing point.
    #[serde(default)]
    pub scale: Option<f64>,
}

/// The position of an extrusion light that is turned off.
pub const LIGHT_OFF: u8 = 255;

fn two() -> u8 {
    2
}

impl Default for ExtrudeEffect {
    fn default() -> Self {
        ExtrudeEffect { depth: Vec2::new(18.0, -18.0), color: Color::rgb(128, 128, 128), shade: Color::BLACK, steps: 24, kind: 0, vp_page: false, rotation: [0.0; 3], color_mode: 2, bevel: None, bevel_only: false, lights: vec![], drape: false, bevel_fill: false, bevel_color: None, scale: None }
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

/// A value of a bitmap effect setting: a number (also check boxes, 0/1, and choices, their index),
/// a color, a list of numbers (curves, meshes, kernels) or a text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FxValue {
    Num(f64),
    List(Vec<f64>),
    Color(Color),
    Text(String),
}

/// One effect of Effects ▸ <category> ▸ <effect> (the Bitmap Effects list of the Properties
/// docker): the effect's id (the reference's effect name, "GaussianBlur"), whether it is on, and
/// its settings by name ("Radius"). Settings that are missing take the effect's default.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BitmapEffect {
    pub id: String,
    pub enabled: bool,
    pub params: std::collections::BTreeMap<String, FxValue>,
}

impl BitmapEffect {
    pub fn new(id: &str) -> Self {
        BitmapEffect { id: id.to_string(), enabled: true, params: Default::default() }
    }
    pub fn with(mut self, key: &str, v: f64) -> Self {
        self.params.insert(key.to_string(), FxValue::Num(v));
        self
    }
    /// A number setting, or `def`.
    pub fn num(&self, key: &str, def: f64) -> f64 {
        match self.params.get(key) {
            Some(FxValue::Num(v)) => *v,
            _ => def,
        }
    }
    pub fn flag(&self, key: &str, def: bool) -> bool {
        self.num(key, if def { 1.0 } else { 0.0 }) != 0.0
    }
    pub fn color(&self, key: &str, def: Color) -> Color {
        match self.params.get(key) {
            Some(FxValue::Color(c)) => *c,
            _ => def,
        }
    }
    pub fn list(&self, key: &str) -> Option<&[f64]> {
        match self.params.get(key) {
            Some(FxValue::List(v)) => Some(v),
            _ => None,
        }
    }
    pub fn set(&mut self, key: &str, v: FxValue) {
        self.params.insert(key.to_string(), v);
    }
}

impl<'de> Deserialize<'de> for BitmapEffect {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        /// The effects AnyVector had before the reference's effects (kept so old drawings open).
        #[derive(Deserialize)]
        enum Old {
            GaussianBlur { radius: f64 },
            Glow { radius: f64, color: Color },
            Emboss { depth: f64 },
            Noise { amount: f64 },
            Pixelate { size: f64 },
            Grayscale,
            Sepia,
            Invert,
        }
        #[derive(Deserialize)]
        struct New {
            id: String,
            #[serde(default = "yes")]
            enabled: bool,
            #[serde(default)]
            params: std::collections::BTreeMap<String, FxValue>,
        }
        fn yes() -> bool {
            true
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Any {
            New(New),
            Old(Old),
        }
        Ok(match Any::deserialize(d)? {
            Any::New(n) => BitmapEffect { id: n.id, enabled: n.enabled, params: n.params },
            Any::Old(o) => match o {
                Old::GaussianBlur { radius } => BitmapEffect::new("GaussianBlur").with("Radius", radius),
                Old::Glow { radius, color } => {
                    let mut e = BitmapEffect::new("Glow").with("Radius", radius);
                    e.set("Color", FxValue::Color(color));
                    e
                }
                Old::Emboss { depth } => BitmapEffect::new("Emboss").with("Depth", depth),
                Old::Noise { amount } => BitmapEffect::new("AddNoise").with("Level", amount),
                Old::Pixelate { size } => BitmapEffect::new("Pixelate").with("Width", size).with("Height", size),
                Old::Grayscale => BitmapEffect::new("Desaturate"),
                Old::Sepia => BitmapEffect::new("Sepia"),
                Old::Invert => BitmapEffect::new("Invert"),
            },
        })
    }
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
    /// Links and Rollovers: the behavior, target frame, comments and hotspot of the link.
    #[serde(default)]
    pub link: LinkInfo,
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

/// Links and Rollovers: a bookmark (else the object is a Web link), the browser window the link
/// opens in, the alternate comments, and the hotspot: the object's shape instead of its bounds.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LinkInfo {
    #[serde(default)]
    pub bookmark: Option<String>,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub alt: String,
    #[serde(default)]
    pub hotspot_shape: bool,
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
            link: LinkInfo::default(),
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
            ObjKind::Blend(b) => {
                let mut v = vec![b.start.as_ref(), b.end.as_ref()];
                v.extend(b.path.as_ref().map(|p| p.obj.as_ref()));
                v.extend(b.splits.iter().map(|s| s.obj.as_ref()));
                v
            }
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
            ObjKind::Blend(b) => {
                let mut v = vec![b.start.as_mut(), b.end.as_mut()];
                v.extend(b.path.as_mut().map(|p| p.obj.as_mut()));
                v.extend(b.splits.iter_mut().map(|s| s.obj.as_mut()));
                v
            }
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
            ObjKind::Blend(b) => {
                let (s, e) = (b.start.bounds_with(sym), b.end.bounds_with(sym));
                let mut r = s.union(e);
                for x in &b.splits {
                    r = r.union(x.obj.bounds_with(sym));
                }
                if let Some(p) = &b.path {
                    // the steps sit on the path: it, and half a step on each side of it
                    let (w, h) = (s.width().max(e.width()), s.height().max(e.height()));
                    let (dx, dy) = if p.rotate { (w.hypot(h) / 2.0, w.hypot(h) / 2.0) } else { (w / 2.0, h / 2.0) };
                    r = r.union(p.obj.bounds_with(sym).inflate(dx, dy));
                }
                r
            }
            ObjKind::Symbol(id) => sym(*id).map(|r| self.transform.transform_rect_bbox(r)).unwrap_or(Rect::ZERO),
            ObjKind::Text(t) => {
                if self.effects.warps_geometry() {
                    self.world_path().bounding_box()
                } else {
                    let l = text::layout(t);
                    self.transform.transform_rect_bbox(l.bounds)
                }
            }
            // the lines and the value, wherever its placement puts it
            ObjKind::Dimension(d) => {
                let p = self.world_path();
                let t = dimension_text(d).ink_outline().map(|t| (self.transform * t).bounding_box());
                match (p.elements().is_empty(), t) {
                    (false, Some(t)) => p.bounding_box().union(t),
                    (false, None) => p.bounding_box(),
                    (true, Some(t)) => t,
                    (true, None) => self.transform.transform_rect_bbox(Rect::ZERO),
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
                if let Some(p) = &mut b.path {
                    p.obj.apply_transform(a);
                }
                for s in b.splits.iter_mut() {
                    s.obj.apply_transform(a);
                }
                // a fixed spacing is a distance: it scales with the blend
                if let Some(d) = &mut b.spacing {
                    let (_, _, _, sx, sy) = geom::decompose(a);
                    let k = (sx.abs() * sy.abs()).sqrt();
                    if k.is_finite() && k > 0.0 {
                        *d *= k;
                    }
                }
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
                    // concave quarter circle centered at the corner: it leaves each edge at a
                    // right angle, bowing in towards the middle of the rectangle
                    let a2 = a - c;
                    let b2 = b - c;
                    bp.curve_to(a + b2 * k, b + a2 * k, b);
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
        EllipseKind::Ellipse => {
            // kurbo's ellipse ends where it starts but has no ClosePath: without it a converted
            // ellipse became an open curve that couldn't be picked from inside (#97)
            let mut bp = kurbo::Ellipse::new(c, (rx, ry), 0.0).to_path(0.01);
            bp.close_path();
            bp
        }
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

impl PolygonShape {
    /// Number of corners (star tips) the path is made of.
    pub fn corner_count(&self) -> usize {
        let n = self.sides.max(3) as usize;
        if self.kind == PolygonKind::ComplexStar {
            n.max(5)
        } else {
            n
        }
    }

    /// The regular shape's outer node `i` (a corner) or inner node `i` (the middle of the side
    /// after corner `i`, or the star valley there): angle (radians, y down, from 3 o'clock) and
    /// radius (1 = on the bounding ellipse).
    fn node_base(&self, inner: bool, i: usize) -> (f64, f64) {
        let n = self.corner_count() as f64;
        let start = -PI / 2.0;
        if !inner {
            return (start + 2.0 * PI * i as f64 / n, 1.0);
        }
        // inner radius from sharpness: 0% → the middles of the sides, 100% → very thin
        let base_inner = (PI / n).cos();
        let r = match self.kind {
            PolygonKind::Star => base_inner * (1.0 - (self.sharpness / 100.0).clamp(0.0, 0.99)),
            _ => base_inner,
        };
        (start + PI * (2 * i + 1) as f64 / n, r)
    }

    /// Local position of outer or inner node `i`, with the Shape tool edits.
    pub fn node_point(&self, inner: bool, i: usize) -> Point {
        let (a, r) = self.node_base(inner, i);
        let (scale, turn) = if inner { (self.nodes.inner_scale, self.nodes.inner_turn) } else { (self.nodes.outer_scale, self.nodes.outer_turn) };
        let a = a + turn.to_radians();
        let (rx, ry) = (self.w / 2.0, self.h / 2.0);
        Point::new(rx + rx * r * scale * a.cos(), ry + ry * r * scale * a.sin())
    }

    /// The nodes the Shape tool shows: (local point, inner, index). A polygon and a star have an
    /// outer and an inner node per corner; a complex star only its corners.
    pub fn node_points(&self) -> Vec<(Point, bool, usize)> {
        let n = self.corner_count();
        let mut v = vec![];
        for i in 0..n {
            v.push((self.node_point(false, i), false, i));
            if self.kind != PolygonKind::ComplexStar {
                v.push((self.node_point(true, i), true, i));
            }
        }
        v
    }

    /// Shape tool: node `i` (outer or inner) dragged to local point `to`. Every node of the same
    /// set follows, as in CorelDRAW: they all move away from (or toward) the center by the same
    /// factor and turn by the same angle. `radial_only` (Ctrl) keeps the turn.
    pub fn move_node(&mut self, inner: bool, i: usize, to: Point, radial_only: bool) {
        let (rx, ry) = (self.w / 2.0, self.h / 2.0);
        if rx < 1e-9 || ry < 1e-9 {
            return;
        }
        let (a0, r0) = self.node_base(inner, i);
        if r0 < 1e-9 {
            return;
        }
        let u = Vec2::new((to.x - rx) / rx, (to.y - ry) / ry);
        let scale = u.hypot() / r0;
        let turn = (u.y.atan2(u.x) - a0).to_degrees();
        let turn = (turn + 180.0).rem_euclid(360.0) - 180.0;
        let (s, t) = if inner { (&mut self.nodes.inner_scale, &mut self.nodes.inner_turn) } else { (&mut self.nodes.outer_scale, &mut self.nodes.outer_turn) };
        *s = scale;
        if !radial_only {
            *t = turn;
        }
    }
}

impl EllipseShape {
    /// Local point of the ellipse at `deg` (counter-clockwise from 3 o'clock, as `start`/`end`).
    pub fn angle_point(&self, deg: f64) -> Point {
        let a = -deg.to_radians();
        let (rx, ry) = (self.w / 2.0, self.h / 2.0);
        Point::new(rx + rx * a.cos(), ry + ry * a.sin())
    }

    /// The angle (degrees, 0..360, counter-clockwise from 3 o'clock) of a local point seen from
    /// the center, measured on the ellipse (as `start`/`end` are).
    pub fn angle_of(&self, p: Point) -> f64 {
        let (rx, ry) = ((self.w / 2.0).max(1e-9), (self.h / 2.0).max(1e-9));
        let (x, y) = ((p.x - rx) / rx, (p.y - ry) / ry);
        (-y.atan2(x)).to_degrees().rem_euclid(360.0)
    }

    /// The local point is inside the full ellipse.
    pub fn contains_local(&self, p: Point) -> bool {
        let (rx, ry) = ((self.w / 2.0).max(1e-9), (self.h / 2.0).max(1e-9));
        let (x, y) = ((p.x - rx) / rx, (p.y - ry) / ry);
        x * x + y * y < 1.0
    }

    /// The Shape tool's nodes: the start node of a whole ellipse, the start and end nodes of a
    /// pie or an arc (true = start).
    pub fn nodes(&self) -> Vec<(Point, bool)> {
        if self.kind == EllipseKind::Ellipse {
            vec![(self.angle_point(self.start), true)]
        } else {
            vec![(self.angle_point(self.start), true), (self.angle_point(self.end), false)]
        }
    }

    /// Shape tool (#108): a node dragged to local point `to`, as in CorelDRAW: inside the ellipse
    /// it makes a pie, outside an arc. `handle` is the node (true = start); `None` for the one
    /// node of a whole ellipse, which splits in two: the wedge opens on the side it is dragged to,
    /// so the node that moves is the one at that edge of the visible part. `step` (Ctrl) keeps
    /// the angle to multiples of it. Returns the new shape and the node being dragged (still
    /// `None` while the pointer has not left the node's angle).
    pub fn drag_node(&self, handle: Option<bool>, to: Point, step: Option<f64>) -> (EllipseShape, Option<bool>) {
        let mut ang = self.angle_of(to);
        if let Some(s) = step.filter(|s| *s > 0.0) {
            ang = ((ang / s).round() * s).rem_euclid(360.0);
        }
        let whole = self.kind == EllipseKind::Ellipse;
        let handle = match handle {
            Some(h) => h,
            None => {
                let d = (ang - self.start + 180.0).rem_euclid(360.0) - 180.0;
                if d.abs() < 0.5 {
                    return (self.clone(), None);
                }
                // the visible part runs counter-clockwise from `start` (from `end` when clockwise)
                (d > 0.0) != self.clockwise
            }
        };
        let mut e = self.clone();
        if whole {
            e.end = e.start;
        }
        if handle {
            e.start = ang;
        } else {
            e.end = ang;
        }
        e.kind = if self.contains_local(to) { EllipseKind::Pie } else { EllipseKind::Arc };
        (e, Some(handle))
    }
}

pub fn polygon_path(p: &PolygonShape) -> BezPath {
    let n = p.corner_count();
    let mut bp = BezPath::new();
    match p.kind {
        PolygonKind::Polygon | PolygonKind::Star => {
            // a regular polygon is its corners; a star, or a polygon whose nodes were moved,
            // alternates corners and inner nodes
            let inner = p.kind == PolygonKind::Star || !p.nodes.is_regular();
            for i in 0..n {
                let q = p.node_point(false, i);
                if i == 0 {
                    bp.move_to(q)
                } else {
                    bp.line_to(q)
                }
                if inner {
                    bp.line_to(p.node_point(true, i));
                }
            }
            bp.close_path();
        }
        PolygonKind::ComplexStar => {
            let skip = (((p.sharpness / 100.0) * ((n - 1) / 2) as f64).round() as usize).clamp(2, (n - 1) / 2);
            let pts: Vec<Point> = (0..n).map(|i| p.node_point(false, i)).collect();
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
    let carrier = DimCarrier::of(d);
    // the value only shapes the lines when they break around it, reach out to it or box it
    let text = carrier.filter(|_| d.placement == TextPlacement::Through || d.boxed_text || !d.text_centered).map(|_| dimension_text(d));
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
            line_spans(&mut bp, a2, b2, &spans(d, carrier, text.as_ref(), 2.0 * sz));
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
                line_spans(&mut bp, Point::new(a.x, y), Point::new(b.x, y), &spans(d, carrier, text.as_ref(), 2.0 * sz));
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
                line_spans(&mut bp, Point::new(x, a.y), Point::new(x, b.y), &spans(d, carrier, text.as_ref(), 2.0 * sz));
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
            let pieces = spans(d, carrier, text.as_ref(), 2.0 * sz);
            if pieces == [(0.0, sweep.abs())] {
                let arc = kurbo::Arc::new(v, (r, r), a1, a2 - a1, 0.0);
                let p0 = Point::new(v.x + r * a1.cos(), v.y + r * a1.sin());
                bp.move_to(p0);
                arc.to_cubic_beziers(0.01, |p1, p2, p3| bp.curve_to(p1, p2, p3));
            } else {
                // broken around the value, or reaching out to it
                let sg = if sweep < 0.0 { -1.0 } else { 1.0 };
                for (u0, u1) in pieces {
                    let th = a1 + sg * u0;
                    bp.move_to(Point::new(v.x + r * th.cos(), v.y + r * th.sin()));
                    kurbo::Arc::new(v, (r, r), th, sg * (u1 - u0), 0.0).to_cubic_beziers(0.01, |p1, p2, p3| bp.curve_to(p1, p2, p3));
                }
            }
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
    if d.boxed_text {
        if let Some(b) = text.as_ref().and_then(|t| t.clear.map(|r| t.transform * r.to_path(0.1))) {
            bp.extend(b.elements().iter().copied());
        }
    }
    bp
}

/// Draw the stretches `spans` (distances from `a`) of the dimension line from `a` to `b`; the
/// whole line is drawn from `a` to `b` exactly.
fn line_spans(bp: &mut BezPath, a: Point, b: Point, spans: &[(f64, f64)]) {
    let len = (b - a).hypot();
    let t = if len > 1e-12 { (b - a) / len } else { Vec2::ZERO };
    let at = |u: f64| {
        if u == 0.0 {
            a
        } else if u == len {
            b
        } else {
            a + t * u
        }
    };
    for &(u0, u1) in spans {
        bp.move_to(at(u0));
        bp.line_to(at(u1));
    }
}

/// The space between a dimension line and its value (above or below), times the font size.
const DIM_TEXT_GAP: f64 = 0.3;
/// The margin around the value kept by a line broken around it and by the box around it,
/// times the font size.
const DIM_TEXT_PAD: f64 = 0.2;

/// An angle turned half a turn when needed, so that text along it doesn't read upside down.
fn readable_angle(a: f64) -> f64 {
    let mut a = a;
    while a > PI / 2.0 {
        a -= PI;
    }
    while a < -PI / 2.0 {
        a += PI;
    }
    a
}

fn rotate_vec(v: Vec2, a: f64) -> Vec2 {
    (Affine::rotate(a) * v.to_point()).to_vec2()
}

/// The line or arc the value of a dimension is placed against. Positions along it (`u`) are
/// distances from its start on a line, and angles from its start in the sweep's direction on an
/// arc.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DimCarrier {
    /// A dimension line from `a` to `b`; `angle`: the readable angle of text along it.
    Line { a: Point, b: Point, angle: f64 },
    /// The arc of an angular dimension: centre, radius, start angle and signed sweep (radians).
    Arc { c: Point, r: f64, start: f64, sweep: f64 },
}

impl DimCarrier {
    /// The dimension line of a linear dimension, the arc of an angular one; none for callouts.
    fn of(d: &DimensionObj) -> Option<DimCarrier> {
        let pts = &d.points;
        match d.kind {
            DimensionKind::Parallel | DimensionKind::Segment if pts.len() >= 2 => {
                let (a, b) = (pts[0], pts[1]);
                if (b - a).hypot2() < 1e-18 {
                    return None;
                }
                let lp = pts.get(2).copied().unwrap_or(a.midpoint(b));
                let dir = (b - a).normalize();
                let normal = Vec2::new(-dir.y, dir.x);
                let off = (lp - a).dot(normal);
                Some(DimCarrier::Line { a: a + normal * off, b: b + normal * off, angle: readable_angle(dir.y.atan2(dir.x)) })
            }
            DimensionKind::HorizontalVertical if pts.len() >= 2 => {
                let (a, b) = (pts[0], pts[1]);
                let (_, mid, ang) = dimension_label(d);
                Some(if ang == 0.0 {
                    DimCarrier::Line { a: Point::new(a.x, mid.y), b: Point::new(b.x, mid.y), angle: 0.0 }
                } else {
                    DimCarrier::Line { a: Point::new(mid.x, a.y), b: Point::new(mid.x, b.y), angle: -PI / 2.0 }
                })
            }
            DimensionKind::Angular if pts.len() >= 3 => {
                let (v, a, b) = (pts[0], pts[1], pts[2]);
                let r = pts.get(3).map(|lp| (*lp - v).hypot() * 0.8).unwrap_or(30.0);
                let start = (a - v).atan2();
                let mut sweep = (b - v).atan2() - start;
                if sweep > PI {
                    sweep -= 2.0 * PI;
                }
                if sweep < -PI {
                    sweep += 2.0 * PI;
                }
                Some(DimCarrier::Arc { c: v, r, start, sweep })
            }
            _ => None,
        }
    }

    /// The length of the line, the angle of the arc.
    fn len(&self) -> f64 {
        match *self {
            DimCarrier::Line { a, b, .. } => (b - a).hypot(),
            DimCarrier::Arc { sweep, .. } => sweep.abs(),
        }
    }

    /// How far a unit of `u` reaches on the page: 1 on a line, the radius on an arc.
    fn scale(&self) -> f64 {
        match *self {
            DimCarrier::Line { .. } => 1.0,
            DimCarrier::Arc { r, .. } => r.max(1e-9),
        }
    }

    fn angle_at(&self, u: f64) -> f64 {
        match *self {
            DimCarrier::Line { .. } => 0.0,
            DimCarrier::Arc { start, sweep, .. } => start + if sweep < 0.0 { -u } else { u },
        }
    }

    fn at(&self, u: f64) -> Point {
        match *self {
            DimCarrier::Line { a, .. } => a + self.tangent(u) * u,
            DimCarrier::Arc { c, r, .. } => {
                let th = self.angle_at(u);
                c + Vec2::new(th.cos(), th.sin()) * r
            }
        }
    }

    /// The direction of increasing `u`.
    fn tangent(&self, u: f64) -> Vec2 {
        match *self {
            DimCarrier::Line { a, b, angle } => {
                let l = (b - a).hypot();
                if l > 1e-12 {
                    (b - a) / l
                } else {
                    Vec2::new(angle.cos(), angle.sin())
                }
            }
            DimCarrier::Arc { sweep, .. } => {
                let th = self.angle_at(u);
                Vec2::new(-th.sin(), th.cos()) * if sweep < 0.0 { -1.0 } else { 1.0 }
            }
        }
    }

    /// "Above": the up of text along the line; out from the vertex on an arc.
    fn up(&self, u: f64) -> Vec2 {
        match *self {
            DimCarrier::Line { angle, .. } => Vec2::new(angle.sin(), -angle.cos()),
            DimCarrier::Arc { .. } => {
                let th = self.angle_at(u);
                Vec2::new(th.cos(), th.sin())
            }
        }
    }

    /// The angle of the value: along the line unless forced horizontal; level on an arc.
    fn text_angle(&self, d: &DimensionObj) -> f64 {
        match *self {
            DimCarrier::Line { angle, .. } if !d.horizontal_text => angle,
            _ => 0.0,
        }
    }

    /// The position of `p` along the carrier: its projection on a line; on an arc its angle,
    /// taken the way round nearer to the arc.
    fn param(&self, p: Point) -> f64 {
        match *self {
            DimCarrier::Line { a, .. } => (p - a).dot(self.tangent(0.0)),
            DimCarrier::Arc { c, start, sweep, .. } => {
                let sg = if sweep < 0.0 { -1.0 } else { 1.0 };
                let len = sweep.abs();
                let u = (((p - c).atan2() - start) * sg).rem_euclid(2.0 * PI);
                // past the end: nearer to the start the other way round
                if u > len && u - len > 2.0 * PI - u {
                    u - 2.0 * PI
                } else {
                    u
                }
            }
        }
    }

    /// Where the value goes: the middle with Center Text Between Extension Lines, else the
    /// label point.
    fn anchor(&self, d: &DimensionObj) -> f64 {
        let label = match d.kind {
            DimensionKind::Angular => d.points.get(3),
            _ => d.points.get(2),
        };
        match label {
            Some(p) if !d.text_centered => self.param(*p),
            _ => self.len() / 2.0,
        }
    }

    fn anchor_point(&self, d: &DimensionObj) -> Point {
        match *self {
            // the middle of the line computed as before 0.6.2
            DimCarrier::Line { a, b, .. } if d.text_centered => a.midpoint(b),
            _ => self.at(self.anchor(d)),
        }
    }
}

/// The value of a dimension as drawn: its text, where it goes and the box kept around it, in the
/// dimension's own space (that of `points`).
#[derive(Clone, Debug)]
pub struct DimText {
    pub text: String,
    /// Places the label (artistic text, centred, baseline at the origin) in the dimension.
    pub transform: Affine,
    /// The glyph outlines, in the label's space.
    pub glyphs: BezPath,
    /// The ink box of the glyphs, in the label's space (none without text).
    pub ink: Option<Rect>,
    /// The ink box with a margin: what a line broken around the value leaves out and what Draw
    /// Box Around Text draws, in the label's space.
    pub clear: Option<Rect>,
}

impl DimText {
    /// The glyphs in the dimension's space.
    pub fn path(&self) -> BezPath {
        self.transform * self.glyphs.clone()
    }

    /// The ink box in the dimension's space (turned with the text).
    pub fn ink_outline(&self) -> Option<BezPath> {
        self.ink.map(|r| self.transform * r.to_path(0.1))
    }

    /// Is `p` (dimension space) on the value, within `tol`?
    pub fn hit(&self, p: Point, tol: f64) -> bool {
        self.ink.is_some_and(|r| r.inflate(tol, tol).contains(self.transform.inverse() * p))
    }
}

fn dimension_glyphs(text: &str, size: f64) -> BezPath {
    if text.is_empty() {
        return BezPath::new();
    }
    let mut t = TextObj::artistic(text, text::CharFormat { size, ..Default::default() });
    t.align = text::TextAlign::Center;
    text::layout(&t).to_path()
}

/// The value of a dimension and where it goes (CorelDRAW's `Placement`, `TextCentered`,
/// `HorizontalText` and `BoxedText`). Above puts the baseline a gap above the line (where the
/// value always sat before 0.6.2), Below the top of the text a gap below it, Through the middle
/// of the text on it; a boxed value keeps its box clear of the line instead. A value forced
/// horizontal, or on an arc, keeps its whole box that far from the line, along the line's up.
pub fn dimension_text(d: &DimensionObj) -> DimText {
    let text = dimension_label(d).0;
    let size = d.font_size;
    let glyphs = dimension_glyphs(&text, size);
    let ink = (!glyphs.elements().is_empty()).then(|| glyphs.bounding_box());
    let pad = size * DIM_TEXT_PAD;
    let gap = size * DIM_TEXT_GAP;
    let clear = ink.map(|r| r.inflate(pad, pad));
    let transform = match DimCarrier::of(d) {
        // callouts: the value at the label point, as always
        None => {
            let (_, at, ang) = dimension_label(d);
            Affine::translate(at.to_vec2()) * Affine::rotate(readable_angle(ang)) * Affine::translate((0.0, -gap))
        }
        Some(c) => {
            let u = c.anchor(d);
            let angle = c.text_angle(d);
            // the line's up in the label's space
            let up = rotate_vec(c.up(u), -angle);
            // what keeps clear of the line: the text from its top down to the baseline, or the box
            let body = match ink {
                Some(i) if d.boxed_text => i.inflate(pad, pad),
                Some(i) => Rect::new(i.x0, i.y0.min(0.0), i.x1, 0.0),
                None => Rect::ZERO,
            };
            let reach = body.width() / 2.0 * up.x.abs() + body.height() / 2.0 * up.y.abs();
            let target = match d.placement {
                TextPlacement::Above => reach + gap,
                TextPlacement::Through => 0.0,
                TextPlacement::Below => -(reach + gap),
            };
            let shift = up * (target - body.center().to_vec2().dot(up));
            Affine::translate(c.anchor_point(d).to_vec2()) * Affine::rotate(angle) * Affine::translate(shift)
        }
    };
    DimText { text, transform, glyphs, ink, clear }
}

/// The stretches of a dimension's line or arc to draw (positions along it, see [`DimCarrier`]):
/// all of it, reaching out to a value set past its ends, broken around a value that goes
/// through it. The ends keep a stub of `stub` points (a third of the line at most) when the value
/// is wider than the line.
fn spans(d: &DimensionObj, carrier: Option<DimCarrier>, text: Option<&DimText>, stub: f64) -> Vec<(f64, f64)> {
    let Some(c) = carrier else { return vec![] };
    let len = c.len();
    let (Some(t), Some(clear)) = (text, text.and_then(|t| t.clear)) else { return vec![(0.0, len)] };
    let u = c.anchor(d);
    let inside = (0.0..=len).contains(&u);
    let (mut lo, mut hi): (f64, f64) = (0.0, len);
    if !inside {
        // the line goes on under the value
        let mid = c.param(t.transform * clear.center());
        let along = rotate_vec(c.tangent(mid), -c.text_angle(d));
        let half = (clear.width() / 2.0 * along.x.abs() + clear.height() / 2.0 * along.y.abs()) / c.scale();
        lo = lo.min(mid - half);
        hi = hi.max(mid + half);
    }
    if d.placement != TextPlacement::Through {
        return vec![(lo, hi)];
    }
    let Some((mut b0, mut b1)) = crossing(&c, t, clear, u) else { return vec![(lo, hi)] };
    if inside {
        let stub = (stub / c.scale()).min(len / 3.0);
        b0 = b0.max(stub);
        b1 = b1.min(len - stub);
    }
    if b1 <= b0 {
        return vec![(lo, hi)];
    }
    let mut v = vec![];
    if b0 > lo {
        v.push((lo, b0.min(hi)));
    }
    if b1 < hi {
        v.push((b1.max(lo), hi));
    }
    v
}

/// Where the line or arc runs through the box `clear` of the value placed at `u`: the stretch
/// to leave out.
fn crossing(c: &DimCarrier, t: &DimText, clear: Rect, u: f64) -> Option<(f64, f64)> {
    let inv = t.transform.inverse();
    match *c {
        DimCarrier::Line { a, .. } => {
            // the line in the label's space (the text is only turned and moved: lengths stay)
            let p = inv * a;
            let dir = inv * (a + c.tangent(0.0)) - p;
            let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
            for (o, dv, lo, hi) in [(p.x, dir.x, clear.x0, clear.x1), (p.y, dir.y, clear.y0, clear.y1)] {
                if dv.abs() < 1e-12 {
                    if o < lo || o > hi {
                        return None;
                    }
                } else {
                    let (s0, s1) = ((lo - o) / dv, (hi - o) / dv);
                    t0 = t0.max(s0.min(s1));
                    t1 = t1.min(s0.max(s1));
                }
            }
            (t0 < t1).then_some((t0, t1))
        }
        DimCarrier::Arc { .. } => {
            let inside = |v: f64| clear.contains(inv * c.at(v));
            if !inside(u) {
                return None;
            }
            // out from the value both ways to where the arc leaves its box
            let edge = |dir: f64| {
                let (mut lo, mut hi) = (0.0, (clear.width().hypot(clear.height()) / c.scale()).max(1e-6));
                while inside(u + dir * hi) && hi < 2.0 * PI {
                    lo = hi;
                    hi *= 2.0;
                }
                for _ in 0..48 {
                    let m = (lo + hi) / 2.0;
                    if inside(u + dir * m) {
                        lo = m;
                    } else {
                        hi = m;
                    }
                }
                u + dir * hi
            };
            Some((edge(-1.0), edge(1.0)))
        }
    }
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

/// The intermediate objects of a blend, with its accelerations, color direction and loop, its
/// split points (in their places) and its path. Everything that draws or exports a blend draws
/// the path (if any), the start, these and the end.
pub fn blend_steps(b: &BlendObj, mut new_id: impl FnMut() -> ObjId) -> Vec<Object> {
    crate::blend_path::steps(b, &mut new_id)
}

/// Interpolate two objects for a blend step (t in 0..1). Geometry is matched by flattening both
/// to the same number of points per subpath.
pub fn blend_step(a: &Object, b: &Object, t: f64, id: ObjId, rotation: f64) -> Object {
    blend_step_paths(a, b, &a.world_path(), &b.world_path(), (None, None), t, id, rotation)
}

/// [`blend_step`] on given outlines of `a` and `b` (the objects give the styles), with the closed
/// subpath of each outline that Map Nodes matched read from that fraction of its length on.
#[allow(clippy::too_many_arguments)]
pub fn blend_step_paths(a: &Object, b: &Object, pa: &BezPath, pb: &BezPath, map: (Option<(usize, f64)>, Option<(usize, f64)>), t: f64, id: ObjId, rotation: f64) -> Object {
    let ra = resample_from(pa, 96, map.0);
    let rb = resample_from(pb, 96, map.1);
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
    resample_from(path, n, None)
}

/// [`resample`], with the closed subpath `map.0` read from the fraction `map.1` of its length
/// on (Map Nodes).
pub fn resample_from(path: &BezPath, n: usize, map: Option<(usize, f64)>) -> Vec<(Vec<Point>, bool)> {
    let polys = geom::flatten_polylines(path, 0.2);
    polys
        .into_iter()
        .enumerate()
        .map(|(si, (pts, closed))| {
            let mut pts = pts;
            if closed {
                pts.push(pts[0]);
            }
            let mut acc = vec![0.0];
            for w in pts.windows(2) {
                acc.push(acc.last().unwrap() + (w[1] - w[0]).hypot());
            }
            let total = *acc.last().unwrap();
            let off = match map {
                Some((k, f)) if k == si && closed && f.is_finite() => f.rem_euclid(1.0) * total,
                _ => 0.0,
            };
            let mut out = vec![];
            let m = if closed { n } else { n + 1 };
            for i in 0..m {
                let mut s = total * i as f64 / n as f64;
                if off > 0.0 {
                    s = (s + off).rem_euclid(total.max(1e-12));
                }
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

/// #150: the text position of dimensions (CorelDRAW's Placement, TextCentered, HorizontalText and
/// BoxedText).
#[cfg(test)]
mod dimension_text_tests {
    use super::*;
    use kurbo::PathEl;

    const SIZE: f64 = 12.0;

    fn dim(kind: DimensionKind, points: Vec<Point>) -> DimensionObj {
        DimensionObj { kind, points, font_size: SIZE, ..Default::default() }
    }

    /// A horizontal parallel dimension 200 pt long, its line at y = 100.
    fn horizontal() -> DimensionObj {
        dim(DimensionKind::Parallel, vec![Point::new(100.0, 140.0), Point::new(300.0, 140.0), Point::new(200.0, 100.0)])
    }

    fn with(mut d: DimensionObj, f: impl FnOnce(&mut DimensionObj)) -> DimensionObj {
        f(&mut d);
        d
    }

    /// The ink box of the value in the dimension's space.
    fn ink(d: &DimensionObj) -> Rect {
        dimension_text(d).ink_outline().expect("a value").bounding_box()
    }

    /// The straight segments of a path (move-to then line-to), in order.
    fn segments(bp: &BezPath) -> Vec<(Point, Point)> {
        let mut out = vec![];
        let mut at = Point::ZERO;
        for el in bp.elements() {
            match *el {
                PathEl::MoveTo(p) => at = p,
                PathEl::LineTo(p) => {
                    out.push((at, p));
                    at = p;
                }
                PathEl::QuadTo(_, p) | PathEl::CurveTo(_, _, p) => at = p,
                PathEl::ClosePath => {}
            }
        }
        out
    }

    /// The pieces of the dimension line y = `y` (horizontal segments on it), left to right.
    fn on_line(d: &DimensionObj, y: f64) -> Vec<(f64, f64)> {
        let mut v: Vec<(f64, f64)> = segments(&dimension_path(d))
            .into_iter()
            .filter(|(a, b)| (a.y - y).abs() < 1e-6 && (b.y - y).abs() < 1e-6 && (a.x - b.x).abs() > 1e-6)
            .map(|(a, b)| (a.x.min(b.x), a.x.max(b.x)))
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    /// Documents from before the option existed keep their look: the value centred, its baseline
    /// 0.3 × the font size above an unbroken line, turned with the line, as in 0.6.1.
    #[test]
    fn defaults_place_the_value_as_before() {
        for d in [
            horizontal(),
            dim(DimensionKind::Parallel, vec![Point::new(0.0, 0.0), Point::new(120.0, 90.0), Point::new(20.0, 60.0)]),
            dim(DimensionKind::Segment, vec![Point::new(300.0, 0.0), Point::new(100.0, 50.0), Point::new(0.0, 0.0)]),
            dim(DimensionKind::HorizontalVertical, vec![Point::new(0.0, 0.0), Point::new(30.0, 200.0), Point::new(-40.0, 90.0)]),
            dim(DimensionKind::HorizontalVertical, vec![Point::new(0.0, 0.0), Point::new(150.0, 20.0), Point::new(70.0, 80.0)]),
        ] {
            let (_, pos, ang) = dimension_label(&d);
            let old = Affine::translate(pos.to_vec2()) * Affine::rotate(readable_angle(ang)) * Affine::translate((0.0, -SIZE * 0.3));
            let new = dimension_text(&d).transform;
            for (a, b) in old.as_coeffs().iter().zip(new.as_coeffs()) {
                assert!((a - b).abs() < 1e-9, "{:?}: {old:?} != {new:?}", d.kind);
            }
        }
        // and the line runs unbroken from one arrow to the other
        assert_eq!(on_line(&horizontal(), 100.0), vec![(100.0, 300.0)]);
    }

    #[test]
    fn above_through_and_below_the_line() {
        let gap = SIZE * 0.3;
        let above = horizontal();
        let t = dimension_text(&above);
        assert!(close(t.transform.translation().y, 100.0 - gap), "Above: the baseline a gap above the line");
        assert!(ink(&above).y1 < 100.0);
        let below = with(horizontal(), |d| d.placement = TextPlacement::Below);
        assert!(close(ink(&below).y0, 100.0 + gap), "Below: the top of the text a gap below the line: {:?}", ink(&below));
        let through = with(horizontal(), |d| d.placement = TextPlacement::Through);
        let t = dimension_text(&through);
        let i = t.ink.unwrap();
        // the text above its baseline is centred on the line
        let mid = t.transform * Point::new(0.0, i.y0 / 2.0);
        assert!(close(mid.y, 100.0), "Through: {mid:?}");
        // all three stay centred between the extension lines
        for d in [&above, &below, &through] {
            assert!((ink(d).center().x - 200.0).abs() < SIZE, "{:?}", ink(d));
        }
    }

    #[test]
    fn through_breaks_the_line_around_the_value() {
        let d = with(horizontal(), |d| d.placement = TextPlacement::Through);
        let i = ink(&d);
        let pad = SIZE * 0.2;
        let pieces = on_line(&d, 100.0);
        assert_eq!(pieces.len(), 2, "{pieces:?}");
        assert!(close(pieces[0].0, 100.0) && close(pieces[1].1, 300.0), "the line still reaches both arrows: {pieces:?}");
        assert!(close(pieces[0].1, i.x0 - pad) && close(pieces[1].0, i.x1 + pad), "broken a margin around the text: {pieces:?} {i:?}");
        // a value wider than the line: the arrows keep a stub, no piece is inverted
        let short = with(d.clone(), |d| d.points = vec![Point::new(190.0, 140.0), Point::new(210.0, 140.0), Point::new(200.0, 100.0)]);
        let pieces = on_line(&short, 100.0);
        assert_eq!(pieces.len(), 2, "{pieces:?}");
        // two arrowheads long, a third of the line at most
        let stub = ((SIZE * 0.5).max(3.0) * 2.0).min(20.0 / 3.0);
        assert!(close(pieces[0].1 - pieces[0].0, stub) && close(pieces[1].1 - pieces[1].0, stub), "{pieces:?}");
    }

    #[test]
    fn boxed_text_adds_a_box_around_the_value() {
        for p in [TextPlacement::Above, TextPlacement::Through, TextPlacement::Below] {
            let plain = with(horizontal(), |d| d.placement = p);
            let boxed = with(plain.clone(), |d| d.boxed_text = true);
            let closes = |d: &DimensionObj| dimension_path(d).elements().iter().filter(|e| matches!(e, PathEl::ClosePath)).count();
            assert_eq!(closes(&boxed), closes(&plain) + 1, "{p:?}: one more closed figure, the box");
            let t = dimension_text(&boxed);
            let r = (t.transform * t.clear.unwrap().to_path(0.1)).bounding_box();
            let i = ink(&boxed);
            assert!(r.contains(Point::new(i.x0, i.y0)) && r.contains(Point::new(i.x1, i.y1)), "{p:?}: the box holds the text");
            assert!(close(r.width() - i.width(), SIZE * 0.4), "{p:?}: a margin of 0.2 × the font size");
            match p {
                // the box, not the text, keeps the gap from the line
                TextPlacement::Above => assert!(close(r.y1, 100.0 - SIZE * 0.3), "{r:?}"),
                TextPlacement::Below => assert!(close(r.y0, 100.0 + SIZE * 0.3), "{r:?}"),
                // the line stops at the box
                TextPlacement::Through => {
                    let pieces = on_line(&boxed, 100.0);
                    assert!(close(pieces[0].1, r.x0) && close(pieces[1].0, r.x1), "{pieces:?} {r:?}");
                }
            }
        }
    }

    #[test]
    fn centered_uses_the_middle_else_the_label_point() {
        let off = |c: bool| with(horizontal(), |d| {
            d.points[2] = Point::new(130.0, 100.0);
            d.text_centered = c;
        });
        assert!(close(dimension_text(&off(true)).transform.translation().x, 200.0));
        assert!(close(dimension_text(&off(false)).transform.translation().x, 130.0));
        // the label point decides only where along the line the value goes
        assert!(close(dimension_text(&off(false)).transform.translation().y, 100.0 - SIZE * 0.3));
        // set past an extension line, the line reaches out under the value
        let out = with(horizontal(), |d| {
            d.points[2] = Point::new(360.0, 100.0);
            d.text_centered = false;
        });
        let i = ink(&out);
        let pieces = on_line(&out, 100.0);
        assert_eq!(pieces.len(), 1);
        assert!(close(pieces[0].0, 100.0) && pieces[0].1 >= i.x1, "{pieces:?} {i:?}");
        // through and past the end: the line reaches out to the value and stops at it
        let out = with(out, |d| d.placement = TextPlacement::Through);
        let pieces = on_line(&out, 100.0);
        assert_eq!(pieces.len(), 1, "{pieces:?}");
        assert!(close(pieces[0].0, 100.0) && pieces[0].1 > 300.0 && pieces[0].1 < ink(&out).x0, "{pieces:?}");
    }

    #[test]
    fn force_horizontal_keeps_the_value_level_and_clear_of_the_line() {
        // a line at 30°, and a vertical one
        for pts in [vec![Point::new(0.0, 0.0), Point::new(173.2, 100.0), Point::new(0.0, -40.0)], vec![Point::new(0.0, 0.0), Point::new(0.0, 200.0), Point::new(-50.0, 100.0)]] {
            let turned = dim(DimensionKind::Parallel, pts.clone());
            assert!(dimension_text(&turned).transform.as_coeffs()[1].abs() > 0.1, "the value turns with the line");
            let c = DimCarrier::of(&turned).unwrap();
            let DimCarrier::Line { a, b, .. } = c else { unreachable!() };
            let up = c.up(0.0);
            let side = |p: Point| (p - a).dot(up);
            for (placement, sign) in [(TextPlacement::Above, 1.0), (TextPlacement::Below, -1.0), (TextPlacement::Through, 0.0)] {
                let d = with(turned.clone(), |d| {
                    d.horizontal_text = true;
                    d.placement = placement;
                });
                let t = dimension_text(&d);
                let k = t.transform.as_coeffs();
                assert!(k[1].abs() < 1e-12 && close(k[0], 1.0), "level: {k:?}");
                let r = ink(&d);
                let corners = [Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x0, r.y1), Point::new(r.x1, r.y1)];
                if sign == 0.0 {
                    assert!(corners.iter().any(|p| side(*p) > 0.0) && corners.iter().any(|p| side(*p) < 0.0), "{placement:?}: on the line");
                    // and broken around it
                    let gap = segments(&dimension_path(&d)).iter().filter(|(p, q)| side(*p).abs() < 1e-6 && side(*q).abs() < 1e-6).count();
                    assert_eq!(gap, 2, "{placement:?}: two pieces of line");
                } else {
                    let nearest = corners.iter().map(|p| side(*p) * sign).fold(f64::INFINITY, f64::min);
                    assert!(nearest > 0.0, "{placement:?}: the whole value on its side of the line ({nearest})");
                }
                // still by the middle of the line
                assert!((r.center() - a.midpoint(b)).hypot() < 60.0);
            }
        }
    }

    /// An angle of 90° at (100, 100) with its arc at radius 80.
    fn angular() -> DimensionObj {
        dim(DimensionKind::Angular, vec![Point::new(100.0, 100.0), Point::new(300.0, 100.0), Point::new(100.0, 300.0), Point::new(200.0, 100.0)])
    }

    #[test]
    fn angular_above_is_outward_and_through_breaks_the_arc() {
        let v = Point::new(100.0, 100.0);
        let r = 80.0;
        let dist = |d: &DimensionObj| (ink(d).center() - v).hypot();
        let above = angular();
        let below = with(angular(), |d| d.placement = TextPlacement::Below);
        let through = with(angular(), |d| d.placement = TextPlacement::Through);
        assert!(dist(&above) > r + SIZE * 0.3, "outward: {}", dist(&above));
        assert!(dist(&below) < r - SIZE * 0.3, "inward: {}", dist(&below));
        assert!((dist(&through) - r).abs() < SIZE, "on the arc: {}", dist(&through));
        // centred: on the bisector (45°)
        for d in [&above, &below, &through] {
            let c = ink(d).center() - v;
            assert!((c.y.atan2(c.x) - PI / 4.0).abs() < 0.05, "{:?}", c);
        }
        // the arc is one piece, or two around the value
        let arcs = |d: &DimensionObj| segments(&dimension_path(d)).len();
        let moves = |d: &DimensionObj| dimension_path(d).elements().iter().filter(|e| matches!(e, PathEl::MoveTo(_))).count();
        assert_eq!(arcs(&through), arcs(&above));
        assert_eq!(moves(&through), moves(&above) + 1, "the arc broken around the value");
        // no point of the arc inside the value's box
        let t = dimension_text(&through);
        let inv = t.transform.inverse();
        let clear = t.clear.unwrap().inflate(-0.05, -0.05);
        for el in dimension_path(&through).elements() {
            if let PathEl::CurveTo(_, _, p) | PathEl::MoveTo(p) = *el {
                assert!(!clear.contains(inv * p), "{p:?} inside the box");
            }
        }
        // not centred: at the label point's angle
        let off = with(angular(), |d| {
            d.points[3] = Point::new(100.0 + 100.0 * 0.3f64.cos(), 100.0 + 100.0 * 0.3f64.sin());
            d.text_centered = false;
        });
        let c = ink(&off).center() - v;
        assert!((c.y.atan2(c.x) - 0.3).abs() < 0.1, "{c:?}");
        // an angular value is always level
        let level = with(angular(), |d| d.horizontal_text = true);
        assert_eq!(dimension_text(&level).transform, dimension_text(&angular()).transform);
    }

    #[test]
    fn bounds_and_hits_take_in_the_value() {
        let mk = |d: DimensionObj| Object::new(1, ObjKind::Dimension(d), Affine::IDENTITY, Style::default());
        for p in [TextPlacement::Above, TextPlacement::Below] {
            let d = with(horizontal(), |d| d.placement = p);
            let i = ink(&d);
            let o = mk(d);
            let b = o.bounds();
            assert!(b.contains(Point::new(i.x0, i.y0)) && b.contains(Point::new(i.x1, i.y1)), "{p:?}: {b:?} {i:?}");
            let doc = crate::document::Document::default();
            assert!(doc.hit_body(&o, i.center(), 0.5, false), "{p:?}: a click on the value");
        }
        // the box around the value is in the outline
        let o = mk(with(horizontal(), |d| d.boxed_text = true));
        let t = match &o.kind {
            ObjKind::Dimension(d) => dimension_text(d),
            _ => unreachable!(),
        };
        let r = (t.transform * t.clear.unwrap().to_path(0.1)).bounding_box();
        assert!(o.bounds().contains(Point::new(r.x0, r.y0)));
    }

    #[test]
    fn old_files_load_with_the_old_placement() {
        let d = with(horizontal(), |d| {
            d.placement = TextPlacement::Below;
            d.text_centered = false;
            d.horizontal_text = true;
            d.boxed_text = true;
        });
        // a dimension saved by 0.6.1: no text position fields
        let mut v = serde_json::to_value(&d).unwrap();
        for k in ["placement", "text_centered", "horizontal_text", "boxed_text"] {
            assert!(v.as_object_mut().unwrap().remove(k).is_some(), "{k}");
        }
        let old: DimensionObj = serde_json::from_value(v).unwrap();
        assert_eq!((old.placement, old.text_centered, old.horizontal_text, old.boxed_text), (TextPlacement::Above, true, false, false));
        assert_eq!(dimension_text(&old).transform, dimension_text(&horizontal()).transform);
        // and the new fields survive a save
        let back: DimensionObj = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
        assert_eq!(serde_json::to_value(TextPlacement::Through).unwrap(), serde_json::json!("Through"));
    }
}
