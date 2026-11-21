//! What the Shape tool can grab (#104, #108, #122, #123): the control points it shows, the nodes
//! of curves, rectangle corners, polygon and ellipse nodes, curve segments, and the spacing
//! handles of text (#110). One hit test for the press, the cursor and the overlay, so they always
//! agree.

use super::*;

/// The thing under the pointer, in the order the Shape tool tries them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeHit {
    /// A control point shown for the selected nodes or segment.
    Control { obj: ObjId, si: usize, ni: usize, incoming: bool },
    Node { obj: ObjId, si: usize, ni: usize },
    /// A rectangle corner (rounding).
    Corner { obj: ObjId, corner: usize },
    /// An outer (corner) or inner node of a polygon or star.
    PolygonNode { obj: ObjId, inner: bool, index: usize },
    /// The start (`true`) or end node of an ellipse, pie or arc.
    EllipseNode { obj: ObjId, start: bool },
    /// A point on segment `seg` of a selected curve, at parameter `t`.
    Segment { obj: ObjId, si: usize, seg: usize, t: f64 },
    /// A spacing handle of a selected text (#110): the vertical one (line spacing) or the
    /// horizontal one (character spacing).
    TextSpacing { obj: ObjId, vertical: bool },
}

/// What a text spacing handle of the Shape tool changes (#110), as CorelDRAW's interactive
/// spacing arrows: the horizontal one the character spacing, with Shift the word spacing; the
/// vertical one the line spacing, with Shift the space after paragraphs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSpacing {
    Char,
    Word,
    Line,
    Para,
}

impl TextSpacing {
    /// The spacing a handle drag changes: `vertical` for the line spacing handle.
    pub fn of_handle(vertical: bool, shift: bool) -> Self {
        match (vertical, shift) {
            (false, false) => TextSpacing::Char,
            (false, true) => TextSpacing::Word,
            (true, false) => TextSpacing::Line,
            (true, true) => TextSpacing::Para,
        }
    }

    pub fn vertical(self) -> bool {
        matches!(self, TextSpacing::Line | TextSpacing::Para)
    }

    /// The undo label: the name of the field in the Text docker.
    pub fn label(self) -> &'static str {
        match self {
            TextSpacing::Char => "Character Spacing",
            TextSpacing::Word => "Word Spacing",
            TextSpacing::Line => "Line Spacing",
            TextSpacing::Para => "After Paragraph Spacing",
        }
    }

    /// The value, in %, as the Text docker shows it.
    pub fn value(self, t: &TextObj) -> f64 {
        match self {
            TextSpacing::Char => t.format.tracking,
            TextSpacing::Word => t.word_spacing,
            TextSpacing::Line => t.line_spacing,
            TextSpacing::Para => t.para_spacing,
        }
    }

    /// `orig` with this spacing changed by `delta` %, within the Text docker's limits; the
    /// character spacing of every run changes by the same amount.
    pub fn changed(self, orig: &TextObj, delta: f64) -> TextObj {
        let (lo, hi) = if self == TextSpacing::Char { (-100.0, 2000.0) } else { (0.0, 2000.0) };
        let v = |x: f64| (((x + delta) * 10.0).round() / 10.0).clamp(lo, hi);
        let mut t = orig.clone();
        match self {
            TextSpacing::Char => {
                t.format.tracking = v(orig.format.tracking);
                for s in t.spans.iter_mut() {
                    s.format.tracking = v(s.format.tracking);
                }
            }
            TextSpacing::Word => t.word_spacing = v(orig.word_spacing),
            TextSpacing::Line => t.line_spacing = v(orig.line_spacing),
            TextSpacing::Para => t.para_spacing = v(orig.para_spacing),
        }
        t
    }

    /// How far the handle moves, in the text's own units, for 1 % more of this spacing, so the
    /// handle follows the pointer: the widest line grows by the character spacing of each of its
    /// characters (a % of a quarter of their size, the width of a space) or the word spacing of
    /// each of its spaces; the last line moves down by the line spacing of each line after the
    /// first (a % of the line's height) or the space after each paragraph that ends (a % of the
    /// size).
    pub fn rate(self, id: ObjId, t: &TextObj) -> f64 {
        let l = super::overlay::cached_layout(id, t);
        let chars: Vec<char> = t.content.chars().collect();
        let control = |c: char| matches!(c, '\n' | '\t' | vector_core::text::COLUMN_BREAK | vector_core::text::SOFT_HYPHEN);
        let space = |c: char| c == ' ' || c == vector_core::text::NBSP;
        let visible: Vec<&vector_core::text::LaidLine> = l.lines.iter().filter(|x| !x.hidden).collect();
        let first_col: Vec<&vector_core::text::LaidLine> = visible.iter().copied().filter(|x| visible.first().is_some_and(|f| f.column == x.column)).collect();
        let widest = visible.iter().copied().max_by(|a, b| (a.x1 - a.x0).total_cmp(&(b.x1 - b.x0)));
        let line_chars = |ln: &vector_core::text::LaidLine| (ln.first_char.min(chars.len())..ln.end_char.min(chars.len())).map(|i| (i, chars[i]));
        let per = match self {
            TextSpacing::Char => widest.map_or(0.0, |ln| line_chars(ln).filter(|&(_, c)| !control(c)).map(|(i, _)| t.format_at(i).size * 0.25).sum()),
            TextSpacing::Word => widest.map_or(0.0, |ln| {
                line_chars(ln)
                    .filter(|&(_, c)| space(c))
                    .map(|(i, _)| {
                        // the natural width of the space, from where the caret goes past it
                        let f = t.format_at(i);
                        let gap = l.carets.get(i + 1).zip(l.carets.get(i)).map_or(0.0, |(b, a)| b.0 - a.0);
                        let w = (gap - f.tracking / 100.0 * f.size * 0.25) / (t.word_spacing / 100.0);
                        if t.word_spacing >= 1.0 && w.is_finite() && w > 0.0 && w < f.size * 2.0 && !matches!(t.align, TextAlign::Justify | TextAlign::ForceJustify) {
                            w
                        } else {
                            f.size * 0.25
                        }
                    })
                    .sum()
            }),
            TextSpacing::Line => first_col.iter().skip(1).map(|ln| ln.ascent + ln.descent).sum(),
            TextSpacing::Para => first_col
                .windows(2)
                .filter(|w| w[1].first_char > 0 && chars.get(w[1].first_char - 1).is_some_and(|&c| vector_core::text::is_para_end(c)))
                .map(|w| t.format_at(w[0].first_char).size)
                .sum(),
        };
        // one line, no space, one paragraph: as if there were one more
        let per = if per > 1e-9 {
            per
        } else {
            match self {
                TextSpacing::Char | TextSpacing::Word => t.format.size * 0.25,
                TextSpacing::Line => first_col.first().map_or(t.format.size, |ln| ln.ascent + ln.descent),
                TextSpacing::Para => t.format.size,
            }
        };
        (per / 100.0).max(1e-6)
    }
}

