//! On-canvas feedback drawn over the rendered page: grid, guides, selection handles, nodes,
//! previews of drags in progress, text caret and snap marks.

use super::interact::{creation_object, envelope_handle, handle_point, three_point_object, warp_bbox, xform_affine};
use super::*;
use crate::theme;
use egui::{pos2, Color32, Painter, Pos2, Rect as ERect, Shape as EShape, Stroke};
use kurbo::{BezPath, Shape};
use vector_core::geom;
use vector_core::object::{dimension_path, ConnectorEnd, ConnectorKind, ConnectorObj, DimensionKind, DimensionObj};

const PREVIEW: Color32 = Color32::from_rgb(0x1e, 0x90, 0xff);

fn screen_affine(d: &DocView) -> Affine {
    Affine::translate((d.canvas_rect.min.x as f64, d.canvas_rect.min.y as f64)) * d.view()
}

fn to_pos(p: Point) -> Pos2 {
    pos2(p.x as f32, p.y as f32)
}

/// Stroke a document-space path on screen.
pub fn stroke_bez(painter: &Painter, d: &DocView, bp: &BezPath, stroke: Stroke) {
    let mut t = bp.clone();
    t.apply_affine(screen_affine(d));
    for (pts, closed) in geom::flatten_polylines(&t, 0.35) {
        let pts: Vec<Pos2> = pts.into_iter().map(to_pos).collect();
        if pts.len() < 2 {
            continue;
        }
        if closed {
            painter.add(EShape::closed_line(pts, stroke));
        } else {
            painter.add(EShape::line(pts, stroke));
        }
    }
}

fn dashed_bez(painter: &Painter, d: &DocView, bp: &BezPath, color: Color32) {
    let mut t = bp.clone();
    t.apply_affine(screen_affine(d));
    for (mut pts, closed) in geom::flatten_polylines(&t, 0.35) {
        if closed && !pts.is_empty() {
            pts.push(pts[0]);
        }
        let pts: Vec<Pos2> = pts.into_iter().map(to_pos).collect();
        painter.extend(EShape::dashed_line(&pts, Stroke::new(1.0, color), 4.0, 3.0));
    }
}

fn dashed_rect(painter: &Painter, r: ERect, color: Color32) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    painter.extend(EShape::dashed_line(&pts, Stroke::new(1.0, Color32::from_black_alpha(160)), 4.0, 0.0));
    painter.extend(EShape::dashed_line(&pts, Stroke::new(1.0, color), 4.0, 3.0));
}

fn handle(painter: &Painter, c: Pos2, size: f32, filled: bool) {
    let r = ERect::from_center_size(c, egui::vec2(size, size));
    painter.rect_filled(r, 0.0, if filled { Color32::BLACK } else { Color32::WHITE });
    painter.rect_stroke(r, 0.0, Stroke::new(1.0, if filled { Color32::WHITE } else { Color32::BLACK }), egui::StrokeKind::Middle);
}

fn node_size(s: &Settings) -> f32 {
    match s.node_size {
        crate::settings::NodeSize::Small => 5.0,
        crate::settings::NodeSize::Medium => 7.0,
        crate::settings::NodeSize::Large => 9.0,
    }
}

pub fn draw_overlay(app: &mut AnyVectorApp, painter: &Painter) {
    let time = app.time();
    let tool = app.tool;
    let Some(ai) = app.active else { return };
    let ts = &app.ts;
    let s = &app.settings;
    let Some(d) = app.docs.get(ai) else { return };
    draw_grid(painter, d);
    draw_pixel_grid(painter, d);
    draw_baseline_grid(painter, d);
    draw_guides(painter, d, &ts.drag, s.guide_line_style);
    draw_text_frames(painter, d, s);
    if app.dockers.ui.comment_show {
        draw_comments(painter, d, app.dockers.ui.comment_resolved, app.dockers.ui.comment_sel);
    }
    crate::dialogs::symmetry::draw_overlay(painter, d);

    let dragging_xform = matches!(ts.drag, Drag::Transform { .. });
    match tool {
        ToolId::Shape => draw_nodes(painter, d, s),
        ToolId::Text if d.text_edit.is_some() => {}
        ToolId::Envelope => draw_envelope(painter, d),
        ToolId::InteractiveFill | ToolId::Transparency => draw_gradient_vector(painter, d, tool),
        ToolId::MeshFill => draw_mesh(painter, d, ts),
        ToolId::EditAnchor => draw_anchors(painter, d, ts),
        _ => {
            if !dragging_xform && !(s.hide_bbox_curve_tools && tool.is_curve_creation()) {
                draw_selection(painter, d, tool == ToolId::FreeTransform || d.rotate_mode);
            }
        }
    }
    if tool == ToolId::Table {
        draw_table_cell(painter, d, ts);
    }
    if tool == ToolId::VariableOutline {
        draw_var_outline(painter, d, ts);
    }
    // LiveSketch strokes waiting for the timer
    if s.topts.ls_preview {
        for st in &ts.ls_pending {
            let pts: Vec<Pos2> = st.iter().map(|p| d.to_screen(*p)).collect();
            if pts.len() >= 2 {
                painter.add(EShape::line(pts, Stroke::new(1.5, Color32::from_rgb(0x1e, 0x90, 0xff))));
            }
        }
    }
    if tool == ToolId::Text {
        draw_selection(painter, d, false);
    }
    draw_drag_preview(painter, d, ts, s, tool);
    draw_in_progress(painter, d, ts, s, tool);
    draw_text_caret(painter, d, ts, time);
    // brush cursor
    if matches!(tool, ToolId::Smooth | ToolId::Smear | ToolId::Twirl | ToolId::AttractRepel | ToolId::Smudge | ToolId::Roughen | ToolId::Eraser) {
        if let Some(h) = ts.hover_doc {
            let r = if tool == ToolId::Eraser { s.eraser_thickness / 2.0 } else { ts.brush_size / 2.0 };
            painter.circle_stroke(d.to_screen(h), (r * d.zoom) as f32, Stroke::new(1.0, Color32::from_gray(200)));
            painter.circle_stroke(d.to_screen(h), (r * d.zoom) as f32 + 1.0, Stroke::new(1.0, Color32::from_black_alpha(120)));
        }
    }
    // alignment and dynamic guides
    for (a, b, kind) in &ts.guide_lines {
        let (a, b) = (d.to_screen(*a), d.to_screen(*b));
        let (c, style) = match kind {
            0 => (s.align_guide_color, s.align_guide_style),
            1 => (s.guide_margin_color, s.guide_margin_style),
            _ => (s.dyn_guide_color, s.dyn_guide_style),
        };
        crate::dockers::styled_line(painter, a, b, Stroke::new(1.0, Color32::from_rgb(c[0], c[1], c[2])), style);
    }
    // snap mark
    if s.show_snap_marks {
        if let Some((p, label)) = ts.snap_mark {
            let c = d.to_screen(p);
            painter.rect_stroke(ERect::from_center_size(c, egui::vec2(9.0, 9.0)), 0.0, Stroke::new(1.5, Color32::from_rgb(0x00, 0xb4, 0xff)), egui::StrokeKind::Middle);
            if s.show_snap_tip {
                painter.text(c + egui::vec2(9.0, -12.0), egui::Align2::LEFT_BOTTOM, label, egui::FontId::proportional(11.0), Color32::from_rgb(0x00, 0xb4, 0xff));
            }
        }
    }
}

