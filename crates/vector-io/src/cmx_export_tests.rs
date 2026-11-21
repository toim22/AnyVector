use super::*;
use kurbo::{Affine, BezPath, Point, Rect};
use vector_core::document::{Document, Layer};
use vector_core::object::{EllipseKind, EllipseShape, ObjKind, Object, RectShape};
use vector_core::path::NodePath;
use vector_core::style::{Fill, Gradient, GradientStop, LineCap, LineJoin, Outline, Style};
use vector_core::text::{CharFormat, TextObj};

fn style(fill: Fill, outline: Outline) -> Style {
    Style { fill, outline, ..Default::default() }
}

fn pen(color: Color, width: f64) -> Outline {
    Outline { color: Some(color), width, ..Default::default() }
}

/// The drawing CorelDRAW is checked against (`write_samples_for_coreldraw`): two letter pages;
/// the first with two layers, a group, a fountain fill, a dashed outline, a CMYK color and text,
/// the second with a rounded rectangle.
pub(crate) fn sample() -> Document {
    let mut doc = Document::new(612.0, 792.0);
    let red = Color::rgb(255, 0, 0);
    let id = doc.new_id();
    let rect = Object::new(id, ObjKind::Rect(RectShape { w: 144.0, h: 144.0, radii: [0.0; 4], corner: Default::default() }), Affine::translate((72.0, 72.0)), style(Fill::Solid(red), pen(Color::BLACK, 1.0)));
    let id = doc.new_id();
    let ellipse = Object::new(id, ObjKind::Ellipse(EllipseShape { w: 144.0, h: 72.0, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false }), Affine::translate((288.0, 72.0)), style(Fill::Solid(Color::rgb(0, 0, 255)), Outline::none()));
    let mut bp = BezPath::new();
    bp.move_to((72.0, 360.0));
    bp.curve_to((108.0, 288.0), (180.0, 288.0), (216.0, 360.0));
    bp.line_to((216.0, 432.0));
    let id = doc.new_id();
    let curve = Object::new(id, ObjKind::Path(NodePath::from_bez(&bp)), Affine::IDENTITY, style(Fill::None, Outline { cap: LineCap::Round, join: LineJoin::Round, ..pen(Color::rgb(0, 128, 0), 2.0) }));
    doc.pages[0].layers[0].objects = vec![rect, ellipse, curve];

    let mut second = Layer::new(doc.new_id(), "Second");
    let id = doc.new_id();
    let mut yellow = Color::from_cmyk(0.0, 0.0, 100.0, 0.0);
    yellow.cmyk = Some([0, 0, 100, 0]);
    second.objects.push(Object::new(id, ObjKind::Rect(RectShape { w: 144.0, h: 144.0, radii: [0.0; 4], corner: Default::default() }), Affine::translate((360.0, 432.0)), style(Fill::Solid(yellow), Outline::none())));
    let id = doc.new_id();
    let dashed = Object::new(id, ObjKind::Rect(RectShape { w: 72.0, h: 72.0, radii: [0.0; 4], corner: Default::default() }), Affine::translate((36.0, 576.0)), style(Fill::None, Outline { dash: vec![5.0, 2.0], ..pen(Color::BLACK, 3.0) }));
    let id = doc.new_id();
    let mut g = Gradient::linear(Color::rgb(255, 255, 0), Color::rgb(0, 0, 255));
    g.stops = vec![GradientStop { pos: 0.0, color: Color::rgb(255, 255, 0) }, GradientStop { pos: 1.0, color: Color::rgb(0, 0, 255) }];
    let fountain = Object::new(id, ObjKind::Ellipse(EllipseShape { w: 72.0, h: 72.0, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false }), Affine::translate((72.0, 612.0)), style(Fill::Gradient(g), pen(Color::BLACK, 0.5)));
    let id = doc.new_id();
    second.objects.push(Object::new(id, ObjKind::Group(vec![dashed, fountain]), Affine::IDENTITY, Style::default()));
    let id = doc.new_id();
    let text = TextObj::artistic("Hello CMX", CharFormat { size: 24.0, ..Default::default() });
    second.objects.push(Object::new(id, ObjKind::Text(text), Affine::translate((288.0, 648.0)), style(Fill::Solid(Color::BLACK), Outline::none())));
    doc.pages[0].layers.push(second);

    let p = doc.add_page(1, 612.0, 792.0);
    let id = doc.new_id();
    doc.pages[p].layers[0].objects.push(Object::new(id, ObjKind::Rect(RectShape { w: 144.0, h: 144.0, radii: [20.0; 4], corner: Default::default() }), Affine::translate((144.0, 144.0)), style(Fill::Solid(Color::rgb(0, 200, 0)), pen(Color::BLACK, 0.5))));
    doc
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn bounds_near(a: Rect, b: Rect, tol: f64) -> bool {
    near(a.x0, b.x0, tol) && near(a.y0, b.y0, tol) && near(a.x1, b.x1, tol) && near(a.y1, b.y1, tol)
}

/// Every leaf object of a page, in drawing order.
fn leaves(o: &Object, out: &mut Vec<Object>) {
    match &o.kind {
        ObjKind::Group(kids) => kids.iter().for_each(|k| leaves(k, out)),
        _ => out.push(o.clone()),
    }
}

#[test]
fn a_cmx_copy_reads_back_with_its_pages_layers_and_shapes() {
    let doc = sample();
    let (bytes, notes) = to_bytes(&doc, &CmxOptions::default()).unwrap();
    assert!(crate::corel::cmx::is_cmx(&bytes));
    assert_eq!(&bytes[8..12], b"CMX1");
    assert_eq!(notes, vec![NOTE_TEXT.to_string()], "only the text changes");
    // a .cdr named copy is recognised as a Corel file, not as AnyVector's
    assert_eq!(crate::cdr::detect(&bytes), crate::cdr::CdrKind::CorelDraw);

    let back = crate::corel::cmx::load_from_bytes(&bytes).unwrap();
    assert_eq!(back.pages.len(), 2);
    for p in &back.pages {
        assert!(near(p.width, 612.0, 0.01) && near(p.height, 792.0, 0.01), "{} x {}", p.width, p.height);
    }
    let names: Vec<&str> = back.pages[0].layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Layer 1", "Second"]);

    // first layer: the rectangle, the ellipse and the open curve, where they were
    let l1 = &back.pages[0].layers[0].objects;
    assert_eq!(l1.len(), 3);
    assert!(bounds_near(l1[0].bounds(), Rect::new(72.0, 72.0, 216.0, 216.0), 0.01), "{:?}", l1[0].bounds());
    assert_eq!(l1[0].style.fill, Fill::Solid(Color::rgb(255, 0, 0)));
    assert!(near(l1[0].style.outline.width, 1.0, 0.001), "{}", l1[0].style.outline.width);
    assert_eq!(l1[0].style.outline.color, Some(Color::BLACK));
    assert!(bounds_near(l1[1].bounds(), Rect::new(288.0, 72.0, 432.0, 144.0), 0.01), "{:?}", l1[1].bounds());
    assert_eq!(l1[1].style.fill, Fill::Solid(Color::rgb(0, 0, 255)));
    assert!(!l1[1].style.outline.is_visible());
    let c = &l1[2];
    assert!(matches!(c.kind, ObjKind::Path(_)));
    assert_eq!(c.style.fill, Fill::None);
    assert_eq!(c.style.outline.color, Some(Color::rgb(0, 128, 0)));
    assert!(near(c.style.outline.width, 2.0, 0.001));
    assert_eq!((c.style.outline.cap, c.style.outline.join), (LineCap::Round, LineJoin::Round));
    if let ObjKind::Path(np) = &c.kind {
        assert!(!np.is_closed());
        let bp = np.to_bez();
        let kinds: Vec<u8> = bp.elements().iter().map(|e| match e {
            PathEl::MoveTo(_) => b'M',
            PathEl::LineTo(_) => b'L',
            PathEl::CurveTo(..) => b'C',
            _ => b'?',
        }).collect();
        assert_eq!(kinds, b"MCL");
        assert!(bp.elements()[0].end_point().unwrap().distance(Point::new(72.0, 360.0)) < 0.01);
    }

    // second layer: the CMYK rectangle, the group (dashed square and fountain circle), the text
    let l2 = &back.pages[0].layers[1].objects;
    assert_eq!(l2.len(), 3, "{:?}", l2.iter().map(|o| o.kind.type_name()).collect::<Vec<_>>());
    match &l2[0].style.fill {
        // the reader turns CMYK into RGB with its own profile, so allow a few levels
        Fill::Solid(c) => {
            let y = yellow_rgb();
            assert!([(c.r, y.r), (c.g, y.g), (c.b, y.b)].iter().all(|(a, b)| a.abs_diff(*b) <= 4), "{c:?} vs {y:?}");
        }
        f => panic!("{f:?}"),
    }
    let ObjKind::Group(kids) = &l2[1].kind else { panic!("{:?}", l2[1].kind.type_name()) };
    assert_eq!(kids.len(), 2);
    assert_eq!(kids[0].style.outline.dash, vec![5.0, 2.0]);
    assert!(near(kids[0].style.outline.width, 3.0, 0.001));
    assert_eq!(kids[0].style.fill, Fill::None);
    let Fill::Gradient(g) = &kids[1].style.fill else { panic!("{:?}", kids[1].style.fill) };
    let s = g.sorted_stops();
    assert_eq!((s[0].color, s[s.len() - 1].color), (Color::rgb(255, 255, 0), Color::rgb(0, 0, 255)));
    // the text, as curves, where it was
    let mut text = vec![];
    leaves(&l2[2], &mut text);
    assert!(!text.is_empty() && text.iter().all(|o| matches!(o.kind, ObjKind::Path(_))));
    let tb = text.iter().map(|o| o.bounds()).reduce(|a, b| a.union(b)).unwrap();
    let source = doc.pages[0].layers[1].objects[2].bounds();
    assert!(tb.x0 >= source.x0 - 1.0 && tb.x1 <= source.x1 + 1.0 && tb.y0 >= source.y0 - 1.0 && tb.y1 <= source.y1 + 1.0, "{tb:?} outside {source:?}");
    assert!(tb.width() > 50.0 && tb.height() > 10.0, "{tb:?}");

    // the second page's rounded rectangle, with its round corners
    let p2 = &back.pages[1].layers[0].objects;
    assert_eq!(p2.len(), 1);
    assert!(bounds_near(p2[0].bounds(), Rect::new(144.0, 144.0, 288.0, 288.0), 0.01), "{:?}", p2[0].bounds());
    if let ObjKind::Path(np) = &p2[0].kind {
        assert!(np.is_closed());
        assert!(np.to_bez().elements().iter().any(|e| matches!(e, PathEl::CurveTo(..))));
    } else {
        panic!("{}", p2[0].kind.type_name());
    }
}

fn yellow_rgb() -> Color {
    Color::from_cmyk(0.0, 0.0, 100.0, 0.0)
}

#[test]
fn the_file_is_laid_out_as_coreldraw_writes_it() {
    let (b, _) = to_bytes(&sample(), &CmxOptions::default()).unwrap();
    // header: id, platform, byte order, coordinate size, version, units
    assert_eq!(&b[0..4], b"RIFF");
    assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize, b.len() - 8);
    assert_eq!(&b[12..16], b"cont");
    let cont = &b[20..];
    assert!(cont.starts_with(b"{Corel Binary Meta File}"));
    assert_eq!(&cont[48..49], b"2");
    assert_eq!(&cont[52..53], b"4");
    assert_eq!(u16::from_le_bytes([cont[62], cont[63]]), 35);
    // the master index lists the tables CorelDRAW looks for
    let index = u32::from_le_bytes(cont[84..88].try_into().unwrap()) as usize;
    assert_eq!(&b[index..index + 4], b"ixmr");
    let n = u16::from_le_bytes([b[index + 12], b[index + 13]]) as usize;
    let ids: Vec<u16> = (0..n).map(|i| u16::from_le_bytes([b[index + 14 + i * 6], b[index + 15 + i * 6]])).collect();
    assert_eq!(ids, [1, 2, 3, 15, 16, 17, 18, 19, 20, 21]);
    for i in 0..n {
        let at = u32::from_le_bytes(b[index + 16 + i * 6..index + 20 + i * 6].try_into().unwrap()) as usize;
        let tag = &b[at..at + 4];
        let want: &[u8] = match ids[i] {
            1 => b"ixmr",
            2 => b"ixpg",
            3 => b"ixtl",
            15 => b"rotl",
            16 => b"rott",
            17 => b"rota",
            18 => b"rscr",
            19 => b"rpen",
            20 => b"rdot",
            _ => b"rclr",
        };
        assert_eq!(tag, want, "table {}", ids[i]);
    }
    // every chunk ends where the next begins; every instruction of a page has an even size
    // (CorelDRAW drops the rest of a page after an odd one)
    let mut at = 12;
    let mut pages = 0;
    while at + 8 <= b.len() {
        let size = u32::from_le_bytes(b[at + 4..at + 8].try_into().unwrap()) as usize;
        if &b[at..at + 4] == b"page" {
            pages += 1;
            let (mut p, end) = (at + 8, at + 8 + size);
            while p < end {
                let short = i16::from_le_bytes([b[p], b[p + 1]]);
                let n = if short < 0 { i32::from_le_bytes(b[p + 2..p + 6].try_into().unwrap()) as usize } else { short as usize };
                assert!(n >= 4 && n % 2 == 0, "instruction of size {n} at {p:#x}");
                p += n;
            }
            assert_eq!(p, end);
        }
        at += 8 + size + (size & 1);
    }
    assert_eq!(at, b.len());
    assert_eq!(pages, 2);
}

