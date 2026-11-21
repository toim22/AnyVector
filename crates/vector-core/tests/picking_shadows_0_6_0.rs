//! Regression tests for 0.6.0: Alt+click through a stack of objects (#139), clicking an object's
//! shadow (#106) and breaking a shadow off its object (#105).

use kurbo::{Affine, BezPath, Point, Rect, Shape, Vec2};
use vector_core::document::Layer;
use vector_core::object::{BlockShadow, CornerType, DropShadow, ObjKind, Object, RectShape, ShadowPart};
use vector_core::ops::{self, ShadowRaster};
use vector_core::path::NodePath;
use vector_core::style::{MergeMode, Outline};
use vector_core::text::{CharFormat, TextObj};
use vector_core::{Color, Document, Fill, Style};

fn filled() -> Style {
    Style { fill: Fill::Solid(Color::rgb(200, 40, 40)), outline: Outline::none(), ..Style::default() }
}

fn square(doc: &mut Document, at: (f64, f64), w: f64, style: Style) -> u64 {
    let id = doc.new_id();
    let r = RectShape { w, h: w, radii: [0.0; 4], corner: CornerType::default() };
    doc.pages[0].layers.last_mut().unwrap().objects.push(Object::new(id, ObjKind::Rect(r), Affine::translate(at), style));
    id
}

fn obj<'a>(doc: &'a mut Document, id: u64) -> &'a mut Object {
    doc.find_mut(id).unwrap()
}

fn index_of(doc: &Document, id: u64) -> usize {
    doc.locate(id).unwrap().2
}

// ---- #139: the objects at a point, topmost first ----

#[test]
fn hit_list_is_every_object_at_the_point_topmost_first() {
    let mut doc = Document::new(400.0, 400.0);
    let a = square(&mut doc, (100.0, 100.0), 100.0, filled());
    let b = square(&mut doc, (120.0, 120.0), 100.0, filled());
    let c = square(&mut doc, (140.0, 140.0), 100.0, filled());
    let _away = square(&mut doc, (300.0, 300.0), 50.0, filled());
    let p = Point::new(160.0, 160.0);
    assert_eq!(doc.hit_list(0, p, 1.0, true), vec![c, b, a]);
    assert_eq!(doc.hit_test(0, p, 1.0, true), Some(c), "hit_test is the first of them");
    assert_eq!(doc.hit_list(0, Point::new(110.0, 110.0), 1.0, true), vec![a]);
    assert!(doc.hit_list(0, Point::new(20.0, 20.0), 1.0, true).is_empty(), "empty canvas");
}

#[test]
fn hit_list_follows_treat_as_filled() {
    let mut doc = Document::new(400.0, 400.0);
    let a = square(&mut doc, (100.0, 100.0), 100.0, Style::default());
    let b = square(&mut doc, (100.0, 100.0), 60.0, Style::default());
    // inside both unfilled squares: only with Treat All Objects as Filled
    assert_eq!(doc.hit_list(0, Point::new(130.0, 130.0), 1.0, true), vec![b, a]);
    assert!(doc.hit_list(0, Point::new(130.0, 130.0), 1.0, false).is_empty());
    // on the outline they share, both are under the pointer either way
    assert_eq!(doc.hit_list(0, Point::new(100.0, 130.0), 1.0, false), vec![b, a]);
}

#[test]
fn hit_list_leaves_out_hidden_objects_and_hidden_or_locked_layers() {
    let mut doc = Document::new(400.0, 400.0);
    let a = square(&mut doc, (100.0, 100.0), 100.0, filled());
    let hidden = square(&mut doc, (100.0, 100.0), 100.0, filled());
    obj(&mut doc, hidden).visible = false;
    let lid = doc.new_id();
    doc.pages[0].layers.push(Layer::new(lid, "Locked"));
    let on_locked = square(&mut doc, (100.0, 100.0), 100.0, filled());
    doc.pages[0].layers.last_mut().unwrap().locked = true;
    let lid = doc.new_id();
    doc.pages[0].layers.push(Layer::new(lid, "Hidden"));
    let on_hidden = square(&mut doc, (100.0, 100.0), 100.0, filled());
    doc.pages[0].layers.last_mut().unwrap().visible = false;
    let list = doc.hit_list(0, Point::new(150.0, 150.0), 1.0, true);
    assert_eq!(list, vec![a]);
    assert!(!list.contains(&hidden) && !list.contains(&on_locked) && !list.contains(&on_hidden));
}

// ---- #106: the shadow is part of the object under the pointer ----

