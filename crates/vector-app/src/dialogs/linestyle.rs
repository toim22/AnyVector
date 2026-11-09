//! Properties ▸ Outline ▸ Edit Line Style: a line style is a row of squares (dots and spaces,
//! each one outline width long) whose length is set with the marker above them.

use super::{button, window};
use crate::app::VectorApp;
use crate::propbar::edit_sel;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};

/// The longest line style, in squares.
const SLOTS: usize = 50;

#[derive(Clone, Debug)]
pub struct LineStyleState {
    /// The squares (true: a dot).
    pub dots: [bool; SLOTS],
    /// The length of the style, in squares (2..=50).
    pub len: usize,
    /// The custom line style being edited (Replace), when the outline had one.
    pub original: Option<Vec<f64>>,
    pub hint: bool,
}

impl LineStyleState {
    /// The editor of the line style `dash` (multiples of the outline width; empty: solid).
    pub fn from_dash(dash: &[f64]) -> Self {
        let mut dots = [false; SLOTS];
        let mut len = 0;
        for (i, l) in dash.iter().enumerate() {
            let n = (l.round() as usize).max(1);
            for _ in 0..n {
                if len < SLOTS {
                    dots[len] = i % 2 == 0;
                    len += 1;
                }
            }
        }
        if len < 2 {
            dots = [false; SLOTS];
            dots[0] = true;
            len = 2;
        }
        dots[0] = true;
        dots[len - 1] = false;
        LineStyleState { dots, len, original: Some(dash.to_vec()).filter(|d| !d.is_empty()), hint: false }
    }

    /// The dashes: the lengths of the runs of dots and spaces.
    pub fn dash(&self) -> Vec<f64> {
        let mut out: Vec<f64> = vec![];
        let mut cur = true;
        let mut run = 0.0;
        for &d in &self.dots[..self.len] {
            if d == cur {
                run += 1.0;
            } else {
                out.push(run);
                cur = d;
                run = 1.0;
            }
        }
        out.push(run);
        if out.len() % 2 == 1 {
            // a style ends with a space
            out.push(1.0);
        }
        out
    }
}

/// The squares: the ruler with the length marker above them, the locks of the first dot and
/// the last space under them.
fn adjust(ui: &mut Ui, st: &mut LineStyleState) {
    let cell = 12.0;
    let w = SLOTS as f32 * cell;
    let (r, resp) = ui.allocate_exact_size(vec2(w + 2.0, 44.0), Sense::click_and_drag());
    let p = ui.painter();
    let top = r.top() + 14.0;
    let left = r.left() + 1.0;
    // the ruler
    for i in 0..=SLOTS {
        let x = left + i as f32 * cell;
        let h = if i % 5 == 0 { 6.0 } else { 3.0 };
        p.line_segment([pos2(x, top - h - 1.0), pos2(x, top - 1.0)], Stroke::new(1.0, theme::TEXT_DIM));
    }
    // the squares
    for i in 0..SLOTS {
        let sq = Rect::from_min_size(pos2(left + i as f32 * cell, top), vec2(cell, cell));
        let c = if i >= st.len {
            Color32::from_gray(0x9a)
        } else if st.dots[i] {
            Color32::from_gray(0xf0)
        } else {
            Color32::from_gray(0x5a)
        };
        p.rect_filled(sq.shrink(0.5), 0.0, c);
        p.rect_stroke(sq, 0.0, Stroke::new(1.0, Color32::from_gray(0x70)), egui::StrokeKind::Inside);
    }
    // the length marker (an hourglass on the ruler) and the locks
    let mx = left + st.len as f32 * cell;
    let m = pos2(mx, top - 8.0);
    p.add(egui::Shape::convex_polygon(vec![pos2(m.x - 4.0, m.y - 5.0), pos2(m.x + 4.0, m.y - 5.0), pos2(m.x, m.y)], theme::TEXT, Stroke::NONE));
    p.add(egui::Shape::convex_polygon(vec![pos2(m.x, m.y), pos2(m.x + 4.0, m.y + 5.0), pos2(m.x - 4.0, m.y + 5.0)], theme::TEXT, Stroke::NONE));
    for i in [0, st.len - 1] {
        let c = pos2(left + (i as f32 + 0.5) * cell, top + cell + 6.0);
        p.rect_filled(Rect::from_center_size(c + vec2(0.0, 1.5), vec2(7.0, 5.0)), 1.0, theme::TEXT);
        p.circle_stroke(c + vec2(0.0, -1.5), 2.2, Stroke::new(1.0, theme::TEXT));
    }
    // dragging on the ruler sets the length; a click on a square turns it on or off
    if let Some(pos) = resp.interact_pointer_pos() {
        let slot = ((pos.x - left) / cell).floor() as isize;
        if pos.y < top && (resp.dragged() || resp.clicked()) {
            let n = (((pos.x - left) / cell).round() as usize).clamp(2, SLOTS);
            if n != st.len {
                if n > st.len {
                    for d in st.dots[st.len - 1..n].iter_mut() {
                        *d = false;
                    }
                }
                st.len = n;
                st.dots[n - 1] = false;
            }
        } else if resp.clicked() && slot >= 0 && (slot as usize) < SLOTS {
            let i = slot as usize;
            if i >= st.len {
                // a square past the end makes the style longer
                for d in st.dots[st.len - 1..=i].iter_mut() {
                    *d = false;
                }
                st.dots[i] = true;
                st.len = (i + 2).min(SLOTS);
                st.dots[st.len - 1] = false;
            } else if i != 0 && i != st.len - 1 {
                st.dots[i] = !st.dots[i];
            }
        }
    }
}

