//! The Object Data docker's dialogs: Object Data Manager (the spreadsheet) and the Object
//! Data Field Editor.

use super::{button, window};
use crate::app::AnyVectorApp;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use vector_core::{ObjKind, Object};

/// Object Data Field Editor: the selected fields and the one being renamed.
#[derive(Clone, Default)]
pub struct FieldEditorState {
    pub sel: Vec<usize>,
    pub rename: Option<(usize, String)>,
}

/// The document's fields, without the hidden CDRStaticID.
fn fields(app: &AnyVectorApp) -> Vec<String> {
    app.doc().map(|d| d.doc.data_fields.iter().filter(|f| *f != "CDRStaticID").cloned().collect()).unwrap_or_default()
}

fn header(ui: &mut Ui, r: Rect, text: &str) {
    ui.painter().rect_filled(r, 0.0, theme::BUTTON());
    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, theme::SEP()), egui::StrokeKind::Inside);
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(12.5), theme::TEXT());
}

pub fn field_editor(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.data_field_editor.clone() else { return };
    if !app.has_doc() {
        app.dialogs.data_field_editor = None;
        return;
    }
    let list = fields(app);
    let has_sel_objects = app.has_selection();
    let mut close = false;
    let mut new_field = false;
    let mut add_to_selection = false;
    let mut delete = false;
    let mut renamed: Option<(usize, String)> = None;
    let ((), dismissed) = window(ctx, "data_field_editor", "Object Data Field Editor", vec2(380.0, 300.0), |ui| {
        ui.add_space(8.0);
        let top = ui.cursor().top();
        let left = ui.max_rect().left();
        let lw = 180.0;
        header(ui, Rect::from_min_size(pos2(left, top), vec2(lw, 22.0)), "Name");
        let list_rect = Rect::from_min_size(pos2(left, top + 22.0), vec2(lw, 200.0));
        ui.painter().rect_filled(list_rect, 0.0, theme::FIELD());
        ui.scope_builder(egui::UiBuilder::new().max_rect(list_rect.shrink(1.0)), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, f) in list.iter().enumerate() {
                if let Some((k, name)) = st.rename.as_mut().filter(|(k, _)| *k == i) {
                    let r = ui.add(egui::TextEdit::singleline(name).desired_width(lw - 4.0));
                    if r.lost_focus() {
                        renamed = Some((*k, name.trim().to_string()));
                    } else {
                        r.request_focus();
                    }
                    continue;
                }
                let on = st.sel.contains(&i);
                let (r, resp) = ui.allocate_exact_size(vec2(lw - 2.0, 24.0), Sense::click());
                if on {
                    ui.painter().rect_filled(r, 0.0, theme::ACCENT());
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, theme::HOVER());
                }
                ui.painter().text(pos2(r.left() + 4.0, r.center().y), egui::Align2::LEFT_CENTER, f, egui::FontId::proportional(13.0), theme::TEXT());
                if resp.clicked() {
                    if ui.input(|i| i.modifiers.ctrl) {
                        if on {
                            st.sel.retain(|x| *x != i);
                        } else {
                            st.sel.push(i);
                        }
                    } else {
                        st.sel = vec![i];
                    }
                }
                if resp.double_clicked() {
                    st.rename = Some((i, f.clone()));
                }
            }
        });
        let br = Rect::from_min_size(pos2(left + lw + 20.0, top), vec2(ui.max_rect().right() - left - lw - 20.0, 120.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(br), |ui| {
            let w = br.width();
            if ui.add(egui::Button::new(egui::RichText::new("New Field").color(Color32::WHITE)).fill(theme::PRIMARY()).min_size(vec2(w, 24.0))).clicked() {
                new_field = true;
            }
            ui.add_space(4.0);
            if ui.add_enabled(has_sel_objects && !st.sel.is_empty(), egui::Button::new("Add Selected Field(s)").min_size(vec2(w, 24.0))).clicked() {
                add_to_selection = true;
            }
            ui.add_space(4.0);
            if ui.add_enabled(!st.sel.is_empty(), egui::Button::new("Delete Field(s)").min_size(vec2(w, 24.0))).clicked() {
                delete = true;
            }
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            if button(ui, "Close", true) {
                close = true;
            }
        });
    });
    let Some(d) = app.doc_mut() else { return };
    if let Some((i, name)) = renamed {
        st.rename = None;
        if let Some(old) = list.get(i).cloned() {
            if !name.is_empty() && name != old && !list.contains(&name) {
                d.modify("Rename Field", |doc, _| {
                    if let Some(f) = doc.data_fields.iter_mut().find(|f| **f == old) {
                        *f = name.clone();
                    }
                    for p in doc.pages.iter_mut() {
                        for l in p.layers.iter_mut() {
                            for o in l.objects.iter_mut() {
                                o.walk_mut(&mut |x| {
                                    for e in x.data.iter_mut().filter(|e| e.0 == old) {
                                        e.0 = name.clone();
                                    }
                                });
                            }
                        }
                    }
                });
            }
        }
    }
    if new_field {
        let mut k = 1;
        while list.contains(&format!("Field{k}")) {
            k += 1;
        }
        let name = format!("Field{k}");
        d.modify("New Field", |doc, _| {
            let at = doc.data_fields.iter().position(|f| f == "CDRStaticID").unwrap_or(doc.data_fields.len());
            doc.data_fields.insert(at, name.clone());
        });
        st.sel = vec![list.len()];
        st.rename = Some((list.len(), name));
    }
    if add_to_selection {
        let names: Vec<String> = st.sel.iter().filter_map(|i| list.get(*i).cloned()).collect();
        let sel = d.selection.clone();
        d.modify("Add Fields", |doc, _| {
            for id in &sel {
                if let Some(o) = doc.find_mut(*id) {
                    for n in &names {
                        if !o.data.iter().any(|e| e.0 == *n) {
                            o.data.push((n.clone(), String::new()));
                        }
                    }
                }
            }
        });
    }
    if delete {
        let names: Vec<String> = st.sel.iter().filter_map(|i| list.get(*i).cloned()).collect();
        d.modify("Delete Fields", |doc, _| {
            doc.data_fields.retain(|f| !names.contains(f));
            for p in doc.pages.iter_mut() {
                for l in p.layers.iter_mut() {
                    for o in l.objects.iter_mut() {
                        o.walk_mut(&mut |x| x.data.retain(|e| !names.contains(&e.0)));
                    }
                }
            }
        });
        st.sel.clear();
    }
    app.dialogs.data_field_editor = if close || dismissed { None } else { Some(st) };
}

