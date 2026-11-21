//! The page border on the desktop (#141) and bitmaps in Wireframe view (#129).

use kurbo::Affine;
use std::sync::Arc;
use vector_core::color::Color;
use vector_core::document::ImageData;
use vector_core::object::{ImageObj, ObjKind, Object};
use vector_core::style::Style;
use vector_core::Document;
use vector_render::{RenderOptions, Renderer};

fn rgb(p: &tiny_skia::Pixmap, x: u32, y: u32) -> [u8; 3] {
    let c = p.pixel(x, y).unwrap();
    [c.red(), c.green(), c.blue()]
}

fn lum(c: [u8; 3]) -> f64 {
    0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64
}

/// A 100 × 80 pt page drawn at `zoom` on a 200 × 200 px window, 40 px in from the top left.
fn page_on_desktop(desktop: Color, page: Option<Color>, zoom: f64, border: bool) -> tiny_skia::Pixmap {
    let mut doc = Document::new(100.0, 80.0);
    doc.pages[0].background = page;
    let mut pm = tiny_skia::Pixmap::new(200 + (200.0 * zoom) as u32, 200 + (200.0 * zoom) as u32).unwrap();
    let opts = RenderOptions { desktop, page_border: border, ..Default::default() };
    Renderer::new().render_page(&doc, 0, Affine::translate((40.0, 40.0)) * Affine::scale(zoom), &mut pm, &opts);
    pm
}

#[test]
fn white_page_on_a_white_desktop_has_a_border_all_around() {
    for zoom in [1.0, 2.0] {
        let p = page_on_desktop(Color::WHITE, None, zoom, true);
        let (x1, y1) = (40 + (100.0 * zoom) as u32, 40 + (80.0 * zoom) as u32);
        // the line runs just outside the page on all four sides, the left and top too (where no
        // shadow falls)
        for (x, y) in [(39, 60), (60, 39), (x1, 60), (60, y1)] {
            assert!(lum(rgb(&p, x, y)) < 200.0, "zoom {zoom}: no border at ({x}, {y}): {:?}", rgb(&p, x, y));
        }
        // inside the page and out on the desktop it stays white
        assert_eq!(rgb(&p, 41, 41), [255; 3], "zoom {zoom}");
        assert_eq!(rgb(&p, 60, 60), [255; 3]);
        assert_eq!(rgb(&p, 20, 20), [255; 3], "the border is drawn out on the desktop");
        assert_eq!(rgb(&p, x1 + 20, y1 + 20), [255; 3]);
    }
}

#[test]
fn dark_page_on_a_dark_desktop_has_a_light_border() {
    let dark = Color::rgb(0x20, 0x20, 0x20);
    let p = page_on_desktop(dark, Some(Color::rgb(0x18, 0x18, 0x18)), 1.0, true);
    assert!(lum(rgb(&p, 39, 60)) > 70.0, "{:?}", rgb(&p, 39, 60));
    assert!(lum(rgb(&p, 60, 39)) > 70.0);
}

#[test]
fn page_border_turns_off_with_its_shadow() {
    let p = page_on_desktop(Color::WHITE, None, 1.0, false);
    // View ▸ Page ▸ Page Border off: no line and no shadow, the page is still filled
    for (x, y) in [(39, 60), (60, 39), (140, 60), (142, 60), (60, 122)] {
        assert_eq!(rgb(&p, x, y), [255; 3], "({x}, {y})");
    }
    let p = page_on_desktop(Color::rgb(0x40, 0x40, 0x40), None, 1.0, false);
    assert_eq!(rgb(&p, 60, 60), [255; 3]);
    assert_eq!(rgb(&p, 39, 60), [0x40; 3]);
}

/// A 40 × 40 px bitmap, dark red on the left half and blue on the right, as a 100 × 100 pt
/// object at (20, 20) on a white page.
fn bitmap_doc(transform: Affine, crop: [f64; 4]) -> Document {
    let mut doc = Document::new(200.0, 200.0);
    let (w, h) = (40u32, 40u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for _y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(if x < w / 2 { &[150, 0, 0, 255] } else { &[0, 0, 200, 255] });
        }
    }
    let data = ImageData { width: w, height: h, rgba: Arc::new(rgba), encoded: Arc::new(vec![]), format: "png".into(), name: "test".into() };
    let image = doc.add_image(data);
    let kind = ObjKind::Image(ImageObj { image, w: 100.0, h: 100.0, crop, adjust: Default::default() });
    let o = Object::new(doc.new_id(), kind, transform, Style::default());
    doc.pages[0].layers[0].objects.push(o);
    doc
}

