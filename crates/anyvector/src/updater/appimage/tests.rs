use super::*;
use crate::app::AnyVectorApp;
use crate::updater::{fetch, start_check_at, Install, Status, KEEP_UP_TO_DATE};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use std::net::TcpListener;
use std::os::unix::fs::symlink;
use std::time::Instant;

/// An empty folder for `test`, without links in its path.
fn folder(test: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("anyvector-appimage-{}-{test}", std::process::id()));
    let _ = fs::set_permissions(&d, fs::Permissions::from_mode(0o755));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

/// The bytes of a program that runs (it prints its environment) with the mark of an AppImage, and
/// `version` after it, as the image comes after the runtime.
fn program(version: &str) -> Vec<u8> {
    let mut b = fs::read("/usr/bin/env").expect("the env program");
    b[8..11].copy_from_slice(b"AI\x02");
    b.extend_from_slice(version.as_bytes());
    b
}

fn put(file: &Path, bytes: &[u8], mode: u32) {
    fs::write(file, bytes).unwrap();
    fs::set_permissions(file, fs::Permissions::from_mode(mode)).unwrap();
}

fn sha256(bytes: &[u8]) -> String {
    let sum: [u8; 32] = Sha256::digest(bytes).into();
    format!("sha256:{}", sum.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

/// The release 99.0.0 whose AppImage has these bytes.
fn release(bytes: &[u8]) -> Release {
    Release {
        version: "99.0.0".into(),
        tag: "v99.0.0".into(),
        page: "https://github.com/toim22/AnyVector/releases/tag/v99.0.0".into(),
        asset_name: format!("AnyVector-99.0.0-{}.AppImage", std::env::consts::ARCH),
        asset_size: bytes.len() as u64,
        digest: sha256(bytes),
        ..Default::default()
    }
}

/// The AppImage in `file`, as if AnyVector ran from it.
fn running(file: &Path) -> AppImage {
    let file = file.canonicalize().unwrap();
    AppImage { id: id(&fs::metadata(&file).unwrap()), extracted: false, writable: writable(&file), file }
}

fn mode(file: &Path) -> u32 {
    fs::metadata(file).unwrap().mode() & 0o7777
}

fn names(dir: &Path) -> Vec<String> {
    let mut n: Vec<String> = fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    n.sort();
    n
}

/// Root may write everywhere: what can't be written is not tested then.
fn is_root() -> bool {
    rustix::process::geteuid().is_root()
}

// ------------------------------------------------------------------ what AnyVector runs from

#[test]
fn an_appimage_is_where_its_runtime_started_anyvector() {
    let d = folder("detect");
    let file = d.join("AnyVector.AppImage");
    put(&file, &program("old"), 0o755);
    let appdir = d.join("mount");
    let exe = appdir.join("usr/bin/anyvector");
    fs::create_dir_all(exe.parent().unwrap()).unwrap();
    fs::write(&exe, "").unwrap();
    let me = id(&fs::metadata(&file).unwrap());
    let detect = |appimage: &Path, appdir: &Path, exe: &Path, mounted: bool, runtime: Option<(u64, u64)>| detect_from(Some(appimage.as_os_str()), Some(appdir.as_os_str()), exe, |_| mounted, || runtime);
    let found = AppImage { file: file.clone(), id: me, extracted: false, writable: true };
    // mounted: whatever the parent process is
    assert_eq!(detect(&file, &appdir, &exe, true, None), Some(found.clone()));
    // extracted: the runtime is the parent process
    assert_eq!(detect(&file, &appdir, &exe, false, Some(me)), Some(AppImage { extracted: true, ..found.clone() }));
    // through links: the file itself is the one replaced
    let link = d.join("link.AppImage");
    symlink(&file, &link).unwrap();
    let linkdir = d.join("linkdir");
    symlink(&appdir, &linkdir).unwrap();
    assert_eq!(detect(&link, &linkdir, &exe, true, None), Some(found));

    // started by another program's AppImage: its variables, but not its folder
    let installed = d.join("usr/bin/anyvector");
    fs::create_dir_all(installed.parent().unwrap()).unwrap();
    fs::write(&installed, "").unwrap();
    assert_eq!(detect(&file, &appdir, &installed, true, Some(me)), None, "a package's program");
    assert_eq!(detect(&file, &d.join("mou"), &exe, true, Some(me)), None, "a folder whose name starts the same");
    // an AppDir extracted by hand: neither mounted nor started by that runtime
    assert_eq!(detect(&file, &appdir, &exe, false, None), None);
    assert_eq!(detect(&file, &appdir, &exe, false, Some((me.0, me.1 + 1))), None);
    // $APPIMAGE is no AppImage, or not a file
    let plain = d.join("plain");
    put(&plain, &fs::read("/usr/bin/env").unwrap(), 0o755);
    let script = d.join("script");
    put(&script, b"#!/bin/sh\n", 0o755);
    let pipe = Path::new("/dev/null");
    for not in [plain.as_path(), &script, &appdir, &d.join("gone.AppImage"), Path::new("AnyVector.AppImage"), Path::new(""), pipe] {
        assert_eq!(detect(not, &appdir, &exe, true, Some(me)), None, "{}", not.display());
    }
    // inside the image itself
    let inside = appdir.join("inner.AppImage");
    put(&inside, &program("x"), 0o755);
    assert_eq!(detect(&inside, &appdir, &exe, true, Some(me)), None);
    // no or odd folders
    for dir in ["", "mount", "/", "/nowhere"] {
        assert_eq!(detect(&file, Path::new(dir), &exe, true, Some(me)), None, "{dir:?}");
    }
    assert_eq!(detect_from(None, Some(appdir.as_os_str()), &exe, |_| true, || Some(me)), None);
    assert_eq!(detect_from(Some(file.as_os_str()), None, &exe, |_| true, || Some(me)), None);
    // no FUSE mounts: plain folders, the root, mounts of another kind
    for dir in [appdir.as_path(), Path::new("/"), Path::new("/proc"), Path::new("/nowhere")] {
        assert!(!is_fuse_mount(dir), "{}", dir.display());
    }
    // the test program is not in an AppImage
    assert_eq!(super::detect(), None);
    assert_eq!(Install::detect(), Install::Other);
    fs::remove_dir_all(d).unwrap();
}

#[test]
fn a_read_only_appimage_or_folder_is_not_writable() {
    let d = folder("writable");
    let file = d.join("AnyVector.AppImage");
    put(&file, &program("old"), 0o755);
    assert!(writable(&file));
    assert!(!writable(&d.join("gone")));
    // the program of a running process can be replaced (it only can't be written in place)
    assert!(writable(&std::env::current_exe().unwrap()));
    if !is_root() {
        put(&file, &program("old"), 0o555);
        assert!(!writable(&file), "the file");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&d, fs::Permissions::from_mode(0o555)).unwrap();
        assert!(!writable(&file), "the folder");
        fs::set_permissions(&d, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!writable(Path::new("/usr/bin/env")), "root's file");
    }
    fs::remove_dir_all(d).unwrap();
}

// ------------------------------------------------------------------ the downloaded file

#[test]
fn only_the_releases_appimage_is_accepted() {
    let d = folder("verify");
    let file = d.join("new");
    let good = program("new");
    let r = release(&good);
    let check = |bytes: &[u8], r: &Release| {
        fs::write(&file, bytes).unwrap();
        verify(&file, r)
    };
    assert_eq!(check(&good, &r), Ok(()));
    // the size is not always known
    assert_eq!(check(&good, &Release { asset_size: 0, ..r.clone() }), Ok(()));
    let upper = format!("sha256:{}", r.digest["sha256:".len()..].to_uppercase());
    assert_eq!(check(&good, &Release { digest: upper, ..r.clone() }), Ok(()));
    // one byte changed, same size
    let mut changed = good.clone();
    *changed.last_mut().unwrap() ^= 1;
    assert_eq!(check(&changed, &r), Err(BAD_CHECKSUM.into()));
    // no checksum, or one that isn't
    for digest in ["", "sha256:", "sha256:xyz", "md5:d41d8cd98f00b204e9800998ecf8427e"] {
        assert_eq!(check(&good, &Release { digest: digest.into(), ..r.clone() }), Err(BAD_CHECKSUM.into()), "{digest:?}");
    }
    // cut short, longer, empty
    assert_eq!(check(&good[..good.len() - 1], &r), Err(INTERRUPTED.into()));
    assert_eq!(check(&[&good[..], b"x"].concat(), &r), Err(INTERRUPTED.into()));
    assert_eq!(check(b"", &Release { asset_size: 0, ..r.clone() }), Err(INTERRUPTED.into()));
    assert_eq!(check(&good[..good.len() - 1], &Release { asset_size: 0, ..r.clone() }), Err(BAD_CHECKSUM.into()));
    // not an AppImage for this computer, even with its own checksum
    let with = |at: usize, bytes: &[u8]| {
        let mut b = good.clone();
        b[at..at + bytes.len()].copy_from_slice(bytes);
        b
    };
    let other_machine: &[u8] = if cfg!(target_arch = "x86_64") { &[183, 0] } else { &[62, 0] };
    for (what, bytes) in [
        ("a program", with(8, &[0, 0, 0])),
        ("type 1", with(10, &[1])),
        ("no ELF", with(0, b"MZ\0\0")),
        ("32 bits", with(4, &[1])),
        ("another processor", with(18, other_machine)),
        ("a web page", b"<!DOCTYPE html><html><body>Not found</body></html>".to_vec()),
        ("a few bytes", good[..HEAD - 1].to_vec()),
    ] {
        assert_eq!(check(&bytes, &release(&bytes)), Err(NOT_APPIMAGE.into()), "{what}");
    }
    assert!(verify(&d.join("gone"), &r).unwrap_err().starts_with("Could not read "));
    assert!(verify(&d, &r).is_err(), "a folder");
    fs::remove_dir_all(d).unwrap();
}

// ------------------------------------------------------------------ replacing

#[test]
fn the_new_appimage_takes_the_place_of_the_old_one() {
    let d = folder("replace");
    let new = d.join("download");
    fs::write(&new, program("new")).unwrap();
    // the permissions stay, with execution for whoever reads; setuid and the like don't
    for (before, after) in [(0o755, 0o755), (0o700, 0o700), (0o744, 0o755), (0o640, 0o750), (0o775, 0o775), (0o4755, 0o755), (0o600, 0o700)] {
        let apps = d.join(format!("apps {before:o}"));
        fs::create_dir(&apps).unwrap();
        // the user's own name, not the release's
        let file = apps.join("Meu AnyVector é ótimo.AppImage");
        put(&file, &program("old"), before);
        let a = running(&file);
        let mut open = File::open(&file).unwrap();
        assert_eq!(replace(&a, &new, &release(&program("new"))), Ok(()));
        assert_eq!(fs::read(&file).unwrap(), program("new"));
        assert_eq!(mode(&file), after, "{before:o}");
        assert_eq!(names(&apps), ["Meu AnyVector é ótimo.AppImage"], "nothing left next to it");
        // the running image goes on reading the old file
        assert_ne!(id(&fs::metadata(&file).unwrap()), a.id);
        let mut old = vec![];
        open.read_to_end(&mut old).unwrap();
        assert_eq!(old, program("old"));
    }
    assert!(new.exists(), "the download is for the caller to remove");
    fs::remove_dir_all(d).unwrap();
}

#[test]
fn a_linked_appimage_is_replaced_where_it_is() {
    let d = folder("links");
    let new = d.join("download");
    fs::write(&new, program("new")).unwrap();
    for n in ["apps", "bin"] {
        fs::create_dir(d.join(n)).unwrap();
    }
    let file = d.join("apps/AnyVector-0.7.0.AppImage");
    put(&file, &program("old"), 0o755);
    symlink("../apps/AnyVector-0.7.0.AppImage", d.join("bin/second")).unwrap();
    symlink("second", d.join("bin/anyvector")).unwrap();
    let a = running(&d.join("bin/anyvector"));
    assert_eq!(a.file, file);
    assert_eq!(replace(&a, &new, &release(&program("new"))), Ok(()));
    for l in ["bin/anyvector", "bin/second"] {
        assert!(fs::symlink_metadata(d.join(l)).unwrap().is_symlink(), "{l} is still a link");
        assert_eq!(fs::read(d.join(l)).unwrap(), program("new"));
    }
    assert!(fs::symlink_metadata(&file).unwrap().is_file());
    assert_eq!((names(&d.join("apps")).len(), names(&d.join("bin")).len()), (1, 2));
    fs::remove_dir_all(d).unwrap();
}

#[test]
fn a_failure_leaves_the_old_appimage() {
    let d = folder("failures");
    let apps = d.join("apps");
    fs::create_dir(&apps).unwrap();
    let file = apps.join("AnyVector.AppImage");
    put(&file, &program("old"), 0o750);
    let a = running(&file);
    let good = program("new");
    let r = release(&good);
    let new = d.join("download");
    let intact = |what: &str| {
        assert_eq!(fs::read(&file).unwrap(), program("old"), "{what}");
        assert_eq!((mode(&file), id(&fs::metadata(&file).unwrap())), (0o750, a.id), "{what}");
        assert_eq!(names(&apps), ["AnyVector.AppImage"], "{what}");
    };
    // the download changed since it was checked
    let mut changed = good.clone();
    changed[good.len() - 1] ^= 1;
    fs::write(&new, &changed).unwrap();
    assert_eq!(replace(&a, &new, &r), Err(BAD_CHECKSUM.into()));
    intact("checksum");
    fs::write(&new, &good[..1000]).unwrap();
    assert_eq!(replace(&a, &new, &r), Err(INTERRUPTED.into()));
    intact("cut short");
    let html = b"<html>".repeat(100);
    fs::write(&new, &html).unwrap();
    assert_eq!(replace(&a, &new, &release(&html)), Err(NOT_APPIMAGE.into()));
    intact("not an AppImage");
    fs::remove_file(&new).unwrap();
    assert!(replace(&a, &new, &r).unwrap_err().starts_with("Could not read "));
    intact("no download");
    fs::write(&new, &good).unwrap();
    if !is_root() {
        let refused = format!("You do not have permission to replace {}.", file.display());
        fs::set_permissions(&apps, fs::Permissions::from_mode(0o555)).unwrap();
        assert_eq!(replace(&a, &new, &r), Err(refused.clone()));
        fs::set_permissions(&apps, fs::Permissions::from_mode(0o755)).unwrap();
        intact("folder");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o550)).unwrap();
        assert_eq!(replace(&a, &new, &r), Err(refused));
        fs::set_permissions(&file, fs::Permissions::from_mode(0o750)).unwrap();
        intact("file");
    }
    // another file in its place (a program, a link, another AppImage) is not touched
    let moved = format!("{} was moved or changed while AnyVector was running.", file.display());
    fs::rename(&file, d.join("elsewhere")).unwrap();
    assert_eq!(replace(&a, &new, &r), Err(moved.clone()));
    assert!(names(&apps).is_empty());
    put(&file, &program("another"), 0o755);
    assert_eq!(replace(&a, &new, &r), Err(moved.clone()));
    assert_eq!(fs::read(&file).unwrap(), program("another"));
    fs::remove_file(&file).unwrap();
    symlink(d.join("elsewhere"), &file).unwrap();
    assert_eq!(replace(&a, &new, &r), Err(moved));
    assert_eq!(fs::read(d.join("elsewhere")).unwrap(), program("old"));
    // unless another AnyVector that was open has already installed this release
    fs::remove_file(&file).unwrap();
    put(&file, &good, 0o755);
    let theirs = id(&fs::metadata(&file).unwrap());
    assert_eq!(replace(&a, &new, &r), Ok(()));
    assert_eq!(id(&fs::metadata(&file).unwrap()), theirs);
    fs::remove_dir_all(d).unwrap();
}

