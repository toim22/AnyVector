//! Menu bar. "File mode" (default, as in the screenshots): File · Tools · Window · Help (Purchase removed by request).
//! "Normal mode" adds Edit · View · Layout · Object · Effects · Bitmaps · Text · Table.

use crate::app::{Toolbar, VectorApp};
use crate::commands::{default_shortcut, Cmd, OptionsPage};
use crate::dockers::DockerId;
use crate::docview::ViewMode;
use crate::settings::{ButtonSize, MenuBarMode};
use crate::theme;
use crate::widgets::mnemonic;
use egui::menu::{MenuButton, SubMenuButton};
use egui::{Button, Image, MenuBar, Ui};
use vector_core::ops::{Align, Order, Shaping};

fn icon_img(app: &mut VectorApp, ui: &Ui, icon: Option<&str>, enabled: bool) -> Image<'static> {
    let name = icon.unwrap_or("blank");
    match app.icons.image(ui.ctx(), name, 16.0, enabled) {
        Some(i) => i,
        None => app.icons.image(ui.ctx(), "blank", 16.0, true).expect("blank icon"),
    }
}

fn shortcut_text(app: &VectorApp, cmd: Cmd) -> Option<String> {
    let name = format!("{cmd:?}");
    if let Some((_, s)) = app.settings.shortcuts.iter().find(|(c, _)| *c == name) {
        return Some(s.clone());
    }
    default_shortcut(cmd).map(|s| s.to_string())
}

/// One entry of the command catalog (Options > Customization > Commands).
#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub icon: Option<String>,
    pub label: String,
    pub cmd: Cmd,
}

thread_local! {
    static COLLECT: std::cell::RefCell<Option<Vec<CatalogEntry>>> = const { std::cell::RefCell::new(None) };
}

fn collecting() -> bool {
    COLLECT.with(|c| c.borrow().is_some())
}

fn collect(icon: Option<&str>, label: &str, cmd: Cmd) -> bool {
    COLLECT.with(|c| match c.borrow_mut().as_mut() {
        Some(v) => {
            if !v.iter().any(|e| e.cmd == cmd) {
                v.push(CatalogEntry { icon: icon.map(|s| s.to_string()), label: label.to_string(), cmd });
            }
            true
        }
        None => false,
    })
}

/// Menu names for the Commands page category combo, with their builder functions.
pub const CATALOG_MENUS: &[(&str, fn(&mut VectorApp, &mut Ui))] = &[
    ("File", file_menu),
    ("Edit", edit_menu),
    ("View", view_menu),
    ("Layout", layout_menu),
    ("Object", object_menu),
    ("Effects", effects_menu),
    ("Bitmaps", bitmaps_menu),
    ("Text", text_menu),
    ("Table", table_menu),
    ("Tools", tools_menu),
    ("Window", window_menu),
    ("Help", help_menu),
];

/// Collect the commands of one menu (recursing into submenus) without drawing anything.
pub fn catalog(app: &mut VectorApp, ui: &mut Ui, menu: fn(&mut VectorApp, &mut Ui)) -> Vec<CatalogEntry> {
    COLLECT.with(|c| *c.borrow_mut() = Some(vec![]));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(ui.min_rect().min, egui::vec2(1.0, 1.0))).invisible());
    menu(app, &mut child);
    COLLECT.with(|c| c.borrow_mut().take()).unwrap_or_default()
}

/// A command item: icon · label (with &mnemonic) · shortcut.
pub fn item(app: &mut VectorApp, ui: &mut Ui, icon: Option<&str>, label: &str, cmd: Cmd, enabled: bool) {
    if collect(icon, label, cmd) {
        return;
    }
    let img = icon_img(app, ui, icon, enabled);
    let color = if enabled { theme::TEXT } else { theme::DISABLED };
    let mut b = Button::image_and_text(img, mnemonic(label, color, 12.5)).min_size(egui::vec2(200.0, 22.0));
    if let Some(sc) = shortcut_text(app, cmd) {
        b = b.shortcut_text(egui::RichText::new(sc).color(color));
    }
    if ui.add_enabled(enabled, b).clicked() {
        app.run(cmd);
    }
}

/// A checkable item (✓ drawn in the icon column).
pub fn check(app: &mut VectorApp, ui: &mut Ui, label: &str, checked: bool, cmd: Cmd, enabled: bool) {
    mark_item(app, ui, label, if checked { Some('✓') } else { None }, cmd, enabled);
}

/// A radio item (● drawn in the icon column).
pub fn radio(app: &mut VectorApp, ui: &mut Ui, label: &str, selected: bool, cmd: Cmd, enabled: bool) {
    mark_item(app, ui, label, if selected { Some('●') } else { None }, cmd, enabled);
}

fn mark_item(app: &mut VectorApp, ui: &mut Ui, label: &str, mark: Option<char>, cmd: Cmd, enabled: bool) {
    if collect(None, label, cmd) {
        return;
    }
    let img = icon_img(app, ui, None, enabled);
    let color = if enabled { theme::TEXT } else { theme::DISABLED };
    let mut b = Button::image_and_text(img, mnemonic(label, color, 12.5)).min_size(egui::vec2(200.0, 22.0));
    if let Some(sc) = shortcut_text(app, cmd) {
        b = b.shortcut_text(egui::RichText::new(sc).color(color));
    }
    let r = ui.add_enabled(enabled, b);
    if let Some(m) = mark {
        let c = r.rect.left_center() + egui::vec2(ui.spacing().button_padding.x + 8.0, 0.0);
        let size = if m == '●' { 8.0 } else { 13.0 };
        ui.painter().text(c, egui::Align2::CENTER_CENTER, m, egui::FontId::proportional(size), color);
    }
    if r.clicked() {
        app.run(cmd);
    }
}

