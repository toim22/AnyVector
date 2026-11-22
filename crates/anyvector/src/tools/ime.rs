//! Input methods (IME: Japanese, Chinese, Korean, Vietnamese Telex...) for the text typed on the
//! canvas: in a text object being edited, at a Text tool click before the object exists, and in
//! the chosen table cell.
//!
//! egui's text fields take part in the IME protocol themselves; the canvas editor does it here:
//! - every frame something on the canvas takes typing, [`publish`] puts an `IMEOutput` in the
//!   platform output. That is what switches the window's IME on (egui-winit allows the IME only
//!   while the output carries one) and it places the candidate window at the caret.
//! - [`event`] turns `Event::Ime` into the composition (pre-edit) shown at the caret and, on
//!   commit, into the text to insert, which goes through the usual typing path (one undo step,
//!   QuickCorrect, a pending text position becoming a text object).
//! - while composing, the keys and text events belong to the IME ([`swallows`]) and shortcuts
//!   do not run ([`holds_keys`]).
//!
//! The composition is not document content: it lives here, is drawn from a copy of the document
//! with the composition put in ([`Preview`]), and makes no undo step. When the composition stops
//! fitting what is being edited (the caret was clicked elsewhere, the tool or the document
//! changed, a field or a dialog took the keyboard), it is kept where it was typed, as Windows
//! programs do with an unfinished composition, and the system's composition is interrupted.

use crate::app::AnyVectorApp;
use crate::docview::DocView;
use kurbo::{Affine, Point, Rect};
use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use vector_core::text::{TextAlign, TextObj};
use vector_core::{Document, ObjId, ObjKind};

/// The text an input method is composing, shown at the caret and not yet in the document.
#[derive(Clone, Debug, PartialEq)]
pub struct Preedit {
    pub text: String,
    /// Characters of `text` the input method marks: a non-empty range is the clause being
    /// converted, an empty one the caret inside the composition.
    pub active: Option<Range<usize>>,
}

/// What typing goes into.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// The text object being edited, with its caret and selection anchor (in characters).
    Text { doc: u64, obj: ObjId, caret: usize, anchor: usize },
    /// A Text tool click (and paragraph frame) with no text object yet.
    Pending { doc: u64, at: Point, frame: Option<Rect> },
    /// The chosen cell of the selected table.
    Cell { doc: u64, table: ObjId, cell: (usize, usize) },
}

impl Target {
    fn doc(&self) -> u64 {
        match *self {
            Target::Text { doc, .. } | Target::Pending { doc, .. } | Target::Cell { doc, .. } => doc,
        }
    }
}

/// What a handler has to do for an IME event.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Insert committed text, as typed.
    Insert(String),
    /// Delete characters before and after the caret (counted in characters).
    Delete { before: usize, after: usize },
}

/// The document as it looks with the composition put in, for drawing it in place.
pub struct Preview {
    key: u64,
    doc_uid: u64,
    pub doc: Document,
    /// The object showing the composition (a table for a cell, a new text object for a pending
    /// position), the cell, and where the composition is in that text (characters).
    pub obj: ObjId,
    pub cell: Option<(usize, usize)>,
    pub start: usize,
    pub len: usize,
}

/// The input method state of the canvas editor (one composition at a time).
#[derive(Default)]
pub struct ImeState {
    preedit: Option<Preedit>,
    target: Option<Target>,
    /// The document's revision when the composition started: a composition left behind by an
    /// undo or another change of the document is dropped instead of kept.
    revision: u64,
    /// Ask the system to end its composition with the next IME output.
    interrupt: bool,
    /// The last frame published an IME output (the window's IME was on).
    published: bool,
    /// Text the last commit inserted and when: integrations that also send it as a text event
    /// must not insert it twice.
    committed: Option<(String, f64)>,
    preview: Option<Preview>,
}

impl ImeState {
    /// A composition is in progress.
    pub fn composing(&self) -> bool {
        self.preedit.is_some()
    }

