//! File ▸ Export, Export For (Office, Web, WordPress), Publish to PDF and the format dialogs
//! that follow them: PDF Settings (with PDF presets), Export to <bitmap> and SVG Export.

use super::{button, combo, group, note, ok_button, row, tabs, window};
use crate::app::AnyVectorApp;
use crate::printing::Issue;
use crate::theme;
use egui::{vec2, Color32, Ui};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use vector_core::{Color, Document, ObjId};
use vector_io::pdf::{PdfColor, PdfCompat, PdfCompression, PdfOptions, PdfSecurity, PdfStart};

// ==========================================================================================
// PDF settings and presets
// ==========================================================================================

/// The PDF Settings that a PDF preset holds (security and the export range are never saved).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PdfSettings {
    pub compat: PdfCompat,
    pub only_on_page: bool,
    pub use_proof: bool,
    pub color: PdfColor,
    pub overprint_black: bool,
    pub hyperlinks: bool,
    pub bookmarks: bool,
    pub thumbnails: bool,
    pub start: PdfStart,
    pub comments: bool,
    pub ascii85: bool,
    pub compression: PdfCompression,
    pub jpeg_quality: u8,
    pub compress_text: bool,
    pub ds_color: Option<f64>,
    pub ds_gray: Option<f64>,
    pub ds_mono: Option<f64>,
    pub crop_marks: bool,
    pub registration_marks: bool,
    pub densitometer: bool,
    pub file_info: bool,
    /// Bleed limit in points.
    pub bleed: Option<f64>,
}

impl Default for PdfSettings {
    fn default() -> Self {
        preset_values("Document distribution").unwrap()
    }
}

fn base() -> PdfSettings {
    PdfSettings {
        compat: PdfCompat::Acrobat9,
        only_on_page: false,
        use_proof: false,
        color: PdfColor::Rgb,
        overprint_black: false,
        hyperlinks: true,
        bookmarks: true,
        thumbnails: false,
        start: PdfStart::PageOnly,
        comments: false,
        ascii85: false,
        compression: PdfCompression::Zip,
        jpeg_quality: 85,
        compress_text: true,
        ds_color: None,
        ds_gray: None,
        ds_mono: None,
        crop_marks: false,
        registration_marks: false,
        densitometer: false,
        file_info: false,
        bleed: None,
    }
}

/// The PDF presets that come with the program, in the reference order.
pub const PRESETS: [&str; 9] = ["Archiving (CMYK)", "Archiving (RGB)", "Current proof settings", "Document distribution", "Editing", "PDF/X-1a", "PDF/X-3", "Prepress", "Web"];

fn preset_values(name: &str) -> Option<PdfSettings> {
    let b = base();
    Some(match name {
        "Archiving (CMYK)" => PdfSettings { compat: PdfCompat::PdfA1b, color: PdfColor::Cmyk, ..b },
        "Archiving (RGB)" => PdfSettings { compat: PdfCompat::PdfA1b, color: PdfColor::Rgb, ..b },
        "Current proof settings" => PdfSettings { use_proof: true, ..b },
        "Document distribution" => PdfSettings { compression: PdfCompression::Jpeg, ..b },
        "Editing" => PdfSettings { compression: PdfCompression::Lzw, thumbnails: true, jpeg_quality: 100, ..b },
        "PDF/X-1a" => PdfSettings { compat: PdfCompat::PdfX1a, color: PdfColor::Cmyk, hyperlinks: false, bookmarks: false, ..b },
        "PDF/X-3" => PdfSettings { compat: PdfCompat::PdfX3, color: PdfColor::Native, hyperlinks: false, bookmarks: false, ..b },
        "Prepress" => PdfSettings { color: PdfColor::Native, hyperlinks: false, ..b },
        "Web" => PdfSettings { compression: PdfCompression::Jpeg, compress_text: true, ..b },
        _ => return user_preset(name),
    })
}

fn presets_dir() -> PathBuf {
    let d = crate::settings::config_dir().join("PDF Presets");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn user_presets() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(presets_dir())
        .map(|r| r.flatten().filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().to_string())).collect())
        .unwrap_or_default();
    v.retain(|n| !PRESETS.contains(&n.as_str()));
    v.sort();
    v
}

fn user_preset(name: &str) -> Option<PdfSettings> {
    let s = std::fs::read_to_string(presets_dir().join(format!("{name}.json"))).ok()?;
    serde_json::from_str(&s).ok()
}

fn all_presets() -> Vec<String> {
    PRESETS.iter().map(|s| s.to_string()).chain(user_presets()).collect()
}

const RANGES: [&str; 5] = ["Current document", "Documents", "Selection", "Current page", "Pages"];

pub struct PdfState {
    pub settings: PdfSettings,
    /// The preset the settings came from ("" once changed).
    pub preset: String,
    pub security: PdfSecurity,
    pub open_pw_on: bool,
    pub perm_pw_on: bool,
    pub range: usize,
    pub pages_text: String,
    pub docs_checked: Vec<bool>,
    /// Page size defined by the selected objects (Selection range).
    pub page_from_selection: bool,
    pub tab: usize,
    pub path: String,
    /// The Publish to PDF dialog is open; the Settings dialog opens over it.
    pub publish: bool,
    pub settings_open: bool,
    /// Settings as they were when the Settings dialog opened (restored on Cancel).
    before: Option<PdfSettings>,
    new_preset: Option<String>,
}

impl PdfState {
    pub fn new(app: &AnyVectorApp, publish: bool) -> PdfState {
        let d = app.doc();
        let stem = d.map(|d| d.stem()).unwrap_or_else(|| "Untitled".into());
        let dir = app.settings.last_export_dir.clone().or_else(|| app.settings.last_dir.clone()).unwrap_or_else(crate::settings::documents_dir);
        let preset = app.settings.pdf_preset.clone().unwrap_or_else(|| "Document distribution".into());
        let settings = app.settings.pdf.clone().unwrap_or_else(|| preset_values(&preset).unwrap_or_default());
        PdfState {
            settings,
            preset,
            security: PdfSecurity { printing: 2, editing: 2, copying: true, ..Default::default() },
            open_pw_on: false,
            perm_pw_on: false,
            range: if app.has_selection() && !publish { 2 } else { 0 },
            pages_text: String::new(),
            docs_checked: app.docs.iter().enumerate().map(|(i, _)| Some(i) == app.active).collect(),
            page_from_selection: true,
            tab: 0,
            path: dir.join(format!("{stem}.pdf")).display().to_string(),
            publish,
            settings_open: !publish,
            before: None,
            new_preset: None,
        }
    }

    fn options(&self) -> PdfOptions {
        let s = &self.settings;
        PdfOptions {
            pages: vec![],
            raster_dpi: s.ds_color.unwrap_or(300.0),
            compress: s.compress_text,
            jpeg_images: true,
            include_hyperlinks: s.hyperlinks,
            printing: true,
            compat: s.compat,
            color: s.color,
            bookmarks: s.bookmarks,
            thumbnails: s.thumbnails,
            start: s.start,
            comments: s.comments,
            ascii85: s.ascii85,
            image_compression: s.compression,
            jpeg_quality: s.jpeg_quality,
            downsample_color: s.ds_color,
            downsample_gray: s.ds_gray,
            downsample_mono: s.ds_mono,
            crop_marks: s.crop_marks,
            registration_marks: s.registration_marks,
            densitometer: s.densitometer,
            file_info: s.file_info,
            bleed: s.bleed.unwrap_or(0.0),
            only_on_page: s.only_on_page,
            overprint_black: s.overprint_black,
            security: (self.perm_pw_on && s.compat.allows_security()).then(|| PdfSecurity {
                open_password: if self.open_pw_on { self.security.open_password.clone() } else { String::new() },
                ..self.security.clone()
            }),
            file_name: Path::new(&self.path).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            id_seed: None,
        }
    }