/// The spacing handles the Shape tool shows on a selected text (#110), as CorelDRAW does: the
/// horizontal one past the lower right corner, the vertical one below the lower left corner (of
/// the frame of paragraph text, of the lines of artistic text). Document points, each with the
/// corner it is drawn from: [horizontal, vertical]. None for a text they don't apply to: being
/// edited, on a path, vertical, warped by an effect, or empty.
pub fn text_spacing_handles(d: &DocView, o: &Object) -> Option<[(Point, Point); 2]> {
    let ObjKind::Text(t) = &o.kind else { return None };
    if t.on_path.is_some() || t.vertical || o.effects.warps_geometry() || d.text_edit.as_ref().is_some_and(|e| e.obj == o.id) {
        return None;
    }
    let b = match t.kind {
        vector_core::text::TextKind::Paragraph { width, height } => Rect::new(0.0, 0.0, width, height),
        vector_core::text::TextKind::Artistic => {
            if t.content.is_empty() {
                return None;
            }
            let l = super::overlay::cached_layout(o.id, t);
            let lines: Vec<&vector_core::text::LaidLine> = l.lines.iter().filter(|x| !x.hidden).collect();
            let (first, last) = (lines.first()?, lines.last()?);
            let x0 = lines.iter().map(|x| x.x0).fold(f64::INFINITY, f64::min);
            let x1 = lines.iter().map(|x| x.x1).fold(f64::NEG_INFINITY, f64::max);
            Rect::new(x0, first.baseline - first.ascent, x1, last.baseline + last.descent)
        }
    };
    // 12 pixels out from the corner, along the text's own x or y
    let tr = o.transform;
    let gap = d.doc_len(12.0);
    let out = |p: Point, v: Vec2| {
        let w = (tr * (p + v)) - (tr * p);
        let len = w.hypot();
        (if len > 1e-12 { tr * p + w * (gap / len) } else { tr * p }, tr * p)
    };
    Some([out(Point::new(b.x1, b.y1), Vec2::new(1.0, 0.0)), out(Point::new(b.x0, b.y1), Vec2::new(0.0, 1.0))])
}

/// The segment clicked with the Shape tool, while its curve is still selected and has it.
pub fn selected_segment(d: &DocView) -> Option<(ObjId, usize, usize)> {
    let (id, si, seg) = d.seg_sel?;
    if !d.selection.contains(&id) {
        return None;
    }
    match &d.doc.find(id)?.kind {
        ObjKind::Path(np) => np.segment_nodes(si, seg).map(|_| (id, si, seg)),
        _ => None,
    }
}

