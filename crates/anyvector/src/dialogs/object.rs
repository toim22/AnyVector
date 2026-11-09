//! The Object menu: Create ▸ Arrowhead and Pattern Fill, Insert ▸ QR Code, Barcode and
//! Validate Barcode, Symbol ▸ Edit Master, Break Link, Update from Link and Export Library, and
//! Rollover.

use super::window;
use crate::app::AnyVectorApp;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect as ERect, Sense, Stroke, Ui};
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use std::sync::Arc;
use vector_core::codes::{self, BarcodeKind, Special};
use vector_core::document::{ImageData, NamedPattern, SymbolDef};
use vector_core::path::NodePath;
use vector_core::style::{ArrowHead, CustomArrow, PatternFill, PatternKind};
use vector_core::{Color, Document, Fill, ObjId, ObjKind, Object, Outline, Style};

/// Every leaf of an object as one path, in page coordinates.
fn outline_of(o: &Object) -> BezPath {
    let mut bp = BezPath::new();
    o.walk(&mut |x| {
        if !matches!(x.kind, ObjKind::Group(_) | ObjKind::PowerClip { .. } | ObjKind::Blend(_) | ObjKind::Symbol(_)) {
            for el in x.world_path().elements() {
                bp.push(*el);
            }
        }
    });
    bp
}

fn unique_id() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) | 1
}

// ------------------------------------------------------------------------------------------
// Create ▸ Arrowhead
// ------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct ArrowheadState {
    pub name: String,
    /// Points.
    pub length: f64,
    pub width: f64,
    pub proportional: bool,
    /// The shape: tip at (0, 0), pointing along +x, one unit long and `ratio` wide.
    pub path: BezPath,
    pub ratio: f64,
    pub filled: bool,
}

/// The arrowheads to choose from: the built-in ones, then those made with Create Arrowhead.
pub fn arrow_choices(app: &AnyVectorApp) -> Vec<(ArrowHead, String)> {
    let mut v: Vec<(ArrowHead, String)> = ArrowHead::ALL.iter().map(|a| (*a, a.name().to_string())).collect();
    let mut seen = vec![];
    let doc = app.doc().map(|d| d.doc.arrowheads.clone()).unwrap_or_default();
    for c in app.settings.custom_arrowheads.iter().chain(doc.iter()) {
        if !seen.contains(&c.id) {
            seen.push(c.id);
            v.push((ArrowHead::Custom(c.id), c.name.clone()));
        }
    }
    v
}

pub fn arrow_name(app: &AnyVectorApp, a: ArrowHead) -> String {
    arrow_choices(app).into_iter().find(|(x, _)| *x == a).map(|(_, n)| n).unwrap_or_else(|| a.name().to_string())
}

/// Before a custom arrowhead is applied, copy it into the document so it travels with it.
pub fn ensure_arrow(app: &mut AnyVectorApp, a: Option<ArrowHead>) {
    let Some(ArrowHead::Custom(id)) = a else { return };
    let Some(ca) = app.settings.custom_arrowheads.iter().find(|c| c.id == id).cloned() else { return };
    if let Some(d) = app.doc_mut() {
        if !d.doc.arrowheads.iter().any(|c| c.id == id) {
            d.doc.arrowheads.push(ca);
        }
    }
}

impl ArrowheadState {
    pub fn new(app: &AnyVectorApp) -> Option<ArrowheadState> {
        let d = app.doc()?;
        let [id] = d.selection.as_slice() else { return None };
        let o = d.doc.find(*id)?;
        let bp = outline_of(o);
        let b = bp.bounding_box();
        if b.width() <= 0.0 {
            return None;
        }
        let l = b.width();
        let path = Affine::scale(1.0 / l) * Affine::translate((-b.x1, -b.center().y)) * bp;
        let count = ArrowHead::ALL.len() + app.settings.custom_arrowheads.len() + 1;
        Some(ArrowheadState { name: format!("Arrowhead {count}"), length: l, width: b.height(), proportional: true, path, ratio: b.height() / l, filled: o.style.fill.main_color().is_some() || o.is_closed_shape() })
    }
    pub(crate) fn arrow(&self) -> CustomArrow {
        let w = self.width / self.length.max(1e-6);
        let p = Affine::scale_non_uniform(1.0, w / self.ratio.max(1e-6)) * self.path.clone();
        CustomArrow { id: unique_id(), name: self.name.trim().to_string(), path: p.to_svg(), width: w, filled: self.filled }
    }
}

