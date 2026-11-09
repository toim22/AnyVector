//! The Coordinates docker: draw or change objects numerically (rectangle, ellipse, polygon, star,
//! complex star, 2-point line, multi-point curve), with Create object and Replace object.

use super::{apply_bar, bold_title, form_row, no_doc, section, spin};
use crate::app::AnyVectorApp;
use crate::theme;
use egui::{pos2, vec2, Sense, Stroke, Ui};
use kurbo::{Affine, Point, Rect, Vec2};
use vector_core::object::{EllipseKind, EllipseShape, PolygonKind, PolygonShape, RectShape};
use vector_core::path::polyline;
use vector_core::{ObjKind, Object};

/// Where a point clicked on the canvas (or a length/angle measured there) goes.
pub const SLOT_POS: u8 = 10;
pub const SLOT_LL: u8 = 11;
pub const SLOT_UR: u8 = 12;
pub const SLOT_W: u8 = 13;
pub const SLOT_H: u8 = 14;
pub const SLOT_ANGLE: u8 = 15;
pub const SLOT_START: u8 = 16;
pub const SLOT_END: u8 = 17;
pub const SLOT_SIZE: u8 = 18;
pub const SLOT_SIDE: u8 = 19;

const TABS: [(&str, &str, &str); 7] = [
    ("rectangle", "Rectangle", "Rectangle"),
    ("ellipse", "Ellipse", "Ellipses"),
    ("polygon", "Polygon", "Polygon"),
    ("star", "Star", "Star"),
    ("complex_star", "Complex Star", "Star"),
    ("two_point_line", "2-Point Line", "2-Points Line"),
    ("polyline", "Multi-Point Curve", "Multi-Point Curve"),
];

/// What the Coordinates docker holds (in document points; shown in ruler coordinates).
pub struct CoUi {
    pub tab: usize,
    pub anchor: usize,
    pub pos: [f64; 2],
    pub size: [f64; 2],
    pub prop: bool,
    pub angle: f64,
    pub ll: [f64; 2],
    pub ur: [f64; 2],
    pub sides: u32,
    pub side_len: f64,
    pub sharp: f64,
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub len: f64,
    pub pts: Vec<[f64; 2]>,
    pub row: usize,
    pub key: u64,
}

impl Default for CoUi {
    fn default() -> Self {
        CoUi { tab: 0, anchor: 4, pos: [252.0, 360.0], size: [216.0, 144.0], prop: false, angle: 0.0, ll: [144.0, 432.0], ur: [360.0, 288.0], sides: 5, side_len: 0.0, sharp: 53.0, start: [270.0, 396.0], end: [342.0, 396.0], len: 72.0, pts: vec![], row: 0, key: 0 }
    }
}

fn len(app: &AnyVectorApp, ui: &mut Ui, id: &str, v: &mut f64, enabled: bool) -> bool {
    let (u, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    crate::widgets::unit_field(ui, egui::Id::unique(("co", id)), v, u, dpi, 72.0, enabled)
}

/// A small button that picks a point, length or angle on the canvas.
fn pick(app: &mut AnyVectorApp, ui: &mut Ui, icon: &str, tip: &str, slot: u8, segment: bool) {
    if crate::widgets::icon_button_sized(&mut app.icons, ui, icon, tip, true, app.dockers.ui.pick_slot == slot && app.ts.pick_target.is_some(), 22.0, 16.0).clicked() {
        app.dockers.ui.pick_slot = slot;
        app.ts.pick_target = Some(if segment { crate::tools::PickTarget::Measure { first: None } } else { crate::tools::PickTarget::DockerPoint });
    }
}

fn anchor_offset(anchor: usize, w: f64, h: f64) -> Vec2 {
    Vec2::new(w * (anchor % 3) as f64 / 2.0, h * (anchor / 3) as f64 / 2.0)
}

/// Fill the fields from the selected object.
fn refresh(app: &AnyVectorApp, c: &mut CoUi, origin: Point) {
    let Some(d) = app.doc() else { return };
    let Some(bb) = d.selection_bounds() else { return };
    let r = |p: Point| [p.x - origin.x, p.y - origin.y];
    let a = bb.origin() + anchor_offset(c.anchor, bb.width(), bb.height());
    c.pos = r(a);
    c.size = [bb.width(), bb.height()];
    c.ll = r(Point::new(bb.x0, bb.y1));
    c.ur = r(Point::new(bb.x1, bb.y0));
    let Some(o) = d.selection.first().and_then(|id| d.doc.find(*id)) else { return };
    match &o.kind {
        ObjKind::Polygon(p) => {
            c.sides = p.sides;
            c.sharp = p.sharpness;
        }
        ObjKind::Path(np) if np.subpaths.len() == 1 && np.subpaths[0].nodes.len() == 2 && !np.subpaths[0].closed => {
            let n = &np.subpaths[0].nodes;
            let (s, e) = (o.transform * n[0].p, o.transform * n[1].p);
            c.start = r(s);
            c.end = r(e);
            c.len = (e - s).hypot();
        }
        _ => {}
    }
    // the nodes of the object (its outline for shapes)
    c.pts = vector_core::geom::flatten_polylines(&o.world_path(), 0.25)
        .into_iter()
        .flat_map(|(mut pts, _)| {
            // a closed outline does not repeat its first point
            if pts.len() > 1 && (pts[0] - pts[pts.len() - 1]).hypot() < 1e-6 {
                pts.pop();
            }
            pts
        })
        .take(500)
        .map(r)
        .collect();
    c.pts.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6);
    c.row = c.row.min(c.pts.len().saturating_sub(1));
}

