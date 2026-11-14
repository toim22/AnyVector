use super::*;
use vector_core::color::Color;
use vector_core::style::{Fill, GradientKind};

fn eps(body: &str) -> Vec<u8> {
    format!("%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 100 100\n%%EndComments\n{body}\n%%EOF\n").into_bytes()
}

fn load(body: &str) -> (Document, Vec<String>) {
    load_with_notes(&eps(body)).expect("loads")
}

fn objects(doc: &Document) -> &Vec<Object> {
    &doc.pages[0].layers[0].objects
}

/// Every object, inside groups and PowerClips too.
fn all(doc: &Document) -> Vec<Object> {
    let mut v = vec![];
    for o in objects(doc) {
        o.walk(&mut |x| v.push(x.clone()));
    }
    v
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.05
}

fn solid(o: &Object) -> Color {
    match &o.style.fill {
        Fill::Solid(c) => *c,
        f => panic!("not a solid fill: {f:?}"),
    }
}

#[test]
fn rectangle_path_is_placed_on_the_page() {
    let (doc, notes) = load("1 0 0 setrgbcolor newpath 10 10 moveto 30 10 lineto 30 30 lineto 10 30 lineto closepath fill");
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(doc.pages[0].width, 100.0);
    let o = &objects(&doc)[0];
    assert!(matches!(o.kind, ObjKind::Path(_)));
    assert_eq!(solid(o), Color::rgb(255, 0, 0));
    assert!(o.style.outline.color.is_none());
    // y up in PostScript, down on the page
    let b = o.bounds();
    assert!(near(b.x0, 10.0) && near(b.x1, 30.0) && near(b.y0, 70.0) && near(b.y1, 90.0), "{b:?}");
}

#[test]
fn transforms_curves_and_arcs() {
    let (doc, _) = load("gsave 50 50 translate 2 2 scale 45 rotate 0 0 moveto 10 0 lineto 0 setgray 1 setlinewidth stroke grestore \
                         newpath 50 50 10 0 360 arc closepath fill \
                         newpath 0 0 moveto 10 0 10 10 0 10 rcurveto stroke");
    let objs = objects(&doc);
    assert_eq!(objs.len(), 3);
    // the scaled line: 20 long at 45 degrees from the center, 2 wide
    let b = objs[0].bounds();
    assert!(near(b.x0, 50.0) && near(b.x1, 50.0 + 20.0 / 2f64.sqrt()), "{b:?}");
    assert!(near(objs[0].style.outline.width, 2.0));
    let c = objs[1].bounds();
    assert!(near(c.width(), 20.0) && near(c.height(), 20.0) && near(c.center().x, 50.0), "{c:?}");
    let ObjKind::Path(p) = &objs[2].kind else { panic!() };
    assert_eq!(p.subpaths[0].nodes.len(), 2);
}

#[test]
fn fill_then_stroke_of_one_path_is_one_object() {
    let (doc, _) = load("10 10 moveto 50 10 lineto 50 50 lineto closepath gsave 0 0 1 setrgbcolor fill grestore 3 setlinewidth [6 3] 0 setdash 1 setlinejoin stroke");
    let objs = objects(&doc);
    assert_eq!(objs.len(), 1);
    let o = &objs[0];
    assert_eq!(solid(o), Color::rgb(0, 0, 255));
    assert_eq!(o.style.outline.color, Some(Color::BLACK));
    assert!(near(o.style.outline.width, 3.0));
    assert_eq!(o.style.outline.dash, vec![2.0, 1.0]);
    assert_eq!(o.style.outline.join, vector_core::style::LineJoin::Round);
}

#[test]
fn cmyk_gray_hsb_and_spot_colors() {
    let (doc, _) = load("0 1 1 0 setcmykcolor 0 0 10 10 rectfill \
                         0.5 setgray 10 0 10 10 rectfill \
                         [/Separation (PANTONE 185 C) /DeviceCMYK {dup 0.91 mul exch dup 0.76 mul exch 0 exch 0 exch pop 0 0 4 2 roll}] setcolorspace 1 setcolor 20 0 10 10 rectfill \
                         0 1 1 sethsbcolor 30 0 10 10 rectfill \
                         0 0 0 1 (Gold) findcmykcustomcolor 0.5 setcustomcolor 40 0 10 10 rectfill");
    let objs = objects(&doc);
    assert_eq!(objs.len(), 5);
    // CMYK through CorelDRAW's default profile, as CorelDRAW shows it
    assert_eq!(solid(&objs[0]), crate::corel::color::cmyk(0.0, 1.0, 1.0, 0.0));
    assert_eq!(solid(&objs[1]), Color::rgb(128, 128, 128));
    assert_eq!(solid(&objs[3]), Color::rgb(255, 0, 0));
    // spot colors join the document palette
    let spot = doc.palette.iter().find(|c| c.name == "PANTONE 185 C").expect("spot in palette");
    assert!(spot.spot);
    assert_eq!(solid(&objs[2]), spot.color);
    assert!(doc.palette.iter().any(|c| c.name == "Gold" && c.spot));
}

