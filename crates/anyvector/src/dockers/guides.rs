//! The Live Guides docker (alignment guides, margins, dynamic guides) and the Guidelines docker.

use super::{no_doc, spin};
use crate::app::{Toolbar, AnyVectorApp};
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use kurbo::Point;
use vector_core::{Color, Guide, GuideKind};

/// The line styles of guides: solid, dashed, dotted, long dashes.
pub const LINE_STYLES: usize = 4;

/// A line in one of the guide line styles.
pub fn styled_line(p: &egui::Painter, a: egui::Pos2, b: egui::Pos2, stroke: Stroke, style: usize) {
    match style {
        1 => {
            p.extend(egui::Shape::dashed_line(&[a, b], stroke, 4.0, 3.0));
        }
        2 => {
            p.extend(egui::Shape::dotted_line(&[a, b], stroke.color, 3.0, stroke.width * 0.6));
        }
        3 => {
            p.extend(egui::Shape::dashed_line(&[a, b], stroke, 9.0, 4.0));
        }
        _ => {
            p.line_segment([a, b], stroke);
        }
    }
}

fn c32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// The line style list: a white field with the line drawn in it.
fn style_combo(ui: &mut Ui, id: &str, style: &mut usize, color: Color32, w: f32, enabled: bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    let field = Rect::from_min_max(r.min, pos2(r.right() - 16.0, r.bottom()));
    p.rect_filled(field.shrink(2.0), 1.0, if enabled { Color32::WHITE } else { Color32::from_gray(150) });
    styled_line(p, pos2(field.left() + 5.0, field.center().y), pos2(field.right() - 5.0, field.center().y), Stroke::new(1.0, color), *style);
    crate::widgets::chevron(p, pos2(r.right() - 8.0, r.center().y), 3.0, if enabled { theme::TEXT() } else { theme::DISABLED() });
    egui::Popup::menu(&resp).id(egui::Id::unique(("lg_style", id))).show(|ui| {
        for s in 0..LINE_STYLES {
            let (lr, lresp) = ui.allocate_exact_size(vec2(w, 20.0), Sense::click());
            ui.painter().rect_filled(lr.shrink(1.0), 0.0, if s == *style { theme::ACCENT() } else { theme::STRONG() });
            styled_line(ui.painter(), pos2(lr.left() + 6.0, lr.center().y), pos2(lr.right() - 6.0, lr.center().y), Stroke::new(1.0, Color32::from_gray(40)), s);
            if lresp.clicked() {
                *style = s;
                ui.close();
            }
        }
    });
}

/// The color list: a swatch that opens a color picker.
fn color_combo(ui: &mut Ui, id: &str, color: &mut [u8; 3], enabled: bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(40.0, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    p.rect_filled(Rect::from_min_size(r.min + vec2(2.0, 4.0), vec2(22.0, 14.0)), 0.0, if enabled { c32(*color) } else { c32(*color).gamma_multiply(0.5) });
    crate::widgets::chevron(p, pos2(r.right() - 8.0, r.center().y), 3.0, if enabled { theme::TEXT() } else { theme::DISABLED() });
    egui::Popup::menu(&resp).id(egui::Id::unique(("lg_color", id))).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        let mut c = Color::rgb(color[0], color[1], color[2]);
        if super::props::color_edit(ui, &mut c, false) {
            *color = [c.r, c.g, c.b];
        }
    });
}

/// A collapsible section header: an arrow, the bold title and a check box at the right.
fn header(ui: &mut Ui, title: &str, open: &mut bool, check: &mut bool, scroll_here: bool) {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    if scroll_here {
        resp.scroll_to_me(Some(egui::Align::TOP));
    }
    let p = ui.painter();
    let a = pos2(r.left() + 7.0, r.center().y);
    if *open {
        crate::widgets::chevron_up(p, a, 3.5, theme::TEXT());
    } else {
        crate::widgets::chevron(p, a, 3.5, theme::TEXT());
    }
    p.text(pos2(r.left() + 16.0, r.center().y), egui::Align2::LEFT_CENTER, title, theme::bold(12.5), theme::TEXT());
    let cb = Rect::from_center_size(pos2(r.right() - 10.0, r.center().y), vec2(12.0, 12.0));
    let cr = ui.interact(cb.expand(3.0), ui.scope_id().with(("lg_hdr", title)), Sense::click());
    checkbox_box(ui.painter(), cb, *check, true);
    if cr.clicked() {
        *check = !*check;
    } else if resp.clicked() {
        *open = !*open;
    }
}

