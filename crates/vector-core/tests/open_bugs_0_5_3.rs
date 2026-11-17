//! Regression tests for the bugs fixed in 0.5.3: picking unfilled shapes (#96), converted
//! ellipses that stayed open curves (#97), Break Apart on artistic text (#98) and Adobe Swatch
//! Exchange palettes with spot colors (#95).

use kurbo::{Affine, BezPath, Point, Shape};
use vector_core::color::Palette;
use vector_core::object::{CornerType, EllipseKind, EllipseShape, RectShape};
use vector_core::ops;
use vector_core::path::NodePath;
use vector_core::text::{CharFormat, Span, TextObj};
use vector_core::{Color, Document, Fill, ObjKind, Object, Style};

fn push(doc: &mut Document, kind: ObjKind, at: (f64, f64), style: Style) -> u64 {
    let id = doc.new_id();
    let o = Object::new(id, kind, Affine::translate(at), style);
    doc.pages[0].layers.last_mut().unwrap().objects.push(o);
    id
}

fn ellipse(w: f64, h: f64) -> ObjKind {
    ObjKind::Ellipse(EllipseShape { w, h, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false })
}

// ---- #96: Treat all objects as filled ----

#[test]
fn unfilled_shape_is_not_picked_inside_unless_treated_as_filled() {
    let mut doc = Document::new(200.0, 200.0);
    let r = RectShape { w: 100.0, h: 80.0, radii: [0.0; 4], corner: CornerType::default() };
    let id = push(&mut doc, ObjKind::Rect(r), (20.0, 20.0), Style::default());
    assert!(doc.pages[0].layers.last().unwrap().objects[0].style.fill.is_none());
    assert_eq!(doc.hit_test(0, Point::new(70.0, 60.0), 5.0, false), None, "inside, not treated as filled");
    assert_eq!(doc.hit_test(0, Point::new(70.0, 60.0), 5.0, true), Some(id), "inside, treated as filled");
    assert_eq!(doc.hit_test(0, Point::new(20.0, 60.0), 5.0, false), Some(id), "on the outline");
    // the document copy of the option is off by default and no longer read
    assert!(!Document::new(10.0, 10.0).settings.treat_as_filled);
}

#[test]
fn a_click_inside_an_unfilled_shape_reaches_the_filled_one_behind() {
    let mut doc = Document::new(300.0, 300.0);
    let filled = Style { fill: Fill::Solid(Color::rgb(255, 0, 0)), ..Style::default() };
    let back = push(&mut doc, ObjKind::Rect(RectShape { w: 60.0, h: 60.0, radii: [0.0; 4], corner: CornerType::default() }), (70.0, 70.0), filled);
    let _front = push(&mut doc, ObjKind::Rect(RectShape { w: 200.0, h: 200.0, radii: [0.0; 4], corner: CornerType::default() }), (20.0, 20.0), Style::default());
    assert_eq!(doc.hit_test(0, Point::new(100.0, 100.0), 3.0, false), Some(back));
}

// ---- #97: a converted ellipse is a closed curve ----

#[test]
fn ellipse_path_is_closed_and_arc_stays_open() {
    let bp = vector_core::object::ellipse_path(&EllipseShape { w: 80.0, h: 40.0, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false });
    assert!(bp.elements().iter().any(|e| matches!(e, kurbo::PathEl::ClosePath)));
    assert!(NodePath::from_bez(&bp).subpaths[0].closed);
    let arc = vector_core::object::ellipse_path(&EllipseShape { w: 80.0, h: 40.0, kind: EllipseKind::Arc, start: 0.0, end: 90.0, clockwise: false });
    assert!(!NodePath::from_bez(&arc).subpaths[0].closed, "an arc is open");
    let pie = vector_core::object::ellipse_path(&EllipseShape { w: 80.0, h: 40.0, kind: EllipseKind::Pie, start: 0.0, end: 90.0, clockwise: false });
    assert!(NodePath::from_bez(&pie).subpaths[0].closed, "a pie is closed");
}

#[test]
fn converted_ellipse_is_picked_inside_when_treated_as_filled() {
    let mut doc = Document::new(300.0, 300.0);
    let id = push(&mut doc, ellipse(100.0, 60.0), (50.0, 50.0), Style::default());
    let ids = ops::convert_to_curves(&mut doc, &[id]);
    let o = doc.find(ids[0]).unwrap();
    assert!(matches!(o.kind, ObjKind::Path(_)));
    assert!(o.is_closed_shape(), "the converted ellipse is closed");
    assert_eq!(doc.hit_test(0, Point::new(100.0, 80.0), 3.0, true), Some(ids[0]));
    assert_eq!(doc.hit_test(0, Point::new(100.0, 80.0), 3.0, false), None, "unfilled interior with the option off");
}

