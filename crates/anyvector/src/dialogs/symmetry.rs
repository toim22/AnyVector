//! Object ▸ Symmetry (live mirror copies), Object ▸ Perspective (drawing on the planes of a
//! perspective field) and Object ▸ Insert ▸ Object (linked and embedded files).

use crate::app::AnyVectorApp;
use crate::docview::DocView;
use egui::{Color32, Context, Painter, Pos2, Stroke};
use kurbo::{Affine, Point, Rect, Vec2};
use std::path::{Path, PathBuf};
use vector_core::codes::Special;
use vector_core::path::{polyline, NodePath};
use vector_core::{geom, Color, Document, Fill, ObjId, ObjKind, Object, Outline, Style};

const LINE: Color32 = Color32::from_rgb(0x1e, 0x90, 0xff);

// ============================================================================== Symmetry

/// The symmetry being edited: its mirror lines and the objects drawn in it.
#[derive(Clone, Debug)]
pub struct SymEdit {
    /// Number of mirror lines (1..12), the center of symmetry and the angle of the first line.
    pub lines: u32,
    pub center: Point,
    pub angle: f64,
    /// The objects of the symmetry; objects with a higher id than `first_new` were drawn while
    /// editing and join it.
    pub ids: Vec<ObjId>,
    pub first_new: ObjId,
    pub page: usize,
    /// The group being edited (a new symmetry has none yet).
    pub group: Option<ObjId>,
    pub show_lines: bool,
}

/// The transforms that make the mirror copies: the rotations and the reflections across each
/// mirror line (`angle` in degrees, 90 = a vertical line).
pub fn symmetry_transforms(lines: u32, center: Point, angle: f64) -> Vec<Affine> {
    let n = lines.clamp(1, 12) as usize;
    let c = center.to_vec2();
    let about = |a: Affine| Affine::translate(c) * a * Affine::translate(-c);
    let mut v = vec![];
    for k in 1..n {
        v.push(about(Affine::rotate(std::f64::consts::TAU * k as f64 / n as f64)));
    }
    for k in 0..n {
        let phi = angle.to_radians() + std::f64::consts::PI * k as f64 / n as f64;
        let (s, co) = (2.0 * phi).sin_cos();
        v.push(about(Affine::new([co, s, s, -co, 0.0, 0.0])));
    }
    v
}

pub fn is_symmetry(o: &Object) -> bool {
    matches!(o.special, Some(Special::Symmetry { .. }))
}

fn selected_special(app: &AnyVectorApp, f: fn(&Object) -> bool) -> Option<ObjId> {
    let d = app.doc()?;
    let [id] = d.selection.as_slice() else { return None };
    d.doc.find(*id).filter(|o| f(o)).map(|o| o.id)
}

pub fn selected_symmetry(app: &AnyVectorApp) -> Option<ObjId> {
    selected_special(app, is_symmetry)
}

/// A symmetry group: the primary objects followed by their mirror copies.
fn symmetry_group(doc: &mut Document, id: ObjId, prim: Vec<Object>, lines: u32, center: Point, angle: f64) -> Object {
    let mut kids = prim.clone();
    for t in symmetry_transforms(lines, center, angle) {
        for p in &prim {
            let mut c = p.clone();
            doc.reassign_ids(&mut c);
            c.apply_transform(t);
            kids.push(c);
        }
    }
    let mut g = Object::new(id, ObjKind::Group(kids), Affine::IDENTITY, Style::default());
    g.name = "Symmetry".into();
    g.special = Some(Special::Symmetry { lines, center: [center.x, center.y], angle, primaries: prim.len() });
    g
}

/// Take the top-level objects that `keep` accepts off a page: the objects and where the first was.
fn take_objects(doc: &mut Document, page: usize, keep: impl Fn(&Object) -> bool) -> (Vec<Object>, Option<(usize, usize)>) {
    let mut out = vec![];
    let mut at = None;
    for l in 0..doc.pages[page].layers.len() {
        let objs = &mut doc.pages[page].layers[l].objects;
        let mut i = 0;
        while i < objs.len() {
            if keep(&objs[i]) {
                at.get_or_insert((l, i));
                out.push(objs.remove(i));
            } else {
                i += 1;
            }
        }
    }
    (out, at)
}

fn insert_at(doc: &mut Document, page: usize, at: (usize, usize), objs: Vec<Object>) {
    let Some(layer) = doc.pages.get_mut(page).and_then(|p| p.layers.get_mut(at.0)) else { return };
    let i = at.1.min(layer.objects.len());
    for (k, o) in objs.into_iter().enumerate() {
        layer.objects.insert(i + k, o);
    }
}

/// Object ▸ Symmetry ▸ Create New Symmetry: the selected objects (or nothing yet) get a mirror line.
pub fn create_symmetry(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    if d.symmetry_edit.is_some() || d.persp_edit.is_some() {
        return;
    }
    let center = d.selection_bounds().map(|b| Point::new(b.x1, b.center().y)).unwrap_or_else(|| d.page_ref().rect().center());
    let first_new = d.doc.new_id();
    d.symmetry_edit = Some(SymEdit { lines: 1, center, angle: 90.0, ids: d.selection.clone(), first_new, page: d.page, group: None, show_lines: true });
    d.cache.key = 0;
}

/// Object ▸ Symmetry ▸ Edit Symmetry: the primary objects can be edited; the copies follow.
pub fn edit_symmetry(app: &mut AnyVectorApp) {
    let Some(id) = selected_symmetry(app) else { return };
    let Some(d) = app.doc_mut() else { return };
    let Some(o) = d.doc.find(id).cloned() else { return };
    let (Some(Special::Symmetry { lines, center, angle, primaries }), ObjKind::Group(kids)) = (&o.special, &o.kind) else { return };
    let prim: Vec<Object> = kids.iter().take(*primaries).cloned().collect();
    let ids: Vec<ObjId> = prim.iter().map(|p| p.id).collect();
    let page = d.page;
    d.modify("Edit Symmetry", |doc, sel| {
        let (_, at) = take_objects(doc, page, |x| x.id == id);
        if let Some(at) = at {
            insert_at(doc, page, at, prim);
        }
        sel.clear();
    });
    let first_new = d.doc.new_id();
    d.symmetry_edit = Some(SymEdit { lines: *lines, center: Point::new(center[0], center[1]), angle: *angle, ids, first_new, page, group: Some(id), show_lines: true });
    d.cache.key = 0;
}

