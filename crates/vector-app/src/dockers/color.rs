//! Color docker.

use crate::app::VectorApp;
use egui::Ui;
use vector_core::Color;

/// The color of the selection the Color docker edits: its fill (or outline) color.
fn selection_color(app: &VectorApp, outline: bool) -> Option<Color> {
    let d = app.doc()?;
    let o = d.doc.find(*d.selection.first()?)?;
    if outline {
        o.style.outline.color
    } else {
        o.style.fill.main_color()
    }
}

pub fn color(app: &mut VectorApp, ui: &mut Ui) {
    let outline = app.dockers.ui.color_auto_outline;
    let reference = selection_color(app, outline).unwrap_or(app.default_fill);
    let mut c = app.dockers.ui.color_edit.unwrap_or(reference);
    let mut ed = app.dockers.ui.color_ed;
    ui.add_space(4.0);
    let mut changed = super::coled::editor(ui, &mut ed, &mut c, true);
    app.dockers.ui.color_ed = ed;
    ui.add_space(6.0);
    super::separator(ui);
    ui.add_space(6.0);
    let (ch, act) = super::coled::bar(ui, "color_docker", &mut c, Some(reference), false);
    changed |= ch;
    if act == super::coled::BarAction::Swap {
        c = reference;
        changed = true;
    }
    ui.add_space(4.0);
    let sel = app.has_selection();
    let mut apply: Option<bool> = None;
    ui.horizontal(|ui| {
        let auto = app.dockers.ui.color_auto;
        if super::icon_button(app, ui, if auto { "lock" } else { "unlock" }, "Auto-apply color", true) {
            app.dockers.ui.color_auto = !auto;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.dockers.ui.color_auto {
                // the target of the automatic changes
                if ui.add(egui::Button::selectable(app.dockers.ui.color_auto_outline, "Outline")).clicked() {
                    app.dockers.ui.color_auto_outline = true;
                }
                if ui.add(egui::Button::selectable(!app.dockers.ui.color_auto_outline, "Fill")).clicked() {
                    app.dockers.ui.color_auto_outline = false;
                }
            } else {
                if ui.button("Outline").on_hover_text("Apply the color to the outline of the selected objects").clicked() {
                    apply = Some(true);
                }
                if ui.button("Fill").on_hover_text("Apply the color to the fill of the selected objects").clicked() {
                    apply = Some(false);
                }
            }
        });
    });
    if changed && app.dockers.ui.color_auto && sel {
        apply = Some(app.dockers.ui.color_auto_outline);
    }
    app.dockers.ui.color_edit = Some(c);
    if let Some(o) = apply {
        crate::exec::apply_color(app, Some(c), o);
    }
}
