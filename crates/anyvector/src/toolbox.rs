//! Toolbox (left column): the reference Toolbox bar, one button per item in its order. A tool
//! flyout shows the tool last used from it and opens from its corner triangle or on
//! press-and-hold; Quick Customize (+) shows and hides items.

use crate::app::AnyVectorApp;
use crate::icons::Tone;
use crate::theme;
use crate::tools::ToolId;
use crate::ui::{self, Item, Kind, EXCLUDED, NO_SUB};
use egui::{pos2, vec2, Color32, Popup, PopupCloseBehavior, Rect, Sense, Stroke, Ui};
use std::collections::HashMap;

/// What each toolbox slot shows, and the kind each reference tool that stands for several AnyVector
/// tools (Connector, Shadow) was last used as.
#[derive(Default)]
pub struct ToolboxState {
    /// slot item id → the tool item it shows
    shown: HashMap<&'static str, &'static str>,
    /// tool item (first 8 characters) → the AnyVector tool last used for it
    kind: HashMap<&'static str, ToolId>,
}

impl ToolboxState {
    /// `t` is now the tool in use: its slot shows it from now on.
    pub fn remember(&mut self, t: ToolId) {
        let item = crate::tools::tool_item(t);
        if item.is_empty() {
            return;
        }
        if crate::tools::item_tools(item).len() > 1 {
            self.kind.insert(item, t);
        }
        for slot in slots() {
            if let Some(it) = slot_tools(slot).into_iter().find(|i| key8(i.id) == item) {
                self.shown.insert(slot.id, it.id);
            }
        }
    }

    /// The tool to start for `t`: where `t` is the first of several tools one reference tool
    /// stands for, the one of them used last.
    pub fn kind_of(&self, t: ToolId) -> ToolId {
        let item = crate::tools::tool_item(t);
        if crate::tools::item_tools(item).first() == Some(&t) {
            self.kind.get(item).copied().unwrap_or(t)
        } else {
            t
        }
    }
}

fn key8(id: &str) -> String {
    id.get(..8).unwrap_or(id).to_ascii_lowercase()
}

fn toolbox_bar() -> Option<&'static ui::Bar> {
    ui::bar_named("Toolbox").map(|(_, b)| b)
}

/// The items of the Toolbox bar (no separators).
fn slots() -> impl Iterator<Item = &'static Item> {
    toolbox_bar().into_iter().flat_map(|b| b.items.iter()).filter(|i| i.kind != Kind::Separator)
}

