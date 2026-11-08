//! Property bar: context-sensitive controls for the active tool and selection.
//! With no document / no selection it shows the page controls exactly as in the screenshots.

use crate::app::VectorApp;
use crate::commands::Cmd;
use crate::theme;
use crate::tools::ToolId;
use crate::widgets::{icon_button_sized, unit_field, vsep};
use egui::{vec2, Ui};
use kurbo::{Affine, Point};
use vector_core::object::{CornerType, EllipseKind, PolygonKind};
use vector_core::{ObjKind, Object, Unit};

fn btn(app: &mut VectorApp, ui: &mut Ui, icon: &str, tip: &str, enabled: bool, checked: bool) -> egui::Response {
    icon_button_sized(&mut app.icons, ui, icon, tip, enabled, checked, 30.0, 20.0)
}

/// Two stacked controls (e.g. width over height) in the 44px tall bar.
fn stacked(ui: &mut Ui, f: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add_space(1.0);
        f(ui);
    });
}

fn small_icon(app: &mut VectorApp, ui: &mut Ui, icon: &str, enabled: bool) {
    if let Some(img) = app.icons.image(ui.ctx(), icon, 16.0, enabled) {
        ui.add(img);
    }
}

/// Modify every selected object as one undo step.
pub fn edit_sel(app: &mut VectorApp, label: &str, mut f: impl FnMut(&mut Object)) {
    let Some(d) = app.doc_mut() else { return };
    if d.selection.is_empty() {
        return;
    }
    let sel = d.selection.clone();
    d.modify(label, |doc, _| {
        for id in &sel {
            if let Some(o) = doc.find_mut(*id) {
                f(o);
            }
        }
    });
}

pub fn property_bar(app: &mut VectorApp, ui: &mut Ui) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.add_space(4.0);
        let has_sel = app.has_selection();
        let t = app.tool;
        if !app.has_doc() {
            page_controls(app, ui);
            return;
        }
        match t {
            ToolId::Pick | ToolId::FreehandPick | ToolId::FreeTransform if !has_sel => page_controls(app, ui),
            ToolId::Pick | ToolId::FreehandPick | ToolId::FreeTransform => object_controls(app, ui),
            ToolId::Shape => shape_controls(app, ui),
            ToolId::Smooth | ToolId::Smear | ToolId::Twirl | ToolId::AttractRepel | ToolId::Smudge | ToolId::Roughen => brush_tool_controls(app, ui),
            ToolId::Zoom | ToolId::Pan => zoom_controls(app, ui),
            ToolId::Text => {
                text_controls(app, ui);
            }
            ToolId::Rectangle | ToolId::Rect3 => {
                rect_controls(app, ui);
                outline_controls(app, ui);
            }
            ToolId::Ellipse | ToolId::Ellipse3 => {
                ellipse_controls(app, ui);
                outline_controls(app, ui);
            }
            ToolId::Polygon | ToolId::Star => {
                polygon_controls(app, ui);
                outline_controls(app, ui);
            }
            ToolId::Spiral => spiral_controls(app, ui),
            ToolId::GraphPaper => {
                ui.label("Columns:");
                ui.add(egui::DragValue::new(&mut app.settings.graph_cols).range(1..=99));
                ui.label("Rows:");
                ui.add(egui::DragValue::new(&mut app.settings.graph_rows).range(1..=99));
            }
            ToolId::Table => {
                ui.label("Rows:");
                ui.add(egui::DragValue::new(&mut app.settings.table_rows).range(1..=99));
                ui.label("Columns:");
                ui.add(egui::DragValue::new(&mut app.settings.table_cols).range(1..=99));
                if has_sel {
                    vsep(ui);
                    table_controls(app, ui);
                }
            }
            ToolId::Freehand | ToolId::TwoPointLine | ToolId::Bezier | ToolId::Pen | ToolId::BSpline | ToolId::Polyline | ToolId::ThreePointCurve | ToolId::LiveSketch | ToolId::SmartDrawing => {
                curve_controls(app, ui);
            }
            ToolId::ArtisticMedia | ToolId::PainterlyBrush => media_controls(app, ui),
            ToolId::Eraser => {
                ui.label("Eraser thickness:");
                let (u, dpi) = unit_dpi(app);
                unit_field(ui, ui.scope_id().with("eraser_w"), &mut app.settings.eraser_thickness, u, dpi, 64.0, true);
                ui.checkbox(&mut app.settings.eraser_reduce_nodes, "Auto-reduce nodes");
            }
            ToolId::Crop => {
                let busy = app.ts.crop_rect.is_some();
                if ui.add_enabled(busy, egui::Button::new("Crop")).clicked() {
                    app.finish_tool();
                }
                if ui.add_enabled(busy, egui::Button::new("Clear crop marquee")).clicked() {
                    app.ts.cancel();
                }
            }
            ToolId::Knife | ToolId::VirtualDelete => {
                ui.label(egui::RichText::new(app.ts.hint(t)).color(theme::TEXT_DIM));
            }
            ToolId::ParallelDim | ToolId::HvDim | ToolId::AngularDim | ToolId::SegmentDim | ToolId::Callout => dimension_controls(app, ui),
            ToolId::StraightConnector | ToolId::RightAngleConnector | ToolId::RoundedConnector | ToolId::EditAnchor => outline_controls(app, ui),
            ToolId::DropShadow | ToolId::InnerShadow | ToolId::BlockShadow => shadow_controls(app, ui),
            ToolId::Contour => contour_controls(app, ui),
            ToolId::Blend => blend_controls(app, ui),
            ToolId::Distort => distort_controls(app, ui),
            ToolId::Envelope => {
                let enabled = has_sel;
                if ui.add_enabled(enabled, egui::Button::new("Clear Envelope")).clicked() {
                    edit_sel(app, "Clear Envelope", |o| o.effects.envelope = None);
                }
                ui.label(egui::RichText::new("Drag the handles to shape the envelope").color(theme::TEXT_DIM));
            }
            ToolId::Extrude => extrude_controls(app, ui),
            ToolId::Transparency => transparency_controls(app, ui),
            ToolId::ColorEyedropper | ToolId::AttributesEyedropper => {
                ui.label(egui::RichText::new(app.ts.hint(t)).color(theme::TEXT_DIM));
            }
            ToolId::InteractiveFill | ToolId::MeshFill | ToolId::SmartFill => fill_controls(app, ui),
            ToolId::CommonShapes => common_shape_controls(app, ui),
        }
    });
}

fn unit_dpi(app: &VectorApp) -> (Unit, f64) {
    app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0))
}

// ----------------------------------------------------------------------------- page controls

