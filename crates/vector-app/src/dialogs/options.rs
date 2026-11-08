//! Tools > Options dialog (806×681): category buttons, page list and pages, exactly as in the
//! reference screenshots. Edits apply live; Cancel restores the settings from when it opened.

use super::{heading, note, separator};
use crate::app::VectorApp;
use crate::commands::{Cmd, OptionsPage};
use crate::icons::Tone;
use crate::settings::{ButtonSize, MenuBarMode, NodeSize, Settings, StartupAction, WheelAction};
use crate::theme;
use crate::tools::ToolId;
use egui::{pos2, vec2, Align2, Color32, FontId, Id, Rect, Sense, Stroke, Ui, UiBuilder};
use vector_core::document::{DocInfo, DocSettings, GridKind};
use vector_core::{Color, Guide, GuideKind, Unit};

pub const SIZE: egui::Vec2 = egui::vec2(806.0, 681.0);

/// Document-category edits (applied to the active document on OK).
#[derive(Clone)]
pub struct DocEdit {
    pub settings: DocSettings,
    pub width: f64,
    pub height: f64,
    pub background: Option<Color>,
    pub info: DocInfo,
    pub guides: Vec<Guide>,
    pub all_pages: bool,
}

#[derive(Default)]
pub struct OptionsState {
    pub open: bool,
    /// Requested/selected page name.
    pub page: Option<String>,
    pub category: Option<OptionsPage>,
    pub backup: Option<Settings>,
    pub doc_edit: Option<DocEdit>,
    pub tab: usize,
    pub replace_from: String,
    pub replace_with: String,
    pub replace_sel: Option<usize>,
    pub commands_menu: usize,
    pub commands_search: String,
    pub commands_sel: Option<Cmd>,
    pub commands_tab: usize,
    pub capture: Option<String>,
    pub catalog: Vec<(usize, Vec<crate::menus::CatalogEntry>)>,
    pub plugin_sel: Option<usize>,
    pub file_loc_sel: Option<usize>,
    pub workspace_sel: Option<usize>,
    pub guide_pos: f64,
    pub guide_sel: Option<usize>,
    pub pen_strokes: Vec<Vec<egui::Pos2>>,
    pub security_dialog: bool,
    pub zoom_calibrate: bool,
    pub new_cmd_bar: String,
}

const CATEGORIES: [(OptionsPage, &str, &str); 6] = [
    (OptionsPage::Document, "opt_document", "Document"),
    (OptionsPage::App, "opt_app", "Vector"),
    (OptionsPage::Customization, "opt_custom", "Customization"),
    (OptionsPage::Tools, "opt_tools", "Tools"),
    (OptionsPage::Global, "opt_global", "Global"),
    (OptionsPage::Workspaces, "opt_workspaces", "Workspaces"),
];

fn title_of(c: OptionsPage) -> &'static str {
    CATEGORIES.iter().find(|x| x.0 == c).map(|x| x.2).unwrap_or("Vector")
}

/// Pages per category (name, optional tool icon).
pub fn pages(c: OptionsPage) -> &'static [(&'static str, Option<ToolId>)] {
    match c {
        OptionsPage::Document => &[("General", None), ("Page Size", None), ("Layout", None), ("Background", None), ("Rulers", None), ("Grid", None), ("Guidelines", None), ("Save", None)],
        OptionsPage::App => &[
            ("General", None),
            ("Tablet Mode", None),
            ("Display", None),
            ("Edit", None),
            ("Nodes and Handles", None),
            ("PowerClip", None),
            ("PowerTRACE", None),
            ("Snapping", None),
            ("Save", None),
            ("Text", None),
            ("Pen Settings", None),
            ("Warnings", None),
            ("Scripts", None),
            ("Plug-ins", None),
        ],
        OptionsPage::Customization => &[("Appearance", None), ("Command Bars", None), ("Commands", None), ("Color Palette", None)],
        OptionsPage::Tools => &[
            ("Pick", Some(ToolId::Pick)),
            ("Eraser", Some(ToolId::Eraser)),
            ("Zoom/Pan", Some(ToolId::Zoom)),
            ("Freehand/Bézier", Some(ToolId::Freehand)),
            ("Smart Drawing", Some(ToolId::SmartDrawing)),
            ("Rectangle", Some(ToolId::Rectangle)),
            ("Ellipse", Some(ToolId::Ellipse)),
            ("Graph Paper", Some(ToolId::GraphPaper)),
            ("Spiral", Some(ToolId::Spiral)),
            ("Table", Some(ToolId::Table)),
            ("Dimension", Some(ToolId::ParallelDim)),
            ("Angular Dimension", Some(ToolId::AngularDim)),
            ("Connector", Some(ToolId::StraightConnector)),
            ("Mesh Fill", Some(ToolId::MeshFill)),
        ],
        OptionsPage::Global => &[("General", None), ("Printing", None), ("File Locations", None), ("File Formats", None), ("Color", None), ("User ID", None), ("Privacy", None)],
        OptionsPage::Workspaces => &[("Workspaces", None)],
    }
}

/// The default "Show warnings when:" list (all enabled, alphabetical).
pub fn default_warnings() -> Vec<(String, bool)> {
    [
        "Adding folders that already exist as subfolders in the assets list",
        "Adding folders that already exist in the assets list",
        "Adding folders to the assets list as parent folders for existing folders",
        "Applying effects to text with embedded graphics (Blend)",
        "Applying effects to text with embedded graphics (Contour)",
        "Applying effects to text with embedded graphics (Convert to Curves)",
        "Applying effects to text with embedded graphics (Distortion)",
        "Applying effects to text with embedded graphics (Envelope)",
        "Applying effects to text with embedded graphics (Extrude)",
        "Applying effects to text with embedded graphics (Lens)",
        "Applying lens effects to duotones",
        "Applying Painterly brushstrokes to incompatible objects",
        "Artistic Media Brush contains invalid objects",
        "Breaking apart objects that mix spot colors",
        "Brushstrokes exceed the document rendering resolution",
        "Changing the document color settings",
        "Closing a document that contains unsaved changes",
        "Combining objects with different fill types",
        "Converting a bitmap that contains transparency",
        "Converting objects with effects to curves",
        "Converting paragraph text that overflows its frame",
        "Copying objects that contain linked bitmaps",
        "Cropping objects that contain effects",
        "Deleting a layer that contains objects",
        "Deleting a page that contains objects",
        "Deleting a symbol that is used in the document",
        "Deleting color styles that are used in the document",
        "Deleting guidelines on a locked layer",
        "Deleting object styles that are used in the document",
        "Discarding changes made in the Script Editor",
        "Editing a locked object",
        "Embedding fonts that do not allow embedding",
        "Exporting a document that contains RGB and CMYK colors",
        "Exporting a page with no objects",
        "Exporting bitmaps with a resolution lower than 72 dpi",
        "Exporting files with font substitutions",
        "Exporting text as curves",
        "Fitting text to a path with an envelope",
        "Grouping objects on different layers",
        "Importing a file with missing fonts",
        "Importing a large bitmap",
        "Importing files that contain spot colors",
        "Importing SVG files with unsupported elements",
        "Joining curves that are far apart",
        "Linking a bitmap that cannot be found",
        "Opening a document created in a newer version",
        "Opening a document that contains missing fonts",
        "Opening a document that is already open",
        "Opening a read-only document",
        "Overwriting an existing file when exporting",
        "Pasting objects from a document with a different color mode",
        "PowerClip contents extend beyond the frame",
        "Printing a document with missing links",
        "Printing to a printer that does not support the page size",
        "Publishing to PDF with font substitutions",
        "Removing a palette that is in use",
        "Resampling a bitmap to a higher resolution",
        "Reverting to the last saved version",
        "Running a script from an untrusted location",
        "Saving a document with unsupported features to an older version",
        "Saving a template over an existing template",
        "Saving with font embedding restrictions",
        "Shaping objects that contain effects",
        "Spell checking a document with no text",
        "Text exceeds the maximum length for a single text object",
        "Trimming objects with effects",
        "Ungrouping objects that contain effects",
        "Using a large number of steps in a blend",
        "Using too many nodes in a single curve",
        "Welding objects with different outlines",
    ]
    .iter()
    .map(|s| (s.to_string(), true))
    .collect()
}

fn find_category(name: &str, prefer: OptionsPage) -> Option<OptionsPage> {
    if pages(prefer).iter().any(|p| p.0 == name) {
        return Some(prefer);
    }
    CATEGORIES.iter().map(|c| c.0).find(|c| pages(*c).iter().any(|p| p.0 == name))
}

impl DocEdit {
    fn from_app(app: &VectorApp) -> Option<DocEdit> {
        let d = app.doc()?;
        let p = d.page_ref();
        Some(DocEdit {
            settings: d.doc.settings.clone(),
            width: p.width,
            height: p.height,
            background: p.background,
            info: d.doc.info.clone(),
            guides: p.guides.clone(),
            all_pages: true,
        })
    }
}

pub fn show(app: &mut VectorApp, ctx: &egui::Context) {
    // first frame: snapshot for Cancel
    if app.dialogs.options.backup.is_none() {
        app.dialogs.options.open = true;
        app.dialogs.options.backup = Some(app.settings.clone());
        app.dialogs.options.doc_edit = DocEdit::from_app(app);
        app.dialogs.options.tab = 0;
    }
    let st = &mut app.dialogs.options;
    let mut cat = st.category.unwrap_or(OptionsPage::App);
    if cat == OptionsPage::Document && st.doc_edit.is_none() {
        cat = OptionsPage::App;
    }
    let page = match st.page.clone() {
        Some(p) => match find_category(&p, cat) {
            Some(c) => {
                cat = c;
                p
            }
            None => pages(cat)[0].0.to_string(),
        },
        None => pages(cat)[0].0.to_string(),
    };
    st.category = Some(cat);
    st.page = Some(page.clone());

    let mut result = (false, false);
    let ((), dismissed) = super::window(ctx, "options", "Options", SIZE, |ui| {
        result = body(app, ui, cat, &page);
    });
    let (ok, cancel) = result;
    if ok {
        apply_doc_edit(app);
        app.settings.save();
        close(app, false);
    } else if cancel || dismissed {
        close(app, true);
    }
}

fn close(app: &mut VectorApp, restore: bool) {
    let st = std::mem::take(&mut app.dialogs.options);
    if restore {
        if let Some(b) = st.backup {
            app.settings = b;
            ctx_zoom_reset(app);
        }
    }
}

fn ctx_zoom_reset(_app: &mut VectorApp) {}

fn apply_doc_edit(app: &mut VectorApp) {
    let Some(e) = app.dialogs.options.doc_edit.clone() else { return };
    let Some(d) = app.doc_mut() else { return };
    let cur = d.page;
    let changed = d.doc.settings != e.settings
        || d.doc.info != e.info
        || d.page_ref().width != e.width
        || d.page_ref().height != e.height
        || d.page_ref().background != e.background
        || d.page_ref().guides != e.guides;
    if !changed {
        return;
    }
    d.modify("Document Options", |doc, _| {
        doc.settings = e.settings.clone();
        doc.info = e.info.clone();
        for (i, p) in doc.pages.iter_mut().enumerate() {
            if i == cur || e.all_pages {
                p.width = e.width;
                p.height = e.height;
                p.background = e.background;
            }
        }
        doc.pages[cur].guides = e.guides.clone();
    });
    d.cache.key = 0;
}

