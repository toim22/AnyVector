//! Effects ▸ Extrude: the Camera, Rotation, Lighting, Extrude color and Bevel pages. The
//! Rotation, Lighting, Extrude color and Bevel pages are also the drop-down panels of the Extrude
//! tool's property bar.

use super::{bold_title, choice, color_pick_w, form_row, no_doc, radio, separator, spin};
use crate::app::VectorApp;
use crate::icons::Icons;
use crate::propbar::edit_sel;
use crate::theme;
use egui::{pos2, vec2, Color32, Pos2, Rect, Sense, Stroke, Ui};
use vector_core::object::{ExtrudeEffect, LIGHT_OFF};

/// The extrusion types, in the order of the property bar's list.
pub const TYPES: [&str; 6] = ["Small Back", "Small Front", "Big Back", "Big Front", "Back Parallel", "Front Parallel"];
const VP_MODES: [&str; 4] = ["VP Locked To Object", "VP Locked To Page", "Copy VP From...", "Shared Vanishing Point"];
/// Where lights 1, 2 and 3 go when they are turned on: the front top left, top right and center.
const DEFAULT_SPOT: [u8; 3] = [0, 2, 4];

pub fn extrude(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        no_doc(ui);
        return;
    }
    let cur = app.doc().and_then(|d| d.selection.first().and_then(|id| d.doc.find(*id)).map(|o| (o.effects.extrude.clone(), o.bounds())));
    let (cur, bounds) = match cur {
        Some((e, b)) => (e, Some(b)),
        None => (None, None),
    };
    let on = cur.is_some();
    let mut e = cur.unwrap_or_default();
    let tab = app.dockers.ui.ex_tab;
    ui.horizontal(|ui| {
        for (i, (icon, tip)) in [("ex_camera", "Camera"), ("ex_rotation", "Rotation"), ("ex_lighting", "Lighting"), ("ex_color", "Extrude color"), ("ex_bevel", "Bevel")].iter().enumerate() {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, tab == i).clicked() {
                app.dockers.ui.ex_tab = i;
            }
        }
    });
    ui.add_space(2.0);
    separator(ui);
    ui.add_space(4.0);
    let (un, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    let changed = match app.dockers.ui.ex_tab {
        0 => camera(app, ui, &mut e, on, bounds),
        1 => rotation_ui(&mut app.icons, ui, &mut e, on),
        2 => {
            let mut sel = app.dockers.ui.ex_light;
            let c = lighting_ui(ui, &mut e, on, &mut sel);
            app.dockers.ui.ex_light = sel;
            c
        }
        3 => color_ui(ui, &mut e, on),
        _ => bevel_ui(&mut app.icons, ui, &mut e, on, un, dpi),
    };
    ui.add_space(6.0);
    separator(ui);
    if changed && on {
        edit_sel(app, "Extrude", |o| {
            if o.effects.extrude.is_some() {
                o.effects.extrude = Some(e.clone());
            }
        });
    }
}

/// Camera: the extrusion type, the vanishing point and the depth.
fn camera(app: &mut VectorApp, ui: &mut Ui, e: &mut ExtrudeEffect, on: bool, bounds: Option<kurbo::Rect>) -> bool {
    let (un, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    let w = ui.available_width();
    let fw = w - 4.0;
    let mut ch = false;
    ui.label("Camera");
    ui.label("Extrusion Type:");
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let (r, _) = ui.allocate_exact_size(vec2(w - 22.0, 100.0), Sense::hover());
        type_preview(ui.painter(), r, e.kind);
    });
    ui.add_space(4.0);
    let mut k = (e.kind as usize).min(5);
    ui.add_enabled_ui(on, |ui| {
        if choice(ui, "ex_type", &mut k, &TYPES, fw - 8.0) {
            e.kind = k as u8;
            ch = true;
        }
    });
    ui.add_space(4.0);
    ui.label("Vanishing point:");
    let mut vp = e.vp_page as usize;
    ui.add_enabled_ui(on, |ui| {
        egui::ComboBox::from_id_salt("ex_vp").width(fw - 8.0).selected_text(VP_MODES[vp]).show_ui(ui, |ui| {
            for (i, n) in VP_MODES.iter().enumerate() {
                // a vanishing point shared by several extrusions is not kept
                if ui.add_enabled_ui(i < 3, |ui| ui.selectable_label(vp == i, *n)).inner.clicked() {
                    vp = i;
                }
            }
        });
    });
    let mut copy_vp = false;
    if vp != e.vp_page as usize {
        if vp == 2 {
            copy_vp = true;
        } else {
            e.vp_page = vp == 1;
            ch = true;
        }
    }
    ui.add_space(4.0);
    ui.label("Depth:");
    let mut depth = if on { e.steps.clamp(1, 99) } else { 1 };
    ui.add_enabled_ui(on, |ui| {
        if spin(ui, &mut depth, 1..=99, 1.0, 0, "", fw) {
            e.steps = depth;
            ch = true;
        }
    });
    // the vanishing point, from the page origin or from the object's center
    let from_center = app.dockers.ui.ex_from_center;
    let c = if from_center { kurbo::Point::ZERO } else { bounds.map(|b| b.center()).unwrap_or_default() };
    for (i, n) in ["X:", "Y:"].iter().enumerate() {
        ui.add_space(4.0);
        ui.label(*n);
        let mut v = match (on, i) {
            (false, _) => 0.0,
            (_, 0) => c.x + e.depth.x,
            _ => c.y + e.depth.y,
        };
        if crate::widgets::unit_field(ui, egui::Id::unique(("ex_vp_xy", i)), &mut v, un, dpi, fw - 20.0, on) {
            if i == 0 {
                e.depth.x = v - c.x;
            } else {
                e.depth.y = v - c.y;
            }
            ch = true;
        }
    }
    ui.add_space(6.0);
    ui.label("Measure from:");
    if radio(ui, !from_center, "Page Origin", true) {
        app.dockers.ui.ex_from_center = false;
    }
    if radio(ui, from_center, "Object center", on) {
        app.dockers.ui.ex_from_center = true;
    }
    if copy_vp {
        crate::propbar::press(app, "1d271da3");
    }
    ch
}

