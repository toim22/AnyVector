//! The facts shown by Help ▸ About AnyVector ▸ System Info..., one list per category.
//!
//! Windows asks its APIs. Linux reads `/etc/os-release` and `/proc` (kernel, processor, memory,
//! loaded libraries), the session's environment (desktop, Wayland/X11, locale), `xrandr` or the DRM
//! connectors in `/sys` for the monitors, `df` for the free space and CUPS' `lpstat` for the printers.

/// The categories of the "Choose a category" list, in the reference's order.
#[cfg(windows)]
pub const CATEGORIES: [&str; 5] = ["System", "Display", "Printing", "AnyVector EXEs & DLLs", "System DLLs"];
/// The categories of the "Choose a category" list, in the reference's order (Linux loads shared
/// libraries, not DLLs).
#[cfg(not(windows))]
pub const CATEGORIES: [&str; 5] = ["System", "Display", "Printing", "AnyVector Programs & Libraries", "System Libraries"];

pub type Rows = Vec<(String, String)>;

/// The line ending of the saved report: CR LF on Windows, LF elsewhere.
const NEWLINE: &str = if cfg!(windows) { "\r\n" } else { "\n" };

fn gb(bytes: u64) -> String {
    format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}

fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

/// The rows of category `index` (see [`CATEGORIES`]). `gpu` describes the graphics adapter the
/// program renders with.
pub fn collect(index: usize, gpu: &str) -> Rows {
    match index {
        0 => system(),
        1 => display(gpu),
        2 => printing(),
        3 => modules(true),
        _ => modules(false),
    }
}

/// Every category as plain text (for the Save button).
pub fn report(gpu: &str) -> String {
    let mut s = format!("AnyVector {} - System Info", vector_core::APP_VERSION);
    s.push_str(NEWLINE);
    for (i, name) in CATEGORIES.iter().enumerate() {
        s.push_str(&format!("{NEWLINE}[{name}]{NEWLINE}"));
        for (k, v) in collect(i, gpu) {
            s.push_str(&format!("{k:<34}{v}{NEWLINE}"));
        }
    }
    s
}

fn app_dir() -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default()
}

#[cfg(windows)]
fn with_backslash(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    if s.ends_with('\\') {
        s
    } else {
        format!("{s}\\")
    }
}

/// A folder with a slash at the end, as the reference shows folders ("/tmp/").
#[cfg(not(windows))]
fn with_slash(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    if s.ends_with('/') {
        s
    } else {
        format!("{s}/")
    }
}