fn arrow_preview(ui: &mut Ui, st: &ArrowheadState) {
    let (r, _) = ui.allocate_exact_size(vec2(200.0, 200.0), Sense::hover());
    let p = ui.painter();
    p.rect_stroke(r, 0.0, Stroke::new(1.0, theme::TEXT), egui::StrokeKind::Inside);
    let w = st.width / st.length.max(1e-6);
    let shape = Affine::scale_non_uniform(1.0, w / st.ratio.max(1e-6)) * st.path.clone();
    // fit the arrowhead in the right part of the box, a line stub coming from the left
    let s = (66.0f64).min(150.0 / w.max(1e-6)).max(1.0);
    let tip = Point::new(r.right() as f64 - 66.0, r.center().y as f64);
    let line_y = tip.y as f32;
    let back = (tip.x - s) as f32;
    let fill = Color32::from_rgb(0xcf, 0xd2, 0xd8);
    p.rect_filled(ERect::from_min_max(pos2(r.left() + 1.0, line_y - 5.0), pos2(back + 2.0, line_y + 5.0)), 0.0, fill);
    let a = Affine::translate(tip.to_vec2()) * Affine::scale(s) * shape;
    let mut polys: Vec<Vec<egui::Pos2>> = vec![];
    kurbo::flatten(a.elements().iter().copied(), 0.5, |el| match el {
        kurbo::PathEl::MoveTo(q) => polys.push(vec![pos2(q.x as f32, q.y as f32)]),
        kurbo::PathEl::LineTo(q) => {
            if let Some(v) = polys.last_mut() {
                v.push(pos2(q.x as f32, q.y as f32));
            }
        }
        _ => {}
    });
    for poly in polys {
        if poly.len() >= 3 && st.filled {
            p.add(egui::Shape::Path(egui::epaint::PathShape::convex_polygon(poly, fill, Stroke::NONE)));
        } else if poly.len() >= 2 {
            p.add(egui::Shape::line(poly, Stroke::new(2.0, fill)));
        }
    }
}

pub fn arrowhead(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.arrowhead.take() else { return };
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "create_arrowhead", "Create Arrowhead", vec2(516.0, 367.0), |ui| {
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(232.0);
                ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(218.0));
                ui.add_space(18.0);
                super::layout::rule_heading(ui, "Size");
                let ratio = st.width / st.length.max(1e-6);
                let mut changed_len = false;
                let mut changed_w = false;
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(80.0, 22.0), egui::Label::new("Length:").halign(egui::Align::Max));
                    changed_len = ui.add_sized(vec2(70.0, 22.0), egui::DragValue::new(&mut st.length).range(1.0..=10000.0).fixed_decimals(1)).changed();
                    ui.label("pt");
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(80.0, 22.0), egui::Label::new("Width:").halign(egui::Align::Max));
                    changed_w = ui.add_sized(vec2(70.0, 22.0), egui::DragValue::new(&mut st.width).range(1.0..=10000.0).fixed_decimals(1)).changed();
                    ui.label("pt");
                });
                if st.proportional {
                    if changed_len {
                        st.width = st.length * ratio;
                    } else if changed_w {
                        st.length = st.width / ratio.max(1e-6);
                    }
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(40.0);
                    ui.checkbox(&mut st.proportional, "Proportional");
                });
            });
            ui.add_space(12.0);
            arrow_preview(ui, &st);
        });
        ui.add_space(16.0);
        super::separator(ui);
        ui.add_space(8.0);
        super::table::info_box(ui, &["Creates an arrowhead from the selected object. The new arrowhead is added to the Arrowhead selectors.".to_string()]);
        (ok, cancel) = super::ok_cancel_plain(ui, !st.name.trim().is_empty());
    });
    if ok {
        let ca = st.arrow();
        app.settings.custom_arrowheads.push(ca.clone());
        if let Some(d) = app.doc_mut() {
            d.modify("Create Arrowhead", |doc, _| doc.arrowheads.push(ca));
        }
        app.flash(&format!("{} was added to the Arrowhead selectors.", st.name.trim()));
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.arrowhead = Some(st);
    }
}

// ------------------------------------------------------------------------------------------
// Create ▸ Pattern Fill
// ------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub struct PatternState {
    /// 0 vector, 1 bitmap, 2 two color.
    pub kind: u8,
    /// 0 low, 1 medium, 2 high.
    pub resolution: u8,
}

pub fn pattern(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.pattern else { return };
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "create_pattern", "Create Pattern", vec2(282.0, 258.0), |ui| {
        ui.add_space(4.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(132.0, 100.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                super::group(ui, "Type", |ui| {
                    ui.radio_value(&mut st.kind, 0, "Vector");
                    ui.radio_value(&mut st.kind, 1, "Bitmap");
                    ui.radio_value(&mut st.kind, 2, "Two color");
                });
            });
            ui.add_space(8.0);
            ui.allocate_ui_with_layout(vec2(112.0, 100.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                super::group(ui, "Resolution", |ui| {
                    ui.add_enabled_ui(st.kind == 2, |ui| {
                        ui.radio_value(&mut st.resolution, 0, "Low");
                        ui.radio_value(&mut st.resolution, 1, "Medium");
                        ui.radio_value(&mut st.resolution, 2, "High");
                    });
                });
            });
        });
        ui.add_space(6.0);
        ui.add(egui::Label::new("After pressing OK, select pattern area with a marquee.").wrap());
        (ok, cancel) = super::ok_cancel_plain(ui, true);
    });
    app.dialogs.pattern = None;
    if ok {
        app.set_tool(crate::tools::ToolId::Pick);
        app.ts.pattern_area = Some((st.kind, st.resolution));
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.pattern = Some(st);
    }
}

