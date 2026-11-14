//! The AI providers AI Generate can use: Anthropic's Messages API, any OpenAI-compatible Chat
//! Completions endpoint (OpenAI, OpenRouter, Groq, Together, Azure-style endpoints, local
//! servers), and the local servers Ollama and LM Studio, which need no API key. ComfyUI (see
//! `comfyui.rs`) can make the bitmaps of Create image and Remix image.
//!
//! Requests and replies are built and read by pure functions; `send` does the HTTP. API keys are
//! never part of these settings: they stay in environment variables (see `ai::api_key`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

pub const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
pub const ANTHROPIC_MODEL: &str = "claude-opus-5-5";
pub const OPENAI_URL: &str = "https://api.openai.com/v1";
pub const OPENAI_KEY_ENV: &str = "OPENAI_API_KEY";
pub const OLLAMA_URL: &str = "http://localhost:11434";
pub const LMSTUDIO_URL: &str = "http://localhost:1234/v1";
pub const COMFY_URL: &str = "http://127.0.0.1:8188";

/// The service that writes the drawings (and, unless ComfyUI is chosen, the images).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provider {
    OpenAi,
    Ollama,
    LmStudio,
    /// The default; an unknown name in the settings file also reads as Anthropic.
    #[default]
    #[serde(other)]
    Anthropic,
}

impl Provider {
    /// In the order of the Options list.
    pub const ALL: [Provider; 4] = [Provider::Anthropic, Provider::OpenAi, Provider::Ollama, Provider::LmStudio];

    pub fn label(self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic (Claude)",
            Provider::OpenAi => "OpenAI-compatible endpoint",
            Provider::Ollama => "Ollama (local)",
            Provider::LmStudio => "LM Studio (local)",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic",
            Provider::OpenAi => "the AI service",
            Provider::Ollama => "Ollama",
            Provider::LmStudio => "LM Studio",
        }
    }

    /// A server on the user's own computer, without an API key.
    pub fn is_local(self) -> bool {
        matches!(self, Provider::Ollama | Provider::LmStudio)
    }
}

/// What makes the bitmaps of Create image and Remix image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageProvider {
    ComfyUi,
    /// The chosen AI model draws the image as vectors, which are made a bitmap.
    #[default]
    #[serde(other)]
    Llm,
}

/// The AI settings beyond the Anthropic URL and key variable (which keep their old names in
/// `Settings`). No API keys here: only the names of the environment variables that hold them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub provider: Provider,
    pub anthropic_model: String,
    /// OpenAI-compatible: the base URL (…/v1), or the full …/chat/completions URL.
    pub openai_url: String,
    pub openai_model: String,
    pub openai_key_env: String,
    /// The model accepts images (vision content parts).
    pub openai_vision: bool,
    pub ollama_url: String,
    pub ollama_model: String,
    /// Use Ollama's own /api/chat instead of its OpenAI-compatible /v1 API.
    pub ollama_native: bool,
    pub ollama_vision: bool,
    pub lmstudio_url: String,
    pub lmstudio_model: String,
    pub lmstudio_vision: bool,
    /// How long to wait for an answer, in seconds (local models can be slow).
    pub timeout_secs: u64,
    pub image_provider: ImageProvider,
    pub comfy_url: String,
    /// An API-format workflow file; empty for the built-in workflows.
    pub comfy_workflow: String,
    /// The checkpoint for the built-in workflows; empty for the first one installed.
    pub comfy_checkpoint: String,
    pub comfy_width: u32,
    pub comfy_height: u32,
    pub comfy_steps: u32,
    pub comfy_negative: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        AiConfig {
            provider: Provider::Anthropic,
            anthropic_model: ANTHROPIC_MODEL.into(),
            openai_url: OPENAI_URL.into(),
            openai_model: "gpt-4o".into(),
            openai_key_env: OPENAI_KEY_ENV.into(),
            openai_vision: true,
            ollama_url: OLLAMA_URL.into(),
            ollama_model: String::new(),
            ollama_native: false,
            ollama_vision: false,
            lmstudio_url: LMSTUDIO_URL.into(),
            lmstudio_model: String::new(),
            lmstudio_vision: false,
            timeout_secs: 600,
            image_provider: ImageProvider::Llm,
            comfy_url: COMFY_URL.into(),
            comfy_workflow: String::new(),
            comfy_checkpoint: String::new(),
            comfy_width: 1024,
            comfy_height: 1024,
            comfy_steps: 20,
            comfy_negative: "blurry, low quality, watermark, text, signature".into(),
        }
    }
}

fn or(s: &str, d: &str) -> String {
    let s = s.trim();
    if s.is_empty() { d.to_string() } else { s.to_string() }
}

impl AiConfig {
    /// The model of the chosen provider.
    pub fn model(&self) -> String {
        match self.provider {
            Provider::Anthropic => or(&self.anthropic_model, ANTHROPIC_MODEL),
            Provider::OpenAi => self.openai_model.trim().to_string(),
            Provider::Ollama => self.ollama_model.trim().to_string(),
            Provider::LmStudio => self.lmstudio_model.trim().to_string(),
        }
    }

