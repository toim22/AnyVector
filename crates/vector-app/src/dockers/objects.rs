//! Objects docker (pages › layers › objects tree) and Pages docker.

use super::{hint, no_doc};
use crate::app::VectorApp;
use crate::commands::Cmd;
use crate::icons::Tone;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Ui};
use vector_core::{Layer, ObjId, Object};

fn icon_toggle(app: &mut VectorApp, ui: &mut Ui, on_icon: &str, off_icon: &str, on: bool, tip: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 2.0, theme::HOVER);
    }
    if let Some(t) = app.icons.get(ui.ctx(), if on { on_icon } else { off_icon }, 14.0, if on { Tone::Normal } else { Tone::Disabled }) {
        ui.painter().image(t.id, Rect::from_center_size(r.center(), vec2(14.0, 14.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    resp.on_hover_text(tip).clicked()
}

pub fn objects(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    // toolbar
    ui.horizontal(|ui| {
        if crate::widgets::icon_button(&mut app.icons, ui, "layer_new", "New Layer", true, false).clicked() {
            if let Some(d) = app.doc_mut() {
                let pi = d.page;
                d.modify("New Layer", |doc, _| {
                    let id = doc.new_id();
                    let n = doc.pages[pi].layers.len() + 1;
                    doc.pages[pi].layers.push(Layer::new(id, &format!("Layer {n}")));
                });
                d.layer = d.doc.pages[pi].layers.len() - 1;
            }
        }
        let can_del = app.doc().map(|d| d.page_ref().layers.len() > 1).unwrap_or(false);
        if crate::widgets::icon_button(&mut app.icons, ui, "delete", "Delete Layer", can_del, false).clicked() {
            if let Some(d) = app.doc_mut() {
                let (pi, li) = (d.page, d.layer);
                d.modify("Delete Layer", |doc, _| {
                    doc.pages[pi].layers.remove(li);
                });
            }
        }
        ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.objects_filter).hint_text("Search objects").desired_width(ui.available_width()));
    });
    ui.separator();
    let Some(d) = app.doc() else { return };
    let pi = d.page;
    let layers: Vec<(usize, Layer)> = d.page_ref().layers.iter().cloned().enumerate().rev().collect();
    let active_layer = d.layer;
    let filter = app.dockers.ui.objects_filter.to_lowercase();
    ui.label(egui::RichText::new(d.doc.page_name(pi)).strong());
    for (li, layer) in layers {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            if icon_toggle(app, ui, "eye", "eye_off", layer.visible, "Show or hide") {
                set_layer(app, pi, li, |l| l.visible = !l.visible);
            }
            if icon_toggle(app, ui, "printer_small", "no_print", layer.printable, "Enable or disable printing and exporting") {
                set_layer(app, pi, li, |l| l.printable = !l.printable);
            }
            if icon_toggle(app, ui, "lock", "unlock", layer.locked, "Lock or unlock") {
                set_layer(app, pi, li, |l| l.locked = !l.locked);
            }
            let (sw, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().rect_filled(sw, 1.0, theme::to_c32(layer.color));
            if let Some((rl, name)) = app.dockers.ui.layer_rename.as_mut().filter(|(l, _)| *l == li) {
                let _ = rl;
                let r = ui.text_edit_singleline(name);
                if r.lost_focus() {
                    let n = name.clone();
                    app.dockers.ui.layer_rename = None;
                    set_layer(app, pi, li, move |l| l.name = n.clone());
                } else {
                    r.request_focus();
                }
            } else {
                let r = ui.selectable_label(li == active_layer, egui::RichText::new(&layer.name).strong());
                if r.clicked() {
                    if let Some(d) = app.doc_mut() {
                        d.layer = li;
                    }
                }
                if r.double_clicked() {
                    app.dockers.ui.layer_rename = Some((li, layer.name.clone()));
                }
            }
        });
        for o in layer.objects.iter().rev() {
            object_row(app, ui, o, 1, &filter);
        }
    }
    // master guides & desktop
    ui.add_space(6.0);
    hint(ui, "Double-click a name to rename. Ctrl+click to add to the selection.");
}

fn set_layer(app: &mut VectorApp, pi: usize, li: usize, f: impl Fn(&mut Layer)) {
    if let Some(d) = app.doc_mut() {
        d.modify("Layer Properties", |doc, _| {
            if let Some(l) = doc.pages[pi].layers.get_mut(li) {
                f(l);
            }
        });
    }
}

