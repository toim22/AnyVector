//! Toolbars drawn from the reference interface data (`crate::ui`): the standard toolbar, the
//! optional toolbars of Window ▸ Toolbars and the property bars, item by item in the
//! reference order, with the reference separators, captions, icons and enabled states.
//!
//! Buttons, check buttons, flyouts and separators come straight from the data; fields (zoom
//! level, font, position, …) are drawn by [`control`] by item id.

use crate::app::{Toolbar, AnyVectorApp};
use crate::commands::Cmd;
use crate::theme;
use crate::ui::{self, bind, Face, Item, Kind, NO_SUB};
use crate::widgets::{arrow_button, chevron, dropdown_button, icon_button_sized, vsep};
use egui::{pos2, vec2, Popup, PopupCloseBehavior, Rect, Sense, Ui};

/// The reference bar of a toolbar.
pub fn toolbar_bar(tb: Toolbar) -> Option<u16> {
    let id = match tb {
        Toolbar::Standard => "c2b44f69-6dec-444e-a37e-5dbf7ff43dae",
        Toolbar::ProjectTimer => "0a1f35d2-4bc0-4ecc-b908-fbbc7b640302",
        Toolbar::Layout => "26278692-c566-47cb-83c4-79b58ce69538",
        Toolbar::Zoom => "3cc76550-c8c1-420f-bccb-c5ea2baf2e65",
        Toolbar::Internet => "57ddc38e-3367-479f-8b2a-4b531ae8cf2a",
        Toolbar::Macros => "bf935726-aeb5-4b71-8c3c-8d17ab7d53aa",
        Toolbar::ParallelDrawing => "ca73b05f-26c8-49ee-88e2-56f4f9945bb4",
        Toolbar::Text => "e71e8d2e-9f74-44da-8582-20f977363f5b",
        Toolbar::Transform => "f82791a4-048c-48f1-a059-cb2c3fd0815b",
        _ => return None,
    };
    ui::bar_named(id).map(|(i, _)| i)
}

fn key8(id: &str) -> String {
    id.get(..8).unwrap_or(id).to_ascii_lowercase()
}

/// The tooltip of an item: its tip (or caption) and its shortcut.
pub fn tooltip(app: &AnyVectorApp, it: &Item) -> String {
    let name = if it.tip.is_empty() { ui::plain(it.text) } else { it.tip.to_string() };
    let key = bind::cmd(it.id).and_then(|c| crate::shortcuts::shortcut_text(app, c)).or_else(|| it.keys.first().map(|k| k.to_string()));
    match key {
        Some(k) if !k.is_empty() => format!("{name} ({k})"),
        _ => name,
    }
}

pub fn enabled(app: &AnyVectorApp, it: &Item, ctx: u32) -> bool {
    bind::enabled(app, it.id).unwrap_or(it.on & ctx != 0)
}

pub fn run(app: &mut AnyVectorApp, it: &Item) {
    match bind::cmd(it.id) {
        Some(c) => app.run(c),
        None => app.flash(format!("{} is not available in this version yet.", ui::plain(ui::label(it)).trim_end_matches("..."))),
    }
}

fn icon_size(app: &AnyVectorApp) -> f32 {
    app.settings.button_size.icon()
}

