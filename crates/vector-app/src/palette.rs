//! Color palette column (right edge) and the document palette strip (bottom).

use crate::app::VectorApp;
use crate::settings::Settings;
use crate::theme;
use crate::widgets::paint_well;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use vector_core::color::{NamedColor, Palette};
use vector_core::Color;

/// Drag-and-drop payload for colors.
#[derive(Clone, Debug)]
pub struct ColorDrag(pub Option<Color>);

pub struct PaletteState {
    pub palettes: Vec<Palette>,
    pub scroll: usize,
    pub recent: Vec<String>,
    /// Color being sampled with the palette eyedropper.
    pub eyedropper: bool,
}

impl PaletteState {
    pub fn new(s: &Settings) -> PaletteState {
        let mut palettes = vec![Palette::default_palette()];
        for p in &s.open_palettes {
            if let Ok(t) = std::fs::read_to_string(p) {
                let name = std::path::Path::new(p).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                palettes.push(Palette::parse(&name, &t));
            }
        }
        PaletteState { palettes, scroll: 0, recent: vec![], eyedropper: false }
    }

    pub fn open_by_name(&mut self, name: &str) {
        let p = match name {
            "Default Palette" => Palette::default_palette(),
            "Grayscale" => Palette::grayscale(),
            "RGB" => Palette::rgb_palette(),
            "Web" => Palette::web_palette(),
            _ => return,
        };
        if !self.palettes.iter().any(|x| x.name == p.name) {
            self.palettes.push(p);
        }
    }

    pub fn colors(&self) -> Vec<NamedColor> {
        self.palettes.first().map(|p| p.colors.clone()).unwrap_or_default()
    }
}

pub fn column_width(s: &Settings) -> f32 {
    let w = if s.large_swatches { 26.0 } else { 18.0 };
    w * s.palette_rows.max(1) as f32 + 4.0
}

fn swatch_h(s: &Settings) -> f32 {
    if s.large_swatches {
        22.0
    } else {
        16.0
    }
}

