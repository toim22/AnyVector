//! Properties docker: Fill · Outline · Transparency · Summary of the selection.

use super::{heading, hint, no_doc};
use crate::app::VectorApp;
use crate::propbar::edit_sel;
use crate::theme;
use egui::Ui;
use vector_core::style::{dash_presets, ArrowHead, Gradient, GradientKind, GradientStop, LineCap, LineJoin, MergeMode, OutlinePosition, PatternFill, PatternKind, SpreadMode};
use vector_core::{Color, Fill, Object, Transparency};

fn first(app: &VectorApp) -> Option<Object> {
    let d = app.doc()?;
    d.doc.find(*d.selection.first()?).cloned()
}

pub fn color_edit(ui: &mut Ui, c: &mut Color, alpha: bool) -> bool {
    let mut c32 = theme::to_c32(*c);
    let changed = egui::color_picker::color_edit_button_srgba(ui, &mut c32, if alpha { egui::color_picker::Alpha::OnlyBlend } else { egui::color_picker::Alpha::Opaque }).changed();
    if changed {
        *c = theme::from_c32(c32);
    }
    changed
}

pub fn properties(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let Some(o) = first(app) else {
        hint(ui, "Select an object to view and edit its fill, outline and transparency.");
        doc_summary(app, ui);
        return;
    };
    let n = app.doc().map(|d| d.selection.len()).unwrap_or(0);
    ui.label(egui::RichText::new(if n > 1 { format!("{n} objects") } else { o.display_name() }).strong());
    // name
    if n == 1 {
        let mut name = o.name.clone();
        ui.horizontal(|ui| {
            ui.label("Name:");
            if ui.text_edit_singleline(&mut name).lost_focus() && name != o.name {
                edit_sel(app, "Rename", |x| x.name = name.clone());
            }
        });
    }
    fill_section(app, ui, &o);
    outline_section(app, ui, &o);
    transparency_section(app, ui, &o);
    heading(ui, "Summary");
    let info = crate::statusbar::object_info(app);
    hint(ui, &info);
    if let Some(bb) = app.doc().and_then(|d| d.selection_bounds()) {
        let (u, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap();
        hint(ui, &format!("Size: {} × {}", u.format(bb.width(), dpi), u.format(bb.height(), dpi)));
    }
    let mut vis = o.visible;
    let mut print = o.printable;
    let mut lock = o.locked;
    ui.horizontal(|ui| {
        if ui.checkbox(&mut vis, "Visible").changed() {
            edit_sel(app, "Visibility", |x| x.visible = vis);
        }
        if ui.checkbox(&mut print, "Printable").changed() {
            edit_sel(app, "Printable", |x| x.printable = print);
        }
        if ui.checkbox(&mut lock, "Locked").changed() {
            edit_sel(app, "Lock", |x| x.locked = lock);
        }
    });
}

fn doc_summary(app: &VectorApp, ui: &mut Ui) {
    let Some(d) = app.doc() else { return };
    heading(ui, "Document");
    let u = d.doc.settings.unit;
    let dpi = d.doc.settings.dpi;
    let p = d.page_ref();
    hint(ui, &format!("Page {} of {}: {} × {}", d.page + 1, d.doc.pages.len(), u.format(p.width, dpi), u.format(p.height, dpi)));
    let count: usize = p.layers.iter().map(|l| l.objects.len()).sum();
    hint(ui, &format!("{count} objects on {} layers", p.layers.len()));
    hint(ui, &format!("Resolution: {} dpi", dpi));
}

fn fill_section(app: &mut VectorApp, ui: &mut Ui, o: &Object) {
    heading(ui, "Fill");
    let fill = o.style.fill.clone();
    let base = fill.main_color().unwrap_or(app.default_fill);
    ui.horizontal_wrapped(|ui| {
        for (i, name) in ["None", "Uniform", "Fountain", "Pattern", "Mesh"].iter().enumerate() {
            let on = matches!((i, &fill), (0, Fill::None) | (1, Fill::Solid(_)) | (2, Fill::Gradient(_)) | (3, Fill::Pattern(_)) | (4, Fill::Mesh(_)));
            if ui.selectable_label(on, *name).clicked() && !on {
                let f = match i {
                    0 => Fill::None,
                    1 => Fill::Solid(base),
                    2 => Fill::Gradient(Gradient::linear(base, Color::WHITE)),
                    3 => Fill::Pattern(PatternFill { kind: PatternKind::ALL[0], fg: base, bg: Color::WHITE, tile: 18.0, angle: 0.0, image: None, transform_with_object: true }),
                    _ => Fill::Mesh(vector_core::style::MeshFill::new(3, 3, base)),
                };
                edit_sel(app, "Fill", move |x| x.style.fill = f.clone());
            }
        }
    });
    match fill {
        Fill::None => hint(ui, "No fill"),
        Fill::Solid(mut c) => {
            let mut c32 = theme::to_c32(c);
            if egui::color_picker::color_picker_color32(ui, &mut c32, egui::color_picker::Alpha::Opaque) {
                c = theme::from_c32(c32);
                set_fill_coalesced(app, Fill::Solid(c));
            }
            hex_row(app, ui, c, false);
        }
        Fill::Gradient(mut g) => {
            if gradient_editor(ui, &mut g) {
                set_fill_coalesced(app, Fill::Gradient(g));
            }
        }
        Fill::Pattern(mut p) => {
            let mut ch = false;
            egui::ComboBox::from_id_salt("dk_pat").selected_text(p.kind.name()).show_ui(ui, |ui| {
                for k in PatternKind::ALL {
                    ch |= ui.selectable_value(&mut p.kind, k, k.name()).changed();
                }
            });
            ui.horizontal(|ui| {
                ui.label("Foreground");
                ch |= color_edit(ui, &mut p.fg, false);
                ui.label("Background");
                ch |= color_edit(ui, &mut p.bg, false);
            });
            ch |= ui.add(egui::Slider::new(&mut p.tile, 2.0..=200.0).text("Tile size (pt)")).changed();
            ch |= ui.add(egui::Slider::new(&mut p.angle, -180.0..=180.0).text("Angle")).changed();
            ch |= ui.checkbox(&mut p.transform_with_object, "Transform with object").changed();
            if ch {
                set_fill_coalesced(app, Fill::Pattern(p));
            }
        }
        Fill::Mesh(mut m) => {
            hint(ui, "Use the Mesh Fill tool to edit individual patches.");
            let mut ch = false;
            ui.horizontal_wrapped(|ui| {
                for c in m.colors.iter_mut() {
                    ch |= color_edit(ui, c, false);
                }
            });
            if ch {
                set_fill_coalesced(app, Fill::Mesh(m));
            }
        }
    }
    let mut eo = o.style.even_odd;
    if ui.checkbox(&mut eo, "Even-odd fill (holes in combined curves)").changed() {
        edit_sel(app, "Fill Rule", |x| x.style.even_odd = eo);
    }
    let mut op = o.style.overprint_fill;
    if ui.checkbox(&mut op, "Overprint fill").changed() {
        edit_sel(app, "Overprint", |x| x.style.overprint_fill = op);
    }
}

/// Continuous edits (dragging in a color picker) are merged into one undo step per second.
fn set_fill_coalesced(app: &mut VectorApp, f: Fill) {
    let now = app.time();
    let merge = now - app.ts.last_coalesce < 1.0;
    app.ts.last_coalesce = now;
    let Some(d) = app.doc_mut() else { return };
    let sel = d.selection.clone();
    if !merge {
        d.checkpoint("Fill");
    }
    for id in sel {
        if let Some(o) = d.doc.find_mut(id) {
            o.style.fill = f.clone();
        }
    }
    d.history.touch();
}

fn hex_row(app: &mut VectorApp, ui: &mut Ui, c: Color, outline: bool) {
    let id = ui.scope_id().with(("hex", outline));
    let mut s: String = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| c.to_hex());
    ui.horizontal(|ui| {
        ui.label("Hex:");
        let r = ui.add(egui::TextEdit::singleline(&mut s).desired_width(90.0));
        if r.has_focus() {
            ui.data_mut(|d| d.insert_temp(id, s.clone()));
        } else {
            ui.data_mut(|d| d.remove::<String>(id));
        }
        if r.lost_focus() {
            if let Some(nc) = Color::from_hex(&s) {
                if outline {
                    edit_sel(app, "Outline Color", |x| x.style.outline.color = Some(nc));
                } else {
                    edit_sel(app, "Fill", |x| x.style.fill = Fill::Solid(nc));
                }
            }
        }
        let [cc, m, y, k] = c.to_cmyk();
        hint(ui, &format!("C{cc:.0} M{m:.0} Y{y:.0} K{k:.0}"));
    });
}

