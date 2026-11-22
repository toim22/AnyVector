//! Blends along a path (#145): stations by arclength, closed paths and their seam, the controls
//! kept on the path, Blend Along Full Path, Rotate All Objects, Fixed Spacing, split and fuse,
//! Map Nodes, New Start / New End, Detach, Delete of a part, picking and the file format.

use kurbo::{Affine, BezPath, Point, Shape};
use vector_core::blend_path::{self as bp, BlendError, Fuse, Stations};
use vector_core::object::{blend_steps, BlendObj, BlendPart, CornerType, EllipseKind, EllipseShape, PolygonKind, PolygonShape, RectShape};
use vector_core::{Color, Document, Fill, ObjKind, Object, Style};

fn solid(c: Color) -> Style {
    Style { fill: Fill::Solid(c), ..Style::default() }
}

fn circle(doc: &mut Document, c: (f64, f64), r: f64) -> Object {
    let e = EllipseShape { w: 2.0 * r, h: 2.0 * r, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false };
    Object::new(doc.new_id(), ObjKind::Ellipse(e), Affine::translate((c.0 - r, c.1 - r)), solid(Color::rgb(200, 40, 40)))
}

fn rect(doc: &mut Document, c: (f64, f64), w: f64, h: f64, color: Color) -> Object {
    Object::new(doc.new_id(), ObjKind::Rect(RectShape { w, h, radii: [0.0; 4], corner: CornerType::default() }), Affine::translate((c.0 - w / 2.0, c.1 - h / 2.0)), solid(color))
}

fn curve(doc: &mut Document, bez: &BezPath) -> Object {
    Object::new(doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(bez)), Affine::IDENTITY, Style::default())
}

fn push(doc: &mut Document, o: Object) -> u64 {
    let id = o.id;
    doc.pages[0].layers.last_mut().unwrap().objects.push(o);
    id
}

/// A blend of `a` to `b` with `steps` steps on the page; gives its id.
fn blend(doc: &mut Document, a: Object, b: Object, steps: u32) -> u64 {
    let (ia, ib) = (push(doc, a), push(doc, b));
    vector_core::ops::create_blend(doc, ia, ib, steps).unwrap()
}

fn get(doc: &Document, id: u64) -> &BlendObj {
    bp::blend_of(doc, id).unwrap()
}

fn centres(b: &BlendObj) -> Vec<Point> {
    blend_steps(b, || 0).iter().map(|o| o.bounds().center()).collect()
}

/// The direction (degrees) from an object's middle to the first node of its outline: how far a
/// step is turned.
fn turn_of(o: &Object) -> f64 {
    let c = o.bounds().center();
    let first = o.world_path().elements().iter().find_map(|e| match e {
        kurbo::PathEl::MoveTo(p) => Some(*p),
        _ => None,
    });
    let p = first.unwrap();
    (p.y - c.y).atan2(p.x - c.x).to_degrees()
}

fn wrap180(d: f64) -> f64 {
    let mut d = d % 360.0;
    if d > 180.0 {
        d -= 360.0;
    } else if d <= -180.0 {
        d += 360.0;
    }
    d
}

/// A blend of two small rectangles placed on a circle of radius 100 about the origin, the circle
/// attached as its path. Gives (document, blend, start id, end id).
fn circle_blend(steps: u32) -> (Document, u64, u64, u64) {
    let mut doc = Document::new(800.0, 600.0);
    let a = rect(&mut doc, (100.0, 0.0), 20.0, 6.0, Color::rgb(200, 0, 0));
    let b = rect(&mut doc, (-100.0, 0.0), 20.0, 6.0, Color::rgb(0, 0, 200));
    let (ia, ib) = (a.id, b.id);
    let id = blend(&mut doc, a, b, steps);
    let mut c = circle(&mut doc, (0.0, 0.0), 100.0);
    c.style.fill = Fill::None;
    let cid = push(&mut doc, c);
    bp::set_path(&mut doc, id, cid).unwrap();
    (doc, id, ia, ib)
}

