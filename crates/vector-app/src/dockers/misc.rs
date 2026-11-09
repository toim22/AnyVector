//! Comments, Object Data, Object Styles, Symbols, Export, Assets, Tray, Links and Rollovers,
//! Sources, Views, History, Scripts and Learn dockers.

use super::{heading, hint, no_doc, primary};
use crate::app::VectorApp;
use crate::commands::Cmd;
use crate::propbar::edit_sel;
use egui::Ui;
use vector_core::document::{Comment, ObjectStyle, SavedView, SymbolDef};
use vector_core::{ObjKind, Object};

fn first(app: &VectorApp) -> Option<Object> {
    let d = app.doc()?;
    d.doc.find(*d.selection.first()?).cloned()
}

pub fn comments(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    ui.add(egui::TextEdit::multiline(&mut app.dockers.ui.comment).hint_text("Add a comment").desired_rows(3).desired_width(f32::INFINITY));
    if primary(ui, "Post", !app.dockers.ui.comment.trim().is_empty()) {
        let text = std::mem::take(&mut app.dockers.ui.comment);
        let author = if app.settings.user_name.is_empty() { std::env::var("USERNAME").unwrap_or_else(|_| "User".into()) } else { app.settings.user_name.clone() };
        let pos = app.doc().and_then(|d| d.selection_bounds()).map(|b| b.center()).unwrap_or_default();
        if let Some(d) = app.doc_mut() {
            let pid = d.doc.pages[d.page].id;
            d.modify("Add Comment", |doc, _| {
                let id = doc.new_id();
                doc.comments.push(Comment { id, author, text, pos, page: pid, resolved: false, replies: vec![] });
            });
        }
    }
    let Some(d) = app.doc() else { return };
    let comments = d.doc.comments.clone();
    if comments.is_empty() {
        hint(ui, "No comments in this document.");
    }
    let mut toggle = None;
    let mut delete = None;
    for (i, c) in comments.iter().enumerate() {
        egui::Frame::new().fill(crate::theme::FIELD).inner_margin(6).corner_radius(3.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(&c.author).strong());
            let txt = egui::RichText::new(&c.text);
            ui.label(if c.resolved { txt.strikethrough().color(crate::theme::TEXT_DIM) } else { txt });
            ui.horizontal(|ui| {
                if ui.small_button(if c.resolved { "Reopen" } else { "Resolve" }).clicked() {
                    toggle = Some(i);
                }
                if ui.small_button("Delete").clicked() {
                    delete = Some(i);
                }
            });
        });
    }
    if let Some(d) = app.doc_mut() {
        if let Some(i) = toggle {
            d.modify("Resolve Comment", |doc, _| doc.comments[i].resolved = !doc.comments[i].resolved);
        }
        if let Some(i) = delete {
            d.modify("Delete Comment", |doc, _| {
                doc.comments.remove(i);
            });
        }
    }
}

pub fn object_data(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let Some(o) = first(app) else {
        hint(ui, "Select an object to view or edit its data fields.");
        return;
    };
    egui::Grid::new("objdata").num_columns(3).striped(true).show(ui, |ui| {
        ui.label(egui::RichText::new("Field").strong());
        ui.label(egui::RichText::new("Value").strong());
        ui.end_row();
        let mut remove = None;
        for (i, (k, v)) in o.data.iter().enumerate() {
            ui.label(k);
            let mut val = v.clone();
            if ui.text_edit_singleline(&mut val).lost_focus() && val != *v {
                edit_sel(app, "Object Data", |x| {
                    if let Some(e) = x.data.get_mut(i) {
                        e.1 = val.clone();
                    }
                });
            }
            if ui.small_button("×").clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
        if let Some(i) = remove {
            edit_sel(app, "Object Data", |x| {
                if i < x.data.len() {
                    x.data.remove(i);
                }
            });
        }
    });
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.data_name).hint_text("Field").desired_width(90.0));
        ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.data_value).hint_text("Value").desired_width(110.0));
        if ui.button("Add").clicked() && !app.dockers.ui.data_name.is_empty() {
            let kv = (std::mem::take(&mut app.dockers.ui.data_name), std::mem::take(&mut app.dockers.ui.data_value));
            edit_sel(app, "Object Data", |x| x.data.push(kv.clone()));
        }
    });
}

