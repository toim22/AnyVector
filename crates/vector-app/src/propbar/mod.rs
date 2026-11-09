//! Property bar: the reference property bar for the tool in use and the selection, drawn item by
//! item from the reference data (`crate::cmdbar::strip`). The fields Vector has are drawn here by
//! item id ([`control`]); the option buttons it has (corner type, ellipse or pie, …) report their
//! state and run through [`option_state`] and [`press`]; the other items keep the reference look,
//! disabled.

mod fields;
mod text;

pub use text::text_controls;

use crate::app::VectorApp;
use crate::commands::Cmd;
use crate::tools::ToolId;
use crate::ui::{self, Item};
use crate::widgets::icon_button_sized;
use egui::{Id, Ui};
use fields::{Fmt, Lead};
use kurbo::{Affine, Point, Rect, Vec2};
use vector_core::object::{ContourDir, CornerType, DistortKind, EllipseKind, PolygonKind};
use vector_core::style::{GradientKind, MergeMode};
use vector_core::text::TextAlign;
use vector_core::{Color, Fill, ObjKind, Object, Transparency, Unit};

fn key8(id: &str) -> String {
    id.get(..8).unwrap_or(id).to_ascii_lowercase()
}

pub(crate) fn btn(app: &mut VectorApp, ui: &mut Ui, icon: &str, tip: &str, enabled: bool, checked: bool) -> egui::Response {
    icon_button_sized(&mut app.icons, ui, icon, tip, enabled, checked, 30.0, 20.0)
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

pub fn transform_sel(app: &mut VectorApp, label: &str, m: Affine) {
    let Some(d) = app.doc_mut() else { return };
    let sel = d.selection.clone();
    d.modify(label, |doc, _| doc.transform_objects(&sel, m));
}

/// The one selected object's kind, mapped by `f`.
pub(crate) fn selected_kind<T>(app: &VectorApp, f: impl Fn(&ObjKind) -> Option<T>) -> Option<T> {
    let d = app.doc()?;
    if d.selection.len() != 1 {
        return None;
    }
    f(&d.doc.find(d.selection[0])?.kind)
}

/// The first selected object.
fn first<T>(app: &VectorApp, f: impl Fn(&Object) -> T) -> Option<T> {
    let d = app.doc()?;
    d.doc.find(*d.selection.first()?).map(f)
}

fn unit_dpi(app: &VectorApp) -> (Unit, f64) {
    app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0))
}

fn len(app: &VectorApp) -> Fmt {
    let (u, dpi) = unit_dpi(app);
    Fmt::Len(u, dpi)
}

fn pid(ui: &Ui, k: &str) -> Id {
    ui.make_persistent_id(("propbar", k))
}

fn lead(it: &Item) -> Lead {
    if it.icon.is_empty() {
        Lead::None
    } else {
        Lead::Icon(it.icon)
    }
}

fn tip(it: &Item) -> String {
    if it.tip.is_empty() {
        ui::plain(it.text)
    } else {
        it.tip.to_string()
    }
}

// ----------------------------------------------------------------------------- which bar

/// The bar with this id prefix.
fn bar_key(prefix: &str) -> Option<u16> {
    ui::bars().iter().position(|b| b.id.get(..8).is_some_and(|k| k.eq_ignore_ascii_case(prefix))).map(|i| i as u16)
}

/// The id of the reference property bar for the tool in use and the selection (each one checked
/// against the reference captures of every tool and selection).
fn bar_id(app: &VectorApp) -> &'static str {
    use ToolId::*;
    if !app.has_doc() {
        return "232a3ed2";
    }
    let table_cell = app.ts.table_cell.is_some() && selected_kind(app, |k| matches!(k, ObjKind::Table(_)).then_some(())).is_some();
    match app.tool {
        Pick | FreehandPick => selection_bar(app),
        FreeTransform => "e05e3796",
        Shape => "2acb551e",
        Smooth => "83b47e8a",
        Smear => "e11b3b11",
        Twirl => "bee33964",
        AttractRepel => "9d88aeb5",
        Smudge => "b3e80658",
        Roughen => "bf1d7dcd",
        Crop => "2b40f132",
        Knife => "c44787ac",
        VirtualDelete => "729d6113",
        Eraser => "dc647760",
        Zoom | Pan => "4f637ea3",
        Freehand | Bezier | BSpline => "d8e78151",
        TwoPointLine => "4d6bd188",
        Pen => "3eca4d09",
        Polyline => "450ce33d",
        ThreePointCurve => "26b3cd0f",
        PainterlyBrush => "f49ca8d5",
        ArtisticMedia => match app.ts.media_kind {
            3 => "104c5566",
            4 => "e934d52a",
            _ => "8fc0b9c2",
        },
        LiveSketch => "8200a31a",
        SmartDrawing => "5415c496",
        Rectangle => "b5286ff5",
        Rect3 => "2f924263",
        Ellipse => "1fa965b7",
        Ellipse3 => "1927dac7",
        Polygon => "61ccb32d",
        Star => "93e04431",
        Spiral => "ef9f9c87",
        CommonShapes => "bd10d1b3",
        GraphPaper => "7b31a57f",
        Text => "6118b138",
        Table if table_cell => "bde2df13",
        Table => "390243ba",
        ParallelDim | HvDim | AngularDim => "47015b40",
        SegmentDim => "b1ae77c4",
        Callout => "820aed33",
        StraightConnector | RightAngleConnector | RoundedConnector => "70618151",
        EditAnchor => "290f6196",
        DropShadow => "edcfb996",
        InnerShadow => "f9711864",
        Contour => "d2ef66d1",
        Blend => "d16cdcfb",
        Distort => match distort_kind(app) {
            DistortKind::PushPull => "9f3f2abf",
            DistortKind::Zipper => "d456c1b1",
            DistortKind::Twister => "d569238c",
        },
        Envelope => "b76f232e",
        Extrude => "b9c86a57",
        BlockShadow => "aa88b391",
        Transparency => match first(app, |o| o.style.transparency.clone()) {
            Some(vector_core::Transparency::Uniform(_)) => "5ad90a83",
            Some(vector_core::Transparency::Gradient(_)) => "c4ec19af",
            _ => "1b9d2d53",
        },
        ColorEyedropper => "7ba65308",
        AttributesEyedropper => "23f810c3",
        InteractiveFill => match first(app, |o| o.style.fill.clone()) {
            Some(Fill::Solid(_)) => "0eaa695a",
            Some(Fill::Gradient(_)) => "7f4de96f",
            Some(Fill::Pattern(_)) => "83ecaa5f",
            _ => "00b9d138",
        },
        SmartFill => "b07326e8",
        MeshFill => "211be451",
    }
}

/// The Pick tool's bar for the selection.
fn selection_bar(app: &VectorApp) -> &'static str {
    let Some(d) = app.doc() else { return "232a3ed2" };
    match d.selection.len() {
        0 => return "232a3ed2",
        1 => {}
        _ => return "4568875c",
    }
    d.doc.find(d.selection[0]).map(|o| object_bar(app, o)).unwrap_or("232a3ed2")
}

fn object_bar(app: &VectorApp, o: &Object) -> &'static str {
    match &o.kind {
        ObjKind::Rect(_) => "b5286ff5",
        ObjKind::Ellipse(_) => "1fa965b7",
        ObjKind::Polygon(p) if p.kind == PolygonKind::Polygon => "61ccb32d",
        ObjKind::Polygon(_) => "93e04431",
        ObjKind::Path(_) | ObjKind::Spiral(_) => "d8e78151",
        ObjKind::Text(t) if t.on_path.is_some() => "a2bbfc75",
        ObjKind::Text(_) => "6118b138",
        ObjKind::Image(_) => "188ce420",
        ObjKind::Group(_) | ObjKind::Blend(_) => "003886b3",
        // a PowerClip shows the bar of its frame
        ObjKind::PowerClip { frame, .. } => object_bar(app, frame),
        ObjKind::Table(_) if app.tool == ToolId::Table && app.ts.table_cell.is_some() => "bde2df13",
        ObjKind::Table(_) => "390243ba",
        ObjKind::Symbol(_) => "45ef38f6",
        ObjKind::Dimension(dd) if dd.kind == vector_core::object::DimensionKind::Callout => "820aed33",
        ObjKind::Dimension(_) => "47015b40",
        ObjKind::Connector(_) => "05586960",
    }
}

/// The reference property bar shown now.
pub fn bar_for(app: &VectorApp) -> Option<u16> {
    bar_key(bar_id(app))
}

pub fn property_bar(app: &mut VectorApp, ui: &mut Ui) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.add_space(4.0);
        if let Some(b) = bar_for(app) {
            crate::cmdbar::strip(app, ui, b);
        }
        crate::widgets::vsep(ui);
        let r = btn(app, ui, "plus", "Quick Customize", true, false);
        customize_popup(app, &r);
    });
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

// ----------------------------------------------------------------------------- fields

/// A field Vector does not have yet: empty and disabled, at the reference size.
pub fn placeholder(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    fields::empty(app, ui, it);
}