#[test]
fn stations_are_exact_by_arclength_and_safe_at_the_ends() {
    // a straight cubic with very uneven parameterisation: stations are still evenly spaced
    let mut bez = BezPath::new();
    bez.move_to((0.0, 0.0));
    bez.curve_to((1.0, 0.0), (2.0, 0.0), (400.0, 0.0));
    let st = Stations::new(&bez, &[]).unwrap();
    assert!((st.total() - 400.0).abs() < 0.01, "{}", st.total());
    for k in 0..=8 {
        let s = st.total() * k as f64 / 8.0;
        assert!((st.point(s).x - s).abs() < 0.05, "station {k} at {:?}", st.point(s));
    }
    // the boundaries (#102 class): no NaN, no hang, the last point exactly
    for s in [st.total(), st.total() + 1e-12, st.total() * 2.0, -1.0, f64::NAN, f64::INFINITY] {
        let (p, a) = st.at(s);
        assert!(p.x.is_finite() && p.y.is_finite() && a.is_finite(), "s = {s}: {p:?} {a}");
    }
    assert_eq!(st.point(st.total()), Point::new(400.0, 0.0));
    assert_eq!(st.point(0.0), Point::new(0.0, 0.0));
    assert!((st.project(Point::new(123.0, 40.0)) - 123.0).abs() < 1e-6);
    // a path of no length can't be followed
    let mut dot = BezPath::new();
    dot.move_to((5.0, 5.0));
    dot.line_to((5.0, 5.0));
    assert!(Stations::new(&dot, &[]).is_none());
    // a closed path wraps round
    let c = kurbo::Circle::new((0.0, 0.0), 100.0).to_path(0.01);
    let st = Stations::new(&c, &[]).unwrap();
    assert!(st.closed());
    assert!((st.total() - 200.0 * std::f64::consts::PI).abs() < 0.5, "{}", st.total());
    assert!((st.point(st.total() + 10.0) - st.point(10.0)).hypot() < 1e-6);
}

#[test]
fn attaching_a_path_consumes_it_and_puts_the_ends_on_it() {
    let (doc, id, ia, ib) = circle_blend(5);
    // the circle left the page: it is the blend's path now
    assert_eq!(doc.pages[0].layers.last().unwrap().objects.len(), 1);
    let b = get(&doc, id);
    assert!(b.path.is_some());
    // the ends were on the circle already: they stay there
    assert!((b.start.bounds().center() - Point::new(100.0, 0.0)).hypot() < 0.5);
    assert!((b.end.bounds().center() - Point::new(-100.0, 0.0)).hypot() < 0.5);
    // the parts are found and named
    assert_eq!(bp::part_of(&doc, ia), Some((id, BlendPart::Start)));
    assert_eq!(bp::part_of(&doc, ib), Some((id, BlendPart::End)));
    let pid = b.path.as_ref().unwrap().obj.id;
    assert_eq!(bp::part_of(&doc, pid), Some((id, BlendPart::Path)));
    assert_eq!(doc.parent_of(pid), Some(id));
    // every step is on the circle, half way round from the start to the end (forward)
    let cs = centres(b);
    assert_eq!(cs.len(), 5);
    for c in &cs {
        assert!((c.to_vec2().hypot() - 100.0).abs() < 0.5, "step off the circle: {c:?}");
    }
    // all on one side: the forward half from the start (angle 0) to the end (angle 180)
    let side = cs[0].y.signum();
    assert!(cs.iter().all(|c| c.y.signum() == side), "the steps took both halves: {cs:?}");
}