    /// The model accepts images.
    pub fn vision(&self) -> bool {
        match self.provider {
            Provider::Anthropic => true,
            Provider::OpenAi => self.openai_vision,
            Provider::Ollama => self.ollama_vision,
            Provider::LmStudio => self.lmstudio_vision,
        }
    }

    /// The chosen provider's URL (`anthropic_url` is the Messages URL setting).
    pub fn url(&self, anthropic_url: &str) -> String {
        match self.provider {
            Provider::Anthropic => or(anthropic_url, ANTHROPIC_URL),
            Provider::OpenAi => or(&self.openai_url, OPENAI_URL),
            Provider::Ollama => or(&self.ollama_url, OLLAMA_URL),
            Provider::LmStudio => or(&self.lmstudio_url, LMSTUDIO_URL),
        }
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs.clamp(10, 24 * 3600))
    }
}

/// The host of a URL (`https://api.openai.com/v1` → `api.openai.com`).
pub fn host(url: &str) -> String {
    let rest = url.trim().split("://").nth(1).unwrap_or(url.trim());
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    let auth = auth.rsplit('@').next().unwrap_or(auth);
    if auth.starts_with('[') {
        return auth.split(']').next().unwrap_or("").trim_start_matches('[').to_string();
    }
    auth.split(':').next().unwrap_or("").to_ascii_lowercase()
}

/// The URL points at this computer.
pub fn is_local_url(url: &str) -> bool {
    let h = host(url);
    h == "localhost" || h.ends_with(".localhost") || h == "::1" || h.starts_with("127.")
}

/// Everything a chat request needs.
#[derive(Clone, Debug)]
pub struct Target {
    pub provider: Provider,
    /// Anthropic: the Messages URL. OpenAI-compatible and LM Studio: the base URL. Ollama: the
    /// server URL.
    pub url: String,
    pub model: String,
    pub key: Option<String>,
    pub vision: bool,
    pub ollama_native: bool,
    pub timeout: Duration,
}

/// An HTTP request: GET when `body` is null.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpReq {
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Value,
}

#[cfg(test)]
impl HttpReq {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|h| h.0.eq_ignore_ascii_case(name)).map(|h| h.1.as_str())
    }
}

/// Split `url` into the part before `?` and the query (with its `?`).
fn split_query(url: &str) -> (&str, &str) {
    let url = url.trim();
    match url.find('?') {
        Some(i) => (url[..i].trim_end_matches('/'), &url[i..]),
        None => (url.trim_end_matches('/'), ""),
    }
}

/// The Chat Completions URL from a base URL (`…/v1` → `…/v1/chat/completions`); a full URL
/// and any query (Azure's `api-version`) are kept.
pub fn chat_url(base: &str) -> String {
    let (path, query) = split_query(base);
    if path.ends_with("/chat/completions") { format!("{path}{query}") } else { format!("{path}/chat/completions{query}") }
}

/// The model list URL of an OpenAI-compatible base URL.
pub fn models_url(base: &str) -> String {
    let (path, query) = split_query(base);
    format!("{}/models{query}", path.trim_end_matches("/chat/completions"))
}

/// Ollama's server URL without an API path (`…:11434/v1` → `…:11434`).
pub fn ollama_host(url: &str) -> String {
    let (path, _) = split_query(url);
    let mut p = path.to_string();
    for suffix in ["/chat/completions", "/v1", "/api/chat", "/api"] {
        if let Some(s) = p.strip_suffix(suffix) {
            p = s.to_string();
        }
    }
    p
}

/// Azure OpenAI takes the key in an `api-key` header.
fn is_azure(url: &str) -> bool {
    let h = host(url);
    h.ends_with(".azure.com") || h.ends_with(".azure-api.net")
}

fn openai_headers(url: &str, key: Option<&str>) -> Vec<(&'static str, String)> {
    let mut h = vec![("content-type", "application/json".to_string())];
    if let Some(k) = key {
        if is_azure(url) {
            h.push(("api-key", k.to_string()));
        } else {
            h.push(("authorization", format!("Bearer {k}")));
        }
    }
    h
}