/// The disk fills while the new file is written: what was written of it goes away.
#[test]
fn a_copy_that_stops_halfway_is_removed() {
    let d = folder("halfway");
    let file = d.join("AnyVector.AppImage");
    put(&file, &program("old"), 0o755);
    let new = d.join("download");
    let good = program("new").repeat(8);
    fs::write(&new, &good).unwrap();
    let path = d.join(format!("{NEW_PREFIX}test"));
    let mut f = File::create(&path).unwrap();
    let guard = NewFile(Some(path));
    let mut blocks = 0;
    let e = copy_checked(&new, &release(&good), |b| {
        blocks += 1;
        f.write_all(b).unwrap();
        if blocks == 2 { Err("No space left on device (os error 28)".into()) } else { Ok(()) }
    });
    assert_eq!(e, Err("No space left on device (os error 28)".into()));
    assert_eq!(names(&d).len(), 3);
    drop(guard);
    assert_eq!(names(&d), ["AnyVector.AppImage", "download"]);
    assert_eq!(fs::read(&file).unwrap(), program("old"));
    fs::remove_dir_all(d).unwrap();
}

#[test]
fn leftovers_of_an_update_cut_short_are_removed() {
    let d = folder("leftovers");
    let file = d.join("AnyVector.AppImage");
    put(&file, &program("old"), 0o755);
    let new = d.join("download");
    fs::write(&new, program("new")).unwrap();
    let put_old = |name: &str, hours: u64| {
        let f = File::create(d.join(name)).unwrap();
        f.set_modified(std::time::SystemTime::now() - Duration::from_secs(hours * 3600)).unwrap();
    };
    put_old(&format!("{NEW_PREFIX}1"), 5);
    put_old(&format!("{NEW_PREFIX}2"), 0); // another AnyVector may be writing it
    put_old(&format!("{NEW_PREFIX}{}", std::process::id()), 0); // this process' number, used before
    put_old("drawing.cdr", 5);
    assert_eq!(replace(&running(&file), &new, &release(&program("new"))), Ok(()));
    assert_eq!(names(&d), [format!("{NEW_PREFIX}2"), "AnyVector.AppImage".into(), "download".into(), "drawing.cdr".into()]);
    fs::remove_dir_all(d).unwrap();
}

