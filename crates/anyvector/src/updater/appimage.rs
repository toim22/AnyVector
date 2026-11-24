//! Updating an AnyVector that runs from its AppImage (Linux): the new AppImage, downloaded and
//! checked against the release, takes the place of the running one's file when AnyVector closes,
//! and is started.
//!
//! An AppImage is a program (the runtime) followed by a file system image. The runtime mounts the
//! image in `$APPDIR` with FUSE, or extracts it there on computers without FUSE, sets `$APPIMAGE` to
//! its own file and starts the program in it. The mounted image is read from the file it was opened
//! from, so a new file renamed over it leaves the running AnyVector as it is.

use super::{no_permission, AppImage, InstallError, Release};
use crate::renderer;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

// ------------------------------------------------------------------ what AnyVector runs from

fn id(m: &Metadata) -> (u64, u64) {
    (m.dev(), m.ino())
}

/// The folder of the AppImage this process runs from: `$APPDIR` without links, when the program is in
/// it. The programs an AppImage starts inherit its variables, so they say nothing by themselves.
pub(super) fn app_dir(appdir: Option<&OsStr>, exe: &Path) -> Option<PathBuf> {
    let dir = Path::new(appdir.filter(|d| Path::new(d).is_absolute())?).canonicalize().ok()?;
    (dir.parent().is_some() && exe.starts_with(&dir)).then_some(dir)
}

/// The AppImage this process runs from, if it does.
pub(super) fn detect() -> Option<AppImage> {
    let runtime = || fs::metadata(format!("/proc/{}/exe", std::os::unix::process::parent_id())).ok().map(|m| id(&m));
    detect_from(std::env::var_os("APPIMAGE").as_deref(), std::env::var_os("APPDIR").as_deref(), &std::env::current_exe().ok()?, is_fuse_mount, runtime)
}

/// [`detect`] with `$APPIMAGE` and `$APPDIR`, the program's path, whether a folder is a FUSE mount and
/// the file the parent process runs. An AppImage when
/// - the program is in `$APPDIR` (an AnyVector started by another program's AppImage inherits that
///   one's `$APPIMAGE` and `$APPDIR`, but is not in it),
/// - `$APPIMAGE` is a regular file with the mark of an AppImage, and
/// - its runtime put the folder there: it is a FUSE mount, or the runtime, which extracted it, is
///   waiting for this process. So an extracted AppDir whose AppRun sets `$APPDIR`, run from another
///   program's AppImage, doesn't count.
fn detect_from(appimage: Option<&OsStr>, appdir: Option<&OsStr>, exe: &Path, mounted: impl Fn(&Path) -> bool, runtime: impl Fn() -> Option<(u64, u64)>) -> Option<AppImage> {
    let dir = app_dir(appdir, exe)?;
    let file = Path::new(appimage.filter(|f| Path::new(f).is_absolute())?).canonicalize().ok()?;
    // an image in its own folder would be another one's
    if file.starts_with(&dir) {
        return None;
    }
    let meta = fs::metadata(&file).ok().filter(Metadata::is_file)?;
    let mut head = [0; HEAD];
    File::open(&file).and_then(|mut f| f.read_exact(&mut head)).ok()?;
    if !is_appimage(&head) {
        return None;
    }
    let extracted = !mounted(&dir);
    if extracted && runtime() != Some(id(&meta)) {
        return None;
    }
    Some(AppImage { writable: writable(&file), id: id(&meta), file, extracted })
}

/// `dir` is the root of a FUSE file system, as the runtime mounts the image.
fn is_fuse_mount(dir: &Path) -> bool {
    const FUSE_SUPER_MAGIC: u64 = 0x6573_5546;
    let dev = |p: &Path| fs::metadata(p).ok().map(|m| m.dev());
    let root = dir.parent().is_some_and(|parent| dev(dir).is_some() && dev(dir) != dev(parent));
    root && rustix::fs::statfs(dir).is_ok_and(|s| s.f_type as u64 == FUSE_SUPER_MAGIC)
}

/// The user can replace `file`: write it, and rename a file over it in its folder. (A folder where
/// only the owner may rename, like /tmp, can still refuse; then the replacement fails and says so.)
pub(super) fn writable(file: &Path) -> bool {
    use rustix::fs::{access, Access};
    access(file, Access::WRITE_OK).is_ok() && file.parent().is_some_and(|d| access(d, Access::WRITE_OK | Access::EXEC_OK).is_ok())
}

