//! Command execution: every menu item, toolbar button, shortcut and script command ends here.

use crate::app::{Toolbar, VectorApp};
use crate::commands::{Cmd, OptionsPage};
use crate::dialogs::{ExportState, NewDocState, PrintState, ScriptEditorState, TraceState};
use crate::dockers::DockerId;
use crate::docview::DocView;
use crate::tools::{PickTarget, ToolId};
use kurbo::{Affine, Point, Rect, Vec2};
use std::path::{Path, PathBuf};
use vector_core::object::{BitmapEffect, ImageAdjust, ObjKind};
use vector_core::ops::{self, AlignTo};
use vector_core::{Color, Document, Fill, ObjId, Object};

pub const BITMAP_EFFECTS: [&str; 8] = ["Gaussian Blur", "Glow", "Emboss", "Add Noise", "Pixelate", "Grayscale", "Sepia", "Invert"];

pub fn bitmap_effect_has_amount(i: usize) -> bool {
    i < 5
}

pub fn bitmap_effect(i: usize, amount: f64) -> BitmapEffect {
    match i {
        0 => BitmapEffect::GaussianBlur { radius: amount },
        1 => BitmapEffect::Glow { radius: amount, color: Color::rgb(255, 230, 120) },
        2 => BitmapEffect::Emboss { depth: amount.max(0.5) },
        3 => BitmapEffect::Noise { amount: (amount * 2.0).min(100.0) },
        4 => BitmapEffect::Pixelate { size: amount.max(1.0) },
        5 => BitmapEffect::Grayscale,
        6 => BitmapEffect::Sepia,
        _ => BitmapEffect::Invert,
    }
}

/// Whether the system clipboard holds something pasteable (cached for a second).
pub fn system_clipboard_has_content() -> bool {
    use std::sync::Mutex;
    static CACHE: Mutex<Option<(std::time::Instant, bool)>> = Mutex::new(None);
    let mut c = CACHE.lock().unwrap();
    if let Some((t, v)) = *c {
        if t.elapsed().as_secs_f64() < 1.0 {
            return v;
        }
    }
    let v = arboard::Clipboard::new().map(|mut cb| cb.get_text().map(|t| !t.is_empty()).unwrap_or(false) || cb.get_image().is_ok()).unwrap_or(false);
    *c = Some((std::time::Instant::now(), v));
    v
}

/// Apply a color from a palette/docker: fill (or outline) of the selection, or the default when
/// nothing is selected.
pub fn apply_color(app: &mut VectorApp, color: Option<Color>, outline: bool) {
    if !app.has_selection() {
        if !outline {
            if let Some(c) = color {
                app.default_fill = c;
            }
        }
        app.flash(if outline { "Select objects to change their outline color." } else { "Default fill set for new objects." });
        return;
    }
    let width = app.ts.default_outline_width;
    let Some(d) = app.doc_mut() else { return };
    let ids = d.selection.clone();
    d.modify(if outline { "Outline Color" } else { "Fill Color" }, |doc, _| {
        for id in &ids {
            if let Some(o) = doc.find_mut(*id) {
                o.walk_mut(&mut |x| {
                    if x.is_container() && !matches!(x.kind, ObjKind::PowerClip { .. }) {
                        return;
                    }
                    if outline {
                        x.style.outline.color = color;
                        if color.is_some() && x.style.outline.width <= 0.0 {
                            x.style.outline.width = width.max(0.1);
                        }
                    } else {
                        x.style.fill = match color {
                            Some(c) => Fill::Solid(c),
                            None => Fill::None,
                        };
                    }
                });
            }
        }
    });
    if let Some(c) = color {
        if app.settings.auto_update_doc_palette {
            if let Some(d) = app.doc_mut() {
                if !d.doc.palette.iter().any(|n| n.color == c) {
                    d.doc.palette.push(vector_core::NamedColor::new(c.to_hex(), c));
                }
            }
        }
    }
}

/// Turn the selection into a new symbol definition and replace it with an instance.
pub fn new_symbol(app: &mut VectorApp, name: &str) {
    let Some(d) = app.doc_mut() else { return };
    let ids = d.selection.clone();
    if ids.is_empty() {
        return;
    }
    d.modify("New Symbol", |doc, sel| {
        let bb = doc.selection_bounds(&ids).unwrap_or(Rect::ZERO);
        let first = doc.locate(ids[0]);
        let mut objs = vec![];
        for id in &ids {
            if let Some(mut o) = doc.remove(*id) {
                o.apply_transform(Affine::translate(-bb.origin().to_vec2()));
                objs.push(o);
            }
        }
        let sid = doc.new_id();
        let n = if name.trim().is_empty() { format!("Symbol {}", doc.symbols.len() + 1) } else { name.trim().to_string() };
        doc.symbols.push(vector_core::document::SymbolDef { id: sid, name: n.clone(), objects: objs });
        let iid = doc.new_id();
        let mut inst = Object::new(iid, ObjKind::Symbol(sid), Affine::translate(bb.origin().to_vec2()), Default::default());
        inst.name = n;
        let (p, l) = first.map(|(p, l, _)| (p, l)).unwrap_or((0, 0));
        doc.pages[p].layers[l].objects.push(inst);
        *sel = vec![iid];
    });
}

/// Export the current page (or selection) with the Export docker's quick settings.
pub fn quick_export(app: &mut VectorApp, format: &str, dpi: f64, selection_only: bool) {
    let idx = vector_io::EXPORT_TYPES.iter().position(|t| t.extensions.contains(&format.to_lowercase().as_str())).unwrap_or(2);
    let st = ExportState { format: idx, range: if selection_only { 2 } else { 0 }, dpi, ..Default::default() };
    app.export_with(&st);
}

fn rotate_about(c: Point, deg: f64) -> Affine {
    Affine::translate(c.to_vec2()) * Affine::rotate(-deg.to_radians()) * Affine::translate(-c.to_vec2())
}

fn scale_about(c: Point, sx: f64, sy: f64) -> Affine {
    Affine::translate(c.to_vec2()) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-c.to_vec2())
}

/// A document containing only the given objects on one page sized to their bounds.
pub fn selection_document(doc: &Document, ids: &[ObjId]) -> Option<Document> {
    let bb = doc.selection_bounds(ids)?;
    let mut out = doc.clone();
    let objs: Vec<Object> = ids.iter().filter_map(|id| doc.find(*id).cloned()).collect();
    let mut page = out.pages[0].clone();
    page.width = bb.width().max(1.0);
    page.height = bb.height().max(1.0);
    page.background = None;
    page.guides.clear();
    page.layers.truncate(1);
    page.layers[0].objects = objs;
    page.layers[0].visible = true;
    page.layers[0].printable = true;
    for o in page.layers[0].objects.iter_mut() {
        o.apply_transform(Affine::translate(-bb.origin().to_vec2()));
    }
    out.pages = vec![page];
    Some(out)
}

fn open_in_shell(path: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
}

fn reveal_in_explorer(path: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    }
}

impl VectorApp {
    fn sel(&self) -> Vec<ObjId> {
        self.doc().map(|d| d.selection.clone()).unwrap_or_default()
    }

    fn unit_default(&self) -> vector_core::Unit {
        self.settings.default_unit
    }

    // -------------------------------------------------------------------------------- documents

    fn push_doc(&mut self, dv: DocView) {
        self.finish_tool();
        self.docs.push(dv);
        self.active = Some(self.docs.len() - 1);
    }

    pub fn new_default_document(&mut self) {
        self.untitled_counter += 1;
        let st = NewDocState::new(self.untitled_counter, self.unit_default());
        self.create_document(&st);
    }

    pub fn create_document(&mut self, st: &NewDocState) {
        let mut doc = Document::new(st.width, st.height);
        doc.settings.unit = st.unit;
        doc.settings.dpi = st.dpi;
        doc.info.author = self.settings.user_name.clone();
        for k in 1..st.pages.max(1) {
            doc.add_page(k, st.width, st.height);
        }
        let mut dv = DocView::new(doc, format!("{}.vct", st.name.trim_end_matches(".vct")), self.settings.undo_levels);
        dv.history.mark_saved();
        self.push_doc(dv);
    }

    fn new_document_cmd(&mut self) {
        if self.settings.show_new_doc_dialog {
            self.untitled_counter += 1;
            let mut st = NewDocState::new(self.untitled_counter, self.unit_default());
            if let Some(d) = self.doc() {
                st.width = d.page_ref().width;
                st.height = d.page_ref().height;
                st.unit = d.doc.settings.unit;
            }
            self.dialogs.new_doc = Some(st);
        } else {
            self.new_default_document();
        }
    }

    fn open_filters(d: rfd::FileDialog, import: bool) -> rfd::FileDialog {
        let types = if import { vector_io::IMPORT_TYPES } else { vector_io::OPEN_TYPES };
        let mut all: Vec<&str> = types.iter().flat_map(|t| t.extensions.iter().copied()).collect();
        if !import {
            for t in vector_io::IMPORT_TYPES.iter().filter(|t| !t.vector) {
                all.extend(t.extensions.iter().copied());
            }
        }
        let mut d = d.add_filter("All File Formats", &all);
        for t in types {
            d = d.add_filter(t.name, t.extensions);
        }
        d
    }

    fn open_dialog(&mut self, dir: Option<PathBuf>) {
        let mut fd = Self::open_filters(rfd::FileDialog::new().set_title("Open Drawing"), false);
        if let Some(d) = dir.or_else(|| self.settings.last_dir.clone()) {
            fd = fd.set_directory(d);
        }
        if let Some(files) = fd.pick_files() {
            for f in files {
                self.open_path(&f);
            }
        }
    }

