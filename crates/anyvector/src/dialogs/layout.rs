//! The Layout menu dialogs: Insert, Duplicate, Rename, Delete and Go to Page, Page Number
//! Settings and Border and Grommet.

use super::{button, combo, ok_button, window};
use crate::app::AnyVectorApp;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use kurbo::{Affine, Point};
use std::sync::Arc;
use vector_core::document::ImageData;
use vector_core::object::{EllipseKind, EllipseShape, ImageAdjust, ImageObj, RectShape};
use vector_core::{Color, Document, Fill, ObjKind, Object, Style};

// ------------------------------------------------------------------------------------------
// Page Number Settings
// ------------------------------------------------------------------------------------------

pub const NUMBER_STYLES: [&str; 5] = ["1, 2, 3, 4, 5...", "I, II, III, IV, V...", "i, ii, iii, iv, v ...", "A, B, C, D, E...", "a, b, c, d, e ..."];

#[derive(Clone)]
pub struct PageNumberState {
    pub start: u32,
    pub from_page: u32,
    pub style: usize,
}

pub fn page_number_settings(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.page_numbers.take() else { return };
    let pages = app.doc().map(|d| d.doc.pages.len()).unwrap_or(1) as u32;
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "page_numbers", "Page Number Settings", vec2(275.0, 195.0), |ui| {
        ui.add_space(10.0);
        egui::Grid::new("pn_grid").num_columns(2).spacing(vec2(10.0, 10.0)).show(ui, |ui| {
            ui.label("Start at number:");
            ui.add_sized(vec2(140.0, 22.0), egui::DragValue::new(&mut st.start).range(1..=99999));
            ui.end_row();
            ui.label("Start on page:");
            ui.add_sized(vec2(140.0, 22.0), egui::DragValue::new(&mut st.from_page).range(1..=pages.max(1)));
            ui.end_row();
            ui.label("Style:");
            combo(ui, "pn_style", &mut st.style, &NUMBER_STYLES, 128.0);
            ui.end_row();
        });
        ui.add_space(16.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            cancel = button(ui, "Cancel", true);
            ui.add_space(4.0);
            ok = ok_button(ui, "OK", true);
        });
    });
    if ok {
        if let Some(d) = app.doc_mut() {
            d.modify("Page Number Settings", |doc, _| {
                doc.settings.page_number_start = st.start.max(1);
                doc.settings.page_number_from = st.from_page.max(1);
                doc.settings.page_number_style = st.style as u8;
            });
        }
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.page_numbers = Some(st);
    }
}

/// Layout ▸ Insert Page Number: 0 on the active layer of the current page, 1 on all pages,
/// 2 on all odd pages, 3 on all even pages. The number is a text object that follows the page
/// order and the Page Number Settings.
pub fn insert_page_number(app: &mut AnyVectorApp, which: u8) {
    let Some(d) = app.doc_mut() else { return };
    let (cur, layer) = (d.page, d.active_loc().1);
    // On all pages / odd pages / even pages: a new master layer of that kind holds the number.
    d.modify("Insert Page Number", |doc, sel| {
        sel.clear();
        let mut t = vector_core::text::TextObj::artistic("#", vector_core::text::CharFormat { size: 12.0, ..Default::default() });
        t.page_number = true;
        t.align = vector_core::text::TextAlign::Center;
        let (w, h) = (doc.pages[cur].width, doc.pages[cur].height);
        let id = doc.new_id();
        let mut o = Object::new(id, ObjKind::Text(t), Affine::translate((w / 2.0, h - 36.0)), Style::text_default());
        o.name = "Page Number".into();
        let (p, l) = if which == 0 { (cur, layer.min(doc.pages[cur].layers.len().saturating_sub(1))) } else { (0, doc.new_master_layer(which)) };
        if let Some(layer) = doc.pages[p].layers.get_mut(l) {
            layer.objects.push(o);
            sel.push(id);
        }
    });
}

// ------------------------------------------------------------------------------------------
// Border and Grommet
// ------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct BorderGrommet {
    /// Source: the page (false) or the selection (true).
    pub selection: bool,
    pub border: bool,
    /// 0 Page, 1 Color, 2 Stretch, 3 Mirror.
    pub border_type: usize,
    pub border_size: f64,
    pub border_color: Color,
    pub grommets: bool,
    pub use_border_area: bool,
    pub grommet_size: f64,
    pub margin: f64,
    /// Corners, Top, Bottom, Left, Right.
    pub placement: [bool; 5],
    /// Number of grommets (false) or Spacing (true).
    pub by_spacing: bool,
    pub vertical: u32,
    pub horizontal: u32,
    pub spacing: f64,
}

