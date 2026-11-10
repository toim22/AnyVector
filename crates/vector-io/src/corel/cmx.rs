//! Corel Presentation Exchange (CMX): Corel's binary metafile.
//!
//! A CMX file is a RIFF (little-endian) or RIFX (big-endian) stream whose form type starts with
//! `CMX` (`CMX1`); 1990s clipart CDs also ship CMX files named `.cdr`, with the form type `CDRX`.
//! CorelDRAW 5 to X2 store the drawing of a vector pattern fill as an embedded CMX document.
//!
//! Unlike a CorelDRAW drawing, the file is not a tree of objects but a set of tables reached
//! through an index (`ixmr`): colors (`rclr`), pens (`rpen`), line styles (`rott`), dashes
//! (`rdot`), outlines (`rotl`, each naming a line style, color, pen and dash), fonts, embedded
//! bitmaps and pages (`ixpg`). A page is a stream of size-prefixed instructions (begin/end
//! page, layer and group, curves, rectangles, ellipses, bitmaps, text), each drawing
//! instruction naming its fill and outline. Coordinates are 16-bit (thousandths of an inch) or
//! 32-bit (a 254000th of an inch) with y up; the 32-bit variant wraps every record in tagged
//! fields. A `pack` chunk, when present, holds the deflated rest of the file and every offset
//! counts in the inflated file.
//!
//! The layout was read from real files (the CorelDRAW 7 clipart CD), with libcdr's CMX parser
//! (MPL-2.0) as the map of the record numbers. What is not converted (procedures, bitmap,
//! texture and PostScript fills, arrowheads) is skipped or approximated, never fatal.

use super::build::{ImportNote, Report};
use super::color;
use crate::{IoError, Result};
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::sync::Arc;
use vector_core::color::Color;
use vector_core::document::{Document, ImageData, Layer, Page};
use vector_core::object::{EllipseKind, EllipseShape, ImageAdjust, ImageObj, ObjKind, Object, RectShape};
use vector_core::path::NodePath;
use vector_core::style::{Fill, Gradient, GradientKind, GradientStop, LineCap, LineJoin, Outline, PatternFill, PatternKind, PatternSource, SpreadMode, Style, Transparency};
use vector_core::text::{CharFormat, TextObj};

/// Whether the bytes are a CMX metafile (`RIFF`/`RIFX` with a `CMX?` or `CDRX` form type).
pub fn is_cmx(b: &[u8]) -> bool {
    b.len() >= 12 && (&b[0..4] == b"RIFF" || &b[0..4] == b"RIFX") && (b[8..11].eq_ignore_ascii_case(b"CMX") || &b[8..12] == b"CDRX")
}

/// The drawing of one CMX page, in points: the page's top-left corner is the origin, y grows
/// downwards. Its bitmaps are in the document the drawing was read into.
#[derive(Clone, Debug)]
pub struct Drawing {
    pub width: f64,
    pub height: f64,
    pub layers: Vec<Layer>,
}

impl Drawing {
    /// All top-level objects, layer after layer.
    pub fn objects(self) -> Vec<Object> {
        self.layers.into_iter().flat_map(|l| l.objects).collect()
    }
}

/// Read the first page of a CMX document into `doc` (its ids and bitmaps), without making it a
/// page of `doc`: for a vector pattern, whose drawing is a CMX document.
pub fn first_page(bytes: &[u8], doc: &mut Document) -> Result<Drawing> {
    let mut report = Report::default();
    let mut pages = read(bytes, doc, &mut report, true)?;
    Ok(pages.remove(0))
}

/// Open a CMX file as a document.
pub fn load_from_bytes(bytes: &[u8]) -> Result<Document> {
    load_with_report(bytes).map(|r| r.0)
}

/// Open a CMX file and say what was left out or approximated.
pub fn load_with_report(bytes: &[u8]) -> Result<(Document, Report)> {
    let mut doc = Document::new(612.0, 792.0);
    let mut report = Report::default();
    let pages = read(bytes, &mut doc, &mut report, false)?;
    doc.pages = pages
        .into_iter()
        .map(|d| {
            let id = doc.new_id();
            let mut p = Page::new(id, 0, d.width, d.height);
            p.layers = d.layers;
            p
        })
        .collect();
    doc.ensure_next_id();
    Ok((doc, report))
}

/// The preview a CMX file carries (a Windows bitmap in its `DISP` chunk), as RGBA pixels.
pub fn thumbnail(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if !is_cmx(bytes) {
        return None;
    }
    let be = &bytes[0..4] == b"RIFX";
    let (at, size) = chunks(bytes, 12, bytes.len(), be).into_iter().find(|c| &c.0 == b"DISP").map(|c| (c.1, c.2))?;
    // a 32-bit word, then the bitmap's header, palette and rows
    let dib = bytes.get(at + 4..at + size)?;
    let mut r = Rd::new(dib, false);
    let header = r.u32() as usize;
    r.p = 14;
    let bpp = r.u16() as usize;
    r.p = 32;
    let used = r.u32() as usize;
    let colors = if used > 0 { used.min(256) } else if bpp <= 8 { 1 << bpp } else { 0 };
    let offset = 14 + header + colors * 4;
    let mut file = b"BM".to_vec();
    file.extend(((14 + dib.len()) as u32).to_le_bytes());
    file.extend([0, 0, 0, 0]);
    file.extend((offset as u32).to_le_bytes());
    file.extend_from_slice(dib);
    let img = image::load_from_memory_with_format(&file, image::ImageFormat::Bmp).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((w, h, img.into_raw()))
}

// ------------------------------------------------------------------------------------------
// Reading
// ------------------------------------------------------------------------------------------

/// A reader of either byte order that never fails: reads past the end give zero.
#[derive(Clone)]
struct Rd<'a> {
    b: &'a [u8],
    p: usize,
    be: bool,
}

impl<'a> Rd<'a> {
    fn new(b: &'a [u8], be: bool) -> Self {
        Rd { b, p: 0, be }
    }
    fn at(b: &'a [u8], p: usize, be: bool) -> Self {
        Rd { b, p, be }
    }
    fn skip(&mut self, n: usize) {
        self.p = self.p.saturating_add(n);
    }
    fn bytes(&mut self, n: usize) -> &'a [u8] {
        let s = self.p.min(self.b.len());
        let e = self.p.saturating_add(n).min(self.b.len());
        self.p = self.p.saturating_add(n);
        &self.b[s..e]
    }
    fn arr<const N: usize>(&mut self) -> [u8; N] {
        let mut a = [0u8; N];
        let s = self.bytes(N);
        if s.len() == N {
            a.copy_from_slice(s);
            if self.be {
                a.reverse();
            }
        }
        a
    }
    fn u8(&mut self) -> u8 {
        self.arr::<1>()[0]
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.arr())
    }
    fn i16(&mut self) -> i16 {
        i16::from_le_bytes(self.arr())
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.arr())
    }
    fn i32(&mut self) -> i32 {
        i32::from_le_bytes(self.arr())
    }
    fn f64(&mut self) -> f64 {
        let v = f64::from_le_bytes(self.arr());
        if v.is_finite() {
            v
        } else {
            0.0
        }
    }
    /// A string: a 16-bit length, then the characters (Windows code page 1252).
    fn string(&mut self) -> String {
        let n = self.u16() as usize;
        let s = self.bytes(n);
        s.iter().take_while(|c| **c != 0).map(|c| cp1252(*c as u16)).collect::<String>().trim().to_string()
    }
}

/// The chunks in `b[start..end]`: tag, body offset, body size.
fn chunks(b: &[u8], start: usize, end: usize, be: bool) -> Vec<([u8; 4], usize, usize)> {
    let end = end.min(b.len());
    let mut out = vec![];
    let mut o = start;
    while o + 8 <= end {
        let tag = [b[o], b[o + 1], b[o + 2], b[o + 3]];
        let size = Rd::at(b, o + 4, be).u32() as usize;
        let body = o + 8;
        let size = size.min(end - body);
        out.push((tag, body, size));
        o = body + size + (size & 1);
    }
    out
}

/// The file with its `pack` chunk inflated in place: the offsets in a packed file count in the
/// inflated one.
fn unpack(bytes: &[u8], be: bool) -> std::borrow::Cow<'_, [u8]> {
    for (tag, body, size) in chunks(bytes, 12, bytes.len(), be) {
        if &tag != b"pack" {
            continue;
        }
        // the inflated size, "CPng", two words, then a zlib stream
        let mut r = Rd::at(bytes, body, be);
        let hint = r.u32() as usize;
        if bytes.get(body + 4..body + 8) != Some(b"CPng") {
            break;
        }
        let mut out = Vec::with_capacity(hint.min(64 << 20));
        let z = flate2::read::ZlibDecoder::new(&bytes[(body + 12).min(body + size)..body + size]);
        if z.take(512 << 20).read_to_end(&mut out).is_err() && out.is_empty() {
            break;
        }
        let mut v = bytes[..body - 8].to_vec();
        v.extend_from_slice(&out);
        v.extend_from_slice(bytes.get(body + size + (size & 1)..).unwrap_or(&[]));
        return std::borrow::Cow::Owned(v);
    }
    std::borrow::Cow::Borrowed(bytes)
}

/// Windows code page 1252 to Unicode (the 0x80..0x9f block differs from Latin-1).
fn cp1252(c: u16) -> char {
    const HIGH: [u16; 32] = [
        0x20ac, 0x81, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8d, 0x017d, 0x8f, 0x90, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x02dc, 0x2122, 0x0161, 0x203a, 0x0153, 0x9d, 0x017e, 0x0178,
    ];
    let c = if (0x80..0xa0).contains(&c) { HIGH[(c - 0x80) as usize] } else { c };
    char::from_u32(c as u32).unwrap_or('\u{fffd}')
}

// ------------------------------------------------------------------------------------------
// The tables
// ------------------------------------------------------------------------------------------

const SEC_PAGES: u16 = 2;
const SEC_PROCEDURES: u16 = 4;
const SEC_FONTS: u16 = 7;
const SEC_EMBEDDED: u16 = 8;
const SEC_OUTLINES: u16 = 15;
const SEC_LINE_STYLES: u16 = 16;
const SEC_PENS: u16 = 19;
const SEC_DASHES: u16 = 20;
const SEC_COLORS: u16 = 21;

const BEGIN_PAGE: i32 = 9;
const END_PAGE: i32 = 10;
const BEGIN_LAYER: i32 = 11;
const END_LAYER: i32 = 12;
const BEGIN_GROUP: i32 = 13;
const END_GROUP: i32 = 14;
const END_PROCEDURE: i32 = 18;
const DRAW_CHARS: i32 = 65;
const ELLIPSE: i32 = 66;
const POLYCURVE: i32 = 67;
const RECTANGLE: i32 = 68;
const DRAW_IMAGE: i32 = 69;
const BEGIN_TEXT_GROUP: i32 = 72;
const END_TEXT_GROUP: i32 = 73;
const SET_CHAR_STYLE: i32 = 85;
const SIMPLE_WIDE_TEXT: i32 = 86;
const JUMP_ABSOLUTE: i32 = 111;