pub fn object_styles(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.style_name).hint_text("Style name").desired_width(140.0));
        if ui.add_enabled(app.has_selection() && !app.dockers.ui.style_name.is_empty(), egui::Button::new("New from selection")).clicked() {
            if let Some(o) = first(app) {
                let name = std::mem::take(&mut app.dockers.ui.style_name);
                if let Some(d) = app.doc_mut() {
                    d.modify("New Object Style", |doc, _| {
                        doc.object_styles.retain(|s| s.name != name);
                        doc.object_styles.push(ObjectStyle { name, style: o.style.clone() });
                    });
                }
            }
        }
    });
    let Some(d) = app.doc() else { return };
    let styles = d.doc.object_styles.clone();
    if styles.is_empty() {
        hint(ui, "No object styles yet. Select an object and create a style from it.");
    }
    let mut apply = None;
    let mut delete = None;
    for (i, s) in styles.iter().enumerate() {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
            crate::widgets::paint_well(ui.painter(), r, s.style.fill.main_color());
            ui.label(&s.name);
            if ui.small_button("Apply").clicked() {
                apply = Some(s.clone());
            }
            if ui.small_button("×").clicked() {
                delete = Some(i);
            }
        });
    }
    if let Some(s) = apply {
        edit_sel(app, "Apply Object Style", |x| {
            x.style = s.style.clone();
            x.object_style = Some(s.name.clone());
        });
    }
    if let Some(i) = delete {
        if let Some(d) = app.doc_mut() {
            d.modify("Delete Object Style", |doc, _| {
                doc.object_styles.remove(i);
            });
        }
    }
}

pub fn symbols(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.sym_name).hint_text("Symbol name").desired_width(140.0));
        if ui.add_enabled(app.has_selection(), egui::Button::new("New Symbol")).clicked() {
            let name = std::mem::take(&mut app.dockers.ui.sym_name);
            crate::exec::new_symbol(app, &name);
        }
    });
    let Some(d) = app.doc() else { return };
    let syms: Vec<SymbolDef> = d.doc.symbols.clone();
    if syms.is_empty() {
        hint(ui, "Convert objects to a symbol to reuse them. Every instance updates when the symbol changes.");
    }
    let mut insert = None;
    let mut delete = None;
    for s in &syms {
        ui.horizontal(|ui| {
            if let Some(tex) = thumb(app, ui.ctx(), &format!("sym{}", s.id), &s.objects) {
                ui.image(egui::load::SizedTexture::new(tex.id(), egui::vec2(36.0, 36.0)));
            }
            ui.label(&s.name);
            let count = app.doc().map(|d| count_instances(&d.doc, s.id)).unwrap_or(0);
            ui.label(egui::RichText::new(format!("({count})")).color(crate::theme::TEXT_DIM));
            if ui.small_button("Insert").clicked() {
                insert = Some(s.id);
            }
            if ui.small_button("×").on_hover_text("Delete symbol (instances become groups)").clicked() {
                delete = Some(s.id);
            }
        });
    }
    if let Some(id) = insert {
        if let Some(d) = app.doc_mut() {
            let c = d.page_ref().rect().center();
            let bb = d.doc.symbol_bounds(id).unwrap_or_default();
            let oid = d.doc.new_id();
            let o = Object::new(oid, ObjKind::Symbol(id), kurbo::Affine::translate(c - bb.center()), Default::default());
            d.add_objects("Insert Symbol", vec![o]);
        }
    }
    if let Some(id) = delete {
        if let Some(d) = app.doc_mut() {
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

fn thumb(app: &mut VectorApp, ctx: &egui::Context, key: &str, objs: &[Object]) -> Option<egui::TextureHandle> {
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

pub fn export(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    use super::EXPORT_DOCKER_FORMATS as F;
    ui.horizontal(|ui| {
        ui.label("Default file format:");
        egui::ComboBox::from_id_salt("dk_exp").selected_text(F[app.dockers.ui.export_format.min(F.len() - 1)]).width(80.0).show_ui(ui, |ui| {
            for (i, f) in F.iter().enumerate() {
                ui.selectable_value(&mut app.dockers.ui.export_format, i, *f);
            }
        });
    });
    ui.horizontal(|ui| {
        let has_sel = app.has_selection();
        if ui.add_enabled(has_sel, egui::Button::new("Add selection")).clicked() {
            app.run(Cmd::ExportDockerAddSelection);
        }
        if ui.button("Add current page").clicked() {
            app.run(Cmd::ExportDockerAddPage);
        }
        if ui.button("Add all pages").clicked() {
            app.run(Cmd::ExportDockerAddAllPages);
        }
    });
    ui.separator();
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
    if app.dockers.ui.export_items.is_empty() {
        hint(ui, "Add the selection, the current page or all pages to export them as assets.");
        return;
    }
    let mut run: Vec<usize> = vec![];
    let mut delete: Vec<usize> = vec![];
    egui::ScrollArea::vertical().id_salt("dk_exp_items").max_height(360.0).show(ui, |ui| {
        for (i, it) in app.dockers.ui.export_items.iter_mut().enumerate() {
            egui::Frame::new().stroke(egui::Stroke::new(1.0, crate::theme::SEP)).inner_margin(6.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.checkbox(&mut it.selected, "");
                    ui.add(egui::TextEdit::singleline(&mut it.name).desired_width(120.0));
                    ui.add(egui::TextEdit::singleline(&mut it.suffix).hint_text("suffix").desired_width(60.0));
                });
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt(("dk_exp_fmt", i)).selected_text(F[it.format.min(F.len() - 1)]).width(64.0).show_ui(ui, |ui| {
                        for (k, f) in F.iter().enumerate() {
                            ui.selectable_value(&mut it.format, k, *f);
                        }
                    });
                    if !matches!(F[it.format], "PDF" | "SVG") {
                        ui.add(egui::DragValue::new(&mut it.dpi).range(36.0..=1200.0).suffix(" dpi"));
                    }
                });
                let folder = it.folder.display().to_string();
                ui.add(egui::Label::new(egui::RichText::new(&folder).small()).truncate()).on_hover_text(&folder);
                ui.horizontal(|ui| {
                    if ui.button("Folder...").on_hover_text("Destination folder").clicked() {
                        if let Some(d) = rfd::FileDialog::new().set_directory(&it.folder).pick_folder() {
                            it.folder = d;
                        }
                    }
                    if ui.button("Export").clicked() {
                        run.push(i);
                    }
                    if ui.button("Delete").clicked() {
                        delete.push(i);
                    }
                });
            });
        }
    });
    ui.horizontal(|ui| {
        let all = app.dockers.ui.export_items.iter().all(|i| i.selected);
        if ui.button(if all { "Deselect all" } else { "Select all" }).clicked() {
            app.dockers.ui.export_items.iter_mut().for_each(|i| i.selected = !all);
        }
        let any = app.dockers.ui.export_items.iter().any(|i| i.selected);
        if ui.add_enabled(any, egui::Button::new("Export")).clicked() {
            run.extend(app.dockers.ui.export_items.iter().enumerate().filter(|(_, i)| i.selected).map(|(k, _)| k));
        }
        if ui.add_enabled(any, egui::Button::new("Delete")).clicked() {
            delete.extend(app.dockers.ui.export_items.iter().enumerate().filter(|(_, i)| i.selected).map(|(k, _)| k));
        }
    });
    let mut done = 0;
    for i in run {
        let it = app.dockers.ui.export_items[i].clone();
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
        app.dockers.ui.export_items.remove(i);
    }
}

