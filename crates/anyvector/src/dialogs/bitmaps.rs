//! The Bitmaps menu: Convert To Bitmap, Straighten Image, Image Adjustment Lab, Resample,
//! Mode (Convert to 1 bit, Duotone, Convert to Paletted), Inflate Bitmap, Edit Bitmap, the
//! links, and PowerTRACE (Outline Trace, Centerline Trace, Quick Trace).

use super::{button, ok_button, window};
use crate::app::AnyVectorApp;
use crate::bitmaps::{self, Px};
use crate::theme;
use egui::{pos2, vec2, Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Ui, UiBuilder, Vec2};
use kurbo::Affine;
use vector_core::object::ImageObj;
use vector_core::{Color, ObjId, ObjKind};

// ------------------------------------------------------------------------------ state

/// The open Bitmaps dialogs and what they remember.
#[derive(Default)]
pub struct BitmapDialogs {
    pub convert: Option<ConvertState>,
    pub convert_last: Option<ConvertState>,
    pub straighten: Option<StraightenState>,
    pub straighten_saved: Option<bitmaps::Straighten>,
    pub lab: Option<LabState>,
    pub lab_saved: Option<bitmaps::AdjustLab>,
    pub resample: Option<ResampleState>,
    pub one_bit: Option<OneBitState>,
    pub duotone: Option<DuotoneState>,
    pub paletted: Option<PalettedState>,
    /// Convert to Paletted ▸ Presets added with +.
    pub paletted_presets: Vec<(String, bitmaps::Paletted)>,
    pub inflate: Option<InflateState>,
    pub trace: Option<TraceState>,
    /// The settings of the last trace (Repeat Last Trace, Open Last Trace Settings).
    pub last_trace: Option<TraceOpts>,
    /// Bitmaps being edited in the image editor: the file, the object and its last change.
    pub edits: Vec<(std::path::PathBuf, ObjId, Option<std::time::SystemTime>)>,
    pub edits_checked: f64,
}

impl BitmapDialogs {
    pub fn any_open(&self) -> bool {
        self.convert.is_some() || self.straighten.is_some() || self.lab.is_some() || self.resample.is_some() || self.one_bit.is_some() || self.duotone.is_some() || self.paletted.is_some() || self.inflate.is_some() || self.trace.is_some()
    }
}

pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    convert(app, ctx);
    straighten(app, ctx);
    lab(app, ctx);
    resample(app, ctx);
    one_bit(app, ctx);
    duotone(app, ctx);
    paletted(app, ctx);
    inflate(app, ctx);
    trace(app, ctx);
}

// ------------------------------------------------------------------------------ the bitmap

/// The selected bitmap.
pub struct Target {
    pub obj: ObjId,
    pub io: ImageObj,
    /// All its pixels.
    pub full: Px,
    /// Its pixels per inch on the page.
    pub ppi: f64,
    /// How the object scales its local box onto the page (x, y).
    pub scale: (f64, f64),
}

impl Target {
    /// The pixels shown (inside the crop).
    pub fn visible(&self) -> Px {
        if self.io.crop == [0.0, 0.0, 1.0, 1.0] {
            self.full.clone()
        } else {
            bitmaps::crop(&self.full, self.io.crop)
        }
    }
}

/// The first selected object when it is a bitmap.
pub fn target(app: &AnyVectorApp) -> Option<Target> {
    let d = app.doc()?;
    let o = d.doc.find(*d.selection.first()?)?;
    let ObjKind::Image(io) = &o.kind else { return None };
    let img = d.doc.images.get(&io.image)?;
    if img.width == 0 || img.height == 0 {
        return None;
    }
    let c = o.transform.as_coeffs();
    let scale = (c[0].hypot(c[1]), c[2].hypot(c[3]));
    let cw = (io.crop[2] - io.crop[0]).max(1e-6);
    let ppi = img.width as f64 * cw / (io.w * scale.0 / 72.0).max(1e-9);
    Some(Target { obj: o.id, io: io.clone(), full: Px::of(img), ppi, scale })
}

/// The color mode of the selected bitmap (`bitmaps::MODES`).
pub fn selected_mode(app: &AnyVectorApp) -> Option<u8> {
    let d = app.doc()?;
    let o = d.doc.find(*d.selection.first()?)?;
    let ObjKind::Image(io) = &o.kind else { return None };
    Some(d.doc.image_modes.get(&io.image).copied().unwrap_or(bitmaps::MODE_RGB))
}

/// Puts new pixels in a bitmap as a new image (the old one may be shared), with its color mode
/// (else the old one). `area`: the part of the object's local box the pixels cover (the box
/// itself when None, keeping the crop).
pub fn replace_bitmap(app: &mut AnyVectorApp, label: &str, id: ObjId, px: &Px, mode: Option<u8>, area: Option<kurbo::Rect>) {
    let Some(d) = app.doc_mut() else { return };
    let Some(ObjKind::Image(io)) = d.doc.find(id).map(|o| o.kind.clone()) else { return };
    let Some(old) = d.doc.images.get(&io.image).cloned() else { return };
    let data = px.image(&old);
    let old_mode = d.doc.image_modes.get(&io.image).copied();
    d.modify(label, |doc, _| {
        let nid = doc.add_image(data);
        if let Some(m) = mode.or(old_mode).filter(|m| *m != bitmaps::MODE_RGB) {
            doc.image_modes.insert(nid, m);
        }
        if let Some(o) = doc.find_mut(id) {
            if let ObjKind::Image(i) = &mut o.kind {
                i.image = nid;
                if let Some(r) = area {
                    i.w = r.width().max(1e-6);
                    i.h = r.height().max(1e-6);
                    i.crop = [0.0, 0.0, 1.0, 1.0];
                    o.transform = o.transform * Affine::translate((r.x0, r.y0));
                }
            }
        }
    });
}

/// Records a color mode for the selected bitmap without changing its pixels.
fn set_mode(app: &mut AnyVectorApp, label: &str, m: u8) {
    let Some(d) = app.doc_mut() else { return };
    let Some(ObjKind::Image(io)) = d.selection.first().and_then(|id| d.doc.find(*id)).map(|o| o.kind.clone()) else { return };
    d.modify(label, |doc, _| {
        if m == bitmaps::MODE_RGB {
            doc.image_modes.remove(&io.image);
        } else {
            doc.image_modes.insert(io.image, m);
        }
    });
}

fn need_bitmap(app: &mut AnyVectorApp) -> Option<Target> {
    let t = target(app);
    if t.is_none() {
        app.flash("Select a bitmap.");
    }
    t
}

// ------------------------------------------------------------------------------ commands

/// Bitmaps ▸ Mode ▸ `bitmaps::MODES[m]`.
pub fn mode(app: &mut AnyVectorApp, m: u8) {
    let Some(t) = need_bitmap(app) else { return };
    match m {
        0 => app.dialogs.bm.one_bit = Some(OneBitState::new(&t)),
        1 => {
            let px = bitmaps::grayscale(&t.full);
            replace_bitmap(app, "Convert to Grayscale", t.obj, &px, Some(1), None);
        }
        2 => app.dialogs.bm.duotone = Some(DuotoneState::new(&t)),
        3 => app.dialogs.bm.paletted = Some(PalettedState::new(&t)),
        _ => set_mode(app, &format!("Convert to {}", bitmaps::MODES[m as usize]), m),
    }
}

pub fn remove_jpeg_artifacts(app: &mut AnyVectorApp) {
    let Some(t) = need_bitmap(app) else { return };
    let px = bitmaps::remove_jpeg_artifacts(&t.full);
    replace_bitmap(app, "Remove JPEG Artifacts", t.obj, &px, None, None);
}

/// Effects ▸ Adjust ▸ Auto Adjust: each selected bitmap gets the color and tone correction the
/// Image Adjustment Lab's Auto adjust finds for it.
pub fn auto_adjust(app: &mut AnyVectorApp) {
    let Some(d) = app.doc() else { return };
    let ids: Vec<ObjId> = d.selection.iter().copied().filter(|id| d.doc.find(*id).is_some_and(|o| matches!(o.kind, ObjKind::Image(_)))).collect();
    if ids.is_empty() {
        app.flash("Select a bitmap.");
        return;
    }
    for id in ids {
        let Some(img) = app.doc().and_then(|d| match &d.doc.find(id)?.kind {
            ObjKind::Image(io) => d.doc.images.get(&io.image).cloned(),
            _ => None,
        }) else {
            continue;
        };
        let px = Px::of(&img);
        let out = bitmaps::apply_effects(&px, &bitmaps::AdjustLab::auto(&px).effects());
        replace_bitmap(app, "Auto Adjust", id, &out, None, None);
    }
}

pub fn remove_background(app: &mut AnyVectorApp) {
    let Some(t) = need_bitmap(app) else { return };
    let px = bitmaps::remove_background(&t.full);
    replace_bitmap(app, "Remove Background", t.obj, &px, None, None);
}

/// Crop Bitmap: the pixels outside the crop go.
pub fn crop_bitmap(app: &mut AnyVectorApp) {
    let Some(t) = need_bitmap(app) else { return };
    let px = t.visible();
    replace_bitmap(app, "Crop Bitmap", t.obj, &px, None, Some(kurbo::Rect::new(0.0, 0.0, t.io.w, t.io.h)));
}

/// Whether the selected bitmap is cropped (Crop Bitmap has work to do).
pub fn is_cropped(app: &AnyVectorApp) -> bool {
    target(app).is_some_and(|t| t.io.crop != [0.0, 0.0, 1.0, 1.0])
}

/// The link file of the selected bitmap's image.
pub fn link_of(app: &AnyVectorApp) -> Option<(u64, String)> {
    let d = app.doc()?;
    let o = d.doc.find(*d.selection.first()?)?;
    let ObjKind::Image(io) = &o.kind else { return None };
    d.doc.links.get(&io.image).map(|p| (io.image, p.clone()))
}

pub fn break_link(app: &mut AnyVectorApp) {
    let Some((id, _)) = link_of(app) else { return };
    if let Some(d) = app.doc_mut() {
        d.modify("Break Link", |doc, _| {
            doc.links.remove(&id);
        });
    }
}

pub fn update_link(app: &mut AnyVectorApp) {
    if let Some((id, _)) = link_of(app) {
        crate::dockers::misc::sync_links(app, &[id]);
    }
}

/// Edit Bitmap: the bitmap opens in the default image editor; saving it there updates it here.
pub fn edit_bitmap(app: &mut AnyVectorApp) {
    let Some(t) = need_bitmap(app) else { return };
    let dir = std::env::temp_dir().join("AnyVector");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("bitmap-{}.png", t.obj));
    let bytes = vector_io::image_io::encode_png(t.full.w as u32, t.full.h as u32, &t.full.p.iter().flat_map(|c| *c).collect::<Vec<u8>>());
    if let Err(e) = std::fs::write(&path, bytes) {
        app.dialogs.message_box = Some(("Edit Bitmap".into(), format!("Could not write {}: {e}", path.display())));
        return;
    }
    let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    let _ = std::process::Command::new("cmd").args(["/C", "start", ""]).arg(&path).spawn();
    app.dialogs.bm.edits.retain(|e| e.1 != t.obj);
    app.dialogs.bm.edits.push((path, t.obj, stamp));
    app.flash("Edit the bitmap and save it: AnyVector updates it.");
}

/// Reads back the bitmaps saved in the image editor (checked once a second).
pub fn poll_edits(app: &mut AnyVectorApp, ctx: &egui::Context) {
    if app.dialogs.bm.edits.is_empty() {
        return;
    }
    ctx.request_repaint_after(std::time::Duration::from_secs(1));
    let now = ctx.input(|i| i.time);
    if now - app.dialogs.bm.edits_checked < 1.0 {
        return;
    }
    app.dialogs.bm.edits_checked = now;
    let mut changed = vec![];
    for e in app.dialogs.bm.edits.iter_mut() {
        let stamp = std::fs::metadata(&e.0).and_then(|m| m.modified()).ok();
        if stamp.is_some() && stamp != e.2 {
            e.2 = stamp;
            changed.push((e.0.clone(), e.1));
        }
    }
    for (path, obj) in changed {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(img) = vector_io::image_io::decode(&bytes, "bitmap.png") else { continue };
        replace_bitmap(app, "Edit Bitmap", obj, &Px::of(&img), None, None);
    }
    let alive: Vec<bool> = app.dialogs.bm.edits.iter().map(|e| app.docs.iter().any(|d| d.doc.find(e.1).is_some())).collect();
    let mut k = 0;
    app.dialogs.bm.edits.retain(|_| {
        k += 1;
        alive[k - 1]
    });
}

pub fn open_convert(app: &mut AnyVectorApp) {
    if app.doc().is_none_or(|d| d.selection.is_empty()) {
        app.flash("Select the objects to convert to a bitmap.");
        return;
    }
    app.dialogs.bm.convert = Some(app.dialogs.bm.convert_last.clone().unwrap_or_default());
}

pub fn open_straighten(app: &mut AnyVectorApp) {
    let Some(t) = need_bitmap(app) else { return };
    let o = app.dialogs.bm.straighten_saved.clone().unwrap_or_default();
    app.dialogs.bm.straighten = Some(StraightenState::new(&t, o));
}

pub fn open_lab(app: &mut AnyVectorApp) {
    let Some(t) = need_bitmap(app) else { return };
    let a = app.dialogs.bm.lab_saved.clone().unwrap_or_default();
    app.dialogs.bm.lab = Some(LabState::new(&t, a));
}

pub fn open_resample(app: &mut AnyVectorApp) {
    let Some(t) = need_bitmap(app) else { return };
    app.dialogs.bm.resample = Some(ResampleState::new(&t));
}

pub fn open_inflate(app: &mut AnyVectorApp) {
    let Some(t) = need_bitmap(app) else { return };
    let v = t.visible();
    app.dialogs.bm.inflate = Some(InflateState { w0: v.w, h0: v.h, w: v.w as f64, h: v.h as f64, keep: true });
}

// ------------------------------------------------------------------------------ widgets

/// A texture of pixels.
fn tex(ctx: &egui::Context, name: &str, px: &Px) -> TextureHandle {
    ctx.load_texture(name, px.color_image(), egui::TextureOptions::LINEAR)
}

/// The preview's layout, zoom and pan.
#[derive(Clone)]
pub struct View {
    /// 0 before and after side by side, 1 the result alone, 2 split.
    pub layout: u8,
    pub zoom: f32,
    pub pan: Vec2,
    /// Zoom to 100% at the next draw.
    pub actual: bool,
}

impl Default for View {
    fn default() -> Self {
        View { layout: 0, zoom: 1.0, pan: Vec2::ZERO, actual: false }
    }
}

fn icon(app: &mut AnyVectorApp, ui: &mut Ui, name: &str, tip: &str, on: bool) -> bool {
    crate::widgets::icon_button(&mut app.icons, ui, name, tip, true, on).clicked()
}

