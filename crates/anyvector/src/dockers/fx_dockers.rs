//! Effects dockers: Artistic Media, Bitmap Mask, PhotoCocktail and Pointillizer.

use super::{apply_bar, choice, form_row, icon_button, icon_button_resp, no_doc, section, spin};
use crate::app::AnyVectorApp;
use crate::propbar::edit_sel;
use crate::theme;
use egui::{pos2, vec2, Color32, Sense, Stroke, Ui};
use kurbo::{Affine, BezPath, Point, Rect, Shape, Vec2};
use std::path::{Path, PathBuf};
use vector_core::document::ImageData;
use vector_core::path::NodePath;
use vector_core::style::{VarWidth, WidthProfile};
use vector_core::{Color, Document, Fill, ObjId, ObjKind, Object, Outline, Style};

/// What the effects dockers remember between frames.
pub struct FxUi {
    // Artistic Media
    pub am_recent: Vec<WidthProfile>,
    pub am_type: usize,
    /// Gear menu: show brushes, object sprayer and preset strokes.
    pub am_show: [bool; 3],
    pub am_sel: Option<usize>,
    // Bitmap Mask: (checked, color, tolerance %) for the ten colors
    pub bm_rows: [(bool, Color, f64); 10],
    pub bm_row: usize,
    pub bm_hide: bool,
    // PhotoCocktail
    pub pc_keep: bool,
    pub pc_folder: Option<PathBuf>,
    pub pc_library: Vec<LibImage>,
    pub pc_columns: u32,
    pub pc_blending: f64,
    pub pc_dups: bool,
    pub pc_dup_tiles: u32,
    pub pc_composition: usize,
    pub pc_edges: usize,
    pub pc_priority: usize,
    pub pc_dpi: f64,
    pub pc_out: [u32; 2],
    pub pc_tile: [u32; 2],
    // Pointillizer
    pub pt_density: f64,
    pub pt_scale: f64,
    pub pt_angle: f64,
    pub pt_keep: bool,
    pub pt_limit: bool,
    pub pt_colors: u32,
    pub pt_method: usize,
    pub pt_merge: u32,
    pub pt_weld: bool,
    pub pt_shape: usize,
    pub pt_custom: Option<BezPath>,
}

impl Default for FxUi {
    fn default() -> Self {
        FxUi {
            am_recent: vec![],
            am_type: 0,
            am_show: [true; 3],
            am_sel: None,
            bm_rows: [(false, Color::BLACK, 0.0); 10],
            bm_row: 0,
            bm_hide: false,
            pc_keep: true,
            pc_folder: None,
            pc_library: vec![],
            pc_columns: 25,
            pc_blending: 30.0,
            pc_dups: true,
            pc_dup_tiles: 3,
            pc_composition: 1,
            pc_edges: 0,
            pc_priority: 0,
            pc_dpi: 100.0,
            pc_out: [1000, 1000],
            pc_tile: [30, 30],
            pt_density: 10.0,
            pt_scale: 1.0,
            pt_angle: 30.0,
            pt_keep: false,
            pt_limit: true,
            pt_colors: 16,
            pt_method: 1,
            pt_merge: 1,
            pt_weld: true,
            pt_shape: 0,
            pt_custom: None,
        }
    }
}

fn selected_ids(app: &AnyVectorApp) -> Vec<ObjId> {
    app.doc().map(|d| d.selection.clone()).unwrap_or_default()
}

/// The selection drawn into a bitmap: (pixels, width, height, bounds in the document).
fn render_selection(app: &AnyVectorApp, px_per_pt: f64) -> Option<(Vec<u8>, u32, u32, Rect)> {
    let d = app.doc()?;
    let ids = &d.selection;
    let sd = crate::exec::selection_document(&d.doc, ids)?;
    let bb = d.doc.selection_bounds(ids)?;
    let pm = vector_render::render_page_image(&sd, 0, 72.0 * px_per_pt, None, None, true)?;
    Some((vector_render::pixmap_to_rgba(&pm), pm.width(), pm.height(), bb))
}

/// Put the objects `build` makes in place of (or above) the selection, as one undo step.
fn place_result(app: &mut AnyVectorApp, label: &str, keep: bool, build: impl FnOnce(&mut Document) -> Vec<Object>) {
    let Some(d) = app.doc_mut() else { return };
    let ids = d.selection.clone();
    let page = d.page;
    d.modify(label, |doc, sel| {
        let objs = build(doc);
        // the result goes where the topmost selected object is
        let top = ids.iter().copied().filter(|id| doc.locate(*id).is_some()).max_by_key(|id| doc.locate(*id));
        if !keep {
            for id in ids.iter().filter(|id| Some(**id) != top) {
                doc.remove(*id);
            }
        }
        let (p, l, i) = top.and_then(|id| doc.locate(id)).unwrap_or((page, 0, 0));
        if let (false, Some(t)) = (keep, top) {
            doc.remove(t);
        }
        let layer = &mut doc.pages[p].layers[l].objects;
        let at = if keep { i + 1 } else { i }.min(layer.len());
        *sel = objs.iter().map(|o| o.id).collect();
        for (k, o) in objs.into_iter().enumerate() {
            layer.insert(at + k, o);
        }
    });
}

// ============================================================================== Artistic Media

/// The preset strokes: their width along the stroke (0..1 of the stroke width), as nodes.
const PRESET_SHAPES: &[&[(f64, f64)]] = &[
    &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)],
    &[(0.0, 1.0), (1.0, 0.0)],
    &[(0.0, 0.0), (1.0, 1.0)],
    &[(0.0, 0.1), (0.3, 1.0), (1.0, 0.1)],
    &[(0.0, 0.1), (0.7, 1.0), (1.0, 0.1)],
    &[(0.0, 0.5), (0.5, 1.0), (1.0, 0.5)],
    &[(0.0, 0.05), (0.15, 1.0), (0.85, 1.0), (1.0, 0.05)],
    &[(0.0, 0.0), (0.25, 1.0), (0.5, 0.3), (0.75, 1.0), (1.0, 0.0)],
    &[(0.0, 1.0), (0.5, 0.2), (1.0, 1.0)],
    &[(0.0, 0.2), (0.2, 0.8), (0.4, 0.3), (0.6, 0.8), (0.8, 0.3), (1.0, 0.0)],
    &[(0.0, 0.0), (0.1, 0.6), (0.9, 0.6), (1.0, 0.0)],
    &[(0.0, 0.8), (0.6, 0.6), (1.0, 0.0)],
    &[(0.0, 0.0), (0.4, 0.6), (1.0, 0.8)],
    &[(0.0, 0.0), (0.5, 0.45), (1.0, 0.0)],
    &[(0.0, 0.3), (0.5, 0.6), (1.0, 0.3)],
    &[(0.0, 0.0), (0.05, 1.0), (1.0, 0.0)],
    &[(0.0, 0.0), (0.95, 1.0), (1.0, 0.0)],
    &[(0.0, 1.0), (0.3, 0.3), (0.6, 0.6), (1.0, 0.0)],
    &[(0.0, 0.0), (0.4, 0.6), (0.7, 0.3), (1.0, 1.0)],
    &[(0.0, 0.0), (0.2, 0.4), (0.5, 1.0), (0.8, 0.4), (1.0, 0.0)],
    &[(0.0, 0.15), (1.0, 0.15)],
    &[(0.0, 0.0), (0.33, 1.0), (0.66, 1.0), (1.0, 0.0)],
    &[(0.0, 0.6), (0.2, 0.2), (0.8, 0.2), (1.0, 0.6)],
    &[(0.0, 1.0), (0.1, 0.4), (1.0, 0.0)],
    &[(0.0, 0.0), (0.9, 0.4), (1.0, 1.0)],
];