fn submenu(app: &mut VectorApp, ui: &mut Ui, icon: Option<&str>, label: &str, enabled: bool, content: impl FnOnce(&mut VectorApp, &mut Ui)) {
    if collecting() {
        content(app, ui);
        return;
    }
    let img = icon_img(app, ui, icon, enabled);
    let color = if enabled { theme::TEXT } else { theme::DISABLED };
    if !enabled {
        let b = Button::image_and_text(img, mnemonic(label, color, 12.5)).min_size(egui::vec2(200.0, 22.0)).right_text(SubMenuButton::arrow_atom(Some(color)));
        ui.add_enabled(false, b);
        return;
    }
    SubMenuButton::from_button(Button::image_and_text(img, mnemonic(label, color, 12.5)).min_size(egui::vec2(200.0, 22.0)).right_text(SubMenuButton::arrow_atom(None))).ui(ui, |ui| {
        menu_style(ui);
        content(app, ui)
    });
}

fn menu_style(ui: &mut Ui) {
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = theme::HOVER;
    ui.style_mut().visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
}

fn top(app: &mut VectorApp, ui: &mut Ui, label: &str, content: impl FnOnce(&mut VectorApp, &mut Ui)) {
    MenuButton::new(mnemonic(label, theme::TEXT, 12.5)).ui(ui, |ui| {
        menu_style(ui);
        ui.set_min_width(220.0);
        content(app, ui);
    });
}

fn sep(ui: &mut Ui) {
    if collecting() {
        return;
    }
    ui.add_space(2.0);
    let r = ui.available_rect_before_wrap();
    let y = r.top();
    ui.painter().line_segment([egui::pos2(r.left() + 28.0, y), egui::pos2(r.right(), y)], egui::Stroke::new(1.0, theme::SEP));
    ui.add_space(3.0);
}

pub fn menu_bar(app: &mut VectorApp, ui: &mut Ui) {
    MenuBar::new().ui(ui, |ui| {
        // Fill the bar height so the labels sit vertically centered with breathing room
        // above the standard toolbar.
        ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);
        ui.spacing_mut().interact_size.y = 24.0;
        ui.set_min_height(24.0);
        let normal = app.settings.menu_bar_mode == MenuBarMode::Normal;
        top(app, ui, "&File", file_menu);
        if normal {
            top(app, ui, "&Edit", edit_menu);
            top(app, ui, "&View", view_menu);
            top(app, ui, "&Layout", layout_menu);
            top(app, ui, "&Object", object_menu);
            top(app, ui, "Effe&cts", effects_menu);
            top(app, ui, "&Bitmaps", bitmaps_menu);
            top(app, ui, "Te&xt", text_menu);
            top(app, ui, "T&able", table_menu);
        }
        top(app, ui, "&Tools", tools_menu);
        top(app, ui, "&Window", window_menu);
        top(app, ui, "&Help", help_menu);
    });
}

