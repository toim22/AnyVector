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
    /// View ▸ Pattern Preview (pattern documents): the page repeated as tiles around it.
    PatternPreview,
}

/// A pattern document: the document whose pattern (the tile image `image`) it edits.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternLink {
    pub group: u64,
    pub image: u64,
    /// The pattern tile whose objects the document shows (`Document::pattern_tiles`); `None`
    /// for a pattern known only by its picture.
    pub tile: Option<u64>,
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
    /// The guideline clicked with the Pick tool (page, index into its `guides`): Delete removes it.
    pub guide_sel: Option<(usize, usize)>,
    pub page: usize,
    pub layer: usize,
    /// The active layer is a master layer (stored with the first page; `layer` indexes the
    /// first page's layers).
    pub on_master: bool,
    /// Screen points per document point.
    pub zoom: f64,
    /// Screen position (relative to the canvas rect min) of document point (0,0).
    pub origin: Vec2,
    pub needs_fit: bool,
    /// The view (zoom, origin) the last automatic page fit produced. While the view is still
    /// exactly that one (not zoomed or scrolled since), a change of the canvas size fits the
    /// page again: at start-up the window grows from the splash size to its real size (and
    /// the display scale is applied) after the first document has already been fitted.
    pub auto_fit: Option<(f64, Vec2)>,
    pub view_mode: ViewMode,
    /// View ▸ Simulate Overprints.
    pub simulate_overprint: bool,
    pub cache: RenderCache,
    /// Selected nodes: (object, subpath, node).
    pub node_sel: Vec<(ObjId, usize, usize)>,
    /// Shape tool: the segment clicked (object, subpath, segment), see `tools::selected_segment`.
    pub seg_sel: Option<(ObjId, usize, usize)>,
    pub text_edit: Option<TextEdit>,
    /// PowerClip contents editing / focus mode container.
    pub focus: Vec<ObjId>,
    /// Pick tool shows rotate/skew handles.
    pub rotate_mode: bool,
    /// The rotation center moved away from the middle of the selection (issue 79): for the
    /// objects it was moved for, and where it is. It goes along when a command replaces the
    /// selection with copies (Duplicate, Step and Repeat, Paste; issue 126) and is kept with each
    /// undo step; commands that make new objects out of the selection (Group, Combine, ...) leave
    /// it behind, so those turn around their middle again.
    pub rot_center: Option<(Vec<ObjId>, Point)>,
    /// The property-bar slider drag the last change belongs to (its key, and the history revision
    /// right after it): the following frames of the same drag change the document without one
    /// more undo step each (issue 107).
    pub live_edit: Option<(u64, u64)>,
    pub canvas_rect: ERect,
    pub show_page_sorter: bool,
    /// Object ▸ Symbol ▸ Edit Master in progress.
    pub symbol_edit: Option<crate::dialogs::object::SymbolEdit>,
    /// Object ▸ Rollover ▸ Edit Rollover in progress: the rollover.
    pub rollover_edit: Option<ObjId>,
    /// Object ▸ Symmetry: the symmetry being drawn or edited.
    pub symmetry_edit: Option<crate::dialogs::symmetry::SymEdit>,
    /// Object ▸ Perspective: the perspective group being drawn or edited.
    pub persp_edit: Option<crate::dialogs::symmetry::PerspEdit>,
    pub last_saved_backup: f64,
    /// Window ▸ New Window: the windows showing the same document share a group (the first
    /// window's uid) and are numbered 1, 2, ... (0 while the document has one window).
    pub group: u64,
    pub window_no: usize,
    /// Properties ▸ Vector Pattern ▸ Edit fill: this document is that pattern's tile.
    pub pattern: Option<PatternLink>,
}

pub fn view_affine(zoom: f64, origin: Vec2) -> Affine {
    Affine::translate((origin.x as f64, origin.y as f64)) * Affine::scale(zoom)
}

