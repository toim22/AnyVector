//! Keyboard shortcut dispatch (defaults + user overrides from Options > Customization > Commands).

use crate::app::AnyVectorApp;
use crate::commands::{alt_shortcuts, context_shortcuts, default_shortcut, parse_shortcut, shortcut_commands, Cmd, KeyContext, Shortcut};
use crate::tools::ToolId;
use egui::{Key, Modifiers};

/// The effective shortcut text for a command (user override first).
pub fn shortcut_text(app: &AnyVectorApp, cmd: Cmd) -> Option<String> {
    let name = crate::scripting::cmd_to_text(&cmd);
    if let Some((_, sc)) = app.settings.shortcuts.iter().find(|(c, _)| *c == name) {
        return if sc.is_empty() { None } else { Some(sc.clone()) };
    }
    default_shortcut(cmd).map(|s| s.to_string())
}

fn bindings(app: &AnyVectorApp) -> Vec<(Shortcut, Cmd)> {
    let mut v = vec![];
    for c in shortcut_commands() {
        if let Some(s) = shortcut_text(app, c).and_then(|t| parse_shortcut(&t)) {
            v.push((s, c));
        }
        // the reference's other chords for the command, unless the user rebound it
        let name = crate::scripting::cmd_to_text(&c);
        if !app.settings.shortcuts.iter().any(|(n, _)| *n == name) {
            v.extend(alt_shortcuts(c).iter().filter_map(|t| parse_shortcut(t)).map(|s| (s, c)));
        }
    }
    for (name, sc) in &app.settings.shortcuts {
        if let (Some(c), Some(s)) = (crate::scripting::text_to_cmd(name), parse_shortcut(sc)) {
            if !v.iter().any(|(_, x)| *x == c) {
                v.push((s, c));
            }
        }
    }
    by_specificity(&mut v);
    v
}

/// egui also matches a chord with extra Shift / Alt held: the chords with more modifiers go first.
fn by_specificity<T>(v: &mut [(Shortcut, T)]) {
    v.sort_by_key(|(s, _)| std::cmp::Reverse(s.ctrl as u8 + s.shift as u8 + s.alt as u8));
}

/// The context table active now (text editing, Shape-tool nodes, table cells).
fn key_context(app: &AnyVectorApp) -> Option<KeyContext> {
    let d = app.doc()?;
    if d.text_edit.is_some() {
        Some(KeyContext::Text)
    } else if app.tool == ToolId::Table && app.ts.table_cell.is_some() {
        Some(KeyContext::TableCells)
    } else if app.tool == ToolId::Shape && !d.node_sel.is_empty() {
        Some(KeyContext::Nodes)
    } else {
        None
    }
}

fn mods(s: &Shortcut) -> Modifiers {
    Modifiers { alt: s.alt, ctrl: s.ctrl, shift: s.shift, mac_cmd: false, command: s.ctrl }
}

pub fn handle(app: &mut AnyVectorApp, ctx: &egui::Context) {
    // while a menu is open the keyboard belongs to it (Escape closes the menu, nothing else)
    if app.splash.is_some() || app.dialogs.any_modal_open() || egui::Popup::is_any_open(ctx) {
        return;
    }
    let typing = ctx.egui_wants_keyboard_input();
    let text_editing = app.doc().map(|d| d.text_edit.is_some()).unwrap_or(false) || app.ts.pending_text.is_some() || crate::tools::table_typing(app);
    // keys held while dragging Bézier / Pen handles are tool modifiers (C = cusp), not shortcuts
    let handle_drag = matches!(app.ts.drag, crate::tools::Drag::BezierHandle { .. });
    let mut fired: Vec<Cmd> = vec![];
    let binds = bindings(app);
    // a context table's chords replace the global ones while it is active (`None`: the global
    // command must not run, the key is left to the editor)
    let mut context: Vec<(Shortcut, Option<Cmd>)> = key_context(app).map(|k| context_shortcuts(k).iter().filter_map(|(t, c)| parse_shortcut(t).map(|s| (s, *c))).collect()).unwrap_or_default();
    by_specificity(&mut context);
    let same = |a: &Shortcut, b: &Shortcut| a.key == b.key && a.ctrl == b.ctrl && a.shift == b.shift && a.alt == b.alt;
    ctx.input_mut(|i| {
        if !typing && !handle_drag {
            for (s, c) in &context {
                let Some(c) = c else { continue };
                if i.consume_shortcut(&egui::KeyboardShortcut::new(mods(s), s.key)) {
                    fired.push(*c);
                }
            }
        }
        for (s, c) in &binds {
            let plain = !s.ctrl && !s.alt;
            if handle_drag && plain {
                continue;
            }
            if context.iter().any(|(x, _)| same(x, s)) {
                continue;
            }
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
        // eframe turns the clipboard chords (Ctrl+X/C/V, Shift+Delete, Ctrl+Insert, Shift+Insert)
        // into Cut/Copy/Paste events instead of key presses: map them back to the chord that was
        // pressed and run what is bound to it (text fields and in-place text keep them)
        if !typing && !text_editing {
            let m = i.modifiers;
            let mut keep = vec![];
            for ev in std::mem::take(&mut i.events) {
                let keys: &[Key] = match ev {
                    egui::Event::Cut if m.shift && !m.ctrl => &[Key::Delete],
                    egui::Event::Cut => &[Key::X, Key::Cut],
                    egui::Event::Copy => &[Key::C, Key::Insert, Key::Copy],
                    egui::Event::Paste(_) | egui::Event::PasteImage(_) if m.shift && !m.ctrl => &[Key::Insert],
                    egui::Event::Paste(_) | egui::Event::PasteImage(_) => &[Key::V, Key::Paste],
                    _ => {
                        keep.push(ev);
                        continue;
                    }
                };
                let hit = keys.iter().find_map(|k| binds.iter().find(|(s, _)| s.key == *k && s.ctrl == m.ctrl && s.shift == m.shift && s.alt == m.alt).map(|(_, c)| *c));
                match hit {
                    Some(c) => {
                        if !fired.contains(&c) {
                            fired.push(c);
                        }
                    }
                    None => keep.push(ev),
                }
            }
            i.events = keep;
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
                // the plain key last: egui matches it with Shift held too; Alt+arrows are left alone
                for (m, f) in [(Modifiers::SHIFT, sup), (Modifiers::CTRL, 1.0 / micro.max(1.0)), (Modifiers::NONE, 1.0)] {
                    if m == Modifiers::NONE && i.modifiers.alt {
                        continue;
                    }
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
