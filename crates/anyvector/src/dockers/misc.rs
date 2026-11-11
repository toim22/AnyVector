//! Comments, Object Data, Object Styles, Symbols, Export, Assets, Tray, Links and Rollovers,
//! Sources, Views, History, Scripts and Learn dockers.

use super::{choice, drop_mark, heading, icon_button, icon_button_resp, list_row, no_doc, separator, table_header};
use crate::app::AnyVectorApp;
use crate::commands::Cmd;
use crate::propbar::edit_sel;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Ui};
use vector_core::document::{SavedView, SymbolDef};
use vector_core::{ObjKind, Object};

fn first(app: &AnyVectorApp) -> Option<Object> {
    let d = app.doc()?;
    d.doc.find(*d.selection.first()?).cloned()
}

const MARKS: [(&str, &str); 6] = [("cm_note", "Note"), ("cm_arrow", "Arrow Markup"), ("cm_highlight", "Highlighter"), ("cm_freehand", "Freehand Markup"), ("rectangle", "Rectangle Markup"), ("ellipse", "Ellipse Markup")];

pub fn comments(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    // a comment just placed with a tool: type its text
    if let Some(id) = app.ts.comment_new.take() {
        app.dockers.ui.comment_edit = Some((id, String::new(), true));
        app.dockers.ui.comment_sel = Some(id);
    }
    let Some(d) = app.doc() else { return };
    let comments = d.doc.comments.clone();
    let any_open = comments.iter().any(|c| !c.resolved);
    let mut resolve_all = false;
    let mut delete_all = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (i, (icon, tip)) in MARKS.iter().enumerate() {
            let on = app.ts.markup == Some(i as u8);
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, on).clicked() {
                app.ts.cancel();
                app.ts.markup = if on { None } else { Some(i as u8) };
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let gear = icon_button_resp(app, ui, "gear", "Options", true);
            drop_mark(ui, gear.rect);
            egui::Popup::menu(&gear).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.set_min_width(150.0);
                ui.checkbox(&mut app.dockers.ui.comment_show, "Show Annotations");
                ui.checkbox(&mut app.dockers.ui.comment_resolved, "Show Resolved");
                ui.separator();
                if ui.add_enabled(any_open, egui::Button::new("Resolve All")).clicked() {
                    resolve_all = true;
                    ui.close();
                }
                if ui.add_enabled(!comments.is_empty(), egui::Button::new("Delete All")).clicked() {
                    delete_all = true;
                    ui.close();
                }
            });
        });
    });
    ui.add_space(4.0);
    ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.comment_search).hint_text("Search").desired_width(f32::INFINITY));
    ui.add_space(4.0);
    let q = app.dockers.ui.comment_search.trim().to_lowercase();
    let show_resolved = app.dockers.ui.comment_resolved;
    let mut select = None;
    let mut commit: Option<(u64, String, bool)> = None;
    let mut cancel_new = None;
    let mut reply = None;
    let mut toggle = None;
    let mut delete = None;
    for c in comments.iter().filter(|c| show_resolved || !c.resolved) {
        let hay = format!("{} {} {}", c.author, c.text, c.replies.iter().map(|r| format!("{} {}", r.0, r.1)).collect::<Vec<_>>().join(" ")).to_lowercase();
        if !q.is_empty() && !hay.contains(&q) {
            continue;
        }
        let sel = app.dockers.ui.comment_sel == Some(c.id);
        let page = app.doc().and_then(|d| d.doc.pages.iter().position(|p| p.id == c.page)).map(|i| i + 1).unwrap_or(1);
        let fr = egui::Frame::new().fill(if sel { theme::BUTTON() } else { theme::BG() }).stroke(egui::Stroke::new(1.0, if sel { theme::FOCUS() } else { theme::SEP() })).inner_margin(6).corner_radius(2.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if let Some(t) = app.icons.get(ui.ctx(), MARKS[c.mark.min(5) as usize].0, 16.0, crate::icons::Tone::Normal) {
                    ui.add(egui::Image::from_texture(t));
                }
                ui.label(egui::RichText::new(&c.author).strong());
                ui.label(egui::RichText::new(format!("{}  ·  Page {page}", c.date)).color(theme::TEXT_DIM()).size(11.5));
            });
            let editing = app.dockers.ui.comment_edit.as_ref().is_some_and(|e| e.0 == c.id);
            if editing {
                let (id, text, new) = app.dockers.ui.comment_edit.clone().unwrap_or_default();
                let mut t = text;
                let r = ui.add(egui::TextEdit::multiline(&mut t).desired_rows(2).desired_width(f32::INFINITY).hint_text("Type a comment"));
                let enter = r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                if esc {
                    if new {
                        cancel_new = Some(id);
                    }
                    app.dockers.ui.comment_edit = None;
                } else if enter || (r.lost_focus() && !t.trim().is_empty()) {
                    commit = Some((id, t.trim().to_string(), new));
                } else {
                    if !r.has_focus() && new {
                        r.request_focus();
                    }
                    app.dockers.ui.comment_edit = Some((id, t, new));
                }
            } else {
                let txt = egui::RichText::new(&c.text);
                ui.label(if c.resolved { txt.strikethrough().color(theme::TEXT_DIM()) } else { txt });
            }
            for (who, what) in &c.replies {
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new(who).strong());
                    ui.label(what);
                });
            }
            if sel && !editing {
                let r = ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.comment_reply).hint_text("Reply").desired_width(f32::INFINITY));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !app.dockers.ui.comment_reply.trim().is_empty() {
                    reply = Some((c.id, std::mem::take(&mut app.dockers.ui.comment_reply).trim().to_string()));
                }
                ui.horizontal(|ui| {
                    if ui.button(if c.resolved { "Re-open" } else { "Resolve" }).clicked() {
                        toggle = Some(c.id);
                    }
                    let actions = ui.button("Actions ⏷");
                    egui::Popup::menu(&actions).show(|ui| {
                        if ui.button("Edit").clicked() {
                            app.dockers.ui.comment_edit = Some((c.id, c.text.clone(), false));
                            ui.close();
                        }
                        if ui.button("Copy Text").clicked() {
                            ui.ctx().copy_text(c.text.clone());
                            ui.close();
                        }
                        if ui.button(if c.resolved { "Re-open" } else { "Resolve" }).clicked() {
                            toggle = Some(c.id);
                            ui.close();
                        }
                        if ui.button("Delete").clicked() {
                            delete = Some(c.id);
                            ui.close();
                        }
                    });
                });
            }
        });
        if fr.response.interact(Sense::click()).clicked() {
            select = Some(c.id);
        }
        ui.add_space(3.0);
    }
    if let Some(id) = select {
        app.dockers.ui.comment_sel = Some(id);
    }
    let author = if app.settings.user_name.is_empty() { std::env::var("USERNAME").unwrap_or_else(|_| "User".into()) } else { app.settings.user_name.clone() };
    if commit.is_some() {
        app.dockers.ui.comment_edit = None;
    }
    let Some(d) = app.doc_mut() else { return };
    if let Some((id, text, _)) = commit {
        d.modify("Comment", |doc, _| {
            if let Some(c) = doc.comments.iter_mut().find(|c| c.id == id) {
                c.text = text;
            }
        });
    }
    if let Some(id) = cancel_new {
        d.doc.comments.retain(|c| c.id != id);
    }
    if let Some((id, text)) = reply {
        d.modify("Reply", |doc, _| {
            if let Some(c) = doc.comments.iter_mut().find(|c| c.id == id) {
                c.replies.push((author.clone(), text));
            }
        });
    }
    if let Some(id) = toggle {
        d.modify("Resolve Comment", |doc, _| {
            if let Some(c) = doc.comments.iter_mut().find(|c| c.id == id) {
                c.resolved = !c.resolved;
            }
        });
    }
    if let Some(id) = delete {
        d.modify("Delete Comment", |doc, _| doc.comments.retain(|c| c.id != id));
    }
    if resolve_all {
        d.modify("Resolve All", |doc, _| doc.comments.iter_mut().for_each(|c| c.resolved = true));
    }
    if delete_all {
        d.modify("Delete All Comments", |doc, _| doc.comments.clear());
    }
}

// ------------------------------------------------------------------ Object Data

const SLOT_COPY_DATA: u8 = 40;

/// The fields shown: the document's fields (not the hidden CDRStaticID), then any other field
/// the object has.
fn data_fields(doc: &vector_core::Document, o: Option<&Object>) -> Vec<String> {
    let mut v: Vec<String> = doc.data_fields.iter().filter(|f| *f != "CDRStaticID").cloned().collect();
    if let Some(o) = o {
        for (k, _) in &o.data {
            if !v.contains(k) && k != "CDRStaticID" {
                v.push(k.clone());
            }
        }
    }
    v
}

