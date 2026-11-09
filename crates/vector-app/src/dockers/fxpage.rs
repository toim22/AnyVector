//! The page of one bitmap effect in the Properties docker (Bitmap Effects): its title bar (fold
//! arrow, name, reset and on/off) and its settings, drawn from the effect table (`fx.rs`).

use super::props::{edit_live, PropsUi};
use crate::app::VectorApp;
use crate::fx::{self, Ctl, Grad};
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use vector_core::object::{BitmapEffect, FxValue, Object};
use vector_core::Color;
use vector_render::bitfx;

/// What the page is editing.
struct Cx<'a> {
    app: &'a mut VectorApp,
    st: &'a mut PropsUi,
    o: &'a Object,
    /// The effect's place in the object's list.
    i: usize,
    e: BitmapEffect,
    /// The setting changed this frame (for the locks).
    last: Option<String>,
    changed: bool,
    /// Room kept at the right of the sliders (the bracket of a lock).
    reserve: f32,
    /// The row of the last slider or field drawn.
    row: Option<Rect>,
}

impl Cx<'_> {
    fn num(&self, key: &str, def: f64) -> f64 {
        self.e.num(&fx::resolve(&self.e, key), def)
    }
    fn set(&mut self, key: &str, v: FxValue) {
        let k = fx::resolve(&self.e, key);
        self.e.set(&k, v);
        self.last = Some(k);
        self.changed = true;
    }
    /// Whether the canvas is picking for `key` ("" the center, else a color setting).
    fn picking(&self, key: &str) -> bool {
        self.st.fx_pick.as_ref().is_some_and(|(i, k)| *i == self.i && k == key) && self.app.dockers.ui.pick_slot == SLOT_FX
    }
    /// Starts or stops picking on the canvas.
    fn pick(&mut self, key: &str) {
        if self.picking(key) {
            self.app.ts.pick_target = None;
            self.app.dockers.ui.pick_slot = 0;
            self.st.fx_pick = None;
            return;
        }
        self.app.ts.cancel();
        self.app.dockers.ui.pick_slot = SLOT_FX;
        self.app.ts.picked_point = None;
        self.app.ts.picked_color = None;
        self.app.ts.pick_target = Some(if key.is_empty() { crate::tools::PickTarget::DockerPoint } else { crate::tools::PickTarget::SampleColor });
        self.st.fx_pick = Some((self.i, key.to_string()));
    }
}

/// The pick slot of the effect pages (the center of an effect, a color from the document).
pub(crate) const SLOT_FX: u8 = 60;

/// Takes what the canvas picked for an effect page: the center of the effect (relative to the
/// object) or a color.
pub(crate) fn take_pick(app: &mut VectorApp, o: &Object, st: &mut PropsUi) {
    if app.dockers.ui.pick_slot != SLOT_FX {
        st.fx_pick = None;
        return;
    }
    let Some((i, key)) = st.fx_pick.clone() else {
        app.dockers.ui.pick_slot = 0;
        return;
    };
    let mut done = false;
    if key.is_empty() {
        if let Some(p) = app.ts.picked_point.take() {
            let b = o.visual_bounds();
            let u = ((p.x - b.x0) / b.width().max(1e-9)).clamp(0.0, 1.0);
            let v = ((p.y - b.y0) / b.height().max(1e-9)).clamp(0.0, 1.0);
            crate::propbar::edit_sel(app, "Effect Center", |x| {
                if let Some(e) = x.effects.bitmap.get_mut(i) {
                    e.set("CenterX", FxValue::Num(u));
                    e.set("CenterY", FxValue::Num(v));
                }
            });
            done = true;
        }
    } else if let Some(c) = app.ts.picked_color.take() {
        crate::propbar::edit_sel(app, "Effect Color", |x| {
            if let Some(e) = x.effects.bitmap.get_mut(i) {
                e.set(&key, FxValue::Color(c));
            }
        });
        done = true;
    }
    if done || app.ts.pick_target.is_none() {
        st.fx_pick = None;
        app.dockers.ui.pick_slot = 0;
    }
}

const W_SPIN: f32 = 54.0;
const KNOB: Color32 = Color32::from_rgb(0x15, 0x3a, 0x58);
const KNOB_RING: Color32 = Color32::from_rgb(0x2a, 0x6c, 0x9c);

/// The title bar and the settings of effect `i` of the object.
pub(crate) fn effect_page(app: &mut VectorApp, ui: &mut Ui, o: &Object, i: usize, st: &mut PropsUi) {
    let Some(e) = o.effects.bitmap.get(i).cloned() else { return };
    let def = fx::by_id(&e.id);
    let open_id = egui::Id::unique(("pr_fx_open", e.id.clone()));
    let mut open = ui.ctx().data(|d| d.get_temp::<bool>(open_id).unwrap_or(true));
    let mut cx = Cx { app, st, o, i, e, last: None, changed: false, reserve: 0.0, row: None };
    // the title bar
    let w = ui.available_width();
    ui.horizontal(|ui| {
        ui.set_width(w);
        let (r, resp) = ui.allocate_exact_size(vec2(w - 70.0, 24.0), Sense::click());
        let a = pos2(r.left() + 6.0, r.center().y);
        if open {
            crate::widgets::chevron_up(ui.painter(), a, 3.5, theme::TEXT);
        } else {
            crate::widgets::chevron(ui.painter(), a, 3.5, theme::TEXT);
        }
        ui.painter().text(pos2(r.left() + 18.0, r.center().y), egui::Align2::LEFT_CENTER, fx::name(&cx.e), theme::bold(12.5), theme::TEXT);
        if resp.clicked() {
            open = !open;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut on = cx.e.enabled;
            if check_resp(ui, &mut on, "").on_hover_text("Turn the effect on or off").changed() {
                cx.e.enabled = on;
                cx.changed = true;
            }
            if let Some(d) = def {
                if super::icon_button(cx.app, ui, "reset", "Reset to the default settings", true) {
                    let mut n = d.create();
                    n.enabled = cx.e.enabled;
                    cx.e = n;
                    cx.changed = true;
                }
                if d.id == "ToneCurve" && super::icon_button(cx.app, ui, "fx_auto", "Auto tone curve", true) {
                    auto_curve(&mut cx);
                }
            }
        });
    });
    ui.ctx().data_mut(|d| d.insert_temp(open_id, open));
    if open {
        match def {
            Some(d) => {
                ui.add_space(2.0);
                let enabled = cx.e.enabled;
                ui.add_enabled_ui(enabled, |ui| ctls(&mut cx, ui, d.ctls));
            }
            None => super::hint(ui, "This effect has no settings."),
        }
    }
    if cx.changed {
        let e = cx.e.clone();
        let label = format!("{} Settings", fx::name(&e));
        edit_live(cx.app, &label, |x| {
            if let Some(t) = x.effects.bitmap.get_mut(i) {
                if t.id == e.id {
                    *t = e.clone();
                }
            }
        });
    }
}

