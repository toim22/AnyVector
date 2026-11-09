//! Finding the ICC color profiles named in a document's color settings, so "Embed color
//! profiles" can store the real profile data in the drawing.
//!
//! Profiles are looked up in the Windows color directory
//! (`%WINDIR%\System32\spool\drivers\color`) by the description stored inside each ICC file
//! (e.g. "sRGB IEC61966-2.1"), which is the name the dialogs show.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use vector_core::document::ColorSettings;
use vector_io::cdr::EmbeddedFile;

struct Installed {
    path: PathBuf,
    description: String,
}

fn color_dir() -> PathBuf {
    let win = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    win.join("System32").join("spool").join("drivers").join("color")
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

/// The profile description ('desc' tag) of an ICC profile.
pub fn icc_description(b: &[u8]) -> Option<String> {
    if b.len() < 132 || b.get(36..40)? != b"acsp" {
        return None;
    }
    let count = be32(b, 128)? as usize;
    for i in 0..count.min(200) {
        let e = 132 + i * 12;
        if b.get(e..e + 4)? != b"desc" {
            continue;
        }
        let off = be32(b, e + 4)? as usize;
        let len = be32(b, e + 8)? as usize;
        let tag = b.get(off..off + len)?;
        return match tag.get(0..4)? {
            // textDescriptionType: ASCII count + string
            b"desc" => {
                let n = be32(tag, 8)? as usize;
                let s = tag.get(12..12 + n)?;
                Some(String::from_utf8_lossy(s).trim_end_matches('\0').trim().to_string())
            }
            // multiLocalizedUnicodeType: first record, UTF-16BE
            b"mluc" => {
                let rec_len = be32(tag, 20)? as usize;
                let rec_off = be32(tag, 24)? as usize;
                let s = tag.get(rec_off..rec_off + rec_len)?;
                let u: Vec<u16> = s.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                Some(String::from_utf16_lossy(&u).trim_end_matches('\0').trim().to_string())
            }
            _ => None,
        };
    }
    None
}

fn installed() -> &'static Vec<Installed> {
    static LIST: OnceLock<Vec<Installed>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut v = vec![];
        if let Ok(rd) = std::fs::read_dir(color_dir()) {
            for e in rd.flatten() {
                let p = e.path();
                let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                if ext != "icm" && ext != "icc" {
                    continue;
                }
                // only the header and tag table are needed for the description
                if let Ok(b) = std::fs::read(&p) {
                    if let Some(d) = icc_description(&b) {
                        v.push(Installed { path: p, description: d });
                    }
                }
            }
        }
        v
    })
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

/// The installed profile files for the document's RGB, CMYK and grayscale profiles.
/// Profiles that are not installed on this computer are skipped.
pub fn profile_files(c: &ColorSettings) -> Vec<EmbeddedFile> {
    let mut out: Vec<EmbeddedFile> = vec![];
    for name in [&c.rgb_profile, &c.cmyk_profile, &c.gray_profile] {
        let n = norm(name);
        let Some(found) = installed().iter().find(|i| norm(&i.description) == n) else { continue };
        let file = found.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if out.iter().any(|f| f.name == file) {
            continue;
        }
        if let Ok(data) = std::fs::read(&found.path) {
            out.push(EmbeddedFile { name: file, data: Arc::new(data) });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_desc_tag() {
        // minimal ICC: 128-byte header with 'acsp', tag table with one 'desc' tag
        let mut b = vec![0u8; 128];
        b[36..40].copy_from_slice(b"acsp");
        b.extend_from_slice(&1u32.to_be_bytes());
        let text = b"sRGB IEC61966-2.1\0";
        let off = 132 + 12;
        b.extend_from_slice(b"desc");
        b.extend_from_slice(&(off as u32).to_be_bytes());
        b.extend_from_slice(&((12 + text.len()) as u32).to_be_bytes());
        b.extend_from_slice(b"desc\0\0\0\0");
        b.extend_from_slice(&(text.len() as u32).to_be_bytes());
        b.extend_from_slice(text);
        assert_eq!(icc_description(&b).as_deref(), Some("sRGB IEC61966-2.1"));
    }

    #[test]
    fn finds_windows_srgb() {
        // Every Windows install ships the sRGB profile.
        if cfg!(windows) {
            let files = profile_files(&ColorSettings::default());
            assert!(files.iter().any(|f| f.data.len() > 1000), "sRGB profile not found in {}", color_dir().display());
        }
    }
}