#[cfg(windows)]
mod win {
    use windows::core::{HSTRING, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{HMODULE, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW, DEVMODEW, ENUM_CURRENT_SETTINGS, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW};
    use windows::Win32::Graphics::Printing::{EnumPrintersW, GetDefaultPrinterW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_2W};
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows::Win32::System::ProcessStatus::{EnumProcessModules, GetModuleFileNameExW, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
    use windows::Win32::System::Registry::{RegGetValueW, HKEY, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    use windows::Win32::System::SystemInformation::{GetNativeSystemInfo, GetWindowsDirectoryW, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO};
    use windows::Win32::System::Threading::{GetCurrentProcess, GetGuiResources, IsProcessorFeaturePresent, GR_GDIOBJECTS, GR_USEROBJECTS, PF_FLOATING_POINT_EMULATED};

    pub fn reg_str(key: HKEY, sub: &str, value: &str) -> Option<String> {
        let (sub, value) = (HSTRING::from(sub), HSTRING::from(value));
        let mut size = 0u32;
        unsafe { RegGetValueW(key, &sub, &value, RRF_RT_REG_SZ, None, None, Some(&mut size)) }.ok().ok()?;
        let mut buf = vec![0u16; size as usize / 2 + 1];
        let mut size = (buf.len() * 2) as u32;
        unsafe { RegGetValueW(key, &sub, &value, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut size)) }.ok().ok()?;
        let n = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..n]).trim().to_string())
    }

    fn wide(buf: &[u16]) -> String {
        let n = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..n])
    }

    pub fn platform() -> String {
        const NT: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        let build: u32 = reg_str(HKEY_LOCAL_MACHINE, NT, "CurrentBuildNumber").and_then(|b| b.parse().ok()).unwrap_or(0);
        let edition = reg_str(HKEY_LOCAL_MACHINE, NT, "EditionID").unwrap_or_default();
        // ProductName still says "Windows 10" on Windows 11; the build number tells them apart
        let name = if build >= 22000 { "Windows 11" } else { "Windows 10" };
        if edition.is_empty() {
            name.to_string()
        } else {
            format!("{name} {edition}")
        }
    }

    pub fn processor() -> String {
        reg_str(HKEY_LOCAL_MACHINE, r"HARDWARE\DESCRIPTION\System\CentralProcessor\0", "ProcessorNameString").unwrap_or_default()
    }

    pub fn processors() -> u32 {
        let mut si = SYSTEM_INFO::default();
        unsafe { GetNativeSystemInfo(&mut si) };
        si.dwNumberOfProcessors
    }

    pub fn fpu() -> bool {
        !unsafe { IsProcessorFeaturePresent(PF_FLOATING_POINT_EMULATED) }.as_bool()
    }

    /// (total physical, available physical, total virtual, available virtual)
    pub fn memory() -> (u64, u64, u64, u64) {
        let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        let _ = unsafe { GlobalMemoryStatusEx(&mut m) };
        (m.ullTotalPhys, m.ullAvailPhys, m.ullTotalPageFile, m.ullAvailPageFile)
    }

    pub fn process_memory() -> u64 {
        let mut c = PROCESS_MEMORY_COUNTERS_EX { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32, ..Default::default() };
        let _ = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), (&mut c as *mut PROCESS_MEMORY_COUNTERS_EX).cast::<PROCESS_MEMORY_COUNTERS>(), c.cb) };
        c.PrivateUsage as u64
    }

    pub fn gui_objects() -> (u32, u32) {
        unsafe { (GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS), GetGuiResources(GetCurrentProcess(), GR_USEROBJECTS)) }
    }

    pub fn free_space(root: &str) -> u64 {
        let mut free = 0u64;
        let _ = unsafe { GetDiskFreeSpaceExW(&HSTRING::from(root), Some(&mut free), None, None) };
        free
    }

    pub fn windows_dir() -> String {
        let mut buf = [0u16; 260];
        let n = unsafe { GetWindowsDirectoryW(Some(&mut buf)) } as usize;
        String::from_utf16_lossy(&buf[..n.min(buf.len())])
    }

    pub struct Monitor {
        pub device: String,
        pub primary: bool,
        pub width: u32,
        pub height: u32,
        pub bits: u32,
        pub hz: u32,
        pub rect: RECT,
    }

    unsafe extern "system" fn monitor_cb(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> windows::core::BOOL {
        let list = &mut *(data.0 as *mut Vec<Monitor>);
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(h, (&mut mi as *mut MONITORINFOEXW).cast::<MONITORINFO>()).as_bool() {
            let mut dm = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
            let _ = EnumDisplaySettingsW(PCWSTR(mi.szDevice.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm);
            list.push(Monitor {
                device: wide(&mi.szDevice),
                primary: mi.monitorInfo.dwFlags & 1 != 0,
                width: dm.dmPelsWidth,
                height: dm.dmPelsHeight,
                bits: dm.dmBitsPerPel,
                hz: dm.dmDisplayFrequency,
                rect: mi.monitorInfo.rcMonitor,
            });
        }
        true.into()
    }

    pub fn monitors() -> Vec<Monitor> {
        let mut list: Vec<Monitor> = vec![];
        unsafe {
            let _ = EnumDisplayMonitors(None, None, Some(monitor_cb), LPARAM(&mut list as *mut Vec<Monitor> as isize));
        }
        list
    }

    pub fn default_printer() -> Option<String> {
        let mut len = 0u32;
        unsafe {
            let _ = GetDefaultPrinterW(None, &mut len);
            if len == 0 {
                return None;
            }
            let mut buf = vec![0u16; len as usize];
            GetDefaultPrinterW(Some(PWSTR(buf.as_mut_ptr())), &mut len).as_bool().then(|| wide(&buf))
        }
    }

    /// (name, driver, port) of every installed printer.
    pub fn printers() -> Vec<(String, String, String)> {
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let (mut needed, mut count) = (0u32, 0u32);
        unsafe {
            let _ = EnumPrintersW(flags, PCWSTR::null(), 2, None, &mut needed, &mut count);
            if needed == 0 {
                return vec![];
            }
            // u64 storage keeps the PRINTER_INFO_2W records aligned
            let mut store = vec![0u64; (needed as usize).div_ceil(8)];
            let bytes = std::slice::from_raw_parts_mut(store.as_mut_ptr().cast::<u8>(), needed as usize);
            if EnumPrintersW(flags, PCWSTR::null(), 2, Some(bytes), &mut needed, &mut count).is_err() {
                return vec![];
            }
            let infos = std::slice::from_raw_parts(store.as_ptr().cast::<PRINTER_INFO_2W>(), count as usize);
            infos
                .iter()
                .map(|p| {
                    let s = |w: PWSTR| if w.is_null() { String::new() } else { w.to_string().unwrap_or_default() };
                    (s(p.pPrinterName), s(p.pDriverName), s(p.pPortName))
                })
                .collect()
        }
    }

    pub fn modules() -> Vec<String> {
        let mut mods = vec![HMODULE::default(); 2048];
        let mut needed = 0u32;
        unsafe {
            if EnumProcessModules(GetCurrentProcess(), mods.as_mut_ptr(), (mods.len() * std::mem::size_of::<HMODULE>()) as u32, &mut needed).is_err() {
                return vec![];
            }
            let n = (needed as usize / std::mem::size_of::<HMODULE>()).min(mods.len());
            mods[..n]
                .iter()
                .filter_map(|m| {
                    let mut buf = [0u16; 1024];
                    let len = GetModuleFileNameExW(Some(GetCurrentProcess()), Some(*m), &mut buf) as usize;
                    (len > 0).then(|| String::from_utf16_lossy(&buf[..len]))
                })
                .collect()
        }
    }
}

#[cfg(windows)]
fn system() -> Rows {
    let (tp, ap, tv, av) = win::memory();
    let (gdi, user) = win::gui_objects();
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    vec![
        ("Platform:".into(), win::platform()),
        ("Processor:".into(), win::processor()),
        ("Number of Processors:".into(), win::processors().to_string()),
        ("Math Coprocessor:".into(), if win::fpu() { "Yes" } else { "No" }.into()),
        ("Total physical memory:".into(), gb(tp)),
        ("Available physical memory:".into(), gb(ap)),
        ("Total virtual memory:".into(), gb(tv)),
        ("Available virtual memory:".into(), gb(av)),
        ("Total memory used by app:".into(), gb(win::process_memory())),
        ("GDI Object Count:".into(), gdi.to_string()),
        ("User Object Count:".into(), user.to_string()),
        (format!("Available space on drive {}", drive.trim_end_matches('\\')), gb(win::free_space(&format!("{drive}\\")))),
        ("Windows Directory:".into(), win::windows_dir()),
        ("TEMP directory:".into(), with_backslash(&std::env::temp_dir())),
        ("Application directory:".into(), app_dir().display().to_string()),
        ("Box name:".into(), format!("Vector_{}", env!("CARGO_PKG_VERSION_MAJOR"))),
    ]
}

#[cfg(windows)]
fn display(gpu: &str) -> Rows {
    let mut rows: Rows = vec![("Graphics adapter:".into(), if gpu.is_empty() { "Unknown".into() } else { gpu.to_string() })];
    let mons = win::monitors();
    rows.push(("Number of displays:".into(), mons.len().to_string()));
    for (i, m) in mons.iter().enumerate() {
        rows.push((format!("Display {}:", i + 1), format!("{}{}", m.device, if m.primary { " (primary)" } else { "" })));
        rows.push(("    Resolution:".into(), format!("{} x {} pixels", m.width, m.height)));
        rows.push(("    Color depth:".into(), format!("{} bits", m.bits)));
        rows.push(("    Refresh rate:".into(), format!("{} Hz", m.hz)));
        rows.push(("    Position:".into(), format!("{}, {}", m.rect.left, m.rect.top)));
    }
    rows
}

#[cfg(windows)]
fn printing() -> Rows {
    let mut rows: Rows = vec![("Default printer:".into(), win::default_printer().unwrap_or_else(|| "None".into()))];
    let list = win::printers();
    rows.push(("Installed printers:".into(), list.len().to_string()));
    for (name, driver, port) in list {
        rows.push((format!("{name}:"), format!("{driver} ({port})")));
    }
    rows
}

#[cfg(windows)]
fn modules(own: bool) -> Rows {
    let dir = app_dir().display().to_string().to_lowercase();
    let mut rows: Rows = vec![];
    for path in win::modules() {
        let mine = path.to_lowercase().starts_with(&dir);
        if mine != own {
            continue;
        }
        let p = std::path::Path::new(&path);
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let value = if own {
            let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            let ver = if name.eq_ignore_ascii_case("anyvector.exe") || p.extension().is_some_and(|e| e == "exe") { format!("{}, ", vector_core::APP_VERSION) } else { String::new() };
            format!("{ver}{}, {path}", mb(size))
        } else {
            path.clone()
        };
        rows.push((format!("{name}:"), value));
    }
    rows.sort_by_key(|(k, _)| k.to_lowercase());
    rows
}

/// Linux: what `/etc/os-release`, `/proc`, the session's environment, `xrandr`, `df` and `lpstat`
/// say, read from their text (so the tests run on every system).
#[cfg(any(not(windows), test))]
#[cfg_attr(windows, allow(dead_code))]
mod linux {
    use std::collections::{HashMap, HashSet};

    /// The distribution and its version from os-release(5): PRETTY_NAME, else NAME and VERSION
    /// ("Ubuntu 24.04.5 LTS").
    pub fn os_release_name(text: &str) -> Option<String> {
        let mut fields = HashMap::new();
        for line in text.lines().map(str::trim).filter(|l| !l.starts_with('#')) {
            if let Some((k, v)) = line.split_once('=') {
                fields.insert(k.trim(), unquote(v.trim()));
            }
        }
        let get = |k: &str| fields.get(k).filter(|v| !v.is_empty()).cloned();
        get("PRETTY_NAME").or_else(|| {
            let name = get("NAME")?;
            Some(match get("VERSION").or_else(|| get("VERSION_ID")) {
                Some(v) => format!("{name} {v}"),
                None => name,
            })
        })
    }

    /// An os-release value, which follows the shell's quoting: "a b" (with \ escapes), 'a b' or a.
    fn unquote(v: &str) -> String {
        if let Some(inner) = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            let mut out = String::with_capacity(inner.len());
            let mut chars = inner.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => out.extend(chars.next()),
                    c => out.push(c),
                }
            }
            out
        } else if let Some(inner) = v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
            inner.to_string()
        } else {
            v.to_string()
        }
    }

    /// The version of WSL (Windows Subsystem for Linux) the system runs in: WSL 1's kernel release
    /// reads "4.4.0-19041-Microsoft", WSL 2's "…-microsoft-standard-WSL2"; a WSL 2 with its own
    /// kernel still has Windows' interop registered (`/proc/sys/fs/binfmt_misc/WSLInterop`).
    pub fn wsl_version(kernel_release: &str, interop: bool) -> Option<u8> {
        let k = kernel_release.to_ascii_lowercase();
        if k.contains("microsoft") {
            Some(if k.contains("wsl2") || k.contains("microsoft-standard") { 2 } else { 1 })
        } else {
            interop.then_some(2)
        }
    }

    /// The desktop: XDG_CURRENT_DESKTOP ("ubuntu:GNOME", "KDE"), else XDG_SESSION_DESKTOP or
    /// DESKTOP_SESSION; WSLg (the Wayland and X server WSL brings) when none is set but WSLg runs.
    pub fn desktop(current: Option<&str>, session_desktop: Option<&str>, desktop_session: Option<&str>, wslg: bool) -> Option<String> {
        [current, session_desktop, desktop_session]
            .into_iter()
            .flatten()
            .map(|s| s.trim().rsplit('/').next().unwrap_or(""))
            .find(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| wslg.then(|| "WSLg".to_string()))
    }

    /// The session type and the display servers it can reach: "Wayland (WAYLAND_DISPLAY=wayland-0,
    /// DISPLAY=:0)". XDG_SESSION_TYPE names the session; without it a Wayland display makes it Wayland,
    /// an X display X11.
    pub fn session(session_type: Option<&str>, wayland: Option<&str>, x: Option<&str>) -> String {
        fn set(s: Option<&str>) -> Option<&str> {
            s.map(str::trim).filter(|s| !s.is_empty())
        }
        let (wayland, x) = (set(wayland), set(x));
        let kind = match set(session_type).map(str::to_ascii_lowercase).as_deref() {
            Some("wayland") => "Wayland".to_string(),
            Some("x11") => "X11".to_string(),
            Some("tty") => "TTY".to_string(),
            Some("mir") => "Mir".to_string(),
            Some(other) if other != "unspecified" => other.to_string(),
            _ if wayland.is_some() => "Wayland".to_string(),
            _ if x.is_some() => "X11".to_string(),
            _ => "Unknown".to_string(),
        };
        let displays: Vec<String> = [("WAYLAND_DISPLAY", wayland), ("DISPLAY", x)].iter().filter_map(|(k, v)| v.map(|v| format!("{k}={v}"))).collect();
        if displays.is_empty() {
            kind
        } else {
            format!("{kind} ({})", displays.join(", "))
        }
    }

    /// The locale the messages follow (LC_ALL, else LC_MESSAGES, else LANG; C when none is set), with
    /// the variables that change the language, the units or the paper: "pt_BR.UTF-8 (LANGUAGE=pt_BR:pt:en)".
    pub fn locale(var: impl Fn(&str) -> Option<String>) -> String {
        let set = |name: &str| var(name).filter(|v| !v.trim().is_empty());
        let all = set("LC_ALL");
        let messages = all.clone().or_else(|| set("LC_MESSAGES")).or_else(|| set("LANG")).unwrap_or_else(|| "C".into());
        // LC_ALL overrides the categories
        let names: &[&str] = if all.is_some() { &["LANGUAGE"] } else { &["LANGUAGE", "LC_MEASUREMENT", "LC_PAPER"] };
        let extra: Vec<String> = names.iter().filter_map(|n| set(n).map(|v| format!("{n}={v}"))).collect();
        if extra.is_empty() {
            messages
        } else {
            format!("{messages} ({})", extra.join(", "))
        }
    }

    /// From /proc/cpuinfo: the processor's name ("model name" on x86; "Model name", "cpu model",
    /// "Hardware" or "Processor" elsewhere), the number of logical processors and whether it has a
    /// floating-point unit.
    pub fn cpu(text: &str) -> (Option<String>, usize, bool) {
        let (mut name, mut count, mut fpu) = (None, 0, true);
        for line in text.lines() {
            let Some((k, v)) = line.split_once(':') else { continue };
            match (k.trim(), v.trim()) {
                ("processor", _) => count += 1,
                ("model name" | "Model name" | "cpu model" | "Hardware" | "Processor", v) if name.is_none() && !v.is_empty() => name = Some(v.to_string()),
                ("fpu", "no") => fpu = false,
                _ => {}
            }
        }
        (name, count, fpu)
    }

    /// A "Name:   1234 kB" value of /proc/meminfo or /proc/self/status, in bytes.
    fn kb(text: &str, key: &str) -> Option<u64> {
        text.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim() == key).then(|| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())?
        }).map(|v| v * 1024)
    }

    /// /proc/meminfo, in bytes: total and available RAM (before Linux 3.14 had MemAvailable: free,
    /// buffers and cache), total and free swap.
    pub fn meminfo(text: &str) -> (u64, u64, u64, u64) {
        let total = kb(text, "MemTotal").unwrap_or(0);
        let available = kb(text, "MemAvailable").unwrap_or_else(|| ["MemFree", "Buffers", "Cached"].iter().filter_map(|k| kb(text, k)).sum());
        (total, available, kb(text, "SwapTotal").unwrap_or(0), kb(text, "SwapFree").unwrap_or(0))
    }

    /// The memory that is the program's alone (as Windows' private bytes): the private pages of
    /// /proc/self/smaps_rollup and those swapped out, in bytes.
    pub fn private_memory(smaps_rollup: &str) -> Option<u64> {
        let parts: Vec<u64> = ["Private_Clean", "Private_Dirty", "Swap"].iter().filter_map(|k| kb(smaps_rollup, k)).collect();
        (!parts.is_empty()).then(|| parts.iter().sum())
    }

    /// The resident memory of /proc/self/status (VmRSS), in bytes, for kernels without smaps_rollup.
    pub fn resident_memory(status: &str) -> Option<u64> {
        kb(status, "VmRSS")
    }

    /// The space available to the user, in bytes, from `df -Pk` (a header, then "filesystem 1024-blocks
    /// used available capacity mount-point"; names with spaces are found around the capacity, "42%").
    pub fn df_available(text: &str) -> Option<u64> {
        let fields: Vec<&str> = text.lines().nth(1)?.split_whitespace().collect();
        let cap = fields.iter().position(|f| f.len() > 1 && f.ends_with('%') && f[..f.len() - 1].bytes().all(|b| b.is_ascii_digit()))?;
        let available: u64 = fields.get(cap.checked_sub(1)?)?.parse().ok()?;
        Some(available * 1024)
    }

    /// A monitor: its output, whether it is the primary one, its size in pixels, refresh rate and
    /// position on the desktop (what is known).
    #[derive(Debug, PartialEq)]
    pub struct Monitor {
        pub name: String,
        pub primary: bool,
        pub size: Option<(u32, u32)>,
        pub hz: Option<f32>,
        pub pos: Option<(i32, i32)>,
    }

    /// "1920x1080" → (1920, 1080).
    pub fn parse_mode(s: &str) -> Option<(u32, u32)> {
        let (w, h) = s.trim().split_once('x')?;
        Some((w.parse().ok()?, h.trim_end_matches(|c: char| !c.is_ascii_digit()).parse().ok()?))
    }

    /// "2560x1440+1920+0" → (2560, 1440, 1920, 0).
    fn parse_geometry(s: &str) -> Option<(u32, u32, i32, i32)> {
        let (size, pos) = s.split_once('+')?;
        let (w, h) = parse_mode(size)?;
        let (x, y) = pos.split_once('+')?;
        Some((w, h, x.parse().ok()?, y.parse().ok()?))
    }

    /// The monitors that are on in `xrandr --query`: "HDMI-1 connected primary 2560x1440+0+0 (normal
    /// left inverted right x axis y axis) 597mm x 336mm", then its modes, the current one marked `*`
    /// ("   2560x1440     59.95*+  60.00").
    pub fn xrandr_monitors(text: &str) -> Vec<Monitor> {
        let mut out: Vec<Monitor> = vec![];
        let mut modes_of_last = false;
        for line in text.lines() {
            if !line.starts_with(char::is_whitespace) {
                modes_of_last = false;
                let mut words = line.split_whitespace();
                let (Some(name), Some("connected")) = (words.next(), words.next()) else { continue };
                let rest: Vec<&str> = words.collect();
                // a connected output that is switched off has no geometry
                let Some((w, h, x, y)) = rest.iter().find_map(|w| parse_geometry(w)) else { continue };
                out.push(Monitor { name: name.to_string(), primary: rest.first() == Some(&"primary"), size: Some((w, h)), hz: None, pos: Some((x, y)) });
                modes_of_last = true;
            } else if modes_of_last {
                if let Some(m) = out.last_mut().filter(|m| m.hz.is_none()) {
                    m.hz = line.split_whitespace().skip(1).find(|w| w.contains('*')).and_then(|w| w.trim_end_matches(['*', '+']).parse().ok());
                }
            }
        }
        out
    }

    /// The default printer of `lpstat -d` ("system default destination: Office").
    pub fn lpstat_default(text: &str) -> Option<String> {
        text.lines().find_map(|l| l.strip_prefix("system default destination:")).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }

    /// The printers of `lpstat -v` and where they print ("device for Office: ipp://printer.local/ipp/print").
    pub fn lpstat_devices(text: &str) -> Vec<(String, String)> {
        text.lines()
            .filter_map(|l| {
                let (name, uri) = l.strip_prefix("device for ")?.split_once(':')?;
                Some((name.trim().to_string(), uri.trim().to_string()))
            })
            .collect()
    }

    /// A shared library's file name: "libc.so.6", "libvulkan.so", "ld-linux-x86-64.so.2" (not
    /// "ld.so.cache").
    fn is_shared_object(file: &str) -> bool {
        file.ends_with(".so") || file.split_once(".so.").is_some_and(|(_, v)| v.starts_with(|c: char| c.is_ascii_digit()))
    }

    /// The program `exe` and the shared libraries mapped into the process (`/proc/self/maps`: address,
    /// permissions, offset, device, inode and path), each once, in the order of the map.
    pub fn mapped_modules(maps: &str, exe: Option<&str>) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut out = vec![];
        for line in maps.lines() {
            // the path is what follows the fifth field; it may hold spaces
            let mut rest = line;
            for _ in 0..5 {
                rest = rest.trim_start();
                rest = &rest[rest.find(char::is_whitespace).unwrap_or(rest.len())..];
            }
            let path = rest.trim();
            let path = path.strip_suffix(" (deleted)").unwrap_or(path);
            if !path.starts_with('/') || path.starts_with("/dev/") || path.starts_with("/memfd:") || path.starts_with("/SYSV") {
                continue;
            }
            let file = path.rsplit('/').next().unwrap_or("");
            if (is_shared_object(file) || Some(path) == exe) && seen.insert(path) {
                out.push(path.to_string());
            }
        }
        out
    }
}

