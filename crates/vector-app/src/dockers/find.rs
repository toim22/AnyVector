//! Find and Replace docker: Find Objects (by properties or by names and styles, with saved
//! queries), Replace Objects (colors, outline and text properties) and Find and Replace Text, over
//! a search range.

use super::{fold_header, no_doc, radio, spin};
use crate::app::VectorApp;
use crate::icons::Tone;
use crate::menus::{item, submenu};
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use serde::{Deserialize, Serialize};
use vector_core::{Color, Fill, ObjId, ObjKind, Object, Transparency};

pub const MODES: [&str; 3] = ["Find Objects", "Replace Objects", "Find and Replace Text"];
/// The saved queries.
pub const PRESETS: [&str; 7] = ["Custom", "Default", "Overprinted outlines", "Pantone fills", "Pantone outlines", "Scale with image off", "Text with RGB fills"];
const STYLE_TYPES: [&str; 8] = ["Bitmap Effects", "Character", "Fill", "Frame", "Outline", "Paragraph", "Style Set", "Transparency"];
const OBJECT_TYPES: [&str; 14] = ["Artistic Text", "Bitmap", "Callout", "Connector", "Curve", "Dimension", "Ellipse", "Group", "Paragraph Text", "Polygon", "PowerClip", "Rectangle", "Symbol", "Table"];
const FILLS: [&str; 8] = ["No Fill", "Uniform Fill", "Fountain Fill", "Vector Pattern", "Bitmap Pattern", "Two-color Pattern", "Mesh Fill", "Fill with RGB color"];
const OUTLINES: [&str; 5] = ["No Outline", "Outline", "Overprinted Outline", "Scale with image off", "Pantone outline"];
const EFFECTS: [&str; 12] = ["Bevel", "Bitmap Effects", "Blend", "Block Shadow", "Contour", "Distortion", "Drop Shadow", "Envelope", "Extrude", "Inner Shadow", "Lens", "Transparency"];

/// A query of Find Objects: an object type, a fill, an outline and a special effect; None is
/// "Undefined" (anything).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FindQuery {
    pub object_type: Option<String>,
    pub fill: Option<String>,
    pub outline: Option<String>,
    pub effect: Option<String>,
}

/// What the Find and Replace docker keeps between frames.
pub struct FrUi {
    pub mode: usize,
    pub preset: usize,
    pub by_names: bool,
    pub q: FindQuery,
    pub name_on: bool,
    pub name: String,
    pub name_case: bool,
    pub style_on: bool,
    pub style_by_type: bool,
    pub style_name: usize,
    pub style_type: usize,
    pub range_open: bool,
    /// 0 selection, 1 current page, 2 all pages, 3 the pages typed.
    pub range: usize,
    pub pages: String,
    /// The last matches and the one shown.
    pub found: Vec<(usize, ObjId)>,
    pub at: Option<usize>,
    /// Replace Objects: 0 color, 1 color model or palette, 2 outline properties, 3 text
    /// properties.
    pub rep: usize,
    pub find_color: Option<Color>,
    pub rep_color: Option<Color>,
    /// Outlines, uniform fills, fountain fills, two-color fills, mesh fills, monochrome bitmaps.
    pub apply_to: [bool; 6],
    pub ol_find: Option<f64>,
    pub ol_rep: Option<f64>,
    pub font_find: Option<String>,
    pub font_rep: Option<String>,
    pub size_find: Option<f64>,
    pub size_rep: Option<f64>,
    // Find and Replace Text
    pub text: String,
    pub replace_on: bool,
    pub rep_text: String,
    pub case: bool,
    pub whole: bool,
}

impl Default for FrUi {
    fn default() -> Self {
        FrUi {
            mode: 0,
            preset: 0,
            by_names: false,
            q: FindQuery::default(),
            name_on: false,
            name: String::new(),
            name_case: false,
            style_on: false,
            style_by_type: true,
            style_name: 0,
            style_type: 0,
            range_open: false,
            range: 1,
            pages: String::new(),
            found: vec![],
            at: None,
            rep: 0,
            find_color: Some(Color::BLACK),
            rep_color: Some(Color::WHITE),
            apply_to: [true; 6],
            ol_find: None,
            ol_rep: None,
            font_find: None,
            font_rep: None,
            size_find: None,
            size_rep: None,
            text: String::new(),
            replace_on: false,
            rep_text: String::new(),
            case: false,
            whole: false,
        }
    }
}