pub fn object_data(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let o = first(app);
    let has = o.is_some();
    // Copy data from: the object clicked gives its fields to the selection
    if app.dockers.ui.pick_slot == SLOT_COPY_DATA {
        if let Some(t) = app.ts.picked_object.take() {
            app.dockers.ui.pick_slot = 0;
            let src = app.doc().and_then(|d| d.doc.find(t)).map(|x| x.data.clone()).unwrap_or_default();
            edit_sel(app, "Copy Data From", |x| {
                for (k, v) in &src {
                    match x.data.iter_mut().find(|e| e.0 == *k) {
                        Some(e) => e.1 = v.clone(),
                        None => x.data.push((k.clone(), v.clone())),
                    }
                }
            });
        }
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if icon_button(app, ui, "od_sheet", "Open spreadsheet", true) {
            app.dialogs.object_data_manager = true;
        }
        if icon_button(app, ui, "od_fields", "Open field editor", true) {
            app.dialogs.data_field_editor = Some(Default::default());
        }
        if icon_button(app, ui, "od_copy_from", "Copy data from", has) {
            app.dockers.ui.pick_slot = SLOT_COPY_DATA;
            app.ts.picked_object = None;
            app.ts.pick_target = Some(crate::tools::PickTarget::DockerObject);
        }
        if icon_button(app, ui, "od_clear", "Clear all fields", has) {
            edit_sel(app, "Clear Object Data", |x| x.data.clear());
        }
    });
    ui.add_space(2.0);
    let full = ui.available_width();
    let name_w = (full * 0.33).round();
    let value_w = (full * 0.33).round();
    table_header(ui, &[("Name", name_w), ("Value", name_w + value_w), ("", full)]);
    let Some(d) = app.doc() else { return };
    let fields = data_fields(&d.doc, o.as_ref());
    for f in fields {
        let numeric = f == "Cost";
        let y = ui.cursor().top();
        let (resp, _) = list_row(app, ui, 0, None, if numeric { "od_number" } else { "od_text" }, f.as_str(), None);
        let _ = resp;
        let Some(o) = &o else { continue };
        let cur = o.data.iter().find(|e| e.0 == f).map(|e| e.1.clone()).unwrap_or_default();
        let r = Rect::from_min_size(pos2(ui.min_rect().left() + name_w + 2.0, y + 2.0), vec2(full - name_w - 4.0, 20.0));
        let id = egui::Id::unique(("od_value", o.id, f.as_str()));
        let mut val = ui.data_mut(|m| m.get_temp::<String>(id)).unwrap_or_else(|| cur.clone());
        let te = ui.put(r, egui::TextEdit::singleline(&mut val).id(id.with("te")).frame(egui::Frame::NONE));
        if te.changed() {
            ui.data_mut(|m| m.insert_temp(id, val.clone()));
        }
        if te.lost_focus() {
            ui.data_mut(|m| m.remove::<String>(id));
            if val != cur {
                let (k, v) = (f.clone(), val.clone());
                edit_sel(app, "Object Data", |x| match x.data.iter_mut().find(|e| e.0 == k) {
                    Some(e) => e.1 = v.clone(),
                    None => x.data.push((k.clone(), v.clone())),
                });
            }
        }
    }
}

/// Does a symbol hold instances of other symbols?
fn nested(s: &SymbolDef) -> bool {
    let mut n = false;
    for o in &s.objects {
        o.walk(&mut |x| {
            if matches!(x.kind, ObjKind::Symbol(_)) {
                n = true;
            }
        });
    }
    n
}

/// Delete a symbol of the document; its instances become groups.
fn delete_symbol(app: &mut AnyVectorApp, id: u64) {
    let Some(d) = app.doc_mut() else { return };
    d.modify("Delete Symbol", |doc, _| {
        let def = doc.symbols.iter().find(|s| s.id == id).cloned();
        if let Some(def) = def {
            let mut next = doc.new_id();
            for p in doc.pages.iter_mut() {
                for l in p.layers.iter_mut() {
                    for o in l.objects.iter_mut() {
                        o.walk_mut(&mut |x| {
                            if x.kind == ObjKind::Symbol(id) {
                                let mut kids = def.objects.clone();
                                for k in kids.iter_mut() {
                                    k.walk_mut(&mut |y| {
                                        y.id = next;
                                        next += 1;
                                    });
                                }
                                x.kind = ObjKind::Group(kids);
                            }
                        });
                    }
                }
            }
            doc.ensure_next_id();
        }
        doc.symbols.retain(|s| s.id != id);
    });
}

/// Insert an instance of a symbol at the center of the page; a library's symbol is added to the
/// document first, linked to the library.
fn insert_symbol(app: &mut AnyVectorApp, s: &SymbolDef, library: Option<&std::path::Path>) {
    let Some(d) = app.doc_mut() else { return };
    let link = library.map(|p| p.to_string_lossy().to_string());
    let c = d.page_ref().rect().center();
    let (page, layer) = d.active_loc();
    d.modify("Insert Symbol", |doc, sel| {
        let id = match &link {
            None => s.id,
            Some(l) => match doc.symbols.iter().find(|x| x.name == s.name && x.link.as_deref() == Some(l.as_str())) {
                Some(x) => x.id,
                None => crate::dialogs::object::add_linked_symbol(doc, s, l),
            },
        };
        let bb = doc.symbol_bounds(id).unwrap_or_default();
        let oid = doc.new_id();
        let o = Object::new(oid, ObjKind::Symbol(id), kurbo::Affine::translate(c - bb.center()), Default::default());
        let l = layer.min(doc.pages[page].layers.len().saturating_sub(1));
        doc.pages[page].layers[l].objects.push(o);
        sel.clear();
        sel.push(oid);
    });
    d.cache.key = 0;
}