impl Default for BorderGrommet {
    fn default() -> Self {
        BorderGrommet {
            selection: false,
            border: false,
            border_type: 0,
            border_size: 90.0,
            border_color: Color::WHITE,
            grommets: false,
            use_border_area: false,
            grommet_size: 18.0,
            margin: 90.0,
            placement: [true, false, false, false, false],
            by_spacing: false,
            vertical: 3,
            horizontal: 3,
            spacing: 144.0,
        }
    }
}

const BORDER_TYPES: [&str; 4] = ["Page", "Color", "Stretch", "Mirror"];

impl BorderGrommet {
    /// The grommet centers for artwork occupying `art` (border added outside it).
    pub fn grommet_points(&self, art: kurbo::Rect) -> Vec<Point> {
        if !self.grommets {
            return vec![];
        }
        let g = if self.use_border_area && self.border {
            art.inflate(self.border_size / 2.0, self.border_size / 2.0)
        } else {
            art.inset(-self.margin)
        };
        if g.width() < 0.0 || g.height() < 0.0 {
            return vec![];
        }
        let mut pts: Vec<Point> = vec![];
        let mut add = |p: Point| {
            if !pts.iter().any(|q| (*q - p).hypot() < 0.01) {
                pts.push(p);
            }
        };
        let along = |a: Point, b: Point, count: u32, spacing: f64| -> Vec<Point> {
            let len = (b - a).hypot();
            let n = if self.by_spacing { ((len / spacing.max(1.0)).floor() as u32 + 1).max(2) } else { count.max(2) };
            (0..n).map(|k| a.lerp(b, k as f64 / (n - 1) as f64)).collect()
        };
        let [corners, top, bottom, left, right] = self.placement;
        if corners {
            for p in [Point::new(g.x0, g.y0), Point::new(g.x1, g.y0), Point::new(g.x1, g.y1), Point::new(g.x0, g.y1)] {
                add(p);
            }
        }
        if top {
            for p in along(Point::new(g.x0, g.y0), Point::new(g.x1, g.y0), self.horizontal, self.spacing) {
                add(p);
            }
        }
        if bottom {
            for p in along(Point::new(g.x0, g.y1), Point::new(g.x1, g.y1), self.horizontal, self.spacing) {
                add(p);
            }
        }
        if left {
            for p in along(Point::new(g.x0, g.y0), Point::new(g.x0, g.y1), self.vertical, self.spacing) {
                add(p);
            }
        }
        if right {
            for p in along(Point::new(g.x1, g.y0), Point::new(g.x1, g.y1), self.vertical, self.spacing) {
                add(p);
            }
        }
        pts
    }
}

/// Build the banner document: the artwork of the page (or the selection) with the border
/// around it and the grommets on top. The original document is not changed.
pub fn border_and_grommet(src: &Document, page: usize, selection: &[vector_core::ObjId], o: &BorderGrommet) -> Document {
    let pg = &src.pages[page];
    let objs: Vec<Object> = if o.selection {
        selection.iter().filter_map(|id| src.find(*id).cloned()).collect()
    } else {
        pg.layers.iter().filter(|l| l.visible).flat_map(|l| l.objects.iter().cloned()).collect()
    };
    let art = if o.selection && !objs.is_empty() { vector_core::object::union_bounds(objs.iter().map(|x| src.bounds_of(x))) } else { pg.rect() };
    let b = if o.border { o.border_size.max(0.0) } else { 0.0 };
    let (w, h) = (art.width() + 2.0 * b, art.height() + 2.0 * b);
    let mut doc = Document::new(w.max(1.0), h.max(1.0));
    doc.settings = src.settings.clone();
    doc.settings.multipage_view = false;
    doc.symbols = src.symbols.clone();
    doc.images = src.images.clone();
    doc.ensure_next_id();
    let shift = Affine::translate((b - art.x0, b - art.y0));
    let mut content = vec![];
    // the border
    if o.border && b > 0.0 {
        match o.border_type {
            0 => doc.pages[0].background = Some(pg.background.unwrap_or(Color::WHITE)),
            1 => doc.pages[0].background = Some(o.border_color),
            _ => content.extend(bitmap_border(src, page, art, b, o.border_type == 3, &mut doc)),
        }
    } else {
        doc.pages[0].background = pg.background;
    }
    if o.border && b > 0.0 && o.border_type >= 2 {
        // behind the artwork, the page itself stays white
        let id = doc.new_id();
        let mut st = Style::default();
        st.fill = Fill::Solid(pg.background.unwrap_or(Color::WHITE));
        st.outline = vector_core::Outline::none();
        content.insert(0, Object::new(id, ObjKind::Rect(RectShape { w: art.width(), h: art.height(), radii: [0.0; 4], corner: Default::default() }), Affine::translate((b, b)), st));
    }
    for mut x in objs {
        doc.reassign_ids(&mut x);
        x.apply_transform(shift);
        content.push(x);
    }
    doc.pages[0].layers[0].objects = content;
    // the grommets, on a layer of their own
    let pts = o.grommet_points(art);
    if !pts.is_empty() {
        let lid = doc.new_id();
        let mut layer = vector_core::document::Layer::new(lid, "Grommets");
        let r = o.grommet_size.max(1.0);
        for p in pts {
            let p = shift * p;
            let id = doc.new_id();
            let mut st = Style::default();
            st.fill = Fill::Solid(Color::WHITE);
            st.outline.width = 0.5;
            st.outline.color = Some(Color::BLACK);
            let mut g = Object::new(id, ObjKind::Ellipse(EllipseShape { w: r, h: r, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false }), Affine::translate((p.x - r / 2.0, p.y - r / 2.0)), st);
            g.name = "Grommet".into();
            layer.objects.push(g);
        }
        doc.pages[0].layers.push(layer);
    }
    doc
}