pub fn file_menu(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    let modified = app.doc().map(|d| d.history.is_modified() && d.path.is_some()).unwrap_or(false);
    item(app, ui, Some("new"), "&New...", Cmd::New, true);
    item(app, ui, Some("new_template"), "New from &Template...", Cmd::NewFromTemplate, true);
    item(app, ui, Some("ai"), "Quick AI &Generate...", Cmd::QuickAiGenerate, true);
    item(app, ui, Some("open"), "&Open...", Cmd::Open, true);
    item(app, ui, Some("open_cloud"), "Open from Clou&d...", Cmd::OpenFromCloud, true);
    submenu(app, ui, None, "Open &Recent", true, |app, ui| {
        let recent = app.settings.recent_files.clone();
        if recent.is_empty() {
            ui.add_enabled(false, Button::new("(empty)"));
        }
        for (i, p) in recent.iter().enumerate() {
            let label = format!("&{} {}", (i + 1) % 10, p.display());
            item(app, ui, None, &label, Cmd::OpenRecent(i), true);
        }
        if !recent.is_empty() {
            sep(ui);
            item(app, ui, None, "&Clear List", Cmd::ClearRecent, true);
        }
    });
    sep(ui);
    item(app, ui, Some("close_doc"), "&Close", Cmd::Close, doc);
    item(app, ui, Some("close_all"), "Close A&ll", Cmd::CloseAll, doc);
    sep(ui);
    item(app, ui, Some("save"), "&Save...", Cmd::Save, doc);
    item(app, ui, Some("save_as"), "Save &As...", Cmd::SaveAs, doc);
    item(app, ui, Some("save_cloud"), "Save to Clo&ud...", Cmd::SaveToCloud, doc);
    item(app, ui, Some("save_template"), "Save as Te&mplate...", Cmd::SaveAsTemplate, doc);
    item(app, ui, Some("revert"), "Re&vert", Cmd::Revert, modified);
    sep(ui);
    submenu(app, ui, None, "Ac&quire Image", true, |app, ui| {
        item(app, ui, None, "Select &TWAIN Source...", Cmd::SelectTwain, true);
        item(app, ui, None, "Select &WIA Source...", Cmd::SelectWia, true);
        sep(ui);
        item(app, ui, Some("acquire"), "&Acquire...", Cmd::Acquire, true);
    });
    sep(ui);
    item(app, ui, Some("import"), "&Import...", Cmd::Import, doc);
    item(app, ui, Some("export"), "&Export...", Cmd::Export, doc);
    submenu(app, ui, None, "Export &For", doc, |app, ui| {
        item(app, ui, None, "&Web...", Cmd::ExportForWeb, true);
        item(app, ui, None, "&Office...", Cmd::ExportForOffice, true);
        item(app, ui, None, "&Inkjet Cutting...", Cmd::ExportForInkjet, true);
    });
    submenu(app, ui, None, "Export Doc&ker", doc, |app, ui| {
        item(app, ui, Some("export"), "&Export", Cmd::ExportDockerOpen, true);
    });
    submenu(app, ui, None, "Sen&d To", doc, |app, ui| {
        item(app, ui, None, "&Mail Recipient", Cmd::SendToMail, true);
        item(app, ui, None, "&Desktop (create shortcut)", Cmd::SendToDesktop, true);
        item(app, ui, None, "D&ocuments", Cmd::SendToDocuments, true);
    });
    item(app, ui, Some("pdf"), "Pu&blish to PDF", Cmd::PublishPdf, doc);
    sep(ui);
    submenu(app, ui, None, "Print &Merge", doc, |app, ui| {
        item(app, ui, None, "&Create/Load Merge Fields...", Cmd::PrintMergeCreate, true);
        item(app, ui, None, "&Edit Merge Fields...", Cmd::PrintMergeEdit, true);
        item(app, ui, None, "&Perform Merge...", Cmd::PrintMergePerform, true);
    });
    sep(ui);
    item(app, ui, Some("print"), "&Print...", Cmd::Print, doc);
    item(app, ui, Some("print_preview"), "Print Pre&view...", Cmd::PrintPreview, doc);
    sep(ui);
    item(app, ui, Some("doc_props"), "Document Proper&ties...", Cmd::DocumentProperties, doc);
    sep(ui);
    item(app, ui, Some("exit"), "E&xit", Cmd::Exit, true);
}

pub fn tools_menu(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    submenu(app, ui, Some("options"), "&Options", true, |app, ui| options_items(app, ui));
    item(app, ui, Some("save_default"), "Save Settings as &Default...", Cmd::SaveSettingsAsDefault, doc);
    sep(ui);
    item(app, ui, Some("color_mgmt"), "Color &Management...", Cmd::ColorManagement, true);
    sep(ui);
    submenu(app, ui, Some("script"), "&Scripts", true, |app, ui| {
        let rec = app.recording.is_some();
        let paused = app.recording.as_ref().map(|r| r.paused).unwrap_or(false);
        item(app, ui, Some("script"), "Script &Editor...", Cmd::ScriptEditor, true);
        item(app, ui, Some("run"), "&Run Script...", Cmd::RunScript, true);
        sep(ui);
        item(app, ui, None, "&Visual Studio Editor...", Cmd::VsEditor, true);
        sep(ui);
        item(app, ui, Some("record"), "&Start Recording", Cmd::StartRecording, doc && !rec);
        item(app, ui, Some("pause"), if paused { "Resume Recording" } else { "&Pause Recording" }, Cmd::PauseRecording, rec);
        item(app, ui, Some("stop"), "S&top Recording", Cmd::StopRecording, rec);
        item(app, ui, Some("cancel"), "&Cancel Recording", Cmd::CancelRecording, rec);
        sep(ui);
        item(app, ui, None, "Record Temporary Script", Cmd::RecordTemporary, doc);
        item(app, ui, None, "Run Temporary Script", Cmd::RunTemporary, doc && !app.temp_script.is_empty());
        sep(ui);
        item(app, ui, Some("script"), "Scripts", Cmd::Docker(DockerId::Scripts), true);
    });
}

pub fn options_items(app: &mut VectorApp, ui: &mut Ui) {
    item(app, ui, None, "&Vector...", Cmd::Options(OptionsPage::App), true);
    item(app, ui, None, "&Customization...", Cmd::Options(OptionsPage::Customization), true);
    item(app, ui, None, "&Tools...", Cmd::Options(OptionsPage::Tools), true);
    item(app, ui, None, "&Global...", Cmd::Options(OptionsPage::Global), true);
    item(app, ui, None, "&Workspaces...", Cmd::Options(OptionsPage::Workspaces), true);
}

