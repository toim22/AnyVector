//! Status bar: ⚙ mode menu · tool hints / object information · fill & outline · document colour settings.

use crate::app::AnyVectorApp;
use crate::icons::Tone;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Ui};
use vector_core::{Fill, ObjKind};

const MODES: [&str; 4] = ["Tool Hints", "Object Details", "Cursor Coordinates", "Document Color Settings"];

fn icon(app: &mut AnyVectorApp, ui: &mut Ui, name: &str, r: Rect, size: f32) {
    if let Some(t) = app.icons.get(ui.ctx(), name, size, Tone::Normal) {
        ui.painter().image(t.id, Rect::from_center_size(r.center(), vec2(size, size)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
}

pub fn status_bar(app: &mut AnyVectorApp, ui: &mut Ui) {
    let full = ui.max_rect();
    let cy = full.center().y;
    let font = theme::body(12.5);
    // ⚙ menu
    let gr = Rect::from_center_size(pos2(full.left() + 12.0, cy), vec2(22.0, 22.0));
    let g = ui.interact(gr, ui.scope_id().with("sb_gear"), Sense::click());
    if g.hovered() {
        ui.painter().rect_filled(gr, 2.0, theme::HOVER());
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
    let right = full.right();
    let ox = right - 268.0;
    // the fill field makes room for the ⚙ in the narrowest windows
    let fx = (right - 468.0).max(full.left() + 26.0);
    // left texts, up to the fill field; a cut one shows whole on hover
    let mode = app.settings.show_tool_hints as usize;
    let (first, second) = left_texts(app, mode);
    let p = ui.painter().clone();
    let (first, second) = (crate::i18n::t(&first), crate::i18n::t(&second));
    let width = |t: &str| p.layout_no_wrap(t.to_string(), font.clone(), theme::TEXT()).size().x;
    let left = full.left() + 32.0;
    let room = (fx - 12.0 - left).max(0.0);
    let (w1, x2, w2) = left_widths(room, width(&first), width(&second));
    let mut cut = vec![];
    for (id, x, w, text) in [("sb_hint", left, w1, &first), ("sb_info", left + x2, w2, &second)] {
        let shown = fit(&p, text, &font, w);
        if shown != *text {
            let r = Rect::from_min_size(pos2(x, full.top()), vec2(w, full.height()));
            cut.push((id, r, text.clone()));
        }
        if shown != "..." {
            p.text(pos2(x, cy), egui::Align2::LEFT_CENTER, shown, font.clone(), theme::TEXT());
        }
    }
    for (id, r, text) in cut {
        if r.width() > 0.0 {
            ui.interact(r, ui.scope_id().with(id), Sense::hover()).on_hover_text(text);
        }
    }

    // right side: fill · outline · │ · document colour settings
    let (fill, outline) = current_fill_outline(app);
    let dc = Rect::from_center_size(pos2(right - 16.0, cy), vec2(24.0, 22.0));
    let dcr = ui.interact(dc, ui.scope_id().with("sb_dc"), Sense::click()).on_hover_text("Document Color Settings");
    if dcr.hovered() {
        p.rect_filled(dc, 2.0, theme::HOVER());
    }
    icon(app, ui, "doc_color", dc, 18.0);
    if dcr.clicked() {
        app.run(crate::commands::Cmd::ColorManagement);
    }
    p.line_segment([pos2(right - 36.0, full.top() + 6.0), pos2(right - 36.0, full.bottom() - 6.0)], egui::Stroke::new(1.0, theme::SEP()));
    // fill
    let fr = Rect::from_min_size(pos2(fx, full.top()), vec2(ox - 10.0 - fx, full.height()));
    let fresp = ui.interact(fr, ui.scope_id().with("sb_fill"), Sense::click()).on_hover_text("Fill: double-click to edit");
    icon(app, ui, "fill_icon", Rect::from_center_size(pos2(fx + 10.0, cy), vec2(18.0, 18.0)), 16.0);
    let sw = Rect::from_center_size(pos2(fx + 33.0, cy), vec2(19.0, 19.0));
    swatch(&p, sw, &fill);
    p.text(pos2(fx + 50.0, cy), egui::Align2::LEFT_CENTER, fit(&p, &crate::i18n::t(&fill_text(&fill)), &font, ox - 14.0 - (fx + 50.0)), font.clone(), theme::TEXT());
    if fresp.double_clicked() && app.has_doc() {
        app.run(crate::commands::Cmd::Docker(crate::dockers::DockerId::Properties));
    }
    // outline: the color, then the width in the outline unit (#138); a hairline is an outline
    // with a color (#127)
    let (u, dpi) = crate::lengths::outline_unit(app);
    let (ctext, wtext) = outline_texts(outline, u, dpi);
    let orr = Rect::from_min_size(pos2(ox, full.top()), vec2(220.0, full.height()));
    let tip = match &ctext {
        Some(c) => format!("{}\n{c}  {}", crate::i18n::t("Outline: double-click to edit"), crate::i18n::t(&wtext)),
        None => crate::i18n::t("Outline: double-click to edit"),
    };
    let oresp = ui.interact(orr, ui.scope_id().with("sb_outline"), Sense::click()).on_hover_text(tip);
    icon(app, ui, "outline_icon", Rect::from_center_size(pos2(ox + 10.0, cy), vec2(18.0, 18.0)), 16.0);
    let sw = Rect::from_center_size(pos2(ox + 33.0, cy), vec2(19.0, 19.0));
    let ofill = match outline {
        Some((Some(c), _)) => Some(Fill::Solid(c)),
        Some((None, _)) => Some(Fill::None),
        None => None,
    };
    swatch(&p, sw, &ofill);
    // the width always shows whole; the color gives up its room when the field is narrow
    let room = 166.0;
    let wtext = crate::i18n::t(&wtext);
    let ww = p.layout_no_wrap(wtext.clone(), font.clone(), theme::TEXT()).size().x;
    let mut x = ox + 50.0;
    if let Some(c) = &ctext {
        let shown = fit(&p, c, &font, (room - ww - 10.0).max(0.0));
        if shown != "..." {
            p.text(pos2(x, cy), egui::Align2::LEFT_CENTER, &shown, font.clone(), theme::TEXT());
            x += p.layout_no_wrap(shown, font.clone(), theme::TEXT()).size().x + 10.0;
        }
    }
    p.text(pos2(x, cy), egui::Align2::LEFT_CENTER, wtext, font, theme::TEXT());
    // double-click: the Outline Pen (the outline of new objects when nothing is selected, #103)
    if oresp.double_clicked() && app.has_doc() {
        app.run(crate::commands::Cmd::OutlinePen);
    }
}

/// The hint keeps this much room (or its own width) before the object information is cut.
const MIN_HINT: f32 = 120.0;

/// How the hint and the object information, `w1` and `w2` wide, share `room`: the hint's width,
/// where the information starts and its width. The hint gives up its room first.
fn left_widths(room: f32, w1: f32, w2: f32) -> (f32, f32, f32) {
    if w2 == 0.0 {
        return (w1.min(room), room, 0.0);
    }
    let x2 = (w1 + 24.0).max(124.0);
    if x2 + w2 <= room {
        return (w1, x2, w2);
    }
    let hint = (room - 24.0 - w2).clamp(w1.min(MIN_HINT).min(room), w1.max(0.0).min(room));
    let x2 = (hint + 24.0).min(room);
    (hint, x2, w2.min(room - x2))
}

/// The outline well's texts: the color (none without one) and the width part, "Outline Color"
/// with no document, "None" without an outline, "Hairline" or the width in `u`.
pub fn outline_texts(outline: Option<(Option<vector_core::Color>, f64)>, u: vector_core::Unit, dpi: f64) -> (Option<String>, String) {
    match outline {
        None => (None, "Outline Color".to_string()),
        Some((None, _)) => (None, "None".to_string()),
        Some((Some(c), w)) => (Some(color_text(c)), crate::lengths::width_text(w, u, dpi)),
    }
}

fn fill_text(f: &Option<Fill>) -> String {
    match f {
        None => "Fill Color".into(),
        Some(Fill::None) => "None".into(),
        Some(Fill::Solid(c)) => color_text(*c),
        Some(other) => other.type_name().into(),
    }
}

/// A color as the reference status bar writes it: "R:70 G:130 B:200 (#4682C8)".
fn color_text(c: vector_core::Color) -> String {
    format!("R:{} G:{} B:{} ({})", c.r, c.g, c.b, c.to_hex())
}

/// `text` cut to `width` with "..." at the end, as the status bar fields show long values.
pub fn fit(p: &egui::Painter, text: &str, font: &egui::FontId, width: f32) -> String {
    let w = |s: &str| p.layout_no_wrap(s.to_string(), font.clone(), theme::TEXT()).size().x;
    if w(text) <= width {
        return text.to_string();
    }
    // the most characters that fit, found by halves
    let cut = |n: usize| format!("{}...", text.chars().take(n).collect::<String>());
    let (mut fits, mut over) = (0, text.chars().count());
    while over - fits > 1 {
        let n = (fits + over) / 2;
        if w(&cut(n)) <= width {
            fits = n;
        } else {
            over = n;
        }
    }
    cut(fits)
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
pub fn current_fill_outline(app: &AnyVectorApp) -> (Option<Fill>, Option<(Option<vector_core::Color>, f64)>) {
    let Some(d) = app.doc() else { return (None, None) };
    if let Some(id) = d.selection.first() {
        if let Some(o) = d.doc.find(*id) {
            // the color says whether there is an outline; a width of 0 is a hairline (#127)
            return (Some(o.style.fill.clone()), Some((o.style.outline.color, o.style.outline.width)));
        }
    }
    let ol = &app.ts.default_outline;
    (Some(Fill::None), Some((ol.color, ol.width)))
}

fn left_texts(app: &AnyVectorApp, mode: usize) -> (String, String) {
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

pub fn object_info(app: &AnyVectorApp) -> String {
    let Some(d) = app.doc() else { return "Object Information".into() };
    let n = d.selection.len();
    if n == 0 {
        return String::new();
    }
    // (an object inside a group or a blend is on the layer of what holds it)
    let layer_name = |id| d.doc.top_level_of(id).and_then(|t| d.doc.locate(t)).map(|(p, l, _)| d.doc.pages[p].layers[l].name.clone()).unwrap_or_default();
    if n == 1 {
        let o = match d.doc.find(d.selection[0]) {
            Some(o) => o,
            None => return String::new(),
        };
        // the shadow clicked with the Pick tool (#106)
        match d.shadow_focus().filter(|_| app.tool == crate::tools::ToolId::Pick) {
            Some(vector_core::object::ShadowPart::Block) => return format!("Block Shadow on {}", layer_name(o.id)),
            Some(vector_core::object::ShadowPart::Drop) => return format!("Drop Shadow on {}", layer_name(o.id)),
            _ => {}
        }
        // a part of a blend shown on its own (#145)
        if let Some(s) = crate::blendpath::part_info(d) {
            return s;
        }
        let extra = match &o.kind {
            ObjKind::Path(p) => format!(" with {} nodes", p.node_count()),
            ObjKind::Group(c) => format!(" of {} objects", c.len()),
            ObjKind::Text(t) => format!(" ({} characters)", t.char_count()),
            ObjKind::Image(i) => d.doc.images.get(&i.image).map(|im| format!(" ({} x {} pixels)", im.width, im.height)).unwrap_or_default(),
            _ => String::new(),
        };
        // an object selected inside a group (Ctrl+click, #151): "Child Rectangle on Layer 1", as
        // CorelDRAW says it
        if d.doc.parent_of(o.id).is_some() {
            return format!("Child {}{} on {}", o.display_name(), extra, layer_name(o.id));
        }
        return format!("{}{} on {}", o.display_name(), extra, layer_name(o.id));
    }
    format!("{n} objects selected on {}", layer_name(d.selection[0]))
}

fn object_details(app: &AnyVectorApp) -> String {
    let Some(d) = app.doc() else { return String::new() };
    let Some(bb) = d.selection_bounds() else { return String::new() };
    let u = d.doc.settings.unit;
    let dpi = d.doc.settings.dpi;
    format!("W: {}  H: {}  Center: ({}, {})", u.format(bb.width(), dpi), u.format(bb.height(), dpi), u.format(bb.center().x, dpi), u.format(bb.center().y, dpi))
}

#[cfg(test)]
mod tests {
    use super::left_widths;

    #[test]
    fn the_hint_gives_up_its_room_first() {
        // room for both: where they always were (the information 156 from the left edge at least)
        assert_eq!(left_widths(900.0, 400.0, 200.0), (400.0, 424.0, 200.0));
        assert_eq!(left_widths(900.0, 60.0, 200.0), (60.0, 124.0, 200.0));
        assert_eq!(left_widths(900.0, 0.0, 200.0), (0.0, 124.0, 200.0));
        // the hint is cut, the information whole
        assert_eq!(left_widths(500.0, 400.0, 200.0), (276.0, 300.0, 200.0));
        // then the information, with 120 left for the hint
        assert_eq!(left_widths(300.0, 400.0, 200.0), (120.0, 144.0, 156.0));
        // a short hint stays whole
        assert_eq!(left_widths(250.0, 60.0, 200.0), (60.0, 84.0, 166.0));
        // no information: the hint has it all
        assert_eq!(left_widths(500.0, 800.0, 0.0), (500.0, 500.0, 0.0));
        assert_eq!(left_widths(500.0, 300.0, 0.0).0, 300.0);
        // hardly any room, or none
        for room in [0.0, 10.0, 100.0, 130.0, 200.0, 400.0] {
            for (w1, w2) in [(0.0, 0.0), (0.0, 300.0), (50.0, 300.0), (500.0, 300.0)] {
                let (a, x, b) = left_widths(room, w1, w2);
                assert!(a >= 0.0 && b >= 0.0 && a <= x && x + b <= room, "{room} {w1} {w2}: {a} {x} {b}");
            }
        }
    }
}