fn preview(ui: &mut Ui, st: &LineStyleState) {
    let w = ui.available_width() - 22.0;
    let (r, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
    let p = ui.painter();
    p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_gray(0x80)), egui::StrokeKind::Inside);
    let line = r.shrink2(vec2(8.0, 0.0));
    let unit = 4.0;
    let y = line.center().y;
    let mut x = line.left();
    'outer: loop {
        for &d in &st.dots[..st.len] {
            if x + unit > line.right() {
                break 'outer;
            }
            if d {
                p.rect_filled(Rect::from_min_size(pos2(x, y - unit / 2.0), vec2(unit, unit)), 0.0, Color32::from_gray(0xf0));
            }
            x += unit + 0.0;
        }
    }
}

pub fn show(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.line_style.take() else { return };
    let editable = st.original.as_ref().map(|o| app.settings.custom_dashes.contains(o)).unwrap_or(false);
    let (mut add, mut replace, mut cancel) = (false, false, false);
    let ((), dismissed) = window(ctx, "edit_line_style", "Edit Line Style", vec2(678.0, 288.0), |ui| {
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(23.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Adjust").strong().color(theme::TEXT));
                adjust(ui, &mut st);
                ui.label(egui::RichText::new("Click on the squares to turn dots in the line style on and off.").color(theme::TEXT));
                ui.add_space(14.0);
                ui.label(egui::RichText::new("Preview").strong().color(theme::TEXT));
                preview(ui, &st);
                ui.add_space(14.0);
                let (hr, hresp) = ui.allocate_exact_size(vec2(60.0, 18.0), Sense::click());
                let c = pos2(hr.left() + 7.0, hr.center().y);
                if st.hint {
                    crate::widgets::chevron_up(ui.painter(), c, 3.0, theme::TEXT);
                } else {
                    crate::widgets::chevron(ui.painter(), c, 3.0, theme::TEXT);
                }
                ui.painter().text(pos2(hr.left() + 17.0, hr.center().y), egui::Align2::LEFT_CENTER, "Hint", egui::FontId::proportional(12.5), theme::TEXT);
                if hresp.clicked() {
                    st.hint = !st.hint;
                }
                if st.hint {
                    ui.label(egui::RichText::new("Drag the marker above the squares to set the length of the line style. A line style starts with a dot and ends with a space. Add makes a new line style; Replace changes the one you are editing.").color(theme::TEXT_DIM));
                }
            });
        });
        let rect = Rect::from_min_max(pos2(ui.max_rect().left(), ui.max_rect().bottom() - 30.0), ui.max_rect().right_bottom());
        ui.painter().hline(rect.x_range(), rect.top() - 4.0, Stroke::new(1.0, theme::SEP));
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Button::new("?").min_size(vec2(24.0, 24.0)).stroke(Stroke::new(1.0, theme::ACCENT))).on_hover_text("Help");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    cancel = button(ui, "Cancel", true);
                    ui.add_space(4.0);
                    add = button(ui, "Add", true);
                    ui.add_space(4.0);
                    replace = button(ui, "Replace", editable);
                });
            });
        });
    });
    if add || replace {
        let d = st.dash();
        if replace {
            if let Some(o) = &st.original {
                if let Some(slot) = app.settings.custom_dashes.iter_mut().find(|x| *x == o) {
                    *slot = d.clone();
                }
            }
        } else if !app.settings.custom_dashes.contains(&d) {
            app.settings.custom_dashes.push(d.clone());
        }
        edit_sel(app, "Line Style", move |x| x.style.outline.dash = d.clone());
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.line_style = Some(st);
    }
}