pub fn window_menu(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    item(app, ui, Some("window_new"), "&New Window", Cmd::NewWindow, doc);
    item(app, ui, Some("refresh"), "&Refresh Window", Cmd::RefreshWindow, doc);
    item(app, ui, Some("window_close"), "&Close Window", Cmd::CloseWindow, doc);
    item(app, ui, None, "Close A&ll", Cmd::CloseAllWindows, doc);
    sep(ui);
    item(app, ui, Some("cascade"), "C&ascade", Cmd::Cascade, doc);
    item(app, ui, Some("tile_h"), "Tile &Horizontally", Cmd::TileH, doc);
    item(app, ui, Some("tile_v"), "Tile &Vertically", Cmd::TileV, doc);
    item(app, ui, Some("combine"), "Com&bine Windows", Cmd::CombineWindows, doc);
    item(app, ui, Some("dock"), "&Dock Window", Cmd::DockWindow, doc);
    sep(ui);
    submenu(app, ui, Some("workspace"), "&Workspace", true, |app, ui| {
        let ws = app.settings.workspace.clone();
        radio(app, ui, "&Lite", ws == "Lite", Cmd::Workspace(0), true);
        radio(app, ui, "&Default", ws == "Default", Cmd::Workspace(1), true);
        submenu(app, ui, None, "&Touch", true, |app, ui| {
            let ws = app.settings.workspace.clone();
            radio(app, ui, "&Touch", ws == "Touch", Cmd::Workspace(2), true);
        });
        submenu(app, ui, None, "&Specialty", true, |app, ui| {
            let ws = app.settings.workspace.clone();
            radio(app, ui, "&Illustration", ws == "Illustration", Cmd::Workspace(3), true);
            radio(app, ui, "&Page Layout", ws == "Page Layout", Cmd::Workspace(4), true);
            radio(app, ui, "&Adobe Illustrator", ws == "Adobe Illustrator", Cmd::Workspace(5), true);
        });
        sep(ui);
        item(app, ui, None, "&New...", Cmd::WorkspaceNew, true);
        item(app, ui, None, "&Delete", Cmd::WorkspaceDelete, true);
        item(app, ui, None, "&Import...", Cmd::WorkspaceImport, true);
        item(app, ui, None, "&Export...", Cmd::WorkspaceExport, true);
    });
    sep(ui);
    submenu(app, ui, Some("dockers"), "D&ockers", true, dockers_menu);
    submenu(app, ui, Some("toolbars"), "&Toolbars", true, toolbars_menu);
    submenu(app, ui, Some("palettes"), "Color &Palettes", true, palettes_menu);
    sep(ui);
    submenu(app, ui, Some("window_list"), "Window &List", doc, |app, ui| {
        let names: Vec<String> = app.docs.iter().map(|d| d.display_name()).collect();
        let active = app.active;
        for (i, n) in names.iter().enumerate() {
            check(app, ui, &format!("&{} {}", i + 1, n), active == Some(i), Cmd::WindowList(i), true);
        }
    });
}

fn docker_item(app: &mut VectorApp, ui: &mut Ui, d: DockerId) {
    let open = app.dockers.open.contains(&d);
    let img = icon_img(app, ui, Some(crate::dockers::icon(d)), true);
    let label = crate::dockers::menu_label(d);
    let mut b = Button::image_and_text(img, mnemonic(label, theme::TEXT, 12.5)).min_size(egui::vec2(220.0, 22.0)).selected(open);
    if let Some(sc) = crate::dockers::shortcut(d) {
        b = b.shortcut_text(sc);
    }
    if ui.add(b).clicked() {
        app.run(Cmd::Docker(d));
    }
}

pub fn dockers_menu(app: &mut VectorApp, ui: &mut Ui) {
    use DockerId::*;
    for d in [Properties, Objects, Pages, Comments, ObjectData, ObjectStyles, Symbols, FindReplace, Export, AiGenerate] {
        docker_item(app, ui, d);
    }
    sep(ui);
    for d in [Transform, Coordinates] {
        docker_item(app, ui, d);
    }
    sep(ui);
    for d in [LiveGuides, Guidelines] {
        docker_item(app, ui, d);
    }
    sep(ui);
    for d in [AlignDistribute, FitObjectsToPath, StepRepeat] {
        docker_item(app, ui, d);
    }
    sep(ui);
    for d in [Shape, Corners, JoinCurves] {
        docker_item(app, ui, d);
    }
    sep(ui);
    for d in [Text, FontSampler, Glyphs] {
        docker_item(app, ui, d);
    }
    sep(ui);
    submenu(app, ui, Some("effects"), "&Effects", true, |app, ui| {
        for d in [Contour, Blend, Extrude, Bevel, Lens, Envelope, Distort, Shadow, BitmapEffects] {
            docker_item(app, ui, d);
        }
    });
    sep(ui);
    for d in [Color, Palettes, ColorStyles, ColorProofing] {
        docker_item(app, ui, d);
    }
    sep(ui);
    for d in [Assets, Tray] {
        docker_item(app, ui, d);
    }
    sep(ui);
    for d in [LinksRollovers, Sources] {
        docker_item(app, ui, d);
    }
    sep(ui);
    for d in [Views, History, Scripts] {
        docker_item(app, ui, d);
    }
    sep(ui);
    docker_item(app, ui, Learn);
}

pub fn toolbars_menu(app: &mut VectorApp, ui: &mut Ui) {
    submenu(app, ui, None, "Default &Button Size", true, |app, ui| {
        let bs = app.settings.button_size;
        radio(app, ui, "&Small", bs == ButtonSize::Small, Cmd::ButtonSize(ButtonSize::Small), true);
        radio(app, ui, "&Medium", bs == ButtonSize::Medium, Cmd::ButtonSize(ButtonSize::Medium), true);
        radio(app, ui, "&Large", bs == ButtonSize::Large, Cmd::ButtonSize(ButtonSize::Large), true);
    });
    sep(ui);
    let s = &app.settings;
    let states = [
        (Toolbar::MenuBar, s.show_menu_bar),
        (Toolbar::StatusBar, s.show_status_bar),
        (Toolbar::Standard, s.show_standard),
        (Toolbar::PropertyBar, s.show_property_bar),
        (Toolbar::Toolbox, s.show_toolbox),
    ];
    for (tb, on) in states {
        check(app, ui, tb.name(), on, Cmd::ToggleToolbar(tb), true);
    }
    for tb in Toolbar::EXTRA {
        let on = app.settings.extra_toolbars.iter().any(|x| x == tb.name());
        check(app, ui, tb.name(), on, Cmd::ToggleToolbar(tb), true);
    }
    sep(ui);
    let locked = app.settings.lock_toolbars;
    check(app, ui, "&Lock Toolbars", locked, Cmd::LockToolbars, true);
}

