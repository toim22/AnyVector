//! What the program paints, made into AnyVector objects: filled and stroked paths (a fill and
//! a stroke of the same path become one object), text, images, and PowerClips for the clipping
//! paths the objects were painted through.

use kurbo::{Affine, BezPath, PathEl, Rect, Shape};
use std::rc::Rc;
use vector_core::color::{Color, NamedColor};
use vector_core::document::Document;
use vector_core::object::{ObjKind, Object};
use vector_core::path::NodePath;
use vector_core::style::{Fill, Outline, Style};

/// A clipping path (in device space) and the one it was intersected with.
pub struct ClipNode {
    pub path: BezPath,
    pub even_odd: bool,
    pub parent: Option<Rc<ClipNode>>,
}

pub enum Item {
    Path { path: BezPath, style: Style, glyphs: bool },
    Obj(Object),
    Clip { path: BezPath, even_odd: bool, items: Vec<Item> },
}

struct Frame {
    clip: Option<Rc<ClipNode>>,
    items: Vec<Item>,
}

pub struct Builder {
    /// Holds the images and gives the ids.
    pub doc: Document,
    /// Device space (points, y up) to the page (points, y down, from its top-left corner).
    pub to_doc: Affine,
    pub pages: Vec<Vec<Object>>,
    stack: Vec<Frame>,
    /// While a Type 3 font's glyphs are drawn: fills of one style become one curve.
    pub glyphs: usize,
    /// When set, paint is not kept; the colors used are (for a tiling pattern's color).
    pub capture: Option<Vec<Color>>,
    pub spots: Vec<NamedColor>,
    /// Objects painted so far.
    pub painted: usize,
}

impl Builder {
    pub fn new(to_doc: Affine) -> Builder {
        Builder { doc: Document::new(612.0, 792.0), to_doc, pages: vec![], stack: vec![Frame { clip: None, items: vec![] }], glyphs: 0, capture: None, spots: vec![], painted: 0 }
    }

    pub fn add_spot(&mut self, name: &str, c: Color) {
        if !self.spots.iter().any(|s| s.name == name) {
            self.spots.push(NamedColor { name: name.to_string(), color: c, spot: true });
        }
    }

    pub fn has_content(&self) -> bool {
        self.stack.iter().any(|f| !f.items.is_empty())
    }

    /// Make the open frames those of `clip`.
    fn place(&mut self, clip: &Option<Rc<ClipNode>>) {
        let mut chain = vec![];
        let mut n = clip.clone();
        while let Some(c) = n {
            n = c.parent.clone();
            chain.push(c);
        }
        chain.reverse();
        let mut p = 0;
        while p < chain.len() && p + 1 < self.stack.len() {
            match &self.stack[p + 1].clip {
                Some(c) if Rc::ptr_eq(c, &chain[p]) => p += 1,
                _ => break,
            }
        }
        while self.stack.len() > p + 1 {
            self.close_frame();
        }
        for node in &chain[p..] {
            let path = self.to_doc * node.path.clone();
            // the same clipping path again right after: back into that PowerClip
            let parent = self.stack.last_mut().unwrap();
            let reopen = matches!(parent.items.last(), Some(Item::Clip { path: q, even_odd, .. }) if *even_odd == node.even_odd && same_path(q, &path));
            let items = if reopen {
                match parent.items.pop() {
                    Some(Item::Clip { items, .. }) => items,
                    _ => vec![],
                }
            } else {
                vec![]
            };
            self.stack.push(Frame { clip: Some(node.clone()), items });
        }
    }

    fn close_frame(&mut self) {
        let f = self.stack.pop().unwrap();
        let Some(node) = f.clip else { return };
        if f.items.is_empty() {
            return;
        }
        let path = self.to_doc * node.path.clone();
        self.stack.last_mut().unwrap().items.push(Item::Clip { path, even_odd: node.even_odd, items: f.items });
    }

    /// Paint a path (device space).
    pub fn add_path(&mut self, path: &BezPath, style: Style, clip: &Option<Rc<ClipNode>>) {
        if let Some(c) = &mut self.capture {
            if let Some(col) = style.fill.main_color().or(style.outline.color) {
                c.push(col);
            }
            return;
        }
        if path.elements().len() < 2 {
            return;
        }
        self.painted += 1;
        self.place(clip);
        let path = self.to_doc * path.clone();
        let glyphs = self.glyphs > 0;
        let top = self.stack.last_mut().unwrap();
        if let Some(Item::Path { path: q, style: s, glyphs: g }) = top.items.last_mut() {
            let stroke_only = style.fill.is_none() && style.outline.color.is_some();
            let fill_only = !style.fill.is_none() && style.outline.color.is_none();
            if stroke_only && s.outline.color.is_none() && !s.fill.is_none() && same_path(q, &path) {
                s.outline = style.outline;
                return;
            }
            if fill_only && s.fill.is_none() && s.outline.color.is_some() && same_path(q, &path) {
                s.fill = style.fill;
                s.even_odd = style.even_odd;
                s.outline.behind_fill = true;
                return;
            }
            if glyphs && *g && *s == style {
                q.extend(path.elements().iter().copied());
                return;
            }
        }
        top.items.push(Item::Path { path, style, glyphs });
    }

