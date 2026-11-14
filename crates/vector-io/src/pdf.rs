//! PDF writer (Publish to PDF). Multi-page, vector paths, axial/radial shadings, transparency
//! groups with blend modes, images (JPEG passthrough / Flate / LZW / JPEG + SMask), hyperlinks,
//! bookmarks, thumbnails, comments, printer's marks, RGB/CMYK/grayscale output, ASCII85 and
//! 128-bit security. Text is written as curves.

use crate::flat::{FlatOptions, Flattener, Paint, Prim, StrokeStyle};
use crate::pdf_crypt::{hex, Crypt};
use crate::Result;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use kurbo::{Affine, BezPath, PathEl};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write;
use std::path::Path;
use vector_core::color::Color;
use vector_core::document::Document;
use vector_core::style::*;

/// Settings ▸ General ▸ Compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PdfCompat {
    Acrobat4,
    Acrobat5,
    Acrobat6,
    Acrobat8,
    #[default]
    Acrobat9,
    PdfX1a,
    PdfX3,
    PdfA1b,
}

impl PdfCompat {
    pub const ALL: [PdfCompat; 8] =
        [PdfCompat::Acrobat4, PdfCompat::Acrobat5, PdfCompat::Acrobat6, PdfCompat::Acrobat8, PdfCompat::Acrobat9, PdfCompat::PdfX1a, PdfCompat::PdfX3, PdfCompat::PdfA1b];
    pub fn name(self) -> &'static str {
        match self {
            PdfCompat::Acrobat4 => "Acrobat 4.0",
            PdfCompat::Acrobat5 => "Acrobat 5.0",
            PdfCompat::Acrobat6 => "Acrobat 6.0",
            PdfCompat::Acrobat8 => "Acrobat 8.0",
            PdfCompat::Acrobat9 => "Acrobat 9.0",
            PdfCompat::PdfX1a => "PDF/X-1a",
            PdfCompat::PdfX3 => "PDF/X-3",
            PdfCompat::PdfA1b => "PDF/A-1b",
        }
    }
    pub fn version(self) -> &'static str {
        match self {
            PdfCompat::Acrobat4 | PdfCompat::PdfX1a | PdfCompat::PdfX3 => "1.3",
            PdfCompat::Acrobat5 | PdfCompat::PdfA1b => "1.4",
            PdfCompat::Acrobat6 => "1.5",
            PdfCompat::Acrobat8 | PdfCompat::Acrobat9 => "1.7",
        }
    }
    /// Whether the version can hold transparency; otherwise transparency is flattened.
    pub fn transparency(self) -> bool {
        matches!(self, PdfCompat::Acrobat5 | PdfCompat::Acrobat6 | PdfCompat::Acrobat8 | PdfCompat::Acrobat9)
    }
    /// PDF/X and PDF/A do not allow security.
    pub fn allows_security(self) -> bool {
        !matches!(self, PdfCompat::PdfX1a | PdfCompat::PdfX3 | PdfCompat::PdfA1b)
    }
}

/// Settings ▸ Color ▸ Output color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PdfColor {
    #[default]
    Rgb,
    Cmyk,
    Grayscale,
    Native,
}

impl PdfColor {
    pub const ALL: [PdfColor; 4] = [PdfColor::Rgb, PdfColor::Cmyk, PdfColor::Grayscale, PdfColor::Native];
    pub fn name(self) -> &'static str {
        match self {
            PdfColor::Rgb => "RGB",
            PdfColor::Cmyk => "CMYK",
            PdfColor::Grayscale => "Grayscale",
            PdfColor::Native => "Native",
        }
    }
}

/// Settings ▸ Objects ▸ Bitmap compression type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PdfCompression {
    None,
    Lzw,
    Jpeg,
    #[default]
    Zip,
}

impl PdfCompression {
    pub const ALL: [PdfCompression; 4] = [PdfCompression::None, PdfCompression::Lzw, PdfCompression::Jpeg, PdfCompression::Zip];
    pub fn name(self) -> &'static str {
        match self {
            PdfCompression::None => "None",
            PdfCompression::Lzw => "LZW",
            PdfCompression::Jpeg => "JPEG",
            PdfCompression::Zip => "ZIP",
        }
    }
}

/// Settings ▸ Document ▸ On start, display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PdfStart {
    #[default]
    PageOnly,
    FullScreen,
    Thumbnails,
    Bookmarks,
}

impl PdfStart {
    pub const ALL: [PdfStart; 4] = [PdfStart::PageOnly, PdfStart::FullScreen, PdfStart::Thumbnails, PdfStart::Bookmarks];
    pub fn name(self) -> &'static str {
        match self {
            PdfStart::PageOnly => "Page only",
            PdfStart::FullScreen => "Full screen",
            PdfStart::Thumbnails => "Thumbnails",
            PdfStart::Bookmarks => "Bookmarks",
        }
    }
    fn mode(self) -> &'static str {
        match self {
            PdfStart::PageOnly => "UseNone",
            PdfStart::FullScreen => "FullScreen",
            PdfStart::Thumbnails => "UseThumbs",
            PdfStart::Bookmarks => "UseOutlines",
        }
    }
}

/// Settings ▸ Security. Never saved with presets.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PdfSecurity {
    pub open_password: String,
    pub permission_password: String,
    /// 0 = None, 1 = Low resolution, 2 = High resolution.
    pub printing: u8,
    /// 0 = None, 1 = Insert, delete, rotate pages, 2 = Any except extracting pages.
    pub editing: u8,
    pub copying: bool,
}

