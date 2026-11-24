//! Product updates: AnyVector looks for a newer release of the project on GitHub, downloads its
//! setup (or AppImage) and installs it when the program closes.
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
//! - Linux: an AnyVector that runs from its AppImage downloads the new AppImage, checks it and
//!   replaces its own file when it closes, then starts again (`appimage.rs`). Any other copy (a .deb
//!   package, a plain program) and an AppImage the user can't write are only told of a new version:
//!   Open Release Page takes the user to its download page, and nothing is downloaded or run. See
//!   [`Install`].

use crate::app::AnyVectorApp;
use crate::bugreport::REPO;
use crate::dialogs::{heading, note, ok_button, separator, window};
use crate::theme;
use egui::{vec2, Color32, RichText};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

#[cfg(target_os = "linux")]
mod appimage;

/// Allow product updates (Options ▸ Global ▸ General), in Corel's order and words.
pub const MODES: [&str; 3] = ["Keep the product up to date (recommended)", "Check for updates and let me choose when to install them", "Never check for updates"];
/// What each of the [`MODES`] does where AnyVector installs new versions by itself.
const MODE_NOTES: [&str; 3] = [
    "Automatically downloads product updates, but prompts you before installing them.",
    "AnyVector tells you when a new version is available; you choose when to download and install it.",
    "AnyVector never connects to the Internet to look for updates. Help ▸ Updates still checks when you ask.",
];
/// Where new versions are only announced, keeping up to date means hearing of each one.
const ANNOUNCED_MODE_NOTES: [&str; 3] = ["AnyVector looks for a new version at every start and tells you when one is available.", MODE_NOTES[1], MODE_NOTES[2]];
pub const KEEP_UP_TO_DATE: u8 = 0;
pub const NEVER: u8 = 2;

/// How this copy of AnyVector was installed, which decides whether it installs new versions by itself.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub enum Install {
    /// Windows: the new setup is downloaded and run.
    Setup,
    /// Linux, running from an AppImage: its file is replaced by the new AppImage.
    AppImage(AppImage),
    /// A package (.deb), a plain program, another system: new versions are only announced.
    Other,
}

/// The AppImage this AnyVector runs from.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub struct AppImage {
    /// Its file (links followed).
    pub file: PathBuf,
    /// The file's device and inode at start: another file that takes its place is not replaced.
    pub id: (u64, u64),
    /// The runtime extracted the AppImage to a folder instead of mounting it
    /// (`--appimage-extract-and-run`, for computers without FUSE).
    pub extracted: bool,
    /// The user can write the file and its folder.
    pub writable: bool,
}

impl Default for Install {
    fn default() -> Install {
        Install::detect()
    }
}

impl Install {
    /// How the running program was installed.
    pub fn detect() -> Install {
        if cfg!(windows) {
            return Install::Setup;
        }
        #[cfg(target_os = "linux")]
        if let Some(a) = appimage::detect() {
            return Install::AppImage(a);
        }
        Install::Other
    }

    /// Looks again whether the AppImage's file can be replaced.
    fn refresh(&mut self) {
        #[cfg(target_os = "linux")]
        if let Install::AppImage(a) = self {
            a.writable = appimage::writable(&a.file);
        }
    }

    /// What the mode of [`MODES`] chosen does for this copy of AnyVector.
    pub fn mode_note(&self, mode: u8) -> &'static str {
        let by_itself = match self {
            Install::Setup => true,
            Install::AppImage(a) => a.writable,
            Install::Other => false,
        };
        (if by_itself { MODE_NOTES } else { ANNOUNCED_MODE_NOTES })[(mode as usize).min(2)]
    }

    /// Why the release is only announced (with Open Release Page), or `None` when Install Now (or
    /// OK) downloads and installs it.
    pub fn announced_because(&self, r: &Release) -> Option<String> {
        let has_asset_of = |os| asset_for(os, std::env::consts::ARCH, &r.version, [r.asset_name.as_str()]).is_some();
        match self {
            Install::Setup => (!has_asset_of("windows")).then(|| NO_SETUP.to_string()),
            Install::Other => Some("Only the AppImage of AnyVector installs updates by itself on Linux.".into()),
            Install::AppImage(_) if !has_asset_of("linux") => Some("This release has no AppImage for this computer.".into()),
            // The program is not replaced by a file that can't be checked. GitHub gives the checksum of
            // every file uploaded since June 2025, so a release without one is not what it should be.
            Install::AppImage(_) if r.sha256().is_none() => Some("This release has no checksum to verify its AppImage with.".into()),
            Install::AppImage(a) if !a.writable => Some(no_permission(&a.file)),
            Install::AppImage(_) => None,
        }
    }

    /// Install Now (or OK) downloads and installs the release; otherwise it is only announced.
    pub fn installs(&self, r: &Release) -> bool {
        self.announced_because(r).is_none()
    }
}

