//! `docs/COVERAGE.md`: every menu item, dialog, docker, tool, toolbar and context menu of the
//! CorelDRAW reference, with AnyVector's status for each one.
//!
//! Status comes from two places:
//! - `docs/coverage-status.tsv` (section, key, status, note), kept by hand as batches land. It
//!   always wins.
//! - Otherwise, a name that already appears as a string in AnyVector's UI code counts as `partial`
//!   (it exists, but has not been checked against the reference yet), and anything else as `todo`.

use crate::{reference, root, Result};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Status {
    Done,
    Partial,
    Todo,
    Omitted,
}

impl Status {
    fn parse(s: &str) -> Option<Status> {
        match s.trim() {
            "done" => Some(Status::Done),
            "partial" => Some(Status::Partial),
            "todo" => Some(Status::Todo),
            "omitted" => Some(Status::Omitted),
            _ => None,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Status::Done => "✅ done",
            Status::Partial => "🟡 partial",
            Status::Todo => "⬜ todo",
            Status::Omitted => "➖ omitted",
        }
    }
    const ALL: [Status; 4] = [Status::Done, Status::Partial, Status::Todo, Status::Omitted];
}

struct Row {
    cols: Vec<String>,
    status: Status,
    note: String,
    reference: String,
}

struct Section {
    title: &'static str,
    headers: Vec<&'static str>,
    groups: Vec<(String, Vec<Row>)>,
}

/// A `;`-separated CSV line, with "quoted" fields.
fn split_csv(line: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ';' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

fn read_csv(path: &Path) -> Result<Vec<HashMap<String, String>>> {
    let text = std::fs::read_to_string(path)?;
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    let head = split_csv(lines.next().ok_or("empty csv")?);
    Ok(lines.filter(|l| !l.trim().is_empty()).map(|l| head.iter().cloned().zip(split_csv(l)).collect()).collect())
}

/// Normalized UI text: no mnemonic `&`, no trailing `...`, lowercase.
fn norm(s: &str) -> String {
    s.replace('&', "").trim().trim_end_matches("...").trim_end_matches('…').trim().to_lowercase()
}

/// Every string literal in AnyVector's app code, normalized (the names the UI already shows).
fn ui_strings() -> Result<HashSet<String>> {
    let mut set = HashSet::new();
    let mut stack = vec![root().join("crates/anyvector/src")];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(dir)? {
            let p = e?.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let src = std::fs::read_to_string(&p)?;
                let mut rest = src.as_str();
                while let Some(i) = rest.find('"') {
                    let after = &rest[i + 1..];
                    let mut end = None;
                    let mut esc = false;
                    for (j, c) in after.char_indices() {
                        match c {
                            '\\' if !esc => esc = true,
                            '"' if !esc => {
                                end = Some(j);
                                break;
                            }
                            _ => esc = false,
                        }
                    }
                    let Some(j) = end else { break };
                    let lit = &after[..j];
                    if !lit.is_empty() && lit.len() < 120 {
                        set.insert(norm(lit));
                    }
                    rest = &after[j + 1..];
                }
            }
        }
    }
    Ok(set)
}

/// The hand-kept statuses: `section \t key \t status \t note`.
fn overrides() -> Result<HashMap<(String, String), (Status, String)>> {
    let mut map = HashMap::new();
    let path = root().join("docs/coverage-status.tsv");
    let Ok(text) = std::fs::read_to_string(&path) else { return Ok(map) };
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 3 {
            return Err(format!("{}:{}: expected section, key, status[, note]", path.display(), n + 1).into());
        }
        let st = Status::parse(f[2]).ok_or_else(|| format!("{}:{}: unknown status {:?}", path.display(), n + 1, f[2]))?;
        map.insert((f[0].to_string(), f[1].to_string()), (st, f.get(3).unwrap_or(&"").to_string()));
    }
    Ok(map)
}

/// Files directly in `dir` with the given extension, sorted by name.
fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && p.extension().is_some_and(|x| x == ext)).collect()).unwrap_or_default();
    v.sort();
    v
}

fn dirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
    v.sort();
    v
}

