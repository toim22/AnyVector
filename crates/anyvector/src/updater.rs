//! Product updates: AnyVector looks for a newer release of the project on GitHub, downloads its
//! setup and installs it when the program closes.
//!
//! - Every start looks for a new version in the background (the start is not delayed) and shows
//!   a message only when one is found: OK closes AnyVector, asking to save the drawings, installs
//!   the new version and starts it again.
//! - Tools ▸ Options ▸ Global ▸ General ▸ Allow product updates chooses how (Corel's three
//!   choices): keep the product up to date (downloads by itself, asks before installing), check
//!   and let the user choose when to install, or never check.
//! - Help ▸ Updates shows the version, checks now and installs a new version.
//! - The release is read from the GitHub API; when the repository is private the GitHub CLI
//!   (`gh`, signed in) reads and downloads it instead.
//! - Installing runs the downloaded setup silently after AnyVector closes (so open drawings are
//!   saved first); the setup starts AnyVector again when it is done.

use crate::app::AnyVectorApp;
use crate::bugreport::REPO;
use crate::dialogs::{heading, note, ok_button, separator, window};
use crate::theme;
use egui::{vec2, Color32, RichText};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

/// Allow product updates (Options ▸ Global ▸ General), in Corel's order and words.
pub const MODES: [&str; 3] = ["Keep the product up to date (recommended)", "Check for updates and let me choose when to install them", "Never check for updates"];
pub const MODE_NOTES: [&str; 3] = [
    "Automatically downloads product updates, but prompts you before installing them.",
    "AnyVector tells you when a new version is available; you choose when to download and install it.",
    "AnyVector never connects to the Internet to look for updates. Help ▸ Updates still checks when you ask.",
];
pub const KEEP_UP_TO_DATE: u8 = 0;
pub const NEVER: u8 = 2;

#[derive(Clone, Debug)]
pub struct Release {
    /// "0.2.0"
    pub version: String,
    /// "v0.2.0"
    pub tag: String,
    /// Release notes (GitHub Markdown).
    pub notes: String,
    /// The release page.
    pub page: String,
    /// The setup file and its download address.
    pub asset_name: String,
    pub asset_url: String,
    /// The setup's size in bytes (0 when unknown): a setup already downloaded is used again.
    pub asset_size: u64,
    /// Read through the GitHub CLI (private repository): download with it too.
    pub via_gh: bool,
}

#[derive(Clone, Debug, Default)]
pub enum Status {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Downloading(Release, f32),
    Ready(Release, PathBuf),
    Failed(String),
}

enum Msg {
    Checked(Result<Option<Release>, String>),
    Progress(f32),
    Downloaded(Result<PathBuf, String>),
}

#[derive(Default)]
pub struct UpdateState {
    pub status: Status,
    rx: Option<Receiver<Msg>>,
    /// Help ▸ Updates is open.
    pub dialog: bool,
    /// The "new version" prompt is open.
    pub prompt: bool,
    /// The check was started by the program (not by Help ▸ Updates): failures stay quiet.
    quiet: bool,
    /// Install as soon as the download finishes.
    pub(crate) install_when_ready: bool,
    /// The setup to run when AnyVector closes.
    pub install_on_exit: Option<PathBuf>,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// "v1.2.3" → [1, 2, 3]
pub fn parse_version(s: &str) -> [u64; 3] {
    let mut out = [0u64; 3];
    let core = s.trim().trim_start_matches(['v', 'V']);
    let core = core.split(['-', '+']).next().unwrap_or("");
    for (i, p) in core.split('.').take(3).enumerate() {
        out[i] = p.trim().parse().unwrap_or(0);
    }
    out
}

pub fn is_newer(remote: &str, local: &str) -> bool {
    parse_version(remote) > parse_version(local)
}

/// The release described by GitHub's JSON, if it carries a setup for Windows.
pub fn release_from_json(v: &serde_json::Value, via_gh: bool) -> Option<Release> {
    let tag = v["tag_name"].as_str()?.to_string();
    let asset = v["assets"].as_array()?.iter().find(|a| a["name"].as_str().is_some_and(|n| n.to_ascii_lowercase().ends_with("-setup.exe")))?;
    Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        notes: v["body"].as_str().unwrap_or_default().to_string(),
        page: v["html_url"].as_str().map(str::to_string).unwrap_or_else(|| format!("https://github.com/{REPO}/releases")),
        asset_name: asset["name"].as_str().unwrap_or("AnyVector-setup.exe").to_string(),
        asset_url: asset["browser_download_url"].as_str().unwrap_or_default().to_string(),
        asset_size: asset["size"].as_u64().unwrap_or(0),
        tag,
        via_gh,
    })
}

