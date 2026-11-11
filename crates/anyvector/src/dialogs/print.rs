//! File ▸ Print (General, Color, Composite, Layout, Prepress and Preflight tabs) and
//! File ▸ Print Preview. The printing itself is in `crate::printing`.

use super::{combo, group, row, tabs, window};
use crate::app::AnyVectorApp;
use crate::printing::{self, Issue, JobDoc, Orientation, OutputColor, Overprints, Paper, Parity, Placement, PrintSettings, Range, ToFile, ANCHORS};
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use std::collections::HashMap;

pub struct PrintState {
    pub settings: PrintSettings,
    pub printer: String,
    pub printers: Vec<String>,
    /// Printer preferences chosen in this dialog (a DEVMODE).
    pub devmode: Option<Vec<u8>>,
    pub style: String,
    pub tab: usize,
    pub mini_preview: bool,
    /// Which open documents print with Range::Documents.
    pub docs_checked: Vec<bool>,
    /// The printer's paper, per orientation.
    paper: HashMap<bool, Paper>,
    /// Print Preview is open (the Print dialog is hidden meanwhile).
    pub preview: Option<PreviewState>,
    /// The dialog was opened from Print Merge ▸ Print Merged Document.
    pub merge: bool,
    issues: Option<(u64, Vec<Issue>)>,
}

#[derive(Default)]
pub struct PreviewState {
    pub sheet: usize,
    /// 0 = to fit, else percent.
    pub zoom: u32,
    tex: Option<(u64, egui::TextureHandle)>,
    /// Return to the Print dialog when closing.
    pub from_dialog: bool,
}

impl PrintState {
    pub fn new(app: &AnyVectorApp, preview: bool) -> PrintState {
        let printers = printing::printers();
        let printer = app.settings.printer.clone().filter(|p| printers.contains(p)).or_else(printing::default_printer).or_else(|| printers.first().cloned()).unwrap_or_default();
        let mut settings = app.settings.print.clone();
        if app.settings.print_only_current {
            settings.range = Range::CurrentPage;
        }
        if settings.range == Range::Selection && !app.has_selection() {
            settings.range = Range::CurrentDocument;
        }
        PrintState {
            settings,
            printer,
            printers,
            devmode: None,
            style: printing::DEFAULT_STYLE.into(),
            tab: 0,
            mini_preview: false,
            docs_checked: app.docs.iter().map(|_| true).collect(),
            paper: HashMap::new(),
            preview: preview.then(PreviewState::default),
            merge: false,
            issues: None,
        }
    }

    fn paper(&mut self, landscape: bool) -> Paper {
        if self.printer.is_empty() {
            return Paper::fallback(landscape);
        }
        let (pr, dm) = (self.printer.clone(), self.devmode.clone());
        *self.paper.entry(landscape).or_insert_with(|| printing::paper_for(&pr, dm.as_deref(), landscape))
    }
}

/// The documents and pages a print would send, as a job (without the printer chosen yet).
pub fn job(app: &AnyVectorApp, st: &mut PrintState) -> Option<printing::Job> {
    let ai = app.active?;
    let s = &st.settings;
    let mut docs = vec![];
    let mut pages = vec![];
    let push_doc = |docs: &mut Vec<JobDoc>, i: usize, only: Option<Vec<vector_core::ObjId>>| {
        let d = &app.docs[i];
        let mut doc = d.doc.clone();
        if st.merge {
            doc = crate::exec::merged_document(&d.doc).unwrap_or(doc);
        }
        docs.push(JobDoc { doc, name: d.display_name().trim_end_matches('*').to_string(), only });
        docs.len() - 1
    };
    match s.range {
        Range::CurrentDocument => {
            let k = push_doc(&mut docs, ai, None);
            pages.extend((0..docs[k].doc.pages.len()).map(|p| (k, p)));
        }
        Range::Documents => {
            for i in 0..app.docs.len() {
                if st.docs_checked.get(i).copied().unwrap_or(true) {
                    let k = push_doc(&mut docs, i, None);
                    pages.extend((0..docs[k].doc.pages.len()).map(|p| (k, p)));
                }
            }
        }
        Range::CurrentPage => {
            let k = push_doc(&mut docs, ai, None);
            pages.push((k, app.docs[ai].page));
        }
        Range::Selection => {
            let d = &app.docs[ai];
            let k = push_doc(&mut docs, ai, Some(d.selection.clone()));
            pages.push((k, d.page));
        }
        Range::Pages => {
            let k = push_doc(&mut docs, ai, None);
            let n = docs[k].doc.pages.len();
            pages.extend(printing::page_range(&s.pages, n, s.parity).into_iter().map(|p| (k, p)));
        }
    }
    Some(printing::Job { printer: st.printer.clone(), devmode: st.devmode.clone(), docs, pages, settings: st.settings.clone(), file: None, title: app.docs[ai].display_name().trim_end_matches('*').to_string() })
}

