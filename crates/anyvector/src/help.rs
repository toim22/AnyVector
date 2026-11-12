//! Help ▸ Product Help, Quick Start Guide, User Guide and Updates: HTML pages that open in the
//! default browser. The User Guide and the shortcut list are built from AnyVector's own interface
//! data (the menus, tools, dockers and keys the program has), so they always match the program.

use crate::ui::{self, Kind, NO_SUB};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

/// The release notes of this version (also shown by Help ▸ Updates).
const RELEASE_NOTES: &str = include_str!("../../../docs/RELEASE-NOTES-0.4.0.md");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    /// Help ▸ Product Help: the start page with the other pages.
    Home,
    QuickStart,
    UserGuide,
    Shortcuts,
    /// Help ▸ Updates: the version and its release notes.
    ReleaseNotes,
}

impl Page {
    fn file(self) -> &'static str {
        match self {
            Page::Home => "index.html",
            Page::QuickStart => "quick-start.html",
            Page::UserGuide => "user-guide.html",
            Page::Shortcuts => "shortcuts.html",
            Page::ReleaseNotes => "release-notes.html",
        }
    }
}

/// Where the help pages are written.
pub fn dir() -> PathBuf {
    crate::settings::config_dir().join("help")
}

/// Writes the help pages and opens `page` in the default browser.
pub fn open(page: Page) -> Result<PathBuf, String> {
    let d = dir();
    write_all(&d)?;
    let p = d.join(page.file());
    if crate::bugreport::open_in_browser(&p.to_string_lossy()) {
        Ok(p)
    } else {
        Err(format!("The help page could not be opened: {}", p.display()))
    }
}