    /// The document and pages the export range covers.
    fn source(&self, app: &AnyVectorApp) -> Option<(Document, Vec<usize>)> {
        let d = app.doc()?;
        let n = d.doc.pages.len();
        match self.range {
            1 => {
                let docs: Vec<&Document> = app.docs.iter().zip(&self.docs_checked).filter(|(_, c)| **c).map(|(v, _)| &v.doc).collect();
                let doc = combine_documents(&docs)?;
                let n = doc.pages.len();
                Some((doc, (0..n).collect()))
            }
            2 => {
                if self.page_from_selection {
                    Some((crate::exec::selection_document(&d.doc, &d.selection)?, vec![0]))
                } else {
                    Some((only_objects(&d.doc, d.page, &d.selection), vec![d.page]))
                }
            }
            3 => Some((d.doc.clone(), vec![d.page])),
            4 => {
                let p = super::forms::parse_page_range(&self.pages_text, n);
                (!p.is_empty()).then(|| (d.doc.clone(), p))
            }
            _ => Some((d.doc.clone(), (0..n).collect())),
        }
    }

    fn issues(&self, app: &AnyVectorApp) -> Vec<Issue> {
        let mut v = vec![];
        let s = &self.settings;
        let Some((doc, pages)) = self.source(app) else {
            v.push(Issue { title: "Nothing to export".into(), detail: "The export range does not contain any pages or objects.".into() });
            return v;
        };
        if !s.compat.transparency() && uses_transparency(&doc, &pages) {
            v.push(Issue {
                title: "Transparency will be flattened".into(),
                detail: format!("{} does not support transparency. Transparent objects are blended with white and drawn opaque.", s.compat.name()),
            });
        }
        if self.perm_pw_on && !s.compat.allows_security() {
            v.push(Issue { title: "Security is not available".into(), detail: format!("Passwords cannot be used with {}. The PDF is saved without security.", s.compat.name()) });
        }
        if self.open_pw_on && !self.perm_pw_on {
            v.push(Issue { title: "Open password without a permission password".into(), detail: "Set a permission password too; the open password is only used together with one.".into() });
        }
        if self.perm_pw_on && self.security.permission_password.is_empty() {
            v.push(Issue { title: "The permission password is empty".into(), detail: "Type a permission password on the Security tab.".into() });
        }
        if s.compat == PdfCompat::PdfA1b {
            v.push(Issue { title: "No output profile is embedded".into(), detail: "PDF/A-1b needs an embedded ICC output profile; the file is identified as PDF/A-1b but carries no profile.".into() });
        }
        if s.thumbnails && s.compat == PdfCompat::PdfA1b {
            v.push(Issue { title: "Thumbnails in PDF/A".into(), detail: "Thumbnails are allowed but make the file larger.".into() });
        }
        let off: usize = pages
            .iter()
            .filter_map(|p| doc.pages.get(*p))
            .flat_map(|pg| pg.layers.iter().flat_map(|l| l.objects.iter()).map(move |o| (pg, o)))
            .filter(|(pg, o)| {
                let b = o.bounds();
                let r = pg.rect();
                b.x1 < r.x0 || b.y1 < r.y0 || b.x0 > r.x1 || b.y0 > r.y1
            })
            .count();
        if off > 0 && !s.only_on_page {
            v.push(Issue {
                title: format!("{off} object(s) are outside the page"),
                detail: "Objects off the page are exported but not visible. Select Export only objects on page on the General tab to leave them out.".into(),
            });
        }
        v
    }
}

/// PDF options from the last used PDF settings (Export docker items, quick exports).
pub fn pdf_options(app: &AnyVectorApp) -> PdfOptions {
    let mut st = PdfState::new(app, false);
    st.perm_pw_on = false;
    st.options()
}

/// Copies of documents' pages in one document, with fresh ids (Export range ▸ Documents).
pub fn combine_documents(docs: &[&Document]) -> Option<Document> {
    let (first, rest) = docs.split_first()?;
    let mut out = (*first).clone();
    for d in rest {
        let mut images = std::collections::HashMap::new();
        for (old, data) in d.images.iter() {
            let new = out.add_image(data.clone());
            images.insert(*old, new);
        }
        let mut symbols = std::collections::HashMap::new();
        for s in &d.symbols {
            let mut s = s.clone();
            let new = out.new_id();
            symbols.insert(s.id, new);
            s.id = new;
            out.symbols.push(s);
        }
        for pg in &d.pages {
            let mut p = pg.clone();
            p.id = out.new_id();
            for l in p.layers.iter_mut() {
                l.id = out.new_id();
                for o in l.objects.iter_mut() {
                    out.reassign_ids(o);
                    o.walk_mut(&mut |x| match &mut x.kind {
                        vector_core::ObjKind::Image(i) => {
                            if let Some(n) = images.get(&i.image) {
                                i.image = *n;
                            }
                        }
                        vector_core::ObjKind::Symbol(s) => {
                            if let Some(n) = symbols.get(s) {
                                *s = *n;
                            }
                        }
                        _ => {}
                    });
                }
            }
            out.pages.push(p);
        }
    }
    Some(out)
}

/// A copy of the document with only `ids` left on page `page`.
fn only_objects(doc: &Document, page: usize, ids: &[ObjId]) -> Document {
    let mut out = doc.clone();
    if let Some(pg) = out.pages.get_mut(page) {
        for l in pg.layers.iter_mut() {
            l.objects.retain(|o| ids.contains(&o.id));
        }
    }
    out
}

fn uses_transparency(doc: &Document, pages: &[usize]) -> bool {
    let mut found = false;
    for p in pages.iter().filter_map(|p| doc.pages.get(*p)) {
        for l in &p.layers {
            for o in &l.objects {
                o.walk(&mut |x| {
                    if !matches!(x.style.transparency, vector_core::Transparency::None) {
                        found = true;
                    }
                });
            }
        }
    }
    found
}

// ------------------------------------------------------------------------------ dialogs

pub fn show_pdf(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.pdf.take() else { return };
    if !app.has_doc() {
        return;
    }
    let mut keep = true;
    if st.publish {
        keep = publish_dialog(app, ctx, &mut st);
    }
    if keep && st.settings_open {
        if st.before.is_none() {
            st.before = Some(st.settings.clone());
        }
        match settings_dialog(app, ctx, &mut st) {
            Some(true) => {
                st.settings_open = false;
                st.before = None;
                app.settings.pdf = Some(st.settings.clone());
                app.settings.pdf_preset = Some(st.preset.clone());
                if !st.publish {
                    run_pdf(app, &st);
                    keep = false;
                }
            }
            Some(false) => {
                if let Some(b) = st.before.take() {
                    st.settings = b;
                }
                st.settings_open = false;
                if !st.publish {
                    keep = false;
                }
            }
            None => {}
        }
    }
    if keep {
        app.dialogs.pdf = Some(st);
    }
}