// ------------------------------------------------------------------ starting again

#[test]
fn the_new_appimage_starts_with_the_environment_before_the_old_one() {
    let vars = |list: &[(&str, &str)], extracted: bool| {
        let mut v: Vec<(OsString, OsString)> = list.iter().map(|(k, v)| (k.into(), v.into())).collect();
        clean_environment(&mut v, extracted);
        v.into_iter().map(|(k, v)| (k.into_string().unwrap(), v.into_string().unwrap())).collect::<Vec<_>>()
    };
    let own = |list: &[(&str, &str)]| list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>();
    let m = "/tmp/.mount_AnyVecABC";
    let inside = |s: &str| s.replace("@", m);
    let path = inside("@/usr/bin:/usr/local/bin:/usr/bin");
    let libs = inside("@/usr/lib:@/usr/lib/x86_64:");
    let data = inside("/usr/local/share:@/usr/share:/usr/share");
    let schemas = inside("@/usr/share/glib-2.0/schemas");
    let before = [
        ("HOME", "/home/ana"),
        ("APPIMAGE", "/home/ana/AnyVector.AppImage"),
        ("APPDIR", m),
        ("ARGV0", "./AnyVector.AppImage"),
        ("OWD", "/home/ana"),
        ("TARGET_APPIMAGE", "/home/ana/AnyVector.AppImage"),
        ("PATH", &path),
        ("LD_LIBRARY_PATH", &libs),
        ("XDG_DATA_DIRS", &data),
        ("GSETTINGS_SCHEMA_DIR", &schemas),
        ("XDG_ACTIVATION_TOKEN", "t"),
        ("DESKTOP_STARTUP_ID", "t"),
        ("WAYLAND_DISPLAY", "wayland-0"),
        ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1000/bus"),
        ("OTHER", "/tmp/.mount_AnyVecABCD/usr:/x"),
        ("APPIMAGE_EXTRACT_AND_RUN", "1"),
    ];
    let after = own(&[
        ("HOME", "/home/ana"),
        ("PATH", "/usr/local/bin:/usr/bin"),
        ("XDG_DATA_DIRS", "/usr/local/share:/usr/share"),
        ("WAYLAND_DISPLAY", "wayland-0"),
        ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1000/bus"),
        ("OTHER", "/tmp/.mount_AnyVecABCD/usr:/x"),
        ("APPIMAGE_EXTRACT_AND_RUN", "1"),
    ]);
    assert_eq!(vars(&before, false), after);
    // an extracted AppImage is extracted again, whichever way it was asked for
    assert_eq!(vars(&before, true), after);
    assert_eq!(vars(&before[..1], true), own(&[("HOME", "/home/ana"), ("APPIMAGE_EXTRACT_AND_RUN", "1")]));
    assert_eq!(vars(&before[..1], false), own(&[("HOME", "/home/ana")]));
    // OpenGL: AnyVector's own choice is tried anew, the user's stays
    assert_eq!(vars(&[(renderer::ENV, "gl"), (renderer::FALLBACK_ENV, "1")], false), []);
    assert_eq!(vars(&[(renderer::FALLBACK_ENV, "1"), (renderer::ENV, "gl")], false), []);
    assert_eq!(vars(&[(renderer::ENV, "gl")], false), own(&[(renderer::ENV, "gl")]));
    // a folder that would match everything
    assert_eq!(vars(&[("APPDIR", "/"), ("PATH", "/usr/bin")], false), own(&[("PATH", "/usr/bin")]));
}