fn no_permission(file: &Path) -> String {
    format!("You do not have permission to replace {}.", file.display())
}

#[derive(Clone, Debug, Default)]
pub struct Release {
    /// "0.2.0"
    pub version: String,
    /// "v0.2.0"
    pub tag: String,
    /// Release notes (GitHub Markdown).
    pub notes: String,
    /// The release page.
    pub page: String,
    /// The setup (or AppImage) and its download address (empty when the release has nothing for
    /// this system, see [`asset_for`]).
    pub asset_name: String,
    pub asset_url: String,
    /// The file's size in bytes (0 when unknown): a file already downloaded is used again.
    pub asset_size: u64,
    /// The file's checksum as GitHub gives it, "sha256:..." (empty when it gives none).
    pub digest: String,
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
    /// The release to install when AnyVector closes, and its downloaded file.
    pub install_on_exit: Option<(Release, PathBuf)>,
    /// How this copy of AnyVector was installed.
    pub install: Install,
    /// The update that could not be installed when AnyVector last closed, to be shown.
    pub error: Option<InstallError>,
    /// Where updates are downloaded, when not in [`download_dir`] (tests).
    pub(crate) dir: Option<PathBuf>,
}

/// An update that could not be installed when AnyVector closed (Linux): kept in the download folder
/// until the next start shows it, as there is no window left to show it in.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub struct InstallError {
    /// The version that was to be installed, and its page.
    pub version: String,
    pub page: String,
    pub reason: String,
}

impl UpdateState {
    fn dir(&self) -> PathBuf {
        self.dir.clone().unwrap_or_else(download_dir)
    }

    /// The release found, when it is only announced, and why.
    fn announced(&self) -> Option<(Release, String)> {
        match &self.status {
            Status::Available(r) | Status::Ready(r, _) => self.install.announced_because(r).map(|why| (r.clone(), why)),
            _ => None,
        }
    }
}

/// The folder of the AppImage AnyVector runs from, where its program and libraries are.
#[cfg(not(windows))]
pub fn appimage_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    return appimage::app_dir(std::env::var_os("APPDIR").as_deref(), &std::env::current_exe().ok()?);
    #[cfg(not(target_os = "linux"))]
    None
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

/// The file of release `version` for the system `os` on an `arch` processor
/// (`std::env::consts::OS` and `ARCH`), among the release's file names: the setup on Windows
/// (`AnyVector-0.6.2-setup.exe`), the AppImage on Linux (`AnyVector-0.7.0-x86_64.AppImage`). The .deb
/// package is never chosen: the system's package manager installs it. Whether AnyVector installs the
/// file by itself depends on how it was installed ([`Install`]).
pub fn asset_for<'a>(os: &str, arch: &str, version: &str, names: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let appimage = format!("AnyVector-{version}-{arch}.AppImage");
    names.into_iter().find(|n| match os {
        "windows" => n.to_ascii_lowercase().ends_with("-setup.exe"),
        // the whole name, which is also a plain file name: no folder, nothing after the extension
        "linux" => n.eq_ignore_ascii_case(&appimage) && n.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.+".contains(&b)),
        _ => false,
    })
}

impl Release {
    /// The checksum of the file, when GitHub gives one.
    fn sha256(&self) -> Option<[u8; 32]> {
        let hex = self.digest.strip_prefix("sha256:").filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))?;
        let mut out = [0; 32];
        for (b, pair) in out.iter_mut().zip(hex.as_bytes().chunks(2)) {
            *b = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
        }
        Some(out)
    }
}