/// File ▸ Publish to PDF: file name, PDF preset and Settings.
fn publish_dialog(app: &mut AnyVectorApp, ctx: &egui::Context, st: &mut PdfState) -> bool {
    let mut save = false;
    let mut cancel = false;
    let presets = all_presets();
    let ((), dismissed) = window(ctx, "publish_pdf", "Publish to PDF", vec2(600.0, 200.0), |ui| {
        ui.add_space(12.0);
        row(ui, "File name:", 100.0, |ui| {
            ui.add(egui::TextEdit::singleline(&mut st.path).desired_width(360.0));
            if button(ui, "Browse...", true) {
                let p = PathBuf::from(&st.path);
                let mut fd = rfd::FileDialog::new().set_title("Publish to PDF").add_filter("PDF - Portable Document Format (*.pdf)", &["pdf"]);
                if let Some(dir) = p.parent() {
                    fd = fd.set_directory(dir);
                }
                if let Some(name) = p.file_name() {
                    fd = fd.set_file_name(name.to_string_lossy());
                }
                if let Some(p) = fd.save_file() {
                    st.path = p.display().to_string();
                }
            }
        });
        row(ui, "Save as type:", 100.0, |ui| {
            ui.label("PDF - Portable Document Format (*.pdf)");
        });
        row(ui, "PDF preset:", 100.0, |ui| {
            preset_combo(ui, st, &presets, 280.0);
            if button(ui, "Settings...", true) {
                st.settings_open = true;
            }
        });
        let bottom = egui::Rect::from_min_max(egui::pos2(ui.max_rect().left(), ui.max_rect().bottom() - 24.0), ui.max_rect().right_bottom());
        ui.scope_builder(egui::UiBuilder::new().max_rect(bottom), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cancel = button(ui, "Cancel", true);
                ui.add_space(4.0);
                save = ok_button(ui, "Save", !st.path.trim().is_empty());
            });
        });
    });
    if save && !st.settings_open {
        if !st.path.to_lowercase().ends_with(".pdf") {
            st.path.push_str(".pdf");
        }
        app.settings.pdf = Some(st.settings.clone());
        app.settings.pdf_preset = Some(st.preset.clone());
        run_pdf(app, st);
        return false;
    }
    !(cancel || (dismissed && !st.settings_open))
}

fn preset_combo(ui: &mut Ui, st: &mut PdfState, presets: &[String], width: f32) {
    let label = if st.preset.is_empty() { "(Custom)".to_string() } else { st.preset.clone() };
    egui::ComboBox::from_id_salt("pdf_preset").selected_text(label).width(width).show_ui(ui, |ui| {
        for p in presets {
            if ui.selectable_label(st.preset == *p, p).clicked() {
                if let Some(v) = preset_values(p) {
                    st.settings = v;
                    st.preset = p.clone();
                }
            }
        }
    });
}

/// The PDF Settings dialog. Some(true) = OK, Some(false) = Cancel.
fn settings_dialog(app: &mut AnyVectorApp, ctx: &egui::Context, st: &mut PdfState) -> Option<bool> {
    let issues = st.issues(app);
    let has_sel = app.has_selection();
    let doc_names: Vec<String> = app.docs.iter().map(|d| d.title.clone()).collect();
    if st.docs_checked.len() != doc_names.len() {
        st.docs_checked.resize(doc_names.len(), false);
    }
    let before = st.settings.clone();
    let mut result = None;
    let title = if st.publish { "PDF Settings" } else { "Export PDF Settings" };
    let ((), dismissed) = window(ctx, "pdf_settings", title, vec2(620.0, 600.0), |ui| {
        ui.add_space(4.0);
        let pre = match issues.len() {
            0 => "No Issues".to_string(),
            1 => "1 Issue".to_string(),
            n => format!("{n} Issues"),
        };
        let names: Vec<String> = ["General", "Color", "Document", "Objects", "Prepress", "Security"].iter().map(|s| s.to_string()).chain([pre]).collect();
        tabs(ui, &mut st.tab, &names);
        egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| match st.tab {
            0 => general_tab(ui, st, has_sel, &doc_names),
            1 => color_tab(ui, st),
            2 => document_tab(ui, st),
            3 => objects_tab(ui, st),
            4 => prepress_tab(ui, st),
            5 => security_tab(ui, st),
            _ => super::print::preflight_list(ui, &issues),
        });
        let (ok, cancel) = super::ok_cancel_row(ui, "OK", st.range != 4 || !st.pages_text.trim().is_empty());
        if ok {
            result = Some(true);
        } else if cancel {
            result = Some(false);
        }
    });
    if st.settings != before && preset_values(&st.preset).as_ref() != Some(&st.settings) {
        st.preset.clear();
    }
    if dismissed && result.is_none() {
        result = Some(false);
    }
    result
}

fn general_tab(ui: &mut Ui, st: &mut PdfState, has_sel: bool, doc_names: &[String]) {
    ui.horizontal(|ui| {
        ui.label("File name:");
        ui.add(egui::TextEdit::singleline(&mut st.path).desired_width(440.0));
    });
    group(ui, "Export range", |ui| {
        for (i, n) in RANGES.iter().enumerate() {
            let enabled = i != 2 || has_sel;
            ui.horizontal(|ui| {
                ui.add_enabled_ui(enabled, |ui| ui.radio_value(&mut st.range, i, *n));
                if i == 4 {
                    ui.add_enabled(st.range == 4, egui::TextEdit::singleline(&mut st.pages_text).desired_width(160.0).hint_text("1-3, 5"));
                }
            });
            if i == 1 && st.range == 1 {
                for (k, name) in doc_names.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add_space(22.0);
                        ui.checkbox(&mut st.docs_checked[k], name);
                    });
                }
            }
        }
    });
    group(ui, "Page size", |ui| {
        ui.add_enabled_ui(st.range == 2, |ui| {
            ui.radio_value(&mut st.page_from_selection, false, "Defined by document");
            ui.radio_value(&mut st.page_from_selection, true, "Defined by selected objects");
        });
    });
    ui.add_space(6.0);
    row(ui, "Compatibility:", 110.0, |ui| {
        let mut i = PdfCompat::ALL.iter().position(|c| *c == st.settings.compat).unwrap_or(4);
        let names: Vec<&str> = PdfCompat::ALL.iter().map(|c| c.name()).collect();
        if combo(ui, "pdf_compat", &mut i, &names, 200.0) {
            st.settings.compat = PdfCompat::ALL[i];
            if st.settings.compat == PdfCompat::PdfX1a {
                st.settings.color = PdfColor::Cmyk;
            }
        }
    });
    ui.checkbox(&mut st.settings.only_on_page, "Export only objects on page");
    ui.add_space(8.0);
    let presets = all_presets();
    row(ui, "PDF preset:", 110.0, |ui| {
        preset_combo(ui, st, &presets, 240.0);
        if ui.button("+").on_hover_text("Add PDF preset").clicked() {
            st.new_preset = Some(String::new());
        }
        let user = !PRESETS.contains(&st.preset.as_str()) && !st.preset.is_empty();
        if ui.add_enabled(user, egui::Button::new("−")).on_hover_text("Delete PDF preset").clicked() {
            let _ = std::fs::remove_file(presets_dir().join(format!("{}.json", st.preset)));
            st.preset.clear();
        }
    });
    if let Some(name) = st.new_preset.as_mut() {
        let mut done = None;
        ui.horizontal(|ui| {
            ui.add_space(114.0);
            ui.add(egui::TextEdit::singleline(name).desired_width(200.0).hint_text("Preset name"));
            let n = name.trim().to_string();
            let valid = !n.is_empty() && !n.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) && !PRESETS.contains(&n.as_str());
            if ui.add_enabled(valid, egui::Button::new("Save")).clicked() {
                done = Some(Some(n));
            }
            if ui.button("Cancel").clicked() {
                done = Some(None);
            }
        });
        if let Some(r) = done {
            if let Some(n) = r {
                if let Ok(json) = serde_json::to_string_pretty(&st.settings) {
                    let _ = std::fs::write(presets_dir().join(format!("{n}.json")), json);
                    st.preset = n;
                }
            }
            st.new_preset = None;
        }
    }
}