fn count_png(dir: &Path) -> usize {
    let mut n = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for p in std::fs::read_dir(&d).map(|rd| rd.flatten().map(|e| e.path()).collect::<Vec<_>>()).unwrap_or_default() {
            if p.is_dir() {
                if !p.ends_with("tela-inteira") {
                    stack.push(p);
                }
            } else if p.extension().is_some_and(|x| x == "png") {
                n += 1;
            }
        }
    }
    n
}

fn stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

/// "012 Help ▸ About CorelDRAW" → "Help ▸ About CorelDRAW".
fn strip_number(s: &str) -> String {
    match s.split_once(' ') {
        Some((n, rest)) if n.chars().all(|c| c.is_ascii_digit()) => rest.to_string(),
        _ => s.to_string(),
    }
}

fn rel(p: &Path) -> String {
    p.strip_prefix(reference()).unwrap_or(p).display().to_string()
}

/// Menu items that are out of scope: the Purchase menu, the hidden Debug menu, and the Corel
/// online services the project decided to leave out.
fn omitted_by_decision(path: &str) -> Option<&'static str> {
    if path == "Purchase" || path.starts_with("Purchase >") {
        return Some("Purchase menu removed");
    }
    if path.starts_with('<') {
        return Some("hidden Debug menu");
    }
    let corel = [
        ("File > Open from Cloud", "Corel Cloud"),
        ("File > Save to Cloud", "Corel Cloud"),
        ("Window > Dockers > Learn", "Corel Learn content"),
        ("Help > Learn", "Corel Learn content"),
        ("Help > Video Tutorials", "Corel Learn content"),
        ("Help > Corel Support", "Corel Support"),
        ("Help > CorelDRAW Community", "Corel online community"),
        ("Help > Share Your Experience", "Share Your Experience"),
        ("Help > Message Settings", "Corel in-product messages"),
        ("Help > Sign out of Cloud", "Corel Cloud"),
        ("Help > Refresh your account", "Corel account"),
        ("Help > Refresh your Account", "Corel account"),
        ("Help > Account Settings", "Corel account"),
        ("Help > Restore Purchases", "Corel account"),
        ("Help > <placeHolder >", "Corel account placeholder"),
    ];
    corel.iter().find(|(p, _)| path == *p || path.starts_with(&format!("{p}..."))).map(|(_, why)| *why)
}

struct Ctx {
    ui: HashSet<String>,
    over: HashMap<(String, String), (Status, String)>,
    used: HashSet<(String, String)>,
}

impl Ctx {
    /// Status for `key` in `section`: the hand-kept one, else partial if `name` is in the UI.
    fn status(&mut self, section: &str, key: &str, name: &str) -> (Status, String) {
        let k = (section.to_string(), key.to_string());
        if let Some(v) = self.over.get(&k) {
            self.used.insert(k);
            return v.clone();
        }
        if self.ui.contains(&norm(name)) {
            (Status::Partial, "exists in AnyVector; not yet checked against the reference".into())
        } else {
            (Status::Todo, String::new())
        }
    }
}

fn menus(cx: &mut Ctx) -> Result<Section> {
    let rows = read_csv(&reference().join("90-relatorios/cobertura_final.csv"))?;
    let mut groups: Vec<(String, Vec<Row>)> = vec![];
    for r in rows {
        let path = r.get("caminho").cloned().unwrap_or_default();
        let top = path.split(" > ").next().unwrap_or("").to_string();
        let (status, note) = if let Some(why) = omitted_by_decision(&path) {
            (Status::Omitted, why.to_string())
        } else {
            let last = path.rsplit(" > ").next().unwrap_or(&path).to_string();
            cx.status("menu", &path, &last)
        };
        let img = r.get("imagem").cloned().unwrap_or_default();
        let inner = r.get("prints_dentro").cloned().unwrap_or_default();
        let kind = match r.get("tipo").map(String::as_str).unwrap_or_default() {
            "abre tela" => "opens a window",
            "comando" => "command",
            "fora do escopo" => "out of scope",
            "menu" => "menu",
            "submenu" => "submenu",
            other => other,
        }
        .to_string();
        let row = Row { cols: vec![path, kind, inner], status, note, reference: img };
        let group = if top.starts_with('<') { "Debug (hidden)".to_string() } else { top };
        match groups.iter_mut().find(|(g, _)| *g == group) {
            Some((_, v)) => v.push(row),
            None => groups.push((group, vec![row])),
        }
    }
    Ok(Section { title: "Menus", headers: vec!["Menu path", "Kind", "Inner prints"], groups })
}

