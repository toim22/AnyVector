//! Edit ▸ Paste Special (choose the clipboard format to paste) and Edit ▸ Copy Properties From
//! (choose which properties to copy, then click the object to copy them from).

use super::{button, ok_button, window};
use crate::app::VectorApp;
use crate::theme;
use egui::{vec2, Color32};

/// The formats Paste Special can paste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteKind {
    /// Objects copied in Vector.
    Native,
    Svg,
    Text,
    Bitmap,
}

impl PasteKind {
    pub fn name(self) -> &'static str {
        match self {
            PasteKind::Native => "AnyVector Graphic",
            PasteKind::Svg => "SVG Document",
            PasteKind::Text => "Text",
            PasteKind::Bitmap => "Device Independent Bitmap",
        }
    }
    fn result(self) -> &'static str {
        match self {
            PasteKind::Native => "Inserts the contents of the clipboard into your document as AnyVector objects.",
            PasteKind::Svg => "Inserts the contents of the clipboard into your document as SVG objects.",
            PasteKind::Text => "Inserts the contents of the clipboard into your document as Text.",
            PasteKind::Bitmap => "Inserts the contents of the clipboard into your document as a bitmap.",
        }
    }
}

pub struct PasteSpecialState {
    pub source: String,
    pub kinds: Vec<PasteKind>,
    pub chosen: usize,
}

/// What the clipboard holds, best format first.
pub fn clipboard_kinds(app: &VectorApp) -> (Vec<PasteKind>, String) {
    let mut kinds = vec![];
    let mut text = None;
    let mut image = false;
    if let Ok(mut cb) = arboard::Clipboard::new() {
        text = cb.get_text().ok();
        image = cb.get_image().is_ok();
    }
    let ours = text.as_deref().is_some_and(|t| t.contains("data-vector-clip"));
    if (ours || (text.is_none() && !image)) && !app.clip.objects.is_empty() {
        kinds.push(PasteKind::Native);
    }
    if let Some(t) = &text {
        let t = t.trim();
        if t.starts_with("<svg") || t.starts_with("<?xml") {
            kinds.push(PasteKind::Svg);
        }
        if !t.is_empty() && !ours {
            kinds.push(PasteKind::Text);
        }
    }
    if image {
        kinds.push(PasteKind::Bitmap);
    }
    let source = if ours || kinds.first() == Some(&PasteKind::Native) { vector_core::APP_NAME.to_string() } else { "Unknown Source".to_string() };
    (kinds, source)
}

pub fn paste_special(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.paste_special.take() else { return };
    let mut ok = false;
    let mut cancel = false;
    let ((), dismissed) = window(ctx, "paste_special", "Paste Special", vec2(456.0, 267.0), |ui| {
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(330.0);
                ui.horizontal(|ui| {
                    ui.label("Source:");
                    ui.label(&st.source);
                });
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(90.0);
                        ui.add_space(26.0);
                        let _ = ui.radio(true, "Paste");
                        ui.add_space(14.0);
                        ui.add_enabled(false, egui::RadioButton::new(false, "Paste Link"));
                    });
                    ui.vertical(|ui| {
                        ui.label("As:");
                        egui::Frame::new().fill(theme::FIELD).stroke(egui::Stroke::new(1.0, theme::SEP)).inner_margin(2.0).show(ui, |ui| {
                            ui.set_min_size(vec2(226.0, 80.0));
                            egui::ScrollArea::vertical().max_height(80.0).show(ui, |ui| {
                                for (i, k) in st.kinds.iter().enumerate() {
                                    if ui.selectable_label(st.chosen == i, k.name()).clicked() {
                                        st.chosen = i;
                                    }
                                }
                            });
                        });
                    });
                });
                ui.add_space(6.0);
                super::group(ui, "Result", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("📋").size(22.0));
                        let text = st.kinds.get(st.chosen).map(|k| k.result()).unwrap_or("The clipboard is empty.");
                        ui.add(egui::Label::new(text).wrap());
                    });
                });
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ok = ok_button(ui, "OK", !st.kinds.is_empty());
                cancel = button(ui, "Cancel", true);
                ui.add_space(50.0);
                ui.add_enabled(false, egui::Checkbox::new(&mut false, "Display As Icon"));
            });
        });
    });
    if ok {
        let kind = st.kinds.get(st.chosen).copied();
        app.paste_as(kind, None);
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.paste_special = Some(st);
    }
}

#[derive(Clone, Default)]
pub struct CopyPropsState {
    pub pen: bool,
    pub color: bool,
    pub fill: bool,
    pub text: bool,
}

pub fn copy_properties(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.copy_props.take() else { return };
    // Text Properties needs text in the selection
    let has_text = app.doc().is_some_and(|d| d.selection.iter().any(|id| d.doc.find(*id).is_some_and(|o| matches!(o.kind, vector_core::ObjKind::Text(_)))));
    if !has_text {
        st.text = false;
    }
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "copy_props", "Copy Properties", vec2(424.0, 253.0), |ui| {
        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(170.0);
                ui.checkbox(&mut st.pen, "Outline Pen");
                ui.checkbox(&mut st.color, "Outline Color");
                ui.checkbox(&mut st.fill, "Fill");
                ui.add_enabled(has_text, egui::Checkbox::new(&mut st.text, "Text Properties"));
            });
            ui.vertical(|ui| {
                egui::Frame::new().stroke(egui::Stroke::new(1.0, theme::SEP)).inner_margin(10.0).show(ui, |ui| {
                    ui.set_width(196.0);
                    ui.set_min_height(86.0);
                    ui.add(egui::Label::new("Hint: You may copy properties by dragging one object onto another using the right mouse button.").wrap());
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let any = st.pen || st.color || st.fill || st.text;
                    ok = ok_button(ui, "OK", any);
                    ui.add_space(8.0);
                    cancel = button(ui, "Cancel", true);
                });
            });
        });
        ui.add_space(10.0);
        ui.separator();
        ui.label(egui::RichText::new("After pressing OK, choose the object to copy from.").color(Color32::from_gray(0xc8)));
    });
    if ok {
        app.set_tool(crate::tools::ToolId::Pick);
        app.ts.pick_target = Some(crate::tools::PickTarget::CopyProperties { pen: st.pen, color: st.color, fill: st.fill, text: st.text });
        app.flash("Click the object to copy properties from.");
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.copy_props = Some(st);
    }
}