fn preset_profile(shape: &[(f64, f64)], half: f64) -> WidthProfile {
    WidthProfile::Variable { nodes: shape.iter().map(|(t, w)| VarWidth { t: *t, side1: w * half, side2: w * half }).collect(), scale: true }
}

/// Width (0..1) of a profile along the stroke, for previews.
fn profile_at(p: &WidthProfile, t: f64) -> f64 {
    match p {
        WidthProfile::Taper => (std::f64::consts::PI * t).sin(),
        WidthProfile::TaperEnd => 1.0 - t,
        WidthProfile::TaperStart => t,
        WidthProfile::Variable { nodes, .. } => {
            let max = nodes.iter().map(|n| n.side1 + n.side2).fold(1e-9, f64::max);
            let (a, b) = vector_core::style::var_width_at(nodes, t);
            (a + b) / max
        }
        _ => 0.7,
    }
}

/// A preview of a stroke: the profile along a gentle wave, black on white.
fn stroke_preview(ui: &mut Ui, p: &WidthProfile, size: egui::Vec2, selected: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    painter.rect_filled(r, 0.0, Color32::WHITE);
    if selected {
        painter.rect_stroke(r, 0.0, Stroke::new(2.0, theme::FOCUS), egui::StrokeKind::Inside);
    }
    let n = 40;
    let mut mesh = egui::Mesh::default();
    let pts: Vec<egui::Pos2> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            pos2(r.left() + 6.0 + t * (r.width() - 12.0), r.center().y + (t * std::f32::consts::TAU * 0.75).sin() * r.height() * 0.22)
        })
        .collect();
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let w = profile_at(p, t) as f32 * r.height() * 0.22;
        let dir = if i < n { pts[i + 1] - pts[i] } else { pts[i] - pts[i - 1] };
        let nrm = vec2(-dir.y, dir.x).normalized() * w.max(0.3);
        mesh.colored_vertex(pts[i] + nrm, Color32::from_gray(0x30));
        mesh.colored_vertex(pts[i] - nrm, Color32::from_gray(0x30));
        if i > 0 {
            let k = (i * 2) as u32;
            mesh.add_triangle(k - 2, k - 1, k);
            mesh.add_triangle(k - 1, k + 1, k);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
    resp
}

/// A stroke sized for an outline `width` points wide (variable strokes keep their widths in points).
pub fn fit_profile(p: &WidthProfile, width: f64) -> WidthProfile {
    match p {
        WidthProfile::Variable { nodes, scale } => {
            let half = width / 2.0;
            let max = nodes.iter().map(|n| n.side1.max(n.side2)).fold(1e-9, f64::max);
            WidthProfile::Variable { nodes: nodes.iter().map(|n| VarWidth { t: n.t, side1: n.side1 / max * half, side2: n.side2 / max * half }).collect(), scale: *scale }
        }
        other => other.clone(),
    }
}

fn use_stroke(app: &mut AnyVectorApp, p: WidthProfile) {
    let fx = &mut app.dockers.ui.fx;
    fx.am_recent.retain(|x| *x != p);
    fx.am_recent.insert(0, p.clone());
    fx.am_recent.truncate(12);
    if app.has_selection() {
        let w = app.ts.media_width.max(0.1);
        edit_sel(app, "Artistic Media", |o| {
            if o.is_container() || matches!(o.kind, ObjKind::Text(_) | ObjKind::Image(_)) {
                return;
            }
            if o.style.outline.color.is_none() {
                o.style.outline.color = Some(Color::BLACK);
            }
            if o.style.outline.width < 0.1 {
                o.style.outline.width = w;
            }
            o.style.outline.profile = Some(fit_profile(&p, o.style.outline.width));
            o.style.outline.cap = vector_core::style::LineCap::Round;
            o.style.outline.join = vector_core::style::LineJoin::Round;
        });
    }
    // the next strokes of the Artistic Media tool use it too
    app.ts.media_kind = 0;
    app.ts.media_profile = Some(p);
}

