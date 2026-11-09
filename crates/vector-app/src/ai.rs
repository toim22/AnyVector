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

/// What AI Generate makes (Object ▸ AI Generate): an editable vector graphic, an image, or a
/// remix of a source image. Images and remixes are placed as bitmaps.
pub const MODES: [&str; 3] = ["Create vector graphic", "Create image", "Remix image"];

/// The selection loaded as reference (Create … Guided by Selection) or as the source of a
/// remix (Remix Selection).
pub struct Reference {
    /// The selection rendered as PNG.
    pub png: Vec<u8>,
    /// The selection as SVG (sent with a remix).
    pub svg: String,
    pub bounds: kurbo::Rect,
    pub count: usize,
}

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
    /// 0 vector graphic, 1 image, 2 remix.
    pub mode: u8,
    pub reference: Option<Reference>,
    rx: Option<Receiver<Result<String, String>>>,
    started: f64,
    /// The request's mode and where its reference was.
    pending: (u8, Option<kurbo::Rect>),
}

impl Default for AiState {
    fn default() -> Self {
        AiState { prompt: String::new(), style: 0, colors: 6, busy: false, status: String::new(), error: None, quick_open: false, history: vec![], mode: 0, reference: None, rx: None, started: 0.0, pending: (0, None) }
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
    let mut user = format!(
        "Create: {}\nStyle: {}\nUse a palette of about {} harmonious colors.",
        app.ai.prompt.trim(),
        STYLES[app.ai.style.min(STYLES.len() - 1)],
        app.ai.colors
    );
    if app.ai.mode == 1 {
        user.push_str("\nThis becomes a bitmap image: make it rich and detailed, with gradients, shading and a full background.");
    }
    let content = match &app.ai.reference {
        None => serde_json::json!(user),
        Some(r) => {
            let text = if app.ai.mode == 2 {
                format!(
                    "Remix the attached artwork: keep its key elements and composition, and change it as described.\n{user}\nThe artwork's SVG source:\n{}",
                    r.svg.chars().take(60_000).collect::<String>()
                )
            } else {
                format!("Use the attached artwork as the reference for the composition, proportions and content.\n{user}")
            };
            serde_json::json!([
                { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": base64(&r.png) } },
                { "type": "text", "text": text },
            ])
        }
    };
    app.ai.pending = (app.ai.mode, app.ai.reference.as_ref().map(|r| r.bounds));
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
        let _ = tx.send(request(&url, &key, content));
    });
}

fn request(url: &str, key: &str, user: serde_json::Value) -> Result<String, String> {
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
pub fn guided(app: &mut VectorApp, mode: u8) {
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
pub fn poll(app: &mut VectorApp) {
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
        Ok(svg) => {
            if !app.has_doc() {
                app.new_default_document();
            }
            let name = format!("AI: {}", app.ai.prompt.chars().take(40).collect::<String>());
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
                        match bitmap_of(&mut d.doc, &svg, fit, &name) {
                            Some(o) => o,
                            None => {
                                app.ai.error = Some("Could not render the generated image.".into());
                                return;
                            }
                        }
                    };
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

/// The generated SVG as a bitmap object filling `at` (300 dpi, at most 2048 pixels).
fn bitmap_of(doc: &mut vector_core::Document, svg: &str, at: kurbo::Rect, name: &str) -> Option<vector_core::Object> {
    let k = (300.0 / 72.0f64).min(2048.0 / at.width().max(at.height()).max(1.0));
    let (w, h) = ((at.width() * k).round().max(1.0) as u32, (at.height() * k).round().max(1.0) as u32);
    let img = crate::icons::rasterize_svg(svg, w, h)?;
    let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    let data = vector_core::document::ImageData { width: w, height: h, rgba: std::sync::Arc::new(rgba), encoded: std::sync::Arc::new(vec![]), format: "png".into(), name: name.to_string() };
    let iid = doc.add_image(data);
    let mut st = vector_core::Style::default();
    st.fill = vector_core::Fill::None;
    st.outline = vector_core::Outline::none();
    let io = vector_core::object::ImageObj { image: iid, w: at.width(), h: at.height(), crop: [0.0, 0.0, 1.0, 1.0], adjust: Default::default() };
    let mut o = vector_core::Object::new(doc.new_id(), vector_core::ObjKind::Image(io), kurbo::Affine::translate(at.origin().to_vec2()), st);
    o.name = name.to_string();
    Some(o)
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
    ui.horizontal(|ui| {
        for (i, m) in MODES.iter().enumerate() {
            ui.selectable_value(&mut app.ai.mode, i as u8, *m);
        }
    });
    let mut clear = false;
    if let Some(r) = &app.ai.reference {
        ui.horizontal(|ui| {
            let what = if app.ai.mode == 2 { "Source image" } else { "Reference image" };
            ui.label(egui::RichText::new(format!("{what}: the selection ({} objects)", r.count)).color(crate::theme::TEXT_DIM));
            clear = ui.small_button("Clear reference content").clicked();
        });
    } else if app.ai.mode == 2 {
        ui.label(egui::RichText::new("Select the artwork to remix, then choose Object ▸ AI Generate ▸ Remix Selection.").color(crate::theme::TEXT_DIM));
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
    ui.horizontal(|ui| {
        let can = !app.ai.busy && app.ai.prompt.trim().chars().count() >= 3 && (app.ai.mode != 2 || app.ai.reference.is_some());
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