/// Paints an icon that is no button.
fn glyph(app: &mut AnyVectorApp, ui: &Ui, name: &str, at: Pos2, size: f32) {
    if let Some(t) = app.icons.get(ui.ctx(), name, size, crate::icons::Tone::Normal) {
        ui.painter().image(t.id, Rect::from_center_size(at, vec2(size, size)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
}

/// The preview layout buttons (`layouts`) at the left and the zoom buttons at the right.
fn view_tools(app: &mut AnyVectorApp, ui: &mut Ui, v: &mut View, layouts: bool, actual: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if layouts {
            for (k, (i, tip)) in [("pv_side", "Before and after"), ("pv_full", "Full preview"), ("pv_split", "Split preview")].into_iter().enumerate() {
                if icon(app, ui, i, tip, v.layout == k as u8) {
                    v.layout = k as u8;
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            icon(app, ui, "pan", "Pan", true);
            if actual && icon(app, ui, "zoom_actual", "Zoom 100%", false) {
                v.actual = true;
            }
            if icon(app, ui, "zoom_fit", "Zoom to fit", false) {
                v.zoom = 1.0;
                v.pan = Vec2::ZERO;
            }
            if icon(app, ui, "zoom_out", "Zoom out", false) {
                v.zoom = (v.zoom / 1.25).max(0.1);
            }
            if icon(app, ui, "zoom_in", "Zoom in", false) {
                v.zoom = (v.zoom * 1.25).min(16.0);
            }
        });
    });
}

/// Paints an image into a pane at the view's zoom and pan; gives the image rectangle.
fn paint_in(ui: &Ui, v: &View, pane: Rect, t: &TextureHandle, size: Vec2, part: Option<Rect>) -> Rect {
    let fit = (pane.width() / size.x).min(pane.height() / size.y);
    let r = Rect::from_center_size(pane.center() + v.pan, size * fit * v.zoom);
    let clip = part.unwrap_or(pane).intersect(pane);
    ui.painter().with_clip_rect(clip).image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    r
}

/// The preview panes in `rect` (before and after, the result alone, or split); dragging pans.
/// Gives where a click fell on the image (0..1 of its size).
fn panes(ui: &mut Ui, v: &mut View, rect: Rect, before: Option<&TextureHandle>, after: Option<&TextureHandle>, size: Vec2, titles: bool) -> Option<Pos2> {
    let pane_bg = Color32::from_rgb(0x26, 0x26, 0x26);
    let border = Stroke::new(1.0, Color32::from_gray(0x6a));
    let font = egui::FontId::proportional(12.0);
    if v.actual {
        let fit = match v.layout {
            0 => ((rect.width() / 2.0 - 6.0) / size.x).min((rect.height() - 20.0) / size.y),
            _ => (rect.width() / size.x).min(rect.height() / size.y),
        };
        v.zoom = (1.0 / fit.max(1e-3)).clamp(0.1, 16.0);
        v.actual = false;
    }
    ui.painter().rect_stroke(rect, 0.0, border, egui::StrokeKind::Inside);
    let mut list: Vec<(Rect, Option<&TextureHandle>, Option<Rect>)> = vec![];
    match v.layout {
        0 => {
            let top = if titles { 20.0 } else { 0.0 };
            let half = (rect.width() - 6.0) / 2.0;
            let a = Rect::from_min_size(rect.min + vec2(1.0, top + 10.0), vec2(half - 1.0, rect.height() - top - 11.0));
            let b = a.translate(vec2(half + 5.0, 0.0));
            if titles {
                ui.painter().text(pos2(a.center().x, rect.top() + 20.0), egui::Align2::CENTER_CENTER, "Before", font.clone(), theme::TEXT);
                ui.painter().text(pos2(b.center().x, rect.top() + 20.0), egui::Align2::CENTER_CENTER, "After", font.clone(), theme::TEXT);
            }
            list.push((a, before, None));
            list.push((b, after, None));
        }
        1 => {
            let a = rect.shrink(1.0);
            list.push((a, after, None));
        }
        _ => {
            let a = rect.shrink(1.0);
            let mid = a.center().x;
            list.push((a, before, Some(Rect::from_min_max(a.min, pos2(mid, a.max.y)))));
            list.push((a, after, Some(Rect::from_min_max(pos2(mid, a.min.y), a.max))));
        }
    }
    let mut click = None;
    for (k, (pane, t, part)) in list.iter().enumerate() {
        if part.is_none() || k == 0 {
            ui.painter().rect_filled(*pane, 0.0, pane_bg);
            if v.layout == 0 {
                ui.painter().rect_stroke(*pane, 0.0, border, egui::StrokeKind::Outside);
            }
        }
        if let Some(t) = t {
            let r = paint_in(ui, v, *pane, t, size, *part);
            let resp = ui.interact(*pane, ui.scope_id().with(("bm_pane", k)), Sense::click_and_drag());
            if resp.dragged() {
                v.pan += resp.drag_delta();
            }
            if resp.clicked() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let u = (p - r.min) / r.size();
                    if (0.0..=1.0).contains(&u.x) && (0.0..=1.0).contains(&u.y) {
                        click = Some(pos2(u.x, u.y));
                    }
                }
            }
        }
    }
    if v.layout == 2 {
        let a = rect.shrink(1.0);
        ui.painter().vline(a.center().x, a.y_range(), Stroke::new(1.0, Color32::WHITE));
    }
    if v.layout == 1 && titles {
        // the result's title above it
    }
    click
}

/// How a slider looks.
#[derive(Clone, Copy, PartialEq)]
enum Sl {
    /// Blue up to the knob.
    Blue,
    /// Blue up to the knob, with ticks under it.
    BlueTicks,
    /// A plain gray track.
    Plain,
    /// A colored track: (left, middle, right).
    Grad(Color32, Color32, Color32),
    /// A dark line with ticks and a gray knob.
    Dots,
}

const KNOB: Color32 = Color32::from_rgb(0x15, 0x3a, 0x58);
const KNOB_RING: Color32 = Color32::from_rgb(0x2a, 0x6c, 0x9c);
const BLUE: Color32 = Color32::from_rgb(0x1f, 0x7f, 0xd0);

/// A slider `w` wide; true when the value changed.
fn slider(ui: &mut Ui, v: &mut f64, lo: f64, hi: f64, w: f32, style: Sl) -> bool {
    let en = ui.is_enabled();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), if en { Sense::click_and_drag() } else { Sense::hover() });
    let x0 = r.left() + 6.0;
    let x1 = r.right() - 6.0;
    let mut changed = false;
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        let t = ((p.x - x0) / (x1 - x0)).clamp(0.0, 1.0) as f64;
        let nv = lo + t * (hi - lo);
        if (nv - *v).abs() > 1e-9 {
            *v = nv;
            changed = true;
        }
    }
    let t = ((*v - lo) / (hi - lo).max(1e-9)).clamp(0.0, 1.0) as f32;
    let kx = x0 + t * (x1 - x0);
    let ty = r.center().y - if matches!(style, Sl::BlueTicks | Sl::Dots) { 3.0 } else { 0.0 };
    let p = ui.painter();
    let dim = |c: Color32| if en { c } else { c.gamma_multiply(0.45) };
    match style {
        Sl::Blue | Sl::BlueTicks => {
            p.line_segment([pos2(x0, ty), pos2(x1, ty)], Stroke::new(4.0, dim(Color32::from_gray(0x4a))));
            p.line_segment([pos2(x0, ty), pos2(kx, ty)], Stroke::new(4.0, dim(BLUE)));
        }
        Sl::Plain => {
            p.line_segment([pos2(x0, ty), pos2(x1, ty)], Stroke::new(8.0, dim(Color32::from_gray(0x4a))));
        }
        Sl::Grad(a, m, b) => {
            let tr = Rect::from_min_max(pos2(x0, ty - 5.0), pos2(x1, ty + 5.0));
            let mut mesh = egui::Mesh::default();
            let cm = tr.center().x;
            for (xa, xb, ca, cb) in [(tr.left(), cm, a, m), (cm, tr.right(), m, b)] {
                let i = mesh.vertices.len() as u32;
                mesh.colored_vertex(pos2(xa, tr.top()), dim(ca));
                mesh.colored_vertex(pos2(xb, tr.top()), dim(cb));
                mesh.colored_vertex(pos2(xb, tr.bottom()), dim(cb));
                mesh.colored_vertex(pos2(xa, tr.bottom()), dim(ca));
                mesh.add_triangle(i, i + 1, i + 2);
                mesh.add_triangle(i, i + 2, i + 3);
            }
            p.add(egui::Shape::mesh(mesh));
        }
        Sl::Dots => {
            p.line_segment([pos2(x0, ty), pos2(x1, ty)], Stroke::new(4.0, dim(Color32::from_gray(0x16))));
        }
    }
    if matches!(style, Sl::BlueTicks | Sl::Dots) {
        for i in 0..=20 {
            let x = x0 + (x1 - x0) * i as f32 / 20.0;
            let len = if i % 5 == 0 { 4.0 } else { 2.0 };
            p.line_segment([pos2(x, ty + 6.0), pos2(x, ty + 6.0 + len)], Stroke::new(1.0, dim(Color32::from_gray(0xb0))));
        }
    }
    if style == Sl::Dots {
        p.circle_filled(pos2(kx, ty), 5.0, dim(Color32::from_gray(0x9a)));
    } else {
        p.circle_filled(pos2(kx, ty), 6.0, dim(KNOB));
        p.circle_stroke(pos2(kx, ty), 6.0, Stroke::new(1.5, dim(KNOB_RING)));
    }
    changed
}

/// A boxed number shown after a slider.
fn num_box(ui: &mut Ui, v: &mut f64, lo: f64, hi: f64, dec: usize, w: f32) -> bool {
    ui.add_sized(vec2(w, 22.0), egui::DragValue::new(v).range(lo..=hi).max_decimals(dec).min_decimals(dec).speed((hi - lo) / 300.0)).changed()
}

fn spin(ui: &mut Ui, v: &mut f64, lo: f64, hi: f64, step: f64, dec: usize, suffix: &str, w: f32) -> bool {
    crate::dockers::spin(ui, v, lo..=hi, step, dec, suffix, w)
}

fn label_at(ui: &mut Ui, text: &str, w: f32, right: bool) {
    let l = egui::Label::new(egui::RichText::new(text).color(if ui.is_enabled() { theme::TEXT } else { theme::TEXT_DIM }));
    ui.add_sized(vec2(w, 22.0), if right { l.halign(egui::Align::Max) } else { l.halign(egui::Align::Min) });
}

/// A button that looks pressed when `on` (Curves / Overprint, tab buttons).
fn toggle_button(ui: &mut Ui, text: &str, on: bool) -> bool {
    let b = egui::Button::new(egui::RichText::new(text).color(theme::TEXT)).fill(if on { theme::ACCENT } else { Color32::from_rgb(0x3a, 0x3a, 0x3a) }).min_size(vec2(0.0, 22.0));
    ui.add(b).clicked()
}

/// Content placed at a rectangle of the dialog.
fn at<R>(ui: &mut Ui, r: Rect, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope_builder(UiBuilder::new().max_rect(r), f).inner
}

/// The bottom row: optional ?, Reset, then OK and Cancel at the right; (ok, cancel, reset).
fn bottom(ui: &mut Ui, help: bool, reset: bool) -> (bool, bool, bool) {
    let (mut ok, mut cancel, mut rs) = (false, false, false);
    let r = Rect::from_min_max(pos2(ui.max_rect().left(), ui.max_rect().bottom() - 26.0), ui.max_rect().right_bottom());
    at(ui, r, |ui| {
        ui.horizontal(|ui| {
            if help {
                ui.add(egui::Button::new("?").min_size(vec2(24.0, 24.0))).on_hover_text("Help");
                ui.add_space(6.0);
            }
            if reset {
                rs = button(ui, "Reset", true);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cancel = button(ui, "Cancel", true);
                ui.add_space(4.0);
                ok = ok_button(ui, "OK", true);
            });
        });
    });
    (ok, cancel, rs)
}

fn hline(ui: &Ui, x: impl Into<egui::Rangef>, y: f32) {
    ui.painter().hline(x, y, Stroke::new(1.0, theme::SEP));
}

// ------------------------------------------------------------------------------ Convert To Bitmap

const BITMAP_MODES: [&str; 6] = ["Black and White (1-bit)", "16 Colors (4-bit)", "Grayscale (8-bit)", "Paletted (8-bit)", "RGB Color (24-bit)", "CMYK Color (32-bit)"];
const DPIS: [&str; 6] = ["72", "96", "100", "150", "200", "300"];

#[derive(Clone)]
pub struct ConvertState {
    pub dpi: String,
    pub mode: usize,
    pub dithered: bool,
    pub overprint_black: bool,
    pub antialias: bool,
    pub transparent: bool,
}

impl Default for ConvertState {
    fn default() -> Self {
        ConvertState { dpi: "300".into(), mode: 5, dithered: false, overprint_black: false, antialias: true, transparent: true }
    }
}

fn convert(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.bm.convert.take() else { return };
    let size_pt = app.doc().and_then(|d| d.doc.selection_bounds(&d.selection)).map(|b| (b.width(), b.height())).unwrap_or((0.0, 0.0));
    let mut res = (false, false, false);
    let ((), dismissed) = window(ctx, "bm_convert", "Convert To Bitmap", vec2(330.0, 322.0), |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            label_at(ui, "Resolution:", 74.0, false);
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_sized(vec2(60.0, 22.0), egui::TextEdit::singleline(&mut st.dpi));
            let b = ui.add_sized(vec2(18.0, 22.0), egui::Button::new(""));
            crate::widgets::chevron(ui.painter(), b.rect.center(), 3.0, theme::TEXT);
            egui::Popup::menu(&b).show(|ui| {
                ui.set_min_width(80.0);
                for d in DPIS {
                    if ui.selectable_label(st.dpi == d, d).clicked() {
                        st.dpi = d.into();
                    }
                }
            });
            ui.add_space(8.0);
            ui.label("dpi");
        });
        super::group(ui, "Color", |ui| {
            ui.horizontal(|ui| {
                label_at(ui, "Color mode:", 70.0, false);
                super::combo(ui, "bm_cv_mode", &mut st.mode, &BITMAP_MODES, 210.0);
            });
            ui.add_enabled(matches!(st.mode, 0 | 1 | 3), egui::Checkbox::new(&mut st.dithered, "Dithered"));
            ui.checkbox(&mut st.overprint_black, "Always overprint black");
        });
        super::group(ui, "Options", |ui| {
            ui.checkbox(&mut st.antialias, "Anti-aliasing");
            ui.checkbox(&mut st.transparent, "Transparent background");
        });
        ui.add_space(8.0);
        let dpi = st.dpi.trim().parse::<f64>().unwrap_or(300.0).clamp(10.0, 2400.0);
        let px = (size_pt.0 * dpi / 72.0).ceil() * (size_pt.1 * dpi / 72.0).ceil();
        let bytes = px * [0.125, 0.5, 1.0, 1.0, 3.0, 4.0][st.mode];
        ui.label(format!("Uncompressed file size: {:.2} MB", bytes / 1024.0 / 1024.0));
        res = bottom(ui, true, false);
    });
    if res.0 {
        app.dialogs.bm.convert_last = Some(st.clone());
        convert_apply(app, &st);
    } else if !(res.1 || dismissed) {
        app.dialogs.bm.convert = Some(st);
    }
}