pub fn symbols(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let full = ui.available_width();
    let Some(d) = app.doc() else { return };
    let title = d.stem();
    let doc_syms = d.doc.symbols.clone();
    // the symbols of the library shown
    let lib = app.dockers.ui.sym_lib;
    let file = app.dockers.ui.sym_file.clone();
    if lib == 4 {
        if let Some(f) = &file {
            if app.dockers.ui.sym_cache.as_ref().is_none_or(|c| c.0 != *f) {
                let v = crate::dialogs::object::read_library(&f.to_string_lossy()).unwrap_or_default();
                app.dockers.ui.sym_cache = Some((f.clone(), v));
            }
        }
    }
    let shown: Vec<SymbolDef> = match lib {
        0 => doc_syms.iter().filter(|s| s.link.is_none()).cloned().collect(),
        1 => doc_syms.iter().filter(|s| s.link.is_some()).cloned().collect(),
        4 => app.dockers.ui.sym_cache.as_ref().map(|c| c.1.clone()).unwrap_or_default(),
        _ => vec![],
    };
    let sel = app.dockers.ui.sym_sel.clone().and_then(|n| shown.iter().find(|s| s.name == n).cloned());
    // the selected symbol
    let mut menu_act = None;
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let (pr, _) = ui.allocate_exact_size(vec2(100.0, 70.0), Sense::hover());
        ui.painter().rect_stroke(pr, 0.0, egui::Stroke::new(1.0, theme::SEP()), egui::StrokeKind::Inside);
        if let Some(s) = &sel {
            if let Some(t) = thumb(app, ui.ctx(), &format!("sym:{}:{}:{}", lib, s.id, s.name), &s.objects) {
                let sz = t.size_vec2();
                let k = (96.0 / sz.x).min(66.0 / sz.y);
                ui.painter().image(t.id(), Rect::from_center_size(pr.center(), sz * k), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        }
        ui.vertical(|ui| {
            ui.set_width(full - 140.0);
            if let Some(s) = &sel {
                ui.label(egui::RichText::new(&s.name).strong().size(14.0));
                let n = if lib <= 1 { app.doc().map(|d| count_instances(&d.doc, s.id)).unwrap_or(0) } else { 0 };
                ui.label(format!("{n} instances"));
                ui.label(if s.link.is_some() || lib == 4 { "Linked" } else { "Local" });
                ui.label(if nested(s) { "Nested" } else { "Unnested" });
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            let gear = icon_button_resp(app, ui, "gear", "Options", true);
            egui::Popup::menu(&gear).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.set_min_width(170.0);
                if ui.add_enabled(lib == 1 || lib == 2, egui::Button::new("Import Library")).clicked() {
                    menu_act = Some(0);
                    ui.close();
                }
                if ui.add_enabled(!doc_syms.is_empty(), egui::Button::new("Export Library")).clicked() {
                    menu_act = Some(1);
                    ui.close();
                }
                ui.checkbox(&mut app.dockers.ui.sym_world, "Scale to world units");
                if ui.add_enabled(!doc_syms.is_empty(), egui::Button::new("Delete unused symbols")).clicked() {
                    menu_act = Some(2);
                    ui.close();
                }
            });
        });
    });
    ui.add_space(4.0);
    // the libraries
    let locals: Vec<std::path::PathBuf> = {
        let mut v: Vec<_> = std::fs::read_dir(crate::dialogs::object::symbols_dir()).map(|r| r.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "csl")).collect()).unwrap_or_default();
        v.sort();
        v
    };
    egui::Frame::new().stroke(egui::Stroke::new(1.0, theme::SEP())).show(ui, |ui| {
        ui.set_width(full - 2.0);
        egui::ScrollArea::vertical().id_salt("sym_libs").max_height(96.0).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let fill = |on: bool| on.then_some(theme::ACCENT());
            let (r, _) = list_row(app, ui, 0, Some(None), "symbols", title.as_str(), fill(lib == 0));
            if r.clicked() {
                app.dockers.ui.sym_lib = 0;
            }
            let (r, _) = list_row(app, ui, 0, Some(None), "links", "Linked Symbols", fill(lib == 1));
            if r.clicked() {
                app.dockers.ui.sym_lib = 1;
            }
            let open = app.dockers.ui.sym_local_open;
            let (r, chev) = list_row(app, ui, 0, Some(if locals.is_empty() { None } else { Some(open) }), "open", "Local Symbols", fill(lib == 2));
            if chev {
                app.dockers.ui.sym_local_open = !open;
            } else if r.clicked() {
                app.dockers.ui.sym_lib = 2;
            }
            if open {
                for f in &locals {
                    let name = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    let (r, _) = list_row(app, ui, 1, Some(None), "symbols", name.as_str(), fill(lib == 4 && file.as_ref() == Some(f)));
                    if r.clicked() {
                        app.dockers.ui.sym_lib = 4;
                        app.dockers.ui.sym_file = Some(f.clone());
                    }
                }
            }
            let (r, _) = list_row(app, ui, 0, Some(None), "open", "Network Symbols", fill(lib == 3));
            if r.clicked() {
                app.dockers.ui.sym_lib = 3;
            }
        });
    });
    ui.add_space(4.0);
    ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.sym_search).hint_text("Search").desired_width(f32::INFINITY));
    ui.add_space(4.0);
    let full = ui.available_width();
    table_header(ui, &[("", 46.0), ("Name", (full - 46.0) * 0.6 + 46.0), ("Description", full)]);
    let q = app.dockers.ui.sym_search.trim().to_lowercase();
    let mut insert = None;
    let mut delete = None;
    for s in shown.iter().filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q)) {
        if let Some((_, name)) = app.dockers.ui.sym_rename.as_mut().filter(|(id, _)| *id == s.id && lib == 0) {
            let r = ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
            if r.lost_focus() {
                let name = name.trim().to_string();
                app.dockers.ui.sym_rename = None;
                if !name.is_empty() {
                    let id = s.id;
                    if let Some(d) = app.doc_mut() {
                        d.modify("Rename Symbol", |doc, _| {
                            if let Some(x) = doc.symbols.iter_mut().find(|x| x.id == id) {
                                x.name = name.clone();
                            }
                        });
                    }
                    app.dockers.ui.sym_sel = Some(name);
                }
            } else {
                r.request_focus();
            }
            continue;
        }
        let on = app.dockers.ui.sym_sel.as_deref() == Some(s.name.as_str());
        let (r, resp) = ui.allocate_exact_size(vec2(full, 32.0), Sense::click());
        if on {
            ui.painter().rect_filled(r, 0.0, theme::ACCENT());
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 0.0, theme::HOVER());
        }
        let icon = if lib == 0 { "unlock" } else { "links" };
        if let Some(t) = app.icons.get(ui.ctx(), icon, 12.0, crate::icons::Tone::Normal) {
            ui.painter().image(t.id, Rect::from_center_size(pos2(r.left() + 10.0, r.center().y + 4.0), vec2(12.0, 12.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let tr = Rect::from_min_size(pos2(r.left() + 18.0, r.top() + 5.0), vec2(28.0, 22.0));
        if let Some(t) = thumb(app, ui.ctx(), &format!("sym:{}:{}:{}", lib, s.id, s.name), &s.objects) {
            let sz = t.size_vec2();
            let k = (28.0 / sz.x).min(22.0 / sz.y);
            ui.painter().image(t.id(), Rect::from_center_size(tr.center(), sz * k), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        ui.painter().text(pos2(r.left() + 52.0, r.center().y), egui::Align2::LEFT_CENTER, &s.name, egui::FontId::proportional(13.0), theme::TEXT());
        if resp.clicked() {
            app.dockers.ui.sym_sel = Some(s.name.clone());
        }
        if resp.double_clicked() {
            insert = Some(s.clone());
        }
        resp.context_menu(|ui| {
            if ui.button("Insert Symbol").clicked() {
                insert = Some(s.clone());
                ui.close();
            }
            if lib == 0 {
                if ui.button("Rename").clicked() {
                    app.dockers.ui.sym_rename = Some((s.id, s.name.clone()));
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    delete = Some(s.id);
                    ui.close();
                }
            }
        });
    }
    if let Some(s) = insert {
        let libf = if lib == 4 { file.clone() } else { None };
        insert_symbol(app, &s, libf.as_deref());
    }
    if let Some(id) = delete {
        delete_symbol(app, id);
    }
    match menu_act {
        Some(0) => crate::dialogs::object::import_library(app),
        Some(1) => crate::dialogs::object::export_library(app),
        Some(2) => {
            let unused: Vec<u64> = app.doc().map(|d| d.doc.symbols.iter().filter(|s| count_instances(&d.doc, s.id) == 0).map(|s| s.id).collect()).unwrap_or_default();
            if let Some(d) = app.doc_mut() {
                d.modify("Delete Unused Symbols", |doc, _| doc.symbols.retain(|s| !unused.contains(&s.id)));
            }
        }
        _ => {}
    }
}

fn count_instances(doc: &vector_core::Document, id: u64) -> usize {
    let mut n = 0;
    for p in &doc.pages {
        for o in p.all_objects() {
            o.walk(&mut |x| {
                if x.kind == ObjKind::Symbol(id) {
                    n += 1;
                }
            });
        }
    }
    n
}

fn thumb(app: &mut AnyVectorApp, ctx: &egui::Context, key: &str, objs: &[Object]) -> Option<egui::TextureHandle> {
    if let Some(t) = app.ts.texture_cache.get(key) {
        return Some(t.clone());
    }
    let doc = &app.doc()?.doc;
    let pm = vector_render::render_objects_image(doc, objs, 72, None)?;
    let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
    let t = ctx.load_texture(key, img, egui::TextureOptions::LINEAR);
    app.ts.texture_cache.insert(key.to_string(), t.clone());
    Some(t)
}

/// The Export docker: the items to export (pages first, then objects), or three hints when there
/// are none; at the bottom Add (Selection, Current Page, All Pages), the default file format for
/// new items, Select all, Delete, the destination folder and Export.
pub fn export(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    use super::EXPORT_DOCKER_FORMATS as F;
    let doc_pages: Vec<u64> = app.doc().map(|d| d.doc.pages.iter().map(|p| p.id).collect()).unwrap_or_default();
    // items whose page or object no longer exists are removed
    let alive: Vec<bool> = app
        .dockers
        .ui
        .export_items
        .iter()
        .map(|it| match (it.page, it.object) {
            (Some(p), _) => doc_pages.contains(&p),
            (_, Some(o)) => app.doc().map(|d| d.doc.find(o).is_some()).unwrap_or(false),
            _ => false,
        })
        .collect();
    let mut k = 0;
    app.dockers.ui.export_items.retain(|_| {
        k += 1;
        alive[k - 1]
    });
    let mut run: Vec<usize> = vec![];
    let mut delete: Vec<usize> = vec![];
    let mut dup: Vec<usize> = vec![];
    let mut like: Option<usize> = None;
    let has_sel0 = app.has_selection();
    let list_h = (ui.clip_rect().bottom() - ui.cursor().top() - 44.0).max(60.0);
    egui::ScrollArea::vertical().id_salt("dk_exp_items").max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
        if app.dockers.ui.export_items.is_empty() {
            ui.add_space(14.0);
            for (text, icon) in [
                ("In the document, select objects and click Add > Selection.", "ex_add"),
                ("Or click Add > Current Page to add the current page.", "ex_page"),
                ("Or click Add > All Pages to add all pages.", "ex_pages"),
            ] {
                ui.vertical_centered(|ui| {
                    ui.set_max_width(ui.available_width() - 24.0);
                    ui.label(egui::RichText::new(text).italics().color(crate::theme::gray(205)));
                    ui.add_space(4.0);
                    if let Some(t) = app.icons.get(ui.ctx(), icon, 20.0, crate::icons::Tone::Normal) {
                        ui.add(egui::Image::new(t).fit_to_exact_size(vec2(20.0, 20.0)));
                    }
                });
                ui.add_space(18.0);
            }
            return;
        }
        let n = app.dockers.ui.export_items.len();
        for i in 0..n {
            let it = app.dockers.ui.export_items[i].clone();
            let tex = export_thumb(app, ui.ctx(), &it);
            let it = &mut app.dockers.ui.export_items[i];
            egui::Frame::new().stroke(egui::Stroke::new(1.0, crate::theme::SEP())).inner_margin(6.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.checkbox(&mut it.selected, "").on_hover_text("Select item");
                    if let Some(t) = &tex {
                        let s = t.size_vec2();
                        let f = 40.0 / s.x.max(s.y).max(1.0);
                        ui.add(egui::Image::new(t).fit_to_exact_size(s * f));
                    }
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut it.name).desired_width(ui.available_width() - 70.0));
                            ui.add(egui::TextEdit::singleline(&mut it.suffix).hint_text("Suffix").desired_width(60.0));
                        });
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(("dk_exp_fmt", i)).selected_text(F[it.format.min(F.len() - 1)]).width(64.0).show_ui(ui, |ui| {
                                for (k, f) in F.iter().enumerate() {
                                    ui.selectable_value(&mut it.format, k, *f);
                                }
                            });
                            if !matches!(F[it.format.min(F.len() - 1)], "PDF" | "SVG") {
                                ui.add(egui::DragValue::new(&mut it.dpi).range(36.0..=1200.0).suffix(" dpi"));
                            }
                        });
                    });
                });
                ui.horizontal(|ui| {
                    let folder = it.folder.display().to_string();
                    if crate::widgets::icon_button(&mut app.icons, ui, "open", "Destination folder", true, false).clicked() {
                        if let Some(d) = rfd::FileDialog::new().set_directory(&it.folder).pick_folder() {
                            it.folder = d;
                        }
                    }
                    ui.add(egui::Label::new(egui::RichText::new(&folder).small()).truncate()).on_hover_text(&folder);
                });
                let opts = ui.add_sized(vec2(ui.available_width(), 20.0), egui::Button::new("Options"));
                egui::Popup::menu(&opts).show(|ui| {
                    crate::menus::menu_style(ui);
                    if ui.button("Export Item").clicked() {
                        run.push(i);
                    }
                    if ui.button("Duplicate").clicked() {
                        dup.push(i);
                    }
                    if ui.add_enabled(has_sel0, egui::Button::new("Add Asset with These Settings")).clicked() {
                        like = Some(i);
                    }
                    if ui.button("Delete").clicked() {
                        delete.push(i);
                    }
                });
            });
            ui.add_space(2.0);
        }
    });
    // the bottom bar
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, egui::Stroke::new(1.0, crate::theme::SEP()));
    let has_sel = app.has_selection();
    let any = app.dockers.ui.export_items.iter().any(|i| i.selected);
    let all = !app.dockers.ui.export_items.is_empty() && app.dockers.ui.export_items.iter().all(|i| i.selected);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let add = super::icon_button_resp(app, ui, "ex_add", "Add", true);
        super::drop_mark(ui, add.rect);
        egui::Popup::menu(&add).show(|ui| {
            crate::menus::menu_style(ui);
            if crate::menus::item(app, ui, "&Selection", has_sel, false) {
                app.run(Cmd::ExportDockerAddSelection);
            }
            if crate::menus::item(app, ui, "&Current Page", true, false) {
                app.run(Cmd::ExportDockerAddPage);
            }
            if crate::menus::item(app, ui, "&All Pages", true, false) {
                app.run(Cmd::ExportDockerAddAllPages);
            }
        });
        let fmt = &mut app.dockers.ui.export_format;
        egui::ComboBox::from_id_salt("dk_exp").selected_text(F[(*fmt).min(F.len() - 1)]).width(62.0).show_ui(ui, |ui| {
            for (i, f) in F.iter().enumerate() {
                ui.selectable_value(fmt, i, *f);
            }
        })
        .response
        .on_hover_text("Choose a default file format for new items.");
        let items_on = !app.dockers.ui.export_items.is_empty();
        if crate::widgets::icon_button(&mut app.icons, ui, "ex_selall", "Select all", items_on, all).clicked() {
            app.dockers.ui.export_items.iter_mut().for_each(|i| i.selected = !all);
        }
        if super::icon_button(app, ui, "delete", "Delete", any) {
            delete.extend(app.dockers.ui.export_items.iter().enumerate().filter(|(_, i)| i.selected).map(|(k, _)| k));
        }
        if super::icon_button(app, ui, "open", "Destination folder", any) {
            let start = app.dockers.ui.export_items.iter().find(|i| i.selected).map(|i| i.folder.clone()).unwrap_or_else(crate::settings::documents_dir);
            if let Some(d) = rfd::FileDialog::new().set_directory(&start).pick_folder() {
                for it in app.dockers.ui.export_items.iter_mut().filter(|i| i.selected) {
                    it.folder = d.clone();
                }
            }
        }
        let w = ui.available_width();
        if ui.add_enabled_ui(any, |ui| ui.add_sized(vec2(w, 24.0), egui::Button::new("Export"))).inner.clicked() {
            run.extend(app.dockers.ui.export_items.iter().enumerate().filter(|(_, i)| i.selected).map(|(k, _)| k));
        }
    });
    if let Some(i) = like {
        let base = app.dockers.ui.export_items[i].clone();
        let n0 = app.dockers.ui.export_items.len();
        app.run(Cmd::ExportDockerAddSelection);
        for it in app.dockers.ui.export_items.iter_mut().skip(n0) {
            it.format = base.format;
            it.dpi = base.dpi;
            it.folder = base.folder.clone();
            it.suffix = base.suffix.clone();
        }
    }
    for i in dup.into_iter().rev() {
        let mut c = app.dockers.ui.export_items[i].clone();
        c.selected = false;
        app.dockers.ui.export_items.insert(i + 1, c);
    }
    let mut done = 0;
    run.sort_unstable();
    run.dedup();
    for i in run {
        let Some(it) = app.dockers.ui.export_items.get(i).cloned() else { continue };
        match crate::exec::export_item(app, &it) {
            Ok(_) => done += 1,
            Err(e) => {
                app.dialogs.message_box = Some(("Export".into(), format!("Could not export {}:\n{e}", it.name)));
                break;
            }
        }
    }
    if done > 0 {
        app.flash(format!("Exported {done} item(s)."));
    }
    delete.sort_unstable();
    delete.dedup();
    for i in delete.into_iter().rev() {
        if i < app.dockers.ui.export_items.len() {
            app.dockers.ui.export_items.remove(i);
        }
    }
}