fn preset_query(i: usize) -> FindQuery {
    let s = |x: &str| Some(x.to_string());
    match i {
        2 => FindQuery { outline: s("Overprinted Outline"), ..Default::default() },
        3 => FindQuery { fill: s("Pantone fill"), ..Default::default() },
        4 => FindQuery { outline: s("Pantone outline"), ..Default::default() },
        5 => FindQuery { outline: s("Scale with image off"), ..Default::default() },
        6 => FindQuery { object_type: s("Artistic Text"), fill: s("Fill with RGB color"), ..Default::default() },
        _ => FindQuery::default(),
    }
}

fn type_of(o: &Object) -> &'static str {
    match &o.kind {
        ObjKind::Text(t) => {
            if matches!(t.kind, vector_core::text::TextKind::Paragraph { .. }) {
                "Paragraph Text"
            } else {
                "Artistic Text"
            }
        }
        ObjKind::Image(_) => "Bitmap",
        ObjKind::Rect(_) => "Rectangle",
        ObjKind::Ellipse(_) => "Ellipse",
        ObjKind::Polygon(_) => "Polygon",
        ObjKind::Group(_) => "Group",
        ObjKind::PowerClip { .. } => "PowerClip",
        ObjKind::Table(_) => "Table",
        ObjKind::Dimension(d) => {
            if d.kind == vector_core::object::DimensionKind::Callout {
                "Callout"
            } else {
                "Dimension"
            }
        }
        ObjKind::Connector(_) => "Connector",
        ObjKind::Symbol(_) => "Symbol",
        _ => "Curve",
    }
}

fn fill_of(o: &Object) -> &'static str {
    match &o.style.fill {
        Fill::None => "No Fill",
        Fill::Solid(_) => "Uniform Fill",
        Fill::Gradient(_) => "Fountain Fill",
        Fill::Mesh(_) => "Mesh Fill",
        Fill::Pattern(p) => {
            if p.vector {
                "Vector Pattern"
            } else if p.image.is_some() {
                "Bitmap Pattern"
            } else {
                "Two-color Pattern"
            }
        }
    }
}

fn effect_of(o: &Object, name: &str) -> bool {
    let e = &o.effects;
    match name {
        "Bevel" => e.bevel.is_some(),
        "Bitmap Effects" => !e.bitmap.is_empty(),
        "Blend" => matches!(o.kind, ObjKind::Blend(_)),
        "Block Shadow" => e.block_shadow.is_some(),
        "Contour" => e.contour.is_some(),
        "Distortion" => e.distort.is_some(),
        "Drop Shadow" => e.drop_shadow.is_some(),
        "Envelope" => e.envelope.is_some(),
        "Extrude" => e.extrude.is_some(),
        "Inner Shadow" => e.inner_shadow.is_some(),
        "Lens" => e.lens.is_some(),
        "Transparency" => !matches!(o.style.transparency, Transparency::None),
        _ => false,
    }
}

/// Does an object answer the query?
pub fn matches(o: &Object, q: &FindQuery) -> bool {
    if q == &FindQuery::default() {
        return false;
    }
    if let Some(t) = &q.object_type {
        if type_of(o) != t {
            return false;
        }
    }
    if let Some(f) = &q.fill {
        let ok = match f.as_str() {
            // colors keep no palette names
            "Pantone fill" => false,
            "Fill with RGB color" => matches!(o.style.fill, Fill::Solid(_)),
            f => fill_of(o) == f,
        };
        if !ok {
            return false;
        }
    }
    if let Some(ol) = &q.outline {
        let has = o.style.outline.color.is_some();
        let ok = match ol.as_str() {
            "No Outline" => !has,
            "Outline" => has,
            "Overprinted Outline" => has && o.style.overprint_outline,
            "Scale with image off" => has && !o.style.outline.scale_with_object,
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    if let Some(e) = &q.effect {
        if !effect_of(o, e) {
            return false;
        }
    }
    true
}

pub fn find_replace(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let mut st = std::mem::take(&mut app.dockers.ui.fr);
    body(app, ui, &mut st);
    app.dockers.ui.fr = st;
}

fn line(ui: &mut Ui) {
    ui.add_space(6.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, Color32::from_rgb(82, 82, 82)));
    ui.add_space(6.0);
}

/// A button `w` wide with its text centered.
fn wide(ui: &mut Ui, text: &str, enabled: bool, w: f32) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| ui.add_sized(vec2(w, 24.0), egui::Button::new(text))).inner
}

