//! Command bar customization, as in CorelDRAW (#124, #136): the order and the items of the
//! toolbars, the property bars and the toolbox, the command bars the user makes, and how their
//! buttons look.
//!
//! - **Alt+drag** a button (or a separator) along its bar to move it, onto another bar to move it
//!   there (**Ctrl+Alt** copies), or off every bar to remove it. While Options ▸ Customization is
//!   open the buttons drag without Alt.
//! - Drag a command from Options ▸ Customization ▸ Commands onto any bar to add it.
//! - Quick Customize shows and hides items; right-click ▸ Customize resets a bar, adds a
//!   separator or deletes the item clicked.
//!
//! The reference bars (`crate::ui::data`) are fixed; what the user changes is kept in
//! [`BarLayout`]s (`Settings::bar_layouts`), which the bars draw in place of the reference
//! order. Item ids that no longer exist are skipped, and a layout none of whose items exist
//! falls back to the reference order. The layouts belong to the workspace
//! (`crate::workspaces::Layout`).

use crate::app::AnyVectorApp;
use crate::commands::Cmd;
use crate::settings::{ButtonSize, Settings};
use crate::theme;
use crate::ui::{self, Bar, Face, Item, Kind, NO_SUB};
use egui::{pos2, vec2, Color32, Id, LayerId, Order, Pos2, Rect, Stroke};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// What the user changed on one command bar, or a command bar the user made.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BarLayout {
    /// The bar: the reference bar's id, [`PROPERTY_BARS`] (the look all property bars share), or
    /// `custom:<n>` for a bar the user made.
    pub bar: String,
    /// The name of a bar the user made.
    pub name: String,
    /// The items in order: reference item ids, "" for a separator. `None` keeps the reference
    /// order.
    pub items: Option<Vec<String>>,
    /// How the buttons show: [`STYLES`].
    pub style: u8,
    /// The size of the buttons; `None` is the global button size.
    pub size: Option<ButtonSize>,
    /// A bar the user made is open.
    pub shown: bool,
}

/// Prefix of the keys of the command bars the user makes.
pub const CUSTOM: &str = "custom:";
/// The key under which the property bars keep their look (they all share one).
pub const PROPERTY_BARS: &str = "propbar";
/// Command Bars ▸ Style, by [`BarLayout::style`].
pub const STYLES: [&str; 4] = ["Default", "Image only", "Text only", "Image and text"];

/// A separator in a resolved item list.
pub static SEPARATOR: Item = Item { id: "", text: "", tip: "", kind: Kind::Separator, keys: &[], sub: NO_SUB, icon: "", menu_icon: false, on: u32::MAX, face: Face::Default, flags: 0 };

/// The item with this id, from any reference bar (its first appearance).
pub fn find_item(id: &str) -> Option<&'static Item> {
    static INDEX: OnceLock<HashMap<&'static str, &'static Item>> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        let mut m = HashMap::new();
        for b in ui::bars() {
            for it in b.items {
                if it.kind != Kind::Separator && !it.id.is_empty() {
                    m.entry(it.id).or_insert(it);
                }
            }
        }
        m
    });
    index.get(id).copied()
}

/// The item a command stands on, for placing the command on a bar: a toolbar's or the toolbox's
/// button where there is one, else a menu's.
pub fn item_for_cmd(cmd: Cmd) -> Option<&'static Item> {
    let mut best: Option<(u8, &'static Item)> = None;
    for b in ui::bars() {
        for it in b.items {
            if !matches!(it.kind, Kind::Button | Kind::Check | Kind::Radio | Kind::ButtonFlyout) || ui::bind::cmd(it.id) != Some(cmd) {
                continue;
            }
            let rank = if it.icon.is_empty() || it.icon == "new" { 2 } else if it.text.is_empty() || !it.tip.is_empty() { 0 } else { 1 };
            if best.is_none_or(|(r, _)| rank < r) {
                best = Some((rank, it));
            }
        }
    }
    best.map(|(_, it)| it)
}

pub fn is_custom(key: &str) -> bool {
    key.starts_with(CUSTOM)
}

/// The reference bar with this id.
pub fn reference(key: &str) -> Option<&'static Bar> {
    ui::bars().iter().find(|b| b.id == key)
}

pub fn layout<'a>(s: &'a Settings, key: &str) -> Option<&'a BarLayout> {
    s.bar_layouts.iter().find(|l| l.bar == key)
}

fn layout_mut<'a>(s: &'a mut Settings, key: &str) -> &'a mut BarLayout {
    match s.bar_layouts.iter().position(|l| l.bar == key) {
        Some(i) => &mut s.bar_layouts[i],
        None => {
            s.bar_layouts.push(BarLayout { bar: key.to_string(), ..Default::default() });
            s.bar_layouts.last_mut().unwrap()
        }
    }
}

