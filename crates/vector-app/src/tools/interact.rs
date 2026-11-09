//! Pointer and keyboard interaction of every tool in the drawing window.
//!
//! The canvas builds an [`Input`] each frame; [`handle_input`] routes it to the active tool.
//! Previews of drags in progress are drawn by `overlay`, which uses the same geometry
//! helpers (`creation_object`, `xform_affine`...) so what you see is what gets created.

use super::*;
use crate::commands::Cmd;
use egui::Pos2;
use kurbo::{BezPath, CubicBez, ParamCurve, Shape};
use vector_core::object::*;
use vector_core::path::{fit_freehand, polyline};
use vector_core::style::{Gradient, Transparency, WidthProfile};
use vector_core::text::TextKind;
use vector_core::{geom, ops};

/// Pointer state for one frame, in screen coordinates.
#[derive(Clone, Debug)]
pub struct Input {
    pub pos: Pos2,
    /// The pointer is over the drawing area.
    pub hovered: bool,
    pub pressed: bool,
    pub released: bool,
    pub down: bool,
    pub double: bool,
    pub secondary: bool,
    pub mods: egui::Modifiers,
    pub time: f64,
    pub dt: f64,
}

#[derive(Default)]
struct Out {
    tool: Option<ToolId>,
    flash: Option<String>,
    cmds: Vec<Cmd>,
}

pub fn handle_input(app: &mut VectorApp, inp: &Input) {
    let Some(ai) = app.active else { return };
    if ai >= app.docs.len() {
        return;
    }
    let mut out = Out::default();
    {
        let tool = app.tool;
        let default_fill = app.default_fill;
        let ts = &mut app.ts;
        let s = &app.settings;
        let d = &mut app.docs[ai];
        ts.mods = inp.mods;
        let raw = d.to_doc(inp.pos);
        ts.hover_doc = inp.hovered.then_some(raw);
        if inp.pressed {
            ts.press = Some(inp.pos);
            ts.moved = false;
        }
        if let Some(p0) = ts.press {
            if (inp.pos - p0).length() > 3.0 {
                ts.moved = true;
            }
        }
        let mut cx = Cx { ts, d, s, tool, inp, out: &mut out, default_fill, raw };
        cx.run();
        if inp.released {
            cx.ts.press = None;
        }
    }
    if let Some(t) = out.tool {
        app.set_tool(t);
    }
    if let Some(m) = out.flash {
        app.flash(m);
    }
    for c in out.cmds {
        app.run(c);
    }
}

struct Cx<'a> {
    ts: &'a mut ToolState,
    d: &'a mut DocView,
    s: &'a Settings,
    tool: ToolId,
    inp: &'a Input,
    out: &'a mut Out,
    default_fill: Color,
    raw: Point,
}

/// Handle positions around a bounding box: 0 TL, 1 T, 2 TR, 3 R, 4 BR, 5 B, 6 BL, 7 L.
pub fn handle_point(bb: Rect, i: usize) -> Point {
    let (cx, cy) = ((bb.x0 + bb.x1) / 2.0, (bb.y0 + bb.y1) / 2.0);
    match i % 8 {
        0 => Point::new(bb.x0, bb.y0),
        1 => Point::new(cx, bb.y0),
        2 => Point::new(bb.x1, bb.y0),
        3 => Point::new(bb.x1, cy),
        4 => Point::new(bb.x1, bb.y1),
        5 => Point::new(cx, bb.y1),
        6 => Point::new(bb.x0, bb.y1),
        _ => Point::new(bb.x0, cy),
    }
}

/// The affine of an interactive transform in progress.
pub fn xform_affine(kind: Xform, start: Point, cur: Point, bb: Rect, center: Point, mods: egui::Modifiers, constrain_deg: f64) -> Affine {
    match kind {
        Xform::Move => {
            let mut dv = cur - start;
            if mods.ctrl {
                if dv.x.abs() > dv.y.abs() {
                    dv.y = 0.0;
                } else {
                    dv.x = 0.0;
                }
            }
            Affine::translate(dv)
        }
        Xform::Scale(h) => {
            let hp = handle_point(bb, h);
            let anchor = if mods.shift { bb.center() } else { handle_point(bb, (h + 4) % 8) };
            let d0 = hp - anchor;
            let d1 = (hp + (cur - start)) - anchor;
            let mut sx = 1.0;
            let mut sy = 1.0;
            if d0.x.abs() > 1e-9 && h != 1 && h != 5 {
                sx = d1.x / d0.x;
            }
            if d0.y.abs() > 1e-9 && h != 3 && h != 7 {
                sy = d1.y / d0.y;
            }
            if h % 2 == 0 {
                // corner handles keep the proportions
                let k = sx.abs().max(sy.abs());
                sx = k * if sx < 0.0 { -1.0 } else { 1.0 };
                sy = k * if sy < 0.0 { -1.0 } else { 1.0 };
            } else if mods.ctrl {
                // Ctrl on a side handle: scale in 100% steps (mirror)
                sx = sx.round();
                sy = sy.round();
                if sx == 0.0 {
                    sx = 1.0;
                }
                if sy == 0.0 {
                    sy = 1.0;
                }
            }
            let sx = if sx.abs() < 1e-4 { 1e-4 } else { sx };
            let sy = if sy.abs() < 1e-4 { 1e-4 } else { sy };
            Affine::translate(anchor.to_vec2()) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-anchor.to_vec2())
        }
        Xform::Rotate => {
            let a0 = (start - center).atan2();
            let a1 = (cur - center).atan2();
            let mut a = a1 - a0;
            if mods.ctrl {
                let step = constrain_deg.max(1.0).to_radians();
                a = (a / step).round() * step;
            }
            Affine::rotate_about(a, center)
        }
        Xform::Skew(h) => {
            let dv = cur - start;
            if h == 1 || h == 5 {
                let (moving, anchor) = if h == 1 { (bb.y0, bb.y1) } else { (bb.y1, bb.y0) };
                let den = moving - anchor;
                let k = if den.abs() > 1e-9 { dv.x / den } else { 0.0 };
                Affine::new([1.0, 0.0, k, 1.0, -k * anchor, 0.0])
            } else {
                let (moving, anchor) = if h == 3 { (bb.x1, bb.x0) } else { (bb.x0, bb.x1) };
                let den = moving - anchor;
                let k = if den.abs() > 1e-9 { dv.y / den } else { 0.0 };
                Affine::new([1.0, k, 0.0, 1.0, 0.0, -k * anchor])
            }
        }
    }
}

/// Rectangle from a drag with Ctrl (constrain) and Shift (from center).
pub fn drag_rect(a: Point, b: Point, mods: egui::Modifiers) -> Rect {
    let mut dx = b.x - a.x;
    let mut dy = b.y - a.y;
    if mods.ctrl {
        let m = dx.abs().max(dy.abs());
        dx = m * if dx < 0.0 { -1.0 } else { 1.0 };
        dy = m * if dy < 0.0 { -1.0 } else { 1.0 };
    }
    if mods.shift {
        Rect::new(a.x - dx, a.y - dy, a.x + dx, a.y + dy).abs()
    } else {
        Rect::new(a.x, a.y, a.x + dx, a.y + dy).abs()
    }
}

/// The object a two-point drag would create (also used for the live preview).
pub fn creation_object(ts: &ToolState, s: &Settings, tool: ToolId, a: Point, b: Point, mods: egui::Modifiers, id: ObjId) -> Option<Object> {
    let r = drag_rect(a, b, mods);
    if r.width() < 0.01 && r.height() < 0.01 {
        return None;
    }
    let (w, h) = (r.width().max(0.01), r.height().max(0.01));
    let tr = Affine::translate((r.x0, r.y0));
    let style = ts.new_style();
    let kind = match tool {
        ToolId::Rectangle => ObjKind::Rect(RectShape {
            w,
            h,
            radii: s.rect_radii,
            corner: match s.rect_corner {
                1 => CornerType::Scallop,
                2 => CornerType::Chamfer,
                _ => CornerType::Round,
            },
        }),
        ToolId::Ellipse => ObjKind::Ellipse(EllipseShape {
            w,
            h,
            kind: match s.ellipse_kind {
                1 => EllipseKind::Pie,
                2 => EllipseKind::Arc,
                _ => EllipseKind::Ellipse,
            },
            start: s.ellipse_start,
            end: s.ellipse_end,
            clockwise: s.ellipse_clockwise,
        }),
        ToolId::Polygon => ObjKind::Polygon(PolygonShape { w, h, sides: s.polygon_sides.max(3), kind: PolygonKind::Polygon, sharpness: 0.0 }),
        ToolId::Star => ObjKind::Polygon(PolygonShape { w, h, sides: s.star_points.max(3), kind: PolygonKind::Star, sharpness: s.star_sharpness }),
        ToolId::Spiral => ObjKind::Spiral(SpiralShape { w, h, revolutions: s.spiral_revs.max(1) as f64, logarithmic: s.spiral_log, expansion: s.spiral_expansion }),
        ToolId::CommonShapes => ObjKind::Path(shapes::common_shape(ts.common_shape, w, h)),
        ToolId::Table => ObjKind::Table(TableObj::new(s.table_rows.max(1) as usize, s.table_cols.max(1) as usize, w, h)),
        ToolId::GraphPaper => {
            let cols = s.graph_cols.max(1) as usize;
            let rows = s.graph_rows.max(1) as usize;
            let (cw, ch) = (w / cols as f64, h / rows as f64);
            let mut kids = vec![];
            let mut k = 1;
            for rr in 0..rows {
                for cc in 0..cols {
                    let cell = Object::new(
                        id + k,
                        ObjKind::Rect(RectShape { w: cw, h: ch, radii: [0.0; 4], corner: CornerType::Round }),
                        Affine::translate((r.x0 + cc as f64 * cw, r.y0 + rr as f64 * ch)),
                        style.clone(),
                    );
                    k += 1;
                    kids.push(cell);
                }
            }
            return Some(Object::new(id, ObjKind::Group(kids), Affine::IDENTITY, Style::default()));
        }
        ToolId::TwoPointLine => {
            let (a, b) = constrain_line(a, b, mods, s.constrain_angle);
            let mut st = style;
            st.fill = Fill::None;
            return Some(Object::new(id, ObjKind::Path(polyline(&[a, b], false)), Affine::IDENTITY, st));
        }
        _ => return None,
    };
    let mut o = Object::new(id, kind, tr, style);
    if tool == ToolId::Table {
        o.style.fill = Fill::None;
    }
    Some(o)
}

/// Constrain a line to multiples of the constrain angle (Ctrl).
pub fn constrain_line(a: Point, b: Point, mods: egui::Modifiers, constrain_deg: f64) -> (Point, Point) {
    if !mods.ctrl {
        return (a, b);
    }
    let v = b - a;
    let len = v.hypot();
    let step = constrain_deg.max(1.0).to_radians();
    let ang = (v.atan2() / step).round() * step;
    (a, a + Vec2::new(ang.cos(), ang.sin()) * len)
}

/// Object for the three-point tools (rectangle, ellipse, curve) once the third point is known.
pub fn three_point_object(ts: &ToolState, tool: ToolId, a: Point, b: Point, c: Point, id: ObjId) -> Option<Object> {
    let v = b - a;
    let len = v.hypot();
    if len < 1e-6 {
        return None;
    }
    let ang = v.atan2();
    let dir = v / len;
    let normal = Vec2::new(-dir.y, dir.x);
    let hgt = (c - a).dot(normal);
    let style = ts.new_style();
    match tool {
        ToolId::Rect3 => {
            if hgt.abs() < 1e-6 {
                return None;
            }
            let tr = Affine::translate(a.to_vec2()) * Affine::rotate(ang) * Affine::translate((0.0, hgt.min(0.0)));
            Some(Object::new(id, ObjKind::Rect(RectShape { w: len, h: hgt.abs(), radii: [0.0; 4], corner: CornerType::Round }), tr, style))
        }
        ToolId::Ellipse3 => {
            if hgt.abs() < 1e-6 {
                return None;
            }
            let h = hgt.abs() * 2.0;
            let tr = Affine::translate(a.to_vec2()) * Affine::rotate(ang) * Affine::translate((0.0, -h / 2.0));
            Some(Object::new(id, ObjKind::Ellipse(EllipseShape { w: len, h, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false }), tr, style))
        }
        ToolId::ThreePointCurve => {
            // quadratic through c at t = 0.5, raised to a cubic
            let q = Point::new(2.0 * c.x - 0.5 * (a.x + b.x), 2.0 * c.y - 0.5 * (a.y + b.y));
            let c1 = a + (q - a) * (2.0 / 3.0);
            let c2 = b + (q - b) * (2.0 / 3.0);
            let mut na = Node::corner(a);
            na.c_out = c1;
            na.line_out = false;
            let mut nb = Node::corner(b);
            nb.c_in = c2;
            let np = NodePath { subpaths: vec![SubPath { nodes: vec![na, nb], closed: false }] };
            let mut st = style;
            st.fill = Fill::None;
            Some(Object::new(id, ObjKind::Path(np), Affine::IDENTITY, st))
        }
        _ => None,
    }
}

