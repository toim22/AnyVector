//! The drawing window: rulers, the rendered page (cached texture), tool input and overlay,
//! the page navigator at the bottom, the page sorter view and full-screen preview.

use crate::app::VectorApp;
use crate::commands::Cmd;
use crate::docview::ViewMode;
use crate::theme;
use crate::tools::{self, Drag, ToolId};
use egui::{pos2, vec2, Color32, CursorIcon, Id, Pos2, Rect, Sense, Stroke, Ui};
use kurbo::{Affine, Point};
use std::hash::{Hash, Hasher};
use vector_core::Color;

const RULER: f32 = 18.0;
const NAV_H: f32 = 24.0;

pub fn drawing_window(app: &mut VectorApp, ui: &mut Ui) {
    let Some(ai) = app.active else { return };
    if ai >= app.docs.len() {
        return;
    }
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let nav_rect = Rect::from_min_max(pos2(full.min.x, full.max.y - NAV_H), full.max);
    let area = Rect::from_min_max(full.min, pos2(full.max.x, full.max.y - NAV_H));
    page_navigator(app, ui, nav_rect);

    if app.docs[ai].show_page_sorter {
        page_sorter(app, ui, area);
        return;
    }
    let show_rulers = app.docs[ai].doc.settings.show_rulers;
    let r = if show_rulers { RULER } else { 0.0 };
    let canvas = Rect::from_min_max(area.min + vec2(r, r), area.max);
    {
        let d = &mut app.docs[ai];
        if d.canvas_rect != canvas {
            // keep the view anchored at the top-left when the window is resized
            d.canvas_rect = canvas;
        }
        if d.needs_fit && canvas.width() > 40.0 && canvas.height() > 40.0 {
            d.zoom_page();
            d.needs_fit = false;
        }
    }

    // ------------------------------------------------------------------ input
    let ctx = ui.ctx().clone();
    let resp = ui.interact(canvas, Id::unique(("canvas", app.docs[ai].uid)), Sense::click_and_drag());
    let hovered = resp.contains_pointer();
    let (pos, pressed, released, down, sec_pressed, mods, scroll, zoom_delta, time, dt) = ctx.input(|i| {
        (
            i.pointer.latest_pos(),
            i.pointer.primary_pressed(),
            i.pointer.primary_released(),
            i.pointer.primary_down(),
            i.pointer.button_pressed(egui::PointerButton::Secondary),
            i.modifiers,
            i.smooth_scroll_delta,
            i.zoom_delta(),
            i.time,
            i.stable_dt as f64,
        )
    });
    let modal = app.dialogs.any_modal_open();
    if !modal {
        // wheel: zoom or scroll (Ctrl/Alt inverts, as set in Options > Display)
        if hovered {
            let p = pos.unwrap_or(canvas.center());
            let d = &mut app.docs[ai];
            if zoom_delta != 1.0 {
                d.zoom_at(p, zoom_delta as f64);
            }
            if scroll != egui::Vec2::ZERO {
                let zoom_wheel = matches!(app.settings.wheel_action, crate::settings::WheelAction::Zoom) != mods.alt;
                if zoom_wheel && scroll.x == 0.0 {
                    let rate = 1.0 + (app.settings.zoom_rate.max(1.0) / 100.0).min(1.0);
                    d.zoom_at(p, rate.powf(scroll.y as f64 / 120.0));
                } else {
                    d.origin += scroll;
                }
            }
        }
        // middle button pans with every tool
        if resp.dragged_by(egui::PointerButton::Middle) {
            app.docs[ai].origin += resp.drag_delta();
        }
        let inp = tools::Input {
            pos: pos.unwrap_or(canvas.center()),
            hovered,
            pressed: pressed && hovered,
            released,
            down,
            double: resp.double_clicked(),
            secondary: sec_pressed && hovered,
            mods,
            time,
            dt,
        };
        if inp.pressed || hovered || app.ts.press.is_some() || inp.secondary {
            if inp.pressed {
                // clicking the drawing leaves any text field so tool keys and typing reach the canvas
                ctx.memory_mut(|m| {
                    if let Some(f) = m.focused() {
                        m.surrender_focus(f);
                    }
                });
            }
            tools::handle_input(app, &inp);
        }
        if hovered {
            app.cursor_doc = pos.map(|p| app.docs[ai].to_doc(p));
        }
        tools::handle_text_keys(app, &ctx);
    }
    if !app.active.map(|a| a < app.docs.len()).unwrap_or(false) {
        return;
    }
    let ai = app.active.unwrap();

    // ------------------------------------------------------------------ render
    render_cache(app, ui.ctx(), ai, canvas);
    let painter = ui.painter_at(canvas);
    if let Some(tex) = &app.docs[ai].cache.tex {
        painter.image(tex.id(), canvas, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    } else {
        painter.rect_filled(canvas, 0.0, theme::DESKTOP);
    }
    tools::draw_overlay(app, &painter);

    // ------------------------------------------------------------------ rulers
    if show_rulers {
        rulers(app, ui, ai, area, canvas, hovered.then_some(pos).flatten());
    }

    // ------------------------------------------------------------------ cursor
    if hovered && !modal {
        ctx.set_cursor_icon(cursor_for(app, pos.unwrap_or_default()));
    }
    if app.ts.is_busy() || app.docs[ai].text_edit.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// Re-render the page into the cached texture when anything that affects it changed.
fn render_cache(app: &mut VectorApp, ctx: &egui::Context, ai: usize, canvas: Rect) {
    let ppp = ctx.pixels_per_point();
    let pw = (canvas.width() * ppp).round().max(1.0) as u32;
    let ph = (canvas.height() * ppp).round().max(1.0) as u32;
    let settings = &app.settings;
    let ts = &mut app.ts;
    let d = &mut app.docs[ai];
    let mut h = std::collections::hash_map::DefaultHasher::new();
    d.uid.hash(&mut h);
    d.history.revision.hash(&mut h);
    d.zoom.to_bits().hash(&mut h);
    d.origin.x.to_bits().hash(&mut h);
    d.origin.y.to_bits().hash(&mut h);
    (pw, ph).hash(&mut h);
    ppp.to_bits().hash(&mut h);
    (d.view_mode as u8).hash(&mut h);
    d.page.hash(&mut h);
    d.focus.hash(&mut h);
    settings.desktop_color.hash(&mut h);
    settings.inactive_visibility.to_bits().hash(&mut h);
    d.doc.settings.show_page_border.hash(&mut h);
    let key = h.finish() | 1;
    if d.cache.key == key && d.cache.tex.is_some() {
        return;
    }
    let Some(mut pm) = tiny_skia::Pixmap::new(pw, ph) else { return };
    let view = Affine::scale(ppp as f64) * d.view();
    let dc = settings.desktop_color;
    let opts = vector_render::RenderOptions {
        wireframe: d.view_mode == ViewMode::Wireframe,
        draft: false,
        printing: false,
        draw_page: d.doc.settings.show_page_border || true,
        desktop: Color::rgb(dc[0], dc[1], dc[2]),
        hidden: vec![],
        dim_others: d.focus.map(|f| (vec![f], (settings.inactive_visibility / 100.0).clamp(0.0, 1.0) as f32)),
        show_powerclip_frames: settings.powerclip_empty_lines,
        antialias: d.view_mode != ViewMode::Pixels,
    };
    if d.view_mode == ViewMode::Pixels {
        // render at the document resolution, then show the pixels enlarged
        let dpi = d.doc.settings.dpi.max(1.0);
        let k = (d.zoom * ppp as f64) / (dpi / 72.0);
        if k > 1.0 {
            let sw = ((pw as f64) / k).ceil().max(1.0) as u32;
            let sh = ((ph as f64) / k).ceil().max(1.0) as u32;
            if let Some(mut small) = tiny_skia::Pixmap::new(sw, sh) {
                let sv = Affine::scale(1.0 / k) * view;
                ts.renderer.render_page(&d.doc, d.page, sv, &mut small, &opts);
                let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Nearest, ..Default::default() };
                pm.draw_pixmap(0, 0, small.as_ref(), &paint, tiny_skia::Transform::from_scale(k as f32, k as f32), None);
            }
        } else {
            ts.renderer.render_page(&d.doc, d.page, view, &mut pm, &opts);
        }
    } else {
        ts.renderer.render_page(&d.doc, d.page, view, &mut pm, &opts);
    }
    let img = egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], pm.data());
    match &mut d.cache.tex {
        Some(t) => t.set(img, egui::TextureOptions::NEAREST),
        None => d.cache.tex = Some(ctx.load_texture(format!("canvas{}", d.uid), img, egui::TextureOptions::NEAREST)),
    }
    d.cache.key = key;
}