const TAG_END: u8 = 255;
const GRAY: Color = Color::rgb(200, 200, 200);

#[derive(Clone, Copy, Debug, Default)]
struct Pen {
    /// File units.
    width: f64,
    /// Percent.
    aspect: f64,
    /// Degrees.
    angle: f64,
    matrix: Affine,
}

/// A fill as stored, before it is fitted to an object.
#[derive(Clone, Debug)]
enum RawFill {
    None,
    Solid(Color),
    /// kind, angle (degrees), center offset (fractions of the box), edge pad (0..0.5), stops
    Gradient { kind: GradientKind, angle: f64, cx: f64, cy: f64, pad: f64, stops: Vec<GradientStop> },
    /// A full-color (vector) pattern: the procedure that draws it, the tile's width and height
    /// (points, or fractions of the object's size when `relative`), and the box of the
    /// procedure's drawing (file units) that the tile repeats.
    Vector { proc: u16, w: f64, h: f64, relative: bool, frame: Rect },
}

/// The rendering attributes of a drawing instruction.
#[derive(Clone, Debug, Default)]
struct Attrs {
    fill: Option<RawFill>,
    outline: Option<Outline>,
    /// 0 opaque .. 100 transparent.
    transparency: Option<f64>,
}

/// What the importer left out, counted.
#[derive(Default)]
struct Tally {
    /// Instructions not converted, by number.
    skipped: BTreeMap<u32, usize>,
}

struct Cmx<'a> {
    b: &'a [u8],
    be: bool,
    /// 32-bit coordinates and tagged records.
    wide: bool,
    /// Points per file unit.
    k: f64,
    /// The drawing's box from the header (file units), when it has one.
    bbox: Option<Rect>,
    sections: HashMap<u16, usize>,
    colors: Vec<Color>,
    dashes: Vec<Vec<f64>>,
    line_styles: Vec<(u8, u8)>,
    pens: Vec<Pen>,
    outlines: Vec<[u16; 6]>,
    fonts: Vec<String>,
    /// Embedded bitmap number to the document's image and its pixel size.
    images: HashMap<u16, (u64, u32, u32)>,
    /// Procedures (drawings that full-color pattern fills repeat): number to their `proc` chunk.
    procs: HashMap<u16, usize>,
    /// The tiles made of procedures, by procedure and tile size: the document's image and tile.
    pattern_tiles: HashMap<(u16, u64, u64), Option<(u64, u64)>>,
    /// The procedures being drawn, innermost last.
    drawing: Vec<u16>,
    tally: Tally,
}

fn read(bytes: &[u8], doc: &mut Document, report: &mut Report, first_only: bool) -> Result<Vec<Drawing>> {
    let fail = |why: &str| IoError::Format(format!("this is a Corel metafile (CMX), and it could not be read: {why}"));
    if !is_cmx(bytes) {
        return Err(fail("it is not a CMX file"));
    }
    let be = &bytes[0..4] == b"RIFX";
    let b = unpack(bytes, be);
    let mut cmx = Cmx::header(&b, be).ok_or_else(|| fail("its header is missing or damaged"))?;
    cmx.tables(doc);
    let mut pages = vec![];
    for at in cmx.page_offsets() {
        if let Some(p) = cmx.page(at, doc) {
            pages.push(p);
            if first_only {
                break;
            }
        }
    }
    if pages.is_empty() {
        return Err(fail("it has no pages"));
    }
    for (code, n) in &cmx.tally.skipped {
        report.skipped.push((*code, *n));
        report.notes.push((ImportNote::SkippedObjects { kind: *code }, *n));
    }
    Ok(pages)
}

impl<'a> Cmx<'a> {
    /// The `cont` header: byte order, coordinate size, units and the index's offset.
    fn header(b: &'a [u8], be: bool) -> Option<Cmx<'a>> {
        let (_, body, size) = chunks(b, 12, b.len(), be).into_iter().find(|c| &c.0 == b"cont")?;
        if size < 70 {
            return None;
        }
        let text = |s: &[u8]| String::from_utf8_lossy(s).trim_end_matches('\0').trim().to_string();
        let order = text(&b[body + 48..body + 52]);
        let be = match order.as_str() {
            "4" => true,
            "2" => false,
            _ => be,
        };
        let wide = text(&b[body + 52..body + 54]) == "4";
        let mut r = Rd::at(b, body + 62, be);
        let unit = r.u16();
        let scale = r.f64();
        r.skip(12);
        let index = r.u32() as usize;
        let _info = r.u32();
        let _thumbnail = r.u32();
        // the box is 32-bit in either variant
        let (x0, y0, x1, y1) = (r.i32() as f64, r.i32() as f64, r.i32() as f64, r.i32() as f64);
        let bbox = Rect::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
        // the scale is in inches (unit 64, the clipart CDs: 0.001) or in metres (unit 35:
        // CorelDRAW 2026 writes 1e-7, a 254000th of an inch)
        let per = match unit {
            35 => 72.0 / 0.0254,
            _ => 72.0,
        };
        let fallback = if wide { 72.0 / 254_000.0 } else { 72.0 / 1000.0 };
        let k = if scale > 1e-9 && scale < 1.0 && matches!(unit, 35 | 64) { scale * per } else { fallback };
        let mut cmx = Cmx {
            b,
            be,
            wide,
            k,
            bbox: (bbox.width() > 0.0 && bbox.height() > 0.0).then_some(bbox),
            sections: HashMap::new(),
            colors: vec![],
            dashes: vec![],
            line_styles: vec![],
            pens: vec![],
            outlines: vec![],
            fonts: vec![],
            images: HashMap::new(),
            procs: HashMap::new(),
            pattern_tiles: HashMap::new(),
            drawing: vec![],
            tally: Tally::default(),
        };
        // the master index: section number to offset
        if b.get(index..index + 4) == Some(b"ixmr") {
            let mut r = Rd::at(b, index + 8, be);
            let _master = r.u16();
            let _size = r.u16();
            let n = r.u16() as usize;
            for _ in 0..n.min(b.len() / 6) {
                let id = r.u16();
                let at = r.u32() as usize;
                if at < b.len() {
                    cmx.sections.insert(id, at);
                }
            }
        }
        Some(cmx)
    }

    fn rd(&self, at: usize) -> Rd<'a> {
        Rd::at(self.b, at, self.be)
    }

    fn coord(&self, r: &mut Rd) -> f64 {
        if self.wide {
            r.i32() as f64
        } else {
            r.i16() as f64
        }
    }

    /// An angle in degrees.
    fn angle(&self, r: &mut Rd) -> f64 {
        if self.wide {
            r.i32() as f64 / 1_000_000.0
        } else {
            r.i16() as f64 / 10.0
        }
    }

    /// A matrix: its type (0 and 1 are the identity), then six numbers, the translation in file
    /// units.
    fn matrix(&self, r: &mut Rd) -> Affine {
        let kind = r.u16();
        if kind <= 1 {
            return Affine::IDENTITY;
        }
        let v = [r.f64(), r.f64(), r.f64(), r.f64(), r.f64(), r.f64()];
        let m = Affine::new(v);
        if m.determinant().abs() < 1e-12 || v.iter().any(|x| x.abs() > 1e12) {
            Affine::IDENTITY
        } else {
            m
        }
    }

    fn bbox(&self, r: &mut Rd) -> Rect {
        let (x0, y0, x1, y1) = (self.coord(r), self.coord(r), self.coord(r), self.coord(r));
        Rect::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
    }

    /// The tagged fields of a 32-bit record, from `r` to `end` (or the end tag): id, body.
    fn tags(&self, r: &mut Rd, end: usize) -> Vec<(u8, usize, usize)> {
        let end = end.min(self.b.len());
        let mut out = vec![];
        while r.p < end {
            let start = r.p;
            let id = r.u8();
            if id == TAG_END {
                break;
            }
            let len = (r.u16() as usize).max(3);
            let stop = (start + len).min(end);
            out.push((id, start + 3, stop));
            r.p = start + len;
        }
        out
    }

    /// Where a table starts: from the index, else the first chunk with its tag.
    fn table(&self, section: u16, tag: &[u8; 4]) -> Option<(usize, usize)> {
        let ok = |at: usize| self.b.get(at..at + 4) == Some(tag);
        let at = match self.sections.get(&section) {
            Some(&at) if ok(at) => at,
            _ => {
                let all = chunks(self.b, 12, self.b.len(), self.be);
                let mut found = all.iter().find(|c| &c.0 == tag).map(|c| c.1 - 8);
                if found.is_none() {
                    for (t, body, size) in &all {
                        if t == b"LIST" {
                            found = chunks(self.b, body + 4, body + size, self.be).iter().find(|c| &c.0 == tag).map(|c| c.1 - 8);
                            if found.is_some() {
                                break;
                            }
                        }
                    }
                }
                found?
            }
        };
        let size = self.rd(at + 4).u32() as usize;
        Some((at + 8, (at + 8).saturating_add(size).min(self.b.len())))
    }

    /// Read the description tables and the embedded bitmaps.
    fn tables(&mut self, doc: &mut Document) {
        if let Some((at, end)) = self.table(SEC_COLORS, b"rclr") {
            self.colors = self.read_colors(at, end);
        }
        if let Some((at, end)) = self.table(SEC_DASHES, b"rdot") {
            self.dashes = self.records(at, end, |_, r, _| {
                let n = r.u16() as usize;
                (0..n.min(32)).map(|_| r.u16() as f64).collect::<Vec<f64>>()
            });
        }
        if let Some((at, end)) = self.table(SEC_PENS, b"rpen") {
            self.pens = self.records(at, end, |s, r, _| {
                let width = s.coord(r).abs();
                let aspect = r.u16() as f64;
                let angle = s.angle(r);
                if !s.wide {
                    r.skip(2);
                }
                let matrix = s.matrix(r);
                Pen { width, aspect, angle, matrix }
            });
        }
        if let Some((at, end)) = self.table(SEC_LINE_STYLES, b"rott") {
            self.line_styles = self.records(at, end, |_, r, _| (r.u8(), r.u8()));
        }
        if let Some((at, end)) = self.table(SEC_OUTLINES, b"rotl") {
            self.outlines = self.records(at, end, |_, r, _| [r.u16(), r.u16(), r.u16(), r.u16(), r.u16(), r.u16()]);
        }
        self.fonts = self.read_fonts();
        self.read_images(doc);
        self.procs = self.read_procedures();
    }