#[test]
fn curves_close_on_their_first_point_with_coreldraws_point_types() {
    let mut bp = BezPath::new();
    bp.move_to((0.0, 0.0));
    bp.line_to((100.0, 0.0));
    bp.curve_to((150.0, 0.0), (150.0, 100.0), (100.0, 100.0));
    bp.close_path();
    // a stray point alone is left out
    bp.move_to((500.0, 500.0));
    let (pts, kinds) = points(&bp);
    assert_eq!(pts.len(), 6);
    assert_eq!(pts[5], pts[0], "a closed subpath ends on its first point");
    assert_eq!(kinds, vec![0x0c, 0x44, 0xc0, 0xc0, 0x84, 0x48]);
    // an ellipse's nodes are symmetrical
    let e = kurbo::Ellipse::new((0.0, 0.0), (1000.0, 1000.0), 0.0).to_path(0.1);
    let (_, kinds) = points(&e);
    assert!(kinds.iter().filter(|k| *k & 0xc0 == PT_CURVE).take(3).all(|k| k & PT_SYMMETRIC != 0), "{kinds:x?}");
}

#[test]
fn fountain_fills_keep_their_colors_where_the_gradient_puts_them() {
    // a linear fill from a quarter to three quarters of the width: CorelDRAW spans the whole
    // box, so the colors move to 25% and 75%
    let mut doc = Document::new(200.0, 200.0);
    let mut g = Gradient::linear(Color::rgb(255, 0, 0), Color::rgb(0, 0, 255));
    g.start = Point::new(0.25, 0.5);
    g.end = Point::new(0.75, 0.5);
    let id = doc.new_id();
    doc.pages[0].layers[0].objects.push(Object::new(id, ObjKind::Rect(RectShape { w: 100.0, h: 50.0, radii: [0.0; 4], corner: Default::default() }), Affine::translate((50.0, 50.0)), style(Fill::Gradient(g.clone()), Outline::none())));
    let mut b = Builder { doc: &doc, tables: Tables::new(), notes: Notes::default(), to_file: Affine::IDENTITY };
    let page = b.page(0);
    let Item::Curve { fill: Some(CmxFill::Fountain { kind, angle, stops, .. }), .. } = &page.layers[0].items[0] else { panic!() };
    assert_eq!(*kind, 0);
    assert!(angle.abs() < 1e-6);
    let at: Vec<u16> = stops.iter().map(|s| s.1).collect();
    assert_eq!(at, [0, 25, 75, 100]);
    // read back, the gradient draws the same colors at the same places
    let (bytes, _) = to_bytes(&doc, &CmxOptions::default()).unwrap();
    let back = crate::corel::cmx::load_from_bytes(&bytes).unwrap();
    let o = &back.pages[0].layers[0].objects[0];
    let Fill::Gradient(rg) = &o.style.fill else { panic!() };
    for x in [0.1, 0.25, 0.5, 0.75, 0.9] {
        let want = g.color_at(((x - 0.25) / 0.5f64).clamp(0.0, 1.0));
        let t = ((x - rg.start.x) / (rg.end.x - rg.start.x)).clamp(0.0, 1.0);
        let got = rg.color_at(t);
        assert!((want.r as i32 - got.r as i32).abs() <= 8 && (want.b as i32 - got.b as i32).abs() <= 8, "at {x}: {want:?} vs {got:?}");
    }

    // a radial fill reaching half way out: CorelDRAW runs it from the edge to the center
    let mut g = Gradient::radial(Color::WHITE, Color::BLACK);
    g.end = Point::new(0.75, 0.5);
    let mut b = Builder { doc: &doc, tables: Tables::new(), notes: Notes::default(), to_file: Affine::IDENTITY };
    let CmxFill::Fountain { kind, stops, cx, cy, .. } = b.fountain(&g, Affine::scale(100.0), Rect::new(0.0, 0.0, 100.0, 100.0)) else { panic!() };
    assert_eq!((kind, cx, cy), (1, 0, 0));
    let at: Vec<u16> = stops.iter().map(|s| s.1).collect();
    assert_eq!(at, [0, 50, 100], "black from the edge to half way, then to white at the center");
    assert_eq!(b.tables.colors[stops[0].0 as usize - 1], ColorRec::Rgb([0, 0, 0]));
    assert_eq!(b.tables.colors[stops[2].0 as usize - 1], ColorRec::Rgb([255, 255, 255]));
}

