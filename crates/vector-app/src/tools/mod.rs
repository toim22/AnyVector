//! Tools: state shared by the toolbox, property bar and canvas, plus the operations the
//! property bar and dockers trigger (node editing, corner rounding, text insertion).
//! Pointer interaction lives in `interact`, on-canvas feedback in `overlay`.

pub mod ids;
mod interact;
mod overlay;
pub mod shapes;

pub use ids::*;
pub use interact::{guide_at, handle_input, handle_point, handle_table_keys, handle_text_keys, table_typing, Input};
pub use overlay::draw_overlay;

use crate::app::VectorApp;
use crate::docview::DocView;
use crate::settings::Settings;
use kurbo::{Affine, Point, Rect, Vec2};
use std::collections::HashMap;
use vector_core::object::{DistortKind, DropShadow};
use vector_core::path::{Node, NodeKind, NodePath, SubPath};
use vector_core::text::{CharFormat, TextAlign, TextObj};
use vector_core::{Color, Fill, ObjId, ObjKind, Object, Outline, Style};

pub use shapes::COMMON_SHAPES;

/// A one-shot "click an object" target set by commands such as PowerClip > Place Inside.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickTarget {
    PowerClip,
    /// Copy Properties From: which properties to copy.
    CopyProperties { pen: bool, color: bool, fill: bool, text: bool },
    /// Copy Effect / Clone Effect From: the effect (see `Cmd::CloneEffectFrom`), 255 = all.
    CopyEffect { kind: u8, clone: bool },
    FitToPath,
    /// Order ▸ In Front Of / Behind.
    Order { in_front: bool },
    CopyPowerClip,
}

/// Kind of interactive transform on the Pick tool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Xform {
    Move,
    /// Handle index 0..8: TL, T, TR, R, BR, B, BL, L.
    Scale(usize),
    Rotate,
    Skew(usize),
}

/// The pointer drag in progress.
#[derive(Clone, Debug, Default)]
pub enum Drag {
    #[default]
    None,
    Marquee { start: Point },
    /// Object ▸ Create ▸ Pattern Fill: the area the pattern is made from.
    PatternArea { start: Point },
    Lasso { points: Vec<Point> },
    Transform { kind: Xform, start: Point, bb: Rect, ids: Vec<ObjId>, center: Point },
    /// Two-point drags: shapes, text frames, tables, zoom area, crop, knife, effects.
    Create { start: Point },
    Nodes { start: Point, orig: Vec<(ObjId, NodePath)>, moved: bool },
    Control { obj: ObjId, si: usize, ni: usize, incoming: bool, orig: NodePath, moved: bool },
    NodeMarquee { start: Point },
    Corner { obj: ObjId, start: Point, orig: [f64; 4], moved: bool },
    Pan { last: egui::Pos2 },
    Guide { index: Option<usize>, vertical: bool },
    Stroke { points: Vec<Point>, times: Vec<f64> },
    Brush { last: Point, started: bool },
    Effect { start: Point, obj: ObjId },
    Blend { from: ObjId, start: Point },
    Connector { start: Point, from: Option<(ObjId, Point)> },
    ConnectorEnd { obj: ObjId, is_start: bool },
    Envelope { obj: ObjId, handle: usize, start: Point, orig: [Vec2; 8], moved: bool },
    BezierHandle { node: Point },
    /// Table tool: dragging across the cells of a table.
    TableCells { obj: ObjId },
}

impl Drag {
    pub fn is_none(&self) -> bool {
        matches!(self, Drag::None)
    }
}

