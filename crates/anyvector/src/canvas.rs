//! The drawing window: rulers, the rendered page (cached texture), tool input and overlay,
//! the page navigator at the bottom, the page sorter view and full-screen preview.

use crate::app::AnyVectorApp;
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
/// View ▸ Scrollbars: the thin bars along the bottom and the right of the drawing window.
const SB: f32 = 12.0;
/// Multipage view: where each page sits, as an offset from the current page, by the Multipage
/// view settings (a grid of columns, one column, one row; custom as one row).
pub fn page_offsets(doc: &vector_core::Document, cur: usize) -> Vec<(usize, kurbo::Vec2)> {
    let s = &doc.settings;
    let gap = s.multipage_spacing.max(0.0);
    let cols = match s.multipage_arrangement {
        0 => s.multipage_columns.max(1) as usize,
        1 => 1,
        _ => usize::MAX,
    };
    let mut at = Vec::with_capacity(doc.pages.len());
    let (mut x, mut y, mut row_h) = (0.0, 0.0, 0.0f64);
    for (i, p) in doc.pages.iter().enumerate() {
        if i > 0 && i % cols == 0 {
            x = 0.0;
            y += row_h + gap;
            row_h = 0.0;
        }
        at.push(kurbo::Vec2::new(x, y));
        x += p.width + gap;
        row_h = row_h.max(p.height);
    }
    let base = at.get(cur).copied().unwrap_or_default();
    at.iter().enumerate().map(|(i, v)| (i, *v - base)).collect()
}

pub fn drawing_window(app: &mut AnyVectorApp, ui: &mut Ui) {
    let Some(ai) = app.active else { return };
    if ai >= app.docs.len() {
        return;
    }
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let show_nav = app.settings.show_doc_navigator;
    let sb = if app.settings.show_scrollbars { SB } else { 0.0 };
    let nav_h = if show_nav { NAV_H } else { 0.0 };
    let nav_rect = Rect::from_min_max(pos2(full.min.x, full.max.y - nav_h), full.max);
    let area = Rect::from_min_max(full.min, pos2(full.max.x - sb, full.max.y - nav_h - sb));
    if show_nav {
        page_navigator(app, ui, nav_rect);
    }

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
    // Edit Master shows a Finish button at the top left of the drawing
    let finish = finish_label(&app.docs[ai]).map(|t| {
        let w = 18.0 + ctx.fonts_mut(|f| f.layout_no_wrap(crate::i18n::t(t), egui::FontId::proportional(12.0), Color32::WHITE).size().x);
        (t, Rect::from_min_size(canvas.min + vec2(8.0, 8.0), vec2(w, 24.0)))
    });
    let over_finish = finish.is_some_and(|(_, r)| ctx.input(|i| i.pointer.latest_pos()).is_some_and(|p| r.contains(p)));
    let hovered = resp.contains_pointer() && !over_finish;
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
            key_c: ctx.input(|i| i.key_down(egui::Key::C)),
            time,
            dt,
        };
        if inp.pressed && app.docs[ai].doc.settings.multipage_view {
            // Multipage view: a click on another page makes it the current page
            let d = &mut app.docs[ai];
            let p = d.to_doc(inp.pos);
            if !d.page_ref().rect().contains(p) {
                let hit = page_offsets(&d.doc, d.page).into_iter().find(|(i, dv)| *i != d.page && d.doc.pages[*i].rect().contains(p - *dv));
                if let Some((i, dv)) = hit {
                    d.page = i;
                    d.selection.clear();
                    d.node_sel.clear();
                    d.text_edit = None;
                    d.origin.x += (dv.x * d.zoom) as f32;
                    d.origin.y += (dv.y * d.zoom) as f32;
                    d.cache.key = 0;
                    d.after_change();
                }
            }
        }
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
        tools::tick(app, &ctx);
        if hovered {
            app.cursor_doc = pos.map(|p| app.docs[ai].to_doc(p));
        }
        tools::handle_text_keys(app, &ctx);
        tools::handle_table_keys(app, &ctx);
        if app.rollover_live {
            let at = if hovered { pos.map(|p| app.docs[ai].to_doc(p)) } else { None };
            crate::dialogs::object::live_preview(app, at, down);
        }
    }
    // right-click menu for what is under the pointer (the right-click itself selected it)
    if let Some(name) = crate::menus::canvas_menu(app) {
        resp.context_menu(|ui| crate::menus::context_menu(app, ui, name));
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
        painter.rect_filled(canvas, 0.0, theme::DESKTOP());
    }
    tools::draw_overlay(app, &painter);
    crate::dockers::draw_hotspots(app, &painter, &app.docs[ai]);
    printable_area(app, &painter, ai);
    if let Some((text, r)) = finish {
        let b = ui.interact(r, Id::unique(("finish_edit", app.docs[ai].uid)), Sense::click());
        let fill = if b.hovered() { theme::HOVER() } else { Color32::from_rgb(0x2b, 0x2b, 0x2b) };
        painter.rect(r, 3.0, fill, egui::Stroke::new(1.0, Color32::from_gray(90)), egui::StrokeKind::Inside);
        painter.text(r.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(12.0), theme::TEXT());
        if b.clicked() && !modal {
            app.run(crate::commands::Cmd::PowerClipFinish);
        }
    }

    // ------------------------------------------------------------------ scrollbars
    if sb > 0.0 {
        let hbar = Rect::from_min_max(pos2(area.min.x, area.max.y), pos2(area.max.x, area.max.y + sb));
        let vbar = Rect::from_min_max(pos2(area.max.x, canvas.min.y), pos2(area.max.x + sb, area.max.y));
        let corner = Rect::from_min_size(area.max, vec2(sb, sb));
        scrollbars(app, ui, ai, canvas, hbar, vbar, corner);
    }

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