pub fn palettes_menu(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    let sel = app.has_selection();
    item(app, ui, None, "Add Colors from &Selection...", Cmd::AddColorsFromSelection, sel);
    item(app, ui, None, "Add Colors from &Document...", Cmd::AddColorsFromDocument, doc);
    sep(ui);
    item(app, ui, None, "Create Palette from S&election...", Cmd::CreatePaletteFromSelection, sel);
    item(app, ui, None, "Create Palette from Doc&ument...", Cmd::CreatePaletteFromDocument, doc);
    item(app, ui, Some("open"), "&Open Palette...", Cmd::OpenPalette, true);
    item(app, ui, None, "&Close All Palettes", Cmd::CloseAllPalettes, true);
    sep(ui);
    item(app, ui, None, "Palette &Editor...", Cmd::PaletteEditor, true);
    item(app, ui, Some("palettes"), "&Palettes", Cmd::Docker(DockerId::Palettes), true);
    sep(ui);
    submenu(app, ui, None, "&Recent", true, |app, ui| {
        let names: Vec<String> = app.palettes.recent.clone();
        if names.is_empty() {
            ui.add_enabled(false, Button::new("(empty)"));
        }
        for n in names {
            if ui.button(&n).clicked() {
                app.palettes.open_by_name(&n);
            }
        }
    });
    sep(ui);
    let (a, b, c) = (app.settings.default_palette_visible, app.settings.doc_palette_visible, app.settings.color_styles_palette_visible);
    check(app, ui, "Default &Palette", a, Cmd::ToggleDefaultPalette, true);
    check(app, ui, "&Document Palette", b, Cmd::ToggleDocumentPalette, true);
    check(app, ui, "Color &Styles Palette", c, Cmd::ToggleColorStylesPalette, true);
}

pub fn help_menu(app: &mut VectorApp, ui: &mut Ui) {
    item(app, ui, Some("help"), "&Product Help...", Cmd::ProductHelp, true);
    sep(ui);
    item(app, ui, Some("welcome"), "&Welcome Screen", Cmd::WelcomeScreen, true);
    item(app, ui, Some("video"), "&Video Tutorials...", Cmd::VideoTutorials, true);
    item(app, ui, Some("learn"), "&Learn", Cmd::Docker(DockerId::Learn), true);
    item(app, ui, Some("book"), "&Quick Start Guide...", Cmd::QuickStart, true);
    item(app, ui, Some("book"), "&User Guide...", Cmd::UserGuide, true);
    sep(ui);
    item(app, ui, Some("whats_new"), "What's &New?...", Cmd::WhatsNew, true);
    sep(ui);
    item(app, ui, Some("updates"), "&Updates...", Cmd::Updates, true);
    item(app, ui, Some("message"), "&Message Settings", Cmd::MessageSettings, true);
    item(app, ui, Some("community"), "Vector &Community", Cmd::Community, true);
    item(app, ui, Some("support"), "Vector &Support...", Cmd::Support, true);
    item(app, ui, Some("feedback"), "Submit &Ideas and Feedback...", Cmd::Feedback, true);
    submenu(app, ui, Some("bug"), "Report a &Bug", true, |app, ui| {
        item(app, ui, Some("bug"), "&Report a Bug...", Cmd::ReportBug, true);
        item(app, ui, Some("community"), "&View Reported Bugs...", Cmd::ViewReportedBugs, true);
    });
    sep(ui);
    item(app, ui, Some("refresh"), "&Refresh your Account", Cmd::RefreshAccount, true);
    item(app, ui, Some("account"), "&Account Settings...", Cmd::AccountSettings, true);
    sep(ui);
    item(app, ui, Some("about"), "&About Vector...", Cmd::About, true);
}

// ---------------------------------------------------------------------------------------------
// Normal-mode menus

