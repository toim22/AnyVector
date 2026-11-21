//! Options ▸ Customization ▸ Commands: the commands by menu (and the toolbox, and the commands
//! only a key runs), each with its shortcut keys, and the Shortcut Keys tab where any of them gets
//! the chord you press, as in CorelDRAW (#125).

use super::{combo, label_w, tabs};
use crate::app::AnyVectorApp;
use crate::commands::{format_shortcut, Cmd, Shortcut};
use crate::dialogs::note;
use crate::menus::CatalogEntry;
use crate::theme;
use egui::{vec2, Stroke, Ui};
use std::sync::OnceLock;

/// The categories of the list: the menus of the menu bar, the toolbars, the toolbox, and the commands with a
/// shortcut key that no menu shows.
pub(crate) fn categories() -> &'static [(String, Vec<CatalogEntry>)] {
    static CATS: OnceLock<Vec<(String, Vec<CatalogEntry>)>> = OnceLock::new();
    CATS.get_or_init(|| {
        // the menus and the toolbars; the toolbox is listed below from the tools themselves
        let mut out: Vec<(String, Vec<CatalogEntry>)> = crate::menus::catalog_menus().into_iter().filter(|(name, _)| name != "Toolbox").map(|(name, bar)| (name, crate::menus::catalog(bar))).collect();
        let tools: Vec<CatalogEntry> = crate::tools::TOOLS.iter().map(|t| CatalogEntry { icon: Some(t.icon.to_string()), label: t.name.to_string(), cmd: Cmd::Tool(t.id) }).collect();
        let listed = |c: &Cmd, out: &[(String, Vec<CatalogEntry>)]| out.iter().any(|(_, es)| es.iter().any(|e| e.cmd == *c));
        let mut other = vec![];
        for c in crate::commands::shortcut_commands() {
            if !listed(&c, &out) && !tools.iter().any(|e| e.cmd == c) {
                other.push(CatalogEntry { icon: None, label: other_label(c), cmd: c });
            }
        }
        other.sort_by(|a, b| crate::widgets::plain(&a.label).cmp(&crate::widgets::plain(&b.label)));
        out.push(("Toolbox".into(), tools));
        out.push(("Other Commands".into(), other));
        out
    })
}

/// The caption of a command no menu lists: its item on another bar (a property bar button),
/// or its tool, docker or effect.
fn other_label(c: Cmd) -> String {
    for b in crate::ui::bars() {
        if let Some(it) = b.items.iter().find(|it| crate::ui::bind::cmd(it.id) == Some(c) && !crate::ui::label(it).is_empty()) {
            return crate::ui::label(it).to_string();
        }
    }
    match c {
        Cmd::Tool(t) => crate::tools::info(t).name.to_string(),
        Cmd::Docker(d) => crate::dockers::title(d),
        Cmd::BitmapEffect(i) => crate::fx::FX.get(i as usize).map(|f| f.name.to_string()).unwrap_or_default(),
        Cmd::DuplicateInPlace => "Duplicate in Place".into(),
        Cmd::TogglePickState => "Toggle Pick State".into(),
        Cmd::JoinCurves => "Join Curves".into(),
        Cmd::NextPage => "Next Page".into(),
        Cmd::PrevPage => "Previous Page".into(),
        // the Transform docker's tabs
        Cmd::TransformTab(i) => ["Position", "Rotate", "Scale and Mirror", "Size", "Skew"].get(i as usize).copied().unwrap_or("Transformations").into(),
        // the Align and Distribute docker's buttons
        Cmd::Distribute(d) => {
            use vector_core::ops::Distribute as Di;
            match d {
                Di::Left => "Distribute left",
                Di::CenterH => "Distribute centers horizontally",
                Di::Right => "Distribute right",
                Di::SpacingH => "Distribute spacing horizontally",
                Di::Top => "Distribute top",
                Di::CenterV => "Distribute centers vertically",
                Di::Bottom => "Distribute bottom",
                Di::SpacingV => "Distribute spacing vertically",
            }
            .into()
        }
        _ => crate::scripting::cmd_to_text(&c),
    }
}

