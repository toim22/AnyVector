//! The fields of the property bars, drawn as in the reference: a dark field with the value,
//! small up/down arrows on spin boxes, a chevron on lists, two rows on the stacked fields (X over
//! Y, width over height) and a slider button next to the value of slider fields.

use crate::app::AnyVectorApp;
use crate::icons::Tone;
use crate::theme;
use crate::widgets::{chevron, chevron_up, paint_well};
use egui::{pos2, vec2, Color32, FontId, Id, Popup, PopupCloseBehavior, Rect, Response, Sense, Stroke, StrokeKind, Ui};
use vector_core::Unit;

/// Height of a one-row field.
pub const ROW: f32 = 22.0;
/// Height of each row of a stacked field.
const SROW: f32 = 18.0;
const BORDER: Color32 = Color32::from_rgb(0x50, 0x50, 0x50);

fn font() -> FontId {
    FontId::proportional(12.5)
}

fn ink(enabled: bool) -> Color32 {
    if enabled {
        theme::TEXT
    } else {
        theme::DISABLED
    }
}

/// How a numeric field shows and reads its value.
#[derive(Clone, Copy)]
pub enum Fmt {
    /// A length in points, shown in the document unit (`3.5 "`).
    Len(Unit, f64),
    /// A plain number with these decimals and this suffix (`90.0 °`).
    Num(usize, &'static str),
    /// A number without trailing zeros and with this suffix (`24 pt`).
    Trim(&'static str),
}

impl Fmt {
    pub fn show(self, v: f64) -> String {
        match self {
            Fmt::Len(u, dpi) => u.format(v, dpi),
            Fmt::Num(d, suffix) => {
                let s = format!("{v:.d$}");
                let s = if s == "-0" || s.starts_with("-0.") && s.trim_start_matches("-0.").chars().all(|c| c == '0') { s.trim_start_matches('-').to_string() } else { s };
                format!("{s}{suffix}")
            }
            Fmt::Trim(suffix) => {
                let s = format!("{v:.2}");
                format!("{}{suffix}", s.trim_end_matches('0').trim_end_matches('.'))
            }
        }
    }

    fn parse(self, s: &str) -> Option<f64> {
        match self {
            Fmt::Len(u, dpi) => Unit::parse_value(s, u, dpi),
            Fmt::Num(_, suffix) | Fmt::Trim(suffix) => s.trim().trim_end_matches(suffix.trim()).trim().trim_end_matches(['°', '%']).trim().parse().ok(),
        }
    }

    fn step(self) -> f64 {
        match self {
            Fmt::Len(u, dpi) => u.points_per_unit(dpi) * if u == Unit::Inches { 0.1 } else { 1.0 },
            Fmt::Num(..) | Fmt::Trim(_) => 1.0,
        }
    }
}

/// What stands left of a value: a caption ("X:") or an icon.
#[derive(Clone, Copy)]
pub enum Lead {
    None,
    Text(&'static str),
    Icon(&'static str),
}

impl Lead {
    fn width(self) -> f32 {
        match self {
            Lead::None => 0.0,
            Lead::Text(_) => 16.0,
            Lead::Icon(_) => 18.0,
        }
    }

    fn paint(self, app: &mut AnyVectorApp, ui: &Ui, at: egui::Pos2, enabled: bool) {
        match self {
            Lead::None => {}
            Lead::Text(t) => {
                ui.painter().text(at + vec2(2.0, 0.0), egui::Align2::LEFT_CENTER, t, font(), ink(enabled));
            }
            Lead::Icon(name) => {
                let tone = if enabled { Tone::Normal } else { Tone::Disabled };
                if let Some(t) = app.icons.get(ui.ctx(), name, 14.0, tone) {
                    ui.painter().image(t.id, Rect::from_center_size(at + vec2(8.0, 0.0), vec2(14.0, 14.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
            }
        }
    }
}

/// The dark field background with its border.
pub fn frame(ui: &Ui, rect: Rect, enabled: bool) {
    ui.painter().rect_filled(rect, 2.0, theme::FIELD);
    ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, if enabled { BORDER } else { theme::SEP }), StrokeKind::Inside);
}

/// The editable text of a field in `rect`; the text when the user commits it (Enter or leaving
/// the field).
fn text_in(ui: &mut Ui, id: Id, rect: Rect, shown: &str, enabled: bool) -> Option<String> {
    let focused = ui.memory(|m| m.has_focus(id));
    let mut text = if focused { ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| shown.to_string()) } else { shown.to_string() };
    let r = ui.place(rect, egui::TextEdit::singleline(&mut text).id(id).frame(egui::Frame::NONE).font(font()).text_color(ink(enabled)).interactive(enabled).margin(vec2(0.0, 0.0)));
    if r.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
        None
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
        r.lost_focus().then_some(text)
    }
}

/// Up/down arrows in `rect`: +1, -1 or 0.
fn arrows(ui: &mut Ui, id: Id, rect: Rect, enabled: bool) -> i32 {
    let up = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.center().y));
    let down = Rect::from_min_max(pos2(rect.min.x, rect.center().y), rect.max);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let ru = ui.interact(up, id.with("up"), sense);
    let rd = ui.interact(down, id.with("down"), sense);
    let col = |r: &Response| if !enabled { theme::DISABLED } else if r.hovered() { Color32::WHITE } else { theme::TEXT_DIM };
    chevron_up(ui.painter(), up.center() + vec2(0.0, 1.0), 3.0, col(&ru));
    chevron(ui.painter(), down.center() - vec2(0.0, 1.0), 3.0, col(&rd));
    if ru.clicked() {
        1
    } else if rd.clicked() {
        -1
    } else {
        0
    }
}

/// The value and (with `spin`) the arrows of a field in `rect`; the new value when it changed.
fn value_in(ui: &mut Ui, id: Id, rect: Rect, v: Option<f64>, fmt: Fmt, enabled: bool, spin: bool) -> Option<f64> {
    let aw = if spin { 14.0 } else { 0.0 };
    let tr = Rect::from_min_max(rect.min + vec2(4.0, 0.0), pos2(rect.max.x - aw - 1.0, rect.max.y));
    let shown = v.map(|v| fmt.show(v)).unwrap_or_default();
    let mut out = None;
    if let Some(t) = text_in(ui, id, tr, &shown, enabled && v.is_some()) {
        if let (Some(n), Some(v)) = (fmt.parse(&t), v) {
            if (n - v).abs() > 1e-9 {
                out = Some(n);
            }
        }
    }
    if spin {
        let ar = Rect::from_min_max(pos2(rect.max.x - aw - 1.0, rect.min.y + 1.0), pos2(rect.max.x - 1.0, rect.max.y - 1.0));
        let d = arrows(ui, id, ar, enabled && v.is_some());
        if d != 0 {
            out = v.map(|v| v + d as f64 * fmt.step());
        }
    }
    out
}

/// Room for a unit after a field ("°", "%"): the text and a gap on each side.
fn trail_width(ui: &Ui, trail: &str) -> f32 {
    if trail.is_empty() {
        0.0
    } else {
        ui.painter().layout_no_wrap(trail.to_string(), font(), Color32::WHITE).size().x + 8.0
    }
}

/// A spin box with an optional caption or icon before it and a unit after it ("°", "%").
#[allow(clippy::too_many_arguments)]
pub fn spin(app: &mut AnyVectorApp, ui: &mut Ui, id: Id, lead: Lead, v: Option<f64>, fmt: Fmt, w: f32, trail: &str, enabled: bool, tip: &str) -> Option<f64> {
    let lw = lead.width();
    let tw = trail_width(ui, trail);
    let (rect, resp) = ui.allocate_exact_size(vec2(lw + w + tw, ROW), Sense::hover());
    lead.paint(app, ui, rect.left_center(), enabled);
    let fr = Rect::from_min_size(rect.min + vec2(lw, 0.0), vec2(w, ROW));
    frame(ui, fr, enabled);
    let out = value_in(ui, id, fr, v, fmt, enabled, true);
    if !trail.is_empty() {
        ui.painter().text(pos2(fr.right() + 4.0, rect.center().y), egui::Align2::LEFT_CENTER, trail, font(), ink(enabled));
    }
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
    out
}

/// Two values one above the other. `boxed` draws one frame around both rows and their leads
/// (the reference X/Y and size fields); otherwise each value has its own frame.
#[allow(clippy::too_many_arguments)]
pub fn stacked(app: &mut AnyVectorApp, ui: &mut Ui, id: Id, leads: [Lead; 2], vals: [Option<f64>; 2], fmt: Fmt, w: f32, trail: &str, boxed: bool, spin: bool, enabled: bool, tip: &str) -> [Option<f64>; 2] {
    let lw = leads[0].width().max(leads[1].width());
    let tw = trail_width(ui, trail);
    let h = SROW * 2.0 + 4.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(lw + w + tw, h), Sense::hover());
    if boxed {
        frame(ui, Rect::from_min_size(rect.min, vec2(lw + w, h)), enabled);
    }
    let mut out = [None, None];
    for i in 0..2 {
        let y = rect.top() + 2.0 + i as f32 * SROW;
        let row = Rect::from_min_size(pos2(rect.left(), y), vec2(lw + w, SROW));
        leads[i].paint(app, ui, row.left_center() + vec2(2.0, 0.0), enabled);
        let fr = Rect::from_min_max(pos2(row.left() + lw, row.top()), row.max);
        if !boxed {
            frame(ui, fr.expand2(vec2(0.0, 1.0)), enabled);
        }
        out[i] = value_in(ui, id.with(i), fr, vals[i], fmt, enabled, spin);
        if !trail.is_empty() {
            ui.painter().text(pos2(fr.right() + 4.0, row.center().y), egui::Align2::LEFT_CENTER, trail, font(), ink(enabled));
        }
    }
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
    out
}

/// Two icons one above the other, beside a `stacked` field (the corner radius markers).
pub fn lead_column(app: &mut AnyVectorApp, ui: &mut Ui, leads: [Lead; 2], enabled: bool) {
    let lw = leads[0].width().max(leads[1].width());
    let (rect, _) = ui.allocate_exact_size(vec2(lw, SROW * 2.0 + 4.0), Sense::hover());
    for (i, lead) in leads.into_iter().enumerate() {
        let y = rect.top() + 2.0 + (i as f32 + 0.5) * SROW;
        lead.paint(app, ui, pos2(rect.left(), y), enabled);
    }
}

/// A list field: the text, a chevron, and the list `popup` draws below it.
pub fn combo(app: &mut AnyVectorApp, ui: &mut Ui, lead: Lead, text: &str, w: f32, enabled: bool, tip: &str, popup: impl FnOnce(&mut AnyVectorApp, &mut Ui)) -> Response {
    let lw = lead.width();
    let (rect, r) = ui.allocate_exact_size(vec2(lw + w, ROW), if enabled { Sense::click() } else { Sense::hover() });
    lead.paint(app, ui, rect.left_center(), enabled);
    let fr = Rect::from_min_max(rect.min + vec2(lw, 0.0), rect.max);
    ui.painter().rect_filled(fr, 2.0, if enabled && r.hovered() { theme::HOVER } else { Color32::from_rgb(0x33, 0x33, 0x33) });
    let clip = Rect::from_min_max(fr.min, pos2(fr.right() - 16.0, fr.bottom()));
    ui.painter().with_clip_rect(clip).text(pos2(fr.left() + 6.0, fr.center().y), egui::Align2::LEFT_CENTER, text, font(), ink(enabled));
    chevron(ui.painter(), pos2(fr.right() - 9.0, fr.center().y), 3.5, ink(enabled));
    let r = if tip.is_empty() { r } else { r.on_hover_text(tip) };
    Popup::menu(&r).show(|ui| {
        crate::menus::menu_style(ui);
        ui.set_min_width(w.max(120.0));
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| popup(app, ui));
    });
    r
}