/// The picture of an extrusion type: the object's face drawn thick, the extruded face and the
/// edges joining them, arrows toward the vanishing point.
fn type_preview(p: &egui::Painter, r: Rect, kind: u8) {
    let p = p.with_clip_rect(r);
    p.rect_filled(r, 0.0, Color32::BLACK);
    let white = Color32::WHITE;
    let front_kind = matches!(kind, 1 | 3 | 5);
    let (scale, toward) = match kind {
        0 | 1 => (0.6, true),
        2 | 3 => (1.35, true),
        _ => (1.0, false),
    };
    let off = vec2(62.0, -30.0);
    let dir = if front_kind { -off } else { off };
    let fc = pos2(r.center().x + if front_kind { 30.0 } else { -30.0 }, r.center().y + if front_kind { -12.0 } else { 14.0 });
    let face = Rect::from_center_size(fc, vec2(58.0, 46.0));
    let ext = Rect::from_center_size(fc + dir, face.size() * scale);
    let corners = |q: Rect| [q.left_top(), q.right_top(), q.right_bottom(), q.left_bottom()];
    for (a, b) in corners(face).into_iter().zip(corners(ext)) {
        p.line_segment([a, b], Stroke::new(1.5, white));
    }
    p.rect_stroke(ext, 0.0, Stroke::new(1.5, white), egui::StrokeKind::Middle);
    p.rect_stroke(face, 0.0, Stroke::new(3.0, white), egui::StrokeKind::Middle);
    if toward {
        for a in [face.left_bottom(), face.center()] {
            p.arrow(a, dir * 0.85, Stroke::new(1.5, white));
        }
    }
}

fn wrap(a: f64) -> f64 {
    (a + 180.0).rem_euclid(360.0) - 180.0
}

/// Rotation: a 3D figure turned with the extrusion (drag it to turn), Reset, and the angles.
pub(crate) fn rotation_ui(icons: &mut Icons, ui: &mut Ui, e: &mut ExtrudeEffect, on: bool) -> bool {
    let w = ui.available_width();
    let mut ch = false;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 130.0), if on { Sense::drag() } else { Sense::hover() });
    if resp.dragged() {
        let d = resp.drag_delta();
        e.rotation[1] = wrap(e.rotation[1] + d.x as f64 * 0.8);
        e.rotation[0] = wrap(e.rotation[0] - d.y as f64 * 0.8);
        ch = true;
    }
    draw_three(ui.painter(), r, e.rotation, on);
    ui.horizontal(|ui| {
        ui.add_space(w / 2.0 - 13.0);
        if crate::widgets::icon_button(icons, ui, "rotate_ccw", "Reset", on, false).clicked() {
            e.rotation = [0.0; 3];
            ch = true;
        }
    });
    ui.label("Rotation values");
    for (i, n) in ["x:", "y:", "z:"].iter().enumerate() {
        ui.label(*n);
        let mut v = e.rotation[i];
        ui.add_enabled_ui(on, |ui| {
            if spin(ui, &mut v, -180.0..=180.0, 1.0, 0, "", w - 4.0) {
                e.rotation[i] = v;
                ch = true;
            }
        });
        ui.add_space(2.0);
    }
    ch
}

