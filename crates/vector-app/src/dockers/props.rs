//! Properties docker: the sections of the selection (Outline, Fill, Transparency, Bitmap Effects,
//! the object's own properties, Painterly Brush and Summary), one at a time or all in a column,
//! as the reference lays them out.

use super::coled::{self, ColorEd};
use super::{bold_title, color_pick_w, drop_mark, separator, spin};
use crate::app::VectorApp;
use crate::propbar::edit_sel;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use vector_core::style::{ArrowHead, Gradient, GradientKind, GradientStop, LineCap, LineJoin, MergeMode, OutlinePosition, PatternFill, PatternKind, SpreadMode, VarWidth, WidthProfile};
use vector_core::{Color, Fill, ObjKind, Object, Transparency, Unit};

/// The state of the Properties docker.
#[derive(Clone)]
pub struct PropsUi {
    /// The section shown (0 outline, 1 fill, 2 transparency, 3 effects, 4 object, 5 brush,
    /// 6 summary).
    pub tab: u8,
    /// One section at a time (else all of them in a column).
    pub tabbed: bool,
    /// Show the summary.
    pub summary: bool,
    /// The folds of the outline, fill and transparency sections (true: open).
    pub open: [bool; 3],
    pub fill_ed: ColorEd,
    /// The selected node of the fountain fill and of the fountain transparency.
    pub node: [usize; 2],
    /// The unit of the outline width.
    pub width_unit: Unit,
    /// The selected node of the variable outline.
    pub vo_node: Option<usize>,
    /// The selected bitmap effect.
    pub fx_sel: Option<usize>,
    pub fx_search: String,
    /// The canvas is picking for effect `.0`: its center (`.1` empty) or the color setting `.1`.
    pub fx_pick: Option<(usize, String)>,
    /// The preset chosen on each effect's page (by effect id).
    pub fx_preset: std::collections::HashMap<String, usize>,
    /// All sections in a column: the folded ones, and the section to scroll to.
    pub folded: [bool; 7],
    pub jump: bool,
}

impl Default for PropsUi {
    fn default() -> Self {
        PropsUi {
            tab: 0,
            tabbed: true,
            summary: true,
            open: [true; 3],
            fill_ed: ColorEd { model: 2, ..Default::default() },
            node: [0; 2],
            width_unit: Unit::Points,
            vo_node: None,
            fx_sel: None,
            fx_search: String::new(),
            fx_pick: None,
            fx_preset: Default::default(),
            folded: [false; 7],
            jump: false,
        }
    }
}

// ------------------------------------------------------------------------------ fill and
// transparency types (shared with the property bar)

/// The type of a fill: 0 none, 1 uniform, 2 fountain, 3 vector pattern, 4 bitmap pattern,
/// 5 two-color pattern, 6 mesh.
pub fn fill_kind(f: &Fill) -> u8 {
    match f {
        Fill::None => 0,
        Fill::Solid(_) => 1,
        Fill::Gradient(_) => 2,
        Fill::Pattern(p) => pattern_kind(p),
        Fill::Mesh(_) => 6,
    }
}

fn pattern_kind(p: &PatternFill) -> u8 {
    if p.vector {
        3
    } else if p.image.is_some() {
        4
    } else {
        5
    }
}

/// The type of a transparency: 0 none, 1 uniform, 2 fountain, 3 vector pattern, 4 bitmap
/// pattern, 5 two-color pattern.
pub fn transparency_kind(t: &Transparency) -> u8 {
    match t {
        Transparency::None => 0,
        Transparency::Uniform(_) => 1,
        Transparency::Gradient(_) => 2,
        Transparency::Pattern { pattern, .. } => pattern_kind(pattern),
    }
}

/// The pattern a new pattern fill or transparency of `kind` (3, 4, 5) starts with: the
/// document's first pattern of that kind, else a built-in one.
fn default_pattern(app: &mut VectorApp, kind: u8, fg: Color, bg: Color) -> PatternFill {
    let base = PatternFill { kind: PatternKind::Dots, fg, bg, tile: 36.0, angle: 0.0, image: None, transform_with_object: true, vector: false, mirror: 0 };
    let named = app.doc().map(|d| d.doc.patterns.clone()).unwrap_or_default();
    match kind {
        3 => named.iter().find(|n| n.fill.vector).map(|n| n.fill.clone()).unwrap_or(PatternFill { kind: PatternKind::Diamonds, vector: true, ..base }),
        4 => match named.iter().find(|n| !n.fill.vector && n.fill.image.is_some()) {
            Some(n) => n.fill.clone(),
            None => {
                let id = app.doc_mut().map(|d| bitmap_pattern_image(&mut d.doc)).unwrap_or(0);
                PatternFill { image: Some(id), ..base }
            }
        },
        _ => base,
    }
}

/// The document's built-in bitmap pattern tile (a stone texture), added the first time.
fn bitmap_pattern_image(doc: &mut vector_core::Document) -> u64 {
    if let Some((id, _)) = doc.images.iter().find(|(_, im)| im.name == "Bitmap Pattern") {
        return *id;
    }
    let n = 64u32;
    let mut rgba = Vec::with_capacity((n * n * 4) as usize);
    let hash = |x: u32, y: u32| -> f64 {
        let mut h = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263);
        h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
        ((h ^ (h >> 16)) & 0xffff) as f64 / 65535.0
    };
    for y in 0..n {
        for x in 0..n {
            // a few octaves of tiling value noise
            let mut v = 0.0;
            let mut amp = 0.5;
            for o in 0..4u32 {
                let cell = 16 >> o;
                let (cx, cy) = (x / cell, y / cell);
                let (fx, fy) = ((x % cell) as f64 / cell as f64, (y % cell) as f64 / cell as f64);
                let m = n / cell;
                let g = |a: u32, b: u32| hash((a % m) + o * 97, (b % m) + o * 31);
                let top = g(cx, cy) * (1.0 - fx) + g(cx + 1, cy) * fx;
                let bot = g(cx, cy + 1) * (1.0 - fx) + g(cx + 1, cy + 1) * fx;
                v += amp * (top * (1.0 - fy) + bot * fy);
                amp *= 0.5;
            }
            let r = (120.0 + v * 110.0) as u8;
            let g = (82.0 + v * 90.0) as u8;
            let b = (70.0 + v * 70.0) as u8;
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    doc.add_image(vector_core::document::ImageData { width: n, height: n, rgba: std::sync::Arc::new(rgba), encoded: std::sync::Arc::new(vec![]), format: "png".into(), name: "Bitmap Pattern".into() })
}

/// A new fill of type `kind` (see [`fill_kind`]) from the color `base`.
pub fn new_fill(app: &mut VectorApp, kind: u8, base: Color) -> Fill {
    match kind {
        0 => Fill::None,
        1 => Fill::Solid(base),
        2 => Fill::Gradient(Gradient::linear(base, Color::WHITE)),
        k @ 3..=5 => Fill::Pattern(default_pattern(app, k, base, Color::WHITE)),
        _ => Fill::Mesh(vector_core::style::MeshFill::new(3, 3, base)),
    }
}

/// A new transparency of type `kind` (see [`transparency_kind`]).
pub fn new_transparency(app: &mut VectorApp, kind: u8) -> Transparency {
    match kind {
        0 => Transparency::None,
        1 => Transparency::Uniform(50.0),
        2 => Transparency::Gradient(Gradient::linear(Color::WHITE, Color::BLACK)),
        k => Transparency::Pattern { pattern: default_pattern(app, k, Color::BLACK, Color::WHITE), front: 100.0, back: 0.0 },
    }
}

// ------------------------------------------------------------------------------ helpers

fn first(app: &VectorApp) -> Option<Object> {
    let d = app.doc()?;
    d.doc.find(*d.selection.first()?).cloned()
}

/// A color button opening the color editor (`alpha`: with an opacity slider).
pub fn color_edit(ui: &mut Ui, c: &mut Color, alpha: bool) -> bool {
    let resp = crate::widgets::color_well(ui, Some(*c), egui::vec2(40.0, 18.0));
    let key = format!("{:?}", resp.id);
    let mut changed = false;
    let open = egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        let mut v = Some(*c);
        if coled::picker(ui, &key, &mut v, false) {
            if let Some(n) = v {
                *c = n.with_alpha(c.a);
                changed = true;
            }
        }
        if alpha {
            ui.horizontal(|ui| {
                ui.label("Opacity:");
                let mut a = c.a as f64 / 255.0 * 100.0;
                if ui.add(egui::Slider::new(&mut a, 0.0..=100.0).suffix(" %").max_decimals(0)).changed() {
                    *c = c.with_alpha((a / 100.0 * 255.0).round() as u8);
                    changed = true;
                }
            });
        }
    });
    if open.is_none() {
        coled::picker_closed(ui.ctx(), &key);
    }
    changed
}

/// Continuous edits (dragging a slider, a color viewer) make one undo step a second.
pub(crate) fn edit_live(app: &mut VectorApp, label: &str, mut f: impl FnMut(&mut Object)) {
    let now = app.time();
    let merge = now - app.ts.last_coalesce < 1.0;
    app.ts.last_coalesce = now;
    let Some(d) = app.doc_mut() else { return };
    let sel = d.selection.clone();
    if !merge {
        d.checkpoint(label);
    }
    for id in sel {
        if let Some(o) = d.doc.find_mut(id) {
            f(o);
        }
    }
    d.history.touch();
}

/// The small square at the right end of a row (the style override mark).
fn mark(ui: &mut Ui) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let (r, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
        ui.painter().rect_stroke(Rect::from_center_size(r.center(), vec2(7.0, 7.0)), 0.0, Stroke::new(1.0, Color32::from_gray(96)), egui::StrokeKind::Inside);
    });
}

fn column_id() -> egui::Id {
    egui::Id::unique("pr_column")
}

/// The title of a section (the column of all sections shows its own headers instead).
fn title(ui: &mut Ui, text: &str) {
    if !ui.ctx().data(|d| d.get_temp::<bool>(column_id()).unwrap_or(false)) {
        row(ui, |ui| bold_title(ui, text));
    }
}

/// The title of a fill or transparency type, `add`: with Save as new at its right.
fn subtitle(app: &mut VectorApp, ui: &mut Ui, text: &str, add: bool) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(text).color(theme::TEXT));
        if add {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                super::icon_button(app, ui, "add_circle", "Save as new", false);
            });
        } else {
            mark(ui);
        }
    });
}

/// The mark inside a row, right after a control.
fn dot(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(12.0, 18.0), Sense::hover());
    ui.painter().rect_stroke(Rect::from_center_size(r.center(), vec2(7.0, 7.0)), 0.0, Stroke::new(1.0, Color32::from_gray(96)), egui::StrokeKind::Inside);
}

/// A row of controls with the mark at its right end.
fn row<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let r = f(ui);
        mark(ui);
        r
    })
    .inner
}

/// A label in a column `w` points wide, right-aligned.
fn rlabel(ui: &mut Ui, text: &str, w: f32, enabled: bool) {
    let (r, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    ui.painter().text(pos2(r.right() - 6.0, r.center().y), egui::Align2::RIGHT_CENTER, text, egui::FontId::proportional(12.5), if enabled { theme::TEXT } else { theme::DISABLED });
}

fn label(ui: &mut Ui, text: &str, enabled: bool) {
    ui.label(egui::RichText::new(text).color(if enabled { theme::TEXT } else { theme::DISABLED }));
}

/// An icon button that shows whether it is on.
fn tbtn(app: &mut VectorApp, ui: &mut Ui, icon: &str, tip: &str, enabled: bool, on: bool) -> bool {
    crate::widgets::icon_button(&mut app.icons, ui, icon, tip, enabled, on).clicked() && enabled
}

/// The fold arrow centered under a section's first controls.
fn fold(ui: &mut Ui, open: &mut bool) {
    let w = ui.available_width() - 14.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 14.0), Sense::click());
    let c = if resp.hovered() { Color32::WHITE } else { theme::TEXT_DIM };
    if *open {
        crate::widgets::chevron_up(ui.painter(), r.center(), 3.5, c);
    } else {
        crate::widgets::chevron(ui.painter(), r.center(), 3.5, c);
    }
    if resp.clicked() {
        *open = !*open;
    }
}

fn gap(ui: &mut Ui) {
    ui.add_space(6.0);
    separator(ui);
    ui.add_space(6.0);
}

/// A slider without its value.
fn slider(ui: &mut Ui, v: &mut f64, range: std::ops::RangeInclusive<f64>, w: f32) -> bool {
    ui.spacing_mut().slider_width = w;
    ui.add(egui::Slider::new(v, range).show_value(false)).changed()
}

/// A field with a + mark (the reference's slider fields): drag it or type a value.
fn plus_field(ui: &mut Ui, v: &mut f64, range: std::ops::RangeInclusive<f64>, suffix: &str, w: f32) -> bool {
    plus_field_d(ui, v, range, suffix, w, 0)
}

fn plus_field_d(ui: &mut Ui, v: &mut f64, range: std::ops::RangeInclusive<f64>, suffix: &str, w: f32, decimals: usize) -> bool {
    let (lo, hi) = (*range.start(), *range.end());
    let r = ui.add_sized(vec2(w, 22.0), egui::DragValue::new(v).range(range).speed(((hi - lo) / 200.0).max(0.1)).fixed_decimals(decimals).suffix(suffix));
    let c = pos2(r.rect.right() - 9.0, r.rect.center().y);
    let col = if ui.is_enabled() { theme::TEXT } else { theme::DISABLED };
    ui.painter().line_segment([c - vec2(4.0, 0.0), c + vec2(4.0, 0.0)], Stroke::new(1.0, col));
    ui.painter().line_segment([c - vec2(0.0, 4.0), c + vec2(0.0, 4.0)], Stroke::new(1.0, col));
    r.changed()
}