/// Draw the field `it` if Vector has it; false leaves it to the caller.
pub fn control(app: &mut VectorApp, ui: &mut Ui, it: &'static Item) -> bool {
    let k = key8(it.id);
    match k.as_str() {
        // the page (No Selection)
        "c2fa4323" => paper(app, ui, it),
        "f8b0a4f2" => page_size(app, ui, it),
        "ab655c9e" => units(app, ui, it),
        "a49dc96d" => nudge(app, ui, it),
        "fd116738" => duplicate_distance(app, ui, it),
        // transformations
        "3148e122" => {
            let on = app.has_selection();
            if let Some(o) = fields::anchors(ui, on.then_some(app.dockers.ui.t_anchor as u8), on, &tip(it)) {
                app.dockers.ui.t_anchor = o as usize;
            }
        }
        "a41164ff" => position(app, ui, it),
        "325d7c86" => size(app, ui, it),
        "42bef211" => scale(app, ui, it),
        "8723ad52" => angle(app, ui, it),
        "8137d409" | "03ba0025" => crop_marquee(app, ui, it, k == "8137d409"),
        // outline
        "97e5c8b9" | "8340f20d" => outline_width(app, ui, it),
        "6bd00383" => outline_style(app, ui, it),
        "76dd9e0c" | "40cc3adc" => arrowhead(app, ui, it, k == "76dd9e0c"),
        // Internet toolbar: Active Rollover State
        "bc13bc0e" => {
            let cur = crate::dialogs::object::active_state(app);
            let text = cur.map(|s| format!("[{}]", crate::dialogs::object::ROLLOVER_STATES[s as usize % 3].to_uppercase())).unwrap_or_else(|| "[NORMAL]".into());
            let mut pick = None;
            fields::combo(app, ui, Lead::None, &text, 82.0, cur.is_some(), &tip(it), |_, ui| {
                for (i, n) in crate::dialogs::object::ROLLOVER_STATES.iter().enumerate() {
                    if ui.selectable_label(cur == Some(i as u8), *n).clicked() {
                        pick = Some(i as u8);
                    }
                }
            });
            if let Some(st) = pick {
                crate::dialogs::object::show_state(app, st);
            }
        }
        // Smart Drawing
        "0f15674a" | "20e29071" => {
            let recognition = k == "0f15674a";
            let names = ["None", "Lowest", "Low", "Medium", "High", "Highest"];
            fields::caption(ui, if recognition { "Shape Recognition Level:" } else { "Smart Smoothing Level:" }, true);
            let cur = if recognition { app.settings.smart_recognition } else { app.settings.smart_smoothing }.min(5) as usize;
            let mut pick = None;
            fields::combo(app, ui, Lead::None, names[cur], 76.0, true, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i as u8);
                    }
                }
            });
            if let Some(i) = pick {
                if recognition {
                    app.settings.smart_recognition = i;
                } else {
                    app.settings.smart_smoothing = i;
                }
            }
        }
        // shapes
        "f7ba8f56" => {
            fields::combo(app, ui, Lead::None, "Rectangular", 96.0, false, &tip(it), |_, _| {});
        }
        "11a5ff09" | "4c1b898d" => corner_radius(app, ui, it, k == "11a5ff09"),
        "ef600b16" => ellipse_angles(app, ui, it),
        "abbe0d50" | "22d985d0" => polygon_value(app, ui, it, k == "abbe0d50"),
        "a05699ce" => {
            let mut v = app.settings.spiral_revs as f64;
            if let Some(n) = fields::spin(app, ui, pid(ui, &k), lead(it), Some(v), Fmt::Num(0, ""), 44.0, "", true, &tip(it)) {
                v = n.clamp(1.0, 100.0);
                app.settings.spiral_revs = v as u32;
            }
        }
        "e37e909f" => {
            let on = app.settings.spiral_log;
            if let Some(v) = fields::slider(app, ui, pid(ui, &k), lead(it), Some(app.settings.spiral_expansion), 1.0..=100.0, Fmt::Num(0, ""), 36.0, on, &tip(it)) {
                app.settings.spiral_expansion = v.clamp(1.0, 100.0);
            }
        }
        "5264e150" => {
            let r = fields::stacked(app, ui, pid(ui, &k), [lead(it), lead(it)], [Some(app.settings.graph_cols as f64), Some(app.settings.graph_rows as f64)], Fmt::Num(0, ""), 48.0, "", false, true, true, &tip(it));
            if let Some(v) = r[0] {
                app.settings.graph_cols = v.clamp(1.0, 99.0) as u32;
            }
            if let Some(v) = r[1] {
                app.settings.graph_rows = v.clamp(1.0, 99.0) as u32;
            }
        }
        "baecf0e4" => table_size(app, ui, it),
        "18db85a5" => {
            let names = crate::tools::COMMON_SHAPES;
            let cur = app.ts.common_shape.min(names.len() - 1);
            let mut pick = None;
            fields::combo(app, ui, Lead::None, names[cur], 120.0, true, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i);
                    }
                }
            });
            if let Some(i) = pick {
                app.ts.common_shape = i;
            }
        }
        // drawing
        "4c28b6fb" | "81f3fc56" | "99c95fa0" => {
            if let Some(v) = fields::slider(app, ui, pid(ui, &k), lead(it), Some(app.settings.freehand_smoothing), 0.0..=100.0, Fmt::Num(0, ""), 40.0, true, &tip(it)) {
                app.settings.freehand_smoothing = v.clamp(0.0, 100.0);
            }
        }
        "b90b5211" | "dd1e9a96" => {
            let f = len(app);
            if let Some(v) = fields::spin(app, ui, pid(ui, &k), lead(it), Some(app.ts.media_width), f, 64.0, "", true, &tip(it)) {
                app.ts.media_width = v.max(0.1);
            }
        }
        "1cb31cca" => {
            let names = ["Taper", "Taper end", "Taper start"];
            let cur = app.ts.media_kind.min(2);
            let mut pick = None;
            fields::combo(app, ui, Lead::None, names[cur], 110.0, true, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i);
                    }
                }
            });
            if let Some(i) = pick {
                app.ts.media_kind = i;
            }
        }
        // brushes and the eraser
        "1cedaa4d" | "45f60795" | "5dbd9843" => {
            let f = len(app);
            if let Some(v) = fields::spin(app, ui, pid(ui, &k), lead(it), Some(app.ts.brush_size), f, 64.0, "", true, &tip(it)) {
                app.ts.brush_size = v.max(0.5);
            }
        }
        "9fbdb66b" | "ac94dcfd" | "0d122216" => {
            if let Some(v) = fields::slider(app, ui, pid(ui, &k), lead(it), Some(app.ts.brush_strength), 1.0..=100.0, Fmt::Num(0, ""), 40.0, true, &tip(it)) {
                app.ts.brush_strength = v.clamp(1.0, 100.0);
            }
        }
        "9b07b079" => {
            let f = len(app);
            if let Some(v) = fields::spin(app, ui, pid(ui, &k), lead(it), Some(app.settings.eraser_thickness), f, 64.0, "", true, &tip(it)) {
                app.settings.eraser_thickness = v.max(0.1);
            }
        }
        // text
        "586a5b77" => text::font_list(app, ui, Lead::None, 170.0, &tip(it)),
        "4ecf9fb2" => text::font_size(app, ui, Lead::None, &tip(it)),
        // dimensions
        "f7a6809b" | "6d34d885" | "d131a8b8" => dimension_list(app, ui, it, &k),
        "7385bb6e" | "ce2dd235" => dimension_text(app, ui, it, k == "7385bb6e"),
        // connectors
        "25f235bf" => {
            let r = selected_kind(app, |k| if let ObjKind::Connector(c) = k { Some(c.corner_radius) } else { None });
            let f = len(app);
            if let Some(v) = fields::spin(app, ui, pid(ui, &k), lead(it), Some(r.unwrap_or(0.0)), f, 64.0, "", r.is_some(), &tip(it)) {
                edit_sel(app, "Connector", |o| {
                    if let ObjKind::Connector(c) = &mut o.kind {
                        c.corner_radius = v.max(0.0);
                    }
                });
            }
        }
        // effects
        "b9180b11" | "50947468" | "c6e4e963" | "bf98d85a" => shadow(app, ui, it, &k),
        "fbe66f8d" | "d7f2a0b0" | "df167d6c" | "8ea8f115" => contour(app, ui, it, &k),
        "a869281c" | "f46d4cfe" => blend(app, ui, it, &k),
        "03c0c841" | "28f1dce2" | "fca9d3fd" | "d09509c4" | "474fcebf" => distort(app, ui, it, &k),
        "bb7f7a77" => {
            let e = first(app, |o| o.effects.extrude.clone()).flatten();
            if let Some(v) = fields::spin(app, ui, pid(ui, &k), lead(it), e.as_ref().map(|e| e.steps as f64), Fmt::Num(0, ""), 44.0, "", e.is_some(), &tip(it)) {
                edit_sel(app, "Extrude", |o| {
                    if let Some(e) = &mut o.effects.extrude {
                        e.steps = v.clamp(1.0, 200.0) as u32;
                    }
                });
            }
        }
        "8fb33834" | "4239c3f3" | "e3e40f28" => block_shadow(app, ui, it, &k),
        "2fbb3a5c" => merge_mode(app, ui, it),
        "f8e3d90b" => {
            let t = first(app, |o| if let Transparency::Uniform(t) = o.style.transparency { Some(t) } else { None }).flatten();
            if let Some(v) = fields::slider(app, ui, pid(ui, &k), lead(it), t, 0.0..=100.0, Fmt::Num(0, ""), 40.0, t.is_some(), &tip(it)) {
                let v = v.clamp(0.0, 100.0);
                edit_sel(app, "Transparency", |o| o.style.transparency = Transparency::Uniform(v));
            }
        }
        "815f5d95" | "73149189" | "b54b6289" => fill_color(app, ui, it, &k),
        "5627125d" => {
            let r = fields::stacked(app, ui, pid(ui, &k), [lead(it), lead(it)], [Some(app.settings.mesh_cols as f64), Some(app.settings.mesh_rows as f64)], Fmt::Num(0, ""), 44.0, "", false, true, true, &tip(it));
            if let Some(v) = r[0] {
                app.settings.mesh_cols = v.clamp(1.0, 20.0) as u32;
            }
            if let Some(v) = r[1] {
                app.settings.mesh_rows = v.clamp(1.0, 20.0) as u32;
            }
        }
        _ => return false,
    }
    true
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