#[derive(Clone, Debug)]
pub struct PdfOptions {
    /// Page indices to export; empty = all.
    pub pages: Vec<usize>,
    pub raster_dpi: f64,
    /// Compress text and line art.
    pub compress: bool,
    /// Keep JPEG images as they are (no recompression).
    pub jpeg_images: bool,
    pub include_hyperlinks: bool,
    pub printing: bool,
    pub compat: PdfCompat,
    pub color: PdfColor,
    pub bookmarks: bool,
    pub thumbnails: bool,
    pub start: PdfStart,
    pub comments: bool,
    pub ascii85: bool,
    pub image_compression: PdfCompression,
    pub jpeg_quality: u8,
    /// Downsample color / grayscale / monochrome bitmaps above this resolution.
    pub downsample_color: Option<f64>,
    pub downsample_gray: Option<f64>,
    pub downsample_mono: Option<f64>,
    pub crop_marks: bool,
    pub registration_marks: bool,
    pub densitometer: bool,
    pub file_info: bool,
    /// Bleed limit in points (content outside the page is kept up to this distance).
    pub bleed: f64,
    /// Clip everything to the page (plus bleed).
    pub only_on_page: bool,
    /// Set 100% black fills and outlines to overprint.
    pub overprint_black: bool,
    pub security: Option<PdfSecurity>,
    /// Shown by File information.
    pub file_name: String,
    /// Fixed ID seed (tests); `None` = from the time.
    pub id_seed: Option<u64>,
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions {
            pages: vec![],
            raster_dpi: 300.0,
            compress: true,
            jpeg_images: true,
            include_hyperlinks: true,
            printing: true,
            compat: PdfCompat::Acrobat9,
            color: PdfColor::Rgb,
            bookmarks: true,
            thumbnails: false,
            start: PdfStart::PageOnly,
            comments: false,
            ascii85: false,
            image_compression: PdfCompression::Zip,
            jpeg_quality: 85,
            downsample_color: None,
            downsample_gray: None,
            downsample_mono: None,
            crop_marks: false,
            registration_marks: false,
            densitometer: false,
            file_info: false,
            bleed: 0.0,
            only_on_page: false,
            overprint_black: false,
            security: None,
            file_name: String::new(),
            id_seed: None,
        }
    }
}

impl PdfOptions {
    pub fn has_marks(&self) -> bool {
        self.crop_marks || self.registration_marks || self.densitometer || self.file_info
    }
    /// Room around the page for marks and bleed.
    pub fn margin(&self) -> f64 {
        if self.has_marks() {
            36.0f64.max(self.bleed + 18.0)
        } else {
            self.bleed.max(0.0)
        }
    }
}

struct Writer {
    buf: Vec<u8>,
    offsets: Vec<usize>,
    crypt: Option<Crypt>,
    ascii85: bool,
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
    /// Replace the string markers written by `pdf_string` with (encrypted) hex strings.
    fn strings(&self, id: usize, body: &str) -> String {
        if !body.contains('\u{1}') {
            return body.to_string();
        }
        let mut out = String::with_capacity(body.len());
        let mut rest = body;
        while let Some(i) = rest.find('\u{1}') {
            out.push_str(&rest[..i]);
            let after = &rest[i + 1..];
            let end = after.find('\u{2}').unwrap_or(after.len());
            let raw: Vec<u8> = (0..end / 2).filter_map(|k| u8::from_str_radix(&after[k * 2..k * 2 + 2], 16).ok()).collect();
            let bytes = match &self.crypt {
                Some(c) => c.encrypt(id, &raw),
                None => raw,
            };
            write!(out, "<{}>", hex(&bytes)).unwrap();
            rest = &after[(end + 1).min(after.len())..];
        }
        out.push_str(rest);
        out
    }
    fn obj(&mut self, id: usize, body: &str) {
        let body = self.strings(id, body);
        self.begin(id);
        self.buf.extend_from_slice(body.as_bytes());
        self.buf.extend_from_slice(b"\nendobj\n");
    }
    /// An object that is never encrypted (the Encrypt dictionary).
    fn obj_plain(&mut self, id: usize, body: &str) {
        self.begin(id);
        self.buf.extend_from_slice(body.as_bytes());
        self.buf.extend_from_slice(b"\nendobj\n");
    }
    /// A stream; `compress` applies Flate.
    fn stream(&mut self, id: usize, dict: &str, data: &[u8], compress: bool) {
        if compress {
            self.put(id, dict, flate(data), Some("FlateDecode"));
        } else {
            self.put(id, dict, data.to_vec(), None);
        }
    }
    /// A stream whose data is already encoded with `filter`.
    fn put(&mut self, id: usize, dict: &str, data: Vec<u8>, filter: Option<&str>) {
        let (data, filter) = if self.ascii85 {
            let f = match filter {
                Some(f) => format!(" /Filter [/ASCII85Decode /{f}]"),
                None => " /Filter /ASCII85Decode".to_string(),
            };
            (ascii85(&data), f)
        } else {
            (data, filter.map(|f| format!(" /Filter /{f}")).unwrap_or_default())
        };
        let data = match &self.crypt {
            Some(c) => c.encrypt(id, &data),
            None => data,
        };
        let dict = self.strings(id, dict);
        self.begin(id);
        write!(self.buf, "<< {dict}{filter} /Length {} >>\nstream\n", data.len()).unwrap();
        self.buf.extend_from_slice(&data);
        self.buf.extend_from_slice(b"\nendstream\nendobj\n");
    }
}

