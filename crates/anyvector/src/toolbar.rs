//! The standard toolbar and the optional toolbars (Window ▸ Toolbars), drawn from the reference
//! data by `crate::cmdbar`, and the zoom level field they share.

use crate::app::{Toolbar, AnyVectorApp};
use crate::commands::Cmd;
use crate::theme;
use egui::{vec2, Popup, Ui};

/// The standard toolbar, from the reference data.
pub fn standard(app: &mut AnyVectorApp, ui: &mut Ui) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        ui.add_space(2.0);
        if let Some(b) = crate::cmdbar::toolbar_bar(Toolbar::Standard) {
            crate::cmdbar::strip(app, ui, b);
        }
    });
}

pub fn zoom_combo(app: &mut AnyVectorApp, ui: &mut Ui, doc: bool) {
    let id = ui.make_persistent_id("zoom_combo");
    let pct = app.doc().map(|d| d.zoom_percent()).unwrap_or(100.0);
    let mut text = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_default();
    let focused = ui.memory(|m| m.has_focus(id.with("te")));
    if !focused {
        text = format!("{}%", (pct.round() as i64));
    }
    let (rect, _) = ui.allocate_exact_size(vec2(84.0, 24.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, theme::FIELD());
    let te_rect = egui::Rect::from_min_max(rect.min + vec2(2.0, 2.0), egui::pos2(rect.max.x - 18.0, rect.max.y - 2.0));
    let r = ui.place(
        te_rect,
        egui::TextEdit::singleline(&mut text).id(id.with("te")).frame(egui::Frame::NONE).text_color(if doc { theme::TEXT() } else { theme::DISABLED() }).interactive(doc),
    );
    if !doc {
        // the screenshot shows "100%" selected (highlighted) in the empty state
        let g = ui.painter().layout_no_wrap("100%".into(), egui::FontId::proportional(12.5), egui::Color32::WHITE);
        let tr = egui::Rect::from_min_size(te_rect.left_center() - vec2(-2.0, g.size().y / 2.0), g.size());
        ui.painter().rect_filled(tr, 0.0, theme::FOCUS());
        ui.painter().galley(tr.min, g, egui::Color32::WHITE);
    }
    if r.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }
    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        if let Ok(v) = text.trim().trim_end_matches('%').trim().parse::<f64>() {
            app.run(Cmd::ZoomSet(v));
        }
    }
    let arrow = egui::Rect::from_min_max(egui::pos2(rect.max.x - 18.0, rect.min.y), rect.max);
    let ar = ui.interact(arrow, id.with("arrow"), if doc { egui::Sense::click() } else { egui::Sense::hover() });
    crate::widgets::chevron(ui.painter(), arrow.center(), 3.5, if doc { theme::TEXT() } else { theme::DISABLED() });
    Popup::menu(&ar).show(|ui| {
        ui.set_min_width(140.0);
        for (label, cmd) in [
            ("To Page", Cmd::ZoomToPage),
            ("To Width", Cmd::ZoomToWidth),
            ("To Height", Cmd::ZoomToHeight),
            ("To Selected", Cmd::ZoomToSelected),
            ("To All Objects", Cmd::ZoomToAll),
        ] {
            if ui.button(label).clicked() {
                app.run(cmd);
            }
        }
        ui.separator();
        for z in [10.0, 25.0, 50.0, 75.0, 100.0, 200.0, 400.0, 800.0, 1600.0] {
            if ui.button(format!("{z}%")).clicked() {
                app.run(Cmd::ZoomSet(z));
            }
        }
    });
}

/// An optional toolbar (Window ▸ Toolbars), from the reference data.
pub fn extra(app: &mut AnyVectorApp, ui: &mut Ui, tb: Toolbar) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(1.0, 2.0);
        if let Some(b) = crate::cmdbar::toolbar_bar(tb) {
            crate::cmdbar::strip(app, ui, b);
        }
    });
}