/// The sheets of a job with this state's paper.
fn sheets(st: &mut PrintState, job: &printing::Job) -> Vec<(printing::Sheet, Paper)> {
    let list = job.page_list();
    let portrait = st.paper(false);
    let landscape = st.paper(true);
    printing::layout(&st.settings, &list, &|l| if l { landscape } else { portrait })
}

/// Start printing `job` on a background thread; the result shows as a status message.
pub fn send(app: &mut AnyVectorApp, mut job: printing::Job) {
    if job.printer.is_empty() {
        app.dialogs.message_box = Some(("Print".into(), "There is no printer installed. Add a printer in Windows Settings > Bluetooth & devices > Printers & scanners.".into()));
        return;
    }
    if job.settings.to_file {
        let ext = "prn";
        let name = format!("{}.{ext}", job.title.trim_end_matches(".cdr"));
        match rfd::FileDialog::new().set_title("Print to File").add_filter("Print file (*.prn)", &["prn"]).add_filter("PostScript file (*.ps)", &["ps"]).set_file_name(name).save_file() {
            Some(p) => job.file = Some(p),
            None => return,
        }
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(printing::run_job(job));
    });
    app.background.push(rx);
    app.flash("Printing...");
}

fn issues(app: &AnyVectorApp, st: &mut PrintState) -> Vec<Issue> {
    let Some(j) = job(app, st) else { return vec![] };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    format!("{:?}{}{:?}", st.settings, st.printer, j.pages).hash(&mut h);
    app.doc().map(|d| d.history.revision).hash(&mut h);
    let key = h.finish();
    if let Some((k, v)) = &st.issues {
        if *k == key {
            return v.clone();
        }
    }
    let paper = st.paper(false);
    let mut v = vec![];
    for (di, jd) in j.docs.iter().enumerate() {
        let pages: Vec<usize> = j.pages.iter().filter(|(d, _)| *d == di).map(|(_, p)| *p).collect();
        v.extend(printing::preflight(&jd.doc, &pages, &st.settings, &paper));
    }
    st.issues = Some((key, v.clone()));
    v
}

pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.print.take() else { return };
    if app.doc().is_none() {
        return;
    }
    if st.preview.is_some() {
        preview(app, ctx, st);
        return;
    }
    let found = issues(app, &mut st);
    let names: Vec<String> = ["General", "Color", "Composite", "Layout", "Prepress"].iter().map(|s| s.to_string()).chain(std::iter::once(if found.is_empty() { "No Issues".to_string() } else { format!("{} Issue{}", found.len(), if found.len() == 1 { "" } else { "s" }) })).collect();
    let width = if st.mini_preview { 860.0 } else { 560.0 };
    let mut action = None;
    let has_sel = app.has_selection();
    let doc_names: Vec<String> = app.docs.iter().map(|d| d.display_name()).collect();
    let unit = app.doc().map(|d| d.doc.settings.unit).unwrap_or(vector_core::units::Unit::Inches);
    let mini_tex = if st.mini_preview { mini_texture(app, ctx, &mut st) } else { None };
    let ((), dismissed) = window(ctx, "print", if st.merge { "Print Merge" } else { "Print" }, vec2(width, 600.0), |ui| {
        ui.add_space(4.0);
        let body = Rect::from_min_max(ui.max_rect().min, pos2(ui.max_rect().left() + 540.0, ui.max_rect().bottom() - 36.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
            tabs(ui, &mut st.tab, &names);
            ui.spacing_mut().item_spacing.y = 6.0;
            match st.tab {
                0 => general(ui, &mut st, has_sel, &doc_names),
                1 => color(ui, &mut st),
                2 => composite(ui, &mut st),
                3 => layout_tab(ui, &mut st, unit),
                4 => prepress(ui, &mut st),
                _ => preflight_list(ui, &found),
            }
        });
        if let Some(tex) = &mini_tex {
            let r = Rect::from_min_max(pos2(body.right() + 12.0, body.top() + 4.0), pos2(ui.max_rect().right(), body.bottom()));
            ui.painter().rect_filled(r, 0.0, Color32::from_rgb(0x1c, 0x1c, 0x1c));
            let s = tex.size_vec2();
            let k = ((r.width() - 20.0) / s.x).min((r.height() - 20.0) / s.y);
            let ir = Rect::from_center_size(r.center(), s * k);
            ui.painter().rect_filled(ir.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(120));
            ui.painter().image(tex.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        // bottom row
        let bottom = Rect::from_min_max(pos2(ui.max_rect().left(), ui.max_rect().bottom() - 26.0), ui.max_rect().right_bottom());
        ui.scope_builder(egui::UiBuilder::new().max_rect(bottom), |ui| {
            ui.horizontal(|ui| {
                if super::button(ui, "Print Preview", true) {
                    action = Some("preview");
                }
                let arrow = if st.mini_preview { "◂" } else { "▸" };
                if ui.add(egui::Button::new(arrow).min_size(vec2(24.0, 24.0))).on_hover_text("Mini preview").clicked() {
                    st.mini_preview = !st.mini_preview;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if super::button(ui, "Apply", true) {
                        action = Some("apply");
                    }
                    if super::button(ui, "Cancel", true) {
                        action = Some("cancel");
                    }
                    ui.add_space(4.0);
                    if super::ok_button(ui, "Print", !st.printer.is_empty()) {
                        action = Some("print");
                    }
                });
            });
        });
    });
    let keep = |app: &mut AnyVectorApp, st: &PrintState| {
        app.settings.print = st.settings.clone();
        if !st.printer.is_empty() {
            app.settings.printer = Some(st.printer.clone());
        }
    };
    match action {
        Some("print") => {
            keep(app, &st);
            if let Some(j) = job(app, &mut st) {
                send(app, j);
            }
        }
        Some("apply") => {
            keep(app, &st);
            app.dialogs.print = Some(st);
        }
        Some("preview") => {
            st.preview = Some(PreviewState { from_dialog: true, ..Default::default() });
            app.dialogs.print = Some(st);
        }
        Some("cancel") => {}
        _ if dismissed => {}
        _ => app.dialogs.print = Some(st),
    }
}