pub struct ToolState {
    pub previous_tool: Option<ToolId>,
    pub parallel_drawing: bool,
    pub parallel_count: u32,
    pub parallel_distance: f64,
    pub default_outline_width: f64,
    pub lock_ratio: bool,
    pub lock_corners: bool,
    pub crop_rect: Option<Rect>,
    pub common_shape: usize,
    pub elastic: bool,
    pub brush_size: f64,
    pub brush_strength: f64,
    pub auto_close: bool,
    pub media_width: f64,
    pub media_kind: usize,
    pub blend_steps: u32,
    pub distort_kind: DistortKind,
    pub text_format: CharFormat,
    pub text_align: TextAlign,
    /// Paragraph settings for the text you create next (Text menu dialogs used with no text selected).
    pub text_para: TextObj,
    pub recent_fonts: Vec<String>,
    pub last_coalesce: f64,
    pub texture_cache: HashMap<String, egui::TextureHandle>,
    pub shaping_op: usize,
    pub leave_source: bool,
    pub leave_target: bool,
    // ---- interaction state
    pub drag: Drag,
    /// Current (snapped) document point of the pointer during a drag.
    pub cur: Point,
    /// Nodes of a curve being drawn (Bézier, Pen, Polyline, B-Spline).
    pub curve: Vec<Node>,
    /// First click of a two-click line (Freehand / 2-point line).
    pub line_start: Option<Point>,
    /// Points collected by multi-click tools (dimensions, 3-point shapes).
    pub stage: Vec<Point>,
    pub pick_target: Option<PickTarget>,
    pub table_cell: Option<(usize, usize)>,
    /// The other corner of the selected block of cells (Table tool).
    pub table_cell_to: Option<(usize, usize)>,
    /// Object ▸ Create ▸ Pattern Fill waiting for its marquee: type and resolution.
    pub pattern_area: Option<(u8, u8)>,
    /// Fill for new objects (`None` = no fill, as in the original's defaults).
    pub new_fill: Option<Color>,
    pub eyedrop_color: Option<Color>,
    pub eyedrop_attrs: Option<(Style, vector_core::object::Effects)>,
    pub snap_mark: Option<(Point, &'static str)>,
    /// Alignment and dynamic guides shown while snapping to them.
    pub guide_lines: Vec<(Point, Point)>,
    pub renderer: vector_render::Renderer,
    /// Text typed at a point before the text object exists: (point, paragraph frame).
    pub pending_text: Option<(Point, Option<Rect>)>,
    pub text_last_edit: f64,
    pub hover_doc: Option<Point>,
    /// Screen position where the primary button went down, and whether it moved since.
    pub press: Option<egui::Pos2>,
    pub moved: bool,
    pub mods: egui::Modifiers,
    /// Object whose selection state the last press changed (to toggle rotate mode on a second click).
    pub was_selected: bool,
    /// The curve being drawn holds B-spline control points.
    pub bspline: bool,
    /// Right-click during a Pick drag leaves a copy.
    pub copy_on_drop: bool,
    /// A Text tool drag selects characters instead of drawing a frame.
    pub text_drag_select: bool,
}

impl Default for ToolState {
    fn default() -> Self {
        ToolState {
            previous_tool: None,
            parallel_drawing: false,
            parallel_count: 1,
            parallel_distance: 7.2,
            default_outline_width: 0.5669,
            lock_ratio: false,
            lock_corners: true,
            crop_rect: None,
            common_shape: 0,
            elastic: false,
            brush_size: 18.0,
            brush_strength: 50.0,
            auto_close: false,
            media_width: 7.2,
            media_kind: 0,
            blend_steps: 20,
            distort_kind: DistortKind::PushPull,
            text_format: CharFormat::default(),
            text_align: TextAlign::Left,
            text_para: TextObj::artistic("", CharFormat::default()),
            recent_fonts: vec![],
            last_coalesce: -10.0,
            texture_cache: HashMap::new(),
            shaping_op: 0,
            leave_source: false,
            leave_target: false,
            drag: Drag::None,
            cur: Point::ZERO,
            curve: vec![],
            line_start: None,
            stage: vec![],
            pick_target: None,
            table_cell: None,
            table_cell_to: None,
            pattern_area: None,
            new_fill: None,
            eyedrop_color: None,
            eyedrop_attrs: None,
            snap_mark: None,
            guide_lines: vec![],
            renderer: vector_render::Renderer::new(),
            pending_text: None,
            text_last_edit: -10.0,
            hover_doc: None,
            press: None,
            moved: false,
            mods: egui::Modifiers::NONE,
            was_selected: false,
            bspline: false,
            copy_on_drop: false,
            text_drag_select: false,
        }
    }
}

impl ToolState {
    /// The selected block of cells of table `t`.
    pub fn table_range(&self, t: &vector_core::object::TableObj) -> Option<vector_core::object::CellRange> {
        range_of(t, self.table_cell, self.table_cell_to)
    }
    /// Move the selected cells after rows or columns were inserted before them.
    pub fn shift_cells(&mut self, (dr, dc): (usize, usize)) {
        for c in [&mut self.table_cell, &mut self.table_cell_to].into_iter().flatten() {
            c.0 += dr;
            c.1 += dc;
        }
    }
    /// Status bar hint for a tool, adapted to what is in progress.
    pub fn hint(&self, t: ToolId) -> String {
        if let Some(p) = self.pick_target {
            return match p {
                PickTarget::PowerClip => "Click the frame object to place the selection inside it (Esc to cancel)",
                PickTarget::CopyProperties { .. } => "Click the object to copy properties from (Esc to cancel)",
                PickTarget::CopyEffect { .. } => "Click the object to copy the effect from (Esc to cancel)",
                PickTarget::Order { in_front: true } => "Click the object to place the selection in front of (Esc to cancel)",
                PickTarget::Order { in_front: false } => "Click the object to place the selection behind (Esc to cancel)",
                PickTarget::CopyPowerClip => "Click the PowerClip to copy the contents from (Esc to cancel)",
                PickTarget::FitToPath => "Click the path to fit the text to (Esc to cancel)",
            }
            .into();
        }
        match t {
            ToolId::Bezier | ToolId::Pen | ToolId::Polyline | ToolId::BSpline if !self.curve.is_empty() => "Click to add nodes, drag to set curve handles; double-click, Enter or Space to finish; click the first node to close".into(),
            ToolId::ThreePointCurve | ToolId::Rect3 | ToolId::Ellipse3 if self.stage.len() == 2 => "Move the pointer to set the shape, then click".into(),
            ToolId::ParallelDim | ToolId::HvDim | ToolId::SegmentDim if !self.stage.is_empty() => "Click to place the dimension line and text".into(),
            ToolId::AngularDim if !self.stage.is_empty() => "Click the second and third points, then place the text".into(),
            ToolId::Callout if !self.stage.is_empty() => "Click to place the elbow, then click to place the text".into(),
            ToolId::Crop if self.crop_rect.is_some() => "Double-click inside the crop area or press Enter to crop; Esc to cancel".into(),
            ToolId::ColorEyedropper if self.eyedrop_color.is_some() => "Click objects to apply the sampled color (right-click: outline); Shift+click to sample again".into(),
            ToolId::AttributesEyedropper if self.eyedrop_attrs.is_some() => "Click objects to apply the sampled attributes; Shift+click to sample again".into(),
            _ => info(t).hint.to_string(),
        }
    }