pub fn edit_menu(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    let sel = app.has_selection();
    let (can_undo, can_redo, undo_l, redo_l) = match app.doc() {
        Some(d) => (d.history.can_undo(), d.history.can_redo(), d.history.undo_labels().first().cloned().unwrap_or_default(), d.history.redo_labels().first().cloned().unwrap_or_default()),
        None => (false, false, String::new(), String::new()),
    };
    item(app, ui, Some("undo"), &format!("&Undo {undo_l}"), Cmd::Undo, can_undo);
    item(app, ui, Some("redo"), &format!("&Redo {redo_l}"), Cmd::Redo, can_redo);
    item(app, ui, None, "R&epeat", Cmd::Repeat, doc && app.last_cmd.is_some());
    sep(ui);
    item(app, ui, Some("cut"), "Cu&t", Cmd::Cut, sel);
    item(app, ui, Some("copy"), "&Copy", Cmd::Copy, sel);
    item(app, ui, Some("paste"), "&Paste", Cmd::Paste, doc);
    item(app, ui, None, "Paste &Special...", Cmd::PasteSpecial, doc);
    item(app, ui, Some("delete"), "&Delete", Cmd::Delete, sel);
    sep(ui);
    item(app, ui, None, "D&uplicate", Cmd::Duplicate, sel);
    item(app, ui, None, "Clo&ne", Cmd::Clone, sel);
    item(app, ui, Some("step_repeat"), "Step and Repe&at...", Cmd::StepAndRepeat, sel);
    sep(ui);
    item(app, ui, None, "C&opy Properties From...", Cmd::CopyPropertiesFrom, sel);
    sep(ui);
    submenu(app, ui, None, "&Select All", doc, |app, ui| {
        item(app, ui, None, "&Objects", Cmd::SelectAll, true);
        item(app, ui, None, "&Text", Cmd::SelectAllText, true);
        item(app, ui, None, "&Guidelines", Cmd::SelectAllGuides, true);
        item(app, ui, None, "&Nodes", Cmd::SelectAllNodes, true);
    });
    item(app, ui, Some("find"), "&Find and Replace...", Cmd::FindReplace, doc);
}

pub fn view_menu(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    let vm = app.doc().map(|d| d.view_mode);
    for m in [ViewMode::Wireframe, ViewMode::Normal, ViewMode::Enhanced, ViewMode::Pixels, ViewMode::SimulateOverprint] {
        radio(app, ui, m.name(), vm == Some(m), Cmd::ViewMode(m), doc);
    }
    sep(ui);
    item(app, ui, Some("fullscreen"), "F&ull-Screen Preview", Cmd::FullScreenPreview, doc);
    item(app, ui, Some("pages"), "Page &Sorter View", Cmd::PageSorterView, doc);
    sep(ui);
    let s = app.doc().map(|d| d.doc.settings.clone());
    let get = |f: fn(&vector_core::document::DocSettings) -> bool| s.as_ref().map(f).unwrap_or(false);
    check(app, ui, "&Rulers", get(|s| s.show_rulers), Cmd::ToggleRulers, doc);
    check(app, ui, "&Grid", get(|s| s.show_grid), Cmd::ToggleGrid, doc);
    check(app, ui, "Pi&xel Grid", get(|s| s.pixel_grid), Cmd::TogglePixelGrid, doc);
    check(app, ui, "Gu&idelines", get(|s| s.show_guides), Cmd::ToggleGuidelines, doc);
    check(app, ui, "Page &Border", get(|s| s.show_page_border), Cmd::TogglePageBorder, doc);
    check(app, ui, "&Bleed", get(|s| s.show_bleed), Cmd::ToggleBleed, doc);
    sep(ui);
    check(app, ui, "Snap &Off", get(|s| s.snap_off), Cmd::SnapOff, doc);
    check(app, ui, "Snap to Gr&id", get(|s| s.snap_to_grid), Cmd::SnapToGrid, doc);
    check(app, ui, "Snap to Guidelin&es", get(|s| s.snap_to_guides), Cmd::SnapToGuides, doc);
    check(app, ui, "Snap to Ob&jects", get(|s| s.snap_to_objects), Cmd::SnapToObjects, doc);
    check(app, ui, "Snap to &Page", get(|s| s.snap_to_page), Cmd::SnapToPage, doc);
    sep(ui);
    item(app, ui, Some("zoom_in"), "Zoom &In", Cmd::ZoomIn, doc);
    item(app, ui, Some("zoom_out"), "Zoom &Out", Cmd::ZoomOut, doc);
    item(app, ui, Some("zoom_selected"), "Zoom to &Selected", Cmd::ZoomToSelected, doc);
    item(app, ui, Some("zoom_all"), "Zoom to &All Objects", Cmd::ZoomToAll, doc);
    item(app, ui, Some("zoom_page"), "Zoom to &Page", Cmd::ZoomToPage, doc);
    item(app, ui, Some("zoom_width"), "Zoom to Page &Width", Cmd::ZoomToWidth, doc);
    item(app, ui, Some("zoom_height"), "Zoom to Page &Height", Cmd::ZoomToHeight, doc);
    item(app, ui, None, "Zoom &100%", Cmd::ZoomActual, doc);
}

pub fn layout_menu(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    let multi = app.doc().map(|d| d.doc.pages.len() > 1).unwrap_or(false);
    item(app, ui, Some("page_new"), "&Insert Page...", Cmd::InsertPage, doc);
    item(app, ui, None, "D&uplicate Page...", Cmd::DuplicatePage, doc);
    item(app, ui, None, "R&ename Page...", Cmd::RenamePage, doc);
    item(app, ui, Some("delete"), "&Delete Page...", Cmd::DeletePage, multi);
    sep(ui);
    item(app, ui, None, "&Go To Page...", Cmd::GoToPage, multi);
    item(app, ui, Some("landscape"), "S&witch Page Orientation", Cmd::SwitchOrientation, doc);
    sep(ui);
    item(app, ui, None, "Page &Layout...", Cmd::PageSetup, doc);
    item(app, ui, None, "Page &Background...", Cmd::PageBackground, doc);
}