#[test]
fn circle_full_path_spreads_evenly_and_closes_without_a_seam() {
    let (mut doc, id, _, _) = circle_blend(3);
    assert!(bp::set_full_path(&mut doc, id, true));
    assert!(bp::set_rotate_all(&mut doc, id, true));
    let b = get(&doc, id).clone();
    // the start and the end meet at the beginning of the circle
    assert!((b.start.bounds().center() - b.end.bounds().center()).hypot() < 0.5);
    let steps = blend_steps(&b, || 0);
    assert_eq!(steps.len(), 3);
    let start_angle = b.start.bounds().center().y.atan2(b.start.bounds().center().x).to_degrees();
    for (k, o) in steps.iter().enumerate() {
        let c = o.bounds().center();
        assert!((c.to_vec2().hypot() - 100.0).abs() < 0.5, "step {k} off the circle: {c:?}");
        // 90° apart round the circle
        let a = c.y.atan2(c.x).to_degrees();
        let want = start_angle + 90.0 * (k + 1) as f64;
        assert!(wrap180(a - want).abs() < 0.5, "step {k} at {a}°, wanted {want}°");
    }
    // each step turned 90° more than the one before (never the long way)
    let turns: Vec<f64> = std::iter::once(&*b.start).chain(steps.iter()).map(turn_of).collect();
    for w in turns.windows(2) {
        let d = wrap180(w[1] - w[0]);
        assert!((d.abs() - 90.0).abs() < 1.0, "turn between steps {d}° ({turns:?})");
    }
    // with many steps every turn is small and the same: no jump at the seam
    let mut doc2 = doc.clone();
    if let Some(ObjKind::Blend(b)) = doc2.find_mut(id).map(|o| &mut o.kind) {
        b.steps = 100;
    }
    let b = get(&doc2, id);
    let steps = blend_steps(b, || 0);
    let turns: Vec<f64> = std::iter::once(&*b.start).chain(steps.iter()).chain(std::iter::once(&*b.end)).map(turn_of).collect();
    let step = 360.0 / 101.0;
    for (i, w) in turns.windows(2).enumerate() {
        let d = wrap180(w[1] - w[0]);
        assert!((d.abs() - step).abs() < 0.5, "turn {i} is {d}°, not {step}°");
    }
    // and the positions are evenly spaced by arclength
    let st = Stations::new(&b.path.as_ref().unwrap().obj.world_path(), &[]).unwrap();
    let ss: Vec<f64> = steps.iter().map(|o| st.project(o.bounds().center())).collect();
    let gap = st.total() / 101.0;
    for w in ss.windows(2) {
        let d = (w[1] - w[0]).rem_euclid(st.total());
        assert!((d - gap).abs() < 0.5, "uneven gap {d} (want {gap})");
    }
}

#[test]
fn closed_path_span_runs_forward_through_the_seam() {
    let mut doc = Document::new(800.0, 600.0);
    let c = circle(&mut doc, (0.0, 0.0), 100.0);
    let st = Stations::new(&c.world_path(), &[]).unwrap();
    let total = st.total();
    // the ends at 0.9 and 0.1 of the circle
    let (p0, p1) = (st.point(0.9 * total), st.point(0.1 * total));
    let a = rect(&mut doc, (p0.x, p0.y), 10.0, 10.0, Color::BLACK);
    let b = rect(&mut doc, (p1.x, p1.y), 10.0, 10.0, Color::WHITE);
    let id = blend(&mut doc, a, b, 7);
    let cid = push(&mut doc, c);
    bp::set_path(&mut doc, id, cid).unwrap();
    let b = get(&doc, id);
    for (k, c) in centres(b).iter().enumerate() {
        let s = st.project(*c) / total;
        assert!(!(0.1 + 1e-3..0.9 - 1e-3).contains(&s), "step {k} went the long way round, at {s}");
    }
}