fn color_tab(ui: &mut Ui, st: &mut PdfState) {
    let s = &mut st.settings;
    ui.radio_value(&mut s.use_proof, false, "Use document color settings");
    ui.radio_value(&mut s.use_proof, true, "Use color proof settings");
    group(ui, "Color management", |ui| {
        let x1a = s.compat == PdfCompat::PdfX1a;
        row(ui, "Output colors as:", 130.0, |ui| {
            for c in PdfColor::ALL {
                ui.add_enabled_ui(!x1a || c == PdfColor::Cmyk, |ui| ui.radio_value(&mut s.color, c, c.name()));
            }
        });
        row(ui, "Convert spot colors to:", 130.0, |ui| {
            ui.add_enabled(false, egui::Label::new(if s.color == PdfColor::Native { "RGB" } else { s.color.name() }));
        });
        ui.add_enabled(false, egui::Checkbox::new(&mut false, "Embed color profile")).on_disabled_hover_text("Color profiles are not embedded.");
    });
    group(ui, "Overprints", |ui| {
        ui.add_enabled(false, egui::Checkbox::new(&mut false, "Preserve document overprints")).on_disabled_hover_text("Object overprint settings are not exported.");
        ui.add_enabled(s.color == PdfColor::Cmyk || s.compat == PdfCompat::PdfX1a, egui::Checkbox::new(&mut s.overprint_black, "Always overprint black"));
    });
}

fn document_tab(ui: &mut Ui, st: &mut PdfState) {
    let s = &mut st.settings;
    ui.checkbox(&mut s.hyperlinks, "Include hyperlinks");
    ui.checkbox(&mut s.bookmarks, "Generate bookmarks");
    ui.checkbox(&mut s.thumbnails, "Generate thumbnails");
    ui.checkbox(&mut s.comments, "Include comments");
    group(ui, "On start, display", |ui| {
        for v in PdfStart::ALL {
            let enabled = match v {
                PdfStart::Bookmarks => s.bookmarks,
                PdfStart::Thumbnails => s.thumbnails,
                _ => true,
            };
            ui.add_enabled_ui(enabled, |ui| ui.radio_value(&mut s.start, v, v.name()));
        }
    });
    group(ui, "Encoding", |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut s.ascii85, true, "ASCII 85");
            ui.radio_value(&mut s.ascii85, false, "Binary");
        });
    });
    ui.add_enabled(false, egui::Checkbox::new(&mut false, "Optimize for web")).on_disabled_hover_text("Linearized (fast web view) files are not written.");
}

fn dpi_option(ui: &mut Ui, label: &str, v: &mut Option<f64>, default: f64) {
    ui.horizontal(|ui| {
        let mut on = v.is_some();
        if ui.add_sized(vec2(130.0, 20.0), egui::Checkbox::new(&mut on, label)).changed() {
            *v = on.then_some(default);
        }
        let mut x = v.unwrap_or(default);
        if ui.add_enabled(on, egui::DragValue::new(&mut x).range(10.0..=2400.0).suffix(" dpi")).changed() {
            *v = Some(x);
        }
    });
}

fn objects_tab(ui: &mut Ui, st: &mut PdfState) {
    let s = &mut st.settings;
    group(ui, "Bitmap compression", |ui| {
        row(ui, "Compression type:", 130.0, |ui| {
            let mut i = PdfCompression::ALL.iter().position(|c| *c == s.compression).unwrap_or(3);
            let names: Vec<&str> = PdfCompression::ALL.iter().map(|c| c.name()).collect();
            if combo(ui, "pdf_comp", &mut i, &names, 120.0) {
                s.compression = PdfCompression::ALL[i];
            }
        });
        row(ui, "JPEG quality:", 130.0, |ui| {
            ui.add_enabled(s.compression == PdfCompression::Jpeg, egui::Slider::new(&mut s.jpeg_quality, 2..=100));
        });
    });
    ui.checkbox(&mut s.compress_text, "Compress text and line art");
    group(ui, "Bitmap downsampling", |ui| {
        dpi_option(ui, "Color", &mut s.ds_color, 300.0);
        dpi_option(ui, "Grayscale", &mut s.ds_gray, 300.0);
        dpi_option(ui, "Monochrome", &mut s.ds_mono, 1200.0);
    });
    group(ui, "Text exporting", |ui| {
        ui.add_enabled(false, egui::Checkbox::new(&mut true, "Export all text as curves")).on_disabled_hover_text("Text is always written as curves.");
        ui.add_enabled(false, egui::Checkbox::new(&mut false, "Embed fonts in document"));
    });
}

fn prepress_tab(ui: &mut Ui, st: &mut PdfState) {
    let s = &mut st.settings;
    ui.horizontal(|ui| {
        let mut on = s.bleed.is_some();
        if ui.checkbox(&mut on, "Bleed limit:").changed() {
            s.bleed = on.then_some(9.0);
        }
        let mut inches = s.bleed.unwrap_or(9.0) / 72.0;
        if ui.add_enabled(on, egui::DragValue::new(&mut inches).range(0.0..=2.0).speed(0.01).max_decimals(3).suffix(" in")).changed() {
            s.bleed = Some(inches * 72.0);
        }
    });
    group(ui, "Printer's marks", |ui| {
        ui.checkbox(&mut s.crop_marks, "Crop marks");
        ui.checkbox(&mut s.registration_marks, "Registration marks");
        ui.checkbox(&mut s.densitometer, "Densitometer scales");
        ui.checkbox(&mut s.file_info, "File information");
    });
    ui.add_enabled(false, egui::Checkbox::new(&mut false, "Maintain OPI links"));
}

fn security_tab(ui: &mut Ui, st: &mut PdfState) {
    let allowed = st.settings.compat.allows_security();
    if !allowed {
        note(ui, &format!("Security options are not available for {}.", st.settings.compat.name()));
    }
    ui.add_enabled_ui(allowed, |ui| {
        group(ui, "Permission password", |ui| {
            ui.checkbox(&mut st.perm_pw_on, "Permission password");
            ui.add_enabled_ui(st.perm_pw_on, |ui| {
                row(ui, "Password:", 150.0, |ui| ui.add(egui::TextEdit::singleline(&mut st.security.permission_password).password(true).desired_width(220.0)));
                let pr = ["None", "Low resolution", "High resolution"];
                let mut i = st.security.printing as usize;
                row(ui, "Printing permissions:", 150.0, |ui| {
                    if combo(ui, "pdf_print_perm", &mut i, &pr, 220.0) {
                        st.security.printing = i as u8;
                    }
                });
                let ed = ["None", "Insert, delete, rotate pages", "Any except extracting pages"];
                let mut e = st.security.editing as usize;
                row(ui, "Editing permissions:", 150.0, |ui| {
                    if combo(ui, "pdf_edit_perm", &mut e, &ed, 220.0) {
                        st.security.editing = e as u8;
                    }
                });
                ui.checkbox(&mut st.security.copying, "Enable copying of content");
            });
        });
        group(ui, "Open password", |ui| {
            ui.add_enabled(st.perm_pw_on, egui::Checkbox::new(&mut st.open_pw_on, "Open password"));
            ui.add_enabled_ui(st.open_pw_on && st.perm_pw_on, |ui| {
                row(ui, "Password:", 150.0, |ui| ui.add(egui::TextEdit::singleline(&mut st.security.open_password).password(true).desired_width(220.0)));
            });
        });
    });
}