fn body(app: &mut VectorApp, ui: &mut Ui, st: &mut FrUi) {
    ui.add_space(2.0);
    let w = ui.available_width();
    if super::choice(ui, "fr_mode", &mut st.mode, &MODES, w) {
        st.found.clear();
        st.at = None;
    }
    ui.add_space(4.0);
    match st.mode {
        0 => find_objects(app, ui, st),
        1 => replace_objects(app, ui, st),
        _ => find_text(app, ui, st),
    }
    search_range(ui, st);
    line(ui);
    nav_buttons(app, ui, st);
}

fn find_objects(app: &mut VectorApp, ui: &mut Ui, st: &mut FrUi) {
    // the saved queries, open and save
    ui.horizontal(|ui| {
        let w = ui.available_width() - 62.0;
        if super::choice(ui, "fr_preset", &mut st.preset, &PRESETS, w) {
            st.q = preset_query(st.preset);
        }
        if super::icon_button(app, ui, "open", "Load a query", true) {
            if let Some(q) = rfd::FileDialog::new().set_title("Load Query").add_filter("Vector Find Query (*.vfind)", &["vfind"]).pick_file().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str::<FindQuery>(&s).ok()) {
                st.q = q;
                st.preset = 0;
            }
        }
        if super::icon_button(app, ui, "save", "Save the query", st.q != FindQuery::default()) {
            if let Some(p) = rfd::FileDialog::new().set_title("Save Query").add_filter("Vector Find Query (*.vfind)", &["vfind"]).set_file_name("Query.vfind").save_file() {
                let _ = std::fs::write(p, serde_json::to_string_pretty(&st.q).unwrap_or_default());
            }
        }
    });
    ui.add_space(10.0);
    ui.indent("fr_by", |ui| {
        if radio(ui, !st.by_names, "Properties", true) {
            st.by_names = false;
        }
        if radio(ui, st.by_names, "Names or Styles", true) {
            st.by_names = true;
        }
    });
    line(ui);
    if st.by_names {
        names_or_styles(app, ui, st);
        return;
    }
    ui.add_space(8.0);
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Search query").font(theme::bold(12.5)));
    ui.add_space(2.0);
    let rows = [("fr_type", &st.q.object_type), ("fill_icon", &st.q.fill), ("outline_icon", &st.q.outline), ("pr_fx", &st.q.effect)];
    for (icon, v) in rows {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
        if let Some(t) = app.icons.get(ui.ctx(), icon, 16.0, Tone::Normal) {
            ui.painter().image(t.id, Rect::from_center_size(pos2(r.left() + 10.0, r.center().y), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let (text, c) = match v {
            Some(s) => (s.as_str(), theme::TEXT),
            None => ("Undefined", theme::DISABLED),
        };
        ui.painter().text(pos2(r.left() + 25.0, r.center().y), egui::Align2::LEFT_CENTER, text, theme::body(12.5), c);
    }
    line(ui);
    ui.add_space(8.0);
    let w = ui.available_width();
    let add = wide(ui, "Add Query...", true, w);
    egui::Popup::menu(&add).show(|ui| {
        crate::menus::menu_style(ui);
        let mut set = |app: &mut VectorApp, ui: &mut Ui, title: &str, items: &[&str], k: usize| {
            submenu(app, ui, title, true, |app, ui| {
                for it in items {
                    if item(app, ui, it, true, false) {
                        let v = Some(it.to_string());
                        match k {
                            0 => st.q.object_type = v,
                            1 => st.q.fill = v,
                            2 => st.q.outline = v,
                            _ => st.q.effect = v,
                        }
                        st.preset = 0;
                    }
                }
            });
        };
        set(app, ui, "&Object Types", &OBJECT_TYPES, 0);
        set(app, ui, "&Fills", &FILLS, 1);
        set(app, ui, "Ou&tlines", &OUTLINES, 2);
        set(app, ui, "&Special Effects", &EFFECTS, 3);
    });
    ui.horizontal(|ui| {
        let w = (ui.available_width() - 4.0) / 2.0;
        let one = app.doc().and_then(|d| d.selection.first().and_then(|id| d.doc.find(*id)).cloned());
        if wide(ui, "From Selection", one.is_some(), w).clicked() {
            if let Some(o) = one {
                st.q = FindQuery {
                    object_type: Some(type_of(&o).into()),
                    fill: Some(fill_of(&o).into()),
                    outline: Some(if o.style.outline.color.is_some() { "Outline" } else { "No Outline" }.into()),
                    effect: EFFECTS.iter().find(|e| effect_of(&o, e)).map(|e| e.to_string()),
                };
                st.preset = 0;
            }
        }
        if wide(ui, "Reset", st.q != FindQuery::default(), w).clicked() {
            st.q = FindQuery::default();
            st.preset = 0;
        }
    });
}

fn names_or_styles(app: &mut VectorApp, ui: &mut Ui, st: &mut FrUi) {
    let styles: Vec<String> = app.doc().map(|d| d.doc.object_styles.iter().map(|s| s.name.clone()).collect()).unwrap_or_default();
    ui.checkbox(&mut st.name_on, "Object name:");
    ui.add_enabled_ui(st.name_on, |ui| {
        ui.indent("fr_name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(f32::INFINITY));
            ui.checkbox(&mut st.name_case, "Match case");
        });
    });
    ui.add_space(8.0);
    ui.checkbox(&mut st.style_on, "Style:");
    ui.add_enabled_ui(st.style_on, |ui| {
        ui.indent("fr_style", |ui| {
            ui.horizontal(|ui| {
                if radio(ui, !st.style_by_type, "Name", true) {
                    st.style_by_type = false;
                }
                ui.add_space(ui.available_width() - 140.0);
                let names: Vec<&str> = styles.iter().map(|s| s.as_str()).collect();
                ui.add_enabled_ui(!st.style_by_type && !names.is_empty(), |ui| {
                    super::choice(ui, "fr_style_name", &mut st.style_name, if names.is_empty() { &[""] } else { &names }, 140.0);
                });
            });
            ui.horizontal(|ui| {
                if radio(ui, st.style_by_type, "Type", true) {
                    st.style_by_type = true;
                }
                ui.add_space(ui.available_width() - 140.0);
                ui.add_enabled_ui(st.style_by_type, |ui| {
                    super::choice(ui, "fr_style_type", &mut st.style_type, &STYLE_TYPES, 140.0);
                });
            });
        });
    });
    ui.add_space(8.0);
}

fn replace_objects(app: &mut VectorApp, ui: &mut Ui, st: &mut FrUi) {
    ui.add_space(4.0);
    for (k, text) in ["Color", "Color model or palette", "Outline properties", "Text properties"].into_iter().enumerate() {
        if radio(ui, st.rep == k, text, k != 1) {
            st.rep = k;
        }
    }
    line(ui);
    match st.rep {
        0 => {
            egui::Grid::new("fr_colors").num_columns(2).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
                ui.label("Find:");
                super::color_pick_w(ui, "fr_find_color", &mut st.find_color, false, true, 120.0);
                ui.end_row();
                ui.label("Replace:");
                super::color_pick_w(ui, "fr_rep_color", &mut st.rep_color, false, true, 120.0);
                ui.end_row();
            });
            ui.add_space(6.0);
            ui.label("Apply to:");
            for (k, t) in ["Outlines", "Uniform fills", "Fountain fills", "2-color fills", "Mesh fills", "Monochrome bitmaps"].into_iter().enumerate() {
                ui.checkbox(&mut st.apply_to[k], t);
            }
        }
        2 => {
            opt_value(ui, "Outline width: find", &mut st.ol_find, 0.5, " pt");
            opt_value(ui, "Outline width: replace", &mut st.ol_rep, 1.0, " pt");
        }
        3 => {
            let fonts: Vec<String> = vector_core::text::fonts().lock().families().to_vec();
            opt_font(ui, "fr_font_find", "Font: find", &mut st.font_find, &fonts);
            opt_font(ui, "fr_font_rep", "Font: replace", &mut st.font_rep, &fonts);
            opt_value(ui, "Size: find", &mut st.size_find, 12.0, " pt");
            opt_value(ui, "Size: replace", &mut st.size_rep, 12.0, " pt");
        }
        _ => {}
    }
    let _ = app;
}