// ------------------------------------------------------------------ the downloaded file

/// The bytes of an ELF header [`is_appimage`] reads.
const HEAD: usize = 20;

/// The header of an AppImage of type 2 for this processor: a 64-bit little-endian ELF program with
/// "AI" and the type in the padding of its identification.
fn is_appimage(head: &[u8; HEAD]) -> bool {
    let machine = match std::env::consts::ARCH {
        "x86_64" => 62,
        "aarch64" => 183,
        _ => return false,
    };
    head.starts_with(b"\x7fELF\x02\x01") && &head[8..11] == b"AI\x02" && u16::from_le_bytes([head[18], head[19]]) == machine
}

const NOT_APPIMAGE: &str = "The downloaded file is not an AppImage for this computer.";
const BAD_CHECKSUM: &str = "The downloaded file does not match the checksum of the release. Please try again.";
const INTERRUPTED: &str = "The download was interrupted. Please try again.";

/// Check that `file` is the AppImage of release `r`: its size and checksum, and that it is an
/// AppImage. The text of the error is shown.
pub(super) fn verify(file: &Path, r: &Release) -> Result<(), String> {
    copy_checked(file, r, |_| Ok(()))
}

/// [`verify`] `file` while its bytes go to `write`: what was checked is what was written, whatever
/// happens to the file meanwhile. An error of `write` is returned as it is.
fn copy_checked(file: &Path, r: &Release, mut write: impl FnMut(&[u8]) -> Result<(), String>) -> Result<(), String> {
    let unreadable = |e: io::Error| format!("Could not read {}: {e}", file.display());
    let mut f = File::open(file).map_err(unreadable)?;
    let size = f.metadata().map_err(unreadable)?.len();
    // before reading it all: a download that stopped halfway
    if size == 0 || (r.asset_size > 0 && size != r.asset_size) {
        return Err(INTERRUPTED.into());
    }
    let (mut hash, mut buf, mut done) = (Sha256::new(), vec![0u8; 1 << 17], 0u64);
    loop {
        // fill the buffer, so the header is whole in the first one
        let mut n = 0;
        while n < buf.len() {
            match f.read(&mut buf[n..]) {
                Ok(0) => break,
                Ok(m) => n += m,
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(unreadable(e)),
            }
        }
        if n == 0 {
            break;
        }
        if done == 0 && !buf[..n].first_chunk::<HEAD>().is_some_and(is_appimage) {
            return Err(NOT_APPIMAGE.into());
        }
        hash.update(&buf[..n]);
        write(&buf[..n])?;
        done += n as u64;
    }
    if done != size {
        return Err(INTERRUPTED.into());
    }
    let sum: [u8; 32] = hash.finalize().into();
    if r.sha256() != Some(sum) {
        return Err(BAD_CHECKSUM.into());
    }
    Ok(())
}

// ------------------------------------------------------------------ replacing the AppImage

/// What the new file is called until it takes the AppImage's place, before the process id.
const NEW_PREFIX: &str = ".anyvector-update-";

/// Removes the new file unless it took the AppImage's place.
struct NewFile(Option<PathBuf>);

impl Drop for NewFile {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            let _ = fs::remove_file(p);
        }
    }
}