    /// Add an object already in page coordinates.
    pub fn add_object(&mut self, o: Object, clip: &Option<Rc<ClipNode>>) {
        if let Some(c) = &mut self.capture {
            if let Some(col) = o.style.fill.main_color() {
                c.push(col);
            }
            return;
        }
        self.painted += 1;
        self.place(clip);
        self.stack.last_mut().unwrap().items.push(Item::Obj(o));
    }

    /// End a page: what was painted becomes the page's objects.
    pub fn finish_page(&mut self) {
        while self.stack.len() > 1 {
            self.close_frame();
        }
        let items = std::mem::take(&mut self.stack[0].items);
        let objs = self.objects(items);
        self.pages.push(objs);
    }

    fn objects(&mut self, items: Vec<Item>) -> Vec<Object> {
        let mut out = vec![];
        for it in items {
            match it {
                Item::Path { path, style, .. } => {
                    let id = self.doc.new_id();
                    out.push(Object::new(id, ObjKind::Path(NodePath::from_bez(&path)), Affine::IDENTITY, style));
                }
                Item::Obj(mut o) => {
                    self.doc.reassign_ids(&mut o);
                    out.push(o);
                }
                Item::Clip { path, even_odd, items } => {
                    let reach = items_bounds(&items);
                    let contents = self.objects(items);
                    if contents.is_empty() {
                        continue;
                    }
                    // a rectangle around everything inside clips nothing
                    if let (Some(r), Some(b)) = (rect_of(&path), reach) {
                        if r.inflate(0.01, 0.01).contains(Point2::new(b.x0, b.y0)) && r.inflate(0.01, 0.01).contains(Point2::new(b.x1, b.y1)) {
                            out.extend(contents);
                            continue;
                        }
                    }
                    let mut st = Style::default();
                    st.fill = Fill::None;
                    st.outline = Outline::none();
                    st.even_odd = even_odd;
                    let id = self.doc.new_id();
                    let frame = Object::new(id, ObjKind::Path(NodePath::from_bez(&path)), Affine::IDENTITY, st.clone());
                    let id = self.doc.new_id();
                    out.push(Object::new(id, ObjKind::PowerClip { frame: Box::new(frame), contents }, Affine::IDENTITY, st));
                }
            }
        }
        out
    }
}

type Point2 = kurbo::Point;

/// How far painted items reach (paths with half their outline).
fn items_bounds(items: &[Item]) -> Option<Rect> {
    let mut r: Option<Rect> = None;
    let mut add = |b: Rect| r = Some(r.map_or(b, |x| x.union(b)));
    for it in items {
        match it {
            Item::Path { path, style, .. } => {
                let w = if style.outline.color.is_some() { style.outline.width / 2.0 * style.outline.miter_limit.max(1.0) } else { 0.0 };
                add(path.bounding_box().inflate(w, w));
            }
            Item::Obj(o) => add(o.bounds()),
            Item::Clip { path, .. } => add(path.bounding_box()),
        }
    }
    r
}

/// The rectangle a path is, when it is one with straight sides.
pub fn rect_of(p: &BezPath) -> Option<Rect> {
    let mut pts = vec![];
    for (i, el) in p.elements().iter().enumerate() {
        match el {
            PathEl::MoveTo(a) if i == 0 => pts.push(*a),
            PathEl::LineTo(a) => pts.push(*a),
            PathEl::ClosePath if i == p.elements().len() - 1 => {}
            _ => return None,
        }
    }
    if pts.len() == 5 && (pts[4] - pts[0]).hypot() < 1e-6 {
        pts.pop();
    }
    if pts.len() != 4 {
        return None;
    }
    let e = 1e-6;
    let horiz = |a: kurbo::Point, b: kurbo::Point| (a.y - b.y).abs() < e;
    let vert = |a: kurbo::Point, b: kurbo::Point| (a.x - b.x).abs() < e;
    let ok = (horiz(pts[0], pts[1]) && vert(pts[1], pts[2]) && horiz(pts[2], pts[3]) && vert(pts[3], pts[0]))
        || (vert(pts[0], pts[1]) && horiz(pts[1], pts[2]) && vert(pts[2], pts[3]) && horiz(pts[3], pts[0]));
    ok.then(|| Rect::from_points(pts[0], pts[2]))
}

/// Whether two paths are the same, give or take rounding.
pub fn same_path(a: &BezPath, b: &BezPath) -> bool {
    let (ea, eb) = (a.elements(), b.elements());
    if ea.len() != eb.len() {
        return false;
    }
    let close = |p: kurbo::Point, q: kurbo::Point| (p - q).hypot() < 1e-4;
    ea.iter().zip(eb).all(|(x, y)| match (x, y) {
        (PathEl::MoveTo(p), PathEl::MoveTo(q)) | (PathEl::LineTo(p), PathEl::LineTo(q)) => close(*p, *q),
        (PathEl::QuadTo(p1, p2), PathEl::QuadTo(q1, q2)) => close(*p1, *q1) && close(*p2, *q2),
        (PathEl::CurveTo(p1, p2, p3), PathEl::CurveTo(q1, q2, q3)) => close(*p1, *q1) && close(*p2, *q2) && close(*p3, *q3),
        (PathEl::ClosePath, PathEl::ClosePath) => true,
        _ => false,
    })
}
