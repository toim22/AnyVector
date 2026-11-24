//! File ▸ Save As on Linux: the system save dialog (rfd) picks the folder, name and type, then
//! this dialog shows what the Windows Save Drawing dialog adds to its own (see `save_dialog.rs`):
//! Title, Subject and Rating; for AnyVector's formats Embed color profiles, Embed fonts, the
//! version and Advanced...; for the other types the note that a copy is written. Save writes the
//! drawing with these choices, Cancel drops the save, as cancelling the Windows dialog does.
//!
//! The dialog is drawn over frames, so the save that opened it has already returned: the state
//! carries what to do afterwards ([`AfterSave`]: close the document, go on quitting).

use super::{button, combo, note, ok_button, ok_cancel_plain, row, separator, window};
use crate::app::AnyVectorApp;
use crate::exec::AfterSave;
use crate::save_dialog::{has_drawing_options, size_text, Advanced, SaveRequest, SaveResult, CDR_NOTE, COPY_NOTE};
use egui::vec2;
use std::path::PathBuf;

pub struct SaveOptionsState {
    /// The document being saved.
    pub doc: usize,
    /// Where the system dialog said to save.
    pub path: PathBuf,
    /// Into `req.types`: the type of `path`.
    pub type_index: usize,
    /// The values shown, changed in place (title, subject, rating, embedding, version, advanced).
    pub req: SaveRequest,
    /// Advanced Save Options while it is open: the values being changed.
    pub advanced_open: Option<Advanced>,
    pub then: AfterSave,
}

impl SaveOptionsState {
    pub fn new(doc: usize, req: SaveRequest, path: PathBuf, then: AfterSave) -> Self {
        let type_index = crate::exec::saved_type(&req.types, &path, req.type_index);
        Self { doc, path, type_index, req, advanced_open: None, then }
    }

    /// The extension of the chosen type.
    pub fn ext(&self) -> &str {
        self.req.types.get(self.type_index).map(|t| t.1.as_str()).unwrap_or("cdr")
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
    let order = crate::exec::save_filter_order(types.len(), req.type_index);
    let mut fd = order.into_iter().fold(rfd::FileDialog::new().set_title(crate::i18n::t("Save Drawing")), |fd, k| fd.add_filter(crate::i18n::t(&types[k].0), &[types[k].1.as_str()]));
    if let Some(dir) = &req.dir {
        fd = fd.set_directory(dir);
    }
    fd.set_file_name(req.file_name.as_str()).save_file()
}

/// Save with the values shown, then go on with what started the save.
pub fn confirm(app: &mut AnyVectorApp) {
    let Some(st) = app.dialogs.save_options.take() else { return };
    let saved = app.save_with(st.doc, st.result());
    app.after_save(st.doc, st.then, saved);
}

/// Cancel: nothing is saved, and Close All or Exit stop, as when the Windows dialog is cancelled.
pub fn cancel(app: &mut AnyVectorApp) {
    let Some(st) = app.dialogs.save_options.take() else { return };
    app.after_save(st.doc, st.then, false);
}

/// Browse...: the system dialog again, to change the folder, name or type.
fn browse(st: &mut SaveOptionsState) {
    let mut req_dir = st.path.parent().map(|p| p.to_path_buf());
    std::mem::swap(&mut st.req.dir, &mut req_dir);
    let name = std::mem::replace(&mut st.req.file_name, st.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    let start_type = std::mem::replace(&mut st.req.type_index, st.type_index);
    let picked = pick_path(&st.req);
    st.req.dir = req_dir;
    st.req.file_name = name;
    st.req.type_index = start_type;
    if let Some(p) = picked {
        st.type_index = crate::exec::saved_type(&st.req.types, &p, st.type_index);
        st.path = p;
    }
}

const LABEL_W: f32 = 100.0;

pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(st) = app.dialogs.save_options.as_mut() else { return };
    if st.doc >= app.docs.len() {
        app.dialogs.save_options = None;
        return;
    }
    // the Advanced dialog on top takes Escape and Enter
    let advanced_was_open = st.advanced_open.is_some();
    let native = has_drawing_options(st.ext());
    let (mut save, mut cancel_clicked, mut browse_clicked) = (false, false, false);
    let size = if native { vec2(540.0, 482.0) } else { vec2(540.0, 330.0) };
    let ((), dismissed) = window(ctx, "save_options", "Save Drawing", size, |ui| {
        ui.add_space(8.0);
        let field_w = ui.available_width() - LABEL_W - 8.0;
        let name = st.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let folder = st.path.parent().map(|p| p.display().to_string()).unwrap_or_default();
        row(ui, "File name:", LABEL_W, |ui| {
            ui.add_sized(vec2(field_w - 90.0, 22.0), egui::Label::new(egui::RichText::new(name).strong()).truncate().halign(egui::Align::Min));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                browse_clicked = button(ui, "Browse...", true);
            });
        });
        row(ui, "Folder:", LABEL_W, |ui| {
            ui.add_sized(vec2(field_w, 20.0), egui::Label::new(folder).truncate().halign(egui::Align::Min));
        });
        let type_name = st.req.types.get(st.type_index).map(|t| t.0.clone()).unwrap_or_default();
        row(ui, "Save as type:", LABEL_W, |ui| {
            ui.add_sized(vec2(field_w, 20.0), egui::Label::new(type_name).truncate().halign(egui::Align::Min));
        });
        separator(ui);
        // the shell's property fields of the Windows dialog
        row(ui, "Title:", LABEL_W, |ui| {
            ui.add(egui::TextEdit::singleline(&mut st.req.title).desired_width(field_w));
        });
        row(ui, "Subject:", LABEL_W, |ui| {
            ui.add(egui::TextEdit::singleline(&mut st.req.subject).desired_width(field_w));
        });
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
            ui.add(egui::Label::new(egui::RichText::new(CDR_NOTE).color(crate::theme::TEXT_DIM()).size(12.0)).wrap());
        } else {
            note(ui, COPY_NOTE);
        }
        // Advanced... sits next to Save, as on Windows; it belongs to AnyVector's formats only
        let rect = egui::Rect::from_min_max(egui::pos2(ui.max_rect().left(), ui.max_rect().bottom() - 24.0), ui.max_rect().right_bottom());
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cancel_clicked = button(ui, "Cancel", true);
                ui.add_space(4.0);
                save = ok_button(ui, "Save", true);
                if native {
                    ui.add_space(4.0);
                    if button(ui, "Advanced...", true) {
                        st.advanced_open = Some(st.req.advanced);
                    }
                }
            });
        });
    });
    if let Some(a) = st.advanced_open.as_mut() {
        match advanced_dialog(ctx, a) {
            Some(true) => {
                st.req.advanced = *a;
                st.advanced_open = None;
            }
            Some(false) => st.advanced_open = None,
            None => {}
        }
    }
    if advanced_was_open {
        return;
    }
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter)) && !egui::Popup::is_any_open(ctx);
    if browse_clicked {
        browse(st);
    } else if save || enter {
        confirm(app);
    } else if cancel_clicked || dismissed {
        cancel(app);
    }
}

