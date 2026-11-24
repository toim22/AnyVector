//! File ▸ Save As on Linux: the system save dialog (rfd) picks the folder, name and type, then
//! this dialog shows what the Windows Save Drawing dialog adds to its own (see `save_dialog.rs`):
//! Title, Subject and Rating; for AnyVector's formats Embed color profiles, Embed fonts, the
//! version and Advanced...; for the other types the note that a copy is written. Save writes the
//! drawing with these choices, Cancel drops the save, as cancelling the Windows dialog does.
//! "Save as type" can still be changed here: rfd does not say which filter was chosen, so the
//! system dialog only gives the type through the name's extension.
//!
//! The dialog is drawn over frames, so the save that opened it has already returned: the state
//! carries what to do afterwards ([`AfterSave`]: close the document, go on quitting).

use super::{button, combo, note, ok_button, ok_cancel_plain, row, separator, window};
use crate::app::AnyVectorApp;
use crate::exec::{save_filter_order, saved_type, with_type, AfterSave};
use crate::save_dialog::{has_drawing_options, size_text, Advanced, SaveRequest, SaveResult, CDR_NOTE, COPY_NOTE};
use egui::{vec2, Ui};
use std::path::PathBuf;

pub struct SaveOptionsState {
    /// The `uid` of the document being saved (its place among the tabs may change while the
    /// dialog is open, e.g. by a file dropped on the window).
    pub doc: u64,
    /// Where to save, with the extension of its type.
    pub path: PathBuf,
    /// The name the system dialog returned: it has already asked before replacing that file.
    picked: PathBuf,
    /// Into `req.types`: the type of `path`.
    pub type_index: usize,
    /// The values shown, changed in place (title, subject, rating, embedding, version, advanced).
    pub req: SaveRequest,
    /// Advanced Save Options while it is open: the values being changed.
    pub advanced_open: Option<Advanced>,
    /// "... already exists. Do you want to replace it?" is open.
    pub confirm_replace: bool,
    pub then: AfterSave,
}

impl SaveOptionsState {
    pub fn new(doc: u64, req: SaveRequest, path: PathBuf, then: AfterSave) -> Self {
        let type_index = req.type_index;
        let mut st = Self { doc, path: PathBuf::new(), picked: PathBuf::new(), type_index, req, advanced_open: None, confirm_replace: false, then };
        st.set_picked(path);
        st
    }

    /// Take over the name the system dialog returned: its extension says the type, and a name
    /// without one gets the extension of the type.
    fn set_picked(&mut self, path: PathBuf) {
        self.type_index = saved_type(&self.req.types, &path, self.type_index);
        self.path = with_type(path.clone(), self.ext());
        self.picked = path;
    }

    /// Another "Save as type": the name changes its extension, as in the Windows dialog.
    pub fn set_type(&mut self, type_index: usize) {
        self.type_index = type_index;
        let known = self.req.types.iter().any(|t| t.1 == vector_io::extension(&self.path));
        let base = if known { self.path.with_extension("") } else { self.path.clone() };
        let mut s = base.into_os_string();
        s.push(format!(".{}", self.ext()));
        self.path = PathBuf::from(s);
    }

    /// The extension of the chosen type.
    pub fn ext(&self) -> &str {
        self.req.types.get(self.type_index).map(|t| t.1.as_str()).unwrap_or("cdr")
    }

    /// Whether Save replaces a file the system dialog did not ask about (the extension was added
    /// or the type changed after it).
    fn replaces_unasked(&self) -> bool {
        self.path != self.picked && self.path.exists()
    }

    /// What the Windows dialog returns, from the values shown.
    pub fn result(&self) -> SaveResult {
        let r = &self.req;
        SaveResult {
            path: self.path.clone(),
            type_index: self.type_index,
            title: r.title.clone(),
            subject: r.subject.clone(),
            rating: r.rating,
            embed_profiles: r.embed_profiles,
            embed_fonts: r.embed_fonts,
            version: r.version,
            advanced: r.advanced,
        }
    }
}

/// Whether Save As uses this dialog: off Windows, and in tests that give the file names.
pub fn own_step() -> bool {
    cfg!(not(windows)) || test_seam::active()
}