fn page_controls(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    let presets = vector_core::units::page_presets();
    let (pw, ph, unit, dpi) = app.doc().map(|d| (d.page_ref().width, d.page_ref().height, d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((612.0, 792.0, Unit::Inches, 300.0));
    let current = presets.iter().find(|(_, w, h)| ((w - pw).abs() < 0.5 && (h - ph).abs() < 0.5) || ((w - ph).abs() < 0.5 && (h - pw).abs() < 0.5)).map(|p| p.0).unwrap_or("Custom");
    ui.add_enabled_ui(doc, |ui| {
        egui::ComboBox::from_id_salt("page_size").selected_text(current).width(150.0).height(400.0).show_ui(ui, |ui| {
            for (name, w, h) in &presets {
                if ui.selectable_label(*name == current, *name).clicked() {
                    let land = pw > ph;
                    let (w, h) = if land { (*h, *w) } else { (*w, *h) };
                    set_page_size(app, w, h);
                }
            }
        });
    });
    vsep(ui);
    stacked(ui, |ui| {
        let mut w = pw;
        let mut h = ph;
        ui.horizontal(|ui| {
            small_icon(app, ui, "page_width", doc);
            if unit_field(ui, egui::Id::unique("pg_w"), &mut w, unit, dpi, 84.0, doc) {
                set_page_size(app, w.max(1.0), ph);
            }
        });
        ui.horizontal(|ui| {
            small_icon(app, ui, "page_height", doc);
            if unit_field(ui, egui::Id::unique("pg_h"), &mut h, unit, dpi, 84.0, doc) {
                set_page_size(app, pw, h.max(1.0));
            }
        });
    });
    vsep(ui);
    let land = pw > ph;
    if btn(app, ui, "portrait", "Portrait", doc, false).clicked() && land {
        app.run(Cmd::Landscape(false));
    }
    if btn(app, ui, "landscape", "Landscape", doc, false).clicked() && !land {
        app.run(Cmd::Landscape(true));
    }
    vsep(ui);
    let all = app.doc().map(|d| d.doc.settings.apply_to_all_pages).unwrap_or(true);
    if btn(app, ui, "all_pages", "All Pages", doc, false).clicked() {
        if let Some(d) = app.doc_mut() {
            d.doc.settings.apply_to_all_pages = true;
        }
    }
    if btn(app, ui, "current_page", "Current Page", doc && all, false).clicked() {
        if let Some(d) = app.doc_mut() {
            d.doc.settings.apply_to_all_pages = false;
        }
    }
    vsep(ui);
    ui.label(egui::RichText::new("Units:").color(if doc { theme::TEXT } else { theme::DISABLED }));
    ui.add_enabled_ui(doc, |ui| {
        let txt = if doc { unit.name() } else { "" };
        egui::ComboBox::from_id_salt("units").selected_text(txt).width(110.0).show_ui(ui, |ui| {
            for u in Unit::ALL {
                if ui.selectable_label(u == unit, u.name()).clicked() {
                    if let Some(d) = app.doc_mut() {
                        d.doc.settings.unit = u;
                    }
                    app.settings.default_unit = u;
                }
            }
        });
    });
    vsep(ui);
    // nudge
    small_icon(app, ui, "nudge", doc);
    let mut nudge = app.doc().map(|d| d.doc.settings.nudge).unwrap_or(0.0);
    if unit_field(ui, egui::Id::unique("nudge"), &mut nudge, unit, dpi, 74.0, doc) {
        if let Some(d) = app.doc_mut() {
            d.doc.settings.nudge = nudge.max(0.0);
        }
    }
    vsep(ui);
    // duplicate distance
    let dup = app.doc().map(|d| d.doc.settings.duplicate_offset).unwrap_or(kurbo::Vec2::new(18.0, 18.0));
    stacked(ui, |ui| {
        let mut x = dup.x;
        let mut y = dup.y;
        ui.horizontal(|ui| {
            small_icon(app, ui, "dup_x", doc);
            if unit_field(ui, egui::Id::unique("dup_x"), &mut x, unit, dpi, 74.0, doc) {
                if let Some(d) = app.doc_mut() {
                    d.doc.settings.duplicate_offset.x = x;
                }
            }
        });
        ui.horizontal(|ui| {
            small_icon(app, ui, "dup_y", doc);
            if unit_field(ui, egui::Id::unique("dup_y"), &mut y, unit, dpi, 74.0, doc) {
                if let Some(d) = app.doc_mut() {
                    d.doc.settings.duplicate_offset.y = y;
                }
            }
        });
    });
    vsep(ui);
    let taf = app.settings.treat_as_filled;
    if btn(app, ui, "treat_filled", "Treat all objects as filled", true, taf).clicked() {
        app.settings.treat_as_filled = !taf;
    }
    vsep(ui);
    let so = app.ts.scale_outline_with_object;
    if btn(app, ui, "scale_outline", "Scale outline with object", doc, so && doc).clicked() {
        app.ts.scale_outline_with_object = !so;
    }
    let tf = app.ts.transform_fill_with_object;
    if btn(app, ui, "transform_fill", "Transform fill with object", doc, tf && doc).clicked() {
        app.ts.transform_fill_with_object = !tf;
    }
    vsep(ui);
    let r = btn(app, ui, "plus", "Quick Customize", true, false);
    customize_popup(app, &r);
}

fn customize_popup(app: &mut VectorApp, r: &egui::Response) {
    egui::Popup::menu(r).show(|ui| {
        ui.set_min_width(220.0);
        let mut on = app.settings.show_property_bar;
        if ui.checkbox(&mut on, "Property bar").changed() {
            app.settings.show_property_bar = on;
        }
        if ui.button("Customize...").clicked() {
            app.run(Cmd::Options(crate::commands::OptionsPage::Customization));
            app.dialogs.options.page = Some("Command Bars".into());
        }
    });
}

fn set_page_size(app: &mut VectorApp, w: f64, h: f64) {
    let Some(d) = app.doc_mut() else { return };
    let all = d.doc.settings.apply_to_all_pages;
    let cur = d.page;
    d.modify("Page Size", |doc, _| {
        for (i, p) in doc.pages.iter_mut().enumerate() {
            if all || i == cur {
                p.width = w;
                p.height = h;
            }
        }
    });
    d.needs_fit = true;
}

// ----------------------------------------------------------------------------- object (Pick tool with selection)

fn object_controls(app: &mut VectorApp, ui: &mut Ui) {
    let Some(d) = app.doc() else { return };
    let Some(bb) = d.selection_bounds() else { return };
    let unit = d.doc.settings.unit;
    let dpi = d.doc.settings.dpi;
    let single = d.selection.len() == 1;
    let first = d.doc.find(d.selection[0]).cloned();
    let angle = first.as_ref().filter(|_| single).map(|o| vector_core::geom::rotation_deg(o.transform)).unwrap_or(0.0);
    let ruler_origin = d.doc.settings.ruler_origin;
    // position (center)
    stacked(ui, |ui| {
        let mut x = bb.center().x - ruler_origin.x;
        let mut y = bb.center().y - ruler_origin.y;
        ui.horizontal(|ui| {
            ui.label("X:");
            if unit_field(ui, egui::Id::unique("obj_x"), &mut x, unit, dpi, 80.0, true) {
                let dx = x + app.doc().unwrap().doc.settings.ruler_origin.x - bb.center().x;
                transform_sel(app, "Move", Affine::translate((dx, 0.0)));
            }
        });
        ui.horizontal(|ui| {
            ui.label("Y:");
            if unit_field(ui, egui::Id::unique("obj_y"), &mut y, unit, dpi, 80.0, true) {
                let dy = y + app.doc().unwrap().doc.settings.ruler_origin.y - bb.center().y;
                transform_sel(app, "Move", Affine::translate((0.0, dy)));
            }
        });
    });
    vsep(ui);
    let lock = app.ts.lock_ratio;
    stacked(ui, |ui| {
        let mut w = bb.width();
        let mut h = bb.height();
        ui.horizontal(|ui| {
            small_icon(app, ui, "page_width", true);
            if unit_field(ui, egui::Id::unique("obj_w"), &mut w, unit, dpi, 80.0, true) && bb.width() > 0.0 {
                let sx = w.max(0.001) / bb.width();
                let sy = if lock { sx } else { 1.0 };
                scale_sel(app, sx, sy);
            }
        });
        ui.horizontal(|ui| {
            small_icon(app, ui, "page_height", true);
            if unit_field(ui, egui::Id::unique("obj_h"), &mut h, unit, dpi, 80.0, true) && bb.height() > 0.0 {
                let sy = h.max(0.001) / bb.height();
                let sx = if lock { sy } else { 1.0 };
                scale_sel(app, sx, sy);
            }
        });
    });
    stacked(ui, |ui| {
        let mut sx = 100.0;
        let mut sy = 100.0;
        if ui.add(egui::DragValue::new(&mut sx).suffix(" %").range(1.0..=10000.0).speed(0.5)).changed() {
            scale_sel(app, sx / 100.0, if lock { sx / 100.0 } else { 1.0 });
        }
        if ui.add(egui::DragValue::new(&mut sy).suffix(" %").range(1.0..=10000.0).speed(0.5)).changed() {
            scale_sel(app, if lock { sy / 100.0 } else { 1.0 }, sy / 100.0);
        }
    });
    if btn(app, ui, if lock { "lock" } else { "unlock" }, "Lock ratio", true, lock).clicked() {
        app.ts.lock_ratio = !lock;
    }
    vsep(ui);
    small_icon(app, ui, "rotate", true);
    let mut a = angle;
    if ui.add(egui::DragValue::new(&mut a).suffix(" °").speed(0.5).max_decimals(1)).changed() {
        let c = bb.center();
        let m = Affine::translate(c.to_vec2()) * Affine::rotate(-(a - angle).to_radians()) * Affine::translate(-c.to_vec2());
        transform_sel(app, "Rotate", m);
    }
    vsep(ui);
    if btn(app, ui, "mirror_h", "Mirror Horizontally", true, false).clicked() {
        app.run(Cmd::MirrorH);
    }
    if btn(app, ui, "mirror_v", "Mirror Vertically", true, false).clicked() {
        app.run(Cmd::MirrorV);
    }
    vsep(ui);
    // object-type specific
    if let Some(o) = first.filter(|_| single) {
        match &o.kind {
            ObjKind::Rect(_) => rect_controls(app, ui),
            ObjKind::Ellipse(_) => ellipse_controls(app, ui),
            ObjKind::Polygon(_) => polygon_controls(app, ui),
            ObjKind::Text(_) => {
                text_controls(app, ui);
                return;
            }
            ObjKind::Table(_) => table_controls(app, ui),
            ObjKind::Image(_) => {
                if ui.button("Trace Bitmap").on_hover_text("Quick Trace").clicked() {
                    app.run(Cmd::QuickTrace);
                }
            }
            _ => {}
        }
    }
    outline_controls(app, ui);
    vsep(ui);
    let n = app.doc().map(|d| d.selection.len()).unwrap_or(0);
    let is_group = app.doc().and_then(|d| d.doc.find(d.selection[0])).map(|o| matches!(o.kind, ObjKind::Group(_))).unwrap_or(false);
    if btn(app, ui, "group", "Group (Ctrl+G)", n > 1, false).clicked() {
        app.run(Cmd::Group);
    }
    if btn(app, ui, "ungroup", "Ungroup (Ctrl+U)", is_group, false).clicked() {
        app.run(Cmd::Ungroup);
    }
    vsep(ui);
    if n > 1 {
        for (icon, tip, op) in [
            ("weld", "Weld", vector_core::ops::Shaping::Weld),
            ("trim", "Trim", vector_core::ops::Shaping::Trim),
            ("intersect", "Intersect", vector_core::ops::Shaping::Intersect),
            ("simplify", "Simplify", vector_core::ops::Shaping::Simplify),
            ("front_minus_back", "Front Minus Back", vector_core::ops::Shaping::FrontMinusBack),
            ("back_minus_front", "Back Minus Front", vector_core::ops::Shaping::BackMinusFront),
            ("boundary", "Create Boundary", vector_core::ops::Shaping::Boundary),
        ] {
            if btn(app, ui, icon, tip, true, false).clicked() {
                app.run(Cmd::Shaping(op));
            }
        }
        vsep(ui);
    }
    if btn(app, ui, "to_curves", "Convert to Curves (Ctrl+Q)", true, false).clicked() {
        app.run(Cmd::ConvertToCurves);
    }
    if btn(app, ui, "align", "Align and Distribute (Ctrl+Shift+A)", true, false).clicked() {
        app.run(Cmd::Docker(crate::dockers::DockerId::AlignDistribute));
    }
}

pub fn transform_sel(app: &mut VectorApp, label: &str, m: Affine) {
    let Some(d) = app.doc_mut() else { return };
    let sel = d.selection.clone();
    d.modify(label, |doc, _| doc.transform_objects(&sel, m));
}

fn scale_sel(app: &mut VectorApp, sx: f64, sy: f64) {
    let Some(bb) = app.doc().and_then(|d| d.selection_bounds()) else { return };
    let c = bb.center();
    let m = Affine::translate(c.to_vec2()) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-c.to_vec2());
    transform_sel(app, "Scale", m);
}

// ----------------------------------------------------------------------------- shapes

fn selected_kind<T>(app: &VectorApp, f: impl Fn(&ObjKind) -> Option<T>) -> Option<T> {
    let d = app.doc()?;
    if d.selection.len() != 1 {
        return None;
    }
    f(&d.doc.find(d.selection[0])?.kind)
}

fn rect_controls(app: &mut VectorApp, ui: &mut Ui) {
    let sel = selected_kind(app, |k| if let ObjKind::Rect(r) = k { Some(r.clone()) } else { None });
    let (mut corner, mut radii) = match &sel {
        Some(r) => (r.corner, r.radii),
        None => (
            match app.settings.rect_corner {
                1 => CornerType::Scallop,
                2 => CornerType::Chamfer,
                _ => CornerType::Round,
            },
            app.settings.rect_radii,
        ),
    };
    let mut changed = false;
    for (icon, tip, ct) in [("corners", "Round corner", CornerType::Round), ("corners", "Scalloped corner", CornerType::Scallop), ("corners", "Chamfered corner", CornerType::Chamfer)] {
        if btn(app, ui, icon, tip, true, corner == ct).clicked() {
            corner = ct;
            changed = true;
        }
    }
    let (u, dpi) = unit_dpi(app);
    stacked(ui, |ui| {
        ui.horizontal(|ui| {
            changed |= unit_field(ui, egui::Id::unique("rr0"), &mut radii[0], u, dpi, 60.0, true);
            changed |= unit_field(ui, egui::Id::unique("rr1"), &mut radii[1], u, dpi, 60.0, true);
        });
        ui.horizontal(|ui| {
            changed |= unit_field(ui, egui::Id::unique("rr3"), &mut radii[3], u, dpi, 60.0, true);
            changed |= unit_field(ui, egui::Id::unique("rr2"), &mut radii[2], u, dpi, 60.0, true);
        });
    });
    let locked = app.ts.lock_corners;
    if btn(app, ui, if locked { "lock" } else { "unlock" }, "Edit corners together", true, locked).clicked() {
        app.ts.lock_corners = !locked;
    }
    if changed {
        if app.ts.lock_corners {
            // the edited value wins: pick the one that differs from the others
            let v = radii.iter().copied().find(|r| radii.iter().filter(|x| (*x - r).abs() < 1e-9).count() == 1).unwrap_or(radii[0]);
            radii = [v.max(0.0); 4];
        }
        for r in radii.iter_mut() {
            *r = r.max(0.0);
        }
        app.settings.rect_radii = radii;
        app.settings.rect_corner = match corner {
            CornerType::Round => 0,
            CornerType::Scallop => 1,
            CornerType::Chamfer => 2,
        };
        if sel.is_some() {
            edit_sel(app, "Corners", |o| {
                if let ObjKind::Rect(r) = &mut o.kind {
                    r.corner = corner;
                    r.radii = radii;
                }
            });
        }
    }
    vsep(ui);
}

fn ellipse_controls(app: &mut VectorApp, ui: &mut Ui) {
    let sel = selected_kind(app, |k| if let ObjKind::Ellipse(e) = k { Some(e.clone()) } else { None });
    let (mut kind, mut start, mut end) = match &sel {
        Some(e) => (e.kind, e.start, e.end),
        None => (
            match app.settings.ellipse_kind {
                1 => EllipseKind::Pie,
                2 => EllipseKind::Arc,
                _ => EllipseKind::Ellipse,
            },
            app.settings.ellipse_start,
            app.settings.ellipse_end,
        ),
    };
    let mut changed = false;
    for (icon, tip, k) in [("ellipse", "Ellipse", EllipseKind::Ellipse), ("ellipse3", "Pie", EllipseKind::Pie), ("three_point_curve", "Arc", EllipseKind::Arc)] {
        if btn(app, ui, icon, tip, true, kind == k).clicked() {
            kind = k;
            changed = true;
        }
    }
    let en = kind != EllipseKind::Ellipse;
    stacked(ui, |ui| {
        changed |= ui.add_enabled(en, egui::DragValue::new(&mut start).suffix(" °").range(-360.0..=360.0)).changed();
        changed |= ui.add_enabled(en, egui::DragValue::new(&mut end).suffix(" °").range(-360.0..=360.0)).changed();
    });
    if changed {
        app.settings.ellipse_kind = kind as u8;
        app.settings.ellipse_start = start;
        app.settings.ellipse_end = end;
        if sel.is_some() {
            edit_sel(app, "Ellipse", |o| {
                if let ObjKind::Ellipse(e) = &mut o.kind {
                    e.kind = kind;
                    e.start = start;
                    e.end = end;
                }
            });
        }
    }
    vsep(ui);
}

fn polygon_controls(app: &mut VectorApp, ui: &mut Ui) {
    let sel = selected_kind(app, |k| if let ObjKind::Polygon(p) = k { Some(p.clone()) } else { None });
    let star_tool = app.tool == ToolId::Star;
    let (mut kind, mut sides, mut sharp) = match &sel {
        Some(p) => (p.kind, p.sides, p.sharpness),
        None if star_tool => (PolygonKind::Star, app.settings.star_points, app.settings.star_sharpness),
        None => (PolygonKind::Polygon, app.settings.polygon_sides, app.settings.star_sharpness),
    };
    let mut changed = false;
    if star_tool || matches!(kind, PolygonKind::Star | PolygonKind::ComplexStar) {
        for (icon, tip, k) in [("star", "Star", PolygonKind::Star), ("complex_star", "Complex Star", PolygonKind::ComplexStar)] {
            if btn(app, ui, icon, tip, true, kind == k).clicked() {
                kind = k;
                changed = true;
            }
        }
    }
    small_icon(app, ui, if kind == PolygonKind::Polygon { "polygon" } else { "star" }, true);
    changed |= ui.add(egui::DragValue::new(&mut sides).range(3..=500)).on_hover_text("Points or sides").changed();
    if kind != PolygonKind::Polygon {
        ui.label("Sharpness:");
        changed |= ui.add(egui::DragValue::new(&mut sharp).range(1.0..=99.0).suffix("")).changed();
    }
    if changed {
        if star_tool || kind != PolygonKind::Polygon {
            app.settings.star_points = sides;
            app.settings.star_sharpness = sharp;
        } else {
            app.settings.polygon_sides = sides;
        }
        if sel.is_some() {
            edit_sel(app, "Polygon", |o| {
                if let ObjKind::Polygon(p) = &mut o.kind {
                    p.kind = kind;
                    p.sides = sides;
                    p.sharpness = sharp;
                }
            });
        }
    }
    vsep(ui);
}

fn spiral_controls(app: &mut VectorApp, ui: &mut Ui) {
    small_icon(app, ui, "spiral", true);
    ui.add(egui::DragValue::new(&mut app.settings.spiral_revs).range(1..=100)).on_hover_text("Spiral revolutions");
    vsep(ui);
    let log = app.settings.spiral_log;
    if btn(app, ui, "spiral", "Symmetrical spiral", true, !log).clicked() {
        app.settings.spiral_log = false;
    }
    if btn(app, ui, "twirl", "Logarithmic spiral", true, log).clicked() {
        app.settings.spiral_log = true;
    }
    ui.add_enabled(log, egui::Slider::new(&mut app.settings.spiral_expansion, 1.0..=100.0).text("Expansion"));
}

fn table_controls(app: &mut VectorApp, ui: &mut Ui) {
    for (label, cmd) in [("Insert Row", Cmd::TableInsertRow(false)), ("Insert Column", Cmd::TableInsertCol(false)), ("Delete Row", Cmd::TableDeleteRow), ("Delete Column", Cmd::TableDeleteCol)] {
        if ui.button(label).clicked() {
            app.run(cmd);
        }
    }
    vsep(ui);
}

fn common_shape_controls(app: &mut VectorApp, ui: &mut Ui) {
    ui.label("Shape:");
    let names = crate::tools::COMMON_SHAPES;
    let cur = app.ts.common_shape.min(names.len() - 1);
    egui::ComboBox::from_id_salt("common_shape").selected_text(names[cur]).width(150.0).show_ui(ui, |ui| {
        for (i, n) in names.iter().enumerate() {
            ui.selectable_value(&mut app.ts.common_shape, i, *n);
        }
    });
    outline_controls(app, ui);
}

// ----------------------------------------------------------------------------- outline

fn outline_controls(app: &mut VectorApp, ui: &mut Ui) {
    let sel_w = app.doc().and_then(|d| d.selection.first().and_then(|id| d.doc.find(*id))).map(|o| if o.style.outline.color.is_some() { o.style.outline.width } else { 0.0 });
    let mut w = sel_w.unwrap_or(app.ts.default_outline_width);
    let presets = vector_core::style::outline_width_presets();
    let (u, dpi) = unit_dpi(app);
    small_icon(app, ui, "outline_icon", true);
    let label = if w <= 0.0 { "None".to_string() } else { Unit::Points.format(w, dpi) };
    let _ = u;
    egui::ComboBox::from_id_salt("outline_w").selected_text(label).width(90.0).show_ui(ui, |ui| {
        if ui.selectable_label(w <= 0.0, "None").clicked() {
            w = 0.0;
            set_outline_width(app, 0.0);
        }
        for (name, pw) in &presets {
            if ui.selectable_label((w - pw).abs() < 1e-6, *name).clicked() {
                w = *pw;
                set_outline_width(app, *pw);
            }
        }
    });
}

fn set_outline_width(app: &mut VectorApp, w: f64) {
    if app.has_selection() {
        edit_sel(app, "Outline Width", |o| {
            o.style.outline.width = w;
            if w > 0.0 && o.style.outline.color.is_none() {
                o.style.outline.color = Some(vector_core::Color::BLACK);
            }
        });
    } else {
        app.ts.default_outline_width = w;
    }
}

// ----------------------------------------------------------------------------- shape tool (nodes)

fn shape_controls(app: &mut VectorApp, ui: &mut Ui) {
    let has_nodes = app.doc().map(|d| !d.node_sel.is_empty()).unwrap_or(false);
    let is_curve = selected_kind(app, |k| matches!(k, ObjKind::Path(_)).then_some(())).is_some();
    if !is_curve && app.has_selection() {
        if btn(app, ui, "to_curves", "Convert to Curves (Ctrl+Q)", true, false).clicked() {
            app.run(Cmd::ConvertToCurves);
        }
        vsep(ui);
    }
    use crate::tools::NodeOp;
    let ops: [(&str, &str, NodeOp, bool); 16] = [
        ("node_add", "Add Nodes", NodeOp::Add, has_nodes),
        ("node_delete", "Delete Nodes", NodeOp::Delete, has_nodes),
        ("node_join", "Join Two Nodes", NodeOp::Join, has_nodes),
        ("node_break", "Break Curve", NodeOp::Break, has_nodes),
        ("to_line", "Convert to Line", NodeOp::ToLine, has_nodes),
        ("to_curve", "Convert to Curve", NodeOp::ToCurve, has_nodes),
        ("cusp", "Make Node a Cusp", NodeOp::Cusp, has_nodes),
        ("smooth_node", "Make Node Smooth", NodeOp::Smooth, has_nodes),
        ("symmetric", "Make Node Symmetrical", NodeOp::Symmetric, has_nodes),
        ("reverse", "Reverse Direction", NodeOp::Reverse, is_curve),
        ("extend_close", "Extend Curve to Close", NodeOp::ExtendClose, has_nodes),
        ("close_curve", "Close Curve", NodeOp::Close, is_curve),
        ("extract", "Extract Subpath", NodeOp::Extract, has_nodes),
        ("reduce_nodes", "Reduce Nodes", NodeOp::Reduce, is_curve),
        ("select_all_nodes", "Select All Nodes", NodeOp::SelectAll, is_curve),
        ("elastic", "Elastic Mode", NodeOp::Elastic, true),
    ];
    for (i, (icon, tip, op, en)) in ops.iter().enumerate() {
        let checked = *op == NodeOp::Elastic && app.ts.elastic;
        if btn(app, ui, icon, tip, *en, checked).clicked() {
            crate::tools::node_op(app, *op);
        }
        if matches!(i, 3 | 5 | 8 | 12) {
            vsep(ui);
        }
    }
}

fn brush_tool_controls(app: &mut VectorApp, ui: &mut Ui) {
    let (u, dpi) = unit_dpi(app);
    ui.label("Nib size:");
    unit_field(ui, egui::Id::unique("brush_size"), &mut app.ts.brush_size, u, dpi, 64.0, true);
    ui.label("Strength:");
    ui.add(egui::DragValue::new(&mut app.ts.brush_strength).range(1.0..=100.0).suffix(" %"));
}

fn zoom_controls(app: &mut VectorApp, ui: &mut Ui) {
    let pct = app.doc().map(|d| d.zoom_percent()).unwrap_or(100.0);
    egui::ComboBox::from_id_salt("pb_zoom").selected_text(format!("{:.0}%", pct)).width(90.0).show_ui(ui, |ui| {
        for z in [10.0, 25.0, 50.0, 75.0, 100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0] {
            if ui.selectable_label((pct - z).abs() < 0.5, format!("{z:.0}%")).clicked() {
                app.run(Cmd::ZoomSet(z));
            }
        }
    });
    for (icon, tip, cmd) in [
        ("zoom_in", "Zoom In (F2)", Cmd::ZoomIn),
        ("zoom_out", "Zoom Out (F3)", Cmd::ZoomOut),
        ("zoom_selected", "Zoom to Selected (Shift+F2)", Cmd::ZoomToSelected),
        ("zoom_all", "Zoom to All Objects (F4)", Cmd::ZoomToAll),
        ("zoom_page", "Zoom to Page (Shift+F4)", Cmd::ZoomToPage),
        ("zoom_width", "Zoom to Page Width", Cmd::ZoomToWidth),
        ("zoom_height", "Zoom to Page Height", Cmd::ZoomToHeight),
    ] {
        let en = cmd != Cmd::ZoomToSelected || app.has_selection();
        if btn(app, ui, icon, tip, en, false).clicked() {
            app.run(cmd);
        }
    }
}

fn curve_controls(app: &mut VectorApp, ui: &mut Ui) {
    ui.label("Freehand smoothing:");
    ui.add(egui::Slider::new(&mut app.settings.freehand_smoothing, 0.0..=100.0).show_value(true));
    vsep(ui);
    outline_controls(app, ui);
    vsep(ui);
    let closed = app.ts.auto_close;
    if btn(app, ui, "close_curve", "Close curve automatically", true, closed).clicked() {
        app.ts.auto_close = !closed;
    }
}

fn media_controls(app: &mut VectorApp, ui: &mut Ui) {
    let (u, dpi) = unit_dpi(app);
    ui.label("Width:");
    unit_field(ui, egui::Id::unique("media_w"), &mut app.ts.media_width, u, dpi, 64.0, true);
    ui.label("Smoothing:");
    ui.add(egui::Slider::new(&mut app.settings.freehand_smoothing, 0.0..=100.0));
    vsep(ui);
    ui.label("Stroke:");
    let names = ["Taper", "Taper end", "Taper start", "Calligraphic", "Pressure", "Painterly"];
    let cur = app.ts.media_kind.min(names.len() - 1);
    egui::ComboBox::from_id_salt("media_kind").selected_text(names[cur]).width(110.0).show_ui(ui, |ui| {
        for (i, n) in names.iter().enumerate() {
            ui.selectable_value(&mut app.ts.media_kind, i, *n);
        }
    });
    if app.tool == ToolId::PainterlyBrush {
        app.ts.media_kind = 5;
    }
}

fn dimension_controls(app: &mut VectorApp, ui: &mut Ui) {
    ui.label("Precision:");
    ui.add(egui::DragValue::new(&mut app.settings.dim_precision).range(0..=6));
    ui.label("Prefix:");
    ui.add(egui::TextEdit::singleline(&mut app.settings.dim_prefix).desired_width(40.0));
    ui.label("Suffix:");
    ui.add(egui::TextEdit::singleline(&mut app.settings.dim_suffix).desired_width(40.0));
    vsep(ui);
    outline_controls(app, ui);
}

// ----------------------------------------------------------------------------- effects

fn sel_obj(app: &VectorApp) -> Option<Object> {
    let d = app.doc()?;
    d.doc.find(*d.selection.first()?).cloned()
}

fn shadow_controls(app: &mut VectorApp, ui: &mut Ui) {
    let o = sel_obj(app);
    let inner = app.tool == ToolId::InnerShadow;
    if app.tool == ToolId::BlockShadow {
        let bs = o.as_ref().and_then(|o| o.effects.block_shadow.clone());
        let mut s = bs.clone().unwrap_or(vector_core::object::BlockShadow { offset: kurbo::Vec2::new(8.0, 8.0), color: vector_core::Color::BLACK });
        let (u, dpi) = unit_dpi(app);
        let mut ch = false;
        ui.label("Offset:");
        ch |= unit_field(ui, egui::Id::unique("bs_x"), &mut s.offset.x, u, dpi, 64.0, bs.is_some());
        ch |= unit_field(ui, egui::Id::unique("bs_y"), &mut s.offset.y, u, dpi, 64.0, bs.is_some());
        if ch {
            edit_sel(app, "Block Shadow", |o| o.effects.block_shadow = Some(s.clone()));
        }
        if ui.add_enabled(bs.is_some(), egui::Button::new("Clear Block Shadow")).clicked() {
            edit_sel(app, "Clear Block Shadow", |o| o.effects.block_shadow = None);
        }
        return;
    }
    let cur = o.as_ref().and_then(|o| if inner { o.effects.inner_shadow.clone() } else { o.effects.drop_shadow.clone() });
    let en = cur.is_some();
    let mut s = cur.clone().unwrap_or_else(|| crate::tools::default_shadow(inner));
    let mut ch = false;
    let (u, dpi) = unit_dpi(app);
    stacked(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("X:");
            ch |= unit_field(ui, egui::Id::unique("sh_x"), &mut s.offset.x, u, dpi, 64.0, en);
        });
        ui.horizontal(|ui| {
            ui.label("Y:");
            ch |= unit_field(ui, egui::Id::unique("sh_y"), &mut s.offset.y, u, dpi, 64.0, en);
        });
    });
    vsep(ui);
    ui.label("Opacity:");
    ch |= ui.add_enabled(en, egui::DragValue::new(&mut s.opacity).range(0.0..=100.0)).changed();
    ui.label("Feather:");
    ch |= ui.add_enabled(en, egui::DragValue::new(&mut s.feather).range(0.0..=100.0)).changed();
    let mut c = theme::to_c32(s.color);
    if ui.add_enabled(en, egui::Button::new("").fill(c).min_size(vec2(22.0, 18.0))).on_hover_text("Shadow color").clicked() {}
    if en && egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::Opaque).changed() {
        s.color = theme::from_c32(c);
        ch = true;
    }
    if ch && en {
        edit_sel(app, "Shadow", |o| {
            if inner {
                o.effects.inner_shadow = Some(s.clone())
            } else {
                o.effects.drop_shadow = Some(s.clone())
            }
        });
    }
    vsep(ui);
    if ui.add_enabled(en, egui::Button::new("Clear Shadow")).clicked() {
        edit_sel(app, "Clear Shadow", |o| {
            if inner {
                o.effects.inner_shadow = None
            } else {
                o.effects.drop_shadow = None
            }
        });
    }
}

