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
        let overprints = ctx.opts.overprint && (o.style.overprint_fill || o.style.overprint_outline);
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
            if let Some((depth, angle)) = o.effects.bevel {
                fx::bevel(&mut layer, (depth * view_scale(ctx.view)) as f32, angle as f32);
            }
            let mut opacity = o.style.transparency.uniform_opacity();
            if let Some(d) = dimmed {
                opacity *= d;
            }
            let mut mask = None;
            if let Transparency::Gradient(g) = &o.style.transparency {
                mask = gradient_mask(o, g, &sub.view, w, h);
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
        let aa = ctx.opts.antialias;
        let view = ctx.view;
        if ctx.opts.wireframe {
            return self.render_wire(ctx, o, target);
        }
        // shadows / extrusions behind the object
        if let Some(s) = &o.effects.drop_shadow {
            self.render_drop_shadow(ctx, o, s, target);
        }
        if let Some(b) = &o.effects.block_shadow {
            self.render_block_shadow(ctx, o, b, target);
        }
        if let Some(e) = &o.effects.extrude {
            self.render_extrude(ctx, o, e, target);
        }
        if let Some(c) = &o.effects.contour {
            if c.dir == ContourDir::Outside {
                self.render_contour(ctx, o, c, target);
            }
        }
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
                for k in 1..=b.steps {
                    let t = k as f64 / (b.steps + 1) as f64;
                    let t = accel(t, b.acceleration);
                    let step = blend_step(&b.start, &b.end, t, 0, b.rotation);
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
        if let Some(l) = &o.effects.lens {
            self.render_lens(ctx, o, l, target);
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
                if g.steps > 0 || g.kind == GradientKind::Rectangular {
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
                let zoom = view_scale(ctx.view);
                let tile_px = (pf.tile * zoom).clamp(2.0, 1024.0);
                let img_pm = pf.image.and_then(|id| self.image_pixmap(ctx.doc, id, &ImageAdjust::default()));
                let tile = match img_pm {
                    Some(p) => p,
                    None => self.pattern_tile(pf, 64),
                };
                let tw = tile.width() as f64;
                let scale = tile_px / tw / zoom;
                let space = if pf.transform_with_object {
                    let s = Self::fill_space(o, path);
                    let origin = s * Point::ZERO;
                    Affine::translate(origin.to_vec2()) * Affine::rotate(-pf.angle.to_radians())
                } else {
                    Affine::rotate(-pf.angle.to_radians())
                };
                let shader_tf = to_sk(space * Affine::scale(scale));
                let mut p = sk::Paint::default();
                p.shader = sk::Pattern::new(tile.as_ref().as_ref(), sk::SpreadMode::Repeat, sk::FilterQuality::Bilinear, 1.0, shader_tf);
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
                WidthProfile::Painterly { bristles, jitter, seed } => {
                    painterly(path, ol.width.max(1.0), color, *bristles, *jitter, *seed, ctx.view, target, aa);
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
                        draw_arrow(a, tip, tip - from, w, color, ctx.view, target, aa);
                    }
                }
            }
            if let Some((pts, closed)) = polys.last() {
                if !closed && pts.len() >= 2 {
                    if let Some(a) = ol.end_arrow {
                        let n = pts.len();
                        draw_arrow(a, pts[n - 1], pts[n - 1] - pts[n - 2], w, color, ctx.view, target, aa);
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
        for r in 0..t.rows {
            for c in 0..t.cols {
                let cell = &t.cells[r * t.cols + c];
                let rc = t.cell_rect(r, c);
                if let Some(f) = cell.fill {
                    self.fill_path_simple(&rc.to_path(0.1), f, view, target, aa);
                }
                if !cell.text.is_empty() {
                    let mut tobj = text::TextObj::paragraph(&cell.text, (rc.width() - 4.0).max(1.0), (rc.height() - 4.0).max(1.0), t.text_format.clone());
                    tobj.align = text::TextAlign::Left;
                    let lay = text::layout(&tobj);
                    let tp = Affine::translate((rc.x0 + 2.0, rc.y0 + 2.0)) * lay.to_path();
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
        // grid lines
        let mut bp = BezPath::new();
        let (w, h) = (t.width(), t.height());
        let mut y = 0.0;
        for r in 0..=t.rows {
            bp.move_to((0.0, y));
            bp.line_to((w, y));
            if r < t.rows {
                y += t.row_heights[r];
            }
        }
        let mut x = 0.0;
        for c in 0..=t.cols {
            bp.move_to((x, 0.0));
            bp.line_to((x, h));
            if c < t.cols {
                x += t.col_widths[c];
            }
        }
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
        target.draw_pixmap(x0, y0, layer.as_ref(), &sk::PixmapPaint { blend_mode: sk::BlendMode::Multiply, ..Default::default() }, sk::Transform::identity(), None);
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
        let path = o.world_path();
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
            target.fill_path(&sp, &paint_solid(b.color, ctx.opts.antialias), sk::FillRule::Winding, to_sk(ctx.view), None);
            let mut st = sk::Stroke::default();
            st.width = 0.0;
            if let Some(op) = to_sk_path(&paths[paths.len() - 1]) {
                let _ = op;
            }
        }
    }

    fn render_extrude(&mut self, ctx: &Ctx, o: &Object, e: &ExtrudeEffect, target: &mut sk::Pixmap) {
        let path = o.world_path();
        let polys = geom::flatten_polylines(&path, 0.5);
        let mut quads: Vec<([Point; 4], f64)> = vec![];
        let light = Vec2::new(-0.6, -0.8);
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
                let shade = (normal.dot(light) * 0.5 + 0.5).clamp(0.0, 1.0);
                quads.push(([a, b, b + e.depth, a + e.depth], shade));
            }
        }
        // painter's order: draw faces farther along the depth first
        let dir = e.depth.normalize();
        quads.sort_by(|x, y| {
            let dx = (x.0[0].to_vec2() + x.0[1].to_vec2()).dot(dir);
            let dy = (y.0[0].to_vec2() + y.0[1].to_vec2()).dot(dir);
            dy.partial_cmp(&dx).unwrap_or(std::cmp::Ordering::Equal)
        });
        // back face
        let back = Affine::translate(e.depth) * path.clone();
        self.fill_path_simple(&back, e.shade, ctx.view, target, ctx.opts.antialias);
        for (q, shade) in quads {
            let mut bp = BezPath::new();
            bp.move_to(q[0]);
            bp.line_to(q[1]);
            bp.line_to(q[2]);
            bp.line_to(q[3]);
            bp.close_path();
            let c = Color::lerp(e.shade, e.color, shade);
            if let Some(sp) = to_sk_path(&bp) {
                let p = paint_solid(c, ctx.opts.antialias);
                target.fill_path(&sp, &p, sk::FillRule::Winding, to_sk(ctx.view), None);
                let mut st = sk::Stroke::default();
                st.width = 0.6;
                target.stroke_path(&sp, &p, &st, to_sk(ctx.view), None);
            }
        }
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
                let base = o.world_path();
                let mut v = vec![];
                let start_fill = o.style.fill.main_color().unwrap_or(Color::WHITE);
                let start_ol = o.style.outline.color.unwrap_or(Color::BLACK);
                let steps = c.steps.max(1).min(64);
                for k in 1..=steps {
                    let d = match c.dir {
                        ContourDir::Outside => c.offset * k as f64,
                        _ => -c.offset * k as f64,
                    };
                    let p = geom::offset_path(&base, d, kurbo::Join::Round);
                    if p.elements().is_empty() || p.area().abs() < 1e-3 {
                        break;
                    }
                    let t = k as f64 / steps as f64;
                    v.push((p, Fill::Solid(Color::lerp(start_fill, c.end_fill, t)), Some(Color::lerp(start_ol, c.end_outline, t))));
                }
                if c.dir == ContourDir::Outside {
                    v.reverse();
                }
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
        let mut copy = target.clone();
        match l {
            LensEffect::Magnify(f) => {
                let c = ctx.view * path.bounding_box().center();
                let tf = Affine::translate(c.to_vec2()) * Affine::scale(*f) * Affine::translate(-c.to_vec2());
                let mut mag = sk::Pixmap::new(w, h).unwrap();
                mag.draw_pixmap(0, 0, target.as_ref(), &sk::PixmapPaint { quality: sk::FilterQuality::Bicubic, ..Default::default() }, to_sk(tf), None);
                copy = mag;
            }
            LensEffect::Brighten(b) => fx::adjust(&mut copy, *b as f32, 0.0, 0.0),
            LensEffect::Invert => fx::invert(&mut copy),
            LensEffect::Grayscale => fx::grayscale(&mut copy),
            LensEffect::ColorAdd(c) => {
                let (cr, cg, cb) = (c.r as f32, c.g as f32, c.b as f32);
                fx::map_colors(&mut copy, |r, g, b| (r + cr * 0.5, g + cg * 0.5, b + cb * 0.5));
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

fn accel(t: f64, a: f64) -> f64 {
    if a.abs() < 1e-6 {
        return t;
    }
    let k = 1.0 + a.abs() / 25.0;
    if a > 0.0 {
        t.powf(k)
    } else {
        1.0 - (1.0 - t).powf(k)
    }
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
fn draw_arrow(kind: ArrowHead, tip: Point, dir: Vec2, w: f64, color: Color, view: Affine, target: &mut sk::Pixmap, aa: bool) {
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
                WidthProfile::Painterly { .. } => 1.0,
            }
        };
        let mut left = vec![];
        let mut right = vec![];
        for i in 0..n {
            let prev = if i == 0 { pts[0] } else { pts[i - 1] };
            let next = if i + 1 < n { pts[i + 1] } else { pts[n - 1] };
            let dir = (next - prev).normalize();
            let dir = if dir.x.is_finite() { dir } else { Vec2::new(1.0, 0.0) };
            let normal = Vec2::new(-dir.y, dir.x);
            let hw = width / 2.0 * wfn(acc[i], dir);
            left.push(pts[i] + normal * hw);
            right.push(pts[i] - normal * hw);
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
fn painterly(path: &BezPath, width: f64, color: Color, bristles: u32, jitter: f64, seed: u32, view: Affine, target: &mut sk::Pixmap, aa: bool) {
    let mut state = seed.wrapping_mul(747796405).wrapping_add(2891336453).max(1);
    let mut rnd = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f64 / u32::MAX as f64
    };
    let bristles = bristles.clamp(1, 24);
    for (pts, _) in geom::flatten_polylines(path, 0.3) {
        if pts.len() < 2 {
            continue;
        }
        for b in 0..bristles {
            let off = (b as f64 / (bristles.max(2) - 1) as f64 - 0.5) * width;
            let shade = 0.75 + rnd() * 0.5;
            let c = Color::rgba(
                (color.r as f64 * shade).clamp(0.0, 255.0) as u8,
                (color.g as f64 * shade).clamp(0.0, 255.0) as u8,
                (color.b as f64 * shade).clamp(0.0, 255.0) as u8,
                (180.0 + rnd() * 75.0) as u8,
            );
            let mut bp = BezPath::new();
            let mut started = false;
            let n = pts.len();
            for i in 0..n {
                let prev = if i == 0 { pts[0] } else { pts[i - 1] };
                let next = if i + 1 < n { pts[i + 1] } else { pts[n - 1] };
                let dir = (next - prev).normalize();
                if !dir.x.is_finite() {
                    continue;
                }
                let normal = Vec2::new(-dir.y, dir.x);
                let j = (rnd() - 0.5) * jitter * width * 0.2;
                let p = pts[i] + normal * (off + j);
                if !started {
                    bp.move_to(p);
                    started = true;
                } else {
                    bp.line_to(p);
                }
            }
            if let Some(sp) = to_sk_path(&bp) {
                let mut st = sk::Stroke::default();
                st.width = (width / bristles as f64 * 1.6) as f32;
                st.line_cap = sk::LineCap::Round;
                st.line_join = sk::LineJoin::Round;
                target.stroke_path(&sp, &paint_solid(c, aa), &st, to_sk(view), None);
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