/// A preview box with a drop arrow (the pickers of fills and transparencies).
fn picker_box(ui: &mut Ui, w: f32, h: f32, paint: impl FnOnce(&egui::Painter, Rect)) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(w + 18.0, h), Sense::click());
    ui.painter().rect_filled(r, 2.0, Color32::from_rgb(0x33, 0x33, 0x33));
    let inner = Rect::from_min_size(r.min, vec2(w, h));
    paint(ui.painter(), inner);
    crate::widgets::chevron(ui.painter(), pos2(r.right() - 9.0, r.center().y), 3.0, theme::TEXT);
    resp
}

fn paint_gradient(p: &egui::Painter, r: Rect, g: &Gradient) {
    crate::widgets::checker(p, r, 4.0);
    let n = 48;
    for i in 0..n {
        let t = i as f64 / (n - 1) as f64;
        let x0 = r.left() + r.width() * i as f32 / n as f32;
        let x1 = r.left() + r.width() * (i + 1) as f32 / n as f32 + 0.5;
        p.rect_filled(Rect::from_min_max(pos2(x0, r.top()), pos2(x1, r.bottom())), 0.0, theme::to_c32(g.color_at(t)));
    }
}

fn paint_pattern(app: &mut VectorApp, p: &egui::Painter, r: Rect, pf: &PatternFill) {
    match pf.image.and_then(|id| app.doc().and_then(|d| d.doc.images.get(&id).cloned())) {
        Some(img) => {
            // a few tiles of the image
            let key = format!("pattern_thumb_{}_{}_{}", img.width, img.height, std::sync::Arc::as_ptr(&img.rgba) as usize);
            let kid = egui::Id::unique(key.as_str());
            let tex = p.ctx().data_mut(|m| m.get_temp::<egui::TextureHandle>(kid));
            let tex = match tex {
                Some(t) => t,
                None => {
                    let ci = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba);
                    let t = p.ctx().load_texture(key.clone(), ci, egui::TextureOptions::LINEAR);
                    p.ctx().data_mut(|m| m.insert_temp(kid, t.clone()));
                    t
                }
            };
            let s = r.height() / 2.0;
            let mut y = r.top();
            while y < r.bottom() {
                let mut x = r.left();
                while x < r.right() {
                    let cell = Rect::from_min_size(pos2(x, y), vec2(s, s)).intersect(r);
                    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(cell.width() / s, cell.height() / s));
                    p.image(tex.id(), cell, uv, Color32::WHITE);
                    x += s;
                }
                y += s;
            }
        }
        None => {
            p.rect_filled(r, 0.0, theme::to_c32(pf.bg));
            let s = r.height() / 4.0;
            let fg = theme::to_c32(pf.fg);
            let p = p.with_clip_rect(r);
            let mut y = r.top();
            let mut row = 0;
            while y < r.bottom() {
                let mut x = r.left() + if row % 2 == 1 && pf.kind != PatternKind::Checker { s / 2.0 } else { 0.0 };
                let mut col = 0;
                while x < r.right() {
                    let c = pos2(x + s / 2.0, y + s / 2.0);
                    match pf.kind {
                        PatternKind::Checker => {
                            if (col + row) % 2 == 0 {
                                p.rect_filled(Rect::from_min_size(pos2(x, y), vec2(s, s)), 0.0, fg);
                            }
                        }
                        PatternKind::HorizontalLines => {
                            p.line_segment([pos2(x, c.y), pos2(x + s, c.y)], Stroke::new(s * 0.25, fg));
                        }
                        PatternKind::VerticalLines => {
                            p.line_segment([pos2(c.x, y), pos2(c.x, y + s)], Stroke::new(s * 0.25, fg));
                        }
                        PatternKind::DiagonalLines | PatternKind::CrossHatch => {
                            p.line_segment([pos2(x, y + s), pos2(x + s, y)], Stroke::new(s * 0.2, fg));
                            if pf.kind == PatternKind::CrossHatch {
                                p.line_segment([pos2(x, y), pos2(x + s, y + s)], Stroke::new(s * 0.2, fg));
                            }
                        }
                        PatternKind::Grid => {
                            p.rect_stroke(Rect::from_min_size(pos2(x, y), vec2(s, s)), 0.0, Stroke::new(1.0, fg), egui::StrokeKind::Inside);
                        }
                        PatternKind::Bricks => {
                            p.rect_stroke(Rect::from_min_size(pos2(x, y), vec2(s, s * 0.5)), 0.0, Stroke::new(1.0, fg), egui::StrokeKind::Inside);
                        }
                        PatternKind::Waves => {
                            let pts: Vec<egui::Pos2> = (0..=8).map(|i| pos2(x + s * i as f32 / 8.0, c.y + (i as f32 / 8.0 * std::f32::consts::TAU).sin() * s * 0.2)).collect();
                            p.add(egui::Shape::line(pts, Stroke::new(1.2, fg)));
                        }
                        PatternKind::Diamonds => {
                            p.add(egui::Shape::convex_polygon(vec![pos2(c.x, y), pos2(x + s, c.y), pos2(c.x, y + s), pos2(x, c.y)], fg, Stroke::NONE));
                        }
                        PatternKind::Dots => {
                            p.circle_filled(c, s * 0.3, fg);
                        }
                    }
                    x += s;
                    col += 1;
                }
                y += s;
                row += 1;
            }
        }
    }
}

// ------------------------------------------------------------------------------ the docker

/// The icon and the tooltip of the object tab of a selection.
fn object_tab(o: &Object) -> (&'static str, &'static str) {
    match &o.kind {
        ObjKind::Rect(_) => ("rectangle", "Jump to the rectangle properties."),
        ObjKind::Ellipse(_) => ("ellipse", "Jump to the ellipse properties."),
        ObjKind::Polygon(p) if p.kind != vector_core::object::PolygonKind::Polygon => ("star", "Jump to the star properties."),
        ObjKind::Polygon(_) => ("polygon", "Jump to the polygon properties."),
        ObjKind::Image(_) => ("bitmap", "Jump to the bitmap properties."),
        ObjKind::Symbol(_) => ("symbols", "Jump to the symbol properties."),
        ObjKind::Text(_) => ("text", "Jump to the frame properties."),
        _ => ("bezier", "Jump to the curve properties."),
    }
}

pub fn properties(app: &mut VectorApp, ui: &mut Ui) {
    let o = first(app);
    let mut st = app.dockers.ui.pr.clone();
    // the toolbar: the sections, then the two view buttons
    ui.add_space(2.0);
    let (obj_icon, obj_tip) = o.as_ref().map(object_tab).unwrap_or(("rectangle", "Jump to the rectangle properties."));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let tabs: [(&str, &str); 7] = [
            ("outline_icon", "Jump to the outline properties."),
            ("fill_icon", "Jump to the fill properties."),
            ("transparency", "Jump to the transparency properties."),
            ("pr_fx", "Jump to the effects properties."),
            (obj_icon, obj_tip),
            ("painterly", "Jump to the Painterly brushstroke properties."),
            ("pr_summary", "Jump to the summary properties."),
        ];
        for (i, (icon, tip)) in tabs.iter().enumerate() {
            if tbtn(app, ui, icon, tip, true, st.tab == i as u8) {
                st.tab = i as u8;
                st.jump = true;
                if i == 6 {
                    st.summary = true;
                }
                st.folded[i] = false;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if tbtn(app, ui, "pr_show_summary", "Show or hide the summary.", true, st.summary) {
                st.summary = !st.summary;
                if !st.summary && st.tab == 6 {
                    st.tab = 4;
                }
            }
            let (r, _) = ui.allocate_exact_size(vec2(1.0, 22.0), Sense::hover());
            ui.painter().line_segment([r.center_top(), r.center_bottom()], Stroke::new(1.0, theme::SEP));
            if tbtn(app, ui, if st.tabbed { "pr_tabbed" } else { "pr_all" }, "Show or hide all properties.", true, st.tabbed) {
                st.tabbed = !st.tabbed;
            }
        });
    });
    ui.add_space(4.0);
    separator(ui);
    ui.add_space(6.0);
    if !app.has_doc() {
        app.dockers.ui.pr = st;
        return super::no_doc(ui);
    }
    let Some(o) = o else {
        // nothing selected: the document's summary
        bold_title(ui, "Summary");
        ui.add_space(4.0);
        doc_summary(app, ui);
        app.dockers.ui.pr = st;
        return;
    };
    if !st.summary && st.tab == 6 {
        st.tab = 4;
    }
    let sections: Vec<u8> = if st.tabbed { vec![st.tab] } else { (0..7).filter(|s| *s != 6 || st.summary).collect() };
    let obj_title = match &o.kind {
        ObjKind::Polygon(p) if p.kind != vector_core::object::PolygonKind::Polygon => "Star",
        k => k.type_name(),
    };
    let names = ["Outline", "Fill", "Transparency", "Bitmap Effects", obj_title, "Painterly Brush", "Summary"];
    ui.ctx().data_mut(|d| d.insert_temp(column_id(), !st.tabbed));
    for (k, s) in sections.iter().enumerate() {
        let s = *s;
        if k > 0 {
            gap(ui);
        }
        if !st.tabbed {
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 20.0, 20.0), Sense::click());
            let c = pos2(r.left() + 7.0, r.center().y);
            if st.folded[s as usize] {
                crate::widgets::chevron(ui.painter(), c, 3.0, theme::TEXT);
            } else {
                crate::widgets::chevron_up(ui.painter(), c, 3.0, theme::TEXT);
            }
            ui.painter().text(pos2(r.left() + 16.0, r.center().y), egui::Align2::LEFT_CENTER, names[s as usize], egui::FontId::proportional(13.0), Color32::WHITE);
            if resp.clicked() {
                st.folded[s as usize] = !st.folded[s as usize];
            }
            if st.jump && s == st.tab {
                resp.scroll_to_me(Some(egui::Align::TOP));
                st.jump = false;
            }
            if st.folded[s as usize] {
                continue;
            }
            ui.add_space(4.0);
        }
        ui.push_id(("pr_section", s), |ui| match s {
            0 => outline(app, ui, &o, &mut st),
            1 => fill(app, ui, &o, &mut st),
            2 => transparency(app, ui, &o, &mut st),
            3 => effects(app, ui, &o, &mut st),
            4 => object(app, ui, &o),
            5 => brush(app, ui, &o),
            _ => summary(app, ui, &o),
        });
    }
    ui.ctx().data_mut(|d| d.remove::<bool>(column_id()));
    st.jump = false;
    app.dockers.ui.pr = st;
}

// ------------------------------------------------------------------------------ outline

/// The outline widths of the list (points; 0 hairline).
const WIDTHS: [f64; 14] = [0.0, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 8.0, 10.0, 12.0, 16.0, 24.0, 36.0];
/// The units of the outline width, in the order of their list.
const WIDTH_UNITS: [Unit; 11] = [Unit::Inches, Unit::Millimeters, Unit::Picas, Unit::Points, Unit::Pixels, Unit::Ciceros, Unit::Didots, Unit::Feet, Unit::Yards, Unit::Centimeters, Unit::Meters];

/// The line styles of the list: the built-in ones, then those made with Edit Line Style.
pub fn line_styles(app: &VectorApp) -> Vec<Vec<f64>> {
    let mut v: Vec<Vec<f64>> = vector_core::style::dash_presets().into_iter().map(|p| p.1).collect();
    for extra in [vec![2.0, 2.0], vec![3.0, 3.0], vec![6.0, 2.0], vec![12.0, 4.0], vec![1.0, 4.0], vec![8.0, 3.0, 2.0, 3.0], vec![10.0, 3.0, 1.0, 3.0, 1.0, 3.0, 1.0, 3.0], vec![16.0, 6.0]] {
        if !v.contains(&extra) {
            v.push(extra);
        }
    }
    for d in &app.settings.custom_dashes {
        if !v.contains(d) {
            v.push(d.clone());
        }
    }
    v
}

pub fn dash_line(p: &egui::Painter, r: Rect, dash: &[f64], c: Color32) {
    let y = r.center().y;
    if dash.is_empty() {
        p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.5, c));
        return;
    }
    let mut x = r.left();
    let mut i = 0;
    while x < r.right() {
        let l = (dash[i % dash.len()] as f32 * 1.5).max(1.0);
        if i % 2 == 0 {
            p.line_segment([pos2(x, y), pos2((x + l).min(r.right()), y)], Stroke::new(1.5, c));
        }
        x += l;
        i += 1;
    }
}