/// The canvas button that ends Edit Master (Edit Rollover ends from the Internet toolbar).
fn finish_label(d: &crate::docview::DocView) -> Option<&'static str> {
    d.symbol_edit.is_some().then_some("✓  Finish")
}

/// Re-render the page into the cached texture when anything that affects it changed.
fn render_cache(app: &mut AnyVectorApp, ctx: &egui::Context, ai: usize, canvas: Rect) {
    let ppp = ctx.pixels_per_point();
    let pw = (canvas.width() * ppp).round().max(1.0) as u32;
    let ph = (canvas.height() * ppp).round().max(1.0) as u32;
    let gamut = crate::dockers::gamut_overlay(app);
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
    d.doc.settings.multipage_view.hash(&mut h);
    d.simulate_overprint.hash(&mut h);
    (settings.proof_colors, settings.proof_env_kind, settings.proof_preserve, gamut).hash(&mut h);
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
        dim_others: (!d.focus.is_empty()).then(|| {
            // the focused objects and everything inside them stay at full strength
            let mut keep = d.focus.clone();
            for id in &d.focus {
                if let Some(o) = d.doc.find(*id) {
                    o.walk(&mut |x| keep.push(x.id));
                }
            }
            (keep, (settings.inactive_visibility / 100.0).clamp(0.0, 1.0) as f32)
        }),
        show_powerclip_frames: settings.powerclip_empty_lines,
        antialias: d.view_mode != ViewMode::Pixels,
        overprint: d.simulate_overprint,
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
    if d.view_mode == ViewMode::PatternPreview {
        // View ▸ Pattern Preview: the page is one tile, repeated over the whole window
        let pr = view.transform_rect_bbox(d.doc.pages[d.page].rect());
        let (tw, th) = (pr.width().ceil() as u32, pr.height().ceil() as u32);
        if (2..8192).contains(&tw) && (2..8192).contains(&th) {
            if let Some(mut tile) = tiny_skia::Pixmap::new(tw, th) {
                tile.fill(tiny_skia::Color::WHITE);
                let tv = Affine::translate((-pr.x0, -pr.y0)) * view;
                let inner = vector_render::RenderOptions { draw_page: false, ..opts.clone() };
                ts.renderer.render_page(&d.doc, d.page, tv, &mut tile, &inner);
                let (w, h) = (pr.width(), pr.height());
                let kx0 = (-pr.x0 / w).floor() as i64 - 1;
                let kx1 = ((pw as f64 - pr.x0) / w).ceil() as i64;
                let ky0 = (-pr.y0 / h).floor() as i64 - 1;
                let ky1 = ((ph as f64 - pr.y0) / h).ceil() as i64;
                for ky in ky0.max(-60)..=ky1.min(60) {
                    for kx in kx0.max(-60)..=kx1.min(60) {
                        let (x, y) = (pr.x0 + kx as f64 * w, pr.y0 + ky as f64 * h);
                        pm.draw_pixmap(0, 0, tile.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::from_translate(x as f32, y as f32), None);
                    }
                }
                // the tile being edited keeps its border
                if let Some(r) = tiny_skia::Rect::from_ltrb(pr.x0 as f32, pr.y0 as f32, pr.x1 as f32, pr.y1 as f32) {
                    let mut paint = tiny_skia::Paint::default();
                    paint.set_color(tiny_skia::Color::from_rgba8(0x40, 0x40, 0x40, 255));
                    let stroke = tiny_skia::Stroke { width: 1.0, ..Default::default() };
                    pm.stroke_path(&tiny_skia::PathBuilder::from_rect(r), &paint, &stroke, tiny_skia::Transform::identity(), None);
                }
            }
        }
    }
    if d.doc.settings.multipage_view && d.doc.pages.len() > 1 && d.view_mode != ViewMode::Pixels {
        // the other pages, side by side with the current one
        let others = vector_render::RenderOptions { draw_page: false, dim_others: None, ..opts.clone() };
        for (i, dv) in page_offsets(&d.doc, d.page) {
            if i == d.page {
                continue;
            }
            let v = view * Affine::translate(dv);
            let pr = v.transform_rect_bbox(d.doc.pages[i].rect());
            if pr.x1 < 0.0 || pr.x0 > pw as f64 || pr.y1 < 0.0 || pr.y0 > ph as f64 {
                continue;
            }
            let fill = |pm: &mut tiny_skia::Pixmap, r: kurbo::Rect, c: tiny_skia::Color| {
                if let Some(r) = tiny_skia::Rect::from_ltrb(r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32) {
                    let mut paint = tiny_skia::Paint::default();
                    paint.set_color(c);
                    pm.fill_rect(r, &paint, tiny_skia::Transform::identity(), None);
                }
            };
            fill(&mut pm, pr + kurbo::Vec2::new(3.0, 3.0), tiny_skia::Color::from_rgba8(0, 0, 0, 110));
            fill(&mut pm, pr, tiny_skia::Color::WHITE);
            ts.renderer.render_page(&d.doc, i, v, &mut pm, &others);
        }
    }
    if settings.proof_colors || gamut.is_some() {
        // without Proof colors the view keeps its colors and shows the gamut warning only
        let kind = if settings.proof_colors { settings.proof_env_kind } else { 9 };
        proof_pixmap(&mut pm, kind, gamut, [dc[0], dc[1], dc[2], 255]);
    }
    let img = egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], pm.data());
    match &mut d.cache.tex {
        Some(t) => t.set(img, egui::TextureOptions::NEAREST),
        None => d.cache.tex = Some(ctx.load_texture(format!("canvas{}", d.uid), img, egui::TextureOptions::NEAREST)),
    }
    d.cache.key = key;
}

