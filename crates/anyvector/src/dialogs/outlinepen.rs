//! Outline Pen (F12, the Outline Pen flyout, a double-click on the status bar's outline well):
//! every property of an outline in one dialog. With objects selected it changes their outline
//! (one undo step, only what was changed in the dialog); with nothing selected it sets the
//! outline of new objects, which is kept between sessions (#103).

use super::{group, ok_cancel_row, window};
use crate::app::AnyVectorApp;
use crate::dockers::props;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use vector_core::style::{ArrowHead, LineCap, LineJoin, Outline, OutlinePosition};
use vector_core::{Color, ObjKind, Object, Unit};

pub struct OutlinePenState {
    /// The outline being edited.
    pub ol: Outline,
    /// The outline the dialog opened with: OK changes only what differs from it.
    pub orig: Outline,
    pub overprint: bool,
    orig_overprint: bool,
    /// The unit of the width (OK makes it the outline unit, `Settings::outline_unit`).
    pub unit: Unit,
    /// Editing the selection's outline (else the outline of new objects).
    pub selection: bool,
}

impl OutlinePenState {
    /// The dialog on the first selected object's outline, or on the outline of new objects.
    pub fn new(app: &AnyVectorApp) -> OutlinePenState {
        let sel = app.doc().and_then(|d| d.selection.first().and_then(|id| d.doc.find(*id)).map(first_leaf));
        let (ol, overprint) = match sel {
            Some(o) => (o.style.outline.clone(), o.style.overprint_outline),
            None => (app.ts.default_outline.clone(), false),
        };
        OutlinePenState { orig: ol.clone(), ol, overprint, orig_overprint: overprint, unit: app.settings.outline_unit, selection: sel.is_some() }
    }
}

/// The first object of `o` that draws an outline (the first leaf of a group).
fn first_leaf(o: &Object) -> &Object {
    match &o.kind {
        ObjKind::Group(c) if !c.is_empty() => first_leaf(&c[0]),
        _ => o,
    }
}

/// Copies onto `to` the outline properties that differ between `orig` and `new`.
pub fn apply_changes(to: &mut Outline, orig: &Outline, new: &Outline) {
    macro_rules! changed {
        ($($f:ident),*) => {
            $(if orig.$f != new.$f {
                to.$f = new.$f.clone();
            })*
        };
    }
    changed!(width, color, join, cap, dash, miter_limit, position, behind_fill, scale_with_object, start_arrow, end_arrow, dash_mode, stretch, nib_angle, share_arrows);
    // a width given to an outline without a color makes it visible
    if orig.width != new.width && to.color.is_none() {
        to.color = new.color.or(Some(Color::BLACK));
    }
}

/// A row: a label `w` wide, then the controls.
fn labeled<R>(ui: &mut Ui, label: &str, w: f32, body: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.add_sized(vec2(w, 22.0), egui::Label::new(label).halign(egui::Align::Min));
        body(ui)
    })
    .inner
}

/// Three icon toggles (corners, caps, position); the one picked.
fn toggles<T: Copy + PartialEq>(app: &mut AnyVectorApp, ui: &mut Ui, cur: T, items: &[(&str, &str, T)]) -> Option<T> {
    let mut out = None;
    for (icon, tip, v) in items {
        if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, cur == *v).clicked() {
            out = Some(*v);
        }
    }
    out
}

/// The list of line styles (dashes), `w` wide.
fn style_list(app: &AnyVectorApp, ui: &mut Ui, dash: &mut Vec<f64>, w: f32) {
    let styles = props::line_styles(app);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
    ui.painter().rect_filled(r, 2.0, theme::FIELD());
    ui.painter().rect_stroke(r, 2.0, Stroke::new(1.0, theme::SEP()), egui::StrokeKind::Inside);
    props::dash_line(ui.painter(), Rect::from_min_max(r.min + vec2(6.0, 0.0), r.max - vec2(22.0, 0.0)), dash, theme::TEXT());
    crate::widgets::chevron(ui.painter(), pos2(r.right() - 9.0, r.center().y), 3.0, theme::TEXT());
    let resp = resp.on_hover_text("Line style\nChoose a style for the line or outline.");
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for d in &styles {
                let (rr, rs) = ui.allocate_exact_size(vec2(w - 6.0, 16.0), Sense::click());
                if *d == *dash || rs.hovered() {
                    ui.painter().rect_filled(rr, 0.0, if *d == *dash { theme::ACCENT() } else { theme::HOVER() });
                }
                props::dash_line(ui.painter(), rr.shrink2(vec2(6.0, 0.0)), d, theme::TEXT());
                if rs.clicked() {
                    *dash = d.clone();
                    ui.close();
                }
            }
        });
    });
}

