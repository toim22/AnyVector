//! Window ▸ Color Palettes: the Palette Editor and its Select Color dialog, Open Palette and
//! Create Palette from Selection / Document. Palettes are saved as custom palettes (*.xml).

use std::path::{Path, PathBuf};

use egui::{pos2, vec2, Align2, Color32, FontId, Id, Rect, Sense, Stroke, Ui, UiBuilder};
use vector_core::color::{NamedColor, Palette};
use vector_core::Color;

use super::workspace::{hline, label_left, put_button, put_check, put_radio, put_text};
use crate::app::VectorApp;
use crate::icons::Tone;
use crate::theme;

/// The file type of palettes.
pub const FILTER: &str = "Custom palette (*.xml)";

/// Which palette an edit is for.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Document,
    ColorStyles,
    /// The default palette on the right edge (kept in My Palettes).
    Default,
    File(PathBuf),
}

impl Source {
    pub fn label(&self) -> String {
        match self {
            Source::Document => "Document Palette".into(),
            Source::ColorStyles => "Color Styles Palette".into(),
            Source::Default => "Default Palette".into(),
            Source::File(p) => p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
        }
    }
}

/// A palette being edited, and how it was when the editor got it.
#[derive(Clone)]
pub struct Edit {
    pub source: Source,
    pub palette: Palette,
    pub original: Palette,
}

pub struct EditorState {
    /// The palettes edited so far (OK saves them all); `cur` is the one shown.
    pub edits: Vec<Edit>,
    pub cur: usize,
    pub sel: Option<usize>,
    pub select: Option<SelectColor>,
}

impl EditorState {
    /// The editor is out of the way while the Select Color eyedropper samples a color.
    pub fn sampling(&self) -> bool {
        self.select.as_ref().is_some_and(|s| s.sampling)
    }
}

pub enum Purpose {
    Edit(usize),
    Add,
}

/// Select Color (Edit Color and Add Color of the Palette Editor).
pub struct SelectColor {
    pub purpose: Purpose,
    pub old: Color,
    pub color: Color,
    pub name: String,
    /// Color palettes (true) or Color viewers.
    pub palettes_mode: bool,
    /// The palette list and the shown palette in it.
    pub choices: Vec<Choice>,
    pub pal: usize,
    pub show_names: bool,
    pub sel: Option<usize>,
    /// The first row shown.
    pub row: usize,
    pub sampling: bool,
}

/// An entry of the palette lists: a palette, or a heading (`palette` None).
#[derive(Clone)]
pub struct Choice {
    pub label: String,
    pub indent: u8,
    pub palette: Option<Palette>,
}

// ------------------------------------------------------------------------------------------
// the palettes
// ------------------------------------------------------------------------------------------

/// The file the default palette is kept in once it is edited.
pub fn default_palette_file() -> PathBuf {
    crate::dockers::palettes::my_palettes_dir().join("Default Palette.xml")
}

/// The user's palette files in My Palettes (folders included), with their depth.
fn my_palette_files() -> Vec<(PathBuf, u8)> {
    fn walk(dir: &Path, depth: u8, out: &mut Vec<(PathBuf, u8)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        v.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default());
        for p in v {
            if p.is_dir() {
                if depth < 4 {
                    walk(&p, depth + 1, out);
                }
            } else {
                let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
                if ["xml", "vpal", "gpl", "txt"].contains(&ext.as_str()) && p != default_palette_file() {
                    out.push((p, depth));
                }
            }
        }
    }
    let mut out = vec![];
    walk(&crate::dockers::palettes::my_palettes_dir(), 0, &mut out);
    out
}

fn read_file(p: &Path) -> Option<Palette> {
    let t = std::fs::read_to_string(p).ok()?;
    let stem = p.file_stem()?.to_string_lossy().to_string();
    let mut pal = Palette::parse(&stem, &t);
    if pal.name.is_empty() {
        pal.name = stem;
    }
    Some(pal)
}

fn load(app: &VectorApp, s: &Source) -> Option<Palette> {
    match s {
        Source::Document => app.doc().map(|d| Palette { name: "Document Palette".into(), colors: d.doc.palette.clone() }),
        Source::ColorStyles => app.doc().map(|d| Palette { name: "Color Styles Palette".into(), colors: d.doc.color_styles.clone() }),
        Source::Default => Some(app.palettes.palettes.first().cloned().unwrap_or_else(Palette::default_palette)),
        Source::File(p) => read_file(p),
    }
}

/// Saves an edited palette where it came from, and updates it on screen.
fn store(app: &mut VectorApp, e: &Edit) {
    let colors = e.palette.colors.clone();
    match &e.source {
        Source::Document => {
            if let Some(d) = app.doc_mut() {
                d.modify("Edit Palette", |doc, _| doc.palette = colors);
            }
        }
        Source::ColorStyles => {
            if let Some(d) = app.doc_mut() {
                d.modify("Edit Color Styles", |doc, _| doc.color_styles = colors);
            }
        }
        Source::Default => {
            let mut p = e.palette.clone();
            p.name = "Default Palette".into();
            let _ = std::fs::write(default_palette_file(), p.to_xml());
            match app.palettes.palettes.first_mut() {
                Some(first) => *first = p,
                None => app.palettes.palettes.push(p),
            }
        }
        Source::File(path) => {
            let _ = std::fs::write(path, e.palette.to_xml());
            if let Some(open) = app.palettes.palettes.iter_mut().skip(1).find(|x| x.name == e.original.name) {
                *open = e.palette.clone();
            }
        }
    }
}