/// View ▸ Proof Colors: the colors as the proofing environment shows them (`kind` 0: the press,
/// U.S. Web Coated (SWOP); 1: an RGB device, unchanged; 2: grayscale; else unchanged), and the
/// colors the press cannot print covered with the `gamut` overlay. Pixels of the desktop color
/// are left alone.
pub fn proof_pixmap(pm: &mut tiny_skia::Pixmap, kind: u8, gamut: Option<[u8; 4]>, skip: [u8; 4]) {
    for px in pm.data_mut().chunks_exact_mut(4) {
        if *px == skip || px[3] == 0 {
            continue;
        }
        let a = px[3] as f32;
        let un = |c: u8| (c as f32 * 255.0 / a).min(255.0);
        let (r, g, b) = (un(px[0]), un(px[1]), un(px[2]));
        let press = press_rgb(r / 255.0, g / 255.0, b / 255.0);
        let (r0, g0, b0) = (r, g, b);
        let (mut r, mut g, mut b) = match kind {
            0 => press,
            2 => {
                let l = 0.299 * r + 0.587 * g + 0.114 * b;
                (l, l, l)
            }
            _ => (r, g, b),
        };
        if let Some([gr, gg, gb, ga]) = gamut {
            // out of gamut: the press moves the color far from the original
            let dist = (press.0 - r0).abs() + (press.1 - g0).abs() + (press.2 - b0).abs();
            if dist > 60.0 {
                let t = ga as f32 / 255.0;
                r = r * (1.0 - t) + gr as f32 * t;
                g = g * (1.0 - t) + gg as f32 * t;
                b = b * (1.0 - t) + gb as f32 * t;
            }
        }
        let pre = |c: f32| (c.clamp(0.0, 255.0) * a / 255.0).round() as u8;
        px[0] = pre(r);
        px[1] = pre(g);
        px[2] = pre(b);
    }
}