fn checkbox_box(p: &egui::Painter, cb: Rect, on: bool, enabled: bool) {
    let col = if enabled { theme::TEXT_DIM() } else { theme::SEP() };
    p.rect_stroke(cb, 1.0, Stroke::new(1.0, col), egui::StrokeKind::Inside);
    if on {
        p.line_segment([cb.left_center() + vec2(2.5, 0.0), cb.center_bottom() - vec2(0.5, 3.0)], Stroke::new(1.5, theme::TEXT()));
        p.line_segment([cb.center_bottom() - vec2(0.5, 3.0), cb.right_top() + vec2(-2.5, 2.5)], Stroke::new(1.5, theme::TEXT()));
    }
}

fn toggle(app: &mut AnyVectorApp, ui: &mut Ui, icon: &str, tip: &str, on: &mut bool, enabled: bool) {
    if crate::widgets::icon_button_sized(&mut app.icons, ui, icon, tip, enabled, *on, 26.0, 18.0).clicked() {
        *on = !*on;
    }
}

fn vbar(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(9.0, 22.0), Sense::hover());
    ui.painter().line_segment([r.center_top(), r.center_bottom()], Stroke::new(1.0, theme::SEP()));
}

fn unit_field(app: &AnyVectorApp, ui: &mut Ui, id: &str, v: &mut f64, w: f32, enabled: bool) -> bool {
    let (u, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    crate::widgets::unit_field(ui, egui::Id::unique(("lg", id)), v, u, dpi, w, enabled)
}

const MARGIN_MODES: [&str; 4] = ["Offset", "Inset", "Offset and inset", "No margin"];

pub fn live_guides(app: &mut AnyVectorApp, ui: &mut Ui) {
    let full_w = ui.available_width();
    let mut scroll_to = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (k, (icon, tip)) in [("lg_align", "Alignment guides"), ("lg_dynamic", "Dynamic guides")].iter().enumerate() {
            if crate::widgets::icon_button_sized(&mut app.icons, ui, icon, tip, true, app.dockers.ui.lg_section == k, 26.0, 18.0).clicked() {
                app.dockers.ui.lg_section = k;
                scroll_to = Some(k);
            }
        }
        ui.add_space((ui.available_width() - 36.0).max(0.0));
        vbar(ui);
        let mut c = app.dockers.ui.lg_collapse;
        toggle(app, ui, "lg_collapse", "Show one section at a time", &mut c, true);
        app.dockers.ui.lg_collapse = c;
    });
    let collapse = app.dockers.ui.lg_collapse;
    if collapse {
        let k = app.dockers.ui.lg_section;
        app.dockers.ui.lg_open = [k == 0, k == 1];
    }
    // ------------------------------------------------------------------ alignment guides
    let mut open = app.dockers.ui.lg_open;
    let mut on = app.settings.live_guides;
    header(ui, "Alignment guides", &mut open[0], &mut on, scroll_to == Some(0));
    app.settings.live_guides = on;
    if open[0] {
        let s = &mut app.settings;
        ui.horizontal(|ui| {
            style_combo(ui, "align", &mut s.align_guide_style, c32(s.align_guide_color), full_w - 48.0, true);
            color_combo(ui, "align", &mut s.align_guide_color, true);
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let mut v = [app.settings.live_guides_edges, app.settings.live_guides_centers, app.settings.live_guides_groups, app.settings.intelligent_spacing, app.settings.intelligent_dimensioning];
            for (k, (icon, tip)) in [("snap_edges", "Object edges"), ("snap_centers", "Object centers"), ("snap_group_children", "Individual objects in a group"), ("intelligent_spacing", "Intelligent spacing"), ("intelligent_dimensioning", "Intelligent dimensioning")].iter().enumerate() {
                toggle(app, ui, icon, tip, &mut v[k], on);
            }
            [app.settings.live_guides_edges, app.settings.live_guides_centers, app.settings.live_guides_groups, app.settings.intelligent_spacing, app.settings.intelligent_dimensioning] = v;
            ui.add_space((ui.available_width() - 36.0).max(0.0));
            vbar(ui);
            let mut shown = app.settings.extra_toolbars.iter().any(|t| t == Toolbar::Layout.name());
            if crate::widgets::icon_button_sized(&mut app.icons, ui, "layout_toolbar", "Layout toolbar", true, shown, 26.0, 18.0).clicked() {
                shown = !shown;
                app.settings.extra_toolbars.retain(|t| t != Toolbar::Layout.name());
                if shown {
                    app.settings.extra_toolbars.push(Toolbar::Layout.name().to_string());
                }
            }
        });
        // margins
        let mut mopen = app.dockers.ui.lg_margins_open;
        let (r, resp) = ui.allocate_exact_size(vec2(full_w, 12.0), Sense::click());
        let a = r.center();
        if mopen {
            crate::widgets::chevron_up(ui.painter(), a, 3.0, theme::TEXT_DIM());
        } else {
            crate::widgets::chevron(ui.painter(), a, 3.0, theme::TEXT_DIM());
        }
        if resp.clicked() {
            mopen = !mopen;
        }
        app.dockers.ui.lg_margins_open = mopen;
        if mopen {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 4)).show(ui, |ui| {
                ui.set_min_width(full_w);
                let mut m = app.settings.guide_margins;
                ui.add_enabled_ui(on, |ui| ui.checkbox(&mut m, "Margins"));
                app.settings.guide_margins = m;
                let en = on && m;
                let s = &mut app.settings;
                ui.horizontal(|ui| {
                    style_combo(ui, "margin", &mut s.guide_margin_style, c32(s.guide_margin_color), full_w - 48.0, true);
                    color_combo(ui, "margin", &mut s.guide_margin_color, true);
                });
                ui.horizontal(|ui| {
                    ui.add_space((full_w - 200.0).max(0.0));
                    ui.vertical(|ui| {
                        for k in 0..2 {
                            ui.horizontal(|ui| {
                                let icon = if k == 0 { "margin_h" } else { "margin_v" };
                                let tip = if k == 0 { "Horizontal margin" } else { "Vertical margin" };
                                let mode = app.settings.guide_margin_mode[k];
                                let r = crate::widgets::icon_button_sized(&mut app.icons, ui, icon, &format!("{tip}: {}", MARGIN_MODES[mode]), en, false, 26.0, 18.0);
                                egui::Popup::menu(&r).show(|ui| {
                                    for (i, n) in MARGIN_MODES.iter().enumerate() {
                                        if ui.add(egui::Button::new(*n).selected(i == mode)).clicked() {
                                            app.settings.guide_margin_mode[k] = i;
                                            ui.close();
                                        }
                                    }
                                });
                                let mut v = app.settings.guide_margin[k];
                                if unit_field(app, ui, &format!("m{k}"), &mut v, 70.0, en && mode != 3) {
                                    app.settings.guide_margin[k] = v;
                                    if app.settings.guide_margins_locked {
                                        app.settings.guide_margin[1 - k] = v;
                                    }
                                }
                            });
                        }
                    });
                    // the lock joins the two margins
                    let (lr, lresp) = ui.allocate_exact_size(vec2(18.0, 50.0), if en { Sense::click() } else { Sense::hover() });
                    let p = ui.painter();
                    let col = if en { theme::TEXT() } else { theme::DISABLED() };
                    p.line_segment([lr.left_top() + vec2(4.0, 6.0), lr.center_top() + vec2(4.0, 6.0)], Stroke::new(1.0, col));
                    p.line_segment([lr.center_top() + vec2(4.0, 6.0), lr.center() + vec2(4.0, -9.0)], Stroke::new(1.0, col));
                    p.line_segment([lr.left_bottom() + vec2(4.0, -6.0), lr.center_bottom() + vec2(4.0, -6.0)], Stroke::new(1.0, col));
                    p.line_segment([lr.center_bottom() + vec2(4.0, -6.0), lr.center() + vec2(4.0, 9.0)], Stroke::new(1.0, col));
                    let icon = if app.settings.guide_margins_locked { "lock" } else { "unlock" };
                    if let Some(t) = app.icons.get(ui.ctx(), icon, 14.0, if en { crate::icons::Tone::Normal } else { crate::icons::Tone::Disabled }) {
                        ui.painter().image(t.id, Rect::from_center_size(lr.center(), vec2(14.0, 14.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    if lresp.on_hover_text("Lock ratio").clicked() {
                        app.settings.guide_margins_locked = !app.settings.guide_margins_locked;
                    }
                });
            });
        }
    }
    ui.add_space(4.0);
    // ------------------------------------------------------------------ dynamic guides
    let mut on = app.settings.dynamic_guides;
    header(ui, "Dynamic guides", &mut open[1], &mut on, scroll_to == Some(1));
    app.settings.dynamic_guides = on;
    if collapse && open != app.dockers.ui.lg_open {
        // opening a section closes the other
        let k = if open[1] && !app.dockers.ui.lg_open[1] { 1 } else { 0 };
        app.dockers.ui.lg_section = k;
        open = [k == 0, k == 1];
    }
    app.dockers.ui.lg_open = open;
    if !open[1] {
        return;
    }
    let s = &mut app.settings;
    ui.horizontal(|ui| {
        style_combo(ui, "dyn", &mut s.dyn_guide_style, c32(s.dyn_guide_color), full_w - 48.0, true);
        color_combo(ui, "dyn", &mut s.dyn_guide_color, true);
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let mut v = [app.settings.dyn_screen_tips, app.settings.dyn_extend_segment, app.settings.dyn_ticks];
        for (k, (icon, tip)) in [("dyn_tips", "Display screen tips"), ("dyn_extend", "Extend along segment"), ("dyn_ticks", "Snap to tick spacing")].iter().enumerate() {
            toggle(app, ui, icon, tip, &mut v[k], on);
        }
        [app.settings.dyn_screen_tips, app.settings.dyn_extend_segment, app.settings.dyn_ticks] = v;
        ui.add_space((ui.available_width() - 130.0).max(0.0));
        vbar(ui);
        let en = on && app.settings.dyn_ticks;
        if let Some(t) = app.icons.get(ui.ctx(), "tick_spacing", 16.0, if en { crate::icons::Tone::Normal } else { crate::icons::Tone::Disabled }) {
            let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
            ui.painter().image(t.id, Rect::from_center_size(r.center(), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let mut t = app.settings.dyn_tick_spacing;
        if unit_field(app, ui, "tick", &mut t, 70.0, en) {
            app.settings.dyn_tick_spacing = t.max(0.01);
        }
    });
    list_and_preview(app, ui, on, full_w);
    // the grip under the list
    let (r, _) = ui.allocate_exact_size(vec2(full_w, 10.0), Sense::hover());
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "···", egui::FontId::proportional(11.0), theme::TEXT_DIM());
    ui.painter().hline(r.left()..=r.right() - 6.0, r.bottom(), Stroke::new(1.0, theme::SEP()));
}

fn list_and_preview(app: &mut AnyVectorApp, ui: &mut Ui, on: bool, full_w: f32) {
    ui.horizontal_top(|ui| {
        // the angles, each with its check box
        let list_h = 205.0;
        let mut toggle_k = None;
        egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP())).fill(theme::FIELD()).show(ui, |ui| {
            ui.set_width(98.0);
            egui::ScrollArea::vertical().id_salt("dyn_angles").max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let sel = app.dockers.ui.lg_angle_sel;
                ui.vertical(|ui| for (k, (a, chk)) in app.settings.dyn_angles.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(vec2(84.0, 25.0), Sense::click());
                    if k == sel {
                        ui.painter().rect_filled(r, 0.0, theme::ACCENT());
                    }
                    let cb = Rect::from_center_size(pos2(r.left() + 8.0, r.center().y), vec2(10.0, 10.0));
                    checkbox_box(ui.painter(), cb, *chk, on);
                    ui.painter().text(pos2(r.left() + 18.0, r.center().y), egui::Align2::LEFT_CENTER, format!("{a} °"), egui::FontId::proportional(11.5), if on { theme::TEXT() } else { theme::DISABLED() });
                    if on && resp.clicked() {
                        if resp.interact_pointer_pos().is_some_and(|q| q.x < cb.right() + 3.0) {
                            toggle_k = Some(k);
                        }
                        app.dockers.ui.lg_angle_sel = k;
                    }
                });
            });
        });
        if let Some(k) = toggle_k {
            app.settings.dyn_angles[k].1 = !app.settings.dyn_angles[k].1;
        }
        ui.vertical(|ui| {
            // the guides of the checked angles through a point
            let (r, _) = ui.allocate_exact_size(vec2((full_w - 118.0).max(60.0), 95.0), Sense::hover());
            let p = ui.painter();
            p.rect_stroke(r, 0.0, Stroke::new(1.0, theme::TEXT_DIM()), egui::StrokeKind::Inside);
            let c = r.center();
            let col = if on { c32(app.settings.dyn_guide_color) } else { theme::DISABLED() };
            for (k, (a, chk)) in app.settings.dyn_angles.iter().enumerate() {
                if on && (*chk || k == app.dockers.ui.lg_angle_sel) {
                    let d = vec2(a.to_radians().cos() as f32, -a.to_radians().sin() as f32) * 200.0;
                    let pp = p.with_clip_rect(r.shrink(1.0));
                    styled_line(&pp, c - d, c + d, Stroke::new(if k == app.dockers.ui.lg_angle_sel { 1.6 } else { 1.0 }, col), app.settings.dyn_guide_style);
                }
            }
            ui.add_space(4.0);
            ui.add_enabled_ui(on, |ui| {
                ui.horizontal(|ui| {
                    if let Some(t) = app.icons.get(ui.ctx(), "rotate_ccw", 16.0, crate::icons::Tone::Normal) {
                        let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                        ui.painter().image(t.id, Rect::from_center_size(r.center(), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    spin(ui, &mut app.dockers.ui.lg_add_angle, -360.0..=360.0, 5.0, 1, "", 70.0);
                    ui.label("°");
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if crate::widgets::icon_button(&mut app.icons, ui, "add_circle", "Add angle", on, false).clicked() {
                        let a = (app.dockers.ui.lg_add_angle * 10.0).round() / 10.0;
                        let a = if a < 0.0 { a + 180.0 } else { a } % 180.0;
                        if !app.settings.dyn_angles.iter().any(|(x, _)| (x - a).abs() < 1e-6) {
                            app.settings.dyn_angles.push((a, true));
                            app.settings.dyn_angles.sort_by(|x, y| x.0.total_cmp(&y.0));
                        }
                    }
                    let sel = app.dockers.ui.lg_angle_sel;
                    if crate::widgets::icon_button(&mut app.icons, ui, "remove_circle", "Delete angle", on && sel < app.settings.dyn_angles.len(), false).clicked() {
                        app.settings.dyn_angles.remove(sel);
                        app.dockers.ui.lg_angle_sel = sel.min(app.settings.dyn_angles.len().saturating_sub(1));
                    }
                    if crate::widgets::icon_button(&mut app.icons, ui, "check_all", "Select all", on, false).clicked() {
                        app.settings.dyn_angles.iter_mut().for_each(|a| a.1 = true);
                    }
                    if crate::widgets::icon_button(&mut app.icons, ui, "uncheck_all", "Deselect all", on, false).clicked() {
                        app.settings.dyn_angles.iter_mut().for_each(|a| a.1 = false);
                    }
                });
            });
        });
    });
}

// ================================================================================== Guidelines

const GUIDE_TYPES: [&str; 3] = ["Horizontal", "Vertical", "Angled"];

fn guide_label(g: &Guide, u: vector_core::Unit, dpi: f64, o: Point) -> String {
    match g.kind {
        GuideKind::Horizontal => format!("{}", u.format(g.pos.y - o.y, dpi)),
        GuideKind::Vertical => format!("{}", u.format(g.pos.x - o.x, dpi)),
        GuideKind::Angled(a) => format!("{}, {}  {a:.1}°", u.format(g.pos.x - o.x, dpi), u.format(g.pos.y - o.y, dpi)),
    }
}

pub fn guidelines(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let full_w = ui.available_width();
    let (show, snap) = app.doc().map(|d| (d.doc.settings.show_guides, d.doc.settings.snap_to_guides)).unwrap_or((true, true));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let (mut sh, mut sn) = (show, snap);
        toggle(app, ui, "guidelines", "Show guidelines", &mut sh, true);
        toggle(app, ui, "snap", "Snap to guidelines", &mut sn, true);
        if (sh, sn) != (show, snap) {
            if let Some(d) = app.doc_mut() {
                d.doc.settings.show_guides = sh;
                d.doc.settings.snap_to_guides = sn;
                d.history.touch();
            }
        }
        ui.add_space((ui.available_width() - 28.0).max(0.0));
        let r = crate::widgets::icon_button(&mut app.icons, ui, "gear", "Options", true, false);
        egui::Popup::menu(&r).show(|ui| {
            ui.checkbox(&mut app.settings.guides_at_scale, "Show at scale");
        });
    });
    let s = &mut app.settings;
    ui.horizontal(|ui| {
        style_combo(ui, "guide", &mut s.guide_line_style, c32(s.guide_color), full_w - 48.0, true);
        color_combo(ui, "guide", &mut s.guide_color, true);
    });
    let mut u = std::mem::take(&mut app.dockers.ui);
    super::choice(ui, "g_type", &mut u.guide_type, &GUIDE_TYPES, full_w - 12.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("x:");
                unit_field(app, ui, "gx", &mut u.guide_xy[0], 76.0, u.guide_type != 0);
            });
            ui.horizontal(|ui| {
                ui.label("y:");
                unit_field(app, ui, "gy", &mut u.guide_xy[1], 76.0, u.guide_type != 1);
            });
        });
        ui.add_enabled_ui(u.guide_type == 2, |ui| {
            if let Some(t) = app.icons.get(ui.ctx(), "rotate_ccw", 16.0, if u.guide_type == 2 { crate::icons::Tone::Normal } else { crate::icons::Tone::Disabled }) {
                let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                ui.painter().image(t.id, Rect::from_center_size(r.center(), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            spin(ui, &mut u.guide_angle, -360.0..=360.0, 1.0, 1, "", 70.0);
        });
    });
    let sel = u.guide_sel;
    let (kind, xy, gcol) = (u.guide_type, u.guide_xy, app.settings.guide_color);
    let angle = u.guide_angle;
    app.dockers.ui = u;
    let (add, modify) = ui
        .columns(2, |c| {
            let a = c[0].add_sized(vec2(c[0].available_width(), 24.0), egui::Button::new("Add")).clicked();
            let m = c[1].add_enabled(sel.is_some(), egui::Button::new("Modify").frame(false).min_size(vec2(c[1].available_width(), 24.0))).clicked();
            (a, m)
        });
    let make = |o: Point| Guide {
        kind: match kind {
            0 => GuideKind::Horizontal,
            1 => GuideKind::Vertical,
            _ => GuideKind::Angled(angle),
        },
        pos: Point::new(xy[0] + o.x, xy[1] + o.y),
        locked: false,
        color: Color::rgb(gcol[0], gcol[1], gcol[2]),
    };
    if add || modify {
        if let Some(d) = app.doc_mut() {
            let pi = d.page;
            d.modify(if add { "Add Guideline" } else { "Modify Guideline" }, |doc, _| {
                let g = make(doc.settings.ruler_origin);
                let guides = &mut doc.pages[pi].guides;
                match (add, sel) {
                    (false, Some(i)) if i < guides.len() => {
                        let locked = guides[i].locked;
                        guides[i] = Guide { locked, ..g };
                    }
                    _ => guides.push(g),
                }
            });
        }
    }
    // the guidelines of the page
    let Some(d) = app.doc() else { return };
    let guides = d.page_ref().guides.clone();
    let (unit, dpi, o) = (d.doc.settings.unit, d.doc.settings.dpi, d.doc.settings.ruler_origin);
    let list_h = (ui.available_height() - 34.0).max(120.0);
    let mut pick = None;
    egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP())).show(ui, |ui| {
        ui.set_min_size(vec2(full_w - 2.0, list_h));
        egui::ScrollArea::vertical().id_salt("guide_list").max_height(list_h).show(ui, |ui| {
            for (i, g) in guides.iter().enumerate() {
                let (r, resp) = ui.allocate_exact_size(vec2(full_w - 10.0, 20.0), Sense::click());
                if Some(i) == sel {
                    ui.painter().rect_filled(r, 0.0, theme::ACCENT());
                }
                let t = match g.kind {
                    GuideKind::Horizontal => "Horizontal",
                    GuideKind::Vertical => "Vertical",
                    GuideKind::Angled(_) => "Angled",
                };
                ui.painter().line_segment([pos2(r.left() + 4.0, r.center().y), pos2(r.left() + 18.0, r.center().y)], Stroke::new(2.0, Color32::from_rgb(g.color.r, g.color.g, g.color.b)));
                ui.painter().text(pos2(r.left() + 24.0, r.center().y), egui::Align2::LEFT_CENTER, format!("{t}: {}", guide_label(g, unit, dpi, o)), egui::FontId::proportional(12.0), theme::TEXT());
                if g.locked {
                    ui.painter().text(pos2(r.right() - 4.0, r.center().y), egui::Align2::RIGHT_CENTER, "🔒", egui::FontId::proportional(10.0), theme::TEXT_DIM());
                }
                if resp.clicked() {
                    pick = Some(i);
                }
            }
        });
    });
    if let Some(i) = pick {
        let g = &guides[i];
        let u = &mut app.dockers.ui;
        u.guide_sel = Some(i);
        u.guide_type = match g.kind {
            GuideKind::Horizontal => 0,
            GuideKind::Vertical => 1,
            GuideKind::Angled(_) => 2,
        };
        if let GuideKind::Angled(a) = g.kind {
            u.guide_angle = a;
        }
        u.guide_xy = [g.pos.x - o.x, g.pos.y - o.y];
    }
    let sel = app.dockers.ui.guide_sel.filter(|i| *i < guides.len());
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - 56.0).max(0.0));
        let locked = sel.is_some_and(|i| guides[i].locked);
        let lock = crate::widgets::icon_button(&mut app.icons, ui, if locked { "unlock" } else { "lock" }, if locked { "Unlock guideline" } else { "Lock guideline" }, sel.is_some(), false).clicked();
        let del = crate::widgets::icon_button(&mut app.icons, ui, "delete", "Delete guideline", sel.is_some(), false).clicked();
        let (Some(i), Some(d)) = (sel, app.doc_mut()) else { return };
        let pi = d.page;
        if lock {
            d.modify(if locked { "Unlock Guideline" } else { "Lock Guideline" }, |doc, _| doc.pages[pi].guides[i].locked = !locked);
        }
        if del {
            d.modify("Delete Guideline", |doc, _| {
                doc.pages[pi].guides.remove(i);
            });
            app.dockers.ui.guide_sel = None;
        }
    });
}
