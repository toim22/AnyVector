//! Menus drawn from the reference interface data (`crate::ui`): the menu bar (the full one with
//! a document open, File · Tools · Window · Help without), its submenus and the right-click
//! menus, item by item in the reference order, with the reference separators, shortcuts and
//! enabled states.

use crate::app::{Toolbar, AnyVectorApp};
use crate::commands::Cmd;
use crate::theme;
use crate::ui::{self, bind, Item, Kind, ADD_SEPARATOR, HIDE_DISABLED, NO_SUB, UNKNOWN_STATE};
use crate::widgets::mnemonic;
use egui::menu::{MenuButton, SubMenuButton};
use egui::{Button, Image, MenuBar, Ui};

/// What choosing a menu row does.
#[derive(Clone)]
enum Act {
    Cmd(Cmd),
    /// A reference item AnyVector does not have yet: says so.
    Pending(String),
    /// Opens the bar as a submenu.
    Sub(u16),
    /// Opens a submenu of run-time rows.
    Rows(Vec<Row>),
    OpenPalette(String),
    None,
}

#[derive(Clone, Copy, PartialEq)]
enum Mark {
    None,
    Check(bool),
    Radio(bool),
}

#[derive(Clone)]
struct Entry {
    text: String,
    icon: Option<&'static str>,
    key: Option<String>,
    mark: Mark,
    enabled: bool,
    act: Act,
}

#[derive(Clone)]
enum Row {
    Sep,
    Item(Entry),
}

impl Entry {
    fn new(text: impl Into<String>, act: Act) -> Entry {
        Entry { text: text.into(), icon: None, key: None, mark: Mark::None, enabled: true, act }
    }
    fn mark(mut self, m: Mark) -> Entry {
        self.mark = m;
        self
    }
    fn enabled(mut self, e: bool) -> Entry {
        self.enabled = e;
        self
    }
}

fn key8(id: &str) -> String {
    id.get(..8).unwrap_or(id).to_ascii_lowercase()
}

/// The shortcut shown for an item: the user's own if they changed it, else the reference one.
fn shortcut_for(app: &AnyVectorApp, it: &Item, cmd: Option<Cmd>) -> Option<String> {
    if let Some(c) = cmd {
        let name = format!("{c:?}");
        if let Some((_, s)) = app.settings.shortcuts.iter().find(|(n, _)| *n == name) {
            return Some(s.clone());
        }
    }
    it.keys.first().map(|k| k.to_string())
}

/// `strict`: the item sits in a placeholder that hides disabled items, so an item the reference
/// shows no state for is off unless AnyVector knows it applies.
fn entry(app: &AnyVectorApp, it: &'static Item, ctx: u32, strict: bool) -> Entry {
    let cmd = bind::cmd(it.id);
    let text = spot_caption(it).or_else(|| bind::caption(app, it)).unwrap_or_else(|| ui::label(it).to_string());
    let enabled = bind::enabled(app, it.id).unwrap_or(it.on & ctx != 0 && !(strict && it.flags & UNKNOWN_STATE != 0));
    let mark = match it.kind {
        Kind::Check => Mark::Check(bind::checked(app, it.id).unwrap_or(false)),
        Kind::Radio => Mark::Radio(bind::checked(app, it.id).unwrap_or(false)),
        _ => Mark::None,
    };
    let act = match it.kind {
        Kind::Flyout | Kind::ButtonFlyout if it.sub != NO_SUB => Act::Sub(it.sub),
        _ => match cmd {
            Some(c) => Act::Cmd(c),
            None => Act::Pending(ui::plain(&text)),
        },
    };
    // a submenu made only of run-time lists is unavailable while those lists are empty
    let enabled = enabled
        && match act {
            Act::Sub(b) => {
                let bar = ui::bar(b).map(|b| b.items).unwrap_or_default();
                !bar.iter().all(|i| i.kind == Kind::Placeholder) || !rows(app, b, ctx, false).is_empty()
            }
            _ => true,
        };
    Entry { text, icon: (it.menu_icon && !it.icon.is_empty()).then_some(it.icon), key: shortcut_for(app, it, cmd), mark, enabled, act }
}

/// The rows of a bar in the current state, with run-time lists filled in and separators tidied.
fn rows(app: &AnyVectorApp, bar: u16, ctx: u32, strict: bool) -> Vec<Row> {
    let mut out = raw_rows(app, bar, ctx, strict);
    tidy(&mut out);
    out
}