fn contour_controls(app: &mut VectorApp, ui: &mut Ui) {
    use vector_core::object::{ContourDir, ContourEffect};
    let cur = sel_obj(app).and_then(|o| o.effects.contour);
    let en = app.has_selection();
    let mut c = cur.clone().unwrap_or(ContourEffect { dir: ContourDir::Outside, steps: 3, offset: 6.0, end_fill: vector_core::Color::WHITE, end_outline: vector_core::Color::BLACK });
    let mut ch = false;
    for (icon, tip, dir) in [("contour", "To Center", ContourDir::ToCenter), ("contour", "Inside", ContourDir::Inside), ("contour", "Outside", ContourDir::Outside)] {
        if btn(app, ui, icon, tip, en, cur.is_some() && c.dir == dir).clicked() {
            c.dir = dir;
            ch = true;
        }
    }
    ui.label("Steps:");
    ch |= ui.add_enabled(en, egui::DragValue::new(&mut c.steps).range(1..=999)).changed();
    let (u, dpi) = unit_dpi(app);
    ui.label("Offset:");
    ch |= unit_field(ui, egui::Id::unique("ct_off"), &mut c.offset, u, dpi, 64.0, en);
    if ch && en {
        edit_sel(app, "Contour", |o| o.effects.contour = Some(c.clone()));
    }
    if ui.add_enabled(cur.is_some(), egui::Button::new("Clear Contour")).clicked() {
        edit_sel(app, "Clear Contour", |o| o.effects.contour = None);
    }
}

