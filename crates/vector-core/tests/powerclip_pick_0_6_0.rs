//! PowerClip picking and extraction for 0.6.0 (#111, #112): the contents under a point (Alt+click
//! and the contents being edited), the marquee over them, and Extract Contents of a PowerClip that
//! sits inside another one.

use kurbo::{Affine, Point, Rect};
use vector_core::object::{CornerType, EllipseKind, EllipseShape, RectShape};
use vector_core::{Color, Document, Fill, ObjKind, Object, Style};

fn rect(doc: &mut Document, at: (f64, f64), w: f64, h: f64) -> Object {
    let st = Style { fill: Fill::Solid(Color::rgb(200, 40, 40)), ..Style::default() };
    Object::new(doc.new_id(), ObjKind::Rect(RectShape { w, h, radii: [0.0; 4], corner: CornerType::default() }), Affine::translate(at), st)
}

fn ellipse(doc: &mut Document, at: (f64, f64), w: f64, h: f64) -> Object {
    let e = EllipseShape { w, h, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false };
    Object::new(doc.new_id(), ObjKind::Ellipse(e), Affine::translate(at), Style::default())
}

fn powerclip(doc: &mut Document, frame: Object, contents: Vec<Object>) -> Object {
    Object::new(doc.new_id(), ObjKind::PowerClip { frame: Box::new(frame), contents }, Affine::IDENTITY, Style::default())
}

fn push(doc: &mut Document, o: Object) -> u64 {
    let id = o.id;
    doc.pages[0].layers.last_mut().unwrap().objects.push(o);
    id
}

/// A PowerClip with an ellipse frame (100,100)–(400,300) holding two rectangles, a left one and
/// a right one that runs past the frame.
fn sample() -> (Document, u64, u64, u64) {
    let mut doc = Document::new(800.0, 600.0);
    let frame = ellipse(&mut doc, (100.0, 100.0), 300.0, 200.0);
    let a = rect(&mut doc, (150.0, 150.0), 80.0, 100.0);
    let b = rect(&mut doc, (260.0, 170.0), 300.0, 60.0);
    let (ia, ib) = (a.id, b.id);
    let pc = powerclip(&mut doc, frame, vec![a, b]);
    let id = push(&mut doc, pc);
    (doc, id, ia, ib)
}

#[test]
fn contents_under_a_point_inside_the_frame() {
    let (doc, pc, a, b) = sample();
    let o = doc.find(pc).unwrap();
    // the page picks the PowerClip itself
    assert_eq!(doc.hit_test(0, Point::new(190.0, 200.0), 2.0, false), Some(pc));
    // Alt+click: the content under the point
    assert_eq!(doc.powerclip_content_at(o, Point::new(190.0, 200.0), 2.0, false), Some(a));
    assert_eq!(doc.powerclip_content_at(o, Point::new(300.0, 200.0), 2.0, false), Some(b));
    // inside the frame but on no content
    assert_eq!(doc.powerclip_content_at(o, Point::new(250.0, 120.0), 2.0, false), None);
    // the right rectangle runs past the frame: there it is clipped away, so Alt+click finds nothing
    assert_eq!(doc.powerclip_content_at(o, Point::new(500.0, 200.0), 2.0, false), None);
    // but the contents being edited show in full and pick there too
    assert_eq!(doc.powerclip_child_at(o, Point::new(500.0, 200.0), 2.0, false), Some(b));
    // not a PowerClip: nothing to dig into
    let plain = doc.find(a).unwrap();
    assert_eq!(doc.powerclip_content_at(plain, Point::new(190.0, 200.0), 2.0, false), None);
}

#[test]
fn hidden_contents_are_not_picked() {
    let (mut doc, pc, a, _) = sample();
    doc.find_mut(a).unwrap().visible = false;
    let o = doc.find(pc).unwrap();
    assert_eq!(doc.powerclip_content_at(o, Point::new(190.0, 200.0), 2.0, false), None);
    assert_eq!(doc.powerclip_child_at(o, Point::new(190.0, 200.0), 2.0, false), None);
}