impl DocView {
    pub fn new(doc: Document, title: String, undo_levels: usize) -> DocView {
        static UID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let uid = UID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        DocView {
            uid,
            group: uid,
            window_no: 0,
            pattern: None,
            doc,
            history: History::new(undo_levels),
            path: None,
            title,
            selection: vec![],
            guide_sel: None,
            page: 0,
            layer: 0,
            on_master: false,
            zoom: 1.0,
            origin: Vec2::ZERO,
            needs_fit: true,
            auto_fit: None,
            view_mode: ViewMode::Enhanced,
            simulate_overprint: false,
            cache: RenderCache::default(),
            node_sel: vec![],
            seg_sel: None,
            text_edit: None,
            focus: vec![],
            rotate_mode: false,
            rot_center: None,
            live_edit: None,
            canvas_rect: ERect::NOTHING,
            show_page_sorter: false,
            symbol_edit: None,
            rollover_edit: None,
            symmetry_edit: None,
            persp_edit: None,
            last_saved_backup: 0.0,
        }
    }

    /// The document name without the modified mark and the drawing extension.
    pub fn stem(&self) -> String {
        let n = self.display_name();
        let n = n.trim_end_matches('*');
        let n = if self.window_no > 0 { n.rsplit_once(':').map(|(a, _)| a).unwrap_or(n) } else { n };
        n.strip_suffix(".cdr").or_else(|| n.strip_suffix(".vct")).unwrap_or(n).to_string()
    }