pub fn object_menu(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    let sel = app.has_selection();
    let multi = app.doc().map(|d| d.selection.len() > 1).unwrap_or(false);
    submenu(app, ui, Some("symbols"), "&Symbol", doc, |app, ui| {
        let sel = app.has_selection();
        item(app, ui, None, "&New Symbol", Cmd::NewSymbol, sel);
        item(app, ui, None, "&Revert to Objects", Cmd::RevertSymbol, sel);
        item(app, ui, Some("symbols"), "&Symbols", Cmd::Docker(DockerId::Symbols), true);
    });
    submenu(app, ui, Some("transform"), "&Transform", sel, |app, ui| {
        item(app, ui, Some("transform"), "&Transform", Cmd::Docker(DockerId::Transform), true);
        item(app, ui, None, "&Clear Transformations", Cmd::ClearTransformations, true);
        item(app, ui, Some("rotate"), "Rotate 90° &Clockwise", Cmd::Rotate(-90.0), true);
        item(app, ui, Some("rotate"), "Rotate 90° Co&unterclockwise", Cmd::Rotate(90.0), true);
        item(app, ui, Some("mirror_h"), "Mirror &Horizontally", Cmd::MirrorH, true);
        item(app, ui, Some("mirror_v"), "Mirror &Vertically", Cmd::MirrorV, true);
    });
    submenu(app, ui, Some("align"), "&Align and Distribute", sel, |app, ui| {
        item(app, ui, Some("align_left"), "Align &Left", Cmd::Align(Align::Left), true);
        item(app, ui, Some("align_right"), "Align &Right", Cmd::Align(Align::Right), true);
        item(app, ui, Some("align_top"), "Align &Top", Cmd::Align(Align::Top), true);
        item(app, ui, Some("align_bottom"), "Align &Bottom", Cmd::Align(Align::Bottom), true);
        item(app, ui, Some("align_hcenter"), "Align &Centers Horizontally", Cmd::Align(Align::CenterH), true);
        item(app, ui, Some("align_vcenter"), "Align Centers &Vertically", Cmd::Align(Align::CenterV), true);
        sep(ui);
        item(app, ui, None, "Center to &Page", Cmd::AlignTo(Align::CenterH), true);
        sep(ui);
        item(app, ui, Some("align"), "Align and &Distribute...", Cmd::Docker(DockerId::AlignDistribute), true);
    });
    submenu(app, ui, Some("to_front"), "&Order", sel, |app, ui| {
        item(app, ui, Some("to_front"), "To &Front of Page", Cmd::Order(Order::ToFront), true);
        item(app, ui, Some("to_back"), "To &Back of Page", Cmd::Order(Order::ToBack), true);
        item(app, ui, Some("forward_one"), "Forward &One", Cmd::Order(Order::ForwardOne), true);
        item(app, ui, Some("back_one"), "Back O&ne", Cmd::Order(Order::BackOne), true);
        item(app, ui, None, "&Reverse Order", Cmd::ReverseOrder, true);
    });
    sep(ui);
    item(app, ui, Some("group"), "&Group Objects", Cmd::Group, multi);
    item(app, ui, Some("ungroup"), "&Ungroup Objects", Cmd::Ungroup, sel);
    item(app, ui, None, "Ungroup A&ll", Cmd::UngroupAll, sel);
    sep(ui);
    item(app, ui, Some("combine_obj"), "&Combine", Cmd::Combine, multi);
    item(app, ui, Some("break_apart"), "&Break Curve Apart", Cmd::BreakApart, sel);
    sep(ui);
    item(app, ui, Some("lock"), "&Lock Object", Cmd::Lock, sel);
    item(app, ui, Some("unlock"), "Unloc&k Object", Cmd::Unlock, sel);
    item(app, ui, None, "Unlock All Objects", Cmd::UnlockAll, doc);
    item(app, ui, Some("eye_off"), "&Hide Object", Cmd::HideObject, sel);
    item(app, ui, Some("eye"), "Show All Objects", Cmd::ShowAllObjects, doc);
    sep(ui);
    submenu(app, ui, Some("weld"), "S&haping", sel, |app, ui| {
        let multi = app.doc().map(|d| d.selection.len() > 1).unwrap_or(false);
        item(app, ui, Some("weld"), "&Weld", Cmd::Shaping(Shaping::Weld), multi);
        item(app, ui, Some("trim"), "&Trim", Cmd::Shaping(Shaping::Trim), multi);
        item(app, ui, Some("intersect"), "&Intersect", Cmd::Shaping(Shaping::Intersect), multi);
        item(app, ui, Some("simplify"), "&Simplify", Cmd::Shaping(Shaping::Simplify), multi);
        item(app, ui, Some("front_minus_back"), "Front Minus &Back", Cmd::Shaping(Shaping::FrontMinusBack), multi);
        item(app, ui, Some("back_minus_front"), "Back Minus &Front", Cmd::Shaping(Shaping::BackMinusFront), multi);
        item(app, ui, Some("boundary"), "B&oundary", Cmd::Shaping(Shaping::Boundary), true);
    });
    submenu(app, ui, Some("powerclip"), "&PowerClip", doc, |app, ui| {
        let sel = app.has_selection();
        item(app, ui, None, "&Place inside Frame...", Cmd::PowerClipInside, sel);
        item(app, ui, None, "&Edit PowerClip", Cmd::PowerClipEdit, sel);
        item(app, ui, None, "&Finish Editing PowerClip", Cmd::PowerClipFinish, true);
        item(app, ui, None, "E&xtract Contents", Cmd::PowerClipExtract, sel);
    });
    sep(ui);
    item(app, ui, Some("to_curves"), "Con&vert to Curves", Cmd::ConvertToCurves, sel);
    item(app, ui, None, "Convert Outline to Ob&ject", Cmd::OutlineToObject, sel);
    item(app, ui, Some("join_curves"), "&Join Curves", Cmd::JoinCurves, sel);
    sep(ui);
    item(app, ui, Some("properties"), "Object P&roperties", Cmd::ObjectProperties, true);
    item(app, ui, Some("objects"), "Ob&jects", Cmd::Docker(DockerId::Objects), true);
}

