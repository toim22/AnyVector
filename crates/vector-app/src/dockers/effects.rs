//! Effects dockers: Contour, Blend, Extrude, Bevel, Lens, Envelope, Distort, Shadow, Bitmap Effects.

use super::{hint, len_field, no_doc, primary};
use crate::app::VectorApp;
use crate::propbar::edit_sel;
use egui::Ui;
use vector_core::object::{ContourDir, ContourEffect, EnvelopeEffect, ExtrudeEffect, LensEffect};
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
    let cur = first(app).and_then(|o| o.effects.contour);
    let u = &mut app.dockers.ui;
    if let Some(c) = &cur {
        u.contour_steps = c.steps;
        u.contour_offset = c.offset;
        u.contour_dir = match c.dir {
            ContourDir::ToCenter => 0,
            ContourDir::Inside => 1,
            ContourDir::Outside => 2,
        };
    }
    ui.horizontal(|ui| {
        for (i, n) in ["To center", "Inside", "Outside"].iter().enumerate() {
            ui.radio_value(&mut app.dockers.ui.contour_dir, i, *n);
        }
    });
    ui.add(egui::Slider::new(&mut app.dockers.ui.contour_steps, 1..=100).text("Steps"));
    let mut off = app.dockers.ui.contour_offset;
    ui.horizontal(|ui| {
        ui.label("Offset:");
        len_field(app, ui, "ct_off", &mut off);
    });
    app.dockers.ui.contour_offset = off.max(0.1);
    let mut end_fill = cur.as_ref().map(|c| c.end_fill).unwrap_or(Color::WHITE);
    let mut end_ol = cur.as_ref().map(|c| c.end_outline).unwrap_or(Color::BLACK);
    ui.horizontal(|ui| {
        ui.label("Fill color:");
        super::props::color_edit(ui, &mut end_fill, false);
        ui.label("Outline color:");
        super::props::color_edit(ui, &mut end_ol, false);
    });
    ui.horizontal(|ui| {
        if primary(ui, "Apply", app.has_selection()) {
            let dir = [ContourDir::ToCenter, ContourDir::Inside, ContourDir::Outside][app.dockers.ui.contour_dir];
            let c = ContourEffect { dir, steps: app.dockers.ui.contour_steps, offset: app.dockers.ui.contour_offset, end_fill, end_outline: end_ol, ..Default::default() };
            edit_sel(app, "Contour", |o| o.effects.contour = Some(c.clone()));
        }
        if ui.add_enabled(cur.is_some(), egui::Button::new("Clear")).clicked() {
            edit_sel(app, "Clear Contour", |o| o.effects.contour = None);
        }
    });
}

pub fn blend(app: &mut VectorApp, ui: &mut Ui) {
    if !guard(app, ui) {
        return;
    }
    let is_blend = first(app).map(|o| matches!(o.kind, ObjKind::Blend(_))).unwrap_or(false);
    ui.add(egui::Slider::new(&mut app.dockers.ui.blend_steps, 1..=500).text("Steps"));
    let n = app.doc().map(|d| d.selection.len()).unwrap_or(0);
    hint(ui, "Select two objects to blend between them, or drag from one object to another with the Blend tool.");
    ui.horizontal(|ui| {
        if primary(ui, "Apply", n == 2 || is_blend) {
            let steps = app.dockers.ui.blend_steps;
            if is_blend {
                edit_sel(app, "Blend", |o| {
                    if let ObjKind::Blend(b) = &mut o.kind {
                        b.steps = steps;
                    }
                });
            } else if let Some(d) = app.doc_mut() {
                let (a, b) = (d.selection[0], d.selection[1]);
                d.modify("Blend", |doc, sel| {
                    if let Some(id) = vector_core::ops::create_blend(doc, a, b, steps) {
                        *sel = vec![id];
                    }
                });
            }
        }
        if ui.add_enabled(is_blend, egui::Button::new("Break Apart")).clicked() {
            app.run(crate::commands::Cmd::BreakBlend);
        }
    });
}

pub fn extrude(app: &mut VectorApp, ui: &mut Ui) {
    if !guard(app, ui) {
        return;
    }
    let cur = first(app).and_then(|o| o.effects.extrude);
    let mut e = cur.clone().unwrap_or(ExtrudeEffect { depth: kurbo::Vec2::new(18.0, -18.0), color: Color::from_hex("#808080").unwrap(), shade: Color::BLACK, steps: 24, ..Default::default() });
    ui.horizontal(|ui| {
        ui.label("Depth X:");
        len_field(app, ui, "ex_dx", &mut e.depth.x);
    });
    ui.horizontal(|ui| {
        ui.label("Depth Y:");
        len_field(app, ui, "ex_dy", &mut e.depth.y);
    });
    ui.add(egui::Slider::new(&mut e.steps, 2..=200).text("Smoothness"));
    ui.horizontal(|ui| {
        ui.label("Color:");
        super::props::color_edit(ui, &mut e.color, false);
        ui.label("Shade:");
        super::props::color_edit(ui, &mut e.shade, false);
    });
    ui.horizontal(|ui| {
        if primary(ui, "Apply", app.has_selection()) {
            edit_sel(app, "Extrude", |o| o.effects.extrude = Some(e.clone()));
        }
        if ui.add_enabled(cur.is_some(), egui::Button::new("Clear")).clicked() {
            edit_sel(app, "Clear Extrude", |o| o.effects.extrude = None);
        }
    });
}

