//! File ▸ Acquire Image off Windows, through SANE (`sane.rs`), in the steps the Windows version
//! takes through WIA:
//!
//! - Select Source (the WIA and TWAIN items alike): the scanners `scanimage -L` finds; the chosen
//!   one is kept in the settings, as the WIA device is.
//! - Acquire: the resolution and color mode (what the WIA scan dialog asks), then the scan in the
//!   background to a PNG in the private temp folder, imported as the Windows scan is. Without a
//!   chosen scanner it is chosen first (or taken, when there is only one).

use super::sane::{self, Device, DeviceOptions};
use super::{ok_cancel_plain, window};
use crate::app::AnyVectorApp;
use egui::vec2;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

/// `scanimage` is not installed.
pub const NO_SANE: &str = "Scanning on Linux uses SANE, and its scanimage program was not found. Install it with your package manager (sane-utils on Debian and Ubuntu, sane-backends on Fedora and Arch) together with your scanner's SANE driver, then try again.";
/// SANE found no scanner.
pub const NO_SCANNER: &str = "No scanner was found. Check that the scanner is connected and turned on, and that its SANE driver is installed (for network scanners, sane-airscan). The command scanimage -L lists the scanners SANE sees.";
/// Added to the two messages above when GNOME's scanning program is there.
pub const SIMPLE_SCAN: &str = "Document Scanner (simple-scan) is installed: you can scan with it, save the image and bring it in with File ▸ Import.";

/// Why the device list could not be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListError {
    NotInstalled,
    Failed(String),
}

enum Stage {
    /// `scanimage -L` is running.
    Looking(Receiver<Result<Vec<Device>, ListError>>),
    /// The device list.
    Choose { devices: Vec<Device>, sel: usize },
    /// The scan settings; the device's own options are being read while `reading` is there.
    Settings { device: Device, reading: Option<Receiver<Option<DeviceOptions>>>, opts: DeviceOptions, mode: usize, dpi: usize },
}

pub struct ScanState {
    /// Acquire (scan once a device is chosen) rather than Select Source.
    acquire: bool,
    stage: Stage,
}

impl ScanState {
    fn title(&self) -> &'static str {
        if self.acquire {
            "Acquire Image"
        } else {
            "Select Source"
        }
    }

    /// Tests: the dialog showing `devices` to choose from.
    #[cfg(test)]
    pub fn choosing(devices: Vec<Device>, acquire: bool) -> ScanState {
        ScanState { acquire, stage: Stage::Choose { devices, sel: 0 } }
    }

    /// Tests: the scan settings of `device` with options already read.
    #[cfg(test)]
    pub fn settings(device: Device, opts: DeviceOptions) -> ScanState {
        ScanState { acquire: true, stage: Stage::Settings { device, reading: None, opts, mode: 0, dpi: 0 } }
    }

    /// Tests: the device list as `scanimage -L` would have given it.
    #[cfg(test)]
    pub fn listed(r: Result<Vec<Device>, ListError>, acquire: bool) -> ScanState {
        let (tx, rx) = channel();
        let _ = tx.send(r);
        ScanState { acquire, stage: Stage::Looking(rx) }
    }
}

/// Whether a program is on the PATH.
fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

/// A message about scanning, with a pointer to Document Scanner when it is installed.
fn with_simple_scan(text: &str) -> String {
    if on_path("simple-scan") {
        format!("{text}\n\n{SIMPLE_SCAN}")
    } else {
        text.to_string()
    }
}

/// Run `scanimage` with `args`; its output, or why it failed.
fn scanimage(args: &[&str]) -> Result<String, ListError> {
    match std::process::Command::new(sane::SCANIMAGE).args(args).stdin(std::process::Stdio::null()).output() {
        Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).into_owned()),
        Ok(o) => Err(ListError::Failed(sane::error_line(&String::from_utf8_lossy(&o.stderr)))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(ListError::NotInstalled),
        Err(e) => Err(ListError::Failed(e.to_string())),
    }
}

fn list_devices() -> Receiver<Result<Vec<Device>, ListError>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(scanimage(&["-L"]).map(|o| sane::parse_devices(&o)));
    });
    rx
}

fn read_options(device: &str) -> Receiver<Option<DeviceOptions>> {
    let (tx, rx) = channel();
    let device = device.to_string();
    std::thread::spawn(move || {
        let _ = tx.send(scanimage(&["-d", &device, "-A"]).ok().map(|o| sane::parse_options(&o)));
    });
    rx
}