fn blend_controls(app: &mut VectorApp, ui: &mut Ui) {
    let cur = selected_kind(app, |k| if let ObjKind::Blend(b) = k { Some((b.steps, b.rotation, b.acceleration)) } else { None });
    let (mut steps, mut rot, mut acc) = cur.unwrap_or((app.ts.blend_steps, 0.0, 0.0));
    let mut ch = false;
    ui.label("Steps:");
    ch |= ui.add(egui::DragValue::new(&mut steps).range(1..=999)).changed();
    ui.label("Rotation:");
    ch |= ui.add_enabled(cur.is_some(), egui::DragValue::new(&mut rot).suffix(" °")).changed();
    ui.label("Acceleration:");
    ch |= ui.add_enabled(cur.is_some(), egui::Slider::new(&mut acc, -100.0..=100.0)).changed();
    if ch {
        app.ts.blend_steps = steps;
        if cur.is_some() {
            edit_sel(app, "Blend", |o| {
                if let ObjKind::Blend(b) = &mut o.kind {
                    b.steps = steps;
                    b.rotation = rot;
                    b.acceleration = acc;
                }
            });
        }
    }
    if ui.add_enabled(cur.is_some(), egui::Button::new("Break Blend Apart")).clicked() {
        app.run(Cmd::BreakBlend);
    }
}