    /// The procedure index (`ixpc`): per procedure (numbered from 1; 32-bit records start with
    /// their size) the offset of its reference list, then of its `proc` chunk.
    fn read_procedures(&self) -> HashMap<u16, usize> {
        let mut out = HashMap::new();
        let Some(&at) = self.sections.get(&SEC_PROCEDURES) else { return out };
        if self.b.get(at..at + 4) != Some(b"ixpc") {
            return out;
        }
        let mut r = self.rd(at + 8);
        let n = r.u16() as usize;
        for j in 1..=n.min(self.b.len() / 8).min(u16::MAX as usize) {
            let start = r.p;
            let size = if self.wide { r.u16() as usize } else { 8 };
            if size < 8 {
                break;
            }
            let _refs = r.u32();
            let proc = r.u32() as usize;
            r.p = start + if self.wide { size + 2 } else { 8 };
            if self.b.get(proc..proc + 4) == Some(b"proc") {
                out.insert(j as u16, proc);
            }
        }
        out
    }

    /// A table of `count` records; in the 32-bit variant each sits in its first tagged field.
    fn records<T: Default>(&self, at: usize, end: usize, read: impl Fn(&Self, &mut Rd<'a>, usize) -> T) -> Vec<T> {
        let mut r = self.rd(at);
        let n = r.u16() as usize;
        let mut out = vec![];
        for _ in 0..n {
            if r.p >= end {
                break;
            }
            if self.wide {
                let tags = self.tags(&mut r, end);
                let v = tags.iter().find(|t| t.0 == 1).map(|t| read(self, &mut self.rd(t.1), t.2)).unwrap_or_default();
                out.push(v);
            } else {
                out.push(read(self, &mut r, end));
            }
        }
        out
    }

    fn read_colors(&self, at: usize, end: usize) -> Vec<Color> {
        let mut r = self.rd(at);
        let n = r.u16() as usize;
        let mut out = vec![];
        for _ in 0..n {
            if r.p >= end {
                break;
            }
            if self.wide {
                let mut model = 0;
                let mut c = Color::BLACK;
                for (id, body, _) in self.tags(&mut r, end) {
                    match id {
                        1 => model = self.rd(body).u8(),
                        2 => c = self.color_value(&mut self.rd(body), model).unwrap_or(Color::BLACK),
                        _ => {}
                    }
                }
                out.push(c);
            } else {
                let model = r.u8();
                let _palette = r.u8();
                match self.color_value(&mut r, model) {
                    Some(c) => out.push(c),
                    // the size of an unknown model's value is not known: the rest is lost
                    None => break,
                }
            }
        }
        out
    }

    /// A color value of a color model.
    fn color_value(&self, r: &mut Rd, model: u8) -> Option<Color> {
        let pack = |v: [u8; 4]| u32::from_le_bytes(v);
        Some(match model {
            // Pantone: the color's number and its tint; without the tables, a gray of the tint
            1 => {
                let _id = r.u16();
                let tint = (r.u16() as f64).min(100.0);
                let g = (255.0 - tint * 1.28).round().clamp(0.0, 255.0) as u8;
                Color::rgb(g, g, g)
            }
            2 | 3 => {
                let v = [r.u8(), r.u8(), r.u8(), r.u8()];
                color::convert(model as u16, pack(v))
            }
            4 => {
                let v = [r.u8(), r.u8(), r.u8(), 0];
                color::convert(4, pack(v))
            }
            5 => {
                let (red, g, b) = (r.u8(), r.u8(), r.u8());
                Color::rgb(red, g, b)
            }
            6 | 7 => {
                let h = r.u16().to_le_bytes();
                let (a, b) = (r.u8(), r.u8());
                color::convert(model as u16, pack([h[0], h[1], a, b]))
            }
            8 | 9 => color::convert(model as u16, r.u8() as u32),
            10 => {
                let v = [r.u8(), r.u8(), r.u8(), 0];
                color::convert(0x0b, pack(v))
            }
            11 | 12 => {
                let v = [r.u8(), r.u8(), r.u8(), 0];
                color::convert(0x0c, pack(v))
            }
            0xff => {
                r.skip(4);
                Color::BLACK
            }
            _ => return None,
        })
    }

    /// The font index (a table of offsets to name records).
    fn read_fonts(&self) -> Vec<String> {
        let Some(&at) = self.sections.get(&SEC_FONTS) else { return vec![] };
        self.offsets(at, b"ixtl")
            .into_iter()
            .map(|(off, _)| {
                if self.b.get(off..off + 4) == Some(b"rfnt") {
                    // the font table itself: its records follow
                    return String::new();
                }
                let mut r = self.rd(off);
                if self.wide {
                    let tags = self.tags(&mut r, self.b.len());
                    tags.iter().find(|t| t.0 == 1).map(|t| self.rd(t.1).string()).unwrap_or_default()
                } else {
                    r.string()
                }
            })
            .collect()
    }

    /// An index table: (offset, type) per record. 16-bit: the count, the type, the offsets;
    /// 32-bit: the count, the record size, the type, then the records.
    fn offsets(&self, at: usize, tag: &[u8; 4]) -> Vec<(usize, u16)> {
        if self.b.get(at..at + 4) != Some(tag) {
            return vec![];
        }
        let mut r = self.rd(at + 8);
        let n = r.u16() as usize;
        let size = if self.wide { r.u16() as usize } else { 4 };
        if size < 4 {
            return vec![];
        }
        let kind = r.u16();
        let mut out = vec![];
        for _ in 0..n.min(self.b.len() / 4) {
            let start = r.p;
            let off = r.u32() as usize;
            if off < self.b.len() {
                out.push((off, kind));
            }
            r.p = start + size;
        }
        out
    }

    /// The embedded files: bitmaps (type 0x11) as `LIST imag` with an `info` and a `data` chunk.
    fn read_images(&mut self, doc: &mut Document) {
        let Some(&at) = self.sections.get(&SEC_EMBEDDED) else { return };
        if self.b.get(at..at + 4) != Some(b"ixef") {
            return;
        }
        let mut r = self.rd(at + 8);
        let n = r.u16() as usize;
        for j in 1..=n.min(4096) {
            let start = r.p;
            let size = if self.wide { r.u16() as usize } else { 6 };
            if size < 6 {
                break;
            }
            let off = r.u32() as usize;
            let kind = r.u16();
            r.p = start + if self.wide { size + 2 } else { 6 };
            if kind != 0x11 {
                continue;
            }
            if let Some((w, h, rgba)) = self.image(off) {
                let encoded = crate::image_io::encode_png(w, h, &rgba);
                let data = ImageData { width: w, height: h, rgba: Arc::new(rgba), encoded: Arc::new(encoded), format: "png".into(), name: format!("bitmap{j}") };
                let id = doc.add_image(data);
                self.images.insert(j as u16, (id, w, h));
            }
        }
    }

    /// An embedded bitmap at `at`: `LIST imag` { `info`, `data` }.
    fn image(&self, at: usize) -> Option<(u32, u32, Vec<u8>)> {
        if self.b.get(at..at + 4) != Some(b"LIST") || self.b.get(at + 8..at + 12) != Some(b"imag") {
            return None;
        }
        let size = self.rd(at + 4).u32() as usize;
        let parts = chunks(self.b, at + 12, (at + 8).saturating_add(size), self.be);
        let (_, body, len) = parts.iter().find(|c| &c.0 == b"data")?;
        let data = &self.b[*body..body + len];
        // 32-bit: tagged, the image in tag 2 with a 32-bit length
        let data = if self.wide && data.first() != Some(&b'R') && data.first() != Some(&b'B') {
            let mut r = Rd::new(data, self.be);
            let mut found = None;
            while r.p < data.len() {
                let start = r.p;
                let id = r.u8();
                if id == TAG_END {
                    break;
                }
                let n = (r.u32() as usize).max(5);
                if id == 2 {
                    found = data.get(start + 5..(start + n).min(data.len()));
                    break;
                }
                r.p = start + n;
            }
            found?
        } else {
            data
        };
        match data.get(0..2)? {
            b"RI" => rimage(data, 14),
            b"BM" => {
                let img = image::load_from_memory_with_format(data, image::ImageFormat::Bmp).ok()?.to_rgba8();
                let (w, h) = img.dimensions();
                Some((w, h, img.into_raw()))
            }
            _ => None,
        }
    }

    /// Where the pages start: from the page index, else every `page` chunk.
    fn page_offsets(&self) -> Vec<usize> {
        let mut out = vec![];
        if let Some(&at) = self.sections.get(&SEC_PAGES) {
            if self.b.get(at..at + 4) == Some(b"ixpg") {
                let mut r = self.rd(at + 8);
                let n = r.u16() as usize;
                for _ in 0..n.min(self.b.len() / 16) {
                    let start = r.p;
                    let size = if self.wide { r.u16() as usize } else { 16 };
                    if size < 16 {
                        break;
                    }
                    let page = r.u32() as usize;
                    r.p = start + if self.wide { size + 2 } else { 16 };
                    if self.b.get(page..page + 4) == Some(b"page") && !out.contains(&page) {
                        out.push(page);
                    }
                }
            }
        }
        if out.is_empty() {
            out = chunks(self.b, 12, self.b.len(), self.be).into_iter().filter(|c| &c.0 == b"page").map(|c| c.1 - 8).collect();
        }
        out
    }
}

/// A Corel bitmap block ("RI"): the color model field at `at`, then the size, the bits per
/// pixel, the row stride, a palette for indexed images, and the rows, bottom first.
fn rimage(b: &[u8], at: usize) -> Option<(u32, u32, Vec<u8>)> {
    let mut r = Rd::at(b, at, false);
    let model = r.u32();
    r.skip(4);
    let w = r.u32() as usize;
    let h = r.u32() as usize;
    r.skip(4);
    let bpp = r.u32() as usize;
    let mut stride = r.u32() as usize;
    let _size = r.u32();
    r.skip(32);
    if w == 0 || h == 0 || w.checked_mul(h)? > 100_000_000 || !matches!(bpp, 1 | 4 | 8 | 24 | 32) {
        return None;
    }
    let min = (w * bpp + 7) / 8;
    if stride < min || stride > min + 8 {
        stride = (min + 3) & !3;
    }
    let mut palette: Vec<[u8; 3]> = vec![];
    if bpp < 24 && model != 5 && model != 6 {
        r.skip(2);
        let n = r.u16() as usize;
        for _ in 0..n.min(256) {
            let e = r.bytes(3);
            if e.len() == 3 {
                palette.push([e[2], e[1], e[0]]);
            }
        }
    }
    let data = r.p;
    if data.checked_add(stride.checked_mul(h)?)? > b.len() + stride {
        return None;
    }
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = data + (h - 1 - y) * stride;
        let get = |o: usize| b.get(row + o).copied().unwrap_or(0);
        for x in 0..w {
            let p = match bpp {
                32 => {
                    let (a, c, d, e) = (get(x * 4), get(x * 4 + 1), get(x * 4 + 2), get(x * 4 + 3));
                    if model == 3 || model == 0x11 {
                        let col = color::cmyk(a as f64 / 255.0, c as f64 / 255.0, d as f64 / 255.0, e as f64 / 255.0);
                        [col.r, col.g, col.b]
                    } else {
                        [d, c, a]
                    }
                }
                24 => [get(x * 3 + 2), get(x * 3 + 1), get(x * 3)],
                8 => {
                    let v = get(x);
                    palette.get(v as usize).copied().unwrap_or([v, v, v])
                }
                4 => {
                    let v = (get(x / 2) >> if x % 2 == 0 { 4 } else { 0 }) & 0x0f;
                    palette.get(v as usize).copied().unwrap_or([v * 17, v * 17, v * 17])
                }
                _ => {
                    let v = (get(x / 8) >> (7 - x % 8)) & 1;
                    palette.get(v as usize).copied().unwrap_or(if v == 1 { [255, 255, 255] } else { [0, 0, 0] })
                }
            };
            rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
        }
    }
    Some((w as u32, h as u32, rgba))
}

