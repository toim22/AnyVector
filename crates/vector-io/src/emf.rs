//! Enhanced Metafile (EMF) export: an "EMF+ dual" metafile, the kind Office and GDI+ write.
//!
//! Every drawing primitive is written twice: as EMF+ records (kept in EMR_COMMENT records),
//! which GDI+ and Office play with Bézier curves, ARGB transparency, gradient brushes, dashed
//! pens and embedded PNG/JPEG images; and as plain GDI records, which older programs play
//! instead (curves as GDI Béziers, gradients as bands, no transparency). Text is converted to
//! curves, so the metafile looks the same without the fonts.
//!
//! Coordinates: the reference device is a 96 dpi display; one device pixel is
//! [`GDI_UNITS`] GDI logical units (an MM_ANISOTROPIC mapping), and the EMF+ records use device
//! pixels as floats. The frame gives the exact size in 0.01 mm.

use crate::flat::{FlatOptions, Flattener, Paint, Prim, StrokeStyle};
use crate::image_io::encode_png;
use crate::Result;
use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape};
use std::path::Path;
use vector_core::color::Color;
use vector_core::document::{Document, ImageData};
use vector_core::style::{Gradient, GradientKind, LineCap, LineJoin, SpreadMode};

#[derive(Clone, Debug)]
pub struct EmfOptions {
    /// Resolution of effects that have no vector equivalent (embedded as bitmaps).
    pub raster_dpi: f64,
    /// Write EMF+ records along with the GDI ones (dual mode); off = a plain GDI metafile.
    pub emf_plus: bool,
}

impl Default for EmfOptions {
    fn default() -> Self {
        EmfOptions { raster_dpi: 300.0, emf_plus: true }
    }
}

/// Resolution of the reference device (pixels per inch).
pub const DEVICE_DPI: f64 = 96.0;
/// Device pixels per point.
const K: f64 = DEVICE_DPI / 72.0;
/// GDI logical units per device pixel.
pub const GDI_UNITS: f64 = 16.0;
/// The reference device: 1920 x 1440 pixels on 508 x 381 mm (exactly 96 dpi).
const DEVICE_PX: (i32, i32) = (1920, 1440);
const DEVICE_MM: (i32, i32) = (508, 381);
/// Largest side of the GDI copy of a bitmap (the EMF+ copy keeps the full image).
const GDI_MAX_BITMAP: u32 = 2048;

// GDI record types ([MS-EMF] 2.1.1)
pub(crate) mod emr {
    pub const HEADER: u32 = 1;
    pub const POLYBEZIER: u32 = 2;
    pub const POLYGON: u32 = 3;
    pub const POLYLINE: u32 = 4;
    pub const POLYBEZIERTO: u32 = 5;
    pub const POLYLINETO: u32 = 6;
    pub const POLYPOLYLINE: u32 = 7;
    pub const POLYPOLYGON: u32 = 8;
    pub const SETWINDOWEXTEX: u32 = 9;
    pub const SETWINDOWORGEX: u32 = 10;
    pub const SETVIEWPORTEXTEX: u32 = 11;
    pub const SETVIEWPORTORGEX: u32 = 12;
    pub const EOF: u32 = 14;
    pub const SETMAPMODE: u32 = 17;
    pub const SETBKMODE: u32 = 18;
    pub const SETPOLYFILLMODE: u32 = 19;
    pub const SETSTRETCHBLTMODE: u32 = 21;
    pub const SETTEXTALIGN: u32 = 22;
    pub const SETTEXTCOLOR: u32 = 24;
    pub const MOVETOEX: u32 = 27;
    pub const SAVEDC: u32 = 33;
    pub const RESTOREDC: u32 = 34;
    pub const SETWORLDTRANSFORM: u32 = 35;
    pub const MODIFYWORLDTRANSFORM: u32 = 36;
    pub const SELECTOBJECT: u32 = 37;
    pub const CREATEPEN: u32 = 38;
    pub const CREATEBRUSHINDIRECT: u32 = 39;
    pub const DELETEOBJECT: u32 = 40;
    pub const ELLIPSE: u32 = 42;
    pub const RECTANGLE: u32 = 43;
    pub const LINETO: u32 = 54;
    pub const SETMITERLIMIT: u32 = 58;
    pub const BEGINPATH: u32 = 59;
    pub const ENDPATH: u32 = 60;
    pub const CLOSEFIGURE: u32 = 61;
    pub const FILLPATH: u32 = 62;
    pub const STROKEANDFILLPATH: u32 = 63;
    pub const STROKEPATH: u32 = 64;
    pub const SELECTCLIPPATH: u32 = 67;
    pub const COMMENT: u32 = 70;
    pub const STRETCHDIBITS: u32 = 81;
    pub const EXTCREATEFONTINDIRECTW: u32 = 82;
    pub const EXTTEXTOUTW: u32 = 84;
    pub const POLYBEZIER16: u32 = 85;
    pub const POLYGON16: u32 = 86;
    pub const POLYLINE16: u32 = 87;
    pub const POLYBEZIERTO16: u32 = 88;
    pub const POLYLINETO16: u32 = 89;
    pub const POLYPOLYLINE16: u32 = 90;
    pub const POLYPOLYGON16: u32 = 91;
    pub const EXTCREATEPEN: u32 = 95;
    pub const ALPHABLEND: u32 = 114;
}

// EMF+ record types ([MS-EMFPLUS] 2.1.1.1)
pub(crate) mod plus {
    pub const HEADER: u16 = 0x4001;
    pub const END_OF_FILE: u16 = 0x4002;
    pub const OBJECT: u16 = 0x4008;
    pub const FILL_PATH: u16 = 0x4014;
    pub const DRAW_PATH: u16 = 0x4015;
    pub const DRAW_IMAGE_POINTS: u16 = 0x401B;
    pub const SET_ANTI_ALIAS_MODE: u16 = 0x401E;
    pub const SET_INTERPOLATION_MODE: u16 = 0x4021;
    pub const SET_PIXEL_OFFSET_MODE: u16 = 0x4022;
    pub const SAVE: u16 = 0x4025;
    pub const RESTORE: u16 = 0x4026;
    pub const SET_PAGE_TRANSFORM: u16 = 0x4030;
    pub const SET_CLIP_PATH: u16 = 0x4033;
    // object types (EmfPlusObject flags, bits 8-14)
    pub const OBJ_BRUSH: u16 = 1;
    pub const OBJ_PEN: u16 = 2;
    pub const OBJ_PATH: u16 = 3;
    pub const OBJ_IMAGE: u16 = 5;
    pub const OBJ_IMAGE_ATTRIBUTES: u16 = 8;
    /// "EMF+" in an EMR_COMMENT.
    pub const COMMENT_ID: u32 = 0x2B46_4D45;
    /// Metafile signature 0xDBC01 and graphics version 2 (GDI+ 1.1).
    pub const VERSION: u32 = 0xDBC0_1002;
}