/// Linux: reading the facts (files, environment and the programs that know them).
#[cfg(not(windows))]
mod unix {
    use super::linux;
    use std::path::Path;
    use std::time::{Duration, Instant};

    pub fn read(path: &str) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    pub fn env(name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.trim().is_empty())
    }

    /// What a program prints, run with the C locale (so it prints English); `None` when it isn't
    /// installed or doesn't finish within two seconds (a printer server that doesn't answer).
    pub fn run(program: &str, args: &[&str]) -> Option<String> {
        use std::process::{Command, Stdio};
        let mut child = Command::new(program).args(args).env("LC_ALL", "C").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
        let mut out = child.stdout.take()?;
        let reader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = std::io::Read::read_to_string(&mut out, &mut s);
            s
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return reader.join().ok(),
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
            }
        }
    }

    fn kernel_release() -> String {
        read("/proc/sys/kernel/osrelease").trim().to_string()
    }

    pub fn wsl() -> Option<u8> {
        let interop = ["WSLInterop", "WSLInterop-late"].iter().any(|f| Path::new("/proc/sys/fs/binfmt_misc").join(f).exists());
        linux::wsl_version(&kernel_release(), interop)
    }

    /// "Ubuntu 24.04.5 LTS", with "(WSL 2)" inside WSL.
    pub fn platform() -> String {
        let name = linux::os_release_name(&read("/etc/os-release")).or_else(|| linux::os_release_name(&read("/usr/lib/os-release"))).unwrap_or_else(|| "Linux".into());
        match wsl() {
            Some(v) => format!("{name} (WSL {v})"),
            None => name,
        }
    }

    /// "Linux 6.8.0-49-generic (x86_64)".
    pub fn kernel() -> String {
        match kernel_release() {
            r if r.is_empty() => format!("Linux ({})", std::env::consts::ARCH),
            r => format!("Linux {r} ({})", std::env::consts::ARCH),
        }
    }

    pub fn desktop() -> Option<String> {
        let wslg = Path::new("/mnt/wslg").is_dir() && (env("WAYLAND_DISPLAY").is_some() || env("DISPLAY").is_some());
        linux::desktop(env("XDG_CURRENT_DESKTOP").as_deref(), env("XDG_SESSION_DESKTOP").as_deref(), env("DESKTOP_SESSION").as_deref(), wslg)
    }

    pub fn session() -> String {
        linux::session(env("XDG_SESSION_TYPE").as_deref(), env("WAYLAND_DISPLAY").as_deref(), env("DISPLAY").as_deref())
    }

    pub fn locale() -> String {
        linux::locale(env)
    }

    pub fn process_memory() -> Option<u64> {
        linux::private_memory(&read("/proc/self/smaps_rollup")).or_else(|| linux::resident_memory(&read("/proc/self/status")))
    }

    pub fn free_space(dir: &Path) -> Option<u64> {
        linux::df_available(&run("df", &["-Pk", "--", &dir.to_string_lossy()])?)
    }

    /// The monitors: `xrandr` on an X display (also Xwayland), else the connected DRM connectors
    /// (`/sys/class/drm/card0-HDMI-A-1`, names only: the current mode isn't there).
    pub fn monitors() -> Vec<linux::Monitor> {
        if env("DISPLAY").is_some() {
            let list = run("xrandr", &["--query"]).map(|t| linux::xrandr_monitors(&t)).unwrap_or_default();
            if !list.is_empty() {
                return list;
            }
        }
        let mut list: Vec<linux::Monitor> = std::fs::read_dir("/sys/class/drm")
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        let connector = name.strip_prefix("card")?.split_once('-')?.1.to_string();
                        let connected = std::fs::read_to_string(e.path().join("status")).is_ok_and(|s| s.trim() == "connected");
                        connected.then_some(linux::Monitor { name: connector, primary: false, size: None, hz: None, pos: None })
                    })
                    .collect()
            })
            .unwrap_or_default();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    /// The default printer and every printer with where it prints, from CUPS (`lpstat`).
    pub fn printers() -> (Option<String>, Vec<(String, String)>) {
        let default = run("lpstat", &["-d"]).and_then(|t| linux::lpstat_default(&t));
        let list = run("lpstat", &["-v"]).map(|t| linux::lpstat_devices(&t)).unwrap_or_default();
        (default, list)
    }
}