// ------------------------------------------------------------------------------------------
// opening
// ------------------------------------------------------------------------------------------

/// Window ▸ Color Palettes ▸ Palette Editor: the default palette first.
pub fn open_editor(app: &mut VectorApp) {
    open_editor_for(app, Source::Default);
}

pub fn open_editor_for(app: &mut VectorApp, s: Source) {
    let Some(p) = load(app, &s) else { return open_editor_for(app, Source::Default) };
    app.dialogs.palette_editor = Some(EditorState { edits: vec![Edit { source: s, palette: p.clone(), original: p }], cur: 0, sel: Some(0), select: None });
}

/// Window ▸ Color Palettes ▸ Open Palette.
pub fn open_palette(app: &mut VectorApp) {
    let Some(p) = rfd::FileDialog::new().set_title("Open Palette").set_directory(crate::dockers::palettes::my_palettes_dir()).add_filter(FILTER, &["xml"]).pick_file() else { return };
    show_palette_file(app, &p);
}

/// Opens a palette file on the right edge.
pub fn show_palette_file(app: &mut VectorApp, p: &Path) {
    let Some(pal) = read_file(p) else {
        app.dialogs.message_box = Some(("Open Palette".into(), format!("{} is not a palette file.", p.display())));
        return;
    };
    let name = pal.name.clone();
    app.palettes.recent.retain(|r| *r != name);
    app.palettes.recent.insert(0, name.clone());
    if !app.palettes.palettes.iter().any(|x| x.name == name) {
        app.palettes.palettes.push(pal);
    }
    let s = p.display().to_string();
    if !app.settings.open_palettes.contains(&s) {
        app.settings.open_palettes.push(s);
    }
}

/// Window ▸ Color Palettes ▸ Create Palette from Selection / Document: the colors go into a
/// new palette file, which opens on the right edge.
pub fn create_palette(app: &mut VectorApp, colors: Vec<Color>) {
    let Some(p) = rfd::FileDialog::new().set_title("New Palette").set_directory(crate::dockers::palettes::my_palettes_dir()).add_filter(FILTER, &["xml"]).save_file() else { return };
    let p = if p.extension().is_none() { p.with_extension("xml") } else { p };
    let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut pal = Palette { name, colors: vec![] };
    for c in colors {
        if !pal.colors.iter().any(|n| n.color == c) {
            pal.colors.push(NamedColor::new(c.to_hex(), c));
        }
    }
    match std::fs::write(&p, pal.to_xml()) {
        Ok(()) => show_palette_file(app, &p),
        Err(e) => app.dialogs.message_box = Some(("New Palette".into(), format!("The palette could not be saved: {e}"))),
    }
}

// ------------------------------------------------------------------------------------------
// drawing helpers
// ------------------------------------------------------------------------------------------