/// Stretch (repeat the edge pixels) or Mirror (reflect the edge band) borders, as bitmaps.
fn bitmap_border(src: &Document, page: usize, art: kurbo::Rect, b: f64, mirror: bool, doc: &mut Document) -> Vec<Object> {
    let dpi = 150.0;
    let Some(pm) = vector_render::render_page_image(src, page, dpi, src.pages[page].background.or(Some(Color::WHITE)), Some(art), true) else { return vec![] };
    let (w, h) = (pm.width() as i64, pm.height() as i64);
    let bp = ((b * dpi / 72.0).round() as i64).max(1);
    // unpremultiplied RGBA of the rendered artwork
    let px: Vec<u8> = pm.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }).collect();
    let get = |x: i64, y: i64| -> [u8; 4] {
        let (x, y) = if mirror {
            // reflect across the artwork edge
            let fx = if x < 0 { -x - 1 } else if x >= w { 2 * w - x - 1 } else { x };
            let fy = if y < 0 { -y - 1 } else if y >= h { 2 * h - y - 1 } else { y };
            (fx.clamp(0, w - 1), fy.clamp(0, h - 1))
        } else {
            (x.clamp(0, w - 1), y.clamp(0, h - 1))
        };
        let i = ((y * w + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    };
    let (ow, oh) = (w + 2 * bp, h + 2 * bp);
    let mut out = vec![0u8; (ow * oh * 4) as usize];
    for y in 0..oh {
        for x in 0..ow {
            let (sx, sy) = (x - bp, y - bp);
            if sx >= 0 && sx < w && sy >= 0 && sy < h {
                continue; // the artwork itself stays vector
            }
            let c = get(sx, sy);
            let i = ((y * ow + x) * 4) as usize;
            out[i..i + 4].copy_from_slice(&c);
        }
    }
    let img = ImageData { width: ow as u32, height: oh as u32, rgba: Arc::new(out), encoded: Arc::new(vec![]), format: "png".into(), name: "Border".into() };
    let iid = doc.add_image(img);
    let id = doc.new_id();
    let mut st = Style::default();
    st.fill = Fill::None;
    st.outline = vector_core::Outline::none();
    let full = (art.width() + 2.0 * b, art.height() + 2.0 * b);
    let mut o = Object::new(id, ObjKind::Image(ImageObj { image: iid, w: full.0, h: full.1, crop: [0.0, 0.0, 1.0, 1.0], adjust: ImageAdjust::default() }), Affine::IDENTITY, st);
    o.name = "Border".into();
    vec![o]
}

pub struct BorderGrommetState {
    pub opts: BorderGrommet,
}

pub(crate) fn length(ui: &mut Ui, v: &mut f64, unit: vector_core::Unit, dpi: f64, enabled: bool) {
    let k = unit.points_per_unit(dpi).max(1e-9);
    let mut x = *v / k;
    let r = ui.add_enabled(enabled, egui::DragValue::new(&mut x).speed(0.01).range(0.0..=10000.0).max_decimals(unit.decimals()).suffix(format!(" {}", unit.suffix())));
    if r.changed() {
        *v = x * k;
    }
}

pub fn border_grommet(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.border_grommet.take() else { return };
    let Some(d) = app.doc() else { return };
    let (unit, dpi) = (d.doc.settings.unit, d.doc.settings.dpi);
    let has_sel = !d.selection.is_empty();
    let art = if st.opts.selection && has_sel { d.selection_bounds().unwrap_or(d.page_ref().rect()) } else { d.page_ref().rect() };
    let (mut ok, mut cancel, mut reset) = (false, false, false);
    let ((), dismissed) = window(ctx, "border_grommet", "Border and Grommet", vec2(786.0, 584.0), |ui| {
        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            // preview
            let (pr, _) = ui.allocate_exact_size(vec2(500.0, 490.0), Sense::hover());
            preview(ui, pr, &st.opts, art);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.set_width(250.0);
                let o = &mut st.opts;
                panel(ui, "Source", |ui| {
                    ui.add_enabled_ui(has_sel, |ui| {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut o.selection, false, "Page");
                            ui.radio_value(&mut o.selection, true, "Selection");
                        });
                    });
                });
                panel(ui, "Border", |ui| {
                    ui.checkbox(&mut o.border, "Add border");
                    ui.add_enabled_ui(o.border, |ui| {
                        egui::Grid::new("bg_border").num_columns(2).spacing(vec2(14.0, 6.0)).show(ui, |ui| {
                            ui.label("   Type:");
                            ui.horizontal(|ui| {
                                combo(ui, "bg_type", &mut o.border_type, &BORDER_TYPES, 66.0);
                                if o.border_type == 1 {
                                    let mut c = Color32::from_rgb(o.border_color.r, o.border_color.g, o.border_color.b);
                                    if ui.color_edit_button_srgba(&mut c).changed() {
                                        o.border_color = Color::rgb(c.r(), c.g(), c.b());
                                    }
                                }
                            });
                            ui.end_row();
                            ui.label("   Size:");
                            length(ui, &mut o.border_size, unit, dpi, true);
                            ui.end_row();
                        });
                    });
                });
                panel(ui, "Grommet", |ui| {
                    ui.checkbox(&mut o.grommets, "Add grommets");
                    let on = o.grommets;
                    ui.add_enabled(on && o.border, egui::Checkbox::new(&mut o.use_border_area, "Use border area"));
                    egui::Grid::new("bg_grommet").num_columns(2).spacing(vec2(30.0, 6.0)).show(ui, |ui| {
                        ui.label("   Size:");
                        length(ui, &mut o.grommet_size, unit, dpi, on);
                        ui.end_row();
                        ui.label("   Margin:");
                        length(ui, &mut o.margin, unit, dpi, on && !(o.use_border_area && o.border));
                        ui.end_row();
                    });
                    ui.separator();
                    ui.label(egui::RichText::new("Placement and distribution").strong());
                    ui.horizontal(|ui| {
                        ui.add_space(36.0);
                        let tips = ["Corners", "Top", "Bottom", "Left", "Right"];
                        for (i, tip) in tips.iter().enumerate() {
                            let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), if on { Sense::click() } else { Sense::hover() });
                            if resp.clicked() {
                                o.placement[i] = !o.placement[i];
                            }
                            placement_icon(ui, r, i, o.placement[i], on, resp.hovered());
                            resp.on_hover_text(*tip);
                            ui.add_space(6.0);
                        }
                    });
                    ui.add_enabled_ui(on, |ui| {
                        ui.radio_value(&mut o.by_spacing, false, "Number of grommets");
                        ui.radio_value(&mut o.by_spacing, true, "Spacing");
                        egui::Grid::new("bg_count").num_columns(2).spacing(vec2(30.0, 6.0)).show(ui, |ui| {
                            if !on {
                                // the reference shows the counts only while grommets are on
                            } else if o.by_spacing {
                                ui.label("   Spacing:");
                                length(ui, &mut o.spacing, unit, dpi, true);
                                ui.end_row();
                            } else {
                                ui.label("   Vertical:");
                                ui.add(egui::DragValue::new(&mut o.vertical).range(2..=999));
                                ui.end_row();
                                ui.label("   Horizontal:");
                                ui.add(egui::DragValue::new(&mut o.horizontal).range(2..=999));
                                ui.end_row();
                            }
                        });
                    });
                });
            });
        });
        ui.add_space(6.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.add(egui::Button::new("?").min_size(vec2(24.0, 24.0))).on_hover_text("Help");
            reset = button(ui, "Reset", true);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cancel = button(ui, "Cancel", true);
                ui.add_space(4.0);
                ok = ok_button(ui, "OK", true);
            });
        });
    });
    if reset {
        let sel = st.opts.selection;
        st.opts = BorderGrommet { selection: sel, ..Default::default() };
    }
    if ok {
        let d = app.doc().unwrap();
        let doc = border_and_grommet(&d.doc, d.page, &d.selection, &st.opts);
        let title = format!("{} (banner).cdr", d.stem());
        let dv = crate::docview::DocView::new(doc, title, app.settings.undo_levels);
        app.push_doc(dv);
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.border_grommet = Some(st);
    }
}