fn outline(app: &mut VectorApp, ui: &mut Ui, o: &Object, st: &mut PropsUi) {
    let ol = o.style.outline.clone();
    let has = ol.color.is_some();
    let dpi = app.doc().map(|d| d.doc.settings.dpi).unwrap_or(300.0);
    title(ui, "Outline");
    ui.add_space(4.0);
    // color
    row(ui, |ui| {
        let mut c = ol.color;
        if color_pick_tabs(ui, "pr_ol_color", &mut c, 84.0) {
            edit_live(app, "Outline Color", |x| x.style.outline.color = c);
        }
    });
    // width and its unit
    row(ui, |ui| {
        let u = st.width_unit;
        let shown = if !has { "None".to_string() } else if ol.width <= 0.0 { "Hairline".to_string() } else { format!("{:.*} {}", u.decimals().min(2), u.to_unit(ol.width, dpi), u.suffix()) };
        let id = egui::Id::unique("pr_ol_width");
        let mut text = ui.data_mut(|m| m.get_temp::<String>(id)).unwrap_or_else(|| shown.clone());
        let mut pick: Option<Option<f64>> = None;
        ui.spacing_mut().item_spacing.x = 0.0;
        let r = ui.add_sized(vec2(70.0, 22.0), egui::TextEdit::singleline(&mut text));
        if r.has_focus() {
            ui.data_mut(|m| m.insert_temp(id, text.clone()));
        } else {
            ui.data_mut(|m| m.remove::<String>(id));
        }
        if r.lost_focus() {
            let t = text.trim().to_lowercase();
            if t == "none" {
                pick = Some(None);
            } else if t == "hairline" {
                pick = Some(Some(0.0));
            } else if let Some(v) = Unit::parse_value(&t, u, dpi) {
                pick = Some(Some(v.max(0.0)));
            }
        }
        let (ar, aresp) = ui.allocate_exact_size(vec2(16.0, 22.0), Sense::click());
        ui.painter().rect_filled(ar, 2.0, Color32::from_rgb(0x33, 0x33, 0x33));
        crate::widgets::chevron(ui.painter(), ar.center(), 3.0, theme::TEXT);
        egui::Popup::menu(&aresp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.set_min_width(80.0);
            if ui.selectable_label(!has, "None").clicked() {
                pick = Some(None);
                ui.close();
            }
            for w in WIDTHS {
                let name = if w == 0.0 { "Hairline".to_string() } else { format!("{w:.2} pt") };
                if ui.selectable_label(has && (ol.width - w).abs() < 1e-6, name).clicked() {
                    pick = Some(Some(w));
                    ui.close();
                }
            }
        });
        ui.add_space(6.0);
        let mut ui_i = WIDTH_UNITS.iter().position(|x| *x == u).unwrap_or(3);
        let names: Vec<&str> = WIDTH_UNITS.iter().map(|u| u.name()).collect();
        if super::choice(ui, "pr_ol_units", &mut ui_i, &names, 80.0) {
            st.width_unit = WIDTH_UNITS[ui_i];
        }
        match pick {
            Some(None) => edit_sel(app, "Outline", |x| x.style.outline.color = None),
            Some(Some(w)) => edit_sel(app, "Outline Width", |x| {
                x.style.outline.width = w;
                if x.style.outline.color.is_none() {
                    x.style.outline.color = Some(Color::BLACK);
                }
            }),
            None => {}
        }
    });
    // line style and its editor
    row(ui, |ui| {
        let styles = line_styles(app);
        let (r, resp) = ui.allocate_exact_size(vec2(170.0, 22.0), Sense::click());
        ui.painter().rect_filled(r, 2.0, Color32::from_rgb(0x33, 0x33, 0x33));
        dash_line(ui.painter(), Rect::from_min_max(r.min + vec2(6.0, 0.0), r.max - vec2(22.0, 0.0)), &ol.dash, theme::TEXT);
        crate::widgets::chevron(ui.painter(), pos2(r.right() - 9.0, r.center().y), 3.0, theme::TEXT);
        let resp = resp.on_hover_text("Line style\nChoose a style for the line or outline.");
        let mut pick = None;
        egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for d in &styles {
                    let (rr, rs) = ui.allocate_exact_size(vec2(164.0, 16.0), Sense::click());
                    if *d == ol.dash || rs.hovered() {
                        ui.painter().rect_filled(rr, 0.0, if *d == ol.dash { theme::ACCENT } else { theme::HOVER });
                    }
                    dash_line(ui.painter(), rr.shrink2(vec2(6.0, 0.0)), d, theme::TEXT);
                    if rs.clicked() {
                        pick = Some(d.clone());
                        ui.close();
                    }
                }
            });
        });
        if let Some(d) = pick {
            edit_sel(app, "Line Style", move |x| x.style.outline.dash = d.clone());
        }
        ui.add_space(4.0);
        if super::icon_button(app, ui, "more", "Edit Line Style", true) {
            app.dialogs.line_style = Some(crate::dialogs::linestyle::LineStyleState::from_dash(&ol.dash));
        }
    });
    // dashes
    row(ui, |ui| {
        let dashed = !ol.dash.is_empty();
        for (i, (icon, tip)) in [("dash_default", "Default dashes"), ("dash_align", "Align dashes"), ("dash_fixed", "Fixed dashes")].iter().enumerate() {
            if tbtn(app, ui, icon, tip, dashed, dashed && ol.dash_mode == i as u8) {
                edit_sel(app, "Dashes", |x| x.style.outline.dash_mode = i as u8);
            }
        }
    });
    ui.add_space(4.0);
    fold(ui, &mut st.open[0]);
    if !st.open[0] {
        return;
    }
    // miter limit, corners, caps, position
    let lw = 86.0;
    row(ui, |ui| {
        rlabel(ui, "Miter limit:", lw, true);
        let mut ml = miter_degrees(ol.miter_limit);
        if spin(ui, &mut ml, 1.0..=180.0, 1.0, 1, "", 86.0) {
            let v = miter_from_degrees(ml);
            edit_sel(app, "Miter Limit", |x| x.style.outline.miter_limit = v);
        }
    });
    row(ui, |ui| {
        rlabel(ui, "Corners:", lw, true);
        for (icon, tip, j) in [("join_miter", "Miter corners", LineJoin::Miter), ("join_round", "Round corners", LineJoin::Round), ("join_bevel", "Bevel corners", LineJoin::Bevel)] {
            if tbtn(app, ui, icon, tip, true, ol.join == j) {
                edit_sel(app, "Corners", |x| x.style.outline.join = j);
            }
        }
    });
    row(ui, |ui| {
        rlabel(ui, "Line caps:", lw, true);
        for (icon, tip, c) in [("cap_butt", "Square cap", LineCap::Butt), ("cap_round", "Round cap", LineCap::Round), ("cap_square", "Extended square cap", LineCap::Square)] {
            if tbtn(app, ui, icon, tip, true, ol.cap == c) {
                edit_sel(app, "Line Caps", |x| x.style.outline.cap = c);
            }
        }
    });
    row(ui, |ui| {
        rlabel(ui, "Position:", lw, true);
        for (icon, tip, p) in [("pos_outside", "Outside outline", OutlinePosition::Outside), ("pos_center", "Centered outline", OutlinePosition::Center), ("pos_inside", "Inside outline", OutlinePosition::Inside)] {
            if tbtn(app, ui, icon, tip, true, ol.position == p) {
                edit_sel(app, "Outline Position", |x| x.style.outline.position = p);
            }
        }
    });
    // arrowheads
    gap(ui);
    row(ui, |ui| bold_title(ui, "Arrowheads"));
    ui.add_space(4.0);
    for start in [true, false] {
        row(ui, |ui| {
            let (ar, _) = ui.allocate_exact_size(vec2(18.0, 22.0), Sense::hover());
            let c = ar.center();
            let d = if start { -1.0 } else { 1.0 };
            ui.painter().line_segment([c - vec2(6.0, 0.0), c + vec2(6.0, 0.0)], Stroke::new(1.5, theme::TEXT));
            ui.painter().line_segment([c + vec2(6.0 * d, 0.0), c + vec2(2.0 * d, -4.0)], Stroke::new(1.5, theme::TEXT));
            ui.painter().line_segment([c + vec2(6.0 * d, 0.0), c + vec2(2.0 * d, 4.0)], Stroke::new(1.5, theme::TEXT));
            ui.add_space(4.0);
            let cur = if start { ol.start_arrow } else { ol.end_arrow };
            arrow_combo(app, ui, cur, start, ol.share_arrows);
            ui.add_space(4.0);
            let more = super::icon_button_resp(app, ui, "more", "Arrowhead settings\nModify arrowhead attributes such as size, offset, and rotation angle.", true);
            drop_mark(ui, more.rect);
            egui::Popup::menu(&more).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.set_min_width(120.0);
                if ui.add_enabled(cur.is_some(), egui::Button::new("Attributes...")).clicked() {
                    app.run(crate::commands::Cmd::CreateArrowhead);
                    ui.close();
                }
                ui.separator();
                if ui.button("New...").clicked() {
                    app.run(crate::commands::Cmd::CreateArrowhead);
                    ui.close();
                }
                let custom = matches!(cur, Some(ArrowHead::Custom(_)));
                if ui.add_enabled(custom, egui::Button::new("Edit...")).clicked() {
                    app.run(crate::commands::Cmd::CreateArrowhead);
                    ui.close();
                }
                if ui.add_enabled(custom, egui::Button::new("Delete")).clicked() {
                    edit_sel(app, "Arrowhead", |x| {
                        if start {
                            x.style.outline.start_arrow = None;
                        } else {
                            x.style.outline.end_arrow = None;
                        }
                    });
                    ui.close();
                }
            });
        });
    }
    row(ui, |ui| {
        ui.add_space(20.0);
        let mut share = ol.share_arrows;
        if ui.checkbox(&mut share, "Share Attributes").changed() {
            edit_sel(app, "Arrowheads", |x| {
                x.style.outline.share_arrows = share;
                if share {
                    x.style.outline.end_arrow = x.style.outline.start_arrow;
                }
            });
        }
    });
    // variable outline
    gap(ui);
    ui.horizontal(|ui| {
        bold_title(ui, "Variable Outline");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            if super::icon_button(app, ui, "variable_outline", "Variable Outline tool", true) {
                app.set_tool(crate::tools::ToolId::VariableOutline);
            }
        });
    });
    ui.add_space(4.0);
    variable_outline(app, ui, &ol, st);
    // calligraphy
    gap(ui);
    ui.horizontal(|ui| {
        bold_title(ui, "Calligraphy");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            if super::icon_button(app, ui, "reset", "Default nib", (ol.stretch - 100.0).abs() > 1e-6 || ol.nib_angle != 0.0) {
                edit_sel(app, "Calligraphy", |x| {
                    x.style.outline.stretch = 100.0;
                    x.style.outline.nib_angle = 0.0;
                });
            }
        });
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                super::icon_button(app, ui, "cal_stretch", "Stretch", false);
                let mut s = ol.stretch;
                if spin(ui, &mut s, 1.0..=100.0, 1.0, 0, "", 90.0) {
                    edit_live(app, "Calligraphy", |x| x.style.outline.stretch = s);
                }
            });
            ui.horizontal(|ui| {
                super::icon_button(app, ui, "cal_angle", "Angle", false);
                let mut a = ol.nib_angle;
                if spin(ui, &mut a, -360.0..=360.0, 1.0, 1, "", 90.0) {
                    edit_live(app, "Calligraphy", |x| x.style.outline.nib_angle = a);
                }
            });
        });
        // the nib: drag in it to turn it
        let (r, resp) = ui.allocate_exact_size(vec2(48.0, 48.0), Sense::click_and_drag());
        ui.painter().rect_filled(r, 0.0, Color32::WHITE);
        let nib = Rect::from_center_size(r.center(), vec2(30.0, (30.0 * (ol.stretch / 100.0) as f32).max(1.5)));
        let rot = egui::emath::Rot2::from_angle(-(ol.nib_angle as f32).to_radians());
        let pts: Vec<egui::Pos2> = [nib.left_top(), nib.right_top(), nib.right_bottom(), nib.left_bottom()].iter().map(|p| r.center() + rot * (*p - r.center())).collect();
        ui.painter().add(egui::Shape::convex_polygon(pts, Color32::from_gray(0x22), Stroke::NONE));
        if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
            let d = p - r.center();
            let a = ((-d.y).atan2(d.x).to_degrees() as f64).round();
            edit_live(app, "Calligraphy", |x| x.style.outline.nib_angle = a);
        }
        mark(ui);
    });
    gap(ui);
    let mut behind = ol.behind_fill;
    let mut scale = ol.scale_with_object;
    let mut over = o.style.overprint_outline;
    row(ui, |ui| {
        if ui.checkbox(&mut behind, "Behind fill").changed() {
            edit_sel(app, "Behind Fill", |x| x.style.outline.behind_fill = behind);
        }
    });
    row(ui, |ui| {
        if ui.checkbox(&mut scale, "Scale with object").changed() {
            edit_sel(app, "Scale Outline", |x| x.style.outline.scale_with_object = scale);
        }
    });
    row(ui, |ui| {
        if ui.checkbox(&mut over, "Overprint outline").changed() {
            edit_sel(app, "Overprint Outline", |x| x.style.overprint_outline = over);
        }
    });
}

/// The miter limit as the angle under which corners are beveled (the reference shows
/// degrees): limit = 1 / sin(angle / 2).
fn miter_degrees(limit: f64) -> f64 {
    let l = limit.max(1.0);
    ((2.0 * (1.0 / l).asin()).to_degrees() * 10.0).round() / 10.0
}

fn miter_from_degrees(deg: f64) -> f64 {
    1.0 / (deg.clamp(1.0, 179.9).to_radians() / 2.0).sin()
}

