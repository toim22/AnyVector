//! Undo/redo by document snapshots. Image pixel data is shared (`Arc`) so snapshots stay cheap.

use crate::document::Document;
use crate::object::ObjId;

#[derive(Clone)]
pub struct Snapshot {
    pub label: String,
    pub doc: Document,
    pub selection: Vec<ObjId>,
    pub page: usize,
}

#[derive(Default)]
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
    pub fn push(&mut self, label: &str, doc: &Document, selection: &[ObjId], page: usize) {
        self.undo.push(Snapshot { label: label.to_string(), doc: doc.clone(), selection: selection.to_vec(), page });
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

    pub fn undo(&mut self, doc: &mut Document, selection: &mut Vec<ObjId>, page: &mut usize) -> Option<String> {
        let s = self.undo.pop()?;
        self.redo.push(Snapshot { label: s.label.clone(), doc: doc.clone(), selection: selection.clone(), page: *page });
        *doc = s.doc;
        *selection = s.selection;
        *page = s.page.min(doc.pages.len().saturating_sub(1));
        self.revision += 1;
        Some(s.label)
    }

    pub fn redo(&mut self, doc: &mut Document, selection: &mut Vec<ObjId>, page: &mut usize) -> Option<String> {
        let s = self.redo.pop()?;
        self.undo.push(Snapshot { label: s.label.clone(), doc: doc.clone(), selection: selection.clone(), page: *page });
        *doc = s.doc;
        *selection = s.selection;
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