fn agent(timeout: u64) -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(Duration::from_secs(timeout))).build().into()
}

const NO_INTERNET: &str = "An Internet connection is required. Please connect to the Internet and try again.";

/// The latest release: `Ok(None)` when it is not newer than this version.
fn check() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let ua = format!("AnyVector/{}", current_version());
    let web = agent(20).get(&url).header("Accept", "application/vnd.github+json").header("User-Agent", &ua).call();
    let mut reached = false;
    let release = match web {
        Ok(mut r) if r.status().as_u16() == 200 => {
            let v: serde_json::Value = r.body_mut().read_json().map_err(|e| format!("Invalid answer from the update server: {e}"))?;
            Some(release_from_json(&v, false).ok_or("The latest release has no setup for Windows.")?)
        }
        Ok(_) => {
            reached = true;
            None
        }
        Err(_) => None,
    };
    let release = match release {
        Some(r) => r,
        // not public (or no network): the GitHub CLI, when it is signed in, can read a private repository
        None => match gh_json(&["api", &format!("repos/{REPO}/releases/latest")]) {
            Some(v) => release_from_json(&v, true).ok_or("The latest release has no setup for Windows.")?,
            None if reached => return Err(format!("No published release was found at github.com/{REPO}.")),
            None => return Err(NO_INTERNET.into()),
        },
    };
    Ok(is_newer(&release.version, current_version()).then_some(release))
}

fn gh_json(args: &[&str]) -> Option<serde_json::Value> {
    let out = crate::bugreport::gh(args)?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

pub fn download_dir() -> PathBuf {
    std::env::temp_dir().join("AnyVector Updates")
}

fn download(r: &Release, tx: &Sender<Msg>) -> Result<PathBuf, String> {
    let dir = download_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(&r.asset_name);
    if r.via_gh {
        let out = crate::bugreport::gh(&["release", "download", &r.tag, "--repo", REPO, "--pattern", &r.asset_name, "--dir", &dir.to_string_lossy(), "--clobber"]).ok_or(NO_INTERNET)?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
    } else {
        let ua = format!("AnyVector/{}", current_version());
        let mut resp = agent(1800).get(&r.asset_url).header("User-Agent", &ua).call().map_err(|_| NO_INTERNET.to_string())?;
        if resp.status().as_u16() != 200 {
            return Err(format!("The update server answered {}.", resp.status().as_u16()));
        }
        let total: Option<u64> = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
        let part = path.with_extension("part");
        let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut reader = resp.body_mut().with_config().limit(1 << 31).reader();
        let mut buf = vec![0u8; 1 << 16];
        let mut done = 0u64;
        loop {
            let n = std::io::Read::read(&mut reader, &mut buf).map_err(|_| NO_INTERNET.to_string())?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| e.to_string())?;
            done += n as u64;
            if let Some(t) = total.filter(|t| *t > 0) {
                let _ = tx.send(Msg::Progress(done as f32 / t as f32));
            }
        }
        drop(file);
        if total.is_some_and(|t| t != done) {
            return Err("The download was interrupted. Please try again.".into());
        }
        let _ = std::fs::remove_file(&path);
        std::fs::rename(&part, &path).map_err(|e| e.to_string())?;
    }
    match std::fs::metadata(&path) {
        Ok(m) if m.len() > 0 => Ok(path),
        _ => Err("The downloaded setup is empty.".into()),
    }
}

