//! File ▸ Print Merge ▸ Create/load Print Merge and Edit Print Merge: the data source (columns
//! and records, imported from CSV/TXT or typed in) and Insert Field, which puts a «field» on the
//! page. Print Merged Document and Create Merged Document fill the fields record by record.

use super::{combo, group, window};
use crate::app::AnyVectorApp;
use crate::theme;
use egui::{vec2, Ui};
use vector_core::document::{MergeField, MergeKind, MergeRecord, PrintMerge};

pub struct MergeState {
    pub data: PrintMerge,
    pub field: usize,
    pub new_name: String,
    pub new_kind: usize,
    /// Single-record view: the record shown; None = all records.
    pub single: Option<usize>,
    pub selected_only: bool,
    /// The data source looked up by name when its stored path is gone: (source, drawing folder,
    /// file found).
    moved_source: Option<(String, Option<std::path::PathBuf>, Option<std::path::PathBuf>)>,
}

impl MergeState {
    pub fn new(data: Option<PrintMerge>) -> MergeState {
        MergeState { data: data.unwrap_or_default(), field: 0, new_name: String::new(), new_kind: 0, single: None, selected_only: false, moved_source: None }
    }

    /// The data source file to sync with: the stored path when it is there, else a file of that
    /// name (any case) next to the drawing, as when the drawing and its data were copied from
    /// another computer (`C:\Users\…\Merge data.csv` read on Linux). The search is made once per
    /// source and drawing folder, not at every frame.
    fn source_file(&mut self, drawing_dir: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
        let src = self.data.source.clone()?;
        let stored = std::path::Path::new(&src);
        if stored.exists() {
            return Some(stored.to_path_buf());
        }
        let dir = drawing_dir.map(std::path::Path::to_path_buf);
        if self.moved_source.as_ref().is_none_or(|(s, d, _)| *s != src || *d != dir) {
            let dirs: Vec<&std::path::Path> = drawing_dir.into_iter().collect();
            let found = vector_io::paths::resolve_stored_path(&src, &dirs);
            self.moved_source = Some((src, dir, found));
        }
        self.moved_source.as_ref().and_then(|(_, _, p)| p.clone()).filter(|p| p.exists())
    }
}

/// The text of a CSV/TXT data source: UTF-8, else one byte per character (files saved in the
/// Windows code page, as spreadsheets often save them).
fn read_data_source(p: &std::path::Path) -> std::io::Result<String> {
    let bytes = std::fs::read(p)?;
    Ok(match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => e.into_bytes().iter().map(|b| *b as char).collect(),
    })
}

const KINDS: [&str; 3] = ["Text", "Numeric", "Path"];

fn kind_index(k: MergeKind) -> usize {
    match k {
        MergeKind::Text => 0,
        MergeKind::Numeric => 1,
        MergeKind::Path => 2,
    }
}

pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.merge.take() else { return };
    if app.doc().is_none() {
        return;
    }
    let mut ok = false;
    let mut cancel = false;
    let mut insert: Option<String> = None;
    let drawing_dir = app.doc().and_then(|d| d.path.as_deref()).and_then(|p| p.parent()).map(|p| p.to_path_buf());
    let ((), dismissed) = window(ctx, "print_merge", "Print Merge", vec2(760.0, 540.0), |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Import Data Source...").clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("Data source (*.csv;*.txt)", &vector_io::paths::dialog_extensions(&["csv", "txt"])).pick_file() {
                    match read_data_source(&p) {
                        Ok(text) => {
                            st.data = PrintMerge::parse(&text);
                            st.data.source = Some(p.display().to_string());
                            st.field = 0;
                            st.single = None;
                        }
                        Err(e) => app.dialogs.message_box = Some(("Print Merge".into(), e.to_string())),
                    }
                }
            }
            let sync = st.source_file(drawing_dir.as_deref());
            if ui.add_enabled(sync.is_some(), egui::Button::new("Sync with data source")).clicked() {
                if let Some(text) = sync.and_then(|p| read_data_source(&p).ok()) {
                    let src = st.data.source.clone();
                    st.data = PrintMerge::parse(&text);
                    st.data.source = src;
                }
            }
            if ui.add_enabled(!st.data.fields.is_empty(), egui::Button::new("Save Data Source...")).clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("CSV (*.csv)", &["csv"]).set_file_name("Merge data.csv").save_file() {
                    if let Err(e) = std::fs::write(&p, st.data.to_csv(st.selected_only)) {
                        app.dialogs.message_box = Some(("Print Merge".into(), e.to_string()));
                    }
                }
            }
            ui.checkbox(&mut st.selected_only, "Selected records only");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Clear print merge data").clicked() {
                    st.data = PrintMerge::default();
                    st.single = None;
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(220.0);
                columns(ui, &mut st);
            });
            ui.vertical(|ui| {
                records(ui, &mut st);
            });
        });
        let bottom = egui::Rect::from_min_max(egui::pos2(ui.max_rect().left(), ui.max_rect().bottom() - 60.0), ui.max_rect().right_bottom());
        ui.scope_builder(egui::UiBuilder::new().max_rect(bottom), |ui| {
            ui.horizontal(|ui| {
                ui.label("Field:");
                let names: Vec<&str> = st.data.fields.iter().map(|f| f.name.as_str()).collect();
                ui.add_enabled_ui(!names.is_empty(), |ui| {
                    combo(ui, "merge_field", &mut st.field, &names, 200.0);
                    if ui.button("Insert Field").clicked() {
                        insert = names.get(st.field).map(|s| s.to_string());
                    }
                });
                super::note(ui, "Fields show in text as «Name» and are filled in when merging.");
            });
            ui.add_space(6.0);
            let (o, c) = super::ok_cancel_row(ui, "OK", true);
            ok = o;
            cancel = c;
        });
    });
    if let Some(name) = insert {
        insert_field(app, &name);
        // keep the dialog's data in the document so fields and records stay together
        if let Some(d) = app.doc_mut() {
            let data = st.data.clone();
            d.modify("Print Merge", |doc, _| doc.print_merge = Some(data));
        }
    }
    if ok {
        if let Some(d) = app.doc_mut() {
            let data = (!st.data.fields.is_empty()).then(|| st.data.clone());
            if d.doc.print_merge != data {
                d.modify("Print Merge", |doc, _| doc.print_merge = data);
            }
        }
        return;
    }
    if cancel || dismissed {
        return;
    }
    app.dialogs.merge = Some(st);
}

