//! Selecting inside a container for 0.6.2 (#151, #149): the object of a group or the control of a
//! blend under a point (one level, then the whole chain), ancestors, the selection kept free of
//! an object and its group together, and the edits that work on an object inside a group
//! (move, delete, duplicate, group and ungroup).

use kurbo::{Affine, BezPath, Point, Vec2};
use vector_core::blend_path as bp;
use vector_core::object::{BlendPart, CornerType, EllipseKind, EllipseShape, RectShape};
use vector_core::{ops, Color, Document, Fill, ObjKind, Object, Style};

fn solid() -> Style {
    Style { fill: Fill::Solid(Color::rgb(200, 40, 40)), ..Style::default() }
}

fn rect(doc: &mut Document, at: (f64, f64), w: f64, h: f64) -> Object {
    Object::new(doc.new_id(), ObjKind::Rect(RectShape { w, h, radii: [0.0; 4], corner: CornerType::default() }), Affine::translate(at), solid())
}

fn circle(doc: &mut Document, c: (f64, f64), r: f64) -> Object {
    let e = EllipseShape { w: 2.0 * r, h: 2.0 * r, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false };
    Object::new(doc.new_id(), ObjKind::Ellipse(e), Affine::translate((c.0 - r, c.1 - r)), solid())
}

fn group(doc: &mut Document, children: Vec<Object>) -> Object {
    Object::new(doc.new_id(), ObjKind::Group(children), Affine::IDENTITY, Style::default())
}

fn push(doc: &mut Document, o: Object) -> u64 {
    let id = o.id;
    doc.pages[0].layers.last_mut().unwrap().objects.push(o);
    id
}

fn x0(doc: &Document, id: u64) -> f64 {
    doc.find(id).unwrap().bounds().x0
}

/// Two levels of groups: `outer` holds a rectangle `a` (100,100)–(200,200) and the group `inner`,
/// which holds a rectangle `b` (300,100)–(400,200) and, over it, a rectangle `c` (350,150)–(450,250).
/// Gives (doc, outer, inner, a, b, c).
fn nested() -> (Document, u64, u64, u64, u64, u64) {
    let mut doc = Document::new(800.0, 600.0);
    let a = rect(&mut doc, (100.0, 100.0), 100.0, 100.0);
    let b = rect(&mut doc, (300.0, 100.0), 100.0, 100.0);
    let c = rect(&mut doc, (350.0, 150.0), 100.0, 100.0);
    let (ia, ib, ic) = (a.id, b.id, c.id);
    let inner = group(&mut doc, vec![b, c]);
    let ii = inner.id;
    let outer = group(&mut doc, vec![a, inner]);
    let io = push(&mut doc, outer);
    (doc, io, ii, ia, ib, ic)
}

#[test]
fn child_at_picks_one_level_down() {
    let (mut doc, outer, inner, a, b, c) = nested();
    let o = doc.find(outer).unwrap();
    assert_eq!(doc.child_at(o, Point::new(150.0, 150.0), 2.0, false), Some(a));
    // the inner group, not its rectangle: one level only
    assert_eq!(doc.child_at(o, Point::new(320.0, 120.0), 2.0, false), Some(inner));
    // inside the inner group, the topmost object under the point
    let i = doc.find(inner).unwrap();
    assert_eq!(doc.child_at(i, Point::new(380.0, 180.0), 2.0, false), Some(c), "the rectangle on top wins");
    assert_eq!(doc.child_at(i, Point::new(320.0, 120.0), 2.0, false), Some(b));
    // nothing there, and not a container
    assert_eq!(doc.child_at(o, Point::new(600.0, 500.0), 2.0, false), None);
    assert_eq!(doc.child_at(doc.find(a).unwrap(), Point::new(150.0, 150.0), 2.0, false), None);
    // a hidden object is skipped: the one under it is picked
    doc.find_mut(c).unwrap().visible = false;
    assert_eq!(doc.child_at(doc.find(inner).unwrap(), Point::new(380.0, 180.0), 2.0, false), Some(b));
    doc.find_mut(c).unwrap().visible = true;
    // a locked object is picked (it can be seen selected, as on the page; it just doesn't move)
    doc.find_mut(c).unwrap().locked = true;
    assert_eq!(doc.child_at(doc.find(inner).unwrap(), Point::new(380.0, 180.0), 2.0, false), Some(c));
    // but nothing inside a locked group is: it is locked with the group
    doc.find_mut(inner).unwrap().locked = true;
    assert_eq!(doc.child_at(doc.find(inner).unwrap(), Point::new(320.0, 120.0), 2.0, false), None);
    assert_eq!(doc.locked_within(&[a, b, c, inner]).len(), 2, "b and c are locked with their group");
    assert!(doc.locked_within(&[a, b]).contains(&b));
}