/// The objects of the symmetry being edited.
fn sym_members(d: &DocView, se: &SymEdit) -> Vec<ObjId> {
    d.doc.pages.get(se.page).map(|p| p.layers.iter().flat_map(|l| l.objects.iter()).filter(|o| se.ids.contains(&o.id) || o.id > se.first_new).map(|o| o.id).collect()).unwrap_or_default()
}

/// Object ▸ Symmetry ▸ Finish Editing Symmetry.
pub fn finish_symmetry(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    let Some(se) = d.symmetry_edit.take() else { return };
    let members = sym_members(d, &se);
    d.modify("Finish Editing Symmetry", |doc, sel| {
        let (prim, at) = take_objects(doc, se.page, |o| members.contains(&o.id));
        let Some(at) = at else { return };
        let id = se.group.unwrap_or_else(|| doc.new_id());
        let g = symmetry_group(doc, id, prim, se.lines, se.center, se.angle);
        insert_at(doc, se.page, at, vec![g]);
        *sel = vec![id];
    });
    d.cache.key = 0;
}

/// Object ▸ Symmetry ▸ Remove Symmetry: only the primary objects stay.
pub fn remove_symmetry(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    if let Some(se) = d.symmetry_edit.take() {
        d.selection = sym_members(d, &se);
        d.cache.key = 0;
        return;
    }
    let Some(id) = selected_symmetry(app) else { return };
    let Some(d) = app.doc_mut() else { return };
    let page = d.page;
    d.modify("Remove Symmetry", |doc, sel| {
        let (mut taken, at) = take_objects(doc, page, |x| x.id == id);
        let (Some(at), Some(o)) = (at, taken.pop()) else { return };
        let (Some(Special::Symmetry { primaries, .. }), ObjKind::Group(kids)) = (o.special, o.kind) else { return };
        let prim: Vec<Object> = kids.into_iter().take(primaries).collect();
        *sel = prim.iter().map(|p| p.id).collect();
        insert_at(doc, page, at, prim);
    });
}

/// Object ▸ Symmetry ▸ Break Symmetry Link: the symmetry becomes a regular group.
pub fn break_symmetry(app: &mut AnyVectorApp) {
    let Some(id) = selected_symmetry(app) else { return };
    let Some(d) = app.doc_mut() else { return };
    d.modify("Break Symmetry Link", |doc, _| {
        if let Some(o) = doc.find_mut(id) {
            o.special = None;
            o.name = String::new();
        }
    });
}

/// The mirror lines of a symmetry as segments long enough to cross the view.
fn mirror_lines(se: &SymEdit, len: f64) -> Vec<(Point, Point)> {
    let n = se.lines.clamp(1, 12);
    (0..n)
        .map(|k| {
            let a = (se.angle + 180.0 * k as f64 / n as f64).to_radians();
            let v = Vec2::new(a.cos(), a.sin()) * len;
            (se.center - v, se.center + v)
        })
        .collect()
}

/// Snap to symmetry lines (on while a symmetry is edited) and to perspective lines (on while
/// drawing in perspective): the nearest point of a mirror line or of a line of the plane grid.
pub fn snap_symmetry(d: &DocView, p: Point, reach: f64) -> Option<Point> {
    let lines = if let Some(se) = d.symmetry_edit.as_ref() {
        mirror_lines(se, 1e5)
    } else {
        let pe = d.persp_edit.as_ref()?;
        let f = pe.field;
        let pr = |q: Point| project_point(pe.kind, f, pe.horizon, pe.plane, q);
        (0..=6)
            .flat_map(|i| {
                let t = i as f64 / 6.0;
                let (x, y) = (f.x0 + t * f.width(), f.y0 + t * f.height());
                [(pr(Point::new(x, f.y0)), pr(Point::new(x, f.y1))), (pr(Point::new(f.x0, y)), pr(Point::new(f.x1, y)))]
            })
            .collect()
    };
    lines
        .into_iter()
        .filter(|(a, b)| (*b - *a).hypot2() > 1e-12)
        .map(|(a, b)| {
            let v = b - a;
            a + v * ((p - a).dot(v) / v.hypot2())
        })
        .filter(|q| (*q - p).hypot() <= reach)
        .min_by(|x, y| (*x - p).hypot().total_cmp(&(*y - p).hypot()))
}

// ============================================================================== Perspective

/// Perspective types and planes, as the floating toolbar names them.
pub const PERSP_TYPES: [&str; 4] = ["One-point", "Two-point", "Three-point (worm's-eye)", "Three-point (bird's-eye)"];
pub const PLANES: [&str; 5] = ["Orthographic", "Top", "Left", "Right", "Side"];

/// The perspective group being drawn or edited.
#[derive(Clone, Debug)]
pub struct PerspEdit {
    pub kind: u8,
    /// The perspective field (viewport) and the height of the horizon.
    pub field: Rect,
    pub horizon: f64,
    /// The plane new objects are drawn on.
    pub plane: u8,
    /// The flat objects and their planes.
    pub planes: Vec<(ObjId, u8)>,
    /// Objects with a higher id appeared since the last frame: they go on the current plane.
    pub last_seen: ObjId,
    pub page: usize,
    pub group: Option<ObjId>,
}

pub fn is_perspective(o: &Object) -> bool {
    matches!(o.special, Some(Special::Perspective { .. }))
}

pub fn selected_perspective(app: &AnyVectorApp) -> Option<ObjId> {
    selected_special(app, is_perspective)
}