#[test]
fn from_bez_closes_a_path_whose_ends_meet_but_not_a_two_point_line() {
    let mut bp = BezPath::new();
    bp.move_to((0.0, 0.0));
    bp.line_to((10.0, 0.0));
    bp.line_to((10.0, 10.0));
    bp.line_to((0.0, 0.0));
    let np = NodePath::from_bez(&bp);
    assert!(np.subpaths[0].closed);
    assert_eq!(np.subpaths[0].nodes.len(), 3, "the end node merges into the start");
    let mut line = BezPath::new();
    line.move_to((0.0, 0.0));
    line.line_to((0.0, 0.0));
    assert!(!NodePath::from_bez(&line).subpaths[0].closed);
    let mut open = BezPath::new();
    open.move_to((0.0, 0.0));
    open.line_to((10.0, 0.0));
    open.line_to((10.0, 10.0));
    assert!(!NodePath::from_bez(&open).subpaths[0].closed);
}

#[test]
fn loading_repairs_ellipses_saved_open() {
    // what a converted ellipse looked like in drawings saved before 0.5.3
    let bp = kurbo::Ellipse::new((50.0, 30.0), (50.0, 30.0), 0.0).to_path(0.01);
    let mut np = NodePath::from_bez(&bp);
    np.subpaths[0].closed = false;
    let n = np.subpaths[0].nodes.len();
    let last = np.subpaths[0].nodes[0];
    np.subpaths[0].nodes.push(last);
    let mut doc = Document::new(200.0, 200.0);
    let id = push(&mut doc, ObjKind::Path(np), (10.0, 10.0), Style::default());
    assert!(!doc.find(id).unwrap().is_closed_shape());
    doc.repair_closure();
    let o = doc.find(id).unwrap();
    assert!(o.is_closed_shape());
    let ObjKind::Path(np) = &o.kind else { panic!() };
    assert_eq!(np.subpaths[0].nodes.len(), n);
}

// ---- #98: Break Apart keeps text as text ----

fn texts(doc: &Document) -> Vec<String> {
    doc.pages[0].layers.last().unwrap().objects.iter().map(|o| match &o.kind {
        ObjKind::Text(t) => t.content.clone(),
        k => panic!("Break Apart made a {k:?}, not text"),
    }).collect()
}

#[test]
fn break_apart_splits_text_into_lines_then_words_then_characters() {
    let mut doc = Document::new(400.0, 400.0);
    let id = push(&mut doc, ObjKind::Text(TextObj::artistic("Hello World\nSecond line", CharFormat::default())), (20.0, 60.0), Style::default());
    let ids = ops::break_apart(&mut doc, &[id]);
    assert_eq!(texts(&doc), ["Hello World", "Second line"]);
    assert_eq!(ids.len(), 2);
    let ids = ops::break_apart(&mut doc, &ids);
    assert_eq!(texts(&doc), ["Hello", "World", "Second", "line"]);
    let ids = ops::break_apart(&mut doc, &ids);
    assert_eq!(texts(&doc).concat(), "HelloWorldSecondline");
    assert_eq!(ids.len(), 20);
    let again = ops::break_apart(&mut doc, &ids);
    assert_eq!(again, ids, "single characters stay as they are");
    assert_eq!(texts(&doc).len(), 20);
}

#[test]
fn broken_text_stays_where_it_was() {
    let mut doc = Document::new(400.0, 400.0);
    let t = TextObj::artistic("Hello World\nSecond line", CharFormat::default());
    let id = push(&mut doc, ObjKind::Text(t), (20.0, 60.0), Style::default());
    let before = doc.find(id).unwrap().world_path().bounding_box();
    let ids = ops::break_apart(&mut doc, &[id]);
    let ids = ops::break_apart(&mut doc, &ids);
    let after = ids.iter().map(|i| doc.find(*i).unwrap().world_path().bounding_box()).reduce(|a, b| a.union(b)).unwrap();
    for (a, b) in [(before.x0, after.x0), (before.y0, after.y0), (before.y1, after.y1)] {
        assert!((a - b).abs() < 0.5, "{before:?} vs {after:?}");
    }
    // the second line's words sit on the second baseline
    let second = doc.find(ids[2]).unwrap().transform.translation();
    let first = doc.find(ids[0]).unwrap().transform.translation();
    assert!(second.y > first.y + 10.0);
}

