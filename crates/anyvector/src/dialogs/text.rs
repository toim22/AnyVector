//! The Text menu dialogs: Tabs (with Leader Settings), Columns, Bullets and Numbering, Drop Cap, Text
//! Statistics, Custom Optional Hyphens, Writing Tools (Spell Checker, Grammatik and Thesaurus), Text
//! Language, Change Case, Text Encoding and Edit Text.

use super::{button, combo, group, ok_button, ok_cancel_plain, ok_cancel_row, separator, window};
use crate::app::AnyVectorApp;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect as ERect, Sense, Stroke, Ui};
use vector_core::text::{self as vt, DropCap, Leader, ListFormat, TabAlign, TabStop, TextKind, TextObj};
use vector_core::{Document, ObjId, ObjKind, Unit};

// ---------------------------------------------------------------------------------------------
// The texts the dialogs work on
// ---------------------------------------------------------------------------------------------

/// The first frame of a chain of linked frames.
pub fn chain_head(doc: &Document, mut id: ObjId) -> ObjId {
    for _ in 0..256 {
        match doc.find(id).map(|o| &o.kind) {
            Some(ObjKind::Text(t)) if t.link_prev.is_some() => id = t.link_prev.unwrap_or(id),
            _ => break,
        }
    }
    id
}

/// The text objects the Text dialogs work on: the text being edited, or the selected texts (inside groups
/// too).
pub fn target_ids(app: &AnyVectorApp) -> Vec<ObjId> {
    let Some(d) = app.doc() else { return vec![] };
    if let Some(te) = &d.text_edit {
        return vec![te.obj];
    }
    let mut out = vec![];
    for id in &d.selection {
        if let Some(o) = d.doc.find(*id) {
            o.walk(&mut |x| {
                if matches!(x.kind, ObjKind::Text(_)) {
                    out.push(x.id);
                }
            });
        }
    }
    out
}

/// The first target text, or the settings new text gets.
pub fn first_text(app: &AnyVectorApp) -> TextObj {
    let ids = target_ids(app);
    app.doc()
        .and_then(|d| ids.first().and_then(|id| d.doc.find(chain_head(&d.doc, *id))))
        .and_then(|o| match &o.kind {
            ObjKind::Text(t) => Some(t.clone()),
            _ => None,
        })
        .unwrap_or_else(|| app.ts.text_para.clone())
}

/// Change the target texts as one undo step; paragraph settings go to the first frame of linked frames
/// (`head`). With no text selected the change is for the text you create next.
pub fn modify_texts(app: &mut AnyVectorApp, label: &str, head: bool, f: impl Fn(&mut TextObj)) -> bool {
    let ids = target_ids(app);
    if ids.is_empty() {
        f(&mut app.ts.text_para);
        return false;
    }
    let Some(d) = app.doc_mut() else { return false };
    let mut ids: Vec<ObjId> = if head { ids.iter().map(|id| chain_head(&d.doc, *id)).collect() } else { ids };
    ids.dedup();
    d.modify(label, |doc, _| {
        for id in &ids {
            if let Some(o) = doc.find_mut(*id) {
                if let ObjKind::Text(t) = &mut o.kind {
                    f(t);
                }
            }
        }
    });
    true
}

/// Paragraph settings a new text takes from the Text dialogs used with nothing selected.
pub fn apply_defaults(t: &mut TextObj, d: &TextObj) {
    t.tabs = d.tabs.clone();
    t.leader = d.leader;
    t.list = d.list.clone();
    t.drop_cap = d.drop_cap;
    t.hyphenate = d.hyphenate;
    t.hyphenation = d.hyphenation;
    t.align_grid = d.align_grid;
    if matches!(t.kind, TextKind::Paragraph { .. }) {
        t.columns = d.columns;
        t.column_gap = d.column_gap;
    }
}

fn unit_of(app: &AnyVectorApp) -> (Unit, f64) {
    app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((Unit::Inches, 300.0))
}

fn unit_name(u: Unit) -> &'static str {
    if u == Unit::Inches {
        "in"
    } else {
        u.suffix()
    }
}

fn fmt_len(v: f64, u: Unit, dpi: f64) -> String {
    format!("{:.3} {}", v / u.points_per_unit(dpi), unit_name(u))
}

/// A length field in document units (the unit after the number, as "0.25 \"").
fn length(ui: &mut Ui, v: &mut f64, u: Unit, dpi: f64, enabled: bool, w: f32) -> bool {
    let k = u.points_per_unit(dpi).max(1e-9);
    let mut x = *v / k;
    let r = ui.add_enabled(enabled, egui::DragValue::new(&mut x).speed(0.01).range(-1000.0..=10000.0).max_decimals(u.decimals().min(3)).suffix(format!(" {}", u.suffix())));
    let _ = w;
    if r.changed() {
        *v = x * k;
        return true;
    }
    false
}

/// A label right-aligned in a fixed width.
fn rlabel(ui: &mut Ui, text: &str, w: f32, enabled: bool) {
    let col = if enabled { theme::TEXT } else { theme::TEXT_DIM };
    ui.add_sized(vec2(w, 22.0), egui::Label::new(egui::RichText::new(text).color(col)).halign(egui::Align::Max));
}

/// A boxed list with an optional header row; returns (clicked row, double-clicked row).
fn list_box(ui: &mut Ui, id: &str, size: egui::Vec2, header: &[&str], widths: &[f32], rows: &[Vec<String>], sel: Option<usize>) -> (Option<usize>, Option<usize>) {
    let (mut clicked, mut double) = (None, None);
    egui::Frame::new().fill(theme::FIELD).stroke(Stroke::new(1.0, theme::SEP)).show(ui, |ui| { ui.vertical(|ui| {
        ui.set_min_size(size);
        ui.set_max_size(size);
        ui.spacing_mut().item_spacing.y = 0.0;
        if !header.is_empty() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (h, w) in header.iter().zip(widths) {
                    let (r, _) = ui.allocate_exact_size(vec2(*w, 26.0), Sense::hover());
                    ui.painter().rect_filled(r, 0.0, theme::BG);
                    ui.painter().vline(r.right(), r.y_range(), Stroke::new(1.0, theme::SEP));
                    ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(1.0, theme::SEP));
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, *h, egui::FontId::proportional(12.5), theme::TEXT);
                }
            });
        }
        egui::ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, |ui| {
            for (i, row) in rows.iter().enumerate() {
                let (r, resp) = ui.allocate_exact_size(vec2(size.x - 14.0, 25.0), Sense::click());
                if sel == Some(i) {
                    ui.painter().rect_filled(r, 0.0, theme::ACCENT);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, theme::HOVER);
                }
                let mut x = r.left() + 5.0;
                for (k, cell) in row.iter().enumerate() {
                    let w = widths.get(k).copied().unwrap_or(size.x);
                    ui.painter().text(pos2(x, r.center().y), egui::Align2::LEFT_CENTER, cell, egui::FontId::proportional(12.5), theme::TEXT);
                    x += w;
                }
                if resp.double_clicked() {
                    double = Some(i);
                }
                if resp.clicked() {
                    clicked = Some(i);
                }
            }
        });
    });
});
    (clicked, double)
}

// ---------------------------------------------------------------------------------------------
// Text ▸ Tabs
// ---------------------------------------------------------------------------------------------

pub struct TabsState {
    pub tabs: Vec<TabStop>,
    pub loc: f64,
    pub sel: Option<usize>,
    pub leader: Leader,
    /// Leader Settings, while open.
    pub leader_dlg: Option<Leader>,
}

impl TabsState {
    pub fn new(app: &AnyVectorApp) -> TabsState {
        let t = first_text(app);
        let width = match t.kind {
            TextKind::Paragraph { width, .. } => width,
            TextKind::Artistic => 8.0 * 72.0,
        };
        TabsState { tabs: vt::tab_list(&t, width), loc: 36.0, sel: None, leader: t.leader, leader_dlg: None }
    }
}

pub fn tabs(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.tabs.take() else { return };
    let (u, dpi) = unit_of(app);
    if let Some(mut l) = st.leader_dlg.take() {
        let (mut ok, mut cancel) = (false, false);
        let ((), dismissed) = window(ctx, "leader", "Leader Settings", vec2(326.0, 198.0), |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                rlabel(ui, "Character:", 66.0, true);
                ui.add_space(6.0);
                let chars = ['.', '-', '_', '·', '•', '*', '~', '=', '+', '>', ' '];
                egui::ComboBox::from_id_salt("leader_ch").selected_text(l.ch.to_string()).width(38.0).show_ui(ui, |ui| {
                    for c in chars {
                        ui.selectable_value(&mut l.ch, c, if c == ' ' { "(space)".to_string() } else { c.to_string() });
                    }
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                rlabel(ui, "Spacing:", 66.0, true);
                ui.add_space(6.0);
                ui.add(egui::DragValue::new(&mut l.spacing).range(0..=10).speed(0.1));
                ui.label("characters");
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                rlabel(ui, "Preview:", 66.0, true);
                ui.add_space(26.0);
                let fill: String = std::iter::repeat_n(format!("{}{}", l.ch, " ".repeat(l.spacing as usize)), (24 / (l.spacing as usize + 1)).max(4)).collect();
                ui.label(format!("a{fill}b"));
            });
            (ok, cancel) = ok_cancel_row(ui, "OK", true);
        });
        if ok {
            st.leader = l;
        } else if !(cancel || dismissed) {
            st.leader_dlg = Some(l);
        }
        app.dialogs.tabs = Some(st);
        return;
    }
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "tabs", "Tab Settings", vec2(468.0, 424.0), |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(22.0);
            ui.label("Tab location:");
            ui.add_space(14.0);
            let k = u.points_per_unit(dpi);
            let mut x = st.loc / k;
            if ui.add(egui::DragValue::new(&mut x).speed(0.01).range(0.0..=1000.0).fixed_decimals(3)).changed() {
                st.loc = x * k;
            }
            ui.label(unit_name(u));
            ui.add_space(30.0);
            if ui.add(egui::Button::new("Add").min_size(vec2(80.0, 24.0))).clicked() && !st.tabs.iter().any(|t| (t.pos - st.loc).abs() < 0.01) {
                st.tabs.push(TabStop { pos: st.loc, align: TabAlign::Left, leader: false });
                st.tabs.sort_by(|a, b| a.pos.total_cmp(&b.pos));
                st.sel = st.tabs.iter().position(|t| (t.pos - st.loc).abs() < 0.01);
            }
        });
        ui.add_space(10.0);
        // the tab list: position, alignment and leaders (the selected row's cells are lists)
        let size = vec2(392.0, 248.0);
        let widths = [174.0, 156.0, 48.0];
        ui.horizontal(|ui| {
            ui.add_space(22.0);
            egui::Frame::new().fill(theme::FIELD).stroke(Stroke::new(1.0, theme::SEP)).show(ui, |ui| { ui.vertical(|ui| {
                ui.set_min_size(size);
                ui.set_max_size(size);
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (h, w) in ["Tabs", "Alignment", "Leaders"].iter().zip(widths) {
                        let (r, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
                        ui.painter().rect_filled(r, 0.0, theme::BG);
                        ui.painter().vline(r.right(), r.y_range(), Stroke::new(1.0, theme::SEP));
                        ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(1.0, theme::SEP));
                        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, *h, egui::FontId::proportional(12.5), theme::TEXT);
                    }
                });
                egui::ScrollArea::vertical().id_salt("tab_list").auto_shrink([false, false]).show(ui, |ui| {
                    let n = st.tabs.len();
                    for i in 0..n {
                        let selected = st.sel == Some(i);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            let (r, resp) = ui.allocate_exact_size(vec2(widths[0], 25.0), Sense::click());
                            if selected {
                                ui.painter().rect_filled(r.with_max_x(r.left() + size.x - 14.0), 0.0, theme::ACCENT);
                            }
                            ui.painter().text(pos2(r.left() + 5.0, r.center().y), egui::Align2::LEFT_CENTER, fmt_len(st.tabs[i].pos, u, dpi), egui::FontId::proportional(12.5), theme::TEXT);
                            if resp.clicked() {
                                st.sel = Some(i);
                                st.loc = st.tabs[i].pos;
                            }
                            if selected {
                                let tab = &mut st.tabs[i];
                                egui::ComboBox::from_id_salt(("tab_al", i)).selected_text(tab.align.name()).width(widths[1] - 12.0).show_ui(ui, |ui| {
                                    for a in TabAlign::ALL {
                                        ui.selectable_value(&mut tab.align, a, a.name());
                                    }
                                });
                                ui.add_space(12.0);
                                egui::ComboBox::from_id_salt(("tab_ld", i)).selected_text(if tab.leader { "On" } else { "Off" }).width(widths[2] - 4.0).show_ui(ui, |ui| {
                                    ui.selectable_value(&mut tab.leader, true, "On");
                                    ui.selectable_value(&mut tab.leader, false, "Off");
                                });
                            } else {
                                let (r2, resp2) = ui.allocate_exact_size(vec2(widths[1] + widths[2], 25.0), Sense::click());
                                ui.painter().text(pos2(r2.left() + 5.0, r2.center().y), egui::Align2::LEFT_CENTER, st.tabs[i].align.name(), egui::FontId::proportional(12.5), theme::TEXT);
                                ui.painter().text(pos2(r2.left() + widths[1] + 5.0, r2.center().y), egui::Align2::LEFT_CENTER, if st.tabs[i].leader { "On" } else { "Off" }, egui::FontId::proportional(12.5), theme::TEXT);
                                if resp2.clicked() {
                                    st.sel = Some(i);
                                    st.loc = st.tabs[i].pos;
                                }
                            }
                        });
                    }
                });
            });
});
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(22.0);
            if button(ui, "Remove", st.sel.is_some()) {
                if let Some(i) = st.sel.take() {
                    if i < st.tabs.len() {
                        st.tabs.remove(i);
                    }
                }
            }
            if button(ui, "Remove All", !st.tabs.is_empty()) {
                st.tabs.clear();
                st.sel = None;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(22.0);
                if ui.add(egui::Button::new("Leader Options...").min_size(vec2(130.0, 24.0))).clicked() {
                    st.leader_dlg = Some(st.leader);
                }
            });
        });
        separator(ui);
        (ok, cancel) = ok_cancel_plain(ui, true);
    });
    if ok {
        let (tabs, leader) = (st.tabs.clone(), st.leader);
        modify_texts(app, "Tab Settings", true, |t| {
            t.tabs = Some(tabs.clone());
            t.leader = leader;
        });
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.tabs = Some(st);
    }
}

