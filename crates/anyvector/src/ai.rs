//! AI Generate: turns a text prompt into editable vector objects.
//!
//! Asks the chosen AI provider (Anthropic's Messages API by default; any OpenAI-compatible
//! endpoint; Ollama or LM Studio on this computer, see `ai_provider.rs`) for a self-contained SVG,
//! which is imported as native AnyVector objects (paths, gradients, groups). Create image and
//! Remix image can use ComfyUI instead (`comfyui.rs`), whose image is placed as a bitmap.
//! API keys are read from the environment variable named in Settings (default
//! `ANTHROPIC_API_KEY`, or `OPENAI_API_KEY` for OpenAI-compatible endpoints); they are never
//! stored in settings or documents. Tools ▸ Options ▸ Global ▸ Privacy can write the key into
//! that variable of the user's Windows account, or on Linux of the user's systemd environment
//! (`~/.config/environment.d`, see [`environment_d`]). Local providers need no key.

use crate::ai_provider::{self as prov, ImageProvider, Provider};
use crate::app::AnyVectorApp;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// The default Anthropic model (Options can choose another).
pub const MODEL: &str = prov::ANTHROPIC_MODEL;

/// What AI Generate makes (Object ▸ AI Generate): an editable vector graphic, an image, or a
/// remix of a source image. Images and remixes are placed as bitmaps.
pub const MODES: [&str; 3] = ["Create vector graphic", "Create image", "Remix image"];

/// The selection loaded as reference (Create … Guided by Selection) or as the source of a
/// remix (Remix Selection).
pub struct Reference {
    /// The selection rendered as PNG.
    pub png: Vec<u8>,
    /// The selection as SVG (sent with a remix, and in place of the image to models that do not
    /// take images).
    pub svg: String,
    pub bounds: kurbo::Rect,
    pub count: usize,
}

pub const STYLES: [&str; 8] = ["Flat illustration", "Line art", "Logo / icon", "Isometric", "Geometric pattern", "Cartoon", "Infographic", "Silhouette"];

/// What a request brings back.
pub enum Output {
    Svg(String),
    /// An encoded image (ComfyUI).
    Bitmap(Vec<u8>),
}

/// The result and a note for the user (such as the reference going as SVG text).
type Reply = Result<(Output, Option<String>), String>;

/// A list fetched from a server in the background (models, ComfyUI checkpoints).
#[derive(Default)]
pub struct Fetch {
    pub items: Vec<String>,
    pub busy: bool,
    pub msg: Option<String>,
    rx: Option<Receiver<Result<Vec<String>, String>>>,
}

impl Fetch {
    pub fn start(&mut self, f: impl FnOnce() -> Result<Vec<String>, String> + Send + 'static) {
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.busy = true;
        self.msg = None;
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
    }

    fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        let Ok(r) = rx.try_recv() else { return };
        self.rx = None;
        self.busy = false;
        match r {
            Ok(items) => {
                self.msg = Some(if items.len() == 1 { "1 found.".to_string() } else { format!("{} found.", items.len()) });
                self.items = items;
            }
            Err(e) => self.msg = Some(e),
        }
    }
}

pub struct AiState {
    pub prompt: String,
    pub style: usize,
    pub colors: usize,
    pub busy: bool,
    pub status: String,
    pub error: Option<String>,
    /// The error is something to set up (an API key, a model): the panel offers to open the AI
    /// settings.
    pub error_no_key: bool,
    /// Quick AI Generate dialog visible.
    pub quick_open: bool,
    pub history: Vec<String>,
    /// 0 vector graphic, 1 image, 2 remix.
    pub mode: u8,
    pub reference: Option<Reference>,
    /// Something the user should know about the last result.
    pub note: Option<String>,
    /// Options ▸ AI Generate: the provider's models and ComfyUI's checkpoints.
    pub models: Fetch,
    pub checkpoints: Fetch,
    /// Options ▸ AI Generate: the result of loading a ComfyUI workflow (ok, message).
    pub workflow_msg: Option<(bool, String)>,
    rx: Option<Receiver<Reply>>,
    cancel: Arc<AtomicBool>,
    started: f64,
    /// The request's mode and where its reference was.
    pending: (u8, Option<kurbo::Rect>),
}

impl Default for AiState {
    fn default() -> Self {
        AiState {
            prompt: String::new(),
            style: 0,
            colors: 6,
            busy: false,
            status: String::new(),
            error: None,
            error_no_key: false,
            quick_open: false,
            history: vec![],
            mode: 0,
            reference: None,
            note: None,
            models: Fetch::default(),
            checkpoints: Fetch::default(),
            workflow_msg: None,
            rx: None,
            cancel: Arc::default(),
            started: 0.0,
            pending: (0, None),
        }
    }
}

fn system_prompt() -> &'static str {
    "You generate artwork for a professional vector graphics editor. Reply with exactly one complete, \
     self-contained SVG 1.1 document and nothing else: no prose, no markdown fences. Requirements: \
     a viewBox; only shapes, paths, groups, linear/radial gradients and clip paths; no <text>, \
     no <image>, no scripts, no external references, no CSS animations, no filters. Use clean, \
     well-structured closed paths that a designer can edit, grouped logically with descriptive ids. \
     Keep it under 400 elements."
}

/// The environment variable that holds the chosen provider's API key (none for the local
/// providers).
pub fn key_var(app: &AnyVectorApp) -> Option<String> {
    let (v, d) = match app.settings.ai_config.provider {
        Provider::Anthropic => (app.settings.ai_api_key_env.trim(), "ANTHROPIC_API_KEY"),
        Provider::OpenAi => (app.settings.ai_config.openai_key_env.trim(), prov::OPENAI_KEY_ENV),
        Provider::Ollama | Provider::LmStudio => return None,
    };
    Some(if v.is_empty() { d.to_string() } else { v.to_string() })
}

/// The API key: the environment of this process, then the variable of the user's Windows account
/// (so a key saved in Options, or with Windows' Environment Variables, works without a restart); on
/// Linux the user's environment.d files stand for the Windows account.
pub fn api_key(app: &AnyVectorApp) -> Option<String> {
    let name = key_var(app)?;
    std::env::var(&name).ok().or_else(|| user_env(&name)).filter(|k| !k.trim().is_empty())
}

/// The chosen provider needs an API key: hosted services do, servers on this computer don't.
pub fn needs_key(app: &AnyVectorApp) -> bool {
    let c = &app.settings.ai_config;
    match c.provider {
        Provider::Anthropic => true,
        Provider::OpenAi => !prov::is_local_url(&c.url("")),
        Provider::Ollama | Provider::LmStudio => false,
    }
}

