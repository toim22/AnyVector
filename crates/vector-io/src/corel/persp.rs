//! Perspective and envelope effects. CorelDRAW keeps the object as drawn and stores the effect
//! next to its transform: a perspective maps the object's box onto a four-sided shape, an
//! envelope onto a shape with curved sides. The importer bakes both into the curves.

use kurbo::{Affine, BezPath, CubicBez, ParamCurve, PathEl, PathSeg, Point, Rect};
use vector_core::object::{ObjKind, Object};
use vector_core::path::NodePath;

/// A projective map, in the coordinate space of the object's parent (file units).
#[derive(Clone, Copy, Debug)]
pub(super) struct Persp {
    m: [f64; 8],
    /// The box the map starts from: left, bottom, width, height.
    from: [f64; 4],
}

impl Persp {
    /// The map that takes the box (left, top, right, bottom) to the corners
    /// lower-left, upper-left, upper-right, lower-right.
    pub fn new(l: f64, t: f64, r: f64, b: f64, quad: [Point; 4]) -> Option<Persp> {
        let (w, h) = (r - l, t - b);
        if w.abs() < 1e-9 || h.abs() < 1e-9 || quad.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return None;
        }
        // unit square (0,0) (1,0) (1,1) (0,1) to the quad
        let [q0, q3, q2, q1] = quad;
        let sx = q0.x - q1.x + q2.x - q3.x;
        let sy = q0.y - q1.y + q2.y - q3.y;
        let m = if sx.abs() < 1e-9 && sy.abs() < 1e-9 {
            [q1.x - q0.x, q2.x - q1.x, q0.x, q1.y - q0.y, q2.y - q1.y, q0.y, 0.0, 0.0]
        } else {
            let (dx1, dx2, dy1, dy2) = (q1.x - q2.x, q3.x - q2.x, q1.y - q2.y, q3.y - q2.y);
            let den = dx1 * dy2 - dx2 * dy1;
            if den.abs() < 1e-12 {
                return None;
            }
            let g = (sx * dy2 - dx2 * sy) / den;
            let hh = (dx1 * sy - sx * dy1) / den;
            [q1.x - q0.x + g * q1.x, q3.x - q0.x + hh * q3.x, q0.x, q1.y - q0.y + g * q1.y, q3.y - q0.y + hh * q3.y, q0.y, g, hh]
        };
        Some(Persp { m, from: [l, b, w, h] })
    }

    pub fn apply(&self, p: Point) -> Point {
        let [l, b, w, h] = self.from;
        let (u, v) = ((p.x - l) / w, (p.y - b) / h);
        let m = &self.m;
        let d = m[6] * u + m[7] * v + 1.0;
        let d = if d.abs() < 1e-9 { 1e-9_f64.copysign(d) } else { d };
        Point::new((m[0] * u + m[1] * v + m[2]) / d, (m[3] * u + m[4] * v + m[5]) / d)
    }
}

/// An envelope: the box of the object and the four sides it is mapped onto, each a run of
/// segments from one corner to the next (bottom and top left to right, left and right bottom
/// to top). Inside, points are placed by blending the sides (a Coons patch).
#[derive(Clone, Debug)]
pub(super) struct Envelope {
    from: Rect,
    sides: [Vec<PathSeg>; 4],
}

impl Envelope {
    /// The envelope of an object whose box is `from`, given the closed outline of the envelope
    /// (all in the same space).
    pub fn new(from: Rect, outline: &BezPath) -> Option<Envelope> {
        if from.width() < 1e-6 || from.height() < 1e-6 {
            return None;
        }
        let segs: Vec<PathSeg> = outline.segments().filter(|s| (s.start() - s.end()).hypot() > 1e-9 || matches!(s, PathSeg::Cubic(_))).collect();
        let n = segs.len();
        if n < 4 {
            return None;
        }
        // the nodes nearest to the box's corners: lower-left, lower-right, upper-right, upper-left
        let corners = [Point::new(from.x0, from.y0), Point::new(from.x1, from.y0), Point::new(from.x1, from.y1), Point::new(from.x0, from.y1)];
        let mut at = [0usize; 4];
        for (k, c) in corners.iter().enumerate() {
            at[k] = (0..n).min_by(|a, b| (segs[*a].start() - *c).hypot().total_cmp(&(segs[*b].start() - *c).hypot()))?;
        }
        if (0..4).any(|i| (0..i).any(|j| at[i] == at[j])) {
            return None;
        }
        // the run from corner a to corner b that passes no other corner (segment i joins node i
        // to node i + 1)
        let run = |a: usize, b: usize| -> Option<Vec<PathSeg>> {
            let fwd = (b + n - a) % n;
            if (1..fwd).all(|i| !at.contains(&((a + i) % n))) {
                return Some((0..fwd).map(|i| segs[(a + i) % n]).collect());
            }
            let back = (a + n - b) % n;
            if (1..back).all(|i| !at.contains(&((a + n - i) % n))) {
                return Some((0..back).map(|i| segs[(a + 2 * n - 1 - i) % n].reverse()).collect());
            }
            None
        };
        let [ll, lr, ur, ul] = at;
        Some(Envelope { from, sides: [run(ll, lr)?, run(ul, ur)?, run(ll, ul)?, run(lr, ur)?] })
    }