/// The arrowhead list: a grid of the arrowheads (the first one: none).
fn arrow_combo(app: &mut VectorApp, ui: &mut Ui, cur: Option<ArrowHead>, start: bool, share: bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(64.0, 22.0), Sense::click());
    ui.painter().rect_filled(r, 2.0, Color32::from_rgb(0x33, 0x33, 0x33));
    let a = Rect::from_min_max(r.min + vec2(4.0, 0.0), r.max - vec2(18.0, 0.0));
    paint_arrow(ui.painter(), a, cur, start);
    crate::widgets::chevron(ui.painter(), pos2(r.right() - 9.0, r.center().y), 3.0, theme::TEXT);
    let tip = if start { "Start arrowhead\nAdd an arrowhead at the start of the line." } else { "End arrowhead\nAdd an arrowhead at the end of the line." };
    let resp = resp.on_hover_text(tip);
    let choices = crate::dialogs::object::arrow_choices(app);
    let mut pick: Option<Option<ArrowHead>> = None;
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        let all: Vec<(Option<ArrowHead>, String)> = std::iter::once((None, "No arrowhead".to_string())).chain(choices.iter().map(|c| (Some(c.0), c.1.clone()))).collect();
        egui::Grid::new(("pr_arrows", start)).spacing(vec2(2.0, 2.0)).show(ui, |ui| {
            for (i, (a, name)) in all.iter().enumerate() {
                let (rr, rs) = ui.allocate_exact_size(vec2(34.0, 24.0), Sense::click());
                if *a == cur || rs.hovered() {
                    ui.painter().rect_filled(rr, 2.0, if *a == cur { theme::ACCENT } else { theme::HOVER });
                }
                paint_arrow(ui.painter(), rr.shrink(4.0), *a, start);
                if rs.on_hover_text(name.as_str()).clicked() {
                    pick = Some(*a);
                    ui.close();
                }
                if i % 4 == 3 {
                    ui.end_row();
                }
            }
        });
    });
    if let Some(a) = pick {
        if let Some(a) = a {
            crate::dialogs::object::ensure_arrow(app, Some(a));
        }
        edit_sel(app, "Arrowhead", |x| {
            if start || share {
                x.style.outline.start_arrow = a;
            }
            if !start || share {
                x.style.outline.end_arrow = a;
            }
        });
    }
}

/// A short line with the arrowhead at its end (at its start for the start arrowhead).
fn paint_arrow(p: &egui::Painter, r: Rect, a: Option<ArrowHead>, start: bool) {
    let y = r.center().y;
    let c = theme::TEXT;
    let (tip, tail) = if start { (pos2(r.left() + 2.0, y), pos2(r.right() - 2.0, y)) } else { (pos2(r.right() - 2.0, y), pos2(r.left() + 2.0, y)) };
    match a {
        None => {
            p.line_segment([pos2(r.center().x - 8.0, y), pos2(r.center().x + 8.0, y)], Stroke::new(1.5, c));
        }
        Some(_) => {
            p.line_segment([tip, tail], Stroke::new(1.5, c));
            let d = if start { 1.0 } else { -1.0 };
            p.add(egui::Shape::convex_polygon(vec![tip, tip + vec2(8.0 * d, -4.0), tip + vec2(8.0 * d, 4.0)], c, Stroke::NONE));
        }
    }
}

/// The variable outline: its nodes along a bar, then the widths of the selected node.
fn variable_outline(app: &mut VectorApp, ui: &mut Ui, ol: &vector_core::style::Outline, st: &mut PropsUi) {
    let (nodes, scale) = match &ol.profile {
        Some(WidthProfile::Variable { nodes, scale }) => (nodes.clone(), *scale),
        _ => (vec![], false),
    };
    let on = !nodes.is_empty();
    row(ui, |ui| {
        let w = ui.available_width() - 20.0;
        let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
        let y = r.center().y;
        let col = Color32::from_gray(0x80);
        ui.painter().line_segment([pos2(r.left() + 4.0, y), pos2(r.right() - 4.0, y)], Stroke::new(3.0, col));
        for i in 0..5 {
            let x = r.left() + 4.0 + (r.width() - 8.0) * i as f32 / 4.0;
            ui.painter().line_segment([pos2(x, y - 7.0), pos2(x, y + 7.0)], Stroke::new(1.5, col));
        }
        for (i, n) in nodes.iter().enumerate() {
            let x = r.left() + 4.0 + (r.width() - 8.0) * n.t as f32;
            let sel = st.vo_node == Some(i);
            ui.painter().rect_filled(Rect::from_center_size(pos2(x, y), vec2(7.0, 11.0)), 1.0, if sel { theme::FOCUS } else { theme::TEXT });
        }
        if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.clicked()) {
            let t = ((p.x - r.left() - 4.0) / (r.width() - 8.0)).clamp(0.0, 1.0) as f64;
            st.vo_node = nodes.iter().enumerate().filter(|(_, n)| (n.t - t).abs() < 0.05).min_by(|a, b| (a.1.t - t).abs().total_cmp(&(b.1.t - t).abs())).map(|(i, _)| i);
        }
    });
    let sel = st.vo_node.filter(|i| *i < nodes.len());
    let cur = sel.map(|i| nodes[i].clone()).unwrap_or(VarWidth { t: 0.0, side1: 0.0, side2: 0.0 });
    let en = on && sel.is_some();
    let lw = 70.0;
    let set = |app: &mut VectorApp, f: &dyn Fn(&mut VarWidth)| {
        if let Some(i) = sel {
            edit_live(app, "Variable Outline", |x| {
                if let Some(WidthProfile::Variable { nodes, .. }) = &mut x.style.outline.profile {
                    if let Some(n) = nodes.get_mut(i) {
                        f(n);
                    }
                }
            });
        }
    };
    ui.add_enabled_ui(en, |ui| {
        ui.horizontal(|ui| {
            rlabel(ui, "Position:", lw, en);
            let mut t = cur.t * 100.0;
            if spin(ui, &mut t, 0.0..=100.0, 1.0, 1, " %", 80.0) {
                set(app, &|n| n.t = t / 100.0);
            }
        });
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    rlabel(ui, "Side 1:", lw, en);
                    let mut s1 = cur.side1;
                    if spin(ui, &mut s1, 0.0..=720.0, 0.5, 1, " pt", 80.0) {
                        set(app, &|n| n.side1 = s1);
                    }
                });
                ui.horizontal(|ui| {
                    rlabel(ui, "Side 2:", lw, en);
                    let mut s2 = cur.side2;
                    if spin(ui, &mut s2, 0.0..=720.0, 0.5, 1, " pt", 80.0) {
                        set(app, &|n| n.side2 = s2);
                    }
                });
            });
            super::icon_button(app, ui, "unlock", "Lock side widths", en);
        });
        ui.horizontal(|ui| {
            rlabel(ui, "Total:", lw, en);
            let mut t = cur.side1 + cur.side2;
            if spin(ui, &mut t, 0.0..=1440.0, 1.0, 1, " pt", 80.0) {
                let k = if cur.side1 + cur.side2 > 0.0 { t / (cur.side1 + cur.side2) } else { 1.0 };
                set(app, &|n| {
                    n.side1 *= k;
                    n.side2 *= k;
                });
            }
        });
    });
    ui.add_space(4.0);
    row(ui, |ui| {
        let mut s = scale;
        if ui.add_enabled(on, egui::Checkbox::new(&mut s, "Scale width with Stroke")).changed() {
            edit_sel(app, "Variable Outline", |x| {
                if let Some(WidthProfile::Variable { scale, .. }) = &mut x.style.outline.profile {
                    *scale = s;
                }
            });
        }
    });
}

/// A color list whose popup shows the color editor with its viewer tabs.
fn color_pick_tabs(ui: &mut Ui, id: &str, c: &mut Option<Color>, w: f32) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(r, 2.0, Color32::from_rgb(0x33, 0x33, 0x33));
    let sw = Rect::from_min_size(r.min + vec2(4.0, 5.0), vec2(w - 24.0, 12.0));
    crate::widgets::paint_well(p, sw, *c);
    p.rect_stroke(sw, 0.0, Stroke::new(1.0, Color32::from_gray(140)), egui::StrokeKind::Outside);
    crate::widgets::chevron(p, pos2(r.right() - 9.0, r.center().y), 3.0, theme::TEXT);
    let mut changed = false;
    let open = egui::Popup::menu(&resp).id(egui::Id::unique(("pr_color", id))).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        changed = coled::picker_ex(ui, id, c, true, true);
    });
    if open.is_none() {
        coled::picker_closed(ui.ctx(), id);
    }
    changed
}

// ------------------------------------------------------------------------------ fill

const FILL_TYPES: [(&str, &str); 6] = [
    ("no_fill", "No fill"),
    ("uniform_fill", "Uniform fill"),
    ("fountain_fill", "Fountain fill"),
    ("vector_pattern", "Vector pattern fill"),
    ("bitmap_fill", "Bitmap pattern fill"),
    ("two_color_fill", "More\nDisplay other fill types."),
];

fn fill(app: &mut VectorApp, ui: &mut Ui, o: &Object, st: &mut PropsUi) {
    let f = o.style.fill.clone();
    let kind = fill_kind(&f);
    title(ui, "Fill");
    ui.add_space(4.0);
    let base = f.main_color().unwrap_or(app.default_fill);
    row(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        for (i, (icon, tip)) in FILL_TYPES.iter().enumerate() {
            let r = crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, kind == i as u8);
            if i == 5 {
                drop_mark(ui, r.rect);
                egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                    ui.set_min_width(170.0);
                    if menu_icon_item(app, ui, "two_color_fill", "Two-color Pattern Fill", true) {
                        let nf = new_fill(app, 5, base);
                        edit_sel(app, "Fill", move |x| x.style.fill = nf.clone());
                        ui.close();
                    }
                    menu_icon_item(app, ui, "texture_fill", "Texture Fill", false);
                    menu_icon_item(app, ui, "postscript_fill", "PostScript Fill", false);
                });
            } else if r.clicked() && kind != i as u8 {
                let nf = new_fill(app, i as u8, base);
                edit_sel(app, "Fill", move |x| x.style.fill = nf.clone());
            }
        }
    });
    gap(ui);
    match f {
        Fill::None => {
            ui.label(egui::RichText::new("The object has no fill.").color(theme::TEXT));
        }
        Fill::Solid(c) => {
            let mut c = c;
            let mut ed = st.fill_ed;
            let mut changed = coled::editor(ui, &mut ed, &mut c, true);
            st.fill_ed = ed;
            ui.add_space(6.0);
            let (ch, act) = coled::bar(ui, "pr_fill", &mut c, None, true);
            changed |= ch;
            if act == coled::BarAction::NoColor {
                edit_sel(app, "Fill", |x| x.style.fill = Fill::None);
                return;
            }
            if changed {
                edit_live(app, "Fill Color", |x| x.style.fill = Fill::Solid(c));
            }
            ui.add_space(2.0);
            fold(ui, &mut st.open[1]);
            if !st.open[1] {
                return;
            }
            winding(app, ui, o, true);
            gap(ui);
            for (k, v) in coled::value_lines(c) {
                ui.horizontal(|ui| {
                    rlabel(ui, k, 60.0, true);
                    ui.label(egui::RichText::new(v).color(theme::TEXT));
                });
            }
        }
        Fill::Gradient(g) => fountain(app, ui, o, g, st, false),
        Fill::Pattern(p) => pattern(app, ui, o, p, kind, false, 0.0, 0.0),
        Fill::Mesh(_) => {
            subtitle(app, ui, "Mesh Fill", false);
            ui.add_space(4.0);
            if ui.button("Mesh Fill tool").clicked() {
                app.set_tool(crate::tools::ToolId::MeshFill);
            }
        }
    }
}

fn menu_icon_item(app: &mut VectorApp, ui: &mut Ui, icon: &str, text: &str, enabled: bool) -> bool {
    let tex = app.icons.get(ui.ctx(), icon, 16.0, if enabled { crate::icons::Tone::Normal } else { crate::icons::Tone::Disabled });
    let b = match tex {
        Some(t) => egui::Button::image_and_text(egui::Image::new(t).fit_to_exact_size(vec2(16.0, 16.0)), text),
        None => egui::Button::new(text),
    };
    ui.add_enabled(enabled, b).clicked()
}

fn winding(app: &mut VectorApp, ui: &mut Ui, o: &Object, overprint: bool) {
    let mut eo = !o.style.even_odd;
    row(ui, |ui| {
        if ui.checkbox(&mut eo, "Fill winding").changed() {
            edit_sel(app, "Fill Winding", |x| x.style.even_odd = !eo);
        }
    });
    if overprint {
        let mut op = o.style.overprint_fill;
        row(ui, |ui| {
            if ui.checkbox(&mut op, "Overprint fill").changed() {
                edit_sel(app, "Overprint Fill", |x| x.style.overprint_fill = op);
            }
        });
    }
}

/// Built-in fountain fills of the fill picker (grayscale ones for transparencies).
fn gradient_presets(gray: bool) -> Vec<Gradient> {
    let c = Color::rgb;
    if gray {
        return vec![
            Gradient::linear(Color::WHITE, Color::BLACK),
            Gradient::linear(Color::BLACK, Color::WHITE),
            Gradient::radial(Color::WHITE, Color::BLACK),
            Gradient { kind: GradientKind::Conical, ..Gradient::radial(Color::WHITE, Color::BLACK) },
            Gradient { kind: GradientKind::Rectangular, ..Gradient::radial(Color::WHITE, Color::BLACK) },
        ];
    }
    vec![
        Gradient::linear(c(70, 130, 200), Color::WHITE),
        Gradient::linear(Color::BLACK, Color::WHITE),
        Gradient::radial(Color::WHITE, c(70, 130, 200)),
        Gradient { stops: vec![GradientStop { pos: 0.0, color: c(255, 0, 0) }, GradientStop { pos: 0.5, color: c(255, 255, 0) }, GradientStop { pos: 1.0, color: c(0, 160, 255) }], ..Gradient::linear(Color::BLACK, Color::WHITE) },
        Gradient { kind: GradientKind::Conical, ..Gradient::radial(c(255, 200, 0), c(200, 40, 40)) },
        Gradient { kind: GradientKind::Rectangular, ..Gradient::radial(Color::WHITE, c(40, 40, 40)) },
    ]
}