/// The object the docker describes, in document coordinates.
fn build(c: &CoUi, origin: Point) -> Option<(ObjKind, Affine)> {
    let doc = |p: [f64; 2]| Point::new(p[0] + origin.x, p[1] + origin.y);
    let (w, h) = (c.size[0].max(0.01), c.size[1].max(0.01));
    let tl = doc(c.pos) - anchor_offset(c.anchor, w, h);
    let rot = |w: f64, h: f64, tl: Point| {
        let ctr = tl + Vec2::new(w / 2.0, h / 2.0);
        Affine::translate(ctr.to_vec2()) * Affine::rotate(-c.angle.to_radians()) * Affine::translate(-ctr.to_vec2()) * Affine::translate(tl.to_vec2())
    };
    Some(match c.tab {
        0 => (ObjKind::Rect(RectShape { w, h, radii: [0.0; 4], corner: Default::default() }), rot(w, h, tl)),
        1 => (ObjKind::Ellipse(EllipseShape { w, h, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false }), rot(w, h, tl)),
        2..=4 => {
            let kind = [PolygonKind::Polygon, PolygonKind::Star, PolygonKind::ComplexStar][c.tab - 2];
            (ObjKind::Polygon(PolygonShape { w, h, sides: c.sides.max(3), kind, sharpness: c.sharp }), rot(w, h, tl))
        }
        5 => (ObjKind::Path(polyline(&[doc(c.start), doc(c.end)], false)), Affine::IDENTITY),
        _ => {
            if c.pts.len() < 2 {
                return None;
            }
            let pts: Vec<Point> = c.pts.iter().map(|p| doc(*p)).collect();
            let closed = pts.len() > 2 && (pts[0] - pts[pts.len() - 1]).hypot() < 1e-6;
            (ObjKind::Path(polyline(if closed { &pts[..pts.len() - 1] } else { &pts }, closed)), Affine::IDENTITY)
        }
    })
}

fn rotation_row(app: &mut AnyVectorApp, ui: &mut Ui, c: &mut CoUi) {
    form_row(ui, 52.0, "", true, |ui| {
        let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
        if let Some(t) = app.icons.get(ui.ctx(), "rotate_ccw", 16.0, crate::icons::Tone::Normal) {
            ui.painter().image(t.id, egui::Rect::from_center_size(r.center(), vec2(16.0, 16.0)), egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        spin(ui, &mut c.angle, -360.0..=360.0, 1.0, 1, "", 70.0);
        ui.label("°");
        ui.add_space(16.0);
        pick(app, ui, "pick_angle", "Set the angle interactively: click two points on the canvas", SLOT_ANGLE, true);
    });
}

/// The X/Y/W/H rows (with the object origin grid) of the shape tabs.
fn position_rows(app: &mut AnyVectorApp, ui: &mut Ui, c: &mut CoUi, show_size: bool) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.add_space(8.0);
            super::arrange::origin_grid(ui, &mut c.anchor, true);
        });
        ui.vertical(|ui| {
            form_row(ui, 40.0, "X:", true, |ui| {
                len(app, ui, "x", &mut c.pos[0], true);
                ui.add_space(8.0);
                pick(app, ui, "pick_point", "Set the origin interactively: click on the canvas", SLOT_POS, false);
            });
            form_row(ui, 40.0, "Y:", true, |ui| len(app, ui, "y", &mut c.pos[1], true));
            if show_size {
                size_rows(app, ui, c);
            }
        });
    });
}