/// Write the PDF on a background thread.
fn run_pdf(app: &mut AnyVectorApp, st: &PdfState) {
    let Some((doc, pages)) = st.source(app) else {
        app.dialogs.message_box = Some(("Publish to PDF".into(), "Nothing to export in the selected range.".into()));
        return;
    };
    let mut opts = st.options();
    opts.pages = pages;
    let path = PathBuf::from(&st.path);
    app.settings.last_export_dir = path.parent().map(|p| p.to_path_buf());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let r = vector_io::pdf::export(&doc, &path, &opts).map(|_| format!("Published {}", path.display())).map_err(|e| format!("Could not publish:\n{e}"));
        let _ = tx.send(r);
    });
    app.background.push(rx);
    app.flash("Publishing to PDF...");
}

// ==========================================================================================
// File ▸ Export (the file step)
// ==========================================================================================

#[derive(Clone)]
pub struct ExportState {
    pub path: String,
    pub format: usize,
    pub selected_only: bool,
    pub this_page_only: bool,
    pub no_filter_dialog: bool,
}

impl ExportState {
    pub fn new(app: &AnyVectorApp) -> ExportState {
        let d = app.doc();
        let stem = d.map(|d| d.stem()).unwrap_or_else(|| "Untitled".into());
        let format = app.settings.export_format.min(vector_io::EXPORT_TYPES.len() - 1);
        let ext = vector_io::EXPORT_TYPES[format].extensions[0];
        let dir = app.settings.last_export_dir.clone().or_else(|| app.settings.last_dir.clone()).unwrap_or_else(crate::settings::documents_dir);
        ExportState {
            path: dir.join(format!("{stem}.{ext}")).display().to_string(),
            format,
            selected_only: app.has_selection(),
            this_page_only: true,
            no_filter_dialog: false,
        }
    }
}

pub fn show_export(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.export.take() else { return };
    if !app.has_doc() {
        return;
    }
    let types = vector_io::EXPORT_TYPES;
    let has_sel = app.has_selection();
    let multi = app.doc().map(|d| d.doc.pages.len() > 1).unwrap_or(false);
    let mut go = false;
    let mut cancel = false;
    let ((), dismissed) = window(ctx, "export", "Export", vec2(620.0, 250.0), |ui| {
        ui.add_space(12.0);
        row(ui, "File name:", 100.0, |ui| {
            ui.add(egui::TextEdit::singleline(&mut st.path).desired_width(380.0));
            if button(ui, "Browse...", true) {
                let t = &types[st.format];
                let p = PathBuf::from(&st.path);
                let mut fd = rfd::FileDialog::new().set_title("Export").add_filter(t.name, t.extensions);
                if let Some(dir) = p.parent() {
                    fd = fd.set_directory(dir);
                }
                if let Some(n) = p.file_name() {
                    fd = fd.set_file_name(n.to_string_lossy());
                }
                if let Some(p) = fd.save_file() {
                    st.path = p.display().to_string();
                }
            }
        });
        row(ui, "Save as type:", 100.0, |ui| {
            let names: Vec<&str> = types.iter().map(|t| t.name).collect();
            if combo(ui, "ex_type", &mut st.format, &names, 380.0) {
                let ext = types[st.format].extensions[0];
                st.path = PathBuf::from(&st.path).with_extension(ext).display().to_string();
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(104.0);
            ui.vertical(|ui| {
                ui.add_enabled(has_sel, egui::Checkbox::new(&mut st.selected_only, "Selected only"));
                ui.add_enabled(multi && !st.selected_only, egui::Checkbox::new(&mut st.this_page_only, "Export this page only"));
                ui.checkbox(&mut st.no_filter_dialog, "Do not show filter dialog");
            });
        });
        let bottom = egui::Rect::from_min_max(egui::pos2(ui.max_rect().left(), ui.max_rect().bottom() - 24.0), ui.max_rect().right_bottom());
        ui.scope_builder(egui::UiBuilder::new().max_rect(bottom), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cancel = button(ui, "Cancel", true);
                ui.add_space(4.0);
                go = ok_button(ui, "Export", !st.path.trim().is_empty());
            });
        });
    });
    if go {
        let t = &types[st.format];
        let mut path = PathBuf::from(st.path.trim());
        if !t.extensions.iter().any(|e| path.extension().map(|x| x.to_string_lossy().eq_ignore_ascii_case(e)).unwrap_or(false)) {
            path = path.with_extension(t.extensions[0]);
        }
        app.settings.export_format = st.format;
        app.settings.last_export_dir = path.parent().map(|p| p.to_path_buf());
        let sel = st.selected_only && has_sel;
        let d = app.doc().unwrap();
        let pages: Vec<usize> = if sel || st.this_page_only || !multi { vec![d.page] } else { (0..d.doc.pages.len()).collect() };
        match t.extensions[0] {
            "pdf" => {
                let mut p = PdfState::new(app, false);
                p.path = path.display().to_string();
                p.range = if sel {
                    2
                } else if pages.len() == 1 && multi {
                    3
                } else {
                    0
                };
                if st.no_filter_dialog {
                    run_pdf(app, &p);
                } else {
                    app.dialogs.pdf = Some(p);
                }
            }
            "svg" => {
                let s = SvgState { path, pages, selection: sel, unit: app.settings.svg_unit.min(3), dpi: 300.0 };
                if st.no_filter_dialog {
                    run_svg(app, &s);
                } else {
                    app.dialogs.svg = Some(s);
                }
            }
            "emf" => run_emf(app, &path, &pages, sel),
            ext => {
                let b = BitmapState::new(app, BitmapMode::Export, Some(path), ext, pages, sel);
                if st.no_filter_dialog {
                    write_bitmaps(app, &b);
                } else {
                    app.dialogs.bitmap = Some(b);
                }
            }
        }
        return;
    }
    if cancel || dismissed {
        return;
    }
    app.dialogs.export = Some(st);
}

// ==========================================================================================
// SVG Export
// ==========================================================================================

pub struct SvgState {
    pub path: PathBuf,
    pub pages: Vec<usize>,
    pub selection: bool,
    pub unit: usize,
    pub dpi: f64,
}

const SVG_UNITS: [&str; 4] = ["pt", "in", "mm", "px"];