/// A value the user can type, with a chevron that lists common values (`(caption, value)`).
#[allow(clippy::too_many_arguments)]
pub fn edit_list(app: &mut AnyVectorApp, ui: &mut Ui, id: Id, lead: Lead, v: Option<f64>, fmt: Fmt, w: f32, enabled: bool, tip: &str, list: &[(String, f64)]) -> Option<f64> {
    let lw = lead.width();
    let (rect, resp) = ui.allocate_exact_size(vec2(lw + w, ROW), Sense::hover());
    lead.paint(app, ui, rect.left_center(), enabled);
    let fr = Rect::from_min_max(rect.min + vec2(lw, 0.0), rect.max);
    frame(ui, fr, enabled);
    let vr = Rect::from_min_max(fr.min, pos2(fr.right() - 16.0, fr.bottom()));
    let mut out = value_in(ui, id, vr, v, fmt, enabled, false);
    let ar = Rect::from_min_max(pos2(fr.right() - 16.0, fr.top()), fr.max);
    let on = enabled && v.is_some();
    let a = ui.interact(ar, id.with("list"), if on { Sense::click() } else { Sense::hover() });
    if on && a.hovered() {
        ui.painter().rect_filled(ar.shrink(1.0), 2.0, theme::HOVER);
    }
    chevron(ui.painter(), ar.center(), 3.5, ink(on));
    Popup::menu(&a).show(|ui| {
        crate::menus::menu_style(ui);
        ui.set_min_width(w.max(90.0));
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            for (name, x) in list {
                if ui.selectable_label(v.is_some_and(|v| (v - x).abs() < 1e-6), name).clicked() {
                    out = Some(*x);
                }
            }
        });
    });
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
    out
}