/// Alt+click digs through a PowerClip nested in the contents to its own contents; the contents
/// being edited count a nested PowerClip as one object.
#[test]
fn nested_powerclips() {
    let mut doc = Document::new(800.0, 600.0);
    let inner_frame = rect(&mut doc, (150.0, 150.0), 200.0, 100.0);
    let deep = rect(&mut doc, (160.0, 160.0), 60.0, 60.0);
    let deep_id = deep.id;
    let inner = powerclip(&mut doc, inner_frame, vec![deep]);
    let inner_id = inner.id;
    let frame = rect(&mut doc, (100.0, 100.0), 400.0, 300.0);
    let outer = powerclip(&mut doc, frame, vec![inner]);
    let outer_id = push(&mut doc, outer);
    let o = doc.find(outer_id).unwrap();
    assert_eq!(doc.powerclip_content_at(o, Point::new(180.0, 180.0), 2.0, false), Some(deep_id), "Alt+click didn't reach the deepest content");
    // inside the inner frame but off its contents: the inner PowerClip itself
    assert_eq!(doc.powerclip_content_at(o, Point::new(300.0, 200.0), 2.0, false), Some(inner_id));
    assert_eq!(doc.powerclip_child_at(o, Point::new(180.0, 180.0), 2.0, false), Some(inner_id), "editing picks one level at a time");
}

#[test]
fn marquee_over_the_contents() {
    let (doc, pc, a, b) = sample();
    let o = doc.find(pc).unwrap();
    assert_eq!(doc.powerclip_contents_in_rect(o, Rect::new(140.0, 140.0, 240.0, 260.0), false), vec![a]);
    assert_eq!(doc.powerclip_contents_in_rect(o, Rect::new(140.0, 140.0, 600.0, 260.0), false), vec![a, b]);
    // touching mode (Alt)
    assert_eq!(doc.powerclip_contents_in_rect(o, Rect::new(500.0, 160.0, 520.0, 180.0), true), vec![b]);
    assert!(doc.powerclip_contents_in_rect(o, Rect::new(0.0, 0.0, 50.0, 50.0), true).is_empty());
}

#[test]
fn extract_gives_the_contents_and_keeps_the_frame() {
    let (mut doc, pc, a, b) = sample();
    let frame_id = match &doc.find(pc).unwrap().kind {
        ObjKind::PowerClip { frame, .. } => frame.id,
        _ => unreachable!(),
    };
    let out = vector_core::ops::extract_powerclip(&mut doc, pc);
    assert_eq!(out, vec![a, b], "Extract gives the contents (they are what gets selected)");
    let ids: Vec<u64> = doc.pages[0].layers.last().unwrap().objects.iter().map(|o| o.id).collect();
    assert_eq!(ids, vec![frame_id, a, b], "the frame stays where the PowerClip was, the contents in front of it");
    assert!(doc.find(pc).is_none());
    assert!(matches!(doc.find(frame_id).unwrap().kind, ObjKind::Ellipse(_)));
}

/// A PowerClip inside the contents of another one (as reached while editing the outer one)
/// extracts in place: its contents stay inside the outer PowerClip.
#[test]
fn extract_a_nested_powerclip() {
    let mut doc = Document::new(800.0, 600.0);
    let inner_frame = rect(&mut doc, (150.0, 150.0), 200.0, 100.0);
    let inner_frame_id = inner_frame.id;
    let deep = rect(&mut doc, (160.0, 160.0), 60.0, 60.0);
    let deep_id = deep.id;
    let inner = powerclip(&mut doc, inner_frame, vec![deep]);
    let inner_id = inner.id;
    let other = rect(&mut doc, (300.0, 300.0), 50.0, 50.0);
    let other_id = other.id;
    let frame = rect(&mut doc, (100.0, 100.0), 400.0, 300.0);
    let outer = powerclip(&mut doc, frame, vec![inner, other]);
    let outer_id = push(&mut doc, outer);
    assert_eq!(vector_core::ops::extract_powerclip(&mut doc, inner_id), vec![deep_id]);
    let ObjKind::PowerClip { contents, .. } = &doc.find(outer_id).unwrap().kind else { panic!("the outer PowerClip is gone") };
    let ids: Vec<u64> = contents.iter().map(|o| o.id).collect();
    assert_eq!(ids, vec![inner_frame_id, deep_id, other_id]);
    assert_eq!(doc.parent_of(deep_id), Some(outer_id));
    // not a PowerClip: nothing happens
    assert!(vector_core::ops::extract_powerclip(&mut doc, other_id).is_empty());
    assert_eq!(doc.pages[0].layers.last().unwrap().objects.len(), 1);
}