    pub fn preedit(&self) -> Option<&Preedit> {
        self.preedit.as_ref()
    }

    /// The document with the composition in it, while composing in document `uid`.
    pub fn preview_for(&self, uid: u64) -> Option<&Preview> {
        self.preview.as_ref().filter(|p| p.doc_uid == uid && self.preedit.is_some())
    }

    /// Changes whenever the composition shown in document `uid` changes (for the render cache).
    pub fn preview_key(&self, uid: u64) -> Option<u64> {
        self.preview_for(uid).map(|p| p.key)
    }

    fn clear(&mut self) {
        self.preedit = None;
        self.target = None;
        self.preview = None;
    }
}

/// What typing goes into now, if anything.
pub fn current_target(app: &AnyVectorApp) -> Option<Target> {
    let d = app.doc()?;
    if let Some(te) = &d.text_edit {
        return Some(Target::Text { doc: d.uid, obj: te.obj, caret: te.caret, anchor: te.anchor });
    }
    if let Some((at, frame)) = app.ts.pending_text {
        return Some(Target::Pending { doc: d.uid, at, frame });
    }
    if super::table_typing(app) {
        return Some(Target::Cell { doc: d.uid, table: *d.selection.first()?, cell: app.ts.table_cell? });
    }
    None
}

/// Start of a frame (before shortcuts run): a composition whose IME was switched off last frame
/// (nothing on the canvas took typing), or that no longer fits what is edited, ends.
pub fn begin_frame(app: &mut AnyVectorApp) {
    let published = std::mem::take(&mut app.ts.ime.published);
    if !published {
        // the window's IME is off: there is no system composition left to interrupt
        app.ts.ime.interrupt = false;
        if app.ts.ime.composing() {
            end_stale(app);
            app.ts.ime.interrupt = false;
        }
        return;
    }
    sync(app);
}

/// End a composition that no longer fits what is being edited (run before handling typing).
pub fn sync(app: &mut AnyVectorApp) {
    if !app.ts.ime.composing() {
        return;
    }
    let now = current_target(app);
    if now.is_some() && now == app.ts.ime.target && !app.dialogs.any_modal_open() {
        return;
    }
    end_stale(app);
}

/// The composition stopped fitting: keep its text where it was typed (when that is still there
/// and the document did not change meanwhile) and interrupt the system's composition.
fn end_stale(app: &mut AnyVectorApp) {
    let ime = &mut app.ts.ime;
    let (Some(p), target) = (ime.preedit.take(), ime.target.take()) else {
        ime.clear();
        return;
    };
    ime.preview = None;
    ime.interrupt = true;
    let revision = ime.revision;
    let Some(target) = target else { return };
    if p.text.is_empty() {
        return;
    }
    let now = app.time();
    let Some(d) = app.docs.iter_mut().find(|d| d.uid == target.doc()) else { return };
    if d.history.revision != revision {
        return;
    }
    let kept = match target {
        Target::Text { obj, caret, anchor, .. } => keep_in_text(d, obj, caret, anchor, &p.text),
        Target::Pending { at, frame, .. } => {
            let id = d.doc.new_id();
            let o = super::interact::new_text_object(&app.ts, id, at, frame, &p.text);
            let selection = d.selection.clone();
            d.add_objects("Create Text", vec![o]);
            d.selection = selection;
            true
        }
        Target::Cell { table, cell, .. } => {
            d.checkpoint("Edit Table Text");
            let ok = append_to_cell(&mut d.doc, table, cell, &p.text);
            d.history.touch();
            ok
        }
    };
    if kept {
        app.ts.ime.committed = Some((p.text, now));
    }
}