fn convert_apply(app: &mut AnyVectorApp, st: &ConvertState) {
    let Some(d) = app.doc() else { return };
    let ids = d.selection.clone();
    let (Some(sd), Some(bb)) = (crate::exec::selection_document(&d.doc, &ids), d.doc.selection_bounds(&ids)) else { return };
    let dpi = st.dpi.trim().parse::<f64>().unwrap_or(300.0).clamp(10.0, 2400.0);
    let bg = if st.transparent { None } else { Some(Color::WHITE) };
    let Some(pm) = vector_render::render_page_image(&sd, 0, dpi, bg, None, st.antialias) else { return };
    let px = Px::from_pixmap(&pm);
    let px = match st.mode {
        0 => bitmaps::one_bit(&px, &bitmaps::OneBit { method: if st.dithered { 4 } else { 0 }, ..Default::default() }),
        1 => {
            let o = bitmaps::Paletted { palette: 7, dither: if st.dithered { 4 } else { 0 }, ..Default::default() };
            let pal = bitmaps::palette_colors(&px, &o, &[], &[]);
            bitmaps::paletted(&px, &o, &pal)
        }
        2 => bitmaps::grayscale(&px),
        3 => {
            let o = bitmaps::Paletted { palette: 6, dither: if st.dithered { 4 } else { 0 }, ..Default::default() };
            let pal = bitmaps::palette_colors(&px, &o, &[], &[]);
            bitmaps::paletted(&px, &o, &pal)
        }
        _ => px,
    };
    let mode = [0u8, 3, 1, 3, 4, 6][st.mode];
    let like = vector_core::document::ImageData { width: 0, height: 0, rgba: Default::default(), encoded: Default::default(), format: "png".into(), name: "Bitmap".into() };
    let data = px.image(&like);
    let overprint = st.overprint_black;
    let Some(d) = app.doc_mut() else { return };
    d.modify("Convert to Bitmap", |doc, sel| {
        let loc = doc.locate(ids[0]);
        for id in &ids {
            doc.remove(*id);
        }
        let mut o = vector_io::image_io::image_object(doc, data, dpi);
        if let ObjKind::Image(i) = &o.kind {
            if mode != bitmaps::MODE_RGB {
                doc.image_modes.insert(i.image, mode);
            }
        }
        o.style.overprint_bitmap = overprint;
        o.apply_transform(Affine::translate(bb.origin().to_vec2()));
        let (p, l, i) = loc.unwrap_or((0, 0, usize::MAX));
        *sel = vec![o.id];
        let list = &mut doc.pages[p].layers[l].objects;
        list.insert(i.min(list.len()), o);
    });
}

// ------------------------------------------------------------------------------ Inflate Bitmap

pub struct InflateState {
    pub w0: usize,
    pub h0: usize,
    pub w: f64,
    pub h: f64,
    pub keep: bool,
}

fn inflate(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.bm.inflate.take() else { return };
    let mut res = (false, false, false);
    let ((), dismissed) = window(ctx, "bm_inflate", "Inflate Bitmap", vec2(462.0, 244.0), |ui| {
        ui.add_space(8.0);
        let (w0, h0) = (st.w0 as f64, st.h0 as f64);
        egui::Grid::new("bm_inflate_grid").num_columns(6).spacing(vec2(12.0, 20.0)).show(ui, |ui| {
            ui.label("");
            ui.label("Original Size");
            ui.label("Inflate to:");
            ui.label("");
            ui.label("Inflate by:");
            ui.end_row();
            let mut pw = st.w / w0 * 100.0;
            let mut ph = st.h / h0 * 100.0;
            ui.label("Width:");
            ui.label(format!("{} (pixels)", st.w0));
            if spin(ui, &mut st.w, w0, w0 * 10.0, 1.0, 0, "", 64.0) && st.keep {
                st.h = (st.w / w0 * h0).round();
            }
            ui.label("pixels");
            if spin(ui, &mut pw, 100.0, 1000.0, 1.0, 0, "", 64.0) {
                st.w = (w0 * pw / 100.0).round();
                if st.keep {
                    st.h = (h0 * pw / 100.0).round();
                }
            }
            ui.label("%");
            ui.end_row();
            ui.label("Height:");
            ui.label(format!("{} (pixels)", st.h0));
            if spin(ui, &mut st.h, h0, h0 * 10.0, 1.0, 0, "", 64.0) && st.keep {
                st.w = (st.h / h0 * w0).round();
            }
            ui.label("pixels");
            if spin(ui, &mut ph, 100.0, 1000.0, 1.0, 0, "", 64.0) {
                st.h = (h0 * ph / 100.0).round();
                if st.keep {
                    st.w = (w0 * ph / 100.0).round();
                }
            }
            ui.label("%");
            ui.end_row();
        });
        ui.add_space(8.0);
        ui.checkbox(&mut st.keep, "Maintain aspect ratio");
        res = bottom(ui, false, false);
    });
    if res.0 {
        inflate_apply(app, st.w as usize, st.h as usize);
    } else if !(res.1 || dismissed) {
        app.dialogs.bm.inflate = Some(st);
    }
}

fn inflate_apply(app: &mut AnyVectorApp, nw: usize, nh: usize) {
    let Some(t) = target(app) else { return };
    let v = t.visible();
    if nw <= v.w && nh <= v.h {
        return;
    }
    let out = bitmaps::inflate(&v, nw, nh);
    let (sx, sy) = (t.io.w / v.w as f64, t.io.h / v.h as f64);
    let (ox, oy) = (((out.w - v.w) / 2) as f64, ((out.h - v.h) / 2) as f64);
    let area = kurbo::Rect::new(-ox * sx, -oy * sy, (out.w as f64 - ox) * sx, (out.h as f64 - oy) * sy);
    replace_bitmap(app, "Inflate Bitmap", t.obj, &out, None, Some(area));
}

// ------------------------------------------------------------------------------ Resample

/// Resample's units and their size in points (pixels use the resolution).
const UNITS: [(&str, f64); 8] = [("inches", 72.0), ("millimeters", 72.0 / 25.4), ("picas, points", 12.0), ("points", 1.0), ("centimeters", 72.0 / 2.54), ("pixels", 0.0), ("ciceros, didots", 12.7889), ("didots", 1.06574)];

pub struct ResampleState {
    pub obj: ObjId,
    /// Original pixels and resolution.
    pub px: (f64, f64),
    pub dpi0: (f64, f64),
    /// New size in points and resolution.
    pub w: f64,
    pub h: f64,
    pub dpi: (f64, f64),
    pub unit: usize,
    pub identical: bool,
    pub mode: usize,
    pub noise: f64,
    pub keep_ratio: bool,
    pub keep_size: bool,
    pub scale: (f64, f64),
}

impl ResampleState {
    fn new(t: &Target) -> ResampleState {
        let v = (t.full.w as f64 * (t.io.crop[2] - t.io.crop[0]), t.full.h as f64 * (t.io.crop[3] - t.io.crop[1]));
        let w = t.io.w * t.scale.0;
        let h = t.io.h * t.scale.1;
        let dpi0 = ((v.0 / (w / 72.0)).round(), (v.1 / (h / 72.0)).round());
        ResampleState { obj: t.obj, px: v, dpi0, w, h, dpi: dpi0, unit: 0, identical: true, mode: 0, noise: 0.0, keep_ratio: true, keep_size: false, scale: t.scale }
    }

    fn new_px(&self) -> (f64, f64) {
        ((self.w / 72.0 * self.dpi.0).round().max(1.0), (self.h / 72.0 * self.dpi.1).round().max(1.0))
    }

    fn per_unit(&self, horiz: bool) -> f64 {
        match UNITS[self.unit].1 {
            0.0 => 72.0 / if horiz { self.dpi.0 } else { self.dpi.1 }.max(1.0),
            u => u,
        }
    }
}

fn kb(px: (f64, f64)) -> String {
    format!("{} KB", ((px.0 * px.1 * 3.0) / 1024.0).round())
}

fn resample(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.bm.resample.take() else { return };
    let mut res = (false, false, false);
    let ((), dismissed) = window(ctx, "bm_resample", "Resample", vec2(462.0, 544.0), |ui| {
        ui.add_space(6.0);
        let section = |ui: &mut Ui, t: &str| {
            ui.horizontal(|ui| {
                ui.label(t);
                let r = ui.available_rect_before_wrap();
                hline(ui, r.left() + 4.0..=r.right(), r.center().y);
            });
        };
        section(ui, "Image size");
        let w0 = st.px.0 / st.dpi0.0 * 72.0;
        let h0 = st.px.1 / st.dpi0.1 * 72.0;
        let mut changed_size = None;
        for horiz in [true, false] {
            ui.horizontal(|ui| {
                label_at(ui, if horiz { "Width:" } else { "Height:" }, 92.0, false);
                let pu = st.per_unit(horiz);
                let (orig, cur) = if horiz { (w0, st.w) } else { (h0, st.h) };
                label_at(ui, &format!("{:.1}", orig / pu), 28.0, true);
                let mut v = cur / pu;
                if spin(ui, &mut v, 0.001, 100000.0, 1.0, 5, "", 86.0) {
                    changed_size = Some((horiz, v * pu));
                }
                ui.add_space(4.0);
                let mut pct = cur / orig * 100.0;
                if spin(ui, &mut pct, 1.0, 10000.0, 1.0, 0, "", 86.0) {
                    changed_size = Some((horiz, orig * pct / 100.0));
                }
                ui.label("%");
                if horiz {
                    ui.add_space(14.0);
                    let names: Vec<&str> = UNITS.iter().map(|u| u.0).collect();
                    super::combo(ui, "bm_rs_unit", &mut st.unit, &names, 86.0);
                }
            });
            ui.add_space(4.0);
        }
        if let Some((horiz, v)) = changed_size {
            let (old_w, old_h) = (st.w, st.h);
            if horiz {
                st.w = v;
                if st.keep_ratio {
                    st.h = v * h0 / w0;
                }
            } else {
                st.h = v;
                if st.keep_ratio {
                    st.w = v * w0 / h0;
                }
            }
            if st.keep_size {
                // the same pixels over the new size: the resolution follows
                st.dpi.0 = (st.dpi.0 * old_w / st.w).max(1.0);
                st.dpi.1 = (st.dpi.1 * old_h / st.h).max(1.0);
            }
        }
        ui.add_space(6.0);
        section(ui, "Resolution");
        let mut dpi_changed = None;
        for horiz in [true, false] {
            ui.horizontal(|ui| {
                label_at(ui, if horiz { "Horizontal:" } else { "Vertical:" }, 150.0, false);
                label_at(ui, &format!("{}", if horiz { st.dpi0.0 } else { st.dpi0.1 }), 28.0, true);
                let mut v = if horiz { st.dpi.0 } else { st.dpi.1 };
                if spin(ui, &mut v, 1.0, 10000.0, 1.0, 0, "", 80.0) {
                    dpi_changed = Some((horiz, v));
                }
                ui.label("dpi");
                if horiz {
                    ui.add_space(14.0);
                    ui.add_enabled(!st.keep_ratio, egui::Checkbox::new(&mut st.identical, "Identical values"));
                }
            });
            ui.add_space(4.0);
        }
        if let Some((horiz, v)) = dpi_changed {
            let same = st.identical || st.keep_ratio;
            if horiz || same {
                st.dpi.0 = v;
            }
            if !horiz || same {
                st.dpi.1 = v;
            }
            if st.keep_size {
                st.w = st.px.0 / st.dpi.0 * 72.0;
                st.h = st.px.1 / st.dpi.1 * 72.0;
            }
        }
        ui.add_space(6.0);
        section(ui, "Mode");
        for (k, a, b) in [(0, "Bicubic", "Illustration ✦"), (1, "Nearest neighbor", "Photorealistic ✦")] {
            ui.horizontal(|ui| {
                let r = ui.radio(st.mode == k, a);
                if r.clicked() {
                    st.mode = k;
                }
                ui.add_space((220.0 - r.rect.width()).max(0.0));
                ui.add_enabled(false, egui::RadioButton::new(st.mode == k + 2, b));
            });
        }
        ui.add_enabled_ui(st.mode >= 2, |ui| {
            ui.horizontal(|ui| {
                label_at(ui, "Noise reduction:", 96.0, false);
                slider(ui, &mut st.noise, 0.0, 100.0, 270.0, Sl::Dots);
            });
            ui.horizontal(|ui| {
                ui.add_space(96.0);
                label_at(ui, "None", 40.0, false);
                ui.add_space(170.0);
                label_at(ui, "Maximum", 60.0, true);
            });
        });
        ui.add_space(4.0);
        let r = ui.available_rect_before_wrap();
        hline(ui, r.x_range(), r.top() + 4.0);
        ui.add_space(14.0);
        let np = st.new_px();
        egui::Grid::new("bm_rs_info").num_columns(3).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
            label_at(ui, "Original file size:", 100.0, false);
            label_at(ui, &kb(st.px), 96.0, false);
            ui.checkbox(&mut st.keep_ratio, "Maintain aspect ratio");
            ui.end_row();
            label_at(ui, "New file size:", 100.0, false);
            label_at(ui, &kb(np), 96.0, false);
            ui.checkbox(&mut st.keep_size, "Maintain original file size");
            ui.end_row();
        });
        ui.add_space(6.0);
        egui::Grid::new("bm_rs_px").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
            label_at(ui, "Original pixel size:", 100.0, false);
            label_at(ui, &format!("{} x {} px", st.px.0, st.px.1), 120.0, false);
            ui.end_row();
            label_at(ui, "New pixel size:", 100.0, false);
            label_at(ui, &format!("{} x {} px", np.0, np.1), 120.0, false);
            ui.end_row();
        });
        let y = ui.max_rect().bottom() - 40.0;
        hline(ui, ui.max_rect().x_range(), y);
        res = bottom(ui, true, true);
    });
    if res.2 {
        if let Some(t) = target(app) {
            st = ResampleState::new(&t);
        }
    }
    if res.0 {
        resample_apply(app, &st);
    } else if !(res.1 || dismissed) {
        app.dialogs.bm.resample = Some(st);
    }
}

fn resample_apply(app: &mut AnyVectorApp, st: &ResampleState) {
    let Some(t) = target(app).filter(|t| t.obj == st.obj) else { return };
    let v = t.visible();
    let (nw, nh) = st.new_px();
    let Some(buf) = image::RgbaImage::from_raw(v.w as u32, v.h as u32, v.p.iter().flat_map(|c| *c).collect()) else { return };
    let filter = if st.mode == 1 { image::imageops::FilterType::Nearest } else { image::imageops::FilterType::CatmullRom };
    let out = image::imageops::resize(&buf, nw as u32, nh as u32, filter);
    let px = Px { w: out.width() as usize, h: out.height() as usize, p: out.pixels().map(|p| p.0).collect() };
    let area = kurbo::Rect::new(0.0, 0.0, st.w / st.scale.0.max(1e-9), st.h / st.scale.1.max(1e-9));
    replace_bitmap(app, "Resample", t.obj, &px, None, Some(area));
}

// ------------------------------------------------------------------------------ previews

/// A dialog's preview: the bitmap made small, its texture, and the result's texture with the
/// settings it shows.
pub struct Preview<S> {
    pub src: Px,
    pub before: Option<TextureHandle>,
    pub after: Option<TextureHandle>,
    pub shown: Option<S>,
    pub view: View,
}

impl<S: Clone + PartialEq> Preview<S> {
    fn new(t: &Target, max: usize) -> Preview<S> {
        Preview { src: t.visible().fit(max), before: None, after: None, shown: None, view: View::default() }
    }

    /// Makes the textures, the result again when the settings changed.
    fn update(&mut self, ctx: &egui::Context, name: &str, s: &S, make: impl FnOnce(&Px) -> Px) {
        if self.before.is_none() {
            self.before = Some(tex(ctx, &format!("{name}_before"), &self.src));
        }
        if self.shown.as_ref() != Some(s) {
            let out = make(&self.src);
            self.after = Some(tex(ctx, &format!("{name}_after"), &out));
            self.shown = Some(s.clone());
        }
    }

