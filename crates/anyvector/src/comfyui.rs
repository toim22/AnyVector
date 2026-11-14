//! ComfyUI image generation for AI Generate's Create image and Remix image modes.
//!
//! A workflow in ComfyUI's API format is filled in (placeholders such as `%prompt%`), queued with
//! `POST /prompt`, followed with `GET /history/{id}` until it finishes, and its first saved image
//! is fetched with `GET /view`. A source image (Remix, or Create Image Guided by Selection) is
//! uploaded first with `POST /upload/image`. The built-in workflows are the standard
//! text-to-image and image-to-image graphs; users can load their own API-format workflow.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The built-in text-to-image workflow (ComfyUI's default graph, in API format).
pub const TEXT_TO_IMAGE: &str = r#"{
  "3": { "class_type": "KSampler", "inputs": { "seed": "%seed%", "steps": "%steps%", "cfg": 7, "sampler_name": "euler", "scheduler": "normal", "denoise": 1, "model": ["4", 0], "positive": ["6", 0], "negative": ["7", 0], "latent_image": ["5", 0] } },
  "4": { "class_type": "CheckpointLoaderSimple", "inputs": { "ckpt_name": "%checkpoint%" } },
  "5": { "class_type": "EmptyLatentImage", "inputs": { "width": "%width%", "height": "%height%", "batch_size": 1 } },
  "6": { "class_type": "CLIPTextEncode", "inputs": { "text": "%prompt%", "clip": ["4", 1] } },
  "7": { "class_type": "CLIPTextEncode", "inputs": { "text": "%negative%", "clip": ["4", 1] } },
  "8": { "class_type": "VAEDecode", "inputs": { "samples": ["3", 0], "vae": ["4", 2] } },
  "9": { "class_type": "SaveImage", "inputs": { "filename_prefix": "AnyVector", "images": ["8", 0] } }
}"#;

/// The built-in image-to-image workflow, used when a source image is given.
pub const IMAGE_TO_IMAGE: &str = r#"{
  "3": { "class_type": "KSampler", "inputs": { "seed": "%seed%", "steps": "%steps%", "cfg": 7, "sampler_name": "euler", "scheduler": "normal", "denoise": "%denoise%", "model": ["4", 0], "positive": ["6", 0], "negative": ["7", 0], "latent_image": ["11", 0] } },
  "4": { "class_type": "CheckpointLoaderSimple", "inputs": { "ckpt_name": "%checkpoint%" } },
  "6": { "class_type": "CLIPTextEncode", "inputs": { "text": "%prompt%", "clip": ["4", 1] } },
  "7": { "class_type": "CLIPTextEncode", "inputs": { "text": "%negative%", "clip": ["4", 1] } },
  "8": { "class_type": "VAEDecode", "inputs": { "samples": ["3", 0], "vae": ["4", 2] } },
  "9": { "class_type": "SaveImage", "inputs": { "filename_prefix": "AnyVector", "images": ["8", 0] } },
  "10": { "class_type": "LoadImage", "inputs": { "image": "%image%" } },
  "11": { "class_type": "VAEEncode", "inputs": { "pixels": ["10", 0], "vae": ["4", 2] } }
}"#;

/// The placeholders a workflow can use, for the Options page.
pub const PLACEHOLDERS: &str = "%prompt%, %negative%, %seed%, %steps%, %width%, %height%, %checkpoint%, %image%, %denoise%";

/// What fills a workflow's placeholders.
#[derive(Clone, Debug, Default)]
pub struct Params {
    pub prompt: String,
    pub negative: String,
    pub seed: u64,
    pub steps: u32,
    pub width: u32,
    pub height: u32,
    pub checkpoint: String,
    /// The uploaded source image's name on the server.
    pub image: Option<String>,
    pub denoise: f64,
}

/// Parse a workflow file: it must be in ComfyUI's API format (node id → `class_type` and
/// `inputs`), not the editor format with `nodes` and `links`.
pub fn parse_workflow(text: &str) -> Result<Value, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("The workflow is not valid JSON: {e}"))?;
    let Some(map) = v.as_object() else { return Err("The workflow must be a JSON object.".into()) };
    if map.contains_key("nodes") && map.contains_key("links") {
        return Err("This workflow is in ComfyUI's editor format. In ComfyUI, choose Workflow ▸ Export (API) and load that file.".into());
    }
    if map.is_empty() || !map.values().all(|n| n.get("class_type").is_some()) {
        return Err("This is not a ComfyUI workflow in API format (every node needs a class_type).".into());
    }
    Ok(v)
}