/// The controls of a list, with the locks of the sliders around them.
fn ctls(cx: &mut Cx, ui: &mut Ui, list: &[Ctl]) {
    let mut j = 0;
    while j < list.len() {
        // a color without a label after a list of options sits on the row of the last option
        if let (Ctl::Radio { key, label, opts, def }, Some(after @ Ctl::Color { label: "", .. })) = (&list[j], list.get(j + 1)) {
            radio_list(cx, ui, key, label, opts, *def, Some(after));
            j += 2;
            continue;
        }
        // two values tied by a lock: the lock between them in a bracket at their right
        if let (Some(a), Some(lock @ Ctl::Lock { .. }), Some(b)) = (list.get(j), list.get(j + 1), list.get(j + 2)) {
            if key_of(a).is_some() && key_of(b).is_some() {
                cx.reserve = 26.0;
                ctl(cx, ui, a);
                let r1 = cx.row;
                ctl(cx, ui, lock);
                ctl(cx, ui, b);
                let r2 = cx.row;
                cx.reserve = 0.0;
                if let (Some(r1), Some(r2)) = (r1, r2) {
                    let x = ui.max_rect().right() - 12.0;
                    let x0 = r1.right() + 4.0;
                    let c = if ui.is_enabled() { Color32::from_gray(0xa0) } else { Color32::from_gray(0x55) };
                    let st = Stroke::new(1.0, c);
                    let (y1, y2) = (r1.center().y, r2.center().y);
                    ui.painter().line_segment([pos2(x0, y1), pos2(x, y1)], st);
                    ui.painter().line_segment([pos2(x, y1), pos2(x, y1 + 7.0)], st);
                    ui.painter().line_segment([pos2(x0, y2), pos2(x, y2)], st);
                    ui.painter().line_segment([pos2(x, y2), pos2(x, y2 - 7.0)], st);
                }
                j += 3;
                continue;
            }
        }
        ctl(cx, ui, &list[j]);
        j += 1;
    }
    // a lock keeps the settings before and after it equal
    for (j, c) in list.iter().enumerate() {
        let Ctl::Lock { key, .. } = c else { continue };
        if !cx.e.flag(key, false) || j == 0 || j + 1 >= list.len() {
            continue;
        }
        let (Some(a), Some(b)) = (key_of(&list[j - 1]), key_of(&list[j + 1])) else { continue };
        let (a, b) = (fx::resolve(&cx.e, a), fx::resolve(&cx.e, b));
        match cx.last.clone() {
            Some(k) if k == a => {
                let v = cx.e.num(&a, 0.0);
                cx.e.set(&b, FxValue::Num(v));
            }
            Some(k) if k == b => {
                let v = cx.e.num(&b, 0.0);
                cx.e.set(&a, FxValue::Num(v));
            }
            _ => {}
        }
    }
}

/// A list of options, one under the other (`after`: a color on the row of the last one).
#[allow(clippy::too_many_arguments)]
fn radio_list(cx: &mut Cx, ui: &mut Ui, key: &str, label: &str, opts: &[&str], def: usize, after: Option<&Ctl>) {
    text(ui, label);
    let cur = cx.num(key, def as f64).round() as usize;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for (k, o) in opts.iter().enumerate() {
            ui.horizontal(|ui| {
                if radio_row(ui, cur == k, o.trim()) {
                    cx.set(key, FxValue::Num(k as f64));
                }
                if let (true, Some(Ctl::Color { key: ck, def: cd, dropper, .. })) = (k + 1 == opts.len(), after) {
                    ui.add_space(8.0);
                    color_row(cx, ui, ck, *cd, *dropper);
                }
            });
        }
    });
    ui.add_space(4.0);
}

/// An option button of the effect pages (a small circle, the dot when chosen).
fn radio_row(ui: &mut Ui, on: bool, label: &str) -> bool {
    let en = ui.is_enabled();
    let galley_w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_string(), egui::FontId::proportional(12.5), theme::TEXT).size().x);
    let (r, resp) = ui.allocate_exact_size(vec2(galley_w + 22.0, 18.0), if en { Sense::click() } else { Sense::hover() });
    let c = pos2(r.left() + 7.0, r.center().y);
    let p = ui.painter();
    p.circle_stroke(c, 5.5, Stroke::new(1.0, if en { Color32::from_gray(0xb0) } else { Color32::from_gray(0x55) }));
    if on {
        p.circle_filled(c, 3.0, if en { Color32::from_gray(0xe8) } else { Color32::from_gray(0x70) });
    }
    p.text(pos2(r.left() + 19.0, r.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), if en { theme::TEXT } else { theme::DISABLED });
    resp.clicked() && en && !on
}

/// A check box as the reference draws it: a box, light and ticked when on.
fn check_resp(ui: &mut Ui, on: &mut bool, label: &str) -> egui::Response {
    let en = ui.is_enabled();
    let tw = if label.is_empty() { 0.0 } else { ui.fonts_mut(|f| f.layout_no_wrap(label.to_string(), egui::FontId::proportional(12.5), theme::TEXT).size().x) + 8.0 };
    let (r, mut resp) = ui.allocate_exact_size(vec2(16.0 + tw, 20.0), if en { Sense::click() } else { Sense::hover() });
    let b = Rect::from_center_size(pos2(r.left() + 7.0, r.center().y), vec2(12.0, 12.0));
    let p = ui.painter();
    if *on {
        p.rect_filled(b, 2.0, if en { Color32::from_gray(0xd6) } else { Color32::from_gray(0x5a) });
        let k = Color32::from_gray(0x22);
        p.line_segment([b.left_center() + vec2(2.5, 0.0), b.center_bottom() - vec2(0.5, 3.0)], Stroke::new(1.6, k));
        p.line_segment([b.center_bottom() - vec2(0.5, 3.0), b.right_top() + vec2(-2.5, 3.0)], Stroke::new(1.6, k));
    } else {
        p.rect_stroke(b, 2.0, Stroke::new(1.0, if en { Color32::from_gray(0x9a) } else { Color32::from_gray(0x50) }), egui::StrokeKind::Inside);
    }
    if !label.is_empty() {
        p.text(pos2(r.left() + 20.0, r.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), if en { theme::TEXT } else { theme::DISABLED });
    }
    if resp.clicked() && en {
        *on = !*on;
        resp.mark_changed();
    }
    resp
}

fn check(ui: &mut Ui, on: &mut bool, label: &str) -> bool {
    check_resp(ui, on, label).changed()
}

fn key_of(c: &Ctl) -> Option<&'static str> {
    match c {
        Ctl::Slider { key, .. } | Ctl::Spin { key, .. } => Some(key),
        _ => None,
    }
}

fn text(ui: &mut Ui, s: &str) {
    if !s.is_empty() {
        let c = if ui.is_enabled() { theme::TEXT } else { theme::DISABLED };
        ui.label(egui::RichText::new(s).size(12.5).color(c));
    }
}