fn cursor_for(app: &VectorApp, pos: Pos2) -> CursorIcon {
    if matches!(app.ts.drag, Drag::Pan { .. }) {
        return CursorIcon::Grabbing;
    }
    if app.ts.pick_target.is_some() {
        return CursorIcon::PointingHand;
    }
    let Some(d) = app.doc() else { return CursorIcon::Default };
    if app.settings.crosshair_cursor && app.tool != ToolId::Pick {
        return CursorIcon::Crosshair;
    }
    match app.tool {
        ToolId::Pick | ToolId::FreeTransform => {
            if let Drag::Transform { kind, .. } = &app.ts.drag {
                return match kind {
                    tools::Xform::Move => CursorIcon::Move,
                    tools::Xform::Rotate => CursorIcon::Alias,
                    _ => CursorIcon::Crosshair,
                };
            }
            if tools::guide_at(d, pos).is_some() {
                return CursorIcon::Grab;
            }
            if let Some(bb) = d.selection_bounds() {
                let e = d.doc_len(6.0);
                let eb = bb.inflate(e, e);
                for i in 0..8 {
                    if (d.to_screen(tools::handle_point(eb, i)) - pos).length() <= 6.0 {
                        if d.rotate_mode || app.tool == ToolId::FreeTransform {
                            return CursorIcon::Alias;
                        }
                        return match i {
                            0 | 4 => CursorIcon::ResizeNwSe,
                            2 | 6 => CursorIcon::ResizeNeSw,
                            1 | 5 => CursorIcon::ResizeVertical,
                            _ => CursorIcon::ResizeHorizontal,
                        };
                    }
                }
                if bb.contains(d.to_doc(pos)) {
                    return CursorIcon::Move;
                }
            }
            CursorIcon::Default
        }
        ToolId::Pan => CursorIcon::Grab,
        ToolId::Zoom => {
            if app.ts.mods.shift {
                CursorIcon::ZoomOut
            } else {
                CursorIcon::ZoomIn
            }
        }
        ToolId::Text => CursorIcon::Text,
        ToolId::Shape => CursorIcon::Default,
        ToolId::ColorEyedropper | ToolId::AttributesEyedropper | ToolId::SmartFill | ToolId::MeshFill => CursorIcon::Cell,
        ToolId::Blend | ToolId::Envelope | ToolId::EditAnchor => CursorIcon::PointingHand,
        _ => CursorIcon::Crosshair,
    }
}