// ---------------------------------------------------------------------------------------------
// Text ▸ Columns
// ---------------------------------------------------------------------------------------------

pub struct ColumnsState {
    /// (width, gutter after) of each column.
    pub cols: Vec<(f64, f64)>,
    pub equal: bool,
    /// Frame Settings: automatically adjust the frame width (else keep it).
    pub auto_width: bool,
    pub frame_w: f64,
    pub sel: usize,
}

impl ColumnsState {
    pub fn new(app: &AnyVectorApp) -> ColumnsState {
        let t = first_text(app);
        let w = match t.kind {
            TextKind::Paragraph { width, .. } => width,
            TextKind::Artistic => 18.0,
        };
        let cols: Vec<(f64, f64)> = if t.column_widths.is_empty() {
            let boxes = vt::column_boxes(&t, w);
            let n = boxes.len();
            boxes.iter().enumerate().map(|(i, b)| (b.1, if i + 1 < n { t.column_gap } else { 0.0 })).collect()
        } else {
            t.column_widths.clone()
        };
        ColumnsState { equal: t.column_widths.is_empty(), cols, auto_width: false, frame_w: w, sel: 0 }
    }

    /// Equal columns over the frame width (or the frame grows to fit them).
    fn rebalance(&mut self) {
        let n = self.cols.len().max(1);
        if self.equal {
            let gutter = self.cols.first().map(|c| c.1).filter(|_| n > 1).unwrap_or(0.0);
            for (i, c) in self.cols.iter_mut().enumerate() {
                c.1 = if i + 1 < n { gutter } else { 0.0 };
            }
            if self.auto_width {
                let w = self.cols[0].0;
                for c in self.cols.iter_mut() {
                    c.0 = w;
                }
            } else {
                let w = ((self.frame_w - gutter * (n as f64 - 1.0)) / n as f64).max(1.0);
                for c in self.cols.iter_mut() {
                    c.0 = w;
                }
            }
        }
        if let Some(l) = self.cols.last_mut() {
            l.1 = 0.0;
        }
        if self.auto_width || !self.equal {
            self.frame_w = self.cols.iter().map(|c| c.0 + c.1).sum();
        }
    }

    fn set_count(&mut self, n: usize) {
        let n = n.clamp(1, 20);
        let gutter = if self.cols.len() > 1 { self.cols[0].1 } else { 18.0 };
        if self.auto_width {
            let w = self.cols.first().map(|c| c.0).unwrap_or(72.0);
            self.cols.resize(n, (w, gutter));
        } else {
            self.cols = vec![(1.0, gutter); n];
            self.equal = true;
        }
        for i in 0..n {
            self.cols[i].1 = if i + 1 < n { gutter } else { 0.0 };
        }
        self.rebalance();
        self.sel = self.sel.min(n - 1);
    }
}

fn columns_preview(ui: &mut Ui, st: &ColumnsState) {
    let (r, _) = ui.allocate_exact_size(vec2(185.0, 193.0), Sense::hover());
    let p = ui.painter();
    p.rect_stroke(r, 0.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Inside);
    let frame = ERect::from_min_max(r.min + vec2(27.0, 10.0), r.max - vec2(27.0, 10.0));
    // dashed frame
    let dash = |a: egui::Pos2, b: egui::Pos2| {
        let len = a.distance(b);
        let dir = (b - a) / len.max(1.0);
        let mut s = 0.0;
        while s < len {
            p.line_segment([a + dir * s, a + dir * (s + 4.0).min(len)], Stroke::new(1.0, Color32::from_gray(220)));
            s += 7.0;
        }
    };
    dash(frame.left_top(), frame.right_top());
    dash(frame.right_top(), frame.right_bottom());
    dash(frame.right_bottom(), frame.left_bottom());
    dash(frame.left_bottom(), frame.left_top());
    let total: f64 = st.cols.iter().map(|c| c.0 + c.1).sum::<f64>().max(1.0);
    let inner = frame.shrink(12.0);
    let k = inner.width() as f64 / total.max(st.frame_w);
    let mut x = inner.left() as f64;
    for (w, g) in &st.cols {
        let x1 = x + w * k;
        let mut y = inner.top() + 3.0;
        for _ in 0..16 {
            p.line_segment([pos2(x as f32, y), pos2(x1 as f32, y)], Stroke::new(1.5, Color32::from_gray(225)));
            y += 4.0;
        }
        x = x1 + g * k;
    }
}

pub fn columns(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.columns.take() else { return };
    let (u, dpi) = unit_of(app);
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "columns", "Column Settings", vec2(647.0, 392.0), |ui| {
        ui.add_space(14.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(270.0, 270.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.add_sized(vec2(170.0, 22.0), egui::Label::new("Number of columns:").halign(egui::Align::Min));
                    let mut n = st.cols.len();
                    if ui.add(egui::DragValue::new(&mut n).range(1..=20).speed(0.1)).changed() {
                        st.set_count(n);
                    }
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    let widths = [82.0f32, 96.0, 76.0];
                    egui::Frame::new().fill(theme::FIELD).stroke(Stroke::new(1.0, theme::SEP)).show(ui, |ui| { ui.vertical(|ui| {
                        ui.set_min_size(vec2(256.0, 154.0));
                        ui.set_max_size(vec2(256.0, 154.0));
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for (h, w) in ["Column", "Width", "Gutter"].iter().zip(widths) {
                                let (r, _) = ui.allocate_exact_size(vec2(w, 25.0), Sense::hover());
                                ui.painter().rect_filled(r, 0.0, theme::BG);
                                ui.painter().vline(r.right(), r.y_range(), Stroke::new(1.0, theme::SEP));
                                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, *h, egui::FontId::proportional(12.5), theme::TEXT);
                            }
                        });
                        egui::ScrollArea::vertical().id_salt("col_list").auto_shrink([false, false]).show(ui, |ui| {
                            let n = st.cols.len();
                            let mut changed = false;
                            for i in 0..n {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    let (r, resp) = ui.allocate_exact_size(vec2(widths[0], 24.0), Sense::click());
                                    if st.sel == i {
                                        ui.painter().rect_filled(r.with_max_x(r.left() + 242.0), 0.0, theme::ACCENT);
                                    }
                                    ui.painter().text(pos2(r.left() + 6.0, r.center().y), egui::Align2::LEFT_CENTER, (i + 1).to_string(), egui::FontId::proportional(12.5), theme::TEXT);
                                    if resp.clicked() {
                                        st.sel = i;
                                    }
                                    ui.add_space(8.0);
                                    let (mut w, mut g) = st.cols[i];
                                    ui.scope(|ui| {
                                        ui.set_width(widths[1] - 8.0);
                                        if length(ui, &mut w, u, dpi, true, 80.0) {
                                            changed = true;
                                        }
                                    });
                                    ui.add_space(4.0);
                                    if length(ui, &mut g, u, dpi, i + 1 < n, 70.0) {
                                        changed = true;
                                    }
                                    let old = st.cols[i];
                                    if (w, g) != old {
                                        if st.equal {
                                            for c in st.cols.iter_mut() {
                                                if w != old.0 {
                                                    c.0 = w;
                                                }
                                                if g != old.1 {
                                                    c.1 = g;
                                                }
                                            }
                                        }
                                        st.cols[i] = (w.max(1.0), g.max(0.0));
                                    }
                                });
                            }
                            if changed {
                                if st.equal && !st.auto_width {
                                    // keep the frame width: the gutter wins, the widths follow
                                    st.rebalance();
                                } else {
                                    st.frame_w = st.cols.iter().map(|c| c.0 + c.1).sum();
                                }
                            }
                        });
                    });
});
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    if ui.checkbox(&mut st.equal, "Equal column width").changed() {
                        st.rebalance();
                    }
                });
            });
            ui.add_space(60.0);
            ui.allocate_ui_with_layout(vec2(240.0, 280.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(46.0);
                    columns_preview(ui, &st);
                });
                ui.add_space(14.0);
                group(ui, "Frame Settings", |ui| {
                    ui.radio_value(&mut st.auto_width, false, "Maintain current frame width");
                    ui.radio_value(&mut st.auto_width, true, "Automatically adjust frame width");
                });
            });
        });
        separator(ui);
        (ok, cancel) = ok_cancel_row(ui, "OK", true);
    });
    if ok {
        let st2 = (st.cols.clone(), st.equal, st.frame_w);
        modify_texts(app, "Column Settings", false, |t| {
            let (cols, equal, fw) = (&st2.0, st2.1, st2.2);
            t.columns = cols.len() as u32;
            t.column_gap = cols.first().map(|c| c.1).filter(|_| cols.len() > 1).unwrap_or(t.column_gap);
            t.column_widths = if equal { vec![] } else { cols.clone() };
            if let TextKind::Paragraph { width, .. } = &mut t.kind {
                *width = fw.max(1.0);
            }
        });
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.columns = Some(st);
    }
}

// ---------------------------------------------------------------------------------------------
// Text ▸ Bullets and Numbering
// ---------------------------------------------------------------------------------------------

pub struct BulletsState {
    pub on: bool,
    pub list: ListFormat,
}

impl BulletsState {
    pub fn new(app: &AnyVectorApp) -> BulletsState {
        let t = first_text(app);
        BulletsState { on: t.list.is_some(), list: t.list.unwrap_or_default() }
    }
}

/// The bullet glyphs offered by the Glyph list: the characters of the font.
fn glyph_picker(app: &mut AnyVectorApp, ui: &mut Ui, font: &str, glyph: &mut char, enabled: bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(66.0, 50.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(r, 0.0, theme::FIELD);
    p.rect_stroke(r, 0.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Inside);
    let arrow = ERect::from_min_max(pos2(r.right() - 20.0, r.top()), r.max);
    p.rect_filled(arrow, 0.0, theme::BG);
    p.text(arrow.center(), egui::Align2::CENTER_CENTER, "⌄", egui::FontId::proportional(13.0), theme::TEXT);
    if let Some(tex) = crate::dockers::text::sample_texture(app, ui.ctx(), font, &glyph.to_string(), 18.0) {
        let s = tex.size_vec2().min(vec2(36.0, 36.0));
        ui.painter().image(tex.id(), ERect::from_center_size(pos2(r.left() + 23.0, r.center().y), s), ERect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), if enabled { Color32::WHITE } else { Color32::from_gray(120) });
    }
    let id = ui.make_persistent_id("bullet_glyphs");
    if resp.clicked() && enabled {
        egui::Popup::toggle_id(ui.ctx(), id);
    }
    egui::Popup::from_response(&resp).id(id).open_memory(None::<egui::SetOpenCommand>).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_width(250.0);
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            let chars = vt::font_chars(font, false, false, 600);
            egui::Grid::new("bullet_grid").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
                let mut k = 0;
                for c in chars.into_iter().filter(|c| !c.is_whitespace() && !c.is_control()) {
                    let Some(tex) = crate::dockers::text::sample_texture(app, ui.ctx(), font, &c.to_string(), 20.0) else { continue };
                    let (r, resp) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::click());
                    if *glyph == c {
                        ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, theme::ACCENT), egui::StrokeKind::Inside);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, 0.0, theme::HOVER);
                    }
                    let s = tex.size_vec2().min(vec2(30.0, 30.0));
                    ui.painter().image(tex.id(), ERect::from_center_size(r.center(), s), ERect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    if resp.clicked() {
                        *glyph = c;
                        egui::Popup::close_id(ui.ctx(), id);
                    }
                    k += 1;
                    if k % 5 == 0 {
                        ui.end_row();
                    }
                }
            });
        });
    });
}

