//! DXF import tests (issue #89).

use super::*;
use crate::dxf::{to_dxf, DxfOptions, DxfUnits, DxfVersion};

/// A DXF file from (code, value) pairs.
fn dxf(pairs: &[(i32, &str)]) -> Vec<u8> {
    let mut s = String::new();
    for (c, v) in pairs {
        s.push_str(&format!("{c}\r\n{v}\r\n"));
    }
    s.into_bytes()
}

const MM: f64 = 72.0 / 25.4;

fn objects(d: &Document) -> Vec<(&str, &Object)> {
    d.pages[0].layers.iter().flat_map(|l| l.objects.iter().map(move |o| (l.name.as_str(), o))).collect()
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn sample() -> Vec<u8> {
    dxf(&[
        (0, "SECTION"), (2, "HEADER"), (9, "$INSUNITS"), (70, "4"), (0, "ENDSEC"),
        (0, "SECTION"), (2, "TABLES"),
        (0, "TABLE"), (2, "LAYER"),
        (0, "LAYER"), (2, "Walls"), (70, "0"), (62, "1"), (370, "50"),
        (0, "LAYER"), (2, "Hidden"), (70, "0"), (62, "-5"),
        (0, "ENDTAB"),
        (0, "TABLE"), (2, "STYLE"), (0, "STYLE"), (2, "Notes"), (3, "times.ttf"), (0, "ENDTAB"),
        (0, "ENDSEC"),
        (0, "SECTION"), (2, "BLOCKS"),
        (0, "BLOCK"), (2, "Peg"), (10, "0"), (20, "0"),
        (0, "LINE"), (8, "0"), (62, "0"), (10, "0"), (20, "0"), (11, "10"), (21, "0"),
        (0, "ENDBLK"),
        (0, "ENDSEC"),
        (0, "SECTION"), (2, "ENTITIES"),
        // 0: a 100 mm line along the bottom, red by its layer, 0.5 mm wide
        (0, "LINE"), (8, "Walls"), (10, "0"), (20, "0"), (11, "100"), (21, "0"),
        // 1: a green circle of radius 10 at (50, 50)
        (0, "CIRCLE"), (8, "Walls"), (62, "3"), (10, "50"), (20, "50"), (40, "10"),
        // 2: a closed polyline with a half-circle side (bulge 1)
        (0, "LWPOLYLINE"), (8, "Walls"), (90, "2"), (70, "1"), (10, "0"), (20, "100"), (42, "1"), (10, "20"), (20, "100"),
        // 3: a quarter arc, on the hidden layer
        (0, "ARC"), (8, "Hidden"), (10, "0"), (20, "0"), (40, "30"), (50, "0"), (51, "90"),
        // 4: a cubic spline, one Bezier piece
        (0, "SPLINE"), (8, "Walls"), (71, "3"), (72, "8"), (73, "4"),
        (40, "0"), (40, "0"), (40, "0"), (40, "0"), (40, "1"), (40, "1"), (40, "1"), (40, "1"),
        (10, "0"), (20, "0"), (10, "10"), (20, "20"), (10, "30"), (20, "20"), (10, "40"), (20, "0"),
        // 5: text with a degree sign, 5 units high, in the Notes style
        (0, "TEXT"), (8, "Walls"), (10, "10"), (20, "80"), (40, "5"), (1, "Angle 45%%d"), (7, "Notes"),
        // 6: multiline text with formatting codes
        (0, "MTEXT"), (8, "Walls"), (10, "60"), (20, "90"), (40, "4"), (71, "1"), (1, "{\\fArial|b1;One}\\PTwo \\S1^2;"),
        // 7: the block, turned 90 degrees and doubled, in the block's color (blue)
        (0, "INSERT"), (8, "Walls"), (62, "5"), (2, "Peg"), (10, "80"), (20, "10"), (41, "2"), (42, "2"), (50, "90"),
        // 8: a solid hatch square
        (0, "HATCH"), (8, "Walls"), (62, "2"), (70, "1"), (91, "1"), (92, "2"), (72, "0"), (73, "1"), (93, "4"),
        (10, "70"), (20, "60"), (10, "80"), (20, "60"), (10, "80"), (20, "70"), (10, "70"), (20, "70"), (97, "0"),
        // left out: paper space and a 3D solid
        (0, "LINE"), (67, "1"), (10, "0"), (20, "0"), (11, "1"), (21, "1"),
        (0, "3DSOLID"), (8, "Walls"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ])
}

#[test]
fn reads_entities_layers_units_and_colors() {
    let (d, notes) = load_with_notes(&sample()).unwrap();
    let names: Vec<&str> = d.pages[0].layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Walls", "Hidden"]);
    assert!(!d.pages[0].layers[1].visible, "a layer that is off comes in hidden");
    let objs = objects(&d);
    assert_eq!(objs.len(), 9, "{:?}", objs.iter().map(|o| o.1.kind.type_name()).collect::<Vec<_>>());
    // the page fits the drawing: x from 0 to 100 mm, y from 0 to 100 mm (the polyline's chord)
    let pg = &d.pages[0];
    assert!(near(pg.width, 100.0 * MM, 0.01), "{}", pg.width);
    assert!(near(pg.height, 100.0 * MM, 0.01), "{}", pg.height);
    // the line: 100 mm long on the bottom edge of the page (y up became y down)
    let line = objs[0].1.world_path().bounding_box();
    assert!(near(line.width(), 100.0 * MM, 1e-6) && near(line.y0, pg.height, 1e-6), "{line:?}");
    assert_eq!(objs[0].1.style.outline.color, Some(Color::rgb(255, 0, 0)), "red, from the layer");
    assert!(near(objs[0].1.style.outline.width, 0.5 * MM, 1e-9), "0.5 mm, from the layer");
    // the circle
    let c = objs[1].1.world_path().bounding_box();
    assert!(near(c.width(), 20.0 * MM, 1e-3) && near(c.center().x, 50.0 * MM, 1e-3) && near(c.center().y, pg.height - 50.0 * MM, 1e-3), "{c:?}");
    assert_eq!(objs[1].1.style.outline.color, Some(Color::rgb(0, 255, 0)));
    // the bulge: a half circle of radius 10 under the chord (counterclockwise from (0,100) to (20,100))
    let b = objs[2].1.world_path().bounding_box();
    assert!(near(b.width(), 20.0 * MM, 1e-3) && near(b.height(), 10.0 * MM, 1e-3), "{b:?}");
    assert!(near(b.y0, pg.height - 100.0 * MM, 1e-3), "the chord is at y=100, the half circle under it: {b:?}");
    // the arc on the hidden layer (objects are listed layer by layer, so it is last)
    assert_eq!(objs[8].0, "Hidden");
    let a = objs[8].1.world_path().bounding_box();
    assert!(near(a.width(), 30.0 * MM, 1e-3) && near(a.height(), 30.0 * MM, 1e-3), "{a:?}");
    // the spline: one cubic through its end points
    let ObjKind::Path(sp) = &objs[3].1.kind else { panic!() };
    let bez = sp.to_bez();
    assert_eq!(bez.elements().iter().filter(|e| matches!(e, PathEl::CurveTo(..))).count(), 1);
    // text
    let ObjKind::Text(t) = &objs[4].1.kind else { panic!("{:?}", objs[4].1.kind.type_name()) };
    assert_eq!(t.content, "Angle 45°");
    assert_eq!(t.format.font, "Times New Roman", "the font of its text style");
    let ObjKind::Text(m) = &objs[5].1.kind else { panic!() };
    assert_eq!(m.content, "One\nTwo 1/2");
    // the block: a 10-unit line, doubled to 20 and turned upright, blue from the insert
    let peg = objs[6].1.world_path().bounding_box();
    assert!(near(peg.height(), 20.0 * MM, 1e-6) && near(peg.width(), 0.0, 1e-6), "{peg:?}");
    assert!(near(peg.x0, 80.0 * MM, 1e-6), "{peg:?}");
    assert_eq!(objs[6].1.style.outline.color, Some(Color::rgb(0, 0, 255)), "BYBLOCK takes the insert's color");
    // the hatch: filled, no outline
    assert_eq!(objs[7].1.style.fill, Fill::Solid(Color::rgb(255, 255, 0)));
    assert!(objs[7].1.style.outline.color.is_none());
    assert!(notes.iter().any(|n| n.contains("paper space")), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("3DSOLID")), "{notes:?}");
}

#[test]
fn text_sits_on_its_insertion_point() {
    let (d, _) = load_with_notes(&sample()).unwrap();
    let objs = objects(&d);
    let pg = &d.pages[0];
    // TEXT is anchored at the left end of its baseline: (10, 80) in DXF
    let t = objs[4].1;
    let origin = t.transform * Point::ZERO;
    assert!(near(origin.x, 10.0 * MM, 1e-6) && near(origin.y, pg.height - 80.0 * MM, 1e-6), "{origin:?}");
    // MTEXT attached top left at (60, 90): its top edge is there
    let m = objs[5].1;
    let ObjKind::Text(mt) = &m.kind else { panic!() };
    let lay = text::layout(mt);
    let top_left = m.transform * Point::new(lay.bounds.x0, -lay.lines[0].ascent);
    assert!(near(top_left.x, 60.0 * MM, 1e-6) && near(top_left.y, pg.height - 90.0 * MM, 1e-6), "{top_left:?}");
}

#[test]
fn round_trip_through_dxf_export() {
    // a square and a circle exported by AnyVector come back where they were
    let mut doc = Document::new(200.0, 150.0);
    let mut st = Style::default();
    st.fill = Fill::None;
    st.outline.color = Some(Color::rgb(255, 0, 0));
    let sq = vector_core::path::polyline(&[Point::new(20.0, 30.0), Point::new(80.0, 30.0), Point::new(80.0, 90.0), Point::new(20.0, 90.0)], true);
    let id = doc.new_id();
    doc.pages[0].layers[0].objects.push(Object::new(id, ObjKind::Path(sq), Affine::IDENTITY, st.clone()));
    let e = EllipseShape { w: 40.0, h: 40.0, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false };
    let id = doc.new_id();
    doc.pages[0].layers[0].objects.push(Object::new(id, ObjKind::Ellipse(e), Affine::translate((150.0, 100.0)), st));
    for (version, splines, units) in [(DxfVersion::R12, false, DxfUnits::Inches), (DxfVersion::R2000, true, DxfUnits::Millimeters)] {
        let s = to_dxf(&doc, &DxfOptions { version, splines, units, ..Default::default() });
        // R12 has no units in the file: the reader is told (inches, as exported)
        let (back, notes) = load_with_units(s.as_bytes(), 72.0).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        let objs = objects(&back);
        assert_eq!(objs.len(), 2);
        // the page is fitted to the drawing, so compare the shapes relative to each other
        let a = objs[0].1.world_path().bounding_box();
        let b = objs[1].1.world_path().bounding_box();
        assert!(near(a.width(), 60.0, 0.01) && near(a.height(), 60.0, 0.01), "{version:?} {a:?}");
        assert!(near(b.width(), 40.0, 0.05) && near(b.height(), 40.0, 0.05), "{version:?} {b:?}");
        let (oa, ob) = (doc.pages[0].layers[0].objects[0].world_path().bounding_box(), doc.pages[0].layers[0].objects[1].world_path().bounding_box());
        let (want, got) = (ob.center() - oa.center(), b.center() - a.center());
        assert!((want - got).hypot() < 0.05, "{version:?} the circle is {got:?} from the square, was {want:?}");
        assert_eq!(objs[0].1.style.outline.color, Some(Color::rgb(255, 0, 0)));
    }
}

#[test]
fn unknown_units_are_millimeters_and_said() {
    let f = dxf(&[(0, "SECTION"), (2, "ENTITIES"), (0, "LINE"), (10, "0"), (20, "0"), (11, "10"), (21, "0"), (0, "ENDSEC"), (0, "EOF")]);
    let (d, notes) = load_with_notes(&f).unwrap();
    assert!(near(d.pages[0].width, 10.0 * MM, 1e-9));
    assert!(notes.iter().any(|n| n.contains("millimeters")), "{notes:?}");
}

#[test]
fn binary_and_broken_files_are_refused() {
    assert!(load_with_notes(b"AutoCAD Binary DXF\r\n\x1a\x00").is_err());
    assert!(load_with_notes(b"hello").is_err());
}

#[test]
fn mtext_codes() {
    assert_eq!(mtext_plain("\\A1;{\\C1;Red} \\Ltext\\l\\PNext\\~line"), "Red text\nNext line");
    assert_eq!(mtext_plain("\\U+00B0C"), "°C");
    assert_eq!(special_chars("%%c20 %%p0.1 100%%%"), "Ø20 ±0.1 100%");
}