fn general(ui: &mut Ui, st: &mut PrintState, has_sel: bool, doc_names: &[String]) {
    group(ui, "Destination", |ui| {
        row(ui, "Printer:", 70.0, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("pr_printer").selected_text(if st.printer.is_empty() { "(no printer)" } else { &st.printer }).width(300.0).show_ui(ui, |ui| {
                for p in &st.printers {
                    if ui.selectable_label(st.printer == *p, p).clicked() && st.printer != *p {
                        st.printer = p.clone();
                        changed = true;
                    }
                }
            });
            if changed {
                st.devmode = None;
                st.paper.clear();
            }
            if super::button(ui, "Preferences...", !st.printer.is_empty()) {
                if let Some(dm) = printing::open_preferences(&st.printer, st.devmode.as_deref()) {
                    st.devmode = Some(dm);
                    st.paper.clear();
                }
            }
        });
        row(ui, "Page:", 70.0, |ui| {
            let mut i = Orientation::ALL.iter().position(|o| *o == st.settings.orientation).unwrap_or(0);
            let names: Vec<&str> = Orientation::ALL.iter().map(|o| o.name()).collect();
            if combo(ui, "pr_orient", &mut i, &names, 300.0) {
                st.settings.orientation = Orientation::ALL[i];
            }
        });
        let p = st.paper(false);
        row(ui, "Paper:", 70.0, |ui| {
            ui.label(egui::RichText::new(format!("{:.2} × {:.2} in.", p.width / 72.0, p.height / 72.0)).color(theme::TEXT_DIM()));
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut st.settings.to_file, "Print to file");
            ui.add_enabled_ui(st.settings.to_file, |ui| {
                let mut i = ToFile::ALL.iter().position(|o| *o == st.settings.file_mode).unwrap_or(0);
                let names: Vec<&str> = ToFile::ALL.iter().map(|o| o.name()).collect();
                if combo(ui, "pr_file", &mut i, &names, 180.0) {
                    st.settings.file_mode = ToFile::ALL[i];
                }
            });
        });
    });
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(300.0);
            group(ui, "Print range", |ui| {
                let s = &mut st.settings;
                ui.radio_value(&mut s.range, Range::CurrentDocument, "Current document");
                ui.radio_value(&mut s.range, Range::Documents, "Documents");
                if s.range == Range::Documents {
                    for (i, n) in doc_names.iter().enumerate() {
                        if let Some(c) = st.docs_checked.get_mut(i) {
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                ui.checkbox(c, n);
                            });
                        }
                    }
                }
                let s = &mut st.settings;
                ui.radio_value(&mut s.range, Range::CurrentPage, "Current page");
                ui.add_enabled_ui(has_sel, |ui| ui.radio_value(&mut s.range, Range::Selection, "Selection"));
                ui.horizontal(|ui| {
                    ui.radio_value(&mut s.range, Range::Pages, "Pages:");
                    ui.add_enabled(s.range == Range::Pages, egui::TextEdit::singleline(&mut s.pages).hint_text("1-3, 5").desired_width(70.0));
                    ui.add_enabled_ui(s.range == Range::Pages, |ui| {
                        let mut i = Parity::ALL.iter().position(|o| *o == s.parity).unwrap_or(0);
                        let names: Vec<&str> = Parity::ALL.iter().map(|o| o.name()).collect();
                        if combo(ui, "pr_parity", &mut i, &names, 90.0) {
                            s.parity = Parity::ALL[i];
                        }
                    });
                });
            });
        });
        ui.vertical(|ui| {
            group(ui, "Copies", |ui| {
                row(ui, "Number of copies:", 112.0, |ui| ui.add(egui::DragValue::new(&mut st.settings.copies).range(1..=999)));
                ui.add_enabled(st.settings.copies > 1, egui::Checkbox::new(&mut st.settings.collate, "Collate"));
            });
        });
    });
    ui.add_space(6.0);
    row(ui, "Print style:", 70.0, |ui| {
        let mut chosen = None;
        egui::ComboBox::from_id_salt("pr_style").selected_text(&st.style).width(300.0).show_ui(ui, |ui| {
            if ui.selectable_label(st.style == printing::DEFAULT_STYLE, printing::DEFAULT_STYLE).clicked() {
                chosen = Some((printing::DEFAULT_STYLE.to_string(), PrintSettings::default()));
            }
            for n in printing::style_names() {
                if ui.selectable_label(st.style == n, &n).clicked() {
                    if let Some(s) = printing::load_style(&printing::style_path(&n)) {
                        chosen = Some((n.clone(), s));
                    }
                }
            }
            if ui.selectable_label(false, "Browse...").clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("Print style (*.prs)", &["prs"]).set_directory(printing::styles_dir()).pick_file() {
                    if let Some(s) = printing::load_style(&p) {
                        chosen = Some((p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), s));
                    }
                }
            }
        });
        if let Some((n, s)) = chosen {
            st.style = n;
            st.settings = s;
        }
        if super::button(ui, "Save As...", true) {
            let _ = std::fs::create_dir_all(printing::styles_dir());
            if let Some(p) = rfd::FileDialog::new().set_title("Save Settings As").add_filter("Print style (*.prs)", &["prs"]).set_directory(printing::styles_dir()).set_file_name(format!("{}.prs", st.style)).save_file() {
                if printing::save_style(&p, &st.settings).is_ok() {
                    st.style = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                }
            }
        }
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut st.settings.as_bitmap, "Print as bitmap:");
        ui.add_enabled(st.settings.as_bitmap, egui::DragValue::new(&mut st.settings.bitmap_dpi).range(72..=1200).suffix(" dpi"));
    });
}