/// A plain or checked toolbar button: the icon, or the caption when the item has no icon.
fn button(app: &mut AnyVectorApp, ui: &mut Ui, it: &'static Item, on: bool, checked: bool) -> egui::Response {
    let s = icon_size(app);
    let tip = tooltip(app, it);
    let text_face = it.face != Face::Default || it.icon.is_empty();
    if !text_face {
        // a lock toggle shows the closed padlock while it is on, the open one while it is off
        let icon = match it.icon {
            "lock" | "unlock" if it.kind != Kind::Button => if checked { "lock" } else { "unlock" },
            icon => icon,
        };
        return icon_button_sized(&mut app.icons, ui, icon, &tip, on, checked, s + 14.0, s);
    }
    // caption (with the icon on its left for "text right of image")
    let text = ui::plain(ui::label(it));
    let font = egui::FontId::proportional(12.5);
    let tw = ui.painter().layout_no_wrap(text.clone(), font.clone(), theme::TEXT).size().x;
    let with_icon = it.face == Face::TextRight && !it.icon.is_empty();
    let w = tw + 16.0 + if with_icon { s + 6.0 } else { 0.0 };
    let (rect, r) = ui.allocate_exact_size(vec2(w, s + 8.0), if on { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if checked || (on && r.is_pointer_button_down_on()) {
        p.rect_filled(rect.shrink(1.0), 2.0, theme::PRESSED);
    } else if on && r.hovered() {
        p.rect_filled(rect.shrink(1.0), 2.0, theme::HOVER);
    }
    let mut x = rect.left() + 8.0;
    if with_icon {
        let tone = if on { crate::icons::Tone::Normal } else { crate::icons::Tone::Disabled };
        if let Some(t) = app.icons.get(ui.ctx(), it.icon, s, tone) {
            p.image(t.id, Rect::from_center_size(pos2(x + s / 2.0, rect.center().y), vec2(s, s)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        x += s + 6.0;
    }
    p.text(pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, text, font, if on { theme::TEXT } else { theme::DISABLED });
    r.on_hover_text(tip)
}

/// A button that opens a menu: the icon (or caption) with a small arrow.
fn flyout(app: &mut AnyVectorApp, ui: &mut Ui, it: &'static Item, on: bool) -> egui::Response {
    let s = icon_size(app);
    let text = ui::plain(ui::label(it));
    let font = egui::FontId::proportional(12.5);
    let show_text = it.face != Face::Default || it.icon.is_empty();
    let show_icon = !it.icon.is_empty() && it.face != Face::TextOnly;
    if !show_text {
        // an icon with a small triangle in its corner
        let tip = tooltip(app, it);
        let r = icon_button_sized(&mut app.icons, ui, it.icon, &tip, on, false, s + 14.0, s);
        let c = r.rect.center() + vec2(s / 2.0 + 1.0, s / 2.0 + 1.0);
        let color = if on { theme::TEXT } else { theme::DISABLED };
        ui.painter().add(egui::Shape::convex_polygon(vec![c, c - vec2(4.0, 0.0), c - vec2(0.0, 4.0)], color, egui::Stroke::NONE));
        return r;
    }
    let tw = ui.painter().layout_no_wrap(text.clone(), font.clone(), theme::TEXT).size().x + 6.0;
    let w = 8.0 + if show_icon { s + 4.0 } else { 0.0 } + tw + 14.0;
    let (rect, r) = ui.allocate_exact_size(vec2(w, s + 8.0), if on { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if on && (r.hovered() || Popup::is_id_open(ui.ctx(), Popup::default_response_id(&r))) {
        p.rect_filled(rect.shrink(1.0), 3.0, theme::HOVER);
    } else {
        p.rect_filled(rect.shrink(1.0), 3.0, egui::Color32::from_rgb(0x33, 0x33, 0x33));
    }
    let c = if on { theme::TEXT } else { theme::DISABLED };
    let mut x = rect.left() + 6.0;
    if show_icon {
        let tone = if on { crate::icons::Tone::Normal } else { crate::icons::Tone::Disabled };
        if let Some(t) = app.icons.get(ui.ctx(), it.icon, s, tone) {
            p.image(t.id, Rect::from_center_size(pos2(x + s / 2.0, rect.center().y), vec2(s, s)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        x += s + 4.0;
    }
    p.text(pos2(x + 2.0, rect.center().y), egui::Align2::LEFT_CENTER, text, font, c);
    chevron(p, pos2(rect.right() - 9.0, rect.center().y), 3.0, c);
    r.on_hover_text(tooltip(app, it))
}

/// The menu a flyout or a button's arrow opens.
fn popup_rows(app: &mut AnyVectorApp, ui: &mut Ui, it: &'static Item) {
    ui.set_min_width(180.0);
    match key8(it.id).as_str() {
        // Undo / Redo: the history, newest first; choosing one undoes everything up to it
        "fc9524bc" | "ddc87e9c" => {
            let undo = key8(it.id) == "fc9524bc";
            let labels = app.doc().map(|d| if undo { d.history.undo_labels() } else { d.history.redo_labels() }).unwrap_or_default();
            for (i, l) in labels.iter().enumerate().take(30) {
                if ui.button(l).clicked() {
                    app.run(if undo { Cmd::UndoTo(i + 1) } else { Cmd::RedoTo(i + 1) });
                }
            }
        }
        _ if it.sub != NO_SUB => crate::menus::show_bar(app, ui, it.sub),
        _ => {}
    }
}

/// Whether a button's arrow has anything to show.
fn arrow_enabled(app: &AnyVectorApp, it: &Item, on: bool) -> bool {
    match key8(it.id).as_str() {
        "fc9524bc" => app.doc().is_some_and(|d| d.history.can_undo()),
        "ddc87e9c" => app.doc().is_some_and(|d| d.history.can_redo()),
        // Open: the recent files
        "2d9c749f" => !app.settings.recent_files.is_empty(),
        _ => on,
    }
}

/// Tests: the bar items AnyVector does not have yet, recorded while bars are drawn (bar, item, kind).
pub static MISSING: std::sync::Mutex<Option<std::collections::BTreeSet<(String, String, String)>>> = std::sync::Mutex::new(None);
thread_local! {
    static CURRENT_BAR: std::cell::Cell<&'static str> = const { std::cell::Cell::new("") };
}

pub fn note_missing(it: &Item, what: &str) {
    if let Ok(mut m) = MISSING.lock() {
        if let Some(set) = m.as_mut() {
            set.insert((CURRENT_BAR.with(|c| c.get()).to_string(), format!("{} [{}]", ui::plain(ui::label(it)).trim(), key8(it.id)), what.to_string()));
        }
    }
}

/// Draw the items of a bar side by side.
pub fn strip(app: &mut AnyVectorApp, ui: &mut Ui, bar: u16) {
    let Some(b) = ui::bar(bar) else { return };
    let ctx = ui::context_bit(app);
    CURRENT_BAR.with(|c| c.set(b.name));
    for it in b.items {
        item(app, ui, it, ctx);
    }
}

fn item(app: &mut AnyVectorApp, ui: &mut Ui, it: &'static Item, ctx: u32) {
    match it.kind {
        Kind::Separator => vsep(ui),
        Kind::GroupBegin | Kind::GroupEnd | Kind::Spacer | Kind::Placeholder => {}
        Kind::Button | Kind::Check | Kind::Radio => {
            if bind::cmd(it.id).is_none() {
                note_missing(it, "button");
            }
            let on = enabled(app, it, ctx);
            let checked = it.kind != Kind::Button && bind::checked(app, it.id).unwrap_or(false);
            if button(app, ui, it, on, checked).clicked() {
                run(app, it);
            }
        }
        Kind::ButtonFlyout => {
            let on = enabled(app, it, ctx);
            let checked = bind::checked(app, it.id).unwrap_or(false);
            if button(app, ui, it, on, checked).clicked() {
                run(app, it);
            }
            let r = arrow_button(ui, arrow_enabled(app, it, on), "");
            Popup::menu(&r).show(|ui| {
                crate::menus::menu_style(ui);
                popup_rows(app, ui, it);
            });
            ui.add_space(4.0);
        }
        Kind::Flyout | Kind::ToolFlyout => {
            let on = enabled(app, it, ctx);
            let r = flyout(app, ui, it, on);
            Popup::menu(&r).show(|ui| {
                crate::menus::menu_style(ui);
                popup_rows(app, ui, it);
            });
        }
        _ => control(app, ui, it, ctx),
    }
}

/// The fields of a bar, by item id. Fields AnyVector does not have yet are drawn empty and disabled.
fn control(app: &mut AnyVectorApp, ui: &mut Ui, it: &'static Item, ctx: u32) {
    let doc = app.has_doc();
    if crate::propbar::control(app, ui, it) {
        return;
    }
    match key8(it.id).as_str() {
        // Zoom Levels
        "fc07ed64" => crate::toolbar::zoom_combo(app, ui, doc),
        // Snap To: the snapping modes of View ▸ Snap To
        "9a7cfe1a" => {
            let on = enabled(app, it, ctx) && doc;
            let r = dropdown_button(ui, &ui::plain(it.text), 66.0, on).on_hover_text(tooltip(app, it));
            Popup::menu(&r).close_behavior(PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                crate::menus::menu_style(ui);
                if let Some((b, _)) = ui::bar_named("Snap To") {
                    crate::menus::show_bar(app, ui, b);
                }
            });
        }
        // Project Timer: the task and the time recorded on it
        "7e6b5490" => {
            let secs = app.time() as u64;
            let text = format!("Task 1 ({:02}:{:02})", secs / 3600, (secs / 60) % 60);
            dropdown_button(ui, &text, 96.0, doc).on_hover_text(tooltip(app, it));
            ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
        }
        // a drop-down list of commands: like a flyout
        _ if it.kind == Kind::DropDown && it.sub != NO_SUB => {
            let on = enabled(app, it, ctx);
            let r = flyout(app, ui, it, on);
            Popup::menu(&r).show(|ui| {
                crate::menus::menu_style(ui);
                popup_rows(app, ui, it);
            });
        }
        _ => {
            note_missing(it, "field");
            crate::propbar::placeholder(app, ui, it)
        }
    }
}