/// The chat request: the system prompt, the user's text, and the reference image (PNG) when
/// there is one and the model accepts images.
pub fn build_request(t: &Target, system: &str, text: &str, image: Option<&[u8]>) -> HttpReq {
    let img = image.filter(|_| t.vision).map(crate::ai::base64);
    let key = t.key.as_deref().filter(|k| !k.trim().is_empty());
    match t.provider {
        Provider::Anthropic => {
            let content = match &img {
                None => json!(text),
                Some(b) => json!([
                    { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": b } },
                    { "type": "text", "text": text },
                ]),
            };
            HttpReq {
                url: t.url.clone(),
                headers: vec![
                    ("content-type", "application/json".into()),
                    ("x-api-key", key.unwrap_or("").to_string()),
                    ("anthropic-version", "2026-10-21".into()),
                    ("anthropic-beta", "server-side-fallback-2026-07-01".into()),
                ],
                body: json!({
                    "model": t.model,
                    "max_tokens": 16000,
                    "output_config": { "effort": "medium" },
                    "fallbacks": "default",
                    "system": system,
                    "messages": [{ "role": "user", "content": content }],
                }),
            }
        }
        Provider::Ollama if t.ollama_native => {
            let mut user = json!({ "role": "user", "content": text });
            if let Some(b) = &img {
                user["images"] = json!([b]);
            }
            HttpReq {
                url: format!("{}/api/chat", ollama_host(&t.url)),
                headers: vec![("content-type", "application/json".into())],
                body: json!({ "model": t.model, "stream": false, "messages": [{ "role": "system", "content": system }, user] }),
            }
        }
        _ => {
            let url = if t.provider == Provider::Ollama { format!("{}/v1/chat/completions", ollama_host(&t.url)) } else { chat_url(&t.url) };
            let content = match &img {
                None => json!(text),
                Some(b) => json!([
                    { "type": "text", "text": text },
                    { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{b}") } },
                ]),
            };
            let key = if t.provider.is_local() { None } else { key };
            HttpReq {
                headers: openai_headers(&url, key),
                url,
                body: json!({
                    "model": t.model,
                    "stream": false,
                    "messages": [{ "role": "system", "content": system }, { "role": "user", "content": content }],
                }),
            }
        }
    }
}

/// The request that lists the installed (or offered) models.
pub fn build_models_request(t: &Target) -> HttpReq {
    let key = t.key.as_deref().filter(|k| !k.trim().is_empty());
    match t.provider {
        Provider::Anthropic => {
            let (path, _) = split_query(&t.url);
            let url = format!("{}/models", path.trim_end_matches("/messages"));
            HttpReq { url, headers: vec![("x-api-key", key.unwrap_or("").to_string()), ("anthropic-version", "2026-10-21".into())], body: Value::Null }
        }
        Provider::Ollama => HttpReq { url: format!("{}/api/tags", ollama_host(&t.url)), headers: vec![], body: Value::Null },
        Provider::LmStudio => HttpReq { url: models_url(&t.url), headers: vec![], body: Value::Null },
        Provider::OpenAi => {
            let url = models_url(&t.url);
            let mut headers = openai_headers(&url, key);
            headers.retain(|h| h.0 != "content-type");
            HttpReq { url, headers, body: Value::Null }
        }
    }
}

/// The model names in a model list (`{"data": [{"id"}]}`, or Ollama's `{"models": [{"name"}]}`).
pub fn parse_models(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = v["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["id"].as_str())
        .chain(v["models"].as_array().into_iter().flatten().filter_map(|m| m["name"].as_str().or(m["model"].as_str())))
        .map(str::to_string)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The message of an error reply, in any of the usual shapes.
pub fn error_message(v: &Value) -> Option<String> {
    v["error"]["message"]
        .as_str()
        .or_else(|| v["error"].as_str())
        .or_else(|| v["message"].as_str())
        .or_else(|| v.as_str().filter(|s| !s.trim().is_empty()))
        .map(|s| s.trim().chars().take(600).collect())
}

/// The error says the model does not take images: worth trying again with text only.
pub fn rejects_images(status: u16, v: &Value) -> bool {
    if !(400..500).contains(&status) || status == 401 || status == 403 || status == 429 {
        return false;
    }
    let m = error_message(v).unwrap_or_default().to_ascii_lowercase();
    ["image", "vision", "multimodal", "multi-modal"].iter().any(|w| m.contains(w))
}

fn too_long() -> String {
    "The drawing was too complex and got cut off. Try a simpler prompt.".into()
}

/// The text of a chat reply.
pub fn parse_reply(t: &Target, status: u16, v: &Value) -> Result<String, String> {
    if status >= 400 {
        let msg = error_message(v).unwrap_or_else(|| "unknown error".into());
        let who = if t.provider == Provider::OpenAi { "AI service".to_string() } else { t.provider.name().to_string() };
        if status == 404 && t.provider.is_local() && msg.to_ascii_lowercase().contains("model") {
            return Err(format!("{who} does not have the model \"{}\": {msg}. Choose an installed model in the AI settings (Refresh models).", t.model));
        }
        return Err(format!("{who} error ({status}): {msg}"));
    }
    match t.provider {
        Provider::Anthropic => {
            match v["stop_reason"].as_str() {
                Some("refusal") => {
                    let why = v["stop_details"]["explanation"].as_str().unwrap_or("The request was declined.");
                    return Err(format!("The AI declined this prompt. {why}"));
                }
                Some("max_tokens") => return Err(too_long()),
                _ => {}
            }
            Ok(v["content"].as_array().map(|a| a.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("")).unwrap_or_default())
        }
        Provider::Ollama if t.ollama_native => {
            if let Some(e) = v["error"].as_str() {
                return Err(format!("Ollama error: {e}"));
            }
            if v["done_reason"] == "length" {
                return Err(too_long());
            }
            v["message"]["content"].as_str().map(str::to_string).ok_or_else(|| "Invalid response from Ollama.".into())
        }
        _ => {
            if !v["error"].is_null() {
                return Err(format!("AI service error: {}", error_message(v).unwrap_or_else(|| "unknown error".into())));
            }
            let choice = &v["choices"][0];
            if let Some(r) = choice["message"]["refusal"].as_str().filter(|r| !r.trim().is_empty()) {
                return Err(format!("The AI declined this prompt. {r}"));
            }
            match choice["finish_reason"].as_str() {
                Some("length") => return Err(too_long()),
                Some("content_filter") => return Err("The AI declined this prompt (content filter).".into()),
                _ => {}
            }
            let c = &choice["message"]["content"];
            if let Some(s) = c.as_str() {
                return Ok(s.to_string());
            }
            // content as an array of parts
            c.as_array()
                .map(|a| a.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""))
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "Invalid response from the AI service.".into())
        }
    }
}

/// Remove the reasoning some local models write before the answer (`<think>…</think>`).
pub fn strip_think(text: &str) -> String {
    let mut s = text.to_string();
    while let (Some(a), Some(b)) = (s.find("<think>"), s.find("</think>")) {
        if b < a {
            break;
        }
        s.replace_range(a..b + "</think>".len(), "");
    }
    // a reply that starts thinking without the opening tag
    if let Some(b) = s.find("</think>") {
        s = s[b + "</think>".len()..].to_string();
    }
    s
}

// ------------------------------------------------------------------------------------------
// HTTP
// ------------------------------------------------------------------------------------------

/// Why a request failed before it got an answer.
pub fn transport_error(t: &Target, url: &str, e: &ureq::Error) -> String {
    let local = t.provider.is_local() || is_local_url(url);
    let name = match t.provider {
        Provider::OpenAi if local => "the AI server",
        Provider::OpenAi => "the AI service",
        p => p.name(),
    };
    let hint = match t.provider {
        Provider::Ollama => " (start the Ollama app, or run `ollama serve`)",
        Provider::LmStudio => " and its local server is started (Developer ▸ Start Server)",
        _ => "",
    };
    match e {
        ureq::Error::Timeout(_) => format!(
            "No answer from {name} within {} seconds. Local models can be slow: raise the time limit in {}, or try a smaller model.",
            t.timeout.as_secs(),
            crate::ai::KEY_PLACE
        ),
        ureq::Error::Io(_) | ureq::Error::ConnectionFailed | ureq::Error::HostNotFound if local => {
            let host = url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or(url);
            format!("Could not connect to {name} at {host}. Make sure {name} is running{hint}, then try again.")
        }
        e => format!("Could not reach {name}: {e}"),
    }
}

/// Send `req` and read the JSON answer (a non-JSON answer comes back as a string).
pub fn send(req: &HttpReq, timeout: Duration) -> Result<(u16, Value), ureq::Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(timeout)).build().into();
    let mut resp = if req.body.is_null() {
        let mut r = agent.get(&req.url);
        for (k, v) in &req.headers {
            r = r.header(*k, v);
        }
        r.call()?
    } else {
        let mut r = agent.post(&req.url);
        for (k, v) in &req.headers {
            r = r.header(*k, v);
        }
        r.send_json(&req.body)?
    };
    let status = resp.status().as_u16();
    let s = resp.body_mut().with_config().limit(64 << 20).read_to_string()?;
    Ok((status, serde_json::from_str(&s).unwrap_or(Value::String(s))))
}