#[test]
fn what_a_cmx_copy_cannot_hold_is_said() {
    let mut doc = Document::new(200.0, 200.0);
    let id = doc.new_id();
    let mut o = Object::new(id, ObjKind::Rect(RectShape { w: 50.0, h: 50.0, radii: [0.0; 4], corner: Default::default() }), Affine::translate((10.0, 10.0)), style(Fill::Solid(Color::BLACK), Outline::none()));
    o.style.transparency = Transparency::Uniform(50.0);
    o.effects.drop_shadow = Some(Default::default());
    doc.pages[0].layers[0].objects.push(o);
    let mut hidden = Layer::new(doc.new_id(), "Hidden");
    hidden.visible = false;
    let id = doc.new_id();
    hidden.objects.push(Object::new(id, ObjKind::Rect(RectShape { w: 5.0, h: 5.0, radii: [0.0; 4], corner: Default::default() }), Affine::IDENTITY, Style::default()));
    doc.pages[0].layers.push(hidden);
    let (bytes, notes) = to_bytes(&doc, &CmxOptions::default()).unwrap();
    assert_eq!(notes, vec![NOTE_RASTER_EFFECTS.to_string(), NOTE_TRANSPARENCY.to_string(), NOTE_HIDDEN.to_string()]);
    // the rectangle itself is there
    let back = crate::corel::cmx::load_from_bytes(&bytes).unwrap();
    assert_eq!(back.pages[0].layers.len(), 1);
    assert_eq!(back.pages[0].layers[0].objects.len(), 1);
}

