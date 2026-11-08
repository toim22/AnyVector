//! Standard toolbar and the optional toolbars (Window > Toolbars).

use crate::app::{Toolbar, VectorApp};
use crate::commands::{Cmd, OptionsPage};
use crate::dockers::DockerId;
use crate::theme;
use crate::widgets::{arrow_button, icon_button_sized, vsep};
use egui::{vec2, Popup, PopupCloseBehavior, Ui};

fn btn(app: &mut VectorApp, ui: &mut Ui, icon: &str, tip: &str, enabled: bool, checked: bool) -> egui::Response {
    let s = app.settings.button_size.icon();
    icon_button_sized(&mut app.icons, ui, icon, tip, enabled, checked, s + 14.0, s)
}

fn cmd_btn(app: &mut VectorApp, ui: &mut Ui, icon: &str, tip: &str, cmd: Cmd, enabled: bool) {
    if btn(app, ui, icon, tip, enabled, false).clicked() {
        app.run(cmd);
    }
}

pub fn standard(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    let sel = app.has_selection();
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        ui.add_space(2.0);
        cmd_btn(app, ui, "new", "New (Ctrl+N)", Cmd::New, true);
        ui.add_space(4.0);
        cmd_btn(app, ui, "open", "Open (Ctrl+O)", Cmd::Open, true);
        let r = arrow_button(ui, true, "Open Recent");
        Popup::menu(&r).show(|ui| {
            let recent = app.settings.recent_files.clone();
            if recent.is_empty() {
                ui.add_enabled(false, egui::Button::new("No recent files"));
            }
            for (i, p) in recent.iter().enumerate() {
                if ui.button(p.display().to_string()).clicked() {
                    app.run(Cmd::OpenRecent(i));
                }
            }
        });
        vsep(ui);
        cmd_btn(app, ui, "save", "Save (Ctrl+S)", Cmd::Save, doc);
        cmd_btn(app, ui, "open_cloud", "Open from Cloud", Cmd::OpenFromCloud, true);
        cmd_btn(app, ui, "save_cloud", "Save to Cloud", Cmd::SaveToCloud, doc);
        cmd_btn(app, ui, "print", "Print (Ctrl+P)", Cmd::Print, doc);
        vsep(ui);
        cmd_btn(app, ui, "cut", "Cut (Ctrl+X)", Cmd::Cut, sel);
        cmd_btn(app, ui, "copy", "Copy (Ctrl+C)", Cmd::Copy, sel);
        cmd_btn(app, ui, "paste", "Paste (Ctrl+V)", Cmd::Paste, doc && (!app.clip.objects.is_empty() || crate::exec::system_clipboard_has_content()));
        vsep(ui);
        let (can_undo, can_redo) = app.doc().map(|d| (d.history.can_undo(), d.history.can_redo())).unwrap_or((false, false));
        let undo_tip = app.doc().and_then(|d| d.history.undo_labels().first().cloned()).map(|l| format!("Undo {l} (Ctrl+Z)")).unwrap_or_else(|| "Undo (Ctrl+Z)".into());
        cmd_btn(app, ui, "undo", &undo_tip, Cmd::Undo, can_undo);
        let r = arrow_button(ui, can_undo, "Undo list");
        Popup::menu(&r).show(|ui| {
            let labels = app.doc().map(|d| d.history.undo_labels()).unwrap_or_default();
            for (i, l) in labels.iter().enumerate().take(30) {
                if ui.button(l).clicked() {
                    app.run(Cmd::UndoTo(i + 1));
                }
            }
        });
        ui.add_space(6.0);
        let redo_tip = app.doc().and_then(|d| d.history.redo_labels().first().cloned()).map(|l| format!("Redo {l} (Ctrl+Shift+Z)")).unwrap_or_else(|| "Redo (Ctrl+Shift+Z)".into());
        cmd_btn(app, ui, "redo", &redo_tip, Cmd::Redo, can_redo);
        let r = arrow_button(ui, can_redo, "Redo list");
        Popup::menu(&r).show(|ui| {
            let labels = app.doc().map(|d| d.history.redo_labels()).unwrap_or_default();
            for (i, l) in labels.iter().enumerate().take(30) {
                if ui.button(l).clicked() {
                    app.run(Cmd::RedoTo(i + 1));
                }
            }
        });
        vsep(ui);
        cmd_btn(app, ui, "import", "Import (Ctrl+I)", Cmd::Import, doc);
        cmd_btn(app, ui, "export", "Export (Ctrl+E)", Cmd::Export, doc);
        cmd_btn(app, ui, "pdf", "Publish to PDF", Cmd::PublishPdf, doc);
        vsep(ui);
        zoom_combo(app, ui, doc);
        cmd_btn(app, ui, "fullscreen", "Full-Screen Preview (F9)", Cmd::FullScreenPreview, doc);
        vsep(ui);
        let s = app.doc().map(|d| d.doc.settings.clone());
        let on = |f: fn(&vector_core::document::DocSettings) -> bool| s.as_ref().map(f).unwrap_or(false);
        if btn(app, ui, "rulers", "Show Rulers", doc, on(|s| s.show_rulers)).clicked() {
            app.run(Cmd::ToggleRulers);
        }
        if btn(app, ui, "grid", "Show Grid", doc, on(|s| s.show_grid)).clicked() {
            app.run(Cmd::ToggleGrid);
        }
        if btn(app, ui, "guidelines", "Show Guidelines", doc, on(|s| s.show_guides)).clicked() {
            app.run(Cmd::ToggleGuidelines);
        }
        vsep(ui);
        let snap_off = on(|s| s.snap_off);
        if btn(app, ui, if snap_off || !doc { "snap_off" } else { "snap" }, "Snap Off (Alt+Q)", doc, false).clicked() {
            app.run(Cmd::SnapOff);
        }
        snap_to(app, ui, doc);
        vsep(ui);
        ai_generate(app, ui);
        vsep(ui);
        let r = btn(app, ui, "gear", "Options", true, false);
        Popup::menu(&r).show(|ui| {
            ui.set_min_width(200.0);
            crate::menus::options_items(app, ui);
        });
    });
}

