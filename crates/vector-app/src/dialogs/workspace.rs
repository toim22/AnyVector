//! The workspace dialogs: New Workspace, Duplicate Workspace, Delete Workspace, Export
//! Workspace and Import Workspace, and the Workspaces page of Tools ▸ Options.

use std::collections::BTreeSet;

use egui::{pos2, vec2, Align2, Color32, FontId, Id, Rect, RichText, Sense, Stroke, Ui, UiBuilder};

use crate::app::VectorApp;
use crate::theme;
use crate::workspaces::{self as ws, Group, Layout, WorkspaceFile};

#[derive(Default)]
pub struct WorkspaceDialogs {
    pub new: Option<NewWs>,
    /// Delete Workspace: the workspace to delete.
    pub delete: Option<String>,
    pub export: Option<ExportWs>,
    pub import: Option<ImportWs>,
}

impl WorkspaceDialogs {
    pub fn any_open(&self) -> bool {
        self.new.is_some() || self.delete.is_some() || self.export.is_some() || self.import.is_some()
    }
}

/// New Workspace (Window ▸ Workspace ▸ New) and Duplicate Workspace (Options ▸ Workspaces).
pub struct NewWs {
    pub name: String,
    /// The workspace it is based on (a name of `ws::names`).
    pub base: String,
    pub description: String,
    pub set_current: bool,
    /// Duplicate Workspace: no "Base on" list, and the button says Duplicate.
    pub duplicate: bool,
}

/// The elements tree of Export and Import Workspace.
pub struct Tree {
    pub groups: Vec<Group>,
    pub checked: BTreeSet<String>,
    pub expanded: BTreeSet<usize>,
}

impl Tree {
    fn new(groups: Vec<Group>, only: Option<&BTreeSet<String>>) -> Tree {
        let mut groups = groups;
        if let Some(only) = only {
            for g in groups.iter_mut() {
                g.items.retain(|(k, _)| only.contains(k));
            }
            groups.retain(|g| only.contains(&g.key));
        }
        let checked = ws::all_keys(&groups);
        Tree { groups, checked, expanded: BTreeSet::new() }
    }

    /// The checked keys, with the key of every group that has a checked item.
    fn chosen(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for g in &self.groups {
            let items: Vec<&String> = g.items.iter().map(|(k, _)| k).filter(|k| self.checked.contains(*k)).collect();
            if (g.items.is_empty() && self.checked.contains(&g.key)) || !items.is_empty() || (g.key == "dockers" && self.checked.contains(&g.key)) {
                out.insert(g.key.clone());
            }
            out.extend(items.into_iter().cloned());
        }
        out
    }
}

pub struct ExportWs {
    pub name: String,
    pub description: String,
    pub layout: Layout,
    pub tree: Tree,
}

pub struct ImportWs {
    pub file: WorkspaceFile,
    pub tree: Tree,
    /// Import selected elements into: true a new workspace, false the current workspace.
    pub new_ws: bool,
    pub base: String,
    pub name: String,
    pub description: String,
}

// ------------------------------------------------------------------------------------------
// opening
// ------------------------------------------------------------------------------------------

pub fn open_new(app: &mut VectorApp) {
    let base = app.settings.workspace.clone();
    app.dialogs.ws.new = Some(NewWs { name: String::new(), base, description: String::new(), set_current: true, duplicate: false });
}

pub fn open_duplicate(app: &mut VectorApp, base: &str) {
    app.dialogs.ws.new = Some(NewWs { name: String::new(), base: base.to_string(), description: String::new(), set_current: true, duplicate: true });
}

pub fn open_export(app: &mut VectorApp, name: &str) {
    let layout = ws::layout_of(app, name);
    let tree = Tree::new(ws::elements(&layout), None);
    let description = ws::description(&app.settings, name);
    app.dialogs.ws.export = Some(ExportWs { name: name.to_string(), description, layout, tree });
}