fn panel(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP())).corner_radius(3.0).inner_margin(8.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(title).strong());
        body(ui);
    });
    ui.add_space(6.0);
}

/// The placement buttons: a frame with the grommet spots on its corners or one side.
fn placement_icon(ui: &Ui, r: Rect, which: usize, on: bool, enabled: bool, hovered: bool) {
    let p = ui.painter();
    if on {
        p.rect_filled(r, 2.0, if enabled { theme::ACCENT() } else { theme::FIELD() });
    } else if hovered && enabled {
        p.rect_filled(r, 2.0, theme::HOVER());
    }
    let col = if enabled { theme::TEXT() } else { theme::TEXT_DIM() };
    let f = r.shrink(5.0);
    p.rect_stroke(f, 0.0, Stroke::new(1.0, col), egui::StrokeKind::Middle);
    let dots: Vec<egui::Pos2> = match which {
        0 => vec![f.left_top(), f.right_top(), f.right_bottom(), f.left_bottom()],
        1 => (0..4).map(|k| pos2(f.left() + f.width() * k as f32 / 3.0, f.top() + 2.0)).collect(),
        2 => (0..4).map(|k| pos2(f.left() + f.width() * k as f32 / 3.0, f.bottom() - 2.0)).collect(),
        3 => (0..4).map(|k| pos2(f.left() + 2.0, f.top() + f.height() * k as f32 / 3.0)).collect(),
        _ => (0..4).map(|k| pos2(f.right() - 2.0, f.top() + f.height() * k as f32 / 3.0)).collect(),
    };
    for d in dots {
        let d = if which == 0 { pos2(d.x + if d.x < f.center().x { 2.0 } else { -2.0 }, d.y + if d.y < f.center().y { 2.0 } else { -2.0 }) } else { d };
        p.circle_stroke(d, 1.2, Stroke::new(1.0, col));
    }
}