/// The rows of a bar with its own separators kept, also at the ends (a placeholder brings its
/// bar's leading and trailing separators along when it brings any item).
fn raw_rows(app: &AnyVectorApp, bar: u16, ctx: u32, strict: bool) -> Vec<Row> {
    let mut out = vec![];
    if let Some(b) = ui::bar(bar) {
        for it in b.items {
            match it.kind {
                Kind::Separator => out.push(Row::Sep),
                Kind::Placeholder => placeholder(app, it, ctx, strict, &mut out),
                Kind::GroupBegin | Kind::GroupEnd | Kind::Spacer | Kind::Anchors => {}
                _ if !spot_shows(it) || !selection_shows(app, it) => {}
                _ if it.flags & ui::RUNTIME_VISIBLE != 0 && !bind::visible(app, it.id) => {}
                _ => out.push(Row::Item(entry(app, it, ctx, strict))),
            }
        }
    }
    out
}

/// No separator at either end or twice in a row.
fn tidy(rows: &mut Vec<Row>) {
    let mut out: Vec<Row> = Vec::with_capacity(rows.len());
    for r in rows.drain(..) {
        if matches!(r, Row::Sep) && matches!(out.last(), None | Some(Row::Sep)) {
            continue;
        }
        out.push(r);
    }
    while matches!(out.last(), Some(Row::Sep)) {
        out.pop();
    }
    *rows = out;
}

fn cmd_row(text: impl Into<String>, cmd: Cmd) -> Row {
    Row::Item(Entry::new(text, Act::Cmd(cmd)))
}

/// The Windows "Send To" folder entries ("Desktop (create shortcut)", "Documents", …).
fn send_to_links() -> Vec<String> {
    let Some(dir) = dirs::data_dir().map(|d| d.join("Microsoft\\Windows\\SendTo")) else {
        return vec![];
    };
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.file_name().is_some_and(|n| !n.eq_ignore_ascii_case("desktop.ini")))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
                .collect()
        })
        .unwrap_or_default();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// Fill in a placeholder: the items of its source bar, or a run-time list.