fn icon_btn(app: &mut VectorApp, ui: &mut Ui, c: egui::Pos2, icon: &str, tip: &str, enabled: bool) -> bool {
    let r = Rect::from_center_size(c, vec2(26.0, 26.0));
    let resp = ui.interact(r, ui.scope_id().with(("pe_icon", icon)), if enabled { Sense::click() } else { Sense::hover() }).on_hover_text(tip);
    if resp.hovered() && enabled {
        ui.painter().rect_filled(r, 3.0, theme::HOVER);
    }
    let tone = if enabled { Tone::Normal } else { Tone::Disabled };
    if let Some(t) = app.icons.get(ui.ctx(), icon, 22.0, tone) {
        ui.painter().image(t.id, Rect::from_center_size(c, vec2(22.0, 22.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    enabled && resp.clicked()
}

fn swatch(ui: &Ui, r: Rect, c: Color) {
    crate::widgets::paint_well(ui.painter(), r, Some(c));
}

/// A palette list with headings (My Palettes, Palette Libraries) that cannot be chosen.
fn palette_combo(ui: &mut Ui, r: Rect, id: &str, cur: &mut usize, items: &[(String, u8, bool)]) -> bool {
    let mut changed = false;
    ui.scope_builder(UiBuilder::new().max_rect(r), |ui| {
        ui.spacing_mut().interact_size.y = r.height();
        let text = items.get(*cur).map(|i| i.0.clone()).unwrap_or_default();
        egui::ComboBox::from_id_salt(id).selected_text(text).width(r.width()).height(360.0).show_ui(ui, |ui| {
            for (k, (label, indent, pick)) in items.iter().enumerate() {
                let t = format!("{}{label}", "    ".repeat(*indent as usize));
                if *pick {
                    if ui.selectable_label(*cur == k, t).clicked() {
                        *cur = k;
                        changed = true;
                    }
                } else {
                    ui.label(egui::RichText::new(t).color(theme::TEXT));
                }
            }
        });
    });
    changed
}

/// The components shown for a color: CMYK for the default palette, RGB for the others.
fn components(c: Color, cmyk: bool) -> String {
    if cmyk {
        let [cc, m, y, k] = c.to_cmyk();
        format!("C:{:.0} M:{:.0} Y:{:.0} K:{:.0}", cc, m, y, k)
    } else {
        format!("R:{} G:{} B:{}", c.r, c.g, c.b)
    }
}

// ------------------------------------------------------------------------------------------
// the Palette Editor
// ------------------------------------------------------------------------------------------

pub fn show(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.palette_editor.take() else { return };
    // the eyedropper of Select Color: the canvas takes the click
    if let Some(sc) = st.select.as_mut() {
        if sc.sampling {
            if let Some(c) = app.ts.picked_color.take() {
                sc.color = c;
                sc.palettes_mode = false;
                sc.sampling = false;
            } else if app.ts.pick_target.is_none() {
                sc.sampling = false;
            }
            if sc.sampling {
                app.dialogs.palette_editor = Some(st);
                return;
            }
        }
    }
    let files = my_palette_files();
    let mut keep = true;
    editor_window(app, ctx, &mut st, &files, &mut keep);
    if st.select.is_some() {
        select_color(app, ctx, &mut st);
    }
    if keep {
        app.dialogs.palette_editor = Some(st);
    }
}

fn editor_window(app: &mut VectorApp, ctx: &egui::Context, st: &mut EditorState, files: &[(PathBuf, u8)], keep: &mut bool) {
    // the palettes of the list: Document, Color Styles, then My Palettes
    let mut sources: Vec<(Option<Source>, String, u8)> = vec![];
    if app.has_doc() {
        sources.push((Some(Source::Document), "Document Palette".into(), 0));
        sources.push((Some(Source::ColorStyles), "Color Styles Palette".into(), 0));
    }
    sources.push((None, "My Palettes".into(), 0));
    sources.push((Some(Source::Default), "Default Palette".into(), 1));
    for (p, d) in files {
        sources.push((Some(Source::File(p.clone())), p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), d + 1));
    }
    for e in &st.edits {
        if !sources.iter().any(|(s, _, _)| s.as_ref() == Some(&e.source)) {
            sources.push((Some(e.source.clone()), e.source.label(), 1));
        }
    }
    let items: Vec<(String, u8, bool)> = sources.iter().map(|(s, l, d)| (l.clone(), *d, s.is_some())).collect();
    let cur_source = st.edits[st.cur].source.clone();
    let mut list_at = sources.iter().position(|(s, _, _)| s.as_ref() == Some(&cur_source)).unwrap_or(0);
    let mut result = None;
    let mut action: Option<&str> = None;
    let ((), dismissed) = super::window(ctx, "palette_editor", "Palette Editor", vec2(480.0, 544.0), |ui| {
        let o = ui.max_rect().min - vec2(10.0, 32.0);
        let at = |x: f32, y: f32| o + vec2(x, y);
        // the palette, and New, Open, Save and Save As
        if palette_combo(ui, Rect::from_min_max(at(40.0, 41.0), at(294.0, 64.0)), "pe_palette", &mut list_at, &items) {
            if let Some(s) = sources[list_at].0.clone() {
                match st.edits.iter().position(|e| e.source == s) {
                    Some(i) => st.cur = i,
                    None => {
                        if let Some(p) = load(app, &s) {
                            st.edits.push(Edit { source: s, palette: p.clone(), original: p });
                            st.cur = st.edits.len() - 1;
                        }
                    }
                }
                st.sel = (!st.edits[st.cur].palette.colors.is_empty()).then_some(0);
            }
        }
        let dirty = st.edits[st.cur].palette != st.edits[st.cur].original;
        if icon_btn(app, ui, at(334.0, 52.0), "pal_new", "New Palette", true) {
            action = Some("new");
        }
        if icon_btn(app, ui, at(366.0, 52.0), "open", "Open Palette", true) {
            action = Some("open");
        }
        if icon_btn(app, ui, at(398.0, 52.0), "save", "Save Palette", dirty) {
            action = Some("save");
        }
        if icon_btn(app, ui, at(430.0, 52.0), "pal_save_as", "Save Palette As", true) {
            action = Some("save_as");
        }
        // the swatches
        let grid = Rect::from_min_max(at(40.0, 72.0), at(313.0, 328.0));
        if swatch_grid(ui, grid, st) {
            action = Some("edit");
        }
        // the buttons
        let has_sel = st.sel.is_some_and(|i| i < st.edits[st.cur].palette.colors.len());
        let b = |y: f32| Rect::from_min_size(at(334.0, y), vec2(109.0, 24.0));
        if put_button(ui, b(72.0), "Edit Color", false, has_sel) {
            action = Some("edit");
        }
        if put_button(ui, b(102.0), "Add Color", false, true) {
            action = Some("add");
        }
        if put_button(ui, b(132.0), "Delete Color", false, has_sel) {
            action = Some("delete");
        }
        sort_menu(ui, b(163.0), st);
        let resettable = cur_source == Source::Default && st.edits[st.cur].palette.colors != Palette::default_palette().colors;
        if put_button(ui, b(194.0), "Reset Palette", false, resettable) {
            st.edits[st.cur].palette.colors = Palette::default_palette().colors;
            st.sel = Some(0);
        }
        // Selected color
        ui.painter().text(at(40.0, 381.0), Align2::LEFT_CENTER, "Selected color", FontId::proportional(13.0), theme::TEXT);
        hline(ui, o.x + 135.0, o.x + 441.0, o.y + 381.0);
        label_left(ui, at(40.0, 419.0), "Name:");
        label_left(ui, at(40.0, 451.0), "Treat as:");
        label_left(ui, at(40.0, 479.0), "Components:");
        let cmyk = cur_source == Source::Default;
        let colors = &mut st.edits[st.cur].palette.colors;
        match st.sel.and_then(|i| colors.get_mut(i)) {
            Some(nc) => {
                put_text(ui, Rect::from_min_max(at(133.0, 407.0), at(213.0, 431.0)), &mut nc.name, false, true, false);
                let mut spot = nc.spot as usize;
                ui.scope_builder(UiBuilder::new().max_rect(Rect::from_min_max(at(133.0, 440.0), at(213.0, 463.0))), |ui| {
                    super::combo(ui, "pe_treat", &mut spot, &["Process", "Spot"], 66.0);
                });
                nc.spot = spot == 1;
                ui.painter().text(at(133.0, 479.0), Align2::LEFT_CENTER, components(nc.color, cmyk), FontId::proportional(13.0), theme::TEXT);
                let pr = Rect::from_min_max(at(242.0, 406.0), at(321.0, 463.0));
                swatch(ui, pr, nc.color);
                ui.painter().rect_stroke(pr, 0.0, Stroke::new(1.0, Color32::from_gray(0x70)), egui::StrokeKind::Inside);
            }
            None => {
                let mut empty = String::new();
                put_text(ui, Rect::from_min_max(at(133.0, 407.0), at(213.0, 431.0)), &mut empty, false, false, false);
                let pr = Rect::from_min_max(at(242.0, 406.0), at(321.0, 463.0));
                ui.painter().rect_stroke(pr, 0.0, Stroke::new(1.0, Color32::from_gray(0x70)), egui::StrokeKind::Inside);
            }
        }
        // ?, OK and Cancel
        let _ = put_button(ui, Rect::from_min_max(at(16.0, 507.0), at(38.0, 531.0)), "?", false, true);
        if put_button(ui, Rect::from_min_max(at(301.0, 507.0), at(381.0, 531.0)), "OK", true, true) {
            result = Some(true);
        }
        if put_button(ui, Rect::from_min_max(at(387.0, 507.0), at(467.0, 531.0)), "Cancel", false, true) {
            result = Some(false);
        }
        if st.select.is_none() && ui.input(|i| i.key_pressed(egui::Key::Delete)) && !ui.ctx().egui_wants_keyboard_input() && has_sel {
            action = Some("delete");
        }
    });
    match action {
        Some("new") => {
            if let Some(p) = rfd::FileDialog::new().set_title("New Palette").set_directory(crate::dockers::palettes::my_palettes_dir()).add_filter(FILTER, &["xml"]).save_file() {
                let p = if p.extension().is_none() { p.with_extension("xml") } else { p };
                let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let pal = Palette { name, colors: vec![] };
                if std::fs::write(&p, pal.to_xml()).is_ok() {
                    st.edits.push(Edit { source: Source::File(p), palette: pal.clone(), original: pal });
                    st.cur = st.edits.len() - 1;
                    st.sel = None;
                }
            }
        }
        Some("open") => {
            if let Some(p) = rfd::FileDialog::new().set_title("Open Palette").set_directory(crate::dockers::palettes::my_palettes_dir()).add_filter(FILTER, &["xml"]).pick_file() {
                if let Some(pal) = read_file(&p) {
                    st.edits.push(Edit { source: Source::File(p), palette: pal.clone(), original: pal });
                    st.cur = st.edits.len() - 1;
                    st.sel = (!st.edits[st.cur].palette.colors.is_empty()).then_some(0);
                }
            }
        }
        Some("save") => {
            let e = st.edits[st.cur].clone();
            store(app, &e);
            st.edits[st.cur].original = e.palette;
        }
        Some("save_as") => {
            let e = &st.edits[st.cur];
            let fname = format!("{}.xml", e.source.label());
            if let Some(p) = rfd::FileDialog::new().set_title("Save Palette As").set_directory(crate::dockers::palettes::my_palettes_dir()).add_filter(FILTER, &["xml"]).set_file_name(fname).save_file() {
                let p = if p.extension().is_none() { p.with_extension("xml") } else { p };
                let mut pal = e.palette.clone();
                pal.name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if std::fs::write(&p, pal.to_xml()).is_ok() {
                    let old = st.cur;
                    st.edits[old].palette = st.edits[old].original.clone();
                    st.edits.push(Edit { source: Source::File(p), palette: pal.clone(), original: pal });
                    st.cur = st.edits.len() - 1;
                }
            }
        }
        Some("edit") => {
            if let Some(i) = st.sel {
                let nc = st.edits[st.cur].palette.colors[i].clone();
                st.select = Some(new_select(app, Purpose::Edit(i), nc.color, &nc.name));
            }
        }
        Some("add") => {
            let c = st.sel.and_then(|i| st.edits[st.cur].palette.colors.get(i)).map(|c| c.color).unwrap_or(Color::BLACK);
            st.select = Some(new_select(app, Purpose::Add, c, &c.to_hex()));
        }
        Some("delete") => {
            if let Some(i) = st.sel {
                let colors = &mut st.edits[st.cur].palette.colors;
                if i < colors.len() {
                    colors.remove(i);
                    st.sel = if colors.is_empty() { None } else { Some(i.min(colors.len() - 1)) };
                }
            }
        }
        _ => {}
    }
    match result {
        Some(true) => {
            for e in st.edits.clone() {
                if e.palette != e.original {
                    store(app, &e);
                }
            }
            *keep = false;
        }
        Some(false) => *keep = false,
        None if dismissed && st.select.is_none() => *keep = false,
        None => {}
    }
}

/// The palette's swatches, 16 points each; click selects, double-click edits (true).
fn swatch_grid(ui: &mut Ui, grid: Rect, st: &mut EditorState) -> bool {
    ui.painter().rect_filled(grid, 0.0, Color32::from_gray(0x55));
    let colors = st.edits[st.cur].palette.colors.clone();
    let cell = 16.0;
    let cols = (grid.width() / cell).floor().max(1.0) as usize;
    let mut edit = None;
    ui.scope_builder(UiBuilder::new().max_rect(grid), |ui| {
        egui::ScrollArea::vertical().id_salt(("pe_grid", st.cur)).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let rows = colors.len().div_ceil(cols).max(1);
            let (area, _) = ui.allocate_exact_size(vec2(cols as f32 * cell, rows as f32 * cell), Sense::hover());
            for (i, c) in colors.iter().enumerate() {
                let r = Rect::from_min_size(area.min + vec2((i % cols) as f32 * cell, (i / cols) as f32 * cell), vec2(cell, cell));
                let resp = ui.interact(r, Id::unique(("pe_sw", st.cur, i)), Sense::click()).on_hover_text(&c.name);
                swatch(ui, r.shrink(0.5), c.color);
                if st.sel == Some(i) {
                    ui.painter().rect_stroke(r.shrink(1.0), 0.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Inside);
                    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Inside);
                }
                if resp.clicked() {
                    st.sel = Some(i);
                }
                if resp.double_clicked() {
                    edit = Some(i);
                }
            }
        });
    });
    if let Some(i) = edit {
        st.sel = Some(i);
    }
    edit.is_some()
}