/// A "3" with some depth, turned by the rotation (degrees around x, y and z) and seen in perspective.
fn draw_three(p: &egui::Painter, r: Rect, rot: [f64; 3], on: bool) {
    let mut pts = vec![];
    let mut arc = |cx: f64, cy: f64, rad: f64, a0: f64, sweep: f64| {
        for k in 0..=24 {
            let a = (a0 + sweep * k as f64 / 24.0).to_radians();
            pts.push((cx + a.cos() * rad, cy + a.sin() * rad));
        }
    };
    arc(0.0, -0.3, 0.3, 210.0, 240.0);
    arc(0.0, 0.33, 0.33, 270.0, 240.0);
    let (rx, ry, rz) = (rot[0].to_radians(), rot[1].to_radians(), rot[2].to_radians());
    let s = r.height() as f64 * 0.75;
    let proj = |x: f64, y: f64, z: f64| -> Pos2 {
        let (y1, z1) = (y * rx.cos() - z * rx.sin(), y * rx.sin() + z * rx.cos());
        let (x2, z2) = (x * ry.cos() + z1 * ry.sin(), -x * ry.sin() + z1 * ry.cos());
        let (x3, y3) = (x2 * rz.cos() - y1 * rz.sin(), x2 * rz.sin() + y1 * rz.cos());
        let f = 3.0 / (3.0 + z2);
        pos2(r.center().x + (x3 * f * s) as f32, r.center().y + (y3 * f * s) as f32)
    };
    let back: Vec<Pos2> = pts.iter().map(|&(x, y)| proj(x, y, 0.18)).collect();
    let front: Vec<Pos2> = pts.iter().map(|&(x, y)| proj(x, y, 0.0)).collect();
    let (fc, bc) = if on { (Color32::from_rgb(0x1e, 0x88, 0xd8), Color32::from_rgb(0x10, 0x4a, 0x7a)) } else { (Color32::from_gray(72), Color32::from_gray(54)) };
    let sw = (s * 0.09) as f32;
    for k in (0..front.len()).step_by(5) {
        p.line_segment([front[k], back[k]], Stroke::new(sw * 0.5, bc));
    }
    p.add(egui::Shape::line(back, Stroke::new(sw, bc)));
    p.add(egui::Shape::line(front, Stroke::new(sw, fc)));
}

/// A light position on the light cube: (layer from the front, row, column), a 3 × 3 × 3 grid.
fn spot_parts(pos: u8) -> (u8, u8, u8) {
    (pos / 9, (pos % 9) / 3, pos % 3)
}

/// Lights can sit where the cube's lines cross on its visible faces: front, top and right.
fn spot_visible(pos: u8) -> bool {
    let (d, row, col) = spot_parts(pos);
    pos < 27 && (d == 0 || row == 0 || col == 2)
}

/// Where a light position is drawn in the light cube of `r`.
fn spot_at(r: Rect, pos: u8) -> Pos2 {
    let (d, row, col) = spot_parts(pos);
    let s = r.width();
    let a = s * 0.58;
    let depth = vec2(s * 0.28, -s * 0.24);
    pos2(r.left() + s * 0.06, r.top() + s * 0.34) + vec2(col as f32 * a / 2.0, row as f32 * a / 2.0) + depth * (d as f32 / 2.0)
}