pub fn artistic_media(app: &mut AnyVectorApp, ui: &mut Ui) {
    let full_w = ui.available_width();
    ui.label("Last Used:");
    let recent = app.dockers.ui.fx.am_recent.clone();
    let mut pick = None;
    egui::Frame::new().fill(theme::BG).stroke(Stroke::new(1.0, theme::SEP)).show(ui, |ui| {
        ui.set_min_size(vec2(full_w - 2.0, 150.0));
        egui::ScrollArea::vertical().id_salt("am_recent").max_height(150.0).show(ui, |ui| {
            for p in &recent {
                if stroke_preview(ui, p, vec2(86.0, 20.0), false).on_hover_text("Click to use this stroke").clicked() {
                    pick = Some(p.clone());
                }
            }
        });
    });
    ui.add_space(2.0);
    ui.label("Stroke type:");
    ui.horizontal(|ui| {
        choice(ui, "am_type", &mut app.dockers.ui.fx.am_type, &["Default", "Custom"], full_w - 44.0);
        let gear = icon_button_resp(app, ui, "gear", "Show", true);
        egui::Popup::menu(&gear).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            for (i, n) in ["Brushes", "Object Sprayer", "Preset"].iter().enumerate() {
                ui.checkbox(&mut app.dockers.ui.fx.am_show[i], *n);
            }
        });
    });
    // the strokes of the chosen type
    let custom = app.dockers.ui.fx.am_type == 1;
    let list: Vec<(String, WidthProfile)> = if custom {
        app.settings.custom_strokes.clone()
    } else if app.dockers.ui.fx.am_show[2] {
        PRESET_SHAPES.iter().enumerate().map(|(i, s)| (format!("Preset {}", i + 1), preset_profile(s, 1.0))).collect()
    } else {
        vec![]
    };
    let sel = app.dockers.ui.fx.am_sel;
    let list_h = (ui.available_height() - 40.0).max(160.0);
    egui::Frame::new().fill(theme::BG).stroke(Stroke::new(1.0, theme::SEP)).show(ui, |ui| {
        ui.set_min_width(full_w - 2.0);
        egui::ScrollArea::vertical().id_salt("am_list").max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 5.0;
            for (i, (name, p)) in list.iter().enumerate() {
                ui.horizontal(|ui| {
                    let (ir, _) = ui.allocate_exact_size(vec2(24.0, 20.0), Sense::hover());
                    if let Some(t) = app.icons.get(ui.ctx(), "preset_stroke", 16.0, crate::icons::Tone::Normal) {
                        ui.painter().image(t.id, egui::Rect::from_center_size(ir.center(), vec2(16.0, 16.0)), egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    ui.add_space(70.0);
                    let r = stroke_preview(ui, p, vec2(86.0, 20.0), sel == Some(i)).on_hover_text(name);
                    if r.clicked() {
                        app.dockers.ui.fx.am_sel = Some(i);
                        pick = Some(p.clone());
                    }
                });
            }
        });
    });
    ui.horizontal(|ui| {
        if icon_button(app, ui, "open", "Open a stroke list", true) {
            if let Some(path) = rfd::FileDialog::new().add_filter("Strokes", &["json"]).pick_file() {
                match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<Vec<(String, WidthProfile)>>(&s).map_err(|e| e.to_string())) {
                    Ok(v) => {
                        app.settings.custom_strokes.extend(v);
                        app.dockers.ui.fx.am_type = 1;
                    }
                    Err(e) => app.flash(format!("Could not open the strokes: {e}")),
                }
            }
        }
        let cur = app.doc().and_then(|d| d.selection.first().and_then(|id| d.doc.find(*id))).and_then(|o| o.style.outline.profile.clone());
        if icon_button(app, ui, "save", "Save the stroke of the selected object as a custom stroke", cur.is_some()) {
            if let Some(p) = cur {
                let n = app.settings.custom_strokes.len() + 1;
                app.settings.custom_strokes.push((format!("Custom {n}"), p));
                app.dockers.ui.fx.am_type = 1;
            }
        }
        ui.add_space((ui.available_width() - 28.0).max(0.0));
        let can_delete = custom && sel.is_some_and(|i| i < app.settings.custom_strokes.len());
        if icon_button(app, ui, "delete", "Delete the custom stroke", can_delete) {
            if let Some(i) = sel {
                app.settings.custom_strokes.remove(i);
                app.dockers.ui.fx.am_sel = None;
            }
        }
    });
    if let Some(p) = pick {
        use_stroke(app, p);
    }
}

// ============================================================================== Bitmap Mask

fn selected_bitmap(app: &AnyVectorApp) -> Option<(ObjId, u64)> {
    let d = app.doc()?;
    let [id] = d.selection.as_slice() else { return None };
    match &d.doc.find(*id)?.kind {
        ObjKind::Image(io) => Some((*id, io.image)),
        _ => None,
    }
}

/// Hide (or keep only) the pixels within the tolerance of the checked colors.
pub fn mask_pixels(rgba: &mut [u8], rows: &[(bool, Color, f64)], hide: bool) {
    let on: Vec<&(bool, Color, f64)> = rows.iter().filter(|r| r.0).collect();
    for px in rgba.chunks_exact_mut(4) {
        let hit = on.iter().any(|(_, c, tol)| {
            let lim = tol / 100.0 * 255.0;
            (px[0] as f64 - c.r as f64).abs() <= lim && (px[1] as f64 - c.g as f64).abs() <= lim && (px[2] as f64 - c.b as f64).abs() <= lim
        });
        if hit == hide {
            px[3] = 0;
        }
    }
}

fn save_mask(path: &Path, fx: &FxUi) -> std::io::Result<()> {
    let mut s = String::from("[BitmapMask]\r\n");
    s += &format!("Mode={}\r\n", if fx.bm_hide { "Hide" } else { "Show" });
    for (i, (on, c, tol)) in fx.bm_rows.iter().enumerate() {
        s += &format!("Color{}={},{},{}\r\nTolerance{}={}\r\nEnabled{}={}\r\n", i + 1, c.r, c.g, c.b, i + 1, tol, i + 1, *on as u8);
    }
    std::fs::write(path, s)
}

fn open_mask(path: &Path, fx: &mut FxUi) -> std::io::Result<()> {
    let s = std::fs::read_to_string(path)?;
    for line in s.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        if k == "Mode" {
            fx.bm_hide = v.eq_ignore_ascii_case("hide");
            continue;
        }
        let digits = k.trim_start_matches(|c: char| c.is_alphabetic());
        let Ok(n) = digits.parse::<usize>() else { continue };
        let Some(row) = fx.bm_rows.get_mut(n.wrapping_sub(1)) else { continue };
        match k.trim_end_matches(digits) {
            "Color" => {
                let c: Vec<u8> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                if c.len() == 3 {
                    row.1 = Color::rgb(c[0], c[1], c[2]);
                }
            }
            "Tolerance" => row.2 = v.parse().unwrap_or(0.0),
            "Enabled" => row.0 = v == "1",
            _ => {}
        }
    }
    Ok(())
}