pub fn bullets(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.bullets.take() else { return };
    let (u, dpi) = unit_of(app);
    let fams: Vec<String> = vt::fonts().lock().families().to_vec();
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "bullets", "Bullets and Numbering", vec2(580.0, 393.0), |ui| {
        ui.add_space(8.0);
        let on = st.on;
        ui.horizontal(|ui| {
            ui.checkbox(&mut st.on, "List");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut v = st.list.frame_to_list;
                ui.scope(|ui| {
                    ui.set_width(70.0);
                    if length(ui, &mut v, u, dpi, on, 70.0) {
                        st.list.frame_to_list = v.max(0.0);
                    }
                });
                ui.label(egui::RichText::new("Text frame to list:").color(if on { theme::TEXT } else { theme::TEXT_DIM }));
            });
        });
        separator(ui);
        ui.horizontal_top(|ui| {
            let l = &mut st.list;
            // Level
            ui.allocate_ui_with_layout(vec2(100.0, 250.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.label(egui::RichText::new("Level").strong());
                ui.add_space(6.0);
                egui::Frame::new().fill(theme::FIELD).show(ui, |ui| { ui.vertical(|ui| {
                    ui.set_min_size(vec2(100.0, 226.0));
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for lv in 1..=9u8 {
                        let (r, resp) = ui.allocate_exact_size(vec2(82.0, 25.0), Sense::click());
                        if l.level == lv {
                            ui.painter().rect_filled(r, 0.0, theme::ACCENT);
                        }
                        ui.painter().text(pos2(r.left() + 4.0, r.center().y), egui::Align2::LEFT_CENTER, lv.to_string(), egui::FontId::proportional(12.5), theme::TEXT);
                        if resp.clicked() {
                            l.level = lv;
                        }
                    }
                });
});
            });
            ui.add_space(18.0);
            // Type
            ui.allocate_ui_with_layout(vec2(186.0, 250.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.add_enabled_ui(on, |ui| {
                    ui.label(egui::RichText::new("Type").strong());
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut l.numbers, false, "Bullets");
                        ui.add_space(20.0);
                        ui.radio_value(&mut l.numbers, true, "Numbers");
                    });
                    ui.add_space(4.0);
                    ui.checkbox(&mut l.use_para_font, "Use paragraph font");
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        rlabel(ui, "Font:", 54.0, on);
                        ui.add_space(8.0);
                        egui::ComboBox::from_id_salt("bullet_font").selected_text(&l.font).width(110.0).height(300.0).show_ui(ui, |ui| {
                            for f in &fams {
                                ui.selectable_value(&mut l.font, f.clone(), f);
                            }
                        });
                    });
                    ui.add_space(6.0);
                    if l.numbers {
                        ui.horizontal(|ui| {
                            rlabel(ui, "Style:", 54.0, on);
                            ui.add_space(8.0);
                            let mut i = l.style as usize;
                            if combo(ui, "num_style", &mut i, vt::NUMBER_STYLES, 104.0) {
                                l.style = i as u8;
                            }
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            rlabel(ui, "Prefix:", 54.0, on);
                            ui.add_space(8.0);
                            ui.add(egui::TextEdit::singleline(&mut l.prefix).desired_width(110.0));
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            rlabel(ui, "Suffix:", 54.0, on);
                            ui.add_space(8.0);
                            ui.add(egui::TextEdit::singleline(&mut l.suffix).desired_width(110.0));
                        });
                    }
                });
            });
            let numbers = l.numbers;
            if !numbers {
                // the Glyph list (outside the enabled scope: it needs the app for the samples)
                let font = if st.list.use_para_font { first_text(app).format.font } else { st.list.font.clone() };
                let mut g = st.list.glyph;
                ui.scope_builder(egui::UiBuilder::new().max_rect(ERect::from_min_size(ui.min_rect().left_top() + vec2(136.0, 136.0), vec2(160.0, 56.0))), |ui| {
                    ui.horizontal(|ui| {
                        rlabel(ui, "Glyph:", 54.0, on);
                        ui.add_space(8.0);
                        glyph_picker(app, ui, &font, &mut g, on);
                    });
                });
                st.list.glyph = g;
            }
            let l = &mut st.list;
            ui.add_space(8.0);
            let (r, _) = ui.allocate_exact_size(vec2(1.0, 250.0), Sense::hover());
            ui.painter().vline(r.center().x, r.y_range(), Stroke::new(1.0, theme::SEP));
            ui.add_space(16.0);
            // Size and spacing
            ui.allocate_ui_with_layout(vec2(200.0, 250.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.add_enabled_ui(on, |ui| {
                    ui.label(egui::RichText::new("Size and spacing").strong());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        rlabel(ui, "Size:", 110.0, on && !l.use_para_font);
                        ui.add_space(6.0);
                        ui.add_enabled(!l.use_para_font, egui::DragValue::new(&mut l.size).range(1.0..=999.0).speed(0.2).max_decimals(1).suffix(" pt"));
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        rlabel(ui, "Baseline shift:", 110.0, on);
                        ui.add_space(6.0);
                        ui.add(egui::DragValue::new(&mut l.baseline_shift).range(-999.0..=999.0).speed(0.2).max_decimals(1).suffix(" pt"));
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        rlabel(ui, "Glyph to list text:", 110.0, on);
                        ui.add_space(6.0);
                        let mut v = l.glyph_to_text;
                        if length(ui, &mut v, u, dpi, on, 70.0) {
                            l.glyph_to_text = v.max(0.0);
                        }
                    });
                    ui.add_space(4.0);
                    ui.checkbox(&mut l.align_lines, "Align lines in a list item");
                });
            });
        });
        (ok, cancel) = ok_cancel_row(ui, "OK", true);
    });
    if ok {
        let list = st.on.then(|| st.list.clone());
        modify_texts(app, "Bullets and Numbering", true, |t| t.list = list.clone());
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.bullets = Some(st);
    }
}

// ---------------------------------------------------------------------------------------------
// Text ▸ Drop Cap
// ---------------------------------------------------------------------------------------------

pub struct DropCapState {
    pub on: bool,
    pub dc: DropCap,
}

impl DropCapState {
    pub fn new(app: &AnyVectorApp) -> DropCapState {
        let t = first_text(app);
        DropCapState { on: t.drop_cap.is_some(), dc: t.drop_cap.unwrap_or_default() }
    }
}

pub fn drop_cap(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.drop_cap.take() else { return };
    let (u, dpi) = unit_of(app);
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "drop_cap", "Drop Cap", vec2(400.0, 244.0), |ui| {
        ui.add_space(8.0);
        ui.checkbox(&mut st.on, "Use drop cap");
        let on = st.on;
        ui.add_enabled_ui(on, |ui| {
            group(ui, "Appearance", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Number of lines dropped:");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(egui::DragValue::new(&mut st.dc.lines).range(2..=10).speed(0.1));
                    });
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Space after drop cap:");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut v = st.dc.space;
                        if length(ui, &mut v, u, dpi, on, 70.0) {
                            st.dc.space = v.max(0.0);
                        }
                    });
                });
                ui.add_space(4.0);
                ui.checkbox(&mut st.dc.hanging, "Use hanging indent style for drop cap");
            });
        });
        separator(ui);
        (ok, cancel) = ok_cancel_row(ui, "OK", true);
    });
    if ok {
        let dc = st.on.then_some(st.dc);
        modify_texts(app, "Drop Cap", true, |t| t.drop_cap = dc);
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.drop_cap = Some(st);
    }
}

// ---------------------------------------------------------------------------------------------
// Text ▸ Text Statistics
// ---------------------------------------------------------------------------------------------

pub struct StatsState {
    pub styles: bool,
}

/// Lines, words, characters and the fonts of a set of texts.
pub fn statistics(texts: &[TextObj]) -> (usize, usize, usize, Vec<String>) {
    let (mut lines, mut words, mut chars) = (0, 0, 0);
    let mut fonts: Vec<String> = vec![];
    for t in texts {
        if t.link_prev.is_some() {
            continue;
        }
        let lay = vt::layout(t);
        lines += lay.lines.iter().filter(|l| l.end_char > l.first_char || !t.content.is_empty()).count();
        words += t.content.split(|c: char| c.is_whitespace() || vt::is_para_end(c)).filter(|w| w.chars().any(|c| c.is_alphanumeric())).count();
        chars += t.content.chars().filter(|c| !vt::is_para_end(*c)).count();
        for f in std::iter::once(&t.format).chain(t.spans.iter().map(|s| &s.format)) {
            if !fonts.contains(&f.font) {
                fonts.push(f.font.clone());
            }
        }
    }
    (lines, words, chars, fonts)
}

pub fn text_stats(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.text_stats.take() else { return };
    let ids = target_ids(app);
    let mut texts: Vec<TextObj> = vec![];
    if let Some(d) = app.doc() {
        if ids.is_empty() {
            for p in &d.doc.pages {
                for o in p.all_objects() {
                    o.walk(&mut |x| {
                        if let ObjKind::Text(t) = &x.kind {
                            texts.push(t.clone());
                        }
                    });
                }
            }
        } else {
            for id in &ids {
                if let Some(ObjKind::Text(t)) = d.doc.find(chain_head(&d.doc, *id)).map(|o| &o.kind) {
                    texts.push(t.clone());
                }
            }
        }
    }
    let (lines, words, chars, fonts) = statistics(&texts);
    let mut close = false;
    let ((), dismissed) = window(ctx, "text_stats", "Statistics", vec2(400.0, 362.0), |ui| {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(if ids.is_empty() { "All Text In Document" } else { "Selected Text" });
        });
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label("Statistics:");
        });
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(6.0).show(ui, |ui| { ui.vertical(|ui| {
                ui.set_min_size(vec2(343.0, 138.0));
                ui.set_max_width(343.0);
                egui::ScrollArea::vertical().max_height(138.0).show(ui, |ui| {
                    egui::Grid::new("stats").num_columns(2).min_col_width(170.0).spacing(vec2(30.0, 6.0)).show(ui, |ui| {
                        for (k, v) in [("Lines:", lines), ("Words:", words), ("Characters:", chars)] {
                            ui.label(k);
                            ui.label(egui::RichText::new(v.to_string()).color(Color32::from_rgb(0xf0, 0xc0, 0x80)));
                            ui.end_row();
                        }
                        if st.styles && !fonts.is_empty() {
                            ui.label("Fonts used:");
                            ui.label(egui::RichText::new(fonts.len().to_string()).color(Color32::from_rgb(0xf0, 0xc0, 0x80)));
                            ui.end_row();
                            for f in &fonts {
                                ui.label(format!("    {f}"));
                                ui.end_row();
                            }
                        }
                    });
                });
            });
});
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.checkbox(&mut st.styles, "Show style statistics");
        });
        let rect = ERect::from_min_max(pos2(ui.max_rect().left(), ui.max_rect().bottom() - 24.0), ui.max_rect().right_bottom());
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.add(egui::Button::new("?").min_size(vec2(24.0, 24.0)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(6.0);
                    close = ok_button(ui, "Close", true);
                });
            });
        });
    });
    if !(close || dismissed) {
        app.dialogs.text_stats = Some(st);
    }
}

// ---------------------------------------------------------------------------------------------
// Text ▸ Insert Formatting Code ▸ Custom Optional Hyphens
// ---------------------------------------------------------------------------------------------

pub struct HyphensState {
    pub typing: bool,
    pub pasting: bool,
    pub lang: Option<usize>,
    pub show_all: bool,
    pub word: String,
    pub hyphenated: String,
    pub defs: Vec<(String, String)>,
    pub sel: Option<usize>,
}

impl HyphensState {
    pub fn new(app: &AnyVectorApp) -> HyphensState {
        let s = &app.settings;
        HyphensState { typing: s.hyphen_when_typing, pasting: s.hyphen_when_pasting, lang: None, show_all: false, word: String::new(), hyphenated: String::new(), defs: s.hyphen_defs.clone(), sel: None }
    }
}

/// Put the custom optional hyphens into every defined word of a text. Returns the new text when it changed.
pub fn apply_hyphen_defs(content: &str, defs: &[(String, String)]) -> Option<String> {
    let mut out = String::with_capacity(content.len());
    let mut changed = false;
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String, changed: &mut bool| {
        if let Some((_, h)) = defs.iter().find(|(w, _)| w.eq_ignore_ascii_case(word)) {
            // keep the word's own letters (case) and add the breaks of the definition
            let mut k = word.chars();
            for hc in h.chars() {
                if hc == '-' {
                    out.push(vt::SOFT_HYPHEN);
                    *changed = true;
                } else if let Some(c) = k.next() {
                    out.push(c);
                }
            }
            out.extend(k);
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for c in content.chars() {
        if c.is_alphanumeric() {
            word.push(c);
        } else {
            flush(&mut word, &mut out, &mut changed);
            out.push(c);
        }
    }
    flush(&mut word, &mut out, &mut changed);
    changed.then_some(out)
}

/// Replace a text's content keeping the formatting of the characters that stay.
pub fn set_content(t: &mut TextObj, new: &str) {
    let old: Vec<char> = t.content.chars().collect();
    let nw: Vec<char> = new.chars().collect();
    let mut a = 0;
    while a < old.len() && a < nw.len() && old[a] == nw[a] {
        a += 1;
    }
    let mut b = 0;
    while b < old.len() - a && b < nw.len() - a && old[old.len() - 1 - b] == nw[nw.len() - 1 - b] {
        b += 1;
    }
    let (oe, ne) = (old.len() - b, nw.len() - b);
    if oe > a {
        t.delete(a, oe);
    }
    if ne > a {
        let s: String = nw[a..ne].iter().collect();
        t.insert(a, &s);
    }
}

pub fn hyphens(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.hyphens.take() else { return };
    let (mut ok, mut cancel, mut apply) = (false, false, false);
    let can_apply = !target_ids(app).is_empty();
    let ((), dismissed) = window(ctx, "hyphens", "Custom Optional Hyphens", vec2(434.0, 545.0), |ui| {
        ui.add_space(8.0);
        ui.label("Automatically insert custom option hyphens:");
        ui.checkbox(&mut st.typing, "When typing");
        ui.checkbox(&mut st.pasting, "When pasting and importing text");
        group(ui, "Define custom option hyphens", |ui| {
            let langs: Vec<&str> = if st.show_all { crate::spell::LANGUAGES.iter().map(|l| l.0).collect() } else { vec!["English (United States)"] };
            ui.horizontal(|ui| {
                ui.add_sized(vec2(116.0, 22.0), egui::Label::new("Language:").halign(egui::Align::Min));
                let cur = st.lang.and_then(|i| langs.get(i).copied()).unwrap_or("");
                egui::ComboBox::from_id_salt("hy_lang").selected_text(cur).width(246.0).show_ui(ui, |ui| {
                    for (i, l) in langs.iter().enumerate() {
                        if ui.selectable_label(st.lang == Some(i), *l).clicked() {
                            st.lang = Some(i);
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.add_space(120.0);
                ui.checkbox(&mut st.show_all, "Show all languages");
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_sized(vec2(116.0, 22.0), egui::Label::new("Word:").halign(egui::Align::Min));
                let before = st.word.clone();
                ui.add(egui::TextEdit::singleline(&mut st.word).desired_width(254.0));
                if st.word != before {
                    st.hyphenated = st.word.clone();
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_sized(vec2(116.0, 22.0), egui::Label::new("Hyphenated Word:").halign(egui::Align::Min));
                ui.add(egui::TextEdit::singleline(&mut st.hyphenated).desired_width(254.0));
            });
            ui.add_space(6.0);
            let valid = !st.word.trim().is_empty() && st.hyphenated.contains('-') && st.hyphenated.replace('-', "").eq_ignore_ascii_case(st.word.trim());
            ui.horizontal(|ui| {
                ui.add_space(116.0);
                if ui.add_enabled(valid, egui::Button::new("Add Definition").min_size(vec2(118.0, 24.0))).clicked() {
                    let w = st.word.trim().to_string();
                    st.defs.retain(|d| !d.0.eq_ignore_ascii_case(&w));
                    st.defs.push((w, st.hyphenated.trim().to_string()));
                    st.defs.sort_by_key(|d| d.0.to_lowercase());
                    st.word.clear();
                    st.hyphenated.clear();
                }
                ui.add_space(6.0);
                if ui.add_enabled(st.sel.is_some(), egui::Button::new("Remove Definition").min_size(vec2(130.0, 24.0))).clicked() {
                    if let Some(i) = st.sel.take() {
                        if i < st.defs.len() {
                            st.defs.remove(i);
                        }
                    }
                }
            });
            ui.add_space(12.0);
            ui.label("List of custom definitions:");
            ui.add_space(4.0);
            let rows: Vec<Vec<String>> = st.defs.iter().map(|d| vec![d.0.clone(), d.1.clone()]).collect();
            let (clicked, _) = list_box(ui, "hy_list", vec2(384.0, 150.0), &["Word", "Hyphenated Word"], &[190.0, 190.0], &rows, st.sel);
            if let Some(i) = clicked {
                st.sel = Some(i);
                st.word = st.defs[i].0.clone();
                st.hyphenated = st.defs[i].1.clone();
            }
        });
        separator(ui);
        let rect = ERect::from_min_max(pos2(ui.max_rect().left(), ui.max_rect().bottom() - 24.0), ui.max_rect().right_bottom());
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Button::new("?").min_size(vec2(24.0, 24.0)));
                ui.add_space(6.0);
                apply = ui.add_enabled(can_apply && !st.defs.is_empty(), egui::Button::new("Apply to Selection").min_size(vec2(136.0, 24.0))).clicked();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    cancel = button(ui, "Cancel", true);
                    ui.add_space(4.0);
                    ok = ok_button(ui, "OK", true);
                });
            });
        });
    });
    if apply {
        let defs = st.defs.clone();
        modify_texts(app, "Custom Optional Hyphens", true, |t| {
            if let Some(s) = apply_hyphen_defs(&t.content, &defs) {
                set_content(t, &s);
            }
        });
    }
    if ok {
        let s = &mut app.settings;
        s.hyphen_when_typing = st.typing;
        s.hyphen_when_pasting = st.pasting;
        s.hyphen_defs = st.defs.clone();
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.hyphens = Some(st);
    }
}