/// RGB (0..1) → CMYK → the RGB that CMYK prints as on coated stock (0..255).
pub fn press_rgb(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let k = 1.0 - r.max(g).max(b);
    let (c, m, y) = if k >= 1.0 { (0.0, 0.0, 0.0) } else { ((1.0 - r - k) / (1.0 - k), (1.0 - g - k) / (1.0 - k), (1.0 - b - k) / (1.0 - k)) };
    let r = 255.0
        + c * (-4.387332 * c + 54.486152 * m + 18.822905 * y + 212.256625 * k - 285.233103)
        + m * (1.714976 * m - 5.609674 * y - 17.873871 * k - 5.497006)
        + y * (-2.521734 * y - 21.248923 * k + 17.511927)
        + k * (-21.861221 * k - 189.481808);
    let g = 255.0
        + c * (8.841041 * c + 60.118027 * m + 6.871426 * y + 31.159100 * k - 79.297084)
        + m * (-15.310361 * m + 17.575251 * y + 131.352509 * k - 190.945330)
        + y * (4.444339 * y + 9.863286 * k - 24.867416)
        + k * (-20.737325 * k - 187.804537);
    let b = 255.0
        + c * (0.884252 * c + 8.078678 * m + 30.899783 * y - 0.238832 * k - 14.183577)
        + m * (10.495933 * m + 63.023785 * y + 50.606958 * k - 112.238843)
        + y * (0.032960 * y + 115.603844 * k - 193.582094)
        + k * (-22.338168 * k - 180.126140);
    (r.clamp(0.0, 255.0), g.clamp(0.0, 255.0), b.clamp(0.0, 255.0))
}

/// View ▸ Page ▸ Printable Area: the part of the page the default printer can print on, with
/// the page centered on the paper.
fn printable_area(app: &mut AnyVectorApp, painter: &egui::Painter, ai: usize) {
    if !app.docs[ai].doc.settings.show_printable_area {
        return;
    }
    let landscape = app.docs[ai].page_ref().is_landscape();
    if app.printable_paper.map(|(l, _)| l) != Some(landscape) {
        let printer = crate::printing::default_printer().unwrap_or_default();
        let paper = if printer.is_empty() { crate::printing::Paper::fallback(landscape) } else { crate::printing::paper_for(&printer, None, landscape) };
        app.printable_paper = Some((landscape, paper));
    }
    let Some((_, paper)) = app.printable_paper else { return };
    let d = &app.docs[ai];
    let pg = d.page_ref();
    let off = kurbo::Vec2::new((pg.width - paper.width) / 2.0, (pg.height - paper.height) / 2.0);
    let r = d.to_screen_rect(paper.printable + off);
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.0, Color32::from_rgb(0x9a, 0x9a, 0x9a)), 6.0, 4.0));
}

/// The part of the drawing the scrollbars range over: the pages with a page-sized margin around
/// them, and what is in view.
fn scroll_extent(d: &crate::docview::DocView) -> kurbo::Rect {
    let mut r = d.page_ref().rect();
    if d.doc.settings.multipage_view {
        for (i, dv) in page_offsets(&d.doc, d.page) {
            r = r.union(d.doc.pages[i].rect() + dv);
        }
    }
    let pr = d.page_ref().rect();
    let r = r.inflate(pr.width(), pr.height());
    let (a, b) = (d.to_doc(d.canvas_rect.min), d.to_doc(d.canvas_rect.max));
    r.union(kurbo::Rect::new(a.x, a.y, b.x, b.y))
}

