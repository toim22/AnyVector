//! The data files of the Linux packages (`packaging/linux`) against the program they describe: its
//! application id, the files it opens, its version, its languages and its licenses.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives in the repository").to_path_buf()
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}")).replace("\r\n", "\n")
}

/// The Linux `APP_ID` of the anyvector crate (a program, so it cannot be used from here).
fn app_id() -> String {
    let main = read("crates/anyvector/src/main.rs");
    let linux = main.split("#[cfg(target_os = \"linux\")]\npub const APP_ID: &str = \"").nth(1).expect("APP_ID for Linux in main.rs");
    linux.split('"').next().unwrap().to_string()
}

fn file(ext: &str) -> String {
    read(&format!("packaging/linux/{}.{ext}", app_id()))
}

/// The keys of the desktop file.
fn desktop() -> BTreeMap<String, String> {
    let text = file("desktop");
    assert!(text.starts_with("[Desktop Entry]\n"));
    let mut keys = BTreeMap::new();
    for l in text.lines().skip(1).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (k, v) = l.split_once('=').unwrap_or_else(|| panic!("{l}"));
        assert!(keys.insert(k.to_string(), v.to_string()).is_none(), "{k} twice");
    }
    keys
}

/// The MIME type of each extension of File ▸ Open.
const TYPES: [(&str, &str); 14] = [
    ("cdr", "application/vnd.corel-draw"),
    ("cdt", "image/x-coreldrawtemplate"),
    ("cdrt", "image/x-coreldrawtemplate"),
    ("cmx", "image/x-cmx"),
    ("vct", vector_io::native::MIMETYPE),
    ("svg", "image/svg+xml"),
    ("svgz", "image/svg+xml-compressed"),
    ("pdf", "application/pdf"),
    ("eps", "image/x-eps"),
    ("epsf", "image/x-eps"),
    ("ps", "application/postscript"),
    // a print file: PostScript or not, so it has no type of its own
    ("prn", "application/postscript"),
    ("ai", "application/illustrator"),
    ("dxf", "image/vnd.dxf"),
];
/// The ones shared-mime-info does not have (2.1, Ubuntu 22.04, to 2.4): `mime.xml` adds them.
const OWN: [&str; 4] = ["cdrt", "cdt", "cmx", "vct"];

fn mime_of(ext: &str) -> &'static str {
    TYPES.iter().find(|(e, _)| *e == ext).unwrap_or_else(|| panic!("File > Open reads .{ext}: give it a MIME type here and in packaging/linux")).1
}

#[test]
fn desktop_file_is_the_app_ids() {
    let id = app_id();
    let keys = desktop();
    for (key, value) in [("Type", "Application"), ("Name", "AnyVector"), ("Icon", &id), ("StartupWMClass", &id), ("Exec", "anyvector %F"), ("Terminal", "false")] {
        assert_eq!(keys.get(key).map(String::as_str), Some(value), "{key}");
    }
    // the metainfo file is the same program's
    let text = file("metainfo.xml");
    let meta = roxmltree::Document::parse(&text).unwrap();
    let get = |tag: &str| meta.root_element().children().find(|n| n.has_tag_name(tag)).and_then(|n| n.text()).unwrap_or_default().to_string();
    assert_eq!(get("id"), id);
    assert_eq!(get("launchable"), format!("{id}.desktop"));
    assert_eq!((get("metadata_license"), get("project_license")), ("CC0-1.0".into(), "LicenseRef-proprietary".into()));
    // its categories are the desktop file's
    let categories: Vec<_> = meta.descendants().filter(|n| n.has_tag_name("category")).filter_map(|n| n.text()).collect();
    assert_eq!(keys["Categories"], format!("{};", categories.join(";")));
}

#[test]
fn desktop_file_opens_what_file_open_reads() {
    let keys = desktop();
    let listed: Vec<&str> = keys["MimeType"].strip_suffix(';').expect("a ; at the end").split(';').collect();
    let mut opened = BTreeSet::new();
    for t in vector_io::OPEN_TYPES {
        for ext in t.extensions {
            opened.insert(mime_of(ext));
        }
    }
    assert_eq!(listed.iter().copied().collect::<BTreeSet<_>>(), opened);
    assert_eq!(listed.len(), opened.len(), "a type listed twice");
}

#[test]
fn mime_file_has_the_types_the_system_lacks() {
    let text = file("mime.xml");
    let doc = roxmltree::Document::parse(&text).unwrap();
    let mut globs = BTreeMap::new();
    for t in doc.descendants().filter(|n| n.has_tag_name("mime-type")) {
        let ty = t.attribute("type").unwrap();
        // never `glob-deleteall` or a weight: other programs' types keep their files
        for c in t.children().filter(|c| c.is_element()) {
            assert!(!c.tag_name().name().ends_with("deleteall"), "{ty}");
            assert_eq!(c.attribute("weight"), None, "{ty}");
        }
        for g in t.children().filter(|n| n.has_tag_name("glob")) {
            globs.insert(g.attribute("pattern").unwrap().strip_prefix("*.").unwrap().to_string(), ty.to_string());
        }
    }
    let own: BTreeMap<String, String> = OWN.iter().map(|e| (e.to_string(), mime_of(e).to_string())).collect();
    assert_eq!(globs, own);
    // what the desktop file lists and shared-mime-info lacks is here, and nothing more
    let listed = &desktop()["MimeType"];
    for ty in globs.values() {
        assert!(listed.split(';').any(|l| l == ty), "{ty}");
    }
    // a .vct is told from a vCard by its first bytes
    let vct = vector_io::native::save_to_bytes(&vector_core::document::Document::new(100.0, 100.0)).unwrap();
    let start = format!("mimetype{}", vector_io::native::MIMETYPE);
    assert!(vct.starts_with(b"PK\x03\x04") && vct[30..].starts_with(start.as_bytes()));
    assert!(text.contains(&format!("value=\"{start}\" offset=\"30\"")));
}

