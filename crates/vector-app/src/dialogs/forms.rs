//! The smaller dialogs: Document Properties,
//! Script Editor, About, Palette Editor, page dialogs, confirmations, Resample and PowerTRACE.

use super::{heading, note, ok_cancel_row, separator, window};
use crate::app::VectorApp;
use crate::theme;
use egui::{vec2, Color32};
use vector_core::{Color, NamedColor, Palette};

// ------------------------------------------------------------------------------------------
// state
// ------------------------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct ScriptEditorState {
    pub code: String,
    pub path: Option<std::path::PathBuf>,
    pub output: String,
    pub dirty: bool,
}

#[derive(Clone)]
pub struct PaletteEditorState {
    pub palette: Palette,
    pub selected: Option<usize>,
    pub path: Option<std::path::PathBuf>,
}

#[derive(Clone)]
pub struct TraceState {
    pub params: crate::trace::TraceParams,
    pub method: String,
    pub delete_original: bool,
    pub group: bool,
    pub preview: Option<(u64, Vec<vector_core::Object>, usize, usize)>,
    pub tex: Option<egui::TextureHandle>,
    pub image_id: u64,
    pub obj_id: u64,
}

// ------------------------------------------------------------------------------------------

pub fn show_all(app: &mut VectorApp, ctx: &egui::Context) {
    if app.dialogs.confirm_quit {
        confirm_quit(app, ctx);
    }
    if app.dialogs.confirm_close.is_some() {
        confirm_close(app, ctx);
    }
    if app.dialogs.new_doc.is_some() {
        super::new_doc::show(app, ctx);
    }
    if app.dialogs.print.is_some() {
        super::print::show(app, ctx);
    }
    if app.dialogs.doc_props {
        super::docprops::show(app, ctx);
    }
    if app.dialogs.script_editor.is_some() {
        script_editor(app, ctx);
    }
    if app.dialogs.palette_editor.is_some() {
        palette_editor(app, ctx);
    }
    if app.dialogs.rename_page.is_some() {
        super::layout::rename_page(app, ctx);
    }
    if app.dialogs.goto_page.is_some() {
        super::layout::goto_page(app, ctx);
    }
    if app.dialogs.insert_page.is_some() {
        super::layout::insert_page(app, ctx);
    }
    if app.dialogs.duplicate_page.is_some() {
        super::layout::duplicate_page(app, ctx);
    }
    if app.dialogs.delete_page.is_some() {
        super::layout::delete_page(app, ctx);
    }
    if app.dialogs.resample.is_some() {
        resample(app, ctx);
    }
    if app.dialogs.trace.is_some() {
        trace(app, ctx);
    }
    if app.dialogs.message_box.is_some() {
        message_box(app, ctx);
    }
}



// ------------------------------------------------------------------------------------------
// confirmations
// ------------------------------------------------------------------------------------------

fn confirm_quit(app: &mut VectorApp, ctx: &egui::Context) {
    let names: Vec<String> = app.docs.iter().filter(|d| d.history.is_modified()).map(|d| d.display_name()).collect();
    let mut choice = None;
    let ((), dismissed) = window(ctx, "confirm_quit", vector_core::APP_NAME, vec2(460.0, 200.0), |ui| {
        ui.add_space(12.0);
        ui.label("Save changes to the following documents before closing?");
        egui::ScrollArea::vertical().max_height(70.0).show(ui, |ui| {
            for n in &names {
                ui.label(egui::RichText::new(n).strong());
            }
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            ui.horizontal(|ui| {
                if super::button(ui, "Cancel", true) {
                    choice = Some(super::Choice::Cancel);
                }
                if super::button(ui, "No", true) {
                    choice = Some(super::Choice::No);
                }
                if super::ok_button(ui, "Yes", true) {
                    choice = Some(super::Choice::Yes);
                }
            });
        });
    });
    if dismissed {
        choice = Some(super::Choice::Cancel);
    }
    match choice {
        Some(super::Choice::Yes) => {
            app.dialogs.confirm_quit = false;
            let mut ok = true;
            for i in 0..app.docs.len() {
                if app.docs[i].history.is_modified() && !app.save_doc(i, false) {
                    ok = false;
                    break;
                }
            }
            if ok {
                app.quit_requested = true;
            }
        }
        Some(super::Choice::No) => {
            app.dialogs.confirm_quit = false;
            app.quit_requested = true;
        }
        Some(super::Choice::Cancel) => app.dialogs.confirm_quit = false,
        None => {}
    }
}