pub fn show_svg(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.svg.take() else { return };
    let mut result = (false, false);
    let ((), dismissed) = window(ctx, "svg_export", "SVG Export", vec2(440.0, 300.0), |ui| {
        ui.add_space(10.0);
        row(ui, "Compatibility:", 120.0, |ui| ui.add_enabled(false, egui::Label::new("SVG 1.1")));
        row(ui, "Encoding:", 120.0, |ui| ui.add_enabled(false, egui::Label::new("Unicode (UTF-8)")));
        row(ui, "Styling:", 120.0, |ui| ui.add_enabled(false, egui::Label::new("Presentation attributes")));
        row(ui, "Units:", 120.0, |ui| {
            combo(ui, "svg_unit", &mut st.unit, &["Points", "Inches", "Millimeters", "Pixels"], 140.0);
        });
        row(ui, "Bitmap resolution:", 120.0, |ui| ui.add(egui::DragValue::new(&mut st.dpi).range(36.0..=1200.0).suffix(" dpi")));
        note(ui, "Bitmaps and effects that cannot be described in SVG are embedded as images at this resolution.");
        result = super::ok_cancel_row(ui, "OK", true);
    });
    if result.0 {
        app.settings.svg_unit = st.unit;
        run_svg(app, &st);
    } else if !(result.1 || dismissed) {
        app.dialogs.svg = Some(st);
    }
}

fn run_svg(app: &mut AnyVectorApp, st: &SvgState) {
    let Some(d) = app.doc() else { return };
    let (doc, pages) = if st.selection {
        match crate::exec::selection_document(&d.doc, &d.selection) {
            Some(sd) => (sd, vec![0]),
            None => return,
        }
    } else {
        (d.doc.clone(), st.pages.clone())
    };
    let opts = vector_io::svg_export::SvgOptions { raster_dpi: st.dpi, unit: SVG_UNITS[st.unit.min(3)], ..Default::default() };
    let res: Result<(), String> = pages.iter().try_for_each(|p| vector_io::svg_export::export(&doc, *p, &numbered(&st.path, *p, pages.len()), &opts).map_err(|e| e.to_string()));
    match res {
        Ok(()) => app.flash(format!("Exported {}", st.path.display())),
        Err(e) => app.dialogs.message_box = Some(("Export".into(), format!("Export failed:\n{e}"))),
    }
}

/// EMF export: an EMF+ dual metafile per page (curves, transparency and text as curves).
fn run_emf(app: &mut AnyVectorApp, path: &Path, pages: &[usize], selection: bool) {
    let Some(d) = app.doc() else { return };
    let (doc, pages) = if selection {
        match crate::exec::selection_document(&d.doc, &d.selection) {
            Some(sd) => (sd, vec![0]),
            None => return,
        }
    } else {
        (d.doc.clone(), pages.to_vec())
    };
    let opts = vector_io::emf::EmfOptions::default();
    let res: Result<(), String> = pages.iter().try_for_each(|p| vector_io::emf::export(&doc, *p, &numbered(path, *p, pages.len()), &opts).map_err(|e| e.to_string()));
    match res {
        Ok(()) => app.flash(format!("Exported {}", path.display())),
        Err(e) => app.dialogs.message_box = Some(("Export".into(), format!("Export failed:\n{e}"))),
    }
}

fn numbered(path: &Path, page: usize, count: usize) -> PathBuf {
    if count <= 1 {
        return path.to_path_buf();
    }
    let s = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
    path.with_file_name(format!("{s}-{}.{ext}", page + 1))
}

// ==========================================================================================
// Bitmap export: Export to <FMT>, Export For Web, Export For Office, Export For WordPress
// ==========================================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitmapMode {
    Export,
    Web,
    Office,
    WordPress,
}

pub struct BitmapState {
    pub mode: BitmapMode,
    pub path: Option<PathBuf>,
    pub ext: String,
    pub pages: Vec<usize>,
    pub selection: bool,
    /// The exported area of the first page, in document points.
    pub area: kurbo::Rect,
    pub dpi: f64,
    pub width: u32,
    pub height: u32,
    pub keep_ratio: bool,
    pub anti_alias: bool,
    pub transparent: bool,
    /// 0 = RGB color (24-bit), 1 = Grayscale (8-bit).
    pub color_mode: usize,
    pub quality: u8,
    pub office_target: usize,
    pub office_best: usize,
    pub office_for: usize,
    pub wp_site: String,
    pub wp_user: String,
    pub wp_password: String,
    tex: Option<egui::TextureHandle>,
    tex_key: u64,
    est: Option<usize>,
    est_key: u64,
}

const WEB_FORMATS: [&str; 4] = ["jpg", "png", "gif", "webp"];
const OFFICE_DPI: [f64; 3] = [96.0, 150.0, 300.0];

impl BitmapState {
    pub fn new(app: &AnyVectorApp, mode: BitmapMode, path: Option<PathBuf>, ext: &str, pages: Vec<usize>, selection: bool) -> BitmapState {
        let (area, page) = app
            .doc()
            .map(|d| {
                let r = if selection { d.doc.selection_bounds(&d.selection).unwrap_or_else(|| d.page_ref().rect()) } else { d.doc.pages[pages.first().copied().unwrap_or(d.page)].rect() };
                (r, d.page)
            })
            .unwrap_or((kurbo::Rect::new(0.0, 0.0, 612.0, 792.0), 0));
        let _ = page;
        let dpi = match mode {
            BitmapMode::Web | BitmapMode::WordPress => 96.0,
            BitmapMode::Office => 150.0,
            BitmapMode::Export => app.settings.export_dpi.unwrap_or(300.0),
        };
        let mut s = BitmapState {
            mode,
            path,
            ext: ext.to_string(),
            pages,
            selection,
            area,
            dpi,
            width: 0,
            height: 0,
            keep_ratio: true,
            anti_alias: true,
            transparent: matches!(ext, "png" | "webp" | "gif" | "tif") && mode != BitmapMode::Office,
            color_mode: 0,
            quality: 85,
            office_target: 0,
            office_best: 0,
            office_for: 1,
            wp_site: app.settings.wordpress_site.clone().unwrap_or_default(),
            wp_user: app.settings.wordpress_user.clone().unwrap_or_default(),
            wp_password: String::new(),
            tex: None,
            tex_key: 0,
            est: None,
            est_key: 0,
        };
        s.size_from_dpi();
        s
    }

    fn size_from_dpi(&mut self) {
        self.width = ((self.area.width() * self.dpi / 72.0).round() as u32).max(1);
        self.height = ((self.area.height() * self.dpi / 72.0).round() as u32).max(1);
    }

    fn key(&self, preview: bool) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.width, self.height, self.anti_alias, self.transparent, self.color_mode, &self.ext).hash(&mut h);
        if !preview {
            self.quality.hash(&mut h);
        }
        h.finish()
    }

    fn source(&self, app: &AnyVectorApp) -> Option<Document> {
        let d = app.doc()?;
        if self.selection {
            crate::exec::selection_document(&d.doc, &d.selection)
        } else {
            Some(d.doc.clone())
        }
    }

    fn title(&self) -> String {
        match self.mode {
            BitmapMode::Export => format!("Export to {}", self.ext.to_uppercase()),
            BitmapMode::Web => "Export For Web".into(),
            BitmapMode::Office => "Export For Office".into(),
            BitmapMode::WordPress => "Export For WordPress".into(),
        }
    }
}

/// Render one page (or the selection document's only page) with the bitmap settings.
fn render(doc: &Document, page: usize, st: &BitmapState, size: (u32, u32)) -> Option<(u32, u32, Vec<u8>)> {
    let bg = if st.transparent && matches!(st.ext.as_str(), "png" | "webp" | "gif" | "tif") { None } else { Some(Color::WHITE) };
    let opts = vector_io::image_io::RasterOptions { dpi: st.dpi, antialias: st.anti_alias, background: bg, jpeg_quality: st.quality, area: None, size: Some(size) };
    let (w, h, mut rgba) = vector_io::image_io::render_rgba(doc, page, &opts)?;
    if st.color_mode == 1 {
        for p in rgba.chunks_exact_mut(4) {
            let l = (0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64).round() as u8;
            p[0] = l;
            p[1] = l;
            p[2] = l;
        }
    }
    Some((w, h, rgba))
}

