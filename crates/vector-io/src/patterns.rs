//! Pattern tiles drawn again from what they are made of.
//!
//! A pattern fill always carries a picture of its tile (`PatternFill::image`), which is what
//! the renderer and the exports use. When the fill also knows its source (the tile's objects,
//! or a CorelDRAW texture's settings), these functions draw that picture again after the
//! source changes.

use std::sync::Arc;
use vector_core::document::{Document, ImageData, PatternTile};
use vector_core::style::PatternSource;

/// The resolution a changed tile is drawn at: 600 dpi, at most `MAX_SIDE` pixels on a side.
const DPI: f64 = 600.0;
const MAX_SIDE: f64 = 4096.0;

/// Draw a tile's objects into pixels of `size` (RGBA, not premultiplied), with `doc`'s images
/// and pattern tiles available to them.
pub fn tile_pixels(doc: &Document, tile: &PatternTile, size: (u32, u32)) -> Option<(u32, u32, Vec<u8>)> {
    if tile.width <= 0.0 || tile.height <= 0.0 {
        return None;
    }
    let mut d = Document::new(tile.width, tile.height);
    d.images = doc.images.clone();
    d.pattern_tiles = doc.pattern_tiles.clone();
    if let Some(l) = d.pages[0].layers.first_mut() {
        l.objects = tile.objects.clone();
    }
    let opts = crate::image_io::RasterOptions { size: Some(size), background: tile.background, ..Default::default() };
    crate::image_io::render_rgba(&d, 0, &opts)
}

/// The pixel size a changed tile is drawn at.
pub fn tile_size(tile: &PatternTile) -> (u32, u32) {
    let scale = (DPI / 72.0).min(MAX_SIDE / tile.width.max(tile.height).max(1.0));
    ((tile.width * scale).round().max(1.0) as u32, (tile.height * scale).round().max(1.0) as u32)
}

/// Put new objects into a pattern tile: the tile's revision goes up and the picture of every
/// fill that uses the tile is drawn again. `false` when there is no such tile or it can't be
/// drawn.
pub fn set_tile_objects(doc: &mut Document, id: u64, objects: Vec<vector_core::object::Object>) -> bool {
    let Some(t) = doc.pattern_tiles.get_mut(&id) else { return false };
    t.objects = objects;
    t.revision += 1;
    redraw_tile(doc, id)
}

/// Draw a pattern tile again into the image of every fill that uses it.
pub fn redraw_tile(doc: &mut Document, id: u64) -> bool {
    let Some(tile) = doc.pattern_tiles.get(&id) else { return false };
    let Some((w, h, rgba)) = tile_pixels(doc, tile, tile_size(tile)) else { return false };
    let name = tile.name.clone();
    let encoded = Arc::new(crate::image_io::encode_png(w, h, &rgba));
    let rgba = Arc::new(rgba);
    let images = doc.tile_images(id);
    for img in images {
        let name = doc.images.get(&img).map(|i| i.name.clone()).unwrap_or_else(|| name.clone());
        doc.images.insert(img, ImageData { width: w, height: h, rgba: rgba.clone(), encoded: encoded.clone(), format: "png".into(), name });
    }
    true
}