/// Dialogs (and commands that change the main window) from `03-telas`, grouped by menu.
fn dialogs(cx: &mut Ctx) -> Result<(Section, Section)> {
    let base = reference().join("03-telas");
    let mut by_menu: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    let mut effects: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    let mut seen = HashSet::new();
    for ctx_dir in dirs(&base) {
        let context = stem(&ctx_dir);
        for menu_dir in dirs(&ctx_dir) {
            let menu = strip_number(&stem(&menu_dir));
            for f in files(&menu_dir, "png") {
                let name = strip_number(&stem(&f));
                let inner = count_png(&menu_dir.join(stem(&f)));
                let clean = name.replace(" (abre no docker Properties)", "").replace(" (sem janela, tela principal)", "");
                let path = clean.replace(" ▸ ", " > ");
                let last = path.rsplit(" > ").next().unwrap_or(&path).to_string();
                if !seen.insert(path.clone()) {
                    continue;
                }
                if name.ends_with("(abre no docker Properties)") {
                    let (status, note) = cx.status("effect", &path, &last);
                    let group = path.split(" > ").take(2).collect::<Vec<_>>().join(" > ");
                    effects.entry(group).or_default().push(Row { cols: vec![path, context.clone()], status, note, reference: rel(&f) });
                    continue;
                }
                let kind = if name.ends_with("(sem janela, tela principal)") { "mode (no window)" } else { "dialog" };
                let (status, note) = cx.status("dialog", &path, &last);
                by_menu.entry(menu.clone()).or_default().push(Row { cols: vec![path, kind.into(), context.clone(), inner.to_string()], status, note, reference: rel(&f) });
            }
        }
    }
    let order = ["File", "Edit", "View", "Layout", "Object", "Effects", "Bitmaps", "Text", "Table", "Tools", "Window", "Help"];
    let mut groups: Vec<(String, Vec<Row>)> = by_menu.into_iter().collect();
    groups.sort_by_key(|(m, _)| order.iter().position(|o| o == m).unwrap_or(99));
    Ok((
        Section { title: "Dialogs and screens", headers: vec!["Opened from", "Kind", "Context", "Inner prints"], groups },
        Section { title: "Bitmap effects (Properties docker)", headers: vec!["Effect", "Context"], groups: effects.into_iter().collect() },
    ))
}

fn dockers(cx: &mut Ctx) -> Result<Section> {
    let base = reference().join("04-dockers");
    let mut rows = vec![];
    let inside: Vec<PathBuf> = dirs(&base.join("dentro"));
    for f in files(&base.join("retangulo"), "png") {
        let s = stem(&f);
        if s.contains("rolagem") {
            continue;
        }
        let name = strip_number(&s).replace(" ▸ ", " > ");
        let last = name.rsplit(" > ").next().unwrap_or(&name).to_string();
        let states = inside.iter().find(|d| strip_number(&stem(d)).replace(" ▸ ", " > ") == name).map(|d| count_png(d)).unwrap_or(0);
        let (status, note) = if name == "Learn" { (Status::Omitted, "Corel Learn content".to_string()) } else { cx.status("docker", &name, &last) };
        rows.push(Row { cols: vec![name, states.to_string()], status, note, reference: rel(&f) });
    }
    Ok(Section { title: "Dockers", headers: vec!["Docker", "Inner states"], groups: vec![(String::new(), rows)] })
}