// ---------------------------------------------------------------------------------- rulers

fn rulers(app: &mut VectorApp, ui: &mut Ui, ai: usize, area: Rect, canvas: Rect, hover: Option<Pos2>) {
    let top = Rect::from_min_max(pos2(canvas.min.x, area.min.y), pos2(area.max.x, canvas.min.y));
    let left = Rect::from_min_max(pos2(area.min.x, canvas.min.y), pos2(canvas.min.x, area.max.y));
    let corner = Rect::from_min_max(area.min, canvas.min);
    let painter = ui.painter();
    let d = &app.docs[ai];
    let unit = d.doc.settings.unit;
    let dpi = d.doc.settings.dpi;
    let origin = d.doc.settings.ruler_origin;
    let ppu = unit.points_per_unit(dpi);
    let (step, subdiv) = unit.ruler_step(d.zoom, dpi);
    let tick = Color32::from_gray(0x8c);
    let label_col = Color32::from_gray(0xb0);
    let font = egui::FontId::proportional(9.0);
    painter.rect_filled(top, 0.0, theme::RULER);
    painter.rect_filled(left, 0.0, theme::RULER);
    painter.rect_filled(corner, 0.0, theme::RULER);
    painter.line_segment([pos2(top.min.x, top.max.y - 0.5), pos2(top.max.x, top.max.y - 0.5)], Stroke::new(1.0, theme::SEP));
    painter.line_segment([pos2(left.max.x - 0.5, left.min.y), pos2(left.max.x - 0.5, left.max.y)], Stroke::new(1.0, theme::SEP));
    painter.line_segment([pos2(corner.max.x - 0.5, corner.min.y), pos2(corner.max.x - 0.5, corner.max.y)], Stroke::new(1.0, theme::SEP));
    painter.line_segment([pos2(corner.min.x, corner.max.y - 0.5), pos2(corner.max.x, corner.max.y - 0.5)], Stroke::new(1.0, theme::SEP));
    // corner: crosshair glyph (drag to move the ruler origin)
    let cc = corner.center();
    painter.line_segment([cc - vec2(5.0, 0.0), cc + vec2(5.0, 0.0)], Stroke::new(1.0, tick));
    painter.line_segment([cc - vec2(0.0, 5.0), cc + vec2(0.0, 5.0)], Stroke::new(1.0, tick));
    painter.circle_stroke(cc, 3.0, Stroke::new(1.0, tick));

    let step_pt = step * ppu;
    let sub_pt = step_pt / subdiv as f64;
    let decimals = if step < 1.0 { ((-step.log10()).ceil() as usize).min(3) } else { 0 };
    let fmt = |v: f64| -> String {
        let v = if v.abs() < 1e-9 { 0.0 } else { v };
        format!("{:.*}", decimals, v)
    };
    // horizontal
    {
        let x0 = d.to_doc(pos2(top.min.x, 0.0)).x;
        let x1 = d.to_doc(pos2(top.max.x, 0.0)).x;
        let first = ((x0 - origin.x) / sub_pt).floor() as i64;
        let last = ((x1 - origin.x) / sub_pt).ceil() as i64;
        if last - first < 4000 {
            for k in first..=last {
                let x = origin.x + k as f64 * sub_pt;
                let sx = d.to_screen(Point::new(x, 0.0)).x.round() + 0.5;
                let major = k.rem_euclid(subdiv as i64) == 0;
                let half = subdiv % 2 == 0 && k.rem_euclid(subdiv as i64) == subdiv as i64 / 2;
                let h = if major { RULER - 2.0 } else if half { 7.0 } else { 4.0 };
                painter.line_segment([pos2(sx, top.max.y - h), pos2(sx, top.max.y)], Stroke::new(1.0, tick));
                if major {
                    painter.text(pos2(sx + 2.0, top.min.y + 1.0), egui::Align2::LEFT_TOP, fmt(k as f64 * sub_pt / ppu), font.clone(), label_col);
                }
            }
        }
    }
    // vertical (labels rotated)
    {
        let y0 = d.to_doc(pos2(0.0, left.min.y)).y;
        let y1 = d.to_doc(pos2(0.0, left.max.y)).y;
        let first = ((y0 - origin.y) / sub_pt).floor() as i64;
        let last = ((y1 - origin.y) / sub_pt).ceil() as i64;
        if last - first < 4000 {
            for k in first..=last {
                let y = origin.y + k as f64 * sub_pt;
                let sy = d.to_screen(Point::new(0.0, y)).y.round() + 0.5;
                let major = k.rem_euclid(subdiv as i64) == 0;
                let half = subdiv % 2 == 0 && k.rem_euclid(subdiv as i64) == subdiv as i64 / 2;
                let w = if major { RULER - 2.0 } else if half { 7.0 } else { 4.0 };
                painter.line_segment([pos2(left.max.x - w, sy), pos2(left.max.x, sy)], Stroke::new(1.0, tick));
                if major {
                    // y grows downwards in the document; the ruler shows values growing upwards
                    let galley = painter.layout_no_wrap(fmt(-(k as f64) * sub_pt / ppu), font.clone(), label_col);
                    let p = pos2(left.min.x + 1.0, sy - 2.0);
                    painter.add(egui::epaint::TextShape::new(p, galley, label_col).with_angle(-std::f32::consts::FRAC_PI_2));
                }
            }
        }
    }
    // pointer position markers
    if let Some(h) = hover {
        let st = Stroke::new(1.0, Color32::from_rgb(0x4d, 0xa3, 0xff));
        painter.line_segment([pos2(h.x, top.min.y), pos2(h.x, top.max.y)], st);
        painter.line_segment([pos2(left.min.x, h.y), pos2(left.max.x, h.y)], st);
    }
    // drag from a ruler to create a guideline; double-click opens the ruler options
    let uid = d.uid;
    for (rect, vertical, name) in [(top, false, "top"), (left, true, "left")] {
        let r = ui.interact(rect, Id::unique(("ruler", name, uid)), Sense::click_and_drag());
        if r.drag_started_by(egui::PointerButton::Primary) {
            app.ts.cancel();
            app.ts.drag = Drag::Guide { index: None, vertical };
            app.ts.press = r.interact_pointer_pos();
            app.ts.moved = true;
        }
        if r.double_clicked() {
            app.open_options(crate::commands::OptionsPage::Document, Some("Rulers"));
        }
        if r.hovered() {
            ui.ctx().set_cursor_icon(if vertical { CursorIcon::ResizeHorizontal } else { CursorIcon::ResizeVertical });
        }
    }
    let rc = ui.interact(corner, Id::unique(("ruler_corner", uid)), Sense::click_and_drag());
    if rc.drag_stopped() {
        if let Some(p) = ui.ctx().pointer_latest_pos() {
            if canvas.contains(p) {
                let dp = app.docs[ai].to_doc(p);
                let d = &mut app.docs[ai];
                d.modify("Move Ruler Origin", |doc, _| doc.settings.ruler_origin = dp);
            }
        }
    }
    if rc.double_clicked() {
        let d = &mut app.docs[ai];
        d.modify("Reset Ruler Origin", |doc, _| doc.settings.ruler_origin = Point::ZERO);
    }
    if rc.dragged() {
        if let Some(p) = ui.ctx().pointer_latest_pos() {
            let lp = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::unique("ruler_origin_drag")));
            let st = Stroke::new(1.0, Color32::from_rgb(0x4d, 0xa3, 0xff));
            lp.line_segment([pos2(canvas.min.x, p.y), pos2(canvas.max.x, p.y)], st);
            lp.line_segment([pos2(p.x, canvas.min.y), pos2(p.x, canvas.max.y)], st);
        }
    }
}