fn distort_controls(app: &mut VectorApp, ui: &mut Ui) {
    use vector_core::object::{DistortEffect, DistortKind};
    let cur = sel_obj(app).and_then(|o| o.effects.distort);
    let en = app.has_selection();
    let mut dd = cur.clone().unwrap_or(DistortEffect { kind: app.ts.distort_kind, amount: 30.0, frequency: 6.0, center: Point::new(0.5, 0.5) });
    let mut ch = false;
    for (tip, k) in [("Push and Pull", DistortKind::PushPull), ("Zipper", DistortKind::Zipper), ("Twister", DistortKind::Twister)] {
        if btn(app, ui, "distort", tip, true, dd.kind == k).clicked() {
            dd.kind = k;
            app.ts.distort_kind = k;
            ch = true;
        }
    }
    ui.label("Amplitude:");
    ch |= ui.add_enabled(en, egui::DragValue::new(&mut dd.amount).range(-200.0..=200.0)).changed();
    if dd.kind == DistortKind::Zipper {
        ui.label("Frequency:");
        ch |= ui.add_enabled(en, egui::DragValue::new(&mut dd.frequency).range(0.0..=100.0)).changed();
    }
    if ch && cur.is_some() {
        edit_sel(app, "Distortion", |o| o.effects.distort = Some(dd.clone()));
    }
    if ui.add_enabled(cur.is_some(), egui::Button::new("Clear Distortion")).clicked() {
        edit_sel(app, "Clear Distortion", |o| o.effects.distort = None);
    }
}

