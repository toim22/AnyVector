//! The color editor shared by the Color docker, the Properties docker (uniform fill, outline
//! color) and the color pickers: color viewers, color sliders and color palettes, a color model,
//! and the swatch, eyedropper and more-options bar under them.

use crate::app::VectorApp;
use crate::icons::Tone;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, TextureId, Ui};
use std::sync::Arc;
use vector_core::{Color, NamedColor};

/// What the editors need from the app, published in the egui context every frame (so the color
/// pickers inside any panel can show the full editor): the icon textures (normal, disabled) and
/// the palettes (the document palette last).
#[derive(Clone, Default)]
pub struct Env {
    icons: Vec<(&'static str, TextureId, TextureId)>,
    pals: Arc<Vec<(String, Vec<NamedColor>)>>,
    has_doc: bool,
}

const ENV_ICONS: [&str; 6] = ["color_viewers", "color_sliders", "color_palettes", "gear", "eyedropper", "more"];

fn env_id() -> egui::Id {
    egui::Id::unique("coled_env")
}

/// The editors' requests to the app: sample a color with the eyedropper (for the editor that
/// asked), add a color to the document palette or to the color styles.
#[derive(Clone, Default)]
struct Requests {
    eyedropper: Option<String>,
    add_spot: Option<Color>,
    add_palette: Option<Color>,
    add_style: Option<Color>,
}

fn req_id() -> egui::Id {
    egui::Id::unique("coled_requests")
}

fn picked_id(owner: &str) -> egui::Id {
    egui::Id::unique(("coled_picked", owner))
}

/// Publish the environment of the editors and carry out their requests (once a frame).
pub fn publish(app: &mut VectorApp, ctx: &egui::Context) {
    let mut icons = vec![];
    for n in ENV_ICONS {
        if let (Some(a), Some(b)) = (app.icons.get(ctx, n, 16.0, Tone::Normal), app.icons.get(ctx, n, 16.0, Tone::Disabled)) {
            icons.push((n, a.id, b.id));
        }
    }
    let mut pals: Vec<(String, Vec<NamedColor>)> = app.palettes.palettes.iter().map(|p| (p.name.clone(), p.colors.clone())).collect();
    if let Some(d) = app.doc() {
        pals.push(("Document Palette".into(), d.doc.palette.clone()));
    }
    let has_doc = app.has_doc();
    ctx.data_mut(|m| m.insert_temp(env_id(), Env { icons, pals: Arc::new(pals), has_doc }));
    let req = ctx.data_mut(|m| m.remove_temp::<Requests>(req_id())).unwrap_or_default();
    if let Some(owner) = req.eyedropper {
        app.ts.cancel();
        app.dockers.ui.pick_slot = SLOT_EYEDROPPER;
        app.dockers.ui.coled_owner = Some(owner);
        app.ts.picked_color = None;
        app.ts.pick_target = Some(crate::tools::PickTarget::SampleColor);
    }
    if app.dockers.ui.pick_slot == SLOT_EYEDROPPER {
        if let Some(c) = app.ts.picked_color.take() {
            app.dockers.ui.pick_slot = 0;
            if let Some(owner) = app.dockers.ui.coled_owner.take() {
                ctx.data_mut(|m| m.insert_temp(picked_id(&owner), c));
            }
        }
    }
    if let Some(c) = req.add_spot {
        let pals = &mut app.palettes.palettes;
        let i = match pals.iter().position(|p| p.name == "Custom Spot Colors") {
            Some(i) => i,
            None => {
                pals.push(vector_core::Palette { name: "Custom Spot Colors".into(), colors: vec![] });
                pals.len() - 1
            }
        };
        let n = pals[i].colors.len() + 1;
        let mut nc = NamedColor::new(format!("Spot Color {n}"), c);
        nc.spot = true;
        pals[i].colors.push(nc);
    }
    if let Some(c) = req.add_palette {
        if let Some(d) = app.doc_mut() {
            d.modify("Add Color", |doc, _| doc.palette.push(NamedColor::new(c.to_hex(), c)));
        }
    }
    if let Some(c) = req.add_style {
        if let Some(d) = app.doc_mut() {
            let n = d.doc.color_styles.len() + 1;
            d.modify("New Color Style", |doc, _| doc.color_styles.push(NamedColor::new(format!("Color Style {n}"), c)));
        }
    }
}

fn env(ui: &Ui) -> Env {
    ui.data(|m| m.get_temp::<Env>(env_id())).unwrap_or_default()
}

fn request(ui: &Ui, f: impl FnOnce(&mut Requests)) {
    ui.data_mut(|m| {
        let mut r = m.get_temp::<Requests>(req_id()).unwrap_or_default();
        f(&mut r);
        m.insert_temp(req_id(), r);
    });
}

/// An icon button drawn with the published icons.
fn tex_button(ui: &mut Ui, env: &Env, icon: &str, tip: &str, enabled: bool, checked: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), if enabled { Sense::click() } else { Sense::hover() });
    let resp = resp.on_hover_text(tip);
    if checked || (enabled && resp.is_pointer_button_down_on()) {
        ui.painter().rect_filled(r.shrink(1.0), 2.0, theme::PRESSED);
    } else if enabled && resp.hovered() {
        ui.painter().rect_filled(r.shrink(1.0), 2.0, theme::HOVER);
    }
    if let Some((_, a, b)) = env.icons.iter().find(|x| x.0 == icon) {
        ui.painter().image(if enabled { *a } else { *b }, Rect::from_center_size(r.center(), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    resp
}

/// The color models, in the order of the Color model list.
pub const MODELS: [&str; 9] = ["CMY", "CMYK", "RGB", "HSB", "HLS", "Lab", "YIQ", "Grayscale", "Registration Color"];
/// The color viewers (gear menu of the viewers).
const VIEWERS: [&str; 3] = ["HSB - Hue Based", "HSB - Brightness Based", "HSB - Wheel Based"];

/// How a color editor shows colors: the page (0 viewers, 1 sliders, 2 palettes), the model, the
/// viewer, the palette and its tint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorEd {
    pub view: u8,
    pub model: usize,
    pub viewer: u8,
    pub palette: usize,
    pub tint: f64,
    /// First row of the palette shown.
    pub scroll: usize,
}

impl Default for ColorEd {
    fn default() -> Self {
        ColorEd { view: 0, model: 1, viewer: 0, palette: 0, tint: 100.0, scroll: 0 }
    }
}

// ------------------------------------------------------------------ color models

fn srgb_to_lin(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn lin_to_srgb(v: f64) -> f64 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// CIE L*a*b* (D65) of a color.
pub fn to_lab(c: Color) -> [f64; 3] {
    let (r, g, b) = (srgb_to_lin(c.r as f64 / 255.0), srgb_to_lin(c.g as f64 / 255.0), srgb_to_lin(c.b as f64 / 255.0));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.950_47;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.088_83;
    let f = |t: f64| if t > 0.008_856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

pub fn from_lab(l: f64, a: f64, b: f64) -> Color {
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let inv = |t: f64| if t.powi(3) > 0.008_856 { t.powi(3) } else { (t - 16.0 / 116.0) / 7.787 };
    let (x, y, z) = (inv(fx) * 0.950_47, inv(fy), inv(fz) * 1.088_83);
    let r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
    let g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
    let bb = 0.0557 * x - 0.2040 * y + 1.0570 * z;
    let q = |v: f64| (lin_to_srgb(v.clamp(0.0, 1.0)) * 255.0).round() as u8;
    Color::rgb(q(r), q(g), q(bb))
}

fn to_hls(c: Color) -> [f64; 3] {
    let (r, g, b) = (c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    let s = if d == 0.0 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()) };
    [c.to_hsb()[0], l * 100.0, s * 100.0]
}

fn from_hls(h: f64, l: f64, s: f64) -> Color {
    let (l, s) = (l / 100.0, s / 100.0);
    let v = l + s * l.min(1.0 - l);
    let sv = if v == 0.0 { 0.0 } else { 2.0 * (1.0 - l / v) };
    Color::from_hsb(h, sv * 100.0, v * 100.0)
}

/// The components of `c` in `model`: (name, value, min, max).
pub fn components(model: usize, c: Color) -> Vec<(&'static str, f64, f64, f64)> {
    let (r, g, b) = (c.r as f64, c.g as f64, c.b as f64);
    match model {
        0 => vec![("C", 255.0 - r, 0.0, 255.0), ("M", 255.0 - g, 0.0, 255.0), ("Y", 255.0 - b, 0.0, 255.0)],
        1 => {
            let k = c.to_cmyk();
            vec![("C", k[0].round(), 0.0, 100.0), ("M", k[1].round(), 0.0, 100.0), ("Y", k[2].round(), 0.0, 100.0), ("K", k[3].round(), 0.0, 100.0)]
        }
        2 => vec![("R", r, 0.0, 255.0), ("G", g, 0.0, 255.0), ("B", b, 0.0, 255.0)],
        3 => {
            let h = c.to_hsb();
            vec![("H", h[0].round(), 0.0, 360.0), ("S", h[1].round(), 0.0, 100.0), ("B", h[2].round(), 0.0, 100.0)]
        }
        4 => {
            let h = to_hls(c);
            vec![("H", h[0].round(), 0.0, 360.0), ("L", h[1].round(), 0.0, 100.0), ("S", h[2].round(), 0.0, 100.0)]
        }
        5 => {
            let l = to_lab(c);
            vec![("L", l[0].round(), 0.0, 100.0), ("a", l[1].round(), -128.0, 127.0), ("b", l[2].round(), -128.0, 127.0)]
        }
        6 => {
            let y = 0.299 * r + 0.587 * g + 0.114 * b;
            let i = 0.596 * r - 0.274 * g - 0.322 * b;
            let q = 0.211 * r - 0.523 * g + 0.312 * b;
            vec![("Y", y.round(), 0.0, 255.0), ("I", (i / 1.192 + 127.5).round(), 0.0, 255.0), ("Q", (q / 1.046 + 127.5).round(), 0.0, 255.0)]
        }
        7 => vec![("L", (0.299 * r + 0.587 * g + 0.114 * b).round(), 0.0, 255.0)],
        _ => vec![],
    }
}

/// The color of components in `model` (the alpha of `old`).
pub fn from_components(model: usize, v: &[f64], old: Color) -> Color {
    let q = |x: f64| x.round().clamp(0.0, 255.0) as u8;
    let c = match (model, v) {
        (0, [c, m, y]) => Color::rgb(q(255.0 - c), q(255.0 - m), q(255.0 - y)),
        (1, [c, m, y, k]) => Color::from_cmyk(*c, *m, *y, *k),
        (2, [r, g, b]) => Color::rgb(q(*r), q(*g), q(*b)),
        (3, [h, s, b]) => Color::from_hsb(*h, *s, *b),
        (4, [h, l, s]) => from_hls(*h, *l, *s),
        (5, [l, a, b]) => from_lab(*l, *a, *b),
        (6, [y, i, qq]) => {
            let (i, qq) = ((i - 127.5) * 1.192, (qq - 127.5) * 1.046);
            Color::rgb(q(y + 0.956 * i + 0.621 * qq), q(y - 0.272 * i - 0.647 * qq), q(y - 1.106 * i + 1.703 * qq))
        }
        (7, [l]) => Color::rgb(q(*l), q(*l), q(*l)),
        _ => old,
    };
    c.with_alpha(old.a)
}

/// The color values the Properties docker lists under a uniform fill.
pub fn value_lines(c: Color) -> Vec<(&'static str, String)> {
    let k = c.to_cmyk();
    let h = c.to_hsb();
    let l = to_lab(c);
    vec![
        ("CMYK:", format!("C:{:.0} M:{:.0} Y:{:.0} K:{:.0}", k[0], k[1], k[2], k[3])),
        ("RGB:", format!("R:{} G:{} B:{}", c.r, c.g, c.b)),
        ("HSB:", format!("H:{:.0} S:{:.0} B:{:.0}", h[0], h[1], h[2])),
        ("Lab:", format!("L:{:.0} a:{:.0} b:{:.0}", l[0], l[1], l[2])),
        ("Hex:", c.to_hex().to_uppercase()),
    ]
}

// ------------------------------------------------------------------ drawing

/// A rectangle shaded by `f(x, y)` (0..1 each), as a mesh of `n` × `n` cells.
fn shade(ui: &Ui, r: Rect, n: usize, f: impl Fn(f64, f64) -> Color) {
    let mut mesh = egui::Mesh::default();
    for j in 0..=n {
        for i in 0..=n {
            let (x, y) = (i as f64 / n as f64, j as f64 / n as f64);
            let c = f(x, y);
            mesh.colored_vertex(pos2(r.left() + r.width() * x as f32, r.top() + r.height() * y as f32), Color32::from_rgb(c.r, c.g, c.b));
        }
    }
    let w = (n + 1) as u32;
    for j in 0..n as u32 {
        for i in 0..n as u32 {
            let a = j * w + i;
            mesh.add_triangle(a, a + 1, a + w);
            mesh.add_triangle(a + 1, a + w + 1, a + w);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

fn marker(ui: &Ui, p: egui::Pos2) {
    let r = Rect::from_center_size(p, vec2(7.0, 7.0));
    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Outside);
    ui.painter().rect_stroke(r.shrink(1.0), 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Outside);
}

/// A numeric component field; true when changed.
fn field(ui: &mut Ui, v: &mut f64, lo: f64, hi: f64, w: f32) -> bool {
    ui.add_sized(vec2(w, 22.0), egui::DragValue::new(v).range(lo..=hi).speed(1.0).max_decimals(0)).changed()
}

/// The hex field: true (and `c` set) when a valid code was entered.
fn hex_field(ui: &mut Ui, c: &mut Color, w: f32) -> bool {
    let id = ui.make_persistent_id("coled_hex");
    let mut s = ui.data_mut(|m| m.get_temp::<String>(id)).unwrap_or_else(|| c.to_hex().to_uppercase());
    let r = ui.add_sized(vec2(w, 22.0), egui::TextEdit::singleline(&mut s));
    if r.has_focus() {
        ui.data_mut(|m| m.insert_temp(id, s.clone()));
    } else {
        ui.data_mut(|m| m.remove::<String>(id));
    }
    if r.lost_focus() {
        if let Some(n) = Color::from_hex(s.trim()) {
            *c = n.with_alpha(c.a);
            return true;
        }
    }
    false
}

/// The viewer tabs (`tabs`; the pickers show the viewer only), the model (or palette) list with
/// the gear, and the page shown; the RGB model shows the hex code too. True when the color
/// changed.
pub fn editor(ui: &mut Ui, ed: &mut ColorEd, c: &mut Color, tabs: bool) -> bool {
    let env = env(ui);
    let full = ui.available_width();
    let mut changed = false;
    if tabs {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (i, (icon, tip)) in [("color_viewers", "Show color viewers"), ("color_sliders", "Show color sliders"), ("color_palettes", "Show color palettes")].iter().enumerate() {
                if tex_button(ui, &env, icon, tip, true, ed.view == i as u8).clicked() {
                    ed.view = i as u8;
                }
            }
        });
        ui.add_space(2.0);
    } else {
        ed.view = 0;
    }
    let hex = ed.model == 2;
    // the model or the palette, and the gear
    let pals = env.pals.clone();
    ui.horizontal(|ui| {
        let w = full - 30.0;
        if ed.view == 2 {
            ed.palette = ed.palette.min(pals.len().saturating_sub(1));
            let names: Vec<&str> = pals.iter().map(|p| p.0.as_str()).collect();
            super::choice(ui, "coled_pal", &mut ed.palette, &names, w - 8.0);
        } else {
            super::choice(ui, "coled_model", &mut ed.model, &MODELS, w - 8.0);
        }
        let gear = tex_button(ui, &env, "gear", "Color viewers", true, false);
        egui::Popup::menu(&gear).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            for (i, n) in VIEWERS.iter().enumerate() {
                if ui.radio(ed.viewer == i as u8, *n).clicked() {
                    ed.viewer = i as u8;
                    ui.close();
                }
            }
        });
    });
    ui.add_space(4.0);
    let comps = components(ed.model, *c);
    match ed.view {
        0 => {
            ui.horizontal_top(|ui| {
                let vw = (full - 112.0).clamp(90.0, 140.0);
                ui.vertical(|ui| {
                    ui.add_space(2.0);
                    changed |= viewer(ui, ed.viewer, c, vw);
                });
                ui.add_space((full - vw - 106.0).max(4.0));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let mut vals: Vec<f64> = comps.iter().map(|x| x.1).collect();
                    let mut ch = false;
                    for (k, (n, _, lo, hi)) in comps.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add_sized(vec2(24.0, 22.0), egui::Label::new(*n));
                            ui.add_space(8.0);
                            ch |= field(ui, &mut vals[k], *lo, *hi, 40.0);
                        });
                    }
                    if ch {
                        *c = from_components(ed.model, &vals, *c);
                        changed = true;
                    }
                    if hex {
                        ui.horizontal(|ui| {
                            ui.add_space(36.0);
                            changed |= hex_field(ui, c, 64.0);
                        });
                    }
                });
            });
        }
        1 => {
            let mut vals: Vec<f64> = comps.iter().map(|x| x.1).collect();
            let mut ch = false;
            for (k, (n, _, lo, hi)) in comps.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(12.0, 22.0), egui::Label::new(*n));
                    let (r, resp) = ui.allocate_exact_size(vec2(full - 72.0, 10.0), Sense::click_and_drag());
                    let base = *c;
                    let model = ed.model;
                    let v0 = vals.clone();
                    shade(ui, r, 24, |x, _| {
                        let mut v = v0.clone();
                        v[k] = lo + (hi - lo) * x;
                        from_components(model, &v, base)
                    });
                    let t = ((vals[k] - lo) / (hi - lo)) as f32;
                    let mx = r.left() + r.width() * t;
                    ui.painter().rect_filled(Rect::from_center_size(pos2(mx, r.center().y), vec2(3.0, 16.0)), 0.0, Color32::BLACK);
                    ui.painter().rect_stroke(Rect::from_center_size(pos2(mx, r.center().y), vec2(3.0, 16.0)), 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Outside);
                    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
                        vals[k] = (lo + (hi - lo) * ((p.x - r.left()) / r.width()).clamp(0.0, 1.0) as f64).round();
                        ch = true;
                    }
                    ch |= field(ui, &mut vals[k], *lo, *hi, 46.0);
                });
            }
            if ch {
                *c = from_components(ed.model, &vals, *c);
                changed = true;
            }
            if hex {
                ui.horizontal(|ui| {
                    ui.add_space(full - 72.0);
                    changed |= hex_field(ui, c, 64.0);
                });
            }
        }
        _ => {
            let colors = pals.get(ed.palette).map(|p| p.1.clone()).unwrap_or_default();
            let gw = full - 40.0;
            let cell = 18.0;
            let cols = ((gw / cell).floor() as usize).max(4);
            let shown = 8;
            let total_rows = colors.len().div_ceil(cols).max(1);
            ed.scroll = ed.scroll.min(total_rows.saturating_sub(shown));
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(cols as f32 * cell, shown as f32 * cell), Sense::hover());
                if ui.rect_contains_pointer(r) {
                    let dy = ui.input(|i| i.smooth_scroll_delta.y);
                    if dy < -1.0 {
                        ed.scroll = (ed.scroll + 1).min(total_rows.saturating_sub(shown));
                    } else if dy > 1.0 {
                        ed.scroll = ed.scroll.saturating_sub(1);
                    }
                }
                for (i, nc) in colors.iter().enumerate().skip(ed.scroll * cols).take(cols * shown) {
                    let k = i - ed.scroll * cols;
                    let cr = Rect::from_min_size(r.min + vec2((k % cols) as f32 * cell, (k / cols) as f32 * cell), vec2(cell - 1.0, cell - 1.0));
                    let col = Color::lerp(Color::WHITE, nc.color, ed.tint / 100.0);
                    ui.painter().rect_filled(cr, 0.0, theme::to_c32(col));
                    if nc.spot {
                        let m = Rect::from_min_size(pos2(cr.left() + 2.0, cr.bottom() - 6.0), vec2(4.0, 4.0));
                        ui.painter().rect_filled(m, 0.0, Color32::WHITE);
                        ui.painter().rect_stroke(m, 0.0, Stroke::new(0.5, Color32::from_gray(90)), egui::StrokeKind::Outside);
                    }
                    let resp = ui.interact(cr, ui.scope_id().with(("coled_cell", i)), Sense::click()).on_hover_text(&nc.name);
                    if nc.color.with_alpha(255) == c.with_alpha(255) {
                        ui.painter().rect_stroke(cr, 0.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Inside);
                    }
                    if resp.clicked() {
                        *c = col.with_alpha(c.a);
                        changed = true;
                    }
                }
                // the overview of the whole palette: click to scroll there
                ui.add_space((full - cols as f32 * cell - 14.0).max(4.0));
                let (sr, sresp) = ui.allocate_exact_size(vec2(8.0, shown as f32 * cell + 30.0), Sense::click_and_drag());
                if !colors.is_empty() {
                    let lines = total_rows as f32;
                    let lh = sr.height() / lines;
                    for row in 0..total_rows {
                        for j in 0..cols {
                            if let Some(nc) = colors.get(row * cols + j) {
                                let x0 = sr.left() + sr.width() * j as f32 / cols as f32;
                                let rr = Rect::from_min_max(pos2(x0, sr.top() + row as f32 * lh), pos2(x0 + sr.width() / cols as f32 + 0.5, sr.top() + (row + 1) as f32 * lh));
                                ui.painter().rect_filled(rr, 0.0, theme::to_c32(nc.color));
                            }
                        }
                    }
                    let win = Rect::from_min_max(pos2(sr.left() - 1.0, sr.top() + ed.scroll as f32 * lh), pos2(sr.right() + 1.0, sr.top() + ((ed.scroll + shown).min(total_rows)) as f32 * lh));
                    ui.painter().rect_stroke(win, 0.0, Stroke::new(1.0, Color32::from_gray(40)), egui::StrokeKind::Outside);
                    if let Some(p) = sresp.interact_pointer_pos().filter(|_| sresp.clicked() || sresp.dragged()) {
                        let row = ((p.y - sr.top()) / lh).max(0.0) as usize;
                        ed.scroll = row.saturating_sub(shown / 2).min(total_rows.saturating_sub(shown));
                    }
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(full - 56.0, 10.0), Sense::click_and_drag());
                let base = colors.iter().find(|n| n.color.with_alpha(255) == c.with_alpha(255)).map(|n| n.color).unwrap_or(*c);
                shade(ui, r, 12, |x, _| Color::lerp(Color::WHITE, base, x));
                let mx = r.left() + r.width() * (ed.tint / 100.0) as f32;
                ui.painter().rect_filled(Rect::from_center_size(pos2(mx, r.center().y), vec2(3.0, 14.0)), 0.0, Color32::from_gray(230));
                if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.clicked() || resp.dragged()) {
                    ed.tint = (((p.x - r.left()) / r.width()).clamp(0.0, 1.0) as f64 * 100.0).round();
                }
                field(ui, &mut ed.tint, 0.0, 100.0, 40.0);
            });
        }
    }
    changed
}

