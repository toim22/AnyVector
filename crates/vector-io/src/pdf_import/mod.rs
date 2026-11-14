//! PDF import (File ▸ Open and File ▸ Import): pages, vector paths, fills and outlines,
//! clipping (as PowerClips), images, transparency, axial and radial shadings and text.
//!
//! The file's object layer (cross-reference tables and streams, object streams, filters,
//! encryption with an empty user password) is read by lopdf; the content streams are run by
//! [`interp`]. Each PDF page becomes a page of the drawing (Open) or a group (Import).

mod color;
mod encodings;
mod font;
mod func;
mod image;
mod interp;
mod lexer;

use crate::{IoError, Result};
use kurbo::{Affine, Rect};
use lopdf::{Dictionary, Object};
use std::collections::BTreeMap;
use vector_core::color::NamedColor;
use vector_core::document::Document;
use vector_core::object::{ObjKind, Object as VObj};
use vector_core::style::{Outline, Style};

pub use image::ImageSkip;

/// What to bring in: the pages (0-based; empty = all) and how text comes in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PdfImportOptions {
    pub pages: Vec<usize>,
    /// Import text as curves (instead of editable text where the font is installed).
    pub text_as_curves: bool,
}

/// Something the importer left out or approximated.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Note {
    FontSubstituted(String),
    CurvesNotInstalled(String),
    CurvesFromInstalled(String),
    UnknownCMap(String),
    VerticalText,
    InvisibleText,
    TextClip,
    Image(ImageSkip),
    SoftMask,
    TilingPattern,
    MeshShading,
    Annotations,
}

impl Note {
    /// The note as the user reads it, for `count` occurrences. Each text is a key of the
    /// translation file (`{}` stands for a value), so keep the wording and the translation in step.
    pub fn describe(&self, count: usize) -> String {
        match self {
            Note::FontSubstituted(f) => format!("The font \"{}\" isn't installed and its outlines aren't in the file in a form AnyVector reads, so its text is shown in a substitute font", f),
            Note::CurvesNotInstalled(f) => format!("The font \"{}\" isn't installed, so its text came in as curves from the font embedded in the file", f),
            Note::CurvesFromInstalled(f) => format!("The font \"{}\" isn't embedded in the file, so its text was converted to curves with the installed font that stands in for it", f),
            Note::UnknownCMap(f) => format!("The font \"{}\" uses a character map AnyVector doesn't read, so some of its characters may be wrong", f),
            Note::VerticalText => "Vertical text came in as horizontal text".to_string(),
            Note::InvisibleText => "Invisible text (such as the recognized text over a scanned page) was left out".to_string(),
            Note::TextClip => "Text used as a clipping path was brought in as plain text, without clipping".to_string(),
            Note::Image(ImageSkip::Jpeg2000) => format!("Left out: {} image(s) in JPEG 2000 format", count),
            Note::Image(ImageSkip::Fax) => format!("Left out: {} image(s) in CCITT fax format", count),
            Note::Image(ImageSkip::Jbig2) => format!("Left out: {} image(s) in JBIG2 format", count),
            Note::Image(ImageSkip::Broken) => format!("Left out: {} image(s) that could not be read", count),
            Note::SoftMask => "Soft masks were left out, so some transparency is shown as opaque".to_string(),
            Note::TilingPattern => format!("{} pattern fill(s) came in as a uniform color", count),
            Note::MeshShading => format!("{} mesh or function shading(s) came in as a uniform color", count),
            Note::Annotations => "Annotations and form fields were left out".to_string(),
        }
    }
}

#[derive(Default)]
pub struct Notes(pub BTreeMap<Note, usize>);

impl Notes {
    pub fn lines(&self) -> Vec<String> {
        self.0.iter().map(|(n, c)| n.describe(*c)).collect()
    }
}

/// The object layer, with lookups that follow references.
pub struct Pdf<'a> {
    pub doc: &'a lopdf::Document,
}

static NULL: Object = Object::Null;