/// Uniform cubic B-spline through control points (clamped at both ends), as Bézier nodes.
pub fn bspline_path(ctrl: &[Point], closed: bool) -> NodePath {
    if ctrl.len() < 3 {
        return polyline(ctrl, closed);
    }
    let mut p: Vec<Point> = vec![];
    if closed {
        p.extend_from_slice(ctrl);
        p.extend_from_slice(&ctrl[..3]);
    } else {
        p.push(ctrl[0]);
        p.push(ctrl[0]);
        p.extend_from_slice(ctrl);
        p.push(*ctrl.last().unwrap());
        p.push(*ctrl.last().unwrap());
    }
    let mut bp = BezPath::new();
    for i in 0..p.len() - 3 {
        let (p0, p1, p2, p3) = (p[i].to_vec2(), p[i + 1].to_vec2(), p[i + 2].to_vec2(), p[i + 3].to_vec2());
        let b0 = (p0 + p1 * 4.0 + p2) / 6.0;
        let b1 = (p1 * 2.0 + p2) / 3.0;
        let b2 = (p1 + p2 * 2.0) / 3.0;
        let b3 = (p1 + p2 * 4.0 + p3) / 6.0;
        if i == 0 {
            bp.move_to(b0.to_point());
        }
        bp.curve_to(b1.to_point(), b2.to_point(), b3.to_point());
    }
    if closed {
        bp.close_path();
    }
    NodePath::from_bez(&bp)
}

/// Normalized fill-space coordinates of a document point for an object (gradients, transparency).
pub fn fill_uv(o: &Object, p: Point) -> Point {
    let space = vector_render::Renderer::fill_space(o, &o.world_path());
    space.inverse() * p
}

/// The bounding box envelope / distort handles refer to (geometry before warps).
pub fn warp_bbox(o: &Object) -> Rect {
    (o.transform * o.local_path()).bounding_box()
}

