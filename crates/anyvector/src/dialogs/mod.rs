//! Modal dialogs: Options, New Document, Export, Print, Document Properties, Script Editor,
//! About, Palette Editor, page dialogs, confirmations and the Bitmaps menu (PowerTRACE).

pub mod bitmaps;
pub mod forms;
pub mod layout;
pub mod linestyle;
pub mod clipboard;
pub mod cutter;
pub mod docprops;
pub mod export;
pub mod merge;
pub mod new_doc;
pub mod objdata;
pub mod object;
pub mod palette;
pub mod options;
pub mod print;
pub mod symmetry;
pub mod table;
pub mod text;
pub mod workspace;

use crate::app::AnyVectorApp;
use crate::theme;
use egui::{pos2, vec2, Color32, Frame, Id, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};

pub use export::ExportState;
pub use forms::ScriptEditorState;
pub use print::PrintState;
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
    /// Properties ▸ Outline ▸ Edit Line Style.
    pub line_style: Option<linestyle::LineStyleState>,
    /// Shape tool ▸ Align Nodes: horizontally, vertically, control points.
    pub node_align: Option<[bool; 3]>,
    pub options: OptionsState,
    pub new_doc: Option<NewDocState>,
    pub export: Option<ExportState>,
    pub print: Option<PrintState>,
    pub doc_props: bool,
    pub script_editor: Option<ScriptEditorState>,
    /// Object Data docker: the spreadsheet (Object Data Manager), its levels, and the field editor.
    pub object_data_manager: bool,
    pub data_levels: usize,
    pub data_field_editor: Option<objdata::FieldEditorState>,
    pub about: bool,
    /// Window ▸ Color Palettes ▸ Palette Editor (and its Select Color).
    pub palette_editor: Option<palette::EditorState>,
    /// Rename page: (page index, edited name).
    pub rename_page: Option<(usize, String)>,
    /// Go to page: edited 1-based page number.
    pub goto_page: Option<usize>,
    /// Confirm closing a modified document (doc index).
    pub confirm_close: Option<usize>,
    /// The Bitmaps menu dialogs.
    pub bm: bitmaps::BitmapDialogs,
    /// Window ▸ Workspace: New, Delete, Import and Export.
    pub ws: workspace::WorkspaceDialogs,
    /// Simple message box (title, text).
    pub message_box: Option<(String, String)>,
    /// Layout ▸ Insert Page, Duplicate Page and Delete Page.
    pub insert_page: Option<layout::InsertPageState>,
    pub duplicate_page: Option<layout::DuplicatePageState>,
    pub delete_page: Option<layout::DeletePageState>,
    /// Docs still waiting for a decision while closing all / quitting.
    pub close_queue: Vec<usize>,
    pub bug_report: Option<crate::bugreport::BugReportState>,
    /// Help ▸ Submit Ideas and Feedback.
    pub feedback: Option<crate::bugreport::BugReportState>,
    /// Ask for the user interface language at start-up (Options ▸ Global ▸ General).
    pub ask_language: bool,
    /// Help > About > System Info...
    pub system_info: Option<crate::about::SystemInfoState>,
    /// Help > About > Legal Notices...
    pub legal_notices: bool,
    /// File > Print Merge (the data source editor).
    pub merge: Option<merge::MergeState>,
    /// Publish to PDF / PDF Settings.
    pub pdf: Option<export::PdfState>,
    /// SVG Export options.
    pub svg: Option<export::SvgState>,
    /// Export to <bitmap>, Export For Office / Web / WordPress.
    pub bitmap: Option<export::BitmapState>,
    pub paste_special: Option<clipboard::PasteSpecialState>,
    pub copy_props: Option<clipboard::CopyPropsState>,
    /// Layout ▸ Page Number Settings.
    pub page_numbers: Option<layout::PageNumberState>,
    /// Layout ▸ Border and Grommet.
    pub border_grommet: Option<layout::BorderGrommetState>,
    /// The Table menu dialogs.
    pub table: Option<table::TableDialog>,
    /// Object ▸ Create ▸ Arrowhead.
    pub arrowhead: Option<object::ArrowheadState>,
    /// Object ▸ Create ▸ Pattern Fill.
    pub pattern: Option<object::PatternState>,
    /// Object ▸ Insert ▸ Barcode.
    pub barcode: Option<object::BarcodeState>,
    /// Object ▸ Insert ▸ Object.
    pub insert_object: Option<symmetry::InsertObject>,
    /// The Text menu dialogs.
    pub tabs: Option<text::TabsState>,
    pub columns: Option<text::ColumnsState>,
    pub bullets: Option<text::BulletsState>,
    pub drop_cap: Option<text::DropCapState>,
    pub text_stats: Option<text::StatsState>,
    pub hyphens: Option<text::HyphensState>,
    pub writing: Option<text::WritingState>,
    pub text_lang: Option<text::LanguageState>,
    /// Change Case: the chosen row.
    pub change_case: Option<usize>,
    pub encode: Option<text::EncodeState>,
    pub edit_text: Option<text::EditTextState>,
    /// HPGL Export, DXF Export and Create Cut Contour.
    pub cutter: cutter::CutterDialogs,
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
            || self.palette_editor.as_ref().is_some_and(|e| !e.sampling())
            || self.rename_page.is_some()
            || self.goto_page.is_some()
            || self.confirm_close.is_some()
            || self.bm.any_open()
            || self.message_box.is_some()
            || self.insert_page.is_some()
            || self.duplicate_page.is_some()
            || self.delete_page.is_some()
            || self.bug_report.is_some()
            || self.feedback.is_some()
            || self.system_info.is_some()
            || self.legal_notices
            || self.merge.is_some()
            || self.pdf.is_some()
            || self.svg.is_some()
            || self.bitmap.is_some()
            || self.paste_special.is_some()
            || self.copy_props.is_some()
            || self.page_numbers.is_some()
            || self.border_grommet.is_some()
            || self.table.is_some()
            || self.arrowhead.is_some()
            || self.line_style.is_some()
            || self.pattern.is_some()
            || self.barcode.is_some()
            || self.tabs.is_some()
            || self.columns.is_some()
            || self.bullets.is_some()
            || self.drop_cap.is_some()
            || self.text_stats.is_some()
            || self.hyphens.is_some()
            || self.writing.is_some()
            || self.text_lang.is_some()
            || self.change_case.is_some()
            || self.encode.is_some()
            || self.edit_text.is_some()
            || self.ws.any_open()
            || self.cutter.any_open()
    }
}