/// The tools a slot offers: the tools of its flyout, or the slot itself.
fn slot_tools(slot: &'static Item) -> Vec<&'static Item> {
    match slot.kind {
        Kind::ToolFlyout => ui::bar(slot.sub).map(|b| b.items.iter().filter(|i| i.kind == Kind::Radio).collect()).unwrap_or_default(),
        _ => vec![slot],
    }
}

/// Whether Quick Customize shows the item (items the reference leaves out start hidden).
fn shown(app: &AnyVectorApp, it: &Item) -> bool {
    let k = key8(it.id);
    app.settings.toolbox_items.iter().find(|(i, _)| *i == k).map(|(_, v)| *v).unwrap_or(it.flags & EXCLUDED == 0)
}

fn set_shown(app: &mut AnyVectorApp, it: &Item, on: bool) {
    let k = key8(it.id);
    app.settings.toolbox_items.retain(|(i, _)| *i != k);
    if on != (it.flags & EXCLUDED == 0) {
        app.settings.toolbox_items.push((k, on));
    }
}

/// The tool a slot shows: the one in use, else the one used last, else its first; `None` when
/// all its tools are hidden.
fn current(app: &AnyVectorApp, slot: &'static Item) -> Option<&'static Item> {
    let tools: Vec<&'static Item> = slot_tools(slot).into_iter().filter(|i| shown(app, i)).collect();
    tools
        .iter()
        .find(|i| crate::tools::item_tools(i.id).contains(&app.tool))
        .or_else(|| app.toolbox.shown.get(slot.id).and_then(|id| tools.iter().find(|i| i.id == *id)))
        .or(tools.first())
        .copied()
}

fn icon_of(it: &Item) -> &'static str {
    if !it.icon.is_empty() {
        return it.icon;
    }
    crate::tools::item_tools(it.id).first().map(|t| t.icon()).unwrap_or("blank")
}

pub fn toolbox(app: &mut AnyVectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    ui.spacing_mut().item_spacing.y = 1.0;
    let w = 30.0;
    let Some(bar) = toolbox_bar() else { return };
    // a separator shows only between visible buttons
    let (mut any, mut sep) = (false, false);
    for slot in bar.items {
        if slot.kind == Kind::Separator {
            sep = any;
            continue;
        }
        let Some(cur) = current(app, slot) else { continue };
        if sep {
            let (r, _) = ui.allocate_exact_size(vec2(w, 9.0), Sense::hover());
            ui.painter().line_segment([pos2(r.left() + 4.0, r.center().y), pos2(r.right() - 4.0, r.center().y)], Stroke::new(1.0, theme::SEP));
            sep = false;
        }
        any = true;
        slot_button(app, ui, slot, cur, doc, w);
    }
    ui.add_space(4.0);
    // Quick Customize
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect.shrink(1.0), 2.0, theme::HOVER);
    }
    if let Some(t) = app.icons.get(ui.ctx(), "plus", 16.0, Tone::Normal) {
        ui.painter().image(t.id, Rect::from_center_size(rect.center(), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    let resp = resp.on_hover_text("Quick Customize");
    Popup::menu(&resp).close_behavior(PopupCloseBehavior::CloseOnClickOutside).align(egui::RectAlign::RIGHT_START).show(|ui| {
        quick_customize(app, ui);
    });
}

fn slot_button(app: &mut AnyVectorApp, ui: &mut Ui, slot: &'static Item, cur: &'static Item, doc: bool, w: f32) {
    let active = doc && crate::tools::item_tools(cur.id).contains(&app.tool);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), if doc { Sense::click_and_drag() } else { Sense::hover() });
    let p = ui.painter();
    if active {
        p.rect_filled(rect.shrink(1.0), 2.0, theme::PRESSED);
    } else if doc && resp.hovered() {
        p.rect_filled(rect.shrink(1.0), 2.0, theme::HOVER);
    }
    let tone = if !doc {
        Tone::Disabled
    } else if active {
        Tone::Bright
    } else {
        Tone::Normal
    };
    if let Some(t) = app.icons.get(ui.ctx(), icon_of(cur), 20.0, tone) {
        p.image(t.id, Rect::from_center_size(rect.center() - vec2(1.0, 0.0), vec2(20.0, 20.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    let has_flyout = slot.sub != NO_SUB && matches!(slot.kind, Kind::ToolFlyout | Kind::Flyout);
    if has_flyout {
        let c = if doc { Color32::from_gray(0xb0) } else { Color32::from_gray(0x55) };
        let br = rect.right_bottom() - vec2(3.0, 3.0);
        p.add(egui::Shape::convex_polygon(vec![br, br - vec2(5.0, 0.0), br - vec2(0.0, 5.0)], c, Stroke::NONE));
    }
    let tip = crate::cmdbar::tooltip(app, cur);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, doc, &tip));
    let resp = resp.on_hover_text(tip);
    // the flyout opens from the corner triangle, on press-and-hold or on a drag; a flyout of
    // commands (Outline Pen) opens on any click
    let mut open = false;
    if resp.clicked() {
        let in_corner = resp.interact_pointer_pos().is_some_and(|p| p.x > rect.right() - 9.0 && p.y > rect.bottom() - 9.0);
        if has_flyout && (in_corner || slot.kind == Kind::Flyout) {
            open = true;
        } else {
            crate::cmdbar::run(app, cur);
        }
    }
    if has_flyout && (resp.drag_started() || (resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.press_start_time().is_some_and(|t| i.time - t > 0.45)))) {
        open = true;
    }
    let hidden: Vec<String> = ui::bar(slot.sub).map(|b| b.items.iter().filter(|i| !shown(app, i)).map(|i| key8(i.id)).collect()).unwrap_or_default();
    // a menu next to the button, its first row level with the button
    Popup::new(ui.make_persistent_id(("flyout", slot.id)), ui.ctx().clone(), egui::PopupAnchor::Position(rect.right_top() + vec2(2.0, -4.0)), ui.layer_id())
        .kind(egui::PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .style(egui::containers::menu::menu_style)
        .gap(0.0)
        .open_memory(open.then_some(egui::SetOpenCommand::Bool(true)))
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .show(|ui| crate::menus::show_flyout(app, ui, slot.sub, |it| it.kind == Kind::Radio && hidden.contains(&key8(it.id))));
}

/// The name Quick Customize gives a slot: its flyout's name, or its own caption.
fn slot_title(slot: &Item) -> String {
    match slot.kind {
        Kind::ToolFlyout => ui::bar(slot.sub).map(|b| b.name.to_string()).unwrap_or_default(),
        _ => ui::plain(slot.text),
    }
}

fn quick_customize(app: &mut AnyVectorApp, ui: &mut Ui) {
    ui.set_min_width(240.0);
    egui::ScrollArea::vertical().max_height(ui.ctx().content_rect().height() - 120.0).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for (si, slot) in slots().enumerate() {
            if si > 0 {
                let r = ui.available_rect_before_wrap();
                ui.painter().line_segment([pos2(r.left(), r.top() + 2.0), pos2(r.right(), r.top() + 2.0)], Stroke::new(1.0, Color32::from_rgb(0x2a, 0x6a, 0xc8)));
                ui.add_space(5.0);
            }
            ui.label(egui::RichText::new(slot_title(slot)).color(theme::TEXT_DIM));
            for it in slot_tools(slot) {
                let mut on = shown(app, it);
                let key = crate::ui::bind::cmd(it.id).and_then(|c| crate::shortcuts::shortcut_text(app, c)).or_else(|| it.keys.first().map(|k| k.to_string()));
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut on, "").changed() {
                        set_shown(app, it, on);
                    }
                    if let Some(img) = app.icons.image(ui.ctx(), icon_of(it), 16.0, true) {
                        ui.add(img);
                    }
                    ui.label(ui::plain(ui::label(it)));
                    if let Some(k) = key.filter(|k| !k.is_empty()) {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(k).color(theme::TEXT_DIM));
                        });
                    }
                });
            }
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Reset toolbar").clicked() {
            app.settings.toolbox_items.clear();
        }
        if ui.button("Customize...").clicked() {
            app.run(crate::commands::Cmd::Options(crate::commands::OptionsPage::Customization));
            app.dialogs.options.page = Some("Commands".into());
            ui.close();
        }
    });
}