/// Window ▸ Workspace ▸ Import: the file, then the Import Workspace dialog.
pub fn import(app: &mut VectorApp) {
    let Some(p) = rfd::FileDialog::new().set_title("Import Workspace").add_filter("Workspace File (*.cdws)", &["cdws"]).pick_file() else { return };
    open_import(app, &p);
}

pub fn open_import(app: &mut VectorApp, p: &std::path::Path) {
    match ws::read_file(p) {
        Ok(file) => {
            let only: BTreeSet<String> = file.elements.iter().cloned().collect();
            let tree = Tree::new(ws::elements(&file.layout), Some(&only));
            let mut name = file.name.clone();
            if name.is_empty() {
                name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            }
            let description = file.description.clone();
            let base = app.settings.workspace.clone();
            app.dialogs.ws.import = Some(ImportWs { file, tree, new_ws: true, base, name, description });
        }
        Err(e) => app.dialogs.message_box = Some(("Import Workspace".into(), e)),
    }
}

// ------------------------------------------------------------------------------------------
// small widgets (placed at window coordinates, like the reference)
// ------------------------------------------------------------------------------------------

pub(super) fn label_right(ui: &Ui, right: egui::Pos2, text: &str, enabled: bool) {
    ui.painter().text(right, Align2::RIGHT_CENTER, text, FontId::proportional(13.0), if enabled { theme::TEXT } else { theme::DISABLED });
}

pub(super) fn label_left(ui: &Ui, left: egui::Pos2, text: &str) {
    ui.painter().text(left, Align2::LEFT_CENTER, text, FontId::proportional(13.0), theme::TEXT);
}

pub(super) fn put_button(ui: &mut Ui, r: Rect, text: &str, primary: bool, enabled: bool) -> bool {
    let mut b = if primary { egui::Button::new(RichText::new(text).color(if enabled { Color32::WHITE } else { theme::DISABLED })) } else { egui::Button::new(text) };
    if primary && enabled {
        b = b.fill(theme::ACCENT);
    }
    let layout = egui::Layout::centered_and_justified(egui::Direction::LeftToRight);
    ui.scope_builder(UiBuilder::new().max_rect(r).layout(layout), |ui| ui.add_enabled(enabled, b.min_size(r.size())).clicked()).inner
}

pub(super) fn put_text(ui: &mut Ui, r: Rect, s: &mut String, multiline: bool, enabled: bool, focus: bool) {
    ui.scope_builder(UiBuilder::new().max_rect(r), |ui| {
        ui.add_enabled_ui(enabled, |ui| {
            let te = if multiline { egui::TextEdit::multiline(s).desired_rows(5) } else { egui::TextEdit::singleline(s) };
            let resp = ui.add_sized(r.size(), te.margin(egui::Margin::symmetric(5, 4)));
            if focus && !resp.has_focus() && ui.memory(|m| m.focused().is_none()) {
                resp.request_focus();
            }
        });
    });
}

fn put_combo(ui: &mut Ui, r: Rect, id: &str, cur: &mut String, items: &[String], enabled: bool) {
    ui.scope_builder(UiBuilder::new().max_rect(r), |ui| {
        ui.add_enabled_ui(enabled, |ui| {
            ui.spacing_mut().interact_size.y = r.height();
            egui::ComboBox::from_id_salt(id).selected_text(ws::display(cur)).width(r.width()).height(300.0).show_ui(ui, |ui| {
                for n in items {
                    ui.selectable_value(cur, n.clone(), ws::display(n));
                }
            });
        });
    });
}

pub(super) fn put_check(ui: &mut Ui, at: egui::Pos2, v: &mut bool, text: &str) {
    ui.scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(at, vec2(220.0, 22.0))), |ui| {
        ui.checkbox(v, text);
    });
}

pub(super) fn put_radio(ui: &mut Ui, at: egui::Pos2, on: bool, text: &str) -> bool {
    ui.scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(at, vec2(160.0, 22.0))), |ui| ui.radio(on, text).clicked()).inner
}

pub(super) fn hline(ui: &Ui, x0: f32, x1: f32, y: f32) {
    ui.painter().hline(x0..=x1, y.round() + 0.5, Stroke::new(1.0, theme::SEP));
}