// ---------------------------------------------------------------------------------------------
// Text ▸ Writing Tools (Spell Checker, Grammatik, Thesaurus)
// ---------------------------------------------------------------------------------------------

/// Text the writing tools check: a text object, or a table cell (its index).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    pub obj: ObjId,
    pub cell: Option<usize>,
}

/// A word or passage the checker stopped at.
#[derive(Clone, Debug)]
pub struct Found {
    pub src: Source,
    pub start: usize,
    pub len: usize,
    pub word: String,
    pub rule: &'static str,
    pub explanation: String,
    pub sentence: String,
}

pub struct WritingState {
    /// 0 Spell Checker, 1 Grammatik, 2 Thesaurus.
    pub tab: usize,
    /// Check: Document (else Selected Text).
    pub check_doc: bool,
    /// Where checking goes on from: source index and character.
    pub pos: (usize, usize),
    pub found: Option<Found>,
    pub done: bool,
    pub replace_with: String,
    pub replacements: Vec<String>,
    pub rep_sel: Option<usize>,
    pub skip_all: Vec<String>,
    /// Replacements made (Undo takes them back).
    pub undo: usize,
    /// The checking language (BCP 47).
    pub lang: String,
    // Grammatik options
    pub suggest_spelling: bool,
    // Thesaurus
    pub word: String,
    pub history: Vec<String>,
    pub results: Vec<String>,
    pub res_sel: Option<usize>,
    pub looked: Option<String>,
    pub suggestions: Vec<String>,
    /// Synonym, Antonym, Related Words, Is a Type of, Has Types, Is a Part of, Has Parts, Example.
    pub relations: [bool; 8],
    /// Language… was chosen from the Options menu.
    pub pick_lang: Option<usize>,
}

impl WritingState {
    pub fn new(app: &AnyVectorApp, tab: usize) -> WritingState {
        let selected = !target_ids(app).is_empty() || app.doc().is_some_and(|d| d.selection.iter().any(|id| d.doc.find(*id).is_some_and(|o| matches!(o.kind, ObjKind::Table(_)))));
        let lang = first_text(app).format.lang;
        let word = selected_word(app).unwrap_or_default();
        let mut st = WritingState {
            tab,
            check_doc: !selected,
            pos: (0, 0),
            found: None,
            done: false,
            replace_with: String::new(),
            replacements: vec![],
            rep_sel: None,
            skip_all: vec![],
            undo: 0,
            lang: crate::spell::lang_of(&lang).to_string(),
            suggest_spelling: true,
            word: word.clone(),
            history: vec![],
            results: vec![],
            res_sel: None,
            looked: None,
            suggestions: vec![],
            relations: [true; 8],
            pick_lang: None,
        };
        if tab == 2 && !word.is_empty() && app.settings.thes_auto_lookup {
            st.look_up(&word, app.settings.thes_spelling_assist);
        }
        if tab < 2 && app.settings.spell_auto_start {
            st.next(app);
        }
        st
    }

    fn look_up(&mut self, word: &str, assist: bool) {
        let w = word.trim().to_string();
        self.word = w.clone();
        self.results = if self.relations[0] { crate::spell::synonyms(&w) } else { vec![] };
        self.res_sel = None;
        self.suggestions = if self.results.is_empty() && assist { crate::spell::suggest(&w, &self.lang) } else { vec![] };
        self.looked = Some(w.clone());
        if !w.is_empty() && !self.history.contains(&w) {
            self.history.insert(0, w);
        }
    }

    /// Find the next stop from `pos` on.
    pub fn next(&mut self, app: &AnyVectorApp) {
        let sources = sources(app, self.check_doc);
        self.found = None;
        self.done = false;
        if !crate::spell::supported(&self.lang) {
            // no dictionary for the language on this computer
            self.pos = (sources.len(), 0);
        }
        while self.pos.0 < sources.len() {
            let src = sources[self.pos.0];
            let text = source_text(app, src).unwrap_or_default();
            let chars: Vec<char> = text.chars().collect();
            let stops = self.stops(app, &text);
            if let Some((s, l, rule, expl, reps)) = stops.into_iter().find(|e| e.0 >= self.pos.1) {
                let word: String = chars[s..(s + l).min(chars.len())].iter().collect();
                let sentence = sentence_of(&chars, s, l, reps.first().map(|r| r.as_str()));
                self.replacements = reps;
                self.rep_sel = (!self.replacements.is_empty()).then_some(0);
                self.replace_with = self.replacements.first().cloned().unwrap_or_else(|| word.clone());
                self.found = Some(Found { src, start: s, len: l, word, rule, explanation: expl, sentence });
                return;
            }
            self.pos = (self.pos.0 + 1, 0);
        }
        self.done = true;
        self.replacements.clear();
        self.replace_with.clear();
        self.rep_sel = None;
    }

    /// Where a text needs checking: (start, len, rule, explanation, replacements).
    fn stops(&self, app: &AnyVectorApp, text: &str) -> Vec<(usize, usize, &'static str, String, Vec<String>)> {
        let s = &app.settings;
        let chars: Vec<char> = text.chars().collect();
        let word_at = |a: usize, l: usize| chars[a..(a + l).min(chars.len())].iter().collect::<String>();
        let keep = |a: usize, l: usize| {
            let w = word_at(a, l);
            !self.skip_all.iter().any(|x| *x == w) && (s.spell_numbers || !w.chars().any(|c| c.is_ascii_digit()))
        };
        if self.tab == 1 {
            return crate::spell::grammar(text, &self.lang)
                .into_iter()
                .filter(|e| keep(e.0, e.1))
                .map(|mut e| {
                    if e.2 == "Spelling" && !self.suggest_spelling {
                        e.4.clear();
                    }
                    e
                })
                .collect();
        }
        crate::spell::check(text, &self.lang)
            .into_iter()
            .filter(|e| keep(e.start, e.len) && (s.spell_duplicates || !e.delete))
            .filter(|e| {
                // irregular capitals ("tHe") only when asked
                let w = word_at(e.start, e.len);
                s.spell_caps || e.delete || !(w.chars().skip(1).any(|c| c.is_uppercase()) && w.chars().any(|c| c.is_lowercase()))
            })
            .map(|e| {
                let w = word_at(e.start, e.len);
                if e.delete {
                    (e.start, e.len, "Duplicate word", format!("`{w}` is repeated."), vec![String::new()])
                } else {
                    let mut sug = crate::spell::suggest(&w, &self.lang);
                    if let Some(r) = &e.replacement {
                        sug.retain(|x| x != r);
                        sug.insert(0, r.clone());
                    }
                    sug.truncate(s.spell_suggestions.max(1) as usize * 3);
                    (e.start, e.len, "Spelling", format!("`{w}` is not in the spelling dictionary."), sug)
                }
            })
            .collect()
    }
}

/// The sentence around a stop, with the replacement in `quotes`.
fn sentence_of(chars: &[char], s: usize, l: usize, rep: Option<&str>) -> String {
    let mut a = s;
    while a > 0 && !matches!(chars[a - 1], '.' | '!' | '?' | '\n') {
        a -= 1;
    }
    let mut b = (s + l).min(chars.len());
    while b < chars.len() && !matches!(chars[b], '.' | '!' | '?' | '\n') {
        b += 1;
    }
    if b < chars.len() && chars[b] != '\n' {
        b += 1;
    }
    let before: String = chars[a..s].iter().collect();
    let after: String = chars[(s + l).min(chars.len())..b].iter().collect();
    let mid: String = rep.map(|r| r.to_string()).unwrap_or_else(|| chars[s..(s + l).min(chars.len())].iter().collect());
    format!("{}`{}`{}", before.trim_start(), mid, after.trim_end())
}

/// The selected word while editing text (or the word at the caret).
fn selected_word(app: &AnyVectorApp) -> Option<String> {
    let d = app.doc()?;
    let te = d.text_edit.as_ref()?;
    let ObjKind::Text(t) = &d.doc.find(te.obj)?.kind else { return None };
    let chars: Vec<char> = t.content.chars().collect();
    let (mut a, mut b) = (te.caret.min(te.anchor).min(chars.len()), te.caret.max(te.anchor).min(chars.len()));
    if a == b {
        while a > 0 && chars[a - 1].is_alphanumeric() {
            a -= 1;
        }
        while b < chars.len() && chars[b].is_alphanumeric() {
            b += 1;
        }
    }
    let w: String = chars[a..b].iter().collect();
    (!w.trim().is_empty()).then(|| w.trim().to_string())
}

/// Texts to check: the selection (or text being edited), or every text of the document.
pub fn sources(app: &AnyVectorApp, whole_doc: bool) -> Vec<Source> {
    let Some(d) = app.doc() else { return vec![] };
    let mut out = vec![];
    let mut add = |o: &vector_core::Object| {
        o.walk(&mut |x| match &x.kind {
            ObjKind::Text(t) if t.link_prev.is_none() => out.push(Source { obj: x.id, cell: None }),
            ObjKind::Table(t) => {
                for (r, c) in t.anchors() {
                    out.push(Source { obj: x.id, cell: Some(r * t.cols + c) });
                }
            }
            _ => {}
        })
    };
    if whole_doc {
        for p in &d.doc.pages {
            for o in p.all_objects() {
                add(o);
            }
        }
    } else if let Some(te) = &d.text_edit {
        out.push(Source { obj: chain_head(&d.doc, te.obj), cell: None });
    } else {
        for id in &d.selection {
            if let Some(o) = d.doc.find(*id) {
                add(o);
            }
        }
    }
    out
}

pub fn source_text(app: &AnyVectorApp, s: Source) -> Option<String> {
    let d = app.doc()?;
    match (&d.doc.find(s.obj)?.kind, s.cell) {
        (ObjKind::Text(t), None) => Some(t.content.clone()),
        (ObjKind::Table(t), Some(i)) => t.cells.get(i).map(|c| c.text.clone()),
        _ => None,
    }
}

/// Replace characters [start, start + len) of a source.
pub fn replace_in(app: &mut AnyVectorApp, s: Source, start: usize, len: usize, with: &str, label: &str) {
    let Some(d) = app.doc_mut() else { return };
    d.modify(label, |doc, _| match doc.find_mut(s.obj).map(|o| &mut o.kind) {
        Some(ObjKind::Text(t)) => {
            t.delete(start, start + len);
            t.insert(start, with);
        }
        Some(ObjKind::Table(t)) => {
            if let Some(c) = s.cell.and_then(|i| t.cells.get_mut(i)) {
                let chars: Vec<char> = c.text.chars().collect();
                let a = start.min(chars.len());
                let b = (start + len).min(chars.len());
                c.text = format!("{}{}{}", chars[..a].iter().collect::<String>(), with, chars[b..].iter().collect::<String>());
            }
        }
        _ => {}
    });
}

const SPELL_OPTIONS: [&str; 8] = ["Auto Start", "Beep on Misspelled", "Recheck All Text", "Check Words with Numbers", "Check Duplicate Words", "Check Irregular Capitalization", "Prompt Before Auto Replacement", "Show Phonetic Suggestions"];
const RELATIONS: [&str; 8] = ["Synonym", "Antonym", "Related Words", "Is a Type of", "Has Types", "Is a Part of", "Has Parts", "Example"];

fn spell_option(app: &mut AnyVectorApp, i: usize) -> &mut bool {
    let s = &mut app.settings;
    match i {
        0 => &mut s.spell_auto_start,
        1 => &mut s.spell_beep,
        2 => &mut s.spell_recheck,
        3 => &mut s.spell_numbers,
        4 => &mut s.spell_duplicates,
        5 => &mut s.spell_caps,
        6 => &mut s.spell_prompt,
        _ => &mut s.spell_phonetic,
    }
}

fn check_item(ui: &mut Ui, on: bool, text: &str) -> bool {
    ui.add(egui::Button::new(format!("{}  {text}", if on { "✔" } else { "   " })).frame(false)).clicked()
}

