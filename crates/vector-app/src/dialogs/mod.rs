//! Modal dialogs: Options, New Document, Export, Print, Document Properties, Script Editor,
//! About, Palette Editor, page dialogs, confirmations and PowerTRACE.

pub mod forms;
pub mod new_doc;
pub mod options;

use crate::app::VectorApp;
use crate::theme;
use egui::{pos2, vec2, Color32, Frame, Id, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};

pub use forms::{ExportState, PaletteEditorState, PrintState, ScriptEditorState, TraceState};
pub use new_doc::NewDocState;
pub use options::OptionsState;

/// What the user chose in a confirmation box.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    Yes,
    No,
    Cancel,
}

#[derive(Default)]
pub struct Dialogs {
    pub confirm_quit: bool,
    pub options: OptionsState,
    pub new_doc: Option<NewDocState>,
    pub export: Option<ExportState>,
    pub print: Option<PrintState>,
    pub doc_props: bool,
    pub script_editor: Option<ScriptEditorState>,
    pub about: bool,
    pub palette_editor: Option<PaletteEditorState>,
    /// Rename page: (page index, edited name).
    pub rename_page: Option<(usize, String)>,
    /// Go to page: edited 1-based page number.
    pub goto_page: Option<usize>,
    /// Confirm closing a modified document (doc index).
    pub confirm_close: Option<usize>,
    pub trace: Option<TraceState>,
    /// Simple message box (title, text).
    pub message_box: Option<(String, String)>,
    /// Resample bitmap: (width px, height px, keep ratio).
    pub resample: Option<(f64, f64, bool)>,
    /// Insert page dialog: (count, before current).
    pub insert_page: Option<(usize, bool)>,
    /// Docs still waiting for a decision while closing all / quitting.
    pub close_queue: Vec<usize>,
    pub bug_report: Option<crate::bugreport::BugReportState>,
    /// Help > About > System Info...
    pub system_info: Option<crate::about::SystemInfoState>,
    /// Help > About > Legal Notices...
    pub legal_notices: bool,
}

impl Dialogs {
    pub fn any_modal_open(&self) -> bool {
        self.confirm_quit
            || self.options.open
            || self.new_doc.is_some()
            || self.export.is_some()
            || self.print.is_some()
            || self.doc_props
            || self.script_editor.is_some()
            || self.about
            || self.palette_editor.is_some()
            || self.rename_page.is_some()
            || self.goto_page.is_some()
            || self.confirm_close.is_some()
            || self.trace.is_some()
            || self.message_box.is_some()
            || self.resample.is_some()
            || self.insert_page.is_some()
            || self.bug_report.is_some()
            || self.system_info.is_some()
            || self.legal_notices
    }
}

/// Show all open dialogs.
pub fn show(app: &mut VectorApp, ctx: &egui::Context) {
    crate::ai::quick_dialog(app, ctx);
    if app.dialogs.options.open || app.dialogs.options.page.is_some() {
        options::show(app, ctx);
    }
    forms::show_all(app, ctx);
    if app.dialogs.bug_report.is_some() {
        crate::bugreport::dialog(app, ctx);
    }
    if app.dialogs.about {
        crate::about::about(app, ctx);
    }
    if app.dialogs.system_info.is_some() {
        crate::about::system_info(app, ctx);
    }
    if app.dialogs.legal_notices {
        crate::about::legal_notices(app, ctx);
    }
}

/// A dialog window in the application style: dark frame, title text at the top left and a
/// close ✕ at the top right, centered over the main window and modal.
/// Returns the body's result and whether the dialog was dismissed (✕ or Escape).
pub fn window<R>(ctx: &egui::Context, id: &str, title: &str, size: Vec2, body: impl FnOnce(&mut Ui) -> R) -> (R, bool) {
    let mut dismissed = false;
    let resp = egui::Modal::new(Id::unique(("dlg", id)))
        .backdrop_color(Color32::from_black_alpha(70))
        .frame(Frame::new().fill(theme::BG).stroke(Stroke::new(1.0, Color32::from_rgb(0x48, 0x48, 0x48))).corner_radius(4.0).inner_margin(0.0))
        .show(ctx, |ui| {
            let (full, _) = ui.allocate_exact_size(size, Sense::hover());
            let title_rect = Rect::from_min_size(full.min, vec2(size.x, 32.0));
            let p = ui.painter();
            p.text(pos2(title_rect.left() + 11.0, title_rect.center().y), egui::Align2::LEFT_CENTER, title, egui::FontId::proportional(13.0), theme::TEXT);
            let close = Rect::from_center_size(pos2(title_rect.right() - 22.0, title_rect.center().y), vec2(30.0, 26.0));
            let cr = ui.interact(close, Id::unique(("dlg_close", id)), Sense::click());
            if cr.hovered() {
                ui.painter().rect_filled(close, 2.0, Color32::from_rgb(0xc4, 0x2b, 0x1c));
            }
            let c = close.center();
            let s = 6.0;
            let st = Stroke::new(1.2, theme::TEXT);
            ui.painter().line_segment([pos2(c.x - s, c.y - s), pos2(c.x + s, c.y + s)], st);
            ui.painter().line_segment([pos2(c.x - s, c.y + s), pos2(c.x + s, c.y - s)], st);
            if cr.clicked() {
                dismissed = true;
            }
            let body_rect = Rect::from_min_max(pos2(full.left() + 10.0, title_rect.bottom()), pos2(full.right() - 10.0, full.bottom() - 9.0));
            ui.scope_builder(UiBuilder::new().max_rect(body_rect), |ui| {
                ui.set_clip_rect(body_rect.expand(2.0));
                body(ui)
            })
            .inner
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !egui::Popup::is_any_open(ctx) {
        dismissed = true;
    }
    (resp.inner, dismissed)
}

/// The blue default button (OK) used at the bottom right of dialogs.
pub fn ok_button(ui: &mut Ui, text: &str, enabled: bool) -> bool {
    let r = ui.add_enabled(enabled, egui::Button::new(egui::RichText::new(text).color(Color32::WHITE)).fill(theme::ACCENT).min_size(vec2(80.0, 24.0)));
    r.clicked()
}

/// A normal dialog button (Cancel, Browse..., etc.).
pub fn button(ui: &mut Ui, text: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(text).min_size(vec2(75.0, 24.0))).clicked()
}

/// Bottom button row: `?` at the left; returns (ok, cancel) clicks for OK/Cancel at the right.
pub fn ok_cancel_row(ui: &mut Ui, ok_text: &str, ok_enabled: bool) -> (bool, bool) {
    let mut ok = false;
    let mut cancel = false;
    let rect = Rect::from_min_max(pos2(ui.max_rect().left(), ui.max_rect().bottom() - 24.0), ui.max_rect().right_bottom());
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::Button::new("?").min_size(vec2(24.0, 24.0))).on_hover_text("Help");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cancel = button(ui, "Cancel", true);
                ui.add_space(4.0);
                ok = ok_button(ui, ok_text, ok_enabled);
            });
        });
    });
    (ok, cancel)
}

/// Horizontal separator line spanning the available width.
pub fn separator(ui: &mut Ui) {
    ui.add_space(10.0);
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, theme::SEP));
    ui.add_space(10.0);
}

/// Bold section heading.
pub fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).strong().color(theme::TEXT).size(13.0));
    ui.add_space(6.0);
}

/// Dimmed explanatory text.
pub fn note(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(theme::TEXT_DIM).size(12.0));
}
