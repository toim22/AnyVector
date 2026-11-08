//! Color, Palettes, Color Styles and Color Proofing dockers.

use super::{heading, hint, no_doc, primary};
use crate::app::VectorApp;
use crate::theme;
use egui::Ui;
use vector_core::{Color, NamedColor};

pub fn color(app: &mut VectorApp, ui: &mut Ui) {
    let mut c = app.dockers.ui.color_edit.unwrap_or(app.default_fill);
    let names = ["RGB", "CMYK", "HSB"];
    ui.horizontal(|ui| {
        for (i, n) in names.iter().enumerate() {
            ui.selectable_value(&mut app.dockers.ui.color_model, i, *n);
        }
    });
    let mut c32 = theme::to_c32(c);
    if egui::color_picker::color_picker_color32(ui, &mut c32, egui::color_picker::Alpha::OnlyBlend) {
        c = theme::from_c32(c32);
    }
    match app.dockers.ui.color_model {
        0 => {
            let (mut r, mut g, mut b) = (c.r, c.g, c.b);
            let mut ch = ui.add(egui::Slider::new(&mut r, 0..=255).text("R")).changed();
            ch |= ui.add(egui::Slider::new(&mut g, 0..=255).text("G")).changed();
            ch |= ui.add(egui::Slider::new(&mut b, 0..=255).text("B")).changed();
            if ch {
                c = Color { r, g, b, a: c.a };
            }
        }
        1 => {
            let [mut cc, mut m, mut y, mut k] = c.to_cmyk();
            let mut ch = ui.add(egui::Slider::new(&mut cc, 0.0..=100.0).text("C")).changed();
            ch |= ui.add(egui::Slider::new(&mut m, 0.0..=100.0).text("M")).changed();
            ch |= ui.add(egui::Slider::new(&mut y, 0.0..=100.0).text("Y")).changed();
            ch |= ui.add(egui::Slider::new(&mut k, 0.0..=100.0).text("K")).changed();
            if ch {
                c = Color::from_cmyk(cc, m, y, k).with_alpha(c.a);
            }
        }
        _ => {
            let [mut h, mut s, mut v] = c.to_hsb();
            let mut ch = ui.add(egui::Slider::new(&mut h, 0.0..=360.0).text("H")).changed();
            ch |= ui.add(egui::Slider::new(&mut s, 0.0..=100.0).text("S")).changed();
            ch |= ui.add(egui::Slider::new(&mut v, 0.0..=100.0).text("B")).changed();
            if ch {
                c = Color::from_hsb(h, s, v).with_alpha(c.a);
            }
        }
    }
    let mut hex = c.to_hex();
    ui.horizontal(|ui| {
        ui.label("Hex:");
        if ui.add(egui::TextEdit::singleline(&mut hex).desired_width(90.0)).lost_focus() {
            if let Some(n) = Color::from_hex(&hex) {
                c = n;
            }
        }
    });
    app.dockers.ui.color_edit = Some(c);
    ui.horizontal(|ui| {
        if primary(ui, "Fill", app.has_selection()) {
            crate::exec::apply_color(app, Some(c), false);
        }
        if ui.add_enabled(app.has_selection(), egui::Button::new("Outline")).clicked() {
            crate::exec::apply_color(app, Some(c), true);
        }
        if ui.add_enabled(app.has_doc(), egui::Button::new("Add to document palette")).clicked() {
            if let Some(d) = app.doc_mut() {
                d.checkpoint("Add Color");
                d.doc.palette.push(NamedColor::new(c.to_hex(), c));
            }
        }
    });
    if !app.has_selection() {
        hint(ui, "With nothing selected, Fill sets the default fill for new objects.");
        if ui.button("Set as default fill").clicked() {
            app.default_fill = c;
        }
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