/// View ▸ Grid ▸ Baseline Grid: lines across the page from its top, one baseline apart.
fn draw_baseline_grid(painter: &Painter, d: &DocView) {
    let ds = &d.doc.settings;
    let sp = ds.baseline_spacing.max(0.1);
    if !ds.show_baseline_grid || sp * d.zoom < 3.0 {
        return;
    }
    let pr = d.page_ref().rect();
    let (x0, x1) = (d.to_screen(Point::new(pr.x0, 0.0)).x, d.to_screen(Point::new(pr.x1, 0.0)).x);
    let col = Color32::from_rgba_unmultiplied(0x4d, 0xc3, 0xe6, 170);
    let cr = d.canvas_rect;
    let mut y = pr.y0 + sp;
    while y < pr.y1 {
        let sy = d.to_screen(Point::new(0.0, y)).y;
        if sy > cr.bottom() {
            break;
        }
        if sy >= cr.top() {
            painter.line_segment([pos2(x0, sy), pos2(x1, sy)], Stroke::new(1.0, col));
        }
        y += sp;
    }
}

/// View ▸ Grid ▸ Pixel Grid (Pixels view): the document pixels, once they are big enough to see.
fn draw_pixel_grid(painter: &Painter, d: &DocView) {
    let ds = &d.doc.settings;
    if !ds.pixel_grid || d.view_mode != crate::docview::ViewMode::Pixels {
        return;
    }
    let px = 72.0 / ds.dpi.max(1.0);
    if px * d.zoom < 6.0 {
        return;
    }
    let col = Color32::from_rgba_unmultiplied(0x80, 0x80, 0x80, 90);
    let cr = d.canvas_rect;
    let (tl, br) = (d.to_doc(cr.min), d.to_doc(cr.max));
    let mut x = (tl.x / px).floor() * px;
    while x <= br.x {
        let sx = d.to_screen(Point::new(x, 0.0)).x;
        painter.line_segment([pos2(sx, cr.top()), pos2(sx, cr.bottom())], Stroke::new(1.0, col));
        x += px;
    }
    let mut y = (tl.y / px).floor() * px;
    while y <= br.y {
        let sy = d.to_screen(Point::new(0.0, y)).y;
        painter.line_segment([pos2(cr.left(), sy), pos2(cr.right(), sy)], Stroke::new(1.0, col));
        y += px;
    }
}

fn draw_grid(painter: &Painter, d: &DocView) {
    let ds = &d.doc.settings;
    if !ds.show_grid {
        return;
    }
    let mut gx = ds.grid_spacing.x.max(1e-3);
    let mut gy = ds.grid_spacing.y.max(1e-3);
    while gx * d.zoom < 6.0 {
        gx *= 2.0;
    }
    while gy * d.zoom < 6.0 {
        gy *= 2.0;
    }
    let c = ds.grid_color;
    let alpha = (ds.grid_opacity.clamp(0.0, 100.0) / 100.0 * 255.0) as u8;
    let col = Color32::from_rgba_unmultiplied(c.r, c.g, c.b, alpha);
    let cr = d.canvas_rect;
    let tl = d.to_doc(cr.min);
    let br = d.to_doc(cr.max);
    let o = ds.ruler_origin;
    let dots = matches!(ds.grid_kind, vector_core::document::GridKind::Dots);
    let mut x = ((tl.x - o.x) / gx).floor() * gx + o.x;
    let mut count = 0;
    while x <= br.x && count < 2000 {
        let sx = d.to_screen(Point::new(x, 0.0)).x;
        if dots {
            let mut y = ((tl.y - o.y) / gy).floor() * gy + o.y;
            while y <= br.y && count < 40000 {
                let sy = d.to_screen(Point::new(0.0, y)).y;
                painter.rect_filled(ERect::from_center_size(pos2(sx, sy), egui::vec2(1.5, 1.5)), 0.0, col);
                y += gy;
                count += 1;
            }
        } else {
            painter.line_segment([pos2(sx, cr.top()), pos2(sx, cr.bottom())], Stroke::new(1.0, col));
        }
        x += gx;
        count += 1;
    }
    if !dots {
        let mut y = ((tl.y - o.y) / gy).floor() * gy + o.y;
        let mut count = 0;
        while y <= br.y && count < 2000 {
            let sy = d.to_screen(Point::new(0.0, y)).y;
            painter.line_segment([pos2(cr.left(), sy), pos2(cr.right(), sy)], Stroke::new(1.0, col));
            y += gy;
            count += 1;
        }
    }
}

fn draw_guides(painter: &Painter, d: &DocView, drag: &Drag, style: usize) {
    if !d.doc.settings.show_guides {
        return;
    }
    let cr = d.canvas_rect;
    let moving = match drag {
        Drag::Guide { index: Some(i), .. } => Some(*i),
        _ => None,
    };
    let page = d.page_ref();
    for (i, g) in page.guides.iter().enumerate().map(|(i, g)| (Some(i), g)).chain(d.doc.master_guides.iter().map(|g| (None, g))) {
        if i.is_some() && i == moving {
            continue;
        }
        // the selected guideline is red, as in CorelDRAW
        let selected = i.is_some() && i == d.guide_sel.filter(|(p, _)| *p == d.page && d.selection.is_empty()).map(|(_, i)| i);
        let col = if selected { Color32::from_rgb(0xff, 0x00, 0x00) } else { Color32::from_rgb(g.color.r, g.color.g, g.color.b) };
        let stroke = Stroke::new(if selected { 1.5 } else { 1.0 }, col);
        match g.kind {
            vector_core::GuideKind::Horizontal => {
                let y = d.to_screen(Point::new(0.0, g.pos.y)).y;
                if g.locked {
                    painter.extend(EShape::dashed_line(&[pos2(cr.left(), y), pos2(cr.right(), y)], stroke, 6.0, 3.0));
                } else {
                    crate::dockers::styled_line(painter, pos2(cr.left(), y), pos2(cr.right(), y), stroke, style);
                }
            }
            vector_core::GuideKind::Vertical => {
                let x = d.to_screen(Point::new(g.pos.x, 0.0)).x;
                if g.locked {
                    painter.extend(EShape::dashed_line(&[pos2(x, cr.top()), pos2(x, cr.bottom())], stroke, 6.0, 3.0));
                } else {
                    crate::dockers::styled_line(painter, pos2(x, cr.top()), pos2(x, cr.bottom()), stroke, style);
                }
            }
            vector_core::GuideKind::Angled(deg) => {
                let a = deg.to_radians();
                let dir = Vec2::new(a.cos(), -a.sin()) * 1e5;
                let p0 = d.to_screen(g.pos - dir);
                let p1 = d.to_screen(g.pos + dir);
                if g.locked {
                    painter.extend(EShape::dashed_line(&[p0, p1], stroke, 6.0, 3.0));
                } else {
                    crate::dockers::styled_line(painter, p0, p1, stroke, style);
                }
            }
        }
    }
}

thread_local! {
    /// Text layouts of the frames on screen (laid out again only when the text changes).
    static TEXT_LAYOUTS: std::cell::RefCell<std::collections::HashMap<vector_core::ObjId, (vector_core::text::TextObj, std::rc::Rc<vector_core::text::TextLayout>)>> = Default::default();
    /// Spelling errors per text (content and language).
    static SPELL_ERRORS: std::cell::RefCell<std::collections::HashMap<(String, String), Vec<crate::spell::SpellError>>> = Default::default();
}