/// Put the downloaded AppImage `new` in the place of `a`: checked again while it is copied next to
/// it, then renamed over it, so the file is always one of the two, whole. The name stays the user's
/// own (a link to it keeps leading to it) and so do the permissions, plus execution for whoever can
/// read it. On an error the old file is as it was.
fn replace(a: &AppImage, new: &Path, r: &Release) -> Result<(), String> {
    let moved = || format!("{} was moved or changed while AnyVector was running.", a.file.display());
    let refused = |e: io::Error| match e.kind() {
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem => no_permission(&a.file),
        _ => format!("Could not write {}: {e}", a.file.display()),
    };
    let old = fs::symlink_metadata(&a.file).map_err(|_| moved())?;
    if !old.is_file() || id(&old) != a.id {
        // another AnyVector that was open installed it already
        return if old.is_file() && verify(&a.file, r).is_ok() { Ok(()) } else { Err(moved()) };
    }
    if !writable(&a.file) {
        return Err(no_permission(&a.file));
    }
    let dir = a.file.parent().ok_or_else(moved)?;
    remove_leftovers(dir);
    let path = dir.join(format!("{NEW_PREFIX}{}", std::process::id()));
    let _ = fs::remove_file(&path);
    let mut f = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).map_err(refused)?;
    let mut new_file = NewFile(Some(path.clone()));
    copy_checked(new, r, |bytes| f.write_all(bytes).map_err(refused))?;
    // the group of a folder shared by several users (only root can give a file to another owner)
    if f.metadata().is_ok_and(|m| m.gid() != old.gid()) {
        let _ = std::os::unix::fs::fchown(&f, None, Some(old.gid()));
    }
    let mode = old.mode() & 0o777;
    let mode = mode | ((mode & 0o444) >> 2) | 0o500;
    f.set_permissions(fs::Permissions::from_mode(mode)).map_err(refused)?;
    f.sync_all().map_err(refused)?;
    drop(f);
    fs::rename(&path, &a.file).map_err(refused)?;
    new_file.0 = None;
    // the rename reaches the disk too; the file is whole either way
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// Remove the new files an update that was cut short (power, kill) left in `dir`.
fn remove_leftovers(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten().filter(|e| e.file_name().as_bytes().starts_with(NEW_PREFIX.as_bytes())) {
        let old = e.metadata().ok().filter(Metadata::is_file).and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok());
        if old.is_some_and(|age| age > Duration::from_secs(3600)) {
            let _ = fs::remove_file(e.path());
        }
    }
}

// ------------------------------------------------------------------ installing on exit

/// Install release `r` (downloaded to `new`) and start AnyVector again. When that fails, the
/// AnyVector the user has is started again instead and says why (`dir` keeps the message: see
/// [`take_error`]).
pub(super) fn install(a: &AppImage, r: &Release, new: &Path, dir: &Path) {
    match replace(a, new, r) {
        Ok(()) => {
            let _ = fs::remove_file(new);
        }
        Err(reason) => {
            eprintln!("AnyVector: {} could not be installed: {reason}", r.version);
            keep_error(dir, &InstallError { version: r.version.clone(), page: r.page.clone(), reason });
            // not a file of which nothing is known
            if !fs::symlink_metadata(&a.file).is_ok_and(|m| m.is_file() && id(&m) == a.id) {
                return;
            }
        }
    }
    // tests start it themselves, waiting for another process than the test program
    if !cfg!(test) {
        close_fds_on_exec();
        // the folder of an extracted AppImage is deleted by the runtime, which waits for this process
        let wait_for = if a.extracted { std::os::unix::process::parent_id() } else { std::process::id() };
        if let Err(e) = restart_command(a, std::env::vars_os(), wait_for).spawn() {
            eprintln!("AnyVector: can't start again ({e})");
        }
    }
}

/// Starts the AppImage `a` again once the process `wait_for` has ended (at most 10 seconds later), as
/// the Windows setup does: without this process' terminal and session, and with the environment
/// `vars` as it was before the AppImage started.
fn restart_command(a: &AppImage, vars: impl IntoIterator<Item = (OsString, OsString)>, wait_for: u32) -> Command {
    // /proc/<id>/exe of a process that ended but was not waited for is unreadable
    const WAIT: &str = r#"n=0; while [ -e "/proc/$1/exe" ] && [ $n -lt 100 ]; do sleep 0.1; n=$((n+1)); done; exec "$2""#;
    let mut vars: Vec<(OsString, OsString)> = vars.into_iter().collect();
    let var = |name: &str| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
    let appdir = var("APPDIR").map(PathBuf::from);
    let owd = var("OWD").map(PathBuf::from).filter(|d| d.is_absolute() && d.is_dir());
    clean_environment(&mut vars, a.extracted);
    let mut c = Command::new("/bin/sh");
    c.args(["-c", WAIT, "anyvector-update"]).arg(wait_for.to_string()).arg(&a.file);
    c.env_clear().envs(vars).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // a working folder in the old AppImage would keep it mounted (or be deleted)
    if let (Ok(cwd), Some(appdir)) = (std::env::current_dir(), appdir) {
        if cwd.starts_with(&appdir) || appdir.canonicalize().is_ok_and(|d| cwd.starts_with(d)) {
            c.current_dir(owd.or_else(dirs::home_dir).unwrap_or_else(|| "/".into()));
        }
    }
    // SAFETY: setsid is async-signal-safe and allocates nothing
    unsafe {
        c.pre_exec(|| rustix::process::setsid().map(drop).map_err(io::Error::from));
    }
    c
}