/// The release described by GitHub's JSON. On Windows only one that carries its setup; elsewhere
/// every release, to announce it.
pub fn release_from_json(v: &serde_json::Value, via_gh: bool) -> Option<Release> {
    release_for(std::env::consts::OS, std::env::consts::ARCH, v, via_gh)
}

fn release_for(os: &str, arch: &str, v: &serde_json::Value, via_gh: bool) -> Option<Release> {
    let tag = v["tag_name"].as_str()?.to_string();
    let version = tag.trim_start_matches(['v', 'V']).to_string();
    let assets = v["assets"].as_array().map(Vec::as_slice).unwrap_or_default();
    let name = asset_for(os, arch, &version, assets.iter().filter_map(|a| a["name"].as_str()));
    let asset = name.and_then(|n| assets.iter().find(|a| a["name"].as_str() == Some(n)));
    // Windows installs the setup, so a release without one is of no use there
    if asset.is_none() && os == "windows" {
        return None;
    }
    let field = |k: &str| asset.and_then(|a| a[k].as_str()).unwrap_or_default().to_string();
    Some(Release {
        version,
        notes: v["body"].as_str().unwrap_or_default().to_string(),
        page: v["html_url"].as_str().map(str::to_string).unwrap_or_else(|| format!("https://github.com/{REPO}/releases")),
        asset_name: field("name"),
        asset_url: field("browser_download_url"),
        asset_size: asset.and_then(|a| a["size"].as_u64()).unwrap_or(0),
        digest: field("digest"),
        tag,
        via_gh,
    })
}

fn agent(timeout: u64) -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(Duration::from_secs(timeout))).build().into()
}

const NO_SETUP: &str = "The latest release has no setup for Windows.";
const NO_INTERNET: &str = "An Internet connection is required. Please connect to the Internet and try again.";

/// Where GitHub describes the latest release.
fn latest_url() -> String {
    format!("https://api.github.com/repos/{REPO}/releases/latest")
}

/// The latest release, described at `url` ([`latest_url`]): `Ok(None)` when it is not newer than
/// this version.
fn check(url: &str) -> Result<Option<Release>, String> {
    let ua = format!("AnyVector/{}", current_version());
    let web = agent(20).get(url).header("Accept", "application/vnd.github+json").header("User-Agent", &ua).call();
    let mut reached = false;
    let release = match web {
        Ok(mut r) if r.status().as_u16() == 200 => {
            let v: serde_json::Value = r.body_mut().read_json().map_err(|e| format!("Invalid answer from the update server: {e}"))?;
            Some(release_from_json(&v, false).ok_or_else(unusable_release)?)
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
            Some(v) => release_from_json(&v, true).ok_or_else(unusable_release)?,
            None if reached => return Err(format!("No published release was found at github.com/{REPO}.")),
            None => return Err(NO_INTERNET.into()),
        },
    };
    Ok(is_newer(&release.version, current_version()).then_some(release))
}

/// Why the latest release is of no use ([`release_from_json`] refused it): Windows needs its setup;
/// elsewhere only a release without a tag is refused.
fn unusable_release() -> String {
    if cfg!(windows) {
        NO_SETUP.to_string()
    } else {
        format!("No published release was found at github.com/{REPO}.")
    }
}

fn gh_json(args: &[&str]) -> Option<serde_json::Value> {
    let out = crate::bugreport::gh(args)?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

/// Where the update is downloaded: Windows' temporary folder (each user has their own); elsewhere
/// the user's cache folder (`$XDG_CACHE_HOME/AnyVector/Updates`), not the `/tmp` all users share,
/// where another user could leave a file of the same name and size to be installed.
pub fn download_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::temp_dir().join("AnyVector Updates")
    } else {
        dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("AnyVector").join("Updates")
    }
}

