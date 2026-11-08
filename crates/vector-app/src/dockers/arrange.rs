//! Transform, Coordinates, Live Guides, Guidelines, Align and Distribute, Fit Objects to Path,
//! Step and Repeat, Shape (shaping), Corners and Join Curves dockers.

use super::{heading, hint, len_field, no_doc, primary};
use crate::app::VectorApp;
use crate::commands::Cmd;
use egui::Ui;
use kurbo::{Affine, Point, Rect, Vec2};
use vector_core::ops::{self, Align, AlignTo, Distribute, Shaping};
use vector_core::{Guide, GuideKind, ObjKind};

fn anchor_point(bb: Rect, anchor: usize) -> Point {
    let fx = (anchor % 3) as f64 / 2.0;
    let fy = (anchor / 3) as f64 / 2.0;
    Point::new(bb.x0 + bb.width() * fx, bb.y0 + bb.height() * fy)
}

/// 3×3 anchor grid selector.
fn anchor_grid(ui: &mut Ui, anchor: &mut usize) {
    egui::Grid::new(ui.scope_id().with("anchor")).spacing(egui::vec2(2.0, 2.0)).show(ui, |ui| {
        for r in 0..3 {
            for c in 0..3 {
                let i = r * 3 + c;
                if ui.radio(*anchor == i, "").clicked() {
                    *anchor = i;
                }
            }
            ui.end_row();
        }
    });
}

pub fn transform(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    ui.horizontal(|ui| {
        for (i, (icon, tip)) in [("nudge", "Position"), ("rotate", "Rotate"), ("mirror_h", "Scale and Mirror"), ("page_width", "Size"), ("transform", "Skew")].iter().enumerate() {
            let on = app.dockers.ui.transform_tab == i;
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, on).clicked() {
                app.dockers.ui.transform_tab = i;
            }
        }
    });
    let bb = app.doc().and_then(|d| d.selection_bounds());
    let has = bb.is_some();
    let mut u = std::mem::take(&mut app.dockers.ui);
    match u.transform_tab {
        0 => {
            heading(ui, "Position");
            ui.horizontal(|ui| {
                ui.label("X:");
                len_field(app, ui, "t_px", &mut u.t_pos[0]);
            });
            ui.horizontal(|ui| {
                ui.label("Y:");
                len_field(app, ui, "t_py", &mut u.t_pos[1]);
            });
            ui.checkbox(&mut u.t_relative, "Relative position");
        }
        1 => {
            heading(ui, "Rotate");
            ui.add(egui::DragValue::new(&mut u.t_rot).suffix(" °").speed(0.5));
            ui.horizontal(|ui| {
                for a in [-90.0, -45.0, 45.0, 90.0, 180.0] {
                    if ui.small_button(format!("{a}°")).clicked() {
                        u.t_rot = a;
                    }
                }
            });
        }
        2 => {
            heading(ui, "Scale and Mirror");
            ui.horizontal(|ui| {
                ui.label("H:");
                ui.add(egui::DragValue::new(&mut u.t_scale[0]).suffix(" %").speed(0.5));
                ui.label("V:");
                ui.add(egui::DragValue::new(&mut u.t_scale[1]).suffix(" %").speed(0.5));
            });
            ui.horizontal(|ui| {
                if ui.button("Mirror horizontally").clicked() {
                    u.t_scale[0] = -u.t_scale[0];
                }
                if ui.button("Mirror vertically").clicked() {
                    u.t_scale[1] = -u.t_scale[1];
                }
            });
            ui.checkbox(&mut u.t_proportional, "Proportional");
            if u.t_proportional {
                u.t_scale[1] = u.t_scale[1].signum() * u.t_scale[0].abs();
            }
        }
        3 => {
            heading(ui, "Size");
            if let (Some(b), true) = (bb, ui.button("Use selection size").clicked()) {
                u.t_size = [b.width(), b.height()];
            }
            ui.horizontal(|ui| {
                ui.label("W:");
                len_field(app, ui, "t_w", &mut u.t_size[0]);
            });
            ui.horizontal(|ui| {
                ui.label("H:");
                len_field(app, ui, "t_h", &mut u.t_size[1]);
            });
            ui.checkbox(&mut u.t_proportional, "Proportional");
        }
        _ => {
            heading(ui, "Skew");
            ui.horizontal(|ui| {
                ui.label("X:");
                ui.add(egui::DragValue::new(&mut u.t_skew[0]).suffix(" °").range(-89.0..=89.0));
                ui.label("Y:");
                ui.add(egui::DragValue::new(&mut u.t_skew[1]).suffix(" °").range(-89.0..=89.0));
            });
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label("Anchor:");
            anchor_grid(ui, &mut u.t_anchor);
        });
        ui.vertical(|ui| {
            ui.label("Copies:");
            ui.add(egui::DragValue::new(&mut u.t_copies).range(0..=999));
        });
    });
    let apply = primary(ui, "Apply", has);
    let tab = u.transform_tab;
    let copies = u.t_copies;
    let params = (u.t_pos, u.t_relative, u.t_rot, u.t_scale, u.t_size, u.t_proportional, u.t_skew, u.t_anchor);
    app.dockers.ui = u;
    if apply {
        let Some(bb) = bb else { return };
        let (pos, rel, rot, scale, size, prop, skew, anchor) = params;
        let a = anchor_point(bb, anchor);
        let m = match tab {
            0 => {
                if rel {
                    Affine::translate((pos[0], pos[1]))
                } else {
                    let o = app.doc().map(|d| d.doc.settings.ruler_origin).unwrap_or_default();
                    Affine::translate((pos[0] + o.x - a.x, pos[1] + o.y - a.y))
                }
            }
            1 => Affine::translate(a.to_vec2()) * Affine::rotate(-rot.to_radians()) * Affine::translate(-a.to_vec2()),
            2 => Affine::translate(a.to_vec2()) * Affine::scale_non_uniform(scale[0] / 100.0, scale[1] / 100.0) * Affine::translate(-a.to_vec2()),
            3 => {
                let sx = size[0] / bb.width().max(1e-9);
                let sy = if prop { sx } else { size[1] / bb.height().max(1e-9) };
                Affine::translate(a.to_vec2()) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-a.to_vec2())
            }
            _ => Affine::translate(a.to_vec2()) * Affine::new([1.0, skew[1].to_radians().tan(), skew[0].to_radians().tan(), 1.0, 0.0, 0.0]) * Affine::translate(-a.to_vec2()),
        };
        let Some(d) = app.doc_mut() else { return };
        let sel = d.selection.clone();
        d.modify("Transform", |doc, selection| {
            if copies == 0 {
                doc.transform_objects(&sel, m);
            } else {
                let mut cur = sel.clone();
                for _ in 0..copies {
                    let dup = ops::duplicate(doc, &cur, Vec2::ZERO);
                    doc.transform_objects(&dup, m);
                    cur = dup;
                }
                *selection = cur;
            }
        });
    }
}