    fn side(&self, k: usize, t: f64) -> Point {
        let s = &self.sides[k];
        let x = t.clamp(0.0, 1.0) * s.len() as f64;
        let i = (x.floor() as usize).min(s.len() - 1);
        s[i].eval(x - i as f64)
    }

    pub fn apply(&self, p: Point) -> Point {
        let u = (p.x - self.from.x0) / self.from.width();
        let v = (p.y - self.from.y0) / self.from.height();
        let (b, t, l, r) = (self.side(0, u), self.side(1, u), self.side(2, v), self.side(3, v));
        let (p00, p10, p01, p11) = (self.side(0, 0.0), self.side(0, 1.0), self.side(1, 0.0), self.side(1, 1.0));
        let ruled = |a: Point, b: Point, t: f64| a.to_vec2() * (1.0 - t) + b.to_vec2() * t;
        let c = ruled(b, t, v) + ruled(l, r, u) - (ruled(p00, p10, u) * (1.0 - v) + ruled(p01, p11, u) * v);
        c.to_point()
    }
}

/// How a warp moves points.
#[derive(Clone, Debug)]
pub(super) enum Map {
    Persp(Persp),
    Envelope(Envelope),
}

/// One warp and the transform from its space to the page.
#[derive(Clone, Debug)]
pub(super) struct Warp {
    pub map: Map,
    pub space: Affine,
}

impl Warp {
    fn apply(&self, q: Point) -> Point {
        let p = self.space.inverse() * q;
        self.space
            * match &self.map {
                Map::Persp(m) => m.apply(p),
                Map::Envelope(m) => m.apply(p),
            }
    }
}

/// Apply `warps` (innermost first) to a page-space path.
fn warp_path(p: &BezPath, warps: &[Warp]) -> BezPath {
    let map = |q: Point| warps.iter().fold(q, |q, w| w.apply(q));
    let straight = warps.iter().all(|w| matches!(w.map, Map::Persp(_)));
    let mut out = BezPath::new();
    let mut last = Point::ZERO;
    let mut first = Point::ZERO;
    for el in p.elements() {
        match *el {
            PathEl::MoveTo(a) => {
                out.move_to(map(a));
                last = a;
                first = a;
            }
            // straight lines stay straight under a perspective, not under an envelope
            PathEl::LineTo(a) if straight => {
                out.line_to(map(a));
                last = a;
            }
            PathEl::LineTo(a) => {
                cubic(&mut out, line(last, a), &map);
                last = a;
            }
            PathEl::QuadTo(a, b) => {
                let c = kurbo::QuadBez::new(last, a, b).raise();
                cubic(&mut out, c, &map);
                last = b;
            }
            PathEl::CurveTo(a, b, c) => {
                cubic(&mut out, CubicBez::new(last, a, b, c), &map);
                last = c;
            }
            PathEl::ClosePath => {
                // the closing side bends too
                if !straight && (last - first).hypot() > 1e-9 {
                    cubic(&mut out, line(last, first), &map);
                }
                last = first;
                out.close_path();
            }
        }
    }
    out
}

fn line(a: Point, b: Point) -> CubicBez {
    CubicBez::new(a, a.lerp(b, 1.0 / 3.0), a.lerp(b, 2.0 / 3.0), b)
}

/// A curve is not mapped exactly; mapping the control points of short pieces is close.
fn cubic(out: &mut BezPath, c: CubicBez, map: &dyn Fn(Point) -> Point) {
    const N: usize = 4;
    for i in 0..N {
        let s = c.subsegment(i as f64 / N as f64..(i + 1) as f64 / N as f64);
        out.curve_to(map(s.p1), map(s.p2), map(s.p3));
    }
}

/// Bake `warps` into an object and its contents. Bitmaps keep their box.
pub(super) fn bake(o: &mut Object, warps: &[Warp]) {
    if warps.is_empty() {
        return;
    }
    match &mut o.kind {
        ObjKind::Group(kids) => kids.iter_mut().for_each(|k| bake(k, warps)),
        ObjKind::PowerClip { frame, contents } => {
            bake(frame, warps);
            contents.iter_mut().for_each(|k| bake(k, warps));
        }
        ObjKind::Image(_) => {}
        _ => {
            let p = o.world_path();
            if p.elements().is_empty() {
                return;
            }
            o.kind = ObjKind::Path(NodePath::from_bez(&warp_path(&p, warps)));
            o.transform = Affine::IDENTITY;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_land_on_the_quad() {
        let quad = [Point::new(0.0, 2.0), Point::new(0.0, 8.0), Point::new(5.0, 9.0), Point::new(5.0, 1.0)];
        let p = Persp::new(0.0, 10.0, 20.0, 0.0, quad).unwrap();
        let near = |a: Point, b: Point| (a - b).hypot() < 1e-9;
        assert!(near(p.apply(Point::new(0.0, 0.0)), quad[0]));
        assert!(near(p.apply(Point::new(0.0, 10.0)), quad[1]));
        assert!(near(p.apply(Point::new(20.0, 10.0)), quad[2]));
        assert!(near(p.apply(Point::new(20.0, 0.0)), quad[3]));
    }
}