#[test]
fn clipping_makes_powerclips() {
    let (doc, _) = load("gsave newpath 20 20 moveto 60 20 lineto 40 60 lineto closepath clip newpath 0 0 100 100 rectfill grestore \
                         gsave 0 0 100 100 rectclip 0 0 1 setrgbcolor 10 10 5 5 rectfill grestore");
    let objs = objects(&doc);
    assert_eq!(objs.len(), 2, "a clip that holds everything inside it is dropped");
    let ObjKind::PowerClip { frame, contents } = &objs[0].kind else { panic!("powerclip expected") };
    assert!(near(frame.bounds().width(), 40.0));
    assert_eq!(contents.len(), 1);
    assert!(matches!(objs[1].kind, ObjKind::Path(_)));
}

#[test]
fn same_clip_again_shares_one_powerclip() {
    let (doc, _) = load("gsave 20 20 40 40 rectclip 0 0 50 50 rectfill grestore gsave 20 20 40 40 rectclip 1 0 0 setrgbcolor 30 30 50 50 rectfill grestore");
    let objs = objects(&doc);
    assert_eq!(objs.len(), 1);
    let ObjKind::PowerClip { contents, .. } = &objs[0].kind else { panic!() };
    assert_eq!(contents.len(), 2);
}

fn image_of(doc: &Document, o: &Object) -> ImageData {
    let ObjKind::Image(im) = &o.kind else { panic!("image expected") };
    doc.images[&im.image].clone()
}

#[test]
fn images_from_strings_files_and_filters() {
    // 2x2 RGB from a hex string, 2x1 gray from currentfile, 2x2 CMYK through ASCII85
    let a85 = {
        let raw = [0u8, 0, 0, 0, 255, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 255];
        let mut s = String::new();
        for chunk in raw.chunks(4) {
            let v = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            if v == 0 {
                s.push('z');
                continue;
            }
            let mut d = [0u8; 5];
            let mut x = v;
            for k in (0..5).rev() {
                d[k] = (x % 85) as u8 + b'!';
                x /= 85;
            }
            s.push_str(std::str::from_utf8(&d).unwrap());
        }
        s
    };
    let body = format!(
        "gsave 10 10 translate 20 20 scale 2 2 8 [2 0 0 -2 0 2] <ff000000ff000000ffffffff> false 3 colorimage grestore\n\
         gsave 50 10 translate 20 10 scale /picstr 2 string def 2 1 8 [2 0 0 -1 0 1] {{currentfile picstr readhexstring pop}} image\n00ff\ngrestore\n\
         gsave 10 50 translate 20 20 scale /DeviceCMYK setcolorspace << /ImageType 1 /Width 2 /Height 2 /BitsPerComponent 8 /Decode [0 1 0 1 0 1 0 1] /ImageMatrix [2 0 0 -2 0 2] /DataSource currentfile /ASCII85Decode filter >> image\n{a85}~>\ngrestore\n\
         0 0 1 setrgbcolor gsave 50 50 translate 20 20 scale 8 1 false [8 0 0 1 0 0] <0f> imagemask grestore"
    );
    let (doc, notes) = load(&body);
    assert!(notes.is_empty(), "{notes:?}");
    let objs = objects(&doc);
    assert_eq!(objs.len(), 4);
    let rgb = image_of(&doc, &objs[0]);
    assert_eq!((rgb.width, rgb.height), (2, 2));
    assert_eq!(&rgb.rgba[..8], &[255, 0, 0, 255, 0, 255, 0, 255]);
    let b = objs[0].bounds();
    assert!(near(b.x0, 10.0) && near(b.width(), 20.0) && near(b.y0, 70.0), "{b:?}");
    let gray = image_of(&doc, &objs[1]);
    assert_eq!(&gray.rgba[..], &[0, 0, 0, 255, 255, 255, 255, 255]);
    let cmyk = image_of(&doc, &objs[2]);
    assert_eq!(&cmyk.rgba[..3], &{
        let c = crate::corel::color::cmyk(0.0, 0.0, 0.0, 0.0);
        [c.r, c.g, c.b]
    });
    assert_eq!(&cmyk.rgba[4..7], &{
        let c = crate::corel::color::cmyk(1.0, 0.0, 0.0, 0.0);
        [c.r, c.g, c.b]
    });
    // the mask paints the zero bits in the current color, the rest is transparent
    let m = image_of(&doc, &objs[3]);
    assert_eq!(m.width, 8);
    assert_eq!(&m.rgba[..4], &[0, 0, 255, 255]);
    assert_eq!(m.rgba[4 * 4 + 3], 0);
}

