//! The Table menu dialogs: Create New Table, Convert Text to Table, Convert Table to Text,
//! Insert Rows, Insert Columns and Split Cells.

use super::layout::{length, rrow};
use super::{button, ok_button, ok_cancel_row, window};
use crate::app::AnyVectorApp;
use egui::{vec2, Color32, Ui};
use kurbo::Affine;
use vector_core::object::{CellRange, TableObj};
use vector_core::text::{TextKind, TextObj};
use vector_core::{ObjId, ObjKind, Object, Style};

#[derive(Clone, Debug, PartialEq)]
pub enum TableDialog {
    Create { rows: usize, cols: usize, height: f64, width: f64 },
    /// Insert Rows (`cols` false) or Insert Columns; `before` = above / left of the selection.
    Insert { cols: bool, count: usize, before: bool },
    /// Split into Rows (`cols` false) or Split into Columns.
    Split { cols: bool, count: usize },
    /// 0 commas, 1 tabs, 2 paragraphs, 3 user defined.
    TextToTable { sep: u8, user: String },
    TableToText { sep: u8, user: String },
}

fn separator_of(sep: u8, user: &str) -> Option<String> {
    match sep {
        0 => Some(",".into()),
        1 => Some("\t".into()),
        2 => None,
        _ => Some(user.to_string()),
    }
}

/// The cells of a text split by the separator: a row per paragraph and a column at each
/// separator; with Paragraphs, one row with a column per paragraph.
pub fn text_cells(text: &str, sep: u8, user: &str) -> Vec<Vec<String>> {
    let paras: Vec<&str> = text.split('\n').collect();
    match separator_of(sep, user) {
        None => vec![paras.iter().map(|p| p.trim_end_matches('\r').to_string()).collect()],
        Some(s) if s.is_empty() => paras.iter().map(|p| vec![p.trim_end_matches('\r').to_string()]).collect(),
        Some(s) => paras.iter().map(|p| p.trim_end_matches('\r').split(s.as_str()).map(|c| c.to_string()).collect()).collect(),
    }
}

/// The text of a table: cells joined by the separator, a paragraph per row (Paragraphs puts
/// every cell in a paragraph of its own).
pub fn table_text(t: &TableObj, sep: u8, user: &str) -> String {
    let anchors = t.anchors();
    let rows: Vec<Vec<String>> = (0..t.rows).map(|r| anchors.iter().filter(|(ar, _)| *ar == r).map(|(ar, ac)| t.cell(*ar, *ac).text.clone()).collect()).filter(|v: &Vec<String>| !v.is_empty()).collect();
    match separator_of(sep, user) {
        None => rows.into_iter().flatten().collect::<Vec<_>>().join("\n"),
        Some(s) => rows.iter().map(|r| r.join(&s)).collect::<Vec<_>>().join("\n"),
    }
}

fn selected_text(app: &AnyVectorApp) -> Option<(ObjId, TextObj)> {
    let d = app.doc()?;
    let [id] = d.selection.as_slice() else { return None };
    match &d.doc.find(*id)?.kind {
        ObjKind::Text(t) => Some((*id, t.clone())),
        _ => None,
    }
}

fn selected_table(app: &AnyVectorApp) -> Option<(ObjId, TableObj)> {
    let d = app.doc()?;
    let [id] = d.selection.as_slice() else { return None };
    match &d.doc.find(*id)?.kind {
        ObjKind::Table(t) => Some((*id, t.clone())),
        _ => None,
    }
}

pub(crate) fn info_box(ui: &mut Ui, lines: &[String]) {
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), egui::Sense::hover());
        ui.painter().circle_filled(r.center(), 11.0, Color32::from_rgb(0x1e, 0x88, 0xe5));
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "i", egui::FontId::proportional(15.0), Color32::WHITE);
        ui.add_space(6.0);
        ui.vertical(|ui| {
            for l in lines {
                ui.add(egui::Label::new(l.as_str()).wrap());
                ui.add_space(4.0);
            }
        });
    });
}

fn separator_radios(ui: &mut Ui, sep: &mut u8, user: &mut String, user_label: &str) {
    ui.spacing_mut().item_spacing.y = 8.0;
    ui.radio_value(sep, 0, "Commas");
    ui.radio_value(sep, 1, "Tabs");
    ui.radio_value(sep, 2, "Paragraphs");
    ui.horizontal(|ui| {
        ui.radio_value(sep, 3, user_label);
        ui.add_space(8.0);
        ui.add_enabled(*sep == 3, egui::TextEdit::singleline(user).desired_width(64.0).char_limit(1));
    });
}

pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.table.take() else { return };
    let (unit, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((vector_core::Unit::Inches, 300.0));
    let ((ok, cancel), dismissed) = match &mut st {
        TableDialog::Create { rows, cols, height, width } => {
            window(ctx, "table_create", "Create New Table", vec2(240.0, 230.0), |ui| {
                ui.add_space(10.0);
                ui.spacing_mut().item_spacing.y = 10.0;
                rrow(ui, "Number of Rows:", 126.0, |ui| ui.add_sized(vec2(70.0, 22.0), egui::DragValue::new(rows).range(1..=999)));
                rrow(ui, "Number of Columns:", 126.0, |ui| ui.add_sized(vec2(70.0, 22.0), egui::DragValue::new(cols).range(1..=999)));
                rrow(ui, "Height:", 126.0, |ui| length(ui, height, unit, dpi, true));
                rrow(ui, "Width:", 126.0, |ui| length(ui, width, unit, dpi, true));
                ok_cancel_row(ui, "OK", true)
            })
        }
        TableDialog::Insert { cols, count, before } => {
            let (title, what, a, b) = if *cols { ("Insert Columns", "Number of Columns:", "Left of the selection", "Right of the selection") } else { ("Insert Rows", "Number of Rows:", "Above the selection", "Below the selection") };
            window(ctx, "table_insert", title, vec2(if *cols { 310.0 } else { 283.0 }, 170.0), |ui| {
                ui.add_space(8.0);
                egui::Grid::new("ti_grid").num_columns(2).spacing(vec2(16.0, 6.0)).show(ui, |ui| {
                    ui.label(what);
                    ui.add_sized(vec2(128.0, 22.0), egui::DragValue::new(count).range(1..=999));
                    ui.end_row();
                    ui.label("Where:");
                    ui.radio_value(before, true, a);
                    ui.end_row();
                    ui.label("");
                    ui.radio_value(before, false, b);
                    ui.end_row();
                });
                ok_cancel_row(ui, "OK", true)
            })
        }
        TableDialog::Split { cols, count } => {
            let label = if *cols { "Number of columns:" } else { "Number of rows:" };
            window(ctx, "table_split", "Split Cells", vec2(if *cols { 227.0 } else { 200.0 }, 125.0), |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.add_space(8.0);
                    ui.add_sized(vec2(78.0, 22.0), egui::DragValue::new(count).range(2..=99));
                });
                let (mut o, mut c) = (false, false);
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            c = button(ui, "Cancel", true);
                            ui.add_space(4.0);
                            o = ok_button(ui, "OK", true);
                        });
                    });
                });
                (o, c)
            })
        }
        TableDialog::TextToTable { sep, user } => {
            let text = selected_text(app).map(|(_, t)| t.content).unwrap_or_default();
            window(ctx, "text_to_table", "Convert Text to Table", vec2(255.0, 340.0), |ui| {
                ui.add_space(6.0);
                ui.add(egui::Label::new("Create columns based on the following separator:").wrap());
                ui.add_space(4.0);
                separator_radios(ui, sep, user, "User defined:");
                let cells = text_cells(&text, *sep, user);
                let (rows, cols) = (cells.len(), cells.iter().map(|r| r.len()).max().unwrap_or(1));
                ui.add_space(10.0);
                let mut lines = vec![];
                if *sep != 2 {
                    lines.push("A new row will be created for each paragraph.".to_string());
                }
                lines.push(format!("Based on the current separator, the table will have {rows} rows and {cols} columns."));
                info_box(ui, &lines);
                ok_cancel_row(ui, "OK", *sep != 3 || !user.is_empty())
            })
        }
        TableDialog::TableToText { sep, user } => {
            window(ctx, "table_to_text", "Convert Table to Text", vec2(245.0, 255.0), |ui| {
                ui.add_space(6.0);
                ui.label("Separate cell text with:");
                ui.add_space(4.0);
                separator_radios(ui, sep, user, "User Defined:");
                ui.add_space(6.0);
                info_box(ui, &["A new paragraph will be created for each row.".to_string()]);
                ok_cancel_row(ui, "OK", *sep != 3 || !user.is_empty())
            })
        }
    };
    if ok {
        apply(app, &st);
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.table = Some(st);
    }
}

