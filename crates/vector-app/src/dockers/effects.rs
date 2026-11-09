//! Effects dockers: Contour, Blend, Extrude, Bevel, Lens, Envelope, Distort, Shadow, Bitmap Effects.

use super::{apply_bar, bold_title, choice, color_pick, color_pick_w, form_row, hint, no_doc, radio, separator, spin, tick_slider};
use crate::app::VectorApp;
use crate::propbar::edit_sel;
use egui::Ui;
use vector_core::object::{ContourDir, EnvelopeEffect, LensEffect, LensKind};
use vector_core::{Color, ObjKind, Object};

fn first(app: &VectorApp) -> Option<Object> {
    let d = app.doc()?;
    d.doc.find(*d.selection.first()?).cloned()
}

fn guard(app: &VectorApp, ui: &mut Ui) -> bool {
    if !app.has_doc() {
        no_doc(ui);
        return false;
    }
    if !app.has_selection() {
        hint(ui, "Select an object to apply this effect.");
    }
    true
}

pub fn contour(app: &mut VectorApp, ui: &mut Ui) {
    if !guard(app, ui) {
        return;
    }
    // the settings of the selected object's contour
    let sel = app.doc().and_then(|d| d.selection.first().copied());
    if sel != app.dockers.ui.contour_for {
        app.dockers.ui.contour_for = sel;
        if let Some(c) = first(app).and_then(|o| o.effects.contour) {
            app.dockers.ui.contour = c;
        }
    }
    let (un, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    let mut c = app.dockers.ui.contour.clone();
    let lw = 120.0;
    bold_title(ui, "Contour alignment:");
    ui.horizontal(|ui| {
        for (dir, icon, tip) in [(ContourDir::ToCenter, "contour_center", "To center"), (ContourDir::Inside, "contour_inside", "Inside contour"), (ContourDir::Outside, "contour_outside", "Outside contour")] {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, c.dir == dir).clicked() {
                c.dir = dir;
            }
        }
    });
    ui.add_space(12.0);
    form_row(ui, lw, "Steps:", c.dir != ContourDir::ToCenter, |ui| spin(ui, &mut c.steps, 1..=999, 1.0, 0, "", 92.0));
    ui.add_space(5.0);
    form_row(ui, lw, "Offset:", true, |ui| crate::widgets::unit_field(ui, egui::Id::unique("ct_offset"), &mut c.offset, un, dpi, 74.0, true));
    c.offset = c.offset.max(0.01);
    ui.add_space(14.0);
    ui.label("Corner:");
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        for (k, icon, tip) in [(0, "contour_corners", "Mitered corners"), (1, "contour_round", "Round corners"), (2, "contour_bevel", "Bevel corners")] {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, c.corner == k).clicked() {
                c.corner = k;
            }
        }
    });
    ui.add_space(12.0);
    separator(ui);
    ui.add_space(12.0);
    bold_title(ui, "Color blend");
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        for (k, icon, tip) in [(0, "contour_colors", "Linear contour colors"), (1, "contour_cw", "Clockwise contour colors"), (2, "contour_ccw", "Counterclockwise contour colors")] {
            if crate::widgets::icon_button_sized(&mut app.icons, ui, icon, tip, true, c.color_dir == k, 28.0, 20.0).clicked() {
                c.color_dir = k;
            }
        }
    });
    ui.add_space(12.0);
    form_row(ui, lw, "Outline:", true, |ui| {
        let mut v = Some(c.end_outline);
        if color_pick_w(ui, "ct_outline", &mut v, false, true, 92.0) {
            c.end_outline = v.unwrap_or(Color::BLACK);
        }
    });
    ui.add_space(5.0);
    form_row(ui, lw, "Fill:", true, |ui| {
        let mut v = Some(c.end_fill);
        if color_pick_w(ui, "ct_fill", &mut v, false, true, 92.0) {
            c.end_fill = v.unwrap_or(Color::WHITE);
        }
    });
    ui.add_space(12.0);
    separator(ui);
    ui.add_space(12.0);
    bold_title(ui, "Acceleration");
    ui.add_space(10.0);
    ui.label("Object:");
    ui.add_space(4.0);
    if accel_track(ui, &mut c.accel, true) && c.link_accel {
        c.color_accel = c.accel;
    }
    ui.add_space(12.0);
    ui.label("Color:");
    ui.add_space(4.0);
    if accel_track(ui, &mut c.color_accel, true) && c.link_accel {
        c.accel = c.color_accel;
    }
    ui.add_space(2.0);
    if ui.checkbox(&mut c.link_accel, "Link acceleration").changed() && c.link_accel {
        c.color_accel = c.accel;
    }
    ui.add_space(10.0);
    separator(ui);
    ui.add_space(6.0);
    app.dockers.ui.contour = c.clone();
    if apply_bar(ui, "Apply", app.has_selection()) {
        edit_sel(app, "Contour", |o| o.effects.contour = Some(c.clone()));
    }
}

/// An acceleration track across the docker: -100..100, the middle marked under the track.
fn accel_track(ui: &mut Ui, v: &mut f64, enabled: bool) -> bool {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 20.0), if enabled { egui::Sense::click_and_drag() } else { egui::Sense::hover() });
    let (x0, x1, y) = (r.left() + 10.0, r.right() - 10.0, r.center().y - 2.0);
    let mut changed = false;
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        *v = (((p.x - x0) / (x1 - x0)).clamp(0.0, 1.0) as f64 * 200.0 - 100.0).round();
        changed = true;
    }
    let kx = x0 + ((*v + 100.0) / 200.0).clamp(0.0, 1.0) as f32 * (x1 - x0);
    let blue = if enabled { egui::Color32::from_rgb(0x1e, 0x88, 0xd8) } else { egui::Color32::from_gray(90) };
    let p = ui.painter();
    p.line_segment([egui::pos2(x0, y), egui::pos2(x1, y)], egui::Stroke::new(3.0, egui::Color32::from_gray(80)));
    p.line_segment([egui::pos2(x0, y), egui::pos2(kx, y)], egui::Stroke::new(3.0, blue));
    let mx = (x0 + x1) / 2.0;
    p.line_segment([egui::pos2(mx, y + 5.0), egui::pos2(mx, y + 8.0)], egui::Stroke::new(1.0, crate::theme::TEXT));
    p.circle_filled(egui::pos2(kx, y), 5.5, crate::theme::BG);
    p.circle_stroke(egui::pos2(kx, y), 5.5, egui::Stroke::new(1.5, blue));
    changed
}