#[test]
fn the_chain_under_a_point_goes_down_to_the_object() {
    let (doc, outer, inner, a, b, c) = nested();
    assert_eq!(doc.hit_chain(0, Point::new(150.0, 150.0), 2.0, false), vec![outer, a]);
    assert_eq!(doc.hit_chain(0, Point::new(320.0, 120.0), 2.0, false), vec![outer, inner, b]);
    assert_eq!(doc.hit_chain(0, Point::new(420.0, 230.0), 2.0, false), vec![outer, inner, c]);
    // the page still picks the outer group, as a plain click does
    assert_eq!(doc.hit_test(0, Point::new(420.0, 230.0), 2.0, false), Some(outer));
    assert!(doc.hit_chain(0, Point::new(700.0, 500.0), 2.0, false).is_empty());
    // ancestors
    assert!(doc.is_ancestor(outer, c) && doc.is_ancestor(inner, c) && doc.is_ancestor(outer, inner));
    assert!(!doc.is_ancestor(c, outer) && !doc.is_ancestor(inner, a) && !doc.is_ancestor(a, a));
}

#[test]
fn a_turned_group_is_picked_where_its_objects_are_drawn() {
    let (mut doc, outer, inner, _, b, _) = nested();
    // a quarter turn about (300, 300): b (300,100)–(400,200) goes to (400,300)–(500,400), and the
    // corner of b that c doesn't cover, (300..350, 100..150), to its upper right
    doc.transform_objects(&[outer], Affine::rotate_about(std::f64::consts::FRAC_PI_2, Point::new(300.0, 300.0)));
    let bb = doc.find(b).unwrap().bounds();
    assert!((bb.x0 - 400.0).abs() < 1e-6 && (bb.y0 - 300.0).abs() < 1e-6, "{bb:?}");
    let at = Point::new(bb.x1 - 10.0, bb.y0 + 10.0);
    let chain = doc.hit_chain(0, at, 2.0, false);
    assert_eq!(chain.first(), Some(&outer));
    assert!(chain.contains(&inner), "{chain:?}");
    assert_eq!(chain.last(), Some(&b), "{chain:?} at {at:?}");
}

#[test]
fn an_object_and_its_group_never_move_twice() {
    let (mut doc, outer, inner, a, b, c) = nested();
    assert_eq!(doc.outermost(&[b, outer, c]), vec![outer]);
    assert_eq!(doc.outermost(&[b, a]), vec![b, a]);
    assert_eq!(doc.outermost(&[c, inner]), vec![inner]);
    // moved with its group in one go: once
    let (bx, ax) = (x0(&doc, b), x0(&doc, a));
    doc.transform_objects(&[b, outer], Affine::translate((10.0, 0.0)));
    assert!((x0(&doc, b) - bx - 10.0).abs() < 1e-9, "b moved {}", x0(&doc, b) - bx);
    assert!((x0(&doc, a) - ax - 10.0).abs() < 1e-9);
    // an object of a group on its own: only it moves
    let (bx, cx) = (x0(&doc, b), x0(&doc, c));
    doc.transform_objects(&[b], Affine::translate((40.0, 0.0)));
    assert!((x0(&doc, b) - bx - 40.0).abs() < 1e-9);
    assert_eq!(x0(&doc, c), cx, "its neighbour moved");
    assert_eq!(doc.parent_of(b), Some(inner));
}

#[test]
fn deleting_the_last_object_of_a_group_deletes_the_group() {
    let (mut doc, outer, inner, a, b, c) = nested();
    assert!(doc.remove_pruning(b).is_some());
    assert!(doc.find(inner).is_some(), "a group with one object left stays a group");
    doc.remove_pruning(c);
    assert!(doc.find(inner).is_none(), "the empty inner group stayed");
    assert!(doc.find(outer).is_some() && doc.find(a).is_some());
    doc.remove_pruning(a);
    assert!(doc.find(outer).is_none(), "the empty outer group stayed");
    assert!(doc.pages[0].layers.iter().all(|l| l.objects.is_empty()));
}