fn columns(ui: &mut Ui, st: &mut MergeState) {
    group(ui, "Columns", |ui| {
        egui::ScrollArea::vertical().id_salt("merge_cols").max_height(220.0).show(ui, |ui| {
            for (i, f) in st.data.fields.iter().enumerate() {
                if ui.selectable_label(st.field == i, format!("{}  ({})", f.name, KINDS[kind_index(f.kind)])).clicked() {
                    st.field = i;
                }
            }
            if st.data.fields.is_empty() {
                ui.label(egui::RichText::new("No columns yet.").color(theme::TEXT_DIM()));
            }
        });
        ui.add_space(6.0);
        ui.add(egui::TextEdit::singleline(&mut st.new_name).hint_text("Column name").desired_width(190.0));
        ui.horizontal(|ui| {
            combo(ui, "merge_kind", &mut st.new_kind, &KINDS, 90.0);
            let name = st.new_name.trim().to_string();
            let free = !name.is_empty() && !st.data.fields.iter().any(|f| f.name == name);
            if ui.add_enabled(free, egui::Button::new("Add Column")).clicked() {
                let kind = [MergeKind::Text, MergeKind::Numeric, MergeKind::Path][st.new_kind];
                st.data.fields.push(MergeField { name, kind });
                for r in st.data.records.iter_mut() {
                    r.values.push(String::new());
                }
                st.new_name.clear();
                st.field = st.data.fields.len() - 1;
            }
        });
        if ui.add_enabled(st.field < st.data.fields.len(), egui::Button::new("Delete Column")).clicked() {
            let i = st.field;
            st.data.fields.remove(i);
            for r in st.data.records.iter_mut() {
                if i < r.values.len() {
                    r.values.remove(i);
                }
            }
            st.field = st.field.saturating_sub(1);
        }
    });
}