/// The color viewer: a field and a strip for the chosen HSB viewer.
fn viewer(ui: &mut Ui, kind: u8, c: &mut Color, w: f32) -> bool {
    let [h, s, b] = c.to_hsb();
    let mut changed = false;
    let (r, resp) = ui.allocate_exact_size(vec2(w, w), Sense::click_and_drag());
    let pick = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()).map(|p| (((p.x - r.left()) / r.width()).clamp(0.0, 1.0) as f64, ((p.y - r.top()) / r.height()).clamp(0.0, 1.0) as f64));
    match kind {
        0 => {
            shade(ui, r, 16, |x, y| Color::from_hsb(h, x * 100.0, (1.0 - y) * 100.0));
            marker(ui, pos2(r.left() + r.width() * (s / 100.0) as f32, r.top() + r.height() * (1.0 - b / 100.0) as f32));
            if let Some((x, y)) = pick {
                *c = Color::from_hsb(h, x * 100.0, (1.0 - y) * 100.0).with_alpha(c.a);
                changed = true;
            }
        }
        1 => {
            shade(ui, r, 24, |x, y| Color::from_hsb(x * 360.0, (1.0 - y) * 100.0, b.max(1.0)));
            marker(ui, pos2(r.left() + r.width() * (h / 360.0) as f32, r.top() + r.height() * (1.0 - s / 100.0) as f32));
            if let Some((x, y)) = pick {
                *c = Color::from_hsb(x * 360.0, (1.0 - y) * 100.0, b.max(1.0)).with_alpha(c.a);
                changed = true;
            }
        }
        _ => {
            // a hue wheel, saturation out from the center
            let cen = r.center();
            let rad = r.width().min(r.height()) / 2.0 - 2.0;
            let mut mesh = egui::Mesh::default();
            let (rings, segs) = (8u32, 48u32);
            for ri in 0..=rings {
                for si in 0..=segs {
                    let a = si as f64 / segs as f64 * std::f64::consts::TAU;
                    let rr = ri as f64 / rings as f64;
                    let col = Color::from_hsb(a.to_degrees(), rr * 100.0, b.max(1.0));
                    mesh.colored_vertex(cen + vec2((a.cos() * rr) as f32 * rad, (-a.sin() * rr) as f32 * rad), Color32::from_rgb(col.r, col.g, col.b));
                }
            }
            let w = segs + 1;
            for ri in 0..rings {
                for si in 0..segs {
                    let a = ri * w + si;
                    mesh.add_triangle(a, a + 1, a + w);
                    mesh.add_triangle(a + 1, a + w + 1, a + w);
                }
            }
            ui.painter().add(egui::Shape::mesh(mesh));
            let a = h.to_radians();
            marker(ui, cen + vec2((a.cos() * s / 100.0) as f32 * rad, (-a.sin() * s / 100.0) as f32 * rad));
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
                let d = p - cen;
                let hh = (-d.y as f64).atan2(d.x as f64).to_degrees().rem_euclid(360.0);
                let ss = ((d.length() / rad) as f64).min(1.0) * 100.0;
                *c = Color::from_hsb(hh, ss, b.max(1.0)).with_alpha(c.a);
                changed = true;
            }
        }
    }
    // the strip: hue for the hue-based viewer, brightness for the others
    ui.add_space(4.0);
    let (sr, sresp) = ui.allocate_exact_size(vec2(w, 10.0), Sense::click_and_drag());
    let [h, s, b] = c.to_hsb();
    if kind == 0 {
        shade(ui, sr, 36, |x, _| Color::from_hsb(x * 360.0, 100.0, 100.0));
        marker(ui, pos2(sr.left() + sr.width() * (h / 360.0) as f32, sr.center().y));
    } else {
        shade(ui, sr, 12, |x, _| Color::from_hsb(h, s, x * 100.0));
        marker(ui, pos2(sr.left() + sr.width() * (b / 100.0) as f32, sr.center().y));
    }
    if let Some(p) = sresp.interact_pointer_pos().filter(|_| sresp.dragged() || sresp.clicked()) {
        let x = ((p.x - sr.left()) / sr.width()).clamp(0.0, 1.0) as f64;
        *c = if kind == 0 { Color::from_hsb(x * 360.0, s, b) } else { Color::from_hsb(h, s, x * 100.0) }.with_alpha(c.a);
        changed = true;
    }
    changed
}

