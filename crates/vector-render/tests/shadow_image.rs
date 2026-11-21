//! A shadow drawn alone, for Break Apart (#105), and the block shadow drawn from the outline
//! vector-core computes (the one picking and Break Apart use too).

use kurbo::{Affine, Vec2};
use vector_core::color::Color;
use vector_core::object::{BlockShadow, CornerType, DropShadow, ObjKind, Object, RectShape};
use vector_core::style::{Fill, Outline, Style};
use vector_core::Document;

/// A 100 pt red square at (100, 100) on a 400 pt page.
fn square() -> (Document, Object) {
    let mut doc = Document::new(400.0, 400.0);
    let st = Style { fill: Fill::Solid(Color::rgb(220, 0, 0)), outline: Outline::none(), ..Style::default() };
    let r = RectShape { w: 100.0, h: 100.0, radii: [0.0; 4], corner: CornerType::default() };
    let o = Object::new(doc.new_id(), ObjKind::Rect(r), Affine::translate((100.0, 100.0)), st);
    doc.pages[0].layers[0].objects.push(o.clone());
    (doc, o)
}

fn alpha_at(pm: &tiny_skia::Pixmap, at: kurbo::Rect, x: f64, y: f64) -> u8 {
    let sx = ((x - at.x0) / at.width() * pm.width() as f64) as u32;
    let sy = ((y - at.y0) / at.height() * pm.height() as f64) as u32;
    pm.pixel(sx.min(pm.width() - 1), sy.min(pm.height() - 1)).unwrap().alpha()
}

#[test]
fn a_drop_shadow_alone_covers_the_shadow_and_its_feathering() {
    let (doc, mut o) = square();
    o.effects.drop_shadow = Some(DropShadow { offset: Vec2::new(30.0, 30.0), feather: 8.0, opacity: 60.0, ..Default::default() });
    let (pm, at) = vector_render::shadow_image(&doc, &o, false, 144.0).expect("a shadow image");
    // the shifted square (130..230) and some of the feathering around it, cut to what shows
    assert!(at.x0 > 110.0 && at.x0 < 130.0 && at.x1 > 230.0 && at.x1 < 250.0, "{at:?}");
    assert!(at.y0 > 110.0 && at.y0 < 130.0 && at.y1 > 230.0 && at.y1 < 250.0, "{at:?}");
    assert_eq!(pm.width(), (at.width() * 2.0).round() as u32, "144 dpi is 2 pixels a point");
    // the middle is the shadow's opacity; the object itself isn't in it
    let a = alpha_at(&pm, at, 180.0, 180.0);
    assert!((a as f64 - 0.6 * 255.0).abs() < 8.0, "{a}");
    assert!(pm.pixels().iter().all(|p| p.red() == 0), "only the black shadow: the red object isn't drawn");
    // no shadow, no image
    o.effects.drop_shadow = None;
    assert!(vector_render::shadow_image(&doc, &o, false, 144.0).is_none());
}

#[test]
fn an_inner_shadow_alone_stays_inside_the_object() {
    let (doc, mut o) = square();
    o.effects.inner_shadow = Some(DropShadow { offset: Vec2::new(6.0, 6.0), feather: 4.0, opacity: 80.0, inner: true, ..Default::default() });
    let (pm, at) = vector_render::shadow_image(&doc, &o, true, 72.0).expect("an inner shadow image");
    assert!(at.x0 >= 99.0 && at.y0 >= 99.0 && at.x1 <= 201.0 && at.y1 <= 201.0, "{at:?}");
    // dark along the top left edges, clear in the middle
    assert!(alpha_at(&pm, at, 101.5, 150.0) > 100);
    assert_eq!(alpha_at(&pm, at, 160.0, 160.0), 0);
}

#[test]
fn a_block_shadow_is_drawn_swept_along_its_offset() {
    let (mut doc, mut o) = square();
    o.effects.block_shadow = Some(BlockShadow { offset: Vec2::new(40.0, 40.0), color: Color::rgb(0, 0, 200), expand: 4.0, ..Default::default() });
    doc.pages[0].layers[0].objects[0] = o;
    let pm = vector_render::render_page_image(&doc, 0, 72.0, Some(Color::WHITE), None, true).unwrap();
    let px = |x: u32, y: u32| {
        let c = pm.pixel(x, y).unwrap();
        (c.red(), c.green(), c.blue())
    };
    assert_eq!(px(150, 150), (220, 0, 0), "the object over its shadow");
    assert_eq!(px(215, 150), (0, 0, 200), "the band the side sweeps");
    assert_eq!(px(240, 240), (0, 0, 200), "the far end");
    assert_eq!(px(102, 102), (220, 0, 0));
    assert_eq!(px(98, 150), (0, 0, 200), "grown by Expand");
    assert_eq!(px(235, 110), (255, 255, 255), "off the swept shape");
}