/// Where the AI settings are, for messages.
pub const KEY_PLACE: &str = "Tools ▸ Options ▸ Global ▸ Privacy ▸ AI Generate";

#[cfg(windows)]
fn user_env(name: &str) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ};
    let (sub, value) = (HSTRING::from("Environment"), HSTRING::from(name));
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ;
    let mut size = 0u32;
    unsafe { RegGetValueW(HKEY_CURRENT_USER, &sub, &value, flags, None, None, Some(&mut size)) }.ok().ok()?;
    let mut buf = vec![0u16; size as usize / 2 + 1];
    let mut size = (buf.len() * 2) as u32;
    unsafe { RegGetValueW(HKEY_CURRENT_USER, &sub, &value, flags, None, Some(buf.as_mut_ptr().cast()), Some(&mut size)) }.ok().ok()?;
    let n = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..n]).trim().to_string())
}

#[cfg(not(windows))]
fn user_env(name: &str) -> Option<String> {
    environment_d::lookup(&environment_d::dir()?, name)
}

/// The user's own environment variables on Linux, the counterpart of Windows' user environment
/// (HKCU\Environment): systemd's environment.d(5) files in `$XDG_CONFIG_HOME/environment.d`
/// (normally `~/.config/environment.d`). The systemd user manager reads them when the user logs in
/// and hands the variables to the session, so the desktop (GNOME, KDE...) and the programs it starts
/// see them. Save Key keeps its keys in `anyvector.conf` there, readable by the user alone.
#[cfg(any(not(windows), test))]
#[cfg_attr(windows, allow(dead_code))]
pub mod environment_d {
    use std::io;
    use std::path::{Path, PathBuf};

    /// The file Save Key writes.
    pub const FILE: &str = "anyvector.conf";

    const HEADER: &str = "# AnyVector's AI Generate API keys (Tools > Options > Global > Privacy > AI Generate).\n\
# systemd gives these environment variables to the programs of your session when you log in\n\
# (man environment.d). Remove Key in AnyVector, or deleting a line here, removes a key.\n";

