use super::*;
use crate::pdf::{export_bytes, PdfColor, PdfCompat, PdfOptions, PdfSecurity};
use kurbo::{Point, Shape};
use lopdf::{dictionary, Stream};
use vector_core::color::Color;
use vector_core::object::{CornerType, ImageAdjust, ImageObj, RectShape};
use vector_core::style::{Fill, Gradient, Transparency};

fn rect(doc: &mut Document, x: f64, y: f64, w: f64, h: f64, fill: Color) -> VObj {
    let id = doc.new_id();
    let mut o = VObj::new(id, ObjKind::Rect(RectShape { w, h, radii: [0.0; 4], corner: CornerType::default() }), Affine::translate((x, y)), Style::filled(fill));
    o.style.outline = Outline::none();
    o
}

fn all(objs: &[VObj]) -> Vec<&VObj> {
    let mut v = vec![];
    for o in objs {
        o.walk(&mut |x| v.push(x));
    }
    v
}

fn paths(doc: &Document, page: usize) -> Vec<&VObj> {
    all(&doc.pages[page].layers[0].objects).into_iter().filter(|o| matches!(o.kind, ObjKind::Path(_))).collect()
}

fn opts() -> PdfOptions {
    PdfOptions { compress: true, id_seed: Some(1), ..Default::default() }
}

fn close(a: Rect, b: Rect) -> bool {
    (a.x0 - b.x0).abs() < 0.05 && (a.y0 - b.y0).abs() < 0.05 && (a.x1 - b.x1).abs() < 0.05 && (a.y1 - b.y1).abs() < 0.05
}

/// A PDF built with lopdf: pages of (size, content, resources).
fn build_pdf(pages: Vec<((f64, f64), &str, lopdf::Dictionary)>, extra: impl FnOnce(&mut lopdf::Document, &mut Vec<lopdf::ObjectId>)) -> Vec<u8> {
    let mut d = lopdf::Document::with_version("1.7");
    let pages_id = d.new_object_id();
    let mut kids = vec![];
    for ((w, h), content, res) in pages {
        let c = d.add_object(Stream::new(dictionary! {}, content.as_bytes().to_vec()));
        let r = d.add_object(res);
        let p = d.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id, "MediaBox" => vec![0.into(), 0.into(), w.into(), h.into()], "Contents" => c, "Resources" => r });
        kids.push(p);
    }
    extra(&mut d, &mut kids);
    let count = kids.len() as i64;
    d.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids.into_iter().map(Object::Reference).collect::<Vec<_>>(), "Count" => count }));
    let cat = d.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    d.trailer.set("Root", cat);
    let mut out = vec![];
    d.save_to(&mut out).unwrap();
    out
}

#[test]
fn round_trip_geometry_and_colors() {
    let mut doc = Document::new(300.0, 200.0);
    let mut a = rect(&mut doc, 10.0, 20.0, 100.0, 50.0, Color::rgb(200, 30, 30));
    a.style.outline = Outline { width: 2.0, color: Some(Color::rgb(0, 0, 255)), ..Outline::default() };
    let b = rect(&mut doc, 150.0, 100.0, 40.0, 60.0, Color::rgb(10, 200, 90));
    let (ba, bb) = (a.bounds(), b.bounds());
    doc.pages[0].layers[0].objects = vec![a, b];
    let bytes = export_bytes(&doc, &opts()).unwrap();
    let (back, notes) = load_with_notes(&bytes, &PdfImportOptions::default()).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(back.pages.len(), 1);
    assert_eq!((back.pages[0].width, back.pages[0].height), (300.0, 200.0));
    let ps = paths(&back, 0);
    assert_eq!(ps.len(), 2, "fill and stroke of the first rectangle make one object");
    let world = |o: &VObj| o.world_path().bounding_box();
    assert!(close(world(ps[0]), ba), "{:?} vs {:?}", world(ps[0]), ba);
    assert!(close(world(ps[1]), bb));
    assert_eq!(ps[0].style.fill, Fill::Solid(Color::rgb(200, 30, 30)));
    assert_eq!(ps[0].style.outline.color, Some(Color::rgb(0, 0, 255)));
    assert!((ps[0].style.outline.width - 2.0).abs() < 1e-6);
    assert_eq!(ps[1].style.fill, Fill::Solid(Color::rgb(10, 200, 90)));
    assert!(ps[1].style.outline.color.is_none());
}