fn color(ui: &mut Ui, st: &mut PrintState) {
    let s = &mut st.settings;
    let mut composite = true;
    ui.radio_value(&mut composite, true, "Print composite");
    ui.add_enabled(false, egui::RadioButton::new(false, "Print separations")).on_disabled_hover_text("Separations need a PostScript printer.");
    super::separator(ui);
    if ui.radio(!s.color_proof, "Use document color settings").clicked() {
        s.color_proof = false;
    }
    if ui.radio(s.color_proof, "Use color proof settings").clicked() {
        s.color_proof = true;
    }
    super::separator(ui);
    row(ui, "Output colors as:", 130.0, |ui| {
        let mut i = OutputColor::ALL.iter().position(|o| *o == s.output).unwrap_or(0);
        let names: Vec<&str> = OutputColor::ALL.iter().map(|o| o.name()).collect();
        if combo(ui, "pr_output", &mut i, &names, 200.0) {
            s.output = OutputColor::ALL[i];
        }
    });
    row(ui, "Rendering intent:", 130.0, |ui| {
        let all = vector_core::document::RenderingIntent::ALL;
        let mut i = all.iter().position(|o| *o == s.intent).unwrap_or(1);
        let names: Vec<&str> = all.iter().map(|o| o.name()).collect();
        if combo(ui, "pr_intent", &mut i, &names, 200.0) {
            s.intent = all[i];
        }
    });
    super::note(ui, "This printer accepts RGB or grayscale color.");
}

fn composite(ui: &mut Ui, st: &mut PrintState) {
    let s = &mut st.settings;
    row(ui, "Document overprints:", 140.0, |ui| {
        let mut i = Overprints::ALL.iter().position(|o| *o == s.overprints).unwrap_or(0);
        let names: Vec<&str> = Overprints::ALL.iter().map(|o| o.name()).collect();
        if combo(ui, "pr_overprints", &mut i, &names, 160.0) {
            s.overprints = Overprints::ALL[i];
        }
    });
    let mut off = false;
    ui.add_enabled(false, egui::Checkbox::new(&mut off, "Always overprint black"));
    group(ui, "Trapping", |ui| {
        ui.add_enabled(false, egui::Checkbox::new(&mut off, "Auto-spreading"));
        ui.add_enabled(false, egui::Checkbox::new(&mut off, "PostScript 3 In-RIP trapping"));
    });
    super::note(ui, "Trapping and overprinting black need a PostScript printer.");
}