/// View ▸ Scrollbars, and the navigator at their corner: press it to see the whole drawing and
/// drag to choose the part to view.
fn scrollbars(app: &mut AnyVectorApp, ui: &mut Ui, ai: usize, canvas: Rect, hbar: Rect, vbar: Rect, corner: Rect) {
    let uid = app.docs[ai].uid;
    {
        let p = ui.painter();
        for r in [hbar, vbar, corner] {
            p.rect_filled(r, 0.0, theme::DESKTOP());
        }
    }
    let ext = scroll_extent(&app.docs[ai]);
    for vertical in [false, true] {
        let bar = if vertical { vbar } else { hbar };
        let resp = ui.interact(bar, Id::unique(("scrollbar", uid, vertical)), Sense::click_and_drag());
        let d = &mut app.docs[ai];
        let (a, b) = (d.to_doc(canvas.min), d.to_doc(canvas.max));
        let (e0, e1, v0, v1, len, start) = if vertical { (ext.y0, ext.y1, a.y, b.y, bar.height(), bar.min.y) } else { (ext.x0, ext.x1, a.x, b.x, bar.width(), bar.min.x) };
        let span = (e1 - e0).max(1e-6);
        let t0 = start + ((v0 - e0) / span) as f32 * len;
        let t1 = (start + ((v1 - e0) / span) as f32 * len).max(t0 + 20.0);
        if resp.dragged() {
            let delta = if vertical { resp.drag_delta().y } else { resp.drag_delta().x };
            let doc_delta = delta as f64 * span / len as f64;
            if vertical {
                d.origin.y -= (doc_delta * d.zoom) as f32;
            } else {
                d.origin.x -= (doc_delta * d.zoom) as f32;
            }
        } else if resp.clicked() {
            // a click on the track pages toward the pointer
            if let Some(pp) = resp.interact_pointer_pos() {
                let at = if vertical { pp.y } else { pp.x };
                let step = if vertical { canvas.height() } else { canvas.width() } * 0.9;
                let dir = if at < t0 { 1.0 } else if at > t1 { -1.0 } else { 0.0 };
                if vertical {
                    d.origin.y += dir * step;
                } else {
                    d.origin.x += dir * step;
                }
            }
        }
        let hot = resp.hovered() || resp.dragged();
        let col = if hot { Color32::from_gray(0xb8) } else { Color32::from_gray(0x8a) };
        let w = if hot { 4.0 } else { 2.0 };
        let p = ui.painter();
        if vertical {
            let x = bar.center().x;
            p.line_segment([pos2(x, t0.max(bar.min.y)), pos2(x, t1.min(bar.max.y))], Stroke::new(w, col));
        } else {
            let y = bar.center().y;
            p.line_segment([pos2(t0.max(bar.min.x), y), pos2(t1.min(bar.max.x), y)], Stroke::new(w, col));
        }
    }
    // the navigator
    let resp = ui.interact(corner, Id::unique(("navigator", uid)), Sense::click_and_drag());
    {
        let p = ui.painter();
        let c = corner.center() + vec2(-1.0, -1.0);
        let col = if resp.hovered() { theme::TEXT() } else { theme::TEXT_DIM() };
        p.circle_stroke(c, 3.2, Stroke::new(1.2, col));
        p.line_segment([c + vec2(2.3, 2.3), c + vec2(5.0, 5.0)], Stroke::new(1.5, col));
    }
    let resp = resp.on_hover_text("Navigator");
    if !(resp.is_pointer_button_down_on() || resp.dragged()) {
        return;
    }
    let ctx = ui.ctx().clone();
    let k = (180.0 / ext.width().max(ext.height())) as f32;
    let size = vec2(ext.width() as f32 * k, ext.height() as f32 * k);
    let popup = Rect::from_min_size(corner.min - size - vec2(2.0, 2.0), size);
    let to_pop = |q: Point| popup.min + vec2(((q.x - ext.x0) as f32) * k, ((q.y - ext.y0) as f32) * k);
    if let Some(pp) = ctx.input(|i| i.pointer.interact_pos()) {
        if popup.contains(pp) {
            // center the view on the point under the pointer
            let q = Point::new(ext.x0 + ((pp.x - popup.min.x) / k) as f64, ext.y0 + ((pp.y - popup.min.y) / k) as f64);
            let d = &mut app.docs[ai];
            let cs = d.canvas_rect.size();
            d.origin = vec2(cs.x / 2.0 - (q.x * d.zoom) as f32, cs.y / 2.0 - (q.y * d.zoom) as f32);
        }
    }
    let d = &app.docs[ai];
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::unique(("navigator_popup", uid))));
    p.rect_filled(popup.expand(4.0), 2.0, theme::BAR());
    p.rect_stroke(popup.expand(4.0), 2.0, Stroke::new(1.0, theme::SEP()), egui::StrokeKind::Inside);
    p.rect_filled(popup, 0.0, theme::DESKTOP());
    let pages: Vec<(usize, kurbo::Vec2)> = if d.doc.settings.multipage_view { page_offsets(&d.doc, d.page) } else { vec![(d.page, kurbo::Vec2::ZERO)] };
    for (i, dv) in pages {
        let pr = d.doc.pages[i].rect() + dv;
        let r = Rect::from_two_pos(to_pop(Point::new(pr.x0, pr.y0)), to_pop(Point::new(pr.x1, pr.y1)));
        p.rect_filled(r, 0.0, Color32::WHITE);
        if i == d.page {
            for o in d.page_ref().layers.iter().filter(|l| l.visible).flat_map(|l| l.objects.iter()) {
                let b = d.doc.bounds_of(o);
                let br = Rect::from_two_pos(to_pop(Point::new(b.x0, b.y0)), to_pop(Point::new(b.x1, b.y1)));
                p.rect_filled(br, 0.0, Color32::from_gray(0xb0));
            }
        }
    }
    let (a, b) = (d.to_doc(canvas.min), d.to_doc(canvas.max));
    let vr = Rect::from_two_pos(to_pop(a), to_pop(b)).intersect(popup);
    p.rect_stroke(vr, 0.0, Stroke::new(1.5, theme::FOCUS()), egui::StrokeKind::Middle);
}

