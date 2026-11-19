//! Shape tool geometry for #104 (polygon nodes), #108 (ellipse pie/arc nodes) and #123 (adding
//! nodes, dragging segments).

use kurbo::{PathEl, Point, Vec2};
use vector_core::object::{ellipse_path, polygon_path, EllipseKind, EllipseShape, PolygonKind, PolygonNodes, PolygonShape};
use vector_core::path::{polyline, Node, NodePath, SubPath};

fn poly(kind: PolygonKind, sides: u32) -> PolygonShape {
    PolygonShape { w: 100.0, h: 100.0, sides, kind, sharpness: 50.0, nodes: PolygonNodes::default() }
}

fn points(bp: &kurbo::BezPath) -> Vec<Point> {
    bp.elements()
        .iter()
        .filter_map(|e| match e {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(*p),
            _ => None,
        })
        .collect()
}

fn near(a: Point, b: Point) -> bool {
    (a - b).hypot() < 1e-6
}

/// A regular polygon is still its corners only, and its nodes are the corners and the middles of
/// the sides.
#[test]
fn regular_polygon_path_is_unchanged() {
    let p = poly(PolygonKind::Polygon, 5);
    let pts = points(&polygon_path(&p));
    assert_eq!(pts.len(), 5);
    assert!(near(pts[0], Point::new(50.0, 0.0)), "{pts:?}");
    let nodes = p.node_points();
    assert_eq!(nodes.len(), 10);
    // the inner node after corner 0 is the middle of the side from corner 0 to corner 1
    assert!(near(nodes[1].0, pts[0].midpoint(pts[1])), "{:?}", nodes[1]);
    assert!(nodes[1].1 && !nodes[0].1);
    // serialized as before: no `nodes` field
    let json = serde_json::to_string(&p).unwrap();
    assert!(!json.contains("nodes"), "{json}");
    let back: PolygonShape = serde_json::from_str(r#"{"w":1.0,"h":1.0,"sides":3,"kind":"Polygon","sharpness":0.0}"#).unwrap();
    assert!(back.nodes.is_regular());
}

/// #104: dragging one outer node moves every outer node the same way (all stay on a circle, the
/// same turn), the inner nodes stay, and the object is still a polygon with its sides.
#[test]
fn dragging_a_polygon_node_moves_the_matching_nodes() {
    let mut p = poly(PolygonKind::Polygon, 5);
    let before = p.node_points();
    // corner 0 (top) dragged straight up by 20: every corner moves out by 40% of the radius
    p.move_node(false, 0, Point::new(50.0, -20.0), false);
    assert!((p.nodes.outer_scale - 1.4).abs() < 1e-9, "{:?}", p.nodes);
    assert!(p.nodes.outer_turn.abs() < 1e-9);
    let after = p.node_points();
    let c = Point::new(50.0, 50.0);
    for (b, a) in before.iter().zip(&after) {
        if a.1 {
            assert!(near(a.0, b.0), "an inner node moved: {b:?} → {a:?}");
        } else {
            assert!(((a.0 - c).hypot() - 70.0).abs() < 1e-6, "corner {a:?} is not 70 from the center");
        }
    }
    // the path now alternates corners and the (unmoved) side middles: a star-like shape
    let pts = points(&polygon_path(&p));
    assert_eq!(pts.len(), 10);
    assert!(near(pts[0], Point::new(50.0, -20.0)));
    // turning: a corner dragged sideways turns every corner by the same angle
    let mut q = poly(PolygonKind::Polygon, 4);
    let target = Point::new(100.0, 50.0); // corner 0 (top) to 3 o'clock: a quarter turn
    q.move_node(false, 0, target, false);
    assert!((q.nodes.outer_turn - 90.0).abs() < 1e-6, "{:?}", q.nodes);
    for i in 0..4 {
        let a = q.node_point(false, i);
        let expect = poly(PolygonKind::Polygon, 4).node_point(false, (i + 1) % 4);
        assert!(near(a, expect), "corner {i}: {a:?} vs {expect:?}");
    }
    // Ctrl: only toward or away from the center
    let mut r = poly(PolygonKind::Polygon, 4);
    r.move_node(true, 0, Point::new(50.0, 50.0) + Vec2::new(10.0, -30.0), true);
    assert_eq!(r.nodes.inner_turn, 0.0);
    // the size and the sides still regenerate the shape
    p.sides = 6;
    assert_eq!(points(&polygon_path(&p)).len(), 12);
}

/// #104: a star's inner nodes (valleys) move together; dragging them to the center makes it thin.
#[test]
fn star_inner_nodes_move_together() {
    let mut s = poly(PolygonKind::Star, 5);
    let regular = points(&polygon_path(&s));
    assert_eq!(regular.len(), 10);
    let valley = s.node_point(true, 2);
    let to = Point::new(50.0, 50.0) + (valley - Point::new(50.0, 50.0)) * 0.5;
    s.move_node(true, 2, to, false);
    assert!((s.nodes.inner_scale - 0.5).abs() < 1e-9 && s.nodes.inner_turn.abs() < 1e-6, "{:?}", s.nodes);
    let pts = points(&polygon_path(&s));
    for k in 0..5 {
        assert!(near(pts[2 * k], regular[2 * k]), "tip {k} moved");
        let (r0, r1) = ((regular[2 * k + 1] - Point::new(50.0, 50.0)).hypot(), (pts[2 * k + 1] - Point::new(50.0, 50.0)).hypot());
        assert!((r1 - r0 * 0.5).abs() < 1e-6, "valley {k}: {r0} → {r1}");
    }
    // a complex star has only its corners
    assert_eq!(poly(PolygonKind::ComplexStar, 7).node_points().len(), 7);
}

fn ellipse() -> EllipseShape {
    EllipseShape { w: 200.0, h: 100.0, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false }
}

/// #108: the node of a whole ellipse sits at the start angle; dragging it inside makes a pie,
/// outside an arc; the wedge opens where it is dragged.
#[test]
fn ellipse_node_drag_makes_a_pie_or_an_arc() {
    let e = ellipse();
    let nodes = e.nodes();
    assert_eq!(nodes.len(), 1);
    assert!(near(nodes[0].0, Point::new(100.0, 0.0)), "{nodes:?}");
    // not moved off the node's angle yet: undecided
    let (same, h) = e.drag_node(None, Point::new(100.0, -5.0), None);
    assert!(h.is_none() && same == e);
    // dragged clockwise (toward 3 o'clock), inside: a pie whose end follows
    let (pie, h) = e.drag_node(None, Point::new(150.0, 20.0), None);
    assert_eq!(h, Some(false));
    assert_eq!(pie.kind, EllipseKind::Pie);
    assert_eq!(pie.start, 90.0);
    assert!(pie.end > 0.0 && pie.end < 90.0, "{pie:?}");
    // the visible part runs counter-clockwise from 90° to the end: almost all of the ellipse
    assert!(near(e.angle_point(pie.end), e.angle_point(e.angle_of(Point::new(150.0, 20.0)))));
    let bp = ellipse_path(&pie);
    assert!(matches!(bp.elements()[0], PathEl::MoveTo(p) if near(p, Point::new(100.0, 50.0))), "a pie starts at the center");
    // the same drag outside: an arc
    let (arc, h) = e.drag_node(None, Point::new(250.0, 20.0), None);
    assert_eq!((arc.kind, h), (EllipseKind::Arc, Some(false)));
    // dragged counter-clockwise: the start follows
    let (pie2, h) = e.drag_node(None, Point::new(40.0, 20.0), None);
    assert_eq!(h, Some(true));
    assert!(pie2.start > 90.0 && pie2.start < 180.0 && pie2.end == 90.0, "{pie2:?}");
    // a pie has two nodes; dragging the end node moves only the end, and it can go back to a pie
    assert_eq!(pie.nodes().len(), 2);
    let (p3, h) = arc.drag_node(Some(false), Point::new(100.0, 90.0), None);
    assert_eq!((p3.kind, h, p3.start), (EllipseKind::Pie, Some(false), 90.0));
    assert!((p3.end - 270.0).abs() < 1e-6, "{p3:?}");
    // Ctrl: steps of the constrain angle
    let (p4, _) = arc.drag_node(Some(true), Point::new(180.0, 10.0), Some(15.0));
    assert!((p4.start / 15.0 - (p4.start / 15.0).round()).abs() < 1e-9, "{p4:?}");
    // a clockwise ellipse opens the other way
    let cw = EllipseShape { clockwise: true, ..ellipse() };
    assert_eq!(cw.drag_node(None, Point::new(150.0, 20.0), None).1, Some(true));
}

fn curve() -> NodePath {
    // 0 -- 1 -- 2 -- 3, open, straight
    polyline(&[Point::new(0.0, 0.0), Point::new(100.0, 0.0), Point::new(200.0, 0.0), Point::new(300.0, 0.0)], false)
}

/// #123: `+` with two neighbouring nodes adds one node between them (not one after each), and the
/// new node is returned to be selected.
#[test]
fn add_nodes_between_two_selected_neighbours() {
    let mut np = curve();
    let new = np.add_nodes(&[(0, 1), (0, 2)]);
    assert_eq!(np.node_count(), 5);
    assert_eq!(new, vec![(0, 2)]);
    assert!(near(np.subpaths[0].nodes[2].p, Point::new(150.0, 0.0)));
    // again on the new node: the middle of the segment leaving it
    let new = np.add_nodes(&new);
    assert_eq!(new, vec![(0, 3)]);
    assert!(near(np.subpaths[0].nodes[3].p, Point::new(175.0, 0.0)));
    // nodes that are not neighbours: each leaving segment; the last node of an open curve has none
    let mut np = curve();
    let new = np.add_nodes(&[(0, 0), (0, 2), (0, 3)]);
    // 2 and 3 are neighbours: only between them
    assert_eq!(new, vec![(0, 3)]);
    let mut np = curve();
    let new = np.add_nodes(&[(0, 0), (0, 3)]);
    assert_eq!(new, vec![(0, 1)]);
    assert_eq!(np.node_count(), 5);
    // closed curve: the last and first nodes are neighbours
    let mut sq = polyline(&[Point::new(0.0, 0.0), Point::new(100.0, 0.0), Point::new(100.0, 100.0), Point::new(0.0, 100.0)], true);
    let new = sq.add_nodes(&[(0, 3), (0, 0)]);
    assert_eq!(new, vec![(0, 4)]);
    assert!(near(sq.subpaths[0].nodes[4].p, Point::new(0.0, 50.0)));
    // every node selected: every segment gets a node
    let mut sq = polyline(&[Point::new(0.0, 0.0), Point::new(100.0, 0.0), Point::new(100.0, 100.0)], true);
    let new = sq.add_nodes(&sq.all_refs());
    assert_eq!(new, vec![(0, 1), (0, 3), (0, 5)]);
    assert_eq!(sq.node_count(), 6);
}

/// #123: dragging a straight segment moves it (both nodes); dragging a curved one bends it through
/// the point dragged while its nodes stay.
#[test]
fn segment_drag_moves_a_line_and_bends_a_curve() {
    let mut np = curve();
    np.move_segment(0, 1, Vec2::new(0.0, 30.0));
    let n = &np.subpaths[0].nodes;
    assert!(near(n[1].p, Point::new(100.0, 30.0)) && near(n[2].p, Point::new(200.0, 30.0)));
    assert!(near(n[0].p, Point::new(0.0, 0.0)) && near(n[3].p, Point::new(300.0, 0.0)));
    // a line can't bend
    let mut np = curve();
    np.bend_segment(0, 0, 0.5, Vec2::new(0.0, 30.0));
    assert_eq!(np, curve());
    // a curve: the point at t follows the pointer
    let mut a = Node::corner(Point::new(0.0, 0.0));
    a.line_out = false;
    a.c_out = Point::new(30.0, 0.0);
    let mut b = Node::corner(Point::new(90.0, 0.0));
    b.c_in = Point::new(60.0, 0.0);
    let mut np = NodePath { subpaths: vec![SubPath { nodes: vec![a, b], closed: false }] };
    for t in [0.1, 0.3, 0.5, 0.8] {
        let mut bent = np.clone();
        let p0 = kurbo::ParamCurve::eval(&bent.subpaths[0].segment(0).unwrap().0, t);
        bent.bend_segment(0, 0, t, Vec2::new(5.0, 20.0));
        let p1 = kurbo::ParamCurve::eval(&bent.subpaths[0].segment(0).unwrap().0, t);
        assert!(near(p1, p0 + Vec2::new(5.0, 20.0)), "t {t}: {p0:?} → {p1:?}");
        assert!(near(bent.subpaths[0].nodes[0].p, a.p) && near(bent.subpaths[0].nodes[1].p, b.p));
    }
    np.bend_segment(0, 0, 0.5, Vec2::ZERO);
    assert_eq!(np.subpaths[0].nodes[0].c_out, Point::new(30.0, 0.0));
}