fn records(ui: &mut Ui, st: &mut MergeState) {
    let n = st.data.records.len();
    ui.horizontal(|ui| {
        let all = st.single.is_none();
        if ui.selectable_label(all, "View All Records").clicked() {
            st.single = None;
        }
        if ui.selectable_label(!all, "View Single Record").clicked() && n > 0 {
            st.single = Some(0);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Deselect all").clicked() {
                st.data.records.iter_mut().for_each(|r| r.selected = false);
            }
            if ui.button("Select all").clicked() {
                st.data.records.iter_mut().for_each(|r| r.selected = true);
            }
        });
    });
    let cols = st.data.fields.len();
    egui::Frame::new().fill(theme::FIELD()).stroke(egui::Stroke::new(1.0, theme::SEP())).inner_margin(6.0).show(ui, |ui| {
        ui.set_min_size(vec2(ui.available_width(), 300.0));
        if cols == 0 {
            ui.label(egui::RichText::new("Add columns, or import a data source (CSV or text file).").color(theme::TEXT_DIM()));
            return;
        }
        match st.single {
            None => {
                egui::ScrollArea::both().id_salt("merge_recs").max_height(300.0).show(ui, |ui| {
                    egui::Grid::new("merge_grid").striped(true).spacing(vec2(6.0, 2.0)).show(ui, |ui| {
                        ui.label("");
                        ui.label("#");
                        for f in &st.data.fields {
                            ui.label(egui::RichText::new(&f.name).strong());
                        }
                        ui.end_row();
                        for (i, r) in st.data.records.iter_mut().enumerate() {
                            ui.checkbox(&mut r.selected, "");
                            ui.label((i + 1).to_string());
                            for v in r.values.iter_mut() {
                                ui.add(egui::TextEdit::singleline(v).desired_width(110.0));
                            }
                            ui.end_row();
                        }
                    });
                });
            }
            Some(i) => {
                let i = i.min(n.saturating_sub(1));
                if let Some(r) = st.data.records.get_mut(i) {
                    ui.checkbox(&mut r.selected, format!("Record {} of {n}", i + 1));
                    egui::Grid::new("merge_one").spacing(vec2(10.0, 4.0)).show(ui, |ui| {
                        for (f, v) in st.data.fields.iter().zip(r.values.iter_mut()) {
                            ui.label(&f.name);
                            ui.add(egui::TextEdit::singleline(v).desired_width(300.0));
                            ui.end_row();
                        }
                    });
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(i > 0, egui::Button::new("|◀")).on_hover_text("First").clicked() {
                        st.single = Some(0);
                    }
                    if ui.add_enabled(i > 0, egui::Button::new("◀")).on_hover_text("Previous").clicked() {
                        st.single = Some(i - 1);
                    }
                    if ui.add_enabled(i + 1 < n, egui::Button::new("▶")).on_hover_text("Next").clicked() {
                        st.single = Some(i + 1);
                    }
                    if ui.add_enabled(i + 1 < n, egui::Button::new("▶|")).on_hover_text("Last").clicked() {
                        st.single = Some(n - 1);
                    }
                });
            }
        }
    });
    ui.horizontal(|ui| {
        if ui.add_enabled(cols > 0, egui::Button::new("Add a record")).clicked() {
            st.data.records.push(MergeRecord { selected: true, values: vec![String::new(); cols] });
            if st.single.is_some() {
                st.single = Some(st.data.records.len() - 1);
            }
        }
        let target = st.single.unwrap_or(n.saturating_sub(1));
        if ui.add_enabled(n > 0, egui::Button::new("Delete a record")).clicked() && target < n {
            st.data.records.remove(target);
            if let Some(s) = st.single.as_mut() {
                *s = (*s).min(st.data.records.len().saturating_sub(1));
            }
        }
    });
}

/// Put a «field» text object at the middle of the current page.
fn insert_field(app: &mut AnyVectorApp, name: &str) {
    let Some(d) = app.doc_mut() else { return };
    let text = PrintMerge::marker(name);
    let center = d.page_ref().rect().center();
    let fmt = vector_core::text::CharFormat { size: 24.0, ..Default::default() };
    let id = d.doc.new_id();
    let o = vector_core::Object::new(id, vector_core::ObjKind::Text(vector_core::text::TextObj::artistic(&text, fmt)), kurbo::Affine::translate((center.x - 40.0, center.y)), vector_core::Style::text_default());
    d.add_objects("Insert Field", vec![o]);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drawing with print merge data, copied with its CSV from a Windows computer: Sync finds
    /// the CSV next to the drawing, whatever the case of its name, and reads it like Import does
    /// (UTF-8, else the Windows code page byte by byte).
    #[test]
    fn data_source_found_next_to_a_moved_drawing() {
        let dir = std::env::temp_dir().join(format!("anyvector-merge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Merge Data.CSV"), b"Name,City\nJos\xe9,S\xe3o Paulo\n").unwrap();
        let mut st = MergeState::new(Some(PrintMerge { source: Some(r"C:\Users\ana\Documents\merge data.csv".into()), ..Default::default() }));
        let found = st.source_file(Some(&dir)).expect("the CSV next to the drawing");
        assert!(found.starts_with(&dir));
        let text = read_data_source(&found).unwrap();
        assert!(text.contains("Jos\u{e9}") && text.contains("S\u{e3}o Paulo"), "{text}");
        let data = PrintMerge::parse(&text);
        assert_eq!(data.fields.len(), 2);
        assert_eq!(data.records.len(), 1);
        // the search is kept; a source that is nowhere gives nothing
        assert_eq!(st.source_file(Some(&dir)), Some(found.clone()));
        st.data.source = Some(r"C:\Elsewhere\other.csv".into());
        assert_eq!(st.source_file(Some(&dir)), None);
        // an unsaved drawing has no folder to look in
        st.data.source = Some(r"C:\Users\ana\Documents\merge data.csv".into());
        assert_eq!(st.source_file(None), None);
        // the stored path itself, when it is there
        st.data.source = Some(found.to_string_lossy().to_string());
        assert_eq!(st.source_file(None), Some(found));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
