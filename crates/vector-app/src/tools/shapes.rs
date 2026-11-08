//! The Common Shapes library: basic shapes, arrows, flowchart symbols, banners and callouts.
//! Every shape is defined in a unit box (0..1) and scaled to the dragged rectangle.

use kurbo::{BezPath, Point};
use vector_core::path::NodePath;

pub const COMMON_SHAPES: &[&str] = &[
    "Parallelogram",
    "Trapezoid",
    "Triangle",
    "Right Triangle",
    "Diamond",
    "Pentagon",
    "Hexagon",
    "Octagon",
    "Cross",
    "Heart",
    "Lightning",
    "Moon",
    "Ring",
    "Right Arrow",
    "Left Arrow",
    "Up Arrow",
    "Down Arrow",
    "Left-Right Arrow",
    "Up-Down Arrow",
    "Chevron",
    "Process",
    "Decision",
    "Terminator",
    "Document",
    "Data",
    "Banner",
    "Rectangle Callout",
    "Round Callout",
];

fn poly(pts: &[(f64, f64)]) -> BezPath {
    let mut bp = BezPath::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        if i == 0 {
            bp.move_to((*x, *y));
        } else {
            bp.line_to((*x, *y));
        }
    }
    bp.close_path();
    bp
}

fn regular(n: usize, rot: f64) -> BezPath {
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let a = rot + i as f64 * std::f64::consts::TAU / n as f64;
            (0.5 + 0.5 * a.cos(), 0.5 + 0.5 * a.sin())
        })
        .collect();
    poly(&pts)
}

fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64, reverse: bool) -> BezPath {
    let k = 0.5522847498;
    let mut bp = BezPath::new();
    let (s, e): (f64, f64) = if reverse { (-1.0, 1.0) } else { (1.0, 1.0) };
    bp.move_to((cx + rx, cy));
    bp.curve_to((cx + rx, cy + s * ry * k), (cx + rx * k, cy + s * ry), (cx, cy + s * ry));
    bp.curve_to((cx - rx * k, cy + s * ry), (cx - rx, cy + s * ry * k), (cx - rx, cy));
    bp.curve_to((cx - rx, cy - s * ry * k * e), (cx - rx * k, cy - s * ry), (cx, cy - s * ry));
    bp.curve_to((cx + rx * k, cy - s * ry), (cx + rx, cy - s * ry * k), (cx + rx, cy));
    bp.close_path();
    bp
}