fn cursor_for(app: &AnyVectorApp, pos: Pos2) -> CursorIcon {
    if matches!(app.ts.drag, Drag::Pan { .. }) {
        return CursorIcon::Grabbing;
    }
    if app.ts.pick_target.is_some() {
        return CursorIcon::PointingHand;
    }
    if app.ts.markup.is_some() {
        return CursorIcon::Crosshair;
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

fn rulers(app: &mut AnyVectorApp, ui: &mut Ui, ai: usize, area: Rect, canvas: Rect, hover: Option<Pos2>) {
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
    painter.rect_filled(top, 0.0, theme::RULER());
    painter.rect_filled(left, 0.0, theme::RULER());
    painter.rect_filled(corner, 0.0, theme::RULER());
    painter.line_segment([pos2(top.min.x, top.max.y - 0.5), pos2(top.max.x, top.max.y - 0.5)], Stroke::new(1.0, theme::SEP()));
    painter.line_segment([pos2(left.max.x - 0.5, left.min.y), pos2(left.max.x - 0.5, left.max.y)], Stroke::new(1.0, theme::SEP()));
    painter.line_segment([pos2(corner.max.x - 0.5, corner.min.y), pos2(corner.max.x - 0.5, corner.max.y)], Stroke::new(1.0, theme::SEP()));
    painter.line_segment([pos2(corner.min.x, corner.max.y - 0.5), pos2(corner.max.x, corner.max.y - 0.5)], Stroke::new(1.0, theme::SEP()));
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
        r.context_menu(|ui| crate::menus::context_menu(app, ui, "Ruler Menu"));
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
        p.rect_filled(rect.shrink(1.0), 2.0, theme::HOVER());
    }
    let col = if enabled { theme::TEXT() } else { theme::DISABLED() };
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

fn page_navigator(app: &mut AnyVectorApp, ui: &mut Ui, rect: Rect) {
    let Some(ai) = app.active else { return };
    let p = ui.painter();
    p.rect_filled(rect, 0.0, theme::BAR());
    p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, theme::SEP()));
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
    ui.painter().text(lr.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(12.0), theme::TEXT());
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
        let w = ui.painter().layout_no_wrap(name.clone(), font.clone(), theme::TEXT()).size().x + 18.0;
        let tr = Rect::from_min_size(pos2(x, rect.min.y), vec2(w, NAV_H));
        if tr.min.x > rect.max.x {
            break;
        }
        let resp = ui.interact(tr, Id::unique(("page_tab", uid, i)), Sense::click());
        let p = ui.painter();
        if i == cur {
            p.rect_filled(tr.shrink2(vec2(0.0, 1.0)), 0.0, theme::BG_CANVAS_TAB());
            p.line_segment([pos2(tr.min.x, tr.max.y - 1.5), pos2(tr.max.x, tr.max.y - 1.5)], Stroke::new(2.0, theme::FOCUS()));
        } else if resp.hovered() {
            p.rect_filled(tr.shrink2(vec2(0.0, 1.0)), 0.0, theme::HOVER());
        }
        p.text(tr.center(), egui::Align2::CENTER_CENTER, &name, font.clone(), if i == cur { theme::TEXT() } else { theme::TEXT_DIM() });
        if resp.clicked() {
            goto = Some(i);
        }
        if resp.double_clicked() {
            app.dialogs.rename_page = Some((i, name.clone()));
        }
        if resp.secondary_clicked() {
            goto = Some(i);
        }
        resp.context_menu(|ui| crate::menus::context_menu(app, ui, "Page Tab"));
        x += w;
        ui.painter().line_segment([pos2(x, rect.min.y + 5.0), pos2(x, rect.max.y - 5.0)], Stroke::new(1.0, theme::SEP()));
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

fn page_sorter(app: &mut AnyVectorApp, ui: &mut Ui, area: Rect) {
    let Some(ai) = app.active else { return };
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(area, 0.0, theme::DESKTOP());
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
                    ui.painter().rect_stroke(ir.expand(3.0), 0.0, Stroke::new(2.0, theme::FOCUS()), egui::StrokeKind::Middle);
                } else if resp.hovered() {
                    ui.painter().rect_stroke(ir.expand(3.0), 0.0, Stroke::new(1.0, theme::TEXT_DIM()), egui::StrokeKind::Middle);
                }
            }
            ui.painter().text(pos2(cell.center().x, cell.max.y + 12.0), egui::Align2::CENTER_CENTER, d.doc.page_name(i), egui::FontId::proportional(12.0), theme::TEXT());
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