fn lerp(a: Point, b: Point, t: f64) -> Point {
    a + (b - a) * t
}

/// Where the lines a→b and c→d cross.
fn cross(a: Point, b: Point, c: Point, d: Point) -> Point {
    let (r, s) = (b - a, d - c);
    let den = r.cross(s);
    if den.abs() < 1e-12 {
        return lerp(b, d, 0.5);
    }
    a + r * ((c - a).cross(s) / den)
}

/// The vanishing points of a field: left, right and (three-point) vertical.
fn vanishing_points(kind: u8, f: Rect, horizon: f64) -> (Point, Point, Option<Point>) {
    let (w, h) = (f.width(), f.height());
    match kind {
        0 => {
            let v = Point::new(f.center().x, horizon);
            (v, v, None)
        }
        _ => (
            Point::new(f.x0 - w * 0.5, horizon),
            Point::new(f.x1 + w * 0.5, horizon),
            match kind {
                2 => Some(Point::new(f.center().x, f.y0 - h * 1.5)),
                3 => Some(Point::new(f.center().x, f.y1 + h * 1.5)),
                _ => None,
            },
        ),
    }
}

/// The quadrilateral a plane makes of the field (corners in the order top-left, top-right,
/// bottom-right, bottom-left of the flat drawing); `None` for the orthographic plane.
pub fn plane_quad(kind: u8, f: Rect, horizon: f64, plane: u8) -> Option<[Point; 4]> {
    let (tl, tr, br, bl) = (Point::new(f.x0, f.y0), Point::new(f.x1, f.y0), Point::new(f.x1, f.y1), Point::new(f.x0, f.y1));
    let (vl, vr, v3) = vanishing_points(kind, f, horizon);
    let k = 0.55;
    let (ft, fb) = (Point::new(f.center().x, f.y0), Point::new(f.center().x, f.y1));
    let mut q = match (kind, plane) {
        (_, 0) => return None,
        (0, 1) => [lerp(bl, vl, k), lerp(br, vl, k), br, bl],
        (0, 2) => [tl, lerp(tl, vl, k), lerp(bl, vl, k), bl],
        (0, 3) => [lerp(tr, vl, k), tr, br, lerp(br, vl, k)],
        (0, _) => [lerp(tl, vl, k * 0.5), lerp(tr, vl, k * 0.5), lerp(br, vl, k * 0.5), lerp(bl, vl, k * 0.5)],
        (_, 1) => {
            let (left, right) = (lerp(fb, vl, k), lerp(fb, vr, k));
            [cross(left, vr, right, vl), right, fb, left]
        }
        (_, 3) => [ft, lerp(ft, vr, k), lerp(fb, vr, k), fb],
        _ => [lerp(ft, vl, k), ft, fb, lerp(fb, vl, k)],
    };
    // three-point fields: the verticals of the walls meet at the third vanishing point
    if let (Some(v3), true) = (v3, plane >= 2) {
        let (a, b) = if kind == 2 { (0, 1) } else { (3, 2) };
        q[a] = lerp(q[a], v3, 0.2);
        q[b] = lerp(q[b], v3, 0.2);
    }
    Some(q)
}

/// The projective map from the unit square to a quadrilateral.
fn square_to_quad(q: [Point; 4]) -> [f64; 8] {
    let (x0, y0, x1, y1, x2, y2, x3, y3) = (q[0].x, q[0].y, q[1].x, q[1].y, q[2].x, q[2].y, q[3].x, q[3].y);
    let (dx1, dx2, dx3) = (x1 - x2, x3 - x2, x0 - x1 + x2 - x3);
    let (dy1, dy2, dy3) = (y1 - y2, y3 - y2, y0 - y1 + y2 - y3);
    let (g, h) = if dx3.abs() < 1e-12 && dy3.abs() < 1e-12 {
        (0.0, 0.0)
    } else {
        let den = dx1 * dy2 - dx2 * dy1;
        if den.abs() < 1e-12 {
            (0.0, 0.0)
        } else {
            ((dx3 * dy2 - dx2 * dy3) / den, (dx1 * dy3 - dx3 * dy1) / den)
        }
    };
    [x1 - x0 + g * x1, x3 - x0 + h * x3, x0, y1 - y0 + g * y1, y3 - y0 + h * y3, y0, g, h]
}

/// A point of the flat field on a plane.
pub fn project_point(kind: u8, f: Rect, horizon: f64, plane: u8, p: Point) -> Point {
    let Some(q) = plane_quad(kind, f, horizon, plane) else { return p };
    let m = square_to_quad(q);
    let (u, v) = ((p.x - f.x0) / f.width().max(1e-9), (p.y - f.y0) / f.height().max(1e-9));
    let w = m[6] * u + m[7] * v + 1.0;
    let w = if w.abs() < 1e-9 { 1e-9 } else { w };
    Point::new((m[0] * u + m[1] * v + m[2]) / w, (m[3] * u + m[4] * v + m[5]) / w)
}

/// An object drawn flat, as it looks on its plane.
fn project_object(kind: u8, f: Rect, horizon: f64, plane: u8, o: &Object, id: ObjId) -> Object {
    if plane == 0 {
        let mut c = o.clone();
        c.id = id;
        return c;
    }
    let mut np = NodePath::new();
    for (pts, closed) in geom::flatten_polylines(&o.world_path(), 0.25) {
        let mapped: Vec<Point> = pts.iter().map(|p| project_point(kind, f, horizon, plane, *p)).collect();
        if mapped.len() >= 2 {
            np.subpaths.extend(polyline(&mapped, closed).subpaths);
        }
    }
    let mut c = Object::new(id, ObjKind::Path(np), Affine::IDENTITY, o.style.clone());
    c.name = o.name.clone();
    c
}

