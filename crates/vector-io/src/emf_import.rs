//! Enhanced Metafile (EMF) import: the GDI records of a metafile (in an EMF+ dual metafile, its
//! GDI copy) as curves with fills and outlines, bitmaps and artistic text. Clipping paths become
//! PowerClips. EMF+ records are skipped, so an EMF+ only metafile (no GDI records) brings in
//! nothing.

use crate::emf::emr;
use crate::{IoError, Result};
use kurbo::{Affine, BezPath, Point, Shape};
use std::collections::HashMap;
use std::sync::Arc;
use vector_core::color::Color;
use vector_core::document::{Document, ImageData};
use vector_core::object::{ImageAdjust, ImageObj, ObjKind, Object};
use vector_core::path::NodePath;
use vector_core::style::*;
use vector_core::text;

/// Whether the bytes are an EMF (header record and " EMF" signature).
pub fn is_emf(data: &[u8]) -> bool {
    data.len() >= 88 && rd_u32(data, 0) == Some(emr::HEADER) && rd_u32(data, 40) == Some(0x464D_4520)
}

/// The metafile as a document the size of its frame.
pub fn document_from_emf(data: &[u8]) -> Result<Document> {
    let size = frame_size(data).ok_or_else(|| IoError::Format("not an enhanced metafile".into()))?;
    let mut doc = Document::new(size.0.max(1.0), size.1.max(1.0));
    let objs = objects_from_emf(&mut doc, data)?;
    doc.pages[0].layers[0].objects = objs;
    Ok(doc)
}

/// The size of the metafile's frame in points.
pub fn frame_size(data: &[u8]) -> Option<(f64, f64)> {
    if !is_emf(data) {
        return None;
    }
    let f = [rd_i32(data, 24)?, rd_i32(data, 28)?, rd_i32(data, 32)?, rd_i32(data, 36)?];
    let mm = |v: i32| v as f64 / 100.0 / 25.4 * 72.0;
    Some((mm(f[2] - f[0]), mm(f[3] - f[1])))
}

/// The metafile's drawing as objects (one group when there are several), placed with the
/// frame's top-left corner at the origin.
pub fn objects_from_emf(doc: &mut Document, data: &[u8]) -> Result<Vec<Object>> {
    if !is_emf(data) {
        return Err(IoError::Format("not an enhanced metafile".into()));
    }
    let mut r = Reader::new(doc, data);
    r.run();
    r.close_clip();
    let mut objs = std::mem::take(&mut r.out);
    if objs.is_empty() {
        return Err(IoError::Format("the metafile has no drawing that can be read".into()));
    }
    if objs.len() > 1 {
        let id = doc.new_id();
        let mut st = Style::default();
        st.outline = Outline::none();
        st.fill = Fill::None;
        objs = vec![Object::new(id, ObjKind::Group(objs), Affine::IDENTITY, st)];
    }
    Ok(objs)
}

fn rd_u32(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn rd_i32(d: &[u8], o: usize) -> Option<i32> {
    rd_u32(d, o).map(|v| v as i32)
}
fn rd_i16(d: &[u8], o: usize) -> Option<i16> {
    d.get(o..o + 2).map(|b| i16::from_le_bytes([b[0], b[1]]))
}
fn rd_f32(d: &[u8], o: usize) -> Option<f32> {
    rd_u32(d, o).map(f32::from_bits)
}
fn colorref(v: u32) -> Color {
    Color::rgb((v & 0xff) as u8, ((v >> 8) & 0xff) as u8, ((v >> 16) & 0xff) as u8)
}
fn xform(d: &[u8], o: usize) -> Option<Affine> {
    let mut c = [0.0; 6];
    for (i, v) in c.iter_mut().enumerate() {
        *v = rd_f32(d, o + i * 4)? as f64;
    }
    c.iter().all(|v| v.is_finite()).then(|| Affine::new(c))
}

#[derive(Clone, Debug)]
struct Pen {
    color: Color,
    /// Logical units; 0 = a cosmetic (one pixel) pen.
    width: f64,
    style: u32,
    dashes: Vec<f64>,
}

#[derive(Clone, Debug)]
struct Font {
    height: f64,
    escapement: f64,
    weight: i32,
    italic: bool,
    underline: bool,
    strike: bool,
    face: String,
}

#[derive(Clone, Debug)]
enum Gdi {
    Brush(Option<Color>),
    Pen(Option<Pen>),
    Font(Font),
    Other,
}

#[derive(Clone)]
struct Dc {
    map_mode: u32,
    win_org: Point,
    win_ext: (f64, f64),
    vp_org: Point,
    vp_ext: (f64, f64),
    world: Affine,
    brush: Option<Color>,
    pen: Option<Pen>,
    font: Option<Font>,
    even_odd: bool,
    text_color: Color,
    text_align: u32,
    miter: f64,
    /// Current position (logical units).
    pos: Point,
    /// Clipping path (document space).
    clip: Option<BezPath>,
}

impl Default for Dc {
    fn default() -> Self {
        Dc {
            map_mode: 1,
            win_org: Point::ZERO,
            win_ext: (1.0, 1.0),
            vp_org: Point::ZERO,
            vp_ext: (1.0, 1.0),
            world: Affine::IDENTITY,
            brush: Some(Color::WHITE),
            pen: Some(Pen { color: Color::BLACK, width: 0.0, style: 0, dashes: vec![] }),
            font: None,
            even_odd: true,
            text_color: Color::BLACK,
            text_align: 0,
            miter: 10.0,
            pos: Point::ZERO,
            clip: None,
        }
    }
}

struct Reader<'a> {
    doc: &'a mut Document,
    data: &'a [u8],
    dc: Dc,
    saved: Vec<Dc>,
    objects: HashMap<u32, Gdi>,
    /// Device pixels per millimeter of the reference device.
    px_mm: (f64, f64),
    /// Device position of the frame's top-left corner.
    frame_dev: Point,
    /// The path being built between BEGINPATH and ENDPATH, and the finished one.
    building: Option<BezPath>,
    path: Option<BezPath>,
    /// Connected lines drawn outside a path, stroked together.
    lines: Option<BezPath>,
    out: Vec<Object>,
    /// Objects drawn while a clipping path is active.
    clipped: Option<(BezPath, Vec<Object>)>,
}