fn page_wh(app: &VectorApp) -> Option<(f64, f64)> {
    app.doc().map(|d| (d.page_ref().width, d.page_ref().height))
}

fn paper(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let presets = vector_core::units::page_presets();
    let wh = page_wh(app);
    let current = wh
        .map(|(pw, ph)| presets.iter().find(|(_, w, h)| ((w - pw).abs() < 0.5 && (h - ph).abs() < 0.5) || ((w - ph).abs() < 0.5 && (h - pw).abs() < 0.5)).map(|p| p.0).unwrap_or("Custom"))
        .unwrap_or("");
    let mut pick = None;
    fields::combo(app, ui, Lead::None, current, 150.0, wh.is_some(), &tip(it), |_, ui| {
        for (name, w, h) in &presets {
            if ui.selectable_label(*name == current, *name).clicked() {
                pick = Some((*w, *h));
            }
        }
    });
    if let (Some((w, h)), Some((pw, ph))) = (pick, wh) {
        let (w, h) = if pw > ph { (h, w) } else { (w, h) };
        set_page_size(app, w, h);
    }
}

fn page_size(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let wh = page_wh(app);
    let f = len(app);
    let r = fields::stacked(app, ui, pid(ui, "page_size"), [Lead::Icon("page_width"), Lead::Icon("page_height")], [wh.map(|w| w.0), wh.map(|w| w.1)], f, 84.0, "", false, true, wh.is_some(), &tip(it));
    if let Some((pw, ph)) = wh {
        if let Some(w) = r[0] {
            set_page_size(app, w.max(1.0), ph);
        }
        if let Some(h) = r[1] {
            set_page_size(app, pw, h.max(1.0));
        }
    }
}

fn units(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let doc = app.has_doc();
    let unit = unit_dpi(app).0;
    fields::caption(ui, "Units:", doc);
    let mut pick = None;
    fields::combo(app, ui, Lead::None, if doc { unit.name() } else { "" }, 100.0, doc, &tip(it), |_, ui| {
        for u in Unit::ALL {
            if ui.selectable_label(u == unit, u.name()).clicked() {
                pick = Some(u);
            }
        }
    });
    if let Some(u) = pick {
        if let Some(d) = app.doc_mut() {
            d.doc.settings.unit = u;
        }
        app.settings.default_unit = u;
    }
}

fn nudge(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let v = app.doc().map(|d| d.doc.settings.nudge);
    let f = len(app);
    let lead = if it.icon.is_empty() { Lead::Icon("nudge") } else { lead(it) };
    if let Some(n) = fields::spin(app, ui, pid(ui, "nudge"), lead, v, f, 70.0, "", v.is_some(), &tip(it)) {
        if let Some(d) = app.doc_mut() {
            d.doc.settings.nudge = n.max(0.0);
        }
    }
}

fn duplicate_distance(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let v = app.doc().map(|d| d.doc.settings.duplicate_offset);
    let f = len(app);
    let r = fields::stacked(app, ui, pid(ui, "dup"), [Lead::Icon("dup_x"), Lead::Icon("dup_y")], [v.map(|v| v.x), v.map(|v| v.y)], f, 70.0, "", false, true, v.is_some(), &tip(it));
    if let Some(d) = app.doc_mut() {
        if let Some(x) = r[0] {
            d.doc.settings.duplicate_offset.x = x;
        }
        if let Some(y) = r[1] {
            d.doc.settings.duplicate_offset.y = y;
        }
    }
}

/// The selection's bounds and its reference point (the object origin).
fn origin(app: &VectorApp) -> Option<(Rect, Point)> {
    let bb = app.doc()?.selection_bounds()?;
    let a = app.dockers.ui.t_anchor;
    let (fx, fy) = ((a % 3) as f64 / 2.0, (a / 3) as f64 / 2.0);
    Some((bb, Point::new(bb.x0 + bb.width() * fx, bb.y0 + bb.height() * fy)))
}

fn position(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let o = origin(app);
    let ro = app.doc().map(|d| d.doc.settings.ruler_origin).unwrap_or_default();
    let f = len(app);
    let r = fields::stacked(app, ui, pid(ui, "pos"), [Lead::Text("X:"), Lead::Text("Y:")], [o.map(|o| o.1.x - ro.x), o.map(|o| o.1.y - ro.y)], f, 86.0, "", true, false, o.is_some(), &tip(it));
    if let Some((_, a)) = o {
        if let Some(x) = r[0] {
            transform_sel(app, "Move", Affine::translate((x + ro.x - a.x, 0.0)));
        }
        if let Some(y) = r[1] {
            transform_sel(app, "Move", Affine::translate((0.0, y + ro.y - a.y)));
        }
    }
}

fn scale_about(app: &mut VectorApp, a: Point, sx: f64, sy: f64) {
    let m = Affine::translate(a.to_vec2()) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-a.to_vec2());
    transform_sel(app, "Scale", m);
}

fn size(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let o = origin(app);
    let f = len(app);
    let r = fields::stacked(app, ui, pid(ui, "size"), [Lead::Icon("size_w"), Lead::Icon("size_h")], [o.map(|o| o.0.width()), o.map(|o| o.0.height())], f, 84.0, "", true, false, o.is_some(), &tip(it));
    let Some((bb, a)) = o else { return };
    let lock = app.ts.lock_ratio;
    if let Some(w) = r[0].filter(|_| bb.width() > 0.0) {
        let sx = w.max(0.001) / bb.width();
        scale_about(app, a, sx, if lock { sx } else { 1.0 });
    }
    if let Some(h) = r[1].filter(|_| bb.height() > 0.0) {
        let sy = h.max(0.001) / bb.height();
        scale_about(app, a, if lock { sy } else { 1.0 }, sy);
    }
}

fn scale(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let o = origin(app);
    let v = o.map(|_| 100.0);
    let r = fields::stacked(app, ui, pid(ui, "scale"), [Lead::None, Lead::None], [v, v], Fmt::Num(1, ""), 50.0, "%", true, false, o.is_some(), &tip(it));
    let Some((_, a)) = o else { return };
    let lock = app.ts.lock_ratio;
    if let Some(s) = r[0] {
        let s = s.max(0.1) / 100.0;
        scale_about(app, a, s, if lock { s } else { 1.0 });
    }
    if let Some(s) = r[1] {
        let s = s.max(0.1) / 100.0;
        scale_about(app, a, if lock { s } else { 1.0 }, s);
    }
}

fn angle(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let bb = app.doc().and_then(|d| d.selection_bounds());
    let cur = app.doc().filter(|d| d.selection.len() == 1).and_then(|d| d.doc.find(d.selection[0])).map(|o| vector_core::geom::rotation_deg(o.transform)).unwrap_or(0.0);
    if let Some(a) = fields::spin(app, ui, pid(ui, "angle"), Lead::Icon("rotate"), bb.map(|_| cur), Fmt::Num(1, ""), 52.0, "°", bb.is_some(), &tip(it)) {
        let c = bb.unwrap().center();
        let m = Affine::translate(c.to_vec2()) * Affine::rotate(-(a - cur).to_radians()) * Affine::translate(-c.to_vec2());
        transform_sel(app, "Rotate", m);
    }
}