/// Object ▸ Perspective ▸ Draw in Perspective: a perspective field over the page; objects drawn
/// go on the chosen plane.
pub fn draw_in_perspective(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    if d.persp_edit.is_some() || d.symmetry_edit.is_some() {
        return;
    }
    d.selection.clear();
    let f = d.page_ref().rect();
    let last_seen = d.doc.new_id();
    d.persp_edit = Some(PerspEdit { kind: 0, field: f, horizon: f.y0 + f.height() * 0.4, plane: 1, planes: vec![], last_seen, page: d.page, group: None });
    d.cache.key = 0;
}

/// Object ▸ Perspective ▸ Edit Perspective Group: the objects are flat again until Finish.
pub fn edit_perspective(app: &mut AnyVectorApp) {
    let Some(id) = selected_perspective(app) else { return };
    let Some(d) = app.doc_mut() else { return };
    let Some(Special::Perspective { kind, field, horizon, flats }) = d.doc.find(id).and_then(|o| o.special.clone()) else { return };
    let page = d.page;
    let planes: Vec<(ObjId, u8)> = flats.iter().map(|(p, o)| (o.id, *p)).collect();
    d.modify("Edit Perspective Group", |doc, sel| {
        let (_, at) = take_objects(doc, page, |x| x.id == id);
        if let Some(at) = at {
            insert_at(doc, page, at, flats.into_iter().map(|(_, o)| o).collect());
        }
        sel.clear();
    });
    let last_seen = d.doc.new_id();
    let field = Rect::new(field[0], field[1], field[2], field[3]);
    d.persp_edit = Some(PerspEdit { kind, field, horizon, plane: 1, planes, last_seen, page, group: Some(id) });
    d.cache.key = 0;
}

/// Finish drawing in perspective: the flat objects become the perspective group.
pub fn finish_perspective(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    let Some(pe) = d.persp_edit.take() else { return };
    d.modify("Draw in Perspective", |doc, sel| {
        let (flat, at) = take_objects(doc, pe.page, |o| pe.planes.iter().any(|(id, _)| *id == o.id));
        let Some(at) = at else { return };
        let mut kids = vec![];
        let mut flats = vec![];
        for o in flat {
            let plane = pe.planes.iter().find(|(id, _)| *id == o.id).map(|x| x.1).unwrap_or(0);
            let nid = doc.new_id();
            kids.push(project_object(pe.kind, pe.field, pe.horizon, plane, &o, nid));
            flats.push((plane, o));
        }
        let id = pe.group.unwrap_or_else(|| doc.new_id());
        let mut g = Object::new(id, ObjKind::Group(kids), Affine::IDENTITY, Style::default());
        g.name = "Perspective Group".into();
        let f = pe.field;
        g.special = Some(Special::Perspective { kind: pe.kind, field: [f.x0, f.y0, f.x1, f.y1], horizon: pe.horizon, flats });
        insert_at(doc, pe.page, at, vec![g]);
        *sel = vec![id];
    });
    d.cache.key = 0;
}

/// Right-click ▸ Move to Plane: the selected objects go on another plane.
pub fn move_to_plane(app: &mut AnyVectorApp, plane: u8) {
    let Some(d) = app.doc_mut() else { return };
    let sel = d.selection.clone();
    let Some(pe) = d.persp_edit.as_mut() else { return };
    for id in sel {
        match pe.planes.iter_mut().find(|(x, _)| *x == id) {
            Some(e) => e.1 = plane,
            None => pe.planes.push((id, plane)),
        }
    }
    d.cache.key = 0;
}

// ============================================================================== floating toolbars

/// The floating toolbars of symmetry and perspective editing, and the bookkeeping that puts new
/// objects on the current perspective plane.
pub fn floating_bars(app: &mut AnyVectorApp, ctx: &Context) {
    let Some(ai) = app.active.filter(|a| *a < app.docs.len()) else { return };
    let pos = app.docs[ai].canvas_rect.min + egui::vec2(12.0, 12.0);
    let (unit, dpi) = (app.docs[ai].doc.settings.unit, app.docs[ai].doc.settings.dpi);
    if app.docs[ai].symmetry_edit.is_some() {
        let mut finish = false;
        let mut remove = false;
        egui::Area::new(egui::Id::unique("symmetry_bar")).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let d = &mut app.docs[ai];
                    let Some(se) = d.symmetry_edit.as_mut() else { return };
                    let before = (se.lines, se.angle, se.center, se.show_lines);
                    ui.label("Mirror lines:");
                    ui.add(egui::DragValue::new(&mut se.lines).range(1..=12));
                    ui.label("Angle of rotation:");
                    ui.add(egui::DragValue::new(&mut se.angle).speed(0.5).suffix("°"));
                    let k = 1.0 / unit.points_per_unit(dpi).max(1e-9);
                    let (mut x, mut y) = (se.center.x * k, se.center.y * k);
                    ui.label("X:");
                    ui.add(egui::DragValue::new(&mut x).speed(0.1).suffix(format!(" {}", unit.suffix())));
                    ui.label("Y:");
                    ui.add(egui::DragValue::new(&mut y).speed(0.1).suffix(format!(" {}", unit.suffix())));
                    se.center = Point::new(x / k, y / k);
                    ui.checkbox(&mut se.show_lines, "Show symmetry lines");
                    if before != (se.lines, se.angle, se.center, se.show_lines) {
                        d.cache.key = 0;
                    }
                    ui.separator();
                    finish = ui.button("Finish").clicked();
                    remove = ui.button("Remove").on_hover_text("Remove symmetry: keep the objects you drew").clicked();
                });
            });
        });
        if finish {
            finish_symmetry(app);
        } else if remove {
            remove_symmetry(app);
        }
    }
    if app.docs[ai].persp_edit.is_some() {
        // new objects go on the current plane
        {
            let d = &mut app.docs[ai];
            let page = d.page;
            let fresh: Vec<ObjId> = match d.persp_edit.as_ref() {
                Some(pe) if pe.page == page => d.page_ref().layers.iter().flat_map(|l| l.objects.iter()).filter(|o| o.id > pe.last_seen).map(|o| o.id).collect(),
                _ => vec![],
            };
            if let Some(pe) = d.persp_edit.as_mut() {
                for id in fresh {
                    if !pe.planes.iter().any(|(x, _)| *x == id) {
                        pe.planes.push((id, pe.plane));
                    }
                    pe.last_seen = pe.last_seen.max(id);
                }
            }
        }
        let mut finish = false;
        egui::Area::new(egui::Id::unique("perspective_bar")).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let d = &mut app.docs[ai];
                    let page_rect = d.page_ref().rect();
                    let Some(pe) = d.persp_edit.as_mut() else { return };
                    let before = (pe.kind, pe.plane, pe.field, pe.horizon);
                    // the type cannot change once objects are drawn
                    ui.add_enabled_ui(pe.planes.is_empty(), |ui| {
                        egui::ComboBox::from_id_salt("persp_type").selected_text(PERSP_TYPES[pe.kind as usize % 4]).show_ui(ui, |ui| {
                            for (i, n) in PERSP_TYPES.iter().enumerate() {
                                ui.selectable_value(&mut pe.kind, i as u8, *n);
                            }
                        });
                    });
                    ui.separator();
                    for (i, n) in PLANES.iter().enumerate() {
                        // the side plane is for one-point perspective
                        if i == 4 && pe.kind != 0 {
                            continue;
                        }
                        ui.selectable_value(&mut pe.plane, i as u8, *n);
                    }
                    ui.separator();
                    let k = 1.0 / unit.points_per_unit(dpi).max(1e-9);
                    let mut hz = (pe.horizon - pe.field.y0) * k;
                    ui.label("Horizon:");
                    ui.add(egui::DragValue::new(&mut hz).speed(0.1).suffix(format!(" {}", unit.suffix())));
                    pe.horizon = pe.field.y0 + hz / k;
                    if ui.button("Fill page").on_hover_text("The perspective field covers the page").clicked() {
                        let h = (pe.horizon - pe.field.y0) / pe.field.height().max(1e-9);
                        pe.field = page_rect;
                        pe.horizon = page_rect.y0 + h * page_rect.height();
                    }
                    if before != (pe.kind, pe.plane, pe.field, pe.horizon) {
                        d.cache.key = 0;
                    }
                    ui.separator();
                    finish = ui.button("Finish").clicked();
                });
            });
        });
        if finish {
            finish_perspective(app);
        }
    }
}