/// The thumbnail of an Export docker item: its page or its object.
fn export_thumb(app: &mut AnyVectorApp, ctx: &egui::Context, it: &super::ExportItem) -> Option<egui::TextureHandle> {
    let d = app.doc()?;
    let rev = d.history.revision;
    let key = format!("exp:{:?}:{:?}:{rev}", it.page, it.object);
    if let Some(t) = app.ts.texture_cache.get(&key) {
        return Some(t.clone());
    }
    let pm = match (it.page, it.object) {
        (Some(pid), _) => {
            let i = d.doc.pages.iter().position(|p| p.id == pid)?;
            let p = &d.doc.pages[i];
            let dpi = 48.0 * 72.0 / p.width.max(p.height).max(1.0);
            vector_render::render_page_image(&d.doc, i, dpi, Some(vector_core::Color::WHITE), None, true)?
        }
        (_, Some(oid)) => {
            let o = d.doc.find(oid)?.clone();
            vector_render::render_objects_image(&d.doc, &[o], 48, None)?
        }
        _ => return None,
    };
    let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
    let t = ctx.load_texture(&key, img, egui::TextureOptions::LINEAR);
    app.ts.texture_cache.insert(key, t.clone());
    Some(t)
}

// ------------------------------------------------------------------ Tray

/// An item of a tray: objects, with the images and symbols they use.
#[derive(Clone)]
pub struct TrayItem {
    pub id: u64,
    pub name: String,
    pub doc: vector_core::Document,
}

/// A tray of the Tray docker.
#[derive(Clone)]
pub struct TrayDef {
    pub name: String,
    pub items: Vec<TrayItem>,
}

impl TrayDef {
    pub fn new(name: &str) -> TrayDef {
        TrayDef { name: name.to_string(), items: vec![] }
    }
}

/// A tray item holding `objs` of `src`.
fn tray_item(id: u64, name: String, src: &vector_core::Document, objs: Vec<Object>) -> TrayItem {
    let mut doc = vector_core::Document::new(1.0, 1.0);
    for o in &objs {
        o.walk(&mut |x| {
            if let ObjKind::Image(i) = &x.kind {
                if let Some(d) = src.images.get(&i.image) {
                    doc.images.insert(i.image, d.clone());
                }
            }
        });
    }
    doc.symbols = src.symbols.clone();
    doc.pages[0].layers[0].objects = objs;
    TrayItem { id, name, doc }
}

fn tray_add_selection(app: &mut AnyVectorApp) {
    let id = app.dockers.ui.tray_next + 1;
    let Some(d) = app.doc() else { return };
    let objs: Vec<Object> = d.selection.iter().filter_map(|id| d.doc.find(*id).cloned()).collect();
    let name = match objs.as_slice() {
        [] => return,
        [o] if !o.name.is_empty() => o.name.clone(),
        [o] => o.kind.type_name().to_string(),
        v => format!("{} objects", v.len()),
    };
    let item = tray_item(id, name, &d.doc, objs);
    app.dockers.ui.tray_next = id;
    let t = app.dockers.ui.tray_sel;
    app.dockers.ui.trays[t].items.push(item);
}

/// Tray management ▸ Open tray file: drawings and images, each an item of the tray shown.
fn tray_open_files(app: &mut AnyVectorApp) {
    let fd = AnyVectorApp::open_filters(rfd::FileDialog::new().set_title("Open tray file"), true);
    let Some(files) = fd.pick_files() else { return };
    for p in files {
        let mut doc = vector_core::Document::new(612.0, 792.0);
        match vector_io::import(&mut doc, &p) {
            Ok(objs) => {
                if objs.is_empty() {
                    continue;
                }
                doc.pages[0].layers[0].objects = objs;
                app.dockers.ui.tray_next += 1;
                let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let t = app.dockers.ui.tray_sel;
                app.dockers.ui.trays[t].items.push(TrayItem { id: app.dockers.ui.tray_next, name, doc });
            }
            Err(e) => app.dialogs.message_box = Some(("Open tray file".into(), format!("Could not open {}:\n{e}", p.display()))),
        }
    }
}

