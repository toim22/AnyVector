//! `target/visual/compare.html`: each Vector render (written by the visual tests to
//! `target/visual/`) next to the reference screenshot it should match.
//!
//! The pairs are listed in `docs/reference-map.tsv`: `render file \t reference path \t title`,
//! the reference path being relative to the reference folder. Renders without a pair are shown
//! alone at the end, so nothing is missed.

use crate::{reference, root, Result};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;

fn file_url(p: &Path) -> String {
    let s = p.display().to_string().replace('\\', "/");
    let enc: String = s
        .chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '#' => "%23".to_string(),
            '?' => "%3F".to_string(),
            _ => c.to_string(),
        })
        .collect();
    format!("file:///{}", enc.trim_start_matches('/'))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn run() -> Result<()> {
    let visual = root().join("target/visual");
    let map = std::fs::read_to_string(root().join("docs/reference-map.tsv")).unwrap_or_default();
    let mut html = String::from(
        "<!doctype html><meta charset=utf-8><title>Vector vs reference</title><style>\
         body{background:#1e1e1e;color:#ddd;font:14px Segoe UI,sans-serif;margin:16px}\
         h2{font-weight:600;font-size:15px;margin:28px 0 8px}\
         .pair{display:grid;grid-template-columns:1fr 1fr;gap:12px;align-items:start}\
         figure{margin:0}figcaption{color:#999;font-size:12px;margin-bottom:4px}\
         img{max-width:100%;border:1px solid #444;background:#000}\
         .missing{color:#e5484d}</style><h1>Vector renders next to the reference</h1>",
    );
    let mut paired = HashSet::new();
    let mut n = 0;
    for line in map.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 2 {
            continue;
        }
        let render = visual.join(f[0]);
        let refimg = reference().join(f[1]);
        paired.insert(f[0].to_string());
        let title = f.get(2).copied().unwrap_or(f[0]);
        let side = |label: &str, p: &Path| {
            if p.exists() {
                format!("<figure><figcaption>{label}</figcaption><img src=\"{}\"></figure>", esc(&file_url(p)))
            } else {
                format!("<figure><figcaption class=missing>{label}: missing {}</figcaption></figure>", esc(&p.display().to_string()))
            }
        };
        write!(html, "<h2>{}</h2><div class=pair>{}{}</div>", esc(title), side("Vector", &render), side("Reference", &refimg))?;
        n += 1;
    }
    let mut extra: Vec<_> = std::fs::read_dir(&visual).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "png")).collect()).unwrap_or_default();
    extra.sort();
    for p in extra {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if !paired.contains(&name) {
            write!(html, "<h2>{} <span class=missing>(no reference pair)</span></h2><div class=pair><figure><img src=\"{}\"></figure></div>", esc(&name), esc(&file_url(&p)))?;
        }
    }
    std::fs::create_dir_all(&visual)?;
    let out = visual.join("compare.html");
    std::fs::write(&out, html)?;
    println!("wrote {} ({n} pairs)", out.display());
    Ok(())
}