/// The preview: the artwork outline, the border (dashed) and the grommets.
fn preview(ui: &Ui, area: Rect, o: &BorderGrommet, art: kurbo::Rect) {
    let p = ui.painter();
    p.rect_filled(area, 0.0, Color32::WHITE);
    let b = if o.border { o.border_size } else { 0.0 };
    let total = art.inflate(b, b);
    let k = ((area.width() as f64 - 120.0) / total.width().max(1.0)).min((area.height() as f64 - 120.0) / total.height().max(1.0)).min(area.width() as f64 / 2.0 / art.width().max(1.0)) as f32;
    let k = k.max(0.01);
    let c = area.center();
    let tc = art.center();
    let map = |q: Point| pos2(c.x + ((q.x - tc.x) as f32) * k, c.y + ((q.y - tc.y) as f32) * k);
    let rect = |r: kurbo::Rect| Rect::from_two_pos(map(Point::new(r.x0, r.y0)), map(Point::new(r.x1, r.y1)));
    if o.border {
        let br = rect(total);
        if o.border_type == 1 {
            p.rect_filled(br, 0.0, Color32::from_rgb(o.border_color.r, o.border_color.g, o.border_color.b));
            p.rect_filled(rect(art), 0.0, Color32::WHITE);
        }
        let pts = [br.left_top(), br.right_top(), br.right_bottom(), br.left_bottom(), br.left_top()];
        p.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.0, Color32::BLACK), 4.0, 3.0));
    }
    p.rect_stroke(rect(art), 0.0, Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Middle);
    let r = (o.grommet_size as f32 * k / 2.0).max(2.0);
    for g in o.grommet_points(art) {
        p.circle_stroke(map(g), r, Stroke::new(1.0, Color32::BLACK));
    }
}

// ------------------------------------------------------------------------------------------
// Insert, Duplicate, Rename, Delete and Go to Page
// ------------------------------------------------------------------------------------------

/// A section title with a rule to its right ("Pages ────").
pub(crate) fn rule_heading(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(text).strong());
        let w = ui.available_width() - 4.0;
        let (r, _) = ui.allocate_exact_size(vec2(w.max(0.0), 14.0), Sense::hover());
        ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, theme::SEP()));
    });
    ui.add_space(6.0);
}

