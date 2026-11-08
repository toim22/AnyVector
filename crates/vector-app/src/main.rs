#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Vector — vector graphics editor for Windows.

mod ai;
mod app;
mod bugreport;
mod canvas;
mod color_profiles;
mod commands;
mod dialogs;
mod dockers;
mod docview;
mod exec;
mod icons;
mod menus;
mod palette;
mod propbar;
mod save_dialog;
mod scripting;
mod settings;
mod shortcuts;
mod splash;
mod statusbar;
mod tabs;
mod theme;
mod toolbar;
mod toolbox;
mod tools;
mod trace;
mod welcome;
mod widgets;

pub const ICON_PNG: &[u8] = include_bytes!("../assets/icon-256.png");
pub const SPLASH_PNG: &[u8] = include_bytes!("../assets/splash-airship.png");

fn window_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(ICON_PNG).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width: w, height: h })
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(vector_core::APP_NAME)
        .with_app_id("Vector")
        // the window starts as an undecorated splash and becomes the main window once loaded
        .with_inner_size([620.0, 380.0])
        .with_decorations(false)
        .with_resizable(false)
        .with_transparent(true)
        .with_min_inner_size([480.0, 320.0]);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        renderer: eframe::Renderer::Wgpu,
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(vector_core::APP_NAME, options, Box::new(|cc| Ok(Box::new(app::VectorApp::new(cc)))))
}

#[cfg(test)]
mod visual_tests;