#[test]
fn cmyk_comes_back_as_the_same_inks() {
    let mut doc = Document::new(200.0, 100.0);
    let colors = [Color::from_cmyk(0.0, 0.0, 0.0, 100.0), Color::from_cmyk(100.0, 0.0, 0.0, 0.0), Color::from_cmyk(0.0, 100.0, 100.0, 0.0), Color::from_cmyk(0.0, 0.0, 100.0, 0.0)];
    let objs: Vec<VObj> = colors.iter().enumerate().map(|(i, c)| rect(&mut doc, 10.0 + i as f64 * 40.0, 10.0, 30.0, 30.0, *c)).collect();
    doc.pages[0].layers[0].objects = objs;
    let pdf_opts = PdfOptions { color: PdfColor::Cmyk, compress: false, ..opts() };
    let bytes = export_bytes(&doc, &pdf_opts).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("0 0 0 1 k"));
    let (back, _) = load_with_notes(&bytes, &PdfImportOptions::default()).unwrap();
    let got: Vec<Fill> = paths(&back, 0).iter().map(|o| o.style.fill.clone()).collect();
    assert_eq!(got, colors.iter().map(|c| Fill::Solid(*c)).collect::<Vec<_>>());
    // and they go out again as the same CMYK values
    let again = String::from_utf8_lossy(&export_bytes(&back, &pdf_opts).unwrap()).into_owned();
    for k in ["0 0 0 1 k", "1 0 0 0 k", "0 1 1 0 k", "0 0 1 0 k"] {
        assert!(again.contains(k), "{k}");
    }
    // a CMYK file that wasn't written by AnyVector
    let bytes = build_pdf(vec![((100.0, 100.0), "0.1 0.2 0.3 0.4 k 0 0 50 50 re f", dictionary! {})], |_, _| {});
    let (back, _) = load_with_notes(&bytes, &PdfImportOptions::default()).unwrap();
    assert_eq!(paths(&back, 0)[0].style.fill, Fill::Solid(Color::from_cmyk(10.0, 20.0, 30.0, 40.0)));
}

#[test]
fn every_page_or_the_chosen_ones() {
    let mut doc = Document::new(200.0, 100.0);
    doc.add_page(1, 300.0, 400.0);
    doc.add_page(2, 500.0, 250.0);
    for p in 0..3 {
        let o = rect(&mut doc, 10.0, 10.0, 20.0 + p as f64 * 10.0, 20.0, Color::rgb(0, 0, 0));
        doc.pages[p].layers[0].objects.push(o);
    }
    let bytes = export_bytes(&doc, &opts()).unwrap();
    assert_eq!(page_sizes(&bytes).unwrap(), vec![(200.0, 100.0), (300.0, 400.0), (500.0, 250.0)]);
    let (back, _) = load_with_notes(&bytes, &PdfImportOptions::default()).unwrap();
    assert_eq!(back.pages.len(), 3);
    for p in 0..3 {
        assert_eq!((back.pages[p].width, back.pages[p].height), (doc.pages[p].width, doc.pages[p].height));
        let w = paths(&back, p)[0].world_path().bounding_box().width();
        assert!((w - (20.0 + p as f64 * 10.0)).abs() < 0.01);
    }
    let (one, _) = load_with_notes(&bytes, &PdfImportOptions { pages: vec![2], ..Default::default() }).unwrap();
    assert_eq!(one.pages.len(), 1);
    assert_eq!(one.pages[0].width, 500.0);
    // import: one group per page, with the page sizes
    let mut target = Document::new(100.0, 100.0);
    let (pages, _) = import_pages(&mut target, &bytes, &PdfImportOptions { pages: vec![0, 2], ..Default::default() }).unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[1].1, (500.0, 250.0));
    assert_eq!(pages[0].0.len(), 1);
}

