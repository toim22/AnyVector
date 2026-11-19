//! Regression tests for the pick and edit operations fixed after 0.5.4: Virtual Segment Delete on
//! shapes that aren't curves yet (#117) and Trim that deleted a shape (#118).

use kurbo::{Affine, BezPath, Point, Shape};
use vector_core::object::{CornerType, EllipseKind, EllipseShape, PolygonKind, PolygonShape, RectShape, SpiralShape};
use vector_core::ops::{self, Shaping};
use vector_core::path::NodePath;
use vector_core::{Document, ObjKind, Object, Style};

fn push(doc: &mut Document, kind: ObjKind, at: (f64, f64)) -> u64 {
    let id = doc.new_id();
    let o = Object::new(id, kind, Affine::translate(at), Style::default());
    doc.pages[0].layers.last_mut().unwrap().objects.push(o);
    id
}

fn rect(w: f64, h: f64) -> ObjKind {
    ObjKind::Rect(RectShape { w, h, radii: [0.0; 4], corner: CornerType::default() })
}

fn poly(pts: &[(f64, f64)], closed: bool) -> ObjKind {
    let mut bp = BezPath::new();
    bp.move_to(pts[0]);
    for p in &pts[1..] {
        bp.line_to(*p);
    }
    if closed {
        bp.close_path();
    }
    ObjKind::Path(NodePath::from_bez(&bp))
}

fn count(doc: &Document) -> usize {
    doc.pages[0].all_objects().count()
}

fn area(doc: &Document, id: u64) -> f64 {
    doc.find(id).unwrap().world_path().area().abs()
}

fn curve(doc: &Document, id: u64) -> NodePath {
    match &doc.find(id).expect("the object is gone").kind {
        ObjKind::Path(np) => np.clone(),
        k => panic!("not converted to curves: {k:?}"),
    }
}

fn segments(np: &NodePath) -> usize {
    np.subpaths.iter().map(|s| s.segment_count()).sum()
}

// ---- #117: Virtual Segment Delete ----

/// Delete the piece under `p` as the tool does.
fn vdelete(doc: &mut Document, p: Point) -> Option<Option<u64>> {
    let cut = ops::virtual_cut_at(doc, 0, p, 3.0)?;
    Some(ops::apply_virtual_cut(doc, &cut))
}

#[test]
fn virtual_delete_works_on_a_rectangle() {
    let mut doc = Document::new(400.0, 400.0);
    let id = push(&mut doc, rect(100.0, 80.0), (20.0, 20.0));
    let cut = ops::virtual_cut_at(&doc, 0, Point::new(70.0, 20.0), 3.0).expect("the rectangle's edge wasn't found");
    assert!(cut.convert, "a rectangle is converted to curves first");
    assert_eq!(ops::apply_virtual_cut(&mut doc, &cut), Some(id));
    let np = curve(&doc, id);
    assert!(!np.is_closed(), "one side was deleted: the curve is open");
    assert_eq!(segments(&np), 3);
    // the other three sides are where they were
    let bb = doc.find(id).unwrap().world_path().bounding_box();
    assert!((bb.x0 - 20.0).abs() < 1e-6 && (bb.x1 - 120.0).abs() < 1e-6 && (bb.y1 - 100.0).abs() < 1e-6, "{bb:?}");
}

#[test]
fn virtual_delete_works_on_an_ellipse() {
    let mut doc = Document::new(400.0, 400.0);
    let e = EllipseShape { w: 200.0, h: 100.0, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false };
    let id = push(&mut doc, ObjKind::Ellipse(e), (50.0, 50.0));
    let bb = doc.find(id).unwrap().world_path().bounding_box();
    let before = segments(&doc.find(id).unwrap().to_curve().unwrap());
    // the top of the ellipse
    assert_eq!(vdelete(&mut doc, Point::new(bb.center().x + 20.0, bb.y0)), Some(Some(id)));
    let np = curve(&doc, id);
    assert!(!np.is_closed());
    assert_eq!(segments(&np), before - 1);
}