/// A row with its label right-aligned in a column `w` wide.
pub(crate) fn rrow<R>(ui: &mut Ui, label: &str, w: f32, body: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.add_sized(vec2(w, 22.0), egui::Label::new(label).halign(egui::Align::Max));
        ui.add_space(4.0);
        body(ui)
    })
    .inner
}

#[derive(Clone)]
pub struct InsertPageState {
    pub count: usize,
    pub before: bool,
    /// 1-based.
    pub existing: usize,
    /// Points.
    pub w: f64,
    pub h: f64,
    pub unit: vector_core::Unit,
}

impl InsertPageState {
    pub fn new(app: &AnyVectorApp, before: bool) -> Option<InsertPageState> {
        let d = app.doc()?;
        let pg = d.page_ref();
        Some(InsertPageState { count: 1, before, existing: d.page + 1, w: pg.width, h: pg.height, unit: d.doc.settings.unit })
    }
}

fn preset_index(w: f64, h: f64) -> usize {
    let ps = vector_core::units::page_presets();
    ps.iter()
        .position(|(n, pw, ph)| *n != "Custom" && (((pw - w).abs() < 0.5 && (ph - h).abs() < 0.5) || ((pw - h).abs() < 0.5 && (ph - w).abs() < 0.5)))
        .unwrap_or(ps.len() - 1)
}

/// The portrait and landscape buttons.
fn orientation_button(ui: &mut Ui, landscape: bool, on: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(r, 2.0, theme::ACCENT());
    } else if resp.hovered() {
        p.rect_filled(r, 2.0, theme::HOVER());
    }
    let sz = if landscape { vec2(14.0, 10.0) } else { vec2(10.0, 14.0) };
    let pg = Rect::from_center_size(r.center(), sz);
    let fold = 3.0;
    let pts = vec![pg.left_top(), pos2(pg.right() - fold, pg.top()), pos2(pg.right(), pg.top() + fold), pg.right_bottom(), pg.left_bottom(), pg.left_top()];
    p.add(egui::Shape::line(pts, Stroke::new(1.0, theme::TEXT())));
    resp.on_hover_text(if landscape { "Landscape" } else { "Portrait" }).clicked()
}

pub fn insert_page(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.insert_page.take() else { return };
    let Some(d) = app.doc() else { return };
    let (pages, dpi) = (d.doc.pages.len(), d.doc.settings.dpi);
    let presets = vector_core::units::page_presets();
    let names: Vec<&str> = presets.iter().map(|p| p.0).collect();
    let unit_names: Vec<&str> = vector_core::Unit::ALL.iter().map(|u| u.name()).collect();
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "insert_page", "Insert Page", vec2(385.0, 430.0), |ui| {
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing.y = 10.0;
        rule_heading(ui, "Pages");
        rrow(ui, "Number of pages:", 160.0, |ui| ui.add_sized(vec2(58.0, 22.0), egui::DragValue::new(&mut st.count).range(1..=999)));
        rrow(ui, "Place:", 160.0, |ui| ui.radio_value(&mut st.before, true, "Before"));
        rrow(ui, "", 160.0, |ui| ui.radio_value(&mut st.before, false, "After"));
        rrow(ui, "Existing Page:", 160.0, |ui| ui.add_sized(vec2(58.0, 22.0), egui::DragValue::new(&mut st.existing).range(1..=pages)));
        ui.add_space(4.0);
        rule_heading(ui, "Page size");
        let mut size = preset_index(st.w, st.h);
        rrow(ui, "Size:", 160.0, |ui| {
            if combo(ui, "ip_size", &mut size, &names, 152.0) {
                let (_, pw, ph) = presets[size];
                let landscape = st.w > st.h;
                (st.w, st.h) = if landscape { (pw.max(ph), pw.min(ph)) } else { (pw, ph) };
            }
        });
        let k = st.unit.points_per_unit(dpi).max(1e-9);
        let dec = st.unit.decimals();
        rrow(ui, "Width:", 160.0, |ui| {
            let mut v = st.w / k;
            if ui.add_sized(vec2(80.0, 22.0), egui::DragValue::new(&mut v).speed(0.01).range(0.001..=100000.0).max_decimals(dec)).changed() {
                st.w = v * k;
            }
            let mut ui_unit = vector_core::Unit::ALL.iter().position(|u| *u == st.unit).unwrap_or(0);
            if combo(ui, "ip_unit", &mut ui_unit, &unit_names, 66.0) {
                st.unit = vector_core::Unit::ALL[ui_unit];
            }
        });
        rrow(ui, "Height:", 160.0, |ui| {
            let mut v = st.h / k;
            if ui.add_sized(vec2(80.0, 22.0), egui::DragValue::new(&mut v).speed(0.01).range(0.001..=100000.0).max_decimals(dec)).changed() {
                st.h = v * k;
            }
            let landscape = st.w > st.h;
            if orientation_button(ui, false, !landscape) && landscape {
                (st.w, st.h) = (st.h, st.w);
            }
            if orientation_button(ui, true, landscape) && !landscape {
                (st.w, st.h) = (st.h, st.w);
            }
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    cancel = button(ui, "Cancel", true);
                    ui.add_space(4.0);
                    ok = ok_button(ui, "OK", true);
                });
            });
        });
    });
    if ok {
        if let Some(d) = app.doc_mut() {
            let ex = st.existing.clamp(1, d.doc.pages.len()) - 1;
            let at = if st.before { ex } else { ex + 1 };
            let (w, h, n) = (st.w, st.h, st.count);
            d.modify("Insert Page", |doc, sel| {
                for k in 0..n {
                    doc.add_page(at + k, w, h);
                }
                sel.clear();
            });
            d.page = at;
            d.needs_fit = true;
        }
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.insert_page = Some(st);
    }
}