    /// `$XDG_CONFIG_HOME/environment.d`.
    pub fn dir() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("environment.d"))
    }

    /// The file Save Key writes, as messages show it (`~/.config/environment.d/anyvector.conf`).
    pub fn shown_path() -> String {
        let Some(path) = dir().map(|d| d.join(FILE)) else { return format!("~/.config/environment.d/{FILE}") };
        match dirs::home_dir().and_then(|h| path.strip_prefix(h).ok().map(Path::to_path_buf)) {
            Some(rel) => format!("~/{}", rel.display()),
            None => path.display().to_string(),
        }
    }

    /// The value the files of `dir` give the variable `name`, as systemd reads them: every `*.conf`
    /// in the order of their names, the last assignment winning, `$OTHER` / `${OTHER}` taken from
    /// the assignments before it or from this process's environment. An empty value is no value.
    pub fn lookup(dir: &Path, name: &str) -> Option<String> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "conf") && p.is_file()).collect();
        files.sort();
        let mut vars: Vec<(String, String)> = vec![];
        for file in files {
            let Ok(text) = std::fs::read_to_string(&file) else { continue };
            for (k, v) in parse(&text) {
                // systemd skips names it doesn't take and assignments without a value
                if !valid_name(&k) || v.is_empty() {
                    continue;
                }
                let v = expand(&v, &vars);
                vars.retain(|(n, _)| *n != k);
                vars.push((k, v));
            }
        }
        vars.into_iter().find(|(k, _)| k == name).map(|(_, v)| v).filter(|v| !v.trim().is_empty())
    }

    /// A variable name systemd takes: letters, digits and `_`, not starting with a digit.
    pub fn valid_name(name: &str) -> bool {
        name.bytes().next().is_some_and(|b| !b.is_ascii_digit()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    }

    /// The `KEY=value` assignments of an environment.d file, unquoted, in order. The syntax of
    /// systemd's environment files: `#` and `;` comments, values in '…' or "…" (with `\` escapes),
    /// `\` before a line break continues the line; space around an unquoted value is dropped.
    pub fn parse(text: &str) -> Vec<(String, String)> {
        #[derive(Clone, Copy, PartialEq)]
        enum S {
            PreKey,
            Key,
            PreValue,
            Value,
            ValueEscape,
            Single,
            Double,
            DoubleEscape,
            Comment,
        }
        let mut out = vec![];
        let (mut key, mut value) = (String::new(), String::new());
        // unquoted spaces at the end of the value, dropped unless more text follows
        let mut spaces = String::new();
        let mut state = S::PreKey;
        let mut finish = |key: &mut String, value: &mut String, spaces: &mut String| {
            let k = key.trim().to_string();
            if !k.is_empty() {
                out.push((k, std::mem::take(value)));
            }
            key.clear();
            value.clear();
            spaces.clear();
        };
        for c in text.chars() {
            state = match (state, c) {
                (S::PreKey, '#' | ';') => S::Comment,
                (S::PreKey, c) if c.is_whitespace() => S::PreKey,
                (S::PreKey, c) => {
                    key.push(c);
                    S::Key
                }
                // a line without '=' is no assignment
                (S::Key, '\n' | '\r') => {
                    key.clear();
                    S::PreKey
                }
                (S::Key, '=') => S::PreValue,
                (S::Key, c) => {
                    key.push(c);
                    S::Key
                }
                (S::PreValue | S::Value, '\n' | '\r') => {
                    finish(&mut key, &mut value, &mut spaces);
                    S::PreKey
                }
                (S::PreValue, c) if c.is_whitespace() => S::PreValue,
                (S::PreValue, '\'') => S::Single,
                (S::PreValue, '"') => S::Double,
                (S::PreValue | S::Value, '\\') => {
                    value.push_str(&std::mem::take(&mut spaces));
                    S::ValueEscape
                }
                (S::PreValue | S::Value, c) if c.is_whitespace() => {
                    spaces.push(c);
                    S::Value
                }
                (S::PreValue | S::Value, c) => {
                    value.push_str(&std::mem::take(&mut spaces));
                    value.push(c);
                    S::Value
                }
                (S::ValueEscape, '\n' | '\r') => S::Value,
                (S::ValueEscape, c) => {
                    value.push(c);
                    S::Value
                }
                (S::Single, '\'') => S::PreValue,
                (S::Single, c) => {
                    value.push(c);
                    S::Single
                }
                (S::Double, '"') => S::PreValue,
                (S::Double, '\\') => S::DoubleEscape,
                (S::Double, c) => {
                    value.push(c);
                    S::Double
                }
                (S::DoubleEscape, '\n') => S::Double,
                (S::DoubleEscape, c) => {
                    if !"\"\\`$".contains(c) {
                        value.push('\\');
                    }
                    value.push(c);
                    S::Double
                }
                // a `\` at the end of a comment doesn't continue it (systemd 254 and later)
                (S::Comment, '\n' | '\r') => S::PreKey,
                (S::Comment, _) => S::Comment,
            };
        }
        // the last line needs no line break, and a quote left open still gives its value
        if matches!(state, S::PreValue | S::Value | S::ValueEscape | S::Single | S::Double | S::DoubleEscape) {
            finish(&mut key, &mut value, &mut spaces);
        }
        out
    }

    /// `$NAME`, `${NAME}`, `${NAME:-default}`, `${NAME:+alternate}` and `$$` (a `$`), as systemd
    /// expands environment.d values: `vars` first, then this process's environment.
    pub fn expand(v: &str, vars: &[(String, String)]) -> String {
        let get = |n: &str| vars.iter().rev().find(|(k, _)| k == n).map(|(_, v)| v.clone()).or_else(|| std::env::var(n).ok()).unwrap_or_default();
        let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_';
        let mut out = String::with_capacity(v.len());
        let mut rest = v;
        while let Some(i) = rest.find('$') {
            out.push_str(&rest[..i]);
            let after = &rest[i + 1..];
            if let Some(tail) = after.strip_prefix('$') {
                out.push('$');
                rest = tail;
            } else if let Some(body) = after.strip_prefix('{') {
                let Some(end) = body.find('}') else {
                    // no closing brace: as written
                    out.push_str(&rest[i..]);
                    return out;
                };
                let inner = &body[..end];
                out.push_str(&match (inner.split_once(":-"), inner.split_once(":+")) {
                    (Some((n, default)), _) => Some(get(n)).filter(|x| !x.is_empty()).unwrap_or_else(|| expand(default, vars)),
                    (None, Some((n, alternate))) => if get(n).is_empty() { String::new() } else { expand(alternate, vars) },
                    _ => get(inner),
                });
                rest = &body[end + 1..];
            } else {
                let n = after.find(|c: char| !is_name(c)).unwrap_or(after.len());
                if n == 0 {
                    out.push('$');
                } else {
                    out.push_str(&get(&after[..n]));
                }
                rest = &after[n..];
            }
        }
        out.push_str(rest);
        out
    }

    /// A value as written into the file: as it is when it is plain, else in double quotes with
    /// `"`, `\` and `` ` `` escaped and `$` doubled (systemd would expand it).
    pub fn quote(v: &str) -> String {
        if !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || "-_.:/+=@,%^~".contains(c)) {
            return v.to_string();
        }
        let mut out = String::from("\"");
        for c in v.chars() {
            match c {
                '"' | '\\' | '`' => {
                    out.push('\\');
                    out.push(c);
                }
                '$' => out.push_str("$$"),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }

    /// The name a line assigns (`NAME=…`), if it is an assignment.
    fn assigned(line: &str) -> Option<&str> {
        let l = line.trim_start();
        if l.starts_with(['#', ';']) {
            return None;
        }
        l.split_once('=').map(|(k, _)| k.trim()).filter(|k| !k.is_empty())
    }

    /// Sets `name` to `value` in `dir`'s [`FILE`], or removes it (`None`), keeping the file's other
    /// lines. The file is created as needed, readable and writable by the user alone (it holds
    /// secrets), and deleted when it has nothing left to set.
    pub fn store(dir: &Path, name: &str, value: Option<&str>) -> io::Result<()> {
        let path = dir.join(FILE);
        let old = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let mut lines: Vec<String> = old.lines().filter(|l| assigned(l) != Some(name)).map(str::to_string).collect();
        if let Some(v) = value {
            lines.push(format!("{name}={}", quote(v)));
        }
        if !lines.iter().any(|l| assigned(l).is_some()) {
            return match std::fs::remove_file(&path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        let mut text = if old.trim().is_empty() { HEADER.to_string() } else { String::new() };
        for l in lines {
            text.push_str(&l);
            text.push('\n');
        }
        std::fs::create_dir_all(dir)?;
        write_private(&path, &text)
    }

    /// Writes the file whole or not at all (through a temporary file systemd doesn't read), with
    /// permissions for the user alone.
    fn write_private(path: &Path, text: &str) -> io::Result<()> {
        use std::io::Write;
        let tmp = path.with_extension("conf.tmp");
        let _ = std::fs::remove_file(&tmp);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut f = options.open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    }

    /// Tells the running systemd user manager, when there is one, so the programs it starts from now
    /// on (services, D-Bus activated programs) have the variable before the next login. The key goes
    /// through the environment of `systemctl`, never its command line, in the background; without
    /// systemd nothing happens (the file counts from the next login).
    #[cfg(not(windows))]
    pub fn tell_session(name: &str, value: Option<&str>) {
        use std::process::{Command, Stdio};
        let mut cmd = Command::new("systemctl");
        cmd.arg("--user").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        match value {
            Some(v) => cmd.args(["import-environment", name]).env(name, v),
            None => cmd.args(["unset-environment", name]),
        };
        std::thread::spawn(move || {
            let _ = cmd.status();
        });
    }
}

/// The variable name and the key can go into the environment: `=` and NUL would make the system
/// refuse it (and the program stop); on Linux the name must also be one systemd accepts and the key
/// a single line.
fn valid_key(name: &str, key: &str) -> bool {
    if name.contains(['=', '\0']) || key.contains('\0') {
        return false;
    }
    #[cfg(not(windows))]
    if !environment_d::valid_name(name) || key.chars().any(char::is_control) {
        return false;
    }
    true
}

/// The message after Save Key: where the key went.
pub fn key_saved_message(var: &str) -> String {
    if cfg!(windows) {
        format!("The key was saved in the {var} environment variable of your Windows account. AI Generate is ready.")
    } else {
        format!("The key was saved in the {var} environment variable of your user account. AI Generate is ready.")
    }
}

/// What Save Key does, under the key.
pub fn save_key_note() -> String {
    #[cfg(windows)]
    {
        "Save Key writes the key into this environment variable of your Windows account, right away. AnyVector never saves the key in its own settings or documents.".to_string()
    }
    #[cfg(not(windows))]
    {
        format!(
            "Save Key writes the key into this environment variable of your user account ({}), right away; other programs see it after you log in again. AnyVector never saves the key in its own settings or documents.",
            environment_d::shown_path()
        )
    }
}

/// Write the API key into the environment variable `name` of the user's Windows account (on Linux
/// the user's systemd environment, [`environment_d`]); an empty key removes it. Also into this
/// process's environment. Other programs see it when they next start (on Linux, after the next
/// login, or when systemd starts them).
pub fn save_user_key(name: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    if name.trim().is_empty() {
        return Err("Type the name of the environment variable first.".into());
    }
    if !valid_key(name.trim(), key) {
        return Err("The name of the environment variable or the key has characters that cannot be saved.".into());
    }
    // tests never write the user's environment
    #[cfg(not(windows))]
    if !cfg!(test) {
        let name = name.trim();
        let dir = environment_d::dir().ok_or_else(|| format!("Could not save {}:\n{}", environment_d::shown_path(), std::io::Error::from(std::io::ErrorKind::NotFound)))?;
        let value = (!key.is_empty()).then_some(key);
        environment_d::store(&dir, name, value).map_err(|e| format!("Could not save {}:\n{e}", dir.join(environment_d::FILE).display()))?;
        environment_d::tell_session(name, value);
    }
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, LPARAM, WPARAM};
        use windows::Win32::System::Registry::{RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
        use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE};
        let (sub, value) = (HSTRING::from("Environment"), HSTRING::from(name.trim()));
        let r = if key.is_empty() {
            let r = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, &sub, &value) };
            if r == ERROR_FILE_NOT_FOUND { ERROR_SUCCESS } else { r }
        } else {
            let data: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, &sub, &value, REG_SZ.0, Some(data.as_ptr().cast()), (data.len() * 2) as u32) }
        };
        if r != ERROR_SUCCESS {
            return Err(format!("Could not save the environment variable (error {}).", r.0));
        }
        // tell running programs (Explorer) that the environment changed, without waiting on them
        std::thread::spawn(|| {
            let env: Vec<u16> = "Environment".encode_utf16().chain(std::iter::once(0)).collect();
            unsafe { SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, WPARAM(0), LPARAM(env.as_ptr() as isize), SMTO_ABORTIFHUNG, 2000, None) };
        });
    }
    if key.is_empty() {
        std::env::remove_var(name.trim());
    } else {
        std::env::set_var(name.trim(), key);
    }
    Ok(())
}

