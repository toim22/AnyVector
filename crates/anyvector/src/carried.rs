//! The libraries the AppImage carries (`packaging/linux/appimage-libraries.txt`): the X11 and
//! keyboard ones AnyVector opens when it starts and that not every system has.
//!
//! The system's own library is used where there is one. The carried one is only opened where there
//! is none, by its path; from then on whoever asks for it by name (winit, xkbcommon-dl, another
//! library) gets the one already open. Nothing is put in `LD_LIBRARY_PATH`, so the programs
//! AnyVector starts keep the system's libraries.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

const LIST: &str = include_str!("../../../packaging/linux/appimage-libraries.txt");

/// The libraries' names, each one after the ones it needs.
fn names() -> impl Iterator<Item = &'static str> {
    LIST.lines().filter(|l| !l.starts_with('#')).filter_map(|l| l.split_whitespace().next())
}

/// Where the AppImage has them: `usr/lib/anyvector`, next to the program's `usr/bin`.
fn folder(exe: &Path) -> Option<PathBuf> {
    Some(exe.parent()?.parent()?.join("lib/anyvector"))
}

/// Opens a library, by its name or path, for as long as AnyVector runs.
fn open(library: &std::ffi::OsStr) -> bool {
    extern "C" {
        fn dlopen(file: *const std::ffi::c_char, flags: std::ffi::c_int) -> *mut std::ffi::c_void;
    }
    const RTLD_NOW: std::ffi::c_int = 2;
    let Ok(name) = CString::new(library.as_bytes()) else { return false };
    // SAFETY: a NUL-terminated name. What is opened runs the library's own start code, as when
    // winit opens the same ones a moment later.
    !unsafe { dlopen(name.as_ptr(), RTLD_NOW) }.is_null()
}

/// The libraries of `names` that came from `folder`, as the system has none.
fn open_missing<'a>(names: impl Iterator<Item = &'a str>, folder: &Path) -> Vec<&'a str> {
    names
        .filter(|name| {
            let carried = folder.join(name);
            carried.is_file() && !open(name.as_ref()) && open(carried.as_os_str())
        })
        .collect()
}

/// Opens what the system lacks. Called before the window's libraries are. Does nothing where
/// AnyVector carries none (the .deb, a build).
pub fn open_libraries() {
    let Some(folder) = std::env::current_exe().ok().as_deref().and_then(folder).filter(|f| f.is_dir()) else { return };
    let opened = open_missing(names(), &folder);
    if !opened.is_empty() {
        eprintln!("AnyVector: not on this system, using the AppImage's: {}", opened.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_names_the_libraries() {
        let names: Vec<_> = names().collect();
        assert_eq!(names.len(), 8);
        for n in &names {
            assert!(n.starts_with("lib") && n.contains(".so.") && !n.contains('/'), "{n}");
        }
        let at = |n: &str| names.iter().position(|x| *x == n).unwrap_or_else(|| panic!("{n}"));
        // what needs another one comes after it
        for (library, needs) in [("libxkbcommon-x11.so.0", "libxkbcommon.so.0"), ("libxkbcommon-x11.so.0", "libxcb-xkb.so.1"), ("libXi.so.6", "libXext.so.6"), ("libXcursor.so.1", "libXrender.so.1"), ("libXcursor.so.1", "libXfixes.so.3")] {
            assert!(at(library) > at(needs), "{library}");
        }
    }

    #[test]
    fn carried_libraries_are_next_to_the_program() {
        assert_eq!(folder(Path::new("/tmp/.mount_Any/usr/bin/anyvector")), Some("/tmp/.mount_Any/usr/lib/anyvector".into()));
        assert_eq!(folder(Path::new("/")), None);
    }

    #[test]
    fn only_what_the_system_lacks_is_opened() {
        let dir = std::env::temp_dir().join(format!("anyvector-carried-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // a library of the system, under a name the system doesn't have
        let maps = std::fs::read_to_string("/proc/self/maps").unwrap();
        let any = maps.lines().filter_map(|l| l.split_whitespace().nth(5)).find(|p| p.contains("/libm.so") || p.contains("/libgcc_s.so")).expect("a library of this program");
        let own = "libanyvector-carried-test.so.1";
        std::fs::copy(any, dir.join(own)).unwrap();
        // one the system has, and a damaged one
        std::fs::copy(any, dir.join("libc.so.6")).unwrap();
        std::fs::write(dir.join("libanyvector-damaged.so.1"), b"").unwrap();
        let names = [own, "libc.so.6", "libanyvector-damaged.so.1", "libanyvector-not-there.so.1"];
        assert_eq!(open_missing(names.into_iter(), &dir), [own]);
        assert!(!open("libanyvector-not-there.so.1".as_ref()) && !open("a\0b".as_ref()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
