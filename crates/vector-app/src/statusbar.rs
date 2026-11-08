//! Status bar: ⚙ mode menu · tool hints / object information · fill & outline · document colour settings.

use crate::app::VectorApp;
use crate::icons::Tone;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Ui};
use vector_core::{Fill, ObjKind};

const MODES: [&str; 4] = ["Tool Hints", "Object Details", "Cursor Coordinates", "Document Color Settings"];

fn icon(app: &mut VectorApp, ui: &mut Ui, name: &str, r: Rect, size: f32) {
    if let Some(t) = app.icons.get(ui.ctx(), name, size, Tone::Normal) {
        ui.painter().image(t.id, Rect::from_center_size(r.center(), vec2(size, size)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
}

pub fn status_bar(app: &mut VectorApp, ui: &mut Ui) {
    let full = ui.max_rect();
    let cy = full.center().y;
    let font = theme::body(12.5);
    // ⚙ menu
    let gr = Rect::from_center_size(pos2(full.left() + 12.0, cy), vec2(22.0, 22.0));
    let g = ui.interact(gr, ui.scope_id().with("sb_gear"), Sense::click());
    if g.hovered() {
        ui.painter().rect_filled(gr, 2.0, theme::HOVER);
    }
    icon(app, ui, "gear", gr, 16.0);
    egui::Popup::menu(&g).align(egui::RectAlign::TOP_START).show(|ui| {
        ui.set_min_width(200.0);
        for (i, m) in MODES.iter().enumerate() {
            if ui.radio(app.settings.show_tool_hints as usize == i, *m).clicked() {
                app.settings.show_tool_hints = i as u8;
                ui.close();
            }
        }
    });
    // left texts
    let mode = app.settings.show_tool_hints as usize;
    let (first, second) = left_texts(app, mode);
    let p = ui.painter().clone();
    p.text(pos2(full.left() + 32.0, cy), egui::Align2::LEFT_CENTER, &first, font.clone(), theme::TEXT);
    let w1 = p.layout_no_wrap(first.clone(), font.clone(), theme::TEXT).size().x;
    let x2 = (full.left() + 156.0).max(full.left() + 32.0 + w1 + 24.0);
    p.text(pos2(x2, cy), egui::Align2::LEFT_CENTER, &second, font.clone(), theme::TEXT);

    // right side: fill · outline · │ · document colour settings
    let (fill, outline) = current_fill_outline(app);
    let right = full.right();
    let dc = Rect::from_center_size(pos2(right - 16.0, cy), vec2(24.0, 22.0));
    let dcr = ui.interact(dc, ui.scope_id().with("sb_dc"), Sense::click()).on_hover_text("Document Color Settings");
    if dcr.hovered() {
        p.rect_filled(dc, 2.0, theme::HOVER);
    }
    icon(app, ui, "doc_color", dc, 18.0);
    if dcr.clicked() {
        app.run(crate::commands::Cmd::ColorManagement);
    }
    p.line_segment([pos2(right - 36.0, full.top() + 6.0), pos2(right - 36.0, full.bottom() - 6.0)], egui::Stroke::new(1.0, theme::SEP));
    let ox = right - 238.0;
    let fx = right - 438.0;
    // fill
    let fr = Rect::from_min_size(pos2(fx, full.top()), vec2(190.0, full.height()));
    let fresp = ui.interact(fr, ui.scope_id().with("sb_fill"), Sense::click()).on_hover_text("Fill: double-click to edit");
    icon(app, ui, "fill_icon", Rect::from_center_size(pos2(fx + 10.0, cy), vec2(18.0, 18.0)), 16.0);
    let sw = Rect::from_center_size(pos2(fx + 33.0, cy), vec2(19.0, 19.0));
    swatch(&p, sw, &fill);
    p.text(pos2(fx + 50.0, cy), egui::Align2::LEFT_CENTER, fill_text(&fill), font.clone(), theme::TEXT);
    if fresp.double_clicked() && app.has_doc() {
        app.run(crate::commands::Cmd::Docker(crate::dockers::DockerId::Properties));
    }
    // outline
    let orr = Rect::from_min_size(pos2(ox, full.top()), vec2(190.0, full.height()));
    let oresp = ui.interact(orr, ui.scope_id().with("sb_outline"), Sense::click()).on_hover_text("Outline: double-click to edit");
    icon(app, ui, "outline_icon", Rect::from_center_size(pos2(ox + 10.0, cy), vec2(18.0, 18.0)), 16.0);
    let sw = Rect::from_center_size(pos2(ox + 33.0, cy), vec2(19.0, 19.0));
    let ofill = match outline {
        Some((Some(c), _)) => Some(Fill::Solid(c)),
        Some((None, _)) => Some(Fill::None),
        None => None,
    };
    swatch(&p, sw, &ofill);
    let otext = match outline {
        None => "Outline Color".to_string(),
        Some((None, _)) => "None".to_string(),
        Some((Some(c), w)) => format!("{}  {}", c.to_hex(), app.doc().map(|d| d.doc.settings.unit.format(w, d.doc.settings.dpi)).unwrap_or_default()),
    };
    p.text(pos2(ox + 50.0, cy), egui::Align2::LEFT_CENTER, otext, font, theme::TEXT);
    if oresp.double_clicked() && app.has_doc() {
        app.run(crate::commands::Cmd::Docker(crate::dockers::DockerId::Properties));
    }
}

fn fill_text(f: &Option<Fill>) -> String {
    match f {
        None => "Fill Color".into(),
        Some(Fill::None) => "None".into(),
        Some(Fill::Solid(c)) => c.to_hex(),
        Some(other) => other.type_name().into(),
    }
}

fn swatch(p: &egui::Painter, r: Rect, f: &Option<Fill>) {
    match f.as_ref().and_then(|f| f.main_color()) {
        Some(c) if !matches!(f, Some(Fill::None)) => {
            crate::widgets::checker(p, r, 4.0);
            p.rect_filled(r, 2.0, theme::to_c32(c));
            if let Some(Fill::Gradient(g)) = f {
                // show the gradient as a left→right ramp
                let n = 12;
                for i in 0..n {
                    let t = i as f64 / (n - 1) as f64;
                    let x0 = r.left() + r.width() * i as f32 / n as f32;
                    let x1 = r.left() + r.width() * (i + 1) as f32 / n as f32;
                    p.rect_filled(Rect::from_min_max(pos2(x0, r.top()), pos2(x1, r.bottom())), 0.0, theme::to_c32(g.color_at(t)));
                }
            }
        }
        _ => {
            p.rect_filled(r, 2.0, Color32::WHITE);
            p.line_segment([r.left_bottom() + vec2(2.0, -2.0), r.right_top() + vec2(-2.0, 2.0)], egui::Stroke::new(1.3, Color32::from_rgb(0xe0, 0x20, 0x20)));
        }
    }
}

/// Fill/outline of the selection (or of the defaults when nothing is selected).
fn current_fill_outline(app: &VectorApp) -> (Option<Fill>, Option<(Option<vector_core::Color>, f64)>) {
    let Some(d) = app.doc() else { return (None, None) };
    if let Some(id) = d.selection.first() {
        if let Some(o) = d.doc.find(*id) {
            if d.selection.len() > 1 {
                return (Some(o.style.fill.clone()), Some((o.style.outline.color, o.style.outline.width)));
            }
            return (Some(o.style.fill.clone()), Some((if o.style.outline.width > 0.0 { o.style.outline.color } else { None }, o.style.outline.width)));
        }
    }
    (Some(Fill::None), Some((Some(vector_core::Color::BLACK), app.ts.default_outline_width)))
}

fn left_texts(app: &VectorApp, mode: usize) -> (String, String) {
    let Some(d) = app.doc() else { return ("Tool Hints".into(), "Object Information".into()) };
    let obj_info = object_info(app);
    match mode {
        1 => (obj_info.clone(), object_details(app)),
        2 => {
            let c = app.cursor_doc.map(|p| {
                let u = d.doc.settings.unit;
                let dpi = d.doc.settings.dpi;
                let o = d.doc.settings.ruler_origin;
                format!("( {}, {} )", u.format(p.x - o.x, dpi), u.format(p.y - o.y, dpi))
            });
            (c.unwrap_or_default(), obj_info)
        }
        3 => ("Color profile: sRGB IEC61966-2.1".into(), "Color mode: RGB".into()),
        _ => {
            let hint = if !app.status_hover.is_empty() { app.status_hover.clone() } else { app.ts.hint(app.tool) };
            (hint, obj_info)
        }
    }
}

pub fn object_info(app: &VectorApp) -> String {
    let Some(d) = app.doc() else { return "Object Information".into() };
    let n = d.selection.len();
    if n == 0 {
        return String::new();
    }
    let layer_name = |id| d.doc.locate(id).map(|(p, l, _)| d.doc.pages[p].layers[l].name.clone()).unwrap_or_default();
    if n == 1 {
        let o = match d.doc.find(d.selection[0]) {
            Some(o) => o,
            None => return String::new(),
        };
        let extra = match &o.kind {
            ObjKind::Path(p) => format!(" with {} nodes", p.node_count()),
            ObjKind::Group(c) => format!(" of {} objects", c.len()),
            ObjKind::Text(t) => format!(" ({} characters)", t.char_count()),
            ObjKind::Image(i) => d.doc.images.get(&i.image).map(|im| format!(" ({} x {} pixels)", im.width, im.height)).unwrap_or_default(),
            _ => String::new(),
        };
        return format!("{}{} on {}", o.display_name(), extra, layer_name(o.id));
    }
    format!("{n} objects selected on {}", layer_name(d.selection[0]))
}

fn object_details(app: &VectorApp) -> String {
    let Some(d) = app.doc() else { return String::new() };
    let Some(bb) = d.selection_bounds() else { return String::new() };
    let u = d.doc.settings.unit;
    let dpi = d.doc.settings.dpi;
    format!("W: {}  H: {}  Center: ({}, {})", u.format(bb.width(), dpi), u.format(bb.height(), dpi), u.format(bb.center().x, dpi), u.format(bb.center().y, dpi))
}