/// A section title with a line running on to the right edge.
fn line_title(ui: &mut Ui, title: &str) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(egui::vec2(w, 20.0), egui::Sense::hover());
    let g = ui.painter().layout_no_wrap(title.to_string(), egui::FontId::proportional(12.5), crate::theme::TEXT);
    let tw = g.size().x;
    ui.painter().galley(egui::pos2(r.left(), r.center().y - g.size().y / 2.0), g, crate::theme::TEXT);
    ui.painter().hline(r.left() + tw + 20.0..=r.right() - 4.0, r.center().y, egui::Stroke::new(1.0, crate::theme::SEP));
}

/// An acceleration slider: -100..100, the middle marked.
fn accel_slider(ui: &mut Ui, label: &str, v: &mut f64, enabled: bool) {
    ui.horizontal(|ui| {
        let (lr, _) = ui.allocate_exact_size(egui::vec2(70.0, 22.0), egui::Sense::hover());
        ui.painter().text(egui::pos2(lr.right() - 6.0, lr.center().y), egui::Align2::RIGHT_CENTER, label, egui::FontId::proportional(12.5), if enabled { crate::theme::TEXT } else { crate::theme::DISABLED });
        let w = ui.available_width() - 6.0;
        let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 22.0), if enabled { egui::Sense::click_and_drag() } else { egui::Sense::hover() });
        let (x0, x1, y) = (r.left() + 6.0, r.right() - 6.0, r.center().y);
        if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
            *v = (((p.x - x0) / (x1 - x0)).clamp(0.0, 1.0) as f64 * 200.0 - 100.0).round();
        }
        let kx = x0 + ((*v + 100.0) / 200.0) as f32 * (x1 - x0);
        let blue = if enabled { egui::Color32::from_rgb(0x1e, 0x88, 0xd8) } else { egui::Color32::from_gray(90) };
        let p = ui.painter();
        p.line_segment([egui::pos2(x0, y), egui::pos2(x1, y)], egui::Stroke::new(3.0, egui::Color32::from_gray(80)));
        p.line_segment([egui::pos2(x0, y), egui::pos2(kx, y)], egui::Stroke::new(3.0, blue));
        p.line_segment([egui::pos2((x0 + x1) / 2.0, y + 5.0), egui::pos2((x0 + x1) / 2.0, y + 7.0)], egui::Stroke::new(1.0, crate::theme::TEXT_DIM));
        p.circle_filled(egui::pos2(kx, y), 5.5, crate::theme::BG);
        p.circle_stroke(egui::pos2(kx, y), 5.5, egui::Stroke::new(1.5, blue));
    });
}