fn ctl(cx: &mut Cx, ui: &mut Ui, c: &Ctl) {
    match c {
        Ctl::Slider { key, label, min, max, step, dec, log, def, unit, grad } => {
            text(ui, label);
            let mut v = cx.num(key, *def);
            let w = ui.available_width() - cx.reserve;
            let (changed, row) = slider(ui, &mut v, *min, *max, *step, *dec, *log, unit, *grad, w);
            cx.row = Some(row);
            if changed {
                cx.set(key, FxValue::Num(v));
            }
            ui.add_space(4.0);
        }
        Ctl::Spin { key, label, min, max, step, dec, def, unit } => {
            text(ui, label);
            let mut v = cx.num(key, *def);
            let suffix = if unit.is_empty() { String::new() } else { format!(" {unit}") };
            let r = ui.horizontal(|ui| super::spin(ui, &mut v, *min..=*max, *step, *dec as usize, &suffix, 80.0));
            cx.row = Some(r.response.rect.with_max_x(r.response.rect.left() + 80.0));
            if r.inner {
                cx.set(key, FxValue::Num(v));
            }
            ui.add_space(4.0);
        }
        Ctl::Angle { key, label, def, quarter } => {
            text(ui, label);
            let mut v = cx.num(key, *def);
            let max = if *quarter { 90.0 } else { 360.0 };
            ui.horizontal(|ui| {
                if dial(ui, &mut v, *quarter) {
                    cx.set(key, FxValue::Num(v));
                }
                ui.add_space(4.0);
                if super::spin(ui, &mut v, 0.0..=max, 1.0, 0, " °", 76.0) {
                    cx.set(key, FxValue::Num(v));
                }
            });
            ui.add_space(4.0);
        }
        Ctl::Radio { key, label, opts, def } => radio_list(cx, ui, key, label, opts, *def, None),
        Ctl::Icons { key, label, icons, def } => {
            text(ui, label);
            let cur = cx.num(key, *def as f64).round() as usize;
            // two rows of two when there are four (the corners of Page Curl)
            let per = if icons.len() == 4 { 2 } else { icons.len() };
            for chunk in icons.chunks(per).enumerate() {
                ui.horizontal(|ui| {
                    ui.add_space(if per == 2 { ui.available_width() / 2.0 - 30.0 } else { 0.0 });
                    for (k, (icon, tip)) in chunk.1.iter().enumerate() {
                        let n = chunk.0 * per + k;
                        if crate::widgets::icon_button(&mut cx.app.icons, ui, icon, tip, true, cur == n).clicked() {
                            cx.set(key, FxValue::Num(n as f64));
                        }
                    }
                });
            }
            ui.add_space(4.0);
        }
        Ctl::Check { key, label, def } => {
            let mut on = cx.num(key, if *def { 1.0 } else { 0.0 }) != 0.0;
            if check(ui, &mut on, label) {
                cx.set(key, FxValue::Num(if on { 1.0 } else { 0.0 }));
            }
        }
        Ctl::Combo { key, label, opts, def, inline } => {
            let mut cur = cx.num(key, *def as f64).round() as usize;
            let id = format!("fx_{}_{}", cx.e.id, key);
            let changed = if *inline {
                ui.horizontal(|ui| {
                    text(ui, label);
                    super::choice(ui, &id, &mut cur, opts, 90.0)
                })
                .inner
            } else {
                text(ui, label);
                super::choice(ui, &id, &mut cur, opts, 110.0)
            };
            if changed {
                cx.set(key, FxValue::Num(cur as f64));
            }
            ui.add_space(4.0);
        }
        Ctl::Color { key, label, def, dropper } => {
            text(ui, label);
            color_row(cx, ui, key, *def, *dropper);
            ui.add_space(4.0);
        }
        Ctl::Toggle { key, def, swatch, icon } => {
            let mut on = cx.num(key, if *def { 1.0 } else { 0.0 }) != 0.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if check(ui, &mut on, "") {
                    cx.set(key, FxValue::Num(if on { 1.0 } else { 0.0 }));
                }
                if icon.is_empty() {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    ui.painter().rect_filled(r, 0.0, Color32::from_rgb(swatch[0], swatch[1], swatch[2]));
                } else if crate::widgets::icon_button(&mut cx.app.icons, ui, icon, key, true, on).clicked() {
                    cx.set(key, FxValue::Num(if on { 0.0 } else { 1.0 }));
                }
            });
        }
        Ctl::Center { .. } => {
            ui.horizontal(|ui| {
                ui.add_space(ui.available_width() / 2.0 - 12.0);
                let picking = cx.picking("");
                let en = ui.is_enabled();
                if crate::widgets::icon_button(&mut cx.app.icons, ui, "fx_pick_center", "Pick the center of the effect on the object", en, picking).clicked() && en {
                    cx.pick("");
                }
            });
        }
        Ctl::Lock { key, def } => {
            let on = cx.num(key, if *def { 1.0 } else { 0.0 }) != 0.0;
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 22.0).max(0.0));
                let en = ui.is_enabled();
                let tip = if on { "Unlock the two values" } else { "Keep the two values equal" };
                if crate::widgets::icon_button_sized(&mut cx.app.icons, ui, if on { "lock" } else { "unlock" }, tip, en, false, 20.0, 14.0).clicked() && en {
                    cx.set(key, FxValue::Num(if on { 0.0 } else { 1.0 }));
                }
            });
        }
        Ctl::Seed { key, label } => {
            ui.horizontal(|ui| {
                let mut v = cx.num(key, 1.0);
                if ui.button(*label).clicked() {
                    let t = cx.app.time();
                    v = ((t * 7919.0) as i64).rem_euclid(999) as f64 + 1.0;
                    cx.set(key, FxValue::Num(v));
                }
                if super::spin(ui, &mut v, 1.0..=999.0, 1.0, 0, "", 70.0) {
                    cx.set(key, FxValue::Num(v));
                }
            });
        }
        Ctl::Thumbs { ops } => thumbs(cx, ui, ops),
        Ctl::Styles { key, opts, def } => styles(cx, ui, key, opts, *def),
        Ctl::Swatches { key, label, opts, def, split } => swatches(cx, ui, key, label, opts, *def, *split),
        Ctl::Heading(t) => text(ui, t),
        Ctl::Preview => preview(cx, ui),
        Ctl::Presets { opts } => {
            text(ui, "Presets");
            ui.horizontal(|ui| {
                // the reference's presets, then the ones saved with (+)
                let id = cx.e.id.clone();
                let user = cx.app.settings.fx_presets.get(&id).cloned().unwrap_or_default();
                let mut names: Vec<&str> = opts.to_vec();
                names.extend(user.iter().map(|u| u.0.as_str()));
                let mut cur = cx.st.fx_preset.get(&id).copied().unwrap_or(0).min(names.len().saturating_sub(1));
                if super::choice(ui, &format!("fx_presets_{id}"), &mut cur, &names, ui.available_width() - 70.0) {
                    cx.st.fx_preset.insert(id.clone(), cur);
                    let params = match cur.checked_sub(opts.len()) {
                        Some(k) => user.get(k).map(|u| u.1.clone()),
                        // Default: the effect's own settings; Last Used keeps the current ones
                        None if opts[cur] == "Default" => fx::FX.iter().find(|f| f.id == id).map(|f| f.create().params),
                        None => None,
                    };
                    if let Some(p) = params {
                        cx.e.params = p;
                        cx.changed = true;
                    }
                }
                if super::icon_button(cx.app, ui, "plus", "Add preset", true) {
                    let list = cx.app.settings.fx_presets.entry(id.clone()).or_default();
                    list.push((format!("Preset {}", list.len() + 1), cx.e.params.clone()));
                    cx.st.fx_preset.insert(id.clone(), opts.len() + list.len() - 1);
                }
                if super::icon_button(cx.app, ui, "minus", "Delete preset", cur >= opts.len()) {
                    if let Some(list) = cx.app.settings.fx_presets.get_mut(&id) {
                        if cur - opts.len() < list.len() {
                            list.remove(cur - opts.len());
                        }
                    }
                    cx.st.fx_preset.insert(id, 0);
                }
            });
        }
        Ctl::Texture { key, label } => texture_pick(cx, ui, key, label),
        Ctl::Curve { key, hist } => curve(cx, ui, key, *hist),
        Ctl::Histogram { markers } => {
            let h = histogram(cx, ui);
            let r = hist_box(ui, &h, 150.0);
            if *markers {
                threshold_markers(cx, ui, r);
            }
        }
        Ctl::Levels => levels(cx, ui),
        Ctl::Kernel => kernel(cx, ui),
        Ctl::Mesh => mesh(cx, ui),
        Ctl::Rotate3d => rotate3d(cx, ui),
        Ctl::Wheel => wheel(cx, ui),
        Ctl::Frames => frames(cx, ui),
        Ctl::When { key, val, ctls: inner } => {
            if (cx.e.num(key, 0.0) - val).abs() < 1e-9 {
                ctls(cx, ui, inner);
            }
        }
        Ctl::Enable { key, ctls: inner } => {
            let on = fx::enabled(&cx.e, key) && ui.is_enabled();
            ui.add_enabled_ui(on, |ui| ctls(cx, ui, inner));
        }
        Ctl::Tabs { key, tabs } => {
            let cur = cx.e.num(key, 0.0).round() as usize;
            ui.horizontal(|ui| {
                for (k, (icon, tip, _)) in tabs.iter().enumerate() {
                    if crate::widgets::icon_button(&mut cx.app.icons, ui, icon, tip, true, cur == k).clicked() {
                        cx.set(key, FxValue::Num(k as f64));
                    }
                }
            });
            ui.add_space(4.0);
            if let Some((_, _, inner)) = tabs.get(cur) {
                ctls(cx, ui, inner);
            }
        }
        Ctl::Row(inner) => match inner {
            // a list and its color: the label, then the list with the color after it
            [Ctl::Combo { key, label, opts, def, .. }, Ctl::Color { key: ck, label: "", def: cd, dropper }] => {
                text(ui, label);
                ui.horizontal(|ui| {
                    let mut cur = cx.num(key, *def as f64).round() as usize;
                    if super::choice(ui, &format!("fx_{}_{}", cx.e.id, key), &mut cur, opts, 110.0) {
                        cx.set(key, FxValue::Num(cur as f64));
                    }
                    ui.add_space(6.0);
                    color_row(cx, ui, ck, *cd, *dropper);
                });
                ui.add_space(4.0);
            }
            _ => {
                ui.horizontal_top(|ui| {
                    for c in inner.iter() {
                        ui.vertical(|ui| ctl(cx, ui, c));
                        ui.add_space(10.0);
                    }
                });
            }
        },
    }
}