/// A check box drawn for the tree: 0 clear, 1 partly checked, 2 checked.
fn tree_check(ui: &Ui, r: Rect, state: u8) {
    let p = ui.painter();
    let b = Rect::from_center_size(r.center(), vec2(13.0, 13.0));
    if state == 0 {
        p.rect_stroke(b, 2.0, Stroke::new(1.0, Color32::from_gray(0x9a)), egui::StrokeKind::Inside);
    } else {
        p.rect_filled(b, 2.0, theme::FOCUS);
        if state == 2 {
            p.line(vec![b.left_center() + vec2(3.0, 0.0), b.center_bottom() + vec2(-1.0, -3.5), b.right_top() + vec2(-3.0, 3.5)], Stroke::new(1.6, Color32::WHITE));
        } else {
            p.hline(b.left() + 3.5..=b.right() - 3.5, b.center().y, Stroke::new(1.6, Color32::WHITE));
        }
    }
}

/// The elements tree: a triangle opens a group, the check boxes choose the elements.
fn tree(ui: &mut Ui, area: Rect, t: &mut Tree, salt: &str) {
    ui.painter().rect_filled(area, 0.0, theme::FIELD);
    ui.painter().rect_stroke(area, 0.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Inside);
    let mut y = area.top() + 4.0;
    let row_h = 22.0;
    let clip = ui.clip_rect();
    ui.set_clip_rect(area.shrink(1.0));
    for gi in 0..t.groups.len() {
        let g = t.groups[gi].clone();
        let row = Rect::from_min_size(pos2(area.left() + 1.0, y), vec2(area.width() - 2.0, row_h));
        let tri = Rect::from_center_size(pos2(row.left() + 12.0, row.center().y), vec2(14.0, 14.0));
        let open = t.expanded.contains(&gi);
        if !g.items.is_empty() {
            let c = tri.center();
            let pts = if open { vec![c + vec2(-4.0, -2.0), c + vec2(4.0, -2.0), c + vec2(0.0, 3.0)] } else { vec![c + vec2(-2.0, -4.0), c + vec2(3.0, 0.0), c + vec2(-2.0, 4.0)] };
            ui.painter().add(egui::Shape::convex_polygon(pts, theme::TEXT_DIM, Stroke::NONE));
            if ui.interact(tri, Id::unique((salt, "tri", gi)), Sense::click()).clicked() {
                if open {
                    t.expanded.remove(&gi);
                } else {
                    t.expanded.insert(gi);
                }
            }
        }
        let state = if g.items.is_empty() {
            if t.checked.contains(&g.key) {
                2
            } else {
                0
            }
        } else {
            let n = g.items.iter().filter(|(k, _)| t.checked.contains(k)).count();
            if n == g.items.len() {
                2
            } else if n > 0 {
                1
            } else {
                0
            }
        };
        let cb = Rect::from_center_size(pos2(row.left() + 32.0, row.center().y), vec2(18.0, 18.0));
        tree_check(ui, cb, state);
        ui.painter().text(pos2(cb.right() + 6.0, row.center().y), Align2::LEFT_CENTER, &g.label, FontId::proportional(13.0), theme::TEXT);
        let hit = Rect::from_min_max(cb.min, pos2(row.right(), cb.max.y));
        if ui.interact(hit, Id::unique((salt, "g", gi)), Sense::click()).clicked() {
            let on = state != 2;
            for k in std::iter::once(&g.key).chain(g.items.iter().map(|(k, _)| k)) {
                if on {
                    t.checked.insert(k.clone());
                } else {
                    t.checked.remove(k);
                }
            }
        }
        y += row_h;
        if open {
            for (ii, (k, label)) in g.items.iter().enumerate() {
                let row = Rect::from_min_size(pos2(area.left() + 1.0, y), vec2(area.width() - 2.0, row_h));
                let cb = Rect::from_center_size(pos2(row.left() + 52.0, row.center().y), vec2(18.0, 18.0));
                let on = t.checked.contains(k);
                tree_check(ui, cb, if on { 2 } else { 0 });
                ui.painter().text(pos2(cb.right() + 6.0, row.center().y), Align2::LEFT_CENTER, label, FontId::proportional(13.0), theme::TEXT);
                let hit = Rect::from_min_max(cb.min, pos2(row.right(), cb.max.y));
                if ui.interact(hit, Id::unique((salt, "i", gi, ii)), Sense::click()).clicked() {
                    if on {
                        t.checked.remove(k);
                    } else {
                        t.checked.insert(k.clone());
                    }
                    // the group follows its items
                    if g.items.iter().any(|(k, _)| t.checked.contains(k)) {
                        t.checked.insert(g.key.clone());
                    } else if g.key != "dockers" {
                        t.checked.remove(&g.key);
                    }
                }
                y += row_h;
            }
        }
    }
    ui.set_clip_rect(clip);
}