#[test]
fn the_new_appimage_starts_detached_when_the_old_one_has_ended() {
    let d = folder("restart");
    let file = d.join("My AnyVector.AppImage");
    put(&file, &program("new"), 0o755);
    let out = d.join("started");
    let mut old = Command::new("sleep").arg("30").spawn().unwrap();
    let a = running(&file);
    let vars = [("APPIMAGE", file.as_os_str()), ("APPDIR", d.as_os_str()), ("LEFT", OsStr::new("yes")), ("PATH", OsStr::new("/usr/bin:/bin"))];
    let vars = vars.map(|(k, v)| (OsString::from(k), v.to_os_string()));
    let mut c = restart_command(&a, vars, old.id());
    let mut started = c.stdout(File::create(&out).unwrap()).spawn().unwrap();
    // it waits for the old one, in a session of its own
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(fs::read_to_string(&out).unwrap(), "");
    assert!(started.try_wait().unwrap().is_none());
    let stat = fs::read_to_string(format!("/proc/{}/stat", started.id())).unwrap();
    let after_name: Vec<&str> = stat.rsplit_once(')').unwrap().1.split_whitespace().collect();
    assert_eq!((after_name[2], after_name[3]), (started.id().to_string().as_str(), started.id().to_string().as_str()), "process group and session");
    // a process that ended counts, also before anyone waits for it
    old.kill().unwrap();
    let t = Instant::now();
    assert!(started.wait().unwrap().success());
    assert!(t.elapsed() < Duration::from_secs(8));
    let env = fs::read_to_string(&out).unwrap();
    assert!(env.lines().any(|l| l == "LEFT=yes"), "{env}");
    assert!(!env.contains("APPIMAGE") && !env.contains("APPDIR"), "{env}");
    old.wait().unwrap();
    fs::remove_dir_all(d).unwrap();
}