    fn size(&self) -> Vec2 {
        vec2(self.src.w as f32, self.src.h as f32)
    }
}

// ------------------------------------------------------------------------------ Convert to 1 bit

const LINE_UNITS: [(&str, f64); 8] = [("inch", 72.0), ("millimeter", 72.0 / 25.4), ("pica", 12.0), ("point", 1.0), ("centimeter", 72.0 / 2.54), ("pixel", 0.0), ("cicero", 12.7889), ("didot", 1.06574)];

pub struct OneBitState {
    pub o: bitmaps::OneBit,
    /// Line Art threshold.
    pub threshold: f64,
    pub unit: usize,
    /// The screen frequency in lines per `unit`.
    pub lines: f64,
    pub pv: Preview<(bitmaps::OneBit, u64)>,
}

impl OneBitState {
    fn new(t: &Target) -> OneBitState {
        let o = bitmaps::OneBit { ppi: t.ppi, ..Default::default() };
        OneBitState { lines: o.lpi, o, threshold: 128.0, unit: 0, pv: Preview::new(t, 420) }
    }

    /// The settings with the frequency per inch and the threshold folded in.
    fn settings(&self) -> bitmaps::OneBit {
        let mut o = self.o.clone();
        let per = match LINE_UNITS[self.unit].1 {
            0.0 => 72.0 / o.ppi.max(1.0),
            u => u,
        };
        o.lpi = self.lines * 72.0 / per;
        if o.method == 0 {
            // Line Art: the threshold is the level that becomes white
            o.intensity = 100.0 + (128.0 - self.threshold) / 1.27;
        }
        o
    }
}

fn one_bit(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.bm.one_bit.take() else { return };
    let s = st.settings();
    // the preview is smaller than the bitmap: its screen is scaled with it
    let k = st.pv.src.w as f64 / target(app).map(|t| t.visible().w).unwrap_or(st.pv.src.w).max(1) as f64;
    let mut ps = s.clone();
    ps.ppi *= k;
    st.pv.update(ctx, "bm_1bit", &(s.clone(), 0), |px| bitmaps::one_bit(px, &ps));
    let mut res = (false, false, false);
    let ((), dismissed) = window(ctx, "bm_1bit", "Convert to 1 bit", vec2(569.0, 612.0), |ui| {
        ui.add_space(4.0);
        view_tools(app, ui, &mut st.pv.view, true, false);
        let top = ui.cursor().top() + 4.0;
        let r = Rect::from_min_max(pos2(ui.max_rect().left() + 5.0, top), pos2(ui.max_rect().right() - 5.0, top + 298.0));
        let size = st.pv.size();
        let (b, a) = (st.pv.before.clone(), st.pv.after.clone());
        panes(ui, &mut st.pv.view, r, b.as_ref(), a.as_ref(), size, true);
        ui.allocate_rect(r, Sense::hover());
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            label_at(ui, "Conversion method:", 134.0, false);
            super::combo(ui, "bm_1bit_m", &mut st.o.method, &bitmaps::ONE_BIT_METHODS, 186.0);
        });
        super::group(ui, "Options", |ui| {
            let half = st.o.method == 5;
            if st.o.method == 0 {
                ui.horizontal(|ui| {
                    label_at(ui, "Threshold:", 70.0, false);
                    slider(ui, &mut st.threshold, 0.0, 255.0, 400.0, Sl::Dots);
                    num_box(ui, &mut st.threshold, 0.0, 255.0, 0, 36.0);
                });
            } else {
                ui.add_enabled_ui(!half, |ui| {
                    ui.horizontal(|ui| {
                        label_at(ui, "Intensity:", 70.0, false);
                        slider(ui, &mut st.o.intensity, 0.0, 100.0, 400.0, Sl::Dots);
                        num_box(ui, &mut st.o.intensity, 0.0, 100.0, 0, 36.0);
                    });
                });
            }
            if half {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    label_at(ui, "Screen type:", 90.0, false);
                    super::combo(ui, "bm_1bit_screen", &mut st.o.screen, &bitmaps::SCREENS, 66.0);
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    spin(ui, &mut st.o.angle, -360.0, 360.0, 1.0, 0, "", 80.0);
                    ui.add_space(8.0);
                    ui.label("degrees");
                    ui.add_space(24.0);
                    spin(ui, &mut st.lines, 1.0, 1000.0, 1.0, 0, "", 80.0);
                    ui.add_space(8.0);
                    ui.label("lines per");
                    ui.add_space(24.0);
                    let names: Vec<&str> = LINE_UNITS.iter().map(|u| u.0).collect();
                    super::combo(ui, "bm_1bit_unit", &mut st.unit, &names, 66.0);
                });
            }
        });
        res = bottom(ui, true, true);
    });
    if res.2 {
        let keep = (st.pv.src.clone(), st.o.ppi);
        st.o = bitmaps::OneBit { ppi: keep.1, ..Default::default() };
        st.threshold = 128.0;
        st.lines = st.o.lpi;
        st.unit = 0;
    }
    if res.0 {
        if let Some(t) = target(app) {
            let px = bitmaps::one_bit(&t.full, &st.settings());
            replace_bitmap(app, "Convert to 1 bit", t.obj, &px, Some(0), None);
        }
    } else if !(res.1 || dismissed) {
        app.dialogs.bm.one_bit = Some(st);
    }
}

// ------------------------------------------------------------------------------ Duotone

const DUO_TYPES: [&str; 4] = ["Monotone", "Duotone", "Tritone", "Quadtone"];

pub struct DuotoneState {
    pub d: bitmaps::Duotone,
    pub tab: usize,
    pub sel: usize,
    pub show_all: bool,
    pub drag: Option<usize>,
    pub pv: Preview<bitmaps::Duotone>,
}

impl DuotoneState {
    fn new(t: &Target) -> DuotoneState {
        DuotoneState { d: Default::default(), tab: 0, sel: 0, show_all: false, drag: None, pv: Preview::new(t, 420) }
    }
}

fn c32(c: Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

fn duotone(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.bm.duotone.take() else { return };
    let d = st.d.clone();
    st.pv.update(ctx, "bm_duo", &d, |px| bitmaps::duotone(&bitmaps::grayscale(px), &d));
    let mut res = (false, false, false);
    let ((), dismissed) = window(ctx, "bm_duo", "Duotone", vec2(561.0, 712.0), |ui| {
        ui.add_space(4.0);
        view_tools(app, ui, &mut st.pv.view, true, false);
        let top = ui.cursor().top() + 4.0;
        let r = Rect::from_min_max(pos2(ui.max_rect().left() + 5.0, top), pos2(ui.max_rect().right() - 5.0, top + 298.0));
        let size = st.pv.size();
        let (b, a) = (st.pv.before.clone(), st.pv.after.clone());
        panes(ui, &mut st.pv.view, r, b.as_ref(), a.as_ref(), size, true);
        ui.allocate_rect(r, Sense::hover());
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(24.0);
            if toggle_button(ui, "Curves", st.tab == 0) {
                st.tab = 0;
            }
            if toggle_button(ui, "Overprint", st.tab == 1) {
                st.tab = 1;
            }
        });
        let body_top = ui.cursor().top() + 6.0;
        let left = Rect::from_min_max(pos2(ui.max_rect().left() + 24.0, body_top), pos2(ui.max_rect().left() + 282.0, body_top + 220.0));
        let curve = Rect::from_min_size(pos2(ui.max_rect().left() + 296.0, body_top + 4.0), vec2(210.0, 212.0));
        at(ui, left, |ui| {
            if st.tab == 0 {
                ui.horizontal(|ui| {
                    label_at(ui, "Type:", 44.0, false);
                    let mut k = st.d.kind;
                    if super::combo(ui, "bm_duo_type", &mut k, &DUO_TYPES, 186.0) {
                        st.d = bitmaps::Duotone { kind: k, inks: bitmaps::Duotone::inks_for(k) };
                        st.sel = 0;
                    }
                });
                ui.add_space(2.0);
                for (i, ink) in st.d.inks.clone().iter().enumerate() {
                    let (rr, resp) = ui.allocate_exact_size(vec2(250.0, 20.0), Sense::click());
                    ui.painter().rect_filled(rr, 0.0, if st.sel == i { Color32::from_gray(0x48) } else { Color32::from_gray(0x33) });
                    let sw = Rect::from_min_size(rr.min + vec2(6.0, 3.0), vec2(14.0, 14.0));
                    ui.painter().rect_filled(sw, 0.0, c32(ink.1));
                    ui.painter().rect_stroke(sw, 0.0, Stroke::new(1.0, Color32::from_gray(0x80)), egui::StrokeKind::Inside);
                    ui.painter().text(pos2(rr.left() + 32.0, rr.center().y), egui::Align2::LEFT_CENTER, &ink.0, egui::FontId::proportional(12.0), theme::TEXT);
                    if resp.clicked() {
                        st.sel = i;
                    }
                }
            } else {
                ui.label("Overprint colors:");
                ui.add_space(4.0);
                let inks = st.d.inks.clone();
                for i in 0..inks.len() {
                    for j in i + 1..inks.len() {
                        let (a, b) = (inks[i].1, inks[j].1);
                        let m = Color::rgb((a.r as u16 * b.r as u16 / 255) as u8, (a.g as u16 * b.g as u16 / 255) as u8, (a.b as u16 * b.b as u16 / 255) as u8);
                        ui.horizontal(|ui| {
                            let (sw, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                            ui.painter().rect_filled(sw, 0.0, c32(m));
                            ui.label(format!("{} + {}", inks[i].0, inks[j].0));
                        });
                    }
                }
                if inks.len() < 2 {
                    super::note(ui, "A monotone prints with one ink: there are no overprints.");
                }
            }
        });
        // the buttons under the ink list
        let br = Rect::from_min_size(pos2(left.left(), body_top + 150.0), vec2(250.0, 60.0));
        at(ui, br, |ui| {
            ui.horizontal(|ui| {
                let sel = st.sel.min(st.d.inks.len().saturating_sub(1));
                let mut c = st.d.inks.get(sel).map(|i| i.1);
                let resp = ui.add_sized(vec2(78.0, 24.0), egui::Button::new("Edit"));
                egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                    if crate::dockers::color_pick_w(ui, "bm_duo_ink", &mut c, false, true, 200.0) {
                        if let (Some(ink), Some(c)) = (st.d.inks.get_mut(sel), c) {
                            ink.1 = c;
                            ink.0 = c.to_hex();
                        }
                    }
                });
                if ui.add_sized(vec2(78.0, 24.0), egui::Button::new("Null")).clicked() {
                    if let Some(ink) = st.d.inks.get_mut(sel) {
                        ink.2 = vec![(0.0, 0.0), (255.0, 0.0)];
                    }
                }
                ui.checkbox(&mut st.show_all, "Show All");
            });
            ui.horizontal(|ui| {
                if ui.add_sized(vec2(78.0, 24.0), egui::Button::new("Load")).clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("Duotone curves", &["txt"]).pick_file() {
                        if let Ok(t) = std::fs::read_to_string(p) {
                            load_curves(&mut st.d, &t);
                        }
                    }
                }
                if ui.add_sized(vec2(78.0, 24.0), egui::Button::new("Save")).clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("Duotone curves", &["txt"]).set_file_name("duotone.txt").save_file() {
                        let _ = std::fs::write(p, save_curves(&st.d));
                    }
                }
            });
        });
        // the curve of the selected ink (all of them with Show All)
        let p = ui.painter();
        p.rect_filled(curve, 0.0, Color32::WHITE);
        for i in 1..10 {
            let f = i as f32 / 10.0;
            p.vline(curve.left() + curve.width() * f, curve.y_range(), Stroke::new(1.0, Color32::from_gray(0xd0)));
            p.hline(curve.x_range(), curve.top() + curve.height() * f, Stroke::new(1.0, Color32::from_gray(0xd0)));
        }
        let to_screen = |q: (f32, f32)| pos2(curve.left() + q.0 / 255.0 * curve.width(), curve.bottom() - q.1 / 255.0 * curve.height());
        let sel = st.sel.min(st.d.inks.len().saturating_sub(1));
        for (i, ink) in st.d.inks.iter().enumerate() {
            if i != sel && !st.show_all {
                continue;
            }
            let lut = vector_render::bitfx::curve_lut(&ink.2);
            let pts: Vec<Pos2> = (0..=255).step_by(3).map(|x| to_screen((x as f32, lut[x]))).collect();
            let col = if i == sel { Color32::from_gray(0x30) } else { c32(ink.1) };
            ui.painter().add(egui::Shape::line(pts, Stroke::new(if i == sel { 1.6 } else { 1.0 }, col)));
            if i == sel {
                for q in &ink.2 {
                    ui.painter().circle_filled(to_screen(*q), 3.0, Color32::from_gray(0x20));
                }
            }
        }
        let resp = ui.interact(curve, ui.scope_id().with("bm_duo_curve"), Sense::click_and_drag());
        if let (Some(ink), Some(pp)) = (st.d.inks.get_mut(sel), resp.interact_pointer_pos()) {
            let q = (((pp.x - curve.left()) / curve.width() * 255.0).clamp(0.0, 255.0), ((curve.bottom() - pp.y) / curve.height() * 255.0).clamp(0.0, 255.0));
            if resp.drag_started() || resp.clicked() {
                st.drag = ink.2.iter().position(|c| (to_screen(*c) - pp).length() < 7.0).or_else(|| {
                    ink.2.push(q);
                    Some(ink.2.len() - 1)
                });
            }
            if resp.dragged() {
                if let Some(k) = st.drag.filter(|k| *k < ink.2.len()) {
                    // the end points keep their place across
                    let x = if k < 2 && (ink.2[k].0 == 0.0 || ink.2[k].0 == 255.0) { ink.2[k].0 } else { q.0 };
                    ink.2[k] = (x, q.1);
                }
            }
        }
        if resp.drag_stopped() {
            st.drag = None;
        }
        // the tones the selected ink prints
        let bar = Rect::from_min_size(pos2(curve.left() - 7.0, curve.bottom() + 14.0), vec2(224.0, 17.0));
        if let Some(ink) = st.d.inks.get(sel) {
            let mut mesh = egui::Mesh::default();
            let i0 = mesh.vertices.len() as u32;
            let dark = c32(ink.1);
            mesh.colored_vertex(bar.left_top(), Color32::WHITE);
            mesh.colored_vertex(bar.right_top(), dark);
            mesh.colored_vertex(bar.right_bottom(), dark);
            mesh.colored_vertex(bar.left_bottom(), Color32::WHITE);
            mesh.add_triangle(i0, i0 + 1, i0 + 2);
            mesh.add_triangle(i0, i0 + 2, i0 + 3);
            ui.painter().add(egui::Shape::mesh(mesh));
        }
        ui.allocate_rect(Rect::from_min_max(pos2(ui.max_rect().left(), body_top), bar.max), Sense::hover());
        res = bottom(ui, false, true);
    });
    if res.2 {
        st.d = bitmaps::Duotone::default();
        st.sel = 0;
    }
    if res.0 {
        if let Some(t) = target(app) {
            let px = bitmaps::duotone(&bitmaps::grayscale(&t.full), &st.d);
            replace_bitmap(app, "Convert to Duotone", t.obj, &px, Some(2), None);
        }
    } else if !(res.1 || dismissed) {
        app.dialogs.bm.duotone = Some(st);
    }
}

