//! Small custom widgets used by bars, dockers and dialogs.

use crate::icons::{Icons, Tone};
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Response, Sense, Stroke, Ui};
use vector_core::Unit;

/// Flat icon button as in the toolbars (hover box, pressed/checked blue).
pub fn icon_button(icons: &mut Icons, ui: &mut Ui, icon: &str, tip: &str, enabled: bool, checked: bool) -> Response {
    icon_button_sized(icons, ui, icon, tip, enabled, checked, 26.0, 18.0)
}

#[allow(clippy::too_many_arguments)]
pub fn icon_button_sized(icons: &mut Icons, ui: &mut Ui, icon: &str, tip: &str, enabled: bool, checked: bool, box_size: f32, icon_size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(box_size, box_size.max(icon_size + 4.0)), if enabled { Sense::click() } else { Sense::hover() });
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if checked {
            p.rect_filled(rect.shrink(1.0), 2.0, theme::PRESSED);
        } else if enabled && resp.is_pointer_button_down_on() {
            p.rect_filled(rect.shrink(1.0), 2.0, theme::PRESSED);
        } else if enabled && resp.hovered() {
            p.rect_filled(rect.shrink(1.0), 2.0, theme::HOVER);
        }
        let tone = if !enabled {
            Tone::Disabled
        } else if checked {
            Tone::Bright
        } else {
            Tone::Normal
        };
        if let Some(t) = icons.get(ui.ctx(), icon, icon_size, tone) {
            let r = Rect::from_center_size(rect.center(), vec2(icon_size, icon_size));
            p.image(t.id, r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
    }
    if !tip.is_empty() {
        resp.on_hover_text(tip)
    } else {
        resp
    }
}

/// Small dropdown arrow button (e.g. next to Open, Undo, Redo).
pub fn arrow_button(ui: &mut Ui, enabled: bool, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(14.0, 26.0), if enabled { Sense::click() } else { Sense::hover() });
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect.shrink2(vec2(0.0, 1.0)), 2.0, theme::HOVER);
    }
    let c = if enabled { theme::TEXT } else { theme::DISABLED };
    chevron(ui.painter(), rect.center(), 3.0, c);
    if tip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tip)
    }
}

pub fn chevron(p: &egui::Painter, c: egui::Pos2, s: f32, color: Color32) {
    p.line_segment([pos2(c.x - s, c.y - s * 0.5), pos2(c.x, c.y + s * 0.5)], Stroke::new(1.0, color));
    p.line_segment([pos2(c.x, c.y + s * 0.5), pos2(c.x + s, c.y - s * 0.5)], Stroke::new(1.0, color));
}

pub fn chevron_up(p: &egui::Painter, c: egui::Pos2, s: f32, color: Color32) {
    p.line_segment([pos2(c.x - s, c.y + s * 0.5), pos2(c.x, c.y - s * 0.5)], Stroke::new(1.0, color));
    p.line_segment([pos2(c.x, c.y - s * 0.5), pos2(c.x + s, c.y + s * 0.5)], Stroke::new(1.0, color));
}

pub fn chevron_right(p: &egui::Painter, c: egui::Pos2, s: f32, color: Color32) {
    p.line_segment([pos2(c.x - s * 0.5, c.y - s), pos2(c.x + s * 0.5, c.y)], Stroke::new(1.0, color));
    p.line_segment([pos2(c.x + s * 0.5, c.y), pos2(c.x - s * 0.5, c.y + s)], Stroke::new(1.0, color));
}

/// Vertical separator line in a horizontal bar.
pub fn vsep(ui: &mut Ui) {
    let h = ui.available_height().min(30.0);
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, h), Sense::hover());
    let x = rect.center().x;
    ui.painter().line_segment([pos2(x, rect.top() + 4.0), pos2(x, rect.bottom() - 4.0)], Stroke::new(1.0, theme::SEP));
}

