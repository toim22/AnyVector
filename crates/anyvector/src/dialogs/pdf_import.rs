//! File ▸ Open and File ▸ Import of a PDF: the Import PDF dialog (as CorelDRAW's: "Import text
//! as:" Text or Curves, and for an import the pages to bring in), then the open or import.

use super::{group, ok_cancel_plain, window};
use crate::app::AnyVectorApp;
use egui::vec2;
use kurbo::{Affine, Point};
use std::path::{Path, PathBuf};
use vector_io::pdf_import::{self, PdfImportOptions};

pub struct PdfImportState {
    pub path: PathBuf,
    bytes: Vec<u8>,
    /// The size of each page (points).
    sizes: Vec<(f64, f64)>,
    /// File ▸ Open (every page, a new drawing) rather than File ▸ Import.
    open: bool,
    /// Where an import was dropped.
    at: Option<Point>,
    curves: bool,
    all_pages: bool,
    chosen: Vec<bool>,
}

/// Read the PDF's page list and ask how to bring it in. False when it can't be read.
pub fn start(app: &mut AnyVectorApp, p: &Path, open: bool, at: Option<Point>) -> bool {
    let title = if open { "Open" } else { "Import" };
    let read = std::fs::read(p).map_err(vector_io::IoError::from).and_then(|b| pdf_import::page_sizes(&b).map(|s| (b, s)));
    match read {
        Ok((bytes, sizes)) if !sizes.is_empty() => {
            let n = sizes.len();
            let mut chosen = vec![false; n];
            chosen[0] = true;
            app.dialogs.pdf_import = Some(PdfImportState { path: p.to_path_buf(), bytes, sizes, open, at, curves: false, all_pages: open || n == 1, chosen });
            true
        }
        Ok(_) => {
            app.dialogs.message_box = Some((title.into(), format!("Could not open {}:\nthe PDF has no pages", p.display())));
            false
        }
        Err(e) => {
            app.dialogs.message_box = Some((title.into(), format!("Could not open {}:\n{e}", p.display())));
            false
        }
    }
}

fn size_text(w: f64, h: f64) -> String {
    format!("{:.0} × {:.0} mm", w * 25.4 / 72.0, h * 25.4 / 72.0)
}

pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.pdf_import.take() else { return };
    let many = !st.open && st.sizes.len() > 1;
    let height = if many { 380.0 } else { 170.0 };
    let name = st.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ((ok, cancel), dismissed) = window(ctx, "pdf_import", "Import PDF", vec2(340.0, height), |ui| {
        ui.add_space(8.0);
        ui.label(&name);
        group(ui, "Import text as:", |ui| {
            ui.radio_value(&mut st.curves, false, "Text");
            ui.radio_value(&mut st.curves, true, "Curves");
        });
        if many {
            group(ui, "Pages", |ui| {
                ui.radio_value(&mut st.all_pages, true, format!("All pages ({})", st.sizes.len()));
                ui.radio_value(&mut st.all_pages, false, "Selected pages:");
                ui.add_enabled_ui(!st.all_pages, |ui| {
                    egui::ScrollArea::vertical().max_height(130.0).show(ui, |ui| {
                        for (i, (w, h)) in st.sizes.iter().enumerate() {
                            ui.checkbox(&mut st.chosen[i], format!("Page {}   ({})", i + 1, size_text(*w, *h)));
                        }
                    });
                });
            });
        }
        let any = st.all_pages || st.chosen.iter().any(|c| *c);
        ok_cancel_plain(ui, any)
    });
    if cancel || dismissed {
        return;
    }
    if !ok {
        app.dialogs.pdf_import = Some(st);
        return;
    }
    let pages = if st.all_pages { vec![] } else { st.chosen.iter().enumerate().filter(|(_, c)| **c).map(|(i, _)| i).collect() };
    let opts = PdfImportOptions { pages, text_as_curves: st.curves };
    if st.open {
        let r = pdf_import::load_with_notes(&st.bytes, &opts);
        app.finish_open(&st.path, r);
    } else {
        import(app, &st.path, &st.bytes, &opts, st.at);
    }
}

/// Import the chosen pages: the first on the active page, each further one on a new page after
/// it, of the PDF page's size.
fn import(app: &mut AnyVectorApp, p: &Path, bytes: &[u8], opts: &PdfImportOptions, at: Option<Point>) {
    if !app.has_doc() {
        app.new_default_document();
    }
    let Some(d) = app.doc_mut() else { return };
    let (mut pages, notes) = match pdf_import::import_pages(&mut d.doc, bytes, opts) {
        Ok(r) => r,
        Err(e) => {
            app.dialogs.message_box = Some(("Import".into(), format!("Could not import {}:\n{e}", p.display())));
            return;
        }
    };
    if pages.is_empty() {
        return;
    }
    let rest = pages.split_off(1);
    let (first, _) = pages.pop().unwrap();
    app.place_import(p, at, first, vec![]);
    if !rest.is_empty() {
        if let Some(d) = app.doc_mut() {
            let mut page = d.page;
            let mut last_ids = vec![];
            d.modify("Import", |doc, sel| {
                for (mut objs, (w, h)) in rest {
                    page = doc.add_page(page + 1, w, h);
                    let bb = vector_core::object::union_bounds(objs.iter().map(|o| o.bounds()));
                    let a = Affine::translate(Point::new(w / 2.0, h / 2.0) - bb.center());
                    for o in objs.iter_mut() {
                        o.apply_transform(a);
                    }
                    last_ids = objs.iter().map(|o| o.id).collect();
                    let layer = doc.pages[page].layers.iter().position(|l| !l.locked).unwrap_or(0);
                    doc.pages[page].layers[layer].objects.extend(objs);
                }
                *sel = last_ids.clone();
            });
            d.page = page;
            d.needs_fit = true;
        }
    }
    app.show_import_notes("Import", p, &notes);
}