/// The fountain fill (or transparency, `transp`): the picker, the nodes, the type, blend,
/// arrangement, acceleration, steps and the transformations.
fn fountain(app: &mut VectorApp, ui: &mut Ui, o: &Object, g: Gradient, st: &mut PropsUi, transp: bool) {
    let mut g = g;
    let mut ch = false;
    let k = if transp { 1 } else { 0 };
    subtitle(app, ui, if transp { "Fountain Transparency" } else { "Fountain Fill" }, true);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let r = picker_box(ui, 52.0, 52.0, |p, r| paint_gradient(p, r, &g));
        egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            egui::Grid::new("pr_grad_presets").spacing(vec2(4.0, 4.0)).show(ui, |ui| {
                for (i, pg) in gradient_presets(transp).into_iter().enumerate() {
                    let (rr, rs) = ui.allocate_exact_size(vec2(48.0, 48.0), Sense::click());
                    paint_gradient(ui.painter(), rr, &pg);
                    if rs.clicked() {
                        g = pg;
                        ch = true;
                        ui.close();
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        });
        ui.add_space(8.0);
        super::icon_button(app, ui, if transp { "edit_transparency" } else { "edit_fill" }, if transp { "Edit transparency" } else { "Edit fill" }, false);
        mark(ui);
    });
    // the nodes
    ui.add_space(6.0);
    let nsel = &mut st.node[k];
    *nsel = (*nsel).min(g.stops.len().saturating_sub(1));
    row(ui, |ui| ch |= node_bar(ui, &mut g, nsel, transp));
    let sel = *nsel;
    let (lo, hi) = g.stops.iter().fold((f64::MAX, f64::MIN), |a, s| (a.0.min(s.pos), a.1.max(s.pos)));
    let end = g.stops.get(sel).map(|s| s.pos == lo || s.pos == hi).unwrap_or(true);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if let Some(s) = g.stops.get_mut(sel) {
            if !transp {
                ui.vertical(|ui| {
                    label(ui, "Color:", true);
                    let mut c = Some(s.color);
                    if color_pick_tabs(ui, "pr_node_color", &mut c, 44.0) {
                        if let Some(c) = c {
                            s.color = c.with_alpha(s.color.a);
                            ch = true;
                        }
                    }
                });
            }
            ui.vertical(|ui| {
                label(ui, "Position:", !end);
                ui.add_enabled_ui(!end, |ui| {
                    let mut p = (s.pos * 1000.0).round() / 10.0;
                    if plus_field_d(ui, &mut p, 0.0..=100.0, "", 64.0, 1) {
                        s.pos = p / 100.0;
                        ch = true;
                    }
                });
            });
            if transp {
                ui.vertical(|ui| {
                    label(ui, "Transparency:", true);
                    let mut t = (100.0 - s.color.luminance() / 255.0 * 100.0).round();
                    if plus_field(ui, &mut t, 0.0..=100.0, " %", 64.0) {
                        let v = (255.0 * (1.0 - t / 100.0)).round() as u8;
                        s.color = Color::rgb(v, v, v);
                        ch = true;
                    }
                });
            } else {
                ui.vertical(|ui| {
                    label(ui, "Transparency:", true);
                    let mut t = (100.0 - s.color.a as f64 / 255.0 * 100.0).round();
                    if plus_field(ui, &mut t, 0.0..=100.0, "", 64.0) {
                        s.color = s.color.with_alpha((255.0 * (1.0 - t / 100.0)).round() as u8);
                        ch = true;
                    }
                });
            }
        }
        mark(ui);
    });
    ui.add_space(6.0);
    let fk = if transp { 2 } else { 1 };
    fold(ui, &mut st.open[fk]);
    if st.open[fk] {
        ui.add_space(2.0);
        if transp {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    label(ui, "Target:", true);
                    ui.horizontal(|ui| target_buttons(app, ui, o));
                });
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    label(ui, "Type:", true);
                    ui.horizontal(|ui| ch |= gradient_types(app, ui, &mut g));
                });
                mark(ui);
            });
        } else {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    label(ui, "Type:", true);
                    ui.horizontal(|ui| ch |= gradient_types(app, ui, &mut g));
                });
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    label(ui, "Blend direction:", true);
                    ui.horizontal(|ui| {
                        for (i, (icon, tip)) in [("blend_direct", "Linear color blend"), ("blend_cw", "Clockwise color blend"), ("blend_ccw", "Counterclockwise color blend")].iter().enumerate() {
                            if tbtn(app, ui, icon, tip, true, g.blend == i as u8) {
                                g.blend = i as u8;
                                ch = true;
                            }
                        }
                    });
                });
                mark(ui);
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    label(ui, "Arrangement:", true);
                    ui.horizontal(|ui| {
                        for (icon, tip, m) in [("fountain_fill", "Default fountain fill", SpreadMode::Pad), ("fountain_mirror", "Repeat and mirror", SpreadMode::Reflect), ("fountain_repeat", "Repeat", SpreadMode::Repeat)] {
                            if tbtn(app, ui, icon, tip, true, g.spread == m) {
                                g.spread = m;
                                ch = true;
                            }
                        }
                    });
                });
                ui.add_space(30.0);
                ui.vertical(|ui| {
                    label(ui, "Reverse fill:", true);
                    if tbtn(app, ui, "reverse_fill", "Reverse fill", true, false) {
                        for s in g.stops.iter_mut() {
                            s.pos = 1.0 - s.pos;
                        }
                        ch = true;
                    }
                });
                mark(ui);
            });
            gap(ui);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    label(ui, "Acceleration:", true);
                    let mut a = g.accel;
                    if plus_field(ui, &mut a, -100.0..=100.0, "", 70.0) {
                        g.accel = a;
                        ch = true;
                    }
                });
                ui.add_space(20.0);
                ui.vertical(|ui| {
                    let mut on = g.steps > 0;
                    if ui.checkbox(&mut on, "Steps:").changed() {
                        g.steps = if on { 256 } else { 0 };
                        ch = true;
                    }
                    ui.add_enabled_ui(on, |ui| {
                        let mut s = if on { g.steps as f64 } else { 256.0 };
                        if spin(ui, &mut s, 2.0..=999.0, 1.0, 0, "", 70.0) {
                            g.steps = s as u32;
                            ch = true;
                        }
                    });
                });
                mark(ui);
            });
            row(ui, |ui| {
                let mut s = g.smooth;
                if ui.checkbox(&mut s, "Smooth transition").changed() {
                    g.smooth = s;
                    ch = true;
                }
            });
        }
        if transp {
            row(ui, |ui| {
                let mut f = g.free_scale;
                if ui.checkbox(&mut f, "Free scale and skew").changed() {
                    g.free_scale = f;
                    ch = true;
                }
            });
        } else {
            gap(ui);
            ch |= gradient_transform(app, ui, &mut g);
            gap(ui);
            winding(app, ui, o, true);
        }
    }
    if ch {
        if transp {
            edit_live(app, "Transparency", |x| x.style.transparency = Transparency::Gradient(g.clone()));
        } else {
            edit_live(app, "Fountain Fill", |x| x.style.fill = Fill::Gradient(g.clone()));
        }
    }
}

fn gradient_types(app: &mut VectorApp, ui: &mut Ui, g: &mut Gradient) -> bool {
    let mut ch = false;
    for (icon, tip, k) in [("fountain_fill", "Linear fountain fill", GradientKind::Linear), ("fountain_elliptical", "Elliptical fountain fill", GradientKind::Elliptical), ("fountain_conical", "Conical fountain fill", GradientKind::Conical), ("fountain_rectangular", "Rectangular fountain fill", GradientKind::Rectangular)] {
        if tbtn(app, ui, icon, tip, true, g.kind == k) && g.kind != k {
            let c = pos_mid(g);
            let was_linear = g.kind == GradientKind::Linear;
            g.kind = k;
            if was_linear != (k == GradientKind::Linear) {
                let v = g.end - g.start;
                let len = (v.x * v.x + v.y * v.y).sqrt().max(0.05);
                if k == GradientKind::Linear {
                    g.start = kurbo::Point::new(c.x - len, c.y);
                    g.end = kurbo::Point::new(c.x + len, c.y);
                } else {
                    g.start = c;
                    g.end = kurbo::Point::new(c.x + len / 2.0, c.y);
                }
            }
            ch = true;
        }
    }
    ch
}

fn pos_mid(g: &Gradient) -> kurbo::Point {
    match g.kind {
        GradientKind::Linear => kurbo::Point::new((g.start.x + g.end.x) / 2.0, (g.start.y + g.end.y) / 2.0),
        _ => g.start,
    }
}

/// The color band with the nodes under it (click a node to select it, drag to move it,
/// double-click the band to add a node, double-click a node to delete it).
fn node_bar(ui: &mut Ui, g: &mut Gradient, sel: &mut usize, transp: bool) -> bool {
    let mut ch = false;
    let w = ui.available_width() - 26.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
    let band = Rect::from_min_max(pos2(r.left() + 5.0, r.top() + 6.0), pos2(r.right() - 5.0, r.top() + 20.0));
    paint_gradient(ui.painter(), band, g);
    ui.painter().rect_stroke(band, 0.0, Stroke::new(1.0, Color32::from_gray(150)), egui::StrokeKind::Outside);
    // the midpoint mark of the first two nodes
    let stops = g.sorted_stops();
    if stops.len() >= 2 {
        let mid = (stops[0].pos + stops[1].pos) / 2.0;
        let x = band.left() + band.width() * mid as f32;
        ui.painter().add(egui::Shape::convex_polygon(vec![pos2(x - 4.0, band.top() - 5.0), pos2(x + 4.0, band.top() - 5.0), pos2(x, band.top())], theme::TEXT, Stroke::NONE));
    }
    let n = g.stops.len();
    let mut remove = None;
    for i in 0..n {
        let x = band.left() + band.width() * g.stops[i].pos as f32;
        let nr = Rect::from_center_size(pos2(x, band.bottom() + 5.0), vec2(9.0, 9.0));
        let nresp = ui.interact(nr, ui.scope_id().with(("pr_node", transp, i)), Sense::click_and_drag());
        ui.painter().rect_filled(nr, 1.0, theme::to_c32(g.stops[i].color.with_alpha(255)));
        let on = *sel == i;
        ui.painter().rect_stroke(nr, 1.0, Stroke::new(if on { 2.0 } else { 1.0 }, if on { theme::FOCUS } else { Color32::from_gray(160) }), egui::StrokeKind::Outside);
        if nresp.clicked() || nresp.drag_started() {
            *sel = i;
        }
        if nresp.dragged() {
            if let Some(p) = nresp.interact_pointer_pos() {
                g.stops[i].pos = (((p.x - band.left()) / band.width()) as f64).clamp(0.0, 1.0);
                ch = true;
            }
        }
        if nresp.double_clicked() && n > 2 {
            remove = Some(i);
        }
    }
    if let Some(i) = remove {
        g.stops.remove(i);
        *sel = 0;
        ch = true;
    }
    if resp.double_clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let t = (((p.x - band.left()) / band.width()) as f64).clamp(0.0, 1.0);
            let c = g.color_at(t);
            g.stops.push(GradientStop { pos: t, color: c });
            *sel = g.stops.len() - 1;
            ch = true;
        }
    }
    ch
}

/// Fill width and height (percent of the object), the center offset, rotation and Free scale
/// and skew.
fn gradient_transform(app: &mut VectorApp, ui: &mut Ui, g: &mut Gradient) -> bool {
    let mut ch = false;
    subtitle(app, ui, "Transformations", false);
    ui.add_space(4.0);
    let c = pos_mid(g);
    let v = g.end - g.start;
    let len = (v.x * v.x + v.y * v.y).sqrt();
    let size = if g.kind == GradientKind::Linear { len } else { len * 2.0 };
    let ang = (-v.y).atan2(v.x).to_degrees();
    let rebuild = |g: &mut Gradient, c: kurbo::Point, size: f64, ang: f64| {
        let (s, co) = ang.to_radians().sin_cos();
        if g.kind == GradientKind::Linear {
            let h = size / 2.0;
            g.start = kurbo::Point::new(c.x - co * h, c.y + s * h);
            g.end = kurbo::Point::new(c.x + co * h, c.y - s * h);
        } else {
            g.start = c;
            g.end = kurbo::Point::new(c.x + co * size / 2.0, c.y - s * size / 2.0);
        }
    };
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            for lab in ["W:", "H:"] {
                ui.horizontal(|ui| {
                    rlabel(ui, lab, 22.0, true);
                    let mut w = (size * 1000.0).round() / 10.0;
                    if spin(ui, &mut w, 1.0..=10000.0, 1.0, 1, " %", 76.0) {
                        rebuild(g, c, w / 100.0, ang);
                        ch = true;
                    }
                });
            }
        });
        super::icon_button(app, ui, "lock", "Lock ratio", false);
        ui.vertical(|ui| {
            for (lab, axis) in [("X:", 0), ("Y:", 1)] {
                ui.horizontal(|ui| {
                    rlabel(ui, lab, 18.0, true);
                    // the offset of the center from the object's center, y up
                    let mut v = ((if axis == 0 { c.x - 0.5 } else { 0.5 - c.y }) * 1000.0).round() / 10.0;
                    if spin(ui, &mut v, -1000.0..=1000.0, 1.0, 1, " %", 76.0) {
                        let nc = if axis == 0 { kurbo::Point::new(v / 100.0 + 0.5, c.y) } else { kurbo::Point::new(c.x, 0.5 - v / 100.0) };
                        rebuild(g, nc, size, ang);
                        ch = true;
                    }
                });
            }
        });
        mark(ui);
    });
    ui.horizontal(|ui| {
        super::icon_button(app, ui, "skew", "Skew", false);
        ui.add_enabled_ui(false, |ui| {
            let mut s = 0.0;
            spin(ui, &mut s, -89.0..=89.0, 1.0, 1, " °", 76.0);
        });
        ui.add_space(8.0);
        super::icon_button(app, ui, "rotate", "Rotate", false);
        let mut a = (ang * 10.0).round() / 10.0;
        if spin(ui, &mut a, -360.0..=360.0, 1.0, 1, " °", 76.0) {
            rebuild(g, c, size, a);
            ch = true;
        }
        mark(ui);
    });
    row(ui, |ui| {
        let mut f = g.free_scale;
        if ui.checkbox(&mut f, "Free scale and skew").changed() {
            g.free_scale = f;
            ch = true;
        }
    });
    ch
}