pub fn coordinates(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    hint(ui, "Create or position objects with exact coordinates.");
    let mut u = std::mem::take(&mut app.dockers.ui);
    ui.horizontal(|ui| {
        ui.label("X:");
        len_field(app, ui, "c_x", &mut u.t_pos[0]);
        ui.label("Y:");
        len_field(app, ui, "c_y", &mut u.t_pos[1]);
    });
    ui.horizontal(|ui| {
        ui.label("W:");
        len_field(app, ui, "c_w", &mut u.t_size[0]);
        ui.label("H:");
        len_field(app, ui, "c_h", &mut u.t_size[1]);
    });
    let (x, y, w, h) = (u.t_pos[0], u.t_pos[1], u.t_size[0], u.t_size[1]);
    app.dockers.ui = u;
    ui.horizontal(|ui| {
        if ui.button("Create Rectangle").clicked() {
            let _ = crate::scripting::run_script(app, &format!("rect({x}, {y}, {w}, {h});"));
        }
        if ui.button("Create Ellipse").clicked() {
            let _ = crate::scripting::run_script(app, &format!("ellipse({}, {}, {}, {});", x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0));
        }
    });
    if ui.add_enabled(app.has_selection(), egui::Button::new("Apply to selection")).clicked() {
        if let Some(bb) = app.doc().and_then(|d| d.selection_bounds()) {
            let m = Affine::translate((x, y)) * Affine::scale_non_uniform(w / bb.width().max(1e-9), h / bb.height().max(1e-9)) * Affine::translate((-bb.x0, -bb.y0));
            crate::propbar::transform_sel(app, "Coordinates", m);
        }
    }
}