fn tools(cx: &mut Ctx) -> Result<Section> {
    let base = reference().join("05-ferramentas");
    let mut groups = vec![];
    let mut bars = vec![];
    for f in files(&base.join("barra-de-propriedades"), "png") {
        let s = strip_number(&stem(&f));
        let (tool, context) = match s.rsplit_once(" (") {
            Some((t, c)) => (t.to_string(), c.trim_end_matches(')').to_string()),
            None => (s.clone(), String::new()),
        };
        let (status, note) = cx.status("tool", &tool, &tool);
        bars.push(Row { cols: vec![tool, context], status, note, reference: rel(&f) });
    }
    groups.push(("Property bar of each tool".to_string(), bars));
    let mut flyouts = vec![];
    for f in files(&base.join("flyouts"), "png") {
        let s = strip_number(&stem(&f));
        let key = format!("flyout: {s}");
        let (status, note) = cx.status("tool", &key, "\u{0}");
        flyouts.push(Row { cols: vec![s, String::new()], status, note, reference: rel(&f) });
    }
    groups.push(("Toolbox and flyouts".to_string(), flyouts));
    let mut pick = vec![];
    for f in files(&base.join("barra-pick-por-contexto"), "png") {
        let s = strip_number(&stem(&f));
        let key = format!("pick bar: {s}");
        let (status, note) = cx.status("tool", &key, "\u{0}");
        pick.push(Row { cols: vec!["Pick".into(), s], status, note, reference: rel(&f) });
    }
    groups.push(("Pick tool property bar by selection".to_string(), pick));
    Ok(Section { title: "Tools", headers: vec!["Tool", "Context"], groups })
}

fn toolbars(cx: &mut Ctx) -> Result<Section> {
    let names: [(&str, &str); 12] = [
        ("Barra de status", "Status Bar"),
        ("Paleta de cores (lateral)", "Color palette"),
        ("Paleta do documento", "Document palette"),
        ("Standard (barra padrao)", "Standard"),
        ("ProjectTimer", "Project Timer"),
        ("Layout", "Layout"),
        ("ZoomPanTool", "Zoom"),
        ("InternetToolbar", "Internet"),
        ("Macros", "Macros"),
        ("ParallelLines", "Parallel Lines"),
        ("Text", "Text"),
        ("Transform", "Transform"),
    ];
    let mut rows = vec![];
    for f in files(&reference().join("06-barras"), "png") {
        let s = strip_number(&stem(&f));
        let Some((_, en)) = names.iter().find(|(pt, _)| *pt == s) else { continue };
        // short names ("Text", "Layout") match unrelated UI strings, so toolbars are only set by hand
        let (status, note) = cx.status("toolbar", en, "\u{0}");
        rows.push(Row { cols: vec![en.to_string()], status, note, reference: rel(&f) });
    }
    Ok(Section { title: "Toolbars", headers: vec!["Toolbar"], groups: vec![(String::new(), rows)] })
}