/// EMF+ object table slots used by the writer.
const SLOT_PATH: u8 = 0;
const SLOT_BRUSH: u8 = 1;
const SLOT_PEN: u8 = 2;
const SLOT_IMAGE: u8 = 3;
const SLOT_CLIP: u8 = 4;
const SLOT_ATTRS: u8 = 5;
/// GDI object table handles (index 0 is reserved).
const H_BRUSH: u32 = 1;
const H_PEN: u32 = 2;
const STOCK_NULL_BRUSH: u32 = 0x8000_0005;
const STOCK_NULL_PEN: u32 = 0x8000_0008;
/// EmfPlusObject records are split above this size (continued objects).
const MAX_OBJECT_CHUNK: usize = 0xF000;

/// Export one page to an EMF file.
pub fn export(doc: &Document, page: usize, path: &Path, opts: &EmfOptions) -> Result<()> {
    std::fs::write(path, page_to_emf(doc, page, opts))?;
    Ok(())
}

/// One page as EMF bytes.
pub fn page_to_emf(doc: &Document, page: usize, opts: &EmfOptions) -> Vec<u8> {
    let Some(pg) = doc.pages.get(page) else { return vec![] };
    let mut f = Flattener::new(doc, FlatOptions { raster_dpi: opts.raster_dpi, conical_gradients: false, repeat_gradients: true, gradient_alpha: true, printing: false });
    f.page(page);
    let prims = std::mem::take(&mut f.out);
    write(doc, &prims, Rect::new(0.0, 0.0, pg.width, pg.height), opts)
}

/// A list of objects as EMF bytes (clipboard): the picture is the objects' bounding box.
pub fn objects_to_emf(doc: &Document, objs: &[vector_core::Object], opts: &EmfOptions) -> Vec<u8> {
    page_to_emf(&crate::svg_export::objects_document(doc, objs), 0, opts)
}

/// Write the primitives of `area` (document points) as an EMF.
pub fn write(doc: &Document, prims: &[Prim], area: Rect, opts: &EmfOptions) -> Vec<u8> {
    let wpx = (area.width() * K).max(1.0);
    let hpx = (area.height() * K).max(1.0);
    let (wd, hd) = (wpx.ceil() as i32, hpx.ceil() as i32);
    let mut w = Writer { out: vec![], records: 0, plus: vec![], emf_plus: opts.emf_plus, origin: area.origin(), alpha: vec![1.0], stack: vec![], save_index: 0, attrs_written: false, made: (false, false) };
    if w.emf_plus {
        let mut d = vec![];
        for v in [plus::VERSION, 1, DEVICE_DPI as u32, DEVICE_DPI as u32] {
            put_u32(&mut d, v);
        }
        // flags: dual (EMF+ records with GDI fallback); EmfPlusFlags 1: the reference device is a display
        w.plus_rec(plus::HEADER, 1, &d);
        // anti-aliased (8x4), page unit = pixel at scale 1
        w.plus_rec(plus::SET_ANTI_ALIAS_MODE, (4 << 1) | 1, &[]);
        w.plus_rec(plus::SET_PAGE_TRANSFORM, 2, &1.0f32.to_le_bytes());
        w.plus_rec(plus::SET_INTERPOLATION_MODE, 7, &[]);
        w.plus_rec(plus::SET_PIXEL_OFFSET_MODE, 2, &[]);
    }
    // GDI: logical units are 1/16 device pixel
    w.rec(emr::SETMAPMODE, &le_u32(&[8])); // MM_ANISOTROPIC
    w.rec(emr::SETWINDOWEXTEX, &le_i32(&[wd * GDI_UNITS as i32, hd * GDI_UNITS as i32]));
    w.rec(emr::SETWINDOWORGEX, &le_i32(&[0, 0]));
    w.rec(emr::SETVIEWPORTEXTEX, &le_i32(&[wd, hd]));
    w.rec(emr::SETVIEWPORTORGEX, &le_i32(&[0, 0]));
    w.rec(emr::SETBKMODE, &le_u32(&[1])); // TRANSPARENT
    w.rec(emr::SETPOLYFILLMODE, &le_u32(&[2])); // WINDING
    w.rec(emr::SETSTRETCHBLTMODE, &le_u32(&[3])); // COLORONCOLOR
    for p in prims {
        w.prim(doc, p);
    }
    // close what is still open
    while let Some(k) = w.stack.pop() {
        w.end(k);
    }
    if w.emf_plus {
        w.plus_rec(plus::END_OF_FILE, 0, &[]);
    }
    w.rec(emr::EOF, &le_u32(&[0, 16, 20]));

    // header
    let title = if doc.info.title.is_empty() { "Drawing".to_string() } else { doc.info.title.clone() };
    let mut desc: Vec<u16> = vector_core::APP_NAME.encode_utf16().collect();
    desc.push(0);
    desc.extend(title.encode_utf16());
    desc.extend([0, 0]);
    let mut desc_bytes: Vec<u8> = desc.iter().flat_map(|c| c.to_le_bytes()).collect();
    while desc_bytes.len() % 4 != 0 {
        desc_bytes.push(0);
    }
    let hsize = 108 + desc_bytes.len();
    let total = hsize + w.out.len();
    let frame = (((area.width() / 72.0) * 2540.0).round() as i32, ((area.height() / 72.0) * 2540.0).round() as i32);
    let mut h = Vec::with_capacity(total);
    put_u32(&mut h, emr::HEADER);
    put_u32(&mut h, hsize as u32);
    for v in [0, 0, wd - 1, hd - 1, 0, 0, frame.0, frame.1] {
        put_i32(&mut h, v);
    }
    put_u32(&mut h, 0x464D_4520); // " EMF"
    put_u32(&mut h, 0x0001_0000);
    put_u32(&mut h, total as u32);
    put_u32(&mut h, w.records + 1);
    h.extend_from_slice(&3u16.to_le_bytes()); // handles: index 0, brush, pen
    h.extend_from_slice(&0u16.to_le_bytes());
    put_u32(&mut h, desc.len() as u32);
    put_u32(&mut h, 108);
    put_u32(&mut h, 0); // palette entries
    for v in [DEVICE_PX.0, DEVICE_PX.1, DEVICE_MM.0, DEVICE_MM.1] {
        put_i32(&mut h, v);
    }
    for v in [0u32, 0, 0] {
        put_u32(&mut h, v); // pixel format, OpenGL
    }
    put_i32(&mut h, DEVICE_MM.0 * 1000);
    put_i32(&mut h, DEVICE_MM.1 * 1000);
    h.extend_from_slice(&desc_bytes);
    h.extend_from_slice(&w.out);
    h
}