fn opt_value(ui: &mut Ui, label: &str, v: &mut Option<f64>, default: f64, suffix: &str) {
    ui.horizontal(|ui| {
        let mut on = v.is_some();
        if ui.checkbox(&mut on, label).changed() {
            *v = on.then_some(default);
        }
        if let Some(x) = v.as_mut() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                spin(ui, x, 0.0..=10000.0, 0.5, 3, suffix, 90.0);
            });
        }
    });
}

fn opt_font(ui: &mut Ui, id: &str, label: &str, v: &mut Option<String>, fonts: &[String]) {
    ui.horizontal(|ui| {
        let mut on = v.is_some();
        if ui.checkbox(&mut on, label).changed() {
            *v = on.then(|| "Arial".to_string());
        }
        if let Some(f) = v.as_mut() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt(id).width(130.0).selected_text(f.clone()).show_ui(ui, |ui| {
                    for n in fonts {
                        ui.selectable_value(f, n.clone(), n);
                    }
                });
            });
        }
    });
}

fn find_text(_app: &mut VectorApp, ui: &mut Ui, st: &mut FrUi) {
    ui.add_space(6.0);
    ui.label("Find:");
    ui.add(egui::TextEdit::singleline(&mut st.text).desired_width(f32::INFINITY));
    ui.checkbox(&mut st.case, "Match case");
    ui.checkbox(&mut st.whole, "Find whole words only");
    ui.add_space(4.0);
    ui.checkbox(&mut st.replace_on, "Replace:");
    ui.add_enabled(st.replace_on, egui::TextEdit::singleline(&mut st.rep_text).desired_width(f32::INFINITY));
}

