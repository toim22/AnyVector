//! AI Generate: turns a text prompt into editable vector objects.
//!
//! Calls the Claude Messages API (raw HTTP, there is no official Rust SDK) and asks for a
//! self-contained SVG, which is imported as native Vector objects (paths, gradients, groups).
//! The API key is read from the environment variable named in Settings (default
//! `ANTHROPIC_API_KEY`); it is never stored in settings or documents.

use crate::app::VectorApp;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

pub const MODEL: &str = "claude-opus-5-5";
pub const DEFAULT_URL: &str = "https://api.anthropic.com/v1/messages";

pub const STYLES: [&str; 8] = ["Flat illustration", "Line art", "Logo / icon", "Isometric", "Geometric pattern", "Cartoon", "Infographic", "Silhouette"];

pub struct AiState {
    pub prompt: String,
    pub style: usize,
    pub colors: usize,
    pub busy: bool,
    pub status: String,
    pub error: Option<String>,
    /// Quick AI Generate dialog visible.
    pub quick_open: bool,
    pub history: Vec<String>,
    rx: Option<Receiver<Result<String, String>>>,
    started: f64,
}

impl Default for AiState {
    fn default() -> Self {
        AiState { prompt: String::new(), style: 0, colors: 6, busy: false, status: String::new(), error: None, quick_open: false, history: vec![], rx: None, started: 0.0 }
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

pub fn api_key(app: &VectorApp) -> Option<String> {
    let name = if app.settings.ai_api_key_env.trim().is_empty() { "ANTHROPIC_API_KEY".to_string() } else { app.settings.ai_api_key_env.trim().to_string() };
    std::env::var(name).ok().filter(|k| !k.trim().is_empty())
}

/// Start a generation request in the background.
pub fn start(app: &mut VectorApp) {
    if app.ai.busy || app.ai.prompt.trim().is_empty() {
        return;
    }
    let Some(key) = api_key(app) else {
        let var = if app.settings.ai_api_key_env.is_empty() { "ANTHROPIC_API_KEY" } else { app.settings.ai_api_key_env.as_str() };
        app.ai.error = Some(format!("AI Generate needs an API key. Set the {var} environment variable and restart Vector."));
        return;
    };
    let url = if app.settings.ai_provider_url.trim().is_empty() { DEFAULT_URL.to_string() } else { app.settings.ai_provider_url.trim().to_string() };
    let user = format!(
        "Create: {}\nStyle: {}\nUse a palette of about {} harmonious colors.",
        app.ai.prompt.trim(),
        STYLES[app.ai.style.min(STYLES.len() - 1)],
        app.ai.colors
    );
    let (tx, rx) = channel();
    app.ai.rx = Some(rx);
    app.ai.busy = true;
    app.ai.error = None;
    app.ai.status = "Generating...".into();
    app.ai.started = app.time();
    if !app.ai.history.contains(&app.ai.prompt) {
        app.ai.history.insert(0, app.ai.prompt.clone());
        app.ai.history.truncate(12);
    }
    std::thread::spawn(move || {
        let _ = tx.send(request(&url, &key, &user));
    });
}

fn request(url: &str, key: &str, user: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 16000,
        "output_config": { "effort": "medium" },
        "fallbacks": "default",
        "system": system_prompt(),
        "messages": [{ "role": "user", "content": user }],
    });
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(Duration::from_secs(600))).build().into();
    let mut resp = agent
        .post(url)
        .header("content-type", "application/json")
        .header("x-api-key", key)
        .header("anthropic-version", "2026-10-21")
        .header("anthropic-beta", "server-side-fallback-2026-07-01")
        .send_json(&body)
        .map_err(|e| format!("Could not reach the AI service: {e}"))?;
    let status = resp.status().as_u16();
    let v: serde_json::Value = resp.body_mut().read_json().map_err(|e| format!("Invalid response from the AI service: {e}"))?;
    if status >= 400 {
        let msg = v["error"]["message"].as_str().unwrap_or("unknown error");
        return Err(format!("AI service error ({status}): {msg}"));
    }
    match v["stop_reason"].as_str() {
        Some("refusal") => {
            let why = v["stop_details"]["explanation"].as_str().unwrap_or("The request was declined.");
            return Err(format!("The AI declined this prompt. {why}"));
        }
        Some("max_tokens") => return Err("The drawing was too complex and got cut off. Try a simpler prompt.".into()),
        _ => {}
    }
    let text: String = v["content"].as_array().map(|a| a.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("")).unwrap_or_default();
    extract_svg(&text).ok_or_else(|| "The AI did not return a drawing. Try rephrasing the prompt.".into())
}

