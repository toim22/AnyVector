use kurbo::Affine;
use vector_core::object::*;
use vector_core::style::*;
use vector_core::text::{CharFormat, TextObj};
use vector_core::{Color, Document, ObjKind, Object};

fn sample() -> Document {
    let mut d = Document::new(612.0, 792.0);
    let id = d.new_id();
    let mut st = Style::filled(Color::RED);
    st.outline = Outline { width: 2.0, ..Default::default() };
    let r = Object::new(id, ObjKind::Rect(RectShape { w: 200.0, h: 100.0, radii: [10.0; 4], corner: CornerType::Round }), Affine::translate((50.0, 50.0)), st);
    let id = d.new_id();
    let mut st2 = Style::default();
    st2.fill = Fill::Gradient(Gradient::linear(Color::BLUE, Color::WHITE));
    let mut e = Object::new(id, ObjKind::Ellipse(EllipseShape { w: 150.0, h: 150.0, kind: EllipseKind::Ellipse, start: 0.0, end: 360.0, clockwise: false }), Affine::translate((300.0, 300.0)), st2);
    e.style.transparency = Transparency::Uniform(30.0);
    let id = d.new_id();
    let mut star = Object::new(id, ObjKind::Polygon(PolygonShape { w: 120.0, h: 120.0, sides: 5, kind: PolygonKind::Star, sharpness: 53.0 }), Affine::translate((80.0, 400.0)), Style::filled(Color::rgb(255, 241, 0)));
    star.effects.drop_shadow = Some(DropShadow { offset: kurbo::Vec2::new(6.0, 6.0), feather: 4.0, color: Color::BLACK, opacity: 50.0, inner: false, ..Default::default() });
    let id = d.new_id();
    let t = Object::new(id, ObjKind::Text(TextObj::artistic("Vector", CharFormat::default())), Affine::translate((60.0, 650.0)), Style::text_default());
    d.pages[0].layers[0].objects = vec![r, e, star, t];
    d
}

#[test]
fn native_roundtrip() {
    let d = sample();
    let bytes = vector_io::native::save_to_bytes(&d).unwrap();
    let d2 = vector_io::native::load_from_bytes(&bytes).unwrap();
    assert_eq!(d.pages[0].layers[0].objects, d2.pages[0].layers[0].objects);
}

#[test]
fn exports() {
    let d = sample();
    let dir = std::env::temp_dir().join("vector-io-test");
    std::fs::create_dir_all(&dir).unwrap();
    let svg = vector_io::svg_export::page_to_svg(&d, 0, &Default::default());
    assert!(svg.contains("<svg"));
    std::fs::write(dir.join("out.svg"), &svg).unwrap();
    let d3 = vector_io::svg_import::document_from_svg(svg.as_bytes()).unwrap();
    assert!(!d3.pages[0].layers[0].objects.is_empty());
    let pdf = vector_io::pdf::export_bytes(&d, &Default::default()).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    std::fs::write(dir.join("out.pdf"), &pdf).unwrap();
    vector_io::image_io::export_raster(&d, 0, &dir.join("out.png"), &vector_io::image_io::RasterOptions { dpi: 72.0, ..Default::default() }).unwrap();
    println!("written to {}", dir.display());
}