fn confirm_close(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(i) = app.dialogs.confirm_close else { return };
    if i >= app.docs.len() {
        app.dialogs.confirm_close = None;
        return;
    }
    let name = app.docs[i].display_name().trim_end_matches('*').to_string();
    let mut choice = None;
    let ((), dismissed) = window(ctx, "confirm_close", vector_core::APP_NAME, vec2(420.0, 150.0), |ui| {
        ui.add_space(14.0);
        ui.label(format!("Save changes to {name}?"));
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            ui.horizontal(|ui| {
                if super::button(ui, "Cancel", true) {
                    choice = Some(super::Choice::Cancel);
                }
                if super::button(ui, "No", true) {
                    choice = Some(super::Choice::No);
                }
                if super::ok_button(ui, "Yes", true) {
                    choice = Some(super::Choice::Yes);
                }
            });
        });
    });
    if dismissed {
        choice = Some(super::Choice::Cancel);
    }
    match choice {
        Some(super::Choice::Yes) => {
            app.dialogs.confirm_close = None;
            if app.save_doc(i, false) {
                app.close_doc_now(i);
                app.continue_close_queue();
            } else {
                app.dialogs.close_queue.clear();
            }
        }
        Some(super::Choice::No) => {
            app.dialogs.confirm_close = None;
            app.close_doc_now(i);
            app.continue_close_queue();
        }
        Some(super::Choice::Cancel) => {
            app.dialogs.confirm_close = None;
            app.dialogs.close_queue.clear();
        }
        None => {}
    }
}

fn message_box(app: &mut VectorApp, ctx: &egui::Context) {
    let Some((title, text)) = app.dialogs.message_box.clone() else { return };
    let mut close = false;
    let ((), dismissed) = window(ctx, "msgbox", &title, vec2(440.0, 170.0), |ui| {
        ui.add_space(12.0);
        egui::ScrollArea::vertical().max_height(90.0).show(ui, |ui| {
            ui.label(&text);
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            if super::ok_button(ui, "OK", true) {
                close = true;
            }
        });
    });
    if close || dismissed {
        app.dialogs.message_box = None;
    }
}

// ------------------------------------------------------------------------------------------
// Export
// ------------------------------------------------------------------------------------------

// ------------------------------------------------------------------------------------------
// Print / Print Preview
// ------------------------------------------------------------------------------------------

pub fn parse_page_range(s: &str, n: usize) -> Vec<usize> {
    let mut v = vec![];
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.trim().parse::<usize>(), b.trim().parse::<usize>()) {
                for p in a.min(b)..=a.max(b) {
                    if p >= 1 && p <= n && !v.contains(&(p - 1)) {
                        v.push(p - 1);
                    }
                }
            }
        } else if let Ok(p) = part.parse::<usize>() {
            if p >= 1 && p <= n && !v.contains(&(p - 1)) {
                v.push(p - 1);
            }
        }
    }
    v
}

// ------------------------------------------------------------------------------------------
// Document Properties
// ------------------------------------------------------------------------------------------

// ------------------------------------------------------------------------------------------
// Script Editor
// ------------------------------------------------------------------------------------------

pub const SCRIPT_TEMPLATE: &str = "// Vector script (Rhai). Coordinates are in points, y down.\n// Shapes: rect(x, y, w, h), ellipse(x, y, w, h), line(x0, y0, x1, y1), text(x, y, \"text\")\n// Style:  fill(\"#ff0000\"), outline(\"#000000\", width)\n// Edit:   select_all(), move_by(dx, dy), rotate(deg), scale(sx, sy)\n// Any command: run(\"Group\"), run(\"ZoomToPage\")\n\nfor i in 0..5 {\n    ellipse(100 + i * 60, 100, 50, 50);\n    fill(\"#1d4a85\");\n}\n";