fn cached_layout(id: vector_core::ObjId, t: &vector_core::text::TextObj) -> std::rc::Rc<vector_core::text::TextLayout> {
    TEXT_LAYOUTS.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((k, l)) = c.get(&id) {
            if k == t {
                return l.clone();
            }
        }
        let l = std::rc::Rc::new(vector_core::text::layout(t));
        if c.len() > 400 {
            c.clear();
        }
        c.insert(id, (t.clone(), l.clone()));
        l
    })
}

fn spell_errors(t: &vector_core::text::TextObj) -> Vec<crate::spell::SpellError> {
    let lang = crate::spell::lang_of(&t.format.lang).to_string();
    SPELL_ERRORS.with(|c| {
        let mut c = c.borrow_mut();
        let key = (t.content.clone(), lang.clone());
        if let Some(e) = c.get(&key) {
            return e.clone();
        }
        let e = crate::spell::check(&t.content, &lang);
        if c.len() > 400 {
            c.clear();
        }
        c.insert(key, e.clone());
        e
    })
}

/// Text frames (Text ▸ Show Text Frames; red when text does not fit), the links between linked frames,
/// non-printing characters (Text ▸ Show Non-Printing Characters) and words the spell checker does not
/// know (Options ▸ Text ▸ Spelling ▸ Perform automatic spell checking).
fn draw_text_frames(painter: &Painter, d: &DocView, s: &Settings) {
    use vector_core::text::{self as vt, TextKind};
    let editing = d.text_edit.as_ref().map(|te| te.obj);
    let mut texts: Vec<(vector_core::ObjId, Affine, &vector_core::text::TextObj)> = vec![];
    for l in d.page_ref().layers.iter().filter(|l| l.visible) {
        for o in &l.objects {
            o.walk(&mut |x| {
                if let ObjKind::Text(t) = &x.kind {
                    texts.push((x.id, x.transform, t));
                }
            });
        }
    }
    let sa = screen_affine(d);
    let blue = Color32::from_rgb(0x1e, 0x88, 0xe5);
    let red = Color32::from_rgb(0xe0, 0x30, 0x30);
    for (id, xf, t) in &texts {
        let selected = d.selection.contains(id) || editing == Some(*id);
        let m = sa * *xf;
        let scale = m.determinant().abs().sqrt() as f32;
        let to = |p: Point| {
            let q = m * p;
            Pos2::new(q.x as f32, q.y as f32)
        };
        let lay = if s.show_nonprinting || s.show_text_frames || selected || s.spell_check { Some(cached_layout(*id, t)) } else { None };
        if let TextKind::Paragraph { width, height } = t.kind {
            if s.show_text_frames || selected {
                let over = t.link_next.is_none() && lay.as_ref().is_some_and(|l| l.fit_end < t.char_count());
                let bp = match &t.frame_shape {
                    Some(fs) => *xf * fs.to_bez(),
                    None => *xf * Rect::new(0.0, 0.0, width, height).to_path(0.1),
                };
                dashed_bez(painter, d, &bp, if over { red } else { Color32::from_gray(140) });
                // the tab at the bottom of the frame: ▼ when text does not fit, linked frames in blue
                if selected || over {
                    let c = to(Point::new(width / 2.0, height));
                    let r = egui::Rect::from_center_size(c, egui::vec2(11.0, 11.0));
                    let col = if over { red } else if t.link_next.is_some() || t.link_prev.is_some() { blue } else { Color32::from_gray(150) };
                    painter.rect_filled(r, 1.0, Color32::WHITE);
                    painter.rect_stroke(r, 1.0, Stroke::new(1.0, col), egui::StrokeKind::Inside);
                    if over {
                        painter.add(EShape::convex_polygon(vec![c + egui::vec2(-3.0, -2.0), c + egui::vec2(3.0, -2.0), c + egui::vec2(0.0, 2.5)], red, Stroke::NONE));
                    } else if t.link_next.is_some() {
                        painter.line_segment([c + egui::vec2(-3.0, 0.0), c + egui::vec2(3.0, 0.0)], Stroke::new(1.5, col));
                        painter.line_segment([c + egui::vec2(0.0, -3.0), c + egui::vec2(0.0, 3.0)], Stroke::new(1.5, col));
                    }
                }
            }
            // where the text goes next: a blue line to the next frame
            if s.show_text_links && selected {
                if let Some(next) = t.link_next.and_then(|n| d.doc.find(n)) {
                    let a = to(Point::new(width / 2.0, height));
                    let b = next.transform * Point::ZERO;
                    let b = sa * b;
                    painter.line_segment([a, Pos2::new(b.x as f32, b.y as f32)], Stroke::new(1.0, blue));
                }
                if let Some(prev) = t.link_prev.and_then(|n| d.doc.find(n)) {
                    if let ObjKind::Text(pt) = &prev.kind {
                        if let TextKind::Paragraph { width: pw, height: ph } = pt.kind {
                            let a = sa * (prev.transform * Point::new(pw / 2.0, ph));
                            painter.line_segment([Pos2::new(a.x as f32, a.y as f32), to(Point::ZERO)], Stroke::new(1.0, blue));
                        }
                    }
                }
            }
        }
        let Some(lay) = lay else { continue };
        let chars: Vec<char> = t.content.chars().collect();
        let visible = |i: usize| lay.carets.get(i).and_then(|c| lay.lines.get(c.2)).is_some_and(|l| !l.hidden);
        let width_of = |i: usize| match (lay.carets.get(i), lay.carets.get(i + 1)) {
            (Some(a), Some(b)) if a.2 == b.2 => (b.0 - a.0).abs(),
            _ => t.format_at(i).size * 0.25,
        };
        if s.show_nonprinting {
            for (i, ch) in chars.iter().enumerate() {
                if !visible(i) || t.on_path.is_some() {
                    continue;
                }
                let (x, base, _) = lay.carets[i];
                let size = t.format_at(i).size;
                let w = width_of(i);
                let mid = Point::new(x + w / 2.0, base - size * 0.3);
                match *ch {
                    ' ' => {
                        painter.circle_filled(to(mid), (1.2 * scale.clamp(0.6, 2.0)).max(1.0), blue);
                    }
                    vt::NBSP => {
                        painter.circle_stroke(to(mid), (size as f32 * 0.12 * scale).clamp(1.5, 6.0), Stroke::new(1.0, blue));
                    }
                    vt::EM_SPACE | vt::EN_SPACE | vt::QUARTER_EM_SPACE => {
                        let (a, b) = (to(Point::new(x + w * 0.15, base - size * 0.3)), to(Point::new(x + w * 0.85, base - size * 0.3)));
                        painter.line_segment([a, b], Stroke::new(1.0, blue));
                        painter.line_segment([a + egui::vec2(0.0, -2.0), a + egui::vec2(0.0, 2.0)], Stroke::new(1.0, blue));
                        painter.line_segment([b + egui::vec2(0.0, -2.0), b + egui::vec2(0.0, 2.0)], Stroke::new(1.0, blue));
                    }
                    '\t' => {
                        let (a, b) = (to(Point::new(x + w * 0.1, base - size * 0.3)), to(Point::new(x + w.max(size * 0.5) * 0.9, base - size * 0.3)));
                        painter.line_segment([a, b], Stroke::new(1.0, blue));
                        let dir = (b - a).normalized();
                        let n = egui::vec2(-dir.y, dir.x);
                        painter.line_segment([b, b - dir * 4.0 + n * 3.0], Stroke::new(1.0, blue));
                        painter.line_segment([b, b - dir * 4.0 - n * 3.0], Stroke::new(1.0, blue));
                    }
                    vt::COLUMN_BREAK => {
                        let a = to(Point::new(x + 1.0, base - size * 0.7));
                        let b = to(Point::new(x + 1.0, base));
                        painter.line_segment([a, b], Stroke::new(1.0, blue));
                        painter.line_segment([b, b + egui::vec2(-3.0, -4.0)], Stroke::new(1.0, blue));
                        painter.line_segment([b, b + egui::vec2(3.0, -4.0)], Stroke::new(1.0, blue));
                    }
                    vt::SOFT_HYPHEN => {
                        painter.text(to(Point::new(x, base)), egui::Align2::CENTER_BOTTOM, "¬", egui::FontId::proportional((size as f32 * 0.6 * scale).clamp(7.0, 60.0)), blue);
                    }
                    c if vt::is_para_end(c) => {
                        painter.text(to(Point::new(x + size * 0.05, base)), egui::Align2::LEFT_BOTTOM, "¶", egui::FontId::proportional((size as f32 * 0.8 * scale).clamp(7.0, 80.0)), blue);
                    }
                    _ => {}
                }
            }
        }
        // spelling: a red zigzag under the words that are not in the dictionary
        if s.spell_check && (s.spell_errors_all || selected) && t.char_count() > 0 && t.on_path.is_none() {
            for e in spell_errors(t).iter().filter(|e| !e.delete) {
                let mut i = e.start;
                while i < (e.start + e.len).min(chars.len()) {
                    let Some(&(x0, base, line)) = lay.carets.get(i) else { break };
                    let mut j = i;
                    while j + 1 < (e.start + e.len).min(chars.len()) && lay.carets.get(j + 1).is_some_and(|c| c.2 == line) {
                        j += 1;
                    }
                    if visible(i) {
                        let x1 = lay.carets.get(j + 1).filter(|c| c.2 == line).map(|c| c.0).unwrap_or(lay.carets[j].0 + width_of(j));
                        let y = base + t.format_at(i).size * 0.12;
                        let (a, b) = (to(Point::new(x0, y)), to(Point::new(x1, y)));
                        let len = (b - a).length();
                        let dir = (b - a) / len.max(1.0);
                        let n = egui::vec2(-dir.y, dir.x);
                        let steps = (len / 2.0).ceil().max(1.0) as usize;
                        let pts: Vec<Pos2> = (0..=steps).map(|k| a + dir * (k as f32 * len / steps as f32) + n * if k % 2 == 0 { -1.0 } else { 1.0 }).collect();
                        painter.add(EShape::line(pts, Stroke::new(1.0, red)));
                    }
                    i = j + 1;
                }
            }
        }
    }
}