// ------------------------------------------------------------------------------------------
// little-endian helpers

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_f32(v: &mut Vec<u8>, x: f32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn le_u32(xs: &[u32]) -> Vec<u8> {
    xs.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn le_i32(xs: &[i32]) -> Vec<u8> {
    xs.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn pad4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

/// EmfPlusARGB: blue, green, red, alpha.
fn argb(c: Color) -> u32 {
    (c.a as u32) << 24 | (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32
}
/// GDI COLORREF: 0x00BBGGRR.
fn colorref(c: Color) -> u32 {
    (c.b as u32) << 16 | (c.g as u32) << 8 | c.r as u32
}

fn put_matrix(v: &mut Vec<u8>, a: Affine) {
    for c in a.as_coeffs() {
        put_f32(v, c as f32);
    }
}

/// The path as cubic segments only (quadratics raised), in the order of the elements.
fn cubic_elements(path: &BezPath) -> Vec<PathEl> {
    let mut out = Vec::with_capacity(path.elements().len());
    let (mut last, mut start) = (Point::ZERO, Point::ZERO);
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                start = p;
                last = p;
                out.push(PathEl::MoveTo(p));
            }
            PathEl::LineTo(p) => {
                last = p;
                out.push(PathEl::LineTo(p));
            }
            PathEl::QuadTo(c, p) => {
                let c1 = last + (c - last) * (2.0 / 3.0);
                let c2 = p + (c - p) * (2.0 / 3.0);
                last = p;
                out.push(PathEl::CurveTo(c1, c2, p));
            }
            PathEl::CurveTo(a, b, p) => {
                last = p;
                out.push(PathEl::CurveTo(a, b, p));
            }
            PathEl::ClosePath => {
                last = start;
                out.push(PathEl::ClosePath);
            }
        }
    }
    out
}

/// The color stops of a gradient, positions 0..1 with a stop at each end (acceleration, smooth
/// and color-wheel blends are sampled).
fn gradient_stops(g: &Gradient) -> Vec<(f64, Color)> {
    if g.accel.abs() > 1e-9 || g.smooth || g.blend != 0 {
        return (0..=32).map(|i| (i as f64 / 32.0, g.color_at(i as f64 / 32.0))).collect();
    }
    let mut s: Vec<(f64, Color)> = g.sorted_stops().iter().map(|s| (s.pos.clamp(0.0, 1.0), s.color)).collect();
    if s[0].0 > 0.0 {
        s.insert(0, (0.0, s[0].1));
    }
    if s[s.len() - 1].0 < 1.0 {
        s.push((1.0, s[s.len() - 1].1));
    }
    s
}

/// Color of the stops at `t` (0..1).
fn stop_color(stops: &[(f64, Color)], t: f64) -> Color {
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        if t <= w[1].0 {
            let span = w[1].0 - w[0].0;
            if span <= 1e-12 {
                return w[1].1;
            }
            return Color::lerp(w[0].1, w[1].1, (t - w[0].0) / span);
        }
    }
    stops[stops.len() - 1].1
}

/// Gradient parameter `t` (any value) folded into 0..1 by the spread mode.
fn spread_t(t: f64, spread: SpreadMode) -> f64 {
    match spread {
        SpreadMode::Pad => t.clamp(0.0, 1.0),
        SpreadMode::Repeat => t - t.floor(),
        SpreadMode::Reflect => {
            let m = t.rem_euclid(2.0);
            if m > 1.0 {
                2.0 - m
            } else {
                m
            }
        }
    }
}

/// The stops over the parameter range lo..hi (spread applied), as positions 0..1 of that range.
fn expand_stops(stops: &[(f64, Color)], spread: SpreadMode, lo: f64, hi: f64) -> Vec<(f32, Color)> {
    let span = (hi - lo).max(1e-9);
    let color = |t: f64| stop_color(stops, spread_t(t, spread));
    let mut ts = vec![lo, hi];
    let cycles = (hi.ceil() - lo.floor()).abs();
    if spread == SpreadMode::Pad || cycles > 256.0 {
        ts.extend(stops.iter().map(|s| s.0).filter(|t| *t > lo && *t < hi));
    } else {
        let mut k = lo.floor();
        while k < hi {
            for s in stops {
                let p = if spread == SpreadMode::Reflect && (k as i64).rem_euclid(2) == 1 { 1.0 - s.0 } else { s.0 };
                let t = k + p;
                if t > lo && t < hi {
                    ts.push(t);
                }
            }
            k += 1.0;
        }
    }
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<(f32, Color)> = vec![];
    let eps = span * 1e-5;
    for t in ts {
        let pos = ((t - lo) / span) as f32;
        let (before, after) = (color(t - eps), color(t + eps));
        if out.last().is_some_and(|l| (l.0 - pos).abs() < 1e-6 && l.1 == before && before == after) {
            continue;
        }
        if before != after && t > lo && t < hi {
            // a hard stop: two colors, a hair apart
            out.push((pos, before));
            out.push(((pos + 1e-4).min(1.0), after));
        } else {
            out.push((pos, color(t)));
        }
    }
    if let Some(f) = out.first_mut() {
        f.0 = 0.0;
    }
    if let Some(l) = out.last_mut() {
        l.0 = 1.0;
    }
    out
}

fn scale_alpha(c: Color, a: f32) -> Color {
    if a >= 0.999 {
        c
    } else {
        c.with_alpha((c.a as f32 * a).round().clamp(0.0, 255.0) as u8)
    }
}