fn extrude_controls(app: &mut VectorApp, ui: &mut Ui) {
    let cur = sel_obj(app).and_then(|o| o.effects.extrude);
    let Some(mut e) = cur.clone() else {
        ui.label(egui::RichText::new("Drag from an object to extrude it").color(theme::TEXT_DIM));
        return;
    };
    let (u, dpi) = unit_dpi(app);
    let mut ch = false;
    ui.label("Depth X:");
    ch |= unit_field(ui, egui::Id::unique("ex_x"), &mut e.depth.x, u, dpi, 64.0, true);
    ui.label("Y:");
    ch |= unit_field(ui, egui::Id::unique("ex_y"), &mut e.depth.y, u, dpi, 64.0, true);
    ui.label("Steps:");
    ch |= ui.add(egui::DragValue::new(&mut e.steps).range(1..=200)).changed();
    if ch {
        edit_sel(app, "Extrude", |o| o.effects.extrude = Some(e.clone()));
    }
    if ui.button("Clear Extrude").clicked() {
        edit_sel(app, "Clear Extrude", |o| o.effects.extrude = None);
    }
}

fn transparency_controls(app: &mut VectorApp, ui: &mut Ui) {
    use vector_core::style::MergeMode;
    use vector_core::Transparency;
    let o = sel_obj(app);
    let en = o.is_some();
    let cur = o.as_ref().map(|o| o.style.transparency.clone()).unwrap_or_default();
    let kind = match cur {
        Transparency::None => 0,
        Transparency::Uniform(_) => 1,
        Transparency::Gradient(_) => 2,
    };
    for (i, (icon, tip)) in [("no_color", "No Transparency"), ("transparency", "Uniform Transparency"), ("interactive_fill", "Fountain Transparency")].iter().enumerate() {
        if btn(app, ui, icon, tip, en, en && kind == i).clicked() {
            let t = match i {
                0 => Transparency::None,
                1 => Transparency::Uniform(50.0),
                _ => Transparency::Gradient(vector_core::style::Gradient::linear(vector_core::Color::WHITE, vector_core::Color::BLACK)),
            };
            edit_sel(app, "Transparency", move |o| o.style.transparency = t.clone());
        }
    }
    vsep(ui);
    let merge = o.as_ref().map(|o| o.style.merge).unwrap_or_default();
    ui.add_enabled_ui(en, |ui| {
        egui::ComboBox::from_id_salt("merge").selected_text(merge.name()).width(110.0).show_ui(ui, |ui| {
            for m in MergeMode::ALL {
                if ui.selectable_label(m == merge, m.name()).clicked() {
                    edit_sel(app, "Merge Mode", |o| o.style.merge = m);
                }
            }
        });
    });
    if let Transparency::Uniform(mut t) = cur {
        ui.label("Transparency:");
        if ui.add(egui::Slider::new(&mut t, 0.0..=100.0).suffix("")).changed() {
            edit_sel(app, "Transparency", |o| o.style.transparency = Transparency::Uniform(t));
        }
    }
}