/// Put `s` in place of the selection `caret`..`anchor` of text object `obj` (one undo step);
/// the caret of the text being edited now moves along when it is in the same object.
fn keep_in_text(d: &mut DocView, obj: ObjId, caret: usize, anchor: usize, s: &str) -> bool {
    let Some(ObjKind::Text(t)) = d.doc.find(obj).map(|o| &o.kind) else { return false };
    let len = t.char_count();
    let (a, b) = (caret.min(anchor).min(len), caret.max(anchor).min(len));
    d.checkpoint("Edit Text");
    crate::dialogs::text::edit_chain(&mut d.doc, obj, a, b, s);
    d.history.touch();
    let n = s.chars().count();
    if let Some(te) = d.text_edit.as_mut().filter(|te| te.obj == obj) {
        let shift = |x: usize| if x >= b { x - (b - a) + n } else if x > a { a + n } else { x };
        te.caret = shift(te.caret);
        te.anchor = shift(te.anchor);
    }
    true
}

fn append_to_cell(doc: &mut Document, table: ObjId, (r, c): (usize, usize), s: &str) -> bool {
    let Some(ObjKind::Table(t)) = doc.find_mut(table).map(|o| &mut o.kind) else { return false };
    if r >= t.rows || c >= t.cols {
        return false;
    }
    t.cell_mut(r, c).text.push_str(s);
    if t.auto_resize {
        t.fit_rows();
    }
    true
}

/// An IME event for the canvas editor; what the caller has to do to the text, if anything.
pub fn event(app: &mut AnyVectorApp, e: &egui::ImeEvent, now: f64) -> Option<Action> {
    match e {
        egui::ImeEvent::Preedit { text, active_range_chars } => {
            if text.is_empty() {
                // the composition was dismissed (cancelled, or emptied with Backspace)
                app.ts.ime.clear();
                return None;
            }
            if text == "\n" || text == "\r" {
                return None;
            }
            let target = current_target(app)?;
            let revision = app.docs.iter().find(|d| d.uid == target.doc()).map(|d| d.history.revision).unwrap_or(0);
            let n = text.chars().count();
            let active = active_range_chars.clone().map(|r| r.start.min(n)..r.end.min(n)).filter(|r| r.start <= r.end);
            let ime = &mut app.ts.ime;
            if !ime.composing() {
                ime.target = Some(target);
                ime.revision = revision;
            }
            ime.preedit = Some(Preedit { text: text.clone(), active });
            ime.preview = None;
            ime.committed = None;
            None
        }
        egui::ImeEvent::Commit(text) => {
            app.ts.ime.clear();
            let text = text.replace("\r\n", "\n");
            // Enter arrives as a key; an empty commit only ends the composition
            if text.is_empty() || text == "\n" || text == "\r" {
                return None;
            }
            app.ts.ime.committed = Some((text.clone(), now));
            Some(Action::Insert(text))
        }
        egui::ImeEvent::DeleteSurrounding { before_chars, after_chars } => {
            (*before_chars > 0 || *after_chars > 0).then_some(Action::Delete { before: *before_chars, after: *after_chars })
        }
        _ => None,
    }
}

/// After the text changed under a composition (an IME deletion around the caret): the
/// composition goes on at the new caret.
pub fn retarget(app: &mut AnyVectorApp) {
    if !app.ts.ime.composing() {
        return;
    }
    let target = current_target(app);
    let revision = target.and_then(|t| app.docs.iter().find(|d| d.uid == t.doc())).map(|d| d.history.revision).unwrap_or(0);
    let ime = &mut app.ts.ime;
    ime.target = target;
    ime.revision = revision;
    ime.preview = None;
}

/// Events the composition takes away from the editor: while composing every key and text event
/// belongs to the input method (Escape cancels the composition, not the editing); a text event
/// repeating what a commit just inserted is the same input.
pub fn swallows(app: &mut AnyVectorApp, ev: &egui::Event, now: f64) -> bool {
    let ime = &mut app.ts.ime;
    if ime.composing() {
        return match ev {
            egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                ime.clear();
                ime.interrupt = true;
                true
            }
            egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::Paste(_) | egui::Event::Copy | egui::Event::Cut => true,
            _ => false,
        };
    }
    if let egui::Event::Text(t) = ev {
        if ime.committed.as_ref().is_some_and(|(s, at)| s == t && now - at < 0.1) {
            ime.committed = None;
            return true;
        }
    }
    false
}