/// The system in one line, for bug reports: "Ubuntu 24.04.5 LTS (WSL 2); Linux 6.18.58.1-microsoft-
/// standard-WSL2 (x86_64); desktop WSLg; Wayland (WAYLAND_DISPLAY=wayland-0, DISPLAY=:0)".
#[cfg(not(windows))]
pub fn platform_summary() -> String {
    format!("{}; {}; desktop {}; {}", unix::platform(), unix::kernel(), unix::desktop().unwrap_or_else(|| "unknown".into()), unix::session())
}

#[cfg(not(windows))]
fn system() -> Rows {
    let (cpu, count, fpu) = linux::cpu(&unix::read("/proc/cpuinfo"));
    let count = if count > 0 { count } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) };
    let mut rows: Rows = vec![
        ("Platform:".into(), unix::platform()),
        ("Kernel:".into(), unix::kernel()),
        ("Desktop:".into(), unix::desktop().unwrap_or_else(|| "Unknown".into())),
        ("Session type:".into(), unix::session()),
        ("Locale:".into(), unix::locale()),
        ("Processor:".into(), cpu.unwrap_or_else(|| std::env::consts::ARCH.into())),
        ("Number of Processors:".into(), count.to_string()),
        ("Math Coprocessor:".into(), if fpu { "Yes" } else { "No" }.into()),
    ];
    let (tp, ap, swap, swap_free) = linux::meminfo(&unix::read("/proc/meminfo"));
    if tp > 0 {
        rows.push(("Total physical memory:".into(), gb(tp)));
        rows.push(("Available physical memory:".into(), gb(ap)));
        // memory and swap, as Windows counts memory and the page file
        rows.push(("Total virtual memory:".into(), gb(tp + swap)));
        rows.push(("Available virtual memory:".into(), gb(ap + swap_free)));
    }
    if let Some(m) = unix::process_memory() {
        rows.push(("Total memory used by app:".into(), gb(m)));
    }
    if let Some(home) = dirs::home_dir() {
        if let Some(free) = unix::free_space(&home) {
            rows.push((format!("Available space in {}:", home.display()), gb(free)));
        }
    }
    rows.push(("TEMP directory:".into(), with_slash(&std::env::temp_dir())));
    rows.push(("Application directory:".into(), app_dir().display().to_string()));
    rows.push(("Box name:".into(), format!("Vector_{}", env!("CARGO_PKG_VERSION_MAJOR"))));
    rows
}

