//! Flattening of the document model into simple drawing primitives shared by the SVG and PDF
//! exporters. Anything that has no faithful vector equivalent is rasterized with the renderer.

use kurbo::{Affine, BezPath, Rect, Shape};
use vector_core::color::Color;
use vector_core::document::Document;
use vector_core::geom;
use vector_core::object::*;
use vector_core::style::*;
use vector_core::text;

#[derive(Clone, Debug)]
pub enum Paint {
    Solid(Color),
    /// Gradient with `space` mapping the unit square (gradient coordinates) to document space.
    Gradient(Gradient, Affine),
}

#[derive(Clone, Debug)]
pub struct StrokeStyle {
    pub width: f64,
    pub join: LineJoin,
    pub cap: LineCap,
    pub dash: Vec<f64>,
    pub miter: f64,
}

#[derive(Clone, Debug)]
pub enum Prim {
    Fill { path: BezPath, paint: Paint, even_odd: bool },
    Stroke { path: BezPath, color: Color, style: StrokeStyle },
    /// Raster image: `xf` maps image pixel space to document space; `clip` is the visible region.
    Image { image: u64, xf: Affine, clip: BezPath },
    /// Pre-rendered pixels covering `rect` (document space), straight RGBA.
    Raster { rect: Rect, w: u32, h: u32, rgba: Vec<u8> },
    BeginGroup { opacity: f32, blend: MergeMode },
    EndGroup,
    BeginClip { path: BezPath },
    EndClip,
    /// Hyperlink area.
    Link { rect: Rect, url: String },
}

#[derive(Clone, Debug)]
pub struct FlatOptions {
    /// Resolution for rasterized effects.
    pub raster_dpi: f64,
    /// Supported natively by the target (SVG yes, PDF no).
    pub conical_gradients: bool,
    pub repeat_gradients: bool,
    /// Gradient stops with alpha supported natively.
    pub gradient_alpha: bool,
    pub printing: bool,
}

pub struct Flattener<'a> {
    pub doc: &'a Document,
    pub opts: FlatOptions,
    pub out: Vec<Prim>,
}

fn to_join(j: LineJoin) -> kurbo::Join {
    match j {
        LineJoin::Miter => kurbo::Join::Miter,
        LineJoin::Round => kurbo::Join::Round,
        LineJoin::Bevel => kurbo::Join::Bevel,
    }
}
fn to_cap(c: LineCap) -> kurbo::Cap {
    match c {
        LineCap::Butt => kurbo::Cap::Butt,
        LineCap::Round => kurbo::Cap::Round,
        LineCap::Square => kurbo::Cap::Square,
    }
}

impl<'a> Flattener<'a> {
    pub fn new(doc: &'a Document, opts: FlatOptions) -> Self {
        Flattener { doc, opts, out: vec![] }
    }

    pub fn page(&mut self, page: usize) {
        let Some(pg) = self.doc.pages.get(page) else { return };
        if let Some(bg) = pg.background {
            self.out.push(Prim::Fill { path: pg.rect().to_path(0.1), paint: Paint::Solid(bg), even_odd: false });
        }
        for l in &pg.layers {
            if !l.visible || (self.opts.printing && !l.printable) {
                continue;
            }
            for o in &l.objects {
                self.object(o);
            }
        }
    }

    fn needs_raster(&self, o: &Object) -> bool {
        let e = &o.effects;
        if e.drop_shadow.is_some()
            || e.inner_shadow.is_some()
            || e.lens.is_some()
            || !e.bitmap.is_empty()
            || e.bevel.is_some()
            || matches!(o.style.transparency, Transparency::Gradient(_))
        {
            return true;
        }
        if matches!(o.kind, ObjKind::Group(_) | ObjKind::PowerClip { .. } | ObjKind::Blend(_) | ObjKind::Symbol(_)) {
            return false;
        }
        match &o.style.fill {
            Fill::Mesh(_) | Fill::Pattern(_) => return true,
            Fill::Gradient(g) => {
                if g.steps > 0 || g.kind == GradientKind::Rectangular {
                    return true;
                }
                if g.kind == GradientKind::Conical && !self.opts.conical_gradients {
                    return true;
                }
                if g.spread != SpreadMode::Pad && !self.opts.repeat_gradients {
                    return true;
                }
                if !self.opts.gradient_alpha && g.stops.iter().any(|s| s.color.a < 255) {
                    return true;
                }
            }
            _ => {}
        }
        matches!(o.style.outline.profile, Some(WidthProfile::Painterly { .. }))
    }