/// An arrowhead list (`start`: the start of the line); the arrowhead picked.
fn arrow_list(app: &AnyVectorApp, ui: &mut Ui, cur: Option<ArrowHead>, start: bool) -> Option<Option<ArrowHead>> {
    let (r, resp) = ui.allocate_exact_size(vec2(84.0, 30.0), Sense::click());
    ui.painter().rect_filled(r, 2.0, theme::FIELD());
    ui.painter().rect_stroke(r, 2.0, Stroke::new(1.0, theme::SEP()), egui::StrokeKind::Inside);
    props::paint_arrow(ui.painter(), Rect::from_min_max(r.min + vec2(6.0, 0.0), r.max - vec2(20.0, 0.0)), cur, start);
    crate::widgets::chevron(ui.painter(), pos2(r.right() - 9.0, r.center().y), 3.0, theme::TEXT());
    let resp = resp.on_hover_text(if start { "Start arrowhead" } else { "End arrowhead" });
    let choices = crate::dialogs::object::arrow_choices(app);
    let mut pick = None;
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        let all: Vec<(Option<ArrowHead>, String)> = std::iter::once((None, "No arrowhead".to_string())).chain(choices.iter().map(|c| (Some(c.0), c.1.clone()))).collect();
        egui::Grid::new(("op_arrows", start)).spacing(vec2(2.0, 2.0)).show(ui, |ui| {
            for (i, (a, name)) in all.iter().enumerate() {
                let (rr, rs) = ui.allocate_exact_size(vec2(34.0, 24.0), Sense::click());
                if *a == cur || rs.hovered() {
                    ui.painter().rect_filled(rr, 2.0, if *a == cur { theme::ACCENT() } else { theme::HOVER() });
                }
                props::paint_arrow(ui.painter(), rr.shrink(4.0), *a, start);
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
    pick
}

/// The calligraphic nib: its stretch and angle; dragging in it turns it.
fn nib(ui: &mut Ui, ol: &mut Outline) {
    let (r, resp) = ui.allocate_exact_size(vec2(72.0, 72.0), Sense::click_and_drag());
    ui.painter().rect_filled(r, 0.0, Color32::WHITE);
    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, theme::SEP()), egui::StrokeKind::Outside);
    let n = Rect::from_center_size(r.center(), vec2(48.0, (48.0 * (ol.stretch / 100.0) as f32).max(1.5)));
    let rot = egui::emath::Rot2::from_angle(-(ol.nib_angle as f32).to_radians());
    let pts: Vec<egui::Pos2> = [n.left_top(), n.right_top(), n.right_bottom(), n.left_bottom()].iter().map(|p| r.center() + rot * (*p - r.center())).collect();
    ui.painter().add(egui::Shape::convex_polygon(pts, Color32::from_gray(0x22), Stroke::NONE));
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
        let d = p - r.center();
        ol.nib_angle = ((-d.y).atan2(d.x).to_degrees() as f64).round();
    }
}

pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.outline_pen.take() else { return };
    let dpi = app.doc().map(|d| d.doc.settings.dpi).unwrap_or(300.0);
    let lw = 74.0;
    let ((ok, cancel), dismissed) = window(ctx, "outline_pen", "Outline Pen", vec2(560.0, 476.0), |ui| {
        ui.add_space(8.0);
        if !st.selection {
            super::note(ui, "Nothing is selected: these settings are the outline of new objects.");
            ui.add_space(4.0);
        }
        ui.horizontal_top(|ui| {
            // left: color, width, style, corners, caps, position
            ui.allocate_ui_with_layout(vec2(270.0, 380.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                labeled(ui, "Color:", lw, |ui| {
                    let mut c = st.ol.color;
                    if props::color_pick_tabs(ui, "op_color", &mut c, 84.0) {
                        st.ol.color = c;
                    }
                });
                ui.label("Width:");
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let w = st.ol.is_visible().then_some(st.ol.width);
                    if let Some(p) = props::width_field(ui, "op_width", w, st.unit, dpi, 96.0) {
                        match p {
                            None => st.ol.color = None,
                            Some(w) => {
                                st.ol.width = w;
                                st.ol.color.get_or_insert(Color::BLACK);
                            }
                        }
                    }
                    ui.add_space(8.0);
                    props::unit_choice(ui, "op_unit", &mut st.unit, 110.0);
                });
                ui.label("Style:");
                style_list(app, ui, &mut st.ol.dash, 236.0);
                ui.add_space(6.0);
                labeled(ui, "Corners:", lw, |ui| {
                    if let Some(j) = toggles(app, ui, st.ol.join, &[("join_miter", "Miter corners", LineJoin::Miter), ("join_round", "Round corners", LineJoin::Round), ("join_bevel", "Bevel corners", LineJoin::Bevel)]) {
                        st.ol.join = j;
                    }
                });
                labeled(ui, "Miter limit:", lw, |ui| {
                    let mut ml = props::miter_degrees(st.ol.miter_limit);
                    if crate::dockers::spin(ui, &mut ml, 1.0..=180.0, 1.0, 1, "°", 90.0) {
                        st.ol.miter_limit = props::miter_from_degrees(ml);
                    }
                });
                labeled(ui, "Line caps:", lw, |ui| {
                    if let Some(c) = toggles(app, ui, st.ol.cap, &[("cap_butt", "Square cap", LineCap::Butt), ("cap_round", "Round cap", LineCap::Round), ("cap_square", "Extended square cap", LineCap::Square)]) {
                        st.ol.cap = c;
                    }
                });
                labeled(ui, "Position:", lw, |ui| {
                    if let Some(p) = toggles(app, ui, st.ol.position, &[("pos_outside", "Outside outline", OutlinePosition::Outside), ("pos_center", "Centered outline", OutlinePosition::Center), ("pos_inside", "Inside outline", OutlinePosition::Inside)]) {
                        st.ol.position = p;
                    }
                });
                ui.add_space(4.0);
                ui.add_enabled(st.selection, egui::Checkbox::new(&mut st.overprint, "Overprint outline"));
            });
            ui.add_space(12.0);
            // right: arrowheads, calligraphy, behind fill, scale
            ui.allocate_ui_with_layout(vec2(250.0, 380.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                group(ui, "Arrows", |ui| {
                    ui.horizontal(|ui| {
                        if let Some(a) = arrow_list(app, ui, st.ol.start_arrow, true) {
                            st.ol.start_arrow = a;
                            if st.ol.share_arrows {
                                st.ol.end_arrow = a;
                            }
                        }
                        ui.add_space(6.0);
                        if let Some(a) = arrow_list(app, ui, st.ol.end_arrow, false) {
                            st.ol.end_arrow = a;
                            if st.ol.share_arrows {
                                st.ol.start_arrow = a;
                            }
                        }
                    });
                    if ui.checkbox(&mut st.ol.share_arrows, "Share attributes").changed() && st.ol.share_arrows {
                        st.ol.end_arrow = st.ol.start_arrow;
                    }
                });
                group(ui, "Calligraphy", |ui| {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.label("Stretch:");
                            crate::dockers::spin(ui, &mut st.ol.stretch, 1.0..=100.0, 1.0, 0, " %", 90.0);
                            ui.label("Angle:");
                            crate::dockers::spin(ui, &mut st.ol.nib_angle, -360.0..=360.0, 1.0, 1, "°", 90.0);
                        });
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.label("Nib shape:");
                            nib(ui, &mut st.ol);
                            if ui.button("Default").clicked() {
                                st.ol.stretch = 100.0;
                                st.ol.nib_angle = 0.0;
                            }
                        });
                    });
                });
                ui.add_space(6.0);
                ui.checkbox(&mut st.ol.behind_fill, "Behind fill");
                ui.checkbox(&mut st.ol.scale_with_object, "Scale with object");
            });
        });
        ok_cancel_row(ui, "OK", true)
    });
    // Enter is OK (a width typed before it is taken first)
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter)) && !egui::Popup::is_any_open(ctx);
    if ok || enter {
        apply(app, &st);
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.outline_pen = Some(st);
    }
}

/// OK: the selection's outlines get what was changed (one undo step); with nothing selected the
/// outline becomes the outline of new objects. The unit chosen becomes the outline unit.
pub fn apply(app: &mut AnyVectorApp, st: &OutlinePenState) {
    app.settings.outline_unit = st.unit;
    if !st.selection {
        app.set_default_outline(st.ol.clone());
        return;
    }
    if st.ol == st.orig && st.overprint == st.orig_overprint {
        return;
    }
    for a in [st.ol.start_arrow, st.ol.end_arrow] {
        crate::dialogs::object::ensure_arrow(app, a);
    }
    let Some(d) = app.doc_mut() else { return };
    let ids = d.selection.clone();
    let overprint = (st.overprint != st.orig_overprint).then_some(st.overprint);
    d.modify("Outline Pen", |doc, _| {
        for id in &ids {
            if let Some(o) = doc.find_mut(*id) {
                o.walk_mut(&mut |x| {
                    if x.is_container() && !matches!(x.kind, ObjKind::PowerClip { .. }) {
                        return;
                    }
                    apply_changes(&mut x.style.outline, &st.orig, &st.ol);
                    if let Some(v) = overprint {
                        x.style.overprint_outline = v;
                    }
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_changed_properties_are_applied() {
        let orig = Outline::default();
        let new = Outline { width: 0.0, join: LineJoin::Round, ..orig.clone() };
        // another object keeps its own color, caps and dashes
        let mut other = Outline { color: Some(Color::rgb(200, 0, 0)), cap: LineCap::Round, dash: vec![2.0, 2.0], width: 4.0, ..Outline::default() };
        apply_changes(&mut other, &orig, &new);
        assert_eq!(other.width, 0.0);
        assert_eq!(other.join, LineJoin::Round);
        assert_eq!(other.color, Some(Color::rgb(200, 0, 0)));
        assert_eq!(other.cap, LineCap::Round);
        assert_eq!(other.dash, vec![2.0, 2.0]);
        // a width on an object without an outline gives it one
        let mut none = Outline::none();
        apply_changes(&mut none, &orig, &Outline { width: 2.0, ..orig.clone() });
        assert!(none.is_visible() && none.width == 2.0);
        // nothing changed: nothing happens
        let mut same = other.clone();
        apply_changes(&mut same, &orig, &orig);
        assert_eq!(same, other);
    }
}
