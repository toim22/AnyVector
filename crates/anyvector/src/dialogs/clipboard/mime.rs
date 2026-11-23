//! Clipboard formats by MIME type, as X11 and Wayland name them: what Copy offers on Linux and
//! how Paste picks among what another program offers. Pure functions, so they are tested on every
//! platform; `linux.rs` moves the bytes.

use super::{ClipFormat, ClipImage, OWN_FORMAT, SVG_FORMAT};
use std::path::PathBuf;

/// AnyVector's own format on Linux: the SVG with the `data-vector-clip` marker, as the format
/// registered as "AnyVector Graphic" holds it on Windows (Paste brings the objects back).
pub const OWN_MIME: &str = "application/x-anyvector-graphic";
/// SVG pictures: Inkscape, LibreOffice and browsers paste this one.
pub const SVG_MIME: &str = "image/svg+xml";
pub const PNG_MIME: &str = "image/png";

/// SVG pictures, best first (Inkscape adds its own name for the same SVG).
pub const SVG_MIMES: &[&str] = &[SVG_MIME, "image/x-inkscape-svg"];
/// Bitmaps, best first: the lossless formats before JPEG.
pub const IMAGE_MIMES: &[&str] = &[PNG_MIME, "image/bmp", "image/x-bmp", "image/x-ms-bmp", "image/tiff", "image/webp", "image/jpeg", "image/gif"];
/// Plain text, best first (UTF8_STRING and STRING are the X11 names).
pub const TEXT_MIMES: &[&str] = &["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING"];
/// Enhanced Metafiles. Nothing on Linux offers CF_ENHMETAFILE; LibreOffice offers its drawings as
/// EMF under its own name, which pastes as curves like an Office metafile on Windows.
pub const EMF_MIMES: &[&str] = &["image/emf", "image/x-emf", "application/x-openoffice-emf;windows_formatname=\"Image EMF\""];
/// Files copied in a file manager: the standard list, then GNOME's own (the same list after a
/// "copy" or "cut" line).
pub const FILE_MIMES: &[&str] = &["text/uri-list", "x-special/gnome-copied-files"];

/// The MIME type a format made by Copy is offered under on Linux. None for the formats with no
/// Linux counterpart: there are no metafiles on the Linux clipboard, and the PNG stands for the
/// DIB.
pub fn offer_mime(f: ClipFormat) -> Option<&'static str> {
    match f {
        ClipFormat::Named(n) if n == OWN_FORMAT => Some(OWN_MIME),
        ClipFormat::Named(n) if n == SVG_FORMAT => Some(SVG_MIME),
        ClipFormat::Named(n) if n.eq_ignore_ascii_case("PNG") => Some(PNG_MIME),
        ClipFormat::Named(n) if n.contains('/') => Some(n),
        _ => None,
    }
}

/// What Copy offers on Linux, in the order of [`super::copy_formats`]: AnyVector's own format,
/// the SVG and the PNG. Like on Windows there is no plain text (a text editor would get the SVG
/// source).
pub fn offers(formats: &[(ClipFormat, Vec<u8>)]) -> Vec<(&'static str, &[u8])> {
    let mut v: Vec<(&'static str, &[u8])> = vec![];
    for (f, data) in formats {
        if let Some(m) = offer_mime(*f) {
            if !v.iter().any(|(x, _)| *x == m) {
                v.push((m, data.as_slice()));
            }
        }
    }
    v
}

/// The MIME types a format registered by name on Windows is read from on Linux, best first.
pub fn read_mimes(name: &str) -> Vec<&str> {
    if name == OWN_FORMAT {
        vec![OWN_MIME]
    } else if name == SVG_FORMAT {
        SVG_MIMES.to_vec()
    } else if name.eq_ignore_ascii_case("PNG") {
        vec![PNG_MIME]
    } else {
        vec![name]
    }
}

/// A MIME type compared loosely: case and spaces ignored ("text/plain; charset=UTF-8").
fn norm(m: &str) -> String {
    m.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase()
}

/// The first of `wanted` (best first) that the clipboard offers, spelled as it is offered.
pub fn pick<'a>(offered: &'a [String], wanted: &[&str]) -> Option<&'a str> {
    wanted.iter().find_map(|w| {
        let w = norm(w);
        offered.iter().find(|o| norm(o) == w).map(|s| s.as_str())
    })
}