/// Start a check in the background. `quiet`: started by the program, so a failure is not shown.
pub fn start_check(app: &mut AnyVectorApp, quiet: bool) {
    let u = &mut app.update;
    if u.rx.is_some() {
        return;
    }
    u.quiet = quiet;
    u.status = Status::Checking;
    let (tx, rx) = channel();
    u.rx = Some(rx);
    std::thread::spawn(move || {
        let _ = tx.send(Msg::Checked(check()));
    });
}

fn start_download(app: &mut AnyVectorApp, r: Release) {
    let u = &mut app.update;
    if u.rx.is_some() {
        return;
    }
    // the setup downloaded at an earlier start (the user chose Later) is used again
    let path = download_dir().join(&r.asset_name);
    if r.asset_size > 0 && std::fs::metadata(&path).is_ok_and(|m| m.len() == r.asset_size) {
        u.status = Status::Ready(r, path.clone());
        if std::mem::take(&mut u.install_when_ready) {
            install(app, path);
        }
        return;
    }
    u.status = Status::Downloading(r.clone(), 0.0);
    let (tx, rx) = channel();
    u.rx = Some(rx);
    std::thread::spawn(move || {
        let res = download(&r, &tx);
        let _ = tx.send(Msg::Downloaded(res));
    });
}

/// The automatic check, at every start (not in tests). It runs on its own thread, so the start is
/// not delayed, and it stays silent unless a new version is found (`poll` opens the message).
pub fn startup(app: &mut AnyVectorApp) {
    if cfg!(test) || app.settings.update_mode >= NEVER {
        return;
    }
    start_check(app, true);
}

/// Help ▸ Updates.
pub fn open_dialog(app: &mut AnyVectorApp) {
    app.update.dialog = true;
    if matches!(app.update.status, Status::Idle | Status::UpToDate | Status::Failed(_)) {
        start_check(app, false);
    }
}

/// Install now: the setup runs when AnyVector closes, after the open drawings are saved.
fn install(app: &mut AnyVectorApp, path: PathBuf) {
    app.update.install_on_exit = Some(path);
    app.update.prompt = false;
    app.update.dialog = false;
    app.run(crate::commands::Cmd::Exit);
}

/// Run the downloaded setup (on exit): silently, closing AnyVector if needed and starting it
/// again afterwards.
pub fn run_pending_install(app: &mut AnyVectorApp) {
    let Some(path) = app.update.install_on_exit.take() else { return };
    let _ = launch_setup(&path);
}