/// The yellow warning sign of the Delete Workspace box.
fn warning_sign(ui: &Ui, c: egui::Pos2) {
    let p = ui.painter();
    let pts = vec![c + vec2(0.0, -14.0), c + vec2(15.0, 12.0), c + vec2(-15.0, 12.0)];
    p.add(egui::Shape::convex_polygon(pts, Color32::from_rgb(0xf2, 0xc0, 0x1e), Stroke::NONE));
    p.text(c + vec2(0.0, 3.0), Align2::CENTER_CENTER, "!", theme::bold(16.0), Color32::from_rgb(0x20, 0x20, 0x20));
}

// ------------------------------------------------------------------------------------------
// the dialogs
// ------------------------------------------------------------------------------------------

pub fn show(app: &mut VectorApp, ctx: &egui::Context) {
    if app.dialogs.ws.new.is_some() {
        new_dialog(app, ctx);
    }
    if app.dialogs.ws.delete.is_some() {
        delete_dialog(app, ctx);
    }
    if app.dialogs.ws.export.is_some() {
        export_dialog(app, ctx);
    }
    if app.dialogs.ws.import.is_some() {
        import_dialog(app, ctx);
    }
}

fn new_dialog(app: &mut VectorApp, ctx: &egui::Context) {
    let names = ws::names(&app.settings);
    let Some(st) = app.dialogs.ws.new.as_mut() else { return };
    let dup = st.duplicate;
    let (title, h) = if dup { ("Duplicate Workspace", 256.0) } else { ("New Workspace", 289.0) };
    let name_ok = !st.name.trim().is_empty() && !ws::exists(&app.settings, &st.name);
    let mut done = None;
    let ((), dismissed) = super::window(ctx, "ws_new", title, vec2(422.0, h), |ui| {
        let o = ui.max_rect().min - vec2(10.0, 32.0);
        let at = |x: f32, y: f32| o + vec2(x, y);
        label_right(ui, at(115.0, 64.0), "Name:", true);
        put_text(ui, Rect::from_min_max(at(122.0, 52.0), at(382.0, 76.0)), &mut st.name, false, true, true);
        let mut y = 106.0;
        if !dup {
            label_right(ui, at(115.0, 106.0), "Base on:", true);
            put_combo(ui, Rect::from_min_max(at(122.0, 95.0), at(382.0, 118.0)), "ws_base", &mut st.base, &names, true);
            y = 138.0;
        }
        label_right(ui, at(115.0, y + 13.0), "Description:", true);
        put_text(ui, Rect::from_min_max(at(122.0, y), at(382.0, y + 97.0)), &mut st.description, true, true, false);
        let sep = y + 105.0;
        hline(ui, o.x + 10.0, o.x + 412.0, o.y + sep);
        let by = sep + 9.0;
        put_check(ui, at(16.0, by + 1.0), &mut st.set_current, "Set as Current");
        if put_button(ui, Rect::from_min_size(at(243.0, by), vec2(80.0, 24.0)), if dup { "Duplicate" } else { "OK" }, true, name_ok) {
            done = Some(true);
        }
        if put_button(ui, Rect::from_min_size(at(327.0, by), vec2(80.0, 24.0)), "Cancel", false, true) {
            done = Some(false);
        }
    });
    if done == Some(true) {
        if let Some(st) = app.dialogs.ws.new.take() {
            ws::create(app, &st.name, &st.base, &st.description, st.set_current);
            if app.dialogs.options.open {
                app.dialogs.options.workspace_sel = ws::names(&app.settings).iter().position(|n| *n == st.name.trim());
            }
        }
    } else if done == Some(false) || dismissed {
        app.dialogs.ws.new = None;
    }
}