pub fn effects_menu(app: &mut VectorApp, ui: &mut Ui) {
    let sel = app.has_selection();
    item(app, ui, Some("contour"), "&Contour", Cmd::Docker(DockerId::Contour), true);
    item(app, ui, Some("blend"), "&Blend", Cmd::Docker(DockerId::Blend), true);
    item(app, ui, Some("extrude"), "E&xtrude", Cmd::Docker(DockerId::Extrude), true);
    item(app, ui, Some("bevel"), "Be&vel", Cmd::Docker(DockerId::Bevel), true);
    item(app, ui, Some("envelope"), "&Envelope", Cmd::Docker(DockerId::Envelope), true);
    item(app, ui, Some("lens"), "&Lens", Cmd::Docker(DockerId::Lens), true);
    item(app, ui, Some("drop_shadow"), "&Shadow", Cmd::Docker(DockerId::Shadow), true);
    item(app, ui, Some("perspective"), "Add &Perspective", Cmd::AddPerspective, sel);
    sep(ui);
    item(app, ui, None, "Clea&r Effect", Cmd::ClearEffect, sel);
    item(app, ui, None, "Copy E&ffect From...", Cmd::CopyEffectFrom, sel);
}

pub fn bitmaps_menu(app: &mut VectorApp, ui: &mut Ui) {
    let sel = app.has_selection();
    item(app, ui, Some("bitmap"), "Con&vert to Bitmap...", Cmd::ConvertToBitmap, sel);
    sep(ui);
    item(app, ui, None, "&Auto Adjust", Cmd::AutoAdjust, sel);
    item(app, ui, None, "&Resample...", Cmd::ResampleBitmap, sel);
    sep(ui);
    item(app, ui, Some("trace"), "&Quick Trace", Cmd::QuickTrace, sel);
    item(app, ui, Some("trace"), "&Outline Trace (PowerTRACE)...", Cmd::OutlineTrace, sel);
    sep(ui);
    submenu(app, ui, Some("effects"), "&Effects", sel, |app, ui| {
        for (i, n) in crate::exec::BITMAP_EFFECTS.iter().enumerate() {
            item(app, ui, None, n, Cmd::BitmapEffect(i as u8), true);
        }
    });
}

pub fn text_menu(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    let sel = app.has_selection();
    item(app, ui, Some("text_docker"), "&Text", Cmd::Docker(DockerId::Text), true);
    item(app, ui, None, "&Edit Text...", Cmd::EditText, sel);
    sep(ui);
    item(app, ui, Some("glyphs"), "Insert &Character", Cmd::InsertGlyph, doc);
    sep(ui);
    item(app, ui, Some("fit_path"), "&Fit Text To Path", Cmd::FitTextToPath, sel);
    item(app, ui, None, "&Straighten Text", Cmd::StraightenText, sel);
    item(app, ui, None, "Con&vert", Cmd::ConvertText, sel);
    sep(ui);
    submenu(app, ui, None, "Change &Case", sel, |app, ui| {
        item(app, ui, None, "&Sentence case.", Cmd::ChangeCase(0), true);
        item(app, ui, None, "&lowercase", Cmd::ChangeCase(1), true);
        item(app, ui, None, "&UPPERCASE", Cmd::ChangeCase(2), true);
        item(app, ui, None, "&Title Case", Cmd::ChangeCase(3), true);
        item(app, ui, None, "t&OGGLE cASE", Cmd::ChangeCase(4), true);
    });
    item(app, ui, None, "Spell C&heck...", Cmd::SpellCheck, doc);
    item(app, ui, Some("font_sampler"), "Font Sa&mpler", Cmd::Docker(DockerId::FontSampler), true);
}

pub fn table_menu(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    let tbl = app.doc().map(|d| d.selection.len() == 1 && d.doc.find(d.selection[0]).map(|o| matches!(o.kind, vector_core::ObjKind::Table(_))).unwrap_or(false)).unwrap_or(false);
    item(app, ui, Some("table"), "Create &New Table...", Cmd::CreateTable, doc);
    sep(ui);
    item(app, ui, None, "Insert Row &Above", Cmd::TableInsertRow(true), tbl);
    item(app, ui, None, "Insert Row &Below", Cmd::TableInsertRow(false), tbl);
    item(app, ui, None, "Insert Column &Left", Cmd::TableInsertCol(true), tbl);
    item(app, ui, None, "Insert Column &Right", Cmd::TableInsertCol(false), tbl);
    sep(ui);
    item(app, ui, None, "Delete R&ow", Cmd::TableDeleteRow, tbl);
    item(app, ui, None, "Delete &Column", Cmd::TableDeleteCol, tbl);
    sep(ui);
    item(app, ui, None, "Distribute Rows E&venly", Cmd::TableDistributeRows, tbl);
    item(app, ui, None, "Distribute Columns Evenl&y", Cmd::TableDistributeCols, tbl);
}