#[test]
fn duplicate_group_and_ungroup_inside_a_group() {
    let (mut doc, outer, inner, a, b, c) = nested();
    // a copy of an object of a group goes just above it, in the group
    let copies = ops::duplicate(&mut doc, &[b], Vec2::new(5.0, 0.0));
    assert_eq!(copies.len(), 1);
    let copy = copies[0];
    assert_eq!(doc.parent_of(copy), Some(inner));
    let ObjKind::Group(kids) = &doc.find(inner).unwrap().kind else { panic!() };
    assert_eq!(kids.iter().map(|o| o.id).collect::<Vec<_>>(), vec![b, copy, c]);
    assert!((x0(&doc, copy) - x0(&doc, b) - 5.0).abs() < 1e-9);
    // a group and an object of it: one copy, of the group
    assert_eq!(ops::duplicate(&mut doc, &[outer, a], Vec2::ZERO).len(), 1);
    doc.remove(*doc.pages[0].layers.last().unwrap().objects.last().map(|o| &o.id).unwrap());
    // grouping two objects of the same group makes a group inside it, where the upper one was
    let g = ops::group(&mut doc, &[c, b]).expect("grouped inside the group");
    assert_eq!(doc.parent_of(g), Some(inner));
    let ObjKind::Group(kids) = &doc.find(inner).unwrap().kind else { panic!() };
    assert_eq!(kids.iter().map(|o| o.id).collect::<Vec<_>>(), vec![copy, g]);
    let ObjKind::Group(new) = &doc.find(g).unwrap().kind else { panic!() };
    assert_eq!(new.iter().map(|o| o.id).collect::<Vec<_>>(), vec![b, c], "the stacking order changed");
    // objects of different groups aren't grouped
    assert_eq!(ops::group(&mut doc, &[a, b]), None);
    // ungrouping it puts them back in its place
    let out = ops::ungroup(&mut doc, &[g], false);
    assert_eq!(out, vec![b, c]);
    let ObjKind::Group(kids) = &doc.find(inner).unwrap().kind else { panic!() };
    assert_eq!(kids.iter().map(|o| o.id).collect::<Vec<_>>(), vec![copy, b, c]);
    // a group inside a group, ungrouped: its objects go into the outer group
    assert_eq!(ops::ungroup(&mut doc, &[inner], false), vec![copy, b, c]);
    assert_eq!(doc.parent_of(b), Some(outer));
    assert!(doc.find(inner).is_none());
}

/// A blend of a circle at (100, 300) (radius 30) to one at (500, 300) (radius 40) in 4 steps, and
/// the circle about (300, 300) of radius 200 it may follow. Gives (doc, blend, start, end, path).
fn blend_doc() -> (Document, u64, u64, u64, u64) {
    let mut doc = Document::new(800.0, 600.0);
    let s = circle(&mut doc, (100.0, 300.0), 30.0);
    let e = circle(&mut doc, (500.0, 300.0), 40.0);
    let mut path = circle(&mut doc, (300.0, 300.0), 200.0);
    path.style.fill = Fill::None;
    let (is, ie, ip) = (push(&mut doc, s), push(&mut doc, e), push(&mut doc, path));
    let blend = ops::create_blend(&mut doc, is, ie, 4).unwrap();
    (doc, blend, is, ie, ip)
}

