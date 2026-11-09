//! Turns the chunk tree of a CorelDRAW drawing into an AnyVector document.

use super::color;
use super::persp;
use super::riff::{Chunk, Rd};
use super::text;
use crate::{IoError, Result};
use kurbo::{Affine, BezPath, Point, Shape};
use std::collections::HashMap;
use vector_core::color::Color;
use vector_core::document::{Document, Layer, Page};
use vector_core::object::{CornerType, EllipseKind, EllipseShape, ObjKind, Object, RectShape};
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
const T_PATH: u32 = 0x25;

/// A fill as stored, before it is fitted to an object.
#[derive(Clone, Debug)]
pub(super) enum RawFill {
    #[allow(dead_code)]
    None,
    Solid(Color),
    /// kind, angle (degrees), center offset (fractions of the box), edge pad (0..0.5), stops
    Gradient { kind: GradientKind, angle: f64, cx: f64, cy: f64, pad: f64, stops: Vec<GradientStop> },
    /// A bitmap, full-color or texture fill: the bitmap id and the tile width in points.
    Image { bitmap: u32, tile: f64 },
}

pub(super) struct Ctx<'a> {
    /// File version times 100 (X5 = 1500).
    pub ver: u32,
    /// Points per file unit.
    pub k: f64,
    pub fills: HashMap<u32, RawFill>,
    pub outlines: HashMap<u32, Outline>,
    pub fonts: HashMap<u32, String>,
    pub styles: text::Styles,
    /// Bitmap id to the document's image id and its pixel size.
    pub images: HashMap<u32, (u64, u32, u32)>,
    /// PowerClip contents: the groups of the clip table, by id.
    pub clips: HashMap<u32, &'a Chunk>,
    /// Object types that were not converted, with how many of each.
    pub skipped: std::collections::BTreeMap<u32, usize>,
    /// Perspectives of the groups being converted, innermost last.
    pub warps: Vec<persp::Warp>,
    pub doc: Document,
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
        let kind = self.uint(&mut r);
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
                let m = if self.wide() {
                    r.skip(6);
                    let v: [f64; 6] = std::array::from_fn(|_| r.f64());
                    Affine::new([v[0], v[3], v[1], v[4], v[2], v[5]])
                } else {
                    let fixed = |r: &mut Rd| r.i32() as f64 / 65536.0;
                    let v0 = fixed(&mut r);
                    let v1 = fixed(&mut r);
                    let x0 = r.i32() as f64;
                    let v3 = fixed(&mut r);
                    let v4 = fixed(&mut r);
                    let y0 = r.i32() as f64;
                    Affine::new([v0, v3, v1, v4, x0, y0])
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
                r.skip(if self.wide() { 10 } else { 6 });
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

    // ---------------------------------------------------------------- document resources

    fn collect(&mut self, c: &'a Chunk) {
        if c.list.is_some() {
            if c.is_list(b"page") {
                return;
            }
            if c.is_list(b"clpt") {
                for k in c.children.iter().filter(|k| k.list.is_some()) {
                    let id = k.list_child(b"lgob").and_then(|l| l.child(b"loda")).and_then(|l| self.loda(&l.data).arg(ARG_CLIP_ID).map(|a| Rd::new(a).u32()));
                    if let Some(id) = id {
                        self.clips.insert(id, k);
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
            b"fild" => self.fild(&c.data),
            b"outl" => self.outl(&c.data),
            b"font" => text::font(self, &c.data),
            b"stlt" => text::stlt(self, &c.data),
            b"bmp " => super::bitmap::bitmap(self, &c.data),
            _ => {}
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
            9 | 10 | 11 if self.ver >= 1300 => {
                // tile size, then the bitmap that holds the tile
                let b = r.b;
                let tile = Rd::at(b, r.p + 16).i32().abs() as f64 * self.k;
                RawFill::Image { bitmap: Rd::at(b, r.p + 48).u32(), tile }
            }
            _ => RawFill::Solid(Color::rgb(200, 200, 200)),
        };
        self.fills.insert(id, fill);
    }

    /// A two-color pattern: use its background and foreground mixed.
    fn two_color(&self, r: &mut Rd) -> RawFill {
        let _pattern = r.u32();
        let skip = if self.wide() { 8 } else { 4 };
        r.skip(skip); // tile width and height
        r.skip(if self.wide() { 6 } else { 4 });
        if self.ver >= 1300 {
            r.skip(5);
        }
        let a = color::read(r, self.ver);
        if self.ver >= 1300 {
            r.skip(31);
        }
        let b = color::read(r, self.ver);
        RawFill::Solid(Color::lerp(a, b, 0.5))
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
        let w = self.coord(&mut r).abs() * self.k;
        let h = self.coord(&mut r).abs() * self.k;
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
            let loda = layr.list_child(b"lgob").and_then(|l| l.child(b"loda"));
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
            for c in layr.children.iter().rev() {
                self.object(c, to_page, Affine::IDENTITY, &mut objects);
            }
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

    // ---------------------------------------------------------------- objects

    /// Convert one `obj `/`grp ` list (or any list holding objects) and push the result.
    fn object(&mut self, c: &Chunk, to_page: Affine, parent: Affine, out: &mut Vec<Object>) {
        let Some(lt) = c.list else { return };
        if &lt == b"lgob" || &lt == b"trfl" {
            return;
        }
        let lgob = c.list_child(b"lgob");
        let loda = lgob.and_then(|l| l.child(b"loda")).map(|l| self.loda(&l.data));
        let is_group = &lt == b"grp ";
        let t = if is_group { parent } else { parent * lgob.map(|l| self.transform(l)).unwrap_or(Affine::IDENTITY) };
        let name = loda.as_ref().and_then(|l| l.arg(ARG_NAME)).map(|n| self.string(n)).unwrap_or_default();
        let has_children = c.children.iter().any(|k| k.list.is_some() && !k.is_list(b"lgob"));
        let kind = loda.as_ref().map(|l| l.kind).unwrap_or(0);
        let own = lgob.and_then(|l| self.perspective(l)).map(|p| persp::Warp { persp: p, space: to_page * parent });
        let shape = match (&loda, &lt) {
            (Some(l), b"obj ") => self.shape(c, l, to_page * t),
            _ => None,
        };
        if let Some(mut o) = shape {
            o.name = name;
            // a PowerClip frame: its contents are a group of the clip table
            if let Some(a) = loda.as_ref().and_then(|l| l.arg(ARG_POWERCLIP)) {
                let mut contents = vec![];
                if let Some(clip) = self.clip(a) {
                    for k in clip.children.iter().rev() {
                        self.object(k, to_page, Affine::IDENTITY, &mut contents);
                    }
                }
                {
                    let style = o.style.clone();
                    let name = std::mem::take(&mut o.name);
                    let mut pc = Object::new(self.doc.new_id(), ObjKind::PowerClip { frame: Box::new(o), contents }, Affine::IDENTITY, style);
                    pc.name = name;
                    o = pc;
                }
            }
            let warps: Vec<persp::Warp> = own.into_iter().chain(self.warps.iter().rev().copied()).collect();
            persp::bake(&mut o, &warps);
            out.push(o);
            return;
        }
        if has_children {
            let mut kids = vec![];
            let pushed = own.map(|w| self.warps.push(w)).is_some();
            for k in c.children.iter().rev() {
                self.object(k, to_page, t, &mut kids);
            }
            if pushed {
                self.warps.pop();
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
        if !has_children && &lt == b"obj " {
            *self.skipped.entry(kind).or_default() += 1;
        }
    }

    /// The clip-table group of a PowerClip frame: the group's id follows the frame's.
    fn clip(&self, a: &[u8]) -> Option<&'a Chunk> {
        let n = Rd::new(a).u32();
        self.clips.get(&n.wrapping_add(1)).copied()
    }

    pub(super) fn style_of(&self, l: &Loda, bounds: kurbo::Rect, world: Affine) -> Style {
        let mut st = Style::default();
        st.fill = Fill::None;
        st.outline = Outline::none();
        if let Some(a) = l.arg(ARG_FILL) {
            let id = Rd::new(a).u32();
            st.fill = match self.fills.get(&id) {
                Some(RawFill::Solid(c)) => Fill::Solid(*c),
                Some(RawFill::Gradient { kind, angle, cx, cy, pad, stops }) => fit_gradient(*kind, *angle, *cx, *cy, *pad, stops, bounds, world),
                Some(RawFill::Image { bitmap, tile }) => match self.images.get(bitmap) {
                    Some((image, _, _)) => Fill::Pattern(PatternFill {
                        kind: PatternKind::Checker,
                        fg: Color::BLACK,
                        bg: Color::WHITE,
                        tile: if *tile > 0.1 { *tile } else { 36.0 },
                        angle: 0.0,
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
        if let Some(a) = l.arg(ARG_OUTLINE) {
            let id = Rd::new(a).u32();
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
                    for rad in radii.iter_mut() {
                        *rad = (false, self.coord(&mut r).abs() * k);
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
            T_CURVE | T_PATH => {
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

pub(super) fn document(root: &Chunk) -> Result<Document> {
    document_with_report(root).map(|r| r.0)
}

/// The document and the object types left out (type, count).
pub(super) fn document_with_report(root: &Chunk) -> Result<(Document, Vec<(u32, usize)>)> {
    let ver = version(root);
    let k = if ver >= 600 { 72.0 / 254_000.0 } else { 72.0 / 1000.0 };
    let mut cx = Ctx { ver, k, fills: HashMap::new(), outlines: HashMap::new(), fonts: HashMap::new(), styles: Default::default(), images: HashMap::new(), clips: HashMap::new(), skipped: Default::default(), warps: vec![], doc: Document::new(612.0, 792.0) };
    cx.collect(root);
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
        if out.is_empty() {
            if let Some(m) = master {
                let mut ml = cx.layers(pages[m], to_page, true);
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
        out.push(page);
    }
    let skipped = cx.skipped.iter().map(|(k, v)| (*k, *v)).collect();
    let mut doc = cx.doc;
    doc.pages = out;
    doc.ensure_next_id();
    Ok((doc, skipped))
}