#[test]
fn open_files_do_not_reach_the_new_appimage() {
    // as the runtime's pipe: inherited, without close-on-exec
    let d = folder("fds");
    let f = File::create(d.join("pipe")).unwrap();
    rustix::io::fcntl_setfd(&f, rustix::io::FdFlags::empty()).unwrap();
    close_fds_on_exec();
    assert_eq!(rustix::io::fcntl_getfd(&f).unwrap(), rustix::io::FdFlags::CLOEXEC);
    fs::remove_dir_all(d).unwrap();
}

// ------------------------------------------------------------------ telling the user

#[test]
fn the_error_is_told_once_at_the_next_start() {
    let d = folder("error").join("Updates");
    assert_eq!(take_error(&d), None);
    let e = InstallError { version: "99.0.0".into(), page: "https://github.com/toim22/AnyVector/releases/tag/v99.0.0".into(), reason: "You do not have permission to replace /a \"b\"\n.".into() };
    keep_error(&d, &e);
    assert_eq!(take_error(&d), Some(e.clone()));
    assert_eq!(take_error(&d), None);
    // installed some other way meanwhile
    keep_error(&d, &InstallError { version: super::super::current_version().into(), ..e.clone() });
    assert_eq!(take_error(&d), None);
    keep_error(&d, &InstallError { page: "file:///etc/passwd".into(), ..e });
    assert_eq!(take_error(&d), None);
    for text in ["", "{", "[]", "{\"version\":\"99.0.0\"}", "\u{fffd}"] {
        fs::write(error_file(&d), text).unwrap();
        assert_eq!(take_error(&d), None, "{text}");
        assert!(!error_file(&d).exists());
    }
    fs::remove_dir_all(d.parent().unwrap()).unwrap();
}