/// The system save dialog, starting on the drawing's type (rfd starts on the first filter).
pub fn pick_path(req: &SaveRequest) -> Option<PathBuf> {
    if test_seam::active() {
        return test_seam::next();
    }
    let types = &req.types;
    let order = save_filter_order(types.len(), req.type_index);
    let mut fd = order.into_iter().fold(rfd::FileDialog::new().set_title(crate::i18n::t("Save Drawing")), |fd, k| fd.add_filter(crate::i18n::t(&types[k].0), &[types[k].1.as_str()]));
    if let Some(dir) = &req.dir {
        fd = fd.set_directory(dir);
    }
    fd.set_file_name(req.file_name.as_str()).save_file()
}

/// Save: first ask before replacing a file the system dialog did not ask about.
pub fn save_clicked(app: &mut AnyVectorApp) {
    match &mut app.dialogs.save_options {
        Some(st) if st.replaces_unasked() => st.confirm_replace = true,
        _ => confirm(app),
    }
}

/// Save with the values shown, then go on with what started the save.
pub fn confirm(app: &mut AnyVectorApp) {
    let Some(st) = app.dialogs.save_options.take() else { return };
    let Some(i) = app.docs.iter().position(|d| d.uid == st.doc) else { return };
    let saved = app.save_with(i, st.result());
    app.after_save(i, st.then, saved);
}

/// Cancel: nothing is saved, and Close All or Exit stop, as when the Windows dialog is cancelled.
pub fn cancel(app: &mut AnyVectorApp) {
    let Some(st) = app.dialogs.save_options.take() else { return };
    let Some(i) = app.docs.iter().position(|d| d.uid == st.doc) else { return };
    app.after_save(i, st.then, false);
}

/// Browse...: the system dialog again, on the name and type shown.
fn browse(st: &mut SaveOptionsState) {
    st.req.dir = st.path.parent().map(|p| p.to_path_buf());
    st.req.file_name = st.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    st.req.type_index = st.type_index;
    if let Some(p) = pick_path(&st.req) {
        st.set_picked(p);
    }
}

const LABEL_W: f32 = 100.0;

/// What was clicked in the dialog.
#[derive(Clone, Copy, PartialEq)]
enum Action {
    Browse,
    Type(usize),
    Advanced,
    Save,
    Cancel,
}

/// A name or folder, read only. A text box, because labels are translated and a file's name must
/// not be. Returns whether Enter (or a click elsewhere) took the focus from it.
fn path_field(ui: &mut Ui, text: &str, width: f32) -> bool {
    let mut t = text;
    ui.add(egui::TextEdit::singleline(&mut t).desired_width(width)).lost_focus()
}

/// Enter without a modifier.
fn enter_pressed(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.is_none())
}

pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    // a message over this dialog has the keys (the note about the drawing saved before this one,
    // when quitting)
    let message = app.dialogs.message_box.is_some();
    let Some(st) = app.dialogs.save_options.as_mut() else { return };
    if !app.docs.iter().any(|d| d.uid == st.doc) {
        app.dialogs.save_options = None;
        return;
    }
    let covered = message || st.advanced_open.is_some() || st.confirm_replace;
    // Enter is Save unless it belongs to a focused button or to an open list
    let popup = egui::Popup::is_any_open(ctx);
    let enter = enter_pressed(ctx);
    let unfocused = ctx.memory(|m| m.focused().is_none());
    let mut left_text = false;
    let native = has_drawing_options(st.ext());
    let mut action = None;
    let size = if native { vec2(540.0, 478.0) } else { vec2(540.0, 330.0) };
    let ((), dismissed) = window(ctx, "save_options", "Save Drawing", size, |ui| {
        ui.add_space(8.0);
        let field_w = ui.available_width() - LABEL_W - 16.0;
        let name = st.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let folder = st.path.parent().map(|p| p.display().to_string()).unwrap_or_default();
        row(ui, "File name:", LABEL_W, |ui| {
            left_text |= path_field(ui, &name, field_w - 92.0);
            if button(ui, "Browse...", true) {
                action = Some(Action::Browse);
            }
        });
        row(ui, "Folder:", LABEL_W, |ui| left_text |= path_field(ui, &folder, field_w));
        row(ui, "Save as type:", LABEL_W, |ui| {
            let names: Vec<&str> = st.req.types.iter().map(|t| t.0.as_str()).collect();
            let mut k = st.type_index;
            if combo(ui, "save_type", &mut k, &names, field_w) {
                action = Some(Action::Type(k));
            }
        });
        separator(ui);
        // the shell's property fields of the Windows dialog
        row(ui, "Title:", LABEL_W, |ui| left_text |= ui.add(egui::TextEdit::singleline(&mut st.req.title).desired_width(field_w)).lost_focus());
        row(ui, "Subject:", LABEL_W, |ui| left_text |= ui.add(egui::TextEdit::singleline(&mut st.req.subject).desired_width(field_w)).lost_focus());
        row(ui, "Rating:", LABEL_W, |ui| {
            let mut r = st.req.rating.min(5) as usize;
            if combo(ui, "save_rating", &mut r, &super::docprops::RATINGS, 140.0) {
                st.req.rating = r as u8;
            }
        });
        separator(ui);
        if native {
            ui.checkbox(&mut st.req.embed_profiles, format!("Embed color profiles ({})", size_text(st.req.profiles_size)));
            ui.horizontal_top(|ui| {
                ui.add_space(24.0);
                ui.label("Color profiles:");
                ui.add(egui::Label::new(egui::RichText::new(&st.req.profiles_summary).color(crate::theme::TEXT_DIM())).wrap());
            });
            ui.add_space(4.0);
            ui.checkbox(&mut st.req.embed_fonts, format!("Embed fonts ({})", size_text(st.req.fonts_size)));
            ui.add_space(4.0);
            row(ui, "Version:", LABEL_W, |ui| {
                let names: Vec<&str> = st.req.versions.iter().map(String::as_str).collect();
                let mut v = st.req.version.min(names.len().saturating_sub(1));
                if combo(ui, "save_version", &mut v, &names, 180.0) {
                    st.req.version = v;
                }
            });
            note(ui, CDR_NOTE);
        } else {
            note(ui, COPY_NOTE);
        }
        // Advanced... sits next to Save, as on Windows; it belongs to AnyVector's formats only
        let rect = egui::Rect::from_min_max(egui::pos2(ui.max_rect().left(), ui.max_rect().bottom() - 24.0), ui.max_rect().right_bottom());
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button(ui, "Cancel", true) {
                    action = Some(Action::Cancel);
                }
                ui.add_space(4.0);
                if ok_button(ui, "Save", true) {
                    action = Some(Action::Save);
                }
                if native {
                    ui.add_space(4.0);
                    if button(ui, "Advanced...", true) {
                        action = Some(Action::Advanced);
                    }
                }
            });
        });
    });
    if covered {
        on_top(app, ctx);
        return;
    }
    if action.is_none() && !popup {
        if enter && (unfocused || left_text) {
            action = Some(Action::Save);
        } else if dismissed {
            action = Some(Action::Cancel);
        }
    }
    match action {
        Some(Action::Browse) => browse(st),
        Some(Action::Type(k)) => st.set_type(k),
        Some(Action::Advanced) => st.advanced_open = Some(st.req.advanced),
        Some(Action::Save) => save_clicked(app),
        Some(Action::Cancel) => cancel(app),
        None => {}
    }
}

/// The dialogs on top of this one: Advanced Save Options and the question before replacing a file.
fn on_top(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(st) = app.dialogs.save_options.as_mut() else { return };
    if let Some(a) = st.advanced_open.as_mut() {
        match advanced_dialog(ctx, a) {
            Some(true) => {
                st.req.advanced = *a;
                st.advanced_open = None;
            }
            Some(false) => st.advanced_open = None,
            None => {}
        }
    } else if st.confirm_replace {
        let name = st.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let enter = enter_pressed(ctx) && ctx.memory(|m| m.focused().is_none());
        let mut choice = None;
        let ((), dismissed) = window(ctx, "save_replace", "Save Drawing", vec2(420.0, 150.0), |ui| {
            ui.add_space(14.0);
            ui.label(format!("{name} already exists.\nDo you want to replace it?"));
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
                ui.horizontal(|ui| {
                    // No is the default, as in the Windows dialog
                    if ok_button(ui, "No", true) {
                        choice = Some(false);
                    }
                    if button(ui, "Yes", true) {
                        choice = Some(true);
                    }
                });
            });
        });
        if choice.is_none() && (dismissed || enter) {
            choice = Some(false);
        }
        match choice {
            Some(true) => confirm(app),
            Some(false) => st.confirm_replace = false,
            None => {}
        }
    }
}