    fn rasterize(&mut self, o: &Object) {
        let b = o.visual_bounds().inflate(2.0, 2.0);
        if b.width() <= 0.0 || b.height() <= 0.0 {
            return;
        }
        let mut dpi = self.opts.raster_dpi;
        let max_px = 6000.0;
        let long = b.width().max(b.height()) * dpi / 72.0;
        if long > max_px {
            dpi *= max_px / long;
        }
        let s = dpi / 72.0;
        let w = (b.width() * s).ceil().max(1.0) as u32;
        let h = (b.height() * s).ceil().max(1.0) as u32;
        let Some(mut pm) = tiny_skia::Pixmap::new(w, h) else { return };
        let view = Affine::scale(s) * Affine::translate((-b.x0, -b.y0));
        let mut r = vector_render::Renderer::new();
        let opts = vector_render::RenderOptions { draw_page: false, printing: self.opts.printing, ..Default::default() };
        r.render_objects(self.doc, std::slice::from_ref(o), view, &mut pm, &opts);
        let rect = Rect::new(b.x0, b.y0, b.x0 + w as f64 / s, b.y0 + h as f64 / s);
        self.out.push(Prim::Raster { rect, w, h, rgba: vector_render::pixmap_to_rgba(&pm) });
    }

    pub fn object(&mut self, o: &Object) {
        if !o.visible || (self.opts.printing && !o.printable) {
            return;
        }
        if self.needs_raster(o) {
            self.rasterize(o);
            return;
        }
        let opacity = o.style.transparency.uniform_opacity();
        let grouped = opacity < 0.999 || o.style.merge != MergeMode::Normal;
        if grouped {
            self.out.push(Prim::BeginGroup { opacity, blend: o.style.merge });
        }
        // vector effects behind the object
        if let Some(b) = &o.effects.block_shadow {
            let path = o.world_path();
            let steps = 48;
            let mut all = BezPath::new();
            for k in 0..=steps {
                let t = k as f64 / steps as f64;
                all.extend((Affine::translate(b.offset * t) * path.clone()).elements().iter().copied());
            }
            let u = geom::simplify_self(&all);
            self.out.push(Prim::Fill { path: u, paint: Paint::Solid(b.color), even_odd: false });
        }
        if let Some(e) = &o.effects.extrude {
            self.extrude(o, e);
        }
        if let Some(c) = &o.effects.contour {
            if c.dir == ContourDir::Outside {
                self.contour(o, c);
            }
        }
        match &o.kind {
            ObjKind::Group(children) => {
                for c in children {
                    self.object(c);
                }
            }
            ObjKind::PowerClip { frame, contents } => {
                let mut f2 = (**frame).clone();
                let ol = std::mem::replace(&mut f2.style.outline, Outline::none());
                self.object(&f2);
                self.out.push(Prim::BeginClip { path: frame.world_path() });
                for c in contents {
                    self.object(c);
                }
                self.out.push(Prim::EndClip);
                f2.style.outline = ol;
                f2.style.fill = Fill::None;
                self.object(&f2);
            }
            ObjKind::Blend(b) => {
                self.object(&b.start);
                for step in vector_core::object::blend_steps(b, || 0) {
                    self.object(&step);
                }
                self.object(&b.end);
            }
            ObjKind::Symbol(sid) => {
                if let Some(def) = self.doc.symbols.iter().find(|s| s.id == *sid) {
                    for c in &def.objects {
                        let mut c2 = c.clone();
                        c2.apply_transform(o.transform);
                        self.object(&c2);
                    }
                }
            }
            ObjKind::Image(img) => {
                if let Some(data) = self.doc.images.get(&img.image) {
                    let (iw, ih) = (data.width as f64, data.height as f64);
                    let c = img.crop;
                    let cw = (c[2] - c[0]).max(1e-6);
                    let ch = (c[3] - c[1]).max(1e-6);
                    let to_local = Affine::scale_non_uniform(img.w / (cw * iw), img.h / (ch * ih)) * Affine::translate((-c[0] * iw, -c[1] * ih));
                    let clip = o.transform * Rect::new(0.0, 0.0, img.w, img.h).to_path(0.1);
                    if o.effects.warps_geometry() || img.adjust != ImageAdjust::default() {
                        self.rasterize(o);
                    } else {
                        self.out.push(Prim::Image { image: img.image, xf: o.transform * to_local, clip: clip.clone() });
                    }
                    self.stroke(o, &clip);
                }
            }
            ObjKind::Text(t) => {
                let lay = text::layout(t);
                let space = vector_render::Renderer::fill_space(o, &BezPath::new());
                let groups = lay.colored_paths();
                for (c, local) in &groups {
                    let wp = o.apply_warps(o.transform * local.clone());
                    if o.style.outline.behind_fill {
                        self.stroke(o, &wp);
                    }
                    let paint = match c {
                        Some(c) => Some(Paint::Solid(*c)),
                        None => self.paint(&o.style.fill, space),
                    };
                    if let Some(p) = paint {
                        self.out.push(Prim::Fill { path: wp.clone(), paint: p, even_odd: false });
                    }
                    if !o.style.outline.behind_fill {
                        self.stroke(o, &wp);
                    }
                }
            }
            ObjKind::Table(t) => self.table(o, t),
            ObjKind::Dimension(d) => {
                let path = o.world_path();
                let mut o2 = o.clone();
                if o2.style.outline.color.is_none() {
                    o2.style.outline.color = Some(Color::BLACK);
                }
                self.stroke(&o2, &path);
                let (label, pos, ang) = dimension_label(d);
                let mut tobj = text::TextObj::artistic(&label, text::CharFormat { size: d.font_size, ..Default::default() });
                tobj.align = text::TextAlign::Center;
                let lay = text::layout(&tobj);
                let mut a = ang;
                while a > std::f64::consts::FRAC_PI_2 {
                    a -= std::f64::consts::PI;
                }
                while a < -std::f64::consts::FRAC_PI_2 {
                    a += std::f64::consts::PI;
                }
                let tr = Affine::translate(pos.to_vec2()) * Affine::rotate(a) * Affine::translate((0.0, -d.font_size * 0.3));
                self.out.push(Prim::Fill { path: tr * lay.to_path(), paint: Paint::Solid(o2.style.outline.color.unwrap()), even_odd: false });
            }
            _ => {
                let path = o.world_path();
                let space = vector_render::Renderer::fill_space(o, &path);
                let fill_ok = o.is_closed_shape() || matches!(o.kind, ObjKind::Path(_));
                if o.style.outline.behind_fill {
                    self.stroke(o, &path);
                }
                if fill_ok {
                    if let Some(p) = self.paint(&o.style.fill, space) {
                        self.out.push(Prim::Fill { path: path.clone(), paint: p, even_odd: o.style.even_odd });
                    }
                }
                if let Some(c) = &o.effects.contour {
                    if c.dir != ContourDir::Outside {
                        self.contour(o, c);
                    }
                }
                if !o.style.outline.behind_fill {
                    self.stroke(o, &path);
                }
            }
        }
        if grouped {
            self.out.push(Prim::EndGroup);
        }
        if let Some(url) = &o.url {
            if !url.is_empty() {
                self.out.push(Prim::Link { rect: o.bounds(), url: url.clone() });
            }
        }
    }

