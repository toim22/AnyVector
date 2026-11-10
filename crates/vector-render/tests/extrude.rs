//! Extrusions with every light off are drawn as CorelDRAW draws them: one color, or shaded
//! from the extrusion's color at the object to the shade color at the far end.

use kurbo::Affine;
use vector_core::color::Color;
use vector_core::object::{CornerType, ExtrudeEffect, ObjKind, Object, RectShape};
use vector_core::style::{Fill, Style};
use vector_core::Document;

/// A 40 pt square at (20, 100) on a 200 pt page, extruded 80 pt to the right.
fn extruded(color_mode: u8, lights: Vec<(u8, f64)>) -> tiny_skia::Pixmap {
    let mut doc = Document::new(200.0, 200.0);
    let rect = ObjKind::Rect(RectShape { w: 40.0, h: 40.0, radii: [0.0; 4], corner: CornerType::Round });
    let mut style = Style::default();
    style.fill = Fill::Solid(Color::rgb(0, 150, 220));
    style.outline.color = None;
    let mut o = Object::new(doc.new_id(), rect, Affine::translate((20.0, 100.0)), style);
    o.effects.extrude = Some(ExtrudeEffect {
        depth: kurbo::Vec2::new(80.0, 0.0),
        color: Color::rgb(200, 0, 0),
        shade: Color::rgb(0, 0, 60),
        kind: 4,
        color_mode,
        lights,
        scale: Some(1.0),
        ..Default::default()
    });
    doc.pages[0].layers[0].objects.push(o);
    vector_render::render_page_image(&doc, 0, 72.0, Some(Color::WHITE), None, true).unwrap()
}

fn at(p: &tiny_skia::Pixmap, x: u32, y: u32) -> [u8; 3] {
    let c = p.pixel(x, y).unwrap();
    [c.red(), c.green(), c.blue()]
}

fn close(a: [u8; 3], b: [u8; 3], tol: i32) -> bool {
    a.iter().zip(b).all(|(x, y)| (*x as i32 - y as i32).abs() <= tol)
}

#[test]
fn unlit_shading_runs_from_the_object_to_the_far_end() {
    let p = extruded(2, vec![]);
    // the right side, just right of the square and at its far end
    let (near, far) = (at(&p, 62, 101), at(&p, 138, 101));
    assert!(close(near, [200, 0, 0], 30), "near {near:?}");
    assert!(close(far, [0, 0, 60], 30), "far {far:?}");
}

#[test]
fn unlit_solid_and_object_colors_are_flat() {
    let p = extruded(1, vec![]);
    assert!(close(at(&p, 62, 101), [200, 0, 0], 4) && close(at(&p, 138, 101), [200, 0, 0], 4));
    let p = extruded(0, vec![]);
    assert!(close(at(&p, 62, 101), [0, 150, 220], 4) && close(at(&p, 138, 101), [0, 150, 220], 4));
}