/// Lays out header, page list, page and buttons. Returns (ok, cancel).
fn body(app: &mut VectorApp, ui: &mut Ui, cat: OptionsPage, page: &str) -> (bool, bool) {
    let r = ui.max_rect();
    let ctx = ui.ctx().clone();
    // header: category title + category buttons
    ui.painter().text(pos2(r.left(), r.top() + 21.0), Align2::LEFT_CENTER, title_of(cat), theme::bold(13.0), theme::TEXT);
    for (k, (c, icon, name)) in CATEGORIES.iter().enumerate() {
        let cx = r.right() - 12.0 - 32.0 * (5 - k) as f32;
        let br = Rect::from_center_size(pos2(cx, r.top() + 21.0), vec2(24.0, 24.0));
        let enabled = *c != OptionsPage::Document || app.has_doc();
        let resp = ui.interact(br, Id::unique(("optcat", k)), if enabled { Sense::click() } else { Sense::hover() }).on_hover_text(*name);
        if *c == cat {
            ui.painter().rect_filled(br, 3.0, theme::ACCENT);
        } else if resp.hovered() && enabled {
            ui.painter().rect_filled(br, 3.0, theme::HOVER);
        }
        let tone = if !enabled {
            Tone::Disabled
        } else if *c == cat {
            Tone::Bright
        } else {
            Tone::Normal
        };
        if let Some(t) = app.icons.get(&ctx, icon, 18.0, tone) {
            ui.painter().image(t.id, Rect::from_center_size(br.center(), vec2(18.0, 18.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        if resp.clicked() && *c != cat {
            app.dialogs.options.category = Some(*c);
            app.dialogs.options.page = None;
            app.dialogs.options.tab = 0;
        }
    }
    // left list
    let list = Rect::from_min_max(pos2(r.left(), r.top() + 41.0), pos2(r.left() + 178.0, r.bottom() - 33.0));
    ui.painter().rect_stroke(list, 0.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Inside);
    for (i, (name, tool)) in pages(cat).iter().enumerate() {
        let row = Rect::from_min_size(pos2(list.left() + 1.0, list.top() + 1.0 + 25.0 * i as f32), vec2(list.width() - 2.0, 25.0));
        let resp = ui.interact(row, Id::unique(("optpage", cat as u8, i)), Sense::click());
        if *name == page {
            ui.painter().rect_filled(row, 0.0, Color32::from_rgb(0x3a, 0x3a, 0x3a));
        } else if resp.hovered() {
            ui.painter().rect_filled(row, 0.0, Color32::from_rgb(0x31, 0x31, 0x31));
        }
        let mut tx = row.left() + 3.0;
        if let Some(t) = tool {
            if let Some(tex) = app.icons.get(&ctx, crate::tools::info(*t).icon, 18.0, Tone::Normal) {
                ui.painter().image(tex.id, Rect::from_center_size(pos2(row.left() + 15.0, row.center().y), vec2(18.0, 18.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            tx = row.left() + 31.0;
        }
        ui.painter().text(pos2(tx, row.center().y), Align2::LEFT_CENTER, *name, FontId::proportional(13.0), theme::TEXT);
        if resp.clicked() && *name != page {
            app.dialogs.options.page = Some(name.to_string());
            app.dialogs.options.tab = 0;
        }
    }
    // page panel
    let panel = Rect::from_min_max(pos2(r.left() + 188.0, list.top()), pos2(r.right(), list.bottom()));
    ui.painter().rect_stroke(panel, 0.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Inside);
    let tabbed = matches!((cat, page), (OptionsPage::App, "Text") | (OptionsPage::Global, "Printing"));
    let inner = if tabbed { panel.shrink2(vec2(6.0, 10.0)) } else { Rect::from_min_max(panel.min + vec2(15.0, 14.0), panel.max - vec2(8.0, 8.0)) };
    let mut child = ui.new_child(UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(inner.expand(1.0));
    child.spacing_mut().item_spacing.y = 7.0;
    page_ui(app, &mut child, cat, page);
    // buttons
    super::ok_cancel_row(ui, "OK", true)
}

fn page_ui(app: &mut VectorApp, ui: &mut Ui, cat: OptionsPage, page: &str) {
    let id = format!("optscroll_{cat:?}_{page}");
    let scroll_pages = matches!(page, "Display" | "Nodes and Handles" | "PowerClip" | "Snapping" | "Save" | "Commands" | "Command Bars" | "Appearance");
    if scroll_pages {
        egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 14.0);
            page_content(app, ui, cat, page);
        });
    } else {
        page_content(app, ui, cat, page);
    }
}

fn page_content(app: &mut VectorApp, ui: &mut Ui, cat: OptionsPage, page: &str) {
    match cat {
        OptionsPage::Document => doc_page(app, ui, page),
        OptionsPage::App => match page {
            "General" => app_general(app, ui),
            "Tablet Mode" => tablet(app, ui),
            "Display" => display(app, ui),
            "Edit" => edit(app, ui),
            "Nodes and Handles" => nodes(app, ui),
            "PowerClip" => powerclip(app, ui),
            "PowerTRACE" => powertrace(app, ui),
            "Snapping" => snapping(app, ui),
            "Save" => save(app, ui),
            "Text" => text(app, ui),
            "Pen Settings" => pen(app, ui),
            "Warnings" => warnings(app, ui),
            "Scripts" => scripts(app, ui),
            _ => plugins(app, ui),
        },
        OptionsPage::Customization => match page {
            "Appearance" => appearance(app, ui),
            "Command Bars" => command_bars(app, ui),
            "Commands" => commands(app, ui),
            _ => color_palette(app, ui),
        },
        OptionsPage::Tools => tool_page(app, ui, page),
        OptionsPage::Global => match page {
            "General" => global_general(app, ui),
            "Printing" => printing(app, ui),
            "File Locations" => file_locations(app, ui),
            "File Formats" => file_formats(ui),
            "Color" => global_color(app, ui),
            "User ID" => user_id(app, ui),
            _ => privacy(app, ui),
        },
        OptionsPage::Workspaces => workspaces(app, ui),
    }
}

// ------------------------------------------------------------------------------------------
// small widgets
// ------------------------------------------------------------------------------------------

fn label_w(ui: &mut Ui, text: &str, w: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    ui.painter().text(pos2(r.left(), r.center().y), Align2::LEFT_CENTER, text, FontId::proportional(13.0), theme::TEXT);
}

fn label_right(ui: &mut Ui, text: &str, w: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    ui.painter().text(pos2(r.right() - 8.0, r.center().y), Align2::RIGHT_CENTER, text, FontId::proportional(13.0), theme::TEXT);
}

fn combo(ui: &mut Ui, id: &str, cur: &mut usize, items: &[&str], w: f32) -> bool {
    let before = *cur;
    let i = (*cur).min(items.len().saturating_sub(1));
    egui::ComboBox::from_id_salt(id).selected_text(items.get(i).copied().unwrap_or("")).width(w).show_ui(ui, |ui| {
        for (k, it) in items.iter().enumerate() {
            ui.selectable_value(cur, k, *it);
        }
    });
    before != *cur
}

fn combo_str(ui: &mut Ui, id: &str, cur: &mut String, items: &[&str], w: f32) {
    egui::ComboBox::from_id_salt(id).selected_text(cur.as_str()).width(w).show_ui(ui, |ui| {
        for it in items {
            ui.selectable_value(cur, it.to_string(), *it);
        }
    });
}

fn combo_u8(ui: &mut Ui, id: &str, cur: &mut u8, items: &[&str], w: f32) {
    let mut c = *cur as usize;
    if combo(ui, id, &mut c, items, w) {
        *cur = c as u8;
    }
}

fn spin(ui: &mut Ui, v: &mut f64, range: std::ops::RangeInclusive<f64>, suffix: &str, decimals: usize) -> bool {
    ui.add_sized(vec2(70.0, 22.0), egui::DragValue::new(v).range(range).suffix(suffix).fixed_decimals(decimals).speed(0.1)).changed()
}

fn spin_u32(ui: &mut Ui, v: &mut u32, range: std::ops::RangeInclusive<u32>) -> bool {
    ui.add_sized(vec2(60.0, 22.0), egui::DragValue::new(v).range(range)).changed()
}

fn spin_usize(ui: &mut Ui, v: &mut usize, range: std::ops::RangeInclusive<usize>) -> bool {
    ui.add_sized(vec2(60.0, 22.0), egui::DragValue::new(v).range(range)).changed()
}

fn unit_spin(ui: &mut Ui, id: &str, v: &mut f64, unit: Unit, dpi: f64) -> bool {
    crate::widgets::unit_field(ui, Id::unique(id), v, unit, dpi, 80.0, true)
}

fn reset(ui: &mut Ui) -> bool {
    ui.add(egui::Button::new("Reset").min_size(vec2(60.0, 22.0))).clicked()
}

fn rgb_edit(ui: &mut Ui, c: &mut [u8; 3]) -> bool {
    let mut c32 = Color32::from_rgb(c[0], c[1], c[2]);
    let ch = egui::color_picker::color_edit_button_srgba(ui, &mut c32, egui::color_picker::Alpha::Opaque).changed();
    if ch {
        *c = [c32.r(), c32.g(), c32.b()];
    }
    ch
}

/// Tab strip in the dialog style (selected tab blue).
fn tabs(ui: &mut Ui, cur: &mut usize, names: &[&str]) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, n) in names.iter().enumerate() {
            let sel = *cur == i;
            let b = egui::Button::new(egui::RichText::new(*n).color(if sel { Color32::WHITE } else { theme::TEXT }))
                .fill(if sel { theme::ACCENT } else { Color32::from_rgb(0x33, 0x33, 0x33) })
                .corner_radius(egui::CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 })
                .min_size(vec2(64.0, 24.0));
            if ui.add(b).clicked() {
                *cur = i;
            }
        }
    });
}

/// The bordered content area below a tab strip.
fn tab_body(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let r = Rect::from_min_max(ui.cursor().min - vec2(0.0, 7.0), ui.max_rect().max);
    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Inside);
    let inner = r.shrink2(vec2(24.0, 10.0));
    let mut child = ui.new_child(UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(inner);
    child.spacing_mut().item_spacing.y = 7.0;
    egui::ScrollArea::vertical().id_salt(("tabbody", ui.scope_id())).auto_shrink([false, false]).show(&mut child, |ui| {
        ui.set_width(ui.available_width() - 14.0);
        add(ui)
    });
}

fn unit_dpi(app: &VectorApp) -> (Unit, f64) {
    app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0))
}

const UNIT_NAMES: [&str; 11] = ["inches", "millimeters", "centimeters", "meters", "points", "picas", "pixels", "feet", "yards", "ciceros", "didots"];

fn unit_combo(ui: &mut Ui, id: &str, u: &mut Unit) {
    let mut i = Unit::ALL.iter().position(|x| x == u).unwrap_or(0);
    if combo(ui, id, &mut i, &UNIT_NAMES, 140.0) {
        *u = Unit::ALL[i];
    }
}

// ------------------------------------------------------------------------------------------
// Document pages
// ------------------------------------------------------------------------------------------