fn script_editor(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.script_editor.clone() else { return };
    let mut run = false;
    let mut close = false;
    let title = format!("Script Editor - {}{}", st.path.as_ref().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".into()), if st.dirty { "*" } else { "" });
    let ((), dismissed) = window(ctx, "script_editor", &title, vec2(820.0, 620.0), |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("New").clicked() {
                st = ScriptEditorState { code: SCRIPT_TEMPLATE.to_string(), ..Default::default() };
            }
            if ui.button("Open...").clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("Vector script", &["rhai", "vsc"]).set_directory(crate::scripting::scripts_dir()).pick_file() {
                    if let Ok(t) = std::fs::read_to_string(&p) {
                        st.code = t;
                        st.path = Some(p);
                        st.dirty = false;
                    }
                }
            }
            if ui.button("Save").clicked() {
                let p = st.path.clone().or_else(|| rfd::FileDialog::new().add_filter("Vector script", &["rhai"]).set_directory(crate::scripting::scripts_dir()).set_file_name("Script.rhai").save_file());
                if let Some(p) = p {
                    match std::fs::write(&p, &st.code) {
                        Ok(_) => {
                            st.path = Some(p);
                            st.dirty = false;
                        }
                        Err(e) => st.output = format!("Could not save: {e}"),
                    }
                }
            }
            ui.separator();
            if ui.add(egui::Button::new(egui::RichText::new("▶ Run").color(Color32::WHITE)).fill(theme::ACCENT)).clicked() {
                run = true;
            }
            ui.separator();
            if ui.button("Insert recorded").on_hover_text("Insert the last recorded commands").clicked() {
                if !app.temp_script.is_empty() {
                    st.code.push('\n');
                    st.code.push_str(&app.temp_script);
                    st.dirty = true;
                }
            }
        });
        let avail = ui.available_height() - 120.0;
        egui::Frame::new().fill(theme::FIELD).stroke(egui::Stroke::new(1.0, theme::SEP)).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("script_code").max_height(avail).auto_shrink([false, false]).show(ui, |ui| {
                let r = ui.add(egui::TextEdit::multiline(&mut st.code).code_editor().desired_width(f32::INFINITY).desired_rows(24).frame(egui::Frame::NONE));
                if r.changed() {
                    st.dirty = true;
                }
            });
        });
        ui.label("Output:");
        egui::Frame::new().fill(theme::FIELD).stroke(egui::Stroke::new(1.0, theme::SEP)).inner_margin(4.0).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("script_out").max_height(60.0).auto_shrink([false, false]).show(ui, |ui| {
                ui.label(egui::RichText::new(&st.output).monospace().color(theme::TEXT_DIM));
            });
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            if super::button(ui, "Close", true) {
                close = true;
            }
        });
    });
    if run {
        let code = st.code.clone();
        st.output = match crate::scripting::run_script(app, &code) {
            Ok(()) => "Script finished.".into(),
            Err(e) => format!("Error: {e}"),
        };
    }
    if close || dismissed {
        app.dialogs.script_editor = None;
    } else {
        app.dialogs.script_editor = Some(st);
    }
}

// ------------------------------------------------------------------------------------------
// Palette Editor
// ------------------------------------------------------------------------------------------