/// Ask the model and return the reply's text and a note for the user. With a reference image
/// and a model that takes images, the image is sent as a vision content part; if the model turns
/// it down, or does not take images, `plain` (the request with the artwork as SVG text) is sent.
pub fn chat(t: &Target, system: &str, with_image: &str, plain: &str, image: Option<&[u8]>) -> Result<(String, Option<String>), String> {
    let model = if t.model.is_empty() { "The model".to_string() } else { t.model.clone() };
    let mut note = None;
    let (text, image) = match image {
        Some(png) if t.vision => (with_image, Some(png)),
        Some(_) => {
            note = Some(format!("{model} is set as not taking images, so the reference was sent as SVG text."));
            (plain, None)
        }
        None => (with_image, None),
    };
    let req = build_request(t, system, text, image);
    let (mut status, mut v) = send(&req, t.timeout).map_err(|e| transport_error(t, &req.url, &e))?;
    if image.is_some() && rejects_images(status, &v) {
        note = Some(format!("{model} did not take the image, so the reference was sent as SVG text."));
        let req = build_request(t, system, plain, None);
        (status, v) = send(&req, t.timeout).map_err(|e| transport_error(t, &req.url, &e))?;
    }
    parse_reply(t, status, &v).map(|s| (s, note))
}

/// The models the server offers.
pub fn list_models(t: &Target) -> Result<Vec<String>, String> {
    let req = build_models_request(t);
    let (status, v) = send(&req, Duration::from_secs(20)).map_err(|e| transport_error(t, &req.url, &e))?;
    if status >= 400 {
        return Err(format!("Could not list the models ({status}): {}", error_message(&v).unwrap_or_default()));
    }
    let m = parse_models(&v);
    if m.is_empty() && t.provider.is_local() {
        return Err(format!("{} has no models installed yet.", t.provider.name()));
    }
    Ok(m)
}

