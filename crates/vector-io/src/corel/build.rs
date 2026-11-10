//! Turns the chunk tree of a CorelDRAW drawing into an AnyVector document.

use super::color;
use super::persp;
use super::riff::{Chunk, Rd};
use super::text;
use crate::{IoError, Result};
use kurbo::{Affine, BezPath, Point, Shape};
use std::collections::HashMap;
use vector_core::color::Color;
use vector_core::document::{Document, Guide, GuideKind, Layer, Page};
use vector_core::object::{BlendObj, CornerType, DropShadow, ExtrudeEffect, EllipseKind, EllipseShape, ObjKind, Object, RectShape};
use vector_core::path::NodePath;
use vector_core::style::*;

// loda argument types
const ARG_OUTLINE: u32 = 0x0a;
const ARG_FILL: u32 = 0x14;
const ARG_COORDS: u32 = 0x1e;
pub(super) const ARG_STYLE: u32 = 0xc8;
pub(super) const ARG_STYLE_JSON: u32 = 0xc9;
const ARG_NAME: u32 = 0x3e8;
const ARG_LENS: u32 = 0x1f40;
const ARG_POLYGON: u32 = 0x2af8;
const ARG_PAGE_SIZE: u32 = 0x4aba;
const ARG_POWERCLIP: u32 = 0x1f45;
const ARG_CLIP_ID: u32 = 0xdac;
/// Blend and contour settings, on their link group.
const ARG_BLEND: u32 = 0xbb8;
/// Points of blur per unit of CorelDRAW's shadow feathering.
const SHADOW_FEATHER: f64 = 0.5;

// loda object types
const T_RECT: u32 = 1;
const T_ELLIPSE: u32 = 2;
const T_CURVE: u32 = 3;
const T_ARTISTIC: u32 = 4;
const T_BITMAP: u32 = 5;
const T_PARAGRAPH: u32 = 6;
const T_GRID_LAYER: u32 = 0x0b;
const T_GUIDES_LAYER: u32 = 0x0c;
const T_DESKTOP_LAYER: u32 = 0x11;
const T_POLYGON: u32 = 0x14;
const T_GUIDE: u32 = 0x16;
const ARG_GUIDE: u32 = 0x3e80;
const T_PATH: u32 = 0x25;
const T_POWERLINE4: u32 = 0x0e;

/// A fill as stored, before it is fitted to an object.
#[derive(Clone, Debug)]
pub(super) enum RawFill {
    #[allow(dead_code)]
    None,
    Solid(Color),
    /// kind, angle (degrees), center offset (fractions of the box), edge pad (0..0.5), stops
    Gradient { kind: GradientKind, angle: f64, cx: f64, cy: f64, pad: f64, stops: Vec<GradientStop> },
    /// A bitmap, full-color or texture fill: the bitmap id and the tile width in points.
    Image { bitmap: u32, tile: f64, angle: f64 },
    /// A two-color pattern: the pattern's id, the tile width in points, and the colors of its
    /// 0 bits (foreground) and 1 bits (background).
    TwoColor { pattern: u32, tile: f64, fg: Color, bg: Color },
    /// A PostScript fill: its number in CorelDRAW's library and its settings.
    PostScript { number: u16, params: Vec<i32> },
    /// A CorelDRAW 4 vector pattern: the `spnd` of the group in `vect` that draws the tile, and
    /// the tile's width and height in points.
    Vector { pattern: u32, w: f64, h: f64 },
}

pub(super) struct Ctx<'a> {
    /// File version times 100 (X5 = 1500).
    pub ver: u32,
    /// Points per file unit.
    pub k: f64,
    pub fills: HashMap<u32, RawFill>,
    pub outlines: HashMap<u32, Outline>,
    pub fonts: HashMap<u32, String>,
    /// Font id to the Windows character set of its one-byte text.
    pub charsets: HashMap<u32, u16>,
    /// Fonts whose character set comes from their name ("Arial CYR").
    pub named_charsets: std::collections::HashSet<u32>,
    pub styles: text::Styles,
    /// Bitmap id to the document's image id and its pixel size.
    pub images: HashMap<u32, (u64, u32, u32)>,
    /// PowerClip contents: the groups of the clip table, by id.
    pub clips: HashMap<u32, &'a Chunk>,
    /// CorelDRAW 4 vector patterns: the group that draws each, by its `spnd`.
    pub vects: HashMap<u32, &'a Chunk>,
    /// The same groups by the frame that owns them (their `spnd`).
    pub clip_owners: HashMap<u32, &'a Chunk>,
    /// Versions 5 and 6: style id to the (fill, outline) ids its objects take.
    pub graphic_styles: HashMap<u32, (u32, u32)>,
    /// Arrowheads: the drawing's id to the document's custom arrowhead.
    pub arrows: HashMap<u32, u64>,
    /// Two-color patterns (`bmpf`): id to width, height and one bit per pixel, top row first.
    pub patterns: HashMap<u32, (usize, usize, Vec<bool>)>,
    /// Object types that were not converted, with how many of each.
    pub skipped: std::collections::BTreeMap<u32, usize>,
    /// Text fitted to a path (`lnkt`): the `spnd` ids of the two objects, in either order.
    pub text_paths: Vec<(u32, u32)>,
    /// Where each text fitted to a path sits on it, by the text's id.
    pub path_places: HashMap<u64, text::PathPlace>,
    /// Converted objects by their `spnd` id.
    pub by_spnd: HashMap<u32, u64>,
    /// Perspectives of the groups being converted, innermost last.
    pub warps: Vec<persp::Warp>,
    /// CorelDRAW 4 paragraph texts (`btxt`) by their id: each paragraph's style and characters.
    pub texts4: HashMap<u32, Vec<(u32, Vec<u16>)>>,
    /// The text whose records are being read.
    pub text4: Option<u32>,
    /// CorelDRAW 4 paragraph text frames (`lnkt`): the text's id, then the `spnd` ids of the
    /// frames it flows through.
    pub chains4: Vec<(u32, Vec<u32>)>,
    pub doc: Document,
}

/// An object's `spnd` id: 16 bits before CorelDRAW 6, 32 from then on.
pub(super) fn spnd(b: &[u8]) -> u32 {
    if b.len() < 4 {
        Rd::new(b).u16() as u32
    } else {
        Rd::new(b).u32()
    }
}

pub(super) struct Loda<'a> {
    pub kind: u32,
    pub args: Vec<(u32, &'a [u8])>,
}

impl<'a> Loda<'a> {
    pub fn arg(&self, t: u32) -> Option<&'a [u8]> {
        self.args.iter().find(|a| a.0 == t).map(|a| a.1)
    }
}

impl<'a> Ctx<'a> {
    /// Versions before 6 store sizes, counts and coordinates in 16 bits.
    pub fn wide(&self) -> bool {
        self.ver >= 600
    }
    pub fn uint(&self, r: &mut Rd) -> u32 {
        if self.wide() {
            r.u32()
        } else {
            r.u16() as u32
        }
    }
    pub fn int(&self, r: &mut Rd) -> i32 {
        if self.wide() {
            r.i32()
        } else {
            r.i16() as i32
        }
    }
    /// A coordinate in file units.
    pub fn coord(&self, r: &mut Rd) -> f64 {
        self.int(r) as f64
    }
    /// An angle in degrees.
    pub fn angle(&self, r: &mut Rd) -> f64 {
        if self.wide() {
            r.i32() as f64 / 1_000_000.0
        } else {
            r.i16() as f64 / 10.0
        }
    }