/// Insert a tray item in the active document, centered on the page.
fn tray_insert(app: &mut AnyVectorApp, it: &TrayItem) {
    if !app.has_doc() {
        app.new_default_document();
    }
    let Some(d) = app.doc_mut() else { return };
    let mut tmp = it.doc.clone();
    tmp.symbols.retain(|s| !d.doc.symbols.iter().any(|x| x.id == s.id));
    let mut objs = vector_io::merge_document(&mut d.doc, tmp);
    if objs.is_empty() {
        return;
    }
    let bb = vector_core::object::union_bounds(objs.iter().map(|o| o.bounds()));
    let c = d.page_ref().rect().center();
    for o in objs.iter_mut() {
        o.apply_transform(kurbo::Affine::translate(c - bb.center()));
    }
    d.add_objects("Insert from Tray", objs);
}

fn tray_thumb(app: &mut AnyVectorApp, ctx: &egui::Context, it: &TrayItem) -> Option<egui::TextureHandle> {
    let key = format!("tray:{}", it.id);
    if let Some(t) = app.ts.texture_cache.get(&key) {
        return Some(t.clone());
    }
    let pm = vector_render::render_objects_image(&it.doc, &it.doc.pages[0].layers[0].objects, 128, None)?;
    let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
    let t = ctx.load_texture(&key, img, egui::TextureOptions::LINEAR);
    app.ts.texture_cache.insert(key, t.clone());
    Some(t)
}

pub fn tray(app: &mut AnyVectorApp, ui: &mut Ui) {
    let full_w = ui.available_width();
    if app.dockers.ui.trays.is_empty() {
        app.dockers.ui.trays.push(TrayDef::new("Tray 1"));
    }
    let n = app.dockers.ui.trays.len();
    app.dockers.ui.tray_sel = app.dockers.ui.tray_sel.min(n - 1);
    let sel = app.dockers.ui.tray_sel;
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let w = (full_w - 44.0).max(60.0);
        if let Some(name) = app.dockers.ui.tray_rename.as_mut() {
            let r = ui.add(egui::TextEdit::singleline(name).desired_width(w));
            if r.lost_focus() {
                let name = name.trim().to_string();
                if !name.is_empty() {
                    app.dockers.ui.trays[sel].name = name;
                }
                app.dockers.ui.tray_rename = None;
            } else {
                r.request_focus();
            }
        } else {
            let names: Vec<String> = app.dockers.ui.trays.iter().map(|t| t.name.clone()).collect();
            let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
            let mut cur = sel;
            choice(ui, "tray_pick", &mut cur, &refs, w);
            app.dockers.ui.tray_sel = cur;
        }
        let gear = icon_button_resp(app, ui, "gear", "Tray management", true);
        drop_mark(ui, gear.rect);
        egui::Popup::menu(&gear).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.set_min_width(130.0);
            if ui.button("Add tray").clicked() {
                let mut k = n + 1;
                while app.dockers.ui.trays.iter().any(|t| t.name == format!("Tray {k}")) {
                    k += 1;
                }
                app.dockers.ui.trays.push(TrayDef::new(&format!("Tray {k}")));
                app.dockers.ui.tray_sel = app.dockers.ui.trays.len() - 1;
                ui.close();
            }
            if ui.add_enabled(n > 1, egui::Button::new("Delete tray")).clicked() {
                app.dockers.ui.trays.remove(sel);
                app.dockers.ui.tray_sel = sel.saturating_sub(1);
                ui.close();
            }
            if ui.button("Rename tray").clicked() {
                app.dockers.ui.tray_rename = Some(app.dockers.ui.trays[sel].name.clone());
                ui.close();
            }
            if ui.button("Open tray file...").clicked() {
                ui.close();
                tray_open_files(app);
            }
        });
    });
    // the items: double-click (or right-click ▸ Insert) to insert one in the document
    let sel = app.dockers.ui.tray_sel.min(app.dockers.ui.trays.len() - 1);
    let items = app.dockers.ui.trays[sel].items.clone();
    let area = ui.available_rect_before_wrap();
    let bg = ui.interact(area, egui::Id::unique("tray_area"), Sense::click());
    let can_add = app.has_selection();
    bg.context_menu(|ui| {
        if ui.add_enabled(can_add, egui::Button::new("Add to tray")).clicked() {
            tray_add_selection(app);
            ui.close();
        }
    });
    let mut insert = None;
    let mut remove = None;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area.shrink(4.0)).layout(egui::Layout::left_to_right(egui::Align::TOP).with_main_wrap(true)));
    for (i, it) in items.iter().enumerate() {
        let (r, resp) = child.allocate_exact_size(vec2(72.0, 90.0), Sense::click());
        let resp = resp.on_hover_text(&it.name);
        if resp.hovered() {
            child.painter().rect_filled(r, 2.0, theme::HOVER());
        }
        let tile = Rect::from_min_size(r.min + vec2(4.0, 4.0), vec2(64.0, 64.0));
        child.painter().rect_filled(tile, 1.0, crate::theme::FAINT());
        if let Some(t) = tray_thumb(app, child.ctx(), it) {
            let sz = t.size_vec2();
            let s = (60.0 / sz.x).min(60.0 / sz.y);
            child.painter().image(t.id(), Rect::from_center_size(tile.center(), sz * s), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let g = egui::WidgetText::from(egui::RichText::new(&it.name).size(11.0).color(theme::TEXT_DIM())).into_galley(&child, Some(egui::TextWrapMode::Truncate), 68.0, egui::TextStyle::Body);
        child.painter().galley(pos2(r.center().x - g.size().x / 2.0, tile.bottom() + 4.0), g, theme::TEXT_DIM());
        if resp.double_clicked() {
            insert = Some(i);
        }
        resp.context_menu(|ui| {
            if ui.button("Insert").clicked() {
                insert = Some(i);
                ui.close();
            }
            if ui.button("Remove from tray").clicked() {
                remove = Some(i);
                ui.close();
            }
        });
    }
    if let Some(i) = insert {
        tray_insert(app, &items[i]);
    }
    if let Some(i) = remove {
        app.dockers.ui.trays[sel].items.remove(i);
    }
}

pub fn links(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let o = first(app);
    let has = o.is_some();
    let link = o.as_ref().map(|o| o.link.clone()).unwrap_or_default();
    let url = o.as_ref().and_then(|o| o.url.clone()).unwrap_or_default();
    let bookmark = link.bookmark.is_some();
    let w = ui.available_width() - 6.0;
    let lbl = |ui: &mut Ui, t: &str, on: bool| {
        ui.add_enabled(on, egui::Label::new(t));
        ui.add_space(2.0);
    };
    ui.add_space(2.0);
    // Behavior: a Web link or a bookmark
    lbl(ui, "Behavior:", true);
    let mut b = bookmark as usize;
    ui.add_enabled_ui(has, |ui| {
        if super::choice(ui, "lr_behavior", &mut b, &["Link", "Bookmark"], w) {
            let to_bm = b == 1;
            edit_sel(app, "Behavior", |x| {
                if to_bm {
                    x.link.bookmark = Some(x.link.bookmark.clone().unwrap_or_default());
                    x.url = None;
                } else {
                    x.link.bookmark = None;
                }
            });
        }
    });
    ui.add_space(4.0);
    if bookmark {
        lbl(ui, "Bookmark:", true);
        let mut name = link.bookmark.clone().unwrap_or_default();
        if ui.add_enabled(has, egui::TextEdit::singleline(&mut name).desired_width(w)).on_hover_text("Add descriptive information about a bookmark or anchor.").changed() {
            edit_sel(app, "Bookmark", |x| x.link.bookmark = Some(name.clone()));
        }
    } else {
        lbl(ui, "URL:", true);
        // the URL, or one of the document's bookmarks
        let marks: Vec<String> = app.doc().map(|d| {
            let mut v = vec![];
            for p in &d.doc.pages {
                for x in p.all_objects() {
                    x.walk(&mut |y| {
                        if let Some(b) = &y.link.bookmark {
                            if !b.is_empty() {
                                v.push(format!("#{b}"));
                            }
                        }
                    });
                }
            }
            v
        }).unwrap_or_default();
        if app.dockers.ui.link_url.is_empty() || !has {
            app.dockers.ui.link_url = url.clone();
        }
        ui.add_enabled_ui(has, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let r = ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.link_url).desired_width(w - 20.0)).on_hover_text("Assign a hyperlink to the Web object or bookmark.");
                if r.lost_focus() && app.dockers.ui.link_url != url {
                    let u = app.dockers.ui.link_url.trim().to_string();
                    edit_sel(app, "Hyperlink", |x| x.url = (!u.is_empty()).then(|| u.clone()));
                }
                let (ar, aresp) = ui.allocate_exact_size(vec2(20.0, 22.0), Sense::click());
                crate::widgets::chevron(ui.painter(), ar.center(), 3.0, crate::theme::TEXT());
                egui::Popup::menu(&aresp).show(|ui| {
                    crate::menus::menu_style(ui);
                    if marks.is_empty() {
                        ui.add_enabled(false, egui::Button::new("(no bookmarks)"));
                    }
                    for m in &marks {
                        if ui.button(m).clicked() {
                            let u = m.clone();
                            app.dockers.ui.link_url = u.clone();
                            edit_sel(app, "Hyperlink", |x| x.url = Some(u.clone()));
                        }
                    }
                });
            });
        });
        ui.add_space(4.0);
        let on = has && !url.is_empty();
        lbl(ui, "Target:", on);
        let targets = ["_blank", "_self", "_top", "_parent"];
        let mut t = targets.iter().position(|x| *x == link.target).unwrap_or(0);
        ui.add_enabled_ui(on, |ui| {
            if !on {
                super::choice(ui, "lr_target_off", &mut 0, &[""], w);
            } else if super::choice(ui, "lr_target", &mut t, &targets, w) {
                let v = targets[t].to_string();
                edit_sel(app, "Target Frame", |x| x.link.target = v.clone());
            }
        });
        let mut alt = link.alt.clone();
        if ui.add_enabled(on, egui::TextEdit::singleline(&mut alt).desired_width(w)).changed() {
            edit_sel(app, "Alt Comments", |x| x.link.alt = alt.clone());
        }
        ui.add_space(8.0);
        lbl(ui, "Define hotspot using:", on);
        ui.indent("lr_hot", |ui| {
            for (k, t) in [(true, "Shape"), (false, "Bounds")] {
                if super::radio(ui, link.hotspot_shape == k, t, on) {
                    edit_sel(app, "Hotspot", |x| x.link.hotspot_shape = k);
                }
                ui.add_space(3.0);
            }
        });
        egui::Grid::new("lr_hot_colors").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
            ui.label("Cross-hatch:");
            let mut c = Some(app.dockers.ui.hotspot_colors.0);
            if super::color_pick_w(ui, "lr_hatch", &mut c, false, true, 46.0) {
                if let Some(c) = c {
                    app.dockers.ui.hotspot_colors.0 = c;
                }
            }
            ui.end_row();
            ui.label("Background:");
            let mut c = app.dockers.ui.hotspot_colors.1;
            if super::color_pick_w(ui, "lr_hot_bg", &mut c, true, true, 46.0) {
                app.dockers.ui.hotspot_colors.1 = c;
            }
            ui.end_row();
        });
    }
    ui.add_space(10.0);
    separator(ui);
    ui.add_space(10.0);
    // Rollover
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Rollover").font(crate::theme::bold(12.5)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (live, has_doc) = (app.rollover_live, app.has_doc());
            if crate::widgets::icon_button(&mut app.icons, ui, "rollover_preview", "Rollover live preview", has_doc, live).clicked() {
                app.run(Cmd::RolloverLivePreview);
            }
        });
    });
    ui.add_space(4.0);
    let en = |app: &AnyVectorApp, id: &str| crate::ui::bind::enabled(app, id).unwrap_or(true);
    ui.horizontal(|ui| {
        for (icon, tip, id, c) in [
            ("rollover_create", "Create rollover", "91dbd3be", Cmd::RolloverCreate),
            ("rollover_edit", "Edit rollover", "54cac40e", Cmd::RolloverEdit),
            ("rollover_extract", "Extract rollover objects", "9af6ee00", Cmd::RolloverExtract),
            ("rollover_finish", "Finish editing rollover", "7b203f56", Cmd::RolloverFinish),
        ] {
            let on = en(app, id);
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, on, false).clicked() && on {
                app.run(c);
            }
        }
    });
    lbl(ui, "Active rollover state:", true);
    let state = crate::dialogs::object::active_state(app);
    ui.horizontal(|ui| {
        let mut s = state.unwrap_or(0) as usize;
        ui.add_enabled_ui(state.is_some(), |ui| {
            let cw = (w - 60.0).max(80.0);
            if state.is_none() {
                super::choice(ui, "lr_state_off", &mut 0, &[""], cw);
            } else if super::choice(ui, "lr_state", &mut s, &["[NORMAL]", "[OVER]", "[DOWN]"], cw) {
                crate::dialogs::object::show_state(app, s as u8);
            }
        });
        for (icon, tip, id, c) in [("duplicate", "Duplicate rollover state", "83501505", Cmd::RolloverDuplicateState), ("delete", "Deletes rollover state", "217155e9", Cmd::RolloverDeleteState)] {
            let on = en(app, id);
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, on, false).clicked() && on {
                app.run(c);
            }
        }
    });
    ui.horizontal(|ui| {
        let web = en(app, "e4f6d170");
        let web_on = crate::ui::bind::checked(app, "e4f6d170").unwrap_or(false);
        if crate::widgets::icon_button(&mut app.icons, ui, "web_text", "Make text Web-compatible", web, web_on).clicked() && web {
            app.run(Cmd::MakeWebCompatible);
        }
        let enc = en(app, "63992d01");
        if crate::widgets::icon_button(&mut app.icons, ui, "encode", "Encode...", enc, false).clicked() && enc {
            app.run(Cmd::EncodeText);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let on = app.settings.show_hotspots;
            if crate::widgets::icon_button(&mut app.icons, ui, "hotspots", "Show or hide hotspots on objects.", true, on).clicked() {
                app.run(Cmd::ShowHotspots);
            }
        });
    });
}