/// The request target for the chosen provider, or what to set up first.
pub fn target(app: &AnyVectorApp, need_model: bool) -> Result<prov::Target, String> {
    let c = &app.settings.ai_config;
    let key = api_key(app);
    if key.is_none() && needs_key(app) {
        let var = key_var(app).unwrap_or_default();
        if cfg!(windows) {
            return Err(format!("AI Generate needs an API key. Add your key in {KEY_PLACE}: AnyVector keeps it in the {var} environment variable of your Windows account, never in its own files."));
        }
        return Err(format!("AI Generate needs an API key. Add your key in {KEY_PLACE}: AnyVector keeps it in the {var} environment variable of your user account, never in its settings or documents."));
    }
    let model = c.model();
    if need_model && model.is_empty() {
        let extra = if c.provider.is_local() { " Refresh models lists the models installed." } else { "" };
        return Err(format!("Choose the model to use in {KEY_PLACE}.{extra}"));
    }
    Ok(prov::Target { provider: c.provider, url: c.url(&app.settings.ai_provider_url), model, key, vision: c.vision(), ollama_native: c.ollama_native, timeout: c.timeout() })
}

/// Options ▸ AI Generate ▸ Refresh models.
pub fn refresh_models(app: &mut AnyVectorApp) {
    match target(app, false) {
        Ok(t) => app.ai.models.start(move || prov::list_models(&t)),
        Err(e) => app.ai.models.msg = Some(e),
    }
}

/// Options ▸ AI Generate ▸ ComfyUI checkpoint ▸ Refresh.
pub fn refresh_checkpoints(app: &mut AnyVectorApp) {
    let url = app.settings.ai_config.comfy_url.clone();
    app.ai.checkpoints.start(move || crate::comfyui::list_checkpoints(&url));
}

/// The ComfyUI workflow to run: the user's file, or the built-in text-to-image or (with a source
/// image) image-to-image one; and a note when the user's workflow cannot use the source.
fn comfy_workflow(cfg: &prov::AiConfig, has_source: bool) -> Result<(serde_json::Value, Option<String>), String> {
    let path = cfg.comfy_workflow.trim();
    if path.is_empty() {
        let text = if has_source { crate::comfyui::IMAGE_TO_IMAGE } else { crate::comfyui::TEXT_TO_IMAGE };
        return Ok((crate::comfyui::parse_workflow(text)?, None));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("Could not read the ComfyUI workflow {path}: {e}"))?;
    let wf = crate::comfyui::parse_workflow(&text)?;
    let note = (has_source && !crate::comfyui::wants_image(&wf)).then(|| "The ComfyUI workflow has no %image% placeholder, so the selection was not used.".to_string());
    Ok((wf, note))
}

/// The request's text: with the reference as an attached image, and with the reference as SVG
/// text instead (for models that do not take images).
pub fn user_texts(mode: u8, prompt: &str, style: &str, colors: usize, svg: Option<&str>) -> (String, String) {
    let mut user = format!("Create: {}\nStyle: {style}\nUse a palette of about {colors} harmonious colors.", prompt.trim());
    if mode == 1 {
        user.push_str("\nThis becomes a bitmap image: make it rich and detailed, with gradients, shading and a full background.");
    }
    let Some(svg) = svg else { return (user.clone(), user) };
    let svg: String = svg.chars().take(60_000).collect();
    if mode == 2 {
        (
            format!("Remix the attached artwork: keep its key elements and composition, and change it as described.\n{user}\nThe artwork's SVG source:\n{svg}"),
            format!("Remix the artwork whose SVG source is below: keep its key elements and composition, and change it as described.\n{user}\nThe artwork's SVG source:\n{svg}"),
        )
    } else {
        (
            format!("Use the attached artwork as the reference for the composition, proportions and content.\n{user}"),
            format!("Use the artwork whose SVG source is below as the reference for the composition, proportions and content.\n{user}\nThe artwork's SVG source:\n{svg}"),
        )
    }
}

