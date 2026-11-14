//! The content stream interpreter: runs the operators of a page with the graphics state stack
//! and turns what they paint into AnyVector objects. Clipping paths become PowerClips (left out
//! when they don't cut anything), form XObjects are drawn in place (transparency groups with
//! a constant alpha or a blend mode become a group with that transparency), axial and radial
//! shadings become fountain fills, text becomes artistic text or curves (see `glyph`).

use super::color::Cs;
use super::font::{Font, Glyph};
use super::func::Function;
use super::image;
use super::lexer::{self, Op};
use super::{Note, Notes, Pdf, PdfImportOptions};
use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape, Vec2};
use lopdf::{Dictionary, Object};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use vector_core::color::Color;
use vector_core::document::Document;
use vector_core::object::{ImageAdjust, ImageObj, ObjKind, Object as VObj};
use vector_core::path::NodePath;
use vector_core::style::*;
use vector_core::text::{CharFormat, TextObj};

/// A clipping path in document coordinates, with the clip it was intersected with.
pub struct Clip {
    path: BezPath,
    even_odd: bool,
    parent: Option<Rc<Clip>>,
    rect: Option<Rect>,
}

/// An object and the clip it was painted under.
pub struct Item {
    obj: VObj,
    clip: Option<Rc<Clip>>,
}

#[derive(Clone)]
struct Gs<'a> {
    ctm: Affine,
    clip: Option<Rc<Clip>>,
    fill_cs: Rc<Cs>,
    fill: Vec<f64>,
    fill_pat: Option<&'a Object>,
    stroke_cs: Rc<Cs>,
    stroke: Vec<f64>,
    stroke_pat: Option<&'a Object>,
    lw: f64,
    cap: LineCap,
    join: LineJoin,
    miter: f64,
    dash: Vec<f64>,
    ca: f64,
    ca_stroke: f64,
    blend: MergeMode,
    font: Option<Rc<Font<'a>>>,
    size: f64,
    tc: f64,
    tw: f64,
    th: f64,
    tl: f64,
    rise: f64,
    tr: i64,
}

impl Gs<'_> {
    fn new(ctm: Affine) -> Self {
        Gs {
            ctm,
            clip: None,
            fill_cs: Rc::new(Cs::Gray),
            fill: vec![0.0],
            fill_pat: None,
            stroke_cs: Rc::new(Cs::Gray),
            stroke: vec![0.0],
            stroke_pat: None,
            lw: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter: 10.0,
            dash: vec![],
            ca: 1.0,
            ca_stroke: 1.0,
            blend: MergeMode::Normal,
            font: None,
            size: 0.0,
            tc: 0.0,
            tw: 0.0,
            th: 1.0,
            tl: 0.0,
            rise: 0.0,
            tr: 0,
        }
    }
}

/// Text (or glyph outlines) gathered from consecutive glyphs on one line.
struct Run<'a> {
    font: Rc<Font<'a>>,
    style: Style,
    clip: Option<Rc<Clip>>,
    /// Text: the object's transform and font size; curves: unused.
    xf: Affine,
    size: f64,
    text: String,
    curves: Option<BezPath>,
    /// Where the next glyph is expected, the baseline direction and one em, in the document.
    end: Point,
    dir: Vec2,
    em: f64,
}

/// What one content stream (a page, a form, a pattern tile, a Type 3 glyph) runs with.
struct State<'a> {
    res: Option<&'a Dictionary>,
    /// The stream's default space (for patterns).
    base: Affine,
    gs: Gs<'a>,
    stack: Vec<Gs<'a>>,
    path: BezPath,
    cur: Point,
    start: Point,
    pending_clip: Option<bool>,
    tm: Affine,
    tlm: Affine,
    items: Vec<Item>,
    run: Option<Run<'a>>,
    /// The last item is a fill-only path that a stroke of the same path may join.
    last_fill: Option<BezPath>,
    /// A Type 3 glyph that is drawn in the text color (`d1`).
    uncolored: bool,
}

pub struct Ctx<'p, 'a> {
    pub pdf: &'p Pdf<'a>,
    pub doc: &'p mut Document,
    pub opts: &'p PdfImportOptions,
    pub notes: &'p mut Notes,
    fonts: HashMap<usize, Rc<Font<'a>>>,
    images: HashMap<(usize, Color), Option<u64>>,
    forms: Vec<usize>,
    /// Spot colors met: name → color at full tint.
    pub spots: BTreeMap<String, Color>,
    /// The page's area in the document, for shadings painted without a clip.
    page_rect: Rect,
}

fn num(o: &Object) -> f64 {
    o.as_float().map(|v| v as f64).unwrap_or(0.0)
}

fn nums(args: &[Object]) -> Vec<f64> {
    args.iter().filter_map(|o| o.as_float().ok().map(|v| v as f64)).collect()
}

fn affine(v: &[f64]) -> Option<Affine> {
    (v.len() >= 6).then(|| Affine::new([v[0], v[1], v[2], v[3], v[4], v[5]]))
}

/// How much a transform scales lengths (the square root of its area scale).
fn scale_of(a: Affine) -> f64 {
    a.determinant().abs().sqrt()
}

/// Turns a PDF path into a node path. PDF allows things the node path
/// builder does not: closing a path twice in a row, or drawing straight on
/// after a close without a new move. Those are tidied up first: a repeated
/// close is dropped and a segment after a close starts from the point the
/// closed subpath began at, as PDF says it should.
fn node_path(p: &BezPath) -> NodePath {
    let mut out = BezPath::new();
    let mut start: Option<Point> = None;
    let mut open = false;
    for el in p.elements() {
        match *el {
            PathEl::MoveTo(q) => {
                start = Some(q);
                open = false;
                out.push(PathEl::MoveTo(q));
            }
            PathEl::ClosePath => {
                if open {
                    out.push(PathEl::ClosePath);
                    open = false;
                }
            }
            seg => {
                let Some(s) = start else { continue };
                if !open {
                    if matches!(out.elements().last(), Some(PathEl::ClosePath)) {
                        out.push(PathEl::MoveTo(s));
                    }
                    open = true;
                }
                out.push(seg);
            }
        }
    }
    NodePath::from_bez(&out)
}

/// The average of a picture's opaque pixels (sampling at most about 64k of them).
fn average_color(img: &vector_core::document::ImageData) -> Option<Color> {
    let px = img.rgba.chunks_exact(4);
    let step = (px.len() / 65536).max(1);
    let (mut sum, mut n) = ([0.0f64; 3], 0.0f64);
    for p in px.step_by(step) {
        let a = p[3] as f64 / 255.0;
        for i in 0..3 {
            sum[i] += p[i] as f64 * a;
        }
        n += a;
    }
    (n > 0.0).then(|| Color::rgb((sum[0] / n).round() as u8, (sum[1] / n).round() as u8, (sum[2] / n).round() as u8))
}