/// Drops a layout that no longer changes anything.
fn tidy(s: &mut Settings, key: &str) {
    s.bar_layouts.retain(|l| is_custom(&l.bar) || l.bar != key || l.items.is_some() || l.style != 0 || l.size.is_some());
}

/// The items `ids` name, looked up in `bar` first and then in every bar.
fn resolve(bar: Option<&'static Bar>, ids: &[String]) -> Vec<&'static Item> {
    ids.iter()
        .filter_map(|id| {
            if id.is_empty() {
                return Some(&SEPARATOR);
            }
            bar.and_then(|b| b.items.iter().find(|i| i.id == id)).or_else(|| find_item(id))
        })
        .collect()
}

/// The items a bar shows, in order: the user's layout, or the reference order.
pub fn items_of(s: &Settings, key: &str) -> Vec<&'static Item> {
    let bar = reference(key);
    let reference_items = || bar.map(|b| b.items.iter().collect()).unwrap_or_default();
    match layout(s, key).and_then(|l| l.items.as_ref()) {
        Some(ids) => {
            let v = resolve(bar, ids);
            // a layout whose items are all gone (an older or newer AnyVector) shows the reference
            if bar.is_some() && !ids.iter().all(|i| i.is_empty()) && v.iter().all(|i| i.kind == Kind::Separator) {
                reference_items()
            } else {
                v
            }
        }
        None => reference_items(),
    }
}

fn ids_of(items: &[&Item]) -> Vec<String> {
    items.iter().map(|i| if i.kind == Kind::Separator { String::new() } else { i.id.to_string() }).collect()
}

/// Whether `it` can go on the toolbox, which only holds buttons.
fn fits_toolbox(it: &Item) -> bool {
    matches!(it.kind, Kind::Button | Kind::Check | Kind::Radio | Kind::Flyout | Kind::ToolFlyout | Kind::ButtonFlyout | Kind::Separator)
}

/// Puts the item `id` ("" for a separator) on the bar `key` at `at`.
pub fn insert(s: &mut Settings, key: &str, at: usize, id: &str) {
    let mut ids = ids_of(&items_of(s, key));
    ids.insert(at.min(ids.len()), id.to_string());
    layout_mut(s, key).items = Some(ids);
}

/// Takes the item at `at` off the bar `key`; returns its id.
pub fn remove(s: &mut Settings, key: &str, at: usize) -> Option<String> {
    let mut ids = ids_of(&items_of(s, key));
    if at >= ids.len() {
        return None;
    }
    let id = ids.remove(at);
    layout_mut(s, key).items = Some(ids);
    Some(id)
}

/// Back to the reference: the order, the items and the look of the bar `key`.
pub fn reset(s: &mut Settings, key: &str) {
    if is_custom(key) {
        return;
    }
    s.bar_layouts.retain(|l| l.bar != key);
}

/// Whether the item `id` of the reference bar `key` is on the bar (Quick Customize).
pub fn shown(s: &Settings, key: &str, id: &str) -> bool {
    items_of(s, key).iter().any(|i| i.id == id)
}

/// Quick Customize: shows or hides the reference item `id` of the bar `key`. An item shown again
/// goes back after the reference item before it that is still on the bar.
pub fn set_shown(s: &mut Settings, key: &str, id: &str, on: bool) {
    let cur = items_of(s, key);
    let mut ids = ids_of(&cur);
    if !on {
        ids.retain(|i| i != id);
    } else if !ids.iter().any(|i| i == id) {
        let Some(bar) = reference(key) else { return };
        let Some(k) = bar.items.iter().position(|i| i.id == id) else { return };
        let at = bar.items[..k].iter().rev().filter(|i| i.kind != Kind::Separator).find_map(|p| ids.iter().position(|i| i == p.id)).map(|p| p + 1).unwrap_or(0);
        ids.insert(at, id.to_string());
    }
    if bar_matches_reference(key, &ids) {
        layout_mut(s, key).items = None;
        tidy(s, key);
    } else {
        layout_mut(s, key).items = Some(ids);
    }
}

fn bar_matches_reference(key: &str, ids: &[String]) -> bool {
    reference(key).is_some_and(|b| ids_of(&b.items.iter().collect::<Vec<_>>()) == ids)
}

// ------------------------------------------------------------------------------------------
// The bars the user makes
// ------------------------------------------------------------------------------------------

/// The command bars the user made, in the order they were made.
pub fn custom_bars(s: &Settings) -> impl Iterator<Item = &BarLayout> {
    custom_bars_of(&s.bar_layouts)
}

