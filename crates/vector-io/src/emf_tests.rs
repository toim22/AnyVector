//! Tests of the EMF writer and reader.

use super::*;
use vector_core::object::{ObjKind, Object};
use vector_core::path::NodePath;
use vector_core::style::{Fill, Outline, Style};

/// (type, record bytes) of every record; asserts the framing on the way.
fn records(emf: &[u8]) -> Vec<(u32, Vec<u8>)> {
    let mut out = vec![];
    let mut off = 0;
    while off < emf.len() {
        let ty = u32_at(emf, off);
        let size = u32_at(emf, off + 4) as usize;
        assert!(size >= 8 && size % 4 == 0, "record {ty} has size {size}");
        assert!(off + size <= emf.len(), "record {ty} runs past the end");
        out.push((ty, emf[off..off + size].to_vec()));
        off += size;
    }
    out
}

/// (type, flags, data) of the EMF+ records in the comments.
fn plus_records(emf: &[u8]) -> Vec<(u16, u16, Vec<u8>)> {
    let mut out = vec![];
    for (ty, r) in records(emf) {
        if ty != emr::COMMENT || r.len() < 16 || u32_at(&r, 12) != plus::COMMENT_ID {
            continue;
        }
        assert_eq!(u32_at(&r, 8) as usize, r.len() - 12, "comment data size");
        let mut o = 16;
        while o + 12 <= r.len() {
            let t = u16::from_le_bytes([r[o], r[o + 1]]);
            let f = u16::from_le_bytes([r[o + 2], r[o + 3]]);
            let size = u32_at(&r, o + 4) as usize;
            let dsize = u32_at(&r, o + 8) as usize;
            assert_eq!(size, dsize + 12, "EMF+ record {t:#x} sizes");
            assert_eq!(size % 4, 0);
            out.push((t, f, r[o + 12..o + size].to_vec()));
            o += size;
        }
        assert_eq!(o, r.len(), "EMF+ records fill the comment");
    }
    out
}

fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
fn i32_at(d: &[u8], o: usize) -> i32 {
    u32_at(d, o) as i32
}

fn shape(doc: &mut Document, path: BezPath, fill: Fill, outline: Outline) {
    let mut st = Style::default();
    st.fill = fill;
    st.outline = outline;
    let o = Object::new(doc.new_id(), ObjKind::Path(NodePath::from_bez(&path)), Affine::IDENTITY, st);
    doc.pages[0].layers[0].objects.push(o);
}

/// A 2 x 1 inch page: a half-transparent red ellipse with a blue 2 pt dashed outline.
fn sample() -> Document {
    let mut doc = Document::new(144.0, 72.0);
    let mut e = kurbo::Ellipse::new((72.0, 36.0), (60.0, 30.0), 0.0).to_path(0.1);
    e.close_path();
    let ol = Outline { width: 2.0, color: Some(Color::rgb(0, 0, 255)), dash: vec![2.0, 1.0], ..Outline::default() };
    shape(&mut doc, e, Fill::Solid(Color::rgba(255, 0, 0, 128)), ol);
    doc
}

fn image_data(w: u32, h: u32, rgba: Vec<u8>) -> ImageData {
    ImageData { width: w, height: h, rgba: std::sync::Arc::new(rgba), encoded: Default::default(), format: "png".into(), name: "x".into() }
}

fn place_image(doc: &mut Document, img: ImageData, w: f64, h: f64, xf: Affine) {
    let iid = doc.add_image(img);
    let io = vector_core::object::ImageObj { image: iid, w, h, crop: [0.0, 0.0, 1.0, 1.0], adjust: Default::default() };
    let mut st = Style::default();
    st.outline = Outline::none();
    let o = Object::new(doc.new_id(), ObjKind::Image(io), xf, st);
    doc.pages[0].layers[0].objects.push(o);
}