fn doc_page(app: &mut VectorApp, ui: &mut Ui, page: &str) {
    let Some(mut e) = app.dialogs.options.doc_edit.clone() else {
        note(ui, "Open a document to edit its options.");
        return;
    };
    let (u, dpi) = (e.settings.unit, e.settings.dpi);
    match page {
        "General" => {
            ui.horizontal(|ui| {
                label_w(ui, "Title:", 110.0);
                ui.add_sized(vec2(360.0, 22.0), egui::TextEdit::singleline(&mut e.info.title));
            });
            ui.horizontal(|ui| {
                label_w(ui, "Subject:", 110.0);
                ui.add_sized(vec2(360.0, 22.0), egui::TextEdit::singleline(&mut e.info.subject));
            });
            ui.horizontal(|ui| {
                label_w(ui, "Author:", 110.0);
                ui.add_sized(vec2(360.0, 22.0), egui::TextEdit::singleline(&mut e.info.author));
            });
            ui.horizontal(|ui| {
                label_w(ui, "Keywords:", 110.0);
                ui.add_sized(vec2(360.0, 22.0), egui::TextEdit::singleline(&mut e.info.keywords));
            });
            ui.horizontal(|ui| {
                label_w(ui, "Notes:", 110.0);
                ui.add_sized(vec2(360.0, 80.0), egui::TextEdit::multiline(&mut e.info.notes));
            });
            separator(ui);
            heading(ui, "Units");
            ui.horizontal(|ui| {
                label_w(ui, "Drawing units:", 110.0);
                unit_combo(ui, "doc_unit", &mut e.settings.unit);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Rendering resolution:", 140.0);
                spin(ui, &mut e.settings.dpi, 72.0..=2400.0, " dpi", 0);
            });
        }
        "Page Size" => {
            let presets = vector_core::units::page_presets();
            let names: Vec<&str> = presets.iter().map(|p| p.0).collect();
            let mut cur = presets.iter().position(|p| ((p.1 - e.width).abs() < 0.5 && (p.2 - e.height).abs() < 0.5) || ((p.2 - e.width).abs() < 0.5 && (p.1 - e.height).abs() < 0.5)).unwrap_or(names.len() - 1);
            ui.horizontal(|ui| {
                label_w(ui, "Size:", 110.0);
                if combo(ui, "doc_size", &mut cur, &names, 200.0) {
                    let land = e.width > e.height;
                    let (w, h) = (presets[cur].1, presets[cur].2);
                    if land {
                        e.width = h;
                        e.height = w;
                    } else {
                        e.width = w;
                        e.height = h;
                    }
                }
            });
            ui.horizontal(|ui| {
                label_w(ui, "Width:", 110.0);
                unit_spin(ui, "doc_w", &mut e.width, u, dpi);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Height:", 110.0);
                unit_spin(ui, "doc_h", &mut e.height, u, dpi);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Orientation:", 110.0);
                let land = e.width > e.height;
                if ui.radio(!land, "Portrait").clicked() && land {
                    std::mem::swap(&mut e.width, &mut e.height);
                }
                if ui.radio(land, "Landscape").clicked() && !land {
                    std::mem::swap(&mut e.width, &mut e.height);
                }
            });
            ui.checkbox(&mut e.all_pages, "Apply size to all pages");
            separator(ui);
            ui.horizontal(|ui| {
                ui.checkbox(&mut e.settings.show_bleed, "Show bleed area");
            });
            ui.horizontal(|ui| {
                label_w(ui, "Bleed:", 110.0);
                unit_spin(ui, "doc_bleed", &mut e.settings.bleed, u, dpi);
            });
        }
        "Layout" => {
            ui.checkbox(&mut e.settings.facing_pages, "Facing pages");
            note(ui, "Facing pages displays left and right pages side by side, as in a book or magazine.");
            separator(ui);
            heading(ui, "Nudge");
            ui.horizontal(|ui| {
                label_w(ui, "Nudge:", 130.0);
                unit_spin(ui, "doc_nudge", &mut e.settings.nudge, u, dpi);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Super nudge:", 130.0);
                spin(ui, &mut e.settings.super_nudge_mult, 1.0..=100.0, " ×", 1);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Micro nudge:", 130.0);
                spin(ui, &mut e.settings.micro_nudge_div, 1.0..=100.0, " ÷", 1);
            });
            separator(ui);
            heading(ui, "Duplicate placement");
            ui.horizontal(|ui| {
                label_w(ui, "Horizontal offset:", 130.0);
                unit_spin(ui, "doc_dupx", &mut e.settings.duplicate_offset.x, u, dpi);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Vertical offset:", 130.0);
                unit_spin(ui, "doc_dupy", &mut e.settings.duplicate_offset.y, u, dpi);
            });
            separator(ui);
            ui.checkbox(&mut e.settings.treat_as_filled, "Treat all objects as filled");
        }
        "Background" => {
            let mut solid = e.background.is_some();
            if ui.radio(!solid, "No background").clicked() {
                solid = false;
                e.background = None;
            }
            ui.horizontal(|ui| {
                if ui.radio(solid, "Solid").clicked() && !solid {
                    e.background = Some(Color::WHITE);
                }
                if let Some(c) = e.background.as_mut() {
                    crate::dockers::props::color_edit(ui, c, false);
                }
            });
            ui.checkbox(&mut e.all_pages, "Apply to all pages");
            note(ui, "The background is printed and exported with the page.");
        }
        "Rulers" => {
            ui.checkbox(&mut e.settings.show_rulers, "Show rulers");
            separator(ui);
            heading(ui, "Units");
            ui.horizontal(|ui| {
                label_w(ui, "Horizontal:", 110.0);
                unit_combo(ui, "ruler_unit", &mut e.settings.unit);
            });
            separator(ui);
            heading(ui, "Origin");
            ui.horizontal(|ui| {
                label_w(ui, "Horizontal:", 110.0);
                unit_spin(ui, "ruler_ox", &mut e.settings.ruler_origin.x, u, dpi);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Vertical:", 110.0);
                unit_spin(ui, "ruler_oy", &mut e.settings.ruler_origin.y, u, dpi);
            });
            separator(ui);
            heading(ui, "Drawing scale");
            ui.horizontal(|ui| {
                label_w(ui, "Scale (1 : n):", 110.0);
                spin(ui, &mut e.settings.drawing_scale, 0.001..=100000.0, "", 3);
            });
        }
        "Grid" => {
            ui.checkbox(&mut e.settings.show_grid, "Show grid");
            ui.checkbox(&mut e.settings.snap_to_grid, "Snap to grid");
            ui.checkbox(&mut e.settings.pixel_grid, "Show pixel grid");
            separator(ui);
            heading(ui, "Document grid");
            ui.horizontal(|ui| {
                label_w(ui, "Horizontal spacing:", 140.0);
                unit_spin(ui, "grid_x", &mut e.settings.grid_spacing.x, u, dpi);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Vertical spacing:", 140.0);
                unit_spin(ui, "grid_y", &mut e.settings.grid_spacing.y, u, dpi);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Display as:", 140.0);
                ui.radio_value(&mut e.settings.grid_kind, GridKind::Lines, "Lines");
                ui.radio_value(&mut e.settings.grid_kind, GridKind::Dots, "Dots");
            });
            ui.horizontal(|ui| {
                label_w(ui, "Color:", 140.0);
                crate::dockers::props::color_edit(ui, &mut e.settings.grid_color, false);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Opacity:", 140.0);
                ui.add(egui::Slider::new(&mut e.settings.grid_opacity, 0.0..=100.0).suffix(" %"));
            });
        }
        "Guidelines" => {
            ui.checkbox(&mut e.settings.show_guides, "Show guidelines");
            ui.checkbox(&mut e.settings.snap_to_guides, "Snap to guidelines");
            separator(ui);
            let st = &mut app.dialogs.options;
            ui.horizontal(|ui| {
                label_w(ui, "Position:", 80.0);
                unit_spin(ui, "guide_pos", &mut st.guide_pos, u, dpi);
                if ui.button("Add horizontal").clicked() {
                    e.guides.push(Guide { kind: GuideKind::Horizontal, pos: kurbo::Point::new(0.0, st.guide_pos), locked: false, color: Color::rgb(0, 153, 255) });
                }
                if ui.button("Add vertical").clicked() {
                    e.guides.push(Guide { kind: GuideKind::Vertical, pos: kurbo::Point::new(st.guide_pos, 0.0), locked: false, color: Color::rgb(0, 153, 255) });
                }
            });
            egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(4.0).show(ui, |ui| {
                ui.set_min_size(vec2(400.0, 200.0));
                egui::ScrollArea::vertical().id_salt("guides_list").max_height(200.0).show(ui, |ui| {
                    for (i, g) in e.guides.iter().enumerate() {
                        let text = match g.kind {
                            GuideKind::Horizontal => format!("Horizontal  {}", u.format(g.pos.y, dpi)),
                            GuideKind::Vertical => format!("Vertical  {}", u.format(g.pos.x, dpi)),
                            GuideKind::Angled(a) => format!("Angled {a:.1}°  ({}, {})", u.format(g.pos.x, dpi), u.format(g.pos.y, dpi)),
                        };
                        if ui.selectable_label(st.guide_sel == Some(i), text).clicked() {
                            st.guide_sel = Some(i);
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(st.guide_sel.is_some(), egui::Button::new("Delete")).clicked() {
                    if let Some(i) = st.guide_sel.take() {
                        if i < e.guides.len() {
                            e.guides.remove(i);
                        }
                    }
                }
                if ui.button("Clear").clicked() {
                    e.guides.clear();
                    st.guide_sel = None;
                }
            });
        }
        _ => {
            // Save
            heading(ui, "Save options");
            let sv = &mut app.settings.save;
            ui.horizontal(|ui| {
                ui.label("Optimize for:");
                ui.radio_value(&mut sv.optimize_for_size, false, "Speed");
                ui.radio_value(&mut sv.optimize_for_size, true, "Size");
            });
            ui.checkbox(&mut sv.save_presentation_attributes, "Save presentation attributes (views)");
            ui.checkbox(&mut sv.bitmap_compression, "Use bitmap compression");
            ui.checkbox(&mut sv.object_compression, "Use graphic object compression");
            ui.checkbox(&mut sv.save_thumbnail, "Save thumbnail");
            ui.checkbox(&mut sv.embed_color_profiles, "Embed color profiles");
            ui.checkbox(&mut app.settings.embed_fonts, "Embed fonts when saving files");
            note(ui, "Drawings are saved as .cdr files: a compressed container with the drawing, its images, metadata (title, subject, rating), a thumbnail and, optionally, the fonts and color profiles it uses.");
        }
    }
    app.dialogs.options.doc_edit = Some(e);
}

// ------------------------------------------------------------------------------------------
// Vector (application) pages
// ------------------------------------------------------------------------------------------

fn app_general(app: &mut VectorApp, ui: &mut Ui) {
    let s = &mut app.settings;
    heading(ui, "Getting started");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        label_w(ui, "On Vector start-up:", 150.0);
        let names: Vec<&str> = StartupAction::ALL.iter().map(|a| a.name()).collect();
        let mut i = StartupAction::ALL.iter().position(|a| *a == s.startup).unwrap_or(0);
        if combo(ui, "startup", &mut i, &names, 190.0) {
            s.startup = StartupAction::ALL[i];
        }
    });
    ui.add_space(4.0);
    ui.checkbox(&mut s.show_new_doc_dialog, "Show New Document dialog box");
    separator(ui);
    heading(ui, "Undo levels");
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.label("Regular:");
        if spin_usize(ui, &mut s.undo_levels, 1..=99999) {
            let n = s.undo_levels;
            for d in app.docs.iter_mut() {
                d.history.limit = n;
            }
        }
    });
}

fn tablet(app: &mut VectorApp, ui: &mut Ui) {
    let s = &mut app.settings;
    ui.checkbox(&mut s.auto_tablet, "Auto - switch between Tablet and Desktop workspaces");
    ui.add_space(6.0);
    ui.label("When switching to:");
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        label_w(ui, "Desktop mode", 110.0);
        combo_str(ui, "ws_desktop", &mut s.tablet_desktop_ws, &["Default", "Lite", "Touch"], 180.0);
    });
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        label_w(ui, "Tablet mode", 110.0);
        combo_str(ui, "ws_tablet", &mut s.tablet_touch_ws, &["Touch", "Default", "Lite"], 180.0);
    });
}

