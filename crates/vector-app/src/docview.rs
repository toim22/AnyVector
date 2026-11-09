//! An open document: model, history, selection and view state.

use egui::{Pos2, Rect as ERect, Vec2};
use kurbo::{Affine, Point, Rect};
use std::path::PathBuf;
use vector_core::history::History;
use vector_core::{Document, ObjId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ViewMode {
    Wireframe,
    Normal,
    Enhanced,
    Pixels,
}

impl ViewMode {
}

pub struct RenderCache {
    pub key: u64,
    pub tex: Option<egui::TextureHandle>,
}

impl Default for RenderCache {
    fn default() -> Self {
        RenderCache { key: 0, tex: None }
    }
}

/// Text editing state (caret/selection in characters).
#[derive(Clone, Debug)]
pub struct TextEdit {
    pub obj: ObjId,
    pub caret: usize,
    pub anchor: usize,
    pub blink: f64,
}

pub struct DocView {
    pub uid: u64,
    pub doc: Document,
    pub history: History,
    pub path: Option<PathBuf>,
    pub title: String,
    pub selection: Vec<ObjId>,
    pub page: usize,
    pub layer: usize,
    /// Screen points per document point.
    pub zoom: f64,
    /// Screen position (relative to the canvas rect min) of document point (0,0).
    pub origin: Vec2,
    pub needs_fit: bool,
    pub view_mode: ViewMode,
    /// View ▸ Simulate Overprints.
    pub simulate_overprint: bool,
    pub cache: RenderCache,
    /// Selected nodes: (object, subpath, node).
    pub node_sel: Vec<(ObjId, usize, usize)>,
    pub text_edit: Option<TextEdit>,
    /// PowerClip contents editing / focus mode container.
    pub focus: Vec<ObjId>,
    /// Pick tool shows rotate/skew handles.
    pub rotate_mode: bool,
    pub canvas_rect: ERect,
    pub show_page_sorter: bool,
    /// Object ▸ Symbol ▸ Edit Master in progress.
    pub symbol_edit: Option<crate::dialogs::object::SymbolEdit>,
    /// Object ▸ Rollover ▸ Edit Rollover in progress: the rollover.
    pub rollover_edit: Option<ObjId>,
    pub last_saved_backup: f64,
}

pub fn view_affine(zoom: f64, origin: Vec2) -> Affine {
    Affine::translate((origin.x as f64, origin.y as f64)) * Affine::scale(zoom)
}

impl DocView {
    pub fn new(doc: Document, title: String, undo_levels: usize) -> DocView {
        static UID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        DocView {
            uid: UID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            doc,
            history: History::new(undo_levels),
            path: None,
            title,
            selection: vec![],
            page: 0,
            layer: 0,
            zoom: 1.0,
            origin: Vec2::ZERO,
            needs_fit: true,
            view_mode: ViewMode::Enhanced,
            simulate_overprint: false,
            cache: RenderCache::default(),
            node_sel: vec![],
            text_edit: None,
            focus: vec![],
            rotate_mode: false,
            canvas_rect: ERect::NOTHING,
            show_page_sorter: false,
            symbol_edit: None,
            rollover_edit: None,
            last_saved_backup: 0.0,
        }
    }

    /// The document name without the modified mark and the drawing extension.
    pub fn stem(&self) -> String {
        let n = self.display_name();
        let n = n.trim_end_matches('*');
        n.strip_suffix(".cdr").or_else(|| n.strip_suffix(".vct")).unwrap_or(n).to_string()
    }

    pub fn display_name(&self) -> String {
        let n = self.path.as_ref().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| self.title.clone());
        if self.history.is_modified() {
            format!("{n}*")
        } else {
            n
        }
    }

    /// Record an undo step before modifying the document.
    pub fn checkpoint(&mut self, label: &str) {
        self.history.push(label, &self.doc, &self.selection, self.page);
    }

    /// Run a modification as one undoable step.
    pub fn modify<R>(&mut self, label: &str, f: impl FnOnce(&mut Document, &mut Vec<ObjId>) -> R) -> R {
        self.checkpoint(label);
        let r = f(&mut self.doc, &mut self.selection);
        self.after_change();
        r
    }

    pub fn after_change(&mut self) {
        self.doc.update_connectors();
        self.doc.update_page_numbers();
        self.doc.update_text_flow();
        let doc = &self.doc;
        self.selection.retain(|id| doc.find(*id).is_some());
        self.node_sel.retain(|(id, _, _)| doc.find(*id).is_some());
        if self.page >= self.doc.pages.len() {
            self.page = self.doc.pages.len().saturating_sub(1);
        }
        let nl = self.doc.pages[self.page].layers.len();
        if self.layer >= nl {
            self.layer = nl.saturating_sub(1);
        }
    }

    pub fn undo(&mut self) -> Option<String> {
        let r = self.history.undo(&mut self.doc, &mut self.selection, &mut self.page);
        self.text_edit = None;
        self.node_sel.clear();
        self.after_change();
        r
    }

    pub fn redo(&mut self) -> Option<String> {
        let r = self.history.redo(&mut self.doc, &mut self.selection, &mut self.page);
        self.text_edit = None;
        self.node_sel.clear();
        self.after_change();
        r
    }

    // ---------------------------------------------------------------- coordinates

    pub fn view(&self) -> Affine {
        view_affine(self.zoom, self.origin)
    }

    /// Document point → absolute screen position.
    pub fn to_screen(&self, p: Point) -> Pos2 {
        let v = self.view() * p;
        self.canvas_rect.min + egui::vec2(v.x as f32, v.y as f32)
    }

    pub fn to_screen_rect(&self, r: Rect) -> ERect {
        ERect::from_two_pos(self.to_screen(Point::new(r.x0, r.y0)), self.to_screen(Point::new(r.x1, r.y1)))
    }

    /// Absolute screen position → document point.
    pub fn to_doc(&self, p: Pos2) -> Point {
        let local = p - self.canvas_rect.min;
        Point::new((local.x as f64 - self.origin.x as f64) / self.zoom, (local.y as f64 - self.origin.y as f64) / self.zoom)
    }

    /// Screen distance in points → document distance.
    pub fn doc_len(&self, screen: f32) -> f64 {
        screen as f64 / self.zoom
    }

    /// Zoom so the rect (document coords) fills the canvas.
    pub fn zoom_to_rect(&mut self, r: Rect, margin: f32) {
        let cs = self.canvas_rect.size();
        if cs.x <= 1.0 || r.width() <= 0.0 && r.height() <= 0.0 {
            return;
        }
        let zx = (cs.x - margin * 2.0) as f64 / r.width().max(1e-6);
        let zy = (cs.y - margin * 2.0) as f64 / r.height().max(1e-6);
        self.zoom = zx.min(zy).clamp(0.002, 2000.0);
        let c = r.center();
        self.origin = egui::vec2((cs.x as f64 / 2.0 - c.x * self.zoom) as f32, (cs.y as f64 / 2.0 - c.y * self.zoom) as f32);
    }

    pub fn zoom_page(&mut self) {
        let r = self.doc.pages[self.page].rect();
        self.zoom_to_rect(r, 24.0);
    }

    pub fn zoom_page_width(&mut self) {
        let r = self.doc.pages[self.page].rect();
        let cs = self.canvas_rect.size();
        self.zoom = ((cs.x - 40.0) as f64 / r.width()).clamp(0.002, 2000.0);
        let c = r.center();
        self.origin = egui::vec2((cs.x as f64 / 2.0 - c.x * self.zoom) as f32, (20.0 - r.y0 * self.zoom as f64) as f32);
    }

    pub fn zoom_page_height(&mut self) {
        let r = self.doc.pages[self.page].rect();
        let cs = self.canvas_rect.size();
        self.zoom = ((cs.y - 40.0) as f64 / r.height()).clamp(0.002, 2000.0);
        let c = r.center();
        self.origin = egui::vec2((cs.x as f64 / 2.0 - c.x * self.zoom) as f32, (cs.y as f64 / 2.0 - c.y * self.zoom) as f32);
    }

    /// Zoom by a factor around a screen point.
    pub fn zoom_at(&mut self, screen: Pos2, factor: f64) {
        let d = self.to_doc(screen);
        self.zoom = (self.zoom * factor).clamp(0.002, 2000.0);
        let local = screen - self.canvas_rect.min;
        self.origin = egui::vec2((local.x as f64 - d.x * self.zoom) as f32, (local.y as f64 - d.y * self.zoom) as f32);
    }

    /// 100% = actual size (96 screen points per inch).
    pub fn zoom_percent(&self) -> f64 {
        self.zoom / (96.0 / 72.0) * 100.0
    }

    pub fn set_zoom_percent(&mut self, pct: f64) {
        let c = self.canvas_rect.center();
        let f = (pct / 100.0 * 96.0 / 72.0) / self.zoom;
        self.zoom_at(c, f);
    }

    pub fn selection_bounds(&self) -> Option<Rect> {
        self.doc.selection_bounds(&self.selection)
    }

    pub fn page_ref(&self) -> &vector_core::Page {
        &self.doc.pages[self.page]
    }

    /// The layer new objects go to (unlocked, visible).
    pub fn active_layer_mut(&mut self) -> &mut vector_core::Layer {
        let p = &mut self.doc.pages[self.page];
        let idx = self.layer.min(p.layers.len() - 1);
        &mut p.layers[idx]
    }

    /// Add objects on top of the active layer and select them.
    pub fn add_objects(&mut self, label: &str, objs: Vec<vector_core::Object>) {
        if objs.is_empty() {
            return;
        }
        self.checkpoint(label);
        let ids: Vec<ObjId> = objs.iter().map(|o| o.id).collect();
        self.active_layer_mut().objects.extend(objs);
        self.selection = ids;
        self.after_change();
    }
}