/// The scan settings of `device`, its options read in the background.
fn settings_stage(device: Device) -> Stage {
    let reading = Some(read_options(&device.name));
    Stage::Settings { device, reading, opts: DeviceOptions::default(), mode: 0, dpi: 0 }
}

/// File ▸ Acquire Image ▸ Select WIA/TWAIN Source.
pub fn select_source(app: &mut AnyVectorApp) {
    app.dialogs.scan = Some(ScanState { acquire: false, stage: Stage::Looking(list_devices()) });
}

/// File ▸ Acquire Image ▸ Acquire.
pub fn acquire(app: &mut AnyVectorApp) {
    let stage = match app.settings.scan.device.clone() {
        Some(name) => {
            let label = app.settings.scan.label.clone().unwrap_or_else(|| name.clone());
            settings_stage(Device { name, label })
        }
        None => Stage::Looking(list_devices()),
    };
    app.dialogs.scan = Some(ScanState { acquire: true, stage });
}

/// Scan with `scanimage` to `out` (a PNG); what `poll_background` takes: the file to import, or
/// the error to show.
fn scan_to(args: Vec<String>, out: PathBuf) -> Result<String, String> {
    let file = std::fs::File::create(&out).map_err(|e| format!("The image could not be acquired:\n{e}"))?;
    let r = std::process::Command::new(sane::SCANIMAGE).args(&args).stdin(std::process::Stdio::null()).stdout(file).stderr(std::process::Stdio::piped()).output();
    let ok = match &r {
        Ok(o) => o.status.success() && std::fs::metadata(&out).is_ok_and(|m| m.len() > 0),
        Err(_) => false,
    };
    if ok {
        return Ok(format!("\u{1}import:{}", out.display()));
    }
    let _ = std::fs::remove_file(&out);
    Err(match r {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => with_simple_scan(NO_SANE),
        Err(e) => format!("The image could not be acquired:\n{e}"),
        Ok(o) => format!("The image could not be acquired:\n{}", sane::error_line(&String::from_utf8_lossy(&o.stderr))),
    })
}

/// Start the scan in the background; the image is imported when it is there.
fn start_scan(app: &mut AnyVectorApp, device: &Device, mode: Option<&str>, dpi: Option<u32>) {
    let s = &mut app.settings.scan;
    s.device = Some(device.name.clone());
    s.label = Some(device.label.clone());
    if mode.is_some() {
        s.mode = mode.map(String::from);
    }
    if dpi.is_some() {
        s.dpi = dpi;
    }
    let out = crate::platform::temp_dir().join(format!("anyvector-scan-{}.png", std::process::id()));
    let args = sane::scan_args(&device.name, mode, dpi);
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(scan_to(args, out));
    });
    app.background.push(rx);
    app.flash("Waiting for the scanner...");
}

/// The device list arrived: on to the next step, or say why there is none.
fn listed(app: &mut AnyVectorApp, st: &mut ScanState, r: Result<Vec<Device>, ListError>) -> bool {
    let title = st.title();
    match r {
        Err(ListError::NotInstalled) => {
            app.dialogs.message_box = Some((title.into(), with_simple_scan(NO_SANE)));
            false
        }
        Err(ListError::Failed(e)) => {
            app.dialogs.message_box = Some((title.into(), format!("Looking for scanners failed:\n{e}")));
            false
        }
        Ok(devices) if devices.is_empty() => {
            app.dialogs.message_box = Some((title.into(), with_simple_scan(NO_SCANNER)));
            false
        }
        // Acquire with a single scanner: no need to ask which
        Ok(mut devices) if st.acquire && devices.len() == 1 => {
            st.stage = settings_stage(devices.remove(0));
            true
        }
        Ok(devices) => {
            let sel = app.settings.scan.device.as_ref().and_then(|n| devices.iter().position(|d| &d.name == n)).unwrap_or(0);
            st.stage = Stage::Choose { devices, sel };
            true
        }
    }
}

pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.scan.take() else { return };
    let title = st.title();
    // results of the background work
    let list = match &st.stage {
        Stage::Looking(rx) => Some(rx.try_recv()),
        _ => None,
    };
    match list {
        Some(Ok(r)) => {
            if !listed(app, &mut st, r) {
                return;
            }
        }
        Some(Err(TryRecvError::Disconnected)) => return,
        Some(Err(TryRecvError::Empty)) => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
        None => {}
    }
    let last_mode = app.settings.scan.mode.clone();
    let last_dpi = app.settings.scan.dpi;
    if let Stage::Settings { reading, opts, mode, dpi, .. } = &mut st.stage {
        if let Some(rx) = reading {
            let got = match rx.try_recv() {
                Ok(o) => Some(o),
                Err(TryRecvError::Disconnected) => Some(None),
                Err(TryRecvError::Empty) => None,
            };
            match got {
                // a device that says nothing about its options: the usual ones
                Some(o) => {
                    *opts = o.unwrap_or_else(sane::fallback_options);
                    *mode = sane::initial_mode(opts, last_mode.as_deref());
                    *dpi = sane::initial_dpi(opts, last_dpi);
                    *reading = None;
                }
                None => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
            }
        }
    }
    let size = match st.stage {
        Stage::Choose { .. } => vec2(440.0, 300.0),
        _ => vec2(400.0, 200.0),
    };
    let ((ok, cancel), dismissed) = window(ctx, "scan", title, size, |ui| {
        ui.add_space(10.0);
        match &mut st.stage {
            Stage::Looking(_) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Looking for scanners...");
                });
                ui.add_space((ui.available_height() - 30.0).max(0.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| (false, super::button(ui, "Cancel", true))).inner
            }
            Stage::Choose { devices, sel } => {
                ui.label("Choose the scanner to use:");
                ui.add_space(6.0);
                egui::Frame::new().fill(crate::theme::FIELD()).stroke(egui::Stroke::new(1.0, crate::theme::SEP())).inner_margin(4.0).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::vertical().max_height(170.0).auto_shrink([false, false]).show(ui, |ui| {
                        for (i, d) in devices.iter().enumerate() {
                            let text = egui::RichText::new(&d.label);
                            if ui.selectable_label(*sel == i, text).on_hover_text(&d.name).clicked() {
                                *sel = i;
                            }
                        }
                    });
                });
                ok_cancel_plain(ui, !devices.is_empty())
            }
            Stage::Settings { device, reading, opts, mode, dpi } => {
                let w = 100.0;
                super::row(ui, "Scanner:", w, |ui| ui.label(&device.label).on_hover_text(&device.name));
                ui.add_space(4.0);
                let modes: Vec<&str> = opts.modes.iter().map(|m| sane::mode_label(m)).collect();
                let dpis: Vec<String> = opts.resolutions.iter().map(|d| format!("{d} dpi")).collect();
                let dpi_names: Vec<&str> = dpis.iter().map(String::as_str).collect();
                ui.add_enabled_ui(reading.is_none(), |ui| {
                    super::row(ui, "Resolution:", w, |ui| {
                        if dpi_names.is_empty() {
                            super::combo(ui, "scan_dpi", &mut 0, &["Default"], 160.0)
                        } else {
                            super::combo(ui, "scan_dpi", dpi, &dpi_names, 160.0)
                        }
                    });
                    super::row(ui, "Color mode:", w, |ui| {
                        if modes.is_empty() {
                            super::combo(ui, "scan_mode", &mut 0, &["Default"], 160.0)
                        } else {
                            super::combo(ui, "scan_mode", mode, &modes, 160.0)
                        }
                    });
                });
                if reading.is_some() {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        super::note(ui, "Reading the scanner's settings...");
                    });
                }
                super::ok_cancel_at_bottom(ui, "Scan", reading.is_none(), false)
            }
        }
    });
    if cancel || dismissed {
        return;
    }
    if !ok {
        app.dialogs.scan = Some(st);
        return;
    }
    match st.stage {
        Stage::Choose { mut devices, sel } if sel < devices.len() => {
            let d = devices.swap_remove(sel);
            if st.acquire {
                st.stage = settings_stage(d);
                app.dialogs.scan = Some(st);
            } else {
                app.settings.scan.device = Some(d.name);
                app.settings.scan.label = Some(d.label);
                app.flash("Scanner selected.");
            }
        }
        Stage::Settings { device, opts, mode, dpi, .. } => {
            let m = opts.modes.get(mode).cloned();
            let d = opts.resolutions.get(dpi).copied();
            start_scan(app, &device, m.as_deref(), d);
        }
        _ => {}
    }
}

/// Tests: scan with a SANE device directly, as Scan in the dialog does, and wait for the result.
#[cfg(test)]
pub fn scan_now(device: &str, mode: Option<&str>, dpi: Option<u32>, out: &std::path::Path) -> Result<String, String> {
    scan_to(sane::scan_args(device, mode, dpi), out.to_path_buf())
}

/// Tests: the device list from the real `scanimage -L`.
#[cfg(test)]
pub fn devices_now() -> Result<Vec<Device>, ListError> {
    scanimage(&["-L"]).map(|o| sane::parse_devices(&o))
}