fn placeholder(app: &AnyVectorApp, it: &'static Item, ctx: u32, strict: bool, out: &mut Vec<Row>) {
    if it.sub != NO_SUB {
        let hide = it.flags & HIDE_DISABLED != 0;
        let mut inner = raw_rows(app, it.sub, ctx, strict || hide);
        if hide {
            inner.retain(|r| match r {
                Row::Item(e) => e.enabled,
                Row::Sep => true,
            });
        }
        if inner.iter().any(|r| matches!(r, Row::Item(_))) {
            if it.flags & ADD_SEPARATOR != 0 {
                out.push(Row::Sep);
            }
            out.extend(inner);
        }
        return;
    }
    match key8(it.id).as_str() {
        // File ▸ Open Recent
        "d35d9eb3" => {
            for (i, p) in app.settings.recent_files.iter().enumerate() {
                out.push(cmd_row(format!("&{} {}", (i + 1) % 10, p.display()), Cmd::OpenRecent(i)));
            }
        }
        // File ▸ Send To
        "4038b5c7" => {
            for name in send_to_links() {
                let lower = name.to_lowercase();
                let act = if lower.starts_with("desktop") {
                    Act::Cmd(Cmd::SendToDesktop)
                } else if lower.starts_with("documents") {
                    Act::Cmd(Cmd::SendToDocuments)
                } else if lower.contains("mail") {
                    Act::Cmd(Cmd::SendToMail)
                } else {
                    Act::Pending(format!("Send To {name}"))
                };
                out.push(Row::Item(Entry::new(name, act).enabled(app.has_doc())));
            }
        }
        // Window ▸ Toolbars: every toolbar but the menu and status bars
        "1cff26b2" => {
            for tb in [Toolbar::Standard, Toolbar::PropertyBar, Toolbar::Toolbox].into_iter().chain(Toolbar::EXTRA) {
                let on = bind::toolbar_on(app, tb);
                out.push(Row::Item(Entry::new(tb.name(), Act::Cmd(Cmd::ToggleToolbar(tb))).mark(Mark::Check(on))));
            }
        }
        // Window ▸ Workspace
        "a124f904" => {
            let ws = app.settings.workspace.clone();
            let radio = |name: &str, i: u8| Row::Item(Entry::new(name, Act::Cmd(Cmd::Workspace(i))).mark(Mark::Radio(ws == crate::widgets::plain(name))));
            out.push(radio("&Lite", 0));
            out.push(radio("&Default", 1));
            out.push(Row::Item(Entry::new("&Touch", Act::Rows(vec![radio("&Touch", 2)]))));
            out.push(Row::Item(Entry::new("&Specialty", Act::Rows(vec![radio("&Illustration", 3), radio("&Page Layout", 4), radio("&Adobe Illustrator", 5)]))));
            for c in &app.settings.custom_workspaces {
                if let Some(i) = crate::workspaces::index_of(&app.settings, &c.name) {
                    out.push(Row::Item(Entry::new(c.name.replace('&', "&&"), Act::Cmd(Cmd::Workspace(i))).mark(Mark::Radio(ws == c.name))));
                }
            }
            out.push(Row::Sep);
        }
        // Window ▸ Window List: the Welcome Screen, then the open documents
        "92fa5ba8" => {
            out.push(Row::Sep);
            if app.home_open {
                out.push(Row::Item(Entry::new("Welcome Screen", Act::Cmd(Cmd::WelcomeScreen)).mark(Mark::Radio(app.active.is_none()))));
            }
            for (i, d) in app.docs.iter().enumerate() {
                out.push(Row::Item(Entry::new(d.display_name().replace('&', "&&"), Act::Cmd(Cmd::WindowList(i))).mark(Mark::Radio(app.active == Some(i)))));
            }
        }
        // Window ▸ Color Palettes: the palettes shown on screen
        "60be24a7" => out.push(Row::Item(Entry::new("Default Palette", Act::Cmd(Cmd::ToggleDefaultPalette)).mark(Mark::Check(app.settings.default_palette_visible)))),
        "866d09b5" => out.push(Row::Item(Entry::new("Document Palette", Act::Cmd(Cmd::ToggleDocumentPalette)).mark(Mark::Check(app.settings.doc_palette_visible)))),
        "78f112b4" => out.push(Row::Item(Entry::new("Color Styles Palette", Act::Cmd(Cmd::ToggleColorStylesPalette)).mark(Mark::Check(app.settings.color_styles_palette_visible)))),
        // Window ▸ Color Palettes ▸ Recent
        "cad5a5c3" => {
            for n in &app.palettes.recent {
                out.push(Row::Item(Entry::new(n.replace('&', "&&"), Act::OpenPalette(n.clone()))));
            }
        }
        // Object ▸ Object Styles ▸ Apply Style, Edit Style, New Style From
        "44d9aafa" | "9273fe44" | "8b49f41d" => {
            let which = match key8(it.id).as_str() {
                "44d9aafa" => 0,
                "9273fe44" => 1,
                _ => 2,
            };
            for (text, i, on) in crate::dockers::ostyles::menu_styles(app, which) {
                let c = match which {
                    0 => Cmd::ApplyObjectStyle(i),
                    1 => Cmd::EditObjectStyle(i),
                    _ => Cmd::NewStyleFrom(i as u8),
                };
                out.push(Row::Item(Entry::new(text, Act::Cmd(c)).enabled(on)));
            }
        }
        // Bitmaps ▸ Plug-ins
        "be2d1143" => out.push(Row::Item(Entry::new(it.text, Act::None).enabled(false))),
        _ => {}
    }
}

fn icon_img(app: &mut AnyVectorApp, ui: &Ui, icon: Option<&str>, enabled: bool) -> Image<'static> {
    match icon.and_then(|i| app.icons.image(ui.ctx(), i, 16.0, enabled)) {
        Some(i) => i,
        None => app.icons.image(ui.ctx(), "blank", 16.0, true).expect("blank icon"),
    }
}

pub fn menu_style(ui: &mut Ui) {
    // 22 px rows, as in the reference
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.spacing_mut().button_padding = egui::vec2(6.0, 2.0);
    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = theme::HOVER();
    ui.style_mut().visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
}

/// A row of a docker's own menu (Options and the like), drawn as the reference menus: the
/// check mark in the icon column, the mnemonic underlined. Gives whether it was chosen.
pub fn item(app: &mut AnyVectorApp, ui: &mut Ui, text: &str, enabled: bool, check: bool) -> bool {
    let img = icon_img(app, ui, None, enabled);
    let color = if enabled { theme::TEXT() } else { theme::DISABLED() };
    let r = ui.add_enabled(enabled, Button::image_and_text(img, mnemonic(text, color, 12.5)).min_size(egui::vec2(200.0, 22.0)));
    if check {
        let c = r.rect.left_center() + egui::vec2(ui.spacing().button_padding.x + 8.0, 0.0);
        ui.painter().text(c, egui::Align2::CENTER_CENTER, '✓', egui::FontId::proportional(13.0), color);
    }
    r.clicked()
}

