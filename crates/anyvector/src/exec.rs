//! Command execution: every menu item, toolbar button, shortcut and script command ends here.

use crate::app::{Toolbar, AnyVectorApp};
use crate::commands::{Cmd, OptionsPage};
use crate::dialogs::bitmaps as bm;
use crate::dialogs::{NewDocState, PrintState, ScriptEditorState};
use crate::dockers::DockerId;
use crate::docview::DocView;
use crate::tools::{PickTarget, ToolId};
use kurbo::{Affine, Point, Rect, Vec2};
use std::path::{Path, PathBuf};
use vector_core::object::ObjKind;
use vector_core::ops::{self, AlignTo};
use vector_core::{Color, Document, Fill, ObjId, Object};

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
    let v = crate::dialogs::clipboard::system_has_content();
    *c = Some((std::time::Instant::now(), v));
    v
}

/// Apply a color from a palette/docker: fill (or outline) of the selection, or the default when
/// nothing is selected.
pub fn apply_color(app: &mut AnyVectorApp, color: Option<Color>, outline: bool) {
    if !app.has_selection() {
        if !outline {
            if let Some(c) = color {
                app.default_fill = c;
            }
        }
        app.flash(if outline { "Select objects to change their outline color." } else { "Default fill set for new objects." });
        return;
    }
    if let (false, Some(c)) = (outline, color) {
        if crate::propbar::mesh_palette_color(app, c) {
            return;
        }
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
pub fn new_symbol(app: &mut AnyVectorApp, name: &str) {
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
        doc.symbols.push(vector_core::document::SymbolDef { id: sid, name: n.clone(), objects: objs, link: None });
        let iid = doc.new_id();
        let mut inst = Object::new(iid, ObjKind::Symbol(sid), Affine::translate(bb.origin().to_vec2()), Default::default());
        inst.name = n;
        let (p, l) = first.map(|(p, l, _)| (p, l)).unwrap_or((0, 0));
        doc.pages[p].layers[l].objects.push(inst);
        *sel = vec![iid];
    });
}

/// Export one Export docker item (a page or an object) to its folder; returns the file written.
pub fn export_item(app: &mut AnyVectorApp, it: &crate::dockers::ExportItem) -> Result<PathBuf, String> {
    let d = app.doc().ok_or("No document")?;
    let (doc, page) = match (it.page, it.object) {
        (Some(pid), _) => (d.doc.clone(), d.doc.pages.iter().position(|p| p.id == pid).ok_or("The page no longer exists.")?),
        (_, Some(o)) => (selection_document(&d.doc, &[o]).ok_or("The object no longer exists.")?, 0),
        _ => return Err("Nothing to export".into()),
    };
    let fmt = crate::dockers::EXPORT_DOCKER_FORMATS[it.format.min(crate::dockers::EXPORT_DOCKER_FORMATS.len() - 1)].to_lowercase();
    let _ = std::fs::create_dir_all(&it.folder);
    let path = it.folder.join(format!("{}{}.{fmt}", it.name, it.suffix));
    match fmt.as_str() {
        "pdf" => {
            let mut opts = crate::dialogs::export::pdf_options(app);
            opts.pages = vec![page];
            vector_io::pdf::export(&doc, &path, &opts).map_err(|e| e.to_string())?;
        }
        "svg" => vector_io::svg_export::export(&doc, page, &path, &Default::default()).map_err(|e| e.to_string())?,
        _ => {
            let bg = if matches!(fmt.as_str(), "png" | "webp" | "tif" | "gif" | "jxl") { None } else { Some(Color::WHITE) };
            let opts = vector_io::image_io::RasterOptions { dpi: it.dpi, antialias: true, background: bg, jpeg_quality: 85, area: None, size: None };
            vector_io::image_io::export_raster(&doc, page, &path, &opts).map_err(|e| e.to_string())?;
        }
    }
    Ok(path)
}