/// Overlay of symmetry and perspective editing: mirror lines and copies, the perspective field
/// and how the flat objects look on their planes.
pub fn draw_overlay(painter: &Painter, d: &DocView) {
    let to = |p: Point| -> Pos2 { d.to_screen(p) };
    if let Some(se) = &d.symmetry_edit {
        if se.show_lines {
            for (a, b) in mirror_lines(se, 1e4) {
                painter.extend(egui::Shape::dashed_line(&[to(a), to(b)], Stroke::new(1.0, LINE), 6.0, 4.0));
            }
            painter.circle_stroke(to(se.center), 5.0, Stroke::new(1.5, LINE));
        }
        let ts = symmetry_transforms(se.lines, se.center, se.angle);
        for id in sym_members(d, se) {
            let Some(o) = d.doc.find(id) else { continue };
            let wp = o.world_path();
            for t in &ts {
                crate::tools::overlay::stroke_bez(painter, d, &(*t * wp.clone()), Stroke::new(1.0, Color32::from_rgb(0x60, 0x60, 0x60)));
            }
        }
    }
    if let Some(pe) = &d.persp_edit {
        let f = pe.field;
        // horizon and vanishing points
        painter.line_segment([to(Point::new(f.x0 - f.width(), pe.horizon)), to(Point::new(f.x1 + f.width(), pe.horizon))], Stroke::new(1.0, Color32::from_rgb(0xe0, 0x40, 0x40)));
        let (vl, vr, v3) = vanishing_points(pe.kind, f, pe.horizon);
        for v in [Some(vl), Some(vr), v3].into_iter().flatten() {
            painter.circle_stroke(to(v), 4.0, Stroke::new(1.5, Color32::from_rgb(0xe0, 0x40, 0x40)));
        }
        // the planes as grids of perspective lines; the current one stronger
        for plane in 1..=4u8 {
            if plane == 4 && pe.kind != 0 {
                continue;
            }
            let Some(q) = plane_quad(pe.kind, f, pe.horizon, plane) else { continue };
            let strong = plane == pe.plane;
            let st = Stroke::new(if strong { 1.2 } else { 0.6 }, if strong { LINE } else { Color32::from_rgba_unmultiplied(0x1e, 0x90, 0xff, 90) });
            for i in 0..=6 {
                let t = i as f64 / 6.0;
                for (a, b) in [(Point::new(f.x0 + t * f.width(), f.y0), Point::new(f.x0 + t * f.width(), f.y1)), (Point::new(f.x0, f.y0 + t * f.height()), Point::new(f.x1, f.y0 + t * f.height()))] {
                    let (pa, pb) = (project_point(pe.kind, f, pe.horizon, plane, a), project_point(pe.kind, f, pe.horizon, plane, b));
                    painter.line_segment([to(pa), to(pb)], st);
                }
            }
            let _ = q;
        }
        painter.rect_stroke(d.to_screen_rect(f), 0.0, Stroke::new(1.0, Color32::from_gray(150)), egui::StrokeKind::Middle);
        // the flat objects as they will look
        for (id, plane) in &pe.planes {
            let Some(o) = d.doc.find(*id) else { continue };
            let p = project_object(pe.kind, f, pe.horizon, *plane, o, 0);
            crate::tools::overlay::stroke_bez(painter, d, &p.world_path(), Stroke::new(1.5, Color32::from_rgb(0x20, 0xa0, 0x40)));
        }
    }
}

// ============================================================================== Insert ▸ Object