fn rect_of(p: &BezPath) -> Option<Rect> {
    let els = p.elements();
    let mut pts = vec![];
    for (i, e) in els.iter().enumerate() {
        match e {
            PathEl::MoveTo(q) if i == 0 => pts.push(*q),
            PathEl::LineTo(q) if i > 0 => pts.push(*q),
            PathEl::ClosePath if i == els.len() - 1 => {}
            _ => return None,
        }
    }
    if pts.len() == 5 && (pts[4] - pts[0]).hypot() < 1e-6 {
        pts.pop();
    }
    if pts.len() != 4 {
        return None;
    }
    let close = |a: f64, b: f64| (a - b).abs() < 1e-6;
    let axis = (0..4).all(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % 4]);
        close(a.x, b.x) || close(a.y, b.y)
    });
    axis.then(|| Rect::from_points(pts[0], pts[2]))
}

fn same_clip(a: &Option<Rc<Clip>>, b: &Option<Rc<Clip>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

fn chain(c: &Option<Rc<Clip>>) -> Vec<Rc<Clip>> {
    let mut v = vec![];
    let mut cur = c.clone();
    while let Some(n) = cur {
        cur = n.parent.clone();
        v.push(n);
    }
    v.reverse();
    v
}

fn alpha(c: Color, a: f64) -> Color {
    c.with_alpha(((c.a as f64) * a.clamp(0.0, 1.0)).round() as u8)
}

impl<'p, 'a> Ctx<'p, 'a> {
    pub fn new(pdf: &'p Pdf<'a>, doc: &'p mut Document, opts: &'p PdfImportOptions, notes: &'p mut Notes) -> Self {
        Ctx { pdf, doc, opts, notes, fonts: HashMap::new(), images: HashMap::new(), forms: vec![], spots: BTreeMap::new(), page_rect: Rect::ZERO }
    }

    /// Run a page's content and return its objects.
    pub fn page(&mut self, contents: &[u8], res: Option<&'a Dictionary>, base: Affine, page_rect: Rect) -> Vec<VObj> {
        self.page_rect = page_rect;
        let mut st = State::new(res, base, Gs::new(base));
        let ops = lexer::parse(contents);
        self.exec(&ops, &mut st, 0);
        self.flush(&mut st);
        let items = std::mem::take(&mut st.items);
        self.build(items, 0)
    }

    fn note(&mut self, n: Note) {
        *self.notes.0.entry(n).or_insert(0) += 1;
    }

    // --------------------------------------------------------------------------------------
    // Output
    // --------------------------------------------------------------------------------------

    fn emit(&mut self, st: &mut State<'a>, obj: VObj) {
        self.flush(st);
        st.last_fill = None;
        st.items.push(Item { obj, clip: st.gs.clip.clone() });
    }

    /// Nest the items into PowerClips by their clips, from clip depth `depth` on.
    fn build(&mut self, items: Vec<Item>, depth: usize) -> Vec<VObj> {
        let mut out = vec![];
        let mut it = items.into_iter().map(|i| (chain(&i.clip), i.obj)).peekable();
        while let Some((ch, obj)) = it.next() {
            if ch.len() <= depth {
                out.push(obj);
                continue;
            }
            let node = ch[depth].clone();
            let mut run = vec![Item { obj, clip: ch.last().cloned() }];
            while let Some((c2, _)) = it.peek() {
                if c2.len() > depth && Rc::ptr_eq(&c2[depth], &node) {
                    let (c2, o2) = it.next().unwrap();
                    run.push(Item { obj: o2, clip: c2.last().cloned() });
                } else {
                    break;
                }
            }
            let contents = self.build(run, depth + 1);
            if contents.is_empty() {
                continue;
            }
            let bounds = vector_core::object::union_bounds(contents.iter().map(|o| o.bounds()));
            let redundant = node.rect.is_some_and(|r| r.inflate(0.5, 0.5).contains_rect(bounds)) || (node.path.elements().is_empty());
            let outside = !node.path.bounding_box().inflate(0.5, 0.5).overlaps(bounds) && bounds.area() > 0.0;
            if outside {
                continue;
            }
            if redundant {
                out.extend(contents);
                continue;
            }
            let mut fst = Style::default();
            fst.outline = Outline::none();
            fst.even_odd = node.even_odd;
            let frame = VObj::new(self.doc.new_id(), ObjKind::Path(node_path(&node.path)), Affine::IDENTITY, fst);
            let mut st = Style::default();
            st.outline = Outline::none();
            out.push(VObj::new(self.doc.new_id(), ObjKind::PowerClip { frame: Box::new(frame), contents }, Affine::IDENTITY, st));
        }
        out
    }

    // --------------------------------------------------------------------------------------
    // Resources
    // --------------------------------------------------------------------------------------

    fn resource(&self, st: &State<'a>, cat: &[u8], name: &[u8]) -> Option<&'a Object> {
        let d = self.pdf.get_dict(st.res?, cat)?;
        self.pdf.get(d, name)
    }

    fn font(&mut self, st: &State<'a>, name: &[u8]) -> Option<Rc<Font<'a>>> {
        let o = self.resource(st, b"Font", name)?;
        let d = self.pdf.dict(o)?;
        let key = d as *const Dictionary as usize;
        if let Some(f) = self.fonts.get(&key) {
            return Some(f.clone());
        }
        let f = Rc::new(Font::load(self.pdf, d));
        if f.unknown_cmap {
            self.note(Note::UnknownCMap(f.name.clone()));
        }
        self.fonts.insert(key, f.clone());
        Some(f)
    }

    // --------------------------------------------------------------------------------------
    // The interpreter
    // --------------------------------------------------------------------------------------

    fn exec(&mut self, ops: &[Op], st: &mut State<'a>, depth: usize) {
        for op in ops {
            self.op(op, st, depth);
        }
    }

    fn op(&mut self, op: &Op, st: &mut State<'a>, depth: usize) {
        let a = &op.args;
        let n = |i: usize| a.get(i).map(num).unwrap_or(0.0);
        let ctm = st.gs.ctm;
        match op.op.as_slice() {
            // graphics state
            b"q" => {
                if st.stack.len() < 256 {
                    st.stack.push(st.gs.clone());
                }
            }
            b"Q" => {
                if let Some(g) = st.stack.pop() {
                    st.gs = g;
                }
            }
            b"cm" => {
                if let Some(m) = affine(&nums(a)) {
                    st.gs.ctm = st.gs.ctm * m;
                }
            }
            b"w" => st.gs.lw = n(0),
            b"J" => {
                st.gs.cap = match n(0) as i64 {
                    1 => LineCap::Round,
                    2 => LineCap::Square,
                    _ => LineCap::Butt,
                }
            }
            b"j" => {
                st.gs.join = match n(0) as i64 {
                    1 => LineJoin::Round,
                    2 => LineJoin::Bevel,
                    _ => LineJoin::Miter,
                }
            }
            b"M" => st.gs.miter = n(0),
            b"d" => {
                st.gs.dash = a.first().and_then(|x| x.as_array().ok()).map(|v| nums(v)).unwrap_or_default();
            }
            b"gs" => {
                if let Some(d) = a.first().and_then(|x| x.as_name().ok()).and_then(|nm| self.resource(st, b"ExtGState", nm)).and_then(|o| self.pdf.dict(o)) {
                    self.ext_gstate(st, d);
                }
            }
            b"ri" | b"i" => {}
            // path construction
            b"m" => {
                let p = ctm * Point::new(n(0), n(1));
                st.path.move_to(p);
                st.cur = p;
                st.start = p;
            }
            b"l" => {
                let p = ctm * Point::new(n(0), n(1));
                if st.path.elements().is_empty() {
                    st.path.move_to(p);
                    st.start = p;
                } else {
                    st.path.line_to(p);
                }
                st.cur = p;
            }
            b"c" => {
                let (p1, p2, p3) = (ctm * Point::new(n(0), n(1)), ctm * Point::new(n(2), n(3)), ctm * Point::new(n(4), n(5)));
                self.curve(st, p1, p2, p3);
            }
            b"v" => {
                let (p2, p3) = (ctm * Point::new(n(0), n(1)), ctm * Point::new(n(2), n(3)));
                let p1 = st.cur;
                self.curve(st, p1, p2, p3);
            }
            b"y" => {
                let (p1, p3) = (ctm * Point::new(n(0), n(1)), ctm * Point::new(n(2), n(3)));
                self.curve(st, p1, p3, p3);
            }
            b"h" => {
                if !st.path.elements().is_empty() {
                    st.path.close_path();
                    st.cur = st.start;
                }
            }
            b"re" => {
                let (x, y, w, h) = (n(0), n(1), n(2), n(3));
                let p0 = ctm * Point::new(x, y);
                st.path.move_to(p0);
                st.path.line_to(ctm * Point::new(x + w, y));
                st.path.line_to(ctm * Point::new(x + w, y + h));
                st.path.line_to(ctm * Point::new(x, y + h));
                st.path.close_path();
                st.cur = p0;
                st.start = p0;
            }
            // painting
            b"f" | b"F" => self.paint(st, true, false, false),
            b"f*" => self.paint(st, true, false, true),
            b"S" => self.paint(st, false, true, false),
            b"s" => {
                st.path.close_path();
                self.paint(st, false, true, false)
            }
            b"B" => self.paint(st, true, true, false),
            b"B*" => self.paint(st, true, true, true),
            b"b" => {
                st.path.close_path();
                self.paint(st, true, true, false)
            }
            b"b*" => {
                st.path.close_path();
                self.paint(st, true, true, true)
            }
            b"n" => self.paint(st, false, false, false),
            b"W" => st.pending_clip = Some(false),
            b"W*" => st.pending_clip = Some(true),
            // color
            b"CS" | b"cs" => {
                let stroke = op.op == b"CS";
                if st.uncolored {
                    return;
                }
                let cs = a.first().and_then(|o| Cs::parse(self.pdf, o, st.res)).unwrap_or(Cs::Gray);
                let init = cs.initial();
                if stroke {
                    st.gs.stroke_cs = Rc::new(cs);
                    st.gs.stroke = init;
                    st.gs.stroke_pat = None;
                } else {
                    st.gs.fill_cs = Rc::new(cs);
                    st.gs.fill = init;
                    st.gs.fill_pat = None;
                }
            }
            b"SC" | b"SCN" | b"sc" | b"scn" => {
                if st.uncolored {
                    return;
                }
                let stroke = op.op[0] == b'S';
                let comps = nums(a);
                let pat = a.last().and_then(|o| o.as_name().ok()).and_then(|nm| self.resource(st, b"Pattern", nm));
                if stroke {
                    st.gs.stroke = comps;
                    st.gs.stroke_pat = pat;
                } else {
                    st.gs.fill = comps;
                    st.gs.fill_pat = pat;
                }
            }
            b"G" | b"g" | b"RG" | b"rg" | b"K" | b"k" => {
                if st.uncolored {
                    return;
                }
                let cs = match op.op[0].to_ascii_lowercase() {
                    b'g' => Cs::Gray,
                    b'r' => Cs::Rgb,
                    _ => Cs::Cmyk,
                };
                let stroke = op.op[0].is_ascii_uppercase();
                if stroke {
                    st.gs.stroke_cs = Rc::new(cs);
                    st.gs.stroke = nums(a);
                    st.gs.stroke_pat = None;
                } else {
                    st.gs.fill_cs = Rc::new(cs);
                    st.gs.fill = nums(a);
                    st.gs.fill_pat = None;
                }
            }
            // shading, images, forms
            b"sh" => {
                if let Some(sh) = a.first().and_then(|x| x.as_name().ok()).and_then(|nm| self.resource(st, b"Shading", nm)) {
                    self.shade(st, sh);
                }
            }
            b"Do" => {
                if let Some(nm) = a.first().and_then(|x| x.as_name().ok()) {
                    if let Some(o) = self.resource(st, b"XObject", nm) {
                        self.xobject(st, o, &String::from_utf8_lossy(nm), depth);
                    }
                }
            }
            b"BI" => {
                if let (Some(Object::Dictionary(d)), Some(Object::String(data, _))) = (a.first(), a.get(1)) {
                    let d = image::expand_inline(d);
                    self.image(st, &d, data, None, "Inline image");
                }
            }
            // text
            b"BT" => {
                st.tm = Affine::IDENTITY;
                st.tlm = Affine::IDENTITY;
            }
            b"ET" => self.flush(st),
            b"Tc" => st.gs.tc = n(0),
            b"Tw" => st.gs.tw = n(0),
            b"Tz" => st.gs.th = n(0) / 100.0,
            b"TL" => st.gs.tl = n(0),
            b"Ts" => st.gs.rise = n(0),
            b"Tr" => st.gs.tr = n(0) as i64,
            b"Tf" => {
                st.gs.font = a.first().and_then(|x| x.as_name().ok()).and_then(|nm| self.font(st, nm));
                st.gs.size = n(1);
            }
            b"Td" => {
                st.tlm = st.tlm * Affine::translate((n(0), n(1)));
                st.tm = st.tlm;
            }
            b"TD" => {
                st.gs.tl = -n(1);
                st.tlm = st.tlm * Affine::translate((n(0), n(1)));
                st.tm = st.tlm;
            }
            b"Tm" => {
                if let Some(m) = affine(&nums(a)) {
                    st.tlm = m;
                    st.tm = m;
                }
            }
            b"T*" => {
                st.tlm = st.tlm * Affine::translate((0.0, -st.gs.tl));
                st.tm = st.tlm;
            }
            b"Tj" => {
                if let Some(s) = a.first().and_then(|x| x.as_str().ok()) {
                    self.show(st, s, depth);
                }
            }
            b"'" => {
                st.tlm = st.tlm * Affine::translate((0.0, -st.gs.tl));
                st.tm = st.tlm;
                if let Some(s) = a.first().and_then(|x| x.as_str().ok()) {
                    self.show(st, s, depth);
                }
            }
            b"\"" => {
                st.gs.tw = n(0);
                st.gs.tc = n(1);
                st.tlm = st.tlm * Affine::translate((0.0, -st.gs.tl));
                st.tm = st.tlm;
                if let Some(s) = a.get(2).and_then(|x| x.as_str().ok()) {
                    self.show(st, s, depth);
                }
            }
            b"TJ" => {
                if let Some(arr) = a.first().and_then(|x| x.as_array().ok()) {
                    for e in arr {
                        match e {
                            Object::String(s, _) => self.show(st, s, depth),
                            Object::Integer(_) | Object::Real(_) => {
                                let adj = -num(e) / 1000.0 * st.gs.size;
                                let f = st.gs.font.as_ref().is_some_and(|f| f.vertical);
                                st.tm = if f { st.tm * Affine::translate((0.0, adj)) } else { st.tm * Affine::translate((adj * st.gs.th, 0.0)) };
                            }
                            _ => {}
                        }
                    }
                }
            }
            b"d0" => {}
            b"d1" => st.uncolored = true,
            _ => {}
        }
    }

    fn curve(&mut self, st: &mut State<'a>, p1: Point, p2: Point, p3: Point) {
        if st.path.elements().is_empty() {
            st.path.move_to(st.cur);
            st.start = st.cur;
        }
        st.path.curve_to(p1, p2, p3);
        st.cur = p3;
    }

    fn ext_gstate(&mut self, st: &mut State<'a>, d: &'a Dictionary) {
        let pdf = self.pdf;
        if let Some(v) = pdf.num(d, b"LW") {
            st.gs.lw = v;
        }
        if let Some(v) = pdf.int(d, b"LC") {
            st.gs.cap = match v {
                1 => LineCap::Round,
                2 => LineCap::Square,
                _ => LineCap::Butt,
            };
        }
        if let Some(v) = pdf.int(d, b"LJ") {
            st.gs.join = match v {
                1 => LineJoin::Round,
                2 => LineJoin::Bevel,
                _ => LineJoin::Miter,
            };
        }
        if let Some(v) = pdf.num(d, b"ML") {
            st.gs.miter = v;
        }
        if let Some(Object::Array(dash)) = pdf.get(d, b"D") {
            st.gs.dash = dash.first().map(|x| pdf.deref(x)).and_then(|x| x.as_array().ok()).map(|v| v.iter().map(|x| num(pdf.deref(x))).collect()).unwrap_or_default();
        }
        if let Some(v) = pdf.num(d, b"ca") {
            st.gs.ca = v.clamp(0.0, 1.0);
        }
        if let Some(v) = pdf.num(d, b"CA") {
            st.gs.ca_stroke = v.clamp(0.0, 1.0);
        }
        if let Some(bm) = pdf.get(d, b"BM") {
            let name = match bm {
                Object::Array(a) => a.first().and_then(|x| pdf.deref(x).as_name().ok()),
                o => o.as_name().ok(),
            };
            st.gs.blend = match name.unwrap_or(b"Normal") {
                b"Multiply" => MergeMode::Multiply,
                b"Screen" => MergeMode::Screen,
                b"Overlay" => MergeMode::Overlay,
                b"Darken" => MergeMode::Darken,
                b"Lighten" => MergeMode::Lighten,
                b"ColorDodge" => MergeMode::ColorDodge,
                b"ColorBurn" => MergeMode::ColorBurn,
                b"HardLight" => MergeMode::HardLight,
                b"SoftLight" => MergeMode::SoftLight,
                b"Difference" => MergeMode::Difference,
                b"Exclusion" => MergeMode::Exclusion,
                b"Hue" => MergeMode::Hue,
                b"Saturation" => MergeMode::Saturation,
                b"Color" => MergeMode::Color,
                b"Luminosity" => MergeMode::Luminosity,
                _ => MergeMode::Normal,
            };
        }
        if let Some(sm) = pdf.get(d, b"SMask") {
            if !matches!(sm, Object::Name(n) if n == b"None") {
                self.note(Note::SoftMask);
            }
        }
        if let Some(Object::Array(f)) = pdf.get(d, b"Font") {
            if let (Some(fd), Some(size)) = (f.first().and_then(|x| pdf.dict(x)), f.get(1)) {
                let key = fd as *const Dictionary as usize;
                let font = match self.fonts.get(&key) {
                    Some(f) => f.clone(),
                    None => {
                        let fnt = Rc::new(Font::load(pdf, fd));
                        self.fonts.insert(key, fnt.clone());
                        fnt
                    }
                };
                st.gs.font = Some(font);
                st.gs.size = num(pdf.deref(size));
            }
        }
    }

    // --------------------------------------------------------------------------------------
    // Paths
    // --------------------------------------------------------------------------------------

    /// The fill of the current fill (or stroke) color, for an object with bounds `bb`.
    fn paint_of(&mut self, st: &State<'a>, stroke: bool, bb: Rect) -> Fill {
        let (cs, comps, pat, a) = if stroke { (&st.gs.stroke_cs, &st.gs.stroke, st.gs.stroke_pat, st.gs.ca_stroke) } else { (&st.gs.fill_cs, &st.gs.fill, st.gs.fill_pat, st.gs.ca) };
        let cs = cs.clone();
        if let Cs::Pattern(base) = cs.as_ref() {
            let Some(p) = pat else { return Fill::None };
            let Some(pd) = self.pdf.dict(p) else { return Fill::None };
            let m = affine(&self.pdf.nums(pd, b"Matrix")).unwrap_or(Affine::IDENTITY);
            return match self.pdf.int(pd, b"PatternType") {
                Some(2) => match self.pdf.get(pd, b"Shading") {
                    Some(sh) => self.shading_fill(sh, st.base * m, bb, a).unwrap_or(Fill::None),
                    None => Fill::None,
                },
                _ => {
                    // tiling pattern: a flat color
                    self.note(Note::TilingPattern);
                    if self.pdf.int(pd, b"PaintType") == Some(2) {
                        let c = base.as_ref().map(|b| b.to_color(comps)).unwrap_or(Color::BLACK);
                        Fill::Solid(alpha(c, a))
                    } else {
                        let c = self.tile_color(p, st).unwrap_or(Color::rgb(128, 128, 128));
                        Fill::Solid(alpha(c, a))
                    }
                }
            };
        }
        if let Cs::Separation { name, .. } = cs.as_ref() {
            if name != "All" && name != "None" && !self.spots.contains_key(name) {
                let full = cs.to_color(&[1.0]);
                self.spots.insert(name.clone(), full);
            }
        }
        let c = cs.to_color(comps);
        if c.a == 0 {
            return Fill::None;
        }
        Fill::Solid(alpha(c, a))
    }

    /// The color of the first thing a colored tiling pattern's cell paints.
    fn tile_color(&mut self, p: &'a Object, st: &State<'a>) -> Option<Color> {
        let key = p as *const Object as usize;
        if self.forms.contains(&key) || self.forms.len() > 16 {
            return None;
        }
        let data = self.pdf.stream_data(p)?;
        let pd = self.pdf.dict(p)?;
        let mut sub = State::new(self.pdf.get_dict(pd, b"Resources").or(st.res), Affine::IDENTITY, Gs::new(Affine::IDENTITY));
        self.forms.push(key);
        let ops = lexer::parse(&data);
        self.exec(&ops, &mut sub, self.forms.len());
        self.flush(&mut sub);
        self.forms.pop();
        let mut found = None;
        let mut picture = None;
        for it in &sub.items {
            it.obj.walk(&mut |o| {
                if found.is_none() {
                    found = o.style.fill.main_color().or(o.style.outline.color);
                }
                if let ObjKind::Image(im) = &o.kind {
                    picture.get_or_insert(im.image);
                }
            });
            if found.is_some() {
                break;
            }
        }
        // a cell that only paints a picture: the picture's average color
        found.or_else(|| picture.and_then(|id| self.doc.images.get(&id)).and_then(average_color))
    }

    fn outline(&mut self, st: &State<'a>, bb: Rect) -> Outline {
        let s = scale_of(st.gs.ctm);
        let width = st.gs.lw * s;
        let color = match self.paint_of(st, true, bb) {
            Fill::Solid(c) => Some(c),
            Fill::Gradient(g) => Some(g.color_at(0.5)),
            _ => None,
        };
        let mut o = Outline { width, color, join: st.gs.join, cap: st.gs.cap, miter_limit: st.gs.miter.max(1.0), ..Outline::default() };
        if !st.gs.dash.is_empty() && st.gs.dash.iter().any(|d| *d > 0.0) {
            let unit = if width > 1e-9 { width } else { 1.0 };
            o.dash = st.gs.dash.iter().map(|d| d * s / unit).collect();
            if o.dash.len() % 2 == 1 {
                let d = o.dash.clone();
                o.dash.extend(d);
            }
        }
        o
    }

    fn paint(&mut self, st: &mut State<'a>, fill: bool, stroke: bool, even_odd: bool) {
        let path = std::mem::take(&mut st.path);
        if (fill || stroke) && !path.elements().is_empty() {
            let bb = path.bounding_box();
            let joins = !fill && stroke && st.last_fill.as_ref().is_some_and(|p| *p == path) && st.run.is_none();
            if joins {
                let ol = self.outline(st, bb);
                if let Some(last) = st.items.last_mut() {
                    last.obj.style.outline = ol;
                }
                st.last_fill = None;
            } else {
                let mut style = Style::default();
                style.fill = if fill { self.paint_of(st, false, bb) } else { Fill::None };
                style.outline = if stroke { self.outline(st, bb) } else { Outline::none() };
                style.even_odd = even_odd;
                style.merge = st.gs.blend;
                if !style.fill.is_none() || style.outline.color.is_some() {
                    let obj = VObj::new(self.doc.new_id(), ObjKind::Path(node_path(&path)), Affine::IDENTITY, style);
                    self.emit(st, obj);
                    if fill && !stroke {
                        st.last_fill = Some(path.clone());
                    }
                }
            }
        }
        if let Some(eo) = st.pending_clip.take() {
            let rect = rect_of(&path);
            st.gs.clip = Some(Rc::new(Clip { path, even_odd: eo, parent: st.gs.clip.clone(), rect }));
        }
    }

    // --------------------------------------------------------------------------------------
    // Shadings
    // --------------------------------------------------------------------------------------

    fn shade(&mut self, st: &mut State<'a>, sh: &'a Object) {
        // painted inside the current clip: the innermost clip is the shape
        let (shape, clip) = match &st.gs.clip {
            Some(c) => (c.path.clone(), c.parent.clone()),
            None => {
                let r = self.page_rect;
                (r.to_path(0.1), None)
            }
        };
        let bb = shape.bounding_box();
        let Some(fill) = self.shading_fill(sh, st.gs.ctm, bb, st.gs.ca) else { return };
        let mut style = Style::default();
        style.fill = fill;
        style.outline = Outline::none();
        style.merge = st.gs.blend;
        if let Some(c) = &st.gs.clip {
            style.even_odd = c.even_odd;
        }
        let obj = VObj::new(self.doc.new_id(), ObjKind::Path(node_path(&shape)), Affine::IDENTITY, style);
        self.flush(st);
        st.last_fill = None;
        st.items.push(Item { obj, clip });
    }

    /// A shading as a fill for an object with bounds `bb`; `m` maps shading space to the
    /// document.
    fn shading_fill(&mut self, sh: &'a Object, m: Affine, bb: Rect, a: f64) -> Option<Fill> {
        let pdf = self.pdf;
        let d = pdf.dict(sh)?;
        let ty = pdf.int(d, b"ShadingType")?;
        let cs = d.get(b"ColorSpace").ok().and_then(|c| Cs::parse(pdf, c, None)).unwrap_or(Cs::Rgb);
        let func = d.get(b"Function").ok().and_then(|f| Function::parse(pdf, f));
        let color_at = |t: f64| -> Color {
            let comps = match &func {
                Some(f) => f.eval(&[t]),
                None => vec![t],
            };
            alpha(cs.to_color(&comps), a)
        };
        if ty != 2 && ty != 3 {
            // function-based and mesh shadings: their average color
            self.note(Note::MeshShading);
            let c = match ty {
                1 => {
                    let dom = pdf.nums(d, b"Domain");
                    let (x, y) = if dom.len() >= 4 { ((dom[0] + dom[1]) / 2.0, (dom[2] + dom[3]) / 2.0) } else { (0.5, 0.5) };
                    func.as_ref().map(|f| alpha(cs.to_color(&f.eval(&[x, y])), a))
                }
                _ => mesh_average(pdf, sh, &cs, func.as_ref()).map(|c| alpha(c, a)),
            };
            return Some(Fill::Solid(c.unwrap_or(Color::rgb(128, 128, 128))));
        }
        let dom = pdf.nums(d, b"Domain");
        let (t0, t1) = if dom.len() >= 2 { (dom[0], dom[1]) } else { (0.0, 1.0) };
        let coords = pdf.nums(d, b"Coords");
        let w = bb.width().max(1e-6);
        let h = bb.height().max(1e-6);
        let unit = |p: Point| Point::new((p.x - bb.x0) / w, (p.y - bb.y0) / h);
        // sample positions (0..1 along the gradient), with the function's hard steps
        let mut ts: Vec<f64> = (0..=24).map(|i| i as f64 / 24.0).collect();
        if let Some(f) = &func {
            for b in f.breaks() {
                let t = (b - t0) / (t1 - t0);
                if t > 0.0 && t < 1.0 {
                    ts.push(t - 1e-4);
                    ts.push(t + 1e-4);
                }
            }
        }
        ts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        let mut g;
        let pos_of: Box<dyn Fn(f64) -> f64>;
        if ty == 2 {
            if coords.len() < 4 {
                return None;
            }
            let p0 = m * Point::new(coords[0], coords[1]);
            let p1 = m * Point::new(coords[2], coords[3]);
            let dd = p1 - p0;
            let l2 = dd.hypot2().max(1e-12);
            let v = Vec2::new(dd.x * w / l2, dd.y * h / l2);
            let dv = v / v.hypot2().max(1e-12);
            let s = unit(p0);
            g = Gradient::linear(Color::BLACK, Color::WHITE);
            g.start = s;
            g.end = s + dv;
            pos_of = Box::new(|t| t);
        } else {
            if coords.len() < 6 {
                return None;
            }
            let (c0, r0, c1, r1) = (Point::new(coords[0], coords[1]), coords[2].max(0.0), Point::new(coords[3], coords[4]), coords[5].max(0.0));
            let big = r0.max(r1).max(1e-9);
            let center = if r1 >= r0 { c1 } else { c0 };
            let rd = big * scale_of(m);
            let s = unit(m * center);
            g = Gradient::radial(Color::BLACK, Color::WHITE);
            g.start = s;
            g.end = s + Vec2::new(rd / (w * h).sqrt(), 0.0);
            pos_of = Box::new(move |t| (r0 + t * (r1 - r0)) / big);
        }
        let mut stops: Vec<GradientStop> = ts.iter().map(|t| GradientStop { pos: pos_of(*t).clamp(0.0, 1.0), color: color_at(t0 + t * (t1 - t0)) }).collect();
        stops.sort_by(|x, y| x.pos.partial_cmp(&y.pos).unwrap_or(std::cmp::Ordering::Equal));
        g.stops = simplify(stops);
        Some(Fill::Gradient(g))
    }

    // --------------------------------------------------------------------------------------
    // XObjects
    // --------------------------------------------------------------------------------------

    fn xobject(&mut self, st: &mut State<'a>, o: &'a Object, name: &str, depth: usize) {
        let Object::Stream(s) = o else { return };
        let d = &s.dict;
        match self.pdf.name(d, b"Subtype").as_deref() {
            Some("Image") => {
                let key = o as *const Object as usize;
                self.image(st, d, &s.content, Some(key), name);
            }
            Some("Form") => self.form(st, o, d, depth),
            _ => {}
        }
    }

    fn form(&mut self, st: &mut State<'a>, o: &'a Object, d: &'a Dictionary, depth: usize) {
        let key = o as *const Object as usize;
        if self.forms.contains(&key) || depth > 24 {
            return;
        }
        let Some(data) = self.pdf.stream_data(o) else { return };
        self.flush(st);
        st.last_fill = None;
        let m = affine(&self.pdf.nums(d, b"Matrix")).unwrap_or(Affine::IDENTITY);
        let res = self.pdf.get_dict(d, b"Resources").or(st.res);
        let mut gs = st.gs.clone();
        gs.ctm = gs.ctm * m;
        let group = self.pdf.get(d, b"Group").and_then(|g| self.pdf.dict(g)).is_some_and(|g| self.pdf.name(g, b"S").as_deref() == Some("Transparency"));
        let wrap = group && (st.gs.ca < 0.999 || st.gs.blend != MergeMode::Normal);
        let (ga, gblend) = (st.gs.ca, st.gs.blend);
        if wrap {
            gs.ca = 1.0;
            gs.ca_stroke = 1.0;
            gs.blend = MergeMode::Normal;
        }
        // the form's bounding box clips it
        let bbox = self.pdf.nums(d, b"BBox");
        if bbox.len() == 4 {
            let r = Rect::new(bbox[0], bbox[1], bbox[2], bbox[3]);
            let p = gs.ctm * r.to_path(0.1);
            let rect = rect_of(&p);
            gs.clip = Some(Rc::new(Clip { path: p, even_odd: false, parent: gs.clip.clone(), rect }));
        }
        let mut sub = State::new(res, gs.ctm, gs);
        self.forms.push(key);
        let ops = lexer::parse(&data);
        self.exec(&ops, &mut sub, depth + 1);
        self.flush(&mut sub);
        self.forms.pop();
        if !wrap {
            st.items.append(&mut sub.items);
            return;
        }
        let base_depth = chain(&st.gs.clip).len();
        let mut objs = self.build(std::mem::take(&mut sub.items), base_depth);
        if objs.is_empty() {
            return;
        }
        let tr = (1.0 - ga) * 100.0;
        let obj = if objs.len() == 1 && objs[0].style.transparency == Transparency::None && objs[0].style.merge == MergeMode::Normal {
            let mut o = objs.pop().unwrap();
            if tr > 0.05 {
                o.style.transparency = Transparency::Uniform(tr);
            }
            o.style.merge = gblend;
            o
        } else {
            let mut style = Style::default();
            style.outline = Outline::none();
            if tr > 0.05 {
                style.transparency = Transparency::Uniform(tr);
            }
            style.merge = gblend;
            VObj::new(self.doc.new_id(), ObjKind::Group(objs), Affine::IDENTITY, style)
        };
        st.items.push(Item { obj, clip: st.gs.clip.clone() });
    }

    fn image(&mut self, st: &mut State<'a>, d: &Dictionary, raw: &[u8], key: Option<usize>, name: &str) {
        let is_mask = self.pdf.get(d, b"ImageMask").and_then(|o| o.as_bool().ok()).unwrap_or(false);
        let fill = if is_mask {
            match self.paint_of(st, false, Rect::new(0.0, 0.0, 1.0, 1.0)) {
                Fill::Solid(c) => c.with_alpha(255),
                Fill::Gradient(g) => g.color_at(0.5).with_alpha(255),
                _ => Color::BLACK,
            }
        } else {
            Color::BLACK
        };
        let cached = key.and_then(|k| self.images.get(&(k, fill)).copied());
        let id = match cached {
            Some(id) => id,
            None => {
                let id = match image::decode(self.pdf, d, raw, st.res, fill, name) {
                    Ok(img) => Some(self.doc.add_image(img)),
                    Err(e) => {
                        self.note(Note::Image(e));
                        None
                    }
                };
                if let Some(k) = key {
                    self.images.insert((k, fill), id);
                }
                id
            }
        };
        let Some(id) = id else { return };
        // the unit square, in the document, top row first
        let m = st.gs.ctm;
        let w = (m * Point::new(1.0, 0.0) - m * Point::ORIGIN).hypot().max(1e-6);
        let h = (m * Point::new(0.0, 1.0) - m * Point::ORIGIN).hypot().max(1e-6);
        let xf = m * Affine::new([1.0 / w, 0.0, 0.0, -1.0 / h, 0.0, 1.0]);
        let mut style = Style::default();
        style.outline = Outline::none();
        style.merge = st.gs.blend;
        let a = st.gs.ca;
        if a < 0.999 && !is_mask {
            style.transparency = Transparency::Uniform((1.0 - a) * 100.0);
        }
        if is_mask && a < 0.999 {
            style.transparency = Transparency::Uniform((1.0 - a) * 100.0);
        }
        let mut obj = VObj::new(self.doc.new_id(), ObjKind::Image(ImageObj { image: id, w, h, crop: [0.0, 0.0, 1.0, 1.0], adjust: ImageAdjust::default() }), xf, style);
        obj.name = name.to_string();
        self.emit(st, obj);
    }

    // --------------------------------------------------------------------------------------
    // Text
    // --------------------------------------------------------------------------------------

    fn show(&mut self, st: &mut State<'a>, s: &[u8], depth: usize) {
        let Some(font) = st.gs.font.clone() else { return };
        let size = st.gs.size;
        for g in font.decode(s) {
            let w0 = font.width(&g);
            let trm = st.gs.ctm * st.tm * Affine::new([size * st.gs.th, 0.0, 0.0, size, 0.0, st.gs.rise]);
            self.glyph(st, &font, &g, trm, depth);
            let extra = st.gs.tc + if g.space { st.gs.tw } else { 0.0 };
            if font.vertical {
                st.tm = st.tm * Affine::translate((0.0, -(size + extra)));
            } else {
                st.tm = st.tm * Affine::translate(((w0 * size + extra) * st.gs.th, 0.0));
            }
            if let Some(r) = &mut st.run {
                r.end = st.gs.ctm * st.tm * Point::new(0.0, st.gs.rise);
            }
        }
    }

    /// One glyph: the text it continues, or curves, or (Type 3) the objects its procedure paints.
    ///
    /// Text comes in as editable artistic text when its font is installed; a font that isn't
    /// installed comes in as curves from its embedded program, or as text (shown in a
    /// substitute) when the program can't be read. With "Import text as curves" every glyph is
    /// a curve: from the embedded program, else from the installed font that stands in.
    fn glyph(&mut self, st: &mut State<'a>, font: &Rc<Font<'a>>, g: &Glyph, trm: Affine, depth: usize) {
        let mode = st.gs.tr & 3;
        if st.gs.tr >= 4 {
            self.note(Note::TextClip);
        }
        if mode == 3 {
            self.note(Note::InvisibleText);
            return;
        }
        if let Some(t3) = &font.type3 {
            self.type3(st, font, t3.matrix, g, trm, depth);
            return;
        }
        let text = font.unicode(g);
        let fill = mode == 0 || mode == 2;
        let stroke = mode == 1 || mode == 2;
        let embedded = font.program.is_some();
        let curves = self.opts.text_as_curves || (!font.installed && embedded);
        let outline: Option<BezPath> = if curves {
            if embedded {
                font.outline(g).map(|p| (*p).clone())
            } else {
                font.installed_outline(&text)
            }
        } else {
            None
        };
        if curves && !embedded {
            self.note(Note::CurvesFromInstalled(font.name.clone()));
        }
        if curves && embedded && !self.opts.text_as_curves {
            self.note(Note::CurvesNotInstalled(font.name.clone()));
        }
        if !curves && !font.installed {
            self.note(Note::FontSubstituted(font.name.clone()));
        }
        if font.vertical {
            self.note(Note::VerticalText);
        }
        let origin = trm * Point::ORIGIN;
        let dir = trm * Point::new(1.0, 0.0) - origin;
        let dir = if dir.hypot() > 1e-9 { dir / dir.hypot() } else { Vec2::new(1.0, 0.0) };
        let up = trm * Point::new(0.0, 1.0) - origin;
        let em = up.hypot().max(1e-6);
        // does it continue the current run?
        let fits = st.run.as_ref().is_some_and(|r| {
            Rc::ptr_eq(&r.font, font)
                && same_clip(&r.clip, &st.gs.clip)
                && r.curves.is_some() == curves
                && (r.em - em).abs() < 1e-3 * em
                && (r.dir - dir).hypot() < 1e-3
                && {
                    let d = origin - r.end;
                    let along = d.dot(dir);
                    let across = d.cross(dir).abs();
                    across < 0.05 * em && along > -0.3 * em && along < 1.5 * em
                }
        });
        let style = self.text_style(st, fill, stroke);
        let fits = fits && st.run.as_ref().is_some_and(|r| r.style == style);
        if !fits {
            self.flush(st);
            st.last_fill = None;
            let size = em;
            let xf = trm * Affine::scale_non_uniform(1.0 / size, -1.0 / size);
            st.run = Some(Run { font: font.clone(), style, clip: st.gs.clip.clone(), xf, size, text: String::new(), curves: curves.then(BezPath::new), end: origin, dir, em });
        }
        let r = st.run.as_mut().unwrap();
        match &mut r.curves {
            Some(path) => {
                if let Some(o) = outline {
                    path.extend((trm * o).elements().iter().copied());
                }
            }
            None => {
                let gap = (origin - r.end).dot(dir);
                if !r.text.is_empty() && gap > 0.2 * em && !r.text.ends_with(char::is_whitespace) && !text.starts_with(char::is_whitespace) {
                    r.text.push(' ');
                }
                r.text.push_str(&text);
            }
        }
        r.end = origin;
    }

    fn text_style(&mut self, st: &State<'a>, fill: bool, stroke: bool) -> Style {
        let mut style = Style::default();
        let bb = Rect::new(0.0, 0.0, 1.0, 1.0);
        style.fill = if fill {
            match self.paint_of(st, false, bb) {
                Fill::Gradient(g) => Fill::Solid(g.color_at(0.5)),
                f => f,
            }
        } else {
            Fill::None
        };
        style.outline = if stroke { self.outline(st, bb) } else { Outline::none() };
        style.even_odd = false;
        style.merge = st.gs.blend;
        style
    }

    fn type3(&mut self, st: &mut State<'a>, font: &Rc<Font<'a>>, matrix: Affine, g: &Glyph, trm: Affine, depth: usize) {
        let Some(t3) = &font.type3 else { return };
        let Some(name) = font.glyph_name(g.code) else { return };
        let Some(proc_) = self.pdf.get(t3.procs, name.as_bytes()) else { return };
        let key = proc_ as *const Object as usize;
        if self.forms.contains(&key) || depth > 24 {
            return;
        }
        let Some(data) = self.pdf.stream_data(proc_) else { return };
        self.flush(st);
        st.last_fill = None;
        let mut gs = st.gs.clone();
        gs.ctm = trm * matrix;
        let res = t3.resources.or(st.res);
        let mut sub = State::new(res, gs.ctm, gs);
        self.forms.push(key);
        let ops = lexer::parse(&data);
        self.exec(&ops, &mut sub, depth + 1);
        self.flush(&mut sub);
        self.forms.pop();
        st.items.append(&mut sub.items);
    }

    /// Turn the pending text run into an object.
    fn flush(&mut self, st: &mut State<'a>) {
        let Some(r) = st.run.take() else { return };
        let obj = match r.curves {
            Some(path) => {
                if path.elements().is_empty() {
                    return;
                }
                let mut style = r.style;
                style.even_odd = false;
                VObj::new(self.doc.new_id(), ObjKind::Path(node_path(&path)), Affine::IDENTITY, style)
            }
            None => {
                let text = r.text.trim_end().to_string();
                if text.trim().is_empty() {
                    return;
                }
                let mut fmt = CharFormat::default();
                fmt.font = r.font.family.clone();
                fmt.bold = r.font.bold;
                fmt.italic = r.font.italic;
                fmt.size = (r.size * 100.0).round() / 100.0;
                let t = TextObj::artistic(&text, fmt);
                // keep the transform exact: the size was rounded
                let xf = r.xf * Affine::scale(fmt_scale(r.size));
                let mut o = VObj::new(self.doc.new_id(), ObjKind::Text(t), xf, r.style);
                o.style.even_odd = false;
                o
            }
        };
        st.items.push(Item { obj, clip: r.clip });
    }
}

/// The scale that makes up for rounding a font size to hundredths.
fn fmt_scale(size: f64) -> f64 {
    let rounded = (size * 100.0).round() / 100.0;
    if rounded > 0.0 {
        size / rounded
    } else {
        1.0
    }
}

impl<'a> State<'a> {
    fn new(res: Option<&'a Dictionary>, base: Affine, gs: Gs<'a>) -> Self {
        State {
            res,
            base,
            gs,
            stack: vec![],
            path: BezPath::new(),
            cur: Point::ORIGIN,
            start: Point::ORIGIN,
            pending_clip: None,
            tm: Affine::IDENTITY,
            tlm: Affine::IDENTITY,
            items: vec![],
            run: None,
            last_fill: None,
            uncolored: false,
        }
    }
}

/// Drop the stops a straight blend of their neighbours already gives.
fn simplify(stops: Vec<GradientStop>) -> Vec<GradientStop> {
    if stops.len() <= 2 {
        return stops;
    }
    let mut out = vec![stops[0]];
    for i in 1..stops.len() - 1 {
        let (a, b, c) = (*out.last().unwrap(), stops[i], stops[i + 1]);
        let span = c.pos - a.pos;
        let t = if span > 1e-9 { (b.pos - a.pos) / span } else { 0.0 };
        let mid = Color::lerp(a.color, c.color, t);
        let diff = [(mid.r, b.color.r), (mid.g, b.color.g), (mid.b, b.color.b), (mid.a, b.color.a)].iter().map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0);
        if diff > 1 {
            out.push(b);
        }
    }
    out.push(*stops.last().unwrap());
    // stops at the same place keep only a hard step
    out.dedup_by(|b, a| (a.pos - b.pos).abs() < 1e-9 && a.color == b.color);
    out
}

/// The average color of a mesh shading's vertices (types 4 to 7).
fn mesh_average(pdf: &Pdf, sh: &Object, cs: &Cs, func: Option<&Function>) -> Option<Color> {
    let d = pdf.dict(sh)?;
    let ty = pdf.int(d, b"ShadingType")?;
    let data = pdf.stream_data(sh)?;
    let bpc = pdf.int(d, b"BitsPerCoordinate").unwrap_or(16) as u32;
    let bcomp = pdf.int(d, b"BitsPerComponent").unwrap_or(8) as u32;
    let bflag = pdf.int(d, b"BitsPerFlag").unwrap_or(8) as u32;
    let decode = pdf.nums(d, b"Decode");
    let ncomp = if func.is_some() { 1 } else { cs.ncomps().max(1) };
    if decode.len() < 4 + 2 * ncomp || bpc == 0 || bcomp == 0 {
        return None;
    }
    let mut pos = 0usize;
    let read = |pos: &mut usize, bits: u32| -> Option<u64> {
        let mut v = 0u64;
        for _ in 0..bits {
            let byte = *data.get(*pos / 8)?;
            v = (v << 1) | ((byte >> (7 - (*pos % 8))) & 1) as u64;
            *pos += 1;
        }
        Some(v)
    };
    let maxc = ((1u64 << bcomp.min(32)) - 1) as f64;
    let mut sum = [0f64; 4];
    let mut count = 0usize;
    let mut color = |pos: &mut usize| -> Option<()> {
        let comps: Vec<f64> = (0..ncomp).map(|k| read(pos, bcomp).map(|v| decode[4 + 2 * k] + v as f64 / maxc * (decode[5 + 2 * k] - decode[4 + 2 * k]))).collect::<Option<_>>()?;
        let c = match func {
            Some(f) => cs.to_color(&f.eval(&comps)),
            None => cs.to_color(&comps),
        };
        sum[0] += c.r as f64;
        sum[1] += c.g as f64;
        sum[2] += c.b as f64;
        count += 1;
        Some(())
    };
    let align = |pos: &mut usize| *pos = pos.div_ceil(8) * 8;
    let per_row = pdf.int(d, b"VerticesPerRow").unwrap_or(2).max(2) as usize;
    for _ in 0..100_000 {
        match ty {
            4 => {
                read(&mut pos, bflag)?;
                read(&mut pos, bpc * 2)?;
                color(&mut pos)?;
                align(&mut pos);
            }
            5 => {
                for _ in 0..per_row {
                    read(&mut pos, bpc * 2)?;
                    color(&mut pos)?;
                    align(&mut pos);
                }
            }
            6 | 7 => {
                let flag = read(&mut pos, bflag)?;
                let pts = if ty == 6 { 12 } else { 16 };
                let (np, nc) = if flag == 0 { (pts, 4) } else { (pts - 4, 2) };
                for _ in 0..np {
                    read(&mut pos, bpc * 2)?;
                }
                for _ in 0..nc {
                    color(&mut pos)?;
                }
                align(&mut pos);
            }
            _ => return None,
        }
        if pos >= data.len() * 8 {
            break;
        }
    }
    (count > 0).then(|| Color::rgb((sum[0] / count as f64).round() as u8, (sum[1] / count as f64).round() as u8, (sum[2] / count as f64).round() as u8))
}
