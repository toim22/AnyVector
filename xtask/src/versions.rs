//! The version of each system and where people see it: `cargo run -p xtask -- versions` writes the list of
//! the latest versions (with its downloads) in README.md, and the text of each system's GitHub release
//! (that list, then its release notes) in target/release-text. `xtask/tests/versions.rs` fails until the
//! README has the versions of Cargo.toml.

use crate::{root, Result};

#[allow(dead_code)]
#[path = "../../crates/vector-core/version.rs"]
pub mod version;

const REPO: &str = "https://github.com/toim22/AnyVector";

pub fn version_on(system: &str) -> String {
    version::version_on(&std::fs::read_to_string(root().join("Cargo.toml")).expect("Cargo.toml"), system)
}

/// A system's name and the files of its release (what the link says, the file).
fn system(id: &str, v: &str) -> (&'static str, Vec<(&'static str, String)>) {
    match id {
        "windows" => ("Windows", vec![("Setup", format!("AnyVector-{v}-windows-x64-setup.exe"))]),
        "linux" => ("Linux (beta)", vec![("AppImage", format!("AnyVector-{v}-linux-x86_64.AppImage")), (".deb", format!("AnyVector-{v}-linux-amd64.deb"))]),
        _ => panic!("what are the files of a release for {id}?"),
    }
}

/// The list of the latest versions, in Markdown.
pub fn list(cargo_toml: &str) -> String {
    let mut s = String::from("| System | Latest version | Download |\n| --- | --- | --- |\n");
    for (id, v) in version::versions(cargo_toml) {
        let (name, files) = system(&id, &v);
        let links: Vec<String> = files.iter().map(|(what, file)| format!("[{what}]({REPO}/releases/download/v{v}-{id}/{file})")).collect();
        s += &format!("| **{name}** | [{v}]({REPO}/releases/tag/v{v}-{id}) | {} |\n", links.join(" · "));
    }
    s
}

/// `readme` with the list of the latest versions between its markers.
pub fn with_list(readme: &str, cargo_toml: &str) -> Result<String> {
    let crlf = readme.contains("\r\n");
    let text = readme.replace("\r\n", "\n");
    let (start, end) = ("<!-- versions: cargo run -p xtask -- versions -->\n", "<!-- /versions -->");
    let a = text.find(start).ok_or("README.md has no versions marker")? + start.len();
    let b = a + text[a..].find(end).ok_or("README.md has no end of the versions")?;
    let text = format!("{}{}{}", &text[..a], list(cargo_toml), &text[b..]);
    Ok(if crlf { text.replace('\n', "\r\n") } else { text })
}

/// The text of the GitHub release of a system: the list, then the release notes without their title
/// (the release has one).
pub fn release_text(cargo_toml: &str, notes: &str) -> String {
    let notes = notes.replace("\r\n", "\n");
    let body = notes.split_once('\n').map_or("", |(_, b)| b).trim_start();
    format!("## Latest versions\n\n{}\n## What is new\n\n{body}", list(cargo_toml))
}

pub fn run() -> Result<()> {
    let toml = std::fs::read_to_string(root().join("Cargo.toml"))?;
    let readme = root().join("README.md");
    std::fs::write(&readme, with_list(&std::fs::read_to_string(&readme)?, &toml)?)?;
    println!("wrote {}", readme.display());
    let dir = root().join("target/release-text");
    std::fs::create_dir_all(&dir)?;
    for (id, v) in version::versions(&toml) {
        let notes = std::fs::read_to_string(root().join(format!("docs/RELEASE-NOTES-{v}-{id}.md")))?;
        let file = dir.join(format!("v{v}-{id}.md"));
        std::fs::write(&file, release_text(&toml, &notes))?;
        println!("wrote {}", file.display());
    }
    Ok(())
}