fn size_rows(app: &mut AnyVectorApp, ui: &mut Ui, c: &mut CoUi) {
    let w0 = c.size[0];
    form_row(ui, 40.0, "W:", true, |ui| {
        len(app, ui, "w", &mut c.size[0], true);
        ui.add_space(8.0);
        pick(app, ui, "pick_size", "Set the width interactively: click two points on the canvas", SLOT_W, true);
    });
    if c.prop && w0 != c.size[0] && w0 > 0.0 {
        c.size[1] *= c.size[0] / w0;
    }
    let h0 = c.size[1];
    form_row(ui, 40.0, "H:", true, |ui| {
        len(app, ui, "h", &mut c.size[1], true);
        ui.add_space(8.0);
        pick(app, ui, "pick_size", "Set the height interactively: click two points on the canvas", SLOT_H, true);
    });
    if c.prop && h0 != c.size[1] && h0 > 0.0 {
        c.size[0] *= c.size[1] / h0;
    }
    form_row(ui, 40.0, "", true, |ui| ui.checkbox(&mut c.prop, "Set proportional"));
}

fn corner_rows(app: &mut AnyVectorApp, ui: &mut Ui, label: &str, v: &mut [f64; 2], id: &str, slot: u8) {
    ui.label(label);
    form_row(ui, 92.0, "X:", true, |ui| {
        len(app, ui, &format!("{id}x"), &mut v[0], true);
        ui.add_space(8.0);
        pick(app, ui, "pick_point", "Set the corner interactively: click on the canvas", slot, false);
    });
    form_row(ui, 92.0, "Y:", true, |ui| len(app, ui, &format!("{id}y"), &mut v[1], true));
}