fn draw_selection(painter: &Painter, d: &DocView, rotate: bool) {
    let Some(bb) = d.selection_bounds() else { return };
    let e = d.doc_len(6.0);
    let eb = bb.inflate(e, e);
    if rotate {
        for i in 0..8 {
            let c = d.to_screen(handle_point(eb, i));
            if i % 2 == 0 {
                // curved rotation arrow
                let out = match i {
                    0 => egui::vec2(-1.0, -1.0),
                    2 => egui::vec2(1.0, -1.0),
                    4 => egui::vec2(1.0, 1.0),
                    _ => egui::vec2(-1.0, 1.0),
                };
                let pts: Vec<Pos2> = (0..=8)
                    .map(|k| {
                        let a = std::f32::consts::FRAC_PI_2 * (k as f32 / 8.0) + out.y.atan2(out.x) - std::f32::consts::FRAC_PI_4;
                        c + egui::vec2(a.cos(), a.sin()) * 6.0
                    })
                    .collect();
                painter.add(EShape::line(pts.clone(), Stroke::new(3.0, Color32::WHITE)));
                painter.add(EShape::line(pts.clone(), Stroke::new(1.5, Color32::BLACK)));
                let tip = *pts.last().unwrap();
                painter.circle_filled(tip, 2.0, Color32::BLACK);
            } else {
                // double-headed skew arrow along the side
                let dir = if i == 1 || i == 5 { egui::vec2(1.0, 0.0) } else { egui::vec2(0.0, 1.0) };
                painter.line_segment([c - dir * 6.0, c + dir * 6.0], Stroke::new(3.0, Color32::WHITE));
                painter.line_segment([c - dir * 6.0, c + dir * 6.0], Stroke::new(1.5, Color32::BLACK));
                for sgn in [-1.0f32, 1.0] {
                    let tip = c + dir * 6.0 * sgn;
                    let n = egui::vec2(-dir.y, dir.x);
                    painter.add(EShape::convex_polygon(vec![tip + dir * 2.0 * sgn, tip - dir * 2.0 * sgn + n * 3.0, tip - dir * 2.0 * sgn - n * 3.0], Color32::BLACK, Stroke::NONE));
                }
            }
        }
        // rotation center
        let c = d.to_screen(bb.center());
        painter.circle_stroke(c, 4.0, Stroke::new(1.5, Color32::BLACK));
        painter.circle_filled(c, 1.5, Color32::BLACK);
    } else {
        for i in 0..8 {
            handle(painter, d.to_screen(handle_point(eb, i)), 7.0, true);
        }
        if d.selection.len() == 1 {
            let c = d.to_screen(bb.center());
            let st = Stroke::new(1.0, Color32::BLACK);
            painter.line_segment([c + egui::vec2(-3.0, -3.0), c + egui::vec2(3.0, 3.0)], st);
            painter.line_segment([c + egui::vec2(-3.0, 3.0), c + egui::vec2(3.0, -3.0)], st);
        }
    }
}