/// Command Bars ▸ New: an empty bar, open, called `name` (or "New Toolbar <n>"); returns its key.
pub fn new_bar(s: &mut Settings, name: &str) -> String {
    let n = (1..).find(|n| !s.bar_layouts.iter().any(|l| l.bar == format!("{CUSTOM}{n}"))).unwrap_or(1);
    let name = match name.trim() {
        "" => {
            let k = (1..).find(|k| !custom_bars(s).any(|l| l.name == format!("New Toolbar {k}"))).unwrap_or(1);
            format!("New Toolbar {k}")
        }
        t => t.to_string(),
    };
    let key = format!("{CUSTOM}{n}");
    s.bar_layouts.push(BarLayout { bar: key.clone(), name, items: Some(vec![]), shown: true, ..Default::default() });
    key
}

/// The bars the user made among `layouts`.
pub fn custom_bars_of(layouts: &[BarLayout]) -> impl Iterator<Item = &BarLayout> {
    layouts.iter().filter(|l| is_custom(&l.bar))
}

/// The workspace element (Export and Import Workspace) a layout belongs to: the toolbar's.
pub fn element_of(l: &BarLayout) -> Option<String> {
    use crate::app::Toolbar;
    if is_custom(&l.bar) {
        return Some(format!("bar:c:{}", l.bar));
    }
    if l.bar == PROPERTY_BARS {
        return Some("bar:property".into());
    }
    let b = reference(&l.bar)?;
    if b.name.starts_with("Property Bar") {
        return Some("bar:property".into());
    }
    match b.name {
        "Toolbox" => Some("bar:toolbox".into()),
        "Standard" => Some("bar:standard".into()),
        _ => Toolbar::EXTRA.into_iter().find(|t| crate::cmdbar::toolbar_bar(*t).and_then(ui::bar).is_some_and(|x| x.id == b.id)).map(|t| format!("bar:x:{}", t.name())),
    }
}

pub fn delete_bar(s: &mut Settings, key: &str) {
    if is_custom(key) {
        s.bar_layouts.retain(|l| l.bar != key);
    }
}

/// A `'static` copy of a bar's name, for the right-click menu (one copy per name).
pub fn intern(name: &str) -> &'static str {
    static NAMES: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
    let mut v = NAMES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(n) = v.iter().find(|n| **n == name) {
        return n;
    }
    let n: &'static str = Box::leak(name.to_string().into_boxed_str());
    v.push(n);
    n
}

// ------------------------------------------------------------------------------------------
// How the buttons look
// ------------------------------------------------------------------------------------------

/// The key a bar keeps its look under: the property bars share one.
pub fn look_key(bar: &Bar) -> &'static str {
    if bar.name.starts_with("Property Bar") {
        PROPERTY_BARS
    } else {
        bar.id
    }
}

/// The style and the button size of the bar `key`.
pub fn look(s: &Settings, key: &str) -> (u8, ButtonSize) {
    let l = layout(s, key);
    (l.map(|l| l.style).unwrap_or(0), l.and_then(|l| l.size).unwrap_or(s.button_size))
}

pub fn set_style(s: &mut Settings, key: &str, style: u8) {
    layout_mut(s, key).style = style.min(STYLES.len() as u8 - 1);
    tidy(s, key);
}

pub fn set_size(s: &mut Settings, key: &str, size: Option<ButtonSize>) {
    layout_mut(s, key).size = size;
    tidy(s, key);
}

/// How an item shows on a bar of this style: the item's own face, its image, its caption, or both.
pub fn face(style: u8, it: &Item) -> Face {
    match style {
        1 if !it.icon.is_empty() => Face::Default,
        2 => Face::TextOnly,
        3 if !it.icon.is_empty() => Face::TextRight,
        3 => Face::TextOnly,
        _ => it.face,
    }
}

// ------------------------------------------------------------------------------------------
// Dragging
// ------------------------------------------------------------------------------------------

/// An item being dragged.
#[derive(Clone, Debug)]
struct Drag {
    /// The item ("" for a separator).
    id: &'static str,
    /// The bar and the place it comes from; `None` from the Commands list.
    from: Option<(String, usize)>,
    copy: bool,
    start: Pos2,
    /// Moved far enough to be a drag (an Alt+click without moving does nothing).
    active: bool,
}

/// A bar drawn this frame: where it is and where its items are.
#[derive(Clone, Debug)]
struct Area {
    key: String,
    rect: Rect,
    vertical: bool,
    toolbox: bool,
    layer: LayerId,
    len: usize,
    /// The drawn items: (index in the bar's list, rectangle).
    items: Vec<(usize, Rect)>,
}

fn drag_id() -> Id {
    Id::unique("customize_drag")
}

fn areas_id() -> Id {
    Id::unique("customize_areas")
}

/// The bars drawn last frame (whole: this frame's list fills up as the bars are drawn).
fn drawn_id() -> Id {
    Id::unique("customize_drawn")
}