/// Duotone curve files: one line per ink, `name;#rrggbb;x,y x,y ...`.
fn save_curves(d: &bitmaps::Duotone) -> String {
    d.inks.iter().map(|(n, c, pts)| format!("{n};{};{}", c.to_hex(), pts.iter().map(|(x, y)| format!("{x},{y}")).collect::<Vec<_>>().join(" "))).collect::<Vec<_>>().join("\n")
}

fn load_curves(d: &mut bitmaps::Duotone, t: &str) {
    let mut inks = vec![];
    for line in t.lines() {
        let mut f = line.split(';');
        let (Some(n), Some(c), Some(p)) = (f.next(), f.next(), f.next()) else { continue };
        let Some(c) = Color::from_hex(c) else { continue };
        let pts: Vec<(f32, f32)> = p.split_whitespace().filter_map(|q| q.split_once(',')).filter_map(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))).collect();
        inks.push((n.to_string(), c, pts));
    }
    if !inks.is_empty() {
        inks.truncate(4);
        d.kind = inks.len() - 1;
        d.inks = inks;
    }
}

// ------------------------------------------------------------------------------ Convert to Paletted

const PAL_PRESETS: [&str; 4] = ["Default", "8 bits(256 colors)", "4 bits(16 colors)", "2 bits(4 colors)"];

#[derive(Clone, PartialEq)]
pub struct PalSettings {
    pub o: bitmaps::Paletted,
    pub range: bool,
    pub range_color: Color,
    /// Importance, Lightness, a, b.
    pub sens: [f64; 4],
}

pub struct PalettedState {
    pub s: PalSettings,
    pub tab: usize,
    pub preset: usize,
    pub palette: Vec<[u8; 3]>,
    pub pv: Preview<PalSettings>,
    pub doc_pal: Vec<Color>,
    pub styles: Vec<Color>,
}

impl PalettedState {
    fn new(t: &Target) -> PalettedState {
        let s = PalSettings { o: Default::default(), range: false, range_color: Color::rgb(0x45, 0x63, 0x8a), sens: [100.0; 4] };
        PalettedState { s, tab: 0, preset: 0, palette: vec![], pv: Preview::new(t, 420), doc_pal: vec![], styles: vec![] }
    }
}

/// The palette for the settings: with a sensitivity color, more of the palette goes near it.
fn pal_colors(px: &Px, s: &PalSettings, doc: &[Color], styles: &[Color]) -> Vec<[u8; 3]> {
    if s.range && s.o.palette == 3 {
        // weigh the colors near the range color more: add copies of them to the samples
        let rc = s.range_color;
        let near: Vec<[u8; 4]> = px.p.iter().filter(|c| (c[0] as f64 - rc.r as f64).abs() + (c[1] as f64 - rc.g as f64).abs() + (c[2] as f64 - rc.b as f64).abs() < 90.0 * s.sens[1] / 100.0).copied().collect();
        let mut more = px.clone();
        let copies = (s.sens[0] / 25.0).round() as usize;
        for _ in 0..copies {
            more.p.extend(near.iter().copied());
        }
        more.h = more.p.len() / more.w.max(1);
        more.p.truncate(more.w * more.h);
        return bitmaps::palette_colors(&more, &s.o, doc, styles);
    }
    bitmaps::palette_colors(px, &s.o, doc, styles)
}

fn paletted(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.bm.paletted.take() else { return };
    if let Some(d) = app.doc() {
        st.doc_pal = d.doc.palette.iter().map(|c| c.color).collect();
        st.styles = d.doc.color_styles.iter().map(|c| c.color).collect();
    }
    let s = st.s.clone();
    let (dp, sy) = (st.doc_pal.clone(), st.styles.clone());
    let mut made = vec![];
    st.pv.update(ctx, "bm_pal", &s, |px| {
        made = pal_colors(px, &s, &dp, &sy);
        bitmaps::paletted(px, &s.o, &made)
    });
    if !made.is_empty() {
        st.palette = made;
    }
    let mut res = (false, false, false);
    let ((), dismissed) = window(ctx, "bm_pal", "Convert to Paletted", vec2(714.0, 739.0), |ui| {
        ui.add_space(4.0);
        let inner = Rect::from_min_max(pos2(ui.max_rect().left() + 30.0, ui.cursor().top()), pos2(ui.max_rect().right() - 30.0, ui.max_rect().bottom()));
        at(ui, inner, |ui| {
            view_tools(app, ui, &mut st.pv.view, true, false);
            let top = ui.cursor().top() + 4.0;
            let r = Rect::from_min_max(pos2(ui.max_rect().left(), top), pos2(ui.max_rect().right(), top + 282.0));
            let size = st.pv.size();
            let (b, a) = (st.pv.before.clone(), st.pv.after.clone());
            panes(ui, &mut st.pv.view, r, b.as_ref(), a.as_ref(), size, true);
            ui.allocate_rect(r, Sense::hover());
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                for (k, n) in ["Options", "Range Sensitivity", "Processed Palette"].into_iter().enumerate() {
                    if toggle_button(ui, n, st.tab == k) {
                        st.tab = k;
                    }
                }
            });
            let box_r = Rect::from_min_size(pos2(ui.max_rect().left(), ui.cursor().top()), vec2(ui.max_rect().width(), 252.0));
            ui.painter().rect_stroke(box_r, 0.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Inside);
            at(ui, box_r.shrink2(vec2(26.0, 18.0)), |ui| match st.tab {
                0 => {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        label_at(ui, "Smoothing:", 104.0, false);
                        slider(ui, &mut st.s.o.smoothing, 0.0, 100.0, 300.0, Sl::Dots);
                        num_box(ui, &mut st.s.o.smoothing, 0.0, 100.0, 0, 34.0);
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        label_at(ui, "Palette:", 104.0, false);
                        let mut p = st.s.o.palette;
                        if super::combo(ui, "bm_pal_p", &mut p, &bitmaps::PALETTES, 120.0) {
                            if p == 8 {
                                // More Palettes…: a palette file
                                if let Some(f) = rfd::FileDialog::new().add_filter("Palettes", &["xml", "gpl", "txt", "cpl"]).pick_file() {
                                    if let Ok(t) = std::fs::read_to_string(&f) {
                                        let pal = vector_core::color::Palette::parse("palette", &t);
                                        st.doc_pal = pal.colors.iter().map(|c| c.color).collect();
                                        st.s.o.palette = 0;
                                    }
                                }
                            } else {
                                st.s.o.palette = p;
                            }
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        label_at(ui, "Dithering", 104.0, false);
                        super::combo(ui, "bm_pal_d", &mut st.s.o.dither, &bitmaps::DITHERS, 120.0);
                    });
                    ui.add_space(4.0);
                    ui.add_enabled_ui(st.s.o.dither > 0, |ui| {
                        ui.horizontal(|ui| {
                            label_at(ui, "Dither Intensity:", 112.0, false);
                            slider(ui, &mut st.s.o.intensity, 0.0, 100.0, 300.0, Sl::Dots);
                            num_box(ui, &mut st.s.o.intensity, 0.0, 100.0, 0, 34.0);
                        });
                    });
                    ui.add_space(4.0);
                    ui.add_enabled_ui(matches!(st.s.o.palette, 2..=4), |ui| {
                        ui.horizontal(|ui| {
                            label_at(ui, "Colors:", 104.0, false);
                            let mut c = st.s.o.colors as f64;
                            if spin(ui, &mut c, 2.0, 256.0, 1.0, 0, "", 50.0) {
                                st.s.o.colors = c as usize;
                            }
                        });
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(st.s.o.palette == 3, |ui| {
                            ui.add_sized(vec2(330.0, 22.0), egui::Checkbox::new(&mut st.s.range, "Color range sensivity to:"));
                            let mut c = Some(st.s.range_color);
                            if crate::dockers::color_pick_w(ui, "bm_pal_range", &mut c, false, st.s.range, 140.0) {
                                if let Some(c) = c {
                                    st.s.range_color = c;
                                }
                            }
                            ui.add_space(12.0);
                            if ui.add_enabled(st.s.range, egui::Button::new("Reset").min_size(vec2(80.0, 24.0))).clicked() {
                                st.s.range_color = Color::rgb(0x45, 0x63, 0x8a);
                                st.s.sens = [100.0; 4];
                            }
                        });
                    });
                }
                1 => {
                    ui.add_space(12.0);
                    ui.add_enabled_ui(st.s.range && st.s.o.palette == 3, |ui| {
                        for (k, n) in ["Importance:", "Lightness:", "a:", "b:"].into_iter().enumerate() {
                            ui.horizontal(|ui| {
                                label_at(ui, n, 104.0, false);
                                slider(ui, &mut st.s.sens[k], 0.0, 200.0, 300.0, Sl::Dots);
                                num_box(ui, &mut st.s.sens[k], 0.0, 200.0, 0, 34.0);
                            });
                            ui.add_space(6.0);
                        }
                    });
                }
                _ => {
                    // the colors of the converted bitmap
                    let cell = 13.0;
                    let cols = ((ui.available_width() / cell) as usize).max(1);
                    let start = ui.cursor().min + vec2(0.0, 6.0);
                    for (i, c) in st.palette.iter().enumerate() {
                        let r = Rect::from_min_size(start + vec2((i % cols) as f32 * cell, (i / cols) as f32 * cell), vec2(cell - 1.0, cell - 1.0));
                        ui.painter().rect_filled(r, 0.0, Color32::from_rgb(c[0], c[1], c[2]));
                    }
                    ui.label(format!("{} colors", st.palette.len()));
                }
            });
            ui.allocate_rect(box_r, Sense::hover());
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                label_at(ui, "Presets:", 56.0, false);
                let names: Vec<String> = PAL_PRESETS.iter().map(|s| s.to_string()).chain(app.dialogs.bm.paletted_presets.iter().map(|p| p.0.clone())).collect();
                let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                if super::combo(ui, "bm_pal_preset", &mut st.preset, &refs, 136.0) {
                    st.s.o = match st.preset {
                        0 => Default::default(),
                        1 => bitmaps::Paletted { palette: 2, colors: 256, ..Default::default() },
                        2 => bitmaps::Paletted { palette: 2, colors: 16, ..Default::default() },
                        3 => bitmaps::Paletted { palette: 2, colors: 4, ..Default::default() },
                        k => app.dialogs.bm.paletted_presets.get(k - PAL_PRESETS.len()).map(|p| p.1.clone()).unwrap_or_default(),
                    };
                }
                if icon(app, ui, "plus", "Add preset", false) {
                    let n = format!("Preset {}", app.dialogs.bm.paletted_presets.len() + 1);
                    app.dialogs.bm.paletted_presets.push((n, st.s.o.clone()));
                    st.preset = PAL_PRESETS.len() + app.dialogs.bm.paletted_presets.len() - 1;
                }
                if icon(app, ui, "minus", "Delete preset", false) && st.preset >= PAL_PRESETS.len() {
                    app.dialogs.bm.paletted_presets.remove(st.preset - PAL_PRESETS.len());
                    st.preset = 0;
                }
            });
        });
        res = bottom(ui, true, true);
    });
    if res.2 {
        st.s = PalSettings { o: Default::default(), range: false, range_color: Color::rgb(0x45, 0x63, 0x8a), sens: [100.0; 4] };
        st.preset = 0;
    }
    if res.0 {
        if let Some(t) = target(app) {
            let pal = pal_colors(&t.full, &st.s, &st.doc_pal, &st.styles);
            let px = bitmaps::paletted(&t.full, &st.s.o, &pal);
            replace_bitmap(app, "Convert to Paletted", t.obj, &px, Some(3), None);
        }
    } else if !(res.1 || dismissed) {
        app.dialogs.bm.paletted = Some(st);
    }
}

// ------------------------------------------------------------------------------ Straighten Image

pub struct StraightenState {
    pub o: bitmaps::Straighten,
    pub grid: bool,
    pub grid_size: f64,
    pub grid_color: Color,
    pub preview: bool,
    pub remember: bool,
    pub pv: Preview<(bitmaps::Straighten, bool)>,
}

impl StraightenState {
    fn new(t: &Target, o: bitmaps::Straighten) -> StraightenState {
        let mut pv = Preview::new(t, 480);
        pv.view.layout = 1;
        StraightenState { o, grid: true, grid_size: 50.0, grid_color: Color::rgb(0xcc, 0xcc, 0xff), preview: true, remember: true, pv }
    }
}

/// A labelled slider row of the Straighten and Lab dialogs, with its number box.
#[allow(clippy::too_many_arguments)]
fn value_row(ui: &mut Ui, label: &str, lw: f32, v: &mut f64, lo: f64, hi: f64, dec: usize, unit: &str, sw: f32, style: Sl) -> bool {
    ui.horizontal(|ui| {
        label_at(ui, label, lw, true);
        ui.add_space(8.0);
        let a = slider(ui, v, lo, hi, sw, style);
        ui.add_space(8.0);
        let b = spin(ui, v, lo, hi, if hi - lo > 1000.0 { 50.0 } else { 1.0 }, dec, unit, 60.0);
        a || b
    })
    .inner
}

