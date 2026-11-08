//! Toolbox (left column) with flyouts and the Quick Customize popup.

use crate::app::VectorApp;
use crate::icons::Tone;
use crate::theme;
use crate::tools::{info, ToolId, GROUPS, SEPARATOR_AFTER};
use egui::{pos2, vec2, Color32, Popup, PopupCloseBehavior, Rect, Sense, Stroke, Ui};

pub fn toolbox(app: &mut VectorApp, ui: &mut Ui) {
    let doc = app.has_doc();
    ui.spacing_mut().item_spacing.y = 1.0;
    let w = 30.0;
    for (gi, (_title, tools)) in GROUPS.iter().enumerate() {
        let visible: Vec<ToolId> = tools.iter().copied().filter(|t| !app.settings.hidden_tools.contains(t)).collect();
        if visible.is_empty() {
            continue;
        }
        let mut current = app.group_tool[gi];
        if !visible.contains(&current) {
            current = visible[0];
        }
        let active = tools.contains(&app.tool) && doc;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), if doc { Sense::click_and_drag() } else { Sense::hover() });
        let p = ui.painter();
        if active {
            p.rect_filled(rect.shrink(1.0), 2.0, theme::PRESSED);
        } else if doc && resp.hovered() {
            p.rect_filled(rect.shrink(1.0), 2.0, theme::HOVER);
        }
        let tone = if !doc {
            Tone::Disabled
        } else if active {
            Tone::Bright
        } else {
            Tone::Normal
        };
        if let Some(t) = app.icons.get(ui.ctx(), info(current).icon, 20.0, tone) {
            p.image(t.id, Rect::from_center_size(rect.center() - vec2(1.0, 0.0), vec2(20.0, 20.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let has_flyout = tools.len() > 1 || gi == 5;
        if has_flyout {
            let c = if doc { Color32::from_gray(0xb0) } else { Color32::from_gray(0x55) };
            let br = rect.right_bottom() - vec2(3.0, 3.0);
            p.add(egui::Shape::convex_polygon(vec![br, br - vec2(5.0, 0.0), br - vec2(0.0, 5.0)], c, Stroke::NONE));
        }
        let tip = {
            let i = info(current);
            if i.shortcut.is_empty() {
                i.name.to_string()
            } else {
                format!("{} ({})", i.name, i.shortcut)
            }
        };
        let resp = resp.on_hover_text(tip);
        // flyout opens on click of the triangle area or on press-and-hold / drag
        let flyout_id = ui.make_persistent_id(("flyout", gi));
        let mut open_flyout = false;
        if resp.clicked() {
            let in_corner = resp.interact_pointer_pos().map(|p| p.x > rect.right() - 9.0 && p.y > rect.bottom() - 9.0).unwrap_or(false);
            if has_flyout && in_corner {
                open_flyout = true;
            } else {
                app.set_tool(current);
            }
        }
        if has_flyout && (resp.drag_started() || (resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.press_start_time().map(|t| i.time - t > 0.45).unwrap_or(false)))) {
            open_flyout = true;
        }
        Popup::new(flyout_id, ui.ctx().clone(), egui::PopupAnchor::Position(rect.right_top() + vec2(2.0, 0.0)), ui.layer_id())
            .open_memory(open_flyout.then_some(egui::SetOpenCommand::Bool(true)))
            .close_behavior(PopupCloseBehavior::CloseOnClick)
            .show(|ui| {
                ui.set_min_width(230.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                for t in &visible {
                    let i = info(*t);
                    let img = app.icons.image(ui.ctx(), i.icon, 18.0, true).unwrap();
                    let mut b = egui::Button::image_and_text(img, i.name).min_size(vec2(230.0, 26.0)).selected(app.tool == *t);
                    if !i.shortcut.is_empty() {
                        b = b.shortcut_text(i.shortcut);
                    }
                    if ui.add(b).clicked() {
                        app.set_tool(*t);
                    }
                }
            });
        if SEPARATOR_AFTER.contains(&gi) {
            let (r, _) = ui.allocate_exact_size(vec2(w, 9.0), Sense::hover());
            ui.painter().line_segment([pos2(r.left() + 4.0, r.center().y), pos2(r.right() - 4.0, r.center().y)], Stroke::new(1.0, theme::SEP));
        }
    }
    ui.add_space(4.0);
    // Quick Customize
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect.shrink(1.0), 2.0, theme::HOVER);
    }
    if let Some(t) = app.icons.get(ui.ctx(), "plus", 16.0, Tone::Normal) {
        ui.painter().image(t.id, Rect::from_center_size(rect.center(), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    let resp = resp.on_hover_text("Quick Customize");
    Popup::menu(&resp).close_behavior(PopupCloseBehavior::CloseOnClickOutside).align(egui::RectAlign::RIGHT_START).show(|ui| {
        quick_customize(app, ui);
    });
}

fn quick_customize(app: &mut VectorApp, ui: &mut Ui) {
    ui.set_min_width(240.0);
    egui::ScrollArea::vertical().max_height(ui.ctx().content_rect().height() - 120.0).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for (gi, (title, tools)) in GROUPS.iter().enumerate() {
            if gi > 0 {
                let r = ui.available_rect_before_wrap();
                ui.painter().line_segment([pos2(r.left(), r.top() + 2.0), pos2(r.right(), r.top() + 2.0)], Stroke::new(1.0, Color32::from_rgb(0x2a, 0x6a, 0xc8)));
                ui.add_space(5.0);
            }
            ui.label(egui::RichText::new(*title).color(theme::TEXT_DIM));
            for t in tools.iter() {
                let i = info(*t);
                let mut on = !app.settings.hidden_tools.contains(t);
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut on, "").changed() {
                        if on {
                            app.settings.hidden_tools.retain(|x| x != t);
                        } else {
                            app.settings.hidden_tools.push(*t);
                        }
                    }
                    if let Some(img) = app.icons.image(ui.ctx(), i.icon, 16.0, true) {
                        ui.add(img);
                    }
                    ui.label(i.name);
                    if !i.shortcut.is_empty() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(i.shortcut).color(theme::TEXT_DIM));
                        });
                    }
                });
            }
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Reset toolbar").clicked() {
            app.settings.hidden_tools.clear();
        }
        if ui.button("Customize...").clicked() {
            app.run(crate::commands::Cmd::Options(crate::commands::OptionsPage::Customization));
            app.dialogs.options.page = Some("Commands".into());
            ui.close();
        }
    });
}
