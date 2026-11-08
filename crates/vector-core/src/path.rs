//! Editable node-based paths (what the Shape tool manipulates) and conversion to/from kurbo paths.

use kurbo::{Affine, BezPath, CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveNearest, PathEl, Point, Rect, Shape};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NodeKind {
    #[default]
    Cusp,
    Smooth,
    Symmetric,
}

/// A node. `c_in` is the control point of the segment arriving at this node,
/// `c_out` of the segment leaving it. `line_out == true` means the segment from
/// this node to the next is a straight line (controls ignored).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub p: Point,
    pub c_in: Point,
    pub c_out: Point,
    pub kind: NodeKind,
    pub line_out: bool,
}

impl Node {
    pub fn corner(p: Point) -> Node {
        Node { p, c_in: p, c_out: p, kind: NodeKind::Cusp, line_out: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct SubPath {
    pub nodes: Vec<Node>,
    pub closed: bool,
}

impl SubPath {
    pub fn segment_count(&self) -> usize {
        if self.nodes.len() < 2 {
            0
        } else if self.closed {
            self.nodes.len()
        } else {
            self.nodes.len() - 1
        }
    }

    /// Segment `i` as a cubic (lines are converted to degenerate cubics).
    pub fn segment(&self, i: usize) -> Option<(CubicBez, bool)> {
        let n = self.nodes.len();
        if i >= self.segment_count() {
            return None;
        }
        let a = &self.nodes[i];
        let b = &self.nodes[(i + 1) % n];
        if a.line_out {
            let p1 = a.p.lerp(b.p, 1.0 / 3.0);
            let p2 = a.p.lerp(b.p, 2.0 / 3.0);
            Some((CubicBez::new(a.p, p1, p2, b.p), true))
        } else {
            Some((CubicBez::new(a.p, a.c_out, b.c_in, b.p), false))
        }
    }

    pub fn reverse(&mut self) {
        let n = self.nodes.len();
        if n == 0 {
            return;
        }
        // line flags belong to outgoing segments; shift them.
        let flags: Vec<bool> = self.nodes.iter().map(|n| n.line_out).collect();
        self.nodes.reverse();
        for nd in self.nodes.iter_mut() {
            std::mem::swap(&mut nd.c_in, &mut nd.c_out);
        }
        // after reversal node k (old index n-1-k) outgoing segment = old incoming segment of old node,
        // which was old node (n-2-k)'s outgoing flag.
        for k in 0..n {
            let old = n - 1 - k;
            let prev_old = if old == 0 { n - 1 } else { old - 1 };
            self.nodes[k].line_out = if !self.closed && old == 0 { true } else { flags[prev_old] };
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct NodePath {
    pub subpaths: Vec<SubPath>,
}

/// Reference to a node: (subpath index, node index).
pub type NodeRef = (usize, usize);

impl NodePath {
    pub fn new() -> Self {
        NodePath { subpaths: vec![] }
    }

    pub fn is_empty(&self) -> bool {
        self.subpaths.iter().all(|s| s.nodes.is_empty())
    }

    pub fn node_count(&self) -> usize {
        self.subpaths.iter().map(|s| s.nodes.len()).sum()
    }

    pub fn is_closed(&self) -> bool {
        !self.subpaths.is_empty() && self.subpaths.iter().all(|s| s.closed)
    }

    pub fn node(&self, r: NodeRef) -> Option<&Node> {
        self.subpaths.get(r.0)?.nodes.get(r.1)
    }

    pub fn node_mut(&mut self, r: NodeRef) -> Option<&mut Node> {
        self.subpaths.get_mut(r.0)?.nodes.get_mut(r.1)
    }

    pub fn all_refs(&self) -> Vec<NodeRef> {
        let mut v = vec![];
        for (si, s) in self.subpaths.iter().enumerate() {
            for ni in 0..s.nodes.len() {
                v.push((si, ni));
            }
        }
        v
    }

    pub fn to_bez(&self) -> BezPath {
        let mut bp = BezPath::new();
        for sp in &self.subpaths {
            if sp.nodes.is_empty() {
                continue;
            }
            bp.move_to(sp.nodes[0].p);
            let n = sp.nodes.len();
            for i in 0..sp.segment_count() {
                let a = &sp.nodes[i];
                let b = &sp.nodes[(i + 1) % n];
                if a.line_out {
                    bp.line_to(b.p);
                } else {
                    bp.curve_to(a.c_out, b.c_in, b.p);
                }
            }
            if sp.closed {
                bp.close_path();
            }
        }
        bp
    }

    pub fn from_bez(bp: &BezPath) -> NodePath {
        let mut out = NodePath::new();
        let mut cur: Option<SubPath> = None;
        for el in bp.elements() {
            match *el {
                PathEl::MoveTo(p) => {
                    if let Some(sp) = cur.take() {
                        if !sp.nodes.is_empty() {
                            out.subpaths.push(sp);
                        }
                    }
                    cur = Some(SubPath { nodes: vec![Node::corner(p)], closed: false });
                }
                PathEl::LineTo(p) => {
                    let sp = cur.get_or_insert_with(|| SubPath { nodes: vec![Node::corner(Point::ZERO)], closed: false });
                    if let Some(last) = sp.nodes.last_mut() {
                        last.line_out = true;
                        last.c_out = last.p;
                    }
                    sp.nodes.push(Node::corner(p));
                }
                PathEl::QuadTo(c, p) => {
                    let sp = cur.get_or_insert_with(|| SubPath { nodes: vec![Node::corner(Point::ZERO)], closed: false });
                    let last = *sp.nodes.last().unwrap();
                    let c1 = last.p + (c - last.p) * (2.0 / 3.0);
                    let c2 = p + (c - p) * (2.0 / 3.0);
                    if let Some(l) = sp.nodes.last_mut() {
                        l.line_out = false;
                        l.c_out = c1;
                    }
                    sp.nodes.push(Node { p, c_in: c2, c_out: p, kind: NodeKind::Cusp, line_out: true });
                }
                PathEl::CurveTo(c1, c2, p) => {
                    let sp = cur.get_or_insert_with(|| SubPath { nodes: vec![Node::corner(Point::ZERO)], closed: false });
                    if let Some(l) = sp.nodes.last_mut() {
                        l.line_out = false;
                        l.c_out = c1;
                    }
                    sp.nodes.push(Node { p, c_in: c2, c_out: p, kind: NodeKind::Cusp, line_out: true });
                }
                PathEl::ClosePath => {
                    if let Some(mut sp) = cur.take() {
                        // merge duplicated end node into start
                        if sp.nodes.len() > 1 {
                            let first = sp.nodes[0].p;
                            let last = sp.nodes[sp.nodes.len() - 1];
                            if (last.p - first).hypot() < 1e-6 {
                                sp.nodes.pop();
                                sp.nodes[0].c_in = last.c_in;
                                // the closing segment's flag is last-but-one's line_out (already set)
                                let n = sp.nodes.len();
                                let prev_flag = sp.nodes[n - 1].line_out;
                                let _ = prev_flag;
                            } else {
                                let n = sp.nodes.len();
                                sp.nodes[n - 1].line_out = true;
                            }
                        }
                        sp.closed = true;
                        let start = sp.nodes[0].p;
                        out.subpaths.push(sp);
                        cur = Some(SubPath { nodes: vec![], closed: false });
                        let _ = start;
                    }
                }
            }
        }
        if let Some(sp) = cur.take() {
            if sp.nodes.len() > 0 {
                out.subpaths.push(sp);
            }
        }
        // Fix up: a closed path whose final segment came from the merged node — when we popped the
        // duplicated end node, the segment arriving at node 0 is described by node[n-1].line_out and
        // c_out (set when that segment was appended), which is already correct.
        for sp in out.subpaths.iter_mut() {
            sp.nodes.retain(|_| true);
            infer_node_kinds(sp);
        }
        out.subpaths.retain(|s| !s.nodes.is_empty());
        out
    }

    pub fn transform(&mut self, a: Affine) {
        for sp in self.subpaths.iter_mut() {
            for n in sp.nodes.iter_mut() {
                n.p = a * n.p;
                n.c_in = a * n.c_in;
                n.c_out = a * n.c_out;
            }
        }
    }

    pub fn bounds(&self) -> Rect {
        self.to_bez().bounding_box()
    }

    /// Nearest point on the path: (subpath, segment, t, point, distance).
    pub fn nearest(&self, p: Point) -> Option<(usize, usize, f64, Point, f64)> {
        let mut best: Option<(usize, usize, f64, Point, f64)> = None;
        for (si, sp) in self.subpaths.iter().enumerate() {
            for i in 0..sp.segment_count() {
                if let Some((c, _)) = sp.segment(i) {
                    let n = c.nearest(p, 1e-6);
                    let q = c.eval(n.t);
                    let d = (q - p).hypot();
                    if best.map_or(true, |b| d < b.4) {
                        best = Some((si, i, n.t, q, d));
                    }
                }
            }
        }
        best
    }

    /// Split segment `seg` of subpath `si` at parameter `t`, inserting a new node. Returns its ref.
    pub fn insert_node(&mut self, si: usize, seg: usize, t: f64) -> Option<NodeRef> {
        let sp = self.subpaths.get_mut(si)?;
        let (c, is_line) = sp.segment(seg)?;
        let n = sp.nodes.len();
        let next = (seg + 1) % n;
        let t = t.clamp(0.001, 0.999);
        if is_line {
            let p = c.eval(t);
            let mut nd = Node::corner(p);
            nd.line_out = true;
            sp.nodes.insert(seg + 1, nd);
        } else {
            let (a, b) = (c.subsegment(0.0..t), c.subsegment(t..1.0));
            sp.nodes[seg].c_out = a.p1;
            sp.nodes[next].c_in = b.p2;
            let nd = Node { p: a.p3, c_in: a.p2, c_out: b.p1, kind: NodeKind::Smooth, line_out: false };
            sp.nodes.insert(seg + 1, nd);
        }
        Some((si, seg + 1))
    }

    pub fn delete_nodes(&mut self, refs: &[NodeRef]) {
        let mut refs = refs.to_vec();
        refs.sort();
        refs.dedup();
        for &(si, ni) in refs.iter().rev() {
            if let Some(sp) = self.subpaths.get_mut(si) {
                if ni < sp.nodes.len() {
                    sp.nodes.remove(ni);
                }
            }
        }
        self.subpaths.retain(|s| s.nodes.len() >= 2 || (s.nodes.len() == 1 && false));
        for sp in self.subpaths.iter_mut() {
            if sp.nodes.len() < 3 {
                sp.closed = false;
            }
        }
    }

    /// Convert the segment leaving the node to a line or a curve.
    pub fn set_segment_line(&mut self, r: NodeRef, line: bool) {
        let n = match self.subpaths.get(r.0) {
            Some(s) => s.nodes.len(),
            None => return,
        };
        if n < 2 {
            return;
        }
        let sp = &mut self.subpaths[r.0];
        if !sp.closed && r.1 == n - 1 {
            return;
        }
        let next = (r.1 + 1) % n;
        let a = sp.nodes[r.1].p;
        let b = sp.nodes[next].p;
        sp.nodes[r.1].line_out = line;
        if !line {
            sp.nodes[r.1].c_out = a.lerp(b, 1.0 / 3.0);
            sp.nodes[next].c_in = a.lerp(b, 2.0 / 3.0);
        }
    }

    pub fn set_node_kind(&mut self, r: NodeRef, kind: NodeKind) {
        if let Some(nd) = self.node_mut(r) {
            nd.kind = kind;
            if kind != NodeKind::Cusp {
                let p = nd.p;
                let vi = p - nd.c_in;
                let vo = nd.c_out - p;
                let li = vi.hypot();
                let lo = vo.hypot();
                if li > 1e-9 && lo > 1e-9 {
                    let dir = (vi / li + vo / lo).normalize();
                    let (li2, lo2) = if kind == NodeKind::Symmetric {
                        let avg = (li + lo) / 2.0;
                        (avg, avg)
                    } else {
                        (li, lo)
                    };
                    nd.c_in = p - dir * li2;
                    nd.c_out = p + dir * lo2;
                }
            }
        }
    }

    /// Break a closed subpath at a node, or split an open subpath into two.
    pub fn break_at(&mut self, r: NodeRef) {
        let Some(sp) = self.subpaths.get(r.0).cloned() else { return };
        let n = sp.nodes.len();
        if n < 2 {
            return;
        }
        if sp.closed {
            let mut nodes = vec![];
            for k in 0..=n {
                nodes.push(sp.nodes[(r.1 + k) % n]);
            }
            let last = nodes.len() - 1;
            nodes[last].line_out = true;
            self.subpaths[r.0] = SubPath { nodes, closed: false };
        } else {
            if r.1 == 0 || r.1 == n - 1 {
                return;
            }
            let mut a = sp.nodes[..=r.1].to_vec();
            let b = sp.nodes[r.1..].to_vec();
            if let Some(l) = a.last_mut() {
                l.line_out = true;
            }
            self.subpaths[r.0] = SubPath { nodes: a, closed: false };
            self.subpaths.insert(r.0 + 1, SubPath { nodes: b, closed: false });
        }
    }

    /// Join two end nodes (either of the same open subpath — closing it — or of two subpaths).
    pub fn join_nodes(&mut self, a: NodeRef, b: NodeRef) -> bool {
        if a.0 == b.0 {
            let Some(sp) = self.subpaths.get_mut(a.0) else { return false };
            let n = sp.nodes.len();
            if sp.closed || n < 2 {
                return false;
            }
            let ends = [0, n - 1];
            if !(ends.contains(&a.1) && ends.contains(&b.1) && a.1 != b.1) {
                return false;
            }
            let mid = sp.nodes[0].p.midpoint(sp.nodes[n - 1].p);
            let last = sp.nodes.pop().unwrap();
            sp.nodes[0].p = mid;
            sp.nodes[0].c_in = last.c_in + (mid - last.p);
            sp.closed = true;
            // the segment arriving at node 0 now comes from node n-2
            return true;
        }
        let (Some(sa), Some(sb)) = (self.subpaths.get(a.0).cloned(), self.subpaths.get(b.0).cloned()) else {
            return false;
        };
        if sa.closed || sb.closed {
            return false;
        }
        let mut na = sa.nodes.clone();
        let mut nb = sb.nodes.clone();
        // orient so that a's end is last of na and b's end is first of nb
        let mut spa = SubPath { nodes: na.clone(), closed: false };
        if a.1 == 0 && na.len() > 1 {
            spa.reverse();
            na = spa.nodes.clone();
        }
        let mut spb = SubPath { nodes: nb.clone(), closed: false };
        if b.1 != 0 {
            spb.reverse();
            nb = spb.nodes.clone();
        }
        let la = na.pop().unwrap();
        let fb = nb[0];
        let mid = la.p.midpoint(fb.p);
        let merged = Node { p: mid, c_in: la.c_in + (mid - la.p), c_out: fb.c_out + (mid - fb.p), kind: NodeKind::Cusp, line_out: fb.line_out };
        na.push(merged);
        na.extend_from_slice(&nb[1..]);
        let (lo, hi) = if a.0 < b.0 { (a.0, b.0) } else { (b.0, a.0) };
        self.subpaths.remove(hi);
        self.subpaths[lo] = SubPath { nodes: na, closed: false };
        true
    }

    /// Connect two end nodes with a straight segment (extend curve to close).
    pub fn extend_to_close(&mut self, a: NodeRef, b: NodeRef) -> bool {
        if a.0 == b.0 {
            if let Some(sp) = self.subpaths.get_mut(a.0) {
                if !sp.closed && sp.nodes.len() > 2 {
                    let n = sp.nodes.len();
                    sp.nodes[n - 1].line_out = true;
                    sp.closed = true;
                    return true;
                }
            }
            return false;
        }
        // join different subpaths with a line by moving b's endpoint onto... keep both, add a segment
        let (Some(sa), Some(sb)) = (self.subpaths.get(a.0).cloned(), self.subpaths.get(b.0).cloned()) else {
            return false;
        };
        let mut spa = sa.clone();
        if a.1 == 0 {
            spa.reverse();
        }
        let mut spb = sb.clone();
        if b.1 != 0 {
            spb.reverse();
        }
        if let Some(l) = spa.nodes.last_mut() {
            l.line_out = true;
        }
        spa.nodes.extend(spb.nodes);
        let (lo, hi) = if a.0 < b.0 { (a.0, b.0) } else { (b.0, a.0) };
        self.subpaths.remove(hi);
        self.subpaths[lo] = spa;
        true
    }

    pub fn close_all(&mut self) {
        for sp in self.subpaths.iter_mut() {
            if !sp.closed && sp.nodes.len() > 2 {
                let n = sp.nodes.len();
                sp.nodes[n - 1].line_out = true;
                sp.closed = true;
            }
        }
    }

    pub fn reverse(&mut self) {
        for sp in self.subpaths.iter_mut() {
            sp.reverse();
        }
    }

    pub fn length(&self) -> f64 {
        let mut l = 0.0;
        for sp in &self.subpaths {
            for i in 0..sp.segment_count() {
                if let Some((c, _)) = sp.segment(i) {
                    l += c.arclen(1e-3);
                }
            }
        }
        l
    }

    /// Remove nodes that don't change the shape much (auto-reduce).
    pub fn reduce(&mut self, tolerance: f64) {
        let bp = self.to_bez();
        let simplified = kurbo::simplify::simplify_bezpath(bp.elements().iter().copied(), tolerance, &Default::default());
        let closed_flags: Vec<bool> = self.subpaths.iter().map(|s| s.closed).collect();
        let mut np = NodePath::from_bez(&simplified);
        for (i, sp) in np.subpaths.iter_mut().enumerate() {
            if let Some(&c) = closed_flags.get(i) {
                if c && !sp.closed && sp.nodes.len() > 2 {
                    let first = sp.nodes[0].p;
                    let n = sp.nodes.len();
                    if (sp.nodes[n - 1].p - first).hypot() < tolerance.max(0.01) {
                        let last = sp.nodes.pop().unwrap();
                        sp.nodes[0].c_in = last.c_in;
                    }
                    sp.closed = true;
                }
            }
        }
        *self = np;
    }

    /// Align a node's handles so the node becomes smooth along the line from prev to next.
    pub fn smooth_all(&mut self) {
        let refs = self.all_refs();
        for r in refs {
            self.set_node_kind(r, NodeKind::Smooth);
        }
    }

    /// Hit-test a line segment of the path (for Virtual Segment Delete etc.).
    pub fn segment_line(&self, si: usize, seg: usize) -> Option<Line> {
        let sp = self.subpaths.get(si)?;
        let n = sp.nodes.len();
        if seg >= sp.segment_count() {
            return None;
        }
        Some(Line::new(sp.nodes[seg].p, sp.nodes[(seg + 1) % n].p))
    }

    /// Delete a whole segment; splits or opens the subpath.
    pub fn delete_segment(&mut self, si: usize, seg: usize) {
        let Some(sp) = self.subpaths.get(si).cloned() else { return };
        let n = sp.nodes.len();
        if seg >= sp.segment_count() {
            return;
        }
        if sp.closed {
            let mut nodes = vec![];
            for k in 0..n {
                nodes.push(sp.nodes[(seg + 1 + k) % n]);
            }
            let last = nodes.len() - 1;
            nodes[last].line_out = true;
            self.subpaths[si] = SubPath { nodes, closed: false };
        } else {
            let mut a = sp.nodes[..=seg].to_vec();
            let b = sp.nodes[seg + 1..].to_vec();
            if let Some(l) = a.last_mut() {
                l.line_out = true;
            }
            let mut repl = vec![];
            if a.len() >= 2 {
                repl.push(SubPath { nodes: a, closed: false });
            }
            if b.len() >= 2 {
                repl.push(SubPath { nodes: b, closed: false });
            }
            self.subpaths.splice(si..si + 1, repl);
        }
    }
}

fn infer_node_kinds(sp: &mut SubPath) {
    let n = sp.nodes.len();
    for i in 0..n {
        let nd = sp.nodes[i];
        let has_in = (nd.c_in - nd.p).hypot() > 1e-6;
        let has_out = (nd.c_out - nd.p).hypot() > 1e-6 && !nd.line_out;
        if has_in && has_out {
            let vi = (nd.p - nd.c_in).normalize();
            let vo = (nd.c_out - nd.p).normalize();
            let cross = vi.cross(vo).abs();
            let dot = vi.dot(vo);
            if cross < 0.02 && dot > 0.0 {
                let li = (nd.p - nd.c_in).hypot();
                let lo = (nd.c_out - nd.p).hypot();
                sp.nodes[i].kind = if (li - lo).abs() < 0.01 * li.max(lo) { NodeKind::Symmetric } else { NodeKind::Smooth };
            }
        }
    }
}

/// Build a path from polyline points.
pub fn polyline(points: &[Point], closed: bool) -> NodePath {
    let nodes: Vec<Node> = points.iter().map(|p| Node::corner(*p)).collect();
    NodePath { subpaths: vec![SubPath { nodes, closed }] }
}

/// Fit smooth cubic Béziers through freehand sample points (Schneider-style via kurbo simplify).
pub fn fit_freehand(points: &[Point], smoothing: f64, closed: bool) -> NodePath {
    if points.len() < 2 {
        return polyline(points, false);
    }
    // de-duplicate
    let mut pts: Vec<Point> = vec![points[0]];
    for p in &points[1..] {
        if (*p - *pts.last().unwrap()).hypot() > 0.05 {
            pts.push(*p);
        }
    }
    if pts.len() < 3 {
        return polyline(&pts, false);
    }
    // Catmull-Rom through points, then simplify with a tolerance derived from the smoothing (0..100).
    let mut bp = BezPath::new();
    bp.move_to(pts[0]);
    let n = pts.len();
    for i in 0..n - 1 {
        let p0 = if i == 0 { pts[0] } else { pts[i - 1] };
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = if i + 2 < n { pts[i + 2] } else { pts[n - 1] };
        let c1 = p1 + (p2 - p0) / 6.0;
        let c2 = p2 - (p3 - p1) / 6.0;
        bp.curve_to(c1, c2, p2);
    }
    let tol = 0.05 + (smoothing.clamp(0.0, 100.0) / 100.0).powi(2) * 6.0;
    let simplified = kurbo::simplify::simplify_bezpath(bp.elements().iter().copied(), tol, &Default::default());
    let mut np = NodePath::from_bez(&simplified);
    if closed {
        np.close_all();
    }
    np
}

/// Approximate arc length parameterization helper: point and tangent at a fraction of total length.
pub fn point_at_length(bp: &BezPath, s: f64) -> Option<(Point, kurbo::Vec2)> {
    let mut acc = 0.0;
    for seg in bp.segments() {
        let l = seg.arclen(1e-3);
        if acc + l >= s && l > 0.0 {
            let t = seg.inv_arclen(s - acc, 1e-3);
            let p = seg.eval(t);
            let d = match seg {
                kurbo::PathSeg::Line(l) => l.p1 - l.p0,
                kurbo::PathSeg::Quad(q) => kurbo::ParamCurveDeriv::deriv(&q).eval(t).to_vec2(),
                kurbo::PathSeg::Cubic(c) => kurbo::ParamCurveDeriv::deriv(&c).eval(t).to_vec2(),
            };
            return Some((p, d));
        }
        acc += l;
    }
    let last = bp.segments().last()?;
    let p = last.eval(1.0);
    let d = match last {
        kurbo::PathSeg::Line(l) => l.p1 - l.p0,
        kurbo::PathSeg::Quad(q) => kurbo::ParamCurveDeriv::deriv(&q).eval(1.0).to_vec2(),
        kurbo::PathSeg::Cubic(c) => kurbo::ParamCurveDeriv::deriv(&c).eval(1.0).to_vec2(),
    };
    Some((p, d))
}

pub fn bez_length(bp: &BezPath) -> f64 {
    bp.segments().map(|s| s.arclen(1e-3)).sum()
}
