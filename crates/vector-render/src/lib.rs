//! CPU renderer for Vector documents (tiny-skia). Used for the drawing window, export and print.

pub mod fx;

use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape, Vec2};
use std::collections::HashMap;
use std::sync::Arc;
use tiny_skia as sk;
use vector_core::color::Color;
use vector_core::document::{Document, ImageData};
use vector_core::geom;
use vector_core::object::*;
use vector_core::style::*;
use vector_core::text;

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub wireframe: bool,
    pub draft: bool,
    /// Only printable layers/objects.
    pub printing: bool,
    /// Draw desktop + page + page shadow.
    pub draw_page: bool,
    pub desktop: Color,
    /// Objects not to draw (e.g. those being dragged).
    pub hidden: Vec<ObjId>,
    /// Objects drawn at reduced opacity (inactive objects during focus mode), 0..1.
    pub dim_others: Option<(Vec<ObjId>, f32)>,
    pub show_powerclip_frames: bool,
    pub antialias: bool,
    /// Simulate overprints: objects set to overprint multiply with what is below them.
    pub overprint: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            wireframe: false,
            draft: false,
            printing: false,
            draw_page: true,
            desktop: Color::rgb(0x26, 0x26, 0x26),
            hidden: vec![],
            dim_others: None,
            show_powerclip_frames: true,
            antialias: true,
            overprint: false,
        }
    }
}

pub fn to_sk(a: Affine) -> sk::Transform {
    let c = a.as_coeffs();
    sk::Transform::from_row(c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32, c[4] as f32, c[5] as f32)
}

pub fn sk_color(c: Color) -> sk::Color {
    sk::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

pub fn to_sk_path(bp: &BezPath) -> Option<sk::Path> {
    let mut pb = sk::PathBuilder::new();
    let mut any = false;
    for el in bp.elements() {
        match *el {
            PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
            PathEl::LineTo(p) => {
                pb.line_to(p.x as f32, p.y as f32);
                any = true;
            }
            PathEl::QuadTo(c, p) => {
                pb.quad_to(c.x as f32, c.y as f32, p.x as f32, p.y as f32);
                any = true;
            }
            PathEl::CurveTo(a, b, p) => {
                pb.cubic_to(a.x as f32, a.y as f32, b.x as f32, b.y as f32, p.x as f32, p.y as f32);
                any = true;
            }
            PathEl::ClosePath => pb.close(),
        }
    }
    if !any {
        return None;
    }
    pb.finish()
}

fn paint_solid(c: Color, aa: bool) -> sk::Paint<'static> {
    let mut p = sk::Paint::default();
    p.set_color(sk_color(c));
    p.anti_alias = aa;
    p
}

/// Rendering context for one pass.
pub struct Renderer {
    images: HashMap<(u64, usize, u64), Arc<sk::Pixmap>>,
    geo_cache: HashMap<ObjId, (u64, Arc<Vec<(BezPath, Fill, Option<Color>)>>)>,
    pattern_cache: HashMap<String, Arc<sk::Pixmap>>,
}

impl Default for Renderer {
    fn default() -> Self {
        Renderer::new()
    }
}

struct Ctx<'a> {
    doc: &'a Document,
    view: Affine,
    opts: &'a RenderOptions,
}

impl Renderer {
    pub fn new() -> Renderer {
        Renderer { images: HashMap::new(), geo_cache: HashMap::new(), pattern_cache: HashMap::new() }
    }

    pub fn clear_caches(&mut self) {
        self.images.clear();
        self.geo_cache.clear();
        self.pattern_cache.clear();
    }

    /// Render a page. `view` maps document points to device pixels of `target`.
    pub fn render_page(&mut self, doc: &Document, page: usize, view: Affine, target: &mut sk::Pixmap, opts: &RenderOptions) {
        let Some(pg) = doc.pages.get(page) else { return };
        if opts.draw_page {
            target.fill(sk_color(opts.desktop));
            let pr = pg.rect();
            // page shadow
            let shadow = view.transform_rect_bbox(pr) + Vec2::new(3.0, 3.0);
            if let Some(r) = sk::Rect::from_ltrb(shadow.x0 as f32, shadow.y0 as f32, shadow.x1 as f32, shadow.y1 as f32) {
                target.fill_rect(r, &paint_solid(Color::rgba(0, 0, 0, 110), true), sk::Transform::identity(), None);
            }
            let bg = pg.background.unwrap_or(Color::WHITE);
            if let Some(path) = to_sk_path(&pr.to_path(0.1)) {
                target.fill_path(&path, &paint_solid(bg, true), sk::FillRule::Winding, to_sk(view), None);
            }
        } else if let Some(bg) = pg.background {
            let pr = pg.rect();
            if let Some(path) = to_sk_path(&pr.to_path(0.1)) {
                target.fill_path(&path, &paint_solid(bg, true), sk::FillRule::Winding, to_sk(view), None);
            }
        }
        let ctx = Ctx { doc, view, opts };
        for layer in &pg.layers {
            if !layer.visible || (opts.printing && !layer.printable) {
                continue;
            }
            for o in &layer.objects {
                self.render_object(&ctx, o, target);
            }
        }
    }

    /// Render a list of objects (e.g. symbol previews, drag ghosts).
    pub fn render_objects(&mut self, doc: &Document, objs: &[Object], view: Affine, target: &mut sk::Pixmap, opts: &RenderOptions) {
        let ctx = Ctx { doc, view, opts };
        for o in objs {
            self.render_object(&ctx, o, target);
        }
    }

    fn image_pixmap(&mut self, doc: &Document, id: u64, adjust: &ImageAdjust) -> Option<Arc<sk::Pixmap>> {
        let data: &ImageData = doc.images.get(&id)?;
        let key_adj = {
            let s = format!("{:?}", adjust);
            hash_str(&s)
        };
        let key = (id, Arc::as_ptr(&data.rgba) as usize, key_adj);
        if let Some(p) = self.images.get(&key) {
            return Some(p.clone());
        }
        let mut pm = sk::Pixmap::new(data.width.max(1), data.height.max(1))?;
        {
            let dst = pm.data_mut();
            let src = &data.rgba;
            let n = (data.width * data.height) as usize;
            for i in 0..n.min(src.len() / 4) {
                let a = src[i * 4 + 3] as u32;
                dst[i * 4] = ((src[i * 4] as u32 * a + 127) / 255) as u8;
                dst[i * 4 + 1] = ((src[i * 4 + 1] as u32 * a + 127) / 255) as u8;
                dst[i * 4 + 2] = ((src[i * 4 + 2] as u32 * a + 127) / 255) as u8;
                dst[i * 4 + 3] = a as u8;
            }
        }
        if adjust.brightness != 0.0 || adjust.contrast != 0.0 || adjust.saturation != 0.0 {
            fx::adjust(&mut pm, adjust.brightness as f32, adjust.contrast as f32, adjust.saturation as f32);
        }
        if adjust.grayscale {
            fx::grayscale(&mut pm);
        }
        if adjust.invert {
            fx::invert(&mut pm);
        }
        let arc = Arc::new(pm);
        self.images.insert(key, arc.clone());
        Some(arc)
    }