#[test]
fn a_curve_too_long_for_one_instruction_is_split() {
    let mut doc = Document::new(1000.0, 1000.0);
    let mut bp = BezPath::new();
    bp.move_to((0.0, 0.0));
    for i in 1..20_000 {
        bp.line_to((i as f64 * 0.05, (i as f64 * 0.01).sin() * 100.0 + 500.0));
    }
    let id = doc.new_id();
    doc.pages[0].layers[0].objects.push(Object::new(id, ObjKind::Path(NodePath::from_bez(&bp)), Affine::IDENTITY, style(Fill::None, pen(Color::BLACK, 1.0))));
    let (bytes, _) = to_bytes(&doc, &CmxOptions::default()).unwrap();
    let back = crate::corel::cmx::load_from_bytes(&bytes).unwrap();
    let mut out = vec![];
    for o in &back.pages[0].layers[0].objects {
        leaves(o, &mut out);
    }
    assert!(out.len() >= 3, "{}", out.len());
    let nodes: usize = out.iter().map(|o| if let ObjKind::Path(np) = &o.kind { np.to_bez().elements().len() } else { 0 }).sum();
    assert!(nodes >= 20_000, "{nodes}");
}

/// Writes the sample drawing and a few more to `target/cmx/` for checking in CorelDRAW:
/// `cargo test -p vector-io write_samples_for_coreldraw -- --ignored`.
#[test]
#[ignore]
fn write_samples_for_coreldraw() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/cmx");
    std::fs::create_dir_all(&dir).unwrap();
    let notes = export(&sample(), &dir.join("sample.cmx"), &CmxOptions::default()).unwrap();
    println!("sample.cmx: {notes:?}");
    let notes = export(&fills_sample(), &dir.join("fills.cmx"), &CmxOptions::default()).unwrap();
    println!("fills.cmx: {notes:?}");
}