#[cfg(not(windows))]
fn display(gpu: &str) -> Rows {
    let mut rows: Rows = vec![("Graphics adapter:".into(), if gpu.is_empty() { "Unknown".into() } else { gpu.to_string() })];
    let mons = unix::monitors();
    if !mons.is_empty() {
        rows.push(("Number of displays:".into(), mons.len().to_string()));
    }
    for (i, m) in mons.iter().enumerate() {
        rows.push((format!("Display {}:", i + 1), format!("{}{}", m.name, if m.primary { " (primary)" } else { "" })));
        if let Some((w, h)) = m.size {
            rows.push(("    Resolution:".into(), format!("{w} x {h} pixels")));
        }
        if let Some(hz) = m.hz {
            rows.push(("    Refresh rate:".into(), format!("{hz:.0} Hz")));
        }
        if let Some((x, y)) = m.pos {
            rows.push(("    Position:".into(), format!("{x}, {y}")));
        }
    }
    rows
}

#[cfg(not(windows))]
fn printing() -> Rows {
    let (default, list) = unix::printers();
    let mut rows: Rows = vec![("Default printer:".into(), default.unwrap_or_else(|| "None".into()))];
    rows.push(("Installed printers:".into(), list.len().to_string()));
    for (name, uri) in list {
        rows.push((format!("{name}:"), uri));
    }
    rows
}