fn crop_marquee(app: &mut VectorApp, ui: &mut Ui, it: &Item, pos: bool) {
    let r = app.ts.crop_rect;
    let f = len(app);
    let (leads, vals) = if pos { ([Lead::Text("X:"), Lead::Text("Y:")], [r.map(|r| r.center().x), r.map(|r| r.center().y)]) } else { ([Lead::Icon("size_w"), Lead::Icon("size_h")], [r.map(|r| r.width()), r.map(|r| r.height())]) };
    let out = fields::stacked(app, ui, pid(ui, if pos { "crop_pos" } else { "crop_size" }), leads, vals, f, 84.0, "", true, false, r.is_some(), &tip(it));
    if let Some(mut c) = r {
        if pos {
            let d = Vec2::new(out[0].map(|x| x - c.center().x).unwrap_or(0.0), out[1].map(|y| y - c.center().y).unwrap_or(0.0));
            c = c + d;
        } else {
            let center = c.center();
            let w = out[0].unwrap_or(c.width()).max(1.0);
            let h = out[1].unwrap_or(c.height()).max(1.0);
            c = Rect::from_center_size(center, (w, h));
        }
        app.ts.crop_rect = Some(c);
    }
}

// ----------------------------------------------------------------------------- outline

/// The outline width shown: `None` for no outline, 0 for hairline.
fn outline_now(app: &VectorApp) -> Option<f64> {
    if app.has_selection() {
        first(app, |o| o.style.outline.color.map(|_| o.style.outline.width)).flatten()
    } else {
        Some(app.ts.default_outline_width)
    }
}

fn outline_width(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let w = outline_now(app);
    let dpi = unit_dpi(app).1;
    let text = match w {
        None => "None".to_string(),
        Some(w) if w <= 0.0 => "Hairline".to_string(),
        Some(w) => Unit::Points.format(w, dpi),
    };
    let doc = app.has_doc();
    let lead = if it.icon.is_empty() { Lead::Icon("outline_icon") } else { lead(it) };
    let mut pick: Option<Option<f64>> = None;
    fields::combo(app, ui, lead, &text, 78.0, doc, &tip(it), |_, ui| {
        if ui.selectable_label(w.is_none(), "None").clicked() {
            pick = Some(None);
        }
        for (name, pw) in vector_core::style::outline_width_presets() {
            if ui.selectable_label(w.is_some_and(|w| (w - pw).abs() < 1e-6), name).clicked() {
                pick = Some(Some(pw));
            }
        }
    });
    match pick {
        Some(None) if app.has_selection() => edit_sel(app, "Outline", |o| o.style.outline.color = None),
        Some(Some(w)) if app.has_selection() => edit_sel(app, "Outline Width", |o| {
            o.style.outline.width = w;
            if o.style.outline.color.is_none() {
                o.style.outline.color = Some(Color::BLACK);
            }
        }),
        Some(Some(w)) => app.ts.default_outline_width = w,
        _ => {}
    }
}

fn outline_style(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let dash = first(app, |o| o.style.outline.dash.clone()).unwrap_or_default();
    let on = app.has_selection();
    let presets = vector_core::style::dash_presets();
    let mut pick = None;
    let r = fields::combo(app, ui, Lead::None, "", 96.0, app.has_doc(), &tip(it), |_, ui| {
        for (name, d) in &presets {
            if ui.selectable_label(*d == dash, *name).clicked() {
                pick = Some(d.clone());
            }
        }
    });
    paint_dash(ui, r.rect, &dash, app.has_doc());
    if let Some(d) = pick.filter(|_| on) {
        edit_sel(app, "Outline Style", |o| o.style.outline.dash = d.clone());
    }
}

/// The line of a dash pattern across a list field.
fn paint_dash(ui: &Ui, rect: egui::Rect, dash: &[f64], enabled: bool) {
    let y = rect.center().y;
    let (x0, x1) = (rect.left() + 6.0, rect.right() - 20.0);
    let c = if enabled { crate::theme::TEXT } else { crate::theme::DISABLED };
    if dash.is_empty() {
        ui.painter().line_segment([egui::pos2(x0, y), egui::pos2(x1, y)], egui::Stroke::new(1.0, c));
        return;
    }
    let scale = 2.0;
    let mut x = x0;
    let mut i = 0;
    while x < x1 {
        let l = (dash[i % dash.len()] as f32 * scale).max(1.0);
        if i % 2 == 0 {
            ui.painter().line_segment([egui::pos2(x, y), egui::pos2((x + l).min(x1), y)], egui::Stroke::new(1.0, c));
        }
        x += l;
        i += 1;
    }
}

fn arrowhead(app: &mut VectorApp, ui: &mut Ui, it: &Item, start: bool) {
    use vector_core::style::ArrowHead;
    let cur = first(app, |o| if start { o.style.outline.start_arrow } else { o.style.outline.end_arrow }).flatten();
    let on = app.has_selection();
    let text = match cur {
        None => "—".to_string(),
        Some(a) => crate::dialogs::object::arrow_name(app, a),
    };
    let mut pick: Option<Option<ArrowHead>> = None;
    let choices = crate::dialogs::object::arrow_choices(app);
    fields::combo(app, ui, Lead::None, &text, 48.0, on, &tip(it), |_, ui| {
        if ui.selectable_label(cur.is_none(), "None").clicked() {
            pick = Some(None);
        }
        for (a, name) in &choices {
            if ui.selectable_label(cur == Some(*a), name).clicked() {
                pick = Some(Some(*a));
            }
        }
    });
    if let Some(a) = pick {
        crate::dialogs::object::ensure_arrow(app, a);
        edit_sel(app, "Arrowhead", |o| {
            if start {
                o.style.outline.start_arrow = a;
            } else {
                o.style.outline.end_arrow = a;
            }
        });
    }
}

// ----------------------------------------------------------------------------- shapes

/// The rectangle's corners: the selected rectangle's, else those for new rectangles.
fn corners(app: &VectorApp) -> (CornerType, [f64; 4], bool) {
    match selected_kind(app, |k| if let ObjKind::Rect(r) = k { Some((r.corner, r.radii)) } else { None }) {
        Some((c, r)) => (c, r, true),
        None => (
            match app.settings.rect_corner {
                1 => CornerType::Scallop,
                2 => CornerType::Chamfer,
                _ => CornerType::Round,
            },
            app.settings.rect_radii,
            false,
        ),
    }
}

fn set_corners(app: &mut VectorApp, corner: CornerType, radii: [f64; 4]) {
    let (_, _, sel) = corners(app);
    app.settings.rect_radii = radii;
    app.settings.rect_corner = match corner {
        CornerType::Round => 0,
        CornerType::Scallop => 1,
        CornerType::Chamfer => 2,
    };
    if sel {
        edit_sel(app, "Corners", |o| {
            if let ObjKind::Rect(r) = &mut o.kind {
                r.corner = corner;
                r.radii = radii;
            }
        });
    }
}

/// The left column of corner radii (top left over bottom left) or the right one (top right over
/// bottom right).
fn corner_radius(app: &mut VectorApp, ui: &mut Ui, it: &Item, left: bool) {
    let (corner, mut radii, _) = corners(app);
    let idx = if left { [0, 3] } else { [1, 2] };
    let f = len(app);
    let on = app.has_doc();
    let marks = if left { [Lead::Icon("corner_tl"), Lead::Icon("corner_bl")] } else { [Lead::Icon("corner_tr"), Lead::Icon("corner_br")] };
    if !left {
        fields::lead_column(app, ui, marks, on);
    }
    let r = fields::stacked(app, ui, pid(ui, if left { "radius_l" } else { "radius_r" }), [Lead::None, Lead::None], [Some(radii[idx[0]]), Some(radii[idx[1]])], f, 76.0, "", true, true, on, &tip(it));
    if left {
        fields::lead_column(app, ui, marks, on);
    }
    let mut changed = false;
    for i in 0..2 {
        if let Some(v) = r[i] {
            let v = v.max(0.0);
            if app.ts.lock_corners {
                radii = [v; 4];
            } else {
                radii[idx[i]] = v;
            }
            changed = true;
        }
    }
    if changed {
        set_corners(app, corner, radii);
    }
}

fn ellipse_now(app: &VectorApp) -> (EllipseKind, f64, f64, bool) {
    match selected_kind(app, |k| if let ObjKind::Ellipse(e) = k { Some((e.kind, e.start, e.end)) } else { None }) {
        Some((k, s, e)) => (k, s, e, true),
        None => (
            match app.settings.ellipse_kind {
                1 => EllipseKind::Pie,
                2 => EllipseKind::Arc,
                _ => EllipseKind::Ellipse,
            },
            app.settings.ellipse_start,
            app.settings.ellipse_end,
            false,
        ),
    }
}

fn set_ellipse(app: &mut VectorApp, kind: EllipseKind, start: f64, end: f64) {
    let sel = ellipse_now(app).3;
    app.settings.ellipse_kind = kind as u8;
    app.settings.ellipse_start = start;
    app.settings.ellipse_end = end;
    if sel {
        edit_sel(app, "Ellipse", |o| {
            if let ObjKind::Ellipse(e) = &mut o.kind {
                e.kind = kind;
                e.start = start;
                e.end = end;
            }
        });
    }
}