#[test]
fn header_and_record_stream() {
    let emf = page_to_emf(&sample(), 0, &EmfOptions::default());
    let recs = records(&emf);
    let h = &recs[0].1;
    assert_eq!(recs[0].0, emr::HEADER);
    assert_eq!(u32_at(h, 40), 0x464D_4520, "signature");
    assert_eq!(u32_at(h, 48) as usize, emf.len(), "nBytes");
    assert_eq!(u32_at(h, 52) as usize, recs.len(), "nRecords");
    assert_eq!(u16::from_le_bytes([h[56], h[57]]), 3, "handles");
    // 2 x 1 in: 192 x 96 device pixels at 96 dpi, 50.8 x 25.4 mm
    assert_eq!([i32_at(h, 8), i32_at(h, 12), i32_at(h, 16), i32_at(h, 20)], [0, 0, 191, 95], "bounds");
    assert_eq!([i32_at(h, 24), i32_at(h, 28), i32_at(h, 32), i32_at(h, 36)], [0, 0, 5080, 2540], "frame");
    assert_eq!([i32_at(h, 72), i32_at(h, 76), i32_at(h, 80), i32_at(h, 84)], [1920, 1440, 508, 381], "device");
    let n = u32_at(h, 60) as usize;
    let desc: Vec<u16> = h[108..108 + n * 2].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    assert!(String::from_utf16_lossy(&desc).starts_with(vector_core::APP_NAME));
    let last = recs.last().unwrap();
    assert_eq!(last.0, emr::EOF);
    assert_eq!(last.1.len(), 20);
    // EMF+: the header comes first (dual), the end of file last
    let plus = plus_records(&emf);
    assert_eq!(recs[1].0, emr::COMMENT, "the EMF+ header follows the EMF header");
    assert_eq!((plus[0].0, plus[0].1), (plus::HEADER, 1));
    assert_eq!(u32_at(&plus[0].2, 0), plus::VERSION);
    assert_eq!(plus.last().unwrap().0, plus::END_OF_FILE);
    // GDI: a path filled with the brush and one stroked with the pen
    let types: Vec<u32> = recs.iter().map(|r| r.0).collect();
    for t in [emr::SETMAPMODE, emr::BEGINPATH, emr::POLYBEZIERTO, emr::ENDPATH, emr::FILLPATH, emr::EXTCREATEPEN, emr::STROKEPATH] {
        assert!(types.contains(&t), "missing GDI record {t}");
    }
}

#[test]
fn curves_colors_and_pens_in_emf_plus() {
    let emf = page_to_emf(&sample(), 0, &EmfOptions::default());
    let plus = plus_records(&emf);
    // the ellipse path: Bézier points (type 3)
    let path = plus.iter().find(|r| r.0 == plus::OBJECT && r.1 >> 8 == plus::OBJ_PATH).expect("path object");
    let n = u32_at(&path.2, 4) as usize;
    let types = &path.2[12 + n * 8..12 + n * 9];
    assert_eq!(types[0], 0, "starts a figure");
    assert!(types.iter().filter(|t| **t & 7 == 3).count() >= 12, "curves stay curves: {types:?}");
    assert!(types.last().unwrap() & 0x80 != 0, "the figure is closed: {types:?}");
    // first point: (132, 36) pt = (176, 48) px
    let x = f32::from_le_bytes(path.2[12..16].try_into().unwrap());
    let y = f32::from_le_bytes(path.2[16..20].try_into().unwrap());
    assert!((x - 176.0).abs() < 0.01 && (y - 48.0).abs() < 0.01, "({x}, {y})");
    // a solid ARGB fill with half alpha
    let fill = plus.iter().find(|r| r.0 == plus::FILL_PATH).expect("fill");
    assert!(fill.1 & 0x8000 != 0, "a solid color, not a brush object");
    assert_eq!(u32_at(&fill.2, 0), 0x80FF_0000);
    // the pen: 2 pt = 2.667 px, custom dashes in pen widths, blue
    let pen = plus.iter().find(|r| r.0 == plus::OBJECT && r.1 >> 8 == plus::OBJ_PEN).expect("pen");
    let flags = u32_at(&pen.2, 8);
    assert!(flags & 0x100 != 0, "dash pattern");
    let width = f32::from_le_bytes(pen.2[16..20].try_into().unwrap());
    assert!((width - 2.0 * 96.0 / 72.0).abs() < 1e-4);
    assert_eq!(u32_at(&pen.2, pen.2.len() - 4), 0xFF00_00FF, "pen color");
    assert!(plus.iter().any(|r| r.0 == plus::DRAW_PATH));
    // plain EMF: no comments at all
    let gdi = page_to_emf(&sample(), 0, &EmfOptions { emf_plus: false, ..Default::default() });
    assert!(plus_records(&gdi).is_empty());
    assert!(records(&gdi).iter().any(|r| r.0 == emr::FILLPATH));
}