fn draw_nodes(painter: &Painter, d: &DocView, s: &Settings) {
    let ns = node_size(s);
    let main = Color32::from_rgb(s.node_main_color[0], s.node_main_color[1], s.node_main_color[2]);
    for id in &d.selection {
        let Some(o) = d.doc.find(*id) else { continue };
        let t = o.transform;
        match &o.kind {
            ObjKind::Path(np) => {
                stroke_bez(painter, d, &(t * np.to_bez()), Stroke::new(1.0, main));
                for (si, sp) in np.subpaths.iter().enumerate() {
                    for (ni, n) in sp.nodes.iter().enumerate() {
                        let selected = d.node_sel.contains(&(*id, si, ni));
                        let c = d.to_screen(t * n.p);
                        if selected {
                            for (cp, show) in [(n.c_in, n.c_in != n.p), (n.c_out, n.c_out != n.p && !n.line_out)] {
                                if show {
                                    let cs = d.to_screen(t * cp);
                                    painter.line_segment([c, cs], Stroke::new(1.0, main));
                                    painter.add(EShape::convex_polygon(
                                        vec![cs + egui::vec2(0.0, -3.5), cs + egui::vec2(3.5, 0.0), cs + egui::vec2(0.0, 3.5), cs + egui::vec2(-3.5, 0.0)],
                                        Color32::WHITE,
                                        Stroke::new(1.0, main),
                                    ));
                                }
                            }
                        }
                        let size = if ni == 0 { ns + 2.0 } else { ns };
                        let r = ERect::from_center_size(c, egui::vec2(size, size));
                        let fill = if selected { main } else { Color32::WHITE };
                        if s.node_types_colors && !selected {
                            let tint = match n.kind {
                                NodeKind::Cusp => Color32::WHITE,
                                NodeKind::Smooth => Color32::from_rgb(0xc8, 0xe6, 0xff),
                                NodeKind::Symmetric => Color32::from_rgb(0xff, 0xe0, 0xb0),
                            };
                            painter.rect_filled(r, 0.0, tint);
                        } else {
                            painter.rect_filled(r, 0.0, fill);
                        }
                        painter.rect_stroke(r, 0.0, Stroke::new(1.0, main), egui::StrokeKind::Middle);
                    }
                }
                if s.show_curve_direction {
                    for sp in &np.subpaths {
                        if sp.nodes.len() >= 2 {
                            let a = d.to_screen(t * sp.nodes[0].p);
                            let b = d.to_screen(t * sp.nodes[1].p);
                            let dir = (b - a).normalized();
                            let m = a + dir * 14.0;
                            let nrm = egui::vec2(-dir.y, dir.x);
                            painter.add(EShape::convex_polygon(vec![m + dir * 5.0, m + nrm * 3.0, m - nrm * 3.0], main, Stroke::NONE));
                        }
                    }
                }
            }
            ObjKind::Rect(r) => {
                for c in [Point::new(0.0, 0.0), Point::new(r.w, 0.0), Point::new(r.w, r.h), Point::new(0.0, r.h)] {
                    handle(painter, d.to_screen(t * c), ns, false);
                }
            }
            _ => {
                stroke_bez(painter, d, &o.world_path(), Stroke::new(1.0, main));
            }
        }
    }
}

fn draw_envelope(painter: &Painter, d: &DocView) {
    for id in &d.selection {
        let Some(o) = d.doc.find(*id) else { continue };
        let Some(env) = &o.effects.envelope else {
            draw_selection(painter, d, false);
            continue;
        };
        let bb = warp_bbox(o);
        let hp: Vec<Point> = (0..8).map(|i| envelope_handle(bb, env, i)).collect();
        let mut bp = BezPath::new();
        bp.move_to(hp[0]);
        for i in 1..8 {
            bp.line_to(hp[i]);
        }
        bp.close_path();
        dashed_bez(painter, d, &bp, Color32::from_rgb(0x1e, 0x90, 0xff));
        for p in hp {
            handle(painter, d.to_screen(p), 7.0, false);
        }
    }
}

fn draw_gradient_vector(painter: &Painter, d: &DocView, tool: ToolId) {
    draw_selection(painter, d, false);
    for id in &d.selection {
        let Some(o) = d.doc.find(*id) else { continue };
        let g = match (&o.style.fill, &o.style.transparency, tool) {
            (Fill::Gradient(g), _, ToolId::InteractiveFill) => g,
            (_, vector_core::style::Transparency::Gradient(g), ToolId::Transparency) => g,
            _ => continue,
        };
        let space = vector_render::Renderer::fill_space(o, &o.world_path());
        let a = d.to_screen(space * g.start);
        let b = d.to_screen(space * g.end);
        painter.line_segment([a, b], Stroke::new(3.0, Color32::WHITE));
        painter.line_segment([a, b], Stroke::new(1.0, Color32::BLACK));
        for (p, st) in [(a, g.stops.first()), (b, g.stops.last())] {
            let r = ERect::from_center_size(p, egui::vec2(9.0, 9.0));
            let c = st.map(|s| Color32::from_rgb(s.color.r, s.color.g, s.color.b)).unwrap_or(Color32::WHITE);
            painter.rect_filled(r, 0.0, c);
            painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Middle);
        }
    }
}

fn draw_mesh(painter: &Painter, d: &DocView, ts: &ToolState) {
    for id in &d.selection {
        let Some(o) = d.doc.find(*id) else { continue };
        let Fill::Mesh(m) = &o.style.fill else {
            draw_selection(painter, d, false);
            continue;
        };
        let space = vector_render::Renderer::fill_space(o, &o.world_path());
        let st = Stroke::new(1.0, Color32::from_rgb(0x1e, 0x90, 0xff));
        for c in 0..=m.cols {
            let u = c as f64 / m.cols.max(1) as f64;
            painter.line_segment([d.to_screen(space * Point::new(u, 0.0)), d.to_screen(space * Point::new(u, 1.0))], st);
        }
        for r in 0..=m.rows {
            let v = r as f64 / m.rows.max(1) as f64;
            painter.line_segment([d.to_screen(space * Point::new(0.0, v)), d.to_screen(space * Point::new(1.0, v))], st);
        }
        if let Some((r, c)) = ts.table_cell {
            let (u0, v0) = (c as f64 / m.cols.max(1) as f64, r as f64 / m.rows.max(1) as f64);
            let (u1, v1) = ((c + 1) as f64 / m.cols.max(1) as f64, (r + 1) as f64 / m.rows.max(1) as f64);
            let pts: Vec<Pos2> = [(u0, v0), (u1, v0), (u1, v1), (u0, v1)].iter().map(|(u, v)| d.to_screen(space * Point::new(*u, *v))).collect();
            painter.add(EShape::closed_line(pts, Stroke::new(2.0, Color32::from_rgb(0xff, 0x80, 0x00))));
        }
    }
}

/// Variable Outline tool: the width nodes of the selected objects.
fn draw_var_outline(painter: &Painter, d: &DocView, ts: &ToolState) {
    for id in &d.selection {
        let Some(o) = d.doc.find(*id) else { continue };
        let Some(vector_core::style::WidthProfile::Variable { nodes, .. }) = &o.style.outline.profile else { continue };
        let wp = o.world_path();
        for (i, w) in nodes.iter().enumerate() {
            let Some((p, n)) = super::interact::outline_point(&wp, w.t) else { continue };
            let (a, b) = (d.to_screen(p + n * w.side1), d.to_screen(p - n * w.side2));
            painter.line_segment([a, b], Stroke::new(1.0, Color32::from_rgb(0x1e, 0x90, 0xff)));
            let sel = ts.vo_node == Some(i);
            handle(painter, d.to_screen(p), 7.0, sel);
            painter.circle_stroke(a, 3.0, Stroke::new(1.0, Color32::from_rgb(0x1e, 0x90, 0xff)));
            painter.circle_stroke(b, 3.0, Stroke::new(1.0, Color32::from_rgb(0x1e, 0x90, 0xff)));
        }
    }
}

fn draw_anchors(painter: &Painter, d: &DocView, ts: &ToolState) {
    for id in &d.selection {
        let Some(o) = d.doc.find(*id) else { continue };
        match &o.kind {
            ObjKind::Connector(c) => {
                for e in [&c.start, &c.end] {
                    let p = d.to_screen(e.point);
                    painter.circle_filled(p, 4.0, if e.attached.is_some() { Color32::from_rgb(0xff, 0x40, 0x40) } else { Color32::WHITE });
                    painter.circle_stroke(p, 4.0, Stroke::new(1.0, Color32::BLACK));
                }
            }
            _ => {
                let b = d.doc.bounds_of(o);
                for (i, a) in o.effects.anchor_list().iter().enumerate() {
                    let p = d.to_screen(Point::new(b.x0 + a.pos.x * b.width(), b.y0 + a.pos.y * b.height()));
                    let r = ERect::from_center_size(p, egui::vec2(7.0, 7.0));
                    if ts.anchor_sel == Some((*id, i)) {
                        painter.rect_filled(r, 0.0, Color32::from_rgb(0xff, 0x40, 0x40));
                    }
                    painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_rgb(0xff, 0x40, 0x40)), egui::StrokeKind::Middle);
                    if let Some(deg) = a.dir {
                        let v = egui::vec2(deg.to_radians().cos() as f32, -deg.to_radians().sin() as f32) * 14.0;
                        painter.arrow(p, v, Stroke::new(1.0, Color32::from_rgb(0xff, 0x40, 0x40)));
                    }
                }
            }
        }
    }
}