fn contains(v: &Value, needle: &str) -> bool {
    match v {
        Value::String(s) => s.contains(needle),
        Value::Array(a) => a.iter().any(|x| contains(x, needle)),
        Value::Object(m) => m.values().any(|x| contains(x, needle)),
        _ => false,
    }
}

/// The workflow takes a source image (`%image%`).
pub fn wants_image(workflow: &Value) -> bool {
    contains(workflow, "%image%")
}

/// The workflow names its checkpoint with `%checkpoint%`.
pub fn wants_checkpoint(workflow: &Value) -> bool {
    contains(workflow, "%checkpoint%")
}

/// Fill the placeholders. A string that is exactly a numeric placeholder (`%seed%`, `%steps%`,
/// `%width%`, `%height%`, `%denoise%`) becomes a number; text placeholders are replaced inside
/// longer strings too. The workflow must contain `%prompt%`.
pub fn substitute(workflow: &Value, p: &Params) -> Result<Value, String> {
    if !contains(workflow, "%prompt%") {
        return Err("The workflow has no %prompt% placeholder. Type %prompt% into the text of its positive prompt node, export it again with Export (API), and reload it.".into());
    }
    if wants_image(workflow) && p.image.is_none() {
        return Err("This workflow needs a source image (%image%). Select artwork and choose Object ▸ AI Generate ▸ Remix Selection.".into());
    }
    fn walk(v: &Value, p: &Params) -> Value {
        match v {
            Value::String(s) => match s.as_str() {
                "%seed%" => json!(p.seed),
                "%steps%" => json!(p.steps),
                "%width%" => json!(p.width),
                "%height%" => json!(p.height),
                "%denoise%" => json!(p.denoise),
                _ => Value::String(
                    s.replace("%prompt%", &p.prompt)
                        .replace("%negative%", &p.negative)
                        .replace("%checkpoint%", &p.checkpoint)
                        .replace("%image%", p.image.as_deref().unwrap_or(""))
                        .replace("%seed%", &p.seed.to_string()),
                ),
            },
            Value::Array(a) => Value::Array(a.iter().map(|x| walk(x, p)).collect()),
            Value::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), walk(x, p))).collect()),
            other => other.clone(),
        }
    }
    Ok(walk(workflow, p))
}

/// The body of `POST /prompt`.
pub fn prompt_body(workflow: Value) -> Value {
    json!({ "prompt": workflow, "client_id": "anyvector" })
}

/// The prompt id from the answer to `POST /prompt`, or the validation errors.
pub fn parse_prompt_response(status: u16, v: &Value) -> Result<String, String> {
    if status < 400 {
        if let Some(id) = v["prompt_id"].as_str() {
            return Ok(id.to_string());
        }
    }
    let mut msg = v["error"]["message"].as_str().or_else(|| v["error"].as_str()).unwrap_or("the workflow was rejected").to_string();
    if let Some(nodes) = v["node_errors"].as_object() {
        for n in nodes.values() {
            let class = n["class_type"].as_str().unwrap_or("node");
            for e in n["errors"].as_array().into_iter().flatten() {
                let m = e["message"].as_str().unwrap_or("");
                let d = e["details"].as_str().unwrap_or("");
                msg.push_str(&format!("\n{class}: {m}{}{d}", if d.is_empty() { "" } else { ": " }));
            }
        }
    }
    Err(format!("ComfyUI could not run the workflow ({status}): {msg}"))
}

/// An image a workflow saved.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageRef {
    pub filename: String,
    pub subfolder: String,
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum History {
    /// Queued or running.
    Pending,
    Failed(String),
    Done(Vec<ImageRef>),
}