// ------------------------------------------------------------------ the whole way

/// Serves `files` (path, content) on a local address until it is dropped, counting the answers.
struct Server {
    url: String,
    served: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Server {
    fn start(files: impl FnOnce(&str) -> Vec<(String, Vec<u8>)>) -> Server {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        let files = files(&url);
        let s = Server { url, served: Default::default(), stop: Default::default() };
        let (served, stop) = (s.served.clone(), s.stop.clone());
        std::thread::spawn(move || {
            for c in l.incoming().flatten() {
                if stop.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                let mut c = c;
                let (mut request, mut byte) = (vec![], [0u8]);
                while !request.ends_with(b"\r\n\r\n") && c.read(&mut byte).is_ok_and(|n| n == 1) {
                    request.push(byte[0]);
                }
                let path = String::from_utf8_lossy(&request).split(' ').nth(1).unwrap_or_default().to_string();
                let found = files.iter().find(|(p, _)| *p == path);
                let (status, body) = found.map(|(_, b)| ("200 OK", &b[..])).unwrap_or(("404 Not Found", b""));
                served.lock().unwrap().push(path);
                let head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = c.write_all(head.as_bytes()).and_then(|()| c.write_all(body));
            }
        });
        s
    }

    fn served(&self, path: &str) -> usize {
        self.served.lock().unwrap().iter().filter(|p| *p == path).count()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = std::net::TcpStream::connect(self.url.trim_start_matches("http://"));
    }
}

/// A server with the release 99.0.0: its AppImage (`described` says what GitHub knows of it, `sent`
/// what is downloaded), a .deb and a setup for Windows.
fn release_server(described: &[u8], sent: &[u8]) -> Server {
    let name = release(described).asset_name;
    let json = |url: &str| {
        json!({
            "tag_name": "v99.0.0",
            "body": "## What's new\n\n- Everything",
            "html_url": "https://github.com/toim22/AnyVector/releases/tag/v99.0.0",
            "assets": [
                { "name": "AnyVector-99.0.0-setup.exe", "browser_download_url": format!("{url}/setup"), "size": 5, "digest": sha256(b"setup") },
                { "name": "anyvector_99.0.0_amd64.deb", "browser_download_url": format!("{url}/deb"), "size": 3, "digest": sha256(b"deb") },
                { "name": name, "browser_download_url": format!("{url}/appimage"), "size": described.len(), "digest": sha256(described) }
            ]
        })
        .to_string()
    };
    Server::start(|url| vec![("/latest".into(), json(url).into_bytes()), ("/appimage".into(), sent.to_vec()), ("/setup".into(), b"setup".to_vec()), ("/deb".into(), b"deb".to_vec())])
}

/// AnyVector without a window, running from the AppImage `file`, with its downloads in `dir`.
fn anyvector<'a>(file: &Path, dir: &Path) -> Harness<'a, AnyVectorApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 900.0)).build_eframe(|cc| {
        let mut app = AnyVectorApp::new(cc);
        app.splash = None;
        app.pending.clear();
        app
    });
    let u = &mut h.state_mut().update;
    u.install = Install::AppImage(running(file));
    u.dir = Some(dir.to_path_buf());
    h.state_mut().settings.update_mode = KEEP_UP_TO_DATE;
    for _ in 0..3 {
        h.step();
    }
    h
}