fn flate(data: &[u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn lzw(data: &[u8]) -> Vec<u8> {
    let mut enc = weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
    enc.encode(data).unwrap_or_default()
}

pub fn ascii85(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 5 / 4 + 8);
    let mut col = 0;
    for chunk in data.chunks(4) {
        let mut b = [0u8; 4];
        b[..chunk.len()].copy_from_slice(chunk);
        let v = u32::from_be_bytes(b);
        if chunk.len() == 4 && v == 0 {
            out.push(b'z');
            col += 1;
        } else {
            let mut d = [0u8; 5];
            let mut x = v;
            for k in (0..5).rev() {
                d[k] = (x % 85) as u8 + b'!';
                x /= 85;
            }
            out.extend_from_slice(&d[..chunk.len() + 1]);
            col += chunk.len() + 1;
        }
        if col >= 75 {
            out.push(b'\n');
            col = 0;
        }
    }
    out.extend_from_slice(b"~>");
    out
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

/// A text string for a dictionary: written as a marker the writer turns into a hex string
/// (encrypted when the file has security). UTF-16BE with a BOM for non-ASCII text.
fn pdf_string(s: &str) -> String {
    let mut h = String::from("\u{1}");
    if s.is_ascii() {
        for b in s.bytes() {
            write!(h, "{b:02X}").unwrap();
        }
    } else {
        h.push_str("FEFF");
        for u in s.encode_utf16() {
            write!(h, "{u:04X}").unwrap();
        }
    }
    h.push('\u{2}');
    h
}

/// A literal string inside a content stream (ASCII only).
fn content_string(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_ascii() && !c.is_ascii_control() { c } else { '?' }).collect();
    format!("({})", s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)"))
}

/// The CMYK (each 0..1) of a color: the inks of a CMYK color exactly as entered, an RGB color
/// converted with the document's color profiles and rendering intent.
fn cmyk_of(c: Color) -> [f64; 4] {
    c.to_cmyk().map(|v| (v / 100.0).clamp(0.0, 1.0))
}

/// The gray level (0..1) of a color through the document's grayscale profile.
fn gray_of(c: Color) -> f64 {
    let e = vector_core::cms::current();
    let g = match c.cmyk {
        Some(k) => e.cmyk_to_gray(k.map(|v| v as f64)),
        None => e.rgb_to_gray(c),
    };
    (g / 255.0).clamp(0.0, 1.0)
}

fn color_components(c: Color, space: PdfColor) -> String {
    match space {
        PdfColor::Cmyk => cmyk_of(c).iter().map(|v| f(*v)).collect::<Vec<_>>().join(" "),
        PdfColor::Grayscale => f(gray_of(c)),
        _ => format!("{} {} {}", f(c.r as f64 / 255.0), f(c.g as f64 / 255.0), f(c.b as f64 / 255.0)),
    }
}

/// The fill (`stroke` = false) or stroke color operator.
fn color_op(c: Color, space: PdfColor, stroke: bool) -> String {
    // Native: CMYK colors stay CMYK, RGB colors RGB
    let space = if space == PdfColor::Native && c.cmyk.is_some() { PdfColor::Cmyk } else { space };
    let op = match space {
        PdfColor::Cmyk => "k",
        PdfColor::Grayscale => "g",
        _ => "rg",
    };
    format!("{} {}", color_components(c, space), if stroke { op.to_uppercase() } else { op.to_string() })
}

fn color_space(space: PdfColor) -> &'static str {
    match space {
        PdfColor::Cmyk => "/DeviceCMYK",
        PdfColor::Grayscale => "/DeviceGray",
        _ => "/DeviceRGB",
    }
}

fn is_black(c: Color) -> bool {
    c.a == 255 && match c.cmyk {
        Some(k) => k == [0, 0, 0, 100],
        None => c.r == 0 && c.g == 0 && c.b == 0,
    }
}

/// A color with its alpha flattened against white (for versions without transparency).
fn over_white(c: Color) -> Color {
    let a = c.a as f64 / 255.0;
    let m = |v: u8| (v as f64 * a + 255.0 * (1.0 - a)).round() as u8;
    // CMYK inks thin out the same way
    let cmyk = c.cmyk.map(|k| k.map(|v| (v as f64 * a).round() as u8));
    Color { r: m(c.r), g: m(c.g), b: m(c.b), a: 255, cmyk }
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
    font: Option<usize>,
}

struct Ctx<'a> {
    w: Writer,
    doc: &'a Document,
    opts: &'a PdfOptions,
    image_cache: HashMap<(u64, u32), usize>,
    gs_cache: HashMap<String, usize>,
    counter: usize,
    font: Option<usize>,
}

impl<'a> Ctx<'a> {
    fn name(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("{prefix}{}", self.counter)
    }

    fn space(&self) -> PdfColor {
        if matches!(self.opts.compat, PdfCompat::PdfX1a) && self.opts.color != PdfColor::Grayscale {
            PdfColor::Cmyk
        } else {
            self.opts.color
        }
    }