/// Which kinds of content the clipboard offers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offered {
    pub own: bool,
    pub svg: bool,
    pub image: bool,
    pub text: bool,
    pub files: bool,
    pub emf: bool,
}

impl Offered {
    pub fn of(types: &[String]) -> Self {
        let has = |list: &[&str]| pick(types, list).is_some();
        Offered { own: has(&[OWN_MIME]), svg: has(SVG_MIMES), image: has(IMAGE_MIMES), text: has(TEXT_MIMES), files: has(FILE_MIMES), emf: has(EMF_MIMES) }
    }

    /// Something Paste can use (the Edit menu enables Paste and Paste Special).
    pub fn any(&self) -> bool {
        self.own || self.svg || self.image || self.text || self.files || self.emf
    }

    /// Whether Paste opens the copied files: a file manager's list, with no picture that Paste
    /// takes first (AnyVector's own copy, an SVG or a bitmap). The files come before the text
    /// (a file manager adds the paths as text).
    pub fn paste_files(&self) -> bool {
        self.files && !self.own && !self.svg && !self.image
    }

    /// Whether text that egui pastes is the SVG source: an SVG with no plain text (AnyVector's
    /// own copy, Inkscape), which `wl-clipboard-rs` counts as text on Wayland.
    pub fn text_is_svg_source(&self) -> bool {
        self.svg && !self.text
    }
}

/// Text in the given format: STRING is Latin-1, the rest UTF-8 (a trailing NUL dropped).
pub fn decode_text(mime: &str, data: &[u8]) -> String {
    let mut end = data.len();
    while end > 0 && data[end - 1] == 0 {
        end -= 1;
    }
    let data = &data[..end];
    if norm(mime) == "string" {
        data.iter().map(|&b| b as char).collect()
    } else {
        String::from_utf8_lossy(data).into_owned()
    }
}

/// A bitmap in the given format (the PNG bytes are kept, as on Windows).
pub fn decode_image(mime: &str, data: &[u8]) -> Option<ClipImage> {
    let format = match norm(mime).as_str() {
        "image/png" => return super::decode_png(data),
        "image/bmp" | "image/x-bmp" | "image/x-ms-bmp" => image::ImageFormat::Bmp,
        "image/jpeg" | "image/jpg" => image::ImageFormat::Jpeg,
        "image/tiff" => image::ImageFormat::Tiff,
        "image/webp" => image::ImageFormat::WebP,
        "image/gif" => image::ImageFormat::Gif,
        _ => image::guess_format(data).ok()?,
    };
    let img = image::load_from_memory_with_format(data, format).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    let mut rgba = img.into_raw();
    super::fix_zero_alpha(&mut rgba);
    (width > 0 && height > 0).then_some(ClipImage { width, height, rgba, png: vec![] })
}

/// The local files a `text/uri-list` names (RFC 2483: one URI a line, `#` starts a comment), or
/// GNOME's `x-special/gnome-copied-files` (the same after a "copy"/"cut" line). Other URIs
/// (http…) are left out.
pub fn uri_list_paths(data: &[u8]) -> Vec<PathBuf> {
    String::from_utf8_lossy(data).lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).filter_map(file_uri_path).collect()
}

/// The path of a `file:` URI on this computer (`file:///p` or `file://localhost/p`).
pub fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.get(..7).filter(|s| s.eq_ignore_ascii_case("file://")).map(|_| &uri[7..])?;
    let path = match rest.find('/') {
        Some(0) => rest,
        Some(i) if rest[..i].eq_ignore_ascii_case("localhost") => &rest[i..],
        _ => return None,
    };
    let bytes = percent_decode(path);
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(PathBuf::from(std::ffi::OsStr::from_bytes(&bytes)))
    }
    #[cfg(not(unix))]
    {
        Some(PathBuf::from(String::from_utf8_lossy(&bytes).into_owned()))
    }
}

