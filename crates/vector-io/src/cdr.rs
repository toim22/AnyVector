//! `.cdr` drawings: the default document format of AnyVector.
//!
//! Following the container design in the reverse-engineering notes
//! (`docs/03-dll-core-document.md`, "O que reimplementar"), a drawing is a ZIP container with
//! typed parts and XMP metadata:
//!
//! ```text
//! drawing.cdr (zip)
//!   mimetype                    application/vnd.vector.cdr (stored, first entry)
//!   META-INF/manifest.xml       every part with its media type, plus the format version
//!   document.xml                document schema: pages, layers, sizes, color settings
//!   content/document.json       the object graph (pages, layers, objects, styles...)
//!   metadata.xmp                XMP/RDF: title, subject, rating, creator tool, dates
//!   resources/images/<id>.png   bitmaps
//!   resources/fonts/<file>      embedded fonts (optional)
//!   resources/profiles/<file>   embedded ICC color profiles (optional)
//!   previews/thumbnail.png      preview of the first page (optional)
//! ```
//!
//! This is AnyVector's own container. Drawings saved by CorelDRAW itself use a different,
//! undocumented internal format (a `content/riffData.cdr` RIFF stream); those are detected and
//! reported as not supported rather than misread.

use crate::{image_io, IoError, Result};
use std::io::{Read, Seek, Write};
use std::path::Path;
use std::sync::Arc;
use vector_core::document::Document;
use zip::write::SimpleFileOptions;

pub const MIMETYPE: &str = "application/vnd.vector.cdr";
/// Current container format version (Save Drawing ▸ Version).
pub const FORMAT_VERSION: &str = "0.1 (2026.0)";

/// A file stored inside the container (font or color profile).
#[derive(Clone, Debug)]
pub struct EmbeddedFile {
    pub name: String,
    pub data: Arc<Vec<u8>>,
}

/// Options chosen in Save Drawing (and its Advanced... dialog).
#[derive(Clone, Debug)]
pub struct SaveOptions {
    pub version: String,
    pub fonts: Vec<EmbeddedFile>,
    pub color_profiles: Vec<EmbeddedFile>,
    /// Compress the object data (content/document.json, document.xml).
    pub object_compression: bool,
    /// Compress bitmaps (PNG/JPEG are already compressed; this deflates them further).
    pub bitmap_compression: bool,
    /// Optimize for size (maximum compression) instead of speed.
    pub optimize_for_size: bool,
    pub thumbnail: bool,
    /// Keep saved views and other presentation attributes.
    pub presentation_attributes: bool,
}

impl Default for SaveOptions {
    fn default() -> Self {
        SaveOptions { version: FORMAT_VERSION.into(), fonts: vec![], color_profiles: vec![], object_compression: true, bitmap_compression: true, optimize_for_size: false, thumbnail: true, presentation_attributes: true }
    }
}

fn zerr(e: zip::result::ZipError) -> IoError {
    IoError::Format(e.to_string())
}

pub fn save(doc: &Document, path: &Path, opts: &SaveOptions) -> Result<()> {
    let bytes = save_to_bytes(doc, opts)?;
    let tmp = path.with_extension("cdr.tmp");
    std::fs::write(&tmp, &bytes)?;
    if std::fs::rename(&tmp, path).is_err() {
        std::fs::copy(&tmp, path)?;
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(())
}

pub fn save_to_bytes(doc: &Document, opts: &SaveOptions) -> Result<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    write_zip(doc, opts, &mut buf)?;
    Ok(buf.into_inner())
}

/// Characters that must be escaped in XML text and attributes.
pub fn xml_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' && c != '\r' => {}
            c => o.push(c),
        }
    }
    o
}

/// ISO 8601 UTC timestamp ("2026-11-08T14:05:00Z").
pub fn iso_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, (rem % 3600) / 60, rem % 60)
}