#[test]
fn gradients_and_images() {
    use vector_core::style::{Gradient, GradientStop};
    let mut doc = Document::new(100.0, 100.0);
    shape(&mut doc, Rect::new(10.0, 10.0, 90.0, 40.0).to_path(0.1), Fill::Gradient(Gradient::linear(Color::rgb(255, 0, 0), Color::rgb(0, 0, 255))), Outline::none());
    let mut g = Gradient::radial(Color::WHITE, Color::BLACK);
    g.stops.push(GradientStop { pos: 0.5, color: Color::rgba(0, 255, 0, 100) });
    shape(&mut doc, Rect::new(10.0, 50.0, 90.0, 90.0).to_path(0.1), Fill::Gradient(g), Outline::none());
    place_image(&mut doc, image_data(2, 2, vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 128]), 20.0, 20.0, Affine::translate((5.0, 5.0)));
    let emf = page_to_emf(&doc, 0, &EmfOptions::default());
    let plus = plus_records(&emf);
    let brushes: Vec<u32> = plus.iter().filter(|r| r.0 == plus::OBJECT && r.1 >> 8 == plus::OBJ_BRUSH).map(|r| u32_at(&r.2, 4)).collect();
    assert_eq!(brushes, vec![4, 3], "a linear and a path gradient brush");
    let img = plus.iter().find(|r| r.0 == plus::OBJECT && r.1 >> 8 == plus::OBJ_IMAGE).expect("image object");
    assert_eq!(&img.2[28..32], b"\x89PNG", "the bitmap is embedded as PNG");
    assert!(plus.iter().any(|r| r.0 == plus::DRAW_IMAGE_POINTS));
    // GDI: the bitmap has alpha, so it is alpha-blended
    assert!(records(&emf).iter().any(|r| r.0 == emr::ALPHABLEND));
}

#[test]
fn large_objects_are_continued() {
    let mut doc = Document::new(100.0, 100.0);
    // noise, so the PNG stays large
    let (w, h) = (300u32, 300u32);
    let rgba: Vec<u8> = (0..w * h * 4).map(|i| if i % 4 == 3 { 255 } else { (i.wrapping_mul(2_654_435_761) >> 13) as u8 }).collect();
    place_image(&mut doc, image_data(w, h, rgba), 100.0, 100.0, Affine::IDENTITY);
    let emf = page_to_emf(&doc, 0, &EmfOptions::default());
    let parts: Vec<_> = plus_records(&emf).into_iter().filter(|r| r.0 == plus::OBJECT && (r.1 >> 8) & 0x7f == plus::OBJ_IMAGE).collect();
    assert!(parts.len() > 1, "split in parts");
    let total = u32_at(&parts[0].2, 0) as usize;
    assert!(parts.iter().all(|p| p.1 & 0x8000 != 0 && u32_at(&p.2, 0) as usize == total));
    let sum: usize = parts.iter().map(|p| p.2.len() - 4).sum();
    assert!(sum >= total && sum < total + 4 * parts.len(), "the parts add up to the object ({sum} vs {total})");
}

#[test]
fn stop_expansion() {
    let stops = vec![(0.0, Color::BLACK), (1.0, Color::WHITE)];
    let pad = expand_stops(&stops, SpreadMode::Pad, -1.0, 2.0);
    assert_eq!(pad.first().unwrap(), &(0.0, Color::BLACK));
    assert_eq!(pad.last().unwrap(), &(1.0, Color::WHITE));
    assert!(pad.iter().any(|(p, c)| (*p - 1.0 / 3.0).abs() < 1e-3 && *c == Color::BLACK));
    let rep = expand_stops(&stops, SpreadMode::Reflect, 0.0, 3.0);
    assert!(rep.windows(2).all(|w| w[0].0 <= w[1].0), "ascending");
    assert_eq!(rep.last().unwrap().1, Color::WHITE, "reflect ends white after 3 cycles");
    let rep = expand_stops(&stops, SpreadMode::Repeat, 0.0, 2.0);
    assert!(rep.len() >= 4, "a hard stop between the cycles: {rep:?}");
}