/// What the bar under the editor asks for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BarAction {
    None,
    /// The no-color swatch was clicked.
    NoColor,
    /// More color options ▸ Swap Colors.
    Swap,
}

/// Pick slot of the color editors' eyedropper.
pub const SLOT_EYEDROPPER: u8 = 50;

/// The swatch (and the no-color swatch), the eyedropper and the more-options menu of the editor
/// `owner` (the eyedropper's color comes back to it).
pub fn bar(ui: &mut Ui, owner: &str, c: &mut Color, reference: Option<Color>, no_color: bool) -> (bool, BarAction) {
    let swap = reference.is_some();
    let env = env(ui);
    let mut changed = false;
    let mut act = BarAction::None;
    // a color sampled with the eyedropper
    if let Some(p) = ui.data_mut(|m| m.remove_temp::<Color>(picked_id(owner))) {
        *c = p.with_alpha(c.a);
        changed = true;
    }
    ui.horizontal(|ui| {
        let (sw, sresp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
        match reference {
            Some(rc) => {
                let (top, bottom) = (Rect::from_min_max(sw.min, pos2(sw.right(), sw.center().y)), Rect::from_min_max(pos2(sw.left(), sw.center().y), sw.max));
                crate::widgets::paint_well(ui.painter(), top, Some(rc));
                crate::widgets::paint_well(ui.painter(), bottom, Some(*c));
                sresp.on_hover_text("Reference color (top) and new color (bottom)");
            }
            None => crate::widgets::paint_well(ui.painter(), sw, Some(*c)),
        }
        ui.painter().rect_stroke(sw, 0.0, Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
        if no_color {
            let (nr, nresp) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::click());
            crate::widgets::paint_well(ui.painter(), nr, None);
            if nresp.on_hover_text("No color").clicked() {
                act = BarAction::NoColor;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let more = tex_button(ui, &env, "more", "More color options", true, false);
            super::drop_mark(ui, more.rect);
            egui::Popup::menu(&more).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.set_min_width(180.0);
                if ui.add_enabled(swap, egui::Button::new("Swap Colors")).clicked() {
                    act = BarAction::Swap;
                    ui.close();
                }
                ui.separator();
                let col = *c;
                if ui.button("Add to Custom Spot Colors").clicked() {
                    request(ui, |r| r.add_spot = Some(col));
                    ui.close();
                }
                if ui.add_enabled(env.has_doc, egui::Button::new("Add to Document Palette")).clicked() {
                    request(ui, |r| r.add_palette = Some(col));
                    ui.close();
                }
                if ui.add_enabled(env.has_doc, egui::Button::new("Add to Color Styles Palette")).clicked() {
                    request(ui, |r| r.add_style = Some(col));
                    ui.close();
                }
            });
            if tex_button(ui, &env, "eyedropper", "Eyedropper", env.has_doc, false).clicked() {
                let o = owner.to_string();
                request(ui, |r| r.eyedropper = Some(o));
            }
        });
    });
    (changed, act)
}