/// Start a generation request in the background.
pub fn start(app: &mut AnyVectorApp) {
    if app.ai.busy || app.ai.prompt.trim().is_empty() {
        return;
    }
    let cfg = app.settings.ai_config.clone();
    let style = STYLES[app.ai.style.min(STYLES.len() - 1)];
    let source = app.ai.reference.as_ref().map(|r| r.png.clone());
    type Job = Box<dyn FnOnce(&AtomicBool) -> Reply + Send>;
    let job: Job = if app.ai.mode != 0 && cfg.image_provider == ImageProvider::ComfyUi {
        let (workflow, note) = match comfy_workflow(&cfg, source.is_some()) {
            Ok(w) => w,
            Err(e) => {
                app.ai.error = Some(e);
                app.ai.error_no_key = true;
                return;
            }
        };
        let params = crate::comfyui::Params {
            prompt: format!("{}, {} style", app.ai.prompt.trim(), style.to_lowercase()),
            negative: cfg.comfy_negative.clone(),
            seed: crate::comfyui::random_seed(),
            steps: cfg.comfy_steps.clamp(1, 200),
            width: cfg.comfy_width.clamp(64, 8192),
            height: cfg.comfy_height.clamp(64, 8192),
            checkpoint: cfg.comfy_checkpoint.trim().to_string(),
            image: None,
            denoise: if app.ai.mode == 2 { 0.55 } else { 0.75 },
        };
        let (url, timeout) = (cfg.comfy_url.clone(), cfg.timeout());
        Box::new(move |cancel| crate::comfyui::generate(&url, &workflow, params, source.as_deref(), cancel, timeout).map(|b| (Output::Bitmap(b), note)))
    } else {
        let t = match target(app, true) {
            Ok(t) => t,
            Err(e) => {
                app.ai.error = Some(e);
                app.ai.error_no_key = true;
                return;
            }
        };
        let svg = app.ai.reference.as_ref().map(|r| r.svg.as_str());
        let (with_image, plain) = user_texts(app.ai.mode, &app.ai.prompt, style, app.ai.colors, svg);
        Box::new(move |_| {
            let (text, note) = prov::chat(&t, system_prompt(), &with_image, &plain, source.as_deref())?;
            let svg = extract_svg(&prov::strip_think(&text)).ok_or("The AI did not return a drawing. Try rephrasing the prompt.")?;
            Ok((Output::Svg(svg), note))
        })
    };
    app.ai.pending = (app.ai.mode, app.ai.reference.as_ref().map(|r| r.bounds));
    let (tx, rx) = channel();
    app.ai.rx = Some(rx);
    app.ai.busy = true;
    app.ai.error = None;
    app.ai.error_no_key = false;
    app.ai.note = None;
    app.ai.status = "Generating...".into();
    app.ai.started = app.time();
    if !app.ai.history.contains(&app.ai.prompt) {
        app.ai.history.insert(0, app.ai.prompt.clone());
        app.ai.history.truncate(12);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    app.ai.cancel = cancel.clone();
    std::thread::spawn(move || {
        let _ = tx.send(job(&cancel));
    });
}

/// Stop waiting for the request. ComfyUI is told to drop the job; an AI model's late answer is
/// ignored.
pub fn cancel(app: &mut AnyVectorApp) {
    app.ai.cancel.store(true, Ordering::Relaxed);
    app.ai.rx = None;
    app.ai.busy = false;
    app.ai.pending = (0, None);
    app.ai.status = "Cancelled".into();
}

/// Standard base64 (for the reference image).
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Object ▸ AI Generate ▸ Create Vector Graphic / Create Image Guided by Selection and Remix
/// Selection: load the selection into the AI Generate docker.
pub fn guided(app: &mut AnyVectorApp, mode: u8) {
    let Some(d) = app.doc() else { return };
    let objs: Vec<vector_core::Object> = d.selection.iter().filter_map(|id| d.doc.find(*id).cloned()).collect();
    if objs.is_empty() {
        return;
    }
    let bounds = vector_core::object::union_bounds(objs.iter().map(|o| d.doc.bounds_of(o)));
    // render the selection alone, at most 1024 pixels across
    let mut tmp = d.doc.clone();
    for p in tmp.pages.iter_mut() {
        for l in p.layers.iter_mut() {
            l.objects.clear();
        }
    }
    let page = d.page;
    if let Some(l) = tmp.pages[page].layers.first_mut() {
        l.objects = objs.clone();
    }
    let dpi = (1024.0 * 72.0 / bounds.width().max(bounds.height()).max(1.0)).min(300.0);
    let png = vector_render::render_page_image(&tmp, page, dpi, Some(vector_core::Color::WHITE), Some(bounds), true).and_then(|pm| pm.encode_png().ok()).unwrap_or_default();
    let svg = vector_io::svg_export::objects_to_svg(&d.doc, &objs);
    app.ai.reference = Some(Reference { png, svg, bounds, count: objs.len() });
    app.ai.mode = mode;
    app.ai.error = None;
    app.dockers.show(crate::dockers::DockerId::AiGenerate);
}

fn extract_svg(text: &str) -> Option<String> {
    let start = text.find("<svg")?;
    let end = text.rfind("</svg>")? + "</svg>".len();
    (end > start).then(|| text[start..end].to_string())
}

/// Poll the background request; on success insert the objects into the active document.
pub fn poll(app: &mut AnyVectorApp) {
    app.ai.models.poll();
    app.ai.checkpoints.poll();
    let Some(rx) = &app.ai.rx else { return };
    let Ok(res) = rx.try_recv() else {
        let secs = (app.time() - app.ai.started) as u64;
        app.ai.status = format!("Generating... {secs}s");
        return;
    };
    app.ai.rx = None;
    app.ai.busy = false;
    let (mode, target) = std::mem::take(&mut app.ai.pending);
    match res {
        Err(e) => {
            app.ai.status.clear();
            app.ai.error = Some(e);
        }
        Ok((out, note)) => {
            app.ai.note = note;
            if !app.has_doc() {
                app.new_default_document();
            }
            let name = format!("AI: {}", app.ai.prompt.chars().take(40).collect::<String>());
            match out {
                Output::Svg(svg) => place_svg(app, &svg, mode, target, name),
                Output::Bitmap(bytes) => {
                    let Some(d) = app.doc_mut() else { return };
                    let page = d.doc.pages[d.page].rect();
                    match bitmap_from_bytes(&mut d.doc, &bytes, page, target, &name) {
                        Ok(o) => {
                            d.add_objects("AI Generate", vec![o]);
                            app.ai.status = "Done".into();
                        }
                        Err(e) => app.ai.error = Some(e),
                    }
                }
            }
        }
    }
}

/// Import the generated SVG: as a group of vector objects, or (images and remixes) a bitmap.
fn place_svg(app: &mut AnyVectorApp, svg: &str, mode: u8, target: Option<kurbo::Rect>, name: String) {
    let Some(d) = app.doc_mut() else { return };
    match vector_io::svg_import::objects_from_svg(&mut d.doc, svg.as_bytes()) {
        Ok(mut objs) if !objs.is_empty() => {
            // fit into the page at 80% and centre; a guided result goes beside its
            // reference, the same size
            let page = d.doc.pages[d.page].rect();
            let area = match target {
                Some(t) => t + kurbo::Vec2::new(t.width() + 18.0, 0.0),
                None => page.scale_from_origin(0.8) + (page.center() - page.scale_from_origin(0.8).center()),
            };
            let bb = vector_core::object::union_bounds(objs.iter().map(|o| o.bounds()));
            if bb.width() > 0.0 && bb.height() > 0.0 {
                let s = (area.width() / bb.width()).min(area.height() / bb.height());
                let a = kurbo::Affine::translate(area.center().to_vec2()) * kurbo::Affine::scale(s) * kurbo::Affine::translate(-bb.center().to_vec2());
                for o in objs.iter_mut() {
                    o.apply_transform(a);
                }
            }
            let group = if mode == 0 {
                let g = d.doc.new_id();
                let mut group = vector_core::Object::new(g, vector_core::ObjKind::Group(objs), kurbo::Affine::IDENTITY, Default::default());
                group.name = name;
                group
            } else {
                // images and remixes are bitmaps
                let fit = vector_core::object::union_bounds(objs.iter().map(|o| o.bounds()));
                match bitmap_of(&mut d.doc, svg, fit, &name) {
                    Some(o) => o,
                    None => {
                        app.ai.error = Some("Could not render the generated image.".into());
                        return;
                    }
                }
            };
            if let Some(d) = app.doc_mut() {
                d.add_objects("AI Generate", vec![group]);
            }
            app.ai.status = "Done".into();
        }
        Ok(_) => app.ai.error = Some("The generated drawing was empty.".into()),
        Err(e) => app.ai.error = Some(format!("Could not read the generated drawing: {e}")),
    }
}

/// Where an image of `w` × `h` pixels goes: beside the reference at its height, or centred in
/// the page at 80%.
pub fn bitmap_rect(page: kurbo::Rect, target: Option<kurbo::Rect>, w: u32, h: u32) -> kurbo::Rect {
    let aspect = w.max(1) as f64 / h.max(1) as f64;
    match target {
        Some(t) => {
            let ht = t.height().max(1.0);
            kurbo::Rect::from_origin_size((t.x1 + 18.0, t.y0), (ht * aspect, ht))
        }
        None => {
            let (aw, ah) = (page.width() * 0.8, page.height() * 0.8);
            let (bw, bh) = if aw / ah > aspect { (ah * aspect, ah) } else { (aw, aw / aspect) };
            kurbo::Rect::from_center_size(page.center(), (bw, bh))
        }
    }
}

/// An encoded image (ComfyUI's output) as a bitmap object.
fn bitmap_from_bytes(doc: &mut vector_core::Document, bytes: &[u8], page: kurbo::Rect, target: Option<kurbo::Rect>, name: &str) -> Result<vector_core::Object, String> {
    let fmt = image::guess_format(bytes).ok();
    let img = image::load_from_memory(bytes).map_err(|e| format!("Could not read the generated image: {e}"))?.to_rgba8();
    let (w, h) = img.dimensions();
    let at = bitmap_rect(page, target, w, h);
    // keep PNG and JPEG files as they came, for lossless saving
    let (encoded, format) = match fmt {
        Some(image::ImageFormat::Png) => (bytes.to_vec(), "png"),
        Some(image::ImageFormat::Jpeg) => (bytes.to_vec(), "jpg"),
        _ => (vec![], "png"),
    };
    let data = vector_core::document::ImageData { width: w, height: h, rgba: Arc::new(img.into_raw()), encoded: Arc::new(encoded), format: format.into(), name: name.to_string() };
    Ok(image_object(doc, data, at, name))
}

fn image_object(doc: &mut vector_core::Document, data: vector_core::document::ImageData, at: kurbo::Rect, name: &str) -> vector_core::Object {
    let iid = doc.add_image(data);
    let mut st = vector_core::Style::default();
    st.fill = vector_core::Fill::None;
    st.outline = vector_core::Outline::none();
    let io = vector_core::object::ImageObj { image: iid, w: at.width(), h: at.height(), crop: [0.0, 0.0, 1.0, 1.0], adjust: Default::default() };
    let mut o = vector_core::Object::new(doc.new_id(), vector_core::ObjKind::Image(io), kurbo::Affine::translate(at.origin().to_vec2()), st);
    o.name = name.to_string();
    o
}

/// The generated SVG as a bitmap object filling `at` (300 dpi, at most 2048 pixels).
fn bitmap_of(doc: &mut vector_core::Document, svg: &str, at: kurbo::Rect, name: &str) -> Option<vector_core::Object> {
    let k = (300.0 / 72.0f64).min(2048.0 / at.width().max(at.height()).max(1.0));
    let (w, h) = ((at.width() * k).round().max(1.0) as u32, (at.height() * k).round().max(1.0) as u32);
    let img = crate::icons::rasterize_svg(svg, w, h)?;
    let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    let data = vector_core::document::ImageData { width: w, height: h, rgba: Arc::new(rgba), encoded: Arc::new(vec![]), format: "png".into(), name: name.to_string() };
    Some(image_object(doc, data, at, name))
}

/// Quick AI Generate dialog (File > Quick AI Generate...).
pub fn quick_dialog(app: &mut AnyVectorApp, ctx: &egui::Context) {
    if !app.ai.quick_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Quick AI Generate").open(&mut open).collapsible(false).resizable(false).default_width(420.0).pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center()).show(ctx, |ui| {
        panel(app, ui);
    });
    if !open {
        app.ai.quick_open = false;
    }
}