/// A color list: the swatch and a chevron; the new color when one is picked (`Some(None)` for
/// no color).
pub fn color_combo(ui: &mut Ui, id: Id, color: Option<vector_core::Color>, w: f32, enabled: bool, tip: &str) -> Option<Option<vector_core::Color>> {
    let (rect, r) = ui.allocate_exact_size(vec2(w, ROW), if enabled { Sense::click() } else { Sense::hover() });
    ui.painter().rect_filled(rect, 2.0, if enabled && r.hovered() { theme::HOVER } else { Color32::from_rgb(0x33, 0x33, 0x33) });
    let well = Rect::from_min_max(rect.min + vec2(4.0, 4.0), pos2(rect.right() - 18.0, rect.bottom() - 4.0));
    if enabled {
        paint_well(ui.painter(), well, color);
    } else {
        ui.painter().rect_filled(well, 0.0, theme::FIELD);
    }
    ui.painter().rect_stroke(well, 0.0, Stroke::new(1.0, BORDER), StrokeKind::Outside);
    chevron(ui.painter(), pos2(rect.right() - 9.0, rect.center().y), 3.5, ink(enabled));
    let r = if tip.is_empty() { r } else { r.on_hover_text(tip) };
    let mut out = None;
    Popup::menu(&r).close_behavior(PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        let mut c = theme::to_c32(color.unwrap_or(vector_core::Color::BLACK));
        if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque) {
            out = Some(Some(theme::from_c32(c)));
        }
        if ui.button("None").clicked() {
            out = Some(None);
            ui.close();
        }
    });
    let _ = id;
    out
}