pub fn bitmap_mask(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        no_doc(ui);
        return;
    }
    // a color picked on the canvas goes to the chosen row
    if let Some(c) = app.ts.picked_color.take() {
        let fx = &mut app.dockers.ui.fx;
        let r = &mut fx.bm_rows[fx.bm_row];
        r.1 = c;
        r.0 = true;
    }
    let bitmap = selected_bitmap(app);
    let full_w = ui.available_width();
    ui.horizontal(|ui| {
        if icon_button(app, ui, "eyedropper", "Color selector: click a color in the bitmap", bitmap.is_some()) {
            app.ts.pick_target = Some(crate::tools::PickTarget::SampleColor);
            app.flash("Click a color in the bitmap.");
        }
        let edit = icon_button_resp(app, ui, "edit_fill", "Edit color", true);
        egui::Popup::menu(&edit).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            let fx = &mut app.dockers.ui.fx;
            let row = fx.bm_row;
            let mut c = fx.bm_rows[row].1;
            if super::props::color_edit(ui, &mut c, false) {
                fx.bm_rows[row].1 = c;
                fx.bm_rows[row].0 = true;
            }
        });
    });
    egui::Frame::new().fill(theme::BG).stroke(Stroke::new(1.0, theme::SEP)).show(ui, |ui| {
        ui.set_min_width(full_w - 2.0);
        egui::ScrollArea::vertical().id_salt("bm_rows").max_height(180.0).show(ui, |ui| {
            for i in 0..10 {
                let fx = &mut app.dockers.ui.fx;
                let selected = fx.bm_row == i;
                let (r, resp) = ui.allocate_exact_size(vec2(full_w - 14.0, 24.0), Sense::click());
                if selected {
                    ui.painter().rect_filled(r, 0.0, theme::HOVER);
                }
                let cb = egui::Rect::from_min_size(pos2(r.left() + 4.0, r.center().y - 6.0), vec2(12.0, 12.0));
                ui.painter().rect_stroke(cb, 1.0, Stroke::new(1.0, theme::TEXT_DIM), egui::StrokeKind::Inside);
                if fx.bm_rows[i].0 {
                    ui.painter().line_segment([cb.left_center() + vec2(2.0, 0.0), cb.center_bottom() - vec2(0.0, 3.0)], Stroke::new(1.5, theme::TEXT));
                    ui.painter().line_segment([cb.center_bottom() - vec2(0.0, 3.0), cb.right_top() + vec2(-2.0, 3.0)], Stroke::new(1.5, theme::TEXT));
                }
                let c = fx.bm_rows[i].1;
                let sw = egui::Rect::from_min_max(pos2(cb.right() + 8.0, r.top() + 2.0), pos2(r.right() - 40.0, r.bottom() - 2.0));
                ui.painter().rect_filled(sw, 0.0, Color32::from_rgb(c.r, c.g, c.b));
                ui.painter().rect_stroke(sw, 0.0, Stroke::new(1.0, Color32::from_gray(200)), egui::StrokeKind::Inside);
                ui.painter().text(pos2(sw.right() + 8.0, r.center().y), egui::Align2::LEFT_CENTER, format!("{}", fx.bm_rows[i].2.round()), egui::FontId::proportional(12.0), Color32::from_rgb(0xe8, 0xa0, 0x40));
                if resp.clicked() {
                    if resp.interact_pointer_pos().is_some_and(|p| p.x < cb.right() + 4.0) {
                        fx.bm_rows[i].0 = !fx.bm_rows[i].0;
                    }
                    fx.bm_row = i;
                }
            }
        });
    });
    ui.horizontal(|ui| {
        if icon_button(app, ui, "save", "Save mask", true) {
            if let Some(p) = rfd::FileDialog::new().add_filter("Mask", &["ini"]).set_file_name("mask.ini").save_file() {
                if let Err(e) = save_mask(&p, &app.dockers.ui.fx) {
                    app.flash(format!("Could not save the mask: {e}"));
                }
            }
        }
        if icon_button(app, ui, "open", "Open mask", true) {
            if let Some(p) = rfd::FileDialog::new().add_filter("Mask", &["ini"]).pick_file() {
                if let Err(e) = open_mask(&p, &mut app.dockers.ui.fx) {
                    app.flash(format!("Could not open the mask: {e}"));
                }
            }
        }
        ui.add_space((ui.available_width() - 28.0).max(0.0));
        let row = app.dockers.ui.fx.bm_row;
        let used = app.dockers.ui.fx.bm_rows[row].0;
        if icon_button(app, ui, "delete", "Remove the color", used) {
            app.dockers.ui.fx.bm_rows[row] = (false, Color::BLACK, 0.0);
        }
    });
    ui.add_space(6.0);
    ui.label("Tolerance:");
    ui.horizontal(|ui| {
        let fx = &mut app.dockers.ui.fx;
        let row = fx.bm_row;
        let mut t = fx.bm_rows[row].2;
        let w = (full_w - 90.0).max(60.0);
        ui.spacing_mut().slider_width = w;
        ui.add(egui::Slider::new(&mut t, 0.0..=100.0).show_value(false));
        spin(ui, &mut t, 0.0..=100.0, 1.0, 0, "", 50.0);
        ui.label("%");
        fx.bm_rows[row].2 = t;
    });
    ui.add_space(6.0);
    ui.horizontal_top(|ui| {
        ui.label("Colors:");
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.radio_value(&mut app.dockers.ui.fx.bm_hide, true, "Hide selected");
            ui.radio_value(&mut app.dockers.ui.fx.bm_hide, false, "Show selected");
        });
    });
    let any = app.dockers.ui.fx.bm_rows.iter().any(|r| r.0);
    if apply_bar(ui, "Apply", bitmap.is_some() && any) {
        let (Some((id, img)), Some(d)) = (bitmap, app.doc_mut()) else { return };
        let Some(src) = d.doc.images.get(&img).cloned() else { return };
        let mut rgba = (*src.rgba).clone();
        mask_pixels(&mut rgba, &app.dockers.ui.fx.bm_rows, app.dockers.ui.fx.bm_hide);
        let Some(d) = app.doc_mut() else { return };
        d.modify("Bitmap Mask", |doc, _| {
            let data = ImageData { rgba: std::sync::Arc::new(rgba), encoded: Default::default(), format: "png".into(), ..src };
            let nid = doc.add_image(data);
            if let Some(o) = doc.find_mut(id) {
                if let ObjKind::Image(io) = &mut o.kind {
                    io.image = nid;
                }
            }
        });
    }
}

// ============================================================================== PhotoCocktail

/// A picture of the PhotoCocktail library: its file, average color and a square sample.
#[derive(Clone)]
pub struct LibImage {
    pub path: PathBuf,
    pub avg: [f64; 3],
    pub thumb: image::RgbaImage,
}

const THUMB: u32 = 48;

fn load_library(folder: &Path) -> Vec<LibImage> {
    let Ok(rd) = std::fs::read_dir(folder) else { return vec![] };
    let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png" | "bmp" | "gif" | "webp" | "tif" | "tiff"))).collect();
    files.sort();
    files.truncate(2000);
    files
        .into_iter()
        .filter_map(|p| {
            let img = image::open(&p).ok()?;
            let thumb = img.resize_to_fill(THUMB, THUMB, image::imageops::FilterType::Triangle).to_rgba8();
            let n = (THUMB * THUMB) as f64;
            let mut avg = [0.0; 3];
            for px in thumb.pixels() {
                for k in 0..3 {
                    avg[k] += px[k] as f64 / n;
                }
            }
            Some(LibImage { path: p, avg, thumb })
        })
        .collect()
}

