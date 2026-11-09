//! Workspaces (Window ▸ Workspace, Tools ▸ Options ▸ Workspaces): the built-in layouts, the
//! user's own workspaces and workspace files (*.cdws).
//!
//! A workspace holds the toolbars, the dockers, the menu bar and the shortcut keys. Each
//! workspace keeps the layout it had when the user left it, so switching back restores it.

use std::collections::BTreeSet;
use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

use crate::app::VectorApp;
use crate::dialogs::options::WORKSPACES;
use crate::dockers::DockerId;
use crate::settings::{ButtonSize, MenuBarMode, Settings};

/// What a workspace holds: the toolbars, the dockers, the menu bar and the shortcut keys.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub show_menu_bar: bool,
    pub menu_bar_mode: MenuBarMode,
    pub show_standard: bool,
    pub show_property_bar: bool,
    pub show_toolbox: bool,
    pub show_status_bar: bool,
    pub extra_toolbars: Vec<String>,
    pub button_size: ButtonSize,
    pub lock_toolbars: bool,
    pub open_dockers: Vec<String>,
    pub docker_width: f32,
    pub shortcuts: Vec<(String, String)>,
}

impl Default for Layout {
    fn default() -> Layout {
        let mut l = Layout::of(&Settings::default());
        l.open_dockers.clear();
        l
    }
}

impl Layout {
    /// The layout the settings hold now.
    pub fn of(s: &Settings) -> Layout {
        Layout {
            show_menu_bar: s.show_menu_bar,
            menu_bar_mode: s.menu_bar_mode,
            show_standard: s.show_standard,
            show_property_bar: s.show_property_bar,
            show_toolbox: s.show_toolbox,
            show_status_bar: s.show_status_bar,
            extra_toolbars: s.extra_toolbars.clone(),
            button_size: s.button_size,
            lock_toolbars: s.lock_toolbars,
            open_dockers: s.open_dockers.clone(),
            docker_width: s.docker_width,
            shortcuts: s.shortcuts.clone(),
        }
    }

    /// Puts the layout into the settings.
    pub fn put(&self, s: &mut Settings) {
        s.show_menu_bar = self.show_menu_bar;
        s.menu_bar_mode = self.menu_bar_mode;
        s.show_standard = self.show_standard;
        s.show_property_bar = self.show_property_bar;
        s.show_toolbox = self.show_toolbox;
        s.show_status_bar = self.show_status_bar;
        s.extra_toolbars = self.extra_toolbars.clone();
        s.button_size = self.button_size;
        s.lock_toolbars = self.lock_toolbars;
        s.open_dockers = self.open_dockers.clone();
        s.docker_width = self.docker_width;
        s.shortcuts = self.shortcuts.clone();
    }
}

/// A workspace the user made (New, Duplicate or Import).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Custom {
    pub name: String,
    /// The workspace it was based on.
    pub base: String,
    pub description: String,
}

/// The name shown in the lists.
pub fn display(name: &str) -> &str {
    match name {
        "Adobe Illustrator" => "Adobe® Illustrator®",
        n => n,
    }
}

/// What the built-in workspaces are for (Options ▸ Workspaces ▸ Description).
fn builtin_description(name: &str) -> &'static str {
    match name {
        "Lite" => "The Lite workspace is ideal for new users who are getting started. Its clean look invites you to explore AnyVector in a friendly environment.",
        "Default" => "The Default workspace places tools and controls where they are easy to find. It suits users who know other vector graphics software as well as long-time users.",
        "Touch" => "Optimized for touch-enabled devices, the Touch workspace is ideal for field work and sketching. Work with touch, a dial or a stylus, without a mouse or keyboard.",
        "Illustration" => "The Illustration workspace is ideal for an intuitive and efficient workflow when you create book covers, magazine ads, storyboards and other illustrations.",
        "Page Layout" => "The Page Layout workspace is optimized for arranging graphics and text into layouts for business cards, branding, product packaging and multipage documents such as brochures and newsletters.",
        "Adobe Illustrator" => "This workspace has the look and feel of Adobe® Illustrator®, helping Illustrator users to get started quickly with AnyVector.",
        _ => "",
    }
}