#[test]
fn dib_layout() {
    let d = dib_from_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 255, 0], false, Some(Color::WHITE));
    assert_eq!(d.len(), 40 + 8);
    assert_eq!(&d[40..48], &[0, 0, 255, 255, 255, 255, 255, 255], "BGRA, transparent over white");
}

// ------------------------------------------------------------------------------------------
// reading back

#[test]
fn round_trip_through_the_reader() {
    let emf = page_to_emf(&sample(), 0, &EmfOptions::default());
    let size = crate::emf_import::frame_size(&emf).unwrap();
    assert!((size.0 - 144.0).abs() < 0.01 && (size.1 - 72.0).abs() < 0.01, "{size:?}");
    let doc = crate::emf_import::document_from_emf(&emf).unwrap();
    let objs = &doc.pages[0].layers[0].objects;
    assert_eq!(objs.len(), 1, "fill and outline of one shape come back as one object");
    let o = &objs[0];
    let ObjKind::Path(np) = &o.kind else { panic!("not a path") };
    // GDI has no transparency: the color comes back opaque
    assert_eq!(o.style.fill, Fill::Solid(Color::rgb(255, 0, 0)));
    assert_eq!(o.style.outline.color, Some(Color::rgb(0, 0, 255)));
    assert!((o.style.outline.width - 2.0).abs() < 0.05, "outline width {}", o.style.outline.width);
    assert!(!o.style.outline.dash.is_empty(), "dashed");
    let bez = np.to_bez();
    assert!(bez.elements().iter().any(|e| matches!(e, PathEl::CurveTo(..))), "curves come back as curves");
    let b = bez.bounding_box();
    assert!((b.x0 - 12.0).abs() < 0.1 && (b.x1 - 132.0).abs() < 0.1 && (b.y0 - 6.0).abs() < 0.1 && (b.y1 - 66.0).abs() < 0.1, "{b:?}");
}

#[test]
fn round_trip_gradient_and_bitmap() {
    use vector_core::style::Gradient;
    let mut doc = Document::new(100.0, 100.0);
    let oval = kurbo::Ellipse::from_rect(Rect::new(10.0, 10.0, 90.0, 40.0)).to_path(0.1);
    shape(&mut doc, oval, Fill::Gradient(Gradient::linear(Color::rgb(255, 0, 0), Color::rgb(0, 0, 255))), Outline::none());
    place_image(&mut doc, image_data(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255]), 40.0, 20.0, Affine::translate((10.0, 60.0)));
    let emf = page_to_emf(&doc, 0, &EmfOptions::default());
    let back = crate::emf_import::document_from_emf(&emf).unwrap();
    let ObjKind::Group(items) = &back.pages[0].layers[0].objects[0].kind else { panic!("a group") };
    // the gradient's bands, clipped to the oval
    let ObjKind::PowerClip { frame, contents } = &items[0].kind else { panic!("a PowerClip, got {:?}", items[0].kind) };
    let fb = frame.world_path().bounding_box();
    assert!((fb.x0 - 10.0).abs() < 0.1 && (fb.x1 - 90.0).abs() < 0.1, "{fb:?}");
    assert!(contents.len() > 10, "bands");
    let first = contents.first().unwrap().style.fill.main_color().unwrap();
    assert!(first.r > 200 && first.b < 60, "starts red: {first:?}");
    // the bitmap's own rectangular clip clips nothing: it is dropped
    // the bitmap, in place
    let img = items.iter().find(|o| matches!(o.kind, ObjKind::Image(_))).expect("bitmap");
    let ib = img.bounds();
    assert!((ib.x0 - 10.0).abs() < 0.1 && (ib.y0 - 60.0).abs() < 0.1 && (ib.width() - 40.0).abs() < 0.1 && (ib.height() - 20.0).abs() < 0.1, "{ib:?}");
    let ObjKind::Image(io) = &img.kind else { unreachable!() };
    let data = back.images.get(&io.image).unwrap();
    assert_eq!((data.width, data.height), (2, 1));
    assert_eq!(&data.rgba[..], &[255, 0, 0, 255, 0, 0, 255, 255]);
}

#[test]
fn reader_rejects_other_data() {
    assert!(!crate::emf_import::is_emf(b"not a metafile at all"));
    let mut d = Document::new(10.0, 10.0);
    assert!(crate::emf_import::objects_from_emf(&mut d, &[0; 200]).is_err());
}

