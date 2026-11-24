//! Reading the installed fonts (see `text::system_fonts`).
//!
//! On Windows this is fontdb's own scan of the fonts folders. On Linux and the other free systems
//! fontdb takes the folders and the generic families (serif, sans-serif, monospace...) from
//! fontconfig's configuration. Two things are added there:
//! - the standard folders are read as well, so fonts are found even where the configuration leaves
//!   a folder out or is missing (containers, minimal systems): the user's `~/.local/share/fonts`
//!   (`$XDG_DATA_HOME/fonts`) and `~/.fonts`, and `fonts` in each of `$XDG_DATA_DIRS` (by default
//!   `/usr/local/share/fonts` and `/usr/share/fonts`);
//! - the generic families are given installed faces. fontconfig may name a family that isn't
//!   installed (Ubuntu's configuration names FreeSans), and text whose font is missing then showed
//!   in whatever face happened to come first. The Windows family comes first (Arial for
//!   sans-serif, as fontdb has it on Windows), then the open fonts made to its metrics
//!   (`fontsub::EQUIVALENTS`: Liberation Sans), then fontconfig's choice, then fonts most
//!   systems have, so a drawing measures the same as on Windows wherever it can.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Read the installed fonts into `db`.
pub fn load(db: &mut fontdb::Database) {
    db.load_system_fonts();
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
    {
        load_missing(db, &standard_dirs(|k| std::env::var(k).ok()));
        settle_generic_families(db);
    }
}

