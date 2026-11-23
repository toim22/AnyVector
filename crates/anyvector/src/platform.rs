//! Desktop integration: opening web pages, documents and folders with the programs the desktop
//! chose for them, showing a file selected in the file manager, and starting an e-mail.
//!
//! | Action | Windows | Linux |
//! | --- | --- | --- |
//! | [`open_url`] | `cmd /C start "" "<url>"`, without a console window | `xdg-open <url>` |
//! | [`open_file`] | `cmd /C start "" "<file>"` | `xdg-open <file>` |
//! | [`open_folder`] | `explorer <folder>` | `xdg-open <folder>` |
//! | [`reveal`] | `explorer /select,<file>` | the file manager's D-Bus method `org.freedesktop.FileManager1.ShowItems` (Files, Dolphin, Nemo, Caja and Thunar have it), called with `dbus-send`, else `gdbus`; when no file manager answers, `xdg-open <folder>` |
//! | [`open_mailto`] | `cmd /C start "" "mailto:?subject=<subject>"` | `xdg-open mailto:?subject=<subject>` |
//! | `send_mail` | Simple MAPI (in `exec.rs`) | `xdg-email --utf8 --subject <subject> --attach <file>` |
//!
//! On Windows the target of `start` reaches `cmd` through an environment variable, in quotes, so
//! a `&` in a URL or file name is not read as another command. Nothing here waits for the program it
//! starts: an error comes back only when the program could not be started, as on Windows. On Linux
//! a thread waits for each program to end, so none stays behind as a zombie process, and a program
//! that ends with an error is logged.

use std::io;
use std::path::Path;

#[cfg(windows)]
use self::win as os;
#[cfg(not(windows))]
use self::xdg as os;

/// Open a web page in the default browser (also a page on disk: Help writes its pages there).
pub fn open_url(url: &str) -> io::Result<()> {
    os::open_url(url)
}

/// Open a document with the program the desktop opens it with (Edit Bitmap, linked files).
pub fn open_file(path: &Path) -> io::Result<()> {
    os::open_file(path)
}

/// Open a folder in the file manager.
pub fn open_folder(dir: &Path) -> io::Result<()> {
    os::open_folder(dir)
}

/// Show a file selected in its folder in the file manager. Nothing comes back: on Linux the file
/// manager answers later, and when none does the folder opens instead.
pub fn reveal(path: &Path) {
    os::reveal(path)
}

/// A new e-mail with this subject in the default mail program.
pub fn open_mailto(subject: &str) -> io::Result<()> {
    os::open_mailto(subject)
}

/// An e-mail with `attachment` and `subject` in the default mail program, through `xdg-email`.
/// `done` learns whether a mail program took it: false when there is none or it failed, true when
/// it ended well or still runs after a few seconds (Thunderbird's `-compose` runs until Thunderbird
/// closes). It is called on another thread, or at once when `xdg-email` can't be started. Windows
/// sends the e-mail through Simple MAPI instead (`exec.rs`).
#[cfg(not(windows))]
pub fn send_mail(subject: &str, attachment: &Path, done: impl FnOnce(bool) + Send + 'static) {
    let _ = xdg::send_mail(&xdg::Tools::default(), subject, attachment, xdg::MAIL_GRACE, done);
}