pub fn blend(app: &mut VectorApp, ui: &mut Ui) {
    if !guard(app, ui) {
        return;
    }
    let cur = first(app).and_then(|o| if let ObjKind::Blend(b) = o.kind { Some(b) } else { None });
    let is_blend = cur.is_some();
    // the settings of the selected blend
    let sel = app.doc().and_then(|d| d.selection.first().copied());
    if sel != app.dockers.ui.blend_for {
        app.dockers.ui.blend_for = sel;
        if let Some(b) = &cur {
            let u = &mut app.dockers.ui;
            u.blend_steps = b.steps;
            u.blend_rot = b.rotation;
            u.blend_loop = b.looped;
            u.blend_accel = b.acceleration;
            u.blend_caccel = b.color_accel;
            u.blend_link = b.link_accel;
            u.blend_size = b.size_accel;
            u.blend_dir = b.color_dir;
        }
    }
    let n = app.doc().map(|d| d.selection.len()).unwrap_or(0);
    let full_w = ui.available_width();
    let (un, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    let mut u = std::mem::take(&mut app.dockers.ui);
    // number of steps or fixed spacing (the spacing is for blends along a path)
    ui.horizontal(|ui| {
        ui.add_space((full_w / 2.0 - 40.0).max(0.0));
        crate::widgets::icon_button_sized(&mut app.icons, ui, "blend_steps", "Number of steps", true, true, 26.0, 18.0);
        crate::widgets::icon_button_sized(&mut app.icons, ui, "blend_spacing", "Fixed spacing", false, false, 26.0, 18.0);
    });
    separator(ui);
    let icon_row = |app: &mut VectorApp, ui: &mut Ui, icon: &str, enabled: bool| {
        if let Some(t) = app.icons.get(ui.ctx(), icon, 16.0, if enabled { crate::icons::Tone::Normal } else { crate::icons::Tone::Disabled }) {
            let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 20.0), egui::Sense::hover());
            ui.painter().image(t.id, egui::Rect::from_center_size(r.center(), egui::vec2(16.0, 16.0)), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
    };
    let fw = (full_w - 110.0).max(80.0);
    ui.horizontal(|ui| {
        ui.add_space(46.0);
        icon_row(app, ui, "blend_steps", true);
        spin(ui, &mut u.blend_steps, 1..=999, 1.0, 0, "", fw);
    });
    ui.horizontal(|ui| {
        ui.add_space(46.0);
        icon_row(app, ui, "blend_spacing", false);
        ui.add_enabled_ui(false, |ui| crate::widgets::unit_field(ui, egui::Id::unique("bl_space"), &mut u.blend_spacing, un, dpi, fw - 14.0, false));
    });
    ui.horizontal(|ui| {
        ui.add_space(46.0);
        icon_row(app, ui, "blend_rotate", true);
        spin(ui, &mut u.blend_rot, -360.0..=360.0, 1.0, 1, "", fw);
        ui.label("°");
    });
    let rot = u.blend_rot.abs() > 1e-9;
    ui.horizontal(|ui| {
        ui.add_space(64.0);
        ui.add_enabled(rot, egui::Checkbox::new(&mut u.blend_loop, "Loop"));
    });
    let mut full_path = true;
    let mut rotate_all = false;
    for (v, t) in [(&mut full_path, "Blend along full path"), (&mut rotate_all, "Rotate all objects")] {
        ui.horizontal(|ui| {
            ui.add_space(64.0);
            ui.add_enabled(false, egui::Checkbox::new(v, t));
        });
    }
    ui.add_space(4.0);
    line_title(ui, "Acceleration");
    accel_slider(ui, "Object:", &mut u.blend_accel, true);
    if u.blend_link {
        u.blend_caccel = u.blend_accel;
    }
    accel_slider(ui, "Color:", &mut u.blend_caccel, !u.blend_link);
    for (v, t) in [(&mut u.blend_size, "Apply to sizing"), (&mut u.blend_link, "Link acceleration")] {
        ui.horizontal(|ui| {
            ui.add_space(64.0);
            ui.checkbox(v, t);
        });
    }
    let reset = ui
        .with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(4.0);
            ui.add(egui::Button::new("Reset").fill(egui::Color32::from_rgb(0x33, 0x33, 0x33)).min_size(egui::vec2(80.0, 24.0))).clicked()
        })
        .inner;
    if reset {
        u.blend_accel = 0.0;
        u.blend_caccel = 0.0;
    }
    ui.add_space(4.0);
    line_title(ui, "Color Blend");
    ui.horizontal(|ui| {
        ui.add_space(64.0);
        for (k, (icon, tip)) in [("blend_direct", "Direct color blend"), ("blend_cw", "Clockwise color blend"), ("blend_ccw", "Counterclockwise color blend")].into_iter().enumerate() {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, u.blend_dir == k as u8).clicked() {
                u.blend_dir = k as u8;
            }
        }
    });
    separator(ui);
    let settings = (u.blend_steps, u.blend_rot, u.blend_loop, u.blend_accel, u.blend_caccel, u.blend_link, u.blend_size, u.blend_dir);
    let apply = ui
        .with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(4.0);
            ui.add_enabled(n == 2 || is_blend, egui::Button::new("Apply").fill(egui::Color32::from_rgb(0x33, 0x33, 0x33)).min_size(egui::vec2(80.0, 24.0))).clicked()
        })
        .inner;
    // more options
    let (r, resp) = ui.allocate_exact_size(egui::vec2(full_w, 12.0), egui::Sense::click());
    if u.blend_more {
        crate::widgets::chevron_up(ui.painter(), r.center(), 3.0, crate::theme::TEXT_DIM);
    } else {
        crate::widgets::chevron(ui.painter(), r.center(), 3.0, crate::theme::TEXT_DIM);
    }
    if resp.clicked() {
        u.blend_more = !u.blend_more;
    }
    let more = u.blend_more;
    app.dockers.ui = u;
    if more {
        ui.columns(2, |c| {
            for (k, t) in ["Map nodes", "Split", "Fuse start", "Fuse end"].into_iter().enumerate() {
                let ui = &mut c[k / 2];
                ui.add_enabled(false, egui::Button::new(t).fill(egui::Color32::from_rgb(0x2c, 0x2c, 0x2c)).min_size(egui::vec2(ui.available_width(), 26.0)));
            }
        });
        separator(ui);
        ui.horizontal(|ui| {
            for (icon, tip) in [("blend_start", "Start object"), ("blend_end", "End object"), ("blend_path", "Path properties")] {
                crate::widgets::icon_button(&mut app.icons, ui, icon, tip, false, false);
            }
        });
    }
    if apply {
        let (steps, rot, looped, accel, caccel, link, size, dir) = settings;
        let set = move |b: &mut vector_core::object::BlendObj| {
            b.steps = steps;
            b.rotation = rot;
            b.looped = looped;
            b.acceleration = accel;
            b.color_accel = caccel;
            b.link_accel = link;
            b.size_accel = size;
            b.color_dir = dir;
        };
        if is_blend {
            edit_sel(app, "Blend", |o| {
                if let ObjKind::Blend(b) = &mut o.kind {
                    set(b);
                }
            });
        } else if let Some(d) = app.doc_mut() {
            let (a, b) = (d.selection[0], d.selection[1]);
            d.modify("Blend", |doc, sel| {
                if let Some(id) = vector_core::ops::create_blend(doc, a, b, steps) {
                    if let Some(ObjKind::Blend(bl)) = doc.find_mut(id).map(|o| &mut o.kind) {
                        set(bl);
                    }
                    *sel = vec![id];
                }
            });
        }
    }
}