#[test]
fn break_apart_keeps_character_formatting() {
    let mut doc = Document::new(400.0, 400.0);
    let mut t = TextObj::artistic("ab cd", CharFormat::default());
    let red = CharFormat { color: Some(Color::rgb(255, 0, 0)), ..CharFormat::default() };
    t.spans = vec![Span { start: 3, end: 5, format: red.clone() }];
    let id = push(&mut doc, ObjKind::Text(t), (0.0, 50.0), Style::default());
    let ids = ops::break_apart(&mut doc, &[id]);
    let ObjKind::Text(cd) = &doc.find(ids[1]).unwrap().kind else { panic!() };
    assert_eq!(cd.content, "cd");
    assert_eq!(cd.spans, vec![Span { start: 0, end: 2, format: red }]);
    let ObjKind::Text(ab) = &doc.find(ids[0]).unwrap().kind else { panic!() };
    assert!(ab.spans.is_empty());
}

#[test]
fn single_character_and_paragraph_text_are_not_turned_into_curves() {
    let mut doc = Document::new(400.0, 400.0);
    let one = push(&mut doc, ObjKind::Text(TextObj::artistic("O", CharFormat::default())), (0.0, 50.0), Style::default());
    assert_eq!(ops::break_apart(&mut doc, &[one]), vec![one]);
    let mut para = TextObj::artistic("One\nTwo", CharFormat::default());
    para.kind = vector_core::text::TextKind::Paragraph { width: 200.0, height: 100.0 };
    let p = push(&mut doc, ObjKind::Text(para), (0.0, 100.0), Style::default());
    assert_eq!(ops::break_apart(&mut doc, &[p]), vec![p]);
    assert_eq!(texts(&doc).len(), 2);
    assert!(ops::next_text_break(&TextObj::artistic("   ", CharFormat::default())).is_none());
}

#[test]
fn break_apart_still_splits_combined_curves() {
    let mut doc = Document::new(400.0, 400.0);
    let a = push(&mut doc, ellipse(40.0, 40.0), (10.0, 10.0), Style::default());
    let b = push(&mut doc, ellipse(40.0, 40.0), (100.0, 10.0), Style::default());
    let c = ops::combine(&mut doc, &[a, b]).expect("combined");
    assert_eq!(ops::break_apart(&mut doc, &[c]).len(), 2);
}

// ---- #95: Adobe Swatch Exchange palettes ----

fn ase_color(name: &str, model: &[u8; 4], values: &[f32], kind: u16) -> Vec<u8> {
    let mut body = vec![];
    let units: Vec<u16> = name.encode_utf16().chain([0]).collect();
    body.extend((units.len() as u16).to_be_bytes());
    for u in units {
        body.extend(u.to_be_bytes());
    }
    body.extend(model);
    for v in values {
        body.extend(v.to_be_bytes());
    }
    body.extend(kind.to_be_bytes());
    let mut b = 1u16.to_be_bytes().to_vec();
    b.extend((body.len() as u32).to_be_bytes());
    b.extend(body);
    b
}

#[test]
fn ase_palettes_keep_names_and_spot_colors() {
    let blocks = [
        ase_color("PANTONE 485 C", b"CMYK", &[0.0, 0.95, 1.0, 0.0], 1),
        ase_color("Sky", b"RGB ", &[0.0, 0.5, 1.0], 2),
        ase_color("Mid gray", b"Gray", &[0.5], 0),
        ase_color("Lab red", b"LAB ", &[0.5, 60.0, 50.0], 2),
    ];
    let mut data = b"ASEF".to_vec();
    data.extend(1u16.to_be_bytes());
    data.extend(0u16.to_be_bytes());
    data.extend((blocks.len() as u32).to_be_bytes());
    for b in &blocks {
        data.extend(b);
    }
    let p = Palette::parse_ase("Book", &data).expect("an ASE palette");
    assert_eq!(p.name, "Book");
    let names: Vec<&str> = p.colors.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["PANTONE 485 C", "Sky", "Mid gray", "Lab red"]);
    assert!(p.colors[0].spot && !p.colors[1].spot && !p.colors[2].spot);
    assert_eq!(p.colors[0].color.cmyk, Some([0, 95, 100, 0]));
    assert_eq!((p.colors[1].color.r, p.colors[1].color.g, p.colors[1].color.b), (0, 128, 255));
    assert_eq!(p.colors[2].color.r, 128);
    assert!(p.colors[3].color.r > 150 && p.colors[3].color.g < 100);
    assert!(Palette::parse_ase("x", b"not a palette").is_none());
    // a damaged file gives what it could read
    assert_eq!(Palette::parse_ase("Book", &data[..data.len() - 5]).map(|p| p.colors.len()), Some(3));
}