/// A packed DIB (BITMAPINFOHEADER + 32-bit bottom-up pixels) from straight RGBA. `premultiply`
/// for AlphaBlend; otherwise `over` is the background the pixels are composited on (opaque).
pub fn dib_from_rgba(w: u32, h: u32, rgba: &[u8], premultiply: bool, over: Option<Color>) -> Vec<u8> {
    let mut v = Vec::with_capacity(40 + (w * h * 4) as usize);
    for x in [40u32, w, h] {
        put_u32(&mut v, x);
    }
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&32u16.to_le_bytes());
    for x in [0u32, w * h * 4, 2835, 2835, 0, 0] {
        put_u32(&mut v, x);
    }
    let row = (w * 4) as usize;
    for y in (0..h as usize).rev() {
        for p in rgba[y * row..(y + 1) * row].chunks_exact(4) {
            let a = p[3] as u32;
            let px = if premultiply {
                [(p[2] as u32 * a / 255) as u8, (p[1] as u32 * a / 255) as u8, (p[0] as u32 * a / 255) as u8, p[3]]
            } else if let Some(bg) = over {
                let mix = |c: u8, b: u8| ((c as u32 * a + b as u32 * (255 - a)) / 255) as u8;
                [mix(p[2], bg.b), mix(p[1], bg.g), mix(p[0], bg.r), 255]
            } else {
                [p[2], p[1], p[0], p[3]]
            };
            v.extend_from_slice(&px);
        }
    }
    v
}

#[derive(Clone, Copy)]
enum Open {
    Group,
    Clip(u32),
}

struct Writer {
    out: Vec<u8>,
    records: u32,
    /// EMF+ records waiting to be wrapped in an EMR_COMMENT.
    plus: Vec<u8>,
    emf_plus: bool,
    origin: Point,
    /// Group opacity (product of the open groups).
    alpha: Vec<f32>,
    stack: Vec<Open>,
    save_index: u32,
    attrs_written: bool,
    /// The GDI brush / pen handles exist (so they are deleted before being made again).
    made: (bool, bool),
}

impl Writer {
    fn a(&self) -> f32 {
        *self.alpha.last().unwrap_or(&1.0)
    }

    /// Document space to device pixels.
    fn dev_xf(&self) -> Affine {
        Affine::scale(K) * Affine::translate(-self.origin.to_vec2())
    }

    fn rec(&mut self, ty: u32, data: &[u8]) {
        self.flush_plus();
        let mut d = data.to_vec();
        pad4(&mut d);
        put_u32(&mut self.out, ty);
        put_u32(&mut self.out, 8 + d.len() as u32);
        self.out.extend_from_slice(&d);
        self.records += 1;
    }

    fn plus_rec(&mut self, ty: u16, flags: u16, data: &[u8]) {
        if !self.emf_plus {
            return;
        }
        let mut d = data.to_vec();
        pad4(&mut d);
        self.plus.extend_from_slice(&ty.to_le_bytes());
        self.plus.extend_from_slice(&flags.to_le_bytes());
        put_u32(&mut self.plus, 12 + d.len() as u32);
        put_u32(&mut self.plus, d.len() as u32);
        self.plus.extend_from_slice(&d);
    }

    fn flush_plus(&mut self) {
        if self.plus.is_empty() {
            return;
        }
        let p = std::mem::take(&mut self.plus);
        put_u32(&mut self.out, emr::COMMENT);
        put_u32(&mut self.out, 16 + p.len() as u32);
        put_u32(&mut self.out, 4 + p.len() as u32);
        put_u32(&mut self.out, plus::COMMENT_ID);
        self.out.extend_from_slice(&p);
        self.records += 1;
    }

    /// An EMF+ object, split into continued records when it is large.
    fn plus_object(&mut self, kind: u16, slot: u8, data: &[u8]) {
        if !self.emf_plus {
            return;
        }
        let flags = (kind << 8) | slot as u16;
        if data.len() <= MAX_OBJECT_CHUNK {
            self.plus_rec(plus::OBJECT, flags, data);
            return;
        }
        let total = data.len() as u32;
        let chunks: Vec<&[u8]> = data.chunks(MAX_OBJECT_CHUNK).collect();
        for c in chunks {
            // every part carries the continuation bit and the total size, as GDI+ writes them
            let mut d = Vec::with_capacity(c.len() + 4);
            put_u32(&mut d, total);
            d.extend_from_slice(c);
            self.plus_rec(plus::OBJECT, 0x8000 | flags, &d);
            self.flush_plus();
        }
    }

    // ------------------------------------------------------------------ EMF+ objects

    /// EmfPlusPath of a device-space path.
    fn plus_path(&mut self, slot: u8, path: &BezPath, winding: bool) {
        let mut pts: Vec<(f32, f32)> = vec![];
        let mut types: Vec<u8> = vec![];
        for el in cubic_elements(path) {
            match el {
                PathEl::MoveTo(p) => {
                    pts.push((p.x as f32, p.y as f32));
                    types.push(0);
                }
                PathEl::LineTo(p) => {
                    pts.push((p.x as f32, p.y as f32));
                    types.push(1);
                }
                PathEl::CurveTo(a, b, p) => {
                    for q in [a, b, p] {
                        pts.push((q.x as f32, q.y as f32));
                        types.push(3);
                    }
                }
                PathEl::ClosePath => {
                    if let Some(t) = types.last_mut() {
                        *t |= 0x80;
                    }
                }
                PathEl::QuadTo(..) => {}
            }
        }
        let mut d = vec![];
        put_u32(&mut d, plus::VERSION);
        put_u32(&mut d, pts.len() as u32);
        // 0x2000: nonzero winding fill (else alternate)
        put_u32(&mut d, if winding { 0x2000 } else { 0 });
        for (x, y) in &pts {
            put_f32(&mut d, *x);
            put_f32(&mut d, *y);
        }
        d.extend_from_slice(&types);
        pad4(&mut d);
        self.plus_object(plus::OBJ_PATH, slot, &d);
    }

