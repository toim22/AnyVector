//! Import of drawings saved by CorelDRAW (`.cdr`, `.cdt`, `.cdrt`).
//!
//! The reader is written from the observed layout of files saved by CorelDRAW itself; every
//! record it does not understand is skipped, so an unknown feature costs that feature only, never
//! the drawing.

mod bitmap;
mod build;
mod color;
mod palettes;
mod psfill;
mod persp;
mod riff;
mod text;
mod texture;
mod tiff;
mod waldo;

use crate::{IoError, Result};
use riff::{Chunk, External};
use std::io::Read;
use vector_core::document::Document;

/// The main stream of the drawing and the data streams its chunks may point into.
struct Container {
    main: Vec<u8>,
    streams: Option<Vec<Vec<u8>>>,
}

fn read_entry<R: Read + std::io::Seek>(z: &mut zip::ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let mut f = z.by_name(name).ok()?;
    let mut v = Vec::new();
    f.read_to_end(&mut v).ok()?;
    Some(v)
}

fn container(bytes: &[u8]) -> Result<Container> {
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" {
        return Ok(Container { main: bytes.to_vec(), streams: None });
    }
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| IoError::Format("not a CorelDRAW drawing".into()))?;
    if let Some(main) = read_entry(&mut z, "content/riffData.cdr") {
        return Ok(Container { main, streams: None });
    }
    let main = read_entry(&mut z, "content/root.dat").ok_or_else(|| IoError::Format("not a CorelDRAW drawing".into()))?;
    let list = read_entry(&mut z, "content/dataFileList.dat").unwrap_or_default();
    let mut streams = vec![];
    for name in String::from_utf8_lossy(&list).split('\n').map(str::trim).filter(|n| !n.is_empty()) {
        streams.push(read_entry(&mut z, &format!("content/data/{name}")).unwrap_or_default());
    }
    Ok(Container { main, streams: Some(streams) })
}

/// The chunk tree of any drawing: CorelDRAW 1 and 2 files are rewritten as one.
fn root(bytes: &[u8]) -> Result<Chunk> {
    if waldo::is_waldo(bytes) {
        return waldo::tree(bytes).ok_or_else(|| IoError::Format("the CorelDRAW 1 or 2 drawing is damaged".into()));
    }
    tree(&container(bytes)?)
}

fn tree(c: &Container) -> Result<Chunk> {
    let mut out = vec![];
    let ext = c.streams.as_ref().map(|s| External { streams: s });
    riff::parse(&c.main, 0, c.main.len(), None, ext.as_ref(), 0, &mut out);
    out.into_iter().find(|c| c.list.is_some()).ok_or_else(|| IoError::Format("the CorelDRAW drawing is empty or damaged".into()))
}

/// Open a drawing saved by CorelDRAW.
pub fn load_from_bytes(bytes: &[u8]) -> Result<Document> {
    let root = root(bytes)?;
    color::with_profile(cmyk_profile(bytes).as_deref(), || build::document(&root))
}

/// Open a drawing and also list the object types that were left out: (type, count).
pub fn load_with_report(bytes: &[u8]) -> Result<(Document, Vec<(u32, usize)>)> {
    let root = root(bytes)?;
    color::with_profile(cmyk_profile(bytes).as_deref(), || build::document_with_report(&root))
}

/// The CMYK color profile the drawing was made with, from the metadata of X4 and later files:
/// `CmykProfileName="..."` (X6 and later) or `<Cmyk><Name>...</Name>` (X4 and X5).
fn cmyk_profile(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 4 || &bytes[0..2] != b"PK" {
        return None;
    }
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    let xml = read_entry(&mut z, "META-INF/metadata.xml").or_else(|| read_entry(&mut z, "metadata/metadata.xml"))?;
    let xml = String::from_utf8_lossy(&xml);
    let after = |s: &str, start: &str, end: &str| -> Option<String> {
        let i = s.find(start)? + start.len();
        let n = s[i..].find(end)?;
        Some(s[i..i + n].to_string())
    };
    let name = after(&xml, "CmykProfileName=\"", "\"").or_else(|| {
        let i = xml.find("<Cmyk>")?;
        after(&xml[i..], "<Name>", "</Name>")
    })?;
    let name = name.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&");
    (!name.trim().is_empty()).then_some(name)
}

/// The version of CorelDRAW that saved the drawing, times 100 (X5 = 1500).
pub fn version(bytes: &[u8]) -> Result<u32> {
    Ok(build::version(&root(bytes)?))
}

/// A readable outline of the file's chunk tree (for tests and diagnosis).
pub fn describe(bytes: &[u8]) -> Result<String> {
    fn walk(c: &Chunk, depth: usize, out: &mut String) {
        let name = |t: &[u8; 4]| String::from_utf8_lossy(t).to_string();
        match &c.list {
            Some(l) => {
                out.push_str(&format!("{}[{}]\n", "  ".repeat(depth), name(l)));
                for k in &c.children {
                    walk(k, depth + 1, out);
                }
            }
            None => out.push_str(&format!("{}{} {}\n", "  ".repeat(depth), name(&c.tag), c.data.len())),
        }
    }
    let root = root(bytes)?;
    let mut s = String::new();
    walk(&root, 0, &mut s);
    Ok(s)
}