#[test]
fn flate_compressed_image() {
    use std::io::Write;
    let mut e = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::default());
    e.write_all(&[10, 20, 30, 40]).unwrap();
    let z = e.finish().unwrap();
    let hex: String = z.iter().map(|b| format!("{b:02x}")).collect();
    let body = format!("/DeviceGray setcolorspace << /ImageType 1 /Width 4 /Height 1 /BitsPerComponent 8 /Decode [0 1] /ImageMatrix [4 0 0 -1 0 1] /DataSource currentfile /ASCIIHexDecode filter /FlateDecode filter >> image\n{hex}>\n");
    let (doc, _) = load(&body);
    let img = image_of(&doc, &objects(&doc)[0]);
    assert_eq!(img.rgba.iter().step_by(4).copied().collect::<Vec<u8>>(), vec![10, 20, 30, 40]);
}

#[test]
fn text_becomes_artistic_text() {
    let (doc, _) = load("/Helvetica-Bold findfont 12 scalefont setfont 0 0 1 setrgbcolor 10 20 moveto (Hello) show (!) show \
                         /Times-Roman findfont dup length dict begin {1 index /FID ne {def} {pop pop} ifelse} forall /Encoding ISOLatin1Encoding def currentdict end /Times-ISO exch definefont pop \
                         /Times-ISO 20 selectfont 10 50 moveto (Caf\\351) show");
    let objs = objects(&doc);
    assert_eq!(objs.len(), 3);
    let ObjKind::Text(t) = &objs[0].kind else { panic!("text expected") };
    assert_eq!(t.content, "Hello");
    assert!(near(t.format.size, 12.0));
    assert!(t.format.bold);
    assert_eq!(solid(&objs[0]), Color::rgb(0, 0, 255));
    // baseline at y 20 (80 on the page)
    let p = objs[0].transform * kurbo::Point::ZERO;
    assert!(near(p.x, 10.0) && near(p.y, 80.0), "{p:?}");
    // the second show starts where the first ended
    let q = objs[1].transform * kurbo::Point::ZERO;
    assert!(q.x > 30.0 && near(q.y, 80.0), "{q:?}");
    let ObjKind::Text(t) = &objs[2].kind else { panic!() };
    assert_eq!(t.content, "Café");
    assert!(near(t.format.size, 20.0));
}

#[test]
fn rotated_text_keeps_its_angle() {
    let (doc, _) = load("/Courier findfont 10 scalefont setfont 50 50 translate 90 rotate 0 0 moveto (Up) show");
    let o = &objects(&doc)[0];
    // the text's x axis points up the page
    let v = o.transform * kurbo::Point::new(1.0, 0.0) - o.transform * kurbo::Point::ZERO;
    assert!(near(v.x, 0.0) && near(v.y, -1.0), "{v:?}");
}

#[test]
fn type3_glyphs_become_curves() {
    let (doc, _) = load(
        "8 dict begin /FontType 3 def /FontMatrix [0.001 0 0 0.001 0 0] def /FontBBox [0 0 1000 1000] def \
         /Encoding 256 array def 0 1 255 {Encoding exch /.notdef put} for Encoding 65 /box put \
         /BuildChar { pop pop 1000 0 0 0 800 800 setcachedevice 0 0 moveto 800 0 lineto 800 800 lineto closepath fill } def \
         currentdict end /BoxFont exch definefont pop \
         /BoxFont findfont 10 scalefont setfont 10 10 moveto (AAA) show",
    );
    let objs = objects(&doc);
    // three glyphs, one curve
    assert_eq!(objs.len(), 1);
    let ObjKind::Path(p) = &objs[0].kind else { panic!() };
    assert_eq!(p.subpaths.len(), 3);
    let b = objs[0].bounds();
    assert!(near(b.x0, 10.0) && near(b.x1, 38.0), "{b:?}");
}