/// The Generative AI terms, shown until they are accepted: what the AI features need and send
/// (to the chosen provider), the policy, and I agree / I do not agree.
fn terms(app: &mut AnyVectorApp, ui: &mut egui::Ui) {
    let policy = prov::policy_url(&app.settings.ai_config, &app.settings.ai_provider_url);
    ui.add_space(10.0);
    ui.label(egui::RichText::new("Generative AI Terms").font(crate::theme::bold(13.0)));
    ui.add_space(10.0);
    if policy.is_some() {
        ui.label("To use the AI features of AnyVector, you must be at least 18 years old and accept the Generative AI Policy. If you are under 18 or do not accept it, do not use these features.");
    } else {
        ui.label("To use the AI features of AnyVector, you must be at least 18 years old. If you are under 18, do not use these features.");
    }
    ui.add_space(10.0);
    ui.label(prov::disclosure(&app.settings.ai_config, &app.settings.ai_provider_url));
    ui.add_space(14.0);
    if let Some(url) = policy {
        ui.vertical_centered(|ui| {
            if ui.add(egui::Button::new("Generative AI Policy...").min_size(egui::vec2(150.0, 24.0))).clicked() {
                crate::bugreport::open_in_browser(url);
            }
        });
        ui.add_space(14.0);
        ui.label("By clicking 'I agree', you accept the Generative AI Policy and confirm that you are at least 18 years old.");
    } else {
        ui.label("By clicking 'I agree', you confirm that you are at least 18 years old and that you follow the terms of the AI service or model you use.");
    }
    ui.add_space(14.0);
    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 26.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.add(egui::Button::new("I do not agree").min_size(egui::vec2(90.0, 24.0))).clicked() {
            app.dockers.close(crate::dockers::DockerId::AiGenerate);
        }
        if ui.add(egui::Button::new("I agree").min_size(egui::vec2(60.0, 24.0))).clicked() {
            app.settings.ai_terms_accepted = true;
            app.settings.save();
        }
    });
}

