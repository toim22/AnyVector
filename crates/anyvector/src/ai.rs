//! AI Generate: turns a text prompt into editable vector objects.
//!
//! Asks the chosen AI provider (Anthropic's Messages API by default; any OpenAI-compatible
//! endpoint; Ollama or LM Studio on this computer, see `ai_provider.rs`) for a self-contained SVG,
//! which is imported as native AnyVector objects (paths, gradients, groups). Create image and
//! Remix image can use ComfyUI instead (`comfyui.rs`), whose image is placed as a bitmap.
//! API keys are read from the environment variable named in Settings (default
//! `ANTHROPIC_API_KEY`, or `OPENAI_API_KEY` for OpenAI-compatible endpoints); they are never
//! stored in settings or documents. Tools ▸ Options ▸ Global ▸ Privacy can write the key into
//! that variable of the user's Windows account. Local providers need no key.

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
/// (so a key saved in Options, or with Windows' Environment Variables, works without a restart).
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
fn user_env(_name: &str) -> Option<String> {
    None
}

/// Write the API key into the environment variable `name` of the user's Windows account (an
/// empty key removes it), and into this process's environment. Other programs see it when they
/// next start.
pub fn save_user_key(name: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    if name.trim().is_empty() {
        return Err("Type the name of the environment variable first.".into());
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
        return Err(format!("AI Generate needs an API key. Add your key in {KEY_PLACE}: AnyVector keeps it in the {var} environment variable of your Windows account, never in its own files."));
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
    egui::Window::new("Quick AI Generate").open(&mut open).collapsible(false).resizable(false).default_width(420.0).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
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