fn layout_tab(ui: &mut Ui, st: &mut PrintState, unit: vector_core::units::Unit) {
    let s = &mut st.settings;
    group(ui, "Image position and size", |ui| {
        ui.radio_value(&mut s.placement, Placement::AsInDocument, "As in document");
        ui.radio_value(&mut s.placement, Placement::FitToPage, "Fit to page");
        ui.horizontal(|ui| {
            ui.radio_value(&mut s.placement, Placement::Reposition, "Reposition artwork to:");
            ui.add_enabled_ui(s.placement == Placement::Reposition, |ui| {
                combo(ui, "pr_anchor", &mut s.anchor, &ANCHORS, 140.0);
            });
        });
        ui.add_enabled_ui(s.placement == Placement::Reposition, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.label("Position:");
                crate::widgets::unit_field(ui, ui.scope_id().with("pr_x"), &mut s.offset.0, unit, 300.0, 70.0, true);
                crate::widgets::unit_field(ui, ui.scope_id().with("pr_y"), &mut s.offset.1, unit, 300.0, 70.0, true);
                ui.label("Scale:");
                ui.add(egui::DragValue::new(&mut s.scale).range(1.0..=1000.0).suffix(" %"));
            });
        });
    });
    group(ui, "Tiling", |ui| {
        ui.checkbox(&mut s.tiled, "Print tiled pages");
        ui.add_enabled_ui(s.tiled, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.label("Tile overlap:");
                crate::widgets::unit_field(ui, ui.scope_id().with("pr_overlap"), &mut s.tile_overlap, unit, 300.0, 80.0, true);
            });
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.checkbox(&mut s.tiling_marks, "Tiling marks");
            });
        });
    });
    row(ui, "Bleed limit:", 80.0, |ui| crate::widgets::unit_field(ui, ui.scope_id().with("pr_bleed"), &mut s.bleed_limit, unit, 300.0, 80.0, true));
    row(ui, "Imposition layout:", 120.0, |ui| {
        let mut i = 0;
        ui.add_enabled_ui(false, |ui| combo(ui, "pr_impose", &mut i, &["As in document"], 200.0));
    });
}

fn prepress(ui: &mut Ui, st: &mut PrintState) {
    let s = &mut st.settings;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(250.0);
            group(ui, "Paper/film settings", |ui| {
                ui.checkbox(&mut s.invert, "Invert");
                ui.checkbox(&mut s.mirror, "Mirror");
            });
            group(ui, "Crop/fold marks", |ui| {
                ui.checkbox(&mut s.crop_marks, "Crop/fold marks");
                ui.add_enabled(s.crop_marks, egui::Checkbox::new(&mut s.exterior_only, "Exterior only"));
            });
            group(ui, "Calibration bars", |ui| {
                ui.checkbox(&mut s.calibration_bar, "Color calibration bar");
                ui.checkbox(&mut s.densitometer, "Densitometer scales");
            });
        });
        ui.vertical(|ui| {
            group(ui, "File information", |ui| {
                ui.checkbox(&mut s.file_info, "Print file information");
                ui.checkbox(&mut s.page_numbers, "Print page numbers");
            });
            group(ui, "Registration marks", |ui| {
                ui.checkbox(&mut s.registration_marks, "Print registration marks");
            });
            group(ui, "Marks placement", |ui| {
                ui.checkbox(&mut s.marks_to_objects, "Marks to objects");
            });
        });
    });
}

pub fn preflight_list(ui: &mut Ui, found: &[Issue]) {
    if found.is_empty() {
        ui.add_space(10.0);
        ui.label("No issues were found.");
        return;
    }
    egui::ScrollArea::vertical().max_height(440.0).show(ui, |ui| {
        for i in found {
            ui.label(egui::RichText::new(format!("⚠ {}", i.title)).color(Color32::from_rgb(0xf0, 0xc0, 0x40)));
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                ui.add(egui::Label::new(egui::RichText::new(&i.detail).color(theme::TEXT_DIM())).wrap());
            });
            ui.add_space(4.0);
        }
    });
}

fn mini_texture(app: &AnyVectorApp, ctx: &egui::Context, st: &mut PrintState) -> Option<egui::TextureHandle> {
    let j = job(app, st)?;
    let sh = sheets(st, &j);
    let (sheet, paper) = sh.first()?.clone();
    let img = printing::render_sheet(&j.docs[sheet.doc], &sheet, &paper, &st.settings, sheet.page + 1, 30.0)?;
    let ci = egui::ColorImage::from_rgba_premultiplied([img.width() as usize, img.height() as usize], img.data());
    Some(ctx.load_texture("print_mini", ci, egui::TextureOptions::LINEAR))
}