/// A document type Create New makes: the types Windows can create empty (the New menu of the
/// desktop), opened in their program once inserted.
#[derive(Clone, Debug)]
pub struct NewType {
    pub name: String,
    pub ext: String,
    how: NewHow,
}

#[derive(Clone, Debug)]
enum NewHow {
    Empty,
    Template(PathBuf),
    Data(Vec<u8>),
}

pub fn new_types() -> &'static [NewType] {
    static TYPES: std::sync::OnceLock<Vec<NewType>> = std::sync::OnceLock::new();
    TYPES.get_or_init(|| {
        let mut v = load_new_types();
        if v.is_empty() {
            v = vec![
                NewType { name: "Bitmap image".into(), ext: ".bmp".into(), how: NewHow::Empty },
                NewType { name: "Rich Text Document".into(), ext: ".rtf".into(), how: NewHow::Data(b"{\\rtf1}".to_vec()) },
                NewType { name: "Text Document".into(), ext: ".txt".into(), how: NewHow::Empty },
            ];
        }
        v.sort_by_key(|t| t.name.to_lowercase());
        v.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
        v
    })
}

/// A template file of the New menu: absolute, or in the Windows and user template folders.
fn template_path(f: &str) -> Option<PathBuf> {
    let mut s = f.to_string();
    for (k, v) in std::env::vars() {
        s = s.replace(&format!("%{k}%"), &v).replace(&format!("%{}%", k.to_uppercase()), &v);
    }
    let p = PathBuf::from(&s);
    if p.is_absolute() {
        return p.exists().then_some(p);
    }
    let mut dirs = vec![];
    if let Ok(w) = std::env::var("WINDIR") {
        dirs.push(PathBuf::from(w).join("ShellNew"));
    }
    if let Some(t) = dirs::template_dir() {
        dirs.push(t);
    }
    if let Ok(pd) = std::env::var("ProgramData") {
        dirs.push(PathBuf::from(pd).join(r"Microsoft\Windows\Templates"));
    }
    dirs.into_iter().map(|d| d.join(&s)).find(|p| p.exists())
}