    /// A gradient brush object for a device-space gradient covering `bounds` (device space).
    fn plus_gradient(&mut self, g: &Gradient, space: Affine, bounds: Rect) {
        let a = self.a();
        let stops: Vec<(f64, Color)> = gradient_stops(g).into_iter().map(|(t, c)| (t, scale_alpha(c, a))).collect();
        let corners = [bounds.origin(), Point::new(bounds.x1, bounds.y0), Point::new(bounds.x1, bounds.y1), Point::new(bounds.x0, bounds.y1)];
        let mut d = vec![];
        put_u32(&mut d, plus::VERSION);
        if g.kind == GradientKind::Elliptical {
            // path gradient: a circle around the center, radius 1 = the gradient's end
            let r = (g.end - g.start).hypot().max(1e-9);
            let m = self.dev_xf() * space * Affine::translate(g.start.to_vec2()) * Affine::scale(r);
            let inv = m.inverse();
            let reach = corners.iter().map(|c| (inv * *c).to_vec2().hypot()).fold(1.0f64, f64::max) * 1.02;
            let ex = expand_stops(&stops, g.spread, 0.0, reach);
            put_u32(&mut d, 3);
            put_u32(&mut d, 0x02 | 0x04); // transform, preset colors
            put_i32(&mut d, 4); // WrapModeClamp
            put_u32(&mut d, argb(ex[0].1));
            put_f32(&mut d, 0.0);
            put_f32(&mut d, 0.0);
            put_u32(&mut d, 1);
            put_u32(&mut d, argb(ex[ex.len() - 1].1));
            let n = 128;
            put_i32(&mut d, n);
            for i in 0..n {
                let t = i as f64 / n as f64 * std::f64::consts::TAU;
                put_f32(&mut d, (reach * t.cos()) as f32);
                put_f32(&mut d, (reach * t.sin()) as f32);
            }
            put_matrix(&mut d, m);
            // preset colors run from the boundary (0) to the center (1)
            put_u32(&mut d, ex.len() as u32);
            for (p, _) in ex.iter().rev() {
                put_f32(&mut d, 1.0 - p);
            }
            for (_, c) in ex.iter().rev() {
                put_u32(&mut d, argb(*c));
            }
        } else {
            // linear: x along the gradient (0 = start, 1 = end), widened to cover the shape
            let dv = g.end - g.start;
            let len = dv.hypot().max(1e-9);
            let m = self.dev_xf() * space * Affine::translate(g.start.to_vec2()) * Affine::rotate(dv.y.atan2(dv.x)) * Affine::scale(len);
            let inv = m.inverse();
            let xs: Vec<f64> = corners.iter().map(|c| (inv * *c).x).collect();
            let lo = xs.iter().copied().fold(0.0f64, f64::min) - 0.01;
            let hi = xs.iter().copied().fold(1.0f64, f64::max) + 0.01;
            let ex = expand_stops(&stops, g.spread, lo, hi);
            put_u32(&mut d, 4);
            put_u32(&mut d, 0x02 | 0x04);
            put_i32(&mut d, 0); // WrapModeTile (the rectangle covers the shape)
            for v in [lo as f32, 0.0, (hi - lo) as f32, 1.0] {
                put_f32(&mut d, v);
            }
            put_u32(&mut d, argb(ex[0].1));
            put_u32(&mut d, argb(ex[ex.len() - 1].1));
            put_u32(&mut d, 0);
            put_u32(&mut d, 0);
            put_matrix(&mut d, m);
            put_u32(&mut d, ex.len() as u32);
            for (p, _) in &ex {
                put_f32(&mut d, *p);
            }
            for (_, c) in &ex {
                put_u32(&mut d, argb(*c));
            }
        }
        self.plus_object(plus::OBJ_BRUSH, SLOT_BRUSH, &d);
    }

    fn plus_pen(&mut self, color: Color, st: &StrokeStyle) {
        let width = st.width.max(0.0) * K;
        let mut flags = 0x02 | 0x04 | 0x08;
        if st.join == LineJoin::Miter {
            flags |= 0x10;
        }
        let dashes = dash_pattern(&st.dash);
        if !dashes.is_empty() {
            flags |= 0x20 | 0x40 | 0x100;
        }
        let cap = match st.cap {
            LineCap::Butt => 0,
            LineCap::Square => 1,
            LineCap::Round => 2,
        };
        let mut d = vec![];
        put_u32(&mut d, plus::VERSION);
        put_u32(&mut d, 0);
        put_u32(&mut d, flags);
        put_u32(&mut d, 0); // UnitWorld
        put_f32(&mut d, width as f32);
        put_i32(&mut d, cap);
        put_i32(&mut d, cap);
        put_i32(&mut d, match st.join {
            LineJoin::Miter => 0,
            LineJoin::Bevel => 1,
            LineJoin::Round => 2,
        });
        if st.join == LineJoin::Miter {
            put_f32(&mut d, st.miter.max(1.0) as f32);
        }
        if !dashes.is_empty() {
            put_i32(&mut d, 5); // custom dashes
            put_i32(&mut d, if st.cap == LineCap::Round { 2 } else { 0 });
            put_u32(&mut d, dashes.len() as u32);
            // in pen widths (a hairline: in device pixels)
            let unit = if width > 0.0 { st.width } else { 1.0 / K };
            for v in &dashes {
                put_f32(&mut d, ((v / unit) as f32).max(0.01));
            }
        }
        // the pen's brush: solid
        put_u32(&mut d, plus::VERSION);
        put_u32(&mut d, 0);
        put_u32(&mut d, argb(scale_alpha(color, self.a())));
        self.plus_object(plus::OBJ_PEN, SLOT_PEN, &d);
    }

    // ------------------------------------------------------------------ GDI helpers

    fn log_pt(p: Point) -> (i32, i32) {
        ((p.x * GDI_UNITS).round() as i32, (p.y * GDI_UNITS).round() as i32)
    }

    /// RECTL bounds in device pixels of device-space points.
    fn bounds_of(pts: &[(i32, i32)]) -> [i32; 4] {
        let s = GDI_UNITS as i32;
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for (x, y) in pts {
            x0 = x0.min(*x);
            y0 = y0.min(*y);
            x1 = x1.max(*x);
            y1 = y1.max(*y);
        }
        if pts.is_empty() {
            return [0, 0, -1, -1];
        }
        [x0.div_euclid(s), y0.div_euclid(s), x1.div_euclid(s) + 1, y1.div_euclid(s) + 1]
    }

    fn poly_rec(&mut self, ty: u32, pts: &[(i32, i32)]) {
        if pts.is_empty() {
            return;
        }
        let mut d = le_i32(&Self::bounds_of(pts));
        put_u32(&mut d, pts.len() as u32);
        for (x, y) in pts {
            put_i32(&mut d, *x);
            put_i32(&mut d, *y);
        }
        self.rec(ty, &d);
    }