#[test]
fn virtual_delete_works_on_a_polygon_and_a_spiral() {
    let mut doc = Document::new(400.0, 400.0);
    let p = PolygonShape { w: 100.0, h: 100.0, sides: 5, kind: PolygonKind::Polygon, sharpness: 0.0, nodes: Default::default() };
    let pid = push(&mut doc, ObjKind::Polygon(p), (20.0, 20.0));
    let pc = doc.find(pid).unwrap().to_curve().unwrap();
    let (c, _) = pc.subpaths[0].segment(1).unwrap();
    let mid = kurbo::ParamCurve::eval(&c, 0.5);
    assert_eq!(vdelete(&mut doc, mid), Some(Some(pid)));
    let np = curve(&doc, pid);
    assert!(!np.is_closed());
    assert_eq!(segments(&np), 4);

    let s = SpiralShape { w: 100.0, h: 100.0, revolutions: 3.0, logarithmic: false, expansion: 1.0 };
    let sid = push(&mut doc, ObjKind::Spiral(s), (250.0, 250.0));
    let sc = doc.find(sid).unwrap().to_curve().unwrap();
    let before = segments(&sc);
    let (c, _) = sc.subpaths[0].segment(0).unwrap();
    let start = kurbo::ParamCurve::eval(&c, 0.5);
    assert_eq!(vdelete(&mut doc, start), Some(Some(sid)));
    let np = curve(&doc, sid);
    assert!(segments(&np) < before, "the spiral didn't lose a segment");
}

#[test]
fn virtual_delete_stops_at_an_intersection() {
    let mut doc = Document::new(400.0, 400.0);
    let line = doc.new_id();
    doc.pages[0].layers.last_mut().unwrap().objects.push(Object::new(line, poly(&[(0.0, 50.0), (200.0, 50.0)], false), Affine::IDENTITY, Style::default()));
    let cross = doc.new_id();
    doc.pages[0].layers.last_mut().unwrap().objects.push(Object::new(cross, poly(&[(100.0, 0.0), (100.0, 100.0)], false), Affine::IDENTITY, Style::default()));
    assert_eq!(vdelete(&mut doc, Point::new(40.0, 50.0)), Some(Some(line)));
    let bb = doc.find(line).unwrap().world_path().bounding_box();
    assert!((bb.x0 - 100.0).abs() < 1.0 && (bb.x1 - 200.0).abs() < 1e-6, "only the piece left of the crossing goes: {bb:?}");
    assert!(doc.find(cross).is_some(), "the crossing line was touched");
}

#[test]
fn virtual_delete_of_the_closing_segment_opens_the_curve() {
    let mut doc = Document::new(400.0, 400.0);
    let id = doc.new_id();
    let tri = poly(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)], true);
    doc.pages[0].layers.last_mut().unwrap().objects.push(Object::new(id, tri, Affine::IDENTITY, Style::default()));
    // the closing segment runs from (100, 100) back to (0, 0)
    assert_eq!(vdelete(&mut doc, Point::new(50.0, 50.0)), Some(Some(id)));
    let np = curve(&doc, id);
    assert!(!np.is_closed());
    assert_eq!(segments(&np), 2);
    let nodes = &np.subpaths[0].nodes;
    assert_eq!((nodes[0].p, nodes[nodes.len() - 1].p), (Point::new(0.0, 0.0), Point::new(100.0, 100.0)));
}

#[test]
fn virtual_delete_on_empty_space_changes_nothing_and_the_last_segment_removes_the_object() {
    let mut doc = Document::new(400.0, 400.0);
    let id = push(&mut doc, rect(100.0, 80.0), (20.0, 20.0));
    assert_eq!(vdelete(&mut doc, Point::new(300.0, 300.0)), None);
    assert!(matches!(doc.find(id).unwrap().kind, ObjKind::Rect(_)), "a miss converted the shape");
    let line = doc.new_id();
    doc.pages[0].layers.last_mut().unwrap().objects.push(Object::new(line, poly(&[(200.0, 300.0), (380.0, 300.0)], false), Affine::IDENTITY, Style::default()));
    assert_eq!(vdelete(&mut doc, Point::new(290.0, 300.0)), Some(None));
    assert!(doc.find(line).is_none(), "a curve with no segment left stays");
}

// ---- #118: Trim and the other shaping operations ----