pub fn fullscreen_preview(app: &mut AnyVectorApp, ui: &mut Ui) {
    let rect = ui.max_rect();
    let ctx = ui.ctx().clone();
    let bg = if app.settings.fullscreen_page_border { Color32::from_gray(0x26) } else { Color32::WHITE };
    ui.painter().rect_filled(rect, 0.0, bg);
    let Some(ai) = app.active.filter(|a| *a < app.docs.len()) else {
        app.fullscreen_preview = false;
        return;
    };
    let ppp = ctx.pixels_per_point();
    let key = format!("fullscreen:{}:{}:{}:{}x{}:{}", app.docs[ai].uid, app.docs[ai].history.revision, app.docs[ai].page, rect.width() as u32, rect.height() as u32, app.preview_selected);
    if !app.ts.texture_cache.contains_key(&key) {
        app.ts.texture_cache.retain(|k, _| !k.starts_with("fullscreen:"));
        let d = &app.docs[ai];
        let pg = d.page_ref();
        let k = ((rect.width() as f64 - 40.0) / pg.width).min((rect.height() as f64 - 40.0) / pg.height).max(0.01);
        let dpi = k * 72.0 * ppp as f64;
        let enhanced = app.settings.fullscreen_enhanced;
        let pm = if app.preview_selected {
            // View ▸ Preview Selected Only: everything but the selection is left out
            let hidden: Vec<vector_core::ObjId> = pg.layers.iter().flat_map(|l| l.objects.iter()).map(|o| o.id).filter(|id| !d.selection.contains(id)).collect();
            let (w, h) = ((pg.width * dpi / 72.0).ceil().max(1.0) as u32, (pg.height * dpi / 72.0).ceil().max(1.0) as u32);
            tiny_skia::Pixmap::new(w, h).map(|mut pm| {
                pm.fill(tiny_skia::Color::WHITE);
                let opts = vector_render::RenderOptions { draw_page: false, hidden, antialias: enhanced, ..Default::default() };
                app.ts.renderer.render_page(&d.doc, d.page, Affine::scale(dpi / 72.0), &mut pm, &opts);
                pm
            })
        } else {
            vector_render::render_page_image(&d.doc, d.page, dpi, Some(Color::WHITE), None, enhanced)
        };
        if let Some(pm) = pm {
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
        app.preview_selected = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
    }
}

/// Window ▸ Cascade, Tile Horizontally and Tile Vertically: every document in its own window
/// with a title bar (its name, Maximize and Close). The active window takes the input; a click
/// on another one makes it active.
pub fn arranged_windows(app: &mut AnyVectorApp, ui: &mut Ui) {
    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());
    ui.painter().rect_filled(area, 0.0, theme::BG());
    let n = app.docs.len();
    let ai = app.active.unwrap_or(0).min(n - 1);
    let area = area.shrink(2.0);
    let rects: Vec<Rect> = match app.doc_layout {
        // one above another (a grid past three windows)
        1 | 2 if n > 3 => {
            let cols = (n as f32).sqrt().ceil() as usize;
            let rows = n.div_ceil(cols);
            let (w, h) = (area.width() / cols as f32, area.height() / rows as f32);
            (0..n).map(|i| Rect::from_min_size(area.min + vec2((i % cols) as f32 * w, (i / cols) as f32 * h), vec2(w, h)).shrink(1.0)).collect()
        }
        1 => {
            let h = area.height() / n as f32;
            (0..n).map(|i| Rect::from_min_size(area.min + vec2(0.0, i as f32 * h), vec2(area.width(), h)).shrink(1.0)).collect()
        }
        2 => {
            let w = area.width() / n as f32;
            (0..n).map(|i| Rect::from_min_size(area.min + vec2(i as f32 * w, 0.0), vec2(w, area.height())).shrink(1.0)).collect()
        }
        // cascade: each window down and to the right of the one before
        _ => {
            let step = 28.0;
            let size = vec2((area.width() - step * (n - 1) as f32).max(area.width() * 0.5), (area.height() - step * (n - 1) as f32).max(area.height() * 0.5));
            (0..n).map(|i| Rect::from_min_size(area.min + vec2(step, step) * (i as f32 % 12.0), size)).collect()
        }
    };
    // the active window last, on top
    let mut order: Vec<usize> = (0..n).filter(|i| *i != ai).collect();
    order.push(ai);
    let (mut activate, mut close, mut maximize) = (None, None, None);
    for i in order {
        let r = rects[i];
        let active = i == ai;
        let title = Rect::from_min_size(r.min, vec2(r.width(), 26.0));
        let body = Rect::from_min_max(pos2(r.min.x + 1.0, title.max.y), pos2(r.max.x - 1.0, r.max.y - 1.0));
        let p = ui.painter().clone();
        p.rect_filled(r, 0.0, theme::BAR());
        p.rect_stroke(r, 0.0, Stroke::new(1.0, if active { theme::FOCUS() } else { theme::SEP() }), egui::StrokeKind::Inside);
        let t = ui.interact(title, Id::unique(("doc_win_title", app.docs[i].uid)), Sense::click());
        if t.clicked() || t.double_clicked() {
            activate = Some(i);
        }
        if t.double_clicked() {
            maximize = Some(i);
        }
        let name = app.docs[i].display_name();
        p.text(pos2(title.left() + 10.0, title.center().y), egui::Align2::LEFT_CENTER, name, theme::body(12.0), if active { theme::TEXT() } else { theme::TEXT_DIM() });
        // Maximize (back to the tabs with this document) and Close
        for (k, tip) in [(0, "Close"), (1, "Maximize")] {
            let b = Rect::from_center_size(pos2(title.right() - 16.0 - k as f32 * 30.0, title.center().y), vec2(28.0, 22.0));
            let resp = ui.interact(b, Id::unique(("doc_win_btn", app.docs[i].uid, k)), Sense::click()).on_hover_text(tip);
            if resp.hovered() {
                p.rect_filled(b, 2.0, if k == 0 { Color32::from_rgb(0xc4, 0x2b, 0x1c) } else { theme::HOVER() });
            }
            let c = b.center();
            let st = Stroke::new(1.1, theme::TEXT());
            if k == 0 {
                p.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], st);
                p.line_segment([c + vec2(-5.0, 5.0), c + vec2(5.0, -5.0)], st);
            } else {
                p.rect_stroke(Rect::from_center_size(c, vec2(10.0, 10.0)), 0.0, st, egui::StrokeKind::Inside);
            }
            if resp.clicked() {
                if k == 0 {
                    close = Some(i);
                } else {
                    maximize = Some(i);
                }
            }
        }
        if active {
            ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                ui.set_clip_rect(body);
                drawing_window(app, ui);
            });
        } else if passive_window(app, ui, i, body) {
            activate = Some(i);
        }
    }
    if let Some(i) = activate {
        if app.active != Some(i) {
            app.finish_tool();
            app.active = Some(i);
        }
    }
    if let Some(i) = maximize {
        app.active = Some(i);
        app.doc_layout = 0;
    }
    if let Some(i) = close {
        app.request_close_doc(i);
    }
}