    pub fn is_busy(&self) -> bool {
        !self.drag.is_none() || !self.curve.is_empty() || self.crop_rect.is_some() || self.line_start.is_some() || !self.stage.is_empty() || self.pick_target.is_some() || self.pending_text.is_some() || self.pattern_area.is_some()
    }

    pub fn cancel(&mut self) {
        self.drag = Drag::None;
        self.curve.clear();
        self.crop_rect = None;
        self.line_start = None;
        self.stage.clear();
        self.pick_target = None;
        self.pending_text = None;
        self.pattern_area = None;
        self.snap_mark = None;
        self.guide_lines.clear();
        self.press = None;
        self.bspline = false;
        self.copy_on_drop = false;
    }

    /// Style for newly drawn objects.
    pub fn new_style(&self) -> Style {
        Style {
            fill: self.new_fill.map(Fill::Solid).unwrap_or(Fill::None),
            outline: Outline { width: self.default_outline_width, color: Some(Color::BLACK), ..Outline::default() },
            ..Default::default()
        }
    }
}

/// Finish whatever the active tool has in progress (curve being drawn, crop marquee...).
pub fn finish(ts: &mut ToolState, d: &mut DocView, _settings: &Settings) {
    // drop the duplicate node a double-click leaves at the end
    if ts.curve.len() >= 2 {
        let n = ts.curve.len();
        if (ts.curve[n - 1].p - ts.curve[n - 2].p).hypot() < d.doc_len(3.0) {
            ts.curve.pop();
        }
    }
    if ts.curve.len() >= 2 {
        let nodes = std::mem::take(&mut ts.curve);
        let np = if ts.bspline {
            let pts: Vec<Point> = nodes.iter().map(|n| n.p).collect();
            interact::bspline_path(&pts, false)
        } else {
            NodePath { subpaths: vec![SubPath { nodes, closed: false }] }
        };
        add_curve(ts, d, np, if ts.bspline { "Create B-Spline" } else { "Create Curve" });
    }
    ts.bspline = false;
    ts.curve.clear();
    if let Some(r) = ts.crop_rect.take() {
        let page = d.page;
        let ids = d.selection.clone();
        d.modify("Crop", |doc, sel| {
            ops_crop(doc, page, &ids, r);
            sel.clear();
        });
    }
    if let Some(te) = d.text_edit.take() {
        // remove empty text objects
        if let Some(o) = d.doc.find(te.obj) {
            if let ObjKind::Text(t) = &o.kind {
                if t.content.is_empty() {
                    d.doc.remove(te.obj);
                    d.selection.retain(|x| *x != te.obj);
                    d.history.touch();
                }
            }
        }
    }
    ts.cancel();
}

fn ops_crop(doc: &mut vector_core::Document, page: usize, ids: &[ObjId], r: Rect) {
    let ids: Vec<ObjId> = if ids.is_empty() { doc.pages[page].layers.iter().filter(|l| !l.locked).flat_map(|l| l.objects.iter().map(|o| o.id)).collect() } else { ids.to_vec() };
    vector_core::ops::crop(doc, page, &ids, r);
}

/// Add a drawn curve (plus parallel copies when Parallel Drawing is on).
pub fn add_curve(ts: &ToolState, d: &mut DocView, np: NodePath, label: &str) {
    if np.is_empty() || np.node_count() < 2 {
        return;
    }
    let mut objs = vec![];
    let id = d.doc.new_id();
    let mut style = ts.new_style();
    if !np.is_closed() {
        style.fill = Fill::None;
    }
    let base = Object::new(id, ObjKind::Path(np.clone()), Affine::IDENTITY, style.clone());
    if ts.parallel_drawing {
        let bp = np.to_bez();
        for k in 1..=ts.parallel_count.max(1) {
            for sign in [1.0, -1.0] {
                let off = vector_core::geom::offset_path(&bp, sign * ts.parallel_distance * k as f64, kurbo::Join::Round);
                if off.elements().is_empty() {
                    continue;
                }
                let pid = d.doc.new_id();
                objs.push(Object::new(pid, ObjKind::Path(NodePath::from_bez(&off)), Affine::IDENTITY, style.clone()));
            }
        }
    }
    objs.insert(0, base);
    d.add_objects(label, objs);
}

pub fn default_shadow(inner: bool) -> DropShadow {
    DropShadow { offset: Vec2::new(6.0, 6.0), feather: 8.0, color: Color::BLACK, opacity: 50.0, inner }
}

// ------------------------------------------------------------------------------------------
// Node editing (Shape tool)
// ------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum NodeOp {
    Add,
    Delete,
    Join,
    Break,
    ToLine,
    ToCurve,
    Cusp,
    Smooth,
    Symmetric,
    Reverse,
    ExtendClose,
    Close,
    Extract,
    Reduce,
    SelectAll,
    Elastic,
}