#[test]
fn transparency_gradients_and_images() {
    let mut doc = Document::new(200.0, 200.0);
    let a = rect(&mut doc, 10.0, 10.0, 50.0, 50.0, Color::rgba(255, 0, 0, 128));
    let mut b = rect(&mut doc, 70.0, 10.0, 50.0, 50.0, Color::rgb(0, 0, 255));
    b.style.transparency = Transparency::Uniform(50.0);
    let mut c = rect(&mut doc, 10.0, 70.0, 100.0, 50.0, Color::BLACK);
    c.style.fill = Fill::Gradient(Gradient::linear(Color::rgb(255, 0, 0), Color::rgb(0, 0, 255)));
    let rgba: Vec<u8> = (0..4 * 3 * 2).map(|i| if i % 4 == 3 { 255 } else { (i * 20) as u8 }).collect();
    let img = vector_core::document::ImageData { width: 3, height: 2, rgba: std::sync::Arc::new(rgba.clone()), encoded: std::sync::Arc::new(vec![]), format: "png".into(), name: "x".into() };
    let iid = doc.add_image(img);
    let id = doc.new_id();
    let mut st = Style::default();
    st.outline = Outline::none();
    let im = VObj::new(id, ObjKind::Image(ImageObj { image: iid, w: 30.0, h: 20.0, crop: [0.0, 0.0, 1.0, 1.0], adjust: ImageAdjust::default() }), Affine::translate((130.0, 130.0)), st);
    let im_bounds = im.bounds();
    doc.pages[0].layers[0].objects = vec![a, b, c, im];
    let bytes = export_bytes(&doc, &PdfOptions { compat: PdfCompat::Acrobat9, ..opts() }).unwrap();
    let (back, notes) = load_with_notes(&bytes, &PdfImportOptions::default()).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    let objs = all(&back.pages[0].layers[0].objects);
    let ps: Vec<&&VObj> = objs.iter().filter(|o| matches!(o.kind, ObjKind::Path(_))).collect();
    assert_eq!(ps[0].style.fill, Fill::Solid(Color::rgba(255, 0, 0, 128)));
    assert_eq!(ps[1].style.transparency, Transparency::Uniform(50.0));
    let Fill::Gradient(g) = &ps[2].style.fill else { panic!("{:?}", ps[2].style.fill) };
    let stops = g.sorted_stops();
    assert_eq!(stops.first().unwrap().color, Color::rgb(255, 0, 0));
    assert_eq!(stops.last().unwrap().color, Color::rgb(0, 0, 255));
    assert!(g.start.x < 0.05 && g.end.x > 0.95 && (g.start.y - g.end.y).abs() < 1e-6, "{:?} {:?}", g.start, g.end);
    let im = objs.iter().find(|o| matches!(o.kind, ObjKind::Image(_))).expect("image");
    assert!(close(im.bounds(), im_bounds), "{:?} {:?}", im.bounds(), im_bounds);
    let ObjKind::Image(io) = &im.kind else { unreachable!() };
    let data = &back.images[&io.image];
    assert_eq!((data.width, data.height), (3, 2));
    assert_eq!(data.rgba.as_slice(), rgba.as_slice());
}

#[test]
fn encrypted_with_an_empty_user_password() {
    let mut doc = Document::new(100.0, 100.0);
    let o = rect(&mut doc, 10.0, 10.0, 20.0, 20.0, Color::rgb(1, 2, 3));
    doc.pages[0].layers[0].objects.push(o);
    let sec = PdfSecurity { open_password: String::new(), permission_password: "owner".into(), printing: 2, editing: 0, copying: false };
    let bytes = export_bytes(&doc, &PdfOptions { security: Some(sec), ..opts() }).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("/Encrypt"));
    let (back, _) = load_with_notes(&bytes, &PdfImportOptions::default()).unwrap();
    assert_eq!(paths(&back, 0)[0].style.fill, Fill::Solid(Color::rgb(1, 2, 3)));
    // a password to open it: refused with a clear message
    let sec = PdfSecurity { open_password: "secret".into(), permission_password: "owner".into(), printing: 2, editing: 0, copying: false };
    let bytes = export_bytes(&doc, &PdfOptions { security: Some(sec), ..opts() }).unwrap();
    let err = load_with_notes(&bytes, &PdfImportOptions::default()).err().expect("needs a password");
    assert!(err.to_string().contains("password"), "{err}");
}