/// The program and its libraries (`own`), or the system's libraries, loaded in this process. An
/// AppImage's own libraries are those inside it (`$APPDIR`).
#[cfg(not(windows))]
fn modules(own: bool) -> Rows {
    use std::path::Path;
    let exe = std::env::current_exe().ok();
    let root = crate::updater::appimage_dir().unwrap_or_else(app_dir);
    let exe_str = exe.as_ref().map(|e| e.to_string_lossy().to_string());
    let mut rows: Rows = vec![];
    for path in linux::mapped_modules(&unix::read("/proc/self/maps"), exe_str.as_deref()) {
        let p = Path::new(&path);
        let is_exe = exe.as_deref() == Some(p);
        if (is_exe || p.starts_with(&root)) != own {
            continue;
        }
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let value = if own {
            let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            let ver = if is_exe { format!("{}, ", vector_core::APP_VERSION) } else { String::new() };
            format!("{ver}{}, {path}", mb(size))
        } else {
            path.clone()
        };
        rows.push((format!("{name}:"), value));
    }
    rows.sort_by_key(|(k, _)| k.to_lowercase());
    rows
}

#[cfg(test)]
mod tests {
    use super::linux;

    #[test]
    fn every_category_has_rows() {
        for i in 0..super::CATEGORIES.len() {
            let rows = super::collect(i, "Test adapter");
            // printers may be absent on a build machine; the others always have something
            if i != 2 {
                assert!(!rows.is_empty(), "category {} is empty", super::CATEGORIES[i]);
            }
        }
        let sys = super::collect(0, "");
        assert!(sys.iter().any(|(k, v)| k == "Platform:" && v.starts_with("Windows")) || !cfg!(windows));
        // Linux: the distribution, kernel, session and memory; this program among its own modules and
        // the C library among the system's
        if cfg!(target_os = "linux") {
            let row = |k: &str| sys.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone()).unwrap_or_default();
            assert!(!row("Platform:").is_empty() && row("Platform:") != "linux", "{sys:?}");
            assert!(row("Kernel:").starts_with("Linux "), "{sys:?}");
            assert!(!row("Session type:").is_empty() && !row("Locale:").is_empty(), "{sys:?}");
            assert!(row("Total physical memory:").ends_with(" GB"), "{sys:?}");
            assert!(row("TEMP directory:").ends_with('/'), "{sys:?}");
            let exe = std::env::current_exe().unwrap().to_string_lossy().to_string();
            let own = super::collect(3, "");
            assert!(own.iter().any(|(_, v)| v.ends_with(&exe) && v.starts_with(vector_core::APP_VERSION)), "{own:?}");
            let libs = super::collect(4, "");
            assert!(libs.iter().any(|(k, _)| k.starts_with("libc.") || k.starts_with("ld-")), "{libs:?}");
            assert!(libs.iter().all(|(_, v)| v.starts_with('/') && *v != exe), "{libs:?}");
            let printing = super::collect(2, "");
            assert_eq!(printing[0].0, "Default printer:");
            assert_eq!(printing[1].0, "Installed printers:");
        }
    }

    #[test]
    fn linux_distribution_kernel_and_wsl() {
        let ubuntu = "PRETTY_NAME=\"Ubuntu 24.04.5 LTS\"\nNAME=\"Ubuntu\"\nVERSION_ID=\"24.04\"\nVERSION=\"24.04.5 LTS (Noble Numbat)\"\nID=ubuntu\n";
        assert_eq!(linux::os_release_name(ubuntu).as_deref(), Some("Ubuntu 24.04.5 LTS"));
        assert_eq!(linux::os_release_name("# comment\nNAME='Fedora Linux'\nVERSION=\"41 (Workstation Edition)\"\n").as_deref(), Some("Fedora Linux 41 (Workstation Edition)"));
        assert_eq!(linux::os_release_name("NAME=Arch\nPRETTY_NAME=\"\"\n").as_deref(), Some("Arch"));
        assert_eq!(linux::os_release_name("PRETTY_NAME=\"Say \\\"hi\\\" \\$HOME\"").as_deref(), Some("Say \"hi\" $HOME"));
        assert_eq!(linux::os_release_name(""), None);
        assert_eq!(linux::wsl_version("6.18.58.1-microsoft-standard-WSL2", true), Some(2));
        assert_eq!(linux::wsl_version("5.15.167.4-microsoft-standard-WSL2", false), Some(2));
        assert_eq!(linux::wsl_version("4.4.0-19041-Microsoft", true), Some(1));
        assert_eq!(linux::wsl_version("6.8.0-49-generic", false), None);
        // WSL 2 with a kernel of its own
        assert_eq!(linux::wsl_version("6.12.1-custom", true), Some(2));
    }

    #[test]
    fn linux_desktop_session_and_locale() {
        assert_eq!(linux::desktop(Some("ubuntu:GNOME"), Some("ubuntu"), None, false).as_deref(), Some("ubuntu:GNOME"));
        assert_eq!(linux::desktop(None, Some("plasma"), Some("plasma"), false).as_deref(), Some("plasma"));
        assert_eq!(linux::desktop(Some(" "), None, Some("/usr/share/xsessions/xfce"), false).as_deref(), Some("xfce"));
        assert_eq!(linux::desktop(None, None, None, true).as_deref(), Some("WSLg"));
        assert_eq!(linux::desktop(None, None, None, false), None);
        assert_eq!(linux::session(Some("wayland"), Some("wayland-0"), Some(":0")), "Wayland (WAYLAND_DISPLAY=wayland-0, DISPLAY=:0)");
        assert_eq!(linux::session(Some("x11"), None, Some(":1")), "X11 (DISPLAY=:1)");
        assert_eq!(linux::session(None, Some("wayland-0"), Some(":0")), "Wayland (WAYLAND_DISPLAY=wayland-0, DISPLAY=:0)");
        assert_eq!(linux::session(Some("unspecified"), None, Some(":0")), "X11 (DISPLAY=:0)");
        assert_eq!(linux::session(Some("tty"), None, None), "TTY");
        assert_eq!(linux::session(None, Some(""), None), "Unknown");
        let locale = |pairs: &[(&str, &str)]| {
            let map: std::collections::HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            linux::locale(move |k| map.get(k).cloned())
        };
        assert_eq!(locale(&[("LANG", "pt_BR.UTF-8"), ("LANGUAGE", "pt_BR:pt:en")]), "pt_BR.UTF-8 (LANGUAGE=pt_BR:pt:en)");
        assert_eq!(locale(&[("LANG", "en_US.UTF-8"), ("LC_MESSAGES", "de_DE.UTF-8"), ("LC_PAPER", "en_GB.UTF-8")]), "de_DE.UTF-8 (LC_PAPER=en_GB.UTF-8)");
        assert_eq!(locale(&[("LC_ALL", "fr_FR.UTF-8"), ("LC_PAPER", "en_US.UTF-8")]), "fr_FR.UTF-8");
        assert_eq!(locale(&[]), "C");
    }

    #[test]
    fn linux_processor_memory_and_disk() {
        let cpuinfo = "processor\t: 0\nvendor_id\t: GenuineIntel\nmodel name\t: Intel(R) Core(TM) i7-12700H\nfpu\t\t: yes\n\nprocessor\t: 1\nmodel name\t: Intel(R) Core(TM) i7-12700H\nfpu\t\t: yes\n";
        assert_eq!(linux::cpu(cpuinfo), (Some("Intel(R) Core(TM) i7-12700H".to_string()), 2, true));
        assert_eq!(linux::cpu("processor\t: 0\nBogoMIPS\t: 48.00\nCPU implementer\t: 0x41\n\nprocessor\t: 1\n"), (None, 2, true));
        assert_eq!(linux::cpu("processor : 0\nfpu : no\n"), (None, 1, false));
        let meminfo = "MemTotal:       32617452 kB\nMemFree:         1000000 kB\nMemAvailable:   20000000 kB\nBuffers:          100000 kB\nCached:          5000000 kB\nSwapCached:            0 kB\nSwapTotal:       8388608 kB\nSwapFree:        8000000 kB\n";
        assert_eq!(linux::meminfo(meminfo), (32617452 * 1024, 20000000 * 1024, 8388608 * 1024, 8000000 * 1024));
        // before MemAvailable: free + buffers + cache (SwapCached is not Cached)
        assert_eq!(linux::meminfo("MemTotal: 4000 kB\nMemFree: 1000 kB\nBuffers: 100 kB\nCached: 500 kB\nSwapCached: 7 kB\n"), (4000 * 1024, 1600 * 1024, 0, 0));
        let rollup = "55d0a1a00000-7ffc6a5f5000 ---p 00000000 00:00 0                          [rollup]\nRss:              250000 kB\nPrivate_Clean:      1000 kB\nPrivate_Dirty:    150000 kB\nSwap:                500 kB\nSwapPss:             500 kB\n";
        assert_eq!(linux::private_memory(rollup), Some(151_500 * 1024));
        assert_eq!(linux::private_memory(""), None);
        assert_eq!(linux::resident_memory("Name:\tanyvector\nVmRSS:\t  123456 kB\n"), Some(123456 * 1024));
        let df = "Filesystem     1024-blocks      Used Available Capacity Mounted on\n/dev/sdc        1055762868 158000000 843999999      16% /\n";
        assert_eq!(linux::df_available(df), Some(843999999 * 1024));
        let spaced = "Filesystem 1024-blocks Used Available Capacity Mounted on\nmy disk 1000 400 600 40% /media/me/My Disk\n";
        assert_eq!(linux::df_available(spaced), Some(600 * 1024));
        assert_eq!(linux::df_available("df: /nowhere: No such file or directory\n"), None);
    }

    #[test]
    fn linux_monitors_and_printers() {
        let xrandr = "Screen 0: minimum 320 x 200, current 4480 x 1440, maximum 16384 x 16384
HDMI-1 connected primary 2560x1440+0+0 (normal left inverted right x axis y axis) 597mm x 336mm
   2560x1440     59.95*+
   1920x1080     60.00    50.00
eDP-1 connected 1920x1080+2560+180 (normal left inverted right x axis y axis) 344mm x 194mm
   1920x1080    144.00*+  60.01
DP-1 disconnected (normal left inverted right x axis y axis)
DP-2 connected (normal left inverted right x axis y axis)
   1280x1024     60.02 +
";
        let m = linux::xrandr_monitors(xrandr);
        assert_eq!(m.len(), 2, "{m:?}");
        assert_eq!(m[0], linux::Monitor { name: "HDMI-1".into(), primary: true, size: Some((2560, 1440)), hz: Some(59.95), pos: Some((0, 0)) });
        assert_eq!(m[1], linux::Monitor { name: "eDP-1".into(), primary: false, size: Some((1920, 1080)), hz: Some(144.0), pos: Some((2560, 180)) });
        assert_eq!(linux::xrandr_monitors("Can't open display :9\n"), vec![]);
        assert_eq!(linux::parse_mode("3840x2160\n"), Some((3840, 2160)));
        assert_eq!(linux::parse_mode("1920x1080i"), Some((1920, 1080)));
        assert_eq!(linux::lpstat_default("system default destination: Office_Laser\n").as_deref(), Some("Office_Laser"));
        assert_eq!(linux::lpstat_default("no system default destination\n"), None);
        let devices = "device for Office_Laser: ipp://printer.local:631/ipp/print\ndevice for PDF: cups-pdf:/\n";
        assert_eq!(linux::lpstat_devices(devices), vec![("Office_Laser".to_string(), "ipp://printer.local:631/ipp/print".to_string()), ("PDF".to_string(), "cups-pdf:/".to_string())]);
        assert_eq!(linux::lpstat_devices("lpstat: Scheduler is not running.\n"), vec![]);
    }

    #[test]
    fn linux_loaded_libraries() {
        let maps = "55f0c0000000-55f0c0100000 r--p 00000000 08:20 1234                       /opt/AnyVector/anyvector
55f0c0100000-55f0c0900000 r-xp 00100000 08:20 1234                       /opt/AnyVector/anyvector
7f00000000-7f00001000 r--p 00000000 08:20 99                         /usr/lib/x86_64-linux-gnu/libc.so.6
7f00001000-7f00002000 r-xp 00001000 08:20 99                         /usr/lib/x86_64-linux-gnu/libc.so.6
7f00003000-7f00004000 r--p 00000000 08:20 98                         /usr/lib/x86_64-linux-gnu/libvulkan_lvp.so
7f00005000-7f00006000 r--p 00000000 08:20 97                         /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
7f00007000-7f00008000 r--p 00000000 08:20 96                         /etc/ld.so.cache
7f00009000-7f0000a000 rw-s 00000000 00:01 95                         /memfd:wayland-shm (deleted)
7f0000b000-7f0000c000 rw-s 00000000 00:05 94                         /dev/dri/renderD128
7f0000d000-7f0000e000 r--p 00000000 08:20 93                         /home/me/My Libs/libfoo.so.1.2 (deleted)
7f0000f000-7f00010000 r--p 00000000 08:20 92                         /usr/lib/locale/C.utf8/LC_CTYPE
7ffd00000000-7ffd00021000 rw-p 00000000 00:00 0                          [stack]
7ffd00100000-7ffd00102000 r-xp 00000000 00:00 0                          [vdso]
";
        assert_eq!(
            linux::mapped_modules(maps, Some("/opt/AnyVector/anyvector")),
            vec!["/opt/AnyVector/anyvector", "/usr/lib/x86_64-linux-gnu/libc.so.6", "/usr/lib/x86_64-linux-gnu/libvulkan_lvp.so", "/home/me/My Libs/libfoo.so.1.2"]
        );
        // without the program's path, only libraries
        assert_eq!(linux::mapped_modules(maps, None).len(), 3);
    }
}