/// Make the pattern from the marquee area (page coordinates) and add it to the document's
/// pattern fills. Vector patterns are kept as 600 dpi tiles.
pub fn make_pattern(doc: &mut Document, page: usize, area: Rect, kind: u8, resolution: u8) -> Option<String> {
    if area.width() < 1.0 || area.height() < 1.0 {
        return None;
    }
    let dpi = match (kind, resolution) {
        (0, _) => 600.0,
        (1, _) => 300.0,
        (_, 0) => 150.0,
        (_, 1) => 300.0,
        _ => 600.0,
    };
    let pm = vector_render::render_page_image(doc, page, dpi, None, Some(area), true)?;
    let mut rgba: Vec<u8> = Vec::with_capacity((pm.width() * pm.height() * 4) as usize);
    for p in pm.pixels() {
        let c = p.demultiply();
        if kind == 2 {
            // two color: dark ink becomes the foreground, the rest the background
            let lum = 0.299 * c.red() as f32 + 0.587 * c.green() as f32 + 0.114 * c.blue() as f32;
            let ink = c.alpha() > 127 && lum < 160.0;
            rgba.extend_from_slice(if ink { &[0, 0, 0, 255] } else { &[255, 255, 255, 255] });
        } else {
            rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
        }
    }
    let n = doc.patterns.len() + 1;
    let name = format!("Pattern {n}");
    let img = ImageData { width: pm.width(), height: pm.height(), rgba: Arc::new(rgba), encoded: Arc::new(vec![]), format: "png".into(), name: name.clone() };
    let iid = doc.add_image(img);
    let fill = PatternFill { kind: PatternKind::Checker, fg: Color::BLACK, bg: Color::WHITE, tile: area.width().max(area.height()), angle: 0.0, image: Some(iid), transform_with_object: true, vector: true, mirror: 0 };
    // the objects in the area, kept to edit the pattern later
    let mut source = vec![];
    if kind == 0 {
        for layer in &doc.pages[page].layers {
            for o in &layer.objects {
                if doc.bounds_of(o).intersect(area).area() > 0.0 {
                    let mut c = o.clone();
                    c.apply_transform(Affine::translate((-area.x0, -area.y0)));
                    source.push(c);
                }
            }
        }
    }
    doc.patterns.push(NamedPattern { name: name.clone(), fill, source });
    Some(name)
}

/// Properties ▸ Vector Pattern ▸ Edit fill: the pattern opens as its own document, one tile on
/// the page, in View ▸ Pattern Preview. Closing it puts the changes into the pattern.
pub fn edit_pattern(app: &mut AnyVectorApp, image: u64) {
    let Some(a) = app.active.filter(|a| *a < app.docs.len()) else { return };
    let group = app.docs[a].group;
    // already open: show it
    if let Some(i) = app.docs.iter().position(|d| d.pattern.as_ref().is_some_and(|l| l.image == image && l.group == group)) {
        app.active = Some(i);
        return;
    }
    let src = &app.docs[a];
    let Some(np) = src.doc.patterns.iter().find(|n| n.fill.image == Some(image)).cloned() else { return };
    let Some(img) = src.doc.images.get(&image).cloned() else { return };
    let big = img.width.max(img.height).max(1) as f64;
    let (w, h) = (np.fill.tile * img.width as f64 / big, np.fill.tile * img.height as f64 / big);
    let mut doc = Document::new(w.max(1.0), h.max(1.0));
    doc.settings.unit = src.doc.settings.unit;
    // the bitmaps of the source document, so the pattern's objects find theirs; new ids stay
    // clear of the ids the source document gives out meanwhile
    doc.images = src.doc.images.clone();
    doc.reserve_ids_below(src.doc.next_id_value() + 1_000_000);
    let objs = if np.source.is_empty() {
        // a pattern without its objects: the tile itself
        let mut st = Style::default();
        st.fill = Fill::None;
        st.outline = Outline::none();
        let id = doc.new_id();
        let mut o = Object::new(id, ObjKind::Image(vector_core::object::ImageObj { image, w, h, crop: [0.0, 0.0, 1.0, 1.0], adjust: Default::default() }), Affine::IDENTITY, st);
        o.name = np.name.clone();
        vec![o]
    } else {
        np.source.clone()
    };
    if let Some(l) = doc.pages[0].layers.first_mut() {
        l.objects = objs;
    }
    let mut dv = crate::docview::DocView::new(doc, np.name.clone(), app.settings.undo_levels);
    dv.pattern = Some(crate::docview::PatternLink { group, image });
    dv.view_mode = crate::docview::ViewMode::PatternPreview;
    app.push_doc(dv);
}