/// Every workspace: the built-in ones (alphabetical, as in the lists) and then the user's.
pub fn names(s: &Settings) -> Vec<String> {
    let mut v: Vec<&str> = WORKSPACES.iter().map(|(n, _)| *n).collect();
    v.sort_by_key(|n| display(n).to_lowercase());
    let mut out: Vec<String> = v.into_iter().map(String::from).collect();
    out.extend(s.custom_workspaces.iter().map(|c| c.name.clone()));
    out
}

pub fn is_custom(s: &Settings, name: &str) -> bool {
    s.custom_workspaces.iter().any(|c| c.name == name)
}

pub fn exists(s: &Settings, name: &str) -> bool {
    let n = name.trim().to_lowercase();
    names(s).iter().any(|w| w.to_lowercase() == n || display(w).to_lowercase() == n)
}

pub fn description(s: &Settings, name: &str) -> String {
    match s.custom_workspaces.iter().find(|c| c.name == name) {
        Some(c) => c.description.clone(),
        None => builtin_description(name).to_string(),
    }
}

/// The workspace `Cmd::Workspace(i)` stands for: the built-in ones in menu order, then the user's.
pub fn by_index(s: &Settings, i: usize) -> Option<String> {
    match WORKSPACES.get(i) {
        Some((n, _)) => Some(n.to_string()),
        None => s.custom_workspaces.get(i - WORKSPACES.len()).map(|c| c.name.clone()),
    }
}

pub fn index_of(s: &Settings, name: &str) -> Option<u8> {
    if let Some(i) = WORKSPACES.iter().position(|(n, _)| *n == name) {
        return Some(i as u8);
    }
    s.custom_workspaces.iter().position(|c| c.name == name).map(|k| (WORKSPACES.len() + k) as u8)
}

/// The layout a built-in workspace starts with.
pub fn preset(name: &str) -> Layout {
    let mut l = Layout::default();
    let dk = |d: &[DockerId]| d.iter().map(|d| format!("{d:?}")).collect::<Vec<_>>();
    match name {
        "Lite" => l.open_dockers = dk(&[DockerId::Properties]),
        "Touch" => l.button_size = ButtonSize::Large,
        "Illustration" | "Adobe Illustrator" => l.open_dockers = dk(&[DockerId::Objects, DockerId::Properties]),
        "Page Layout" => l.open_dockers = dk(&[DockerId::Pages, DockerId::Properties]),
        _ => {}
    }
    l
}

/// The layout on screen now.
pub fn capture(app: &mut VectorApp) -> Layout {
    app.dockers.save(&mut app.settings);
    Layout::of(&app.settings)
}

/// The layout a workspace has: the screen for the current one, else the one it had last.
pub fn layout_of(app: &mut VectorApp, name: &str) -> Layout {
    if app.settings.workspace == name {
        return capture(app);
    }
    if let Some(l) = app.settings.workspace_layouts.get(name) {
        return l.clone();
    }
    match app.settings.custom_workspaces.iter().find(|c| c.name == name) {
        Some(c) => preset(&c.base),
        None => preset(name),
    }
}

/// Shows a layout: the toolbars, the dockers and the shortcut keys.
pub fn apply(app: &mut VectorApp, l: &Layout) {
    l.put(&mut app.settings);
    app.dockers.open = l.open_dockers.iter().filter_map(|n| crate::dockers::ALL.iter().copied().find(|d| format!("{d:?}") == *n)).collect();
    app.dockers.active = app.dockers.open.last().copied();
    app.dockers.set_width = None;
}

/// Makes `name` the current workspace. The workspace left keeps its layout for later, unless
/// it is gone (deleted).
pub fn switch(app: &mut VectorApp, name: &str) {
    let cur = app.settings.workspace.clone();
    if cur != name && names(&app.settings).contains(&cur) {
        let now = capture(app);
        app.settings.workspace_layouts.insert(cur, now);
    }
    let l = layout_of(app, name);
    app.settings.workspace = name.to_string();
    apply(app, &l);
    app.flash(format!("Workspace: {}", display(name)));
}