    /// A GDI path bracket for a device-space path.
    fn gdi_path(&mut self, path: &BezPath) {
        self.rec(emr::BEGINPATH, &[]);
        let mut lines: Vec<(i32, i32)> = vec![];
        let mut curves: Vec<(i32, i32)> = vec![];
        for el in cubic_elements(path) {
            match el {
                PathEl::MoveTo(p) => {
                    self.poly_rec(emr::POLYLINETO, &std::mem::take(&mut lines));
                    self.poly_rec(emr::POLYBEZIERTO, &std::mem::take(&mut curves));
                    let (x, y) = Self::log_pt(p);
                    self.rec(emr::MOVETOEX, &le_i32(&[x, y]));
                }
                PathEl::LineTo(p) => {
                    self.poly_rec(emr::POLYBEZIERTO, &std::mem::take(&mut curves));
                    lines.push(Self::log_pt(p));
                }
                PathEl::CurveTo(a, b, p) => {
                    self.poly_rec(emr::POLYLINETO, &std::mem::take(&mut lines));
                    curves.extend([Self::log_pt(a), Self::log_pt(b), Self::log_pt(p)]);
                }
                PathEl::ClosePath => {
                    self.poly_rec(emr::POLYLINETO, &std::mem::take(&mut lines));
                    self.poly_rec(emr::POLYBEZIERTO, &std::mem::take(&mut curves));
                    self.rec(emr::CLOSEFIGURE, &[]);
                }
                PathEl::QuadTo(..) => {}
            }
        }
        self.poly_rec(emr::POLYLINETO, &lines);
        self.poly_rec(emr::POLYBEZIERTO, &curves);
        self.rec(emr::ENDPATH, &[]);
    }

    fn dev_bounds(path: &BezPath) -> [i32; 4] {
        let b = path.bounding_box();
        [b.x0.floor() as i32, b.y0.floor() as i32, b.x1.ceil() as i32, b.y1.ceil() as i32]
    }

    fn gdi_brush(&mut self, c: Color) {
        if self.made.0 {
            self.rec(emr::SELECTOBJECT, &le_u32(&[STOCK_NULL_BRUSH]));
            self.rec(emr::DELETEOBJECT, &le_u32(&[H_BRUSH]));
        }
        self.made.0 = true;
        self.rec(emr::CREATEBRUSHINDIRECT, &le_u32(&[H_BRUSH, 0, colorref(c), 0]));
        self.rec(emr::SELECTOBJECT, &le_u32(&[H_BRUSH]));
    }

    fn gdi_pen(&mut self, c: Color, st: &StrokeStyle) {
        if self.made.1 {
            self.rec(emr::SELECTOBJECT, &le_u32(&[STOCK_NULL_PEN]));
            self.rec(emr::DELETEOBJECT, &le_u32(&[H_PEN]));
        }
        self.made.1 = true;
        let w =(st.width.max(0.0) * K * GDI_UNITS).round() as u32;
        let mut d = le_u32(&[H_PEN, 0, 0, 0, 0]);
        if w == 0 {
            // a hairline: one device pixel
            d.extend(le_u32(&[0, 1, 0, colorref(c), 0, 0]));
        } else {
            let dashes = dash_pattern(&st.dash);
            let mut style = 0x0001_0000; // PS_GEOMETRIC
            style |= match st.cap {
                LineCap::Round => 0,
                LineCap::Square => 0x100,
                LineCap::Butt => 0x200,
            };
            style |= match st.join {
                LineJoin::Round => 0,
                LineJoin::Bevel => 0x1000,
                LineJoin::Miter => 0x2000,
            };
            if !dashes.is_empty() {
                style |= 7; // PS_USERSTYLE
            }
            d.extend(le_u32(&[style, w, 0, colorref(c), 0, dashes.len() as u32]));
            for v in &dashes {
                put_u32(&mut d, ((v * K * GDI_UNITS).round() as u32).max(1));
            }
        }
        self.rec(emr::EXTCREATEPEN, &d);
        self.rec(emr::SELECTOBJECT, &le_u32(&[H_PEN]));
        if st.join == LineJoin::Miter && w > 0 {
            // an unsigned integer ([MS-EMF] 2.3.11.21): GDI fails the strokes after a FLOAT here
            self.rec(emr::SETMITERLIMIT, &(st.miter.max(1.0).round() as u32).to_le_bytes());
        }
    }

    fn gdi_fill_mode(&mut self, even_odd: bool) {
        self.rec(emr::SETPOLYFILLMODE, &le_u32(&[if even_odd { 1 } else { 2 }]));
    }

    // ------------------------------------------------------------------ primitives

    fn prim(&mut self, doc: &Document, p: &Prim) {
        match p {
            Prim::Fill { path, paint, even_odd } => self.fill(path, paint, *even_odd),
            Prim::Stroke { path, color, style } => self.stroke(path, *color, style),
            Prim::Image { image, xf, clip } => {
                let Some(img) = doc.images.get(image) else { return };
                self.image(img, *xf, Some(clip));
            }
            Prim::Raster { rect, w, h, rgba } => {
                if *w == 0 || *h == 0 {
                    return;
                }
                let xf = Affine::translate(rect.origin().to_vec2()) * Affine::scale_non_uniform(rect.width() / *w as f64, rect.height() / *h as f64);
                let img = ImageData { width: *w, height: *h, rgba: std::sync::Arc::new(rgba.clone()), encoded: Default::default(), format: "png".into(), name: String::new() };
                self.image(&img, xf, None);
            }
            Prim::BeginGroup { opacity, .. } => {
                // EMF has no transparency groups: the opacity goes into the colors
                let a = self.a() * opacity.clamp(0.0, 1.0);
                self.alpha.push(a);
                self.stack.push(Open::Group);
            }
            Prim::BeginClip { path } => {
                let dp = self.dev_xf() * path.clone();
                self.save_index += 1;
                let idx = self.save_index;
                let mut d = vec![];
                put_u32(&mut d, idx);
                self.plus_rec(plus::SAVE, 0, &d);
                self.plus_path(SLOT_CLIP, &dp, true);
                self.plus_rec(plus::SET_CLIP_PATH, (1 << 8) | SLOT_CLIP as u16, &[]); // intersect
                self.rec(emr::SAVEDC, &[]);
                self.gdi_fill_mode(false);
                self.gdi_path(&dp);
                self.rec(emr::SELECTCLIPPATH, &le_u32(&[1])); // RGN_AND
                self.stack.push(Open::Clip(idx));
            }
            Prim::EndGroup | Prim::EndClip => {
                if let Some(k) = self.stack.pop() {
                    self.end(k);
                }
            }
            Prim::Link { .. } => {}
        }
    }