/// A value with the slider button next to it; the slider opens below.
#[allow(clippy::too_many_arguments)]
pub fn slider(app: &mut AnyVectorApp, ui: &mut Ui, id: Id, lead: Lead, v: Option<f64>, range: std::ops::RangeInclusive<f64>, fmt: Fmt, w: f32, enabled: bool, tip: &str) -> Option<f64> {
    let lw = lead.width();
    let (rect, resp) = ui.allocate_exact_size(vec2(lw + w + 18.0, ROW), Sense::hover());
    lead.paint(app, ui, rect.left_center(), enabled);
    let fr = Rect::from_min_size(rect.min + vec2(lw, 0.0), vec2(w, ROW));
    frame(ui, fr, enabled);
    let mut out = value_in(ui, id, fr, v, fmt, enabled, false);
    // the slider button: a vertical bar crossed by a short line
    let br = Rect::from_min_max(pos2(fr.right() + 1.0, rect.top()), rect.max);
    let on = enabled && v.is_some();
    let b = ui.interact(br, id.with("slider"), if on { Sense::click() } else { Sense::hover() });
    if on && b.hovered() {
        ui.painter().rect_filled(br, 2.0, theme::HOVER);
    }
    let c = br.center();
    let s = Stroke::new(1.0, ink(on));
    ui.painter().line_segment([c - vec2(0.0, 6.0), c + vec2(0.0, 6.0)], s);
    ui.painter().line_segment([c - vec2(3.0, 0.0), c + vec2(3.0, 0.0)], s);
    let lo = *range.start();
    Popup::menu(&b).close_behavior(PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        let mut x = v.unwrap_or(lo);
        if ui.add(egui::Slider::new(&mut x, range).show_value(false)).changed() {
            out = Some(x);
        }
    });
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
    out
}