pub const ENVELOPE_BASE: [(f64, f64); 8] = [(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (1.0, 0.5), (1.0, 1.0), (0.5, 1.0), (0.0, 1.0), (0.0, 0.5)];

pub fn envelope_handle(bb: Rect, env: &EnvelopeEffect, i: usize) -> Point {
    let (u, v) = ENVELOPE_BASE[i];
    let h = env.handles[i];
    Point::new(bb.x0 + (u + h.x) * bb.width(), bb.y0 + (v + h.y) * bb.height())
}

impl<'a> Cx<'a> {
    fn tol(&self) -> f64 {
        self.d.doc_len(5.0)
    }

    fn screen_dist(&self, a: Point, b: Point) -> f32 {
        (self.d.to_screen(a) - self.d.to_screen(b)).length()
    }

    fn hit(&self, p: Point) -> Option<ObjId> {
        let filled = self.s.treat_as_filled || self.d.doc.settings.treat_as_filled;
        let id = self.d.doc.hit_test(self.d.page, p, self.tol(), filled)?;
        // in focus mode only the objects in focus can be picked
        (self.d.focus.is_empty() || self.d.focus.contains(&id)).then_some(id)
    }

    fn snap(&mut self, p: Point, exclude: &[ObjId]) -> Point {
        let (q, label, lines) = snap_point_ex(self.d, self.s, p, exclude);
        self.ts.snap_mark = label.map(|l| (q, l));
        self.ts.guide_lines = lines;
        q
    }

    fn mods(&self) -> egui::Modifiers {
        self.inp.mods
    }

    fn run(&mut self) {
        let p = self.raw;
        if !self.ts.drag.is_none() || self.ts.press.is_none() {
            // keep the pointer position current for previews of multi-click tools
            if self.ts.drag.is_none() {
                let excl = self.d.selection.clone();
                let use_snap = !matches!(self.tool, ToolId::Pick | ToolId::Shape | ToolId::Zoom | ToolId::Pan);
                self.ts.cur = if use_snap && self.inp.hovered { self.snap(p, &excl) } else { p };
                if !use_snap {
                    self.ts.snap_mark = None;
                    self.ts.guide_lines.clear();
                }
            }
        }
        if self.inp.secondary {
            self.secondary();
        }
        if self.ts.pick_target.is_some() {
            if self.inp.pressed {
                self.pick_target_click();
            }
            return;
        }
        if self.inp.pressed {
            self.press();
        }
        if self.inp.down && self.ts.press.is_some() && !self.inp.pressed {
            self.drag();
        }
        if self.inp.released && self.ts.press.is_some() {
            self.release();
        }
        if self.inp.double {
            self.double_click();
        }
    }

    // ------------------------------------------------------------------------------ press

    fn press(&mut self) {
        let p = self.raw;
        use ToolId::*;
        match self.tool {
            Pick | FreeTransform => self.pick_press(),
            FreehandPick => self.ts.drag = Drag::Lasso { points: vec![p] },
            Shape => self.shape_press(),
            Smooth | Smear | Twirl | AttractRepel | Smudge | Roughen => {
                if self.d.selection.is_empty() {
                    if let Some(id) = self.hit(p) {
                        self.d.selection = vec![id];
                    }
                }
                self.ts.drag = Drag::Brush { last: p, started: false };
            }
            Crop => {
                let s = self.snap(p, &[]);
                self.ts.drag = Drag::Create { start: s };
            }
            Knife | Zoom | Rectangle | Ellipse | Polygon | Star | Spiral | GraphPaper | CommonShapes | Table | TwoPointLine => {
                let s = if self.tool == Zoom || self.tool == Knife { p } else { self.snap(p, &[]) };
                self.ts.drag = Drag::Create { start: s };
                self.ts.cur = s;
            }
            Pan => self.ts.drag = Drag::Pan { last: self.inp.pos },
            VirtualDelete => self.virtual_delete(p),
            Eraser | Freehand | LiveSketch | SmartDrawing | ArtisticMedia | PainterlyBrush => {
                let s = if self.tool == Freehand { self.snap(p, &[]) } else { p };
                self.ts.drag = Drag::Stroke { points: vec![s], times: vec![self.inp.time] };
            }
            Bezier | Pen => self.bezier_press(),
            BSpline => {
                let s = self.snap(p, &[]);
                if self.ts.curve.len() >= 3 && self.screen_dist(self.ts.curve[0].p, s) <= 6.0 {
                    let pts: Vec<Point> = self.ts.curve.iter().map(|n| n.p).collect();
                    self.ts.curve.clear();
                    let np = bspline_path(&pts, true);
                    add_curve(self.ts, self.d, np, "Create B-Spline");
                    return;
                }
                self.ts.curve.push(Node::corner(s));
                self.ts.bspline = true;
            }
            Polyline => {
                let s = self.snap(p, &[]);
                if self.ts.curve.len() >= 2 && self.screen_dist(self.ts.curve[0].p, s) <= 6.0 {
                    let mut nodes = std::mem::take(&mut self.ts.curve);
                    if let Some(l) = nodes.last_mut() {
                        l.line_out = true;
                    }
                    let np = NodePath { subpaths: vec![SubPath { nodes, closed: true }] };
                    add_curve(self.ts, self.d, np, "Create Polyline");
                    return;
                }
                self.ts.curve.push(Node::corner(s));
                self.ts.drag = Drag::Stroke { points: vec![s], times: vec![self.inp.time] };
            }
            ThreePointCurve | Rect3 | Ellipse3 => {
                let s = self.snap(p, &[]);
                if self.ts.stage.len() == 2 {
                    let (a, b) = (self.ts.stage[0], self.ts.stage[1]);
                    let id = self.d.doc.new_id();
                    if let Some(o) = three_point_object(self.ts, self.tool, a, b, s, id) {
                        self.d.add_objects(&format!("Create {}", self.tool.name()), vec![o]);
                    }
                    self.ts.stage.clear();
                } else {
                    self.ts.drag = Drag::Create { start: s };
                }
            }
            Text => self.text_press(),
            ParallelDim | HvDim | Callout => {
                let s = self.snap(p, &[]);
                if self.ts.stage.len() == 2 {
                    let mut pts = self.ts.stage.clone();
                    pts.push(s);
                    self.add_dimension(pts);
                    self.ts.stage.clear();
                } else {
                    self.ts.drag = Drag::Create { start: s };
                }
            }
            AngularDim => {
                let s = self.snap(p, &[]);
                self.ts.stage.push(s);
                if self.ts.stage.len() == 4 {
                    let pts = std::mem::take(&mut self.ts.stage);
                    self.add_dimension(pts);
                }
            }
            SegmentDim => {
                let s = self.snap(p, &[]);
                if self.ts.stage.len() == 2 {
                    let mut pts = self.ts.stage.clone();
                    pts.push(s);
                    self.add_dimension(pts);
                    self.ts.stage.clear();
                } else if let Some((a, b)) = self.segment_at(p) {
                    self.ts.stage = vec![a, b];
                } else {
                    self.out.flash = Some("Click a segment of a curve to measure it".into());
                }
            }
            StraightConnector | RightAngleConnector | RoundedConnector => {
                let from = self.attach_at(p);
                let start = from.map(|f| f.2).unwrap_or_else(|| self.snap(p, &[]));
                self.ts.drag = Drag::Connector { start, from: from.map(|f| (f.0, f.1)) };
                self.ts.cur = start;
            }
            EditAnchor => self.edit_anchor_press(),
            DropShadow | InnerShadow | BlockShadow | Contour | Distort | Extrude | Transparency | InteractiveFill => {
                let target = self.hit(p).or_else(|| self.d.selection.first().copied().filter(|id| self.d.doc.find(*id).map(|o| o.bounds().inflate(self.tol(), self.tol()).contains(p)).unwrap_or(false)));
                match target {
                    Some(id) => {
                        self.d.selection = vec![id];
                        self.ts.drag = Drag::Effect { start: p, obj: id };
                        self.ts.cur = p;
                    }
                    None => {
                        self.d.selection.clear();
                    }
                }
            }
            Blend => {
                if let Some(id) = self.hit(p) {
                    self.ts.drag = Drag::Blend { from: id, start: p };
                    self.ts.cur = p;
                }
            }
            Envelope => self.envelope_press(),
            ColorEyedropper => self.color_eyedropper(false),
            AttributesEyedropper => self.attributes_eyedropper(),
            MeshFill => self.mesh_fill_click(),
            SmartFill => self.smart_fill(),
        }
    }

    fn pick_press(&mut self) {
        let p = self.raw;
        let pos = self.inp.pos;
        // guides can be dragged with the Pick tool
        if let Some((idx, vertical)) = guide_at(self.d, pos) {
            if self.hit(p).is_none() {
                self.ts.drag = Drag::Guide { index: Some(idx), vertical };
                return;
            }
        }
        if let Some(bb) = self.d.selection_bounds() {
            let e = self.d.doc_len(6.0);
            let eb = bb.inflate(e, e);
            for i in 0..8 {
                let hp = self.d.to_screen(handle_point(eb, i));
                if (hp - pos).length() <= 6.0 {
                    let kind = if self.d.rotate_mode || self.tool == ToolId::FreeTransform {
                        if i % 2 == 0 {
                            Xform::Rotate
                        } else {
                            Xform::Skew(i)
                        }
                    } else {
                        Xform::Scale(i)
                    };
                    self.ts.drag = Drag::Transform { kind, start: p, bb, ids: self.d.selection.clone(), center: bb.center() };
                    self.ts.cur = p;
                    self.ts.copy_on_drop = false;
                    return;
                }
            }
        }
        let hit = self.hit(p);
        let shift = self.mods().shift;
        match hit {
            Some(id) => {
                self.ts.was_selected = self.d.selection.contains(&id);
                if shift {
                    if self.ts.was_selected {
                        self.d.selection.retain(|x| *x != id);
                    } else {
                        self.d.selection.push(id);
                    }
                } else if !self.ts.was_selected {
                    self.d.selection = vec![id];
                    self.d.rotate_mode = false;
                }
                self.d.node_sel.clear();
                if self.d.selection.contains(&id) {
                    if let Some(bb) = self.d.selection_bounds() {
                        let kind = if self.tool == ToolId::FreeTransform { Xform::Rotate } else { Xform::Move };
                        self.ts.drag = Drag::Transform { kind, start: p, bb, ids: self.d.selection.clone(), center: bb.center() };
                        self.ts.cur = p;
                        self.ts.copy_on_drop = false;
                    }
                }
            }
            None => {
                self.ts.drag = Drag::Marquee { start: p };
                self.ts.cur = p;
            }
        }
    }

    fn shape_press(&mut self) {
        let p = self.raw;
        let tol = self.tol();
        let shift = self.mods().shift;
        // 1. control handles of the selected nodes
        for &(id, si, ni) in &self.d.node_sel.clone() {
            let Some(o) = self.d.doc.find(id) else { continue };
            let ObjKind::Path(np) = &o.kind else { continue };
            let Some(n) = np.node((si, ni)) else { continue };
            for (cp, incoming) in [(n.c_in, true), (n.c_out, false)] {
                if cp != n.p && ((o.transform * cp) - p).hypot() <= tol {
                    self.ts.drag = Drag::Control { obj: id, si, ni, incoming, orig: np.clone(), moved: false };
                    return;
                }
            }
        }
        // 2. nodes of selected curves
        for id in self.d.selection.clone() {
            let Some(o) = self.d.doc.find(id) else { continue };
            match &o.kind {
                ObjKind::Path(np) => {
                    let t = o.transform;
                    let mut found = None;
                    for (si, sp) in np.subpaths.iter().enumerate() {
                        for (ni, n) in sp.nodes.iter().enumerate() {
                            if ((t * n.p) - p).hypot() <= tol {
                                found = Some((si, ni));
                            }
                        }
                    }
                    if let Some((si, ni)) = found {
                        let r = (id, si, ni);
                        let selected = self.d.node_sel.contains(&r);
                        if shift {
                            if selected {
                                self.d.node_sel.retain(|x| *x != r);
                            } else {
                                self.d.node_sel.push(r);
                            }
                        } else if !selected {
                            self.d.node_sel = vec![r];
                        }
                        let mut orig: Vec<(ObjId, NodePath)> = vec![];
                        for (oid, _, _) in &self.d.node_sel {
                            if orig.iter().any(|(x, _)| x == oid) {
                                continue;
                            }
                            if let Some(ObjKind::Path(np)) = self.d.doc.find(*oid).map(|o| &o.kind) {
                                orig.push((*oid, np.clone()));
                            }
                        }
                        let start = t * np.subpaths[si].nodes[ni].p;
                        self.ts.drag = Drag::Nodes { start, orig, moved: false };
                        return;
                    }
                }
                ObjKind::Rect(r) => {
                    let t = o.transform;
                    let corners = [Point::new(0.0, 0.0), Point::new(r.w, 0.0), Point::new(r.w, r.h), Point::new(0.0, r.h)];
                    for c in corners {
                        if ((t * c) - p).hypot() <= tol {
                            self.ts.drag = Drag::Corner { obj: id, start: p, orig: r.radii, moved: false };
                            return;
                        }
                    }
                }
                _ => {}
            }
        }
        // 3. objects
        match self.hit(p) {
            Some(id) => {
                if !self.d.selection.contains(&id) {
                    self.d.selection = vec![id];
                    self.d.node_sel.clear();
                } else if !shift {
                    self.d.node_sel.clear();
                }
            }
            None => {
                if !shift {
                    self.d.node_sel.clear();
                }
                self.ts.drag = Drag::NodeMarquee { start: p };
                self.ts.cur = p;
            }
        }
    }

    fn bezier_press(&mut self) {
        let s = self.snap(self.raw, &[]);
        if self.ts.curve.len() >= 2 && self.screen_dist(self.ts.curve[0].p, s) <= 6.0 {
            let first = self.ts.curve[0].p;
            let mut nodes = std::mem::take(&mut self.ts.curve);
            let _ = first;
            let first_plain = nodes.first().map(|f| f.c_in == f.p).unwrap_or(true);
            if let Some(l) = nodes.last_mut() {
                if l.c_out == l.p {
                    l.line_out = first_plain;
                } else {
                    l.line_out = false;
                }
            }
            let np = NodePath { subpaths: vec![SubPath { nodes, closed: true }] };
            add_curve(self.ts, self.d, np, "Create Curve");
            return;
        }
        if let Some(l) = self.ts.curve.last() {
            if self.screen_dist(l.p, s) < 2.0 {
                return; // second press of a double-click
            }
        }
        self.ts.curve.push(Node::corner(s));
        self.ts.bspline = false;
        self.ts.drag = Drag::BezierHandle { node: s };
    }

    fn text_press(&mut self) {
        let p = self.raw;
        // click into existing text
        if let Some(id) = self.hit(p) {
            if let Some(o) = self.d.doc.find(id) {
                if let ObjKind::Text(t) = &o.kind {
                    let local = o.transform.inverse() * p;
                    let caret = vector_core::text::layout(t).hit(local);
                    self.d.selection = vec![id];
                    self.d.text_edit = Some(crate::docview::TextEdit { obj: id, caret, anchor: caret, blink: self.inp.time });
                    self.ts.pending_text = None;
                    self.ts.drag = Drag::Create { start: p };
                    self.ts.text_drag_select = true;
                    return;
                }
                if let ObjKind::Table(tb) = &o.kind {
                    let local = o.transform.inverse() * p;
                    self.ts.table_cell = tb.cell_at(local);
                    self.d.selection = vec![id];
                    return;
                }
            }
        }
        // leave current editing
        if self.d.text_edit.is_some() {
            super::finish(self.ts, self.d, self.s);
        }
        self.ts.text_drag_select = false;
        let s = self.snap(p, &[]);
        self.ts.drag = Drag::Create { start: s };
        self.ts.cur = s;
    }

    fn edit_anchor_press(&mut self) {
        let p = self.raw;
        let tol = self.tol();
        for id in self.d.page_ref().all_objects().map(|o| o.id).collect::<Vec<_>>() {
            if let Some(ObjKind::Connector(c)) = self.d.doc.find(id).map(|o| &o.kind) {
                if (c.start.point - p).hypot() <= tol {
                    self.d.selection = vec![id];
                    self.ts.drag = Drag::ConnectorEnd { obj: id, is_start: true };
                    return;
                }
                if (c.end.point - p).hypot() <= tol {
                    self.d.selection = vec![id];
                    self.ts.drag = Drag::ConnectorEnd { obj: id, is_start: false };
                    return;
                }
            }
        }
        if let Some(id) = self.hit(p) {
            self.d.selection = vec![id];
        }
    }

    fn envelope_press(&mut self) {
        let p = self.raw;
        let tol = self.tol();
        if let Some(&id) = self.d.selection.first() {
            if let Some(o) = self.d.doc.find(id) {
                if let Some(env) = &o.effects.envelope {
                    let bb = warp_bbox(o);
                    for i in 0..8 {
                        if (envelope_handle(bb, env, i) - p).hypot() <= tol {
                            self.ts.drag = Drag::Envelope { obj: id, handle: i, start: p, orig: env.handles, moved: false };
                            return;
                        }
                    }
                }
            }
        }
        match self.hit(p) {
            Some(id) => {
                self.d.selection = vec![id];
                let has = self.d.doc.find(id).map(|o| o.effects.envelope.is_some()).unwrap_or(true);
                let container = self.d.doc.find(id).map(|o| o.is_container()).unwrap_or(true);
                if !has && !container {
                    self.d.modify("Envelope", |doc, _| {
                        if let Some(o) = doc.find_mut(id) {
                            o.effects.envelope = Some(EnvelopeEffect { handles: [Vec2::ZERO; 8] });
                        }
                    });
                }
            }
            None => self.d.selection.clear(),
        }
    }

    // ------------------------------------------------------------------------------ drag

    fn drag(&mut self) {
        let p = self.raw;
        let moved = self.ts.moved;
        let mut drag = std::mem::take(&mut self.ts.drag);
        match &mut drag {
            Drag::None => {}
            Drag::Marquee { .. } | Drag::NodeMarquee { .. } | Drag::Effect { .. } | Drag::Blend { .. } => self.ts.cur = p,
            Drag::Create { .. } => {
                self.ts.cur = if matches!(self.tool, ToolId::Zoom | ToolId::Knife) { p } else { self.snap(p, &[]) };
                if self.ts.text_drag_select && moved {
                    // extend the text selection
                    if let Some(te) = self.d.text_edit.clone() {
                        if let Some(o) = self.d.doc.find(te.obj) {
                            if let ObjKind::Text(t) = &o.kind {
                                let local = o.transform.inverse() * p;
                                let c = vector_core::text::layout(t).hit(local);
                                if let Some(te) = self.d.text_edit.as_mut() {
                                    te.caret = c;
                                }
                            }
                        }
                    }
                }
            }
            Drag::Lasso { points } => {
                if points.last().map(|l| self.screen_dist(*l, p) > 2.0).unwrap_or(true) {
                    points.push(p);
                }
            }
            Drag::Transform { ids, .. } => {
                let ex = ids.clone();
                self.ts.cur = self.snap(p, &ex);
            }
            Drag::Nodes { start, orig, moved: started } => {
                let cur = self.snap(p, &[]);
                self.ts.cur = cur;
                if !*started && moved {
                    self.d.checkpoint("Move Nodes");
                    *started = true;
                }
                if *started {
                    let dv = cur - *start;
                    let elastic = self.ts.elastic;
                    for (id, np0) in orig.iter() {
                        let refs: Vec<(usize, usize)> = self.d.node_sel.iter().filter(|(o, _, _)| o == id).map(|(_, s, n)| (*s, *n)).collect();
                        if let Some(o) = self.d.doc.find_mut(*id) {
                            let inv = o.transform.inverse();
                            let ld = inv * Point::new(dv.x, dv.y) - inv * Point::ZERO;
                            let mut np = np0.clone();
                            if elastic {
                                elastic_move(&mut np, &refs, ld);
                            } else {
                                move_nodes(&mut np, &refs, ld);
                            }
                            o.kind = ObjKind::Path(np);
                        }
                    }
                    self.d.history.touch();
                }
            }
            Drag::Control { obj, si, ni, incoming, orig, moved: started } => {
                if !*started && moved {
                    self.d.checkpoint("Edit Curve");
                    *started = true;
                }
                if *started {
                    if let Some(o) = self.d.doc.find_mut(*obj) {
                        let lc = o.transform.inverse() * p;
                        let mut np = orig.clone();
                        let closed = np.subpaths.get(*si).map(|s| s.closed).unwrap_or(false);
                        let count = np.subpaths.get(*si).map(|s| s.nodes.len()).unwrap_or(0);
                        if let Some(n) = np.node_mut((*si, *ni)) {
                            if *incoming {
                                n.c_in = lc;
                            } else {
                                n.c_out = lc;
                                n.line_out = false;
                            }
                            let (this, other) = if *incoming { (n.c_in, n.c_out) } else { (n.c_out, n.c_in) };
                            let v = this - n.p;
                            let new_other = match n.kind {
                                NodeKind::Symmetric => Some(n.p - v),
                                NodeKind::Smooth => {
                                    let l = (other - n.p).hypot().max(v.hypot() * 0.3);
                                    let len = v.hypot();
                                    (len > 1e-9).then(|| n.p - v * (l / len))
                                }
                                NodeKind::Cusp => None,
                            };
                            if let Some(no) = new_other {
                                if *incoming {
                                    n.c_out = no;
                                } else {
                                    n.c_in = no;
                                }
                            }
                        }
                        if *incoming {
                            let prev = if *ni > 0 { Some(*ni - 1) } else if closed { Some(count - 1) } else { None };
                            if let Some(pi) = prev {
                                if let Some(pn) = np.node_mut((*si, pi)) {
                                    pn.line_out = false;
                                }
                            }
                        }
                        o.kind = ObjKind::Path(np);
                    }
                    self.d.history.touch();
                }
            }
            Drag::Corner { obj, start, orig, moved: started } => {
                if !*started && moved {
                    self.d.checkpoint("Round Corners");
                    *started = true;
                }
                if *started {
                    let lock = self.ts.lock_corners;
                    if let Some(o) = self.d.doc.find_mut(*obj) {
                        let inv = o.transform.inverse();
                        let (ls, lc) = (inv * *start, inv * p);
                        if let ObjKind::Rect(r) = &mut o.kind {
                            let corners = [Point::new(0.0, 0.0), Point::new(r.w, 0.0), Point::new(r.w, r.h), Point::new(0.0, r.h)];
                            let k = (0..4).min_by(|a, b| (corners[*a] - ls).hypot().total_cmp(&(corners[*b] - ls).hypot())).unwrap_or(0);
                            let center = Point::new(r.w / 2.0, r.h / 2.0);
                            let diag = (center - corners[k]).normalize();
                            let delta = (lc - ls).dot(diag) * std::f64::consts::FRAC_1_SQRT_2;
                            let maxr = r.w.abs().min(r.h.abs()) / 2.0;
                            let nr = (orig[k] + delta).clamp(0.0, maxr);
                            if lock {
                                r.radii = [nr; 4];
                            } else {
                                r.radii = *orig;
                                r.radii[k] = nr;
                            }
                        }
                    }
                    self.d.history.touch();
                }
            }
            Drag::Pan { last } => {
                let dv = self.inp.pos - *last;
                self.d.origin += dv;
                *last = self.inp.pos;
            }
            Drag::Guide { .. } => self.ts.cur = p,
            Drag::Stroke { points, times } => {
                let q = if self.tool == ToolId::Freehand || self.tool == ToolId::Polyline { self.snap(p, &[]) } else { p };
                if points.last().map(|l| self.screen_dist(*l, q) > 1.5).unwrap_or(true) {
                    points.push(q);
                    times.push(self.inp.time);
                    if self.tool == ToolId::Polyline && moved {
                        // freehand segments of the polyline
                        if self.ts.curve.last().map(|l| self.screen_dist(l.p, q) > 4.0).unwrap_or(true) {
                            self.ts.curve.push(Node::corner(q));
                        }
                    }
                }
            }
            Drag::Brush { last, started } => {
                if !*started {
                    self.begin_brush();
                    *started = true;
                }
                let delta = p - *last;
                self.brush_step(p, delta);
                *last = p;
            }
            Drag::Connector { .. } => {
                self.ts.cur = match self.attach_at(p) {
                    Some((_, _, q)) => q,
                    None => self.snap(p, &[]),
                };
            }
            Drag::ConnectorEnd { obj, is_start } => {
                let q = match self.attach_at(p) {
                    Some((_, _, q)) => q,
                    None => p,
                };
                if moved {
                    if let Some(o) = self.d.doc.find_mut(*obj) {
                        if let ObjKind::Connector(c) = &mut o.kind {
                            let e = if *is_start { &mut c.start } else { &mut c.end };
                            e.point = q;
                        }
                    }
                    self.d.history.touch();
                }
                self.ts.cur = q;
            }
            Drag::Envelope { obj, handle, start, orig, moved: started } => {
                if !*started && moved {
                    self.d.checkpoint("Envelope");
                    *started = true;
                }
                if *started {
                    if let Some(o) = self.d.doc.find_mut(*obj) {
                        let bb = warp_bbox(o);
                        if bb.width() > 1e-9 && bb.height() > 1e-9 {
                            let dv = p - *start;
                            let mut h = *orig;
                            h[*handle] += Vec2::new(dv.x / bb.width(), dv.y / bb.height());
                            if let Some(env) = o.effects.envelope.as_mut() {
                                env.handles = h;
                            }
                        }
                    }
                    self.d.history.touch();
                }
            }
            Drag::BezierHandle { node } => {
                if moved {
                    let h = p - *node;
                    let n = self.ts.curve.len();
                    if n >= 1 {
                        let last = &mut self.ts.curve[n - 1];
                        last.c_out = *node + h;
                        last.c_in = *node - h;
                        last.kind = NodeKind::Symmetric;
                        last.line_out = false;
                        if n >= 2 {
                            self.ts.curve[n - 2].line_out = false;
                        }
                    }
                }
                self.ts.cur = p;
            }
        }
        self.ts.drag = drag;
    }

    // ------------------------------------------------------------------------------ release

    fn release(&mut self) {
        let p = self.raw;
        let moved = self.ts.moved;
        let drag = std::mem::take(&mut self.ts.drag);
        let mods = self.mods();
        match drag {
            Drag::None => {}
            Drag::Marquee { start } => {
                if moved {
                    let r = Rect::from_points(start, p);
                    // Alt selects objects touched by the marquee
                    let ids = self.d.doc.objects_in_rect(self.d.page, r, mods.alt);
                    if mods.shift {
                        for id in ids {
                            if !self.d.selection.contains(&id) {
                                self.d.selection.push(id);
                            }
                        }
                    } else {
                        self.d.selection = ids;
                    }
                    self.d.rotate_mode = false;
                } else if !mods.shift {
                    self.d.selection.clear();
                    self.d.rotate_mode = false;
                }
            }
            Drag::Lasso { points } => {
                if points.len() > 2 {
                    let bb = points.iter().fold(Rect::from_points(points[0], points[0]), |r, q| r.union_pt(*q));
                    let poly = polyline(&points, true).to_bez();
                    let ids: Vec<ObjId> = self
                        .d
                        .doc
                        .objects_in_rect(self.d.page, bb, true)
                        .into_iter()
                        .filter(|id| self.d.doc.find(*id).map(|o| geom::contains(&poly, o.bounds().center(), false)).unwrap_or(false))
                        .collect();
                    if mods.shift {
                        for id in ids {
                            if !self.d.selection.contains(&id) {
                                self.d.selection.push(id);
                            }
                        }
                    } else {
                        self.d.selection = ids;
                    }
                }
            }
            Drag::Transform { kind, start, bb, ids, center } => {
                if moved {
                    let a = xform_affine(kind, start, self.ts.cur, bb, center, mods, self.s.constrain_angle);
                    let copy = self.ts.copy_on_drop;
                    let label = match kind {
                        Xform::Move => "Move",
                        Xform::Scale(_) => "Stretch",
                        Xform::Rotate => "Rotate",
                        Xform::Skew(_) => "Skew",
                    };
                    let ids: Vec<ObjId> = ids.into_iter().filter(|id| self.d.doc.find(*id).map(|o| !o.locked).unwrap_or(false)).collect();
                    if !ids.is_empty() {
                        self.d.modify(label, |doc, sel| {
                            let targets = if copy { ops::duplicate(doc, &ids, Vec2::ZERO) } else { ids.clone() };
                            doc.transform_objects(&targets, a);
                            *sel = targets;
                        });
                    }
                    self.ts.copy_on_drop = false;
                } else if self.ts.was_selected && !mods.shift && self.tool == ToolId::Pick {
                    self.d.rotate_mode = !self.d.rotate_mode;
                }
                self.ts.was_selected = false;
            }
            Drag::Create { start } => self.release_create(start, moved),
            Drag::Nodes { moved: started, .. } | Drag::Control { moved: started, .. } | Drag::Corner { moved: started, .. } | Drag::Envelope { moved: started, .. } => {
                if started {
                    self.d.after_change();
                    self.d.history.touch();
                }
            }
            Drag::NodeMarquee { start } => {
                if moved {
                    let r = Rect::from_points(start, p);
                    if !mods.shift {
                        self.d.node_sel.clear();
                    }
                    for id in self.d.selection.clone() {
                        if let Some(o) = self.d.doc.find(id) {
                            if let ObjKind::Path(np) = &o.kind {
                                for (si, sp) in np.subpaths.iter().enumerate() {
                                    for (ni, n) in sp.nodes.iter().enumerate() {
                                        if r.contains(o.transform * n.p) && !self.d.node_sel.contains(&(id, si, ni)) {
                                            self.d.node_sel.push((id, si, ni));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // marquee over unselected curves selects them first
                    if self.d.node_sel.is_empty() {
                        let ids = self.d.doc.objects_in_rect(self.d.page, r, false);
                        if !ids.is_empty() {
                            self.d.selection = ids;
                        }
                    }
                } else if !mods.shift {
                    self.d.selection.clear();
                }
            }
            Drag::Pan { .. } => {}
            Drag::Guide { index, vertical } => self.release_guide(index, vertical),
            Drag::Stroke { points, times } => self.release_stroke(points, times, moved),
            Drag::Brush { started, .. } => {
                if started {
                    self.d.after_change();
                    self.d.history.touch();
                }
            }
            Drag::Effect { start, obj } => {
                if moved {
                    self.apply_effect(obj, start, p);
                }
            }
            Drag::Blend { from, .. } => {
                if let Some(to) = self.hit(p) {
                    if to != from {
                        let steps = self.ts.blend_steps.max(1);
                        self.d.modify("Blend", |doc, sel| {
                            if let Some(id) = ops::create_blend(doc, from, to, steps) {
                                *sel = vec![id];
                            }
                        });
                    }
                }
            }
            Drag::Connector { start, from } => {
                if moved {
                    let end = self.attach_at(p);
                    let to = end.map(|e| e.2).unwrap_or(self.ts.cur);
                    let kind = match self.tool {
                        ToolId::RightAngleConnector => ConnectorKind::RightAngle,
                        ToolId::RoundedConnector => ConnectorKind::RoundedRightAngle,
                        _ => ConnectorKind::Straight,
                    };
                    let c = ConnectorObj {
                        kind,
                        start: ConnectorEnd { point: start, attached: from },
                        end: ConnectorEnd { point: to, attached: end.map(|e| (e.0, e.1)) },
                        corner_radius: if kind == ConnectorKind::RoundedRightAngle { 8.0 } else { 0.0 },
                    };
                    let id = self.d.doc.new_id();
                    let mut st = self.ts.new_style();
                    st.fill = Fill::None;
                    self.d.add_objects("Create Connector", vec![Object::new(id, ObjKind::Connector(c), Affine::IDENTITY, st)]);
                }
            }
            Drag::ConnectorEnd { obj, is_start } => {
                if moved {
                    let att = self.attach_at(p);
                    let q = self.ts.cur;
                    // restore so the undo step records the pre-drag state
                    self.d.modify("Edit Anchor", |doc, _| {
                        if let Some(o) = doc.find_mut(obj) {
                            if let ObjKind::Connector(c) = &mut o.kind {
                                let e = if is_start { &mut c.start } else { &mut c.end };
                                e.point = q;
                                e.attached = att.map(|a| (a.0, a.1));
                            }
                        }
                    });
                }
            }
            Drag::BezierHandle { .. } => {}
        }
        self.ts.snap_mark = None;
        self.ts.text_drag_select = false;
    }

    fn release_create(&mut self, start: Point, moved: bool) {
        let cur = self.ts.cur;
        let mods = self.mods();
        use ToolId::*;
        match self.tool {
            Zoom => {
                if moved {
                    self.d.zoom_to_rect(Rect::from_points(start, self.raw), 0.0);
                } else {
                    let f = if mods.shift { 0.5 } else { 2.0 };
                    let at = if self.s.zoom_center_mouse || true { self.inp.pos } else { self.d.canvas_rect.center() };
                    self.d.zoom_at(at, f);
                }
            }
            Crop => {
                if moved {
                    self.ts.crop_rect = Some(Rect::from_points(start, cur));
                }
            }
            Knife => {
                if moved {
                    let page = self.d.page;
                    let (a, b) = (start, self.raw);
                    self.d.modify("Knife", |doc, sel| {
                        let ids = ops::knife(doc, page, a, b);
                        if !ids.is_empty() {
                            *sel = ids;
                        }
                    });
                }
            }
            Text => {
                if self.ts.text_drag_select {
                    return;
                }
                if moved {
                    let r = Rect::from_points(start, cur);
                    if r.width() > 2.0 && r.height() > 2.0 {
                        self.ts.pending_text = Some((Point::new(r.x0, r.y0), Some(r)));
                        self.d.selection.clear();
                        return;
                    }
                }
                self.ts.pending_text = Some((start, None));
                self.d.selection.clear();
            }
            ThreePointCurve | Rect3 | Ellipse3 => {
                if moved {
                    self.ts.stage = vec![start, cur];
                }
            }
            ParallelDim | HvDim | Callout => {
                if moved {
                    self.ts.stage = vec![start, cur];
                }
            }
            _ => {
                if !moved {
                    return;
                }
                let id = self.d.doc.new_id();
                // reserve ids for graph paper cells
                if self.tool == GraphPaper {
                    for _ in 0..(self.s.graph_cols.max(1) * self.s.graph_rows.max(1)) {
                        self.d.doc.new_id();
                    }
                }
                if let Some(o) = creation_object(self.ts, self.s, self.tool, start, cur, mods, id) {
                    let label = format!("Create {}", self.tool.name());
                    self.d.add_objects(&label, vec![o]);
                }
            }
        }
    }

    fn release_guide(&mut self, index: Option<usize>, vertical: bool) {
        let p = self.raw;
        let inside = self.d.canvas_rect.contains(self.inp.pos);
        let page = self.d.page;
        self.d.modify(if index.is_some() { "Move Guideline" } else { "Add Guideline" }, |doc, _| {
            let guides = &mut doc.pages[page].guides;
            match index {
                Some(i) if i < guides.len() => {
                    if !inside {
                        guides.remove(i);
                    } else if guides[i].locked {
                    } else if vertical {
                        guides[i].pos.x = p.x;
                    } else {
                        guides[i].pos.y = p.y;
                    }
                }
                _ => {
                    if inside {
                        guides.push(vector_core::Guide {
                            kind: if vertical { vector_core::GuideKind::Vertical } else { vector_core::GuideKind::Horizontal },
                            pos: p,
                            locked: false,
                            color: Color::rgb(0x00, 0x99, 0xff),
                        });
                    }
                }
            }
        });
    }

    fn release_stroke(&mut self, points: Vec<Point>, times: Vec<f64>, moved: bool) {
        use ToolId::*;
        match self.tool {
            Eraser => {
                if points.len() >= 1 {
                    let page = self.d.page;
                    let w = self.s.eraser_thickness.max(0.5);
                    let mut pts = points;
                    if pts.len() == 1 {
                        pts.push(pts[0] + Vec2::new(0.01, 0.0));
                    }
                    self.d.modify("Erase", |doc, sel| {
                        let ids = ops::erase(doc, page, &pts, w);
                        if !ids.is_empty() {
                            *sel = ids;
                        }
                    });
                }
            }
            Polyline => {
                // nodes were added on press and while dragging
            }
            Freehand if !moved => {
                // click-click straight lines
                let p = points[0];
                match self.ts.line_start.take() {
                    Some(a) => {
                        let (a, b) = constrain_line(a, p, self.mods(), self.s.constrain_angle);
                        if (b - a).hypot() > 1e-6 {
                            add_curve(self.ts, self.d, polyline(&[a, b], false), "Create Line");
                        }
                    }
                    None => self.ts.line_start = Some(p),
                }
            }
            _ => {
                if points.len() < 2 || !moved {
                    return;
                }
                let zoom = self.d.zoom.max(1e-6);
                let join = self.d.doc_len(self.s.auto_join.max(1.0) as f32);
                let closed_by_proximity = points.len() > 3 && (points[0] - *points.last().unwrap()).hypot() <= join;
                let fit = |pts: &[Point], smoothing: f64, closed: bool| -> NodePath {
                    let scaled: Vec<Point> = pts.iter().map(|p| Point::new(p.x * zoom, p.y * zoom)).collect();
                    let mut np = fit_freehand(&scaled, smoothing, closed);
                    np.transform(Affine::scale(1.0 / zoom));
                    np
                };
                match self.tool {
                    Freehand | LiveSketch => {
                        let smoothing = if self.tool == LiveSketch { (self.s.freehand_smoothing + 30.0).min(100.0) } else { self.s.freehand_smoothing };
                        let closed = self.ts.auto_close || closed_by_proximity;
                        let np = fit(&points, smoothing, closed);
                        add_curve(self.ts, self.d, np, if self.tool == LiveSketch { "LiveSketch" } else { "Freehand" });
                    }
                    SmartDrawing => {
                        let id = self.d.doc.new_id();
                        let level = [0.0, 0.4, 0.7, 1.0, 1.5, 2.0][self.s.smart_recognition.min(5) as usize];
                        let shape = if level > 0.0 { recognize_shape(self.ts, &points, closed_by_proximity, self.d.doc_len(6.0 * level), id) } else { None };
                        let o = shape.unwrap_or_else(|| {
                            let np = fit(&points, self.s.smart_smoothing.min(5) as f64 * 20.0, closed_by_proximity);
                            let mut st = self.ts.new_style();
                            if !closed_by_proximity {
                                st.fill = Fill::None;
                            }
                            Object::new(id, ObjKind::Path(np), Affine::IDENTITY, st)
                        });
                        self.d.add_objects("Smart Drawing", vec![o]);
                    }
                    ArtisticMedia | PainterlyBrush => {
                        let np = fit(&points, 40.0, false);
                        let profile = if self.tool == PainterlyBrush {
                            WidthProfile::Painterly { bristles: 14, jitter: 0.35, seed: (self.inp.time * 1000.0) as u32 }
                        } else {
                            match self.ts.media_kind {
                                1 => WidthProfile::TaperEnd,
                                2 => WidthProfile::TaperStart,
                                3 => WidthProfile::Calligraphic { angle: 45.0, min_ratio: 0.2 },
                                4 => WidthProfile::Pressure(speed_pressure(&points, &times)),
                                5 => WidthProfile::Painterly { bristles: 14, jitter: 0.35, seed: (self.inp.time * 1000.0) as u32 },
                                _ => WidthProfile::Taper,
                            }
                        };
                        let id = self.d.doc.new_id();
                        let color = self.ts.new_fill.unwrap_or(Color::BLACK);
                        let style = Style { fill: Fill::None, outline: Outline { width: self.ts.media_width.max(0.1), color: Some(color), profile: Some(profile), cap: vector_core::style::LineCap::Round, join: vector_core::style::LineJoin::Round, ..Outline::default() }, ..Default::default() };
                        let o = Object::new(id, ObjKind::Path(np), Affine::IDENTITY, style);
                        self.d.add_objects("Artistic Media", vec![o]);
                    }
                    _ => {}
                }
            }
        }
    }

    // ------------------------------------------------------------------------------ other events

    fn double_click(&mut self) {
        let p = self.raw;
        use ToolId::*;
        match self.tool {
            Bezier | Pen | Polyline | BSpline => {
                self.ts.drag = Drag::None;
                super::finish(self.ts, self.d, self.s);
            }
            Crop => {
                if self.ts.crop_rect.map(|r| r.contains(p)).unwrap_or(false) {
                    super::finish(self.ts, self.d, self.s);
                }
            }
            Pick => {
                if let Some(id) = self.hit(p) {
                    match self.d.doc.find(id).map(|o| &o.kind) {
                        Some(ObjKind::Text(_)) => {
                            self.out.tool = Some(Text);
                            self.out.cmds.push(Cmd::EditText);
                        }
                        Some(ObjKind::Path(_)) => self.out.tool = Some(Shape),
                        Some(ObjKind::Group(_)) | Some(ObjKind::PowerClip { .. }) if self.s.dblclick_focus_mode => {
                            self.d.focus = vec![id];
                            self.d.cache.key = 0;
                        }
                        Some(ObjKind::Image(_)) if self.s.dblclick_bitmap_editor => {}
                        _ => {}
                    }
                } else if !self.d.focus.is_empty() {
                    self.d.focus.clear();
                    self.d.cache.key = 0;
                }
            }
            Shape => {
                // add a node where a selected curve was double-clicked
                for id in self.d.selection.clone() {
                    let Some(o) = self.d.doc.find(id) else { continue };
                    let ObjKind::Path(np) = &o.kind else { continue };
                    let local = o.transform.inverse() * p;
                    if let Some((si, seg, t, q, _)) = np.nearest(local) {
                        if ((o.transform * q) - p).hypot() <= self.tol() && t > 0.02 && t < 0.98 {
                            let mut added = None;
                            self.d.modify("Add Node", |doc, _| {
                                if let Some(o) = doc.find_mut(id) {
                                    if let ObjKind::Path(np) = &mut o.kind {
                                        added = np.insert_node(si, seg, t);
                                    }
                                }
                            });
                            if let Some((a, b)) = added {
                                self.d.node_sel = vec![(id, a, b)];
                            }
                            return;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn secondary(&mut self) {
        use ToolId::*;
        if matches!(self.ts.drag, Drag::Transform { .. }) {
            self.ts.copy_on_drop = true;
            self.out.flash = Some("A copy will be left at the original position".into());
            return;
        }
        match self.tool {
            Zoom => self.d.zoom_at(self.inp.pos, 0.5),
            Bezier | Pen | Polyline | BSpline if !self.ts.curve.is_empty() => super::finish(self.ts, self.d, self.s),
            ColorEyedropper => self.color_eyedropper(true),
            // right-clicking an object selects it before its right-click menu opens
            Pick | FreehandPick | FreeTransform => match self.hit(self.raw) {
                Some(t) if !self.d.selection.contains(&t) => self.d.selection = vec![t],
                Some(_) => {}
                None => self.d.selection.clear(),
            },
            _ => {}
        }
    }

    fn pick_target_click(&mut self) {
        let Some(target) = self.ts.pick_target.take() else { return };
        let Some(t) = self.hit(self.raw) else {
            self.out.flash = Some("No object was clicked".into());
            return;
        };
        let sel: Vec<ObjId> = self.d.selection.iter().copied().filter(|x| *x != t).collect();
        if sel.is_empty() {
            return;
        }
        match target {
            PickTarget::PowerClip => {
                self.d.modify("PowerClip Inside", |doc, s| {
                    if let Some(id) = ops::powerclip(doc, &sel, t) {
                        *s = vec![id];
                    }
                });
            }
            PickTarget::CopyProperties { pen, color, fill, text } => {
                let src = self.d.doc.find(t).cloned();
                if let Some(src) = src {
                    self.d.modify("Copy Properties", |doc, _| {
                        for id in &sel {
                            if let Some(o) = doc.find_mut(*id) {
                                if pen {
                                    let c = o.style.outline.color;
                                    o.style.outline = src.style.outline.clone();
                                    o.style.outline.color = c;
                                }
                                if color {
                                    o.style.outline.color = src.style.outline.color;
                                }
                                if fill {
                                    o.style.fill = src.style.fill.clone();
                                }
                                if text {
                                    if let (ObjKind::Text(dst), ObjKind::Text(from)) = (&mut o.kind, &src.kind) {
                                        dst.format = from.format.clone();
                                        dst.spans.clear();
                                        dst.align = from.align.clone();
                                        dst.line_spacing = from.line_spacing;
                                    }
                                }
                            }
                        }
                    });
                }
            }
            PickTarget::CopyEffect => {
                let fx = self.d.doc.find(t).map(|o| o.effects.clone());
                if let Some(fx) = fx {
                    self.d.modify("Copy Effect", |doc, _| {
                        for id in &sel {
                            if let Some(o) = doc.find_mut(*id) {
                                o.effects = fx.clone();
                            }
                        }
                    });
                }
            }
            PickTarget::FitToPath => {
                let text = sel[0];
                let ok = self.d.modify("Fit Text to Path", |doc, _| ops::fit_text_to_path(doc, text, t));
                if !ok {
                    self.out.flash = Some("Select a text object, then click a curve".into());
                }
            }
        }
    }

    // ------------------------------------------------------------------------------ tool actions

    fn add_dimension(&mut self, pts: Vec<Point>) {
        let kind = match self.tool {
            ToolId::HvDim => DimensionKind::HorizontalVertical,
            ToolId::AngularDim => DimensionKind::Angular,
            ToolId::SegmentDim => DimensionKind::Segment,
            ToolId::Callout => DimensionKind::Callout,
            _ => DimensionKind::Parallel,
        };
        let angular = kind == DimensionKind::Angular;
        let dim = DimensionObj {
            kind,
            points: pts,
            text: String::new(),
            precision: if angular { self.s.angular_precision } else { self.s.dim_precision } as usize,
            prefix: self.s.dim_prefix.clone(),
            suffix: self.s.dim_suffix.clone(),
            show_units: !angular,
            font_size: 12.0,
            unit: self.d.doc.settings.unit,
        };
        let id = self.d.doc.new_id();
        let mut st = self.ts.new_style();
        st.fill = Fill::Solid(Color::BLACK);
        st.outline.color = Some(Color::BLACK);
        let label = format!("Create {}", self.tool.name());
        self.d.add_objects(&label, vec![Object::new(id, ObjKind::Dimension(dim), Affine::IDENTITY, st)]);
    }

    /// The straight chord of the segment of a curve under the pointer (document coordinates).
    fn segment_at(&self, p: Point) -> Option<(Point, Point)> {
        let id = self.hit(p)?;
        let o = self.d.doc.find(id)?;
        // to_curve() is already in document coordinates
        let np = o.to_curve()?;
        let (si, seg, _, _, _) = np.nearest(p)?;
        let (c, _) = np.subpaths.get(si)?.segment(seg)?;
        Some((c.p0, c.p3))
    }

    /// Connector attachment under the pointer: (object, normalized anchor, document point).
    fn attach_at(&self, p: Point) -> Option<(ObjId, Point, Point)> {
        let id = self.hit(p)?;
        let o = self.d.doc.find(id)?;
        if matches!(o.kind, ObjKind::Connector(_) | ObjKind::Dimension(_)) {
            return None;
        }
        let b = self.d.doc.bounds_of(o);
        if b.width() <= 0.0 || b.height() <= 0.0 {
            return None;
        }
        let anchors = [(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5), (0.5, 0.5)];
        let mut best = (f64::MAX, Point::ZERO, Point::ZERO);
        for (u, v) in anchors {
            let q = Point::new(b.x0 + u * b.width(), b.y0 + v * b.height());
            let dd = (q - p).hypot();
            if dd < best.0 {
                best = (dd, Point::new(u, v), q);
            }
        }
        Some((id, best.1, best.2))
    }

    fn apply_effect(&mut self, obj: ObjId, start: Point, end: Point) {
        let dv = end - start;
        let tool = self.tool;
        let Some(o) = self.d.doc.find(obj) else { return };
        let bb = o.bounds();
        let center = bb.center();
        let main = o.style.fill.main_color().unwrap_or(Color::rgb(0x80, 0x80, 0x80));
        let uv0 = fill_uv(o, start);
        let uv1 = fill_uv(o, end);
        let wbb = warp_bbox(o);
        let distort_kind = self.ts.distort_kind;
        let label = match tool {
            ToolId::DropShadow => "Drop Shadow",
            ToolId::InnerShadow => "Inner Shadow",
            ToolId::BlockShadow => "Block Shadow",
            ToolId::Contour => "Contour",
            ToolId::Distort => "Distortion",
            ToolId::Extrude => "Extrude",
            ToolId::Transparency => "Transparency",
            _ => "Fill",
        };
        self.d.modify(label, |doc, _| {
            let Some(o) = doc.find_mut(obj) else { return };
            match tool {
                ToolId::DropShadow => o.effects.drop_shadow = Some(DropShadow { offset: dv, feather: 8.0, color: Color::BLACK, opacity: 50.0, inner: false }),
                ToolId::InnerShadow => o.effects.inner_shadow = Some(DropShadow { offset: dv, feather: 6.0, color: Color::BLACK, opacity: 70.0, inner: true }),
                ToolId::BlockShadow => o.effects.block_shadow = Some(BlockShadow { offset: dv, color: Color::rgb(0x40, 0x40, 0x40) }),
                ToolId::Contour => {
                    let outward = (end - center).hypot() > (start - center).hypot();
                    let steps = 4;
                    o.effects.contour = Some(ContourEffect {
                        dir: if outward { ContourDir::Outside } else { ContourDir::Inside },
                        steps,
                        offset: (dv.hypot() / steps as f64).max(0.5),
                        end_fill: Color::WHITE,
                        end_outline: Color::BLACK,
                    });
                }
                ToolId::Distort => {
                    let rmax = (wbb.width().max(wbb.height()) / 2.0).max(1.0);
                    let c = Point::new(((start.x - wbb.x0) / wbb.width().max(1e-9)).clamp(0.0, 1.0), ((start.y - wbb.y0) / wbb.height().max(1e-9)).clamp(0.0, 1.0));
                    let amount = match distort_kind {
                        DistortKind::Twister => dv.x / rmax * 360.0,
                        _ => (dv.x / rmax * 100.0).clamp(-200.0, 200.0),
                    };
                    o.effects.distort = Some(DistortEffect { kind: distort_kind, amount, frequency: 6.0, center: c });
                }
                ToolId::Extrude => {
                    let shade = Color::rgb((main.r as f64 * 0.45) as u8, (main.g as f64 * 0.45) as u8, (main.b as f64 * 0.45) as u8);
                    o.effects.extrude = Some(ExtrudeEffect { depth: dv, color: main, shade, steps: 24 });
                }
                ToolId::Transparency => {
                    let mut g = Gradient::linear(Color::WHITE, Color::BLACK);
                    g.start = uv0;
                    g.end = uv1;
                    o.style.transparency = Transparency::Gradient(g);
                }
                _ => {
                    let mut g = Gradient::linear(main, Color::WHITE);
                    g.start = uv0;
                    g.end = uv1;
                    o.style.fill = Fill::Gradient(g);
                }
            }
        });
    }

    fn color_eyedropper(&mut self, outline: bool) {
        let p = self.raw;
        if self.ts.eyedrop_color.is_none() || self.mods().shift {
            let c = sample_color(self.ts, self.d, p);
            self.ts.eyedrop_color = Some(c);
            self.out.flash = Some(format!("Sampled color R{} G{} B{}", c.r, c.g, c.b));
            return;
        }
        let Some(c) = self.ts.eyedrop_color else { return };
        if let Some(id) = self.hit(p) {
            self.d.modify(if outline { "Outline Color" } else { "Fill Color" }, |doc, _| {
                if let Some(o) = doc.find_mut(id) {
                    if outline {
                        o.style.outline.color = Some(c);
                    } else {
                        o.style.fill = Fill::Solid(c);
                    }
                }
            });
        }
    }

    fn attributes_eyedropper(&mut self) {
        let p = self.raw;
        let Some(id) = self.hit(p) else { return };
        if self.ts.eyedrop_attrs.is_none() || self.mods().shift {
            if let Some(o) = self.d.doc.find(id) {
                self.ts.eyedrop_attrs = Some((o.style.clone(), o.effects.clone()));
                self.out.flash = Some("Attributes sampled; click objects to apply them".into());
            }
            return;
        }
        let Some((st, fx)) = self.ts.eyedrop_attrs.clone() else { return };
        self.d.modify("Apply Attributes", |doc, _| {
            if let Some(o) = doc.find_mut(id) {
                o.style = st;
                o.effects = fx;
            }
        });
    }

    fn mesh_fill_click(&mut self) {
        let p = self.raw;
        let Some(id) = self.hit(p) else {
            self.d.selection.clear();
            return;
        };
        self.d.selection = vec![id];
        let (cols, rows) = (self.s.mesh_cols.max(1) as usize, self.s.mesh_rows.max(1) as usize);
        let Some(o) = self.d.doc.find(id) else { return };
        if let Fill::Mesh(m) = &o.style.fill {
            // select the patch under the pointer (used by palette clicks)
            let uv = fill_uv(o, p);
            let c = ((uv.x * m.cols as f64).floor().max(0.0) as usize).min(m.cols.saturating_sub(1));
            let r = ((uv.y * m.rows as f64).floor().max(0.0) as usize).min(m.rows.saturating_sub(1));
            self.ts.table_cell = Some((r, c));
            return;
        }
        let base = o.style.fill.main_color().unwrap_or(Color::WHITE);
        self.d.modify("Mesh Fill", |doc, _| {
            if let Some(o) = doc.find_mut(id) {
                o.style.fill = Fill::Mesh(vector_core::style::MeshFill::new(cols, rows, base));
            }
        });
    }

    fn smart_fill(&mut self) {
        let p = self.raw;
        let page = self.d.page;
        match ops::smart_fill_region(&self.d.doc, page, p) {
            Some(bp) => {
                let id = self.d.doc.new_id();
                let mut st = self.ts.new_style();
                st.fill = Fill::Solid(self.ts.new_fill.unwrap_or(self.default_fill));
                let o = Object::new(id, ObjKind::Path(NodePath::from_bez(&bp)), Affine::IDENTITY, st);
                self.d.add_objects("Smart Fill", vec![o]);
            }
            None => self.out.flash = Some("Click inside an area enclosed by objects".into()),
        }
    }

    fn virtual_delete(&mut self, p: Point) {
        let tol = self.tol();
        // nearest curve segment under the pointer
        let mut best: Option<(ObjId, usize, usize, f64, f64)> = None;
        for o in self.d.page_ref().all_objects() {
            if !o.visible || o.locked {
                continue;
            }
            let ObjKind::Path(np) = &o.kind else { continue };
            let local = o.transform.inverse() * p;
            if let Some((si, seg, t, q, _)) = np.nearest(local) {
                let dd = ((o.transform * q) - p).hypot();
                if dd <= tol && best.map_or(true, |b| dd < b.4) {
                    best = Some((o.id, si, seg, t, dd));
                }
            }
        }
        let Some((id, si, seg, t, _)) = best else {
            self.out.flash = Some("Click a curve segment between intersections".into());
            return;
        };
        let Some(o) = self.d.doc.find(id) else { return };
        let ObjKind::Path(np) = &o.kind else { return };
        let Some((cubic, _)) = np.subpaths[si].segment(seg) else { return };
        let tr = o.transform;
        let wc = CubicBez::new(tr * cubic.p0, tr * cubic.p1, tr * cubic.p2, tr * cubic.p3);
        // other geometry as polylines
        let mut lines: Vec<Vec<Point>> = vec![];
        for (osi, sp) in np.subpaths.iter().enumerate() {
            for k in 0..sp.segment_count() {
                if osi == si && k == seg {
                    continue;
                }
                if let Some((c, _)) = sp.segment(k) {
                    let c = CubicBez::new(tr * c.p0, tr * c.p1, tr * c.p2, tr * c.p3);
                    lines.push((0..=24).map(|i| c.eval(i as f64 / 24.0)).collect());
                }
            }
        }
        for other in self.d.page_ref().all_objects() {
            if other.id == id || !other.visible {
                continue;
            }
            for (pts, closed) in geom::flatten_polylines(&other.world_path(), 0.1) {
                let mut pts = pts;
                if closed && !pts.is_empty() {
                    pts.push(pts[0]);
                }
                lines.push(pts);
            }
        }
        let n = 96;
        let samples: Vec<Point> = (0..=n).map(|i| wc.eval(i as f64 / n as f64)).collect();
        let mut ts_hits: Vec<f64> = vec![];
        for i in 0..n {
            for l in &lines {
                for w in l.windows(2) {
                    if let Some((ta, _, _)) = geom::seg_intersect(samples[i], samples[i + 1], w[0], w[1]) {
                        let tt = (i as f64 + ta) / n as f64;
                        if tt > 0.005 && tt < 0.995 {
                            ts_hits.push(tt);
                        }
                    }
                }
            }
        }
        let t0 = ts_hits.iter().copied().filter(|x| *x < t).fold(None, |a: Option<f64>, x| Some(a.map_or(x, |a| a.max(x))));
        let t1 = ts_hits.iter().copied().filter(|x| *x > t).fold(None, |a: Option<f64>, x| Some(a.map_or(x, |a| a.min(x))));
        self.d.modify("Virtual Segment Delete", |doc, _| {
            let Some(o) = doc.find_mut(id) else { return };
            let ObjKind::Path(np) = &mut o.kind else { return };
            let mut target = seg;
            if let Some(t1) = t1 {
                np.insert_node(si, seg, t1);
            }
            if let Some(t0) = t0 {
                let tt = match t1 {
                    Some(t1) => t0 / t1,
                    None => t0,
                };
                np.insert_node(si, seg, tt);
                target = seg + 1;
            }
            np.delete_segment(si, target);
            if np.subpaths.iter().all(|s| s.nodes.len() < 2) {
                let _ = doc.remove(id);
            }
        });
    }

    // ------------------------------------------------------------------------------ brushes

    fn begin_brush(&mut self) {
        // the deformation brushes work on curves: convert the selected shapes first
        let needs: Vec<ObjId> = self.d.selection.iter().copied().filter(|id| self.d.doc.find(*id).map(|o| !matches!(o.kind, ObjKind::Path(_)) && !o.is_container()).unwrap_or(false)).collect();
        self.d.checkpoint(self.tool.name());
        if !needs.is_empty() {
            let new_ids = ops::convert_to_curves(&mut self.d.doc, &needs);
            self.d.selection.retain(|id| !needs.contains(id));
            self.d.selection.extend(new_ids);
        }
    }

    fn brush_step(&mut self, p: Point, delta: Vec2) {
        let radius = (self.ts.brush_size / 2.0).max(0.5);
        let strength = (self.ts.brush_strength / 100.0).clamp(0.01, 1.0);
        let dt = self.inp.dt.clamp(0.0, 0.1);
        let tool = self.tool;
        let repel = self.mods().ctrl;
        let ccw = self.mods().ctrl;
        let ids = self.d.selection.clone();
        let mut changed = false;
        for id in ids {
            let Some(o) = self.d.doc.find_mut(id) else { continue };
            let t = o.transform;
            let inv = t.inverse();
            let lp = inv * p;
            let ld = inv * Point::new(delta.x, delta.y) - inv * Point::ZERO;
            let scale = (inv * Point::new(radius, 0.0) - inv * Point::ZERO).hypot();
            let r = scale.max(1e-6);
            let ObjKind::Path(np) = &mut o.kind else { continue };
            if tool == ToolId::Roughen {
                changed |= roughen(np, lp, r, strength);
                continue;
            }
            for sp in np.subpaths.iter_mut() {
                let n = sp.nodes.len();
                let snapshot: Vec<Point> = sp.nodes.iter().map(|x| x.p).collect();
                for i in 0..n {
                    let dist = (sp.nodes[i].p - lp).hypot();
                    if dist > r {
                        continue;
                    }
                    let fall = 1.0 - dist / r;
                    let w = fall * fall * (3.0 - 2.0 * fall);
                    let mv: Vec2 = match tool {
                        ToolId::Smear | ToolId::Smudge => ld * (w * strength),
                        ToolId::AttractRepel => {
                            let v = lp - sp.nodes[i].p;
                            let k = w * strength * dt * 4.0;
                            if repel {
                                -v * k
                            } else {
                                v * k
                            }
                        }
                        ToolId::Twirl => {
                            let ang = w * strength * dt * 6.0 * if ccw { -1.0 } else { 1.0 };
                            let v = sp.nodes[i].p - lp;
                            let (sn, cs) = ang.sin_cos();
                            let nv = Vec2::new(v.x * cs - v.y * sn, v.x * sn + v.y * cs);
                            nv - v
                        }
                        ToolId::Smooth => {
                            if !sp.closed && (i == 0 || i == n - 1) {
                                Vec2::ZERO
                            } else {
                                let prev = snapshot[(i + n - 1) % n];
                                let next = snapshot[(i + 1) % n];
                                let mid = prev.midpoint(next);
                                (mid - snapshot[i]) * (w * strength * 0.15)
                            }
                        }
                        _ => Vec2::ZERO,
                    };
                    if mv.hypot() > 0.0 {
                        let nd = &mut sp.nodes[i];
                        nd.p += mv;
                        nd.c_in += mv;
                        nd.c_out += mv;
                        if tool == ToolId::Smooth {
                            nd.kind = NodeKind::Smooth;
                        }
                        changed = true;
                    }
                }
                if tool == ToolId::Smooth && changed {
                    // re-derive smooth handles for the moved nodes
                    for i in 0..n {
                        if (sp.nodes[i].p - lp).hypot() > r || (!sp.closed && (i == 0 || i == n - 1)) {
                            continue;
                        }
                        let prev = sp.nodes[(i + n - 1) % n].p;
                        let next = sp.nodes[(i + 1) % n].p;
                        let tan = (next - prev) * (1.0 / 6.0);
                        let nd = &mut sp.nodes[i];
                        nd.c_in = nd.p - tan;
                        nd.c_out = nd.p + tan;
                        nd.line_out = false;
                        let pi = (i + n - 1) % n;
                        sp.nodes[pi].line_out = false;
                    }
                }
            }
        }
        if changed {
            self.d.history.touch();
        }
    }
}

/// Elastic mode: unselected nodes follow with a falloff based on their distance to the dragged nodes.
fn elastic_move(np: &mut NodePath, refs: &[(usize, usize)], d: Vec2) {
    let sel_pts: Vec<Point> = refs.iter().filter_map(|r| np.node(*r).map(|n| n.p)).collect();
    let bb = np.bounds();
    let span = bb.width().max(bb.height()).max(1e-6);
    for (si, sp) in np.subpaths.iter_mut().enumerate() {
        for (ni, n) in sp.nodes.iter_mut().enumerate() {
            let w = if refs.contains(&(si, ni)) {
                1.0
            } else {
                let dmin = sel_pts.iter().map(|q| (n.p - *q).hypot()).fold(f64::MAX, f64::min);
                (1.0 - dmin / span).clamp(0.0, 1.0).powi(2)
            };
            let m = d * w;
            n.p += m;
            n.c_in += m;
            n.c_out += m;
        }
    }
}

/// Replace long segments near the brush with zigzags. Returns whether anything changed.
fn roughen(np: &mut NodePath, lp: Point, r: f64, strength: f64) -> bool {
    let spacing = (r / 3.0).max(0.5);
    let amp = r * 0.25 * strength;
    let mut changed = false;
    for sp in np.subpaths.iter_mut() {
        let n = sp.nodes.len();
        if n < 2 {
            continue;
        }
        let segs = if sp.closed { n } else { n - 1 };
        let mut out: Vec<Node> = vec![];
        for i in 0..n {
            let a = sp.nodes[i];
            out.push(a);
            if i >= segs {
                continue;
            }
            let b = sp.nodes[(i + 1) % n];
            let mid = a.p.midpoint(b.p);
            let len = (b.p - a.p).hypot();
            if (mid - lp).hypot() > r || len < spacing * 2.0 {
                continue;
            }
            let c = CubicBez::new(a.p, if a.line_out { a.p } else { a.c_out }, if a.line_out { b.p } else { b.c_in }, b.p);
            let k = (len / spacing).floor() as usize;
            out.last_mut().unwrap().line_out = true;
            out.last_mut().unwrap().c_out = a.p;
            for j in 1..k {
                let t = j as f64 / k as f64;
                let q = c.eval(t);
                let q2 = c.eval((t + 0.01).min(1.0));
                let tan = (q2 - q).normalize();
                let normal = Vec2::new(-tan.y, tan.x);
                let sign = if j % 2 == 0 { 1.0 } else { -1.0 };
                out.push(Node::corner(q + normal * amp * sign));
            }
            changed = true;
        }
        if changed {
            for n in out.iter_mut() {
                if n.c_out == n.p {
                    n.line_out = true;
                }
            }
            sp.nodes = out;
        }
    }
    changed
}

/// Pseudo-pressure from drawing speed (slow = heavy) for the Pressure media.
fn speed_pressure(points: &[Point], times: &[f64]) -> Vec<f32> {
    if points.len() < 2 || times.len() != points.len() {
        return vec![1.0; points.len().max(2)];
    }
    let mut v: Vec<f64> = vec![];
    for i in 1..points.len() {
        let dt = (times[i] - times[i - 1]).max(1e-3);
        v.push((points[i] - points[i - 1]).hypot() / dt);
    }
    let vmax = v.iter().copied().fold(1e-6, f64::max);
    let mut out: Vec<f32> = vec![];
    let mut s = 1.0;
    for x in v {
        let target = 1.0 - (x / vmax) * 0.75;
        s = s * 0.7 + target * 0.3;
        out.push(s as f32);
    }
    out.insert(0, out[0]);
    out
}

/// Smart Drawing: turn a rough stroke into a basic shape when it looks like one.
fn recognize_shape(ts: &ToolState, points: &[Point], closed: bool, tol: f64, id: ObjId) -> Option<Object> {
    let bb = points.iter().fold(Rect::from_points(points[0], points[0]), |r, q| r.union_pt(*q));
    if bb.width() < tol * 2.0 && bb.height() < tol * 2.0 {
        return None;
    }
    let style = ts.new_style();
    let simplified = rdp(points, tol * 1.8);
    if !closed {
        if simplified.len() == 2 {
            let mut st = style;
            st.fill = Fill::None;
            return Some(Object::new(id, ObjKind::Path(polyline(&[points[0], *points.last().unwrap()], false)), Affine::IDENTITY, st));
        }
        return None;
    }
    // ellipse fit error
    let c = bb.center();
    let (rx, ry) = (bb.width() / 2.0, bb.height() / 2.0);
    let ell_err = points.iter().map(|p| (((p.x - c.x) / rx).powi(2) + ((p.y - c.y) / ry).powi(2)).sqrt() - 1.0).map(|e| e.abs()).sum::<f64>() / points.len() as f64;
    // corners of the closed polygon (drop the duplicated end point)
    let mut corners = simplified.clone();
    if corners.len() > 2 && (corners[0] - *corners.last().unwrap()).hypot() < tol * 3.0 {
        corners.pop();
    }
    let n = corners.len();
    if (3..=4).contains(&n) && ell_err > 0.08 {
        if n == 4 {
            // axis-aligned rectangle?
            let rect_err = corners.iter().map(|p| (p.x - bb.x0).abs().min((p.x - bb.x1).abs()) + (p.y - bb.y0).abs().min((p.y - bb.y1).abs())).sum::<f64>() / 4.0;
            if rect_err < tol * 3.0 {
                return Some(Object::new(id, ObjKind::Rect(RectShape { w: bb.width(), h: bb.height(), radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((bb.x0, bb.y0)), style));
            }
        }
        return Some(Object::new(id, ObjKind::Path(polyline(&corners, true)), Affine::IDENTITY, style));
    }
    if ell_err < 0.15 {
        return Some(Object::new(
            id,
            ObjKind::Ellipse(EllipseShape { w: bb.width(), h: bb.height(), kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false }),
            Affine::translate((bb.x0, bb.y0)),
            style,
        ));
    }
    if (5..=8).contains(&n) {
        return Some(Object::new(id, ObjKind::Path(polyline(&corners, true)), Affine::IDENTITY, style));
    }
    None
}

fn rdp(points: &[Point], eps: f64) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let (a, b) = (points[0], *points.last().unwrap());
    let mut idx = 0;
    let mut dmax = 0.0;
    for (i, p) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        let d = geom::dist_point_segment(*p, a, b);
        if d > dmax {
            dmax = d;
            idx = i;
        }
    }
    if dmax > eps {
        let mut left = rdp(&points[..=idx], eps);
        let right = rdp(&points[idx..], eps);
        left.pop();
        left.extend(right);
        left
    } else {
        vec![a, b]
    }
}

/// Render one pixel of the page to sample its color.
fn sample_color(ts: &mut ToolState, d: &DocView, p: Point) -> Color {
    let Some(mut pm) = tiny_skia::Pixmap::new(1, 1) else { return Color::WHITE };
    let view = Affine::translate((0.5 - p.x, 0.5 - p.y));
    let opts = vector_render::RenderOptions { desktop: Color::WHITE, ..Default::default() };
    ts.renderer.render_page(&d.doc, d.page, view, &mut pm, &opts);
    let px = pm.pixel(0, 0).map(|c| c.demultiply());
    match px {
        Some(c) => Color::rgb(c.red(), c.green(), c.blue()),
        None => Color::WHITE,
    }
}

/// Snap a document point to grid, guides, page and object points. Returns the label of the snap.
/// The anchor points of the objects near `p`: bounding box corners, midpoints and centers.
fn anchors_near(d: &DocView, p: Point, reach: f64, exclude: &[ObjId]) -> Vec<(kurbo::Rect, Point)> {
    let mut out = vec![];
    for l in d.page_ref().layers.iter().filter(|l| l.visible) {
        for o in &l.objects {
            if exclude.contains(&o.id) || !o.visible {
                continue;
            }
            let b = d.doc.bounds_of(o);
            if !b.inflate(reach, reach).contains(p) {
                continue;
            }
            out.push((b, b.center()));
            if out.len() > 500 {
                return out;
            }
        }
    }
    out
}

/// Snap a point, also returning the alignment or dynamic guide lines it snapped to.
pub fn snap_point_ex(d: &DocView, s: &Settings, p: Point, exclude: &[ObjId]) -> (Point, Option<&'static str>, Vec<(Point, Point)>) {
    let ds = &d.doc.settings;
    if ds.snap_off {
        return (p, None, vec![]);
    }
    let r = d.doc_len(s.snap_radius.max(2.0) as f32);
    let mut best: Option<(f64, Point, &'static str)> = None;
    let mut consider = |q: Point, label: &'static str| {
        let dist = (q - p).hypot();
        if dist <= r && best.map_or(true, |b| dist < b.0) {
            best = Some((dist, q, label));
        }
    };
    let page = d.page_ref();
    if ds.snap_to_objects && s.snap_to_objects {
        let mut budget = 4000usize;
        for l in &page.layers {
            if !l.visible {
                continue;
            }
            for o in &l.objects {
                if exclude.contains(&o.id) || !o.visible {
                    continue;
                }
                let b = d.doc.bounds_of(o);
                if !b.inflate(r, r).contains(p) {
                    continue;
                }
                for q in [Point::new(b.x0, b.y0), Point::new(b.x1, b.y0), Point::new(b.x1, b.y1), Point::new(b.x0, b.y1)] {
                    consider(q, "Corner");
                }
                consider(b.center(), "Center");
                for q in [Point::new(b.center().x, b.y0), Point::new(b.x1, b.center().y), Point::new(b.center().x, b.y1), Point::new(b.x0, b.center().y)] {
                    consider(q, "Midpoint");
                }
                if let ObjKind::Path(np) = &o.kind {
                    for sp in &np.subpaths {
                        for n in &sp.nodes {
                            if budget == 0 {
                                break;
                            }
                            budget -= 1;
                            consider(o.transform * n.p, "Node");
                        }
                    }
                }
            }
        }
    }
    if ds.snap_to_page || s.snap_to_page {
        let b = page.rect();
        for q in [Point::new(b.x0, b.y0), Point::new(b.x1, b.y0), Point::new(b.x1, b.y1), Point::new(b.x0, b.y1)] {
            consider(q, "Page corner");
        }
        consider(b.center(), "Page center");
        for q in [Point::new(b.center().x, b.y0), Point::new(b.x1, b.center().y), Point::new(b.center().x, b.y1), Point::new(b.x0, b.center().y)] {
            consider(q, "Page edge");
        }
    }
    if let Some((_, q, l)) = best {
        return (q, Some(l), vec![]);
    }
    let mut q = p;
    let mut label = None;
    let mut lines = vec![];
    if ds.snap_to_guides && ds.show_guides {
        for g in page.guides.iter().chain(d.doc.master_guides.iter()) {
            match g.kind {
                vector_core::GuideKind::Horizontal => {
                    if (p.y - g.pos.y).abs() <= r {
                        q.y = g.pos.y;
                        label = Some("Guideline");
                    }
                }
                vector_core::GuideKind::Vertical => {
                    if (p.x - g.pos.x).abs() <= r {
                        q.x = g.pos.x;
                        label = Some("Guideline");
                    }
                }
                vector_core::GuideKind::Angled(_) => {}
            }
        }
    }
    if ds.snap_to_grid {
        let (gx, gy) = (ds.grid_spacing.x.max(1e-3), ds.grid_spacing.y.max(1e-3));
        let o = ds.ruler_origin;
        let sx = ((p.x - o.x) / gx).round() * gx + o.x;
        let sy = ((p.y - o.y) / gy).round() * gy + o.y;
        if (sx - p.x).abs() <= r && label.is_none() {
            q.x = sx;
            label = Some("Grid");
        }
        if (sy - p.y).abs() <= r {
            q.y = sy;
            label = Some("Grid");
        }
    }
    // View ▸ Snap To ▸ Baseline Grid: the lines from the top of the page
    if ds.snap_to_baseline && label.is_none() {
        let pr = page.rect();
        let sp = ds.baseline_spacing.max(0.1);
        let sy = ((p.y - pr.y0) / sp).round() * sp + pr.y0;
        if (sy - p.y).abs() <= r {
            q.y = sy;
            label = Some("Baseline grid");
        }
    }
    let reach = d.doc_len(400.0);
    // View ▸ Dynamic Guides: lines at 0°, 45°, 90° and 135° through the snap points of nearby objects
    if s.dynamic_guides && label.is_none() {
        let mut found: Option<(f64, Point, Point)> = None;
        for (b, c) in anchors_near(d, p, reach, exclude) {
            let pts = [c, Point::new(b.x0, b.y0), Point::new(b.x1, b.y0), Point::new(b.x1, b.y1), Point::new(b.x0, b.y1)];
            for a in pts {
                for deg in [0.0f64, 45.0, 90.0, 135.0] {
                    let dir = kurbo::Vec2::new(deg.to_radians().cos(), -deg.to_radians().sin());
                    let foot = a + dir * (p - a).dot(dir);
                    let dist = (p - foot).hypot();
                    if dist <= r && (foot - a).hypot() > r && found.map_or(true, |f| dist < f.0) {
                        found = Some((dist, foot, a));
                    }
                }
            }
        }
        if let Some((_, f, a)) = found {
            q = f;
            label = Some("Dynamic guide");
            let ext = (f - a) * (d.doc_len(40.0) / (f - a).hypot().max(1e-9));
            lines.push((a, f + ext));
        }
    }
    // View ▸ Alignment Guides: line up with the edges and centers of nearby objects and the page
    if s.live_guides && label.is_none() {
        let mut xs: Vec<(f64, Point)> = vec![];
        let mut ys: Vec<(f64, Point)> = vec![];
        let mut add = |b: kurbo::Rect, edges: bool, centers: bool| {
            let c = b.center();
            if edges {
                xs.extend([(b.x0, Point::new(b.x0, c.y)), (b.x1, Point::new(b.x1, c.y))]);
                ys.extend([(b.y0, Point::new(c.x, b.y0)), (b.y1, Point::new(c.x, b.y1))]);
            }
            if centers {
                xs.push((c.x, c));
                ys.push((c.y, c));
            }
        };
        for (b, _) in anchors_near(d, p, reach, exclude) {
            add(b, s.live_guides_edges, s.live_guides_centers);
        }
        if s.live_guides_page {
            add(page.rect(), true, true);
        }
        let near = |v: f64, list: &[(f64, Point)]| list.iter().filter(|(x, _)| (x - v).abs() <= r).min_by(|a, b| (a.0 - v).abs().total_cmp(&(b.0 - v).abs())).copied();
        let bx = near(p.x, &xs);
        let by = near(p.y, &ys);
        if let Some((x, _)) = bx {
            q.x = x;
        }
        if let Some((y, _)) = by {
            q.y = y;
        }
        if let Some((_, a)) = bx {
            lines.push((a, Point::new(q.x, q.y)));
            label = Some("Alignment guide");
        }
        if let Some((_, a)) = by {
            lines.push((a, Point::new(q.x, q.y)));
            label = Some("Alignment guide");
        }
    }
    // View ▸ Snap To ▸ Pixels (Pixels view): whole document pixels
    if ds.snap_to_pixels && d.view_mode == crate::docview::ViewMode::Pixels && label.is_none() {
        let px = 72.0 / ds.dpi.max(1.0);
        q = Point::new((q.x / px).round() * px, (q.y / px).round() * px);
    }
    (q, label, lines)
}

/// A guide under the pointer: (index into the page guides, vertical).
pub fn guide_at(d: &DocView, pos: Pos2) -> Option<(usize, bool)> {
    if !d.doc.settings.show_guides {
        return None;
    }
    for (i, g) in d.page_ref().guides.iter().enumerate() {
        match g.kind {
            vector_core::GuideKind::Horizontal => {
                if (d.to_screen(Point::new(0.0, g.pos.y)).y - pos.y).abs() <= 3.0 {
                    return Some((i, false));
                }
            }
            vector_core::GuideKind::Vertical => {
                if (d.to_screen(Point::new(g.pos.x, 0.0)).x - pos.x).abs() <= 3.0 {
                    return Some((i, true));
                }
            }
            _ => {}
        }
    }
    None
}

// ------------------------------------------------------------------------------------------
// Text editing keys
// ------------------------------------------------------------------------------------------

/// Keyboard input for the text being edited (or typed at a pending text position).
pub fn handle_text_keys(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(ai) = app.active else { return };
    if app.dialogs.any_modal_open() || ctx.egui_wants_keyboard_input() {
        return;
    }
    let editing = app.docs.get(ai).map(|d| d.text_edit.is_some()).unwrap_or(false);
    if !editing && app.ts.pending_text.is_none() {
        return;
    }
    let events: Vec<egui::Event> = ctx.input(|i| i.events.clone());
    let now = app.time();
    for ev in events {
        match ev {
            egui::Event::Text(t) | egui::Event::Paste(t) => {
                let t = t.replace("\r\n", "\n");
                if !t.is_empty() {
                    type_text(app, &t, now);
                }
            }
            egui::Event::Copy | egui::Event::Cut => {
                let cut = matches!(ev, egui::Event::Cut);
                if let Some(s) = selected_text(app) {
                    ctx.copy_text(s);
                    if cut {
                        delete_selection_or(app, 0, now);
                    }
                }
            }
            egui::Event::Key { key, pressed: true, modifiers, .. } => {
                use egui::Key;
                match key {
                    Key::Escape => {
                        let ts = &mut app.ts;
                        if let Some(d) = app.docs.get_mut(ai) {
                            let obj = d.text_edit.as_ref().map(|t| t.obj);
                            super::finish(ts, d, &app.settings);
                            if let Some(o) = obj {
                                if d.doc.find(o).is_some() {
                                    d.selection = vec![o];
                                }
                            }
                        }
                        ts.pending_text = None;
                    }
                    Key::Enter => type_text(app, "\n", now),
                    Key::Tab => type_text(app, "\t", now),
                    Key::Backspace => delete_selection_or(app, -1, now),
                    Key::Delete => delete_selection_or(app, 1, now),
                    Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End => move_caret(app, key, modifiers),
                    Key::A if modifiers.command => {
                        if let Some(d) = app.docs.get_mut(ai) {
                            let n = d.text_edit.as_ref().and_then(|te| d.doc.find(te.obj)).and_then(|o| if let ObjKind::Text(t) = &o.kind { Some(t.char_count()) } else { None });
                            if let (Some(n), Some(te)) = (n, d.text_edit.as_mut()) {
                                te.anchor = 0;
                                te.caret = n;
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    if let Some(d) = app.docs.get_mut(ai) {
        if d.text_edit.is_some() || app.ts.pending_text.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}

fn selected_text(app: &VectorApp) -> Option<String> {
    let d = app.doc()?;
    let te = d.text_edit.as_ref()?;
    let ObjKind::Text(t) = &d.doc.find(te.obj)?.kind else { return None };
    let (a, b) = (te.caret.min(te.anchor), te.caret.max(te.anchor));
    if a == b {
        return None;
    }
    Some(t.content.chars().skip(a).take(b - a).collect())
}

/// Insert typed text: creates the text object for a pending position, coalesces undo steps.
fn type_text(app: &mut VectorApp, s: &str, now: f64) {
    let fmt = app.ts.text_format.clone();
    let align = app.ts.text_align;
    let fill = app.ts.new_fill;
    let Some(ai) = app.active else { return };
    let ts = &mut app.ts;
    let d = &mut app.docs[ai];
    if let Some((p, frame)) = ts.pending_text.take() {
        let id = d.doc.new_id();
        let mut t = match frame {
            Some(r) => TextObj::paragraph(s, r.width(), r.height(), fmt),
            None => TextObj::artistic(s, fmt),
        };
        t.align = align;
        let mut style = Style::text_default();
        if let Some(f) = fill {
            style.fill = Fill::Solid(f);
        }
        let o = Object::new(id, ObjKind::Text(t), Affine::translate(p.to_vec2()), style);
        d.add_objects("Create Text", vec![o]);
        let n = s.chars().count();
        d.text_edit = Some(crate::docview::TextEdit { obj: id, caret: n, anchor: n, blink: now });
        ts.text_last_edit = now;
        return;
    }
    let Some(te) = d.text_edit.clone() else { return };
    if now - ts.text_last_edit > 1.0 || s == "\n" {
        d.checkpoint("Edit Text");
    }
    ts.text_last_edit = now;
    let (a, b) = (te.caret.min(te.anchor), te.caret.max(te.anchor));
    if let Some(o) = d.doc.find_mut(te.obj) {
        if let ObjKind::Text(t) = &mut o.kind {
            if b > a {
                t.delete(a, b);
            }
            t.insert(a, s);
        }
    }
    d.history.touch();
    let n = s.chars().count();
    if let Some(te) = d.text_edit.as_mut() {
        te.caret = a + n;
        te.anchor = te.caret;
        te.blink = now;
    }
}

/// Delete the selection, or one character before (-1) / after (1) the caret.
fn delete_selection_or(app: &mut VectorApp, dir: i32, now: f64) {
    let Some(ai) = app.active else { return };
    let ts = &mut app.ts;
    let d = &mut app.docs[ai];
    let Some(te) = d.text_edit.clone() else { return };
    let len = match d.doc.find(te.obj).map(|o| &o.kind) {
        Some(ObjKind::Text(t)) => t.char_count(),
        _ => return,
    };
    let (mut a, mut b) = (te.caret.min(te.anchor), te.caret.max(te.anchor));
    if a == b {
        match dir {
            -1 if a > 0 => a -= 1,
            1 if b < len => b += 1,
            _ => return,
        }
    }
    if now - ts.text_last_edit > 1.0 {
        d.checkpoint("Delete Text");
    }
    ts.text_last_edit = now;
    if let Some(o) = d.doc.find_mut(te.obj) {
        if let ObjKind::Text(t) = &mut o.kind {
            t.delete(a, b);
        }
    }
    d.history.touch();
    if let Some(te) = d.text_edit.as_mut() {
        te.caret = a;
        te.anchor = a;
        te.blink = now;
    }
}

fn move_caret(app: &mut VectorApp, key: egui::Key, mods: egui::Modifiers) {
    use egui::Key;
    let now = app.time();
    let Some(d) = app.doc_mut() else { return };
    let Some(te) = d.text_edit.clone() else { return };
    let Some(ObjKind::Text(t)) = d.doc.find(te.obj).map(|o| &o.kind) else { return };
    let lay = vector_core::text::layout(t);
    let n = t.char_count();
    let chars: Vec<char> = t.content.chars().collect();
    let caret = te.caret.min(n);
    let line_of = |i: usize| lay.carets.get(i).map(|c| c.2).unwrap_or(0);
    let mut c = caret;
    match key {
        Key::ArrowLeft => {
            if mods.command {
                while c > 0 && chars[c - 1].is_whitespace() {
                    c -= 1;
                }
                while c > 0 && !chars[c - 1].is_whitespace() {
                    c -= 1;
                }
            } else if !mods.shift && te.anchor != te.caret {
                c = te.caret.min(te.anchor);
            } else {
                c = c.saturating_sub(1);
            }
        }
        Key::ArrowRight => {
            if mods.command {
                while c < n && !chars[c].is_whitespace() {
                    c += 1;
                }
                while c < n && chars[c].is_whitespace() {
                    c += 1;
                }
            } else if !mods.shift && te.anchor != te.caret {
                c = te.caret.max(te.anchor);
            } else {
                c = (c + 1).min(n);
            }
        }
        Key::Home => {
            let l = line_of(caret);
            while c > 0 && line_of(c - 1) == l && chars.get(c - 1) != Some(&'\n') {
                c -= 1;
            }
        }
        Key::End => {
            let l = line_of(caret);
            while c < n && line_of(c + 1) == l && chars.get(c) != Some(&'\n') {
                c += 1;
            }
        }
        Key::ArrowUp | Key::ArrowDown => {
            let l = line_of(caret) as i64;
            let target = if key == Key::ArrowUp { l - 1 } else { l + 1 };
            let x = lay.carets.get(caret).map(|c| c.0).unwrap_or(0.0);
            if target >= 0 {
                let best = lay.carets.iter().enumerate().filter(|(_, cc)| cc.2 as i64 == target).min_by(|a, b| (a.1 .0 - x).abs().total_cmp(&(b.1 .0 - x).abs()));
                if let Some((i, _)) = best {
                    c = i;
                }
            } else {
                c = 0;
            }
        }
        _ => {}
    }
    if let Some(te) = d.text_edit.as_mut() {
        te.caret = c;
        if !mods.shift {
            te.anchor = c;
        }
        te.blink = now;
    }
}

/// Whether the text tool is about to place text (used by the overlay to draw the insertion caret).
#[allow(dead_code)]
pub fn text_kind_is_paragraph(t: &TextObj) -> bool {
    matches!(t.kind, TextKind::Paragraph { .. })
}