/// Unit-box geometry of a common shape.
pub fn unit_shape(index: usize) -> BezPath {
    match COMMON_SHAPES.get(index).copied().unwrap_or("Parallelogram") {
        "Parallelogram" => poly(&[(0.25, 0.0), (1.0, 0.0), (0.75, 1.0), (0.0, 1.0)]),
        "Trapezoid" => poly(&[(0.2, 0.0), (0.8, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        "Triangle" => poly(&[(0.5, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        "Right Triangle" => poly(&[(0.0, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        "Diamond" => poly(&[(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)]),
        "Pentagon" => regular(5, -std::f64::consts::FRAC_PI_2),
        "Hexagon" => poly(&[(0.25, 0.0), (0.75, 0.0), (1.0, 0.5), (0.75, 1.0), (0.25, 1.0), (0.0, 0.5)]),
        "Octagon" => poly(&[(0.3, 0.0), (0.7, 0.0), (1.0, 0.3), (1.0, 0.7), (0.7, 1.0), (0.3, 1.0), (0.0, 0.7), (0.0, 0.3)]),
        "Cross" => poly(&[(0.35, 0.0), (0.65, 0.0), (0.65, 0.35), (1.0, 0.35), (1.0, 0.65), (0.65, 0.65), (0.65, 1.0), (0.35, 1.0), (0.35, 0.65), (0.0, 0.65), (0.0, 0.35), (0.35, 0.35)]),
        "Heart" => {
            let mut bp = BezPath::new();
            bp.move_to((0.5, 0.25));
            bp.curve_to((0.5, 0.05), (0.25, -0.02), (0.1, 0.08));
            bp.curve_to((-0.05, 0.2), (0.0, 0.5), (0.2, 0.68));
            bp.line_to((0.5, 1.0));
            bp.line_to((0.8, 0.68));
            bp.curve_to((1.0, 0.5), (1.05, 0.2), (0.9, 0.08));
            bp.curve_to((0.75, -0.02), (0.5, 0.05), (0.5, 0.25));
            bp.close_path();
            bp
        }
        "Lightning" => poly(&[(0.4, 0.0), (0.75, 0.0), (0.55, 0.38), (0.85, 0.38), (0.25, 1.0), (0.42, 0.55), (0.15, 0.55)]),
        "Moon" => {
            let mut bp = BezPath::new();
            bp.move_to((0.65, 0.0));
            bp.curve_to((0.25, 0.0), (0.0, 0.25), (0.0, 0.5));
            bp.curve_to((0.0, 0.78), (0.25, 1.0), (0.65, 1.0));
            bp.curve_to((0.4, 0.85), (0.3, 0.68), (0.3, 0.5));
            bp.curve_to((0.3, 0.32), (0.4, 0.15), (0.65, 0.0));
            bp.close_path();
            bp
        }
        "Ring" => {
            let mut bp = ellipse(0.5, 0.5, 0.5, 0.5, false);
            for el in ellipse(0.5, 0.5, 0.3, 0.3, true).elements() {
                bp.push(*el);
            }
            bp
        }
        "Right Arrow" => poly(&[(0.0, 0.25), (0.6, 0.25), (0.6, 0.0), (1.0, 0.5), (0.6, 1.0), (0.6, 0.75), (0.0, 0.75)]),
        "Left Arrow" => poly(&[(1.0, 0.25), (0.4, 0.25), (0.4, 0.0), (0.0, 0.5), (0.4, 1.0), (0.4, 0.75), (1.0, 0.75)]),
        "Up Arrow" => poly(&[(0.25, 1.0), (0.25, 0.4), (0.0, 0.4), (0.5, 0.0), (1.0, 0.4), (0.75, 0.4), (0.75, 1.0)]),
        "Down Arrow" => poly(&[(0.25, 0.0), (0.25, 0.6), (0.0, 0.6), (0.5, 1.0), (1.0, 0.6), (0.75, 0.6), (0.75, 0.0)]),
        "Left-Right Arrow" => poly(&[(0.0, 0.5), (0.25, 0.0), (0.25, 0.25), (0.75, 0.25), (0.75, 0.0), (1.0, 0.5), (0.75, 1.0), (0.75, 0.75), (0.25, 0.75), (0.25, 1.0)]),
        "Up-Down Arrow" => poly(&[(0.5, 0.0), (1.0, 0.25), (0.75, 0.25), (0.75, 0.75), (1.0, 0.75), (0.5, 1.0), (0.0, 0.75), (0.25, 0.75), (0.25, 0.25), (0.0, 0.25)]),
        "Chevron" => poly(&[(0.0, 0.0), (0.7, 0.0), (1.0, 0.5), (0.7, 1.0), (0.0, 1.0), (0.3, 0.5)]),
        "Process" => poly(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        "Decision" => poly(&[(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)]),
        "Terminator" => {
            let k = 0.5522847498 * 0.5;
            let mut bp = BezPath::new();
            bp.move_to((0.25, 0.0));
            bp.line_to((0.75, 0.0));
            bp.curve_to((0.75 + k * 0.5, 0.0), (1.0, 0.5 - k), (1.0, 0.5));
            bp.curve_to((1.0, 0.5 + k), (0.75 + k * 0.5, 1.0), (0.75, 1.0));
            bp.line_to((0.25, 1.0));
            bp.curve_to((0.25 - k * 0.5, 1.0), (0.0, 0.5 + k), (0.0, 0.5));
            bp.curve_to((0.0, 0.5 - k), (0.25 - k * 0.5, 0.0), (0.25, 0.0));
            bp.close_path();
            bp
        }
        "Document" => {
            let mut bp = BezPath::new();
            bp.move_to((0.0, 0.0));
            bp.line_to((1.0, 0.0));
            bp.line_to((1.0, 0.85));
            bp.curve_to((0.75, 0.75), (0.5, 0.85), (0.5, 0.9));
            bp.curve_to((0.5, 0.95), (0.25, 1.05), (0.0, 0.92));
            bp.close_path();
            bp
        }
        "Data" => poly(&[(0.2, 0.0), (1.0, 0.0), (0.8, 1.0), (0.0, 1.0)]),
        "Banner" => poly(&[(0.0, 0.2), (0.15, 0.2), (0.15, 0.0), (0.85, 0.0), (0.85, 0.2), (1.0, 0.2), (0.9, 0.5), (1.0, 0.8), (0.85, 0.8), (0.85, 1.0), (0.15, 1.0), (0.15, 0.8), (0.0, 0.8), (0.1, 0.5)]),
        "Rectangle Callout" => poly(&[(0.0, 0.0), (1.0, 0.0), (1.0, 0.75), (0.45, 0.75), (0.2, 1.0), (0.25, 0.75), (0.0, 0.75)]),
        "Round Callout" => {
            let k = 0.5522847498;
            let (cx, cy, rx, ry) = (0.5, 0.4, 0.5, 0.4);
            let mut bp = BezPath::new();
            bp.move_to((cx + rx, cy));
            bp.curve_to((cx + rx, cy + ry * k), (cx + rx * k, cy + ry), (cx, cy + ry));
            bp.line_to((0.15, 1.0));
            bp.line_to((0.28, cy + ry * 0.92));
            bp.curve_to((0.1, cy + ry * 0.8), (cx - rx, cy + ry * k), (cx - rx, cy));
            bp.curve_to((cx - rx, cy - ry * k), (cx - rx * k, cy - ry), (cx, cy - ry));
            bp.curve_to((cx + rx * k, cy - ry), (cx + rx, cy - ry * k), (cx + rx, cy));
            bp.close_path();
            bp
        }
        _ => poly(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]),
    }
}

/// A common shape scaled to w × h (local box 0,0 – w,h) as an editable curve.
pub fn common_shape(index: usize, w: f64, h: f64) -> NodePath {
    let mut bp = unit_shape(index);
    bp.apply_affine(kurbo::Affine::scale_non_uniform(w, h));
    NodePath::from_bez(&bp)
}

/// Small preview polyline points for the property bar picker (unit box).
#[allow(dead_code)]
pub fn preview_points(index: usize) -> Vec<Vec<Point>> {
    vector_core::geom::flatten_polylines(&unit_shape(index), 0.01).into_iter().map(|(p, _)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    #[test]
    fn all_shapes_fill_unit_box() {
        for i in 0..COMMON_SHAPES.len() {
            let b = unit_shape(i).bounding_box();
            assert!(b.width() > 0.5 && b.height() > 0.5, "{} {:?}", COMMON_SHAPES[i], b);
            assert!(b.x0 > -0.1 && b.y0 > -0.1 && b.x1 < 1.1 && b.y1 < 1.1, "{}", COMMON_SHAPES[i]);
        }
    }
}