/// Show Hotspots: the Web links of the page, cross-hatched over their shape or bounds.
pub fn draw_hotspots(app: &AnyVectorApp, painter: &egui::Painter, d: &crate::docview::DocView) {
    if !app.settings.show_hotspots {
        return;
    }
    let (hatch, bg) = app.dockers.ui.hotspot_colors;
    let hc = egui::Color32::from_rgba_unmultiplied(hatch.r, hatch.g, hatch.b, 200);
    for (_, _, l) in d.doc.layers_on_page(d.page) {
        for o in &l.objects {
            o.walk(&mut |x| {
                if x.url.as_deref().is_none_or(str::is_empty) {
                    return;
                }
                let r = d.to_screen_rect(x.bounds());
                if let Some(b) = bg {
                    painter.rect_filled(r, 0.0, egui::Color32::from_rgba_unmultiplied(b.r, b.g, b.b, 90));
                }
                let p = painter.with_clip_rect(r.intersect(painter.clip_rect()));
                let step = 6.0;
                let mut t = -r.height();
                while t < r.width() {
                    p.line_segment([egui::pos2(r.left() + t, r.bottom()), egui::pos2(r.left() + t + r.height(), r.top())], egui::Stroke::new(1.0, hc));
                    t += step;
                }
            });
        }
    }
}

// ------------------------------------------------------------------ Sources

/// The page (1-based) where an image is used first.
fn image_page(doc: &vector_core::Document, image: u64) -> Option<usize> {
    for (pi, p) in doc.pages.iter().enumerate() {
        let mut found = false;
        for o in p.all_objects() {
            o.walk(&mut |x| {
                if matches!(&x.kind, ObjKind::Image(i) if i.image == image) {
                    found = true;
                }
            });
        }
        if found {
            return Some(pi + 1);
        }
    }
    None
}

/// Add linked file: import an image that stays linked to its file.
fn add_linked_file(app: &mut AnyVectorApp) {
    let exts = ["png", "jpg", "jpeg", "jfif", "bmp", "tif", "tiff", "webp", "gif", "ico", "tga"];
    let mut fd = rfd::FileDialog::new().set_title("Add linked file").add_filter("Bitmaps", &exts);
    if let Some(d) = app.settings.last_dir.clone() {
        fd = fd.set_directory(d);
    }
    let Some(p) = fd.pick_file() else { return };
    let before: Vec<u64> = app.doc().map(|d| d.doc.images.keys().copied().collect()).unwrap_or_default();
    app.import_path(&p, None);
    if let Some(d) = app.doc_mut() {
        let new: Vec<u64> = d.doc.images.keys().filter(|k| !before.contains(k)).copied().collect();
        for k in new {
            d.doc.links.insert(k, p.to_string_lossy().to_string());
        }
    }
}

/// Sync linked file: read the linked images again from their files.
pub(crate) fn sync_links(app: &mut AnyVectorApp, ids: &[u64]) {
    let Some(d) = app.doc() else { return };
    let mut fresh = vec![];
    let mut failed = vec![];
    for id in ids {
        let Some(path) = d.doc.links.get(id) else { continue };
        let name = d.doc.images.get(id).map(|i| i.name.clone()).unwrap_or_default();
        match std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| vector_io::image_io::decode(&b, &name).map_err(|e| e.to_string())) {
            Ok(img) => fresh.push((*id, img)),
            Err(e) => failed.push(format!("{path}: {e}")),
        }
    }
    if !fresh.is_empty() {
        if let Some(d) = app.doc_mut() {
            d.modify("Sync Linked File", |doc, _| {
                for (id, img) in fresh {
                    doc.images.insert(id, img);
                }
            });
            d.cache.key = 0;
        }
    }
    if !failed.is_empty() {
        app.dialogs.message_box = Some(("Sources".into(), format!("Could not read the linked file:\n{}", failed.join("\n"))));
    }
}