// ---------------------------------------------------------------------------------- page navigator

fn nav_button(ui: &mut Ui, rect: Rect, id: Id, kind: u8, enabled: bool, tip: &str) -> bool {
    let r = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if r.hovered() && enabled {
        p.rect_filled(rect.shrink(1.0), 2.0, theme::HOVER);
    }
    let col = if enabled { theme::TEXT } else { theme::DISABLED };
    let c = rect.center();
    let tri = |dir: f32, at: Pos2| {
        vec![at + vec2(-3.0 * dir, -4.0), at + vec2(3.0 * dir, 0.0), at + vec2(-3.0 * dir, 4.0)]
    };
    match kind {
        0 => {
            p.line_segment([c + vec2(-4.0, -4.0), c + vec2(-4.0, 4.0)], Stroke::new(1.5, col));
            p.add(egui::Shape::convex_polygon(tri(-1.0, c + vec2(1.0, 0.0)), col, Stroke::NONE));
        }
        1 => {
            p.add(egui::Shape::convex_polygon(tri(-1.0, c), col, Stroke::NONE));
        }
        2 => {
            p.add(egui::Shape::convex_polygon(tri(1.0, c), col, Stroke::NONE));
        }
        3 => {
            p.add(egui::Shape::convex_polygon(tri(1.0, c - vec2(1.0, 0.0)), col, Stroke::NONE));
            p.line_segment([c + vec2(4.0, -4.0), c + vec2(4.0, 4.0)], Stroke::new(1.5, col));
        }
        _ => {
            p.line_segment([c - vec2(5.0, 0.0), c + vec2(5.0, 0.0)], Stroke::new(1.5, col));
            p.line_segment([c - vec2(0.0, 5.0), c + vec2(0.0, 5.0)], Stroke::new(1.5, col));
        }
    }
    let r = r.on_hover_text(tip);
    enabled && r.clicked()
}