pub fn live_guides(app: &mut VectorApp, ui: &mut Ui) {
    hint(ui, "Live guides show temporary alignment lines to nearby objects while you draw and move.");
    ui.checkbox(&mut app.settings.live_guides, "Enable live guides");
    ui.add_enabled_ui(app.settings.live_guides, |ui| {
        ui.checkbox(&mut app.settings.live_guides_edges, "Edges");
        ui.checkbox(&mut app.settings.live_guides_centers, "Centers");
        ui.checkbox(&mut app.settings.live_guides_page, "Page");
    });
}

pub fn guidelines(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let mut u = std::mem::take(&mut app.dockers.ui);
    ui.horizontal(|ui| {
        ui.radio_value(&mut u.guide_vertical, false, "Horizontal");
        ui.radio_value(&mut u.guide_vertical, true, "Vertical");
    });
    ui.horizontal(|ui| {
        ui.label("Position:");
        len_field(app, ui, "g_pos", &mut u.guide_pos);
    });
    let (vert, pos) = (u.guide_vertical, u.guide_pos);
    app.dockers.ui = u;
    if primary(ui, "Add", true) {
        if let Some(d) = app.doc_mut() {
            let pi = d.page;
            d.modify("Add Guideline", |doc, _| {
                let o = doc.settings.ruler_origin;
                let g = Guide { kind: if vert { GuideKind::Vertical } else { GuideKind::Horizontal }, pos: if vert { Point::new(pos + o.x, 0.0) } else { Point::new(0.0, pos + o.y) }, locked: false, color: vector_core::Color::from_hex("#0099ff").unwrap() };
                doc.pages[pi].guides.push(g);
            });
        }
    }
    heading(ui, "Guidelines on this page");
    let Some(d) = app.doc() else { return };
    let guides = d.page_ref().guides.clone();
    let (unit, dpi) = (d.doc.settings.unit, d.doc.settings.dpi);
    let mut del = None;
    for (i, g) in guides.iter().enumerate() {
        ui.horizontal(|ui| {
            let (k, v) = match g.kind {
                GuideKind::Vertical => ("Vertical", g.pos.x),
                GuideKind::Horizontal => ("Horizontal", g.pos.y),
                _ => ("Angled", g.pos.x),
            };
            ui.label(format!("{k}: {}", unit.format(v, dpi)));
            if ui.small_button("Delete").clicked() {
                del = Some(i);
            }
        });
    }
    if guides.is_empty() {
        hint(ui, "Drag from the rulers to create guidelines.");
    }
    if let Some(i) = del {
        if let Some(d) = app.doc_mut() {
            let pi = d.page;
            d.modify("Delete Guideline", |doc, _| {
                doc.pages[pi].guides.remove(i);
            });
        }
    }
    if !guides.is_empty() && ui.button("Delete all").clicked() {
        if let Some(d) = app.doc_mut() {
            let pi = d.page;
            d.modify("Delete Guidelines", |doc, _| doc.pages[pi].guides.clear());
        }
    }
}

pub fn align(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let n = app.doc().map(|d| d.selection.len()).unwrap_or(0);
    heading(ui, "Align");
    let to_names = ["Last selected", "First selected", "Selection", "Page", "Page edge"];
    egui::ComboBox::from_id_salt("align_to").selected_text(format!("Align to: {}", to_names[app.dockers.ui.align_to])).width(200.0).show_ui(ui, |ui| {
        for (i, nm) in to_names.iter().enumerate() {
            ui.selectable_value(&mut app.dockers.ui.align_to, i, *nm);
        }
    });
    let to = match app.dockers.ui.align_to {
        0 => AlignTo::LastSelected,
        1 => AlignTo::FirstSelected,
        2 => AlignTo::Selection,
        3 => AlignTo::Page,
        _ => AlignTo::PageEdge,
    };
    let en = n > 1 || (n == 1 && matches!(to, AlignTo::Page | AlignTo::PageEdge));
    let mut clicked = None;
    ui.horizontal(|ui| {
        for (icon, tip, a) in [
            ("align_left", "Align left (L)", Align::Left),
            ("align_hcenter", "Align centers horizontally (E)", Align::CenterH),
            ("align_right", "Align right (R)", Align::Right),
            ("align_top", "Align top (T)", Align::Top),
            ("align_vcenter", "Align centers vertically (C)", Align::CenterV),
            ("align_bottom", "Align bottom (B)", Align::Bottom),
        ] {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, en, false).clicked() {
                clicked = Some(a);
            }
        }
    });
    ui.checkbox(&mut app.dockers.ui.use_outline, "Use outline (visual bounds)");
    if let Some(a) = clicked {
        let outline = app.dockers.ui.use_outline;
        if let Some(d) = app.doc_mut() {
            let sel = d.selection.clone();
            let page = d.page;
            d.modify("Align", |doc, _| ops::align(doc, &sel, a, to, page, outline));
        }
    }
    heading(ui, "Distribute");
    let mut dclick = None;
    ui.horizontal_wrapped(|ui| {
        for (icon, tip, dd) in [
            ("align_left", "Distribute left", Distribute::Left),
            ("align_hcenter", "Distribute centers horizontally", Distribute::CenterH),
            ("tile_h", "Distribute spacing horizontally", Distribute::SpacingH),
            ("align_right", "Distribute right", Distribute::Right),
            ("align_top", "Distribute top", Distribute::Top),
            ("align_vcenter", "Distribute centers vertically", Distribute::CenterV),
            ("tile_v", "Distribute spacing vertically", Distribute::SpacingV),
            ("align_bottom", "Distribute bottom", Distribute::Bottom),
        ] {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, n > 2, false).clicked() {
                dclick = Some(dd);
            }
        }
    });
    if let Some(dd) = dclick {
        let to_page = matches!(to, AlignTo::Page | AlignTo::PageEdge);
        if let Some(d) = app.doc_mut() {
            let sel = d.selection.clone();
            let ext = if to_page { Some(d.page_ref().rect()) } else { None };
            d.modify("Distribute", |doc, _| ops::distribute(doc, &sel, dd, ext));
        }
    }
}