fn color_row(cx: &mut Cx, ui: &mut Ui, key: &str, def: Option<[u8; 3]>, dropper: bool) {
    let k = fx::resolve(&cx.e, key);
    let mut c = match cx.e.params.get(&k) {
        Some(FxValue::Color(c)) => Some(*c),
        _ => def.map(|d| Color::rgb(d[0], d[1], d[2])),
    };
    ui.horizontal(|ui| {
        let en = ui.is_enabled();
        if super::color_pick_w(ui, &format!("fx_{}_{k}", cx.e.id), &mut c, def.is_none(), en, 52.0) {
            match c {
                Some(c) => cx.set(key, FxValue::Color(c)),
                None => {
                    cx.e.params.remove(&k);
                    cx.changed = true;
                }
            }
        }
        if dropper {
            let picking = cx.picking(&k);
            if crate::widgets::icon_button(&mut cx.app.icons, ui, "eyedropper", "Pick a color from the document", en, picking).clicked() && en {
                cx.pick(&k);
            }
        }
    });
}

// ------------------------------------------------------------------------------ slider

fn to_t(v: f64, lo: f64, hi: f64, log: bool) -> f32 {
    let t = if log && lo > 0.0 { (v.max(lo) / lo).ln() / (hi / lo).ln() } else { (v - lo) / (hi - lo).max(1e-9) };
    t.clamp(0.0, 1.0) as f32
}

fn from_t(t: f64, lo: f64, hi: f64, log: bool) -> f64 {
    if log && lo > 0.0 {
        lo * (hi / lo).powf(t)
    } else {
        lo + t * (hi - lo)
    }
}

/// The colors along a track.
fn grad_color(g: Grad, t: f32) -> Option<Color32> {
    let mix = |a: [u8; 3], b: [u8; 3], t: f32| Color32::from_rgb((a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8, (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8, (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8);
    match g {
        Grad::None => None,
        Grad::Hue => {
            let c = bitfx::hsl_to_rgb(t * 360.0, 1.0, 0.5);
            Some(Color32::from_rgb(c[0] as u8, c[1] as u8, c[2] as u8))
        }
        Grad::Two(a, b) => Some(mix(a, b, t)),
        Grad::Three(a, b, c) => Some(if t < 0.5 { mix(a, b, t * 2.0) } else { mix(b, c, t * 2.0 - 1.0) }),
    }
}

/// The slider of the effect pages: a track (plain or in colors) with a round knob, and the value
/// with arrows at its right.
#[allow(clippy::too_many_arguments)]
fn slider(ui: &mut Ui, v: &mut f64, lo: f64, hi: f64, step: f64, dec: u8, log: bool, unit: &str, g: Grad, w: f32) -> (bool, Rect) {
    let en = ui.is_enabled();
    let mut changed = false;
    let row = ui.horizontal(|ui| {
        let tw = (w - W_SPIN - 12.0).max(60.0);
        let (r, resp) = ui.allocate_exact_size(vec2(tw, 20.0), if en { Sense::click_and_drag() } else { Sense::hover() });
        let (x0, x1) = (r.left() + 6.0, r.right() - 6.0);
        if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
            let t = ((p.x - x0) / (x1 - x0)).clamp(0.0, 1.0) as f64;
            let mut n = from_t(t, lo, hi, log);
            let q = if step > 0.0 { step } else { 1.0 };
            n = (n / q).round() * q;
            *v = n.clamp(lo, hi);
            changed = true;
        }
        let track = Rect::from_min_max(pos2(r.left(), r.center().y - 4.0), pos2(r.right(), r.center().y + 4.0));
        let p = ui.painter();
        match g {
            Grad::None => {
                p.rect_filled(track, 4.0, if en { Color32::from_gray(0x5a) } else { Color32::from_gray(0x3a) });
            }
            _ => {
                let mut mesh = egui::epaint::Mesh::default();
                let n = 24;
                for k in 0..=n {
                    let t = k as f32 / n as f32;
                    let mut c = grad_color(g, t).unwrap_or(Color32::GRAY);
                    if !en {
                        c = c.gamma_multiply(0.35);
                    }
                    let x = track.left() + track.width() * t;
                    mesh.colored_vertex(pos2(x, track.top()), c);
                    mesh.colored_vertex(pos2(x, track.bottom()), c);
                    if k > 0 {
                        let b = (k as u32) * 2;
                        mesh.add_triangle(b - 2, b - 1, b);
                        mesh.add_triangle(b - 1, b, b + 1);
                    }
                }
                p.add(egui::Shape::mesh(mesh));
            }
        }
        let kx = x0 + to_t(*v, lo, hi, log) * (x1 - x0);
        let kc = pos2(kx, r.center().y);
        p.circle_filled(kc, 6.0, if en { KNOB } else { Color32::from_gray(0x30) });
        p.circle_stroke(kc, 6.0, Stroke::new(1.2, if en { KNOB_RING } else { Color32::from_gray(0x50) }));
        ui.add_space(4.0);
        let suffix = if unit.is_empty() { String::new() } else { format!(" {unit}") };
        if super::spin(ui, v, lo..=hi, step.max(0.01), dec as usize, &suffix, W_SPIN) {
            changed = true;
        }
    });
    (changed, row.response.rect)
}

/// The direction dial: a circle with a dot at the angle; drag around it to turn.
fn dial(ui: &mut Ui, v: &mut f64, quarter: bool) -> bool {
    let en = ui.is_enabled();
    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), if en { Sense::click_and_drag() } else { Sense::hover() });
    let c = r.center();
    let mut changed = false;
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        let mut a = (-(p.y - c.y)).atan2(p.x - c.x).to_degrees() as f64;
        if a < 0.0 {
            a += 360.0;
        }
        if quarter {
            a = a.clamp(0.0, 90.0);
        }
        *v = a.round();
        changed = true;
    }
    let p = ui.painter();
    p.circle_stroke(c, 11.0, Stroke::new(1.2, if en { theme::TEXT } else { theme::DISABLED }));
    let a = (*v as f32).to_radians();
    p.circle_filled(c + vec2(a.cos(), -a.sin()) * 6.5, 3.5, if en { theme::TEXT } else { theme::DISABLED });
    changed
}

// ------------------------------------------------------------------------------ pictures

/// A texture made once per key (kept in the egui memory).
fn texture(ui: &Ui, key: u64, make: impl FnOnce() -> egui::ColorImage) -> egui::TextureHandle {
    let id = egui::Id::unique(("fx_tex", key));
    if let Some(t) = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    let t = ui.ctx().load_texture(format!("fx_tex_{key}"), make(), egui::TextureOptions::LINEAR);
    ui.ctx().data_mut(|d| d.insert_temp(id, t.clone()));
    t
}

fn hash_of(e: &BitmapEffect, extra: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(e).unwrap_or_default().hash(&mut h);
    extra.hash(&mut h);
    h.finish()
}

/// The sample pictures of the previews: a checkerboard (the distortions) or a yellow ellipse on
/// blue (the tune thumbnails).
fn sample(w: u32, h: u32, checker: bool) -> tiny_skia::Pixmap {
    let mut pm = tiny_skia::Pixmap::new(w, h).expect("preview size");
    let data = pm.data_mut();
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let c = if checker {
                if ((x / 10) + (y / 10)) % 2 == 0 {
                    [0, 0, 0]
                } else {
                    [255, 255, 255]
                }
            } else {
                let (u, v) = ((x as f32 + 0.5) / w as f32 - 0.5, (y as f32 + 0.5) / h as f32 - 0.5);
                let d = (u / 0.4).powi(2) + (v / 0.3).powi(2);
                if d < 0.94 {
                    [0xf2, 0xc6, 0x2e]
                } else if d < 1.0 {
                    [0x5a, 0x4a, 0x20]
                } else {
                    [0x4a, 0x86, 0xc8]
                }
            };
            data[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    pm
}

fn to_image(pm: &tiny_skia::Pixmap) -> egui::ColorImage {
    egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data())
}