/// A one-line text field ("Prefix:"); the text when committed.
pub fn text(ui: &mut Ui, id: Id, caption: &str, value: &str, w: f32, enabled: bool) -> Option<String> {
    let cw = if caption.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(caption.to_string(), font(), theme::TEXT).size().x + 4.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(cw + w, ROW), Sense::hover());
    if cw > 0.0 {
        ui.painter().text(rect.left_center(), egui::Align2::LEFT_CENTER, caption, font(), ink(enabled));
    }
    let fr = Rect::from_min_max(rect.min + vec2(cw, 0.0), rect.max);
    frame(ui, fr, enabled);
    text_in(ui, id, fr.shrink2(vec2(4.0, 0.0)), value, enabled)
}

/// A caption.
pub fn caption(ui: &mut Ui, text: &str, enabled: bool) {
    let g = ui.painter().layout_no_wrap(text.to_string(), font(), ink(enabled));
    let (rect, _) = ui.allocate_exact_size(vec2(g.size().x + 2.0, ROW), Sense::hover());
    ui.painter().galley(pos2(rect.left(), rect.center().y - g.size().y / 2.0), g, ink(enabled));
}

/// The object origin: a 3×3 grid with the reference point marked; a click lists the nine
/// points. `origin` is 0..9, row by row from the top left.
pub fn anchors(ui: &mut Ui, origin: Option<u8>, enabled: bool, tip: &str) -> Option<u8> {
    let (rect, r) = ui.allocate_exact_size(vec2(26.0, 26.0), if enabled { Sense::click() } else { Sense::hover() });
    if enabled && r.hovered() {
        ui.painter().rect_filled(rect, 2.0, theme::HOVER);
    }
    grid(ui.painter(), rect.shrink(4.0), origin, enabled);
    let r = if tip.is_empty() { r } else { r.on_hover_text(tip) };
    let mut out = None;
    Popup::menu(&r).show(|ui| {
        let (big, resp) = ui.allocate_exact_size(vec2(72.0, 72.0), Sense::click());
        grid(ui.painter(), big.shrink(8.0), origin, true);
        if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.clicked()) {
            let f = (p - big.min) / big.size();
            let col = (f.x * 3.0).floor().clamp(0.0, 2.0) as u8;
            let row = (f.y * 3.0).floor().clamp(0.0, 2.0) as u8;
            out = Some(row * 3 + col);
            ui.close();
        }
    });
    out
}