/// The editor of a color picker's popup (the viewer, the model and the bar), its settings kept
/// for the picker `id`. `none`: the no-color swatch.
pub fn picker(ui: &mut Ui, id: &str, c: &mut Option<Color>, none: bool) -> bool {
    let sid = egui::Id::unique(("coled_picker", id));
    let mut ed = ui.data_mut(|m| m.get_temp::<ColorEd>(sid)).unwrap_or_default();
    // the color when the popup opened is the reference
    let rid = egui::Id::unique(("coled_picker_ref", id));
    let reference = ui.data_mut(|m| *m.get_temp_mut_or_insert_with(rid, || c.unwrap_or(Color::BLACK)));
    ui.set_width(236.0);
    let mut col = c.unwrap_or(Color::BLACK);
    let mut changed = editor(ui, &mut ed, &mut col, false);
    ui.add_space(4.0);
    super::separator(ui);
    ui.add_space(4.0);
    let (ch, act) = bar(ui, id, &mut col, Some(reference), none);
    changed |= ch;
    match act {
        BarAction::NoColor => {
            *c = None;
            ui.data_mut(|m| m.insert_temp(sid, ed));
            return true;
        }
        BarAction::Swap => {
            ui.data_mut(|m| m.insert_temp(rid, col));
            col = reference;
            changed = true;
        }
        BarAction::None => {}
    }
    ui.data_mut(|m| m.insert_temp(sid, ed));
    if changed {
        *c = Some(col);
    }
    changed
}

/// Forget a picker's reference color (when its popup closes).
pub fn picker_closed(ctx: &egui::Context, id: &str) {
    ctx.data_mut(|m| m.remove::<Color>(egui::Id::unique(("coled_picker_ref", id))));
}
