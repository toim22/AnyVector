//! Tools: knife cuts, eraser nibs, envelope edges, separated table cells, connector routing and
//! variable outlines.

use kurbo::{Affine, BezPath, Point, Rect, Shape, Vec2};
use vector_core::object::{
    connector_path, envelope_warp, ConnectorEnd, ConnectorKind, ConnectorObj, CornerType, EnvelopeEffect, RectShape, TableObj,
};
use vector_core::ops::{self, KnifeOpts};
use vector_core::style::{var_width_at, VarWidth};
use vector_core::{Document, ObjKind, Object, Style};

fn doc_with_square() -> (Document, u64) {
    let mut doc = Document::new(400.0, 400.0);
    let id = doc.new_id();
    let r = RectShape { w: 100.0, h: 100.0, radii: [0.0; 4], corner: CornerType::default() };
    let o = Object::new(id, ObjKind::Rect(r), Affine::translate((50.0, 50.0)), Style::default());
    doc.pages[0].layers.last_mut().unwrap().objects.push(o);
    (doc, id)
}

#[test]
fn knife_path_cuts_a_square_in_two() {
    let (mut doc, _) = doc_with_square();
    let cut = [Point::new(100.0, 20.0), Point::new(100.0, 180.0)];
    let ids = ops::knife_path(&mut doc, 0, &cut, KnifeOpts { auto_close: true, ..Default::default() });
    assert_eq!(ids.len(), 2, "two pieces");
    let mut areas: Vec<f64> = ids.iter().map(|id| doc.find(*id).unwrap().world_path().area().abs()).collect();
    areas.sort_by(|a, b| a.total_cmp(b));
    assert!((areas[0] - 5000.0).abs() < 50.0 && (areas[1] - 5000.0).abs() < 50.0, "{areas:?}");
}

#[test]
fn knife_gap_removes_a_strip() {
    let (mut doc, _) = doc_with_square();
    let cut = [Point::new(100.0, 20.0), Point::new(100.0, 180.0)];
    let ids = ops::knife_path(&mut doc, 0, &cut, KnifeOpts { span: 1, width: 10.0, auto_close: true, outline: 0 });
    assert_eq!(ids.len(), 2);
    let total: f64 = ids.iter().map(|id| doc.find(*id).unwrap().world_path().area().abs()).sum();
    assert!((total - 9000.0).abs() < 100.0, "a 10 pt gap leaves 9000, got {total}");
}

#[test]
fn square_eraser_nib_takes_more_than_a_round_one() {
    let stroke = [Point::new(100.0, 100.0)];
    let area_after = |square: bool| {
        let (mut doc, _) = doc_with_square();
        let ids = ops::erase_nib(&mut doc, 0, &stroke, 20.0, square);
        ids.iter().map(|id| doc.find(*id).unwrap().world_path().area().abs()).sum::<f64>()
    };
    let (round, square) = (area_after(false), area_after(true));
    assert!(round < 10000.0 && square < round, "round {round}, square {square}");
}

fn warp_line(mode: u8) -> BezPath {
    let mut env = EnvelopeEffect { handles: [Vec2::ZERO; 8], mode, mapping: 1, keep_lines: false };
    // pull the top middle up and the top-right corner up
    env.handles[1] = Vec2::new(0.0, -0.3);
    let mut top = BezPath::new();
    top.move_to((0.0, 0.0));
    top.line_to((50.0, 0.0));
    top.line_to((100.0, 0.0));
    envelope_warp(&top, Rect::new(0.0, 0.0, 100.0, 100.0), &env)
}

#[test]
fn envelope_edge_modes() {
    // straight line: the middle node does not bend the edge
    let straight = warp_line(0).bounding_box();
    assert!(straight.y0.abs() < 1e-6, "straight edge stays on the line: {straight:?}");
    // single arc and unconstrained: the edge bulges toward the middle node
    for mode in [1, 3] {
        let b = warp_line(mode).bounding_box();
        assert!(b.y0 < -5.0, "mode {mode} bends the edge: {b:?}");
    }
}