#[test]
fn a_block_shadow_is_picked_where_it_is_drawn() {
    let mut doc = Document::new(400.0, 400.0);
    let id = square(&mut doc, (100.0, 100.0), 100.0, filled());
    obj(&mut doc, id).effects.block_shadow = Some(BlockShadow { offset: Vec2::new(30.0, 30.0), ..Default::default() });
    let o = doc.find(id).unwrap().clone();
    // the far corner of the shadow, and a point of the band it sweeps
    for p in [Point::new(225.0, 225.0), Point::new(205.0, 115.0)] {
        assert_eq!(doc.hit_test(0, p, 1.0, true), Some(id), "{p:?}");
        assert!(!doc.hit_body(&o, p, 1.0, true));
        assert_eq!(doc.shadow_at(&o, p, 1.0, true), Some(ShadowPart::Block));
    }
    // inside its bounding box, but not on the swept shape
    assert_eq!(doc.hit_test(0, Point::new(225.0, 105.0), 1.0, true), None);
    assert_eq!(doc.hit_test(0, Point::new(105.0, 225.0), 1.0, true), None);
    // the object itself is the object, not its shadow
    assert!(doc.hit_body(&o, Point::new(150.0, 150.0), 1.0, true));
}

#[test]
fn a_block_shadow_follows_its_options() {
    let mut doc = Document::new(400.0, 400.0);
    // a square with a square hole
    let mut bp = Rect::new(100.0, 100.0, 200.0, 200.0).to_path(0.1);
    let mut hole = BezPath::new();
    hole.move_to((130.0, 130.0));
    hole.line_to((130.0, 170.0));
    hole.line_to((170.0, 170.0));
    hole.line_to((170.0, 130.0));
    hole.close_path();
    bp.extend(hole.elements().iter().copied());
    let id = doc.new_id();
    let mut o = Object::new(id, ObjKind::Path(NodePath::from_bez(&bp)), Affine::IDENTITY, filled());
    let mut b = BlockShadow { offset: Vec2::new(10.0, 0.0), ..Default::default() };
    o.effects.block_shadow = Some(b.clone());
    let mid = Point::new(150.0, 150.0);
    assert!(!b.hit(&o, mid, 0.0), "the hole shows through");
    b.remove_holes = true;
    assert!(b.hit(&o, mid, 0.0), "Remove Holes fills it");
    // Expand and the outline (Generate from Object Outline) grow it
    let mut b = BlockShadow { offset: Vec2::new(30.0, 30.0), expand: 5.0, ..Default::default() };
    let sq = {
        let id = doc.new_id();
        let r = RectShape { w: 100.0, h: 100.0, radii: [0.0; 4], corner: CornerType::default() };
        let mut st = filled();
        st.outline = Outline { width: 4.0, color: Some(Color::BLACK), ..Outline::default() };
        Object::new(id, ObjKind::Rect(r), Affine::translate((100.0, 100.0)), st)
    };
    let bb = b.swept_path(&sq).bounding_box();
    assert!((bb.x0 - 95.0).abs() < 0.5 && (bb.y0 - 95.0).abs() < 0.5 && (bb.x1 - 235.0).abs() < 0.5 && (bb.y1 - 235.0).abs() < 0.5, "{bb:?}");
    b.from_outline = true;
    let bb = b.swept_path(&sq).bounding_box();
    assert!((bb.x0 - 93.0).abs() < 0.5 && (bb.x1 - 237.0).abs() < 0.5, "{bb:?}");
}

#[test]
fn a_block_shadow_is_the_square_swept_along_the_offset() {
    let mut doc = Document::new(400.0, 400.0);
    let id = square(&mut doc, (100.0, 100.0), 100.0, filled());
    let b = BlockShadow { offset: Vec2::new(30.0, 30.0), ..Default::default() };
    let o = doc.find(id).unwrap().clone();
    let swept = b.swept_path(&o);
    // a hexagon: the square plus a 100 x 30 band on two sides
    let area = swept.area().abs();
    assert!((area - 16000.0).abs() < 160.0, "{area}");
    let bb = swept.bounding_box();
    assert!((bb.x0 - 100.0).abs() < 0.5 && (bb.x1 - 230.0).abs() < 0.5 && (bb.y1 - 230.0).abs() < 0.5, "{bb:?}");
    assert!(!vector_core::geom::contains(&swept, Point::new(225.0, 105.0), false));
    assert!(vector_core::geom::contains(&swept, Point::new(225.0, 225.0), false));
}