/// `bytes` percent-encoded: only letters, digits, `-._~` and, with `keep_slash`, `/` stay as they
/// are.
fn percent_encode(bytes: &[u8], keep_slash: bool) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for &b in bytes {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') || (keep_slash && b == b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The `file://` URI of an absolute path, given as bytes (Linux file names are bytes, not always
/// UTF-8). With everything but letters, digits, `-._~` and `/` encoded, the URI has no comma, quote
/// or space: `dbus-send` splits arrays at commas and `gdbus` reads quoted text.
#[cfg(any(not(windows), test))]
fn file_uri(path: &[u8]) -> String {
    format!("file://{}", percent_encode(path, true))
}

/// The `mailto:` URL of a new e-mail with this subject.
fn mailto_url(subject: &str) -> String {
    format!("mailto:?subject={}", percent_encode(subject.as_bytes(), false))
}

// ================================================================================== Windows

#[cfg(windows)]
mod win {
    use std::ffi::OsStr;
    use std::io;
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// The environment variable that hands `cmd` the target of `start`.
    pub(super) const TARGET_VAR: &str = "ANYVECTOR_START_TARGET";

    /// `cmd /C start "" "<target>"`: `start` treats the first quoted argument as the window title,
    /// hence the "".
    fn start(target: &OsStr) -> Command {
        cmd_with_target(&["start", ""], target)
    }

    /// `cmd /C <before> "%ANYVECTOR_START_TARGET%"` with the target in that variable. Written on
    /// `cmd`'s command line, a target would be read as more commands at a `&` outside quotes
    /// (`...?labels=bug&title=...`, `Art&Logos.png`) and its `%NAME%` parts replaced; `cmd`
    /// replaces the variable once, and the quotes keep `&`, `|`, `^`, `<` and `>` in it plain. A `"`
    /// (no file name has one) would end the quotes: it is sent as %22, as in a URL.
    pub(super) fn cmd_with_target(before: &[&str], target: &OsStr) -> Command {
        let lossy = target.to_string_lossy();
        let target = if lossy.contains('"') { lossy.replace('"', "%22").into() } else { target.to_os_string() };
        let mut c = Command::new("cmd");
        c.arg("/C").args(before).raw_arg(format!("\"%{TARGET_VAR}%\"")).env(TARGET_VAR, target);
        c
    }

    pub(super) fn url_command(url: &str) -> Command {
        let mut c = start(OsStr::new(url));
        c.creation_flags(CREATE_NO_WINDOW);
        c
    }

    pub(super) fn file_command(path: &Path) -> Command {
        start(path.as_os_str())
    }

    pub(super) fn folder_command(dir: &Path) -> Command {
        let mut c = Command::new("explorer");
        c.arg(dir);
        c
    }

    pub(super) fn reveal_command(path: &Path) -> Command {
        let mut c = Command::new("explorer");
        c.arg(format!("/select,{}", path.display()));
        c
    }

    pub(super) fn mailto_command(subject: &str) -> Command {
        start(OsStr::new(&super::mailto_url(subject)))
    }

    pub(super) fn open_url(url: &str) -> io::Result<()> {
        url_command(url).spawn().map(drop)
    }

    pub(super) fn open_file(path: &Path) -> io::Result<()> {
        file_command(path).spawn().map(drop)
    }

    pub(super) fn open_folder(dir: &Path) -> io::Result<()> {
        folder_command(dir).spawn().map(drop)
    }

    pub(super) fn reveal(path: &Path) {
        let _ = reveal_command(path).spawn();
    }

    pub(super) fn open_mailto(subject: &str) -> io::Result<()> {
        mailto_command(subject).spawn().map(drop)
    }
}

// ==================================================================================== Linux

#[cfg(not(windows))]
mod xdg {
    use super::{file_uri, mailto_url};
    use std::ffi::{OsStr, OsString};
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    /// How long the file manager may take to answer ShowItems (D-Bus may have to start it).
    pub(super) const SHOW_TIMEOUT: Duration = Duration::from_secs(10);
    /// How long `xdg-email` has to fail before the mail program is taken to have the message.
    pub(super) const MAIL_GRACE: Duration = Duration::from_secs(5);

    /// The programs the actions run, each a program and the arguments that come first (the tests
    /// run fake ones).
    #[derive(Clone, Debug)]
    pub(super) struct Tools {
        pub xdg_open: Vec<OsString>,
        pub xdg_email: Vec<OsString>,
        pub dbus_send: Vec<OsString>,
        pub gdbus: Vec<OsString>,
    }

    impl Default for Tools {
        fn default() -> Tools {
            Tools { xdg_open: vec!["xdg-open".into()], xdg_email: vec!["xdg-email".into()], dbus_send: vec!["dbus-send".into()], gdbus: vec!["gdbus".into()] }
        }
    }

    /// A command for `tool` that reads nothing from AnyVector's terminal and runs in a process
    /// group of its own, so a Ctrl+C in that terminal does not close what it opened.
    fn command(tool: &[OsString]) -> Command {
        let mut c = Command::new(tool.first().map(OsString::as_os_str).unwrap_or_default());
        c.args(tool.iter().skip(1)).stdin(Stdio::null()).process_group(0);
        c
    }

    fn name_of(c: &Command) -> String {
        Path::new(c.get_program()).file_name().unwrap_or_default().to_string_lossy().into_owned()
    }

    pub(super) fn open_command(t: &Tools, target: &OsStr) -> Command {
        let mut c = command(&t.xdg_open);
        c.arg(target);
        c
    }

    /// `dbus-send` and `gdbus` calling `ShowItems([uri], "")` on the file manager; both wait for
    /// the answer, so an error means that no file manager took the call.
    pub(super) fn show_items_commands(t: &Tools, uri: &str, timeout: Duration) -> [Command; 2] {
        let mut dbus_send = command(&t.dbus_send);
        dbus_send.args([
            "--session".to_string(),
            "--print-reply".into(),
            format!("--reply-timeout={}", timeout.as_millis()),
            "--dest=org.freedesktop.FileManager1".into(),
            "--type=method_call".into(),
            "/org/freedesktop/FileManager1".into(),
            "org.freedesktop.FileManager1.ShowItems".into(),
            format!("array:string:{uri}"),
            "string:".into(),
        ]);
        let mut gdbus = command(&t.gdbus);
        gdbus.args([
            "call".to_string(),
            "--session".into(),
            "--dest".into(),
            "org.freedesktop.FileManager1".into(),
            "--object-path".into(),
            "/org/freedesktop/FileManager1".into(),
            "--method".into(),
            "org.freedesktop.FileManager1.ShowItems".into(),
            "--timeout".into(),
            timeout.as_secs().max(1).to_string(),
            format!("['{uri}']"),
            "''".into(),
        ]);
        [dbus_send, gdbus]
    }

    pub(super) fn mail_command(t: &Tools, subject: &str, attachment: &Path) -> Command {
        let mut c = command(&t.xdg_email);
        // the subject is UTF-8 whatever the locale is
        c.arg("--utf8");
        // xdg-email refuses an empty subject
        if !subject.trim().is_empty() {
            c.args(["--subject", subject]);
        }
        c.arg("--attach").arg(attachment);
        c
    }

    /// Start `c` without waiting for it.
    pub(super) fn run(mut c: Command) -> io::Result<()> {
        let child = c.spawn()?;
        reap(child, name_of(&c));
        Ok(())
    }

    /// Wait for `child` on a thread of its own, so it does not stay a zombie, and log a failure.
    pub(super) fn reap(mut child: Child, name: String) {
        let waiting = std::thread::Builder::new().name("anyvector-wait".into()).spawn(move || match child.wait() {
            Ok(s) if !s.success() => log::warn!("{name} ended with {s}"),
            Err(e) => log::warn!("waiting for {name}: {e}"),
            Ok(_) => {}
        });
        if let Err(e) = waiting {
            log::warn!("no thread to wait for a program: {e}");
        }
    }

    pub(super) fn open_url(url: &str) -> io::Result<()> {
        run(open_command(&Tools::default(), OsStr::new(url)))
    }

    pub(super) fn open_file(path: &Path) -> io::Result<()> {
        run(open_command(&Tools::default(), path.as_os_str()))
    }

    pub(super) fn open_folder(dir: &Path) -> io::Result<()> {
        run(open_command(&Tools::default(), dir.as_os_str()))
    }

    pub(super) fn open_mailto(subject: &str) -> io::Result<()> {
        run(open_command(&Tools::default(), OsStr::new(&mailto_url(subject))))
    }

    pub(super) fn reveal(path: &Path) {
        let _ = reveal_with(&Tools::default(), path, SHOW_TIMEOUT);
    }

    /// [`reveal`] with these tools; the thread that does it.
    pub(super) fn reveal_with(t: &Tools, path: &Path, timeout: Duration) -> Option<JoinHandle<()>> {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let t = t.clone();
        let thread = std::thread::Builder::new().name("anyvector-reveal".into()).spawn(move || {
            if show_items(&t, &file_uri(path.as_os_str().as_bytes()), timeout) {
                return;
            }
            // no file manager took the call: open the folder
            let dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("/"));
            if let Err(e) = run(open_command(&t, dir.as_os_str())) {
                log::warn!("could not open the folder {}: {e}", dir.display());
            }
        });
        thread.map_err(|e| log::warn!("no thread to show the file: {e}")).ok()
    }

    /// Ask the file manager to show `uri` selected: true when one took the call. Through
    /// `dbus-send`, or `gdbus` when that is not installed.
    pub(super) fn show_items(t: &Tools, uri: &str, timeout: Duration) -> bool {
        for mut c in show_items_commands(t, uri, timeout) {
            // not installed: the other one
            let Ok(out) = c.stdout(Stdio::null()).stderr(Stdio::piped()).output() else { continue };
            if !out.status.success() {
                log::info!("no file manager showed the file ({}): {}", name_of(&c), String::from_utf8_lossy(&out.stderr).trim());
            }
            return out.status.success();
        }
        false
    }

    /// [`super::send_mail`] with these tools; the thread that waits for `xdg-email`, if it started.
    pub(super) fn send_mail(t: &Tools, subject: &str, attachment: &Path, grace: Duration, done: impl FnOnce(bool) + Send + 'static) -> Option<JoinHandle<()>> {
        let attachment = std::path::absolute(attachment).unwrap_or_else(|_| attachment.to_path_buf());
        let child = match mail_command(t, subject, &attachment).spawn() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("xdg-email could not be started: {e}");
                done(false);
                return None;
            }
        };
        let thread = std::thread::Builder::new().name("anyvector-mail".into()).spawn(move || wait_for_mail(child, grace, done));
        thread.map_err(|e| log::warn!("no thread to wait for xdg-email: {e}")).ok()
    }

    fn wait_for_mail(mut child: Child, grace: Duration, done: impl FnOnce(bool)) {
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        log::warn!("xdg-email ended with {status}");
                    }
                    return done(status.success());
                }
                Ok(None) if start.elapsed() < grace => std::thread::sleep(Duration::from_millis(50)),
                Ok(None) => {
                    // still running: the mail program has the message
                    done(true);
                    if let Ok(status) = child.wait() {
                        if !status.success() {
                            log::warn!("xdg-email ended with {status}");
                        }
                    }
                    return;
                }
                Err(e) => {
                    log::warn!("waiting for xdg-email: {e}");
                    return done(false);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_and_mailto_keep_only_safe_characters() {
        assert_eq!(file_uri(b"/home/ana/My Drawings/a,b'c\"d.cdr"), "file:///home/ana/My%20Drawings/a%2Cb%27c%22d.cdr");
        assert_eq!(file_uri("/tmp/ação [1].png".as_bytes()), "file:///tmp/a%C3%A7%C3%A3o%20%5B1%5D.png");
        // a Linux file name need not be UTF-8: its bytes are kept
        assert_eq!(file_uri(b"/tmp/\xff~x_y-z.png"), "file:///tmp/%FF~x_y-z.png");
        assert_eq!(mailto_url("Logo & Card #2/3"), "mailto:?subject=Logo%20%26%20Card%20%232%2F3");
        assert_eq!(mailto_url(""), "mailto:?subject=");
    }

    /// The Windows commands: `start` for pages, files and e-mail, Explorer for folders.
    #[cfg(windows)]
    #[test]
    fn windows_runs_start_and_explorer() {
        use std::process::Command;
        fn line(c: &Command) -> Vec<String> {
            std::iter::once(c.get_program()).chain(c.get_args()).map(|a| a.to_string_lossy().into_owned()).collect()
        }
        fn target(c: &Command) -> String {
            c.get_envs().find(|(k, _)| *k == win::TARGET_VAR).and_then(|(_, v)| v).map(|v| v.to_string_lossy().into_owned()).unwrap_or_default()
        }
        let start = ["cmd", "/C", "start", "", "\"%ANYVECTOR_START_TARGET%\""];
        let url = "https://github.com/toim22/AnyVector/issues/new?labels=bug&title=T";
        assert_eq!(line(&win::url_command(url)), start);
        assert_eq!(target(&win::url_command(url)), url);
        let file = Path::new(r"C:\Users\Ana\AppData\Local\Temp\AnyVector\bitmap-12.png");
        assert_eq!(line(&win::file_command(file)), start);
        assert_eq!(target(&win::file_command(file)), file.to_str().unwrap());
        let dir = Path::new(r"C:\Users\Ana\AppData\Roaming\AnyVector\Templates");
        assert_eq!(line(&win::folder_command(dir)), ["explorer", r"C:\Users\Ana\AppData\Roaming\AnyVector\Templates"]);
        let doc = Path::new(r"C:\Users\Ana\Documents\My Logo.cdr");
        assert_eq!(line(&win::reveal_command(doc)), ["explorer", r"/select,C:\Users\Ana\Documents\My Logo.cdr"]);
        assert_eq!(target(&win::mailto_command("Logo & Card")), "mailto:?subject=Logo%20%26%20Card");
        // a quote would end cmd's quotes: as in a URL, %22
        assert_eq!(target(&win::url_command("https://example.com/?q=\"a\"")), "https://example.com/?q=%22a%22");
    }

    /// What `cmd` makes of a target with `&` (a bug report URL, a file name): one whole argument,
    /// not a second command, and `%NAME%` in it left alone. `echo` stands in for `start` here, so
    /// nothing opens; a split would run `ver`, which only prints.
    #[cfg(windows)]
    #[test]
    fn windows_targets_with_ampersands_stay_whole() {
        let targets = [
            "https://github.com/toim22/AnyVector/issues/new?labels=bug&title=Crash&body=Steps%20%26%20more",
            r"C:\Users\Ana\Art&ver\logo&ver.png",
            r"C:\Users\Ana\Art & Logos\a|b^c %OS% <d>.png",
        ];
        for t in targets {
            let out = win::cmd_with_target(&["echo"], std::ffi::OsStr::new(t)).output().expect("cmd");
            let said = String::from_utf8_lossy(&out.stdout);
            assert_eq!(said.trim_end(), format!("\"{t}\""), "cmd split or changed {t}");
        }
    }

    #[cfg(not(windows))]
    mod linux {
        use super::super::xdg::*;
        use std::ffi::{OsStr, OsString};
        use std::path::{Path, PathBuf};
        use std::time::{Duration, Instant};

        /// A folder of fake programs that write the arguments they get to `<name>.log`.
        struct Fakes {
            dir: PathBuf,
        }

        impl Fakes {
            fn new(test: &str) -> Fakes {
                let dir = std::env::temp_dir().join(format!("anyvector-platform-{}-{test}", std::process::id()));
                let _ = std::fs::remove_dir_all(&dir);
                std::fs::create_dir_all(&dir).unwrap();
                Fakes { dir }
            }

            /// A fake program `name` that ends with the shell commands `then`. It is a script run
            /// by `sh`: executing a file just written can fail with "text file busy" while other
            /// tests start programs.
            fn tool(&self, name: &str, then: &str) -> Vec<OsString> {
                let script = self.dir.join(format!("{name}.sh"));
                let log = self.dir.join(format!("{name}.log"));
                let body = format!("printf '%s\\n' \"$@\" > '{log}.tmp' && mv '{log}.tmp' '{log}'\n{then}\n", log = log.display());
                std::fs::write(&script, body).unwrap();
                vec!["sh".into(), script.into()]
            }

            fn missing(&self) -> Vec<OsString> {
                vec![self.dir.join("not-installed").into()]
            }

            fn ran(&self, name: &str) -> bool {
                self.dir.join(format!("{name}.log")).exists()
            }

            /// The arguments `name` got, once it has written them.
            fn args(&self, name: &str) -> Vec<String> {
                let log = self.dir.join(format!("{name}.log"));
                let start = Instant::now();
                while !log.exists() && start.elapsed() < Duration::from_secs(10) {
                    std::thread::sleep(Duration::from_millis(20));
                }
                std::fs::read_to_string(&log).unwrap_or_default().lines().map(String::from).collect()
            }

            fn tools(&self) -> Tools {
                Tools { xdg_open: self.tool("xdg-open", "exit 0"), xdg_email: self.tool("xdg-email", "exit 0"), dbus_send: self.tool("dbus-send", "exit 0"), gdbus: self.tool("gdbus", "exit 0") }
            }
        }

        impl Drop for Fakes {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.dir);
            }
        }

        const URI: &str = "file:///tmp/My%20Folder/a%2Cb.cdr";

        #[test]
        fn opening_runs_xdg_open() {
            let f = Fakes::new("open");
            let t = f.tools();
            run(open_command(&t, OsStr::new("https://github.com/toim22/AnyVector/issues/new?labels=bug&title=T"))).unwrap();
            assert_eq!(f.args("xdg-open"), ["https://github.com/toim22/AnyVector/issues/new?labels=bug&title=T"]);
            let t = Tools { xdg_open: f.missing(), ..t };
            assert!(run(open_command(&t, OsStr::new("/tmp"))).is_err(), "a missing xdg-open is an error, as a missing cmd is on Windows");
        }

        #[test]
        fn reveal_asks_the_file_manager_through_dbus_send() {
            let f = Fakes::new("dbus-send");
            assert!(show_items(&f.tools(), URI, SHOW_TIMEOUT));
            let array = format!("array:string:{URI}");
            assert_eq!(
                f.args("dbus-send"),
                ["--session", "--print-reply", "--reply-timeout=10000", "--dest=org.freedesktop.FileManager1", "--type=method_call", "/org/freedesktop/FileManager1", "org.freedesktop.FileManager1.ShowItems", &array, "string:"]
            );
            assert!(!f.ran("gdbus") && !f.ran("xdg-open"));
        }

        #[test]
        fn reveal_uses_gdbus_without_dbus_send() {
            let f = Fakes::new("gdbus");
            let t = Tools { dbus_send: f.missing(), ..f.tools() };
            assert!(show_items(&t, URI, SHOW_TIMEOUT));
            let array = format!("['{URI}']");
            assert_eq!(
                f.args("gdbus"),
                ["call", "--session", "--dest", "org.freedesktop.FileManager1", "--object-path", "/org/freedesktop/FileManager1", "--method", "org.freedesktop.FileManager1.ShowItems", "--timeout", "10", &array, "''"]
            );
        }

        #[test]
        fn reveal_opens_the_folder_when_no_file_manager_answers() {
            let f = Fakes::new("no-file-manager");
            let folder = f.dir.join("My Folder");
            let file = folder.join("a,b.cdr");
            // dbus-send runs but no file manager takes the call (gdbus would get no answer either);
            // each fake has a name of its own, as `..f.tools()` writes the plain names again
            let t = Tools { dbus_send: f.tool("dbus-send-1", "exit 1"), ..f.tools() };
            reveal_with(&t, &file, SHOW_TIMEOUT).unwrap().join().unwrap();
            assert_eq!(f.args("xdg-open"), [folder.to_str().unwrap()]);
            assert!(!f.ran("gdbus"));
            // neither D-Bus tool is installed
            let t = Tools { dbus_send: f.missing(), gdbus: f.missing(), xdg_open: f.tool("xdg-open-2", "exit 0"), ..f.tools() };
            reveal_with(&t, &file, SHOW_TIMEOUT).unwrap().join().unwrap();
            assert_eq!(f.args("xdg-open-2"), [folder.to_str().unwrap()]);
            // a relative path is shown from the current folder
            let t = Tools { dbus_send: f.tool("dbus-send-3", "exit 0"), ..f.tools() };
            reveal_with(&t, Path::new("drawing.cdr"), SHOW_TIMEOUT).unwrap().join().unwrap();
            let cwd = std::env::current_dir().unwrap().join("drawing.cdr");
            let expected = format!("array:string:{}", super::super::file_uri(cwd.to_str().unwrap().as_bytes()));
            assert!(f.args("dbus-send-3").contains(&expected));
        }

        #[test]
        fn send_mail_hands_the_file_to_xdg_email() {
            let f = Fakes::new("mail");
            let file = f.dir.join("Logo & Card.cdr");
            std::fs::write(&file, b"x").unwrap();
            let mail = |tool: Vec<OsString>, subject: &str, grace: Duration| -> bool {
                let (tx, rx) = std::sync::mpsc::channel();
                let t = Tools { xdg_email: tool, ..f.tools() };
                let _ = send_mail(&t, subject, &file, grace, move |ok| {
                    let _ = tx.send(ok);
                });
                rx.recv_timeout(Duration::from_secs(10)).expect("xdg-email was not waited for")
            };
            assert!(mail(f.tool("xdg-email-1", "exit 0"), "Logo & Card", MAIL_GRACE));
            assert_eq!(f.args("xdg-email-1"), ["--utf8", "--subject", "Logo & Card", "--attach", file.to_str().unwrap()]);
            // no subject: xdg-email would refuse "--subject ''"
            assert!(mail(f.tool("xdg-email-2", "exit 0"), " ", MAIL_GRACE));
            assert_eq!(f.args("xdg-email-2"), ["--utf8", "--attach", file.to_str().unwrap()]);
            // no mail program: xdg-email ends with 3 ("a required tool could not be found")
            assert!(!mail(f.tool("xdg-email-3", "exit 3"), "Logo", MAIL_GRACE));
            // xdg-email is not installed
            assert!(!mail(f.missing(), "Logo", MAIL_GRACE));
            // still running after the grace time (Thunderbird -compose): the mail program has it
            let start = Instant::now();
            assert!(mail(f.tool("xdg-email-4", "sleep 3"), "Logo", Duration::from_millis(300)));
            assert!(start.elapsed() < Duration::from_secs(2));
        }

        #[test]
        fn started_programs_do_not_stay_zombies() {
            let child = std::process::Command::new("sh").args(["-c", "exit 0"]).spawn().unwrap();
            let proc = PathBuf::from(format!("/proc/{}", child.id()));
            reap(child, "sh".into());
            // a zombie keeps its /proc entry until it is waited for
            let start = Instant::now();
            while proc.exists() && start.elapsed() < Duration::from_secs(10) {
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(!proc.exists(), "{} was not waited for", proc.display());
        }
    }
}
