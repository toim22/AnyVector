//! Transform, Coordinates, Live Guides, Guidelines, Align and Distribute, Fit Objects to Path,
//! Step and Repeat, Shape (shaping), Corners and Join Curves dockers.

use super::{apply_bar, bold_title, form_row, icon_button, no_doc, section, separator, spin};
use crate::theme;
use crate::app::AnyVectorApp;
use egui::Ui;
use kurbo::{Affine, Point, Rect, Vec2};
use vector_core::ops::{self, Align, AlignTo, Distribute, Shaping, TextSource};
use vector_core::ObjKind;

fn anchor_point(bb: Rect, anchor: usize) -> Point {
    let fx = (anchor % 3) as f64 / 2.0;
    let fy = (anchor / 3) as f64 / 2.0;
    Point::new(bb.x0 + bb.width() * fx, bb.y0 + bb.height() * fy)
}

/// The object origin grid of the Transform docker: nine boxes joined by lines, the chosen one filled.
pub(crate) fn origin_grid(ui: &mut Ui, anchor: &mut usize, enabled: bool) {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(42.0, 38.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let p = ui.painter();
    let col = if enabled { theme::TEXT() } else { theme::DISABLED() };
    let at = |i: usize| egui::pos2(r.left() + 7.0 + (i % 3) as f32 * 14.0, r.top() + 6.0 + (i / 3) as f32 * 13.0);
    for k in 0..3 {
        p.line_segment([at(k * 3), at(k * 3 + 2)], egui::Stroke::new(1.0, col));
        p.line_segment([at(k), at(k + 6)], egui::Stroke::new(1.0, col));
    }
    for i in 0..9 {
        let b = egui::Rect::from_center_size(at(i), egui::vec2(7.0, 7.0));
        p.rect_filled(b, 0.0, theme::BG());
        if i == *anchor {
            p.rect_filled(b, 0.0, col);
        } else {
            p.rect_stroke(b, 0.0, egui::Stroke::new(1.0, col), egui::StrokeKind::Inside);
        }
    }
    if resp.clicked() {
        if let Some(q) = resp.interact_pointer_pos() {
            *anchor = (0..9).min_by(|a, b| at(*a).distance(q).total_cmp(&at(*b).distance(q))).unwrap_or(4);
        }
    }
}

/// A length field of the Transform docker.
fn t_len(app: &AnyVectorApp, ui: &mut Ui, id: &str, v: &mut f64) -> bool {
    let (u, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    crate::widgets::unit_field(ui, egui::Id::unique(("tr", id)), v, u, dpi, 80.0, true)
}

const TRANSFORM_TABS: [(&str, &str); 5] = [("tr_move", "Position"), ("rotate", "Rotate"), ("tr_scale_mirror", "Scale and Mirror"), ("tr_size", "Size"), ("tr_skew", "Skew")];

pub fn transform(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (i, (icon, tip)) in TRANSFORM_TABS.iter().enumerate() {
            let on = app.dockers.ui.transform_tab == i;
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, on).clicked() {
                app.dockers.ui.transform_tab = i;
            }
        }
    });
    let bb = app.doc().and_then(|d| d.selection_bounds());
    let has = bb.is_some();
    let origin = app.doc().map(|d| d.doc.settings.ruler_origin).unwrap_or_default();
    // a length measured on the canvas for Size ▸ Scale portion
    if let (Some((p0, p1)), Some(b), 1) = (app.ts.picked_segment, bb, app.dockers.ui.pick_slot) {
        app.ts.picked_segment = None;
        let k = app.dockers.ui.t_portion / (p1 - p0).hypot().max(1e-9);
        app.dockers.ui.t_size = [b.width() * k, b.height() * k];
    }
    let mut u = std::mem::take(&mut app.dockers.ui);
    // the fields follow the selection (and the origin and relative settings)
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        app.doc().map(|d| d.selection.clone()).hash(&mut h);
        bb.map(|b| [b.x0, b.y0, b.x1, b.y1].map(|v| (v * 100.0).round() as i64)).hash(&mut h);
        (u.t_anchor, u.t_relative, u.t_rel_center).hash(&mut h);
        h.finish()
    };
    if key != u.t_key {
        u.t_key = key;
        if let Some(b) = bb {
            let a = anchor_point(b, u.t_anchor);
            u.t_pos = if u.t_relative { [0.0, 0.0] } else { [a.x - origin.x, a.y - origin.y] };
            let c = b.center();
            u.t_center = if u.t_rel_center { [0.0, 0.0] } else { [c.x - origin.x, c.y - origin.y] };
            u.t_size = [b.width(), b.height()];
        }
    }
    let tab = u.transform_tab;
    ui.add_space(6.0);
    super::bold_title(ui, TRANSFORM_TABS[tab].1);
    ui.add_space(6.0);
    let lw = 44.0;
    match tab {
        0 => {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                origin_grid(ui, &mut u.t_anchor, true);
                ui.vertical(|ui| {
                    form_row(ui, lw, "X:", true, |ui| t_len(app, ui, "px", &mut u.t_pos[0]));
                    form_row(ui, lw, "Y:", true, |ui| t_len(app, ui, "py", &mut u.t_pos[1]));
                    form_row(ui, lw, "", true, |ui| ui.checkbox(&mut u.t_relative, "Relative Position"));
                });
            });
        }
        1 => {
            form_row(ui, 86.0, "Angle:", true, |ui| {
                spin(ui, &mut u.t_rot, -360.0..=360.0, 1.0, 1, "", 80.0);
                ui.label("°");
            });
            section(ui, "Center");
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                origin_grid(ui, &mut u.t_anchor, true);
                ui.vertical(|ui| {
                    form_row(ui, lw, "X:", true, |ui| t_len(app, ui, "cx", &mut u.t_center[0]));
                    form_row(ui, lw, "Y:", true, |ui| t_len(app, ui, "cy", &mut u.t_center[1]));
                    form_row(ui, lw, "", true, |ui| ui.checkbox(&mut u.t_rel_center, "Relative Center"));
                });
            });
        }
        2 => {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                origin_grid(ui, &mut u.t_anchor, true);
                ui.vertical(|ui| {
                    let before = u.t_scale[0];
                    form_row(ui, lw, "X:", true, |ui| {
                        spin(ui, &mut u.t_scale[0], -10000.0..=10000.0, 1.0, 1, "", 80.0);
                        ui.label("%");
                    });
                    if u.t_proportional && before != u.t_scale[0] {
                        u.t_scale[1] = u.t_scale[0];
                    }
                    let before = u.t_scale[1];
                    form_row(ui, lw, "Y:", true, |ui| {
                        spin(ui, &mut u.t_scale[1], -10000.0..=10000.0, 1.0, 1, "", 80.0);
                        ui.label("%");
                    });
                    if u.t_proportional && before != u.t_scale[1] {
                        u.t_scale[0] = u.t_scale[1];
                    }
                    form_row(ui, lw, "", true, |ui| ui.checkbox(&mut u.t_proportional, "Proportional"));
                    form_row(ui, lw, "", true, |ui| {
                        for (k, (icon, tip)) in [("mirror_h", "Mirror horizontally"), ("mirror_v", "Mirror vertically")].iter().enumerate() {
                            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, u.t_mirror[k]).clicked() {
                                u.t_mirror[k] = !u.t_mirror[k];
                            }
                        }
                    });
                });
            });
        }
        3 => {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                origin_grid(ui, &mut u.t_anchor, true);
                ui.vertical(|ui| {
                    form_row(ui, lw, "", true, |ui| ui.radio_value(&mut u.t_size_portion, false, "Distance"));
                    let dist = !u.t_size_portion;
                    let w0 = u.t_size[0];
                    form_row(ui, lw, "W:", dist, |ui| t_len(app, ui, "w", &mut u.t_size[0]));
                    if u.t_proportional && w0 != u.t_size[0] {
                        if let Some(b) = bb {
                            u.t_size[1] = u.t_size[0] * b.height() / b.width().max(1e-9);
                        }
                    }
                    let h0 = u.t_size[1];
                    form_row(ui, lw, "H:", dist, |ui| t_len(app, ui, "h", &mut u.t_size[1]));
                    if u.t_proportional && h0 != u.t_size[1] {
                        if let Some(b) = bb {
                            u.t_size[0] = u.t_size[1] * b.width() / b.height().max(1e-9);
                        }
                    }
                    form_row(ui, lw, "", true, |ui| ui.radio_value(&mut u.t_size_portion, true, "Scale portion"));
                    form_row(ui, lw, "", u.t_size_portion, |ui| {
                        t_len(app, ui, "portion", &mut u.t_portion);
                        if crate::widgets::icon_button(&mut app.icons, ui, "parallel_dim", "Click the two ends of the part of the object to measure", has, false).clicked() {
                            app.ts.pick_target = Some(crate::tools::PickTarget::Measure { first: None });
                            app.dockers.ui.pick_slot = 1;
                        }
                    });
                    form_row(ui, lw, "", true, |ui| ui.checkbox(&mut u.t_proportional, "Proportional"));
                });
            });
        }
        _ => {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                origin_grid(ui, &mut u.t_anchor, u.t_use_anchor);
                ui.vertical(|ui| {
                    form_row(ui, lw, "X:", true, |ui| {
                        spin(ui, &mut u.t_skew[0], -89.0..=89.0, 1.0, 1, "", 80.0);
                        ui.label("°");
                    });
                    form_row(ui, lw, "Y:", true, |ui| {
                        spin(ui, &mut u.t_skew[1], -89.0..=89.0, 1.0, 1, "", 80.0);
                        ui.label("°");
                    });
                    form_row(ui, lw, "", true, |ui| ui.checkbox(&mut u.t_use_anchor, "Use anchor point"));
                });
            });
        }
    }
    ui.add_space(6.0);
    form_row(ui, 86.0, "Copies:", true, |ui| spin(ui, &mut u.t_copies, 0..=999, 1.0, 0, "", 80.0));
    ui.add_space(6.0);
    let apply = apply_bar(ui, "Apply", has);
    let copies = u.t_copies;
    let params = (u.t_pos, u.t_relative, u.t_rot, u.t_center, u.t_rel_center, u.t_scale, u.t_mirror, u.t_size, u.t_proportional, u.t_skew, u.t_use_anchor, u.t_anchor);
    app.dockers.ui = u;
    if apply {
        let Some(bb) = bb else { return };
        let (pos, rel, rot, center, rel_center, scale, mirror, size, prop, skew, use_anchor, anchor) = params;
        let a = anchor_point(bb, anchor);
        let around = |p: Point, m: Affine| Affine::translate(p.to_vec2()) * m * Affine::translate(-p.to_vec2());
        let m = match tab {
            0 if rel => Affine::translate((pos[0], pos[1])),
            0 => Affine::translate((pos[0] + origin.x - a.x, pos[1] + origin.y - a.y)),
            1 => {
                let c = if rel_center { bb.center() + Vec2::new(center[0], center[1]) } else { Point::new(center[0] + origin.x, center[1] + origin.y) };
                around(c, Affine::rotate(-rot.to_radians()))
            }
            2 => {
                let sx = scale[0] / 100.0 * if mirror[0] { -1.0 } else { 1.0 };
                let sy = scale[1] / 100.0 * if mirror[1] { -1.0 } else { 1.0 };
                around(a, Affine::scale_non_uniform(sx, sy))
            }
            3 => {
                let sx = size[0] / bb.width().max(1e-9);
                let sy = if prop { sx } else { size[1] / bb.height().max(1e-9) };
                around(a, Affine::scale_non_uniform(sx, sy))
            }
            _ => around(if use_anchor { a } else { bb.center() }, Affine::new([1.0, skew[1].to_radians().tan(), skew[0].to_radians().tan(), 1.0, 0.0, 0.0])),
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

pub fn align(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let (n, any_text) = app
        .doc()
        .map(|d| (d.selection.len(), d.selection.iter().any(|id| matches!(d.doc.find(*id).map(|o| &o.kind), Some(ObjKind::Text(_))))))
        .unwrap_or((0, false));
    let any = n > 0;
    let full_w = ui.available_width();
    // ------------------------------------------------------------------ align
    bold_title(ui, "Align");
    ui.add_space(8.0);
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for (k, (icon, tip, a)) in [
            ("align_left", "Align left (L)", Align::Left),
            ("align_hcenter", "Align centers horizontally (E)", Align::CenterH),
            ("align_right", "Align right (R)", Align::Right),
            ("align_top", "Align top (T)", Align::Top),
            ("align_vcenter", "Align centers vertically (C)", Align::CenterV),
            ("align_bottom", "Align bottom (B)", Align::Bottom),
        ]
        .into_iter()
        .enumerate()
        {
            if k == 3 {
                ui.add_space(24.0);
            }
            if icon_button(app, ui, icon, tip, any) {
                clicked = Some(a);
            }
        }
    });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.add_enabled(any, egui::Label::new("Align to:"));
        ui.add_space(10.0);
        for (k, (icon, tip)) in [("align_to_active", "Active objects"), ("align_to_page_edge", "Page edge"), ("align_to_page_center", "Page center"), ("align_to_grid", "Grid"), ("align_to_point", "Specified point")].into_iter().enumerate() {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, any, app.dockers.ui.align_to == k).clicked() && any {
                app.dockers.ui.align_to = k;
            }
        }
    });
    // the specified point
    let pt = any && app.dockers.ui.align_to == 4;
    if app.dockers.ui.pick_slot == SLOT_ALIGN_POINT {
        if let Some(p) = app.ts.picked_point.take() {
            let o = app.doc().map(|d| d.doc.settings.ruler_origin).unwrap_or_default();
            app.dockers.ui.align_xy = [p.x - o.x, p.y - o.y];
        }
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            for (k, l) in ["X:", "Y:"].into_iter().enumerate() {
                let mut v = app.dockers.ui.align_xy[k];
                form_row(ui, 28.0, l, pt, |ui| {
                    let (u, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
                    crate::widgets::unit_field(ui, egui::Id::unique(("ad_xy", k)), &mut v, u, dpi, 104.0, pt)
                });
                app.dockers.ui.align_xy[k] = v;
            }
        });
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.add_space(14.0);
            let picking = app.dockers.ui.pick_slot == SLOT_ALIGN_POINT && app.ts.pick_target.is_some();
            if crate::widgets::icon_button_sized(&mut app.icons, ui, "pick_point", "Select a point on the page", pt, picking, 22.0, 16.0).clicked() && pt {
                app.dockers.ui.pick_slot = SLOT_ALIGN_POINT;
                app.ts.pick_target = Some(crate::tools::PickTarget::DockerPoint);
            }
        });
    });
    if let Some(a) = clicked {
        let o = app.doc().map(|d| d.doc.settings.ruler_origin).unwrap_or_default();
        let to = match app.dockers.ui.align_to {
            1 => AlignTo::PageEdge,
            2 => AlignTo::PageCenter,
            3 => AlignTo::Grid,
            4 => AlignTo::Point(app.dockers.ui.align_xy[0] + o.x, app.dockers.ui.align_xy[1] + o.y),
            // Active objects: the last one selected, or the page for a single object
            _ if n == 1 => AlignTo::PageEdge,
            _ => AlignTo::LastSelected,
        };
        let (outline, text) = (app.dockers.ui.use_outline, [TextSource::FirstBaseline, TextSource::LastBaseline, TextSource::BoundingBox][app.dockers.ui.text_source.min(2)]);
        if let Some(d) = app.doc_mut() {
            let sel = d.selection.clone();
            let page = d.page;
            d.modify("Align", |doc, _| ops::align_ex(doc, &sel, a, to, page, outline, text));
        }
    }
    // ------------------------------------------------------------------ distribute
    ui.add_space(8.0);
    section(ui, "Distribute");
    ui.add_space(6.0);
    let mut dclick = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (k, (icon, tip, dd)) in [
            ("dist_left", "Distribute left", Distribute::Left),
            ("dist_hcenter", "Distribute centers horizontally", Distribute::CenterH),
            ("dist_right", "Distribute right", Distribute::Right),
            ("dist_hspace", "Distribute spacing horizontally", Distribute::SpacingH),
            ("dist_top", "Distribute top", Distribute::Top),
            ("dist_vcenter", "Distribute centers vertically", Distribute::CenterV),
            ("dist_bottom", "Distribute bottom", Distribute::Bottom),
            ("dist_vspace", "Distribute spacing vertically", Distribute::SpacingV),
        ]
        .into_iter()
        .enumerate()
        {
            if k == 4 {
                ui.add_space(8.0);
            }
            if icon_button(app, ui, icon, tip, any) {
                dclick = Some(dd);
            }
        }
    });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.add_enabled(any, egui::Label::new("Distribute to:"));
        ui.add_space(10.0);
        for (k, (icon, tip)) in [("dist_to_selection", "Selection extent"), ("dist_to_page", "Page extent"), ("dist_to_spacing", "Spacing")].into_iter().enumerate() {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, any, app.dockers.ui.dist_to == k).clicked() && any {
                app.dockers.ui.dist_to = k;
            }
        }
    });
    let sp = any && app.dockers.ui.dist_to == 2;
    for (k, l) in ["H:", "V:"].into_iter().enumerate() {
        let mut v = app.dockers.ui.dist_gap[k];
        form_row(ui, 28.0, l, sp, |ui| {
            let (u, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
            crate::widgets::unit_field(ui, egui::Id::unique(("ad_gap", k)), &mut v, u, dpi, 104.0, sp)
        });
        app.dockers.ui.dist_gap[k] = v;
    }
    if let Some(dd) = dclick {
        let (to, gap) = (app.dockers.ui.dist_to, app.dockers.ui.dist_gap);
        if let Some(d) = app.doc_mut() {
            let sel = d.selection.clone();
            let ext = (to == 1).then(|| d.page_ref().rect());
            d.modify("Distribute", |doc, _| {
                if to == 2 {
                    let horizontal = matches!(dd, Distribute::Left | Distribute::CenterH | Distribute::SpacingH | Distribute::Right);
                    ops::distribute_spacing(doc, &sel, dd, if horizontal { gap[0] } else { gap[1] });
                } else {
                    ops::distribute(doc, &sel, dd, ext);
                }
            });
        }
    }
    // ------------------------------------------------------------------ text
    ui.add_space(10.0);
    separator(ui);
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Text").font(theme::bold(13.0)).color(if any_text { theme::TEXT() } else { theme::DISABLED() }));
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for (k, (icon, tip)) in [("text_first_baseline", "First line baseline"), ("text_last_baseline", "Last line baseline"), ("text_bounding_box", "Bounding box")].into_iter().enumerate() {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, any_text, any_text && app.dockers.ui.text_source == k).clicked() && any_text {
                app.dockers.ui.text_source = k;
            }
        }
    });
    ui.add_space(6.0);
    let _ = full_w;
    separator(ui);
}