/// Sort Colors: Reverse, Hue, Brightness, Saturation, RGB value, HSB value, Name.
fn sort_menu(ui: &mut Ui, r: Rect, st: &mut EditorState) {
    let enabled = st.edits[st.cur].palette.colors.len() > 1;
    let resp = ui
        .scope_builder(UiBuilder::new().max_rect(r).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)), |ui| ui.add_enabled(enabled, egui::Button::new("Sort Colors        ").min_size(r.size())))
        .inner;
    let c = pos2(r.right() - 12.0, r.center().y);
    ui.painter().line(vec![c + vec2(-4.0, -2.0), c + vec2(0.0, 2.0), c + vec2(4.0, -2.0)], Stroke::new(1.2, if enabled { theme::TEXT } else { theme::DISABLED }));
    let colors = &mut st.edits[st.cur].palette.colors;
    let sel_color = st.sel.and_then(|i| colors.get(i)).cloned();
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClick).show(|ui| {
        ui.set_min_width(140.0);
        let hsb = |c: &NamedColor| c.color.to_hsb();
        let cmp = |a: f64, b: f64| a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal);
        if ui.button("Reverse").clicked() {
            colors.reverse();
        }
        if ui.button("Hue").clicked() {
            colors.sort_by(|a, b| cmp(hsb(a)[0], hsb(b)[0]));
        }
        if ui.button("Brightness").clicked() {
            colors.sort_by(|a, b| cmp(hsb(a)[2], hsb(b)[2]));
        }
        if ui.button("Saturation").clicked() {
            colors.sort_by(|a, b| cmp(hsb(a)[1], hsb(b)[1]));
        }
        if ui.button("RGB value").clicked() {
            colors.sort_by_key(|c| ((c.color.r as u32) << 16) | ((c.color.g as u32) << 8) | c.color.b as u32);
        }
        if ui.button("HSB value").clicked() {
            colors.sort_by(|a, b| {
                let (x, y) = (hsb(a), hsb(b));
                cmp(x[0], y[0]).then(cmp(x[1], y[1])).then(cmp(x[2], y[2]))
            });
        }
        if ui.button("Name").clicked() {
            colors.sort_by_key(|c| c.name.to_lowercase());
        }
    });
    // the selection follows its color
    if let Some(sc) = sel_color {
        st.sel = colors.iter().position(|c| *c == sc).or(st.sel);
    }
}