fn helvetica() -> lopdf::Dictionary {
    dictionary! { "Font" => dictionary! { "F1" => dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" } } }
}

#[test]
fn text_with_an_installed_font_is_editable() {
    let bytes = build_pdf(vec![((400.0, 200.0), "BT /F1 24 Tf 1 0 0 1 50 100 Tm 0 0 1 rg (Hello) Tj [(W) 30 (orld) -3000 (Again)] TJ ET", helvetica())], |_, _| {});
    let (doc, _) = load_with_notes(&bytes, &PdfImportOptions::default()).unwrap();
    let texts: Vec<&VObj> = all(&doc.pages[0].layers[0].objects).into_iter().filter(|o| matches!(o.kind, ObjKind::Text(_))).collect();
    assert_eq!(texts.len(), 2, "a wide gap starts a new text");
    let ObjKind::Text(t) = &texts[0].kind else { unreachable!() };
    assert_eq!(t.content, "HelloWorld");
    assert_eq!(t.format.font, "Arial");
    assert!((t.format.size - 24.0).abs() < 1e-9);
    assert_eq!(texts[0].style.fill, Fill::Solid(Color::rgb(0, 0, 255)));
    // the baseline sits where the PDF puts it: 100 pt up from the bottom of a 200 pt page
    let origin = texts[0].transform * Point::ORIGIN;
    assert!((origin.x - 50.0).abs() < 1e-6 && (origin.y - 100.0).abs() < 1e-6, "{origin:?}");
    let ObjKind::Text(t2) = &texts[1].kind else { unreachable!() };
    assert_eq!(t2.content, "Again");
    // a word gap made of positioning becomes a space
    let bytes = build_pdf(vec![((400.0, 200.0), "BT /F1 10 Tf 50 100 Td [(one) -400 (two)] TJ ET", helvetica())], |_, _| {});
    let (doc, _) = load_with_notes(&bytes, &PdfImportOptions::default()).unwrap();
    let t: Vec<String> = all(&doc.pages[0].layers[0].objects).iter().filter_map(|o| if let ObjKind::Text(t) = &o.kind { Some(t.content.clone()) } else { None }).collect();
    assert_eq!(t, ["one two"]);
}

#[test]
fn text_as_curves() {
    if vector_core::text::fonts().lock().resolve("Arial").substituted() {
        return; // no Arial here to draw with
    }
    let bytes = build_pdf(vec![((400.0, 200.0), "BT /F1 24 Tf 50 100 Td (Hi) Tj ET", helvetica())], |_, _| {});
    let (doc, notes) = load_with_notes(&bytes, &PdfImportOptions { text_as_curves: true, ..Default::default() }).unwrap();
    let ps = paths(&doc, 0);
    assert_eq!(ps.len(), 1);
    let bb = ps[0].world_path().bounding_box();
    assert!(bb.x0 >= 50.0 && bb.x0 < 54.0 && bb.y1 <= 100.5 && bb.height() > 15.0, "{bb:?}");
    assert!(notes.iter().any(|n| n.contains("Helvetica")), "{notes:?}");
}

#[test]
fn clips_become_powerclips_when_they_cut() {
    let content = "q 0 0 100 100 re W n 1 0 0 rg 10 10 20 20 re f Q q 50 50 m 90 50 l 70 90 l h W n 0 0 1 rg 40 40 60 60 re f Q";
    let bytes = build_pdf(vec![((200.0, 200.0), content, dictionary! {})], |_, _| {});
    let (doc, _) = load_with_notes(&bytes, &PdfImportOptions::default()).unwrap();
    let top = &doc.pages[0].layers[0].objects;
    assert_eq!(top.len(), 2);
    assert!(matches!(top[0].kind, ObjKind::Path(_)), "a clip around the whole object is left out");
    let ObjKind::PowerClip { frame, contents } = &top[1].kind else { panic!("{:?}", top[1].kind.type_name()) };
    assert_eq!(contents.len(), 1);
    let fb = frame.world_path().bounding_box();
    assert!(close(fb, Rect::new(50.0, 110.0, 90.0, 150.0)), "{fb:?}");
}

#[test]
fn forms_shadings_spots_and_rotation() {
    let content = "q 2 0 0 2 0 0 cm /Fm1 Do Q /CS0 cs 0.5 scn 0 0 10 10 re f q 0 0 50 50 re W n /Sh1 sh Q";
    let bytes = build_pdf(vec![((100.0, 50.0), content, dictionary! {})], |d, kids| {
        let form = d.add_object(Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 20.into(), 20.into()] }, b"0 1 0 rg 1 1 5 5 re f".to_vec()));
        let tint = d.add_object(dictionary! { "FunctionType" => 2, "Domain" => vec![0.into(), 1.into()], "C0" => vec![0.into(), 0.into(), 0.into(), 0.into()], "C1" => vec![0.into(), 1.into(), 0.into(), 0.into()], "N" => 1 });
        let sh = d.add_object(dictionary! {
            "ShadingType" => 3, "ColorSpace" => "DeviceRGB", "Coords" => vec![25.into(), 25.into(), 0.into(), 25.into(), 25.into(), 25.into()],
            "Function" => dictionary! { "FunctionType" => 2, "Domain" => vec![0.into(), 1.into()], "C0" => vec![1.into(), 1.into(), 1.into()], "C1" => vec![0.into(), 0.into(), 0.into()], "N" => 1 },
        });
        let res = d.add_object(dictionary! {
            "XObject" => dictionary! { "Fm1" => form },
            "ColorSpace" => dictionary! { "CS0" => vec![Object::Name(b"Separation".to_vec()), Object::Name(b"PANTONE 485 C".to_vec()), Object::Name(b"DeviceCMYK".to_vec()), tint.into()] },
            "Shading" => dictionary! { "Sh1" => sh },
        });
        let page = d.get_object_mut(kids[0]).unwrap().as_dict_mut().unwrap();
        page.set("Resources", res);
        page.set("Rotate", 90);
    });
    assert_eq!(page_sizes(&bytes).unwrap(), vec![(50.0, 100.0)]);
    let (doc, notes) = load_with_notes(&bytes, &PdfImportOptions::default()).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    let ps = paths(&doc, 0);
    assert_eq!(ps.len(), 3);
    // the form's square, scaled 2x; the page is turned a quarter clockwise
    assert_eq!(ps[0].style.fill, Fill::Solid(Color::rgb(0, 255, 0)));
    let bb = ps[0].world_path().bounding_box();
    assert!(close(bb, Rect::new(2.0, 2.0, 12.0, 12.0)), "{bb:?}");
    // spot color at half tint
    assert_eq!(ps[1].style.fill, Fill::Solid(Color::from_cmyk(0.0, 50.0, 0.0, 0.0)));
    assert!(doc.palette.iter().any(|c| c.spot && c.name == "PANTONE 485 C" && c.color == Color::from_cmyk(0.0, 100.0, 0.0, 0.0)));
    // the radial shading fills its clip
    let Fill::Gradient(g) = &ps[2].style.fill else { panic!() };
    assert_eq!(g.kind, vector_core::style::GradientKind::Elliptical);
    assert_eq!(g.sorted_stops()[0].color, Color::WHITE);
    assert!((g.start.x - 0.5).abs() < 1e-6 && (g.end.x - 1.0).abs() < 1e-6);
}