pub fn writing_tools(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.writing.take() else { return };
    // Options ▸ Language…: the Text Language list for the checking language
    if let Some(mut sel) = st.pick_lang.take() {
        let (mut ok, mut cancel) = (false, false);
        let ((), dismissed) = window(ctx, "wt_lang", "Text Language", vec2(390.0, 263.0), |ui| {
            (ok, cancel) = language_list(ui, &mut sel);
        });
        if ok {
            st.lang = crate::spell::LANGUAGES[sel].2.to_string();
            st.pos = (st.pos.0, 0);
            if st.tab < 2 {
                st.next(app);
            }
        } else if !(cancel || dismissed) {
            st.pick_lang = Some(sel);
        }
        app.dialogs.writing = Some(st);
        return;
    }
    let mut close = false;
    let mut action: Option<&'static str> = None;
    let mut look: Option<String> = None;
    let has_text = !sources(app, false).is_empty();
    let replace_label = if app.doc().is_some_and(|d| d.text_edit.as_ref().is_some_and(|te| te.caret != te.anchor)) { "Replace" } else { "Insert" };
    let ((), dismissed) = window(ctx, "writing_tools", "Writing Tools", vec2(633.0, 310.0), |ui| {
        ui.add_space(8.0);
        // the tabs, as buttons
        ui.horizontal(|ui| {
            for (i, n) in ["Spell Checker", "Grammatik", "Thesaurus"].iter().enumerate() {
                let on = st.tab == i;
                let b = egui::Button::new(egui::RichText::new(*n).color(if on { Color32::WHITE } else { theme::TEXT })).fill(if on { theme::ACCENT } else { theme::FIELD }).min_size(vec2(0.0, 24.0));
                if ui.add(b).clicked() && !on {
                    st.tab = i;
                    st.pos = (0, 0);
                    st.found = None;
                    st.done = false;
                    if i < 2 && app.settings.spell_auto_start {
                        st.next(app);
                    }
                }
            }
        });
        let body = ui.available_rect_before_wrap();
        let frame = ERect::from_min_max(body.min, pos2(body.right(), body.bottom() - 36.0));
        ui.painter().rect_stroke(frame, 0.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Inside);
        ui.scope_builder(egui::UiBuilder::new().max_rect(frame.shrink(12.0)), |ui| {
            if st.tab == 2 {
                // Thesaurus
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let r = egui::TextEdit::singleline(&mut st.word).desired_width(232.0).show(ui);
                    let hid = ui.make_persistent_id("thes_hist");
                    let ar = ui.add(egui::Button::new("⌄").min_size(vec2(22.0, 22.0)));
                    if ar.clicked() {
                        egui::Popup::toggle_id(ui.ctx(), hid);
                    }
                    egui::Popup::from_response(&ar).id(hid).open_memory(None::<egui::SetOpenCommand>).close_behavior(egui::PopupCloseBehavior::CloseOnClick).show(|ui| {
                        for h in st.history.clone() {
                            if ui.selectable_label(false, &h).clicked() {
                                look = Some(h);
                            }
                        }
                    });
                    if r.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        look = Some(st.word.clone());
                    }
                    ui.add_space(4.0);
                    if ui.add_enabled(!st.word.trim().is_empty(), egui::Button::new("Look Up").min_size(vec2(86.0, 24.0))).clicked() {
                        look = Some(st.word.clone());
                    }
                    if ui.add_enabled(st.res_sel.is_some(), egui::Button::new(replace_label).min_size(vec2(86.0, 24.0))).clicked() {
                        action = Some("thes_insert");
                    }
                    let resp = ui.add(egui::Button::new("Options ⌄").min_size(vec2(66.0, 24.0)));
                    let oid = ui.make_persistent_id("thes_opts");
                    if resp.clicked() {
                        egui::Popup::toggle_id(ui.ctx(), oid);
                    }
                    egui::Popup::from_response(&resp).id(oid).open_memory(None::<egui::SetOpenCommand>).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                        ui.set_width(150.0);
                        let s = &mut app.settings;
                        if check_item(ui, s.thes_auto_lookup, "Auto Look Up") {
                            s.thes_auto_lookup = !s.thes_auto_lookup;
                        }
                        if check_item(ui, s.thes_spelling_assist, "Spelling Assist") {
                            s.thes_spelling_assist = !s.thes_spelling_assist;
                        }
                        ui.separator();
                        for (i, r) in RELATIONS.iter().enumerate() {
                            if check_item(ui, st.relations[i], r) {
                                st.relations[i] = !st.relations[i];
                            }
                        }
                        ui.separator();
                        if ui.add(egui::Button::new("     Language...").frame(false)).clicked() {
                            st.pick_lang = crate::spell::LANGUAGES.iter().position(|l| l.2 == st.lang);
                            egui::Popup::close_id(ui.ctx(), oid);
                        }
                    });
                });
                ui.add_space(8.0);
                egui::Frame::new().fill(theme::FIELD).stroke(Stroke::new(1.0, theme::SEP)).inner_margin(6.0).show(ui, |ui| { ui.vertical(|ui| {
                    ui.set_min_size(vec2(ui.available_width() - 12.0, 86.0));
                    egui::ScrollArea::vertical().max_height(86.0).show(ui, |ui| {
                        if let Some(w) = &st.looked {
                            if st.results.is_empty() {
                                if st.suggestions.is_empty() {
                                    ui.label(format!("\"{w}\" was not found."));
                                } else {
                                    ui.label(format!("\"{w}\" was not found. Did you mean:"));
                                }
                                for s in st.suggestions.clone() {
                                    if ui.selectable_label(false, format!("    {s}")).double_clicked() {
                                        look = Some(s);
                                    }
                                }
                            } else {
                                ui.label(egui::RichText::new(format!("{w} (synonyms)")).strong());
                                ui.horizontal_wrapped(|ui| {
                                    for (i, s) in st.results.iter().enumerate() {
                                        let r = ui.selectable_label(st.res_sel == Some(i), s);
                                        if r.clicked() {
                                            st.res_sel = Some(i);
                                        }
                                        if r.double_clicked() {
                                            look = Some(s.clone());
                                        }
                                    }
                                });
                            }
                        }
                    });
                });
});
            } else {
                // Spell Checker and Grammatik
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(vec2(330.0, 170.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        if st.tab == 0 {
                            ui.horizontal(|ui| {
                                rlabel(ui, "Replace with:", 130.0, true);
                                ui.add_space(4.0);
                                ui.add(egui::TextEdit::singleline(&mut st.replace_with).desired_width(176.0));
                            });
                            ui.add_space(6.0);
                        }
                        ui.horizontal_top(|ui| {
                            rlabel(ui, "Replacements:", 130.0, true);
                            ui.add_space(4.0);
                            let rows: Vec<Vec<String>> = st.replacements.iter().map(|r| vec![if r.is_empty() { "(delete)".to_string() } else { r.clone() }]).collect();
                            let (c, dbl) = list_box(ui, "wt_reps", vec2(184.0, if st.tab == 0 { 96.0 } else { 72.0 }), &[], &[180.0], &rows, st.rep_sel);
                            if let Some(i) = c {
                                st.rep_sel = Some(i);
                                st.replace_with = st.replacements[i].clone();
                            }
                            if dbl.is_some() {
                                action = Some("replace");
                            }
                        });
                        ui.add_space(6.0);
                        let dim = |s: &str| egui::RichText::new(s.to_string()).color(theme::TEXT);
                        if st.tab == 0 {
                            ui.horizontal(|ui| {
                                rlabel(ui, "Not found:", 130.0, true);
                                ui.add_space(8.0);
                                let t = match (&st.found, st.done) {
                                    (Some(f), _) => f.word.clone(),
                                    (None, true) => "Spell check complete.".into(),
                                    _ => String::new(),
                                };
                                ui.label(dim(&t));
                            });
                        } else {
                            ui.horizontal_top(|ui| {
                                rlabel(ui, "New sentence:", 130.0, true);
                                ui.add_space(8.0);
                                ui.add_sized(vec2(176.0, 34.0), egui::Label::new(dim(&st.found.as_ref().map(|f| f.sentence.clone()).unwrap_or_default())).wrap());
                            });
                            ui.horizontal_top(|ui| {
                                let rule = st.found.as_ref().map(|f| f.rule).unwrap_or("Spelling");
                                rlabel(ui, &format!("{rule}:"), 130.0, true);
                                ui.add_space(8.0);
                                let t = match (&st.found, st.done) {
                                    (Some(f), _) => f.explanation.clone(),
                                    (None, true) => "Grammatik check complete.".into(),
                                    _ => String::new(),
                                };
                                ui.add_sized(vec2(176.0, 34.0), egui::Label::new(dim(&t)).wrap());
                            });
                        }
                    });
                    ui.add_space(10.0);
                    let f = st.found.is_some();
                    egui::Grid::new("wt_buttons").spacing(vec2(6.0, 6.0)).show(ui, |ui| {
                        let b = |ui: &mut Ui, t: &str, en: bool| ui.add_enabled(en, egui::Button::new(t).min_size(vec2(110.0, 24.0))).clicked();
                        if b(ui, "Replace", f) {
                            action = Some("replace");
                        }
                        if b(ui, "Auto Replace", f && st.tab == 0) {
                            action = Some("auto");
                        }
                        ui.end_row();
                        if b(ui, "Skip Once", f) {
                            action = Some("skip");
                        }
                        if b(ui, "Undo", st.undo > 0) {
                            action = Some("undo");
                        }
                        ui.end_row();
                        if b(ui, "Skip All", f) {
                            action = Some("skip_all");
                        }
                        let resp = ui.add(egui::Button::new("Options             ⌄").min_size(vec2(110.0, 24.0)));
                        let oid = ui.make_persistent_id(("wt_opts", st.tab));
                        if resp.clicked() {
                            egui::Popup::toggle_id(ui.ctx(), oid);
                        }
                        let tab = st.tab;
                        egui::Popup::from_response(&resp).id(oid).open_memory(None::<egui::SetOpenCommand>).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                            ui.set_width(220.0);
                            if tab == 0 {
                                for (i, n) in SPELL_OPTIONS.iter().enumerate() {
                                    let on = *spell_option(app, i);
                                    if check_item(ui, on, n) {
                                        *spell_option(app, i) = !on;
                                    }
                                }
                            } else {
                                for (i, n) in ["Auto Start", "Prompt Before Auto Replacement"].iter().enumerate() {
                                    let k = if i == 0 { 0 } else { 6 };
                                    let on = *spell_option(app, k);
                                    if check_item(ui, on, n) {
                                        *spell_option(app, k) = !on;
                                    }
                                }
                                if check_item(ui, st.suggest_spelling, "Suggest Spelling Replacements") {
                                    st.suggest_spelling = !st.suggest_spelling;
                                }
                            }
                            ui.separator();
                            if ui.add(egui::Button::new("     Language...").frame(false)).clicked() {
                                st.pick_lang = crate::spell::LANGUAGES.iter().position(|l| l.2 == st.lang);
                                egui::Popup::close_id(ui.ctx(), oid);
                            }
                        });
                        ui.end_row();
                        if b(ui, "Add", f && st.tab == 0) {
                            action = Some("add");
                        }
                        ui.end_row();
                    });
                });
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(392.0);
                    ui.label("Check:");
                    let mut i = if st.check_doc { 0 } else { 1 };
                    let names: Vec<&str> = if has_text { vec!["Document", "Selected Text"] } else { vec!["Document"] };
                    if combo(ui, "wt_check", &mut i, &names, 106.0) {
                        st.check_doc = i == 0;
                        st.pos = (0, 0);
                        if app.settings.spell_recheck {
                            st.skip_all.clear();
                        }
                        st.next(app);
                    }
                });
            }
        });
        let rect = ERect::from_min_max(pos2(ui.max_rect().left(), ui.max_rect().bottom() - 24.0), ui.max_rect().right_bottom());
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Button::new("?").min_size(vec2(24.0, 24.0)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = button(ui, "Close", true);
                });
            });
        });
    });
    if let Some(w) = look {
        let assist = app.settings.thes_spelling_assist;
        st.look_up(&w, assist);
    }
    match action {
        Some("thes_insert") => {
            if let Some(w) = st.res_sel.and_then(|i| st.results.get(i)).cloned() {
                crate::tools::insert_text(app, &w, None);
            }
        }
        Some("replace") | Some("auto") => {
            if let Some(f) = st.found.clone() {
                let with = if st.tab == 0 { st.replace_with.clone() } else { st.rep_sel.and_then(|i| st.replacements.get(i)).cloned().unwrap_or_default() };
                // a repeated word goes with the space before it
                let (start, len) = if with.is_empty() && f.start > 0 { (f.start - 1, f.len + 1) } else { (f.start, f.len) };
                replace_in(app, f.src, start, len, &with, if st.tab == 0 { "Spell Check" } else { "Grammatik" });
                if action == Some("auto") {
                    crate::spell::auto_correct(&f.word, &with, &st.lang);
                    if app.settings.spell_add_to_quickcorrect && !app.settings.replacements.iter().any(|r| r.0 == f.word) {
                        app.settings.replacements.push((f.word.clone(), with.clone()));
                    }
                }
                st.undo += 1;
                st.pos = (st.pos.0, start + with.chars().count());
                st.next(app);
            }
        }
        Some("skip") => {
            if let Some(f) = &st.found {
                st.pos = (st.pos.0, f.start + f.len.max(1));
            }
            st.next(app);
        }
        Some("skip_all") => {
            if let Some(f) = st.found.clone() {
                crate::spell::ignore(&f.word, &st.lang);
                st.skip_all.push(f.word);
            }
            st.next(app);
        }
        Some("add") => {
            if let Some(f) = st.found.clone() {
                crate::spell::add(&f.word, &st.lang);
                st.skip_all.push(f.word);
            }
            st.next(app);
        }
        Some("undo") => {
            if let Some(d) = app.doc_mut() {
                d.undo();
            }
            st.undo -= 1;
            st.pos = (st.pos.0, 0);
            st.next(app);
        }
        _ => {}
    }
    if !(close || dismissed) {
        app.dialogs.writing = Some(st);
    }
}

// ---------------------------------------------------------------------------------------------
// Text ▸ Writing Tools ▸ Language
// ---------------------------------------------------------------------------------------------