fn encode(doc: &Document, page: usize, st: &BitmapState) -> Result<Vec<u8>, String> {
    if st.ext == "emf" {
        // Export For Office ▸ Editing: vectors; the resolution is for effects kept as bitmaps
        return Ok(vector_io::emf::page_to_emf(doc, page, &vector_io::emf::EmfOptions { raster_dpi: st.dpi, ..Default::default() }));
    }
    let (w, h, rgba) = render(doc, page, st, (st.width, st.height)).ok_or("The image is too large.")?;
    vector_io::image_io::encode_raster(w, h, rgba, &st.ext, st.quality, st.dpi).map_err(|e| e.to_string())
}

fn fmt_size(n: usize) -> String {
    if n >= 1 << 20 {
        format!("{:.1} MB", n as f64 / (1 << 20) as f64)
    } else {
        format!("{:.0} KB", (n as f64 / 1024.0).max(1.0))
    }
}

pub fn show_bitmap(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.bitmap.take() else { return };
    let Some(doc) = st.source(app) else { return };
    let page0 = if st.selection { 0 } else { st.pages.first().copied().unwrap_or(0) };
    // preview texture
    let pkey = st.key(true);
    if st.tex.is_none() || st.tex_key != pkey {
        let scale = (420.0 / st.width.max(1) as f64).min(320.0 / st.height.max(1) as f64).min(1.0);
        let size = (((st.width as f64 * scale).round() as u32).max(1), ((st.height as f64 * scale).round() as u32).max(1));
        if let Some((w, h, rgba)) = render(&doc, page0, &st, size) {
            let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            st.tex = Some(ctx.load_texture("bitmap_export_preview", img, egui::TextureOptions::LINEAR));
        }
        st.tex_key = pkey;
    }
    // estimated file size (exact for images up to 4 megapixels)
    let ekey = st.key(false);
    if st.est_key != ekey {
        st.est = ((st.width as u64 * st.height as u64) <= 4_000_000).then(|| encode(&doc, page0, &st).ok().map(|b| b.len())).flatten();
        st.est_key = ekey;
    }
    let mut ok = false;
    let mut cancel = false;
    let title = st.title();
    let ((), dismissed) = window(ctx, "bitmap_export", &title, vec2(820.0, 560.0), |ui| {
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            // preview
            egui::Frame::new().fill(theme::FAINT()).stroke(egui::Stroke::new(1.0, theme::SEP())).show(ui, |ui| {
                ui.set_min_size(vec2(440.0, 440.0));
                ui.centered_and_justified(|ui| {
                    if let Some(t) = &st.tex {
                        let sz = t.size_vec2();
                        let (r, _) = ui.allocate_exact_size(sz, egui::Sense::hover());
                        if st.transparent {
                            checker(ui, r);
                        }
                        ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                    }
                });
            });
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.set_width(330.0);
                settings_panel(ui, &mut st);
                ui.add_space(8.0);
                let est = st.est.map(fmt_size).unwrap_or_else(|| "—".into());
                note(ui, &format!("{} × {} pixels · Estimated file size: {est}", st.width, st.height));
            });
        });
        let ok_text = match st.mode {
            BitmapMode::Web => "Save As...",
            BitmapMode::WordPress => "Upload",
            _ => "OK",
        };
        let ready = st.mode != BitmapMode::WordPress || (!st.wp_site.trim().is_empty() && !st.wp_user.trim().is_empty() && !st.wp_password.is_empty());
        let (o, c) = super::ok_cancel_row(ui, ok_text, ready);
        ok = o;
        cancel = c;
    });
    if ok {
        finish_bitmap(app, st);
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.bitmap = Some(st);
    }
}

fn checker(ui: &Ui, r: egui::Rect) {
    let s = 8.0;
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, Color32::WHITE);
    let (nx, ny) = ((r.width() / s).ceil() as i32, (r.height() / s).ceil() as i32);
    for y in 0..ny {
        for x in 0..nx {
            if (x + y) % 2 == 0 {
                p.rect_filled(egui::Rect::from_min_size(r.min + vec2(x as f32 * s, y as f32 * s), vec2(s, s)), 0.0, Color32::from_gray(0xcc));
            }
        }
    }
}

fn settings_panel(ui: &mut Ui, st: &mut BitmapState) {
    match st.mode {
        BitmapMode::Office => {
            group(ui, "Export to", |ui| {
                ui.radio_value(&mut st.office_target, 0, "Microsoft Office");
                ui.add_enabled_ui(false, |ui| ui.radio_value(&mut st.office_target, 1, "WordPerfect Office")).response.on_disabled_hover_text("WPG files are not written.");
            });
            group(ui, "Graphic best suited for", |ui| {
                ui.radio_value(&mut st.office_best, 0, "Compatibility");
                ui.radio_value(&mut st.office_best, 1, "Editing").on_hover_text("An EMF metafile: curves stay curves, editable in Office.");
            });
            row(ui, "Optimized for:", 110.0, |ui| {
                if combo(ui, "office_for", &mut st.office_for, &["Presentation", "Desktop printing", "Commercial printing"], 180.0) {
                    st.dpi = OFFICE_DPI[st.office_for];
                    st.size_from_dpi();
                }
            });
            st.ext = if st.office_best == 1 { "emf" } else { "png" }.into();
            st.transparent = false;
            return;
        }
        BitmapMode::Web | BitmapMode::WordPress => {
            let mut i = WEB_FORMATS.iter().position(|f| *f == st.ext).unwrap_or(0);
            row(ui, "Format:", 110.0, |ui| {
                if combo(ui, "web_fmt", &mut i, &["JPEG", "PNG", "GIF", "WEBP"], 140.0) {
                    st.ext = WEB_FORMATS[i].into();
                    st.transparent = st.transparent && st.ext != "jpg";
                }
            });
        }
        BitmapMode::Export => {
            row(ui, "Format:", 110.0, |ui| ui.label(st.ext.to_uppercase()));
        }
    }
    group(ui, "Color", |ui| {
        row(ui, "Color mode:", 100.0, |ui| {
            combo(ui, "bm_mode", &mut st.color_mode, &["RGB color (24-bit)", "Grayscale (8-bit)"], 180.0);
        });
        let alpha = matches!(st.ext.as_str(), "png" | "webp" | "gif" | "tif");
        ui.add_enabled(alpha, egui::Checkbox::new(&mut st.transparent, "Transparent background"));
    });
    if matches!(st.ext.as_str(), "jpg" | "webp") {
        row(ui, "Quality:", 110.0, |ui| ui.add(egui::Slider::new(&mut st.quality, 1..=100)));
    }
    group(ui, "Transformations", |ui| {
        let ratio = st.area.height() / st.area.width().max(1e-9);
        row(ui, "Width:", 90.0, |ui| {
            let mut w = st.width;
            if ui.add(egui::DragValue::new(&mut w).range(1..=30000).suffix(" px")).changed() {
                st.width = w;
                if st.keep_ratio {
                    st.height = ((w as f64 * ratio).round() as u32).max(1);
                }
                st.dpi = st.width as f64 / (st.area.width() / 72.0).max(1e-9);
            }
        });
        row(ui, "Height:", 90.0, |ui| {
            let mut h = st.height;
            if ui.add(egui::DragValue::new(&mut h).range(1..=30000).suffix(" px")).changed() {
                st.height = h;
                if st.keep_ratio {
                    st.width = ((h as f64 / ratio.max(1e-9)).round() as u32).max(1);
                }
                st.dpi = st.width as f64 / (st.area.width() / 72.0).max(1e-9);
            }
        });
        row(ui, "Resolution:", 90.0, |ui| {
            let mut d = st.dpi;
            if ui.add(egui::DragValue::new(&mut d).range(10.0..=2400.0).suffix(" dpi")).changed() {
                st.dpi = d;
                st.size_from_dpi();
            }
        });
        ui.checkbox(&mut st.keep_ratio, "Maintain aspect ratio");
    });
    ui.checkbox(&mut st.anti_alias, "Anti-aliasing");
    if st.mode == BitmapMode::WordPress {
        group(ui, "WordPress site", |ui| {
            row(ui, "Site address:", 100.0, |ui| ui.add(egui::TextEdit::singleline(&mut st.wp_site).hint_text("https://example.com").desired_width(200.0)));
            row(ui, "User name:", 100.0, |ui| ui.add(egui::TextEdit::singleline(&mut st.wp_user).desired_width(200.0)));
            row(ui, "Password:", 100.0, |ui| ui.add(egui::TextEdit::singleline(&mut st.wp_password).password(true).desired_width(200.0)));
            note(ui, "Use an application password (WordPress ▸ Users ▸ Profile).");
        });
    }
}

