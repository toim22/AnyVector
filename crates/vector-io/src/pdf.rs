//! PDF 1.4 writer (Publish to PDF). Multi-page, vector paths, axial/radial shadings,
//! transparency groups with blend modes, images (JPEG passthrough / Flate + SMask),
//! hyperlinks and document info. Text is written as curves.

use crate::flat::{FlatOptions, Flattener, Paint, Prim, StrokeStyle};
use crate::Result;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use kurbo::{Affine, BezPath, PathEl};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write;
use std::path::Path;
use vector_core::color::Color;
use vector_core::document::Document;
use vector_core::style::*;

#[derive(Clone, Debug)]
pub struct PdfOptions {
    /// Page indices to export; empty = all.
    pub pages: Vec<usize>,
    pub raster_dpi: f64,
    pub compress: bool,
    pub jpeg_images: bool,
    pub include_hyperlinks: bool,
    pub printing: bool,
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions { pages: vec![], raster_dpi: 300.0, compress: true, jpeg_images: true, include_hyperlinks: true, printing: true }
    }
}

struct Writer {
    buf: Vec<u8>,
    offsets: Vec<usize>,
}

impl Writer {
    fn alloc(&mut self) -> usize {
        self.offsets.push(0);
        self.offsets.len()
    }
    fn begin(&mut self, id: usize) {
        self.offsets[id - 1] = self.buf.len();
        write!(self.buf, "{id} 0 obj\n").unwrap();
    }
    fn obj(&mut self, id: usize, body: &str) {
        self.begin(id);
        self.buf.extend_from_slice(body.as_bytes());
        self.buf.extend_from_slice(b"\nendobj\n");
    }
    fn stream(&mut self, id: usize, dict: &str, data: &[u8], compress: bool) {
        let (data, filter) = if compress {
            let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
            e.write_all(data).unwrap();
            (e.finish().unwrap(), " /Filter /FlateDecode")
        } else {
            (data.to_vec(), "")
        };
        self.begin(id);
        write!(self.buf, "<< {dict}{filter} /Length {} >>\nstream\n", data.len()).unwrap();
        self.buf.extend_from_slice(&data);
        self.buf.extend_from_slice(b"\nendstream\nendobj\n");
    }
    fn raw_stream(&mut self, id: usize, dict: &str, data: &[u8]) {
        self.begin(id);
        write!(self.buf, "<< {dict} /Length {} >>\nstream\n", data.len()).unwrap();
        self.buf.extend_from_slice(data);
        self.buf.extend_from_slice(b"\nendstream\nendobj\n");
    }
}

fn f(v: f64) -> String {
    let r = (v * 10000.0).round() / 10000.0;
    let s = format!("{r}");
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn pdf_string(s: &str) -> String {
    // UTF-16BE with BOM for non-ASCII text
    if s.is_ascii() {
        let e = s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
        format!("({e})")
    } else {
        let mut h = String::from("<FEFF");
        for u in s.encode_utf16() {
            write!(h, "{u:04X}").unwrap();
        }
        h.push('>');
        h
    }
}

fn rgb(c: Color) -> String {
    format!("{} {} {}", f(c.r as f64 / 255.0), f(c.g as f64 / 255.0), f(c.b as f64 / 255.0))
}

fn path_ops(p: &BezPath, out: &mut String) {
    for el in p.elements() {
        match *el {
            PathEl::MoveTo(a) => writeln!(out, "{} {} m", f(a.x), f(a.y)).unwrap(),
            PathEl::LineTo(a) => writeln!(out, "{} {} l", f(a.x), f(a.y)).unwrap(),
            PathEl::QuadTo(c, b) => {
                // elevate (needs current point; approximate by treating as cubic with same control)
                writeln!(out, "{} {} {} {} v", f(c.x), f(c.y), f(b.x), f(b.y)).unwrap()
            }
            PathEl::CurveTo(a, b, c) => writeln!(out, "{} {} {} {} {} {} c", f(a.x), f(a.y), f(b.x), f(b.y), f(c.x), f(c.y)).unwrap(),
            PathEl::ClosePath => out.push_str("h\n"),
        }
    }
}

/// Convert quads to cubics so the PDF path operators are exact.
fn cubic_only(p: &BezPath) -> BezPath {
    let mut out = BezPath::new();
    let mut cur = kurbo::Point::ZERO;
    let mut start = cur;
    for el in p.elements() {
        match *el {
            PathEl::MoveTo(a) => {
                out.move_to(a);
                cur = a;
                start = a;
            }
            PathEl::LineTo(a) => {
                out.line_to(a);
                cur = a;
            }
            PathEl::QuadTo(c, b) => {
                let c1 = cur + (c - cur) * (2.0 / 3.0);
                let c2 = b + (c - b) * (2.0 / 3.0);
                out.curve_to(c1, c2, b);
                cur = b;
            }
            PathEl::CurveTo(a, b, c) => {
                out.curve_to(a, b, c);
                cur = c;
            }
            PathEl::ClosePath => {
                out.close_path();
                cur = start;
            }
        }
    }
    out
}

fn blend_name(m: MergeMode) -> &'static str {
    match m {
        MergeMode::Normal => "Normal",
        MergeMode::Multiply => "Multiply",
        MergeMode::Screen => "Screen",
        MergeMode::Overlay => "Overlay",
        MergeMode::Darken => "Darken",
        MergeMode::Lighten => "Lighten",
        MergeMode::ColorDodge => "ColorDodge",
        MergeMode::ColorBurn => "ColorBurn",
        MergeMode::HardLight => "HardLight",
        MergeMode::SoftLight => "SoftLight",
        MergeMode::Difference => "Difference",
        MergeMode::Exclusion => "Exclusion",
        MergeMode::Hue => "Hue",
        MergeMode::Saturation => "Saturation",
        MergeMode::Color => "Color",
        MergeMode::Luminosity => "Luminosity",
    }
}