fn zoom_combo(app: &mut VectorApp, ui: &mut Ui, doc: bool) {
    let id = ui.make_persistent_id("zoom_combo");
    let pct = app.doc().map(|d| d.zoom_percent()).unwrap_or(100.0);
    let mut text = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_default();
    let focused = ui.memory(|m| m.has_focus(id.with("te")));
    if !focused {
        text = format!("{}%", (pct.round() as i64));
    }
    let (rect, _) = ui.allocate_exact_size(vec2(84.0, 24.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, theme::FIELD);
    let te_rect = egui::Rect::from_min_max(rect.min + vec2(2.0, 2.0), egui::pos2(rect.max.x - 18.0, rect.max.y - 2.0));
    let r = ui.put(
        te_rect,
        egui::TextEdit::singleline(&mut text).id(id.with("te")).frame(egui::Frame::NONE).text_color(if doc { theme::TEXT } else { theme::DISABLED }).interactive(doc),
    );
    if !doc {
        // the screenshot shows "100%" selected (highlighted) in the empty state
        let g = ui.painter().layout_no_wrap("100%".into(), egui::FontId::proportional(12.5), egui::Color32::WHITE);
        let tr = egui::Rect::from_min_size(te_rect.left_center() - vec2(-2.0, g.size().y / 2.0), g.size());
        ui.painter().rect_filled(tr, 0.0, theme::FOCUS);
        ui.painter().galley(tr.min, g, egui::Color32::WHITE);
    }
    if r.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }
    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        if let Ok(v) = text.trim().trim_end_matches('%').trim().parse::<f64>() {
            app.run(Cmd::ZoomSet(v));
        }
    }
    let arrow = egui::Rect::from_min_max(egui::pos2(rect.max.x - 18.0, rect.min.y), rect.max);
    let ar = ui.interact(arrow, id.with("arrow"), if doc { egui::Sense::click() } else { egui::Sense::hover() });
    crate::widgets::chevron(ui.painter(), arrow.center(), 3.5, if doc { theme::TEXT } else { theme::DISABLED });
    Popup::menu(&ar).show(|ui| {
        ui.set_min_width(140.0);
        for (label, cmd) in [
            ("To Page", Cmd::ZoomToPage),
            ("To Width", Cmd::ZoomToWidth),
            ("To Height", Cmd::ZoomToHeight),
            ("To Selected", Cmd::ZoomToSelected),
            ("To All Objects", Cmd::ZoomToAll),
        ] {
            if ui.button(label).clicked() {
                app.run(cmd);
            }
        }
        ui.separator();
        for z in [10.0, 25.0, 50.0, 75.0, 100.0, 200.0, 400.0, 800.0, 1600.0] {
            if ui.button(format!("{z}%")).clicked() {
                app.run(Cmd::ZoomSet(z));
            }
        }
    });
}

