#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! AnyVector — vector graphics editor for Windows.

mod about;
mod ai;
mod ai_provider;
mod app;
mod blendpath;
mod bugreport;
mod canvas;
mod cmdbar;
mod color_profiles;
mod comfyui;
mod commands;
mod customize;
mod dialogs;
mod dockers;
mod docview;
mod bitmaps;
mod exec;
mod file_copy;
mod fx;
mod fxdata;
mod help;
mod i18n;
mod icons;
mod lengths;
mod menus;
mod palette;
mod powerclip;
mod printing;
mod propbar;
mod renderer;
mod save_dialog;
mod scripting;
mod settings;
mod shortcuts;
mod spell;
mod splash;
mod statusbar;
mod sysinfo;
mod tabs;
mod theme;
mod toolbar;
mod toolbox;
mod tools;
mod trace;
mod updater;
mod ui;
mod welcome;
mod widgets;
mod workspaces;

pub const ICON_PNG: &[u8] = include_bytes!("../assets/icon-256.png");
/// The logo (the airship alone), drawn from `assets/brand/anyvector-logo.svg` by `cargo run -p xtask -- brand`.
pub const LOGO_PNG: &[u8] = include_bytes!("../assets/logo.png");

/// The application id of the window: its Wayland app_id and X11 WM_CLASS. On Linux the desktop
/// file must be named after it (`io.github.toim22.AnyVector.desktop`, or say so with
/// `StartupWMClass`): that is how GNOME and KDE find the window's icon and name and group it
/// with its launcher. Wayland has no window icon of its own, only the desktop file's.
#[cfg(target_os = "linux")]
pub const APP_ID: &str = "io.github.toim22.AnyVector";
#[cfg(not(target_os = "linux"))]
pub const APP_ID: &str = "AnyVector";

fn window_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(ICON_PNG).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width: w, height: h })
}

fn native_options() -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(vector_core::APP_NAME)
        .with_app_id(APP_ID)
        // the window starts as an undecorated splash and becomes the main window once loaded
        .with_inner_size(splash::SPLASH_SIZE)
        .with_decorations(false)
        .with_resizable(false)
        .with_transparent(true)
        .with_min_inner_size(splash::MIN_WINDOW_SIZE);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    eframe::NativeOptions {
        viewport,
        centered: true,
        renderer: eframe::Renderer::Wgpu,
        persist_window: false,
        ..Default::default()
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> eframe::Result<()> {
    eframe::run_native(vector_core::APP_NAME, native_options(), Box::new(|cc| Ok(Box::new(app::AnyVectorApp::new(cc)))))
}

/// Linux: starts with wgpu and, when that fails before the window has opened (a graphics
/// driver wgpu can't use), starts again with OpenGL (see `renderer.rs`).
#[cfg(target_os = "linux")]
fn main() -> eframe::Result<()> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let first = renderer::from_env(std::env::var(renderer::ENV).ok().as_deref());
    if first == eframe::Renderer::Glow {
        return run(first, Arc::default());
    }
    let started = Arc::new(AtomicBool::new(false));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(first, Arc::clone(&started))));
    match renderer::wgpu_failure(started.load(Ordering::SeqCst), &result) {
        Some(reason) => {
            eprintln!("AnyVector: wgpu can't draw the window ({reason}); starting again with OpenGL");
            let err = renderer::restart_with_opengl();
            eprintln!("AnyVector: can't start again ({err}); trying OpenGL in this process");
            run(eframe::Renderer::Glow, Arc::default())
        }
        None => result.unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
    }
}

/// Opens the window drawn by `backend` and runs AnyVector until it closes. `started` is set once
/// the renderer works and the app is being created.
#[cfg(target_os = "linux")]
fn run(backend: eframe::Renderer, started: std::sync::Arc<std::sync::atomic::AtomicBool>) -> eframe::Result<()> {
    let mut options = native_options();
    options.renderer = backend;
    eframe::run_native(
        vector_core::APP_NAME,
        options,
        Box::new(move |cc| {
            started.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(Box::new(app::AnyVectorApp::new(cc)))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_id_names_the_desktop_file_on_linux() {
        if cfg!(target_os = "linux") {
            assert_eq!(APP_ID, "io.github.toim22.AnyVector");
        } else {
            assert_eq!(APP_ID, "AnyVector");
        }
        // a valid desktop file id (and D-Bus name): dot-separated elements of ASCII letters,
        // digits, `_` and `-`, none empty or starting with a digit
        let parts: Vec<&str> = APP_ID.split('.').collect();
        assert!(parts.len() >= 3 || !cfg!(target_os = "linux"), "{APP_ID}");
        for p in parts {
            assert!(!p.is_empty() && !p.starts_with(|c: char| c.is_ascii_digit()), "{APP_ID}");
            assert!(p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'), "{APP_ID}");
        }
    }

    #[test]
    fn window_opens_as_the_splash() {
        let o = native_options();
        assert_eq!(o.viewport.app_id.as_deref(), Some(APP_ID));
        assert_eq!(o.viewport.title.as_deref(), Some(vector_core::APP_NAME));
        assert_eq!(o.viewport.inner_size, Some(egui::vec2(620.0, 380.0)));
        assert_eq!(o.viewport.min_inner_size, Some(egui::vec2(480.0, 320.0)));
        assert_eq!((o.viewport.decorations, o.viewport.resizable, o.viewport.transparent), (Some(false), Some(false), Some(true)));
        assert!(o.viewport.icon.is_some(), "the window icon (X11 and Windows)");
        // wgpu first everywhere; on Linux OpenGL only when wgpu fails or is turned off
        assert_eq!(o.renderer, eframe::Renderer::Wgpu);
    }
}

#[cfg(test)]
mod visual_tests;