fn launch_setup(path: &Path) -> std::io::Result<std::process::Child> {
    std::process::Command::new(path).args(["/SILENT", "/SP-", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS", "/RELAUNCH=1"]).spawn()
}

/// Background results, each frame.
pub fn poll(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(rx) = &app.update.rx else { return };
    let mut msgs = vec![];
    while let Ok(m) = rx.try_recv() {
        msgs.push(m);
    }
    ctx.request_repaint_after(Duration::from_millis(200));
    for m in msgs {
        match m {
            Msg::Progress(p) => {
                if let Status::Downloading(_, q) = &mut app.update.status {
                    *q = p;
                }
            }
            Msg::Checked(res) => {
                app.update.rx = None;
                let quiet = app.update.quiet;
                match res {
                    Ok(None) => {
                        app.update.status = Status::UpToDate;
                        // the setup of the version now running is no longer needed
                        let _ = std::fs::remove_dir_all(download_dir());
                    }
                    Ok(Some(r)) => {
                        app.update.status = Status::Available(r.clone());
                        if quiet {
                            // the message at start; keeping up to date downloads meanwhile, so OK installs at once
                            app.update.prompt = true;
                            if app.settings.update_mode == KEEP_UP_TO_DATE {
                                start_download(app, r);
                            }
                        }
                    }
                    Err(e) => app.update.status = if quiet { Status::Idle } else { Status::Failed(e) },
                }
            }
            Msg::Downloaded(res) => {
                app.update.rx = None;
                let Status::Downloading(r, _) = std::mem::take(&mut app.update.status) else { continue };
                match res {
                    Ok(path) => {
                        app.update.status = Status::Ready(r, path.clone());
                        if std::mem::take(&mut app.update.install_when_ready) {
                            install(app, path);
                        }
                    }
                    Err(e) => {
                        app.update.install_when_ready = false;
                        app.update.status = if app.update.quiet && !app.update.dialog { Status::Available(r) } else { Status::Failed(e) };
                    }
                }
            }
        }
    }
}

/// Install Now (or OK in the message): download first when needed, then close and install.
fn install_now(app: &mut AnyVectorApp) {
    match app.update.status.clone() {
        Status::Ready(_, path) => install(app, path),
        Status::Available(r) => {
            app.update.install_when_ready = true;
            app.update.quiet = false;
            start_download(app, r);
        }
        Status::Downloading(..) => {
            app.update.install_when_ready = true;
            app.update.quiet = false;
        }
        _ => {}
    }
}

/// Room the Updates window keeps under the status for the update setting and its note.
const SETTING_ROOM: f32 = 96.0;

/// The release notes, scrolling, in what is left of the window above `room` (at most 180 points).
fn release_notes(ui: &mut egui::Ui, r: &Release, room: f32) {
    let h = (ui.available_height() - room).clamp(36.0, 180.0);
    egui::ScrollArea::vertical().max_height(h).auto_shrink([false, true]).show(ui, |ui| {
        if r.notes.trim().is_empty() {
            ui.label(RichText::new("This release has no notes.").color(theme::TEXT()).size(12.0));
        } else {
            notes_ui(ui, &r.notes);
        }
    });
}

/// A piece of the release notes, as GitHub shows them: a heading, a list item or a paragraph.
#[derive(Debug, PartialEq)]
pub(crate) enum NoteBlock {
    /// `## Heading` (level 2 and below; the `# AnyVector x.y.z` title is left out, the window says it already)
    Heading(Vec<(String, bool)>),
    /// `- item`, nested two spaces per level
    Item(usize, Vec<(String, bool)>),
    Para(Vec<(String, bool)>),
}

/// The text with `**bold**` as (text, bold) runs and the backticks of `code` dropped.
fn note_runs(s: &str) -> Vec<(String, bool)> {
    let s = s.replace('`', "");
    s.split("**").enumerate().filter(|(_, t)| !t.is_empty()).map(|(i, t)| (t.to_string(), i % 2 == 1)).collect()
}

/// The release notes (GitHub Markdown, lines wrapped at a fixed width) as blocks: wrapped lines join their block.
pub(crate) fn note_blocks(md: &str) -> Vec<NoteBlock> {
    let mut out = vec![];
    // the block being read: its kind and its text so far
    let mut cur: Option<(Option<usize>, String)> = None;
    let flush = |cur: &mut Option<(Option<usize>, String)>, out: &mut Vec<NoteBlock>| {
        if let Some((depth, text)) = cur.take() {
            out.push(match depth {
                Some(d) => NoteBlock::Item(d, note_runs(&text)),
                None => NoteBlock::Para(note_runs(&text)),
            });
        }
    };
    for line in md.lines() {
        let t = line.trim();
        if t.is_empty() {
            flush(&mut cur, &mut out);
            continue;
        }
        if t.starts_with('#') {
            flush(&mut cur, &mut out);
            let level = t.chars().take_while(|c| *c == '#').count();
            if level >= 2 {
                out.push(NoteBlock::Heading(note_runs(t.trim_start_matches('#').trim())));
            }
            continue;
        }
        if let Some(item) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            flush(&mut cur, &mut out);
            let indent = line.len() - line.trim_start().len();
            cur = Some((Some(indent / 2), item.trim().to_string()));
            continue;
        }
        match &mut cur {
            Some((_, text)) => {
                text.push(' ');
                text.push_str(t);
            }
            None => cur = Some((None, t.to_string())),
        }
    }
    flush(&mut cur, &mut out);
    out
}

fn notes_ui(ui: &mut egui::Ui, md: &str) {
    let job = |runs: &[(String, bool)], size: f32, base: Color32| {
        let mut job = egui::text::LayoutJob::default();
        for (t, bold) in runs {
            let color = if *bold { crate::theme::STRONG() } else { base };
            job.append(t, 0.0, egui::TextFormat { font_id: egui::FontId::proportional(size), color, ..Default::default() });
        }
        job
    };
    ui.spacing_mut().item_spacing.y = 3.0;
    for (i, b) in note_blocks(md).iter().enumerate() {
        match b {
            NoteBlock::Heading(runs) => {
                if i > 0 {
                    ui.add_space(6.0);
                }
                ui.add(egui::Label::new(job(runs, 13.0, crate::theme::STRONG())).wrap());
            }
            NoteBlock::Para(runs) => {
                ui.add(egui::Label::new(job(runs, 12.0, theme::TEXT())).wrap());
            }
            NoteBlock::Item(depth, runs) => {
                ui.horizontal_top(|ui| {
                    ui.add_space(4.0 + 14.0 * *depth as f32);
                    ui.label(RichText::new(if *depth == 0 { "•" } else { "◦" }).color(theme::TEXT()).size(12.0));
                    ui.add(egui::Label::new(job(runs, 12.0, theme::TEXT())).wrap());
                });
            }
        }
    }
}

fn status_ui(app: &mut AnyVectorApp, ui: &mut egui::Ui, room: f32) {
    match app.update.status.clone() {
        Status::Idle => {
            note(ui, "Click Check Now to look for a new version.");
        }
        Status::Checking => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Checking for updates...");
            });
        }
        Status::UpToDate => {
            ui.label("AnyVector is up to date.");
        }
        Status::Available(r) => {
            ui.label(RichText::new(format!("AnyVector {} is available.", r.version)).strong());
            release_notes(ui, &r, room);
        }
        Status::Downloading(r, p) => {
            ui.label(RichText::new(format!("AnyVector {} is available.", r.version)).strong());
            ui.label(format!("Downloading AnyVector {}...", r.version));
            ui.add(egui::ProgressBar::new(p).desired_width(ui.available_width()).show_percentage());
            if app.update.install_when_ready {
                note(ui, "AnyVector closes to install it as soon as the download finishes (it asks to save your drawings first).");
            }
            release_notes(ui, &r, room);
        }
        Status::Ready(r, _) => {
            ui.label(RichText::new(format!("AnyVector {} is ready to install.", r.version)).strong());
            note(ui, "AnyVector closes to install it (it asks to save your drawings first) and starts again when it is done.");
            release_notes(ui, &r, room);
        }
        Status::Failed(e) => {
            ui.colored_label(Color32::from_rgb(0xe0, 0x70, 0x60), format!("Error: {e}"));
        }
    }
}