/// Show all open dialogs.
pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    crate::ai::quick_dialog(app, ctx);
    crate::updater::dialogs(app, ctx);
    if app.dialogs.options.open || app.dialogs.options.page.is_some() {
        options::show(app, ctx);
    }
    forms::show_all(app, ctx);
    bitmaps::show(app, ctx);
    workspace::show(app, ctx);
    palette::show(app, ctx);
    if app.dialogs.feedback.is_some() {
        crate::bugreport::dialog(app, ctx, true);
    }
    if app.dialogs.ask_language {
        ask_language(app, ctx);
    }
    if app.dialogs.bug_report.is_some() {
        crate::bugreport::dialog(app, ctx, false);
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
    if app.dialogs.merge.is_some() {
        merge::show(app, ctx);
    }
    if app.dialogs.export.is_some() {
        export::show_export(app, ctx);
    }
    if app.dialogs.pdf.is_some() {
        export::show_pdf(app, ctx);
    }
    if app.dialogs.svg.is_some() {
        export::show_svg(app, ctx);
    }
    if app.dialogs.bitmap.is_some() {
        export::show_bitmap(app, ctx);
    }
    cutter::show(app, ctx);
    if app.dialogs.paste_special.is_some() {
        clipboard::paste_special(app, ctx);
    }
    if app.dialogs.page_numbers.is_some() {
        layout::page_number_settings(app, ctx);
    }
    if app.dialogs.border_grommet.is_some() {
        layout::border_grommet(app, ctx);
    }
    if app.dialogs.table.is_some() {
        table::show(app, ctx);
    }
    if app.dialogs.copy_props.is_some() {
        clipboard::copy_properties(app, ctx);
    }
    if app.dialogs.arrowhead.is_some() {
        object::arrowhead(app, ctx);
    }
    if app.dialogs.line_style.is_some() {
        linestyle::show(app, ctx);
    }
    if app.dialogs.pattern.is_some() {
        object::pattern(app, ctx);
    }
    if app.dialogs.barcode.is_some() {
        object::barcode(app, ctx);
    }
    if app.dialogs.insert_object.is_some() {
        symmetry::insert_object_dialog(app, ctx);
    }
    symmetry::floating_bars(app, ctx);
    symmetry::poll_ole(app, ctx);
    text::tabs(app, ctx);
    text::columns(app, ctx);
    text::bullets(app, ctx);
    text::drop_cap(app, ctx);
    text::text_stats(app, ctx);
    text::hyphens(app, ctx);
    text::writing_tools(app, ctx);
    text::language(app, ctx);
    text::change_case(app, ctx);
    text::encode(app, ctx);
    text::edit_text(app, ctx);
}