// ------------------------------------------------------------------------------------------
// Styles
// ------------------------------------------------------------------------------------------

impl<'a> Cmx<'a> {
    fn color(&self, index: u16) -> Color {
        index.checked_sub(1).and_then(|i| self.colors.get(i as usize)).copied().unwrap_or(Color::BLACK)
    }

    /// Rendering attributes: a bit mask (fill, outline, lens, canvas, container), then each
    /// part. `None` when the 16-bit layout cannot be followed past an unknown part.
    fn attrs(&mut self, r: &mut Rd<'a>, end: usize) -> Option<Attrs> {
        let mask = r.u8();
        let mut a = Attrs { fill: Some(RawFill::None), outline: Some(Outline::none()), transparency: None };
        if self.wide {
            for bit in [0x01u8, 0x02, 0x04, 0x08, 0x10] {
                if mask & bit == 0 {
                    continue;
                }
                for (id, body, stop) in self.tags(r, end) {
                    let mut t = self.rd(body);
                    match (bit, id) {
                        (0x01, 1) => a.fill = self.fill(&mut t, stop),
                        (0x02, 1) => a.outline = Some(self.outline(t.u16())),
                        (0x04, 1) => a.transparency = self.lens(&mut t)?,
                        _ => {}
                    }
                }
            }
        } else {
            if mask & 0x01 != 0 {
                a.fill = Some(self.fill(r, end)?);
            }
            if mask & 0x02 != 0 {
                a.outline = Some(self.outline(r.u16()));
            }
            if mask & 0x04 != 0 {
                a.transparency = self.lens(r)?;
            }
            // canvas and container records: their layout is not known
            if mask & 0x18 != 0 {
                return None;
            }
        }
        Some(a)
    }

    /// A fill: its type, then its record.
    fn fill(&mut self, r: &mut Rd<'a>, end: usize) -> Option<RawFill> {
        let kind = r.u16();
        let tagged = |s: &Self, r: &mut Rd<'a>| s.tags(r, end);
        match kind {
            0 => Some(RawFill::None),
            1 => {
                let index = if self.wide { tagged(self, r).iter().find(|t| t.0 == 1).map(|t| self.rd(t.1).u16()).unwrap_or(0) } else { r.u16() };
                if !self.wide {
                    r.skip(2);
                }
                Some(RawFill::Solid(self.color(index)))
            }
            2 => {
                let (mut gkind, mut pad, mut angle, mut cx, mut cy) = (0u16, 0.0, 0.0, 0.0, 0.0);
                let mut stops = vec![];
                let read_stops = |s: &Self, r: &mut Rd, stops: &mut Vec<GradientStop>| {
                    let n = r.u16() as usize;
                    for _ in 0..n.min(256) {
                        let c = s.color(r.u16());
                        let at = r.u16() as f64 / 100.0;
                        stops.push(GradientStop { pos: at.clamp(0.0, 1.0), color: c });
                    }
                };
                if self.wide {
                    for (id, body, _) in tagged(self, r) {
                        let mut t = self.rd(body);
                        match id {
                            1 => {
                                gkind = t.u16();
                                let _screen = t.u16();
                                pad = t.u16() as f64;
                                angle = self.angle(&mut t);
                                cx = t.i32() as f64;
                                cy = t.i32() as f64;
                            }
                            2 => read_stops(self, &mut t, &mut stops),
                            _ => {}
                        }
                    }
                } else {
                    gkind = r.u16();
                    let _screen = r.u16();
                    pad = r.u16() as f64;
                    angle = self.angle(r);
                    r.skip(2);
                    cx = r.i16() as f64;
                    cy = r.i16() as f64;
                    let _steps = r.u16();
                    let _mode = r.u16();
                    read_stops(self, r, &mut stops);
                }
                let kind = match gkind & 0xff {
                    1 => GradientKind::Elliptical,
                    2 => GradientKind::Conical,
                    3 => GradientKind::Rectangular,
                    _ => GradientKind::Linear,
                };
                if stops.len() < 2 {
                    return Some(stops.first().map(|s| RawFill::Solid(s.color)).unwrap_or(RawFill::None));
                }
                Some(RawFill::Gradient { kind, angle, cx: cx / 100.0, cy: cy / 100.0, pad: (pad / 100.0).clamp(0.0, 0.49), stops })
            }
            // two-color bitmap patterns: the mix of their colors
            7 | 8 => {
                let (fg, bg) = if self.wide {
                    let tags = tagged(self, r);
                    let Some(t) = tags.iter().find(|t| t.0 == 1) else { return Some(RawFill::Solid(GRAY)) };
                    let mut t = self.rd(t.1);
                    t.skip(2 + 3 + 8 + 8 + 1);
                    (t.u16(), t.u16())
                } else {
                    r.skip(14);
                    let fg = r.u16();
                    let bg = r.u16();
                    r.skip(2);
                    (fg, bg)
                };
                Some(RawFill::Solid(Color::lerp(self.color(fg), self.color(bg), 0.5)))
            }
            // full-color (vector) patterns: the procedure that draws the tile, the tile's size
            // (thousandths of an inch, or a 254000th in the 32-bit variant, in its own tagged
            // field there), its offsets, flags (4: a size relative to the object, in percent),
            // and the box of the procedure's drawing
            10 => {
                let (proc, w, h, flags, frame) = if self.wide {
                    let tags = tagged(self, r);
                    let (_, body, _) = *tags.iter().find(|t| t.0 == 1)?;
                    let mut t = self.rd(body);
                    let proc = t.u16();
                    // the tile in a tagged field of its own: its id and size, then the record
                    t.skip(3);
                    let (w, h) = (t.i32().unsigned_abs() as f64, t.i32().unsigned_abs() as f64);
                    t.skip(6);
                    let flags = t.u16();
                    t.skip(1);
                    (proc, w / 254_000.0 * 72.0, h / 254_000.0 * 72.0, flags, self.bbox(&mut t))
                } else {
                    let proc = r.u16();
                    let (w, h) = (r.u16() as f64, r.u16() as f64);
                    r.skip(6);
                    let flags = r.u16();
                    (proc, w / 1000.0 * 72.0, h / 1000.0 * 72.0, flags, self.bbox(r))
                };
                // a size relative to the object is in percent of it
                let relative = flags & 0x04 != 0;
                let (w, h) = if relative {
                    let unit = if self.wide { 254_000.0 } else { 1000.0 } / 72.0;
                    (w * unit / 100.0, h * unit / 100.0)
                } else {
                    (w, h)
                };
                Some(RawFill::Vector { proc, w, h, relative, frame })
            }
            // bitmap, texture and PostScript fills: a neutral tone
            6 | 9 | 11 => {
                if !self.wide {
                    match kind {
                        6 => {
                            r.skip(4);
                            let n = r.u16() as usize;
                            r.skip(n * 2);
                            let _ = r.string();
                        }
                        9 => r.skip(14 + 8),
                        _ => {
                            r.skip(14 + 8 + 1 + 4 + 2);
                            for _ in 0..3 {
                                let _ = r.string();
                            }
                            let n = r.u16() as usize;
                            r.skip(n * 8);
                        }
                    }
                }
                Some(RawFill::Solid(GRAY))
            }
            _ => {
                if self.wide {
                        Some(RawFill::Solid(GRAY))
                } else {
                    None
                }
            }
        }
    }

    /// A lens: the transparency of a uniform glass lens; other lenses change nothing. The outer
    /// `None` when the 16-bit layout cannot be followed.
    fn lens(&self, r: &mut Rd) -> Option<Option<f64>> {
        match r.u8() {
            1 => {
                let method = r.u8();
                let rate = r.u16() as f64;
                r.skip(4);
                Some((method == 0).then(|| (100.0 - rate / 10.0).clamp(0.0, 100.0)))
            }
            2 | 3 => {
                r.skip(4);
                Some(None)
            }
            4 => {
                r.skip(8);
                Some(None)
            }
            _ if self.wide => Some(None),
            _ => None,
        }
    }

    /// An outline description: line style, screen, color, arrowheads, pen and dashes.
    fn outline(&self, index: u16) -> Outline {
        let Some(&[style, _screen, color, _arrows, pen, dash]) = index.checked_sub(1).and_then(|i| self.outlines.get(i as usize)) else {
            return Outline::none();
        };
        let (spec, cap_join) = style.checked_sub(1).and_then(|i| self.line_styles.get(i as usize)).copied().unwrap_or((0x02, 0));
        if spec & 0x01 != 0 {
            return Outline::none();
        }
        let mut o = Outline {
            color: Some(self.color(color)),
            cap: match cap_join & 0x0f {
                1 => LineCap::Round,
                2 => LineCap::Square,
                _ => LineCap::Butt,
            },
            join: match cap_join >> 4 {
                1 => LineJoin::Round,
                2 => LineJoin::Bevel,
                _ => LineJoin::Miter,
            },
            behind_fill: spec & 0x10 != 0,
            scale_with_object: spec & 0x20 != 0,
            ..Default::default()
        };
        match pen.checked_sub(1).and_then(|i| self.pens.get(i as usize)) {
            Some(p) => {
                let [a, b, c, d, _, _] = p.matrix.as_coeffs();
                let scale = ((a * a + b * b).sqrt() + (c * c + d * d).sqrt()) / 2.0;
                o.width = p.width * self.k * if scale > 0.0 { scale } else { 1.0 };
                if p.aspect > 0.0 && p.aspect <= 100.0 {
                    o.stretch = p.aspect;
                    o.nib_angle = p.angle;
                }
            }
            None => o.width = 0.0,
        }
        if spec & 0x04 != 0 {
            if let Some(d) = dash.checked_sub(1).and_then(|i| self.dashes.get(i as usize)) {
                if d.iter().any(|v| *v > 0.0) {
                    o.dash = d.clone();
                }
            }
        }
        o
    }