/// Numeric field with unit suffix and stacked up/down chevrons (property bar style).
/// `value` is in points; displayed in `unit`. Returns true when changed (on Enter/focus loss or arrow click).
pub fn unit_field(ui: &mut Ui, id: egui::Id, value: &mut f64, unit: Unit, dpi: f64, width: f32, enabled: bool) -> bool {
    let mut changed = false;
    let mut text: String = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| unit.format(*value, dpi));
    let has_focus = ui.memory(|m| m.has_focus(id));
    if !has_focus {
        text = unit.format(*value, dpi);
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let te = egui::TextEdit::singleline(&mut text).id(id).desired_width(width).margin(vec2(4.0, 2.0)).text_color(if enabled { theme::TEXT } else { theme::DISABLED });
        let r = ui.add_enabled(enabled, te);
        if r.lost_focus() {
            if let Some(v) = Unit::parse_value(&text, unit, dpi) {
                if (v - *value).abs() > 1e-9 {
                    *value = v;
                    changed = true;
                }
            }
        }
        if r.has_focus() {
            ui.data_mut(|d| d.insert_temp(id, text.clone()));
        } else {
            ui.data_mut(|d| d.remove::<String>(id));
        }
        let step = unit.points_per_unit(dpi) * if unit == Unit::Inches { 0.1 } else { 1.0 };
        let (rect, _) = ui.allocate_exact_size(vec2(14.0, 20.0), Sense::hover());
        let up = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.center().y));
        let down = Rect::from_min_max(pos2(rect.min.x, rect.center().y), rect.max);
        let ru = ui.interact(up, id.with("up"), if enabled { Sense::click() } else { Sense::hover() });
        let rd = ui.interact(down, id.with("dn"), if enabled { Sense::click() } else { Sense::hover() });
        let col = |r: &Response| if !enabled { theme::DISABLED } else if r.hovered() { Color32::WHITE } else { theme::TEXT_DIM };
        chevron_up(ui.painter(), up.center(), 3.0, col(&ru));
        chevron(ui.painter(), down.center(), 3.0, col(&rd));
        if ru.clicked() {
            *value += step;
            changed = true;
        }
        if rd.clicked() {
            *value -= step;
            changed = true;
        }
    });
    changed
}

/// Frame-less style marker for TextEdit::frame (true = framed).
#[allow(non_upper_case_globals)]

/// Color well (swatch) button; None = no color (white with red diagonal).
pub fn color_well(ui: &mut Ui, color: Option<vector_core::Color>, size: egui::Vec2) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    paint_well(ui.painter(), rect, color);
    if resp.hovered() {
        ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
    }
    resp
}

pub fn paint_well(p: &egui::Painter, rect: Rect, color: Option<vector_core::Color>) {
    match color {
        Some(c) => {
            if c.a < 255 {
                checker(p, rect, 4.0);
            }
            p.rect_filled(rect, 0.0, theme::to_c32(c));
        }
        None => {
            p.rect_filled(rect, 0.0, Color32::WHITE);
            p.line_segment([rect.left_bottom(), rect.right_top()], Stroke::new(1.5, Color32::from_rgb(0xe0, 0x20, 0x20)));
        }
    }
}

pub fn checker(p: &egui::Painter, rect: Rect, s: f32) {
    p.rect_filled(rect, 0.0, Color32::WHITE);
    let mut y = rect.top();
    let mut row = 0;
    while y < rect.bottom() {
        let mut x = rect.left() + if row % 2 == 0 { 0.0 } else { s };
        while x < rect.right() {
            let r = Rect::from_min_max(pos2(x, y), pos2((x + s).min(rect.right()), (y + s).min(rect.bottom())));
            p.rect_filled(r, 0.0, Color32::from_gray(200));
            x += s * 2.0;
        }
        y += s;
        row += 1;
    }
}

/// Label with one underlined mnemonic character marked by '&'.
pub fn mnemonic(text: &str, color: Color32, size: f32) -> egui::text::LayoutJob {
    // a layout job is not translated by egui: the caption is translated here, with its access key
    let text = &crate::i18n::caption(text);
    let mut job = egui::text::LayoutJob::default();
    let font = egui::FontId::proportional(size);
    let mut chars = text.chars().peekable();
    let mut buf = String::new();
    while let Some(c) = chars.next() {
        if c == '&' {
            if let Some(&n) = chars.peek() {
                if !buf.is_empty() {
                    job.append(&buf, 0.0, egui::TextFormat { font_id: font.clone(), color, ..Default::default() });
                    buf.clear();
                }
                chars.next();
                job.append(&n.to_string(), 0.0, egui::TextFormat { font_id: font.clone(), color, underline: Stroke::new(1.0, color), ..Default::default() });
                continue;
            }
        }
        buf.push(c);
    }
    if !buf.is_empty() {
        job.append(&buf, 0.0, egui::TextFormat { font_id: font, color, ..Default::default() });
    }
    job
}

/// Strip '&' mnemonic markers.
pub fn plain(text: &str) -> String {
    text.replace('&', "")
}

/// A combo-like dropdown button showing `text`; returns the response (open a popup on click).
pub fn dropdown_button(ui: &mut Ui, text: &str, width: f32, enabled: bool) -> Response {
    let c = if enabled { theme::TEXT } else { theme::DISABLED };
    // a longer (translated) caption widens the button so the arrow stays clear of it
    let g = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(12.5), c);
    let width = width.max(g.size().x + 26.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    p.rect_filled(rect, 2.0, if resp.hovered() && enabled { Color32::from_rgb(0x2a, 0x2a, 0x2a) } else { theme::FIELD });
    p.galley(pos2(rect.left() + 6.0, rect.center().y - g.size().y / 2.0), g, c);
    chevron(p, pos2(rect.right() - 10.0, rect.center().y), 3.5, c);
    resp
}