fn page_navigator(app: &mut VectorApp, ui: &mut Ui, rect: Rect) {
    let Some(ai) = app.active else { return };
    let p = ui.painter();
    p.rect_filled(rect, 0.0, theme::BAR);
    p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, theme::SEP));
    let uid = app.docs[ai].uid;
    let n = app.docs[ai].doc.pages.len();
    let cur = app.docs[ai].page;
    let mut x = rect.min.x + 4.0;
    let y = rect.min.y + 1.0;
    let b = |x: f32| Rect::from_min_size(pos2(x, y), vec2(20.0, NAV_H - 2.0));
    let mut goto: Option<usize> = None;
    if nav_button(ui, b(x), Id::unique(("nav_first", uid)), 0, cur > 0, "First Page") {
        goto = Some(0);
    }
    x += 20.0;
    if nav_button(ui, b(x), Id::unique(("nav_prev", uid)), 1, cur > 0, "Previous Page") {
        goto = Some(cur.saturating_sub(1));
    }
    x += 22.0;
    let label = format!("{} of {}", cur + 1, n);
    let lr = Rect::from_min_size(pos2(x, y), vec2(54.0, NAV_H - 2.0));
    let lresp = ui.interact(lr, Id::unique(("nav_label", uid)), Sense::click());
    ui.painter().text(lr.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(12.0), theme::TEXT);
    if lresp.clicked() {
        app.run(Cmd::GoToPage);
    }
    lresp.on_hover_text("Go To Page");
    x += 56.0;
    if nav_button(ui, b(x), Id::unique(("nav_next", uid)), 2, cur + 1 < n, "Next Page") {
        goto = Some(cur + 1);
    }
    x += 20.0;
    if nav_button(ui, b(x), Id::unique(("nav_last", uid)), 3, cur + 1 < n, "Last Page") {
        goto = Some(n - 1);
    }
    x += 24.0;
    if nav_button(ui, b(x), Id::unique(("nav_add", uid)), 4, true, "Add Page") {
        let d = &mut app.docs[ai];
        let (w, h) = (d.page_ref().width, d.page_ref().height);
        let at = n;
        d.modify("Insert Page", |doc, _| {
            doc.add_page(at, w, h);
        });
        d.page = at;
        d.selection.clear();
        d.cache.key = 0;
    }
    x += 26.0;
    // page tabs
    let font = egui::FontId::proportional(12.0);
    for i in 0..n {
        let name = app.docs[ai].doc.page_name(i);
        let w = ui.painter().layout_no_wrap(name.clone(), font.clone(), theme::TEXT).size().x + 18.0;
        let tr = Rect::from_min_size(pos2(x, rect.min.y), vec2(w, NAV_H));
        if tr.min.x > rect.max.x {
            break;
        }
        let resp = ui.interact(tr, Id::unique(("page_tab", uid, i)), Sense::click());
        let p = ui.painter();
        if i == cur {
            p.rect_filled(tr.shrink2(vec2(0.0, 1.0)), 0.0, theme::BG_CANVAS_TAB);
            p.line_segment([pos2(tr.min.x, tr.max.y - 1.5), pos2(tr.max.x, tr.max.y - 1.5)], Stroke::new(2.0, theme::FOCUS));
        } else if resp.hovered() {
            p.rect_filled(tr.shrink2(vec2(0.0, 1.0)), 0.0, theme::HOVER);
        }
        p.text(tr.center(), egui::Align2::CENTER_CENTER, &name, font.clone(), if i == cur { theme::TEXT } else { theme::TEXT_DIM });
        if resp.clicked() {
            goto = Some(i);
        }
        if resp.double_clicked() {
            app.dialogs.rename_page = Some((i, name.clone()));
        }
        resp.context_menu(|ui| {
            if ui.button("Rename Page...").clicked() {
                app.dialogs.rename_page = Some((i, name.clone()));
                ui.close();
            }
            if ui.button("Insert Page After").clicked() {
                app.dialogs.insert_page = Some((i, false));
                ui.close();
            }
            if ui.button("Insert Page Before").clicked() {
                app.dialogs.insert_page = Some((i, true));
                ui.close();
            }
            if ui.button("Duplicate Page...").clicked() {
                if let Some(d) = app.doc_mut() {
                    d.page = i;
                }
                app.run(Cmd::DuplicatePage);
                ui.close();
            }
            ui.separator();
            if ui.add_enabled(n > 1, egui::Button::new("Delete Page")).clicked() {
                if let Some(d) = app.doc_mut() {
                    d.page = i;
                }
                app.run(Cmd::DeletePage);
                ui.close();
            }
        });
        x += w;
        ui.painter().line_segment([pos2(x, rect.min.y + 5.0), pos2(x, rect.max.y - 5.0)], Stroke::new(1.0, theme::SEP));
    }
    if let Some(g) = goto {
        if let Some(d) = app.doc_mut() {
            if g != d.page && g < d.doc.pages.len() {
                d.page = g;
                d.selection.clear();
                d.node_sel.clear();
                d.text_edit = None;
                d.cache.key = 0;
                d.after_change();
            }
        }
        app.ts.cancel();
    }
}