/// A dialog window in the application style: dark frame, title text at the top left and a
/// close ✕ at the top right, centered over the main window and modal.
/// Returns the body's result and whether the dialog was dismissed (✕ or Escape).
pub fn window<R>(ctx: &egui::Context, id: &str, title: &str, size: Vec2, body: impl FnOnce(&mut Ui) -> R) -> (R, bool) {
    let mut dismissed = false;
    let resp = egui::Modal::new(Id::unique(("dlg", id)))
        .backdrop_color(Color32::from_black_alpha(70))
        .frame(Frame::new().fill(theme::BG()).stroke(Stroke::new(1.0, theme::gray(0x48))).corner_radius(4.0).inner_margin(0.0))
        .show(ctx, |ui| {
            let (full, _) = ui.allocate_exact_size(size, Sense::hover());
            let title_rect = Rect::from_min_size(full.min, vec2(size.x, 32.0));
            let p = ui.painter();
            p.text(pos2(title_rect.left() + 11.0, title_rect.center().y), egui::Align2::LEFT_CENTER, title, egui::FontId::proportional(13.0), theme::TEXT());
            let close = Rect::from_center_size(pos2(title_rect.right() - 22.0, title_rect.center().y), vec2(30.0, 26.0));
            let cr = ui.interact(close, Id::unique(("dlg_close", id)), Sense::click());
            if cr.hovered() {
                ui.painter().rect_filled(close, 2.0, Color32::from_rgb(0xc4, 0x2b, 0x1c));
            }
            let c = close.center();
            let s = 6.0;
            let st = Stroke::new(1.2, theme::TEXT());
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
    let r = ui.add_enabled(enabled, egui::Button::new(egui::RichText::new(text).color(Color32::WHITE)).fill(theme::PRIMARY()).min_size(vec2(80.0, 24.0)));
    r.clicked()
}

/// A normal dialog button (Cancel, Browse..., etc.).
pub fn button(ui: &mut Ui, text: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(text).min_size(vec2(75.0, 24.0))).clicked()
}

/// Bottom button row: `?` at the left; returns (ok, cancel) clicks for OK/Cancel at the right.
pub fn ok_cancel_row(ui: &mut Ui, ok_text: &str, ok_enabled: bool) -> (bool, bool) {
    ok_cancel_at_bottom(ui, ok_text, ok_enabled, true)
}

/// Bottom button row with OK and Cancel at the right only (dialogs without a Help button).
pub fn ok_cancel_plain(ui: &mut Ui, ok_enabled: bool) -> (bool, bool) {
    ok_cancel_at_bottom(ui, "OK", ok_enabled, false)
}

fn ok_cancel_at_bottom(ui: &mut Ui, ok_text: &str, ok_enabled: bool, help: bool) -> (bool, bool) {
    let mut ok = false;
    let mut cancel = false;
    let rect = Rect::from_min_max(pos2(ui.max_rect().left(), ui.max_rect().bottom() - 24.0), ui.max_rect().right_bottom());
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal(|ui| {
            if help {
                ui.add(egui::Button::new("?").min_size(vec2(24.0, 24.0))).on_hover_text("Help");
            }
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
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, theme::SEP()));
    ui.add_space(10.0);
}

/// Bold section heading.
pub fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).strong().color(theme::TEXT()).size(13.0));
    ui.add_space(6.0);
}