    /// Object names are UTF-16 from version 12 on, one byte per character before.
    pub fn string(&self, b: &[u8]) -> String {
        if self.ver >= 1200 {
            let u: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|c| *c != 0).collect();
            String::from_utf16_lossy(&u)
        } else {
            b.iter().take_while(|c| **c != 0).map(|c| text::cp1252(*c)).collect()
        }
    }

    /// The argument table shared by `loda` and `trfd` records: (object type, arguments).
    pub fn loda<'b>(&self, b: &'b [u8]) -> Loda<'b> {
        let mut r = Rd::new(b);
        let len = (self.uint(&mut r) as usize).min(b.len());
        let n = self.uint(&mut r) as usize;
        let start_args = self.uint(&mut r) as usize;
        let start_types = self.uint(&mut r) as usize;
        let mut kind = self.uint(&mut r);
        // CorelDRAW 3 numbers its object types from 0: rectangle, ellipse, curve, artistic
        // text, bitmap, paragraph text
        if self.ver < 400 && kind <= 5 {
            kind += 1;
        }
        let w = if self.wide() { 4 } else { 2 };
        let mut args = vec![];
        if n > b.len() / w {
            return Loda { kind, args };
        }
        let offsets: Vec<usize> = (0..n).map(|i| self.uint(&mut Rd::at(b, start_args + i * w)) as usize).collect();
        for i in 0..n {
            let t = self.uint(&mut Rd::at(b, start_types + (n - 1 - i) * w));
            let s = offsets[i].min(len);
            let e = offsets.get(i + 1).copied().unwrap_or(len).clamp(s, len);
            args.push((t, &b[s..e]));
        }
        Loda { kind, args }
    }

    /// A 16.16 fixed-point matrix with whole-unit offsets: a, b, x0, c, d, y0.
    fn fixed_matrix(r: &mut Rd) -> Affine {
        let fixed = |r: &mut Rd| r.i32() as f64 / 65536.0;
        let v0 = fixed(r);
        let v1 = fixed(r);
        let x0 = r.i32() as f64;
        let v3 = fixed(r);
        let v4 = fixed(r);
        let y0 = r.i32() as f64;
        Affine::new([v0, v3, v1, v4, x0, y0])
    }

    /// CorelDRAW 3 keeps an object's transform in its own record (argument 0x64): a header
    /// whose word at 0x0a points at the matrix.
    fn transform3(&self, l: &Loda) -> Option<Affine> {
        let a = l.arg(0x64)?;
        let off = Rd::at(a, 0x0a).u16() as usize;
        (off + 24 <= a.len()).then(|| Self::fixed_matrix(&mut Rd::at(a, off)))
    }

    /// The transform of a `trfl` list, in file units: the product of its records.
    fn transform(&self, lgob: &Chunk) -> Affine {
        let mut t = Affine::IDENTITY;
        let Some(trfl) = lgob.list_child(b"trfl") else { return t };
        for c in trfl.children.iter().filter(|c| c.is(b"trfd")) {
            let b = &c.data[..];
            let mut r = Rd::new(b);
            let _len = self.uint(&mut r);
            let n = self.uint(&mut r) as usize;
            let start_args = self.uint(&mut r) as usize;
            let w = if self.wide() { 4 } else { 2 };
            for i in 0..n.min(64) {
                let off = self.uint(&mut Rd::at(b, start_args + i * w)) as usize;
                let mut r = Rd::at(b, off);
                if self.ver >= 1300 {
                    r.skip(8);
                }
                if r.u16() != 0x08 {
                    continue;
                }
                // doubles from version 5 (after six more bytes from version 6), 16.16 fixed
                // point before
                let m = if self.ver >= 500 {
                    if self.ver >= 600 {
                        r.skip(6);
                    }
                    let v: [f64; 6] = std::array::from_fn(|_| r.f64());
                    Affine::new([v[0], v[3], v[1], v[4], v[2], v[5]])
                } else {
                    Self::fixed_matrix(&mut r)
                };
                t = m * t;
            }
        }
        t
    }

    /// The perspective of a `trfl` list (transform type 2): the box the object had in its
    /// parent's space, and the four corners it is drawn to.
    fn perspective(&self, lgob: &Chunk) -> Option<persp::Persp> {
        let trfl = lgob.list_child(b"trfl")?;
        for c in trfl.children.iter().filter(|c| c.is(b"trfd")) {
            let b = &c.data[..];
            let mut r = Rd::new(b);
            let _len = self.uint(&mut r);
            let n = self.uint(&mut r) as usize;
            let start_args = self.uint(&mut r) as usize;
            let w = if self.wide() { 4 } else { 2 };
            for i in 0..n.min(64) {
                let off = self.uint(&mut Rd::at(b, start_args + i * w)) as usize;
                let mut r = Rd::at(b, off);
                if self.ver >= 1300 {
                    r.skip(8);
                }
                if r.u16() != 0x02 {
                    continue;
                }
                // CorelDRAW 4 has two bytes fewer before the box
                r.skip(if self.wide() {
                    10
                } else if self.ver < 500 {
                    4
                } else {
                    6
                });
                let l = self.coord(&mut r);
                let t = self.coord(&mut r);
                let rt = self.coord(&mut r);
                let bt = self.coord(&mut r);
                let quad: [kurbo::Point; 4] = std::array::from_fn(|_| {
                    let x = self.coord(&mut r);
                    kurbo::Point::new(x, self.coord(&mut r))
                });
                return persp::Persp::new(l, t, rt, bt, quad);
            }
        }
        None
    }

    /// The outline of an envelope (an `envd` record next to the transform), in the space of the
    /// object's parent: its node count, then the node types and the points at the end.
    fn envelope_outline(&self, lgob: &Chunk) -> Option<BezPath> {
        let d = &lgob.list_child(b"trfl")?.child(b"envd")?.data;
        let n = Rd::at(d, 0x10).u32() as usize;
        let w = if self.wide() { 8 } else { 4 };
        let need = n.checked_mul(w + 1)?;
        if n < 4 || need > d.len() {
            return None;
        }
        let points = &d[d.len() - n * w..];
        let types = &d[d.len() - need..d.len() - n * w];
        // as a curve record: count, two spare bytes, the points, the types
        let mut b = (n as u16).to_le_bytes().to_vec();
        b.extend([0, 0]);
        b.extend(points);
        b.extend(types);
        let p = self.curve(&b);
        (p.elements().len() > 4).then_some(p)
    }

    /// An extrusion (transform record type 4, CorelDRAW X3 and later): the vanishing point in
    /// file units, the far face's size in percent, flags and the colors. CorelDRAW keeps only
    /// the geometry: "big back" is saved as "small front" and "big front" as "small back".
    fn extrude(&self, lgob: &Chunk, to_page: Affine, o: &Object) -> Option<ExtrudeEffect> {
        if self.ver < 1300 {
            return None;
        }
        let trfl = lgob.list_child(b"trfl")?;
        for c in trfl.children.iter().filter(|c| c.is(b"trfd")) {
            let b = &c.data[..];
            let mut r = Rd::new(b);
            let _len = r.u32();
            let n = r.u32() as usize;
            let start_args = r.u32() as usize;
            for i in 0..n.min(64) {
                let off = Rd::at(b, start_args + i * 4).u32() as usize;
                if off + 0xf0 > b.len() || Rd::at(b, off + 8).u16() != 4 {
                    continue;
                }
                let mut r = Rd::at(b, off + 0x54);
                let vp = kurbo::Point::new(r.i32() as f64, r.i32() as f64);
                let far = Rd::at(b, off + 0x60).u16() as f64 / 100.0;
                let flags = b[off + 0xea];
                let color = color::read(&mut Rd::at(b, off + 0x6e), self.ver);
                let shade = color::read(&mut Rd::at(b, off + 0xc4), self.ver);
                let perspective = flags & 1 != 0;
                let front = flags & 0x20 != 0;
                let c = o.world_path().bounding_box().center();
                let to_vp = to_page * vp - c;
                let (shift, scale) = if perspective { (to_vp * (1.0 - far), far) } else { (to_vp, 1.0) };
                if !(0.01..=10.0).contains(&scale) {
                    return None;
                }
                let kind = match (perspective, front) {
                    (true, false) => 0,
                    (true, true) => 1,
                    (false, false) => 4,
                    (false, true) => 5,
                };
                let color_mode = if flags & 4 != 0 { 2 } else if flags & 2 != 0 { 1 } else { 0 };
                return Some(ExtrudeEffect { depth: if front { -shift } else { shift }, color, shade, kind, color_mode, scale: Some(scale), ..Default::default() });
            }
        }
        None
    }

    /// A blend's link group (type 7) whose steps are not stored: the number of steps.
    fn blend_steps(&self, c: &Chunk) -> Option<u32> {
        if !c.is_list(b"lnkg") || c.children.iter().any(|k| k.list.is_some() && !k.is_list(b"lgob")) {
            return None;
        }
        let l = self.loda(&Self::record(c)?.data);
        if l.kind != 7 {
            return None;
        }
        let steps = Rd::new(l.arg(ARG_BLEND)?).u32();
        (1..=999).contains(&steps).then_some(steps)
    }

    /// A drop shadow's link group (type 23, seen in CorelDRAW 2026 files): the shadow of the
    /// object above it, with its offset in file units.
    fn shadow(&self, c: &Chunk) -> Option<(kurbo::Vec2, DropShadow)> {
        if !c.is_list(b"lnkg") || c.children.iter().any(|k| k.list.is_some() && !k.is_list(b"lgob")) {
            return None;
        }
        let l = self.loda(&Self::record(c)?.data);
        if l.kind != 23 {
            return None;
        }
        let d = l.arg(ARG_BLEND)?;
        if d.len() < 0x49 {
            return None;
        }
        let mut r = Rd::at(d, 8);
        let (dx, dy) = (r.i32() as f64, r.i32() as f64);
        let opacity = (r.u32() as f64 / 2.5).clamp(0.0, 100.0);
        let feather = r.u32() as f64;
        let color = color::read(&mut Rd::at(d, 0x3d), self.ver);
        let s = DropShadow { feather, color, opacity, ..Default::default() };
        Some((kurbo::Vec2::new(dx, dy), s))
    }

    /// Convert the objects of a list, bottom first. A blend comes as its start object, a link
    /// group and its end object; the steps between are made by AnyVector's live blend.
    fn children(&mut self, list: &Chunk, to_page: Affine, parent: Affine, out: &mut Vec<Object>) {
        let kids: Vec<&Chunk> = list.children.iter().rev().collect();
        // where the object made by the previous chunk is, when it made exactly one
        let mut single: Option<usize> = None;
        let mut i = 0;
        while i < kids.len() {
            if let (Some(steps), Some(si), Some(next)) = (self.blend_steps(kids[i]), single, kids.get(i + 1)) {
                if si + 1 == out.len() {
                    let mut end = vec![];
                    self.object(next, to_page, parent, &mut end);
                    if end.len() == 1 {
                        let (start, end) = (out.pop().unwrap_or_else(|| end[0].clone()), end.remove(0));
                        let b = BlendObj { start: Box::new(start), end: Box::new(end), steps, ..Default::default() };
                        out.push(Object::new(self.doc.new_id(), ObjKind::Blend(b), Affine::IDENTITY, Style::default()));
                    } else {
                        out.extend(end);
                    }
                    single = None;
                    i += 2;
                    continue;
                }
            }
            if let (Some((v, mut s)), Some(next)) = (self.shadow(kids[i]), kids.get(i + 1)) {
                let before = out.len();
                self.object(next, to_page, parent, out);
                if out.len() == before + 1 {
                    let m = to_page;
                    s.offset = m * kurbo::Point::new(v.x, v.y) - m * kurbo::Point::ORIGIN;
                    s.feather *= SHADOW_FEATHER;
                    out[before].effects.drop_shadow = Some(s);
                }
                single = (out.len() == before + 1).then_some(before);
                i += 2;
                continue;
            }
            let before = out.len();
            self.object(kids[i], to_page, parent, out);
            single = (out.len() == before + 1).then_some(before);
            i += 1;
        }
    }

    // ---------------------------------------------------------------- document resources

    fn collect(&mut self, c: &'a Chunk) {
        if c.list.is_some() {
            if c.is_list(b"page") {
                return;
            }
            if c.is_list(b"vect") {
                for g in c.children.iter().filter(|k| k.is_list(b"grp ")) {
                    if let Some(id) = g.child(b"spnd").map(|s| spnd(&s.data)) {
                        self.vects.insert(id, g);
                    }
                }
                return;
            }
            if c.is_list(b"clpt") {
                for k in c.children.iter().filter(|k| k.list.is_some()) {
                    let id = Self::record(k).and_then(|l| self.loda(&l.data).arg(ARG_CLIP_ID).map(|a| Rd::new(a).u32()));
                    if let Some(id) = id {
                        self.clips.insert(id, k);
                    }
                    // the group's `spnd` names the frame it belongs to
                    if let Some(owner) = k.child(b"spnd").map(|s| Rd::new(&s.data).u32()) {
                        self.clip_owners.insert(owner, k);
                    }
                }
                return;
            }
            for k in &c.children {
                self.collect(k);
            }
            return;
        }
        match &c.tag {
            b"fild" | b"fill" => self.fild(&c.data),
            b"arrx" | b"arrw" => self.arrow(&c.tag, &c.data),
            b"outl" => self.outl(&c.data),
            b"font" => text::font(self, &c.data),
            b"ftbl" => text::ftbl(self, &c.data),
            b"stlt" => text::stlt(self, &c.data),
            b"bmp " => super::bitmap::bitmap(self, &c.data),
            b"tiff" => super::bitmap::tiff(self, &c.data),
            b"bmpf" => self.pattern(&c.data),
            b"lnkt" if self.ver < 500 => self.links4(&c.data),
            b"lnkt" => self.links(&c.data),
            // CorelDRAW 4 paragraph text: the text's id, then per paragraph its style and count,
            // and the characters (plain ones 4 bytes each, styled ones 24 bytes each)
            b"btid" => {
                let id = Rd::new(&c.data).u16() as u32;
                self.texts4.insert(id, vec![]);
                self.text4 = Some(id);
            }
            b"para" => {
                if let Some(t) = self.text4.and_then(|id| self.texts4.get_mut(&id)) {
                    t.push((Rd::new(&c.data).u16() as u32, vec![]));
                }
            }
            b"bnch" | b"bsch" => {
                let step = if &c.tag == b"bnch" { 4 } else { 24 };
                if let Some((_, chars)) = self.text4.and_then(|id| self.texts4.get_mut(&id)).and_then(|t| t.last_mut()) {
                    chars.extend(c.data.chunks(step).filter(|r| r.len() >= 4).map(|r| u16::from_le_bytes([r[2], r[3]])));
                }
            }
            _ => {}
        }
    }

    /// The link table: its length, the number of links, where their offsets start, then each link
    /// as its kind and the `spnd` ids of the objects it joins. Kind 0x40 is text fitted to a path.
    fn links(&mut self, b: &[u8]) {
        let n = Rd::at(b, 4).u32() as usize;
        let start = Rd::at(b, 8).u32() as usize;
        for i in 0..n.min(10_000) {
            let at = start + 4 * i;
            if at + 4 > b.len() {
                break;
            }
            let off = Rd::at(b, at).u32() as usize;
            if off + 12 > b.len() {
                continue;
            }
            let mut r = Rd::at(b, off);
            if r.u32() == 0x40 {
                let (a, z) = (r.u32(), r.u32());
                self.text_paths.push((a, z));
            }
        }
    }

    /// CorelDRAW 4 links (`lnkt`): their count and where each starts; a link is its kind, its count of
    /// objects, ten spare bytes, then for paragraph text (kind 0x61) the text's id and its frames.
    fn links4(&mut self, b: &[u8]) {
        let n = Rd::at(b, 2).u16() as usize;
        let start = Rd::at(b, 4).u16() as usize;
        for i in 0..n.min(10_000) {
            let off = Rd::at(b, start + 2 * i).u16() as usize;
            let mut r = Rd::at(b, off);
            if r.u16() != 0x61 {
                continue;
            }
            let count = (r.u16() as usize).min(1000);
            r.skip(10);
            let text = r.u16() as u32;
            let frames: Vec<u32> = (0..count).map(|_| r.u16() as u32).filter(|f| *f != 0).collect();
            if !frames.is_empty() {
                self.chains4.push((text, frames));
            }
        }
    }

    /// From version 13 on a record body is a list of (id, size, data) properties; find one.
    fn property<'b>(&self, r: &mut Rd<'b>, id: u32) -> Option<Rd<'b>> {
        for _ in 0..64 {
            if r.left() < 8 {
                return None;
            }
            let pid = r.u32();
            let len = r.u32() as usize;
            if pid == id {
                let s = r.p.min(r.b.len());
                let e = if len == 0 { r.b.len() } else { s.saturating_add(len).min(r.b.len()) };
                return Some(Rd::new(&r.b[s..e]));
            }
            r.skip(len);
        }
        None
    }

    fn fild(&mut self, b: &[u8]) {
        let mut r = Rd::new(b);
        let id = r.u32();
        let mut new_stops = false;
        if self.ver >= 1300 {
            // the fill is the property tagged with the structure version (1300)
            let mut probe = r.clone();
            match self.property(&mut probe, 0x514) {
                Some(body) => {
                    new_stops = matches!(body.b.len() & 0xffff, 0x9e | 0xac) || self.ver >= 1500;
                    r = body;
                }
                None => r.skip(8),
            }
        }
        let fill = match r.u16() {
            1 => {
                r.skip(if self.ver >= 1300 { 13 } else { 2 });
                RawFill::Solid(color::read(&mut r, self.ver))
            }
            2 => self.gradient(&mut r, new_stops),
            0 => RawFill::None,
            // pattern, texture and PostScript fills: shown as a neutral tone until supported
            7 => {
                r.skip(if self.ver >= 1300 { 8 } else { 2 });
                self.two_color(&mut r)
            }
            // bitmap, vector pattern and texture fills: a list of properties; the one tagged
            // 1300 holds the tile size and, at 0x20, the bitmap CorelDRAW rendered the tile into
            9 | 10 | 11 if self.ver >= 1300 => {
                let mut list = r.clone();
                let body = self.property(&mut list, 0x514).and_then(|mut o| self.property(&mut o, 0x514));
                match body {
                    Some(d) if d.b.len() >= 0x24 => {
                        let tile = Rd::at(d.b, 0).i32().abs() as f64 * self.k;
                        RawFill::Image { bitmap: Rd::at(d.b, 0x20).u32(), tile, angle: 0.0 }
                    }
                    _ => RawFill::Solid(Color::rgb(200, 200, 200)),
                }
            }
            // a PostScript fill: its number in CorelDRAW's library and its settings
            6 => {
                if self.ver >= 1300 {
                    r.skip(8);
                } else {
                    r.skip(2);
                }
                let number = super::psfill::number(r.u16());
                let n = (r.u16() as usize).min(5);
                let params = (0..n).map(|_| r.i16() as i32).collect();
                RawFill::PostScript { number, params }
            }
            // bitmap fills of CorelDRAW 6 to 12: the pattern's id, the tile's size, its offsets
            // and flags, then the bitmap
            9 if self.ver >= 600 => {
                r.skip(2);
                let _pattern = r.u32();
                let tile = r.i32().abs() as f64 * self.k;
                r.skip(4 + 4 + 2 + 1 + 21);
                RawFill::Image { bitmap: r.u32(), tile, angle: 0.0 }
            }
            // CorelDRAW 4 vector patterns: the group that draws the tile, and the tile's size
            10 if self.ver < 500 => {
                r.skip(2);
                let pattern = r.u16() as u32;
                let w = self.coord(&mut r).abs() * self.k;
                let h = self.coord(&mut r).abs() * self.k;
                RawFill::Vector { pattern, w, h }
            }
            // vector pattern and texture fills before X3 keep no bitmap (CorelDRAW draws a
            // texture from its settings): shown as a neutral tone
            _ => RawFill::Solid(Color::rgb(200, 200, 200)),
        };
        self.fills.insert(id, fill);
    }

    /// A two-color pattern: its id, the tile size and offsets, then the two colors.
    fn two_color(&self, r: &mut Rd) -> RawFill {
        let pattern = r.u32();
        let tile = self.coord(r).abs() * self.k;
        self.coord(r); // tile height
        if self.ver >= 1300 {
            // from X3 each color comes after a 01 and its length (12)
            let color_after = |r: &mut Rd| {
                let from = r.p;
                let at = (from..from + 40).find(|&i| r.b.get(i..i + 5) == Some(&[1, 12, 0, 0, 0][..]))?;
                r.p = at + 5;
                let c = color::read(r, self.ver);
                r.p = at + 5 + 12;
                Some(c)
            };
            let (Some(fg), Some(bg)) = (color_after(r), color_after(r)) else { return RawFill::Solid(Color::rgb(200, 200, 200)) };
            return RawFill::TwoColor { pattern, tile, fg, bg };
        }
        // the tile's offsets
        r.skip(if self.wide() { 8 } else { 4 });
        let fg = color::read(r, self.ver);
        let bg = color::read(r, self.ver);
        RawFill::TwoColor { pattern, tile, fg, bg }
    }

    /// A two-color pattern's bits (`bmpf`): its id, then a one-bit Windows bitmap (header,
    /// two palette entries, rows bottom up).
    fn pattern(&mut self, b: &[u8]) {
        let mut r = Rd::new(b);
        let id = r.u32();
        let head = r.u32() as usize;
        let (w, h) = (r.u32() as usize, r.u32() as usize);
        r.skip(2);
        let bpp = r.u16();
        if bpp != 1 || !(1..=1024).contains(&w) || !(1..=1024).contains(&h) {
            return;
        }
        let stride = w.div_ceil(32) * 4;
        let start = 4 + head + 8;
        if start + stride * h > b.len() {
            return;
        }
        let mut bits = Vec::with_capacity(w * h);
        for y in 0..h {
            let row = &b[start + (h - 1 - y) * stride..];
            bits.extend((0..w).map(|x| row[x / 8] & (0x80 >> (x % 8)) != 0));
        }
        self.patterns.insert(id, (w, h, bits));
    }

    /// Give the two-color pattern and PostScript fills their tile image.
    pub(super) fn pattern_images(&mut self) {
        let mut made: HashMap<(u32, [u8; 4], [u8; 4]), u32> = HashMap::new();
        let mut next = 0xff00_0000u32;
        let ids: Vec<u32> = self.fills.keys().copied().collect();
        for id in ids {
            if let Some(RawFill::PostScript { number, params }) = self.fills.get(&id) {
                let fill = match super::psfill::draw(*number, params) {
                    super::psfill::Drawn::Tone(c) => RawFill::Solid(c),
                    super::psfill::Drawn::Tile { rgba, w, h, width, angle } => {
                        let encoded = crate::image_io::encode_png(w, h, &rgba);
                        let data = vector_core::document::ImageData { width: w, height: h, rgba: std::sync::Arc::new(rgba), encoded: std::sync::Arc::new(encoded), format: "png".into(), name: format!("PostScript fill {number}") };
                        let iid = self.doc.add_image(data);
                        next += 1;
                        self.images.insert(next, (iid, w, h));
                        RawFill::Image { bitmap: next, tile: width, angle }
                    }
                };
                self.fills.insert(id, fill);
                continue;
            }
            if let Some(RawFill::Vector { pattern, w, h }) = self.fills.get(&id).cloned() {
                let fill = match self.vector_tile(pattern, w, h) {
                    Some((rgba, pw, ph)) => {
                        let encoded = crate::image_io::encode_png(pw, ph, &rgba);
                        let data = vector_core::document::ImageData { width: pw, height: ph, rgba: std::sync::Arc::new(rgba), encoded: std::sync::Arc::new(encoded), format: "png".into(), name: format!("vector pattern {pattern}") };
                        let iid = self.doc.add_image(data);
                        next += 1;
                        self.images.insert(next, (iid, pw, ph));
                        RawFill::Image { bitmap: next, tile: w, angle: 0.0 }
                    }
                    None => RawFill::Solid(Color::rgb(200, 200, 200)),
                };
                self.fills.insert(id, fill);
                continue;
            }
            let Some(RawFill::TwoColor { pattern, tile, fg, bg }) = self.fills.get(&id).cloned() else { continue };
            let Some((w, h, bits)) = self.patterns.get(&pattern) else {
                self.fills.insert(id, RawFill::Solid(Color::lerp(fg, bg, 0.5)));
                continue;
            };
            let key = (pattern, [fg.r, fg.g, fg.b, fg.a], [bg.r, bg.g, bg.b, bg.a]);
            let bitmap = match made.get(&key) {
                Some(b) => *b,
                None => {
                    let rgba: Vec<u8> = bits.iter().flat_map(|&one| { let c = if one { bg } else { fg }; [c.r, c.g, c.b, c.a] }).collect();
                    let (w, h) = (*w as u32, *h as u32);
                    let encoded = crate::image_io::encode_png(w, h, &rgba);
                    let data = vector_core::document::ImageData { width: w, height: h, rgba: std::sync::Arc::new(rgba), encoded: std::sync::Arc::new(encoded), format: "png".into(), name: format!("pattern{pattern}") };
                    let iid = self.doc.add_image(data);
                    next += 1;
                    self.images.insert(next, (iid, w, h));
                    made.insert(key, next);
                    next
                }
            };
            self.fills.insert(id, RawFill::Image { bitmap, tile: if tile > 0.1 { tile } else { 36.0 }, angle: 0.0 });
        }
    }

    /// The tile of a CorelDRAW 4 vector pattern: the group's objects, fitted to a tile of `w` by
    /// `h` points, drawn into pixels (about 4 per point, at most 512 on a side).
    fn vector_tile(&mut self, pattern: u32, w: f64, h: f64) -> Option<(Vec<u8>, u32, u32)> {
        let group = *self.vects.get(&pattern)?;
        if w < 0.1 || h < 0.1 {
            return None;
        }
        let mut objects = vec![];
        let flip = Affine::scale_non_uniform(self.k, -self.k);
        for c in group.children.iter().filter(|c| c.list.is_some()) {
            self.object(c, flip, Affine::IDENTITY, &mut objects);
        }
        let b = objects.iter().map(|o| o.bounds()).reduce(|a, b| a.union(b))?;
        if b.width() <= 0.0 || b.height() <= 0.0 {
            return None;
        }
        let fit = Affine::scale_non_uniform(w / b.width(), h / b.height()) * Affine::translate((-b.x0, -b.y0));
        for o in &mut objects {
            o.apply_transform(fit);
        }
        let mut tile = Document::new(w, h);
        tile.images = self.doc.images.clone();
        if let Some(l) = tile.pages[0].layers.first_mut() {
            l.objects = objects;
        }
        let scale = (512.0 / w.max(h)).min(4.0);
        let size = ((w * scale).round().max(1.0) as u32, (h * scale).round().max(1.0) as u32);
        let opts = crate::image_io::RasterOptions { size: Some(size), background: Some(Color::rgb(255, 255, 255)), ..Default::default() };
        crate::image_io::render_rgba(&tile, 0, &opts).map(|(pw, ph, rgba)| (rgba, pw, ph))
    }

    fn gradient(&self, r: &mut Rd, new_stops: bool) -> RawFill {
        r.skip(if self.ver >= 1300 { 8 } else { 2 });
        let kind = match r.u8() {
            2 => GradientKind::Elliptical,
            3 => GradientKind::Conical,
            4 => GradientKind::Rectangular,
            _ => GradientKind::Linear,
        };
        let edge;
        if self.ver >= 1300 {
            r.skip(17);
            edge = r.i16() as f64;
        } else if self.ver >= 600 {
            r.skip(19);
            edge = r.i32() as f64;
        } else {
            r.skip(11);
            edge = r.i16() as f64;
        }
        let angle = self.angle(r);
        let cx = self.int(r) as f64;
        let cy = self.int(r) as f64;
        if self.ver >= 600 {
            r.skip(2);
        }
        let _mode = self.uint(r);
        let _midpoint = r.u8();
        r.skip(1);
        let n = (self.uint(r) & 0xffff) as usize;
        if self.ver >= 1300 {
            r.skip(3);
        }
        let mut stops = vec![];
        for _ in 0..n.min(512) {
            if r.eof() {
                break;
            }
            let c = color::read(r, self.ver);
            if self.ver >= 1300 {
                r.skip(if new_stops { 26 } else { 5 });
            }
            let pos = (self.uint(r) & 0xffff) as f64 / 100.0;
            if self.ver >= 1300 {
                r.skip(3);
            }
            stops.push(GradientStop { pos: pos.clamp(0.0, 1.0), color: c });
        }
        if stops.len() < 2 {
            return stops.first().map(|s| RawFill::Solid(s.color)).unwrap_or(RawFill::None);
        }
        RawFill::Gradient { kind, angle, cx: cx / 100.0, cy: cy / 100.0, pad: (edge / 100.0).clamp(0.0, 0.49), stops }
    }

    /// An arrowhead: its id, then its points. `arrx` (X5 and later) also has a name; its points
    /// run along the line, the tip at the line's end. `arrw` stores each point's y first.
    fn arrow(&mut self, tag: &[u8; 4], b: &[u8]) {
        let id = Rd::new(b).u32();
        let (mut r, name, spare) = if tag == b"arrx" {
            let u: Vec<u16> = b.get(0x0c..0x10c).unwrap_or_default().chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|c| *c != 0).collect();
            let mut r = Rd::at(b, 0x10c);
            let n = r.u16();
            r.skip(4);
            (r, (String::from_utf16_lossy(&u), n), false)
        } else {
            let mut r = Rd::at(b, 4);
            r.skip(if self.ver < 600 { 2 } else { 4 });
            let n = r.u16();
            r.skip(4);
            (r, (String::new(), n), true)
        };
        let n = name.1 as usize;
        let w = if self.wide() { 8 } else { 4 };
        if n < 2 || n > r.left() / (w + 1) {
            return;
        }
        let types = r.bytes(n).to_vec();
        // `arrw` has a spare byte after the node types
        if spare {
            r.skip(1);
        }
        // x along the line, the tip at the origin
        let pts: Vec<Point> = (0..n)
            .map(|_| {
                let x = self.coord(&mut r);
                Point::new(x, self.coord(&mut r))
            })
            .collect();
        // as a curve record
        let mut rec = (n as u16).to_le_bytes().to_vec();
        rec.extend([0, 0]);
        for p in &pts {
            for v in [p.x, p.y] {
                if self.wide() {
                    rec.extend((v as i32).to_le_bytes());
                } else {
                    rec.extend((v as i16).to_le_bytes());
                }
            }
        }
        rec.extend(&types);
        // a unit of the arrowhead is half an inch for an outline one point wide; AnyVector's
        // arrowheads are drawn at four times the outline width
        let unit = 4.0 * 36.0 / self.k;
        // (CorelDRAW's y axis points up, AnyVector's down)
        let path = Affine::scale_non_uniform(1.0 / unit, -1.0 / unit) * self.curve(&rec);
        if path.elements().len() < 2 {
            return;
        }
        let bb = path.bounding_box();
        let filled = path.elements().iter().any(|e| matches!(e, kurbo::PathEl::ClosePath));
        let aid = self.doc.new_id();
        let name = if name.0.is_empty() { format!("CorelDRAW arrowhead {id}") } else { name.0 };
        self.doc.arrowheads.push(CustomArrow { id: aid, name, path: path.to_svg(), width: bb.height(), filled });
        self.arrows.insert(id, aid);
    }

    fn outl(&mut self, b: &[u8]) {
        let mut r = Rd::new(b);
        let id = r.u32();
        if self.ver >= 1300 {
            let mut probe = r.clone();
            match self.property(&mut probe, 1) {
                Some(body) => r = body,
                None => return,
            }
        }
        let line_type = r.u16();
        let caps = r.u16();
        let join = r.u16();
        if self.ver < 1300 && self.ver >= 600 {
            r.skip(2);
        }
        let width = self.coord(&mut r) * self.k;
        let stretch = r.u16() as f64;
        if self.ver >= 600 {
            r.skip(2);
        }
        let nib = self.angle(&mut r);
        if self.ver >= 1300 {
            r.skip(46);
        } else if self.ver >= 600 {
            r.skip(52);
        }
        let c = color::read(&mut r, self.ver);
        r.skip(if self.ver < 600 { 10 } else { 16 });
        let n = r.u16() as usize;
        // the arrowheads come a fixed distance after the dash count (22 bytes in versions 6 to
        // X3, 20 before and after)
        let mut ra = Rd::at(r.b, r.p + if (600..1300).contains(&self.ver) { 22 } else { 20 });
        let arrow = |id: u32| self.arrows.get(&id).map(|a| ArrowHead::Custom(*a));
        let (start_arrow, end_arrow) = (arrow(ra.u32()), arrow(ra.u32()));
        let mut dash = vec![];
        if line_type & 0x04 != 0 {
            for _ in 0..n.min(32) {
                dash.push(r.u16() as f64);
            }
        }
        let mut o = Outline {
            width: width.max(0.0),
            color: Some(c),
            cap: match caps {
                1 => LineCap::Round,
                2 => LineCap::Square,
                _ => LineCap::Butt,
            },
            join: match join {
                1 => LineJoin::Round,
                2 => LineJoin::Bevel,
                _ => LineJoin::Miter,
            },
            dash,
            start_arrow,
            end_arrow,
            scale_with_object: line_type & 0x20 != 0,
            behind_fill: line_type & 0x10 != 0,
            ..Default::default()
        };
        if stretch > 0.0 && stretch <= 100.0 {
            o.stretch = stretch;
            o.nib_angle = nib;
        }
        if line_type & 0x01 != 0 {
            o.color = None;
        }
        self.outlines.insert(id, o);
    }

    /// A CorelDRAW 3 fill, stored in the object: its type, then colors or a gradient.
    fn fill3(&self, a: &[u8]) -> RawFill {
        let mut r = Rd::new(a);
        match r.u8() {
            0 => RawFill::None,
            1 => RawFill::Solid(color::read(&mut r, self.ver)),
            t @ (2 | 4) => {
                let angle = self.angle(&mut r);
                let c1 = color::read(&mut r, self.ver);
                let c2 = color::read(&mut r, self.ver);
                let (mut edge, mut cx, mut cy) = (0.0, 0.0, 0.0);
                if self.ver >= 200 {
                    r.skip(7);
                    edge = r.i16() as f64;
                    cx = self.int(&mut r) as f64;
                    cy = self.int(&mut r) as f64;
                }
                let kind = if t == 4 { GradientKind::Elliptical } else { GradientKind::Linear };
                let stops = vec![GradientStop { pos: 0.0, color: c1 }, GradientStop { pos: 1.0, color: c2 }];
                RawFill::Gradient { kind, angle, cx: cx / 100.0, cy: cy / 100.0, pad: (edge / 100.0).clamp(0.0, 0.49), stops }
            }
            // a two-color pattern: its id, tile size and offsets, then the two colors
            7 => {
                r.skip(if self.ver < 300 { 2 } else { 4 } + 4 + 6 + 1);
                let c1 = color::read(&mut r, self.ver);
                let c2 = color::read(&mut r, self.ver);
                RawFill::Solid(Color::lerp(c1, c2, 0.5))
            }
            _ => RawFill::Solid(Color::rgb(200, 200, 200)),
        }
    }

    /// A CorelDRAW 3 outline, stored in the object (type 0: none).
    fn outline3(&self, a: &[u8]) -> Outline {
        let mut r = Rd::new(a);
        let line_type = r.u8();
        if line_type == 0 {
            return Outline::none();
        }
        let width = self.coord(&mut r) * self.k;
        let stretch = r.u16() as f64;
        let nib = self.angle(&mut r);
        let c = color::read(&mut r, self.ver);
        r.skip(7);
        let n = r.u8() as usize;
        let dash_at = r.p;
        let dash: Vec<f64> = if line_type & 0x02 != 0 { (0..n.min(10)).map(|_| r.u8() as f64).collect() } else { vec![] };
        // CorelDRAW 1 has no dashes: the join and the caps (one byte per end) follow the
        // dash count's place; CorelDRAW 2 keeps a byte per end too, then the arrowheads
        let (join, caps, arrows) = match self.ver {
            100 => {
                let mut r = Rd::at(a, dash_at - 1);
                (r.u16(), r.u8() as u16, None)
            }
            200 => {
                let mut r = Rd::at(a, dash_at + 10);
                let (join, caps) = (r.u16(), r.u8() as u16);
                r.skip(1);
                (join, caps, Some((r.u16() as u32, r.u16() as u32)))
            }
            _ => {
                let mut r = Rd::at(a, dash_at + 10);
                (r.u16(), r.u16(), None)
            }
        };
        let arrow = |id: u32| self.arrows.get(&id).map(|a| ArrowHead::Custom(*a));
        let (start_arrow, end_arrow) = arrows.map(|(s, e)| (arrow(s), arrow(e))).unwrap_or_default();
        let mut o = Outline {
            width: width.max(0.0),
            color: Some(c),
            cap: match caps {
                1 => LineCap::Round,
                2 => LineCap::Square,
                _ => LineCap::Butt,
            },
            join: match join {
                1 => LineJoin::Round,
                2 => LineJoin::Bevel,
                _ => LineJoin::Miter,
            },
            dash,
            start_arrow,
            end_arrow,
            scale_with_object: line_type & 0x10 != 0,
            behind_fill: line_type & 0x08 != 0,
            ..Default::default()
        };
        if stretch > 0.0 && stretch <= 100.0 {
            o.stretch = stretch;
            o.nib_angle = nib;
        }
        o
    }

    // ---------------------------------------------------------------- pages

    fn default_page_size(&self, root: &Chunk) -> (f64, f64) {
        fn find<'a>(c: &'a Chunk, tag: &[u8; 4]) -> Option<&'a Chunk> {
            if c.is(tag) {
                return Some(c);
            }
            c.children.iter().find_map(|k| find(k, tag))
        }
        let Some(m) = find(root, b"mcfg") else { return (612.0, 792.0) };
        let off = if self.ver >= 1300 {
            12
        } else if self.ver >= 900 {
            4
        } else if self.ver < 700 && self.ver >= 600 {
            0x1c
        } else {
            0
        };
        let mut r = Rd::at(&m.data, off);
        // before version 4: the page's corners
        let (w, h) = if self.ver < 400 {
            r.skip(2);
            let (x0, y0, x1, y1) = (self.coord(&mut r), self.coord(&mut r), self.coord(&mut r), self.coord(&mut r));
            ((x1 - x0).abs() * self.k, (y1 - y0).abs() * self.k)
        } else {
            (self.coord(&mut r).abs() * self.k, self.coord(&mut r).abs() * self.k)
        };
        if w > 1.0 && h > 1.0 {
            (w, h)
        } else {
            (612.0, 792.0)
        }
    }

    /// The lower-left corner of the first page, in points from the origin, as the document
    /// settings give it (X5 and later). Pages are not always centered on the origin: recent
    /// versions put that corner on a whole pixel of a 300 dpi grid, and templates may start
    /// the page at the origin.
    fn origin(&self, root: &Chunk) -> Option<(f64, f64)> {
        if self.ver < 1500 {
            return None;
        }
        let m = root.list_child(b"doc ")?.child(b"mcfg")?;
        if m.data.len() < 678 {
            return None;
        }
        let mut r = Rd::at(&m.data, 670);
        let (x, y) = (r.i32() as f64 * self.k, r.i32() as f64 * self.k);
        // a corner further than a few pages away is not a corner
        (x.abs() < 20_000.0 && y.abs() < 20_000.0).then_some((x, y))
    }

    /// CorelDRAW 4 facing pages (byte 24 of the page setup), and whether the document starts on
    /// a left page (byte 26).
    fn facing4(&self, root: &Chunk) -> Option<bool> {
        if !(400..500).contains(&self.ver) {
            return None;
        }
        let m = root.list_child(b"doc ")?.child(b"mcfg")?;
        (m.data.len() >= 28 && m.data[24] == 1).then(|| m.data[26] == 1)
    }

    fn page_size(&self, page: &Chunk) -> Option<(f64, f64)> {
        for lgob in page.lists(b"lgob") {
            for c in lgob.children.iter().filter(|c| c.is(b"loda")) {
                if let Some(a) = self.loda(&c.data).arg(ARG_PAGE_SIZE) {
                    let mut r = Rd::new(a);
                    let w = self.coord(&mut r).abs() * self.k;
                    let h = self.coord(&mut r).abs() * self.k;
                    if w > 1.0 && h > 1.0 {
                        return Some((w, h));
                    }
                }
            }
        }
        None
    }

    /// An object's argument record: `lgob/loda`, or a `lobj` in CorelDRAW 3.
    fn record(c: &Chunk) -> Option<&Chunk> {
        c.list_child(b"lgob").and_then(|l| l.child(b"loda")).or_else(|| c.child(b"lobj"))
    }

    fn flags(c: &Chunk) -> u32 {
        c.child(b"flgs").map(|f| Rd::new(&f.data).u32()).unwrap_or(0)
    }

    /// The layers of a page, bottom first. `master`: read the layers the master page owns.
    fn layers(&mut self, page: &Chunk, to_page: Affine, master: bool) -> Vec<Layer> {
        let mut out = vec![];
        let Some(gobj) = page.list_child(b"gobj") else { return out };
        for layr in gobj.lists(b"layr") {
            let flags = Self::flags(layr);
            let is_master_layer = flags & 0x0001_0000 != 0;
            if is_master_layer != master {
                continue;
            }
            let loda = Self::record(layr);
            let (kind, name) = match loda {
                Some(l) => {
                    let l = self.loda(&l.data);
                    (l.kind, l.arg(ARG_NAME).map(|n| self.string(n)).unwrap_or_default())
                }
                None => (0, String::new()),
            };
            if kind == T_GRID_LAYER || kind == T_GUIDES_LAYER {
                continue;
            }
            let mut objects = vec![];
            self.children(layr, to_page, Affine::IDENTITY, &mut objects);
            if objects.is_empty() && (master || kind == T_DESKTOP_LAYER) {
                continue;
            }
            let id = self.doc.new_id();
            let fallback = if kind == T_DESKTOP_LAYER { "Desktop" } else { "Layer" };
            let mut layer = Layer::new(id, if name.is_empty() { fallback } else { &name });
            layer.objects = objects;
            layer.printable = flags & 0x0008 == 0;
            layer.locked = flags & 0x0010 != 0;
            layer.visible = flags & 0x0100 == 0;
            if master {
                layer.master = 1;
            }
            out.push(layer);
        }
        out.reverse();
        out
    }

    /// The guidelines on a guides layer. Each keeps its angle (millionths of a degree,
    /// counterclockwise) and two points on it, after twelve bytes CorelDRAW 9 to 2026 agree on.
    /// Each guide's point is the one nearest `center`.
    fn guides(&self, page: &Chunk, to_page: Affine, center: Point) -> Vec<Guide> {
        let mut out = vec![];
        if self.ver < 600 {
            return out;
        }
        let Some(gobj) = page.list_child(b"gobj") else { return out };
        let layers = gobj.lists(b"layr").filter(|l| Self::record(l).is_some_and(|r| self.loda(&r.data).kind == T_GUIDES_LAYER));
        for o in layers.flat_map(|l| l.lists(b"obj ")) {
            let Some(rec) = Self::record(o) else { continue };
            let l = self.loda(&rec.data);
            let Some(a) = l.arg(ARG_GUIDE).filter(|_| l.kind == T_GUIDE) else { continue };
            if a.len() < 32 {
                continue;
            }
            let mut r = Rd::at(a, 12);
            let angle = (r.i32() as f64 / 1e6).rem_euclid(180.0);
            let p = to_page * Point::new(r.i32() as f64, r.i32() as f64);
            let (kind, p) = if angle.abs() < 1e-3 || (angle - 180.0).abs() < 1e-3 {
                (GuideKind::Horizontal, Point::new(0.0, p.y))
            } else if (angle - 90.0).abs() < 1e-3 {
                (GuideKind::Vertical, Point::new(p.x, 0.0))
            } else {
                // along the line, y down
                let d = kurbo::Vec2::new(angle.to_radians().cos(), -angle.to_radians().sin());
                (GuideKind::Angled(angle), p + d * (center - p).dot(d))
            };
            out.push(Guide { kind, pos: p, locked: false, color: Color::rgb(0, 153, 255) });
        }
        out
    }

    // ---------------------------------------------------------------- objects

    /// Convert one `obj `/`grp ` list (or any list holding objects) and push the result.
    fn object(&mut self, c: &Chunk, to_page: Affine, parent: Affine, out: &mut Vec<Object>) {
        let Some(lt) = c.list else { return };
        if &lt == b"lgob" || &lt == b"trfl" {
            return;
        }
        let lgob = c.list_child(b"lgob");
        let loda = Self::record(c).map(|l| self.loda(&l.data));
        let is_group = &lt == b"grp ";
        // from version 4 a group's transform is repeated in each of its objects
        let t = if self.ver < 400 {
            parent * loda.as_ref().and_then(|l| self.transform3(l)).unwrap_or(Affine::IDENTITY)
        } else if is_group {
            parent
        } else {
            parent * lgob.map(|l| self.transform(l)).unwrap_or(Affine::IDENTITY)
        };
        let name = loda.as_ref().and_then(|l| l.arg(ARG_NAME)).map(|n| self.string(n)).unwrap_or_default();
        let has_children = c.children.iter().any(|k| k.list.is_some() && !k.is_list(b"lgob"));
        let kind = loda.as_ref().map(|l| l.kind).unwrap_or(0);
        let space = to_page * parent;
        let own = lgob.and_then(|l| self.perspective(l)).map(|p| persp::Warp { map: persp::Map::Persp(p), space });
        let envelope = lgob.and_then(|l| self.envelope_outline(l));
        // an envelope maps the box of what it holds, in its parent's space
        let envelope_warp = |objs: &[Object]| -> Option<persp::Warp> {
            let inv = space.inverse();
            let b = objs.iter().map(|o| (inv * o.world_path()).bounding_box()).reduce(|a, b| a.union(b))?;
            persp::Envelope::new(b, envelope.as_ref()?).map(|e| persp::Warp { map: persp::Map::Envelope(e), space })
        };
        let shape = match (&loda, &lt) {
            (Some(l), b"obj ") => self.shape(c, l, to_page * t),
            _ => None,
        };
        if let Some(mut o) = shape {
            o.name = name;
            if let Some(lens) = loda.as_ref().and_then(|l| l.arg(ARG_LENS)).and_then(|a| self.lens(a)) {
                o.effects.lens = Some(lens);
            }
            if let Some(e) = lgob.and_then(|l| self.extrude(l, to_page, &o)) {
                o.effects.extrude = Some(e);
            }
            // a PowerClip frame: its contents are a group of the clip table
            if let Some(a) = loda.as_ref().and_then(|l| l.arg(ARG_POWERCLIP)) {
                let mut contents = vec![];
                if let Some(clip) = self.clip(a) {
                    self.children(clip, to_page, Affine::IDENTITY, &mut contents);
                }
                {
                    let style = o.style.clone();
                    let name = std::mem::take(&mut o.name);
                    let mut pc = Object::new(self.doc.new_id(), ObjKind::PowerClip { frame: Box::new(o), contents }, Affine::IDENTITY, style);
                    pc.name = name;
                    o = pc;
                }
            }
            let own = own.or_else(|| envelope_warp(std::slice::from_ref(&o)));
            let warps: Vec<persp::Warp> = own.into_iter().chain(self.warps.iter().rev().cloned()).collect();
            persp::bake(&mut o, &warps);
            if let Some(s) = c.child(b"spnd") {
                self.by_spnd.insert(spnd(&s.data), o.id);
            }
            out.push(o);
            return;
        }
        if has_children {
            let mut kids = vec![];
            if envelope.is_some() && own.is_none() {
                // the envelope needs the box of the contents: make them without the outer warps,
                // then apply the envelope and the outer warps in that order
                let outer = std::mem::take(&mut self.warps);
                self.children(c, to_page, t, &mut kids);
                self.warps = outer;
                let warps: Vec<persp::Warp> = envelope_warp(&kids).into_iter().chain(self.warps.iter().rev().cloned()).collect();
                kids.iter_mut().for_each(|k| persp::bake(k, &warps));
            } else {
                let pushed = own.map(|w| self.warps.push(w)).is_some();
                self.children(c, to_page, t, &mut kids);
                if pushed {
                    self.warps.pop();
                }
            }
            if kids.is_empty() {
                return;
            }
            if &lt != b"grp " && kids.len() == 1 && name.is_empty() {
                out.extend(kids);
                return;
            }
            let id = self.doc.new_id();
            let mut g = Object::new(id, ObjKind::Group(kids), Affine::IDENTITY, Style::default());
            g.style.outline = Outline::none();
            g.name = name;
            out.push(g);
        }
        // a CorelDRAW 4 PowerLine is the outline of the shapes CorelDRAW drew for it, which the
        // group next to it holds: nothing is left out (CorelDRAW 5 and 6 keep only the PowerLine)
        let powerline = kind == T_POWERLINE4 && self.ver < 500;
        if !has_children && &lt == b"obj " && !powerline {
            *self.skipped.entry(kind).or_default() += 1;
        }
    }

    /// The clip-table group of a PowerClip frame: the group whose `spnd` names the frame, or
    /// the group whose id follows the frame's.
    fn clip(&self, a: &[u8]) -> Option<&'a Chunk> {
        let n = Rd::new(a).u32();
        self.clip_owners.get(&n).or_else(|| self.clips.get(&n.wrapping_add(1))).copied()
    }

    pub(super) fn style_of(&self, l: &Loda, bounds: kurbo::Rect, world: Affine) -> Style {
        let mut st = Style::default();
        st.fill = Fill::None;
        st.outline = Outline::none();
        // CorelDRAW 3 keeps the fill and outline in the object itself
        let fill3 = if self.ver < 400 { l.arg(ARG_FILL).map(|a| self.fill3(a)) } else { None };
        // CorelDRAW 5 and 6 objects without their own fill or outline take their style's
        let (style_fill, style_outline) = match l.arg(ARG_STYLE) {
            Some(a) if self.ver < 700 => self.graphic_styles.get(&self.uint(&mut Rd::new(a))).copied().unwrap_or_default(),
            _ => (0, 0),
        };
        let fill_id = l.arg(ARG_FILL).map(|a| Rd::new(a).u32()).or((style_fill != 0).then_some(style_fill));
        if let Some(id) = fill_id {
            st.fill = match fill3.as_ref().or_else(|| self.fills.get(&id)) {
                Some(RawFill::Solid(c)) => Fill::Solid(*c),
                Some(RawFill::Gradient { kind, angle, cx, cy, pad, stops }) => fit_gradient(*kind, *angle, *cx, *cy, *pad, stops, bounds, world),
                Some(RawFill::Image { bitmap, tile, angle }) => match self.images.get(bitmap) {
                    Some((image, _, _)) => Fill::Pattern(PatternFill {
                        kind: PatternKind::Checker,
                        fg: Color::BLACK,
                        bg: Color::WHITE,
                        tile: if *tile > 0.1 { *tile } else { 36.0 },
                        angle: *angle,
                        image: Some(*image),
                        transform_with_object: true,
                        vector: false,
                        mirror: 0,
                    }),
                    None => Fill::Solid(Color::rgb(200, 200, 200)),
                },
                _ => Fill::None,
            };
        }
        if let (Some(a), true) = (l.arg(ARG_OUTLINE), self.ver < 400) {
            st.outline = self.outline3(a);
        } else if let Some(id) = l.arg(ARG_OUTLINE).map(|a| Rd::new(a).u32()).or((style_outline != 0).then_some(style_outline)) {
            if let Some(o) = self.outlines.get(&id) {
                st.outline = o.clone();
            }
        }
        if let Some(a) = l.arg(ARG_LENS) {
            if let Some(t) = self.transparency(a) {
                st.transparency = t;
            }
        }
        st
    }

    /// A lens (X3 and later): class, id, then for color lenses the kind, the rate in tenths
    /// and the colors.
    fn lens(&self, a: &[u8]) -> Option<vector_core::object::LensEffect> {
        use vector_core::object::{LensEffect, LensKind};
        if self.ver < 1300 {
            return None;
        }
        let mut r = Rd::at(a, 8);
        let class = r.u8();
        let _id = r.u32();
        let color_at = |r: &mut Rd| {
            r.skip(5);
            color::read(r, self.ver)
        };
        let lens = match class {
            1 => {
                let kind = r.u8();
                let rate = r.i16() as f64 / 10.0;
                let c1 = color_at(&mut r);
                r.skip(26);
                let c2 = color_at(&mut r);
                let (kind, rate) = match kind {
                    0 => (LensKind::Transparency, 100.0 - rate),
                    1 => (LensKind::ColorLimit, rate),
                    2 => (LensKind::ColorAdd, rate),
                    3 => (LensKind::Invert, rate),
                    4 => (LensKind::Brighten, rate),
                    5 => (LensKind::TintedGrayscale, rate),
                    6 => (LensKind::CustomColorMap, rate),
                    7 => (LensKind::HeatMap, rate),
                    _ => return None,
                };
                let mut l = LensEffect::new(kind);
                l.rate = rate;
                l.color = c1;
                l.color2 = c2;
                if kind == LensKind::HeatMap {
                    l.rotation = rate;
                }
                l
            }
            2 => {
                let mut l = LensEffect::new(LensKind::Magnify);
                l.amount = r.u16() as f64 / 10.0;
                l
            }
            3 => {
                let mut l = LensEffect::new(LensKind::FishEye);
                l.rate = r.i16() as f64;
                l
            }
            4 => {
                let mut l = LensEffect::new(LensKind::Wireframe);
                let _outline_mode = r.u32();
                l.outline = Some(color_at(&mut r));
                r.skip(26);
                let _fill_mode = r.u32();
                l.fill = Some(color_at(&mut r));
                l
            }
            _ => return None,
        };
        Some(lens)
    }

    /// Uniform transparency stored as a lens.
    fn transparency(&self, a: &[u8]) -> Option<Transparency> {
        if self.ver >= 1300 {
            let mut r = Rd::new(a);
            r.skip(8);
            let lens = r.u8();
            if lens == 0x05 && a.len() >= 0x4d {
                let t = Rd::at(a, 0x45).f64();
                if t > 0.0 && t <= 100.0 {
                    return Some(Transparency::Uniform(t));
                }
            }
            return None;
        }
        let mut r = Rd::at(a, 10);
        let opacity = r.u16() as f64 / 1000.0;
        if opacity > 0.0 && opacity < 1.0 {
            return Some(Transparency::Uniform((1.0 - opacity) * 100.0));
        }
        None
    }

    /// `world` maps the object's file-unit coordinates to the page, in points.
    fn shape(&mut self, obj: &Chunk, l: &Loda, world: Affine) -> Option<Object> {
        let k = self.k;
        match l.kind {
            T_RECT => {
                let mut r = Rd::new(l.arg(ARG_COORDS)?);
                let (x0, y0);
                // corner sizes in points, or as a fraction of half the shorter side
                let mut radii = [(false, 0.0f64); 4];
                let mut corner = CornerType::Round;
                if self.ver >= 1500 {
                    x0 = r.f64();
                    y0 = r.f64();
                    let _scale_x = r.f64();
                    let _scale_y = r.f64();
                    for rad in radii.iter_mut() {
                        let absolute = r.u8() != 0;
                        r.skip(7);
                        let v = r.f64();
                        let ct = r.u8();
                        r.skip(7);
                        *rad = if absolute { (false, v * k) } else { (true, v) };
                        corner = match ct {
                            1 => CornerType::Scallop,
                            2 => CornerType::Chamfer,
                            _ => corner,
                        };
                    }
                } else {
                    x0 = self.coord(&mut r);
                    y0 = self.coord(&mut r);
                    // before version 9 one radius serves all four corners
                    let first = self.coord(&mut r).abs() * k;
                    for (i, rad) in radii.iter_mut().enumerate() {
                        let v = if i == 0 || self.ver < 900 { first } else { self.coord(&mut r).abs() * k };
                        *rad = (false, v);
                    }
                }
                if x0 == 0.0 || y0 == 0.0 {
                    return None;
                }
                // Corners keep their shape when CorelDRAW stretches a rectangle, so a stretch
                // without skew goes into the rectangle's size instead of its transform.
                let [a, b, c, d, _, _] = world.as_coeffs();
                let (ux, uy) = (kurbo::Vec2::new(a, b), kurbo::Vec2::new(c, d));
                let straight = ux.dot(uy).abs() <= 1e-6 * ux.hypot() * uy.hypot();
                let (sx, sy) = if self.ver >= 1500 && straight && ux.hypot() > 0.0 && uy.hypot() > 0.0 { (ux.hypot(), uy.hypot()) } else { (k, k) };
                let (w, h) = (x0.abs() * sx, y0.abs() * sy);
                let half = w.min(h) / 2.0;
                let size = |(relative, v): (bool, f64)| if relative { v * half } else { v };
                // stored: lower-right, upper-right, upper-left, lower-left;
                // ours: top-left, top-right, bottom-right, bottom-left
                let rr = [size(radii[2]), size(radii[1]), size(radii[0]), size(radii[3])];
                let shape = RectShape { w, h, radii: rr, corner };
                let local = Affine::scale_non_uniform(x0.signum() / sx, y0.signum() / sy);
                let xf = world * local;
                let bounds = (xf * kurbo::Rect::new(0.0, 0.0, shape.w, shape.h).to_path(0.1)).bounding_box();
                let st = self.style_of(l, bounds, xf);
                Some(Object::new(self.doc.new_id(), ObjKind::Rect(shape), xf, st))
            }
            T_ELLIPSE => {
                let mut r = Rd::new(l.arg(ARG_COORDS)?);
                let x0 = self.coord(&mut r);
                let y0 = self.coord(&mut r);
                let a1 = self.angle(&mut r);
                let a2 = self.angle(&mut r);
                let pie = self.uint(&mut r) != 0;
                if x0 == 0.0 || y0 == 0.0 {
                    return None;
                }
                let full = (a1 - a2).abs() < 1e-6;
                // stored angles turn towards the box's own y direction; ours are counter-clockwise
                let shape = EllipseShape {
                    w: x0.abs() * k,
                    h: y0.abs() * k,
                    kind: if full {
                        EllipseKind::Ellipse
                    } else if pie {
                        EllipseKind::Pie
                    } else {
                        EllipseKind::Arc
                    },
                    start: if full { 0.0 } else { (-a2).rem_euclid(360.0) },
                    end: if full { 0.0 } else { (-a1).rem_euclid(360.0) },
                    clockwise: false,
                };
                let local = Affine::scale_non_uniform(x0.signum() / k, y0.signum() / k);
                let xf = world * local;
                let bounds = (xf * kurbo::Rect::new(0.0, 0.0, shape.w, shape.h).to_path(0.1)).bounding_box();
                let st = self.style_of(l, bounds, xf);
                Some(Object::new(self.doc.new_id(), ObjKind::Ellipse(shape), xf, st))
            }
            // PowerLines of CorelDRAW 5 and 6 (0x0e): the outline CorelDRAW drew, as a curve
            T_CURVE | T_PATH | T_POWERLINE4 => {
                if l.kind == T_POWERLINE4 && self.ver < 500 {
                    return None;
                }
                let path = world * self.curve(l.arg(ARG_COORDS)?);
                self.path_object(l, path)
            }
            T_POLYGON => {
                let template = self.curve(l.arg(ARG_COORDS)?);
                let mut r = Rd::new(l.arg(ARG_POLYGON)?);
                let n = (self.uint(&mut r) as usize).clamp(1, 500);
                let step = (self.uint(&mut r) as usize).max(1);
                r.skip(if self.wide() { 8 } else { 4 });
                let (rx, ry) = (r.f64(), r.f64());
                let cx = self.coord(&mut r);
                let cy = self.coord(&mut r);
                let path = world * Affine::translate((cx, cy)) * Affine::scale_non_uniform(rx, ry) * polygon(&template, n, step);
                self.path_object(l, path)
            }
            T_ARTISTIC | T_PARAGRAPH => text::object(self, obj, l, world),
            T_BITMAP => super::bitmap::object(self, l, world),
            _ => None,
        }
    }

    fn path_object(&mut self, l: &Loda, path: BezPath) -> Option<Object> {
        if path.elements().len() < 2 {
            return None;
        }
        let st = self.style_of(l, path.bounding_box(), Affine::IDENTITY);
        Some(Object::new(self.doc.new_id(), ObjKind::Path(NodePath::from_bez(&path)), Affine::IDENTITY, st))
    }

    /// A curve: the points, then one flag byte per point.
    pub fn curve(&self, a: &[u8]) -> BezPath {
        let mut r = Rd::new(a);
        let n = r.u16() as usize;
        r.skip(2);
        let w = if self.wide() { 8 } else { 4 };
        let mut bp = BezPath::new();
        if n == 0 || n > r.left() / (w + 1) {
            return bp;
        }
        let pts: Vec<Point> = (0..n).map(|_| Point::new(self.coord(&mut r), self.coord(&mut r))).collect();
        let types = r.bytes(n);
        let mut ctrl: Vec<Point> = vec![];
        let mut open = false;
        for (p, t) in pts.iter().zip(types.iter()) {
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
                0x40 => bp.line_to(*p),
                _ => {
                    if ctrl.len() >= 2 {
                        bp.curve_to(ctrl[0], ctrl[1], *p);
                    } else {
                        bp.line_to(*p);
                    }
                }
            }
            ctrl.clear();
            if t & 0x08 != 0 {
                bp.close_path();
                open = false;
            }
        }
        bp
    }
}