/// The vector, bitmap and two-color patterns (fills, or transparencies with their foreground
/// and background transparency).
#[allow(clippy::too_many_arguments)]
fn pattern(app: &mut VectorApp, ui: &mut Ui, o: &Object, p: PatternFill, kind: u8, transp: bool, front: f64, back: f64) {
    let mut p = p;
    let (mut front, mut back) = (front, back);
    let mut ch = false;
    let head = match (kind, transp) {
        (3, false) => "Vector Pattern",
        (4, false) => "Bitmap Pattern",
        (_, false) => "Two-color Pattern",
        (3, true) => "Vector Pattern Transparency",
        (4, true) => "Bitmap Pattern Transparency",
        _ => "Two-color Pattern Transparency",
    };
    subtitle(app, ui, head, kind != 5);
    ui.add_space(4.0);
    let named = app.doc().map(|d| d.doc.patterns.clone()).unwrap_or_default();
    ui.horizontal(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(70.0, 52.0), Sense::click());
        ui.painter().rect_filled(r, 2.0, Color32::from_rgb(0x33, 0x33, 0x33));
        paint_pattern(app, ui.painter(), Rect::from_min_size(r.min, vec2(52.0, 52.0)), &p);
        crate::widgets::chevron(ui.painter(), pos2(r.right() - 9.0, r.center().y), 3.0, theme::TEXT);
        egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            let mut list: Vec<PatternFill> = vec![];
            match kind {
                5 => {
                    for k in PatternKind::ALL {
                        list.push(PatternFill { kind: k, image: None, vector: false, ..p.clone() });
                    }
                }
                3 => {
                    list.extend(named.iter().filter(|n| n.fill.vector).map(|n| n.fill.clone()));
                    for k in [PatternKind::Diamonds, PatternKind::Bricks, PatternKind::Waves, PatternKind::Grid] {
                        list.push(PatternFill { kind: k, image: None, vector: true, ..p.clone() });
                    }
                }
                _ => list.extend(named.iter().filter(|n| !n.fill.vector && n.fill.image.is_some()).map(|n| n.fill.clone())),
            }
            if list.is_empty() {
                list.push(p.clone());
            }
            egui::Grid::new("pr_patterns").spacing(vec2(4.0, 4.0)).show(ui, |ui| {
                for (i, pf) in list.into_iter().enumerate() {
                    let (rr, rs) = ui.allocate_exact_size(vec2(48.0, 48.0), Sense::click());
                    paint_pattern(app, ui.painter(), rr, &pf);
                    if rs.clicked() {
                        p = PatternFill { tile: p.tile, angle: p.angle, transform_with_object: p.transform_with_object, mirror: p.mirror, ..pf };
                        ch = true;
                        ui.close();
                    }
                    if i % 4 == 3 {
                        ui.end_row();
                    }
                }
            });
        });
        dot(ui);
        if kind != 5 {
            let (tr, _) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::hover());
            paint_pattern(app, ui.painter(), tr, &p);
            ui.painter().rect_stroke(tr, 0.0, Stroke::new(1.0, Color32::from_gray(0x60)), egui::StrokeKind::Outside);
            dot(ui);
        }
        let (sr, _) = ui.allocate_exact_size(vec2(1.0, 52.0), Sense::hover());
        ui.painter().line_segment([sr.center_top(), sr.center_bottom()], Stroke::new(1.0, theme::SEP));
        ui.vertical(|ui| {
            super::icon_button(app, ui, if transp { "edit_transparency" } else { "edit_fill" }, if transp { "Edit transparency" } else { "Edit fill" }, false);
            if kind != 5 {
                let r = super::icon_button_resp(app, ui, "new_source", "New source", true);
                egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                    ui.set_min_width(190.0);
                    if ui.button("New source from document").clicked() {
                        app.run(crate::commands::Cmd::CreatePattern);
                        ui.close();
                    }
                    if ui.button("New source from file...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "webp"]).pick_file() {
                            if let Some(id) = load_pattern_image(app, &path) {
                                p.image = Some(id);
                                p.vector = false;
                                ch = true;
                            }
                        }
                        ui.close();
                    }
                });
            }
        });
        mark(ui);
    });
    if transp {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    rlabel(ui, "Foreground:", 80.0, true);
                    ch |= plus_field(ui, &mut front, 0.0..=100.0, "", 64.0);
                });
                ui.horizontal(|ui| {
                    rlabel(ui, "Background:", 80.0, true);
                    ch |= plus_field(ui, &mut back, 0.0..=100.0, "", 64.0);
                });
            });
            if super::icon_button(app, ui, "reverse_fill", "Swap", true) {
                std::mem::swap(&mut front, &mut back);
                ch = true;
            }
            mark(ui);
        });
        ui.add_space(4.0);
        label(ui, if kind == 3 { "Transparency target:" } else { "Target:" }, true);
        row(ui, |ui| target_buttons(app, ui, o));
        row(ui, |ui| {
            let mut t = p.transform_with_object;
            if ui.checkbox(&mut t, "Free scale and skew").changed() {
                p.transform_with_object = t;
                ch = true;
            }
        });
    } else {
        if kind == 5 {
            ui.add_space(6.0);
            for (lab, fg) in [("Front color:", true), ("Back color:", false)] {
                row(ui, |ui| {
                    rlabel(ui, lab, 86.0, true);
                    let mut c = Some(if fg { p.fg } else { p.bg });
                    if color_pick_w(ui, &format!("pr_two_{fg}"), &mut c, false, true, 64.0) {
                        if let Some(c) = c {
                            if fg {
                                p.fg = c;
                            } else {
                                p.bg = c;
                            }
                            ch = true;
                        }
                    }
                });
            }
        }
        ui.add_space(4.0);
        row(ui, |ui| {
            rlabel(ui, "Mirror tiles:", 86.0, true);
            for (bit, icon, tip) in [(1u8, "mirror_h", "Mirror tiles horizontally"), (2u8, "mirror_v", "Mirror tiles vertically")] {
                if tbtn(app, ui, icon, tip, true, p.mirror & bit != 0) {
                    p.mirror ^= bit;
                    ch = true;
                }
            }
        });
        if kind == 4 {
            gap(ui);
            label(ui, "Blend type:", true);
            row(ui, |ui| {
                tbtn(app, ui, "fountain_elliptical", "Radial blend", false, false);
                tbtn(app, ui, "fountain_fill", "Linear blend", false, false);
                ui.add_enabled_ui(false, |ui| {
                    let mut v = 50.0;
                    slider(ui, &mut v, 0.0..=100.0, 110.0);
                    ui.label("50 %");
                });
            });
            gap(ui);
            for (lab, v) in [("Edge match", 50.0), ("Brightness", 0.0), ("Luminance", 0.0), ("Color", 0.0)] {
                row(ui, |ui| {
                    let mut on = false;
                    ui.add_enabled(false, egui::Checkbox::new(&mut on, ""));
                    ui.add_sized(vec2(90.0, 20.0), egui::Label::new(egui::RichText::new(lab).color(theme::TEXT)));
                    ui.add_enabled_ui(false, |ui| {
                        let mut v = v;
                        plus_field(ui, &mut v, -100.0..=100.0, "", 56.0);
                    });
                });
            }
        }
        gap(ui);
        subtitle(app, ui, "Transformations", false);
        ui.add_space(4.0);
        let dpi = app.doc().map(|d| d.doc.settings.dpi).unwrap_or(300.0);
        let unit = app.doc().map(|d| d.doc.settings.unit).unwrap_or(Unit::Inches);
        let sfx = format!(" {}", unit.suffix());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                for lab in ["W:", "H:"] {
                    ui.horizontal(|ui| {
                        rlabel(ui, lab, 22.0, true);
                        let mut v = unit.to_unit(p.tile, dpi);
                        if spin(ui, &mut v, 0.001..=10000.0, 0.1, 1, &sfx, 82.0) {
                            p.tile = unit.from_unit(v, dpi).max(1.0);
                            ch = true;
                        }
                    });
                }
            });
            super::icon_button(app, ui, "lock", "Lock ratio", false);
            ui.vertical(|ui| {
                for lab in ["X:", "Y:"] {
                    ui.horizontal(|ui| {
                        rlabel(ui, lab, 18.0, true);
                        ui.add_enabled_ui(false, |ui| {
                            let mut v = 0.0;
                            spin(ui, &mut v, -1000.0..=1000.0, 0.1, 1, &sfx, 82.0);
                        });
                    });
                }
            });
            mark(ui);
        });
        ui.horizontal(|ui| {
            super::icon_button(app, ui, "skew", "Skew", false);
            ui.add_enabled_ui(false, |ui| {
                let mut s = 0.0;
                spin(ui, &mut s, -89.0..=89.0, 1.0, 1, " °", 76.0);
            });
            ui.add_space(8.0);
            super::icon_button(app, ui, "rotate", "Rotate", false);
            let mut a = p.angle;
            if spin(ui, &mut a, -360.0..=360.0, 1.0, 1, " °", 76.0) {
                p.angle = a;
                ch = true;
            }
            mark(ui);
        });
        ui.add_space(4.0);
        row(ui, |ui| {
            label(ui, "Offset:", true);
            tbtn(app, ui, "offset_row", "Row offset", false, false);
            tbtn(app, ui, "offset_col", "Column offset", false, false);
            ui.add_enabled_ui(false, |ui| {
                let mut v = 0.0;
                spin(ui, &mut v, 0.0..=100.0, 1.0, 0, " %", 70.0);
            });
        });
        gap(ui);
        winding(app, ui, o, false);
        row(ui, |ui| {
            let mut t = p.transform_with_object;
            if ui.checkbox(&mut t, "Transform with object").changed() {
                p.transform_with_object = t;
                ch = true;
            }
        });
    }
    if ch {
        if transp {
            edit_live(app, "Transparency", |x| x.style.transparency = Transparency::Pattern { pattern: p.clone(), front, back });
        } else {
            edit_live(app, "Pattern Fill", |x| x.style.fill = Fill::Pattern(p.clone()));
        }
    }
}

/// An image file as a bitmap pattern of the document.
fn load_pattern_image(app: &mut VectorApp, path: &std::path::Path) -> Option<u64> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "png".into());
    let d = app.doc_mut()?;
    let id = d.doc.add_image(vector_core::document::ImageData { width: w, height: h, rgba: std::sync::Arc::new(img.into_raw()), encoded: std::sync::Arc::new(bytes), format: ext, name: name.clone() });
    let fill = PatternFill { kind: PatternKind::Checker, fg: Color::BLACK, bg: Color::WHITE, tile: 72.0, angle: 0.0, image: Some(id), transform_with_object: true, vector: false, mirror: 0 };
    d.doc.patterns.push(vector_core::document::NamedPattern { name, fill });
    Some(id)
}

// ------------------------------------------------------------------------------ transparency

const TRANSPARENCY_TYPES: [(&str, &str); 6] = [
    ("no_transparency", "No transparency"),
    ("uniform_transparency", "Uniform transparency"),
    ("fountain_transparency", "Fountain transparency"),
    ("vector_pattern_transparency", "Vector pattern transparency"),
    ("bitmap_transparency", "Bitmap pattern transparency"),
    ("two_color_transparency", "More\nDisplay other transparency types."),
];

fn target_buttons(app: &mut VectorApp, ui: &mut Ui, o: &Object) {
    for (i, (icon, tip)) in [("apply_all", "All"), ("apply_fill", "Fill"), ("apply_outline", "Outline")].iter().enumerate() {
        if tbtn(app, ui, icon, tip, true, o.style.transparency_target == i as u8) {
            edit_sel(app, "Transparency Target", |x| x.style.transparency_target = i as u8);
        }
        if i == 0 {
            ui.add_space(6.0);
        }
    }
}