/// Per-page resource collection.
#[derive(Default)]
struct Res {
    ext_g: Vec<(String, usize)>,
    shadings: Vec<(String, usize)>,
    xobjects: Vec<(String, usize)>,
}

struct Ctx<'a> {
    w: Writer,
    doc: &'a Document,
    opts: &'a PdfOptions,
    image_cache: HashMap<u64, usize>,
    gs_cache: HashMap<String, usize>,
    counter: usize,
}

impl<'a> Ctx<'a> {
    fn name(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("{prefix}{}", self.counter)
    }

    fn ext_gstate(&mut self, res: &mut Res, opacity_fill: f32, opacity_stroke: f32, blend: MergeMode) -> String {
        let key = format!("{opacity_fill}/{opacity_stroke}/{:?}", blend);
        let id = if let Some(id) = self.gs_cache.get(&key) {
            *id
        } else {
            let id = self.w.alloc();
            self.w.obj(id, &format!("<< /Type /ExtGState /ca {} /CA {} /BM /{} >>", f(opacity_fill as f64), f(opacity_stroke as f64), blend_name(blend)));
            self.gs_cache.insert(key, id);
            id
        };
        if let Some((n, _)) = res.ext_g.iter().find(|(_, i)| *i == id) {
            return n.clone();
        }
        let n = self.name("GS");
        res.ext_g.push((n.clone(), id));
        n
    }

    fn function(&mut self, g: &Gradient) -> usize {
        let mut stops = g.sorted_stops();
        if stops[0].pos > 0.0 {
            let c = stops[0].color;
            stops.insert(0, GradientStop { pos: 0.0, color: c });
        }
        if stops.last().unwrap().pos < 1.0 {
            let c = stops.last().unwrap().color;
            stops.push(GradientStop { pos: 1.0, color: c });
        }
        for i in 1..stops.len() {
            if stops[i].pos <= stops[i - 1].pos {
                stops[i].pos = (stops[i - 1].pos + 1e-4).min(1.0);
            }
        }
        let mut subs = vec![];
        for w in stops.windows(2) {
            let id = self.w.alloc();
            self.w.obj(id, &format!("<< /FunctionType 2 /Domain [0 1] /C0 [{}] /C1 [{}] /N 1 >>", rgb(w[0].color), rgb(w[1].color)));
            subs.push(id);
        }
        if subs.len() == 1 {
            return subs[0];
        }
        let bounds: Vec<String> = stops[1..stops.len() - 1].iter().map(|s| f(s.pos)).collect();
        let encode: Vec<&str> = subs.iter().flat_map(|_| ["0", "1"]).collect();
        let fns: Vec<String> = subs.iter().map(|i| format!("{i} 0 R")).collect();
        let id = self.w.alloc();
        self.w.obj(id, &format!("<< /FunctionType 3 /Domain [0 1] /Functions [{}] /Bounds [{}] /Encode [{}] >>", fns.join(" "), bounds.join(" "), encode.join(" ")));
        id
    }