#[test]
fn separated_cells_and_margins() {
    let mut t = TableObj::new(2, 2, 200.0, 100.0);
    assert_eq!(t.cell_box(0, 0), Rect::new(0.0, 0.0, 100.0, 50.0));
    t.spacing = Some((4.0, 6.0));
    assert_eq!(t.cell_box(0, 0), Rect::new(2.0, 3.0, 98.0, 47.0));
    t.margins = [1.0, 2.0, 3.0, 4.0];
    assert_eq!(t.text_rect(0, 0), Rect::new(5.0, 4.0, 94.0, 45.0));
}

#[test]
fn rows_grow_to_fit_their_text() {
    let mut t = TableObj::new(2, 1, 60.0, 20.0);
    let h0 = t.row_heights[0];
    t.cell_mut(0, 0).text = "one two three four five six seven eight nine ten".into();
    t.fit_rows();
    assert!(t.row_heights[0] > h0, "row grew from {h0} to {}", t.row_heights[0]);
    assert_eq!(t.row_heights[1], h0, "empty rows stay");
}

#[test]
fn right_angle_connectors_leave_anchors_in_their_direction() {
    let c = ConnectorObj {
        kind: ConnectorKind::RightAngle,
        // leaves to the right, arrives from above
        start: ConnectorEnd { point: Point::new(0.0, 0.0), attached: None, dir: Some(0.0) },
        end: ConnectorEnd { point: Point::new(100.0, 100.0), attached: None, dir: Some(90.0) },
        corner_radius: 0.0,
    };
    let pts: Vec<Point> = connector_path(&c).elements().iter().filter_map(|e| e.end_point()).collect();
    assert_eq!(pts, vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0), Point::new(100.0, 100.0)]);
}

#[test]
fn variable_width_interpolates_between_nodes() {
    let nodes = [VarWidth { t: 0.0, side1: 1.0, side2: 1.0 }, VarWidth { t: 1.0, side1: 5.0, side2: 3.0 }];
    assert_eq!(var_width_at(&nodes, 0.0), (1.0, 1.0));
    assert_eq!(var_width_at(&nodes, 1.0), (5.0, 3.0));
    let (a, b) = var_width_at(&nodes, 0.5);
    assert!((a - 3.0).abs() < 1e-9 && (b - 2.0).abs() < 1e-9);
}

fn line_doc(n: usize) -> (Document, Vec<u64>, u64) {
    let mut doc = Document::new(600.0, 600.0);
    let mut ids = vec![];
    for k in 0..n {
        let id = doc.new_id();
        let r = RectShape { w: 20.0, h: 10.0, radii: [0.0; 4], corner: CornerType::default() };
        doc.pages[0].layers.last_mut().unwrap().objects.push(Object::new(id, ObjKind::Rect(r), Affine::translate((40.0 * k as f64, 200.0)), Style::default()));
        ids.push(id);
    }
    let pid = doc.new_id();
    let line = vector_core::path::polyline(&[Point::new(100.0, 100.0), Point::new(400.0, 100.0)], false);
    doc.pages[0].layers.last_mut().unwrap().objects.push(Object::new(pid, ObjKind::Path(line), Affine::IDENTITY, Style::default()));
    (doc, ids, pid)
}

#[test]
fn fit_objects_spreads_them_along_the_path() {
    let (mut doc, ids, pid) = line_doc(3);
    let o = ops::FitPathOptions { even_reference: true, ..Default::default() };
    let out = ops::fit_objects_to_path(&mut doc, &ids, pid, &o);
    assert_eq!(out, ids, "the originals move");
    let c: Vec<Point> = ids.iter().map(|i| doc.bounds_of(doc.find(*i).unwrap()).center()).collect();
    for (k, x) in [100.0, 250.0, 400.0].into_iter().enumerate() {
        assert!((c[k].x - x).abs() < 0.5 && (c[k].y - 100.0).abs() < 0.5, "{c:?}");
    }
}