/// Read the answer to `GET /history/{id}`: `{}` until the prompt finishes, then its outputs and
/// status. Saved images (type `output`) come before previews.
pub fn parse_history(v: &Value, id: &str) -> History {
    let Some(h) = v.get(id) else { return History::Pending };
    let status = &h["status"];
    if status["status_str"] == "error" {
        let mut why = "the workflow failed".to_string();
        for m in status["messages"].as_array().into_iter().flatten() {
            if m[0] == "execution_error" {
                let node = m[1]["node_type"].as_str().unwrap_or("node");
                let e = m[1]["exception_message"].as_str().unwrap_or("").trim();
                why = format!("{node}: {e}");
            }
        }
        return History::Failed(format!("ComfyUI could not finish the image. {why}"));
    }
    let mut images: Vec<ImageRef> = vec![];
    for out in h["outputs"].as_object().into_iter().flat_map(|m| m.values()) {
        for i in out["images"].as_array().into_iter().flatten() {
            if let Some(f) = i["filename"].as_str() {
                images.push(ImageRef { filename: f.to_string(), subfolder: i["subfolder"].as_str().unwrap_or("").to_string(), kind: i["type"].as_str().unwrap_or("output").to_string() });
            }
        }
    }
    images.sort_by_key(|i| i.kind != "output");
    if !images.is_empty() {
        History::Done(images)
    } else if status["completed"] == true || status["status_str"] == "success" {
        History::Failed("The workflow finished without an image. Make sure it ends with a Save Image node.".into())
    } else {
        History::Pending
    }
}

/// Percent-encode a query value.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The server URL without a trailing slash.
pub fn base(url: &str) -> String {
    let u = url.trim().trim_end_matches('/');
    if u.is_empty() { crate::ai_provider::COMFY_URL.to_string() } else { u.to_string() }
}

/// `GET /view` for a saved image.
pub fn view_url(base: &str, i: &ImageRef) -> String {
    format!("{base}/view?filename={}&subfolder={}&type={}", encode(&i.filename), encode(&i.subfolder), encode(&i.kind))
}

/// The multipart body of `POST /upload/image`.
pub fn upload_body(boundary: &str, filename: &str, png: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(png.len() + 400);
    b.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"{filename}\"\r\nContent-Type: image/png\r\n\r\n").as_bytes());
    b.extend_from_slice(png);
    b.extend_from_slice(format!("\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"overwrite\"\r\n\r\ntrue\r\n--{boundary}--\r\n").as_bytes());
    b
}

/// The name to give LoadImage, from the answer to `POST /upload/image`.
pub fn parse_upload(v: &Value) -> Option<String> {
    let name = v["name"].as_str()?;
    let sub = v["subfolder"].as_str().unwrap_or("");
    Some(if sub.is_empty() { name.to_string() } else { format!("{sub}/{name}") })
}

/// The checkpoint files listed by `GET /object_info/CheckpointLoaderSimple` (the older
/// `[[names], {..}]` form and the newer `["COMBO", {"options": [names]}]` form).
pub fn parse_checkpoints(v: &Value) -> Vec<String> {
    let spec = &v["CheckpointLoaderSimple"]["input"]["required"]["ckpt_name"];
    let list = if spec[0].is_array() { &spec[0] } else { &spec[1]["options"] };
    list.as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect()
}

/// A seed for a new image.
pub fn random_seed() -> u64 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    (t.as_nanos() as u64).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 14
}

// ------------------------------------------------------------------------------------------
// HTTP
// ------------------------------------------------------------------------------------------

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(timeout)).build().into()
}

fn reach_error(base: &str, e: &ureq::Error) -> String {
    match e {
        ureq::Error::Io(_) | ureq::Error::ConnectionFailed | ureq::Error::HostNotFound => {
            format!("Could not connect to ComfyUI at {base}. Make sure ComfyUI is running (and its server is on this address), then try again.")
        }
        ureq::Error::Timeout(_) => format!("ComfyUI at {base} did not answer in time."),
        e => format!("Could not reach ComfyUI at {base}: {e}"),
    }
}

fn read_json(resp: &mut ureq::http::Response<ureq::Body>) -> Value {
    let s = resp.body_mut().with_config().limit(32 << 20).read_to_string().unwrap_or_default();
    serde_json::from_str(&s).unwrap_or(Value::String(s))
}

fn get_json(a: &ureq::Agent, url: &str, base: &str) -> Result<(u16, Value), String> {
    let mut r = a.get(url).call().map_err(|e| reach_error(base, &e))?;
    Ok((r.status().as_u16(), read_json(&mut r)))
}

/// The checkpoint models installed in ComfyUI.
pub fn list_checkpoints(url: &str) -> Result<Vec<String>, String> {
    let base = base(url);
    let (status, v) = get_json(&agent(Duration::from_secs(20)), &format!("{base}/object_info/CheckpointLoaderSimple"), &base)?;
    if status >= 400 {
        return Err(format!("ComfyUI answered {status} when asked for its models."));
    }
    Ok(parse_checkpoints(&v))
}