/// Advanced Save Options, as the Windows dialog behind Advanced... (also on Options ▸ Document ▸
/// Save). Some(true) on OK, Some(false) on Cancel, None while open.
fn advanced_dialog(ctx: &egui::Context, a: &mut Advanced) -> Option<bool> {
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
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let (ok, cancel) = ok_cancel_plain(ui, true);
        (ok || enter, cancel)
    });
    if ok {
        Some(true)
    } else if cancel || dismissed {
        Some(false)
    } else {
        None
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
    use std::path::Path;

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
        click(&mut h, "Cancel");
        assert!(state(&h).advanced_open.is_none() && state(&h).req.advanced.thumbnail, "cancelled");
        click(&mut h, "Advanced...");
        click(&mut h, "Save thumbnail");
        click(&mut h, "Size");
        click(&mut h, "OK");
        assert!(state(&h).advanced_open.is_none());
        assert!(!state(&h).req.advanced.thumbnail && state(&h).req.advanced.optimize_for_size);
        confirm(&mut h.state_mut());
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
        confirm(&mut h.state_mut());
        assert!(!h.state().quit_requested, "the second drawing is still to save");
        assert_eq!(h.state().docs[first].path.as_deref(), Some(dir.join("One.cdr").as_path()));
        let second = state(&h).doc;
        assert_ne!(first, second);
        assert_eq!(state(&h).ext(), "vct");
        confirm(&mut h.state_mut());
        assert!(h.state().quit_requested, "all saved: quit");
        assert!(Path::new(&dir.join("Two.vct")).exists());
        // Cancel in the options does not quit
        let mut h = harness();
        drawing(&mut h);
        give_picked_paths(vec![Some(dir.join("Three.cdr"))]);
        h.state_mut().dialogs.confirm_quit = true;
        settle(&mut h);
        click(&mut h, "Yes");
        cancel(&mut h.state_mut());
        assert!(!h.state().quit_requested && !h.state().allow_close);
    }
}