/// For each cell color, the library picture closest to it; a picture is not used again within
/// `spacing` cells (no repeats at all when `spacing` is None, while pictures last).
pub fn match_tiles(cells: &[[f64; 3]], cols: usize, lib: &[[f64; 3]], spacing: Option<u32>) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::with_capacity(cells.len());
    let mut used = vec![0u32; lib.len()];
    for (i, c) in cells.iter().enumerate() {
        let (cx, cy) = ((i % cols) as i64, (i / cols) as i64);
        let near = |j: usize| -> bool {
            let Some(s) = spacing else { return false };
            out.iter().enumerate().any(|(k, &t)| {
                t == j && {
                    let (kx, ky) = ((k % cols) as i64, (k / cols) as i64);
                    (kx - cx).abs().max((ky - cy).abs()) <= s as i64
                }
            })
        };
        let dist = |j: usize| (0..3).map(|k| (lib[j][k] - c[k]).powi(2)).sum::<f64>();
        let mut order: Vec<usize> = (0..lib.len()).collect();
        order.sort_by(|a, b| dist(*a).total_cmp(&dist(*b)));
        let pick = order
            .iter()
            .copied()
            .find(|&j| match spacing {
                Some(_) => !near(j),
                None => used[j] == 0,
            })
            .unwrap_or(order[0]);
        used[pick] += 1;
        out.push(pick);
    }
    out
}

fn photococktail_apply(app: &mut AnyVectorApp) -> Result<(), String> {
    let fx = &app.dockers.ui.fx;
    if fx.pc_library.is_empty() {
        return Err("Choose a folder of pictures for the library first.".into());
    }
    let bb = app.doc().and_then(|d| d.doc.selection_bounds(&d.selection)).ok_or("Select the reference object first.")?;
    let cols = fx.pc_columns.max(1) as usize;
    let cell = bb.width() / cols as f64;
    // omit partial tiles: whole square tiles only; stretch: the rows fill the height
    let rows = match fx.pc_edges {
        0 => ((bb.height() / cell).floor() as usize).max(1),
        _ => ((bb.height() / cell).round() as usize).max(1),
    };
    let cell_h = if fx.pc_edges == 0 { cell } else { bb.height() / rows as f64 };
    // pixels per tile
    let doc_dpi = app.doc().map(|d| d.doc.settings.dpi).unwrap_or(300.0);
    let tile_px: (u32, u32) = match fx.pc_priority {
        0 => (((cell / 72.0) * doc_dpi).round() as u32, ((cell_h / 72.0) * doc_dpi).round() as u32),
        1 => (((cell / 72.0) * fx.pc_dpi).round() as u32, ((cell_h / 72.0) * fx.pc_dpi).round() as u32),
        2 => (fx.pc_tile[0], fx.pc_tile[1]),
        _ => (fx.pc_out[0] / cols as u32, fx.pc_out[1] / rows as u32),
    };
    let (tw, th) = (tile_px.0.min(15000 / cols as u32).max(4), tile_px.1.min(15000 / rows as u32).max(4));
    // the reference colors of the cells, from 8x8 samples per cell
    let k = 8usize;
    let scale = (cols * k) as f64 / bb.width().max(1e-6);
    let (rgba, w, h, _) = render_selection(app, scale).ok_or("Nothing to render.")?;
    let fx = &app.dockers.ui.fx;
    let mut cells = vec![[255.0; 3]; cols * rows];
    let mut alpha = vec![0.0; cols * rows];
    for r in 0..rows {
        for c in 0..cols {
            let (mut s, mut a, mut n) = ([0.0; 3], 0.0, 0.0);
            let (y0, y1) = ((r as f64 * cell_h * scale) as u32, ((r + 1) as f64 * cell_h * scale).ceil() as u32);
            for y in y0..y1.min(h) {
                for x in (c * k) as u32..(((c + 1) * k) as u32).min(w) {
                    let i = ((y * w + x) * 4) as usize;
                    let al = rgba[i + 3] as f64 / 255.0;
                    // flattened on white
                    for q in 0..3 {
                        s[q] += rgba[i + q] as f64 * al + 255.0 * (1.0 - al);
                    }
                    a += al;
                    n += 1.0;
                }
            }
            if n > 0.0 {
                cells[r * cols + c] = [(s[0] / n).min(255.0), (s[1] / n).min(255.0), (s[2] / n).min(255.0)];
                alpha[r * cols + c] = a / n;
            }
        }
    }
    let lib: Vec<[f64; 3]> = fx.pc_library.iter().map(|l| l.avg).collect();
    let picks = match_tiles(&cells, cols, &lib, fx.pc_dups.then_some(fx.pc_dup_tiles));
    let blend = (fx.pc_blending / 100.0).clamp(0.0, 1.0);
    let tile_img = |j: usize| image::imageops::resize(&fx.pc_library[j].thumb, tw, th, image::imageops::FilterType::Triangle);
    let tile_name = |j: usize| fx.pc_library[j].path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    // the reference colors as a layer over the tiles
    let blend_layer = || {
        let mut img = image::RgbaImage::new(cols as u32 * tw, rows as u32 * th);
        for (i, c) in cells.iter().enumerate() {
            let (cx, cy) = ((i % cols) as u32 * tw, (i / cols) as u32 * th);
            let a = (blend * alpha[i] * 255.0) as u8;
            for y in 0..th {
                for x in 0..tw {
                    img.put_pixel(cx + x, cy + y, image::Rgba([c[0] as u8, c[1] as u8, c[2] as u8, a]));
                }
            }
        }
        img
    };
    // (picture, where, name) of each bitmap of the result
    let mut parts: Vec<(image::RgbaImage, Point, String)> = vec![];
    match fx.pc_composition {
        // single bitmap: the tiles with the reference colors mixed in
        0 => {
            let mut img = image::RgbaImage::new(cols as u32 * tw, rows as u32 * th);
            for (i, &j) in picks.iter().enumerate() {
                let t = tile_img(j);
                let c = cells[i];
                let a = blend * alpha[i];
                let (cx, cy) = ((i % cols) as u32 * tw, (i / cols) as u32 * th);
                for (x, y, px) in t.enumerate_pixels() {
                    let mix = |q: usize| (px[q] as f64 * (1.0 - a) + c[q] * a) as u8;
                    img.put_pixel(cx + x, cy + y, image::Rgba([mix(0), mix(1), mix(2), 255]));
                }
            }
            parts.push((img, bb.origin(), "PhotoCocktail".into()));
        }
        // bitmap stack: the tiles in one bitmap, the reference colors in a bitmap over it
        1 => {
            let mut img = image::RgbaImage::new(cols as u32 * tw, rows as u32 * th);
            for (i, &j) in picks.iter().enumerate() {
                image::imageops::overlay(&mut img, &tile_img(j), ((i % cols) as u32 * tw) as i64, ((i / cols) as u32 * th) as i64);
            }
            parts.push((img, bb.origin(), "Tiles".into()));
            parts.push((blend_layer(), bb.origin(), "Blending".into()));
        }
        // bitmap array: every tile its own bitmap, then the reference colors
        _ => {
            for (i, &j) in picks.iter().enumerate() {
                let at = bb.origin() + Vec2::new((i % cols) as f64 * cell, (i / cols) as f64 * cell_h);
                parts.push((tile_img(j), at, tile_name(j)));
            }
            parts.push((blend_layer(), bb.origin(), "Blending".into()));
        }
    }
    let single = parts.len() == 1;
    let dpi = tw as f64 / cell * 72.0;
    let keep = fx.pc_keep;
    place_result(app, "PhotoCocktail", keep, move |doc| {
        let mut kids: Vec<Object> = parts
            .into_iter()
            .map(|(img, at, name)| {
                let data = ImageData { width: img.width(), height: img.height(), rgba: std::sync::Arc::new(img.into_raw()), encoded: Default::default(), format: "png".into(), name: name.clone() };
                let mut o = vector_io::image_io::image_object(doc, data, dpi);
                o.apply_transform(Affine::translate(at.to_vec2()));
                o.name = name;
                o
            })
            .collect();
        if single {
            return kids;
        }
        let mut g = Object::new(doc.new_id(), ObjKind::Group(std::mem::take(&mut kids)), Affine::IDENTITY, Style::default());
        g.name = "PhotoCocktail".into();
        vec![g]
    });
    Ok(())
}