fn object_row(app: &mut VectorApp, ui: &mut Ui, o: &Object, depth: usize, filter: &str) {
    let name = o.display_name();
    let matches = filter.is_empty() || name.to_lowercase().contains(filter);
    let selected = app.doc().map(|d| d.selection.contains(&o.id)).unwrap_or(false);
    if matches {
        ui.horizontal(|ui| {
            ui.add_space(8.0 + depth as f32 * 14.0);
            if icon_toggle(app, ui, "eye", "eye_off", o.visible, "Show or hide") {
                set_obj(app, o.id, "Visibility", |x| x.visible = !x.visible);
            }
            if icon_toggle(app, ui, "lock", "unlock", o.locked, "Lock or unlock") {
                set_obj(app, o.id, "Lock", |x| x.locked = !x.locked);
            }
            // fill/outline swatches
            let (sw, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
            crate::widgets::paint_well(ui.painter(), sw, o.style.fill.main_color());
            if app.dockers.ui.rename.as_ref().map(|(id, _)| *id == o.id).unwrap_or(false) {
                let (_, n) = app.dockers.ui.rename.as_mut().unwrap();
                let r = ui.text_edit_singleline(n);
                if r.lost_focus() {
                    let n = n.clone();
                    app.dockers.ui.rename = None;
                    set_obj(app, o.id, "Rename", move |x| x.name = n.clone());
                } else {
                    r.request_focus();
                }
            } else {
                let r = ui.selectable_label(selected, &name);
                if r.clicked() {
                    let add = ui.input(|i| i.modifiers.ctrl || i.modifiers.shift);
                    if let Some(d) = app.doc_mut() {
                        if add {
                            if let Some(p) = d.selection.iter().position(|x| *x == o.id) {
                                d.selection.remove(p);
                            } else {
                                d.selection.push(o.id);
                            }
                        } else {
                            d.selection = vec![o.id];
                        }
                        if let Some((_, l, _)) = d.doc.locate(o.id) {
                            d.layer = l;
                        }
                    }
                }
                if r.double_clicked() {
                    app.dockers.ui.rename = Some((o.id, o.name.clone()));
                }
                r.context_menu(|ui| {
                    if let Some(d) = app.doc_mut() {
                        d.selection = vec![o.id];
                    }
                    for (label, cmd) in [("Bring to Front", Cmd::Order(vector_core::ops::Order::ToFront)), ("Send to Back", Cmd::Order(vector_core::ops::Order::ToBack)), ("Delete", Cmd::Delete), ("Duplicate", Cmd::Duplicate)] {
                        if ui.button(label).clicked() {
                            app.run(cmd);
                            ui.close();
                        }
                    }
                });
            }
        });
    }
    for c in o.children().into_iter().rev() {
        object_row(app, ui, c, depth + 1, filter);
    }
}

fn set_obj(app: &mut VectorApp, id: ObjId, label: &str, f: impl Fn(&mut Object)) {
    if let Some(d) = app.doc_mut() {
        d.modify(label, |doc, _| {
            if let Some(o) = doc.find_mut(id) {
                f(o);
            }
        });
    }
}

pub fn pages(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    ui.horizontal(|ui| {
        if crate::widgets::icon_button(&mut app.icons, ui, "page_new", "Insert Page", true, false).clicked() {
            app.run(Cmd::InsertPage);
        }
        if crate::widgets::icon_button(&mut app.icons, ui, "copy", "Duplicate Page", true, false).clicked() {
            app.run(Cmd::DuplicatePage);
        }
        let n = app.doc().map(|d| d.doc.pages.len()).unwrap_or(1);
        if crate::widgets::icon_button(&mut app.icons, ui, "delete", "Delete Page", n > 1, false).clicked() {
            app.run(Cmd::DeletePage);
        }
    });
    ui.separator();
    let Some(d) = app.doc() else { return };
    let count = d.doc.pages.len();
    let cur = d.page;
    let unit = d.doc.settings.unit;
    let dpi = d.doc.settings.dpi;
    let infos: Vec<(String, f64, f64)> = (0..count).map(|i| (d.doc.page_name(i), d.doc.pages[i].width, d.doc.pages[i].height)).collect();
    for (i, (name, w, h)) in infos.into_iter().enumerate() {
        let thumb_w = 64.0f32;
        let thumb_h = (thumb_w as f64 * h / w.max(1.0)).min(90.0) as f32;
        let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), thumb_h + 12.0), Sense::click());
        let p = ui.painter();
        if i == cur {
            p.rect_filled(row, 3.0, theme::PRESSED);
        } else if resp.hovered() {
            p.rect_filled(row, 3.0, theme::HOVER);
        }
        let tr = Rect::from_min_size(row.min + vec2(8.0, 6.0), vec2(thumb_w, thumb_h));
        p.rect_filled(tr, 0.0, Color32::WHITE);
        p.rect_stroke(tr, 0.0, egui::Stroke::new(1.0, theme::SEP), egui::StrokeKind::Outside);
        p.text(pos2(tr.right() + 10.0, row.center().y - 8.0), egui::Align2::LEFT_CENTER, format!("{}: {}", i + 1, name), theme::body(12.5), theme::TEXT);
        p.text(pos2(tr.right() + 10.0, row.center().y + 9.0), egui::Align2::LEFT_CENTER, format!("{} × {}", unit.format(w, dpi), unit.format(h, dpi)), theme::body(11.0), theme::TEXT_DIM);
        if resp.clicked() {
            if let Some(d) = app.doc_mut() {
                d.page = i;
                d.layer = 0;
                d.selection.clear();
                d.needs_fit = true;
            }
        }
        resp.context_menu(|ui| {
            if let Some(d) = app.doc_mut() {
                d.page = i;
            }
            for (label, cmd) in [("Insert Page", Cmd::InsertPage), ("Duplicate Page", Cmd::DuplicatePage), ("Rename Page...", Cmd::RenamePage), ("Delete Page", Cmd::DeletePage), ("Switch Page Orientation", Cmd::SwitchOrientation)] {
                if ui.button(label).clicked() {
                    app.run(cmd);
                    ui.close();
                }
            }
        });
    }
}