    /// The style of an object whose shape covers `bounds` (page points); `world` takes its own
    /// coordinates to the page.
    fn style(&mut self, a: &Attrs, bounds: Rect, world: Affine, doc: &mut Document) -> Style {
        let mut st = Style::default();
        st.fill = match &a.fill {
            Some(RawFill::Solid(c)) => Fill::Solid(*c),
            Some(RawFill::Gradient { kind, angle, cx, cy, pad, stops }) => fit_gradient(*kind, *angle, *cx, *cy, *pad, stops, bounds, world),
            &Some(RawFill::Vector { proc, w, h, relative, frame }) => {
                let (w, h) = if relative { (w * bounds.width(), h * bounds.height()) } else { (w, h) };
                match self.pattern_tile(proc, w, h, frame, doc) {
                    Some((image, tile)) => Fill::Pattern(PatternFill {
                        kind: PatternKind::Checker,
                        fg: Color::BLACK,
                        bg: Color::WHITE,
                        tile: w,
                        angle: 0.0,
                        image: Some(image),
                        transform_with_object: true,
                        vector: true,
                        mirror: 0,
                        source: Some(PatternSource::Tile(tile)),
                    }),
                    None => Fill::Solid(GRAY),
                }
            }
            _ => Fill::None,
        };
        st.outline = a.outline.clone().unwrap_or_else(Outline::none);
        if let Some(t) = a.transparency {
            if t > 0.0 {
                st.transparency = Transparency::Uniform(t);
            }
        }
        st
    }
}

/// Place a fountain fill on an object. Gradients are positioned in the object's local bounding
/// box, so the page-space line is taken back through the object's transform.
fn fit_gradient(kind: GradientKind, angle: f64, cx: f64, cy: f64, pad: f64, stops: &[GradientStop], bounds: Rect, world: Affine) -> Fill {
    let (w, h) = (bounds.width().max(1e-6), bounds.height().max(1e-6));
    let c = bounds.center();
    let a = angle.to_radians();
    let dir = kurbo::Vec2::new(a.cos(), -a.sin());
    let (start, end) = match kind {
        GradientKind::Linear => {
            let half = (w * dir.x.abs() + h * dir.y.abs()) / 2.0;
            let span = half * (1.0 - 2.0 * pad);
            (c - dir * span, c + dir * span)
        }
        _ => {
            let center = Point::new(c.x + cx * w, c.y - cy * h);
            let reach = (w.max(h) / 2.0) * (1.0 - 2.0 * pad).max(0.05);
            let dir = if kind == GradientKind::Conical { -dir } else { dir };
            (center, center + dir * reach)
        }
    };
    let inv = world.inverse();
    let local = (inv * bounds.to_path(0.1)).bounding_box();
    let unit = |p: Point| {
        let q = inv * p;
        Point::new((q.x - local.x0) / local.width().max(1e-9), (q.y - local.y0) / local.height().max(1e-9))
    };
    let mut st: Vec<GradientStop> = stops.to_vec();
    if !matches!(kind, GradientKind::Linear | GradientKind::Conical) {
        // radial and square fountain fills run from the edge to the center
        for s in st.iter_mut() {
            s.pos = 1.0 - s.pos;
        }
        st.reverse();
    }
    Fill::Gradient(Gradient { kind, stops: st, start: unit(start), end: unit(end), spread: SpreadMode::Pad, steps: 0, accel: 0.0, blend: 0, smooth: false, free_scale: true })
}

// ------------------------------------------------------------------------------------------
// Pages: the instruction stream
// ------------------------------------------------------------------------------------------

/// The text being drawn: the text group's attributes and matrix, the character style.
#[derive(Clone)]
struct TextState {
    attrs: Attrs,
    matrix: Affine,
    font: u16,
    /// File units.
    size: f64,
}

/// What a page builds while its instructions run.
struct Build {
    /// File units to page points.
    to_page: Affine,
    layers: Vec<Layer>,
    /// The open layer: name, matrix, objects.
    layer: Option<(String, Affine, Vec<Object>)>,
    groups: Vec<Vec<Object>>,
    text: Option<TextState>,
}

impl Build {
    fn world(&self) -> Affine {
        self.to_page * self.layer.as_ref().map(|l| l.1).unwrap_or(Affine::IDENTITY)
    }
    fn push(&mut self, o: Object) {
        if let Some(g) = self.groups.last_mut() {
            g.push(o);
        } else if let Some(l) = self.layer.as_mut() {
            l.2.push(o);
        } else {
            self.layer = Some(("Layer 1".into(), Affine::IDENTITY, vec![o]));
        }
    }
    fn end_group(&mut self, doc: &mut Document) {
        if let Some(kids) = self.groups.pop() {
            if !kids.is_empty() {
                self.push(Object::new(doc.new_id(), ObjKind::Group(kids), Affine::IDENTITY, Style::default()));
            }
        }
    }
    fn end_layer(&mut self, doc: &mut Document) {
        while !self.groups.is_empty() {
            self.end_group(doc);
        }
        if let Some((name, _, objects)) = self.layer.take() {
            let mut l = Layer::new(doc.new_id(), if name.is_empty() { "Layer 1" } else { &name });
            l.objects = objects;
            self.layers.push(l);
        }
    }
}

impl<'a> Cmx<'a> {
    /// Run the instructions of the `page` chunk at `at`.
    fn page(&mut self, at: usize, doc: &mut Document) -> Option<Drawing> {
        // without a box: a letter-size page around the origin
        let (hw, hh) = (306.0 / self.k, 396.0 / self.k);
        let box_ = self.bbox.unwrap_or(Rect::new(-hw, -hh, hw, hh));
        let mut b = Build { to_page: Affine::IDENTITY, layers: vec![], layer: None, groups: vec![], text: None };
        let box_ = self.run(at, &mut b, Some(box_), doc).unwrap_or(box_);
        if b.layers.is_empty() {
            b.layers.push(Layer::new(doc.new_id(), "Layer 1"));
        }
        Some(Drawing { width: box_.width() * self.k, height: box_.height() * self.k, layers: b.layers })
    }

    /// Run the instructions of the `page` or `proc` chunk at `at` into `b`. A page places its
    /// box's top-left corner at the origin (`page_box`, which a begin-page instruction can
    /// change; the box in use is returned); a procedure keeps `b`'s placement and ends at its
    /// end-of-procedure instruction.
    fn run(&mut self, at: usize, b: &mut Build, page_box: Option<Rect>, doc: &mut Document) -> Option<Rect> {
        let size = self.rd(at + 4).u32() as usize;
        let mut end = (at + 8).saturating_add(size).min(self.b.len());
        let mut c = at + 8;
        let mut box_ = page_box;
        let set_box = |b: &mut Build, r: Rect, k: f64| b.to_page = Affine::new([k, 0.0, 0.0, -k, -r.x0 * k, r.y1 * k]);
        if let Some(r) = box_ {
            set_box(b, r, self.k);
        }
        let mut jumps = HashSet::new();
        let mut steps = 0usize;
        while c + 4 <= end && steps < 20_000_000 {
            steps += 1;
            let mut r = self.rd(c);
            let mut size = r.i16() as i64;
            let mut min = 4;
            if size < 0 {
                size = r.i32() as i64;
                min = 8;
            }
            let size = (size.max(min) as usize).min(end - c).max(min as usize);
            let code = (r.i16() as i32).abs();
            let stop = (c + size).min(end);
            let mut next = c + size;
            match code {
                BEGIN_PAGE if page_box.is_some() => {
                    if let Some(page_box) = self.begin_page(&mut r, stop) {
                        box_ = Some(page_box);
                        set_box(b, page_box, self.k);
                    }
                }
                END_PAGE => break,
                END_PROCEDURE if page_box.is_none() => break,
                BEGIN_LAYER => {
                    b.end_layer(doc);
                    let (name, matrix) = self.begin_layer(&mut r, stop);
                    b.layer = Some((name, matrix, vec![]));
                }
                END_LAYER => b.end_layer(doc),
                BEGIN_GROUP => b.groups.push(vec![]),
                END_GROUP => b.end_group(doc),
                POLYCURVE | RECTANGLE | ELLIPSE | DRAW_IMAGE => {
                    let o = match code {
                        POLYCURVE => self.polycurve(&mut r, stop, b, doc),
                        RECTANGLE => self.rectangle(&mut r, stop, b, doc),
                        ELLIPSE => self.ellipse(&mut r, stop, b, doc),
                        _ => self.draw_image(&mut r, stop, b, doc),
                    };
                    match o {
                        Some(o) => b.push(o),
                        None => self.skip(code),
                    }
                }
                BEGIN_TEXT_GROUP => b.text = self.begin_text(&mut r, stop),
                END_TEXT_GROUP => b.text = None,
                SET_CHAR_STYLE => self.char_style(&mut r, stop, &mut b.text),
                DRAW_CHARS => match self.draw_chars(&mut r, stop, b, doc) {
                    Some(Some(o)) => b.push(o),
                    Some(None) => {}
                    None => self.skip(code),
                },
                SIMPLE_WIDE_TEXT => self.skip(code),
                JUMP_ABSOLUTE => {
                    let to = if self.wide { self.tags(&mut r, stop).iter().find(|t| t.0 == 1).map(|t| self.rd(t.1).u32()) } else { Some(r.u32()) };
                    if let Some(to) = to.map(|t| t as usize) {
                        if to + 4 <= self.b.len() && jumps.insert(to) {
                            next = to;
                            if to >= end || to < at {
                                end = self.b.len();
                            }
                        }
                    }
                }
                _ => {}
            }
            c = next;
        }
        b.text = None;
        b.end_layer(doc);
        box_
    }

    /// The tile of a full-color (vector) pattern fill: the drawing of procedure `proc`, its box
    /// `frame` (file units) fitted to a tile of `w` by `h` points, kept as a pattern tile of
    /// `doc` and drawn into an image of it. Made once for each procedure and size.
    fn pattern_tile(&mut self, proc: u16, w: f64, h: f64, frame: Rect, doc: &mut Document) -> Option<(u64, u64)> {
        let key = (proc, w.to_bits(), h.to_bits());
        if let Some(made) = self.pattern_tiles.get(&key) {
            return *made;
        }
        // a procedure that uses itself is drawn without its own pattern
        if self.drawing.contains(&proc) || frame.width() <= 0.0 || frame.height() <= 0.0 || w < 0.1 || h < 0.1 {
            return None;
        }
        let &at = self.procs.get(&proc)?;
        self.drawing.push(proc);
        let k = self.k;
        let mut b = Build { to_page: Affine::new([k, 0.0, 0.0, -k, -frame.x0 * k, frame.y1 * k]), layers: vec![], layer: None, groups: vec![], text: None };
        self.run(at, &mut b, None, doc);
        self.drawing.pop();
        let objects: Vec<Object> = b.layers.into_iter().flat_map(|l| l.objects).collect();
        let from = Rect::new(0.0, 0.0, frame.width() * k, frame.height() * k);
        let made = super::build::fitted_tile(doc, objects, w, h, format!("Vector pattern {proc}"), Some(from), true).map(|(tile, rgba, pw, ph)| {
            let encoded = crate::image_io::encode_png(pw, ph, &rgba);
            let image = doc.add_image(ImageData { width: pw, height: ph, rgba: Arc::new(rgba), encoded: Arc::new(encoded), format: "png".into(), name: format!("vector pattern {proc}") });
            (image, doc.add_pattern_tile(tile))
        });
        self.pattern_tiles.insert(key, made);
        made
    }

    /// Count an instruction that could not be converted.
    fn skip(&mut self, code: i32) {
        *self.tally.skipped.entry(code as u32).or_default() += 1;
    }