/// Align and Distribute ▸ Specified point: the point picked on the page.
const SLOT_ALIGN_POINT: u8 = 20;

const FIT_ORDER: [&str; 13] = [
    "Selection: Selection order",
    "Selection: Reversed selection order",
    "Selection: Top to bottom",
    "Selection: Bottom to top",
    "Selection: Left to right",
    "Selection: Right to left",
    "Area: Big to small",
    "Area: Small to big",
    "Height: Tall to short",
    "Height: Short to tall",
    "Width: Wide to narrow",
    "Width: Narrow to wide",
    "Random",
];
const FIT_DISTRIBUTION: [&str; 2] = ["Uniform object spacing", "Uniform reference point spacing"];
const FIT_REFERENCE: [&str; 2] = ["Center of rotation", "Object origin"];
const FIT_STYLE: [&str; 5] = ["None", "Uniform", "Progressive", "Jitter", "Progressive jitter"];

/// A row of the Fit Objects to Path docker: the label indented, the controls from the value column.
fn fit_row<R>(ui: &mut Ui, label: &str, enabled: bool, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(116.0, 22.0), egui::Sense::hover());
        ui.painter().text(egui::pos2(r.left() + 12.0, r.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), if enabled { theme::TEXT() } else { theme::DISABLED() });
        ui.add_enabled_ui(enabled, f).inner
    })
    .inner
}