// ------------------------------------------------------------------------------------------
// Select Color
// ------------------------------------------------------------------------------------------

/// The palettes Select Color lists: Document, Color Styles, My Palettes and the Palette
/// Libraries.
fn select_choices(app: &VectorApp) -> Vec<Choice> {
    let pal = |label: &str, indent: u8, p: Option<Palette>| Choice { label: label.into(), indent, palette: p };
    let mut v = vec![];
    if let Some(d) = app.doc() {
        v.push(pal("Document Palette", 0, Some(Palette { name: "Document Palette".into(), colors: d.doc.palette.clone() })));
        v.push(pal("Color Styles Palette", 0, Some(Palette { name: "Color Styles Palette".into(), colors: d.doc.color_styles.clone() })));
    }
    v.push(pal("My Palettes", 0, None));
    v.push(pal("Default Palette", 1, app.palettes.palettes.first().cloned()));
    for (p, d) in my_palette_files() {
        let label = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        v.push(pal(&label, d + 1, read_file(&p)));
    }
    v.push(pal("Palette Libraries", 0, None));
    v.push(pal("Process", 1, None));
    for (k, n) in [("lab", "Lab Colors"), ("svg", "SVG Colors"), ("uniform", "Uniform Colors"), ("websafe", "Web Safe Colors")] {
        v.push(pal(n, 2, crate::dockers::palettes::library_palette(k)));
    }
    v.push(pal("Spot", 1, None));
    for (k, n) in [("gray256", "256 shades of Gray"), ("cmyk", "CMYK Palette"), ("default", "Default Palette"), ("percent", "Percent Gray"), ("rgb", "RGB Palette")] {
        v.push(pal(n, 1, crate::dockers::palettes::library_palette(k)));
    }
    v
}