fn ellipse_angles(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let (kind, s, e, _) = ellipse_now(app);
    let r = fields::stacked(app, ui, pid(ui, "ellipse_angles"), [Lead::Icon("ellipse3"), Lead::Icon("ellipse3")], [Some(s), Some(e)], Fmt::Num(1, " °"), 66.0, "", false, true, app.has_doc(), &tip(it));
    if r[0].is_some() || r[1].is_some() {
        set_ellipse(app, kind, r[0].unwrap_or(s).clamp(-360.0, 360.0), r[1].unwrap_or(e).clamp(-360.0, 360.0));
    }
}

/// The selected polygon or star, else the values for new ones.
fn polygon_now(app: &VectorApp) -> (PolygonKind, u32, f64, bool) {
    let star_tool = app.tool == ToolId::Star;
    match selected_kind(app, |k| if let ObjKind::Polygon(p) = k { Some((p.kind, p.sides, p.sharpness)) } else { None }) {
        Some((k, n, s)) => (k, n, s, true),
        None if star_tool => (PolygonKind::Star, app.settings.star_points, app.settings.star_sharpness, false),
        None => (PolygonKind::Polygon, app.settings.polygon_sides, app.settings.star_sharpness, false),
    }
}

fn set_polygon(app: &mut VectorApp, kind: PolygonKind, sides: u32, sharp: f64) {
    let sel = polygon_now(app).3;
    if kind == PolygonKind::Polygon {
        app.settings.polygon_sides = sides;
    } else {
        app.settings.star_points = sides;
        app.settings.star_sharpness = sharp;
    }
    if sel {
        edit_sel(app, "Polygon", |o| {
            if let ObjKind::Polygon(p) = &mut o.kind {
                p.kind = kind;
                p.sides = sides;
                p.sharpness = sharp;
            }
        });
    }
}

fn polygon_value(app: &mut VectorApp, ui: &mut Ui, it: &Item, sides: bool) {
    let (kind, n, sharp, _) = polygon_now(app);
    let doc = app.has_doc();
    if sides {
        let lead = if it.icon.is_empty() { Lead::Icon("polygon") } else { lead(it) };
        if let Some(v) = fields::spin(app, ui, pid(ui, "sides"), lead, Some(n as f64), Fmt::Num(0, ""), 44.0, "", doc, &tip(it)) {
            set_polygon(app, kind, v.clamp(3.0, 500.0) as u32, sharp);
        }
    } else {
        let on = doc && kind != PolygonKind::Polygon;
        if let Some(v) = fields::spin(app, ui, pid(ui, "sharpness"), lead(it), Some(sharp), Fmt::Num(0, ""), 44.0, "", on, &tip(it)) {
            set_polygon(app, kind, n, v.clamp(1.0, 99.0));
        }
    }
}

fn table_size(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let sel = selected_kind(app, |k| if let ObjKind::Table(t) = k { Some((t.rows, t.cols)) } else { None });
    let (rows, cols) = sel.map(|(r, c)| (r as f64, c as f64)).unwrap_or((app.settings.table_rows as f64, app.settings.table_cols as f64));
    let r = fields::stacked(app, ui, pid(ui, "table_size"), [lead(it), lead(it)], [Some(rows), Some(cols)], Fmt::Num(0, ""), 44.0, "", false, true, app.has_doc(), &tip(it));
    let rows = r[0].map(|v| v.clamp(1.0, 99.0) as usize);
    let cols = r[1].map(|v| v.clamp(1.0, 99.0) as usize);
    if rows.is_none() && cols.is_none() {
        return;
    }
    if sel.is_some() {
        edit_sel(app, "Table", |o| {
            if let ObjKind::Table(t) = &mut o.kind {
                if let Some(n) = rows {
                    while t.rows < n {
                        t.insert_row(t.rows);
                    }
                    while t.rows > n && t.rows > 1 {
                        t.delete_row(t.rows - 1);
                    }
                }
                if let Some(n) = cols {
                    while t.cols < n {
                        t.insert_col(t.cols);
                    }
                    while t.cols > n && t.cols > 1 {
                        t.delete_col(t.cols - 1);
                    }
                }
            }
        });
    } else {
        if let Some(n) = rows {
            app.settings.table_rows = n as u32;
        }
        if let Some(n) = cols {
            app.settings.table_cols = n as u32;
        }
    }
}

// ----------------------------------------------------------------------------- dimensions

fn dimension_now(app: &VectorApp) -> Option<vector_core::object::DimensionObj> {
    selected_kind(app, |k| if let ObjKind::Dimension(d) = k { Some(d.clone()) } else { None })
}

fn dimension_list(app: &mut VectorApp, ui: &mut Ui, it: &Item, k: &str) {
    let dim = dimension_now(app);
    let doc = app.has_doc();
    match k {
        // style
        "f7a6809b" => {
            let names = ["Decimal", "Fractional", "US Engineering", "US Architectural"];
            let cur = (app.settings.dim_style as usize).min(names.len() - 1);
            let mut pick = None;
            fields::combo(app, ui, Lead::None, names[cur], 86.0, doc, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i);
                    }
                }
            });
            if let Some(i) = pick {
                app.settings.dim_style = i as u8;
            }
        }
        // precision
        "6d34d885" => {
            let cur = dim.as_ref().map(|d| d.precision).unwrap_or(app.settings.dim_precision as usize).min(6);
            let show = |p: usize| if p == 0 { "0".to_string() } else { format!("0.{}", "0".repeat(p)) };
            let mut pick = None;
            fields::combo(app, ui, Lead::None, &show(cur), 80.0, doc, &tip(it), |_, ui| {
                for p in 0..=6 {
                    if ui.selectable_label(p == cur, show(p)).clicked() {
                        pick = Some(p);
                    }
                }
            });
            if let Some(p) = pick {
                app.settings.dim_precision = p as u32;
                edit_sel(app, "Dimension", |o| {
                    if let ObjKind::Dimension(d) = &mut o.kind {
                        d.precision = p;
                    }
                });
            }
        }
        // units
        _ => {
            let cur = dim.as_ref().map(|d| d.unit).unwrap_or(unit_dpi(app).0);
            let mut pick = None;
            fields::combo(app, ui, Lead::None, cur.suffix(), 56.0, doc, &tip(it), |_, ui| {
                for u in Unit::ALL {
                    if ui.selectable_label(u == cur, u.suffix()).on_hover_text(u.name()).clicked() {
                        pick = Some(u);
                    }
                }
            });
            if let Some(u) = pick {
                edit_sel(app, "Dimension", |o| {
                    if let ObjKind::Dimension(d) = &mut o.kind {
                        d.unit = u;
                    }
                });
            }
        }
    }
}

fn dimension_text(app: &mut VectorApp, ui: &mut Ui, it: &Item, prefix: bool) {
    let dim = dimension_now(app);
    let cur = match (&dim, prefix) {
        (Some(d), true) => d.prefix.clone(),
        (Some(d), false) => d.suffix.clone(),
        (None, true) => app.settings.dim_prefix.clone(),
        (None, false) => app.settings.dim_suffix.clone(),
    };
    let cap = if prefix { "Prefix:" } else { "Suffix:" };
    if let Some(t) = fields::text(ui, pid(ui, cap), cap, &cur, 40.0, app.has_doc()) {
        let _ = it;
        if prefix {
            app.settings.dim_prefix = t.clone();
        } else {
            app.settings.dim_suffix = t.clone();
        }
        if dim.is_some() {
            edit_sel(app, "Dimension", |o| {
                if let ObjKind::Dimension(d) = &mut o.kind {
                    if prefix {
                        d.prefix = t.clone();
                    } else {
                        d.suffix = t.clone();
                    }
                }
            });
        }
    }
}

// ----------------------------------------------------------------------------- effects

fn shadow(app: &mut VectorApp, ui: &mut Ui, it: &Item, k: &str) {
    let inner = app.tool == ToolId::InnerShadow;
    let cur = first(app, |o| if inner { o.effects.inner_shadow.clone() } else { o.effects.drop_shadow.clone() }).flatten();
    let on = cur.is_some();
    let mut s = cur.clone().unwrap_or_else(|| crate::tools::default_shadow(inner));
    let mut ch = false;
    let id = pid(ui, k);
    match k {
        "b9180b11" => {
            if let Some(c) = fields::color_combo(ui, id, Some(s.color), 52.0, on, &tip(it)) {
                s.color = c.unwrap_or(Color::BLACK);
                ch = true;
            }
        }
        "50947468" | "c6e4e963" => {
            let opacity = k == "50947468";
            let v = if opacity { s.opacity } else { s.feather };
            if let Some(v) = fields::slider(app, ui, id, lead(it), cur.as_ref().map(|_| v), 0.0..=100.0, Fmt::Num(0, ""), 36.0, on, &tip(it)) {
                if opacity {
                    s.opacity = v.clamp(0.0, 100.0);
                } else {
                    s.feather = v.clamp(0.0, 100.0);
                }
                ch = true;
            }
        }
        _ => {
            let f = len(app);
            let r = fields::stacked(app, ui, id, [Lead::None, Lead::None], [cur.as_ref().map(|s| s.offset.x), cur.as_ref().map(|s| s.offset.y)], f, 66.0, "", false, true, on, &tip(it));
            if let Some(x) = r[0] {
                s.offset.x = x;
                ch = true;
            }
            if let Some(y) = r[1] {
                s.offset.y = y;
                ch = true;
            }
        }
    }
    if ch && on {
        edit_sel(app, "Shadow", |o| {
            if inner {
                o.effects.inner_shadow = Some(s.clone());
            } else {
                o.effects.drop_shadow = Some(s.clone());
            }
        });
    }
}