fn display(app: &mut VectorApp, ui: &mut Ui) {
    let s = &mut app.settings;
    ui.checkbox(&mut s.proof_colors, "Proof colors");
    separator(ui);
    heading(ui, "Other controls");
    egui::Grid::new("disp_other").num_columns(2).spacing(vec2(30.0, 7.0)).show(ui, |ui| {
        ui.checkbox(&mut s.interruptible_refresh, "Interruptible refresh");
        ui.checkbox(&mut s.manual_refresh, "Manual refresh");
        ui.end_row();
        ui.checkbox(&mut s.auto_panning, "Auto-panning");
        ui.checkbox(&mut s.show_tooltips, "Show tooltips");
        ui.end_row();
        ui.checkbox(&mut s.antialias_bitmaps, "Antialias bitmaps in enhanced view");
        ui.checkbox(&mut s.hide_bbox_curve_tools, "Hide bounding box for curve tools");
        ui.end_row();
        ui.checkbox(&mut s.highlight_selected, "Highlight outline for selected objects");
        ui.end_row();
    });
    separator(ui);
    heading(ui, "Full-screen preview");
    ui.horizontal(|ui| {
        ui.radio_value(&mut s.fullscreen_enhanced, false, "Use normal view");
        ui.add_space(20.0);
        ui.radio_value(&mut s.fullscreen_enhanced, true, "Use enhanced view");
    });
    ui.checkbox(&mut s.fullscreen_page_border, "Show page border");
    separator(ui);
    heading(ui, "Navigation");
    ui.horizontal(|ui| {
        label_right(ui, "Default action for mouse wheel:", 250.0);
        let mut i = if s.wheel_action == WheelAction::Zoom { 0 } else { 1 };
        if combo(ui, "wheel", &mut i, &["Zoom", "Scroll"], 120.0) {
            s.wheel_action = if i == 0 { WheelAction::Zoom } else { WheelAction::Scroll };
        }
    });
    ui.horizontal(|ui| {
        label_right(ui, "Zoom rate:", 250.0);
        spin(ui, &mut s.zoom_rate, 1.0..=100.0, " %", 0);
        if reset(ui) {
            s.zoom_rate = 15.0;
        }
    });
    ui.horizontal(|ui| {
        label_right(ui, "Alternate Zoom (Ctrl+Shift) rate:", 250.0);
        spin(ui, &mut s.alt_zoom_rate, 1.0..=500.0, " %", 0);
        if reset(ui) {
            s.alt_zoom_rate = 100.0;
        }
    });
}

fn edit(app: &mut VectorApp, ui: &mut Ui) {
    let s = &mut app.settings;
    ui.horizontal(|ui| {
        label_w(ui, "Constrain angle:", 130.0);
        spin(ui, &mut s.constrain_angle, 0.1..=90.0, "", 1);
        ui.label("degrees");
    });
    ui.horizontal(|ui| {
        label_w(ui, "Drawing precision:", 130.0);
        spin_u32(ui, &mut s.drawing_precision, 0..=6);
        ui.label("decimal places");
    });
    ui.checkbox(&mut s.dblclick_bitmap_editor, "Double-click to edit bitmaps in an image editor");
    separator(ui);
    heading(ui, "Visibility of inactive objects");
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut s.inactive_visibility, 0.0..=100.0).show_value(false));
        spin(ui, &mut s.inactive_visibility, 0.0..=100.0, " %", 0);
        if reset(ui) {
            s.inactive_visibility = 20.0;
        }
    });
    note(ui, "When editing objects, this setting controls the visibility of objects that are not currently active.");
    separator(ui);
    ui.checkbox(&mut s.dblclick_focus_mode, "Double-click to activate Focus Mode");
}

fn nodes(app: &mut VectorApp, ui: &mut Ui) {
    let s = &mut app.settings;
    ui.horizontal(|ui| {
        label_w(ui, "Node size:", 100.0);
        let mut i = match s.node_size {
            NodeSize::Small => 0,
            NodeSize::Medium => 1,
            NodeSize::Large => 2,
        };
        if combo(ui, "node_size", &mut i, &["Small", "Medium", "Large"], 120.0) {
            s.node_size = [NodeSize::Small, NodeSize::Medium, NodeSize::Large][i];
        }
    });
    separator(ui);
    heading(ui, "Node shape");
    ui.horizontal(|ui| {
        ui.label("Cusp:");
        let mut a = 0;
        combo(ui, "ns_cusp", &mut a, &["□"], 40.0);
        ui.add_space(10.0);
        ui.label("Smooth:");
        let mut b = 0;
        combo(ui, "ns_smooth", &mut b, &["○"], 40.0);
        ui.add_space(10.0);
        ui.label("Symmetrical:");
        let mut c = 0;
        combo(ui, "ns_sym", &mut c, &["◇"], 40.0);
        let _ = reset(ui);
    });
    ui.checkbox(&mut s.show_curve_direction, "Show curve direction when editing");
    ui.checkbox(&mut s.node_tracking, "Enable node tracking");
    separator(ui);
    heading(ui, "Colors");
    ui.horizontal(|ui| {
        label_w(ui, "Color scheme:", 120.0);
        let mut i = 0;
        combo(ui, "node_scheme", &mut i, &["Default"], 120.0);
    });
    ui.horizontal(|ui| {
        label_w(ui, "Main color:", 120.0);
        rgb_edit(ui, &mut s.node_main_color);
        if reset(ui) {
            s.node_main_color = [0, 102, 255];
        }
    });
    ui.horizontal(|ui| {
        label_w(ui, "Secondary color:", 120.0);
        rgb_edit(ui, &mut s.node_secondary_color);
        if reset(ui) {
            s.node_secondary_color = [255, 51, 51];
        }
    });
    ui.checkbox(&mut s.show_node_highlight, "Show highlight");
    ui.checkbox(&mut s.node_types_colors, "Show node types in different colors");
}

fn powerclip(app: &mut VectorApp, ui: &mut Ui) {
    let s = &mut app.settings;
    ui.horizontal(|ui| {
        label_w(ui, "Drag content inside PowerClip:", 210.0);
        let mut i = 0;
        combo(ui, "pc_drag", &mut i, &["PowerClip with content"], 200.0);
    });
    ui.radio_value(&mut s.powerclip_drag_mode, 0, "Ignore PowerClip frame");
    ui.radio_value(&mut s.powerclip_drag_mode, 1, "Add content to PowerClip frame");
    ui.radio_value(&mut s.powerclip_drag_mode, 2, "Replace existing content");
    separator(ui);
    heading(ui, "Auto-center new content");
    ui.radio_value(&mut s.powerclip_autocenter, 0, "When content is completely outside frame");
    ui.radio_value(&mut s.powerclip_autocenter, 1, "Always");
    ui.radio_value(&mut s.powerclip_autocenter, 2, "Never");
    note(ui, "New content is centered in the PowerClip frame based on this setting.");
    separator(ui);
    ui.checkbox(&mut s.powerclip_empty_lines, "Show lines in empty PowerClip frames:");
    ui.indent("pc_lines", |ui| {
        ui.add_enabled_ui(s.powerclip_empty_lines, |ui| {
            ui.radio_value(&mut s.powerclip_lines_print, true, "Always (including print and export)");
            ui.radio_value(&mut s.powerclip_lines_print, false, "On screen only");
        });
    });
    note(ui, "Lines in empty PowerClip frames help you identify them; turn them off for a cleaner preview.");
}

fn powertrace(app: &mut VectorApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        label_w(ui, "Quick Trace method:", 150.0);
        combo_str(ui, "qt_method", &mut app.settings.quick_trace_method, &["Clipart", "Line Art", "Logo", "Detailed Logo", "Low Quality Image", "High Quality Image", "Centerline"], 200.0);
    });
}

const SNAP_MODES: [(&str, &str); 9] = [("Node", "□"), ("Intersection", "×"), ("Midpoint", "△"), ("Quadrant", "◇"), ("Tangent", "◯"), ("Perpendicular", "⊥"), ("Edge", "⊣"), ("Center", "⊕"), ("Text Baseline", "T")];

fn snapping(app: &mut VectorApp, ui: &mut Ui) {
    let s = &mut app.settings;
    ui.checkbox(&mut s.snap_to_objects, "Snap to objects (Alt+Z)");
    ui.checkbox(&mut s.snap_self, "Snap objects to themselves (Ctrl+Shift+H)");
    ui.checkbox(&mut s.snap_to_page, "Snap to page");
    ui.horizontal(|ui| {
        label_w(ui, "Snapping radius:", 120.0);
        spin(ui, &mut s.snap_radius, 1.0..=50.0, "", 0);
        ui.label("pixels");
    });
    ui.checkbox(&mut s.show_snap_marks, "Show Snap location marks");
    ui.checkbox(&mut s.show_snap_tip, "Show snapping mode in screen tip");
    separator(ui);
    heading(ui, "Modes");
    egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(6.0).show(ui, |ui| {
        ui.set_width(320.0);
        for (i, (n, sym)) in SNAP_MODES.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.snap_modes[i], *n);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(*sym).color(theme::SELECTION));
                });
            });
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Select All").clicked() {
            s.snap_modes = [true; 9];
        }
        if ui.button("Deselect All").clicked() {
            s.snap_modes = [false; 9];
        }
    });
}

fn browse_folder(cur: &mut Option<std::path::PathBuf>) {
    if let Some(p) = rfd::FileDialog::new().pick_folder() {
        *cur = Some(p);
    }
}

fn save(app: &mut VectorApp, ui: &mut Ui) {
    let s = &mut app.settings;
    heading(ui, "Backup");
    ui.checkbox(&mut s.backup_on_save, "Back up original file before saving");
    ui.indent("bk", |ui| {
        ui.label("Back up to:");
        let mut choose = s.backup_dir.is_some();
        if ui.radio(!choose, "Same folder as original file").clicked() {
            choose = false;
            s.backup_dir = None;
        }
        ui.horizontal(|ui| {
            if ui.radio(choose, "Choose location:").clicked() && !choose {
                s.backup_dir = Some(crate::settings::documents_dir());
            }
            let mut t = s.backup_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            ui.add_enabled(s.backup_dir.is_some(), egui::TextEdit::singleline(&mut t).desired_width(260.0));
            if ui.add_enabled(s.backup_dir.is_some(), egui::Button::new("Browse...")).clicked() {
                browse_folder(&mut s.backup_dir);
            }
        });
    });
    separator(ui);
    heading(ui, "Auto-backup");
    ui.horizontal(|ui| {
        ui.checkbox(&mut s.auto_backup, "Back up every");
        let opts = ["1", "2", "5", "10", "15", "20", "30", "60"];
        let mut i = opts.iter().position(|o| o.parse::<u32>().ok() == Some(s.auto_backup_minutes)).unwrap_or(5);
        if combo(ui, "ab_min", &mut i, &opts, 50.0) {
            s.auto_backup_minutes = opts[i].parse().unwrap_or(20);
        }
        ui.label("minutes");
    });
    ui.indent("abk", |ui| {
        ui.label("Back up to:");
        let mut choose = s.auto_backup_dir.is_some();
        if ui.radio(!choose, "Temporary folder").clicked() {
            choose = false;
            s.auto_backup_dir = None;
        }
        ui.horizontal(|ui| {
            if ui.radio(choose, "Choose location:").clicked() && !choose {
                s.auto_backup_dir = Some(crate::settings::documents_dir());
            }
            let mut t = s.auto_backup_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            ui.add_enabled(s.auto_backup_dir.is_some(), egui::TextEdit::singleline(&mut t).desired_width(260.0));
            if ui.add_enabled(s.auto_backup_dir.is_some(), egui::Button::new("Browse...")).clicked() {
                browse_folder(&mut s.auto_backup_dir);
            }
        });
    });
    separator(ui);
    heading(ui, "Save documents to previous versions");
    ui.checkbox(&mut s.ask_save_previous, "Ask when saving");
    ui.radio_value(&mut s.keep_appearance, true, "Keep appearance (suitable for limited edits)");
    ui.radio_value(&mut s.keep_appearance, false, "Keep editable (may change appearance)");
    separator(ui);
    ui.checkbox(&mut s.embed_fonts, "Embed fonts when saving files");
    ui.horizontal(|ui| {
        label_w(ui, "Set default version:", 150.0);
        combo_str(ui, "def_ver", &mut s.default_version, crate::settings::FILE_VERSIONS, 120.0);
    });
}