fn draw_table_cell(painter: &Painter, d: &DocView, ts: &ToolState) {
    for id in &d.selection {
        let Some(o) = d.doc.find(*id) else { continue };
        if let ObjKind::Table(t) = &o.kind {
            let Some(g) = ts.table_range(t) else { continue };
            let r = t.range_rect(g);
            let pts: Vec<Pos2> = [(r.x0, r.y0), (r.x1, r.y0), (r.x1, r.y1), (r.x0, r.y1)].iter().map(|(x, y)| d.to_screen(o.transform * Point::new(*x, *y))).collect();
            painter.add(EShape::convex_polygon(pts, Color32::from_rgba_unmultiplied(0x1e, 0x90, 0xff, 48), Stroke::NONE));
            stroke_bez(painter, d, &(o.transform * r.to_path(0.1)), Stroke::new(2.0, Color32::from_rgb(0x1e, 0x90, 0xff)));
        }
    }
}

fn preview_obj(painter: &Painter, d: &DocView, o: &Object) {
    let bp = match &o.kind {
        ObjKind::Group(kids) => {
            let mut bp = BezPath::new();
            for k in kids {
                for el in k.world_path().elements() {
                    bp.push(*el);
                }
            }
            bp
        }
        ObjKind::Table(t) => {
            let mut bp = BezPath::new();
            for r in 0..t.rows {
                for c in 0..t.cols {
                    for el in (o.transform * t.cell_rect(r, c).to_path(0.1)).elements() {
                        bp.push(*el);
                    }
                }
            }
            bp
        }
        _ => o.world_path(),
    };
    stroke_bez(painter, d, &bp, Stroke::new(1.0, PREVIEW));
}

/// The marks of the comments on the page, at 50% opacity (notes opaque).
fn draw_comments(painter: &Painter, d: &DocView, resolved: bool, sel: Option<u64>) {
    let pid = d.doc.pages[d.page].id;
    for c in d.doc.comments.iter().filter(|c| c.page == pid && (resolved || !c.resolved)) {
        let mut pts = vec![c.pos];
        pts.extend(c.pts.iter().copied());
        draw_mark(painter, d, c.mark, &pts, c.color, c.width, c.fill, sel == Some(c.id));
    }
}

fn draw_mark(painter: &Painter, d: &DocView, mark: u8, pts: &[Point], color: vector_core::Color, width: f64, fill: Option<vector_core::Color>, selected: bool) {
    let half = |c: vector_core::Color| {
        let c = theme::to_c32(c);
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 128)
    };
    let sp: Vec<Pos2> = pts.iter().map(|p| d.to_screen(*p)).collect();
    let w = (width * d.zoom) as f32;
    let st = Stroke::new(w.max(1.0), half(color));
    match mark {
        0 => {
            // a speech bubble
            let p = sp[0];
            let r = ERect::from_min_size(p + egui::vec2(-2.0, -18.0), egui::vec2(20.0, 15.0));
            painter.rect_filled(r, 2.0, Color32::from_rgb(0xff, 0xd8, 0x3a));
            painter.add(EShape::convex_polygon(vec![pos2(r.left() + 3.0, r.bottom() - 1.0), pos2(r.left() + 9.0, r.bottom() - 1.0), p], Color32::from_rgb(0xff, 0xd8, 0x3a), Stroke::NONE));
            painter.rect_stroke(r, 2.0, Stroke::new(1.0, Color32::from_rgb(0x8a, 0x6d, 0x00)), egui::StrokeKind::Inside);
            for k in 0..2 {
                let y = r.top() + 5.0 + k as f32 * 4.0;
                painter.line_segment([pos2(r.left() + 4.0, y), pos2(r.right() - 4.0, y)], Stroke::new(1.0, Color32::from_rgb(0x8a, 0x6d, 0x00)));
            }
            if selected {
                painter.rect_stroke(r.expand(2.0), 2.0, Stroke::new(1.0, theme::SELECTION), egui::StrokeKind::Outside);
            }
            return;
        }
        1 if sp.len() >= 2 => {
            let (a, b) = (sp[0], sp[1]);
            let v = (b - a).normalized();
            let n = egui::vec2(-v.y, v.x);
            let hl = (w * 4.0).max(10.0);
            painter.line_segment([a, b - v * hl * 0.8], st);
            painter.add(EShape::convex_polygon(vec![b, b - v * hl + n * hl * 0.45, b - v * hl - n * hl * 0.45], half(color), Stroke::NONE));
        }
        2 | 3 => {
            painter.add(EShape::line(sp.clone(), st));
        }
        4 | 5 if sp.len() >= 2 => {
            let r = ERect::from_two_pos(sp[0], sp[1]);
            if mark == 4 {
                if let Some(f) = fill {
                    painter.rect_filled(r, 0.0, half(f));
                }
                painter.rect_stroke(r, 0.0, st, egui::StrokeKind::Middle);
            } else {
                painter.add(EShape::ellipse_filled(r.center(), r.size() / 2.0, fill.map(half).unwrap_or(Color32::TRANSPARENT)));
                painter.add(EShape::ellipse_stroke(r.center(), r.size() / 2.0, st));
            }
        }
        _ => {}
    }
    if selected && !sp.is_empty() {
        let r = sp.iter().fold(ERect::NOTHING, |r, p| r.union(ERect::from_center_size(*p, egui::Vec2::ZERO)));
        painter.rect_stroke(r.expand(w / 2.0 + 3.0), 0.0, Stroke::new(1.0, theme::SELECTION), egui::StrokeKind::Outside);
    }
}

