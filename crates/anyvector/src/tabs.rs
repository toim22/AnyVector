//! Document tab strip: Home (Welcome screen) · open documents · + (new document).

use crate::app::AnyVectorApp;
use crate::commands::Cmd;
use crate::icons::Tone;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};

pub const HEIGHT: f32 = 28.0;

pub fn tab_strip(app: &mut AnyVectorApp, ui: &mut Ui) {
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEIGHT), Sense::hover());
    let p = ui.painter().clone();
    p.line_segment([pos2(strip.left(), strip.bottom() - 0.5), pos2(strip.right(), strip.bottom() - 0.5)], Stroke::new(1.0, theme::SEP()));
    let mut x = strip.left() + 4.0;
    let cy = strip.center().y;
    // the Welcome Screen: a house and its name, blue while it is the active tab, and an X that
    // closes it like a document tab
    if app.home_open {
        let font = theme::body(12.5);
        let label = p.layout_no_wrap("Welcome Screen".to_string(), font, theme::TEXT());
        let hr = Rect::from_min_size(pos2(x, strip.top() + 3.0), vec2(32.0 + label.size().x + 30.0, HEIGHT - 3.0));
        let h = ui.interact(hr, ui.scope_id().with("tab_home"), Sense::click());
        let home = app.active.is_none();
        let bg = if home {
            theme::ACCENT()
        } else if h.hovered() {
            theme::HOVER()
        } else {
            Color32::TRANSPARENT
        };
        p.rect_filled(hr, egui::CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, bg);
        if let Some(t) = app.icons.get(ui.ctx(), "home", 16.0, if home { Tone::Bright } else { Tone::Normal }) {
            p.image(t.id, Rect::from_center_size(pos2(hr.left() + 16.0, hr.center().y), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let text_pos = pos2(hr.left() + 32.0, hr.center().y - label.size().y / 2.0);
        p.galley_with_override_text_color(text_pos, label, if home { theme::STRONG() } else { theme::TEXT() });
        let cr = Rect::from_center_size(pos2(hr.right() - 14.0, hr.center().y), vec2(16.0, 16.0));
        let c = ui.interact(cr, ui.scope_id().with("tab_home_x"), Sense::click()).on_hover_text("Close");
        if c.hovered() {
            p.rect_filled(cr, 2.0, if home { theme::FOCUS() } else { theme::PRESSED() });
        }
        if home || h.hovered() || c.hovered() {
            let s = 3.5;
            let col = if home || c.hovered() { theme::STRONG() } else { theme::TEXT_DIM() };
            p.line_segment([cr.center() + vec2(-s, -s), cr.center() + vec2(s, s)], Stroke::new(1.2, col));
            p.line_segment([cr.center() + vec2(s, -s), cr.center() + vec2(-s, s)], Stroke::new(1.2, col));
        }
        if c.clicked() || h.middle_clicked() {
            close_home(app);
        } else if h.clicked() {
            app.finish_tool();
            app.active = None;
        }
        x += hr.width() + 4.0;
    }
    // documents
    let mut close: Option<usize> = None;
    let mut activate: Option<usize> = None;
    let font = theme::body(12.5);
    let n = app.docs.len();
    for i in 0..n {
        let name = app.docs[i].display_name();
        let tw = p.layout_no_wrap(name.clone(), font.clone(), theme::TEXT()).size().x;
        let w = (tw + 44.0).clamp(90.0, 260.0);
        if x + w > strip.right() - 30.0 {
            break;
        }
        let r = Rect::from_min_size(pos2(x, strip.top() + 3.0), vec2(w, HEIGHT - 3.0));
        let resp = ui.interact(r, ui.scope_id().with(("tab", app.docs[i].uid)), Sense::click());
        let active = app.active == Some(i);
        let bg = if active {
            theme::BG_CANVAS_TAB()
        } else if resp.hovered() {
            theme::HOVER()
        } else {
            Color32::TRANSPARENT
        };
        p.rect_filled(r, egui::CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, bg);
        if active {
            p.line_segment([r.left_top() + vec2(2.0, 0.5), r.right_top() + vec2(-2.0, 0.5)], Stroke::new(2.0, theme::FOCUS()));
        }
        p.text(pos2(r.left() + 10.0, r.center().y), egui::Align2::LEFT_CENTER, &name, font.clone(), if active { theme::STRONG() } else { theme::TEXT() });
        let cr = Rect::from_center_size(pos2(r.right() - 14.0, r.center().y), vec2(16.0, 16.0));
        let c = ui.interact(cr, ui.scope_id().with(("tabx", app.docs[i].uid)), Sense::click()).on_hover_text("Close");
        if c.hovered() {
            p.rect_filled(cr, 2.0, theme::PRESSED());
        }
        if active || resp.hovered() || c.hovered() {
            let s = 3.5;
            let col = if c.hovered() { theme::STRONG() } else { theme::TEXT_DIM() };
            p.line_segment([cr.center() + vec2(-s, -s), cr.center() + vec2(s, s)], Stroke::new(1.2, col));
            p.line_segment([cr.center() + vec2(s, -s), cr.center() + vec2(-s, s)], Stroke::new(1.2, col));
        }
        if c.clicked() {
            close = Some(i);
        } else if resp.clicked() {
            activate = Some(i);
        }
        if resp.middle_clicked() {
            close = Some(i);
        }
        let path = app.docs[i].path.clone();
        resp.on_hover_text(path.map(|p| p.display().to_string()).unwrap_or(name));
        x += w + 2.0;
    }
    // +
    let pr = Rect::from_center_size(pos2(x + 12.0, cy), vec2(24.0, 22.0));
    let pl = ui.interact(pr, ui.scope_id().with("tab_plus"), Sense::click()).on_hover_text("New Document (Ctrl+N)");
    if pl.hovered() {
        p.rect_filled(pr, 2.0, theme::HOVER());
    }
    if let Some(t) = app.icons.get(ui.ctx(), "plus", 16.0, Tone::Normal) {
        p.image(t.id, Rect::from_center_size(pr.center(), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    if pl.clicked() {
        app.run(Cmd::New);
    }
    if let Some(i) = activate {
        app.finish_tool();
        app.active = Some(i);
    }
    if let Some(i) = close {
        app.request_close_doc(i);
    }
}

/// Close the Welcome Screen tab; the first document becomes the active tab.
pub fn close_home(app: &mut AnyVectorApp) {
    app.home_open = false;
    if app.active.is_none() && !app.docs.is_empty() {
        app.active = Some(0);
    }
}