/// The effect drawn on a sample picture.
fn effect_image(e: &BitmapEffect, w: u32, h: u32, checker: bool) -> egui::ColorImage {
    let mut pm = sample(w, h, checker);
    let cx = bitfx::FxCtx { s: 0.6, frame: [0.0, 0.0, w as f32, h as f32], seed: 3 };
    let mut e = e.clone();
    e.enabled = true;
    bitfx::apply(&e, &mut pm, &cx);
    to_image(&pm)
}

fn centered_image(ui: &mut Ui, tex: &egui::TextureHandle, size: egui::Vec2) -> Rect {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, size.y), Sense::hover());
    let ir = Rect::from_center_size(r.center(), size);
    ui.painter().image(tex.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), if ui.is_enabled() { Color32::WHITE } else { Color32::from_gray(110) });
    ir
}

fn preview(cx: &mut Cx, ui: &mut Ui) {
    let e = cx.e.clone();
    let tex = texture(ui, hash_of(&e, "preview"), || effect_image(&e, 100, 100, true));
    centered_image(ui, &tex, vec2(100.0, 100.0));
    ui.add_space(4.0);
}

/// Tune pages: a thumbnail per step; a click adds the step.
fn thumbs(cx: &mut Cx, ui: &mut Ui, ops: &[&str]) {
    let mut add = None;
    egui::Grid::new(("fx_thumbs", cx.e.id.clone())).spacing(vec2(18.0, 6.0)).show(ui, |ui| {
        for (k, name) in ops.iter().enumerate() {
            ui.vertical(|ui| {
                text(ui, name);
                let mut e = cx.e.clone();
                let mut list = e.list("Ops").map(|l| l.to_vec()).unwrap_or_default();
                list.push(k as f64);
                e.set("Ops", FxValue::List(list));
                let tex = texture(ui, hash_of(&e, "thumb"), || effect_image(&e, 98, 78, false));
                let (r, resp) = ui.allocate_exact_size(vec2(98.0, 78.0), Sense::click());
                ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                if resp.hovered() {
                    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.5, theme::ACCENT), egui::StrokeKind::Outside);
                }
                if resp.clicked() {
                    add = Some(k);
                }
            });
            if k % 2 == 1 {
                ui.end_row();
            }
        }
    });
    if let Some(k) = add {
        let mut list = cx.e.list("Ops").map(|l| l.to_vec()).unwrap_or_default();
        list.push(k as f64);
        cx.set("Ops", FxValue::List(list));
    }
    ui.add_space(6.0);
}

/// Thumbnails of which one is chosen (Time Machine).
fn styles(cx: &mut Cx, ui: &mut Ui, key: &str, opts: &[&str], def: usize) {
    let cur = cx.num(key, def as f64).round() as usize;
    let mut pick = None;
    egui::Grid::new(("fx_styles", cx.e.id.clone())).spacing(vec2(14.0, 10.0)).show(ui, |ui| {
        for (k, name) in opts.iter().enumerate() {
            if k + 1 == opts.len() && k % 3 == 0 {
                ui.allocate_exact_size(vec2(58.0, 50.0), Sense::hover());
            }
            let mut e = cx.e.clone();
            e.set(key, FxValue::Num(k as f64));
            e.set("PhotoEdges", FxValue::Num(1.0));
            let tex = texture(ui, hash_of(&e, "style"), || effect_image(&e, 58, 50, false));
            let (r, resp) = ui.allocate_exact_size(vec2(58.0, 50.0), Sense::click());
            let resp = resp.on_hover_text(*name);
            ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            if cur == k {
                ui.painter().rect_stroke(r.expand(2.0), 0.0, Stroke::new(1.5, theme::ACCENT), egui::StrokeKind::Outside);
            }
            if resp.clicked() {
                pick = Some(k);
            }
            if k % 3 == 2 {
                ui.end_row();
            }
        }
    });
    if let Some(k) = pick {
        cx.set(key, FxValue::Num(k as f64));
    }
    ui.add_space(6.0);
}

fn swatches(cx: &mut Cx, ui: &mut Ui, key: &str, label: &str, opts: &[(&str, Option<[u8; 3]>)], def: usize, split: usize) {
    text(ui, label);
    let cur = cx.num(key, def as f64).round() as usize;
    let mut pick = None;
    let groups: Vec<(usize, &[(&str, Option<[u8; 3]>)])> = if split > 0 { vec![(0, &opts[..split]), (split, &opts[split..])] } else { vec![(0, opts)] };
    for (gi, (start, g)) in groups.into_iter().enumerate() {
        if gi == 1 {
            text(ui, "Grays");
        }
        ui.horizontal(|ui| {
            for (k, (name, c)) in g.iter().enumerate() {
                let n = start + k;
                let (r, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
                let resp = resp.on_hover_text(*name);
                let sw = Rect::from_center_size(r.center(), vec2(14.0, 14.0));
                match c {
                    Some(c) => {
                        ui.painter().rect_filled(sw, 0.0, Color32::from_rgb(c[0], c[1], c[2]));
                    }
                    None => {
                        // all colors: four quarters
                        for (q, col) in [(0, Color32::RED), (1, Color32::from_rgb(0, 200, 0)), (2, Color32::BLUE), (3, Color32::YELLOW)] {
                            let (dx, dy) = ((q % 2) as f32 * 7.0, (q / 2) as f32 * 7.0);
                            ui.painter().rect_filled(Rect::from_min_size(sw.min + vec2(dx, dy), vec2(7.0, 7.0)), 0.0, col);
                        }
                    }
                }
                if cur == n {
                    ui.painter().rect_stroke(r.shrink(1.0), 2.0, Stroke::new(1.5, theme::ACCENT), egui::StrokeKind::Inside);
                }
                if resp.clicked() {
                    pick = Some(n);
                }
            }
        });
    }
    if let Some(n) = pick {
        cx.set(key, FxValue::Num(n as f64));
    }
    ui.add_space(4.0);
}

fn texture_pick(cx: &mut Cx, ui: &mut Ui, key: &str, label: &str) {
    text(ui, label);
    let cur = cx.num(key, 0.0).round() as usize;
    let tex_of = |ui: &Ui, k: usize| {
        texture(ui, 0x7e47_0000 + k as u64, || {
            let (w, h) = (120usize, 80usize);
            let mut img = egui::ColorImage::new([w, h], vec![Color32::BLACK; w * h]);
            for y in 0..h {
                for x in 0..w {
                    let v = (bitfx_texture(k, x as f32, y as f32) * 255.0) as u8;
                    img.pixels[y * w + x] = Color32::from_gray(v);
                }
            }
            img
        })
    };
    let t = tex_of(ui, cur);
    let (r, resp) = ui.allocate_exact_size(vec2(160.0, 90.0), Sense::click());
    ui.painter().rect_filled(r, 2.0, Color32::from_rgb(0x33, 0x33, 0x33));
    let ir = Rect::from_min_size(r.min + vec2(4.0, 5.0), vec2(120.0, 80.0));
    ui.painter().image(t.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    crate::widgets::chevron(ui.painter(), pos2(r.right() - 10.0, r.center().y), 3.0, theme::TEXT);
    let mut pick = None;
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        egui::Grid::new(("fx_tex_grid", key)).spacing(vec2(6.0, 6.0)).show(ui, |ui| {
            for (k, name) in vector_render::bitfx_art::TEXTURES.iter().enumerate() {
                let t = tex_of(ui, k);
                let (r, resp) = ui.allocate_exact_size(vec2(72.0, 48.0), Sense::click());
                ui.painter().image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(0.6, 0.6)), Color32::WHITE);
                if k == cur {
                    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.5, theme::ACCENT), egui::StrokeKind::Outside);
                }
                if resp.on_hover_text(*name).clicked() {
                    pick = Some(k);
                }
                if k % 3 == 2 {
                    ui.end_row();
                }
            }
        });
    });
    if let Some(k) = pick {
        cx.set(key, FxValue::Num(k as f64));
    }
    ui.add_space(4.0);
}