pub fn fit_to_path(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    // the objects, then the path: the last object selected
    let (n, path_name) = app
        .doc()
        .map(|d| (d.selection.len(), d.selection.last().and_then(|id| d.doc.find(*id)).map(|o| o.kind.type_name()).unwrap_or("None")))
        .unwrap_or((0, "None"));
    let full_w = ui.available_width();
    let vw = (full_w - 124.0).max(120.0);
    let mut o = std::mem::take(&mut app.dockers.ui.fit);
    bold_title(ui, "Objects:");
    fit_row(ui, "Selection:", true, |ui| ui.label(if n == 0 { "None".to_string() } else { format!("{n} object(s) selected") }));
    fit_row(ui, "Source:", true, |ui| ui.checkbox(&mut o.keep_originals, "Keep originals"));
    fit_row(ui, "Output:", true, |ui| ui.checkbox(&mut o.group, "Group all objects"));
    fit_row(ui, "Duplicate(s):", true, |ui| {
        spin(ui, &mut o.duplicates, 0..=999, 1.0, 0, "", vw - 30.0);
        ui.add_space(6.0);
        ui.label(egui::RichText::new("ℹ").size(15.0).color(theme::TEXT())).on_hover_text("The number of extra copies of the objects fitted along the path.");
    });
    section(ui, "Path:");
    fit_row(ui, "Object used:", true, |ui| ui.label(path_name));
    fit_row(ui, "Subpaths:", true, |ui| ui.checkbox(&mut o.contiguous, "Treat as contiguous"));
    section(ui, "");
    let mut dist = o.even_reference as usize;
    let mut refp = o.object_origin as usize;
    fit_row(ui, "Order:", true, |ui| super::choice(ui, "fit_order", &mut o.order, &FIT_ORDER, vw - 8.0));
    fit_row(ui, "Distribution:", true, |ui| super::choice(ui, "fit_dist", &mut dist, &FIT_DISTRIBUTION, vw - 8.0));
    fit_row(ui, "Reference:", true, |ui| super::choice(ui, "fit_ref", &mut refp, &FIT_REFERENCE, vw - 8.0));
    o.even_reference = dist == 1;
    o.object_origin = refp == 1;
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(116.0, 22.0), egui::Sense::hover());
        ui.painter().text(egui::pos2(r.left() + 12.0, r.center().y), egui::Align2::LEFT_CENTER, "Origin:", egui::FontId::proportional(12.5), theme::TEXT());
        origin_grid(ui, &mut o.origin, true);
    });
    section(ui, "Rotation:");
    fit_row(ui, "Auto-rotate:", true, |ui| ui.checkbox(&mut o.follow_path, "Follow path"));
    fit_row(ui, "Reset:", true, |ui| ui.checkbox(&mut o.ignore_rotation, "Ignore initial rotation")).on_hover_text("Clear previously applied rotations before fitting objects to the path.");
    section(ui, "Modify rotation:");
    fit_row(ui, "Style:", true, |ui| super::choice(ui, "fit_style", &mut o.style, &FIT_STYLE, vw - 8.0));
    let st = o.style;
    let progressive = st == 2 || st == 4;
    fit_row(ui, "Start angle:", st != 0, |ui| spin(ui, &mut o.start_angle, -360.0..=360.0, 1.0, 1, "", vw));
    fit_row(ui, "Spin angle:", progressive, |ui| spin(ui, &mut o.spin_angle, -3600.0..=3600.0, 1.0, 1, "", vw));
    fit_row(ui, "Revolution(s):", progressive, |ui| spin(ui, &mut o.revolutions, 0.0..=100.0, 1.0, 1, "", vw));
    fit_row(ui, "Range (±):", st == 3 || st == 4, |ui| spin(ui, &mut o.range, 0.0..=180.0, 1.0, 1, "", vw));
    fit_row(ui, "Direction:", progressive, |ui| ui.checkbox(&mut o.clockwise, "Clockwise"));
    ui.add_space(8.0);
    separator(ui);
    ui.add_space(8.0);
    let apply = apply_bar(ui, "Apply", n >= 2);
    if apply {
        o.seed = o.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    }
    let opts = o.clone();
    app.dockers.ui.fit = o;
    if apply {
        let Some(d) = app.doc_mut() else { return };
        let mut sel = d.selection.clone();
        let Some(path_id) = sel.pop() else { return };
        let out = d.modify("Fit Objects to Path", |doc, _| ops::fit_objects_to_path(doc, &sel, path_id, &opts));
        if !out.is_empty() {
            d.selection = out;
        }
    }
}