fn grid(p: &egui::Painter, rect: Rect, mark: Option<u8>, enabled: bool) {
    let c = ink(enabled);
    let step = rect.width() / 2.0;
    for i in 0..9u8 {
        let at = rect.min + vec2((i % 3) as f32 * step, (i / 3) as f32 * step);
        let s = Rect::from_center_size(at, vec2(4.5, 4.5));
        if mark == Some(i) {
            p.rect_filled(s, 0.0, c);
        } else {
            p.rect_stroke(s, 0.0, Stroke::new(1.0, c), StrokeKind::Inside);
        }
    }
}

/// A field AnyVector does not have yet, drawn empty and disabled at the reference size.
pub fn empty(app: &mut AnyVectorApp, ui: &mut Ui, it: &crate::ui::Item) {
    use crate::ui::Kind;
    let tip = if it.tip.is_empty() { crate::ui::plain(it.text) } else { it.tip.to_string() };
    let id = ui.make_persistent_id(("empty", it.id));
    let lead = if it.icon.is_empty() { Lead::None } else { Lead::Icon(it.icon) };
    match it.kind {
        Kind::Spin => {
            spin(app, ui, id, lead, None, Fmt::Num(0, ""), 56.0, "", false, &tip);
        }
        Kind::StackedSpin => {
            stacked(app, ui, id, [lead, lead], [None, None], Fmt::Num(0, ""), 72.0, "", false, true, false, &tip);
        }
        Kind::Combo | Kind::CellCombo => {
            combo(app, ui, Lead::None, "", if it.kind == Kind::Combo { 96.0 } else { 70.0 }, false, &tip, |_, _| {});
        }
        Kind::DropDown => {
            let text = if it.icon.is_empty() { crate::ui::plain(it.text) } else { String::new() };
            combo(app, ui, lead, &text, if it.icon.is_empty() { 90.0 } else { 16.0 }, false, &tip, |_, _| {});
        }
        Kind::Slider => {
            slider(app, ui, id, lead, None, 0.0..=100.0, Fmt::Num(0, ""), 40.0, false, &tip);
        }
        Kind::Edit => {
            text(ui, id, &crate::ui::plain(it.text), "", 40.0, false);
        }
        Kind::Anchors => {
            anchors(ui, None, false, &tip);
        }
        Kind::Label => caption(ui, &crate::ui::plain(crate::ui::label(it)), false),
        _ => {}
    }
}

/// A drop-down whose panel stays open while it is used (check boxes, sliders, values); a click
/// outside closes it.
pub fn panel(app: &mut AnyVectorApp, ui: &mut Ui, lead: Lead, text: &str, w: f32, enabled: bool, tip: &str, body: impl FnOnce(&mut AnyVectorApp, &mut Ui)) -> Response {
    let lw = lead.width();
    let (rect, r) = ui.allocate_exact_size(vec2(lw + w, ROW), if enabled { Sense::click() } else { Sense::hover() });
    lead.paint(app, ui, rect.left_center(), enabled);
    let fr = Rect::from_min_max(rect.min + vec2(lw, 0.0), rect.max);
    ui.painter().rect_filled(fr, 2.0, if enabled && r.hovered() { theme::HOVER } else { Color32::from_rgb(0x33, 0x33, 0x33) });
    let clip = Rect::from_min_max(fr.min, pos2(fr.right() - 16.0, fr.bottom()));
    ui.painter().with_clip_rect(clip).text(pos2(fr.left() + 6.0, fr.center().y), egui::Align2::LEFT_CENTER, text, font(), ink(enabled));
    chevron(ui.painter(), pos2(fr.right() - 9.0, fr.center().y), 3.5, ink(enabled));
    let r = if tip.is_empty() { r } else { r.on_hover_text(tip) };
    Popup::menu(&r).close_behavior(PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        crate::menus::menu_style(ui);
        ui.set_min_width(w.max(160.0));
        body(app, ui);
    });
    r
}