pub(crate) fn new_select(app: &VectorApp, purpose: Purpose, c: Color, name: &str) -> SelectColor {
    let choices = select_choices(app);
    let pal = choices.iter().position(|x| x.label == "CMYK Palette" && x.indent == 1).unwrap_or(0);
    let sel = choices[pal].palette.as_ref().and_then(|p| p.colors.iter().position(|x| x.color == c));
    SelectColor { purpose, old: c, color: c, name: name.to_string(), palettes_mode: true, choices, pal, show_names: false, sel, row: 0, sampling: false }
}

/// A number box of Select Color: editable in the viewers, read only for a palette color.
fn num(ui: &mut Ui, r: Rect, v: &mut f64, lo: f64, hi: f64, enabled: bool) -> bool {
    ui.scope_builder(UiBuilder::new().max_rect(r), |ui| {
        ui.add_enabled(enabled, egui::DragValue::new(v).range(lo..=hi).speed(1.0).max_decimals(0).update_while_editing(false)).changed()
    })
    .inner
}

fn select_color(app: &mut VectorApp, ctx: &egui::Context, st: &mut EditorState) {
    let Some(sc) = st.select.as_mut() else { return };
    let mut result = None;
    let mut eyedropper = false;
    let ((), dismissed) = super::window(ctx, "pe_select_color", "Select Color", vec2(596.0, 463.0), |ui| {
        let o = ui.max_rect().min - vec2(10.0, 32.0);
        let at = |x: f32, y: f32| o + vec2(x, y);
        if put_radio(ui, at(14.0, 38.0), !sc.palettes_mode, "Color viewers") {
            sc.palettes_mode = false;
        }
        if put_radio(ui, at(146.0, 38.0), sc.palettes_mode, "Color palettes") {
            sc.palettes_mode = true;
        }
        let shown: Vec<NamedColor> = sc.choices.get(sc.pal).and_then(|c| c.palette.as_ref()).map(|p| p.colors.clone()).unwrap_or_default();
        if sc.palettes_mode {
            let items: Vec<(String, u8, bool)> = sc.choices.iter().map(|c| (c.label.clone(), c.indent, c.palette.is_some())).collect();
            if palette_combo(ui, Rect::from_min_max(at(16.0, 64.0), at(215.0, 87.0)), "sc_palette", &mut sc.pal, &items) {
                sc.sel = None;
                sc.row = 0;
            }
            if icon_btn(app, ui, at(240.0, 75.0), "open", "Open Palette", true) {
                if let Some(p) = rfd::FileDialog::new().set_title("Open Palette").set_directory(crate::dockers::palettes::my_palettes_dir()).add_filter(FILTER, &["xml"]).pick_file() {
                    if let Some(pal) = read_file(&p) {
                        sc.choices.push(Choice { label: pal.name.clone(), indent: 0, palette: Some(pal) });
                        sc.pal = sc.choices.len() - 1;
                        sc.sel = None;
                        sc.row = 0;
                    }
                }
            }
            put_check(ui, at(14.0, 92.0), &mut sc.show_names, "Show color names");
            let grid = Rect::from_min_max(at(16.0, 118.0), at(215.0, 318.0));
            let (cols, cell) = if sc.show_names { (1usize, vec2(199.0, 25.0)) } else { (8usize, vec2(25.0, 25.0)) };
            let rows_shown = (grid.height() / cell.y).floor() as usize;
            let rows = shown.len().div_ceil(cols);
            sc.row = sc.row.min(rows.saturating_sub(rows_shown));
            let grid_resp = ui.interact(grid, Id::unique("sc_grid"), Sense::hover());
            if grid_resp.hovered() {
                let dy = ui.input(|i| i.smooth_scroll_delta.y);
                if dy < -1.0 {
                    sc.row = (sc.row + 1).min(rows.saturating_sub(rows_shown));
                } else if dy > 1.0 {
                    sc.row = sc.row.saturating_sub(1);
                }
            }
            for (k, c) in shown.iter().enumerate().skip(sc.row * cols).take(rows_shown * cols) {
                let i = k - sc.row * cols;
                let r = Rect::from_min_size(grid.min + vec2((i % cols) as f32 * cell.x, (i / cols) as f32 * cell.y), cell);
                let sw = if sc.show_names { Rect::from_min_size(r.min + vec2(1.0, 1.0), vec2(23.0, 23.0)) } else { r.shrink(0.5) };
                let resp = ui.interact(r, Id::unique(("sc_sw", sc.pal, k)), Sense::click()).on_hover_text(&c.name);
                swatch(ui, sw, c.color);
                if sc.show_names {
                    ui.painter().text(pos2(sw.right() + 8.0, r.center().y), Align2::LEFT_CENTER, &c.name, FontId::proportional(12.5), theme::TEXT);
                }
                if sc.sel == Some(k) {
                    ui.painter().rect_stroke(sw, 0.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Inside);
                }
                if resp.clicked() {
                    sc.sel = Some(k);
                    sc.color = c.color;
                    sc.name = c.name.clone();
                }
            }
            // the strip: the whole palette, and the part shown; click to go there
            let strip = Rect::from_min_max(at(237.0, 118.0), at(252.0, 316.0));
            if !shown.is_empty() {
                let h = strip.height() / shown.len() as f32;
                for (k, c) in shown.iter().enumerate() {
                    let r = Rect::from_min_max(pos2(strip.left(), strip.top() + k as f32 * h), pos2(strip.right(), strip.top() + (k + 1) as f32 * h + 0.5));
                    ui.painter().rect_filled(r, 0.0, theme::to_c32(c.color));
                }
                let (a, b) = ((sc.row * cols) as f32 * h, (((sc.row + rows_shown) * cols).min(shown.len())) as f32 * h);
                ui.painter().rect_stroke(Rect::from_min_max(pos2(strip.left() - 1.0, strip.top() + a), pos2(strip.right() + 1.0, strip.top() + b)), 0.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Outside);
                let sr = ui.interact(strip, Id::unique("sc_strip"), Sense::click_and_drag());
                if let Some(p) = sr.interact_pointer_pos().filter(|_| sr.clicked() || sr.dragged()) {
                    let k = (((p.y - strip.top()) / h).max(0.0) as usize).min(shown.len() - 1);
                    sc.row = (k / cols).saturating_sub(rows_shown / 2).min(rows.saturating_sub(rows_shown));
                }
            }
        } else {
            // the color viewer
            let vr = Rect::from_min_max(at(16.0, 64.0), at(252.0, 330.0));
            ui.scope_builder(UiBuilder::new().max_rect(vr), |ui| {
                if crate::dockers::coled::viewer(ui, 0, &mut sc.color, 236.0) {
                    sc.sel = None;
                }
            });
        }
        // the color, Swap and the eyedropper
        let sw = Rect::from_min_max(at(16.0, 346.0), at(55.0, 385.0));
        if sc.old != sc.color {
            swatch(ui, Rect::from_min_max(sw.min, pos2(sw.right(), sw.center().y)), sc.old);
            swatch(ui, Rect::from_min_max(pos2(sw.left(), sw.center().y), sw.max), sc.color);
        } else {
            swatch(ui, sw, sc.color);
        }
        ui.painter().rect_stroke(sw, 0.0, Stroke::new(1.0, Color32::from_gray(0x80)), egui::StrokeKind::Inside);
        let swap = Rect::from_center_size(at(73.0, 367.0), vec2(24.0, 24.0));
        let swr = ui.interact(swap, Id::unique("sc_swap"), Sense::click()).on_hover_text("Swap Colors");
        if swr.hovered() {
            ui.painter().rect_filled(swap, 3.0, theme::HOVER);
        }
        {
            let c = swap.center();
            let s = Stroke::new(1.3, theme::TEXT);
            let p = ui.painter();
            p.line(vec![c + vec2(-6.0, 5.0), c + vec2(-6.0, -4.0), c + vec2(6.0, -4.0), c + vec2(6.0, 2.0)], s);
            p.line(vec![c + vec2(3.5, 0.0), c + vec2(6.0, 3.0), c + vec2(8.5, 0.0)], s);
            p.line(vec![c + vec2(-6.0, 5.0), c + vec2(4.0, 5.0)], s);
            p.line(vec![c + vec2(-8.5, -1.0), c + vec2(-6.0, -4.0), c + vec2(-3.5, -1.0)], s);
        }
        if swr.clicked() {
            std::mem::swap(&mut sc.old, &mut sc.color);
        }
        if icon_btn(app, ui, at(240.0, 367.0), "eyedropper", "Eyedropper", app.has_doc()) {
            eyedropper = true;
        }
        // Name, with the names of the shown palette
        label_left(ui, at(297.0, 48.0), "Name:");
        let nr = Rect::from_min_max(at(297.0, 64.0), at(560.0, 87.0));
        put_text(ui, nr, &mut sc.name, false, true, false);
        let chev = Rect::from_min_max(at(560.0, 64.0), at(581.0, 87.0));
        let cresp = ui.interact(chev, Id::unique("sc_names"), Sense::click());
        {
            let c = chev.center();
            ui.painter().line(vec![c + vec2(-4.0, -2.0), c + vec2(0.0, 2.0), c + vec2(4.0, -2.0)], Stroke::new(1.2, theme::TEXT));
        }
        egui::Popup::menu(&cresp).close_behavior(egui::PopupCloseBehavior::CloseOnClick).show(|ui| {
            ui.set_min_width(280.0);
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                for (k, c) in shown.iter().enumerate() {
                    if ui.selectable_label(sc.sel == Some(k), &c.name).clicked() {
                        sc.sel = Some(k);
                        sc.color = c.color;
                        sc.name = c.name.clone();
                        sc.row = k / 8;
                    }
                }
            });
        });
        // the values
        let en = !sc.palettes_mode;
        let mut c = sc.color;
        let mut changed = false;
        let lab = crate::dockers::coled::to_lab(c);
        let cmyk = c.to_cmyk();
        let hsb = c.to_hsb();
        let field = |x: f32, y: f32| Rect::from_min_max(at(x, y - 11.0), at(x + 40.0, y + 11.0));
        let mut rgb = [c.r as f64, c.g as f64, c.b as f64];
        for (k, (l, y)) in [("R:", 119.0), ("G:", 151.0), ("B:", 183.0)].iter().enumerate() {
            label_left(ui, at(319.0, *y), l);
            if num(ui, field(344.0, *y), &mut rgb[k], 0.0, 255.0, en) {
                changed = true;
            }
        }
        if changed {
            c = Color::rgba(rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, c.a);
        }
        let mut hex = c.to_hex();
        let before = hex.clone();
        put_text(ui, Rect::from_min_max(at(316.0, 203.0), at(384.0, 225.0)), &mut hex, false, en, false);
        if hex != before {
            if let Some(h) = Color::from_hex(hex.trim()) {
                c = h;
                changed = true;
            }
        }
        let mut cm = cmyk;
        let mut cm_changed = false;
        for (k, (l, y)) in [("C:", 119.0), ("M:", 151.0), ("Y:", 183.0), ("K:", 214.0)].iter().enumerate() {
            label_left(ui, at(424.0, *y), l);
            if num(ui, field(444.0, *y), &mut cm[k], 0.0, 100.0, en) {
                cm_changed = true;
            }
        }
        if cm_changed {
            c = Color::from_cmyk(cm[0], cm[1], cm[2], cm[3]);
            changed = true;
        }
        let mut lb = lab;
        let mut lab_changed = false;
        for (k, (l, y, lo, hi)) in [("L:", 119.0, 0.0, 100.0), ("a:", 151.0, -128.0, 127.0), ("b:", 183.0, -128.0, 127.0)].iter().enumerate() {
            label_left(ui, at(520.0, *y), l);
            if num(ui, field(544.0, *y), &mut lb[k], *lo, *hi, en) {
                lab_changed = true;
            }
        }
        if lab_changed {
            c = crate::dockers::coled::from_lab(lb[0], lb[1], lb[2]);
            changed = true;
        }
        let mut hv = hsb;
        let mut hsb_changed = false;
        for (k, (l, y, hi)) in [("H:", 258.0, 360.0), ("S:", 290.0, 100.0), ("B:", 322.0, 100.0)].iter().enumerate() {
            label_left(ui, at(319.0, *y), l);
            if num(ui, field(344.0, *y), &mut hv[k], 0.0, *hi, en) {
                hsb_changed = true;
            }
        }
        if hsb_changed {
            c = Color::from_hsb(hv[0], hv[1], hv[2]);
            changed = true;
        }
        if changed {
            sc.color = c;
            sc.sel = None;
        }
        if put_button(ui, Rect::from_min_max(at(418.0, 426.0), at(497.0, 450.0)), "OK", true, true) {
            result = Some(true);
        }
        if put_button(ui, Rect::from_min_max(at(503.0, 426.0), at(582.0, 450.0)), "Cancel", false, true) {
            result = Some(false);
        }
    });
    if eyedropper {
        app.ts.cancel();
        app.ts.picked_color = None;
        app.ts.pick_target = Some(crate::tools::PickTarget::SampleColor);
        sc.sampling = true;
        return;
    }
    match result {
        Some(true) => {
            let Some(sc) = st.select.take() else { return };
            let name = if sc.name.trim().is_empty() { sc.color.to_hex() } else { sc.name.trim().to_string() };
            let colors = &mut st.edits[st.cur].palette.colors;
            match sc.purpose {
                Purpose::Edit(i) => {
                    if let Some(nc) = colors.get_mut(i) {
                        nc.color = sc.color;
                        nc.name = name;
                    }
                }
                Purpose::Add => {
                    let at = st.sel.map(|i| (i + 1).min(colors.len())).unwrap_or(colors.len());
                    colors.insert(at, NamedColor::new(name, sc.color));
                    st.sel = Some(at);
                }
            }
        }
        Some(false) => st.select = None,
        None if dismissed => st.select = None,
        None => {}
    }
}