fn delete_dialog(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(name) = app.dialogs.ws.delete.clone() else { return };
    let current = app.settings.workspace == name;
    let mut done = None;
    let h = if current { 190.0 } else { 142.0 };
    let ((), dismissed) = super::window(ctx, "ws_delete", "Delete Workspace", vec2(440.0, h), |ui| {
        let o = ui.max_rect().min - vec2(10.0, 32.0);
        warning_sign(ui, o + vec2(36.0, 64.0));
        let mut y = 50.0;
        for line in ["The selected workspace will be deleted", "permanently."] {
            label_left(ui, o + vec2(68.0, y + 8.0), line);
            y += 18.0;
        }
        if current {
            y += 12.0;
            for line in ["The Default workspace will be set as your current", "workspace."] {
                label_left(ui, o + vec2(68.0, y + 8.0), line);
                y += 18.0;
            }
        }
        let sep = y + 10.0;
        hline(ui, o.x + 10.0, o.x + 430.0, o.y + sep);
        let by = sep + 10.0;
        if put_button(ui, Rect::from_min_size(o + vec2(220.0, by), vec2(100.0, 24.0)), "Delete", true, true) {
            done = Some(true);
        }
        if put_button(ui, Rect::from_min_size(o + vec2(326.0, by), vec2(100.0, 24.0)), "Cancel", false, true) {
            done = Some(false);
        }
    });
    if done == Some(true) {
        app.dialogs.ws.delete = None;
        ws::delete(app, &name);
        if app.dialogs.options.open {
            app.dialogs.options.workspace_sel = ws::names(&app.settings).iter().position(|n| *n == app.settings.workspace);
        }
    } else if done.is_some() || dismissed {
        app.dialogs.ws.delete = None;
    }
}

fn export_dialog(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(st) = app.dialogs.ws.export.as_mut() else { return };
    let mut done = None;
    let any = !st.tree.chosen().is_empty();
    let ((), dismissed) = super::window(ctx, "ws_export", "Export Workspace", vec2(432.0, 482.0), |ui| {
        let o = ui.max_rect().min - vec2(10.0, 32.0);
        label_left(ui, o + vec2(16.0, 52.0), "Select the elements that you want to export:");
        tree(ui, Rect::from_min_max(o + vec2(16.0, 66.0), o + vec2(416.0, 416.0)), &mut st.tree, "ws_export_tree");
        hline(ui, o.x + 10.0, o.x + 422.0, o.y + 430.0);
        if put_button(ui, Rect::from_min_size(o + vec2(252.0, 442.0), vec2(80.0, 24.0)), "Export", true, any) {
            done = Some(true);
        }
        if put_button(ui, Rect::from_min_size(o + vec2(336.0, 442.0), vec2(80.0, 24.0)), "Cancel", false, true) {
            done = Some(false);
        }
    });
    if done == Some(true) {
        let Some(st) = app.dialogs.ws.export.take() else { return };
        let file = WorkspaceFile { name: ws::display(&st.name).to_string(), description: st.description.clone(), elements: st.tree.chosen().into_iter().collect(), layout: st.layout.clone() };
        let fname = format!("{}.cdws", ws::display(&st.name).replace(['®', '/', '\\', ':'], ""));
        if let Some(p) = rfd::FileDialog::new().set_title("Export Workspace").add_filter("Workspace File (*.cdws)", &["cdws"]).set_file_name(fname).save_file() {
            match ws::write_file(&p, &file) {
                Ok(()) => app.flash(format!("Workspace exported to {}", p.display())),
                Err(e) => app.dialogs.message_box = Some(("Export Workspace".into(), format!("The workspace could not be exported: {e}"))),
            }
        }
    } else if done.is_some() || dismissed {
        app.dialogs.ws.export = None;
    }
}