/// The release's file for this copy of AnyVector. An AppImage is checked against the release, also
/// when it was downloaded at an earlier start (the user chose Later) and is used again.
fn fetch(install: &Install, r: &Release, dir: &Path, tx: &Sender<Msg>) -> Result<PathBuf, String> {
    #[cfg(target_os = "linux")]
    if matches!(install, Install::AppImage(_)) {
        let earlier = dir.join(&r.asset_name);
        if appimage::verify(&earlier, r).is_ok() {
            return Ok(earlier);
        }
        let path = download(r, dir, tx)?;
        return match appimage::verify(&path, r) {
            Ok(()) => Ok(path),
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                Err(e)
            }
        };
    }
    let _ = install;
    download(r, dir, tx)
}

fn download(r: &Release, dir: &Path, tx: &Sender<Msg>) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
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
    start_check_at(app, quiet, latest_url());
}

fn start_check_at(app: &mut AnyVectorApp, quiet: bool, url: String) {
    let u = &mut app.update;
    if u.rx.is_some() {
        return;
    }
    u.quiet = quiet;
    u.install.refresh();
    u.status = Status::Checking;
    let (tx, rx) = channel();
    u.rx = Some(rx);
    std::thread::spawn(move || {
        let _ = tx.send(Msg::Checked(check(&url)));
    });
}

fn start_download(app: &mut AnyVectorApp, r: Release) {
    let u = &mut app.update;
    // nothing this copy of AnyVector installs: the release is only announced
    if u.rx.is_some() || !u.install.installs(&r) {
        return;
    }
    let (dir, kind) = (u.dir(), u.install.clone());
    // the setup downloaded at an earlier start (the user chose Later) is used again (an AppImage too,
    // once it is checked: `fetch`)
    let path = dir.join(&r.asset_name);
    if kind == Install::Setup && r.asset_size > 0 && std::fs::metadata(&path).is_ok_and(|m| m.len() == r.asset_size) {
        u.status = Status::Ready(r.clone(), path.clone());
        if std::mem::take(&mut u.install_when_ready) {
            install(app, r, path);
        }
        return;
    }
    u.status = Status::Downloading(r.clone(), 0.0);
    let (tx, rx) = channel();
    u.rx = Some(rx);
    std::thread::spawn(move || {
        let res = fetch(&kind, &r, &dir, &tx);
        let _ = tx.send(Msg::Downloaded(res));
    });
}