/// Shared UI used by the AI Generate docker and the quick dialog.
pub fn panel(app: &mut AnyVectorApp, ui: &mut egui::Ui) {
    if !app.settings.ai_terms_accepted {
        return terms(app, ui);
    }
    ui.horizontal_wrapped(|ui| {
        for (i, m) in MODES.iter().enumerate() {
            ui.selectable_value(&mut app.ai.mode, i as u8, *m);
        }
    });
    let mut clear = false;
    if let Some(r) = &app.ai.reference {
        ui.horizontal(|ui| {
            let what = if app.ai.mode == 2 { "Source image" } else { "Reference image" };
            ui.label(egui::RichText::new(format!("{what}: the selection ({} objects)", r.count)).color(crate::theme::TEXT_DIM()));
            clear = ui.small_button("Clear reference content").clicked();
        });
    } else if app.ai.mode == 2 {
        ui.label(egui::RichText::new("Select the artwork to remix, then choose Object ▸ AI Generate ▸ Remix Selection.").color(crate::theme::TEXT_DIM()));
    }
    if clear {
        app.ai.reference = None;
    }
    ui.label("Describe what you want to create:");
    ui.add(egui::TextEdit::multiline(&mut app.ai.prompt).desired_rows(4).desired_width(f32::INFINITY).hint_text("e.g. a hot-air balloon over rolling hills"));
    ui.horizontal(|ui| {
        ui.label("Style:");
        egui::ComboBox::from_id_salt("ai_style").selected_text(STYLES[app.ai.style]).width(170.0).show_ui(ui, |ui| {
            for (i, s) in STYLES.iter().enumerate() {
                ui.selectable_value(&mut app.ai.style, i, *s);
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label("Colors:");
        ui.add(egui::Slider::new(&mut app.ai.colors, 1..=16));
    });
    ui.add_space(6.0);
    let mut stop = false;
    ui.horizontal(|ui| {
        let can = !app.ai.busy && app.ai.prompt.trim().chars().count() >= 3 && (app.ai.mode != 2 || app.ai.reference.is_some());
        let b = egui::Button::new(egui::RichText::new("✦ Generate").color(egui::Color32::WHITE)).fill(crate::theme::PRIMARY()).min_size(egui::vec2(100.0, 26.0));
        if ui.add_enabled(can, b).clicked() {
            start(app);
        }
        if app.ai.busy {
            ui.spinner();
            stop = ui.button("Cancel").clicked();
        }
        ui.label(egui::RichText::new(&app.ai.status).color(crate::theme::TEXT_DIM()));
    });
    if stop {
        cancel(app);
    }
    if let Some(n) = &app.ai.note {
        ui.label(egui::RichText::new(n).small().color(crate::theme::TEXT_DIM()));
    }
    if let Some(e) = &app.ai.error {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(e).color(egui::Color32::from_rgb(0xe8, 0x6a, 0x6a)));
        if app.ai.error_no_key {
            ui.add_space(4.0);
            if ui.button("Open AI Settings...").on_hover_text(KEY_PLACE).clicked() {
                app.open_options(crate::commands::OptionsPage::Global, Some("Privacy"));
            }
        }
    }
    if !app.ai.history.is_empty() {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Recent prompts").color(crate::theme::TEXT_DIM()));
        let hist = app.ai.history.clone();
        for h in hist {
            if ui.link(h.chars().take(60).collect::<String>()).clicked() {
                app.ai.prompt = h;
            }
        }
    }
    ui.add_space(6.0);
    let by = prov::powered_by(&app.settings.ai_config, &app.settings.ai_provider_url);
    ui.label(egui::RichText::new(format!("Results are editable vector objects. {by}")).small().color(crate::theme::TEXT_DIM()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_with_and_without_image() {
        let (a, b) = user_texts(0, " a fox ", "Line art", 4, None);
        assert_eq!(a, b);
        assert!(a.starts_with("Create: a fox\nStyle: Line art\nUse a palette of about 4"));
        let (with, plain) = user_texts(1, "a fox", "Cartoon", 6, Some("<svg id='ref'/>"));
        assert!(with.starts_with("Use the attached artwork"));
        assert!(!with.contains("<svg"), "the image carries the reference");
        assert!(with.contains("rich and detailed"));
        assert!(plain.contains("SVG source is below") && plain.ends_with("<svg id='ref'/>"));
        let (with, plain) = user_texts(2, "make it blue", "Cartoon", 6, Some("<svg/>"));
        assert!(with.starts_with("Remix the attached artwork") && with.ends_with("<svg/>"));
        assert!(plain.starts_with("Remix the artwork whose SVG source is below"));
    }

    #[test]
    fn environment_d_files_read_as_systemd_reads_them() {
        use environment_d::{expand, parse, quote};
        let text = "# comment\n; another\nA=1\n  B = two words  \nC=\"quoted \\\"x\\\" \\\\ \\$ \\n\"\nD='single $A \\n'\nE=con\\\ntinued\nF=\nnot an assignment\nG=\"a\" \"b\"\nH=x#y\n# a comment ending in \\\nJ=after\r\nI=\"no end";
        let pairs: Vec<(&str, &str)> = vec![("A", "1"), ("B", "two words"), ("C", "quoted \"x\" \\ $ \\n"), ("D", "single $A \\n"), ("E", "continued"), ("F", ""), ("G", "ab"), ("H", "x#y"), ("J", "after"), ("I", "no end")];
        assert_eq!(parse(text), pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>());
        let vars = vec![("A".to_string(), "1".to_string()), ("EMPTY".to_string(), String::new())];
        assert_eq!(expand("$A-${A}-$$A-${EMPTY:-def}-${A:-def}-${A:+alt}-${EMPTY:+alt}-${NO_SUCH_VARIABLE_ANYVECTOR}-$-${A", &vars), "1-1-$A-def-1-alt---$-${A");
        // keys are written plain when they can be, and always read back as they were
        assert_eq!(quote("sk-ant-api03_AbC.12/+=="), "sk-ant-api03_AbC.12/+==");
        assert_eq!(quote("a b"), "\"a b\"");
        assert_eq!(quote("p$ss\"w\\rd`"), "\"p$$ss\\\"w\\\\rd\\`\"");
        for v in ["sk-ant-api03_AbC.12/+==", "a b", "p$ss\"w\\rd`", "x'y", "ção", "#;=", "$HOME", ""] {
            let parsed = parse(&format!("K={}\n", quote(v)));
            assert_eq!(expand(&parsed[0].1, &[]), v, "{v}");
        }
    }

    #[test]
    fn environment_d_keys_are_saved_and_found_again() {
        use environment_d::{lookup, store, FILE};
        let dir = std::env::temp_dir().join(format!("anyvector-environment-d-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // a new file, readable by the user alone
        store(&dir, "ANYVECTOR_TEST_KEY", Some("sk-test $1 \"q\"")).unwrap();
        assert_eq!(lookup(&dir, "ANYVECTOR_TEST_KEY").as_deref(), Some("sk-test $1 \"q\""));
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(text.starts_with("# AnyVector") && text.ends_with("\nANYVECTOR_TEST_KEY=\"sk-test $$1 \\\"q\\\"\"\n"), "{text}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dir.join(FILE)).unwrap().permissions().mode() & 0o777, 0o600);
        }
        // the user's own lines stay; the key is replaced, not repeated
        std::fs::write(dir.join(FILE), format!("{text}# mine\nOTHER_ANYVECTOR=1\n")).unwrap();
        store(&dir, "ANYVECTOR_TEST_KEY", Some("new")).unwrap();
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert_eq!(text.matches("ANYVECTOR_TEST_KEY=").count(), 1, "{text}");
        assert!(text.contains("# mine\nOTHER_ANYVECTOR=1\n") && text.ends_with("ANYVECTOR_TEST_KEY=new\n"), "{text}");
        assert_eq!(lookup(&dir, "ANYVECTOR_TEST_KEY").as_deref(), Some("new"));
        // the files in the order of their names, the last assignment winning, earlier variables
        // expanding; systemd skips assignments without a value and names it doesn't take
        std::fs::write(dir.join("zz-later.conf"), "ANYVECTOR_TEST_KEY=${OTHER_ANYVECTOR}-later\nANYVECTOR_TEST_KEY=\nANYVECTOR_TEST_KEY=''\n").unwrap();
        std::fs::write(dir.join("00-first.conf"), "OTHER_ANYVECTOR=0\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "ANYVECTOR_TEST_KEY=ignored\n").unwrap();
        assert_eq!(lookup(&dir, "ANYVECTOR_TEST_KEY").as_deref(), Some("1-later"));
        std::fs::remove_file(dir.join("zz-later.conf")).unwrap();
        // Remove Key: the line goes, and the file with its last assignment
        store(&dir, "ANYVECTOR_TEST_KEY", None).unwrap();
        assert_eq!(lookup(&dir, "ANYVECTOR_TEST_KEY"), None);
        assert!(std::fs::read_to_string(dir.join(FILE)).unwrap().contains("OTHER_ANYVECTOR=1"));
        store(&dir, "OTHER_ANYVECTOR", None).unwrap();
        assert!(!dir.join(FILE).exists());
        store(&dir, "OTHER_ANYVECTOR", None).unwrap();
        assert_eq!(lookup(&dir.join("missing"), "ANYVECTOR_TEST_KEY"), None);
        let _ = std::fs::remove_dir_all(&dir);
        if cfg!(unix) {
            assert!(environment_d::shown_path().ends_with("environment.d/anyvector.conf"), "{}", environment_d::shown_path());
        }
    }

    #[test]
    fn key_names_and_keys_that_cannot_be_saved() {
        assert!(valid_key("ANTHROPIC_API_KEY", "sk-ant-api03-x"));
        // the environment cannot hold these (std::env::set_var would stop the program)
        assert!(!valid_key("A=B", "x"));
        assert!(!valid_key("KEY", "a\0b"));
        assert!(save_user_key("A=B", "x").is_err());
        // systemd's rules on Linux; Windows takes any other name
        assert_eq!(valid_key("MY-KEY", "x"), cfg!(windows));
        assert_eq!(valid_key("1KEY", "x"), cfg!(windows));
        assert_eq!(valid_key("KEY", "two\nlines"), cfg!(windows));
        assert!(environment_d::valid_name("_A1") && !environment_d::valid_name("") && !environment_d::valid_name("A B"));
        // where the messages say the key goes
        assert!(key_saved_message("K").contains(if cfg!(windows) { "Windows account" } else { "user account" }));
        assert!(save_key_note().contains(if cfg!(windows) { "Windows account" } else { "environment.d" }));
    }

    #[test]
    fn svg_from_reasoning_reply() {
        let reply = "<think>I could use <svg> with circles</think>Here you go:\n```svg\n<svg viewBox=\"0 0 1 1\"><rect/></svg>\n```";
        assert_eq!(extract_svg(&prov::strip_think(reply)).unwrap(), "<svg viewBox=\"0 0 1 1\"><rect/></svg>");
    }

    #[test]
    fn bitmap_placement() {
        let page = kurbo::Rect::new(0.0, 0.0, 100.0, 200.0);
        // a square image in a tall page: as wide as 80% of the page, centred
        let r = bitmap_rect(page, None, 512, 512);
        assert!((r.width() - 80.0).abs() < 1e-9 && (r.height() - 80.0).abs() < 1e-9);
        assert_eq!(r.center(), page.center());
        // beside the reference, at its height
        let r = bitmap_rect(page, Some(kurbo::Rect::new(10.0, 10.0, 30.0, 50.0)), 1024, 512);
        assert_eq!(r, kurbo::Rect::new(48.0, 10.0, 128.0, 50.0));
    }
}