/// The control points the Shape tool shows and lets you drag: both control points of each curved
/// segment on either side of a selected node, and of the selected segment, as CorelDRAW shows
/// them (#122). (object, subpath, node, incoming)
pub fn visible_controls(d: &DocView) -> Vec<(ObjId, usize, usize, bool)> {
    let mut segs: Vec<(ObjId, usize, usize)> = vec![];
    for &(id, si, ni) in &d.node_sel {
        let Some(ObjKind::Path(np)) = d.doc.find(id).map(|o| &o.kind) else { continue };
        let Some(sp) = np.subpaths.get(si) else { continue };
        let n = sp.nodes.len();
        if ni >= n {
            continue;
        }
        segs.push((id, si, ni));
        if ni > 0 || sp.closed {
            segs.push((id, si, (ni + n - 1) % n));
        }
    }
    segs.extend(selected_segment(d));
    let mut out = vec![];
    for (id, si, seg) in segs {
        let Some(ObjKind::Path(np)) = d.doc.find(id).map(|o| &o.kind) else { continue };
        let Some((a, b)) = np.segment_nodes(si, seg) else { continue };
        let (Some(na), Some(nb)) = (np.node(a), np.node(b)) else { continue };
        if na.line_out {
            continue;
        }
        // a control point on its node can't be grabbed apart from the node
        for (h, show) in [((id, a.0, a.1, false), na.c_out != na.p), ((id, b.0, b.1, true), nb.c_in != nb.p)] {
            if show && !out.contains(&h) {
                out.push(h);
            }
        }
    }
    out
}

/// What the Shape tool grabs at document point `p`: shown control points first, then the nearest
/// node (curve node, rectangle corner, polygon or ellipse node) of a selected object, then a
/// segment of a selected curve.
pub fn shape_hit(d: &DocView, p: Point, tol: f64) -> Option<ShapeHit> {
    let dist = |a: Point| (a - p).hypot();
    for (obj, si, ni, incoming) in visible_controls(d) {
        let Some(o) = d.doc.find(obj) else { continue };
        let ObjKind::Path(np) = &o.kind else { continue };
        let Some(n) = np.node((si, ni)) else { continue };
        if dist(o.transform * if incoming { n.c_in } else { n.c_out }) <= tol {
            return Some(ShapeHit::Control { obj, si, ni, incoming });
        }
    }
    let mut best: Option<(f64, ShapeHit)> = None;
    let mut consider = |q: Point, h: ShapeHit| {
        let dd = dist(q);
        if dd <= tol && best.is_none_or(|b| dd < b.0) {
            best = Some((dd, h));
        }
    };
    for &obj in &d.selection {
        let Some(o) = d.doc.find(obj) else { continue };
        let t = o.transform;
        match &o.kind {
            ObjKind::Path(np) => {
                for (si, sp) in np.subpaths.iter().enumerate() {
                    for (ni, n) in sp.nodes.iter().enumerate() {
                        consider(t * n.p, ShapeHit::Node { obj, si, ni });
                    }
                }
            }
            ObjKind::Rect(r) => {
                // the nodes where the corners are rounded, or the corners themselves
                for (k, c) in rect_corner_nodes(r).into_iter().chain((0..4).map(|k| (k, rect_corner_dirs(r, k).0))) {
                    consider(t * c, ShapeHit::Corner { obj, corner: k });
                }
            }
            ObjKind::Polygon(pg) => {
                for (q, inner, index) in pg.node_points() {
                    consider(t * q, ShapeHit::PolygonNode { obj, inner, index });
                }
            }
            ObjKind::Ellipse(e) => {
                for (q, start) in e.nodes() {
                    consider(t * q, ShapeHit::EllipseNode { obj, start });
                }
            }
            ObjKind::Text(_) => {
                if let Some([(h, _), (v, _)]) = text_spacing_handles(d, o) {
                    consider(h, ShapeHit::TextSpacing { obj, vertical: false });
                    consider(v, ShapeHit::TextSpacing { obj, vertical: true });
                }
            }
            _ => {}
        }
    }
    if let Some((_, h)) = best {
        return Some(h);
    }
    for &obj in &d.selection {
        let Some(o) = d.doc.find(obj) else { continue };
        let ObjKind::Path(np) = &o.kind else { continue };
        if let Some((si, seg, t, q, _)) = np.nearest(o.transform.inverse() * p) {
            if dist(o.transform * q) <= tol {
                return Some(ShapeHit::Segment { obj, si, seg, t });
            }
        }
    }
    None
}
