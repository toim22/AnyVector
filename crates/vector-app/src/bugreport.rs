//! Help ▸ Report a Bug: files a GitHub issue on the Vector project.
//!
//! Sending tries, in order:
//! 1. the GitHub CLI (`gh issue create`), when it is installed and signed in: the issue is
//!    created directly and its URL is shown;
//! 2. otherwise the browser opens GitHub's "New issue" page with the title, description and
//!    system information already filled in, so the user only has to press "Create".

use crate::app::VectorApp;
use crate::dialogs::{heading, note, separator, window};
use crate::theme;
use egui::{vec2, Color32, RichText};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};

/// The GitHub repository that receives bug reports.
pub const REPO: &str = "toim22/Vector";

pub fn issues_url() -> String {
    format!("https://github.com/{REPO}/issues")
}

#[derive(Debug)]
pub enum Sent {
    /// Created directly; the issue URL.
    Created(String),
    /// Opened the prefilled page in the browser.
    Browser,
    Failed(String),
}

#[derive(Default)]
pub struct BugReportState {
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

    fn can_send(&self) -> bool {
        !self.title.trim().is_empty() && !self.what_happened.trim().is_empty() && self.sending.is_none()
    }
}

/// Vector version, Windows version and display information.
pub fn system_info(ctx: &egui::Context) -> String {
    let os = windows_version();
    let ppp = ctx.pixels_per_point();
    let screen = ctx.content_rect().size() * ppp;
    format!(
        "- Vector: {} ({})\n- Windows: {}\n- Window size: {:.0} × {:.0} px, scale {:.0}%",
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
    b.push_str("### What happened\n\n");
    b.push_str(st.what_happened.trim());
    b.push_str("\n\n");
    if !st.steps.trim().is_empty() {
        b.push_str("### Steps to reproduce\n\n");
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
    b.push_str("\n_Sent from Vector (Help ▸ Report a Bug)._\n");
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

pub fn new_issue_url(title: &str, body: &str) -> String {
    format!("https://github.com/{REPO}/issues/new?labels=bug&title={}&body={}", url_encode(title), url_encode(body))
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

fn gh(args: &[&str]) -> Option<std::process::Output> {
    let mut cmd = Command::new("gh");
    cmd.args(args);
    hide_console(&mut cmd);
    cmd.output().ok()
}

/// Create the issue with the GitHub CLI. `None` if the CLI is missing or not signed in.
fn create_with_gh(title: &str, body: &str) -> Option<Result<String, String>> {
    let auth = gh(&["auth", "status", "--hostname", "github.com"])?;
    if !auth.status.success() {
        return None;
    }
    let mut out = gh(&["issue", "create", "--repo", REPO, "--title", title, "--body", body, "--label", "bug"])?;
    if !out.status.success() {
        // The repository may not have a "bug" label; retry without it.
        out = gh(&["issue", "create", "--repo", REPO, "--title", title, "--body", body])?;
    }
    if out.status.success() {
        let url = String::from_utf8_lossy(&out.stdout).lines().rev().find(|l| l.starts_with("https://")).unwrap_or_default().trim().to_string();
        Some(Ok(if url.is_empty() { issues_url() } else { url }))
    } else {
        Some(Err(String::from_utf8_lossy(&out.stderr).trim().to_string()))
    }
}

/// Send in the background; the dialog polls the receiver.
fn send(title: String, body: String) -> Receiver<Sent> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let r = match create_with_gh(&title, &body) {
            Some(Ok(url)) => Sent::Created(url),
            // CLI present but failed, or absent: fall back to the browser.
            Some(Err(_)) | None => {
                if open_in_browser(&new_issue_url(&title, &body)) {
                    Sent::Browser
                } else {
                    Sent::Failed(format!("Could not open the browser. Please report the bug at {}", issues_url()))
                }
            }
        };
        let _ = tx.send(r);
    });
    rx
}

pub fn dialog(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.bug_report.take() else { return };
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
    let ((), dismissed) = window(ctx, "bug_report", "Report a Bug", vec2(560.0, 560.0), |ui| {
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        if let Some(r) = &st.result {
            heading(ui, match r {
                Sent::Created(_) => "Thank you! Your bug report was sent.",
                Sent::Browser => "Almost done: finish in your browser",
                Sent::Failed(_) => "The report could not be sent",
            });
            ui.add_space(6.0);
            match r {
                Sent::Created(url) => {
                    ui.label("The issue was created on the Vector project on GitHub:");
                    if ui.link(url.as_str()).clicked() {
                        open_in_browser(url);
                    }
                }
                Sent::Browser => {
                    ui.label("GitHub opened in your browser with the report already filled in.");
                    ui.label("Sign in if asked, review it and click \"Create\" to send it.");
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
                    if matches!(st.result, Some(Sent::Failed(_))) && ui.add(egui::Button::new("Back").min_size(vec2(80.0, 24.0))).clicked() {
                        st.result = None;
                    }
                });
            });
            return;
        }
        note(ui, "Describe the problem. It is sent to the Vector project on GitHub as a new issue.");
        ui.add_space(4.0);
        ui.label("Title (a short summary):");
        ui.add(egui::TextEdit::singleline(&mut st.title).hint_text("e.g. Toolbar overlaps the menu bar").desired_width(f32::INFINITY));
        ui.label("What happened?");
        ui.add(egui::TextEdit::multiline(&mut st.what_happened).desired_rows(4).desired_width(f32::INFINITY));
        ui.label("Steps to reproduce (optional):");
        ui.add(egui::TextEdit::multiline(&mut st.steps).hint_text("1. ...\n2. ...").desired_rows(3).desired_width(f32::INFINITY));
        ui.label("What did you expect to happen? (optional)");
        ui.add(egui::TextEdit::multiline(&mut st.expected).desired_rows(2).desired_width(f32::INFINITY));
        separator(ui);
        ui.checkbox(&mut st.include_system, "Include system information");
        if st.include_system {
            ui.indent("sys", |ui| {
                ui.label(RichText::new(system.as_str()).color(theme::TEXT_DIM).size(11.5));
            });
        }
        ui.add_space((ui.available_height() - 34.0).max(0.0));
        ui.horizontal(|ui| {
            if ui.link("View reported bugs").clicked() {
                open_in_browser(&issues_url());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("Cancel").min_size(vec2(80.0, 24.0))).clicked() {
                    close = true;
                }
                let label = if st.sending.is_some() { "Sending..." } else { "Send" };
                if crate::dialogs::ok_button(ui, label, st.can_send()) {
                    let body = issue_body(&st, st.include_system.then_some(system.as_str()));
                    st.sending = Some(send(st.title.trim().to_string(), body));
                }
                if st.sending.is_some() {
                    ui.spinner();
                }
            });
        });
    });
    if !(close || (dismissed && st.sending.is_none())) {
        app.dialogs.bug_report = Some(st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_builds_url() {
        assert_eq!(url_encode("a b&c/ç"), "a%20b%26c%2F%C3%A7");
        let st = BugReportState { title: "T".into(), what_happened: "Crash".into(), steps: "1. Open".into(), ..BugReportState::new() };
        let body = issue_body(&st, Some("- Vector: 1.0.0"));
        assert!(body.contains("### What happened\n\nCrash"));
        assert!(body.contains("### Steps to reproduce"));
        assert!(!body.contains("### Expected behavior"));
        let url = new_issue_url("T", &body);
        assert!(url.starts_with("https://github.com/toim22/Vector/issues/new?labels=bug&title=T&body="));
    }
}