fn apply(app: &mut AnyVectorApp, st: &TableDialog) {
    match st {
        TableDialog::Create { rows, cols, height, width } => {
            let Some(d) = app.doc_mut() else { return };
            let pr = d.page_ref().rect();
            let (w, h) = (*width, *height);
            let id = d.doc.new_id();
            let t = TableObj::new(*rows, *cols, w, h);
            let mut o = Object::new(id, ObjKind::Table(t), Affine::translate((pr.center().x - w / 2.0, pr.center().y - h / 2.0)), Default::default());
            o.style.fill = vector_core::Fill::None;
            d.add_objects("Create Table", vec![o]);
        }
        TableDialog::Insert { cols, count, before } => {
            let (cols, count, before) = (*cols, *count, *before);
            let Some((_, t0)) = selected_table(app) else { return };
            let g = app.ts.table_range(&t0);
            app.table_op(|t, g| {
                for _ in 0..count {
                    match (cols, before, g) {
                        (false, true, Some(g)) => t.insert_row(g[0]),
                        (false, false, Some(g)) => t.insert_row(g[2] + 1),
                        (true, true, Some(g)) => t.insert_col(g[1]),
                        (true, false, Some(g)) => t.insert_col(g[3] + 1),
                        (false, _, None) => t.insert_row(t.rows),
                        (true, _, None) => t.insert_col(t.cols),
                    }
                }
            });
            if let (Some(_), true) = (g, before) {
                app.ts.shift_cells(if cols { (0, count) } else { (count, 0) });
            }
        }
        TableDialog::Split { cols, count } => {
            let (cols, n) = (*cols, *count);
            app.table_op(|t, g| {
                if let Some(g) = g {
                    if cols {
                        t.split_cols(g[0], g[1], n);
                    } else {
                        t.split_rows(g[0], g[1], n);
                    }
                }
            });
            app.ts.table_cell_to = None;
        }
        TableDialog::TextToTable { sep, user } => {
            let Some((id, txt)) = selected_text(app) else { return };
            let (sep, user) = (*sep, user.clone());
            let Some(d) = app.doc_mut() else { return };
            d.modify("Convert Text to Table", |doc, sel| {
                let Some(o) = doc.find(id) else { return };
                let b = doc.bounds_of(o);
                let cells = text_cells(&txt.content, sep, &user);
                let (rows, cols) = (cells.len().max(1), cells.iter().map(|r| r.len()).max().unwrap_or(1).max(1));
                let (w, h) = (b.width().max(cols as f64 * 36.0), b.height().max(rows as f64 * (txt.format.size * 1.6).max(18.0)));
                let mut t = TableObj::new(rows, cols, w, h);
                t.text_format = txt.format.clone();
                for (r, row) in cells.into_iter().enumerate() {
                    for (c, s) in row.into_iter().enumerate() {
                        t.cell_mut(r, c).text = s;
                    }
                }
                let mut no = Object::new(id, ObjKind::Table(t), Affine::translate((b.x0, b.y0)), Style::default());
                no.style.fill = vector_core::Fill::None;
                if let Some(x) = doc.find_mut(id) {
                    *x = no;
                }
                *sel = vec![id];
            });
        }
        TableDialog::TableToText { sep, user } => {
            let Some((id, t)) = selected_table(app) else { return };
            let text = table_text(&t, *sep, user);
            let Some(d) = app.doc_mut() else { return };
            d.modify("Convert Table to Text", |doc, sel| {
                let Some(o) = doc.find(id) else { return };
                let tr = o.transform;
                let mut tx = TextObj::paragraph(&text, t.width(), t.height(), t.text_format.clone());
                tx.kind = TextKind::Paragraph { width: t.width(), height: t.height() };
                let no = Object::new(id, ObjKind::Text(tx), tr, Style::text_default());
                if let Some(x) = doc.find_mut(id) {
                    *x = no;
                }
                *sel = vec![id];
            });
            app.ts.table_cell = None;
            app.ts.table_cell_to = None;
        }
    }
}

#[cfg(test)]
pub fn apply_for_test(app: &mut AnyVectorApp, st: &TableDialog) {
    apply(app, st);
}

/// The selected block of the selected table, for the menu states.
pub fn selected_range(app: &AnyVectorApp) -> Option<(TableObj, CellRange)> {
    let (_, t) = selected_table(app)?;
    let g = app.ts.table_range(&t)?;
    Some((t, g))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_and_table_conversions() {
        let cells = text_cells("a,b,c\nd,e", 0, "");
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0], vec!["a", "b", "c"]);
        assert_eq!(text_cells("a\nb\nc", 2, "")[0].len(), 3);
        assert_eq!(text_cells("a;b", 3, ";")[0], vec!["a", "b"]);
        let mut t = TableObj::new(2, 2, 100.0, 100.0);
        t.cell_mut(0, 0).text = "a".into();
        t.cell_mut(0, 1).text = "b".into();
        t.cell_mut(1, 0).text = "c".into();
        assert_eq!(table_text(&t, 1, ""), "a\tb\nc\t");
        assert_eq!(table_text(&t, 2, ""), "a\nb\nc\n");
        t.merge([0, 0, 0, 1]);
        assert_eq!(table_text(&t, 0, ""), "a b\nc,");
    }
}
