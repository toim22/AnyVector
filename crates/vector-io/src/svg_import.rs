//! SVG import through usvg: paths, groups (opacity/blend), clip paths (as PowerClips),
//! gradients, images and text (converted to curves).

use crate::{image_io, IoError, Result};
use kurbo::{Affine, BezPath, Point, Shape};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use tiny_skia::PathSegment;
use vector_core::color::Color;
use vector_core::document::Document;
use vector_core::object::{ImageAdjust, ImageObj, ObjKind, Object};
use vector_core::path::NodePath;
use vector_core::style::*;

/// SVG user units (px at 96 dpi) to points.
const PX: f64 = 0.75;

fn fontdb() -> Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
    .clone()
}

pub fn parse_tree(data: &[u8]) -> Result<usvg::Tree> {
    parse_tree_in(data, None)
}

/// [`parse_tree`] for an SVG file in `dir`: its linked images (`<image href="images/logo.png">`)
/// are read from beside it rather than from the working folder, and a link written on another
/// system (`images\logo.png`, `C:\Art\Logo.png`, `file:///C:/Art/logo.png`, or a name in another
/// case) is looked for there by name.
fn parse_tree_in(data: &[u8], dir: Option<&Path>) -> Result<usvg::Tree> {
    let mut opt = usvg::Options { fontdb: fontdb(), resources_dir: dir.map(Path::to_path_buf), ..Default::default() };
    if let Some(dir) = dir.map(Path::to_path_buf) {
        let default = usvg::ImageHrefResolver::default_string_resolver();
        opt.image_href_resolver.resolve_string = Box::new(move |href: &str, o: &usvg::Options| {
            default(href, o).or_else(|| {
                let found = crate::paths::resolve_stored_path(&file_url_path(href)?, &[dir.as_path()])?;
                default(&found.to_string_lossy(), o)
            })
        });
    }
    usvg::Tree::from_data(data, &opt).map_err(|e| IoError::Format(e.to_string()))
}

