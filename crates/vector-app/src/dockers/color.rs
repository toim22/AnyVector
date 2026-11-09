//! Color, Palettes, Color Styles and Color Proofing dockers.

use super::{heading, hint, no_doc};
use crate::app::VectorApp;
use egui::Ui;
use vector_core::{Color, NamedColor};

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

pub fn palettes(app: &mut VectorApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if ui.button("Open Palette...").clicked() {
            app.run(crate::commands::Cmd::OpenPalette);
        }
        if ui.button("Palette Editor...").clicked() {
            app.run(crate::commands::Cmd::PaletteEditor);
        }
    });
    for name in ["Default Palette", "Grayscale", "RGB", "Web"] {
        if ui.button(format!("Add {name}")).clicked() {
            app.palettes.open_by_name(name);
        }
    }
    let pals = app.palettes.palettes.clone();
    let mut clicked = None;
    let mut close = None;
    for (pi, p) in pals.iter().enumerate() {
        ui.horizontal(|ui| {
            heading(ui, &p.name);
            if pi > 0 && ui.small_button("Close").clicked() {
                close = Some(pi);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(1.0, 1.0);
            for nc in &p.colors {
                let r = crate::widgets::color_well(ui, Some(nc.color), egui::vec2(16.0, 16.0)).on_hover_text(format!("{}\n{}", nc.name, nc.color.describe()));
                if r.clicked() {
                    clicked = Some((nc.color, false));
                }
                if r.secondary_clicked() {
                    clicked = Some((nc.color, true));
                }
            }
        });
    }
    if let Some(i) = close {
        app.palettes.palettes.remove(i);
    }
    if let Some((c, o)) = clicked {
        crate::exec::apply_color(app, Some(c), o);
    }
    if let Some(d) = app.doc() {
        heading(ui, "Document Palette");
        if d.doc.palette.is_empty() {
            hint(ui, "Empty");
        }
    }
}

pub fn color_styles(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    hint(ui, "Color styles are named colors; changing a style updates every object that uses it.");
    ui.horizontal(|ui| {
        if ui.add_enabled(app.has_selection(), egui::Button::new("New from selection")).clicked() {
            if let Some(d) = app.doc_mut() {
                let mut cols = vec![];
                for id in &d.selection {
                    if let Some(o) = d.doc.find(*id) {
                        o.walk(&mut |x| {
                            if let Some(c) = x.style.fill.main_color() {
                                cols.push(c);
                            }
                            if let Some(c) = x.style.outline.color {
                                cols.push(c);
                            }
                        });
                    }
                }
                d.checkpoint("New Color Styles");
                for c in cols {
                    if !d.doc.color_styles.iter().any(|s| s.color == c) {
                        let n = d.doc.color_styles.len() + 1;
                        d.doc.color_styles.push(NamedColor::new(format!("Color style {n}"), c));
                    }
                }
            }
        }
    });
    let Some(d) = app.doc() else { return };
    let styles = d.doc.color_styles.clone();
    let mut changed: Option<(usize, Color, Color)> = None;
    let mut apply: Option<(Color, bool)> = None;
    let mut delete = None;
    for (i, s) in styles.iter().enumerate() {
        ui.horizontal(|ui| {
            let mut c = s.color;
            if super::props::color_edit(ui, &mut c, false) {
                changed = Some((i, s.color, c));
            }
            ui.label(&s.name);
            if ui.small_button("Fill").clicked() {
                apply = Some((s.color, false));
            }
            if ui.small_button("Outline").clicked() {
                apply = Some((s.color, true));
            }
            if ui.small_button("×").on_hover_text("Delete style").clicked() {
                delete = Some(i);
            }
        });
    }
    if let Some((i, old, new)) = changed {
        if let Some(d) = app.doc_mut() {
            d.modify("Edit Color Style", |doc, _| {
                doc.color_styles[i].color = new;
                for p in doc.pages.iter_mut() {
                    for l in p.layers.iter_mut() {
                        for o in l.objects.iter_mut() {
                            o.walk_mut(&mut |x| {
                                if x.style.fill == vector_core::Fill::Solid(old) {
                                    x.style.fill = vector_core::Fill::Solid(new);
                                }
                                if x.style.outline.color == Some(old) {
                                    x.style.outline.color = Some(new);
                                }
                            });
                        }
                    }
                }
            });
        }
    }
    if let Some((c, o)) = apply {
        crate::exec::apply_color(app, Some(c), o);
    }
    if let Some(i) = delete {
        if let Some(d) = app.doc_mut() {
            d.modify("Delete Color Style", |doc, _| {
                doc.color_styles.remove(i);
            });
        }
    }
}

pub fn proofing(app: &mut VectorApp, ui: &mut Ui) {
    hint(ui, "Preview how the document will look when printed or exported.");
    ui.checkbox(&mut app.settings.proof_colors, "Proof colors");
    ui.add_enabled(app.settings.proof_colors, egui::Checkbox::new(&mut app.dockers.ui.proof_gray, "Simulate grayscale output"));
    if let Some(d) = app.doc() {
        heading(ui, "Out-of-gamut colors");
        let used = d.doc.used_colors();
        let n = used.iter().filter(|c| out_of_cmyk_gamut(**c)).count();
        hint(ui, &format!("{} colors used, {} may shift in CMYK output.", used.len(), n));
    }
}

/// Rough CMYK gamut test: very saturated bright RGB primaries cannot be reproduced.
fn out_of_cmyk_gamut(c: Color) -> bool {
    let [_, s, v] = c.to_hsb();
    s > 85.0 && v > 90.0 && {
        let h = c.to_hsb()[0];
        !(40.0..=70.0).contains(&h)
    }
}