#[cfg(windows)]
fn load_new_types() -> Vec<NewType> {
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{RegEnumKeyExW, RegGetValueW, HKEY_CLASSES_ROOT, REG_BINARY, REG_ROUTINE_FLAGS, REG_VALUE_TYPE, RRF_RT_ANY};
    let get = |sub: &str, value: &str, flags: REG_ROUTINE_FLAGS| -> Option<(Vec<u8>, REG_VALUE_TYPE)> {
        let (s, v) = (HSTRING::from(sub), HSTRING::from(value));
        let mut size = 0u32;
        let mut ty = REG_VALUE_TYPE::default();
        if unsafe { RegGetValueW(HKEY_CLASSES_ROOT, &s, &v, flags, Some(&mut ty), None, Some(&mut size)) } != ERROR_SUCCESS {
            return None;
        }
        let mut buf = vec![0u8; size as usize + 2];
        let mut size = buf.len() as u32;
        if unsafe { RegGetValueW(HKEY_CLASSES_ROOT, &s, &v, flags, Some(&mut ty), Some(buf.as_mut_ptr().cast()), Some(&mut size)) } != ERROR_SUCCESS {
            return None;
        }
        buf.truncate(size as usize);
        Some((buf, ty))
    };
    let text = |b: &[u8]| -> String {
        let w: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let n = w.iter().position(|c| *c == 0).unwrap_or(w.len());
        String::from_utf16_lossy(&w[..n]).trim().to_string()
    };
    // strings and expandable strings
    let sz = REG_ROUTINE_FLAGS(0x0002 | 0x0004);
    let mut out = vec![];
    let mut seen_ext = false;
    for i in 0..100_000u32 {
        let mut name = [0u16; 512];
        let mut len = name.len() as u32;
        let r = unsafe { RegEnumKeyExW(HKEY_CLASSES_ROOT, i, Some(PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None) };
        if r == ERROR_MORE_DATA {
            continue;
        }
        if r != ERROR_SUCCESS {
            break;
        }
        let ext = String::from_utf16_lossy(&name[..len as usize]);
        if !ext.starts_with('.') {
            // the keys come in order: the extensions are together
            if seen_ext {
                break;
            }
            continue;
        }
        seen_ext = true;
        let progid = get(&ext, "", sz).map(|(b, _)| text(&b)).filter(|s| !s.is_empty());
        let mut keys = vec![format!(r"{ext}\ShellNew")];
        if let Some(p) = &progid {
            keys.insert(0, format!(r"{ext}\{p}\ShellNew"));
        }
        for k in keys {
            let how = if get(&k, "NullFile", RRF_RT_ANY).is_some() {
                Some(NewHow::Empty)
            } else if let Some((b, _)) = get(&k, "FileName", sz) {
                template_path(&text(&b)).map(NewHow::Template)
            } else if let Some((b, ty)) = get(&k, "Data", RRF_RT_ANY) {
                Some(NewHow::Data(if ty == REG_BINARY { b } else { text(&b).into_bytes() }))
            } else {
                None
            };
            let Some(how) = how else { continue };
            let name = progid.as_deref().and_then(|p| get(p, "", sz)).map(|(b, _)| text(&b)).filter(|s| !s.is_empty() && !s.starts_with('@'));
            let name = name.unwrap_or_else(|| format!("{} File", ext.trim_start_matches('.').to_uppercase()));
            out.push(NewType { name, ext: ext.clone(), how });
            break;
        }
    }
    out
}

#[cfg(not(windows))]
fn load_new_types() -> Vec<NewType> {
    vec![]
}

/// Object ▸ Insert ▸ Object.
#[derive(Clone, Debug, Default)]
pub struct InsertObject {
    pub from_file: bool,
    pub kind: usize,
    pub path: String,
    pub link: bool,
    pub as_icon: bool,
}

pub fn insert_object_dialog(app: &mut AnyVectorApp, ctx: &Context) {
    let Some(mut st) = app.dialogs.insert_object.take() else { return };
    let types = new_types();
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = super::window(ctx, "insert_object", "Insert Object", egui::vec2(456.0, 300.0), |ui| {
        let body = ui.max_rect();
        ui.add_space(10.0);
        ui.horizontal_top(|ui| {
            // Create New / Create from File
            ui.vertical(|ui| {
                ui.set_width(112.0);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.add_space(14.0);
                ui.radio_value(&mut st.from_file, false, "Create New");
                ui.add_space(10.0);
                ui.radio_value(&mut st.from_file, true, "Create from File");
            });
            ui.vertical(|ui| {
                ui.set_width(190.0);
                if st.from_file {
                    ui.label("File:");
                    ui.add(egui::TextEdit::singleline(&mut st.path).desired_width(186.0));
                    ui.horizontal(|ui| {
                        if super::button(ui, "Browse...", true) {
                            if let Some(p) = rfd::FileDialog::new().set_title("Browse").pick_file() {
                                st.path = p.display().to_string();
                            }
                        }
                        ui.checkbox(&mut st.link, "Link");
                    });
                } else {
                    ui.label("Object Type:");
                    egui::Frame::new().fill(Color32::from_gray(0x1e)).stroke(Stroke::new(1.0, Color32::from_gray(80))).inner_margin(2.0).show(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(132.0).auto_shrink([false, false]).show(ui, |ui| {
                            ui.set_width(180.0);
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                            for (i, t) in types.iter().enumerate() {
                                ui.selectable_value(&mut st.kind, i, &t.name).on_hover_text(&t.ext);
                            }
                        });
                    });
                }
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                let can = !st.from_file || !st.path.trim().is_empty();
                ok = ui.add_enabled(can, egui::Button::new("OK").min_size(egui::vec2(98.0, 24.0))).clicked();
                ui.add_space(2.0);
                cancel = ui.add(egui::Button::new("Cancel").min_size(egui::vec2(98.0, 24.0))).clicked();
                ui.add_space(38.0);
                ui.checkbox(&mut st.as_icon, "Display As Icon");
            });
        });
        let kind = types.get(st.kind).map(|t| t.name.as_str()).unwrap_or("");
        let icon = if st.as_icon { " It will be displayed as an icon." } else { "" };
        let result = match (st.from_file, st.link) {
            (false, _) => format!("Inserts a new {kind} object into your document.{icon}"),
            (true, false) => format!("Inserts the contents of the file as an object into your document so that you may activate it using the program which created it.{icon}"),
            (true, true) => format!("Inserts a picture of the file contents into your document. The picture will be linked to the file so that changes to the file will be reflected in your document.{icon}"),
        };
        let r = egui::Rect::from_min_max(egui::pos2(body.left(), body.bottom() - 82.0), egui::pos2(body.right() - 110.0, body.bottom() - 12.0));
        ui.painter().rect_stroke(r, 2.0, Stroke::new(1.0, Color32::from_gray(80)), egui::StrokeKind::Inside);
        let label = egui::Rect::from_min_size(egui::pos2(r.left() + 6.0, r.top() - 8.0), egui::vec2(44.0, 16.0));
        ui.painter().rect_filled(label, 0.0, crate::theme::BG);
        ui.painter().text(egui::pos2(r.left() + 9.0, r.top()), egui::Align2::LEFT_CENTER, "Result", egui::FontId::proportional(12.0), crate::theme::TEXT);
        ui.scope_builder(egui::UiBuilder::new().max_rect(r.shrink2(egui::vec2(12.0, 14.0))), |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(result).wrap());
            });
        });
    });
    if ok {
        let r = if st.from_file {
            insert_ole(app, Path::new(st.path.trim()), st.link, st.as_icon, false)
        } else {
            new_ole(app, st.kind, st.as_icon)
        };
        if let Err(e) = r {
            app.dialogs.message_box = Some(("Insert Object".into(), e));
        }
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.insert_object = Some(st);
    }
}

/// Create New: a new file of the type, embedded and opened in its program.
fn new_ole(app: &mut AnyVectorApp, kind: usize, as_icon: bool) -> Result<(), String> {
    let types = new_types();
    let t = types.get(kind).or(types.first()).ok_or("No object types are available.")?;
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = ole_dir().join(format!("new-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}{}", t.name, t.ext));
    match &t.how {
        NewHow::Empty => std::fs::write(&path, b""),
        NewHow::Data(b) => std::fs::write(&path, b),
        NewHow::Template(f) => std::fs::copy(f, &path).map(|_| ()),
    }
    .map_err(|e| format!("Could not create {}:\n{e}", path.display()))?;
    insert_ole(app, &path, false, as_icon, true)
}

fn ole_dir() -> PathBuf {
    std::env::temp_dir().join("AnyVector OLE")
}

/// What an object shows: the file's contents when AnyVector can read it, else an icon with its name.
fn ole_appearance(doc: &mut Document, path: &Path, name: &str, as_icon: bool) -> Vec<Object> {
    if !as_icon {
        if let Ok(objs) = vector_io::import(doc, path) {
            if !objs.is_empty() {
                return objs;
            }
        }
    }
    let frame = Object::new(
        doc.new_id(),
        ObjKind::Rect(vector_core::object::RectShape { w: 96.0, h: 72.0, radii: [4.0; 4], corner: Default::default() }),
        Affine::IDENTITY,
        Style { fill: Fill::Solid(Color::rgb(0xf2, 0xf2, 0xf2)), outline: Outline { width: 0.75, color: Some(Color::rgb(0x80, 0x80, 0x80)), ..Outline::default() }, ..Default::default() },
    );
    let fmt = vector_core::text::CharFormat { size: 8.0, ..Default::default() };
    let mut label = Object::new(doc.new_id(), ObjKind::Text(vector_core::text::TextObj::artistic(name, fmt)), Affine::translate((6.0, 58.0)), Style { fill: Fill::Solid(Color::BLACK), ..Default::default() });
    label.name = name.into();
    vec![frame, label]
}