fn transparency(app: &mut VectorApp, ui: &mut Ui, o: &Object, st: &mut PropsUi) {
    let t = o.style.transparency.clone();
    let kind = transparency_kind(&t);
    title(ui, "Transparency");
    ui.add_space(4.0);
    row(ui, |ui| {
        let mut m = MergeMode::ALL.iter().position(|x| *x == o.style.merge).unwrap_or(0);
        let names: Vec<&str> = MergeMode::ALL.iter().map(|m| m.name()).collect();
        let w = ui.available_width() - 20.0;
        if super::choice(ui, "pr_merge", &mut m, &names, w) {
            let mm = MergeMode::ALL[m];
            edit_sel(app, "Merge Mode", |x| x.style.merge = mm);
        }
    });
    ui.add_space(4.0);
    row(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        for (i, (icon, tip)) in TRANSPARENCY_TYPES.iter().enumerate() {
            let r = crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, kind == i as u8);
            if i == 5 {
                drop_mark(ui, r.rect);
                egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                    ui.set_min_width(200.0);
                    if menu_icon_item(app, ui, "two_color_transparency", "Two-color Pattern Transparency", true) {
                        let nt = new_transparency(app, 5);
                        edit_sel(app, "Transparency", move |x| x.style.transparency = nt.clone());
                        ui.close();
                    }
                    menu_icon_item(app, ui, "texture_transparency", "Texture Transparency", false);
                });
            } else if r.clicked() && kind != i as u8 {
                let nt = new_transparency(app, i as u8);
                edit_sel(app, "Transparency", move |x| x.style.transparency = nt.clone());
            }
        }
    });
    gap(ui);
    match t {
        Transparency::None => {}
        Transparency::Uniform(v) => {
            subtitle(app, ui, "Uniform Transparency", false);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let r = picker_box(ui, 62.0, 40.0, |p, r| {
                    crate::widgets::checker(p, r, 5.0);
                    p.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(0x80, 0x80, 0x80, (255.0 * (1.0 - v / 100.0)) as u8));
                });
                let mut pick = None;
                egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                    for p in [0.0, 25.0, 50.0, 75.0, 100.0] {
                        if ui.selectable_label((v - p).abs() < 0.5, format!("{p:.0}")).clicked() {
                            pick = Some(p);
                            ui.close();
                        }
                    }
                });
                if let Some(p) = pick {
                    edit_sel(app, "Transparency", |x| x.style.transparency = Transparency::Uniform(p));
                }
                ui.add_space(8.0);
                super::icon_button(app, ui, "edit_transparency", "Edit transparency", false);
                mark(ui);
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    label(ui, "Transparency:", true);
                    let mut tv = v;
                    if plus_field(ui, &mut tv, 0.0..=100.0, "", 76.0) {
                        edit_live(app, "Transparency", |x| x.style.transparency = Transparency::Uniform(tv));
                    }
                });
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    label(ui, "Transparency target:", true);
                    ui.horizontal(|ui| target_buttons(app, ui, o));
                });
                mark(ui);
            });
        }
        Transparency::Gradient(g) => fountain(app, ui, o, g, st, true),
        Transparency::Pattern { pattern: p, front, back } => {
            let k = pattern_kind(&p);
            pattern(app, ui, o, p, k, true, front, back);
        }
    }
}

// ------------------------------------------------------------------------------ effects

/// The bitmap effect categories of the Add effect menu (those of the Effects menu).
const FX_CATEGORIES: [&str; 14] = ["Adjust", "Art Strokes", "Blur", "Camera", "Color Transform", "Contour", "Correction", "Creative", "Custom", "Distort", "Noise", "Sharpen", "Texture", "Transform"];

fn effects(app: &mut VectorApp, ui: &mut Ui, o: &Object, st: &mut PropsUi) {
    super::fxpage::take_pick(app, o, st);
    title(ui, "Bitmap Effects");
    ui.add_space(4.0);
    // presets
    let w = ui.available_width() - 20.0;
    row(ui, |ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
        ui.painter().rect_filled(r, 2.0, Color32::from_rgb(0x33, 0x33, 0x33));
        ui.painter().text(pos2(r.left() + 6.0, r.center().y), egui::Align2::LEFT_CENTER, "Presets", egui::FontId::proportional(12.5), theme::TEXT);
        crate::widgets::chevron(ui.painter(), pos2(r.right() - 9.0, r.center().y), 3.0, theme::TEXT);
        egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.set_width(w - 12.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut st.fx_search).desired_width(w - 50.0).hint_text("Search"));
                super::icon_button(app, ui, "gear", "Options", false);
            });
            let mut a = 0usize;
            super::choice(ui, "pr_fx_presets", &mut a, &["All presets"], w - 20.0);
            let mut b = 0usize;
            super::choice(ui, "pr_fx_categories", &mut b, &["All categories"], w - 20.0);
            ui.add_space(4.0);
            egui::Grid::new("pr_fx_tiles").spacing(vec2(16.0, 6.0)).show(ui, |ui| {
                let mut k = 0;
                for (name, c) in [("Bright Midtones", Color32::from_rgb(0x5c, 0x9c, 0xd8)), ("Dark Midtones", Color32::from_rgb(0x2e, 0x6a, 0xb8)), ("High Contrast", Color32::from_rgb(0x2f, 0x86, 0xe0)), ("Low Contrast", Color32::from_rgb(0x55, 0x80, 0xc0))] {
                    if !st.fx_search.is_empty() && !name.to_lowercase().contains(&st.fx_search.to_lowercase()) {
                        continue;
                    }
                    ui.vertical(|ui| {
                        let (tr, resp) = ui.allocate_exact_size(vec2(58.0, 48.0), Sense::click());
                        ui.painter().rect_filled(tr, 0.0, c);
                        ui.painter().rect_stroke(tr, 0.0, Stroke::new(1.0, Color32::from_gray(110)), egui::StrokeKind::Outside);
                        ui.label(egui::RichText::new(name).size(11.0).color(theme::TEXT));
                        if resp.clicked() {
                            st.fx_sel = apply_preset(app, name);
                        }
                    });
                    k += 1;
                    if k % 2 == 0 {
                        ui.end_row();
                    }
                }
            });
        });
    });
    ui.add_space(6.0);
    // the effects of the object, in the order they are applied
    let list = o.effects.bitmap.clone();
    if st.fx_sel.is_some_and(|i| i >= list.len()) {
        st.fx_sel = None;
    }
    let shown = st.fx_sel.or(list.len().checked_sub(1));
    let drag_id = ui.scope_id().with("pr_fx_drag");
    let mut drag: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_id)).flatten();
    let mut drop: Option<(usize, usize)> = None;
    egui::Frame::new().fill(Color32::from_rgb(0x26, 0x26, 0x26)).stroke(Stroke::new(1.0, Color32::from_gray(0x5a))).inner_margin(1.0).show(ui, |ui| {
        ui.set_width(w - 2.0);
        ui.set_min_height(158.0);
        egui::ScrollArea::vertical().id_salt("pr_fx_list").max_height(158.0).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut rows = vec![];
            for (i, e) in list.iter().enumerate() {
                let (rr, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click_and_drag());
                if st.fx_sel == Some(i) {
                    ui.painter().rect_filled(rr, 0.0, theme::ACCENT);
                } else if resp.hovered() {
                    ui.painter().rect_filled(rr, 0.0, theme::HOVER);
                }
                let c = if e.enabled { theme::TEXT } else { theme::TEXT_DIM };
                ui.painter().text(pos2(rr.left() + 6.0, rr.center().y), egui::Align2::LEFT_CENTER, crate::fx::name(e), egui::FontId::proportional(12.5), c);
                if resp.clicked() {
                    st.fx_sel = Some(i);
                }
                if resp.drag_started() {
                    drag = Some(i);
                }
                rows.push(rr);
            }
            // dragging an effect up or down the list changes the order
            if let Some(from) = drag {
                match ui.ctx().pointer_interact_pos() {
                    Some(p) => {
                        let to = rows.iter().position(|r| p.y < r.center().y).unwrap_or(rows.len());
                        if let Some(r) = rows.get(to.min(rows.len().saturating_sub(1))) {
                            let y = if to < rows.len() { r.top() } else { r.bottom() };
                            ui.painter().line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(2.0, theme::ACCENT));
                        }
                        if ui.ctx().input(|i| i.pointer.any_released()) {
                            drop = Some((from, to));
                            drag = None;
                        }
                    }
                    None => drag = None,
                }
            }
        });
    });
    ui.ctx().data_mut(|d| d.insert_temp(drag_id, drag));
    if let Some((from, to)) = drop {
        let to = if to > from { to - 1 } else { to };
        if to != from && from < list.len() {
            edit_sel(app, "Effect Order", |x| {
                if from < x.effects.bitmap.len() {
                    let e = x.effects.bitmap.remove(from);
                    let t = to.min(x.effects.bitmap.len());
                    x.effects.bitmap.insert(t, e);
                }
            });
            st.fx_sel = Some(to);
            st.fx_pick = None;
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.set_width(w);
        let add = super::icon_button_resp(app, ui, "plus", "Add effect", true);
        egui::Popup::menu(&add).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.set_min_width(160.0);
            if let Some((_, b)) = crate::ui::bar_named("21cd018a-87c1-476c-9a98-a8e36b9c479e") {
                for it in b.items.iter() {
                    let name = crate::ui::plain(it.text);
                    if it.sub != crate::ui::NO_SUB && FX_CATEGORIES.contains(&name.as_str()) {
                        ui.menu_button(name, |ui| crate::menus::show_bar(app, ui, it.sub));
                    }
                }
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let sel = shown.filter(|i| *i < list.len());
            if super::icon_button(app, ui, "delete", "Delete effect", sel.is_some()) {
                if let Some(i) = sel {
                    edit_sel(app, "Delete Effect", |x| {
                        if i < x.effects.bitmap.len() {
                            x.effects.bitmap.remove(i);
                        }
                    });
                    st.fx_sel = None;
                    st.fx_pick = None;
                }
            }
        });
    });
    // the page of the effect selected in the list (else the last one)
    if let Some(i) = shown.filter(|i| *i < list.len()) {
        ui.add_space(6.0);
        ui.push_id(("pr_fx_page", i, list[i].id.as_str()), |ui| super::fxpage::effect_page(app, ui, o, i, st));
    }
}

/// The adjustment presets of the Presets list, added as a Tone Curve; gives the new effect's place.
fn apply_preset(app: &mut VectorApp, name: &str) -> Option<usize> {
    let pts: &[f64] = match name {
        "Bright Midtones" => &[0.0, 0.0, 128.0, 168.0, 255.0, 255.0],
        "Dark Midtones" => &[0.0, 0.0, 128.0, 88.0, 255.0, 255.0],
        "High Contrast" => &[0.0, 0.0, 64.0, 40.0, 192.0, 216.0, 255.0, 255.0],
        _ => &[0.0, 32.0, 255.0, 223.0],
    };
    let def = crate::fx::by_id("ToneCurve")?;
    let mut e = def.create();
    e.set("Curve0", vector_core::object::FxValue::List(pts.to_vec()));
    let mut at = None;
    edit_sel(app, name, |x| {
        x.effects.bitmap.push(e.clone());
        at = Some(x.effects.bitmap.len() - 1);
    });
    at
}

// ------------------------------------------------------------------------------ object