    fn shading(&mut self, res: &mut Res, g: &Gradient) -> String {
        let func = self.function(g);
        let id = self.w.alloc();
        let body = match g.kind {
            GradientKind::Elliptical => {
                let r = (g.end - g.start).hypot();
                format!("<< /ShadingType 3 /ColorSpace /DeviceRGB /Coords [{} {} 0 {} {} {}] /Function {func} 0 R /Extend [true true] >>", f(g.start.x), f(g.start.y), f(g.start.x), f(g.start.y), f(r))
            }
            _ => format!(
                "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [{} {} {} {}] /Function {func} 0 R /Extend [true true] >>",
                f(g.start.x),
                f(g.start.y),
                f(g.end.x),
                f(g.end.y)
            ),
        };
        self.w.obj(id, &body);
        let n = self.name("Sh");
        res.shadings.push((n.clone(), id));
        n
    }

    /// Image XObject from straight RGBA (Flate) with an SMask if needed.
    fn image_rgba(&mut self, w: u32, h: u32, rgba: &[u8]) -> usize {
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        let mut alpha = Vec::with_capacity((w * h) as usize);
        let mut has_alpha = false;
        for px in rgba.chunks_exact(4) {
            rgb.extend_from_slice(&px[..3]);
            alpha.push(px[3]);
            if px[3] < 255 {
                has_alpha = true;
            }
        }
        let smask = if has_alpha {
            let id = self.w.alloc();
            self.w.stream(id, &format!("/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8"), &alpha, true);
            format!(" /SMask {id} 0 R")
        } else {
            String::new()
        };
        let id = self.w.alloc();
        self.w.stream(id, &format!("/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Interpolate true{smask}"), &rgb, true);
        id
    }