fn finish_bitmap(app: &mut AnyVectorApp, mut st: BitmapState) {
    match st.mode {
        BitmapMode::Export => write_bitmaps(app, &st),
        BitmapMode::Web | BitmapMode::Office => {
            let stem = app.doc().map(|d| d.stem()).unwrap_or_else(|| "Untitled".into());
            let ext = st.ext.clone();
            let t = vector_io::EXPORT_TYPES.iter().find(|t| t.extensions.contains(&ext.as_str()));
            let mut fd = rfd::FileDialog::new().set_title("Save As").set_file_name(format!("{stem}.{ext}"));
            if let Some(t) = t {
                fd = fd.add_filter(t.name, t.extensions);
            }
            if let Some(dir) = app.settings.last_export_dir.clone() {
                fd = fd.set_directory(dir);
            }
            if let Some(p) = fd.save_file() {
                app.settings.last_export_dir = p.parent().map(|p| p.to_path_buf());
                st.path = Some(p);
                write_bitmaps(app, &st);
            }
        }
        BitmapMode::WordPress => {
            let Some(doc) = st.source(app) else { return };
            let page = if st.selection { 0 } else { st.pages.first().copied().unwrap_or(0) };
            let bytes = match encode(&doc, page, &st) {
                Ok(b) => b,
                Err(e) => {
                    app.dialogs.message_box = Some(("Export For WordPress".into(), e));
                    return;
                }
            };
            app.settings.wordpress_site = Some(st.wp_site.trim().to_string());
            app.settings.wordpress_user = Some(st.wp_user.trim().to_string());
            let stem = app.doc().map(|d| d.stem()).unwrap_or_else(|| "image".into());
            let name = format!("{stem}.{}", st.ext);
            let (site, user, pw, ext) = (st.wp_site.trim().to_string(), st.wp_user.trim().to_string(), st.wp_password.clone(), st.ext.clone());
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(wordpress_upload(&site, &user, &pw, &name, &ext, bytes));
            });
            app.background.push(rx);
            app.flash("Uploading to WordPress...");
        }
    }
}

fn write_bitmaps(app: &mut AnyVectorApp, st: &BitmapState) {
    let Some(path) = st.path.clone() else { return };
    let Some(doc) = st.source(app) else { return };
    if st.mode == BitmapMode::Export {
        app.settings.export_dpi = Some(st.dpi);
    }
    let pages = if st.selection { vec![0] } else { st.pages.clone() };
    let mut result = Ok(());
    for p in &pages {
        match encode(&doc, *p, st).and_then(|b| std::fs::write(numbered(&path, *p, pages.len()), b).map_err(|e| e.to_string())) {
            Ok(()) => {}
            Err(e) => {
                result = Err(e);
                break;
            }
        }
    }
    match result {
        Ok(()) => app.flash(format!("Exported {}", path.display())),
        Err(e) => app.dialogs.message_box = Some(("Export".into(), format!("Export failed:\n{e}"))),
    }
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Upload an image to a WordPress media library (REST API, application password).
fn wordpress_upload(site: &str, user: &str, password: &str, name: &str, ext: &str, bytes: Vec<u8>) -> Result<String, String> {
    let site = if site.starts_with("http://") || site.starts_with("https://") { site.to_string() } else { format!("https://{site}") };
    let url = format!("{}/wp-json/wp/v2/media", site.trim_end_matches('/'));
    let mime = match ext {
        "jpg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        _ => "image/webp",
    };
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(std::time::Duration::from_secs(120))).build().into();
    let mut resp = agent
        .post(&url)
        .header("Authorization", format!("Basic {}", base64(format!("{user}:{password}").as_bytes())))
        .header("Content-Disposition", format!("attachment; filename=\"{name}\""))
        .header("Content-Type", mime)
        .send(&bytes[..])
        .map_err(|e| format!("Could not reach the WordPress site:\n{e}"))?;
    let status = resp.status().as_u16();
    let body: serde_json::Value = resp.body_mut().read_json().unwrap_or(serde_json::Value::Null);
    if (200..300).contains(&status) {
        let link = body.get("source_url").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!("Uploaded to WordPress: {link}"))
    } else {
        let msg = body.get("message").and_then(|v| v.as_str()).unwrap_or("The upload was refused.");
        Err(format!("WordPress returned an error ({status}):\n{msg}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_and_base64() {
        for p in PRESETS {
            assert!(preset_values(p).is_some(), "{p}");
        }
        assert_eq!(preset_values("PDF/X-1a").unwrap().color, PdfColor::Cmyk);
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"user:pass word"), "dXNlcjpwYXNzIHdvcmQ=");
    }

    #[test]
    fn combine_keeps_images_apart() {
        let mut a = Document::new(100.0, 100.0);
        let mut b = Document::new(200.0, 100.0);
        let img = |px: Vec<u8>| vector_core::document::ImageData { width: 1, height: 1, rgba: std::sync::Arc::new(px), encoded: Default::default(), format: "png".into(), name: String::new() };
        let ia = a.add_image(img(vec![255, 0, 0, 255]));
        let ib = b.add_image(img(vec![0, 0, 255, 255]));
        let id = b.new_id();
        b.pages[0].layers[0].objects.push(vector_core::Object::new(id, vector_core::ObjKind::Image(vector_core::object::ImageObj { image: ib, w: 1.0, h: 1.0, crop: [0.0, 0.0, 1.0, 1.0], adjust: Default::default() }), kurbo::Affine::IDENTITY, Default::default()));
        let c = combine_documents(&[&a, &b]).unwrap();
        assert_eq!(c.pages.len(), 2);
        assert_eq!(c.pages[1].width, 200.0);
        let o = &c.pages[1].layers[0].objects[0];
        let vector_core::ObjKind::Image(i) = &o.kind else { panic!() };
        assert_ne!(i.image, ia);
        assert_eq!(c.images[&i.image].rgba[2], 255);
    }
}
