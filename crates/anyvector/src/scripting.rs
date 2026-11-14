//! Scripts (Tools > Scripts): Rhai engine, command recording and playback.
//!
//! Recorded scripts are plain Rhai: every command becomes `run("Name")` (or a JSON payload for
//! commands with arguments, e.g. `run("{\"Align\":\"Left\"}")`). Scripts can also draw:
//! `rect(x, y, w, h)`, `ellipse(cx, cy, rx, ry)`, `line(x1, y1, x2, y2)`, `text(x, y, "s")`,
//! `fill("#ff0000")`, `outline("#000000", width)`, `select_all()`, `move_by(dx, dy)`,
//! `rotate(deg)`, `scale(sx, sy)`, `message("...")`. Units are points.
//!
//! Device output plugins: a script in the `Devices` folder of the Scripts folder can add an
//! export format with `register_export("Name (*.ext)", "ext", "writer")` and a function
//! `writer(paths, page)` that receives the page's outlines flattened to polylines (with color,
//! spot color name, layer and pen) and returns the file's bytes. See [`devices`].

pub mod devices;

use crate::app::AnyVectorApp;
use crate::commands::Cmd;
use std::sync::{Arc, Mutex};

pub struct Recorder {
    pub lines: Vec<String>,
    pub paused: bool,
    /// Temporary recording (Record Temporary Script): kept in `app.temp_script`.
    pub temporary: bool,
}

impl Recorder {
    pub fn new(temporary: bool) -> Recorder {
        Recorder { lines: vec![], paused: false, temporary }
    }
    pub fn record(&mut self, cmd: &Cmd) {
        if self.paused || !recordable(cmd) {
            return;
        }
        self.lines.push(format!("run({});", rhai_string(&cmd_to_text(cmd))));
    }
    pub fn script(&self) -> String {
        let mut s = String::from("// AnyVector script\n");
        for l in &self.lines {
            s.push_str(l);
            s.push('\n');
        }
        s
    }
}

/// Commands that make sense to record (no dialogs/recording control).
fn recordable(cmd: &Cmd) -> bool {
    !matches!(
        cmd,
        Cmd::StartRecording | Cmd::PauseRecording | Cmd::StopRecording | Cmd::CancelRecording | Cmd::RecordTemporary | Cmd::RunTemporary | Cmd::ScriptEditor | Cmd::RunScript | Cmd::VsEditor | Cmd::Exit | Cmd::About
    )
}

pub fn cmd_to_text(cmd: &Cmd) -> String {
    match serde_json::to_value(cmd) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(v) => v.to_string(),
        Err(_) => format!("{cmd:?}"),
    }
}

pub fn text_to_cmd(s: &str) -> Option<Cmd> {
    let s = s.trim();
    serde_json::from_str::<Cmd>(s).ok().or_else(|| serde_json::from_value::<Cmd>(serde_json::Value::String(s.to_string())).ok())
}

fn rhai_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Operations produced by a script run, applied to the app in order afterwards.
#[derive(Clone, Debug)]
pub enum Op {
    Cmd(Cmd),
    Rect(f64, f64, f64, f64),
    Ellipse(f64, f64, f64, f64),
    Line(f64, f64, f64, f64),
    Text(f64, f64, String),
    Fill(String),
    Outline(String, f64),
    SelectAll,
    MoveBy(f64, f64),
    Rotate(f64),
    Scale(f64, f64),
    Message(String),
}

fn num(d: rhai::Dynamic) -> f64 {
    d.as_float().unwrap_or_else(|_| d.as_int().map(|i| i as f64).unwrap_or(0.0))
}

pub fn run_script(app: &mut AnyVectorApp, source: &str) -> Result<(), String> {
    let ops: Arc<Mutex<Vec<Op>>> = Arc::new(Mutex::new(vec![]));
    let mut engine = rhai::Engine::new();
    engine.set_max_operations(5_000_000);
    engine.set_max_call_levels(64);
    macro_rules! reg {
        ($name:expr, |$($a:ident),*| $op:expr) => {{
            let o = ops.clone();
            engine.register_fn($name, move |$($a: rhai::Dynamic),*| { o.lock().unwrap().push($op); });
        }};
    }
    {
        let o = ops.clone();
        engine.register_fn("run", move |s: &str| -> Result<(), Box<rhai::EvalAltResult>> {
            match text_to_cmd(s) {
                Some(c) => {
                    o.lock().unwrap().push(Op::Cmd(c));
                    Ok(())
                }
                None => Err(format!("unknown command: {s}").into()),
            }
        });
    }
    reg!("rect", |x, y, w, h| Op::Rect(num(x), num(y), num(w), num(h)));
    reg!("ellipse", |x, y, rx, ry| Op::Ellipse(num(x), num(y), num(rx), num(ry)));
    reg!("line", |x1, y1, x2, y2| Op::Line(num(x1), num(y1), num(x2), num(y2)));
    reg!("move_by", |dx, dy| Op::MoveBy(num(dx), num(dy)));
    reg!("scale", |sx, sy| Op::Scale(num(sx), num(sy)));
    reg!("rotate", |a| Op::Rotate(num(a)));
    {
        let o = ops.clone();
        engine.register_fn("text", move |x: rhai::Dynamic, y: rhai::Dynamic, s: &str| o.lock().unwrap().push(Op::Text(num(x), num(y), s.to_string())));
        let o = ops.clone();
        engine.register_fn("fill", move |s: &str| o.lock().unwrap().push(Op::Fill(s.to_string())));
        let o = ops.clone();
        engine.register_fn("outline", move |s: &str, w: rhai::Dynamic| o.lock().unwrap().push(Op::Outline(s.to_string(), num(w))));
        let o = ops.clone();
        engine.register_fn("select_all", move || o.lock().unwrap().push(Op::SelectAll));
        let o = ops.clone();
        engine.register_fn("message", move |s: &str| o.lock().unwrap().push(Op::Message(s.to_string())));
    }
    engine.run(source).map_err(|e| e.to_string())?;
    let list = std::mem::take(&mut *ops.lock().unwrap());
    apply_ops(app, list);
    Ok(())
}