/// Where the items of the bar `key` were drawn last frame: (index in the bar's list, rectangle).
#[cfg(test)]
pub fn drawn_items(ctx: &egui::Context, key: &str) -> Vec<(usize, Rect)> {
    ctx.data(|d| d.get_temp::<Vec<Area>>(drawn_id())).unwrap_or_default().into_iter().filter(|a| a.key == key).flat_map(|a| a.items).collect()
}

/// Where the bar `key` was drawn last frame.
#[cfg(test)]
pub fn drawn_area(ctx: &egui::Context, key: &str) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Vec<Area>>(drawn_id())).unwrap_or_default().into_iter().find(|a| a.key == key).map(|a| a.rect)
}

fn get_drag(ctx: &egui::Context) -> Option<Drag> {
    ctx.data(|d| d.get_temp::<Drag>(drag_id()))
}

fn set_drag(ctx: &egui::Context, d: Option<Drag>) {
    ctx.data_mut(|m| match d {
        Some(d) => {
            m.insert_temp(drag_id(), d);
        }
        None => m.remove::<Drag>(drag_id()),
    });
}

/// Whether an item is being dragged (or pressed with Alt): the bars then don't run their buttons.
pub fn dragging(ctx: &egui::Context) -> bool {
    get_drag(ctx).is_some()
}

/// Options ▸ Customization is open: the buttons on the bars drag without Alt.
pub fn customize_mode(app: &AnyVectorApp) -> bool {
    app.dialogs.options.open && app.dialogs.options.category == Some(crate::commands::OptionsPage::Customization)
}

/// The Options dialog's rectangle (presses on it are the dialog's own).
fn options_rect(ctx: &egui::Context) -> Rect {
    let off = ctx.data(|d| d.get_temp::<egui::Vec2>(Id::unique(("dlg_offset", "options")))).unwrap_or_default();
    Rect::from_center_size(ctx.content_rect().center() + off, crate::dialogs::options::SIZE)
}

/// Records where a bar's items are as they are drawn, and starts a drag on an Alt+press.
pub struct Recorder {
    area: Area,
    press: Option<(Pos2, bool)>,
}

impl Recorder {
    /// `rect` is the bar's whole area (a drop anywhere on it adds to the bar).
    pub fn new(app: &AnyVectorApp, ui: &egui::Ui, key: &str, len: usize, rect: Rect, vertical: bool, toolbox: bool) -> Recorder {
        let ctx = ui.ctx();
        let custom = customize_mode(app);
        let press = ctx.input(|i| {
            let origin = i.pointer.press_origin()?;
            (i.pointer.primary_pressed() && (i.modifiers.alt || custom)).then_some((origin, i.modifiers.ctrl || i.modifiers.command))
        });
        // only a press on this bar itself (not on a menu or a window over it, nor on the dialog)
        let press = press.filter(|(p, _)| if custom { !options_rect(ctx).contains(*p) } else { ctx.layer_id_at(*p) == Some(ui.layer_id()) });
        Recorder { area: Area { key: key.to_string(), rect, vertical, toolbox, layer: ui.layer_id(), len, items: vec![] }, press }
    }

    /// The item `id` at `index` of the bar's list was drawn in `rect`.
    pub fn item(&mut self, ctx: &egui::Context, index: usize, rect: Rect, id: &'static str) {
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        self.area.items.push((index, rect));
        if let Some((p, copy)) = self.press {
            if rect.contains(p) && get_drag(ctx).is_none() {
                set_drag(ctx, Some(Drag { id, from: Some((self.area.key.clone(), index)), copy, start: p, active: false }));
            }
        }
    }

    pub fn finish(self, ctx: &egui::Context) {
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<Area>>(areas_id()).push(self.area));
    }
}

/// The Commands list: dragging a command from it puts the command on the bar it is dropped on.
pub fn drag_cmd(ctx: &egui::Context, resp: egui::Response, cmd: Cmd) -> egui::Response {
    let resp = resp.interact(egui::Sense::drag());
    if resp.drag_started() {
        if let (Some(it), Some(p)) = (item_for_cmd(cmd), resp.interact_pointer_pos()) {
            set_drag(ctx, Some(Drag { id: it.id, from: None, copy: true, start: p, active: true }));
        }
    }
    if resp.hovered() && !dragging(ctx) {
        ctx.set_cursor_icon(egui::CursorIcon::Grab);
    }
    resp
}