/// The environment of the AppImage without what its runtime, its AppRun and AnyVector put there: the
/// new one sets its own.
fn clean_environment(vars: &mut Vec<(OsString, OsString)>, extracted: bool) {
    const SET_BY_RUNTIME: [&str; 5] = ["APPIMAGE", "APPDIR", "ARGV0", "OWD", "TARGET_APPIMAGE"];
    // an activation token is good for one start
    const STARTUP: [&str; 2] = ["XDG_ACTIVATION_TOKEN", "DESKTOP_STARTUP_ID"];
    let appdirs: Vec<PathBuf> = match vars.iter().find(|(k, _)| k == "APPDIR") {
        Some((_, d)) if Path::new(d).is_absolute() && Path::new(d).parent().is_some() => {
            let d = PathBuf::from(d);
            d.canonicalize().into_iter().chain([d]).collect()
        }
        _ => vec![],
    };
    let fell_back = vars.iter().any(|(k, _)| k == renderer::FALLBACK_ENV);
    vars.retain_mut(|(k, v)| {
        if SET_BY_RUNTIME.iter().chain(&STARTUP).any(|n| k == n) {
            return false;
        }
        // OpenGL chosen by AnyVector itself: the new version tries wgpu first; the user's own choice stays
        if k == renderer::FALLBACK_ENV || (fell_back && k == renderer::ENV) {
            return false;
        }
        // folders inside the AppImage (LD_LIBRARY_PATH, PATH, XDG_DATA_DIRS... of an AppRun)
        let inside = |p: &[u8]| appdirs.iter().any(|d| Path::new(OsStr::from_bytes(p)).starts_with(d));
        let parts: Vec<&[u8]> = v.as_bytes().split(|b| *b == b':').collect();
        if parts.iter().any(|p| inside(p)) {
            let kept = parts.into_iter().filter(|p| !inside(p)).collect::<Vec<_>>().join(&b':');
            if kept.iter().all(|b| *b == b':') {
                return false;
            }
            *v = OsString::from_vec(kept);
        }
        true
    });
    // it was extracted because it couldn't be mounted (the variable, or an argument that is gone now)
    if extracted {
        vars.retain(|(k, _)| k != "APPIMAGE_EXTRACT_AND_RUN");
        vars.push(("APPIMAGE_EXTRACT_AND_RUN".into(), "1".into()));
    }
}

/// Let none of the files this process has open reach the programs it starts from now on: the runtime
/// keeps the old AppImage mounted as long as a process holds its pipe.
fn close_fds_on_exec() {
    let Ok(dir) = fs::read_dir("/proc/self/fd") else { return };
    let fds: Vec<i32> = dir.flatten().filter_map(|e| e.file_name().to_str()?.parse().ok()).filter(|fd| *fd > 2).collect();
    for fd in fds {
        // SAFETY: only its flag is set, and a descriptor closed meanwhile is an error that is ignored
        let fd = unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) };
        let _ = rustix::io::fcntl_setfd(fd, rustix::io::FdFlags::CLOEXEC);
    }
}

// ------------------------------------------------------------------ telling the user

fn error_file(dir: &Path) -> PathBuf {
    dir.join("install-error.json")
}

/// Keep the error for the next start (AnyVector's window is gone).
fn keep_error(dir: &Path, e: &InstallError) {
    let text = json!({ "version": e.version, "page": e.page, "reason": e.reason }).to_string();
    if let Err(e) = fs::create_dir_all(dir).and_then(|()| fs::write(error_file(dir), text)) {
        eprintln!("AnyVector: can't keep the message ({e})");
    }
}

/// The error kept when AnyVector last closed, once. Nothing when the version is installed by now.
pub(super) fn take_error(dir: &Path) -> Option<InstallError> {
    let file = error_file(dir);
    let mut text = String::new();
    File::open(&file).ok()?.take(1 << 16).read_to_string(&mut text).ok();
    let _ = fs::remove_file(&file);
    let v: Value = serde_json::from_str(&text).ok()?;
    let field = |k: &str| v[k].as_str().map(str::to_string);
    let e = InstallError { version: field("version")?, page: field("page")?, reason: field("reason")? };
    let ours = e.page.starts_with("https://github.com/");
    (ours && super::is_newer(&e.version, super::current_version())).then_some(e)
}

#[cfg(test)]
mod tests;