/// The XMP packet with the document's descriptive metadata.
pub fn xmp(doc: &Document, opts: &SaveOptions) -> String {
    let i = &doc.info;
    let now = iso_now();
    let alt = |tag: &str, v: &str| {
        if v.is_empty() {
            String::new()
        } else {
            format!("   <{tag}><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></{tag}>\n", xml_escape(v))
        }
    };
    let mut s = String::new();
    s.push_str("<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n");
    s.push_str("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n");
    s.push_str("  <rdf:Description rdf:about=\"\"\n    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n    xmlns:vct=\"https://github.com/toim22/Vector/ns/cdr/1.0/\">\n");
    s.push_str(&alt("dc:title", &i.title));
    s.push_str(&alt("dc:description", &i.subject));
    if !i.subject.is_empty() {
        s.push_str(&format!("   <dc:subject><rdf:Bag><rdf:li>{}</rdf:li></rdf:Bag></dc:subject>\n", xml_escape(&i.subject)));
    }
    if !i.author.is_empty() {
        s.push_str(&format!("   <dc:creator><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:creator>\n", xml_escape(&i.author)));
    }
    s.push_str(&alt("dc:rights", &i.copyright));
    s.push_str(&format!("   <xmp:Rating>{}</xmp:Rating>\n", i.rating.min(5)));
    s.push_str(&format!("   <xmp:CreatorTool>{} {}</xmp:CreatorTool>\n", vector_core::APP_NAME, env!("CARGO_PKG_VERSION")));
    s.push_str(&format!("   <xmp:ModifyDate>{now}</xmp:ModifyDate>\n   <xmp:MetadataDate>{now}</xmp:MetadataDate>\n"));
    s.push_str(&format!("   <vct:FormatVersion>{}</vct:FormatVersion>\n", xml_escape(&opts.version)));
    s.push_str(&format!("   <vct:ColorProfiles>{}</vct:ColorProfiles>\n", xml_escape(&doc.settings.color.summary())));
    s.push_str("  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>\n");
    s
}

/// Human-readable document schema (pages, layers, settings).
fn document_xml(doc: &Document, opts: &SaveOptions) -> String {
    let c = &doc.settings.color;
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<document xmlns=\"https://github.com/toim22/Vector/ns/cdr/1.0/\" version=\"{}\" unit=\"{}\" resolution=\"{}\" pageView=\"{}\">\n",
        xml_escape(&opts.version),
        doc.settings.unit.name(),
        doc.settings.dpi,
        if doc.settings.multipage_view { "multipage" } else { "single" }
    );
    s.push_str(&format!(
        " <colors primary=\"{}\" rgbProfile=\"{}\" cmykProfile=\"{}\" grayProfile=\"{}\" intent=\"{}\"/>\n",
        if c.cmyk { "CMYK" } else { "RGB" },
        xml_escape(&c.rgb_profile),
        xml_escape(&c.cmyk_profile),
        xml_escape(&c.gray_profile),
        c.intent.name()
    ));
    if doc.settings.bleed > 0.0 {
        s.push_str(&format!(" <bleed size=\"{}\" show=\"{}\"/>\n", doc.settings.bleed, doc.settings.show_bleed));
    }
    s.push_str(" <pages>\n");
    for (i, p) in doc.pages.iter().enumerate() {
        s.push_str(&format!("  <page index=\"{}\" name=\"{}\" width=\"{}\" height=\"{}\">\n", i + 1, xml_escape(&p.name), p.width, p.height));
        for l in &p.layers {
            s.push_str(&format!("   <layer name=\"{}\" objects=\"{}\"/>\n", xml_escape(&l.name), l.objects.len()));
        }
        s.push_str("  </page>\n");
    }
    s.push_str(" </pages>\n</document>\n");
    s
}

