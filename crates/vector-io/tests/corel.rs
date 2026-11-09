//! Drawings saved by CorelDRAW open as AnyVector documents.
//!
//! The fixtures were drawn in CorelDRAW 2026 through its automation interface and saved in
//! several file versions; the expected positions are the ones CorelDRAW reported for them.
//! `shapes-*` holds the basic shapes on two pages of different sizes, `mixed-*` bitmaps,
//! formatted text, fountain fills, a PowerClip and layers in every state.

use std::path::PathBuf;
use vector_core::color::Color;
use vector_core::document::Document;
use vector_core::object::{ObjKind, Object};
use vector_core::style::{Fill, GradientKind, LineCap, LineJoin, Transparency};
use vector_core::text::TextAlign;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/corel").join(name)
}

fn open(name: &str) -> Document {
    vector_io::open(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn find<'a>(doc: &'a Document, page: usize, name: &str) -> &'a Object {
    let mut found = None;
    for o in doc.pages[page].all_objects() {
        o.walk(&mut |x| {
            if x.name == name && found.is_none() {
                found = Some(x);
            }
        });
    }
    found.unwrap_or_else(|| panic!("no object named {name}"))
}

/// Left, bottom, width and height in millimetres from the page's lower-left corner.
fn mm(doc: &Document, page: usize, o: &Object) -> [f64; 4] {
    let b = doc.bounds_of(o);
    let k = 25.4 / 72.0;
    [b.x0 * k, (doc.pages[page].height - b.y1) * k, b.width() * k, b.height() * k]
}

fn close(a: [f64; 4], b: [f64; 4]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 0.05)
}

#[track_caller]
fn at(doc: &Document, page: usize, name: &str, expect: [f64; 4]) {
    let got = mm(doc, page, find(doc, page, name));
    assert!(close(got, expect), "{name}: {got:.3?}, expected {expect:.3?}");
}

const SHAPES: [&str; 4] = ["shapes-x5-riff.cdr", "shapes-x5.cdr", "shapes-x7.cdr", "shapes-2026.cdr"];

#[test]
fn shapes_land_where_coreldraw_put_them() {
    for name in SHAPES {
        let doc = open(name);
        assert_eq!(doc.pages.len(), 2, "{name}");
        let p = &doc.pages[0];
        assert!((p.width - 210.0 / 25.4 * 72.0).abs() < 0.01 && (p.height - 297.0 / 25.4 * 72.0).abs() < 0.01, "{name}");
        let p2 = &doc.pages[1];
        assert!((p2.width - 297.0 / 25.4 * 72.0).abs() < 0.01 && (p2.height - 210.0 / 25.4 * 72.0).abs() < 0.01, "{name}");
        // layers bottom first, like CorelDRAW's Objects docker read upwards
        let layers: Vec<&str> = p.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(layers, ["Base", "Top"], "{name}");
        at(&doc, 0, "rect-rgb", [20.0, 240.0, 60.0, 40.0]);
        at(&doc, 0, "rect-round-cmyk", [100.0, 240.0, 80.0, 40.0]);
        at(&doc, 0, "ellipse", [20.0, 190.0, 60.0, 40.0]);
        at(&doc, 0, "pie", [100.0, 190.0, 55.98, 40.0]);
        at(&doc, 0, "curve-closed", [20.0, 120.0, 60.0, 38.66]);
        at(&doc, 0, "line", [100.0, 120.0, 80.0, 30.0]);
        at(&doc, 0, "polygon", [100.98, 143.82, 38.04, 36.18]);
        at(&doc, 0, "rect-rotated", [15.85, 59.51, 58.30, 50.98]);
        at(&doc, 0, "group", [20.0, 10.0, 45.0, 20.0]);
        at(&doc, 0, "rect-transparent-layer2", [150.0, 10.0, 50.0, 30.0]);
        at(&doc, 1, "page2-rect", [30.0, 30.0, 230.0, 150.0]);
        // drawing order: the first object drawn is at the bottom
        let base: Vec<&str> = p.layers[0].objects.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(base.first(), Some(&"rect-rgb"), "{name}");
        assert_eq!(base.last(), Some(&"group"), "{name}");
    }
}