/// The Text Language list with OK and Cancel; returns (ok, cancel).
fn language_list(ui: &mut Ui, sel: &mut usize) -> (bool, bool) {
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.add_space(22.0);
        ui.label("Tag selected text as:");
    });
    ui.add_space(6.0);
    let mut ok = false;
    // just opened (not shown in the frame before): the list scrolls to the current language
    let key = egui::Id::unique("lang_list_shown");
    let now = ui.ctx().cumulative_frame_nr();
    let fresh = ui.ctx().data(|d| d.get_temp::<u64>(key)).is_none_or(|l| l + 1 < now);
    ui.ctx().data_mut(|d| d.insert_temp(key, now));
    ui.horizontal(|ui| {
        ui.add_space(22.0);
        egui::Frame::new().fill(theme::FIELD).stroke(Stroke::new(1.0, theme::SEP)).show(ui, |ui| { ui.vertical(|ui| {
            ui.set_min_size(vec2(310.0, 124.0));
            ui.set_max_size(vec2(310.0, 124.0));
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::vertical().id_salt("lang_list").auto_shrink([false, false]).show(ui, |ui| {
                for (i, (name, code, _)) in crate::spell::LANGUAGES.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(vec2(296.0, 25.0), Sense::click());
                    if *sel == i {
                        ui.painter().rect_filled(r, 0.0, theme::ACCENT);
                        if fresh {
                            ui.scroll_to_rect(r, Some(egui::Align::Center));
                        }
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, 0.0, theme::HOVER);
                    }
                    ui.painter().text(pos2(r.left() + 5.0, r.center().y), egui::Align2::LEFT_CENTER, *name, egui::FontId::proportional(12.5), theme::TEXT);
                    ui.painter().text(pos2(r.left() + 245.0, r.center().y), egui::Align2::LEFT_CENTER, *code, egui::FontId::proportional(12.5), theme::TEXT);
                    if resp.clicked() {
                        *sel = i;
                    }
                    if resp.double_clicked() {
                        ok = true;
                    }
                }
            });
        });
});
    });
    let (o, c) = ok_cancel_plain(ui, true);
    (ok || o, c)
}

pub struct LanguageState {
    pub sel: usize,
}

impl LanguageState {
    pub fn new(app: &AnyVectorApp) -> LanguageState {
        let tag = crate::spell::lang_of(&first_text(app).format.lang);
        LanguageState { sel: crate::spell::LANGUAGES.iter().position(|l| l.2 == tag).unwrap_or(0) }
    }
}

/// Apply a character format change to the selected characters (text being edited) or whole texts and
/// the text of selected tables.
pub fn format_chars(app: &mut AnyVectorApp, label: &str, f: impl Fn(&mut vt::CharFormat)) {
    let Some(d) = app.doc_mut() else { return };
    if let Some(te) = d.text_edit.clone() {
        let (a, b) = (te.caret.min(te.anchor), te.caret.max(te.anchor));
        d.modify(label, |doc, _| {
            if let Some(ObjKind::Text(t)) = doc.find_mut(te.obj).map(|o| &mut o.kind) {
                let (a, b) = if a == b { (0, t.char_count()) } else { (a, b) };
                if a == 0 && b >= t.char_count() {
                    f(&mut t.format);
                    for s in t.spans.iter_mut() {
                        f(&mut s.format);
                    }
                } else {
                    t.apply_format(a, b, &f);
                }
            }
        });
        return;
    }
    let ids = d.selection.clone();
    d.modify(label, |doc, _| {
        for id in &ids {
            if let Some(o) = doc.find_mut(*id) {
                o.walk_mut(&mut |x| match &mut x.kind {
                    ObjKind::Text(t) => {
                        f(&mut t.format);
                        for s in t.spans.iter_mut() {
                            f(&mut s.format);
                        }
                    }
                    ObjKind::Table(t) => f(&mut t.text_format),
                    _ => {}
                });
            }
        }
    });
}

pub fn language(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.text_lang.take() else { return };
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "text_lang", "Text Language", vec2(390.0, 263.0), |ui| {
        (ok, cancel) = language_list(ui, &mut st.sel);
    });
    if ok {
        let tag = crate::spell::LANGUAGES[st.sel].2.to_string();
        format_chars(app, "Text Language", |f| f.lang = tag.clone());
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.text_lang = Some(st);
    }
}

// ---------------------------------------------------------------------------------------------
// Text ▸ Change Case
// ---------------------------------------------------------------------------------------------

/// The Change Case choices in dialog order, as `change_case` modes (0 lower, 1 upper, 2 title,
/// 3 sentence, 4 toggle).
pub const CASE_MODES: [(u8, &str); 5] = [(3, "Sentence case"), (0, "lowercase"), (1, "UPPERCASE"), (2, "Title Case"), (4, "tOGGLE cASE")];

pub fn change_case_text(s: &str, mode: u8) -> String {
    match mode {
        0 => s.to_lowercase(),
        1 => s.to_uppercase(),
        2 => {
            let mut out = String::with_capacity(s.len());
            let mut start = true;
            for c in s.chars() {
                if c.is_alphanumeric() {
                    if start {
                        out.extend(c.to_uppercase());
                    } else {
                        out.extend(c.to_lowercase());
                    }
                    start = false;
                } else {
                    out.push(c);
                    start = c.is_whitespace() || c == '-';
                }
            }
            out
        }
        3 => {
            let mut out = String::with_capacity(s.len());
            let mut start = true;
            for c in s.chars() {
                if c.is_alphabetic() {
                    if start {
                        out.extend(c.to_uppercase());
                    } else {
                        out.extend(c.to_lowercase());
                    }
                    start = false;
                } else {
                    out.push(c);
                    if matches!(c, '.' | '!' | '?' | '\n') {
                        start = true;
                    }
                }
            }
            out
        }
        _ => s.chars().map(|c| if c.is_uppercase() { c.to_lowercase().next().unwrap_or(c) } else { c.to_uppercase().next().unwrap_or(c) }).collect(),
    }
}

/// Change the case of the selected characters (text being edited), or of the selected texts and table
/// cells.
pub fn apply_case(app: &mut AnyVectorApp, mode: u8) {
    let Some(d) = app.doc_mut() else { return };
    if let Some(te) = d.text_edit.clone() {
        let (a, b) = (te.caret.min(te.anchor), te.caret.max(te.anchor));
        d.modify("Change Case", |doc, _| {
            if let Some(ObjKind::Text(t)) = doc.find_mut(te.obj).map(|o| &mut o.kind) {
                let chars: Vec<char> = t.content.chars().collect();
                let (a, b) = if a == b { (0, chars.len()) } else { (a.min(chars.len()), b.min(chars.len())) };
                let mid: String = chars[a..b].iter().collect();
                let new = format!("{}{}{}", chars[..a].iter().collect::<String>(), change_case_text(&mid, mode), chars[b..].iter().collect::<String>());
                set_content(t, &new);
            }
        });
        return;
    }
    let ids = d.selection.clone();
    d.modify("Change Case", |doc, _| {
        for id in &ids {
            if let Some(o) = doc.find_mut(*id) {
                o.walk_mut(&mut |x| match &mut x.kind {
                    ObjKind::Text(t) => {
                        let new = change_case_text(&t.content, mode);
                        set_content(t, &new);
                    }
                    ObjKind::Table(t) => {
                        for c in t.cells.iter_mut() {
                            c.text = change_case_text(&c.text, mode);
                        }
                    }
                    _ => {}
                });
            }
        }
    });
}

pub fn change_case(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut sel) = app.dialogs.change_case.take() else { return };
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "change_case", "Change Case", vec2(240.0, 214.0), |ui| {
        ui.add_space(8.0);
        for (i, (_, name)) in CASE_MODES.iter().enumerate() {
            ui.radio_value(&mut sel, i, *name);
            ui.add_space(2.0);
        }
        (ok, cancel) = ok_cancel_row(ui, "OK", true);
    });
    if ok {
        apply_case(app, CASE_MODES[sel].0);
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.change_case = Some(sel);
    }
}

// ---------------------------------------------------------------------------------------------
// Text ▸ Encode
// ---------------------------------------------------------------------------------------------

pub struct EncodeState {
    pub other: bool,
    pub pages: Vec<(u32, String)>,
    pub sel: usize,
    /// The text as it is now.
    pub text: String,
    pub scrolled: bool,
}

impl EncodeState {
    pub fn new(app: &AnyVectorApp) -> EncodeState {
        let pages = crate::spell::code_pages();
        let acp = crate::spell::ansi_code_page();
        let sel = pages.iter().position(|p| p.0 == acp).unwrap_or(0);
        let text = sources(app, false).into_iter().filter_map(|s| source_text(app, s)).collect::<Vec<_>>().join("\n");
        EncodeState { other: false, pages, sel, text, scrolled: false }
    }

    /// The text read again in the chosen code page.
    pub fn preview(&self, s: &str) -> String {
        if !self.other {
            return s.to_string();
        }
        let cp = self.pages.get(self.sel).map(|p| p.0).unwrap_or(1252);
        crate::spell::decode(&crate::spell::encode(s, crate::spell::ansi_code_page()), cp)
    }
}

pub fn encode(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.encode.take() else { return };
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "encode", "Text Encoding", vec2(696.0, 540.0), |ui| {
        ui.add_space(6.0);
        ui.label("Select the encoding which makes your text readable:");
        ui.add_space(4.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(318.0, 420.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.radio_value(&mut st.other, false, "Current");
                ui.radio_value(&mut st.other, true, "Other encoding");
                ui.add_space(4.0);
                egui::Frame::new().fill(theme::FIELD).show(ui, |ui| { ui.vertical(|ui| {
                    ui.set_min_size(vec2(318.0, 360.0));
                    ui.set_max_size(vec2(318.0, 360.0));
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let enabled = st.other;
                    egui::ScrollArea::vertical().id_salt("cp_list").auto_shrink([false, false]).show(ui, |ui| {
                        for (i, (_, name)) in st.pages.iter().enumerate() {
                            let (r, resp) = ui.allocate_exact_size(vec2(302.0, 25.0), Sense::click());
                            if st.sel == i {
                                ui.painter().rect_filled(r, 0.0, if enabled { theme::ACCENT } else { theme::HOVER });
                                if !st.scrolled {
                                    ui.scroll_to_rect(r, Some(egui::Align::TOP));
                                }
                            }
                            ui.painter().text(pos2(r.left() + 5.0, r.center().y), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(12.5), if enabled { theme::TEXT } else { theme::TEXT_DIM });
                            if resp.clicked() && enabled {
                                st.sel = i;
                            }
                        }
                    });
                });
});
            });
            ui.add_space(12.0);
            ui.allocate_ui_with_layout(vec2(340.0, 420.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.add_space(26.0);
                ui.label("Preview:");
                ui.add_space(4.0);
                egui::Frame::new().fill(theme::FIELD).inner_margin(6.0).show(ui, |ui| { ui.vertical(|ui| {
                    ui.set_min_size(vec2(328.0, 370.0));
                    ui.set_max_width(328.0);
                    ui.label(egui::RichText::new(st.preview(&st.text)).color(theme::TEXT_DIM));
                });
});
            });
        });
        st.scrolled = true;
        (ok, cancel) = ok_cancel_row(ui, "OK", true);
    });
    if ok {
        if st.other {
            let srcs = sources(app, false);
            let cp = st.pages.get(st.sel).map(|p| p.0).unwrap_or(1252);
            let acp = crate::spell::ansi_code_page();
            let texts: Vec<(Source, String)> = srcs.into_iter().filter_map(|s| source_text(app, s).map(|t| (s, crate::spell::decode(&crate::spell::encode(&t, acp), cp)))).collect();
            if let Some(d) = app.doc_mut() {
                d.modify("Encode", |doc, _| {
                    for (s, new) in &texts {
                        match doc.find_mut(s.obj).map(|o| &mut o.kind) {
                            Some(ObjKind::Text(t)) => set_content(t, new),
                            Some(ObjKind::Table(t)) => {
                                if let Some(c) = s.cell.and_then(|i| t.cells.get_mut(i)) {
                                    c.text = new.clone();
                                }
                            }
                            _ => {}
                        }
                    }
                });
            }
        }
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.encode = Some(st);
    }
}

// ---------------------------------------------------------------------------------------------
// Text ▸ Edit Text
// ---------------------------------------------------------------------------------------------

pub struct EditTextState {
    pub obj: ObjId,
    /// The text being edited (applied with OK).
    pub t: TextObj,
    /// Its characters, as the edit box has them.
    pub plain: String,
    pub zoom: f32,
    pub show_np: bool,
}

impl EditTextState {
    pub fn new(app: &AnyVectorApp) -> Option<EditTextState> {
        let d = app.doc()?;
        let id = target_ids(app).first().map(|id| chain_head(&d.doc, *id))?;
        let ObjKind::Text(t) = &d.doc.find(id)?.kind else { return None };
        Some(EditTextState { obj: id, plain: t.content.clone(), t: t.clone(), zoom: 1.0, show_np: false })
    }
}

