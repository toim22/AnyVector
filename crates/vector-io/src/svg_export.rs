//! SVG export (SVG 1.1 with mix-blend-mode CSS for merge modes).

use crate::flat::{FlatOptions, Flattener, Paint, Prim};
use crate::image_io::{base64, encoded_bytes, encode_png};
use crate::Result;
use kurbo::{Affine, BezPath, PathEl};
use std::fmt::Write;
use std::path::Path;
use vector_core::color::Color;
use vector_core::document::Document;
use vector_core::style::*;

#[derive(Clone, Debug)]
pub struct SvgOptions {
    pub raster_dpi: f64,
    /// Write width/height in this unit suffix ("pt", "in", "mm", "px").
    pub unit: &'static str,
    pub pretty: bool,
}

impl Default for SvgOptions {
    fn default() -> Self {
        SvgOptions { raster_dpi: 300.0, unit: "pt", pretty: true }
    }
}

fn n(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let s = format!("{r}");
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

pub fn path_data(p: &BezPath) -> String {
    let mut s = String::new();
    for el in p.elements() {
        match *el {
            PathEl::MoveTo(a) => write!(s, "M{} {}", n(a.x), n(a.y)).unwrap(),
            PathEl::LineTo(a) => write!(s, "L{} {}", n(a.x), n(a.y)).unwrap(),
            PathEl::QuadTo(a, b) => write!(s, "Q{} {} {} {}", n(a.x), n(a.y), n(b.x), n(b.y)).unwrap(),
            PathEl::CurveTo(a, b, c) => write!(s, "C{} {} {} {} {} {}", n(a.x), n(a.y), n(b.x), n(b.y), n(c.x), n(c.y)).unwrap(),
            PathEl::ClosePath => s.push('Z'),
        }
    }
    s
}

fn color_attr(c: Color) -> (String, Option<String>) {
    let hex = format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b);
    let op = if c.a < 255 { Some(n(c.a as f64 / 255.0)) } else { None };
    (hex, op)
}

fn matrix(a: Affine) -> String {
    let c = a.as_coeffs();
    format!("matrix({} {} {} {} {} {})", n(c[0]), n(c[1]), n(c[2]), n(c[3]), n(c[4]), n(c[5]))
}

