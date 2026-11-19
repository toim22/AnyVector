//! What the Shape tool can grab (#104, #108, #122, #123): the control points it shows, the nodes
//! of curves, rectangle corners, polygon and ellipse nodes, and curve segments. One hit test for
//! the press, the cursor and the overlay, so they always agree.

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