pub fn bevel(app: &mut VectorApp, ui: &mut Ui) {
    if !guard(app, ui) {
        return;
    }
    // the settings of the selected object's bevel
    let sel = app.doc().and_then(|d| d.selection.first().copied());
    if sel != app.dockers.ui.bevel_for {
        app.dockers.ui.bevel_for = sel;
        if let Some(b) = first(app).and_then(|o| o.effects.bevel) {
            app.dockers.ui.bevel = b;
        }
    }
    let (un, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    let mut b = app.dockers.ui.bevel;
    ui.label("Style:");
    if radio(ui, !b.emboss, "Soft edge", true) {
        b.emboss = false;
    }
    if radio(ui, b.emboss, "Emboss", true) {
        b.emboss = true;
    }
    ui.add_space(6.0);
    ui.label("Bevel offset:");
    if radio(ui, b.to_center, "To center", !b.emboss) {
        b.to_center = true;
    }
    ui.horizontal(|ui| {
        if radio(ui, !b.to_center || b.emboss, "Distance:", true) {
            b.to_center = false;
        }
        ui.add_space((96.0 - ui.min_rect().width()).max(4.0));
        let en = !b.to_center || b.emboss;
        crate::widgets::unit_field(ui, egui::Id::unique("bv_dist"), &mut b.distance, un, dpi, 64.0, en);
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(22.0);
        ui.label("Shadow color:");
        color_pick(ui, "bv_shadow", &mut b.shadow, true, true);
    });
    ui.add_space(6.0);
    separator(ui);
    ui.add_space(6.0);
    bold_title(ui, "Light control");
    ui.horizontal(|ui| {
        ui.add_space(36.0);
        ui.label("Light color:");
        let mut l = Some(b.light);
        if color_pick(ui, "bv_light", &mut l, false, true) {
            b.light = l.unwrap_or(Color::WHITE);
        }
    });
    tick_slider(ui, "Intensity:", &mut b.intensity, 0.0..=100.0, true);
    tick_slider(ui, "Direction:", &mut b.direction, 0.0..=360.0, true);
    tick_slider(ui, "Altitude:", &mut b.altitude, 0.0..=90.0, !b.emboss);
    b.distance = b.distance.max(0.1);
    app.dockers.ui.bevel = b;
    ui.add_space(6.0);
    separator(ui);
    if apply_bar(ui, "Apply", app.has_selection()) {
        edit_sel(app, "Bevel", |o| o.effects.bevel = Some(b));
    }
}

pub fn lens(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        no_doc(ui);
        return;
    }
    let sel = app.has_selection();
    let obj = first(app);
    let cur = obj.as_ref().and_then(|o| o.effects.lens.clone());
    let center = obj.as_ref().map(|o| o.bounds().center()).unwrap_or_default();
    let (un, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    let mut names = vec!["No Lens Effect"];
    names.extend(LensKind::ALL.iter().map(|k| k.name()));
    let mut kind = cur.as_ref().map(|l| LensKind::ALL.iter().position(|k| *k == l.kind).unwrap_or(0) + 1).unwrap_or(0);
    let full_w = ui.available_width();
    let mut l = cur.clone().unwrap_or_else(|| LensEffect::new(LensKind::Brighten));
    let mut ch = false;
    let mut remove = false;
    ui.add_enabled_ui(sel, |ui| {
        if choice(ui, "lens_kind", &mut kind, &names, full_w - 10.0) {
            if kind == 0 {
                remove = true;
            } else {
                // a new type starts from its own settings, keeping the options below
                let k = LensKind::ALL[kind - 1];
                l = LensEffect { frozen: l.frozen, remove_face: l.remove_face, viewpoint: l.viewpoint, ..LensEffect::new(k) };
                ch = true;
            }
        }
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let pw = 58.0;
        ui.add_space((full_w - pw) / 2.0);
        let (r, _) = ui.allocate_exact_size(egui::vec2(pw, 40.0), egui::Sense::hover());
        lens_preview(ui.painter(), r, (kind > 0).then_some(&l));
    });
    ui.add_space(8.0);
    separator(ui);
    ui.add_space(8.0);
    let on = kind > 0 && sel;
    let rate_row = |ui: &mut Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, step: f64, dec: usize, unit: &str| -> bool {
        ui.horizontal(|ui| {
            ui.label(label);
            let c = spin(ui, v, range, step, dec, "", 64.0);
            ui.label(unit);
            c
        })
        .inner
    };
    let color_row = |ui: &mut Ui, id: &str, label: &str, c: &mut Color| -> bool {
        ui.horizontal(|ui| {
            ui.label(label);
            let mut v = Some(*c);
            let ch = color_pick(ui, id, &mut v, false, true);
            *c = v.unwrap_or(*c);
            ch
        })
        .inner
    };
    if kind > 0 {
        ui.add_enabled_ui(on, |ui| {
            match l.kind {
                LensKind::Brighten => ch |= rate_row(ui, "Rate:", &mut l.rate, -100.0..=100.0, 1.0, 0, "%"),
                LensKind::ColorAdd | LensKind::ColorLimit | LensKind::Transparency => {
                    ch |= rate_row(ui, "Rate:", &mut l.rate, 0.0..=100.0, 1.0, 0, "%");
                    ch |= color_row(ui, "lens_color", "Color:", &mut l.color);
                }
                LensKind::TintedGrayscale => ch |= color_row(ui, "lens_color", "Color:", &mut l.color),
                LensKind::FishEye => ch |= rate_row(ui, "Rate:", &mut l.rate, -1000.0..=1000.0, 1.0, 0, "%"),
                LensKind::HeatMap => ch |= rate_row(ui, "Palette rotation:", &mut l.rotation, 0.0..=100.0, 1.0, 0, "%"),
                LensKind::Magnify => ch |= rate_row(ui, "Amount:", &mut l.amount, 1.0..=100.0, 0.1, 1, "x"),
                LensKind::CustomColorMap => {
                    for (k, n) in ["Direct palette", "Forward rainbow", "Reverse rainbow"].iter().enumerate() {
                        if radio(ui, l.palette == k as u8, n, on) {
                            l.palette = k as u8;
                            ch = true;
                        }
                    }
                    ui.add_space(8.0);
                    let cw = (full_w - 70.0) / 2.0;
                    ui.horizontal(|ui| {
                        ui.label("From:");
                        ui.add_space(cw + 30.0);
                        ui.label("To:");
                    });
                    ui.horizontal(|ui| {
                        let mut a = Some(l.color);
                        if color_pick_w(ui, "lens_from", &mut a, false, on, cw) {
                            l.color = a.unwrap_or(l.color);
                            ch = true;
                        }
                        if ui.add_sized(egui::vec2(44.0, 22.0), egui::Button::new("< >")).on_hover_text("Swap colors").clicked() {
                            std::mem::swap(&mut l.color, &mut l.color2);
                            ch = true;
                        }
                        let mut b = Some(l.color2);
                        if color_pick_w(ui, "lens_to", &mut b, false, on, cw) {
                            l.color2 = b.unwrap_or(l.color2);
                            ch = true;
                        }
                    });
                }
                LensKind::Wireframe => {
                    for (outline, label) in [(true, "Outline:"), (false, "Fill:")] {
                        ui.horizontal(|ui| {
                            let slot = if outline { &mut l.outline } else { &mut l.fill };
                            let mut use_it = slot.is_some();
                            if ui.checkbox(&mut use_it, label).changed() {
                                *slot = use_it.then_some(if outline { Color::rgb(0x46, 0x82, 0xdc) } else { Color::WHITE });
                                ch = true;
                            }
                            let mut v = Some(slot.unwrap_or(Color::BLACK));
                            if color_pick(ui, if outline { "lens_wire_ol" } else { "lens_wire_fill" }, &mut v, false, use_it) {
                                *slot = v;
                                ch = true;
                            }
                        });
                    }
                }
                LensKind::Invert | LensKind::BitmapEffect => {}
            }
        });
        if !matches!(l.kind, LensKind::Invert | LensKind::BitmapEffect) {
            ui.add_space(8.0);
            separator(ui);
            ui.add_space(8.0);
        }
    }
    // the options of every lens
    let face = on && !matches!(l.kind, LensKind::FishEye | LensKind::Magnify | LensKind::BitmapEffect);
    let mut frozen = l.frozen;
    if ui.add_enabled(on, egui::Checkbox::new(&mut frozen, "Frozen")).changed() {
        l.frozen = frozen;
        ch = true;
    }
    let mut rf = l.remove_face;
    if ui.add_enabled(face, egui::Checkbox::new(&mut rf, "Remove Face")).changed() {
        l.remove_face = rf;
        ch = true;
    }
    let mut vp = l.viewpoint.is_some();
    if ui.add_enabled(on, egui::Checkbox::new(&mut vp, "Viewpoint")).changed() {
        l.viewpoint = vp.then_some(center);
        ch = true;
    }
    let mut p = l.viewpoint.unwrap_or(center);
    for (i, n) in ["X:", "Y:"].iter().enumerate() {
        ui.horizontal(|ui| {
            ui.add_space(30.0);
            ui.add_enabled(on && vp, egui::Label::new(*n));
            let v = if i == 0 { &mut p.x } else { &mut p.y };
            if crate::widgets::unit_field(ui, egui::Id::unique(("lens_vp", i)), v, un, dpi, 74.0, on && vp) {
                ch = true;
            }
        });
    }
    if vp {
        l.viewpoint = Some(p);
    }
    if remove && cur.is_some() {
        edit_sel(app, "Clear Lens", |o| o.effects.lens = None);
    } else if ch && sel && kind > 0 {
        edit_sel(app, "Lens", |o| o.effects.lens = Some(l.clone()));
    }
}

/// The lens preview: a lens over a sample, seen through the lens's settings.
fn lens_preview(p: &egui::Painter, r: egui::Rect, l: Option<&LensEffect>) {
    use egui::{pos2, vec2, Color32};
    let Some(l) = l else {
        // no lens: a white card with a faint lens on it
        let card = egui::Rect::from_center_size(r.center(), vec2(r.height() * 0.9, r.height() * 0.9));
        p.rect_filled(card, 0.0, Color32::WHITE);
        p.circle_stroke(card.center(), card.height() * 0.15, egui::Stroke::new(1.0, Color32::from_gray(200)));
        p.circle_filled(card.center(), 1.5, Color32::from_gray(200));
        return;
    };
    let (w, h) = (r.width(), r.height());
    // the sample: a metal bar across a white card
    let sample = |x: f32, y: f32| -> [f32; 3] {
        let (top, bot) = (h * 0.22, h * 0.78);
        if y < top || y > bot || x < w * 0.04 || x > w * 0.96 {
            return [255.0; 3];
        }
        let t = ((y - top) / (bot - top) - 0.5).abs() * 2.0;
        let v = 255.0 * (1.0 - t.powf(0.7));
        [v; 3]
    };
    let c = (w / 2.0, h / 2.0);
    let rad = h * 0.46;
    let cell = 2.0;
    let mut y = 0.0;
    while y < h {
        let mut x = 0.0;
        while x < w {
            let (dx, dy) = (x + 1.0 - c.0, y + 1.0 - c.1);
            let d = (dx * dx + dy * dy).sqrt();
            let mut rgb = sample(x + 1.0, y + 1.0);
            if d < rad {
                rgb = match l.kind {
                    LensKind::Magnify => sample(c.0 + dx / l.amount.max(0.01) as f32, c.1 + dy / l.amount.max(0.01) as f32),
                    LensKind::FishEye => {
                        let rate = (l.rate / 100.0) as f32;
                        let k = if rate >= 0.0 { 1.0 + rate } else { 1.0 / (1.0 - rate) };
                        let rn = (d / rad).max(1e-3);
                        let f = rn.powf(k) / rn;
                        sample(c.0 + dx * f, c.1 + dy * f)
                    }
                    LensKind::Wireframe => {
                        let edge = (sample(x + 1.0, y - 0.5)[0] - sample(x + 1.0, y + 2.5)[0]).abs() > 120.0 && rgb[0] < 255.0;
                        let col = if edge { l.outline } else if rgb[0] < 255.0 { l.fill } else { None };
                        col.map(|c| [c.r as f32, c.g as f32, c.b as f32]).unwrap_or(rgb)
                    }
                    LensKind::BitmapEffect => rgb,
                    _ => vector_core::object::lens_color(l, rgb),
                };
            }
            let col = Color32::from_rgb(rgb[0] as u8, rgb[1] as u8, rgb[2] as u8);
            p.rect_filled(egui::Rect::from_min_size(pos2(r.left() + x, r.top() + y), vec2(cell, cell)), 0.0, col);
            x += cell;
        }
        y += cell;
    }
    p.circle_stroke(pos2(r.left() + c.0, r.top() + c.1), rad, egui::Stroke::new(1.0, Color32::BLACK));
}

pub fn envelope(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        no_doc(ui);
        return;
    }
    let sel = app.has_selection();
    // the settings of the selected envelope
    let sid = app.doc().and_then(|d| d.selection.first().copied());
    if sid != app.dockers.ui.env_for {
        app.dockers.ui.env_for = sid;
        app.dockers.ui.env_preset = None;
        if let Some(e) = first(app).and_then(|o| o.effects.envelope) {
            let u = &mut app.dockers.ui;
            u.env_mode = e.mode;
            u.env_mapping = e.mapping as usize;
            u.env_keep = e.keep_lines;
        }
    }
    let full_w = ui.available_width();
    let (mode, mapping, keep) = (app.dockers.ui.env_mode, app.dockers.ui.env_mapping as u8, app.dockers.ui.env_keep);
    let add = ui.add_enabled_ui(sel, |ui| ui.add_sized(egui::vec2(full_w, 24.0), egui::Button::new("Add New").fill(egui::Color32::from_rgb(0x33, 0x33, 0x33)))).inner;
    if add.clicked() {
        crate::propbar::bake_warps_with(app, "Add New Envelope", Some(EnvelopeEffect { mode, mapping, keep_lines: keep, ..Default::default() }));
    }
    ui.add_space(2.0);
    ui.label(egui::RichText::new("Choose Preset").color(if sel { crate::theme::TEXT_DIM } else { crate::theme::DISABLED }));
    // the presets: thumbnails of their shapes, five to a row
    let mut picked = None;
    egui::Frame::new().stroke(egui::Stroke::new(1.0, crate::theme::SEP)).inner_margin(egui::Margin::same(2)).show(ui, |ui| {
        let w = full_w - 6.0;
        egui::ScrollArea::vertical().id_salt("env_presets").max_height(196.0).show(ui, |ui| {
            ui.set_width(w);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            let cw = (w - 14.0) / 5.0;
            ui.vertical(|ui| {
                for row in 0..ENV_PRESETS.div_ceil(5) {
                    ui.horizontal(|ui| {
                        for col in 0..5 {
                            let i = row * 5 + col;
                            if i >= ENV_PRESETS {
                                break;
                            }
                            let (r, resp) = ui.allocate_exact_size(egui::vec2(cw, 33.0), if sel { egui::Sense::click() } else { egui::Sense::hover() });
                            let on = app.dockers.ui.env_preset == Some(i);
                            if on {
                                ui.painter().rect_filled(r.shrink(1.0), 2.0, crate::theme::ACCENT);
                            } else if resp.hovered() && sel {
                                ui.painter().rect_filled(r.shrink(1.0), 2.0, crate::theme::HOVER);
                            }
                            let col = if sel { crate::theme::TEXT } else { egui::Color32::from_gray(0x9a) };
                            for (pts, closed) in vector_core::geom::flatten_polylines(&envelope_preset(i), 0.2) {
                                let pts: Vec<egui::Pos2> = pts.iter().map(|p| r.center() + egui::vec2(p.x as f32, p.y as f32)).collect();
                                let shape = if closed { egui::Shape::closed_line(pts, egui::Stroke::new(1.0, col)) } else { egui::Shape::line(pts, egui::Stroke::new(1.0, col)) };
                                ui.painter().add(shape);
                            }
                            if resp.clicked() {
                                picked = Some(i);
                            }
                        }
                    });
                }
            });
        });
    });
    if let Some(i) = picked {
        app.dockers.ui.env_preset = Some(i);
        app.dockers.ui.env_mode = 3;
        if let Some(handles) = crate::tools::envelope_from_shape(&envelope_preset(i)) {
            let env = EnvelopeEffect { handles, mode: 3, mapping, keep_lines: keep };
            edit_sel(app, "Envelope", |o| {
                if !o.is_container() {
                    o.effects.envelope = Some(env.clone());
                }
            });
        }
    }
    ui.add_space(6.0);
    separator(ui);
    ui.add_space(4.0);
    // edit mode and mapping mode
    let mut new_mode = None;
    let mut new_map = None;
    ui.horizontal(|ui| {
        for (m, icon, tip) in [(3, "env_unconstrained", "Unconstrained mode"), (0, "env_straight", "Straight line mode"), (1, "env_single", "Single-arc mode"), (2, "env_double", "Double-arc mode")] {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, app.dockers.ui.env_mode == m).clicked() {
                new_mode = Some(m);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut m = app.dockers.ui.env_mapping;
            if choice(ui, "env_mapping", &mut m, &["Horizontal", "Original", "Putty", "Vertical"], 74.0) {
                new_map = Some(m);
            }
        });
    });
    if let Some(m) = new_mode {
        app.dockers.ui.env_mode = m;
        edit_sel(app, "Envelope", |o| {
            if let Some(e) = &mut o.effects.envelope {
                e.mode = m;
            }
        });
    }
    if let Some(m) = new_map {
        app.dockers.ui.env_mapping = m;
        edit_sel(app, "Envelope", |o| {
            if let Some(e) = &mut o.effects.envelope {
                e.mapping = m as u8;
            }
        });
    }
    ui.add_space(4.0);
    separator(ui);
    ui.add_space(4.0);
    // keep lines, create from, copy envelope properties
    let mut toggle_keep = false;
    ui.horizontal(|ui| {
        if crate::widgets::icon_button(&mut app.icons, ui, "keep_lines", "Keep lines", true, app.dockers.ui.env_keep).clicked() {
            toggle_keep = true;
        }
        if crate::widgets::icon_button(&mut app.icons, ui, "envelope_from", "Create envelope from", sel, false).clicked() {
            crate::propbar::press(app, "1a21e45c");
        }
        if crate::widgets::icon_button(&mut app.icons, ui, "copy_properties", "Copy envelope properties", sel, false).clicked() {
            crate::propbar::press(app, "1f8f2496");
        }
    });
    if toggle_keep {
        let v = !app.dockers.ui.env_keep;
        app.dockers.ui.env_keep = v;
        edit_sel(app, "Envelope", |o| {
            if let Some(e) = &mut o.effects.envelope {
                e.keep_lines = v;
            }
        });
    }
}

