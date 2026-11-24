//! File names and paths that do not follow the rules of the system AnyVector runs on.
//!
//! Windows compares file names without case and takes both `\` and `/` as separators; Linux
//! compares names byte for byte and only knows `/`. Drawings, libraries and data files carry
//! names written on either system (a drawing made on Windows names `C:\Art\Logo.png`), and files
//! often have their extension in capitals (`LOGO.CDR`). These helpers make such names work the
//! same everywhere: extensions are compared without case, a stored path is taken apart at both
//! separators, and a file that is not where its stored path says is looked for by name, ignoring
//! case the way Windows does.

use std::path::{Path, PathBuf};

/// Whether `path` ends in one of `exts` (given without the dot), in any case: `LOGO.CDR` has
/// `cdr`.
pub fn has_extension(path: &Path, exts: &[&str]) -> bool {
    path.extension().is_some_and(|e| {
        let e = e.to_string_lossy();
        exts.iter().any(|x| e.eq_ignore_ascii_case(x))
    })
}

/// `name` without a trailing `.ext` of `exts`, in any case: `("Logo.CDR", ["cdr"])` gives
/// `"Logo"`; a name with another extension is kept whole.
pub fn strip_extension<'a>(name: &'a str, exts: &[&str]) -> &'a str {
    for x in exts {
        let n = x.len() + 1;
        if name.len() >= n && name.is_char_boundary(name.len() - n) {
            let (stem, tail) = name.split_at(name.len() - n);
            if tail.starts_with('.') && tail[1..].eq_ignore_ascii_case(x) {
                return stem;
            }
        }
    }
    name
}

/// The parts of a path written on any system, without the drive (`C:`), the `\\?\` prefix,
/// empty parts, `.` and `..`: `C:\Art\Logo.png` and `/home/ana/Art/Logo.png` both end in
/// `["Art", "Logo.png"]`.
pub fn foreign_components(stored: &str) -> Vec<&str> {
    let s = stored.trim();
    let s = s.strip_prefix(r"\\?\UNC\").or_else(|| s.strip_prefix(r"\\?\")).unwrap_or(s);
    let mut parts: Vec<&str> = s.split(['/', '\\']).filter(|p| !p.is_empty() && *p != "." && *p != "..").collect();
    if parts.first().is_some_and(|p| is_drive(p)) {
        parts.remove(0);
    }
    parts
}

fn is_drive(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// The file name at the end of a path written on any system: `C:\Fonts\ARIAL.TTF` gives
/// `ARIAL.TTF` on Linux too, where `Path::file_name` would give the whole string.
pub fn foreign_file_name(stored: &str) -> &str {
    foreign_components(stored).last().copied().unwrap_or("")
}

/// The entry of `dir` called `name`: the exact name when there is one, else one whose name
/// differs only in case, as Windows finds it (the first in name order when there are several).
pub fn find_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return None;
    }
    let exact = dir.join(name);
    if exact.exists() {
        return Some(exact);
    }
    let want = name.to_lowercase();
    let mut hits: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.to_lowercase() == want))
        .map(|e| e.path())
        .collect();
    hits.sort();
    hits.into_iter().next()
}

/// The entry `parts` (folders, then a name) under `base`, each part found as [`find_in_dir`]
/// finds it.
pub fn find_under(base: &Path, parts: &[&str]) -> Option<PathBuf> {
    let mut p = base.to_path_buf();
    for (i, part) in parts.iter().enumerate() {
        p = find_in_dir(&p, part)?;
        if i + 1 < parts.len() && !p.is_dir() {
            return None;
        }
    }
    (!parts.is_empty()).then_some(p)
}

/// Find a file whose path was stored earlier, maybe on another computer or system (a linked
/// bitmap, a symbol library, a data source): the stored path when it is there; else, under each
/// of `dirs` in turn (the drawing's folder first), the end of the stored path, longest first,
/// each part matched without case. `C:\Work\Logos\a.png` with `/home/ana/Work` in `dirs` finds
/// `/home/ana/Work/Logos/a.png`, else `/home/ana/Work/a.png`.
pub fn resolve_stored_path(stored: &str, dirs: &[&Path]) -> Option<PathBuf> {
    let s = stored.trim();
    if s.is_empty() {
        return None;
    }
    let p = Path::new(s);
    if p.is_absolute() && p.exists() {
        return Some(p.to_path_buf());
    }
    let parts = foreign_components(s);
    for dir in dirs {
        for start in 0..parts.len() {
            if let Some(found) = find_under(dir, &parts[start..]) {
                return Some(found);
            }
        }
    }
    // a relative path that is nowhere else: as it always was, from the working folder
    p.exists().then(|| p.to_path_buf())
}