/// A polygon or star: the stored side, repeated around the center.
fn polygon(side: &BezPath, n: usize, step: usize) -> BezPath {
    let mut bp = BezPath::new();
    let turn = -2.0 * std::f64::consts::PI * step as f64 / n as f64;
    for i in 0..n {
        let rot = Affine::rotate(turn * i as f64);
        for (j, el) in side.elements().iter().enumerate() {
            match (rot * *el, i, j) {
                (kurbo::PathEl::MoveTo(p), 0, 0) => bp.move_to(p),
                (kurbo::PathEl::MoveTo(_), _, _) => {}
                (kurbo::PathEl::ClosePath, _, _) => {}
                (e, _, _) => bp.push(e),
            }
        }
    }
    bp.close_path();
    bp
}

/// Place a stored fountain fill on an object. Our gradients are positioned in the object's
/// local bounding box, so the page-space line is taken back through the object's transform.
fn fit_gradient(kind: GradientKind, angle: f64, cx: f64, cy: f64, pad: f64, stops: &[GradientStop], bounds: kurbo::Rect, world: Affine) -> Fill {
    let (w, h) = (bounds.width().max(1e-6), bounds.height().max(1e-6));
    let c = bounds.center();
    let a = angle.to_radians();
    // direction on the page (y down)
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
    // into the object's own box (0..1)
    let local_box = (world.inverse() * bounds.to_path(0.1)).bounding_box();
    let inv = world.inverse();
    let unit = |p: Point| {
        let q = inv * p;
        Point::new((q.x - local_box.x0) / local_box.width().max(1e-9), (q.y - local_box.y0) / local_box.height().max(1e-9))
    };
    let mut st: Vec<GradientStop> = stops.to_vec();
    if !matches!(kind, GradientKind::Linear | GradientKind::Conical) {
        // elliptical and rectangular fountain fills run from the edge to the center
        for s in st.iter_mut() {
            s.pos = 1.0 - s.pos;
        }
        st.reverse();
    }
    Fill::Gradient(Gradient { kind, stops: st, start: unit(start), end: unit(end), spread: SpreadMode::Pad, steps: 0, accel: 0.0, blend: 0, smooth: false, free_scale: true })
}

