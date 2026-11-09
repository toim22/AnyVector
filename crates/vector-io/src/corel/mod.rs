//! Import of drawings saved by CorelDRAW (`.cdr`, `.cdt`, `.cdrt`).
//!
//! The reader is written from the observed layout of files saved by CorelDRAW itself; every
//! record it does not understand is skipped, so an unknown feature costs that feature only, never
//! the drawing.

mod bitmap;
mod build;
mod color;
mod riff;
mod text;

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
    if bytes.len() >= 2 && &bytes[0..2] == b"WL" {
        return Err(IoError::Unsupported("this drawing was saved by CorelDRAW 1 or 2, a format older than AnyVector can read. Open it in CorelDRAW 3 or later and save it again.".into()));
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

fn tree(c: &Container) -> Result<Chunk> {
    let mut out = vec![];
    let ext = c.streams.as_ref().map(|s| External { streams: s });
    riff::parse(&c.main, 0, c.main.len(), None, ext.as_ref(), 0, &mut out);
    out.into_iter().find(|c| c.list.is_some()).ok_or_else(|| IoError::Format("the CorelDRAW drawing is empty or damaged".into()))
}

/// Open a drawing saved by CorelDRAW.
pub fn load_from_bytes(bytes: &[u8]) -> Result<Document> {
    let c = container(bytes)?;
    let root = tree(&c)?;
    build::document(&root)
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
    let c = container(bytes)?;
    let root = tree(&c)?;
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
            let doc = match super::load_from_bytes(&bytes) {
                Ok(d) => d,
                Err(e) => {
                    println!("{path}: ERROR {e}");
                    continue;
                }
            };
            println!("{path}: {} page(s)", doc.pages.len());
            for (i, p) in doc.pages.iter().enumerate() {
                println!(" page {} {:.1}x{:.1} pt", i + 1, p.width, p.height);
                for l in &p.layers {
                    println!("  layer {:?} master={} visible={} objects={}", l.name, l.master, l.visible, l.objects.len());
                    if std::env::var("ANYVECTOR_CDR_LIST").is_ok() {
                        for o in &l.objects {
                            let b = o.bounds();
                            println!("   {:<10} {:<24} [{:.1} {:.1} {:.1} {:.1}] mm", o.kind.type_name(), o.name, b.x0 * 25.4 / 72.0, (p.height - b.y1) * 25.4 / 72.0, b.width() * 25.4 / 72.0, b.height() * 25.4 / 72.0);
                        }
                    }
                }
                let opts = crate::image_io::RasterOptions { dpi: 96.0, ..Default::default() };
                let _ = crate::image_io::export_raster(&doc, i, std::path::Path::new(&format!("{path}.p{}.png", i + 1)), &opts);
            }
        }
    }
}