fn text(app: &mut VectorApp, ui: &mut Ui) {
    let mut tab = app.dialogs.options.tab;
    tabs(ui, &mut tab, &["Display", "Paragraph Text", "Fonts", "Spelling", "QuickCorrect"]);
    app.dialogs.options.tab = tab;
    tab_body(ui, |ui| match tab {
        0 => {
            let s = &mut app.settings;
            ui.checkbox(&mut s.text_show_handles, "Show handles while editing");
            let mut a = false;
            ui.checkbox(&mut a, "Auto-keyboard switching");
            let mut b = false;
            ui.checkbox(&mut b, "Match font when inserting text in another language");
            ui.horizontal(|ui| {
                label_w(ui, "Keyboard text increment:", 170.0);
                spin(ui, &mut s.text_increment, 0.1..=100.0, "", 1);
                ui.label("points");
            });
            ui.horizontal(|ui| {
                label_w(ui, "Default text units:", 170.0);
                let mut i = 0;
                combo(ui, "text_units", &mut i, &["points"], 120.0);
            });
        }
        1 => {
            let s = &mut app.settings;
            ui.checkbox(&mut s.show_text_links, "Show linking of text frames");
            ui.checkbox(&mut s.show_text_frames, "Show text frames");
            ui.checkbox(&mut s.expand_paragraph_frames, "Expand and shrink paragraph text frames to fit text");
            ui.add_space(6.0);
            ui.label("When editing, apply text frame formatting to:");
            let mut m = 2;
            ui.radio_value(&mut m, 0, "All linked frames");
            ui.radio_value(&mut m, 1, "Selected frames only");
            ui.radio_value(&mut m, 2, "Selected and subsequent frames");
        }
        2 => {
            heading(ui, "Font list contents");
            ui.horizontal(|ui| {
                label_w(ui, "Number of most recently used fonts displayed:", 300.0);
                spin_usize(ui, &mut app.settings.recent_fonts, 0..=20);
            });
            ui.horizontal(|ui| {
                if ui.button("⟳ Update fonts").clicked() {
                    std::thread::spawn(vector_core::text::preload_fonts);
                }
            });
            let mut remote = false;
            ui.checkbox(&mut remote, "Enable remote fonts");
            let n = vector_core::text::fonts().lock().families().len();
            note(ui, &format!("{n} font families installed."));
        }
        3 => {
            let s = &mut app.settings;
            ui.checkbox(&mut s.spell_check, "Perform automatic spell checking");
            ui.label("Show errors in:");
            ui.indent("sp", |ui| {
                ui.radio_value(&mut s.spell_errors_all, true, "All text frames");
                ui.radio_value(&mut s.spell_errors_all, false, "Selected text frame only");
            });
            ui.horizontal(|ui| {
                label_w(ui, "Spelling suggestions:", 150.0);
                let mut n = 0u32;
                spin_u32(ui, &mut n, 0..=10);
            });
            let mut a = false;
            ui.checkbox(&mut a, "Add corrections to QuickCorrect");
            let mut b = false;
            ui.checkbox(&mut b, "Show errors which have been ignored");
        }
        _ => quickcorrect(app, ui),
    });
}

fn quickcorrect(app: &mut VectorApp, ui: &mut Ui) {
    let s = &mut app.settings;
    ui.checkbox(&mut s.quickcorrect_capitalize, "Capitalize first letter of sentences");
    ui.checkbox(&mut s.quickcorrect_two_caps, "Correct two initial, consecutive capitals");
    ui.checkbox(&mut s.quickcorrect_days, "Capitalize names of days");
    ui.checkbox(&mut s.quickcorrect_links, "Automatically create hyperlinks when typing web addresses");
    separator(ui);
    ui.horizontal(|ui| {
        label_w(ui, "Language:", 70.0);
        let mut i = 0;
        combo(ui, "qc_lang", &mut i, &["English (United States)"], 200.0);
    });
    ui.checkbox(&mut s.typographic_quotes, "Change straight quotes to typographic quotes");
    let mut after_num = false;
    ui.checkbox(&mut after_num, "Use straight quotation marks after numbers (e.g. 6')");
    ui.label("Single Quotes:");
    ui.horizontal(|ui| {
        ui.label("Open:");
        let mut a = 0;
        combo(ui, "sq_o", &mut a, &["‘", "'", "‚", "‹", "›"], 30.0);
        ui.label("Close:");
        let mut b = 0;
        combo(ui, "sq_c", &mut b, &["’", "'", "‘", "›", "‹"], 30.0);
    });
    ui.label("Double Quotes:");
    ui.horizontal(|ui| {
        ui.label("Open:");
        let mut a = 0;
        combo(ui, "dq_o", &mut a, &["“", "\"", "„", "«", "»"], 30.0);
        ui.label("Close:");
        let mut b = 0;
        combo(ui, "dq_c", &mut b, &["”", "\"", "“", "»", "«"], 30.0);
    });
    separator(ui);
    heading(ui, "Replacement text");
    ui.checkbox(&mut s.replace_while_typing, "Replace text while typing");
    let st = &mut app.dialogs.options;
    let s = &mut app.settings;
    ui.horizontal(|ui| {
        ui.label("Replace:");
        ui.add(egui::TextEdit::singleline(&mut st.replace_from).desired_width(110.0));
        ui.add_space(20.0);
        ui.label("With:");
        ui.add(egui::TextEdit::singleline(&mut st.replace_with).desired_width(110.0));
    });
    ui.horizontal(|ui| {
        egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(4.0).show(ui, |ui| {
            ui.set_width(350.0);
            egui::ScrollArea::vertical().id_salt("qc_list").max_height(150.0).show(ui, |ui| {
                for (i, (a, b)) in s.replacements.iter().enumerate() {
                    let r = ui.horizontal(|ui| {
                        let sel = st.replace_sel == Some(i);
                        let r1 = ui.add_sized(vec2(170.0, 20.0), egui::Button::selectable(sel, a.as_str()).frame_when_inactive(false));
                        let r2 = ui.add_sized(vec2(160.0, 20.0), egui::Button::selectable(sel, b.as_str()).frame_when_inactive(false));
                        r1.clicked() || r2.clicked()
                    });
                    if r.inner {
                        st.replace_sel = Some(i);
                        st.replace_from = a.clone();
                        st.replace_with = b.clone();
                    }
                }
            });
        });
        ui.vertical(|ui| {
            let can_add = !st.replace_from.is_empty() && !s.replacements.iter().any(|(a, _)| *a == st.replace_from);
            if ui.add_enabled(can_add, egui::Button::new("Add").min_size(vec2(80.0, 22.0))).clicked() {
                s.replacements.push((st.replace_from.clone(), st.replace_with.clone()));
                s.replacements.sort();
            }
            if ui.add_enabled(st.replace_sel.is_some(), egui::Button::new("Delete").min_size(vec2(80.0, 22.0))).clicked() {
                if let Some(i) = st.replace_sel.take() {
                    if i < s.replacements.len() {
                        s.replacements.remove(i);
                    }
                }
            }
            if ui.add_enabled(st.replace_sel.is_some(), egui::Button::new("Replace").min_size(vec2(80.0, 22.0))).clicked() {
                if let Some(i) = st.replace_sel {
                    if i < s.replacements.len() {
                        s.replacements[i] = (st.replace_from.clone(), st.replace_with.clone());
                    }
                }
            }
        });
    });
}

fn pen(app: &mut VectorApp, ui: &mut Ui) {
    let st = &mut app.dialogs.options;
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 220.0), Sense::drag());
    ui.painter().rect_filled(r, 0.0, Color32::WHITE);
    if resp.drag_started() {
        st.pen_strokes.push(vec![]);
    }
    if resp.dragged() {
        if let (Some(p), Some(s)) = (resp.interact_pointer_pos(), st.pen_strokes.last_mut()) {
            s.push(p);
        }
    }
    for s in &st.pen_strokes {
        if s.len() > 1 {
            ui.painter().add(egui::Shape::line(s.clone(), Stroke::new(2.0, Color32::BLACK)));
        }
    }
    note(ui, "Apply at least three strokes with your stylus using a full range of pressure, and click Save.");
    separator(ui);
    heading(ui, "Interface");
    let mut rts = true;
    ui.radio_value(&mut rts, false, "WinTab - not installed");
    ui.radio_value(&mut rts, true, "Real-Time Stylus - installed");
    note(ui, "Real-Time Stylus is the Windows pen interface. WinTab is used by some older graphics tablets.");
    ui.horizontal(|ui| {
        if ui.add_enabled(st.pen_strokes.len() >= 3, egui::Button::new("Save").min_size(vec2(75.0, 22.0))).clicked() {
            st.pen_strokes.clear();
        }
        if ui.button("Clear").clicked() {
            st.pen_strokes.clear();
        }
    });
}

fn warnings(app: &mut VectorApp, ui: &mut Ui) {
    ui.label("Show warnings when:");
    if app.settings.warnings.len() < 10 {
        app.settings.warnings = default_warnings();
    }
    egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(egui::Margin { left: 4, right: 4, top: 2, bottom: 2 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::ScrollArea::vertical().id_salt("warn_list").max_height(370.0).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 5.0;
            for (t, on) in app.settings.warnings.iter_mut() {
                ui.checkbox(on, t.as_str()).on_hover_text(t.as_str());
            }
        });
    });
}

fn scripts(app: &mut VectorApp, ui: &mut Ui) {
    heading(ui, "Security");
    if ui.add(egui::Button::new("Security Options...").min_size(vec2(130.0, 22.0))).clicked() {
        app.dialogs.options.security_dialog = !app.dialogs.options.security_dialog;
    }
    if app.dialogs.options.security_dialog {
        egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(8.0).show(ui, |ui| {
            ui.label(egui::RichText::new("Security Level").strong());
            let mut lvl = if app.settings.trust_scripts { 0 } else { 1 };
            ui.radio_value(&mut lvl, 0, "Low: run all scripts");
            ui.radio_value(&mut lvl, 1, "Medium: ask before running scripts from outside the Scripts folder");
            app.settings.trust_scripts = lvl == 0;
            note(ui, &format!("Scripts folder: {}", crate::scripting::scripts_dir().display()));
        });
    }
    ui.checkbox(&mut app.settings.trust_scripts, "Trust all installed script modules");
    separator(ui);
    ui.checkbox(&mut app.settings.delay_load_scripts, "Delay Load scripting engine");
}

fn plugins(app: &mut VectorApp, ui: &mut Ui) {
    ui.label("Plug-ins Folders");
    let st = &mut app.dialogs.options;
    let s = &mut app.settings;
    egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(4.0).show(ui, |ui| {
        ui.set_min_size(vec2(ui.available_width(), 300.0));
        for (i, (p, on)) in s.plugin_folders.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(on, "");
                if ui.selectable_label(st.plugin_sel == Some(i), p.display().to_string()).clicked() {
                    st.plugin_sel = Some(i);
                }
            });
        }
    });
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new("Add...").min_size(vec2(75.0, 22.0))).clicked() {
            if let Some(p) = rfd::FileDialog::new().pick_folder() {
                s.plugin_folders.push((p, true));
            }
        }
        if ui.add_enabled(st.plugin_sel.is_some(), egui::Button::new("Edit...").min_size(vec2(75.0, 22.0))).clicked() {
            if let (Some(i), Some(p)) = (st.plugin_sel, rfd::FileDialog::new().pick_folder()) {
                if i < s.plugin_folders.len() {
                    s.plugin_folders[i].0 = p;
                }
            }
        }
        if ui.add_enabled(st.plugin_sel.is_some(), egui::Button::new("Remove").min_size(vec2(75.0, 22.0))).clicked() {
            if let Some(i) = st.plugin_sel.take() {
                if i < s.plugin_folders.len() {
                    s.plugin_folders.remove(i);
                }
            }
        }
    });
}

// ------------------------------------------------------------------------------------------
// Customization
// ------------------------------------------------------------------------------------------

