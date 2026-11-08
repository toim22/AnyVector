//! Home tab (Welcome screen). As in the reference screenshots, the Home view is an empty
//! desktop area; documents are created/opened from the File menu, the standard toolbar or
//! the `+` tab, or by dropping files here.

use crate::app::VectorApp;
use egui::{Sense, Ui};

pub fn welcome(app: &mut VectorApp, ui: &mut Ui) {
    let rect = ui.available_rect_before_wrap();
    let resp = ui.interact(rect, ui.scope_id().with("home_area"), Sense::click());
    if resp.double_clicked() {
        app.run(crate::commands::Cmd::New);
    }
    // dropped-file hint while dragging files over the window
    let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
    if hovering {
        ui.painter().rect_stroke(rect.shrink(12.0), 6.0, egui::Stroke::new(2.0, crate::theme::FOCUS), egui::StrokeKind::Inside);
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Drop files to open them", crate::theme::body(16.0), crate::theme::TEXT);
    }
}