fn wait_until(h: &mut Harness<'_, AnyVectorApp>, what: &str, done: impl Fn(&Status) -> bool) {
    let t = Instant::now();
    while !done(&h.state().update.status) {
        assert!(t.elapsed() < Duration::from_secs(30), "{what}: {:?}", h.state().update.status);
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
    for _ in 0..3 {
        h.step();
    }
}

fn click(h: &mut Harness<'_, AnyVectorApp>, label: &str) {
    h.get_by_role_and_label(egui::accesskit::Role::Button, label).click_accesskit();
    for _ in 0..3 {
        h.step();
    }
}

/// The check at start finds a release, its AppImage is downloaded and checked, OK closes AnyVector, and
/// on exit the file is replaced: from a (local) server to the file that starts.
#[test]
fn an_appimage_updates_itself() {
    let d = folder("update");
    let (file, cache) = (d.join("AnyVector.AppImage"), d.join("cache/Updates"));
    put(&file, &program("old"), 0o755);
    let new = program("new");
    let server = release_server(&new, &new);
    let mut h = anyvector(&file, &cache);
    start_check_at(h.state_mut(), true, format!("{}/latest", server.url));
    wait_until(&mut h, "download", |s| matches!(s, Status::Ready(..)));
    let download = cache.join(release(&new).asset_name);
    assert_eq!(fs::read(&download).unwrap(), new);
    assert!(h.state().update.prompt, "the new version is announced");
    assert!(h.query_all_by_label("Open Release Page").next().is_none());
    // Later: nothing happens now, and the download is used again at the next start
    click(&mut h, "Later");
    assert!(h.state().update.install_on_exit.is_none());
    crate::updater::run_pending_install(h.state_mut());
    assert_eq!(fs::read(&file).unwrap(), program("old"));
    let mut h = anyvector(&file, &cache);
    start_check_at(h.state_mut(), true, format!("{}/latest", server.url));
    wait_until(&mut h, "download used again", |s| matches!(s, Status::Ready(..)));
    assert_eq!(server.served("/appimage"), 1);
    // unless it was damaged meanwhile
    let mut damaged = new.clone();
    damaged[new.len() - 1] ^= 1;
    fs::write(&download, &damaged).unwrap();
    let mut h = anyvector(&file, &cache);
    start_check_at(h.state_mut(), true, format!("{}/latest", server.url));
    wait_until(&mut h, "downloaded again", |s| matches!(s, Status::Ready(..)));
    assert_eq!((server.served("/appimage"), fs::read(&download).unwrap()), (2, new.clone()));
    // OK
    click(&mut h, "OK");
    assert!(h.state().update.install_on_exit.is_some() && !h.state().update.prompt);
    assert_eq!(fs::read(&file).unwrap(), program("old"), "the file is replaced when AnyVector closes");
    crate::updater::run_pending_install(h.state_mut());
    assert_eq!((fs::read(&file).unwrap(), mode(&file)), (new, 0o755));
    assert_eq!(names(&d), ["AnyVector.AppImage", "cache"]);
    assert!(names(&cache).is_empty(), "neither the download nor an error is left: {:?}", names(&cache));
    // the new file starts
    let out = d.join("started");
    let mut c = restart_command(&running(&file), [(OsString::from("NEW"), OsString::from("1"))], u32::MAX);
    assert!(c.stdout(File::create(&out).unwrap()).status().unwrap().success());
    assert!(fs::read_to_string(&out).unwrap().lines().any(|l| l == "NEW=1"));
    for path in ["/setup", "/deb"] {
        assert_eq!(server.served(path), 0, "{path}");
    }
    fs::remove_dir_all(d).unwrap();
}

/// What was downloaded is not what the release says: it isn't installed, and the next try starts over.
#[test]
fn a_wrong_download_is_refused() {
    let d = folder("wrong");
    let (file, cache) = (d.join("AnyVector.AppImage"), d.join("cache"));
    put(&file, &program("old"), 0o755);
    let server = release_server(&program("new"), &program("bad"));
    let mut h = anyvector(&file, &cache);
    start_check_at(h.state_mut(), true, format!("{}/latest", server.url));
    // the check at start stays quiet: the version is still announced
    wait_until(&mut h, "refused", |s| matches!(s, Status::Available(_)) && server.served("/appimage") == 1);
    wait_until(&mut h, "settled", |s| matches!(s, Status::Available(_)));
    assert!(names(&cache).is_empty(), "{:?}", names(&cache));
    // the user asked for it: told why
    click(&mut h, "OK");
    wait_until(&mut h, "refused again", |s| matches!(s, Status::Failed(_)));
    assert!(matches!(&h.state().update.status, Status::Failed(e) if e == BAD_CHECKSUM), "{:?}", h.state().update.status);
    assert!(h.state().update.install_on_exit.is_none());
    crate::updater::run_pending_install(h.state_mut());
    assert_eq!(fs::read(&file).unwrap(), program("old"));
    fs::remove_dir_all(d).unwrap();
}

/// The folder becomes read-only between the download and the exit: the old AppImage stays, and the next
/// start says why. A read-only one to begin with is only announced.
#[test]
fn an_update_that_fails_on_exit_is_told_at_the_next_start() {
    if is_root() {
        return;
    }
    let d = folder("exit");
    let (apps, cache) = (d.join("apps"), d.join("cache"));
    fs::create_dir(&apps).unwrap();
    let file = apps.join("AnyVector.AppImage");
    put(&file, &program("old"), 0o755);
    let new = program("new");
    let server = release_server(&new, &new);
    let mut h = anyvector(&file, &cache);
    start_check_at(h.state_mut(), true, format!("{}/latest", server.url));
    wait_until(&mut h, "download", |s| matches!(s, Status::Ready(..)));
    click(&mut h, "OK");
    fs::set_permissions(&apps, fs::Permissions::from_mode(0o555)).unwrap();
    crate::updater::run_pending_install(h.state_mut());
    assert_eq!(fs::read(&file).unwrap(), program("old"));
    let reason = format!("You do not have permission to replace {}.", file.display());
    let mut h = anyvector(&file, &cache);
    h.state_mut().update.error = take_error(&cache);
    for _ in 0..3 {
        h.step();
    }
    assert!(h.query_all_by_label("AnyVector 99.0.0 could not be installed.").next().is_some());
    assert!(h.query_all_by_label(&reason).next().is_some(), "{reason}");
    click(&mut h, "Close");
    // and now it is known to be read-only: announced, nothing downloaded again
    start_check_at(h.state_mut(), true, format!("{}/latest", server.url));
    wait_until(&mut h, "announced", |s| matches!(s, Status::Available(_)));
    assert!(h.query_all_by_label("Open Release Page").next().is_some());
    assert!(h.query_all_by_label("OK").next().is_none());
    assert!(h.query_all_by_label(&reason).next().is_some());
    assert_eq!(server.served("/appimage"), 1);
    // the next start has nothing more to tell
    assert_eq!(take_error(&cache), None);
    // read-only between the message and OK: the message says so now, and AnyVector doesn't close
    fs::set_permissions(&apps, fs::Permissions::from_mode(0o755)).unwrap();
    let mut h = anyvector(&file, &cache);
    start_check_at(h.state_mut(), true, format!("{}/latest", server.url));
    wait_until(&mut h, "download", |s| matches!(s, Status::Ready(..)));
    assert!(h.query_all_by_label("OK").next().is_some());
    fs::set_permissions(&apps, fs::Permissions::from_mode(0o555)).unwrap();
    click(&mut h, "OK");
    assert!(h.state().update.install_on_exit.is_none());
    assert!(h.query_all_by_label(&reason).next().is_some());
    assert!(h.query_all_by_label("Open Release Page").next().is_some() && h.query_all_by_label("OK").next().is_none());
    fs::set_permissions(&apps, fs::Permissions::from_mode(0o755)).unwrap();
    fs::remove_dir_all(d).unwrap();
}

/// A package or a plain program downloads nothing, whatever the release carries.
#[test]
fn a_package_is_only_told() {
    let d = folder("package");
    let (file, cache) = (d.join("AnyVector.AppImage"), d.join("cache"));
    put(&file, &program("old"), 0o755);
    let server = release_server(&program("new"), &program("new"));
    let mut h = anyvector(&file, &cache);
    h.state_mut().update.install = Install::Other;
    start_check_at(h.state_mut(), true, format!("{}/latest", server.url));
    wait_until(&mut h, "announced", |s| matches!(s, Status::Available(_)));
    click(&mut h, "Open Release Page");
    assert!(!h.state().update.prompt);
    assert!(!cache.exists());
    assert_eq!(*server.served.lock().unwrap(), ["/latest"]);
    // fetching is for AppImages; the file of a release that isn't one is refused
    let (tx, _rx) = std::sync::mpsc::channel();
    let mut r = release(&program("new"));
    r.asset_url = format!("{}/setup", server.url);
    assert_eq!(fetch(&Install::AppImage(running(&file)), &r, &cache, &tx), Err(INTERRUPTED.into()));
    r.asset_url = format!("{}/gone", server.url);
    assert!(fetch(&Install::AppImage(running(&file)), &r, &cache, &tx).unwrap_err().contains("404"));
    assert!(names(&cache).iter().all(|n| n != &r.asset_name));
    fs::remove_dir_all(d).unwrap();
}
