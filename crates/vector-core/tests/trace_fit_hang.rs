//! Issue #102: fitting tiny or thin traced loops must not hang (kurbo's inverse arc length
//! never converged when a candidate cubic had a huge arc length).
use kurbo::Point;
use vector_core::path::fit_freehand;

fn pts(v: &[(f64, f64)]) -> Vec<Point> {
    v.iter().map(|p| Point::new(p.0, p.1)).collect()
}

#[test]
fn tiny_three_point_loop_terminates() {
    let np = fit_freehand(&pts(&[(408.5, 64.0), (410.0, 64.5), (408.5, 65.0)]), 14.0, true);
    assert!(!np.subpaths.is_empty());
}

#[test]
fn thin_five_and_six_point_loops_terminate() {
    let a = fit_freehand(&pts(&[(587.5, 500.0), (594.0, 500.5), (591.5, 502.0), (566.0, 501.5), (586.5, 501.0)]), 14.0, true);
    let b = fit_freehand(&pts(&[(605.5, 499.0), (627.5, 499.0), (628.5, 500.0), (637.5, 500.0), (641.0, 501.5), (606.5, 501.0)]), 14.0, true);
    assert!(!a.subpaths.is_empty() && !b.subpaths.is_empty());
}