fn search_range(ui: &mut Ui, st: &mut FrUi) {
    ui.add_space(8.0);
    fold_header(ui, "Search range", &mut st.range_open);
    if !st.range_open {
        return;
    }
    ui.indent("fr_range", |ui| {
        for (k, t) in ["Selection", "Current page", "All pages"].into_iter().enumerate() {
            if radio(ui, st.range == k, t, true) {
                st.range = k;
            }
        }
        ui.horizontal(|ui| {
            if radio(ui, st.range == 3, "Pages", true) {
                st.range = 3;
            }
            ui.add_enabled(st.range == 3, egui::TextEdit::singleline(&mut st.pages).desired_width(110.0));
        });
    });
}

/// The pages of the search range (0-based), the selection's page for Selection.
fn range_pages(app: &VectorApp, st: &FrUi) -> Vec<usize> {
    let Some(d) = app.doc() else { return vec![] };
    let n = d.doc.pages.len();
    match st.range {
        0 | 1 => vec![d.page],
        2 => (0..n).collect(),
        _ => {
            let mut v = vec![];
            for part in st.pages.split(',') {
                let mut it = part.trim().splitn(2, '-');
                let a = it.next().and_then(|x| x.trim().parse::<usize>().ok());
                let b = it.next().and_then(|x| x.trim().parse::<usize>().ok());
                if let Some(a) = a {
                    for p in a..=b.unwrap_or(a) {
                        if p >= 1 && p <= n && !v.contains(&(p - 1)) {
                            v.push(p - 1);
                        }
                    }
                }
            }
            v
        }
    }
}

fn word_hit(hay: &str, needle: &str, case: bool, whole: bool) -> bool {
    if needle.is_empty() {
        return false;
    }
    let (h, n) = if case { (hay.to_string(), needle.to_string()) } else { (hay.to_lowercase(), needle.to_lowercase()) };
    if !whole {
        return h.contains(&n);
    }
    h.match_indices(&n).any(|(i, _)| {
        let before = h[..i].chars().last().map(|c| !c.is_alphanumeric()).unwrap_or(true);
        let after = h[i + n.len()..].chars().next().map(|c| !c.is_alphanumeric()).unwrap_or(true);
        before && after
    })
}