/// The number of envelope presets.
const ENV_PRESETS: usize = 30;

/// An envelope preset: its shape in a box about 40 × 30 around the origin (y down). The envelope
/// takes the shape's outline as for Create Envelope From.
fn envelope_preset(i: usize) -> kurbo::BezPath {
    use kurbo::{BezPath, Point, Shape, Vec2};
    use std::f64::consts::{PI, TAU};
    fn poly(pts: &[(f64, f64)]) -> BezPath {
        let mut b = BezPath::new();
        for (k, p) in pts.iter().enumerate() {
            if k == 0 {
                b.move_to(*p);
            } else {
                b.line_to(*p);
            }
        }
        b.close_path();
        b
    }
    fn ngon(n: usize, r: f64, rot: f64) -> BezPath {
        let pts: Vec<(f64, f64)> = (0..n).map(|k| rot + k as f64 * TAU / n as f64).map(|a| (a.cos() * r, a.sin() * r)).collect();
        poly(&pts)
    }
    // corners joined by edges bowed in toward the center by `bow` at their middles
    fn bowed(corners: &[(f64, f64)], bow: &[f64]) -> BezPath {
        let mut b = BezPath::new();
        b.move_to(corners[0]);
        for k in 0..corners.len() {
            let (p0, p1) = (Point::from(corners[k]), Point::from(corners[(k + 1) % corners.len()]));
            let m = p0.midpoint(p1);
            b.quad_to(m + (Point::ORIGIN - m).normalize() * (2.0 * bow[k]), p1);
        }
        b.close_path();
        b
    }
    // corners joined by wavy edges
    fn wavy(corners: &[(f64, f64)], amp: f64) -> BezPath {
        let mut b = BezPath::new();
        b.move_to(corners[0]);
        for k in 0..corners.len() {
            let (p0, p1) = (Point::from(corners[k]), Point::from(corners[(k + 1) % corners.len()]));
            let d = p1 - p0;
            let n = Vec2::new(-d.y, d.x).normalize() * (2.0 * amp);
            b.quad_to(p0.lerp(p1, 0.25) + n, p0.midpoint(p1));
            b.quad_to(p0.lerp(p1, 0.75) - n, p1);
        }
        b.close_path();
        b
    }
    // an arc of the circle at `c`, from `a0` degrees (y down) turning `sweep` degrees
    fn arc(b: &mut BezPath, c: (f64, f64), r: f64, a0: f64, sweep: f64) {
        let a = kurbo::Arc::new(c, (r, r), a0.to_radians(), sweep.to_radians(), 0.0);
        b.line_to(a.center + Vec2::from_angle(a0.to_radians()) * r);
        for el in a.append_iter(0.05) {
            b.push(el);
        }
    }
    // a circle drawn out to a point
    fn tear(c: (f64, f64), r: f64, tip: (f64, f64)) -> BezPath {
        let (dx, dy) = (tip.0 - c.0, tip.1 - c.1);
        let base = dy.atan2(dx).to_degrees();
        let h = (r / dx.hypot(dy)).acos().to_degrees();
        let mut b = BezPath::new();
        b.move_to(tip);
        arc(&mut b, c, r, base + h, 360.0 - 2.0 * h);
        b.close_path();
        b
    }
    match i {
        0 | 1 => poly(&[(-13.0, -13.0), (13.0, -13.0), (13.0, 13.0), (-13.0, 13.0)]),
        2 => kurbo::Circle::new((0.0, 0.0), 13.0).to_path(0.05),
        3 => kurbo::RoundedRect::new(-18.0, -11.0, 18.0, 11.0, 10.0).to_path(0.05),
        4 => poly(&[(-10.0, -9.0), (18.0, -9.0), (10.0, 9.0), (-18.0, 9.0)]),
        5 => poly(&[(-18.0, -9.0), (10.0, -9.0), (18.0, 9.0), (-10.0, 9.0)]),
        6 => ngon(6, 14.0, 0.0),
        7 => ngon(8, 13.0, -PI / 2.0),
        8 => ngon(8, 13.5, PI / 8.0),
        9 => poly(&[(-18.0, 0.0), (-12.0, -9.0), (12.0, -9.0), (18.0, 0.0), (12.0, 9.0), (-12.0, 9.0)]),
        // flag
        10 => {
            let mut b = BezPath::new();
            b.move_to((-13.0, -10.0));
            b.curve_to((-4.0, -17.0), (4.0, -3.0), (13.0, -10.0));
            b.line_to((13.0, 10.0));
            b.curve_to((4.0, 17.0), (-4.0, 3.0), (-13.0, 10.0));
            b.close_path();
            b
        }
        // heart
        11 => {
            let mut b = BezPath::new();
            b.move_to((0.0, 13.0));
            b.curve_to((-6.0, 7.0), (-16.0, 1.0), (-15.0, -6.0));
            b.curve_to((-14.0, -13.0), (-4.0, -14.0), (0.0, -7.0));
            b.curve_to((4.0, -14.0), (14.0, -13.0), (15.0, -6.0));
            b.curve_to((16.0, 1.0), (6.0, 7.0), (0.0, 13.0));
            b.close_path();
            b
        }
        12 => bowed(&[(-14.0, -14.0), (14.0, -14.0), (14.0, 14.0), (-14.0, 14.0)], &[3.0; 4]),
        13 => bowed(&[(-14.0, -14.0), (14.0, -14.0), (14.0, 14.0), (-14.0, 14.0)], &[7.0; 4]),
        14 => bowed(&[(-18.0, -11.0), (18.0, -11.0), (18.0, 11.0), (-18.0, 11.0)], &[4.5, 1.0, 4.5, 1.0]),
        15 => bowed(&[(-17.0, -11.0), (17.0, -11.0), (17.0, 11.0), (-17.0, 11.0)], &[2.0, 1.5, 2.0, 1.5]),
        16 => wavy(&[(-18.0, -12.0), (18.0, -12.0), (18.0, 12.0), (-18.0, 12.0)], 1.5),
        17 => poly(&[(-18.0, 0.0), (0.0, -12.0), (18.0, 0.0), (0.0, 12.0)]),
        // lens
        18 => {
            let mut b = BezPath::new();
            b.move_to((-18.0, 0.0));
            b.quad_to((0.0, -22.0), (18.0, 0.0));
            b.quad_to((0.0, 22.0), (-18.0, 0.0));
            b.close_path();
            b
        }
        // a label: the corners cut in round
        19 => {
            let mut b = BezPath::new();
            b.move_to((-14.0, -13.0));
            b.line_to((14.0, -13.0));
            b.quad_to((14.0, -9.0), (18.0, -9.0));
            b.line_to((18.0, 9.0));
            b.quad_to((14.0, 9.0), (14.0, 13.0));
            b.line_to((-14.0, 13.0));
            b.quad_to((-14.0, 9.0), (-18.0, 9.0));
            b.line_to((-18.0, -9.0));
            b.quad_to((-14.0, -9.0), (-14.0, -13.0));
            b.close_path();
            b
        }
        20 => poly(&[(-10.0, -13.0), (10.0, -13.0), (10.0, -10.0), (13.0, -10.0), (13.0, 10.0), (10.0, 10.0), (10.0, 13.0), (-10.0, 13.0), (-10.0, 10.0), (-13.0, 10.0), (-13.0, -10.0), (-10.0, -10.0)]),
        21 => poly(&[
            (-6.0, -14.0),
            (6.0, -14.0),
            (6.0, -10.0),
            (11.0, -10.0),
            (11.0, -6.0),
            (18.0, -6.0),
            (18.0, 6.0),
            (11.0, 6.0),
            (11.0, 10.0),
            (6.0, 10.0),
            (6.0, 14.0),
            (-6.0, 14.0),
            (-6.0, 10.0),
            (-11.0, 10.0),
            (-11.0, 6.0),
            (-18.0, 6.0),
            (-18.0, -6.0),
            (-11.0, -6.0),
            (-11.0, -10.0),
            (-6.0, -10.0),
        ]),
        // cloud
        22 => {
            let pts: Vec<(f64, f64)> = (0..160)
                .map(|k| {
                    let a = k as f64 / 160.0 * TAU;
                    let s = 0.82 + 0.2 * (4.0 * a).sin().abs();
                    (a.cos() * 18.0 * s, a.sin() * 11.0 * s)
                })
                .collect();
            poly(&pts)
        }
        // speech bubble
        23 => {
            let mut b = BezPath::new();
            b.move_to((-12.0, -12.0));
            b.line_to((12.0, -12.0));
            b.quad_to((18.0, -12.0), (18.0, -6.0));
            b.line_to((18.0, 0.0));
            b.quad_to((18.0, 6.0), (12.0, 6.0));
            b.line_to((-1.0, 6.0));
            b.line_to((-7.0, 14.0));
            b.line_to((-5.0, 6.0));
            b.line_to((-12.0, 6.0));
            b.quad_to((-18.0, 6.0), (-18.0, 0.0));
            b.line_to((-18.0, -6.0));
            b.quad_to((-18.0, -12.0), (-12.0, -12.0));
            b.close_path();
            b
        }
        // a balloon on its string
        24 => tear((3.0, -4.0), 9.0, (-8.0, 14.0)),
        // light bulb
        25 => {
            let mut b = BezPath::new();
            b.move_to((-5.0, 13.0));
            arc(&mut b, (0.0, -3.0), 11.0, 117.0, 306.0);
            b.line_to((5.0, 13.0));
            b.close_path();
            b
        }
        // drop
        26 => tear((0.0, 5.0), 8.0, (0.0, -14.0)),
        // shield
        27 => {
            let mut b = BezPath::new();
            b.move_to((-12.0, -11.0));
            b.quad_to((0.0, -16.0), (12.0, -11.0));
            b.curve_to((12.0, 2.0), (6.0, 9.0), (0.0, 14.0));
            b.curve_to((-6.0, 9.0), (-12.0, 2.0), (-12.0, -11.0));
            b.close_path();
            b
        }
        // arrow
        28 => poly(&[(0.0, -14.0), (10.0, -3.0), (4.0, -3.0), (4.0, 14.0), (-4.0, 14.0), (-4.0, -3.0), (-10.0, -3.0)]),
        // thermometer
        _ => {
            let mut b = BezPath::new();
            b.move_to((-2.5, 6.26));
            b.line_to((-2.5, -12.0));
            b.quad_to((-2.5, -15.0), (0.0, -15.0));
            b.quad_to((2.5, -15.0), (2.5, -12.0));
            b.line_to((2.5, 6.26));
            arc(&mut b, (0.0, 10.0), 4.5, -56.25, 292.5);
            b.close_path();
            b
        }
    }
}