fn draw_drag_preview(painter: &Painter, d: &DocView, ts: &ToolState, s: &Settings, tool: ToolId) {
    let cur = ts.cur;
    match &ts.drag {
        Drag::Markup { points } => {
            let m = ts.markup.unwrap_or(1);
            let pts = if m == 2 || m == 3 { points.clone() } else { vec![points[0], cur] };
            let (color, width) = if m == 2 { (vector_core::Color::rgb(0xff, 0xe6, 0x00), 12.0) } else { (vector_core::Color::rgb(0xe5, 0x2b, 0x2b), 1.5) };
            draw_mark(painter, d, m, &pts, color, width, None, false);
        }
        Drag::PatternArea { start } => {
            dashed_rect(painter, d.to_screen_rect(Rect::from_points(*start, cur)), Color32::from_rgb(0x3d, 0x8e, 0xf0));
        }
        Drag::Marquee { start } | Drag::NodeMarquee { start } => {
            if ts.moved {
                dashed_rect(painter, d.to_screen_rect(Rect::from_points(*start, ts.hover_doc.unwrap_or(cur))), Color32::from_gray(230));
            }
        }
        Drag::Lasso { points } => {
            let pts: Vec<Pos2> = points.iter().map(|p| d.to_screen(*p)).collect();
            painter.extend(EShape::dashed_line(&pts, Stroke::new(1.0, Color32::from_gray(230)), 4.0, 3.0));
        }
        Drag::Transform { kind, start, bb, ids, center } => {
            if ts.moved {
                let a = xform_affine(*kind, *start, cur, *bb, *center, ts.mods, s.constrain_angle);
                for id in ids {
                    if let Some(o) = d.doc.find(*id) {
                        let mut bp = o.world_path();
                        if bp.elements().is_empty() {
                            bp = d.doc.bounds_of(o).to_path(0.1);
                        }
                        stroke_bez(painter, d, &(a * bp), Stroke::new(1.0, PREVIEW));
                    }
                }
                let nb = a * bb.to_path(0.1);
                dashed_bez(painter, d, &nb, Color32::from_gray(200));
                if matches!(kind, Xform::Rotate) {
                    let c = d.to_screen(*center);
                    painter.circle_stroke(c, 4.0, Stroke::new(1.5, PREVIEW));
                }
            }
        }
        Drag::Create { start } => {
            let mods = ts.mods;
            match tool {
                ToolId::Zoom | ToolId::Crop => {
                    if ts.moved {
                        dashed_rect(painter, d.to_screen_rect(Rect::from_points(*start, cur)), Color32::from_gray(230));
                    }
                }
                ToolId::Knife => {
                    painter.line_segment([d.to_screen(*start), d.to_screen(ts.hover_doc.unwrap_or(cur))], Stroke::new(1.0, Color32::from_rgb(0xff, 0x40, 0x40)));
                }
                ToolId::Impact => {
                    if ts.moved {
                        let frame = ts.impact_outer.unwrap_or(Rect::from_points(*start, cur));
                        for bp in super::interact::impact_paths(&s.topts, frame, ts.impact_inner.as_ref(), 1) {
                            stroke_bez(painter, d, &bp, Stroke::new(1.0, PREVIEW));
                        }
                    }
                }
                ToolId::Text => {
                    if ts.moved && !ts.text_drag_select {
                        dashed_rect(painter, d.to_screen_rect(Rect::from_points(*start, cur)), Color32::from_gray(230));
                    }
                }
                ToolId::ThreePointCurve | ToolId::Rect3 | ToolId::Ellipse3 | ToolId::ParallelDim | ToolId::HvDim | ToolId::Callout => {
                    painter.line_segment([d.to_screen(*start), d.to_screen(cur)], Stroke::new(1.0, PREVIEW));
                }
                _ => {
                    if ts.moved {
                        if let Some(o) = creation_object(ts, s, tool, *start, cur, mods, 0) {
                            preview_obj(painter, d, &o);
                        }
                    }
                }
            }
        }
        Drag::Guide { vertical, .. } => {
            let p = ts.hover_doc.unwrap_or(cur);
            let cr = d.canvas_rect;
            let st = Stroke::new(1.0, theme::GUIDE);
            if *vertical {
                let x = d.to_screen(p).x;
                painter.line_segment([pos2(x, cr.top()), pos2(x, cr.bottom())], st);
            } else {
                let y = d.to_screen(p).y;
                painter.line_segment([pos2(cr.left(), y), pos2(cr.right(), y)], st);
            }
        }
        Drag::Stroke { points, .. } => {
            let pts: Vec<Pos2> = points.iter().map(|p| d.to_screen(*p)).collect();
            if pts.len() >= 2 {
                if tool == ToolId::Eraser {
                    let w = (s.eraser_thickness * d.zoom) as f32;
                    painter.add(EShape::line(pts, Stroke::new(w, Color32::from_rgba_unmultiplied(255, 255, 255, 70))));
                } else if matches!(tool, ToolId::ArtisticMedia | ToolId::PainterlyBrush) {
                    let w = ((ts.media_width * d.zoom) as f32).max(1.0);
                    painter.add(EShape::line(pts, Stroke::new(w, Color32::from_rgba_unmultiplied(0x1e, 0x90, 0xff, 120))));
                } else if tool != ToolId::Polyline {
                    painter.add(EShape::line(pts, Stroke::new(1.0, PREVIEW)));
                }
            }
        }
        Drag::Effect { start, obj } => {
            let a = d.to_screen(*start);
            let b = d.to_screen(cur);
            if ts.moved {
                if let Some(o) = d.doc.find(*obj) {
                    let dv = cur - *start;
                    if matches!(tool, ToolId::DropShadow | ToolId::InnerShadow | ToolId::BlockShadow | ToolId::Extrude) {
                        let bp = Affine::translate(dv) * o.world_path();
                        dashed_bez(painter, d, &bp, Color32::from_gray(200));
                    }
                }
                painter.line_segment([a, b], Stroke::new(3.0, Color32::WHITE));
                painter.line_segment([a, b], Stroke::new(1.0, Color32::BLACK));
                handle(painter, a, 7.0, false);
                handle(painter, b, 7.0, true);
            }
        }
        Drag::Blend { start, .. } => {
            painter.extend(EShape::dashed_line(&[d.to_screen(*start), d.to_screen(ts.hover_doc.unwrap_or(cur))], Stroke::new(1.0, PREVIEW), 4.0, 3.0));
        }
        Drag::Connector { start, .. } => {
            if ts.moved {
                let kind = match tool {
                    ToolId::RightAngleConnector => ConnectorKind::RightAngle,
                    ToolId::RoundedConnector => ConnectorKind::RoundedRightAngle,
                    _ => ConnectorKind::Straight,
                };
                let c = ConnectorObj {
                    kind,
                    start: ConnectorEnd { point: *start, attached: None, dir: None },
                    end: ConnectorEnd { point: cur, attached: None, dir: None },
                    corner_radius: if kind == ConnectorKind::RoundedRightAngle { 8.0 } else { 0.0 },
                };
                stroke_bez(painter, d, &vector_core::object::connector_path(&c), Stroke::new(1.0, PREVIEW));
            }
        }
        _ => {}
    }
}