#[test]
fn a_drop_shadow_is_picked_with_its_feathering() {
    let mut doc = Document::new(400.0, 400.0);
    let id = square(&mut doc, (100.0, 100.0), 100.0, filled());
    obj(&mut doc, id).effects.drop_shadow = Some(DropShadow { offset: Vec2::new(40.0, 40.0), feather: 8.0, ..Default::default() });
    let o = doc.find(id).unwrap().clone();
    for p in [Point::new(230.0, 230.0), Point::new(243.0, 200.0)] {
        assert_eq!(doc.hit_test(0, p, 1.0, true), Some(id), "{p:?}");
        assert_eq!(doc.shadow_at(&o, p, 1.0, true), Some(ShadowPart::Drop));
    }
    // past the feathering, and off the shadow's corner
    assert_eq!(doc.hit_test(0, Point::new(252.0, 200.0), 1.0, true), None);
    assert_eq!(doc.hit_test(0, Point::new(252.0, 252.0), 1.0, true), None);
    // an invisible shadow isn't picked
    obj(&mut doc, id).effects.drop_shadow.as_mut().unwrap().opacity = 0.0;
    assert_eq!(doc.hit_test(0, Point::new(230.0, 230.0), 1.0, true), None);
}

#[test]
fn a_perspective_shadow_is_picked_on_the_ground() {
    let mut doc = Document::new(400.0, 400.0);
    let id = square(&mut doc, (100.0, 100.0), 100.0, filled());
    // half as tall as the object, slanted 45 degrees from its base: from (100..200, 200) to
    // (150..250, 150)
    obj(&mut doc, id).effects.drop_shadow = Some(DropShadow { offset: Vec2::ZERO, feather: 2.0, perspective: true, angle: 45.0, stretch: 50.0, ..Default::default() });
    let o = doc.find(id).unwrap().clone();
    assert_eq!(doc.shadow_at(&o, Point::new(238.0, 160.0), 1.0, true), Some(ShadowPart::Drop));
    assert_eq!(doc.hit_test(0, Point::new(238.0, 160.0), 1.0, true), Some(id));
    // above the ground the shadow lies on
    assert_eq!(doc.hit_test(0, Point::new(238.0, 120.0), 1.0, true), None);
}

#[test]
fn an_object_above_wins_over_a_shadow_below() {
    let mut doc = Document::new(400.0, 400.0);
    let low = square(&mut doc, (100.0, 100.0), 100.0, filled());
    obj(&mut doc, low).effects.block_shadow = Some(BlockShadow { offset: Vec2::new(60.0, 60.0), ..Default::default() });
    let high = square(&mut doc, (210.0, 210.0), 40.0, filled());
    // on the upper object, over the lower one's shadow
    assert_eq!(doc.hit_test(0, Point::new(230.0, 230.0), 1.0, true), Some(high));
    assert_eq!(doc.hit_list(0, Point::new(230.0, 230.0), 1.0, true), vec![high, low]);
    // the shadow where nothing covers it
    assert_eq!(doc.hit_test(0, Point::new(255.0, 255.0), 1.0, true), Some(low));
}

// ---- #105: Break Apart takes the shadows off ----

/// A raster stand-in: a 4 x 2 image over the shadow's offset bounds.
fn fake_raster(_: &Document, o: &Object, part: ShadowPart) -> Option<ShadowRaster> {
    let s = if part == ShadowPart::Inner { o.effects.inner_shadow.as_ref()? } else { o.effects.drop_shadow.as_ref()? };
    Some(ShadowRaster { width: 4, height: 2, rgba: vec![0, 0, 0, 128].repeat(8), at: o.bounds() + s.offset })
}

#[test]
fn break_apart_makes_a_block_shadow_a_curve_behind_the_object() {
    let mut doc = Document::new(400.0, 400.0);
    let below = square(&mut doc, (0.0, 0.0), 10.0, filled());
    let id = square(&mut doc, (100.0, 100.0), 100.0, filled());
    let red = Color::rgb(160, 0, 0);
    obj(&mut doc, id).effects.block_shadow = Some(BlockShadow { offset: Vec2::new(30.0, 30.0), color: red, ..Default::default() });
    let before = doc.find(id).unwrap().clone();
    let out = ops::break_shadows(&mut doc, &[id], &mut fake_raster);
    assert_eq!(out.len(), 2);
    assert_eq!(out[1], id);
    let shadow = doc.find(out[0]).unwrap().clone();
    assert!(matches!(shadow.kind, ObjKind::Path(_)));
    assert_eq!(shadow.style.fill, Fill::Solid(red));
    assert!(!shadow.style.outline.is_visible());
    assert!((shadow.world_path().area().abs() - 16000.0).abs() < 160.0);
    assert_eq!(index_of(&doc, out[0]) + 1, index_of(&doc, id), "right behind the object");
    assert!(index_of(&doc, below) < index_of(&doc, out[0]), "and over what was below it");
    let src = doc.find(id).unwrap();
    assert!(src.effects.block_shadow.is_none());
    assert_eq!(src.style, before.style, "the object keeps its fill and outline");
    assert_eq!(src.kind, before.kind);
    // nothing more to break off
    assert!(ops::drawn_shadows(src).is_empty());
    let n = doc.pages[0].all_objects().count();
    assert_eq!(ops::break_shadows(&mut doc, &[id], &mut fake_raster), vec![id]);
    assert_eq!(doc.pages[0].all_objects().count(), n, "a second Break Apart doesn't copy the shadow");
}