fn palette_editor(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.palette_editor.clone() else { return };
    let mut result = (false, false);
    let ((), dismissed) = window(ctx, "palette_editor", "Palette Editor", vec2(640.0, 500.0), |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Palette:");
            ui.add(egui::TextEdit::singleline(&mut st.palette.name).desired_width(200.0));
            if ui.button("Open...").clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("Palettes", &["vpal", "txt", "gpl"]).pick_file() {
                    if let Ok(t) = std::fs::read_to_string(&p) {
                        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                        st.palette = Palette::parse(&name, &t);
                        st.path = Some(p);
                        st.selected = None;
                    }
                }
            }
            if ui.button("Save As...").clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("Vector palette", &["vpal"]).set_file_name(format!("{}.vpal", st.palette.name)).save_file() {
                    let _ = std::fs::write(&p, st.palette.serialize());
                    st.path = Some(p);
                }
            }
        });
        ui.horizontal_top(|ui| {
            egui::Frame::new().stroke(egui::Stroke::new(1.0, theme::SEP)).inner_margin(4.0).show(ui, |ui| {
                ui.set_width(330.0);
                ui.set_min_height(350.0);
                egui::ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                        for (i, c) in st.palette.colors.iter().enumerate() {
                            let r = crate::widgets::color_well(ui, Some(c.color), vec2(22.0, 22.0)).on_hover_text(&c.name);
                            if st.selected == Some(i) {
                                ui.painter().rect_stroke(r.rect.expand(1.0), 0.0, egui::Stroke::new(2.0, theme::FOCUS), egui::StrokeKind::Outside);
                            }
                            if r.clicked() {
                                st.selected = Some(i);
                            }
                        }
                    });
                });
            });
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                if ui.button("Add Color").clicked() {
                    let c = st.selected.and_then(|i| st.palette.colors.get(i)).map(|c| c.color).unwrap_or(Color::rgb(128, 128, 128));
                    st.palette.colors.push(NamedColor::new(c.to_hex(), c));
                    st.selected = Some(st.palette.colors.len() - 1);
                }
                if ui.add_enabled(st.selected.is_some(), egui::Button::new("Delete Color")).clicked() {
                    if let Some(i) = st.selected.take() {
                        if i < st.palette.colors.len() {
                            st.palette.colors.remove(i);
                        }
                    }
                }
                if ui.button("Sort by hue").clicked() {
                    st.palette.colors.sort_by(|a, b| a.color.to_hsb()[0].partial_cmp(&b.color.to_hsb()[0]).unwrap_or(std::cmp::Ordering::Equal));
                }
                if ui.button("Add document colors").clicked() {
                    if let Some(d) = app.doc() {
                        for c in d.doc.used_colors() {
                            if !st.palette.colors.iter().any(|x| x.color == c) {
                                st.palette.colors.push(NamedColor::new(c.to_hex(), c));
                            }
                        }
                    }
                }
                separator(ui);
                if let Some(i) = st.selected {
                    if let Some(c) = st.palette.colors.get_mut(i) {
                        ui.horizontal(|ui| {
                            ui.label("Name:");
                            ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(150.0));
                        });
                        crate::dockers::props::color_edit(ui, &mut c.color, false);
                        let mut c32 = theme::to_c32(c.color);
                        if egui::color_picker::color_picker_color32(ui, &mut c32, egui::color_picker::Alpha::Opaque) {
                            c.color = theme::from_c32(c32);
                        }
                        ui.checkbox(&mut c.spot, "Spot color");
                    }
                }
            });
        });
        result = ok_cancel_row(ui, "OK", true);
    });
    let (ok, cancel) = result;
    if ok {
        let p = st.palette.clone();
        if let Some(path) = &st.path {
            let _ = std::fs::write(path, p.serialize());
        }
        if let Some(existing) = app.palettes.palettes.iter_mut().find(|x| x.name == p.name) {
            *existing = p;
        } else {
            app.palettes.palettes.push(p);
        }
        app.dialogs.palette_editor = None;
    } else if cancel || dismissed {
        app.dialogs.palette_editor = None;
    } else {
        app.dialogs.palette_editor = Some(st);
    }
}

// ------------------------------------------------------------------------------------------
// Pages
// ------------------------------------------------------------------------------------------

// ------------------------------------------------------------------------------------------
// Resample
// ------------------------------------------------------------------------------------------