#[test]
fn shapes_keep_their_kind_and_look() {
    for name in SHAPES {
        let doc = open(name);
        let r = find(&doc, 0, "rect-rgb");
        assert!(matches!(r.kind, ObjKind::Rect(_)), "{name}");
        assert_eq!(r.style.fill, Fill::Solid(Color::rgb(32, 80, 220)), "{name}");
        assert_eq!(r.style.outline.color, Some(Color::rgb(200, 0, 0)), "{name}");
        assert!((r.style.outline.width - 1.0 / 25.4 * 72.0).abs() < 0.01, "{name}");
        // CMYK red as CorelDRAW shows it, and no outline
        let c = find(&doc, 0, "rect-round-cmyk");
        assert_eq!(c.style.fill, Fill::Solid(Color::rgb(237, 50, 55)), "{name}");
        assert_eq!(c.style.outline.color, None, "{name}");
        match &c.kind {
            ObjKind::Rect(s) => assert!(s.radii.iter().all(|r| *r > 0.5), "{name}: {:?}", s.radii),
            k => panic!("{name}: {}", k.type_name()),
        }
        match &find(&doc, 0, "pie").kind {
            ObjKind::Ellipse(e) => {
                assert_eq!(e.kind, vector_core::object::EllipseKind::Pie, "{name}");
                assert!((e.start - 30.0).abs() < 0.01 && (e.end - 270.0).abs() < 0.01, "{name}: {} {}", e.start, e.end);
            }
            k => panic!("{name}: {}", k.type_name()),
        }
        // a rotated rectangle stays a rectangle
        assert!(matches!(find(&doc, 0, "rect-rotated").kind, ObjKind::Rect(_)), "{name}");
        match &find(&doc, 0, "group").kind {
            ObjKind::Group(kids) => assert_eq!(kids.len(), 2, "{name}"),
            k => panic!("{name}: {}", k.type_name()),
        }
        assert_eq!(find(&doc, 0, "rect-transparent-layer2").style.transparency, Transparency::Uniform(50.0), "{name}");
        match &find(&doc, 0, "paragraph").kind {
            ObjKind::Text(t) => {
                assert_eq!(t.content, "Paragraph text in a frame.\nSecond line.", "{name}");
                assert_eq!(t.format.font, "Arial", "{name}");
                assert!((t.format.size - 12.0).abs() < 0.01, "{name}: {}", t.format.size);
            }
            k => panic!("{name}: {}", k.type_name()),
        }
    }
    // from X7 on the fountain fill and the artistic text are stored as such
    for name in ["shapes-x7.cdr", "shapes-2026.cdr"] {
        let doc = open(name);
        match &find(&doc, 0, "gradient").style.fill {
            Fill::Gradient(g) => {
                assert_eq!(g.kind, GradientKind::Linear);
                assert_eq!(g.stops.first().map(|s| s.color), Some(Color::rgb(255, 0, 0)));
                assert_eq!(g.stops.last().map(|s| s.color), Some(Color::rgb(0, 0, 255)));
                assert!(g.start.x < 0.01 && g.end.x > 0.99, "{:?} {:?}", g.start, g.end);
            }
            f => panic!("{name}: {}", f.type_name()),
        }
        match &find(&doc, 0, "artistic").kind {
            ObjKind::Text(t) => {
                assert_eq!(t.content, "Hello AnyVector");
                assert!((t.format.size - 24.0).abs() < 0.01);
            }
            k => panic!("{name}: {}", k.type_name()),
        }
    }
}

