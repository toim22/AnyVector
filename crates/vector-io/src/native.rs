//! Native `.vct` format: a zip archive containing
//! - `mimetype` (stored, "application/x-vector-drawing")
//! - `document.json` (the document model)
//! - `images/<id>.<png|jpg>` (image pixel data)
//! - `thumbnail.png` (preview of the first page, used by the Welcome screen and Explorer)

use crate::{image_io, IoError, Result};
use std::io::{Read, Seek, Write};
use std::path::Path;
use std::sync::Arc;
use vector_core::document::Document;
use zip::write::SimpleFileOptions;

pub const MIMETYPE: &str = "application/x-vector-drawing";

pub fn save(doc: &Document, path: &Path) -> Result<()> {
    let bytes = save_to_bytes(doc)?;
    // write atomically: temp file then rename
    let tmp = path.with_extension("vct.tmp");
    std::fs::write(&tmp, &bytes)?;
    if std::fs::rename(&tmp, path).is_err() {
        std::fs::copy(&tmp, path)?;
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(())
}

pub fn save_to_bytes(doc: &Document) -> Result<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    write_zip(doc, &mut buf)?;
    Ok(buf.into_inner())
}

fn zerr(e: zip::result::ZipError) -> IoError {
    IoError::Format(e.to_string())
}

fn write_zip<W: Write + Seek>(doc: &Document, w: &mut W) -> Result<()> {
    let mut z = zip::ZipWriter::new(w);
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    z.start_file("mimetype", stored).map_err(zerr)?;
    z.write_all(MIMETYPE.as_bytes())?;
    // only store images that are referenced (plus pattern images)
    let used = doc.referenced_images();
    let mut doc2 = doc.clone();
    doc2.images.retain(|id, _| used.contains(id));
    let json = serde_json::to_vec(&doc2).map_err(|e| IoError::Format(e.to_string()))?;
    z.start_file("document.json", deflated).map_err(zerr)?;
    z.write_all(&json)?;
    for (id, img) in doc2.images.iter() {
        let (bytes, ext) = image_io::encoded_bytes(img);
        z.start_file(format!("images/{id}.{ext}"), stored).map_err(zerr)?;
        z.write_all(&bytes)?;
    }
    if let Some(thumb) = thumbnail(doc, 256) {
        z.start_file("thumbnail.png", stored).map_err(zerr)?;
        z.write_all(&thumb)?;
    }
    z.finish().map_err(zerr)?;
    Ok(())
}

/// PNG thumbnail of the first page, `max` pixels on the long side.
pub fn thumbnail(doc: &Document, max: u32) -> Option<Vec<u8>> {
    let p = doc.pages.first()?;
    let dpi = max as f64 / p.width.max(p.height).max(1.0) * 72.0;
    let pm = vector_render::render_page_image(doc, 0, dpi, Some(p.background.unwrap_or(vector_core::Color::WHITE)), None, true)?;
    let rgba = vector_render::pixmap_to_rgba(&pm);
    Some(image_io::encode_png(pm.width(), pm.height(), &rgba))
}

pub fn load(path: &Path) -> Result<Document> {
    let bytes = std::fs::read(path)?;
    load_from_bytes(&bytes)
}

pub fn load_from_bytes(bytes: &[u8]) -> Result<Document> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(zerr)?;
    let mut json = vec![];
    z.by_name("document.json").map_err(zerr)?.read_to_end(&mut json)?;
    let mut doc: Document = serde_json::from_slice(&json).map_err(|e| IoError::Format(e.to_string()))?;
    let ids: Vec<u64> = doc.images.keys().copied().collect();
    for id in ids {
        let mut data = None;
        for ext in ["png", "jpg"] {
            if let Ok(mut f) = z.by_name(&format!("images/{id}.{ext}")) {
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

/// Read only the thumbnail of a `.vct` file (for the Welcome screen / recent files).
pub fn read_thumbnail(path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let f = std::fs::File::open(path).ok()?;
    let mut z = zip::ZipArchive::new(f).ok()?;
    let mut b = vec![];
    z.by_name("thumbnail.png").ok()?.read_to_end(&mut b).ok()?;
    let img = image::load_from_memory(&b).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((w, h, img.into_raw()))
}