fn contour_now(app: &VectorApp) -> Option<vector_core::object::ContourEffect> {
    first(app, |o| o.effects.contour.clone()).flatten()
}

fn contour(app: &mut VectorApp, ui: &mut Ui, it: &Item, k: &str) {
    let cur = contour_now(app);
    let on = cur.is_some();
    let mut c = cur.clone().unwrap_or_else(default_contour);
    let mut ch = false;
    let id = pid(ui, k);
    match k {
        "fbe66f8d" => {
            if let Some(v) = fields::spin(app, ui, id, lead(it), cur.as_ref().map(|c| c.steps as f64), Fmt::Num(0, ""), 44.0, "", on, &tip(it)) {
                c.steps = v.clamp(1.0, 999.0) as u32;
                ch = true;
            }
        }
        "d7f2a0b0" => {
            let f = len(app);
            if let Some(v) = fields::spin(app, ui, id, lead(it), cur.as_ref().map(|c| c.offset), f, 64.0, "", on, &tip(it)) {
                c.offset = v.max(0.01);
                ch = true;
            }
        }
        _ => {
            let outline = k == "df167d6c";
            let col = if outline { c.end_outline } else { c.end_fill };
            if let Some(n) = fields::color_combo(ui, id, Some(col), 44.0, on, &tip(it)) {
                let n = n.unwrap_or(Color::WHITE);
                if outline {
                    c.end_outline = n;
                } else {
                    c.end_fill = n;
                }
                ch = true;
            }
        }
    }
    if ch && on {
        edit_sel(app, "Contour", |o| o.effects.contour = Some(c.clone()));
    }
}

fn default_contour() -> vector_core::object::ContourEffect {
    vector_core::object::ContourEffect { dir: ContourDir::Outside, steps: 3, offset: 6.0, end_fill: Color::WHITE, end_outline: Color::BLACK }
}

fn blend(app: &mut VectorApp, ui: &mut Ui, it: &Item, k: &str) {
    let cur = selected_kind(app, |k| if let ObjKind::Blend(b) = k { Some((b.steps, b.rotation)) } else { None });
    let id = pid(ui, k);
    if k == "a869281c" {
        let steps = cur.map(|c| c.0).unwrap_or(app.ts.blend_steps) as f64;
        let r = fields::stacked(app, ui, id, [Lead::None, Lead::None], [Some(steps), None], Fmt::Num(0, ""), 44.0, "", false, true, app.has_doc(), &tip(it));
        if let Some(v) = r[0] {
            let n = v.clamp(1.0, 999.0) as u32;
            app.ts.blend_steps = n;
            if cur.is_some() {
                edit_sel(app, "Blend", |o| {
                    if let ObjKind::Blend(b) = &mut o.kind {
                        b.steps = n;
                    }
                });
            }
        }
    } else if let Some(v) = fields::spin(app, ui, id, lead(it), cur.map(|c| c.1), Fmt::Num(1, ""), 52.0, "°", cur.is_some(), &tip(it)) {
        edit_sel(app, "Blend", |o| {
            if let ObjKind::Blend(b) = &mut o.kind {
                b.rotation = v;
            }
        });
    }
}

fn distort_kind(app: &VectorApp) -> DistortKind {
    first(app, |o| o.effects.distort.as_ref().map(|d| d.kind)).flatten().unwrap_or(app.ts.distort_kind)
}

fn distort(app: &mut VectorApp, ui: &mut Ui, it: &Item, k: &str) {
    let cur = first(app, |o| o.effects.distort.clone()).flatten();
    let on = cur.is_some();
    let Some(mut d) = cur.clone() else {
        fields::empty(app, ui, it);
        return;
    };
    let id = pid(ui, k);
    let mut ch = false;
    match k {
        "03c0c841" | "28f1dce2" => {
            if let Some(v) = fields::spin(app, ui, id, lead(it), Some(d.amount), Fmt::Num(0, ""), 48.0, "", on, &tip(it)) {
                d.amount = v.clamp(-200.0, 200.0);
                ch = true;
            }
        }
        "fca9d3fd" => {
            if let Some(v) = fields::spin(app, ui, id, lead(it), Some(d.frequency), Fmt::Num(0, ""), 44.0, "", on, &tip(it)) {
                d.frequency = v.clamp(0.0, 100.0);
                ch = true;
            }
        }
        // Twister: whole turns, and the degrees beyond them
        "d09509c4" => {
            let turns = (d.amount / 360.0).trunc();
            if let Some(v) = fields::spin(app, ui, id, lead(it), Some(turns), Fmt::Num(0, ""), 44.0, "", on, &tip(it)) {
                d.amount = v.clamp(0.0, 9.0) * 360.0 + d.amount % 360.0;
                ch = true;
            }
        }
        _ => {
            let extra = d.amount % 360.0;
            if let Some(v) = fields::spin(app, ui, id, lead(it), Some(extra), Fmt::Num(0, ""), 44.0, "", on, &tip(it)) {
                d.amount = (d.amount / 360.0).trunc() * 360.0 + v.clamp(0.0, 359.0);
                ch = true;
            }
        }
    }
    if ch {
        edit_sel(app, "Distortion", |o| o.effects.distort = Some(d.clone()));
    }
}

fn block_shadow(app: &mut VectorApp, ui: &mut Ui, it: &Item, k: &str) {
    let cur = first(app, |o| o.effects.block_shadow.clone()).flatten();
    let on = cur.is_some();
    let id = pid(ui, k);
    let mut s = cur.clone().unwrap_or(vector_core::object::BlockShadow { offset: Vec2::new(8.0, 8.0), color: Color::BLACK });
    let mut ch = false;
    match k {
        "8fb33834" => {
            let f = len(app);
            if let Some(v) = fields::spin(app, ui, id, lead(it), cur.as_ref().map(|s| s.offset.hypot()), f, 64.0, "", on, &tip(it)) {
                let a = s.offset.y.atan2(s.offset.x);
                s.offset = Vec2::new(a.cos(), a.sin()) * v.max(0.0);
                ch = true;
            }
        }
        "4239c3f3" => {
            let deg = cur.as_ref().map(|s| (-s.offset.y).atan2(s.offset.x).to_degrees().rem_euclid(360.0));
            if let Some(v) = fields::spin(app, ui, id, lead(it), deg, Fmt::Num(1, ""), 52.0, "°", on, &tip(it)) {
                let l = s.offset.hypot();
                let a = v.to_radians();
                s.offset = Vec2::new(a.cos() * l, -a.sin() * l);
                ch = true;
            }
        }
        _ => {
            if let Some(c) = fields::color_combo(ui, id, Some(s.color), 52.0, on, &tip(it)) {
                s.color = c.unwrap_or(Color::BLACK);
                ch = true;
            }
        }
    }
    if ch && on {
        edit_sel(app, "Block Shadow", |o| o.effects.block_shadow = Some(s.clone()));
    }
}

fn merge_mode(app: &mut VectorApp, ui: &mut Ui, it: &Item) {
    let cur = first(app, |o| o.style.merge);
    let mut pick = None;
    fields::combo(app, ui, Lead::None, cur.map(|m| m.name()).unwrap_or(MergeMode::default().name()), 96.0, cur.is_some(), &tip(it), |_, ui| {
        for m in MergeMode::ALL {
            if ui.selectable_label(Some(m) == cur, m.name()).clicked() {
                pick = Some(m);
            }
        }
    });
    if let Some(m) = pick {
        edit_sel(app, "Merge Mode", |o| o.style.merge = m);
    }
}

fn fill_color(app: &mut VectorApp, ui: &mut Ui, it: &Item, k: &str) {
    let fill = first(app, |o| o.style.fill.clone());
    let id = pid(ui, k);
    match (k, fill) {
        ("815f5d95", Some(Fill::Solid(c))) => {
            if let Some(n) = fields::color_combo(ui, id, Some(c), 52.0, true, &tip(it)) {
                edit_sel(app, "Fill", |o| o.style.fill = n.map(Fill::Solid).unwrap_or(Fill::None));
            }
        }
        ("73149189" | "b54b6289", Some(Fill::Pattern(p))) => {
            let front = k == "73149189";
            if let Some(Some(n)) = fields::color_combo(ui, id, Some(if front { p.fg } else { p.bg }), 52.0, true, &tip(it)) {
                edit_sel(app, "Pattern Fill", |o| {
                    if let Fill::Pattern(p) = &mut o.style.fill {
                        if front {
                            p.fg = n;
                        } else {
                            p.bg = n;
                        }
                    }
                });
            }
        }
        _ => fields::empty(app, ui, it),
    }
}