pub fn assets(app: &mut VectorApp, ui: &mut Ui) {
    hint(ui, "Assets: reusable content from your local Vector library folder.");
    let dir = crate::settings::config_dir().join("Assets");
    let _ = std::fs::create_dir_all(&dir);
    if ui.button("Open library folder").clicked() {
        let _ = std::process::Command::new("explorer").arg(&dir).spawn();
    }
    let files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir).map(|r| r.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
    if files.is_empty() {
        hint(ui, "Save .cdr, .svg or image files to the library folder to see them here.");
    }
    for f in files {
        let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if ui.button(&name).on_hover_text("Import into the current document").clicked() {
            if app.has_doc() {
                app.import_path(&f, None);
            } else {
                app.open_path(&f);
            }
        }
    }
}

pub fn tray(app: &mut VectorApp, ui: &mut Ui) {
    hint(ui, "The tray holds objects temporarily, across documents.");
    if ui.add_enabled(app.has_selection(), egui::Button::new("Add selection to tray")).clicked() {
        if let Some(d) = app.doc() {
            let objs: Vec<Object> = d.selection.iter().filter_map(|id| d.doc.find(*id).cloned()).collect();
            let g = Object::new(0, ObjKind::Group(objs), kurbo::Affine::IDENTITY, Default::default());
            app.dockers.ui.tray.push(g);
        }
    }
    let items = app.dockers.ui.tray.clone();
    let mut remove = None;
    for (i, g) in items.iter().enumerate() {
        ui.horizontal(|ui| {
            let n = g.children().len();
            ui.label(format!("Item {} ({} objects)", i + 1, n));
            if ui.add_enabled(app.has_doc(), egui::Button::new("Place")).clicked() {
                if let Some(d) = app.doc_mut() {
                    let mut g = g.clone();
                    d.doc.reassign_ids(&mut g);
                    let kids = match g.kind {
                        ObjKind::Group(k) => k,
                        _ => vec![],
                    };
                    d.add_objects("Place from Tray", kids);
                }
            }
            if ui.small_button("×").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        app.dockers.ui.tray.remove(i);
    }
}

pub fn links(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let Some(o) = first(app) else {
        hint(ui, "Select an object to assign a hyperlink. Links are kept in PDF and SVG exports.");
        return;
    };
    if app.dockers.ui.link_url.is_empty() {
        app.dockers.ui.link_url = o.url.clone().unwrap_or_default();
    }
    ui.label("URL:");
    ui.text_edit_singleline(&mut app.dockers.ui.link_url);
    ui.horizontal(|ui| {
        if primary(ui, "Apply", true) {
            let u = app.dockers.ui.link_url.trim().to_string();
            edit_sel(app, "Hyperlink", |x| x.url = if u.is_empty() { None } else { Some(u.clone()) });
        }
        if ui.button("Remove").clicked() {
            app.dockers.ui.link_url.clear();
            edit_sel(app, "Hyperlink", |x| x.url = None);
        }
    });
}

pub fn sources(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    hint(ui, "Images embedded in this document:");
    let Some(d) = app.doc() else { return };
    for (id, img) in &d.doc.images {
        ui.label(format!("#{id}  {}  {}×{}  {}", if img.name.is_empty() { "image" } else { &img.name }, img.width, img.height, img.format));
    }
    if d.doc.images.is_empty() {
        hint(ui, "None");
    }
}

pub fn views(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.view_name).hint_text("View name").desired_width(140.0));
        if ui.button("Add current view").clicked() {
            let name = if app.dockers.ui.view_name.is_empty() { None } else { Some(std::mem::take(&mut app.dockers.ui.view_name)) };
            if let Some(d) = app.doc_mut() {
                let center = d.to_doc(d.canvas_rect.center());
                let n = d.doc.views.len() + 1;
                let v = SavedView { name: name.unwrap_or_else(|| format!("View-{n}")), page: d.page, zoom: d.zoom, center };
                d.doc.views.push(v);
                d.history.touch();
            }
        }
    });
    let Some(d) = app.doc() else { return };
    let views = d.doc.views.clone();
    let mut go = None;
    let mut del = None;
    for (i, v) in views.iter().enumerate() {
        ui.horizontal(|ui| {
            if ui.button(&v.name).clicked() {
                go = Some(v.clone());
            }
            ui.label(egui::RichText::new(format!("page {}, {:.0}%", v.page + 1, v.zoom / (96.0 / 72.0) * 100.0)).color(crate::theme::TEXT_DIM));
            if ui.small_button("×").clicked() {
                del = Some(i);
            }
        });
    }
    if let Some(d) = app.doc_mut() {
        if let Some(v) = go {
            d.page = v.page.min(d.doc.pages.len() - 1);
            d.zoom = v.zoom;
            let cs = d.canvas_rect.size();
            d.origin = egui::vec2((cs.x as f64 / 2.0 - v.center.x * v.zoom) as f32, (cs.y as f64 / 2.0 - v.center.y * v.zoom) as f32);
            d.needs_fit = false;
        }
        if let Some(i) = del {
            d.doc.views.remove(i);
            d.history.touch();
        }
    }
}