fn import_dialog(app: &mut VectorApp, ctx: &egui::Context) {
    let names = ws::names(&app.settings);
    let exists = |n: &str| ws::exists(&app.settings, n);
    let Some(st) = app.dialogs.ws.import.as_mut() else { return };
    let any = !st.tree.chosen().is_empty();
    let ok = any && (!st.new_ws || (!st.name.trim().is_empty() && !exists(&st.name)));
    let mut done = None;
    let ((), dismissed) = super::window(ctx, "ws_import", "Import Workspace", vec2(432.0, 640.0), |ui| {
        let o = ui.max_rect().min - vec2(10.0, 32.0);
        let at = |x: f32, y: f32| o + vec2(x, y);
        label_left(ui, at(16.0, 52.0), "Select the workspace elements that you want to import.");
        hline(ui, o.x + 16.0, o.x + 416.0, o.y + 70.0);
        tree(ui, Rect::from_min_max(at(16.0, 80.0), at(416.0, 330.0)), &mut st.tree, "ws_import_tree");
        hline(ui, o.x + 16.0, o.x + 416.0, o.y + 342.0);
        label_left(ui, at(16.0, 360.0), "Import selected elements into:");
        hline(ui, o.x + 16.0, o.x + 416.0, o.y + 378.0);
        if put_radio(ui, at(16.0, 386.0), st.new_ws, "New workspace") {
            st.new_ws = true;
        }
        if put_radio(ui, at(150.0, 386.0), !st.new_ws, "Current workspace") {
            st.new_ws = false;
        }
        hline(ui, o.x + 16.0, o.x + 416.0, o.y + 418.0);
        let en = st.new_ws;
        label_right(ui, at(100.0, 440.0), "Base on:", en);
        put_combo(ui, Rect::from_min_max(at(108.0, 428.0), at(288.0, 452.0)), "ws_import_base", &mut st.base, &names, en);
        label_right(ui, at(100.0, 474.0), "Name:", en);
        put_text(ui, Rect::from_min_max(at(108.0, 462.0), at(416.0, 486.0)), &mut st.name, false, en, false);
        label_right(ui, at(100.0, 508.0), "Description:", en);
        put_text(ui, Rect::from_min_max(at(108.0, 496.0), at(416.0, 586.0)), &mut st.description, true, en, false);
        hline(ui, o.x + 10.0, o.x + 422.0, o.y + 596.0);
        if put_button(ui, Rect::from_min_size(at(252.0, 606.0), vec2(80.0, 24.0)), "Import", true, ok) {
            done = Some(true);
        }
        if put_button(ui, Rect::from_min_size(at(336.0, 606.0), vec2(80.0, 24.0)), "Cancel", false, true) {
            done = Some(false);
        }
    });
    if done == Some(true) {
        let Some(st) = app.dialogs.ws.import.take() else { return };
        let keys = st.tree.chosen();
        if st.new_ws {
            let mut l = ws::layout_of(app, &st.base);
            ws::merge(&mut l, &st.file.layout, &keys);
            let name = st.name.trim().to_string();
            app.settings.custom_workspaces.push(ws::Custom { name: name.clone(), base: st.base.clone(), description: st.description.trim().to_string() });
            app.settings.workspace_layouts.insert(name.clone(), l);
            ws::switch(app, &name);
        } else {
            let mut l = ws::capture(app);
            ws::merge(&mut l, &st.file.layout, &keys);
            ws::apply(app, &l);
            app.flash("Workspace imported");
        }
        app.settings.save();
    } else if done.is_some() || dismissed {
        app.dialogs.ws.import = None;
    }
}