const STEP_MODES: [&str; 3] = ["No offset", "Offset", "Spacing between objects"];

/// A Step and Repeat row: the label at the left, the control from `x` to the right edge.
fn sr_row<R>(ui: &mut Ui, label: &str, x: f32, enabled: bool, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(x, 22.0), egui::Sense::hover());
        ui.painter().text(egui::pos2(r.left(), r.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), if enabled { theme::TEXT() } else { theme::DISABLED() });
        ui.add_enabled_ui(enabled, f).inner
    })
    .inner
}

pub fn step_repeat(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let full_w = ui.available_width();
    let mut u = std::mem::take(&mut app.dockers.ui);
    for (k, (title, dirs)) in [("Horizontal Settings", ["Left", "Right"]), ("Vertical Settings", ["Up", "Down"])].into_iter().enumerate() {
        super::fold_header(ui, title, &mut u.step_open[k]);
        if !u.step_open[k] {
            continue;
        }
        super::choice(ui, &format!("sr_mode{k}"), &mut u.step_mode[k], &STEP_MODES, full_w - 12.0);
        let mode = u.step_mode[k];
        sr_row(ui, "Distance:", 70.0, mode != 0, |ui| {
            let (un, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
            let w = ui.available_width() - 20.0;
            crate::widgets::unit_field(ui, egui::Id::unique(("sr_dist", k)), &mut u.step_offset[k], un, dpi, w, mode != 0)
        });
        sr_row(ui, "Direction:", 70.0, mode == 2, |ui| {
            let w = ui.available_width() - 12.0;
            super::choice(ui, &format!("sr_dir{k}"), &mut u.step_dir[k], &dirs, w)
        });
        ui.add_space(2.0);
    }
    sr_row(ui, "Number of copies:", 116.0, true, |ui| {
        let w = ui.available_width() - 4.0;
        spin(ui, &mut u.step_copies, 1..=999, 1.0, 0, "", w)
    });
    let (c, dist, mode, dir) = (u.step_copies, u.step_offset, u.step_mode, u.step_dir);
    app.dockers.ui = u;
    if apply_bar(ui, "Apply", app.has_selection()) {
        if let Some(d) = app.doc_mut() {
            let sel = d.selection.clone();
            let Some(b) = d.selection_bounds() else { return };
            // each copy moves by the offset, or by the size of the selection and the space between
            let step = |k: usize, size: f64| match mode[k] {
                0 => 0.0,
                1 => dist[k],
                _ => (size + dist[k]) * if (k == 0 && dir[0] == 0) || (k == 1 && dir[1] == 0) { -1.0 } else { 1.0 },
            };
            let off = Vec2::new(step(0, b.width()), step(1, b.height()));
            d.modify("Step and Repeat", |doc, selection| {
                let new = ops::step_and_repeat(doc, &sel, c, off);
                if !new.is_empty() {
                    *selection = new;
                }
            });
        }
    }
}

const SHAPE_OPS: [(&str, Shaping); 7] = [
    ("Weld", Shaping::Weld),
    ("Trim", Shaping::Trim),
    ("Intersect", Shaping::Intersect),
    ("Simplify", Shaping::Simplify),
    ("Front minus back", Shaping::FrontMinusBack),
    ("Back minus front", Shaping::BackMinusFront),
    ("Boundary", Shaping::Boundary),
];

/// The part of a convex polygon inside a rectangle.
fn clip_to_rect(poly: &[egui::Pos2], r: egui::Rect) -> Vec<egui::Pos2> {
    let mut out = poly.to_vec();
    let edges: [(fn(egui::Pos2, egui::Rect) -> f32, bool); 4] = [(|p, r| p.x - r.left(), true), (|p, r| r.right() - p.x, true), (|p, r| p.y - r.top(), false), (|p, r| r.bottom() - p.y, false)];
    for (inside, _) in edges {
        let src = std::mem::take(&mut out);
        for k in 0..src.len() {
            let (a, b) = (src[k], src[(k + 1) % src.len()]);
            let (da, db) = (inside(a, r), inside(b, r));
            if da >= 0.0 {
                out.push(a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                out.push(a + (b - a) * (da / (da - db)));
            }
        }
        if out.is_empty() {
            break;
        }
    }
    out
}

/// The picture of the shaping operation: a triangle in front of a rectangle.
fn shape_preview(ui: &mut Ui, op: usize) {
    use egui::{Color32 as C, Shape as S, Stroke};
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(egui::vec2(w, 110.0), egui::Sense::hover());
    let p = ui.painter_at(r);
    p.rect_filled(r, 0.0, C::WHITE);
    let at = |x: f32, y: f32| egui::pos2(r.left() + r.width() * x, r.top() + r.height() * y);
    let rect = egui::Rect::from_min_max(at(0.37, 0.12), at(0.73, 0.67));
    let tri = vec![at(0.26, 0.39), at(0.47, 0.39), at(0.355, 0.9)];
    let both = clip_to_rect(&tri, rect);
    let (dark, gray, light, lav) = (C::from_gray(48), C::from_gray(120), C::from_gray(215), C::from_rgb(208, 202, 244));
    let edge = Stroke::new(1.0, C::from_gray(40));
    let dash = |pts: &[egui::Pos2], col: C| {
        let mut v = pts.to_vec();
        v.push(pts[0]);
        p.extend(S::dashed_line(&v, Stroke::new(1.0, col), 3.0, 2.0));
    };
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
    match op {
        0 => {
            p.rect_filled(rect, 0.0, dark);
            p.add(S::convex_polygon(tri.clone(), dark, Stroke::NONE));
        }
        1 => {
            p.rect_filled(rect, 0.0, gray);
            dash(&corners, C::from_gray(40));
            p.add(S::convex_polygon(tri.clone(), dark, Stroke::NONE));
            p.add(S::convex_polygon(both.clone(), C::WHITE, Stroke::NONE));
            dash(&both, C::from_gray(40));
        }
        2 => {
            p.add(S::convex_polygon(tri.clone(), light, Stroke::NONE));
            dash(&tri, C::from_gray(40));
            p.rect_filled(rect, 0.0, gray);
            dash(&corners, C::from_gray(40));
            p.add(S::convex_polygon(both.clone(), dark, Stroke::NONE));
        }
        3 => {
            p.rect_filled(rect, 0.0, lav);
            p.rect_stroke(rect, 0.0, edge, egui::StrokeKind::Inside);
            p.add(S::convex_polygon(tri.clone(), dark, Stroke::NONE));
        }
        4 => {
            dash(&corners, C::from_rgb(150, 140, 220));
            p.add(S::convex_polygon(tri.clone(), dark, Stroke::NONE));
            p.add(S::convex_polygon(both.clone(), C::WHITE, Stroke::NONE));
        }
        5 => {
            p.rect_filled(rect, 0.0, lav);
            p.rect_stroke(rect, 0.0, edge, egui::StrokeKind::Inside);
            p.add(S::convex_polygon(both.clone(), C::WHITE, Stroke::NONE));
            dash(&tri, C::from_rgb(150, 140, 220));
        }
        _ => {
            p.rect_filled(rect, 0.0, lav);
            p.add(S::convex_polygon(tri.clone(), lav, Stroke::NONE));
            p.rect_stroke(rect, 0.0, edge, egui::StrokeKind::Inside);
            p.add(S::closed_line(tri.clone(), edge));
            dash(&both, C::from_gray(40));
        }
    }
}

/// The Front / Back legend of Simplify, Front minus back and Back minus front.
fn shape_legend(ui: &mut Ui) {
    for (k, t) in ["Front", "Back"].into_iter().enumerate() {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 16.0), egui::Sense::hover());
            let c = r.center();
            if k == 0 {
                ui.painter().add(egui::Shape::convex_polygon(vec![c + egui::vec2(-5.0, -5.0), c + egui::vec2(5.0, -5.0), c + egui::vec2(0.0, 5.0)], theme::TEXT(), egui::Stroke::NONE));
            } else {
                ui.painter().rect_stroke(egui::Rect::from_center_size(c, egui::vec2(11.0, 8.0)), 1.0, egui::Stroke::new(1.0, theme::TEXT()), egui::StrokeKind::Inside);
            }
            ui.label(t);
        });
    }
}