    fn paint(&self, f: &Fill, space: Affine) -> Option<Paint> {
        match f {
            Fill::None => None,
            Fill::Solid(c) => Some(Paint::Solid(*c)),
            Fill::Gradient(g) => Some(Paint::Gradient(g.clone(), space)),
            Fill::Pattern(p) => Some(Paint::Solid(p.fg)),
            Fill::Mesh(m) => m.colors.first().map(|c| Paint::Solid(*c)),
        }
    }

    fn stroke(&mut self, o: &Object, path: &BezPath) {
        let ol = &o.style.outline;
        let Some(color) = ol.color else { return };
        if path.elements().is_empty() {
            return;
        }
        if let Some(profile) = &ol.profile {
            let shape = vector_render::variable_width_outline(path, ol.width.max(0.1), profile);
            self.out.push(Prim::Fill { path: shape, paint: Paint::Solid(color), even_odd: false });
            return;
        }
        let closed = o.is_closed_shape();
        let dash: Vec<f64> = ol.dash.iter().map(|d| d * ol.width.max(0.5)).collect();
        if ol.position != OutlinePosition::Center && closed {
            let outline = geom::stroke_to_path(path, ol.width * 2.0, to_join(ol.join), to_cap(ol.cap), &dash);
            let op = if ol.position == OutlinePosition::Inside { geom::BoolOp::Intersect } else { geom::BoolOp::Difference };
            let shape = geom::boolean(&outline, path, op, o.style.even_odd);
            self.out.push(Prim::Fill { path: shape, paint: Paint::Solid(color), even_odd: false });
        } else {
            let style = StrokeStyle { width: ol.width, join: ol.join, cap: ol.cap, dash, miter: ol.miter_limit };
            self.out.push(Prim::Stroke { path: path.clone(), color, style });
        }
        if ol.start_arrow.is_some() || ol.end_arrow.is_some() {
            for p in arrow_paths(path, ol, &self.doc.arrowheads) {
                self.out.push(p.with_color(color));
            }
        }
    }