fn appearance(app: &mut VectorApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let s = &mut app.settings;
    ui.checkbox(&mut s.center_dialogs, "Center dialog boxes");
    separator(ui);
    heading(ui, "Size");
    ui.horizontal(|ui| {
        let r = ui.add(egui::Slider::new(&mut s.ui_scale, 75.0..=200.0).step_by(25.0).suffix(" %"));
        if r.drag_stopped() || (r.changed() && !r.dragged()) {
            ctx.set_zoom_factor((s.ui_scale / 100.0) as f32);
        }
        if ui.button("Reset to Default").clicked() {
            s.ui_scale = 100.0;
            ctx.set_zoom_factor(1.0);
        }
    });
    note(ui, "Changes the size of the user interface elements, such as toolbars, dockers and dialog boxes.");
    separator(ui);
    heading(ui, "Color");
    ui.horizontal(|ui| {
        label_w(ui, "Theme:", 110.0);
        combo_str(ui, "theme", &mut s.theme_name, &["Dark"], 120.0);
    });
    ui.horizontal(|ui| {
        label_w(ui, "Window border:", 110.0);
        let mut c = [0x2b, 0x2b, 0x2b];
        rgb_edit(ui, &mut c);
    });
    ui.horizontal(|ui| {
        label_w(ui, "Desktop:", 110.0);
        if rgb_edit(ui, &mut s.desktop_color) {
            for d in app.docs.iter_mut() {
                d.cache.key = 0;
            }
        }
    });
}

fn command_bars(app: &mut VectorApp, ui: &mut Ui) {
    use crate::app::Toolbar;
    let mut flash = false;
    let new_cmd_bar = &mut app.dialogs.options.new_cmd_bar;
    let s = &mut app.settings;
    ui.horizontal(|ui| {
        label_w(ui, "Global button size:", 140.0);
        let mut i = match s.button_size {
            ButtonSize::Small => 0,
            ButtonSize::Medium => 1,
            ButtonSize::Large => 2,
        };
        if combo(ui, "btn_size", &mut i, &["Small", "Medium", "Large"], 100.0) {
            s.button_size = [ButtonSize::Small, ButtonSize::Medium, ButtonSize::Large][i];
        }
    });
    let list: [(&str, Option<Toolbar>); 14] = [
        ("Menu bar", Some(Toolbar::MenuBar)),
        ("Context menu bar", None),
        ("Status bar", Some(Toolbar::StatusBar)),
        ("Project timer", Some(Toolbar::ProjectTimer)),
        ("Layout", Some(Toolbar::Layout)),
        ("Zoom", Some(Toolbar::Zoom)),
        ("Internet", Some(Toolbar::Internet)),
        ("Property bar", Some(Toolbar::PropertyBar)),
        ("Toolbox", Some(Toolbar::Toolbox)),
        ("Macros", Some(Toolbar::Macros)),
        ("Standard", Some(Toolbar::Standard)),
        ("Parallel Drawing", Some(Toolbar::ParallelDrawing)),
        ("Text", Some(Toolbar::Text)),
        ("Transform", Some(Toolbar::Transform)),
    ];
    let mut toggle = None;
    ui.horizontal(|ui| {
        egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(4.0).show(ui, |ui| {
            ui.set_width(260.0);
            ui.spacing_mut().item_spacing.y = 3.0;
            for (name, tb) in list {
                let mut on = match tb {
                    Some(Toolbar::MenuBar) => s.show_menu_bar,
                    Some(Toolbar::StatusBar) => s.show_status_bar,
                    Some(Toolbar::Standard) => s.show_standard,
                    Some(Toolbar::PropertyBar) => s.show_property_bar,
                    Some(Toolbar::Toolbox) => s.show_toolbox,
                    Some(t) => s.extra_toolbars.iter().any(|x| x == t.name()),
                    None => true,
                };
                if ui.add_enabled(tb.is_some(), egui::Checkbox::new(&mut on, name)).changed() {
                    toggle = tb;
                }
            }
        });
        ui.vertical(|ui| {
            ui.add(egui::TextEdit::singleline(new_cmd_bar).hint_text("Name").desired_width(100.0));
            if ui.add(egui::Button::new("New").min_size(vec2(75.0, 22.0))).clicked() {
                flash = true;
            }
            if ui.add(egui::Button::new("Reset").min_size(vec2(75.0, 22.0))).clicked() {
                let d = Settings::default();
                s.show_menu_bar = d.show_menu_bar;
                s.show_status_bar = d.show_status_bar;
                s.show_standard = d.show_standard;
                s.show_property_bar = d.show_property_bar;
                s.show_toolbox = d.show_toolbox;
                s.extra_toolbars.clear();
            }
        });
    });
    if flash {
        app.flash("Custom command bars are created from the Commands page.");
    }
    if let Some(t) = toggle {
        app.run(Cmd::ToggleToolbar(t));
    }
    let s = &mut app.settings;
    separator(ui);
    heading(ui, "Button properties");
    ui.horizontal(|ui| {
        label_w(ui, "Size:", 60.0);
        let mut i = match s.button_size {
            ButtonSize::Small => 0,
            ButtonSize::Medium => 1,
            ButtonSize::Large => 2,
        };
        if combo(ui, "btn_size2", &mut i, &["Small", "Medium", "Large"], 100.0) {
            s.button_size = [ButtonSize::Small, ButtonSize::Medium, ButtonSize::Large][i];
        }
        ui.add_space(20.0);
        label_w(ui, "Style:", 50.0);
        let mut st = 0;
        combo(ui, "btn_style", &mut st, &["Default", "Image only", "Text only", "Image and text"], 120.0);
    });
    separator(ui);
    heading(ui, "Toolbar properties");
    ui.checkbox(&mut s.show_floating_title, "Show title when toolbar is floating");
    ui.checkbox(&mut s.lock_toolbars, "Lock toolbars");
    separator(ui);
    heading(ui, "Menu bar mode");
    ui.radio_value(&mut s.menu_bar_mode, MenuBarMode::Normal, "Normal mode");
    ui.radio_value(&mut s.menu_bar_mode, MenuBarMode::File, "File mode");
}

fn shortcut_from_input(ctx: &egui::Context) -> Option<String> {
    ctx.input(|i| {
        for e in &i.events {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = e {
                if matches!(key, egui::Key::Escape) {
                    return None;
                }
                let mut s = String::new();
                if modifiers.ctrl || modifiers.command {
                    s.push_str("Ctrl+");
                }
                if modifiers.shift {
                    s.push_str("Shift+");
                }
                if modifiers.alt {
                    s.push_str("Alt+");
                }
                s.push_str(key.name());
                return Some(s);
            }
        }
        None
    })
}

fn commands(app: &mut VectorApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let names: Vec<&str> = crate::menus::CATALOG_MENUS.iter().map(|m| m.0).collect();
    let mut mi = app.dialogs.options.commands_menu.min(names.len() - 1);
    ui.horizontal(|ui| {
        if combo(ui, "cmd_menu", &mut mi, &names, 150.0) {
            app.dialogs.options.commands_sel = None;
        }
        ui.add(egui::TextEdit::singleline(&mut app.dialogs.options.commands_search).hint_text("Search").desired_width(180.0));
    });
    app.dialogs.options.commands_menu = mi;
    if !app.dialogs.options.catalog.iter().any(|(k, _)| *k == mi) {
        let entries = crate::menus::catalog(app, ui, crate::menus::CATALOG_MENUS[mi].1);
        app.dialogs.options.catalog.push((mi, entries));
    }
    let entries = app.dialogs.options.catalog.iter().find(|(k, _)| *k == mi).map(|(_, e)| e.clone()).unwrap_or_default();
    let q = app.dialogs.options.commands_search.to_lowercase();
    ui.horizontal_top(|ui| {
        egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(4.0).show(ui, |ui| {
            ui.set_width(250.0);
            egui::ScrollArea::vertical().id_salt("cmd_list").max_height(470.0).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for e in &entries {
                    let plain = crate::widgets::plain(&e.label);
                    if !q.is_empty() && !plain.to_lowercase().contains(&q) {
                        continue;
                    }
                    let sel = app.dialogs.options.commands_sel == Some(e.cmd);
                    let img = e.icon.as_deref().and_then(|i| app.icons.image(&ctx, i, 16.0, true));
                    let b = match img {
                        Some(img) => egui::Button::selectable(sel, (img, plain)),
                        None => egui::Button::selectable(sel, format!("      {plain}")),
                    };
                    if ui.add(b.frame_when_inactive(false).min_size(vec2(236.0, 20.0))).clicked() {
                        app.dialogs.options.commands_sel = Some(e.cmd);
                        app.dialogs.options.capture = None;
                    }
                }
            });
        });
        ui.vertical(|ui| {
            let mut tab = app.dialogs.options.commands_tab;
            tabs(ui, &mut tab, &["General", "Shortcut Keys"]);
            app.dialogs.options.commands_tab = tab;
            let Some(cmd) = app.dialogs.options.commands_sel else {
                note(ui, "Select a command.");
                return;
            };
            let entry = entries.iter().find(|e| e.cmd == cmd).cloned();
            let label = entry.as_ref().map(|e| e.label.clone()).unwrap_or_default();
            if tab == 0 {
                ui.horizontal(|ui| {
                    label_w(ui, "Caption:", 70.0);
                    let mut l = label.clone();
                    ui.add_enabled(false, egui::TextEdit::singleline(&mut l).desired_width(200.0));
                });
                ui.horizontal(|ui| {
                    label_w(ui, "Icon:", 70.0);
                    if let Some(img) = entry.as_ref().and_then(|e| e.icon.clone()).and_then(|i| app.icons.image(&ctx, &i, 32.0, true)) {
                        ui.add(img);
                    }
                });
                ui.horizontal(|ui| {
                    label_w(ui, "Tooltip:", 70.0);
                    let mut t = crate::widgets::plain(&label).trim_end_matches("...").to_string();
                    ui.add_enabled(false, egui::TextEdit::singleline(&mut t).desired_width(200.0));
                });
                ui.label(format!("Available on: {}", names[mi]));
            } else {
                let cur = crate::shortcuts::shortcut_text(app, cmd).unwrap_or_default();
                ui.horizontal(|ui| {
                    label_w(ui, "Current shortcut keys:", 150.0);
                    ui.label(if cur.is_empty() { "(none)".to_string() } else { cur.clone() });
                });
                ui.horizontal(|ui| {
                    label_w(ui, "New shortcut key:", 150.0);
                    let capturing = app.dialogs.options.capture.is_some();
                    let text = app.dialogs.options.capture.clone().unwrap_or_default();
                    let r = ui.add(egui::Button::new(if text.is_empty() { if capturing { "Press keys..." } else { "Click, then press keys" } } else { text.as_str() }).min_size(vec2(160.0, 22.0)).selected(capturing));
                    if r.clicked() {
                        app.dialogs.options.capture = Some(String::new());
                    }
                    if capturing {
                        if let Some(sc) = shortcut_from_input(&ctx) {
                            app.dialogs.options.capture = Some(sc);
                        }
                    }
                });
                let new = app.dialogs.options.capture.clone().unwrap_or_default();
                if !new.is_empty() {
                    let conflict = crate::commands::shortcut_commands().into_iter().find(|c| *c != cmd && crate::shortcuts::shortcut_text(app, *c).as_deref() == Some(new.as_str()));
                    if let Some(c) = conflict {
                        note(ui, &format!("Conflicts with: {}", crate::scripting::cmd_to_text(&c)));
                    }
                }
                ui.horizontal(|ui| {
                    let name = crate::scripting::cmd_to_text(&cmd);
                    if ui.add_enabled(!new.is_empty(), egui::Button::new("Assign")).clicked() {
                        app.settings.shortcuts.retain(|(c, _)| *c != name);
                        app.settings.shortcuts.push((name.clone(), new.clone()));
                        app.dialogs.options.capture = None;
                    }
                    if ui.add_enabled(!cur.is_empty(), egui::Button::new("Delete")).clicked() {
                        app.settings.shortcuts.retain(|(c, _)| *c != name);
                        app.settings.shortcuts.push((name.clone(), String::new()));
                    }
                    if ui.button("Reset All").clicked() {
                        app.settings.shortcuts.clear();
                    }
                });
            }
        });
    });
}