impl<'a> Reader<'a> {
    fn new(doc: &'a mut Document, data: &'a [u8]) -> Self {
        let dev = (rd_i32(data, 72).unwrap_or(1920) as f64, rd_i32(data, 76).unwrap_or(1080) as f64);
        let mm = (rd_i32(data, 80).unwrap_or(508) as f64, rd_i32(data, 84).unwrap_or(286) as f64);
        let px_mm = (if mm.0 > 0.0 && dev.0 > 0.0 { dev.0 / mm.0 } else { 96.0 / 25.4 }, if mm.1 > 0.0 && dev.1 > 0.0 { dev.1 / mm.1 } else { 96.0 / 25.4 });
        let frame_dev = Point::new(rd_i32(data, 24).unwrap_or(0) as f64 / 100.0 * px_mm.0, rd_i32(data, 28).unwrap_or(0) as f64 / 100.0 * px_mm.1);
        Reader { doc, data, dc: Dc::default(), saved: vec![], objects: HashMap::new(), px_mm, frame_dev, building: None, path: None, lines: None, out: vec![], clipped: None }
    }

    /// Logical units to document points.
    fn xf(&self) -> Affine {
        let dc = &self.dc;
        let page_to_dev = match dc.map_mode {
            7 | 8 => {
                let mut sx = dc.vp_ext.0 / if dc.win_ext.0 == 0.0 { 1.0 } else { dc.win_ext.0 };
                let mut sy = dc.vp_ext.1 / if dc.win_ext.1 == 0.0 { 1.0 } else { dc.win_ext.1 };
                if dc.map_mode == 7 {
                    let s = sx.abs().min(sy.abs());
                    sx = s * sx.signum();
                    sy = s * sy.signum();
                }
                Affine::translate(dc.vp_org.to_vec2()) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-dc.win_org.to_vec2())
            }
            2..=6 => {
                let unit_mm = match dc.map_mode {
                    2 => 0.1,
                    3 => 0.01,
                    4 => 0.254,
                    5 => 0.0254,
                    _ => 25.4 / 1440.0,
                };
                Affine::translate(dc.vp_org.to_vec2()) * Affine::scale_non_uniform(unit_mm * self.px_mm.0, -unit_mm * self.px_mm.1) * Affine::translate(-dc.win_org.to_vec2())
            }
            _ => Affine::translate(dc.vp_org.to_vec2() - dc.win_org.to_vec2()),
        };
        let dev_to_doc = Affine::scale_non_uniform(72.0 / 25.4 / self.px_mm.0, 72.0 / 25.4 / self.px_mm.1) * Affine::translate(-self.frame_dev.to_vec2());
        dev_to_doc * page_to_dev * dc.world
    }

    fn scale(&self) -> f64 {
        self.xf().determinant().abs().sqrt()
    }

    fn run(&mut self) {
        let data = self.data;
        let mut off = 0usize;
        while off + 8 <= data.len() {
            let (Some(ty), Some(size)) = (rd_u32(data, off), rd_u32(data, off + 4)) else { break };
            let size = size as usize;
            if size < 8 || off + size > data.len() {
                break;
            }
            let rec = &data[off..off + size];
            if ty == emr::EOF {
                break;
            }
            if !matches!(ty, emr::LINETO | emr::POLYLINETO | emr::POLYLINETO16 | emr::POLYBEZIERTO | emr::POLYBEZIERTO16 | emr::MOVETOEX) || self.building.is_some() {
                self.flush_lines();
            }
            self.record(ty, rec);
            off += size;
        }
        self.flush_lines();
    }

    fn points(&self, rec: &[u8], at: usize, n: usize, small: bool) -> Vec<Point> {
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            let p = if small {
                match (rd_i16(rec, at + i * 4), rd_i16(rec, at + i * 4 + 2)) {
                    (Some(x), Some(y)) => Point::new(x as f64, y as f64),
                    _ => break,
                }
            } else {
                match (rd_i32(rec, at + i * 8), rd_i32(rec, at + i * 8 + 4)) {
                    (Some(x), Some(y)) => Point::new(x as f64, y as f64),
                    _ => break,
                }
            };
            v.push(p);
        }
        v
    }

    /// The point array of a POLY* record (bounds, count, points).
    fn poly(&self, rec: &[u8], small: bool) -> Vec<Point> {
        let n = rd_u32(rec, 24).unwrap_or(0) as usize;
        self.points(rec, 28, n.min(rec.len() / 4), small)
    }

    /// The groups of a POLYPOLY* record.
    fn polypoly(&self, rec: &[u8], small: bool) -> Vec<Vec<Point>> {
        let n = rd_u32(rec, 24).unwrap_or(0) as usize;
        let counts: Vec<usize> = (0..n.min(rec.len() / 4)).filter_map(|i| rd_u32(rec, 32 + i * 4).map(|c| c as usize)).collect();
        let total: usize = counts.iter().sum();
        let pts = self.points(rec, 32 + n * 4, total.min(rec.len() / 4), small);
        let mut out = vec![];
        let mut k = 0;
        for c in counts {
            if k + c > pts.len() {
                break;
            }
            out.push(pts[k..k + c].to_vec());
            k += c;
        }
        out
    }

    fn record(&mut self, ty: u32, rec: &[u8]) {
        let xf = self.xf();
        match ty {
            emr::SETMAPMODE => self.dc.map_mode = rd_u32(rec, 8).unwrap_or(1),
            emr::SETWINDOWEXTEX => self.dc.win_ext = (rd_i32(rec, 8).unwrap_or(1) as f64, rd_i32(rec, 12).unwrap_or(1) as f64),
            emr::SETWINDOWORGEX => self.dc.win_org = Point::new(rd_i32(rec, 8).unwrap_or(0) as f64, rd_i32(rec, 12).unwrap_or(0) as f64),
            emr::SETVIEWPORTEXTEX => self.dc.vp_ext = (rd_i32(rec, 8).unwrap_or(1) as f64, rd_i32(rec, 12).unwrap_or(1) as f64),
            emr::SETVIEWPORTORGEX => self.dc.vp_org = Point::new(rd_i32(rec, 8).unwrap_or(0) as f64, rd_i32(rec, 12).unwrap_or(0) as f64),
            emr::SETPOLYFILLMODE => self.dc.even_odd = rd_u32(rec, 8) != Some(2),
            emr::SETTEXTCOLOR => self.dc.text_color = colorref(rd_u32(rec, 8).unwrap_or(0)),
            emr::SETTEXTALIGN => self.dc.text_align = rd_u32(rec, 8).unwrap_or(0),
            emr::SETMITERLIMIT => {
                // an unsigned integer; some writers put a FLOAT there
                let v = rd_u32(rec, 8).unwrap_or(10);
                let m = if v < 0x10000 { v as f64 } else { f32::from_bits(v) as f64 };
                self.dc.miter = if m.is_finite() && m >= 1.0 { m } else { 10.0 };
            }
            emr::SETWORLDTRANSFORM => {
                if let Some(m) = xform(rec, 8) {
                    self.dc.world = m;
                }
            }
            emr::MODIFYWORLDTRANSFORM => {
                let m = xform(rec, 8).unwrap_or(Affine::IDENTITY);
                match rd_u32(rec, 32).unwrap_or(1) {
                    1 => self.dc.world = Affine::IDENTITY,
                    2 => self.dc.world = self.dc.world * m,
                    3 => self.dc.world = m * self.dc.world,
                    4 => self.dc.world = m,
                    _ => {}
                }
            }
            emr::SAVEDC => self.saved.push(self.dc.clone()),
            emr::RESTOREDC => {
                let n = rd_i32(rec, 8).unwrap_or(-1);
                let keep = if n < 0 { self.saved.len().saturating_sub((-n) as usize) } else { (n as usize).saturating_sub(1).min(self.saved.len()) };
                if keep < self.saved.len() {
                    let dc = self.saved[keep].clone();
                    self.saved.truncate(keep);
                    self.set_clip(dc.clip.clone());
                    self.dc = dc;
                }
            }
            emr::CREATEBRUSHINDIRECT => {
                let (Some(h), Some(style), Some(c)) = (rd_u32(rec, 8), rd_u32(rec, 12), rd_u32(rec, 16)) else { return };
                let b = match style {
                    1 => None,
                    _ => Some(colorref(c)),
                };
                self.objects.insert(h, Gdi::Brush(b));
            }
            94 | 93 => {
                // pattern brushes: a mid gray
                if let Some(h) = rd_u32(rec, 8) {
                    self.objects.insert(h, Gdi::Brush(Some(Color::rgb(128, 128, 128))));
                }
            }
            emr::CREATEPEN => {
                let (Some(h), Some(style), Some(w), Some(c)) = (rd_u32(rec, 8), rd_u32(rec, 12), rd_i32(rec, 16), rd_u32(rec, 24)) else { return };
                let pen = (style & 0xf != 5).then(|| Pen { color: colorref(c), width: w.max(0) as f64, style, dashes: vec![] });
                self.objects.insert(h, Gdi::Pen(pen));
            }
            emr::EXTCREATEPEN => {
                let (Some(h), Some(style), Some(w), Some(bs), Some(c), Some(n)) = (rd_u32(rec, 8), rd_u32(rec, 28), rd_u32(rec, 32), rd_u32(rec, 36), rd_u32(rec, 40), rd_u32(rec, 48)) else { return };
                let dashes: Vec<f64> = (0..(n as usize).min(64)).filter_map(|i| rd_u32(rec, 52 + i * 4).map(|v| v as f64)).collect();
                let geometric = style & 0x000F_0000 == 0x0001_0000;
                let pen = (style & 0xf != 5 && bs != 1).then(|| Pen { color: colorref(c), width: if geometric { w as f64 } else { 0.0 }, style, dashes });
                self.objects.insert(h, Gdi::Pen(pen));
            }
            emr::EXTCREATEFONTINDIRECTW => {
                let Some(h) = rd_u32(rec, 8) else { return };
                let face: Vec<u16> = (0..32).filter_map(|i| rec.get(40 + i * 2..42 + i * 2).map(|b| u16::from_le_bytes([b[0], b[1]]))).take_while(|c| *c != 0).collect();
                let f = Font {
                    height: rd_i32(rec, 12).unwrap_or(-12) as f64,
                    escapement: rd_i32(rec, 20).unwrap_or(0) as f64 / 10.0,
                    weight: rd_i32(rec, 28).unwrap_or(400),
                    italic: rec.get(32).is_some_and(|b| *b != 0),
                    underline: rec.get(33).is_some_and(|b| *b != 0),
                    strike: rec.get(34).is_some_and(|b| *b != 0),
                    face: String::from_utf16_lossy(&face),
                };
                self.objects.insert(h, Gdi::Font(f));
            }
            emr::SELECTOBJECT => {
                let Some(h) = rd_u32(rec, 8) else { return };
                if h & 0x8000_0000 != 0 {
                    let gray = |v: u8| Some(Color::rgb(v, v, v));
                    match h & 0x7fff_ffff {
                        0 => self.dc.brush = gray(255),
                        1 => self.dc.brush = gray(192),
                        2 => self.dc.brush = gray(128),
                        3 => self.dc.brush = gray(64),
                        4 => self.dc.brush = gray(0),
                        5 => self.dc.brush = None,
                        6 => self.dc.pen = Some(Pen { color: Color::WHITE, width: 0.0, style: 0, dashes: vec![] }),
                        7 => self.dc.pen = Some(Pen { color: Color::BLACK, width: 0.0, style: 0, dashes: vec![] }),
                        8 => self.dc.pen = None,
                        _ => {}
                    }
                    return;
                }
                match self.objects.get(&h) {
                    Some(Gdi::Brush(b)) => self.dc.brush = *b,
                    Some(Gdi::Pen(p)) => self.dc.pen = p.clone(),
                    Some(Gdi::Font(f)) => self.dc.font = Some(f.clone()),
                    _ => {}
                }
            }
            emr::DELETEOBJECT => {
                if let Some(h) = rd_u32(rec, 8) {
                    self.objects.insert(h, Gdi::Other);
                }
            }
            emr::BEGINPATH => {
                self.building = Some(BezPath::new());
                self.path = None;
            }
            emr::ENDPATH => self.path = self.building.take(),
            emr::CLOSEFIGURE => {
                if let Some(b) = self.building.as_mut() {
                    b.close_path();
                }
            }
            emr::MOVETOEX => {
                let p = Point::new(rd_i32(rec, 8).unwrap_or(0) as f64, rd_i32(rec, 12).unwrap_or(0) as f64);
                self.dc.pos = p;
                let q = xf * p;
                if let Some(b) = self.building.as_mut() {
                    b.move_to(q);
                } else if let Some(l) = self.lines.as_mut() {
                    l.move_to(q);
                }
            }
            emr::LINETO => {
                let p = Point::new(rd_i32(rec, 8).unwrap_or(0) as f64, rd_i32(rec, 12).unwrap_or(0) as f64);
                self.line_to_points(&[p], false);
            }
            emr::POLYLINETO | emr::POLYLINETO16 => {
                let pts = self.poly(rec, ty == emr::POLYLINETO16);
                self.line_to_points(&pts, false);
            }
            emr::POLYBEZIERTO | emr::POLYBEZIERTO16 => {
                let pts = self.poly(rec, ty == emr::POLYBEZIERTO16);
                self.line_to_points(&pts, true);
            }
            emr::POLYGON | emr::POLYGON16 | emr::POLYLINE | emr::POLYLINE16 | emr::POLYBEZIER | emr::POLYBEZIER16 => {
                let small = matches!(ty, emr::POLYGON16 | emr::POLYLINE16 | emr::POLYBEZIER16);
                let pts = self.poly(rec, small);
                let closed = matches!(ty, emr::POLYGON | emr::POLYGON16);
                let bez = matches!(ty, emr::POLYBEZIER | emr::POLYBEZIER16);
                let p = figure(&pts, closed, bez, xf);
                self.shape(p, closed);
            }
            emr::POLYPOLYGON | emr::POLYPOLYGON16 | emr::POLYPOLYLINE | emr::POLYPOLYLINE16 => {
                let small = matches!(ty, emr::POLYPOLYGON16 | emr::POLYPOLYLINE16);
                let closed = matches!(ty, emr::POLYPOLYGON | emr::POLYPOLYGON16);
                let mut all = BezPath::new();
                for pts in self.polypoly(rec, small) {
                    all.extend(figure(&pts, closed, false, xf).elements().iter().copied());
                }
                self.shape(all, closed);
            }
            emr::RECTANGLE | emr::ELLIPSE | 44 => {
                let (Some(l), Some(t), Some(r), Some(b)) = (rd_i32(rec, 8), rd_i32(rec, 12), rd_i32(rec, 16), rd_i32(rec, 20)) else { return };
                let rect = kurbo::Rect::new(l as f64, t as f64, r as f64, b as f64);
                let local = match ty {
                    emr::RECTANGLE => rect.to_path(0.1),
                    emr::ELLIPSE => kurbo::Ellipse::from_rect(rect).to_path(0.1),
                    _ => {
                        let cw = rd_i32(rec, 24).unwrap_or(0) as f64 / 2.0;
                        rect.to_rounded_rect(cw.min(rect.width().abs() / 2.0).min(rect.height().abs() / 2.0)).to_path(0.1)
                    }
                };
                self.shape(xf * local, true);
            }
            emr::FILLPATH | emr::STROKEPATH | emr::STROKEANDFILLPATH => {
                let Some(p) = self.path.take() else { return };
                let fill = ty != emr::STROKEPATH;
                let stroke = ty != emr::FILLPATH;
                self.emit_path(p, fill, stroke);
            }
            emr::SELECTCLIPPATH => {
                let Some(p) = self.path.take() else { return };
                let mode = rd_u32(rec, 8).unwrap_or(5);
                let clip = match (&self.dc.clip, mode) {
                    (Some(old), 1) => vector_core::geom::boolean(old, &p, vector_core::geom::BoolOp::Intersect, self.dc.even_odd),
                    _ => p,
                };
                self.set_clip(Some(clip.clone()));
                self.dc.clip = Some(clip);
            }
            emr::STRETCHDIBITS => {
                let g = |o| rd_i32(rec, o).unwrap_or(0);
                let dest = (g(24) as f64, g(28) as f64, g(72) as f64, g(76) as f64);
                let src = (g(32), g(36), g(40), g(44));
                let (ob, cb, obits, cbits) = (g(48) as usize, g(52) as usize, g(56) as usize, g(60) as usize);
                self.bitmap(rec, (ob, cb, obits, cbits), src, dest, false, xf);
            }
            76 | 77 | emr::ALPHABLEND => {
                let g = |o| rd_i32(rec, o).unwrap_or(0);
                let dest = (g(24) as f64, g(28) as f64, g(32) as f64, g(36) as f64);
                let (ob, cb, obits, cbits) = (g(84) as usize, g(88) as usize, g(92) as usize, g(96) as usize);
                if cb == 0 {
                    // a pattern fill of the destination with the brush (PATCOPY)
                    if ty != emr::ALPHABLEND && g(40) as u32 == 0x00F0_0021 {
                        if let Some(c) = self.dc.brush {
                            let r = kurbo::Rect::new(dest.0, dest.1, dest.0 + dest.2, dest.1 + dest.3);
                            self.emit(xf * r.to_path(0.1), Some(c), None);
                        }
                    }
                    return;
                }
                let (cx, cy) = if ty == 76 { (g(32), g(36)) } else { (g(100), g(104)) };
                let src = (g(44), g(48), cx, cy);
                self.bitmap(rec, (ob, cb, obits, cbits), src, dest, ty == emr::ALPHABLEND, xf);
            }
            emr::EXTTEXTOUTW => self.text(rec, xf),
            _ => {}
        }
    }

    fn line_to_points(&mut self, pts: &[Point], bez: bool) {
        if pts.is_empty() {
            return;
        }
        let xf = self.xf();
        let start = xf * self.dc.pos;
        let target = if let Some(b) = self.building.as_mut() {
            b
        } else {
            self.lines.get_or_insert_with(BezPath::new)
        };
        if target.elements().is_empty() {
            target.move_to(start);
        }
        if bez {
            for c in pts.chunks_exact(3) {
                target.curve_to(xf * c[0], xf * c[1], xf * c[2]);
            }
        } else {
            for p in pts {
                target.line_to(xf * *p);
            }
        }
        let n = if bez { pts.len() / 3 * 3 } else { pts.len() };
        if n > 0 {
            self.dc.pos = pts[n - 1];
        }
    }

    fn flush_lines(&mut self) {
        if let Some(l) = self.lines.take() {
            if l.elements().len() > 1 {
                self.emit_path(l, false, true);
            }
        }
    }

    /// A closed or open figure outside a path bracket is drawn at once; inside, it is added to
    /// the path.
    fn shape(&mut self, p: BezPath, closed: bool) {
        if let Some(b) = self.building.as_mut() {
            b.extend(p.elements().iter().copied());
            return;
        }
        self.emit_path(p, closed, true);
    }

    fn outline(&self) -> Option<Outline> {
        let pen = self.dc.pen.as_ref()?;
        let width = pen.width * self.scale();
        let w_units = pen.width.max(1.0);
        let dash = match pen.style & 0xf {
            1 => vec![3.0, 1.0],
            2 => vec![1.0, 1.0],
            3 => vec![3.0, 1.0, 1.0, 1.0],
            4 => vec![3.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            7 => pen.dashes.iter().map(|d| d / w_units).collect(),
            _ => vec![],
        };
        let geometric = pen.width > 0.0;
        Some(Outline {
            width,
            color: Some(pen.color),
            cap: match (geometric, pen.style & 0xf00) {
                (true, 0x100) => LineCap::Square,
                (true, 0x200) => LineCap::Butt,
                _ => LineCap::Round,
            },
            join: match (geometric, pen.style & 0xf000) {
                (true, 0x1000) => LineJoin::Bevel,
                (true, 0x2000) => LineJoin::Miter,
                _ => LineJoin::Round,
            },
            dash,
            miter_limit: self.dc.miter,
            ..Outline::default()
        })
    }

    fn emit_path(&mut self, p: BezPath, fill: bool, stroke: bool) {
        if p.elements().is_empty() {
            return;
        }
        let f = if fill { self.dc.brush } else { None };
        let o = if stroke { self.outline() } else { None };
        if f.is_none() && o.is_none() {
            return;
        }
        // an outline of the shape just filled goes on that shape (fill then stroke)
        if let (None, Some(ol)) = (f, &o) {
            let list = match self.clipped.as_mut() {
                Some((_, v)) => v,
                None => &mut self.out,
            };
            if let Some(last) = list.last_mut() {
                if let ObjKind::Path(np) = &last.kind {
                    if !last.style.outline.is_visible() && !last.style.fill.is_none() && np.to_bez() == p {
                        last.style.outline = ol.clone();
                        return;
                    }
                }
            }
        }
        self.emit(p, f, o);
    }

    fn emit(&mut self, p: BezPath, fill: Option<Color>, outline: Option<Outline>) {
        let mut st = Style::default();
        st.fill = fill.map(Fill::Solid).unwrap_or(Fill::None);
        st.outline = outline.unwrap_or_else(Outline::none);
        st.even_odd = self.dc.even_odd;
        let id = self.doc.new_id();
        let o = Object::new(id, ObjKind::Path(NodePath::from_bez(&p)), Affine::IDENTITY, st);
        self.push(o);
    }

    fn push(&mut self, o: Object) {
        match self.clipped.as_mut() {
            Some((_, v)) => v.push(o),
            None => self.out.push(o),
        }
    }

    /// The clipping path changes: what was drawn inside the old one becomes a PowerClip.
    fn set_clip(&mut self, clip: Option<BezPath>) {
        let same = match (&self.clipped, &clip) {
            (Some((a, _)), Some(b)) => a == b,
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.close_clip();
        if let Some(c) = clip {
            self.clipped = Some((c, vec![]));
        }
    }

    fn close_clip(&mut self) {
        let Some((frame, contents)) = self.clipped.take() else { return };
        if contents.is_empty() {
            return;
        }
        // a rectangular clip around everything it holds clips nothing (a bitmap's own frame)
        let fb = frame.bounding_box();
        let inside = vector_core::object::union_bounds(contents.iter().map(|o| o.bounds()));
        let rect_like = (frame.area().abs() - fb.area()).abs() <= fb.area() * 0.005;
        if rect_like && fb.inflate(0.05, 0.05).contains(inside.origin()) && fb.inflate(0.05, 0.05).contains(Point::new(inside.x1, inside.y1)) {
            self.out.extend(contents);
            return;
        }
        let mut st = Style::default();
        st.fill = Fill::None;
        st.outline = Outline::none();
        let fid = self.doc.new_id();
        let frame = Object::new(fid, ObjKind::Path(NodePath::from_bez(&frame)), Affine::IDENTITY, st.clone());
        let id = self.doc.new_id();
        self.out.push(Object::new(id, ObjKind::PowerClip { frame: Box::new(frame), contents }, Affine::IDENTITY, st));
    }

    /// A bitmap record: `bmi` = (offBmi, cbBmi, offBits, cbBits) in the record; `src` the source
    /// rectangle in pixels; `dest` the destination (x, y, width, height) in logical units.
    fn bitmap(&mut self, rec: &[u8], bmi: (usize, usize, usize, usize), src: (i32, i32, i32, i32), dest: (f64, f64, f64, f64), premultiplied: bool, xf: Affine) {
        let (ob, cb, obits, cbits) = bmi;
        let (Some(header), Some(bits)) = (rec.get(ob..ob + cb), rec.get(obits..obits + cbits)) else { return };
        let Some((w, h, mut rgba)) = decode_dib(header, bits, premultiplied) else { return };
        // crop to the source rectangle
        let (sx, sy) = (src.0.clamp(0, w as i32) as u32, src.1.clamp(0, h as i32) as u32);
        let sw = (src.2.max(1) as u32).min(w - sx);
        let sh = (src.3.max(1) as u32).min(h - sy);
        if sw == 0 || sh == 0 {
            return;
        }
        if (sx, sy, sw, sh) != (0, 0, w, h) {
            let mut c = Vec::with_capacity((sw * sh * 4) as usize);
            for y in sy..sy + sh {
                let row = ((y * w + sx) * 4) as usize;
                c.extend_from_slice(&rgba[row..row + (sw * 4) as usize]);
            }
            rgba = c;
        }
        let p0 = xf * Point::new(dest.0, dest.1);
        let px = xf * Point::new(dest.0 + dest.2, dest.1);
        let py = xf * Point::new(dest.0, dest.1 + dest.3);
        let (vx, vy) = (px - p0, py - p0);
        let (iw, ih) = (vx.hypot(), vy.hypot());
        if iw < 1e-9 || ih < 1e-9 {
            return;
        }
        let png = crate::image_io::encode_png(sw, sh, &rgba);
        let data = ImageData { width: sw, height: sh, rgba: Arc::new(rgba), encoded: Arc::new(png), format: "png".into(), name: "Metafile bitmap".into() };
        let iid = self.doc.add_image(data);
        let mut st = Style::default();
        st.outline = Outline::none();
        let id = self.doc.new_id();
        let t = Affine::new([vx.x / iw, vx.y / iw, vy.x / ih, vy.y / ih, p0.x, p0.y]);
        let o = Object::new(id, ObjKind::Image(ImageObj { image: iid, w: iw, h: ih, crop: [0.0, 0.0, 1.0, 1.0], adjust: ImageAdjust::default() }), t, st);
        self.push(o);
    }

    fn text(&mut self, rec: &[u8], xf: Affine) {
        let g = |o| rd_u32(rec, o).unwrap_or(0);
        let (rx, ry) = (rd_i32(rec, 36).unwrap_or(0) as f64, rd_i32(rec, 40).unwrap_or(0) as f64);
        let (n, off, options) = (g(44) as usize, g(48) as usize, g(52));
        if options & 0x10 != 0 || n == 0 {
            // glyph indices, not characters
            return;
        }
        let Some(raw) = rec.get(off..off + n * 2) else { return };
        let chars: Vec<u16> = raw.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        let s = String::from_utf16_lossy(&chars);
        if s.trim().is_empty() {
            return;
        }
        let font = self.dc.font.clone().unwrap_or(Font { height: -12.0, escapement: 0.0, weight: 400, italic: false, underline: false, strike: false, face: String::new() });
        let align = self.dc.text_align;
        let reference = if align & 1 != 0 { self.dc.pos } else { Point::new(rx, ry) };
        // the em size in logical units (a positive height is the cell height)
        let em = if font.height < 0.0 { -font.height } else { font.height * 0.87 };
        let em = if em <= 0.0 { 12.0 } else { em };
        // baseline position: GDI's default alignment is the top of the character cell
        let down = match align & 0x18 {
            24 => 0.0,
            8 => -em * 0.21,
            _ => em * 0.9,
        };
        let ang = font.escapement.to_radians();
        let local_down = kurbo::Vec2::new(ang.sin() * down, ang.cos() * down);
        let base = reference + local_down;
        let origin = xf * base;
        let ux = xf * (base + kurbo::Vec2::new(ang.cos(), -ang.sin())) - origin;
        let size = em * self.scale();
        let fmt = text::CharFormat {
            font: if font.face.is_empty() { text::CharFormat::default().font } else { font.face.clone() },
            size: size.max(0.5),
            bold: font.weight >= 600,
            italic: font.italic,
            underline: font.underline,
            strike: font.strike,
            ..Default::default()
        };
        let mut t = text::TextObj::artistic(&s, fmt);
        t.align = match align & 6 {
            6 => text::TextAlign::Center,
            2 => text::TextAlign::Right,
            _ => text::TextAlign::Left,
        };
        let mut st = Style::default();
        st.fill = Fill::Solid(self.dc.text_color);
        st.outline = Outline::none();
        let id = self.doc.new_id();
        let o = Object::new(id, ObjKind::Text(t), Affine::translate(origin.to_vec2()) * Affine::rotate(ux.y.atan2(ux.x)), st);
        self.push(o);
    }
}

/// A figure from GDI points (logical) in document space.
fn figure(pts: &[Point], closed: bool, bez: bool, xf: Affine) -> BezPath {
    let mut b = BezPath::new();
    let Some(first) = pts.first() else { return b };
    b.move_to(xf * *first);
    if bez {
        for c in pts[1..].chunks_exact(3) {
            b.curve_to(xf * c[0], xf * c[1], xf * c[2]);
        }
    } else {
        for p in &pts[1..] {
            b.line_to(xf * *p);
        }
    }
    if closed {
        b.close_path();
    }
    b
}

/// Decode a DIB (header + bits) to straight RGBA, top row first.
fn decode_dib(header: &[u8], bits: &[u8], premultiplied: bool) -> Option<(u32, u32, Vec<u8>)> {
    let hsize = rd_u32(header, 0)? as usize;
    if hsize < 40 {
        return None;
    }
    let w = rd_i32(header, 4)?;
    let h = rd_i32(header, 8)?;
    let bpp = header.get(14..16).map(|b| u16::from_le_bytes([b[0], b[1]]))?;
    let comp = rd_u32(header, 16)?;
    if w <= 0 || h == 0 || w > 30000 || h.abs() > 30000 {
        return None;
    }
    let (uw, uh) = (w as u32, h.unsigned_abs());
    if bpp == 32 && (comp == 0 || comp == 3) {
        // BGRA rows; the fourth byte is alpha when it is not all zero
        let row = (uw * 4) as usize;
        if bits.len() < row * uh as usize {
            return None;
        }
        let has_alpha = bits.chunks_exact(4).any(|p| p[3] != 0);
        let mut out = Vec::with_capacity(row * uh as usize);
        for y in 0..uh as usize {
            let sy = if h > 0 { uh as usize - 1 - y } else { y };
            for p in bits[sy * row..(sy + 1) * row].chunks_exact(4) {
                let a = if has_alpha { p[3] } else { 255 };
                let un = |c: u8| if premultiplied && a > 0 && a < 255 { ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 } else { c };
                out.extend_from_slice(&[un(p[2]), un(p[1]), un(p[0]), a]);
            }
        }
        return Some((uw, uh, out));
    }
    // other depths: through the BMP decoder
    let mut file = Vec::with_capacity(14 + header.len() + bits.len());
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&((14 + header.len() + bits.len()) as u32).to_le_bytes());
    file.extend_from_slice(&[0; 4]);
    file.extend_from_slice(&((14 + header.len()) as u32).to_le_bytes());
    file.extend_from_slice(header);
    file.extend_from_slice(bits);
    let img = image::load_from_memory_with_format(&file, image::ImageFormat::Bmp).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some((w, h, img.into_raw()))
}