#[test]
fn captions_are_in_the_programs_languages() {
    const EN: &str = "Vector graphics editor";
    let keys = desktop();
    let meta = file("metainfo.xml");
    let mime = file("mime.xml");
    assert_eq!(keys["Comment"], EN);
    assert!(meta.contains(&format!("<summary>{EN}</summary>")));
    let mut languages = 0;
    for e in std::fs::read_dir(root().join("crates/anyvector/assets/i18n")).unwrap() {
        let path = e.unwrap().path();
        let code = path.file_stem().unwrap().to_string_lossy().to_string();
        // pt_BR, zh_CN, zh_TW; the others by language only
        let lang = if code.starts_with("pt") || code.starts_with("zh") { code.replace('-', "_") } else { code[..2].to_string() };
        let tsv = std::fs::read_to_string(&path).unwrap();
        let caption = tsv.lines().find_map(|l| l.strip_prefix(EN)?.strip_prefix('\t')).unwrap_or_else(|| panic!("{code}")).trim_end();
        assert_eq!(keys.get(&format!("Comment[{lang}]")).map(String::as_str), Some(caption), "{code}");
        assert!(meta.contains(&format!("<summary xml:lang=\"{lang}\">{caption}</summary>")), "{code}");
        assert_eq!(mime.matches(&format!("<comment xml:lang=\"{lang}\">")).count(), 2, "{code}");
        languages += 1;
    }
    assert_eq!(keys.keys().filter(|k| k.starts_with("Comment[")).count(), languages);
}

#[test]
fn icons_are_drawn_in_their_sizes() {
    let id = app_id();
    for n in [16u32, 22, 24, 32, 48, 64, 128, 256, 512] {
        let path = root().join(format!("packaging/linux/icons/hicolor/{n}x{n}/apps/{id}.png"));
        assert_eq!(image::image_dimensions(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())), (n, n));
    }
    assert_eq!(std::fs::read_dir(root().join("packaging/linux/icons/hicolor")).unwrap().count(), 9, "a size this test does not know");
}

#[test]
fn metainfo_lists_this_version_first() {
    let text = file("metainfo.xml");
    let doc = roxmltree::Document::parse(&text).unwrap();
    let releases: Vec<(&str, &str)> = doc.descendants().filter(|n| n.has_tag_name("release")).map(|n| (n.attribute("version").unwrap(), n.attribute("date").unwrap())).collect();
    assert_eq!(releases[0].0, env!("CARGO_PKG_VERSION"), "add this version, with its release date, to packaging/linux/{}.metainfo.xml", app_id());
    let number = |v: &str| v.split('.').map(|n| n.parse::<u32>().unwrap()).collect::<Vec<_>>();
    for w in releases.windows(2) {
        assert!(number(w[0].0) > number(w[1].0) && w[0].1 >= w[1].1, "{w:?}");
    }
}

#[test]
fn packages_ship_the_licenses() {
    // the MPL-2.0 components: each one's source is named in the copyright file
    let notices = read("crates/anyvector/assets/legal/third-party-licenses.txt");
    let copyright = read("packaging/linux/copyright");
    let mut mpl = 0;
    let mut name = "";
    for l in notices.lines() {
        if let Some(license) = l.strip_prefix("    License: ") {
            if license.contains("MPL") {
                let what = name.split(' ').next().unwrap();
                assert!(copyright.contains(&format!("- {what}:")) || copyright.contains(&format!("- the {name}:")), "{name} ({license})");
                mpl += 1;
            }
        } else if !l.starts_with(' ') {
            name = l;
        }
    }
    assert!(mpl >= 4, "{mpl}");
    // and the files the Windows setup installs, plus the Linux-only font's license, are packaged
    let build = read("packaging/linux/build.sh");
    for f in ["crates/vector-io/src/corel/palettes.rs", "crates/anyvector/assets/legal/MPL-2.0.txt", "crates/anyvector/assets/legal/Selawik-OFL.txt", "crates/anyvector/assets/legal/third-party-licenses.txt"] {
        assert!(build.contains(&format!("$root/{f}")), "{f}");
    }
}

#[test]
fn deb_needs_what_anyvector_starts() {
    let control = read("packaging/linux/control.in");
    let field = |name: &str| control.lines().find_map(|l| l.strip_prefix(name)).unwrap().split(", ").map(|s| s.trim().to_string()).collect::<Vec<_>>();
    let depends = field("Depends:");
    // without one of the two the file dialogs do not open; xdg-utils opens pages, files and e-mails
    for d in ["@SHLIBS@", "xdg-desktop-portal | zenity", "xdg-utils", "libdbus-1-3", "libx11-6", "libwayland-client0", "libxkbcommon0"] {
        assert!(depends.iter().any(|x| x == d), "{d}");
    }
    let recommends = field("Recommends:");
    for d in ["cups-client", "fonts-liberation", "fonts-crosextra-carlito", "fonts-crosextra-caladea", "fonts-dejavu-core", "fonts-noto-core"] {
        assert!(recommends.iter().any(|x| x == d), "{d}");
    }
}

/// A shell script with CRLF lines, or a desktop file with them, does not work on Linux.
#[test]
fn files_have_lf_lines() {
    for e in std::fs::read_dir(root().join("packaging/linux")).unwrap() {
        let path = e.unwrap().path();
        if path.is_file() {
            assert!(!std::fs::read(&path).unwrap().contains(&b'\r'), "{}", path.display());
        }
    }
}