/// Selected nodes grouped by object: (object, [(subpath, node)]).
fn nodes_by_object(d: &DocView) -> Vec<(ObjId, Vec<(usize, usize)>)> {
    let mut out: Vec<(ObjId, Vec<(usize, usize)>)> = vec![];
    for (id, si, ni) in &d.node_sel {
        match out.iter_mut().find(|(o, _)| o == id) {
            Some(e) => e.1.push((*si, *ni)),
            None => out.push((*id, vec![(*si, *ni)])),
        }
    }
    out
}

/// The curve objects the Shape tool operates on (selected paths).
fn selected_paths(d: &DocView) -> Vec<ObjId> {
    d.selection.iter().copied().filter(|id| matches!(d.doc.find(*id).map(|o| &o.kind), Some(ObjKind::Path(_)))).collect()
}

pub fn node_op(app: &mut VectorApp, op: NodeOp) {
    if op == NodeOp::Elastic {
        app.ts.elastic = !app.ts.elastic;
        return;
    }
    let Some(d) = app.doc_mut() else { return };
    let groups = nodes_by_object(d);
    let paths = selected_paths(d);
    match op {
        NodeOp::SelectAll => {
            d.node_sel.clear();
            for id in &paths {
                if let Some(ObjKind::Path(np)) = d.doc.find(*id).map(|o| &o.kind) {
                    for (si, sp) in np.subpaths.iter().enumerate() {
                        for ni in 0..sp.nodes.len() {
                            d.node_sel.push((*id, si, ni));
                        }
                    }
                }
            }
        }
        NodeOp::Reverse | NodeOp::Close | NodeOp::Reduce => {
            if paths.is_empty() {
                return;
            }
            let label = match op {
                NodeOp::Reverse => "Reverse Direction",
                NodeOp::Close => "Close Curve",
                _ => "Reduce Nodes",
            };
            d.modify(label, |doc, _| {
                for id in &paths {
                    if let Some(o) = doc.find_mut(*id) {
                        if let ObjKind::Path(np) = &mut o.kind {
                            match op {
                                NodeOp::Reverse => np.reverse(),
                                NodeOp::Close => np.close_all(),
                                _ => np.reduce(0.5),
                            }
                        }
                    }
                }
            });
            d.node_sel.clear();
        }
        _ => {
            if groups.is_empty() {
                return;
            }
            let label = match op {
                NodeOp::Add => "Add Nodes",
                NodeOp::Delete => "Delete Nodes",
                NodeOp::Join => "Join Nodes",
                NodeOp::Break => "Break Curve",
                NodeOp::ToLine => "To Line",
                NodeOp::ToCurve => "To Curve",
                NodeOp::Cusp => "Cusp",
                NodeOp::Smooth => "Smooth",
                NodeOp::Symmetric => "Symmetrical",
                NodeOp::ExtendClose => "Extend Curve to Close",
                _ => "Extract Subpath",
            };
            let mut new_sel = vec![];
            d.modify(label, |doc, sel| {
                for (id, refs) in &groups {
                    let Some(o) = doc.find_mut(*id) else { continue };
                    let transform = o.transform;
                    let style = o.style.clone();
                    let ObjKind::Path(np) = &mut o.kind else { continue };
                    match op {
                        NodeOp::Add => {
                            // add a node in the middle of each segment that starts at a selected node
                            let mut rs = refs.clone();
                            rs.sort();
                            for (si, ni) in rs.into_iter().rev() {
                                let segs = np.subpaths.get(si).map(|s| s.segment_count()).unwrap_or(0);
                                if ni < segs {
                                    if let Some((a, b)) = np.insert_node(si, ni, 0.5) {
                                        new_sel.push((*id, a, b));
                                    }
                                }
                            }
                        }
                        NodeOp::Delete => np.delete_nodes(refs),
                        NodeOp::Join => {
                            if refs.len() == 2 {
                                np.join_nodes(refs[0], refs[1]);
                            }
                        }
                        NodeOp::ExtendClose => {
                            if refs.len() == 2 {
                                np.extend_to_close(refs[0], refs[1]);
                            }
                        }
                        NodeOp::Break => {
                            let mut rs = refs.clone();
                            rs.sort();
                            for r in rs.into_iter().rev() {
                                np.break_at(r);
                            }
                        }
                        NodeOp::ToLine | NodeOp::ToCurve => {
                            for r in refs {
                                np.set_segment_line(*r, op == NodeOp::ToLine);
                            }
                        }
                        NodeOp::Cusp | NodeOp::Smooth | NodeOp::Symmetric => {
                            let k = match op {
                                NodeOp::Cusp => NodeKind::Cusp,
                                NodeOp::Smooth => NodeKind::Smooth,
                                _ => NodeKind::Symmetric,
                            };
                            for r in refs {
                                np.set_node_kind(*r, k);
                            }
                        }
                        _ => {
                            // Extract subpath: move the subpaths of the selected nodes into new objects
                            let mut sis: Vec<usize> = refs.iter().map(|r| r.0).collect();
                            sis.sort();
                            sis.dedup();
                            if sis.len() >= np.subpaths.len() {
                                continue;
                            }
                            let mut extracted = vec![];
                            for si in sis.into_iter().rev() {
                                extracted.push(np.subpaths.remove(si));
                            }
                            let loc = doc.locate(*id);
                            for sp in extracted {
                                let nid = doc.new_id();
                                let no = Object::new(nid, ObjKind::Path(NodePath { subpaths: vec![sp] }), transform, style.clone());
                                if let Some((p, l, _)) = loc {
                                    doc.pages[p].layers[l].objects.push(no);
                                }
                                sel.push(nid);
                            }
                            continue;
                        }
                    }
                }
            });
            d.node_sel = new_sel;
        }
    }
    d.cache.key = 0;
}