    fn extrude(&mut self, o: &Object, e: &ExtrudeEffect) {
        let path = o.world_path();
        let back = Affine::translate(e.depth) * path.clone();
        self.out.push(Prim::Fill { path: back, paint: Paint::Solid(e.shade), even_odd: false });
        let light = kurbo::Vec2::new(-0.6, -0.8);
        let mut quads = vec![];
        for (pts, closed) in geom::flatten_polylines(&path, 0.5) {
            let n = pts.len();
            let m = if closed { n } else { n.saturating_sub(1) };
            for k in 0..m {
                let a = pts[k];
                let b = pts[(k + 1) % n];
                let edge = b - a;
                if edge.hypot() < 1e-6 {
                    continue;
                }
                let normal = kurbo::Vec2::new(edge.y, -edge.x).normalize();
                let shade = (normal.dot(light) * 0.5 + 0.5).clamp(0.0, 1.0);
                quads.push(([a, b, b + e.depth, a + e.depth], shade));
            }
        }
        let dir = e.depth.normalize();
        quads.sort_by(|x, y| {
            let dx = (x.0[0].to_vec2() + x.0[1].to_vec2()).dot(dir);
            let dy = (y.0[0].to_vec2() + y.0[1].to_vec2()).dot(dir);
            dy.partial_cmp(&dx).unwrap_or(std::cmp::Ordering::Equal)
        });
        for (q, shade) in quads {
            let mut bp = BezPath::new();
            bp.move_to(q[0]);
            bp.line_to(q[1]);
            bp.line_to(q[2]);
            bp.line_to(q[3]);
            bp.close_path();
            let c = Color::lerp(e.shade, e.color, shade);
            self.out.push(Prim::Fill { path: bp.clone(), paint: Paint::Solid(c), even_odd: false });
            self.out.push(Prim::Stroke { path: bp, color: c, style: StrokeStyle { width: 0.3, join: LineJoin::Round, cap: LineCap::Round, dash: vec![], miter: 4.0 } });
        }
    }