pub fn photococktail(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        no_doc(ui);
        return;
    }
    let lw = 104.0;
    let n = app.doc().map(|d| d.selection.len()).unwrap_or(0);
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Reference").strong());
    ui.add_space(4.0);
    form_row(ui, lw, "Selection:", true, |ui| ui.label(match n {
        0 => "No objects selected".to_string(),
        1 => "1 object selected".to_string(),
        n => format!("{n} objects selected"),
    }));
    form_row(ui, lw, "", true, |ui| ui.checkbox(&mut app.dockers.ui.fx.pc_keep, "Keep original source"));
    section(ui, "Library");
    let fx = &app.dockers.ui.fx;
    let count = if fx.pc_library.is_empty() { "None".to_string() } else { fx.pc_library.len().to_string() };
    let folder = fx.pc_folder.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "None".into());
    form_row(ui, lw, "Images:", true, |ui| ui.label(count));
    form_row(ui, lw, "Path:", true, |ui| ui.add(egui::Label::new(folder).truncate()));
    form_row(ui, lw, "", true, |ui| {
        if ui.add(egui::Button::new("Browse...").min_size(vec2(80.0, 24.0))).clicked() {
            if let Some(p) = rfd::FileDialog::new().set_title("Select Folder").pick_folder() {
                let lib = load_library(&p);
                if lib.is_empty() {
                    app.flash("The folder has no pictures AnyVector can read.");
                }
                app.dockers.ui.fx.pc_library = lib;
                app.dockers.ui.fx.pc_folder = Some(p);
            }
        }
    });
    section(ui, "Grid");
    let bb = app.doc().and_then(|d| d.doc.selection_bounds(&d.selection));
    let fx = &mut app.dockers.ui.fx;
    form_row(ui, lw, "Columns:", true, |ui| spin(ui, &mut fx.pc_columns, 1..=500, 1.0, 0, "", 60.0));
    let mut rows = bb.map(|b| ((b.height() / (b.width() / fx.pc_columns.max(1) as f64)).round() as u32).max(1)).unwrap_or(10);
    form_row(ui, lw, "Rows:", false, |ui| spin(ui, &mut rows, 1..=5000, 1.0, 0, "", 60.0));
    form_row(ui, lw, "Blending:", true, |ui| spin(ui, &mut fx.pc_blending, 0.0..=100.0, 1.0, 0, " %", 60.0));
    section(ui, "");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(lw, 22.0), Sense::hover());
        ui.put(r, egui::Checkbox::new(&mut fx.pc_dups, "Duplicates:"));
        ui.add_enabled_ui(fx.pc_dups, |ui| spin(ui, &mut fx.pc_dup_tiles, 0..=50, 1.0, 0, "", 60.0));
    });
    section(ui, "Output");
    form_row(ui, lw, "Composition:", true, |ui| choice(ui, "pc_comp", &mut fx.pc_composition, &["Single bitmap", "Bitmap stack", "Bitmap array"], 150.0));
    form_row(ui, lw, "Edges:", true, |ui| choice(ui, "pc_edges", &mut fx.pc_edges, &["Omit partial tiles", "Stretch output"], 150.0));
    form_row(ui, lw, "Priority:", true, |ui| choice(ui, "pc_prio", &mut fx.pc_priority, &["Match document DPI", "Custom DPI", "Custom tile dimensions", "Custom output dimensions"], 150.0));
    form_row(ui, lw, "DPI:", fx.pc_priority == 1, |ui| spin(ui, &mut fx.pc_dpi, 10.0..=1200.0, 1.0, 0, "", 60.0));
    ui.add_space(6.0);
    ui.columns(2, |c| {
        c[0].label("Artwork:");
        c[1].label("Tile:");
        let out = fx.pc_priority == 3;
        let tile = fx.pc_priority == 2;
        for (i, n) in ["W:", "H:"].iter().enumerate() {
            c[0].horizontal(|ui| {
                ui.label(*n);
                ui.add_enabled_ui(out, |ui| spin(ui, &mut fx.pc_out[i], 1..=15000, 1.0, 0, "", 70.0));
            });
            c[1].horizontal(|ui| {
                ui.label(*n);
                ui.add_enabled_ui(tile, |ui| spin(ui, &mut fx.pc_tile[i], 1..=2000, 1.0, 0, "", 70.0));
            });
        }
    });
    section(ui, "");
    if apply_bar(ui, "Apply", n > 0 && !app.dockers.ui.fx.pc_library.is_empty()) {
        if let Err(e) = photococktail_apply(app) {
            app.flash(e);
        }
    }
}

// ============================================================================== Pointillizer