/// The most a single stream may grow to when it is decompressed.
const STREAM_LIMIT: usize = 1 << 30;

impl<'a> Pdf<'a> {
    pub fn deref(&self, mut o: &'a Object) -> &'a Object {
        for _ in 0..32 {
            match o {
                Object::Reference(id) => match self.doc.get_object(*id) {
                    Ok(x) => o = x,
                    Err(_) => return &NULL,
                },
                _ => return o,
            }
        }
        &NULL
    }
    pub fn dict(&self, o: &'a Object) -> Option<&'a Dictionary> {
        match self.deref(o) {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => Some(&s.dict),
            _ => None,
        }
    }
    pub fn get(&self, d: &'a Dictionary, k: &[u8]) -> Option<&'a Object> {
        d.get(k).ok().map(|o| self.deref(o)).filter(|o| !matches!(o, Object::Null))
    }
    pub fn get_dict(&self, d: &'a Dictionary, k: &[u8]) -> Option<&'a Dictionary> {
        self.get(d, k).and_then(|o| self.dict(o))
    }
    pub fn array(&self, d: &'a Dictionary, k: &[u8]) -> &'a [Object] {
        match self.get(d, k) {
            Some(Object::Array(a)) => a,
            _ => &[],
        }
    }
    pub fn num(&self, d: &'a Dictionary, k: &[u8]) -> Option<f64> {
        self.get(d, k).and_then(|o| o.as_float().ok()).map(|v| v as f64)
    }
    pub fn int(&self, d: &'a Dictionary, k: &[u8]) -> Option<i64> {
        self.num(d, k).map(|v| v as i64)
    }
    pub fn nums(&self, d: &'a Dictionary, k: &[u8]) -> Vec<f64> {
        self.array(d, k).iter().filter_map(|o| self.deref(o).as_float().ok()).map(|v| v as f64).collect()
    }
    pub fn name(&self, d: &'a Dictionary, k: &[u8]) -> Option<String> {
        self.get(d, k).and_then(|o| o.as_name().ok()).map(|n| String::from_utf8_lossy(n).into_owned())
    }
    /// A stream's decoded data.
    pub fn stream_data(&self, o: &'a Object) -> Option<Vec<u8>> {
        match self.deref(o) {
            Object::Stream(s) => s.decompressed_content_with_limit(STREAM_LIMIT).ok(),
            _ => None,
        }
    }
}

fn load(bytes: &[u8]) -> Result<lopdf::Document> {
    let doc = lopdf::Document::load_mem(bytes).map_err(|e| match e {
        lopdf::Error::Decryption(_) | lopdf::Error::InvalidPassword => IoError::Format("the PDF is protected by a password".into()),
        e => IoError::Format(format!("{e}")),
    })?;
    if doc.is_encrypted() && doc.encryption_state.is_none() {
        return Err(IoError::Format("the PDF is protected by a password".into()));
    }
    Ok(doc)
}

/// A page's own value, or the one it inherits from its parents.
fn inherited<'a>(pdf: &Pdf<'a>, page: &'a Dictionary, k: &[u8]) -> Option<&'a Object> {
    let mut d = page;
    for _ in 0..32 {
        if let Some(v) = pdf.get(d, k) {
            return Some(v);
        }
        d = pdf.get_dict(d, b"Parent")?;
    }
    None
}

/// A page's visible area (the crop box within the media box) and its rotation.
struct PageGeom {
    area: Rect,
    rotate: i64,
}

impl PageGeom {
    fn of(pdf: &Pdf, page: &Dictionary) -> PageGeom {
        let rect = |k: &[u8]| -> Option<Rect> {
            let v: Vec<f64> = match inherited(pdf, page, k)? {
                Object::Array(a) => a.iter().filter_map(|o| pdf.deref(o).as_float().ok()).map(|v| v as f64).collect(),
                _ => return None,
            };
            (v.len() == 4).then(|| Rect::new(v[0], v[1], v[2], v[3]).abs()).filter(|r| r.width() > 0.0 && r.height() > 0.0)
        };
        let media = rect(b"MediaBox").unwrap_or(Rect::new(0.0, 0.0, 612.0, 792.0));
        let area = rect(b"CropBox").map(|c| c.intersect(media)).filter(|c| c.width() > 0.0 && c.height() > 0.0).unwrap_or(media);
        let rotate = inherited(pdf, page, b"Rotate").and_then(|o| o.as_float().ok()).map(|v| v as i64).unwrap_or(0).rem_euclid(360) / 90 * 90;
        PageGeom { area, rotate }
    }
    /// The page size in the drawing (points).
    fn size(&self) -> (f64, f64) {
        if self.rotate % 180 == 90 {
            (self.area.height(), self.area.width())
        } else {
            (self.area.width(), self.area.height())
        }
    }
    /// PDF default space → drawing (y down, origin at the top left of the shown page).
    fn transform(&self) -> Affine {
        let a = self.area;
        let flip = Affine::new([1.0, 0.0, 0.0, -1.0, -a.x0, a.y1]);
        let (w, h) = (a.width(), a.height());
        let rot = match self.rotate {
            90 => Affine::new([0.0, 1.0, -1.0, 0.0, h, 0.0]),
            180 => Affine::new([-1.0, 0.0, 0.0, -1.0, w, h]),
            270 => Affine::new([0.0, -1.0, 1.0, 0.0, 0.0, w]),
            _ => Affine::IDENTITY,
        };
        rot * flip
    }
}

/// The size of each page (points, as shown), for the page list of the import dialog.
pub fn page_sizes(bytes: &[u8]) -> Result<Vec<(f64, f64)>> {
    let doc = load(bytes)?;
    let pdf = Pdf { doc: &doc };
    Ok(doc.get_pages().values().filter_map(|id| doc.get_dictionary(*id).ok()).map(|p| PageGeom::of(&pdf, p).size()).collect())
}

/// The concatenated content streams of a page.
fn page_content(pdf: &Pdf, page: &Dictionary) -> Vec<u8> {
    let mut out = vec![];
    match page.get(b"Contents").ok().map(|c| pdf.deref(c)) {
        Some(s @ Object::Stream(_)) => {
            if let Some(d) = pdf.stream_data(s) {
                out = d;
            }
        }
        Some(Object::Array(a)) => {
            for c in a {
                if let Some(d) = pdf.stream_data(c) {
                    out.extend_from_slice(&d);
                    out.push(b'\n');
                }
            }
        }
        _ => {}
    }
    out
}

/// Read a PDF into a new drawing, one page per selected PDF page, and say what could not be
/// brought in exactly.
pub fn load_with_notes(bytes: &[u8], opts: &PdfImportOptions) -> Result<(Document, Vec<String>)> {
    let pdoc = load(bytes)?;
    let pdf = Pdf { doc: &pdoc };
    let pages: Vec<lopdf::ObjectId> = pdoc.get_pages().values().copied().collect();
    if pages.is_empty() {
        return Err(IoError::Format("the PDF has no pages".into()));
    }
    let selected: Vec<usize> = if opts.pages.is_empty() { (0..pages.len()).collect() } else { opts.pages.iter().copied().filter(|p| *p < pages.len()).collect() };
    if selected.is_empty() {
        return Err(IoError::Format("none of the chosen pages is in the PDF".into()));
    }
    let mut doc: Option<Document> = None;
    let mut notes = Notes::default();
    let mut spots = BTreeMap::new();
    for &pi in &selected {
        let Ok(page) = pdoc.get_dictionary(pages[pi]) else { continue };
        let geom = PageGeom::of(&pdf, page);
        let (w, h) = geom.size();
        let d = doc.get_or_insert_with(|| Document::new(w, h));
        let index = if d.pages.len() == 1 && d.pages[0].layers.iter().all(|l| l.objects.is_empty()) && pi == selected[0] {
            d.pages[0].width = w;
            d.pages[0].height = h;
            0
        } else {
            let n = d.pages.len();
            d.add_page(n, w, h)
        };
        let res = inherited(&pdf, page, b"Resources").and_then(|r| pdf.dict(r));
        let content = page_content(&pdf, page);
        let mut ctx = interp::Ctx::new(&pdf, d, opts, &mut notes);
        let objs = ctx.page(&content, res, geom.transform(), Rect::new(0.0, 0.0, w, h));
        spots.append(&mut ctx.spots);
        if pdf.array(page, b"Annots").iter().filter_map(|a| pdf.dict(a)).any(|a| !matches!(pdf.name(a, b"Subtype").as_deref(), Some("Link" | "Popup"))) {
            *notes.0.entry(Note::Annotations).or_insert(0) += 1;
        }
        let d = doc.as_mut().unwrap();
        d.pages[index].layers[0].objects = objs;
    }
    let mut doc = doc.ok_or_else(|| IoError::Format("the PDF has no pages".into()))?;
    // the spot colors the drawing uses, as the document's colors
    for (name, c) in spots {
        if !doc.palette.iter().any(|p| p.name == name) {
            doc.palette.push(NamedColor { name, color: c, spot: true });
        }
    }
    if let Some(info) = pdoc.trailer.get(b"Info").ok().and_then(|i| pdf.dict(i)) {
        let text = |k: &[u8]| info.get(k).ok().map(|o| pdf.deref(o)).and_then(|o| o.as_str().ok()).map(decode_text).unwrap_or_default();
        doc.info.title = text(b"Title");
        doc.info.subject = text(b"Subject");
        doc.info.author = text(b"Author");
        doc.info.keywords = text(b"Keywords");
    }
    doc.ensure_next_id();
    Ok((doc, notes.lines()))
}

/// A PDF text string (UTF-16 with a byte order mark, else PDFDocEncoding).
fn decode_text(b: &[u8]) -> String {
    if b.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = b[2..].chunks(2).map(|c| ((c[0] as u16) << 8) | *c.get(1).unwrap_or(&0) as u16).collect();
        return String::from_utf16_lossy(&units);
    }
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&b[3..]).into_owned();
    }
    b.iter()
        .map(|c| {
            let n = encodings::PDF_DOC[*c as usize];
            if n.is_empty() {
                *c as char
            } else {
                font::glyph_unicode(n).and_then(|s| s.chars().next()).unwrap_or(*c as char)
            }
        })
        .collect()
}

/// Import the selected pages of a PDF into an existing document: one list of top-level
/// objects per page (a group when the page has more than one), not yet placed, with the
/// page sizes.
pub fn import_pages(doc: &mut Document, bytes: &[u8], opts: &PdfImportOptions) -> Result<(Vec<(Vec<VObj>, (f64, f64))>, Vec<String>)> {
    let (other, notes) = load_with_notes(bytes, opts)?;
    let mut image_map = std::collections::HashMap::new();
    for (id, data) in other.images.iter() {
        image_map.insert(*id, doc.add_image(data.clone()));
    }
    for c in &other.palette {
        if c.spot && !doc.palette.iter().any(|p| p.name == c.name) {
            doc.palette.push(c.clone());
        }
    }
    let mut out = vec![];
    for p in other.pages {
        let size = (p.width, p.height);
        let mut objs = vec![];
        for l in p.layers {
            for mut o in l.objects {
                doc.reassign_ids(&mut o);
                o.walk_mut(&mut |x| {
                    if let ObjKind::Image(i) = &mut x.kind {
                        if let Some(n) = image_map.get(&i.image) {
                            i.image = *n;
                        }
                    }
                });
                objs.push(o);
            }
        }
        if objs.len() > 1 {
            let mut st = Style::default();
            st.outline = Outline::none();
            objs = vec![VObj::new(doc.new_id(), ObjKind::Group(objs), Affine::IDENTITY, st)];
        }
        out.push((objs, size));
    }
    Ok((out, notes))
}

#[cfg(test)]
mod tests;