pub fn coordinates(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let origin = app.doc().map(|d| d.doc.settings.ruler_origin).unwrap_or_default();
    let mut c = std::mem::take(&mut app.dockers.ui.co);
    // follow the selection
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        if let Some(d) = app.doc() {
            d.selection.hash(&mut h);
            d.selection_bounds().map(|b| [b.x0, b.y0, b.x1, b.y1].map(|v| (v * 100.0).round() as i64)).hash(&mut h);
        }
        c.anchor.hash(&mut h);
        h.finish()
    };
    if key != c.key {
        c.key = key;
        refresh(app, &mut c, origin);
    }
    // a point, length or angle picked on the canvas
    let slot = app.dockers.ui.pick_slot;
    let r = |p: Point| [p.x - origin.x, p.y - origin.y];
    if let Some(p) = app.ts.picked_point.take() {
        match slot {
            SLOT_POS => c.pos = r(p),
            SLOT_LL => c.ll = r(p),
            SLOT_UR => c.ur = r(p),
            SLOT_START => c.start = r(p),
            SLOT_END => c.end = r(p),
            _ => {}
        }
    }
    if let Some((a, b)) = app.ts.picked_segment.filter(|_| (SLOT_W..=SLOT_SIDE).contains(&slot)) {
        app.ts.picked_segment = None;
        let d = (b - a).hypot();
        match slot {
            SLOT_W => c.size[0] = d,
            SLOT_H => c.size[1] = d,
            SLOT_SIZE => c.len = d,
            SLOT_SIDE => c.side_len = d,
            _ => c.angle = -(b - a).atan2().to_degrees() + 0.0,
        }
    }
    // the tabs
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, (icon, tip, _)) in TABS.iter().enumerate() {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, c.tab == i).clicked() {
                c.tab = i;
            }
        }
    });
    ui.add_space(6.0);
    bold_title(ui, TABS[c.tab].2);
    ui.add_space(4.0);
    let before = (c.ll, c.ur);
    match c.tab {
        0 | 1 => {
            position_rows(app, ui, &mut c, true);
            rotation_row(app, ui, &mut c);
            section(ui, "Bounding box");
            corner_rows(app, ui, "Lower-left corner", &mut c.ll, "ll", SLOT_LL);
            corner_rows(app, ui, "Upper-right corner", &mut c.ur, "ur", SLOT_UR);
        }
        2..=4 => {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    super::arrange::origin_grid(ui, &mut c.anchor, true);
                });
                ui.vertical(|ui| {
                    let star = c.tab == 3 || c.tab == 4;
                    form_row(ui, 40.0, "", true, |ui| {
                        crate::widgets::icon_button_sized(&mut app.icons, ui, "points_sides", if star { "Points" } else { "Points or sides" }, true, false, 20.0, 16.0);
                        spin(ui, &mut c.sides, 3..=500, 1.0, 0, "", 86.0);
                    });
                    form_row(ui, 40.0, "", c.tab != 4, |ui| {
                        if star {
                            crate::widgets::icon_button_sized(&mut app.icons, ui, "star", "Sharpness", true, false, 20.0, 16.0);
                            spin(ui, &mut c.sharp, 1.0..=99.0, 1.0, 0, "", 86.0);
                        } else {
                            crate::widgets::icon_button_sized(&mut app.icons, ui, "polygon", "Side length", true, false, 20.0, 16.0);
                            if len(app, ui, "side", &mut c.side_len, true) && c.side_len > 0.0 {
                                // a regular polygon whose sides are that long
                                let d = c.side_len / (std::f64::consts::PI / c.sides.max(3) as f64).sin();
                                c.size = [d, d];
                            }
                            ui.add_space(4.0);
                            pick(app, ui, "pick_size", "Set the side length interactively: click two points on the canvas", SLOT_SIDE, true);
                        }
                    });
                });
            });
            rotation_row(app, ui, &mut c);
            section(ui, "Bounding box");
            ui.horizontal(|ui| {
                ui.add_space(52.0);
                ui.vertical(|ui| {
                    form_row(ui, 40.0, "X:", true, |ui| {
                        len(app, ui, "px", &mut c.pos[0], true);
                        ui.add_space(8.0);
                        pick(app, ui, "pick_point", "Set the center interactively: click on the canvas", SLOT_POS, false);
                    });
                    form_row(ui, 40.0, "Y:", true, |ui| len(app, ui, "py", &mut c.pos[1], true));
                    size_rows(app, ui, &mut c);
                });
            });
        }
        5 => {
            ui.label("Start point");
            form_row(ui, 92.0, "X:", true, |ui| {
                len(app, ui, "sx", &mut c.start[0], true);
                ui.add_space(8.0);
                pick(app, ui, "pick_point", "Set the start point interactively: click on the canvas", SLOT_START, false);
            });
            form_row(ui, 92.0, "Y:", true, |ui| len(app, ui, "sy", &mut c.start[1], true));
            ui.label("End point");
            form_row(ui, 92.0, "X:", true, |ui| {
                len(app, ui, "ex", &mut c.end[0], true);
                ui.add_space(8.0);
                pick(app, ui, "pick_point", "Set the end point interactively: click on the canvas", SLOT_END, false);
            });
            form_row(ui, 92.0, "Y:", true, |ui| len(app, ui, "ey", &mut c.end[1], true));
            section(ui, "Object size");
            let (l0, a0) = (c.len, c.angle);
            form_row(ui, 92.0, "Size:", true, |ui| {
                len(app, ui, "len", &mut c.len, true);
                ui.add_space(8.0);
                pick(app, ui, "pick_size", "Set the length interactively: click two points on the canvas", SLOT_SIZE, true);
            });
            rotation_row(app, ui, &mut c);
            // length and angle move the end point
            if l0 != c.len || a0 != c.angle {
                let a = (-c.angle).to_radians();
                c.end = [c.start[0] + c.len * a.cos(), c.start[1] + c.len * a.sin()];
            } else {
                let v = Vec2::new(c.end[0] - c.start[0], c.end[1] - c.start[1]);
                c.len = v.hypot();
                if c.len > 1e-9 {
                    c.angle = -v.atan2().to_degrees() + 0.0;
                }
            }
        }
        _ => curve_table(app, ui, &mut c),
    }
    // the corners set the bounding box
    if before != (c.ll, c.ur) {
        let (x0, x1) = (c.ll[0].min(c.ur[0]), c.ll[0].max(c.ur[0]));
        let (y0, y1) = (c.ur[1].min(c.ll[1]), c.ur[1].max(c.ll[1]));
        c.size = [x1 - x0, y1 - y0];
        c.pos = [x0 + c.size[0] * (c.anchor % 3) as f64 / 2.0, y0 + c.size[1] * (c.anchor / 3) as f64 / 2.0];
    }
    section(ui, "");
    let made = build(&c, origin);
    let create = apply_bar(ui, "Create object", made.is_some());
    let has = app.has_selection();
    let replace = apply_bar(ui, "Replace object", made.is_some() && has);
    let style = app.ts.new_style();
    app.dockers.ui.co = c;
    let Some((kind, m)) = made else { return };
    let Some(d) = app.doc_mut() else { return };
    if create {
        let id = d.doc.new_id();
        d.add_objects("Create Object", vec![Object::new(id, kind, m, style)]);
    } else if replace {
        let Some(id) = d.selection.first().copied() else { return };
        d.modify("Replace Object", |doc, sel| {
            if let Some(o) = doc.find_mut(id) {
                o.kind = kind;
                o.transform = m;
                o.effects = Default::default();
            }
            *sel = vec![id];
        });
    }
}