/// A pattern document closes: its page becomes the pattern tile (one undo step in the document
/// the pattern belongs to).
pub fn apply_pattern_doc(app: &mut AnyVectorApp, i: usize) {
    let Some(link) = app.docs.get(i).and_then(|d| d.pattern.clone()) else { return };
    if !app.docs[i].history.is_modified() {
        return;
    }
    let pd = &app.docs[i].doc;
    let area = pd.pages[0].rect();
    let Some(pm) = vector_render::render_page_image(pd, 0, 600.0, None, Some(area), true) else { return };
    let mut rgba: Vec<u8> = Vec::with_capacity((pm.width() * pm.height() * 4) as usize);
    for p in pm.pixels() {
        let c = p.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    let source: Vec<Object> = pd.pages[0].layers.iter().flat_map(|l| l.objects.iter().cloned()).collect();
    let images = pd.images.clone();
    let (pw, ph) = (pm.width(), pm.height());
    let Some(s) = app.docs.iter().position(|d| d.group == link.group && d.pattern.is_none()) else { return };
    let prev = app.active;
    app.active = Some(s);
    if let Some(d) = app.doc_mut() {
        d.modify("Edit Pattern", |doc, _| {
            for (id, im) in images {
                doc.images.entry(id).or_insert(im);
            }
            if let Some(old) = doc.images.get(&link.image).cloned() {
                doc.images.insert(link.image, ImageData { width: pw, height: ph, rgba: Arc::new(rgba), encoded: Arc::new(vec![]), ..old });
            }
            if let Some(np) = doc.patterns.iter_mut().find(|n| n.fill.image == Some(link.image)) {
                np.source = source;
            }
        });
        d.cache.key = 0;
    }
    app.sync_windows();
    app.active = prev;
}

// ------------------------------------------------------------------------------------------
// Insert ▸ QR Code, Barcode, Validate Barcode
// ------------------------------------------------------------------------------------------

/// A QR code object: a black path of modules, 1.5" square.
pub fn qr_object(doc: &mut Document, text: &str, at: Point, size: f64) -> Option<Object> {
    let bp = codes::qr_path(text, size)?;
    let np = NodePath::from_bez(&bp);
    let mut st = Style::default();
    st.fill = Fill::Solid(Color::BLACK);
    st.outline = Outline::none();
    let mut o = Object::new(doc.new_id(), ObjKind::Path(np), Affine::translate((at.x - size / 2.0, at.y - size / 2.0)), st);
    o.name = "QR Code".into();
    o.special = Some(Special::QrCode { text: text.to_string() });
    Some(o)
}

/// Re-make a QR code after its text changed, keeping its place and size.
pub fn set_qr_text(doc: &mut Document, id: ObjId, text: &str) {
    let Some(o) = doc.find(id) else { return };
    let b = o.bounds();
    let Some(bp) = codes::qr_path(text, b.width().max(b.height()).max(1.0)) else { return };
    if let Some(o) = doc.find_mut(id) {
        o.kind = ObjKind::Path(NodePath::from_bez(&bp));
        o.transform = Affine::translate((b.x0, b.y0));
        o.special = Some(Special::QrCode { text: text.to_string() });
    }
}

pub fn insert_qr(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    let c = d.page_ref().rect().center();
    let Some(o) = qr_object(&mut d.doc, "https://", c, 108.0) else { return };
    d.add_objects("Insert QR Code", vec![o]);
    app.dockers.show(crate::dockers::DockerId::Properties);
}

#[derive(Clone)]
pub struct BarcodeState {
    pub kind: usize,
    pub data: String,
    /// Bar height and module width, points.
    pub height: f64,
    pub module: f64,
    pub show_text: bool,
}

/// A barcode object: a group of the bars and, when asked, the text under them.
pub fn barcode_object(doc: &mut Document, st: &BarcodeState, at: Point) -> Result<Object, String> {
    let kind = BarcodeKind::ALL[st.kind.min(3)];
    let text = codes::validate(kind, &st.data)?;
    let n = codes::modules(kind, &text).len() as f64;
    let w = n * st.module;
    let bars = codes::barcode_path(kind, &text, w, st.height);
    let mut bs = Style::default();
    bs.fill = Fill::Solid(Color::BLACK);
    bs.outline = Outline::none();
    let mut kids = vec![Object::new(doc.new_id(), ObjKind::Path(NodePath::from_bez(&bars)), Affine::IDENTITY, bs)];
    if st.show_text {
        let size = (st.height * 0.18).clamp(6.0, 24.0);
        let mut t = vector_core::text::TextObj::artistic(&text, vector_core::text::CharFormat { size, ..Default::default() });
        t.align = vector_core::text::TextAlign::Center;
        kids.push(Object::new(doc.new_id(), ObjKind::Text(t), Affine::translate((w / 2.0, st.height + size * 1.1)), Style::text_default()));
    }
    for k in kids.iter_mut() {
        k.apply_transform(Affine::translate((at.x - w / 2.0, at.y - st.height / 2.0)));
    }
    let mut g = Object::new(doc.new_id(), ObjKind::Group(kids), Affine::IDENTITY, Style::default());
    g.name = format!("Barcode ({})", kind.name());
    g.special = Some(Special::Barcode { kind, data: text });
    Ok(g)
}

pub fn barcode(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.barcode.take() else { return };
    let (unit, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((vector_core::Unit::Inches, 300.0));
    let names: Vec<&str> = BarcodeKind::ALL.iter().map(|k| k.name()).collect();
    let kind = BarcodeKind::ALL[st.kind.min(3)];
    let check = codes::validate(kind, &st.data);
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "barcode", "Barcode Wizard", vec2(460.0, 330.0), |ui| {
        ui.add_space(6.0);
        ui.label("Select from one of the following industry standard formats:");
        super::combo(ui, "bc_kind", &mut st.kind, &names, 200.0);
        ui.add_space(8.0);
        ui.label("Enter keyword data:");
        ui.add(egui::TextEdit::singleline(&mut st.data).desired_width(300.0));
        ui.label(egui::RichText::new(kind.rule()).color(theme::TEXT_DIM).small());
        ui.add_space(4.0);
        match &check {
            Ok(t) => ui.label(egui::RichText::new(format!("Encoded: {t}")).color(theme::TEXT_DIM)),
            Err(e) => ui.label(egui::RichText::new(e).color(Color32::from_rgb(0xe8, 0x6a, 0x6a))),
        };
        ui.add_space(8.0);
        egui::Grid::new("bc_grid").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
            ui.label("Bar height:");
            super::layout::length(ui, &mut st.height, unit, dpi, true);
            ui.end_row();
            ui.label("Bar width (narrow):");
            super::layout::length(ui, &mut st.module, unit, dpi, true);
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut st.show_text, "Show the human-readable text");
            ui.end_row();
        });
        let (o, c) = super::ok_cancel_row(ui, "Finish", check.is_ok());
        ok = o;
        cancel = c;
    });
    if ok {
        if let Some(d) = app.doc_mut() {
            let c = d.page_ref().rect().center();
            match barcode_object(&mut d.doc, &st, c) {
                Ok(o) => d.add_objects("Insert Barcode", vec![o]),
                Err(e) => app.flash(&e),
            }
        }
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.barcode = Some(st);
    }
}