/// The automatic check, at every start (not in tests). It runs on its own thread, so the start is
/// not delayed, and it stays silent unless a new version is found (`poll` opens the message).
pub fn startup(app: &mut AnyVectorApp) {
    if cfg!(test) {
        return;
    }
    #[cfg(target_os = "linux")]
    {
        app.update.error = appimage::take_error(&app.update.dir());
    }
    if app.settings.update_mode >= NEVER {
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

/// Install now: the setup runs (the AppImage is replaced) when AnyVector closes, after the open
/// drawings are saved.
fn install(app: &mut AnyVectorApp, r: Release, path: PathBuf) {
    app.update.install_on_exit = Some((r, path));
    app.update.prompt = false;
    app.update.dialog = false;
    app.run(crate::commands::Cmd::Exit);
}

/// Install the downloaded update (on exit). Windows: run the setup silently, closing AnyVector if
/// needed and starting it again afterwards. Linux: replace the AppImage and start it again.
pub fn run_pending_install(app: &mut AnyVectorApp) {
    let Some((release, path)) = app.update.install_on_exit.take() else { return };
    match &app.update.install {
        #[cfg(target_os = "linux")]
        Install::AppImage(a) => appimage::install(a, &release, &path, &app.update.dir()),
        #[cfg(windows)]
        Install::Setup => {
            let _ = launch_setup(&path);
        }
        // nothing else is installed by AnyVector, and never a Windows setup off Windows
        _ => log::warn!("{} {} is not installed by this copy of AnyVector", release.version, path.display()),
    }
}

#[cfg(windows)]
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
                        let _ = std::fs::remove_dir_all(app.update.dir());
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
                        app.update.status = Status::Ready(r, path);
                        if std::mem::take(&mut app.update.install_when_ready) {
                            install_now(app);
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

/// Open Release Page, for a release this copy of AnyVector doesn't install by itself (Linux): the
/// page to download it from, in the browser; the message at start has done its job.
fn open_release_page(app: &mut AnyVectorApp, page: &str) {
    // tests never open the browser
    if !cfg!(test) {
        crate::bugreport::open_in_browser(page);
    }
    app.update.prompt = false;
}

/// Install Now (or OK in the message): download first when needed, then close and install.
fn install_now(app: &mut AnyVectorApp) {
    // the AppImage may have become read-only: the message then says so and offers the release page
    app.update.install.refresh();
    if app.update.announced().is_some() {
        return;
    }
    match app.update.status.clone() {
        Status::Ready(r, path) => install(app, r, path),
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
            if let Some(why) = app.update.install.announced_because(&r) {
                note(ui, "Click Open Release Page to download it.");
                note(ui, &why);
            }
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
            match app.update.install.announced_because(&r) {
                Some(why) => note(ui, &why),
                None => note(ui, "AnyVector closes to install it (it asks to save your drawings first) and starts again when it is done."),
            }
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
            note(ui, app.update.install.mode_note(app.settings.update_mode));
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
                    // only announced (Linux): the page to download it from
                    let announced = app.update.announced();
                    let (can_install, enabled) = match &app.update.status {
                        _ if announced.is_some() => (false, false),
                        Status::Available(_) | Status::Ready(..) => (true, app.update.rx.is_none()),
                        Status::Downloading(..) => (true, !app.update.install_when_ready),
                        _ => (false, false),
                    };
                    if can_install && ok_button(ui, "Install Now", enabled) {
                        install_now(app);
                    }
                    if let Some((r, _)) = announced {
                        if ok_button(ui, "Open Release Page", true) {
                            open_release_page(app, &r.page);
                        }
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
    if let Some(e) = app.update.error.clone() {
        install_failed(app, ctx, &e);
    } else if app.update.prompt && !app.update.dialog {
        prompt(app, ctx);
    }
}

/// The message shown at the start after an update that could not be installed when AnyVector closed
/// (Linux): why, and the page to download it from. A new version found meanwhile is offered after it.
fn install_failed(app: &mut AnyVectorApp, ctx: &egui::Context, e: &InstallError) {
    let mut close = false;
    let ((), dismissed) = window(ctx, "update_failed", "Updates", vec2(460.0, 230.0), |ui| {
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(RichText::new(format!("AnyVector {} could not be installed.", e.version)).strong().size(14.0));
        ui.label(&e.reason);
        ui.label(format!("New version: {}. Your version: {}.", e.version, current_version()));
        ui.label("Click Open Release Page to download it.");
        ui.add_space((ui.available_height() - 34.0).max(0.0));
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("Close").min_size(vec2(80.0, 24.0))).clicked() {
                    close = true;
                }
                if ok_button(ui, "Open Release Page", true) {
                    open_release_page(app, &e.page);
                    close = true;
                }
            });
        });
    });
    if close || dismissed {
        app.update.error = None;
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
    // only announced (Linux): the page to download it from
    let announced = app.update.announced();
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
                match &announced {
                    Some((_, why)) => {
                        ui.label("Click Open Release Page to download it.");
                        note(ui, why);
                    }
                    None => {
                        ui.label("Click OK to install the new version.");
                        note(ui, "AnyVector closes to install it (it asks to save your drawings first) and starts again when it is done.");
                    }
                }
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
                if let Some((r, _)) = &announced {
                    if ok_button(ui, "Open Release Page", true) {
                        open_release_page(app, &r.page);
                    }
                } else if !failed && ok_button(ui, "OK", !installing) {
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
        for b in [include_str!("../../../docs/RELEASE-NOTES-0.3.0.md"), include_str!("../../../docs/RELEASE-NOTES-0.3.1.md"), include_str!("../../../docs/RELEASE-NOTES-0.4.0.md"), include_str!("../../../docs/RELEASE-NOTES-0.4.1.md"), include_str!("../../../docs/RELEASE-NOTES-0.5.0.md"), include_str!("../../../docs/RELEASE-NOTES-0.5.1.md"), include_str!("../../../docs/RELEASE-NOTES-0.5.2.md"), include_str!("../../../docs/RELEASE-NOTES-0.5.3.md"), include_str!("../../../docs/RELEASE-NOTES-0.5.4.md"), include_str!("../../../docs/RELEASE-NOTES-0.5.5.md"), include_str!("../../../docs/RELEASE-NOTES-0.6.0.md"), include_str!("../../../docs/RELEASE-NOTES-0.6.1.md"), include_str!("../../../docs/RELEASE-NOTES-0.6.2.md")].iter().flat_map(|md| note_blocks(md)) {
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

    /// The JSON of a release with a setup, an AppImage (two processors), a .deb and checksums, as the
    /// releases from 0.7.0 on.
    fn release_json() -> serde_json::Value {
        let file = |name: &str, size: u64, digest: serde_json::Value| serde_json::json!({ "name": name, "browser_download_url": format!("https://x/{name}"), "size": size, "digest": digest });
        serde_json::json!({
            "tag_name": "v0.7.0",
            "body": "New tools",
            "html_url": "https://github.com/toim22/AnyVector/releases/tag/v0.7.0",
            "assets": [
                { "name": "notes.txt", "browser_download_url": "https://x/notes.txt" },
                file("anyvector_0.7.0_amd64.deb", 40_000_000, format!("sha256:{}", "d".repeat(64)).into()),
                file("AnyVector-0.7.0-aarch64.AppImage", 60_000_001, format!("sha256:{}", "b".repeat(64)).into()),
                file("AnyVector-0.7.0-x86_64.AppImage", 60_000_000, format!("sha256:{}", "a".repeat(64)).into()),
                file("AnyVector-0.7.0-x86_64.AppImage.zsync", 1000, format!("sha256:{}", "c".repeat(64)).into()),
                file("AnyVector-0.7.0-setup.exe", 17_972_048, serde_json::Value::Null),
                file("AnyVector-0.7.0-setup.exe.sha256", 64, serde_json::Value::Null)
            ]
        })
    }

    /// A writable AppImage.
    fn appimage(writable: bool) -> Install {
        Install::AppImage(AppImage { file: "/home/ana/AnyVector.AppImage".into(), id: (1, 2), extracted: false, writable })
    }

    #[test]
    fn release_json_gives_each_system_its_file() {
        let v = release_json();
        // Windows: the setup, as always, whatever else the release carries
        let r = release_for("windows", "x86_64", &v, false).unwrap();
        assert_eq!((r.version.as_str(), r.tag.as_str(), r.notes.as_str()), ("0.7.0", "v0.7.0", "New tools"));
        assert_eq!((r.asset_name.as_str(), r.asset_url.as_str(), r.asset_size), ("AnyVector-0.7.0-setup.exe", "https://x/AnyVector-0.7.0-setup.exe", 17_972_048));
        assert_eq!((r.digest.as_str(), r.sha256()), ("", None));
        // Linux: the AppImage of the processor
        let r = release_for("linux", "x86_64", &v, true).unwrap();
        assert_eq!((r.asset_name.as_str(), r.asset_url.as_str(), r.asset_size), ("AnyVector-0.7.0-x86_64.AppImage", "https://x/AnyVector-0.7.0-x86_64.AppImage", 60_000_000));
        assert_eq!((r.sha256(), r.via_gh), (Some([0xaa; 32]), true));
        assert_eq!(r.page, "https://github.com/toim22/AnyVector/releases/tag/v0.7.0");
        assert_eq!(release_for("linux", "aarch64", &v, false).unwrap().sha256(), Some([0xbb; 32]));
        let r = release_for("linux", "riscv64", &v, false).unwrap();
        assert_eq!((r.asset_name.as_str(), r.asset_url.as_str(), r.asset_size, r.digest.as_str()), ("", "", 0, ""));
    }

    #[test]
    fn release_json_without_files() {
        let none = serde_json::json!({ "tag_name": "v0.2.0", "assets": [] });
        assert!(release_for("windows", "x86_64", &none, false).is_none(), "Windows installs the setup");
        // Linux: the release is announced, with its page, and nothing to download (never the Windows setup)
        let r = release_for("linux", "x86_64", &none, true).unwrap();
        assert_eq!((r.page.as_str(), r.asset_name.as_str(), r.via_gh), ("https://github.com/toim22/AnyVector/releases", "", true));
        let windows_only = serde_json::json!({ "tag_name": "v0.6.3", "assets": [{ "name": "AnyVector-0.6.3-setup.exe", "browser_download_url": "https://x/s", "size": 5 }] });
        let r = release_for("linux", "x86_64", &windows_only, false).unwrap();
        assert_eq!((r.asset_name.as_str(), r.asset_url.as_str(), r.asset_size), ("", "", 0));
        for install in [appimage(true), Install::Other, Install::Setup] {
            assert!(!install.installs(&r), "{install:?}");
        }
        for v in [serde_json::json!({ "assets": [] }), serde_json::json!({}), serde_json::json!([]), serde_json::json!({ "tag_name": 7 })] {
            assert!(release_for("linux", "x86_64", &v, false).is_none(), "a release needs its tag: {v}");
        }
        // odd values are not believed
        let odd = serde_json::json!({ "tag_name": "v0.7.0", "assets": [{ "name": "AnyVector-0.7.0-x86_64.AppImage", "size": "60", "digest": ["sha256:aa"] }, { "name": 3 }, null] });
        let r = release_for("linux", "x86_64", &odd, false).unwrap();
        assert_eq!((r.asset_name.as_str(), r.asset_size, r.sha256()), ("AnyVector-0.7.0-x86_64.AppImage", 0, None));
        // this system
        assert_eq!(release_from_json(&none, false).is_some(), !cfg!(windows));
        assert!(release_from_json(&release_json(), false).is_some());
    }

    /// AnyVector 0.6.2 is installed on people's computers: it must find its setup in releases that also
    /// carry Linux files, and so must this version.
    #[test]
    fn windows_updaters_take_the_setup_of_a_release_with_linux_files() {
        // as released in 0.6.2 (`release_from_json`)
        let released_0_6_2 = |v: &serde_json::Value| -> Option<(String, String, u64)> {
            let asset = v["assets"].as_array()?.iter().find(|a| a["name"].as_str().is_some_and(|n| n.to_ascii_lowercase().ends_with("-setup.exe")))?;
            Some((asset["name"].as_str()?.to_string(), asset["browser_download_url"].as_str()?.to_string(), asset["size"].as_u64()?))
        };
        let mut v = release_json();
        let files = v["assets"].as_array().unwrap().clone();
        // whatever order GitHub lists them in
        for i in 0..files.len() {
            let mut order = files.clone();
            order.rotate_left(i);
            v["assets"] = order.into();
            let setup = ("AnyVector-0.7.0-setup.exe".to_string(), "https://x/AnyVector-0.7.0-setup.exe".to_string(), 17_972_048);
            assert_eq!(released_0_6_2(&v), Some(setup.clone()));
            let r = release_for("windows", "x86_64", &v, false).unwrap();
            assert_eq!((r.asset_name.clone(), r.asset_url.clone(), r.asset_size), setup);
            assert!(Install::Setup.installs(&r));
        }
    }

    #[test]
    fn the_asset_each_system_installs() {
        let names = ["AnyVector-0.7.0-x86_64.AppImage", "anyvector_0.7.0_amd64.deb", "AnyVector-0.7.0-Setup.exe", "AnyVector-0.7.0-setup.exe.sha256"];
        let of = |os, arch, names: &[&'static str]| asset_for(os, arch, "0.7.0", names.iter().copied());
        for arch in ["x86_64", "aarch64", "x86"] {
            assert_eq!(of("windows", arch, &names), Some("AnyVector-0.7.0-Setup.exe"));
        }
        assert_eq!(of("windows", "x86_64", &["notes.txt", "setup.exe", "AnyVector-0.7.0-x86_64.AppImage", "anyvector_0.7.0_amd64.deb"]), None);
        // Linux: the AppImage for this processor, and never the Windows setup or the package
        assert_eq!(of("linux", "x86_64", &names), Some("AnyVector-0.7.0-x86_64.AppImage"));
        assert_eq!(of("linux", "x86_64", &["anyvector-0.7.0-X86_64.appimage"]), Some("anyvector-0.7.0-X86_64.appimage"));
        assert_eq!(of("linux", "aarch64", &names), None, "another processor's");
        for other in [
            "AnyVector-0.7.0-setup.exe",
            "anyvector_0.7.0_amd64.deb",
            "AnyVector-0.6.9-x86_64.AppImage",
            "AnyVector-x86_64.AppImage",
            "AnyVector-0.7.0-x86_64.AppImage.zsync",
            "AnyVector-0.7.0-x86_64.AppImage-setup.exe",
            "Other-0.7.0-x86_64.AppImage",
            "../AnyVector-0.7.0-x86_64.AppImage",
        ] {
            assert_eq!(of("linux", "x86_64", &[other]), None, "{other}");
        }
        // a version with folders in it makes no file name
        assert_eq!(asset_for("linux", "x86_64", "/../../0.7.0", ["AnyVector-/../../0.7.0-x86_64.AppImage"]), None);
        for os in ["macos", "freebsd"] {
            assert_eq!(of(os, "x86_64", &names), None, "{os}");
        }
    }

    #[test]
    fn what_each_install_does_with_a_release() {
        let arch = std::env::consts::ARCH;
        let release = |asset: &str, digest: &str| Release { version: "9.9.9".into(), tag: "v9.9.9".into(), asset_name: asset.into(), digest: digest.into(), ..Default::default() };
        let sum = format!("sha256:{}", "0".repeat(64));
        let setup = release("AnyVector-9.9.9-setup.exe", "");
        let image = release(&format!("AnyVector-9.9.9-{arch}.AppImage"), &sum);
        // Windows: the setup only (it has no checksum to check, as always)
        assert!(Install::Setup.installs(&setup));
        for r in [&image, &release("", ""), &release("anyvector_9.9.9_amd64.deb", &sum)] {
            assert!(!Install::Setup.installs(r), "{}", r.asset_name);
        }
        // an AppImage: the AppImage only
        assert!(appimage(true).installs(&image));
        for r in [&setup, &release("", &sum), &release("anyvector_9.9.9_amd64.deb", &sum), &release("AnyVector-9.9.9-mips.AppImage", &sum), &release(&format!("AnyVector-9.9.8-{arch}.AppImage"), &sum)] {
            assert_eq!(appimage(true).announced_because(r).as_deref(), Some("This release has no AppImage for this computer."), "{}", r.asset_name);
        }
        // without a checksum it can't be checked
        for bad in ["", "sha256:", "sha256:1234", "sha512:00", &format!("sha256:{}", "g".repeat(64)), &"0".repeat(64)] {
            let r = release(&image.asset_name, bad);
            assert_eq!(appimage(true).announced_because(&r).as_deref(), Some("This release has no checksum to verify its AppImage with."), "{bad:?}");
        }
        assert_eq!(appimage(false).announced_because(&image).as_deref(), Some("You do not have permission to replace /home/ana/AnyVector.AppImage."));
        // a package or a plain program: nothing
        for r in [&setup, &image] {
            assert_eq!(Install::Other.announced_because(r).as_deref(), Some("Only the AppImage of AnyVector installs updates by itself on Linux."));
        }
        // and the note of each update setting says so
        assert_eq!(Install::detect() == Install::Setup, cfg!(windows));
        for by_itself in [Install::Setup, appimage(true)] {
            assert!(by_itself.mode_note(0).starts_with("Automatically downloads"));
        }
        for told in [Install::Other, appimage(false)] {
            assert!(told.mode_note(0).contains("tells you when one is available"));
            assert_eq!((told.mode_note(1), told.mode_note(2), told.mode_note(9)), (Install::Setup.mode_note(1), Install::Setup.mode_note(2), Install::Setup.mode_note(2)));
        }
    }

    #[test]
    fn updates_download_into_the_users_own_folder() {
        let dir = download_dir();
        if cfg!(windows) {
            assert_eq!(dir, std::env::temp_dir().join("AnyVector Updates"));
        } else {
            // not the /tmp every user shares
            assert!(dir.ends_with("AnyVector/Updates"), "{}", dir.display());
            if let Some(cache) = dirs::cache_dir() {
                assert!(dir.starts_with(cache), "{}", dir.display());
            }
        }
        assert!(unusable_release().contains(if cfg!(windows) { "setup for Windows" } else { "No published release" }));
    }
}
