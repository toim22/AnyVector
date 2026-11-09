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