/// A command's caption (without the `&`), for the conflict line.
pub fn label(c: Cmd) -> String {
    categories().iter().flat_map(|(_, es)| es.iter()).find(|e| e.cmd == c).map(|e| crate::widgets::plain(&e.label)).unwrap_or_else(|| other_label(c))
}

/// What the capture box took this frame.
enum Captured {
    Chord(Shortcut),
    Cancel,
}

/// The chord pressed this frame, for the capture box. Its key events are taken, so the dialog
/// does not act on them (Space or Enter pressing the box, Tab moving the focus away). Escape
/// cancels; Tab, Shift+Tab and Enter alone are left to the dialog; Shift, Ctrl and Alt alone are
/// no key. eframe turns Ctrl+C/X/V (and Shift+Delete, Ctrl+Insert, Shift+Insert) into clipboard
/// events: they are read back as those chords.
fn capture(ctx: &egui::Context) -> Option<Captured> {
    use egui::{Event, Key};
    ctx.input_mut(|i| {
        let m = i.modifiers;
        let mut out = None;
        i.events.retain(|e| {
            if out.is_some() {
                return true;
            }
            let chord = |key: Key, ctrl: bool, shift: bool| Captured::Chord(Shortcut { ctrl, shift, alt: m.alt, key, keypad: false });
            match e {
                Event::Key { key: Key::Escape, pressed: true, modifiers, .. } if !modifiers.any() => out = Some(Captured::Cancel),
                Event::Key { key: Key::Tab | Key::Enter, modifiers, .. } if !modifiers.ctrl && !modifiers.command && !modifiers.alt => return true,
                Event::Key { key, physical_key, pressed: true, modifiers, .. } => {
                    let keypad = *key == Key::Plus && *physical_key == Some(Key::Plus);
                    out = Some(Captured::Chord(Shortcut { ctrl: modifiers.ctrl || modifiers.command, shift: modifiers.shift, alt: modifiers.alt, key: *key, keypad }));
                }
                Event::Cut if m.shift && !m.ctrl => out = Some(chord(Key::Delete, false, true)),
                Event::Cut => out = Some(chord(Key::X, true, m.shift)),
                Event::Copy if m.ctrl && !m.shift && i.keys_down.contains(&Key::Insert) => out = Some(chord(Key::Insert, true, false)),
                Event::Copy => out = Some(chord(Key::C, true, m.shift)),
                Event::Paste(_) | Event::PasteImage(_) if m.shift && !m.ctrl => out = Some(chord(Key::Insert, false, true)),
                Event::Paste(_) | Event::PasteImage(_) => out = Some(chord(Key::V, true, m.shift)),
                _ => return true,
            }
            false
        });
        out
    })
}

/// Give `cmd` the chord `new` (text) in the user's settings. With `delete_conflicts`, the other
/// commands that `sc` runs lose it (they keep their other chords).
pub fn assign(app: &mut AnyVectorApp, cmd: Cmd, sc: &Shortcut, delete_conflicts: bool) {
    let new = format_shortcut(sc);
    if delete_conflicts {
        for c in crate::shortcuts::chord_owners(app, sc, cmd) {
            let rest: Vec<String> = crate::shortcuts::chords(app, c).into_iter().filter(|t| crate::commands::parse_shortcut(t).as_ref() != Some(sc)).collect();
            set(app, c, rest.into_iter().next().unwrap_or_default());
        }
    }
    set(app, cmd, new);
}

/// The user's chord for `cmd` ("" = none).
fn set(app: &mut AnyVectorApp, cmd: Cmd, chord: String) {
    let name = crate::scripting::cmd_to_text(&cmd);
    app.settings.shortcuts.retain(|(c, _)| *c != name);
    app.settings.shortcuts.push((name, chord));
}

