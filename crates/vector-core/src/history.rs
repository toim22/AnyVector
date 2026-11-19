//! Undo/redo by document snapshots. Image pixel data is shared (`Arc`) so snapshots stay cheap.

use crate::document::Document;
use crate::object::ObjId;

#[derive(Clone)]
pub struct Snapshot {
    pub label: String,
    pub doc: Document,
    pub selection: Vec<ObjId>,
    pub page: usize,
    /// A rotation center moved away from the middle of the selection: for these objects, here.
    /// Kept with the step so undo and redo bring it back with the objects.
    pub pivot: Pivot,
}

/// A moved rotation center: the objects it was moved for and where it is.
pub type Pivot = Option<(Vec<ObjId>, kurbo::Point)>;

#[derive(Clone, Default)]
pub struct History {
    pub undo: Vec<Snapshot>,
    pub redo: Vec<Snapshot>,
    pub limit: usize,
    /// Increments on every change; used for "modified" tracking and render invalidation.
    pub revision: u64,
    pub saved_revision: u64,
}

impl History {
    pub fn new(limit: usize) -> History {
        History { undo: vec![], redo: vec![], limit: limit.max(1), revision: 0, saved_revision: 0 }
    }

    /// Record the state *before* a change.
    pub fn push(&mut self, label: &str, doc: &Document, selection: &[ObjId], page: usize, pivot: &Pivot) {
        self.undo.push(Snapshot { label: label.to_string(), doc: doc.clone(), selection: selection.to_vec(), page, pivot: pivot.clone() });
        if self.undo.len() > self.limit {
            let excess = self.undo.len() - self.limit;
            self.undo.drain(0..excess);
        }
        self.redo.clear();
        self.revision += 1;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, doc: &mut Document, selection: &mut Vec<ObjId>, page: &mut usize, pivot: &mut Pivot) -> Option<String> {
        let s = self.undo.pop()?;
        self.redo.push(Snapshot { label: s.label.clone(), doc: doc.clone(), selection: selection.clone(), page: *page, pivot: pivot.take() });
        *doc = s.doc;
        *selection = s.selection;
        *pivot = s.pivot;
        *page = s.page.min(doc.pages.len().saturating_sub(1));
        self.revision += 1;
        Some(s.label)
    }

    pub fn redo(&mut self, doc: &mut Document, selection: &mut Vec<ObjId>, page: &mut usize, pivot: &mut Pivot) -> Option<String> {
        let s = self.redo.pop()?;
        self.undo.push(Snapshot { label: s.label.clone(), doc: doc.clone(), selection: selection.clone(), page: *page, pivot: pivot.take() });
        *doc = s.doc;
        *selection = s.selection;
        *pivot = s.pivot;
        *page = s.page.min(doc.pages.len().saturating_sub(1));
        self.revision += 1;
        Some(s.label)
    }

    pub fn undo_labels(&self) -> Vec<String> {
        self.undo.iter().rev().map(|s| s.label.clone()).collect()
    }
    pub fn redo_labels(&self) -> Vec<String> {
        self.redo.iter().rev().map(|s| s.label.clone()).collect()
    }

    pub fn is_modified(&self) -> bool {
        self.revision != self.saved_revision
    }
    pub fn mark_saved(&mut self) {
        self.saved_revision = self.revision;
    }
    /// Bump the revision without recording an undo step (view-only changes that still need a redraw).
    pub fn touch(&mut self) {
        self.revision += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Point;

    // issue 126: undo and redo bring back the rotation center the step had
    #[test]
    fn undo_and_redo_keep_the_pivot() {
        let mut h = History::new(10);
        let mut doc = Document::new(100.0, 100.0);
        let (mut sel, mut page) = (vec![], 0);
        let before: Pivot = Some((vec![1], Point::new(1.0, 2.0)));
        h.push("Move", &doc, &sel, page, &before);
        let mut pivot: Pivot = Some((vec![1], Point::new(11.0, 12.0)));
        h.undo(&mut doc, &mut sel, &mut page, &mut pivot);
        assert_eq!(pivot, before);
        h.redo(&mut doc, &mut sel, &mut page, &mut pivot);
        assert_eq!(pivot, Some((vec![1], Point::new(11.0, 12.0))));
    }
}