// ---------------------------------------------------------------------------------- page sorter

fn page_sorter(app: &mut VectorApp, ui: &mut Ui, area: Rect) {
    let Some(ai) = app.active else { return };
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(area, 0.0, theme::DESKTOP);
    let n = app.docs[ai].doc.pages.len();
    let thumb = 160.0f32;
    let gap = 28.0f32;
    let cols = (((area.width() - gap) / (thumb + gap)).floor() as usize).max(1);
    let rev = app.docs[ai].history.revision;
    let uid = app.docs[ai].uid;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    egui::ScrollArea::vertical().id_salt(("sorter", uid)).show(&mut child, |ui| {
        let rows = n.div_ceil(cols);
        let (rect, _) = ui.allocate_exact_size(vec2(area.width(), rows as f32 * (thumb + gap + 20.0) + gap), Sense::hover());
        for i in 0..n {
            let (r, c) = (i / cols, i % cols);
            let cell = Rect::from_min_size(rect.min + vec2(gap + c as f32 * (thumb + gap), gap + r as f32 * (thumb + gap + 20.0)), vec2(thumb, thumb));
            let key = format!("sorter:{uid}:{i}:{rev}");
            if !app.ts.texture_cache.contains_key(&key) {
                app.ts.texture_cache.retain(|k, _| !k.starts_with(&format!("sorter:{uid}:{i}:")));
                let d = &app.docs[ai];
                let pg = &d.doc.pages[i];
                let dpi = (thumb as f64 * 72.0 / pg.width.max(pg.height).max(1.0)).max(1.0);
                if let Some(pm) = vector_render::render_page_image(&d.doc, i, dpi, Some(Color::WHITE), None, true) {
                    let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
                    let tex = ctx.load_texture(key.clone(), img, egui::TextureOptions::LINEAR);
                    app.ts.texture_cache.insert(key.clone(), tex);
                }
            }
            let d = &app.docs[ai];
            let selected = d.page == i;
            let resp = ui.interact(cell.expand(4.0), Id::unique(("sorter_page", uid, i)), Sense::click());
            if let Some(tex) = app.ts.texture_cache.get(&key) {
                let s = tex.size_vec2();
                let k = (thumb / s.x.max(s.y)).min(1.0);
                let ir = Rect::from_center_size(cell.center(), s * k);
                ui.painter().rect_filled(ir.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(90));
                ui.painter().image(tex.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                if selected {
                    ui.painter().rect_stroke(ir.expand(3.0), 0.0, Stroke::new(2.0, theme::FOCUS), egui::StrokeKind::Middle);
                } else if resp.hovered() {
                    ui.painter().rect_stroke(ir.expand(3.0), 0.0, Stroke::new(1.0, theme::TEXT_DIM), egui::StrokeKind::Middle);
                }
            }
            ui.painter().text(pos2(cell.center().x, cell.max.y + 12.0), egui::Align2::CENTER_CENTER, d.doc.page_name(i), egui::FontId::proportional(12.0), theme::TEXT);
            if resp.clicked() {
                let d = &mut app.docs[ai];
                d.page = i;
                d.selection.clear();
                d.cache.key = 0;
            }
            if resp.double_clicked() {
                let d = &mut app.docs[ai];
                d.page = i;
                d.show_page_sorter = false;
                d.needs_fit = true;
                d.cache.key = 0;
            }
        }
    });
}

// ---------------------------------------------------------------------------------- full-screen preview

pub fn fullscreen_preview(app: &mut VectorApp, ui: &mut Ui) {
    let rect = ui.max_rect();
    let ctx = ui.ctx().clone();
    let bg = if app.settings.fullscreen_page_border { Color32::from_gray(0x26) } else { Color32::WHITE };
    ui.painter().rect_filled(rect, 0.0, bg);
    let Some(ai) = app.active.filter(|a| *a < app.docs.len()) else {
        app.fullscreen_preview = false;
        return;
    };
    let ppp = ctx.pixels_per_point();
    let key = format!("fullscreen:{}:{}:{}:{}x{}", app.docs[ai].uid, app.docs[ai].history.revision, app.docs[ai].page, rect.width() as u32, rect.height() as u32);
    if !app.ts.texture_cache.contains_key(&key) {
        app.ts.texture_cache.retain(|k, _| !k.starts_with("fullscreen:"));
        let d = &app.docs[ai];
        let pg = d.page_ref();
        let k = ((rect.width() as f64 - 40.0) / pg.width).min((rect.height() as f64 - 40.0) / pg.height).max(0.01);
        let dpi = k * 72.0 * ppp as f64;
        let enhanced = app.settings.fullscreen_enhanced;
        if let Some(pm) = vector_render::render_page_image(&d.doc, d.page, dpi, Some(Color::WHITE), None, enhanced) {
            let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
            let tex = ctx.load_texture(key.clone(), img, egui::TextureOptions::LINEAR);
            app.ts.texture_cache.insert(key.clone(), tex);
        }
    }
    if let Some(tex) = app.ts.texture_cache.get(&key) {
        let s = tex.size_vec2() / ppp;
        let ir = Rect::from_center_size(rect.center(), s);
        if app.settings.fullscreen_page_border {
            ui.painter().rect_filled(ir.translate(vec2(4.0, 4.0)), 0.0, Color32::from_black_alpha(120));
        }
        ui.painter().image(tex.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    // any key or click returns to the drawing window; PageUp/PageDown change pages
    let (exit, next, prev) = ctx.input(|i| {
        let next = i.key_pressed(egui::Key::PageDown) || i.key_pressed(egui::Key::ArrowRight);
        let prev = i.key_pressed(egui::Key::PageUp) || i.key_pressed(egui::Key::ArrowLeft);
        let any_key = i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. }));
        (i.pointer.any_click() || (any_key && !next && !prev), next, prev)
    });
    if let Some(d) = app.doc_mut() {
        if next && d.page + 1 < d.doc.pages.len() {
            d.page += 1;
        }
        if prev && d.page > 0 {
            d.page -= 1;
        }
    }
    if exit {
        app.fullscreen_preview = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
    }
}