/// The Commands page.
pub fn page(app: &mut AnyVectorApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let cats = categories();
    let names: Vec<&str> = cats.iter().map(|c| c.0.as_str()).collect();
    let mut mi = app.dialogs.options.commands_menu.min(names.len() - 1);
    ui.horizontal(|ui| {
        if combo(ui, "cmd_menu", &mut mi, &names, 150.0) {
            app.dialogs.options.commands_sel = None;
            app.dialogs.options.capture = None;
        }
        ui.add(egui::TextEdit::singleline(&mut app.dialogs.options.commands_search).hint_text("Search").desired_width(180.0));
    });
    app.dialogs.options.commands_menu = mi;
    let q = app.dialogs.options.commands_search.trim().to_lowercase();
    // a search looks through every category, by caption or by shortcut key ("ctrl+g")
    let mut entries: Vec<(&CatalogEntry, &str)> = vec![];
    for (ci, (cat, es)) in cats.iter().enumerate() {
        if !q.is_empty() || ci == mi {
            for e in es {
                if !entries.iter().any(|(x, _)| x.cmd == e.cmd) {
                    entries.push((e, cat.as_str()));
                }
            }
        }
    }
    let keys: Vec<Vec<String>> = entries.iter().map(|(e, _)| crate::shortcuts::chords(app, e.cmd)).collect();
    ui.horizontal_top(|ui| {
        // the list: one command per row
        egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP())).inner_margin(4.0).show(ui, |ui| {
            ui.set_width(280.0);
            ui.set_height(470.0);
            egui::ScrollArea::vertical().id_salt("cmd_list").max_height(470.0).auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(270.0);
                ui.spacing_mut().item_spacing.y = 1.0;
                // the page is laid out in a row; the list goes down
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    for ((e, _), k) in entries.iter().zip(&keys) {
                        let plain = crate::widgets::plain(&e.label);
                        if !q.is_empty() && !crate::i18n::t(&plain).to_lowercase().contains(&q) && !plain.to_lowercase().contains(&q) && !k.iter().any(|s| s.to_lowercase().contains(&q)) {
                            continue;
                        }
                        let sel = app.dialogs.options.commands_sel == Some(e.cmd);
                        let img = e.icon.as_deref().and_then(|i| app.icons.image(&ctx, i, 16.0, true));
                        let b = match img {
                            Some(img) => egui::Button::selectable(sel, (img, plain)),
                            None => egui::Button::selectable(sel, format!("      {plain}")),
                        };
                        // the shortcut key, right-aligned
                        let b = b.shortcut_text(egui::RichText::new(k.first().cloned().unwrap_or_default()).color(theme::TEXT_DIM()));
                        // dragged onto a toolbar, the command goes there (#124)
                        if crate::customize::drag_cmd(&ctx, ui.add(b.frame_when_inactive(sel).min_size(vec2(266.0, 20.0))), e.cmd).clicked() {
                            app.dialogs.options.commands_sel = Some(e.cmd);
                            app.dialogs.options.capture = None;
                        }
                    }
                });
            });
        });
        ui.vertical(|ui| {
            let mut tab = app.dialogs.options.commands_tab;
            tabs(ui, &mut tab, &["General", "Shortcut Keys"]);
            app.dialogs.options.commands_tab = tab;
            let Some(cmd) = app.dialogs.options.commands_sel else {
                note(ui, "Select a command.");
                return;
            };
            let found = cats.iter().find_map(|(cat, es)| es.iter().find(|e| e.cmd == cmd).map(|e| (e, cat.as_str())));
            let label = found.map(|(e, _)| crate::widgets::plain(&e.label)).unwrap_or_else(|| label(cmd));
            if tab == 0 {
                general(app, ui, &ctx, found, &label);
            } else {
                shortcut_keys(app, ui, &ctx, cmd);
            }
        });
    });
}

