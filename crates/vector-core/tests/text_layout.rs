//! Text menu layout: tabs, columns, column breaks, hyphenation, bullets and numbering, drop caps and
//! linked frames.

use kurbo::Affine;
use vector_core::text::{self as vt, CharFormat, DropCap, ListFormat, TabAlign, TabStop, TextKind, TextObj};
use vector_core::{Document, ObjKind, Object, Style};

fn fmt(size: f64) -> CharFormat {
    CharFormat { size, ..Default::default() }
}

fn glyph_x(lay: &vt::TextLayout, ci: usize) -> f64 {
    lay.glyphs.iter().find(|g| g.char_index == ci).map(|g| g.origin.x).expect("glyph")
}

#[test]
fn tab_stops_place_text() {
    let mut t = TextObj::artistic("a\tb", fmt(12.0));
    t.tabs = Some(vec![TabStop { pos: 100.0, align: TabAlign::Left, leader: false }]);
    let lay = vt::layout(&t);
    assert!((glyph_x(&lay, 2) - 100.0).abs() < 0.5, "b at {}", glyph_x(&lay, 2));
    // right tab: the text ends at the stop
    t.tabs = Some(vec![TabStop { pos: 100.0, align: TabAlign::Right, leader: false }]);
    let lay = vt::layout(&t);
    let b = lay.glyphs.iter().find(|g| g.char_index == 2).unwrap();
    assert!((b.origin.x + b.advance - 100.0).abs() < 0.5);
    // default stops every half inch
    let stops = vt::tab_list(&TextObj::paragraph("", 144.0, 72.0, fmt(12.0)), 144.0);
    assert_eq!(stops.iter().map(|s| s.pos).collect::<Vec<_>>(), vec![36.0, 72.0, 108.0, 144.0]);
}

#[test]
fn columns_and_column_break() {
    let mut t = TextObj::paragraph("", 200.0, 100.0, fmt(12.0));
    t.columns = 2;
    t.column_gap = 20.0;
    assert_eq!(vt::column_boxes(&t, 200.0), vec![(0.0, 90.0), (110.0, 90.0)]);
    t.insert(0, &format!("one{}two", vt::COLUMN_BREAK));
    let lay = vt::layout(&t);
    let two = lay.glyphs.iter().find(|g| g.char_index == 4).unwrap();
    assert!(two.origin.x >= 110.0 - 0.01, "second column starts at 110");
    assert!(lay.lines.iter().any(|l| l.column == 1));
    // text past the last column does not show and is reported as overflow
    let mut long = TextObj::paragraph(&"word ".repeat(400), 100.0, 40.0, fmt(12.0));
    long.columns = 1;
    let lay = vt::layout(&long);
    assert!(lay.fit_end < long.char_count());
    assert!(lay.lines.iter().any(|l| l.hidden));
}

#[test]
fn hyphenation_breaks_long_words() {
    assert!(!vt::hyphenation_points("internationalization").is_empty());
    let mut t = TextObj::paragraph("a internationalization", 80.0, 200.0, fmt(12.0));
    let plain = vt::layout(&t);
    t.hyphenate = true;
    let hy = vt::layout(&t);
    assert!(hy.lines[0].end_char > plain.lines[0].end_char, "the word starts on the first line when hyphenated");
}

#[test]
fn bullets_numbers_and_drop_cap() {
    assert_eq!(vt::number_label(2, 4, 1), "iv");
    assert_eq!(vt::number_label(5, 28, 1), "AB");
    assert_eq!(vt::number_label(1, 3, 3), "1.1.3");
    assert_eq!(vt::number_label(14, 7, 1), "07");
    let mut t = TextObj::paragraph("first\nsecond", 300.0, 200.0, fmt(12.0));
    t.list = Some(ListFormat::default());
    let lay = vt::layout(&t);
    assert!(lay.carets[0].0 >= 18.0 - 0.01, "text after the bullet");
    assert!(lay.glyphs.iter().any(|g| g.char_index == 0 && g.origin.x < 1.0), "the bullet at the frame edge");
    assert!(lay.glyphs.len() > t.char_count() - 1, "bullets are drawn");
    let mut d = TextObj::paragraph(&"Drop cap paragraph text that runs over several lines of the frame. ".repeat(4), 200.0, 300.0, fmt(12.0));
    d.drop_cap = Some(DropCap { lines: 3, space: 0.0, hanging: false });
    let lay = vt::layout(&d);
    let first = lay.glyphs.iter().find(|g| g.char_index == 0).unwrap();
    assert!(first.size > 24.0, "the first letter is several lines tall");
    let second_line = &lay.lines[1];
    assert!(second_line.x0 > 0.0, "lines beside the drop cap are indented");
}

#[test]
fn linked_frames_flow() {
    let mut doc = Document::new(612.0, 792.0);
    let text = "Linked frames share one text that flows from one frame into the next. ".repeat(6);
    let (h, f) = (doc.new_id(), doc.new_id());
    let mut head = TextObj::paragraph(&text, 120.0, 40.0, fmt(12.0));
    head.link_next = Some(f);
    let mut follower = TextObj::paragraph("", 120.0, 400.0, fmt(12.0));
    follower.link_prev = Some(h);
    let layer = &mut doc.pages[0].layers[0];
    layer.objects.push(Object::new(h, ObjKind::Text(head.clone()), Affine::translate((10.0, 10.0)), Style::text_default()));
    layer.objects.push(Object::new(f, ObjKind::Text(follower), Affine::translate((200.0, 10.0)), Style::text_default()));
    doc.update_text_flow();
    let fit = vt::layout(&head).fit_end;
    assert!(fit > 0 && fit < head.char_count());
    let Some(ObjKind::Text(ft)) = doc.find(f).map(|o| &o.kind) else { panic!() };
    assert_eq!(ft.flow.start, fit);
    assert_eq!(ft.content, text.chars().skip(fit).collect::<String>());
    assert!(matches!(ft.kind, TextKind::Paragraph { height, .. } if height == 400.0), "the frame keeps its own size");
    // a dangling link is dropped
    doc.pages[0].layers[0].objects.retain(|o| o.id != h);
    doc.update_text_flow();
    let Some(ObjKind::Text(ft)) = doc.find(f).map(|o| &o.kind) else { panic!() };
    assert!(ft.link_prev.is_none());
}