/// Move the selected nodes by a document-space offset (arrow-key nudge with the Shape tool).
pub fn nudge_nodes(app: &mut VectorApp, delta: Vec2) {
    let Some(d) = app.doc_mut() else { return };
    let groups = nodes_by_object(d);
    d.modify("Move Nodes", |doc, _| {
        for (id, refs) in &groups {
            if let Some(o) = doc.find_mut(*id) {
                let inv = o.transform.inverse();
                let ld = inv * Point::new(delta.x, delta.y) - inv * Point::ZERO;
                if let ObjKind::Path(np) = &mut o.kind {
                    move_nodes(np, refs, ld);
                }
            }
        }
    });
}

/// Move nodes (and their control points) by a local offset.
pub fn move_nodes(np: &mut NodePath, refs: &[(usize, usize)], d: Vec2) {
    for r in refs {
        if let Some(n) = np.node_mut(*r) {
            n.p += d;
            n.c_in += d;
            n.c_out += d;
        }
    }
}

/// Round / scallop / chamfer the sharp corners of a curve.
/// `kind`: 0 = round (fillet), 1 = scallop, 2 = chamfer.
pub fn round_corners(np: &NodePath, r: f64, kind: usize) -> NodePath {
    if r <= 0.0 {
        return np.clone();
    }
    let mut out = NodePath::new();
    for sp in &np.subpaths {
        let n = sp.nodes.len();
        if n < 3 {
            out.subpaths.push(sp.clone());
            continue;
        }
        let mut nodes: Vec<Node> = vec![];
        for i in 0..n {
            let node = sp.nodes[i];
            let first_or_last = !sp.closed && (i == 0 || i == n - 1);
            let prev = sp.nodes[(i + n - 1) % n];
            let next = sp.nodes[(i + 1) % n];
            let line_in = prev.line_out || (prev.c_out == prev.p && node.c_in == node.p);
            let line_out = node.line_out || (node.c_out == node.p && next.c_in == next.p);
            if first_or_last || node.kind != NodeKind::Cusp || !line_in || !line_out {
                nodes.push(node);
                continue;
            }
            let din = prev.p - node.p;
            let dout = next.p - node.p;
            let (lin, lout) = (din.hypot(), dout.hypot());
            if lin < 1e-6 || lout < 1e-6 {
                nodes.push(node);
                continue;
            }
            let angle = (din.dot(dout) / (lin * lout)).clamp(-1.0, 1.0).acos();
            if angle > std::f64::consts::PI - 0.01 {
                nodes.push(node);
                continue;
            }
            // distance from the corner to the tangent points
            let t = (r / (angle / 2.0).tan()).min(lin * 0.5).min(lout * 0.5);
            let a = node.p + din * (t / lin);
            let b = node.p + dout * (t / lout);
            let mut na = Node::corner(a);
            let mut nb = Node::corner(b);
            match kind {
                2 => {
                    na.line_out = true;
                }
                1 => {
                    // scallop: concave arc bulging towards the corner's inside
                    let k = 0.55;
                    let mirror = Point::new(a.x + b.x - node.p.x, a.y + b.y - node.p.y);
                    na.c_out = a + (mirror - a) * k;
                    nb.c_in = b + (mirror - b) * k;
                    na.line_out = false;
                }
                _ => {
                    let k = 4.0 / 3.0 * ((std::f64::consts::PI - angle) / 4.0).tan();
                    let ka = (t * k).min(t);
                    na.c_out = a + (node.p - a) * (ka / t.max(1e-9));
                    nb.c_in = b + (node.p - b) * (ka / t.max(1e-9));
                    na.line_out = false;
                }
            }
            nb.line_out = node.line_out;
            nb.c_out = b;
            nodes.push(na);
            nodes.push(nb);
        }
        out.subpaths.push(SubPath { nodes, closed: sp.closed });
    }
    out
}