/// Help ▸ Updates and the "new version" prompt.
pub fn dialogs(app: &mut AnyVectorApp, ctx: &egui::Context) {
    poll(app, ctx);
    if app.update.dialog {
        let mut close = false;
        let ((), dismissed) = window(ctx, "updates", "Updates", vec2(520.0, 400.0), |ui| {
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.y = 6.0;
            // the buttons keep the bottom of the window; everything else fits above them (issue #33)
            let full = ui.max_rect();
            let bar = egui::Rect::from_min_max(egui::pos2(full.left(), full.bottom() - 28.0), full.max);
            let top = egui::Rect::from_min_max(full.min, egui::pos2(full.right(), bar.top() - 4.0));
            let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(top).layout(egui::Layout::top_down(egui::Align::Min)));
            let ui = &mut ui;
            heading(ui, &format!("You are using AnyVector {}.", current_version()));
            let room = SETTING_ROOM + if app.update.install_on_exit.is_some() { 24.0 } else { 0.0 };
            status_ui(app, ui, room);
            if app.update.install_on_exit.is_some() {
                note(ui, "The update will be installed when AnyVector closes.");
            }
            separator(ui);
            ui.horizontal(|ui| {
                ui.label("Allow product updates:");
                let mut m = app.settings.update_mode as usize;
                egui::ComboBox::from_id_salt("upd_mode").selected_text(MODES[m.min(2)]).width(300.0).show_ui(ui, |ui| {
                    for (i, t) in MODES.iter().enumerate() {
                        ui.selectable_value(&mut m, i, *t);
                    }
                });
                if m as u8 != app.settings.update_mode {
                    app.settings.update_mode = m as u8;
                    app.settings.save();
                }
            });
            note(ui, MODE_NOTES[(app.settings.update_mode as usize).min(2)]);
            let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::left_to_right(egui::Align::Center)));
            ui.horizontal(|ui| {
                if ui.link("Release Notes").clicked() {
                    match &app.update.status {
                        Status::Available(r) | Status::Downloading(r, _) | Status::Ready(r, _) => {
                            crate::bugreport::open_in_browser(&r.page);
                        }
                        _ => {
                            let _ = crate::help::open(crate::help::Page::ReleaseNotes);
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new("Close").min_size(vec2(80.0, 24.0))).clicked() {
                        close = true;
                    }
                    // also while the setup downloads in the background: it installs when the download ends (issue #33)
                    let (can_install, enabled) = match app.update.status {
                        Status::Available(_) | Status::Ready(..) => (true, app.update.rx.is_none()),
                        Status::Downloading(..) => (true, !app.update.install_when_ready),
                        _ => (false, false),
                    };
                    if can_install && ok_button(ui, "Install Now", enabled) {
                        install_now(app);
                    }
                    let busy = app.update.rx.is_some();
                    if ui.add_enabled(!busy, egui::Button::new("Check Now").min_size(vec2(90.0, 24.0))).clicked() {
                        start_check(app, false);
                    }
                });
            });
        });
        if close || dismissed {
            app.update.dialog = false;
        }
    }
    if app.update.prompt && !app.update.dialog {
        prompt(app, ctx);
    }
}