/// Window ▸ Workspace ▸ New / Options ▸ Workspaces ▸ Duplicate: a workspace of the user's,
/// starting with the layout of `base`.
pub fn create(app: &mut VectorApp, name: &str, base: &str, description: &str, set_current: bool) {
    let name = name.trim().to_string();
    if name.is_empty() || exists(&app.settings, &name) {
        return;
    }
    let l = layout_of(app, base);
    app.settings.custom_workspaces.push(Custom { name: name.clone(), base: base.to_string(), description: description.trim().to_string() });
    app.settings.workspace_layouts.insert(name.clone(), l);
    if set_current {
        switch(app, &name);
    }
    app.settings.save();
}

/// Window ▸ Workspace ▸ Delete: removes one of the user's workspaces. Deleting the current one
/// makes Default the current workspace.
pub fn delete(app: &mut VectorApp, name: &str) {
    if !is_custom(&app.settings, name) {
        return;
    }
    app.settings.custom_workspaces.retain(|c| c.name != name);
    app.settings.workspace_layouts.remove(name);
    if app.settings.workspace == name {
        switch(app, "Default");
    }
    app.settings.save();
}

/// Options ▸ Cancel: workspace changes made in the dialog are kept (they took effect at once).
pub fn keep_on_cancel(backup: &mut Settings, live: &Settings) {
    if backup.workspace == live.workspace && backup.custom_workspaces == live.custom_workspaces {
        return;
    }
    backup.workspace = live.workspace.clone();
    backup.custom_workspaces = live.custom_workspaces.clone();
    backup.workspace_layouts = live.workspace_layouts.clone();
    Layout::of(live).put(backup);
}

// ------------------------------------------------------------------------------------------
// Elements (Export and Import Workspace)
// ------------------------------------------------------------------------------------------

/// A group of elements in the Export and Import trees; a group without items is one element.
#[derive(Clone, Debug)]
pub struct Group {
    pub key: String,
    pub label: String,
    pub items: Vec<(String, String)>,
}

/// The elements of a layout: dockers, toolbars, menus and shortcut keys.
pub fn elements(l: &Layout) -> Vec<Group> {
    let dockers = l
        .open_dockers
        .iter()
        .filter_map(|n| crate::dockers::ALL.iter().copied().find(|d| format!("{d:?}") == *n))
        .map(|d| (format!("docker:{d:?}"), crate::dockers::title(d)))
        .collect();
    let mut bars = vec![
        ("bar:standard".to_string(), "Standard".to_string()),
        ("bar:property".to_string(), "Property Bar".to_string()),
        ("bar:toolbox".to_string(), "Toolbox".to_string()),
        ("bar:status".to_string(), "Status Bar".to_string()),
    ];
    bars.extend(l.extra_toolbars.iter().map(|t| (format!("bar:x:{t}"), t.clone())));
    vec![
        Group { key: "dockers".into(), label: "Dockers".into(), items: dockers },
        Group { key: "toolbars".into(), label: "Toolbars".into(), items: bars },
        Group { key: "menus".into(), label: "Menus".into(), items: vec![("menu:bar".into(), "Menu Bar".into())] },
        Group { key: "keys".into(), label: "Shortcut Keys".into(), items: vec![] },
    ]
}

/// The element keys of every group (the group key for a group without items).
pub fn all_keys(groups: &[Group]) -> BTreeSet<String> {
    let mut k = BTreeSet::new();
    for g in groups {
        k.insert(g.key.clone());
        k.extend(g.items.iter().map(|(i, _)| i.clone()));
    }
    k
}

/// Copies the chosen elements of `from` into `into`.
pub fn merge(into: &mut Layout, from: &Layout, keys: &BTreeSet<String>) {
    let docks: Vec<String> = from.open_dockers.iter().filter(|n| keys.contains(&format!("docker:{n}"))).cloned().collect();
    if keys.contains("dockers") {
        into.open_dockers = docks;
        into.docker_width = from.docker_width;
    }
    let mut any_bar = false;
    for (k, on, to) in [
        ("bar:standard", from.show_standard, &mut into.show_standard),
        ("bar:property", from.show_property_bar, &mut into.show_property_bar),
        ("bar:toolbox", from.show_toolbox, &mut into.show_toolbox),
        ("bar:status", from.show_status_bar, &mut into.show_status_bar),
    ] {
        if keys.contains(k) {
            *to = on;
            any_bar = true;
        }
    }
    for t in &from.extra_toolbars {
        if keys.contains(&format!("bar:x:{t}")) {
            any_bar = true;
            if !into.extra_toolbars.contains(t) {
                into.extra_toolbars.push(t.clone());
            }
        }
    }
    if any_bar {
        into.button_size = from.button_size;
        into.lock_toolbars = from.lock_toolbars;
    }
    if keys.contains("menu:bar") {
        into.show_menu_bar = from.show_menu_bar;
        into.menu_bar_mode = from.menu_bar_mode;
    }
    if keys.contains("keys") {
        into.shortcuts = from.shortcuts.clone();
    }
}

