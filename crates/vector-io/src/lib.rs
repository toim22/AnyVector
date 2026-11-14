//! File input/output for AnyVector: `.cdr` drawings (default), legacy `.vct`, SVG, PDF and raster formats,
//! and import of Corel metafiles (`.cmx`) and PDF files.

pub mod cdr;
pub mod corel;
pub mod cutter;
pub mod dxf;
pub mod flat;
pub mod hpgl;
pub mod image_io;
pub mod native;
pub mod pdf;
pub mod pdf_import;
pub mod patterns;
pub mod pdf_crypt;
pub mod ps_import;
pub mod svg_export;
pub mod svg_import;

use std::path::Path;
use vector_core::document::Document;
use vector_core::object::Object;

#[derive(Debug)]
pub enum IoError {
    Io(std::io::Error),
    Format(String),
    Unsupported(String),
}

impl std::fmt::Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IoError::Io(e) => write!(f, "{e}"),
            IoError::Format(s) => write!(f, "Invalid file: {s}"),
            IoError::Unsupported(s) => write!(f, "Unsupported file type: {s}"),
        }
    }
}

impl std::error::Error for IoError {}

impl From<std::io::Error> for IoError {
    fn from(e: std::io::Error) -> Self {
        IoError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, IoError>;

/// A file type entry for open/save dialogs.
#[derive(Clone, Debug)]
pub struct FileType {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub vector: bool,
}

/// "Save as type" entries of Save Drawing; the first is the default.
pub const SAVE_TYPES: &[FileType] = &[
    FileType { name: "CDR - CorelDRAW (*.cdr)", extensions: &["cdr"], vector: true },
    FileType { name: "VCT - AnyVector Drawing (*.vct)", extensions: &["vct"], vector: true },
];

pub const OPEN_TYPES: &[FileType] = &[
    FileType { name: "CDR - CorelDRAW (*.cdr)", extensions: &["cdr"], vector: true },
    FileType { name: "CDT - CorelDRAW Template (*.cdt;*.cdrt)", extensions: &["cdt", "cdrt"], vector: true },
    FileType { name: "CMX - Corel Presentation Exchange (*.cmx)", extensions: &["cmx"], vector: true },
    FileType { name: "AnyVector Drawing (*.vct)", extensions: &["vct"], vector: true },
    FileType { name: "Scalable Vector Graphics (*.svg)", extensions: &["svg", "svgz"], vector: true },
    FileType { name: "PDF - Adobe Portable Document Format (*.pdf)", extensions: &["pdf"], vector: true },
    FileType { name: "EPS - Encapsulated PostScript (*.eps)", extensions: &["eps", "epsf"], vector: true },
    FileType { name: "PS - PostScript (*.ps;*.prn)", extensions: &["ps", "prn"], vector: true },
    FileType { name: "AI - Adobe Illustrator (*.ai)", extensions: &["ai"], vector: true },
];

pub const IMPORT_TYPES: &[FileType] = &[
    FileType { name: "CDR - CorelDRAW (*.cdr)", extensions: &["cdr"], vector: true },
    FileType { name: "CDT - CorelDRAW Template (*.cdt;*.cdrt)", extensions: &["cdt", "cdrt"], vector: true },
    FileType { name: "CMX - Corel Presentation Exchange (*.cmx)", extensions: &["cmx"], vector: true },
    FileType { name: "AnyVector Drawing (*.vct)", extensions: &["vct"], vector: true },
    FileType { name: "SVG - Scalable Vector Graphics (*.svg;*.svgz)", extensions: &["svg", "svgz"], vector: true },
    FileType { name: "PDF - Adobe Portable Document Format (*.pdf)", extensions: &["pdf"], vector: true },
    FileType { name: "EPS - Encapsulated PostScript (*.eps)", extensions: &["eps", "epsf"], vector: true },
    FileType { name: "PS - PostScript (*.ps;*.prn)", extensions: &["ps", "prn"], vector: true },
    FileType { name: "AI - Adobe Illustrator (*.ai)", extensions: &["ai"], vector: true },
    FileType { name: "PNG - Portable Network Graphics (*.png)", extensions: &["png"], vector: false },
    FileType { name: "JPG - JPEG Bitmaps (*.jpg;*.jpeg;*.jfif)", extensions: &["jpg", "jpeg", "jfif"], vector: false },
    FileType { name: "BMP - Windows Bitmap (*.bmp)", extensions: &["bmp"], vector: false },
    FileType { name: "TIF - TIFF Bitmap (*.tif;*.tiff)", extensions: &["tif", "tiff"], vector: false },
    FileType { name: "WEBP - WebP Image (*.webp)", extensions: &["webp"], vector: false },
    FileType { name: "JXL - JPEG XL (*.jxl)", extensions: &["jxl"], vector: false },
    FileType { name: "GIF - CompuServe Bitmap (*.gif)", extensions: &["gif"], vector: false },
    FileType { name: "ICO - Windows Icon (*.ico)", extensions: &["ico"], vector: false },
    FileType { name: "TGA - Targa Bitmap (*.tga)", extensions: &["tga"], vector: false },
];

pub const EXPORT_TYPES: &[FileType] = &[
    FileType { name: "PDF - Portable Document Format (*.pdf)", extensions: &["pdf"], vector: true },
    FileType { name: "SVG - Scalable Vector Graphics (*.svg)", extensions: &["svg"], vector: true },
    FileType { name: "PNG - Portable Network Graphics (*.png)", extensions: &["png"], vector: false },
    FileType { name: "JPG - JPEG Bitmaps (*.jpg)", extensions: &["jpg", "jpeg"], vector: false },
    FileType { name: "BMP - Windows Bitmap (*.bmp)", extensions: &["bmp"], vector: false },
    FileType { name: "TIF - TIFF Bitmap (*.tif)", extensions: &["tif", "tiff"], vector: false },
    FileType { name: "WEBP - WebP Image (*.webp)", extensions: &["webp"], vector: false },
    FileType { name: "GIF - CompuServe Bitmap (*.gif)", extensions: &["gif"], vector: false },
    FileType { name: "PLT - HPGL Plotter File (*.plt;*.hgl)", extensions: &["plt", "hgl"], vector: true },
    FileType { name: "DXF - AutoCAD (*.dxf)", extensions: &["dxf"], vector: true },
    FileType { name: "JXL - JPEG XL, lossless (*.jxl)", extensions: &["jxl"], vector: false },
];

pub fn extension(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// The extension a file is read by. Illustrator files saved since CS are PDF files with an
/// `.ai` name; those go to the PDF importer, older PostScript-based ones to the EPS importer.
fn dispatch_ext(path: &Path) -> String {
    let ext = extension(path);
    if ext == "ai" && is_pdf_file(path) {
        return "pdf".into();
    }
    ext
}

fn is_pdf_file(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 1024];
    let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut head)).unwrap_or(0);
    head[..n].windows(5).any(|w| w == b"%PDF-")
}

