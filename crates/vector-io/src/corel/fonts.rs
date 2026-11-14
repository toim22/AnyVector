//! Fonts embedded in drawings saved by CorelDRAW (X7 and later, "Embed fonts using
//! CorelDRAW": `CCdrReader::ReadEmbeddedFontInternal` / `TUSRSaveFontData` in CdrCore.dll).
//!
//! Each font is a part of the zip container named `embed/embedding0`, `embed/embedding1`, ...
//! (`CdrSchema::embeddedFont` = "embed/embedding%d"; the reader takes every part of `embed/`
//! with no extension). A part is a small tagged archive followed by the font:
//!
//! ```text
//! blocks, each: tag i32, size u32, `size` bytes; tags in descending order
//!   tag 1: font technology u8, font version UTF-16 [1025]   ("Version 7.00")
//!   tag 0: face name UTF-16 [32], face name ANSI [32], style u32, italic u8, PANOSE [10],
//!          embedding type u32, u8, Type 1 files u8, has font data u8, UTF-16 [260]
//! then, when the "has font data" byte is set, what Windows' TTEmbedFont writes: an Embedded
//! OpenType structure (EOTSize u32, FontDataSize u32, Version u32, Flags u32, PANOSE, names...)
//! that ends with the font file, XORed with 0x50 when Flags has TTEMBED_XORENCRYPTDATA.
//! ```
//!
//! A part without font data stands for a font CorelDRAW did not embed (installed fonts it was
//! told to leave out); Type 1 fonts (two raw files) and MicroType Express compressed fonts are
//! not read.

use std::io::Read;
use std::sync::Arc;

/// TTEMBED_TTCOMPRESSED: the font data is MicroType Express compressed.
const EOT_COMPRESSED: u32 = 0x4;
/// TTEMBED_XORENCRYPTDATA: every byte of the font data is XORed with 0x50.
const EOT_XOR: u32 = 0x1000_0000;

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// A UTF-16 string of a fixed-size field, up to its first NUL.
fn utf16(b: &[u8]) -> String {
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&u| u != 0).collect();
    String::from_utf16_lossy(&units)
}

/// Whether bytes start a font file (TrueType, OpenType or a collection).
fn is_sfnt(b: &[u8]) -> bool {
    matches!(b.get(0..4), Some([0, 1, 0, 0] | b"OTTO" | b"true" | b"ttcf"))
}

/// The font of one `embed/embeddingN` part: (file name to embed it under again, font file).
pub(super) fn decode(part: &[u8]) -> Option<(String, Vec<u8>)> {
    // the tagged blocks: the font's description is the last one (tag 0)
    let mut at = 0;
    let desc = loop {
        let tag = u32_at(part, at)? as i32;
        let size = u32_at(part, at + 4)? as usize;
        let body = part.get(at + 8..at + 8 + size)?;
        at += 8 + size;
        if tag == 0 {
            break body;
        }
        if tag < 0 {
            return None;
        }
    };
    let face = utf16(desc.get(0..64)?);
    let (type1, has_data) = (*desc.get(116)?, *desc.get(117)?);
    if has_data == 0 || type1 != 0 {
        return None;
    }
    let eot = &part[at..];
    let (eot_size, data_size, flags) = (u32_at(eot, 0)? as usize, u32_at(eot, 4)? as usize, u32_at(eot, 12)?);
    if flags & EOT_COMPRESSED != 0 || data_size > eot_size {
        return None;
    }
    let mut font = eot.get(eot_size - data_size..eot_size.min(eot.len()))?.to_vec();
    // CorelDRAW also tells by the first bytes whether the data is XORed
    if flags & EOT_XOR != 0 || (!is_sfnt(&font) && is_sfnt(&font.iter().take(4).map(|b| b ^ 0x50).collect::<Vec<u8>>())) {
        font.iter_mut().for_each(|b| *b ^= 0x50);
    }
    if !is_sfnt(&font) {
        return None;
    }
    let ext = if font.starts_with(b"OTTO") { "otf" } else { "ttf" };
    // the face's full name ("Times New Roman Bold"), else the face name of the drawing
    let full = ttf_parser::Face::parse(&font, 0)
        .ok()
        .and_then(|f| f.names().into_iter().filter(|n| n.name_id == ttf_parser::name_id::FULL_NAME && n.is_unicode()).min_by_key(|n| n.language_id != 0x0409).and_then(|n| n.to_string()))
        .unwrap_or(face);
    let stem: String = full.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    let stem = if stem.trim().is_empty() { "font".to_string() } else { stem.trim().to_string() };
    Some((format!("{stem}.{ext}"), font))
}