/// A submenu row of a docker's own menu.
pub fn submenu(app: &mut AnyVectorApp, ui: &mut Ui, text: &str, enabled: bool, f: impl FnOnce(&mut AnyVectorApp, &mut Ui)) {
    let img = icon_img(app, ui, None, enabled);
    let color = if enabled { theme::TEXT() } else { theme::DISABLED() };
    let b = Button::image_and_text(img, mnemonic(text, color, 12.5)).min_size(egui::vec2(200.0, 22.0));
    if !enabled {
        ui.add_enabled(false, b.right_text(SubMenuButton::arrow_atom(Some(color))));
        return;
    }
    SubMenuButton::from_button(b.right_text(SubMenuButton::arrow_atom(None))).ui(ui, |ui| {
        menu_style(ui);
        f(app, ui);
    });
}

/// A separator of a docker's own menu.
pub fn separator(ui: &mut Ui) {
    sep(ui);
}

fn sep(ui: &mut Ui) {
    ui.add_space(3.0);
    let r = ui.available_rect_before_wrap();
    let y = r.top() + 0.5;
    ui.painter().line_segment([egui::pos2(r.left() + 28.0, y), egui::pos2(r.right(), y)], egui::Stroke::new(1.0, theme::SEP()));
    ui.add_space(4.0);
}

fn draw_rows(app: &mut AnyVectorApp, ui: &mut Ui, rows: Vec<Row>) {
    for r in rows {
        match r {
            Row::Sep => sep(ui),
            Row::Item(e) => draw_entry(app, ui, e),
        }
    }
}

fn draw_entry(app: &mut AnyVectorApp, ui: &mut Ui, e: Entry) {
    let img = icon_img(app, ui, e.icon, e.enabled);
    let color = if e.enabled { theme::TEXT() } else { theme::DISABLED() };
    let mut b = Button::image_and_text(img, mnemonic(&e.text, color, 12.5)).min_size(egui::vec2(200.0, 22.0));
    match e.act {
        Act::Sub(_) | Act::Rows(_) => {
            if !e.enabled {
                ui.add_enabled(false, b.right_text(SubMenuButton::arrow_atom(Some(color))));
                return;
            }
            let act = e.act.clone();
            SubMenuButton::from_button(b.right_text(SubMenuButton::arrow_atom(None))).ui(ui, |ui| {
                menu_style(ui);
                let rows = match act {
                    Act::Sub(bar) => rows(app, bar, ui::context_bit(app), false),
                    Act::Rows(r) => r,
                    _ => vec![],
                };
                draw_rows(app, ui, rows);
            });
        }
        act => {
            if let Some(k) = &e.key {
                b = b.shortcut_text(egui::RichText::new(k).color(color));
            }
            let r = ui.add_enabled(e.enabled, b);
            let mark = match e.mark {
                Mark::Check(true) => Some(('✓', 13.0)),
                Mark::Radio(true) => Some(('●', 11.0)),
                _ => None,
            };
            if let Some((m, size)) = mark {
                let c = r.rect.left_center() + egui::vec2(ui.spacing().button_padding.x + 8.0, 0.0);
                ui.painter().text(c, egui::Align2::CENTER_CENTER, m, egui::FontId::proportional(size), color);
            }
            if r.clicked() {
                match act {
                    Act::Cmd(c) => app.run(c),
                    Act::Pending(what) => app.flash(format!("{what} is not available in this version yet.")),
                    Act::OpenPalette(n) => app.palettes.open_by_name(&n),
                    _ => {}
                }
            }
        }
    }
}

/// Which command bar a right-click menu belongs to: its Customize submenu names that bar.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BarSpot {
    MenuBar,
    StatusBar,
    /// A toolbar, by the name the Customize submenu gives it ("Standard", "Toolbox").
    Toolbar(&'static str),
}

thread_local! {
    /// The command bar whose right-click menu is being drawn, and whether the click hit an item.
    static SPOT: std::cell::Cell<(BarSpot, bool)> = const { std::cell::Cell::new((BarSpot::MenuBar, false)) };
}