/// `a` (source) 100×100 at the origin and `b` (target) 100×100 at `at`.
fn two_squares(at: (f64, f64)) -> (Document, u64, u64) {
    let mut doc = Document::new(400.0, 400.0);
    let a = push(&mut doc, rect(100.0, 100.0), (0.0, 0.0));
    let b = push(&mut doc, rect(100.0, 100.0), at);
    (doc, a, b)
}

#[test]
fn trim_keeps_the_source_when_asked() {
    for leave_source in [true, false] {
        let (mut doc, a, b) = two_squares((50.0, 50.0));
        let out = ops::shaping(&mut doc, &[a, b], Shaping::Trim, leave_source, false);
        assert_eq!(out, vec![b]);
        assert!((area(&doc, b) - 7500.0).abs() < 1.0, "{}", area(&doc, b));
        assert_eq!(doc.find(a).is_some(), leave_source, "leave_source={leave_source}");
    }
    // leave the target too: its original stays and the result is a new curve
    let (mut doc, a, b) = two_squares((50.0, 50.0));
    let out = ops::shaping(&mut doc, &[a, b], Shaping::Trim, true, true);
    assert_eq!(out.len(), 1);
    assert_ne!(out[0], b);
    assert!(matches!(doc.find(b).unwrap().kind, ObjKind::Rect(_)));
    assert_eq!(count(&doc), 3);
}

#[test]
fn trim_without_overlap_deletes_nothing() {
    let (mut doc, a, b) = two_squares((200.0, 200.0));
    assert!(!ops::shaping_has_effect(&doc, &[a, b], Shaping::Trim));
    assert!(ops::shaping(&mut doc, &[a, b], Shaping::Trim, false, false).is_empty());
    assert_eq!(count(&doc), 2);
    assert!(matches!(doc.find(a).unwrap().kind, ObjKind::Rect(_)) && matches!(doc.find(b).unwrap().kind, ObjKind::Rect(_)));
    for op in [Shaping::Intersect, Shaping::FrontMinusBack, Shaping::BackMinusFront] {
        assert!(!ops::shaping_has_effect(&doc, &[a, b], op), "{op:?}");
        assert!(ops::shaping(&mut doc, &[a, b], op, false, false).is_empty());
        assert_eq!(count(&doc), 2, "{op:?} deleted an object that doesn't overlap");
    }
    // Weld still combines objects that don't touch
    assert!(ops::shaping_has_effect(&doc, &[a, b], Shaping::Weld));
}

#[test]
fn a_source_that_does_not_overlap_is_not_consumed() {
    let (mut doc, a, b) = two_squares((50.0, 50.0));
    let far = push(&mut doc, rect(20.0, 20.0), (300.0, 300.0));
    ops::shaping(&mut doc, &[far, a, b], Shaping::Trim, false, false);
    assert!(doc.find(a).is_none(), "the overlapping source wasn't used up");
    assert!(doc.find(far).is_some(), "a source away from the target was deleted");
}

#[test]
fn trim_by_a_covering_source_removes_the_target_only() {
    let mut doc = Document::new(400.0, 400.0);
    let big = push(&mut doc, rect(200.0, 200.0), (0.0, 0.0));
    let small = push(&mut doc, rect(50.0, 50.0), (20.0, 20.0));
    let out = ops::shaping(&mut doc, &[big, small], Shaping::Trim, true, false);
    assert!(out.is_empty());
    assert!(doc.find(small).is_none(), "nothing is left of the target");
    assert!(doc.find(big).is_some());
}

#[test]
fn front_and_back_minus_follow_the_leave_options() {
    // b is in front of a
    let (mut doc, a, b) = two_squares((50.0, 50.0));
    let out = ops::shaping(&mut doc, &[a, b], Shaping::FrontMinusBack, true, false);
    assert_eq!(out, vec![b]);
    assert!(doc.find(a).is_some(), "leave_source kept the back object");
    assert!((area(&doc, b) - 7500.0).abs() < 1.0);
    let (mut doc, a, b) = two_squares((50.0, 50.0));
    let out = ops::shaping(&mut doc, &[a, b], Shaping::BackMinusFront, false, false);
    assert_eq!(out, vec![a]);
    assert!(doc.find(b).is_none());
    assert!((area(&doc, a) - 7500.0).abs() < 1.0);
}
