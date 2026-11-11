//! Help ▸ Report a Bug and Help ▸ Submit Ideas and Feedback: file a GitHub issue on the
//! AnyVector project, labeled `bug` or `enhancement`.
//!
//! Sending tries, in order:
//! 1. the issue relay (`services/issue-relay`, a Cloudflare Worker): it creates the issue as the
//!    AnyVector Reports GitHub App, so the user needs no GitHub account, and its URL is shown;
//! 2. when the relay can't be reached, the browser opens GitHub's "New issue" page with the title,
//!    description and system information already filled in, so the user only has to press "Create"
//!    (that needs a GitHub account).

use crate::app::AnyVectorApp;
use crate::dialogs::{heading, note, separator, window};
use crate::theme;
use egui::{vec2, Color32, RichText};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};

/// The GitHub repository that receives bug reports.
pub const REPO: &str = "toim22/AnyVector";

/// The relay that files the issues for people without a GitHub account: `services/issue-relay`.
/// The environment variable `ANYVECTOR_ISSUE_RELAY` points elsewhere (a local `cf dev`, a test).
pub const RELAY_URL: &str = "https://anyvector-issues.antoniocarlos85.workers.dev";

fn relay_url() -> String {
    std::env::var("ANYVECTOR_ISSUE_RELAY").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| RELAY_URL.to_owned())
}

pub fn issues_url() -> String {
    format!("https://github.com/{REPO}/issues")
}

#[derive(Debug)]
pub enum Sent {
    /// Created directly; the issue URL.
    Created(String),
    /// The relay couldn't be reached: the prefilled page opened in the browser.
    Browser,
    /// The relay asks to wait: several reports in a minute from this connection.
    Later,
    Failed(String),
}

#[derive(Default)]
pub struct BugReportState {
    /// Submit Ideas and Feedback instead of Report a Bug.
    pub idea: bool,
    pub title: String,
    pub what_happened: String,
    pub steps: String,
    pub expected: String,
    pub include_system: bool,
    pub sending: Option<Receiver<Sent>>,
    pub result: Option<Sent>,
}

impl BugReportState {
    pub fn new() -> BugReportState {
        BugReportState { include_system: true, ..Default::default() }
    }

    /// Help ▸ Submit Ideas and Feedback.
    pub fn idea() -> BugReportState {
        BugReportState { idea: true, include_system: false, ..Default::default() }
    }

    /// The GitHub label of the issue.
    pub fn label(&self) -> &'static str {
        if self.idea { "enhancement" } else { "bug" }
    }

    fn can_send(&self) -> bool {
        !self.title.trim().is_empty() && !self.what_happened.trim().is_empty() && self.sending.is_none()
    }
}

/// AnyVector version, Windows version and display information.
pub fn system_info(ctx: &egui::Context) -> String {
    let os = windows_version();
    let ppp = ctx.pixels_per_point();
    let screen = ctx.content_rect().size() * ppp;
    format!(
        "- AnyVector: {} ({})\n- Windows: {}\n- Window size: {:.0} × {:.0} px, scale {:.0}%",
        env!("CARGO_PKG_VERSION"),
        if cfg!(debug_assertions) { "debug build" } else { "release build" },
        os,
        screen.x,
        screen.y,
        ppp * 100.0
    )
}

fn windows_version() -> String {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "ver"]);
    hide_console(&mut cmd);
    cmd.output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::env::consts::OS.to_string())
}

fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

/// The issue body in GitHub Markdown.
pub fn issue_body(st: &BugReportState, system: Option<&str>) -> String {
    let mut b = String::new();
    b.push_str(if st.idea { "### Idea or feedback\n\n" } else { "### What happened\n\n" });
    b.push_str(st.what_happened.trim());
    b.push_str("\n\n");
    if !st.steps.trim().is_empty() {
        b.push_str(if st.idea { "### Why it would help\n\n" } else { "### Steps to reproduce\n\n" });
        b.push_str(st.steps.trim());
        b.push_str("\n\n");
    }
    if !st.expected.trim().is_empty() {
        b.push_str("### Expected behavior\n\n");
        b.push_str(st.expected.trim());
        b.push_str("\n\n");
    }
    if let Some(s) = system {
        b.push_str("### System information\n\n");
        b.push_str(s);
        b.push('\n');
    }
    b.push_str(if st.idea { "\n_Sent from AnyVector (Help ▸ Submit Ideas and Feedback)._\n" } else { "\n_Sent from AnyVector (Help ▸ Report a Bug)._\n" });
    b
}

/// Percent-encode a query-string value.
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn new_issue_url(label: &str, title: &str, body: &str) -> String {
    format!("https://github.com/{REPO}/issues/new?labels={label}&title={}&body={}", url_encode(title), url_encode(body))
}

pub fn open_in_browser(url: &str) -> bool {
    #[cfg(windows)]
    {
        // `start` treats the first quoted argument as the window title, hence the "".
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        hide_console(&mut cmd);
        cmd.spawn().is_ok()
    }
    #[cfg(not(windows))]
    {
        Command::new("xdg-open").arg(url).spawn().is_ok()
    }
}