/// Object ▸ Insert ▸ Validate Barcode: check the selected barcode's data and check digit.
pub fn validate_barcode(app: &mut AnyVectorApp) {
    let Some(d) = app.doc() else { return };
    let found = d.selection.iter().filter_map(|id| d.doc.find(*id)).find_map(|o| match &o.special {
        Some(Special::Barcode { kind, data }) => Some((*kind, data.clone())),
        _ => None,
    });
    let msg = match found {
        None => ("Validate Barcode".to_string(), "Select a barcode made with Object ▸ Insert ▸ Barcode.".to_string()),
        Some((kind, data)) => match codes::validate(kind, &data) {
            Ok(_) => ("Validate Barcode".into(), format!("The {} barcode \u{201c}{data}\u{201d} is valid.", kind.name())),
            Err(e) => ("Validate Barcode".into(), format!("The {} barcode \u{201c}{data}\u{201d} is not valid. {e}", kind.name())),
        },
    };
    app.dialogs.message_box = Some(msg);
}

// ------------------------------------------------------------------------------------------
// Symbols
// ------------------------------------------------------------------------------------------

/// Symbol ▸ Edit Master: the symbol's objects are placed on the page where the instance is,
/// the rest of the drawing is dimmed, and Finish puts them back into the symbol.
#[derive(Clone, Debug)]
pub struct SymbolEdit {
    pub symbol: u64,
    pub instance: ObjId,
    pub transform: Affine,
    pub ids: Vec<ObjId>,
}

pub fn selected_symbol(app: &AnyVectorApp) -> Option<(ObjId, u64)> {
    let d = app.doc()?;
    let [id] = d.selection.as_slice() else { return None };
    match d.doc.find(*id)?.kind {
        ObjKind::Symbol(s) => Some((*id, s)),
        _ => None,
    }
}

pub fn edit_master(app: &mut AnyVectorApp) {
    let Some((inst, sid)) = selected_symbol(app) else { return };
    let Some(d) = app.doc_mut() else { return };
    if d.symbol_edit.is_some() {
        return;
    }
    let Some(def) = d.doc.symbols.iter().find(|s| s.id == sid).cloned() else { return };
    let Some(tr) = d.doc.find(inst).map(|o| o.transform) else { return };
    let (page, layer) = d.active_loc();
    let mut ids = vec![];
    d.modify("Edit Symbol", |doc, sel| {
        if let Some(o) = doc.find_mut(inst) {
            o.visible = false;
        }
        let l = layer.min(doc.pages[page].layers.len().saturating_sub(1));
        for mut o in def.objects.clone() {
            doc.reassign_ids(&mut o);
            o.apply_transform(tr);
            ids.push(o.id);
            doc.pages[page].layers[l].objects.push(o);
        }
        sel.clear();
    });
    d.focus = ids.clone();
    d.cache.key = 0;
    d.symbol_edit = Some(SymbolEdit { symbol: sid, instance: inst, transform: tr, ids });
}