#[test]
fn control_flow_and_procedures() {
    let (doc, notes) = load(
        "/bx { 4 2 roll moveto 1 index 0 rlineto 0 exch rlineto neg 0 rlineto closepath fill } bind def \
         0 10 40 { 0 5 5 bx } for \
         3 { } repeat \
         [1 2] { pop } forall \
         0 { 1 add dup 3 eq { exit } if } loop pop \
         { 1 0 div } stopped { 50 50 5 5 bx } if \
         true { 60 60 5 5 bx } { } ifelse \
         /n 0 def { /n n 1 add def n 2 ge { exit } if } loop n 2 eq { 70 70 5 5 bx } if \
         (abc) (b) search { pop pop pop 80 80 5 5 bx } if",
    );
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(objects(&doc).len(), 9);
}

#[test]
fn unknown_operators_are_skipped_and_counted() {
    let (doc, notes) = load("frobnicate 10 10 20 20 rectfill frobnicate wibble 1 2 3 0 0 5 5 rectfill");
    assert_eq!(objects(&doc).len(), 2);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].starts_with("Left out: 3 use(s) of PostScript operators"), "{}", notes[0]);
    assert!(notes[0].contains("frobnicate") && notes[0].contains("wibble"));
}

#[test]
fn illustrator_operators_without_their_procset() {
    let body = "%%EndSetup\n0 0 1 0 k 0 0 0 1 K 2 w\n10 10 m 40 10 l 40 40 l 10 40 l b\n\
                *u\n0 1 0 0 k 50 50 m 90 50 l 90 90 l 50 90 l f\n60 60 m 80 60 l 80 80 l 60 80 l f\n*U\n\
                0 0 0 0 (Orange Spot) 0 x 0 0 m 5 0 l 5 5 l F\n";
    let (doc, notes) = load(body);
    assert!(notes.is_empty(), "{notes:?}");
    let objs = objects(&doc);
    assert_eq!(objs.len(), 3);
    assert_eq!(solid(&objs[0]), crate::corel::color::cmyk(0.0, 0.0, 1.0, 0.0));
    assert!(near(objs[0].style.outline.width, 2.0));
    assert_eq!(objs[0].style.outline.color, Some(crate::corel::color::cmyk(0.0, 0.0, 0.0, 1.0)));
    // the compound path: one curve of two subpaths
    let ObjKind::Path(p) = &objs[1].kind else { panic!() };
    assert_eq!(p.subpaths.len(), 2);
    assert!(doc.palette.iter().any(|c| c.name == "Orange Spot"));
}

#[test]
fn axial_and_radial_shadings_are_fountain_fills() {
    let (doc, _) = load(
        "gsave 0 0 100 50 rectclip << /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 100 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> >> shfill grestore \
         newpath 50 75 20 0 360 arc << /PatternType 2 /Shading << /ShadingType 3 /ColorSpace /DeviceGray /Coords [50 75 0 50 75 20] /Function << /FunctionType 2 /C0 [1] /C1 [0] /N 1 >> >> >> matrix makepattern setpattern fill",
    );
    let objs = objects(&doc);
    assert_eq!(objs.len(), 2);
    let Fill::Gradient(g) = &objs[0].style.fill else { panic!("gradient expected") };
    assert_eq!(g.kind, GradientKind::Linear);
    assert_eq!(g.stops.first().unwrap().color, Color::rgb(255, 0, 0));
    assert_eq!(g.stops.last().unwrap().color, Color::rgb(0, 0, 255));
    assert!(near(g.start.x, 0.0) && near(g.end.x, 1.0));
    let Fill::Gradient(r) = &objs[1].style.fill else { panic!() };
    assert_eq!(r.kind, GradientKind::Elliptical);
    assert!(near(r.start.x, 0.5) && near(r.start.y, 0.5));
}

#[test]
fn eexec_encrypted_code_runs() {
    let plain = b"\0\0\0\0/Encrypted { 0 0 10 10 rectfill } def currentfile closefile";
    let mut r: u16 = 55665;
    let mut hex = String::new();
    for &p in plain.iter() {
        let c = p ^ (r >> 8) as u8;
        r = (c as u16).wrapping_add(r).wrapping_mul(52845).wrapping_add(22719);
        hex.push_str(&format!("{c:02x}"));
    }
    let zeros = format!("{}\n", "0".repeat(64)).repeat(8);
    let body = format!("currentfile eexec\n{hex}\n{zeros}cleartomark\nEncrypted 20 20 5 5 rectfill");
    let (doc, notes) = load(&format!("mark {body}"));
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(objects(&doc).len(), 2);
}