/// The tile shape, `size` points across, centered on the origin.
fn tile_shape(kind: usize, custom: Option<&BezPath>, size: f64) -> BezPath {
    let r = size / 2.0;
    match (kind, custom) {
        (1, _) => Rect::new(-r, -r, r, r).to_path(0.1),
        (2, Some(c)) => {
            let b = c.bounding_box();
            let k = size / b.width().max(b.height()).max(1e-9);
            Affine::scale(k) * Affine::translate(-b.center().to_vec2()) * c.clone()
        }
        _ => kurbo::Circle::new(Point::ZERO, r).to_path(0.1),
    }
}

/// Reduce colors to at most `n` (k-means), returning the palette index of each color.
pub fn limit_colors(colors: &[[f64; 3]], n: usize) -> (Vec<[f64; 3]>, Vec<usize>) {
    let n = n.clamp(1, colors.len().max(1));
    let mut centers: Vec<[f64; 3]> = (0..n).map(|i| colors[i * colors.len() / n]).collect();
    let mut idx = vec![0; colors.len()];
    for _ in 0..10 {
        for (i, c) in colors.iter().enumerate() {
            idx[i] = (0..n).min_by(|a, b| d2(&centers[*a], c).total_cmp(&d2(&centers[*b], c))).unwrap_or(0);
        }
        let mut sum = vec![[0.0; 4]; n];
        for (i, c) in colors.iter().enumerate() {
            let s = &mut sum[idx[i]];
            s[0] += c[0];
            s[1] += c[1];
            s[2] += c[2];
            s[3] += 1.0;
        }
        for (k, s) in sum.iter().enumerate() {
            if s[3] > 0.0 {
                centers[k] = [s[0] / s[3], s[1] / s[3], s[2] / s[3]];
            }
        }
    }
    (centers, idx)
}

fn d2(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum()
}

/// A tile of the mosaic: where it is, its size (0..1 of the full tile) and its color.
#[derive(Clone, Debug)]
pub struct Tile {
    pub at: Point,
    pub size: f64,
    pub color: [f64; 3],
}

/// The tiles of the mosaic of an image (pixels `rgba`, `w`×`h`, covering `bb`).
pub fn pointillize(rgba: &[u8], w: u32, h: u32, bb: Rect, density: f64, angle: f64, method: usize) -> (Vec<Tile>, f64) {
    let step = 72.0 / density.max(0.01).sqrt();
    let c = bb.center();
    let rot = Affine::translate(c.to_vec2()) * Affine::rotate(-angle.to_radians()) * Affine::translate(-c.to_vec2());
    let reach = (bb.width().hypot(bb.height()) / 2.0 / step).ceil() as i64 + 1;
    let mut tiles = vec![];
    for j in -reach..=reach {
        for i in -reach..=reach {
            let p = rot * Point::new(c.x + i as f64 * step, c.y + j as f64 * step);
            if !bb.contains(p) {
                continue;
            }
            let x = (((p.x - bb.x0) / bb.width()) * w as f64).clamp(0.0, w as f64 - 1.0) as u32;
            let y = (((p.y - bb.y0) / bb.height()) * h as f64).clamp(0.0, h as f64 - 1.0) as u32;
            let k = ((y * w + x) * 4) as usize;
            let a = rgba[k + 3] as f64 / 255.0;
            if a < 0.02 {
                continue;
            }
            let rgb = [rgba[k] as f64, rgba[k + 1] as f64, rgba[k + 2] as f64];
            let (color, size) = match method {
                // uniform: transparency flattened on white
                0 => ([rgb[0] * a + 255.0 * (1.0 - a), rgb[1] * a + 255.0 * (1.0 - a), rgb[2] * a + 255.0 * (1.0 - a)], 1.0),
                // size by opacity
                1 => (rgb, a.sqrt()),
                // size by darkness
                _ => (rgb, (1.0 - (0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]) / 255.0).max(0.0).sqrt() * a.sqrt()),
            };
            if size < 0.05 {
                continue;
            }
            tiles.push(Tile { at: p, size, color });
        }
    }
    (tiles, step)
}

fn pointillizer_apply(app: &mut AnyVectorApp) -> Result<(), String> {
    let fx = &app.dockers.ui.fx;
    let density = fx.pt_density;
    let step = 72.0 / density.max(0.01).sqrt();
    // two samples per tile are enough
    let (rgba, w, h, bb) = render_selection(app, (2.0 / step).clamp(0.02, 4.0)).ok_or("Nothing to render.")?;
    let (mut tiles, step) = pointillize(&rgba, w, h, bb, density, fx.pt_angle, fx.pt_method);
    if tiles.is_empty() {
        return Err("The selection has nothing to turn into tiles.".into());
    }
    if tiles.len() > 60000 {
        return Err(format!("{} tiles is too many: lower the density.", tiles.len()));
    }
    if fx.pt_limit {
        let colors: Vec<[f64; 3]> = tiles.iter().map(|t| t.color).collect();
        let (pal, idx) = limit_colors(&colors, fx.pt_colors as usize);
        for (t, i) in tiles.iter_mut().zip(idx) {
            t.color = pal[i];
        }
    }
    let size = step * fx.pt_scale;
    let base = tile_shape(fx.pt_shape, fx.pt_custom.as_ref(), size);
    let rot = Affine::rotate(-fx.pt_angle.to_radians());
    let merge = fx.pt_merge.max(1) as usize;
    let weld = fx.pt_weld;
    let keep = fx.pt_keep;
    // merge runs of same-colored neighbours in a row into one longer tile
    let key = |c: &[f64; 3]| [c[0].round() as u8, c[1].round() as u8, c[2].round() as u8];
    let mut shapes: Vec<(BezPath, [u8; 3])> = vec![];
    let mut i = 0;
    while i < tiles.len() {
        let t = &tiles[i];
        let mut j = i + 1;
        while merge > 1 && j < tiles.len() && j - i < merge && key(&tiles[j].color) == key(&t.color) && ((tiles[j].at - tiles[j - 1].at).hypot() - step).abs() < step * 0.01 {
            j += 1;
        }
        let n = j - i;
        let s = t.size;
        let shape = if n > 1 {
            let len = step * (n - 1) as f64;
            let r = size * s / 2.0;
            let cap = kurbo::RoundedRect::new(-r, -r, len + r, r, if fx.pt_shape == 1 { 0.0 } else { r });
            Affine::translate(t.at.to_vec2()) * rot * cap.to_path(0.1)
        } else {
            Affine::translate(t.at.to_vec2()) * rot * Affine::scale(s) * base.clone()
        };
        shapes.push((shape, key(&t.color)));
        i = j;
    }
    let style = |c: [u8; 3], even_odd: bool| Style { fill: Fill::Solid(Color::rgb(c[0], c[1], c[2])), outline: Outline { color: None, width: 0.0, ..Outline::default() }, even_odd, ..Default::default() };
    place_result(app, "Pointillizer", keep, move |doc| {
        let mut objs = vec![];
        if weld {
            // one curve per color: the tiles of a color that overlap run together (nonzero fill)
            let mut by: std::collections::BTreeMap<[u8; 3], BezPath> = Default::default();
            for (p, c) in shapes {
                by.entry(c).or_default().extend(p);
            }
            for (c, p) in by {
                objs.push(Object::new(doc.new_id(), ObjKind::Path(NodePath::from_bez(&p)), Affine::IDENTITY, style(c, false)));
            }
        } else {
            for (p, c) in shapes {
                objs.push(Object::new(doc.new_id(), ObjKind::Path(NodePath::from_bez(&p)), Affine::IDENTITY, style(c, true)));
            }
        }
        let mut g = Object::new(doc.new_id(), ObjKind::Group(objs), Affine::IDENTITY, Style::default());
        g.name = "Pointillizer".into();
        vec![g]
    });
    Ok(())
}

