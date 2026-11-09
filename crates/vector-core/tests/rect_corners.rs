use kurbo::{Point, Shape};
use vector_core::object::{rect_path, CornerType, RectShape};

fn shape(corner: CornerType) -> kurbo::BezPath {
    rect_path(&RectShape { w: 100.0, h: 100.0, radii: [50.0; 4], corner })
}

#[test]
fn scalloped_corners_cut_into_the_rectangle() {
    // a square scalloped at half its side is a four-pointed star: its middle is filled,
    // the points sit at the middle of each side, and the corners are cut away
    let star = shape(CornerType::Scallop);
    assert!(star.contains(Point::new(50.0, 50.0)));
    assert!(!star.contains(Point::new(20.0, 20.0)));
    assert!(star.area().abs() < 100.0 * 100.0 - std::f64::consts::PI * 50.0 * 50.0 + 50.0);
    // rounded at the same radius it is a circle, which does hold that point
    assert!(shape(CornerType::Round).contains(Point::new(20.0, 20.0)));
}