fn snap_to(app: &mut VectorApp, ui: &mut Ui, doc: bool) {
    let r = crate::widgets::dropdown_button(ui, "Snap To", 66.0, doc);
    let r = r.on_hover_text("Snap To");
    Popup::menu(&r).close_behavior(PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_min_width(170.0);
        let Some(d) = app.doc_mut() else { return };
        let s = &mut d.doc.settings;
        ui.checkbox(&mut s.snap_off, "None (Snap Off)");
        ui.separator();
        ui.checkbox(&mut s.pixel_grid, "Pixels");
        ui.checkbox(&mut s.snap_to_grid, "Document Grid");
        ui.checkbox(&mut s.snap_to_guides, "Guidelines");
        ui.checkbox(&mut s.snap_to_objects, "Objects");
        ui.checkbox(&mut s.snap_to_page, "Page");
        ui.separator();
        if ui.button("Snapping Options...").clicked() {
            app.run(Cmd::Options(OptionsPage::App));
            app.dialogs.options.page = Some("Snapping".into());
            ui.close();
        }
    });
}

fn ai_generate(app: &mut VectorApp, ui: &mut Ui) {
    let (rect, r) = ui.allocate_exact_size(vec2(106.0, 26.0), egui::Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, if r.hovered() { theme::HOVER } else { egui::Color32::from_rgb(0x33, 0x33, 0x33) });
    if let Some(t) = app.icons.get(ui.ctx(), "ai", 16.0, crate::icons::Tone::Bright) {
        p.image(t.id, egui::Rect::from_center_size(rect.left_center() + vec2(14.0, 0.0), vec2(16.0, 16.0)), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    }
    p.text(rect.left_center() + vec2(26.0, 0.0), egui::Align2::LEFT_CENTER, "AI Generate", egui::FontId::proportional(12.5), theme::TEXT);
    crate::widgets::chevron(p, rect.right_center() - vec2(9.0, 0.0), 3.0, theme::TEXT);
    let r = r.on_hover_text("AI Generate");
    Popup::menu(&r).show(|ui| {
        ui.set_min_width(180.0);
        if ui.button("AI Generate").clicked() {
            app.run(Cmd::Docker(DockerId::AiGenerate));
        }
        if ui.button("Quick AI Generate...").clicked() {
            app.run(Cmd::QuickAiGenerate);
        }
    });
}

pub fn extra(app: &mut VectorApp, ui: &mut Ui, tb: Toolbar) {
    let doc = app.has_doc();
    let sel = app.has_selection();
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        ui.label(egui::RichText::new(tb.name()).color(theme::TEXT_DIM).small());
        vsep(ui);
        match tb {
            Toolbar::Zoom => {
                cmd_btn(app, ui, "zoom_in", "Zoom In", Cmd::ZoomIn, doc);
                cmd_btn(app, ui, "zoom_out", "Zoom Out", Cmd::ZoomOut, doc);
                cmd_btn(app, ui, "zoom_selected", "Zoom to Selected", Cmd::ZoomToSelected, sel);
                cmd_btn(app, ui, "zoom_all", "Zoom to All Objects", Cmd::ZoomToAll, doc);
                cmd_btn(app, ui, "zoom_page", "Zoom to Page", Cmd::ZoomToPage, doc);
                cmd_btn(app, ui, "zoom_width", "Zoom to Width", Cmd::ZoomToWidth, doc);
                cmd_btn(app, ui, "zoom_height", "Zoom to Height", Cmd::ZoomToHeight, doc);
            }
            Toolbar::Text => {
                crate::propbar::text_controls(app, ui);
            }
            Toolbar::Layout => {
                cmd_btn(app, ui, "group", "Group", Cmd::Group, sel);
                cmd_btn(app, ui, "ungroup", "Ungroup", Cmd::Ungroup, sel);
                cmd_btn(app, ui, "combine_obj", "Combine", Cmd::Combine, sel);
                cmd_btn(app, ui, "break_apart", "Break Apart", Cmd::BreakApart, sel);
                vsep(ui);
                cmd_btn(app, ui, "to_front", "To Front of Page", Cmd::Order(vector_core::ops::Order::ToFront), sel);
                cmd_btn(app, ui, "to_back", "To Back of Page", Cmd::Order(vector_core::ops::Order::ToBack), sel);
                cmd_btn(app, ui, "forward_one", "Forward One", Cmd::Order(vector_core::ops::Order::ForwardOne), sel);
                cmd_btn(app, ui, "back_one", "Back One", Cmd::Order(vector_core::ops::Order::BackOne), sel);
                vsep(ui);
                cmd_btn(app, ui, "align", "Align and Distribute", Cmd::Docker(DockerId::AlignDistribute), doc);
            }
            Toolbar::Transform => {
                cmd_btn(app, ui, "rotate", "Rotate 90° Counterclockwise", Cmd::Rotate(90.0), sel);
                cmd_btn(app, ui, "rotate", "Rotate 90° Clockwise", Cmd::Rotate(-90.0), sel);
                cmd_btn(app, ui, "mirror_h", "Mirror Horizontally", Cmd::MirrorH, sel);
                cmd_btn(app, ui, "mirror_v", "Mirror Vertically", Cmd::MirrorV, sel);
                cmd_btn(app, ui, "transform", "Transform", Cmd::Docker(DockerId::Transform), doc);
                cmd_btn(app, ui, "close_x", "Clear Transformations", Cmd::ClearTransformations, sel);
            }
            Toolbar::Macros => {
                let rec = app.recording.is_some();
                cmd_btn(app, ui, "script", "Script Editor", Cmd::ScriptEditor, true);
                cmd_btn(app, ui, "run", "Run Script", Cmd::RunScript, true);
                cmd_btn(app, ui, "record", "Start Recording", Cmd::StartRecording, doc && !rec);
                cmd_btn(app, ui, "pause", "Pause Recording", Cmd::PauseRecording, rec);
                cmd_btn(app, ui, "stop", "Stop Recording", Cmd::StopRecording, rec);
            }
            Toolbar::Internet => {
                cmd_btn(app, ui, "links", "Links and Rollovers", Cmd::Docker(DockerId::LinksRollovers), doc);
                cmd_btn(app, ui, "export", "Export for Web", Cmd::ExportForWeb, doc);
            }
            Toolbar::ProjectTimer => {
                let secs = app.time() as u64;
                ui.label(format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60));
                ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
            }
            Toolbar::ParallelDrawing => {
                let on = app.ts.parallel_drawing;
                if btn(app, ui, "two_point_line", "Parallel Drawing", doc, on).clicked() {
                    app.ts.parallel_drawing = !on;
                }
                ui.label("Lines:");
                ui.add(egui::DragValue::new(&mut app.ts.parallel_count).range(1..=20));
                ui.label("Distance:");
                ui.add(egui::DragValue::new(&mut app.ts.parallel_distance).range(0.5..=200.0).suffix(" pt"));
            }
            _ => {}
        }
    });
}