    fn end(&mut self, k: Open) {
        match k {
            Open::Group => {
                self.alpha.pop();
            }
            Open::Clip(idx) => {
                let mut d = vec![];
                put_u32(&mut d, idx);
                self.plus_rec(plus::RESTORE, 0, &d);
                self.rec(emr::RESTOREDC, &le_i32(&[-1]));
            }
        }
    }

    fn fill(&mut self, path: &BezPath, paint: &Paint, even_odd: bool) {
        if path.elements().is_empty() {
            return;
        }
        let dp = self.dev_xf() * path.clone();
        let a = self.a();
        match paint {
            Paint::Solid(c) => {
                let c = scale_alpha(*c, a);
                if c.a == 0 {
                    return;
                }
                self.plus_path(SLOT_PATH, &dp, !even_odd);
                let mut d = vec![];
                put_u32(&mut d, argb(c));
                self.plus_rec(plus::FILL_PATH, 0x8000 | SLOT_PATH as u16, &d);
                self.gdi_fill_mode(even_odd);
                self.gdi_brush(c);
                self.gdi_path(&dp);
                self.rec(emr::FILLPATH, &le_i32(&Self::dev_bounds(&dp)));
            }
            Paint::Gradient(g, space) => {
                let bounds = dp.bounding_box();
                self.plus_path(SLOT_PATH, &dp, !even_odd);
                self.plus_gradient(g, *space, bounds);
                let mut d = vec![];
                put_u32(&mut d, SLOT_BRUSH as u32);
                self.plus_rec(plus::FILL_PATH, SLOT_PATH as u16, &d);
                self.gdi_gradient(&dp, even_odd, g, *space);
            }
        }
    }

    /// GDI fallback of a gradient: bands of color clipped to the shape.
    fn gdi_gradient(&mut self, dp: &BezPath, even_odd: bool, g: &Gradient, space: Affine) {
        let a = self.a();
        let stops = gradient_stops(g);
        let bounds = dp.bounding_box();
        let corners = [bounds.origin(), Point::new(bounds.x1, bounds.y0), Point::new(bounds.x1, bounds.y1), Point::new(bounds.x0, bounds.y1)];
        self.rec(emr::SAVEDC, &[]);
        self.gdi_fill_mode(even_odd);
        self.gdi_path(dp);
        self.rec(emr::SELECTCLIPPATH, &le_u32(&[1]));
        self.rec(emr::SELECTOBJECT, &le_u32(&[STOCK_NULL_PEN]));
        let n = 96;
        if g.kind == GradientKind::Elliptical {
            let r = (g.end - g.start).hypot().max(1e-9);
            let m = self.dev_xf() * space * Affine::translate(g.start.to_vec2()) * Affine::scale(r);
            let inv = m.inverse();
            let reach = corners.iter().map(|c| (inv * *c).to_vec2().hypot()).fold(1.0f64, f64::max) * 1.02;
            for k in (0..n).rev() {
                let t1 = reach * (k + 1) as f64 / n as f64;
                let c = scale_alpha(stop_color(&stops, spread_t(reach * (k as f64 + 0.5) / n as f64, g.spread)), a);
                let ring: Vec<(i32, i32)> = (0..72).map(|i| {
                    let ang = i as f64 / 72.0 * std::f64::consts::TAU;
                    Self::log_pt(m * Point::new(t1 * ang.cos(), t1 * ang.sin()))
                }).collect();
                self.gdi_brush(c);
                self.poly_rec(emr::POLYGON, &ring);
            }
        } else {
            let dv = g.end - g.start;
            let len = dv.hypot().max(1e-9);
            let m = self.dev_xf() * space * Affine::translate(g.start.to_vec2()) * Affine::rotate(dv.y.atan2(dv.x)) * Affine::scale(len);
            let inv = m.inverse();
            let local: Vec<Point> = corners.iter().map(|c| inv * *c).collect();
            let (lo, hi) = (local.iter().map(|p| p.x).fold(f64::MAX, f64::min), local.iter().map(|p| p.x).fold(f64::MIN, f64::max));
            let (v0, v1) = (local.iter().map(|p| p.y).fold(f64::MAX, f64::min), local.iter().map(|p| p.y).fold(f64::MIN, f64::max));
            let step = (hi - lo) / n as f64;
            for k in 0..n {
                let u0 = lo + step * k as f64;
                let u1 = u0 + step * 1.05;
                let c = scale_alpha(stop_color(&stops, spread_t(u0 + step * 0.5, g.spread)), a);
                let band: Vec<(i32, i32)> = [Point::new(u0, v0), Point::new(u1, v0), Point::new(u1, v1), Point::new(u0, v1)].iter().map(|p| Self::log_pt(m * *p)).collect();
                self.gdi_brush(c);
                self.poly_rec(emr::POLYGON, &band);
            }
        }
        self.rec(emr::RESTOREDC, &le_i32(&[-1]));
    }

    fn stroke(&mut self, path: &BezPath, color: Color, st: &StrokeStyle) {
        if path.elements().is_empty() {
            return;
        }
        let c = scale_alpha(color, self.a());
        if c.a == 0 {
            return;
        }
        let dp = self.dev_xf() * path.clone();
        self.plus_path(SLOT_PATH, &dp, true);
        self.plus_pen(color, st);
        let mut d = vec![];
        put_u32(&mut d, SLOT_PEN as u32);
        self.plus_rec(plus::DRAW_PATH, SLOT_PATH as u16, &d);
        self.gdi_pen(c, st);
        self.gdi_path(&dp);
        self.rec(emr::STROKEPATH, &le_i32(&Self::dev_bounds(&dp)));
    }

