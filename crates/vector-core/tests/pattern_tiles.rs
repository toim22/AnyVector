//! Pattern fills that keep what their tile is made of.

use kurbo::Affine;
use std::sync::Arc;
use vector_core::document::{ImageData, NamedPattern, PatternTile};
use vector_core::object::{CornerType, ObjKind, Object, RectShape};
use vector_core::style::{PatternFill, PatternKind, PatternSource};
use vector_core::{Color, Document, Fill, Style};

fn square(doc: &mut Document) -> Object {
    let shape = RectShape { w: 10.0, h: 10.0, radii: [0.0; 4], corner: CornerType::Round };
    Object::new(doc.new_id(), ObjKind::Rect(shape), Affine::IDENTITY, Style::default())
}

fn fill(image: u64) -> PatternFill {
    PatternFill { kind: PatternKind::Checker, fg: Color::BLACK, bg: Color::WHITE, tile: 20.0, angle: 0.0, image: Some(image), transform_with_object: true, vector: true, mirror: 0, source: None }
}

fn image(doc: &mut Document, w: u32, h: u32) -> u64 {
    doc.add_image(ImageData { width: w, height: h, rgba: Arc::new(vec![0; (w * h * 4) as usize]), encoded: Arc::new(vec![]), format: "png".into(), name: "tile".into() })
}

#[test]
fn fills_saved_without_a_source_load_unchanged() {
    // a pattern fill as files written before sources existed have it
    let json = r#"{"kind":"Checker","fg":{"r":0,"g":0,"b":0,"a":255},"bg":{"r":255,"g":255,"b":255,"a":255},"tile":36.0,"angle":0.0,"image":7,"transform_with_object":true,"vector":true,"mirror":0}"#;
    let old: PatternFill = serde_json::from_str(json).unwrap();
    assert_eq!(old, PatternFill { tile: 36.0, ..fill(7) });
    // a fill without a source writes no source, so older versions read it as before
    let text = serde_json::to_string(&fill(7)).unwrap();
    assert!(!text.contains("source"), "{text}");
    // a document without tiles loads with none
    let mut v = serde_json::to_value(Document::new(100.0, 100.0)).unwrap();
    v.as_object_mut().unwrap().remove("pattern_tiles");
    let doc: Document = serde_json::from_value(v).unwrap();
    assert!(doc.pattern_tiles.is_empty());
}

#[test]
fn sources_round_trip_through_json() {
    let mut doc = Document::new(100.0, 100.0);
    let o = square(&mut doc);
    let t = doc.add_pattern_tile(PatternTile { name: "Squares".into(), objects: vec![o], width: 10.0, height: 5.0, background: Some(Color::WHITE), revision: 3 });
    for source in [
        PatternSource::Tile(t),
        PatternSource::Texture { style: "Sky 2 Colors".into(), kinds: "nnnccb".into(), params: vec![29335, 0, 50, 0, 44, 100, 100, 100, 100, 0] },
        PatternSource::PostScript { number: 12, params: vec![1, -2, 300] },
    ] {
        let mut o = square(&mut doc);
        o.style.fill = Fill::Pattern(PatternFill { source: Some(source), ..fill(1) });
        doc.pages[0].layers[0].objects.push(o);
    }
    let back: Document = serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
    assert_eq!(back.pattern_tiles, doc.pattern_tiles);
    assert_eq!(back.pages[0].layers[0].objects, doc.pages[0].layers[0].objects);
}

#[test]
fn texture_settings_split_by_kind() {
    let s = PatternSource::texture_settings("ncb", &[5, 10, 20, 30, -40]);
    assert_eq!(s, vec![(b'n', vec![5]), (b'c', vec![10, 20, 30]), (b'b', vec![-40])]);
    // settings the list is too short for are left out
    assert_eq!(PatternSource::texture_settings("nc", &[1, 2]).len(), 1);
}

#[test]
fn every_fill_using_a_tile_is_found() {
    let mut doc = Document::new(100.0, 100.0);
    let (a, b) = (image(&mut doc, 4, 4), image(&mut doc, 4, 4));
    let t = doc.add_pattern_tile(PatternTile { name: String::new(), objects: vec![], width: 10.0, height: 10.0, background: None, revision: 0 });
    let tiled = |img| PatternFill { source: Some(PatternSource::Tile(t)), ..fill(img) };
    let mut o1 = square(&mut doc);
    o1.style.fill = Fill::Pattern(tiled(a));
    let mut o2 = square(&mut doc);
    o2.style.transparency = vector_core::style::Transparency::Pattern { pattern: tiled(b), front: 0.0, back: 50.0 };
    let mut o3 = square(&mut doc);
    o3.style.fill = Fill::Pattern(fill(a));
    doc.pages[0].layers[0].objects = vec![o1, o2, o3];
    assert_eq!(doc.tile_images(t), vec![a, b]);
    // the transparency's picture is kept when the document is saved
    assert!(doc.referenced_images().contains(&b));
}

#[test]
fn old_vector_patterns_move_their_objects_into_a_tile() {
    let mut doc = Document::new(100.0, 100.0);
    let img = image(&mut doc, 40, 20);
    let objects = vec![square(&mut doc), square(&mut doc)];
    doc.patterns.push(NamedPattern { name: "Old".into(), fill: fill(img), source: objects.clone() });
    let mut o = square(&mut doc);
    o.style.fill = Fill::Pattern(fill(img));
    doc.pages[0].layers[0].objects.push(o);
    doc.upgrade_pattern_tiles();
    let Some(PatternSource::Tile(t)) = doc.patterns[0].fill.source.clone() else { panic!("no tile") };
    assert!(doc.patterns[0].source.is_empty());
    let tile = &doc.pattern_tiles[&t];
    assert_eq!(tile.objects, objects);
    assert_eq!((tile.width, tile.height), (20.0, 10.0));
    match &doc.pages[0].layers[0].objects[0].style.fill {
        Fill::Pattern(p) => assert_eq!(p.source, Some(PatternSource::Tile(t))),
        f => panic!("{f:?}"),
    }
    // done once
    doc.upgrade_pattern_tiles();
    assert_eq!(doc.pattern_tiles.len(), 1);
}
