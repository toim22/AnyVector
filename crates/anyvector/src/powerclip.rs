//! PowerClip on the canvas (#111, #112): the floating PowerClip toolbar under a selected
//! PowerClip, as CorelDRAW shows it (Edit PowerClip, Select Contents, Extract Contents, Lock
//! Contents to PowerClip and, behind its arrow, where the contents sit in the frame), the one
//! under a PowerClip being edited (Finish Editing PowerClip, Select Contents), and the frame
//! outline drawn while its contents are edited.
//!
//! The buttons are the items of the reference PowerClip bar, so they share their captions, icons,
//! enabled and checked states with the Object ▸ PowerClip menu.

use crate::app::AnyVectorApp;
use crate::commands::Cmd;
use crate::docview::DocView;
use crate::theme;
use crate::tools::ToolId;
use crate::ui::{self, Item};
use egui::{Align2, Context, Margin, Painter, Popup, Stroke};
use vector_core::{ObjId, ObjKind};

/// The reference PowerClip bar (Object ▸ PowerClip).
const BAR: &str = "5f815cd7-d3f8-453e-8e4c-58d06a2b4f9d";
/// Edit PowerClip, Extract Contents, Lock Contents to PowerClip, Finish Editing PowerClip.
const EDIT: &str = "cd3ae51f";
const EXTRACT: &str = "6ad732d1";
const LOCK: &str = "6ea9b15a";
const FINISH: &str = "22eabb92";
/// Center, Fit Proportionally, Fill Proportionally, Stretch to Fill: behind the toolbar's arrow.
const PLACE: [&str; 4] = ["273d95de", "a369e3ae", "56a4aae7", "8ab3326a"];
/// The toolbar's own button: the contents selected, the PowerClip left closed.
pub const SELECT_CONTENTS: &str = "Select Contents";
/// The tooltip of the arrow that opens the placement menu.
pub const ADJUST_CONTENTS: &str = "Adjust contents";
/// Room between the selection handles and the toolbar, in screen points.
const GAP: f32 = 18.0;

/// The item of the PowerClip bar whose id starts with `key`.
fn item(key: &str) -> Option<&'static Item> {
    ui::bar_named(BAR)?.1.items.iter().find(|it| it.id.get(..8).is_some_and(|k| k.eq_ignore_ascii_case(key)))
}

/// The PowerClip the floating toolbar is for, and whether it is being edited: the one being
/// edited, else the one selected PowerClip.
pub fn toolbar_target(d: &DocView) -> Option<(ObjId, bool)> {
    if let Some(pc) = d.editing_powerclip() {
        return Some((pc, true));
    }
    match d.selection.as_slice() {
        [id] if d.doc.find(*id).is_some_and(|o| matches!(o.kind, ObjKind::PowerClip { .. })) => Some((*id, false)),
        _ => None,
    }
}

/// Whether the floating toolbar shows now: the Pick tool, a PowerClip selected or being edited,
/// nothing being dragged or typed, no modal dialog.
fn toolbar_shown(app: &AnyVectorApp) -> Option<(usize, ObjId, bool)> {
    if app.tool != ToolId::Pick || app.ts.is_busy() || app.ts.press.is_some() || app.fullscreen_preview || app.dialogs.any_modal_open() {
        return None;
    }
    let ai = app.active.filter(|a| *a < app.docs.len())?;
    let d = &app.docs[ai];
    if d.show_page_sorter || d.text_edit.is_some() {
        return None;
    }
    let (pc, editing) = toolbar_target(d)?;
    Some((ai, pc, editing))
}

/// The floating PowerClip toolbar: centred under the PowerClip (above it when there is no room
/// below), inside the drawing window.
pub fn floating_toolbar(app: &mut AnyVectorApp, ctx: &Context) {
    let Some((ai, pc, editing)) = toolbar_shown(app) else { return };
    let d = &app.docs[ai];
    let canvas = d.canvas_rect;
    let Some(o) = d.doc.find(pc) else { return };
    let r = d.to_screen_rect(d.doc.bounds_of(o));
    if !r.intersects(canvas) {
        return;
    }
    let s = app.settings.button_size.icon();
    let h = s + 14.0;
    let below = r.bottom() + GAP + h <= canvas.bottom();
    let (pos, pivot) = if below { (egui::pos2(r.center().x, r.bottom() + GAP), Align2::CENTER_TOP) } else { (egui::pos2(r.center().x, r.top() - GAP), Align2::CENTER_BOTTOM) };
    let frame = egui::Frame::window(&ctx.global_style()).fill(theme::BAR()).inner_margin(Margin::symmetric(4, 2)).corner_radius(3);
    egui::Area::new(egui::Id::unique(("powerclip_toolbar", d.uid)))
        .order(egui::Order::Middle)
        .fixed_pos(pos)
        .pivot(pivot)
        .constrain_to(canvas)
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 1.0;
                    if editing {
                        bar_button(app, ui, FINISH);
                        select_contents(app, ui);
                    } else {
                        bar_button(app, ui, EDIT);
                        select_contents(app, ui);
                        bar_button(app, ui, EXTRACT);
                        bar_button(app, ui, LOCK);
                        place_menu(app, ui);
                    }
                });
            });
        });
}