// ------------------------------------------------------------------------------------------
// What the user is told
// ------------------------------------------------------------------------------------------

/// Where the AI features send the user's description and artwork, for the Generative AI terms
/// and the Options page.
pub fn disclosure(cfg: &AiConfig, anthropic_url: &str) -> String {
    let url = cfg.url(anthropic_url);
    let model = cfg.model();
    let model_s = if model.is_empty() { String::new() } else { format!(", model {model}") };
    let what = "your description, and any artwork you choose as a reference,";
    let mut s = match cfg.provider {
        Provider::Anthropic => format!("The AI features send {what} to a third-party AI model (Claude, by Anthropic) to generate the result."),
        p if is_local_url(&url) => {
            let who = if p == Provider::OpenAi { format!("the AI server at {}", host(&url)) } else { p.name().to_string() };
            format!("The AI features run on your computer with {who}{model_s}: {what} stay on this computer.")
        }
        Provider::OpenAi => format!("The AI features send {what} to a third-party AI service ({}{model_s}) to generate the result. That service's terms and policies apply.", host(&url)),
        p => format!("The AI features send {what} to the {} server at {}{model_s}.", p.name(), host(&url)),
    };
    if cfg.image_provider == ImageProvider::ComfyUi {
        let c = or(&cfg.comfy_url, COMFY_URL);
        if is_local_url(&c) {
            s.push_str(" Images are made with ComfyUI, which runs on your computer.");
        } else {
            s.push_str(&format!(" Images are made with the ComfyUI server at {}, which receives your description and source artwork.", host(&c)));
        }
    }
    s
}

/// The usage policy of the chosen provider, when there is a known one.
pub fn policy_url(cfg: &AiConfig, anthropic_url: &str) -> Option<&'static str> {
    match cfg.provider {
        Provider::Anthropic => Some("https://www.anthropic.com/legal/aup"),
        Provider::OpenAi if host(&cfg.url(anthropic_url)) == "api.openai.com" => Some("https://openai.com/policies/usage-policies/"),
        _ => None,
    }
}

/// The line under the AI Generate panel.
pub fn powered_by(cfg: &AiConfig, anthropic_url: &str) -> String {
    let url = cfg.url(anthropic_url);
    let model = cfg.model();
    let who = match cfg.provider {
        Provider::Anthropic => "Claude".to_string(),
        Provider::OpenAi => host(&url),
        p => p.name().to_string(),
    };
    let mut s = if model.is_empty() { format!("Powered by {who}") } else { format!("Powered by {who} ({model})") };
    if is_local_url(&url) {
        s.push_str(", on your computer");
    }
    if cfg.image_provider == ImageProvider::ComfyUi {
        s.push_str("; images by ComfyUI");
    }
    s.push('.');
    s
}