/// Gradient editor: preview ramp, type, stops with colors/positions, angle via start/end.
pub fn gradient_editor(ui: &mut Ui, g: &mut Gradient) -> bool {
    let mut ch = false;
    ui.horizontal(|ui| {
        for (name, k) in [("Linear", GradientKind::Linear), ("Elliptical", GradientKind::Elliptical), ("Conical", GradientKind::Conical), ("Rectangular", GradientKind::Rectangular)] {
            ch |= ui.selectable_value(&mut g.kind, k, name).changed();
        }
    });
    // ramp preview with draggable stops
    let w = ui.available_width().min(320.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 22.0), egui::Sense::hover());
    let p = ui.painter();
    crate::widgets::checker(p, rect, 5.0);
    let n = 64;
    for i in 0..n {
        let t = i as f64 / (n - 1) as f64;
        let x0 = rect.left() + rect.width() * i as f32 / n as f32;
        let x1 = rect.left() + rect.width() * (i + 1) as f32 / n as f32 + 0.5;
        p.rect_filled(egui::Rect::from_min_max(egui::pos2(x0, rect.top()), egui::pos2(x1, rect.bottom())), 0.0, theme::to_c32(g.color_at(t)));
    }
    let mut remove = None;
    let len = g.stops.len();
    for (i, s) in g.stops.iter_mut().enumerate() {
        let x = rect.left() + rect.width() * s.pos as f32;
        let hr = egui::Rect::from_center_size(egui::pos2(x, rect.bottom() + 6.0), egui::vec2(10.0, 12.0));
        let r = ui.interact(hr, ui.scope_id().with(("stop", i)), egui::Sense::click_and_drag());
        ui.painter().add(egui::Shape::convex_polygon(vec![egui::pos2(x, rect.bottom()), egui::pos2(x + 5.0, rect.bottom() + 7.0), egui::pos2(x - 5.0, rect.bottom() + 7.0)], theme::to_c32(s.color), egui::Stroke::new(1.0, if r.hovered() { egui::Color32::WHITE } else { theme::TEXT_DIM })));
        if r.dragged() {
            s.pos = (((r.interact_pointer_pos().unwrap().x - rect.left()) / rect.width()) as f64).clamp(0.0, 1.0);
            ch = true;
        }
        if r.secondary_clicked() && len > 2 {
            remove = Some(i);
        }
    }
    ui.add_space(10.0);
    if let Some(i) = remove {
        g.stops.remove(i);
        ch = true;
    }
    let mut stops = g.sorted_stops();
    for (i, s) in stops.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ch |= color_edit(ui, &mut s.color, true);
            ch |= ui.add(egui::Slider::new(&mut s.pos, 0.0..=1.0).text(format!("Stop {}", i + 1))).changed();
        });
    }
    if ch {
        g.stops = stops;
    }
    ui.horizontal(|ui| {
        if ui.button("Add stop").clicked() {
            let c = g.color_at(0.5);
            g.stops.push(GradientStop { pos: 0.5, color: c });
            ch = true;
        }
        if ui.button("Reverse").clicked() {
            for s in g.stops.iter_mut() {
                s.pos = 1.0 - s.pos;
            }
            ch = true;
        }
        egui::ComboBox::from_id_salt("spread").selected_text(format!("{:?}", g.spread)).width(80.0).show_ui(ui, |ui| {
            for m in [SpreadMode::Pad, SpreadMode::Repeat, SpreadMode::Reflect] {
                ch |= ui.selectable_value(&mut g.spread, m, format!("{m:?}")).changed();
            }
        });
    });
    // angle (start/end are in the unit bbox)
    let v = g.end - g.start;
    let mut ang = (-v.y).atan2(v.x).to_degrees();
    if ui.add(egui::Slider::new(&mut ang, -180.0..=180.0).text("Angle")).changed() {
        let r = (v.x * v.x + v.y * v.y).sqrt().max(0.05) / 2.0;
        let c = kurbo::Point::new((g.start.x + g.end.x) / 2.0, (g.start.y + g.end.y) / 2.0);
        let (s, co) = ang.to_radians().sin_cos();
        g.start = kurbo::Point::new(c.x - co * r, c.y + s * r);
        g.end = kurbo::Point::new(c.x + co * r, c.y - s * r);
        ch = true;
    }
    let mut steps = g.steps as f64;
    if ui.add(egui::Slider::new(&mut steps, 0.0..=256.0).text("Steps (0 = smooth)")).changed() {
        g.steps = steps as u32;
        ch = true;
    }
    ch
}