// ----------------------------------------------------------------------------- option buttons

/// The property bar buttons Vector handles here (first 8 characters of the id).
pub const OPTIONS: &[&str] = &[
    // page: portrait, landscape, default page, current page, treat all objects as filled
    "c5aee741", "e4dbd507", "d02a1f91", "bf372cd8", "b73ed810",
    // proportional ratio
    "d289f32b",
    // rectangle corners, edit corners together, relative corner scaling
    "309de8f6", "aa9fa00c", "761d1d48", "5751ac7e", "4da51d58",
    // ellipse, pie, arc; direction
    "cb723c0a", "24277e3d", "887a0e45", "e8cd9024",
    // star, complex star
    "7624b595", "3e9b0e34",
    // symmetrical and logarithmic spiral
    "a3fd875a", "22050ef9",
    // eraser: round and square nib, auto-reduce
    "737e797f", "4fd43612", "8fff3439",
    // text: bold, italic, underline; alignment
    "bbf8b8dd", "a2e29751", "4e23866f", "6fca7424", "808ee536", "b8f147f2", "40a3ad42", "eed5a12c", "4edf17b6",
    // connector kinds
    "07d6737c", "52514481", "e305fd14",
    // drop and inner shadow; clear shadow
    "e8ef757d", "08f5977a", "bacce2d7",
    // contour: to center, inside, outside; clear contour
    "e9c835fd", "fb15a6d6", "c746acef", "60382072",
    // blend: steps, spacing; clear blend
    "5cefe9f1", "46327bd4", "f1eb59d2",
    // distortion: push and pull, zipper, twister; clear distortion
    "5cc921c9", "de74addf", "7e6c8fad", "e3a841f7",
    // clear envelope, clear extrude, clear block shadow, clear mesh
    "ace700df", "08a395fc", "53bbc973", "1db49c5e",
    // transparency types
    "3827d44e", "311d886a", "40815553", "b70d6f3d", "ede98884", "79a009ac", "ab90096d",
    // fill types
    "9818dc53", "6eba3892", "57d25bb7", "d7f3e609", "067ed1ff", "4b50ea12", "d64625f1", "8fbbe9a2", "c4be6b56",
    // fountain fill types
    "31a78776", "9189e1d9", "b04466f3", "8cabadd3",
    // artistic media: preset, brush, sprayer, calligraphic, expression
    "aebd9625", "f7db3861", "caf8cf4f", "6a38bf24", "e8aed305",
];

/// Whether this id is one of [`OPTIONS`].
pub fn handles(key: &str) -> bool {
    OPTIONS.contains(&key)
}

fn tool_or_sel<T: PartialEq>(cur: T, want: T) -> (bool, bool) {
    (true, cur == want)
}

/// (enabled, checked) of a button of [`OPTIONS`].
pub fn option_state(app: &VectorApp, key: &str) -> Option<(bool, bool)> {
    let doc = app.has_doc();
    let sel = app.has_selection();
    let s = &app.settings;
    Some(match key {
        "c5aee741" | "e4dbd507" => {
            let land = page_wh(app).is_some_and(|(w, h)| w > h);
            (doc, doc && land == (key == "e4dbd507"))
        }
        "d02a1f91" | "bf372cd8" => {
            let all = app.doc().map(|d| d.doc.settings.apply_to_all_pages).unwrap_or(true);
            (doc, doc && all == (key == "d02a1f91"))
        }
        "b73ed810" => (true, s.treat_as_filled),
        "d289f32b" => (sel, app.ts.lock_ratio),
        "309de8f6" | "aa9fa00c" | "761d1d48" => {
            let want = match key {
                "309de8f6" => CornerType::Round,
                "aa9fa00c" => CornerType::Scallop,
                _ => CornerType::Chamfer,
            };
            (doc, corners(app).0 == want)
        }
        "5751ac7e" => (doc, app.ts.lock_corners),
        "4da51d58" => (doc, s.rect_scale_corners),
        "cb723c0a" | "24277e3d" | "887a0e45" => {
            let want = match key {
                "cb723c0a" => EllipseKind::Ellipse,
                "24277e3d" => EllipseKind::Pie,
                _ => EllipseKind::Arc,
            };
            (doc, ellipse_now(app).0 == want)
        }
        "e8cd9024" => (doc && ellipse_now(app).0 != EllipseKind::Ellipse, s.ellipse_clockwise),
        "7624b595" | "3e9b0e34" => tool_or_sel(polygon_now(app).0, if key == "7624b595" { PolygonKind::Star } else { PolygonKind::ComplexStar }),
        "a3fd875a" | "22050ef9" => (doc, s.spiral_log == (key == "22050ef9")),
        // the eraser has a round nib
        "737e797f" => (doc, true),
        "4fd43612" => (false, false),
        "8fff3439" => (doc, s.eraser_reduce_nodes),
        "bbf8b8dd" | "a2e29751" | "4e23866f" => {
            let f = text::format(app);
            (doc, match key {
                "bbf8b8dd" => f.bold,
                "a2e29751" => f.italic,
                _ => f.underline,
            })
        }
        "6fca7424" => (false, false),
        "808ee536" | "b8f147f2" | "40a3ad42" | "eed5a12c" | "4edf17b6" => (doc, Some(text::align(app)) == text_align(key)),
        "07d6737c" | "52514481" | "e305fd14" => {
            let t = connector_tool(key);
            let cur = selected_kind(app, |k| if let ObjKind::Connector(c) = k { Some(c.kind) } else { None });
            let on = match cur {
                Some(k) => connector_kind(t) == k,
                None => app.tool == t,
            };
            (doc, on)
        }
        "e8ef757d" => (doc, app.tool == ToolId::DropShadow),
        "08f5977a" => (doc, app.tool == ToolId::InnerShadow),
        "bacce2d7" => {
            let inner = app.tool == ToolId::InnerShadow;
            (first(app, |o| if inner { o.effects.inner_shadow.is_some() } else { o.effects.drop_shadow.is_some() }).unwrap_or(false), false)
        }
        "e9c835fd" | "fb15a6d6" | "c746acef" => {
            let c = contour_now(app);
            (sel, c.is_some_and(|c| c.dir == contour_dir(key)))
        }
        "60382072" => (contour_now(app).is_some(), false),
        "5cefe9f1" => (doc, true),
        "46327bd4" => (false, false),
        "f1eb59d2" => (selected_kind(app, |k| matches!(k, ObjKind::Blend(_)).then_some(())).is_some(), false),
        "5cc921c9" | "de74addf" | "7e6c8fad" => (doc, distort_kind(app) == distort_of(key)),
        "e3a841f7" => (first(app, |o| o.effects.distort.is_some()).unwrap_or(false), false),
        "ace700df" => (first(app, |o| o.effects.envelope.is_some()).unwrap_or(false), false),
        "08a395fc" => (first(app, |o| o.effects.extrude.is_some()).unwrap_or(false), false),
        "53bbc973" => (first(app, |o| o.effects.block_shadow.is_some()).unwrap_or(false), false),
        "1db49c5e" => (first(app, |o| matches!(o.style.fill, Fill::Mesh(_))).unwrap_or(false), false),
        "3827d44e" | "311d886a" | "40815553" => {
            let t = first(app, |o| match o.style.transparency {
                Transparency::None => "3827d44e",
                Transparency::Uniform(_) => "311d886a",
                Transparency::Gradient(_) => "40815553",
            });
            (sel, t == Some(key))
        }
        // pattern and texture transparencies: not in Vector yet
        "b70d6f3d" | "ede98884" | "79a009ac" | "ab90096d" => (false, false),
        "9818dc53" | "6eba3892" | "57d25bb7" | "d64625f1" => {
            let f = first(app, |o| match o.style.fill {
                Fill::None => "9818dc53",
                Fill::Solid(_) => "6eba3892",
                Fill::Gradient(_) => "57d25bb7",
                Fill::Pattern(_) => "d64625f1",
                Fill::Mesh(_) => "",
            });
            (sel, f == Some(key))
        }
        // vector, bitmap, hatch, texture and PostScript fills: not in Vector yet
        "d7f3e609" | "067ed1ff" | "4b50ea12" | "8fbbe9a2" | "c4be6b56" => (false, false),
        "31a78776" | "9189e1d9" | "b04466f3" | "8cabadd3" => {
            let g = first(app, |o| if let Fill::Gradient(g) = &o.style.fill { Some(g.kind) } else { None }).flatten();
            (g.is_some(), g == Some(gradient_kind(key)))
        }
        "aebd9625" => (doc, app.ts.media_kind <= 2),
        "6a38bf24" => (doc, app.ts.media_kind == 3),
        "e8aed305" => (doc, app.ts.media_kind == 4),
        // brush and sprayer strokes: not in Vector yet
        "f7db3861" | "caf8cf4f" => (false, false),
        _ => return None,
    })
}