fn color_palette(app: &mut VectorApp, ui: &mut Ui) {
    let s = &mut app.settings;
    ui.checkbox(&mut s.auto_update_doc_palette, "Automatically update the document palette");
    separator(ui);
    heading(ui, "Palette options");
    ui.horizontal(|ui| {
        label_w(ui, "Maximum palette rows when docked:", 240.0);
        spin_u32(ui, &mut s.palette_rows, 1..=10);
    });
    separator(ui);
    heading(ui, "Color wells");
    ui.checkbox(&mut s.wide_borders, "Wide borders");
    ui.checkbox(&mut s.large_swatches, "Large swatches");
    ui.checkbox(&mut s.show_no_color_well, "Show \"No Color\" well");
    separator(ui);
    heading(ui, "Secondary mouse button");
    ui.radio_value(&mut s.right_click_outline, false, "Context menu");
    ui.radio_value(&mut s.right_click_outline, true, "Set outline color");
    note(ui, "When \"Set outline color\" is selected, right-clicking a color well sets the outline color; press and hold to open the context menu.");
}

// ------------------------------------------------------------------------------------------
// Tools
// ------------------------------------------------------------------------------------------

fn tool_page(app: &mut VectorApp, ui: &mut Ui, page: &str) {
    let (u, dpi) = unit_dpi(app);
    let s = &mut app.settings;
    match page {
        "Pick" => {
            ui.checkbox(&mut s.crosshair_cursor, "Crosshair cursor");
            ui.checkbox(&mut s.treat_as_filled, "Treat all objects as filled");
            separator(ui);
            heading(ui, "Ctrl and Shift keys");
            ui.radio_value(&mut s.windows_modifiers, false, "Traditional Vector: Ctrl = Constrain, Shift = Transform from center");
            ui.radio_value(&mut s.windows_modifiers, true, "Windows standard: Ctrl = Duplicate/Leave Original, Shift = Constrain");
        }
        "Eraser" => {
            ui.horizontal(|ui| {
                label_w(ui, "Thickness:", 75.0);
                unit_spin(ui, "eraser_t", &mut s.eraser_thickness, u, dpi);
            });
            ui.add_space(4.0);
            ui.checkbox(&mut s.eraser_reduce_nodes, "Auto-reduce nodes of resulting objects");
        }
        "Zoom/Pan" => {
            heading(ui, "Zoom tool");
            ui.label("Secondary mouse button:");
            ui.indent("zt", |ui| {
                ui.radio_value(&mut s.zoom_right_click_out, true, "Zoom out");
                ui.radio_value(&mut s.zoom_right_click_out, false, "Context menu");
            });
            ui.checkbox(&mut s.zoom_relative_1_1, "Zoom relative to 1:1");
            ui.checkbox(&mut s.zoom_center_mouse, "Center mouse when zooming with the mouse wheel");
            separator(ui);
            heading(ui, "Pan tool");
            ui.label("Secondary mouse button:");
            ui.indent("pt", |ui| {
                ui.radio_value(&mut s.zoom_right_click_out, true, "Zoom out");
                ui.radio_value(&mut s.zoom_right_click_out, false, "Context menu");
            });
            separator(ui);
            if ui.button("Calibrate Zoom...").clicked() {
                app.dialogs.options.zoom_calibrate = !app.dialogs.options.zoom_calibrate;
            }
            note(ui, "Calibrating the rulers makes 100% zoom match real-world size on your monitor.");
            if app.dialogs.options.zoom_calibrate {
                let ppi = ui.ctx().pixels_per_point();
                note(ui, &format!("Current display: {:.0} dpi ({:.0}% scaling). Hold a ruler to the screen and adjust Size in Customization > Appearance until an inch matches.", 96.0 * ppi, ppi * 100.0));
            }
        }
        "Freehand/Bézier" => {
            ui.horizontal(|ui| {
                label_w(ui, "Freehand smoothing:", 160.0);
                ui.add(egui::Slider::new(&mut s.freehand_smoothing, 0.0..=100.0));
            });
            ui.horizontal(|ui| {
                label_w(ui, "Corner threshold:", 160.0);
                spin(ui, &mut s.corner_threshold, 1.0..=20.0, "", 0);
                ui.label("pixels");
            });
            ui.horizontal(|ui| {
                label_w(ui, "Straight line threshold:", 160.0);
                spin(ui, &mut s.straight_threshold, 1.0..=20.0, "", 0);
                ui.label("pixels");
            });
            ui.horizontal(|ui| {
                label_w(ui, "Auto-join:", 160.0);
                spin(ui, &mut s.auto_join, 1.0..=20.0, "", 0);
                ui.label("pixels");
            });
        }
        "Smart Drawing" => {
            ui.horizontal(|ui| {
                label_w(ui, "Drawing assistance delay:", 180.0);
                ui.add(egui::Slider::new(&mut s.smart_draw_delay, 0.0..=10.0).step_by(0.1).fixed_decimals(1));
                ui.label("second(s)");
            });
        }
        "Rectangle" => {
            ui.horizontal(|ui| {
                label_w(ui, "Corner type:", 100.0);
                ui.selectable_value(&mut s.rect_corner, 0, "Round");
                ui.selectable_value(&mut s.rect_corner, 1, "Scallop");
                ui.selectable_value(&mut s.rect_corner, 2, "Chamfer");
            });
            heading(ui, "Corners");
            egui::Grid::new("rect_r").num_columns(2).spacing(vec2(20.0, 6.0)).show(ui, |ui| {
                unit_spin(ui, "rr0", &mut s.rect_radii[0], u, dpi);
                unit_spin(ui, "rr1", &mut s.rect_radii[1], u, dpi);
                ui.end_row();
                unit_spin(ui, "rr3", &mut s.rect_radii[3], u, dpi);
                unit_spin(ui, "rr2", &mut s.rect_radii[2], u, dpi);
                ui.end_row();
            });
            ui.checkbox(&mut s.rect_scale_corners, "Scale corners");
        }
        "Ellipse" => {
            ui.horizontal(|ui| {
                label_w(ui, "Type:", 100.0);
                ui.radio_value(&mut s.ellipse_kind, 0, "Ellipse");
                ui.radio_value(&mut s.ellipse_kind, 1, "Pie");
                ui.radio_value(&mut s.ellipse_kind, 2, "Arc");
            });
            let en = s.ellipse_kind != 0;
            ui.add_enabled_ui(en, |ui| {
                ui.horizontal(|ui| {
                    label_w(ui, "Starting angle:", 100.0);
                    spin(ui, &mut s.ellipse_start, -360.0..=360.0, "°", 1);
                });
                ui.horizontal(|ui| {
                    label_w(ui, "Ending angle:", 100.0);
                    spin(ui, &mut s.ellipse_end, -360.0..=360.0, "°", 1);
                });
                ui.horizontal(|ui| {
                    label_w(ui, "Direction:", 100.0);
                    ui.radio_value(&mut s.ellipse_clockwise, true, "Clockwise");
                    ui.radio_value(&mut s.ellipse_clockwise, false, "Counterclockwise");
                });
            });
        }
        "Graph Paper" => {
            ui.horizontal(|ui| {
                label_w(ui, "Number of columns:", 140.0);
                spin_u32(ui, &mut s.graph_cols, 1..=99);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Number of rows:", 140.0);
                spin_u32(ui, &mut s.graph_rows, 1..=99);
            });
        }
        "Spiral" => {
            ui.horizontal(|ui| {
                label_w(ui, "Type:", 140.0);
                ui.radio_value(&mut s.spiral_log, false, "Symmetrical");
                ui.radio_value(&mut s.spiral_log, true, "Logarithmic");
            });
            let (r, _) = ui.allocate_exact_size(vec2(90.0, 90.0), Sense::hover());
            let sp = vector_core::object::SpiralShape { w: 80.0, h: 80.0, revolutions: s.spiral_revs as f64, logarithmic: s.spiral_log, expansion: s.spiral_expansion };
            let path = vector_core::object::spiral_path(&sp);
            let pts: Vec<egui::Pos2> = vector_core::geom::flatten_polylines(&path, 0.5).into_iter().flat_map(|(p, _)| p).map(|p| r.center() + vec2(p.x as f32, p.y as f32)).collect();
            ui.painter().add(egui::Shape::line(pts, Stroke::new(1.0, theme::TEXT)));
            ui.horizontal(|ui| {
                label_w(ui, "Number of revolutions:", 160.0);
                spin_u32(ui, &mut s.spiral_revs, 1..=100);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Expansion:", 160.0);
                ui.add_enabled(s.spiral_log, egui::Slider::new(&mut s.spiral_expansion, 1.0..=100.0));
            });
        }
        "Table" => {
            ui.horizontal(|ui| {
                label_w(ui, "Rows:", 80.0);
                spin_u32(ui, &mut s.table_rows, 1..=99);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Columns:", 80.0);
                spin_u32(ui, &mut s.table_cols, 1..=99);
            });
            separator(ui);
            ui.label("When you edit a table, pressing Tab lets you:");
            ui.indent("tab", |ui| {
                ui.radio_value(&mut s.table_tab_moves, false, "Insert a tab character");
                ui.radio_value(&mut s.table_tab_moves, true, "Move to the next cell");
            });
            ui.horizontal(|ui| {
                label_w(ui, "Tab order:", 80.0);
                let mut i = 0;
                combo(ui, "tab_order", &mut i, &["Left to right, top to bottom", "Right to left, top to bottom", "Top to bottom, left to right"], 240.0);
            });
        }
        "Dimension" => {
            ui.horizontal(|ui| {
                label_w(ui, "Style:", 80.0);
                combo_u8(ui, "dim_style", &mut s.dim_style, &["Decimal", "Fractional", "US Engineering", "US Architectural"], 160.0);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Precision:", 80.0);
                let mut i = s.dim_precision as usize;
                if combo(ui, "dim_prec", &mut i, &["0", "0.0", "0.00", "0.000", "0.0000", "0.00000"], 160.0) {
                    s.dim_precision = i as u32;
                }
            });
            ui.horizontal(|ui| {
                label_w(ui, "Units:", 80.0);
                let mut uu = s.default_unit;
                unit_combo(ui, "dim_unit", &mut uu);
                s.default_unit = uu;
            });
            ui.horizontal(|ui| {
                label_w(ui, "Prefix:", 80.0);
                ui.add(egui::TextEdit::singleline(&mut s.dim_prefix).desired_width(160.0));
            });
            ui.horizontal(|ui| {
                label_w(ui, "Suffix:", 80.0);
                ui.add(egui::TextEdit::singleline(&mut s.dim_suffix).desired_width(160.0));
            });
        }
        "Angular Dimension" => {
            ui.horizontal(|ui| {
                label_w(ui, "Precision:", 80.0);
                let mut i = s.angular_precision as usize;
                if combo(ui, "ang_prec", &mut i, &["0", "0.0", "0.00", "0.000", "0.0000"], 160.0) {
                    s.angular_precision = i as u32;
                }
            });
            ui.horizontal(|ui| {
                label_w(ui, "Units:", 80.0);
                let mut i = 0;
                combo(ui, "ang_unit", &mut i, &["degrees", "radians", "gradians"], 160.0);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Prefix:", 80.0);
                ui.add(egui::TextEdit::singleline(&mut s.dim_prefix).desired_width(160.0));
            });
            ui.horizontal(|ui| {
                label_w(ui, "Suffix:", 80.0);
                ui.add(egui::TextEdit::singleline(&mut s.dim_suffix).desired_width(160.0));
            });
        }
        "Connector" => {
            ui.checkbox(&mut s.connector_geometric_anchors, "Use geometric anchor points as snapping points");
            ui.horizontal(|ui| {
                label_w(ui, "Route distance from object:", 190.0);
                unit_spin(ui, "conn_route", &mut s.connector_route_distance, u, dpi);
            });
        }
        _ => {
            ui.horizontal(|ui| {
                label_w(ui, "Number of columns:", 140.0);
                spin_u32(ui, &mut s.mesh_cols, 1..=50);
            });
            ui.horizontal(|ui| {
                label_w(ui, "Number of rows:", 140.0);
                spin_u32(ui, &mut s.mesh_rows, 1..=50);
            });
        }
    }
}