    /// Helvetica for printer's marks text.
    fn font(&mut self, res: &mut Res) {
        let id = match self.font {
            Some(id) => id,
            None => {
                let id = self.w.alloc();
                self.w.obj(id, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>");
                self.font = Some(id);
                id
            }
        };
        res.font = Some(id);
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

    /// An ExtGState that turns on overprinting (Always overprint black).
    fn overprint(&mut self, res: &mut Res) -> String {
        let id = match self.gs_cache.get("overprint") {
            Some(id) => *id,
            None => {
                let id = self.w.alloc();
                self.w.obj(id, "<< /Type /ExtGState /OP true /op true /OPM 1 >>");
                self.gs_cache.insert("overprint".into(), id);
                id
            }
        };
        if let Some((n, _)) = res.ext_g.iter().find(|(_, i)| *i == id) {
            return n.clone();
        }
        let n = self.name("GS");
        res.ext_g.push((n.clone(), id));
        n
    }

    fn function(&mut self, g: &Gradient) -> usize {
        let space = self.space();
        let flat = !self.opts.compat.transparency();
        let col = |c: Color| color_components(if flat { over_white(c) } else { c }, space);
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
            self.w.obj(id, &format!("<< /FunctionType 2 /Domain [0 1] /C0 [{}] /C1 [{}] /N 1 >>", col(w[0].color), col(w[1].color)));
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
        let cs = color_space(self.space());
        let id = self.w.alloc();
        let body = match g.kind {
            GradientKind::Elliptical => {
                let r = (g.end - g.start).hypot();
                format!("<< /ShadingType 3 /ColorSpace {cs} /Coords [{} {} 0 {} {} {}] /Function {func} 0 R /Extend [true true] >>", f(g.start.x), f(g.start.y), f(g.start.x), f(g.start.y), f(r))
            }
            _ => format!("<< /ShadingType 2 /ColorSpace {cs} /Coords [{} {} {} {}] /Function {func} 0 R /Extend [true true] >>", f(g.start.x), f(g.start.y), f(g.end.x), f(g.end.y)),
        };
        self.w.obj(id, &body);
        let n = self.name("Sh");
        res.shadings.push((n.clone(), id));
        n
    }

    /// Image XObject from straight RGBA with an SMask if needed, in the output color space and
    /// with the bitmap compression from the settings.
    fn image_rgba(&mut self, w: u32, h: u32, rgba: &[u8]) -> usize {
        let space = self.space();
        let flat = !self.opts.compat.transparency();
        let mut alpha = Vec::with_capacity((w * h) as usize);
        let mut has_alpha = false;
        let comps = match space {
            PdfColor::Cmyk => 4,
            PdfColor::Grayscale => 1,
            _ => 3,
        };
        let mut px_out = Vec::with_capacity((w * h) as usize * comps);
        for px in rgba.chunks_exact(4) {
            let mut c = Color::rgba(px[0], px[1], px[2], px[3]);
            if flat {
                c = over_white(c);
            } else {
                alpha.push(px[3]);
                if px[3] < 255 {
                    has_alpha = true;
                }
            }
            match space {
                // converted all at once below
                PdfColor::Cmyk => px_out.extend_from_slice(&[c.r, c.g, c.b]),
                PdfColor::Grayscale => px_out.push(((0.299 * c.r as f64 + 0.587 * c.g as f64 + 0.114 * c.b as f64)).round().clamp(0.0, 255.0) as u8),
                _ => px_out.extend_from_slice(&[c.r, c.g, c.b]),
            }
        }
        if space == PdfColor::Cmyk {
            px_out = vector_core::cms::current().rgb_pixels_to_cmyk(&px_out);
        }
        let smask = if has_alpha {
            let id = self.w.alloc();
            self.w.stream(id, &format!("/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8"), &alpha, true);
            format!(" /SMask {id} 0 R")
        } else {
            String::new()
        };
        let id = self.w.alloc();
        let dict = format!("/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace {} /BitsPerComponent 8 /Interpolate true{smask}", color_space(space));
        match self.opts.image_compression {
            PdfCompression::None => self.w.put(id, &dict, px_out, None),
            PdfCompression::Lzw => self.w.put(id, &dict, lzw(&px_out), Some("LZWDecode")),
            PdfCompression::Jpeg if comps != 4 => {
                let mut out = vec![];
                let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, self.opts.jpeg_quality.clamp(1, 100));
                let ct = if comps == 1 { image::ExtendedColorType::L8 } else { image::ExtendedColorType::Rgb8 };
                match image::ImageEncoder::write_image(enc, &px_out, w, h, ct) {
                    Ok(()) => self.w.put(id, &dict, out, Some("DCTDecode")),
                    Err(_) => self.w.put(id, &dict, flate(&px_out), Some("FlateDecode")),
                }
            }
            _ => self.w.put(id, &dict, flate(&px_out), Some("FlateDecode")),
        }
        id
    }

    /// A document image, downsampled to `target_w` pixels wide when that is smaller.
    fn doc_image(&mut self, image: u64, target_w: Option<u32>) -> Option<usize> {
        let img = self.doc.images.get(&image)?;
        let tw = target_w.filter(|t| *t < img.width && *t > 0).unwrap_or(img.width);
        if let Some(id) = self.image_cache.get(&(image, tw)) {
            return Some(*id);
        }
        let opaque = img.rgba.chunks_exact(4).all(|p| p[3] == 255);
        let keep_jpeg = self.opts.jpeg_images
            && tw == img.width
            && img.format == "jpg"
            && !img.encoded.is_empty()
            && opaque
            && matches!(self.space(), PdfColor::Rgb | PdfColor::Native)
            && matches!(self.opts.image_compression, PdfCompression::Jpeg | PdfCompression::Zip);
        let id = if keep_jpeg {
            let id = self.w.alloc();
            let data = img.encoded.clone();
            self.w.put(id, &format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8", img.width, img.height), data.to_vec(), Some("DCTDecode"));
            id
        } else if tw < img.width {
            let th = ((img.height as f64 * tw as f64 / img.width as f64).round() as u32).max(1);
            let src = image::RgbaImage::from_raw(img.width, img.height, img.rgba.to_vec())?;
            let small = image::imageops::resize(&src, tw, th, image::imageops::FilterType::Triangle);
            self.image_rgba(tw, th, small.as_raw())
        } else {
            let (w, h, rgba) = (img.width, img.height, img.rgba.clone());
            self.image_rgba(w, h, &rgba)
        };
        self.image_cache.insert((image, tw), id);
        Some(id)
    }

    /// The pixel width an image should be downsampled to for its size on the page.
    fn downsample_width(&self, image: u64, xf: Affine) -> Option<u32> {
        let img = self.doc.images.get(&image)?;
        let c = xf.as_coeffs();
        let scale = (c[0] * c[0] + c[1] * c[1]).sqrt();
        if scale <= 0.0 {
            return None;
        }
        let dpi = 72.0 / scale;
        let gray = img.rgba.chunks_exact(4).all(|p| p[0] == p[1] && p[1] == p[2]);
        let mono = gray && img.rgba.chunks_exact(4).all(|p| p[0] == 0 || p[0] == 255);
        let limit = if mono {
            self.opts.downsample_mono
        } else if gray {
            self.opts.downsample_gray
        } else {
            self.opts.downsample_color
        }?;
        (dpi > limit * 1.05).then(|| ((img.width as f64) * limit / dpi).round().max(1.0) as u32)
    }

    fn xobject_name(&mut self, res: &mut Res, id: usize) -> String {
        if let Some((n, _)) = res.xobjects.iter().find(|(_, i)| *i == id) {
            return n.clone();
        }
        let n = self.name("Im");
        res.xobjects.push((n.clone(), id));
        n
    }

    /// Crop marks, registration marks, densitometer scale and file information around the
    /// trim box (`m` = margin, page `w` × `h`), in PDF coordinates.
    fn marks(&mut self, res: &mut Res, m: f64, w: f64, h: f64, page_no: usize) -> String {
        let o = self.opts;
        let mut s = String::from("q\n");
        let space = self.space();
        s.push_str(&color_op(Color::BLACK, space, true));
        s.push('\n');
        s.push_str(&color_op(Color::BLACK, space, false));
        s.push_str("\n0.25 w\n");
        let gap = o.bleed.max(0.0) + 3.0;
        let len = (m - gap - 2.0).clamp(4.0, 18.0);
        if o.crop_marks {
            for (x, y) in [(m, m), (m + w, m), (m, m + h), (m + w, m + h)] {
                let sx = if x > m + w / 2.0 { 1.0 } else { -1.0 };
                let sy = if y > m + h / 2.0 { 1.0 } else { -1.0 };
                writeln!(s, "{} {} m {} {} l S", f(x + sx * gap), f(y), f(x + sx * (gap + len)), f(y)).unwrap();
                writeln!(s, "{} {} m {} {} l S", f(x), f(y + sy * gap), f(x), f(y + sy * (gap + len))).unwrap();
            }
        }
        if o.registration_marks {
            let r = 5.0;
            let c = m / 2.0;
            for (x, y) in [(m + w / 2.0, c), (m + w / 2.0, m + h + m - c), (c, m + h / 2.0), (m + w + m - c, m + h / 2.0)] {
                let k = 0.5523 * r;
                writeln!(
                    s,
                    "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c S",
                    f(x + r),
                    f(y),
                    f(x + r),
                    f(y + k),
                    f(x + k),
                    f(y + r),
                    f(x),
                    f(y + r),
                    f(x - k),
                    f(y + r),
                    f(x - r),
                    f(y + k),
                    f(x - r),
                    f(y),
                    f(x - r),
                    f(y - k),
                    f(x - k),
                    f(y - r),
                    f(x),
                    f(y - r),
                    f(x + k),
                    f(y - r),
                    f(x + r),
                    f(y - k),
                    f(x + r),
                    f(y)
                )
                .unwrap();
                writeln!(s, "{} {} m {} {} l S {} {} m {} {} l S", f(x - r * 1.6), f(y), f(x + r * 1.6), f(y), f(x), f(y - r * 1.6), f(x), f(y + r * 1.6)).unwrap();
            }
        }
        if o.densitometer {
            let size = 9.0;
            let x = (m - size) / 2.0;
            for i in 0..10 {
                let v = (i as f64 * 25.0).min(255.0) as u8;
                let c = Color::rgb(255 - v, 255 - v, 255 - v);
                let y = m + h - (i as f64 + 1.0) * size;
                if y < m {
                    break;
                }
                writeln!(s, "{} {} {} {} {} re f", color_op(c, space, false), f(x), f(y), f(size), f(size)).unwrap();
            }
            s.push_str(&color_op(Color::BLACK, space, false));
            s.push('\n');
        }
        if o.file_info {
            self.font(res);
            let name = if o.file_name.is_empty() { "Untitled" } else { o.file_name.as_str() };
            let text = format!("{name}  Page {page_no}  {}", today());
            let y = (m - gap - 10.0).max(2.0);
            writeln!(s, "BT /F1 7 Tf {} {} Td {} Tj ET", f(m), f(y), content_string(&text)).unwrap();
        }
        s.push_str("Q\n");
        s
    }
}

fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
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
    if let Some(id) = res.font {
        write!(s, " /Font << /F1 {id} 0 R >>").unwrap();
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
    // colors convert with the document's own color profiles
    vector_core::cms::with_settings(&doc.settings.color, || export_with_profiles(doc, opts))
}

fn export_with_profiles(doc: &Document, opts: &PdfOptions) -> Result<Vec<u8>> {
    let seed = opts.id_seed.unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0));
    let id = crate::pdf_crypt::md5(format!("{seed}/{}/{}", opts.file_name, doc.pages.len()).as_bytes());
    let security = opts.security.as_ref().filter(|_| opts.compat.allows_security());
    let crypt = security.map(|s| Crypt::new(&s.open_password, &s.permission_password, crate::pdf_crypt::permissions(s.printing, s.editing, s.copying), id));
    let mut ctx = Ctx {
        w: Writer { buf: vec![], offsets: vec![], crypt, ascii85: opts.ascii85 },
        doc,
        opts,
        image_cache: HashMap::new(),
        gs_cache: HashMap::new(),
        counter: 0,
        font: None,
    };
    let mut version = opts.compat.version();
    if security.is_some() && version < "1.4" {
        version = "1.4";
    }
    ctx.w.buf.extend_from_slice(format!("%PDF-{version}\n").as_bytes());
    ctx.w.buf.extend_from_slice(b"%\xE2\xE3\xCF\xD3\n");
    let space = ctx.space();
    let transparency = opts.compat.transparency();
    let catalog = ctx.w.alloc();
    let pages_id = ctx.w.alloc();
    let info_id = ctx.w.alloc();
    let pages: Vec<usize> = if opts.pages.is_empty() { (0..doc.pages.len()).collect() } else { opts.pages.iter().copied().filter(|p| *p < doc.pages.len()).collect() };
    let m = opts.margin();
    let mut kids = vec![];
    let mut page_ids = vec![];
    for (n, &pi) in pages.iter().enumerate() {
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
        let mut flat_groups = 0usize;
        let mut content = String::new();
        if opts.has_marks() {
            content.push_str(&ctx.marks(&mut res, m, pg.width, pg.height, n + 1));
        }
        content.push_str("q\n");
        content.push_str(&format!("1 0 0 -1 {} {} cm\n", f(m), f(pg.height + m)));
        if opts.only_on_page || m > 0.0 {
            let b = if m > 0.0 { opts.bleed.max(0.0) } else { 0.0 };
            writeln!(content, "{} {} {} {} re W n", f(-b), f(-b), f(pg.width + 2.0 * b), f(pg.height + 2.0 * b)).unwrap();
        }
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
                            let c = if transparency {
                                if a < 0.999 {
                                    let gs = ctx.ext_gstate(&mut res, a, 1.0, MergeMode::Normal);
                                    writeln!(content, "/{gs} gs").unwrap();
                                }
                                c
                            } else {
                                over_white(c)
                            };
                            if opts.overprint_black && is_black(c) {
                                let gs = ctx.overprint(&mut res);
                                writeln!(content, "/{gs} gs").unwrap();
                            }
                            writeln!(content, "{}", color_op(c, space, false)).unwrap();
                            path_ops(&path, &mut content);
                            content.push_str(if even_odd { "f*\nQ\n" } else { "f\nQ\n" });
                        }
                        Paint::Gradient(g, gspace) => {
                            let sh = ctx.shading(&mut res, &g);
                            content.push_str("q\n");
                            path_ops(&path, &mut content);
                            content.push_str(if even_odd { "W* n\n" } else { "W n\n" });
                            content.push_str(&cm(gspace));
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
                    let color = if transparency {
                        if a < 0.999 {
                            let gs = ctx.ext_gstate(&mut res, 1.0, a, MergeMode::Normal);
                            writeln!(content, "/{gs} gs").unwrap();
                        }
                        color
                    } else {
                        over_white(color)
                    };
                    if opts.overprint_black && is_black(color) {
                        let gs = ctx.overprint(&mut res);
                        writeln!(content, "/{gs} gs").unwrap();
                    }
                    writeln!(content, "{}", color_op(color, space, true)).unwrap();
                    stroke_ops(&style, &mut content);
                    path_ops(&path, &mut content);
                    content.push_str("S\nQ\n");
                }
                Prim::Image { image, xf, clip } => {
                    let Some(img) = doc.images.get(&image) else { continue };
                    let (iw, ih) = (img.width as f64, img.height as f64);
                    let target = ctx.downsample_width(image, xf);
                    let Some(id) = ctx.doc_image(image, target) else { continue };
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
                    if transparency {
                        let gs = ctx.ext_gstate(&mut res, opacity, opacity, blend);
                        stack.push((std::mem::take(&mut content), std::mem::take(&mut res), gs));
                    } else {
                        flat_groups += 1;
                    }
                }
                Prim::EndGroup => {
                    if !transparency {
                        flat_groups = flat_groups.saturating_sub(1);
                    } else if let Some((parent, pres, gs)) = stack.pop() {
                        let id = ctx.w.alloc();
                        let dict = format!(
                            "/Type /XObject /Subtype /Form /BBox [-100000 -100000 100000 100000] /Group << /S /Transparency /CS {} >> /Resources {}",
                            color_space(space),
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
        let _ = flat_groups;
        // close any unbalanced groups
        while let Some((parent, pres, _)) = stack.pop() {
            let mut c = parent;
            c.push_str(&content);
            content = c;
            for e in pres.ext_g {
                res.ext_g.push(e);
            }
        }
        content.push_str("Q\n");
        let content_id = ctx.w.alloc();
        ctx.w.stream(content_id, "", content.as_bytes(), opts.compress);
        let mut annots = vec![];
        for (r, url) in links {
            let id = ctx.w.alloc();
            ctx.w.obj(
                id,
                &format!(
                    "<< /Type /Annot /Subtype /Link /Rect [{} {} {} {}] /Border [0 0 0] /A << /S /URI /URI {} >> >>",
                    f(r.x0 + m),
                    f(pg.height + m - r.y1),
                    f(r.x1 + m),
                    f(pg.height + m - r.y0),
                    pdf_string(&url)
                ),
            );
            annots.push(format!("{id} 0 R"));
        }
        if opts.comments {
            for c in doc.comments.iter().filter(|c| c.page == pg.id) {
                let mut text = c.text.clone();
                for (who, t) in &c.replies {
                    text.push_str(&format!("\n{who}: {t}"));
                }
                let (x, y) = (c.pos.x + m, pg.height + m - c.pos.y);
                let id = ctx.w.alloc();
                ctx.w.obj(
                    id,
                    &format!(
                        "<< /Type /Annot /Subtype /Text /Rect [{} {} {} {}] /Contents {} /T {} /Name /Comment /Open false /F 4 >>",
                        f(x),
                        f(y - 20.0),
                        f(x + 20.0),
                        f(y),
                        pdf_string(&text),
                        pdf_string(&c.author)
                    ),
                );
                annots.push(format!("{id} 0 R"));
            }
        }
        let thumb = if opts.thumbnails {
            let dpi = 106.0 / pg.width.max(pg.height).max(1.0) * 72.0;
            vector_render::render_page_image(doc, pi, dpi, Some(Color::WHITE), Some(pg.rect()), true).map(|pm| {
                let rgba = vector_render::pixmap_to_rgba(&pm);
                let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
                let id = ctx.w.alloc();
                ctx.w.stream(id, &format!("/Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8", pm.width(), pm.height()), &rgb, true);
                format!(" /Thumb {id} 0 R")
            })
        } else {
            None
        };
        let page_id = ctx.w.alloc();
        let annots = if annots.is_empty() { String::new() } else { format!(" /Annots [{}]", annots.join(" ")) };
        let boxes = if m > 0.0 {
            let b = opts.bleed.max(0.0);
            format!(
                " /TrimBox [{} {} {} {}] /BleedBox [{} {} {} {}]",
                f(m),
                f(m),
                f(m + pg.width),
                f(m + pg.height),
                f(m - b),
                f(m - b),
                f(m + pg.width + b),
                f(m + pg.height + b)
            )
        } else {
            String::new()
        };
        let group = if transparency { format!(" /Group << /S /Transparency /CS {} >>", color_space(space)) } else { String::new() };
        ctx.w.obj(
            page_id,
            &format!(
                "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {} {}]{boxes} /Contents {content_id} 0 R /Resources {}{group}{annots}{} >>",
                f(pg.width + 2.0 * m),
                f(pg.height + 2.0 * m),
                res_dict(&res),
                thumb.unwrap_or_default()
            ),
        );
        kids.push(format!("{page_id} 0 R"));
        page_ids.push((page_id, if pg.name.is_empty() { format!("Page {}", pi + 1) } else { pg.name.clone() }));
    }
    ctx.w.obj(pages_id, &format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), kids.len()));
    // bookmarks: one per page
    let mut extra = String::new();
    if opts.bookmarks && !page_ids.is_empty() {
        let outlines = ctx.w.alloc();
        let items: Vec<usize> = page_ids.iter().map(|_| ctx.w.alloc()).collect();
        for (i, ((pid, title), id)) in page_ids.iter().zip(&items).enumerate() {
            let mut d = format!("<< /Title {} /Parent {outlines} 0 R /Dest [{pid} 0 R /Fit]", pdf_string(title));
            if i > 0 {
                write!(d, " /Prev {} 0 R", items[i - 1]).unwrap();
            }
            if i + 1 < items.len() {
                write!(d, " /Next {} 0 R", items[i + 1]).unwrap();
            }
            d.push_str(" >>");
            ctx.w.obj(*id, &d);
        }
        ctx.w.obj(outlines, &format!("<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {} >>", items[0], items[items.len() - 1], items.len()));
        write!(extra, " /Outlines {outlines} 0 R").unwrap();
    }
    write!(extra, " /PageMode /{}", opts.start.mode()).unwrap();
    // PDF/X and PDF/A identification
    let i = &doc.info;
    match opts.compat {
        PdfCompat::PdfX1a | PdfCompat::PdfX3 => {
            // the document's CMYK profile is the printing condition: its registered name, and
            // the profile itself when it is installed
            let profile = doc.settings.color.cmyk_profile.clone();
            let icc = vector_core::cms::current().cmyk_icc();
            let mut oi_dict = match vector_core::cms::output_condition(&profile) {
                Some((id, cond)) => format!("<< /Type /OutputIntent /S /GTS_PDFX /OutputConditionIdentifier {} /OutputCondition {} /RegistryName {} /Info {}", pdf_string(id), pdf_string(cond), pdf_string("http://www.color.org"), pdf_string(&profile)),
                None => format!("<< /Type /OutputIntent /S /GTS_PDFX /OutputConditionIdentifier {} /Info {}", pdf_string("Custom"), pdf_string(&profile)),
            };
            if let Some(icc) = icc {
                let pid = ctx.w.alloc();
                ctx.w.stream(pid, "/N 4", &icc, true);
                write!(oi_dict, " /DestOutputProfile {pid} 0 R").unwrap();
            }
            oi_dict.push_str(" >>");
            let oi = ctx.w.alloc();
            ctx.w.obj(oi, &oi_dict);
            write!(extra, " /OutputIntents [{oi} 0 R]").unwrap();
        }
        PdfCompat::PdfA1b => {
            let meta = ctx.w.alloc();
            let xmp = format!(
                "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
<rdf:Description rdf:about=\"\" xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\"><pdfaid:part>1</pdfaid:part><pdfaid:conformance>B</pdfaid:conformance></rdf:Description>\
<rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:title></rdf:Description>\
<rdf:Description rdf:about=\"\" xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\"><pdf:Producer>{}</pdf:Producer></rdf:Description>\
</rdf:RDF></x:xmpmeta>\n<?xpacket end=\"w\"?>",
                xml_escape(&i.title),
                vector_core::APP_NAME
            );
            // metadata streams stay uncompressed so tools can read them
            ctx.w.stream(meta, "/Type /Metadata /Subtype /XML", xmp.as_bytes(), false);
            write!(extra, " /Metadata {meta} 0 R").unwrap();
        }
        _ => {}
    }
    ctx.w.obj(catalog, &format!("<< /Type /Catalog /Pages {pages_id} 0 R{extra} >>"));
    let mut info = format!("<< /Creator {} /Producer {}", pdf_string(&format!("{} {}", vector_core::APP_NAME, vector_core::APP_VERSION)), pdf_string(vector_core::APP_NAME));
    for (k, v) in [("Title", &i.title), ("Author", &i.author), ("Subject", &i.subject), ("Keywords", &i.keywords)] {
        if !v.is_empty() {
            write!(info, " /{k} {}", pdf_string(v)).unwrap();
        }
    }
    match opts.compat {
        PdfCompat::PdfX1a => write!(info, " /GTS_PDFXVersion {} /GTS_PDFXConformance {} /Trapped /False", pdf_string("PDF/X-1:2001"), pdf_string("PDF/X-1a:2001")).unwrap(),
        PdfCompat::PdfX3 => write!(info, " /GTS_PDFXVersion {} /Trapped /False", pdf_string("PDF/X-3:2002")).unwrap(),
        _ => {}
    }
    info.push_str(" >>");
    ctx.w.obj(info_id, &info);
    let encrypt = match ctx.w.crypt.as_ref().map(|c| c.dict()) {
        Some(d) => {
            let id = ctx.w.alloc();
            ctx.w.obj_plain(id, &d);
            format!(" /Encrypt {id} 0 R")
        }
        None => String::new(),
    };
    // xref
    let xref_pos = ctx.w.buf.len();
    let n = ctx.w.offsets.len() + 1;
    write!(ctx.w.buf, "xref\n0 {n}\n0000000000 65535 f \n").unwrap();
    for off in ctx.w.offsets.clone() {
        write!(ctx.w.buf, "{off:010} 00000 n \n").unwrap();
    }
    let idh = hex(&id);
    write!(ctx.w.buf, "trailer\n<< /Size {n} /Root {catalog} 0 R /Info {info_id} 0 R{encrypt} /ID [<{idh}> <{idh}>] >>\nstartxref\n{xref_pos}\n%%EOF\n").unwrap();
    Ok(ctx.w.buf)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn export(doc: &Document, path: &Path, opts: &PdfOptions) -> Result<()> {
    let bytes = export_bytes(doc, opts)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::object::{CornerType, RectShape};
    use vector_core::{ObjKind, Object, Style};

    fn sample() -> Document {
        let mut doc = Document::new(200.0, 100.0);
        let id = doc.new_id();
        let mut o = Object::new(id, ObjKind::Rect(RectShape { w: 50.0, h: 40.0, radii: [0.0; 4], corner: CornerType::default() }), Affine::translate((10.0, 10.0)), Style::filled(Color::rgb(200, 30, 30)));
        o.style.fill = vector_core::style::Fill::Solid(Color::rgba(200, 30, 30, 128));
        doc.pages[0].layers[0].objects.push(o);
        doc.info.title = "Café".into();
        doc
    }

    #[test]
    fn ascii85_and_lzw() {
        assert_eq!(ascii85(b"Man "), b"9jqo^~>");
        assert_eq!(ascii85(b"\0\0\0\0ab"), b"z@:B~>");
        let data: Vec<u8> = (0..5000u32).map(|i| (i * 7 % 13) as u8).collect();
        let enc = lzw(&data);
        let dec = weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8).decode(&enc).unwrap();
        assert_eq!(dec, data);
    }