/// Shape: pick the target, then shape the selection with it.
const SLOT_SHAPE_TARGET: u8 = 30;

pub fn shaping(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let n = app.doc().map(|d| d.selection.len()).unwrap_or(0);
    let full_w = ui.available_width();
    let mut cur = app.ts.shaping_op.min(SHAPE_OPS.len() - 1);
    let names: Vec<&str> = SHAPE_OPS.iter().map(|o| o.0).collect();
    super::choice(ui, "shaping", &mut cur, &names, full_w - 12.0);
    app.ts.shaping_op = cur;
    shape_preview(ui, cur);
    let u = &mut app.dockers.ui;
    match cur {
        0..=2 => {
            ui.checkbox(&mut u.shape_leave[cur][0], "Leave original source object");
            ui.checkbox(&mut u.shape_leave[cur][1], "Leave original target object");
        }
        3..=5 => shape_legend(ui),
        _ => {
            ui.checkbox(&mut u.boundary_behind, "Place behind selected");
            ui.checkbox(&mut u.boundary_leave, "Leave original object");
        }
    }
    ui.add_space(4.0);
    separator(ui);
    ui.add_space(6.0);
    let label = ["Weld to", "Trim", "Intersect with", "Apply", "Apply", "Apply", "Apply"][cur];
    let need = if cur == 6 { 1 } else if cur <= 2 { 1 } else { 2 };
    let picking = app.dockers.ui.pick_slot == SLOT_SHAPE_TARGET && app.ts.pick_target == Some(crate::tools::PickTarget::DockerObject);
    let clicked = ui
        .with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(4.0);
            ui.add_enabled(n >= need, egui::Button::new(label).selected(picking).fill(theme::BUTTON()).min_size(egui::vec2(80.0, 24.0))).clicked()
        })
        .inner;
    let op = SHAPE_OPS[cur].1;
    if clicked && cur <= 2 {
        // Weld to, Trim, Intersect with: then click the target object
        app.dockers.ui.pick_slot = SLOT_SHAPE_TARGET;
        app.ts.picked_object = None;
        app.ts.pick_target = Some(crate::tools::PickTarget::DockerObject);
        return;
    }
    let target = if app.dockers.ui.pick_slot == SLOT_SHAPE_TARGET { app.ts.picked_object.take() } else { None };
    if let Some(t) = target {
        app.dockers.ui.pick_slot = 0;
        let [ls, lt] = app.dockers.ui.shape_leave[cur.min(2)];
        let Some(d) = app.doc_mut() else { return };
        let mut ids: Vec<_> = d.selection.iter().copied().filter(|i| *i != t).collect();
        if ids.is_empty() {
            return;
        }
        ids.push(t);
        d.modify(SHAPE_OPS[cur].0, |doc, selection| {
            let r = ops::shaping(doc, &ids, op, ls, lt);
            if !r.is_empty() {
                *selection = r;
            }
        });
        return;
    }
    if clicked {
        let (behind, leave) = (app.dockers.ui.boundary_behind, app.dockers.ui.boundary_leave);
        let Some(d) = app.doc_mut() else { return };
        let sel = d.selection.clone();
        d.modify(SHAPE_OPS[cur].0, |doc, selection| {
            let r = ops::shaping(doc, &sel, op, true, true);
            if op == Shaping::Boundary {
                if let Some(&b) = r.first() {
                    if behind {
                        // under the backmost selected object
                        let back = sel.iter().filter_map(|id| doc.locate(*id)).min();
                        if let (Some((p, l, i)), Some((bp, bl, bi))) = (back, doc.locate(b)) {
                            if (p, l) == (bp, bl) {
                                let o = doc.pages[bp].layers[bl].objects.remove(bi);
                                doc.pages[p].layers[l].objects.insert(i, o);
                            }
                        }
                    }
                    if !leave {
                        for id in &sel {
                            doc.remove(*id);
                        }
                    }
                }
            }
            if !r.is_empty() {
                *selection = r;
            }
        });
    }
}

