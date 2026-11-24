//! The version of each system (`[workspace.metadata.versions]` in Cargo.toml) is the same everywhere people
//! and the updater see it: README, release notes, setup, packages, CI.

use std::path::PathBuf;

#[allow(dead_code)]
#[path = "../src/versions.rs"]
mod versions;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives in the repository").to_path_buf()
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}")).replace("\r\n", "\n")
}

const TOML: &str = "[workspace]\n\n# the systems\n[workspace.metadata.versions]\n# a comment = \"9\"\nwindows = \"0.8.0\"\nlinux=\"0.1.2\"\n\n[workspace.package]\nversion = \"0.0.0\"\n";

#[test]
fn versions_are_read_from_cargo_toml() {
    assert_eq!(versions::version::versions(TOML), [("windows".into(), "0.8.0".into()), ("linux".into(), "0.1.2".into())]);
    assert_eq!(versions::version::versions(&TOML.replace('\n', "\r\n")), versions::version::versions(TOML));
    assert_eq!(versions::version::version_on(TOML, "linux"), "0.1.2");
    assert_eq!(versions::version::version_on(TOML, "freebsd"), "0.0.0");
    assert!(versions::version::versions("[workspace.package]\nversion = \"1.0.0\"\n").is_empty());
}

#[test]
fn the_list_has_every_system_with_its_files() {
    let list = versions::list(TOML);
    for part in [
        "| **Windows** | [0.8.0](https://github.com/toim22/AnyVector/releases/tag/v0.8.0-windows) | [Setup](https://github.com/toim22/AnyVector/releases/download/v0.8.0-windows/AnyVector-0.8.0-windows-x64-setup.exe) |\n",
        "| **Linux (beta)** | [0.1.2](https://github.com/toim22/AnyVector/releases/tag/v0.1.2-linux) | [AppImage](https://github.com/toim22/AnyVector/releases/download/v0.1.2-linux/AnyVector-0.1.2-linux-x86_64.AppImage) · [.deb](https://github.com/toim22/AnyVector/releases/download/v0.1.2-linux/AnyVector-0.1.2-linux-amd64.deb) |\n",
    ] {
        assert!(list.contains(part), "{list}");
    }
    let text = versions::release_text(TOML, "# AnyVector 0.1.2 for Linux\r\n\r\nFixes.\r\n");
    assert!(text.starts_with("## Latest versions\n\n| System |") && text.ends_with("\n\n## What is new\n\nFixes.\n"), "{text}");
    // what AnyVector looks for to show the notes only (updater.rs, own_notes)
    let updater = read("crates/anyvector/src/updater.rs");
    assert!(updater.contains("== \"## What is new\" && body.starts_with(\"## Latest versions\")"));
    // only between the markers, whatever the line ends
    let readme = "a\n<!-- versions: cargo run -p xtask -- versions -->\nold\n<!-- /versions -->\nz\n";
    let new = versions::with_list(readme, TOML).unwrap();
    assert!(new.starts_with("a\n<!-- versions") && new.ends_with("<!-- /versions -->\nz\n") && new.contains(&list) && !new.contains("old"));
    assert_eq!(versions::with_list(&new.replace('\n', "\r\n"), TOML).unwrap(), new.replace('\n', "\r\n"));
    assert!(versions::with_list("no markers", TOML).is_err());
}

#[test]
fn the_readme_lists_the_versions_of_cargo_toml() {
    let readme = read("README.md");
    assert_eq!(versions::with_list(&readme, &read("Cargo.toml")).unwrap(), readme, "run: cargo run -p xtask -- versions");
}

#[test]
fn every_version_has_its_release_notes() {
    let help = read("crates/anyvector/src/help.rs");
    for (system, v) in versions::version::versions(&read("Cargo.toml")) {
        let name = format!("RELEASE-NOTES-{v}-{system}.md");
        let title = read(&format!("docs/{name}")).lines().next().unwrap().to_lowercase();
        assert!(title.starts_with(&format!("# anyvector {v} for {system}")), "{name}: {title}");
        assert!(help.contains(&name), "help.rs shows {name}");
        assert!(read("docs/STATUS.md").contains(&name), "docs/STATUS.md lists {name}");
    }
}

/// The files are named as the list, the updater (`updater.rs`) and the CI expect them.
#[test]
fn the_files_are_named_after_their_system() {
    let iss = read("installer/AnyVector.iss");
    assert!(iss.contains("OutputBaseFilename=AnyVector-{#AppVersion}-windows-x64-setup\n"));
    assert!(iss.contains("\"workspace.metadata.versions\", \"windows\""));
    let sh = read("packaging/linux/build.sh");
    for name in ["AnyVector-$version-linux-x86_64.AppImage", "AnyVector-$version-linux-amd64.deb", "releases/tag/v$version-linux"] {
        assert!(sh.contains(name), "{name}");
    }
    assert!(read("crates/anyvector/src/updater.rs").contains("format!(\"AnyVector-{version}-linux-{arch}.AppImage\")"));
    let ci = read(".github/workflows/build.yml");
    for part in ["\"v$ver-windows\"", "\"v$ver-linux\"", "./dist/AnyVector-*-linux-amd64.deb", "./dist/AnyVector-*-linux-x86_64.AppImage"] {
        assert!(ci.contains(part), "{part}");
    }
}