// ------------------------------------------------------------------------------------------
// Tools ▸ Options ▸ Workspaces
// ------------------------------------------------------------------------------------------

/// The list of workspaces (in place of the page list) with Import... under it.
pub fn options_list(app: &mut VectorApp, ui: &mut Ui, list: Rect) {
    let names = ws::names(&app.settings);
    if app.dialogs.options.workspace_sel.is_none_or(|i| i >= names.len()) {
        app.dialogs.options.workspace_sel = names.iter().position(|n| *n == app.settings.workspace);
    }
    let frame = Rect::from_min_max(list.min, pos2(list.right(), list.bottom() - 25.0));
    ui.painter().rect_stroke(frame, 0.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Inside);
    for (i, n) in names.iter().enumerate() {
        let row = Rect::from_min_size(pos2(frame.left() + 1.0, frame.top() + 1.0 + 25.0 * i as f32), vec2(frame.width() - 2.0, 25.0));
        if row.bottom() > frame.bottom() {
            break;
        }
        let resp = ui.interact(row, Id::unique(("ws_opt_row", i)), Sense::click());
        if app.dialogs.options.workspace_sel == Some(i) {
            ui.painter().rect_filled(row, 0.0, Color32::from_rgb(0x3a, 0x3a, 0x3a));
        } else if resp.hovered() {
            ui.painter().rect_filled(row, 0.0, Color32::from_rgb(0x31, 0x31, 0x31));
        }
        if *n == app.settings.workspace {
            let c = pos2(row.left() + 24.0, row.center().y);
            ui.painter().line(vec![c + vec2(-6.0, 0.0), c + vec2(-1.5, 5.0), c + vec2(7.0, -6.0)], Stroke::new(2.0, Color32::from_rgb(0x3f, 0xb9, 0x50)));
        }
        ui.painter().text(pos2(row.left() + 47.0, row.center().y), Align2::LEFT_CENTER, ws::display(n), FontId::proportional(13.0), theme::TEXT);
        if resp.clicked() {
            app.dialogs.options.workspace_sel = Some(i);
        }
        if resp.double_clicked() {
            ws::switch(app, n);
        }
    }
    let b = Rect::from_min_max(pos2(list.left(), list.bottom() - 24.0), list.right_bottom());
    if put_button(ui, b, "Import...", false, true) {
        import(app);
    }
}

/// The page: Set as Current, Duplicate..., Delete, Export... and the description.
pub fn options_page(app: &mut VectorApp, ui: &mut Ui) {
    let names = ws::names(&app.settings);
    let sel = app.dialogs.options.workspace_sel.and_then(|i| names.get(i).cloned());
    let r = ui.max_rect();
    let o = r.min;
    let sel_name = sel.clone().unwrap_or_default();
    let custom = ws::is_custom(&app.settings, &sel_name);
    let buttons = [("Set as Current", sel.is_some() && sel_name != app.settings.workspace), ("Duplicate...", sel.is_some()), ("Delete", custom), ("Export...", sel.is_some())];
    for (k, (text, en)) in buttons.iter().enumerate() {
        let br = Rect::from_min_size(o + vec2(0.0, 20.0 + 41.0 * k as f32), vec2(120.0, 24.0));
        if put_button(ui, br, text, false, *en) {
            match k {
                0 => ws::switch(app, &sel_name),
                1 => open_duplicate(app, &sel_name),
                2 => app.dialogs.ws.delete = Some(sel_name.clone()),
                _ => open_export(app, &sel_name),
            }
        }
    }
    hline(ui, o.x + 2.0, r.right() - 8.0, o.y + 204.0);
    ui.painter().text(o + vec2(1.0, 240.0), Align2::LEFT_CENTER, "Description", theme::bold(13.0), theme::TEXT);
    let desc = ws::description(&app.settings, &sel_name);
    let g = ui.painter().layout(desc, FontId::proportional(13.0), theme::TEXT, 500.0_f32.min(r.width() - 10.0));
    ui.painter().galley(o + vec2(1.0, 264.0), g, theme::TEXT);
}