    #[test]
    fn settings_reach_the_file() {
        let doc = sample();
        let opts = PdfOptions {
            compat: PdfCompat::Acrobat6,
            color: PdfColor::Cmyk,
            bookmarks: true,
            thumbnails: true,
            start: PdfStart::Bookmarks,
            crop_marks: true,
            registration_marks: true,
            file_info: true,
            bleed: 9.0,
            compress: false,
            file_name: "test.cdr".into(),
            ..Default::default()
        };
        let b = export_bytes(&doc, &opts).unwrap();
        let s = String::from_utf8_lossy(&b);
        assert!(s.starts_with("%PDF-1.5"));
        assert!(s.contains("/PageMode /UseOutlines"));
        assert!(s.contains("/Outlines"));
        assert!(s.contains("/Thumb"));
        assert!(s.contains("/TrimBox [36 36 236 136]"));
        assert!(s.contains("/BleedBox [27 27 245 145]"));
        assert!(s.contains(" k\n"), "CMYK fill");
        assert!(s.contains("(test.cdr  Page 1"));
        assert!(s.contains("/BaseFont /Helvetica"));
        // Acrobat 4 flattens transparency: no ExtGState, no page group
        let b4 = export_bytes(&doc, &PdfOptions { compat: PdfCompat::Acrobat4, compress: false, ..Default::default() }).unwrap();
        let s4 = String::from_utf8_lossy(&b4);
        assert!(s4.starts_with("%PDF-1.3"));
        assert!(!s4.contains("/ExtGState") && !s4.contains("/S /Transparency"));
        // ASCII85 wraps every stream
        let a = export_bytes(&doc, &PdfOptions { ascii85: true, ..Default::default() }).unwrap();
        assert!(String::from_utf8_lossy(&a).contains("/Filter [/ASCII85Decode /FlateDecode]"));
    }

