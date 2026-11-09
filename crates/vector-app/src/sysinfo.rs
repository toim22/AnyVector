//! The facts shown by Help ▸ About Vector ▸ System Info..., one list per category.

/// The categories of the "Choose a category" list, in the reference's order.
pub const CATEGORIES: [&str; 5] = ["System", "Display", "Printing", "AnyVector EXEs & DLLs", "System DLLs"];

pub type Rows = Vec<(String, String)>;

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
    let mut s = format!("AnyVector {} - System Info\r\n", vector_core::APP_VERSION);
    for (i, name) in CATEGORIES.iter().enumerate() {
        s.push_str(&format!("\r\n[{name}]\r\n"));
        for (k, v) in collect(i, gpu) {
            s.push_str(&format!("{k:<34}{v}\r\n"));
        }
    }
    s
}

fn app_dir() -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default()
}

fn with_backslash(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    if s.ends_with('\\') {
        s
    } else {
        format!("{s}\\")
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

#[cfg(not(windows))]
fn system() -> Rows {
    vec![
        ("Platform:".into(), std::env::consts::OS.into()),
        ("Number of Processors:".into(), std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).to_string()),
        ("TEMP directory:".into(), with_backslash(&std::env::temp_dir())),
        ("Application directory:".into(), app_dir().display().to_string()),
    ]
}

#[cfg(not(windows))]
fn display(gpu: &str) -> Rows {
    vec![("Graphics adapter:".into(), gpu.to_string())]
}

#[cfg(not(windows))]
fn printing() -> Rows {
    vec![]
}

#[cfg(not(windows))]
fn modules(_own: bool) -> Rows {
    vec![]
}

#[cfg(test)]
mod tests {
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
    }
}