fn bitfx_texture(k: usize, x: f32, y: f32) -> f32 {
    vector_render::bitfx_art::texture(k, x, y, 1)
}

// ------------------------------------------------------------------------------ histogram, curves

/// The histogram of the object without its effects (luminance, red, green, blue).
fn histogram(cx: &Cx, ui: &Ui) -> std::sync::Arc<[[u32; 256]; 4]> {
    let mut plain = cx.o.clone();
    plain.effects.bitmap.clear();
    let key = vector_render::obj_hash(&plain);
    let id = egui::Id::unique(("fx_hist", key));
    if let Some(h) = ui.ctx().data(|d| d.get_temp::<std::sync::Arc<[[u32; 256]; 4]>>(id)) {
        return h;
    }
    let h = cx
        .app
        .doc()
        .and_then(|d| vector_render::render_objects_image(&d.doc, std::slice::from_ref(&plain), 160, None))
        .map(|pm| bitfx::histogram(&pm))
        .unwrap_or([[0; 256]; 4]);
    let h = std::sync::Arc::new(h);
    ui.ctx().data_mut(|d| d.insert_temp(id, h.clone()));
    h
}

/// Draws the histogram in a box `h` high across the docker; gives the box.
fn hist_box(ui: &mut Ui, h: &[[u32; 256]; 4], height: f32) -> Rect {
    let w = ui.available_width() - 6.0;
    let (r, _) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
    paint_hist(ui.painter(), r, h);
    r
}

fn paint_hist(p: &egui::Painter, r: Rect, h: &[[u32; 256]; 4]) {
    p.rect_filled(r, 0.0, Color32::from_rgb(0x22, 0x22, 0x22));
    p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_gray(0x60)), egui::StrokeKind::Inside);
    for k in 1..4 {
        let x = r.left() + r.width() * k as f32 / 4.0;
        let y = r.top() + r.height() * k as f32 / 4.0;
        p.line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new(1.0, Color32::from_gray(0x44)));
        p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.0, Color32::from_gray(0x44)));
    }
    let max = h.iter().flat_map(|c| c.iter()).copied().max().unwrap_or(1).max(1) as f32;
    let colors = [Color32::from_gray(200), Color32::from_rgb(230, 40, 40), Color32::from_rgb(40, 200, 40), Color32::from_rgb(90, 90, 240)];
    for (c, col) in h.iter().zip(colors) {
        let pts: Vec<egui::Pos2> = (0..256).map(|i| pos2(r.left() + r.width() * i as f32 / 255.0, r.bottom() - 1.0 - (c[i] as f32 / max).sqrt() * (r.height() - 4.0))).collect();
        p.add(egui::Shape::line(pts, Stroke::new(1.0, col)));
    }
}

fn threshold_markers(cx: &mut Cx, ui: &mut Ui, r: Rect) {
    let (mr, resp) = ui.allocate_exact_size(vec2(r.width(), 12.0), Sense::click_and_drag());
    let bi = cx.e.num("Level", 0.0) >= 2.0;
    let keys: &[&str] = if bi { &["Low", "Mid", "High"] } else { &["Low", "Mid"] };
    for k in keys {
        let v = cx.e.num(k, if *k == "High" { 255.0 } else if *k == "Mid" { 127.0 } else { 0.0 }) as f32;
        let x = mr.left() + mr.width() * v / 255.0;
        ui.painter().add(egui::Shape::convex_polygon(vec![pos2(x, mr.top()), pos2(x - 4.0, mr.bottom()), pos2(x + 4.0, mr.bottom())], Color32::TRANSPARENT, Stroke::new(1.0, theme::TEXT)));
        ui.painter().line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new(1.0, Color32::from_gray(0xc0)));
    }
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        let v = (((p.x - mr.left()) / mr.width()).clamp(0.0, 1.0) * 255.0).round() as f64;
        // the nearest marker follows
        let k = keys.iter().min_by(|a, b| (cx.e.num(a, 0.0) - v).abs().partial_cmp(&(cx.e.num(b, 0.0) - v).abs()).unwrap()).copied().unwrap_or("Mid");
        cx.set(k, FxValue::Num(v));
    }
}

/// Levels: the output levels above the histogram, the input levels (black, gamma, white) under it.
fn levels(cx: &mut Cx, ui: &mut Ui) {
    let ch = cx.e.num("Channel", 0.0).round() as usize;
    let k = |n: &str| format!("{n}{ch}");
    let mut ol = cx.e.num(&k("OutputLow"), 0.0);
    let mut oh = cx.e.num(&k("OutputHigh"), 255.0);
    ui.horizontal(|ui| {
        if super::spin(ui, &mut ol, 0.0..=255.0, 1.0, 0, "", 54.0) {
            cx.set(&k("OutputLow"), FxValue::Num(ol));
        }
        ui.add_space((ui.available_width() - 56.0).max(0.0));
        if super::spin(ui, &mut oh, 0.0..=255.0, 1.0, 0, "", 54.0) {
            cx.set(&k("OutputHigh"), FxValue::Num(oh));
        }
    });
    let h = histogram(cx, ui);
    let r = hist_box(ui, &h, 120.0);
    let (il, g, ih) = (cx.e.num(&k("InputLow"), 0.0), cx.e.num(&k("Gamma"), 1.0), cx.e.num(&k("InputHigh"), 255.0));
    // the markers: output above, input (with the gamma between) under
    let p = ui.painter();
    let x_of = |v: f64| r.left() + r.width() * (v as f32 / 255.0);
    for (v, top) in [(ol, true), (oh, true), (il, false), (ih, false)] {
        let x = x_of(v);
        let y = if top { r.top() - 1.0 } else { r.bottom() + 1.0 };
        let d = if top { -6.0 } else { 6.0 };
        p.add(egui::Shape::convex_polygon(vec![pos2(x, y), pos2(x - 4.0, y + d), pos2(x + 4.0, y + d)], Color32::from_gray(0x30), Stroke::new(1.0, theme::TEXT)));
    }
    let gx = x_of(il + (ih - il) * 0.5f64.powf(g));
    p.circle_filled(pos2(gx, r.bottom() + 4.0), 3.0, Color32::from_gray(0x90));
    ui.add_space(8.0);
    let (mut il2, mut g2, mut ih2) = (il, g, ih);
    ui.horizontal(|ui| {
        if super::spin(ui, &mut il2, 0.0..=254.0, 1.0, 0, "", 54.0) {
            cx.set(&k("InputLow"), FxValue::Num(il2));
        }
        ui.add_space((ui.available_width() / 2.0 - 60.0).max(0.0));
        if super::spin(ui, &mut g2, 0.1..=9.99, 0.1, 1, "", 54.0) {
            cx.set(&k("Gamma"), FxValue::Num(g2));
        }
        ui.add_space((ui.available_width() - 56.0).max(0.0));
        if super::spin(ui, &mut ih2, 1.0..=255.0, 1.0, 0, "", 54.0) {
            cx.set(&k("InputHigh"), FxValue::Num(ih2));
        }
    });
}

/// The points of a curve (pairs 0..255) or its starting line.
fn curve_points(e: &BitmapEffect, key: &str, shear: bool) -> Vec<(f32, f32)> {
    match e.list(key) {
        Some(l) if l.len() >= 4 => l.chunks_exact(2).map(|p| (p[0] as f32, p[1] as f32)).collect(),
        _ if shear => vec![(0.0, 127.5), (255.0, 127.5)],
        _ => vec![(0.0, 0.0), (255.0, 255.0)],
    }
}