/// A window that is not the active one: its drawing, laid out as the drawing window would be
/// (rulers, scrollbars, page navigator), without its controls. True when clicked.
fn passive_window(app: &mut AnyVectorApp, ui: &mut Ui, i: usize, full: Rect) -> bool {
    let sb = if app.settings.show_scrollbars { SB } else { 0.0 };
    let nav_h = if app.settings.show_doc_navigator { NAV_H } else { 0.0 };
    let area = Rect::from_min_max(full.min, pos2(full.max.x - sb, full.max.y - nav_h - sb));
    let r = if app.docs[i].doc.settings.show_rulers { RULER } else { 0.0 };
    let canvas = Rect::from_min_max(area.min + vec2(r, r), area.max);
    let p = ui.painter_at(full);
    p.rect_filled(full, 0.0, theme::BG());
    if r > 0.0 {
        p.rect_filled(Rect::from_min_max(area.min, pos2(area.max.x, area.min.y + r)), 0.0, theme::RULER());
        p.rect_filled(Rect::from_min_max(area.min, pos2(area.min.x + r, area.max.y)), 0.0, theme::RULER());
    }
    if canvas.width() < 8.0 || canvas.height() < 8.0 {
        return false;
    }
    {
        let d = &mut app.docs[i];
        d.canvas_rect = canvas;
        if d.needs_fit {
            d.zoom_page();
            d.needs_fit = false;
        }
    }
    render_cache(app, ui.ctx(), i, canvas);
    if let Some(tex) = &app.docs[i].cache.tex {
        p.image(tex.id(), canvas, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    ui.interact(full, Id::unique(("doc_win_body", app.docs[i].uid)), Sense::click()).clicked()
}