/// Object ▸ AI Generate lists the guided items when something is selected and the plain ones
/// otherwise.
fn selection_shows(app: &AnyVectorApp, it: &Item) -> bool {
    match key8(it.id).as_str() {
        "a0776aad" | "2daeddaf" | "7e547078" => app.has_selection(),
        "9c0ccd48" | "4bf3be7a" | "8ed410c6" => !app.has_selection(),
        _ => true,
    }
}

/// Customize ▸ shows the entry for the bar that was right-clicked (and its item, if one was).
fn spot_shows(it: &Item) -> bool {
    let (spot, item) = SPOT.get();
    match key8(it.id).as_str() {
        "107cf387" => item,
        "ef28d52b" => spot == BarSpot::MenuBar,
        "dde352c6" => spot == BarSpot::StatusBar,
        "d530a686" => matches!(spot, BarSpot::Toolbar(_)),
        _ => true,
    }
}

fn spot_caption(it: &Item) -> Option<String> {
    match (key8(it.id).as_str(), SPOT.get().0) {
        ("107cf387", _) => Some("Toolbar Item".into()),
        ("d530a686", BarSpot::Toolbar(name)) => Some(it.text.replace("%s", name)),
        _ => None,
    }
}

/// The right-click menu of a command bar (`resp` is the bar's panel), on the bar or on one of
/// its buttons.
pub fn command_bar_context(app: &mut AnyVectorApp, resp: &egui::Response, spot: BarSpot) {
    let key = resp.id.with("on_item");
    if resp.container_secondary_clicked() {
        let item = resp.ctx.interaction_snapshot(|s| s.clicked.is_some_and(|id| id != resp.id));
        resp.ctx.data_mut(|d| d.insert_temp(key, item));
    }
    resp.container_context_menu(|ui| {
        let item = ui.ctx().data(|d| d.get_temp::<bool>(key)).unwrap_or(false);
        SPOT.set((spot, item));
        context_menu(app, ui, "Command Bar Context Menu");
    });
}

/// Draw the items of a bar as menu rows (a submenu, or the body of a right-click menu).
pub fn show_bar(app: &mut AnyVectorApp, ui: &mut Ui, bar: u16) {
    menu_style(ui);
    let rows = rows(app, bar, ui::context_bit(app), false);
    draw_rows(app, ui, rows);
}

/// A toolbox flyout: the items of a bar as menu rows, without check marks, leaving out the items
/// `skip` names (hidden with Quick Customize).
pub fn show_flyout(app: &mut AnyVectorApp, ui: &mut Ui, bar: u16, skip: impl Fn(&Item) -> bool) {
    menu_style(ui);
    let ctx = ui::context_bit(app);
    let mut out = vec![];
    if let Some(b) = ui::bar(bar) {
        for it in b.items {
            match it.kind {
                Kind::Separator => out.push(Row::Sep),
                Kind::Placeholder => placeholder(app, it, ctx, false, &mut out),
                Kind::GroupBegin | Kind::GroupEnd | Kind::Spacer | Kind::Anchors => {}
                _ if skip(it) => {}
                _ => {
                    let mut e = entry(app, it, ctx, false);
                    e.mark = Mark::None;
                    out.push(Row::Item(e));
                }
            }
        }
    }
    tidy(&mut out);
    draw_rows(app, ui, out);
}

/// A right-click menu: the reference bar with this name, in the current selection state.
pub fn context_menu(app: &mut AnyVectorApp, ui: &mut Ui, name: &str) {
    ui.set_min_width(220.0);
    if let Some((i, _)) = ui::bar_named(name) {
        show_bar(app, ui, i);
    }
}