/// The path in an image link: the link itself, or the path of a `file:` URL with `%20` and the
/// like decoded (`file:///C:/My%20Art/a.png` gives `C:/My Art/a.png`, `file:///home/a.png` gives
/// `/home/a.png`). None for other URLs (`http:`, …), which are not read.
fn file_url_path(href: &str) -> Option<String> {
    let is_file_url = href.get(..5).is_some_and(|s| s.eq_ignore_ascii_case("file:"));
    if !is_file_url {
        // `C:\…` has a drive letter, not a scheme
        let scheme = href.split_once(':').map(|(s, _)| s).unwrap_or("");
        let url = scheme.len() > 1 && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c));
        return (!url).then(|| href.to_string());
    }
    let mut rest = &href[5..];
    rest = rest.strip_prefix("//").unwrap_or(rest);
    let b = rest.as_bytes();
    if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
        rest = &rest[1..];
    }
    let bytes = rest.as_bytes();
    let hex = |b: Option<&u8>| b.and_then(|b| (*b as char).to_digit(16));
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match (bytes[i], hex(bytes.get(i + 1)), hex(bytes.get(i + 2))) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (c, _, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// Open an SVG file as a document; images it links are read from beside it.
pub fn document_from_svg_file(path: &Path) -> Result<Document> {
    document_from_svg_in(&std::fs::read(path)?, path.parent())
}

/// [`document_from_svg`] for an SVG read from folder `dir` (see [`parse_tree_in`]).
pub fn document_from_svg_in(data: &[u8], dir: Option<&Path>) -> Result<Document> {
    document_from_tree(parse_tree_in(data, dir)?)
}

pub fn document_from_svg(data: &[u8]) -> Result<Document> {
    document_from_tree(parse_tree(data)?)
}

fn document_from_tree(tree: usvg::Tree) -> Result<Document> {
    let size = tree.size();
    let mut doc = Document::new((size.width() as f64 * PX).max(1.0), (size.height() as f64 * PX).max(1.0));
    let mut objs = vec![];
    let mut cv = Converter { doc: &mut doc };
    for n in tree.root().children() {
        cv.node(n, &mut objs);
    }
    doc.pages[0].layers[0].objects = objs;
    Ok(doc)
}

/// Import an SVG as a list of objects into an existing document.
pub fn objects_from_svg(doc: &mut Document, data: &[u8]) -> Result<Vec<Object>> {
    let tree = parse_tree(data)?;
    let mut objs = vec![];
    let mut cv = Converter { doc };
    for n in tree.root().children() {
        cv.node(n, &mut objs);
    }
    Ok(objs)
}

fn affine(t: usvg::Transform) -> Affine {
    Affine::scale(PX) * Affine::new([t.sx as f64, t.ky as f64, t.kx as f64, t.sy as f64, t.tx as f64, t.ty as f64])
}

fn bez(p: &tiny_skia::Path) -> BezPath {
    let mut b = BezPath::new();
    for s in p.segments() {
        match s {
            PathSegment::MoveTo(a) => b.move_to((a.x as f64, a.y as f64)),
            PathSegment::LineTo(a) => b.line_to((a.x as f64, a.y as f64)),
            PathSegment::QuadTo(c, a) => b.quad_to((c.x as f64, c.y as f64), (a.x as f64, a.y as f64)),
            PathSegment::CubicTo(c1, c2, a) => b.curve_to((c1.x as f64, c1.y as f64), (c2.x as f64, c2.y as f64), (a.x as f64, a.y as f64)),
            PathSegment::Close => b.close_path(),
        }
    }
    b
}

fn color(c: usvg::Color, opacity: f32) -> Color {
    Color::rgba(c.red, c.green, c.blue, (opacity.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn blend(m: usvg::BlendMode) -> MergeMode {
    use usvg::BlendMode as B;
    match m {
        B::Normal | B::PlusLighter => MergeMode::Normal,
        B::Multiply => MergeMode::Multiply,
        B::Screen => MergeMode::Screen,
        B::Overlay => MergeMode::Overlay,
        B::Darken => MergeMode::Darken,
        B::Lighten => MergeMode::Lighten,
        B::ColorDodge => MergeMode::ColorDodge,
        B::ColorBurn => MergeMode::ColorBurn,
        B::HardLight => MergeMode::HardLight,
        B::SoftLight => MergeMode::SoftLight,
        B::Difference => MergeMode::Difference,
        B::Exclusion => MergeMode::Exclusion,
        B::Hue => MergeMode::Hue,
        B::Saturation => MergeMode::Saturation,
        B::Color => MergeMode::Color,
        B::Luminosity => MergeMode::Luminosity,
    }
}

struct Converter<'a> {
    doc: &'a mut Document,
}

impl<'a> Converter<'a> {
    fn node(&mut self, n: &usvg::Node, out: &mut Vec<Object>) {
        match n {
            usvg::Node::Group(g) => self.group(g, out),
            usvg::Node::Path(p) => {
                if let Some(o) = self.path(p) {
                    out.push(o);
                }
            }
            usvg::Node::Image(img) => self.image(img, out),
            usvg::Node::Text(t) => self.group(t.flattened(), out),
        }
    }

    fn group(&mut self, g: &usvg::Group, out: &mut Vec<Object>) {
        let mut children = vec![];
        for c in g.children() {
            self.node(c, &mut children);
        }
        if children.is_empty() {
            return;
        }
        let opacity = g.opacity().get();
        let merge = blend(g.blend_mode());
        // clip path → PowerClip
        if let Some(cp) = g.clip_path() {
            let mut frame_paths = vec![];
            collect_paths(cp.root(), &mut frame_paths);
            let xf = affine(g.abs_transform()) * {
                let t = cp.transform();
                Affine::new([t.sx as f64, t.ky as f64, t.kx as f64, t.sy as f64, t.tx as f64, t.ty as f64])
            };
            let _ = xf;
            if !frame_paths.is_empty() {
                let mut fp = BezPath::new();
                for p in frame_paths {
                    fp.extend(p.elements().iter().copied());
                }
                let id = self.doc.new_id();
                let mut st = Style::default();
                st.outline = Outline::none();
                let frame = Object::new(id, ObjKind::Path(NodePath::from_bez(&fp)), Affine::IDENTITY, st);
                let id = self.doc.new_id();
                let mut pc = Object::new(id, ObjKind::PowerClip { frame: Box::new(frame), contents: children }, Affine::IDENTITY, Style::default());
                pc.style.outline = Outline::none();
                apply_group_style(&mut pc, opacity, merge);
                out.push(pc);
                return;
            }
        }
        if children.len() == 1 && opacity >= 0.999 && merge == MergeMode::Normal && g.id().is_empty() {
            out.extend(children);
            return;
        }
        if opacity < 0.999 || merge != MergeMode::Normal {
            if children.len() == 1 {
                let mut c = children.pop().unwrap();
                apply_group_style(&mut c, opacity, merge);
                out.push(c);
                return;
            }
        }
        let id = self.doc.new_id();
        let mut grp = Object::new(id, ObjKind::Group(children), Affine::IDENTITY, Style::default());
        grp.name = g.id().to_string();
        apply_group_style(&mut grp, opacity, merge);
        out.push(grp);
    }

    fn path(&mut self, p: &usvg::Path) -> Option<Object> {
        if !p.is_visible() {
            return None;
        }
        let xf = affine(p.abs_transform());
        let path = xf * bez(p.data());
        if path.elements().is_empty() {
            return None;
        }
        let bb = path.bounding_box();
        let to_unit = |pt: Point| -> Point { Point::new((pt.x - bb.x0) / bb.width().max(1e-9), (pt.y - bb.y0) / bb.height().max(1e-9)) };
        let mut style = Style::default();
        if let Some(fill) = p.fill() {
            style.even_odd = fill.rule() == usvg::FillRule::EvenOdd;
            style.fill = paint_to_fill(fill.paint(), fill.opacity().get(), xf, &to_unit);
        } else {
            style.fill = Fill::None;
        }
        if let Some(s) = p.stroke() {
            let scale = (xf.determinant().abs()).sqrt();
            let width = s.width().get() as f64 * scale / PX * PX;
            let c = match paint_to_fill(s.paint(), s.opacity().get(), xf, &to_unit) {
                Fill::Solid(c) => c,
                Fill::Gradient(g) => g.color_at(0.5),
                f => f.main_color().unwrap_or(Color::BLACK),
            };
            style.outline = Outline {
                width,
                color: Some(c),
                join: match s.linejoin() {
                    usvg::LineJoin::Round => LineJoin::Round,
                    usvg::LineJoin::Bevel => LineJoin::Bevel,
                    _ => LineJoin::Miter,
                },
                cap: match s.linecap() {
                    usvg::LineCap::Round => LineCap::Round,
                    usvg::LineCap::Square => LineCap::Square,
                    usvg::LineCap::Butt => LineCap::Butt,
                },
                dash: s.dasharray().map(|d| d.iter().map(|v| *v as f64 * scale / width.max(0.01)).collect()).unwrap_or_default(),
                miter_limit: s.miterlimit().get() as f64,
                ..Default::default()
            };
            if p.paint_order() == usvg::PaintOrder::StrokeAndFill {
                style.outline.behind_fill = true;
            }
        } else {
            style.outline = Outline::none();
        }
        let id = self.doc.new_id();
        let mut o = Object::new(id, ObjKind::Path(NodePath::from_bez(&path)), Affine::IDENTITY, style);
        o.name = p.id().to_string();
        Some(o)
    }

    fn image(&mut self, img: &usvg::Image, out: &mut Vec<Object>) {
        if !img.is_visible() {
            return;
        }
        let xf = affine(img.abs_transform());
        let size = img.size();
        match img.kind() {
            usvg::ImageKind::SVG(tree) => {
                let mut children = vec![];
                for n in tree.root().children() {
                    self.node(n, &mut children);
                }
                let s = Affine::scale_non_uniform(size.width() as f64 / tree.size().width() as f64, size.height() as f64 / tree.size().height() as f64);
                for c in children.iter_mut() {
                    // children are already in points relative to the nested tree; place them
                    c.apply_transform(xf * Affine::scale(1.0 / PX) * s);
                }
                out.extend(children);
            }
            usvg::ImageKind::JPEG(d) | usvg::ImageKind::PNG(d) | usvg::ImageKind::GIF(d) | usvg::ImageKind::WEBP(d) => {
                let Ok(data) = image_io::decode(d, img.id()) else { return };
                let iid = self.doc.add_image(data);
                let id = self.doc.new_id();
                let mut st = Style::default();
                st.outline = Outline::none();
                let o = Object::new(
                    id,
                    ObjKind::Image(ImageObj { image: iid, w: size.width() as f64, h: size.height() as f64, crop: [0.0, 0.0, 1.0, 1.0], adjust: ImageAdjust::default() }),
                    xf,
                    st,
                );
                out.push(o);
            }
        }
    }
}

fn apply_group_style(o: &mut Object, opacity: f32, merge: MergeMode) {
    if opacity < 0.999 {
        let prev = o.style.transparency.uniform_opacity();
        o.style.transparency = Transparency::Uniform(((1.0 - prev * opacity) * 100.0) as f64);
    }
    if merge != MergeMode::Normal {
        o.style.merge = merge;
    }
}

fn collect_paths(g: &usvg::Group, out: &mut Vec<BezPath>) {
    for n in g.children() {
        match n {
            usvg::Node::Path(p) => out.push(affine(p.abs_transform()) * bez(p.data())),
            usvg::Node::Group(g) => collect_paths(g, out),
            usvg::Node::Text(t) => collect_paths(t.flattened(), out),
            _ => {}
        }
    }
}

fn paint_to_fill(p: &usvg::Paint, opacity: f32, xf: Affine, to_unit: &dyn Fn(Point) -> Point) -> Fill {
    let stops = |base: &usvg::BaseGradient| -> Vec<GradientStop> {
        base.stops().iter().map(|s| GradientStop { pos: s.offset().get() as f64, color: color(s.color(), s.opacity().get() * opacity) }).collect()
    };
    let spread = |m: usvg::SpreadMethod| match m {
        usvg::SpreadMethod::Pad => SpreadMode::Pad,
        usvg::SpreadMethod::Reflect => SpreadMode::Reflect,
        usvg::SpreadMethod::Repeat => SpreadMode::Repeat,
    };
    let gt = |t: usvg::Transform| Affine::new([t.sx as f64, t.ky as f64, t.kx as f64, t.sy as f64, t.tx as f64, t.ty as f64]);
    match p {
        usvg::Paint::Color(c) => Fill::Solid(color(*c, opacity)),
        usvg::Paint::LinearGradient(lg) => {
            let m = xf * gt(lg.transform());
            let a = to_unit(m * Point::new(lg.x1() as f64, lg.y1() as f64));
            let b = to_unit(m * Point::new(lg.x2() as f64, lg.y2() as f64));
            Fill::Gradient(Gradient { kind: GradientKind::Linear, stops: stops(lg), start: a, end: b, spread: spread(lg.spread_method()), steps: 0, accel: 0.0, blend: 0, smooth: false, free_scale: true })
        }
        usvg::Paint::RadialGradient(rg) => {
            let m = xf * gt(rg.transform());
            let c = to_unit(m * Point::new(rg.cx() as f64, rg.cy() as f64));
            let e = to_unit(m * Point::new(rg.cx() as f64 + rg.r().get() as f64, rg.cy() as f64));
            Fill::Gradient(Gradient { kind: GradientKind::Elliptical, stops: stops(rg), start: c, end: e, spread: spread(rg.spread_method()), steps: 0, accel: 0.0, blend: 0, smooth: false, free_scale: true })
        }
        usvg::Paint::Pattern(_) => Fill::Solid(Color::rgba(128, 128, 128, (opacity * 255.0) as u8)),
    }
}

#[cfg(test)]
mod tests {
    use super::file_url_path;

    #[test]
    fn image_links_give_paths() {
        assert_eq!(file_url_path("images/logo.png").as_deref(), Some("images/logo.png"));
        assert_eq!(file_url_path(r"C:\Art\Logo.png").as_deref(), Some(r"C:\Art\Logo.png"));
        assert_eq!(file_url_path("file:///C:/My%20Art/a.png").as_deref(), Some("C:/My Art/a.png"));
        assert_eq!(file_url_path("FILE:///c:/x.png").as_deref(), Some("c:/x.png"));
        assert_eq!(file_url_path("file:///home/ana/a%2Bb.png").as_deref(), Some("/home/ana/a+b.png"));
        assert_eq!(file_url_path("file:/home/ana/a.png").as_deref(), Some("/home/ana/a.png"));
        assert_eq!(file_url_path("100%.png").as_deref(), Some("100%.png"));
        assert_eq!(file_url_path("https://example.com/a.png"), None);
    }
}