/// A button of the PowerClip bar, run as the menu runs it.
fn bar_button(app: &mut AnyVectorApp, ui: &mut egui::Ui, key: &str) {
    let Some(it) = item(key) else { return };
    let s = app.settings.button_size.icon();
    let on = crate::cmdbar::enabled(app, it, ui::context_bit(app));
    let checked = ui::bind::checked(app, it.id).unwrap_or(false);
    let tip = crate::cmdbar::tooltip(app, it);
    let r = crate::widgets::icon_button_sized(&mut app.icons, ui, it.icon, &tip, on, checked, s + 10.0, s);
    if it.kind == ui::Kind::Button {
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, on, &tip));
    } else {
        r.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, on, checked, &tip));
    }
    if r.clicked() {
        crate::cmdbar::run(app, it);
    }
}

/// Select Contents: the contents selected, the PowerClip left closed (being edited, all of its
/// contents).
fn select_contents(app: &mut AnyVectorApp, ui: &mut egui::Ui) {
    let s = app.settings.button_size.icon();
    let on = app.doc().is_some_and(|d| !crate::exec::powerclip_contents(d).is_empty());
    let r = crate::widgets::icon_button_sized(&mut app.icons, ui, "powerclip_select", SELECT_CONTENTS, on, false, s + 10.0, s);
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, on, SELECT_CONTENTS));
    if r.clicked() {
        app.run(Cmd::PowerClipSelectContents);
    }
}

/// The arrow at the end of the toolbar: Center, Fit Proportionally, Fill Proportionally and
/// Stretch to Fill.
fn place_menu(app: &mut AnyVectorApp, ui: &mut egui::Ui) {
    let on = PLACE.iter().filter_map(|k| item(k)).any(|it| crate::cmdbar::enabled(app, it, ui::context_bit(app)));
    let r = crate::widgets::arrow_button(ui, on, ADJUST_CONTENTS);
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, on, ADJUST_CONTENTS));
    Popup::menu(&r).show(|ui| {
        if let Some((b, _)) = ui::bar_named(BAR) {
            crate::menus::show_flyout(app, ui, b, |it| !PLACE.iter().any(|k| it.id.get(..8).is_some_and(|x| x.eq_ignore_ascii_case(k))));
        }
    });
}

/// While the contents of a PowerClip are edited: its frame as a blue outline (the frame itself
/// is not drawn then, and the contents show in full), as in CorelDRAW.
pub fn draw_overlay(painter: &Painter, d: &DocView) {
    let Some(pc) = d.editing_powerclip() else { return };
    let Some(ObjKind::PowerClip { frame, .. }) = d.doc.find(pc).map(|o| &o.kind) else { return };
    crate::tools::overlay::stroke_bez(painter, d, &frame.world_path(), Stroke::new(1.0, theme::SELECTION));
}

/// The screen rectangle of the floating toolbar's PowerClip (tests place clicks with it).
#[cfg(test)]
pub fn target_rect(d: &DocView) -> Option<egui::Rect> {
    let (pc, _) = toolbar_target(d)?;
    Some(d.to_screen_rect(d.doc.bounds_of(d.doc.find(pc)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every button the floating toolbar draws is an item of the reference PowerClip bar.
    #[test]
    fn toolbar_items_exist() {
        for k in [EDIT, EXTRACT, LOCK, FINISH].iter().chain(PLACE.iter()) {
            let it = item(k).unwrap_or_else(|| panic!("no PowerClip bar item {k}"));
            assert!(!it.icon.is_empty(), "{k} has no icon");
            assert!(ui::bind::cmd(it.id).is_some(), "{k} runs no command");
        }
        assert!(crate::icons::body("powerclip_select").is_some());
    }
}