/// A radio button of the Corners and Join Curves dockers.
fn radio_row(ui: &mut Ui, cur: &mut usize, k: usize, label: &str) {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(120.0, 22.0), egui::Sense::click());
    let c = egui::pos2(r.left() + 7.0, r.center().y);
    let p = ui.painter();
    p.circle_stroke(c, 5.5, egui::Stroke::new(1.0, theme::TEXT_DIM()));
    if *cur == k {
        p.circle_filled(c, 3.0, theme::TEXT());
    }
    p.text(egui::pos2(r.left() + 18.0, r.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), theme::TEXT());
    if resp.clicked() {
        *cur = k;
    }
}

/// The Corners picture: the corner, the original edges dotted, the new corner in red.
fn corner_preview(ui: &mut Ui, kind: usize) {
    use egui::{pos2, vec2, Color32 as C, Stroke};
    let (r, _) = ui.allocate_exact_size(vec2(72.0, 90.0), egui::Sense::hover());
    let p = ui.painter_at(r);
    p.rect_filled(r, 0.0, C::WHITE);
    p.rect_filled(egui::Rect::from_min_max(pos2(r.left(), r.bottom() - 7.0), r.max), 0.0, C::from_gray(40));
    let at = |x: f32, y: f32| pos2(r.left() + r.width() * x, r.top() + r.height() * y);
    let (corner, a, b) = (at(0.2, 0.26), at(0.2, 0.52), at(0.6, 0.26));
    let black = Stroke::new(2.5, C::BLACK);
    p.line_segment([a, at(0.2, 0.86)], black);
    p.line_segment([b, at(0.97, 0.26)], black);
    p.extend(egui::Shape::dotted_line(&[a, corner, b], C::BLACK, 3.5, 1.1));
    let red = Stroke::new(2.0, C::from_rgb(230, 30, 30));
    let blue = Stroke::new(1.2, C::from_rgb(20, 150, 230));
    let font = egui::FontId::proportional(11.0);
    match kind {
        2 => {
            p.line_segment([a, b], red);
            p.arrow(at(0.1, 0.27), vec2(0.0, r.height() * 0.24), blue);
            p.arrow(at(0.22, 0.13), vec2(r.width() * 0.4, 0.0), blue);
            p.text(at(0.1, 0.6), egui::Align2::CENTER_CENTER, "A", font.clone(), blue.color);
            p.text(at(0.75, 0.13), egui::Align2::CENTER_CENTER, "B", font, blue.color);
        }
        _ => {
            // fillet bulges out to the corner, scallop curves in
            let ctl = if kind == 0 { [at(0.2, 0.36), at(0.33, 0.26)] } else { [at(0.33, 0.52), at(0.6, 0.39)] };
            p.add(egui::epaint::CubicBezierShape::from_points_stroke([a, ctl[0], ctl[1], b], false, C::TRANSPARENT, red));
            let tip = if kind == 0 { at(0.3, 0.34) } else { at(0.42, 0.46) };
            p.arrow(at(0.55, 0.62), tip - at(0.55, 0.62), blue);
            p.text(at(0.66, 0.5), egui::Align2::CENTER_CENTER, "r", font, blue.color);
        }
    }
}