/// Multi-click tools: curve being drawn, straight-line start, three-point stages, dimensions, crop area.
fn draw_in_progress(painter: &Painter, d: &DocView, ts: &ToolState, s: &Settings, tool: ToolId) {
    let cur = ts.hover_doc.map(|_| ts.cur).unwrap_or(ts.cur);
    if !ts.curve.is_empty() {
        let pts: Vec<Point> = ts.curve.iter().map(|n| n.p).collect();
        let np = if ts.bspline {
            super::interact::bspline_path(&pts, false)
        } else {
            NodePath { subpaths: vec![SubPath { nodes: ts.curve.clone(), closed: false }] }
        };
        if pts.len() >= 2 {
            stroke_bez(painter, d, &np.to_bez(), Stroke::new(1.0, PREVIEW));
        }
        if ts.bspline && pts.len() >= 2 {
            let sp: Vec<Pos2> = pts.iter().map(|p| d.to_screen(*p)).collect();
            painter.extend(EShape::dashed_line(&sp, Stroke::new(1.0, Color32::from_gray(150)), 3.0, 3.0));
        }
        // rubber band to the pointer (the Pen shows it with Preview mode)
        if (ts.drag.is_none() || matches!(ts.drag, Drag::Stroke { .. })) && !(tool == ToolId::Pen && !s.topts.pen_preview) {
            if let Some(last) = ts.curve.last() {
                let mut bp = BezPath::new();
                bp.move_to(last.p);
                if last.c_out != last.p && !last.line_out && !ts.bspline {
                    bp.curve_to(last.c_out, cur, cur);
                } else {
                    bp.line_to(cur);
                }
                dashed_bez(painter, d, &bp, PREVIEW);
            }
        }
        for (i, n) in ts.curve.iter().enumerate() {
            handle(painter, d.to_screen(n.p), if i == 0 { 7.0 } else { 5.0 }, i + 1 == ts.curve.len());
            if !ts.bspline && i + 1 == ts.curve.len() && n.c_out != n.p {
                let c = d.to_screen(n.p);
                for cp in [n.c_in, n.c_out] {
                    let cs = d.to_screen(cp);
                    painter.line_segment([c, cs], Stroke::new(1.0, PREVIEW));
                    painter.circle_filled(cs, 3.0, PREVIEW);
                }
            }
        }
    }
    if let Some(a) = ts.line_start {
        painter.line_segment([d.to_screen(a), d.to_screen(cur)], Stroke::new(1.0, PREVIEW));
        handle(painter, d.to_screen(a), 5.0, true);
    }
    if !ts.stage.is_empty() && ts.drag.is_none() {
        match tool {
            ToolId::Knife => {
                let mut sp: Vec<Pos2> = ts.stage.iter().map(|p| d.to_screen(*p)).collect();
                sp.push(d.to_screen(cur));
                painter.add(EShape::line(sp, Stroke::new(1.0, Color32::from_rgb(0xff, 0x40, 0x40))));
            }
            ToolId::ThreePointCurve | ToolId::Rect3 | ToolId::Ellipse3 if ts.stage.len() == 2 => {
                if let Some(o) = three_point_object(ts, tool, ts.stage[0], ts.stage[1], cur, 0) {
                    preview_obj(painter, d, &o);
                }
            }
            ToolId::ParallelDim | ToolId::HvDim | ToolId::SegmentDim | ToolId::Callout | ToolId::AngularDim => {
                let kind = match tool {
                    ToolId::HvDim => DimensionKind::HorizontalVertical,
                    ToolId::SegmentDim => DimensionKind::Segment,
                    ToolId::Callout => DimensionKind::Callout,
                    ToolId::AngularDim => DimensionKind::Angular,
                    _ => DimensionKind::Parallel,
                };
                let mut pts = ts.stage.clone();
                pts.push(cur);
                if kind == DimensionKind::Angular && pts.len() < 4 {
                    let sp: Vec<Pos2> = pts.iter().map(|p| d.to_screen(*p)).collect();
                    painter.add(EShape::line(sp, Stroke::new(1.0, PREVIEW)));
                } else {
                    let dim = DimensionObj {
                        kind,
                        points: pts,
                        text: String::new(),
                        precision: s.dim_precision as usize,
                        prefix: s.dim_prefix.clone(),
                        suffix: s.dim_suffix.clone(),
                        show_units: true,
                        font_size: 12.0,
                        unit: d.doc.settings.unit, ..Default::default()
                    };
                    stroke_bez(painter, d, &dimension_path(&dim), Stroke::new(1.0, PREVIEW));
                    let (label, at, _) = vector_core::object::dimension_label(&dim);
                    painter.text(d.to_screen(at), egui::Align2::CENTER_BOTTOM, label, egui::FontId::proportional(12.0), PREVIEW);
                }
            }
            _ => {}
        }
    }
    if let (Some(r), true) = (ts.crop_rect, ts.crop_angle.abs() > 1e-9) {
        // a rotated crop area
        let a = Affine::rotate_about(ts.crop_angle.to_radians(), r.center());
        let pts: Vec<Pos2> = [Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)].iter().map(|p| d.to_screen(a * *p)).collect();
        painter.add(EShape::closed_line(pts.clone(), Stroke::new(1.0, Color32::WHITE)));
        for p in pts {
            handle(painter, p, 7.0, true);
        }
    } else if let Some(r) = ts.crop_rect {
        let sr = d.to_screen_rect(r);
        let cr = d.canvas_rect;
        let shade = Color32::from_black_alpha(110);
        painter.rect_filled(ERect::from_min_max(cr.min, pos2(cr.max.x, sr.top())), 0.0, shade);
        painter.rect_filled(ERect::from_min_max(pos2(cr.min.x, sr.bottom()), cr.max), 0.0, shade);
        painter.rect_filled(ERect::from_min_max(pos2(cr.min.x, sr.top()), pos2(sr.left(), sr.bottom())), 0.0, shade);
        painter.rect_filled(ERect::from_min_max(pos2(sr.right(), sr.top()), pos2(cr.max.x, sr.bottom())), 0.0, shade);
        painter.rect_stroke(sr, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Middle);
        for i in 0..8 {
            handle(painter, d.to_screen(handle_point(r, i)), 7.0, true);
        }
    }
}

fn draw_text_caret(painter: &Painter, d: &DocView, ts: &ToolState, time: f64) {
    let blink_on = |since: f64| ((time - since) * 2.0).floor() as i64 % 2 == 0;
    if let Some((p, frame)) = ts.pending_text {
        if let Some(r) = frame {
            dashed_rect(painter, d.to_screen_rect(r), Color32::from_gray(200));
        }
        if blink_on(ts.text_last_edit.min(0.0)) {
            let size = ts.text_format.size;
            let (top, bottom) = match frame {
                Some(_) => (p, p + Vec2::new(0.0, size * 1.2)),
                None => (p - Vec2::new(0.0, size * 0.8), p + Vec2::new(0.0, size * 0.2)),
            };
            painter.line_segment([d.to_screen(top), d.to_screen(bottom)], Stroke::new(1.0, Color32::BLACK));
        }
    }
    let Some(te) = &d.text_edit else { return };
    let Some(o) = d.doc.find(te.obj) else { return };
    let ObjKind::Text(t) = &o.kind else { return };
    let lay = vector_core::text::layout(t);
    let tr = o.transform;
    // selection highlight
    let (a, b) = (te.caret.min(te.anchor), te.caret.max(te.anchor));
    if b > a {
        for i in a..b {
            let (Some(c0), Some(c1)) = (lay.carets.get(i), lay.carets.get(i + 1)) else { continue };
            if c0.2 != c1.2 {
                continue;
            }
            let size = t.format_at(i).size;
            let q = [Point::new(c0.0, c0.1 - size * 0.8), Point::new(c1.0, c0.1 - size * 0.8), Point::new(c1.0, c0.1 + size * 0.2), Point::new(c0.0, c0.1 + size * 0.2)];
            let pts: Vec<Pos2> = q.iter().map(|p| d.to_screen(tr * *p)).collect();
            painter.add(EShape::convex_polygon(pts, Color32::from_rgba_unmultiplied(0x1e, 0x90, 0xff, 90), Stroke::NONE));
        }
    }
    if blink_on(te.blink) {
        if let Some(c) = lay.carets.get(te.caret.min(lay.carets.len().saturating_sub(1))) {
            let size = t.format_at(te.caret.saturating_sub(1)).size;
            let top = tr * Point::new(c.0, c.1 - size * 0.85);
            let bottom = tr * Point::new(c.0, c.1 + size * 0.2);
            painter.line_segment([d.to_screen(top), d.to_screen(bottom)], Stroke::new(1.5, Color32::BLACK));
        }
    }
}