/// Run the GitHub CLI without a console window; `None` when it is not installed.
pub(crate) fn gh(args: &[&str]) -> Option<std::process::Output> {
    let mut cmd = Command::new("gh");
    cmd.args(args);
    hide_console(&mut cmd);
    cmd.output().ok()
}

/// At most `max` characters of `s`, as the relay counts them.
fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// The report as the relay reads it (`services/issue-relay/src/report.ts`), cut to its limits.
pub fn relay_body(st: &BugReportState, system: Option<&str>) -> String {
    serde_json::json!({
        "kind": if st.idea { "idea" } else { "bug" },
        "title": clip(st.title.trim(), 200),
        "text": clip(st.what_happened.trim(), 8000),
        "more": clip(st.steps.trim(), 4000),
        "expected": if st.idea { String::new() } else { clip(st.expected.trim(), 2000) },
        "version": env!("CARGO_PKG_VERSION"),
        "program": "AnyVector",
        "system": system.map(|s| clip(s.trim(), 600)).unwrap_or_default(),
    })
    .to_string()
}

/// Post the report to the relay: the issue's URL, or the HTTP status (0 when unreachable) and why.
fn send_to_relay(base: &str, body: &str) -> Result<String, (u16, String)> {
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(std::time::Duration::from_secs(20))).build().into();
    let url = format!("{}/issues", base.trim_end_matches('/'));
    let ua = format!("AnyVector/{}", env!("CARGO_PKG_VERSION"));
    match agent.post(&url).header("Content-Type", "application/json").header("User-Agent", &ua).send(body) {
        Ok(mut r) => {
            let status = r.status().as_u16();
            let text = r.body_mut().read_to_string().unwrap_or_default();
            if status == 201 || status == 200 {
                let url = serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|v| v["url"].as_str().map(str::to_string)).unwrap_or_default();
                Ok(if url.starts_with("https://github.com/") { url } else { issues_url() })
            } else {
                Err((status, text))
            }
        }
        Err(e) => Err((0, e.to_string())),
    }
}

/// Send in the background; the dialog polls the receiver. The relay first; when it can't take the
/// report (other than asking to wait), GitHub's prefilled page in the browser.
fn send(relay: String, label: &'static str, title: String, body: String) -> Receiver<Sent> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let r = match send_to_relay(&relay_url(), &relay) {
            Ok(url) => Sent::Created(url),
            Err((429, _)) => Sent::Later,
            Err((status, e)) => {
                log::warn!("issue relay did not take the report ({status} {e}); opening the browser");
                if open_in_browser(&new_issue_url(label, &title, &body)) {
                    Sent::Browser
                } else {
                    Sent::Failed(format!("Could not open the browser. Please send it at {}", issues_url()))
                }
            }
        };
        let _ = tx.send(r);
    });
    rx
}