    /// Open a file in a new tab (or activate it if it is already open).
    pub fn open_path(&mut self, p: &Path) -> bool {
        if let Some(i) = self.docs.iter().position(|d| d.path.as_deref() == Some(p)) {
            self.active = Some(i);
            return true;
        }
        match vector_io::open(p) {
            Ok(mut doc) => {
                doc.ensure_next_id();
                let title = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let mut dv = DocView::new(doc, title, self.settings.undo_levels);
                if vector_io::extension(p) == "vct" {
                    dv.path = Some(p.to_path_buf());
                } else {
                    // imported formats open as untitled drawings
                    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    dv.title = format!("{stem}.vct");
                }
                dv.history.mark_saved();
                self.settings.add_recent(p);
                self.settings.last_dir = p.parent().map(|x| x.to_path_buf());
                self.push_doc(dv);
                true
            }
            Err(e) => {
                self.dialogs.message_box = Some(("Open".into(), format!("Could not open {}:\n{e}", p.display())));
                false
            }
        }
    }

    fn import_dialog(&mut self) {
        let mut fd = Self::open_filters(rfd::FileDialog::new().set_title("Import"), true);
        if let Some(d) = self.settings.last_dir.clone() {
            fd = fd.set_directory(d);
        }
        if let Some(files) = fd.pick_files() {
            for f in files {
                self.import_path(&f, None);
            }
        }
    }

    /// Import a file into the active document, centered on the page (or at a point).
    pub fn import_path(&mut self, p: &Path, at: Option<Point>) {
        if !self.has_doc() {
            self.new_default_document();
        }
        let warn_large = self.settings.warnings.iter().any(|(t, on)| *on && t.contains("large bitmap"));
        let Some(d) = self.doc_mut() else { return };
        match vector_io::import(&mut d.doc, p) {
            Ok(mut objs) => {
                if objs.is_empty() {
                    return;
                }
                let bb = vector_core::object::union_bounds(objs.iter().map(|o| o.bounds()));
                let target = at.unwrap_or_else(|| d.page_ref().rect().center());
                let mut a = Affine::translate(target - bb.center());
                // fit very large imports to the page
                let pr = d.page_ref().rect();
                if at.is_none() && (bb.width() > pr.width() * 1.5 || bb.height() > pr.height() * 1.5) {
                    let s = (pr.width() / bb.width()).min(pr.height() / bb.height()) * 0.9;
                    a = Affine::translate(target.to_vec2()) * Affine::scale(s) * Affine::translate(-bb.center().to_vec2());
                }
                for o in objs.iter_mut() {
                    o.apply_transform(a);
                }
                d.add_objects("Import", objs);
                let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                self.settings.last_dir = p.parent().map(|x| x.to_path_buf());
                let large = self.doc().and_then(|d| d.doc.images.values().map(|i| i.width as u64 * i.height as u64).max()).unwrap_or(0) > 40_000_000;
                if large && warn_large {
                    self.flash(format!("Imported {name} (large bitmap: editing may be slower)"));
                } else {
                    self.flash(format!("Imported {name}"));
                }
            }
            Err(e) => self.dialogs.message_box = Some(("Import".into(), format!("Could not import {}:\n{e}", p.display()))),
        }
    }

    /// Save document `i`. Returns false if cancelled or failed.
    pub fn save_doc(&mut self, i: usize, save_as: bool) -> bool {
        if i >= self.docs.len() {
            return false;
        }
        let path = match (&self.docs[i].path, save_as) {
            (Some(p), false) => p.clone(),
            _ => {
                let name = self.docs[i].display_name().trim_end_matches('*').to_string();
                let mut fd = rfd::FileDialog::new().set_title("Save Drawing").add_filter("Vector Drawing (*.vct)", &["vct"]).set_file_name(if name.ends_with(".vct") { name } else { format!("{name}.vct") });
                if let Some(d) = self.docs[i].path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf()).or_else(|| self.settings.last_dir.clone()) {
                    fd = fd.set_directory(d);
                }
                match fd.save_file() {
                    Some(p) => {
                        if vector_io::extension(&p) != "vct" {
                            p.with_extension("vct")
                        } else {
                            p
                        }
                    }
                    None => return false,
                }
            }
        };
        if self.settings.backup_on_save && path.exists() {
            let dir = self.settings.backup_dir.clone().unwrap_or_else(|| path.parent().map(|p| p.to_path_buf()).unwrap_or_default());
            let name = format!("Backup_of_{}", path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::copy(&path, dir.join(name));
        }
        let d = &mut self.docs[i];
        match vector_io::native::save(&d.doc, &path) {
            Ok(()) => {
                d.path = Some(path.clone());
                d.history.mark_saved();
                self.settings.add_recent(&path);
                self.settings.last_dir = path.parent().map(|x| x.to_path_buf());
                self.flash(format!("Saved {}", path.display()));
                true
            }
            Err(e) => {
                self.dialogs.message_box = Some(("Save".into(), format!("Could not save {}:\n{e}", path.display())));
                false
            }
        }
    }

    /// Close a tab, asking to save changes first.
    pub fn request_close_doc(&mut self, i: usize) {
        if i >= self.docs.len() {
            return;
        }
        if self.docs[i].history.is_modified() {
            self.active = Some(i);
            self.dialogs.confirm_close = Some(i);
        } else {
            self.close_doc_now(i);
        }
    }

    pub fn close_doc_now(&mut self, i: usize) {
        if i >= self.docs.len() {
            return;
        }
        if self.active == Some(i) {
            self.finish_tool();
        }
        self.docs.remove(i);
        for q in self.dialogs.close_queue.iter_mut() {
            if *q > i {
                *q -= 1;
            }
        }
        self.active = match self.active {
            _ if self.docs.is_empty() => None,
            Some(a) if a > i => Some(a - 1),
            Some(a) if a == i => Some(i.min(self.docs.len() - 1)),
            other => other,
        };
    }

    /// Continue closing the documents queued by Close All.
    pub fn continue_close_queue(&mut self) {
        while let Some(i) = self.dialogs.close_queue.pop() {
            if i < self.docs.len() {
                if self.docs[i].history.is_modified() {
                    self.active = Some(i);
                    self.dialogs.confirm_close = Some(i);
                    return;
                }
                self.close_doc_now(i);
            }
        }
    }

    fn close_all(&mut self) {
        self.dialogs.close_queue = (0..self.docs.len()).collect();
        self.continue_close_queue();
    }

    // -------------------------------------------------------------------------------- export / print

    pub fn export_with(&mut self, st: &ExportState) {
        let Some(d) = self.doc() else { return };
        let t = &vector_io::EXPORT_TYPES[st.format.min(vector_io::EXPORT_TYPES.len() - 1)];
        let ext = t.extensions[0];
        let stem = d.display_name().trim_end_matches('*').trim_end_matches(".vct").to_string();
        let path = match &st.path {
            Some(p) => p.clone(),
            None => {
                let mut fd = rfd::FileDialog::new().set_title("Export").add_filter(t.name, t.extensions).set_file_name(format!("{stem}.{ext}"));
                if let Some(dir) = self.settings.last_export_dir.clone().or_else(|| self.settings.last_dir.clone()) {
                    fd = fd.set_directory(dir);
                }
                match fd.save_file() {
                    Some(p) => p,
                    None => return,
                }
            }
        };
        self.settings.last_export_dir = path.parent().map(|p| p.to_path_buf());
        let d = self.doc().unwrap();
        let sel_doc;
        let (doc, pages): (&Document, Vec<usize>) = match st.range {
            2 => match selection_document(&d.doc, &d.selection) {
                Some(sd) => {
                    sel_doc = sd;
                    (&sel_doc, vec![0])
                }
                None => {
                    self.flash("Nothing selected.");
                    return;
                }
            },
            1 => (&d.doc, (0..d.doc.pages.len()).collect()),
            _ => (&d.doc, vec![d.page]),
        };
        let numbered = |i: usize| -> PathBuf {
            if pages.len() <= 1 {
                path.clone()
            } else {
                let s = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                path.with_file_name(format!("{s}-{}.{ext}", i + 1))
            }
        };
        let res: Result<(), String> = match ext {
            "pdf" => {
                let opts = vector_io::pdf::PdfOptions { pages: pages.clone(), raster_dpi: st.dpi, ..Default::default() };
                vector_io::pdf::export(doc, &path, &opts).map_err(|e| e.to_string())
            }
            "svg" => {
                let opts = vector_io::svg_export::SvgOptions { raster_dpi: st.dpi, ..Default::default() };
                pages.iter().try_for_each(|p| vector_io::svg_export::export(doc, *p, &numbered(*p), &opts).map_err(|e| e.to_string()))
            }
            _ => {
                let bg = if st.transparent && matches!(ext, "png" | "webp" | "tif" | "gif") { None } else { Some(Color::WHITE) };
                let opts = vector_io::image_io::RasterOptions { dpi: st.dpi, antialias: st.antialias, background: bg, jpeg_quality: st.quality, area: None, size: None };
                pages.iter().try_for_each(|p| vector_io::image_io::export_raster(doc, *p, &numbered(*p), &opts).map_err(|e| e.to_string()))
            }
        };
        match res {
            Ok(()) => self.flash(format!("Exported {}", path.display())),
            Err(e) => self.dialogs.message_box = Some(("Export".into(), format!("Export failed:\n{e}"))),
        }
    }