#[test]
fn the_control_of_a_blend_under_a_point() {
    let (mut doc, blend, start, end, path) = blend_doc();
    let o = doc.find(blend).unwrap();
    // the start and the end are picked; a step between them is the blend (no control)
    assert_eq!(doc.child_at(o, Point::new(100.0, 300.0), 2.0, false), Some(start));
    assert_eq!(doc.child_at(o, Point::new(500.0, 300.0), 2.0, false), Some(end));
    assert_eq!(doc.child_at(o, Point::new(300.0, 300.0), 2.0, false), None, "a step is not a control");
    assert_eq!(doc.hit_chain(0, Point::new(300.0, 300.0), 2.0, false), vec![blend]);
    assert_eq!(doc.hit_chain(0, Point::new(500.0, 300.0), 2.0, false), vec![blend, end]);
    // on the path
    doc.find_mut(blend).unwrap();
    bp::set_path(&mut doc, blend, path).unwrap();
    let o = doc.find(blend).unwrap();
    let b = bp::blend_of(&doc, blend).unwrap();
    let p = b.path.as_ref().unwrap().obj.world_path();
    let top = kurbo::Shape::bounding_box(&p);
    let on = Point::new(top.center().x, top.y0);
    assert_eq!(doc.child_at(o, on, 2.0, true), Some(path), "the path on its outline");
    // inside the closed path, off the steps, is not on the path, even treated as filled
    let s = b.start.bounds().center();
    assert_eq!(doc.child_at(o, s, 2.0, true), Some(start), "the start sits on the path");
    assert_eq!(doc.child_at(o, Point::new(300.0, 420.0), 2.0, true), None);
    // a locked blend gives nothing inside it
    doc.find_mut(blend).unwrap().locked = true;
    assert_eq!(doc.child_at(doc.find(blend).unwrap(), s, 2.0, false), None);
}

#[test]
fn overlapping_controls_the_one_on_top_wins() {
    let (doc, _, start, end, _) = blend_doc();
    let b = bp::blend_of(&doc, doc.parent_of(start).unwrap()).unwrap();
    // both "under the pointer": the end is drawn last, on top
    assert_eq!(bp::control_at(b, |_, _| true), Some(BlendPart::End));
    // only the start
    assert_eq!(bp::control_at(b, |o, _| o.id == start), Some(BlendPart::Start));
    assert_eq!(bp::control_at(b, |o, _| o.id == end), Some(BlendPart::End));
    // hidden: left out
    let mut b2 = b.clone();
    b2.end.visible = false;
    assert_eq!(bp::control_at(&b2, |_, _| true), Some(BlendPart::Start));
    assert_eq!(bp::control_at(b, |_, _| false), None);
}

#[test]
fn moving_a_control_of_a_blend_and_duplicating_it() {
    let (mut doc, blend, start, _, _) = blend_doc();
    let before: Vec<Point> = vector_core::object::blend_steps(bp::blend_of(&doc, blend).unwrap(), || 0).iter().map(|o| o.bounds().center()).collect();
    doc.transform_objects(&[start], Affine::translate((0.0, -100.0)));
    let b = bp::blend_of(&doc, blend).unwrap();
    assert!((b.start.bounds().center() - Point::new(100.0, 200.0)).hypot() < 1e-6);
    let after: Vec<Point> = vector_core::object::blend_steps(b, || 0).iter().map(|o| o.bounds().center()).collect();
    assert!(before.iter().zip(&after).all(|(p, q)| q.y < p.y - 1.0), "the steps didn't follow: {after:?}");
    // a copy of a control is an object on the page, above the blend
    let copy = ops::duplicate(&mut doc, &[start], Vec2::ZERO);
    assert_eq!(copy.len(), 1);
    let (_, _, i) = doc.locate(copy[0]).expect("the copy is on the page");
    assert_eq!(doc.locate(blend).unwrap().2 + 1, i);
}

#[test]
fn list_paths_and_inserting_above() {
    let (mut doc, outer, inner, a, b, _) = nested();
    assert_eq!(doc.list_path(outer), Some((0, doc.pages[0].layers.len() - 1, vec![0])));
    assert_eq!(doc.list_path(b).map(|x| x.2), Some(vec![0, 1, 0]));
    let n = rect(&mut doc, (0.0, 0.0), 1.0, 1.0);
    let nid = n.id;
    assert!(doc.insert_above(a, n).is_ok());
    assert_eq!(doc.parent_of(nid), Some(outer));
    assert_eq!(doc.list_path(nid).map(|x| x.2), Some(vec![0, 1]));
    assert_eq!(doc.list_path(inner).map(|x| x.2), Some(vec![0, 2]));
    let mut bz = BezPath::new();
    bz.move_to((0.0, 0.0));
    bz.line_to((1.0, 1.0));
    let stray = Object::new(doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(&bz)), Affine::IDENTITY, Style::default());
    assert!(doc.insert_above(9999, stray).is_err());
}