pub fn fit_to_path(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    hint(ui, "Select the objects to distribute, then the path last (Shift+click).");
    ui.horizontal(|ui| {
        ui.label("Offset from start:");
        let mut off = app.dockers.ui.fit_offset;
        len_field(app, ui, "fit_off", &mut off);
        app.dockers.ui.fit_offset = off;
    });
    ui.checkbox(&mut app.dockers.ui.fit_rotate, "Rotate objects to follow the path");
    let n = app.doc().map(|d| d.selection.len()).unwrap_or(0);
    if primary(ui, "Apply", n >= 2) {
        let (off, rot) = (app.dockers.ui.fit_offset, app.dockers.ui.fit_rotate);
        let Some(d) = app.doc_mut() else { return };
        let mut sel = d.selection.clone();
        let path_id = sel.pop().unwrap();
        let Some(path) = d.doc.find(path_id).map(|o| o.world_path()) else { return };
        // single text object: fit text to path
        if sel.len() == 1 && matches!(d.doc.find(sel[0]).map(|o| &o.kind), Some(ObjKind::Text(_))) {
            let t = sel[0];
            d.modify("Fit Text to Path", |doc, _| {
                ops::fit_text_to_path(doc, t, path_id);
            });
            return;
        }
        let len = vector_core::path::bez_length(&path);
        let count = sel.len().max(1);
        d.modify("Fit Objects to Path", |doc, _| {
            for (i, id) in sel.iter().enumerate() {
                let s = off + (len - off).max(0.0) * if count == 1 { 0.0 } else { i as f64 / (count - 1) as f64 };
                if let Some((pt, tan)) = vector_core::path::point_at_length(&path, s.min(len)) {
                    let Some(o) = doc.find(*id) else { continue };
                    let c = o.bounds().center();
                    let mut m = Affine::translate(pt - c);
                    if rot {
                        m = Affine::translate(pt.to_vec2()) * Affine::rotate(tan.y.atan2(tan.x)) * Affine::translate(-pt.to_vec2()) * m;
                    }
                    doc.transform_objects(&[*id], m);
                }
            }
        });
    }
}

pub fn step_repeat(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let mut u = std::mem::take(&mut app.dockers.ui);
    heading(ui, "Horizontal settings");
    ui.horizontal(|ui| {
        ui.label("Distance:");
        len_field(app, ui, "sr_x", &mut u.step_offset[0]);
    });
    heading(ui, "Vertical settings");
    ui.horizontal(|ui| {
        ui.label("Distance:");
        len_field(app, ui, "sr_y", &mut u.step_offset[1]);
    });
    ui.horizontal(|ui| {
        ui.label("Number of copies:");
        ui.add(egui::DragValue::new(&mut u.step_copies).range(1..=999));
    });
    let (c, o) = (u.step_copies, u.step_offset);
    app.dockers.ui = u;
    if primary(ui, "Apply", app.has_selection()) {
        if let Some(d) = app.doc_mut() {
            let sel = d.selection.clone();
            d.modify("Step and Repeat", |doc, selection| {
                let new = ops::step_and_repeat(doc, &sel, c, Vec2::new(o[0], o[1]));
                if !new.is_empty() {
                    *selection = new;
                }
            });
        }
    }
}