fn resample(app: &mut VectorApp, ctx: &egui::Context) {
    let Some((mut w, mut h, mut keep)) = app.dialogs.resample else { return };
    let orig = app.selected_image_size();
    let mut result = (false, false);
    let ((), dismissed) = window(ctx, "resample", "Resample", vec2(360.0, 210.0), |ui| {
        ui.add_space(12.0);
        if let Some((ow, oh)) = orig {
            note(ui, &format!("Original size: {ow} × {oh} pixels"));
            ui.horizontal(|ui| {
                ui.label("Width:");
                if ui.add(egui::DragValue::new(&mut w).range(1.0..=20000.0).suffix(" px")).changed() && keep {
                    h = (w * oh as f64 / ow as f64).round();
                }
            });
            ui.horizontal(|ui| {
                ui.label("Height:");
                if ui.add(egui::DragValue::new(&mut h).range(1.0..=20000.0).suffix(" px")).changed() && keep {
                    w = (h * ow as f64 / oh as f64).round();
                }
            });
            ui.checkbox(&mut keep, "Maintain aspect ratio");
        } else {
            note(ui, "Select a bitmap.");
        }
        result = ok_cancel_row(ui, "OK", orig.is_some());
    });
    if result.0 {
        app.dialogs.resample = None;
        app.resample_selected(w as u32, h as u32);
    } else if result.1 || dismissed {
        app.dialogs.resample = None;
    } else {
        app.dialogs.resample = Some((w, h, keep));
    }
}

// ------------------------------------------------------------------------------------------
// PowerTRACE
// ------------------------------------------------------------------------------------------