#[derive(Clone)]
pub struct DuplicatePageState {
    pub page: usize,
    /// 0 before the selected page, 1 after it, 2 to a new document.
    pub place: u8,
    /// Copy the layers and their contents (true) or the layers only (false).
    pub contents: bool,
}

/// A copy of page `i` with fresh ids, its layers emptied unless `contents`.
fn copy_page(src: &Document, i: usize, contents: bool, dst: &mut Document) -> vector_core::Page {
    let mut p = src.pages[i].clone();
    p.id = dst.new_id();
    p.layers.retain(|l| l.master == 0);
    for l in p.layers.iter_mut() {
        l.id = dst.new_id();
        if contents {
            for o in l.objects.iter_mut() {
                dst.reassign_ids(o);
            }
        } else {
            l.objects.clear();
        }
    }
    p
}

pub fn duplicate_page(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.duplicate_page.take() else { return };
    let Some(d) = app.doc() else { return };
    let name = d.doc.page_name(st.page.min(d.doc.pages.len() - 1));
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "duplicate_page", "Duplicate Page", vec2(270.0, 230.0), |ui| {
        ui.add_space(8.0);
        egui::Grid::new("dp_grid").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
            ui.label("Duplicate page:");
            ui.label(&name);
            ui.end_row();
            ui.label("Insert new page:");
            ui.radio_value(&mut st.place, 0, "Before selected page");
            ui.end_row();
            ui.label("");
            ui.radio_value(&mut st.place, 1, "After selected page");
            ui.end_row();
            ui.label("");
            ui.radio_value(&mut st.place, 2, "To new document");
            ui.end_row();
        });
        ui.add_space(6.0);
        ui.radio_value(&mut st.contents, false, "Copy layer(s) only");
        ui.radio_value(&mut st.contents, true, "Copy layer(s) and their contents");
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    cancel = button(ui, "Cancel", true);
                    ui.add_space(4.0);
                    ok = ok_button(ui, "OK", true);
                });
            });
        });
    });
    if ok {
        let cur = st.page;
        if st.place == 2 {
            let d = app.doc().unwrap();
            let src = &d.doc;
            let pg = &src.pages[cur.min(src.pages.len() - 1)];
            let mut doc = Document::new(pg.width, pg.height);
            doc.settings = src.settings.clone();
            doc.settings.multipage_view = false;
            doc.symbols = src.symbols.clone();
            doc.images = src.images.clone();
            doc.palette = src.palette.clone();
            doc.color_styles = src.color_styles.clone();
            doc.object_styles = src.object_styles.clone();
            doc.ensure_next_id();
            let p = copy_page(src, cur.min(src.pages.len() - 1), st.contents, &mut doc);
            doc.pages = vec![p];
            app.untitled_counter += 1;
            let dv = crate::docview::DocView::new(doc, format!("Untitled-{}.cdr", app.untitled_counter), app.settings.undo_levels);
            app.push_doc(dv);
        } else if let Some(d) = app.doc_mut() {
            let at = if st.place == 0 { cur } else { cur + 1 };
            let contents = st.contents;
            d.modify("Duplicate Page", |doc, sel| {
                let src = doc.clone();
                let p = copy_page(&src, cur, contents, doc);
                doc.pages.insert(at, p);
                sel.clear();
            });
            d.page = at;
            d.needs_fit = true;
        }
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.duplicate_page = Some(st);
    }
}

#[derive(Clone)]
pub struct DeletePageState {
    /// 1-based.
    pub from: usize,
    pub through: bool,
    pub to: usize,
}