fn fill_controls(app: &mut VectorApp, ui: &mut Ui) {
    use vector_core::Fill;
    let o = sel_obj(app);
    let en = o.is_some();
    let fill = o.as_ref().map(|o| o.style.fill.clone()).unwrap_or_default();
    let kind = match fill {
        Fill::None => 0,
        Fill::Solid(_) => 1,
        Fill::Gradient(_) => 2,
        Fill::Pattern(_) => 3,
        Fill::Mesh(_) => 4,
    };
    let base = fill.main_color().unwrap_or(app.default_fill);
    for (i, (icon, tip)) in [("no_color", "No Fill"), ("fill_icon", "Uniform Fill"), ("interactive_fill", "Fountain Fill"), ("grid", "Pattern Fill"), ("mesh_fill", "Mesh Fill")].iter().enumerate() {
        if btn(app, ui, icon, tip, en, en && kind == i).clicked() {
            let f = match i {
                0 => Fill::None,
                1 => Fill::Solid(base),
                2 => Fill::Gradient(vector_core::style::Gradient::linear(base, vector_core::Color::WHITE)),
                3 => Fill::Pattern(vector_core::style::PatternFill { kind: vector_core::style::PatternKind::ALL[0], fg: base, bg: vector_core::Color::WHITE, tile: 18.0, angle: 0.0, image: None, transform_with_object: true }),
                _ => Fill::Mesh(vector_core::style::MeshFill::new(app.settings.mesh_cols as usize + 1, app.settings.mesh_rows as usize + 1, base)),
            };
            edit_sel(app, "Fill", move |o| o.style.fill = f.clone());
        }
    }
    vsep(ui);
    if let Fill::Gradient(g) = &fill {
        use vector_core::style::GradientKind;
        for (tip, k) in [("Linear", GradientKind::Linear), ("Elliptical", GradientKind::Elliptical), ("Conical", GradientKind::Conical), ("Rectangular", GradientKind::Rectangular)] {
            if ui.selectable_label(g.kind == k, tip).clicked() {
                edit_sel(app, "Fountain Fill", |o| {
                    if let Fill::Gradient(g) = &mut o.style.fill {
                        g.kind = k;
                    }
                });
            }
        }
    }
    if let Fill::Pattern(p) = &fill {
        egui::ComboBox::from_id_salt("pattern_kind").selected_text(p.kind.name()).width(120.0).show_ui(ui, |ui| {
            for k in vector_core::style::PatternKind::ALL {
                if ui.selectable_label(k == p.kind, k.name()).clicked() {
                    edit_sel(app, "Pattern", |o| {
                        if let Fill::Pattern(p) = &mut o.style.fill {
                            p.kind = k;
                        }
                    });
                }
            }
        });
    }
    if app.tool == ToolId::MeshFill {
        ui.label("Columns:");
        ui.add(egui::DragValue::new(&mut app.settings.mesh_cols).range(1..=20));
        ui.label("Rows:");
        ui.add(egui::DragValue::new(&mut app.settings.mesh_rows).range(1..=20));
    }
}