    /// The page's box (16-bit: page number, flags, box; 32-bit: the same in tag 1).
    fn begin_page(&self, r: &mut Rd, stop: usize) -> Option<Rect> {
        let spec = |s: &Self, r: &mut Rd| {
            r.skip(2);
            let _flags = r.u32();
            s.bbox(r)
        };
        let page = if self.wide {
            let tags = self.tags(r, stop);
            tags.iter().find(|t| t.0 == 1).map(|t| spec(self, &mut self.rd(t.1)))?
        } else {
            spec(self, r)
        };
        (page.width() >= 1.0 && page.height() >= 1.0).then_some(page)
    }

    /// The layer's name and matrix.
    fn begin_layer(&self, r: &mut Rd, stop: usize) -> (String, Affine) {
        let spec = |r: &mut Rd| {
            r.skip(2 + 2 + 4 + 4);
            r.string()
        };
        if self.wide {
            let (mut name, mut m) = (String::new(), Affine::IDENTITY);
            for (id, body, _) in self.tags(r, stop) {
                match id {
                    1 => name = spec(&mut self.rd(body)),
                    2 => m = self.matrix(&mut self.rd(body)),
                    _ => {}
                }
            }
            (name, m)
        } else {
            let name = spec(r);
            let m = if r.p + 2 <= stop { self.matrix(r) } else { Affine::IDENTITY };
            (name, m)
        }
    }

    /// The attributes and the rest of a drawing instruction: 16-bit records follow the
    /// attributes; 32-bit ones are tagged, the attributes in tag 1 and the shape in tag 2.
    fn shape(&mut self, r: &mut Rd<'a>, stop: usize) -> Option<(Attrs, Rd<'a>, usize)> {
        if self.wide {
            let tags = self.tags(r, stop);
            let mut attrs = Attrs::default();
            for (id, body, end) in &tags {
                if *id == 1 {
                    attrs = self.attrs(&mut self.rd(*body), *end)?;
                }
            }
            let (_, body, end) = *tags.iter().find(|t| t.0 == 2)?;
            Some((attrs, self.rd(body), end))
        } else {
            let attrs = self.attrs(r, stop)?;
            Some((attrs, r.clone(), stop))
        }
    }

    fn polycurve(&mut self, r: &mut Rd<'a>, stop: usize, b: &Build, doc: &mut Document) -> Option<Object> {
        let (attrs, mut s, end) = self.shape(r, stop)?;
        let n = s.u16() as usize;
        let each = if self.wide { 9 } else { 5 };
        let n = n.min(end.saturating_sub(s.p) / each);
        let pts: Vec<Point> = (0..n).map(|_| Point::new(self.coord(&mut s), self.coord(&mut s))).collect();
        let kinds = s.bytes(n);
        let path = b.world() * curve(&pts, kinds);
        if path.elements().len() < 2 {
            return None;
        }
        let mut st = self.style(&attrs, path.bounding_box(), Affine::IDENTITY, doc);
        // CorelDRAW never fills an open curve, though the metafile may still name a fill for it.
        if !path.elements().iter().any(|e| matches!(e, kurbo::PathEl::ClosePath)) {
            st.fill = Fill::None;
        }
        Some(Object::new(doc.new_id(), ObjKind::Path(NodePath::from_bez(&path)), Affine::IDENTITY, st))
    }

    fn rectangle(&mut self, r: &mut Rd<'a>, stop: usize, b: &Build, doc: &mut Document) -> Option<Object> {
        let (attrs, mut s, _) = self.shape(r, stop)?;
        let (cx, cy) = (self.coord(&mut s), self.coord(&mut s));
        let (w, h) = (self.coord(&mut s).abs(), self.coord(&mut s).abs());
        let radius = self.coord(&mut s).abs();
        let angle = self.angle(&mut s);
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let k = self.k;
        let rad = radius.min(w.min(h) / 2.0) * k;
        let shape = RectShape { w: w * k, h: h * k, radii: [rad; 4], corner: Default::default() };
        let xf = b.world() * Affine::translate((cx, cy)) * Affine::rotate(angle.to_radians()) * Affine::scale_non_uniform(1.0 / k, -1.0 / k) * Affine::translate((-shape.w / 2.0, -shape.h / 2.0));
        let bounds = (xf * Rect::new(0.0, 0.0, shape.w, shape.h).to_path(0.1)).bounding_box();
        let st = self.style(&attrs, bounds, xf, doc);
        Some(Object::new(doc.new_id(), ObjKind::Rect(shape), xf, st))
    }

    fn ellipse(&mut self, r: &mut Rd<'a>, stop: usize, b: &Build, doc: &mut Document) -> Option<Object> {
        let (attrs, mut s, _) = self.shape(r, stop)?;
        let (cx, cy) = (self.coord(&mut s), self.coord(&mut s));
        let (w, h) = (self.coord(&mut s).abs(), self.coord(&mut s).abs());
        let (a1, a2, rot) = (self.angle(&mut s), self.angle(&mut s), self.angle(&mut s));
        let pie = s.u8() != 0;
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let k = self.k;
        let full = (a1 - a2).abs() < 1e-6;
        // angles counter-clockwise from 3 o'clock, as ours
        let shape = EllipseShape {
            w: w * k,
            h: h * k,
            kind: if full {
                EllipseKind::Ellipse
            } else if pie {
                EllipseKind::Pie
            } else {
                EllipseKind::Arc
            },
            start: if full { 0.0 } else { a1.rem_euclid(360.0) },
            end: if full { 0.0 } else { a2.rem_euclid(360.0) },
            clockwise: false,
        };
        let xf = b.world() * Affine::translate((cx, cy)) * Affine::rotate(rot.to_radians()) * Affine::scale_non_uniform(1.0 / k, -1.0 / k) * Affine::translate((-shape.w / 2.0, -shape.h / 2.0));
        let bounds = (xf * Rect::new(0.0, 0.0, shape.w, shape.h).to_path(0.1)).bounding_box();
        let st = self.style(&attrs, bounds, xf, doc);
        Some(Object::new(doc.new_id(), ObjKind::Ellipse(shape), xf, st))
    }

    /// A bitmap: the box it fills, its crop box, a matrix, the image's type and number.
    fn draw_image(&mut self, r: &mut Rd<'a>, stop: usize, b: &Build, doc: &mut Document) -> Option<Object> {
        let (_attrs, mut s, _) = self.shape(r, stop)?;
        let place = self.bbox(&mut s);
        let _crop = self.bbox(&mut s);
        let m = self.matrix(&mut s);
        let _kind = s.u16();
        let index = s.u16();
        let (image, _, _) = *self.images.get(&index)?;
        let k = self.k;
        if place.width() <= 0.0 || place.height() <= 0.0 {
            return None;
        }
        let img = ImageObj { image, w: place.width() * k, h: place.height() * k, crop: [0.0, 0.0, 1.0, 1.0], adjust: ImageAdjust::default() };
        let xf = b.world() * m * Affine::translate((place.x0, place.y1)) * Affine::scale_non_uniform(1.0 / k, -1.0 / k);
        let mut st = Style::default();
        st.outline = Outline::none();
        Some(Object::new(doc.new_id(), ObjKind::Image(img), xf, st))
    }

    /// A text group: the attributes of its characters, its matrix and box.
    fn begin_text(&mut self, r: &mut Rd<'a>, stop: usize) -> Option<TextState> {
        let mut t = TextState { attrs: Attrs { fill: Some(RawFill::Solid(Color::BLACK)), outline: Some(Outline::none()), transparency: None }, matrix: Affine::IDENTITY, font: 0, size: 0.0 };
        if self.wide {
            for (id, body, end) in self.tags(r, stop) {
                match id {
                    1 => t.attrs = self.attrs(&mut self.rd(body), end)?,
                    2 => t.matrix = self.matrix(&mut self.rd(body)),
                    _ => {}
                }
            }
        } else {
            t.attrs = self.attrs(r, stop)?;
            t.matrix = self.matrix(r);
        }
        Some(t)
    }

    /// A character style: attributes (when the characters differ from their group), the font's
    /// number and the size.
    fn char_style(&mut self, r: &mut Rd<'a>, stop: usize, text: &mut Option<TextState>) {
        let Some(t) = text.as_mut() else { return };
        let spec = |s: &Self, r: &mut Rd| (r.u16(), s.coord(r).abs());
        let (attrs, (font, size)) = if self.wide {
            let tags = self.tags(r, stop);
            let mut attrs = None;
            let mut fs = (0, 0.0);
            for (id, body, end) in tags {
                match id {
                    1 => attrs = self.attrs(&mut self.rd(body), end),
                    2 => fs = spec(self, &mut self.rd(body)),
                    _ => {}
                }
            }
            (attrs, fs)
        } else {
            let attrs = self.attrs(r, stop);
            (attrs, spec(self, r))
        };
        if let Some(a) = attrs {
            if !matches!(a.fill, Some(RawFill::None)) {
                t.attrs.fill = a.fill;
            }
            if a.outline.as_ref().is_some_and(|o| o.is_visible()) {
                t.attrs.outline = a.outline;
            }
        }
        t.font = font;
        t.size = size;
    }

    /// Characters: their count, the characters, then a position (x, y) each. `Some(None)` for
    /// blank characters.
    fn draw_chars(&mut self, r: &mut Rd<'a>, stop: usize, b: &Build, doc: &mut Document) -> Option<Option<Object>> {
        let t = b.text.clone()?;
        let mut s = if self.wide {
            let tags = self.tags(r, stop);
            let (_, body, _) = *tags.iter().find(|t| t.0 == 1)?;
            self.rd(body)
        } else {
            r.clone()
        };
        let n = s.u16() as usize;
        let n = n.min(stop.saturating_sub(s.p) / 2);
        let chars: String = (0..n).map(|_| cp1252(s.u16())).collect();
        let at: Vec<Point> = (0..n).map(|_| Point::new(self.coord(&mut s), self.coord(&mut s))).collect();
        let (x, y) = at.first().map(|p| (p.x, p.y)).unwrap_or_default();
        if chars.trim().is_empty() {
            return Some(None);
        }
        if t.size <= 0.0 {
            return None;
        }
        let k = self.k;
        let font = t.font.checked_sub(1).and_then(|i| self.fonts.get(i as usize)).filter(|f| !f.is_empty()).cloned().unwrap_or_else(vector_core::text::default_font_family);
        let format = CharFormat { font, size: t.size * k, ..Default::default() };
        let text = TextObj::artistic(chars.trim_end(), format);
        // the characters as far apart as CorelDRAW's font put them, when ours is wider or
        // narrower: the line is stretched to end where CorelDRAW's ended
        let last = chars.trim_end().chars().count().saturating_sub(1);
        let mut stretch = 1.0;
        if last > 0 && at.get(last).is_some_and(|p| (p.y - y).abs() < 1e-6) {
            let theirs = (at[last].x - x) * k;
            let ours = vector_core::text::layout(&text).carets.get(last).map(|c| c.0).unwrap_or(0.0);
            if theirs > 0.0 && ours > 0.0 {
                stretch = (theirs / ours).clamp(0.5, 2.0);
            }
        }
        let xf = b.world() * t.matrix * Affine::translate((x, y)) * Affine::scale_non_uniform(1.0 / k, -1.0 / k) * Affine::scale_non_uniform(stretch, 1.0);
        let bounds = (xf * Rect::new(0.0, -t.size * k, t.size * k * n as f64 * 0.5, 0.0).to_path(0.1)).bounding_box();
        let st = self.style(&t.attrs, bounds, xf, doc);
        Some(Some(Object::new(doc.new_id(), ObjKind::Text(text), xf, st)))
    }
}