    /// A bitmap: `xf` maps its pixels to document space; `clip` (document space) limits it.
    fn image(&mut self, img: &ImageData, xf: Affine, clip: Option<&BezPath>) {
        let (w, h) = (img.width, img.height);
        if w == 0 || h == 0 || img.rgba.len() < (w * h * 4) as usize {
            return;
        }
        let a = self.a();
        let faded: Option<Vec<u8>> = (a < 0.999).then(|| {
            let mut v = img.rgba.to_vec();
            for p in v.chunks_exact_mut(4) {
                p[3] = (p[3] as f32 * a).round() as u8;
            }
            v
        });
        let rgba: &[u8] = faded.as_deref().unwrap_or(&img.rgba);
        let dxf = self.dev_xf() * xf;
        let clip_dev = clip.map(|c| self.dev_xf() * c.clone());
        // EMF+: the PNG/JPEG itself
        if self.emf_plus {
            self.save_index += 1;
            let idx = self.save_index;
            let mut d = vec![];
            put_u32(&mut d, idx);
            self.plus_rec(plus::SAVE, 0, &d);
            if let Some(c) = &clip_dev {
                self.plus_path(SLOT_CLIP, c, true);
                self.plus_rec(plus::SET_CLIP_PATH, (1 << 8) | SLOT_CLIP as u16, &[]);
            }
            let keep = faded.is_none() && !img.encoded.is_empty() && matches!(img.format.as_str(), "png" | "jpg" | "jpeg");
            let bytes = if keep { img.encoded.to_vec() } else { encode_png(w, h, rgba) };
            let mut d = vec![];
            put_u32(&mut d, plus::VERSION);
            put_u32(&mut d, 1); // bitmap
            put_i32(&mut d, w as i32);
            put_i32(&mut d, h as i32);
            put_i32(&mut d, 0);
            put_u32(&mut d, 0);
            put_u32(&mut d, 1); // compressed (PNG or JPEG)
            d.extend_from_slice(&bytes);
            pad4(&mut d);
            self.plus_object(plus::OBJ_IMAGE, SLOT_IMAGE, &d);
            if !self.attrs_written {
                // image attributes: tile-flip at the edges, so the borders are not faded
                let mut d = vec![];
                for v in [plus::VERSION, 0, 3, 0, 0, 0] {
                    put_u32(&mut d, v);
                }
                self.plus_object(plus::OBJ_IMAGE_ATTRIBUTES, SLOT_ATTRS, &d);
                self.attrs_written = true;
            }
            let mut d = vec![];
            put_u32(&mut d, SLOT_ATTRS as u32);
            put_i32(&mut d, 2); // UnitPixel
            for v in [0.0, 0.0, w as f32, h as f32] {
                put_f32(&mut d, v);
            }
            put_u32(&mut d, 3);
            for p in [Point::ZERO, Point::new(w as f64, 0.0), Point::new(0.0, h as f64)] {
                let q = dxf * p;
                put_f32(&mut d, q.x as f32);
                put_f32(&mut d, q.y as f32);
            }
            self.plus_rec(plus::DRAW_IMAGE_POINTS, SLOT_IMAGE as u16, &d);
            let mut d = vec![];
            put_u32(&mut d, idx);
            self.plus_rec(plus::RESTORE, 0, &d);
        }
        // GDI: a DIB (smaller when large), placed with a world transform
        let (gw, gh, small);
        let src: &[u8] = if w.max(h) > GDI_MAX_BITMAP {
            let s = GDI_MAX_BITMAP as f64 / w.max(h) as f64;
            gw = ((w as f64 * s).round() as u32).max(1);
            gh = ((h as f64 * s).round() as u32).max(1);
            let buf = image::RgbaImage::from_raw(w, h, rgba.to_vec()).unwrap_or_default();
            small = image::imageops::resize(&buf, gw, gh, image::imageops::FilterType::Triangle).into_raw();
            &small
        } else {
            gw = w;
            gh = h;
            rgba
        };
        let opaque = src.chunks_exact(4).all(|p| p[3] == 255);
        let corners: Vec<(i32, i32)> = [Point::ZERO, Point::new(w as f64, 0.0), Point::new(w as f64, h as f64), Point::new(0.0, h as f64)].iter().map(|p| Self::log_pt(dxf * *p)).collect();
        let bounds = Self::bounds_of(&corners);
        self.rec(emr::SAVEDC, &[]);
        if let Some(c) = &clip_dev {
            self.gdi_fill_mode(false);
            self.gdi_path(c);
            self.rec(emr::SELECTCLIPPATH, &le_u32(&[1]));
        }
        let wxf = Affine::scale(GDI_UNITS) * dxf;
        let mut d = vec![];
        put_matrix(&mut d, wxf);
        self.rec(emr::SETWORLDTRANSFORM, &d);
        let dib = dib_from_rgba(gw, gh, src, !opaque, None);
        let (bmi, bits) = dib.split_at(40);
        let mut d = le_i32(&bounds);
        if opaque {
            // EMR_STRETCHDIBITS
            for v in [0, 0, 0, 0, gw as i32, gh as i32] {
                put_i32(&mut d, v);
            }
            for v in [80u32, 40, 120, bits.len() as u32, 0, 0x00CC_0020] {
                put_u32(&mut d, v);
            }
            put_i32(&mut d, w as i32);
            put_i32(&mut d, h as i32);
            d.extend_from_slice(bmi);
            d.extend_from_slice(bits);
            self.rec(emr::STRETCHDIBITS, &d);
        } else {
            // EMR_ALPHABLEND with per-pixel alpha
            for v in [0, 0, w as i32, h as i32] {
                put_i32(&mut d, v);
            }
            d.extend_from_slice(&[0, 0, 255, 1]); // AC_SRC_OVER, constant alpha 255, AC_SRC_ALPHA
            put_i32(&mut d, 0);
            put_i32(&mut d, 0);
            put_matrix(&mut d, Affine::IDENTITY);
            for v in [0u32, 0, 108, 40, 148, bits.len() as u32] {
                put_u32(&mut d, v);
            }
            put_i32(&mut d, gw as i32);
            put_i32(&mut d, gh as i32);
            d.extend_from_slice(bmi);
            d.extend_from_slice(bits);
            self.rec(emr::ALPHABLEND, &d);
        }
        self.rec(emr::RESTOREDC, &le_i32(&[-1]));
    }
}

/// Dash lengths (document units) with an even count, or empty for a solid line.
fn dash_pattern(dash: &[f64]) -> Vec<f64> {
    if dash.is_empty() || dash.iter().all(|d| *d <= 0.0) {
        return vec![];
    }
    let mut v = dash.to_vec();
    if v.len() % 2 == 1 {
        v.extend_from_slice(dash);
    }
    v
}

#[cfg(test)]
#[path = "emf_tests.rs"]
mod tests;