/// Report a Bug (`idea` false) or Submit Ideas and Feedback (`idea` true).
pub fn dialog(app: &mut AnyVectorApp, ctx: &egui::Context, idea: bool) {
    let slot = if idea { &mut app.dialogs.feedback } else { &mut app.dialogs.bug_report };
    let Some(mut st) = slot.take() else { return };
    if let Some(rx) = &st.sending {
        if let Ok(r) = rx.try_recv() {
            st.sending = None;
            st.result = Some(r);
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    let system = system_info(ctx);
    let mut close = false;
    let (id, title) = if idea { ("feedback", "Submit Ideas and Feedback") } else { ("bug_report", "Report a Bug") };
    let ((), dismissed) = window(ctx, id, title, vec2(560.0, 560.0), |ui| {
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        if let Some(r) = &st.result {
            heading(ui, match r {
                Sent::Created(_) if st.idea => "Thank you! Your idea was sent.",
                Sent::Created(_) => "Thank you! Your bug report was sent.",
                Sent::Browser => "Almost done: finish in your browser",
                Sent::Later => "Please wait a minute",
                Sent::Failed(_) if st.idea => "Your idea could not be sent",
                Sent::Failed(_) => "The report could not be sent",
            });
            ui.add_space(6.0);
            match r {
                Sent::Created(url) => {
                    ui.label("The issue was created on the AnyVector project on GitHub:");
                    if ui.link(url.as_str()).clicked() {
                        open_in_browser(url);
                    }
                }
                Sent::Browser => {
                    ui.label(if st.idea { "GitHub opened in your browser with your idea already filled in." } else { "GitHub opened in your browser with the report already filled in." });
                    ui.label("Sign in if asked, review it and click \"Create\" to send it.");
                }
                Sent::Later => {
                    ui.label("Several reports were sent from this connection in the last minute. Try again in a minute.");
                }
                Sent::Failed(e) => {
                    ui.colored_label(Color32::from_rgb(0xe0, 0x70, 0x60), e.as_str());
                }
            }
            ui.add_space(ui.available_height() - 34.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::dialogs::ok_button(ui, "Close", true) {
                        close = true;
                    }
                    if matches!(st.result, Some(Sent::Failed(_) | Sent::Later)) && ui.add(egui::Button::new("Back").min_size(vec2(80.0, 24.0))).clicked() {
                        st.result = None;
                    }
                });
            });
            return;
        }
        if st.idea {
            note(ui, "Tell us your idea or what you think of AnyVector. It is sent to the AnyVector project on GitHub as a new idea. You don't need a GitHub account.");
            ui.add_space(4.0);
            ui.label("Title (your idea in a few words):");
            ui.add(egui::TextEdit::singleline(&mut st.title).hint_text("e.g. Snap to the center of the page").desired_width(f32::INFINITY));
            ui.label("Describe your idea or feedback:");
            ui.add(egui::TextEdit::multiline(&mut st.what_happened).desired_rows(6).desired_width(f32::INFINITY));
            ui.label("How would it help you? (optional)");
            ui.add(egui::TextEdit::multiline(&mut st.steps).desired_rows(4).desired_width(f32::INFINITY));
        } else {
            note(ui, "Describe the problem. It is sent to the AnyVector project on GitHub as a new issue. You don't need a GitHub account.");
            ui.add_space(4.0);
            ui.label("Title (a short summary):");
            ui.add(egui::TextEdit::singleline(&mut st.title).hint_text("e.g. Toolbar overlaps the menu bar").desired_width(f32::INFINITY));
            ui.label("What happened?");
            ui.add(egui::TextEdit::multiline(&mut st.what_happened).desired_rows(4).desired_width(f32::INFINITY));
            ui.label("Steps to reproduce (optional):");
            ui.add(egui::TextEdit::multiline(&mut st.steps).hint_text("1. ...\n2. ...").desired_rows(3).desired_width(f32::INFINITY));
            ui.label("What did you expect to happen? (optional)");
            ui.add(egui::TextEdit::multiline(&mut st.expected).desired_rows(2).desired_width(f32::INFINITY));
        }
        separator(ui);
        ui.checkbox(&mut st.include_system, "Include system information");
        if st.include_system {
            ui.indent("sys", |ui| {
                ui.label(RichText::new(system.as_str()).color(theme::TEXT_DIM).size(11.5));
            });
        }
        ui.add_space((ui.available_height() - 34.0).max(0.0));
        ui.horizontal(|ui| {
            if ui.link(if st.idea { "View submitted ideas" } else { "View reported bugs" }).clicked() {
                open_in_browser(&if st.idea { format!("{}?q=is%3Aissue+label%3Aenhancement", issues_url()) } else { issues_url() });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("Cancel").min_size(vec2(80.0, 24.0))).clicked() {
                    close = true;
                }
                let label = if st.sending.is_some() { "Sending..." } else { "Send" };
                if crate::dialogs::ok_button(ui, label, st.can_send()) {
                    let sys = st.include_system.then_some(system.as_str());
                    let body = issue_body(&st, sys);
                    st.sending = Some(send(relay_body(&st, sys), st.label(), st.title.trim().to_string(), body));
                }
                if st.sending.is_some() {
                    ui.spinner();
                }
            });
        });
    });
    if !(close || (dismissed && st.sending.is_none())) {
        *(if idea { &mut app.dialogs.feedback } else { &mut app.dialogs.bug_report }) = Some(st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_builds_url() {
        assert_eq!(url_encode("a b&c/ç"), "a%20b%26c%2F%C3%A7");
        let st = BugReportState { title: "T".into(), what_happened: "Crash".into(), steps: "1. Open".into(), ..BugReportState::new() };
        let body = issue_body(&st, Some("- AnyVector: 0.1.0"));
        assert!(body.contains("### What happened\n\nCrash"));
        assert!(body.contains("### Steps to reproduce"));
        assert!(!body.contains("### Expected behavior"));
        let url = new_issue_url(st.label(), "T", &body);
        assert!(url.starts_with("https://github.com/toim22/AnyVector/issues/new?labels=bug&title=T&body="));
        let idea = BugReportState { title: "Snap".into(), what_happened: "Snap to page center".into(), steps: "Faster layout".into(), ..BugReportState::idea() };
        let body = issue_body(&idea, None);
        assert!(body.contains("### Idea or feedback\n\nSnap to page center"));
        assert!(body.contains("### Why it would help\n\nFaster layout"));
        assert!(!body.contains("System information"));
        assert!(new_issue_url(idea.label(), "Snap", &body).contains("labels=enhancement&"));

        // what the relay reads (services/issue-relay/src/report.ts)
        let v: serde_json::Value = serde_json::from_str(&relay_body(&st, Some("- Windows: 11"))).unwrap();
        assert_eq!(v["kind"], "bug");
        assert_eq!(v["title"], "T");
        assert_eq!(v["text"], "Crash");
        assert_eq!(v["more"], "1. Open");
        assert_eq!(v["program"], "AnyVector");
        assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(v["system"], "- Windows: 11");
        let v: serde_json::Value = serde_json::from_str(&relay_body(&BugReportState { expected: "x".into(), title: "é".repeat(300), ..idea }, None)).unwrap();
        assert_eq!((v["kind"].as_str(), v["expected"].as_str(), v["system"].as_str()), (Some("idea"), Some(""), Some("")));
        assert_eq!(v["title"].as_str().unwrap().chars().count(), 200);
    }
}