/// `%XX` escapes to bytes (a malformed escape stays as it is).
fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn list(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Copy offers AnyVector's own format, the SVG and the PNG, in that order; the metafile and
    /// the DIB have no Linux counterpart and there is no text, as on Windows.
    #[test]
    fn copy_offers_by_mime_type() {
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>";
        let px = [255u8, 0, 0, 255];
        let formats = super::super::copy_formats(svg, vec![1, 2, 3, 4], Some((1, 1, &px)));
        let o = offers(&formats);
        let mimes: Vec<&str> = o.iter().map(|(m, _)| *m).collect();
        assert_eq!(mimes, vec![OWN_MIME, SVG_MIME, PNG_MIME]);
        assert!(String::from_utf8_lossy(o[0].1).contains("data-vector-clip"), "the own format keeps the marker");
        assert_eq!(o[1].1, svg.as_bytes(), "other programs get the plain SVG");
        assert_eq!(&o[2].1[1..4], b"PNG");
        // a metafile alone has nothing to offer on Linux
        assert!(offers(&[(ClipFormat::EnhMetafile, vec![1])]).is_empty());
        assert!(offers(&[(ClipFormat::Dib, vec![1])]).is_empty());
        // the same format twice is offered once
        assert_eq!(offers(&[(ClipFormat::Named("PNG"), vec![1]), (ClipFormat::Named("image/png"), vec![2])]).len(), 1);
    }

    #[test]
    fn named_formats_read_by_mime_type() {
        assert_eq!(read_mimes(OWN_FORMAT), vec![OWN_MIME]);
        assert_eq!(read_mimes(SVG_FORMAT), vec!["image/svg+xml", "image/x-inkscape-svg"]);
        assert_eq!(read_mimes("PNG"), vec!["image/png"]);
        assert_eq!(read_mimes("text/html"), vec!["text/html"]);
    }

    /// The best format the other program offers wins, whatever the order it lists them in, and
    /// it is requested with the spelling it was offered with.
    #[test]
    fn pick_prefers_the_best_offered() {
        let gimp = list(&["TARGETS", "image/jpeg", "image/bmp", "image/png", "TIMESTAMP"]);
        assert_eq!(pick(&gimp, IMAGE_MIMES), Some("image/png"));
        let jpeg_only = list(&["image/jpeg"]);
        assert_eq!(pick(&jpeg_only, IMAGE_MIMES), Some("image/jpeg"));
        let x11_text = list(&["TARGETS", "STRING", "UTF8_STRING"]);
        assert_eq!(pick(&x11_text, TEXT_MIMES), Some("UTF8_STRING"));
        let upper = list(&["text/plain; charset=UTF-8", "text/plain"]);
        assert_eq!(pick(&upper, TEXT_MIMES), Some("text/plain; charset=UTF-8"));
        let inkscape = list(&["image/x-inkscape-svg", "image/svg+xml", "image/png", "text/plain"]);
        assert_eq!(pick(&inkscape, SVG_MIMES), Some("image/svg+xml"));
        assert_eq!(pick(&list(&["image/x-inkscape-svg"]), SVG_MIMES), Some("image/x-inkscape-svg"));
        assert_eq!(pick(&list(&["application/x-other"]), IMAGE_MIMES), None);
        let lo = list(&["application/x-openoffice-emf;windows_formatname=\"Image EMF\"", "image/png"]);
        assert!(pick(&lo, EMF_MIMES).is_some());
    }

    /// What the Edit menu sees: Paste is enabled for anything Paste can use.
    #[test]
    fn offered_kinds() {
        let ours = Offered::of(&list(&[OWN_MIME, SVG_MIME, PNG_MIME, "TARGETS", "SAVE_TARGETS"]));
        assert_eq!(ours, Offered { own: true, svg: true, image: true, ..Default::default() });
        let files = Offered::of(&list(&["x-special/gnome-copied-files", "text/uri-list", "text/plain;charset=utf-8"]));
        assert!(files.files && files.text && !files.image);
        let nothing = Offered::of(&list(&["TARGETS", "TIMESTAMP", "MULTIPLE", "application/x-unknown"]));
        assert!(!nothing.any());
        assert!(!Offered::of(&[]).any());
        assert!(Offered::of(&list(&["image/x-emf"])).emf);
    }

    /// Paste's order: AnyVector's own copy, an SVG or a bitmap before copied files, the files
    /// before their paths as text. Text egui pastes is the SVG source when there is no plain text.
    #[test]
    fn paste_order_of_files_and_svg_text() {
        let nautilus = Offered::of(&list(&["x-special/gnome-copied-files", "text/uri-list", "UTF8_STRING"]));
        assert!(nautilus.paste_files());
        let viewer = Offered::of(&list(&["image/png", "text/uri-list"]));
        assert!(!viewer.paste_files(), "the bitmap comes first");
        assert!(!Offered::of(&list(&[OWN_MIME, SVG_MIME, PNG_MIME, "text/uri-list"])).paste_files());
        assert!(!Offered::of(&list(&["UTF8_STRING"])).paste_files());
        // AnyVector's copy and Inkscape's offer the SVG without plain text
        assert!(Offered::of(&list(&[OWN_MIME, SVG_MIME, PNG_MIME])).text_is_svg_source());
        assert!(Offered::of(&list(&["image/x-inkscape-svg", "image/png"])).text_is_svg_source());
        assert!(!Offered::of(&list(&[SVG_MIME, "text/plain;charset=utf-8"])).text_is_svg_source());
        assert!(!Offered::of(&list(&["UTF8_STRING"])).text_is_svg_source());
    }

    #[test]
    fn text_encodings() {
        assert_eq!(decode_text("UTF8_STRING", "Olá\0".as_bytes()), "Olá");
        assert_eq!(decode_text("text/plain;charset=utf-8", "naïve".as_bytes()), "naïve");
        // STRING is ISO Latin-1
        assert_eq!(decode_text("STRING", &[0x4f, 0x6c, 0xe1]), "Olá");
        assert_eq!(decode_text("text/plain", b""), "");
    }

    #[test]
    fn images_by_type() {
        let img = image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]));
        for (mime, format) in [("image/bmp", image::ImageFormat::Bmp), ("image/x-ms-bmp", image::ImageFormat::Bmp), ("image/tiff", image::ImageFormat::Tiff), ("image/png", image::ImageFormat::Png)] {
            let mut bytes = vec![];
            img.write_to(&mut std::io::Cursor::new(&mut bytes), format).unwrap();
            let c = decode_image(mime, &bytes).unwrap_or_else(|| panic!("{mime} did not decode"));
            assert_eq!((c.width, c.height), (3, 2), "{mime}");
            assert_eq!(&c.rgba[0..3], &[10, 20, 30], "{mime}");
            assert_eq!(c.png.is_empty(), mime != "image/png", "only PNG bytes are kept");
        }
        // JPEG has no alpha: it comes out opaque
        let mut jpeg = vec![];
        image::DynamicImage::ImageRgba8(img.clone()).to_rgb8().write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg).unwrap();
        let c = decode_image("image/jpeg", &jpeg).expect("jpeg");
        assert_eq!((c.width, c.height, c.rgba[3]), (3, 2, 255));
        assert!(decode_image("image/png", b"not a png").is_none());
    }

    /// Files copied in Nautilus (GNOME) and Dolphin (KDE): local paths, percent-decoded; web
    /// addresses and comments left out.
    #[test]
    fn copied_file_lists() {
        let dolphin = b"file:///home/ana/Desenhos/logo%20final.svg\r\nfile:///home/ana/Caf%C3%A9.png\r\n";
        assert_eq!(uri_list_paths(dolphin), vec![PathBuf::from("/home/ana/Desenhos/logo final.svg"), PathBuf::from("/home/ana/Café.png")]);
        let gnome = b"copy\nfile:///tmp/a.cdr\nfile:///tmp/b%25.svg";
        assert_eq!(uri_list_paths(gnome), vec![PathBuf::from("/tmp/a.cdr"), PathBuf::from("/tmp/b%.svg")]);
        let mixed = b"# a comment\nhttps://example.com/a.png\nfile://localhost/srv/x.svg\nfile://otherhost/y.svg\n";
        assert_eq!(uri_list_paths(mixed), vec![PathBuf::from("/srv/x.svg")]);
        assert_eq!(file_uri_path("FILE:///a/b"), Some(PathBuf::from("/a/b")));
        assert_eq!(file_uri_path("file:///bad%zzescape%"), Some(Path::new("/bad%zzescape%").to_path_buf()));
        assert_eq!(file_uri_path("/not/a/uri"), None);
    }
}