// ------------------------------------------------------------------------------------------
// Print Preview
// ------------------------------------------------------------------------------------------

fn preview(app: &mut AnyVectorApp, ctx: &egui::Context, mut st: PrintState) {
    let Some(j) = job(app, &mut st) else { return };
    let sh = sheets(&mut st, &j);
    let n = sh.len();
    let mut pv = st.preview.take().unwrap_or_default();
    pv.sheet = pv.sheet.min(n.saturating_sub(1));
    let screen = ctx.content_rect();
    let size = vec2(screen.width() - 40.0, screen.height() - 40.0);
    let mut close = false;
    let mut print = false;
    let mut options = false;
    let found = issues(app, &mut st);
    let ((), dismissed) = window(ctx, "print_preview", "Print Preview", size, |ui| {
        // toolbar
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Print style:");
            egui::ComboBox::from_id_salt("pv_style").selected_text(&st.style).width(160.0).show_ui(ui, |ui| {
                if ui.selectable_label(st.style == printing::DEFAULT_STYLE, printing::DEFAULT_STYLE).clicked() {
                    st.style = printing::DEFAULT_STYLE.into();
                    st.settings = PrintSettings::default();
                }
                for nm in printing::style_names() {
                    if ui.selectable_label(st.style == nm, &nm).clicked() {
                        if let Some(s) = printing::load_style(&printing::style_path(&nm)) {
                            st.style = nm.clone();
                            st.settings = s;
                        }
                    }
                }
            });
            if ui.button("Save Print Style As...").clicked() {
                let _ = std::fs::create_dir_all(printing::styles_dir());
                if let Some(p) = rfd::FileDialog::new().add_filter("Print style (*.prs)", &["prs"]).set_directory(printing::styles_dir()).save_file() {
                    if printing::save_style(&p, &st.settings).is_ok() {
                        st.style = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    }
                }
            }
            let deletable = st.style != printing::DEFAULT_STYLE && printing::style_path(&st.style).exists();
            if ui.add_enabled(deletable, egui::Button::new("Delete Print Style")).clicked() {
                let _ = std::fs::remove_file(printing::style_path(&st.style));
                st.style = printing::DEFAULT_STYLE.into();
            }
            crate::widgets::vsep(ui);
            if ui.button("Print Options...").clicked() {
                options = true;
            }
            if ui.add(egui::Button::new(egui::RichText::new("Print").color(Color32::WHITE)).fill(theme::PRIMARY())).clicked() {
                print = true;
            }
            crate::widgets::vsep(ui);
            let zooms = [(0, "To Fit"), (25, "25%"), (50, "50%"), (75, "75%"), (100, "100%"), (200, "200%")];
            let cur = zooms.iter().find(|z| z.0 == pv.zoom).map(|z| z.1).unwrap_or("To Fit");
            egui::ComboBox::from_id_salt("pv_zoom").selected_text(cur).width(80.0).show_ui(ui, |ui| {
                for (z, name) in zooms {
                    ui.selectable_value(&mut pv.zoom, z, name);
                }
            });
            crate::widgets::vsep(ui);
            ui.toggle_value(&mut st.settings.invert, "Invert");
            ui.toggle_value(&mut st.settings.mirror, "Mirror");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close Print Preview").clicked() {
                    close = true;
                }
                let t = if found.is_empty() { "No Issues".to_string() } else { format!("⚠ {} Issues", found.len()) };
                ui.label(egui::RichText::new(t).color(theme::TEXT_DIM()));
            });
        });
        ui.add_space(4.0);
        // preview area
        let area = Rect::from_min_max(ui.cursor().min, pos2(ui.max_rect().right(), ui.max_rect().bottom() - 30.0));
        ui.painter().rect_filled(area, 0.0, theme::DESKTOP());
        if let Some((sheet, paper)) = sh.get(pv.sheet).cloned() {
            let fit = ((area.width() - 40.0) / paper.width as f32).min((area.height() - 40.0) / paper.height as f32);
            let k = if pv.zoom == 0 { fit } else { pv.zoom as f32 / 100.0 * ctx.pixels_per_point().recip() * 96.0 / 72.0 };
            let dpi = (k * 72.0 * ctx.pixels_per_point()).clamp(10.0, 300.0) as f64;
            let mut h = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            format!("{:?}{:?}{}{}", st.settings, sheet.place, pv.sheet, dpi as u32).hash(&mut h);
            app.doc().map(|d| d.history.revision).hash(&mut h);
            let key = h.finish();
            if pv.tex.as_ref().map(|t| t.0) != Some(key) {
                if let Some(img) = printing::render_sheet(&j.docs[sheet.doc], &sheet, &paper, &st.settings, sheet.page + 1, dpi) {
                    let ci = egui::ColorImage::from_rgba_premultiplied([img.width() as usize, img.height() as usize], img.data());
                    pv.tex = Some((key, ctx.load_texture("print_preview_sheet", ci, egui::TextureOptions::LINEAR)));
                }
            }
            let pr = Rect::from_center_size(area.center(), vec2(paper.width as f32 * k, paper.height as f32 * k));
            let painter = ui.painter_at(area);
            painter.rect_filled(pr.translate(vec2(4.0, 4.0)), 0.0, Color32::from_black_alpha(120));
            if let Some((_, tex)) = &pv.tex {
                painter.image(tex.id(), pr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            // the printable area, dashed
            let pa = Rect::from_min_max(pr.min + vec2(paper.printable.x0 as f32 * k, paper.printable.y0 as f32 * k), pr.min + vec2(paper.printable.x1 as f32 * k, paper.printable.y1 as f32 * k));
            let dash = egui::Shape::dashed_line(&[pa.left_top(), pa.right_top(), pa.right_bottom(), pa.left_bottom(), pa.left_top()], Stroke::new(1.0, Color32::from_gray(0x90)), 4.0, 3.0);
            painter.extend(dash);
        } else {
            ui.painter().text(area.center(), egui::Align2::CENTER_CENTER, "There is nothing to print.", egui::FontId::proportional(14.0), theme::TEXT());
        }
        // page navigation
        let nav = Rect::from_min_max(pos2(area.left(), area.bottom() + 4.0), pos2(area.right(), area.bottom() + 28.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(nav), |ui| {
            ui.horizontal(|ui| {
                if ui.add_enabled(pv.sheet > 0, egui::Button::new("◀")).clicked() {
                    pv.sheet -= 1;
                }
                ui.label(format!("Page {} of {}", (pv.sheet + 1).min(n.max(1)), n.max(1)));
                if ui.add_enabled(pv.sheet + 1 < n, egui::Button::new("▶")).clicked() {
                    pv.sheet += 1;
                }
                if let Some((s, p)) = sh.get(pv.sheet) {
                    ui.label(egui::RichText::new(format!("   {}   {:.2} × {:.2} in.{}", if s.landscape { "Landscape" } else { "Portrait" }, p.width / 72.0, p.height / 72.0, s.tile.map(|(c, r)| format!("   tile {},{}", c + 1, r + 1)).unwrap_or_default())).color(theme::TEXT_DIM()));
                }
            });
        });
        let _ = ui.allocate_rect(area, Sense::hover());
    });
    if ctx.input(|i| i.key_pressed(egui::Key::PageDown)) && pv.sheet + 1 < n {
        pv.sheet += 1;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::PageUp)) && pv.sheet > 0 {
        pv.sheet -= 1;
    }
    app.settings.print = st.settings.clone();
    if print {
        if let Some(j) = job(app, &mut st) {
            send(app, j);
        }
        return;
    }
    if options {
        st.preview = None;
        app.dialogs.print = Some(st);
        return;
    }
    if close || dismissed {
        if pv.from_dialog {
            st.preview = None;
            app.dialogs.print = Some(st);
        }
        return;
    }
    st.preview = Some(pv);
    app.dialogs.print = Some(st);
}