/// The message shown when the check at start finds a new version. OK closes AnyVector (asking to
/// save the drawings), installs the new version and starts it again; Later asks again at the next start.
fn prompt(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let mut close = false;
    let release = match &app.update.status {
        Status::Available(r) | Status::Downloading(r, _) | Status::Ready(r, _) => Some(r.clone()),
        _ => None,
    };
    // after OK, while the setup downloads
    let installing = app.update.install_when_ready;
    let ((), dismissed) = window(ctx, "update_prompt", "Updates", vec2(460.0, 230.0), |ui| {
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing.y = 8.0;
        match (&release, &app.update.status) {
            (_, Status::Failed(e)) => {
                ui.colored_label(Color32::from_rgb(0xe0, 0x70, 0x60), format!("Error: {e}"));
            }
            (Some(r), Status::Downloading(_, p)) if installing => {
                ui.label(format!("Downloading AnyVector {}...", r.version));
                ui.add(egui::ProgressBar::new(*p).desired_width(ui.available_width()).show_percentage());
                note(ui, "AnyVector closes to install it (it asks to save your drawings first) and starts again when it is done.");
            }
            (Some(r), _) => {
                ui.label(RichText::new("There is a new version of AnyVector available.").strong().size(14.0));
                ui.label(format!("New version: {}. Your version: {}.", r.version, current_version()));
                ui.label("Click OK to install the new version.");
                note(ui, "AnyVector closes to install it (it asks to save your drawings first) and starts again when it is done.");
            }
            (None, _) => close = true,
        }
        ui.add_space((ui.available_height() - 34.0).max(0.0));
        ui.horizontal(|ui| {
            if let Some(r) = &release {
                if ui.link("Release Notes").clicked() {
                    crate::bugreport::open_in_browser(&r.page);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let failed = matches!(app.update.status, Status::Failed(_));
                if ui.add(egui::Button::new(if failed { "Close" } else { "Later" }).min_size(vec2(80.0, 24.0))).clicked() {
                    close = true;
                }
                if !failed && ok_button(ui, "OK", !installing) {
                    install_now(app);
                }
            });
        });
    });
    if close || dismissed {
        app.update.prompt = false;
        // the download may go on; the update is offered again at the next start
        app.update.install_when_ready = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_notes_read_as_github_shows_them() {
        let md = "# AnyVector 0.3.0

Intro that wraps
onto a second line.

## What's `new`

- **Fonts.** A missing font
  gets a look-alike:
  - narrow stays narrow
- Last
";
        let b = note_blocks(md);
        let run = |t: &str, bold: bool| (t.to_string(), bold);
        assert_eq!(b, vec![
            NoteBlock::Para(vec![run("Intro that wraps onto a second line.", false)]),
            NoteBlock::Heading(vec![run("What's new", false)]),
            NoteBlock::Item(0, vec![run("Fonts.", true), run(" A missing font gets a look-alike:", false)]),
            NoteBlock::Item(1, vec![run("narrow stays narrow", false)]),
            NoteBlock::Item(0, vec![run("Last", false)]),
        ]);
        // the real notes have no Markdown marks left
        for b in [include_str!("../../../docs/RELEASE-NOTES-0.3.0.md"), include_str!("../../../docs/RELEASE-NOTES-0.3.1.md"), include_str!("../../../docs/RELEASE-NOTES-0.4.0.md")].iter().flat_map(|md| note_blocks(md)) {
            let (NoteBlock::Heading(r) | NoteBlock::Item(_, r) | NoteBlock::Para(r)) = b;
            for (t, _) in r {
                assert!(!t.contains("**") && !t.contains('`') && !t.starts_with('#') && !t.starts_with("- "), "{t}");
            }
        }
    }

    #[test]
    fn versions_compare() {
        assert_eq!(parse_version("v1.2.3"), [1, 2, 3]);
        assert_eq!(parse_version("0.2"), [0, 2, 0]);
        assert_eq!(parse_version("V0.3.1-beta.2"), [0, 3, 1]);
        assert!(is_newer("v0.1.1", "0.1.0"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn release_json_needs_a_setup() {
        let v = serde_json::json!({
            "tag_name": "v0.2.0",
            "body": "New tools",
            "html_url": "https://github.com/toim22/AnyVector/releases/tag/v0.2.0",
            "assets": [
                { "name": "notes.txt", "browser_download_url": "https://x/notes.txt" },
                { "name": "AnyVector-0.2.0-setup.exe", "browser_download_url": "https://x/AnyVector-0.2.0-setup.exe", "size": 17972048 }
            ]
        });
        let r = release_from_json(&v, false).unwrap();
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.asset_name, "AnyVector-0.2.0-setup.exe");
        assert_eq!(r.asset_url, "https://x/AnyVector-0.2.0-setup.exe");
        assert_eq!(r.asset_size, 17972048);
        let none = serde_json::json!({ "tag_name": "v0.2.0", "assets": [] });
        assert!(release_from_json(&none, false).is_none());
    }
}