/// How far half a page side is from the next whole pixel at 300 dpi, in points.
fn origin_shift(side: f64) -> f64 {
    let px = side / 2.0 / 72.0 * 300.0;
    ((px - 1e-6).ceil() - px) / 300.0 * 72.0
}

pub(super) fn version(root: &Chunk) -> u32 {
    if let Some(v) = root.child(b"vrsn") {
        let n = Rd::new(&v.data).u16() as u32;
        if n >= 100 {
            return n;
        }
    }
    // the form type ends with the version: "CDR5".."CDR9", then letters from "CDRA" (10)
    let c = root.list.map(|l| l[3]).unwrap_or(b' ');
    match c {
        b'0'..=b'9' => (c - b'0') as u32 * 100,
        b'A'..=b'Z' => (c - b'A') as u32 * 100 + 1000,
        _ => 300,
    }
}

/// CorelDRAW 4 facing pages: each page record holds a spread, the left page left of the origin
/// and the right page right of it; the master layers hold what left and right pages show the
/// same way. The spreads become pages of their own, and each master layer two: one for the
/// odd pages and one for the even pages. The first spread has only its right page unless the
/// document starts on a left page; the last right page goes when it is empty.
fn split_spreads(spreads: Vec<Page>, left_first: bool, doc: &mut Document) -> Vec<Page> {
    let Some(w) = spreads.first().map(|p| p.width) else { return spreads };
    let n = spreads.len();
    let mut out: Vec<Page> = vec![];
    let mut masters: Vec<Layer> = vec![];
    for (i, spread) in spreads.into_iter().enumerate() {
        let mut halves = [Page::new(doc.new_id(), 0, spread.width, spread.height), Page::new(doc.new_id(), 0, spread.width, spread.height)];
        for p in &mut halves {
            p.guides = spread.guides.clone();
            p.layers.clear();
        }
        for layer in spread.layers {
            let mut sides = [Layer { id: doc.new_id(), objects: vec![], ..layer.clone() }, Layer { id: layer.id, objects: vec![], ..layer.clone() }];
            for mut o in layer.objects {
                let b = o.bounds();
                // left of the spine: the left page
                let right = (b.x0 + b.x1) / 2.0 >= w / 2.0;
                o.apply_transform(Affine::translate((if right { -w / 2.0 } else { w / 2.0 }, 0.0)));
                sides[right as usize].objects.push(o);
            }
            if layer.master != 0 {
                // the odd pages are the right ones unless the document starts on a left page
                for (side, mut l) in sides.into_iter().enumerate().filter(|(_, l)| !l.objects.is_empty()) {
                    let odd = (side == 1) != left_first;
                    l.master = if odd { 2 } else { 3 };
                    masters.push(l);
                }
            } else {
                for (side, l) in sides.into_iter().enumerate() {
                    halves[side].layers.push(l);
                }
            }
        }
        let [left, right] = halves;
        let empty = |p: &Page| p.layers.iter().all(|l| l.objects.is_empty());
        if i > 0 || left_first {
            out.push(left);
        }
        if i + 1 < n || !empty(&right) {
            out.push(right);
        }
    }
    for p in &mut out {
        if p.layers.is_empty() {
            p.layers.push(Layer::new(doc.new_id(), "Layer 1"));
        }
    }
    if let Some(first) = out.first_mut() {
        masters.append(&mut first.layers);
        first.layers = masters;
    }
    out
}