/// The objects of the search range that match, in page order.
fn search(app: &VectorApp, st: &FrUi) -> Vec<(usize, ObjId)> {
    let Some(d) = app.doc() else { return vec![] };
    let styles = &d.doc.object_styles;
    let test = |o: &Object| -> bool {
        match st.mode {
            0 if st.by_names => {
                let mut ok = st.name_on || st.style_on;
                if st.name_on {
                    let n = o.display_name();
                    ok &= if st.name_case { n.contains(&st.name) } else { n.to_lowercase().contains(&st.name.to_lowercase()) };
                }
                if st.style_on {
                    ok &= match (&o.object_style, st.style_by_type) {
                        (Some(s), false) => styles.get(st.style_name).is_some_and(|x| &x.name == s),
                        (Some(s), true) => styles.iter().find(|x| &x.name == s).is_some_and(|x| super::ostyles::KINDS[x.kind as usize] == STYLE_TYPES[st.style_type]),
                        (None, _) => false,
                    };
                }
                ok
            }
            0 => matches(o, &st.q),
            1 => replace_target(o, st),
            _ => match &o.kind {
                ObjKind::Text(t) => word_hit(&t.content, &st.text, st.case, st.whole),
                _ => false,
            },
        }
    };
    let mut v = vec![];
    for pi in range_pages(app, st) {
        for (_, _, l) in d.doc.layers_on_page(pi) {
            for o in &l.objects {
                if st.range == 0 && !d.selection.contains(&o.id) {
                    continue;
                }
                o.walk(&mut |x| {
                    if test(x) && !v.iter().any(|(_, id)| *id == x.id) {
                        v.push((pi, x.id));
                    }
                });
            }
        }
    }
    v
}

fn replace_target(o: &Object, st: &FrUi) -> bool {
    match st.rep {
        0 => {
            let c = st.find_color;
            (st.apply_to[0] && o.style.outline.color.is_some() && o.style.outline.color == c)
                || (st.apply_to[1] && matches!(o.style.fill, Fill::Solid(x) if Some(x) == c))
                || (st.apply_to[2] && matches!(&o.style.fill, Fill::Gradient(g) if g.stops.iter().any(|s| Some(s.color) == c)))
        }
        2 => o.style.outline.color.is_some() && st.ol_find.is_none_or(|w| (o.style.outline.width - w).abs() < 1e-6),
        3 => match &o.kind {
            ObjKind::Text(t) => st.font_find.as_ref().is_none_or(|f| &t.format.font == f) && st.size_find.is_none_or(|s| (t.format.size - s).abs() < 1e-6) && (st.font_find.is_some() || st.size_find.is_some()),
            _ => false,
        },
        _ => false,
    }
}

