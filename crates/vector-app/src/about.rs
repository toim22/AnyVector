//! Help ▸ About Vector, laid out like the reference's About box, with its two child windows:
//! System Info (facts about the computer, one category at a time, savable as text) and
//! Legal Notices (the open-source components Vector is built with and their licenses).

use crate::app::VectorApp;
use crate::dialogs::window;
use crate::theme;
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, Ui, UiBuilder};

pub const SOURCE_ID: &str = env!("VECTOR_SOURCE_ID");
pub const RELEASE: &str = env!("VECTOR_RELEASE");
const LEGAL: &str = include_str!("../assets/legal/third-party-licenses.txt");

#[derive(Clone)]
pub struct SystemInfoState {
    pub category: usize,
    pub rows: crate::sysinfo::Rows,
}

impl SystemInfoState {
    pub fn new(gpu: &str) -> SystemInfoState {
        SystemInfoState { category: 0, rows: crate::sysinfo::collect(0, gpu) }
    }
}

fn logo(app: &mut VectorApp, ctx: &egui::Context) -> egui::TextureHandle {
    if let Some(t) = app.ts.texture_cache.get("about_logo") {
        return t.clone();
    }
    let img = image::load_from_memory(crate::LOGO_PNG).map(|i| i.to_rgba8()).unwrap_or_else(|_| image::RgbaImage::new(1, 1));
    let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
    let t = ctx.load_texture("about_logo", ci, egui::TextureOptions::LINEAR);
    app.ts.texture_cache.insert("about_logo".into(), t.clone());
    t
}

/// A button placed at an exact rectangle inside `ui`.
fn button_at(ui: &mut Ui, r: Rect, text: &str, default: bool) -> bool {
    ui.scope_builder(UiBuilder::new().max_rect(r), |ui| {
        let b = if default {
            egui::Button::new(egui::RichText::new(text).color(Color32::WHITE)).fill(theme::ACCENT)
        } else {
            egui::Button::new(text)
        };
        ui.add_sized(r.size(), b).clicked()
    })
    .inner
}

pub fn about(app: &mut VectorApp, ctx: &egui::Context) {
    let tex = logo(app, ctx);
    let mut close = false;
    let mut sysinfo = false;
    let mut legal = false;
    let ((), dismissed) = window(ctx, "about", &format!("About {}", vector_core::APP_NAME), vec2(579.0, 386.0), |ui| {
        let b = ui.max_rect();
        let p = ui.painter().clone();
        // banner: the name and the airship on black
        let banner = Rect::from_min_max(pos2(b.left() + 5.0, b.top() + 8.0), pos2(b.right() - 5.0, b.top() + 213.0));
        p.rect_filled(banner, 0.0, Color32::BLACK);
        p.rect_stroke(banner, 0.0, Stroke::new(1.0, Color32::from_gray(0x4a)), egui::StrokeKind::Inside);
        let font = theme::light(66.0);
        let name = p.layout_no_wrap(vector_core::APP_NAME.to_string(), font, Color32::WHITE);
        let sz = tex.size_vec2();
        let lw = 150.0;
        let lh = lw * sz.y / sz.x;
        let gap = 18.0;
        let total = name.size().x + gap + lw;
        let x0 = banner.center().x - total / 2.0;
        p.galley(pos2(x0, banner.center().y - name.size().y / 2.0 - 4.0), name.clone(), Color32::WHITE);
        let lr = Rect::from_min_size(pos2(x0 + name.size().x + gap, banner.center().y - lh / 2.0 - 2.0), vec2(lw, lh));
        p.image(tex.id(), lr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        // version and copyright
        let f = egui::FontId::proportional(13.0);
        p.text(pos2(b.center().x, b.top() + 230.0), Align2::CENTER_CENTER, format!("Version {} - {RELEASE} Release", vector_core::APP_VERSION), f.clone(), theme::TEXT);
        p.text(pos2(b.center().x, b.top() + 253.0), Align2::CENTER_CENTER, "(C) 2026 AnyVector Team.  All rights reserved.", f.clone(), theme::TEXT);
        // Source ID box
        let sid = Rect::from_min_max(pos2(b.left() + 5.0, b.top() + 266.0), pos2(b.right() - 5.0, b.top() + 295.0));
        p.rect_stroke(sid, 0.0, Stroke::new(1.0, Color32::from_gray(0x5a)), egui::StrokeKind::Inside);
        p.text(pos2(sid.left() + 8.0, sid.center().y), Align2::LEFT_CENTER, "Source ID:", f.clone(), theme::TEXT);
        p.text(pos2(sid.left() + 84.0, sid.center().y), Align2::LEFT_CENTER, SOURCE_ID, f, theme::TEXT);
        // buttons
        let y = b.top() + 313.0;
        sysinfo = button_at(ui, Rect::from_min_size(pos2(b.left() + 5.0, y), vec2(118.0, 25.0)), "System Info...", false);
        legal = button_at(ui, Rect::from_min_size(pos2(b.left() + 135.0, y), vec2(130.0, 25.0)), "Legal Notices...", false);
        close = button_at(ui, Rect::from_min_size(pos2(b.right() - 85.0, y), vec2(82.0, 25.0)), "Close", true);
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            close = true;
        }
    });
    if sysinfo {
        app.dialogs.system_info = Some(SystemInfoState::new(&app.gpu));
    }
    if legal {
        app.dialogs.legal_notices = true;
    }
    if (close || dismissed) && app.dialogs.system_info.is_none() && !app.dialogs.legal_notices {
        app.dialogs.about = false;
    }
}

