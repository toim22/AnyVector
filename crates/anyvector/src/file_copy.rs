//! Files for other programs (#132). AnyVector's `.cdr` is its own format, which CorelDRAW does
//! not open, so Save As also writes CMX (CorelDRAW-compatible), PDF, SVG and EMF. As with
//! CorelDRAW's Export, those are copies: the drawing keeps its own file, its modified state and
//! its place in the recent files. Also here: the one-time note after saving a `.cdr`, and what
//! Open says about a `.cdr` that no one recognises.

use crate::app::AnyVectorApp;
use std::path::{Path, PathBuf};
use vector_core::Document;

/// The one-time note after the first `.cdr` save (one line per translation key).
pub const CDR_NOTICE: &[&str] = &[
    "CorelDRAW cannot open this file: AnyVector .cdr files open in AnyVector.",
    "To open the drawing in CorelDRAW, use File ▸ Save As and choose CMX (CorelDRAW-compatible), PDF, SVG or EMF.",
    "This note is shown once. Turn it back on in Tools ▸ Options ▸ AnyVector ▸ Save.",
];

/// Write a copy of `doc` as `path`, by its extension (`cmx`, `pdf`, `svg`, `emf`), with every
/// page: one CMX or PDF file, or an SVG/EMF file per page (numbered when there are several).
/// Returns what the copy could not hold (English lines, keys of the translation files).
pub fn write_copy(app: &AnyVectorApp, doc: &Document, path: &Path) -> vector_io::Result<Vec<String>> {
    let pages: Vec<usize> = (0..doc.pages.len()).collect();
    match vector_io::extension(path).as_str() {
        "cmx" => vector_io::cmx_export::export(doc, path, &vector_io::cmx_export::CmxOptions::default()),
        "pdf" => {
            let mut opts = crate::dialogs::export::pdf_options(app);
            opts.pages = pages;
            vector_io::pdf::export(doc, path, &opts).map(|_| vec![])
        }
        "svg" => {
            let units = ["pt", "in", "mm", "px"];
            let opts = vector_io::svg_export::SvgOptions { unit: units[app.settings.svg_unit.min(3)], text_as_text: app.settings.svg_text_as_text, ..Default::default() };
            for &p in &pages {
                vector_io::svg_export::export(doc, p, &page_file(path, p, pages.len()), &opts)?;
            }
            Ok(vec![])
        }
        "emf" => {
            let opts = vector_io::emf::EmfOptions::default();
            for &p in &pages {
                vector_io::emf::export(doc, p, &page_file(path, p, pages.len()), &opts)?;
            }
            Ok(vec![])
        }
        e => Err(vector_io::IoError::Unsupported(e.to_string())),
    }
}

/// The file of page `page` when a format holds one page per file: `name-2.svg` for the second
/// of several pages, the name itself for a one-page drawing (as Export names them).
pub fn page_file(path: &Path, page: usize, count: usize) -> PathBuf {
    crate::dialogs::export::numbered(path, page, count)
}

/// The message after a copy is written with differences from the drawing (CMX).
pub fn copy_notes_text(path: &Path, notes: &[String]) -> String {
    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut text = crate::i18n::t(&format!("{} was saved with these differences from the drawing:", name));
    for n in notes {
        text.push_str("\n• ");
        text.push_str(&crate::i18n::t(n));
    }
    text
}

/// What Open says when `p` does not open. A `.cdr` that neither AnyVector nor CorelDRAW wrote
/// (or one damaged past recognition) gets an explanation instead of "Invalid file".
pub fn open_error_text(p: &Path, e: &vector_io::IoError) -> String {
    if vector_io::extension(p) == "cdr" && vector_io::cdr::is_unrecognised(e) {
        let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        return format!(
            "{}\n{}",
            crate::i18n::t(&format!("{} was not written by AnyVector or CorelDRAW, or it is damaged.", name)),
            crate::i18n::t("If it came from another program, open a PDF, SVG or EPS copy of it instead.")
        );
    }
    format!("Could not open {}:\n{e}", p.display())
}

/// File ▸ Export to CMX: the pages (or the selection) as one CorelDRAW-compatible file.
pub fn export_cmx(app: &mut AnyVectorApp, path: &Path, pages: &[usize], selection: bool) {
    let Some(d) = app.doc() else { return };
    let (doc, pages) = if selection {
        match crate::exec::selection_document(&d.doc, &d.selection) {
            Some(sd) => (sd, vec![0]),
            None => return,
        }
    } else {
        (d.doc.clone(), pages.to_vec())
    };
    match vector_io::cmx_export::export(&doc, path, &vector_io::cmx_export::CmxOptions { pages }) {
        Ok(notes) => {
            app.flash(format!("Exported {}", path.display()));
            if !notes.is_empty() {
                app.dialogs.message_box = Some(("Export".into(), copy_notes_text(path, &notes)));
            }
        }
        Err(e) => app.dialogs.message_box = Some(("Export".into(), format!("Export failed:\n{e}"))),
    }
}

impl AnyVectorApp {
    /// After an interactive `.cdr` save: the note that CorelDRAW does not open the file, the
    /// first time only (the Warnings entry is then off; Options ▸ Save turns it back on).
    pub(crate) fn cdr_saved_notice(&mut self) {
        let Some((_, on)) = self.settings.warnings.iter_mut().find(|(t, _)| t == crate::settings::CDR_NOTICE_WARNING) else { return };
        if !*on {
            return;
        }
        *on = false;
        let text: Vec<String> = CDR_NOTICE.iter().map(|l| crate::i18n::t(l)).collect();
        self.dialogs.message_box = Some(("Save".into(), text.join("\n")));
        self.settings.save();
    }

    /// Save As to CMX, PDF, SVG or EMF: write the copy and say so. The document keeps its
    /// path and modified state; false, so a close or quit that asked to save does not go on.
    pub(crate) fn save_copy(&mut self, i: usize, path: &Path) -> bool {
        let r = write_copy(self, &self.docs[i].doc, path);
        match r {
            Ok(notes) => {
                self.settings.last_dir = path.parent().map(|x| x.to_path_buf());
                self.flash(format!("Saved a copy as {}", path.display()));
                if !notes.is_empty() {
                    self.dialogs.message_box = Some(("Save".into(), copy_notes_text(path, &notes)));
                }
            }
            Err(e) => self.dialogs.message_box = Some(("Save".into(), format!("Could not save {}:\n{e}", path.display()))),
        }
        false
    }
}