    fn doc_image(&mut self, image: u64) -> Option<usize> {
        if let Some(id) = self.image_cache.get(&image) {
            return Some(*id);
        }
        let img = self.doc.images.get(&image)?;
        let opaque = img.rgba.chunks_exact(4).all(|p| p[3] == 255);
        let id = if self.opts.jpeg_images && img.format == "jpg" && !img.encoded.is_empty() && opaque {
            let id = self.w.alloc();
            let data = img.encoded.clone();
            self.w.raw_stream(id, &format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode", img.width, img.height), &data);
            id
        } else {
            let (w, h, rgba) = (img.width, img.height, img.rgba.clone());
            self.image_rgba(w, h, &rgba)
        };
        self.image_cache.insert(image, id);
        Some(id)
    }

    fn xobject_name(&mut self, res: &mut Res, id: usize) -> String {
        if let Some((n, _)) = res.xobjects.iter().find(|(_, i)| *i == id) {
            return n.clone();
        }
        let n = self.name("Im");
        res.xobjects.push((n.clone(), id));
        n
    }
}

fn stroke_ops(s: &StrokeStyle, out: &mut String) {
    writeln!(out, "{} w", f(s.width.max(0.0))).unwrap();
    let j = match s.join {
        LineJoin::Miter => 0,
        LineJoin::Round => 1,
        LineJoin::Bevel => 2,
    };
    let c = match s.cap {
        LineCap::Butt => 0,
        LineCap::Round => 1,
        LineCap::Square => 2,
    };
    writeln!(out, "{j} j {c} J {} M", f(s.miter.max(1.0))).unwrap();
    if s.dash.is_empty() {
        out.push_str("[] 0 d\n");
    } else {
        let d: Vec<String> = s.dash.iter().map(|v| f(*v)).collect();
        writeln!(out, "[{}] 0 d", d.join(" ")).unwrap();
    }
}

fn res_dict(res: &Res) -> String {
    let mut s = String::from("<< /ProcSet [/PDF /ImageC /ImageB]");
    if !res.ext_g.is_empty() {
        s.push_str(" /ExtGState <<");
        for (n, id) in &res.ext_g {
            write!(s, " /{n} {id} 0 R").unwrap();
        }
        s.push_str(" >>");
    }
    if !res.shadings.is_empty() {
        s.push_str(" /Shading <<");
        for (n, id) in &res.shadings {
            write!(s, " /{n} {id} 0 R").unwrap();
        }
        s.push_str(" >>");
    }
    if !res.xobjects.is_empty() {
        s.push_str(" /XObject <<");
        for (n, id) in &res.xobjects {
            write!(s, " /{n} {id} 0 R").unwrap();
        }
        s.push_str(" >>");
    }
    s.push_str(" >>");
    s
}

fn cm(a: Affine) -> String {
    let c = a.as_coeffs();
    format!("{} {} {} {} {} {} cm\n", f(c[0]), f(c[1]), f(c[2]), f(c[3]), f(c[4]), f(c[5]))
}

pub fn export_bytes(doc: &Document, opts: &PdfOptions) -> Result<Vec<u8>> {
    let mut ctx = Ctx { w: Writer { buf: vec![], offsets: vec![] }, doc, opts, image_cache: HashMap::new(), gs_cache: HashMap::new(), counter: 0 };
    ctx.w.buf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let catalog = ctx.w.alloc();
    let pages_id = ctx.w.alloc();
    let info_id = ctx.w.alloc();
    let pages: Vec<usize> = if opts.pages.is_empty() { (0..doc.pages.len()).collect() } else { opts.pages.iter().copied().filter(|p| *p < doc.pages.len()).collect() };
    let mut kids = vec![];
    for &pi in &pages {
        let pg = &doc.pages[pi];
        let mut fl = Flattener::new(
            doc,
            FlatOptions { raster_dpi: opts.raster_dpi, conical_gradients: false, repeat_gradients: false, gradient_alpha: false, printing: opts.printing },
        );
        fl.page(pi);
        let prims = std::mem::take(&mut fl.out);
        let mut res = Res::default();
        // stack of (content, resources, pending group gs name)
        let mut stack: Vec<(String, Res, String)> = vec![];
        let mut content = String::new();
        content.push_str(&format!("1 0 0 -1 0 {} cm\n", f(pg.height)));
        let mut links = vec![];
        for p in prims {
            match p {
                Prim::Fill { path, paint, even_odd } => {
                    let path = cubic_only(&path);
                    if path.elements().is_empty() {
                        continue;
                    }
                    match paint {
                        Paint::Solid(c) => {
                            let a = c.a as f32 / 255.0;
                            content.push_str("q\n");
                            if a < 0.999 {
                                let gs = ctx.ext_gstate(&mut res, a, 1.0, MergeMode::Normal);
                                writeln!(content, "/{gs} gs").unwrap();
                            }
                            writeln!(content, "{} rg", rgb(c)).unwrap();
                            path_ops(&path, &mut content);
                            content.push_str(if even_odd { "f*\nQ\n" } else { "f\nQ\n" });
                        }
                        Paint::Gradient(g, space) => {
                            let sh = ctx.shading(&mut res, &g);
                            content.push_str("q\n");
                            path_ops(&path, &mut content);
                            content.push_str(if even_odd { "W* n\n" } else { "W n\n" });
                            content.push_str(&cm(space));
                            writeln!(content, "/{sh} sh\nQ").unwrap();
                        }
                    }
                }
                Prim::Stroke { path, color, style } => {
                    let path = cubic_only(&path);
                    if path.elements().is_empty() {
                        continue;
                    }
                    content.push_str("q\n");
                    let a = color.a as f32 / 255.0;
                    if a < 0.999 {
                        let gs = ctx.ext_gstate(&mut res, 1.0, a, MergeMode::Normal);
                        writeln!(content, "/{gs} gs").unwrap();
                    }
                    writeln!(content, "{} RG", rgb(color)).unwrap();
                    stroke_ops(&style, &mut content);
                    path_ops(&path, &mut content);
                    content.push_str("S\nQ\n");
                }
                Prim::Image { image, xf, clip } => {
                    let Some(img) = doc.images.get(&image) else { continue };
                    let (iw, ih) = (img.width as f64, img.height as f64);
                    let Some(id) = ctx.doc_image(image) else { continue };
                    let n = ctx.xobject_name(&mut res, id);
                    content.push_str("q\n");
                    path_ops(&cubic_only(&clip), &mut content);
                    content.push_str("W n\n");
                    let unit = xf * Affine::new([iw, 0.0, 0.0, -ih, 0.0, ih]);
                    content.push_str(&cm(unit));
                    writeln!(content, "/{n} Do\nQ").unwrap();
                }
                Prim::Raster { rect, w, h, rgba } => {
                    let id = ctx.image_rgba(w, h, &rgba);
                    let n = ctx.xobject_name(&mut res, id);
                    let unit = Affine::new([rect.width(), 0.0, 0.0, -rect.height(), rect.x0, rect.y1]);
                    content.push_str("q\n");
                    content.push_str(&cm(unit));
                    writeln!(content, "/{n} Do\nQ").unwrap();
                }
                Prim::BeginGroup { opacity, blend } => {
                    let gs = ctx.ext_gstate(&mut res, opacity, opacity, blend);
                    stack.push((std::mem::take(&mut content), std::mem::take(&mut res), gs));
                }
                Prim::EndGroup => {
                    if let Some((parent, pres, gs)) = stack.pop() {
                        let id = ctx.w.alloc();
                        let dict = format!(
                            "/Type /XObject /Subtype /Form /BBox [-100000 -100000 100000 100000] /Group << /S /Transparency /CS /DeviceRGB >> /Resources {}",
                            res_dict(&res)
                        );
                        ctx.w.stream(id, &dict, content.as_bytes(), opts.compress);
                        content = parent;
                        // the group's ExtGState was registered in the parent resources at BeginGroup
                        res = pres;
                        let n = ctx.xobject_name(&mut res, id);
                        writeln!(content, "q\n/{gs} gs\n/{n} Do\nQ").unwrap();
                    }
                }
                Prim::BeginClip { path } => {
                    content.push_str("q\n");
                    path_ops(&cubic_only(&path), &mut content);
                    content.push_str("W n\n");
                }
                Prim::EndClip => content.push_str("Q\n"),
                Prim::Link { rect, url } => {
                    if opts.include_hyperlinks {
                        links.push((rect, url));
                    }
                }
            }
        }
        // close any unbalanced groups
        while let Some((parent, pres, _)) = stack.pop() {
            let mut c = parent;
            c.push_str(&content);
            content = c;
            for e in pres.ext_g {
                res.ext_g.push(e);
            }
        }
        let content_id = ctx.w.alloc();
        ctx.w.stream(content_id, "", content.as_bytes(), opts.compress);
        let mut annots = vec![];
        for (r, url) in links {
            let id = ctx.w.alloc();
            ctx.w.obj(
                id,
                &format!(
                    "<< /Type /Annot /Subtype /Link /Rect [{} {} {} {}] /Border [0 0 0] /A << /S /URI /URI {} >> >>",
                    f(r.x0),
                    f(pg.height - r.y1),
                    f(r.x1),
                    f(pg.height - r.y0),
                    pdf_string(&url)
                ),
            );
            annots.push(format!("{id} 0 R"));
        }
        let page_id = ctx.w.alloc();
        let annots = if annots.is_empty() { String::new() } else { format!(" /Annots [{}]", annots.join(" ")) };
        ctx.w.obj(
            page_id,
            &format!(
                "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {} {}] /Contents {content_id} 0 R /Resources {} /Group << /S /Transparency /CS /DeviceRGB >>{annots} >>",
                f(pg.width),
                f(pg.height),
                res_dict(&res)
            ),
        );
        kids.push(format!("{page_id} 0 R"));
    }
    ctx.w.obj(pages_id, &format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), kids.len()));
    ctx.w.obj(catalog, &format!("<< /Type /Catalog /Pages {pages_id} 0 R >>"));
    let i = &doc.info;
    let mut info = format!("<< /Creator {} /Producer {}", pdf_string(&format!("{} {}", vector_core::APP_NAME, vector_core::APP_VERSION)), pdf_string(vector_core::APP_NAME));
    for (k, v) in [("Title", &i.title), ("Author", &i.author), ("Subject", &i.subject), ("Keywords", &i.keywords)] {
        if !v.is_empty() {
            write!(info, " /{k} {}", pdf_string(v)).unwrap();
        }
    }
    info.push_str(" >>");
    ctx.w.obj(info_id, &info);
    // xref
    let xref_pos = ctx.w.buf.len();
    let n = ctx.w.offsets.len() + 1;
    write!(ctx.w.buf, "xref\n0 {n}\n0000000000 65535 f \n").unwrap();
    for off in ctx.w.offsets.clone() {
        write!(ctx.w.buf, "{off:010} 00000 n \n").unwrap();
    }
    write!(ctx.w.buf, "trailer\n<< /Size {n} /Root {catalog} 0 R /Info {info_id} 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n").unwrap();
    Ok(ctx.w.buf)
}

pub fn export(doc: &Document, path: &Path, opts: &PdfOptions) -> Result<()> {
    let bytes = export_bytes(doc, opts)?;
    std::fs::write(path, bytes)?;
    Ok(())
}