fn replace_in(o: &mut Object, st: &FrUi) {
    match st.rep {
        0 => {
            let (c, r) = (st.find_color, st.rep_color);
            if st.apply_to[0] && o.style.outline.color.is_some() && o.style.outline.color == c {
                o.style.outline.color = r;
            }
            if let Some(r) = r {
                match &mut o.style.fill {
                    Fill::Solid(x) if st.apply_to[1] && Some(*x) == c => *x = r,
                    Fill::Gradient(g) if st.apply_to[2] => {
                        for s in g.stops.iter_mut() {
                            if Some(s.color) == c {
                                s.color = r;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        2 => {
            if let Some(w) = st.ol_rep {
                o.style.outline.width = w;
            }
        }
        3 => {
            if let ObjKind::Text(t) = &mut o.kind {
                if let Some(f) = &st.font_rep {
                    t.format.font = f.clone();
                    for s in t.spans.iter_mut() {
                        s.format.font = f.clone();
                    }
                }
                if let Some(z) = st.size_rep {
                    t.format.size = z;
                    for s in t.spans.iter_mut() {
                        s.format.size = z;
                    }
                }
            }
        }
        _ => {}
    }
}

fn replace_text_in(o: &mut Object, st: &FrUi) {
    if let ObjKind::Text(t) = &mut o.kind {
        if !word_hit(&t.content, &st.text, st.case, st.whole) {
            return;
        }
        let new = if st.case { t.content.replace(&st.text, &st.rep_text) } else { replace_ci(&t.content, &st.text, &st.rep_text) };
        if new != t.content {
            t.spans.clear();
            t.content = new;
        }
    }
}

fn replace_ci(s: &str, needle: &str, rep: &str) -> String {
    let lower = s.to_lowercase();
    let nl = needle.to_lowercase();
    if nl.is_empty() || lower.len() != s.len() {
        return s.replace(needle, rep);
    }
    let mut out = String::new();
    let mut i = 0;
    while let Some(p) = lower[i..].find(&nl) {
        out.push_str(&s[i..i + p]);
        out.push_str(rep);
        i += p + nl.len();
    }
    out.push_str(&s[i..]);
    out
}

/// Show a match: its page and the object selected.
fn show(app: &mut VectorApp, (pi, id): (usize, ObjId)) {
    if let Some(d) = app.doc_mut() {
        if d.page != pi {
            d.page = pi;
            d.needs_fit = true;
        }
        d.selection = vec![id];
    }
}

/// < and > (Find Previous, Find Next), Find All, and Replace / Replace All when replacing.
fn nav_buttons(app: &mut VectorApp, ui: &mut Ui, st: &mut FrUi) {
    let ready = match st.mode {
        0 => if st.by_names { st.name_on || st.style_on } else { st.q != FindQuery::default() },
        1 => st.rep != 1,
        _ => !st.text.is_empty(),
    };
    let w = (ui.available_width() - 4.0) / 2.0;
    let mut step = 0i32;
    ui.horizontal(|ui| {
        if wide(ui, "<", ready, w).on_hover_text("Find Previous").clicked() {
            step = -1;
        }
        if wide(ui, ">", ready, w).on_hover_text("Find Next").clicked() {
            step = 1;
        }
    });
    if step != 0 {
        let found = search(app, st);
        if found != st.found {
            st.found = found;
            st.at = None;
        }
        let n = st.found.len();
        if n == 0 {
            app.flash("No matching objects were found.");
        } else {
            let i = match st.at {
                None if step > 0 => 0,
                None => n - 1,
                Some(i) => ((i as i32 + step).rem_euclid(n as i32)) as usize,
            };
            st.at = Some(i);
            show(app, st.found[i]);
        }
    }
    let full = ui.available_width();
    if wide(ui, "Find All", ready, full).clicked() {
        let found = search(app, st);
        // only the objects on the active page can be selected
        let page = found.first().map(|f| f.0);
        if let (Some(pi), Some(d)) = (page, app.doc_mut()) {
            if d.page != pi {
                d.page = pi;
                d.needs_fit = true;
            }
            d.selection = found.iter().filter(|f| f.0 == pi).map(|f| f.1).collect();
        }
        app.flash(format!("{} matching objects found", found.len()));
        st.found = found;
        st.at = None;
    }
    if st.mode == 0 {
        return;
    }
    let can_replace = ready && (st.mode == 2 && st.replace_on || st.mode == 1);
    ui.horizontal(|ui| {
        if wide(ui, "Replace", can_replace && st.at.is_some(), w).clicked() {
            if let Some(&(_, id)) = st.at.and_then(|i| st.found.get(i)) {
                let s2 = st.clone_for_replace();
                if let Some(d) = app.doc_mut() {
                    d.modify("Replace", |doc, _| {
                        if let Some(o) = doc.find_mut(id) {
                            if s2.mode == 1 {
                                replace_in(o, &s2);
                            } else {
                                replace_text_in(o, &s2);
                            }
                        }
                    });
                }
            }
        }
        if wide(ui, "Replace All", can_replace, w).clicked() {
            let found = search(app, st);
            let s2 = st.clone_for_replace();
            let n = found.len();
            if let Some(d) = app.doc_mut() {
                d.modify("Replace All", |doc, _| {
                    for (_, id) in &found {
                        if let Some(o) = doc.find_mut(*id) {
                            if s2.mode == 1 {
                                replace_in(o, &s2);
                            } else {
                                replace_text_in(o, &s2);
                            }
                        }
                    }
                });
            }
            app.flash(format!("Replaced in {n} objects"));
        }
    });
}

impl FrUi {
    fn clone_for_replace(&self) -> FrUi {
        FrUi {
            mode: self.mode,
            rep: self.rep,
            find_color: self.find_color,
            rep_color: self.rep_color,
            apply_to: self.apply_to,
            ol_find: self.ol_find,
            ol_rep: self.ol_rep,
            font_find: self.font_find.clone(),
            font_rep: self.font_rep.clone(),
            size_find: self.size_find,
            size_rep: self.size_rep,
            text: self.text.clone(),
            rep_text: self.rep_text.clone(),
            case: self.case,
            whole: self.whole,
            ..Default::default()
        }
    }
}