    fn cmyk_doc(profile: &str) -> Document {
        let mut doc = Document::new(200.0, 100.0);
        doc.settings.color.cmyk_profile = profile.into();
        let ink = vector_core::cms::with_settings(&doc.settings.color, || Color::from_cmyk(10.0, 20.0, 30.0, 40.0));
        for (i, c) in [ink, Color::rgb(255, 0, 0)].into_iter().enumerate() {
            let id = doc.new_id();
            let o = Object::new(id, ObjKind::Rect(RectShape { w: 50.0, h: 40.0, radii: [0.0; 4], corner: CornerType::default() }), Affine::translate((10.0 + 60.0 * i as f64, 10.0)), Style::filled(c));
            doc.pages[0].layers[0].objects.push(o);
        }
        doc
    }

    #[test]
    fn cmyk_colors_keep_their_values() {
        vector_core::cms::without_installed_profiles(|| {
            let doc = cmyk_doc("U.S. Web Coated (SWOP) v2");
            let opts = PdfOptions { color: PdfColor::Cmyk, compress: false, ..Default::default() };
            let s = String::from_utf8_lossy(&export_bytes(&doc, &opts).unwrap()).to_string();
            assert!(s.contains("0.1 0.2 0.3 0.4 k\n"), "the inks as entered");
            // RGB red goes through the profile, not the plain formula (0 1 1 0)
            assert!(!s.contains("0 1 1 0 k\n"));
            // Native: CMYK stays CMYK, RGB stays RGB
            let n = String::from_utf8_lossy(&export_bytes(&doc, &PdfOptions { color: PdfColor::Native, compress: false, ..Default::default() }).unwrap()).to_string();
            assert!(n.contains("0.1 0.2 0.3 0.4 k\n") && n.contains("1 0 0 rg\n"));
        });
    }