fn extract_svg(text: &str) -> Option<String> {
    let start = text.find("<svg")?;
    let end = text.rfind("</svg>")? + "</svg>".len();
    (end > start).then(|| text[start..end].to_string())
}

/// Poll the background request; on success insert the objects into the active document.
pub fn poll(app: &mut VectorApp) {
    let Some(rx) = &app.ai.rx else { return };
    let Ok(res) = rx.try_recv() else {
        let secs = (app.time() - app.ai.started) as u64;
        app.ai.status = format!("Generating... {secs}s");
        return;
    };
    app.ai.rx = None;
    app.ai.busy = false;
    match res {
        Err(e) => {
            app.ai.status.clear();
            app.ai.error = Some(e);
        }
        Ok(svg) => {
            if !app.has_doc() {
                app.new_default_document();
            }
            let Some(d) = app.doc_mut() else { return };
            match vector_io::svg_import::objects_from_svg(&mut d.doc, svg.as_bytes()) {
                Ok(mut objs) if !objs.is_empty() => {
                    // fit into the page at 80% and centre
                    let page = d.doc.pages[d.page].rect();
                    let bb = vector_core::object::union_bounds(objs.iter().map(|o| o.bounds()));
                    if bb.width() > 0.0 && bb.height() > 0.0 {
                        let s = (page.width() * 0.8 / bb.width()).min(page.height() * 0.8 / bb.height());
                        let a = kurbo::Affine::translate(page.center().to_vec2()) * kurbo::Affine::scale(s) * kurbo::Affine::translate(-bb.center().to_vec2());
                        for o in objs.iter_mut() {
                            o.apply_transform(a);
                        }
                    }
                    let g = d.doc.new_id();
                    let mut group = vector_core::Object::new(g, vector_core::ObjKind::Group(objs), kurbo::Affine::IDENTITY, Default::default());
                    group.name = format!("AI: {}", app.ai.prompt.chars().take(40).collect::<String>());
                    let label = "AI Generate";
                    if let Some(d) = app.doc_mut() {
                        d.add_objects(label, vec![group]);
                    }
                    app.ai.status = "Done".into();
                }
                Ok(_) => app.ai.error = Some("The generated drawing was empty.".into()),
                Err(e) => app.ai.error = Some(format!("Could not read the generated drawing: {e}")),
            }
        }
    }
}

/// Quick AI Generate dialog (File > Quick AI Generate...).
pub fn quick_dialog(app: &mut VectorApp, ctx: &egui::Context) {
    if !app.ai.quick_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Quick AI Generate").open(&mut open).collapsible(false).resizable(false).default_width(420.0).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
        panel(app, ui);
    });
    if !open {
        app.ai.quick_open = false;
    }
}

/// Shared UI used by the AI Generate docker and the quick dialog.
pub fn panel(app: &mut VectorApp, ui: &mut egui::Ui) {
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
    ui.horizontal(|ui| {
        let can = !app.ai.busy && !app.ai.prompt.trim().is_empty();
        let b = egui::Button::new(egui::RichText::new("✦ Generate").color(egui::Color32::WHITE)).fill(crate::theme::PRESSED).min_size(egui::vec2(100.0, 26.0));
        if ui.add_enabled(can, b).clicked() {
            start(app);
        }
        if app.ai.busy {
            ui.spinner();
        }
        ui.label(egui::RichText::new(&app.ai.status).color(crate::theme::TEXT_DIM));
    });
    if let Some(e) = &app.ai.error {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(e).color(egui::Color32::from_rgb(0xe8, 0x6a, 0x6a)));
    }
    if !app.ai.history.is_empty() {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Recent prompts").color(crate::theme::TEXT_DIM));
        let hist = app.ai.history.clone();
        for h in hist {
            if ui.link(h.chars().take(60).collect::<String>()).clicked() {
                app.ai.prompt = h;
            }
        }
    }
    ui.add_space(6.0);
    ui.label(egui::RichText::new(format!("Results are editable vector objects. Powered by Claude ({MODEL}).")).small().color(crate::theme::TEXT_DIM));
}