fn context_menus(cx: &mut Ctx) -> Result<Section> {
    let places: [(&str, &str); 27] = [
        ("retangulo", "Rectangle"),
        ("elipse", "Ellipse"),
        ("curva", "Curve"),
        ("texto-artistico", "Artistic text"),
        ("texto-paragrafo", "Paragraph text"),
        ("bitmap", "Bitmap"),
        ("grupo", "Group"),
        ("selecao-multipla", "Multiple selection"),
        ("tabela", "Table"),
        ("simbolo", "Symbol"),
        ("powerclip", "PowerClip"),
        ("linha-de-cota", "Dimension line"),
        ("conector", "Connector"),
        ("rollover", "Rollover"),
        ("tabela-celulas", "Table cells"),
        ("texto-em-edicao", "Text being edited"),
        ("nos-da-curva", "Curve nodes (Shape tool)"),
        ("pagina", "Page"),
        ("area-de-trabalho", "Desktop (outside the page)"),
        ("regua", "Ruler"),
        ("paleta-de-cores", "Color palette"),
        ("paleta-do-documento", "Document palette"),
        ("aba-da-pagina", "Page tab"),
        ("aba-do-documento", "Document tab"),
        ("barra-de-ferramentas", "Toolbar"),
        ("barra-de-status", "Status bar"),
        ("caixa-de-ferramentas", "Toolbox"),
    ];
    let mut rows = vec![];
    for d in dirs(&reference().join("07-menus-de-contexto")) {
        let s = strip_number(&stem(&d));
        let en = places.iter().find(|(pt, _)| *pt == s).map(|(_, en)| *en).unwrap_or(s.as_str()).to_string();
        let n = count_png(&d);
        let first = files(&d, "png").into_iter().next();
        let (status, note) = cx.status("context-menu", &en, "\u{0}");
        let (status, note) = if n == 0 && status == Status::Todo { (Status::Omitted, "no menu in the reference".to_string()) } else { (status, note) };
        rows.push(Row { cols: vec![en, n.to_string()], status, note, reference: first.map(|f| rel(&f)).unwrap_or_default() });
    }
    Ok(Section { title: "Right-click menus", headers: vec!["Right-click on", "Prints"], groups: vec![(String::new(), rows)] })
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn counts(rows: &[&Row]) -> [usize; 4] {
    let mut c = [0; 4];
    for r in rows {
        c[Status::ALL.iter().position(|s| *s == r.status).unwrap()] += 1;
    }
    c
}

pub fn run() -> Result<()> {
    let refdir = reference();
    if !refdir.is_dir() {
        return Err(format!("reference folder not found: {} (set ANYVECTOR_REFERENCE)", refdir.display()).into());
    }
    let mut cx = Ctx { ui: ui_strings()?, over: overrides()?, used: HashSet::new() };
    let (dlg, fx) = dialogs(&mut cx)?;
    let sections = vec![menus(&mut cx)?, dlg, fx, dockers(&mut cx)?, tools(&mut cx)?, toolbars(&mut cx)?, context_menus(&mut cx)?];

    let mut md = String::new();
    writeln!(md, "# AnyVector {} — coverage of the CorelDRAW reference\n", env!("CARGO_PKG_VERSION"))?;
    writeln!(md, "Generated by `cargo run -p xtask -- coverage` from the reference folder (`MAPEAMENTO`). Do not edit this file: change statuses in `docs/coverage-status.tsv` and run the command again.\n")?;
    writeln!(md, "- ✅ **done**: matches the reference and works.")?;
    writeln!(md, "- 🟡 **partial**: exists in AnyVector but is not yet checked against the reference, or is incomplete (see note).")?;
    writeln!(md, "- ⬜ **todo**: not in AnyVector yet.")?;
    writeln!(md, "- ➖ **omitted**: left out on purpose (note says why).\n")?;
    writeln!(md, "The *Reference* column is a path inside the reference folder.\n")?;
    writeln!(md, "## Summary\n")?;
    writeln!(md, "| Area | Done | Partial | Todo | Omitted | Total |")?;
    writeln!(md, "|---|---:|---:|---:|---:|---:|")?;
    let mut total = [0usize; 4];
    for s in &sections {
        let all: Vec<&Row> = s.groups.iter().flat_map(|(_, r)| r).collect();
        let c = counts(&all);
        for i in 0..4 {
            total[i] += c[i];
        }
        writeln!(md, "| [{}](#{}) | {} | {} | {} | {} | {} |", s.title, s.title.to_lowercase().replace(' ', "-").replace(['(', ')'], ""), c[0], c[1], c[2], c[3], all.len())?;
    }
    writeln!(md, "| **All** | **{}** | **{}** | **{}** | **{}** | **{}** |\n", total[0], total[1], total[2], total[3], total.iter().sum::<usize>())?;

    for s in &sections {
        writeln!(md, "## {}\n", s.title)?;
        for (g, rows) in &s.groups {
            if !g.is_empty() {
                let c = counts(&rows.iter().collect::<Vec<_>>());
                writeln!(md, "### {g}\n\n{} done · {} partial · {} todo · {} omitted\n", c[0], c[1], c[2], c[3])?;
            }
            write!(md, "|")?;
            for h in &s.headers {
                write!(md, " {h} |")?;
            }
            writeln!(md, " Status | Note | Reference |")?;
            writeln!(md, "|{}---|---|---|", "---|".repeat(s.headers.len()))?;
            for r in rows {
                write!(md, "|")?;
                for c in &r.cols {
                    write!(md, " {} |", cell(c))?;
                }
                let refc = if r.reference.is_empty() { String::new() } else { format!("`{}`", cell(&r.reference)) };
                writeln!(md, " {} | {} | {} |", r.status.label(), cell(&r.note), refc)?;
            }
            writeln!(md)?;
        }
    }
    let out = root().join("docs/COVERAGE.md");
    std::fs::write(&out, md)?;
    println!("wrote {} ({} items: {} done, {} partial, {} todo, {} omitted)", out.display(), total.iter().sum::<usize>(), total[0], total[1], total[2], total[3]);

    let unused: Vec<_> = cx.over.keys().filter(|k| !cx.used.contains(*k)).collect();
    for (s, k) in &unused {
        eprintln!("warning: coverage-status.tsv entry matches nothing: {s}\t{k}");
    }
    Ok(())
}