#[test]
fn steps_follow_an_uneven_curve_evenly_by_arclength() {
    let mut doc = Document::new(800.0, 600.0);
    // a curve whose parameter runs very unevenly
    let mut bez = BezPath::new();
    bez.move_to((0.0, 300.0));
    bez.curve_to((0.0, 300.0), (10.0, 0.0), (500.0, 100.0));
    let st = Stations::new(&bez, &[]).unwrap();
    let a = circle(&mut doc, (0.0, 300.0), 8.0);
    let b = circle(&mut doc, (500.0, 100.0), 8.0);
    let id = blend(&mut doc, a, b, 9);
    let path = curve(&mut doc, &bez);
    let pid = push(&mut doc, path);
    bp::set_path(&mut doc, id, pid).unwrap();
    let ss: Vec<f64> = centres(get(&doc, id)).iter().map(|c| st.project(*c)).collect();
    let gap = st.total() / 10.0;
    for (k, s) in ss.iter().enumerate() {
        let want = gap * (k + 1) as f64;
        assert!((s - want).abs() < 0.5, "step {k} at {s}, wanted {want}");
        // on the curve itself
        assert!((st.point(*s) - centres(get(&doc, id))[k]).hypot() < 0.5);
    }
}

#[test]
fn polygon_path_corners_turn_halfway() {
    let mut doc = Document::new(800.0, 600.0);
    let sq = Object::new(doc.new_id(), ObjKind::Polygon(PolygonShape { kind: PolygonKind::Polygon, sides: 4, w: 200.0, h: 200.0, sharpness: 50.0, nodes: Default::default() }), Affine::translate((0.0, 0.0)), Style::default());
    let st = Stations::new(&sq.world_path(), &[]).unwrap();
    // a corner of the square: the direction there is between the two sides
    let corners: Vec<Point> = sq.world_path().elements().iter().filter_map(|e| match e {
        kurbo::PathEl::LineTo(p) => Some(*p),
        _ => None,
    }).collect();
    let s = st.project(corners[0]);
    let before = st.angle(s - 5.0).to_degrees();
    let after = st.angle(s + 5.0).to_degrees();
    let at = st.angle(s).to_degrees();
    let mid = before + wrap180(after - before) / 2.0;
    assert!(wrap180(at - mid).abs() < 1.0, "corner direction {at}°, sides {before}° and {after}°");
}

#[test]
fn open_path_full_and_short_and_many_steps() {
    let mut doc = Document::new(800.0, 600.0);
    let a = circle(&mut doc, (300.0, 300.0), 10.0);
    let b = circle(&mut doc, (320.0, 300.0), 10.0);
    let id = blend(&mut doc, a, b, 500);
    let mut bez = BezPath::new();
    bez.move_to((0.0, 0.0));
    bez.line_to((100.0, 0.0));
    let o = curve(&mut doc, &bez);
    let pid = push(&mut doc, o);
    // both ends meet on the same point of the line: they go to its two ends instead
    bp::set_path(&mut doc, id, pid).unwrap();
    let b = get(&doc, id);
    assert!((b.start.bounds().center() - Point::new(0.0, 0.0)).hypot() < 0.5, "{:?}", b.start.bounds().center());
    assert!((b.end.bounds().center() - Point::new(100.0, 0.0)).hypot() < 0.5);
    // more steps than the path is long: stacked, no panic, all finite and on the line
    let cs = centres(b);
    assert_eq!(cs.len(), 500);
    assert!(cs.iter().all(|c| c.x.is_finite() && c.y.abs() < 0.5 && (-0.5..=100.5).contains(&c.x)));
    // full path on an open path: from its first to its last point
    assert!(bp::set_full_path(&mut doc, id, true));
    let b = get(&doc, id);
    assert!((b.end.bounds().center() - Point::new(100.0, 0.0)).hypot() < 0.5);
    assert!(!bp::set_full_path(&mut doc, id, true), "nothing to change");
}