/// Place a linked or embedded object (OLE) centered on the page.
fn insert_ole(app: &mut AnyVectorApp, path: &Path, linked: bool, as_icon: bool, open: bool) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read {}:\n{e}", path.display()))?;
    if !linked && bytes.len() > 64 << 20 {
        return Err("The file is too large to embed; insert it as a link.".into());
    }
    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let data = if linked { String::new() } else { base64(&bytes) };
    if !app.has_doc() {
        app.new_default_document();
    }
    let Some(d) = app.doc_mut() else { return Ok(()) };
    let kids = ole_appearance(&mut d.doc, path, &name, as_icon);
    let id = d.doc.new_id();
    let mut g = Object::new(id, ObjKind::Group(kids), Affine::IDENTITY, Style::default());
    g.name = name.clone();
    g.special = Some(Special::Ole { name, path: path.display().to_string(), linked, data, icon: as_icon });
    let bb = d.doc.bounds_of(&g);
    g.apply_transform(Affine::translate(d.page_ref().rect().center() - bb.center()));
    d.add_objects("Insert Object", vec![g]);
    if open {
        open_ole(app, id);
    }
    Ok(())
}

static WATCH: std::sync::Mutex<Vec<(ObjId, PathBuf, Option<std::time::SystemTime>)>> = std::sync::Mutex::new(Vec::new());

fn modified(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// Double-click an inserted object: it opens in its program (an embedded one from a copy).
/// Saving it there updates the object.
pub fn open_ole(app: &mut AnyVectorApp, id: ObjId) {
    let Some(Special::Ole { name, path, linked, data, .. }) = app.doc().and_then(|d| d.doc.find(id)).and_then(|o| o.special.clone()) else { return };
    let file = if linked {
        PathBuf::from(path)
    } else {
        let dir = ole_dir().join(id.to_string());
        let f = dir.join(&name);
        if std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&f, unbase64(&data))).is_err() {
            app.flash("Could not open the object".to_string());
            return;
        }
        f
    };
    if !file.exists() {
        app.dialogs.message_box = Some(("Insert Object".into(), format!("The linked file was not found:\n{}", file.display())));
        return;
    }
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(&file).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(&file).spawn();
    if let Ok(mut w) = WATCH.lock() {
        w.retain(|x| x.0 != id);
        w.push((id, file.clone(), modified(&file)));
    }
}

/// Objects opened in their programs: when the file is saved there the object takes the changes.
pub fn poll_ole(app: &mut AnyVectorApp, ctx: &Context) {
    let Ok(mut w) = WATCH.lock() else { return };
    if w.is_empty() {
        return;
    }
    ctx.request_repaint_after(std::time::Duration::from_secs(1));
    let changed: Vec<(ObjId, PathBuf)> = w
        .iter_mut()
        .filter_map(|(id, p, t)| {
            let now = modified(p);
            (now.is_some() && now != *t).then(|| {
                *t = now;
                (*id, p.clone())
            })
        })
        .collect();
    drop(w);
    for (id, p) in changed {
        let Ok(bytes) = std::fs::read(&p) else { continue };
        let Some(d) = app.doc_mut() else { continue };
        let Some(o) = d.doc.find(id) else { continue };
        let Some(Special::Ole { name, path, linked, icon, .. }) = o.special.clone() else { continue };
        let bb = d.doc.bounds_of(o);
        let kids = ole_appearance(&mut d.doc, &p, &name, icon);
        d.modify("Update Object", |doc, _| {
            let Some(o) = doc.find_mut(id) else { return };
            o.kind = ObjKind::Group(kids);
            o.special = Some(Special::Ole { name, path, linked, data: if linked { String::new() } else { base64(&bytes) }, icon });
            let nb = doc.find(id).map(|o| doc.bounds_of(o)).unwrap_or(bb);
            if let Some(o) = doc.find_mut(id) {
                o.apply_transform(Affine::translate(bb.center() - nb.center()));
            }
        });
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            s.push(if k <= c.len() { B64[(n >> (18 - 6 * k)) as usize & 63] as char } else { '=' });
        }
    }
    s
}

fn unbase64(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0);
    for ch in s.bytes() {
        let Some(v) = B64.iter().position(|x| *x == ch) else { continue };
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        for s in [&b""[..], b"a", b"ab", b"abc", b"hello world!"] {
            assert_eq!(unbase64(&base64(s)), s);
        }
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
    }

    #[test]
    fn mirror_copies() {
        // one vertical line through x = 100: (90, 0) mirrors to (110, 0)
        let t = symmetry_transforms(1, Point::new(100.0, 0.0), 90.0);
        assert_eq!(t.len(), 1);
        assert!(((t[0] * Point::new(90.0, 5.0)) - Point::new(110.0, 5.0)).hypot() < 1e-9);
        // three lines: two rotations and three reflections
        assert_eq!(symmetry_transforms(3, Point::ZERO, 90.0).len(), 5);
    }

    #[test]
    fn planes_keep_their_corners() {
        let f = Rect::new(0.0, 0.0, 100.0, 100.0);
        for kind in 0..4u8 {
            for plane in 1..=4u8 {
                let Some(q) = plane_quad(kind, f, 40.0, plane) else { continue };
                let corners = [Point::new(0.0, 0.0), Point::new(100.0, 0.0), Point::new(100.0, 100.0), Point::new(0.0, 100.0)];
                for (c, want) in corners.iter().zip(q) {
                    assert!((project_point(kind, f, 40.0, plane, *c) - want).hypot() < 1e-6, "kind {kind} plane {plane}");
                }
            }
        }
        // the floor of a one-point field narrows toward the horizon
        let back = project_point(0, f, 40.0, 1, Point::new(0.0, 0.0));
        assert!(back.x > 0.0 && back.y > 40.0 - 1e-9);
    }
}