// ------------------------------------------------------------------------------------------
// Workspace files
// ------------------------------------------------------------------------------------------

/// The content of a workspace file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceFile {
    pub name: String,
    pub description: String,
    /// The element keys the file holds.
    pub elements: Vec<String>,
    pub layout: Layout,
}

const CONTENT: &str = "content/workspace.json";

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Writes a workspace file: a package with the title and description, and the layout.
pub fn write_file(path: &std::path::Path, f: &WorkspaceFile) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipWriter::new(file);
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let meta = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  <rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n   <dc:title>{}</dc:title>\n   <dc:description>{}</dc:description>\n   <dc:creator>AnyVector {}</dc:creator>\n  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n",
        xml_escape(&f.name),
        xml_escape(&f.description),
        vector_core::APP_VERSION
    );
    let body = serde_json::to_vec_pretty(f).map_err(|e| e.to_string())?;
    let parts: [(&str, &[u8], zip::write::SimpleFileOptions); 3] = [("mimetype", b"application/x-vector-workspace", stored), ("META-INF/metadata.xml", meta.as_bytes(), deflated), (CONTENT, &body, deflated)];
    for (name, data, opts) in parts {
        z.start_file(name, opts).map_err(|e| e.to_string())?;
        z.write_all(data).map_err(|e| e.to_string())?;
    }
    z.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Reads a workspace file written by Vector.
pub fn read_file(path: &std::path::Path) -> Result<WorkspaceFile, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(file).map_err(|_| "The file is not a workspace file.".to_string())?;
    let mut s = String::new();
    match z.by_name(CONTENT) {
        Ok(mut e) => {
            e.read_to_string(&mut s).map_err(|e| e.to_string())?;
        }
        Err(_) => return Err("This workspace was not made with AnyVector, so its elements cannot be read.".into()),
    }
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_round_trip() {
        let dir = std::env::temp_dir().join(format!("vector_ws_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("Mine.cdws");
        let mut l = preset("Page Layout");
        l.shortcuts = vec![("Cmd".into(), "Ctrl+K".into())];
        let f = WorkspaceFile { name: "Mine & yours".into(), description: "A <test>".into(), elements: all_keys(&elements(&l)).into_iter().collect(), layout: l.clone() };
        write_file(&p, &f).unwrap();
        let g = read_file(&p).unwrap();
        assert_eq!(g.name, f.name);
        assert_eq!(g.layout, l);
        assert_eq!(g.elements, f.elements);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_chosen_elements() {
        let mut into = preset("Default");
        let mut from = preset("Page Layout");
        from.show_toolbox = false;
        from.extra_toolbars = vec!["Text".into()];
        from.shortcuts = vec![("A".into(), "B".into())];
        let keys: BTreeSet<String> = ["dockers", "docker:Pages", "toolbars", "bar:toolbox", "bar:x:Text"].iter().map(|s| s.to_string()).collect();
        merge(&mut into, &from, &keys);
        assert_eq!(into.open_dockers, vec!["Pages".to_string()]);
        assert!(!into.show_toolbox);
        assert_eq!(into.extra_toolbars, vec!["Text".to_string()]);
        assert!(into.shortcuts.is_empty());
    }

    #[test]
    fn names_are_alphabetical_then_custom() {
        let mut s = Settings::default();
        s.custom_workspaces.push(Custom { name: "Mine".into(), base: "Default".into(), description: String::new() });
        let n = names(&s);
        assert_eq!(n, vec!["Adobe Illustrator", "Default", "Illustration", "Lite", "Page Layout", "Touch", "Mine"]);
        assert_eq!(by_index(&s, 6).as_deref(), Some("Mine"));
        assert_eq!(index_of(&s, "Mine"), Some(6));
        assert!(exists(&s, "adobe® illustrator®"));
    }
}
