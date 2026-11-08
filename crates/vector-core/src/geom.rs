//! Geometry helpers: flattening, boolean operations, hit testing, offsets.

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape, Vec2};

pub type Contour = Vec<[f64; 2]>;
pub type Contours = Vec<Contour>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Intersect,
    /// subject minus clip
    Difference,
    Xor,
}

/// Flatten a path into closed polygons (each subpath becomes a contour).
pub fn flatten(path: &BezPath, tolerance: f64) -> Contours {
    let mut out: Contours = vec![];
    let mut cur: Contour = vec![];
    kurbo::flatten(path.elements().iter().copied(), tolerance.max(0.001), |el| match el {
        PathEl::MoveTo(p) => {
            if cur.len() >= 3 {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
            cur.push([p.x, p.y]);
        }
        PathEl::LineTo(p) => {
            if cur.last().map_or(true, |l| (l[0] - p.x).abs() > 1e-9 || (l[1] - p.y).abs() > 1e-9) {
                cur.push([p.x, p.y]);
            }
        }
        PathEl::ClosePath => {
            if cur.len() >= 3 {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
        _ => {}
    });
    if cur.len() >= 3 {
        out.push(cur);
    }
    out
}

/// Flatten keeping open polylines (for stroke-type operations).
pub fn flatten_polylines(path: &BezPath, tolerance: f64) -> Vec<(Vec<Point>, bool)> {
    let mut out = vec![];
    let mut cur: Vec<Point> = vec![];
    kurbo::flatten(path.elements().iter().copied(), tolerance.max(0.001), |el| match el {
        PathEl::MoveTo(p) => {
            if cur.len() >= 2 {
                out.push((std::mem::take(&mut cur), false));
            } else {
                cur.clear();
            }
            cur.push(p);
        }
        PathEl::LineTo(p) => cur.push(p),
        PathEl::ClosePath => {
            if cur.len() >= 2 {
                out.push((std::mem::take(&mut cur), true));
            } else {
                cur.clear();
            }
        }
        _ => {}
    });
    if cur.len() >= 2 {
        out.push((cur, false));
    }
    out
}

pub fn contours_to_bez(c: &[Contour]) -> BezPath {
    let mut bp = BezPath::new();
    for ct in c {
        if ct.len() < 2 {
            continue;
        }
        bp.move_to((ct[0][0], ct[0][1]));
        for p in &ct[1..] {
            bp.line_to((p[0], p[1]));
        }
        bp.close_path();
    }
    bp
}

/// Boolean operation between two (closed) paths. Result simplified back to curves.
pub fn boolean(a: &BezPath, b: &BezPath, op: BoolOp, even_odd: bool) -> BezPath {
    let tol = flatten_tolerance(a, b);
    let ca = flatten(a, tol);
    let cb = flatten(b, tol);
    let rule = match op {
        BoolOp::Union => OverlayRule::Union,
        BoolOp::Intersect => OverlayRule::Intersect,
        BoolOp::Difference => OverlayRule::Difference,
        BoolOp::Xor => OverlayRule::Xor,
    };
    let fill = if even_odd { FillRule::EvenOdd } else { FillRule::NonZero };
    let shapes = ca.overlay(&cb, rule, fill);
    let mut contours: Contours = vec![];
    for shape in shapes {
        for c in shape {
            contours.push(c);
        }
    }
    smooth_contours(&contours, tol)
}

/// Union of many paths.
pub fn union_all(paths: &[BezPath]) -> BezPath {
    if paths.is_empty() {
        return BezPath::new();
    }
    let mut acc = paths[0].clone();
    for p in &paths[1..] {
        acc = boolean(&acc, p, BoolOp::Union, false);
    }
    acc
}

/// Remove self-overlaps of one path (non-zero union with itself).
pub fn simplify_self(a: &BezPath) -> BezPath {
    let tol = flatten_tolerance(a, a);
    let ca = flatten(a, tol);
    let empty: Contours = vec![];
    let shapes = ca.overlay(&empty, OverlayRule::Subject, FillRule::NonZero);
    let mut contours: Contours = vec![];
    for shape in shapes {
        contours.extend(shape);
    }
    smooth_contours(&contours, tol)
}

fn flatten_tolerance(a: &BezPath, b: &BezPath) -> f64 {
    let r = a.bounding_box().union(b.bounding_box());
    let size = r.width().max(r.height()).max(1.0);
    (size / 4000.0).clamp(0.005, 0.25)
}

/// Turn polygon contours back into paths; dense polyline runs are refit as smooth curves.
pub fn smooth_contours(contours: &[Contour], tol: f64) -> BezPath {
    let bp = contours_to_bez(contours);
    let simplified = kurbo::simplify::simplify_bezpath(bp.elements().iter().copied(), tol * 2.0, &Default::default());
    // simplify_bezpath can drop ClosePath semantics for tiny contours; fall back to polygon if empty
    if simplified.elements().is_empty() {
        bp
    } else {
        simplified
    }
}

/// Is point inside the (filled) path?
pub fn contains(path: &BezPath, p: Point, even_odd: bool) -> bool {
    let w = path.winding(p);
    if even_odd {
        w % 2 != 0
    } else {
        w != 0
    }
}

/// Minimum distance from a point to the outline of a path.
pub fn distance_to_outline(path: &BezPath, p: Point) -> f64 {
    use kurbo::ParamCurveNearest;
    let mut best = f64::INFINITY;
    for seg in path.segments() {
        let n = seg.nearest(p, 1e-4);
        best = best.min(n.distance_sq.sqrt());
    }
    best
}

/// Expand an outline (stroke) into a filled path.
pub fn stroke_to_path(path: &BezPath, width: f64, join: kurbo::Join, cap: kurbo::Cap, dashes: &[f64]) -> BezPath {
    let mut style = kurbo::Stroke::new(width.max(0.01)).with_join(join).with_caps(cap);
    if !dashes.is_empty() {
        style = style.with_dashes(0.0, dashes.iter().map(|d| d * width.max(0.01)));
    }
    kurbo::stroke(path.elements().iter().copied(), &style, &Default::default(), 0.05)
}

/// Offset a closed path by `d` (positive = outward). Implemented as union/difference with the stroke.
pub fn offset_path(path: &BezPath, d: f64, join: kurbo::Join) -> BezPath {
    if d.abs() < 1e-9 {
        return path.clone();
    }
    let stroke = stroke_to_path(path, d.abs() * 2.0, join, kurbo::Cap::Butt, &[]);
    if d > 0.0 {
        boolean(path, &stroke, BoolOp::Union, false)
    } else {
        boolean(path, &stroke, BoolOp::Difference, false)
    }
}

/// Transform-aware bounding box.
pub fn transformed_bbox(path: &BezPath, a: Affine) -> Rect {
    (a * path.clone()).bounding_box()
}

pub fn rect_path(r: Rect) -> BezPath {
    r.to_path(0.1)
}

/// Point to the nearest point on a polyline; returns distance.
pub fn dist_point_segment(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let l2 = ab.hypot2();
    if l2 < 1e-12 {
        return (p - a).hypot();
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    (p - (a + ab * t)).hypot()
}

/// Intersection of two segments.
pub fn seg_intersect(a0: Point, a1: Point, b0: Point, b1: Point) -> Option<(f64, f64, Point)> {
    let r = a1 - a0;
    let s = b1 - b0;
    let den = r.cross(s);
    if den.abs() < 1e-12 {
        return None;
    }
    let q = b0 - a0;
    let t = q.cross(s) / den;
    let u = q.cross(r) / den;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some((t, u, a0 + r * t))
    } else {
        None
    }
}

/// Decompose an affine into (translation, rotation radians, shear x, scale x, scale y): M = T·R·Sh·S.
pub fn decompose(a: Affine) -> (Vec2, f64, f64, f64, f64) {
    let [m11, m21, m12, m22, tx, ty] = a.as_coeffs();
    // columns: (m11, m21), (m12, m22)
    let sx = (m11 * m11 + m21 * m21).sqrt();
    let rot = m21.atan2(m11);
    let det = m11 * m22 - m12 * m21;
    let sy = if sx.abs() > 1e-12 { det / sx } else { 0.0 };
    let shear = if sx.abs() > 1e-12 && sy.abs() > 1e-12 { (m11 * m12 + m21 * m22) / (sx * sy) } else { 0.0 };
    (Vec2::new(tx, ty), rot, shear, sx, sy)
}

pub fn compose(t: Vec2, rot: f64, shear: f64, sx: f64, sy: f64) -> Affine {
    Affine::translate(t) * Affine::rotate(rot) * Affine::new([1.0, 0.0, shear, 1.0, 0.0, 0.0]) * Affine::scale_non_uniform(sx, sy)
}

/// Split an affine into a "rest" part and a pure scale to be absorbed into the geometry.
pub fn split_scale(a: Affine) -> (Affine, f64, f64) {
    let (t, rot, shear, sx, sy) = decompose(a);
    let rest = compose(t, rot, shear, sx.signum(), sy.signum());
    (rest, sx.abs(), sy.abs())
}

/// Rotation angle (degrees) of an affine.
pub fn rotation_deg(a: Affine) -> f64 {
    let (_, r, _, _, _) = decompose(a);
    r.to_degrees()
}

pub fn path_area(path: &BezPath) -> f64 {
    path.area().abs()
}