fn straighten(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.bm.straighten.take() else { return };
    let o = st.o.clone();
    let shown = (bitmaps::Straighten { crop: false, crop_resample: false, ..o.clone() }, st.preview);
    let on = st.preview;
    st.pv.update(ctx, "bm_straighten", &shown, |px| if on { bitmaps::straighten(px, &shown.0) } else { px.clone() });
    let mut res = (false, false, false);
    let ((), dismissed) = window(ctx, "bm_straighten", "Straighten Image", vec2(917.0, 529.0), |ui| {
        let left = ui.max_rect().left();
        let top = ui.max_rect().top();
        // zoom tools and the preview with its grid
        at(ui, Rect::from_min_size(pos2(left + 230.0, top + 30.0), vec2(160.0, 26.0)), |ui| view_tools(app, ui, &mut st.pv.view, false, true));
        let pr = Rect::from_min_size(pos2(left + 40.0, top + 63.0), vec2(350.0, 348.0));
        let size = st.pv.size();
        let a = st.pv.after.clone();
        st.pv.view.layout = 1;
        panes(ui, &mut st.pv.view, pr, None, a.as_ref(), size, false);
        if st.grid {
            let cell = 8.0 + st.grid_size as f32 / 100.0 * 52.0;
            let gc = c32(st.grid_color);
            let mut x = pr.left() + cell;
            while x < pr.right() {
                ui.painter().vline(x, pr.y_range(), Stroke::new(1.0, gc));
                x += cell;
            }
            let mut y = pr.top() + cell;
            while y < pr.bottom() {
                ui.painter().hline(pr.x_range(), y, Stroke::new(1.0, gc));
                y += cell;
            }
        }
        // the crop the result keeps
        if st.o.crop || st.o.crop_resample {
            let share = st.pv.src.w as f32 / st.pv.src.w as f32;
            let _ = share;
            let out = bitmaps::straighten(&Px::new(st.pv.src.w.min(64).max(1), st.pv.src.h.min(64).max(1)), &bitmaps::Straighten { crop: true, crop_resample: false, ..st.o.clone() });
            let fit = (pr.width() / size.x).min(pr.height() / size.y) * st.pv.view.zoom;
            let k = (out.w as f32 / st.pv.src.w.min(64).max(1) as f32, out.h as f32 / st.pv.src.h.min(64).max(1) as f32);
            let cr = Rect::from_center_size(pr.center() + st.pv.view.pan, vec2(size.x * k.0, size.y * k.1) * fit);
            ui.painter().with_clip_rect(pr).rect_stroke(cr, 0.0, Stroke::new(1.5, Color32::from_rgb(0xff, 0xd0, 0x40)), egui::StrokeKind::Middle);
        }
        // the grid settings
        at(ui, Rect::from_min_size(pos2(left + 415.0, top + 30.0), vec2(450.0, 26.0)), |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut st.grid, "Grid:");
                ui.add_space(4.0);
                let c = ui.cursor().min + vec2(9.0, 11.0);
                glyph(app, ui, "grid_fine", c, 16.0);
                ui.add_space(22.0);
                ui.add_enabled_ui(st.grid, |ui| slider(ui, &mut st.grid_size, 0.0, 100.0, 236.0, Sl::Blue));
                let c = ui.cursor().min + vec2(11.0, 11.0);
                glyph(app, ui, "grid_coarse", c, 16.0);
                ui.add_space(26.0);
                let mut gc = Some(st.grid_color);
                if crate::dockers::color_pick_w(ui, "bm_st_grid", &mut gc, false, st.grid, 76.0) {
                    if let Some(c) = gc {
                        st.grid_color = c;
                    }
                }
            });
        });
        // the corrections
        let box_r = Rect::from_min_size(pos2(left + 415.0, top + 63.0), vec2(442.0, 348.0));
        ui.painter().rect_stroke(box_r, 2.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Inside);
        at(ui, box_r.shrink2(vec2(10.0, 14.0)), |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let rows: [(&str, usize, f64, &str, (&str, &str)); 4] = [("Correct lens distortion", 0, 100.0, "", ("lens_barrel", "lens_pincushion")), ("Rotate image", 1, 15.0, " °", ("rotate_cw", "rotate_ccw")), ("Vertical Perspective", 2, 100.0, "", ("persp_v_in", "persp_v_out")), ("Horizontal Perspective", 3, 100.0, "", ("persp_h_in", "persp_h_out"))];
            for (label, k, max, unit, (ia, ib)) in rows {
                let v = match k {
                    0 => &mut st.o.lens,
                    1 => &mut st.o.rotate,
                    2 => &mut st.o.vertical,
                    _ => &mut st.o.horizontal,
                };
                value_row(ui, label, 140.0, v, -max, max, 1, unit, 200.0, Sl::Blue);
                let y = ui.cursor().top() + 10.0;
                let x0 = ui.max_rect().left() + 175.0;
                glyph(app, ui, ia, pos2(x0, y), 16.0);
                glyph(app, ui, ib, pos2(x0 + 160.0, y), 16.0);
                ui.add_space(30.0);
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(150.0);
                if ui.checkbox(&mut st.o.crop, "Crop image").changed() && !st.o.crop {
                    st.o.crop_resample = false;
                }
            });
            ui.horizontal(|ui| {
                ui.add_space(150.0);
                if ui.checkbox(&mut st.o.crop_resample, "Crop and resample to original size").changed() && st.o.crop_resample {
                    st.o.crop = true;
                }
            });
        });
        let y = ui.max_rect().bottom() - 46.0;
        hline(ui, left + 5.0..=ui.max_rect().right() - 5.0, y);
        let br = Rect::from_min_max(pos2(left + 5.0, ui.max_rect().bottom() - 26.0), pos2(ui.max_rect().right() - 5.0, ui.max_rect().bottom()));
        at(ui, br, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut st.preview, "Preview");
                ui.add_space(8.0);
                if button(ui, "Reset", true) {
                    res.2 = true;
                }
                ui.add_space(4.0);
                ui.checkbox(&mut st.remember, "Remember settings");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    res.1 = button(ui, "Cancel", true);
                    ui.add_space(4.0);
                    res.0 = ok_button(ui, "OK", true);
                });
            });
        });
    });
    if res.2 {
        st.o = Default::default();
    }
    if res.0 {
        app.dialogs.bm.straighten_saved = st.remember.then(|| st.o.clone());
        straighten_apply(app, &st.o);
    } else if !(res.1 || dismissed) {
        app.dialogs.bm.straighten = Some(st);
    }
}

fn straighten_apply(app: &mut AnyVectorApp, o: &bitmaps::Straighten) {
    let Some(t) = target(app) else { return };
    let v = t.visible();
    let out = bitmaps::straighten(&v, o);
    let (fw, fh) = (out.w as f64 / v.w as f64, out.h as f64 / v.h as f64);
    let (w, h) = (t.io.w * fw, t.io.h * fh);
    let area = kurbo::Rect::new((t.io.w - w) / 2.0, (t.io.h - h) / 2.0, (t.io.w + w) / 2.0, (t.io.h + h) / 2.0);
    replace_bitmap(app, "Straighten Image", t.obj, &out, None, Some(area));
}

// ------------------------------------------------------------------------------ Image Adjustment Lab

pub struct LabState {
    pub a: bitmaps::AdjustLab,
    /// Quarter turns clockwise.
    pub turns: u8,
    pub preview: bool,
    pub remember: bool,
    pub history: Vec<bitmaps::AdjustLab>,
    pub at: usize,
    pub snapshots: Vec<(bitmaps::AdjustLab, TextureHandle)>,
    pub snap_sel: Option<usize>,
    /// Picking the white (0) or black (1) point.
    pub pick: Option<u8>,
    pub hist: Option<[Vec<u32>; 3]>,
    pub pv: Preview<(bitmaps::AdjustLab, bool, u8)>,
}

impl LabState {
    fn new(t: &Target, a: bitmaps::AdjustLab) -> LabState {
        let mut pv = Preview::new(t, 480);
        pv.view.layout = 1;
        LabState { history: vec![a.clone()], a, turns: 0, preview: true, remember: true, at: 0, snapshots: vec![], snap_sel: None, pick: None, hist: None, pv }
    }
}

/// Quarter turns clockwise.
fn turn(px: &Px, k: u8) -> Px {
    let mut out = px.clone();
    for _ in 0..k % 4 {
        let mut r = Px::new(out.h, out.w);
        for y in 0..out.h {
            for x in 0..out.w {
                r.p[x * r.w + (out.h - 1 - y)] = out.p[y * out.w + x];
            }
        }
        out = r;
    }
    out
}

fn histogram(px: &Px) -> [Vec<u32>; 3] {
    let mut h = [vec![0u32; 256], vec![0u32; 256], vec![0u32; 256]];
    for c in &px.p {
        if c[3] > 0 {
            for k in 0..3 {
                h[k][c[k] as usize] += 1;
            }
        }
    }
    h
}

fn lab(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.bm.lab.take() else { return };
    let key = (st.a.clone(), st.preview, st.turns);
    let (a, on, turns) = key.clone();
    let mut hist = None;
    st.pv.update(ctx, "bm_lab", &key, |px| {
        let out = turn(&if on { bitmaps::apply_effects(px, &a.effects()) } else { px.clone() }, turns);
        hist = Some(histogram(&out));
        out
    });
    if hist.is_some() {
        st.hist = hist;
        // the before image turns with the result
        let src = turn(&st.pv.src, st.turns);
        st.pv.before = Some(tex(ctx, "bm_lab_before", &src));
    }
    let mut res = (false, false, false);
    let ((), dismissed) = window(ctx, "bm_lab", "Image Adjustment Lab", vec2(917.0, 700.0), |ui| {
        let left = ui.max_rect().left();
        let top = ui.max_rect().top();
        let size = if st.turns % 2 == 1 { vec2(st.pv.src.h as f32, st.pv.src.w as f32) } else { st.pv.size() };
        at(ui, Rect::from_min_size(pos2(left + 40.0, top + 30.0), vec2(455.0, 26.0)), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (k, (i, tip)) in [("pv_side", "Before and after"), ("pv_full", "Full preview"), ("pv_split", "Split preview")].into_iter().enumerate() {
                    if icon(app, ui, i, tip, st.pv.view.layout == k as u8) {
                        st.pv.view.layout = k as u8;
                    }
                }
                ui.add_space(70.0);
                if icon(app, ui, "rotate_cw", "Rotate clockwise", false) {
                    st.turns = (st.turns + 1) % 4;
                }
                if icon(app, ui, "rotate_ccw", "Rotate counterclockwise", false) {
                    st.turns = (st.turns + 3) % 4;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    icon(app, ui, "pan", "Pan", true);
                    if icon(app, ui, "zoom_actual", "Zoom 100%", false) {
                        st.pv.view.actual = true;
                    }
                    if icon(app, ui, "zoom_fit", "Zoom to fit", false) {
                        st.pv.view.zoom = 1.0;
                        st.pv.view.pan = Vec2::ZERO;
                    }
                    if icon(app, ui, "zoom_out", "Zoom out", false) {
                        st.pv.view.zoom = (st.pv.view.zoom / 1.25).max(0.1);
                    }
                    if icon(app, ui, "zoom_in", "Zoom in", false) {
                        st.pv.view.zoom = (st.pv.view.zoom * 1.25).min(16.0);
                    }
                });
            });
        });
        let pr = Rect::from_min_size(pos2(left + 40.0, top + 93.0), vec2(455.0, 448.0));
        if st.pv.view.layout == 1 {
            ui.painter().text(pos2(pr.center().x, pr.top() - 10.0), egui::Align2::CENTER_CENTER, "After", egui::FontId::proportional(12.0), theme::TEXT);
        }
        let (b, a) = (st.pv.before.clone(), st.pv.after.clone());
        let titles = st.pv.view.layout == 0;
        let click = panes(ui, &mut st.pv.view, pr, b.as_ref(), a.as_ref(), size, titles);
        if let (Some(k), Some(uv)) = (st.pick, click) {
            let src = turn(&bitmaps::apply_effects(&st.pv.src, &st.a.effects()), st.turns);
            let c = src.p[((uv.y * (src.h - 1) as f32) as usize) * src.w + (uv.x * (src.w - 1) as f32) as usize];
            set_point(&mut st.a, c, k == 0);
            st.pick = None;
        }
        // history and snapshots
        let hr = Rect::from_min_size(pos2(left + 40.0, pr.bottom() + 8.0), vec2(130.0, 1.0));
        hline(ui, hr.x_range(), hr.top());
        at(ui, Rect::from_min_size(pos2(left + 40.0, pr.bottom() + 14.0), vec2(455.0, 70.0)), |ui| {
            ui.horizontal(|ui| {
                if crate::widgets::icon_button(&mut app.icons, ui, "undo", "Undo", st.at > 0, false).clicked() {
                    st.at -= 1;
                    st.a = st.history[st.at].clone();
                }
                if crate::widgets::icon_button(&mut app.icons, ui, "redo", "Redo", st.at + 1 < st.history.len(), false).clicked() {
                    st.at += 1;
                    st.a = st.history[st.at].clone();
                }
                ui.add_space(14.0);
                if icon(app, ui, "snapshot", "Create snapshot", false) {
                    if let Some(t) = st.pv.after.clone() {
                        st.snapshots.push((st.a.clone(), t));
                        st.snap_sel = Some(st.snapshots.len() - 1);
                    }
                }
                if crate::widgets::icon_button(&mut app.icons, ui, "delete", "Delete snapshot", st.snap_sel.is_some(), false).clicked() {
                    if let Some(k) = st.snap_sel.take().filter(|k| *k < st.snapshots.len()) {
                        let _ = st.snapshots.remove(k);
                    }
                }
                ui.add_space(10.0);
                for (k, (sa, t)) in st.snapshots.clone().iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(vec2(44.0, 30.0), Sense::click());
                    ui.painter().image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    if st.snap_sel == Some(k) {
                        ui.painter().rect_stroke(r, 0.0, Stroke::new(2.0, theme::ACCENT), egui::StrokeKind::Outside);
                    }
                    if resp.clicked() {
                        st.snap_sel = Some(k);
                        st.a = sa.clone();
                    }
                }
            });
        });
        // the histogram of the result
        let hrect = Rect::from_min_size(pos2(left + 521.0, top + 63.0), vec2(336.0, 131.0));
        ui.painter().rect_filled(hrect, 0.0, Color32::from_rgb(0xcf, 0xd2, 0xd9));
        if let Some(h) = &st.hist {
            let max = h.iter().flat_map(|c| c.iter()).copied().max().unwrap_or(1).max(1) as f32;
            for (k, col) in [(0usize, Color32::from_rgb(0xe0, 0x20, 0x20)), (1, Color32::from_rgb(0x20, 0xc0, 0x20)), (2, Color32::from_rgb(0x20, 0x40, 0xe0))] {
                for (i, n) in h[k].iter().enumerate() {
                    if *n == 0 {
                        continue;
                    }
                    let x = hrect.left() + i as f32 / 255.0 * hrect.width();
                    let hh = (*n as f32 / max).sqrt() * hrect.height();
                    ui.painter().vline(x, hrect.bottom() - hh..=hrect.bottom(), Stroke::new(1.0, col));
                }
            }
        }
        hline(ui, left + 521.0..=left + 857.0, top + 221.0);
        let before = st.a.clone();
        at(ui, Rect::from_min_size(pos2(left + 521.0, top + 244.0), vec2(346.0, 300.0)), |ui| {
            ui.horizontal(|ui| {
                ui.add_space(88.0);
                if ui.add_sized(vec2(102.0, 24.0), egui::Button::new("Auto adjust")).clicked() {
                    st.a = bitmaps::AdjustLab::auto(&st.pv.src);
                }
                ui.add_space(6.0);
                if icon(app, ui, "eyedrop_white", "Select white point", st.pick == Some(0)) {
                    st.pick = if st.pick == Some(0) { None } else { Some(0) };
                }
                if icon(app, ui, "eyedrop_black", "Select black point", st.pick == Some(1)) {
                    st.pick = if st.pick == Some(1) { None } else { Some(1) };
                }
            });
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.y = 10.0;
            let sw = 174.0;
            value_row(ui, "Temperature", 92.0, &mut st.a.temperature, 1000.0, 10000.0, 0, "", sw, Sl::Grad(Color32::from_rgb(0xf0, 0x8a, 0x24), Color32::from_rgb(0xf4, 0xe8, 0xe0), Color32::from_rgb(0xa8, 0xc4, 0xff)));
            value_row(ui, "Tint", 92.0, &mut st.a.tint, -100.0, 100.0, 0, "", sw, Sl::Grad(Color32::from_rgb(0xe0, 0x70, 0xf0), Color32::from_rgb(0xf0, 0xe8, 0xf0), Color32::from_rgb(0xa8, 0xf0, 0x80)));
            value_row(ui, "Saturation", 92.0, &mut st.a.saturation, -100.0, 100.0, 0, "", sw, Sl::Plain);
            value_row(ui, "Brightness", 92.0, &mut st.a.brightness, -100.0, 100.0, 0, "", sw, Sl::Plain);
            value_row(ui, "Contrast", 92.0, &mut st.a.contrast, -100.0, 100.0, 0, "", sw, Sl::Plain);
            value_row(ui, "Highlights", 92.0, &mut st.a.highlights, -100.0, 100.0, 0, "", sw, Sl::Plain);
            value_row(ui, "Shadows", 92.0, &mut st.a.shadows, -100.0, 100.0, 0, "", sw, Sl::Plain);
            value_row(ui, "Midtones", 92.0, &mut st.a.midtones, -100.0, 100.0, 0, "", sw, Sl::Plain);
        });
        // a settled change goes on the history
        if st.a != before || (st.history.get(st.at) != Some(&st.a) && !ui.ctx().input(|i| i.pointer.any_down())) {
            if !ui.ctx().input(|i| i.pointer.any_down()) && st.history.get(st.at) != Some(&st.a) {
                st.history.truncate(st.at + 1);
                st.history.push(st.a.clone());
                st.at = st.history.len() - 1;
            }
        }
        let y = ui.max_rect().bottom() - 46.0;
        hline(ui, left + 5.0..=ui.max_rect().right() - 5.0, y);
        let br = Rect::from_min_max(pos2(left + 5.0, ui.max_rect().bottom() - 26.0), pos2(ui.max_rect().right() - 5.0, ui.max_rect().bottom()));
        at(ui, br, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut st.preview, "Preview");
                ui.add_space(8.0);
                if button(ui, "Reset", true) {
                    res.2 = true;
                }
                ui.add_space(4.0);
                ui.checkbox(&mut st.remember, "Remember settings");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    res.1 = button(ui, "Cancel", true);
                    ui.add_space(4.0);
                    res.0 = ok_button(ui, "OK", true);
                });
            });
        });
    });
    if res.2 {
        st.a = Default::default();
        st.turns = 0;
    }
    if res.0 {
        app.dialogs.bm.lab_saved = st.remember.then(|| st.a.clone());
        if let Some(t) = target(app) {
            let out = turn(&bitmaps::apply_effects(&t.full, &st.a.effects()), st.turns);
            let area = (st.turns % 2 == 1).then(|| {
                let (w, h) = (t.io.h, t.io.w);
                kurbo::Rect::new((t.io.w - w) / 2.0, (t.io.h - h) / 2.0, (t.io.w + w) / 2.0, (t.io.h + h) / 2.0)
            });
            replace_bitmap(app, "Image Adjustment Lab", t.obj, &out, None, area);
        }
    } else if !(res.1 || dismissed) {
        app.dialogs.bm.lab = Some(st);
    }
}