#[test]
fn refused_paths_leave_the_blend_alone() {
    let mut doc = Document::new(800.0, 600.0);
    let a = circle(&mut doc, (100.0, 100.0), 10.0);
    let b = circle(&mut doc, (300.0, 100.0), 10.0);
    let ia = a.id;
    let id = blend(&mut doc, a, b, 4);
    let before = get(&doc, id).clone();
    // a path of no length
    let mut dot = BezPath::new();
    dot.move_to((5.0, 5.0));
    dot.line_to((5.0, 5.0));
    let o = curve(&mut doc, &dot);
    let did = push(&mut doc, o);
    assert_eq!(bp::set_path(&mut doc, id, did), Err(BlendError::TooShort));
    // the blend itself, or one of its own parts
    assert_eq!(bp::set_path(&mut doc, id, id), Err(BlendError::OwnPart));
    assert_eq!(bp::set_path(&mut doc, id, ia), Err(BlendError::OwnPart));
    // a group is no path
    let g = Object::new(doc.new_id(), ObjKind::Group(vec![rect(&mut doc, (0.0, 0.0), 5.0, 5.0, Color::BLACK)]), Affine::IDENTITY, Style::default());
    let gid = push(&mut doc, g);
    assert_eq!(bp::set_path(&mut doc, id, gid), Err(BlendError::NotAPath));
    // a non-blend
    assert_eq!(bp::set_path(&mut doc, did, gid), Err(BlendError::NotABlend));
    assert_eq!(get(&doc, id), &before);
    assert!(doc.find(did).is_some() && doc.find(gid).is_some(), "a refused path left the page");
}

#[test]
fn moving_a_part_keeps_it_on_the_path() {
    let (mut doc, id, ia, _) = circle_blend(4);
    // drag the start somewhere off the circle: it slides to the nearest point of the circle
    doc.transform_objects(&[ia], Affine::translate((0.0, 150.0)));
    let b = get(&doc, id);
    let c = b.start.bounds().center();
    assert!((c.to_vec2().hypot() - 100.0).abs() < 0.5, "the start left the path: {c:?}");
    assert!(c.y > 50.0, "the start didn't move along the path: {c:?}");
    // moving the path takes the controls with it
    let pid = b.path.as_ref().unwrap().obj.id;
    doc.transform_objects(&[pid], Affine::translate((50.0, 0.0)));
    let b = get(&doc, id);
    for o in [&b.start, &b.end] {
        let c = o.bounds().center();
        assert!(((c - Point::new(50.0, 0.0)).hypot() - 100.0).abs() < 0.5, "a control stayed behind: {c:?}");
    }
    // moving the whole blend moves everything the same
    let before = get(&doc, id).clone();
    doc.transform_objects(&[id], Affine::translate((10.0, 20.0)));
    let b = get(&doc, id);
    assert!((b.start.bounds().center() - before.start.bounds().center() - kurbo::Vec2::new(10.0, 20.0)).hypot() < 1e-6);
    assert!((b.path.as_ref().unwrap().obj.bounds().center() - before.path.as_ref().unwrap().obj.bounds().center() - kurbo::Vec2::new(10.0, 20.0)).hypot() < 1e-6);
}

#[test]
fn rotate_all_turns_the_ends_and_back() {
    let (mut doc, id, _, _) = circle_blend(4);
    let before = get(&doc, id).clone();
    assert!(bp::set_rotate_all(&mut doc, id, true));
    let b = get(&doc, id);
    // the end (at 180°) turned by the direction of the circle there, the start too
    assert!((wrap180(turn_of(&b.end) - turn_of(&before.end)).abs() - 90.0).abs() < 1.0, "end turned {}", turn_of(&b.end) - turn_of(&before.end));
    assert!((wrap180(turn_of(&b.start) - turn_of(&before.start)).abs() - 90.0).abs() < 1.0);
    assert!(!bp::set_rotate_all(&mut doc, id, true));
    assert!(bp::set_rotate_all(&mut doc, id, false));
    let b = get(&doc, id);
    assert!(wrap180(turn_of(&b.end) - turn_of(&before.end)).abs() < 0.5);
    assert!((b.end.bounds().center() - before.end.bounds().center()).hypot() < 0.5);
    // turned blends stay consistent when the whole blend is rotated: the first step still
    // continues the start smoothly
    assert!(bp::set_rotate_all(&mut doc, id, true));
    doc.transform_objects(&[id], Affine::rotate(0.7));
    let b = get(&doc, id);
    let steps = blend_steps(b, || 0);
    let d = wrap180(turn_of(&steps[0]) - turn_of(&b.start));
    assert!(d.abs() < 40.0 && d.abs() > 1.0, "first step turned {d}° from the start");
}

