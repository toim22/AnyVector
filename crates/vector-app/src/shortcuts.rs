//! Keyboard shortcut dispatch (defaults + user overrides from Options > Customization > Commands).

use crate::app::VectorApp;
use crate::commands::{default_shortcut, parse_shortcut, shortcut_commands, Cmd, Shortcut};
use crate::tools::ToolId;
use egui::{Key, Modifiers};

/// The effective shortcut text for a command (user override first).
pub fn shortcut_text(app: &VectorApp, cmd: Cmd) -> Option<String> {
    let name = crate::scripting::cmd_to_text(&cmd);
    if let Some((_, sc)) = app.settings.shortcuts.iter().find(|(c, _)| *c == name) {
        return if sc.is_empty() { None } else { Some(sc.clone()) };
    }
    default_shortcut(cmd).map(|s| s.to_string())
}

fn bindings(app: &VectorApp) -> Vec<(Shortcut, Cmd)> {
    let mut v = vec![];
    for c in shortcut_commands() {
        if let Some(s) = shortcut_text(app, c).and_then(|t| parse_shortcut(&t)) {
            v.push((s, c));
        }
    }
    for (name, sc) in &app.settings.shortcuts {
        if let (Some(c), Some(s)) = (crate::scripting::text_to_cmd(name), parse_shortcut(sc)) {
            if !v.iter().any(|(_, x)| *x == c) {
                v.push((s, c));
            }
        }
    }
    v
}

fn mods(s: &Shortcut) -> Modifiers {
    Modifiers { alt: s.alt, ctrl: s.ctrl, shift: s.shift, mac_cmd: false, command: s.ctrl }
}

pub fn handle(app: &mut VectorApp, ctx: &egui::Context) {
    if app.splash.is_some() || app.dialogs.any_modal_open() {
        return;
    }
    let typing = ctx.egui_wants_keyboard_input();
    let text_editing = app.doc().map(|d| d.text_edit.is_some()).unwrap_or(false) || app.ts.pending_text.is_some();
    let mut fired: Vec<Cmd> = vec![];
    let binds = bindings(app);
    ctx.input_mut(|i| {
        for (s, c) in &binds {
            let plain = !s.ctrl && !s.alt;
            // single keys (tool letters, alignment, Delete) must not fire while typing
            if (typing || text_editing) && plain && !matches!(s.key, Key::F1 | Key::F2 | Key::F3 | Key::F4 | Key::F5 | Key::F6 | Key::F7 | Key::F8 | Key::F9 | Key::F10 | Key::F11 | Key::F12) {
                continue;
            }
            if typing && matches!(c, Cmd::Cut | Cmd::Copy | Cmd::Paste | Cmd::SelectAll | Cmd::Undo | Cmd::Redo) {
                continue;
            }
            if text_editing && matches!(c, Cmd::Cut | Cmd::Copy | Cmd::Paste | Cmd::SelectAll | Cmd::Delete) {
                continue;
            }
            let ks = egui::KeyboardShortcut::new(mods(s), s.key);
            if i.consume_shortcut(&ks) {
                fired.push(*c);
            }
        }
    });
    // Ctrl+Y = Redo as well (Windows convention)
    if !typing && ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::Y))) {
        fired.push(Cmd::Redo);
    }
    if !typing && !text_editing && app.has_doc() {
        // nudge with arrow keys: Shift = super nudge, Ctrl = micro nudge
        let (nudge, sup, micro) = app.doc().map(|d| (d.doc.settings.nudge, d.doc.settings.super_nudge_mult, d.doc.settings.micro_nudge_div)).unwrap_or((7.2, 2.0, 2.0));
        ctx.input_mut(|i| {
            for (k, dx, dy) in [(Key::ArrowLeft, -1.0, 0.0), (Key::ArrowRight, 1.0, 0.0), (Key::ArrowUp, 0.0, -1.0), (Key::ArrowDown, 0.0, 1.0)] {
                for (m, f) in [(Modifiers::NONE, 1.0), (Modifiers::SHIFT, sup), (Modifiers::CTRL, 1.0 / micro.max(1.0))] {
                    if i.consume_key(m, k) {
                        fired.push(Cmd::Nudge(dx * nudge * f, dy * nudge * f));
                    }
                }
            }
        });
        // Space toggles between the Pick tool and the previous tool
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Space)) {
            if app.tool == ToolId::Pick {
                if let Some(prev) = app.ts.previous_tool {
                    app.set_tool(prev);
                }
            } else {
                app.ts.previous_tool = Some(app.tool);
                app.set_tool(ToolId::Pick);
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if app.ts.is_busy() {
                app.ts.cancel();
            } else if let Some(d) = app.doc_mut() {
                d.selection.clear();
                d.node_sel.clear();
            }
        }
        // Enter finishes multi-click tools (polyline, Bézier)
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            app.finish_tool();
        }
    }
    for c in fired {
        app.run(c);
    }
}