pub fn bevel(app: &mut VectorApp, ui: &mut Ui) {
    if !guard(app, ui) {
        return;
    }
    let cur = first(app).and_then(|o| o.effects.bevel);
    if let Some((d, a)) = cur {
        app.dockers.ui.bevel_depth = d;
        app.dockers.ui.bevel_angle = a;
    }
    let mut d = app.dockers.ui.bevel_depth;
    ui.horizontal(|ui| {
        ui.label("Bevel width:");
        len_field(app, ui, "bv_d", &mut d);
    });
    app.dockers.ui.bevel_depth = d.max(0.5);
    ui.add(egui::Slider::new(&mut app.dockers.ui.bevel_angle, 0.0..=360.0).text("Light direction °"));
    ui.horizontal(|ui| {
        if primary(ui, "Apply", app.has_selection()) {
            let v = (app.dockers.ui.bevel_depth, app.dockers.ui.bevel_angle);
            edit_sel(app, "Bevel", |o| o.effects.bevel = Some(v));
        }
        if ui.add_enabled(cur.is_some(), egui::Button::new("Clear")).clicked() {
            edit_sel(app, "Clear Bevel", |o| o.effects.bevel = None);
        }
    });
}

pub fn lens(app: &mut VectorApp, ui: &mut Ui) {
    if !guard(app, ui) {
        return;
    }
    let names = ["Magnify", "Brighten", "Invert", "Color add", "Grayscale"];
    let cur = first(app).and_then(|o| o.effects.lens);
    egui::ComboBox::from_id_salt("lens_kind").selected_text(names[app.dockers.ui.lens_kind]).show_ui(ui, |ui| {
        for (i, n) in names.iter().enumerate() {
            ui.selectable_value(&mut app.dockers.ui.lens_kind, i, *n);
        }
    });
    let k = app.dockers.ui.lens_kind;
    if k == 0 {
        ui.add(egui::Slider::new(&mut app.dockers.ui.lens_amount, 1.0..=20.0).text("Amount ×"));
    } else if k == 1 {
        ui.add(egui::Slider::new(&mut app.dockers.ui.lens_amount, -100.0..=100.0).text("Rate %"));
    }
    let mut add = Color::from_hex("#ff0000").unwrap();
    if k == 3 {
        if let Some(LensEffect::ColorAdd(c)) = cur {
            add = c;
        }
        super::props::color_edit(ui, &mut add, false);
    }
    hint(ui, "The lens changes how objects behind it appear.");
    ui.horizontal(|ui| {
        if primary(ui, "Apply", app.has_selection()) {
            let a = app.dockers.ui.lens_amount;
            let l = match k {
                0 => LensEffect::Magnify(a.max(1.0)),
                1 => LensEffect::Brighten(a.clamp(-100.0, 100.0)),
                2 => LensEffect::Invert,
                3 => LensEffect::ColorAdd(add),
                _ => LensEffect::Grayscale,
            };
            edit_sel(app, "Lens", |o| o.effects.lens = Some(l.clone()));
        }
        if ui.add_enabled(cur.is_some(), egui::Button::new("Clear")).clicked() {
            edit_sel(app, "Clear Lens", |o| o.effects.lens = None);
        }
    });
}

pub fn envelope(app: &mut VectorApp, ui: &mut Ui) {
    if !guard(app, ui) {
        return;
    }
    let cur = first(app).and_then(|o| o.effects.envelope);
    hint(ui, "Add an envelope, then drag its handles with the Envelope tool.");
    let presets: [(&str, [kurbo::Vec2; 8]); 4] = [
        ("Arch", [kurbo::Vec2::new(0.0, 0.0), kurbo::Vec2::new(0.0, -0.25), kurbo::Vec2::new(0.0, 0.0), kurbo::Vec2::ZERO, kurbo::Vec2::new(0.0, 0.0), kurbo::Vec2::new(0.0, -0.25), kurbo::Vec2::new(0.0, 0.0), kurbo::Vec2::ZERO]),
        ("Bulge", [kurbo::Vec2::ZERO, kurbo::Vec2::new(0.0, -0.2), kurbo::Vec2::ZERO, kurbo::Vec2::new(0.2, 0.0), kurbo::Vec2::ZERO, kurbo::Vec2::new(0.0, 0.2), kurbo::Vec2::ZERO, kurbo::Vec2::new(-0.2, 0.0)]),
        ("Pinch", [kurbo::Vec2::ZERO, kurbo::Vec2::new(0.0, 0.2), kurbo::Vec2::ZERO, kurbo::Vec2::new(-0.2, 0.0), kurbo::Vec2::ZERO, kurbo::Vec2::new(0.0, -0.2), kurbo::Vec2::ZERO, kurbo::Vec2::new(0.2, 0.0)]),
        ("Perspective", [kurbo::Vec2::new(0.2, 0.0), kurbo::Vec2::ZERO, kurbo::Vec2::new(-0.2, 0.0), kurbo::Vec2::ZERO, kurbo::Vec2::ZERO, kurbo::Vec2::ZERO, kurbo::Vec2::ZERO, kurbo::Vec2::ZERO]),
    ];
    ui.horizontal_wrapped(|ui| {
        if ui.button("Add New").clicked() {
            edit_sel(app, "Envelope", |o| o.effects.envelope = Some(EnvelopeEffect { handles: [kurbo::Vec2::ZERO; 8], ..Default::default() }));
        }
        for (n, h) in presets {
            if ui.button(n).clicked() {
                edit_sel(app, "Envelope", |o| o.effects.envelope = Some(EnvelopeEffect { handles: h, ..Default::default() }));
            }
        }
    });
    if ui.add_enabled(cur.is_some(), egui::Button::new("Clear Envelope")).clicked() {
        edit_sel(app, "Clear Envelope", |o| o.effects.envelope = None);
    }
}