fn blend_css(m: MergeMode) -> Option<&'static str> {
    Some(match m {
        MergeMode::Normal => return None,
        MergeMode::Multiply => "multiply",
        MergeMode::Screen => "screen",
        MergeMode::Overlay => "overlay",
        MergeMode::Darken => "darken",
        MergeMode::Lighten => "lighten",
        MergeMode::ColorDodge => "color-dodge",
        MergeMode::ColorBurn => "color-burn",
        MergeMode::HardLight => "hard-light",
        MergeMode::SoftLight => "soft-light",
        MergeMode::Difference => "difference",
        MergeMode::Exclusion => "exclusion",
        MergeMode::Hue => "hue",
        MergeMode::Saturation => "saturation",
        MergeMode::Color => "color",
        MergeMode::Luminosity => "luminosity",
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Export one page to an SVG string.
pub fn page_to_svg(doc: &Document, page: usize, opts: &SvgOptions) -> String {
    let pg = &doc.pages[page];
    let mut f = Flattener::new(doc, FlatOptions { raster_dpi: opts.raster_dpi, conical_gradients: false, repeat_gradients: true, gradient_alpha: true, printing: false });
    f.page(page);
    let prims = std::mem::take(&mut f.out);
    let (w, h) = match opts.unit {
        "in" => (pg.width / 72.0, pg.height / 72.0),
        "mm" => (pg.width / 72.0 * 25.4, pg.height / 72.0 * 25.4),
        "px" => (pg.width / 0.75, pg.height / 0.75),
        _ => (pg.width, pg.height),
    };
    let nl = if opts.pretty { "\n" } else { "" };
    let mut defs = String::new();
    let mut body = String::new();
    let mut gid = 0usize;
    let mut depth = 1usize;
    let ind = |d: usize| if opts.pretty { "  ".repeat(d) } else { String::new() };
    for p in prims {
        match p {
            Prim::Fill { path, paint, even_odd } => {
                let d = path_data(&path);
                if d.is_empty() {
                    continue;
                }
                let (fill, op) = match paint {
                    Paint::Solid(c) => color_attr(c),
                    Paint::Gradient(g, space) => {
                        gid += 1;
                        let id = format!("g{gid}");
                        write_gradient(&mut defs, &id, &g, space, nl);
                        (format!("url(#{id})"), None)
                    }
                };
                write!(body, "{}<path d=\"{}\" fill=\"{}\"", ind(depth), d, fill).unwrap();
                if let Some(o) = op {
                    write!(body, " fill-opacity=\"{o}\"").unwrap();
                }
                if even_odd {
                    body.push_str(" fill-rule=\"evenodd\"");
                }
                write!(body, "/>{nl}").unwrap();
            }
            Prim::Stroke { path, color, style } => {
                let d = path_data(&path);
                if d.is_empty() {
                    continue;
                }
                let (c, op) = color_attr(color);
                let width = if style.width <= 0.0 { 0.25 } else { style.width };
                write!(body, "{}<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"", ind(depth), d, c, n(width)).unwrap();
                if style.width <= 0.0 {
                    body.push_str(" vector-effect=\"non-scaling-stroke\"");
                }
                if let Some(o) = op {
                    write!(body, " stroke-opacity=\"{o}\"").unwrap();
                }
                match style.join {
                    LineJoin::Miter => write!(body, " stroke-miterlimit=\"{}\"", n(style.miter)).unwrap(),
                    LineJoin::Round => body.push_str(" stroke-linejoin=\"round\""),
                    LineJoin::Bevel => body.push_str(" stroke-linejoin=\"bevel\""),
                }
                match style.cap {
                    LineCap::Butt => {}
                    LineCap::Round => body.push_str(" stroke-linecap=\"round\""),
                    LineCap::Square => body.push_str(" stroke-linecap=\"square\""),
                }
                if !style.dash.is_empty() {
                    let d: Vec<String> = style.dash.iter().map(|v| n(*v)).collect();
                    write!(body, " stroke-dasharray=\"{}\"", d.join(" ")).unwrap();
                }
                write!(body, "/>{nl}").unwrap();
            }
            Prim::Image { image, xf, clip } => {
                let Some(img) = doc.images.get(&image) else { continue };
                let (bytes, ext) = encoded_bytes(img);
                let mime = if ext == "jpg" { "image/jpeg" } else { "image/png" };
                gid += 1;
                write!(defs, "<clipPath id=\"c{gid}\"><path d=\"{}\"/></clipPath>{nl}", path_data(&clip)).unwrap();
                write!(
                    body,
                    "{}<g clip-path=\"url(#c{gid})\"><image width=\"{}\" height=\"{}\" transform=\"{}\" preserveAspectRatio=\"none\" xlink:href=\"data:{};base64,{}\"/></g>{nl}",
                    ind(depth),
                    img.width,
                    img.height,
                    matrix(xf),
                    mime,
                    base64(&bytes)
                )
                .unwrap();
            }
            Prim::Raster { rect, w, h, rgba } => {
                let png = encode_png(w, h, &rgba);
                write!(
                    body,
                    "{}<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" xlink:href=\"data:image/png;base64,{}\"/>{nl}",
                    ind(depth),
                    n(rect.x0),
                    n(rect.y0),
                    n(rect.width()),
                    n(rect.height()),
                    base64(&png)
                )
                .unwrap();
            }
            Prim::BeginGroup { opacity, blend } => {
                write!(body, "{}<g", ind(depth)).unwrap();
                if opacity < 0.999 {
                    write!(body, " opacity=\"{}\"", n(opacity as f64)).unwrap();
                }
                if let Some(b) = blend_css(blend) {
                    write!(body, " style=\"mix-blend-mode:{b}\"").unwrap();
                }
                write!(body, ">{nl}").unwrap();
                depth += 1;
            }
            Prim::EndGroup | Prim::EndClip => {
                depth = depth.saturating_sub(1).max(1);
                write!(body, "{}</g>{nl}", ind(depth)).unwrap();
            }
            Prim::BeginClip { path } => {
                gid += 1;
                write!(defs, "<clipPath id=\"c{gid}\"><path d=\"{}\"/></clipPath>{nl}", path_data(&path)).unwrap();
                write!(body, "{}<g clip-path=\"url(#c{gid})\">{nl}", ind(depth)).unwrap();
                depth += 1;
            }
            Prim::Link { rect, url } => {
                write!(
                    body,
                    "{}<a xlink:href=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"transparent\"/></a>{nl}",
                    ind(depth),
                    esc(&url),
                    n(rect.x0),
                    n(rect.y0),
                    n(rect.width()),
                    n(rect.height())
                )
                .unwrap();
            }
        }
    }
    let mut s = String::new();
    write!(s, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>{nl}").unwrap();
    write!(s, "<!-- Created with {} {} -->{nl}", vector_core::APP_NAME, vector_core::APP_VERSION).unwrap();
    write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" version=\"1.1\" width=\"{}{}\" height=\"{}{}\" viewBox=\"0 0 {} {}\">{nl}",
        n(w),
        opts.unit,
        n(h),
        opts.unit,
        n(pg.width),
        n(pg.height)
    )
    .unwrap();
    if !doc.info.title.is_empty() {
        write!(s, "  <title>{}</title>{nl}", esc(&doc.info.title)).unwrap();
    }
    if !defs.is_empty() {
        write!(s, "  <defs>{nl}{defs}  </defs>{nl}").unwrap();
    }
    s.push_str(&body);
    s.push_str("</svg>\n");
    s
}

fn write_gradient(defs: &mut String, id: &str, g: &Gradient, space: Affine, nl: &str) {
    let spread = match g.spread {
        SpreadMode::Pad => "pad",
        SpreadMode::Repeat => "repeat",
        SpreadMode::Reflect => "reflect",
    };
    match g.kind {
        GradientKind::Elliptical => {
            let r = (g.end - g.start).hypot();
            write!(
                defs,
                "<radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" cx=\"{}\" cy=\"{}\" r=\"{}\" spreadMethod=\"{spread}\" gradientTransform=\"{}\">",
                n(g.start.x),
                n(g.start.y),
                n(r),
                matrix(space)
            )
            .unwrap();
        }
        _ => {
            write!(
                defs,
                "<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" spreadMethod=\"{spread}\" gradientTransform=\"{}\">",
                n(g.start.x),
                n(g.start.y),
                n(g.end.x),
                n(g.end.y),
                matrix(space)
            )
            .unwrap();
        }
    }
    for st in g.sorted_stops() {
        let (c, op) = color_attr(st.color);
        write!(defs, "<stop offset=\"{}\" stop-color=\"{}\"", n(st.pos.clamp(0.0, 1.0)), c).unwrap();
        if let Some(o) = op {
            write!(defs, " stop-opacity=\"{o}\"").unwrap();
        }
        defs.push_str("/>");
    }
    match g.kind {
        GradientKind::Elliptical => write!(defs, "</radialGradient>{nl}").unwrap(),
        _ => write!(defs, "</linearGradient>{nl}").unwrap(),
    }
}

pub fn export(doc: &Document, page: usize, path: &Path, opts: &SvgOptions) -> Result<()> {
    std::fs::write(path, page_to_svg(doc, page, opts))?;
    Ok(())
}

/// SVG for a list of objects (clipboard export).
pub fn objects_to_svg(doc: &Document, objs: &[vector_core::Object]) -> String {
    let b = vector_core::object::union_bounds(objs.iter().map(|o| o.visual_bounds()));
    let mut d = Document::new(b.width().max(1.0), b.height().max(1.0));
    d.images = doc.images.clone();
    d.symbols = doc.symbols.clone();
    for o in objs {
        let mut o2 = o.clone();
        o2.apply_transform(Affine::translate((-b.x0, -b.y0)));
        d.pages[0].layers[0].objects.push(o2);
    }
    page_to_svg(&d, 0, &SvgOptions::default())
}