pub fn palette_column(app: &mut VectorApp, ui: &mut Ui) {
    let full = ui.available_rect_before_wrap();
    let cols = app.settings.palette_rows.max(1) as usize;
    let sw = (full.width() - 2.0) / cols as f32;
    let sh = swatch_h(&app.settings);
    let mut y = full.top();
    let x0 = full.left() + 1.0;
    let p = ui.painter().clone();
    // dock arrow ›
    let r = Rect::from_min_size(pos2(x0, y), vec2(full.width() - 2.0, 14.0));
    let resp = ui.interact(r, ui.scope_id().with("pal_menu"), Sense::click()).on_hover_text("Palette options");
    crate::widgets::chevron_right(&p, r.center(), 3.0, if resp.hovered() { Color32::WHITE } else { theme::TEXT_DIM });
    egui::Popup::menu(&resp).show(|ui| palette_options_menu(app, ui));
    y += 16.0;
    // eyedropper
    let r = Rect::from_min_size(pos2(x0, y), vec2(full.width() - 2.0, 18.0));
    let resp = ui.interact(r, ui.scope_id().with("pal_eye"), Sense::click()).on_hover_text("Eyedropper: sample a color and apply it to the selection");
    if resp.hovered() || app.palettes.eyedropper {
        p.rect_filled(r, 2.0, if app.palettes.eyedropper { theme::PRESSED } else { theme::HOVER });
    }
    if let Some(t) = app.icons.get(ui.ctx(), "eyedropper", 14.0, crate::icons::Tone::Normal) {
        p.image(t.id, Rect::from_center_size(r.center(), vec2(14.0, 14.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    if resp.clicked() && app.has_doc() {
        app.palettes.eyedropper = !app.palettes.eyedropper;
    }
    y += 19.0;
    // scroll up
    let r = Rect::from_min_size(pos2(x0, y), vec2(full.width() - 2.0, 12.0));
    let up = ui.interact(r, ui.scope_id().with("pal_up"), Sense::click());
    crate::widgets::chevron_up(&p, r.center(), 3.0, if app.palettes.scroll > 0 { theme::TEXT } else { theme::DISABLED });
    if up.clicked() && app.palettes.scroll > 0 {
        app.palettes.scroll -= cols;
    }
    y += 13.0;
    let bottom_reserved = 30.0;
    let avail = full.bottom() - bottom_reserved - y;
    let rows_fit = ((avail / sh).floor() as usize).max(1);
    let mut colors: Vec<Option<NamedColor>> = vec![];
    if app.settings.show_no_color_well {
        colors.push(None);
    }
    colors.extend(app.palettes.colors().into_iter().map(Some));
    let total_rows = colors.len().div_ceil(cols);
    let max_scroll = total_rows.saturating_sub(rows_fit) * cols;
    app.palettes.scroll = app.palettes.scroll.min(max_scroll);
    // wheel scroll over the palette
    if ui.rect_contains_pointer(Rect::from_min_max(pos2(full.left(), y), pos2(full.right(), full.bottom() - bottom_reserved))) {
        let d = ui.input(|i| i.smooth_scroll_delta.y);
        if d < -1.0 {
            app.palettes.scroll = (app.palettes.scroll + cols).min(max_scroll);
        } else if d > 1.0 {
            app.palettes.scroll = app.palettes.scroll.saturating_sub(cols);
        }
    }
    let start = app.palettes.scroll;
    let mut clicked: Option<(Option<Color>, bool)> = None;
    for (k, c) in colors.iter().enumerate().skip(start).take(rows_fit * cols) {
        let i = k - start;
        let row = i / cols;
        let col = i % cols;
        let r = Rect::from_min_size(pos2(x0 + col as f32 * sw, y + row as f32 * sh), vec2(sw - if app.settings.wide_borders { 2.0 } else { 0.0 }, sh - 1.0));
        let id = ui.scope_id().with(("sw", k));
        let resp = ui.interact(r, id, Sense::click_and_drag());
        let color = c.as_ref().map(|nc| nc.color);
        paint_well(&p, r, color);
        if resp.hovered() {
            p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
        }
        let name = c.as_ref().map(|nc| nc.name.clone()).unwrap_or_else(|| "No Color".into());
        let tip = match color {
            Some(cc) => format!("{name}\n{}", cc.describe()),
            None => name,
        };
        let resp = resp.on_hover_text(tip);
        resp.dnd_set_drag_payload(ColorDrag(color));
        if resp.clicked() {
            clicked = Some((color, false));
        }
        if resp.secondary_clicked() {
            if app.settings.right_click_outline {
                clicked = Some((color, true));
            }
        }
        if !app.settings.right_click_outline {
            resp.context_menu(|ui| {
                if ui.button("Set Fill").clicked() {
                    clicked = Some((color, false));
                }
                if ui.button("Set Outline").clicked() {
                    clicked = Some((color, true));
                }
            });
        }
    }
    if let Some((c, outline)) = clicked {
        crate::exec::apply_color(app, c, outline);
    }
    // scroll down and expand
    let yb = full.bottom() - bottom_reserved + 2.0;
    let r = Rect::from_min_size(pos2(x0, yb), vec2(full.width() - 2.0, 12.0));
    let dn = ui.interact(r, ui.scope_id().with("pal_dn"), Sense::click());
    crate::widgets::chevron(&p, r.center(), 3.0, if app.palettes.scroll < max_scroll { theme::TEXT } else { theme::DISABLED });
    if dn.clicked() {
        app.palettes.scroll = (app.palettes.scroll + cols).min(max_scroll);
    }
    let r = Rect::from_min_size(pos2(x0, yb + 13.0), vec2(full.width() - 2.0, 12.0));
    let ex = ui.interact(r, ui.scope_id().with("pal_ex"), Sense::click()).on_hover_text("Expand palette");
    p.text(r.center(), egui::Align2::CENTER_CENTER, "»", egui::FontId::proportional(11.0), if ex.hovered() { Color32::WHITE } else { theme::TEXT_DIM });
    if ex.clicked() {
        app.settings.palette_rows = if app.settings.palette_rows >= 3 { 1 } else { app.settings.palette_rows + 1 };
    }
}

fn palette_options_menu(app: &mut VectorApp, ui: &mut Ui) {
    ui.set_min_width(200.0);
    for name in ["Default Palette", "Grayscale", "RGB", "Web"] {
        let active = app.palettes.palettes.first().map(|p| p.name == name).unwrap_or(false);
        if ui.selectable_label(active, name).clicked() {
            let p = match name {
                "Grayscale" => Palette::grayscale(),
                "RGB" => Palette::rgb_palette(),
                "Web" => Palette::web_palette(),
                _ => Palette::default_palette(),
            };
            app.palettes.palettes.retain(|x| x.name != p.name);
            app.palettes.palettes.insert(0, p);
            app.palettes.scroll = 0;
            if !app.palettes.recent.contains(&name.to_string()) {
                app.palettes.recent.push(name.to_string());
            }
            ui.close();
        }
    }
    ui.separator();
    if ui.button("Open Palette...").clicked() {
        app.run(crate::commands::Cmd::OpenPalette);
        ui.close();
    }
    if ui.button("Palette Editor...").clicked() {
        app.run(crate::commands::Cmd::PaletteEditor);
        ui.close();
    }
    ui.separator();
    ui.checkbox(&mut app.settings.large_swatches, "Large swatches");
    ui.checkbox(&mut app.settings.wide_borders, "Wide borders");
    ui.checkbox(&mut app.settings.show_no_color_well, "Show \"No Color\" well");
}

pub fn document_palette(app: &mut VectorApp, ui: &mut Ui) {
    let full = ui.available_rect_before_wrap();
    let p = ui.painter().clone();
    // drop zone for colors (and objects: their colors are added)
    let resp = ui.interact(full, ui.scope_id().with("docpal"), Sense::hover());
    let drop = resp.dnd_release_payload::<ColorDrag>();
    if let Some(c) = drop {
        if let (Some(color), Some(d)) = (c.0, app.doc_mut()) {
            if !d.doc.palette.iter().any(|n| n.color == color) {
                d.checkpoint("Add Color to Document Palette");
                d.doc.palette.push(NamedColor::new(color.to_hex(), color));
            }
        }
    }
    let colors: Vec<NamedColor> = app.doc().map(|d| d.doc.palette.clone()).unwrap_or_default();
    let right_w = 40.0;
    if colors.is_empty() {
        let hovering_drop = resp.dnd_hover_payload::<ColorDrag>().is_some();
        p.text(
            pos2(full.center().x - right_w / 2.0, full.center().y),
            egui::Align2::CENTER_CENTER,
            "Drag colors (or objects) here to store these colors with your document",
            egui::FontId::new(12.0, egui::FontFamily::Proportional),
            if hovering_drop { Color32::WHITE } else { theme::TEXT_DIM },
        );
        // italic look: draw via galley with italics is unavailable for system fonts; dim text matches.
    } else {
        let sw = 18.0;
        let mut clicked = None;
        for (i, nc) in colors.iter().enumerate() {
            let r = Rect::from_min_size(pos2(full.left() + 4.0 + i as f32 * (sw + 1.0), full.top() + 4.0), vec2(sw, full.height() - 8.0));
            if r.right() > full.right() - right_w {
                break;
            }
            let rr = ui.interact(r, ui.scope_id().with(("dps", i)), Sense::click_and_drag());
            paint_well(&p, r, Some(nc.color));
            if rr.hovered() {
                p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
            }
            rr.dnd_set_drag_payload(ColorDrag(Some(nc.color)));
            let rr = rr.on_hover_text(format!("{}\n{}", nc.name, nc.color.describe()));
            if rr.clicked() {
                clicked = Some((Some(nc.color), false));
            }
            if rr.secondary_clicked() {
                clicked = Some((Some(nc.color), true));
            }
        }
        if let Some((c, o)) = clicked {
            crate::exec::apply_color(app, c, o);
        }
    }
    // › and » buttons at the right
    let r1 = Rect::from_center_size(pos2(full.right() - 30.0, full.center().y), vec2(12.0, 14.0));
    let r2 = Rect::from_center_size(pos2(full.right() - 13.0, full.center().y), vec2(12.0, 14.0));
    let b1 = ui.interact(r1, ui.scope_id().with("dp_menu"), Sense::click()).on_hover_text("Document palette options");
    let b2 = ui.interact(r2, ui.scope_id().with("dp_more"), Sense::click()).on_hover_text("Show more");
    crate::widgets::chevron_right(&p, r1.center(), 3.0, if b1.hovered() { Color32::WHITE } else { theme::TEXT_DIM });
    p.text(r2.center(), egui::Align2::CENTER_CENTER, "»", egui::FontId::proportional(11.0), if b2.hovered() { Color32::WHITE } else { theme::TEXT_DIM });
    egui::Popup::menu(&b1).show(|ui| {
        ui.set_min_width(220.0);
        if ui.button("Add Colors from Selection").clicked() {
            app.run(crate::commands::Cmd::AddColorsFromSelection);
        }
        if ui.button("Add Colors from Document").clicked() {
            app.run(crate::commands::Cmd::AddColorsFromDocument);
        }
        ui.separator();
        ui.checkbox(&mut app.settings.auto_update_doc_palette, "Automatically update");
        if ui.button("Clear Document Palette").clicked() {
            if let Some(d) = app.doc_mut() {
                d.checkpoint("Clear Document Palette");
                d.doc.palette.clear();
            }
        }
    });
    if b2.clicked() {
        app.run(crate::commands::Cmd::Docker(crate::dockers::DockerId::Palettes));
    }
}