    fn contour(&mut self, o: &Object, c: &ContourEffect) {
        let base = o.world_path();
        let start_fill = o.style.fill.main_color().unwrap_or(Color::WHITE);
        let start_ol = o.style.outline.color.unwrap_or(Color::BLACK);
        let steps = c.steps.clamp(1, 64);
        let mut v = vec![];
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
            v.push((p, Color::lerp(start_fill, c.end_fill, t), Color::lerp(start_ol, c.end_outline, t)));
        }
        if c.dir == ContourDir::Outside {
            v.reverse();
        }
        for (p, f, oc) in v {
            if !o.style.fill.is_none() {
                self.out.push(Prim::Fill { path: p.clone(), paint: Paint::Solid(f), even_odd: true });
            }
            if o.style.outline.is_visible() {
                self.out.push(Prim::Stroke { path: p, color: oc, style: StrokeStyle { width: o.style.outline.width, join: LineJoin::Round, cap: LineCap::Round, dash: vec![], miter: 4.0 } });
            }
        }
    }

    fn table(&mut self, o: &Object, t: &TableObj) {
        let xf = o.transform;
        if let Fill::Solid(c) = &o.style.fill {
            self.out.push(Prim::Fill { path: xf * Rect::new(0.0, 0.0, t.width(), t.height()).to_path(0.1), paint: Paint::Solid(*c), even_odd: false });
        }
        for (r, c) in t.anchors() {
            {
                let cell = t.cell(r, c);
                let rc = t.span_rect(r, c);
                if let Some(f) = cell.fill {
                    self.out.push(Prim::Fill { path: xf * rc.to_path(0.1), paint: Paint::Solid(f), even_odd: false });
                }
                if !cell.text.is_empty() {
                    let tobj = text::TextObj::paragraph(&cell.text, (rc.width() - 4.0).max(1.0), (rc.height() - 4.0).max(1.0), t.text_format.clone());
                    let lay = text::layout(&tobj);
                    let tp = xf * Affine::translate((rc.x0 + 2.0, rc.y0 + 2.0)) * lay.to_path();
                    self.out.push(Prim::BeginClip { path: xf * rc.to_path(0.1) });
                    self.out.push(Prim::Fill { path: tp, paint: Paint::Solid(t.text_format.color.unwrap_or(Color::BLACK)), even_odd: false });
                    self.out.push(Prim::EndClip);
                }
            }
        }
        let bp = t.grid_path();
        self.out.push(Prim::Stroke {
            path: xf * bp,
            color: t.border,
            style: StrokeStyle { width: t.border_width, join: LineJoin::Miter, cap: LineCap::Square, dash: vec![], miter: 4.0 },
        });
    }
}

struct ArrowPrim {
    path: BezPath,
    stroke: Option<f64>,
}

impl ArrowPrim {
    fn with_color(self, c: Color) -> Prim {
        match self.stroke {
            Some(w) => Prim::Stroke { path: self.path, color: c, style: StrokeStyle { width: w, join: LineJoin::Round, cap: LineCap::Round, dash: vec![], miter: 4.0 } },
            None => Prim::Fill { path: self.path, paint: Paint::Solid(c), even_odd: false },
        }
    }
}

fn arrow_paths(path: &BezPath, ol: &Outline, custom: &[vector_core::style::CustomArrow]) -> Vec<ArrowPrim> {
    let mut out = vec![];
    let w = ol.width.max(0.5);
    let polys = geom::flatten_polylines(path, 0.1);
    let mut ends = vec![];
    if let (Some(a), Some((pts, false))) = (ol.start_arrow, polys.first()) {
        if pts.len() >= 2 {
            ends.push((a, pts[0], pts[0] - pts[1]));
        }
    }
    if let (Some(a), Some((pts, false))) = (ol.end_arrow, polys.last()) {
        let n = pts.len();
        if n >= 2 {
            ends.push((a, pts[n - 1], pts[n - 1] - pts[n - 2]));
        }
    }
    for (kind, tip, dir) in ends {
        let d = dir.normalize();
        if !d.x.is_finite() {
            continue;
        }
        let n = kurbo::Vec2::new(-d.y, d.x);
        let s = (w * 4.0).max(4.0);
        let mut bp = BezPath::new();
        let mut stroke = None;
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
                stroke = Some(w);
            }
            ArrowHead::Circle => bp = kurbo::Circle::new(tip, s * 0.35).to_path(0.05),
            ArrowHead::Square => {
                let hs = s * 0.3;
                bp.move_to(tip + d * hs + n * hs);
                bp.line_to(tip - d * hs + n * hs);
                bp.line_to(tip - d * hs - n * hs);
                bp.line_to(tip + d * hs - n * hs);
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
                stroke = Some(w);
            }
            ArrowHead::Custom(id) => {
                let Some(ca) = custom.iter().find(|c| c.id == id) else { continue };
                bp = ca.placed(tip, d, w);
                if !ca.filled {
                    stroke = Some(w);
                }
            }
        }
        out.push(ArrowPrim { path: bp, stroke });
    }
    out
}