    fn render_object(&mut self, ctx: &Ctx, o: &Object, target: &mut sk::Pixmap) {
        if !o.visible || (ctx.opts.printing && !o.printable) || ctx.opts.hidden.contains(&o.id) {
            return;
        }
        // cull
        let vb = ctx.view.transform_rect_bbox(o.visual_bounds());
        let canvas = Rect::new(0.0, 0.0, target.width() as f64, target.height() as f64);
        if !matches!(o.kind, ObjKind::Symbol(_)) && vb.intersect(canvas).area() <= 0.0 && vb.width() > 0.0 && vb.height() > 0.0 {
            return;
        }
        let dimmed = ctx.opts.dim_others.as_ref().map(|(keep, a)| (!keep.contains(&o.id), *a)).filter(|x| x.0).map(|x| x.1);
        let overprints = ctx.opts.overprint && (o.style.overprint_fill || o.style.overprint_outline || (o.style.overprint_bitmap && matches!(o.kind, ObjKind::Image(_))));
        if o.hinting && !ctx.opts.printing {
            // Object Hinting: stretch the object so its edges fall on whole device pixels
            let b = ctx.view.transform_rect_bbox(o.bounds());
            if b.width() > 0.5 && b.height() > 0.5 {
                let (x0, y0) = (b.x0.round(), b.y0.round());
                let (x1, y1) = (b.x1.round().max(x0 + 1.0), b.y1.round().max(y0 + 1.0));
                let fix = Affine::translate((x0, y0)) * Affine::scale_non_uniform((x1 - x0) / b.width(), (y1 - y0) / b.height()) * Affine::translate((-b.x0, -b.y0));
                let sub = Ctx { doc: ctx.doc, view: fix * ctx.view, opts: ctx.opts };
                let mut plain = o.clone();
                plain.hinting = false;
                return self.render_object(&sub, &plain, target);
            }
        }
        if o.style.transparency_target != 0 && !matches!(o.style.transparency, Transparency::None) && !ctx.opts.wireframe && o.style.outline.is_visible() && !o.style.fill.is_none() {
            // the transparency applies to the fill or to the outline only: draw them apart
            let mut fill = o.clone();
            fill.style.outline = Outline::none();
            fill.style.transparency_target = 0;
            let mut line = o.clone();
            line.style.fill = Fill::None;
            line.style.transparency_target = 0;
            line.effects.drop_shadow = None;
            line.effects.inner_shadow = None;
            line.effects.block_shadow = None;
            line.effects.extrude = None;
            if o.style.transparency_target == 1 {
                line.style.transparency = Transparency::None;
            } else {
                fill.style.transparency = Transparency::None;
            }
            if o.style.outline.behind_fill {
                fill.effects.drop_shadow = None;
                fill.effects.inner_shadow = None;
                fill.effects.block_shadow = None;
                fill.effects.extrude = None;
                line.effects = o.effects.clone();
                self.render_object(ctx, &line, target);
                self.render_object(ctx, &fill, target);
            } else {
                self.render_object(ctx, &fill, target);
                self.render_object(ctx, &line, target);
            }
            return;
        }
        let needs_layer = !ctx.opts.wireframe
            && (overprints
                || !matches!(o.style.transparency, Transparency::None)
                || o.style.merge != MergeMode::Normal
                || !o.effects.bitmap.is_empty()
                || o.effects.bevel.is_some()
                || dimmed.is_some());
        if needs_layer {
            // render into an offscreen layer covering the object's device bounds
            let pad = 4.0;
            let r = vb.inflate(pad, pad).intersect(canvas);
            let (x0, y0) = (r.x0.floor() as i32, r.y0.floor() as i32);
            let w = (r.x1.ceil() as i32 - x0).max(1) as u32;
            let h = (r.y1.ceil() as i32 - y0).max(1) as u32;
            if w > 8192 || h > 8192 {
                return self.render_plain(ctx, o, target);
            }
            let Some(mut layer) = sk::Pixmap::new(w, h) else { return };
            let sub = Ctx { doc: ctx.doc, view: Affine::translate((-x0 as f64, -y0 as f64)) * ctx.view, opts: ctx.opts };
            self.render_plain(&sub, o, &mut layer);
            for e in &o.effects.bitmap {
                let zoom = view_scale(ctx.view) as f32;
                match e {
                    BitmapEffect::GaussianBlur { radius } => fx::blur(&mut layer, *radius as f32 * zoom),
                    BitmapEffect::Glow { radius, color } => {
                        let mut glow = layer.clone();
                        fx::blur(&mut glow, *radius as f32 * zoom);
                        fx::tint(&mut glow, [color.r, color.g, color.b], color.a as f32 / 255.0);
                        let mut out = sk::Pixmap::new(w, h).unwrap();
                        out.draw_pixmap(0, 0, glow.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
                        out.draw_pixmap(0, 0, layer.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
                        layer = out;
                    }
                    BitmapEffect::Emboss { depth } => fx::emboss(&mut layer, *depth as f32),
                    BitmapEffect::Noise { amount } => fx::noise(&mut layer, *amount as f32, o.id as u32),
                    BitmapEffect::Pixelate { size } => fx::pixelate(&mut layer, (*size as f32 * zoom).max(1.0) as u32),
                    BitmapEffect::Grayscale => fx::grayscale(&mut layer),
                    BitmapEffect::Sepia => fx::sepia(&mut layer),
                    BitmapEffect::Invert => fx::invert(&mut layer),
                }
            }
            if let Some(b) = &o.effects.bevel {
                let depth = if b.to_center { (layer.width().min(layer.height()) as f32) / 2.0 } else { (b.distance * view_scale(ctx.view)) as f32 };
                let col = |c: vector_core::Color| [c.r as f32, c.g as f32, c.b as f32];
                fx::bevel_lit(&mut layer, depth, &fx::BevelLight {
                    emboss: b.emboss,
                    shadow: b.shadow.map(col),
                    light: col(b.light),
                    intensity: (b.intensity / 100.0) as f32,
                    direction: b.direction as f32,
                    altitude: b.altitude as f32,
                });
            }
            let mut opacity = o.style.transparency.uniform_opacity();
            if let Some(d) = dimmed {
                opacity *= d;
            }
            let mut mask = None;
            if let Transparency::Gradient(g) = &o.style.transparency {
                mask = gradient_mask(o, g, &sub.view, w, h);
            }
            if let Transparency::Pattern { pattern, front, back } = &o.style.transparency {
                mask = self.pattern_mask(&sub, o, pattern, *front, *back, w, h);
            }
            let mut pp = sk::PixmapPaint::default();
            pp.opacity = opacity;
            pp.blend_mode = if overprints && o.style.merge == MergeMode::Normal { sk::BlendMode::Multiply } else { blend_mode(o.style.merge) };
            pp.quality = sk::FilterQuality::Nearest;
            if let Some(m) = &mask {
                // apply the gradient transparency mask to the layer itself
                layer.apply_mask(m);
            }
            target.draw_pixmap(x0, y0, layer.as_ref(), &pp, sk::Transform::identity(), None);
        } else {
            self.render_plain(ctx, o, target);
        }
    }

    fn render_plain(&mut self, ctx: &Ctx, o: &Object, target: &mut sk::Pixmap) {
        if ctx.opts.wireframe {
            return self.render_wire(ctx, o, target);
        }
        if let Some(l) = &o.effects.lens {
            // a lens shows what is under it, changed; its own fill is not drawn
            return self.render_lens(ctx, o, l, target);
        }
        // shadows / extrusions behind the object
        if let Some(s) = &o.effects.drop_shadow {
            self.render_drop_shadow(ctx, o, s, target);
        }
        if let Some(b) = &o.effects.block_shadow {
            self.render_block_shadow(ctx, o, b, target);
        }
        if let Some(e) = &o.effects.extrude {
            if !matches!(e.kind, 1 | 3 | 5) {
                self.render_extrude(ctx, o, e, target);
            }
        }
        if let Some(c) = &o.effects.contour {
            if c.dir == ContourDir::Outside {
                self.render_contour(ctx, o, c, target);
            }
        }
        self.render_body(ctx, o, target);
        if let Some(e) = &o.effects.extrude {
            if matches!(e.kind, 1 | 3 | 5) && !e.bevel_only {
                // front extrusions come toward the viewer: drawn over the object, capped with its fill
                self.render_extrude(ctx, o, e, target);
                let cap = extrude_back(o.world_path().bounding_box(), e) * o.world_path();
                let c = o.style.fill.main_color().unwrap_or(e.color);
                self.fill_path_simple(&cap, c, ctx.view, target, ctx.opts.antialias);
            }
            if e.bevel.is_some() && !e.bevel_only {
                self.render_bevel(ctx, o, &o.world_path(), e, target);
            }
        }
    }

    /// The object itself, without the effects drawn behind or in front of it.
    fn render_body(&mut self, ctx: &Ctx, o: &Object, target: &mut sk::Pixmap) {
        let aa = ctx.opts.antialias;
        let view = ctx.view;
        match &o.kind {
            ObjKind::Group(children) => {
                for c in children {
                    self.render_object(ctx, c, target);
                }
            }
            ObjKind::PowerClip { frame, contents } => {
                // frame fill
                let mut f2 = (**frame).clone();
                let outline = std::mem::replace(&mut f2.style.outline, Outline::none());
                self.render_object(ctx, &f2, target);
                // contents clipped
                let fp = frame.world_path();
                if let Some(path) = to_sk_path(&fp) {
                    let mut mask = sk::Mask::new(target.width(), target.height()).unwrap();
                    mask.fill_path(&path, sk::FillRule::Winding, aa, to_sk(view));
                    let mut layer = sk::Pixmap::new(target.width(), target.height()).unwrap();
                    for c in contents {
                        self.render_object(ctx, c, &mut layer);
                    }
                    layer.apply_mask(&mask);
                    target.draw_pixmap(0, 0, layer.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
                    if contents.is_empty() && ctx.opts.show_powerclip_frames && !ctx.opts.printing {
                        // empty frame: diagonal lines
                        let b = fp.bounding_box();
                        let mut x = BezPath::new();
                        x.move_to((b.x0, b.y0));
                        x.line_to((b.x1, b.y1));
                        x.move_to((b.x1, b.y0));
                        x.line_to((b.x0, b.y1));
                        if let Some(xp) = to_sk_path(&x) {
                            let mut st = sk::Stroke::default();
                            st.width = 0.0;
                            target.stroke_path(&xp, &paint_solid(Color::rgb(128, 128, 128), aa), &st, to_sk(view), Some(&mask));
                        }
                    }
                }
                f2.style.outline = outline;
                f2.style.fill = Fill::None;
                self.render_object(ctx, &f2, target);
            }
            ObjKind::Blend(b) => {
                self.render_object(ctx, &b.start, target);
                for step in vector_core::object::blend_steps(b, || 0) {
                    self.render_object(ctx, &step, target);
                }
                self.render_object(ctx, &b.end, target);
            }
            ObjKind::Symbol(sid) => {
                if let Some(def) = ctx.doc.symbols.iter().find(|s| s.id == *sid) {
                    for c in &def.objects {
                        let mut c2 = c.clone();
                        c2.apply_transform(o.transform);
                        self.render_object(ctx, &c2, target);
                    }
                }
            }
            ObjKind::Image(img) => self.render_image(ctx, o, img, target),
            ObjKind::Text(t) => self.render_text(ctx, o, t, target),
            ObjKind::Table(t) => self.render_table(ctx, o, t, target),
            ObjKind::Dimension(d) => {
                let path = o.world_path();
                let mut st = o.style.clone();
                if st.outline.color.is_none() {
                    st.outline.color = Some(Color::BLACK);
                }
                st.fill = Fill::None;
                self.stroke(ctx, &path, &st.outline, o, target, None);
                let (label, pos, ang) = dimension_label(d);
                let mut tobj = text::TextObj::artistic(&label, text::CharFormat { size: d.font_size, ..Default::default() });
                tobj.align = text::TextAlign::Center;
                let lay = text::layout(&tobj);
                let tr = Affine::translate(pos.to_vec2()) * Affine::rotate(normalize_text_angle(ang)) * Affine::translate((0.0, -d.font_size * 0.3));
                let tp = tr * lay.to_path();
                let c = st.outline.color.unwrap_or(Color::BLACK);
                self.fill_path_simple(&tp, c, view, target, aa);
                if d.kind == vector_core::object::DimensionKind::Callout && d.callout_shape > 0 && !tp.elements().is_empty() {
                    let g = d.callout_gap.max(0.0);
                    let r = tp.bounding_box().inflate(g, g);
                    let shape = match d.callout_shape {
                        1 => r.to_path(0.1),
                        2 => kurbo::RoundedRect::from_rect(r, (r.height() / 4.0).min(8.0)).to_path(0.1),
                        _ => kurbo::Ellipse::from_rect(r.inflate(r.width() * 0.2, r.height() * 0.3)).to_path(0.1),
                    };
                    self.stroke(ctx, &shape, &st.outline, o, target, None);
                }
            }
            _ => {
                let path = o.world_path();
                if o.style.outline.behind_fill {
                    self.stroke(ctx, &path, &o.style.outline, o, target, None);
                    self.fill(ctx, &path, &o.style.fill, o, target);
                } else {
                    self.fill(ctx, &path, &o.style.fill, o, target);
                    if let Some(c) = &o.effects.contour {
                        if c.dir != ContourDir::Outside {
                            self.render_contour(ctx, o, c, target);
                        }
                    }
                    if let Some(s) = &o.effects.inner_shadow {
                        self.render_inner_shadow(ctx, o, &path, s, target);
                    }
                    self.stroke(ctx, &path, &o.style.outline, o, target, None);
                }
            }
        }
    }

    fn render_wire(&mut self, ctx: &Ctx, o: &Object, target: &mut sk::Pixmap) {
        let mut st = sk::Stroke::default();
        st.width = 0.0;
        let paint = paint_solid(Color::BLACK, true);
        match &o.kind {
            ObjKind::Group(c) => {
                for x in c {
                    self.render_wire(ctx, x, target);
                }
            }
            ObjKind::PowerClip { frame, contents } => {
                self.render_wire(ctx, frame, target);
                for x in contents {
                    self.render_wire(ctx, x, target);
                }
            }
            ObjKind::Blend(b) => {
                self.render_wire(ctx, &b.start, target);
                self.render_wire(ctx, &b.end, target);
            }
            _ => {
                let p = if matches!(o.kind, ObjKind::Image(_) | ObjKind::Table(_)) {
                    o.transform * o.local_path()
                } else {
                    o.world_path()
                };
                if let Some(sp) = to_sk_path(&p) {
                    target.stroke_path(&sp, &paint, &st, to_sk(ctx.view), None);
                }
            }
        }
    }

    fn fill_path_simple(&self, path: &BezPath, c: Color, view: Affine, target: &mut sk::Pixmap, aa: bool) {
        if let Some(sp) = to_sk_path(path) {
            target.fill_path(&sp, &paint_solid(c, aa), sk::FillRule::Winding, to_sk(view), None);
        }
    }

    /// Unit-square → document mapping for gradient/pattern coordinates of an object.
    pub fn fill_space(o: &Object, path: &BezPath) -> Affine {
        let (local_bb, xf) = match &o.kind {
            ObjKind::Path(_) | ObjKind::Dimension(_) | ObjKind::Connector(_) => (path.bounding_box(), Affine::IDENTITY),
            ObjKind::Text(t) => (text::layout(t).bounds, o.transform),
            _ => {
                if o.effects.warps_geometry() {
                    (path.bounding_box(), Affine::IDENTITY)
                } else {
                    (o.local_path().bounding_box(), o.transform)
                }
            }
        };
        let w = local_bb.width().max(1e-6);
        let h = local_bb.height().max(1e-6);
        xf * Affine::translate((local_bb.x0, local_bb.y0)) * Affine::scale_non_uniform(w, h)
    }

    fn fill(&mut self, ctx: &Ctx, path: &BezPath, fill: &Fill, o: &Object, target: &mut sk::Pixmap) {
        let aa = ctx.opts.antialias;
        if !o.is_closed_shape() && !matches!(o.kind, ObjKind::Path(_)) {
            return;
        }
        let Some(sp) = to_sk_path(path) else { return };
        let rule = if o.style.even_odd { sk::FillRule::EvenOdd } else { sk::FillRule::Winding };
        let view = to_sk(ctx.view);
        match fill {
            Fill::None => {}
            Fill::Solid(c) => target.fill_path(&sp, &paint_solid(*c, aa), rule, view, None),
            Fill::Gradient(g) => {
                let space = Self::fill_space(o, path);
                if g.steps > 0 || g.kind == GradientKind::Rectangular || g.accel != 0.0 || g.blend != 0 || g.smooth {
                    let g2 = g.clone();
                    self.fill_per_pixel(ctx, &sp, rule, space, target, move |u, v| gradient_color(&g2, u, v));
                    return;
                }
                if let Some(shader) = gradient_shader(g, space) {
                    let mut p = sk::Paint::default();
                    p.shader = shader;
                    p.anti_alias = aa;
                    target.fill_path(&sp, &p, rule, view, None);
                }
            }
            Fill::Pattern(pf) => {
                let (tile, tf) = self.pattern_paint(ctx, o, path, pf);
                let mut p = sk::Paint::default();
                p.shader = sk::Pattern::new(tile.as_ref().as_ref(), sk::SpreadMode::Repeat, sk::FilterQuality::Bilinear, 1.0, to_sk(tf));
                p.anti_alias = aa;
                target.fill_path(&sp, &p, rule, view, None);
            }
            Fill::Mesh(m) => {
                let space = Self::fill_space(o, path);
                let m2 = m.clone();
                self.fill_per_pixel(ctx, &sp, rule, space, target, move |u, v| m2.color_at(u, v));
            }
        }
    }

    fn fill_per_pixel(&mut self, ctx: &Ctx, sp: &sk::Path, rule: sk::FillRule, space: Affine, target: &mut sk::Pixmap, f: impl Fn(f64, f64) -> Color) {
        let view = to_sk(ctx.view);
        let Some(dev_path) = sp.clone().transform(view) else { return };
        let b = dev_path.bounds();
        let canvas = Rect::new(0.0, 0.0, target.width() as f64, target.height() as f64);
        let r = Rect::new(b.left() as f64, b.top() as f64, b.right() as f64, b.bottom() as f64).intersect(canvas);
        if r.width() < 1.0 || r.height() < 1.0 {
            return;
        }
        let (x0, y0) = (r.x0.floor() as i32, r.y0.floor() as i32);
        let w = (r.x1.ceil() as i32 - x0).max(1) as u32;
        let h = (r.y1.ceil() as i32 - y0).max(1) as u32;
        let Some(mut layer) = sk::Pixmap::new(w, h) else { return };
        let inv = (ctx.view * space).inverse();
        {
            let data = layer.data_mut();
            for yy in 0..h {
                for xx in 0..w {
                    let dp = Point::new(x0 as f64 + xx as f64 + 0.5, y0 as f64 + yy as f64 + 0.5);
                    let uv = inv * dp;
                    let c = f(uv.x, uv.y);
                    let i = ((yy * w + xx) * 4) as usize;
                    let a = c.a as u32;
                    data[i] = ((c.r as u32 * a + 127) / 255) as u8;
                    data[i + 1] = ((c.g as u32 * a + 127) / 255) as u8;
                    data[i + 2] = ((c.b as u32 * a + 127) / 255) as u8;
                    data[i + 3] = c.a;
                }
            }
        }
        let mut mask = sk::Mask::new(w, h).unwrap();
        mask.fill_path(sp, rule, ctx.opts.antialias, sk::Transform::from_translate(-x0 as f32, -y0 as f32).pre_concat(view));
        target.draw_pixmap(x0, y0, layer.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), Some(&mask_offset(&mask, target, x0, y0)));
    }

    /// The tile of a pattern fill and its transform (object space).
    fn pattern_paint(&mut self, ctx: &Ctx, o: &Object, path: &BezPath, pf: &PatternFill) -> (Arc<sk::Pixmap>, Affine) {
        let zoom = view_scale(ctx.view);
        let tile_px = (pf.tile * zoom).clamp(2.0, 1024.0);
        let img_pm = pf.image.and_then(|id| self.image_pixmap(ctx.doc, id, &ImageAdjust::default()));
        let tile = match img_pm {
            Some(p) => p,
            None => self.pattern_tile(pf, 64),
        };
        let tw = tile.width() as f64;
        let tile = if pf.mirror != 0 { mirror_tile(&tile, pf.mirror) } else { tile };
        let scale = tile_px / tw / zoom;
        let space = if pf.transform_with_object {
            let s = Self::fill_space(o, path);
            let origin = s * Point::ZERO;
            Affine::translate(origin.to_vec2()) * Affine::rotate(-pf.angle.to_radians())
        } else {
            Affine::rotate(-pf.angle.to_radians())
        };
        (tile, space * Affine::scale(scale))
    }

    /// The opacity of a pattern transparency: the pattern's dark parts (its foreground) get the
    /// foreground transparency, its light parts the background one.
    #[allow(clippy::too_many_arguments)]
    fn pattern_mask(&mut self, ctx: &Ctx, o: &Object, pf: &PatternFill, front: f64, back: f64, w: u32, h: u32) -> Option<sk::Mask> {
        let mut pm = sk::Pixmap::new(w, h)?;
        let path = o.world_path();
        let bw = PatternFill { fg: Color::BLACK, bg: Color::WHITE, ..pf.clone() };
        let (tile, tf) = self.pattern_paint(ctx, o, &path, &bw);
        let mut p = sk::Paint::default();
        p.shader = sk::Pattern::new(tile.as_ref().as_ref(), sk::SpreadMode::Repeat, sk::FilterQuality::Bilinear, 1.0, to_sk(ctx.view * tf));
        pm.fill_rect(sk::Rect::from_xywh(0.0, 0.0, w as f32, h as f32)?, &p, sk::Transform::identity(), None);
        let mut m = sk::Mask::new(w, h)?;
        let data = m.data_mut();
        let (fo, bo) = (1.0 - front / 100.0, 1.0 - back / 100.0);
        for (i, p) in pm.pixels().iter().enumerate() {
            let c = p.demultiply();
            let lum = (0.299 * c.red() as f64 + 0.587 * c.green() as f64 + 0.114 * c.blue() as f64) / 255.0;
            let a = fo + (bo - fo) * lum;
            data[i] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        Some(m)
    }

    fn pattern_tile(&mut self, pf: &PatternFill, size: u32) -> Arc<sk::Pixmap> {
        let key = format!("{:?}{:?}{:?}{}", pf.kind, pf.fg, pf.bg, size);
        if let Some(t) = self.pattern_cache.get(&key) {
            return t.clone();
        }
        let s = size as f32;
        let mut pm = sk::Pixmap::new(size, size).unwrap();
        pm.fill(sk_color(pf.bg));
        let fg = paint_solid(pf.fg, true);
        let mut pb = sk::PathBuilder::new();
        let mut stroke: Option<f32> = None;
        match pf.kind {
            PatternKind::Checker => {
                pb.push_rect(sk::Rect::from_xywh(0.0, 0.0, s / 2.0, s / 2.0).unwrap());
                pb.push_rect(sk::Rect::from_xywh(s / 2.0, s / 2.0, s / 2.0, s / 2.0).unwrap());
            }
            PatternKind::HorizontalLines => {
                pb.push_rect(sk::Rect::from_xywh(0.0, s * 0.375, s, s * 0.25).unwrap());
            }
            PatternKind::VerticalLines => {
                pb.push_rect(sk::Rect::from_xywh(s * 0.375, 0.0, s * 0.25, s).unwrap());
            }
            PatternKind::DiagonalLines => {
                pb.move_to(-s * 0.5, s * 0.5);
                pb.line_to(s * 0.5, -s * 0.5);
                pb.move_to(0.0, s);
                pb.line_to(s, 0.0);
                pb.move_to(s * 0.5, s * 1.5);
                pb.line_to(s * 1.5, s * 0.5);
                stroke = Some(s * 0.18);
            }
            PatternKind::CrossHatch => {
                pb.move_to(0.0, s);
                pb.line_to(s, 0.0);
                pb.move_to(0.0, 0.0);
                pb.line_to(s, s);
                stroke = Some(s * 0.12);
            }
            PatternKind::Dots => {
                pb.push_circle(s / 2.0, s / 2.0, s * 0.22);
            }
            PatternKind::Bricks => {
                stroke = Some(s * 0.06);
                pb.move_to(0.0, 0.0);
                pb.line_to(s, 0.0);
                pb.move_to(0.0, s / 2.0);
                pb.line_to(s, s / 2.0);
                pb.move_to(s / 2.0, 0.0);
                pb.line_to(s / 2.0, s / 2.0);
                pb.move_to(0.0, s / 2.0);
                pb.line_to(0.0, s);
            }
            PatternKind::Waves => {
                stroke = Some(s * 0.1);
                pb.move_to(0.0, s / 2.0);
                pb.cubic_to(s / 4.0, s * 0.2, s / 4.0, s * 0.2, s / 2.0, s / 2.0);
                pb.cubic_to(s * 0.75, s * 0.8, s * 0.75, s * 0.8, s, s / 2.0);
            }
            PatternKind::Grid => {
                stroke = Some(s * 0.08);
                pb.move_to(0.0, 0.0);
                pb.line_to(s, 0.0);
                pb.move_to(0.0, 0.0);
                pb.line_to(0.0, s);
            }
            PatternKind::Diamonds => {
                pb.move_to(s / 2.0, 0.0);
                pb.line_to(s, s / 2.0);
                pb.line_to(s / 2.0, s);
                pb.line_to(0.0, s / 2.0);
                pb.close();
            }
        }
        if let Some(path) = pb.finish() {
            if let Some(w) = stroke {
                let mut st = sk::Stroke::default();
                st.width = w;
                pm.stroke_path(&path, &fg, &st, sk::Transform::identity(), None);
            } else {
                pm.fill_path(&path, &fg, sk::FillRule::Winding, sk::Transform::identity(), None);
            }
        }
        let a = Arc::new(pm);
        self.pattern_cache.insert(key, a.clone());
        a
    }

    fn stroke(&mut self, ctx: &Ctx, path: &BezPath, ol: &Outline, o: &Object, target: &mut sk::Pixmap, mask: Option<&sk::Mask>) {
        let Some(color) = ol.color else { return };
        let aa = ctx.opts.antialias;
        let view = to_sk(ctx.view);
        // width profiles (artistic media / painterly)
        if let Some(profile) = &ol.profile {
            match profile {
                WidthProfile::Painterly { bristles, jitter, seed, opts } => {
                    painterly(path, ol.width.max(1.0), color, *bristles, *jitter, *seed, opts, ctx.view, target, aa);
                }
                _ => {
                    let shape = variable_width_outline(path, ol.width.max(0.1), profile);
                    if let Some(sp) = to_sk_path(&shape) {
                        target.fill_path(&sp, &paint_solid(color, aa), sk::FillRule::Winding, view, mask);
                    }
                }
            }
            return;
        }
        // calligraphy: a stretched, turned nib is a round pen in a stretched space
        let nib = if (ol.stretch - 100.0).abs() > 0.01 && ol.stretch > 0.0 { Some(Affine::rotate(-ol.nib_angle.to_radians()) * Affine::scale_non_uniform(1.0, ol.stretch / 100.0)) } else { None };
        let view = match nib {
            Some(t) => to_sk(ctx.view * t),
            None => view,
        };
        let nib_path;
        let path = match nib {
            Some(t) => {
                nib_path = t.inverse() * path.clone();
                &nib_path
            }
            None => path,
        };
        let Some(sp) = to_sk_path(path) else { return };
        let mut st = sk::Stroke::default();
        st.width = ol.width as f32;
        st.miter_limit = ol.miter_limit as f32;
        st.line_join = match ol.join {
            LineJoin::Miter => sk::LineJoin::Miter,
            LineJoin::Round => sk::LineJoin::Round,
            LineJoin::Bevel => sk::LineJoin::Bevel,
        };
        st.line_cap = match ol.cap {
            LineCap::Butt => sk::LineCap::Butt,
            LineCap::Round => sk::LineCap::Round,
            LineCap::Square => sk::LineCap::Square,
        };
        if !ol.dash.is_empty() {
            let w = ol.width.max(0.5) as f32;
            let mut arr: Vec<f32> = ol.dash.iter().map(|d| (*d as f32 * w).max(0.1)).collect();
            if arr.len() % 2 == 1 {
                arr.extend(arr.clone());
            }
            if ol.dash_mode == 1 {
                // aligned dashes: stretch the pattern so a whole dash ends the path
                let len: f64 = path.segments().map(|s| kurbo::ParamCurveArclen::arclen(&s, 0.05)).sum();
                let period: f32 = arr.iter().sum();
                if len > 0.0 && period > 0.0 {
                    let n = ((len as f32 - arr[0]) / period).round().max(1.0);
                    let k = len as f32 / (n * period + arr[0]);
                    for a in arr.iter_mut() {
                        *a *= k;
                    }
                }
            }
            st.dash = sk::StrokeDash::new(arr, 0.0);
        }
        let paint = paint_solid(color, aa);
        let closed = o.is_closed_shape();
        match ol.position {
            OutlinePosition::Center => target.stroke_path(&sp, &paint, &st, view, mask),
            OutlinePosition::Inside | OutlinePosition::Outside if closed => {
                st.width *= 2.0;
                let mut m = sk::Mask::new(target.width(), target.height()).unwrap();
                m.fill_path(&sp, if o.style.even_odd { sk::FillRule::EvenOdd } else { sk::FillRule::Winding }, aa, view);
                if ol.position == OutlinePosition::Outside {
                    m.invert();
                }
                target.stroke_path(&sp, &paint, &st, view, Some(&m));
            }
            _ => target.stroke_path(&sp, &paint, &st, view, mask),
        }
        // arrowheads
        if ol.start_arrow.is_some() || ol.end_arrow.is_some() {
            let w = ol.width.max(0.5);
            let polys = geom::flatten_polylines(path, 0.1);
            if let Some((pts, closed)) = polys.first() {
                if !closed && pts.len() >= 2 {
                    if let Some(a) = ol.start_arrow {
                        let tip = pts[0];
                        let from = pts[1.min(pts.len() - 1)];
                        draw_arrow(a, tip, tip - from, w, color, ctx.view, target, aa, &ctx.doc.arrowheads);
                    }
                }
            }
            if let Some((pts, closed)) = polys.last() {
                if !closed && pts.len() >= 2 {
                    if let Some(a) = ol.end_arrow {
                        let n = pts.len();
                        draw_arrow(a, pts[n - 1], pts[n - 1] - pts[n - 2], w, color, ctx.view, target, aa, &ctx.doc.arrowheads);
                    }
                }
            }
        }
    }

    fn render_image(&mut self, ctx: &Ctx, o: &Object, img: &ImageObj, target: &mut sk::Pixmap) {
        let Some(pm) = self.image_pixmap(ctx.doc, img.image, &img.adjust) else {
            // missing image: draw a placeholder box
            let p = o.transform * Rect::new(0.0, 0.0, img.w, img.h).to_path(0.1);
            if let Some(sp) = to_sk_path(&p) {
                let mut st = sk::Stroke::default();
                st.width = 0.0;
                target.stroke_path(&sp, &paint_solid(Color::rgb(200, 0, 0), true), &st, to_sk(ctx.view), None);
            }
            return;
        };
        let (iw, ih) = (pm.width() as f64, pm.height() as f64);
        let c = img.crop;
        let cw = (c[2] - c[0]).max(1e-6);
        let ch = (c[3] - c[1]).max(1e-6);
        // map image pixels to local: local = (px/iw - c0)/cw * w
        let img_to_local = Affine::scale_non_uniform(img.w / (cw * iw), img.h / (ch * ih)) * Affine::translate((-c[0] * iw, -c[1] * ih));
        let full = ctx.view * o.transform * img_to_local;
        let quality = if ctx.opts.draft { sk::FilterQuality::Nearest } else if view_scale(full) > 1.5 { sk::FilterQuality::Bicubic } else { sk::FilterQuality::Bilinear };
        let mut paint = sk::Paint::default();
        paint.shader = sk::Pattern::new(pm.as_ref().as_ref(), sk::SpreadMode::Pad, quality, 1.0, to_sk(o.transform * img_to_local));
        paint.anti_alias = ctx.opts.antialias;
        let local = Rect::new(0.0, 0.0, img.w, img.h).to_path(0.1);
        let p = o.apply_warps(o.transform * local);
        if let Some(sp) = to_sk_path(&p) {
            target.fill_path(&sp, &paint, sk::FillRule::Winding, to_sk(ctx.view), None);
            if o.style.outline.is_visible() {
                self.stroke(ctx, &p, &o.style.outline, o, target, None);
            }
        }
    }

    fn render_text(&mut self, ctx: &Ctx, o: &Object, t: &text::TextObj, target: &mut sk::Pixmap) {
        let layout = text::layout(t);
        let groups = layout.colored_paths();
        let base_fill = o.style.fill.clone();
        for (c, local) in groups {
            let wp = o.apply_warps(o.transform * local);
            let fill = match c {
                Some(c) => Fill::Solid(c),
                None => base_fill.clone(),
            };
            let mut o2 = o.clone();
            o2.style.even_odd = false;
            if o.style.outline.behind_fill {
                self.stroke(ctx, &wp, &o.style.outline, &o2, target, None);
            }
            self.fill_text(ctx, &wp, &fill, o, target);
            if !o.style.outline.behind_fill {
                self.stroke(ctx, &wp, &o.style.outline, &o2, target, None);
            }
        }
    }

    fn fill_text(&mut self, ctx: &Ctx, wp: &BezPath, fill: &Fill, o: &Object, target: &mut sk::Pixmap) {
        let mut o2 = o.clone();
        o2.style.even_odd = false;
        // text uses nonzero winding for glyph overlap correctness
        self.fill(ctx, wp, fill, &o2, target);
    }

    fn render_table(&mut self, ctx: &Ctx, o: &Object, t: &TableObj, target: &mut sk::Pixmap) {
        let aa = ctx.opts.antialias;
        let view = ctx.view * o.transform;
        // background
        if let Fill::Solid(c) = &o.style.fill {
            self.fill_path_simple(&Rect::new(0.0, 0.0, t.width(), t.height()).to_path(0.1), *c, view, target, aa);
        }
        for (r, c) in t.anchors() {
            {
                let cell = t.cell(r, c);
                let rc = t.cell_box(r, c);
                if let Some(f) = cell.fill {
                    self.fill_path_simple(&rc.to_path(0.1), f, view, target, aa);
                }
                if !cell.text.is_empty() {
                    let tr = t.text_rect(r, c);
                    let mut tobj = text::TextObj::paragraph(&cell.text, tr.width().max(1.0), tr.height().max(1.0), t.text_format.clone());
                    tobj.align = text::TextAlign::Left;
                    let lay = text::layout(&tobj);
                    let tp = Affine::translate((tr.x0, tr.y0)) * lay.to_path();
                    let col = t.text_format.color.unwrap_or(Color::BLACK);
                    let mut mask = sk::Mask::new(target.width(), target.height()).unwrap();
                    if let Some(rp) = to_sk_path(&rc.to_path(0.1)) {
                        mask.fill_path(&rp, sk::FillRule::Winding, false, to_sk(view));
                    }
                    if let Some(sp) = to_sk_path(&tp) {
                        target.fill_path(&sp, &paint_solid(col, aa), sk::FillRule::Winding, to_sk(view), Some(&mask));
                    }
                }
            }
        }
        // cell borders: one grid, or a box around each cell when they are separated
        let bp = match t.spacing {
            Some(_) => {
                let mut bp = BezPath::new();
                for (r, c) in t.anchors() {
                    bp.extend(t.cell_box(r, c).to_path(0.1));
                }
                bp
            }
            None => t.grid_path(),
        };
        if let Some(sp) = to_sk_path(&bp) {
            let mut st = sk::Stroke::default();
            st.width = t.border_width as f32;
            target.stroke_path(&sp, &paint_solid(t.border, aa), &st, to_sk(view), None);
        }
    }

    fn silhouette(&mut self, ctx: &Ctx, o: &Object, layer: &mut sk::Pixmap) {
        // object drawn in solid black (fill + outline)
        let mut o2 = o.clone();
        o2.effects = Effects::default();
        o2.style.transparency = Transparency::None;
        o2.style.merge = MergeMode::Normal;
        recolor(&mut o2, Color::BLACK);
        let sub = Ctx { doc: ctx.doc, view: ctx.view, opts: ctx.opts };
        self.render_plain(&sub, &o2, layer);
    }

    fn render_drop_shadow(&mut self, ctx: &Ctx, o: &Object, s: &DropShadow, target: &mut sk::Pixmap) {
        let zoom = view_scale(ctx.view);
        let feather = s.feather * zoom;
        // a perspective shadow lies on the ground behind the object: flattened and slanted
        let persp = s.perspective.then(|| perspective_shadow(o.bounds(), s));
        let shadowed;
        let o = match persp {
            Some(m) => {
                let mut c = o.clone();
                c.apply_transform(m);
                shadowed = c;
                &shadowed
            }
            None => o,
        };
        let vb = ctx.view.transform_rect_bbox(o.bounds()).inflate(feather * 2.0 + 4.0, feather * 2.0 + 4.0);
        let off = ctx.view.transform_rect_bbox(Rect::from_points(Point::ZERO, Point::new(s.offset.x, s.offset.y)));
        let _ = off;
        let lin = linear_part(ctx.view);
        let dv = lin * Point::new(s.offset.x, s.offset.y);
        let canvas = Rect::new(0.0, 0.0, target.width() as f64, target.height() as f64);
        let r = (vb + dv.to_vec2()).intersect(canvas.inflate(feather * 2.0, feather * 2.0));
        if r.width() < 1.0 || r.height() < 1.0 {
            return;
        }
        let (x0, y0) = (r.x0.floor() as i32, r.y0.floor() as i32);
        let w = ((r.x1.ceil() as i32 - x0).max(1) as u32).min(8192);
        let h = ((r.y1.ceil() as i32 - y0).max(1) as u32).min(8192);
        let Some(mut layer) = sk::Pixmap::new(w, h) else { return };
        let sub = Ctx { doc: ctx.doc, view: Affine::translate((dv.x - x0 as f64, dv.y - y0 as f64)) * ctx.view, opts: ctx.opts };
        self.silhouette(&sub, o, &mut layer);
        fx::blur(&mut layer, feather as f32);
        fx::tint(&mut layer, [s.color.r, s.color.g, s.color.b], (s.opacity / 100.0) as f32);
        if s.fade > 0.0 {
            // fade from the near end (the object's base) to the far end
            let bb = o.bounds();
            let near = (sub.view * Point::new(bb.center().x, if persp.is_some() { bb.y1 } else { bb.y0 })).y;
            let far = (sub.view * Point::new(bb.center().x, if persp.is_some() { bb.y0 } else { bb.y1 })).y;
            fx::fade_rows(&mut layer, near as f32, far as f32, (s.fade / 100.0) as f32);
        }
        target.draw_pixmap(x0, y0, layer.as_ref(), &sk::PixmapPaint { blend_mode: blend_mode(s.merge), ..Default::default() }, sk::Transform::identity(), None);
    }

    fn render_inner_shadow(&mut self, ctx: &Ctx, o: &Object, path: &BezPath, s: &DropShadow, target: &mut sk::Pixmap) {
        let Some(sp) = to_sk_path(path) else { return };
        let zoom = view_scale(ctx.view);
        let feather = (s.feather * zoom) as f32;
        let w = target.width();
        let h = target.height();
        let mut layer = sk::Pixmap::new(w, h).unwrap();
        layer.fill(sk::Color::BLACK);
        let lin = linear_part(ctx.view);
        let dv = lin * Point::new(s.offset.x, s.offset.y);
        let mut clear = sk::Paint::default();
        clear.blend_mode = sk::BlendMode::Clear;
        layer.fill_path(&sp, &clear, sk::FillRule::Winding, to_sk(Affine::translate(dv.to_vec2()) * ctx.view), None);
        fx::blur(&mut layer, feather);
        fx::tint(&mut layer, [s.color.r, s.color.g, s.color.b], (s.opacity / 100.0) as f32);
        let mut mask = sk::Mask::new(w, h).unwrap();
        mask.fill_path(&sp, if o.style.even_odd { sk::FillRule::EvenOdd } else { sk::FillRule::Winding }, true, to_sk(ctx.view));
        target.draw_pixmap(0, 0, layer.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), Some(&mask));
    }

    fn render_block_shadow(&mut self, ctx: &Ctx, o: &Object, b: &BlockShadow, target: &mut sk::Pixmap) {
        let mut path = o.world_path();
        if b.remove_holes {
            path = outer_contours(&path);
        }
        let mut grow = b.expand;
        if b.from_outline && o.style.outline.is_visible() {
            grow += o.style.outline.width / 2.0;
        }
        if grow.abs() > 1e-6 {
            path = geom::offset_path(&path, grow, kurbo::Join::Miter);
        }
        if b.simplify {
            path = geom::simplify_path(&path, 0.5);
        }
        let mut paths = vec![path.clone()];
        // sweep: union of the object translated along the offset, plus side quads
        let len_px = (linear_part(ctx.view) * Point::new(b.offset.x, b.offset.y)).to_vec2().hypot();
        let steps = (len_px / 2.0).clamp(1.0, 120.0) as usize;
        for k in 1..=steps {
            let t = k as f64 / steps as f64;
            paths.push(Affine::translate(b.offset * t) * path.clone());
        }
        let mut all = BezPath::new();
        for p in &paths {
            all.extend(p.elements().iter().copied());
        }
        if let Some(sp) = to_sk_path(&all) {
            let mut paint = paint_solid(b.color, ctx.opts.antialias);
            if b.overprint {
                paint.blend_mode = sk::BlendMode::Multiply;
            }
            target.fill_path(&sp, &paint, sk::FillRule::Winding, to_sk(ctx.view), None);
            let mut st = sk::Stroke::default();
            st.width = 0.0;
            if let Some(op) = to_sk_path(&paths[paths.len() - 1]) {
                let _ = op;
            }
        }
    }

    fn render_extrude(&mut self, ctx: &Ctx, o: &Object, e: &ExtrudeEffect, target: &mut sk::Pixmap) {
        let path = o.world_path();
        if e.bevel_only {
            return self.render_bevel(ctx, o, &path, e, target);
        }
        let back_m = extrude_back(path.bounding_box(), e);
        let polys = geom::flatten_polylines(&path, 0.5);
        let mut quads: Vec<([Point; 4], f64)> = vec![];
        let light = extrude_light(e);
        let base = match e.color_mode {
            0 => o.style.fill.main_color().unwrap_or(e.color),
            _ => e.color,
        };
        for (pts, closed) in &polys {
            let n = pts.len();
            let m = if *closed { n } else { n - 1 };
            for k in 0..m {
                let a = pts[k];
                let b = pts[(k + 1) % n];
                let edge = b - a;
                if edge.hypot() < 1e-6 {
                    continue;
                }
                let normal = Vec2::new(edge.y, -edge.x).normalize();
                let shade = (normal.dot(light.0) * 0.5 + 0.5).clamp(0.0, 1.0) * light.1;
                quads.push(([a, b, back_m * b, back_m * a], shade));
            }
        }
        // painter's order: draw faces farther along the depth first
        let depth = (back_m * path.bounding_box().center()) - path.bounding_box().center();
        let dir = depth.normalize();
        quads.sort_by(|x, y| {
            let dx = (x.0[0].to_vec2() + x.0[1].to_vec2()).dot(dir);
            let dy = (y.0[0].to_vec2() + y.0[1].to_vec2()).dot(dir);
            dy.partial_cmp(&dx).unwrap_or(std::cmp::Ordering::Equal)
        });
        // back face
        let back = back_m * path.clone();
        let back_c = match e.color_mode {
            1 => base,
            0 => Color::lerp(Color::BLACK, base, 0.55),
            _ => e.shade,
        };
        self.fill_path_simple(&back, back_c, ctx.view, target, ctx.opts.antialias);
        for (q, shade) in quads {
            let mut bp = BezPath::new();
            bp.move_to(q[0]);
            bp.line_to(q[1]);
            bp.line_to(q[2]);
            bp.line_to(q[3]);
            bp.close_path();
            let c = match e.color_mode {
                1 => base,
                // draped: the fill runs over the sides, lightly shaded
                0 if e.drape => Color::lerp(Color::lerp(Color::BLACK, base, 0.8), base, shade),
                0 => Color::lerp(Color::lerp(Color::BLACK, base, 0.45), base, shade),
                _ => Color::lerp(e.shade, e.color, shade),
            };
            if let Some(sp) = to_sk_path(&bp) {
                let p = paint_solid(c, ctx.opts.antialias);
                target.fill_path(&sp, &p, sk::FillRule::Winding, to_sk(ctx.view), None);
                let mut st = sk::Stroke::default();
                st.width = 0.6;
                target.stroke_path(&sp, &p, &st, to_sk(ctx.view), None);
            }
        }
    }

    /// The bevel of an extrusion: a ring inside the front face, lit from the extrusion's lights.
    fn render_bevel(&mut self, ctx: &Ctx, o: &Object, path: &BezPath, e: &ExtrudeEffect, target: &mut sk::Pixmap) {
        let Some((depth, angle)) = e.bevel else { return };
        let inner = geom::offset_path(path, -depth.max(0.1), kurbo::Join::Miter);
        let base = if e.bevel_fill { e.color } else { e.bevel_color.or(o.style.fill.main_color()).unwrap_or(e.color) };
        let light = extrude_light(e);
        let lit = Color::lerp(base, Color::WHITE, (0.35 * light.1).clamp(0.0, 0.8));
        let dark = Color::lerp(base, Color::BLACK, (0.25 + angle.clamp(0.0, 89.0) / 300.0).clamp(0.0, 0.8));
        // the ring: outer minus inner, split into a lit half (top left) and a dark half
        let mut ring = path.clone();
        ring.extend(inner.elements().iter().copied());
        let Some(sp) = to_sk_path(&ring) else { return };
        let bb = path.bounding_box();
        let mut mask_lit = sk::Mask::new(target.width(), target.height()).unwrap();
        let mut tri = BezPath::new();
        tri.move_to((bb.x0 - 1.0, bb.y0 - 1.0));
        tri.line_to((bb.x1 + 1.0, bb.y0 - 1.0));
        tri.line_to((bb.x0 - 1.0, bb.y1 + 1.0));
        tri.close_path();
        if let Some(tp) = to_sk_path(&tri) {
            mask_lit.fill_path(&tp, sk::FillRule::Winding, true, to_sk(ctx.view));
        }
        let aa = ctx.opts.antialias;
        target.fill_path(&sp, &paint_solid(dark, aa), sk::FillRule::EvenOdd, to_sk(ctx.view), None);
        target.fill_path(&sp, &paint_solid(lit, aa), sk::FillRule::EvenOdd, to_sk(ctx.view), Some(&mask_lit));
    }

    fn render_contour(&mut self, ctx: &Ctx, o: &Object, c: &ContourEffect, target: &mut sk::Pixmap) {
        let key = obj_hash(o);
        let geo = if let Some((h, g)) = self.geo_cache.get(&o.id) {
            if *h == key {
                Some(g.clone())
            } else {
                None
            }
        } else {
            None
        };
        let geo = match geo {
            Some(g) => g,
            None => {
                let v = vector_core::object::contour_rings(o, c);
                let g = Arc::new(v);
                self.geo_cache.insert(o.id, (key, g.clone()));
                g
            }
        };
        let filled = !o.style.fill.is_none();
        for (p, f, oc) in geo.iter() {
            if let Some(sp) = to_sk_path(p) {
                if filled {
                    if let Fill::Solid(col) = f {
                        target.fill_path(&sp, &paint_solid(*col, ctx.opts.antialias), sk::FillRule::EvenOdd, to_sk(ctx.view), None);
                    }
                }
                if let (Some(col), true) = (oc, o.style.outline.is_visible()) {
                    let mut st = sk::Stroke::default();
                    st.width = o.style.outline.width as f32;
                    st.line_join = match c.corner {
                        1 => sk::LineJoin::Round,
                        2 => sk::LineJoin::Bevel,
                        _ => sk::LineJoin::Miter,
                    };
                    target.stroke_path(&sp, &paint_solid(*col, ctx.opts.antialias), &st, to_sk(ctx.view), None);
                }
            }
        }
    }

    fn render_lens(&mut self, ctx: &Ctx, o: &Object, l: &LensEffect, target: &mut sk::Pixmap) {
        let path = o.world_path();
        let Some(sp) = to_sk_path(&path) else { return };
        let w = target.width();
        let h = target.height();
        let mut mask = sk::Mask::new(w, h).unwrap();
        mask.fill_path(&sp, sk::FillRule::Winding, true, to_sk(ctx.view));
        let c = ctx.view * path.bounding_box().center();
        // what the lens sees: what is under it, or what is at its viewpoint
        let seen = match l.viewpoint {
            Some(vp) => {
                let d = c - ctx.view * vp;
                let mut s = sk::Pixmap::new(w, h).unwrap();
                s.draw_pixmap(0, 0, target.as_ref(), &sk::PixmapPaint::default(), sk::Transform::from_translate(d.x as f32, d.y as f32), None);
                s
            }
            None => target.clone(),
        };
        let mut copy = seen.clone();
        match l.kind {
            LensKind::Magnify => {
                let tf = Affine::translate(c.to_vec2()) * Affine::scale(l.amount.max(0.01)) * Affine::translate(-c.to_vec2());
                let mut mag = sk::Pixmap::new(w, h).unwrap();
                mag.draw_pixmap(0, 0, seen.as_ref(), &sk::PixmapPaint { quality: sk::FilterQuality::Bicubic, ..Default::default() }, to_sk(tf), None);
                copy = mag;
            }
            LensKind::FishEye => {
                let vb = ctx.view.transform_rect_bbox(path.bounding_box());
                fx::fish_eye(&seen, &mut copy, (c.x as f32, c.y as f32), (vb.width().max(vb.height()) / 2.0) as f32, (l.rate / 100.0) as f32);
            }
            LensKind::Wireframe => fx::wireframe(&mut copy, l.outline.map(|c| [c.r, c.g, c.b]), l.fill.map(|c| [c.r, c.g, c.b])),
            LensKind::BitmapEffect => {}
            _ => fx::map_colors(&mut copy, |r, g, b| {
                let [r, g, b] = vector_core::object::lens_color(l, [r, g, b]);
                (r, g, b)
            }),
        }
        if l.remove_face {
            // only where there is something under the lens
            for (m, px) in mask.data_mut().iter_mut().zip(seen.data().chunks_exact(4)) {
                if px[3] == 0 || (px[0] > 250 && px[1] > 250 && px[2] > 250) {
                    *m = 0;
                }
            }
        }
        target.draw_pixmap(0, 0, copy.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), Some(&mask));
        // outline on top
        self.stroke(ctx, &path, &o.style.outline, o, target, None);
    }
}

fn mask_offset(mask: &sk::Mask, _target: &sk::Pixmap, _x0: i32, _y0: i32) -> sk::Mask {
    // draw_pixmap with a mask expects the mask in destination space; build one of target size
    let tw = _target.width();
    let th = _target.height();
    let mut full = sk::Mask::new(tw, th).unwrap();
    let (mw, mh) = (mask.width() as i32, mask.height() as i32);
    let src = mask.data();
    let dst = full.data_mut();
    for y in 0..mh {
        let ty = y + _y0;
        if ty < 0 || ty >= th as i32 {
            continue;
        }
        for x in 0..mw {
            let tx = x + _x0;
            if tx < 0 || tx >= tw as i32 {
                continue;
            }
            dst[(ty as u32 * tw + tx as u32) as usize] = src[(y * mw + x) as usize];
        }
    }
    full
}

fn linear_part(a: Affine) -> Affine {
    let c = a.as_coeffs();
    Affine::new([c[0], c[1], c[2], c[3], 0.0, 0.0])
}

pub fn view_scale(a: Affine) -> f64 {
    a.determinant().abs().sqrt()
}

fn normalize_text_angle(a: f64) -> f64 {
    let mut a = a;
    while a > std::f64::consts::FRAC_PI_2 {
        a -= std::f64::consts::PI;
    }
    while a < -std::f64::consts::FRAC_PI_2 {
        a += std::f64::consts::PI;
    }
    a
}


pub fn hash_str(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

pub fn obj_hash(o: &Object) -> u64 {
    hash_str(&format!("{:?}", o))
}

fn recolor(o: &mut Object, c: Color) {
    o.walk_mut(&mut |x| {
        if !x.style.fill.is_none() || matches!(x.kind, ObjKind::Text(_) | ObjKind::Image(_)) {
            x.style.fill = Fill::Solid(c);
        }
        if x.style.outline.color.is_some() {
            x.style.outline.color = Some(c);
        }
        if let ObjKind::Image(_) = x.kind {
            // images become solid rectangles
            let (w, h) = if let ObjKind::Image(i) = &x.kind { (i.w, i.h) } else { (0.0, 0.0) };
            x.kind = ObjKind::Rect(RectShape { w, h, radii: [0.0; 4], corner: CornerType::Round });
        }
        if let ObjKind::Text(t) = &mut x.kind {
            t.format.color = None;
            for s in t.spans.iter_mut() {
                s.format.color = None;
            }
        }
    });
}

fn blend_mode(m: MergeMode) -> sk::BlendMode {
    match m {
        MergeMode::Normal => sk::BlendMode::SourceOver,
        MergeMode::Multiply => sk::BlendMode::Multiply,
        MergeMode::Screen => sk::BlendMode::Screen,
        MergeMode::Overlay => sk::BlendMode::Overlay,
        MergeMode::Darken => sk::BlendMode::Darken,
        MergeMode::Lighten => sk::BlendMode::Lighten,
        MergeMode::ColorDodge => sk::BlendMode::ColorDodge,
        MergeMode::ColorBurn => sk::BlendMode::ColorBurn,
        MergeMode::HardLight => sk::BlendMode::HardLight,
        MergeMode::SoftLight => sk::BlendMode::SoftLight,
        MergeMode::Difference => sk::BlendMode::Difference,
        MergeMode::Exclusion => sk::BlendMode::Exclusion,
        MergeMode::Hue => sk::BlendMode::Hue,
        MergeMode::Saturation => sk::BlendMode::Saturation,
        MergeMode::Color => sk::BlendMode::Color,
        MergeMode::Luminosity => sk::BlendMode::Luminosity,
    }
}

fn stops(g: &Gradient) -> Vec<sk::GradientStop> {
    g.sorted_stops().iter().map(|s| sk::GradientStop::new(s.pos.clamp(0.0, 1.0) as f32, sk_color(s.color))).collect()
}

fn spread(s: SpreadMode) -> sk::SpreadMode {
    match s {
        SpreadMode::Pad => sk::SpreadMode::Pad,
        SpreadMode::Repeat => sk::SpreadMode::Repeat,
        SpreadMode::Reflect => sk::SpreadMode::Reflect,
    }
}

/// Gradient shader in document space (`space` maps the unit square to the object's box).
pub fn gradient_shader(g: &Gradient, space: Affine) -> Option<sk::Shader<'static>> {
    let s = sk::Point::from_xy(g.start.x as f32, g.start.y as f32);
    let e = sk::Point::from_xy(g.end.x as f32, g.end.y as f32);
    let tf = to_sk(space);
    match g.kind {
        GradientKind::Linear => sk::LinearGradient::new(s, e, stops(g), spread(g.spread), tf),
        GradientKind::Elliptical => {
            let r = ((g.end - g.start).hypot() as f32).max(0.001);
            sk::RadialGradient::new(s, 0.0, s, r, stops(g), spread(g.spread), tf)
        }
        GradientKind::Conical => {
            let ang = (g.end - g.start).atan2().to_degrees() as f32;
            let tf2 = tf.pre_concat(sk::Transform::from_rotate_at(ang, s.x, s.y));
            sk::SweepGradient::new(s, 0.0, 360.0, stops(g), sk::SpreadMode::Pad, tf2)
        }
        GradientKind::Rectangular => None,
    }
}

/// Color of a gradient at unit-space point (u,v), used for stepped and rectangular gradients.
pub fn gradient_color(g: &Gradient, u: f64, v: f64) -> Color {
    let p = Point::new(u, v);
    let mut t = match g.kind {
        GradientKind::Linear => {
            let d = g.end - g.start;
            let l2 = d.hypot2().max(1e-12);
            (p - g.start).dot(d) / l2
        }
        GradientKind::Elliptical => (p - g.start).hypot() / (g.end - g.start).hypot().max(1e-9),
        GradientKind::Conical => {
            let a0 = (g.end - g.start).atan2();
            let a = (p - g.start).atan2() - a0;
            a.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU
        }
        GradientKind::Rectangular => {
            let r = (g.end - g.start).hypot().max(1e-9);
            let d = p - g.start;
            d.x.abs().max(d.y.abs()) / r
        }
    };
    t = match g.spread {
        SpreadMode::Pad => t.clamp(0.0, 1.0),
        SpreadMode::Repeat => t.rem_euclid(1.0),
        SpreadMode::Reflect => {
            let k = t.rem_euclid(2.0);
            if k > 1.0 {
                2.0 - k
            } else {
                k
            }
        }
    };
    if g.steps > 0 {
        let n = g.steps as f64;
        t = (t * n).floor() / (n - 1.0).max(1.0);
        t = t.clamp(0.0, 1.0);
    }
    g.color_at(t)
}

/// A tile twice as wide (and/or high) holding the tile and its mirror images, for patterns whose
/// alternate tiles are mirrored.
fn mirror_tile(tile: &sk::Pixmap, mirror: u8) -> Arc<sk::Pixmap> {
    let (w, h) = (tile.width(), tile.height());
    let (nx, ny) = (if mirror & 1 != 0 { 2 } else { 1 }, if mirror & 2 != 0 { 2 } else { 1 });
    let Some(mut out) = sk::Pixmap::new(w * nx, h * ny) else { return Arc::new(tile.clone()) };
    for j in 0..ny {
        for i in 0..nx {
            let (fx, fy) = (if i == 1 { -1.0 } else { 1.0 }, if j == 1 { -1.0 } else { 1.0 });
            let tx = if i == 1 { (2 * w) as f32 } else { 0.0 };
            let ty = if j == 1 { (2 * h) as f32 } else { 0.0 };
            out.draw_pixmap(0, 0, tile.as_ref(), &sk::PixmapPaint::default(), sk::Transform::from_row(fx, 0.0, 0.0, fy, tx, ty), None);
        }
    }
    Arc::new(out)
}

fn gradient_mask(o: &Object, g: &Gradient, view: &Affine, w: u32, h: u32) -> Option<sk::Mask> {
    let path = o.world_path();
    let space = Renderer::fill_space(o, &path);
    let inv = (*view * space).inverse();
    let mut m = sk::Mask::new(w, h)?;
    let data = m.data_mut();
    for y in 0..h {
        for x in 0..w {
            let uv = inv * Point::new(x as f64 + 0.5, y as f64 + 0.5);
            let c = gradient_color(g, uv.x, uv.y);
            // black = transparent, white = opaque (luminance), stop alpha also applies
            let l = c.luminance() / 255.0 * (c.a as f64 / 255.0);
            data[(y * w + x) as usize] = (l * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    Some(m)
}

#[allow(clippy::too_many_arguments)]
fn draw_arrow(kind: ArrowHead, tip: Point, dir: Vec2, w: f64, color: Color, view: Affine, target: &mut sk::Pixmap, aa: bool, custom: &[vector_core::style::CustomArrow]) {
    let d = dir.normalize();
    if !d.x.is_finite() {
        return;
    }
    let n = Vec2::new(-d.y, d.x);
    let s = (w * 4.0).max(4.0);
    let mut bp = BezPath::new();
    let mut stroke_only = false;
    match kind {
        ArrowHead::Triangle => {
            bp.move_to(tip + d * (s * 0.3));
            bp.line_to(tip - d * s + n * (s * 0.45));
            bp.line_to(tip - d * s - n * (s * 0.45));
            bp.close_path();
        }
        ArrowHead::OpenTriangle => {
            bp.move_to(tip - d * s + n * (s * 0.45));
            bp.line_to(tip);
            bp.line_to(tip - d * s - n * (s * 0.45));
            stroke_only = true;
        }
        ArrowHead::Circle => {
            bp = kurbo::Circle::new(tip, s * 0.35).to_path(0.05);
        }
        ArrowHead::Square => {
            let c = tip;
            let hs = s * 0.3;
            bp.move_to(c + d * hs + n * hs);
            bp.line_to(c - d * hs + n * hs);
            bp.line_to(c - d * hs - n * hs);
            bp.line_to(c + d * hs - n * hs);
            bp.close_path();
        }
        ArrowHead::Diamond => {
            bp.move_to(tip + d * (s * 0.4));
            bp.line_to(tip + n * (s * 0.3));
            bp.line_to(tip - d * (s * 0.4));
            bp.line_to(tip - n * (s * 0.3));
            bp.close_path();
        }
        ArrowHead::Bar => {
            bp.move_to(tip + n * (s * 0.5));
            bp.line_to(tip - n * (s * 0.5));
            stroke_only = true;
        }
        ArrowHead::Custom(id) => {
            let Some(ca) = custom.iter().find(|c| c.id == id) else { return };
            bp = ca.placed(tip, d, w);
            stroke_only = !ca.filled;
        }
    }
    if let Some(sp) = to_sk_path(&bp) {
        let p = paint_solid(color, aa);
        if stroke_only {
            let mut st = sk::Stroke::default();
            st.width = w as f32;
            st.line_cap = sk::LineCap::Round;
            st.line_join = sk::LineJoin::Round;
            target.stroke_path(&sp, &p, &st, to_sk(view), None);
        } else {
            target.fill_path(&sp, &p, sk::FillRule::Winding, to_sk(view), None);
        }
    }
}

/// Build a filled outline for a stroke with a width profile.
pub fn variable_width_outline(path: &BezPath, width: f64, profile: &WidthProfile) -> BezPath {
    let mut out = BezPath::new();
    for (pts, closed) in geom::flatten_polylines(path, 0.2) {
        let n = pts.len();
        if n < 2 {
            continue;
        }
        let mut acc = vec![0.0];
        for w in pts.windows(2) {
            acc.push(acc.last().unwrap() + (w[1] - w[0]).hypot());
        }
        let total = acc.last().copied().unwrap_or(1.0).max(1e-9);
        let wfn = |s: f64, dir: Vec2| -> f64 {
            let t = s / total;
            match profile {
                WidthProfile::Taper => (std::f64::consts::PI * t).sin().max(0.05),
                WidthProfile::TaperEnd => (1.0 - t).max(0.05),
                WidthProfile::TaperStart => t.max(0.05),
                WidthProfile::Calligraphic { angle, min_ratio } => {
                    let a = angle.to_radians();
                    let nib = Vec2::new(a.cos(), a.sin());
                    let c = dir.normalize().cross(nib).abs();
                    min_ratio + (1.0 - min_ratio) * c
                }
                WidthProfile::Pressure(samples) => {
                    if samples.is_empty() {
                        1.0
                    } else {
                        let idx = t * (samples.len() - 1) as f64;
                        let i = idx.floor() as usize;
                        let f = idx - i as f64;
                        let a = samples[i.min(samples.len() - 1)] as f64;
                        let b = samples[(i + 1).min(samples.len() - 1)] as f64;
                        (a + (b - a) * f).max(0.05)
                    }
                }
                WidthProfile::Painterly { .. } | WidthProfile::Variable { .. } => 1.0,
            }
        };
        let side = |s: f64, dir: Vec2| -> (f64, f64) {
            if let WidthProfile::Variable { nodes, .. } = profile {
                return vector_core::style::var_width_at(nodes, s / total);
            }
            let hw = width / 2.0 * wfn(s, dir);
            (hw, hw)
        };
        let mut left = vec![];
        let mut right = vec![];
        for i in 0..n {
            let prev = if i == 0 { pts[0] } else { pts[i - 1] };
            let next = if i + 1 < n { pts[i + 1] } else { pts[n - 1] };
            let dir = (next - prev).normalize();
            let dir = if dir.x.is_finite() { dir } else { Vec2::new(1.0, 0.0) };
            let normal = Vec2::new(-dir.y, dir.x);
            let (hl, hr) = side(acc[i], dir);
            left.push(pts[i] + normal * hl);
            right.push(pts[i] - normal * hr);
        }
        out.move_to(left[0]);
        for p in &left[1..] {
            out.line_to(*p);
        }
        for p in right.iter().rev() {
            out.line_to(*p);
        }
        out.close_path();
        let _ = closed;
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn painterly(path: &BezPath, width: f64, color: Color, bristles: u32, jitter: f64, seed: u32, o: &vector_core::style::PaintOpts, view: Affine, target: &mut sk::Pixmap, aa: bool) {
    let mut state = seed.wrapping_mul(747796405).wrapping_add(2891336453).max(1);
    let mut rnd = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f64 / u32::MAX as f64
    };
    let bristles = bristles.clamp(1, 24);
    let opacity = (1.0 - o.alpha / 100.0).clamp(0.0, 1.0);
    let grain = (o.texture_strength / 100.0).clamp(0.0, 1.0);
    // a tilted nib is flatter and wider, and leans in its direction
    let lean = ((90.0 - o.tilt.clamp(0.0, 90.0)) / 90.0).clamp(0.0, 1.0);
    let width = width * (1.0 + lean * 0.6);
    let lean_v = Vec2::new(-o.tilt_dir.to_radians().cos(), o.tilt_dir.to_radians().sin()) * (lean * width * 0.3);
    let shade_of = |base: Color, t: f64, k: f64| -> (u8, u8, u8) {
        // Glow: the color builds up to white along the stroke
        let g = if o.glow { (t * 0.85).clamp(0.0, 0.85) } else { 0.0 };
        let ch = |c: u8| ((c as f64 * k) * (1.0 - g) + 255.0 * g).clamp(0.0, 255.0) as u8;
        (ch(base.r), ch(base.g), ch(base.b))
    };
    for (pts, _) in geom::flatten_polylines(path, 0.3) {
        if pts.len() < 2 {
            continue;
        }
        let n = pts.len();
        // position along the stroke (0..1) of each point
        let mut along = vec![0.0; n];
        for i in 1..n {
            along[i] = along[i - 1] + (pts[i] - pts[i - 1]).hypot();
        }
        let total = along[n - 1].max(1e-9);
        let taper = |i: usize| -> f64 {
            if o.taper {
                (std::f64::consts::PI * along[i] / total).sin().max(0.0).powf(0.6).max(0.08)
            } else {
                1.0
            }
        };
        let normal_at = |i: usize| -> Option<Vec2> {
            let prev = if i == 0 { pts[0] } else { pts[i - 1] };
            let next = if i + 1 < n { pts[i + 1] } else { pts[n - 1] };
            let dir = (next - prev).normalize();
            dir.x.is_finite().then(|| Vec2::new(-dir.y, dir.x))
        };
        match o.mark {
            // airbrush droplets and particles: dots scattered over the stroke
            1 | 2 => {
                let particles = o.mark == 2;
                let per = (o.density.clamp(1.0, 100.0) / 100.0) * if particles { 0.6 } else { 3.0 };
                for i in 0..n {
                    let Some(nv) = normal_at(i) else { continue };
                    let step = if i == 0 { 0.0 } else { (pts[i] - pts[i - 1]).hypot() };
                    let count = ((step.max(0.5) * per).round() as usize).clamp(1, 60);
                    for _ in 0..count {
                        let off = (rnd() - 0.5) * width * taper(i);
                        let q = pts[i] + nv * off + lean_v + Vec2::new((rnd() - 0.5) * width * 0.2, (rnd() - 0.5) * width * 0.2);
                        let r = if particles { width * (0.04 + rnd() * 0.08) } else { width * 0.02 + 0.15 };
                        let a = opacity * (0.5 + rnd() * 0.5) * (1.0 - grain * rnd() * 0.5);
                        let (r8, g8, b8) = shade_of(color, along[i] / total, 0.85 + rnd() * 0.3);
                        if let Some(c) = sk::PathBuilder::from_circle(q.x as f32, q.y as f32, r.max(0.05) as f32) {
                            target.fill_path(&c, &paint_solid(Color::rgba(r8, g8, b8, (a * 255.0) as u8), aa), sk::FillRule::Winding, to_sk(view), None);
                        }
                    }
                }
            }
            // ink: one solid stroke following the taper
            3 => {
                let mut left = vec![];
                let mut right = vec![];
                for i in 0..n {
                    let Some(nv) = normal_at(i) else { continue };
                    let w = width * 0.5 * taper(i);
                    left.push(pts[i] + nv * w + lean_v);
                    right.push(pts[i] - nv * w + lean_v);
                }
                if left.len() >= 2 {
                    let mut bp = BezPath::new();
                    bp.move_to(left[0]);
                    for q in &left[1..] {
                        bp.line_to(*q);
                    }
                    for q in right.iter().rev() {
                        bp.line_to(*q);
                    }
                    bp.close_path();
                    let (r8, g8, b8) = shade_of(color, 0.5, 1.0);
                    if let Some(sp) = to_sk_path(&bp) {
                        target.fill_path(&sp, &paint_solid(Color::rgba(r8, g8, b8, (opacity * 255.0) as u8), aa), sk::FillRule::Winding, to_sk(view), None);
                    }
                }
            }
            // bristles: one textured line per bristle
            _ => {
                for b in 0..bristles {
                    let off = (b as f64 / (bristles.max(2) - 1) as f64 - 0.5) * width;
                    let shade = 0.75 + rnd() * 0.5;
                    let a = opacity * ((180.0 + rnd() * 75.0) / 255.0) * (1.0 - grain * rnd() * 0.6);
                    // glow: split the bristle in runs so the color can build up along it
                    let runs = if o.glow { 8 } else { 1 };
                    for run in 0..runs {
                        let i0 = run * (n - 1) / runs;
                        let i1 = ((run + 1) * (n - 1) / runs).max(i0 + 1).min(n - 1);
                        let mut bp = BezPath::new();
                        let mut started = false;
                        for i in i0..=i1 {
                            let Some(nv) = normal_at(i) else { continue };
                            let j = (rnd() - 0.5) * jitter * width * 0.2;
                            let q = pts[i] + nv * ((off + j) * taper(i)) + lean_v;
                            if !started {
                                bp.move_to(q);
                                started = true;
                            } else {
                                bp.line_to(q);
                            }
                        }
                        let t = (along[i0] + along[i1]) / 2.0 / total;
                        let (r8, g8, b8) = shade_of(color, t, shade);
                        if let Some(sp) = to_sk_path(&bp) {
                            let mut st = sk::Stroke::default();
                            st.width = (width / bristles as f64 * 1.6) as f32;
                            st.line_cap = sk::LineCap::Round;
                            st.line_join = sk::LineJoin::Round;
                            target.stroke_path(&sp, &paint_solid(Color::rgba(r8, g8, b8, (a * 255.0) as u8), aa), &st, to_sk(view), None);
                        }
                    }
                }
            }
        }
    }
}

/// Render a document page to an RGBA image at a given resolution (for export/print/thumbnails).
pub fn render_page_image(doc: &Document, page: usize, dpi: f64, background: Option<Color>, area: Option<Rect>, antialias: bool) -> Option<sk::Pixmap> {
    let pg = doc.pages.get(page)?;
    let area = area.unwrap_or(pg.rect());
    let scale = dpi / 72.0;
    let w = (area.width() * scale).round().max(1.0) as u32;
    let h = (area.height() * scale).round().max(1.0) as u32;
    if w as u64 * h as u64 > 400_000_000 {
        return None;
    }
    let mut pm = sk::Pixmap::new(w, h)?;
    if let Some(bg) = background {
        pm.fill(sk_color(bg));
    }
    let view = Affine::scale(scale) * Affine::translate((-area.x0, -area.y0));
    let opts = RenderOptions { draw_page: false, printing: false, antialias, ..Default::default() };
    let mut r = Renderer::new();
    r.render_page(doc, page, view, &mut pm, &opts);
    Some(pm)
}

/// Render arbitrary objects to an image (thumbnails for symbols, styles).
pub fn render_objects_image(doc: &Document, objs: &[Object], size: u32, bg: Option<Color>) -> Option<sk::Pixmap> {
    let b = union_bounds(objs.iter().map(|o| o.visual_bounds()));
    if b.width() <= 0.0 && b.height() <= 0.0 {
        return None;
    }
    let s = (size as f64 - 4.0) / b.width().max(b.height()).max(1e-6);
    let mut pm = sk::Pixmap::new(size, size)?;
    if let Some(c) = bg {
        pm.fill(sk_color(c));
    }
    let view = Affine::translate((size as f64 / 2.0, size as f64 / 2.0)) * Affine::scale(s) * Affine::translate(-b.center().to_vec2());
    let mut r = Renderer::new();
    r.render_objects(doc, objs, view, &mut pm, &RenderOptions { draw_page: false, ..Default::default() });
    Some(pm)
}

/// Convert a premultiplied pixmap to straight RGBA bytes.
pub fn pixmap_to_rgba(pm: &sk::Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(pm.data().len());
    for px in pm.pixels() {
        let c = px.demultiply();
        out.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    out
}

/// The ground transform of a perspective shadow: flattened by the stretch and slanted by the angle,
/// from the base of the object.
pub fn perspective_shadow(bb: Rect, s: &DropShadow) -> Affine {
    let base = Point::new(bb.center().x, bb.y1);
    let k = (s.stretch / 100.0).clamp(0.0, 2.0);
    let skew = s.angle.clamp(-89.0, 89.0).to_radians().tan();
    Affine::translate(base.to_vec2()) * Affine::new([1.0, 0.0, -skew * k, k, 0.0, 0.0]) * Affine::translate(-base.to_vec2())
}

/// Only the outer contours of a path (Remove Holes).
fn outer_contours(path: &BezPath) -> BezPath {
    let subs = geom::subpaths(path);
    let Some(main_sign) = subs.iter().map(|p| p.area()).max_by(|a, b| a.abs().partial_cmp(&b.abs()).unwrap_or(std::cmp::Ordering::Equal)).map(|a| a.signum()) else {
        return path.clone();
    };
    let mut out = BezPath::new();
    for p in subs {
        if p.area().signum() == main_sign {
            out.extend(p.elements().iter().copied());
        }
    }
    out
}

/// Where the back (or front) face of an extrusion goes: along the depth, converging toward the
/// vanishing point (small), away from it (big) or parallel; turned by the extrusion's rotation.
pub fn extrude_back(bb: Rect, e: &ExtrudeEffect) -> Affine {
    let [rx, ry, rz] = e.rotation;
    let mut d = e.depth;
    // rotation: turn the depth around the view axis and foreshorten it around the other two
    let (sz, cz) = rz.to_radians().sin_cos();
    d = Vec2::new(d.x * cz - d.y * sz, d.x * sz + d.y * cz);
    d = Vec2::new(d.x * ry.to_radians().cos() + bb.width() * 0.5 * ry.to_radians().sin(), d.y * rx.to_radians().cos() + bb.height() * 0.5 * rx.to_radians().sin());
    let front = matches!(e.kind, 1 | 3 | 5);
    if front {
        d = -d;
    }
    let scale = match e.kind {
        0 | 1 => 0.7,
        2 | 3 => 1.4,
        _ => 1.0,
    };
    let c = bb.center();
    Affine::translate(d) * Affine::translate(c.to_vec2()) * Affine::scale(scale) * Affine::translate(-c.to_vec2())
}

/// The extrusion's light: direction (toward the light) and intensity factor.
fn extrude_light(e: &ExtrudeEffect) -> (Vec2, f64) {
    if e.lights.iter().all(|l| l.0 >= 27) {
        return (Vec2::new(-0.6, -0.8), 1.0);
    }
    let mut dir = Vec2::ZERO;
    let mut total = 0.0;
    for (pos, intensity) in e.lights.iter().filter(|l| l.0 < 27) {
        // where the lines of the light cube cross: layer × 9 + row × 3 + column
        let gx = (*pos % 3) as f64 - 1.0;
        let gy = ((*pos % 9) / 3) as f64 - 1.0;
        let k = intensity / 100.0;
        dir += Vec2::new(gx, gy) * k;
        total += k;
    }
    let n = if dir.hypot() > 1e-6 { dir.normalize() } else { Vec2::new(-0.6, -0.8) };
    (n, (0.4 + total * 0.6).clamp(0.2, 1.6))
}