/// Lighting: the light cube (click a crossing to move the selected light there, or click a light
/// to select it) and the three lights with their intensity.
pub(crate) fn lighting_ui(ui: &mut Ui, e: &mut ExtrudeEffect, on: bool, sel: &mut usize) -> bool {
    let w = ui.available_width();
    let mut ch = false;
    bold_title(ui, "Lighting");
    let mut lights = e.lights.clone();
    lights.resize(3, (LIGHT_OFF, 0.0));
    let side = (w - 12.0).max(60.0);
    let (frame, resp) = ui.allocate_exact_size(vec2(side + 8.0, side + 8.0), if on { Sense::click() } else { Sense::hover() });
    let r = frame.shrink(4.0);
    let p = ui.painter();
    p.rect_stroke(frame, 0.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Inside);
    p.rect_filled(r, 0.0, Color32::WHITE);
    let line = |a: u8, b: u8| p.line_segment([spot_at(r, a), spot_at(r, b)], Stroke::new(1.0, Color32::BLACK));
    for i in 0..3u8 {
        // the front face, the top and the right
        line(i * 3, i * 3 + 2);
        line(i, 6 + i);
        line(i, 18 + i);
        line(9 * i, 9 * i + 2);
        line(i * 3 + 2, 18 + i * 3 + 2);
        line(9 * i + 2, 9 * i + 8);
    }
    let font = egui::FontId::proportional(11.0);
    for (n, (pos, _)) in lights.iter().enumerate() {
        if *pos == LIGHT_OFF || !spot_visible(*pos) {
            continue;
        }
        let c = spot_at(r, *pos);
        let fill = if n == *sel { Color32::from_rgb(0x1e, 0x88, 0xd8) } else { Color32::from_rgb(0xff, 0xe0, 0x40) };
        p.circle_filled(c, 7.5, fill);
        p.circle_stroke(c, 7.5, Stroke::new(1.0, Color32::BLACK));
        p.text(c, egui::Align2::CENTER_CENTER, format!("{}", n + 1), font.clone(), Color32::BLACK);
    }
    if let Some(at) = resp.interact_pointer_pos().filter(|_| resp.clicked()) {
        let near = (0..27u8).filter(|q| spot_visible(*q)).map(|q| (q, spot_at(r, q).distance(at))).filter(|(_, d)| *d < 12.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(q, _)| q);
        if let Some(q) = near {
            if let Some(n) = lights.iter().position(|l| l.0 == q) {
                *sel = n;
            } else if lights[*sel].0 != LIGHT_OFF {
                lights[*sel].0 = q;
                ch = true;
            }
        }
    }
    ui.add_space(6.0);
    for n in 0..3 {
        ui.horizontal(|ui| {
            let mut lit = lights[n].0 != LIGHT_OFF;
            if ui.add_enabled(on, egui::Checkbox::new(&mut lit, format!("{}", n + 1))).changed() {
                lights[n] = if lit { (DEFAULT_SPOT[n], 100.0) } else { (LIGHT_OFF, 0.0) };
                *sel = n;
                ch = true;
            }
            let en = on && lit;
            let mut v = lights[n].1;
            if track(ui, &mut v, w - 120.0, en) {
                lights[n].1 = v;
                ch = true;
            }
            ui.add_enabled_ui(en, |ui| {
                if spin(ui, &mut v, 0.0..=100.0, 1.0, 0, "", 48.0) {
                    lights[n].1 = v;
                    ch = true;
                }
            });
        });
    }
    if ch {
        // lights after the last one turned on are dropped
        while lights.last().is_some_and(|l| l.0 == LIGHT_OFF) {
            lights.pop();
        }
        e.lights = lights;
    }
    ch
}

/// A 0..100 slider track, ticks every 20.
fn track(ui: &mut Ui, v: &mut f64, w: f32, enabled: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(w.max(40.0), 20.0), if enabled { Sense::click_and_drag() } else { Sense::hover() });
    let (x0, x1, y) = (r.left() + 6.0, r.right() - 6.0, r.center().y - 2.0);
    let mut changed = false;
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        *v = (((p.x - x0) / (x1 - x0)).clamp(0.0, 1.0) as f64 * 100.0).round();
        changed = true;
    }
    let kx = x0 + (*v / 100.0).clamp(0.0, 1.0) as f32 * (x1 - x0);
    let blue = if enabled { Color32::from_rgb(0x1e, 0x88, 0xd8) } else { Color32::from_gray(90) };
    let p = ui.painter();
    p.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(3.0, Color32::from_gray(80)));
    p.line_segment([pos2(x0, y), pos2(kx, y)], Stroke::new(3.0, blue));
    for k in 0..=5 {
        let tx = x0 + (x1 - x0) * k as f32 / 5.0;
        p.line_segment([pos2(tx, y + 5.0), pos2(tx, y + 7.0)], Stroke::new(1.0, if enabled { theme::TEXT_DIM } else { theme::SEP }));
    }
    p.circle_filled(pos2(kx, y), 5.0, theme::BG);
    p.circle_stroke(pos2(kx, y), 5.0, Stroke::new(1.5, blue));
    changed
}