fn fmt_num(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

const SIZES: [f64; 20] = [6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 24.0, 30.0, 36.0, 48.0, 60.0, 72.0, 100.0, 150.0, 200.0, 300.0];

pub fn edit_text(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.edit_text.take() else { return };
    let fams: Vec<String> = vt::fonts().lock().families().to_vec();
    let edit_id = egui::Id::unique("edit_text_box");
    // the characters selected in the edit box
    let range = egui::TextEdit::load_state(ctx, edit_id).and_then(|s| s.cursor.char_range()).map(|r| {
        let (a, b) = (r.primary.index.0, r.secondary.index.0);
        (a.min(b), a.max(b))
    });
    let n = st.t.char_count();
    let (sa, sb) = match range {
        Some((a, b)) if b > a => (a.min(n), b.min(n)),
        _ => (0, n),
    };
    let cur = st.t.format_at(sa.min(n.saturating_sub(1))).clone();
    let spell = if app.settings.spell_check { crate::spell::check(&st.plain, crate::spell::lang_of(&st.t.format.lang)) } else { vec![] };
    let (mut ok, mut cancel, mut import) = (false, false, false);
    let ((), dismissed) = window(ctx, "edit_text", "Edit Text", vec2(721.0, 522.0), |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let mut font = cur.font.clone();
            egui::ComboBox::from_id_salt("et_font").selected_text(&font).width(160.0).height(320.0).show_ui(ui, |ui| {
                for f in &fams {
                    ui.selectable_value(&mut font, f.clone(), f);
                }
            });
            if font != cur.font {
                st.t.apply_format(sa, sb.max(sa + 1).min(n.max(1)), |c| c.font = font.clone());
                if n == 0 || (sa == 0 && sb == n) {
                    st.t.format.font = font.clone();
                }
            }
            let mut size = cur.size;
            egui::ComboBox::from_id_salt("et_size").selected_text(format!("{} pt", fmt_num(size))).width(96.0).show_ui(ui, |ui| {
                for s in SIZES {
                    ui.selectable_value(&mut size, s, format!("{} pt", fmt_num(s)));
                }
            });
            if size != cur.size {
                st.t.apply_format(sa, sb.max(sa + 1).min(n.max(1)), |c| c.size = size);
                if n == 0 || (sa == 0 && sb == n) {
                    st.t.format.size = size;
                }
            }
            let toggle = |ui: &mut Ui, label: egui::RichText, on: bool| ui.add(egui::Button::new(label).selected(on).min_size(vec2(26.0, 24.0))).clicked();
            if toggle(ui, egui::RichText::new("B").strong().size(15.0), cur.bold) {
                let v = !cur.bold;
                st.t.apply_format(sa, sb, |c| c.bold = v);
            }
            if toggle(ui, egui::RichText::new("I").italics().size(15.0), cur.italic) {
                let v = !cur.italic;
                st.t.apply_format(sa, sb, |c| c.italic = v);
            }
            if toggle(ui, egui::RichText::new("U").underline().size(15.0), cur.underline) {
                let v = !cur.underline;
                st.t.apply_format(sa, sb, |c| c.underline = v);
            }
            // horizontal alignment
            let names = ["None", "Left", "Center", "Right", "Full justify", "Force justify"];
            let aligns = [vt::TextAlign::Left, vt::TextAlign::Left, vt::TextAlign::Center, vt::TextAlign::Right, vt::TextAlign::Justify, vt::TextAlign::ForceJustify];
            let cur_al = aligns.iter().skip(1).position(|a| *a == st.t.align).map(|i| i + 1).unwrap_or(1);
            let mut al = cur_al;
            egui::ComboBox::from_id_salt("et_align").selected_text(names[al]).width(80.0).show_ui(ui, |ui| {
                for (i, nm) in names.iter().enumerate() {
                    ui.selectable_value(&mut al, i, *nm);
                }
            });
            if al != cur_al {
                st.t.align = aligns[al];
            }
            if toggle(ui, egui::RichText::new("¶").size(16.0), st.show_np) {
                st.show_np = !st.show_np;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("100").min_size(vec2(26.0, 24.0))).on_hover_text("Zoom 100%").clicked() {
                    st.zoom = 1.0;
                }
                if ui.add(egui::Button::new("−").min_size(vec2(26.0, 24.0))).on_hover_text("Zoom out").clicked() {
                    st.zoom = (st.zoom / 1.25).max(0.25);
                }
                if ui.add(egui::Button::new("+").min_size(vec2(26.0, 24.0))).on_hover_text("Zoom in").clicked() {
                    st.zoom = (st.zoom * 1.25).min(8.0);
                }
            });
        });
        ui.add_space(8.0);
        // the text, on white, in its sizes and colors; words not in the dictionary underlined in red
        let t = st.t.clone();
        let zoom = st.zoom;
        let show_np = st.show_np;
        let errs: Vec<(usize, usize)> = spell.iter().filter(|e| !e.delete).map(|e| (e.start, e.start + e.len)).collect();
        let mut layouter = move |ui: &Ui, text: &dyn egui::TextBuffer, wrap: f32| {
            let s = text.as_str();
            let mut job = egui::text::LayoutJob::default();
            for (i, ch) in s.chars().enumerate() {
                let f = t.format_at(i.min(t.char_count().saturating_sub(1)));
                let col = f.color.map(theme::to_c32).unwrap_or(Color32::BLACK);
                let px = (f.size as f32 * zoom).clamp(8.0, 120.0);
                let bad = errs.iter().any(|e| i >= e.0 && i < e.1);
                let fmt = egui::TextFormat {
                    font_id: egui::FontId::proportional(px),
                    color: if show_np && (ch == ' ' || ch == '\t') { Color32::from_rgb(0x1e, 0x88, 0xe5) } else { col },
                    italics: f.italic,
                    underline: if bad { Stroke::new(1.0, Color32::RED) } else if f.underline { Stroke::new(1.0, col) } else { Stroke::NONE },
                    strikethrough: if f.strike { Stroke::new(1.0, col) } else { Stroke::NONE },
                    background: if show_np && (ch == ' ' || ch == '\t' || ch == vt::NBSP) { Color32::from_rgb(0xe3, 0xf0, 0xff) } else { Color32::TRANSPARENT },
                    ..Default::default()
                };
                let mut b = [0u8; 4];
                job.append(ch.encode_utf8(&mut b), 0.0, fmt);
            }
            job.wrap.max_width = wrap;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let r = ui.available_rect_before_wrap();
        let area = ERect::from_min_max(r.min, pos2(r.right(), r.bottom() - 40.0));
        ui.painter().rect_filled(area, 0.0, Color32::WHITE);
        ui.painter().rect_stroke(area, 0.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Outside);
        let before = st.plain.clone();
        ui.scope_builder(egui::UiBuilder::new().max_rect(area.shrink(3.0)), |ui| {
            egui::ScrollArea::vertical().id_salt("et_scroll").auto_shrink([false, false]).show(ui, |ui| {
                ui.visuals_mut().selection.bg_fill = Color32::from_rgb(0xb4, 0xd5, 0xfe);
                ui.visuals_mut().text_cursor.stroke = Stroke::new(1.5, Color32::BLACK);
                ui.add(egui::TextEdit::multiline(&mut st.plain).id(edit_id).frame(egui::Frame::NONE).desired_width(f32::INFINITY).desired_rows(20).layouter(&mut layouter));
            });
        });
        if st.plain != before {
            let p = st.plain.clone();
            set_content(&mut st.t, &p);
        }
        let rect = ERect::from_min_max(pos2(ui.max_rect().left(), ui.max_rect().bottom() - 26.0), ui.max_rect().right_bottom());
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::Button::new("Help").min_size(vec2(86.0, 26.0)));
                cancel = ui.add(egui::Button::new("Cancel").min_size(vec2(86.0, 26.0))).clicked();
                ok = ui.add(egui::Button::new("OK").min_size(vec2(86.0, 26.0))).clicked();
                import = ui.add(egui::Button::new("Import...").min_size(vec2(86.0, 26.0))).clicked();
                let resp = ui.add(egui::Button::new("Options  ⌄").min_size(vec2(86.0, 26.0)));
                let oid = ui.make_persistent_id("et_opts");
                if resp.clicked() {
                    egui::Popup::toggle_id(ui.ctx(), oid);
                }
                egui::Popup::from_response(&resp).id(oid).open_memory(None::<egui::SetOpenCommand>).close_behavior(egui::PopupCloseBehavior::CloseOnClick).show(|ui| {
                    ui.set_width(150.0);
                    ui.label(egui::RichText::new("Change Case").color(theme::TEXT_DIM));
                    for (mode, name) in CASE_MODES {
                        if ui.add(egui::Button::new(format!("   {name}")).frame(false)).clicked() {
                            let chars: Vec<char> = st.plain.chars().collect();
                            let mid: String = chars[sa..sb].iter().collect();
                            let new = format!("{}{}{}", chars[..sa].iter().collect::<String>(), change_case_text(&mid, mode), chars[sb..].iter().collect::<String>());
                            st.plain = new.clone();
                            set_content(&mut st.t, &new);
                        }
                    }
                });
            });
        });
    });
    if import {
        if let Some(p) = rfd::FileDialog::new().add_filter("Text", &["txt"]).pick_file() {
            if let Ok(s) = std::fs::read_to_string(p) {
                let s = s.replace("\r\n", "\n");
                st.t.insert(sa, &s);
                st.plain = st.t.content.clone();
            }
        }
    }
    if ok {
        let (id, t) = (st.obj, st.t.clone());
        if let Some(d) = app.doc_mut() {
            d.modify("Edit Text", |doc, _| {
                if let Some(ObjKind::Text(x)) = doc.find_mut(id).map(|o| &mut o.kind) {
                    x.content = t.content.clone();
                    x.spans = t.spans.clone();
                    x.format = t.format.clone();
                    x.align = t.align;
                }
            });
            if let Some(te) = d.text_edit.as_mut() {
                let n = t.content.chars().count();
                te.caret = te.caret.min(n);
                te.anchor = te.anchor.min(n);
            }
        }
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.edit_text = Some(st);
    }
}

// ---------------------------------------------------------------------------------------------
// Typing: linked frames, QuickCorrect and custom optional hyphens
// ---------------------------------------------------------------------------------------------

/// The text that holds the characters of a frame, and where the frame's characters start in it: the
/// first frame of linked frames holds the text of all of them.
pub fn head_offset(doc: &Document, id: ObjId) -> (ObjId, usize) {
    let head = chain_head(doc, id);
    if head == id {
        return (id, 0);
    }
    match doc.find(id).map(|o| &o.kind) {
        Some(ObjKind::Text(t)) => (head, t.flow.start),
        _ => (id, 0),
    }
}

/// Replace characters [a, b) of a text frame with `s` (in linked frames the first frame changes and the
/// others follow).
pub fn edit_chain(doc: &mut Document, id: ObjId, a: usize, b: usize, s: &str) {
    let (head, off) = head_offset(doc, id);
    let mut linked = head != id;
    if let Some(ObjKind::Text(t)) = doc.find_mut(head).map(|o| &mut o.kind) {
        let n = t.char_count();
        let (a, b) = ((a + off).min(n), (b + off).min(n));
        if b > a {
            t.delete(a, b);
        }
        if !s.is_empty() {
            t.insert(a, s);
        }
        linked |= t.link_next.is_some();
    }
    if linked {
        doc.update_text_flow();
    }
}

const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// QuickCorrect (Tools ▸ Options ▸ Text ▸ QuickCorrect) and custom optional hyphens as you type: the
/// characters before the caret to replace and what goes in place of them and of `typed`.
pub fn quick_correct(s: &crate::settings::Settings, before: &[char], typed: &str) -> (usize, String) {
    let mut it = typed.chars();
    let (Some(c), None) = (it.next(), it.next()) else {
        // pasted text
        if s.hyphen_when_pasting && !s.hyphen_defs.is_empty() {
            if let Some(h) = apply_hyphen_defs(typed, &s.hyphen_defs) {
                return (0, h);
            }
        }
        return (0, typed.to_string());
    };
    let prev = before.last().copied();
    if s.typographic_quotes && (c == '\'' || c == '"') {
        if s.straight_after_numbers && prev.is_some_and(|p| p.is_ascii_digit()) {
            return (0, typed.to_string());
        }
        let open = prev.is_none_or(|p| p.is_whitespace() || vt::is_para_end(p) || "([{<“‘«".contains(p));
        let (o, cl) = if c == '\'' { &s.quote_single } else { &s.quote_double };
        return (0, if open { o.clone() } else { cl.clone() });
    }
    let ends_word = c.is_whitespace() || vt::is_para_end(c) || ".,;:!?)".contains(c);
    if !ends_word {
        return (0, typed.to_string());
    }
    let k = before.iter().rev().take_while(|ch| !ch.is_whitespace() && !vt::is_para_end(**ch)).count();
    if k == 0 {
        return (0, typed.to_string());
    }
    let word: String = before[before.len() - k..].iter().collect();
    let mut new = word.clone();
    if s.replace_while_typing {
        if let Some((_, r)) = s.replacements.iter().find(|r| r.0 == word) {
            new = r.clone();
        }
    }
    if new == word {
        if s.quickcorrect_days {
            if let Some(d) = DAYS.iter().find(|d| d.to_lowercase() == word) {
                new = d.to_string();
            }
        }
        let cs: Vec<char> = new.chars().collect();
        if s.quickcorrect_two_caps && cs.len() >= 3 && cs[0].is_uppercase() && cs[1].is_uppercase() && cs[2..].iter().all(|c| c.is_lowercase()) {
            new = std::iter::once(cs[0]).chain(cs[1].to_lowercase()).chain(cs[2..].iter().copied()).collect();
        }
        if s.quickcorrect_capitalize {
            let rest = &before[..before.len() - k];
            let p = rest.iter().rev().find(|c| **c != ' ' && **c != '\t').copied();
            let start = p.is_none_or(|p| vt::is_para_end(p) || matches!(p, '.' | '!' | '?'));
            let mut cs = new.chars();
            if let Some(f) = cs.next() {
                if start && f.is_lowercase() {
                    new = f.to_uppercase().chain(cs).collect();
                }
            }
        }
        if s.hyphen_when_typing && !s.hyphen_defs.is_empty() {
            if let Some(h) = apply_hyphen_defs(&new, &s.hyphen_defs) {
                new = h;
            }
        }
    }
    if new == word {
        return (0, typed.to_string());
    }
    (k, new + typed)
}

// ---------------------------------------------------------------------------------------------
// Text menu commands
// ---------------------------------------------------------------------------------------------

/// Text ▸ Insert Formatting Code: the character each item inserts.
pub fn format_code(k: u8) -> char {
    match k {
        0 => vt::EM_SPACE,
        1 => vt::EN_SPACE,
        2 => vt::QUARTER_EM_SPACE,
        3 => vt::NBSP,
        4 => '\t',
        5 => vt::COLUMN_BREAK,
        6 => vt::EM_DASH,
        7 => vt::EN_DASH,
        8 => vt::NB_HYPHEN,
        _ => vt::SOFT_HYPHEN,
    }
}

fn scale_sizes(t: &mut TextObj, k: f64) {
    t.format.size *= k;
    for s in t.spans.iter_mut() {
        s.format.size *= k;
    }
}

/// The largest factor (to 0.1%) for which `fits` holds.
fn largest_fit(fits: impl Fn(f64) -> bool) -> f64 {
    let (mut lo, mut hi) = (0.01, 1.0);
    while fits(hi) && hi < 1000.0 {
        lo = hi;
        hi *= 2.0;
    }
    if !fits(lo) {
        return lo;
    }
    for _ in 0..30 {
        let mid = (lo + hi) / 2.0;
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi / lo < 1.001 {
            break;
        }
    }
    lo
}