pub fn finish_symbol(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    let Some(se) = d.symbol_edit.take() else { return };
    let focus = std::mem::take(&mut d.focus);
    d.modify("Finish Editing Symbol", |doc, sel| {
        // the objects being edited, plus any drawn while editing
        let ids: Vec<ObjId> = se.ids.iter().chain(focus.iter()).copied().filter(|id| doc.find(*id).is_some()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        let inv = se.transform.inverse();
        // keep the stacking order: find every object before removing any
        let mut objs: Vec<_> = ids.iter().filter_map(|id| doc.find(*id).cloned().map(|o| (doc.locate(*id), o))).collect();
        for id in &ids {
            doc.remove(*id);
        }
        objs.sort_by_key(|(at, _)| *at);
        if !objs.is_empty() {
            if let Some(def) = doc.symbols.iter_mut().find(|s| s.id == se.symbol) {
                def.objects = objs
                    .into_iter()
                    .map(|(_, mut o)| {
                        o.apply_transform(inv);
                        o
                    })
                    .collect();
            }
        }
        if let Some(o) = doc.find_mut(se.instance) {
            o.visible = true;
        }
        *sel = vec![se.instance];
    });
    d.cache.key = 0;
}

/// The symbol library format of Export Library: the symbols and the images they use.
#[derive(serde::Serialize, serde::Deserialize)]
struct SymbolLibrary {
    symbols: Vec<SymbolDef>,
}

pub fn export_library(app: &mut AnyVectorApp) {
    let Some(d) = app.doc() else { return };
    if d.doc.symbols.is_empty() {
        app.flash("This document has no symbols.");
        return;
    }
    let Some(path) = rfd::FileDialog::new().set_title("Export Library").add_filter("CSL - Symbol Library (*.csl)", &["csl"]).set_directory(symbols_dir()).set_file_name(format!("{}.csl", d.stem())).save_file() else { return };
    let lib = SymbolLibrary { symbols: d.doc.symbols.iter().map(|s| SymbolDef { link: None, ..s.clone() }).collect() };
    match serde_json::to_vec_pretty(&lib).map_err(|e| e.to_string()).and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string())) {
        Ok(()) => app.flash(&format!("Exported {} symbols to {}.", lib.symbols.len(), path.display())),
        Err(e) => app.dialogs.message_box = Some(("Export Library".into(), format!("Could not write the library: {e}"))),
    }
}

/// Import Library: the symbols of a library, linked to it.
pub fn import_library(app: &mut AnyVectorApp) {
    let Some(path) = rfd::FileDialog::new().set_title("Import Library").add_filter("CSL - Symbol Library (*.csl)", &["csl"]).set_directory(symbols_dir()).pick_file() else { return };
    let link = path.to_string_lossy().to_string();
    match read_library(&link) {
        Ok(syms) => {
            if let Some(d) = app.doc_mut() {
                d.modify("Import Library", |doc, _| {
                    for s in syms {
                        if doc.symbols.iter().any(|x| x.name == s.name && x.link.as_deref() == Some(link.as_str())) {
                            continue;
                        }
                        add_linked_symbol(doc, &s, &link);
                    }
                });
            }
        }
        Err(e) => app.dialogs.message_box = Some(("Import Library".into(), format!("Could not read {link}: {e}"))),
    }
}

/// Add a library's symbol to the document, linked to the library; gives its id.
pub fn add_linked_symbol(doc: &mut vector_core::Document, s: &SymbolDef, link: &str) -> u64 {
    let mut objs = s.objects.clone();
    for o in objs.iter_mut() {
        doc.reassign_ids(o);
    }
    let id = doc.new_id();
    doc.symbols.push(SymbolDef { id, name: s.name.clone(), objects: objs, link: Some(link.to_string()) });
    id
}