    pub fn publish_pdf(&mut self, pages: Option<Vec<usize>>) {
        let Some(d) = self.doc() else { return };
        let stem = d.display_name().trim_end_matches('*').trim_end_matches(".vct").to_string();
        let mut fd = rfd::FileDialog::new().set_title("Publish to PDF").add_filter("PDF - Portable Document Format (*.pdf)", &["pdf"]).set_file_name(format!("{stem}.pdf"));
        if let Some(dir) = self.settings.last_export_dir.clone() {
            fd = fd.set_directory(dir);
        }
        let Some(path) = fd.save_file() else { return };
        let opts = vector_io::pdf::PdfOptions { pages: pages.unwrap_or_default(), ..Default::default() };
        match vector_io::pdf::export(&self.doc().unwrap().doc, &path, &opts) {
            Ok(()) => {
                self.settings.last_export_dir = path.parent().map(|p| p.to_path_buf());
                self.flash(format!("Published {}", path.display()));
            }
            Err(e) => self.dialogs.message_box = Some(("Publish to PDF".into(), format!("Could not publish:\n{e}"))),
        }
    }

    /// Print through the Windows shell: the pages are written to a temporary PDF which is sent
    /// to the default printer with the "print" verb.
    pub fn print_pages(&mut self, pages: &[usize], copies: u32) {
        let Some(d) = self.doc() else { return };
        if pages.is_empty() {
            self.flash("No pages to print.");
            return;
        }
        let path = std::env::temp_dir().join(format!("Vector Print {}.pdf", d.uid));
        let opts = vector_io::pdf::PdfOptions { pages: pages.to_vec(), printing: true, ..Default::default() };
        if let Err(e) = vector_io::pdf::export(&d.doc, &path, &opts) {
            self.dialogs.message_box = Some(("Print".into(), format!("Could not prepare the print job:\n{e}")));
            return;
        }
        let mut ok = true;
        for _ in 0..copies.max(1) {
            let r = std::process::Command::new("powershell")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &format!("Start-Process -FilePath '{}' -Verb Print", path.display().to_string().replace('\'', "''"))])
                .spawn();
            ok &= r.is_ok();
        }
        if ok {
            self.flash(format!("Sent {} page(s) to the printer", pages.len()));
        } else {
            open_in_shell(&path);
        }
    }

    // -------------------------------------------------------------------------------- clipboard

    fn copy_selection(&mut self) -> bool {
        let Some(d) = self.doc() else { return false };
        if d.selection.is_empty() {
            return false;
        }
        let objs: Vec<Object> = d.selection.iter().filter_map(|id| d.doc.find(*id).cloned()).collect();
        let mut images = std::collections::BTreeMap::new();
        for o in &objs {
            o.walk(&mut |x| {
                if let ObjKind::Image(i) = &x.kind {
                    if let Some(data) = d.doc.images.get(&i.image) {
                        images.insert(i.image, data.clone());
                    }
                }
            });
        }
        let symbols = d.doc.symbols.clone();
        // system clipboard: SVG text + PNG image
        let svg = vector_io::svg_export::objects_to_svg(&d.doc, &objs);
        let png = vector_render::render_objects_image(&d.doc, &objs, 1024, None);
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(svg.replacen("<svg", "<svg data-vector-clip=\"1\"", 1));
            if let Some(pm) = png {
                let rgba = vector_render::pixmap_to_rgba(&pm);
                let _ = cb.set_image(arboard::ImageData { width: pm.width() as usize, height: pm.height() as usize, bytes: std::borrow::Cow::Owned(rgba) });
                // set_image replaces the text on Windows; keep the SVG as text for other apps
                let _ = cb.set_text(svg.replacen("<svg", "<svg data-vector-clip=\"1\"", 1));
            }
        }
        self.clip = crate::app::Clip { objects: objs, images, symbols };
        true
    }

    fn paste(&mut self) {
        if !self.has_doc() {
            self.new_default_document();
        }
        // what is on the system clipboard?
        let mut text = None;
        let mut image = None;
        if let Ok(mut cb) = arboard::Clipboard::new() {
            text = cb.get_text().ok();
            if text.is_none() {
                image = cb.get_image().ok();
            }
        }
        let ours = text.as_deref().map(|t| t.contains("data-vector-clip")).unwrap_or(false);
        if (ours || (text.is_none() && image.is_none())) && !self.clip.objects.is_empty() {
            let clip = self.clip.clone();
            let d = self.doc_mut().unwrap();
            let mut tmp = Document::new(1.0, 1.0);
            tmp.images = clip.images.clone();
            tmp.symbols = clip.symbols.iter().filter(|s| !d.doc.symbols.iter().any(|x| x.id == s.id)).cloned().collect();
            // keep symbol references to existing definitions
            tmp.pages[0].layers[0].objects = clip.objects.clone();
            let objs = vector_io::merge_document(&mut d.doc, tmp);
            d.add_objects("Paste", objs);
            return;
        }
        if let Some(t) = text {
            let t = t.trim().to_string();
            if t.starts_with("<svg") || t.starts_with("<?xml") {
                let d = self.doc_mut().unwrap();
                if let Ok(mut objs) = vector_io::svg_import::objects_from_svg(&mut d.doc, t.as_bytes()) {
                    let c = d.page_ref().rect().center();
                    let bb = vector_core::object::union_bounds(objs.iter().map(|o| o.bounds()));
                    for o in objs.iter_mut() {
                        o.apply_transform(Affine::translate(c - bb.center()));
                    }
                    d.add_objects("Paste", objs);
                    return;
                }
            }
            if !t.is_empty() {
                crate::tools::insert_text(self, &t, None);
                return;
            }
        }
        if let Some(img) = image {
            let d = self.doc_mut().unwrap();
            let data = vector_core::document::ImageData {
                width: img.width as u32,
                height: img.height as u32,
                rgba: std::sync::Arc::new(img.bytes.into_owned()),
                encoded: Default::default(),
                format: "png".into(),
                name: "Pasted image".into(),
            };
            let mut o = vector_io::image_io::image_object(&mut d.doc, data, 96.0);
            let c = d.page_ref().rect().center();
            let b = o.bounds();
            o.apply_transform(Affine::translate(c - b.center()));
            d.add_objects("Paste", vec![o]);
        }
    }

    // -------------------------------------------------------------------------------- bitmaps

    pub fn selected_image_size(&self) -> Option<(u32, u32)> {
        let d = self.doc()?;
        let id = *d.selection.first()?;
        match &d.doc.find(id)?.kind {
            ObjKind::Image(i) => d.doc.images.get(&i.image).map(|im| (im.width, im.height)),
            _ => None,
        }
    }

    pub fn resample_selected(&mut self, w: u32, h: u32) {
        let Some(d) = self.doc_mut() else { return };
        let Some(id) = d.selection.first().copied() else { return };
        let Some(ObjKind::Image(io)) = d.doc.find(id).map(|o| o.kind.clone()) else { return };
        let Some(src) = d.doc.images.get(&io.image).cloned() else { return };
        let Some(buf) = image::RgbaImage::from_raw(src.width, src.height, src.rgba.to_vec()) else { return };
        let out = image::imageops::resize(&buf, w.max(1), h.max(1), image::imageops::FilterType::Lanczos3);
        let data = vector_core::document::ImageData { width: out.width(), height: out.height(), rgba: std::sync::Arc::new(out.into_raw()), encoded: Default::default(), format: "png".into(), name: src.name.clone() };
        d.modify("Resample", |doc, _| {
            let nid = doc.add_image(data);
            if let Some(o) = doc.find_mut(id) {
                if let ObjKind::Image(i) = &mut o.kind {
                    i.image = nid;
                }
            }
        });
    }

    fn convert_to_bitmap(&mut self) {
        let Some(d) = self.doc_mut() else { return };
        let ids = d.selection.clone();
        let Some(sd) = selection_document(&d.doc, &ids) else { return };
        let bb = d.doc.selection_bounds(&ids).unwrap();
        let dpi = 300.0;
        let Some(pm) = vector_render::render_page_image(&sd, 0, dpi, None, None, true) else { return };
        let rgba = vector_render::pixmap_to_rgba(&pm);
        let data = vector_core::document::ImageData { width: pm.width(), height: pm.height(), rgba: std::sync::Arc::new(rgba), encoded: Default::default(), format: "png".into(), name: "Bitmap".into() };
        d.modify("Convert to Bitmap", |doc, sel| {
            let loc = doc.locate(ids[0]);
            for id in &ids {
                doc.remove(*id);
            }
            let mut o = vector_io::image_io::image_object(doc, data, dpi);
            o.apply_transform(Affine::translate(bb.origin().to_vec2()));
            let (p, l) = loc.map(|(p, l, _)| (p, l)).unwrap_or((0, 0));
            *sel = vec![o.id];
            doc.pages[p].layers[l].objects.push(o);
        });
    }

    fn open_trace(&mut self, quick: bool) {
        let Some(d) = self.doc() else { return };
        let Some(id) = d.selection.first().copied() else { return };
        let Some(o) = d.doc.find(id) else { return };
        let ObjKind::Image(io) = &o.kind else {
            self.flash("Select a bitmap to trace.");
            return;
        };
        let method = self.settings.quick_trace_method.clone();
        let st = TraceState { params: crate::trace::preset(&method), method, delete_original: false, group: true, preview: None, tex: None, image_id: io.image, obj_id: id };
        if quick {
            let img = d.doc.images.get(&io.image).cloned();
            let Some(img) = img else { return };
            let to_doc = o.transform * Affine::scale_non_uniform(io.w / img.width as f64, io.h / img.height as f64);
            let mut tmp = d.doc.clone();
            let r = crate::trace::trace(img.width, img.height, &img.rgba, &st.params, to_doc, || tmp.new_id());
            self.apply_trace(id, r.objects, false, true);
        } else {
            self.dialogs.trace = Some(st);
        }
    }

    pub fn apply_trace(&mut self, source: ObjId, mut objs: Vec<Object>, delete_original: bool, group: bool) {
        let Some(d) = self.doc_mut() else { return };
        if objs.is_empty() {
            return;
        }
        d.modify("Trace Bitmap", |doc, sel| {
            for o in objs.iter_mut() {
                doc.reassign_ids(o);
            }
            let (p, l, idx) = doc.locate(source).unwrap_or((0, 0, 0));
            let new: Vec<Object> = if group {
                let gid = doc.new_id();
                let mut g = Object::new(gid, ObjKind::Group(objs.clone()), Affine::IDENTITY, Default::default());
                g.name = "Traced bitmap".into();
                vec![g]
            } else {
                objs.clone()
            };
            *sel = new.iter().map(|o| o.id).collect();
            let layer = &mut doc.pages[p].layers[l].objects;
            let at = (idx + 1).min(layer.len());
            for (k, o) in new.into_iter().enumerate() {
                layer.insert(at + k, o);
            }
            if delete_original {
                doc.remove(source);
            }
        });
        self.flash("Bitmap traced");
    }

    // -------------------------------------------------------------------------------- object ops

    fn modify_sel(&mut self, label: &str, f: impl FnOnce(&mut Document, &mut Vec<ObjId>)) {
        if let Some(d) = self.doc_mut() {
            if d.selection.is_empty() {
                return;
            }
            d.modify(label, f);
        }
    }

    fn transform_sel(&mut self, label: &str, f: impl Fn(Rect) -> Affine) {
        if let Some(d) = self.doc_mut() {
            let ids = d.selection.clone();
            let Some(bb) = d.selection_bounds() else { return };
            let a = f(bb);
            d.modify(label, |doc, _| doc.transform_objects(&ids, a));
        }
    }

    fn set_all_objects(&mut self, label: &str, f: impl Fn(&mut Object)) {
        if let Some(d) = self.doc_mut() {
            let page = d.page;
            d.modify(label, |doc, _| {
                for l in doc.pages[page].layers.iter_mut() {
                    for o in l.objects.iter_mut() {
                        f(o);
                    }
                }
            });
        }
    }

    fn align_cmd(&mut self, how: ops::Align, to_page: bool) {
        let use_outline = self.dockers.ui.use_outline;
        let Some(d) = self.doc_mut() else { return };
        let ids = d.selection.clone();
        if ids.is_empty() {
            return;
        }
        let to = if to_page || ids.len() == 1 { AlignTo::Page } else { AlignTo::LastSelected };
        let page = d.page;
        d.modify("Align", |doc, _| ops::align(doc, &ids, how, to, page, use_outline));
    }

    fn select_all(&mut self, filter: impl Fn(&Object) -> bool) {
        if let Some(d) = self.doc_mut() {
            let page = d.page;
            let ids: Vec<ObjId> = d.doc.pages[page].layers.iter().filter(|l| l.visible && !l.locked).flat_map(|l| l.objects.iter()).filter(|o| o.visible && filter(o)).map(|o| o.id).collect();
            d.selection = ids;
        }
    }

    fn change_case(&mut self, mode: u8) {
        self.modify_sel("Change Case", |doc, sel| {
            for id in sel.iter() {
                if let Some(o) = doc.find_mut(*id) {
                    o.walk_mut(&mut |x| {
                        if let ObjKind::Text(t) = &mut x.kind {
                            t.content = match mode {
                                0 => t.content.to_lowercase(),
                                1 => t.content.to_uppercase(),
                                2 => title_case(&t.content),
                                3 => sentence_case(&t.content),
                                _ => t.content.chars().map(|c| if c.is_uppercase() { c.to_lowercase().next().unwrap_or(c) } else { c.to_uppercase().next().unwrap_or(c) }).collect(),
                            };
                        }
                    });
                }
            }
        });
    }

    fn table_op(&mut self, f: impl Fn(&mut vector_core::object::TableObj, Option<(usize, usize)>)) {
        let cell = self.ts.table_cell;
        self.modify_sel("Edit Table", |doc, sel| {
            for id in sel.iter() {
                if let Some(o) = doc.find_mut(*id) {
                    if let ObjKind::Table(t) = &mut o.kind {
                        f(t, cell);
                    }
                }
            }
        });
    }

    fn create_table(&mut self) {
        let (rows, cols) = (self.settings.table_rows as usize, self.settings.table_cols as usize);
        let Some(d) = self.doc_mut() else { return };
        let pr = d.page_ref().rect();
        let (w, h) = (pr.width() * 0.6, pr.height() * 0.25);
        let id = d.doc.new_id();
        let t = vector_core::object::TableObj::new(rows, cols, w, h);
        let o = Object::new(id, ObjKind::Table(t), Affine::translate((pr.center().x - w / 2.0, pr.center().y - h / 2.0)), Default::default());
        d.add_objects("Create Table", vec![o]);
    }

    fn apply_workspace(&mut self, i: usize) {
        let names = crate::dialogs::options::WORKSPACES;
        let Some((name, _)) = names.get(i) else { return };
        self.settings.workspace = name.to_string();
        let s = &mut self.settings;
        s.show_menu_bar = true;
        s.show_status_bar = true;
        s.show_standard = true;
        s.show_property_bar = true;
        s.show_toolbox = true;
        s.extra_toolbars.clear();
        s.button_size = crate::settings::ButtonSize::Small;
        self.dockers.open.clear();
        self.dockers.active = None;
        match *name {
            "Lite" => {
                self.dockers.show(DockerId::Properties);
            }
            "Touch" => {
                self.settings.button_size = crate::settings::ButtonSize::Large;
                self.settings.show_property_bar = true;
            }
            "Illustration" => {
                self.dockers.show(DockerId::Objects);
                self.dockers.show(DockerId::Properties);
            }
            _ => {}
        }
        self.dockers.save(&mut self.settings);
        self.flash(format!("Workspace: {name}"));
    }

    fn toggle_toolbar(&mut self, t: Toolbar) {
        let s = &mut self.settings;
        match t {
            Toolbar::MenuBar => s.show_menu_bar = !s.show_menu_bar,
            Toolbar::StatusBar => s.show_status_bar = !s.show_status_bar,
            Toolbar::Standard => s.show_standard = !s.show_standard,
            Toolbar::PropertyBar => s.show_property_bar = !s.show_property_bar,
            Toolbar::Toolbox => s.show_toolbox = !s.show_toolbox,
            other => {
                let n = other.name().to_string();
                if let Some(i) = s.extra_toolbars.iter().position(|x| *x == n) {
                    s.extra_toolbars.remove(i);
                } else {
                    s.extra_toolbars.push(n);
                }
            }
        }
    }

    fn palette_from(&mut self, from_selection: bool, into_document: bool) {
        let Some(d) = self.doc() else { return };
        let colors: Vec<Color> = if from_selection {
            let mut v = vec![];
            for id in &d.selection {
                if let Some(o) = d.doc.find(*id) {
                    o.walk(&mut |x| {
                        if let Some(c) = x.style.fill.main_color() {
                            v.push(c);
                        }
                        if let Some(c) = x.style.outline.color {
                            v.push(c);
                        }
                    });
                }
            }
            v
        } else {
            d.doc.used_colors()
        };
        if into_document {
            let d = self.doc_mut().unwrap();
            d.checkpoint("Add Colors");
            for c in colors {
                if !d.doc.palette.iter().any(|n| n.color == c) {
                    d.doc.palette.push(vector_core::NamedColor::new(c.to_hex(), c));
                }
            }
        } else {
            let mut p = vector_core::Palette { name: format!("Palette {}", self.palettes.palettes.len() + 1), colors: vec![] };
            for c in colors {
                if !p.colors.iter().any(|n| n.color == c) {
                    p.colors.push(vector_core::NamedColor::new(c.to_hex(), c));
                }
            }
            self.dialogs.palette_editor = Some(crate::dialogs::PaletteEditorState { palette: p, selected: None, path: None });
        }
    }

    fn info_box(&mut self, title: &str, text: &str) {
        self.dialogs.message_box = Some((title.to_string(), text.to_string()));
    }

    fn save_copy_to(&mut self, dir: PathBuf, what: &str) {
        let Some(d) = self.doc() else { return };
        let name = d.display_name().trim_end_matches('*').to_string();
        let name = if name.ends_with(".vct") { name } else { format!("{name}.vct") };
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join(name);
        match vector_io::native::save(&d.doc, &p) {
            Ok(()) => self.flash(format!("Saved a copy to {what}: {}", p.display())),
            Err(e) => self.info_box("Send To", &format!("Could not save: {e}")),
        }
    }

    // -------------------------------------------------------------------------------- dispatcher

    pub fn execute(&mut self, ctx: &egui::Context, cmd: Cmd) {
        if let Some(r) = self.recording.as_mut() {
            r.record(&cmd);
        }
        let repeatable = matches!(
            cmd,
            Cmd::Duplicate | Cmd::Delete | Cmd::Group | Cmd::Combine | Cmd::ConvertToCurves | Cmd::Order(_) | Cmd::Align(_) | Cmd::Distribute(_) | Cmd::Shaping(_) | Cmd::MirrorH | Cmd::MirrorV | Cmd::Rotate(_) | Cmd::Nudge(..) | Cmd::BitmapEffect(_) | Cmd::ChangeCase(_) | Cmd::Lock | Cmd::Unlock
        );
        if repeatable {
            self.last_cmd = Some(cmd);
        }
        let has_doc = self.has_doc();
        match cmd {
            // ------------------------------------------------------------------ File
            Cmd::New => self.new_document_cmd(),
            Cmd::NewFromTemplate => {
                let dir = crate::settings::config_dir().join("Templates");
                let _ = std::fs::create_dir_all(&dir);
                if let Some(p) = rfd::FileDialog::new().set_title("New From Template").add_filter("Vector Template (*.vct)", &["vct"]).set_directory(&dir).pick_file() {
                    if self.open_path(&p) {
                        if let Some(d) = self.doc_mut() {
                            d.path = None;
                            d.title = "Untitled.vct".into();
                            d.history.touch();
                        }
                    }
                }
            }
            Cmd::QuickAiGenerate => self.ai.quick_open = true,
            Cmd::AiGenerate => self.dockers.show(DockerId::AiGenerate),
            Cmd::Open => self.open_dialog(None),
            Cmd::OpenFromCloud => {
                let dir = self.settings.sync_folder.clone().or_else(|| rfd::FileDialog::new().set_title("Choose your cloud sync folder (OneDrive, Dropbox...)").pick_folder());
                if let Some(dir) = dir {
                    self.settings.sync_folder = Some(dir.clone());
                    self.open_dialog(Some(dir));
                }
            }
            Cmd::OpenRecent(i) => {
                if let Some(p) = self.settings.recent_files.get(i).cloned() {
                    if p.exists() {
                        self.open_path(&p);
                    } else {
                        self.settings.recent_files.remove(i);
                        self.info_box("Open Recent", &format!("The file {} no longer exists.", p.display()));
                    }
                }
            }
            Cmd::ClearRecent => self.settings.recent_files.clear(),
            Cmd::Close | Cmd::CloseWindow => {
                if let Some(i) = self.active {
                    self.request_close_doc(i);
                }
            }
            Cmd::CloseAll | Cmd::CloseAllWindows => self.close_all(),
            Cmd::Save => {
                if let Some(i) = self.active {
                    self.save_doc(i, false);
                }
            }
            Cmd::SaveAs => {
                if let Some(i) = self.active {
                    self.save_doc(i, true);
                }
            }
            Cmd::SaveToCloud => {
                let dir = self.settings.sync_folder.clone().or_else(|| rfd::FileDialog::new().set_title("Choose your cloud sync folder (OneDrive, Dropbox...)").pick_folder());
                if let Some(dir) = dir {
                    self.settings.sync_folder = Some(dir.clone());
                    self.save_copy_to(dir, "the cloud folder");
                }
            }
            Cmd::SaveAsTemplate => {
                if let Some(d) = self.doc() {
                    let dir = crate::settings::config_dir().join("Templates");
                    let _ = std::fs::create_dir_all(&dir);
                    let stem = d.display_name().trim_end_matches('*').trim_end_matches(".vct").to_string();
                    if let Some(p) = rfd::FileDialog::new().set_title("Save As Template").add_filter("Vector Template (*.vct)", &["vct"]).set_directory(&dir).set_file_name(format!("{stem}.vct")).save_file() {
                        match vector_io::native::save(&d.doc, &p) {
                            Ok(()) => self.flash("Template saved"),
                            Err(e) => self.info_box("Save As Template", &e.to_string()),
                        }
                    }
                }
            }
            Cmd::Revert => {
                if let Some(d) = self.doc_mut() {
                    if let Some(p) = d.path.clone() {
                        match vector_io::open(&p) {
                            Ok(mut doc) => {
                                doc.ensure_next_id();
                                d.checkpoint("Revert");
                                d.doc = doc;
                                d.selection.clear();
                                d.after_change();
                                d.history.mark_saved();
                                d.cache.key = 0;
                            }
                            Err(e) => {
                                let msg = e.to_string();
                                self.info_box("Revert", &msg);
                            }
                        }
                    }
                }
            }
            Cmd::SelectTwain | Cmd::SelectWia => self.info_box("Acquire Image", "Scanners and cameras are accessed through Windows. Scan your image with the Windows Scan app, then use File > Acquire Image > Acquire... or Import to bring it into the drawing."),
            Cmd::Acquire | Cmd::Import => self.import_dialog(),
            Cmd::Export | Cmd::ExportForWeb | Cmd::ExportForOffice | Cmd::ExportForInkjet => {
                if has_doc {
                    let mut st = ExportState::default();
                    match cmd {
                        Cmd::ExportForWeb => {
                            st.format = 2;
                            st.dpi = 96.0;
                        }
                        Cmd::ExportForOffice => {
                            st.format = 2;
                            st.dpi = 300.0;
                            st.transparent = false;
                        }
                        Cmd::ExportForInkjet => st.format = 0,
                        _ => {
                            if self.has_selection() {
                                st.range = 2;
                            }
                        }
                    }
                    self.dialogs.export = Some(st);
                }
            }
            Cmd::ExportDockerOpen => self.dockers.show(DockerId::Export),
            Cmd::SendToMail => {
                if let Some(d) = self.doc() {
                    let stem = d.display_name().trim_end_matches('*').trim_end_matches(".vct").to_string();
                    let p = std::env::temp_dir().join(format!("{stem}.pdf"));
                    if vector_io::pdf::export(&d.doc, &p, &Default::default()).is_ok() {
                        reveal_in_explorer(&p);
                        let _ = std::process::Command::new("cmd").args(["/C", "start", "", &format!("mailto:?subject={}", stem.replace(' ', "%20"))]).spawn();
                        self.flash("A PDF of the drawing is ready to attach to your message.");
                    }
                }
            }
            Cmd::SendToDesktop => {
                if let Some(dir) = dirs::desktop_dir() {
                    self.save_copy_to(dir, "the Desktop");
                }
            }
            Cmd::SendToDocuments => self.save_copy_to(crate::settings::documents_dir(), "Documents"),
            Cmd::PublishPdf => self.publish_pdf(None),
            Cmd::PrintMergeCreate | Cmd::PrintMergeEdit | Cmd::PrintMergePerform => {
                self.dockers.show(DockerId::ObjectData);
                self.flash("Print Merge uses Object Data fields: add fields in the Object Data docker.");
            }
            Cmd::Print => {
                if let Some(d) = self.doc() {
                    let mut st = PrintState::new(false, d.page);
                    if self.settings.print_only_current {
                        st.range = 0;
                    }
                    self.dialogs.print = Some(st);
                }
            }
            Cmd::PrintPreview => {
                if let Some(d) = self.doc() {
                    self.dialogs.print = Some(PrintState::new(true, d.page));
                }
            }
            Cmd::DocumentProperties => self.dialogs.doc_props = has_doc,
            Cmd::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),

            // ------------------------------------------------------------------ Edit
            Cmd::Undo => {
                if self.ts.is_busy() {
                    self.ts.cancel();
                } else if let Some(d) = self.doc_mut() {
                    d.undo();
                    d.cache.key = 0;
                }
            }
            Cmd::Redo => {
                if let Some(d) = self.doc_mut() {
                    d.redo();
                    d.cache.key = 0;
                }
            }
            Cmd::UndoTo(n) => {
                if let Some(d) = self.doc_mut() {
                    for _ in 0..n {
                        d.undo();
                    }
                }
            }
            Cmd::RedoTo(n) => {
                if let Some(d) = self.doc_mut() {
                    for _ in 0..n {
                        d.redo();
                    }
                }
            }
            Cmd::Repeat => {
                if let Some(c) = self.last_cmd {
                    self.execute(ctx, c);
                }
            }
            Cmd::Cut => {
                if self.copy_selection() {
                    self.modify_sel("Cut", |doc, sel| {
                        for id in sel.drain(..) {
                            doc.remove(id);
                        }
                    });
                }
            }
            Cmd::Copy => {
                self.copy_selection();
            }
            Cmd::Paste | Cmd::PasteSpecial => self.paste(),
            Cmd::NewFromClipboard => {
                self.new_default_document();
                self.paste();
            }
            Cmd::Delete => {
                if self.tool == ToolId::Shape && self.doc().map(|d| !d.node_sel.is_empty()).unwrap_or(false) {
                    crate::tools::node_op(self, crate::tools::NodeOp::Delete);
                } else {
                    self.modify_sel("Delete", |doc, sel| {
                        for id in sel.drain(..) {
                            if doc.find(id).map(|o| !o.locked).unwrap_or(false) {
                                doc.remove(id);
                            }
                        }
                    });
                }
            }
            Cmd::Duplicate | Cmd::Clone => {
                let off = self.doc().map(|d| d.doc.settings.duplicate_offset).unwrap_or(Vec2::new(18.0, 18.0));
                self.modify_sel("Duplicate", |doc, sel| {
                    let ids = sel.clone();
                    *sel = ops::duplicate(doc, &ids, off);
                });
            }
            Cmd::StepAndRepeat => self.dockers.show(DockerId::StepRepeat),
            Cmd::CopyPropertiesFrom => {
                if self.has_selection() {
                    self.set_tool(ToolId::Pick);
                    self.ts.pick_target = Some(PickTarget::CopyProperties);
                    self.flash("Click the object to copy properties from.");
                }
            }
            Cmd::CopyEffectFrom => {
                if self.has_selection() {
                    self.set_tool(ToolId::Pick);
                    self.ts.pick_target = Some(PickTarget::CopyEffect);
                    self.flash("Click the object to copy the effect from.");
                }
            }
            Cmd::SelectAll => {
                if self.tool == ToolId::Shape {
                    crate::tools::node_op(self, crate::tools::NodeOp::SelectAll);
                } else {
                    self.select_all(|_| true);
                }
            }
            Cmd::SelectAllText => self.select_all(|o| matches!(o.kind, ObjKind::Text(_))),
            Cmd::SelectAllGuides => {
                if let Some(d) = self.doc_mut() {
                    d.selection.clear();
                    let n = d.page_ref().guides.len();
                    self.flash(format!("{n} guidelines on this page (edit them in the Guidelines docker)."));
                    self.dockers.show(DockerId::Guidelines);
                }
            }
            Cmd::SelectAllNodes => {
                self.set_tool(ToolId::Shape);
                crate::tools::node_op(self, crate::tools::NodeOp::SelectAll);
            }
            Cmd::Deselect => {
                if let Some(d) = self.doc_mut() {
                    d.selection.clear();
                    d.node_sel.clear();
                }
            }
            Cmd::FindReplace => self.dockers.show(DockerId::FindReplace),

            // ------------------------------------------------------------------ View
            Cmd::ViewMode(m) => {
                if let Some(d) = self.doc_mut() {
                    d.view_mode = m;
                    d.cache.key = 0;
                }
            }
            Cmd::FullScreenPreview => self.fullscreen_preview = has_doc && !self.fullscreen_preview,
            Cmd::PageSorterView => {
                if let Some(d) = self.doc_mut() {
                    d.show_page_sorter = !d.show_page_sorter;
                }
            }
            Cmd::ToggleRulers | Cmd::ToggleGrid | Cmd::TogglePixelGrid | Cmd::ToggleGuidelines | Cmd::TogglePageBorder | Cmd::ToggleBleed | Cmd::SnapOff | Cmd::SnapToGrid | Cmd::SnapToGuides | Cmd::SnapToObjects | Cmd::SnapToPage => {
                if let Some(d) = self.doc_mut() {
                    let s = &mut d.doc.settings;
                    match cmd {
                        Cmd::ToggleRulers => s.show_rulers = !s.show_rulers,
                        Cmd::ToggleGrid => s.show_grid = !s.show_grid,
                        Cmd::TogglePixelGrid => s.pixel_grid = !s.pixel_grid,
                        Cmd::ToggleGuidelines => s.show_guides = !s.show_guides,
                        Cmd::TogglePageBorder => s.show_page_border = !s.show_page_border,
                        Cmd::ToggleBleed => s.show_bleed = !s.show_bleed,
                        Cmd::SnapOff => s.snap_off = !s.snap_off,
                        Cmd::SnapToGrid => s.snap_to_grid = !s.snap_to_grid,
                        Cmd::SnapToGuides => s.snap_to_guides = !s.snap_to_guides,
                        Cmd::SnapToObjects => s.snap_to_objects = !s.snap_to_objects,
                        _ => s.snap_to_page = !s.snap_to_page,
                    }
                    d.cache.key = 0;
                }
                if cmd == Cmd::SnapToObjects {
                    self.settings.snap_to_objects = !self.settings.snap_to_objects;
                }
            }
            Cmd::ZoomIn | Cmd::ZoomOut | Cmd::ZoomToSelected | Cmd::ZoomToAll | Cmd::ZoomToPage | Cmd::ZoomToWidth | Cmd::ZoomToHeight | Cmd::ZoomActual | Cmd::ZoomSet(_) => {
                let rate = self.settings.zoom_factor();
                if let Some(d) = self.doc_mut() {
                    let c = d.canvas_rect.center();
                    match cmd {
                        Cmd::ZoomIn => d.zoom_at(c, rate.max(1.25)),
                        Cmd::ZoomOut => d.zoom_at(c, 1.0 / rate.max(1.25)),
                        Cmd::ZoomToSelected => {
                            if let Some(bb) = d.selection_bounds() {
                                d.zoom_to_rect(bb, 30.0);
                            }
                        }
                        Cmd::ZoomToAll => {
                            let page = d.page;
                            let b = vector_core::object::union_bounds(d.doc.pages[page].all_objects().map(|o| o.bounds()));
                            if b.width() > 0.0 || b.height() > 0.0 {
                                d.zoom_to_rect(b, 30.0);
                            } else {
                                d.zoom_page();
                            }
                        }
                        Cmd::ZoomToPage => d.zoom_page(),
                        Cmd::ZoomToWidth => d.zoom_page_width(),
                        Cmd::ZoomToHeight => d.zoom_page_height(),
                        Cmd::ZoomActual => d.set_zoom_percent(100.0),
                        Cmd::ZoomSet(p) => d.set_zoom_percent(p),
                        _ => {}
                    }
                }
            }

            // ------------------------------------------------------------------ Layout
            Cmd::InsertPage => {
                if has_doc {
                    self.dialogs.insert_page = Some((1, false));
                }
            }
            Cmd::DuplicatePage => {
                if let Some(d) = self.doc_mut() {
                    let cur = d.page;
                    d.modify("Duplicate Page", |doc, sel| {
                        let mut p = doc.pages[cur].clone();
                        p.id = doc.new_id();
                        for l in p.layers.iter_mut() {
                            l.id = doc.new_id();
                            for o in l.objects.iter_mut() {
                                doc.reassign_ids(o);
                            }
                        }
                        doc.pages.insert(cur + 1, p);
                        sel.clear();
                    });
                    d.page = cur + 1;
                }
            }
            Cmd::RenamePage => {
                if let Some(d) = self.doc() {
                    self.dialogs.rename_page = Some((d.page, d.doc.page_name(d.page)));
                }
            }
            Cmd::DeletePage => {
                if let Some(d) = self.doc_mut() {
                    if d.doc.pages.len() > 1 {
                        let cur = d.page;
                        d.modify("Delete Page", |doc, sel| {
                            doc.pages.remove(cur);
                            sel.clear();
                        });
                    } else {
                        self.flash("A document must have at least one page.");
                    }
                }
            }
            Cmd::GoToPage => {
                if let Some(d) = self.doc() {
                    self.dialogs.goto_page = Some(d.page + 1);
                }
            }
            Cmd::NextPage | Cmd::PrevPage => {
                if let Some(d) = self.doc_mut() {
                    let n = d.doc.pages.len();
                    let p = if cmd == Cmd::NextPage { (d.page + 1).min(n - 1) } else { d.page.saturating_sub(1) };
                    if p != d.page {
                        d.page = p;
                        d.selection.clear();
                        d.node_sel.clear();
                        d.cache.key = 0;
                    }
                }
            }
            Cmd::SwitchOrientation | Cmd::Landscape(_) => {
                if let Some(d) = self.doc_mut() {
                    let cur = d.page;
                    let land = d.page_ref().is_landscape();
                    let want = match cmd {
                        Cmd::Landscape(b) => b,
                        _ => !land,
                    };
                    if want != land {
                        let all = d.doc.settings.apply_to_all_pages;
                        d.modify("Page Orientation", |doc, _| {
                            for (i, p) in doc.pages.iter_mut().enumerate() {
                                if all || i == cur {
                                    std::mem::swap(&mut p.width, &mut p.height);
                                }
                            }
                        });
                        d.needs_fit = true;
                    }
                }
            }
            Cmd::PageSetup => self.open_options(OptionsPage::Document, Some("Page Size")),
            Cmd::PageBackground => self.open_options(OptionsPage::Document, Some("Background")),

            // ------------------------------------------------------------------ Object
            Cmd::Group => self.modify_sel("Group", |doc, sel| {
                let ids = sel.clone();
                if let Some(g) = ops::group(doc, &ids) {
                    *sel = vec![g];
                }
            }),
            Cmd::Ungroup | Cmd::UngroupAll => self.modify_sel("Ungroup", |doc, sel| {
                let ids = sel.clone();
                *sel = ops::ungroup(doc, &ids, cmd == Cmd::UngroupAll);
            }),
            Cmd::Combine => self.modify_sel("Combine", |doc, sel| {
                let ids = sel.clone();
                if let Some(c) = ops::combine(doc, &ids) {
                    *sel = vec![c];
                }
            }),
            Cmd::BreakApart => self.modify_sel("Break Apart", |doc, sel| {
                let ids = sel.clone();
                *sel = ops::break_apart(doc, &ids);
            }),
            Cmd::Lock | Cmd::Unlock => {
                let lock = cmd == Cmd::Lock;
                self.modify_sel(if lock { "Lock Object" } else { "Unlock Object" }, |doc, _sel| {
                    for id in _sel.iter() {
                        if let Some(o) = doc.find_mut(*id) {
                            o.locked = lock;
                        }
                    }
                });
            }
            Cmd::UnlockAll => self.set_all_objects("Unlock All Objects", |o| o.locked = false),
            Cmd::HideObject => self.modify_sel("Hide Object", |doc, sel| {
                for id in sel.drain(..) {
                    if let Some(o) = doc.find_mut(id) {
                        o.visible = false;
                    }
                }
            }),
            Cmd::ShowAllObjects => self.set_all_objects("Show All Objects", |o| o.walk_mut(&mut |x| x.visible = true)),
            Cmd::Order(o) => self.modify_sel("Order", |doc, sel| ops::reorder(doc, sel, o)),
            Cmd::ReverseOrder => self.modify_sel("Reverse Order", |doc, sel| ops::reverse_order(doc, sel)),
            Cmd::Align(a) => self.align_cmd(a, false),
            Cmd::AlignTo(a) => self.align_cmd(a, true),
            Cmd::Distribute(how) => self.modify_sel("Distribute", |doc, sel| ops::distribute(doc, sel, how, None)),
            Cmd::Shaping(op) => {
                let (ls, lt) = (self.ts.leave_source, self.ts.leave_target);
                self.modify_sel("Shaping", |doc, sel| {
                    let ids = sel.clone();
                    let r = ops::shaping(doc, &ids, op, ls, lt);
                    if !r.is_empty() {
                        *sel = r;
                    }
                });
            }
            Cmd::ConvertToCurves => self.modify_sel("Convert to Curves", |doc, sel| {
                let ids = sel.clone();
                *sel = ops::convert_to_curves(doc, &ids);
            }),
            Cmd::OutlineToObject => self.modify_sel("Convert Outline to Object", |doc, sel| {
                let ids = sel.clone();
                *sel = ops::outline_to_object(doc, &ids);
            }),
            Cmd::JoinCurves => {
                let tol = self.dockers.ui.join_tolerance.max(0.01);
                self.modify_sel("Join Curves", |doc, sel| {
                    let ids = sel.clone();
                    if let Some(j) = ops::join_curves(doc, &ids, tol) {
                        *sel = vec![j];
                    }
                });
            }
            Cmd::PowerClipInside => {
                if self.has_selection() {
                    self.set_tool(ToolId::Pick);
                    self.ts.pick_target = Some(PickTarget::PowerClip);
                    self.flash("Click the frame to place the selection inside.");
                }
            }
            Cmd::PowerClipExtract => self.modify_sel("Extract Contents", |doc, sel| {
                let ids = sel.clone();
                let mut out = vec![];
                for id in ids {
                    out.extend(ops::extract_powerclip(doc, id));
                }
                *sel = out;
            }),
            Cmd::PowerClipEdit => {
                if let Some(d) = self.doc_mut() {
                    if let Some(id) = d.selection.first().copied() {
                        if matches!(d.doc.find(id).map(|o| &o.kind), Some(ObjKind::PowerClip { .. }) | Some(ObjKind::Group(_))) {
                            d.focus = Some(id);
                            d.selection.clear();
                            d.cache.key = 0;
                        }
                    }
                }
            }
            Cmd::PowerClipFinish => {
                if let Some(d) = self.doc_mut() {
                    if let Some(f) = d.focus.take() {
                        d.selection = vec![f];
                        d.cache.key = 0;
                    }
                }
            }
            Cmd::MirrorH | Cmd::MirrorV => self.modify_sel("Mirror", |doc, sel| ops::mirror(doc, sel, cmd == Cmd::MirrorH)),
            Cmd::Rotate(deg) => self.transform_sel("Rotate", |bb| rotate_about(bb.center(), deg)),
            Cmd::ClearTransformations => self.modify_sel("Clear Transformations", |doc, sel| {
                for id in sel.iter() {
                    if let Some(o) = doc.find_mut(*id) {
                        let before = o.bounds().center();
                        let c = o.transform.as_coeffs();
                        o.transform = Affine::translate((c[4], c[5]));
                        let after = o.bounds().center();
                        o.transform = Affine::translate(before - after) * o.transform;
                    }
                }
            }),
            Cmd::NewSymbol => new_symbol(self, ""),
            Cmd::RevertSymbol => self.modify_sel("Revert to Objects", |doc, sel| {
                let ids = sel.clone();
                let mut out = vec![];
                for id in ids {
                    let Some(o) = doc.find(id).cloned() else { continue };
                    let ObjKind::Symbol(sid) = o.kind else { continue };
                    let Some(def) = doc.symbols.iter().find(|s| s.id == sid).cloned() else { continue };
                    let mut children = def.objects.clone();
                    for c in children.iter_mut() {
                        doc.reassign_ids(c);
                        c.apply_transform(o.transform);
                    }
                    let gid = doc.new_id();
                    let g = Object::new(gid, ObjKind::Group(children), Affine::IDENTITY, Default::default());
                    if let Some(x) = doc.find_mut(id) {
                        *x = g;
                    }
                    out.push(gid);
                }
                *sel = out;
            }),
            Cmd::ObjectProperties => self.dockers.toggle(DockerId::Properties),
            Cmd::ClearEffect => self.modify_sel("Clear Effect", |doc, sel| {
                for id in sel.iter() {
                    if let Some(o) = doc.find_mut(*id) {
                        o.effects = Default::default();
                    }
                }
            }),
            Cmd::BreakBlend => self.modify_sel("Break Blend Group Apart", |doc, sel| {
                let ids = sel.clone();
                let mut out = vec![];
                for id in ids {
                    out.extend(ops::break_blend(doc, id));
                }
                *sel = out;
            }),
            Cmd::AddPerspective => self.modify_sel("Add Perspective", |doc, sel| {
                for id in sel.iter() {
                    if let Some(o) = doc.find_mut(*id) {
                        o.effects.envelope = Some(vector_core::object::EnvelopeEffect { handles: [kurbo::Vec2::ZERO; 8] });
                    }
                }
            }),
            Cmd::ClearPerspective => self.modify_sel("Clear Perspective", |doc, sel| {
                for id in sel.iter() {
                    if let Some(o) = doc.find_mut(*id) {
                        o.effects.envelope = None;
                    }
                }
            }),
            Cmd::BevelEffect => self.dockers.show(DockerId::Bevel),
            Cmd::LensEffect => self.dockers.show(DockerId::Lens),

            // ------------------------------------------------------------------ Bitmaps
            Cmd::ConvertToBitmap => self.convert_to_bitmap(),
            Cmd::QuickTrace => self.open_trace(true),
            Cmd::OutlineTrace => self.open_trace(false),
            Cmd::BitmapEffect(i) => {
                let e = bitmap_effect(i as usize, 4.0);
                self.modify_sel("Bitmap Effect", |doc, sel| {
                    for id in sel.iter() {
                        if let Some(o) = doc.find_mut(*id) {
                            o.effects.bitmap.push(e.clone());
                        }
                    }
                });
            }
            Cmd::AutoAdjust => self.modify_sel("Auto Adjust", |doc, sel| {
                for id in sel.iter() {
                    if let Some(o) = doc.find_mut(*id) {
                        if let ObjKind::Image(i) = &mut o.kind {
                            i.adjust = ImageAdjust { brightness: 4.0, contrast: 12.0, saturation: 8.0, ..i.adjust.clone() };
                        }
                    }
                }
            }),
            Cmd::ResampleBitmap => {
                if let Some((w, h)) = self.selected_image_size() {
                    self.dialogs.resample = Some((w as f64, h as f64, true));
                }
            }

            // ------------------------------------------------------------------ Text
            Cmd::EditText => {
                self.set_tool(ToolId::Text);
                self.dockers.show(DockerId::Text);
            }
            Cmd::FitTextToPath => {
                let sel = self.sel();
                let pair = self.doc().and_then(|d| {
                    let t = sel.iter().find(|id| matches!(d.doc.find(**id).map(|o| &o.kind), Some(ObjKind::Text(_))))?;
                    let p = sel.iter().find(|id| *id != t)?;
                    Some((*t, *p))
                });
                match pair {
                    Some((t, p)) => self.modify_sel("Fit Text to Path", |doc, sel| {
                        if ops::fit_text_to_path(doc, t, p) {
                            *sel = vec![t];
                        }
                    }),
                    None => {
                        if self.has_selection() {
                            self.set_tool(ToolId::Pick);
                            self.ts.pick_target = Some(PickTarget::FitToPath);
                            self.flash("Click the path to fit the text to.");
                        }
                    }
                }
            }
            Cmd::StraightenText => self.modify_sel("Straighten Text", |doc, sel| {
                for id in sel.iter() {
                    if let Some(o) = doc.find_mut(*id) {
                        if let ObjKind::Text(t) = &mut o.kind {
                            t.on_path = None;
                        }
                    }
                }
            }),
            Cmd::ConvertText => self.modify_sel("Convert Text", |doc, sel| {
                for id in sel.iter() {
                    if let Some(o) = doc.find_mut(*id) {
                        let b = o.bounds();
                        if let ObjKind::Text(t) = &mut o.kind {
                            t.kind = match t.kind {
                                vector_core::text::TextKind::Artistic => vector_core::text::TextKind::Paragraph { width: b.width().max(20.0) * 1.05, height: b.height().max(20.0) * 1.2 },
                                vector_core::text::TextKind::Paragraph { .. } => vector_core::text::TextKind::Artistic,
                            };
                        }
                    }
                }
            }),
            Cmd::InsertGlyph => self.dockers.show(DockerId::Glyphs),
            Cmd::ChangeCase(m) => self.change_case(m),
            Cmd::SpellCheck => {
                let mut words = 0;
                let mut repeated = vec![];
                if let Some(d) = self.doc() {
                    for p in &d.doc.pages {
                        for o in p.all_objects() {
                            o.walk(&mut |x| {
                                if let ObjKind::Text(t) = &x.kind {
                                    let ws: Vec<String> = t.content.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()).collect();
                                    words += ws.len();
                                    for w in ws.windows(2) {
                                        if w[0] == w[1] && !w[0].is_empty() {
                                            repeated.push(w[0].clone());
                                        }
                                    }
                                }
                            });
                        }
                    }
                }
                let msg = if repeated.is_empty() { format!("Spell check complete. {words} words checked, no repeated words found.") } else { format!("Spell check complete. {words} words checked.\nRepeated words: {}", repeated.join(", ")) };
                self.info_box("Spell Check", &msg);
            }

            // ------------------------------------------------------------------ Table
            Cmd::CreateTable => self.create_table(),
            Cmd::TableInsertRow(above) => self.table_op(|t, cell| {
                let r = cell.map(|c| c.0).unwrap_or(t.rows - 1);
                t.insert_row(if above { r } else { r + 1 });
            }),
            Cmd::TableInsertCol(left) => self.table_op(|t, cell| {
                let c = cell.map(|c| c.1).unwrap_or(t.cols - 1);
                t.insert_col(if left { c } else { c + 1 });
            }),
            Cmd::TableDeleteRow => self.table_op(|t, cell| {
                if t.rows > 1 {
                    t.delete_row(cell.map(|c| c.0).unwrap_or(t.rows - 1).min(t.rows - 1));
                }
            }),
            Cmd::TableDeleteCol => self.table_op(|t, cell| {
                if t.cols > 1 {
                    t.delete_col(cell.map(|c| c.1).unwrap_or(t.cols - 1).min(t.cols - 1));
                }
            }),
            Cmd::TableDistributeRows => self.table_op(|t, _| {
                let h: f64 = t.row_heights.iter().sum::<f64>() / t.rows as f64;
                t.row_heights.iter_mut().for_each(|x| *x = h);
            }),
            Cmd::TableDistributeCols => self.table_op(|t, _| {
                let w: f64 = t.col_widths.iter().sum::<f64>() / t.cols as f64;
                t.col_widths.iter_mut().for_each(|x| *x = w);
            }),

            // ------------------------------------------------------------------ Tools
            Cmd::Options(p) => self.open_options(p, None),
            Cmd::SaveSettingsAsDefault => {
                self.dockers.save(&mut self.settings);
                self.settings.save();
                self.flash("Settings saved as default");
            }
            Cmd::ColorManagement => self.info_box("Color Management", "Vector works in sRGB. CMYK values shown in the color dockers are converted with a standard (uncalibrated) formula; use Color Proofing to preview print output."),
            Cmd::ScriptEditor | Cmd::VsEditor => {
                if self.dialogs.script_editor.is_none() {
                    self.dialogs.script_editor = Some(ScriptEditorState { code: crate::dialogs::forms::SCRIPT_TEMPLATE.to_string(), ..Default::default() });
                }
            }
            Cmd::RunScript => {
                if let Some(p) = rfd::FileDialog::new().set_title("Run Script").add_filter("Vector script", &["rhai", "vsc"]).set_directory(crate::scripting::scripts_dir()).pick_file() {
                    match std::fs::read_to_string(&p) {
                        Ok(src) => {
                            if let Err(e) = crate::scripting::run_script(self, &src) {
                                self.info_box("Run Script", &format!("Error: {e}"));
                            }
                        }
                        Err(e) => self.info_box("Run Script", &e.to_string()),
                    }
                }
            }
            Cmd::StartRecording => {
                self.recording = Some(crate::scripting::Recorder::new(false));
                self.flash("Recording... (Tools > Scripts > Stop Recording to finish)");
            }
            Cmd::RecordTemporary => {
                self.recording = Some(crate::scripting::Recorder::new(true));
                self.flash("Recording temporary script...");
            }
            Cmd::PauseRecording => {
                if let Some(r) = self.recording.as_mut() {
                    r.paused = !r.paused;
                }
            }
            Cmd::StopRecording => {
                if let Some(r) = self.recording.take() {
                    let script = r.script();
                    self.temp_script = script.clone();
                    if r.temporary {
                        self.flash("Temporary script recorded (Ctrl+Shift+P to run)");
                    } else {
                        self.dialogs.script_editor = Some(ScriptEditorState { code: script, dirty: true, ..Default::default() });
                    }
                }
            }
            Cmd::CancelRecording => {
                self.recording = None;
                self.flash("Recording cancelled");
            }
            Cmd::RunTemporary => {
                let s = self.temp_script.clone();
                if s.is_empty() {
                    self.flash("No temporary script recorded.");
                } else if let Err(e) = crate::scripting::run_script(self, &s) {
                    self.info_box("Run Temporary Script", &format!("Error: {e}"));
                }
            }

            // ------------------------------------------------------------------ Window
            Cmd::NewWindow => {
                if let Some(d) = self.doc() {
                    let mut dv = DocView::new(d.doc.clone(), format!("{} (copy)", d.display_name().trim_end_matches('*')), self.settings.undo_levels);
                    dv.history.touch();
                    self.push_doc(dv);
                }
            }
            Cmd::RefreshWindow => {
                for d in self.docs.iter_mut() {
                    d.cache.key = 0;
                }
                self.ts.texture_cache.clear();
            }
            Cmd::Cascade | Cmd::TileH | Cmd::TileV | Cmd::CombineWindows | Cmd::DockWindow => self.flash("Documents are shown as tabs; drag a tab to reorder."),
            Cmd::Workspace(i) => self.apply_workspace(i as usize),
            Cmd::WorkspaceNew | Cmd::WorkspaceExport => {
                self.dockers.save(&mut self.settings);
                if let Some(p) = rfd::FileDialog::new().set_title("Export Workspace").add_filter("Vector workspace", &["vws"]).set_file_name("Workspace.vws").save_file() {
                    if let Ok(b) = serde_json::to_vec_pretty(&self.settings) {
                        let _ = std::fs::write(p, b);
                        self.flash("Workspace exported");
                    }
                }
            }
            Cmd::WorkspaceImport => {
                if let Some(p) = rfd::FileDialog::new().set_title("Import Workspace").add_filter("Vector workspace", &["vws"]).pick_file() {
                    match std::fs::read(&p).ok().and_then(|b| serde_json::from_slice::<crate::settings::Settings>(&b).ok()) {
                        Some(mut s) => {
                            s.recent_files = std::mem::take(&mut self.settings.recent_files);
                            self.settings = s;
                            self.dockers = crate::dockers::DockerState::from_settings(&self.settings);
                            self.flash("Workspace imported");
                        }
                        None => self.info_box("Import Workspace", "The file is not a Vector workspace."),
                    }
                }
            }
            Cmd::WorkspaceDelete => self.apply_workspace(0),
            Cmd::Docker(id) => self.dockers.toggle(id),
            Cmd::ToggleToolbar(t) => self.toggle_toolbar(t),
            Cmd::ButtonSize(s) => self.settings.button_size = s,
            Cmd::LockToolbars => self.settings.lock_toolbars = !self.settings.lock_toolbars,
            Cmd::AddColorsFromSelection => self.palette_from(true, true),
            Cmd::AddColorsFromDocument => self.palette_from(false, true),
            Cmd::CreatePaletteFromSelection => self.palette_from(true, false),
            Cmd::CreatePaletteFromDocument => self.palette_from(false, false),
            Cmd::OpenPalette => {
                if let Some(p) = rfd::FileDialog::new().set_title("Open Palette").add_filter("Palettes", &["vpal", "txt", "gpl"]).pick_file() {
                    if let Ok(t) = std::fs::read_to_string(&p) {
                        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                        let pal = vector_core::Palette::parse(&name, &t);
                        self.palettes.recent.retain(|r| *r != name);
                        self.palettes.recent.insert(0, name);
                        self.palettes.palettes.push(pal);
                        self.settings.default_palette_visible = true;
                    }
                }
            }
            Cmd::CloseAllPalettes => {
                self.palettes.palettes.truncate(1);
                self.settings.default_palette_visible = false;
            }
            Cmd::PaletteEditor => {
                let p = self.palettes.palettes.first().cloned().unwrap_or_else(vector_core::Palette::default_palette);
                self.dialogs.palette_editor = Some(crate::dialogs::PaletteEditorState { palette: p, selected: None, path: None });
            }
            Cmd::ToggleDefaultPalette => self.settings.default_palette_visible = !self.settings.default_palette_visible,
            Cmd::ToggleDocumentPalette => self.settings.doc_palette_visible = !self.settings.doc_palette_visible,
            Cmd::ToggleColorStylesPalette => self.settings.color_styles_palette_visible = !self.settings.color_styles_palette_visible,
            Cmd::WindowList(i) => {
                if i < self.docs.len() {
                    self.finish_tool();
                    self.active = Some(i);
                }
            }

            // ------------------------------------------------------------------ Help
            Cmd::ProductHelp | Cmd::UserGuide | Cmd::QuickStart => self.info_box("Vector Help", HELP_TEXT),
            Cmd::WelcomeScreen | Cmd::ShowHome => {
                self.finish_tool();
                self.active = None;
            }
            Cmd::VideoTutorials => self.dockers.show(DockerId::Learn),
            Cmd::WhatsNew => self.info_box("What's New?", &format!("Vector {}\n\n• Native Windows vector editor written in Rust\n• Vector drawing format (.vct)\n• PDF, SVG and bitmap export\n• PowerTRACE bitmap tracing\n• Scripting with recording\n• AI Generate", vector_core::APP_VERSION)),
            Cmd::Updates => self.info_box("Updates", &format!("You are running Vector {}.", vector_core::APP_VERSION)),
            Cmd::MessageSettings | Cmd::RefreshAccount | Cmd::AccountSettings => self.info_box("Account", "Vector does not require an account. All your work stays on this computer."),
            Cmd::Community | Cmd::Support | Cmd::Feedback => self.info_box("Vector Community", "Share ideas and report problems to your Vector administrator or team."),
            Cmd::About => self.dialogs.about = true,
            Cmd::Purchase => self.info_box("Purchase", "This copy of Vector is fully licensed."),

            // ------------------------------------------------------------------ Misc
            Cmd::Tool(t) => {
                if t == self.tool && t != ToolId::Pick {
                    return;
                }
                if !has_doc && !matches!(t, ToolId::Pick) {
                    self.set_tool(t);
                    return;
                }
                self.set_tool(t);
            }
            Cmd::Nudge(dx, dy) => {
                if self.tool == ToolId::Shape && self.doc().map(|d| !d.node_sel.is_empty()).unwrap_or(false) {
                    crate::tools::nudge_nodes(self, Vec2::new(dx, dy));
                } else {
                    self.transform_sel("Nudge", |_| Affine::translate((dx, dy)));
                }
            }
        }
    }

    pub fn open_options(&mut self, cat: OptionsPage, page: Option<&str>) {
        let o = &mut self.dialogs.options;
        o.open = true;
        o.category = Some(cat);
        o.page = page.map(|s| s.to_string());
    }
}

fn title_case(s: &str) -> String {
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

fn sentence_case(s: &str) -> String {
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

const HELP_TEXT: &str = "Getting started\n\n• File > New (Ctrl+N) creates a drawing; File > Open (Ctrl+O) opens .vct and .svg files.\n• Pick a tool from the toolbox on the left; the property bar above the drawing shows its options.\n• Click a color in the palette on the right to fill the selection; right-click to set the outline.\n• Hold Space to switch to the Pick tool temporarily; use the mouse wheel to zoom.\n• Window > Dockers opens panels such as Properties (Alt+Enter), Objects and Transform.\n• Tools > Options (Ctrl+J) holds all preferences.\n\nFiles are saved in the Vector format (.vct). Use File > Export (Ctrl+E) for PDF, SVG, PNG, JPG and more.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_conversions() {
        assert_eq!(title_case("hello big-world"), "Hello Big-World");
        assert_eq!(sentence_case("HELLO. WORLD"), "Hello. World");
    }
}