/// Extrude color: the object's fill (draped over the extrusion or not), a solid fill, or a shade
/// from one color to another.
pub(crate) fn color_ui(ui: &mut Ui, e: &mut ExtrudeEffect, on: bool) -> bool {
    let w = ui.available_width() - 4.0;
    let mut ch = false;
    bold_title(ui, "Extrude color");
    if radio(ui, e.color_mode == 0, "Use object fill", on) {
        e.color_mode = 0;
        ch = true;
    }
    let mut d = e.drape;
    if ui.add_enabled(on && e.color_mode == 0, egui::Checkbox::new(&mut d, "Drape fills")).changed() {
        e.drape = d;
        ch = true;
    }
    if radio(ui, e.color_mode == 1, "Solid fill", on) {
        e.color_mode = 1;
        ch = true;
    }
    let mut c = Some(e.color);
    if color_pick_w(ui, "ex_solid", &mut c, false, on && e.color_mode == 1, w) {
        e.color = c.unwrap_or(e.color);
        ch = true;
    }
    ui.add_space(4.0);
    if radio(ui, e.color_mode == 2, "Shade", on) {
        e.color_mode = 2;
        ch = true;
    }
    ui.label("From:");
    let mut c = Some(e.color);
    if color_pick_w(ui, "ex_from", &mut c, false, on && e.color_mode == 2, w) {
        e.color = c.unwrap_or(e.color);
        ch = true;
    }
    ui.label("To:");
    let mut c = Some(e.shade);
    if color_pick_w(ui, "ex_to", &mut c, false, on && e.color_mode == 2, w) {
        e.shade = c.unwrap_or(e.shade);
        ch = true;
    }
    ch
}

/// Bevel: the bevel's profile (drag the marker: across is the depth, up the angle), its depth and
/// angle, Show bevel only, and the bevel's color.
pub(crate) fn bevel_ui(icons: &mut Icons, ui: &mut Ui, e: &mut ExtrudeEffect, on: bool, un: vector_core::Unit, dpi: f64) -> bool {
    const FULL: f64 = 36.0;
    let w = ui.available_width();
    let mut ch = false;
    bold_title(ui, "Bevel");
    let mut use_b = e.bevel.is_some();
    if ui.add_enabled(on, egui::Checkbox::new(&mut use_b, "Use bevel")).changed() {
        e.bevel = use_b.then_some((7.2, 45.0));
        if !use_b {
            e.bevel_only = false;
        }
        ch = true;
    }
    let en = on && use_b;
    let (mut depth, mut angle) = e.bevel.unwrap_or((7.2, 45.0));
    let side = (w - 40.0).max(60.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        let (r, resp) = ui.allocate_exact_size(vec2(side, side), if en { Sense::click_and_drag() } else { Sense::hover() });
        if let Some(at) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
            let gw = (r.right() - at.x).clamp(1.0, side) as f64;
            let gh = (r.bottom() - at.y).clamp(1.0, side) as f64;
            depth = (gw / side as f64 * FULL).max(0.1);
            angle = gh.atan2(gw).to_degrees().clamp(1.0, 89.0);
            ch = true;
        }
        let gw = (depth / FULL).clamp(0.0, 1.0) as f32 * side;
        let gh = (gw * (angle.to_radians().tan() as f32)).min(side);
        let m = pos2(r.right() - gw, r.bottom() - gh);
        let p = ui.painter();
        p.rect_filled(r, 0.0, Color32::WHITE);
        p.rect_filled(Rect::from_min_max(m, r.max), 0.0, Color32::from_gray(0x88));
        p.rect_stroke(Rect::from_min_max(m, r.max), 0.0, Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Inside);
        p.circle_filled(m, 4.0, Color32::WHITE);
        p.circle_stroke(m, 4.0, Stroke::new(1.0, Color32::BLACK));
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        form_row(ui, 52.0, "Depth:", en, |ui| {
            if crate::widgets::unit_field(ui, egui::Id::unique("ex_bevel_depth"), &mut depth, un, dpi, 66.0, en) {
                ch = true;
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::widgets::icon_button(icons, ui, "rotate_ccw", "Reset", en, false).clicked() {
                depth = 7.2;
                angle = 45.0;
                ch = true;
            }
        });
    });
    form_row(ui, 52.0, "Angle:", en, |ui| {
        if spin(ui, &mut angle, 1.0..=89.0, 0.5, 1, " °", 86.0) {
            ch = true;
        }
    });
    if en {
        e.bevel = Some((depth.clamp(0.1, 144.0), angle.clamp(1.0, 89.0)));
    }
    ui.add_space(4.0);
    let mut only = e.bevel_only;
    if ui.add_enabled(en, egui::Checkbox::new(&mut only, "Show bevel only")).changed() {
        e.bevel_only = only;
        ch = true;
    }
    let mut fill = e.bevel_fill;
    if ui.add_enabled(en, egui::Checkbox::new(&mut fill, "Use extrude fill for bevel")).changed() {
        e.bevel_fill = fill;
        ch = true;
    }
    if color_pick_w(ui, "ex_bevel_color", &mut e.bevel_color, true, en && !e.bevel_fill, 44.0) {
        ch = true;
    }
    ch
}