/// The right-click menu of the drawing window for the current tool and selection, as the
/// reference shows it. `None` where a right-click does something else (zooming out with the
/// Zoom tool, finishing a curve, picking a color).
pub fn canvas_menu(app: &AnyVectorApp) -> Option<&'static str> {
    use crate::tools::ToolId::*;
    use vector_core::ObjKind;
    let d = app.doc()?;
    match app.tool {
        Zoom | ColorEyedropper => return None,
        Bezier | Pen | Polyline | BSpline if app.ts.is_busy() => return None,
        _ => {}
    }
    let objs: Vec<&vector_core::Object> = d.doc.find_all(&d.selection);
    if let Some(te) = &d.text_edit {
        let para = d.doc.find(te.obj).is_some_and(|o| matches!(&o.kind, ObjKind::Text(t) if matches!(t.kind, vector_core::text::TextKind::Paragraph { .. })));
        return Some(if para { "Paragraph Text Editing" } else { "Artistic Text Editing" });
    }
    if app.tool == crate::tools::ToolId::Shape && objs.len() == 1 && matches!(objs[0].kind, ObjKind::Path(_)) {
        return Some("Node Editing of Curves");
    }
    if app.tool == crate::tools::ToolId::Table && app.ts.table_cell.is_some() && objs.len() == 1 && matches!(objs[0].kind, ObjKind::Table(_)) {
        return Some("Table Edit");
    }
    Some(match objs.as_slice() {
        [] => "Document Desktop",
        [o] if o.locked => "Locked Object Selection",
        [o] => match &o.kind {
            ObjKind::Text(t) => match t.kind {
                vector_core::text::TextKind::Artistic => "Artistic Text Selected",
                vector_core::text::TextKind::Paragraph { .. } => "Paragraph Text Selected",
            },
            ObjKind::Image(_) => "Bitmap Object Selection",
            ObjKind::Group(_) | ObjKind::Table(_) => "Group Object Selection",
            ObjKind::Symbol(_) => "Symbol Instance Context Menu",
            ObjKind::Blend(_) => "Effect Selection",
            _ => "Graphic Object Selection",
        },
        _ => "Multiple Object Selection",
    })
}

/// Height of the menu bar panel.
pub const MENU_BAR_HEIGHT: f32 = 26.0;
/// How far the captions sit above the center of the bar.
const MENU_BAR_LIFT: f32 = 3.0;

pub fn menu_bar(app: &mut AnyVectorApp, ui: &mut Ui) {
    MenuBar::new().ui(ui, |ui| {
        // The buttons fill the bar height. The caption is painted by hand, a little above the
        // geometric center: lowercase letters and the access-key underline sit low in the line,
        // so a centered line box looks low in the bar.
        ui.spacing_mut().button_padding = egui::vec2(8.0, 0.0);
        ui.spacing_mut().interact_size.y = MENU_BAR_HEIGHT;
        ui.set_min_height(MENU_BAR_HEIGHT);
        let Some(bar) = ui::bar(ui::menu_bar_index(app.has_doc())) else { return };
        for it in bar.items {
            if it.sub == NO_SUB {
                continue;
            }
            let galley = ui.painter().layout_job(mnemonic(it.text, theme::TEXT(), 12.5));
            let size = galley.size();
            let atom = egui::IdSalt::new(("menubar_caption", it.sub));
            let (resp, _) = MenuButton::from_button(egui::Button::new(egui::Atom::custom(atom, size))).ui(ui, |ui| {
                ui.set_min_width(220.0);
                show_bar(app, ui, it.sub);
            });
            // The caption is painted, so give the button its name for accessibility (and tests).
            let name = galley.text().to_owned();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
            let pos = (resp.rect.center() - size / 2.0 - egui::vec2(0.0, MENU_BAR_LIFT)).round();
            ui.painter().galley(pos, galley, theme::TEXT());
        }
    });
}

/// One entry of the command catalog (Options ▸ Customization ▸ Commands).
#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub icon: Option<String>,
    pub label: String,
    pub cmd: Cmd,
}

/// The menus of the menu bar, for the Commands page category list.
pub fn catalog_menus() -> Vec<(String, u16)> {
    ui::bar(ui::menu_bar_index(true)).map(|b| b.items.iter().filter(|i| i.sub != NO_SUB).map(|i| (ui::plain(i.text), i.sub)).collect()).unwrap_or_default()
}

/// The commands of one menu and its submenus, in menu order.
pub fn catalog(bar: u16) -> Vec<CatalogEntry> {
    fn walk(bar: u16, seen: &mut Vec<u16>, out: &mut Vec<CatalogEntry>) {
        if seen.contains(&bar) {
            return;
        }
        seen.push(bar);
        let Some(b) = ui::bar(bar) else { return };
        for it in b.items {
            if it.sub != NO_SUB {
                walk(it.sub, seen, out);
            }
            if let Some(c) = bind::cmd(it.id) {
                if !it.text.is_empty() && !out.iter().any(|e| e.cmd == c) {
                    out.push(CatalogEntry { icon: (!it.icon.is_empty()).then(|| it.icon.to_string()), label: it.text.to_string(), cmd: c });
                }
            }
        }
    }
    let mut out = vec![];
    walk(bar, &mut vec![], &mut out);
    out
}