fn text_align(key: &str) -> Option<TextAlign> {
    Some(match key {
        "808ee536" => TextAlign::Left,
        "b8f147f2" => TextAlign::Center,
        "40a3ad42" => TextAlign::Right,
        "eed5a12c" => TextAlign::Justify,
        "4edf17b6" => TextAlign::ForceJustify,
        _ => return None,
    })
}

fn connector_tool(key: &str) -> ToolId {
    match key {
        "52514481" => ToolId::RightAngleConnector,
        "e305fd14" => ToolId::RoundedConnector,
        _ => ToolId::StraightConnector,
    }
}

fn connector_kind(t: ToolId) -> vector_core::object::ConnectorKind {
    use vector_core::object::ConnectorKind;
    match t {
        ToolId::RightAngleConnector => ConnectorKind::RightAngle,
        ToolId::RoundedConnector => ConnectorKind::RoundedRightAngle,
        _ => ConnectorKind::Straight,
    }
}

fn contour_dir(key: &str) -> ContourDir {
    match key {
        "e9c835fd" => ContourDir::ToCenter,
        "fb15a6d6" => ContourDir::Inside,
        _ => ContourDir::Outside,
    }
}

fn distort_of(key: &str) -> DistortKind {
    match key {
        "de74addf" => DistortKind::Zipper,
        "7e6c8fad" => DistortKind::Twister,
        _ => DistortKind::PushPull,
    }
}

fn gradient_kind(key: &str) -> GradientKind {
    match key {
        "9189e1d9" => GradientKind::Elliptical,
        "b04466f3" => GradientKind::Conical,
        "8cabadd3" => GradientKind::Rectangular,
        _ => GradientKind::Linear,
    }
}

/// Run a button of [`OPTIONS`].
pub fn press(app: &mut VectorApp, key: &str) {
    if option_state(app, key).is_some_and(|(on, _)| !on) {
        return;
    }
    match key {
        "c5aee741" => app.run(Cmd::Landscape(false)),
        "e4dbd507" => app.run(Cmd::Landscape(true)),
        "d02a1f91" | "bf372cd8" => {
            if let Some(d) = app.doc_mut() {
                d.doc.settings.apply_to_all_pages = key == "d02a1f91";
            }
        }
        "b73ed810" => app.settings.treat_as_filled = !app.settings.treat_as_filled,
        "d289f32b" => app.ts.lock_ratio = !app.ts.lock_ratio,
        "309de8f6" | "aa9fa00c" | "761d1d48" => {
            let (_, radii, _) = corners(app);
            let c = match key {
                "309de8f6" => CornerType::Round,
                "aa9fa00c" => CornerType::Scallop,
                _ => CornerType::Chamfer,
            };
            set_corners(app, c, radii);
        }
        "5751ac7e" => app.ts.lock_corners = !app.ts.lock_corners,
        "4da51d58" => app.settings.rect_scale_corners = !app.settings.rect_scale_corners,
        "cb723c0a" | "24277e3d" | "887a0e45" => {
            let (_, s, e, _) = ellipse_now(app);
            let k = match key {
                "cb723c0a" => EllipseKind::Ellipse,
                "24277e3d" => EllipseKind::Pie,
                _ => EllipseKind::Arc,
            };
            set_ellipse(app, k, s, e);
        }
        "e8cd9024" => app.settings.ellipse_clockwise = !app.settings.ellipse_clockwise,
        "7624b595" | "3e9b0e34" => {
            let (_, n, sharp, _) = polygon_now(app);
            set_polygon(app, if key == "7624b595" { PolygonKind::Star } else { PolygonKind::ComplexStar }, n, sharp);
        }
        "a3fd875a" | "22050ef9" => app.settings.spiral_log = key == "22050ef9",
        "8fff3439" => app.settings.eraser_reduce_nodes = !app.settings.eraser_reduce_nodes,
        "bbf8b8dd" => {
            let v = !text::format(app).bold;
            text::apply(app, "Bold", &move |c| c.bold = v);
        }
        "a2e29751" => {
            let v = !text::format(app).italic;
            text::apply(app, "Italic", &move |c| c.italic = v);
        }
        "4e23866f" => {
            let v = !text::format(app).underline;
            text::apply(app, "Underline", &move |c| c.underline = v);
        }
        "808ee536" | "b8f147f2" | "40a3ad42" | "eed5a12c" | "4edf17b6" => {
            if let Some(a) = text_align(key) {
                text::set_align(app, a);
            }
        }
        "07d6737c" | "52514481" | "e305fd14" => {
            let t = connector_tool(key);
            let k = connector_kind(t);
            if selected_kind(app, |o| matches!(o, ObjKind::Connector(_)).then_some(())).is_some() {
                edit_sel(app, "Connector", |o| {
                    if let ObjKind::Connector(c) = &mut o.kind {
                        c.kind = k;
                    }
                });
            }
            if matches!(app.tool, ToolId::StraightConnector | ToolId::RightAngleConnector | ToolId::RoundedConnector) {
                app.set_tool(t);
            }
        }
        "e8ef757d" => app.set_tool(ToolId::DropShadow),
        "08f5977a" => app.set_tool(ToolId::InnerShadow),
        "bacce2d7" => {
            let inner = app.tool == ToolId::InnerShadow;
            edit_sel(app, "Clear Shadow", |o| {
                if inner {
                    o.effects.inner_shadow = None;
                } else {
                    o.effects.drop_shadow = None;
                }
            });
        }
        "e9c835fd" | "fb15a6d6" | "c746acef" => {
            let mut c = contour_now(app).unwrap_or_else(default_contour);
            c.dir = contour_dir(key);
            edit_sel(app, "Contour", |o| o.effects.contour = Some(c.clone()));
        }
        "60382072" => edit_sel(app, "Clear Contour", |o| o.effects.contour = None),
        "f1eb59d2" => clear_blend(app),
        "5cc921c9" | "de74addf" | "7e6c8fad" => {
            let k = distort_of(key);
            app.ts.distort_kind = k;
            edit_sel(app, "Distortion", |o| {
                if let Some(d) = &mut o.effects.distort {
                    d.kind = k;
                }
            });
        }
        "e3a841f7" => edit_sel(app, "Clear Distortion", |o| o.effects.distort = None),
        "ace700df" => edit_sel(app, "Clear Envelope", |o| o.effects.envelope = None),
        "08a395fc" => edit_sel(app, "Clear Extrude", |o| o.effects.extrude = None),
        "53bbc973" => edit_sel(app, "Clear Block Shadow", |o| o.effects.block_shadow = None),
        "1db49c5e" => edit_sel(app, "Clear Mesh", |o| {
            if matches!(o.style.fill, Fill::Mesh(_)) {
                o.style.fill = o.style.fill.main_color().map(Fill::Solid).unwrap_or(Fill::None);
            }
        }),
        "3827d44e" | "311d886a" | "40815553" => {
            let t = match key {
                "3827d44e" => Transparency::None,
                "311d886a" => Transparency::Uniform(50.0),
                _ => Transparency::Gradient(vector_core::style::Gradient::linear(Color::WHITE, Color::BLACK)),
            };
            edit_sel(app, "Transparency", move |o| o.style.transparency = t.clone());
        }
        "9818dc53" | "6eba3892" | "57d25bb7" | "d64625f1" => {
            let base = first(app, |o| o.style.fill.main_color()).flatten().unwrap_or(app.default_fill);
            let f = match key {
                "9818dc53" => Fill::None,
                "6eba3892" => Fill::Solid(base),
                "57d25bb7" => Fill::Gradient(vector_core::style::Gradient::linear(base, Color::WHITE)),
                _ => Fill::Pattern(vector_core::style::PatternFill { kind: vector_core::style::PatternKind::ALL[0], fg: base, bg: Color::WHITE, tile: 18.0, angle: 0.0, image: None, transform_with_object: true }),
            };
            edit_sel(app, "Fill", move |o| o.style.fill = f.clone());
        }
        "31a78776" | "9189e1d9" | "b04466f3" | "8cabadd3" => {
            let k = gradient_kind(key);
            edit_sel(app, "Fountain Fill", |o| {
                if let Fill::Gradient(g) = &mut o.style.fill {
                    g.kind = k;
                }
            });
        }
        "aebd9625" => app.ts.media_kind = 0,
        "6a38bf24" => app.ts.media_kind = 3,
        "e8aed305" => app.ts.media_kind = 4,
        _ => {}
    }
}

/// Clear Blend: the blend gives way to its start and end objects.
fn clear_blend(app: &mut VectorApp) {
    let Some(d) = app.doc_mut() else { return };
    let sel = d.selection.clone();
    d.modify("Clear Blend", |doc, selection| {
        let mut out = vec![];
        for id in sel {
            let Some((p, l, i)) = doc.locate(id) else { continue };
            let objs = &mut doc.pages[p].layers[l].objects;
            let ObjKind::Blend(b) = objs[i].kind.clone() else {
                out.push(id);
                continue;
            };
            objs.remove(i);
            out.push(b.end.id);
            out.push(b.start.id);
            objs.insert(i, *b.end);
            objs.insert(i, *b.start);
        }
        *selection = out;
    });
}