/// Multi-Point Curve: the points of the curve, one row each (Insert adds, Delete removes).
fn curve_table(app: &mut AnyVectorApp, ui: &mut Ui, c: &mut CoUi) {
    let w = ui.available_width();
    let (u, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    let fmt = |v: f64| {
        let s = u.format(v, dpi);
        s.trim_end_matches(|ch: char| !ch.is_ascii_digit()).to_string()
    };
    let (hr, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(hr, 0.0, theme::BAR);
    p.line_segment([pos2(hr.center().x, hr.top()), pos2(hr.center().x, hr.bottom())], Stroke::new(1.0, theme::SEP));
    for (k, t) in ["X", "Y"].iter().enumerate() {
        p.text(pos2(hr.left() + w * (0.25 + 0.5 * k as f32), hr.center().y), egui::Align2::CENTER_CENTER, *t, egui::FontId::proportional(12.0), theme::TEXT);
    }
    let mut remove = None;
    egui::ScrollArea::vertical().id_salt("co_pts").max_height(ui.available_height() - 90.0).show(ui, |ui| {
        for i in 0..c.pts.len() {
            let (r, resp) = ui.allocate_exact_size(vec2(w, 20.0), Sense::click());
            if i == c.row {
                ui.painter().rect_filled(r, 0.0, theme::ACCENT);
            }
            if i == c.row && resp.double_clicked() {
                ui.memory_mut(|m| m.request_focus(egui::Id::unique(("co", "ptx"))));
            }
            if resp.clicked() {
                c.row = i;
            }
            for k in 0..2 {
                ui.painter().text(pos2(r.left() + 8.0 + w * 0.5 * k as f32, r.center().y), egui::Align2::LEFT_CENTER, fmt(c.pts[i][k]), egui::FontId::proportional(12.0), theme::TEXT);
            }
        }
    });
    // the selected point
    if let Some(pt) = c.pts.get_mut(c.row) {
        ui.horizontal(|ui| {
            ui.label("X:");
            len(app, ui, "ptx", &mut pt[0], true);
            ui.label("Y:");
            len(app, ui, "pty", &mut pt[1], true);
        });
    }
    if ui.ui_contains_pointer() || ui.memory(|m| m.focused().is_none()) {
        if ui.input(|i| i.key_pressed(egui::Key::Insert)) {
            let at = (c.row + 1).min(c.pts.len());
            let p = c.pts.get(c.row).copied().unwrap_or([0.0, 0.0]);
            c.pts.insert(at, [p[0] + 18.0, p[1]]);
            c.row = at;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Delete)) && !c.pts.is_empty() {
            remove = Some(c.row);
        }
    }
    if let Some(i) = remove {
        c.pts.remove(i);
        c.row = c.row.min(c.pts.len().saturating_sub(1));
    }
    let _ = Rect::ZERO;
}