pub fn sources(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let Some(d) = app.doc() else { return };
    let links: Vec<(u64, String, Option<usize>)> = d.doc.links.iter().filter(|(id, _)| d.doc.images.contains_key(id)).map(|(id, p)| (*id, p.clone(), image_page(&d.doc, *id))).collect();
    let sel = app.dockers.ui.src_sel.filter(|s| links.iter().any(|l| l.0 == *s));
    let mut act = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if icon_button(app, ui, "open", "Open source file", sel.is_some()) {
            act = Some(0);
        }
        if icon_button(app, ui, "link_update", "Sync linked file", sel.is_some()) {
            act = Some(1);
        }
        if icon_button(app, ui, "link_break", "Break link", sel.is_some()) {
            act = Some(2);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(app, ui, "refresh", "Refresh", !links.is_empty()) {
                act = Some(3);
            }
        });
    });
    let page_w = 82.0;
    let name_w = ui.available_width() - page_w;
    table_header(ui, &[("Name", name_w), ("Page", page_w)]);
    let area = ui.available_rect_before_wrap();
    let bg = ui.interact(area, egui::Id::unique("src_area"), Sense::click());
    let mut add = false;
    bg.context_menu(|ui| {
        if ui.button("Add linked file...").clicked() {
            add = true;
            ui.close();
        }
    });
    for (id, path, page) in &links {
        let name = std::path::Path::new(path).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| path.clone());
        let fill = (sel == Some(*id)).then_some(theme::ACCENT());
        let (resp, _) = list_row(app, ui, 0, None, "bitmap", name, fill);
        if let Some(pg) = page {
            ui.painter().text(pos2(resp.rect.left() + name_w + page_w / 2.0, resp.rect.center().y), egui::Align2::CENTER_CENTER, pg.to_string(), egui::FontId::proportional(13.0), theme::TEXT());
        }
        let resp = resp.on_hover_text(path);
        if resp.clicked() {
            app.dockers.ui.src_sel = Some(*id);
        }
        resp.context_menu(|ui| {
            app.dockers.ui.src_sel = Some(*id);
            for (i, t) in ["Open source file", "Sync linked file", "Break link"].iter().enumerate() {
                if ui.button(*t).clicked() {
                    act = Some(i);
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Add linked file...").clicked() {
                add = true;
                ui.close();
            }
        });
    }
    let sel = app.dockers.ui.src_sel;
    match (act, sel) {
        (Some(0), Some(id)) => {
            if let Some(p) = links.iter().find(|l| l.0 == id) {
                let _ = std::process::Command::new("cmd").args(["/C", "start", "", &p.1]).spawn();
            }
        }
        (Some(1), Some(id)) => sync_links(app, &[id]),
        (Some(2), Some(id)) => {
            if let Some(d) = app.doc_mut() {
                d.modify("Break Link", |doc, _| {
                    doc.links.remove(&id);
                });
            }
        }
        (Some(3), _) => {
            let all: Vec<u64> = links.iter().map(|l| l.0).collect();
            sync_links(app, &all);
        }
        _ => {}
    }
    if add {
        add_linked_file(app);
    }
}

// ------------------------------------------------------------------ Views

pub fn views(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let has_sel = app.has_selection();
    let n_views = app.doc().map(|d| d.doc.views.len()).unwrap_or(0);
    let cur = app.dockers.ui.view_sel.filter(|i| *i < n_views);
    let mut add = false;
    let mut del = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if icon_button(app, ui, "zoom", "Zoom One-Shot", true) {
            app.run(Cmd::Tool(crate::tools::ToolId::Zoom));
        }
        let (sr, _) = ui.allocate_exact_size(vec2(9.0, 22.0), Sense::hover());
        ui.painter().vline(sr.center().x, sr.y_range().shrink(2.0), egui::Stroke::new(1.0, theme::SEP()));
        if icon_button(app, ui, "zoom_in", "Zoom In", true) {
            app.run(Cmd::ZoomIn);
        }
        if icon_button(app, ui, "zoom_out", "Zoom Out", true) {
            app.run(Cmd::ZoomOut);
        }
        if icon_button(app, ui, "zoom_selected", "Zoom to Selected", has_sel) {
            app.run(Cmd::ZoomToSelected);
        }
        if icon_button(app, ui, "zoom_all", "Zoom to All Objects", true) {
            app.run(Cmd::ZoomToAll);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let gear = icon_button_resp(app, ui, "gear", "Views", true);
            drop_mark(ui, gear.rect);
            egui::Popup::menu(&gear).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.set_min_width(150.0);
                if ui.button("Add current view").clicked() {
                    add = true;
                    ui.close();
                }
                if ui.add_enabled(cur.is_some(), egui::Button::new("Delete current view")).clicked() {
                    del = true;
                    ui.close();
                }
            });
        });
    });
    ui.add_space(2.0);
    let Some(d) = app.doc() else { return };
    let views = d.doc.views.clone();
    let mut go = None;
    for (i, v) in views.iter().enumerate() {
        if let Some((_, name)) = app.dockers.ui.view_rename.as_mut().filter(|(k, _)| *k == i) {
            let r = ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
            if r.lost_focus() {
                let name = name.trim().to_string();
                app.dockers.ui.view_rename = None;
                if let Some(d) = app.doc_mut() {
                    if !name.is_empty() {
                        d.doc.views[i].name = name;
                        d.history.touch();
                    }
                }
            } else {
                r.request_focus();
            }
            continue;
        }
        let fill = (cur == Some(i)).then_some(theme::ACCENT());
        let (resp, _) = list_row(app, ui, 0, None, "", v.name.as_str(), fill);
        let resp = resp.on_hover_text(format!("Page {}, {:.0}%", v.page + 1, v.zoom / (96.0 / 72.0) * 100.0));
        if resp.clicked() {
            app.dockers.ui.view_sel = Some(i);
            go = Some(v.clone());
        }
        if resp.double_clicked() {
            app.dockers.ui.view_rename = Some((i, v.name.clone()));
        }
    }
    let mut new_sel = app.dockers.ui.view_sel;
    if let Some(d) = app.doc_mut() {
        if let Some(v) = go {
            d.page = v.page.min(d.doc.pages.len() - 1);
            d.zoom = v.zoom;
            let cs = d.canvas_rect.size();
            d.origin = egui::vec2((cs.x as f64 / 2.0 - v.center.x * v.zoom) as f32, (cs.y as f64 / 2.0 - v.center.y * v.zoom) as f32);
            d.needs_fit = false;
        }
        if add {
            let center = d.to_doc(d.canvas_rect.center());
            let mut k = d.doc.views.len() + 1;
            while d.doc.views.iter().any(|v| v.name == format!("View-{k}")) {
                k += 1;
            }
            d.doc.views.push(SavedView { name: format!("View-{k}"), page: d.page, zoom: d.zoom, center });
            d.history.touch();
            new_sel = Some(d.doc.views.len() - 1);
        }
        if let (true, Some(i)) = (del, cur) {
            d.doc.views.remove(i);
            d.history.touch();
            new_sel = None;
        }
    }
    app.dockers.ui.view_sel = new_sel;
}

// ------------------------------------------------------------------ History

/// The icon of an undo step, from its name.
fn history_icon(label: &str) -> &'static str {
    let l = label.to_ascii_lowercase();
    const ICONS: &[(&str, &str)] = &[
        ("fill", "fill_icon"),
        ("outline", "outline_icon"),
        ("rectangle", "rectangle"),
        ("ellipse", "ellipse"),
        ("polygon", "polygon"),
        ("star", "star"),
        ("spiral", "spiral"),
        ("text", "text"),
        ("table", "table"),
        ("blend", "blend"),
        ("contour", "contour"),
        ("extrude", "extrude"),
        ("envelope", "envelope"),
        ("distort", "distort"),
        ("shadow", "drop_shadow"),
        ("transparency", "transparency"),
        ("bevel", "bevel"),
        ("lens", "lens"),
        ("delete", "delete"),
        ("group", "group"),
        ("combine", "combine_obj"),
        ("node", "shape"),
        ("page", "pages"),
        ("paste", "paste"),
        ("import", "import"),
        ("align", "align"),
        ("weld", "weld"),
        ("trim", "trim"),
        ("intersect", "intersect"),
        ("powerclip", "powerclip"),
        ("dimension", "parallel_dim"),
        ("connector", "connector"),
    ];
    ICONS.iter().find(|(k, _)| l.contains(k)).map(|(_, i)| *i).unwrap_or("pick")
}

pub fn history(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let Some(d) = app.doc() else { return };
    let undo = d.history.undo_labels();
    let redo = d.history.redo_labels();
    let first = if d.path.is_some() { ("open", "File Open") } else { ("new", "File New") };
    let current = Some(Color32::from_rgb(0x3a, 0x3a, 0x3a));
    let mut cmd = None;
    // oldest at the top; the current state is highlighted
    let n = undo.len();
    let (r, _) = list_row(app, ui, 0, None, first.0, first.1, if n == 0 { current } else { None });
    if r.clicked() && n > 0 {
        cmd = Some(Cmd::UndoTo(n));
    }
    for (i, l) in undo.iter().enumerate().rev() {
        let (r, _) = list_row(app, ui, 0, None, history_icon(l), l.as_str(), if i == 0 { current } else { None });
        if r.clicked() && i > 0 {
            cmd = Some(Cmd::UndoTo(i));
        }
    }
    for (i, l) in redo.iter().enumerate() {
        let (r, _) = list_row(app, ui, 0, None, history_icon(l), egui::RichText::new(l).color(theme::DISABLED()), None);
        if r.clicked() {
            cmd = Some(Cmd::RedoTo(i + 1));
        }
    }
    let _ = ui.allocate_response(ui.available_size(), Sense::hover()).on_hover_text("Displays documents UNDO and REDO history");
    if let Some(c) = cmd {
        app.run(c);
    }
}