/// Writes every help page into `d`.
pub fn write_all(d: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    for (page, html) in [
        (Page::Home, home()),
        (Page::QuickStart, quick_start()),
        (Page::UserGuide, user_guide()),
        (Page::Shortcuts, shortcuts()),
        (Page::ReleaseNotes, release_notes()),
    ] {
        std::fs::write(d.join(page.file()), html).map_err(|e| format!("{}: {e}", page.file()))?;
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------
// page frame
// ------------------------------------------------------------------------------------------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const STYLE: &str = r#"
:root { color-scheme: dark; }
* { box-sizing: border-box; }
body { margin: 0; background: #262626; color: #dcdcdc; font: 15px/1.55 "Segoe UI", system-ui, sans-serif; }
header { background: #1e1e1e; border-bottom: 1px solid #3a3a3a; padding: 14px 24px; display: flex; gap: 20px; align-items: center; flex-wrap: wrap; }
header b { font-size: 17px; color: #fff; margin-right: 12px; }
header a { color: #c8c8c8; text-decoration: none; }
header a:hover, header a.on { color: #fff; text-decoration: underline; }
main { max-width: 980px; margin: 0 auto; padding: 8px 24px 64px; }
h1 { font-size: 28px; font-weight: 600; color: #fff; margin: 28px 0 8px; }
h2 { font-size: 21px; font-weight: 600; color: #fff; margin: 36px 0 8px; border-bottom: 1px solid #3a3a3a; padding-bottom: 4px; }
h3 { font-size: 17px; font-weight: 600; color: #fff; margin: 24px 0 6px; }
a { color: #6cb4ff; }
p, li { max-width: 72ch; }
kbd { font: 12px "Segoe UI", sans-serif; background: #3a3a3a; border: 1px solid #555; border-radius: 3px; padding: 0 5px; color: #eee; white-space: nowrap; }
ul.menu, ul.menu ul { list-style: none; padding-left: 0; margin: 0; }
ul.menu ul { padding-left: 26px; border-left: 1px solid #3a3a3a; margin-left: 11px; }
ul.menu li { padding: 2px 0; }
.row { display: flex; align-items: center; gap: 8px; }
.tip { color: #9a9a9a; font-size: 13px; margin-left: 32px; }
svg.i { width: 24px; height: 24px; flex: none; }
span.noi { width: 24px; flex: none; }
table { border-collapse: collapse; width: 100%; margin: 8px 0; }
td, th { text-align: left; padding: 5px 8px; border-bottom: 1px solid #353535; vertical-align: top; }
th { color: #fff; font-weight: 600; }
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 14px; margin: 20px 0; }
.card { background: #2f2f2f; border: 1px solid #3c3c3c; border-radius: 6px; padding: 14px 16px; text-decoration: none; color: #dcdcdc; display: block; }
.card:hover { border-color: #6cb4ff; }
.card b { color: #fff; display: block; font-size: 16px; margin-bottom: 4px; }
input#find { width: 100%; max-width: 420px; padding: 7px 10px; background: #1e1e1e; border: 1px solid #555; color: #eee; border-radius: 4px; font: inherit; }
.muted { color: #9a9a9a; }
"#;

fn frame(title: &str, on: Page, body: &str, icons: &BTreeMap<&'static str, String>) -> String {
    let mut nav = String::new();
    for (p, label) in [(Page::Home, "Help"), (Page::QuickStart, "Quick Start Guide"), (Page::UserGuide, "User Guide"), (Page::Shortcuts, "Keyboard Shortcuts"), (Page::ReleaseNotes, "What's New in This Version")] {
        let _ = write!(nav, r#"<a href="{}"{}>{label}</a>"#, p.file(), if p == on { r#" class="on""# } else { "" });
    }
    let mut defs = String::new();
    if !icons.is_empty() {
        defs.push_str(r#"<svg width="0" height="0" style="position:absolute">"#);
        for (name, body) in icons {
            let _ = write!(defs, r##"<symbol id="i-{name}" viewBox="0 0 24 24"><g fill="none" stroke="#c4c4c4" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">{body}</g></symbol>"##);
        }
        defs.push_str("</svg>");
    }
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{} - AnyVector {}</title><style>{STYLE}</style></head>\n<body>{defs}<header><b>AnyVector {}</b>{nav}</header><main>\n{body}\n</main></body></html>\n",
        esc(title),
        vector_core::APP_VERSION,
        vector_core::APP_VERSION
    )
}

/// An icon of the interface, by name (kept once per page).
fn icon(icons: &mut BTreeMap<&'static str, String>, name: &'static str) -> String {
    if name.is_empty() {
        return r#"<span class="noi"></span>"#.into();
    }
    match crate::icons::body(name) {
        Some(b) => {
            icons.entry(name).or_insert(b);
            format!(r##"<svg class="i"><use href="#i-{name}"/></svg>"##)
        }
        None => r#"<span class="noi"></span>"#.into(),
    }
}

fn keys(k: &[&str]) -> String {
    k.iter().map(|k| format!("<kbd>{}</kbd>", esc(k))).collect::<Vec<_>>().join(" ")
}

// ------------------------------------------------------------------------------------------
// the menus, as the program has them
// ------------------------------------------------------------------------------------------

/// The items of a menu that show: separators, run-time lists and left-out items skipped.
fn menu_items(bar: u16, out: &mut Vec<(&'static ui::Item, usize)>, depth: usize) {
    let Some(b) = ui::bar(bar) else { return };
    for it in b.items {
        if it.on == 0 || it.flags & ui::EXCLUDED != 0 {
            continue;
        }
        match it.kind {
            Kind::Separator | Kind::GroupBegin | Kind::GroupEnd | Kind::Spacer | Kind::Anchors => {}
            // a placeholder that brings in the items of another bar
            Kind::Placeholder => {
                if it.sub != NO_SUB && depth < 8 {
                    menu_items(it.sub, out, depth);
                }
            }
            _ => {
                if !ui::plain(ui::label(it)).trim().is_empty() {
                    out.push((it, depth));
                }
            }
        }
    }
}

fn menu_html(bar: u16, depth: usize, path: &str, html: &mut String, icons: &mut BTreeMap<&'static str, String>, keylist: &mut Vec<(String, &'static [&'static str])>) {
    let mut items = vec![];
    menu_items(bar, &mut items, depth);
    html.push_str("<ul class=\"menu\">");
    for (it, _) in items {
        let name = ui::plain(ui::label(it));
        let name = name.trim_end_matches("...").trim();
        let full = format!("{path} ▸ {name}");
        let _ = write!(html, "<li><div class=\"row\">{}<span>{}</span>", icon(icons, it.icon), esc(name));
        if !it.keys.is_empty() {
            let _ = write!(html, " {}", keys(it.keys));
            keylist.push((full.clone(), it.keys));
        }
        html.push_str("</div>");
        let tip = it.tip.trim();
        if !tip.is_empty() && !tip.eq_ignore_ascii_case(name) && !ui::plain(tip).eq_ignore_ascii_case(name) {
            let _ = write!(html, "<div class=\"tip\">{}</div>", esc(tip));
        }
        if matches!(it.kind, Kind::Flyout | Kind::ButtonFlyout) && it.sub != NO_SUB && depth < 6 {
            menu_html(it.sub, depth + 1, &full, html, icons, keylist);
        }
        html.push_str("</li>");
    }
    html.push_str("</ul>");
}

/// The menus of the menu bar (with a document open), and every shortcut found on the way.
fn menus(icons: &mut BTreeMap<&'static str, String>) -> (String, Vec<(String, &'static [&'static str])>) {
    let mut html = String::new();
    let mut keylist = vec![];
    let Some(bar) = ui::bar(ui::menu_bar_index(true)) else { return (html, keylist) };
    for it in bar.items {
        if it.sub == NO_SUB || it.on == 0 || !matches!(it.kind, Kind::Flyout | Kind::ButtonFlyout) {
            continue;
        }
        let name = ui::plain(it.text);
        let _ = write!(html, "<h3 id=\"menu-{}\">{}</h3>", esc(&name.to_lowercase()), esc(&name));
        menu_html(it.sub, 0, &name, &mut html, icons, &mut keylist);
    }
    (html, keylist)
}

fn menu_names() -> Vec<String> {
    let Some(bar) = ui::bar(ui::menu_bar_index(true)) else { return vec![] };
    bar.items.iter().filter(|it| it.sub != NO_SUB && it.on != 0 && matches!(it.kind, Kind::Flyout | Kind::ButtonFlyout)).map(|it| ui::plain(it.text)).collect()
}

// ------------------------------------------------------------------------------------------
// the pages
// ------------------------------------------------------------------------------------------

fn home() -> String {
    let icons = BTreeMap::new();
    let mut b = String::new();
    let _ = write!(
        b,
        r#"<h1>AnyVector Help</h1>
<p>AnyVector is a vector drawing program: page layout, illustration, text and bitmaps, with the menus, tools and
dockers organized as in CorelDRAW. These pages are part of AnyVector {v} and work without an internet connection.</p>
<div class="cards">
<a class="card" href="quick-start.html"><b>Quick Start Guide</b>Your first drawing in ten steps: a document, shapes, color, text, and saving and exporting.</a>
<a class="card" href="user-guide.html"><b>User Guide</b>The workspace, every tool of the toolbox, every menu command and every docker.</a>
<a class="card" href="shortcuts.html"><b>Keyboard Shortcuts</b>All the keys of the menus and tools in one list.</a>
<a class="card" href="release-notes.html"><b>What's New in This Version</b>The release notes of AnyVector {v}.</a>
</div>
<h2>Getting help in the program</h2>
<ul>
<li>Point at a button or a tool to see its name and what it does. The status bar shows hints for the active tool.</li>
<li>The <b>Help ▸ Welcome Screen</b> tab has the getting-started links and the workspaces.</li>
<li><b>Help ▸ Report a Bug</b> sends a problem to the AnyVector team; <b>Help ▸ Submit Ideas and Feedback</b> suggests a feature.</li>
<li><b>Help ▸ About AnyVector</b> shows the version, the build (Source ID) and System Info.</li>
</ul>"#,
        v = vector_core::APP_VERSION
    );
    frame("AnyVector Help", Page::Home, &b, &icons)
}

fn quick_start() -> String {
    let icons = BTreeMap::new();
    let b = r#"<h1>Quick Start Guide</h1>
<p class="muted">Ten steps from an empty window to a finished, exported drawing.</p>
<h2>1. Create a document</h2>
<p>Choose <b>File ▸ New</b> (<kbd>Ctrl+N</kbd>). In the Create a New Document dialog type a name, pick a page size or
a preset, the orientation and the color mode, then click <b>OK</b>. <b>File ▸ New From Template</b> starts from a
template instead.</p>
<h2>2. Know the workspace</h2>
<ul>
<li><b>Menu bar</b> and <b>Standard toolbar</b> at the top; the <b>property bar</b> under them shows the options of
the active tool or of the selected objects.</li>
<li><b>Toolbox</b> on the left. A small triangle on a tool means a flyout with more tools: hold the button or click
the triangle.</li>
<li><b>Drawing window</b> in the middle with the page, the rulers and the page tabs at the bottom.</li>
<li><b>Color palette</b> on the right, <b>dockers</b> (panels such as Properties and Objects) next to it, and the
<b>status bar</b> at the bottom with the size, fill and outline of the selection.</li>
</ul>
<h2>3. Draw shapes</h2>
<p>Pick the <b>Rectangle</b> tool (<kbd>F6</kbd>) and drag on the page. Hold <kbd>Ctrl</kbd> for a square and
<kbd>Shift</kbd> to draw from the center. The <b>Ellipse</b> (<kbd>F7</kbd>), <b>Polygon</b> (<kbd>Y</kbd>) and
<b>Star</b> tools work the same way.</p>
<h2>4. Draw lines and curves</h2>
<p>The <b>Freehand</b> tool (<kbd>F5</kbd>) follows the mouse. The <b>Pen</b> and <b>Bézier</b> tools add a node
at each click; drag to bend the segment, double-click to finish.</p>
<h2>5. Select, move and transform</h2>
<p>The <b>Pick</b> tool selects: click an object, drag a marquee around several, or <kbd>Shift</kbd>+click to add.
Drag to move, drag a corner handle to resize, and click the selection again for the rotate and skew handles.
<kbd>Space</kbd> switches to the Pick tool and back. Exact values go in the property bar or in the
<b>Transform</b> docker (<kbd>Alt+F7</kbd>).</p>
<h2>6. Color</h2>
<p>With objects selected, click a color in the palette to fill them and right-click a color to set the outline.
Click the crossed swatch for no fill. The <b>Properties</b> docker (<kbd>Alt+Enter</kbd>) has every fill and outline
option: gradients, patterns, outline width, style and corners.</p>
<h2>7. Add text</h2>
<p>Pick the <b>Text</b> tool (<kbd>F8</kbd>). Click and type for artistic text, or drag a frame first for paragraph
text. Choose the font and size in the property bar. <b>Text ▸ Fit Text to Path</b> sets text along a curve.</p>
<h2>8. Arrange objects</h2>
<p>Use <b>Object ▸ Order</b> to bring objects forward or back, <b>Object ▸ Align and Distribute</b> to line them up,
<b>Object ▸ Group</b> (<kbd>Ctrl+G</kbd>) to keep them together, and the <b>Objects</b> docker for layers and pages.</p>
<h2>9. Save</h2>
<p><b>File ▸ Save</b> (<kbd>Ctrl+S</kbd>) keeps the drawing as a .cdr file. <b>Edit ▸ Undo</b> (<kbd>Ctrl+Z</kbd>)
steps back; the <b>History</b> docker lists every step.</p>
<h2>10. Export and print</h2>
<p><b>File ▸ Export</b> (<kbd>Ctrl+E</kbd>) writes PDF, SVG, PNG, JPEG and other formats; <b>File ▸ Export For</b>
has ready-made settings for the web and for print. <b>File ▸ Print</b> (<kbd>Ctrl+P</kbd>) prints with a preview.</p>
<p>Next: the <a href="user-guide.html">User Guide</a> describes every tool, menu command and docker.</p>"#;
    frame("Quick Start Guide", Page::QuickStart, b, &icons)
}

fn user_guide() -> String {
    let mut icons = BTreeMap::new();
    let mut b = String::new();
    b.push_str("<h1>User Guide</h1>\n<p class=\"muted\">Built from the program itself: these are the tools, menus and dockers of this version of AnyVector, in the order the program shows them.</p>");
    b.push_str(r#"<p><input id="find" type="search" placeholder="Find a command or tool" oninput="find(this.value)"></p>"#);
    // contents
    b.push_str("<p><a href=\"#workspace\">The workspace</a> · <a href=\"#tools\">Tools</a> · <a href=\"#menus\">Menus</a> (");
    let names = menu_names();
    b.push_str(&names.iter().map(|n| format!("<a href=\"#menu-{}\">{}</a>", esc(&n.to_lowercase()), esc(n))).collect::<Vec<_>>().join(", "));
    b.push_str(") · <a href=\"#dockers\">Dockers</a></p>");
    b.push_str(
        r#"<h2 id="workspace">The workspace</h2>
<ul>
<li><b>Menu bar</b>: the commands, grouped in the menus listed below. Without an open document it shows File, Tools,
Window and Help only.</li>
<li><b>Standard toolbar</b>: New, Open, Save, Print, Cut, Copy, Paste, Undo, Redo, Import, Export, the zoom level and
the snapping and Options buttons.</li>
<li><b>Property bar</b>: the settings of the active tool, or of the selection when the Pick tool is active.</li>
<li><b>Toolbox</b>: the drawing and editing tools. Tools that share a slot open from its flyout.</li>
<li><b>Drawing window</b>: the page, the rulers, the document navigator with the page tabs, and the scroll bars.</li>
<li><b>Color palette</b> and <b>document palette</b>: click a color to fill the selection, right-click it for the
outline, drag it onto an object.</li>
<li><b>Dockers</b>: panels that stay open next to the drawing (Window ▸ Dockers).</li>
<li><b>Status bar</b>: the selection, its fill and outline, and the cursor position.</li>
<li><b>Workspaces</b> (Window ▸ Workspace) keep a layout of toolbars and dockers; Tools ▸ Options ▸ Workspaces
creates, exports and imports them.</li>
</ul>"#,
    );
    // tools
    b.push_str("<h2 id=\"tools\">Tools</h2><table><tr><th></th><th>Tool</th><th>Shortcut</th><th>How to use it</th></tr>");
    for t in crate::tools::TOOLS {
        let _ = write!(b, "<tr class=\"f\"><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", icon(&mut icons, t.icon), esc(t.name), if t.shortcut.is_empty() { String::new() } else { keys(&[t.shortcut]) }, esc(t.hint));
    }
    b.push_str("</table>");
    // menus
    b.push_str("<h2 id=\"menus\">Menus</h2>");
    let (m, _) = menus(&mut icons);
    b.push_str(&m);
    // dockers
    b.push_str("<h2 id=\"dockers\">Dockers</h2><p>Open them from Window ▸ Dockers. Each docker stays open next to the drawing; several dockers share the area as tabs.</p><table><tr><th></th><th>Docker</th><th>Shortcut</th></tr>");
    for d in crate::dockers::ALL {
        if *d == crate::dockers::DockerId::Learn {
            continue;
        }
        let k = crate::dockers::shortcut(*d).map(|k| keys(&[k])).unwrap_or_default();
        let _ = write!(b, "<tr class=\"f\"><td>{}</td><td>{}</td><td>{k}</td></tr>", icon(&mut icons, crate::dockers::icon(*d)), esc(&crate::dockers::title(*d)));
    }
    b.push_str("</table>");
    b.push_str(
        r#"<script>
function find(q) {
  q = q.trim().toLowerCase();
  for (const el of document.querySelectorAll('ul.menu li, tr.f')) {
    el.style.display = !q || el.textContent.toLowerCase().includes(q) ? '' : 'none';
  }
}
</script>"#,
    );
    frame("User Guide", Page::UserGuide, &b, &icons)
}

fn shortcuts() -> String {
    let mut icons = BTreeMap::new();
    let (_, mut list) = menus(&mut icons);
    let icons = BTreeMap::new();
    let mut b = String::from("<h1>Keyboard Shortcuts</h1><p class=\"muted\">Every key of this version of AnyVector. Tools ▸ Options ▸ Customization ▸ Commands changes them.</p>");
    b.push_str("<h2>Tools</h2><table><tr><th>Tool</th><th>Keys</th></tr>");
    for t in crate::tools::TOOLS.iter().filter(|t| !t.shortcut.is_empty()) {
        let _ = write!(b, "<tr><td>{}</td><td>{}</td></tr>", esc(t.name), keys(&[t.shortcut]));
    }
    b.push_str("</table><h2>Dockers</h2><table><tr><th>Docker</th><th>Keys</th></tr>");
    for d in crate::dockers::ALL {
        if let Some(k) = crate::dockers::shortcut(*d) {
            let _ = write!(b, "<tr><td>{}</td><td>{}</td></tr>", esc(&crate::dockers::title(*d)), keys(&[k]));
        }
    }
    b.push_str("</table><h2>Menu commands</h2><table><tr><th>Command</th><th>Keys</th></tr>");
    let mut seen = std::collections::HashSet::new();
    list.retain(|(p, _)| seen.insert(p.clone()));
    for (path, k) in list {
        let _ = write!(b, "<tr><td>{}</td><td>{}</td></tr>", esc(&path), keys(k));
    }
    b.push_str("</table>");
    frame("Keyboard Shortcuts", Page::Shortcuts, &b, &icons)
}

fn release_notes() -> String {
    let icons = BTreeMap::new();
    let b = markdown(RELEASE_NOTES);
    frame("What's New in This Version", Page::ReleaseNotes, &b, &icons)
}

/// The little Markdown the release notes use: headings, bullets (nested by two spaces),
/// paragraphs, **bold** and `code`.
fn markdown(md: &str) -> String {
    fn inline(s: &str) -> String {
        let s = esc(s);
        let mut out = String::new();
        let mut bold = false;
        let mut code = false;
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '*' && chars.peek() == Some(&'*') && !code {
                chars.next();
                out.push_str(if bold { "</b>" } else { "<b>" });
                bold = !bold;
            } else if c == '`' {
                out.push_str(if code { "</code>" } else { "<code>" });
                code = !code;
            } else {
                out.push(c);
            }
        }
        if bold {
            out.push_str("</b>");
        }
        if code {
            out.push_str("</code>");
        }
        out
    }
    let mut out = String::new();
    let mut depth: Vec<usize> = vec![];
    let mut para = String::new();
    let flush = |para: &mut String, out: &mut String| {
        if !para.trim().is_empty() {
            let _ = write!(out, "<p>{}</p>\n", inline(para.trim()));
        }
        para.clear();
    };
    for line in md.lines() {
        let indent = line.len() - line.trim_start().len();
        let t = line.trim();
        let bullet = t.strip_prefix("- ").or_else(|| t.strip_prefix("* "));
        if let Some(item) = bullet {
            flush(&mut para, &mut out);
            while depth.last().is_some_and(|d| *d > indent) {
                out.push_str("</li></ul>");
                depth.pop();
            }
            if depth.last() == Some(&indent) {
                out.push_str("</li>");
            } else {
                out.push_str("<ul>");
                depth.push(indent);
            }
            let _ = write!(out, "<li>{}", inline(item));
            continue;
        }
        if !depth.is_empty() && !t.is_empty() && indent > 0 {
            // a bullet's continuation line
            let _ = write!(out, " {}", inline(t));
            continue;
        }
        while depth.pop().is_some() {
            out.push_str("</li></ul>");
        }
        if t.is_empty() {
            flush(&mut para, &mut out);
        } else if let Some(h) = t.strip_prefix('#') {
            flush(&mut para, &mut out);
            let level = 1 + h.chars().take_while(|c| *c == '#').count();
            let _ = write!(out, "<h{level}>{}</h{level}>\n", inline(h.trim_start_matches('#').trim()));
        } else {
            para.push(' ');
            para.push_str(t);
        }
    }
    flush(&mut para, &mut out);
    while depth.pop().is_some() {
        out.push_str("</li></ul>");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_pages_are_written() {
        let d = std::env::temp_dir().join(format!("vector_help_{}", std::process::id()));
        write_all(&d).unwrap();
        let guide = std::fs::read_to_string(d.join("user-guide.html")).unwrap();
        // the menus and tools of the program, with their keys
        for s in ["<h3 id=\"menu-file\">File</h3>", "Rectangle", "<kbd>F6</kbd>", "Palette Editor", "Properties"] {
            assert!(guide.contains(s), "user guide lacks {s}");
        }
        assert!(!guide.contains("Message Settings"));
        let keys = std::fs::read_to_string(d.join("shortcuts.html")).unwrap();
        assert!(keys.contains("File ▸ New") && keys.contains("<kbd>Ctrl+N</kbd>"));
        let notes = std::fs::read_to_string(d.join("release-notes.html")).unwrap();
        assert!(notes.contains("<h1>") && notes.contains("<li>"));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Writes the pages to `target/help` to look at them (`cargo test -p anyvector write_help_preview -- --ignored`).
    #[test]
    #[ignore]
    fn write_help_preview() {
        write_all(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/help")).unwrap();
    }

    #[test]
    fn markdown_lists_nest() {
        let h = markdown("# T\n\nSome **bold** `code`.\n\n- a\n  - b\n- c\n\nEnd");
        assert_eq!(h, "<h1>T</h1>\n<p>Some <b>bold</b> <code>code</code>.</p>\n<ul><li>a<ul><li>b</li></ul></li><li>c</li></ul><p>End</p>\n");
    }
}