#[test]
fn fit_objects_keeps_originals_adds_duplicates_and_groups() {
    let (mut doc, ids, pid) = line_doc(2);
    let o = ops::FitPathOptions { keep_originals: true, duplicates: 1, group: true, order: 1, ..Default::default() };
    let out = ops::fit_objects_to_path(&mut doc, &ids, pid, &o);
    assert_eq!(out.len(), 1, "one group");
    match &doc.find(out[0]).unwrap().kind {
        ObjKind::Group(c) => assert_eq!(c.len(), 4, "two copies and two duplicates"),
        k => panic!("{k:?}"),
    }
    // the originals stay
    assert!((doc.bounds_of(doc.find(ids[0]).unwrap()).center().y - 205.0).abs() < 1e-6);
}

#[test]
fn fit_objects_progressive_rotation_turns_them() {
    let (mut doc, ids, pid) = line_doc(3);
    let o = ops::FitPathOptions { style: 2, start_angle: 0.0, spin_angle: 90.0, revolutions: 1.0, ..Default::default() };
    ops::fit_objects_to_path(&mut doc, &ids, pid, &o);
    // the last one turned a quarter: taller than wide
    let b = doc.bounds_of(doc.find(ids[2]).unwrap());
    assert!(b.height() > b.width(), "{b:?}");
    let b0 = doc.bounds_of(doc.find(ids[0]).unwrap());
    assert!(b0.width() > b0.height(), "{b0:?}");
}

fn two_lines() -> (Document, Vec<u64>) {
    let mut doc = Document::new(400.0, 400.0);
    let mut ids = vec![];
    for pts in [[Point::new(0.0, 100.0), Point::new(40.0, 10.0)], [Point::new(60.0, 10.0), Point::new(100.0, 100.0)]] {
        let id = doc.new_id();
        let np = vector_core::path::polyline(&pts, false);
        doc.pages[0].layers.last_mut().unwrap().objects.push(Object::new(id, ObjKind::Path(np), Affine::IDENTITY, Style::default()));
        ids.push(id);
    }
    (doc, ids)
}

fn joined_nodes(doc: &Document, id: u64) -> Vec<Point> {
    match &doc.find(id).unwrap().kind {
        ObjKind::Path(np) => {
            assert_eq!(np.subpaths.len(), 1, "one curve");
            let t = doc.find(id).unwrap().transform;
            np.subpaths[0].nodes.iter().map(|n| t * n.p).collect()
        }
        k => panic!("{k:?}"),
    }
}

#[test]
fn join_curves_extend_meets_where_the_ends_point() {
    let (mut doc, ids) = two_lines();
    let id = ops::join_curves_ex(&mut doc, &ids, 30.0, ops::JoinMode::Extend, 0.0).unwrap();
    let pts = joined_nodes(&doc, id);
    assert_eq!(pts.len(), 5, "{pts:?}");
    assert!(pts.iter().any(|p| (p.x - 50.0).abs() < 1e-6 && (p.y + 12.5).abs() < 1e-6), "{pts:?}");
}

#[test]
fn join_curves_chamfer_and_fillet() {
    let (mut doc, ids) = two_lines();
    let id = ops::join_curves_ex(&mut doc, &ids, 30.0, ops::JoinMode::Chamfer, 0.0).unwrap();
    assert_eq!(joined_nodes(&doc, id).len(), 4, "a straight segment between the ends");
    let (mut doc, ids) = two_lines();
    let id = ops::join_curves_ex(&mut doc, &ids, 30.0, ops::JoinMode::Fillet, 5.0).unwrap();
    let pts = joined_nodes(&doc, id);
    assert_eq!(pts.len(), 6, "{pts:?}");
    // the rounded corner stays under the meeting point
    assert!(pts.iter().all(|p| p.y > -12.5 + 1e-6), "{pts:?}");
}

#[test]
fn join_curves_leaves_far_ends_alone() {
    let (mut doc, ids) = two_lines();
    let id = ops::join_curves_ex(&mut doc, &ids, 5.0, ops::JoinMode::Bezier, 0.0).unwrap();
    match &doc.find(id).unwrap().kind {
        ObjKind::Path(np) => assert_eq!(np.subpaths.len(), 2),
        k => panic!("{k:?}"),
    }
}