// ------------------------------------------------------------------ Scripts

fn rhai_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir).map(|r| r.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "rhai")).collect()).unwrap_or_default();
    v.sort();
    v
}

fn run_script_file(app: &mut AnyVectorApp, p: &std::path::Path) {
    match std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|s| crate::scripting::run_script(app, &s)) {
        Ok(()) => app.dockers.ui.script_error = None,
        Err(e) => app.dockers.ui.script_error = Some(e),
    }
}

fn edit_script_file(app: &mut AnyVectorApp, p: &std::path::Path) {
    if let Ok(code) = std::fs::read_to_string(p) {
        app.dialogs.script_editor = Some(crate::dialogs::forms::ScriptEditorState { code, path: Some(p.to_path_buf()), ..Default::default() });
    }
}

/// New script: a script file in `dir`, opened in the Script Editor.
fn new_script(app: &mut AnyVectorApp, dir: &std::path::Path) {
    let mut k = 1;
    while dir.join(format!("Script{k}.rhai")).exists() {
        k += 1;
    }
    let p = dir.join(format!("Script{k}.rhai"));
    if std::fs::write(&p, crate::dialogs::forms::SCRIPT_TEMPLATE).is_ok() {
        app.dockers.ui.script_sel = Some(p.to_string_lossy().to_string());
        edit_script_file(app, &p);
    }
}

pub fn scripts(app: &mut AnyVectorApp, ui: &mut Ui) {
    let user = crate::scripting::scripts_dir();
    // the folder new scripts go to: the selected folder (or the selected script's)
    let target = app.dockers.ui.script_sel.as_ref().map(std::path::PathBuf::from).map(|p| if p.is_file() { p.parent().map(|x| x.to_path_buf()).unwrap_or(p) } else { p }).filter(|p| p.is_dir()).unwrap_or_else(|| user.clone());
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let nb = ui.add_sized(vec2(80.0, 24.0), egui::Button::new("New"));
        let c = pos2(nb.rect.right() - 12.0, nb.rect.center().y);
        ui.painter().add(egui::Shape::line(vec![c + vec2(-4.0, -2.0), c + vec2(0.0, 2.0), c + vec2(4.0, -2.0)], egui::Stroke::new(1.2, theme::TEXT())));
        egui::Popup::menu(&nb).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.set_min_width(120.0);
            if ui.button("New script").clicked() {
                ui.close();
                new_script(app, &target);
            }
        });
        ui.add_space(8.0);
        if ui.add_sized(vec2(104.0, 24.0), egui::Button::new("Load...")).clicked() {
            if let Some(dir) = rfd::FileDialog::new().set_title("Load").pick_folder() {
                if dir != user && !app.settings.script_folders.contains(&dir) {
                    app.settings.script_folders.push(dir.clone());
                    app.settings.save();
                }
                app.dockers.ui.script_sel = Some(dir.to_string_lossy().to_string());
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let on = app.dockers.ui.script_simple;
            if crate::widgets::icon_button(&mut app.icons, ui, "simple_mode", "Simple mode", true, on).clicked() {
                app.dockers.ui.script_simple = !on;
            }
        });
    });
    ui.add_space(2.0);
    // the tree: the script engine, its folders and their scripts
    let mut folders: Vec<(std::path::PathBuf, String, bool)> = vec![(user.clone(), "User Scripts".into(), false)];
    for f in &app.settings.script_folders {
        folders.push((f.clone(), f.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| f.to_string_lossy().to_string()), true));
    }
    let simple = app.dockers.ui.script_simple;
    let sel_fill = |key: &str, app: &AnyVectorApp| (app.dockers.ui.script_sel.as_deref() == Some(key)).then_some(theme::ACCENT());
    let mut run = None;
    let mut edit = None;
    let mut delete = None;
    let mut unload = None;
    let mut new_in = None;
    let open = !app.dockers.ui.script_closed.iter().any(|k| k == "rhai");
    let (r, chev) = list_row(app, ui, 0, Some(Some(open)), "macro_manager", "Rhai", sel_fill("rhai", app));
    if chev {
        toggle_fold(app, "rhai");
    } else if r.clicked() {
        app.dockers.ui.script_sel = Some("rhai".into());
    }
    if open {
        for (dir, name, loaded) in &folders {
            let key = dir.to_string_lossy().to_string();
            let files = rhai_files(dir);
            let mut depth = 1;
            if !simple {
                let fopen = !app.dockers.ui.script_closed.contains(&key);
                let fold = if files.is_empty() { None } else { Some(fopen) };
                let (r, chev) = list_row(app, ui, 1, Some(fold), "open", name.as_str(), sel_fill(&key, app));
                if chev {
                    toggle_fold(app, &key);
                } else if r.clicked() {
                    app.dockers.ui.script_sel = Some(key.clone());
                }
                r.on_hover_text(dir.display().to_string()).context_menu(|ui| {
                    if ui.button("New script").clicked() {
                        new_in = Some(dir.clone());
                        ui.close();
                    }
                    if *loaded && ui.button("Unload").clicked() {
                        unload = Some(dir.clone());
                        ui.close();
                    }
                });
                if !fopen {
                    continue;
                }
                depth = 2;
            }
            for f in files {
                let fkey = f.to_string_lossy().to_string();
                let stem = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if let Some((_, name)) = app.dockers.ui.script_rename.as_mut().filter(|(p, _)| *p == f) {
                    let r = ui.horizontal(|ui| {
                        ui.add_space(6.0 + depth as f32 * 19.0 + 36.0);
                        ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY))
                    });
                    if r.inner.lost_focus() {
                        let name = name.trim().to_string();
                        app.dockers.ui.script_rename = None;
                        if !name.is_empty() && name != stem {
                            let to = f.with_file_name(format!("{name}.rhai"));
                            if to.exists() || std::fs::rename(&f, &to).is_err() {
                                app.dockers.ui.script_error = Some(format!("Could not rename {stem} to {name}"));
                            } else {
                                app.dockers.ui.script_sel = Some(to.to_string_lossy().to_string());
                            }
                        }
                    } else {
                        r.inner.request_focus();
                    }
                    continue;
                }
                let (r, _) = list_row(app, ui, depth, Some(None), "script", stem.as_str(), sel_fill(&fkey, app));
                if r.clicked() {
                    app.dockers.ui.script_sel = Some(fkey.clone());
                }
                if r.double_clicked() {
                    run = Some(f.clone());
                }
                r.context_menu(|ui| {
                    if ui.button("Run").clicked() {
                        run = Some(f.clone());
                        ui.close();
                    }
                    if ui.button("Edit").clicked() {
                        edit = Some(f.clone());
                        ui.close();
                    }
                    if ui.button("Rename").clicked() {
                        app.dockers.ui.script_rename = Some((f.clone(), stem.clone()));
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
                        delete = Some(f.clone());
                        ui.close();
                    }
                });
            }
        }
    }
    if let Some(e) = &app.dockers.ui.script_error {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(e).color(egui::Color32::from_rgb(0xe8, 0x6a, 0x6a)));
    }
    if let Some(p) = run {
        run_script_file(app, &p);
    }
    if let Some(p) = edit {
        edit_script_file(app, &p);
    }
    if let Some(p) = delete {
        if let Err(e) = std::fs::remove_file(&p) {
            app.dockers.ui.script_error = Some(format!("Could not delete {}: {e}", p.display()));
        }
    }
    if let Some(d) = new_in {
        new_script(app, &d);
    }
    if let Some(d) = unload {
        app.settings.script_folders.retain(|x| *x != d);
        app.settings.save();
    }
}

fn toggle_fold(app: &mut AnyVectorApp, key: &str) {
    let v = &mut app.dockers.ui.script_closed;
    if let Some(i) = v.iter().position(|k| k == key) {
        v.remove(i);
    } else {
        v.push(key.to_string());
    }
}

pub fn learn(app: &mut AnyVectorApp, ui: &mut Ui) {
    heading(ui, "Getting started");
    let tips = [
        ("Draw shapes", "Use the Rectangle (F6), Ellipse (F7) and Polygon (Y) tools. Hold Ctrl to constrain and Shift to draw from the center."),
        ("Edit curves", "Select a curve and press F10 for the Shape tool. Double-click a curve to add a node; drag nodes and control handles."),
        ("Color", "Click a palette swatch to fill, right-click to set the outline. Drag colors onto objects."),
        ("Combine shapes", "Select two or more objects and use Weld, Trim or Intersect from the property bar or the Shape docker."),
        ("Text", "Press F8 for the Text tool. Click to type artistic text, or drag a frame for paragraph text."),
        ("Export", "File > Export (Ctrl+E) saves PNG, JPG, SVG, PDF and more. File > Publish to PDF creates print-ready PDFs."),
        ("AI Generate", "Describe what you want in the AI Generate docker; the result is editable vector artwork."),
    ];
    for (t, d) in tips {
        egui::CollapsingHeader::new(t).show(ui, |ui| {
            ui.label(d);
        });
    }
    let _ = app;
}