/// Run `workflow` (placeholders still in it) on the ComfyUI server at `url` and return the
/// bytes of the first image it saves. `source` is the PNG for `%image%`. Stops when `cancel` is
/// set, or after `timeout`.
pub fn generate(url: &str, workflow: &Value, mut params: Params, source: Option<&[u8]>, cancel: &AtomicBool, timeout: Duration) -> Result<Vec<u8>, String> {
    let base = base(url);
    let a = agent(Duration::from_secs(60));
    let deadline = Instant::now() + timeout;
    if wants_image(workflow) {
        if let Some(png) = source {
            let boundary = format!("anyvector{}", random_seed());
            let body = upload_body(&boundary, "anyvector-source.png", png);
            let mut r = a
                .post(format!("{base}/upload/image"))
                .header("content-type", format!("multipart/form-data; boundary={boundary}"))
                .send(&body[..])
                .map_err(|e| reach_error(&base, &e))?;
            let status = r.status().as_u16();
            let v = read_json(&mut r);
            params.image = parse_upload(&v).filter(|_| status < 400);
            if params.image.is_none() {
                return Err(format!("ComfyUI did not accept the source image ({status})."));
            }
        }
    }
    if wants_checkpoint(workflow) && params.checkpoint.trim().is_empty() {
        params.checkpoint = list_checkpoints(&base)?.into_iter().next().ok_or("ComfyUI has no checkpoint models. Put a model in ComfyUI's models\\checkpoints folder, or choose one in the AI settings.")?;
    }
    let wf = substitute(workflow, &params)?;
    let mut r = a.post(format!("{base}/prompt")).send_json(prompt_body(wf)).map_err(|e| reach_error(&base, &e))?;
    let status = r.status().as_u16();
    let id = parse_prompt_response(status, &read_json(&mut r))?;
    let stop = |a: &ureq::Agent| {
        let _ = a.post(format!("{base}/queue")).send_json(json!({ "delete": [id] }));
        let _ = a.post(format!("{base}/interrupt")).send_json(json!({ "prompt_id": id }));
    };
    loop {
        for _ in 0..5 {
            if cancel.load(Ordering::Relaxed) {
                stop(&a);
                return Err("Cancelled.".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if Instant::now() > deadline {
            stop(&a);
            return Err(format!("ComfyUI did not finish within {} seconds. Raise the time limit in the AI settings.", timeout.as_secs()));
        }
        let (_, v) = get_json(&a, &format!("{base}/history/{id}"), &base)?;
        match parse_history(&v, &id) {
            History::Pending => continue,
            History::Failed(e) => return Err(e),
            History::Done(images) => {
                let mut r = a.get(view_url(&base, &images[0])).call().map_err(|e| reach_error(&base, &e))?;
                if r.status().as_u16() >= 400 {
                    return Err(format!("ComfyUI could not send the image ({}).", r.status().as_u16()));
                }
                return r.body_mut().with_config().limit(256 << 20).read_to_vec().map_err(|e| format!("Could not download the image from ComfyUI: {e}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_provider::test_server;

    fn params() -> Params {
        Params { prompt: "a red fox".into(), negative: "blurry".into(), seed: 42, steps: 20, width: 768, height: 512, checkpoint: "sdxl.safetensors".into(), image: None, denoise: 0.6 }
    }

    #[test]
    fn builtin_workflows_parse_and_fill() {
        let wf = parse_workflow(TEXT_TO_IMAGE).unwrap();
        assert!(!wants_image(&wf));
        assert!(wants_checkpoint(&wf));
        let out = substitute(&wf, &params()).unwrap();
        assert_eq!(out["6"]["inputs"]["text"], "a red fox");
        assert_eq!(out["7"]["inputs"]["text"], "blurry");
        assert_eq!(out["3"]["inputs"]["seed"], 42);
        assert_eq!(out["3"]["inputs"]["steps"], 20);
        assert_eq!(out["5"]["inputs"]["width"], 768);
        assert_eq!(out["5"]["inputs"]["height"], 512);
        assert_eq!(out["4"]["inputs"]["ckpt_name"], "sdxl.safetensors");
        // links between nodes stay as they were
        assert_eq!(out["3"]["inputs"]["model"], json!(["4", 0]));
        assert!(!serde_json::to_string(&out).unwrap().contains('%'));

        let i2i = parse_workflow(IMAGE_TO_IMAGE).unwrap();
        assert!(wants_image(&i2i));
        assert!(substitute(&i2i, &params()).is_err(), "needs a source image");
        let mut p = params();
        p.image = Some("anyvector-source.png".into());
        let out = substitute(&i2i, &p).unwrap();
        assert_eq!(out["10"]["inputs"]["image"], "anyvector-source.png");
        assert_eq!(out["3"]["inputs"]["denoise"], 0.6);
    }

    #[test]
    fn user_workflow_placeholders() {
        let wf = parse_workflow(r#"{"1": {"class_type": "CLIPTextEncode", "inputs": {"text": "photo of %prompt%, seed %seed%, \"quoted\""}}, "2": {"class_type": "SaveImage", "inputs": {"seed": "%seed%"}}}"#).unwrap();
        let mut p = params();
        p.prompt = "a \"quoted\" fox\nnew line".into();
        let out = substitute(&wf, &p).unwrap();
        assert_eq!(out["1"]["inputs"]["text"], "photo of a \"quoted\" fox\nnew line, seed 42, \"quoted\"");
        assert_eq!(out["2"]["inputs"]["seed"], 42);
        // no prompt placeholder
        let wf = parse_workflow(r#"{"1": {"class_type": "CLIPTextEncode", "inputs": {"text": "a cat"}}}"#).unwrap();
        assert!(substitute(&wf, &p).unwrap_err().contains("%prompt%"));
    }

    #[test]
    fn rejects_editor_format_and_junk() {
        assert!(parse_workflow(r#"{"nodes": [], "links": [], "version": 0.4}"#).unwrap_err().contains("Export (API)"));
        assert!(parse_workflow("[1, 2]").is_err());
        assert!(parse_workflow("{\"1\": {\"inputs\": {}}}").is_err());
        assert!(parse_workflow("not json").is_err());
    }

    #[test]
    fn prompt_response() {
        assert_eq!(parse_prompt_response(200, &json!({"prompt_id": "abc", "number": 3, "node_errors": {}})).unwrap(), "abc");
        let e = parse_prompt_response(
            400,
            &json!({"error": {"type": "prompt_outputs_failed_validation", "message": "Prompt outputs failed validation", "details": ""},
                    "node_errors": {"4": {"errors": [{"message": "Value not in list", "details": "ckpt_name: 'x' not in []"}], "class_type": "CheckpointLoaderSimple"}}}),
        )
        .unwrap_err();
        assert!(e.contains("Prompt outputs failed validation"));
        assert!(e.contains("CheckpointLoaderSimple: Value not in list: ckpt_name"));
    }

    #[test]
    fn history_states() {
        assert_eq!(parse_history(&json!({}), "p1"), History::Pending);
        let running = json!({"p1": {"outputs": {}, "status": {"status_str": "running", "completed": false, "messages": []}}});
        assert_eq!(parse_history(&running, "p1"), History::Pending);
        let done = json!({"p1": {
            "outputs": {
                "12": {"images": [{"filename": "preview_0001.png", "subfolder": "", "type": "temp"}]},
                "9": {"images": [{"filename": "AnyVector_00001_.png", "subfolder": "sub dir", "type": "output"}]}
            },
            "status": {"status_str": "success", "completed": true, "messages": []}
        }});
        let History::Done(imgs) = parse_history(&done, "p1") else { panic!() };
        assert_eq!(imgs[0], ImageRef { filename: "AnyVector_00001_.png".into(), subfolder: "sub dir".into(), kind: "output".into() });
        assert_eq!(view_url("http://127.0.0.1:8188", &imgs[0]), "http://127.0.0.1:8188/view?filename=AnyVector_00001_.png&subfolder=sub%20dir&type=output");
        let failed = json!({"p1": {"outputs": {}, "status": {"status_str": "error", "completed": false,
            "messages": [["execution_start", {}], ["execution_error", {"node_type": "KSampler", "exception_message": "CUDA out of memory"}]]}}});
        assert_eq!(parse_history(&failed, "p1"), History::Failed("ComfyUI could not finish the image. KSampler: CUDA out of memory".into()));
        let empty = json!({"p1": {"outputs": {}, "status": {"status_str": "success", "completed": true}}});
        assert!(matches!(parse_history(&empty, "p1"), History::Failed(_)));
    }

    #[test]
    fn checkpoints_both_forms() {
        let old = json!({"CheckpointLoaderSimple": {"input": {"required": {"ckpt_name": [["a.safetensors", "b.ckpt"], {}]}}}});
        assert_eq!(parse_checkpoints(&old), vec!["a.safetensors", "b.ckpt"]);
        let new = json!({"CheckpointLoaderSimple": {"input": {"required": {"ckpt_name": ["COMBO", {"options": ["c.safetensors"]}]}}}});
        assert_eq!(parse_checkpoints(&new), vec!["c.safetensors"]);
        assert!(parse_checkpoints(&json!({})).is_empty());
    }

    #[test]
    fn upload_body_and_name() {
        let b = upload_body("XYZ", "src.png", b"PNGDATA");
        let s = String::from_utf8_lossy(&b);
        assert!(s.starts_with("--XYZ\r\nContent-Disposition: form-data; name=\"image\"; filename=\"src.png\"\r\nContent-Type: image/png\r\n\r\nPNGDATA\r\n--XYZ\r\n"));
        assert!(s.ends_with("name=\"overwrite\"\r\n\r\ntrue\r\n--XYZ--\r\n"));
        assert_eq!(parse_upload(&json!({"name": "src.png", "subfolder": "", "type": "input"})).unwrap(), "src.png");
        assert_eq!(parse_upload(&json!({"name": "src.png", "subfolder": "av", "type": "input"})).unwrap(), "av/src.png");
    }

    /// The whole exchange against a stand-in ComfyUI server on this computer.
    #[test]
    fn generate_against_local_server() {
        let polls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let pc = polls.clone();
        let server = test_server::start(move |req| match (req.method.as_str(), req.path.as_str()) {
            ("POST", "/upload/image") => {
                assert!(req.header("content-type").unwrap().starts_with("multipart/form-data; boundary="));
                (200, json!({"name": "anyvector-source.png", "subfolder": "", "type": "input"}).to_string().into_bytes())
            }
            ("GET", "/object_info/CheckpointLoaderSimple") => (200, json!({"CheckpointLoaderSimple": {"input": {"required": {"ckpt_name": [["only.safetensors"], {}]}}}}).to_string().into_bytes()),
            ("POST", "/prompt") => {
                let v: Value = serde_json::from_slice(&req.body).unwrap();
                assert_eq!(v["prompt"]["10"]["inputs"]["image"], "anyvector-source.png");
                assert_eq!(v["prompt"]["4"]["inputs"]["ckpt_name"], "only.safetensors");
                assert_eq!(v["prompt"]["6"]["inputs"]["text"], "a red fox");
                (200, json!({"prompt_id": "p9", "number": 1, "node_errors": {}}).to_string().into_bytes())
            }
            ("GET", "/history/p9") => {
                if pc.fetch_add(1, Ordering::SeqCst) == 0 {
                    (200, b"{}".to_vec())
                } else {
                    (200, json!({"p9": {"outputs": {"9": {"images": [{"filename": "out.png", "subfolder": "", "type": "output"}]}}, "status": {"status_str": "success", "completed": true}}}).to_string().into_bytes())
                }
            }
            ("GET", "/view?filename=out.png&subfolder=&type=output") => (200, b"\x89PNG fake".to_vec()),
            _ => (404, b"not found".to_vec()),
        });
        let wf = parse_workflow(IMAGE_TO_IMAGE).unwrap();
        let mut p = params();
        p.checkpoint.clear();
        let cancel = AtomicBool::new(false);
        let bytes = generate(&server.url, &wf, p, Some(b"source png"), &cancel, Duration::from_secs(30)).unwrap();
        assert_eq!(bytes, b"\x89PNG fake");
        assert_eq!(polls.load(Ordering::SeqCst), 2);
        let paths: Vec<String> = server.requests().iter().map(|r| r.path.clone()).collect();
        assert_eq!(paths.first().map(String::as_str), Some("/upload/image"));
    }

    #[test]
    fn server_not_running() {
        // a port that nothing listens on
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        drop(l);
        let e = list_checkpoints(&url).unwrap_err();
        assert!(e.contains("Make sure ComfyUI is running"), "{e}");
    }
}