/// The curve editor: drag a point, click to add one, double-click a point to remove it.
fn curve(cx: &mut Cx, ui: &mut Ui, key: &str, hist: bool) {
    let shear = !hist;
    let k = fx::resolve(&cx.e, key);
    let mut pts = curve_points(&cx.e, &k, shear);
    let w = (ui.available_width() - 8.0).min(280.0);
    let size = if shear { vec2(w * 0.55, w * 0.55) } else { vec2(w, w * 0.9) };
    let (outer, _) = ui.allocate_exact_size(vec2(ui.available_width(), size.y + 8.0), Sense::hover());
    let r = Rect::from_min_size(pos2(outer.center().x - size.x / 2.0, outer.top() + 4.0), size);
    let resp = ui.interact(r.expand(6.0), ui.scope_id().with(("fx_curve", &k)), Sense::click_and_drag());
    let to_s = |p: (f32, f32)| pos2(r.left() + r.width() * p.0 / 255.0, r.bottom() - r.height() * p.1 / 255.0);
    let from_s = |p: egui::Pos2| (((p.x - r.left()) / r.width() * 255.0).clamp(0.0, 255.0), ((r.bottom() - p.y) / r.height() * 255.0).clamp(0.0, 255.0));
    let drag_id = ui.scope_id().with(("fx_curve_drag", &k));
    let mut dragging: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_id)).flatten();
    let mut changed = false;
    if resp.drag_started() || resp.clicked() || resp.double_clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let near = pts.iter().position(|q| to_s(*q).distance(p) < 7.0);
            if resp.double_clicked() {
                if let Some(i) = near.filter(|i| *i > 0 && *i + 1 < pts.len()) {
                    pts.remove(i);
                    changed = true;
                }
            } else if let Some(i) = near {
                dragging = Some(i);
            } else {
                let q = from_s(p);
                let i = pts.iter().position(|a| a.0 > q.0).unwrap_or(pts.len());
                pts.insert(i, q);
                dragging = Some(i);
                changed = true;
            }
        }
    }
    if resp.dragged() {
        if let (Some(i), Some(p)) = (dragging, resp.interact_pointer_pos()) {
            let mut q = from_s(p);
            let lo = if i > 0 { pts[i - 1].0 + 1.0 } else { 0.0 };
            let hi = if i + 1 < pts.len() { pts[i + 1].0 - 1.0 } else { 255.0 };
            // the end points stay at the ends
            q.0 = if i == 0 { 0.0 } else if i + 1 == pts.len() { 255.0 } else { q.0.clamp(lo, hi) };
            pts[i] = q;
            changed = true;
        }
    }
    if resp.drag_stopped() {
        dragging = None;
    }
    ui.ctx().data_mut(|d| d.insert_temp(drag_id, dragging));
    let p = ui.painter();
    if hist {
        let h = histogram(cx, ui);
        paint_hist(ui.painter(), r, &h);
    } else {
        p.rect_filled(r, 0.0, Color32::BLACK);
        for k2 in 1..10 {
            let x = r.left() + r.width() * k2 as f32 / 10.0;
            let y = r.top() + r.height() * k2 as f32 / 10.0;
            p.line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new(1.0, Color32::from_gray(0xd0)));
            p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.0, Color32::from_gray(0xd0)));
        }
    }
    let lut = bitfx::curve_lut(&pts);
    let line: Vec<egui::Pos2> = (0..256).step_by(3).chain(std::iter::once(255)).map(|i| to_s((i as f32, lut[i]))).collect();
    let p = ui.painter();
    p.add(egui::Shape::line(line, Stroke::new(1.3, Color32::WHITE)));
    for q in &pts {
        p.circle_filled(to_s(*q), 4.0, Color32::WHITE);
        p.circle_stroke(to_s(*q), 4.0, Stroke::new(1.0, Color32::BLACK));
    }
    if changed {
        cx.set(key, FxValue::List(pts.iter().flat_map(|q| [q.0 as f64, q.1 as f64]).collect()));
    }
    if hist {
        // the selected point's position
        let sel = dragging.and_then(|i| pts.get(i)).copied().unwrap_or((0.0, 0.0));
        ui.horizontal(|ui| {
            ui.add_enabled_ui(false, |ui| {
                let (mut x, mut y) = (sel.0.round() as f64, sel.1.round() as f64);
                ui.label("X:");
                super::spin(ui, &mut x, 0.0..=255.0, 1.0, 0, "", 50.0);
                ui.label("Y:");
                super::spin(ui, &mut y, 0.0..=255.0, 1.0, 0, "", 50.0);
            });
        });
    }
}

/// Auto Tone Curve: stretches the levels of the object between its darkest and lightest 0.5 %.
fn auto_curve(cx: &mut Cx) {
    let mut plain = cx.o.clone();
    plain.effects.bitmap.clear();
    let Some(pm) = cx.app.doc().and_then(|d| vector_render::render_objects_image(&d.doc, std::slice::from_ref(&plain), 160, None)) else { return };
    let h = bitfx::histogram(&pm);
    let n: u32 = h[0].iter().sum();
    let cut = (n as f32 * 0.005) as u32;
    let (mut lo, mut hi, mut acc) = (0usize, 255usize, 0u32);
    for (i, v) in h[0].iter().enumerate() {
        acc += v;
        if acc > cut {
            lo = i;
            break;
        }
    }
    acc = 0;
    for (i, v) in h[0].iter().enumerate().rev() {
        acc += v;
        if acc > cut {
            hi = i;
            break;
        }
    }
    let ch = cx.e.num("ActiveChannel", 0.0).round() as usize;
    cx.set(&format!("Curve{ch}"), FxValue::List(vec![0.0, 0.0, lo as f64, 0.0, hi.max(lo + 1) as f64, 255.0, 255.0, 255.0]));
}

// ------------------------------------------------------------------------------ special pages

/// User Defined: the 5 × 5 numbers (mirrored when the kernel is symmetric).
fn kernel(cx: &mut Cx, ui: &mut Ui) {
    let sym = cx.e.flag("SymmetricKernel", true);
    egui::Grid::new(("fx_kernel", cx.e.id.clone())).spacing(vec2(6.0, 6.0)).show(ui, |ui| {
        for j in 0..5 {
            for i in 0..5 {
                let def = match (j, i) {
                    (2, 2) => 5.0,
                    (1, 2) | (2, 1) | (2, 3) | (3, 2) => -1.0,
                    _ => 0.0,
                };
                let key = format!("Kernel_{j}{i}");
                let mut v = cx.e.num(&key, def);
                let resp = ui.add_sized(vec2(40.0, 24.0), egui::DragValue::new(&mut v).range(-999.0..=999.0).speed(0.2).max_decimals(0));
                ui.painter().rect_stroke(resp.rect, 2.0, Stroke::new(1.0, Color32::from_gray(0x5a)), egui::StrokeKind::Inside);
                if resp.changed() {
                    let mut set = vec![(j, i)];
                    if sym {
                        set.extend([(j, 4 - i), (4 - j, i), (4 - j, 4 - i)]);
                    }
                    for (a, b) in set {
                        cx.e.set(&format!("Kernel_{a}{b}"), FxValue::Num(v));
                    }
                    cx.changed = true;
                }
            }
            ui.end_row();
        }
    });
    ui.add_space(4.0);
}