/// The rows of the Object Data Manager: the selected objects (else the page's), with the
/// objects of groups down to `levels`.
fn rows(app: &AnyVectorApp, levels: usize) -> Vec<(usize, Object)> {
    let Some(d) = app.doc() else { return vec![] };
    let top: Vec<Object> = if d.selection.is_empty() { d.page_ref().layers.iter().flat_map(|l| l.objects.iter().cloned()).collect() } else { d.selection.iter().filter_map(|id| d.doc.find(*id).cloned()).collect() };
    fn push(out: &mut Vec<(usize, Object)>, o: &Object, depth: usize, levels: usize) {
        out.push((depth, o.clone()));
        if depth + 1 < levels {
            if let ObjKind::Group(kids) = &o.kind {
                for k in kids {
                    push(out, k, depth + 1, levels);
                }
            }
        }
    }
    let mut out = vec![];
    for o in &top {
        push(&mut out, o, 0, levels);
    }
    out
}

pub fn data_manager(app: &mut AnyVectorApp, ctx: &egui::Context) {
    if !app.has_doc() {
        app.dialogs.object_data_manager = false;
        return;
    }
    let cols = fields(app);
    let mut levels = if app.dialogs.data_levels == 0 { 10 } else { app.dialogs.data_levels.min(99) };
    let rows = rows(app, levels);
    let mut edit: Option<(u64, String, String)> = None;
    let ((), dismissed) = window(ctx, "object_data_manager", "Object Data Manager", vec2(640.0, 420.0), |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Show levels:");
            ui.add(egui::DragValue::new(&mut levels).range(1..=99));
        });
        ui.add_space(6.0);
        let cw = ((ui.available_width() - 60.0) / cols.len().max(1) as f32).clamp(80.0, 180.0);
        egui::ScrollArea::both().id_salt("odm").auto_shrink([false, false]).max_height(ui.available_height() - 34.0).show(ui, |ui| {
            egui::Grid::new("odm_grid").striped(true).min_col_width(40.0).spacing(vec2(6.0, 4.0)).show(ui, |ui| {
                ui.label("");
                for c in &cols {
                    ui.add_sized(vec2(cw, 20.0), egui::Label::new(egui::RichText::new(c).strong()));
                }
                ui.end_row();
                for (i, (depth, o)) in rows.iter().enumerate() {
                    ui.label(format!("{}{}", "  ".repeat(*depth), i + 1));
                    for c in &cols {
                        let cur = o.data.iter().find(|e| e.0 == *c).map(|e| e.1.clone()).unwrap_or_default();
                        let id = egui::Id::unique(("odm_cell", o.id, c.as_str()));
                        let mut v = ui.data_mut(|m| m.get_temp::<String>(id)).unwrap_or_else(|| cur.clone());
                        let r = ui.add_sized(vec2(cw, 20.0), egui::TextEdit::singleline(&mut v));
                        if r.changed() {
                            ui.data_mut(|m| m.insert_temp(id, v.clone()));
                        }
                        if r.lost_focus() {
                            ui.data_mut(|m| m.remove::<String>(id));
                            if v != cur {
                                edit = Some((o.id, c.clone(), v));
                            }
                        }
                    }
                    ui.end_row();
                }
                // totals of the numeric columns
                ui.label(egui::RichText::new("TOTAL").strong());
                for c in &cols {
                    let vals: Vec<f64> = rows.iter().filter(|r| r.0 == 0).filter_map(|(_, o)| o.data.iter().find(|e| e.0 == *c)).filter_map(|e| e.1.trim().trim_start_matches('$').replace(',', "").parse::<f64>().ok()).collect();
                    if c == "Cost" || !vals.is_empty() {
                        let t: f64 = vals.iter().sum::<f64>() + 0.0;
                        ui.label(if c == "Cost" { format!("${t:.2}") } else { format!("{t}") });
                    } else {
                        ui.label("");
                    }
                }
                ui.end_row();
            });
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            if button(ui, "Close", true) {
                app.dialogs.object_data_manager = false;
            }
        });
    });
    app.dialogs.data_levels = levels;
    if dismissed {
        app.dialogs.object_data_manager = false;
    }
    if let Some((id, k, v)) = edit {
        if let Some(d) = app.doc_mut() {
            d.modify("Object Data", |doc, _| {
                if let Some(o) = doc.find_mut(id) {
                    match o.data.iter_mut().find(|e| e.0 == k) {
                        Some(e) => e.1 = v,
                        None => o.data.push((k, v)),
                    }
                }
            });
        }
    }
}