/// Where a drop at `p` goes: the bar and the place in its list, and the line that shows it.
fn drop_target(areas: &[Area], p: Pos2) -> Option<(&Area, usize, [Pos2; 2])> {
    let rank = |a: &Area| match a.layer.order {
        Order::Background => 0,
        Order::Middle => 1,
        _ => 2,
    };
    let a = areas.iter().filter(|a| a.rect.expand(2.0).contains(p)).max_by_key(|a| rank(a))?;
    let Some((k, r)) = a.items.iter().min_by(|x, y| x.1.distance_sq_to_pos(p).total_cmp(&y.1.distance_sq_to_pos(p))) else {
        // an empty bar: at its start
        let r = a.rect.shrink(3.0);
        let line = if a.vertical { [r.left_top(), r.right_top()] } else { [r.left_top(), r.left_bottom()] };
        return Some((a, 0, line));
    };
    let before = if a.vertical { p.y < r.center().y } else { p.x < r.center().x };
    let at = if before { *k } else { k + 1 };
    let line = match (a.vertical, before) {
        (false, true) => [pos2(r.left() - 1.0, r.top()), pos2(r.left() - 1.0, r.bottom())],
        (false, false) => [pos2(r.right() + 1.0, r.top()), pos2(r.right() + 1.0, r.bottom())],
        (true, true) => [pos2(r.left(), r.top() - 1.0), pos2(r.right(), r.top() - 1.0)],
        (true, false) => [pos2(r.left(), r.bottom() + 1.0), pos2(r.right(), r.bottom() + 1.0)],
    };
    Some((a, at.min(a.len), line))
}

/// What a drop does.
#[derive(Clone, Debug, PartialEq)]
enum Drop {
    /// Put the item on the bar at this place (taking it off where it was, unless copied).
    Place(String, usize),
    /// Take it off its bar.
    Remove,
    Nothing,
}

fn drop_of(d: &Drag, target: Option<(&Area, usize)>) -> Drop {
    let item = if d.id.is_empty() { Some(&SEPARATOR) } else { find_item(d.id) };
    match target {
        Some((a, _)) if a.toolbox && !item.is_some_and(fits_toolbox) => Drop::Nothing,
        Some((a, at)) => match &d.from {
            // dropped where it was
            Some((k, i)) if *k == a.key && !d.copy && (at == *i || at == i + 1) => Drop::Nothing,
            _ => Drop::Place(a.key.clone(), at),
        },
        None if d.from.is_some() && !d.copy => Drop::Remove,
        None => Drop::Nothing,
    }
}

/// Carries out a drop.
fn apply_drop(s: &mut Settings, d: &Drag, drop: &Drop) {
    match drop {
        Drop::Nothing => {}
        Drop::Remove => {
            if let Some((k, i)) = &d.from {
                remove(s, k, *i);
            }
        }
        Drop::Place(key, at) => {
            let mut at = *at;
            if let (Some((k, i)), false) = (&d.from, d.copy) {
                remove(s, k, *i);
                if k == key && *i < at {
                    at -= 1;
                }
            }
            insert(s, key, at, d.id);
        }
    }
}