/// The white point makes the picked color a light neutral; the black point darkens the shadows
/// down to it.
fn set_point(a: &mut bitmaps::AdjustLab, c: [u8; 4], white: bool) {
    let (r, g, b) = (c[0] as f64, c[1] as f64, c[2] as f64);
    if white {
        a.temperature = (5000.0 + (r - b) / 255.0 * 3000.0).clamp(1000.0, 10000.0).round();
        a.tint = ((g - (r + b) / 2.0) / 255.0 * 100.0).clamp(-100.0, 100.0).round();
        let l = 0.299 * r + 0.587 * g + 0.114 * b;
        a.brightness = ((235.0 - l) / 255.0 * 100.0).clamp(-100.0, 100.0).round();
    } else {
        let l = 0.299 * r + 0.587 * g + 0.114 * b;
        a.shadows = (-l / 255.0 * 200.0).clamp(-100.0, 0.0).round();
    }
}

// ------------------------------------------------------------------------------ PowerTRACE

pub const OUTLINE_PROFILES: [&str; 6] = ["Line art", "Logo", "Detailed logo", "Clipart", "Low quality image", "High quality image"];
const OUTLINE_PRESETS: [&str; 6] = ["Line Art", "Logo", "Detailed Logo", "Clipart", "Low Quality Image", "High Quality Image"];
pub const CENTER_PROFILES: [&str; 2] = ["Technical Illustration", "Line drawing"];
const RESAMPLE: [&str; 4] = ["Auto-downsample (16 MP+)", "Keep original resolution", "Upsample (Illustration ✦)", "Upsample (Photorealistic ✦)"];
const REMOVE: [&str; 3] = ["None", "Background color", "Specify color"];
const SORT: [&str; 2] = ["Similarity", "Frequency"];
const PREVIEWS: [&str; 3] = ["Before and After", "Large Preview", "Wireframe Overlay"];

/// The settings of a trace.
#[derive(Clone, PartialEq)]
pub struct TraceOpts {
    pub centerline: bool,
    pub profile: usize,
    pub resample: usize,
    pub remove_bg: bool,
    pub remove_jpeg: bool,
    pub delete_original: bool,
    pub detail: f64,
    pub density: f64,
    pub curvature: f64,
    pub remove_overlap: bool,
    pub merge_adjacent: bool,
    pub group: bool,
    /// RGB, CMYK, Grayscale, Black and White, then the palettes (`palette_modes`).
    pub color_mode: usize,
    pub remove: usize,
    pub remove_color: [u8; 3],
    pub colors: usize,
    pub sort: usize,
    /// The colors chosen in the list (merged, edited); else found in the image.
    pub palette: Option<Vec<[u8; 3]>>,
    /// Colors left out (Delete color).
    pub dropped: Vec<[u8; 3]>,
}

impl TraceOpts {
    /// The settings a profile starts with.
    pub fn profile(centerline: bool, k: usize) -> TraceOpts {
        let name = if centerline { CENTER_PROFILES[k.min(1)] } else { OUTLINE_PRESETS[k.min(5)] };
        let detail = if centerline { [66.0, 33.0][k.min(1)] } else { [60.0, 67.0, 50.0, 75.0, 50.0, 75.0][k.min(5)] };
        let base = crate::trace::preset(name);
        TraceOpts {
            centerline,
            profile: k,
            resample: 0,
            remove_bg: false,
            remove_jpeg: false,
            delete_original: false,
            detail,
            density: 75.0,
            curvature: 0.0,
            remove_overlap: false,
            merge_adjacent: true,
            group: true,
            color_mode: 0,
            remove: 0,
            remove_color: [255, 255, 255],
            colors: base.colors,
            sort: 0,
            palette: None,
            dropped: vec![],
        }
    }

    /// The profile of a Quick Trace method (Options ▸ PowerTRACE).
    pub fn quick(method: &str) -> TraceOpts {
        match method {
            "Centerline" => TraceOpts::profile(true, 0),
            m => TraceOpts::profile(false, OUTLINE_PRESETS.iter().position(|p| *p == m).unwrap_or(3)),
        }
    }
}

/// The palettes of Color Mode after the four modes.
fn palette_modes(app: &AnyVectorApp) -> Vec<(String, Vec<[u8; 3]>)> {
    let c3 = |c: &vector_core::color::NamedColor| [c.color.r, c.color.g, c.color.b];
    let mut v = vec![("Document Palette".to_string(), app.doc().map(|d| d.doc.palette.iter().map(c3).collect()).unwrap_or_default())];
    for p in [vector_core::color::Palette::default_palette(), vector_core::color::Palette::rgb_palette()] {
        v.push((p.name.clone(), p.colors.iter().map(c3).collect()));
    }
    for p in &app.palettes.palettes {
        if !v.iter().any(|x| x.0 == p.name) {
            v.push((p.name.clone(), p.colors.iter().map(c3).collect()));
        }
    }
    v
}

/// The pixels a trace works on and its parameters.
fn trace_input(app: &AnyVectorApp, o: &TraceOpts, t: &Target) -> (Px, crate::trace::TraceParams) {
    let name = if o.centerline { CENTER_PROFILES[o.profile.min(1)] } else { OUTLINE_PRESETS[o.profile.min(5)] };
    let mut p = crate::trace::preset(name);
    let mut px = t.visible();
    if o.remove_jpeg {
        px = bitmaps::remove_jpeg_artifacts(&px);
    }
    if o.remove_bg {
        px = bitmaps::remove_background(&px);
    }
    match o.color_mode {
        2 => px = bitmaps::grayscale(&px),
        3 => px = bitmaps::one_bit(&px, &bitmaps::OneBit { method: 0, ..Default::default() }),
        _ => {}
    }
    let side = px.w.max(px.h) as u32;
    p.max_side = match o.resample {
        0 => side.min(1000),
        1 => side,
        _ => (side * 2).min(2400),
    };
    p.colors = if o.color_mode == 3 { 2 } else { o.colors.clamp(1, 256) };
    p.min_area = (1.0 + ((100.0 - o.detail) / 100.0).powi(2) * 120.0) as usize;
    p.smoothing = ((100.0 - o.density) * 1.6 + o.curvature * 0.6).clamp(0.0, 100.0);
    p.remove_background = o.centerline || o.remove == 1;
    p.remove_overlap = o.remove_overlap;
    p.merge_adjacent = o.merge_adjacent;
    p.palette = o.palette.clone();
    if o.color_mode >= 4 && p.palette.is_none() {
        // a palette: the found colors snap to its nearest ones
        if let Some((_, pal)) = palette_modes(app).get(o.color_mode - 4).filter(|p| !p.1.is_empty()) {
            let found = crate::trace::trace(px.w as u32, px.h as u32, &px.p.iter().flat_map(|c| *c).collect::<Vec<u8>>(), &crate::trace::TraceParams { max_side: 200, ..p.clone() }, Affine::IDENTITY, || 0).palette;
            let mut snapped: Vec<[u8; 3]> = vec![];
            for c in found {
                let n = pal.iter().min_by_key(|q| (q[0] as i32 - c.r as i32).pow(2) + (q[1] as i32 - c.g as i32).pow(2) + (q[2] as i32 - c.b as i32).pow(2)).copied().unwrap_or([0, 0, 0]);
                if !snapped.contains(&n) {
                    snapped.push(n);
                }
            }
            p.palette = Some(snapped);
        }
    }
    p.remove_colors = o.dropped.clone();
    if o.remove == 2 {
        p.remove_colors.push(o.remove_color);
    }
    (px, p)
}

/// Traces the bitmap: the objects in document space, the colors and the node count.
fn run_trace(app: &AnyVectorApp, o: &TraceOpts, t: &Target) -> crate::trace::TraceResult {
    let (px, p) = trace_input(app, o, t);
    let to_doc = app.doc().and_then(|d| d.doc.find(t.obj)).map(|ob| ob.transform).unwrap_or_default() * Affine::scale_non_uniform(t.io.w / px.w as f64, t.io.h / px.h as f64);
    let mut next = 1u64 << 40;
    crate::trace::trace(px.w as u32, px.h as u32, &px.p.iter().flat_map(|c| *c).collect::<Vec<u8>>(), &p, to_doc, || {
        next += 1;
        next
    })
}

pub struct TraceState {
    pub o: TraceOpts,
    pub obj: ObjId,
    pub preview: usize,
    pub view: View,
    pub folded: [bool; 5],
    pub history: Vec<TraceOpts>,
    pub at: usize,
    /// The colors selected in the list.
    pub sel: Vec<usize>,
    pub picking: bool,
    pub result: Option<(TraceOpts, crate::trace::TraceResult)>,
    pub before: Option<TextureHandle>,
    pub after: Option<(u64, TextureHandle)>,
    pub size: Vec2,
    pub src: Px,
}

/// Opens PowerTRACE on the selected bitmap with these settings.
pub fn open_trace(app: &mut AnyVectorApp, o: TraceOpts) {
    let Some(t) = target(app) else {
        app.flash("Select a bitmap to trace.");
        return;
    };
    let src = t.visible().fit(700);
    app.dialogs.bm.trace = Some(TraceState { history: vec![o.clone()], o, obj: t.obj, preview: 0, view: View::default(), folded: [false; 5], at: 0, sel: vec![], picking: false, result: None, before: None, after: None, size: vec2(src.w as f32, src.h as f32), src });
}

/// Traces with these settings without the dialog (Quick Trace, Repeat Last Trace).
pub fn trace_now(app: &mut AnyVectorApp, o: TraceOpts) {
    let Some(t) = target(app) else {
        app.flash("Select a bitmap to trace.");
        return;
    };
    let r = run_trace(app, &o, &t);
    app.dialogs.bm.last_trace = Some(o.clone());
    app.apply_trace(t.obj, r.objects, o.delete_original, o.group);
}

fn sort_colors(r: &crate::trace::TraceResult, sort: usize) -> Vec<Color> {
    let mut v = r.palette.clone();
    if sort == 0 {
        // similarity: by hue, then lightness
        v.sort_by(|a, b| {
            let k = |c: &Color| {
                let [h, s, l] = c.to_hsb();
                ((if s < 10.0 { -1.0 } else { h }) * 1000.0 + l) as i64
            };
            k(a).cmp(&k(b))
        });
    }
    v
}