/// The text of a table cell as the renderer lays it out.
fn cell_text(t: &vector_core::object::TableObj, r: usize, c: usize, size: f64) -> TextObj {
    let rc = t.span_rect(r, c);
    let mut f = t.text_format.clone();
    f.size = size;
    TextObj::paragraph(&t.cell(r, c).text, (rc.width() - 4.0).max(1.0), (rc.height() - 4.0).max(1.0), f)
}

/// Text ▸ Fit Text to Frame: the text gets the size that fills its frame (in tables, the size that fits
/// every cell).
pub fn fit_text_to_frame(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    let ids = d.selection.clone();
    d.modify("Fit Text to Frame", |doc, _| {
        for id in &ids {
            match doc.find_mut(*id).map(|o| &mut o.kind) {
                Some(ObjKind::Text(t)) if matches!(t.kind, TextKind::Paragraph { .. }) && t.link_prev.is_none() && t.char_count() > 0 => {
                    let base = t.clone();
                    let n = base.char_count();
                    let k = largest_fit(|k| {
                        let mut x = base.clone();
                        scale_sizes(&mut x, k);
                        vt::layout(&x).fit_end >= n
                    });
                    scale_sizes(t, k);
                }
                Some(ObjKind::Table(t)) => {
                    let size = t.text_format.size;
                    let cells: Vec<(usize, usize)> = t.anchors().into_iter().filter(|(r, c)| !t.cell(*r, *c).text.is_empty()).collect();
                    if cells.is_empty() {
                        continue;
                    }
                    let tt = t.clone();
                    let k = largest_fit(|k| {
                        cells.iter().all(|(r, c)| {
                            let x = cell_text(&tt, *r, *c, size * k);
                            vt::layout(&x).fit_end >= x.char_count()
                        })
                    });
                    t.text_format.size = size * k;
                }
                _ => {}
            }
        }
    });
}

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetuer adipiscing elit, sed diam nonummy nibh euismod tincidunt ut laoreet dolore magna aliquam erat volutpat. Ut wisi enim ad minim veniam, quis nostrud exerci tation ullamcorper suscipit lobortis nisl ut aliquip ex ea commodo consequat. Duis autem vel eum iriure dolor in hendrerit in vulputate velit esse molestie consequat, vel illum dolore eu feugiat nulla facilisis at vero eros et accumsan et iusto odio dignissim qui blandit praesent luptatum zzril delenit augue duis dolore te feugait nulla facilisi.";

/// Placeholder text that fills a frame: whole words, as many as fit.
pub fn placeholder_for(fits: impl Fn(&str) -> bool) -> String {
    let words: Vec<&str> = LOREM.split(' ').collect();
    let mut out = String::new();
    let mut i = 0;
    loop {
        let w = words[i % words.len()];
        let next = if out.is_empty() { w.to_string() } else { format!("{out} {w}") };
        if !fits(&next) || i > 5000 {
            break;
        }
        out = next;
        i += 1;
    }
    out
}

/// Text ▸ Insert Placeholder Text: empty paragraph frames and table cells fill with placeholder text.
pub fn insert_placeholder(app: &mut AnyVectorApp) {
    let cell = app.ts.table_cell;
    let cell_to = app.ts.table_cell_to;
    let Some(d) = app.doc_mut() else { return };
    let mut ids = d.selection.clone();
    if let Some(te) = &d.text_edit {
        ids = vec![te.obj];
    }
    d.modify("Insert Placeholder Text", |doc, _| {
        for id in &ids {
            match doc.find_mut(*id).map(|o| &mut o.kind) {
                Some(ObjKind::Text(t)) if matches!(t.kind, TextKind::Paragraph { .. }) && t.char_count() == 0 => {
                    let base = t.clone();
                    let s = placeholder_for(|s| {
                        let mut x = base.clone();
                        x.insert(0, s);
                        vt::layout(&x).fit_end >= x.char_count()
                    });
                    t.insert(0, &s);
                }
                Some(ObjKind::Table(t)) => {
                    let range = crate::tools::range_of(t, cell, cell_to);
                    let cells: Vec<(usize, usize)> = match range {
                        Some(g) => t.anchors_in(g),
                        None => t.anchors(),
                    };
                    for (r, c) in cells {
                        if !t.cell(r, c).text.is_empty() {
                            continue;
                        }
                        let size = t.text_format.size;
                        let tt = t.clone();
                        let s = placeholder_for(|s| {
                            let mut x = cell_text(&tt, r, c, size);
                            x.insert(0, s);
                            vt::layout(&x).fit_end >= x.char_count()
                        });
                        t.cell_mut(r, c).text = s;
                    }
                }
                _ => {}
            }
        }
    });
}

/// Text ▸ Paragraph Text Frame ▸ Create Empty Text Frame: an empty paragraph frame in the shape of each
/// selected object, ready for typing.
pub fn create_empty_frame(app: &mut AnyVectorApp) {
    let fmt = app.ts.text_format.clone();
    let defaults = app.ts.text_para.clone();
    let now = app.time();
    let Some(d) = app.doc_mut() else { return };
    let mut made = vec![];
    for id in d.selection.clone() {
        let Some(o) = d.doc.find(id) else { continue };
        let wp = match &o.kind {
            ObjKind::PowerClip { frame, .. } => frame.world_path(),
            _ => o.world_path(),
        };
        let r = kurbo::Shape::bounding_box(&wp);
        if r.width() < 1.0 || r.height() < 1.0 {
            continue;
        }
        let mut local = wp.clone();
        local.apply_affine(kurbo::Affine::translate((-r.x0, -r.y0)));
        let mut t = TextObj::paragraph("", r.width(), r.height(), fmt.clone());
        apply_defaults(&mut t, &defaults);
        t.frame_shape = Some(vector_core::path::NodePath::from_bez(&local));
        let nid = d.doc.new_id();
        made.push(vector_core::Object::new(nid, ObjKind::Text(t), kurbo::Affine::translate((r.x0, r.y0)), vector_core::Style::text_default()));
    }
    let first = made.first().map(|o| o.id);
    if !made.is_empty() {
        d.add_objects("Create Empty Text Frame", made);
    }
    if let Some(id) = first {
        d.text_edit = Some(crate::docview::TextEdit { obj: id, caret: 0, anchor: 0, blink: now });
        app.set_tool(crate::tools::ToolId::Text);
    }
}

/// A part of a text: characters [a, b) with their formatting.
fn slice_text(src: &TextObj, a: usize, b: usize) -> (String, Vec<vt::Span>) {
    let chars: Vec<char> = src.content.chars().collect();
    let (a, b) = (a.min(chars.len()), b.min(chars.len()).max(a.min(chars.len())));
    let spans = src
        .spans
        .iter()
        .filter(|s| s.end > a && s.start < b)
        .map(|s| vt::Span { start: s.start.max(a) - a, end: s.end.min(b) - a, format: s.format.clone() })
        .collect();
    (chars[a..b].iter().collect(), spans)
}

/// The frames of a chain, from its first frame.
fn chain_of(doc: &Document, head: ObjId) -> Vec<ObjId> {
    let mut out = vec![head];
    let mut cur = head;
    while let Some(ObjKind::Text(t)) = doc.find(cur).map(|o| &o.kind) {
        match t.link_next {
            Some(n) if !out.contains(&n) => {
                out.push(n);
                cur = n;
            }
            _ => break,
        }
    }
    out
}

/// Text ▸ Paragraph Text Frame ▸ Link: the text flows from the first selected frame (or the end of its
/// chain) into the others in the order they were selected; their own text goes to the end of it.
pub fn link_frames(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    let sel = d.selection.clone();
    if sel.len() < 2 {
        return;
    }
    d.modify("Link", |doc, _| {
        let head = chain_head(doc, sel[0]);
        let mut tail = *chain_of(doc, head).last().unwrap_or(&head);
        for id in &sel[1..] {
            if chain_of(doc, head).contains(id) {
                continue;
            }
            let Some(ObjKind::Text(add)) = doc.find(*id).map(|o| o.kind.clone()) else { continue };
            if add.link_prev.is_some() || !matches!(add.kind, TextKind::Paragraph { .. }) {
                continue;
            }
            // the frame's own text (and its chain's) goes after the text that flows into it
            if let Some(ObjKind::Text(h)) = doc.find_mut(head).map(|o| &mut o.kind) {
                if !add.content.is_empty() {
                    let mut at = h.char_count();
                    if at > 0 && !h.content.ends_with(|c: char| vt::is_para_end(c)) {
                        h.insert(at, "\n");
                        at += 1;
                    }
                    let n = add.char_count();
                    h.insert(at, &add.content);
                    let f = add.format.clone();
                    h.apply_format(at, at + n, |x| *x = f.clone());
                    for s in &add.spans {
                        h.apply_format(at + s.start, at + s.end, |x| *x = s.format.clone());
                    }
                }
            }
            if let Some(ObjKind::Text(t)) = doc.find_mut(tail).map(|o| &mut o.kind) {
                t.link_next = Some(*id);
            }
            if let Some(ObjKind::Text(t)) = doc.find_mut(*id).map(|o| &mut o.kind) {
                t.link_prev = Some(tail);
                t.content.clear();
                t.spans.clear();
            }
            // a frame that led a chain brings the rest of it along
            tail = *chain_of(doc, *id).last().unwrap_or(id);
        }
        doc.update_text_flow();
    });
}

/// Text ▸ Paragraph Text Frame ▸ Unlink: the selected frames stop sharing text with the frames before and
/// after them; each keeps the text it shows (the last one keeps the rest).
pub fn unlink_frames(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    let sel = d.selection.clone();
    d.modify("Unlink", |doc, _| {
        let mut heads: Vec<ObjId> = sel.iter().map(|id| chain_head(doc, *id)).collect();
        heads.dedup();
        for head in heads {
            let frames = chain_of(doc, head);
            if frames.len() < 2 {
                continue;
            }
            let Some(ObjKind::Text(src)) = doc.find(head).map(|o| o.kind.clone()) else { continue };
            let starts: Vec<usize> = frames
                .iter()
                .map(|id| match doc.find(*id).map(|o| &o.kind) {
                    Some(ObjKind::Text(t)) if *id != head => t.flow.start,
                    _ => 0,
                })
                .collect();
            // where the chain breaks: before frame i when it or the frame before it is selected
            let breaks: Vec<usize> = (1..frames.len()).filter(|i| sel.contains(&frames[*i]) || sel.contains(&frames[i - 1])).collect();
            let n = src.char_count();
            let mut seg_starts = vec![0usize];
            seg_starts.extend(breaks.iter().copied());
            for (k, &first) in seg_starts.iter().enumerate() {
                let next = seg_starts.get(k + 1).copied();
                let end = next.map(|i| starts[i]).unwrap_or(n);
                let (content, spans) = slice_text(&src, starts[first], end);
                let last = next.map(|i| i - 1).unwrap_or(frames.len() - 1);
                if let Some(ObjKind::Text(t)) = doc.find_mut(frames[first]).map(|o| &mut o.kind) {
                    t.content = content;
                    t.spans = spans;
                    t.format = src.format.clone();
                    t.link_prev = None;
                    t.flow = vt::Flow::default();
                }
                if let Some(ObjKind::Text(t)) = doc.find_mut(frames[last]).map(|o| &mut o.kind) {
                    t.link_next = None;
                }
            }
        }
        doc.update_text_flow();
    });
}

/// Text ▸ Align to Baseline: characters moved up or down go back to the baseline.
pub fn align_to_baseline(app: &mut AnyVectorApp) {
    modify_texts(app, "Align to Baseline", false, |t| {
        t.format.baseline_shift = 0.0;
        for s in t.spans.iter_mut() {
            s.format.baseline_shift = 0.0;
        }
    });
}

/// Toggle a paragraph setting of the selected texts (or of the text you create next).
pub fn toggle_para(app: &mut AnyVectorApp, label: &str, get: fn(&TextObj) -> bool, set: fn(&mut TextObj, bool)) {
    let v = !get(&first_text(app));
    modify_texts(app, label, true, move |t| set(t, v));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyphen_defs_and_content() {
        let defs = vec![("Vector".to_string(), "Vec-tor".to_string())];
        let s = apply_hyphen_defs("The vector app", &defs).unwrap();
        assert_eq!(s, "The vec\u{ad}tor app");
        let mut t = TextObj::artistic("Hello world", Default::default());
        t.apply_format(6, 11, |f| f.bold = true);
        set_content(&mut t, "Hello big world");
        assert_eq!(t.content, "Hello big world");
        assert!(t.format_at(12).bold && !t.format_at(2).bold);
        assert_eq!(change_case_text("hello there. new one", 3), "Hello there. New one");
        assert_eq!(change_case_text("Hello", 4), "hELLO");
    }

    #[test]
    fn quick_correct_as_you_type() {
        let mut s = crate::settings::Settings::default();
        s.replacements = vec![("teh".into(), "the".into())];
        let w = |x: &str| x.chars().collect::<Vec<char>>();
        assert_eq!(quick_correct(&s, &w("so teh"), " "), (3, "the ".into()));
        assert_eq!(quick_correct(&s, &w("see you monday"), "."), (6, "Monday.".into()));
        assert_eq!(quick_correct(&s, &w("THis"), " "), (4, "This ".into()));
        assert_eq!(quick_correct(&s, &w("say "), "\""), (0, "“".into()));
        assert_eq!(quick_correct(&s, &w("say “hi"), "\""), (0, "”".into()));
        assert_eq!(quick_correct(&s, &w("6"), "'"), (0, "'".into()));
        assert_eq!(quick_correct(&s, &w("plain"), " "), (0, " ".into()));
        s.hyphen_defs = vec![("Vector".into(), "Vec-tor".into())];
        assert_eq!(quick_correct(&s, &w("a vector"), " "), (6, "vec\u{ad}tor ".into()));
        assert_eq!(format_code(9), vector_core::text::SOFT_HYPHEN);
    }
}