/// The extensions to give a file dialog filter (`rfd`) so that it lists files whatever the case
/// of their extension. The Windows and macOS dialogs ignore case already: the list as given. The
/// Linux dialogs (the XDG desktop portal, GTK and zenity) match `*.cdr` case-sensitively, which
/// hides `LOGO.CDR`, so each extension is followed by a pattern for it in any case
/// (`[cC][dD][rR]`), as the portal's documentation advises. The plain extensions stay first: KDE
/// takes the extension it adds to a typed name from the first plain pattern.
pub fn dialog_extensions<S: AsRef<str>>(exts: &[S]) -> Vec<String> {
    let mut out: Vec<String> = exts.iter().map(|e| e.as_ref().to_string()).collect();
    if cfg!(any(windows, target_os = "macos")) {
        return out;
    }
    for e in exts {
        if let Some(g) = any_case_pattern(e.as_ref()) {
            if !out.contains(&g) {
                out.push(g);
            }
        }
    }
    out
}

/// An extension as a glob that matches it in any case: `cdr` gives `[cC][dD][rR]`, `jp2` gives
/// `[jJ][pP]2`. None when case cannot matter (no letters) or the extension is a pattern already.
pub fn any_case_pattern(ext: &str) -> Option<String> {
    if ext.contains(['*', '?', '[', ']']) || !ext.chars().any(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut s = String::with_capacity(ext.len() * 4);
    for c in ext.chars() {
        if c.is_ascii_alphabetic() {
            s.push('[');
            s.push(c.to_ascii_lowercase());
            s.push(c.to_ascii_uppercase());
            s.push(']');
        } else {
            s.push(c);
        }
    }
    Some(s)
}

impl crate::FileType {
    /// [`dialog_extensions`] of this type's extensions.
    pub fn dialog_extensions(&self) -> Vec<String> {
        dialog_extensions(self.extensions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh empty folder under the system temp folder.
    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("anyvector-paths-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn extensions_in_any_case() {
        assert!(has_extension(Path::new("LOGO.CDR"), &["cdr"]));
        assert!(has_extension(Path::new("dir/Logo.Cdr"), &["vct", "cdr"]));
        assert!(has_extension(Path::new("a.prs"), &["PRS"]));
        assert!(!has_extension(Path::new("a.cdrx"), &["cdr"]));
        assert!(!has_extension(Path::new("cdr"), &["cdr"]));
        assert_eq!(strip_extension("Logo.CDR", &["cdr", "vct"]), "Logo");
        assert_eq!(strip_extension("Logo.Vct", &["cdr", "vct"]), "Logo");
        assert_eq!(strip_extension("Logo.cdr", &["cdr"]), "Logo");
        assert_eq!(strip_extension("Logo.svg", &["cdr", "vct"]), "Logo.svg");
        assert_eq!(strip_extension("cdr", &["cdr"]), "cdr");
        assert_eq!(strip_extension("Ça.CDR", &["cdr"]), "Ça");
        assert_eq!(strip_extension("日本.cdr", &["cdr"]), "日本");
    }

    #[test]
    fn windows_and_linux_paths_come_apart_the_same() {
        assert_eq!(foreign_components(r"C:\Art\Logo.png"), ["Art", "Logo.png"]);
        assert_eq!(foreign_components("/home/ana/Art/Logo.png"), ["home", "ana", "Art", "Logo.png"]);
        assert_eq!(foreign_components(r"\\server\share\Art\Logo.png"), ["server", "share", "Art", "Logo.png"]);
        assert_eq!(foreign_components(r"\\?\C:\Art\Logo.png"), ["Art", "Logo.png"]);
        assert_eq!(foreign_components(r"C:/Art/./x/../Logo.png"), ["Art", "x", "Logo.png"]);
        assert_eq!(foreign_components("images/a.png"), ["images", "a.png"]);
        assert_eq!(foreign_file_name(r"C:\WINDOWS\Fonts\ARIALBD.TTF"), "ARIALBD.TTF");
        assert_eq!(foreign_file_name("/usr/share/fonts/arial.ttf"), "arial.ttf");
        assert_eq!(foreign_file_name("arial.ttf"), "arial.ttf");
        assert_eq!(foreign_file_name(""), "");
        assert_eq!(foreign_file_name(r"C:\"), "");
    }

    #[test]
    fn files_are_found_without_case_like_windows_finds_them() {
        let d = temp_dir("find");
        std::fs::write(d.join("Logo.PNG"), b"x").unwrap();
        std::fs::create_dir_all(d.join("Images").join("Sub")).unwrap();
        std::fs::write(d.join("Images").join("Sub").join("Photo.Jpg"), b"x").unwrap();
        assert_eq!(find_in_dir(&d, "Logo.PNG"), Some(d.join("Logo.PNG")));
        // Windows gives the name as asked, Linux the name the file has
        let same = |p: PathBuf| p.is_file() && p.file_name().unwrap().to_string_lossy().eq_ignore_ascii_case("logo.png");
        assert!(find_in_dir(&d, "logo.png").is_some_and(same));
        assert!(find_in_dir(&d, "LOGO.png").is_some_and(same));
        if !cfg!(windows) {
            assert_eq!(find_in_dir(&d, "logo.png"), Some(d.join("Logo.PNG")));
        }
        assert_eq!(find_in_dir(&d, "other.png"), None);
        assert_eq!(find_in_dir(&d, ".."), None);
        assert_eq!(find_in_dir(&d, "Images/Sub"), None);
        let photo = find_under(&d, &["images", "SUB", "photo.jpg"]).unwrap();
        assert!(photo.ends_with(Path::new("Images").join("Sub").join("Photo.Jpg")) || cfg!(windows));
        assert!(photo.is_file());
        // a file is not a folder to go through
        assert_eq!(find_under(&d, &["logo.png", "x"]), None);
        assert_eq!(find_under(&d, &[]), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn stored_windows_paths_are_found_next_to_the_drawing() {
        let d = temp_dir("resolve");
        std::fs::create_dir_all(d.join("Logos")).unwrap();
        std::fs::write(d.join("Logos").join("Star.png"), b"x").unwrap();
        std::fs::write(d.join("Data.CSV"), b"x").unwrap();
        // the folder under the drawing's folder that the stored path ends in, in any case
        let star = resolve_stored_path(r"C:\Users\ana\Work\LOGOS\star.PNG", &[&d]).unwrap();
        assert!(star.is_file());
        assert!(star.starts_with(&d));
        assert_eq!(star.file_name().unwrap().to_string_lossy().to_lowercase(), "star.png");
        // only the file name in common
        let data = resolve_stored_path(r"D:\Merge\data.csv", &[&d]).unwrap();
        assert_eq!(data.file_name().unwrap().to_string_lossy().to_lowercase(), "data.csv");
        // a Linux path read on another computer
        assert!(resolve_stored_path("/home/bob/Work/Logos/Star.png", &[&d]).is_some());
        // a relative path
        assert!(resolve_stored_path("logos/star.png", &[&d]).is_some());
        // an existing absolute path is taken as it is
        let abs = d.join("Logos").join("Star.png");
        assert_eq!(resolve_stored_path(&abs.to_string_lossy(), &[]), Some(abs.clone()));
        // the second folder is tried when the first has nothing
        let empty = temp_dir("resolve-empty");
        assert!(resolve_stored_path(r"C:\x\Data.csv", &[&empty, &d]).is_some());
        assert_eq!(resolve_stored_path(r"C:\x\Missing.csv", &[&empty, &d]), None);
        assert_eq!(resolve_stored_path("", &[&d]), None);
        assert_eq!(resolve_stored_path("  ", &[&d]), None);
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&empty);
    }

    #[test]
    fn dialog_patterns_match_any_case() {
        assert_eq!(any_case_pattern("cdr").as_deref(), Some("[cC][dD][rR]"));
        assert_eq!(any_case_pattern("JP2").as_deref(), Some("[jJ][pP]2"));
        assert_eq!(any_case_pattern("7z").as_deref(), Some("7[zZ]"));
        assert_eq!(any_case_pattern("123"), None);
        assert_eq!(any_case_pattern("*"), None);
        assert_eq!(any_case_pattern(""), None);
        assert_eq!(any_case_pattern("[cC]dr"), None);
        let exts = dialog_extensions(&["cdr", "cdt", "*"]);
        if cfg!(any(windows, target_os = "macos")) {
            // the native dialogs ignore case themselves: nothing changes there
            assert_eq!(exts, ["cdr", "cdt", "*"]);
        } else {
            assert_eq!(exts, ["cdr", "cdt", "*", "[cC][dD][rR]", "[cC][dD][tT]"]);
        }
        // the plain extensions always come first, in their order
        assert_eq!(&exts[..3], ["cdr", "cdt", "*"]);
        let t = crate::FileType { name: "CDR (*.cdr)", extensions: &["cdr"], vector: true };
        assert_eq!(t.dialog_extensions()[0], "cdr");
        // the same list twice gives each pattern once
        let twice = dialog_extensions(&["png", "png"]);
        assert_eq!(twice.iter().filter(|e| e.starts_with('[')).count(), usize::from(!cfg!(any(windows, target_os = "macos"))));
    }

    /// Files named in capitals or mixed case (as cameras and Windows programs often name them)
    /// open and import like lower-case ones, and the images an SVG links are found beside it,
    /// also when the link was written on Windows.
    #[test]
    fn mixed_case_files_open_and_import() {
        use vector_core::document::Document;
        let d = temp_dir("open");
        let png = crate::image_io::encode_png(2, 2, &[200u8; 16]);
        std::fs::create_dir_all(d.join("Images")).unwrap();
        std::fs::write(d.join("Images").join("Logo.PNG"), &png).unwrap();
        std::fs::write(d.join("PHOTO.PNG"), &png).unwrap();
        let vct = d.join("LOGO.VCT");
        crate::native::save(&Document::new(100.0, 50.0), &vct).unwrap();
        assert!(crate::open(&vct).is_ok());
        assert!(crate::is_drawing(&vct));
        assert!(crate::read_thumbnail(&vct).is_some());
        assert_eq!(crate::open(&d.join("PHOTO.PNG")).unwrap().images.len(), 1);
        let svg = |name: &str, href: &str| {
            let p = d.join(name);
            let text = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="20" height="20"><image width="10" height="10" xlink:href="{href}"/></svg>"#);
            std::fs::write(&p, text).unwrap();
            p
        };
        // a link relative to the SVG, whatever the working folder
        let rel = svg("Relative.SVG", "Images/Logo.PNG");
        assert_eq!(crate::open(&rel).unwrap().images.len(), 1);
        // links written on Windows: backslashes, another case, a drive, a file URL
        for (name, href) in [("Back.svg", r"images\logo.png"), ("Drive.Svg", r"C:\Users\ana\Art\Images\LOGO.png"), ("Url.svg", "file:///C:/Users/ana/Art/images/logo.png")] {
            let p = svg(name, href);
            assert_eq!(crate::open(&p).unwrap().images.len(), 1, "{href}");
            let mut doc = Document::new(100.0, 100.0);
            assert!(!crate::import(&mut doc, &p).unwrap().is_empty(), "{href}");
            assert_eq!(doc.images.len(), 1, "{href}");
        }
        // a link to nothing still opens, without the image
        let none = svg("None.svg", r"C:\Art\Missing.png");
        assert_eq!(crate::open(&none).unwrap().images.len(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The glob a dialog builds (`*.` and the pattern) accepts the name in every case: checked
    /// with a small matcher for `*`, `?` and `[...]`, as GTK and Qt match them.
    #[test]
    fn any_case_globs_accept_mixed_case_names() {
        fn glob(p: &[char], s: &[char]) -> bool {
            match p.first() {
                None => s.is_empty(),
                Some('*') => (0..=s.len()).any(|i| glob(&p[1..], &s[i..])),
                Some('?') => !s.is_empty() && glob(&p[1..], &s[1..]),
                Some('[') => {
                    let end = p.iter().position(|c| *c == ']').unwrap();
                    !s.is_empty() && p[1..end].contains(&s[0]) && glob(&p[end + 1..], &s[1..])
                }
                Some(c) => s.first() == Some(c) && glob(&p[1..], &s[1..]),
            }
        }
        let pattern: Vec<char> = format!("*.{}", any_case_pattern("cdr").unwrap()).chars().collect();
        for name in ["logo.cdr", "LOGO.CDR", "Logo.Cdr", "x.cDr"] {
            assert!(glob(&pattern, &name.chars().collect::<Vec<_>>()), "{name}");
        }
        for name in ["logo.cdx", "logo.cdr.bak", "logocdr"] {
            assert!(!glob(&pattern, &name.chars().collect::<Vec<_>>()), "{name}");
        }
    }
}