fn trace(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.bm.trace.take() else { return };
    let Some(t) = target(app).filter(|t| t.obj == st.obj) else { return };
    if st.before.is_none() {
        st.before = Some(tex(ctx, "bm_trace_before", &st.src));
    }
    // trace again once the settings settle
    let settled = !ctx.input(|i| i.pointer.any_down());
    if settled && st.result.as_ref().map(|r| &r.0) != Some(&st.o) {
        let r = run_trace(app, &st.o, &t);
        st.result = Some((st.o.clone(), r));
        st.after = None;
        if st.history.get(st.at) != Some(&st.o) {
            st.history.truncate(st.at + 1);
            st.history.push(st.o.clone());
            st.at = st.history.len() - 1;
        }
    }
    // the result drawn as a picture (plain, or as wireframe over the bitmap)
    let key = vector_render::hash_str(&format!("{}-{}", st.history.len() * 1000 + st.at, st.preview)) ^ st.result.as_ref().map(|r| r.1.node_count as u64 * 7919 + r.1.objects.len() as u64).unwrap_or(0);
    if st.after.as_ref().map(|a| a.0) != Some(key) {
        if let (Some((_, r)), Some(d)) = (&st.result, app.doc()) {
            if let Some(ob) = d.doc.find(st.obj) {
                let bb = ob.bounds();
                let w = st.src.w as f64;
                let h = (w * bb.height() / bb.width().max(1e-6)).max(1.0);
                if let Some(mut pm) = tiny_skia::Pixmap::new(w as u32, h as u32) {
                    let wire = st.preview == 2;
                    pm.fill(if wire { tiny_skia::Color::TRANSPARENT } else { tiny_skia::Color::WHITE });
                    let view = Affine::scale(w / bb.width().max(1e-6)) * Affine::translate((-bb.x0, -bb.y0));
                    let opts = vector_render::RenderOptions { draw_page: false, wireframe: wire, ..Default::default() };
                    vector_render::Renderer::new().render_objects(&d.doc, &r.objects, view, &mut pm, &opts);
                    let ci = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
                    st.after = Some((key, ctx.load_texture("bm_trace_after", ci, egui::TextureOptions::LINEAR)));
                }
            }
        }
    }
    let mut res = (false, false, false);
    let ((), dismissed) = window(ctx, "bm_trace", "PowerTRACE", vec2(1058.0, 712.0), |ui| {
        let left = ui.max_rect().left();
        let top = ui.max_rect().top();
        // the preview
        at(ui, Rect::from_min_size(pos2(left + 17.0, top + 18.0), vec2(606.0, 26.0)), |ui| {
            ui.horizontal(|ui| {
                ui.label("Preview:");
                super::combo(ui, "bm_tr_pv", &mut st.preview, &PREVIEWS, 136.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon(app, ui, "pan", "Pan", true) {}
                    if icon(app, ui, "zoom_fit", "Zoom to fit", false) {
                        st.view.zoom = 1.0;
                        st.view.pan = Vec2::ZERO;
                    }
                    if icon(app, ui, "zoom_out", "Zoom out", false) {
                        st.view.zoom = (st.view.zoom / 1.25).max(0.1);
                    }
                    if icon(app, ui, "zoom_in", "Zoom in", false) {
                        st.view.zoom = (st.view.zoom * 1.25).min(16.0);
                    }
                });
            });
        });
        let full = Rect::from_min_size(pos2(left + 25.0, top + 52.0), vec2(598.0, 534.0));
        let pane_bg = Color32::from_rgb(0x26, 0x26, 0x26);
        let border = Stroke::new(1.0, Color32::from_gray(0x6a));
        let after = st.after.as_ref().map(|a| a.1.clone());
        let before = st.before.clone();
        let size = st.size;
        let draw = |ui: &mut Ui, r: Rect, t: Option<&TextureHandle>, under: Option<&TextureHandle>, v: &mut View, id: &str| -> Option<Pos2> {
            ui.painter().rect_filled(r, 0.0, pane_bg);
            ui.painter().rect_stroke(r, 0.0, border, egui::StrokeKind::Inside);
            let inner = r.shrink(1.0);
            let mut img = None;
            if let Some(u) = under {
                let ir = paint_in(ui, v, inner, u, size, None);
                ui.painter().with_clip_rect(inner).rect_filled(ir, 0.0, Color32::from_white_alpha(150));
                img = Some(ir);
            }
            if let Some(t) = t {
                img = Some(paint_in(ui, v, inner, t, size, None));
            }
            let resp = ui.interact(inner, ui.scope_id().with(id), Sense::click_and_drag());
            if resp.dragged() {
                v.pan += resp.drag_delta();
            }
            match (resp.clicked(), resp.interact_pointer_pos(), img) {
                (true, Some(p), Some(ir)) => {
                    let u = (p - ir.min) / ir.size();
                    ((0.0..=1.0).contains(&u.x) && (0.0..=1.0).contains(&u.y)).then(|| pos2(u.x, u.y))
                }
                _ => None,
            }
        };
        let mut picked = None;
        match st.preview {
            0 => {
                let a = Rect::from_min_size(full.min, vec2(full.width(), full.height() / 2.0 - 4.0));
                let b = a.translate(vec2(0.0, full.height() / 2.0 + 4.0));
                picked = draw(ui, a, before.as_ref(), None, &mut st.view, "bm_tr_a");
                draw(ui, b, after.as_ref(), None, &mut st.view, "bm_tr_b");
            }
            1 => {
                draw(ui, full, after.as_ref(), None, &mut st.view, "bm_tr_b");
            }
            _ => {
                picked = draw(ui, full, after.as_ref(), before.as_ref(), &mut st.view, "bm_tr_b");
            }
        }
        if let (true, Some(uv)) = (st.picking, picked) {
            let c = st.src.p[((uv.y * (st.src.h - 1) as f32) as usize) * st.src.w + (uv.x * (st.src.w - 1) as f32) as usize];
            if st.o.remove == 2 {
                st.o.remove_color = [c[0], c[1], c[2]];
            }
            if let Some((_, r)) = &st.result {
                let list = sort_colors(r, st.o.sort);
                if let Some(k) = (0..list.len()).min_by_key(|k| (list[*k].r as i32 - c[0] as i32).pow(2) + (list[*k].g as i32 - c[1] as i32).pow(2) + (list[*k].b as i32 - c[2] as i32).pow(2)) {
                    st.sel = vec![k];
                }
            }
            st.picking = false;
        }
        let (curves, nodes, colors) = st.result.as_ref().map(|r| (r.1.objects.len(), r.1.node_count, r.1.palette.len())).unwrap_or((0, 0, 0));
        at(ui, Rect::from_min_size(pos2(left + 25.0, top + 594.0), vec2(598.0, 22.0)), |ui| {
            ui.horizontal(|ui| {
                for (n, v) in [("Curves:", curves), ("Nodes:", nodes), ("Colors:", colors)] {
                    label_at(ui, n, 46.0, false);
                    label_at(ui, &v.to_string(), 52.0, false);
                }
            });
        });
        // the settings
        let side = Rect::from_min_size(pos2(left + 645.0, top + 18.0), vec2(365.0, 570.0));
        ui.painter().rect_filled(Rect::from_min_size(pos2(side.right() - 12.0, side.top()), vec2(12.0, side.height())), 0.0, Color32::from_rgb(0x1e, 0x1e, 0x1e));
        let modes = palette_modes(app);
        at(ui, side, |ui| {
            egui::ScrollArea::vertical().id_salt("bm_tr_side").max_height(side.height()).auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(side.width() - 30.0);
                trace_settings(app, ui, &mut st, &modes);
            });
        });
        at(ui, Rect::from_min_size(pos2(left + 649.0, top + 594.0), vec2(200.0, 26.0)), |ui| {
            ui.horizontal(|ui| {
                if ui.add_sized(vec2(80.0, 24.0), egui::Button::new("Reset")).clicked() {
                    st.o = TraceOpts::profile(st.o.centerline, st.o.profile);
                    st.sel.clear();
                }
                ui.add_space(6.0);
                if crate::widgets::icon_button(&mut app.icons, ui, "undo", "Undo", st.at > 0, false).clicked() {
                    st.at -= 1;
                    st.o = st.history[st.at].clone();
                }
                if crate::widgets::icon_button(&mut app.icons, ui, "redo", "Redo", st.at + 1 < st.history.len(), false).clicked() {
                    st.at += 1;
                    st.o = st.history[st.at].clone();
                }
            });
        });
        res = bottom(ui, true, false);
    });
    if res.0 {
        let r = match st.result.take() {
            Some((o, r)) if o == st.o => r,
            _ => run_trace(app, &st.o, &t),
        };
        app.dialogs.bm.last_trace = Some(st.o.clone());
        app.apply_trace(st.obj, r.objects, st.o.delete_original, st.o.group);
    } else if !(res.1 || dismissed) {
        app.dialogs.bm.trace = Some(st);
    }
}

/// A fold-out section title of the PowerTRACE settings; true when open.
fn section(ui: &mut Ui, title: &str, folded: &mut bool) -> bool {
    ui.add_space(6.0);
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
    let c = pos2(r.left() + 8.0, r.center().y);
    if *folded {
        crate::widgets::chevron(ui.painter(), c, 3.0, theme::TEXT);
    } else {
        crate::widgets::chevron_up(ui.painter(), c, 3.0, theme::TEXT);
    }
    ui.painter().text(pos2(r.left() + 20.0, r.center().y), egui::Align2::LEFT_CENTER, title, egui::FontId::proportional(12.5), theme::TEXT);
    ui.painter().text(pos2(r.left() + 20.5, r.center().y), egui::Align2::LEFT_CENTER, title, egui::FontId::proportional(12.5), theme::TEXT);
    if resp.clicked() {
        *folded = !*folded;
    }
    !*folded
}

fn trace_settings(app: &mut AnyVectorApp, ui: &mut Ui, st: &mut TraceState, modes: &[(String, Vec<[u8; 3]>)]) {
    let lw = 110.0;
    let cw = 212.0;
    if section(ui, "Trace type", &mut st.folded[0]) {
        ui.horizontal(|ui| {
            label_at(ui, "Trace type:", lw, true);
            let mut k = if st.o.centerline { 0 } else { 1 };
            if super::combo(ui, "bm_tr_type", &mut k, &["Centerline", "Outline"], cw) {
                st.o = TraceOpts::profile(k == 0, if k == 0 { 0 } else { 3 });
            }
        });
        ui.horizontal(|ui| {
            label_at(ui, "Trace profile:", lw, true);
            let mut k = st.o.profile;
            let list: &[&str] = if st.o.centerline { &CENTER_PROFILES } else { &OUTLINE_PROFILES };
            if super::combo(ui, "bm_tr_prof", &mut k, list, cw) {
                st.o = TraceOpts::profile(st.o.centerline, k);
            }
        });
    }
    if section(ui, "Image handling", &mut st.folded[1]) {
        ui.horizontal(|ui| {
            label_at(ui, "Resample mode:", lw - 4.0, true);
            super::combo(ui, "bm_tr_rs", &mut st.o.resample, &RESAMPLE, cw);
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut st.o.remove_bg, "Remove image background");
            let c = ui.cursor().min + vec2(9.0, 10.0);
            glyph(app, ui, "bg_remove_ai", c, 16.0);
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut st.o.remove_jpeg, "Remove JPEG artifacts");
            let c = ui.cursor().min + vec2(9.0, 10.0);
            glyph(app, ui, "sparkle", c, 14.0);
        });
        ui.checkbox(&mut st.o.delete_original, "Delete original image");
    }
    if section(ui, "Shape detail", &mut st.folded[2]) {
        for (n, v) in [("Detail", &mut st.o.detail), ("Node density", &mut st.o.density), ("Curvature", &mut st.o.curvature)] {
            ui.label(n);
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                slider(ui, v, 0.0, 100.0, 280.0, Sl::BlueTicks);
                ui.add_space(6.0);
                num_box(ui, v, 0.0, 100.0, 0, 36.0);
            });
        }
    }
    if !st.o.centerline && section(ui, "Shape structure", &mut st.folded[3]) {
        ui.checkbox(&mut st.o.remove_overlap, "Remove overlap");
        ui.checkbox(&mut st.o.merge_adjacent, "Merge adjacent shapes");
        ui.checkbox(&mut st.o.group, "Group objects");
    }
    if section(ui, "Color", &mut st.folded[4]) {
        ui.horizontal(|ui| {
            label_at(ui, "Color Mode:", lw, true);
            let mut names: Vec<String> = ["RGB", "CMYK", "Grayscale", "Black and White"].iter().map(|s| s.to_string()).collect();
            names.extend(modes.iter().map(|m| m.0.clone()));
            names.push("More Palettes...".into());
            let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
            let mut k = st.o.color_mode;
            if super::combo(ui, "bm_tr_cm", &mut k, &refs, cw) {
                if k + 1 == refs.len() {
                    if let Some(f) = rfd::FileDialog::new().add_filter("Palettes", &["xml", "gpl", "txt", "cpl"]).pick_file() {
                        if let Ok(t) = std::fs::read_to_string(&f) {
                            let name = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                            app.palettes.palettes.push(vector_core::color::Palette::parse(&name, &t));
                            st.o.color_mode = refs.len() - 1;
                        }
                    }
                } else {
                    st.o.color_mode = k;
                }
                st.o.palette = None;
            }
        });
        ui.horizontal(|ui| {
            label_at(ui, "Remove:", lw, true);
            super::combo(ui, "bm_tr_rm", &mut st.o.remove, &REMOVE, cw);
        });
        ui.horizontal(|ui| {
            label_at(ui, "Number of Colors:", lw, true);
            let mut c = st.o.colors as f64;
            if spin(ui, &mut c, 1.0, 256.0, 1.0, 0, "", cw + 6.0) {
                st.o.colors = c as usize;
                st.o.palette = None;
                st.o.dropped.clear();
            }
        });
        ui.horizontal(|ui| {
            label_at(ui, "Sort by:", lw, true);
            super::combo(ui, "bm_tr_sort", &mut st.o.sort, &SORT, cw);
        });
        ui.add_space(4.0);
        // the colors of the trace
        let list: Vec<Color> = st.result.as_ref().map(|r| sort_colors(&r.1, st.o.sort)).unwrap_or_default();
        let lr = Rect::from_min_size(ui.cursor().min, vec2(328.0, 210.0));
        ui.painter().rect_filled(lr, 0.0, Color32::from_rgb(0x26, 0x26, 0x26));
        ui.painter().rect_stroke(lr, 0.0, Stroke::new(1.0, Color32::from_gray(0x6a)), egui::StrokeKind::Inside);
        at(ui, lr.shrink(1.0), |ui| {
            egui::ScrollArea::vertical().id_salt("bm_tr_colors").max_height(lr.height() - 2.0).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (k, c) in list.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(vec2(lr.width() - 8.0, 26.0), Sense::click());
                    ui.painter().rect_filled(r, 0.0, c32(*c));
                    if st.sel.contains(&k) {
                        ui.painter().rect_stroke(r.shrink(1.0), 0.0, Stroke::new(2.0, theme::ACCENT), egui::StrokeKind::Inside);
                    }
                    let light = 0.299 * c.r as f32 + 0.587 * c.g as f32 + 0.114 * c.b as f32 > 140.0;
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, format!("R{} G{} B{}", c.r, c.g, c.b), egui::FontId::proportional(12.0), if light { Color32::from_gray(0x50) } else { Color32::WHITE });
                    if resp.clicked() {
                        if ui.input(|i| i.modifiers.ctrl || i.modifiers.shift) {
                            if let Some(p) = st.sel.iter().position(|s| *s == k) {
                                st.sel.remove(p);
                            } else {
                                st.sel.push(k);
                            }
                        } else {
                            st.sel = vec![k];
                        }
                    }
                }
            });
        });
        ui.allocate_rect(lr, Sense::hover());
        st.sel.retain(|k| *k < list.len());
        ui.add_space(6.0);
        let picked: Vec<Color> = st.sel.iter().map(|k| list[*k]).collect();
        let merge = |st: &mut TraceState, to: Color| {
            let mut pal: Vec<[u8; 3]> = list.iter().filter(|c| !picked.contains(c)).map(|c| [c.r, c.g, c.b]).collect();
            pal.push([to.r, to.g, to.b]);
            st.o.palette = Some(pal);
            st.sel.clear();
        };
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            if ui.add_enabled(picked.len() > 1, egui::Button::new("Merge by averaging").min_size(vec2(136.0, 24.0))).clicked() {
                let n = picked.len() as u32;
                let avg = |f: fn(&Color) -> u8| (picked.iter().map(|c| f(c) as u32).sum::<u32>() / n) as u8;
                let to = Color::rgb(avg(|c| c.r), avg(|c| c.g), avg(|c| c.b));
                merge(st, to);
            }
            if ui.add_enabled(picked.len() > 1, egui::Button::new("Merge to first selected").min_size(vec2(146.0, 24.0))).clicked() {
                let to = picked[0];
                merge(st, to);
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(64.0);
            if icon(app, ui, "open", "Open color palette", false) {
                if let Some(f) = rfd::FileDialog::new().add_filter("Palettes", &["xml", "gpl", "txt", "cpl"]).pick_file() {
                    if let Ok(t) = std::fs::read_to_string(&f) {
                        let p = vector_core::color::Palette::parse("palette", &t);
                        st.o.palette = Some(p.colors.iter().map(|c| [c.color.r, c.color.g, c.color.b]).collect());
                    }
                }
            }
            if icon(app, ui, "save", "Save color palette", false) {
                if let Some(f) = rfd::FileDialog::new().add_filter("Palette", &["gpl"]).set_file_name("trace.gpl").save_file() {
                    let mut s = String::from("GIMP Palette\nName: PowerTRACE\n#\n");
                    for c in &list {
                        s.push_str(&format!("{} {} {}\t{}\n", c.r, c.g, c.b, c.to_hex()));
                    }
                    let _ = std::fs::write(f, s);
                }
            }
            let one = picked.first().copied();
            let resp = ui.add_enabled(one.is_some(), egui::Button::new("Edit...").min_size(vec2(80.0, 24.0)));
            egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                let mut c = one;
                if crate::dockers::color_pick_w(ui, "bm_tr_edit", &mut c, false, true, 200.0) {
                    if let (Some(old), Some(new)) = (one, c) {
                        let pal: Vec<[u8; 3]> = list.iter().map(|x| if *x == old { [new.r, new.g, new.b] } else { [x.r, x.g, x.b] }).collect();
                        st.o.palette = Some(pal);
                    }
                }
            });
            if icon(app, ui, "eyedropper", "Select color from image", st.picking) {
                st.picking = !st.picking;
            }
            if crate::widgets::icon_button(&mut app.icons, ui, "checker", "Delete color", !picked.is_empty(), false).clicked() {
                st.o.dropped.extend(picked.iter().map(|c| [c.r, c.g, c.b]));
                st.sel.clear();
            }
        });
        ui.add_space(8.0);
    }
}