pub fn history(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let Some(d) = app.doc() else { return };
    let undo = d.history.undo_labels();
    let redo = d.history.redo_labels();
    let mut cmd = None;
    // oldest at the top
    let n = undo.len();
    if ui.selectable_label(false, "Open / New").clicked() && n > 0 {
        cmd = Some(Cmd::UndoTo(n));
    }
    for (i, l) in undo.iter().enumerate().rev() {
        let r = ui.selectable_label(i == 0, l);
        if r.clicked() && i > 0 {
            cmd = Some(Cmd::UndoTo(i));
        }
    }
    for (i, l) in redo.iter().enumerate() {
        if ui.selectable_label(false, egui::RichText::new(l).color(crate::theme::TEXT_DIM)).clicked() {
            cmd = Some(Cmd::RedoTo(i + 1));
        }
    }
    if let Some(c) = cmd {
        app.run(c);
    }
}

pub fn scripts(app: &mut VectorApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if ui.button("Script Editor...").clicked() {
            app.run(Cmd::ScriptEditor);
        }
        if ui.button("Open folder").clicked() {
            let _ = std::process::Command::new("explorer").arg(crate::scripting::scripts_dir()).spawn();
        }
    });
    heading(ui, "Scripts");
    let list = crate::scripting::list_scripts();
    if list.is_empty() {
        hint(ui, "No scripts yet. Record one with Tools > Scripts > Start Recording, or write one in the Script Editor.");
    }
    for p in list {
        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        ui.horizontal(|ui| {
            if ui.button("▶").on_hover_text("Run").clicked() {
                match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|s| crate::scripting::run_script(app, &s)) {
                    Ok(()) => app.dockers.ui.script_error = None,
                    Err(e) => app.dockers.ui.script_error = Some(e),
                }
            }
            ui.label(&name);
        });
    }
    if let Some(e) = &app.dockers.ui.script_error {
        ui.label(egui::RichText::new(e).color(egui::Color32::from_rgb(0xe8, 0x6a, 0x6a)));
    }
}

pub fn learn(app: &mut VectorApp, ui: &mut Ui) {
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