/// Insert text: into the text being edited, or as a new artistic text object at the page center.
pub fn insert_text(app: &mut VectorApp, s: &str, font: Option<&str>) {
    insert_text_at(app, s, font, None);
}

/// Insert text at the caret, or as new artistic text centered on `at` (the page center).
pub fn insert_text_at(app: &mut VectorApp, s: &str, font: Option<&str>, at: Option<Point>) {
    if !app.has_doc() {
        app.new_default_document();
    }
    let mut fmt = app.ts.text_format.clone();
    if let Some(f) = font {
        fmt.font = f.to_string();
    }
    let fill = app.ts.new_fill;
    let para = app.ts.text_para.clone();
    let Some(d) = app.doc_mut() else { return };
    if let Some(te) = d.text_edit.clone() {
        let caret = te.caret.min(te.anchor);
        let end = te.caret.max(te.anchor);
        let id = te.obj;
        d.modify("Insert Text", |doc, _| {
            if let Some(o) = doc.find_mut(id) {
                if let ObjKind::Text(t) = &mut o.kind {
                    if end > caret {
                        t.delete(caret, end);
                    }
                    t.insert(caret, s);
                    if font.is_some() {
                        let n = s.chars().count();
                        let f = fmt.font.clone();
                        t.apply_format(caret, caret + n, |cf| cf.font = f.clone());
                    }
                }
            }
        });
        let n = s.chars().count();
        if let Some(te) = d.text_edit.as_mut() {
            te.caret = caret + n;
            te.anchor = te.caret;
        }
        return;
    }
    let c = at.unwrap_or_else(|| d.page_ref().rect().center());
    let id = d.doc.new_id();
    let mut style = Style::text_default();
    if let Some(f) = fill {
        style.fill = Fill::Solid(f);
    }
    let mut t = TextObj::artistic(s, fmt);
    crate::dialogs::text::apply_defaults(&mut t, &para);
    let mut o = Object::new(id, ObjKind::Text(t), Affine::IDENTITY, style);
    let b = o.bounds();
    o.transform = Affine::translate(c - b.center());
    d.add_objects("Insert Text", vec![o]);
}

/// The block of cells from `a` to `b` (or the cell `a`), when they are in the table.
pub fn range_of(t: &vector_core::object::TableObj, a: Option<(usize, usize)>, b: Option<(usize, usize)>) -> Option<vector_core::object::CellRange> {
    let a = a.filter(|a| a.0 < t.rows && a.1 < t.cols)?;
    let b = b.filter(|b| b.0 < t.rows && b.1 < t.cols).unwrap_or(a);
    Some(t.range(a, b))
}