/// Mesh Warp: the sample picture under the mesh; drag the nodes.
fn mesh(cx: &mut Cx, ui: &mut Ui) {
    let g = cx.e.num("Gridlines", 4.0).round().clamp(2.0, 10.0) as usize;
    let n = (g + 1) * (g + 1);
    let mut nodes: Vec<(f32, f32)> = match cx.e.list("Mesh") {
        Some(l) if l.len() == n * 2 => l.chunks_exact(2).map(|p| (p[0] as f32, p[1] as f32)).collect(),
        _ => (0..n).map(|k| ((k % (g + 1)) as f32 / g as f32, (k / (g + 1)) as f32 / g as f32)).collect(),
    };
    let w = ui.available_width() - 8.0;
    let (outer, _) = ui.allocate_exact_size(vec2(w, w * 0.95), Sense::hover());
    ui.painter().rect_stroke(outer, 0.0, Stroke::new(1.0, Color32::from_gray(0x80)), egui::StrokeKind::Inside);
    let r = Rect::from_center_size(outer.center(), vec2(w * 0.8, w * 0.55));
    let e = cx.e.clone();
    let tex = texture(ui, hash_of(&e, "mesh"), || effect_image(&e, 160, 110, false));
    ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    let to_s = |p: (f32, f32)| pos2(r.left() + r.width() * p.0, r.top() + r.height() * p.1);
    let p = ui.painter();
    for j in 0..=g {
        for i in 0..=g {
            let a = to_s(nodes[j * (g + 1) + i]);
            if i < g {
                p.add(egui::Shape::dashed_line(&[a, to_s(nodes[j * (g + 1) + i + 1])], Stroke::new(1.0, if j == g / 2 { Color32::RED } else { Color32::WHITE }), 4.0, 3.0));
            }
            if j < g {
                p.add(egui::Shape::dashed_line(&[a, to_s(nodes[(j + 1) * (g + 1) + i])], Stroke::new(1.0, if i == g / 2 { Color32::RED } else { Color32::WHITE }), 4.0, 3.0));
            }
        }
    }
    let mut changed = false;
    for j in 1..g {
        for i in 1..g {
            let k = j * (g + 1) + i;
            let c = to_s(nodes[k]);
            let resp = ui.interact(Rect::from_center_size(c, vec2(12.0, 12.0)), ui.scope_id().with(("fx_mesh", k)), Sense::drag());
            if resp.dragged() {
                if let Some(pp) = resp.interact_pointer_pos() {
                    nodes[k] = (((pp.x - r.left()) / r.width()).clamp(-0.2, 1.2), ((pp.y - r.top()) / r.height()).clamp(-0.2, 1.2));
                    changed = true;
                }
            }
            ui.painter().circle_filled(c, 5.0, Color32::WHITE);
            ui.painter().circle_stroke(c, 5.0, Stroke::new(1.0, Color32::BLACK));
        }
    }
    if changed {
        cx.set("Mesh", FxValue::List(nodes.iter().flat_map(|q| [q.0 as f64, q.1 as f64]).collect()));
    }
}

/// 3D Rotate: a box seen at the angles; drag it to turn.
fn rotate3d(cx: &mut Cx, ui: &mut Ui) {
    let w = ui.available_width();
    let (outer, resp) = ui.allocate_exact_size(vec2(w, 160.0), Sense::drag());
    if resp.dragged() {
        let d = resp.drag_delta();
        let h = (cx.e.num("HorizAngle", 0.0) + d.x as f64 * 0.6).clamp(-75.0, 75.0).round();
        let v = (cx.e.num("VertAngle", 0.0) - d.y as f64 * 0.6).clamp(-75.0, 75.0).round();
        cx.set("HorizAngle", FxValue::Num(h));
        cx.set("VertAngle", FxValue::Num(v));
    }
    let (h, v) = ((cx.e.num("HorizAngle", 0.0) as f32).to_radians(), (cx.e.num("VertAngle", 0.0) as f32).to_radians());
    let c = outer.center();
    let s = 50.0;
    let proj = |x: f32, y: f32, z: f32| {
        let (x1, z1) = (x * h.cos() + z * h.sin(), -x * h.sin() + z * h.cos());
        let (y1, z2) = (y * v.cos() - z1 * v.sin(), y * v.sin() + z1 * v.cos());
        let k = 4.0 / (4.0 + z2);
        pos2(c.x + x1 * s * k, c.y + y1 * s * k)
    };
    let front = [proj(-1.0, -1.0, -0.6), proj(1.0, -1.0, -0.6), proj(1.0, 1.0, -0.6), proj(-1.0, 1.0, -0.6)];
    let back = [proj(-0.6, -0.6, 0.6), proj(0.6, -0.6, 0.6), proj(0.6, 0.6, 0.6), proj(-0.6, 0.6, 0.6)];
    let p = ui.painter();
    p.add(egui::Shape::convex_polygon(front.to_vec(), Color32::from_rgb(0x2a, 0x2a, 0x44), Stroke::new(1.0, Color32::BLACK)));
    p.add(egui::Shape::closed_line(back.to_vec(), Stroke::new(1.0, Color32::BLACK)));
    for k in 0..4 {
        p.line_segment([front[k], back[k]], Stroke::new(1.0, Color32::BLACK));
    }
}

/// Replace Colors: the hue wheel with the range of the original color.
fn wheel(cx: &mut Cx, ui: &mut Ui) {
    text(ui, "Range");
    let w = ui.available_width();
    let (outer, _) = ui.allocate_exact_size(vec2(w, 160.0), Sense::hover());
    let c = outer.center();
    let p = ui.painter();
    let set = match cx.e.params.get("ColorItem") {
        Some(FxValue::Color(col)) => Some(bitfx::rgb_to_hsl(bitfx::col(*col)).0),
        _ => None,
    };
    let smooth = cx.e.num("Smoothness", 50.0) as f32;
    for k in 0..72 {
        let a0 = (k as f32 * 5.0).to_radians();
        let a1 = ((k + 1) as f32 * 5.0).to_radians();
        let hue = k as f32 * 5.0;
        let rgb = bitfx::hsl_to_rgb(hue, 0.8, 0.5);
        let inside = set.map(|h| ((hue - h + 180.0).rem_euclid(360.0) - 180.0).abs() < 15.0 + smooth * 0.6).unwrap_or(false);
        let col = Color32::from_rgb(rgb[0] as u8, rgb[1] as u8, rgb[2] as u8).gamma_multiply(if set.is_none() { 0.12 } else if inside { 1.0 } else { 0.35 });
        let (r0, r1) = (55.0, 72.0);
        let pts = vec![c + vec2(a0.cos(), -a0.sin()) * r0, c + vec2(a0.cos(), -a0.sin()) * r1, c + vec2(a1.cos(), -a1.sin()) * r1, c + vec2(a1.cos(), -a1.sin()) * r0];
        p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
    }
    p.circle_stroke(c, 55.0, Stroke::new(1.0, Color32::from_gray(0x40)));
}

/// Frame: the frame chosen from the list, drawn.
fn frames(cx: &mut Cx, ui: &mut Ui) {
    let cur = cx.e.num("CurrentFrameImage", 0.0).round() as usize;
    let mut e = cx.e.clone();
    e.set("ColorItem", FxValue::Color(Color::BLACK));
    e.set("Opacity", FxValue::Num(100.0));
    let tex = texture(ui, hash_of(&e, "frame"), || {
        let mut pm = tiny_skia::Pixmap::new(110, 90).expect("frame preview");
        pm.fill(tiny_skia::Color::WHITE);
        let cxx = bitfx::FxCtx { s: 0.6, frame: [0.0, 0.0, 110.0, 90.0], seed: 3 };
        bitfx::apply(&e, &mut pm, &cxx);
        to_image(&pm)
    });
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(118.0, 96.0), Sense::hover());
        ui.painter().rect_filled(r, 2.0, Color32::from_rgb(0x33, 0x33, 0x33));
        ui.painter().image(tex.id(), r.shrink(4.0), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    });
    ui.add_space(4.0);
    super::table_header(ui, &[("View", 34.0), ("Select Frame", ui.available_width() - 34.0)]);
    for (k, name) in vector_render::bitfx_art::FRAMES.iter().enumerate() {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
        if k == cur {
            ui.painter().rect_filled(r, 0.0, theme::ACCENT);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 0.0, theme::HOVER);
        }
        let b = Rect::from_center_size(pos2(r.left() + 12.0, r.center().y), vec2(11.0, 11.0));
        ui.painter().rect_stroke(b, 1.0, Stroke::new(1.0, theme::TEXT_DIM), egui::StrokeKind::Inside);
        if k == cur {
            ui.painter().line_segment([b.left_center() + vec2(2.0, 0.0), b.center_bottom() - vec2(0.0, 2.5)], Stroke::new(1.5, theme::TEXT));
            ui.painter().line_segment([b.center_bottom() - vec2(0.0, 2.5), b.right_top() + vec2(-2.0, 2.0)], Stroke::new(1.5, theme::TEXT));
        }
        ui.painter().text(pos2(r.left() + 40.0, r.center().y), egui::Align2::LEFT_CENTER, *name, egui::FontId::proportional(12.0), theme::TEXT);
        if resp.clicked() {
            cx.set("CurrentFrameImage", FxValue::Num(k as f64));
        }
    }
    ui.add_space(6.0);
}
