//! The piece Virtual Segment Delete shows under the pointer before the click (#119): the same
//! piece the click removes, on the page.

use kurbo::{Affine, BezPath, ParamCurve, Point};
use vector_core::object::{CornerType, RectShape};
use vector_core::ops;
use vector_core::path::NodePath;
use vector_core::{Document, ObjKind, Object, Style};

fn push(doc: &mut Document, kind: ObjKind, at: Affine) -> u64 {
    let id = doc.new_id();
    doc.pages[0].layers.last_mut().unwrap().objects.push(Object::new(id, kind, at, Style::default()));
    id
}

fn line(a: (f64, f64), b: (f64, f64)) -> ObjKind {
    let mut bp = BezPath::new();
    bp.move_to(a);
    bp.line_to(b);
    ObjKind::Path(NodePath::from_bez(&bp))
}

fn near(a: Point, b: Point) -> bool {
    (a - b).hypot() < 1.0
}

#[test]
fn the_piece_of_a_rectangle_side_is_on_the_page() {
    let mut doc = Document::new(400.0, 400.0);
    let r = RectShape { w: 100.0, h: 80.0, radii: [0.0; 4], corner: CornerType::default() };
    push(&mut doc, ObjKind::Rect(r), Affine::translate((20.0, 20.0)));
    let cut = ops::virtual_cut_at(&doc, 0, Point::new(70.0, 21.0), 3.0).expect("the top side");
    let c = ops::virtual_cut_piece(&doc, &cut).expect("its piece");
    let (a, b) = (c.eval(0.0), c.eval(1.0));
    assert!((near(a, Point::new(20.0, 20.0)) && near(b, Point::new(120.0, 20.0))) || (near(b, Point::new(20.0, 20.0)) && near(a, Point::new(120.0, 20.0))), "{a:?} {b:?}");
}

#[test]
fn the_piece_stops_at_crossings_and_follows_the_transform() {
    let mut doc = Document::new(400.0, 400.0);
    // a line drawn at the origin and moved down by 50: from (0, 50) to (200, 50)
    push(&mut doc, line((0.0, 0.0), (200.0, 0.0)), Affine::translate((0.0, 50.0)));
    push(&mut doc, line((100.0, 0.0), (100.0, 100.0)), Affine::IDENTITY);
    push(&mut doc, line((150.0, 0.0), (150.0, 100.0)), Affine::IDENTITY);
    let cut = ops::virtual_cut_at(&doc, 0, Point::new(120.0, 50.0), 3.0).expect("the line");
    let c = ops::virtual_cut_piece(&doc, &cut).expect("its piece");
    let (a, b) = (c.eval(0.0), c.eval(1.0));
    assert!(near(a, Point::new(100.0, 50.0)) && near(b, Point::new(150.0, 50.0)), "between the crossings: {a:?} {b:?}");
    // and it is the piece the click deletes
    ops::apply_virtual_cut(&mut doc, &cut);
    assert!(ops::virtual_cut_at(&doc, 0, Point::new(120.0, 50.0), 3.0).is_none_or(|c| c.id != cut.id), "the piece is still there");
}