pub fn corners(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let full_w = ui.available_width();
    let mut u = std::mem::take(&mut app.dockers.ui);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            for (k, n) in ["Fillet", "Scallop", "Chamfer"].into_iter().enumerate() {
                radio_row(ui, &mut u.corner_kind, k, n);
            }
        });
        ui.add_space((ui.available_width() - 74.0).max(0.0));
        corner_preview(ui, u.corner_kind);
    });
    ui.add_space(4.0);
    separator(ui);
    let chamfer = u.corner_kind == 2;
    let (un, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    sr_row(ui, "      Radius:", 66.0, !chamfer, |ui| crate::widgets::unit_field(ui, egui::Id::unique("corner_r"), &mut u.corner_radius, un, dpi, 92.0, !chamfer));
    separator(ui);
    ui.label(egui::RichText::new("Chamfer Distance").color(theme::TEXT()));
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            for (k, l) in ["A:", "B:"].into_iter().enumerate() {
                let en = chamfer && (k == 0 || !u.chamfer_lock);
                let mut v = u.chamfer[k];
                sr_row(ui, &format!("          {l}"), 66.0, en, |ui| {
                    if crate::widgets::unit_field(ui, egui::Id::unique(("chamfer", k)), &mut v, un, dpi, 92.0, en) {
                        u.chamfer[k] = v.max(0.0);
                    }
                });
            }
        });
        // the lock keeps B equal to A
        let (lr, lresp) = ui.allocate_exact_size(egui::vec2(18.0, 52.0), if chamfer { egui::Sense::click() } else { egui::Sense::hover() });
        let col = if chamfer { theme::TEXT() } else { theme::DISABLED() };
        let p = ui.painter();
        p.line_segment([lr.left_top() + egui::vec2(4.0, 8.0), lr.center_top() + egui::vec2(4.0, 8.0)], egui::Stroke::new(1.0, col));
        p.line_segment([lr.center_top() + egui::vec2(4.0, 8.0), lr.center_top() + egui::vec2(4.0, 13.0)], egui::Stroke::new(1.0, col));
        p.line_segment([lr.left_bottom() + egui::vec2(4.0, -8.0), lr.center_bottom() + egui::vec2(4.0, -8.0)], egui::Stroke::new(1.0, col));
        p.line_segment([lr.center_bottom() + egui::vec2(4.0, -8.0), lr.center_bottom() + egui::vec2(4.0, -13.0)], egui::Stroke::new(1.0, col));
        let icon = if u.chamfer_lock { "lock" } else { "unlock" };
        if let Some(t) = app.icons.get(ui.ctx(), icon, 14.0, if chamfer { crate::icons::Tone::Normal } else { crate::icons::Tone::Disabled }) {
            let ir = egui::Rect::from_center_size(lr.center() + egui::vec2(4.0, 0.0), egui::vec2(14.0, 14.0));
            if chamfer && u.chamfer_lock {
                ui.painter().rect_filled(ir.expand(1.0), 2.0, theme::ACCENT());
            }
            ui.painter().image(t.id, ir, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        if lresp.on_hover_text("Lock ratio").clicked() {
            u.chamfer_lock = !u.chamfer_lock;
        }
    });
    if u.chamfer_lock {
        u.chamfer[1] = u.chamfer[0];
    }
    separator(ui);
    ui.add_space(4.0);
    let (kind, r, ab) = (u.corner_kind, u.corner_radius, u.chamfer);
    app.dockers.ui = u;
    let _ = full_w;
    let apply = ui
        .with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(4.0);
            ui.add_enabled(app.has_selection(), egui::Button::new("Apply").fill(theme::BUTTON()).min_size(egui::vec2(80.0, 24.0))).clicked()
        })
        .inner;
    if apply {
        let names = ["Fillet", "Scallop", "Chamfer"];
        crate::propbar::edit_sel(app, names[kind], |o| {
            use vector_core::object::CornerType;
            let chamfer_ab = (kind == 2).then_some((ab[0], ab[1]));
            match &mut o.kind {
                ObjKind::Rect(rs) if kind != 2 || ab[0] == ab[1] => {
                    rs.radii = [if kind == 2 { ab[0] } else { r }; 4];
                    rs.corner = [CornerType::Round, CornerType::Scallop, CornerType::Chamfer][kind];
                }
                ObjKind::Rect(_) => {
                    // an uneven chamfer: the rectangle becomes a curve
                    let np = vector_core::path::NodePath::from_bez(&o.local_path());
                    o.kind = ObjKind::Path(crate::tools::round_corners_ex(&np, r, kind, chamfer_ab));
                }
                ObjKind::Path(p) => {
                    *p = crate::tools::round_corners_ex(p, r, kind, chamfer_ab);
                }
                _ => {}
            }
        });
    }
}