#[test]
fn detach_puts_the_path_back_and_keeps_the_ends() {
    let (mut doc, id, _, _) = circle_blend(4);
    assert!(bp::set_spacing(&mut doc, id, Some(40.0)));
    let n = bp::effective_steps(get(&doc, id));
    let before = get(&doc, id).clone();
    let pid = bp::detach_path(&mut doc, id).unwrap();
    let b = get(&doc, id);
    assert!(b.path.is_none() && b.spacing.is_none());
    assert_eq!(b.steps, n, "fixed spacing became its number of steps");
    assert_eq!(b.start, before.start);
    // the path is on the page again, just above the blend
    let objs = &doc.pages[0].layers.last().unwrap().objects;
    assert_eq!(objs.iter().map(|o| o.id).collect::<Vec<_>>(), vec![id, pid]);
    assert!(bp::detach_path(&mut doc, id).is_none());
    // and can be attached again
    bp::set_path(&mut doc, id, pid).unwrap();
    assert!(get(&doc, id).path.is_some());
}

#[test]
fn fixed_spacing_places_steps_by_distance() {
    let mut doc = Document::new(800.0, 600.0);
    let a = circle(&mut doc, (0.0, 0.0), 5.0);
    let b = circle(&mut doc, (300.0, 0.0), 5.0);
    let id = blend(&mut doc, a, b, 3);
    let mut bez = BezPath::new();
    bez.move_to((0.0, 0.0));
    bez.line_to((300.0, 0.0));
    let o = curve(&mut doc, &bez);
    let pid = push(&mut doc, o);
    bp::set_path(&mut doc, id, pid).unwrap();
    assert!(bp::set_spacing(&mut doc, id, Some(40.0)));
    let cs = centres(get(&doc, id));
    // 40, 80, ... 280: seven steps, the last gap shorter
    assert_eq!(cs.len(), 7);
    for (k, c) in cs.iter().enumerate() {
        assert!((c.x - 40.0 * (k + 1) as f64).abs() < 0.5, "step {k} at {c:?}");
    }
    assert_eq!(bp::effective_steps(get(&doc, id)), 7);
    // back to a number of steps: as many as the spacing made
    assert!(bp::set_spacing(&mut doc, id, None));
    assert_eq!(get(&doc, id).steps, 7);
}

#[test]
fn split_then_fuse() {
    let mut doc = Document::new(800.0, 600.0);
    let a = circle(&mut doc, (0.0, 0.0), 10.0);
    let b = circle(&mut doc, (400.0, 0.0), 10.0);
    let id = blend(&mut doc, a, b, 7);
    let plain = centres(get(&doc, id));
    // split at the middle step (4 of 7)
    let at = plain[3];
    assert_eq!(bp::step_at(get(&doc, id), at, 1.0), Some(4));
    let sid = bp::split(&mut doc, id, 4).unwrap();
    assert_eq!(bp::part_of(&doc, sid), Some((id, BlendPart::Split(0))));
    // nothing moved yet: the split object is where the step was
    assert_eq!(centres(get(&doc, id)).len(), 7);
    for (p, q) in centres(get(&doc, id)).iter().zip(plain.iter()) {
        assert!((*p - *q).hypot() < 0.5);
    }
    // the split object can't be split again
    assert_eq!(bp::step_at(get(&doc, id), at, 1.0), None);
    assert!(bp::split(&mut doc, id, 4).is_none());
    // moving the split object bends the blend through it
    doc.transform_objects(&[sid], Affine::translate((0.0, 100.0)));
    let cs = centres(get(&doc, id));
    assert!((cs[3] - Point::new(200.0, 100.0)).hypot() < 0.5, "{:?}", cs[3]);
    assert!(cs[1].y > 30.0 && cs[5].y > 30.0, "the legs don't run through the split: {cs:?}");
    // a second split later on; Fuse Start joins the first, Fuse End the last
    let sid2 = bp::split(&mut doc, id, 6).unwrap();
    assert_eq!(get(&doc, id).splits.len(), 2);
    assert_eq!(get(&doc, id).splits[1].obj.id, sid2);
    assert!(bp::fuse(&mut doc, id, Fuse::First));
    assert_eq!(get(&doc, id).splits.iter().map(|s| s.obj.id).collect::<Vec<_>>(), vec![sid2]);
    assert!(bp::fuse(&mut doc, id, Fuse::Last));
    assert!(get(&doc, id).splits.is_empty());
    assert!(!bp::fuse(&mut doc, id, Fuse::Last));
    for (p, q) in centres(get(&doc, id)).iter().zip(plain.iter()) {
        assert!((*p - *q).hypot() < 0.5, "fused blend differs");
    }
}