/// Open a document file (native or SVG).
pub fn open(path: &Path) -> Result<Document> {
    match dispatch_ext(path).as_str() {
        "cdr" | "cdt" | "cdrt" => cdr::load(path),
        "cmx" => corel::cmx::load_from_bytes(&std::fs::read(path)?),
        "eps" | "epsf" | "ps" | "prn" | "ai" => ps_import::load_with_notes(&std::fs::read(path)?).map(|(d, _)| d),
        "vct" => native::load(path),
        "pdf" => pdf_import::load_with_notes(&std::fs::read(path)?, &Default::default()).map(|(d, _)| d),
        "svg" | "svgz" => {
            let data = std::fs::read(path)?;
            svg_import::document_from_svg(&data)
        }
        e if image_io::is_image_ext(e) => {
            let bytes = std::fs::read(path)?;
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let img = image_io::decode(&bytes, &name)?;
            let mut doc = Document::new(img.width as f64 * 72.0 / 96.0, img.height as f64 * 72.0 / 96.0);
            let obj = image_io::image_object(&mut doc, img, 96.0);
            doc.pages[0].layers[0].objects.push(obj);
            Ok(doc)
        }
        e => Err(IoError::Unsupported(e.to_string())),
    }
}

/// Open a document file and say what could not be brought in exactly, one line each (see
/// [`cdr::load_with_notes`]). Formats that never approximate give no lines.
pub fn open_with_notes(path: &Path) -> Result<(Document, Vec<String>)> {
    match dispatch_ext(path).as_str() {
        "cdr" | "cdt" | "cdrt" => cdr::load_with_notes(&std::fs::read(path)?),
        "cmx" => corel::cmx::load_with_report(&std::fs::read(path)?).map(|(d, r)| (d, r.lines())),
        "pdf" => pdf_import::load_with_notes(&std::fs::read(path)?, &Default::default()),
        "eps" | "epsf" | "ps" | "prn" | "ai" => ps_import::load_with_notes(&std::fs::read(path)?),
        _ => open(path).map(|d| (d, vec![])),
    }
}