#[test]
fn mixed_drawing_text_bitmaps_and_layers() {
    for name in ["mixed-x5.cdr", "mixed-2026.cdr"] {
        let doc = open(name);
        let p = &doc.pages[0];
        let layers: Vec<(&str, u8, bool, bool, bool)> = p.layers.iter().map(|l| (l.name.as_str(), l.master, l.visible, l.printable, l.locked)).collect();
        assert_eq!(
            layers,
            [("MasterArt", 1, true, true, false), ("Art", 0, true, true, false), ("Hidden", 0, false, true, false), ("NoPrint", 0, true, false, false), ("Locked", 0, true, true, true)],
            "{name}"
        );
        // formatted runs: "Bold " bold, "Italic " italic, "Normal" red, all Times New Roman 18 pt
        match &find(&doc, 0, "mixed").kind {
            ObjKind::Text(t) => {
                assert_eq!(t.content, "Bold Italic Normal", "{name}");
                let look = |i: usize| t.format_at(i).clone();
                assert_eq!(look(0).font, "Times New Roman", "{name}");
                assert!((look(0).size - 18.0).abs() < 0.01, "{name}");
                assert!(look(0).bold && !look(0).italic, "{name}");
                assert!(!look(6).bold && look(6).italic, "{name}");
                assert!(!look(14).bold && !look(14).italic, "{name}");
                assert_eq!(look(14).color, Some(Color::rgb(220, 0, 0)), "{name}");
            }
            k => panic!("{name}: {}", k.type_name()),
        }
        match &find(&doc, 0, "two-lines").kind {
            ObjKind::Text(t) => assert_eq!(t.content, "Linha um\nLinha dois a\u{e7}\u{e3}o", "{name}"),
            k => panic!("{name}: {}", k.type_name()),
        }
        match &find(&doc, 0, "para-center").kind {
            ObjKind::Text(t) => {
                assert_eq!(t.align, TextAlign::Center, "{name}");
                assert!((t.format.size - 10.0).abs() < 0.01, "{name}");
            }
            k => panic!("{name}: {}", k.type_name()),
        }
        assert_eq!(find(&doc, 0, "text-rot").style.fill, Fill::Solid(Color::rgb(0, 0, 200)), "{name}");
        // bitmaps: size on the page, pixels, and the transparency mask of the PNG with alpha
        at(&doc, 0, "rgb.png", [15.0, 275.89, 21.17, 14.11]);
        match &find(&doc, 0, "rgb.png").kind {
            ObjKind::Image(i) => {
                let img = &doc.images[&i.image];
                assert_eq!((img.width, img.height), (60, 40), "{name}");
                // top-left pixel of the test image is yellow
                assert_eq!(&img.rgba[0..4], &[255, 255, 0, 255], "{name}");
            }
            k => panic!("{name}: {}", k.type_name()),
        }
        match &find(&doc, 0, "alpha.png").kind {
            ObjKind::Image(i) => {
                let img = &doc.images[&i.image];
                let px = |x: usize, y: usize| &img.rgba[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4];
                assert_eq!(px(0, 0)[3], 0, "{name}");
                assert_eq!(px(20, 20), &[255, 100, 0, 255], "{name}");
            }
            k => panic!("{name}: {}", k.type_name()),
        }
        match &find(&doc, 0, "gray.png").kind {
            ObjKind::Image(i) => assert_eq!(&doc.images[&i.image].rgba[4..8], &[6, 6, 6, 255], "{name}"),
            k => panic!("{name}: {}", k.type_name()),
        }
        // the PowerClip keeps its frame and its contents
        match &find(&doc, 0, "clip-frame").kind {
            ObjKind::PowerClip { frame, contents } => {
                assert!(matches!(frame.kind, ObjKind::Ellipse(_)), "{name}");
                assert_eq!(contents.len(), 1, "{name}");
                assert_eq!(contents[0].name, "clip-content", "{name}");
            }
            k => panic!("{name}: {}", k.type_name()),
        }
        // a combined curve keeps its hole; line caps, corners and dashes come through
        match &find(&doc, 0, "donut").kind {
            ObjKind::Path(np) => assert_eq!(np.subpaths.len(), 2, "{name}"),
            k => panic!("{name}: {}", k.type_name()),
        }
        let z = find(&doc, 0, "zigzag-round");
        assert_eq!((z.style.outline.cap, z.style.outline.join), (LineCap::Round, LineJoin::Round), "{name}");
        assert!(!find(&doc, 0, "dashed").style.outline.dash.is_empty(), "{name}");
        assert_eq!(find(&doc, 0, "transp30").style.transparency, Transparency::Uniform(30.0), "{name}");
        at(&doc, 0, "stretched", [90.0, 235.0, 60.0, 15.0]);
        at(&doc, 0, "on-master", [165.0, 15.0, 20.0, 20.0]);
    }
    // from X7 on fountain fills are stored as such (older versions get them as bitmaps)
    let doc = open("mixed-2026.cdr");
    for (name, kind, stops) in [("linear45", GradientKind::Linear, 2), ("radial", GradientKind::Elliptical, 2), ("conical", GradientKind::Conical, 2), ("three-stop", GradientKind::Linear, 3)] {
        match &find(&doc, 0, name).style.fill {
            Fill::Gradient(g) => {
                assert_eq!(g.kind, kind, "{name}");
                assert_eq!(g.stops.len(), stops, "{name}");
            }
            f => panic!("{name}: {}", f.type_name()),
        }
    }
}

#[test]
fn a_coreldraw_file_is_not_saved_over() {
    // opened drawings that CorelDRAW saved must go through Save As
    assert!(!vector_io::is_drawing(&fixture("shapes-x5.cdr")));
    assert!(!vector_io::is_drawing(&fixture("shapes-x5-riff.cdr")));
    assert!(!vector_io::is_drawing(&fixture("shapes-2026.cdr")));
    let dir = std::env::temp_dir().join(format!("anyvector-corel-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let own = dir.join("own.cdr");
    vector_io::cdr::save(&open("shapes-x5.cdr"), &own, &Default::default()).unwrap();
    assert!(vector_io::is_drawing(&own));
    // what was imported survives AnyVector's own format
    let back = vector_io::open(&own).unwrap();
    assert_eq!(back.pages.len(), 2);
    at(&back, 0, "polygon", [100.98, 143.82, 38.04, 36.18]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn damaged_files_fail_cleanly() {
    let bytes = std::fs::read(fixture("shapes-x5-riff.cdr")).unwrap();
    // every truncation either opens or reports an error; none may panic
    for cut in (0..bytes.len()).step_by(bytes.len() / 60) {
        let _ = vector_io::corel::load_from_bytes(&bytes[..cut]);
    }
    // flipped bytes all over the file
    let mut seed = 0x2545_f491u32;
    for _ in 0..40 {
        let mut b = bytes.clone();
        for _ in 0..24 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let i = seed as usize % b.len();
            b[i] = (seed >> 8) as u8;
        }
        let _ = vector_io::corel::load_from_bytes(&b);
    }
    let zipped = std::fs::read(fixture("shapes-2026.cdr")).unwrap();
    for cut in (0..zipped.len()).step_by(zipped.len() / 40) {
        let _ = vector_io::corel::load_from_bytes(&zipped[..cut]);
    }
}