/// The fonts embedded in a CorelDRAW drawing (zip container), in the order of their parts.
pub(super) fn embedded(bytes: &[u8]) -> Vec<(String, Arc<Vec<u8>>)> {
    if bytes.len() < 4 || &bytes[0..2] != b"PK" {
        return vec![];
    }
    let Ok(mut z) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else { return vec![] };
    let mut names: Vec<(u32, String)> = z
        .file_names()
        .filter_map(|n| {
            let n = n.ok()?;
            let num = n.strip_prefix("embed/embedding")?;
            Some((num.parse().ok()?, n.to_string()))
        })
        .collect();
    names.sort();
    let mut out: Vec<(String, Arc<Vec<u8>>)> = vec![];
    for (_, name) in names {
        let mut part = vec![];
        let Ok(mut f) = z.by_name(&name) else { continue };
        if f.read_to_end(&mut part).is_err() {
            continue;
        }
        if let Some((file, data)) = decode(&part) {
            if !out.iter().any(|(n, d)| n == &file || **d == data) {
                out.push((file, Arc::new(data)));
            }
        }
    }
    out
}

/// Use the fonts embedded in a CorelDRAW drawing for this session (see
/// `vector_core::text::register_embedded_fonts`).
pub(super) fn register(bytes: &[u8]) {
    vector_core::text::register_embedded_fonts(embedded(bytes));
}