pub(super) fn document(root: &Chunk) -> Result<Document> {
    document_with_report(root).map(|r| r.0)
}

/// The document and the object types left out (type, count).
pub(super) fn document_with_report(root: &Chunk) -> Result<(Document, Vec<(u32, usize)>)> {
    let ver = version(root);
    let k = if ver >= 600 { 72.0 / 254_000.0 } else { 72.0 / 1000.0 };
    let mut cx = Ctx { ver, k, fills: HashMap::new(), outlines: HashMap::new(), fonts: HashMap::new(), charsets: HashMap::new(), named_charsets: Default::default(), styles: Default::default(), images: HashMap::new(), clips: HashMap::new(), vects: HashMap::new(), clip_owners: HashMap::new(), graphic_styles: HashMap::new(), arrows: HashMap::new(), patterns: HashMap::new(), text_paths: vec![], path_places: HashMap::new(), by_spnd: HashMap::new(), skipped: Default::default(), warps: vec![], texts4: HashMap::new(), text4: None, chains4: vec![], doc: Document::new(612.0, 792.0) };
    cx.collect(root);
    cx.pattern_images();
    let default_size = cx.default_page_size(root);
    let origin = cx.origin(root);
    let mut first_size: Option<(f64, f64)> = None;
    let centered_on_grid = match (origin, root.lists(b"page").find(|p| Ctx::flags(p) & 0x0001_0000 == 0).map(|p| cx.page_size(p).unwrap_or(default_size))) {
        (Some((x, y)), Some((w, h))) => (x + w / 2.0 + origin_shift(w)).abs() < 0.002 && (y + h / 2.0 + origin_shift(h)).abs() < 0.002,
        _ => false,
    };
    let pages: Vec<&Chunk> = root.lists(b"page").collect();
    if pages.is_empty() {
        return Err(IoError::Format("the CorelDRAW drawing has no pages".into()));
    }
    // the master page comes first when the file has one
    let master = pages.iter().position(|p| Ctx::flags(p) & 0x0001_0000 != 0).filter(|_| pages.len() > 1);
    let mut out: Vec<Page> = vec![];
    for (i, p) in pages.iter().enumerate() {
        if Some(i) == master {
            continue;
        }
        let (w, h) = cx.page_size(p).unwrap_or(default_size);
        // the page's lower-left corner, in points from the origin
        let (first_w, first_h) = *first_size.get_or_insert((w, h));
        let corner = match origin {
            Some((x, y)) if centered_on_grid => {
                let _ = (x, y);
                (-w / 2.0 - origin_shift(w), -h / 2.0 - origin_shift(h))
            }
            Some((x, y)) => (x + first_w / 2.0 - w / 2.0, y + first_h / 2.0 - h / 2.0),
            None => (-w / 2.0, -h / 2.0),
        };
        let to_page = Affine::translate((-corner.0, h + corner.1)) * Affine::scale_non_uniform(k, -k);
        let mut layers = cx.layers(p, to_page, false);
        let page_guides = cx.guides(p, to_page, Point::new(w / 2.0, h / 2.0));
        if out.is_empty() {
            if let Some(m) = master {
                let mut ml = cx.layers(pages[m], to_page, true);
                cx.doc.master_guides = cx.guides(pages[m], to_page, Point::new(w / 2.0, h / 2.0));
                ml.append(&mut layers);
                layers = ml;
            }
        }
        if layers.is_empty() {
            let id = cx.doc.new_id();
            layers.push(Layer::new(id, "Layer 1"));
        }
        let pid = cx.doc.new_id();
        let mut page = Page::new(pid, 0, w, h);
        page.layers = layers;
        page.guides = page_guides;
        out.push(page);
    }
    if let Some(left_first) = cx.facing4(root) {
        out = split_spreads(out, left_first, &mut cx.doc);
    }
    let skipped = cx.skipped.iter().map(|(k, v)| (*k, *v)).collect();
    let mut doc = cx.doc;
    doc.pages = out;
    for (a, z) in &cx.text_paths {
        let (Some(&a), Some(&z)) = (cx.by_spnd.get(a), cx.by_spnd.get(z)) else { continue };
        let is_text = |doc: &Document, id| doc.find(id).is_some_and(|o| matches!(o.kind, ObjKind::Text(_)));
        let (text, path) = if is_text(&doc, a) { (a, z) } else { (z, a) };
        if is_text(&doc, text) && !is_text(&doc, path) {
            vector_core::ops::fit_text_to_path(&mut doc, text, path);
            // a whole ellipse in the text's space
            let ellipse = match (doc.find(path), doc.find(text)) {
                (Some(p), Some(t)) => match &p.kind {
                    ObjKind::Ellipse(e) if e.kind == EllipseKind::Ellipse => Some((e.w, e.h, t.transform.inverse() * p.transform)),
                    _ => None,
                },
                _ => None,
            };
            let place = cx.path_places.get(&text).copied();
            if let Some((ObjKind::Text(t), place)) = doc.find_mut(text).map(|o| &mut o.kind).zip(place) {
                text::place_on_path(t, place, ellipse);
            }
        }
    }
    // CorelDRAW 4 paragraph text that flows through frames: the first frame holds it, the others
    // follow it
    let mut linked = false;
    for (_, frames) in &cx.chains4 {
        let ids: Vec<u64> = frames.iter().filter_map(|f| cx.by_spnd.get(f).copied()).filter(|id| doc.find(*id).is_some_and(|o| matches!(o.kind, ObjKind::Text(_)))).collect();
        for (i, id) in ids.iter().enumerate() {
            if let Some(ObjKind::Text(t)) = doc.find_mut(*id).map(|o| &mut o.kind) {
                t.link_prev = i.checked_sub(1).map(|p| ids[p]);
                t.link_next = ids.get(i + 1).copied();
                linked |= ids.len() > 1;
            }
        }
    }
    if linked {
        doc.update_text_flow();
    }
    doc.ensure_next_id();
    Ok((doc, skipped))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn ctx(ver: u32) -> Ctx<'static> {
        let k = if ver >= 600 { 72.0 / 254_000.0 } else { 72.0 / 1000.0 };
        Ctx { ver, k, fills: HashMap::new(), outlines: HashMap::new(), fonts: HashMap::new(), charsets: HashMap::new(), named_charsets: Default::default(), styles: Default::default(), images: HashMap::new(), clips: HashMap::new(), vects: HashMap::new(), clip_owners: HashMap::new(), graphic_styles: HashMap::new(), arrows: HashMap::new(), patterns: HashMap::new(), text_paths: vec![], path_places: HashMap::new(), by_spnd: HashMap::new(), skipped: Default::default(), warps: vec![], texts4: HashMap::new(), text4: None, chains4: vec![], doc: Document::new(612.0, 792.0) }
    }

    #[test]
    fn coreldraw_4_spreads_become_pages() {
        let mut doc = Document::new(100.0, 50.0);
        let rect = |doc: &mut Document, x: f64| {
            let shape = RectShape { w: 10.0, h: 10.0, radii: [0.0; 4], corner: CornerType::Round };
            Object::new(doc.new_id(), ObjKind::Rect(shape), Affine::translate((x, 20.0)), Style::default())
        };
        // the spine is at x = 50: the first spread has only a right page, the second both
        let spread = |doc: &mut Document, xs: &[f64], master: bool| {
            let mut p = Page::new(doc.new_id(), 0, 100.0, 50.0);
            let mut l = Layer::new(doc.new_id(), if master { "Master" } else { "Layer" });
            l.master = master as u8;
            l.objects = xs.iter().map(|x| rect(doc, *x)).collect();
            p.layers = vec![l];
            p
        };
        let mut first = spread(&mut doc, &[60.0], false);
        let master = spread(&mut doc, &[5.0, 85.0], true).layers.remove(0);
        first.layers.insert(0, master);
        let second = spread(&mut doc, &[10.0, 70.0], false);
        let pages = split_spreads(vec![first, second], false, &mut doc);
        assert_eq!(pages.len(), 3);
        let xs = |p: &Page| p.layers.iter().filter(|l| l.master == 0).flat_map(|l| &l.objects).map(|o| o.bounds().x0.round()).collect::<Vec<_>>();
        assert_eq!(xs(&pages[0]), vec![10.0]);
        assert_eq!(xs(&pages[1]), vec![60.0]);
        assert_eq!(xs(&pages[2]), vec![20.0]);
        // the master's left object shows on even pages, its right one on odd pages
        let masters: Vec<(u8, f64)> = pages[0].layers.iter().filter(|l| l.master != 0).map(|l| (l.master, l.objects[0].bounds().x0.round())).collect();
        assert_eq!(masters, vec![(3, 55.0), (2, 35.0)]);
    }

    #[test]
    fn coreldraw_4_bitmaps_are_bmp_files() {
        let mut bmp = std::io::Cursor::new(vec![]);
        image::RgbImage::from_pixel(3, 2, image::Rgb([200, 10, 10])).write_to(&mut bmp, image::ImageFormat::Bmp).unwrap();
        let mut b = 7u16.to_le_bytes().to_vec();
        b.extend(bmp.into_inner());
        let mut cx = ctx(400);
        super::super::bitmap::bitmap(&mut cx, &b);
        assert_eq!(cx.images.get(&7).map(|i| (i.1, i.2)), Some((3, 2)));
    }

    #[test]
    fn bitmap_fills_before_x3_point_at_their_bitmap() {
        // a CorelDRAW 7 bitmap fill (as 7effects.cdr has it): id, type 9, two spare bytes, the
        // pattern's id, a tile 4 inches square, offsets, flags, then the bitmap's id
        let mut b = 7u32.to_le_bytes().to_vec();
        b.extend(9u16.to_le_bytes());
        b.extend([0, 0]);
        b.extend(1u32.to_le_bytes());
        b.extend((4 * 254_000i32).to_le_bytes());
        b.extend((4 * 254_000i32).to_le_bytes());
        b.extend([0; 4 + 2 + 1 + 21]);
        b.extend(23u32.to_le_bytes());
        b.extend([0; 8]);
        let mut cx = ctx(700);
        cx.fild(&b);
        match cx.fills.get(&7) {
            Some(RawFill::Image { bitmap: 23, tile, .. }) => assert!((tile - 288.0).abs() < 1e-6, "{tile}"),
            f => panic!("{f:?}"),
        }
    }
}