pub fn pointillizer(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        no_doc(ui);
        return;
    }
    if let Some(p) = app.ts.picked_shape.take() {
        app.dockers.ui.fx.pt_custom = Some(p);
        app.dockers.ui.fx.pt_shape = 2;
    }
    let lw = 104.0;
    let fx = &mut app.dockers.ui.fx;
    ui.add_space(4.0);
    form_row(ui, lw, "Density:", true, |ui| spin(ui, &mut fx.pt_density, 0.1..=1000.0, 1.0, 0, "", 80.0));
    form_row(ui, lw, "Scale:", true, |ui| spin(ui, &mut fx.pt_scale, 0.1..=10.0, 0.1, 1, "", 80.0));
    form_row(ui, lw, "Screen angle:", true, |ui| {
        spin(ui, &mut fx.pt_angle, -360.0..=360.0, 1.0, 1, "", 80.0);
        ui.label("°");
    });
    form_row(ui, lw, "", true, |ui| ui.checkbox(&mut fx.pt_keep, "Keep original source"));
    section(ui, "");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(lw, 22.0), Sense::hover());
        ui.put(r, egui::Checkbox::new(&mut fx.pt_limit, "Limit colors:"));
        ui.add_enabled_ui(fx.pt_limit, |ui| spin(ui, &mut fx.pt_colors, 1..=256, 1.0, 0, "", 80.0));
    });
    section(ui, "Tracking");
    form_row(ui, lw, "Method:", true, |ui| choice(ui, "pt_method", &mut fx.pt_method, &["Uniform (white matte)", "Size modulation 1 (opacity)", "Size modulation 2 (luminosity)"], 160.0));
    form_row(ui, lw, "Merge adjacent:", true, |ui| spin(ui, &mut fx.pt_merge, 1..=20, 1.0, 0, "", 80.0));
    ui.checkbox(&mut fx.pt_weld, "Weld adjacent overlap");
    ui.add_space(6.0);
    let custom = fx.pt_shape == 2;
    let mut select = false;
    form_row(ui, lw, "Shape:", true, |ui| {
        choice(ui, "pt_shape", &mut fx.pt_shape, &["Circle", "Square", "Custom"], 80.0);
        select = ui.add_enabled(custom, egui::Button::new("Select").min_size(vec2(80.0, 22.0))).on_hover_text("Click a closed curve to use as the tile").clicked();
    });
    if select {
        app.ts.pick_target = Some(crate::tools::PickTarget::TileShape);
        app.flash("Click a closed curve.");
    }
    // the tile
    let fx = &app.dockers.ui.fx;
    form_row(ui, lw, "", true, |ui| {
        let (r, _) = ui.allocate_exact_size(vec2(96.0, 96.0), Sense::hover());
        ui.painter().rect_filled(r, 0.0, Color32::WHITE);
        let shape = tile_shape(fx.pt_shape, fx.pt_custom.as_ref(), 80.0);
        let pts: Vec<egui::Pos2> = vector_core::geom::flatten_polylines(&shape, 0.5).into_iter().flat_map(|(p, _)| p).map(|p| r.center() + vec2(p.x as f32, p.y as f32)).collect();
        if pts.len() > 2 {
            ui.painter().add(egui::Shape::convex_polygon(pts, Color32::BLACK, Stroke::NONE));
        }
    });
    section(ui, "");
    let n = selected_ids(app).len();
    if apply_bar(ui, "Apply", n > 0) {
        if let Err(e) = pointillizer_apply(app) {
            app.flash(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_hides_or_keeps_colors() {
        let mut px = vec![255, 0, 0, 255, 0, 0, 255, 255];
        let rows = [(true, Color::rgb(250, 5, 0), 5.0)];
        mask_pixels(&mut px, &rows, true);
        assert_eq!((px[3], px[7]), (0, 255), "red hidden, blue kept");
        let mut px = vec![255, 0, 0, 255, 0, 0, 255, 255];
        mask_pixels(&mut px, &rows, false);
        assert_eq!((px[3], px[7]), (255, 0), "only red shown");
    }

    #[test]
    fn tiles_avoid_repeats_nearby() {
        let cells = vec![[10.0, 10.0, 10.0]; 4];
        let lib = [[0.0, 0.0, 0.0], [50.0, 50.0, 50.0], [200.0, 200.0, 200.0]];
        // no repeats: the three pictures are used once before any comes back
        let p = match_tiles(&cells, 4, &lib, None);
        assert_eq!(&p[..3], &[0, 1, 2]);
        // spacing 1: neighbours differ
        let p = match_tiles(&cells, 4, &lib, Some(1));
        assert!(p.windows(2).all(|w| w[0] != w[1]), "{p:?}");
    }

    #[test]
    fn pointillize_follows_opacity_and_limits_colors() {
        // left half opaque red, right half transparent
        let (w, h) = (20u32, 20u32);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w / 2 {
                let k = ((y * w + x) * 4) as usize;
                rgba[k] = 255;
                rgba[k + 3] = 255;
            }
        }
        let bb = Rect::new(0.0, 0.0, 72.0, 72.0);
        let (tiles, _) = pointillize(&rgba, w, h, bb, 100.0, 0.0, 1);
        assert!(!tiles.is_empty());
        assert!(tiles.iter().all(|t| t.at.x < 37.0), "no tiles on the transparent half");
        let colors = vec![[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [250.0, 250.0, 250.0], [255.0, 255.0, 255.0]];
        let (pal, idx) = limit_colors(&colors, 2);
        assert_eq!(pal.len(), 2);
        assert_eq!(idx[0], idx[1]);
        assert_ne!(idx[0], idx[3]);
    }
}