/// An `embed/embeddingN` part as CorelDRAW writes it, holding `font` (tests only).
#[cfg(test)]
fn encode(face: &str, font: &[u8]) -> Vec<u8> {
    let mut out = vec![];
    let utf16_field = |s: &str, units: usize| -> Vec<u8> {
        let mut v: Vec<u8> = s.encode_utf16().take(units - 1).flat_map(|u| u.to_le_bytes()).collect();
        v.resize(units * 2, 0);
        v
    };
    // tag 1
    let mut t1 = vec![4u8];
    t1.extend(utf16_field("Version 1.0", 1025));
    out.extend(1i32.to_le_bytes());
    out.extend((t1.len() as u32).to_le_bytes());
    out.extend(t1);
    // tag 0
    let mut t0 = utf16_field(face, 32);
    let mut ansi: Vec<u8> = face.bytes().take(31).collect();
    ansi.resize(32, 0);
    t0.extend(ansi);
    t0.extend(0x40u32.to_le_bytes());
    t0.push(0);
    t0.extend([2, 11, 6, 4, 2, 2, 2, 2, 2, 4]);
    t0.extend(2u32.to_le_bytes());
    t0.extend([0, 0, 1]);
    t0.extend(utf16_field("", 260));
    out.extend(0i32.to_le_bytes());
    out.extend((t0.len() as u32).to_le_bytes());
    out.extend(t0);
    // the Embedded OpenType structure: a header with the names, then the XORed font
    let mut head = vec![];
    head.extend(0x0002_0002u32.to_le_bytes());
    head.extend(EOT_XOR.to_le_bytes());
    head.extend([0u8; 140]);
    let size = 8 + head.len() + font.len();
    out.extend((size as u32).to_le_bytes());
    out.extend((font.len() as u32).to_le_bytes());
    out.extend(head);
    out.extend(font.iter().map(|b| b ^ 0x50));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use vector_core::text::{font_files, fonts, register_embedded_fonts};

    #[test]
    fn decodes_what_coreldraw_writes() {
        let font = [&[0u8, 1, 0, 0][..], &[7; 60]].concat();
        let part = encode("Some Face", &font);
        let (name, data) = decode(&part).unwrap();
        assert_eq!(data, font);
        // not a font ttf-parser can read, so it's named after the drawing's face name
        assert_eq!(name, "Some Face.ttf");
        // a part without the font (CorelDRAW leaves installed fonts out when told to)
        let mut empty = part.clone();
        let flag = 8 + 2051 + 8 + 117;
        empty[flag] = 0;
        assert!(decode(&empty).is_none());
        // a cut-off part is skipped, not a panic
        for n in [0, 10, 2059, 2059 + 300, part.len() - 5] {
            let _ = decode(&part[..n]);
        }
    }

    /// A small installed TrueType font that may be embedded (None when there is none).
    fn test_font() -> Option<Vec<u8>> {
        let paths: Vec<std::path::PathBuf> = {
            let db = fonts().lock();
            db.db
                .faces()
                .filter_map(|f| match &f.source {
                    fontdb::Source::File(p) if f.index == 0 && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ttf")) => Some(p.clone()),
                    _ => None,
                })
                .collect()
        };
        let mut found: Vec<(usize, std::path::PathBuf, Vec<u8>)> = paths
            .into_iter()
            .filter_map(|p| {
                let b = std::fs::read(&p).ok()?;
                let f = ttf_parser::Face::parse(&b, 0).ok()?;
                let ok = f.glyph_index('A').is_some() && matches!(f.permissions(), None | Some(ttf_parser::Permissions::Installable) | Some(ttf_parser::Permissions::Editable));
                (ok && b.len() < 2_000_000).then(|| (b.len(), p, b))
            })
            .collect();
        found.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        found.into_iter().next().map(|(.., b)| b)
    }

    /// The font with its names replaced by `family`, and its OS/2 embedding permissions by
    /// `fs_type` when given.
    fn renamed(font: &[u8], family: &str, fs_type: Option<u16>) -> Vec<u8> {
        let be16 = |at: usize| u16::from_be_bytes([font[at], font[at + 1]]);
        let be32 = |at: usize| u32::from_be_bytes(font[at..at + 4].try_into().unwrap());
        let n = be16(4) as usize;
        let mut tables: Vec<([u8; 4], Vec<u8>)> = (0..n)
            .map(|i| {
                let r = 12 + 16 * i;
                let (off, len) = (be32(r + 8) as usize, be32(r + 12) as usize);
                (font[r..r + 4].try_into().unwrap(), font[off..off + len].to_vec())
            })
            .collect();
        let be = |s: &str| -> Vec<u8> { s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect() };
        let (s, ps, reg) = (be(family), be(&family.replace(' ', "")), be("Regular"));
        let strings = [(1u16, &s), (2, &reg), (4, &s), (6, &ps)];
        let mut name = vec![];
        name.extend(0u16.to_be_bytes());
        name.extend((strings.len() as u16).to_be_bytes());
        name.extend((6 + 12 * strings.len() as u16).to_be_bytes());
        let mut pool = vec![];
        for (id, v) in strings {
            for x in [3u16, 1, 0x409, id, v.len() as u16, pool.len() as u16] {
                name.extend(x.to_be_bytes());
            }
            pool.extend_from_slice(v);
        }
        name.extend(pool);
        for (tag, data) in &mut tables {
            match &*tag {
                b"name" => *data = name.clone(),
                b"OS/2" => {
                    if let Some(t) = fs_type {
                        data[8..10].copy_from_slice(&t.to_be_bytes());
                    }
                }
                _ => {}
            }
        }
        let mut out = font[0..12].to_vec();
        let mut body = vec![];
        for (tag, data) in &tables {
            out.extend_from_slice(tag);
            out.extend(0u32.to_be_bytes());
            out.extend(((12 + 16 * n + body.len()) as u32).to_be_bytes());
            out.extend((data.len() as u32).to_be_bytes());
            body.extend_from_slice(data);
            while body.len() % 4 != 0 {
                body.push(0);
            }
        }
        out.extend(body);
        out
    }

    /// The zip with `from` renamed to `to` (same length) in every part, and `extra` parts added.
    fn patched(zip_bytes: &[u8], from: &str, to: &str, extra: &[(&str, Vec<u8>)]) -> Vec<u8> {
        assert_eq!(from.len(), to.len());
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).unwrap();
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let replace = |data: &mut Vec<u8>, a: &[u8], b: &[u8]| {
            let mut i = 0;
            while i + a.len() <= data.len() {
                if &data[i..i + a.len()] == a {
                    data[i..i + a.len()].copy_from_slice(b);
                    i += a.len();
                } else {
                    i += 1;
                }
            }
        };
        let u16le = |s: &str| -> Vec<u8> { s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect() };
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().unwrap().to_string();
            let mut data = vec![];
            f.read_to_end(&mut data).unwrap();
            replace(&mut data, from.as_bytes(), to.as_bytes());
            replace(&mut data, &u16le(from), &u16le(to));
            w.start_file(name.as_str(), zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&data).unwrap();
        }
        for (name, data) in extra {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn substituted(family: &str) -> bool {
        fonts().lock().substitute(family).is_some()
    }

    #[test]
    fn a_coreldraw_drawing_shows_its_text_in_the_font_it_embeds() {
        let Some(font) = test_font() else {
            eprintln!("no installed TrueType font to embed; skipped");
            return;
        };
        // the drawing's Times New Roman text is set in a font that isn't installed, and the
        // drawing embeds that font
        let family = "AnyVecEmbedTest";
        let font = renamed(&font, family, None);
        assert!(substituted(family));
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/corel/mixed-2026.cdr");
        let bytes = patched(&std::fs::read(fixture).unwrap(), "Times New Roman", family, &[("embed/embedding0", encode(family, &font))]);
        assert_eq!(embedded(&bytes).len(), 1);
        let (doc, report) = super::super::load_with_report(&bytes).unwrap();
        assert!(doc.used_fonts().iter().any(|f| f.0 == family));
        // not noted as missing, and shown in the embedded font, not in a substitute
        let noted = report.notes.iter().any(|(n, _)| matches!(n, super::super::ImportNote::FontSubstituted { missing, .. } if missing == family));
        assert!(!noted, "{:?}", report.lines());
        assert!(!substituted(family));
        let files = font_files(&[(family.to_string(), false, false)]);
        assert_eq!(files.len(), 1);
        assert_eq!(*files[0].data, font);
        assert!(files[0].embeddable);
        // the font isn't installed on the computer: it is in memory only
        assert!(fonts().lock().db.faces().filter(|f| f.families.iter().any(|(n, _)| n == family)).all(|f| matches!(f.source, fontdb::Source::Binary(_))));
        // saving with "Embed fonts" keeps it (as the app does: the used fonts' files)
        let opts = crate::cdr::SaveOptions {
            fonts: font_files(&doc.used_fonts()).into_iter().filter(|f| f.embeddable).map(|f| crate::cdr::EmbeddedFile { name: f.name, data: f.data }).collect(),
            ..Default::default()
        };
        let saved = crate::cdr::save_to_bytes(&doc, &opts).unwrap();
        assert!(crate::cdr::embedded_fonts(std::io::Cursor::new(&saved)).contains(&format!("{family}.ttf")));
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(&saved)).unwrap();
        let mut again = vec![];
        z.by_name(&format!("resources/fonts/{family}.ttf")).unwrap().read_to_end(&mut again).unwrap();
        assert_eq!(again, font);
    }

    #[test]
    fn an_anyvector_drawing_shows_its_text_in_the_font_it_embeds() {
        let Some(font) = test_font() else {
            eprintln!("no installed TrueType font to embed; skipped");
            return;
        };
        let family = "AnyVecEmbedOwn";
        let font = renamed(&font, family, None);
        let mut doc = vector_core::Document::new(200.0, 100.0);
        let t = vector_core::text::TextObj::artistic("Embedded", vector_core::text::CharFormat { font: family.into(), ..Default::default() });
        let id = doc.new_id();
        doc.pages[0].layers[0].objects.push(vector_core::Object::new(id, vector_core::ObjKind::Text(t), kurbo::Affine::IDENTITY, Default::default()));
        let opts = crate::cdr::SaveOptions { fonts: vec![crate::cdr::EmbeddedFile { name: format!("{family}.ttf"), data: Arc::new(font.clone()) }], ..Default::default() };
        let bytes = crate::cdr::save_to_bytes(&doc, &opts).unwrap();
        assert!(substituted(family));
        let back = crate::cdr::load_from_bytes(&bytes).unwrap();
        assert_eq!(back.used_fonts(), doc.used_fonts());
        assert!(!substituted(family));
        let files = font_files(&back.used_fonts());
        assert_eq!(files[0].name, format!("{family}.ttf"));
        assert_eq!(*files[0].data, font);
        // opening it again (or another drawing that embeds the same font) adds nothing
        let faces = || fonts().lock().db.faces().filter(|f| f.families.iter().any(|(n, _)| n == family)).count();
        assert_eq!(faces(), 1);
        crate::cdr::load_from_bytes(&bytes).unwrap();
        assert_eq!(faces(), 1);
    }

    #[test]
    fn embedding_permissions_are_respected() {
        let Some(font) = test_font() else {
            eprintln!("no installed TrueType font to embed; skipped");
            return;
        };
        // preview & print: shows the text that uses it and is embedded again, but isn't
        // offered for other text
        let print = "AnyVecPrintOnly";
        // restricted: shows the drawing it came with, but is never embedded by AnyVector
        let restricted = "AnyVecRestricted";
        let added = register_embedded_fonts(vec![
            (format!("{print}.ttf"), Arc::new(renamed(&font, print, Some(4)))),
            (format!("{restricted}.ttf"), Arc::new(renamed(&font, restricted, Some(2)))),
        ]);
        assert_eq!(added, vec![print.to_string(), restricted.to_string()]);
        {
            let db = fonts().lock();
            for family in [print, restricted] {
                assert_eq!(db.substitute(family), None);
                assert!(!db.families().iter().any(|f| f == family), "{family}");
            }
        }
        let files = font_files(&[(print.to_string(), false, false), (restricted.to_string(), false, false)]);
        let got: Vec<(&str, bool)> = files.iter().map(|f| (f.name.as_str(), f.embeddable)).collect();
        assert_eq!(got, vec![("AnyVecPrintOnly.ttf", true), ("AnyVecRestricted.ttf", false)]);
        // an editable embedded font is offered like an installed one
        let editable = "AnyVecEditable";
        register_embedded_fonts(vec![(format!("{editable}.ttf"), Arc::new(renamed(&font, editable, Some(8))))]);
        assert!(fonts().lock().families().iter().any(|f| f == editable));
    }
}