    pub fn display_name(&self) -> String {
        let mut n = self.path.as_ref().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| self.title.clone());
        if self.window_no > 0 {
            n = format!("{n}:{}", self.window_no);
        }
        if self.history.is_modified() {
            format!("{n}*")
        } else {
            n
        }
    }

    /// Record an undo step before modifying the document.
    pub fn checkpoint(&mut self, label: &str) {
        self.history.push(label, &self.doc, &self.selection, self.page, &self.rot_center);
    }

    /// Run a modification as one undoable step.
    pub fn modify<R>(&mut self, label: &str, f: impl FnOnce(&mut Document, &mut Vec<ObjId>) -> R) -> R {
        self.checkpoint(label);
        let r = f(&mut self.doc, &mut self.selection);
        self.after_change();
        r
    }

    /// Run a modification that is part of the drag `key` (a property-bar slider): the first one
    /// records an undo step, the next ones of the same drag (with nothing else changed in between)
    /// join it, so one drag is one undo step.
    pub fn modify_live<R>(&mut self, key: u64, label: &str, f: impl FnOnce(&mut Document, &mut Vec<ObjId>) -> R) -> R {
        if self.live_edit == Some((key, self.history.revision)) && self.history.can_undo() {
            self.history.touch();
        } else {
            self.checkpoint(label);
        }
        let r = f(&mut self.doc, &mut self.selection);
        self.after_change();
        self.live_edit = Some((key, self.history.revision));
        r
    }

    pub fn after_change(&mut self) {
        self.doc.update_connectors();
        self.doc.update_page_numbers();
        self.doc.update_text_flow();
        // one walk of the document, not one per selected object: after an ungroup the selection
        // can hold every object of a large drawing
        let ids = self.doc.all_ids();
        self.selection.retain(|id| ids.contains(id));
        self.node_sel.retain(|(id, _, _)| ids.contains(id));
        self.seg_sel = self.seg_sel.filter(|(id, _, _)| ids.contains(id));
        if self.page >= self.doc.pages.len() {
            self.page = self.doc.pages.len().saturating_sub(1);
        }
        if self.on_master && !self.doc.pages[0].layers.get(self.layer).is_some_and(|l| l.master != 0) {
            self.on_master = false;
            self.layer = 0;
        }
        let nl = self.doc.pages[self.active_loc().0].layers.len();
        if self.layer >= nl {
            self.layer = nl.saturating_sub(1);
        }
    }

    /// Where the active layer is stored: (page, layer index).
    pub fn active_loc(&self) -> (usize, usize) {
        if self.on_master {
            (0, self.layer)
        } else {
            (self.page, self.layer)
        }
    }

    pub fn undo(&mut self) -> Option<String> {
        let r = self.history.undo(&mut self.doc, &mut self.selection, &mut self.page, &mut self.rot_center);
        // a PowerClip being edited that the step took away is no longer edited
        self.focus.retain(|id| self.doc.find(*id).is_some());
        self.text_edit = None;
        self.node_sel.clear();
        self.seg_sel = None;
        self.after_change();
        r
    }

    pub fn redo(&mut self) -> Option<String> {
        let r = self.history.redo(&mut self.doc, &mut self.selection, &mut self.page, &mut self.rot_center);
        // a PowerClip being edited that the step took away is no longer edited
        self.focus.retain(|id| self.doc.find(*id).is_some());
        self.text_edit = None;
        self.node_sel.clear();
        self.seg_sel = None;
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

    /// Sets the canvas rect for this frame. Fits the page when a fit is pending (`needs_fit`)
    /// and the canvas is at least `min` points each way, and fits it again when the canvas
    /// changes size while the view is still the automatic fit.
    pub fn set_canvas(&mut self, canvas: ERect, min: f32) {
        let resized = self.canvas_rect.size() != canvas.size();
        self.canvas_rect = canvas;
        let untouched = self.auto_fit.is_some_and(|(z, o)| z == self.zoom && o == self.origin);
        if !untouched {
            self.auto_fit = None;
        }
        if canvas.width() <= min || canvas.height() <= min {
            return;
        }
        if self.needs_fit || (resized && untouched) {
            self.zoom_page();
            self.needs_fit = false;
            self.auto_fit = Some((self.zoom, self.origin));
        }
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

    /// The PowerClip whose contents are being edited (Edit PowerClip, Ctrl+click or a
    /// double-click on it): the one object in focus when it is a PowerClip, outside Edit Master
    /// and rollover editing (which use the focus for their own objects).
    pub fn editing_powerclip(&self) -> Option<ObjId> {
        match self.focus.as_slice() {
            [id] if self.symbol_edit.is_none() && self.rollover_edit.is_none() => {
                self.doc.find(*id).is_some_and(|o| matches!(o.kind, vector_core::ObjKind::PowerClip { .. })).then_some(*id)
            }
            _ => None,
        }
    }

    /// What the rotate handles turn the selection around: where its rotation center was moved
    /// to, or the middle of the selection.
    pub fn rotation_center(&self) -> Option<Point> {
        let bb = self.selection_bounds()?;
        Some(match &self.rot_center {
            Some((ids, p)) if *ids == self.selection => *p,
            _ => bb.center(),
        })
    }

    /// Transform the selection with `m` as one undo step; a moved rotation center goes along.
    pub fn transform_selection(&mut self, label: &str, m: Affine) {
        let ids = self.selection.clone();
        if ids.is_empty() {
            return;
        }
        self.modify(label, |doc, _| doc.transform_objects(&ids, m));
        if let Some((cids, c)) = &mut self.rot_center {
            if *cids == ids {
                *c = m * *c;
            }
        }
    }

    /// A command replaced the selection `old` (with bounds `old_bb`) by new objects, copies of it
    /// (Duplicate, Step and Repeat): a rotation center moved for `old` goes along to them.
    pub fn carry_rot_center(&mut self, old: &[ObjId], old_bb: Rect) {
        match &self.rot_center {
            Some((ids, c)) if ids.as_slice() == old && self.selection.as_slice() != old => {
                let c = *c;
                self.place_rot_center(old_bb, c);
            }
            _ => {}
        }
    }

    /// Give the selection the rotation center `c` had for objects with bounds `old_bb`: at the
    /// same place relative to the selection's bounds (moved by the copy's offset).
    pub fn place_rot_center(&mut self, old_bb: Rect, c: Point) {
        let Some(nb) = self.selection_bounds() else { return };
        let map = |c: f64, o0: f64, o1: f64, n0: f64, n1: f64| {
            if (o1 - o0).abs() > 1e-9 {
                n0 + (c - o0) / (o1 - o0) * (n1 - n0)
            } else {
                c + (n0 + n1) / 2.0 - (o0 + o1) / 2.0
            }
        };
        let p = Point::new(map(c.x, old_bb.x0, old_bb.x1, nb.x0, nb.x1), map(c.y, old_bb.y0, old_bb.y1, nb.y0, nb.y1));
        self.rot_center = Some((self.selection.clone(), p));
    }

    /// The nearest point of the selection's own geometry within the snap radius of `p`: the nodes
    /// of its outlines and the middles of their segments, and the corners, midpoints and center
    /// of the whole selection (issue 126: where the rotation center snaps besides the usual
    /// snap points, which leave the selection out).
    pub fn rot_center_snap(&self, s: &crate::settings::Settings, p: Point) -> Option<(Point, &'static str)> {
        use kurbo::ParamCurve;
        let ds = &self.doc.settings;
        if ds.snap_off || !(ds.snap_to_objects && s.snap_to_objects) {
            return None;
        }
        let r = self.doc_len(s.snap_radius.max(2.0) as f32);
        let mut best: Option<(f64, Point, &'static str)> = None;
        let mut consider = |q: Point, label: &'static str| {
            let dist = (q - p).hypot();
            if dist <= r && best.is_none_or(|b| dist < b.0) {
                best = Some((dist, q, label));
            }
        };
        if let Some(b) = self.selection_bounds() {
            for q in [Point::new(b.x0, b.y0), Point::new(b.x1, b.y0), Point::new(b.x1, b.y1), Point::new(b.x0, b.y1)] {
                consider(q, "Corner");
            }
            consider(b.center(), "Center");
            for q in [Point::new(b.center().x, b.y0), Point::new(b.x1, b.center().y), Point::new(b.center().x, b.y1), Point::new(b.x0, b.center().y)] {
                consider(q, "Midpoint");
            }
        }
        let mut budget = 4000usize;
        for id in &self.selection {
            let Some(o) = self.doc.find(*id) else { continue };
            if !self.doc.bounds_of(o).inflate(r, r).contains(p) {
                continue;
            }
            for seg in o.world_path().segments() {
                if budget == 0 {
                    break;
                }
                budget -= 1;
                consider(seg.start(), "Node");
                consider(seg.end(), "Node");
                consider(seg.eval(0.5), "Midpoint");
            }
        }
        best.map(|(_, q, l)| (q, l))
    }

    pub fn page_ref(&self) -> &vector_core::Page {
        &self.doc.pages[self.page]
    }

    /// The layer new objects go to (unlocked, visible).
    pub fn active_layer_mut(&mut self) -> &mut vector_core::Layer {
        let (pi, li) = self.active_loc();
        let p = &mut self.doc.pages[pi];
        let idx = li.min(p.layers.len() - 1);
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

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::object::{CornerType, DropShadow, ObjKind, Object, RectShape};

    /// A document with one 200 x 100 rectangle at (100, 100), selected.
    fn one_rect() -> (DocView, ObjId) {
        let mut d = DocView::new(Document::new(800.0, 600.0), "t".into(), 50);
        let id = d.doc.new_id();
        let r = Object::new(id, ObjKind::Rect(RectShape { w: 200.0, h: 100.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((100.0, 100.0)), Default::default());
        d.add_objects("add", vec![r]);
        (d, id)
    }

    fn opacity(d: &DocView, id: ObjId) -> Option<f64> {
        d.doc.find(id).unwrap().effects.drop_shadow.as_ref().map(|s| s.opacity)
    }

    // issue 107: the frames of one slider drag are one undo step; the next drag is another one
    #[test]
    fn a_slider_drag_is_one_undo_step() {
        let (mut d, id) = one_rect();
        let steps = d.history.undo.len();
        for v in [10.0, 20.0, 30.0] {
            d.modify_live(7, "Shadow", |doc, _| doc.find_mut(id).unwrap().effects.drop_shadow = Some(DropShadow { opacity: v, ..Default::default() }));
        }
        assert_eq!(d.history.undo.len(), steps + 1, "one step for the whole drag");
        assert_eq!(opacity(&d, id), Some(30.0));
        // another drag (a new key) is a step of its own
        d.modify_live(8, "Shadow", |doc, _| doc.find_mut(id).unwrap().effects.drop_shadow.as_mut().unwrap().opacity = 60.0);
        assert_eq!(d.history.undo.len(), steps + 2);
        d.undo();
        assert_eq!(opacity(&d, id), Some(30.0));
        d.undo();
        assert_eq!(opacity(&d, id), None, "undoing the first drag goes back to before it");
        // a change in between ends a drag, even with the same key
        d.modify_live(9, "Shadow", |doc, _| doc.find_mut(id).unwrap().effects.drop_shadow = Some(Default::default()));
        d.modify("Other", |_, _| {});
        d.modify_live(9, "Shadow", |doc, _| doc.find_mut(id).unwrap().effects.drop_shadow.as_mut().unwrap().opacity = 5.0);
        assert_eq!(d.history.undo.len(), steps + 3);
    }

    // issue 126: a moved rotation center goes along to the copies, at the same place
    #[test]
    fn the_rotation_center_goes_along_to_copies() {
        let (mut d, id) = one_rect();
        let c = Point::new(100.0, 100.0);
        d.rot_center = Some((vec![id], c));
        let bb = d.selection_bounds().unwrap();
        let old = d.selection.clone();
        d.modify("Duplicate", |doc, sel| *sel = vector_core::ops::duplicate(doc, &old, kurbo::Vec2::new(18.0, 18.0)));
        d.carry_rot_center(&old, bb);
        assert_ne!(d.selection, old);
        assert_eq!(d.rotation_center(), Some(Point::new(118.0, 118.0)));
        // undo brings back the original with its center, redo the copy with its own
        d.undo();
        assert_eq!(d.selection, old);
        assert_eq!(d.rotation_center(), Some(c));
        d.redo();
        assert_eq!(d.rotation_center(), Some(Point::new(118.0, 118.0)));
        // a selection of several copies: the same place relative to all of them
        let one = d.selection.clone();
        let bb = d.selection_bounds().unwrap();
        d.modify("Step and Repeat", |doc, sel| *sel = vector_core::ops::step_and_repeat(doc, &one, 3, kurbo::Vec2::new(0.0, 200.0)));
        d.carry_rot_center(&one, bb);
        let nb = d.selection_bounds().unwrap();
        assert_eq!(d.rotation_center(), Some(Point::new(nb.x0, nb.y0)));
        // a center for other objects stays as it is
        let before = d.rot_center.clone();
        d.carry_rot_center(&[id], bb);
        assert_eq!(d.rot_center, before);
    }

    // issue 126: the center snaps to the selection's own corners, midpoints, nodes and middle
    #[test]
    fn the_rotation_center_snaps_to_its_own_object() {
        let (mut d, _) = one_rect();
        let s = crate::settings::Settings::default();
        let snap = |d: &DocView, x: f64, y: f64| d.rot_center_snap(&s, Point::new(x, y));
        assert_eq!(snap(&d, 103.0, 98.0), Some((Point::new(100.0, 100.0), "Corner")));
        assert_eq!(snap(&d, 197.0, 103.0), Some((Point::new(200.0, 100.0), "Midpoint")));
        assert_eq!(snap(&d, 204.0, 146.0), Some((Point::new(200.0, 150.0), "Center")));
        assert_eq!(snap(&d, 250.0, 130.0), None, "nothing near");
        // a turned rectangle: its own corners are nodes away from the bounding box
        let ids = d.selection.clone();
        let turn = Affine::rotate_about(0.5, Point::new(200.0, 150.0));
        d.modify("Rotate", |doc, _| doc.transform_objects(&ids, turn));
        let corner = turn * Point::new(300.0, 200.0);
        let (q, l) = snap(&d, corner.x + 2.0, corner.y - 2.0).unwrap();
        assert!((q - corner).hypot() < 1e-6 && l == "Node", "{q:?} {l}");
        // snapping off: no snap
        d.doc.settings.snap_off = true;
        assert_eq!(snap(&d, 103.0, 98.0), None);
    }
}