fn write_zip<W: Write + Seek>(doc: &Document, opts: &SaveOptions, w: &mut W) -> Result<()> {
    let mut z = zip::ZipWriter::new(w);
    let level = if opts.optimize_for_size { 9 } else { 3 };
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).compression_level(Some(level));
    let objects = if opts.object_compression { deflated } else { stored };
    let bitmaps = if opts.bitmap_compression { deflated } else { stored };
    let mut parts: Vec<(String, &str)> = vec![];

    z.start_file("mimetype", stored).map_err(zerr)?;
    z.write_all(MIMETYPE.as_bytes())?;

    let used = doc.referenced_images();
    let mut doc2 = doc.clone();
    doc2.images.retain(|id, _| used.contains(id));
    if !opts.presentation_attributes {
        doc2.views.clear();
    }
    let json = serde_json::to_vec(&doc2).map_err(|e| IoError::Format(e.to_string()))?;
    z.start_file("content/document.json", objects).map_err(zerr)?;
    z.write_all(&json)?;
    parts.push(("content/document.json".into(), "application/json"));

    z.start_file("document.xml", objects).map_err(zerr)?;
    z.write_all(document_xml(&doc2, opts).as_bytes())?;
    parts.push(("document.xml".into(), "application/xml"));

    z.start_file("metadata.xmp", deflated).map_err(zerr)?;
    z.write_all(xmp(&doc2, opts).as_bytes())?;
    parts.push(("metadata.xmp".into(), "application/rdf+xml"));

    for (id, img) in doc2.images.iter() {
        let (bytes, ext) = image_io::encoded_bytes(img);
        let name = format!("resources/images/{id}.{ext}");
        z.start_file(name.as_str(), bitmaps).map_err(zerr)?;
        z.write_all(&bytes)?;
        parts.push((name, if ext == "jpg" { "image/jpeg" } else { "image/png" }));
    }
    for f in &opts.fonts {
        let name = format!("resources/fonts/{}", f.name);
        z.start_file(name.as_str(), deflated).map_err(zerr)?;
        z.write_all(&f.data)?;
        parts.push((name, "font/sfnt"));
    }
    for f in &opts.color_profiles {
        let name = format!("resources/profiles/{}", f.name);
        z.start_file(name.as_str(), deflated).map_err(zerr)?;
        z.write_all(&f.data)?;
        parts.push((name, "application/vnd.iccprofile"));
    }
    if opts.thumbnail {
        if let Some(thumb) = crate::native::thumbnail(doc, 256) {
            z.start_file("previews/thumbnail.png", stored).map_err(zerr)?;
            z.write_all(&thumb)?;
            parts.push(("previews/thumbnail.png".into(), "image/png"));
        }
    }
    let mut m = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<manifest xmlns=\"https://github.com/toim22/Vector/ns/cdr/1.0/\" version=\"{}\" generator=\"{} {}\">\n", xml_escape(&opts.version), vector_core::APP_NAME, env!("CARGO_PKG_VERSION"));
    for (p, t) in &parts {
        m.push_str(&format!(" <part path=\"{}\" type=\"{}\"/>\n", xml_escape(p), t));
    }
    m.push_str("</manifest>\n");
    z.start_file("META-INF/manifest.xml", deflated).map_err(zerr)?;
    z.write_all(m.as_bytes())?;
    z.finish().map_err(zerr)?;
    Ok(())
}

/// What kind of `.cdr` a file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CdrKind {
    /// AnyVector's container.
    AnyVector,
    /// A `.vct`-style archive that was renamed to `.cdr`.
    LegacyVector,
    /// Written by CorelDRAW (RIFF stream, bare or zipped).
    CorelDraw,
    Unknown,
}

pub fn detect(bytes: &[u8]) -> CdrKind {
    // CorelDRAW drawings ("CDR") and templates ("CDT"); versions 1 and 2 start with "WL"
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && (&bytes[8..11] == b"CDR" || &bytes[8..11] == b"CDT" || &bytes[8..11] == b"cdr") {
        return CdrKind::CorelDraw;
    }
    if bytes.len() >= 2 && &bytes[0..2] == b"WL" {
        return CdrKind::CorelDraw;
    }
    let Ok(mut z) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else { return CdrKind::Unknown };
    if z.by_name("content/document.json").is_ok() {
        CdrKind::AnyVector
    } else if z.by_name("document.json").is_ok() {
        CdrKind::LegacyVector
    } else if z.by_name("content/riffData.cdr").is_ok() || z.by_name("content/root.dat").is_ok() {
        CdrKind::CorelDraw
    } else {
        CdrKind::Unknown
    }
}

pub fn load(path: &Path) -> Result<Document> {
    let bytes = std::fs::read(path)?;
    load_from_bytes(&bytes)
}