// ----------------------------------------------------------------------------- text

/// Font list / size / style / alignment controls (property bar for text, and the Text toolbar).
pub fn text_controls(app: &mut VectorApp, ui: &mut Ui) {
    use vector_core::text::{CharFormat, TextAlign};
    let text_obj = selected_kind(app, |k| if let ObjKind::Text(t) = k { Some(t.clone()) } else { None });
    let range = app.doc().and_then(|d| d.text_edit.as_ref()).filter(|te| te.caret != te.anchor).map(|te| (te.caret.min(te.anchor), te.caret.max(te.anchor)));
    let fmt: CharFormat = match (&text_obj, range) {
        (Some(t), Some((s, _))) => t.format_at(s).clone(),
        (Some(t), None) => t.format.clone(),
        _ => app.ts.text_format.clone(),
    };
    let align = text_obj.as_ref().map(|t| t.align).unwrap_or(app.ts.text_align);
    let apply = |app: &mut VectorApp, label: &str, f: &dyn Fn(&mut CharFormat)| {
        if text_obj.is_some() {
            edit_sel(app, label, |o| {
                if let ObjKind::Text(t) = &mut o.kind {
                    match range {
                        Some((s, e)) => t.apply_format(s, e, f),
                        None => {
                            f(&mut t.format);
                            let n = t.char_count();
                            if !t.spans.is_empty() {
                                t.apply_format(0, n, f);
                            }
                        }
                    }
                }
            });
        } else {
            f(&mut app.ts.text_format);
        }
    };
    // font family
    let families: Vec<String> = vector_core::text::fonts().lock().families().to_vec();
    let mut chosen: Option<String> = None;
    egui::ComboBox::from_id_salt("font_family").selected_text(&fmt.font).width(170.0).height(420.0).show_ui(ui, |ui| {
        for f in &app.ts.recent_fonts {
            if ui.selectable_label(*f == fmt.font, egui::RichText::new(f).strong()).clicked() {
                chosen = Some(f.clone());
            }
        }
        if !app.ts.recent_fonts.is_empty() {
            ui.separator();
        }
        for f in &families {
            if ui.selectable_label(*f == fmt.font, f).clicked() {
                chosen = Some(f.clone());
            }
        }
    });
    if let Some(f) = chosen {
        app.ts.recent_fonts.retain(|x| *x != f);
        app.ts.recent_fonts.insert(0, f.clone());
        app.ts.recent_fonts.truncate(app.settings.recent_fonts.max(1));
        apply(app, "Font", &move |c: &mut CharFormat| c.font = f.clone());
    }
    // size
    let mut size = fmt.size;
    let r = egui::ComboBox::from_id_salt("font_size").selected_text(format!("{} pt", trim_num(size))).width(70.0).show_ui(ui, |ui| {
        for s in [6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 24.0, 36.0, 48.0, 72.0, 100.0, 150.0, 200.0] {
            if ui.selectable_label((size - s).abs() < 1e-6, format!("{} pt", trim_num(s))).clicked() {
                size = s;
            }
        }
    });
    let _ = r;
    if (size - fmt.size).abs() > 1e-9 {
        apply(app, "Font Size", &move |c: &mut CharFormat| c.size = size);
    }
    let mut s2 = fmt.size;
    if ui.add(egui::DragValue::new(&mut s2).range(1.0..=3000.0).speed(0.5).max_decimals(1)).changed() {
        apply(app, "Font Size", &move |c: &mut CharFormat| c.size = s2);
    }
    vsep(ui);
    if btn(app, ui, "bold", "Bold (Ctrl+B)", true, fmt.bold).clicked() {
        let v = !fmt.bold;
        apply(app, "Bold", &move |c: &mut CharFormat| c.bold = v);
    }
    if btn(app, ui, "italic", "Italic (Ctrl+I)", true, fmt.italic).clicked() {
        let v = !fmt.italic;
        apply(app, "Italic", &move |c: &mut CharFormat| c.italic = v);
    }
    if btn(app, ui, "underline", "Underline (Ctrl+U)", true, fmt.underline).clicked() {
        let v = !fmt.underline;
        apply(app, "Underline", &move |c: &mut CharFormat| c.underline = v);
    }
    vsep(ui);
    for (icon, tip, a) in [
        ("align_text_left", "Left", TextAlign::Left),
        ("align_text_center", "Center", TextAlign::Center),
        ("align_text_right", "Right", TextAlign::Right),
        ("align_text_justify", "Full justify", TextAlign::Justify),
    ] {
        if btn(app, ui, icon, tip, true, align == a).clicked() {
            if text_obj.is_some() {
                edit_sel(app, "Alignment", |o| {
                    if let ObjKind::Text(t) = &mut o.kind {
                        t.align = a;
                    }
                });
            } else {
                app.ts.text_align = a;
            }
        }
    }
    vsep(ui);
    if btn(app, ui, "text_docker", "Text Properties (Ctrl+T)", true, false).clicked() {
        app.run(Cmd::Docker(crate::dockers::DockerId::Text));
    }
    let vertical = text_obj.as_ref().map(|t| t.vertical).unwrap_or(false);
    if btn(app, ui, "text_horizontal", "Horizontal text", text_obj.is_some(), !vertical && text_obj.is_some()).clicked() {
        edit_sel(app, "Text Orientation", |o| {
            if let ObjKind::Text(t) = &mut o.kind {
                t.vertical = false;
            }
        });
    }
    if btn(app, ui, "text_vertical", "Vertical text", text_obj.is_some(), vertical).clicked() {
        edit_sel(app, "Text Orientation", |o| {
            if let ObjKind::Text(t) = &mut o.kind {
                t.vertical = true;
            }
        });
    }
}

fn trim_num(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