#[test]
fn map_nodes_untwists_the_blend() {
    let mut doc = Document::new(800.0, 600.0);
    let a = rect(&mut doc, (0.0, 0.0), 100.0, 100.0, Color::BLACK);
    // the same square a quarter turn round: interpolated point by point it shrinks in the middle
    let mut b = rect(&mut doc, (400.0, 0.0), 100.0, 100.0, Color::WHITE);
    b.apply_transform(Affine::rotate_about(std::f64::consts::FRAC_PI_2, Point::new(400.0, 0.0)));
    let id = blend(&mut doc, a, b, 1);
    let area = |o: &Object| o.world_path().area().abs();
    let twisted = area(&blend_steps(get(&doc, id), || 0)[0]);
    // match the top-left corner of the start with the top-left corner of the end
    let bl = get(&doc, id).clone();
    let (sa, fa) = bp::map_node_at(&bl.start, Point::new(-50.0, -50.0), 1.0).unwrap();
    let (sb, fb) = bp::map_node_at(&bl.end, Point::new(350.0, -50.0), 1.0).unwrap();
    assert_eq!((sa, sb), (0, 0));
    assert!(bp::map_nodes(&mut doc, id, sa, fa, fb));
    let mapped = area(&blend_steps(get(&doc, id), || 0)[0]);
    assert!(twisted < 7000.0, "the unmapped blend should twist (area {twisted})");
    assert!((mapped - 10000.0).abs() < 100.0, "mapped step area {mapped}");
    // an open curve has no node to map
    let mut bez = BezPath::new();
    bez.move_to((0.0, 0.0));
    bez.line_to((10.0, 0.0));
    let line = curve(&mut doc, &bez);
    assert!(bp::map_node_at(&line, Point::new(0.0, 0.0), 1.0).is_none());
}

#[test]
fn new_start_and_end_replace_the_controls() {
    let (mut doc, id, ia, ib) = circle_blend(4);
    let n = rect(&mut doc, (0.0, 140.0), 30.0, 30.0, Color::rgb(0, 160, 0));
    let nid = push(&mut doc, n);
    assert_eq!(bp::replace_control(&mut doc, id, nid, false), Ok(ia));
    let b = get(&doc, id);
    assert_eq!(b.start.id, nid);
    // the new start went onto the circle, at its point nearest it
    let c = b.start.bounds().center();
    assert!((c - Point::new(0.0, 100.0)).hypot() < 1.0 && (c.to_vec2().hypot() - 100.0).abs() < 0.5, "{c:?}");
    // the old start is back on the page
    assert!(doc.locate(ia).is_some());
    // the old start can be the new end
    assert_eq!(bp::replace_control(&mut doc, id, ia, true), Ok(ib));
    assert_eq!(get(&doc, id).end.id, ia);
    assert_eq!(bp::replace_control(&mut doc, id, id, true), Err(BlendError::OwnPart));
    assert_eq!(bp::replace_control(&mut doc, id, nid, true), Err(BlendError::OwnPart));
}