/// Advanced Save Options, as the Windows dialog behind Advanced... Some(true) on OK, Some(false) on Cancel, None while open.
fn advanced_dialog(ctx: &egui::Context, a: &mut Advanced) -> Option<bool> {
    let enter = enter_pressed(ctx) && ctx.memory(|m| m.focused().is_none());
    let ((ok, cancel), dismissed) = window(ctx, "save_advanced", "Advanced Save Options", vec2(380.0, 236.0), |ui| {
        ui.add_space(4.0);
        super::group(ui, "Optimize for", |ui| {
            ui.horizontal(|ui| {
                if ui.radio(!a.optimize_for_size, "Speed").clicked() {
                    a.optimize_for_size = false;
                }
                ui.add_space(60.0);
                if ui.radio(a.optimize_for_size, "Size").clicked() {
                    a.optimize_for_size = true;
                }
            });
        });
        ui.add_space(8.0);
        ui.checkbox(&mut a.presentation_attributes, "Save presentation attributes (views)");
        ui.checkbox(&mut a.bitmap_compression, "Use bitmap compression");
        ui.checkbox(&mut a.object_compression, "Use graphic object compression");
        ui.checkbox(&mut a.thumbnail, "Save thumbnail");
        ok_cancel_plain(ui, true)
    });
    if cancel || dismissed {
        Some(false)
    } else {
        (ok || enter).then_some(true)
    }
}

/// Tests give the file names the system dialog would return (None = cancelled), so the flow runs
/// without a desktop, on Windows too.
mod test_seam {
    #[cfg(test)]
    thread_local! {
        static PICKS: std::cell::RefCell<Option<std::collections::VecDeque<Option<std::path::PathBuf>>>> = const { std::cell::RefCell::new(None) };
    }

    #[cfg(test)]
    pub fn active() -> bool {
        PICKS.with(|p| p.borrow().is_some())
    }

    #[cfg(not(test))]
    pub fn active() -> bool {
        false
    }

    /// The next name given; None (cancelled) when the test gave no more.
    #[cfg(test)]
    pub fn next() -> Option<std::path::PathBuf> {
        PICKS.with(|p| p.borrow_mut().as_mut().and_then(|q| q.pop_front()).flatten())
    }

    #[cfg(not(test))]
    pub fn next() -> Option<std::path::PathBuf> {
        None
    }

    /// What the system save dialog returns, in order, on this test thread.
    #[cfg(test)]
    pub fn give(picks: Vec<Option<std::path::PathBuf>>) {
        PICKS.with(|p| *p.borrow_mut() = Some(picks.into()));
    }
}