pub fn system_info(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.system_info.take() else { return };
    let mut close = false;
    let mut save = false;
    let ((), dismissed) = window(ctx, "system_info", "System Info", vec2(599.0, 411.0), |ui| {
        let b = ui.max_rect();
        ui.painter().text(pos2(b.left() + 5.0, b.top() + 20.0), Align2::LEFT_CENTER, "Choose a category:", egui::FontId::proportional(13.0), theme::TEXT);
        let combo = Rect::from_min_size(pos2(b.left() + 138.0, b.top() + 8.0), vec2(270.0, 24.0));
        let mut cat = st.category;
        ui.scope_builder(UiBuilder::new().max_rect(combo), |ui| {
            egui::ComboBox::from_id_salt("sysinfo_category").width(262.0).selected_text(crate::sysinfo::CATEGORIES[cat]).show_ui(ui, |ui| {
                for (i, c) in crate::sysinfo::CATEGORIES.iter().enumerate() {
                    ui.selectable_value(&mut cat, i, *c);
                }
            });
        });
        if cat != st.category {
            st.category = cat;
            st.rows = crate::sysinfo::collect(cat, &app.gpu);
        }
        // the list: name and value columns in a bordered, scrolling box
        let list = Rect::from_min_max(pos2(b.left() + 5.0, b.top() + 40.0), pos2(b.right() - 5.0, b.top() + 288.0));
        ui.painter().rect_stroke(list, 0.0, Stroke::new(1.0, Color32::from_gray(0x5a)), egui::StrokeKind::Inside);
        let inner = list.shrink2(vec2(6.0, 2.0));
        ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for (k, v) in &st.rows {
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 25.0), Sense::hover());
                    let f = egui::FontId::proportional(13.0);
                    ui.painter().text(pos2(r.left(), r.center().y), Align2::LEFT_CENTER, k, f.clone(), theme::TEXT);
                    let vr = Rect::from_min_max(pos2(r.left() + 171.0, r.top()), r.max);
                    ui.painter().with_clip_rect(vr).text(pos2(vr.left(), r.center().y), Align2::LEFT_CENTER, v, f, theme::TEXT);
                }
            });
        });
        let y = b.bottom() - 25.0;
        close = button_at(ui, Rect::from_min_size(pos2(b.left() + 133.0, y), vec2(80.0, 24.0)), "Close", true);
        save = button_at(ui, Rect::from_min_size(pos2(b.left() + 366.0, y), vec2(80.0, 24.0)), "Save", false);
    });
    if save {
        if let Some(p) = rfd::FileDialog::new().set_title("Save System Info").add_filter("Text (*.txt)", &["txt"]).set_file_name("System Info.txt").save_file() {
            if let Err(e) = std::fs::write(&p, crate::sysinfo::report(&app.gpu)) {
                app.dialogs.message_box = Some(("System Info".into(), format!("Could not save {}:\n{e}", p.display())));
            }
        }
    }
    if !(close || dismissed) {
        app.dialogs.system_info = Some(st);
    }
}

pub fn legal_notices(app: &mut VectorApp, ctx: &egui::Context) {
    let mut close = false;
    let ((), dismissed) = window(ctx, "legal_notices", "Legal Notices", vec2(599.0, 460.0), |ui| {
        let b = ui.max_rect();
        let text_rect = Rect::from_min_max(pos2(b.left() + 5.0, b.top() + 8.0), pos2(b.right() - 5.0, b.bottom() - 36.0));
        ui.painter().rect_stroke(text_rect, 0.0, Stroke::new(1.0, Color32::from_gray(0x5a)), egui::StrokeKind::Inside);
        ui.scope_builder(UiBuilder::new().max_rect(text_rect.shrink(6.0)), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.label(egui::RichText::new(LEGAL).font(egui::FontId::monospace(11.5)).color(theme::TEXT));
            });
        });
        close = button_at(ui, Rect::from_min_size(pos2(b.right() - 85.0, b.bottom() - 25.0), vec2(82.0, 24.0)), "Close", true);
    });
    if close || dismissed {
        app.dialogs.legal_notices = false;
    }
}