/// Dimmed explanatory text.
pub fn note(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(theme::TEXT_DIM()).size(12.0));
}

/// A row of tabs across the top of a dialog page; returns true when the tab changed.
pub fn tabs(ui: &mut Ui, current: &mut usize, names: &[String]) -> bool {
    let mut changed = false;
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
    let font = egui::FontId::proportional(12.5);
    let mut x = row.left();
    for (i, n) in names.iter().enumerate() {
        let w = ui.painter().layout_no_wrap(n.clone(), font.clone(), theme::TEXT()).size().x + 22.0;
        let r = Rect::from_min_size(pos2(x, row.top()), vec2(w, 28.0));
        let resp = ui.interact(r, ui.scope_id().with(("tab", i)), Sense::click());
        let on = *current == i;
        if on {
            ui.painter().rect_filled(r, 0.0, theme::FIELD());
            ui.painter().hline(r.x_range(), r.bottom() - 1.0, Stroke::new(2.0, theme::ACCENT()));
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 0.0, theme::HOVER());
        }
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, n, font.clone(), if on { theme::TEXT() } else { theme::TEXT_DIM() });
        if resp.clicked() && !on {
            *current = i;
            changed = true;
        }
        x += w;
    }
    ui.painter().hline(row.x_range(), row.bottom() - 0.5, Stroke::new(1.0, theme::SEP()));
    ui.add_space(8.0);
    changed
}

/// A titled group box (the title on its top edge), as in the reference dialogs.
pub fn group<R>(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(6.0);
    let r = Frame::new().stroke(Stroke::new(1.0, theme::SEP())).corner_radius(3.0).inner_margin(egui::Margin { left: 10, right: 10, top: 12, bottom: 8 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        body(ui)
    });
    let at = r.response.rect.left_top() + vec2(10.0, 0.0);
    let galley = ui.painter().layout_no_wrap(title.to_string(), egui::FontId::proportional(12.0), theme::TEXT());
    let bg = Rect::from_min_size(at - vec2(3.0, galley.size().y / 2.0), galley.size() + vec2(6.0, 0.0));
    ui.painter().rect_filled(bg, 0.0, theme::BG());
    ui.painter().galley(at - vec2(0.0, galley.size().y / 2.0), galley, theme::TEXT());
    r.inner
}

/// A label of fixed width followed by the widgets `body` adds, on one row.
pub fn row<R>(ui: &mut Ui, label: &str, w: f32, body: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.add_sized(vec2(w, 22.0), egui::Label::new(label).halign(egui::Align::Min));
        body(ui)
    })
    .inner
}

/// A combo box over a list of names; returns true when the choice changed.
pub fn combo(ui: &mut Ui, id: &str, current: &mut usize, names: &[&str], width: f32) -> bool {
    let mut changed = false;
    let sel = names.get(*current).copied().unwrap_or("");
    egui::ComboBox::from_id_salt(id).selected_text(sel).width(width).show_ui(ui, |ui| {
        for (i, n) in names.iter().enumerate() {
            if ui.selectable_label(*current == i, *n).clicked() && *current != i {
                *current = i;
                changed = true;
            }
        }
    });
    changed
}

/// "Ask me the next time the application starts": the user interface language, before work begins.
fn ask_language(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let mut done = false;
    let ((), dismissed) = window(ctx, "ask_language", "User interface language", vec2(400.0, 290.0), |ui| {
        ui.add_space(10.0);
        ui.label("Choose the language of the user interface:");
        ui.add_space(6.0);
        let cur = crate::i18n::Lang::resolve(&app.settings.language);
        // two columns: English and the first half, then the rest
        let all = crate::i18n::Lang::ALL;
        let half = all.len().div_ceil(2);
        ui.columns(2, |cols| {
            for (i, l) in all.into_iter().enumerate() {
                if cols[i / half].radio(cur == l, l.native_name()).clicked() {
                    app.settings.language = l.code().into();
                }
            }
        });
        ui.add_space((ui.available_height() - 34.0).max(0.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ok_button(ui, "OK", true) {
                done = true;
            }
        });
    });
    if done || dismissed {
        app.dialogs.ask_language = false;
        app.settings.ui_language_ask = false;
        app.settings.save();
    }
}