pub fn shaping(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let n = app.doc().map(|d| d.selection.len()).unwrap_or(0);
    let ops_list = [
        ("weld", "Weld", Shaping::Weld, "Combine the selected objects into a single shape."),
        ("trim", "Trim", Shaping::Trim, "Remove the overlapping areas of the last-selected object."),
        ("intersect", "Intersect", Shaping::Intersect, "Create an object from the area where objects overlap."),
        ("simplify", "Simplify", Shaping::Simplify, "Trim the hidden areas of overlapping objects."),
        ("front_minus_back", "Front minus back", Shaping::FrontMinusBack, "Remove the back objects from the front object."),
        ("back_minus_front", "Back minus front", Shaping::BackMinusFront, "Remove the front objects from the back object."),
        ("boundary", "Boundary", Shaping::Boundary, "Create an outline around the selected objects."),
    ];
    let cur = app.ts.shaping_op.min(ops_list.len() - 1);
    egui::ComboBox::from_id_salt("shaping").selected_text(ops_list[cur].1).width(200.0).show_ui(ui, |ui| {
        for (i, o) in ops_list.iter().enumerate() {
            ui.selectable_value(&mut app.ts.shaping_op, i, o.1);
        }
    });
    hint(ui, ops_list[cur].3);
    ui.checkbox(&mut app.ts.leave_source, "Leave original: source objects");
    ui.checkbox(&mut app.ts.leave_target, "Leave original: target objects");
    let need = if matches!(ops_list[cur].2, Shaping::Boundary) { 1 } else { 2 };
    if primary(ui, ops_list[cur].1, n >= need) {
        let (op, ls, lt) = (ops_list[cur].2, app.ts.leave_source, app.ts.leave_target);
        if let Some(d) = app.doc_mut() {
            let sel = d.selection.clone();
            d.modify(ops_list[cur].1, |doc, selection| {
                let r = ops::shaping(doc, &sel, op, ls, lt);
                if !r.is_empty() {
                    *selection = r;
                }
            });
        }
    }
}

pub fn corners(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let names = ["Fillet", "Scallop", "Chamfer"];
    ui.horizontal(|ui| {
        for (i, nm) in names.iter().enumerate() {
            ui.radio_value(&mut app.dockers.ui.corner_kind, i, *nm);
        }
    });
    let mut r = app.dockers.ui.corner_radius;
    ui.horizontal(|ui| {
        ui.label("Radius:");
        len_field(app, ui, "corner_r", &mut r);
    });
    app.dockers.ui.corner_radius = r.max(0.0);
    hint(ui, "Applies to rectangles and to sharp corners of curves.");
    if primary(ui, "Apply", app.has_selection()) {
        let kind = app.dockers.ui.corner_kind;
        let r = app.dockers.ui.corner_radius;
        crate::propbar::edit_sel(app, names[kind], |o| {
            use vector_core::object::CornerType;
            match &mut o.kind {
                ObjKind::Rect(rs) => {
                    rs.radii = [r; 4];
                    rs.corner = [CornerType::Round, CornerType::Scallop, CornerType::Chamfer][kind];
                }
                ObjKind::Path(p) => {
                    *p = crate::tools::round_corners(p, r, kind);
                }
                _ => {}
            }
        });
    }
}

pub fn join_curves(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    hint(ui, "Join the end nodes of the selected curves that are within the gap tolerance.");
    let mut t = app.dockers.ui.join_tolerance;
    ui.horizontal(|ui| {
        ui.label("Gap tolerance:");
        len_field(app, ui, "join_t", &mut t);
    });
    app.dockers.ui.join_tolerance = t.max(0.0);
    if primary(ui, "Apply", app.has_selection()) {
        let tol = app.dockers.ui.join_tolerance;
        if let Some(d) = app.doc_mut() {
            let sel = d.selection.clone();
            d.modify("Join Curves", |doc, selection| {
                if let Some(id) = ops::join_curves(doc, &sel, tol) {
                    *selection = vec![id];
                }
            });
        }
    }
    let _ = Cmd::JoinCurves;
}