/// Shortcuts must not run: a composition is in progress or starts with this frame's input.
pub fn holds_keys(app: &AnyVectorApp, ctx: &egui::Context) -> bool {
    app.ts.ime.composing() || ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) if !text.is_empty())))
}

/// Switch the window's IME on for what takes typing on the canvas and put its candidate window
/// at the caret (run after the typing of the frame was handled).
pub fn publish(app: &mut AnyVectorApp, ctx: &egui::Context) {
    // a text field or a dialog has the keyboard: it publishes its own
    if app.dialogs.any_modal_open() || ctx.egui_wants_keyboard_input() {
        return;
    }
    if current_target(app).is_none() {
        return;
    }
    let Some(ai) = app.active else { return };
    let Some(d) = app.docs.get(ai) else { return };
    update_preview(&mut app.ts, d);
    let Some((rect, cursor_rect)) = caret_rect(d, &app.ts) else { return };
    let interrupt = std::mem::take(&mut app.ts.ime.interrupt);
    ctx.output_mut(|o| {
        if o.ime.is_none() {
            o.ime = Some(egui::output::IMEOutput { purpose: egui::IMEPurpose::Normal, rect, cursor_rect, should_interrupt_composition: interrupt });
        }
    });
    app.ts.ime.published = true;
}

/// Build the document with the composition in it when the composition changed.
fn update_preview(ts: &mut super::ToolState, d: &DocView) {
    let (Some(p), Some(target)) = (ts.ime.preedit.clone(), ts.ime.target) else {
        ts.ime.preview = None;
        return;
    };
    if target.doc() != d.uid {
        ts.ime.preview = None;
        return;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    d.uid.hash(&mut h);
    d.history.revision.hash(&mut h);
    p.text.hash(&mut h);
    format!("{target:?}").hash(&mut h);
    let key = h.finish();
    if ts.ime.preview.as_ref().is_some_and(|v| v.key == key) {
        return;
    }
    let n = p.text.chars().count();
    let mut doc = d.doc.clone();
    let shown = match target {
        Target::Text { obj, caret, anchor, .. } => match doc.find(obj).map(|o| &o.kind) {
            Some(ObjKind::Text(t)) => {
                let len = t.char_count();
                let (a, b) = (caret.min(anchor).min(len), caret.max(anchor).min(len));
                crate::dialogs::text::edit_chain(&mut doc, obj, a, b, &p.text);
                Some((obj, None, a))
            }
            _ => None,
        },
        Target::Pending { at, frame, .. } => {
            let id = doc.new_id();
            let o = super::interact::new_text_object(ts, id, at, frame, &p.text);
            // on the active layer, as typing will put it
            let (pi, li) = d.active_loc();
            let layer = doc.pages.get_mut(pi).and_then(|pg| {
                let i = li.min(pg.layers.len().checked_sub(1)?);
                pg.layers.get_mut(i)
            });
            layer.map(|l| {
                l.objects.push(o);
                (id, None, 0)
            })
        }
        Target::Cell { table, cell, .. } => {
            let start = match doc.find(table).map(|o| &o.kind) {
                Some(ObjKind::Table(t)) if cell.0 < t.rows && cell.1 < t.cols => t.cell(cell.0, cell.1).text.chars().count(),
                _ => 0,
            };
            append_to_cell(&mut doc, table, cell, &p.text).then_some((table, Some(cell), start))
        }
    };
    ts.ime.preview = shown.map(|(obj, cell, start)| Preview { key, doc_uid: d.uid, doc, obj, cell, start, len: n });
}

/// The laid-out text of `obj` (or of its `cell`) in `doc`, and where it sits in the document.
pub fn text_frame(doc: &Document, obj: ObjId, cell: Option<(usize, usize)>) -> Option<(Cow<'_, TextObj>, Affine)> {
    let o = doc.find(obj)?;
    match (&o.kind, cell) {
        (ObjKind::Text(t), None) => Some((Cow::Borrowed(t), o.transform)),
        (ObjKind::Table(t), Some((r, c))) if r < t.rows && c < t.cols => {
            // as the renderer lays out cell text
            let tr = t.text_rect(r, c);
            let mut tobj = TextObj::paragraph(&t.cell(r, c).text, tr.width().max(1.0), tr.height().max(1.0), t.text_format.clone());
            tobj.align = TextAlign::Left;
            Some((Cow::Owned(tobj), o.transform * Affine::translate((tr.x0, tr.y0))))
        }
        _ => None,
    }
}

/// The caret box before character `i` of `t` (laid out as `carets`), in document points:
/// (top, bottom) of the caret line.
pub fn caret_line(t: &TextObj, carets: &[(f64, f64, usize)], tr: Affine, i: usize) -> Option<(Point, Point)> {
    let c = carets.get(i.min(carets.len().checked_sub(1)?))?;
    let size = t.format_at(i.saturating_sub(1)).size;
    Some((tr * Point::new(c.0, c.1 - size * 0.85), tr * Point::new(c.0, c.1 + size * 0.2)))
}

fn screen_box(d: &DocView, pts: &[Point]) -> Option<egui::Rect> {
    let mut it = pts.iter().map(|p| d.to_screen(*p));
    let first = it.next()?;
    let r = it.fold(egui::Rect::from_min_max(first, first), |r, p| r.union(egui::Rect::from_min_max(p, p)));
    Some(r.expand2(egui::vec2(1.0, 2.0)))
}

/// Where the candidate window goes, in screen points: (the text's box, the caret). While
/// composing the caret is the clause being converted (or the caret inside the composition), so
/// the candidates open next to it and never over it.
pub fn caret_rect(d: &DocView, ts: &super::ToolState) -> Option<(egui::Rect, egui::Rect)> {
    if let Some(pv) = ts.ime.preview_for(d.uid) {
        let (t, tr) = text_frame(&pv.doc, pv.obj, pv.cell)?;
        let lay = vector_core::text::layout(&t);
        let p = ts.ime.preedit()?;
        let (from, to) = match &p.active {
            Some(r) if !r.is_empty() => (pv.start + r.start, pv.start + r.end),
            Some(r) => (pv.start + r.start, pv.start + r.start),
            None => (pv.start + pv.len, pv.start + pv.len),
        };
        let mut pts = vec![];
        for i in from..=to {
            if let Some((a, b)) = caret_line(&t, &lay.carets, tr, i) {
                pts.extend([a, b]);
            }
        }
        let cursor = screen_box(d, &pts)?;
        let whole = screen_box(d, &[tr * Point::new(lay.bounds.x0, lay.bounds.y0), tr * Point::new(lay.bounds.x1, lay.bounds.y0), tr * Point::new(lay.bounds.x1, lay.bounds.y1), tr * Point::new(lay.bounds.x0, lay.bounds.y1)]).unwrap_or(cursor);
        return Some((whole.union(cursor), cursor));
    }
    if let Some(te) = &d.text_edit {
        let (t, tr) = text_frame(&d.doc, te.obj, None)?;
        let lay = vector_core::text::layout(&t);
        let (a, b) = caret_line(&t, &lay.carets, tr, te.caret)?;
        let cursor = screen_box(d, &[a, b])?;
        let whole = d.to_screen_rect(d.doc.find(te.obj)?.bounds());
        return Some((whole.union(cursor), cursor));
    }
    if let Some((p, frame)) = ts.pending_text {
        let size = ts.text_format.size;
        let (top, bottom) = match frame {
            Some(_) => (p, p + kurbo::Vec2::new(0.0, size * 1.2)),
            None => (p - kurbo::Vec2::new(0.0, size * 0.8), p + kurbo::Vec2::new(0.0, size * 0.2)),
        };
        let cursor = screen_box(d, &[top, bottom])?;
        let whole = frame.map(|r| d.to_screen_rect(r)).unwrap_or(cursor);
        return Some((whole.union(cursor), cursor));
    }
    // the chosen table cell: typing goes to the end of its text
    let (r, c) = ts.table_cell?;
    let id = *d.selection.first()?;
    let o = d.doc.find(id)?;
    let ObjKind::Table(tab) = &o.kind else { return None };
    let (t, tr) = text_frame(&d.doc, id, Some((r, c)))?;
    let cell = d.to_screen_rect(o.transform.transform_rect_bbox(tab.cell_rect(r, c)));
    let cursor = if t.content.is_empty() {
        let size = tab.text_format.size;
        screen_box(d, &[tr * Point::new(0.0, 0.0), tr * Point::new(0.0, size * 1.05)])?
    } else {
        let lay = vector_core::text::layout(&t);
        let (a, b) = caret_line(&t, &lay.carets, tr, t.char_count())?;
        screen_box(d, &[a, b])?
    };
    Some((cell.union(cursor), cursor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_doc_is_the_documents_uid() {
        let t = Target::Text { doc: 7, obj: 1, caret: 0, anchor: 0 };
        assert_eq!(t.doc(), 7);
        assert_eq!(Target::Pending { doc: 3, at: Point::ZERO, frame: None }.doc(), 3);
        assert_eq!(Target::Cell { doc: 5, table: 2, cell: (0, 0) }.doc(), 5);
    }

    #[test]
    fn keeping_a_composition_moves_the_caret_after_it() {
        use vector_core::text::CharFormat;
        let mut d = DocView::new(Document::new(800.0, 600.0), "t".into(), 50);
        let id = d.doc.new_id();
        let o = vector_core::Object::new(id, ObjKind::Text(TextObj::artistic("Hello world", CharFormat::default())), Affine::IDENTITY, vector_core::Style::default());
        d.add_objects("add", vec![o]);
        // the composition was typed after "Hello", then the caret was clicked after "world"
        d.text_edit = Some(crate::docview::TextEdit { obj: id, caret: 11, anchor: 11, blink: 0.0 });
        assert!(keep_in_text(&mut d, id, 5, 5, "日本"));
        let ObjKind::Text(t) = &d.doc.find(id).unwrap().kind else { panic!() };
        assert_eq!(t.content, "Hello日本 world");
        assert_eq!(d.text_edit.as_ref().unwrap().caret, 13);
        // a selection is replaced; a caret before it stays
        d.text_edit = Some(crate::docview::TextEdit { obj: id, caret: 0, anchor: 0, blink: 0.0 });
        assert!(keep_in_text(&mut d, id, 5, 7, "x"));
        let ObjKind::Text(t) = &d.doc.find(id).unwrap().kind else { panic!() };
        assert_eq!(t.content, "Hellox world");
        assert_eq!(d.text_edit.as_ref().unwrap().caret, 0);
        assert!(!keep_in_text(&mut d, 999, 0, 0, "x"), "no such object");
    }

    #[test]
    fn appending_to_a_cell_checks_the_cell() {
        use vector_core::object::TableObj;
        let mut doc = Document::new(800.0, 600.0);
        let id = doc.new_id();
        let t = TableObj::new(2, 2, 200.0, 100.0);
        doc.pages[0].layers[0].objects.push(vector_core::Object::new(id, ObjKind::Table(t), Affine::IDENTITY, vector_core::Style::default()));
        assert!(append_to_cell(&mut doc, id, (1, 1), "表"));
        assert!(!append_to_cell(&mut doc, id, (2, 0), "x"), "no such row");
        let Some(ObjKind::Table(t)) = doc.find(id).map(|o| &o.kind) else { panic!() };
        assert_eq!(t.cell(1, 1).text, "表");
    }
}