/// Import a file into an existing document, like [`import`], and say what could not be brought
/// in exactly.
pub fn import_with_notes(doc: &mut Document, path: &Path) -> Result<(Vec<Object>, Vec<String>)> {
    match dispatch_ext(path).as_str() {
        "cdr" | "cdt" | "cdrt" => {
            let (other, notes) = cdr::load_with_notes(&std::fs::read(path)?)?;
            Ok((merge_document(doc, other), notes))
        }
        "cmx" => {
            let (other, report) = corel::cmx::load_with_report(&std::fs::read(path)?)?;
            Ok((merge_document(doc, other), report.lines()))
        }
        "pdf" => {
            let opts = pdf_import::PdfImportOptions { pages: vec![0], text_as_curves: false };
            let (mut pages, notes) = pdf_import::import_pages(doc, &std::fs::read(path)?, &opts)?;
            Ok((pages.pop().map(|p| p.0).unwrap_or_default(), notes))
        }
        "eps" | "epsf" | "ps" | "prn" | "ai" => ps_import::import(doc, &std::fs::read(path)?),
        _ => import(doc, path).map(|o| (o, vec![])),
    }
}

/// The preview stored in a drawing (`.cdr` or `.vct`), as RGBA pixels.
pub fn read_thumbnail(path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    match extension(path).as_str() {
        "cdr" | "cdt" | "cdrt" => cdr::read_thumbnail(path).or_else(|| native::read_thumbnail(path)).or_else(|| corel::cmx::thumbnail(&std::fs::read(path).ok()?)),
        "cmx" => corel::cmx::thumbnail(&std::fs::read(path).ok()?),
        _ => native::read_thumbnail(path),
    }
}

/// Whether a path is a drawing that can be saved back in place (native formats).
///
/// A `.cdr` saved by CorelDRAW is not one: it opens as a new drawing, so that saving cannot
/// replace the original with a file CorelDRAW no longer reads.
pub fn is_drawing(path: &Path) -> bool {
    match extension(path).as_str() {
        "vct" => true,
        "cdr" => match std::fs::read(path) {
            Ok(bytes) => cdr::detect(&bytes) != cdr::CdrKind::CorelDraw,
            Err(_) => true,
        },
        _ => false,
    }
}

/// Import a file into an existing document, returning the new top-level objects (not yet placed).
pub fn import(doc: &mut Document, path: &Path) -> Result<Vec<Object>> {
    let ext = dispatch_ext(path);
    let bytes = std::fs::read(path)?;
    match ext.as_str() {
        "cdr" | "cdt" | "cdrt" => {
            let other = cdr::load_from_bytes(&bytes)?;
            Ok(merge_document(doc, other))
        }
        "cmx" => {
            let other = corel::cmx::load_from_bytes(&bytes)?;
            Ok(merge_document(doc, other))
        }
        "vct" => {
            let other = native::load(path)?;
            Ok(merge_document(doc, other))
        }
        "eps" | "epsf" | "ps" | "prn" | "ai" => ps_import::import(doc, &bytes).map(|(o, _)| o),
        "svg" | "svgz" => {
            let other = svg_import::document_from_svg(&bytes)?;
            Ok(merge_document(doc, other))
        }
        "pdf" => {
            let opts = pdf_import::PdfImportOptions { pages: vec![0], text_as_curves: false };
            Ok(pdf_import::import_pages(doc, &bytes, &opts)?.0.pop().map(|p| p.0).unwrap_or_default())
        }
        e if image_io::is_image_ext(e) => {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let img = image_io::decode(&bytes, &name)?;
            Ok(vec![image_io::image_object(doc, img, 96.0)])
        }
        e => Err(IoError::Unsupported(e.to_string())),
    }
}

/// Move all objects of the first page of `other` into `doc` (with fresh ids and remapped images),
/// grouping them if there is more than one.
pub fn merge_document(doc: &mut Document, other: Document) -> Vec<Object> {
    let mut image_map = std::collections::HashMap::new();
    for (id, data) in other.images.iter() {
        let nid = doc.add_image(data.clone());
        image_map.insert(*id, nid);
    }
    let mut sym_map = std::collections::HashMap::new();
    for s in &other.symbols {
        let mut s2 = s.clone();
        let nid = doc.new_id();
        sym_map.insert(s.id, nid);
        s2.id = nid;
        for o in s2.objects.iter_mut() {
            doc.reassign_ids(o);
        }
        doc.symbols.push(s2);
    }
    let mut out = vec![];
    if let Some(p) = other.pages.into_iter().next() {
        for l in p.layers {
            for mut o in l.objects {
                doc.reassign_ids(&mut o);
                o.walk_mut(&mut |x| match &mut x.kind {
                    vector_core::ObjKind::Image(i) => {
                        if let Some(n) = image_map.get(&i.image) {
                            i.image = *n;
                        }
                    }
                    vector_core::ObjKind::Symbol(s) => {
                        if let Some(n) = sym_map.get(s) {
                            *s = *n;
                        }
                    }
                    _ => {}
                });
                out.push(o);
            }
        }
    }
    out
}