/// After everything is drawn: the item being dragged follows the pointer, the line shows where
/// it goes, and the drop is made when the button is let go.
pub fn end_frame(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let areas = ctx.data_mut(|d| d.remove_temp::<Vec<Area>>(areas_id())).unwrap_or_default();
    ctx.data_mut(|d| d.insert_temp(drawn_id(), areas.clone()));
    let Some(mut d) = get_drag(ctx) else { return };
    let (pos, down, esc, ctrl) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.primary_down(), i.key_pressed(egui::Key::Escape), i.modifiers.ctrl || i.modifiers.command));
    if esc {
        set_drag(ctx, None);
        return;
    }
    let Some(pos) = pos else { return };
    if !d.active && pos.distance(d.start) > 4.0 {
        d.active = true;
    }
    if !d.active {
        set_drag(ctx, if down { Some(d) } else { None });
        return;
    }
    d.copy |= d.from.is_some() && ctrl;
    let target = drop_target(&areas, pos);
    let drop = drop_of(&d, target.as_ref().map(|(a, at, _)| (*a, *at)));
    if !down {
        let before = app.settings.bar_layouts.clone();
        apply_drop(&mut app.settings, &d, &drop);
        if app.settings.bar_layouts != before {
            app.settings.save();
        }
        set_drag(ctx, None);
        ctx.request_repaint();
        return;
    }
    // the item at the pointer, and the place it would go
    let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::unique("customize_ghost")));
    if let (Some((_, _, line)), Drop::Place(..)) = (&target, &drop) {
        painter.line_segment(*line, Stroke::new(2.0, theme::FOCUS()));
    }
    let item = if d.id.is_empty() { Some(&SEPARATOR) } else { find_item(d.id) };
    let ghost = Rect::from_min_size(pos + vec2(10.0, 12.0), vec2(26.0, 26.0));
    let (text, icon) = match item {
        Some(it) if it.kind == Kind::Separator => (String::new(), ""),
        Some(it) if !it.icon.is_empty() && it.icon != "new" => (String::new(), it.icon),
        Some(it) => (ui::plain(ui::label(it)), ""),
        None => (String::new(), ""),
    };
    let ghost = if text.is_empty() { ghost } else { Rect::from_min_size(ghost.min, vec2(painter.layout_no_wrap(text.clone(), egui::FontId::proportional(12.5), theme::TEXT()).size().x + 14.0, 26.0)) };
    painter.rect_filled(ghost, 2.0, theme::BUTTON().gamma_multiply(0.9));
    painter.rect_stroke(ghost, 2.0, Stroke::new(1.0, theme::FOCUS()), egui::StrokeKind::Inside);
    if !icon.is_empty() {
        if let Some(t) = app.icons.get(ctx, icon, 18.0, crate::icons::Tone::Normal) {
            painter.image(t.id, Rect::from_center_size(ghost.center(), vec2(18.0, 18.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
    } else if !text.is_empty() {
        painter.text(ghost.left_center() + vec2(7.0, 0.0), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(12.5), theme::TEXT());
    } else {
        let x = ghost.center().x;
        painter.line_segment([pos2(x, ghost.top() + 5.0), pos2(x, ghost.bottom() - 5.0)], Stroke::new(1.0, theme::TEXT()));
    }
    match drop {
        // let go here, the item is taken off its bar
        Drop::Remove => {
            let c = ghost.right_top();
            painter.circle_filled(c, 6.0, Color32::from_rgb(0xc4, 0x2b, 0x1c));
            for v in [vec2(2.5, 2.5), vec2(2.5, -2.5)] {
                painter.line_segment([c - v, c + v], Stroke::new(1.5, Color32::WHITE));
            }
            ctx.set_cursor_icon(egui::CursorIcon::Default);
        }
        Drop::Nothing => ctx.set_cursor_icon(egui::CursorIcon::NoDrop),
        Drop::Place(..) => ctx.set_cursor_icon(if d.copy { egui::CursorIcon::Copy } else { egui::CursorIcon::Grabbing }),
    }
    set_drag(ctx, Some(d));
    ctx.request_repaint();
}

// ------------------------------------------------------------------------------------------
// Right-click ▸ Customize
// ------------------------------------------------------------------------------------------

/// Right-click ▸ Customize on a bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum BarOp {
    /// ▸ <bar> Toolbar ▸ Reset to Default.
    Reset,
    /// ▸ <bar> Toolbar ▸ Add Separator (after the item clicked, or at the end).
    AddSeparator,
    /// ▸ Toolbar Item ▸ Delete.
    DeleteItem,
    /// ▸ <bar> Toolbar ▸ Image Only / Caption Only / Caption to Right of Image ([`STYLES`]).
    Style(u8),
}

thread_local! {
    /// The bar (and the item on it) the last right-click on a command bar hit.
    static CONTEXT: std::cell::RefCell<Option<(String, Option<usize>)>> = const { std::cell::RefCell::new(None) };
}

/// A right-click at `p` on a command bar: remembers the bar and the item it hit, for
/// right-click ▸ Customize.
pub fn note_context(ctx: &egui::Context, p: Pos2) {
    let areas = ctx.data(|d| d.get_temp::<Vec<Area>>(drawn_id()).or_else(|| d.get_temp::<Vec<Area>>(areas_id()))).unwrap_or_default();
    let rank = |a: &Area| a.layer.order as u8;
    let hit = areas.iter().filter(|a| a.rect.expand(2.0).contains(p)).max_by_key(|a| rank(a)).map(|a| (a.key.clone(), a.items.iter().find(|(_, r)| r.contains(p)).map(|(i, _)| *i)));
    CONTEXT.with(|c| *c.borrow_mut() = hit);
}

/// The bar (and item) the last right-click on a command bar hit.
pub fn context_target() -> Option<(String, Option<usize>)> {
    CONTEXT.with(|c| c.borrow().clone())
}

/// The key of the bar whose look a right-click ▸ Customize changes.
fn context_look_key(key: &str) -> String {
    reference(key).map(|b| look_key(b).to_string()).unwrap_or_else(|| key.to_string())
}

pub fn apply_op(app: &mut AnyVectorApp, op: BarOp) {
    let Some((key, item)) = context_target() else { return };
    let s = &mut app.settings;
    match op {
        BarOp::Reset => {
            reset(s, &key);
            if reference(&key).is_some_and(|b| b.name == "Toolbox") {
                s.toolbox_items.clear();
            }
        }
        BarOp::AddSeparator => {
            let at = item.map(|i| i + 1).unwrap_or_else(|| items_of(s, &key).len());
            insert(s, &key, at, "");
        }
        BarOp::DeleteItem => {
            if let Some(i) = item {
                remove(s, &key, i);
            }
        }
        BarOp::Style(st) => set_style(s, &context_look_key(&key), st),
    }
    s.save();
}

/// Whether right-click ▸ Customize ▸ <bar> Toolbar ▸ <style> is the clicked bar's style.
pub fn context_style_is(app: &AnyVectorApp, st: u8) -> bool {
    let Some((key, _)) = context_target() else { return st <= 1 };
    let cur = look(&app.settings, &context_look_key(&key)).0;
    cur == st || (st == 1 && cur == 0)
}

/// Options ▸ Cancel keeps the command bar layouts: they were changed on the bars themselves.
pub fn keep_on_cancel(backup: &mut Settings, live: &Settings) {
    backup.bar_layouts = live.bar_layouts.clone();
    backup.toolbox_items = live.toolbox_items.clone();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standard() -> &'static str {
        ui::bar_named("Standard").unwrap().1.id
    }

    fn ids(s: &Settings, key: &str) -> Vec<String> {
        ids_of(&items_of(s, key))
    }

    #[test]
    fn a_bar_without_a_layout_shows_the_reference() {
        let s = Settings::default();
        let b = reference(standard()).unwrap();
        assert_eq!(items_of(&s, standard()).len(), b.items.len());
        assert!(std::ptr::eq(items_of(&s, standard())[0], &b.items[0]));
    }

    #[test]
    fn insert_move_and_remove() {
        let mut s = Settings::default();
        let key = standard();
        let n = ids(&s, key).len();
        let first = ids(&s, key)[0].clone();
        // a command from a menu, at the start
        let zoom = item_for_cmd(Cmd::ZoomToPage).expect("Zoom to Page is on a bar");
        insert(&mut s, key, 0, zoom.id);
        assert_eq!(ids(&s, key)[0], zoom.id);
        assert_eq!(ids(&s, key)[1], first);
        // dragged along the bar to the end
        let d = Drag { id: zoom.id, from: Some((key.to_string(), 0)), copy: false, start: Pos2::ZERO, active: true };
        apply_drop(&mut s, &d, &Drop::Place(key.to_string(), n + 1));
        let v = ids(&s, key);
        assert_eq!(v.len(), n + 1);
        assert_eq!(v[0], first);
        assert_eq!(v[n], zoom.id);
        // dragged off the bar: gone
        let d = Drag { from: Some((key.to_string(), n)), ..d };
        apply_drop(&mut s, &d, &Drop::Remove);
        assert_eq!(ids(&s, key).len(), n);
        assert!(!ids(&s, key).contains(&zoom.id.to_string()));
        // reset
        reset(&mut s, key);
        assert!(layout(&s, key).is_none());
    }

    #[test]
    fn moving_between_bars_and_copying() {
        let mut s = Settings::default();
        let (a, b) = (standard(), ui::bar_named("Zoom").unwrap().1.id);
        let (na, nb) = (ids(&s, a).len(), ids(&s, b).len());
        let moved = ids(&s, a)[0].clone();
        let id = find_item(&moved).unwrap().id;
        let d = Drag { id, from: Some((a.to_string(), 0)), copy: false, start: Pos2::ZERO, active: true };
        apply_drop(&mut s, &d, &Drop::Place(b.to_string(), 0));
        assert_eq!(ids(&s, a).len(), na - 1);
        assert_eq!(ids(&s, b)[0], moved);
        // Ctrl+Alt: a copy, the original stays
        let d = Drag { id, from: Some((b.to_string(), 0)), copy: true, start: Pos2::ZERO, active: true };
        apply_drop(&mut s, &d, &Drop::Place(a.to_string(), 0));
        assert_eq!(ids(&s, a)[0], moved);
        assert_eq!(ids(&s, b).len(), nb + 1);
    }

    #[test]
    fn unknown_ids_are_skipped_and_a_layout_of_only_unknown_ids_falls_back() {
        let mut s = Settings::default();
        let key = standard();
        let n = ids(&s, key).len();
        let keep = ids(&s, key)[0].clone();
        layout_mut(&mut s, key).items = Some(vec!["no-such-item".into(), keep.clone(), String::new()]);
        assert_eq!(ids(&s, key), vec![keep, String::new()]);
        layout_mut(&mut s, key).items = Some(vec!["gone-1".into(), "gone-2".into()]);
        assert_eq!(ids(&s, key).len(), n, "falls back to the reference order");
        // an emptied bar stays empty
        layout_mut(&mut s, key).items = Some(vec![]);
        assert!(ids(&s, key).is_empty());
    }

    #[test]
    fn quick_customize_hides_and_puts_back_in_place() {
        let mut s = Settings::default();
        let key = standard();
        let reference_ids = ids(&s, key);
        let id = reference_ids.iter().find(|i| !i.is_empty()).cloned().unwrap();
        let id2 = reference_ids.iter().filter(|i| !i.is_empty()).nth(3).cloned().unwrap();
        set_shown(&mut s, key, &id2, false);
        set_shown(&mut s, key, &id, false);
        assert!(!shown(&s, key, &id) && !shown(&s, key, &id2));
        set_shown(&mut s, key, &id2, true);
        set_shown(&mut s, key, &id, true);
        assert_eq!(ids(&s, key), reference_ids, "back at their places");
        assert!(layout(&s, key).is_none(), "nothing left to keep");
    }

    #[test]
    fn drops_onto_the_toolbox_take_buttons_only() {
        let toolbox = Area { key: "t".into(), rect: Rect::from_min_size(Pos2::ZERO, vec2(30.0, 300.0)), vertical: true, toolbox: true, layer: LayerId::background(), len: 3, items: vec![] };
        let button = item_for_cmd(Cmd::ZoomToPage).unwrap();
        let field = ui::bars().iter().flat_map(|b| b.items.iter()).find(|i| i.kind == Kind::Spin).unwrap();
        let drag = |id| Drag { id, from: None, copy: true, start: Pos2::ZERO, active: true };
        assert_eq!(drop_of(&drag(button.id), Some((&toolbox, 1))), Drop::Place("t".into(), 1));
        assert_eq!(drop_of(&drag(field.id), Some((&toolbox, 1))), Drop::Nothing);
        // from the list and let go off every bar: nothing
        assert_eq!(drop_of(&drag(button.id), None), Drop::Nothing);
    }

    #[test]
    fn the_drop_place_follows_the_pointer() {
        let item = |x: f32| Rect::from_min_size(pos2(x, 0.0), vec2(20.0, 20.0));
        let bar = Area { key: "b".into(), rect: Rect::from_min_size(Pos2::ZERO, vec2(400.0, 20.0)), vertical: false, toolbox: false, layer: LayerId::background(), len: 4, items: vec![(0, item(0.0)), (1, item(20.0)), (3, item(40.0))] };
        let at = |x: f32| drop_target(std::slice::from_ref(&bar), pos2(x, 10.0)).map(|(_, at, _)| at);
        assert_eq!(at(2.0), Some(0));
        assert_eq!(at(18.0), Some(1));
        assert_eq!(at(45.0), Some(3));
        assert_eq!(at(300.0), Some(4), "past the last item: at the end");
        assert_eq!(drop_target(std::slice::from_ref(&bar), pos2(10.0, 60.0)).map(|(_, at, _)| at), None, "off the bar");
        // dropped back where it was: nothing to do
        let d = Drag { id: "x", from: Some(("b".into(), 1)), copy: false, start: Pos2::ZERO, active: true };
        assert_eq!(drop_of(&d, Some((&bar, 1))), Drop::Nothing);
        assert_eq!(drop_of(&d, Some((&bar, 2))), Drop::Nothing);
        assert_eq!(drop_of(&d, None), Drop::Remove);
    }

    #[test]
    fn bars_the_user_makes() {
        let mut s = Settings::default();
        let k1 = new_bar(&mut s, "");
        let k2 = new_bar(&mut s, "Mine");
        assert_ne!(k1, k2);
        assert!(is_custom(&k1));
        assert_eq!(custom_bars(&s).map(|l| l.name.as_str()).collect::<Vec<_>>(), ["New Toolbar 1", "Mine"]);
        assert!(items_of(&s, &k1).is_empty());
        insert(&mut s, &k1, 0, item_for_cmd(Cmd::ZoomToPage).unwrap().id);
        assert_eq!(items_of(&s, &k1).len(), 1);
        // Reset leaves a bar the user made alone; Delete removes it
        reset(&mut s, &k1);
        assert_eq!(items_of(&s, &k1).len(), 1);
        delete_bar(&mut s, &k1);
        assert_eq!(custom_bars(&s).count(), 1);
        assert_eq!(new_bar(&mut s, ""), k1, "the key is used again");
    }

    #[test]
    fn styles_change_the_face() {
        let it = item_for_cmd(Cmd::ZoomToPage).unwrap();
        assert_eq!(face(0, it), it.face);
        assert_eq!(face(1, it), Face::Default);
        assert_eq!(face(2, it), Face::TextOnly);
        assert_eq!(face(3, it), Face::TextRight);
        let mut s = Settings::default();
        set_style(&mut s, PROPERTY_BARS, 2);
        set_size(&mut s, PROPERTY_BARS, Some(ButtonSize::Large));
        assert_eq!(look(&s, PROPERTY_BARS), (2, ButtonSize::Large));
        set_style(&mut s, PROPERTY_BARS, 0);
        set_size(&mut s, PROPERTY_BARS, None);
        assert!(s.bar_layouts.is_empty(), "a layout that changes nothing is dropped");
    }
}
