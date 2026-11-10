//! Perspective effects. CorelDRAW keeps the object as drawn and stores the effect as a
//! transform record that maps the object's box onto a four-sided shape. AnyVector has no
//! live perspective, so the importer bakes it into the curves.

use kurbo::{Affine, BezPath, CubicBez, ParamCurve, PathEl, Point};
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

/// One perspective and the transform from its space to the page.
#[derive(Clone, Copy, Debug)]
pub(super) struct Warp {
    pub persp: Persp,
    pub space: Affine,
}

/// Apply `warps` (innermost first) to a page-space path.
fn warp_path(p: &BezPath, warps: &[Warp]) -> BezPath {
    let map = |q: Point| warps.iter().fold(q, |q, w| w.space * w.persp.apply(w.space.inverse() * q));
    let mut out = BezPath::new();
    let mut last = Point::ZERO;
    for el in p.elements() {
        match *el {
            PathEl::MoveTo(a) => {
                out.move_to(map(a));
                last = a;
            }
            // straight lines stay straight under a perspective
            PathEl::LineTo(a) => {
                out.line_to(map(a));
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
            PathEl::ClosePath => out.close_path(),
        }
    }
    out
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