/// A texture fill's tile drawn from its recipe (`PatternSource::Texture`): a new image for the
/// document's image store. `None` for another source, or settings that draw nothing.
pub fn texture_image(source: &PatternSource) -> Option<ImageData> {
    let PatternSource::Texture { kinds, params, .. } = source else { return None };
    let (rgba, w, h) = crate::corel::texture_tile(kinds, params)?;
    let encoded = crate::image_io::encode_png(w, h, &rgba);
    Some(ImageData { width: w, height: h, rgba: Arc::new(rgba), encoded: Arc::new(encoded), format: "png".into(), name: "texture fill".into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Affine;
    use vector_core::color::Color;
    use vector_core::object::{ObjKind, Object, RectShape};
    use vector_core::style::{Fill, PatternFill, PatternKind, Style, Transparency};

    fn square(doc: &mut Document, color: Color) -> Object {
        let mut st = Style::default();
        st.fill = Fill::Solid(color);
        st.outline = vector_core::style::Outline::none();
        let shape = RectShape { w: 10.0, h: 10.0, radii: [0.0; 4], corner: vector_core::object::CornerType::Round };
        Object::new(doc.new_id(), ObjKind::Rect(shape), Affine::IDENTITY, st)
    }

    fn fill(image: u64, tile: u64) -> PatternFill {
        PatternFill { kind: PatternKind::Checker, fg: Color::BLACK, bg: Color::WHITE, tile: 10.0, angle: 0.0, image: Some(image), transform_with_object: true, vector: true, mirror: 0, source: Some(PatternSource::Tile(tile)) }
    }

    /// A document with one red tile, used by a fill, a transparency and a named pattern, two of
    /// them through a second copy of the picture.
    fn shared() -> (Document, u64, u64, u64) {
        let mut doc = Document::new(100.0, 100.0);
        let red = square(&mut doc, Color::rgb(255, 0, 0));
        let tile = doc.add_pattern_tile(PatternTile { name: "Red".into(), objects: vec![red], width: 10.0, height: 10.0, background: None, revision: 0 });
        let blank = ImageData { width: 1, height: 1, rgba: Arc::new(vec![0; 4]), encoded: Arc::new(vec![]), format: "png".into(), name: "tile".into() };
        let a = doc.add_image(blank.clone());
        let b = doc.add_image(blank);
        let mut o1 = square(&mut doc, Color::BLACK);
        o1.style.fill = Fill::Pattern(fill(a, tile));
        let mut o2 = square(&mut doc, Color::BLACK);
        o2.style.transparency = Transparency::Pattern { pattern: fill(b, tile), front: 0.0, back: 100.0 };
        doc.pages[0].layers[0].objects = vec![o1, o2];
        doc.patterns.push(vector_core::document::NamedPattern { name: "Red".into(), fill: fill(a, tile), source: vec![] });
        (doc, tile, a, b)
    }

    fn first_pixel(doc: &Document, img: u64) -> [u8; 4] {
        let im = &doc.images[&img];
        [im.rgba[0], im.rgba[1], im.rgba[2], im.rgba[3]]
    }

    #[test]
    fn editing_a_shared_tile_redraws_every_fill_that_uses_it() {
        let (mut doc, tile, a, b) = shared();
        assert_eq!(doc.tile_images(tile), vec![a, b]);
        let blue = square(&mut doc, Color::rgb(0, 0, 255));
        assert!(set_tile_objects(&mut doc, tile, vec![blue]));
        assert_eq!(doc.pattern_tiles[&tile].revision, 1);
        for img in [a, b] {
            assert_eq!(first_pixel(&doc, img), [0, 0, 255, 255], "image {img}");
            assert_eq!((doc.images[&img].width, doc.images[&img].height), tile_size(&doc.pattern_tiles[&tile]));
            assert!(!doc.images[&img].encoded.is_empty());
        }
        // another edit counts again
        let green = square(&mut doc, Color::rgb(0, 255, 0));
        assert!(set_tile_objects(&mut doc, tile, vec![green]));
        assert_eq!(doc.pattern_tiles[&tile].revision, 2);
        assert_eq!(first_pixel(&doc, a), [0, 255, 0, 255]);
        assert!(!set_tile_objects(&mut doc, 999_999, vec![]));
    }

    #[test]
    fn a_tile_keeps_its_background() {
        let mut doc = Document::new(100.0, 100.0);
        let tile = PatternTile { name: String::new(), objects: vec![], width: 4.0, height: 2.0, background: Some(Color::WHITE), revision: 0 };
        let (w, h, rgba) = tile_pixels(&doc, &tile, (8, 4)).unwrap();
        assert_eq!((w, h), (8, 4));
        assert_eq!(&rgba[..4], &[255, 255, 255, 255]);
        let id = doc.add_pattern_tile(tile);
        assert_eq!(doc.tile_images(id), Vec::<u64>::new());
    }

    #[test]
    fn a_tile_and_its_fills_round_trip_through_the_own_format() {
        let (doc, tile, a, _) = shared();
        let bytes = crate::cdr::save_to_bytes(&doc, &Default::default()).unwrap();
        let back = crate::cdr::load_from_bytes(&bytes).unwrap();
        assert_eq!(back.pattern_tiles, doc.pattern_tiles);
        let o = &back.pages[0].layers[0].objects[0];
        match &o.style.fill {
            Fill::Pattern(p) => assert_eq!(p.source, Some(PatternSource::Tile(tile))),
            f => panic!("{f:?}"),
        }
        // the transparency's picture is saved too
        assert_eq!(back.tile_images(tile).len(), 2);
        assert!(back.images.contains_key(&a));
        // new ids stay clear of the tile's
        let mut back = back;
        assert!(back.new_id() > tile);
    }

    #[test]
    fn texture_recipes_draw_a_tile() {
        let src = PatternSource::Texture { style: "Cosmic Clouds".into(), kinds: "nnnnnb".into(), params: vec![9, 0, 0, 0, 0, 0] };
        let im = texture_image(&src).unwrap();
        assert!(im.width > 0 && im.rgba.len() == (im.width * im.height * 4) as usize);
        assert!(texture_image(&PatternSource::PostScript { number: 1, params: vec![] }).is_none());
    }
}