/// A curve: points and one flag byte per point. The two high bits say what the point is (00
/// the start of a subpath, 01 a line's end, 10 a curve's end, 11 a control point); 0x08 closes
/// the subpath at that point.
pub(super) fn curve(pts: &[Point], kinds: &[u8]) -> BezPath {
    let mut bp = BezPath::new();
    let mut ctrl: Vec<Point> = vec![];
    let mut open = false;
    for (p, t) in pts.iter().zip(kinds) {
        match t & 0xc0 {
            0x00 => {
                bp.move_to(*p);
                ctrl.clear();
                open = true;
                continue;
            }
            0xc0 => {
                ctrl.push(*p);
                continue;
            }
            _ if !open => {
                bp.move_to(*p);
                open = true;
                ctrl.clear();
                continue;
            }
            0x80 if ctrl.len() >= 2 => bp.curve_to(ctrl[ctrl.len() - 2], ctrl[ctrl.len() - 1], *p),
            _ => bp.line_to(*p),
        }
        ctrl.clear();
        if t & 0x08 != 0 {
            bp.close_path();
            open = false;
        }
    }
    bp
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes a small CMX file: either byte order, 16- or 32-bit coordinates.
    struct W {
        be: bool,
        wide: bool,
        b: Vec<u8>,
    }

    impl W {
        fn u8(&mut self, v: u8) {
            self.b.push(v);
        }
        fn u16(&mut self, v: u16) {
            self.b.extend(if self.be { v.to_be_bytes() } else { v.to_le_bytes() });
        }
        fn u32(&mut self, v: u32) {
            self.b.extend(if self.be { v.to_be_bytes() } else { v.to_le_bytes() });
        }
        fn f64(&mut self, v: f64) {
            self.b.extend(if self.be { v.to_be_bytes() } else { v.to_le_bytes() });
        }
        fn coord(&mut self, v: i32) {
            if self.wide {
                self.u32(v as u32)
            } else {
                self.u16(v as i16 as u16)
            }
        }
        fn angle(&mut self, degrees: f64) {
            if self.wide {
                self.u32((degrees * 1e6) as i32 as u32)
            } else {
                self.u16((degrees * 10.0) as i16 as u16)
            }
        }
        fn put_u32(&mut self, at: usize, v: u32) {
            let x = if self.be { v.to_be_bytes() } else { v.to_le_bytes() };
            self.b[at..at + 4].copy_from_slice(&x);
        }
        fn put_u16(&mut self, at: usize, v: u16) {
            let x = if self.be { v.to_be_bytes() } else { v.to_le_bytes() };
            self.b[at..at + 2].copy_from_slice(&x);
        }
        /// Start a chunk; returns where its size goes.
        fn chunk(&mut self, tag: &[u8; 4]) -> usize {
            self.b.extend_from_slice(tag);
            self.u32(0);
            self.b.len() - 4
        }
        fn end_chunk(&mut self, at: usize) {
            let n = self.b.len() - at - 4;
            self.put_u32(at, n as u32);
            if n % 2 == 1 {
                self.b.push(0);
            }
        }
        /// A 32-bit tagged field (`f` writes its body); 16-bit records are written bare.
        fn tag(&mut self, id: u8, f: impl FnOnce(&mut W)) {
            if !self.wide {
                return f(self);
            }
            let at = self.b.len();
            self.u8(id);
            self.u16(0);
            f(self);
            let n = self.b.len() - at;
            self.put_u16(at + 1, n as u16);
        }
        fn end_tags(&mut self) {
            if self.wide {
                self.u8(TAG_END);
            }
        }
        /// An instruction.
        fn op(&mut self, code: i32, f: impl FnOnce(&mut W)) {
            let at = self.b.len();
            self.u16(0);
            self.u16((-code) as i16 as u16);
            f(self);
            let n = self.b.len() - at;
            self.put_u16(at, n as u16);
        }
        /// Rendering attributes: a uniform fill of color `fill` and outline `outline` (0: none).
        fn attrs(&mut self, fill: u16, outline: u16) {
            let mask = (fill > 0) as u8 | ((outline > 0) as u8) << 1;
            self.tag(1, |w| {
                w.u8(mask);
                if fill > 0 {
                    w.tag(1, |w| {
                        w.u16(1);
                        w.tag(1, |w| {
                            w.u16(fill);
                            w.u16(0);
                        });
                        w.end_tags();
                    });
                    w.end_tags();
                }
                if outline > 0 {
                    w.tag(1, |w| w.u16(outline));
                    w.end_tags();
                }
            });
        }
        fn string(&mut self, s: &str) {
            self.u16(s.len() as u16 + 1);
            self.b.extend_from_slice(s.as_bytes());
            self.b.push(0);
        }
    }

    /// A page 2000 x 1000 thousandths of an inch around the origin, on a layer "Shapes": a group
    /// of a red triangle with a blue outline and a green rectangle, then a blue ellipse, then
    /// blue text.
    fn sample(be: bool, wide: bool) -> Vec<u8> {
        let mut w = W { be, wide, b: vec![] };
        let unit = if wide { 254.0 } else { 1.0 };
        let u = |v: i32| (v as f64 * unit) as i32;
        w.b.extend_from_slice(if be { b"RIFX" } else { b"RIFF" });
        w.u32(0);
        w.b.extend_from_slice(b"CMX1");
        let cont = w.chunk(b"cont");
        let mut id = b"{Corel Binary Meta File}".to_vec();
        id.resize(32, 0);
        w.b.extend(id);
        let mut platform = b"Windows 3.1".to_vec();
        platform.resize(16, 0);
        w.b.extend(platform);
        w.b.extend(if be { b"4\0\0\0" } else { b"2\0\0\0" });
        w.b.extend(if wide { b"4\0" } else { b"2\0" });
        w.b.extend(b"1\0\0\02\0\0\0");
        w.u16(64);
        w.f64(if wide { 1.0 / 254_000.0 } else { 0.001 });
        w.b.extend([0; 12]);
        let index_at = w.b.len();
        w.u32(0);
        w.u32(u32::MAX);
        w.u32(u32::MAX);
        for v in [-1000, 500, 1000, -500] {
            w.u32(u(v) as u32);
        }
        w.b.extend([0; 16]);
        w.end_chunk(cont);
        // colors: red (CMYK), blue (RGB), green (CMYK 0..255)
        let rclr = w.b.len();
        let c = w.chunk(b"rclr");
        w.u16(3);
        for (model, v) in [(2u8, [0u8, 100, 100, 0]), (5, [0, 0, 255, 0]), (3, [255, 0, 255, 0])] {
            let n = if model == 5 { 3 } else { 4 };
            w.tag(1, |w| {
                w.u8(model);
                w.u8(0);
            });
            w.tag(2, |w| w.b.extend_from_slice(&v[..n]));
            w.end_tags();
        }
        w.end_chunk(c);
        let rpen = w.b.len();
        let c = w.chunk(b"rpen");
        w.u16(1);
        w.tag(1, |w| {
            w.coord(u(50));
            w.u16(100);
            w.angle(0.0);
            if !w.wide {
                w.u16(0);
            }
            w.u16(1);
        });
        w.end_tags();
        w.end_chunk(c);
        let rott = w.b.len();
        let c = w.chunk(b"rott");
        w.u16(1);
        w.tag(1, |w| {
            w.u8(0x02);
            w.u8(0x11);
        });
        w.end_tags();
        w.end_chunk(c);
        let rotl = w.b.len();
        let c = w.chunk(b"rotl");
        w.u16(1);
        w.tag(1, |w| {
            for v in [1u16, 0, 2, 0, 1, 0] {
                w.u16(v);
            }
        });
        w.end_tags();
        w.end_chunk(c);
        // the font index, its one record right after it
        let ixtl = w.b.len();
        let c = w.chunk(b"ixtl");
        w.u16(1);
        if wide {
            w.u16(4);
        }
        w.u16(7);
        let font_offset = w.b.len();
        w.u32(0);
        w.end_chunk(c);
        let font = w.b.len() as u32;
        w.put_u32(font_offset, font);
        w.tag(1, |w| w.string("Courier New"));
        w.end_tags();
        let page = w.b.len();
        let c = w.chunk(b"page");
        w.op(BEGIN_PAGE, |w| {
            w.tag(1, |w| {
                w.u16(1);
                w.u32(0);
                for v in [-1000, 500, 1000, -500] {
                    w.coord(u(v));
                }
            });
            w.end_tags();
        });
        w.op(BEGIN_LAYER, |w| {
            w.tag(1, |w| {
                w.u16(1);
                w.u16(1);
                w.u32(0);
                w.u32(0);
                w.string("Shapes");
            });
            w.tag(2, |w| w.u16(1));
            w.end_tags();
        });
        w.op(BEGIN_GROUP, |w| {
            w.tag(1, |w| {
                for v in [-1000, 500, 0, -500] {
                    w.coord(u(v));
                }
                w.u16(2);
                w.u32(2);
                w.u32(0);
            });
            w.end_tags();
        });
        // a closed triangle: (-1000,-500) (0,-500) (-500,500)
        w.op(POLYCURVE, |w| {
            w.attrs(1, 1);
            w.tag(2, |w| {
                w.u16(4);
                for (x, y) in [(-1000, -500), (0, -500), (-500, 500), (-1000, -500)] {
                    w.coord(u(x));
                    w.coord(u(y));
                }
                w.b.extend([0x0c, 0x44, 0x44, 0x48]);
            });
            w.end_tags();
        });
        // a rectangle 200 x 100 centered on (500, 250)
        w.op(RECTANGLE, |w| {
            w.attrs(3, 0);
            w.tag(2, |w| {
                for v in [500, 250, 200, 100, 0] {
                    w.coord(u(v));
                }
                w.angle(0.0);
            });
            w.end_tags();
        });
        w.op(END_GROUP, |_| {});
        // an ellipse 400 x 200 centered on (500, -250)
        w.op(ELLIPSE, |w| {
            w.attrs(2, 0);
            w.tag(2, |w| {
                for v in [500, -250, 400, 200] {
                    w.coord(u(v));
                }
                w.angle(0.0);
                w.angle(0.0);
                w.angle(0.0);
                w.u8(0);
            });
            w.end_tags();
        });
        // text "Hi" at (100, 0) in the text group, 250 units high
        w.op(BEGIN_TEXT_GROUP, |w| {
            w.attrs(2, 0);
            w.tag(2, |w| {
                w.u16(2);
                for v in [1.0, 0.0, 0.0, 1.0, u(100) as f64, 0.0] {
                    w.f64(v);
                }
            });
            w.end_tags();
        });
        w.op(SET_CHAR_STYLE, |w| {
            w.tag(1, |w| w.u8(0));
            w.tag(2, |w| {
                w.u16(1);
                w.coord(u(250));
            });
            w.end_tags();
        });
        w.op(DRAW_CHARS, |w| {
            w.tag(1, |w| {
                w.u16(2);
                w.u16(b'H' as u16);
                w.u16(b'i' as u16);
                for v in [0, 0, 150, 0] {
                    w.coord(u(v));
                }
            });
            w.end_tags();
        });
        w.op(END_TEXT_GROUP, |_| {});
        // a jump over bytes that are not instructions
        let jump = w.b.len();
        w.op(JUMP_ABSOLUTE, |w| {
            w.tag(1, |w| w.u32(0));
            w.end_tags();
        });
        w.b.extend([0xff; 10]);
        let after = w.b.len() as u32;
        w.put_u32(jump + 4 + if wide { 3 } else { 0 }, after);
        w.op(END_LAYER, |_| {});
        w.op(END_PAGE, |_| {});
        w.end_chunk(c);
        let ixpg = w.b.len();
        let c = w.chunk(b"ixpg");
        w.u16(1);
        if wide {
            w.u16(16);
        }
        w.u32(page as u32);
        w.u32(u32::MAX);
        w.u32(u32::MAX);
        w.u32(u32::MAX);
        w.end_chunk(c);
        let ixmr = w.b.len();
        let c = w.chunk(b"ixmr");
        let sections = [(SEC_PAGES, ixpg), (SEC_FONTS, ixtl), (SEC_OUTLINES, rotl), (SEC_LINE_STYLES, rott), (SEC_PENS, rpen), (SEC_COLORS, rclr)];
        w.u16(1);
        w.u16(6);
        w.u16(sections.len() as u16);
        for (id, at) in sections {
            w.u16(id);
            w.u32(at as u32);
        }
        w.end_chunk(c);
        w.put_u32(index_at, ixmr as u32);
        let n = w.b.len() - 8;
        w.put_u32(4, n as u32);
        w.b
    }

    /// The same file with everything after the header deflated into a `pack` chunk.
    fn packed(file: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let at = 12 + 8 + Rd::at(file, 16, false).u32() as usize;
        let mut z = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::default());
        z.write_all(&file[at..]).unwrap();
        let data = z.finish().unwrap();
        let mut out = file[..at].to_vec();
        out.extend_from_slice(b"pack");
        out.extend(((data.len() + 12) as u32).to_le_bytes());
        out.extend(((file.len() - at) as u32).to_le_bytes());
        out.extend_from_slice(b"CPng");
        out.extend([1, 0, 4, 0]);
        out.extend(data);
        out
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn check(doc: &Document) {
        assert_eq!(doc.pages.len(), 1);
        let p = &doc.pages[0];
        // 2000 x 1000 thousandths of an inch
        assert!(near(p.width, 144.0) && near(p.height, 72.0), "{} x {}", p.width, p.height);
        assert_eq!(p.layers.len(), 1);
        assert_eq!(p.layers[0].name, "Shapes");
        let objs = &p.layers[0].objects;
        assert_eq!(objs.len(), 3, "{:#?}", objs.iter().map(|o| o.kind.type_name()).collect::<Vec<_>>());
        let ObjKind::Group(kids) = &objs[0].kind else { panic!("{:?}", objs[0].kind.type_name()) };
        assert_eq!(kids.len(), 2);
        // the triangle: from the page's lower-left corner to the middle of the bottom, apex at
        // the top
        let tri = &kids[0];
        assert!(matches!(tri.kind, ObjKind::Path(_)));
        let b = tri.bounds();
        assert!(near(b.x0, 0.0) && near(b.x1, 72.0) && near(b.y0, 0.0) && near(b.y1, 72.0), "{b:?}");
        assert_eq!(tri.style.fill, Fill::Solid(color::convert(2, u32::from_le_bytes([0, 100, 100, 0]))));
        assert_eq!(tri.style.outline.color, Some(Color::rgb(0, 0, 255)));
        assert!(near(tri.style.outline.width, 3.6), "{}", tri.style.outline.width);
        assert_eq!((tri.style.outline.cap, tri.style.outline.join), (LineCap::Round, LineJoin::Round));
        // the rectangle: 14.4 x 7.2 pt, its top-left corner at (100.8, 14.4)
        let ObjKind::Rect(r) = &kids[1].kind else { panic!() };
        assert!(near(r.w, 14.4) && near(r.h, 7.2));
        let rb = kids[1].bounds();
        assert!(near(rb.x0, 100.8) && near(rb.y0, 14.4), "{rb:?}");
        assert!(!kids[1].style.outline.is_visible());
        assert_eq!(kids[1].style.fill, Fill::Solid(color::convert(3, u32::from_le_bytes([255, 0, 255, 0]))));
        let ObjKind::Ellipse(e) = &objs[1].kind else { panic!() };
        assert_eq!(e.kind, EllipseKind::Ellipse);
        let eb = objs[1].bounds();
        assert!(near(eb.center().x, 108.0) && near(eb.center().y, 54.0) && near(eb.width(), 28.8), "{eb:?}");
        assert_eq!(objs[1].style.fill, Fill::Solid(Color::rgb(0, 0, 255)));
        let ObjKind::Text(t) = &objs[2].kind else { panic!() };
        assert_eq!(t.content, "Hi");
        assert_eq!(t.format.font, "Courier New");
        assert!(near(t.format.size, 18.0));
        assert_eq!(objs[2].style.fill, Fill::Solid(Color::rgb(0, 0, 255)));
        // the baseline starts 1100 units from the left, halfway down
        let at = objs[2].transform * Point::ZERO;
        assert!(near(at.x, 79.2) && near(at.y, 36.0), "{at:?}");
    }

    #[test]
    fn reads_every_variant() {
        for (be, wide) in [(false, false), (false, true), (true, true), (true, false)] {
            let file = sample(be, wide);
            assert!(is_cmx(&file));
            let (doc, report) = load_with_report(&file).unwrap_or_else(|e| panic!("be {be} wide {wide}: {e}"));
            check(&doc);
            assert!(report.notes.is_empty(), "{:?}", report.notes);
            if !be {
                check(&load_from_bytes(&packed(&file)).unwrap());
            }
        }
    }

    #[test]
    fn first_page_for_a_pattern() {
        let mut doc = Document::new(100.0, 100.0);
        let before = doc.next_id_value();
        let d = first_page(&sample(false, true), &mut doc).unwrap();
        assert!(near(d.width, 144.0) && near(d.height, 72.0));
        let objs = d.objects();
        assert_eq!(objs.len(), 3);
        // ids come from the document the pattern goes into, which keeps its own page
        assert!(objs.iter().all(|o| o.id >= before));
        assert!(doc.next_id_value() > before);
        assert_eq!(doc.pages.len(), 1);
    }

    #[test]
    fn opens_through_the_coreldraw_reader() {
        // clipart CDs name these .cdr, with the form type CDRX
        let mut file = sample(false, false);
        file[8..12].copy_from_slice(b"CDRX");
        check(&super::super::load_from_bytes(&file).unwrap());
        assert_eq!(super::super::version(&file).unwrap(), 0);
        assert_eq!(crate::cdr::detect(&file), crate::cdr::CdrKind::CorelDraw);
        check(&crate::cdr::load_from_bytes(&file).unwrap());
    }

    #[test]
    fn curves_close_and_bend() {
        let pts = [(0.0, 0.0), (1.0, 1.0), (2.0, 1.0), (3.0, 0.0), (3.0, -1.0)].map(|(x, y)| Point::new(x, y));
        let bp = curve(&pts, &[0x00, 0xc0, 0xc0, 0x80, 0x48]);
        let els = bp.elements();
        assert!(matches!(els[1], kurbo::PathEl::CurveTo(..)));
        assert!(matches!(els[2], kurbo::PathEl::LineTo(..)));
        assert!(matches!(els[3], kurbo::PathEl::ClosePath));
    }

    #[test]
    fn damaged_files_fail_cleanly() {
        for file in [sample(false, false), sample(true, true), packed(&sample(false, true))] {
            for cut in 0..file.len() {
                let _ = load_from_bytes(&file[..cut]);
            }
            let mut seed = 0x2545_f491u32;
            for _ in 0..300 {
                let mut b = file.clone();
                for _ in 0..8 {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    let i = 12 + seed as usize % (b.len() - 12);
                    b[i] = (seed >> 8) as u8;
                }
                let _ = load_from_bytes(&b);
            }
        }
        assert!(load_from_bytes(b"RIFF\0\0\0\0CMX1").is_err());
    }

    /// `ANYVECTOR_CMX_DIR=<folder> cargo test -p vector-io --lib cmx_samples -- --ignored --nocapture`:
    /// every `.cmx` or `.cdr` metafile in the folder must open; with `ANYVECTOR_CMX_PNG` also
    /// writes `<file>.ours.png` and, from the preview the file carries, `<file>.preview.png`.
    #[test]
    #[ignore]
    fn cmx_samples() {
        let Ok(dir) = std::env::var("ANYVECTOR_CMX_DIR") else { return };
        let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| matches!(crate::extension(p).as_str(), "cdr" | "cmx")).collect();
        files.sort();
        let (mut opened, mut failed) = (0, vec![]);
        let png = std::env::var("ANYVECTOR_CMX_PNG").is_ok();
        for path in &files {
            let bytes = std::fs::read(path).unwrap();
            if !is_cmx(&bytes) {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            match std::panic::catch_unwind(|| load_with_report(&bytes)) {
                Ok(Ok((doc, report))) => {
                    opened += 1;
                    let mut n = 0;
                    for o in doc.pages.iter().flat_map(|p| p.all_objects()) {
                        o.walk(&mut |_| n += 1);
                    }
                    if n == 0 || !report.notes.is_empty() {
                        println!("{name}: {n} objects {:?}", report.lines());
                    }
                    if png {
                        let opts = crate::image_io::RasterOptions { dpi: 72.0, ..Default::default() };
                        let _ = crate::image_io::export_raster(&doc, 0, &path.with_extension("ours.png"), &opts);
                        if let Some((w, h, rgba)) = thumbnail(&bytes) {
                            let _ = image::RgbaImage::from_raw(w, h, rgba).map(|i| i.save(path.with_extension("preview.png")));
                        }
                    }
                }
                Ok(Err(e)) => failed.push(format!("{name}: {e}")),
                Err(_) => failed.push(format!("{name}: PANIC")),
            }
        }
        println!("{opened} metafile(s) opened, {} failed", failed.len());
        assert!(failed.is_empty(), "{failed:#?}");
    }
}