/// The folders fonts are installed in on Linux and the other free systems (the XDG base
/// directories, as fontconfig's own configuration lists them), the user's first: `fonts` in
/// `$XDG_DATA_HOME` (else `~/.local/share`), the older `~/.fonts`, then `fonts` in each of
/// `$XDG_DATA_DIRS` (else `/usr/local/share` and `/usr/share`), and always `/usr/share/fonts` and
/// `/usr/local/share/fonts`. `env` reads an environment variable; relative paths are ignored, as
/// the XDG specification says (a path is absolute when it starts at the root, so the rules can be
/// tested on Windows too).
pub fn standard_dirs(env: impl Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let abs = |v: Option<String>| v.map(PathBuf::from).filter(|p| p.has_root());
    let home = abs(env("HOME"));
    let mut out: Vec<PathBuf> = vec![];
    match abs(env("XDG_DATA_HOME")) {
        Some(d) => out.push(d.join("fonts")),
        None => out.extend(home.iter().map(|h| h.join(".local/share/fonts"))),
    }
    out.extend(home.iter().map(|h| h.join(".fonts")));
    let data = env("XDG_DATA_DIRS").filter(|v| !v.trim().is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    out.extend(data.split(':').map(PathBuf::from).filter(|p| p.has_root()).map(|p| p.join("fonts")));
    out.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(PathBuf::from));
    let mut seen = HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

/// A font file fontdb reads: `.ttf`, `.otf` and their collections, in any capitals.
fn is_font_file(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| ["ttf", "otf", "ttc", "otc"].iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// Read the font files under `dirs` (and their subfolders) that `db` doesn't have yet, comparing
/// real paths, so a file reached through another folder or a link is read once.
pub fn load_missing(db: &mut fontdb::Database, dirs: &[PathBuf]) {
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let mut have: HashSet<PathBuf> = db
        .faces()
        .filter_map(|f| match &f.source {
            fontdb::Source::File(p) | fontdb::Source::SharedFile(p, _) => Some(real(p)),
            fontdb::Source::Binary(_) => None,
        })
        .collect();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut stack: Vec<PathBuf> = dirs.iter().rev().cloned().collect();
    while let Some(dir) = stack.pop() {
        // a folder once, however it is reached (links can make loops)
        if !visited.insert(real(&dir)) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        // the same order on every run
        entries.sort();
        let mut subdirs = vec![];
        for p in entries {
            if p.is_dir() {
                subdirs.push(p);
            } else if p.is_file() && is_font_file(&p) && have.insert(real(&p)) {
                // a file that can't be read is left out, as fontdb's own scan does
                let _ = db.load_font_file(&p);
            }
        }
        stack.extend(subdirs.into_iter().rev());
    }
}

/// The family `fontdb` names a generic family by on Windows, where it is always installed.
const WINDOWS_GENERIC: [(fontdb::Family<'static>, &str); 5] = [
    (fontdb::Family::Serif, "Times New Roman"),
    (fontdb::Family::SansSerif, "Arial"),
    (fontdb::Family::Monospace, "Courier New"),
    (fontdb::Family::Cursive, "Comic Sans MS"),
    (fontdb::Family::Fantasy, "Impact"),
];

/// Families most free systems have, by generic family, after the Windows one and its equivalents
/// and fontconfig's choice.
fn common(generic: fontdb::Family<'_>) -> &'static [&'static str] {
    match generic {
        fontdb::Family::Serif => &["DejaVu Serif", "Noto Serif", "FreeSerif"],
        fontdb::Family::Monospace => &["DejaVu Sans Mono", "Noto Sans Mono", "FreeMono", "Ubuntu Mono"],
        fontdb::Family::Cursive => &["URW Chancery L", "Z003"],
        _ => &["DejaVu Sans", "Noto Sans", "FreeSans", "Ubuntu", "Cantarell", "Open Sans", "Roboto"],
    }
}

/// The family to use for `generic`, best first: the Windows family, the open fonts made to its
/// metrics, `configured` (fontconfig's choice), then [`common`] ones. `installed` says whether a
/// family is installed. None when none of them is.
pub fn generic_family(generic: fontdb::Family<'_>, configured: &str, installed: impl Fn(&str) -> bool) -> Option<String> {
    let windows = WINDOWS_GENERIC.iter().find(|(g, _)| *g == generic).map(|(_, n)| *n)?;
    std::iter::once(windows)
        .chain(crate::fontsub::equivalents(windows).iter().copied())
        .chain(std::iter::once(configured))
        .chain(common(generic).iter().copied())
        .find(|f| !f.is_empty() && installed(f))
        .map(str::to_string)
}

/// Give each generic family an installed family (see the module notes); one with none of its
/// candidates installed gets sans-serif's, or the first installed family by name.
pub fn settle_generic_families(db: &mut fontdb::Database) {
    let installed: HashSet<String> = db.faces().flat_map(|f| f.families.iter().map(|(n, _)| n.clone())).collect();
    if installed.is_empty() {
        return;
    }
    let first = installed.iter().min_by_key(|n| (n.to_lowercase(), (*n).clone())).cloned().unwrap_or_default();
    let has = |f: &str| installed.contains(f);
    let pick = |db: &fontdb::Database, g: fontdb::Family<'_>| generic_family(g, db.family_name(&g), has);
    let sans = pick(db, fontdb::Family::SansSerif).unwrap_or(first);
    let serif = pick(db, fontdb::Family::Serif).unwrap_or_else(|| sans.clone());
    let mono = pick(db, fontdb::Family::Monospace).unwrap_or_else(|| sans.clone());
    let cursive = pick(db, fontdb::Family::Cursive).unwrap_or_else(|| sans.clone());
    let fantasy = pick(db, fontdb::Family::Fantasy).unwrap_or_else(|| sans.clone());
    db.set_sans_serif_family(sans);
    db.set_serif_family(serif);
    db.set_monospace_family(mono);
    db.set_cursive_family(cursive);
    db.set_fantasy_family(fantasy);
}

/// Give each Windows font that isn't installed the faces of its first installed open equivalent
/// (`fontsub::EQUIVALENTS`: Liberation Sans for Arial), under the Windows name. For programs that
/// find fonts by name only, as usvg, which sets an SVG's text in its first family that is
/// installed, else in the serif face: an SVG's Arial then measures as on Windows instead of
/// turning serif. Not for the drawings' fonts: `FontDb::resolve` knows the equivalents, and tells
/// the font is substituted.
pub fn alias_equivalents(db: &mut fontdb::Database) {
    let named = |f: &fontdb::FaceInfo, name: &str| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name));
    for (windows, equivalents) in crate::fontsub::EQUIVALENTS {
        if db.faces().any(|f| named(f, windows)) {
            continue;
        }
        let Some(faces) = equivalents.iter().map(|e| db.faces().filter(|f| named(f, e)).cloned().collect::<Vec<_>>()).find(|v| !v.is_empty()) else {
            continue;
        };
        for mut f in faces {
            f.families = vec![(windows.to_string(), fontdb::Language::English_UnitedStates)];
            db.push_face_info(f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
    }

    #[test]
    fn standard_folders_follow_the_xdg_variables() {
        let p = |s: &str| PathBuf::from(s);
        // the defaults
        assert_eq!(
            standard_dirs(env(&[("HOME", "/home/ana")])),
            vec![p("/home/ana/.local/share/fonts"), p("/home/ana/.fonts"), p("/usr/local/share/fonts"), p("/usr/share/fonts")]
        );
        // the user's and the system's data folders moved; the standard ones are still read
        assert_eq!(
            standard_dirs(env(&[("HOME", "/home/ana"), ("XDG_DATA_HOME", "/data/ana"), ("XDG_DATA_DIRS", "/opt/share:/usr/share:relative/share")])),
            vec![p("/data/ana/fonts"), p("/home/ana/.fonts"), p("/opt/share/fonts"), p("/usr/share/fonts"), p("/usr/local/share/fonts")]
        );
        // no home folder, a relative XDG_DATA_HOME (ignored), an empty XDG_DATA_DIRS (the default)
        assert_eq!(standard_dirs(env(&[("XDG_DATA_HOME", "rel"), ("XDG_DATA_DIRS", "")])), vec![p("/usr/local/share/fonts"), p("/usr/share/fonts")]);
    }

    #[test]
    fn font_files_are_known_by_their_extension_in_any_capitals() {
        for f in ["a.ttf", "b.OTF", "c.Ttc", "d.otc"] {
            assert!(is_font_file(Path::new(f)), "{f}");
        }
        for f in ["a.pfb", "b.ttf.bak", "fonts.dir", "c"] {
            assert!(!is_font_file(Path::new(f)), "{f}");
        }
    }

    /// A font file to copy around: Selawik, bundled with the app.
    const FONT: &[u8] = include_bytes!("../../anyvector/assets/fonts/selawk.ttf");

    #[test]
    fn fonts_in_the_standard_folders_are_read_once() {
        let root = std::env::temp_dir().join(format!("anyvector-sysfonts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let sub = root.join("user/truetype/selawik");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("Selawik.TTF"), FONT).unwrap();
        std::fs::write(sub.join("notes.txt"), b"not a font").unwrap();
        let mut db = fontdb::Database::new();
        // one folder given twice, and its parent: the file is read once
        let dirs = [root.join("user"), root.join("user/truetype"), root.join("user"), root.join("missing")];
        load_missing(&mut db, &dirs);
        assert_eq!(db.len(), 1);
        assert_eq!(db.faces().next().unwrap().families[0].0, "Selawik");
        // a database that has it already doesn't read it again
        load_missing(&mut db, &dirs);
        assert_eq!(db.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn generic_families_prefer_windows_metrics_then_fontconfig() {
        let installed = |list: &'static [&'static str]| move |f: &str| list.contains(&f);
        let pick = |g, configured, list| generic_family(g, configured, installed(list));
        // Windows' own family when installed
        assert_eq!(pick(fontdb::Family::SansSerif, "DejaVu Sans", &["Arial", "DejaVu Sans"]).as_deref(), Some("Arial"));
        // else the one made to its metrics, before fontconfig's choice
        assert_eq!(pick(fontdb::Family::SansSerif, "DejaVu Sans", &["Liberation Sans", "DejaVu Sans"]).as_deref(), Some("Liberation Sans"));
        assert_eq!(pick(fontdb::Family::Serif, "Noto Serif", &["Liberation Serif", "Noto Serif"]).as_deref(), Some("Liberation Serif"));
        assert_eq!(pick(fontdb::Family::Monospace, "Noto Sans Mono", &["Liberation Mono", "Noto Sans Mono"]).as_deref(), Some("Liberation Mono"));
        // fontconfig's choice when it is installed (FreeSans isn't, as on Ubuntu)
        assert_eq!(pick(fontdb::Family::SansSerif, "Noto Sans", &["Noto Sans", "DejaVu Sans"]).as_deref(), Some("Noto Sans"));
        assert_eq!(pick(fontdb::Family::SansSerif, "FreeSans", &["Noto Sans", "DejaVu Sans"]).as_deref(), Some("DejaVu Sans"));
        assert_eq!(pick(fontdb::Family::SansSerif, "FreeSans", &["Some Font"]), None);
    }

    #[test]
    fn every_generic_family_gets_an_installed_face() {
        let mut db = fontdb::Database::new();
        db.load_font_data(FONT.to_vec());
        // what fontconfig set on Ubuntu, none of it installed here
        db.set_sans_serif_family("FreeSans");
        db.set_serif_family("FreeSerif");
        settle_generic_families(&mut db);
        for g in [fontdb::Family::SansSerif, fontdb::Family::Serif, fontdb::Family::Monospace, fontdb::Family::Cursive, fontdb::Family::Fantasy] {
            assert_eq!(db.family_name(&g), "Selawik");
            assert!(db.query(&fontdb::Query { families: &[g], ..Default::default() }).is_some());
        }
    }

    #[test]
    fn windows_names_get_the_faces_of_their_installed_equivalents() {
        // Selawik under made-up names: a regular and a bold face of each family
        let mut db = fontdb::Database::new();
        for (family, file) in [("Liberation Sans", "selawk.ttf"), ("Liberation Sans", "selawkb.ttf"), ("Times New Roman", "selawk.ttf"), ("Carlito", "selawk.ttf")] {
            let data = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../anyvector/assets/fonts").join(file)).unwrap();
            let id = db.load_font_source(fontdb::Source::Binary(std::sync::Arc::new(data)))[0];
            let mut face = db.face(id).unwrap().clone();
            db.remove_face(id);
            face.families = vec![(family.into(), fontdb::Language::English_UnitedStates)];
            face.post_script_name = format!("{family} {file}");
            db.push_face_info(face);
        }
        alias_equivalents(&mut db);
        let faces = |name: &str| {
            let mut v: Vec<(String, u16)> = db.faces().filter(|f| f.families.iter().any(|(n, _)| n == name)).map(|f| (f.post_script_name.clone(), f.weight.0)).collect();
            v.sort();
            v
        };
        // Arial is Liberation Sans, both faces; Calibri Carlito
        assert_eq!(faces("Arial"), faces("Liberation Sans"));
        assert_eq!(faces("Arial").len(), 2);
        assert_eq!(faces("Calibri"), faces("Carlito"));
        let bold = db.query(&fontdb::Query { families: &[fontdb::Family::Name("Arial")], weight: fontdb::Weight::BOLD, ..Default::default() });
        assert_eq!(db.face(bold.unwrap()).unwrap().post_script_name, "Liberation Sans selawkb.ttf");
        // an installed Windows font is itself; one with no equivalent installed is left out
        assert_eq!(faces("Times New Roman").len(), 1);
        assert!(faces("Cambria").is_empty());
        assert!(faces("Courier New").is_empty());
        // once only
        let n = db.len();
        alias_equivalents(&mut db);
        assert_eq!(db.len(), n);
    }

    /// The installed fonts as the program reads them: on Linux every generic family is an
    /// installed one (fontconfig's choice may not be).
    #[test]
    fn the_system_fonts_have_installed_generic_families() {
        let db = crate::text::system_fonts();
        if db.is_empty() {
            eprintln!("no fonts installed; skipped");
            return;
        }
        if cfg!(all(unix, not(target_os = "macos"))) {
            for g in [fontdb::Family::SansSerif, fontdb::Family::Serif, fontdb::Family::Monospace] {
                assert!(db.query(&fontdb::Query { families: &[g], ..Default::default() }).is_some(), "{g:?} = {}", db.family_name(&g));
            }
        }
    }
}