/// Manual check: `ANYVECTOR_PDF_SAMPLES=<folder of PDFs> ANYVECTOR_PDF_OUT=<folder>` imports
/// the first page of each PDF and renders it to PNG there.
#[test]
fn closing_twice_or_drawing_on_after_a_close() {
    // "h" twice, then "b" (which closes again), then segments after a close with no move
    let content = "0 0 m 10 0 l 10 10 l h h b 20 20 m 30 20 l 30 30 l h 40 40 l 20 40 l h f";
    let bytes = build_pdf(vec![((100.0, 100.0), content, lopdf::Dictionary::new())], |_, _| {});
    let (back, _) = load_with_notes(&bytes, &PdfImportOptions::default()).unwrap();
    let ps = paths(&back, 0);
    assert_eq!(ps.len(), 2);
    let bb = ps[1].world_path().bounding_box();
    assert!(close(bb, Rect::new(20.0, 60.0, 40.0, 80.0)), "{bb:?}");
}

#[test]
#[ignore]
fn sample_files() {
    let (Ok(dir), Ok(out)) = (std::env::var("ANYVECTOR_PDF_SAMPLES"), std::env::var("ANYVECTOR_PDF_OUT")) else { return };
    let curves = std::env::var("ANYVECTOR_PDF_CURVES").is_ok();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if crate::extension(&p) != "pdf" {
            continue;
        }
        let bytes = std::fs::read(&p).unwrap();
        let t = std::time::Instant::now();
        match load_with_notes(&bytes, &PdfImportOptions { pages: vec![0], text_as_curves: curves }) {
            Ok((doc, notes)) => {
                let n = all(&doc.pages[0].layers[0].objects).len();
                println!("{}: {} objects in {:?}; notes {:?}", p.display(), n, t.elapsed(), notes);
                let ro = crate::image_io::RasterOptions { dpi: 72.0, ..Default::default() };
                let name = p.file_stem().unwrap().to_string_lossy().to_string();
                let _ = crate::image_io::export_raster(&doc, 0, &std::path::Path::new(&out).join(format!("{name}.png")), &ro);
            }
            Err(e) => println!("{}: error {e}", p.display()),
        }
    }
}