pub fn load_from_bytes(bytes: &[u8]) -> Result<Document> {
    match detect(bytes) {
        CdrKind::AnyVector => {}
        CdrKind::LegacyVector => return crate::native::load_from_bytes(bytes),
        CdrKind::CorelDraw => return crate::corel::load_from_bytes(bytes),
        CdrKind::Unknown => return Err(IoError::Format("not a .cdr drawing".into())),
    }
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(zerr)?;
    let mut json = vec![];
    z.by_name("content/document.json").map_err(zerr)?.read_to_end(&mut json)?;
    let mut doc: Document = serde_json::from_slice(&json).map_err(|e| IoError::Format(e.to_string()))?;
    let ids: Vec<u64> = doc.images.keys().copied().collect();
    for id in ids {
        let mut data = None;
        for ext in ["png", "jpg"] {
            if let Ok(mut f) = z.by_name(&format!("resources/images/{id}.{ext}")) {
                let mut b = vec![];
                f.read_to_end(&mut b)?;
                data = Some(b);
                break;
            }
        }
        if let Some(b) = data {
            if let Ok(dec) = image_io::decode(&b, "") {
                if let Some(img) = doc.images.get_mut(&id) {
                    img.width = dec.width;
                    img.height = dec.height;
                    img.rgba = dec.rgba;
                    img.encoded = if dec.encoded.is_empty() { Arc::new(b) } else { dec.encoded };
                }
            }
        }
    }
    doc.ensure_next_id();
    Ok(doc)
}

/// Names of the fonts embedded in a `.cdr` drawing.
pub fn embedded_fonts<R: Read + Seek>(r: R) -> Vec<String> {
    let Ok(z) = zip::ZipArchive::new(r) else { return vec![] };
    z.file_names().filter_map(|n| n.ok()).filter_map(|n| n.strip_prefix("resources/fonts/").map(|s| s.to_string())).collect()
}

/// Read only the preview of a `.cdr` drawing.
pub fn read_thumbnail(path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let f = std::fs::File::open(path).ok()?;
    let mut z = zip::ZipArchive::new(f).ok()?;
    let mut b = vec![];
    z.by_name("previews/thumbnail.png").ok()?.read_to_end(&mut b).ok()?;
    let img = image::load_from_memory(&b).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((w, h, img.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_with_metadata() {
        let mut doc = Document::new(612.0, 792.0);
        doc.info.title = "Logo <draft> & co".into();
        doc.info.subject = "Branding".into();
        doc.info.rating = 4;
        doc.settings.color.cmyk = false;
        let opts = SaveOptions { fonts: vec![EmbeddedFile { name: "x.ttf".into(), data: Arc::new(vec![1, 2, 3]) }], ..Default::default() };
        let bytes = save_to_bytes(&doc, &opts).unwrap();
        assert_eq!(detect(&bytes), CdrKind::AnyVector);
        let back = load_from_bytes(&bytes).unwrap();
        assert_eq!(back.info.title, doc.info.title);
        assert_eq!(back.info.rating, 4);
        assert!(!back.settings.color.cmyk);
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        // mimetype first and stored, like other container formats
        assert_eq!(z.by_index(0).unwrap().name().unwrap(), "mimetype");
        let mut x = String::new();
        z.by_name("metadata.xmp").unwrap().read_to_string(&mut x).unwrap();
        assert!(x.contains("Logo &lt;draft&gt; &amp; co"));
        assert!(x.contains("<xmp:Rating>4</xmp:Rating>"));
        let mut m = String::new();
        z.by_name("META-INF/manifest.xml").unwrap().read_to_string(&mut m).unwrap();
        assert!(m.contains("resources/fonts/x.ttf"));
        assert_eq!(embedded_fonts(std::io::Cursor::new(&bytes)), vec!["x.ttf".to_string()]);
    }

    #[test]
    fn detects_coreldraw_files() {
        let mut riff = b"RIFF\0\0\0\0CDRvvers".to_vec();
        riff.extend_from_slice(&[0; 16]);
        assert_eq!(detect(&riff), CdrKind::CorelDraw);
        // a CorelDRAW file with nothing in it is reported as damaged, not as unsupported
        assert!(matches!(load_from_bytes(&riff), Err(IoError::Format(_))));
        assert!(matches!(load_from_bytes(b"WL some old drawing"), Err(IoError::Unsupported(_))));
        // a .vct archive renamed to .cdr still opens
        let v = crate::native::save_to_bytes(&Document::new(100.0, 100.0)).unwrap();
        assert_eq!(detect(&v), CdrKind::LegacyVector);
        assert!(load_from_bytes(&v).is_ok());
    }
}