#[test]
fn delete_a_part() {
    let (mut doc, id, ia, ib) = circle_blend(4);
    let pid = get(&doc, id).path.as_ref().unwrap().obj.id;
    // the path: the blend is straight again and the path is gone
    assert_eq!(bp::delete_part(&mut doc, id, BlendPart::Path), Some(vec![id]));
    assert!(get(&doc, id).path.is_none());
    assert!(doc.find(pid).is_none());
    // the start: the blend comes apart, the end stays
    assert_eq!(bp::delete_part(&mut doc, id, BlendPart::Start), Some(vec![ib]));
    assert!(doc.find(id).is_none() && doc.find(ia).is_none());
    assert!(doc.locate(ib).is_some());
}

#[test]
fn a_blend_on_a_path_picks_on_its_steps_not_its_bounds() {
    let (doc, id, _, _) = circle_blend(6);
    let o = doc.find(id).unwrap();
    // the bounds hold the whole circle (and half a step round it)
    let bb = doc.bounds_of(o);
    assert!(bb.x0 <= -100.0 && bb.x1 >= 100.0 && bb.y0 <= -100.0 && bb.y1 >= 100.0, "{bb:?}");
    // the middle of the circle is not the blend
    assert!(!doc.hit_object(o, Point::new(0.0, 0.0), 1.0, true));
    // a step is
    let c = centres(get(&doc, id))[2];
    assert!(doc.hit_object(o, c, 1.0, true));
    assert_eq!(doc.hit_test(0, c, 1.0, true), Some(id));
}

#[test]
fn file_format_keeps_the_path_and_reads_old_blends() {
    let (mut doc, id, _, _) = circle_blend(4);
    bp::set_full_path(&mut doc, id, true);
    bp::set_rotate_all(&mut doc, id, true);
    bp::split(&mut doc, id, 2).unwrap();
    let b = get(&doc, id).clone();
    // the new fields round-trip
    let json = serde_json::to_string(&b).unwrap();
    let back: BlendObj = serde_json::from_str(&json).unwrap();
    // (JSON numbers may lose the last bit, so the geometry is compared to within a hair)
    assert_eq!((back.steps, back.splits.len(), back.spacing, back.map), (b.steps, 1, b.spacing, b.map));
    assert_eq!(back.splits[0].obj.id, b.splits[0].obj.id);
    assert!((back.splits[0].at - b.splits[0].at).abs() < 1e-12);
    for (x, y) in blend_steps(&back, || 0).iter().zip(blend_steps(&b, || 0).iter()) {
        assert!((x.bounds().center() - y.bounds().center()).hypot() < 1e-6);
    }
    let p = back.path.unwrap();
    assert!(p.full && p.rotate);
    // a 0.6.0 blend (no path, spacing, splits or map) still loads, as a plain blend
    let mut v: serde_json::Value = serde_json::to_value(&b).unwrap();
    for k in ["path", "spacing", "splits", "map"] {
        v.as_object_mut().unwrap().remove(k);
    }
    let old: BlendObj = serde_json::from_value(v).unwrap();
    assert!(old.path.is_none() && old.spacing.is_none() && old.splits.is_empty() && old.map.is_none());
    assert_eq!(old.steps, 4);
    // the whole document too
    let text = serde_json::to_string(&doc).unwrap();
    let again: Document = serde_json::from_str(&text).unwrap();
    let g = bp::blend_of(&again, id).unwrap();
    let gp = g.path.as_ref().unwrap();
    assert!(gp.full && gp.rotate && g.splits.len() == 1);
    assert_eq!(gp.obj.id, b.path.as_ref().unwrap().obj.id);
    assert!((g.start.bounds().center() - b.start.bounds().center()).hypot() < 1e-9);
}