/// The folder of the local symbol libraries (%APPDATA%\AnyVector\Symbols).
pub fn symbols_dir() -> std::path::PathBuf {
    let d = crate::settings::config_dir().join("Symbols");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Read a symbol library written by Export Library.
pub fn read_library(path: &str) -> Result<Vec<SymbolDef>, String> {
    let b = std::fs::read(path).map_err(|e| e.to_string())?;
    let lib: SymbolLibrary = serde_json::from_slice(&b).map_err(|e| e.to_string())?;
    Ok(lib.symbols)
}

pub fn break_link(app: &mut AnyVectorApp) {
    let Some((_, sid)) = selected_symbol(app) else { return };
    if let Some(d) = app.doc_mut() {
        d.modify("Break Link", |doc, _| {
            if let Some(s) = doc.symbols.iter_mut().find(|s| s.id == sid) {
                s.link = None;
            }
        });
    }
}

pub fn update_from_link(app: &mut AnyVectorApp) {
    let Some((_, sid)) = selected_symbol(app) else { return };
    let Some(d) = app.doc() else { return };
    let Some(def) = d.doc.symbols.iter().find(|s| s.id == sid).cloned() else { return };
    let Some(link) = def.link.clone() else { return };
    let src = match read_library(&link) {
        Ok(v) => v,
        Err(e) => {
            app.dialogs.message_box = Some(("Update from Link".into(), format!("Could not read {link}: {e}")));
            return;
        }
    };
    let Some(s) = src.into_iter().find(|s| s.name == def.name) else {
        app.dialogs.message_box = Some(("Update from Link".into(), format!("The library {link} has no symbol named \u{201c}{}\u{201d}.", def.name)));
        return;
    };
    if let Some(d) = app.doc_mut() {
        d.modify("Update from Link", |doc, _| {
            let mut objs = s.objects.clone();
            for o in objs.iter_mut() {
                doc.reassign_ids(o);
            }
            if let Some(x) = doc.symbols.iter_mut().find(|x| x.id == sid) {
                x.objects = objs;
            }
        });
        d.cache.key = 0;
    }
}

pub fn symbol_linked(app: &AnyVectorApp) -> bool {
    let Some((_, sid)) = selected_symbol(app) else { return false };
    app.doc().is_some_and(|d| d.doc.symbols.iter().any(|s| s.id == sid && s.link.is_some()))
}

// ------------------------------------------------------------------------------------------
// Rollovers
// ------------------------------------------------------------------------------------------

pub const ROLLOVER_STATES: [&str; 3] = ["Normal", "Over", "Down"];

pub fn is_rollover(o: &Object) -> bool {
    matches!(o.special, Some(Special::Rollover { .. }))
}

pub fn rollover_state(o: &Object) -> u8 {
    match o.special {
        Some(Special::Rollover { state }) => state,
        _ => 0,
    }
}

/// Show one state of a rollover.
pub fn set_rollover_state(o: &mut Object, state: u8) {
    if !is_rollover(o) {
        return;
    }
    o.special = Some(Special::Rollover { state });
    if let ObjKind::Group(kids) = &mut o.kind {
        for (i, k) in kids.iter_mut().enumerate() {
            k.visible = i == state as usize;
        }
    }
}

/// Object ▸ Rollover ▸ Create Rollover: the selection becomes the Normal state, with copies
/// for Over and Down.
pub fn create_rollover(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    if d.selection.is_empty() {
        return;
    }
    d.modify("Create Rollover", |doc, sel| {
        let mut objs = vec![];
        let mut at = None;
        for id in sel.iter() {
            if let Some(o) = doc.find(*id).cloned() {
                at = at.or(doc.locate(*id));
                objs.push(o);
            }
        }
        for id in sel.iter() {
            doc.remove(*id);
        }
        if objs.is_empty() {
            return;
        }
        let mut states = vec![];
        for (i, name) in ROLLOVER_STATES.iter().enumerate() {
            let mut kids = objs.clone();
            if i > 0 {
                for k in kids.iter_mut() {
                    doc.reassign_ids(k);
                }
            }
            let mut g = Object::new(doc.new_id(), ObjKind::Group(kids), Affine::IDENTITY, Style::default());
            g.name = name.to_string();
            g.visible = i == 0;
            states.push(g);
        }
        let mut r = Object::new(doc.new_id(), ObjKind::Group(states), Affine::IDENTITY, Style::default());
        r.name = "Rollover".into();
        r.special = Some(Special::Rollover { state: 0 });
        let id = r.id;
        let (p, l, i) = at.unwrap_or((0, 0, 0));
        let objs = &mut doc.pages[p].layers[l].objects;
        objs.insert(i.min(objs.len()), r);
        *sel = vec![id];
    });
}

/// Rollover ▸ Extract Rollover Objects: every state becomes a group of its own.
pub fn extract_rollover(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    d.modify("Extract Rollover Objects", |doc, sel| {
        let ids = sel.clone();
        let mut out = vec![];
        for id in ids {
            let Some(o) = doc.find(id).cloned() else { continue };
            if !is_rollover(&o) {
                continue;
            }
            let Some((p, l, i)) = doc.locate(id) else { continue };
            let ObjKind::Group(states) = o.kind else { continue };
            doc.remove(id);
            for (k, mut s) in states.into_iter().enumerate() {
                s.visible = true;
                s.apply_transform(o.transform);
                out.push(s.id);
                let objs = &mut doc.pages[p].layers[l].objects;
                objs.insert((i + k).min(objs.len()), s);
            }
        }
        *sel = out;
    });
}

pub fn selected_rollover(app: &AnyVectorApp) -> Option<ObjId> {
    let d = app.doc()?;
    if let Some(id) = d.rollover_edit {
        return Some(id);
    }
    let [id] = d.selection.as_slice() else { return None };
    d.doc.find(*id).filter(|o| is_rollover(o)).map(|o| o.id)
}

/// The state the selected (or edited) rollover shows.
pub fn active_state(app: &AnyVectorApp) -> Option<u8> {
    let id = selected_rollover(app)?;
    app.doc()?.doc.find(id).map(rollover_state)
}

pub fn edit_rollover(app: &mut AnyVectorApp) {
    let Some(id) = selected_rollover(app) else { return };
    if !app.settings.extra_toolbars.iter().any(|t| t == "Internet") {
        app.settings.extra_toolbars.push("Internet".into());
    }
    let Some(d) = app.doc_mut() else { return };
    d.rollover_edit = Some(id);
    d.focus = vec![id];
    d.selection.clear();
    d.cache.key = 0;
}

pub fn finish_rollover(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    let Some(id) = d.rollover_edit.take() else { return };
    d.focus.clear();
    d.modify("Finish Editing Rollover", |doc, sel| {
        if let Some(o) = doc.find_mut(id) {
            set_rollover_state(o, 0);
        }
        *sel = vec![id];
    });
    d.cache.key = 0;
}

/// The Internet toolbar's Active Rollover State.
pub fn show_state(app: &mut AnyVectorApp, state: u8) {
    let Some(id) = selected_rollover(app) else { return };
    let Some(d) = app.doc_mut() else { return };
    d.modify("Rollover State", |doc, _| {
        if let Some(o) = doc.find_mut(id) {
            set_rollover_state(o, state);
        }
    });
}

/// Duplicate Rollover State copies the Normal state into the active one; Delete Rollover
/// State empties the active one.
pub fn state_contents(app: &mut AnyVectorApp, duplicate: bool) {
    let Some(id) = selected_rollover(app) else { return };
    let Some(d) = app.doc_mut() else { return };
    d.modify(if duplicate { "Duplicate Rollover State" } else { "Delete Rollover State" }, |doc, _| {
        let Some(o) = doc.find(id).cloned() else { return };
        let st = rollover_state(&o) as usize;
        let ObjKind::Group(states) = &o.kind else { return };
        let mut normal = match &states.first().map(|s| &s.kind) {
            Some(ObjKind::Group(k)) => k.clone(),
            _ => vec![],
        };
        for k in normal.iter_mut() {
            doc.reassign_ids(k);
        }
        if let Some(ObjKind::Group(states)) = doc.find_mut(id).map(|o| &mut o.kind) {
            if let Some(ObjKind::Group(kids)) = states.get_mut(st).map(|s| &mut s.kind) {
                if duplicate {
                    if st != 0 {
                        *kids = normal;
                    }
                } else {
                    kids.clear();
                }
            }
        }
    });
}

/// Rollover Live Preview: the rollover under the pointer shows Over, or Down while pressed.
pub fn live_preview(app: &mut AnyVectorApp, pos: Option<Point>, down: bool) {
    let Some(ai) = app.active.filter(|a| *a < app.docs.len()) else { return };
    let d = &mut app.docs[ai];
    let hot = pos.and_then(|p| {
        let pg = &d.doc.pages[d.page];
        pg.layers.iter().filter(|l| l.visible).flat_map(|l| l.objects.iter()).rev().find(|o| is_rollover(o) && d.doc.bounds_of(o).contains(p)).map(|o| o.id)
    });
    let want = hot.map(|h| (h, if down { 2 } else { 1 }));
    if want == app.rollover_hot {
        return;
    }
    // shown without an undo step: the document keeps its Normal states
    if let Some((old, _)) = app.rollover_hot.take() {
        if let Some(o) = d.doc.find_mut(old) {
            set_rollover_state(o, 0);
        }
    }
    if let Some((h, s)) = want {
        if let Some(o) = d.doc.find_mut(h) {
            set_rollover_state(o, s);
        }
    }
    app.rollover_hot = want;
    d.cache.key = 0;
}

pub fn stop_live_preview(app: &mut AnyVectorApp) {
    if let Some((old, _)) = app.rollover_hot.take() {
        if let Some(d) = app.doc_mut() {
            if let Some(o) = d.doc.find_mut(old) {
                set_rollover_state(o, 0);
            }
            d.cache.key = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollover_states_and_codes() {
        let mut doc = Document::new(612.0, 792.0);
        let st = BarcodeState { kind: 2, data: "400638133393".into(), height: 72.0, module: 1.0, show_text: true };
        let b = barcode_object(&mut doc, &st, Point::new(300.0, 300.0)).unwrap();
        assert!(matches!(&b.special, Some(Special::Barcode { data, .. }) if data == "4006381333931"));
        assert!((b.bounds().width() - 95.0).abs() < 1.0);
        let q = qr_object(&mut doc, "Vector", Point::new(100.0, 100.0), 108.0).unwrap();
        assert!((q.bounds().width() - 108.0).abs() < 0.01);
        let mut g = Object::new(1, ObjKind::Group(vec![Object::new(2, ObjKind::Group(vec![]), Affine::IDENTITY, Style::default()), Object::new(3, ObjKind::Group(vec![]), Affine::IDENTITY, Style::default()), Object::new(4, ObjKind::Group(vec![]), Affine::IDENTITY, Style::default())]), Affine::IDENTITY, Style::default());
        g.special = Some(Special::Rollover { state: 0 });
        set_rollover_state(&mut g, 2);
        let ObjKind::Group(k) = &g.kind else { panic!() };
        assert_eq!(k.iter().map(|x| x.visible).collect::<Vec<_>>(), vec![false, false, true]);
    }
}