#[cfg(test)]
pub use test_seam::give as give_picked_paths;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::Cmd;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    fn harness<'a>() -> Harness<'a, AnyVectorApp> {
        let mut h = Harness::builder().with_size(vec2(1600.0, 900.0)).build_eframe(|cc| {
            let mut app = AnyVectorApp::new(cc);
            app.splash = None;
            app.pending.clear();
            app
        });
        settle(&mut h);
        h
    }

    fn settle(h: &mut Harness<'_, AnyVectorApp>) {
        for _ in 0..4 {
            h.step();
        }
    }

    fn click(h: &mut Harness<'_, AnyVectorApp>, label: &str) {
        h.get_by_label(label).click_accesskit();
        settle(h);
    }

    /// A button of the dialog on top, when the one under it has the same.
    fn click_on_top(h: &mut Harness<'_, AnyVectorApp>, label: &str) {
        h.get_all_by_label(label).last().unwrap().click_accesskit();
        settle(h);
    }

    fn has(h: &Harness<'_, AnyVectorApp>, label: &str) -> bool {
        h.query_by_label(label).is_some()
    }

    /// A new drawing with a rectangle on it (so it is modified); its index.
    fn drawing(h: &mut Harness<'_, AnyVectorApp>) -> usize {
        h.state_mut().new_default_document();
        h.state_mut().dialogs.new_doc = None;
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        let shape = vector_core::object::RectShape { w: 100.0, h: 60.0, radii: [0.0; 4], corner: vector_core::object::CornerType::Round };
        d.add_objects("test", vec![vector_core::Object::new(id, vector_core::ObjKind::Rect(shape), kurbo::Affine::translate((50.0, 50.0)), vector_core::Style::default())]);
        settle(h);
        let i = h.state().active.unwrap();
        assert!(h.state().docs[i].history.is_modified());
        i
    }

    fn temp_dir(test: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("anyvector-test-{}-save-options-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn state<'a>(h: &'a Harness<'_, AnyVectorApp>) -> &'a SaveOptionsState {
        h.state().dialogs.save_options.as_ref().expect("the Save Options dialog is open")
    }

    /// Windows keeps its system dialog; the own step is for Linux (and tests that give names).
    #[test]
    fn the_own_step_is_for_linux_only() {
        assert_eq!(own_step(), cfg!(not(windows)));
        give_picked_paths(vec![]);
        assert!(own_step());
    }

    /// Cancel in the system dialog: nothing is saved and no options are asked.
    #[test]
    fn cancel_at_the_file_step_saves_nothing() {
        let mut h = harness();
        let i = drawing(&mut h);
        give_picked_paths(vec![None]);
        assert!(!h.state_mut().save_doc(i, true));
        assert!(h.state().dialogs.save_options.is_none());
        assert!(h.state().docs[i].path.is_none() && h.state().docs[i].history.is_modified());
    }

    /// Cancel in Save Options: nothing is written, the drawing and the settings stay as they were.
    #[test]
    fn cancel_at_the_options_step_saves_nothing() {
        let mut h = harness();
        let i = drawing(&mut h);
        let p = temp_dir("cancel").join("Logo.cdr");
        let fonts = h.state().settings.embed_fonts;
        give_picked_paths(vec![Some(p.clone())]);
        assert!(!h.state_mut().save_doc(i, true), "not saved yet: the options come first");
        settle(&mut h);
        assert_eq!(state(&h).ext(), "cdr");
        h.state_mut().dialogs.save_options.as_mut().unwrap().req.embed_fonts = !fonts;
        h.state_mut().dialogs.save_options.as_mut().unwrap().req.title = "Not saved".into();
        click(&mut h, "Cancel");
        assert!(h.state().dialogs.save_options.is_none());
        assert!(!p.exists());
        let d = &h.state().docs[i];
        assert!(d.path.is_none() && d.history.is_modified() && d.doc.info.title.is_empty());
        assert_eq!(h.state().settings.embed_fonts, fonts, "the choices are kept only when saving");
    }

    /// Save writes the drawing with the chosen title, subject, rating, embedding and Advanced
    /// options, and remembers them, as the Windows dialog does.
    #[test]
    fn confirm_saves_with_the_chosen_options() {
        let mut h = harness();
        let i = drawing(&mut h);
        h.state_mut().settings.save.save_thumbnail = true;
        h.state_mut().settings.save.optimize_for_size = false;
        h.state_mut().settings.embed_fonts = true;
        let p = temp_dir("confirm").join("Logo.cdr");
        give_picked_paths(vec![Some(p.clone())]);
        h.state_mut().run(Cmd::SaveAs);
        settle(&mut h);
        // the drawing options of AnyVector's formats
        for label in ["Advanced...", "Browse...", "Title:", "Subject:", "Rating:", "Version:"] {
            assert!(has(&h, label), "{label}");
        }
        let embed_fonts = format!("Embed fonts ({})", size_text(state(&h).req.fonts_size));
        assert!(has(&h, &embed_fonts));
        assert!(state(&h).req.embed_fonts && state(&h).req.advanced.thumbnail, "it starts with the settings");
        {
            let st = h.state_mut().dialogs.save_options.as_mut().unwrap();
            st.req.title = "Poster".into();
            st.req.subject = "Spring sale".into();
            st.req.rating = 4;
        }
        click(&mut h, &embed_fonts);
        assert!(!state(&h).req.embed_fonts);
        // Advanced...: Cancel drops the changes, OK keeps them
        click(&mut h, "Advanced...");
        assert!(state(&h).advanced_open.is_some());
        click(&mut h, "Save thumbnail");
        assert!(!state(&h).advanced_open.unwrap().thumbnail);
        click_on_top(&mut h, "Cancel");
        assert!(state(&h).advanced_open.is_none() && state(&h).req.advanced.thumbnail, "cancelled");
        click(&mut h, "Advanced...");
        click(&mut h, "Save thumbnail");
        click(&mut h, "Size");
        click(&mut h, "OK");
        assert!(state(&h).advanced_open.is_none());
        assert!(!state(&h).req.advanced.thumbnail && state(&h).req.advanced.optimize_for_size);
        confirm(h.state_mut());
        settle(&mut h);
        assert!(h.state().dialogs.save_options.is_none());
        let d = &h.state().docs[i];
        assert_eq!(d.path.as_deref(), Some(p.as_path()));
        assert!(!d.history.is_modified());
        assert_eq!((d.doc.info.title.as_str(), d.doc.info.subject.as_str(), d.doc.info.rating), ("Poster", "Spring sale", 4));
        let s = &h.state().settings;
        assert!(!s.embed_fonts && !s.save.save_thumbnail && s.save.optimize_for_size, "remembered for next time");
        // in the file
        let back = vector_io::open(&p).unwrap();
        assert_eq!((back.info.title.as_str(), back.info.rating), ("Poster", 4));
        assert!(vector_io::cdr::read_thumbnail(&p).is_none(), "no thumbnail, as chosen");
    }

    /// A .vct drawing starts on VCT, a name typed without extension gets it, and VCT has the
    /// drawing options too.
    #[test]
    fn a_vct_drawing_saves_as_vct() {
        let mut h = harness();
        let i = drawing(&mut h);
        let dir = temp_dir("vct");
        h.state_mut().docs[i].path = Some(dir.join("Old.vct"));
        give_picked_paths(vec![Some(dir.join("Plan"))]);
        assert!(!h.state_mut().save_doc(i, true));
        settle(&mut h);
        assert_eq!(state(&h).ext(), "vct");
        assert!(has(&h, "Advanced...") && has(&h, "Version:"));
        click(&mut h, "Save");
        assert!(dir.join("Plan.vct").exists());
        assert_eq!(h.state().docs[i].path.as_deref(), Some(dir.join("Plan.vct").as_path()));
    }

    /// A copy type (PDF) shows the copy note instead of the drawing options and writes a copy;
    /// Browse... picks another name and type.
    #[test]
    fn a_copy_type_shows_the_copy_note() {
        let mut h = harness();
        let i = drawing(&mut h);
        let dir = temp_dir("copy");
        give_picked_paths(vec![Some(dir.join("Logo.cdr")), Some(dir.join("Logo.pdf"))]);
        assert!(!h.state_mut().save_doc(i, true));
        settle(&mut h);
        assert!(has(&h, "Advanced..."));
        click(&mut h, "Browse...");
        assert_eq!(state(&h).ext(), "pdf");
        assert_eq!(state(&h).path, dir.join("Logo.pdf"));
        assert!(has(&h, COPY_NOTE) && !has(&h, "Advanced...") && !has(&h, "Version:"));
        assert!(has(&h, "Title:"), "the document information is asked for every type");
        click(&mut h, "Save");
        assert!(dir.join("Logo.pdf").exists() && !dir.join("Logo.cdr").exists());
        let d = &h.state().docs[i];
        assert!(d.path.is_none() && d.history.is_modified(), "a copy: the drawing is not saved");
    }

    /// Closing a modified drawing: Yes ▸ system dialog ▸ Save Options ▸ Save, then it closes.
    /// Cancel in the options keeps it open, and stops Close All.
    #[test]
    fn save_before_close_goes_on_after_the_options() {
        let mut h = harness();
        let first = drawing(&mut h);
        let second = drawing(&mut h);
        let dir = temp_dir("close");
        let n = h.state().docs.len();
        // Cancel stops Close All: both stay
        give_picked_paths(vec![Some(dir.join("A.cdr"))]);
        h.state_mut().run(Cmd::CloseAll);
        settle(&mut h);
        assert!(h.state().dialogs.confirm_close.is_some());
        click(&mut h, "Yes");
        assert_eq!(h.state().docs.len(), n, "still open while the options are asked");
        assert!(h.state().dialogs.save_options.is_some());
        click(&mut h, "Cancel");
        assert_eq!(h.state().docs.len(), n);
        assert!(h.state().dialogs.close_queue.is_empty() && h.state().dialogs.confirm_close.is_none());
        assert!(!dir.join("A.cdr").exists());
        // Save closes the drawing and Close All asks about the next one
        give_picked_paths(vec![Some(dir.join("B.cdr"))]);
        h.state_mut().run(Cmd::CloseAll);
        settle(&mut h);
        let asked = h.state().dialogs.confirm_close.unwrap();
        assert!(asked == first || asked == second);
        click(&mut h, "Yes");
        click(&mut h, "Save");
        assert!(dir.join("B.cdr").exists());
        assert_eq!(h.state().docs.len(), n - 1, "the saved drawing closed");
        assert!(h.state().dialogs.confirm_close.is_some(), "Close All goes on with the other drawing");
        click(&mut h, "No");
        assert_eq!(h.state().docs.len(), n - 2);
    }

    /// Exit with two unsaved drawings: Yes asks the options of each, then AnyVector quits.
    #[test]
    fn exit_saves_each_drawing_then_quits() {
        let mut h = harness();
        drawing(&mut h);
        drawing(&mut h);
        let dir = temp_dir("exit");
        give_picked_paths(vec![Some(dir.join("One.cdr")), Some(dir.join("Two.vct"))]);
        h.state_mut().dialogs.confirm_quit = true;
        settle(&mut h);
        click(&mut h, "Yes");
        let first = state(&h).doc;
        confirm(h.state_mut());
        assert!(!h.state().quit_requested, "the second drawing is still to save");
        let d = h.state().docs.iter().find(|d| d.uid == first).unwrap();
        assert_eq!(d.path.as_deref(), Some(dir.join("One.cdr").as_path()));
        let second = state(&h).doc;
        assert_ne!(first, second);
        assert_eq!(state(&h).ext(), "vct");
        confirm(h.state_mut());
        assert!(h.state().quit_requested, "all saved: quit");
        assert!(dir.join("Two.vct").exists());
        // Cancel in the options does not quit
        let mut h = harness();
        drawing(&mut h);
        give_picked_paths(vec![Some(dir.join("Three.cdr"))]);
        h.state_mut().dialogs.confirm_quit = true;
        settle(&mut h);
        click(&mut h, "Yes");
        cancel(h.state_mut());
        assert!(!h.state().quit_requested && !h.state().allow_close);
    }

    /// The system dialog asks before replacing the name it was given; when the extension is added
    /// or the type changed afterwards, AnyVector asks. No (and Escape) goes back to the options.
    #[test]
    fn a_file_the_system_dialog_did_not_ask_about_is_not_replaced_unasked() {
        let mut h = harness();
        let i = drawing(&mut h);
        let dir = temp_dir("replace");
        let p = dir.join("Logo.cdr");
        std::fs::write(&p, b"another drawing").unwrap();
        give_picked_paths(vec![Some(dir.join("Logo"))]);
        assert!(!h.state_mut().save_doc(i, true));
        settle(&mut h);
        assert_eq!(state(&h).path, p, "the name is shown with its extension");
        for no in ["No", "Escape"] {
            save_clicked(h.state_mut());
            settle(&mut h);
            assert!(has(&h, "Logo.cdr already exists.\nDo you want to replace it?"));
            if no == "No" {
                click(&mut h, "No");
            } else {
                h.key_press(egui::Key::Escape);
                settle(&mut h);
            }
            assert!(!state(&h).confirm_replace, "{no}: back to the options");
            assert_eq!(std::fs::read(&p).unwrap(), b"another drawing");
        }
        save_clicked(h.state_mut());
        settle(&mut h);
        click(&mut h, "Yes");
        assert!(h.state().dialogs.save_options.is_none());
        assert_eq!(vector_io::cdr::detect(&std::fs::read(&p).unwrap()), vector_io::cdr::CdrKind::AnyVector);
        // the name the system dialog asked about itself is not asked again
        give_picked_paths(vec![Some(p.clone())]);
        assert!(!h.state_mut().save_doc(i, true));
        save_clicked(h.state_mut());
        assert!(h.state().dialogs.save_options.is_none());
    }

    /// "Save as type" changes the extension of the name and which controls show, as on Windows.
    #[test]
    fn changing_the_type_changes_the_extension() {
        let mut h = harness();
        let i = drawing(&mut h);
        let dir = temp_dir("type");
        give_picked_paths(vec![Some(dir.join("Logo v1.2.cdr"))]);
        assert!(!h.state_mut().save_doc(i, true));
        settle(&mut h);
        let pos = |ext: &str| vector_io::SAVE_TYPES.iter().position(|t| t.extensions[0] == ext).unwrap();
        for (ext, native) in [("svg", false), ("vct", true), ("cmx", false), ("cdr", true)] {
            h.state_mut().dialogs.save_options.as_mut().unwrap().set_type(pos(ext));
            settle(&mut h);
            assert_eq!(state(&h).path, dir.join(format!("Logo v1.2.{ext}")));
            assert_eq!((has(&h, "Advanced..."), has(&h, COPY_NOTE)), (native, !native), "{ext}");
        }
        // a dot in a name without one of the extensions is part of the name
        let st = h.state_mut().dialogs.save_options.as_mut().unwrap();
        st.set_picked(dir.join("Logo v1.2"));
        assert_eq!(st.path, dir.join("Logo v1.2.cdr"));
        cancel(h.state_mut());
    }

    /// Enter saves and Escape cancels, but not while they belong to Advanced Save Options or to a
    /// message on top.
    #[test]
    fn enter_saves_and_escape_cancels() {
        let mut h = harness();
        let i = drawing(&mut h);
        let dir = temp_dir("keys");
        let press = |h: &mut Harness<'_, AnyVectorApp>, k: egui::Key| {
            h.key_press(k);
            settle(h);
        };
        give_picked_paths(vec![Some(dir.join("A.cdr"))]);
        h.state_mut().save_doc(i, true);
        settle(&mut h);
        press(&mut h, egui::Key::Escape);
        assert!(h.state().dialogs.save_options.is_none() && !dir.join("A.cdr").exists());

        give_picked_paths(vec![Some(dir.join("B.cdr"))]);
        h.state_mut().save_doc(i, true);
        settle(&mut h);
        // Advanced Save Options has the keys while it is open
        let st = h.state_mut().dialogs.save_options.as_mut().unwrap();
        st.advanced_open = Some(Advanced { thumbnail: !st.req.advanced.thumbnail, ..st.req.advanced });
        let thumbnail = st.req.advanced.thumbnail;
        press(&mut h, egui::Key::Escape);
        assert!(state(&h).advanced_open.is_none() && state(&h).req.advanced.thumbnail == thumbnail);
        let st = h.state_mut().dialogs.save_options.as_mut().unwrap();
        st.advanced_open = Some(Advanced { thumbnail: !thumbnail, ..st.req.advanced });
        press(&mut h, egui::Key::Enter);
        assert!(state(&h).advanced_open.is_none() && state(&h).req.advanced.thumbnail != thumbnail);
        // so does a message (the .cdr note of the drawing saved before, when quitting)
        h.state_mut().dialogs.message_box = Some(("Save".into(), "A note".into()));
        press(&mut h, egui::Key::Escape);
        assert!(h.state().dialogs.message_box.is_none());
        assert!(h.state().dialogs.save_options.is_some() && !dir.join("B.cdr").exists());
        press(&mut h, egui::Key::Enter);
        assert!(h.state().dialogs.save_options.is_none() && dir.join("B.cdr").exists());
    }

    /// The document is found by its identity: one closed or opened meanwhile does not matter.
    #[test]
    fn the_drawing_is_saved_wherever_its_tab_is() {
        let mut h = harness();
        let first = drawing(&mut h);
        let second = drawing(&mut h);
        let dir = temp_dir("uid");
        give_picked_paths(vec![Some(dir.join("Second.cdr"))]);
        assert!(!h.state_mut().save_doc(second, true));
        let uid = h.state().docs[second].uid;
        h.state_mut().docs.remove(first);
        h.state_mut().active = Some(0);
        confirm(h.state_mut());
        let d = h.state().docs.iter().find(|d| d.uid == uid).unwrap();
        assert_eq!(d.path.as_deref(), Some(dir.join("Second.cdr").as_path()));
        // its own document gone: the dialog goes too
        give_picked_paths(vec![Some(dir.join("Gone.cdr"))]);
        assert!(!h.state_mut().save_doc(0, true));
        h.state_mut().docs.clear();
        h.state_mut().active = None;
        settle(&mut h);
        assert!(h.state().dialogs.save_options.is_none() && !dir.join("Gone.cdr").exists());
    }

    /// The main window is not closed under the dialog.
    #[test]
    fn the_window_stays_while_the_options_are_asked() {
        let mut h = harness();
        let i = drawing(&mut h);
        give_picked_paths(vec![Some(temp_dir("window").join("A.cdr"))]);
        assert!(!h.state_mut().save_doc(i, true));
        settle(&mut h);
        h.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
        settle(&mut h);
        assert!(!h.state().dialogs.confirm_quit && h.state().dialogs.save_options.is_some());
    }
}