// ------------------------------------------------------------------------------------------
// Global
// ------------------------------------------------------------------------------------------

fn global_general(app: &mut VectorApp, ui: &mut Ui) {
    let s = &mut app.settings;
    ui.checkbox(&mut s.background_tasks, "Enable background tasks");
    separator(ui);
    ui.horizontal(|ui| {
        label_w(ui, "User interface language:", 170.0);
        combo_str(ui, "lang", &mut s.language, &["English-US"], 160.0);
    });
    ui.checkbox(&mut s.ui_language_ask, "Ask me the next time the application starts");
    separator(ui);
    ui.horizontal(|ui| {
        ui.checkbox(&mut s.hardware_accel, "Use hardware acceleration");
        let mut i = 0;
        ui.add_enabled_ui(s.hardware_accel, |ui| {
            combo(ui, "hw", &mut i, &["GPU"], 80.0);
        });
    });
    ui.indent("hwopts", |ui| {
        ui.add_enabled_ui(s.hardware_accel, |ui| {
            let mut a = true;
            ui.checkbox(&mut a, "Display");
            let mut b = true;
            ui.checkbox(&mut b, "Effects");
            let mut c = false;
            ui.checkbox(&mut c, "Art Styles");
            let mut d = false;
            ui.checkbox(&mut d, "Resampling");
            let mut e = false;
            ui.checkbox(&mut e, "JPEG Artifact Removal");
        });
    });
    note(ui, "You need to restart the application to apply these changes.");
}

const PRINT_SETTINGS: [(&str, &str); 16] = [
    ("Spot color separations warning", "If more than 2 spot colors used"),
    ("Preview color default", "Auto (Simulate output)"),
    ("Preview separations default", "Auto (Simulate output)"),
    ("Preview image default", "On at startup"),
    ("Overprint black threshold (PS)", "95"),
    ("Send large bitmaps in chunks", "No"),
    ("Bitmap output threshold (K)", "4096"),
    ("Bitmap chunk overlap", "32 pixels"),
    ("Small fonts (Preflight)", "7"),
    ("Image resolution too low (Preflight)", "96"),
    ("Composite crop marks (PS)", "Output on all plates"),
    ("PostScript 2 stroke adjust (PS)", "On"),
    ("Many fonts (Preflight)", "10"),
    ("NT double download workaround (PS)", "On"),
    ("NT bookman download workaround (PS)", "On"),
    ("Render to bitmap resolution", "Automatic dpi"),
];

fn printing(app: &mut VectorApp, ui: &mut Ui) {
    let mut tab = app.dialogs.options.tab.min(1);
    tabs(ui, &mut tab, &["Print Style", "Driver Compatibility"]);
    app.dialogs.options.tab = tab;
    tab_body(ui, |ui| {
        if tab == 0 {
            ui.horizontal(|ui| {
                label_w(ui, "Default print style:", 140.0);
                let mut i = 0;
                combo(ui, "pstyle", &mut i, &["Vector Defaults"], 200.0);
            });
            ui.checkbox(&mut app.settings.print_only_current, "Print only current page");
            separator(ui);
            heading(ui, "Special settings");
            egui::Grid::new("print_special").num_columns(2).striped(true).spacing(vec2(20.0, 4.0)).show(ui, |ui| {
                for (a, b) in PRINT_SETTINGS {
                    ui.label(a);
                    ui.label(egui::RichText::new(b).color(theme::TEXT_DIM));
                    ui.end_row();
                }
            });
        } else {
            note(ui, "Printing uses the Windows print system. Documents are sent as high-resolution page images through the system print dialog, which works with every installed printer driver.");
        }
    });
}

const FILE_LOCATIONS: [&str; 24] = [
    "Fills", "Adjustment Presets", "Artistic Media", "Bitmap Presets", "Blend", "Cloud", "Contour", "Distortion", "Documents", "Envelope", "Export Presets", "Find and Replace", "Fonts", "Image Lists", "Images", "Palettes", "Photo Frames", "Print Styles", "Shadow", "Symbols", "Templates", "Trays", "Vector Extrude", "Writing Tools",
];

fn file_locations(app: &mut VectorApp, ui: &mut Ui) {
    let base = crate::settings::config_dir();
    let st = &mut app.dialogs.options;
    let s = &mut app.settings;
    egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(4.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::ScrollArea::vertical().id_salt("file_locs").max_height(470.0).auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("fl_grid").num_columns(2).striped(true).spacing(vec2(16.0, 4.0)).show(ui, |ui| {
                ui.label(egui::RichText::new("Content Type").strong());
                ui.label(egui::RichText::new("Location").strong());
                ui.end_row();
                for (i, n) in FILE_LOCATIONS.iter().enumerate() {
                    let loc = s.file_locations.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone()).unwrap_or_else(|| {
                        if *n == "Documents" {
                            crate::settings::documents_dir().display().to_string()
                        } else {
                            base.join(n).display().to_string()
                        }
                    });
                    if ui.selectable_label(st.file_loc_sel == Some(i), *n).clicked() {
                        st.file_loc_sel = Some(i);
                    }
                    ui.label(egui::RichText::new(loc).color(theme::TEXT_DIM));
                    ui.end_row();
                }
            });
        });
    });
    ui.horizontal(|ui| {
        if ui.add_enabled(st.file_loc_sel.is_some(), egui::Button::new("Reset").min_size(vec2(75.0, 22.0))).clicked() {
            if let Some(i) = st.file_loc_sel {
                s.file_locations.retain(|(k, _)| k != FILE_LOCATIONS[i]);
            }
        }
        if ui.add_enabled(st.file_loc_sel.is_some(), egui::Button::new("Edit").min_size(vec2(75.0, 22.0))).clicked() {
            if let (Some(i), Some(p)) = (st.file_loc_sel, rfd::FileDialog::new().pick_folder()) {
                s.file_locations.retain(|(k, _)| k != FILE_LOCATIONS[i]);
                s.file_locations.push((FILE_LOCATIONS[i].to_string(), p.display().to_string()));
            }
        }
    });
}

fn file_formats(ui: &mut Ui) {
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label("Available file types:");
            egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(4.0).show(ui, |ui| {
                ui.set_width(250.0);
                ui.set_min_height(400.0);
                egui::CollapsingHeader::new("Raster").default_open(true).show(ui, |ui| {
                    for t in vector_io::IMPORT_TYPES.iter().filter(|t| !t.vector) {
                        ui.label(t.name);
                    }
                });
                egui::CollapsingHeader::new("Vector").default_open(true).show(ui, |ui| {
                    for t in vector_io::IMPORT_TYPES.iter().filter(|t| t.vector) {
                        ui.label(t.name);
                    }
                    ui.label("PDF - Portable Document Format (*.pdf)");
                });
                egui::CollapsingHeader::new("Text").show(ui, |ui| {
                    ui.label("TXT - ANSI Text (*.txt)");
                });
                egui::CollapsingHeader::new("Animation").show(ui, |ui| {
                    ui.label("GIF - CompuServe Bitmap (*.gif)");
                });
            });
        });
        ui.vertical(|ui| {
            ui.add_space(20.0);
            for b in ["Add", "Remove", "Move Up", "Move Down"] {
                ui.add_enabled(false, egui::Button::new(b).min_size(vec2(80.0, 22.0)));
            }
        });
        ui.vertical(|ui| {
            ui.label("Active filters:");
            egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(4.0).show(ui, |ui| {
                ui.set_width(220.0);
                ui.set_min_height(400.0);
                for t in vector_io::IMPORT_TYPES.iter().chain(vector_io::EXPORT_TYPES.iter().filter(|t| t.extensions[0] == "pdf")) {
                    ui.label(t.extensions[0].to_uppercase());
                }
            });
            ui.add_enabled(false, egui::Button::new("Reset to Default"));
        });
    });
}

fn global_color(app: &mut VectorApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        label_w(ui, "Spot color palette:", 120.0);
        let mut i = 0;
        combo(ui, "spot_pal", &mut i, &["Vector Spot Colors"], 280.0);
    });
    separator(ui);
    heading(ui, "Color palette mapping");
    ui.label("Choose the palette or library to use when you open or import files that contain spot colors included in multiple palettes.");
    ui.horizontal(|ui| {
        label_w(ui, "Spot colors:", 120.0);
        let mut i = 0;
        combo(ui, "spot_map", &mut i, &["Vector Spot Color Library"], 280.0);
    });
    ui.horizontal(|ui| {
        ui.add_space(124.0);
        ui.checkbox(&mut app.settings.show_choose_palette, "Show Choose Color Palette dialog box");
    });
}

fn user_id(app: &mut VectorApp, ui: &mut Ui) {
    let s = &mut app.settings;
    ui.horizontal(|ui| {
        label_w(ui, "Name:", 90.0);
        ui.add(egui::TextEdit::singleline(&mut s.user_name).desired_width(280.0));
    });
    ui.horizontal(|ui| {
        label_w(ui, "Company:", 90.0);
        ui.add(egui::TextEdit::singleline(&mut s.user_company).desired_width(280.0));
    });
    note(ui, "Your name is used as the author of new documents and comments.");
}

fn privacy(app: &mut VectorApp, ui: &mut Ui) {
    ui.checkbox(&mut app.settings.usage_stats, "Share anonymous usage statistics to help improve Vector");
    note(ui, "Vector stores your documents and settings on this computer only. AI Generate sends only your prompt to the AI provider when you use it.");
    separator(ui);
    heading(ui, "AI Generate");
    ui.horizontal(|ui| {
        label_w(ui, "API key variable:", 130.0);
        ui.add(egui::TextEdit::singleline(&mut app.settings.ai_api_key_env).desired_width(220.0));
    });
    let has = crate::ai::api_key(app).is_some();
    note(ui, if has { "An API key was found in this environment variable." } else { "Set this environment variable to your API key to enable AI Generate. The key is never saved by Vector." });
}

// ------------------------------------------------------------------------------------------
// Workspaces
// ------------------------------------------------------------------------------------------

pub const WORKSPACES: [(&str, &str); 4] = [
    ("Default", "The standard workspace with all toolbars, the toolbox and dockers."),
    ("Lite", "A simplified workspace for new users with fewer tools and dockers."),
    ("Touch", "A workspace optimized for tablets with larger buttons."),
    ("Illustration", "A workspace for illustrators with the Objects and Properties dockers open."),
];

fn workspaces(app: &mut VectorApp, ui: &mut Ui) {
    let st = &mut app.dialogs.options;
    let s = &mut app.settings;
    ui.label("Workspaces:");
    egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(4.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(240.0);
        egui::Grid::new("ws_grid").num_columns(2).striped(true).spacing(vec2(16.0, 4.0)).show(ui, |ui| {
            for (i, (n, desc)) in WORKSPACES.iter().enumerate() {
                let cur = s.workspace == *n;
                if ui.selectable_label(st.workspace_sel == Some(i), format!("{}{n}", if cur { "● " } else { "   " })).clicked() {
                    st.workspace_sel = Some(i);
                }
                ui.label(egui::RichText::new(*desc).color(theme::TEXT_DIM));
                ui.end_row();
            }
        });
    });
    let mut set = None;
    ui.horizontal(|ui| {
        if ui.add_enabled(st.workspace_sel.is_some(), egui::Button::new("Set as current").min_size(vec2(100.0, 22.0))).clicked() {
            set = st.workspace_sel;
        }
    });
    if let Some(i) = set {
        app.run(Cmd::Workspace(i as u8));
    }
}