fn trace(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.trace.take() else { return };
    let Some(d) = app.doc() else { return };
    let Some(obj) = d.doc.find(st.obj_id).cloned() else { return };
    let Some(img) = d.doc.images.get(&st.image_id).cloned() else { return };
    // source preview texture
    if st.tex.is_none() {
        let ci = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba);
        st.tex = Some(ctx.load_texture("trace_src", ci, egui::TextureOptions::LINEAR));
    }
    // (re)compute the trace preview when parameters change
    let key = {
        let p = &st.params;
        vector_render::hash_str(&format!("{}-{}-{}-{}", p.colors, p.smoothing as i64, p.min_area, p.remove_background))
    };
    if st.preview.as_ref().map(|p| p.0) != Some(key) {
        let (iw, ih) = match &obj.kind {
            vector_core::ObjKind::Image(i) => (i.w, i.h),
            _ => (1.0, 1.0),
        };
        let to_doc = obj.transform * kurbo::Affine::scale_non_uniform(iw / img.width as f64, ih / img.height as f64);
        let mut next = 1u64 << 40;
        let r = crate::trace::trace(img.width, img.height, &img.rgba, &st.params, to_doc, || {
            next += 1;
            next
        });
        let colors = r.palette.len();
        st.preview = Some((key, r.objects, r.node_count, colors));
    }
    let mut result = (false, false);
    let ((), dismissed) = window(ctx, "trace", "PowerTRACE", vec2(900.0, 600.0), |ui| {
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(260.0);
                ui.spacing_mut().item_spacing.y = 8.0;
                ui.horizontal(|ui| {
                    ui.label("Trace type:");
                    let methods = ["Clipart", "Line Art", "Logo", "Detailed Logo", "Low Quality Image", "High Quality Image", "Centerline"];
                    egui::ComboBox::from_id_salt("tr_m").selected_text(st.method.as_str()).width(160.0).show_ui(ui, |ui| {
                        for m in methods {
                            if ui.selectable_label(st.method == m, m).clicked() {
                                st.method = m.to_string();
                                st.params = crate::trace::preset(m);
                            }
                        }
                    });
                });
                separator(ui);
                heading(ui, "Settings");
                ui.add(egui::Slider::new(&mut st.params.colors, 2..=64).text("Colors"));
                ui.add(egui::Slider::new(&mut st.params.smoothing, 0.0..=100.0).text("Smoothing"));
                ui.add(egui::Slider::new(&mut st.params.min_area, 1..=200).text("Remove spots (px)"));
                ui.checkbox(&mut st.params.remove_background, "Remove background");
                separator(ui);
                heading(ui, "Options");
                ui.checkbox(&mut st.delete_original, "Delete original image");
                ui.checkbox(&mut st.group, "Group objects when finished");
                separator(ui);
                if let Some((_, objs, nodes, colors)) = &st.preview {
                    heading(ui, "Trace result details");
                    note(ui, &format!("Curves: {}\nNodes: {}\nColors: {}", objs.len(), nodes, colors));
                }
            });
            ui.separator();
            // before / after preview
            let avail = vec2(ui.available_width(), 500.0);
            let (r, _) = ui.allocate_exact_size(avail, egui::Sense::hover());
            let half = egui::Rect::from_min_size(r.min, vec2(r.width() / 2.0 - 4.0, r.height()));
            let half2 = half.translate(vec2(r.width() / 2.0 + 4.0, 0.0));
            for h in [half, half2] {
                ui.painter().rect_filled(h, 0.0, Color32::WHITE);
            }
            ui.painter().text(half.left_top() + vec2(6.0, 4.0), egui::Align2::LEFT_TOP, "Before", egui::FontId::proportional(11.0), Color32::DARK_GRAY);
            ui.painter().text(half2.left_top() + vec2(6.0, 4.0), egui::Align2::LEFT_TOP, "After", egui::FontId::proportional(11.0), Color32::DARK_GRAY);
            let sz = vec2(img.width as f32, img.height as f32);
            let s = ((half.width() - 20.0) / sz.x).min((half.height() - 30.0) / sz.y);
            let ir = egui::Rect::from_center_size(half.center() + vec2(0.0, 8.0), sz * s);
            if let Some(t) = &st.tex {
                ui.painter().image(t.id(), ir, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
            }
            if let Some((_, objs, _, _)) = &st.preview {
                let tex_key = format!("trace_prev_{}", st.preview.as_ref().map(|p| p.0).unwrap_or(0));
                let tex = match app.ts.texture_cache.get(&tex_key) {
                    Some(t) => Some(t.clone()),
                    None => {
                        let ob = obj.bounds();
                        let px = (ir.width() as f64 * ctx.pixels_per_point() as f64).max(16.0);
                        let mut pm = tiny_skia::Pixmap::new(px as u32, (px * ob.height() / ob.width().max(1e-6)).max(16.0) as u32);
                        pm.as_mut().map(|p| p.fill(tiny_skia::Color::WHITE));
                        let doc = &app.doc().unwrap().doc;
                        let view = kurbo::Affine::scale(px / ob.width().max(1e-6)) * kurbo::Affine::translate((-ob.x0, -ob.y0));
                        let opts = vector_render::RenderOptions { draw_page: false, ..Default::default() };
                        let t = pm.map(|mut pm| {
                            vector_render::Renderer::new().render_objects(doc, objs, view, &mut pm, &opts);
                            let ci = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
                            ctx.load_texture(&tex_key, ci, egui::TextureOptions::LINEAR)
                        });
                        if let Some(t) = &t {
                            app.ts.texture_cache.retain(|k, _| !k.starts_with("trace_prev_"));
                            app.ts.texture_cache.insert(tex_key, t.clone());
                        }
                        t
                    }
                };
                if let Some(t) = tex {
                    let ir2 = egui::Rect::from_center_size(half2.center() + vec2(0.0, 8.0), sz * s);
                    ui.painter().image(t.id(), ir2, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                }
            }
        });
        result = ok_cancel_row(ui, "OK", st.preview.as_ref().map(|p| !p.1.is_empty()).unwrap_or(false));
    });
    if result.0 {
        let objs = st.preview.take().map(|p| p.1).unwrap_or_default();
        app.apply_trace(st.obj_id, objs, st.delete_original, st.group);
        app.ts.texture_cache.retain(|k, _| !k.starts_with("trace_"));
    } else if result.1 || dismissed {
        app.ts.texture_cache.retain(|k, _| !k.starts_with("trace_"));
    } else {
        app.dialogs.trace = Some(st);
    }
}