fn apply_ops(app: &mut AnyVectorApp, ops: Vec<Op>) {
    use kurbo::{Affine, Point};
    use vector_core::object::{EllipseKind, EllipseShape, RectShape};
    use vector_core::{ObjKind, Object, Style};
    for op in ops {
        if let Op::Cmd(c) = op {
            app.run(c);
            continue;
        }
        if let Op::Message(m) = &op {
            app.flash(m.clone());
            continue;
        }
        if !app.has_doc() {
            app.new_default_document();
        }
        let fill = app.default_fill;
        let Some(d) = app.doc_mut() else { continue };
        match op {
            Op::Rect(x, y, w, h) => {
                let id = d.doc.new_id();
                let o = Object::new(id, ObjKind::Rect(RectShape { w, h, radii: [0.0; 4], corner: Default::default() }), Affine::translate((x, y)), Style::default());
                d.add_objects("Script", vec![o]);
            }
            Op::Ellipse(cx, cy, rx, ry) => {
                let id = d.doc.new_id();
                let e = EllipseShape { w: rx * 2.0, h: ry * 2.0, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false };
                let o = Object::new(id, ObjKind::Ellipse(e), Affine::translate((cx - rx, cy - ry)), Style::default());
                d.add_objects("Script", vec![o]);
            }
            Op::Line(x1, y1, x2, y2) => {
                let id = d.doc.new_id();
                let np = vector_core::path::polyline(&[Point::new(x1, y1), Point::new(x2, y2)], false);
                let o = Object::new(id, ObjKind::Path(np), Affine::IDENTITY, Style::default());
                d.add_objects("Script", vec![o]);
            }
            Op::Text(x, y, s) => {
                let id = d.doc.new_id();
                let t = vector_core::text::TextObj::artistic(&s, Default::default());
                let o = Object::new(id, ObjKind::Text(t), Affine::translate((x, y)), Style::text_default());
                d.add_objects("Script", vec![o]);
            }
            Op::Fill(hex) => {
                let c = vector_core::Color::from_hex(&hex).unwrap_or(fill);
                let sel = d.selection.clone();
                d.modify("Script Fill", |doc, _| {
                    for id in sel {
                        if let Some(o) = doc.find_mut(id) {
                            o.style.fill = vector_core::Fill::Solid(c);
                        }
                    }
                });
            }
            Op::Outline(hex, w) => {
                let c = vector_core::Color::from_hex(&hex);
                let sel = d.selection.clone();
                d.modify("Script Outline", |doc, _| {
                    for id in sel {
                        if let Some(o) = doc.find_mut(id) {
                            o.style.outline.color = c;
                            o.style.outline.width = w;
                        }
                    }
                });
            }
            Op::SelectAll => {
                let page = d.page;
                d.selection = d.doc.pages[page].layers.iter().filter(|l| l.visible && !l.locked).flat_map(|l| l.objects.iter().map(|o| o.id)).collect();
            }
            Op::MoveBy(dx, dy) => {
                let sel = d.selection.clone();
                d.modify("Script Move", |doc, _| doc.transform_objects(&sel, Affine::translate((dx, dy))));
            }
            Op::Rotate(a) => {
                if let Some(bb) = d.selection_bounds() {
                    let c = bb.center();
                    let sel = d.selection.clone();
                    // positive angles are counter-clockwise on screen (y-down document)
                    let m = Affine::translate(c.to_vec2()) * Affine::rotate(-a.to_radians()) * Affine::translate(-c.to_vec2());
                    d.modify("Script Rotate", |doc, _| doc.transform_objects(&sel, m));
                }
            }
            Op::Scale(sx, sy) => {
                if let Some(bb) = d.selection_bounds() {
                    let c = bb.center();
                    let sel = d.selection.clone();
                    let m = Affine::translate(c.to_vec2()) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-c.to_vec2());
                    d.modify("Script Scale", |doc, _| doc.transform_objects(&sel, m));
                }
            }
            Op::Cmd(_) | Op::Message(_) => {}
        }
    }
}

/// Folder holding user scripts (%APPDATA%\AnyVector\Scripts).
pub fn scripts_dir() -> std::path::PathBuf {
    let d = crate::settings::config_dir().join("Scripts");
    let _ = std::fs::create_dir_all(&d);
    d
}