// ------------------------------------------------------------------------------------------
// playback by Windows

/// Render an EMF with GDI+ (System.Drawing: EMF+ records) and with GDI (PlayEnhMetaFile: the
/// fallback records) into 192 x 96 PNGs, through PowerShell.
#[cfg(windows)]
fn play(emf: &[u8], tag: &str) -> (image::RgbaImage, image::RgbaImage) {
    let dir = std::env::temp_dir().join(format!("anyvector-emf-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("in.emf");
    std::fs::write(&src, emf).unwrap();
    let (plus_png, gdi_png) = (dir.join("plus.png"), dir.join("gdi.png"));
    let script = format!(
        r#"$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class G {{
  [StructLayout(LayoutKind.Sequential)] public struct RECT {{ public int l, t, r, b; }}
  [DllImport("gdi32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr GetEnhMetaFileW(string f);
  [DllImport("gdi32.dll")] public static extern bool DeleteEnhMetaFile(IntPtr h);
  public delegate int EnhProc(IntPtr hdc, IntPtr ht, IntPtr rec, int n, IntPtr data);
  [DllImport("gdi32.dll")] public static extern bool EnumEnhMetaFile(IntPtr hdc, IntPtr h, EnhProc p, IntPtr data, ref RECT r);
  [DllImport("gdi32.dll")] public static extern bool PlayEnhMetaFileRecord(IntPtr hdc, IntPtr ht, IntPtr rec, uint n);
  static string failed = "";
  static int Proc(IntPtr hdc, IntPtr ht, IntPtr rec, int n, IntPtr data) {{
    if (!PlayEnhMetaFileRecord(hdc, ht, rec, (uint)n)) failed += Marshal.ReadInt32(rec) + " ";
    return 1;
  }}
  public static string Play(IntPtr hdc, IntPtr h, RECT r) {{ failed = ""; EnumEnhMetaFile(hdc, h, Proc, IntPtr.Zero, ref r); return failed; }}
}}
'@
$m = New-Object System.Drawing.Imaging.Metafile('{src}')
$b = New-Object System.Drawing.Bitmap 192, 96
$g = [System.Drawing.Graphics]::FromImage($b); $g.Clear([System.Drawing.Color]::White)
$g.DrawImage($m, 0, 0, 192, 96); $g.Dispose(); $m.Dispose()
$b.Save('{plus}', [System.Drawing.Imaging.ImageFormat]::Png)
$b2 = New-Object System.Drawing.Bitmap 192, 96
$g = [System.Drawing.Graphics]::FromImage($b2); $g.Clear([System.Drawing.Color]::White)
$hdc = $g.GetHdc(); $h = [G]::GetEnhMetaFileW('{src}')
$r = New-Object G+RECT; $r.l = 0; $r.t = 0; $r.r = 192; $r.b = 96
$failed = [G]::Play($hdc, $h, $r)
[void][G]::DeleteEnhMetaFile($h); $g.ReleaseHdc($hdc); $g.Dispose()
$b2.Save('{gdi}', [System.Drawing.Imaging.ImageFormat]::Png)
Write-Output "failed:$failed"
"#,
        src = src.display(),
        plus = plus_png.display(),
        gdi = gdi_png.display()
    );
    let out = std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]).output().expect("powershell");
    assert!(out.status.success(), "playback failed: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let failed = stdout.lines().find_map(|l| l.strip_prefix("failed:")).unwrap_or("").trim().to_string();
    assert!(failed.is_empty(), "GDI could not play records of these types: {failed}");
    let a = image::open(&plus_png).unwrap().into_rgba8();
    let b = image::open(&gdi_png).unwrap().into_rgba8();
    let _ = std::fs::remove_dir_all(&dir);
    (a, b)
}

#[cfg(windows)]
fn near(p: &image::Rgba<u8>, c: [u8; 3], tol: i32) -> bool {
    (0..3).all(|i| (p[i] as i32 - c[i] as i32).abs() <= tol)
}

/// GDI+ and GDI both play the metafile at the right size with the right colors. Run with
/// `cargo test -p vector-io emf_windows -- --ignored`.
#[cfg(windows)]
#[test]
#[ignore]
fn emf_windows_playback() {
    use vector_core::style::Gradient;
    let mut doc = Document::new(144.0, 72.0);
    // left: an opaque green square with a hole (even-odd) and a nonzero one without
    let mut ring = Rect::new(6.0, 6.0, 42.0, 42.0).to_path(0.1);
    ring.extend(Rect::new(18.0, 18.0, 30.0, 30.0).to_path(0.1).elements().iter().copied());
    let mut st = Style::default();
    st.fill = Fill::Solid(Color::rgb(0, 160, 0));
    st.outline = Outline::none();
    st.even_odd = true;
    let o = Object::new(doc.new_id(), ObjKind::Path(NodePath::from_bez(&ring)), Affine::IDENTITY, st.clone());
    doc.pages[0].layers[0].objects.push(o);
    let ring2 = Affine::translate((0.0, 30.0)) * ring.clone();
    st.even_odd = false;
    let o = Object::new(doc.new_id(), ObjKind::Path(NodePath::from_bez(&ring2)), Affine::IDENTITY, st);
    doc.pages[0].layers[0].objects.push(o);
    // middle: a red to blue linear gradient
    shape(&mut doc, Rect::new(48.0, 6.0, 96.0, 66.0).to_path(0.1), Fill::Gradient(Gradient::linear(Color::rgb(255, 0, 0), Color::rgb(0, 0, 255))), Outline::none());
    // right: a radial gradient, white center, black edge; and a big bitmap over the corner
    shape(&mut doc, Rect::new(102.0, 6.0, 138.0, 42.0).to_path(0.1), Fill::Gradient(Gradient::radial(Color::WHITE, Color::BLACK)), Outline::none());
    // noisy, so its PNG is split over several continued EMF+ object records
    let (w, h) = (600u32, 600u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for (i, p) in rgba.chunks_exact_mut(4).enumerate() {
        p.copy_from_slice(&[255, 200, ((i as u32).wrapping_mul(2_654_435_761) >> 29) as u8, 255]);
    }
    let png = encode_png(w, h, &rgba);
    assert!(png.len() > 2 * MAX_OBJECT_CHUNK, "the bitmap is large enough to be split ({})", png.len());
    place_image(&mut doc, image_data(w, h, rgba), 30.0, 18.0, Affine::translate((108.0, 48.0)));
    let emf = page_to_emf(&doc, 0, &EmfOptions::default());
    let (plus_img, gdi_img) = play(&emf, "check");
    for (name, img) in [("GDI+", &plus_img), ("GDI", &gdi_img)] {
        let px = |x: f64, y: f64| *img.get_pixel((x * 96.0 / 72.0) as u32, (y * 96.0 / 72.0) as u32);
        assert!(near(&px(10.0, 10.0), [0, 160, 0], 8), "{name}: the square is green: {:?}", px(10.0, 10.0));
        assert!(near(&px(24.0, 24.0), [255, 255, 255], 8), "{name}: even-odd leaves a hole: {:?}", px(24.0, 24.0));
        assert!(near(&px(24.0, 54.0), [0, 160, 0], 8), "{name}: nonzero fills the middle: {:?}", px(24.0, 54.0));
        let l = px(50.0, 36.0);
        let r = px(94.0, 36.0);
        assert!(l[0] > 200 && l[2] < 60 && r[2] > 200 && r[0] < 60, "{name}: red to blue: {l:?} {r:?}");
        let c = px(120.0, 24.0);
        let e = px(103.0, 7.0);
        assert!(c[0] > 220, "{name}: white center: {c:?}");
        assert!(e[0] < 60, "{name}: dark corner: {e:?}");
        assert!(near(&px(120.0, 56.0), [255, 200, 3], 10), "{name}: the bitmap: {:?}", px(120.0, 56.0));
        assert!(near(&px(140.0, 70.0), [255, 255, 255], 8), "{name}: nothing outside: {:?}", px(140.0, 70.0));
    }
    // transparency: GDI+ blends, GDI cannot
    let emf = page_to_emf(&sample(), 0, &EmfOptions::default());
    let (plus_img, _) = play(&emf, "alpha");
    let p = plus_img.get_pixel(96, 48);
    assert!(near(p, [255, 127, 127], 12), "GDI+ blends half-transparent red over white: {p:?}");
}