    #[test]
    fn pdf_x_output_intent_is_the_document_cmyk_profile() {
        vector_core::cms::without_installed_profiles(|| {
            let doc = cmyk_doc("Coated FOGRA39 (ISO 12647-2:2004)");
            let s = String::from_utf8_lossy(&export_bytes(&doc, &PdfOptions { compat: PdfCompat::PdfX1a, compress: false, ..Default::default() }).unwrap()).to_string();
            assert!(s.contains("/Type /OutputIntent /S /GTS_PDFX"));
            assert!(s.contains("0.1 0.2 0.3 0.4 k\n"));
            // the registered condition stands in for the profile file when it is not installed
            assert!(!s.contains("/DestOutputProfile"));
            // a profile that is not registered: a custom condition
            let doc = cmyk_doc("My Press");
            let s = String::from_utf8_lossy(&export_bytes(&doc, &PdfOptions { compat: PdfCompat::PdfX3, compress: false, ..Default::default() }).unwrap()).to_string();
            assert!(s.contains("/Type /OutputIntent"));
        });
        // with the profile installed, it is embedded
        if vector_core::cms::profile_data("U.S. Web Coated (SWOP) v2").is_some() {
            let doc = cmyk_doc("U.S. Web Coated (SWOP) v2");
            let b = export_bytes(&doc, &PdfOptions { compat: PdfCompat::PdfX1a, compress: false, ..Default::default() }).unwrap();
            let s = String::from_utf8_lossy(&b);
            assert!(s.contains("/DestOutputProfile"));
            assert!(s.contains("/N 4"));
            assert!(s.contains("0.1 0.2 0.3 0.4 k\n"));
        }
    }

    #[test]
    fn security_writes_an_encrypt_dictionary() {
        let doc = sample();
        let sec = PdfSecurity { open_password: String::new(), permission_password: "owner".into(), printing: 2, editing: 0, copying: false };
        let opts = PdfOptions { security: Some(sec), id_seed: Some(7), compat: PdfCompat::Acrobat4, ..Default::default() };
        let b = export_bytes(&doc, &opts).unwrap();
        let s = String::from_utf8_lossy(&b);
        assert!(s.starts_with("%PDF-1.4"), "security needs 1.4");
        assert!(s.contains("/Filter /Standard /V 2 /R 3 /Length 128"));
        assert!(s.contains("/Encrypt"));
        assert!(!s.contains("Caf"), "strings are encrypted");
        if let Ok(dir) = std::env::var("ANYVECTOR_PDF_OUT") {
            std::fs::write(std::path::Path::new(&dir).join("secured.pdf"), &b).unwrap();
        }
        // PDF/X never carries security
        let x = export_bytes(&doc, &PdfOptions { compat: PdfCompat::PdfX1a, ..opts }).unwrap();
        assert!(!String::from_utf8_lossy(&x).contains("/Encrypt"));
    }
}