#[cfg(test)]
mod tests {
    /// Diagnosis aid: `ANYVECTOR_CDR=<file> cargo test -p vector-io corel_probe -- --ignored --nocapture`
    /// prints what was read and writes `<file>.pN.png` renderings.
    #[test]
    #[ignore]
    fn corel_probe() {
        let Ok(list) = std::env::var("ANYVECTOR_CDR") else { return };
        for path in list.split(';') {
            let bytes = std::fs::read(path).unwrap();
            let mut doc = match super::load_from_bytes(&bytes) {
                Ok(d) => d,
                Err(e) => {
                    println!("{path}: ERROR {e}");
                    continue;
                }
            };
            // with ANYVECTOR_CDR_ALL the pages grow to show the whole drawing
            if std::env::var("ANYVECTOR_CDR_ALL").is_ok() {
                for p in &mut doc.pages {
                    let b = p.layers.iter().flat_map(|l| &l.objects).map(|o| o.bounds()).filter(|b| b.width() < 10.0 * p.width && b.height() < 10.0 * p.height).reduce(|a, b| a.union(b));
                    let Some(b) = b else { continue };
                    let b = b.union(kurbo::Rect::new(0.0, 0.0, p.width, p.height));
                    for l in &mut p.layers {
                        l.objects.iter_mut().for_each(|o| o.apply_transform(kurbo::Affine::translate((-b.x0, -b.y0))));
                    }
                    (p.width, p.height) = (b.width(), b.height());
                }
            }
            println!("{path}: {} page(s), {} arrowheads", doc.pages.len(), doc.arrowheads.len());
            for (i, p) in doc.pages.iter().enumerate() {
                println!(" page {} {:.1}x{:.1} pt", i + 1, p.width, p.height);
                for l in &p.layers {
                    println!("  layer {:?} master={} visible={} objects={}", l.name, l.master, l.visible, l.objects.len());
                    if std::env::var("ANYVECTOR_CDR_LIST").is_ok() {
                        // with ANYVECTOR_CDR_DEEP the objects inside groups too
                        let mut all: Vec<&vector_core::object::Object> = vec![];
                        let mut todo: Vec<&vector_core::object::Object> = l.objects.iter().rev().collect();
                        while let Some(o) = todo.pop() {
                            all.push(o);
                            if let (vector_core::object::ObjKind::Group(kids), true) = (&o.kind, std::env::var("ANYVECTOR_CDR_DEEP").is_ok()) {
                                todo.extend(kids.iter().rev());
                            }
                        }
                        for o in all {
                            let b = o.bounds();
                            println!("   {:<10} {:<24} [{:.1} {:.1} {:.1} {:.1}] mm", o.kind.type_name(), o.name, b.x0 * 25.4 / 72.0, (p.height - b.y1) * 25.4 / 72.0, b.width() * 25.4 / 72.0, b.height() * 25.4 / 72.0);
                            if std::env::var("ANYVECTOR_CDR_STYLE").is_ok() {
                                println!("      fill={:?} outline={:?} w={:.2} arrows={:?}/{:?}", o.style.fill, o.style.outline.color, o.style.outline.width, o.style.outline.start_arrow, o.style.outline.end_arrow);
                            }
                            if let vector_core::object::ObjKind::Text(t) = &o.kind {
                                println!("      {:?} font={:?} size={:.1} xf={:?}", t.content.chars().take(30).collect::<String>(), t.format.font, t.format.size, o.transform.as_coeffs());
                            }
                        }
                    }
                }
                let opts = crate::image_io::RasterOptions { dpi: std::env::var("ANYVECTOR_CDR_DPI").ok().and_then(|v| v.parse().ok()).unwrap_or(96.0), ..Default::default() };
                let _ = crate::image_io::export_raster(&doc, i, std::path::Path::new(&format!("{path}.p{}.png", i + 1)), &opts);
            }
        }
    }

    /// `ANYVECTOR_CDR_DIR=<folder> cargo test -p vector-io corel_batch -- --ignored --nocapture`:
    /// one line per drawing in the folder; with `ANYVECTOR_CDR_PNG` also renders page 1.
    #[test]
    #[ignore]
    fn corel_batch() {
        let Ok(dir) = std::env::var("ANYVECTOR_CDR_DIR") else { return };
        let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| matches!(crate::extension(p).as_str(), "cdr" | "cdt" | "cdrt")).collect();
        files.sort();
        for path in files {
            let bytes = std::fs::read(&path).unwrap();
            let t = std::time::Instant::now();
            let ver = super::version(&bytes).unwrap_or(0);
            let r = std::panic::catch_unwind(|| super::load_with_report(&bytes));
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            match r {
                Err(_) => println!("{name} v{ver} PANIC"),
                Ok(Err(e)) => println!("{name} v{ver} ERROR {e}"),
                Ok(Ok((doc, skipped))) => {
                    let mut kinds: std::collections::BTreeMap<&str, usize> = Default::default();
                    for p in &doc.pages {
                        for o in p.all_objects() {
                            o.walk(&mut |x| *kinds.entry(x.kind.type_name()).or_default() += 1);
                        }
                    }
                    let total: usize = kinds.values().sum();
                    println!("{name} v{ver} pages={} objects={total} {:?} images={} {:.0}ms", doc.pages.len(), kinds, doc.images.len(), t.elapsed().as_secs_f64() * 1000.0);
                    if !skipped.is_empty() {
                        println!("   skipped {skipped:x?}");
                    }
                    if std::env::var("ANYVECTOR_CDR_PNG").is_ok() {
                        let opts = crate::image_io::RasterOptions { dpi: 72.0, ..Default::default() };
                        let _ = crate::image_io::export_raster(&doc, 0, &path.with_extension("ours.png"), &opts);
                    }
                }
            }
        }
    }
}