#[test]
fn break_apart_makes_drop_and_inner_shadows_bitmaps() {
    let mut doc = Document::new(400.0, 400.0);
    let id = square(&mut doc, (100.0, 100.0), 100.0, filled());
    obj(&mut doc, id).effects.drop_shadow = Some(DropShadow { offset: Vec2::new(10.0, 10.0), ..Default::default() });
    obj(&mut doc, id).effects.inner_shadow = Some(DropShadow { offset: Vec2::new(4.0, 4.0), inner: true, ..Default::default() });
    obj(&mut doc, id).effects.block_shadow = Some(BlockShadow::default());
    assert_eq!(ops::drawn_shadows(doc.find(id).unwrap()), vec![ShadowPart::Drop, ShadowPart::Block, ShadowPart::Inner]);
    let out = ops::break_shadows(&mut doc, &[id], &mut fake_raster);
    assert_eq!(out.len(), 4);
    assert_eq!(out[2], id);
    // stacked as they were drawn: drop shadow, block shadow, the object, its inner shadow
    let idx: Vec<usize> = out.iter().map(|i| index_of(&doc, *i)).collect();
    assert!(idx.windows(2).all(|w| w[0] + 1 == w[1]), "{idx:?}");
    let drop = doc.find(out[0]).unwrap();
    let ObjKind::Image(img) = &drop.kind else { panic!("the drop shadow isn't a bitmap: {:?}", drop.kind) };
    assert_eq!((img.w, img.h), (100.0, 100.0));
    assert!((drop.bounds().x0 - 110.0).abs() < 1e-9 && (drop.bounds().y0 - 110.0).abs() < 1e-9);
    assert_eq!(drop.style.merge, MergeMode::Multiply, "it keeps the shadow's merge mode");
    assert_eq!(doc.images[&img.image].width, 4);
    assert!(matches!(doc.find(out[1]).unwrap().kind, ObjKind::Path(_)));
    assert!(matches!(doc.find(out[3]).unwrap().kind, ObjKind::Image(_)));
    let fx = &doc.find(id).unwrap().effects;
    assert!(fx.drop_shadow.is_none() && fx.inner_shadow.is_none() && fx.block_shadow.is_none());
}

#[test]
fn break_apart_takes_the_shadow_off_an_object_in_a_group() {
    let mut doc = Document::new(400.0, 400.0);
    let a = square(&mut doc, (100.0, 100.0), 50.0, filled());
    let b = square(&mut doc, (200.0, 100.0), 50.0, filled());
    obj(&mut doc, b).effects.block_shadow = Some(BlockShadow::default());
    let group = ops::group(&mut doc, &[a, b]).unwrap();
    let out = ops::break_shadows(&mut doc, &[b], &mut fake_raster);
    assert_eq!(out.len(), 2);
    assert_eq!(out[1], b);
    let ObjKind::Group(children) = &doc.find(group).unwrap().kind else { panic!("not a group") };
    let ids: Vec<u64> = children.iter().map(|o| o.id).collect();
    assert_eq!(ids, vec![a, out[0], b], "the shadow stays in the group, right behind its object");
    assert!(doc.find(b).unwrap().effects.block_shadow.is_none());
}

#[test]
fn shadowed_text_loses_the_shadow_first_and_splits_next() {
    let mut doc = Document::new(400.0, 400.0);
    let id = doc.new_id();
    let mut t = Object::new(id, ObjKind::Text(TextObj::artistic("AB CD", CharFormat::default())), Affine::translate((50.0, 100.0)), filled());
    t.effects.drop_shadow = Some(DropShadow::default());
    // an inner shadow isn't drawn on text, so there's nothing of it to break off
    t.effects.inner_shadow = Some(DropShadow { inner: true, ..Default::default() });
    doc.pages[0].layers[0].objects.push(t);
    assert_eq!(ops::drawn_shadows(doc.find(id).unwrap()), vec![ShadowPart::Drop]);
    let out = ops::break_shadows(&mut doc, &[id], &mut fake_raster);
    assert_eq!(out.len(), 2);
    assert!(matches!(doc.find(id).unwrap().kind, ObjKind::Text(_)), "the text is still one text");
    let words = ops::break_apart(&mut doc, &[id]);
    assert_eq!(words.len(), 2, "the next Break Apart splits the text");
}