pub fn delete_page(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.delete_page.take() else { return };
    let pages = app.doc().map(|d| d.doc.pages.len()).unwrap_or(1);
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "delete_page", "Delete Page", vec2(315.0, 190.0), |ui| {
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.add_sized(vec2(124.0, 22.0), egui::Label::new("Delete Page:").halign(egui::Align::Min));
            ui.add_sized(vec2(64.0, 22.0), egui::DragValue::new(&mut st.from).range(1..=pages));
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.add_sized(vec2(124.0, 22.0), egui::Checkbox::new(&mut st.through, "Through to page:"));
            ui.add_enabled(st.through, egui::DragValue::new(&mut st.to).range(st.from..=pages)).on_hover_text("Last page to delete");
            ui.add_enabled(st.through, egui::Label::new("Inclusive"));
        });
        st.to = st.to.max(st.from);
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    cancel = button(ui, "Cancel", true);
                    ui.add_space(4.0);
                    ok = ok_button(ui, "OK", true);
                });
            });
        });
    });
    if ok {
        let (a, b) = (st.from.clamp(1, pages) - 1, if st.through { st.to.clamp(st.from, pages) - 1 } else { st.from.clamp(1, pages) - 1 });
        if b - a + 1 >= pages {
            app.flash("A document must have at least one page.");
        } else if let Some(d) = app.doc_mut() {
            d.modify("Delete Page", |doc, sel| {
                let m = doc.take_master_layers();
                doc.pages.drain(a..=b);
                doc.restore_master_layers(m);
                sel.clear();
            });
            d.page = a.min(d.doc.pages.len() - 1);
            d.needs_fit = true;
        }
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.delete_page = Some(st);
    }
}

pub fn rename_page(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some((i, mut name)) = app.dialogs.rename_page.take() else { return };
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "rename_page", "Rename Page", vec2(190.0, 185.0), |ui| {
        ui.add_space(10.0);
        ui.label("Page name:");
        ui.add_space(6.0);
        let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(ui.available_width()));
        if !r.has_focus() && !r.lost_focus() {
            r.request_focus();
        }
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            ok = true;
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    cancel = button(ui, "Cancel", true);
                    ui.add_space(4.0);
                    ok |= ok_button(ui, "OK", true);
                });
            });
        });
    });
    if ok {
        if let Some(d) = app.doc_mut() {
            if i < d.doc.pages.len() {
                d.modify("Rename Page", |doc, _| doc.pages[i].name = name.clone());
            }
        }
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.rename_page = Some((i, name));
    }
}

pub fn goto_page(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut n) = app.dialogs.goto_page.take() else { return };
    let total = app.doc().map(|d| d.doc.pages.len()).unwrap_or(1);
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "goto_page", "Go To Page", vec2(190.0, 180.0), |ui| {
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.label("Go to page:");
            ui.add_space(8.0);
            ui.add_sized(vec2(64.0, 22.0), egui::DragValue::new(&mut n).range(1..=total));
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    cancel = button(ui, "Cancel", true);
                    ui.add_space(4.0);
                    ok = ok_button(ui, "OK", true);
                });
            });
        });
    });
    if ok {
        if let Some(d) = app.doc_mut() {
            d.page = (n.max(1) - 1).min(total - 1);
            d.selection.clear();
            d.needs_fit = true;
        }
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.goto_page = Some(n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grommets_and_banner_size() {
        let mut o = BorderGrommet { border: true, grommets: true, ..Default::default() };
        let art = kurbo::Rect::new(0.0, 0.0, 612.0, 792.0);
        assert_eq!(o.grommet_points(art).len(), 4);
        o.placement = [true, true, false, false, false];
        o.horizontal = 3;
        // the top row adds its middle grommet; its ends are the corners
        assert_eq!(o.grommet_points(art).len(), 5);
        let src = Document::new(612.0, 792.0);
        let doc = border_and_grommet(&src, 0, &[], &o);
        assert_eq!((doc.pages[0].width, doc.pages[0].height), (612.0 + 180.0, 792.0 + 180.0));
        assert_eq!(doc.pages[0].layers.last().unwrap().objects.len(), 5);
        assert_eq!(page_label(0, 1, 1, 1), "I");
        assert_eq!(page_label(3, 1, 1, 2), "iv");
        assert_eq!(page_label(27, 1, 1, 3), "AB");
        assert_eq!(page_label(0, 1, 2, 0), "");
    }

    fn page_label(i: usize, start: u32, from: u32, style: u8) -> String {
        vector_core::document::page_number_label(i, start, from, style)
    }
}