/// A tiny HTTP server for tests: answers each request with `handler` and records it.
#[cfg(test)]
pub mod test_server {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug)]
    pub struct Req {
        pub method: String,
        pub path: String,
        pub headers: Vec<(String, String)>,
        pub body: Vec<u8>,
    }

    impl Req {
        pub fn header(&self, name: &str) -> Option<&str> {
            self.headers.iter().find(|h| h.0.eq_ignore_ascii_case(name)).map(|h| h.1.as_str())
        }
    }

    pub struct Server {
        pub url: String,
        log: Arc<Mutex<Vec<Req>>>,
    }

    impl Server {
        pub fn requests(&self) -> Vec<Req> {
            self.log.lock().unwrap().clone()
        }
    }

    pub fn start(handler: impl Fn(&Req) -> (u16, Vec<u8>) + Send + Sync + 'static) -> Server {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        let log = Arc::new(Mutex::new(vec![]));
        let lg = log.clone();
        let handler = Arc::new(handler);
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(mut s) = s else { break };
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                if r.read_line(&mut line).is_err() {
                    continue;
                }
                let mut parts = line.split_whitespace();
                let method = parts.next().unwrap_or("").to_string();
                let path = parts.next().unwrap_or("").to_string();
                let mut headers = vec![];
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).is_err() || h.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':') {
                        headers.push((k.trim().to_string(), v.trim().to_string()));
                    }
                }
                let len = headers.iter().find(|h| h.0.eq_ignore_ascii_case("content-length")).and_then(|h| h.1.parse().ok()).unwrap_or(0usize);
                let mut body = vec![0; len];
                let _ = r.read_exact(&mut body);
                let req = Req { method, path, headers, body };
                let (status, out) = handler(&req);
                lg.lock().unwrap().push(req);
                let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", out.len());
                let _ = s.write_all(&out);
                let _ = s.flush();
            }
        });
        Server { url, log }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(p: Provider, url: &str) -> Target {
        Target { provider: p, url: url.into(), model: "m1".into(), key: Some("sk-test".into()), vision: true, ollama_native: false, timeout: Duration::from_secs(30) }
    }

    #[test]
    fn urls() {
        assert_eq!(chat_url("https://api.openai.com/v1"), "https://api.openai.com/v1/chat/completions");
        assert_eq!(chat_url("https://api.openai.com/v1/"), "https://api.openai.com/v1/chat/completions");
        assert_eq!(chat_url("https://openrouter.ai/api/v1/chat/completions"), "https://openrouter.ai/api/v1/chat/completions");
        assert_eq!(
            chat_url("https://x.openai.azure.com/openai/deployments/d1?api-version=2024-10-21"),
            "https://x.openai.azure.com/openai/deployments/d1/chat/completions?api-version=2024-10-21"
        );
        assert_eq!(models_url("https://api.groq.com/openai/v1/chat/completions"), "https://api.groq.com/openai/v1/models");
        assert_eq!(ollama_host("http://localhost:11434/v1/"), "http://localhost:11434");
        assert_eq!(ollama_host("http://localhost:11434/api"), "http://localhost:11434");
        assert_eq!(host("https://user@API.example.com:8443/v1?x=1"), "api.example.com");
        assert_eq!(host("http://[::1]:1234/v1"), "::1");
        assert!(is_local_url("http://127.0.0.1:8188"));
        assert!(is_local_url("http://localhost:1234/v1"));
        assert!(!is_local_url("https://api.together.xyz/v1"));
    }

    #[test]
    fn anthropic_request() {
        let t = target(Provider::Anthropic, ANTHROPIC_URL);
        let r = build_request(&t, "sys", "draw a fox", Some(&[1, 2, 3]));
        assert_eq!(r.url, ANTHROPIC_URL);
        assert_eq!(r.header("x-api-key"), Some("sk-test"));
        assert_eq!(r.header("anthropic-version"), Some("2026-10-21"));
        assert!(r.header("authorization").is_none());
        assert_eq!(r.body["model"], "m1");
        assert_eq!(r.body["system"], "sys");
        let c = &r.body["messages"][0]["content"];
        assert_eq!(c[0]["type"], "image");
        assert_eq!(c[0]["source"]["data"], "AQID");
        assert_eq!(c[1]["text"], "draw a fox");
        // text only
        let r = build_request(&t, "sys", "draw a fox", None);
        assert_eq!(r.body["messages"][0]["content"], "draw a fox");
    }

    #[test]
    fn openai_request() {
        let t = target(Provider::OpenAi, "https://api.openai.com/v1");
        let r = build_request(&t, "sys", "draw", Some(&[1, 2, 3]));
        assert_eq!(r.url, "https://api.openai.com/v1/chat/completions");
        assert_eq!(r.header("authorization"), Some("Bearer sk-test"));
        assert_eq!(r.body["stream"], false);
        assert_eq!(r.body["messages"][0], json!({ "role": "system", "content": "sys" }));
        let c = &r.body["messages"][1]["content"];
        assert_eq!(c[0], json!({ "type": "text", "text": "draw" }));
        assert_eq!(c[1]["image_url"]["url"], "data:image/png;base64,AQID");
        // a model without vision gets text only
        let mut tv = t.clone();
        tv.vision = false;
        assert_eq!(build_request(&tv, "sys", "draw", Some(&[1])).body["messages"][1]["content"], "draw");
        // Azure: api-key header, query kept
        let az = target(Provider::OpenAi, "https://x.openai.azure.com/openai/deployments/d1?api-version=2024-10-21");
        let r = build_request(&az, "sys", "draw", None);
        assert_eq!(r.header("api-key"), Some("sk-test"));
        assert!(r.header("authorization").is_none());
        assert!(r.url.ends_with("/chat/completions?api-version=2024-10-21"));
        // a local server without a key
        let mut l = target(Provider::OpenAi, "http://localhost:8080/v1");
        l.key = None;
        assert!(build_request(&l, "s", "d", None).header("authorization").is_none());
    }

    #[test]
    fn local_requests() {
        let mut t = target(Provider::Ollama, OLLAMA_URL);
        let r = build_request(&t, "sys", "draw", Some(&[1, 2, 3]));
        assert_eq!(r.url, "http://localhost:11434/v1/chat/completions");
        assert!(r.header("authorization").is_none(), "no key for local servers");
        assert_eq!(r.body["messages"][1]["content"][1]["type"], "image_url");
        t.ollama_native = true;
        let r = build_request(&t, "sys", "draw", Some(&[1, 2, 3]));
        assert_eq!(r.url, "http://localhost:11434/api/chat");
        assert_eq!(r.body["stream"], false);
        assert_eq!(r.body["messages"][1], json!({ "role": "user", "content": "draw", "images": ["AQID"] }));
        let lm = target(Provider::LmStudio, LMSTUDIO_URL);
        let r = build_request(&lm, "sys", "draw", None);
        assert_eq!(r.url, "http://localhost:1234/v1/chat/completions");
        assert!(r.header("authorization").is_none());
        assert_eq!(build_models_request(&lm).url, "http://localhost:1234/v1/models");
        assert_eq!(build_models_request(&t).url, "http://localhost:11434/api/tags");
        assert_eq!(build_models_request(&target(Provider::Anthropic, ANTHROPIC_URL)).url, "https://api.anthropic.com/v1/models");
        let o = build_models_request(&target(Provider::OpenAi, "https://openrouter.ai/api/v1"));
        assert_eq!(o.url, "https://openrouter.ai/api/v1/models");
        assert_eq!(o.header("authorization"), Some("Bearer sk-test"));
        assert!(o.body.is_null());
    }

    #[test]
    fn replies() {
        let a = target(Provider::Anthropic, ANTHROPIC_URL);
        let v = json!({"content": [{"type": "thinking", "thinking": "x"}, {"type": "text", "text": "<svg/>"}], "stop_reason": "end_turn"});
        assert_eq!(parse_reply(&a, 200, &v).unwrap(), "<svg/>");
        assert!(parse_reply(&a, 200, &json!({"stop_reason": "max_tokens", "content": []})).unwrap_err().contains("too complex"));
        assert!(parse_reply(&a, 200, &json!({"stop_reason": "refusal", "content": []})).unwrap_err().contains("declined"));
        assert_eq!(parse_reply(&a, 401, &json!({"type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}})).unwrap_err(), "Anthropic error (401): invalid x-api-key");

        let o = target(Provider::OpenAi, OPENAI_URL);
        let v = json!({"choices": [{"index": 0, "message": {"role": "assistant", "content": "<svg></svg>"}, "finish_reason": "stop"}]});
        assert_eq!(parse_reply(&o, 200, &v).unwrap(), "<svg></svg>");
        let parts = json!({"choices": [{"message": {"content": [{"type": "text", "text": "<svg>"}, {"type": "text", "text": "</svg>"}]}, "finish_reason": "stop"}]});
        assert_eq!(parse_reply(&o, 200, &parts).unwrap(), "<svg></svg>");
        assert!(parse_reply(&o, 200, &json!({"choices": [{"message": {"content": "<svg"}, "finish_reason": "length"}]})).unwrap_err().contains("cut off"));
        assert!(parse_reply(&o, 200, &json!({"choices": [{"message": {"content": null, "refusal": "No."}, "finish_reason": "stop"}]})).unwrap_err().contains("declined"));
        assert_eq!(parse_reply(&o, 429, &json!({"error": {"message": "Rate limit reached", "type": "requests"}})).unwrap_err(), "AI service error (429): Rate limit reached");
        // OpenRouter reports some errors inside a 200
        assert!(parse_reply(&o, 200, &json!({"error": {"message": "Provider returned error", "code": 502}})).unwrap_err().contains("Provider returned error"));

        let mut ol = target(Provider::Ollama, OLLAMA_URL);
        ol.ollama_native = true;
        let v = json!({"model": "llama3.2", "message": {"role": "assistant", "content": "<svg/>"}, "done": true, "done_reason": "stop"});
        assert_eq!(parse_reply(&ol, 200, &v).unwrap(), "<svg/>");
        assert!(parse_reply(&ol, 200, &json!({"message": {"content": "<svg"}, "done": true, "done_reason": "length"})).unwrap_err().contains("cut off"));
        let e = parse_reply(&ol, 404, &json!({"error": "model 'm1' not found"})).unwrap_err();
        assert!(e.contains("does not have the model \"m1\""), "{e}");
        // Ollama's OpenAI-compatible API answers like OpenAI
        ol.ollama_native = false;
        assert_eq!(parse_reply(&ol, 200, &json!({"choices": [{"message": {"content": "<svg/>"}, "finish_reason": "stop"}]})).unwrap(), "<svg/>");
    }

    #[test]
    fn image_rejections_and_errors() {
        assert!(rejects_images(400, &json!({"error": {"message": "This model does not support image input"}})));
        assert!(rejects_images(400, &json!({"error": "llama3.2 does not support images"})));
        assert!(rejects_images(422, &json!({"detail": "x", "message": "Vision is not enabled for this model"})));
        assert!(!rejects_images(401, &json!({"error": {"message": "bad key for image models"}})));
        assert!(!rejects_images(400, &json!({"error": {"message": "max_tokens too large"}})));
        assert_eq!(error_message(&json!("404 page not found")).unwrap(), "404 page not found");
    }

    #[test]
    fn models_lists() {
        assert_eq!(parse_models(&json!({"object": "list", "data": [{"id": "b"}, {"id": "a"}]})), vec!["a", "b"]);
        assert_eq!(parse_models(&json!({"models": [{"name": "llava:13b", "model": "llava:13b"}, {"name": "llama3.2:latest"}]})), vec!["llama3.2:latest", "llava:13b"]);
        assert!(parse_models(&json!({})).is_empty());
    }

    #[test]
    fn think_blocks() {
        assert_eq!(strip_think("<think>maybe <svg> here</think>\n<svg></svg>"), "\n<svg></svg>");
        assert_eq!(strip_think("reasoning</think><svg/>"), "<svg/>");
        assert_eq!(strip_think("<svg/>"), "<svg/>");
    }

    #[test]
    fn settings_round_trip() {
        // a settings file from before the providers: everything new takes its default
        let old = r#"{"ai_provider_url": "", "ai_api_key_env": "MY_KEY", "ai_terms_accepted": true}"#;
        let s: crate::settings::Settings = serde_json::from_str(old).unwrap();
        assert_eq!(s.ai_api_key_env, "MY_KEY");
        assert!(s.ai_terms_accepted);
        assert_eq!(s.ai_config, AiConfig::default());
        assert_eq!(s.ai_config.provider, Provider::Anthropic);
        // a partial block, and a provider this version does not know
        let part = r#"{"ai_config": {"provider": "Gemini", "ollama_model": "llava", "image_provider": "Midjourney"}}"#;
        let s: crate::settings::Settings = serde_json::from_str(part).unwrap();
        assert_eq!(s.ai_config.provider, Provider::Anthropic);
        assert_eq!(s.ai_config.image_provider, ImageProvider::Llm);
        assert_eq!(s.ai_config.ollama_model, "llava");
        assert_eq!(s.ai_config.ollama_url, OLLAMA_URL);
        // a full round trip keeps everything, and never writes a key
        let mut s = crate::settings::Settings::default();
        s.ai_config.provider = Provider::LmStudio;
        s.ai_config.lmstudio_model = "qwen2.5-vl".into();
        s.ai_config.image_provider = ImageProvider::ComfyUi;
        s.ai_config.comfy_workflow = r"C:\wf\api.json".into();
        let text = serde_json::to_string(&s).unwrap();
        assert!(text.contains("\"provider\":\"LmStudio\""));
        let back: crate::settings::Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.ai_config, s.ai_config);
        assert!(!text.contains("sk-"));
    }

    #[test]
    fn disclosures() {
        let mut c = AiConfig::default();
        assert!(disclosure(&c, "").contains("Claude, by Anthropic"));
        assert_eq!(policy_url(&c, ""), Some("https://www.anthropic.com/legal/aup"));
        assert_eq!(powered_by(&c, ""), format!("Powered by Claude ({ANTHROPIC_MODEL})."));
        c.provider = Provider::OpenAi;
        c.openai_url = "https://openrouter.ai/api/v1".into();
        c.openai_model = "meta/llama".into();
        let d = disclosure(&c, "");
        assert!(d.contains("third-party AI service (openrouter.ai, model meta/llama)"), "{d}");
        assert!(!d.contains("Anthropic"));
        assert_eq!(policy_url(&c, ""), None);
        c.provider = Provider::Ollama;
        c.ollama_model = "llava".into();
        let d = disclosure(&c, "");
        assert!(d.contains("run on your computer with Ollama, model llava"), "{d}");
        assert!(!d.contains("third-party"));
        assert_eq!(powered_by(&c, ""), "Powered by Ollama (llava), on your computer.");
        c.provider = Provider::LmStudio;
        c.lmstudio_url = "http://192.168.1.20:1234/v1".into();
        assert!(disclosure(&c, "").contains("LM Studio server at 192.168.1.20"));
        c.image_provider = ImageProvider::ComfyUi;
        assert!(disclosure(&c, "").contains("ComfyUI, which runs on your computer"));
    }

    /// Requests to a stand-in server: vision first, then text only when the model turns the
    /// image down.
    #[test]
    fn chat_falls_back_to_text() {
        let server = test_server::start(|req| {
            let v: Value = serde_json::from_slice(&req.body).unwrap();
            if v["messages"][1]["content"].is_array() {
                (400, json!({"error": {"message": "image input is not supported by this model"}}).to_string().into_bytes())
            } else {
                (200, json!({"choices": [{"message": {"content": "<svg>ok</svg>"}, "finish_reason": "stop"}]}).to_string().into_bytes())
            }
        });
        let mut t = target(Provider::LmStudio, &format!("{}/v1", server.url));
        t.key = None;
        let (text, note) = chat(&t, "sys", "with image", "as svg text", Some(&[9, 9])).unwrap();
        assert_eq!(text, "<svg>ok</svg>");
        assert!(note.unwrap().contains("did not take the image"));
        let reqs = server.requests();
        assert_eq!(reqs.len(), 2);
        assert_eq!(reqs[0].path, "/v1/chat/completions");
        assert!(reqs[0].header("authorization").is_none());
        let second: Value = serde_json::from_slice(&reqs[1].body).unwrap();
        assert_eq!(second["messages"][1]["content"], "as svg text");
    }

    #[test]
    fn local_server_not_running() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        drop(l);
        let mut t = target(Provider::Ollama, &url);
        t.key = None;
        let e = chat(&t, "s", "d", "d", None).unwrap_err();
        assert!(e.starts_with("Could not connect to Ollama at 127.0.0.1:"), "{e}");
        assert!(e.contains("ollama serve"));
        let e = list_models(&t).unwrap_err();
        assert!(e.contains("Make sure Ollama is running"), "{e}");
    }
}