/// The Join Curves picture: two curve ends and how they join (dotted blue).
fn join_preview(ui: &mut Ui, mode: usize) {
    use egui::{pos2, vec2, Color32 as C, Stroke};
    let (r, _) = ui.allocate_exact_size(vec2(80.0, 85.0), egui::Sense::hover());
    let p = ui.painter_at(r);
    p.rect_stroke(r, 0.0, Stroke::new(1.0, theme::SEP()), egui::StrokeKind::Inside);
    let at = |x: f32, y: f32| pos2(r.left() + r.width() * x, r.top() + r.height() * y);
    let blue = C::from_rgb(40, 160, 230);
    let grey = Stroke::new(1.2, theme::TEXT_DIM());
    let node = |c: egui::Pos2| p.rect_stroke(egui::Rect::from_center_size(c, vec2(6.0, 6.0)), 1.0, grey, egui::StrokeKind::Middle);
    let dotted = |pts: &[egui::Pos2]| p.extend(egui::Shape::dotted_line(pts, blue, 4.0, 1.3));
    let curve = |pts: [egui::Pos2; 4]| {
        let c = egui::epaint::CubicBezierShape::from_points_stroke(pts, false, C::TRANSPARENT, Stroke::NONE);
        let flat: Vec<egui::Pos2> = c.flatten(Some(0.5));
        dotted(&flat);
    };
    match mode {
        1 => {
            let (a, b) = (at(0.3, 0.72), at(0.72, 0.3));
            p.line_segment([at(0.2, 0.82), a], grey);
            p.line_segment([b, at(0.82, 0.2)], grey);
            node(at(0.2, 0.82));
            node(at(0.82, 0.2));
            dotted(&[a, b]);
        }
        _ => {
            let (a, b) = (at(0.3, 0.62), at(0.7, 0.62));
            let (ea, eb) = (at(0.25, 0.8), at(0.75, 0.8));
            p.line_segment([ea, a], grey);
            p.line_segment([eb, b], grey);
            node(ea);
            node(eb);
            match mode {
                0 => dotted(&[a, at(0.5, 0.12), b]),
                2 => curve([a, at(0.3, 0.05), at(0.7, 0.05), b]),
                _ => curve([a, at(0.36, 0.3), at(0.62, 0.0), b]),
            }
        }
    }
}

pub fn join_curves(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let mut u = std::mem::take(&mut app.dockers.ui);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            for (k, n) in ["Extend", "Chamfer", "Fillet", "Bézier Curve"].into_iter().enumerate() {
                radio_row(ui, &mut u.join_mode, k, n);
            }
        });
        ui.add_space((ui.available_width() - 84.0).max(0.0));
        join_preview(ui, u.join_mode);
    });
    ui.add_space(4.0);
    let (un, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    let fillet = u.join_mode == 2;
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(92.0, 22.0), egui::Sense::hover());
        ui.painter().text(egui::pos2(r.right() - 4.0, r.center().y), egui::Align2::RIGHT_CENTER, "Gap tolerance:", egui::FontId::proportional(12.5), theme::TEXT());
        crate::widgets::unit_field(ui, egui::Id::unique("join_t"), &mut u.join_tolerance, un, dpi, 64.0, true);
    });
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(92.0, 22.0), egui::Sense::hover());
        ui.painter().text(egui::pos2(r.right() - 4.0, r.center().y), egui::Align2::RIGHT_CENTER, "Radius:", egui::FontId::proportional(12.5), if fillet { theme::TEXT() } else { theme::DISABLED() });
        crate::widgets::unit_field(ui, egui::Id::unique("join_r"), &mut u.join_radius, un, dpi, 64.0, fillet);
    });
    u.join_tolerance = u.join_tolerance.max(0.0);
    u.join_radius = u.join_radius.max(0.0);
    let (tol, mode, radius) = (u.join_tolerance, u.join_mode, u.join_radius);
    app.dockers.ui = u;
    ui.add_space(8.0);
    if apply_bar(ui, "Apply", app.has_selection()) {
        let mode = [ops::JoinMode::Extend, ops::JoinMode::Chamfer, ops::JoinMode::Fillet, ops::JoinMode::Bezier][mode.min(3)];
        if let Some(d) = app.doc_mut() {
            let sel = d.selection.clone();
            d.modify("Join Curves", |doc, selection| {
                if let Some(id) = ops::join_curves_ex(doc, &sel, tol, mode, radius) {
                    *selection = vec![id];
                }
            });
        }
    }
}