fn render(r: &mut Renderer, doc: &Document, wireframe: bool) -> tiny_skia::Pixmap {
    let mut pm = tiny_skia::Pixmap::new(200, 200).unwrap();
    let opts = RenderOptions { wireframe, desktop: Color::WHITE, ..Default::default() };
    r.render_page(doc, 0, Affine::IDENTITY, &mut pm, &opts);
    pm
}

/// A dark hairline on the pixel column at `x` or the one left of it.
fn frame_at(p: &tiny_skia::Pixmap, x: u32, y: u32) -> bool {
    lum(rgb(p, x - 1, y)).min(lum(rgb(p, x, y))) < 160.0
}

#[test]
fn wireframe_shows_a_bitmap_as_a_light_gray_ghost_with_its_frame() {
    let doc = bitmap_doc(Affine::translate((20.0, 20.0)), [0.0, 0.0, 1.0, 1.0]);
    let mut r = Renderer::new();
    let normal = render(&mut r, &doc, false);
    let wire = render(&mut r, &doc, true);
    for (x, y) in [(40, 70), (100, 70)] {
        let (n, w) = (rgb(&normal, x, y), rgb(&wire, x, y));
        // not empty: something of the bitmap shows
        assert!(w != [255; 3], "({x}, {y}) is empty in wireframe");
        // gray, and much lighter than the bitmap itself
        assert!(w[0] == w[1] && w[1] == w[2], "({x}, {y}) is not gray: {w:?}");
        assert!(lum(w) > lum(n) + 60.0 && lum(w) > 150.0, "({x}, {y}) is not a ghost: normal {n:?}, wireframe {w:?}");
    }
    // the two halves stay apart, so the picture can still be made out
    assert_ne!(rgb(&wire, 40, 70), rgb(&wire, 100, 70));
    // the frame is still drawn around it, and outside it the page is white
    assert!(frame_at(&wire, 20, 70), "no frame: {:?} {:?}", rgb(&wire, 19, 70), rgb(&wire, 20, 70));
    assert_eq!(rgb(&wire, 10, 70), [255; 3]);
    assert_eq!(rgb(&wire, 130, 70), [255; 3]);
}

#[test]
fn wireframe_ghost_follows_crop_and_rotation() {
    // cropped to the left (red) half: the whole object is the red half's ghost
    let doc = bitmap_doc(Affine::translate((20.0, 20.0)), [0.0, 0.0, 0.5, 1.0]);
    let mut r = Renderer::new();
    let wire = render(&mut r, &doc, true);
    let red_ghost = rgb(&wire, 40, 70);
    assert_eq!(rgb(&wire, 100, 70), red_ghost, "the crop is not followed");
    // turned a quarter around its corner at (120, 20): it now covers x 20..120 below y 20 to
    // the left of the pivot, the left (red) half at the top
    let turned = bitmap_doc(Affine::translate((120.0, 20.0)) * Affine::rotate(std::f64::consts::FRAC_PI_2), [0.0, 0.0, 1.0, 1.0]);
    let wire = render(&mut r, &turned, true);
    let top = rgb(&wire, 70, 40);
    let bottom = rgb(&wire, 70, 100);
    assert!(top != [255; 3] && bottom != [255; 3]);
    assert_ne!(top, bottom, "the rotation is not followed");
    assert_eq!(top, red_ghost);
}

#[test]
fn wireframe_of_a_missing_bitmap_still_shows_its_frame() {
    let mut doc = bitmap_doc(Affine::translate((20.0, 20.0)), [0.0, 0.0, 1.0, 1.0]);
    doc.images.clear();
    let wire = render(&mut Renderer::new(), &doc, true);
    assert!(frame_at(&wire, 20, 70));
    assert_eq!(rgb(&wire, 70, 70), [255; 3]);
}