#[test]
fn pages_of_a_postscript_file() {
    let ps = b"%!PS-Adobe-3.0\n%%Pages: 2\n<< /PageSize [200 300] >> setpagedevice\n0 0 10 10 rectfill showpage\n5 5 10 10 rectfill showpage\n";
    let (doc, _) = load_with_notes(ps).unwrap();
    assert_eq!(doc.pages.len(), 2);
    assert_eq!((doc.pages[0].width, doc.pages[0].height), (200.0, 300.0));
    let b = doc.pages[1].layers[0].objects[0].bounds();
    assert!(near(b.y1, 295.0), "{b:?}");
    // importing takes the first page and says so
    let mut d = Document::new(500.0, 500.0);
    let (objs, notes) = import(&mut d, ps).unwrap();
    assert_eq!(objs.len(), 1);
    assert!(notes.iter().any(|n| n.contains("first of 2 pages")), "{notes:?}");
}

/// A DOS EPS: binary header, PostScript and a TIFF preview.
fn dos_eps(ps: &[u8]) -> Vec<u8> {
    let mut tiff = vec![];
    let img = ::image::RgbaImage::from_pixel(4, 2, ::image::Rgba([10, 200, 30, 255]));
    ::image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut tiff), ::image::ImageFormat::Tiff).unwrap();
    let mut out = vec![0xC5, 0xD0, 0xD3, 0xC6];
    let ps_at = 30u32;
    let tiff_at = ps_at + ps.len() as u32;
    for v in [ps_at, ps.len() as u32, 0, 0, tiff_at, tiff.len() as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[0xff, 0xff]);
    out.extend_from_slice(ps);
    out.extend_from_slice(&tiff);
    out
}

#[test]
fn dos_eps_header_is_read() {
    let ps = eps("1 0 0 setrgbcolor 0 0 50 50 rectfill");
    let (doc, notes) = load_with_notes(&dos_eps(&ps)).unwrap();
    assert!(notes.is_empty());
    assert!(matches!(objects(&doc)[0].kind, ObjKind::Path(_)));
}

#[test]
fn unreadable_postscript_falls_back_to_the_tiff_preview() {
    let ps = eps("mysteryop anotherone 1 2 3 thirdop");
    let (doc, notes) = load_with_notes(&dos_eps(&ps)).unwrap();
    let objs = objects(&doc);
    assert_eq!(objs.len(), 1);
    let img = image_of(&doc, &objs[0]);
    assert_eq!((img.width, img.height), (4, 2));
    assert_eq!(&img.rgba[..4], &[10, 200, 30, 255]);
    assert!(near(objs[0].bounds().width(), 100.0));
    assert_eq!(notes[0], PREVIEW_NOTE);
    assert!(notes.iter().any(|n| n.contains("mysteryop")));
}

#[test]
fn import_groups_the_objects() {
    let mut d = Document::new(500.0, 500.0);
    let (objs, _) = import(&mut d, &eps("0 0 10 10 rectfill 20 20 10 10 rectfill")).unwrap();
    assert_eq!(objs.len(), 1);
    assert!(matches!(&objs[0].kind, ObjKind::Group(g) if g.len() == 2));
}

#[test]
fn errors_do_not_stop_the_program() {
    let (doc, notes) = load("0 0 moveto (x) 5 add 10 10 20 20 rectfill");
    assert_eq!(objects(&doc).len(), 1);
    assert!(notes.iter().any(|n| n.contains("typecheck in add")), "{notes:?}");
}

#[test]
fn pdf_based_illustrator_files_are_refused_clearly() {
    assert!(matches!(load_with_notes(b"%PDF-1.5\n..."), Err(IoError::Unsupported(_))));
}

#[test]
fn all_objects_have_distinct_ids() {
    let (doc, _) = load("gsave 0 0 50 50 rectclip 0 0 60 60 rectfill 1 1 5 5 rectfill grestore 0 0 10 10 rectfill");
    let mut ids: Vec<u64> = all(&doc).iter().map(|o| o.id).collect();
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n);
}