/// Every fountain fill kind, a three-color fill at an angle, a rotated star, a dashed open curve
/// with a thick round outline, a hidden layer and a transparent shape.
fn fills_sample() -> Document {
    use vector_core::style::GradientKind;
    let mut doc = Document::new(612.0, 792.0);
    let mut objs = vec![];
    let kinds = [GradientKind::Linear, GradientKind::Elliptical, GradientKind::Conical, GradientKind::Rectangular];
    for (i, k) in kinds.into_iter().enumerate() {
        let mut g = Gradient::linear(Color::rgb(255, 200, 0), Color::rgb(120, 0, 160));
        g.kind = k;
        if k != GradientKind::Linear {
            g.start = Point::new(0.5, 0.5);
            g.end = Point::new(1.0, 0.5);
        }
        let id = doc.new_id();
        objs.push(Object::new(id, ObjKind::Rect(RectShape { w: 120.0, h: 120.0, radii: [0.0; 4], corner: Default::default() }), Affine::translate((36.0 + i as f64 * 140.0, 36.0)), style(Fill::Gradient(g), pen(Color::BLACK, 0.5))));
    }
    let mut g = Gradient::linear(Color::rgb(255, 0, 0), Color::rgb(0, 0, 255));
    g.stops.insert(1, GradientStop { pos: 0.5, color: Color::rgb(0, 200, 0) });
    g.start = Point::new(0.0, 0.0);
    g.end = Point::new(1.0, 1.0);
    let id = doc.new_id();
    objs.push(Object::new(id, ObjKind::Ellipse(EllipseShape { w: 200.0, h: 120.0, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false }), Affine::translate((36.0, 200.0)), style(Fill::Gradient(g), Outline::none())));
    let id = doc.new_id();
    let star = vector_core::object::PolygonShape { w: 150.0, h: 150.0, sides: 5, kind: vector_core::object::PolygonKind::Star, sharpness: 50.0, nodes: Default::default() };
    objs.push(Object::new(id, ObjKind::Polygon(star), Affine::translate((400.0, 280.0)) * Affine::rotate(0.3) * Affine::translate((-75.0, -75.0)), style(Fill::Solid(Color::rgb(255, 128, 0)), pen(Color::rgb(80, 40, 0), 2.0))));
    let mut bp = BezPath::new();
    bp.move_to((36.0, 400.0));
    bp.curve_to((150.0, 340.0), (250.0, 460.0), (360.0, 400.0));
    let id = doc.new_id();
    objs.push(Object::new(id, ObjKind::Path(NodePath::from_bez(&bp)), Affine::IDENTITY, style(Fill::None, Outline { dash: vec![3.0, 1.0], cap: LineCap::Round, ..pen(Color::rgb(0, 100, 200), 6.0) })));
    let id = doc.new_id();
    let mut see_through = style(Fill::Solid(Color::rgb(0, 160, 80)), Outline::none());
    see_through.transparency = vector_core::style::Transparency::Uniform(0.5);
    objs.push(Object::new(id, ObjKind::Rect(RectShape { w: 100.0, h: 100.0, radii: [10.0; 4], corner: Default::default() }), Affine::translate((400.0, 450.0)), see_through));
    doc.pages[0].layers[0].objects = objs;
    let mut hidden = Layer::new(doc.new_id(), "Hidden");
    hidden.visible = false;
    let id = doc.new_id();
    hidden.objects.push(Object::new(id, ObjKind::Rect(RectShape { w: 50.0, h: 50.0, radii: [0.0; 4], corner: Default::default() }), Affine::translate((500.0, 600.0)), style(Fill::Solid(Color::BLACK), Outline::none())));
    doc.pages[0].layers.push(hidden);
    doc
}