fn object(app: &mut VectorApp, ui: &mut Ui, o: &Object) {
    use vector_core::object::{CornerType, EllipseKind, PolygonKind};
    let dpi = app.doc().map(|d| d.doc.settings.dpi).unwrap_or(300.0);
    let unit = app.doc().map(|d| d.doc.settings.unit).unwrap_or(Unit::Inches);
    let sfx = format!(" {}", unit.suffix());
    match &o.kind {
        ObjKind::Rect(r) => {
            title(ui, "Rectangle");
            ui.add_space(6.0);
            let (corner, radii) = (r.corner, r.radii);
            row(ui, |ui| {
                label(ui, "Corner:", true);
                for (icon, tip, k) in [("corner_round", "Round corner", CornerType::Round), ("corner_scallop", "Scalloped corner", CornerType::Scallop), ("corner_chamfer", "Chamfered corner", CornerType::Chamfer)] {
                    if tbtn(app, ui, icon, tip, true, corner == k) {
                        edit_sel(app, "Corners", |x| {
                            if let ObjKind::Rect(r) = &mut x.kind {
                                r.corner = k;
                            }
                        });
                    }
                }
            });
            row(ui, |ui| {
                let (en, mut on) = crate::propbar::option_state(app, "4da51d58").unwrap_or((true, false));
                if ui.add_enabled(en, egui::Checkbox::new(&mut on, "Relative corner scaling")).changed() {
                    crate::propbar::press(app, "4da51d58");
                }
            });
            ui.add_space(4.0);
            let lock = app.ts.lock_corners;
            let mut out = radii;
            let mut ch = false;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    for (i, icon) in [(0usize, "corner_tl"), (3, "corner_bl")] {
                        let mut v = unit.to_unit(radii[i], dpi);
                        ui.horizontal(|ui| {
                        super::icon_button(app, ui, icon, "Corner radius", false);
                        if spin(ui, &mut v, 0.0..=10000.0, 0.1, 1, &sfx, 76.0) {
                            out[i] = unit.from_unit(v, dpi).max(0.0);
                            ch = true;
                        }
                        });
                    }
                });
                if tbtn(app, ui, if lock { "lock" } else { "unlock" }, "Edit corners together", true, lock) {
                    app.ts.lock_corners = !lock;
                }
                ui.vertical(|ui| {
                    for (i, icon) in [(1usize, "corner_tr"), (2, "corner_br")] {
                        let mut v = unit.to_unit(radii[i], dpi);
                        ui.horizontal(|ui| {
                        super::icon_button(app, ui, icon, "Corner radius", false);
                        if spin(ui, &mut v, 0.0..=10000.0, 0.1, 1, &sfx, 76.0) {
                            out[i] = unit.from_unit(v, dpi).max(0.0);
                            ch = true;
                        }
                        });
                    }
                });
                mark(ui);
            });
            if ch {
                if lock {
                    let v = out.iter().zip(radii.iter()).find(|(a, b)| a != b).map(|(a, _)| *a).unwrap_or(out[0]);
                    out = [v; 4];
                }
                edit_sel(app, "Corners", |x| {
                    if let ObjKind::Rect(r) = &mut x.kind {
                        r.radii = out;
                    }
                });
            }
        }
        ObjKind::Ellipse(e) => {
            title(ui, "Ellipse");
            ui.add_space(6.0);
            let e = e.clone();
            row(ui, |ui| {
                for (icon, tip, k) in [("ellipse", "Ellipse", EllipseKind::Ellipse), ("ellipse_pie", "Pie", EllipseKind::Pie), ("ellipse_arc", "Arc", EllipseKind::Arc)] {
                    if tbtn(app, ui, icon, tip, true, e.kind == k) {
                        edit_sel(app, "Ellipse", |x| {
                            if let ObjKind::Ellipse(el) = &mut x.kind {
                                el.kind = k;
                            }
                        });
                    }
                }
            });
            let on = e.kind != EllipseKind::Ellipse;
            for (lab, start) in [("Starting angle:", true), ("Ending angle:", false)] {
                row(ui, |ui| {
                    rlabel(ui, lab, 100.0, on);
                    ui.add_enabled_ui(on, |ui| {
                        let mut a = if start { e.start } else { e.end };
                        if spin(ui, &mut a, -360.0..=360.0, 1.0, 1, " °", 80.0) {
                            edit_sel(app, "Ellipse", |x| {
                                if let ObjKind::Ellipse(el) = &mut x.kind {
                                    if start {
                                        el.start = a;
                                    } else {
                                        el.end = a;
                                    }
                                }
                            });
                        }
                    });
                });
            }
        }
        ObjKind::Polygon(p) => {
            let star = p.kind != PolygonKind::Polygon;
            title(ui, if star { "Star" } else { "Polygon" });
            ui.add_space(6.0);
            let p = p.clone();
            row(ui, |ui| {
                rlabel(ui, if star { "Points:" } else { "Sides:" }, 90.0, true);
                let mut n = p.sides as f64;
                if spin(ui, &mut n, 3.0..=500.0, 1.0, 0, "", 80.0) {
                    edit_sel(app, "Polygon", |x| {
                        if let ObjKind::Polygon(pp) = &mut x.kind {
                            pp.sides = n as u32;
                        }
                    });
                }
            });
            if star {
                row(ui, |ui| {
                    rlabel(ui, "Sharpness:", 90.0, true);
                    let mut s = p.sharpness;
                    if spin(ui, &mut s, 1.0..=99.0, 1.0, 0, "", 80.0) {
                        edit_sel(app, "Star", |x| {
                            if let ObjKind::Polygon(pp) = &mut x.kind {
                                pp.sharpness = s;
                            }
                        });
                    }
                });
            }
        }
        _ => {
            title(ui, o.kind.type_name());
            ui.add_space(4.0);
            special_section(app, ui, o);
            let info = crate::statusbar::object_info(app);
            ui.label(egui::RichText::new(info).color(theme::TEXT_DIM));
        }
    }
}

/// QR codes: the text they hold; barcodes: their format and data.
fn special_section(app: &mut VectorApp, ui: &mut Ui, o: &Object) {
    use vector_core::codes::Special;
    let dim = |ui: &mut Ui, s: &str| {
        ui.label(egui::RichText::new(s).color(theme::TEXT_DIM));
    };
    match &o.special {
        Some(Special::QrCode { text }) => {
            let id = egui::Id::unique(("qr_text", o.id));
            let mut t = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| text.clone());
            let r = ui.add(egui::TextEdit::multiline(&mut t).desired_rows(2).desired_width(f32::INFINITY).hint_text("Text or web address"));
            if r.changed() {
                ui.data_mut(|d| d.insert_temp(id, t.clone()));
            }
            if r.lost_focus() {
                ui.data_mut(|d| d.remove::<String>(id));
                if t != *text && !t.is_empty() {
                    let oid = o.id;
                    if let Some(d) = app.doc_mut() {
                        d.modify("QR Code Text", |doc, _| crate::dialogs::object::set_qr_text(doc, oid, &t));
                    }
                }
            }
        }
        Some(Special::Barcode { kind, data }) => dim(ui, &format!("{}: {data}", kind.name())),
        Some(Special::Rollover { state }) => dim(ui, &format!("Showing the {} state", crate::dialogs::object::ROLLOVER_STATES[*state as usize % 3])),
        Some(Special::Impact { .. }) => dim(ui, "Select it with the Impact tool to change it on the property bar"),
        Some(Special::Symmetry { lines, .. }) => dim(ui, &format!("{lines} mirror line{}; double-click to edit", if *lines == 1 { "" } else { "s" })),
        Some(Special::Perspective { kind, .. }) => dim(ui, &format!("{} perspective; double-click to edit", crate::dialogs::symmetry::PERSP_TYPES[*kind as usize % 4])),
        Some(Special::Ole { name, .. }) => dim(ui, &format!("{name}; double-click to open it in its program")),
        None => {}
    }
}

// ------------------------------------------------------------------------------ brush

fn brush(app: &mut VectorApp, ui: &mut Ui, o: &Object) {
    let painterly = matches!(o.style.outline.profile, Some(WidthProfile::Painterly { .. }));
    title(ui, "Painterly Brush");
    ui.add_space(8.0);
    let brushes = crate::propbar::BRUSHES;
    row(ui, |ui| {
        let cur = app.settings.topts.paint_brush.min(brushes.len() - 1);
        let text = if painterly { brushes[cur].0 } else { "" };
        let w = ui.available_width() - 20.0;
        let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
        ui.painter().rect_filled(r, 2.0, Color32::from_rgb(0x33, 0x33, 0x33));
        ui.painter().text(pos2(r.left() + 6.0, r.center().y), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(12.5), if painterly { theme::TEXT } else { theme::TEXT_DIM });
        crate::widgets::chevron(ui.painter(), pos2(r.right() - 9.0, r.center().y), 3.0, theme::TEXT);
        let mut pick = None;
        egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.set_min_width(w - 12.0);
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for (i, b) in brushes.iter().enumerate() {
                    if ui.selectable_label(painterly && i == cur, b.0).clicked() {
                        pick = Some(i);
                        ui.close();
                    }
                }
            });
        });
        if let Some(b) = pick {
            crate::propbar::choose_brush(app, b);
        }
    });
    ui.add_space(10.0);
    label(ui, "Outline color", true);
    row(ui, |ui| {
        let mut c = o.style.outline.color;
        if color_pick_tabs(ui, "pr_brush_color", &mut c, 84.0) {
            edit_live(app, "Outline Color", |x| x.style.outline.color = c);
        }
    });
    ui.add_space(10.0);
    label(ui, "Size", true);
    row(ui, |ui| {
        let mut s = o.style.outline.width;
        let a = slider(ui, &mut s, 0.5..=200.0, 120.0);
        super::icon_button(app, ui, "brush_size", "Size", false);
        let b = spin(ui, &mut s, 0.5..=200.0, 0.5, 1, " pt", 80.0);
        if a || b {
            edit_live(app, "Brush Size", |x| x.style.outline.width = s);
        }
    });
    ui.add_space(6.0);
    label(ui, "Transparency", true);
    row(ui, |ui| {
        let mut a = app.settings.topts.paint_alpha;
        let c1 = slider(ui, &mut a, 0.0..=100.0, 120.0);
        super::icon_button(app, ui, "transparency", "Transparency", false);
        let c2 = spin(ui, &mut a, 0.0..=100.0, 1.0, 0, "", 80.0);
        if c1 || c2 {
            app.settings.topts.paint_alpha = a;
            crate::propbar::paint_apply(app);
        }
    });
    ui.add_space(6.0);
    label(ui, "Simulate pen pressure", true);
    row(ui, |ui| {
        let (en, on) = crate::propbar::option_state(app, "c15728bb").unwrap_or((true, false));
        if tbtn(app, ui, "pen_pressure", "Simulate pen pressure", en, on) {
            crate::propbar::press(app, "c15728bb");
        }
    });
    ui.add_space(6.0);
    label(ui, "Brush smoothing", true);
    row(ui, |ui| {
        let mut s = app.settings.topts.paint_smoothing;
        let c1 = slider(ui, &mut s, 0.0..=100.0, 120.0);
        super::icon_button(app, ui, "brush_smoothing", "Brush smoothing", false);
        let c2 = spin(ui, &mut s, 0.0..=100.0, 1.0, 0, "", 80.0);
        if c1 || c2 {
            app.settings.topts.paint_smoothing = s;
        }
    });
    ui.add_space(10.0);
    let img = app.icons.get(ui.ctx(), "reset", 14.0, crate::icons::Tone::Disabled);
    let b = match img {
        Some(t) => egui::Button::image_and_text(egui::Image::new(t).fit_to_exact_size(vec2(14.0, 14.0)), "Reset brush"),
        None => egui::Button::new("Reset brush"),
    };
    if ui.add_enabled(painterly, b.min_size(vec2(100.0, 24.0))).clicked() {
        crate::propbar::press(app, "3860fe09");
    }
    ui.add_space(10.0);
    if ui.add_enabled(painterly, egui::Button::new("Clear Brushstroke").min_size(vec2(120.0, 24.0))).clicked() {
        crate::propbar::press(app, "19fd40bd");
    }
}

// ------------------------------------------------------------------------------ summary

fn summary(app: &mut VectorApp, ui: &mut Ui, o: &Object) {
    title(ui, "Summary");
    ui.add_space(4.0);
    let Some(d) = app.doc() else { return };
    let n = d.selection.len();
    let u = d.doc.settings.unit;
    let dpi = d.doc.settings.dpi;
    let layer = d.doc.locate(o.id).map(|(p, l, _)| d.doc.pages[p].layers[l].name.clone()).unwrap_or_default();
    let bb = d.selection_bounds();
    let num = |v: f64| format!("{:.2}", u.to_unit(v, dpi));
    let ab = if u == Unit::Inches { "in" } else { u.suffix() };
    let mut lines: Vec<(&str, String)> = vec![
        ("Selection:", format!("{n} Objects Selected")),
        ("Layer:", layer),
        ("Type:", if n > 1 { "Multiple Objects".to_string() } else { o.kind.type_name().to_string() }),
    ];
    if let Some(b) = bb {
        lines.push(("Selection dimension:", format!("{} {ab} x {} {ab}", num(b.width()), num(b.height()))));
        lines.push(("Selection center:", format!("({}, {}) {ab}", num(b.center().x), num(b.center().y))));
        lines.push(("Rotation center:", format!("({}, {}) {ab}", num(b.center().x), num(b.center().y))));
    }
    for (k, v) in lines {
        ui.horizontal(|ui| {
            rlabel(ui, k, 126.0, true);
            ui.label(egui::RichText::new(v).color(theme::TEXT));
        });
    }
    ui.add_space(20.0);
    gap(ui);
    label(ui, "Wrap paragraph text:", true);
    let wrap = o.effects.wrap;
    let names = ["None", "Contour - Flow Left", "Contour - Flow Right", "Contour - Straddle Text", "Square - Flows Left", "Square - Flows Right", "Square - Straddle Text", "Square - Above/Below"];
    row(ui, |ui| {
        let mut i = wrap.map(|w| w.style as usize + 1).unwrap_or(0);
        if super::choice(ui, "pr_wrap", &mut i, &names, 168.0) {
            let off = wrap.map(|w| w.offset).unwrap_or(8.0);
            let nw = if i == 0 { None } else { Some(vector_core::object::TextWrap { style: (i - 1) as u8, offset: off }) };
            edit_sel(app, "Wrap Paragraph Text", |x| x.effects.wrap = nw);
        }
        ui.add_enabled_ui(wrap.is_some(), |ui| {
            let mut off = wrap.map(|w| w.offset).unwrap_or(0.0);
            if spin(ui, &mut off, 0.0..=720.0, 1.0, 1, " pt", 96.0) {
                edit_sel(app, "Text Wrap Offset", |x| {
                    if let Some(w) = &mut x.effects.wrap {
                        w.offset = off;
                    }
                });
            }
        });
    });
    ui.add_space(6.0);
    row(ui, |ui| {
        let mut w = app.settings.wrap_connector_lines;
        if ui.checkbox(&mut w, "Wrap Connector Line").changed() {
            app.settings.wrap_connector_lines = w;
        }
    });
    ui.add_space(10.0);
    let locked = o.locked;
    let w = ui.available_width() - 20.0;
    if ui.add_sized(vec2(w, 24.0), egui::Button::new(if locked { "Unlock Object" } else { "Lock Object" })).clicked() {
        edit_sel(app, if locked { "Unlock Object" } else { "Lock Object" }, |x| x.locked = !locked);
    }
}

fn doc_summary(app: &VectorApp, ui: &mut Ui) {
    let Some(d) = app.doc() else { return };
    let u = d.doc.settings.unit;
    let dpi = d.doc.settings.dpi;
    let p = d.page_ref();
    let count: usize = p.layers.iter().map(|l| l.objects.len()).sum();
    for (k, v) in [
        ("Page:", format!("{} of {}", d.page + 1, d.doc.pages.len())),
        ("Page size:", format!("{} x {}", u.format(p.width, dpi), u.format(p.height, dpi))),
        ("Objects:", format!("{count} on {} layers", p.layers.len())),
        ("Resolution:", format!("{dpi} dpi")),
    ] {
        ui.label(egui::RichText::new(k).color(theme::TEXT_DIM));
        ui.label(egui::RichText::new(v).color(theme::TEXT));
        ui.add_space(2.0);
    }
}