fn general(app: &mut AnyVectorApp, ui: &mut Ui, ctx: &egui::Context, found: Option<(&CatalogEntry, &str)>, label: &str) {
    ui.horizontal(|ui| {
        label_w(ui, "Caption:", 70.0);
        let mut l = crate::i18n::t(label);
        ui.add_enabled(false, egui::TextEdit::singleline(&mut l).desired_width(200.0));
    });
    ui.horizontal(|ui| {
        label_w(ui, "Icon:", 70.0);
        if let Some(img) = found.and_then(|(e, _)| e.icon.clone()).and_then(|i| app.icons.image(ctx, &i, 32.0, true)) {
            ui.add(img);
        }
    });
    ui.horizontal(|ui| {
        label_w(ui, "Tooltip:", 70.0);
        let mut t = crate::i18n::t(label).trim_end_matches("...").to_string();
        ui.add_enabled(false, egui::TextEdit::singleline(&mut t).desired_width(200.0));
    });
    if let Some((_, cat)) = found {
        ui.label(format!("Available on: {cat}"));
    }
}

fn shortcut_keys(app: &mut AnyVectorApp, ui: &mut Ui, ctx: &egui::Context, cmd: Cmd) {
    // the keys pressed for the capture box, before the box itself sees them
    if app.dialogs.options.capture.is_some() {
        match capture(ctx) {
            Some(Captured::Chord(sc)) => app.dialogs.options.capture = Some(format_shortcut(&sc)),
            Some(Captured::Cancel) => app.dialogs.options.capture = None,
            None => {}
        }
    }
    let cur = crate::shortcuts::chords(app, cmd);
    label_w(ui, "Current shortcut keys:", 150.0);
    egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP())).inner_margin(4.0).show(ui, |ui| {
        ui.set_width(220.0);
        ui.set_min_height(44.0);
        if cur.is_empty() {
            ui.weak("(none)");
        }
        for k in &cur {
            ui.label(k);
        }
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        label_w(ui, "New shortcut key:", 150.0);
    });
    let capturing = app.dialogs.options.capture.is_some();
    let text = app.dialogs.options.capture.clone().unwrap_or_default();
    let caption = if !text.is_empty() {
        text.clone()
    } else if capturing {
        crate::i18n::t("Press keys...")
    } else {
        crate::i18n::t("Click, then press keys")
    };
    let r = ui.add(egui::Button::new(caption).min_size(vec2(220.0, 22.0)).selected(capturing));
    if r.clicked() {
        app.dialogs.options.capture = Some(String::new());
    }
    let new = crate::commands::parse_shortcut(&text);
    let delete_id = egui::Id::unique("cmd_delete_conflicts");
    let mut delete_conflicts = ctx.data(|d| d.get_temp::<bool>(delete_id)).unwrap_or(true);
    if let Some(sc) = &new {
        let owners = crate::shortcuts::chord_owners(app, sc, cmd);
        if !owners.is_empty() {
            let names: Vec<String> = owners.iter().map(|c| label(*c)).collect();
            note(ui, &format!("Conflicts with: {}", names.join(", ")));
        }
    }
    if ui.checkbox(&mut delete_conflicts, "Delete conflicts").changed() {
        ctx.data_mut(|d| d.insert_temp(delete_id, delete_conflicts));
    }
    ui.horizontal(|ui| {
        if ui.add_enabled(new.is_some(), egui::Button::new("Assign")).clicked() {
            if let Some(sc) = &new {
                assign(app, cmd, sc, delete_conflicts);
            }
            app.dialogs.options.capture = None;
        }
        if ui.add_enabled(!cur.is_empty(), egui::Button::new("Delete")).clicked() {
            set(app, cmd, String::new());
        }
        let name = crate::scripting::cmd_to_text(&cmd);
        if ui.add_enabled(app.settings.shortcuts.iter().any(|(c, _)| *c == name), egui::Button::new("Reset")).clicked() {
            app.settings.shortcuts.retain(|(c, _)| *c != name);
        }
        if ui.add_enabled(!app.settings.shortcuts.is_empty(), egui::Button::new("Reset All")).clicked() {
            app.settings.shortcuts.clear();
        }
    });
}