fn outline_section(app: &mut VectorApp, ui: &mut Ui, o: &Object) {
    heading(ui, "Outline");
    let ol = o.style.outline.clone();
    let mut w = ol.width;
    ui.horizontal(|ui| {
        ui.label("Width:");
        if super::len_field(app, ui, "ol_w", &mut w) {
            edit_sel(app, "Outline Width", |x| {
                x.style.outline.width = w.max(0.0);
                if x.style.outline.color.is_none() && w > 0.0 {
                    x.style.outline.color = Some(Color::BLACK);
                }
            });
        }
    });
    ui.horizontal(|ui| {
        ui.label("Color:");
        let mut c = ol.color.unwrap_or(Color::BLACK);
        if color_edit(ui, &mut c, true) {
            edit_sel(app, "Outline Color", |x| x.style.outline.color = Some(c));
        }
        if ui.button("None").clicked() {
            edit_sel(app, "Outline Color", |x| x.style.outline.color = None);
        }
    });
    if let Some(c) = ol.color {
        hex_row(app, ui, c, true);
    }
    let presets = dash_presets();
    let cur = presets.iter().find(|(_, d)| *d == ol.dash).map(|p| p.0).unwrap_or("Custom");
    egui::ComboBox::from_id_salt("dk_dash").selected_text(cur).width(140.0).show_ui(ui, |ui| {
        for (name, d) in &presets {
            if ui.selectable_label(*d == ol.dash, *name).clicked() {
                let d = d.clone();
                edit_sel(app, "Line Style", move |x| x.style.outline.dash = d.clone());
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Corners:");
        for (n, j) in [("Miter", LineJoin::Miter), ("Round", LineJoin::Round), ("Bevel", LineJoin::Bevel)] {
            if ui.selectable_label(ol.join == j, n).clicked() {
                edit_sel(app, "Corners", |x| x.style.outline.join = j);
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Caps:");
        for (n, c) in [("Butt", LineCap::Butt), ("Round", LineCap::Round), ("Square", LineCap::Square)] {
            if ui.selectable_label(ol.cap == c, n).clicked() {
                edit_sel(app, "Line Caps", |x| x.style.outline.cap = c);
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Position:");
        for (n, p) in [("Outside", OutlinePosition::Outside), ("Center", OutlinePosition::Center), ("Inside", OutlinePosition::Inside)] {
            if ui.selectable_label(ol.position == p, n).clicked() {
                edit_sel(app, "Outline Position", |x| x.style.outline.position = p);
            }
        }
    });
    ui.horizontal(|ui| {
        for (label, start) in [("Start arrow", true), ("End arrow", false)] {
            let cur = if start { ol.start_arrow } else { ol.end_arrow };
            egui::ComboBox::from_id_salt(("arrow", start)).selected_text(cur.map(|a| a.name()).unwrap_or(label)).width(100.0).show_ui(ui, |ui| {
                if ui.selectable_label(cur.is_none(), "None").clicked() {
                    edit_sel(app, "Arrowhead", |x| if start { x.style.outline.start_arrow = None } else { x.style.outline.end_arrow = None });
                }
                for a in ArrowHead::ALL {
                    if ui.selectable_label(cur == Some(a), a.name()).clicked() {
                        edit_sel(app, "Arrowhead", |x| if start { x.style.outline.start_arrow = Some(a) } else { x.style.outline.end_arrow = Some(a) });
                    }
                }
            });
        }
    });
    let mut behind = ol.behind_fill;
    let mut scale = ol.scale_with_object;
    ui.horizontal(|ui| {
        if ui.checkbox(&mut behind, "Behind fill").changed() {
            edit_sel(app, "Behind Fill", |x| x.style.outline.behind_fill = behind);
        }
        if ui.checkbox(&mut scale, "Scale with object").changed() {
            edit_sel(app, "Scale Outline", |x| x.style.outline.scale_with_object = scale);
        }
    });
    let mut ml = ol.miter_limit;
    if ui.add(egui::Slider::new(&mut ml, 1.0..=50.0).text("Miter limit")).changed() {
        edit_sel(app, "Miter Limit", |x| x.style.outline.miter_limit = ml);
    }
}

fn transparency_section(app: &mut VectorApp, ui: &mut Ui, o: &Object) {
    heading(ui, "Transparency");
    let t = o.style.transparency.clone();
    ui.horizontal(|ui| {
        for (i, n) in ["None", "Uniform", "Fountain"].iter().enumerate() {
            let on = matches!((i, &t), (0, Transparency::None) | (1, Transparency::Uniform(_)) | (2, Transparency::Gradient(_)));
            if ui.selectable_label(on, *n).clicked() && !on {
                let nt = match i {
                    0 => Transparency::None,
                    1 => Transparency::Uniform(50.0),
                    _ => Transparency::Gradient(Gradient::linear(Color::WHITE, Color::BLACK)),
                };
                edit_sel(app, "Transparency", move |x| x.style.transparency = nt.clone());
            }
        }
    });
    match t {
        Transparency::Uniform(mut v) => {
            if ui.add(egui::Slider::new(&mut v, 0.0..=100.0).text("Transparency %")).changed() {
                edit_sel(app, "Transparency", |x| x.style.transparency = Transparency::Uniform(v));
            }
        }
        Transparency::Gradient(mut g) => {
            hint(ui, "White = opaque, black = transparent");
            if gradient_editor(ui, &mut g) {
                edit_sel(app, "Transparency", |x| x.style.transparency = Transparency::Gradient(g.clone()));
            }
        }
        Transparency::None => {}
    }
    let merge = o.style.merge;
    egui::ComboBox::from_id_salt("dk_merge").selected_text(format!("Merge: {}", merge.name())).width(180.0).show_ui(ui, |ui| {
        for m in MergeMode::ALL {
            if ui.selectable_label(m == merge, m.name()).clicked() {
                edit_sel(app, "Merge Mode", |x| x.style.merge = m);
            }
        }
    });
}