/// Send a file with the default mail program (Simple MAPI); false when there is none.
#[cfg(windows)]
fn mapi_send(path: &Path, subject: &str) -> bool {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::Win32::System::Mapi::{MapiFileDescW, MapiMessageW, LPMAPISENDMAILW, MAPI_DIALOG, MAPI_LOGON_UI};
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    unsafe {
        let Ok(lib) = LoadLibraryW(PCWSTR(wide("MAPI32.DLL").as_ptr())) else { return false };
        let Some(f) = GetProcAddress(lib, windows::core::s!("MAPISendMailW")) else { return false };
        let send: LPMAPISENDMAILW = std::mem::transmute(f);
        let Some(send) = send else { return false };
        let mut file_path = wide(&path.display().to_string());
        let mut file_name = wide(&path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
        let mut subj = wide(subject);
        let mut fd = MapiFileDescW { nPosition: u32::MAX, lpszPathName: PWSTR(file_path.as_mut_ptr()), lpszFileName: PWSTR(file_name.as_mut_ptr()), ..Default::default() };
        let msg = MapiMessageW { lpszSubject: PWSTR(subj.as_mut_ptr()), nFileCount: 1, lpFiles: &mut fd, ..Default::default() };
        let owner = windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow();
        let r = send(0, owner.0 as usize, &msg, MAPI_DIALOG | MAPI_LOGON_UI, 0);
        // 0 = sent, 1 = the user cancelled
        r == 0 || r == 1
    }
}

#[cfg(not(windows))]
fn mapi_send(_path: &Path, _subject: &str) -> bool {
    false
}

/// PowerShell for File ▸ Acquire Image through Windows Image Acquisition: select a device
/// (`select` = true) or scan with the chosen device's dialog and save a PNG to `out`.
fn wia_script(select: bool, device: Option<&str>, out: &Path) -> String {
    let png = "{B96B3CAF-0728-11D3-9D7B-0000F81EF32E}";
    if select {
        return "$ErrorActionPreference='Stop'; $d = New-Object -ComObject WIA.CommonDialog; $dev = $d.ShowSelectDevice(0, $true, $false); if ($dev) { Write-Output $dev.DeviceID }".into();
    }
    let out = out.display().to_string().replace('\'', "''");
    let connect = match device {
        Some(id) => format!(
            "$m = New-Object -ComObject WIA.DeviceManager; $dev = $null; foreach ($i in $m.DeviceInfos) {{ if ($i.DeviceID -eq '{}') {{ $dev = $i.Connect() }} }}; ",
            id.replace('\'', "''")
        ),
        None => "$dev = $null; ".into(),
    };
    format!(
        "$ErrorActionPreference='Stop'; $d = New-Object -ComObject WIA.CommonDialog; {connect}\
         if ($dev) {{ $items = $d.ShowSelectItems($dev, 0, 0, $false, $true, $false); if ($items -and $items.Count -gt 0) {{ $img = $d.ShowTransfer($items.Item(1), '{png}', $false) }} }} \
         else {{ $img = $d.ShowAcquireImage(0, 0, 0, '{png}', $false, $true, $false) }}; \
         if ($img) {{ if ($img.FormatID -ne '{png}') {{ $p = New-Object -ComObject WIA.ImageProcess; $p.Filters.Add($p.FilterInfos.Item('Convert').FilterID); $p.Filters.Item(1).Properties.Item('FormatID').Value = '{png}'; $img = $p.Apply($img) }}; \
         if (Test-Path '{out}') {{ Remove-Item '{out}' }}; $img.SaveFile('{out}'); Write-Output 'OK' }}"
    )
}

/// Run a hidden PowerShell command; returns its output or the error text.
fn powershell(script: &str) -> Result<String, String> {
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// A readable message for a WIA failure.
fn wia_error(e: &str) -> String {
    if e.contains("0x80210015") || e.to_lowercase().contains("no wia device") || e.contains("-2145320939") {
        "No scanner or camera was found. Connect a WIA device and try again.".into()
    } else {
        format!("The image could not be acquired:\n{}", e.lines().next().unwrap_or(e))
    }
}

/// Add pasted objects centered at `at` (else in the middle of the view).
fn paste_placed(d: &mut crate::docview::DocView, mut objs: Vec<Object>, at: Option<Point>) {
    let c = at.unwrap_or_else(|| crate::dialogs::clipboard::paste_center(d));
    let bb = vector_core::object::union_bounds(objs.iter().map(|o| o.bounds()));
    for o in objs.iter_mut() {
        o.apply_transform(Affine::translate(c - bb.center()));
    }
    d.add_objects("Paste", objs);
}

fn rotate_about(c: Point, deg: f64) -> Affine {
    Affine::translate(c.to_vec2()) * Affine::rotate(-deg.to_radians()) * Affine::translate(-c.to_vec2())
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


fn reveal_in_explorer(path: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    }
}

impl AnyVectorApp {
    fn sel(&self) -> Vec<ObjId> {
        self.doc().map(|d| d.selection.clone()).unwrap_or_default()
    }

    // -------------------------------------------------------------------------------- documents

    pub(crate) fn push_doc(&mut self, dv: DocView) {
        self.finish_tool();
        self.docs.push(dv);
        self.active = Some(self.docs.len() - 1);
    }

    /// Window ▸ New Window: another window on the active document. Its changes show in every
    /// window of the document.
    fn new_window(&mut self) {
        let Some(a) = self.active.filter(|a| *a < self.docs.len()) else { return };
        let src = &self.docs[a];
        let mut dv = DocView::new(src.doc.clone(), src.title.clone(), self.settings.undo_levels);
        dv.history = src.history.clone();
        dv.path = src.path.clone();
        dv.group = src.group;
        dv.page = src.page;
        dv.view_mode = src.view_mode;
        let g = src.group;
        if self.docs[a].window_no == 0 {
            self.docs[a].window_no = 1;
        }
        dv.window_no = self.docs.iter().filter(|d| d.group == g).map(|d| d.window_no).max().unwrap_or(1) + 1;
        // the new window comes after the last window of its document
        let at = self.docs.iter().rposition(|d| d.group == g).map(|p| p + 1).unwrap_or(self.docs.len());
        self.finish_tool();
        self.docs.insert(at, dv);
        self.active = Some(at);
    }

    /// Window ▸ Cascade, Tile Horizontally, Tile Vertically, Combine Windows: the windows take
    /// their new places and each one shows its whole page.
    fn arrange_windows(&mut self, layout: u8) {
        if self.doc_layout != layout {
            self.doc_layout = layout;
            for d in self.docs.iter_mut() {
                d.needs_fit = true;
            }
        }
    }

    /// Keeps the windows of one document showing the same drawing: the active window's
    /// document, history and file go to the others when they change.
    pub fn sync_windows(&mut self) {
        let Some(a) = self.active.filter(|a| *a < self.docs.len()) else { return };
        let g = self.docs[a].group;
        if self.docs.iter().filter(|d| d.group == g).count() < 2 {
            return;
        }
        let (rev, saved, path) = (self.docs[a].history.revision, self.docs[a].history.saved_revision, self.docs[a].path.clone());
        for i in 0..self.docs.len() {
            if i == a || self.docs[i].group != g {
                continue;
            }
            let b = &self.docs[i];
            if b.history.revision == rev && b.history.saved_revision == saved && b.path == path {
                continue;
            }
            let (doc, hist) = (self.docs[a].doc.clone(), self.docs[a].history.clone());
            let b = &mut self.docs[i];
            b.doc = doc;
            b.history = hist;
            b.path = path.clone();
            b.page = b.page.min(b.doc.pages.len().saturating_sub(1));
            let doc = &b.doc;
            b.selection.retain(|id| doc.find(*id).is_some());
            b.node_sel.clear();
            b.text_edit = None;
            b.cache.key = 0;
        }
    }

    pub fn new_default_document(&mut self) {
        self.untitled_counter += 1;
        let st = NewDocState::from_settings(self.untitled_counter, &self.settings);
        self.create_document(&st);
    }

    pub fn create_document(&mut self, st: &NewDocState) {
        let mut doc = Document::new(st.width, st.height);
        doc.settings.unit = st.unit;
        doc.settings.dpi = st.dpi;
        doc.settings.color = st.color.clone();
        doc.settings.multipage_view = st.multipage;
        doc.settings.bleed = if st.bleed_on { st.bleed } else { 0.0 };
        doc.settings.show_bleed = st.bleed_on;
        doc.info.author = self.settings.user_name.clone();
        doc.object_styles = self.settings.default_object_styles.clone();
        doc.color_styles = self.settings.default_color_styles.clone();
        doc.color_harmonies = self.settings.default_color_harmonies.clone();
        for k in 1..st.pages.max(1) {
            doc.add_page(k, st.width, st.height);
        }
        let name = st.name.trim();
        let stem = name.strip_suffix(".cdr").or_else(|| name.strip_suffix(".vct")).unwrap_or(name);
        let mut dv = DocView::new(doc, format!("{stem}.cdr"), self.settings.undo_levels);
        dv.history.mark_saved();
        self.push_doc(dv);
    }

    fn new_document_cmd(&mut self) {
        if self.settings.show_new_doc_dialog {
            self.untitled_counter += 1;
            // the dialog opens on the settings of the last new document (as CorelDRAW does);
            // before there is one, on the active document's page size
            let mut st = NewDocState::from_settings(self.untitled_counter, &self.settings);
            if self.settings.last_new_doc.is_none() {
                if let Some(d) = self.doc() {
                    st.width = d.page_ref().width;
                    st.height = d.page_ref().height;
                    st.unit = d.doc.settings.unit;
                }
            }
            self.dialogs.new_doc = Some(st);
        } else {
            self.new_default_document();
        }
    }

    pub(crate) fn open_filters(d: rfd::FileDialog, import: bool) -> rfd::FileDialog {
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
        if vector_io::extension(p) == "pdf" {
            return crate::dialogs::pdf_import::start(self, p, true, None);
        }
        let r = vector_io::open_with_notes(p);
        self.finish_open(p, r)
    }

    /// Show a drawing that was read from `p` in a new tab (or say why it couldn't be read).
    pub(crate) fn finish_open(&mut self, p: &Path, r: vector_io::Result<(vector_core::Document, Vec<String>)>) -> bool {
        match r {
            Ok((mut doc, notes)) => {
                doc.ensure_next_id();
                let title = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let mut dv = DocView::new(doc, title, self.settings.undo_levels);
                if vector_io::is_drawing(p) {
                    dv.path = Some(p.to_path_buf());
                } else {
                    // imported formats open as untitled drawings
                    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    dv.title = format!("{stem}.cdr");
                }
                dv.history.mark_saved();
                self.settings.add_recent(p);
                self.settings.last_dir = p.parent().map(|x| x.to_path_buf());
                self.push_doc(dv);
                self.show_import_notes("Open", p, &notes);
                true
            }
            Err(e) => {
                self.dialogs.message_box = Some(("Open".into(), format!("Could not open {}:\n{e}", p.display())));
                false
            }
        }
    }

    /// Tell the user what a drawing that just opened or was imported differs in from the original
    /// (`notes` from the importer, in English, translated here line by line).
    pub(crate) fn show_import_notes(&mut self, title: &str, p: &Path, notes: &[String]) {
        if notes.is_empty() {
            return;
        }
        let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut text = crate::i18n::t(&format!("{} came in with these differences from the original:", name));
        for n in notes {
            text.push_str("\n• ");
            text.push_str(&crate::i18n::t(n));
        }
        self.dialogs.message_box = Some((title.into(), text));
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
        if vector_io::extension(p) == "pdf" {
            crate::dialogs::pdf_import::start(self, p, false, at);
            return;
        }
        if !self.has_doc() {
            self.new_default_document();
        }
        let Some(d) = self.doc_mut() else { return };
        match vector_io::import_with_notes(&mut d.doc, p) {
            Ok((objs, notes)) => self.place_import(p, at, objs, notes),
            Err(e) => self.dialogs.message_box = Some(("Import".into(), format!("Could not import {}:\n{e}", p.display()))),
        }
    }

    /// Put imported objects on the active page, centered (or at a point), and select them.
    pub(crate) fn place_import(&mut self, p: &Path, at: Option<Point>, mut objs: Vec<vector_core::Object>, notes: Vec<String>) {
        let warn_large = self.settings.warnings.iter().any(|(t, on)| *on && t.contains("large bitmap"));
        let Some(d) = self.doc_mut() else { return };
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
        self.show_import_notes("Import", p, &notes);
    }

    /// The `.cdr` save options for a document from the current settings.
    fn cdr_options(&self, doc: &vector_core::Document, embed_fonts: bool, embed_profiles: bool, version: usize) -> vector_io::cdr::SaveOptions {
        let s = &self.settings.save;
        let fonts = if embed_fonts {
            vector_core::text::font_files(&doc.used_fonts()).into_iter().filter(|f| f.embeddable).map(|f| vector_io::cdr::EmbeddedFile { name: f.name, data: f.data }).collect()
        } else {
            vec![]
        };
        vector_io::cdr::SaveOptions {
            version: crate::settings::FILE_VERSIONS.get(version).unwrap_or(&crate::settings::FILE_VERSIONS[0]).to_string(),
            fonts,
            color_profiles: if embed_profiles { crate::color_profiles::profile_files(&doc.settings.color) } else { vec![] },
            object_compression: s.object_compression,
            bitmap_compression: s.bitmap_compression,
            optimize_for_size: s.optimize_for_size,
            thumbnail: s.save_thumbnail,
            presentation_attributes: s.save_presentation_attributes,
        }
    }

    fn default_version_index(&self) -> usize {
        crate::settings::FILE_VERSIONS.iter().position(|v| *v == self.settings.default_version).unwrap_or(0)
    }

    fn advanced_from_settings(&self) -> crate::save_dialog::Advanced {
        let s = &self.settings.save;
        crate::save_dialog::Advanced {
            optimize_for_size: s.optimize_for_size,
            presentation_attributes: s.save_presentation_attributes,
            bitmap_compression: s.bitmap_compression,
            object_compression: s.object_compression,
            thumbnail: s.save_thumbnail,
        }
    }

    /// Save document `i`. Returns false if cancelled or failed.
    pub fn save_doc(&mut self, i: usize, save_as: bool) -> bool {
        if i >= self.docs.len() {
            return false;
        }
        let mut version = self.default_version_index();
        let (mut embed_fonts, mut embed_profiles) = (self.settings.embed_fonts, self.settings.save.embed_color_profiles);
        let path = match (&self.docs[i].path, save_as) {
            (Some(p), false) => p.clone(),
            _ => {
                let stem = self.docs[i].stem();
                let dir = self.docs[i].path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf()).or_else(|| self.settings.last_dir.clone()).or_else(|| Some(crate::settings::documents_dir()));
                let types: Vec<(String, String)> = vector_io::SAVE_TYPES.iter().map(|t| (t.name.to_string(), t.extensions[0].to_string())).collect();
                // a .vct document stays .vct unless the user picks CDR
                let type_index = match self.docs[i].path.as_ref().map(|p| vector_io::extension(p)) {
                    Some(e) if e == "vct" => 1,
                    _ => 0,
                };
                let advanced = self.advanced_from_settings();
                let d = &self.docs[i].doc;
                let fonts_size: u64 = vector_core::text::font_files(&d.used_fonts()).iter().filter(|f| f.embeddable).map(|f| f.data.len() as u64).sum();
                let profiles_size: u64 = crate::color_profiles::profile_files(&d.settings.color).iter().map(|f| f.data.len() as u64).sum();
                let req = crate::save_dialog::SaveRequest {
                    dir,
                    file_name: format!("{stem}.{}", types[type_index].1),
                    types: types.clone(),
                    type_index,
                    title: d.info.title.clone(),
                    subject: d.info.subject.clone(),
                    rating: d.info.rating,
                    embed_profiles,
                    profiles_size,
                    profiles_summary: d.settings.color.summary(),
                    embed_fonts,
                    fonts_size,
                    versions: crate::settings::FILE_VERSIONS.iter().map(|v| v.to_string()).collect(),
                    version,
                    advanced,
                };
                let res = if cfg!(windows) {
                    crate::save_dialog::show(req)
                } else {
                    rfd::FileDialog::new().set_title("Save Drawing").add_filter(types[0].0.as_str(), &["cdr"]).set_file_name(format!("{stem}.cdr")).save_file().map(|p| crate::save_dialog::SaveResult {
                        path: p,
                        type_index: 0,
                        title: d.info.title.clone(),
                        subject: d.info.subject.clone(),
                        rating: d.info.rating,
                        embed_profiles,
                        embed_fonts,
                        version,
                        advanced,
                    })
                };
                let Some(r) = res else { return false };
                // remember the choices for next time
                embed_fonts = r.embed_fonts;
                embed_profiles = r.embed_profiles;
                version = r.version;
                self.settings.embed_fonts = r.embed_fonts;
                self.settings.save.embed_color_profiles = r.embed_profiles;
                self.settings.save.optimize_for_size = r.advanced.optimize_for_size;
                self.settings.save.save_presentation_attributes = r.advanced.presentation_attributes;
                self.settings.save.bitmap_compression = r.advanced.bitmap_compression;
                self.settings.save.object_compression = r.advanced.object_compression;
                self.settings.save.save_thumbnail = r.advanced.thumbnail;
                let info = &mut self.docs[i].doc.info;
                info.title = r.title;
                info.subject = r.subject;
                info.rating = r.rating;
                let want = types.get(r.type_index).map(|t| t.1.clone()).unwrap_or_else(|| "cdr".into());
                if vector_io::extension(&r.path) == want {
                    r.path
                } else {
                    let mut s = r.path.into_os_string();
                    s.push(format!(".{want}"));
                    PathBuf::from(s)
                }
            }
        };
        if self.settings.backup_on_save && path.exists() {
            let dir = self.settings.backup_dir.clone().unwrap_or_else(|| path.parent().map(|p| p.to_path_buf()).unwrap_or_default());
            let name = format!("Backup_of_{}", path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::copy(&path, dir.join(name));
        }
        let result = if vector_io::extension(&path) == "vct" {
            vector_io::native::save(&self.docs[i].doc, &path)
        } else {
            let opts = self.cdr_options(&self.docs[i].doc, embed_fonts, embed_profiles, version);
            vector_io::cdr::save(&self.docs[i].doc, &path, &opts)
        };
        let d = &mut self.docs[i];
        match result {
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
        // a pattern document puts its changes into the pattern
        if self.docs[i].pattern.is_some() {
            crate::dialogs::object::apply_pattern_doc(self, i);
            self.close_doc_now(i);
            return;
        }
        // another window still shows the document: this one closes without asking
        let g = self.docs[i].group;
        let shared = self.docs.iter().enumerate().any(|(k, d)| k != i && d.group == g);
        let mut i = i;
        if !shared {
            // the patterns of the document being edited go into it first
            while let Some(k) = self.docs.iter().position(|d| d.pattern.as_ref().is_some_and(|l| l.group == g)) {
                crate::dialogs::object::apply_pattern_doc(self, k);
                self.close_doc_now(k);
                if k < i {
                    i -= 1;
                }
            }
        }
        if self.docs[i].history.is_modified() && !shared {
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
        let gone = self.docs.remove(i);
        // the windows left of the document are numbered again
        let mut k = 0;
        let left = self.docs.iter().filter(|d| d.group == gone.group).count();
        for d in self.docs.iter_mut().filter(|d| d.group == gone.group) {
            k += 1;
            d.window_no = if left > 1 { k } else { 0 };
        }
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
                if self.docs[i].pattern.is_some() {
                    crate::dialogs::object::apply_pattern_doc(self, i);
                    self.close_doc_now(i);
                    continue;
                }
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

    /// Print through the Windows shell: the pages are written to a temporary PDF which is sent
    /// to the default printer with the "print" verb.

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
        // system clipboard: SVG, EMF+ metafile, PNG and DIB together (Office pastes vectors)
        if crate::dialogs::clipboard::system_enabled() {
            let svg = vector_io::svg_export::objects_to_svg(&d.doc, &objs);
            let emf = vector_io::emf::objects_to_emf(&d.doc, &objs, &Default::default());
            let png = vector_render::render_objects_image(&d.doc, &objs, 1024, None);
            let rgba = png.as_ref().map(vector_render::pixmap_to_rgba);
            let bitmap = png.as_ref().zip(rgba.as_deref()).map(|(pm, px)| (pm.width(), pm.height(), px));
            crate::dialogs::clipboard::set_system(&crate::dialogs::clipboard::copy_formats(&svg, emf, bitmap));
        }
        self.clip = crate::app::Clip { objects: objs, images, symbols };
        true
    }

    /// Paste the clipboard: `kind` = the format (Paste Special) or the best one; `at` = center
    /// the pasted objects there (Paste in View).
    pub fn paste_as(&mut self, kind: Option<crate::dialogs::clipboard::PasteKind>, at: Option<Point>) {
        use crate::dialogs::clipboard::PasteKind;
        let want = |k: PasteKind| kind.is_none() || kind == Some(k);
        if !self.has_doc() {
            self.new_default_document();
        }
        // what is on the system clipboard?
        let own = crate::dialogs::clipboard::system_own();
        let mut text = if matches!(kind, Some(PasteKind::Bitmap | PasteKind::Metafile)) { None } else { crate::dialogs::clipboard::system_text() };
        let ours = own.is_some() || text.as_deref().map(|t| t.contains("data-vector-clip")).unwrap_or(false);
        let image = if ours || kind.is_some_and(|k| k != PasteKind::Bitmap) { None } else { crate::dialogs::clipboard::system_image() };
        // a browser's "Copy image" may add the image URL as text: paste the bitmap
        if image.is_some() && kind.is_none() && text.as_deref().is_some_and(crate::dialogs::clipboard::text_is_image_link) {
            text = None;
        }
        // vectors from another program: an SVG picture or a metafile
        let foreign_svg = if ours || !want(PasteKind::Svg) || kind == Some(PasteKind::Native) { None } else { crate::dialogs::clipboard::system_svg() };
        let foreign_emf = !ours && want(PasteKind::Metafile) && crate::dialogs::clipboard::system_has_emf();
        let outside = text.is_some() || image.is_some() || foreign_svg.is_some() || foreign_emf;
        if (kind == Some(PasteKind::Native) || (kind.is_none() && (ours || !outside))) && !self.clip.objects.is_empty() {
            let clip = self.clip.clone();
            let d = self.doc_mut().unwrap();
            let mut tmp = Document::new(1.0, 1.0);
            tmp.images = clip.images.clone();
            tmp.symbols = clip.symbols.iter().filter(|s| !d.doc.symbols.iter().any(|x| x.id == s.id)).cloned().collect();
            // keep symbol references to existing definitions
            tmp.pages[0].layers[0].objects = clip.objects.clone();
            let mut objs = vector_io::merge_document(&mut d.doc, tmp);
            if let Some(c) = at {
                let bb = vector_core::object::union_bounds(objs.iter().map(|o| o.bounds()));
                for o in objs.iter_mut() {
                    o.apply_transform(Affine::translate(c - bb.center()));
                }
            }
            d.add_objects("Paste", objs);
            return;
        }
        // an SVG picture: ours (Paste Special, or copied in another AnyVector) or another program's
        let svg = if ours && want(PasteKind::Svg) && kind != Some(PasteKind::Native) { own } else { foreign_svg };
        if let Some(svg) = svg {
            let d = self.doc_mut().unwrap();
            if let Ok(objs) = vector_io::svg_import::objects_from_svg(&mut d.doc, svg.trim().as_bytes()) {
                paste_placed(d, objs, at);
                return;
            }
        }
        if let Some(t) = text {
            let t = t.trim().to_string();
            if (t.starts_with("<svg") || t.starts_with("<?xml")) && want(PasteKind::Svg) {
                let d = self.doc_mut().unwrap();
                if let Ok(objs) = vector_io::svg_import::objects_from_svg(&mut d.doc, t.as_bytes()) {
                    paste_placed(d, objs, at);
                    return;
                }
            }
            if !t.is_empty() && want(PasteKind::Text) {
                crate::tools::insert_text_at(self, &t, None, at);
                return;
            }
        }
        // a metafile (Office, other drawing programs): its GDI records as curves
        if foreign_emf {
            if let Some(bytes) = crate::dialogs::clipboard::system_emf() {
                let d = self.doc_mut().unwrap();
                if let Ok(objs) = vector_io::emf_import::objects_from_emf(&mut d.doc, &bytes) {
                    paste_placed(d, objs, at);
                    return;
                }
            }
        }
        if let Some(img) = image {
            let d = self.doc_mut().unwrap();
            let data = vector_core::document::ImageData {
                width: img.width,
                height: img.height,
                rgba: std::sync::Arc::new(img.rgba),
                encoded: std::sync::Arc::new(img.png),
                format: "png".into(),
                name: "Pasted image".into(),
            };
            // screen bitmaps come in at 96 dpi, as File ▸ Import sizes them
            let dpi = 96.0;
            let mut o = vector_io::image_io::image_object(&mut d.doc, data, dpi);
            let c = at.unwrap_or_else(|| crate::dialogs::clipboard::paste_center(d));
            let b = o.bounds();
            o.apply_transform(Affine::translate(c - b.center()));
            d.add_objects("Paste", vec![o]);
        }
    }

    // -------------------------------------------------------------------------------- bitmaps

    /// Effects ▸ Flatten Effects: each selected object with bitmap effects becomes a bitmap of
    /// how it looks (a bitmap keeps its resolution, other objects get 300 dpi).
    fn flatten_effects(&mut self) {
        let Some(d) = self.doc() else { return };
        let ids: Vec<ObjId> = d.selection.iter().copied().filter(|id| d.doc.find(*id).is_some_and(|o| !o.effects.bitmap.is_empty())).collect();
        if ids.is_empty() {
            self.flash("The selection has no bitmap effects to flatten.");
            return;
        }
        let mut made = vec![];
        for id in &ids {
            let Some(o) = d.doc.find(*id) else { continue };
            let dpi = match &o.kind {
                ObjKind::Image(io) => d
                    .doc
                    .images
                    .get(&io.image)
                    .filter(|im| im.width > 0 && io.w > 0.0)
                    .map(|im| {
                        let c = o.transform.as_coeffs();
                        let pts = io.w * c[0].hypot(c[1]) / (im.width as f64 * (io.crop[2] - io.crop[0]).max(1e-6));
                        (72.0 / pts.max(1e-6)).clamp(30.0, 1200.0)
                    })
                    .unwrap_or(300.0),
                _ => 300.0,
            };
            let (Some(sd), Some(bb)) = (selection_document(&d.doc, &[*id]), d.doc.selection_bounds(&[*id])) else { continue };
            let Some(pm) = vector_render::render_page_image(&sd, 0, dpi, None, None, true) else { continue };
            let rgba = vector_render::pixmap_to_rgba(&pm);
            let name = if o.name.is_empty() { "Bitmap".to_string() } else { o.name.clone() };
            let data = vector_core::document::ImageData { width: pm.width(), height: pm.height(), rgba: std::sync::Arc::new(rgba), encoded: Default::default(), format: "png".into(), name };
            made.push((*id, data, dpi, bb));
        }
        let Some(d) = self.doc_mut() else { return };
        d.modify("Flatten Effects", |doc, sel| {
            let mut out = vec![];
            for (id, data, dpi, bb) in made.iter().cloned() {
                let loc = doc.locate(id);
                doc.remove(id);
                let mut o = vector_io::image_io::image_object(doc, data, dpi);
                o.apply_transform(Affine::translate(bb.origin().to_vec2()));
                out.push(o.id);
                let (p, l, i) = loc.unwrap_or((0, 0, usize::MAX));
                let list = &mut doc.pages[p].layers[l].objects;
                list.insert(i.min(list.len()), o);
            }
            *sel = out;
        });
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

    /// Change the selected tables; `f` gets the selected block of cells, if any.
    pub(crate) fn table_op(&mut self, f: impl Fn(&mut vector_core::object::TableObj, Option<vector_core::object::CellRange>)) {
        let (a, b) = (self.ts.table_cell, self.ts.table_cell_to);
        self.modify_sel("Edit Table", |doc, sel| {
            for id in sel.iter() {
                if let Some(o) = doc.find_mut(*id) {
                    if let ObjKind::Table(t) = &mut o.kind {
                        let g = crate::tools::range_of(t, a, b);
                        f(t, g);
                    }
                }
            }
        });
    }

    /// The selected table and its selected block of cells.
    fn table_sel(&self) -> Option<(ObjId, vector_core::object::TableObj, Option<vector_core::object::CellRange>)> {
        let d = self.doc()?;
        let [id] = d.selection.as_slice() else { return None };
        match &d.doc.find(*id)?.kind {
            ObjKind::Table(t) => Some((*id, t.clone(), self.ts.table_range(t))),
            _ => None,
        }
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
            crate::dialogs::palette::create_palette(self, colors);
        }
    }

    fn info_box(&mut self, title: &str, text: &str) {
        self.dialogs.message_box = Some((title.to_string(), text.to_string()));
    }

    fn save_copy_to(&mut self, dir: PathBuf, what: &str) {
        let Some(d) = self.doc() else { return };
        let name = format!("{}.cdr", d.stem());
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join(name);
        let opts = self.cdr_options(&d.doc, self.settings.embed_fonts, self.settings.save.embed_color_profiles, self.default_version_index());
        match vector_io::cdr::save(&d.doc, &p, &opts) {
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
                // Create a New Document, opened on its Templates tab
                self.untitled_counter += 1;
                let mut st = NewDocState::from_settings(self.untitled_counter, &self.settings);
                st.tab = 1;
                self.dialogs.new_doc = Some(st);
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
            Cmd::NewStyleSetFrom => {
                crate::dockers::ostyles::new_style(self, 0);
                self.dockers.show(crate::dockers::DockerId::ObjectStyles);
            }
            Cmd::NewStyleFrom(k) => {
                crate::dockers::ostyles::new_style(self, k);
                self.dockers.show(crate::dockers::DockerId::ObjectStyles);
            }
            Cmd::ApplyObjectStyle(i) => crate::dockers::ostyles::apply_to_selection(self, i),
            Cmd::EditObjectStyle(i) => crate::dockers::ostyles::edit_style(self, i),
            Cmd::ApplyToStyle => crate::dockers::ostyles::apply_to_style(self),
            Cmd::RevertToStyle => crate::dockers::ostyles::revert_to_style(self),
            Cmd::BreakStyleLink => crate::dockers::ostyles::break_link(self),
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
                    let stem = d.stem();
                    if let Some(p) = rfd::FileDialog::new().set_title("Save Drawing").add_filter("CDRT/CDT - CorelDRAW Template (*.cdrt;*.cdt)", &["cdrt", "cdt"]).set_directory(&dir).set_file_name(format!("{stem}.cdrt")).save_file() {
                        let opts = self.cdr_options(&d.doc, self.settings.embed_fonts, self.settings.save.embed_color_profiles, self.default_version_index());
                        match vector_io::cdr::save(&d.doc, &p, &opts) {
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
            Cmd::SelectWia => {
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let r = powershell(&wia_script(true, None, Path::new(""))).map_err(|e| wia_error(&e)).map(|id| if id.is_empty() { String::new() } else { format!("\u{1}wia:{id}") });
                    let _ = tx.send(r);
                });
                self.background.push(rx);
            }
            Cmd::SelectTwain => {
                let dsm = ["twaindsm.dll", "twain_32.dll"].iter().any(|n| {
                    let win = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
                    [Path::new(&win).join(n), Path::new(&win).join("System32").join(n), Path::new(&win).join("SysWOW64").join(n)].iter().any(|p| p.exists())
                });
                let msg = if dsm {
                    "TWAIN sources are not supported. Choose Select WIA Source: scanners and cameras that come with a WIA driver are acquired through Windows Image Acquisition."
                } else {
                    "No TWAIN data source manager is installed. Choose Select WIA Source to use a scanner or camera through Windows Image Acquisition."
                };
                self.info_box("Select TWAIN Source", msg);
            }
            Cmd::Acquire => {
                let device = self.settings.wia_device.clone();
                let out = std::env::temp_dir().join(format!("vector-acquire-{}.png", std::process::id()));
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let r = powershell(&wia_script(false, device.as_deref(), &out)).map_err(|e| wia_error(&e)).map(|o| if o.contains("OK") && out.exists() { format!("\u{1}import:{}", out.display()) } else { String::new() });
                    let _ = tx.send(r);
                });
                self.background.push(rx);
                self.flash("Waiting for the scanner...");
            }
            Cmd::Import => self.import_dialog(),
            Cmd::Export => {
                if has_doc {
                    self.dialogs.export = Some(crate::dialogs::ExportState::new(self));
                }
            }
            Cmd::ExportForWeb | Cmd::ExportForOffice | Cmd::ExportForWordPress => {
                if let Some(d) = self.doc() {
                    use crate::dialogs::export::{BitmapMode, BitmapState};
                    let (mode, ext) = match cmd {
                        Cmd::ExportForWeb => (BitmapMode::Web, "jpg"),
                        Cmd::ExportForOffice => (BitmapMode::Office, "png"),
                        _ => (BitmapMode::WordPress, "jpg"),
                    };
                    let sel = self.has_selection();
                    let pages = vec![d.page];
                    self.dialogs.bitmap = Some(BitmapState::new(self, mode, None, ext, pages, sel));
                }
            }
            Cmd::ExportDockerOpen => self.dockers.show(DockerId::Export),
            Cmd::ExportDockerAddSelection | Cmd::ExportDockerAddPage | Cmd::ExportDockerAddAllPages => {
                if let Some(d) = self.doc() {
                    let folder = self.settings.last_export_dir.clone().unwrap_or_else(crate::settings::documents_dir);
                    let format = self.dockers.ui.export_format;
                    let stem = d.stem();
                    let mut items = vec![];
                    let item = |page: Option<u64>, object: Option<ObjId>, name: String| crate::dockers::ExportItem { page, object, name, suffix: String::new(), format, folder: folder.clone(), selected: false, dpi: 300.0 };
                    match cmd {
                        Cmd::ExportDockerAddSelection => {
                            for id in &d.selection {
                                if let Some(o) = d.doc.find(*id) {
                                    let name = if o.name.is_empty() { format!("{stem} {}", o.kind.type_name()) } else { o.name.clone() };
                                    items.push(item(None, Some(*id), name));
                                }
                            }
                        }
                        Cmd::ExportDockerAddPage => items.push(item(Some(d.page_ref().id), None, format!("{stem} {}", d.doc.page_name(d.page)))),
                        _ => {
                            for (i, p) in d.doc.pages.iter().enumerate() {
                                items.push(item(Some(p.id), None, format!("{stem} {}", d.doc.page_name(i))));
                            }
                        }
                    }
                    let list = &mut self.dockers.ui.export_items;
                    for it in items {
                        let dup = list.iter().any(|x| x.page == it.page && x.object == it.object && it.page.is_some());
                        if !dup {
                            list.push(it);
                        }
                    }
                    // pages first, then objects
                    list.sort_by_key(|x| x.page.is_none());
                    self.dockers.show(DockerId::Export);
                }
            }
            Cmd::SendToMail => {
                if let Some(d) = self.doc() {
                    let stem = d.stem();
                    let p = match d.path.clone().filter(|p| p.exists() && !d.history.is_modified()) {
                        Some(p) => p,
                        None => {
                            let p = std::env::temp_dir().join(format!("{stem}.cdr"));
                            let opts = self.cdr_options(&d.doc, self.settings.embed_fonts, self.settings.save.embed_color_profiles, self.default_version_index());
                            if let Err(e) = vector_io::cdr::save(&d.doc, &p, &opts) {
                                self.info_box("Send To Mail", &format!("Could not save the drawing: {e}"));
                                return;
                            }
                            p
                        }
                    };
                    if !mapi_send(&p, &stem) {
                        reveal_in_explorer(&p);
                        let _ = std::process::Command::new("cmd").args(["/C", "start", "", &format!("mailto:?subject={}", stem.replace(' ', "%20"))]).spawn();
                        self.flash("No mail program accepted the attachment; the drawing is ready to attach to your message.");
                    }
                }
            }
            Cmd::SendToDesktop => {
                if let Some(dir) = dirs::desktop_dir() {
                    self.save_copy_to(dir, "the Desktop");
                }
            }
            Cmd::SendToDocuments => self.save_copy_to(crate::settings::documents_dir(), "Documents"),
            Cmd::PublishPdf => {
                if has_doc {
                    self.dialogs.pdf = Some(crate::dialogs::export::PdfState::new(self, true));
                }
            }
            Cmd::PrintMergeCreate | Cmd::PrintMergeEdit => {
                if let Some(d) = self.doc() {
                    self.dialogs.merge = Some(crate::dialogs::merge::MergeState::new(d.doc.print_merge.clone()));
                }
            }
            Cmd::PrintMergePerform => {
                if self.doc().map(|d| merged_document(&d.doc).is_some()).unwrap_or(false) {
                    let mut st = PrintState::new(self, false);
                    st.merge = true;
                    st.settings.range = crate::printing::Range::CurrentDocument;
                    self.dialogs.print = Some(st);
                } else {
                    self.info_box("Print Merge", "Select at least one record in the print merge data.");
                }
            }
            Cmd::PrintMergeNewDoc => {
                let merged = self.doc().and_then(|d| merged_document(&d.doc).map(|m| (m, d.stem())));
                match merged {
                    Some((doc, stem)) => {
                        self.untitled_counter += 1;
                        let title = format!("{stem} (merged).cdr");
                        let dv = crate::docview::DocView::new(doc, title, self.settings.undo_levels);
                        self.docs.push(dv);
                        self.active = Some(self.docs.len() - 1);
                    }
                    None => self.info_box("Print Merge", "Select at least one record in the print merge data."),
                }
            }
            Cmd::Print => {
                if has_doc {
                    self.dialogs.print = Some(PrintState::new(self, false));
                }
            }
            Cmd::PrintPreview => {
                if has_doc {
                    self.dialogs.print = Some(PrintState::new(self, true));
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
            Cmd::Paste => self.paste_as(None, None),
            Cmd::PasteSpecial => {
                let (kinds, source) = crate::dialogs::clipboard::clipboard_kinds(self);
                self.dialogs.paste_special = Some(crate::dialogs::clipboard::PasteSpecialState { source, kinds, chosen: 0 });
            }
            Cmd::PasteInView => {
                let c = self.doc().map(|d| d.to_doc(d.canvas_rect.center()));
                self.paste_as(None, c);
            }
            Cmd::BringIntoFocus => {
                if let Some(d) = self.doc_mut() {
                    if !d.selection.is_empty() {
                        d.focus = d.selection.clone();
                        d.cache.key = 0;
                    }
                }
            }
            Cmd::ExitFocusMode if self.doc().is_some_and(|d| d.symbol_edit.is_some()) => crate::dialogs::object::finish_symbol(self),
            Cmd::ExitFocusMode if self.doc().is_some_and(|d| d.rollover_edit.is_some()) => crate::dialogs::object::finish_rollover(self),
            Cmd::ExitFocusMode if self.doc().is_some_and(|d| d.symmetry_edit.is_some()) => crate::dialogs::symmetry::finish_symmetry(self),
            Cmd::ExitFocusMode if self.doc().is_some_and(|d| d.persp_edit.is_some()) => crate::dialogs::symmetry::finish_perspective(self),
            Cmd::ExitFocusMode => {
                if let Some(d) = self.doc_mut() {
                    if !d.focus.is_empty() {
                        d.selection = std::mem::take(&mut d.focus);
                        d.cache.key = 0;
                    }
                }
            }
            Cmd::NewFromClipboard => {
                self.new_default_document();
                self.paste_as(None, None);
            }
            Cmd::Delete => {
                // a guideline clicked with the Pick tool (locked ones stay)
                if let Some(d) = self.doc_mut().filter(|d| d.selection.is_empty() && d.guide_sel.is_some_and(|(p, _)| p == d.page)) {
                    let (page, i) = d.guide_sel.take().unwrap_or_default();
                    if d.doc.pages.get(page).and_then(|p| p.guides.get(i)).is_some_and(|g| !g.locked) {
                        d.modify("Delete Guideline", |doc, _| {
                            doc.pages[page].guides.remove(i);
                        });
                    }
                } else if self.tool == ToolId::Shape && self.doc().map(|d| !d.node_sel.is_empty()).unwrap_or(false) {
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
                    self.dialogs.copy_props = Some(Default::default());
                }
            }
            Cmd::CopyEffectFrom(kind) | Cmd::CloneEffectFrom(kind) => {
                if self.has_selection() {
                    self.set_tool(ToolId::Pick);
                    self.ts.pick_target = Some(PickTarget::CopyEffect { kind, clone: matches!(cmd, Cmd::CloneEffectFrom(_)) });
                    self.flash("Click the object to copy the effect from.");
                }
            }
            Cmd::OrderInFrontOf | Cmd::OrderBehind => {
                if self.has_selection() {
                    self.set_tool(ToolId::Pick);
                    let in_front = cmd == Cmd::OrderInFrontOf;
                    self.ts.pick_target = Some(PickTarget::Order { in_front });
                    self.flash(if in_front { "Click the object to place the selection in front of." } else { "Click the object to place the selection behind." });
                }
            }
            Cmd::CopyPowerClipFrom => {
                if self.has_selection() {
                    self.set_tool(ToolId::Pick);
                    self.ts.pick_target = Some(PickTarget::CopyPowerClip);
                    self.flash("Click the PowerClip to copy the contents from.");
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
            Cmd::FullScreenPreview => {
                self.preview_selected = false;
                self.fullscreen_preview = has_doc && !self.fullscreen_preview;
            }
            Cmd::PreviewSelectedOnly => {
                if self.has_selection() {
                    self.preview_selected = true;
                    self.fullscreen_preview = true;
                }
            }
            Cmd::SnapToPixels | Cmd::SnapToBaseline | Cmd::ToggleMultipageView | Cmd::TogglePrintableArea | Cmd::ToggleBaselineGrid => {
                if let Some(d) = self.doc_mut() {
                    let s = &mut d.doc.settings;
                    match cmd {
                        Cmd::SnapToPixels => s.snap_to_pixels = !s.snap_to_pixels,
                        Cmd::SnapToBaseline => s.snap_to_baseline = !s.snap_to_baseline,
                        Cmd::ToggleMultipageView => s.multipage_view = !s.multipage_view,
                        Cmd::TogglePrintableArea => s.show_printable_area = !s.show_printable_area,
                        _ => s.show_baseline_grid = !s.show_baseline_grid,
                    }
                    d.cache.key = 0;
                }
            }
            Cmd::ToggleSimulateOverprints => {
                if let Some(d) = self.doc_mut() {
                    d.simulate_overprint = !d.simulate_overprint;
                    d.cache.key = 0;
                }
            }
            Cmd::ToggleProofColors => self.settings.proof_colors = !self.settings.proof_colors,
            Cmd::ToggleRasterizeEffects => self.settings.rasterize_complex_effects = !self.settings.rasterize_complex_effects,
            Cmd::ToggleAlignmentGuides => self.settings.live_guides = !self.settings.live_guides,
            Cmd::ToggleDynamicGuides => self.settings.dynamic_guides = !self.settings.dynamic_guides,
            Cmd::ToggleDocNavigator => self.settings.show_doc_navigator = !self.settings.show_doc_navigator,
            Cmd::ToggleScrollbars => self.settings.show_scrollbars = !self.settings.show_scrollbars,
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
                self.dialogs.insert_page = crate::dialogs::layout::InsertPageState::new(self, false);
            }
            Cmd::InsertPageAfter | Cmd::InsertPageBefore => {
                let before = cmd == Cmd::InsertPageBefore;
                if let Some(d) = self.doc_mut() {
                    let (w, h) = (d.page_ref().width, d.page_ref().height);
                    let at = if before { d.page } else { d.page + 1 };
                    d.modify("Insert Page", |doc, sel| {
                        doc.add_page(at, w, h);
                        sel.clear();
                    });
                    d.page = at;
                    d.needs_fit = true;
                }
            }
            Cmd::GridSetup => self.open_options(OptionsPage::Document, Some("Grid")),
            Cmd::CommandBarsSetup => self.open_options(OptionsPage::Customization, Some("Command Bars")),
            Cmd::CommandsSetup => self.open_options(OptionsPage::Customization, Some("Commands")),
            Cmd::RulerSetup => self.open_options(OptionsPage::Document, Some("Rulers")),
            Cmd::GuidelinesSetup => self.open_options(OptionsPage::Document, Some("Guidelines")),
            Cmd::DuplicatePage => {
                if let Some(d) = self.doc() {
                    self.dialogs.duplicate_page = Some(crate::dialogs::layout::DuplicatePageState { page: d.page, place: 1, contents: false });
                }
            }
            Cmd::RenamePage => {
                if let Some(d) = self.doc() {
                    self.dialogs.rename_page = Some((d.page, d.doc.page_name(d.page)));
                }
            }
            Cmd::DeletePage => {
                if let Some(d) = self.doc() {
                    if d.doc.pages.len() > 1 {
                        self.dialogs.delete_page = Some(crate::dialogs::layout::DeletePageState { from: d.page + 1, through: false, to: d.page + 1 });
                    }
                }
            }
            Cmd::DeletePageNow => {
                if let Some(d) = self.doc_mut() {
                    if d.doc.pages.len() > 1 {
                        let cur = d.page;
                        d.modify("Delete Page", |doc, sel| {
                            let m = doc.take_master_layers();
                            doc.pages.remove(cur);
                            doc.restore_master_layers(m);
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
            Cmd::BarOption(k) => crate::propbar::press(self, &format!("{k:08x}")),
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
            Cmd::PageLayout => self.open_options(OptionsPage::Document, Some("Layout")),
            Cmd::PageNumberSettings => {
                if let Some(d) = self.doc() {
                    let s = &d.doc.settings;
                    self.dialogs.page_numbers = Some(crate::dialogs::layout::PageNumberState { start: s.page_number_start, from_page: s.page_number_from, style: s.page_number_style as usize });
                }
            }
            Cmd::InsertPageNumber(which) => crate::dialogs::layout::insert_page_number(self, which),
            Cmd::BorderGrommet => {
                if self.doc().is_some() {
                    self.dialogs.border_grommet = Some(crate::dialogs::layout::BorderGrommetState { opts: Default::default() });
                }
            }
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
            Cmd::CenterToPage => {
                let use_outline = self.dockers.ui.use_outline;
                if let Some(d) = self.doc_mut() {
                    let ids = d.selection.clone();
                    if !ids.is_empty() {
                        let page = d.page;
                        d.modify("Center to Page", |doc, _| {
                            ops::align(doc, &ids, ops::Align::CenterH, AlignTo::Page, page, use_outline);
                            ops::align(doc, &ids, ops::Align::CenterV, AlignTo::Page, page, use_outline);
                        });
                    }
                }
            }
            Cmd::AlignPixelGrid => {
                let px = self.doc().map(|d| 72.0 / d.doc.settings.dpi.max(1.0)).unwrap_or(1.0);
                self.modify_sel("Align with Pixel Grid", move |doc, sel| ops::align_pixel_grid(doc, sel, px));
            }
            Cmd::ShowObjects => self.modify_sel("Show", |doc, sel| {
                for id in sel.iter() {
                    if let Some(o) = doc.find_mut(*id) {
                        o.visible = true;
                    }
                }
            }),
            Cmd::OverprintFill | Cmd::OverprintOutline | Cmd::OverprintBitmap | Cmd::ObjectHinting => {
                let get = |o: &Object| match cmd {
                    Cmd::OverprintFill => o.style.overprint_fill,
                    Cmd::OverprintOutline => o.style.overprint_outline,
                    Cmd::OverprintBitmap => o.style.overprint_bitmap,
                    _ => o.hinting,
                };
                let on = !self.doc().and_then(|d| d.selection.first().and_then(|id| d.doc.find(*id))).is_some_and(get);
                let name = match cmd {
                    Cmd::OverprintFill => "Overprint Fill",
                    Cmd::OverprintOutline => "Overprint Outline",
                    Cmd::OverprintBitmap => "Overprint Bitmap",
                    _ => "Object Hinting",
                };
                self.modify_sel(name, move |doc, sel| {
                    for id in sel.iter() {
                        if let Some(o) = doc.find_mut(*id) {
                            match cmd {
                                Cmd::OverprintFill => o.style.overprint_fill = on,
                                Cmd::OverprintOutline => o.style.overprint_outline = on,
                                Cmd::OverprintBitmap => o.style.overprint_bitmap = on,
                                _ => o.hinting = on,
                            }
                        }
                    }
                });
            }
            Cmd::PowerClipEmptyFrame => self.modify_sel("Create Empty PowerClip Frame", |doc, sel| {
                let mut out = vec![];
                for id in sel.clone() {
                    let Some((p, l, i)) = doc.locate(id) else { continue };
                    let o = doc.pages[p].layers[l].objects[i].clone();
                    if matches!(o.kind, ObjKind::PowerClip { .. }) {
                        out.push(id);
                        continue;
                    }
                    let nid = doc.new_id();
                    let mut w = Object::new(nid, ObjKind::PowerClip { frame: Box::new(o), contents: vec![] }, Affine::IDENTITY, Default::default());
                    w.name = "PowerClip".into();
                    doc.pages[p].layers[l].objects[i] = w;
                    out.push(nid);
                }
                *sel = out;
            }),
            Cmd::PowerClipLock => self.modify_sel("Lock Contents to PowerClip", |doc, sel| {
                for id in sel.iter() {
                    if let Some(o) = doc.find_mut(*id) {
                        if matches!(o.kind, ObjKind::PowerClip { .. }) {
                            o.powerclip_unlocked = !o.powerclip_unlocked;
                        }
                    }
                }
            }),
            Cmd::PowerClipPlace(m) => {
                let name = ["Center Contents", "Fit Contents Proportionally", "Fill Frame Proportionally", "Stretch Contents to Fill Frame"][m.min(3) as usize];
                self.modify_sel(name, move |doc, sel| {
                    for id in sel.iter() {
                        ops::powerclip_place(doc, *id, m);
                    }
                });
            }
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
            Cmd::Node(op) => crate::tools::node_op(self, op),
            Cmd::ConvertToCurves => self.modify_sel("Convert to Curves", |doc, sel| {
                let ids = sel.clone();
                *sel = ops::convert_to_curves(doc, &ids);
            }),
            Cmd::OutlineToObject => self.modify_sel("Convert Outline to Object", |doc, sel| {
                let ids = sel.clone();
                *sel = ops::outline_to_object(doc, &ids);
            }),
            Cmd::CreateCutContour => crate::dialogs::cutter::start_contour(self),
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
                            d.focus = vec![id];
                            d.selection.clear();
                            d.cache.key = 0;
                        }
                    }
                }
            }
            Cmd::PowerClipFinish if self.doc().is_some_and(|d| d.symbol_edit.is_some()) => crate::dialogs::object::finish_symbol(self),
            Cmd::PowerClipFinish if self.doc().is_some_and(|d| d.rollover_edit.is_some()) => crate::dialogs::object::finish_rollover(self),
            Cmd::PowerClipFinish => {
                if let Some(d) = self.doc_mut() {
                    if !d.focus.is_empty() {
                        d.selection = std::mem::take(&mut d.focus);
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
                        o.effects.envelope = Some(vector_core::object::EnvelopeEffect { handles: [kurbo::Vec2::ZERO; 8], ..Default::default() });
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
            Cmd::ConvertToBitmap => bm::open_convert(self),
            Cmd::StraightenImage => bm::open_straighten(self),
            Cmd::ImageAdjustmentLab => bm::open_lab(self),
            Cmd::RemoveJpegArtifacts => bm::remove_jpeg_artifacts(self),
            Cmd::EditBitmap => bm::edit_bitmap(self),
            Cmd::RemoveBackground => bm::remove_background(self),
            Cmd::CropBitmap => bm::crop_bitmap(self),
            Cmd::QuickTrace => {
                let o = bm::TraceOpts::quick(&self.settings.quick_trace_method);
                bm::trace_now(self, o);
            }
            Cmd::OutlineTrace(k) => bm::open_trace(self, bm::TraceOpts::profile(false, k as usize)),
            Cmd::CenterlineTrace(k) => bm::open_trace(self, bm::TraceOpts::profile(true, k as usize)),
            Cmd::RepeatLastTrace => {
                if let Some(o) = self.dialogs.bm.last_trace.clone() {
                    bm::trace_now(self, o);
                }
            }
            Cmd::OpenLastTraceSettings => {
                if let Some(o) = self.dialogs.bm.last_trace.clone() {
                    bm::open_trace(self, o);
                }
            }
            Cmd::BitmapMode(m) => bm::mode(self, m),
            Cmd::AutoInflateBitmap => {
                if let Some(d) = self.doc_mut() {
                    d.modify("Auto Inflate Bitmap", |doc, _| doc.bitmap_no_inflate = !doc.bitmap_no_inflate);
                }
            }
            Cmd::ManualInflateBitmap => bm::open_inflate(self),
            Cmd::BitmapBreakLink => bm::break_link(self),
            Cmd::BitmapUpdateLink => bm::update_link(self),
            Cmd::BitmapEffect(i) => {
                let Some(def) = crate::fx::FX.get(i as usize) else { return };
                if !self.doc().is_some_and(|d| !d.selection.is_empty()) {
                    self.flash("Select an object to apply the effect to.");
                    return;
                }
                let e = def.create();
                self.modify_sel(def.name, |doc, sel| {
                    for id in sel.iter() {
                        if let Some(o) = doc.find_mut(*id) {
                            o.effects.bitmap.push(e.clone());
                        }
                    }
                });
                // the Properties docker shows the page of the last effect of the list
                self.dockers.show(DockerId::Properties);
                let pr = &mut self.dockers.ui.pr;
                pr.tab = 3;
                pr.jump = true;
                pr.folded[3] = false;
                pr.fx_sel = None;
                pr.fx_pick = None;
            }
            Cmd::FlattenEffects => self.flatten_effects(),
            Cmd::AutoAdjust => bm::auto_adjust(self),
            Cmd::ResampleBitmap => bm::open_resample(self),

            // ------------------------------------------------------------------ Text
            Cmd::EditText => match crate::dialogs::text::EditTextState::new(self) {
                Some(st) => self.dialogs.edit_text = Some(st),
                None => {
                    self.set_tool(ToolId::Text);
                    self.dockers.show(DockerId::Text);
                }
            },
            Cmd::TextTabs => self.dialogs.tabs = Some(crate::dialogs::text::TabsState::new(self)),
            Cmd::TextColumns => self.dialogs.columns = Some(crate::dialogs::text::ColumnsState::new(self)),
            Cmd::TextBullets => self.dialogs.bullets = Some(crate::dialogs::text::BulletsState::new(self)),
            Cmd::TextDropCap => self.dialogs.drop_cap = Some(crate::dialogs::text::DropCapState::new(self)),
            Cmd::TextStatistics => self.dialogs.text_stats = Some(crate::dialogs::text::StatsState { styles: true }),
            Cmd::CustomHyphens => self.dialogs.hyphens = Some(crate::dialogs::text::HyphensState::new(self)),
            Cmd::InsertFormatCode(k) => {
                if self.doc().is_some_and(|d| d.text_edit.is_some()) {
                    crate::tools::insert_text(self, &crate::dialogs::text::format_code(k).to_string(), None);
                }
            }
            Cmd::ToggleNonPrinting => self.settings.show_nonprinting = !self.settings.show_nonprinting,
            Cmd::ToggleTextFrames => self.settings.show_text_frames = !self.settings.show_text_frames,
            Cmd::FitTextToFrame => crate::dialogs::text::fit_text_to_frame(self),
            Cmd::InsertPlaceholder => crate::dialogs::text::insert_placeholder(self),
            Cmd::CreateEmptyFrame => crate::dialogs::text::create_empty_frame(self),
            Cmd::LinkFrames => crate::dialogs::text::link_frames(self),
            Cmd::UnlinkFrames => crate::dialogs::text::unlink_frames(self),
            Cmd::AlignToBaseline => crate::dialogs::text::align_to_baseline(self),
            Cmd::AlignToBaselineGrid => crate::dialogs::text::toggle_para(self, "Align to Baseline Grid", |t| t.align_grid, |t, v| t.align_grid = v),
            Cmd::ToggleHyphenation => crate::dialogs::text::toggle_para(self, "Hyphenation", |t| t.hyphenate, |t, v| t.hyphenate = v),
            Cmd::MakeWebCompatible => crate::dialogs::text::toggle_para(self, "Make Text Web Compatible", |t| t.web, |t, v| t.web = v),
            Cmd::WritingTools(k) => self.dialogs.writing = Some(crate::dialogs::text::WritingState::new(self, k as usize)),
            Cmd::QuickCorrect => {
                self.open_options(OptionsPage::App, Some("Text"));
                self.dialogs.options.tab = 4;
            }
            Cmd::WritingToolsSettings => {
                self.open_options(OptionsPage::App, Some("Text"));
                self.dialogs.options.tab = 3;
            }
            Cmd::TextLanguage => self.dialogs.text_lang = Some(crate::dialogs::text::LanguageState::new(self)),
            Cmd::ChangeCaseDialog => self.dialogs.change_case = Some(0),
            Cmd::EncodeText => self.dialogs.encode = Some(crate::dialogs::text::EncodeState::new(self)),
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
            Cmd::SpellCheck => self.dialogs.writing = Some(crate::dialogs::text::WritingState::new(self, 0)),

            // ------------------------------------------------------------------ Table
            Cmd::CreateTable => {
                if self.doc().is_some() {
                    let mm = 72.0 / 25.4;
                    self.dialogs.table = Some(crate::dialogs::table::TableDialog::Create { rows: self.settings.table_rows.max(1) as usize, cols: self.settings.table_cols.max(1) as usize, height: 100.0 * mm, width: 75.0 * mm });
                }
            }
            // Row Above / Below and Column Left / Right insert as many as are selected
            Cmd::TableInsertRow(above) => {
                let n = self.table_sel().and_then(|(_, _, g)| g).map(|g| g[2] - g[0] + 1).unwrap_or(1);
                self.table_op(|t, g| {
                    let at = match g {
                        Some(g) if above => g[0],
                        Some(g) => g[2] + 1,
                        None => t.rows,
                    };
                    for _ in 0..n {
                        t.insert_row(at);
                    }
                });
                if above {
                    self.ts.shift_cells((n, 0));
                }
            }
            Cmd::TableInsertCol(left) => {
                let n = self.table_sel().and_then(|(_, _, g)| g).map(|g| g[3] - g[1] + 1).unwrap_or(1);
                self.table_op(|t, g| {
                    let at = match g {
                        Some(g) if left => g[1],
                        Some(g) => g[3] + 1,
                        None => t.cols,
                    };
                    for _ in 0..n {
                        t.insert_col(at);
                    }
                });
                if left {
                    self.ts.shift_cells((0, n));
                }
            }
            // deleting every row (or column) deletes the table
            Cmd::TableDeleteRow | Cmd::TableDeleteCol => {
                let cols = cmd == Cmd::TableDeleteCol;
                let Some((_, t, g)) = self.table_sel() else { return };
                let g = g.unwrap_or(if cols { [0, t.cols - 1, 0, t.cols - 1] } else { [t.rows - 1, 0, t.rows - 1, 0] });
                let all = if cols { g[3] - g[1] + 1 >= t.cols } else { g[2] - g[0] + 1 >= t.rows };
                if all {
                    self.run(Cmd::Delete);
                    self.ts.table_cell = None;
                    self.ts.table_cell_to = None;
                    return;
                }
                self.table_op(|t, _| {
                    if cols {
                        for c in (g[1]..=g[3]).rev() {
                            t.delete_col(c);
                        }
                    } else {
                        for r in (g[0]..=g[2]).rev() {
                            t.delete_row(r);
                        }
                    }
                });
                self.ts.table_cell = Some(if cols { (g[0], g[1].min(t.cols - 1 - (g[3] - g[1] + 1))) } else { (g[0].min(t.rows - 1 - (g[2] - g[0] + 1)), g[1]) });
                self.ts.table_cell_to = None;
            }
            // Distribute: the selected rows (columns) when more than one is selected, else all
            Cmd::TableDistributeRows => self.table_op(|t, g| {
                let (a, b) = g.filter(|g| g[2] > g[0]).map(|g| (g[0], g[2] + 1)).unwrap_or((0, t.rows));
                let h: f64 = t.row_heights[a..b].iter().sum::<f64>() / (b - a) as f64;
                t.row_heights[a..b].iter_mut().for_each(|x| *x = h);
            }),
            Cmd::TableDistributeCols => self.table_op(|t, g| {
                let (a, b) = g.filter(|g| g[3] > g[1]).map(|g| (g[1], g[3] + 1)).unwrap_or((0, t.cols));
                let w: f64 = t.col_widths[a..b].iter().sum::<f64>() / (b - a) as f64;
                t.col_widths[a..b].iter_mut().for_each(|x| *x = w);
            }),
            Cmd::TextToTable => {
                if self.doc().is_some() {
                    self.dialogs.table = Some(crate::dialogs::table::TableDialog::TextToTable { sep: 0, user: String::new() });
                }
            }
            Cmd::TableToText => {
                if self.table_sel().is_some() {
                    self.dialogs.table = Some(crate::dialogs::table::TableDialog::TableToText { sep: 1, user: String::new() });
                }
            }
            Cmd::TableInsertMany(cols) => {
                if self.table_sel().is_some() {
                    self.dialogs.table = Some(crate::dialogs::table::TableDialog::Insert { cols, count: 1, before: true });
                }
            }
            Cmd::CreateArrowhead => self.dialogs.arrowhead = crate::dialogs::object::ArrowheadState::new(self),
            Cmd::CreatePattern => self.dialogs.pattern = Some(crate::dialogs::object::PatternState { kind: 1, resolution: 1 }),
            Cmd::InsertQrCode => crate::dialogs::object::insert_qr(self),
            Cmd::InsertBarcode => {
                self.dialogs.barcode = Some(crate::dialogs::object::BarcodeState { kind: 0, data: String::new(), height: 54.0, module: 1.0, show_text: true });
            }
            Cmd::ValidateBarcode => crate::dialogs::object::validate_barcode(self),
            Cmd::AiGuided(mode) => crate::ai::guided(self, mode),
            Cmd::AiMode(mode) => {
                self.ai.mode = mode;
                self.ai.reference = None;
                self.dockers.show(DockerId::AiGenerate);
            }
            Cmd::SymbolEditMaster => crate::dialogs::object::edit_master(self),
            Cmd::SymbolFinish => crate::dialogs::object::finish_symbol(self),
            Cmd::SymbolBreakLink => crate::dialogs::object::break_link(self),
            Cmd::SymbolUpdateLink => crate::dialogs::object::update_from_link(self),
            Cmd::SymbolExportLibrary => crate::dialogs::object::export_library(self),
            Cmd::SymmetryCreate => crate::dialogs::symmetry::create_symmetry(self),
            Cmd::SymmetryEdit => crate::dialogs::symmetry::edit_symmetry(self),
            Cmd::SymmetryFinish => crate::dialogs::symmetry::finish_symmetry(self),
            Cmd::SymmetryRemove => crate::dialogs::symmetry::remove_symmetry(self),
            Cmd::SymmetryBreak => crate::dialogs::symmetry::break_symmetry(self),
            Cmd::PerspectiveDraw => crate::dialogs::symmetry::draw_in_perspective(self),
            Cmd::PerspectiveEdit => crate::dialogs::symmetry::edit_perspective(self),
            Cmd::PerspectiveFinish => crate::dialogs::symmetry::finish_perspective(self),
            Cmd::MoveToPlane(p) => crate::dialogs::symmetry::move_to_plane(self, p),
            Cmd::InsertObject => self.dialogs.insert_object = Some(Default::default()),
            Cmd::OleOpen => {
                if let Some(&[id]) = self.doc().map(|d| d.selection.as_slice()) {
                    crate::dialogs::symmetry::open_ole(self, id);
                }
            }
            Cmd::RolloverCreate => crate::dialogs::object::create_rollover(self),
            Cmd::RolloverExtract => crate::dialogs::object::extract_rollover(self),
            Cmd::RolloverEdit => crate::dialogs::object::edit_rollover(self),
            Cmd::RolloverFinish => crate::dialogs::object::finish_rollover(self),
            Cmd::RolloverLivePreview => {
                self.rollover_live = !self.rollover_live;
                if !self.rollover_live {
                    crate::dialogs::object::stop_live_preview(self);
                }
            }
            Cmd::RolloverDuplicateState => crate::dialogs::object::state_contents(self, true),
            Cmd::RolloverDeleteState => crate::dialogs::object::state_contents(self, false),
            Cmd::ShowHotspots => self.settings.show_hotspots = !self.settings.show_hotspots,
            Cmd::TableSplit(cols) => {
                if self.table_sel().is_some() {
                    self.dialogs.table = Some(crate::dialogs::table::TableDialog::Split { cols, count: 2 });
                }
            }
            Cmd::TableSelect(what) => {
                let Some((_, t, g)) = self.table_sel() else { return };
                self.set_tool(ToolId::Table);
                let g = g.unwrap_or([0, 0, 0, 0]);
                let (a, b) = match what {
                    0 => ((g[0], g[1]), None),
                    1 => ((g[0], 0), Some((g[2], t.cols - 1))),
                    2 => ((0, g[1]), Some((t.rows - 1, g[3]))),
                    _ => ((0, 0), Some((t.rows - 1, t.cols - 1))),
                };
                self.ts.table_cell = Some(a);
                self.ts.table_cell_to = b;
            }
            Cmd::TableDeleteTable => {
                if self.table_sel().is_some() {
                    self.run(Cmd::Delete);
                    self.ts.table_cell = None;
                    self.ts.table_cell_to = None;
                }
            }
            Cmd::TableMerge => {
                let Some((_, _, Some(g))) = self.table_sel() else { return };
                self.table_op(|t, _| t.merge(g));
                self.ts.table_cell = Some((g[0], g[1]));
                self.ts.table_cell_to = None;
            }
            Cmd::TableUnmerge => {
                let Some((_, _, Some(g))) = self.table_sel() else { return };
                self.table_op(|t, _| t.unmerge(g));
            }

            // ------------------------------------------------------------------ Tools
            Cmd::Options(p) => self.open_options(p, None),
            Cmd::SaveSettingsAsDefault => {
                self.dockers.save(&mut self.settings);
                self.settings.save();
                self.flash("Settings saved as default");
            }
            Cmd::ColorManagement => crate::dialogs::colormgmt::open(self),
            Cmd::ScriptEditor | Cmd::VsEditor => {
                if self.dialogs.script_editor.is_none() {
                    self.dialogs.script_editor = Some(ScriptEditorState { code: crate::dialogs::forms::SCRIPT_TEMPLATE.to_string(), ..Default::default() });
                }
            }
            Cmd::RunScript => {
                if let Some(p) = rfd::FileDialog::new().set_title("Run Script").add_filter("AnyVector script", &["rhai", "vsc"]).set_directory(crate::scripting::scripts_dir()).pick_file() {
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
            Cmd::NewWindow => self.new_window(),
            Cmd::RefreshWindow => {
                for d in self.docs.iter_mut() {
                    d.cache.key = 0;
                }
                self.ts.texture_cache.clear();
            }
            Cmd::TileH => self.arrange_windows(1),
            Cmd::TileV => self.arrange_windows(2),
            Cmd::Cascade => self.arrange_windows(3),
            Cmd::CombineWindows | Cmd::DockWindow => self.arrange_windows(0),
            Cmd::Workspace(i) => {
                if let Some(n) = crate::workspaces::by_index(&self.settings, i as usize) {
                    crate::workspaces::switch(self, &n);
                }
            }
            Cmd::WorkspaceNew => crate::dialogs::workspace::open_new(self),
            Cmd::WorkspaceExport => {
                let n = self.settings.workspace.clone();
                crate::dialogs::workspace::open_export(self, &n);
            }
            Cmd::WorkspaceImport => crate::dialogs::workspace::import(self),
            Cmd::WorkspaceDelete => {
                if crate::workspaces::is_custom(&self.settings, &self.settings.workspace) {
                    self.dialogs.ws.delete = Some(self.settings.workspace.clone());
                }
            }
            Cmd::Docker(id) => self.dockers.toggle(id),
            Cmd::TransformTab(i) => {
                self.dockers.show(DockerId::Transform);
                self.dockers.ui.transform_tab = (i as usize).min(4);
            }
            Cmd::TogglePickState => {
                if self.tool == ToolId::Pick {
                    if let Some(prev) = self.ts.previous_tool {
                        self.set_tool(prev);
                    }
                } else {
                    self.ts.previous_tool = Some(self.tool);
                    self.set_tool(ToolId::Pick);
                }
            }
            Cmd::NodeToggle(k) => {
                use crate::tools::NodeOp;
                use vector_core::path::NodeKind;
                // the first selected node decides which way the toggle goes
                let node = self.doc().and_then(|d| {
                    let (id, si, ni) = *d.node_sel.first()?;
                    match &d.doc.find(id)?.kind {
                        ObjKind::Path(np) => np.subpaths.get(si)?.nodes.get(ni).copied(),
                        _ => None,
                    }
                });
                if let Some(n) = node {
                    let op = match k {
                        0 if n.kind == NodeKind::Cusp => NodeOp::Smooth,
                        0 => NodeOp::Cusp,
                        1 if n.kind == NodeKind::Smooth => NodeOp::Symmetric,
                        1 => NodeOp::Smooth,
                        _ if n.line_out => NodeOp::ToCurve,
                        _ => NodeOp::ToLine,
                    };
                    // the nodes stay selected, so the key toggles back and forth
                    let keep = self.doc().map(|d| d.node_sel.clone()).unwrap_or_default();
                    crate::tools::node_op(self, op);
                    if let Some(d) = self.doc_mut() {
                        d.node_sel = keep;
                    }
                }
            }
            Cmd::ToggleToolbar(t) => self.toggle_toolbar(t),
            Cmd::ButtonSize(s) => self.settings.button_size = s,
            Cmd::LockToolbars => self.settings.lock_toolbars = !self.settings.lock_toolbars,
            Cmd::AddColorsFromSelection => self.palette_from(true, true),
            Cmd::AddColorsFromDocument => self.palette_from(false, true),
            Cmd::CreatePaletteFromSelection => self.palette_from(true, false),
            Cmd::CreatePaletteFromDocument => self.palette_from(false, false),
            Cmd::OpenPalette => crate::dialogs::palette::open_palette(self),
            Cmd::CloseAllPalettes => {
                self.palettes.palettes.truncate(1);
                self.settings.open_palettes.clear();
                self.settings.default_palette_visible = false;
                self.settings.doc_palette_visible = false;
                self.settings.color_styles_palette_visible = false;
            }
            Cmd::PaletteEditor => crate::dialogs::palette::open_editor(self),
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
            Cmd::Updates => crate::updater::open_dialog(self),
            Cmd::ProductHelp | Cmd::UserGuide | Cmd::QuickStart => {
                let page = match cmd {
                    Cmd::QuickStart => crate::help::Page::QuickStart,
                    Cmd::UserGuide => crate::help::Page::UserGuide,
                    _ => crate::help::Page::Home,
                };
                if let Err(e) = crate::help::open(page) {
                    self.info_box("AnyVector Help", &e);
                }
            }
            Cmd::WelcomeScreen | Cmd::ShowHome => {
                self.finish_tool();
                self.home_open = true;
                self.active = None;
            }
            Cmd::VideoTutorials => self.dockers.show(DockerId::Learn),
            Cmd::WhatsNew => {
                self.finish_tool();
                self.home_open = true;
                self.active = None;
                self.welcome_page = crate::welcome::Page::WhatsNew;
            }
            Cmd::MessageSettings | Cmd::RefreshAccount | Cmd::AccountSettings => self.info_box("Account", "AnyVector does not require an account. All your work stays on this computer."),
            Cmd::Feedback => {
                self.dialogs.feedback = Some(crate::bugreport::BugReportState::idea());
            }
            Cmd::Community | Cmd::Support => self.info_box("AnyVector Community", "Share ideas and report problems with Help > Submit Ideas and Feedback and Help > Report a Bug."),
            Cmd::ReportBug => self.dialogs.bug_report = Some(crate::bugreport::BugReportState::new()),
            Cmd::ViewReportedBugs => {
                crate::bugreport::open_in_browser(&crate::bugreport::issues_url());
            }
            Cmd::About => self.dialogs.about = true,

            // ------------------------------------------------------------------ Misc
            Cmd::Tool(t) => {
                // a reference tool that stands for several AnyVector tools starts as the kind last used
                let t = self.toolbox.kind_of(t);
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_conversions() {
        assert_eq!(title_case("hello big-world"), "Hello Big-World");
        assert_eq!(sentence_case("HELLO. WORLD"), "Hello. World");
    }
}

/// Replace each «field» in a text object by the record's value, keeping the character formats.
pub fn fill_merge_fields(t: &mut vector_core::text::TextObj, pm: &vector_core::document::PrintMerge, rec: &vector_core::document::MergeRecord) {
    for (i, f) in pm.fields.iter().enumerate() {
        let marker = vector_core::document::PrintMerge::marker(&f.name);
        let value = rec.values.get(i).cloned().unwrap_or_default();
        let mut guard = 0;
        while let Some(b) = t.content.find(&marker) {
            guard += 1;
            if guard > 1000 {
                break;
            }
            let at = t.content[..b].chars().count();
            t.delete(at, at + marker.chars().count());
            if !value.is_empty() {
                t.insert(at, &value);
            }
        }
    }
}

/// The merged document of Print Merge: the pages repeated for each selected record, with the
/// fields filled in. None when there is no merge data or no record is selected.
pub fn merged_document(doc: &Document) -> Option<Document> {
    let pm = doc.print_merge.clone()?;
    let records: Vec<_> = pm.records.iter().filter(|r| r.selected).cloned().collect();
    if records.is_empty() {
        return None;
    }
    let mut out = doc.clone();
    out.print_merge = None;
    let originals = std::mem::take(&mut out.pages);
    for rec in &records {
        for pg in &originals {
            let mut p = pg.clone();
            p.id = out.new_id();
            for l in p.layers.iter_mut() {
                l.id = out.new_id();
                for o in l.objects.iter_mut() {
                    out.reassign_ids(o);
                    o.walk_mut(&mut |x| {
                        if let ObjKind::Text(t) = &mut x.kind {
                            fill_merge_fields(t, &pm, rec);
                        }
                    });
                }
            }
            out.pages.push(p);
        }
    }
    Some(out)
}
