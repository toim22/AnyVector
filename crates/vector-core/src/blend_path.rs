//! Blends along a path, split blends and mapped nodes (#145).
//!
//! A blend's path is a plain object embedded in the blend ([`BlendPath`]). The steps are placed
//! along it by arclength ([`Stations`]): the path is flattened once and every station is a
//! distance along that polyline, so the spacing never depends on how the curve is parameterised
//! (an ellipse is four cubics, a hand-drawn curve any number of uneven ones) and no curve inverse
//! is solved (the 0.5.5 freeze, #102, was an `inv_arclen` that never ended at the end of a segment).
//!
//! Where the start, the end and the split objects ("controls") sit on the path is read from the
//! objects themselves: each control's center projected onto the path, or the two ends of the path
//! with Blend Along Full Path. Moving a control (or the path) keeps it on the path
//! ([`settle`]), as the control objects of a CorelDRAW blend slide along their path.
//!
//! With Rotate All Objects each control is turned by the direction of the path where it sits, and
//! every step is the interpolation of the controls taken back to an unturned frame, turned by the
//! direction at its own station. Nothing about the turn is stored, so a blend that is moved,
//! rotated or mirrored as a whole stays consistent, and a circle closes without a seam: each step
//! takes the direction of the path where it is, so no step spins the long way round.
//!
//! On a closed path the controls run forward along the path, through its start if they must; two
//! controls in the same place go once round the whole path. A path with several subpaths is
//! followed along the subpath nearest the start and end objects. `looped` (rotation around the
//! middle of the blend) does not apply on a path; the rotation of each step still does.

use crate::document::Document;
use crate::geom;
use crate::object::{accel, blend_color, blend_step_paths, BlendObj, BlendPart, BlendPath, BlendSplit, ObjId, ObjKind, Object};
use kurbo::{Affine, BezPath, PathEl, Point};
use std::f64::consts::PI;

/// Flattening tolerance of the paths blends follow (points).
const TOLERANCE: f64 = 0.01;
/// A bend of the flattened path sharper than this (radians) is a corner of the path; gentler
/// ones are the chords of a curve, and the direction runs smoothly through them.
const CORNER: f64 = 25.0 * PI / 180.0;
/// The most steps a fixed spacing makes.
const MAX_SPACED_STEPS: usize = 5000;

/// A path prepared for placing objects along it: a polyline with the length run so far at each
/// point, so a station is an exact distance along the path, and the direction of the path at both
/// ends of each of its pieces.
#[derive(Clone, Debug)]
pub struct Stations {
    pts: Vec<Point>,
    acc: Vec<f64>,
    closed: bool,
    /// Per piece: the direction at its start and at its end (radians, the end unwrapped to within
    /// half a turn of the start). On a curve they are those of the curve, so the direction turns
    /// smoothly along it instead of in jumps from chord to chord; at a corner they are the sides'.
    dirs: Vec<(f64, f64)>,
    /// Per point: the direction there when it is a corner (between its two sides).
    corners: Vec<Option<f64>>,
}

impl Stations {
    /// The stations of the subpath of `path` nearest the points `near` (the first subpath when
    /// `near` is empty); `None` when the path has no length.
    pub fn new(path: &BezPath, near: &[Point]) -> Option<Stations> {
        let mut best: Option<(f64, Stations)> = None;
        for (pts, closed) in geom::flatten_polylines(path, TOLERANCE) {
            let Some(st) = Stations::from_polyline(pts, closed) else { continue };
            let d: f64 = near.iter().map(|p| (st.point(st.project(*p)) - *p).hypot()).sum();
            if best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best = Some((d, st));
            }
        }
        best.map(|b| b.1)
    }

    /// Stations along a polyline (closed: back to its first point).
    pub fn from_polyline(mut pts: Vec<Point>, closed: bool) -> Option<Stations> {
        pts.retain(|p| p.x.is_finite() && p.y.is_finite());
        pts.dedup_by(|a, b| (*a - *b).hypot() < 1e-9);
        if closed && pts.len() >= 2 && (pts[0] - pts[pts.len() - 1]).hypot() > 1e-9 {
            pts.push(pts[0]);
        }
        if pts.len() < 2 {
            return None;
        }
        let mut acc = Vec::with_capacity(pts.len());
        acc.push(0.0);
        for w in pts.windows(2) {
            acc.push(acc[acc.len() - 1] + (w[1] - w[0]).hypot());
        }
        let total = acc[acc.len() - 1];
        if !(total > 1e-6 && total.is_finite()) {
            return None;
        }
        let n = pts.len() - 1;
        let seg: Vec<f64> = pts.windows(2).map(|w| (w[1].y - w[0].y).atan2(w[1].x - w[0].x)).collect();
        // the direction at each point: halfway between its two pieces on a curve, or a corner
        let mut at: Vec<(f64, f64)> = seg.iter().map(|d| (*d, *d)).collect();
        let mut corners = vec![None; n + 1];
        // (the last point of a closed path is its first point again; an open path's ends have
        // one piece only)
        for j in 0..n {
            let (before, after) = match (j, closed) {
                (0, true) => (n - 1, 0),
                (0, false) => continue,
                _ => (j - 1, j),
            };
            let turn = norm(seg[after] - seg[before]);
            let mid = seg[before] + turn / 2.0;
            if turn.abs() < CORNER {
                at[before].1 = mid;
                at[after].0 = mid;
            } else {
                corners[j] = Some(mid);
                if closed && j == 0 {
                    corners[n] = Some(mid);
                }
            }
        }
        let dirs = at.into_iter().map(|(a, b)| (a, a + norm(b - a))).collect();
        Some(Stations { pts, acc, closed, dirs, corners })
    }

    /// The length of the path.
    pub fn total(&self) -> f64 {
        self.acc[self.acc.len() - 1]
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    /// `s` on the path: wrapped round a closed path, clamped to the ends of an open one.
    fn wrap(&self, s: f64) -> f64 {
        let total = self.total();
        if !s.is_finite() {
            return 0.0;
        }
        if self.closed {
            s.rem_euclid(total)
        } else {
            s.clamp(0.0, total)
        }
    }

    /// The point `s` along the path.
    pub fn point(&self, s: f64) -> Point {
        let s = self.wrap(s);
        let n = self.pts.len();
        // the ends exactly: a full blend puts its end at `total`
        if s <= 0.0 {
            return self.pts[0];
        }
        if s >= self.total() {
            return self.pts[n - 1];
        }
        let i = self.acc.partition_point(|a| *a <= s).saturating_sub(1).min(n - 2);
        let seg = self.acc[i + 1] - self.acc[i];
        let t = if seg > 1e-12 { ((s - self.acc[i]) / seg).clamp(0.0, 1.0) } else { 0.0 };
        self.pts[i].lerp(self.pts[i + 1], t)
    }

    /// The direction of the path at `s` (radians). Exactly at a corner it is the direction
    /// between the two sides, so a step on a corner of a polygon does not turn with either side.
    pub fn angle(&self, s: f64) -> f64 {
        let s = self.wrap(s);
        let n = self.pts.len() - 1;
        let i = self.acc.partition_point(|a| *a <= s).saturating_sub(1).min(n - 1);
        if (s - self.acc[i]).abs() < 1e-9 {
            if let Some(c) = self.corners[i] {
                return c;
            }
        }
        if (s - self.acc[i + 1]).abs() < 1e-9 {
            if let Some(c) = self.corners[i + 1] {
                return c;
            }
        }
        let seg = self.acc[i + 1] - self.acc[i];
        let t = if seg > 1e-12 { ((s - self.acc[i]) / seg).clamp(0.0, 1.0) } else { 0.0 };
        let (a, b) = self.dirs[i];
        a + (b - a) * t
    }

    /// The point and the direction (degrees) at `s`.
    pub fn at(&self, s: f64) -> (Point, f64) {
        (self.point(s), self.angle(s).to_degrees())
    }

    /// How far along the path the point of the path nearest `p` is.
    pub fn project(&self, p: Point) -> f64 {
        let mut best = (f64::INFINITY, 0.0);
        for (i, w) in self.pts.windows(2).enumerate() {
            let d = w[1] - w[0];
            let l2 = d.hypot2();
            let t = if l2 > 1e-24 { ((p - w[0]).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let q = w[0] + d * t;
            let dist = (p - q).hypot2();
            if dist < best.0 {
                best = (dist, self.acc[i] + (self.acc[i + 1] - self.acc[i]) * t);
            }
        }
        best.1
    }
}

/// The middle of an object's bounds: where a control sits on the path.
fn centre(o: &Object) -> Point {
    o.bounds().center()
}

/// An angle in (-π, π].
fn norm(a: f64) -> f64 {
    let mut d = a % (2.0 * PI);
    if d > PI {
        d -= 2.0 * PI;
    } else if d <= -PI {
        d += 2.0 * PI;
    }
    d
}

/// The controls of a blend in order: the start, the split objects, the end.
pub fn controls(b: &BlendObj) -> Vec<&Object> {
    let mut v = vec![b.start.as_ref()];
    v.extend(b.splits.iter().map(|s| s.obj.as_ref()));
    v.push(b.end.as_ref());
    v
}

fn control_mut(b: &mut BlendObj, i: usize) -> &mut Object {
    let n = b.splits.len();
    if i == 0 {
        &mut b.start
    } else if i <= n {
        &mut b.splits[i - 1].obj
    } else {
        &mut b.end
    }
}

/// The path of a blend prepared for placement, and how far along it each control sits (start,
/// splits, end; increasing along a closed path, through its start if need be).
pub fn control_stations(b: &BlendObj) -> Option<(Stations, Vec<f64>)> {
    let p = b.path.as_ref()?;
    let cs: Vec<Point> = controls(b).into_iter().map(centre).collect();
    let n = cs.len();
    let st = Stations::new(&p.obj.world_path(), &[cs[0], cs[n - 1]])?;
    let total = st.total();
    let mut ss: Vec<f64> = cs.iter().map(|c| st.project(*c)).collect();
    if p.full {
        ss[0] = 0.0;
        ss[n - 1] = total;
        for s in ss[1..n - 1].iter_mut() {
            *s = s.clamp(0.0, total);
        }
    }
    if st.closed() {
        for i in 1..n {
            if p.full && i == n - 1 {
                ss[i] = ss[i].max(ss[i - 1]);
                continue;
            }
            // forward along the path; the same place again is once round it
            while ss[i] < ss[i - 1] + 1e-6 {
                ss[i] += total;
            }
        }
    }
    Some((st, ss))
}

/// One place of the blend between its two ends: a step on the leg from control `leg` to the next
/// (`t` along it, `s` its fixed station with Fixed Spacing; `u` the blend's progress at the two
/// controls), or a split object.
enum Slot {
    Step { leg: usize, t: f64, s: Option<f64>, u: (f64, f64) },
    Split(usize),
}

/// The places of the steps and the split objects, from the start to the end.
fn slots(b: &BlendObj, stations: Option<&(Stations, Vec<f64>)>) -> Vec<Slot> {
    let m = b.splits.len();
    // Fixed Spacing (on a path only): a step every `d` along the path
    if let (Some(d), Some((_, ss))) = (b.spacing, stations) {
        if d > 1e-6 && d.is_finite() {
            let (s0, s1) = (ss[0], ss[ss.len() - 1]);
            let len = (s1 - s0).abs();
            let dir = if s1 >= s0 { 1.0 } else { -1.0 };
            let n = if len > 1e-9 { (((len / d) - 1e-9).ceil() as usize).saturating_sub(1).min(MAX_SPACED_STEPS) } else { 0 };
            let mut out = Vec::with_capacity(n + m);
            let mut next_split = 0;
            for k in 1..=n {
                let s = s0 + dir * k as f64 * d;
                // the split objects passed on the way
                while next_split < m && (ss[next_split + 1] - s0) * dir <= (s - s0) * dir {
                    out.push(Slot::Split(next_split));
                    next_split += 1;
                }
                let leg = (0..ss.len() - 1).find(|&j| (ss[j + 1] - s0) * dir >= (s - s0) * dir).unwrap_or(ss.len() - 2);
                let span = ss[leg + 1] - ss[leg];
                let t = if span.abs() > 1e-12 { ((s - ss[leg]) / span).clamp(0.0, 1.0) } else { 0.0 };
                let u = |j: usize| if len > 1e-12 { ((ss[j] - s0) * dir / len).clamp(0.0, 1.0) } else { 0.0 };
                out.push(Slot::Step { leg, t, s: Some(s), u: (u(leg), u(leg + 1)) });
            }
            for j in next_split..m {
                out.push(Slot::Split(j));
            }
            return out;
        }
    }
    // a number of steps, the split objects in the places of the steps they were
    let n = (b.steps as usize).max(m);
    let mut bounds = vec![0usize];
    for (j, s) in b.splits.iter().enumerate() {
        let lo = bounds[bounds.len() - 1] + 1;
        let hi = n - (m - 1 - j);
        let k = ((s.at * (n + 1) as f64).round().max(0.0) as usize).clamp(lo, hi.max(lo));
        bounds.push(k);
    }
    bounds.push(n + 1);
    let mut out = Vec::with_capacity(n);
    let mut leg = 0;
    for k in 1..=n {
        if leg < m && k == bounds[leg + 1] {
            out.push(Slot::Split(leg));
            leg += 1;
            continue;
        }
        let (a, z) = (bounds[leg], bounds[leg + 1]);
        let t = (k - a) as f64 / (z - a) as f64;
        out.push(Slot::Step { leg, t, s: None, u: (a as f64 / (n + 1) as f64, z as f64 / (n + 1) as f64) });
    }
    out
}

/// The intermediate objects of a blend (see [`crate::object::blend_steps`]): its steps along the
/// straight line between the controls or along its path, and its split objects in their places.
pub fn steps(b: &BlendObj, new_id: &mut dyn FnMut() -> ObjId) -> Vec<Object> {
    let stations = control_stations(b);
    let ctrls = controls(b);
    let last = ctrls.len() - 1;
    let cs: Vec<Point> = ctrls.iter().map(|o| centre(o)).collect();
    let pivot = cs[0].midpoint(cs[last]);
    let rotate = b.path.as_ref().is_some_and(|p| p.rotate);
    let (map_start, map_end) = match b.map {
        Some((si, fa, fb)) => (Some((si, fa)), Some((si, fb))),
        None => (None, None),
    };
    // the outlines of the controls, unturned and centred when they follow a path
    let outlines: Vec<BezPath> = ctrls
        .iter()
        .enumerate()
        .map(|(i, o)| match &stations {
            Some((st, ss)) => {
                let turn = if rotate { st.angle(ss[i]) } else { 0.0 };
                Affine::rotate(-turn) * Affine::translate(-cs[i].to_vec2()) * o.world_path()
            }
            None => o.world_path(),
        })
        .collect();
    slots(b, stations.as_ref())
        .into_iter()
        .map(|slot| {
            let (leg, t, fixed, u) = match slot {
                Slot::Split(j) => return b.splits[j].obj.as_ref().clone(),
                Slot::Step { leg, t, s, u } => (leg, t, s, u),
            };
            let (a, z) = (ctrls[leg], ctrls[leg + 1]);
            let map = (if leg == 0 { map_start } else { None }, if leg + 1 == last { map_end } else { None });
            let to = accel(t, b.acceleration);
            let tc = if b.link_accel { to } else { accel(t, b.color_accel) };
            let ts = if b.size_accel { to } else { t };
            let mut o = match &stations {
                Some((st, ss)) => {
                    let mut o = blend_step_paths(a, z, &outlines[leg], &outlines[leg + 1], map, ts, new_id(), b.rotation);
                    let s = fixed.unwrap_or(ss[leg] + (ss[leg + 1] - ss[leg]) * to);
                    let have = o.bounds().center();
                    let turn = if rotate { st.angle(s) } else { 0.0 };
                    o.apply_transform(Affine::translate(st.point(s).to_vec2()) * Affine::rotate(turn) * Affine::translate(-have.to_vec2()));
                    o
                }
                None => {
                    let mut o = blend_step_paths(a, z, &outlines[leg], &outlines[leg + 1], map, ts, new_id(), if b.looped { 0.0 } else { b.rotation });
                    // position follows the object acceleration
                    let want = cs[leg].lerp(cs[leg + 1], to);
                    let have = o.bounds().center();
                    o.apply_transform(Affine::translate(want - have));
                    if b.looped && b.rotation != 0.0 {
                        let progress = u.0 + (u.1 - u.0) * to;
                        o.apply_transform(Affine::rotate_about((b.rotation * progress).to_radians(), pivot));
                    }
                    o
                }
            };
            // colors follow the color acceleration and direction
            use crate::style::Fill;
            if let (Fill::Solid(x), Fill::Solid(y)) = (&a.style.fill, &z.style.fill) {
                o.style.fill = Fill::Solid(blend_color(*x, *y, tc, b.color_dir));
            }
            if let (Some(x), Some(y)) = (a.style.outline.color, z.style.outline.color) {
                o.style.outline.color = Some(blend_color(x, y, tc, b.color_dir));
            }
            o
        })
        .collect()
}

/// How many steps the blend draws (its split objects left out): `steps`, or what Fixed Spacing
/// makes of the path.
pub fn effective_steps(b: &BlendObj) -> u32 {
    slots(b, control_stations(b).as_ref()).iter().filter(|s| matches!(s, Slot::Step { .. })).count() as u32
}

/// Turn `o` by `turn` (radians) about its middle and put its middle at `q`.
fn put(o: &mut Object, q: Point, turn: f64) {
    if turn.abs() > 1e-12 {
        let c = centre(o);
        o.apply_transform(Affine::rotate_about(turn, c));
    }
    let d = q - centre(o);
    if d.hypot() > 1e-9 {
        o.apply_transform(Affine::translate(d));
    }
}

/// Keep the controls of a blend on its path after something of it moved (`before` is the blend
/// before): each control goes to its place on the path (the ends of the path with Blend Along
/// Full Path), and with Rotate All Objects turns as much as the direction of the path did between
/// its old and its new place.
pub fn settle(b: &mut BlendObj, before: &BlendObj) {
    let Some((st, ss)) = control_stations(b) else { return };
    let rotate = b.path.as_ref().is_some_and(|p| p.rotate);
    let old = if rotate { control_stations(before) } else { None };
    for (i, &s) in ss.iter().enumerate() {
        let turn = match &old {
            Some((ost, oss)) if oss.len() == ss.len() => norm(st.angle(s) - ost.angle(oss[i])),
            _ => 0.0,
        };
        put(control_mut(b, i), st.point(s), turn);
    }
}

/// Put the controls on the path just attached: at the points of the path nearest them (at its
/// two ends when they would meet on an open path, or with Blend Along Full Path).
fn place_on_path(b: &mut BlendObj) {
    let Some((st, mut ss)) = control_stations(b) else { return };
    let n = ss.len();
    let full = b.path.as_ref().is_some_and(|p| p.full);
    if !st.closed() && !full && (ss[n - 1] - ss[0]).abs() < st.total() * 0.01 {
        ss[0] = 0.0;
        ss[n - 1] = st.total();
    }
    for (i, &s) in ss.iter().enumerate() {
        put(control_mut(b, i), st.point(s), 0.0);
    }
}

/// Turn every control by the direction of the path where it sits (`sign` 1), or back (-1).
fn turn_controls(b: &mut BlendObj, sign: f64) {
    let Some((st, ss)) = control_stations(b) else { return };
    for (i, &s) in ss.iter().enumerate() {
        put(control_mut(b, i), st.point(s), st.angle(s) * sign);
    }
}

// ------------------------------------------------------------------------------------------
// Document operations
// ------------------------------------------------------------------------------------------

/// Why a blend operation was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendError {
    /// The object to edit is not a blend.
    NotABlend,
    /// The object chosen is the blend itself or one of its parts.
    OwnPart,
    /// The object chosen can't be a path (text, a bitmap, a group, ...).
    NotAPath,
    /// The path chosen has no length.
    TooShort,
    /// The object chosen can't be the start or end of a blend.
    NotAControl,
}

fn blend_mut(doc: &mut Document, id: ObjId) -> Option<&mut BlendObj> {
    match &mut doc.find_mut(id)?.kind {
        ObjKind::Blend(b) => Some(b),
        _ => None,
    }
}

/// The blend `id`, if it is one.
pub fn blend_of(doc: &Document, id: ObjId) -> Option<&BlendObj> {
    match &doc.find(id)?.kind {
        ObjKind::Blend(b) => Some(b),
        _ => None,
    }
}

/// Objects a blend can follow: curves and shapes.
pub fn can_be_path(o: &Object) -> bool {
    matches!(o.kind, ObjKind::Rect(_) | ObjKind::Ellipse(_) | ObjKind::Polygon(_) | ObjKind::Spiral(_) | ObjKind::Path(_) | ObjKind::Connector(_))
}

/// Objects a blend can start or end with: anything with an outline of its own.
pub fn can_be_control(o: &Object) -> bool {
    !matches!(o.kind, ObjKind::Group(_) | ObjKind::PowerClip { .. } | ObjKind::Blend(_) | ObjKind::Symbol(_) | ObjKind::Image(_))
}

/// The blend `id` is a part of, and which part.
pub fn part_of(doc: &Document, id: ObjId) -> Option<(ObjId, BlendPart)> {
    let mut found = None;
    for p in &doc.pages {
        for l in &p.layers {
            for o in &l.objects {
                o.walk(&mut |x| {
                    if found.is_none() {
                        if let ObjKind::Blend(b) = &x.kind {
                            if let Some(part) = b.part_with(id) {
                                found = Some((x.id, part));
                            }
                        }
                    }
                });
                if found.is_some() {
                    return found;
                }
            }
        }
    }
    None
}

/// Every blend part in the document: part id → (blend, part).
pub fn all_parts(doc: &Document) -> std::collections::HashMap<ObjId, (ObjId, BlendPart)> {
    let mut m = std::collections::HashMap::new();
    for p in &doc.pages {
        for l in &p.layers {
            for o in &l.objects {
                o.walk(&mut |x| {
                    if let ObjKind::Blend(b) = &x.kind {
                        m.insert(b.start.id, (x.id, BlendPart::Start));
                        m.insert(b.end.id, (x.id, BlendPart::End));
                        if let Some(p) = &b.path {
                            m.insert(p.obj.id, (x.id, BlendPart::Path));
                        }
                        for (i, s) in b.splits.iter().enumerate() {
                            m.insert(s.obj.id, (x.id, BlendPart::Split(i)));
                        }
                    }
                });
            }
        }
    }
    m
}

/// Put `o` on the page just above the top-level object that holds `anchor`.
fn insert_above(doc: &mut Document, anchor: ObjId, o: Object) {
    let top = doc.top_level_of(anchor).unwrap_or(anchor);
    if let Some((p, l, i)) = doc.locate(top) {
        let objs = &mut doc.pages[p].layers[l].objects;
        objs.insert((i + 1).min(objs.len()), o);
    } else if let Some(l) = doc.pages.first_mut().and_then(|p| p.layers.last_mut()) {
        l.objects.push(o);
    }
}

/// Path Properties ▸ New Path: the blend follows the object `path`, which becomes part of it
/// (it leaves the page, as the start and end objects did). The controls go onto the path; a path
/// the blend had before goes back on the page. Blend Along Full Path and Rotate All Objects stay
/// as they were.
pub fn set_path(doc: &mut Document, blend: ObjId, path: ObjId) -> Result<(), BlendError> {
    let bo = doc.find(blend).ok_or(BlendError::NotABlend)?;
    if !matches!(bo.kind, ObjKind::Blend(_)) {
        return Err(BlendError::NotABlend);
    }
    if path == blend || bo.find(path).is_some() {
        return Err(BlendError::OwnPart);
    }
    let po = doc.find(path).ok_or(BlendError::NotAPath)?;
    if !can_be_path(po) {
        return Err(BlendError::NotAPath);
    }
    if Stations::new(&po.world_path(), &[]).is_none() {
        return Err(BlendError::TooShort);
    }
    let removed = doc.remove(path).ok_or(BlendError::NotAPath)?;
    let b = blend_mut(doc, blend).ok_or(BlendError::NotABlend)?;
    let old = b.path.take();
    let (full, rotate) = old.as_ref().map(|p| (p.full, p.rotate)).unwrap_or((false, false));
    if let Some(o) = old.as_ref().filter(|o| o.rotate) {
        // turned for the old path: back to plain first
        b.path = Some(o.clone());
        turn_controls(b, -1.0);
    }
    b.path = Some(BlendPath { obj: Box::new(removed), full, rotate: false });
    place_on_path(b);
    if rotate {
        turn_controls(b, 1.0);
        if let Some(p) = &mut b.path {
            p.rotate = true;
        }
    }
    if let Some(o) = old {
        insert_above(doc, blend, *o.obj);
    }
    Ok(())
}

/// Path Properties ▸ Detach from Path: the path goes back on the page as an object of its own;
/// the start and end stay where they are and the blend is straight again. Fixed Spacing becomes
/// the number of steps it made. Gives the path's id.
pub fn detach_path(doc: &mut Document, blend: ObjId) -> Option<ObjId> {
    let o = remove_path(doc, blend)?;
    let id = o.id;
    insert_above(doc, blend, o);
    Some(id)
}

/// The path taken out of the blend (Detach from Path without putting it back: Delete on the
/// path shown).
pub fn remove_path(doc: &mut Document, blend: ObjId) -> Option<Object> {
    let b = blend_mut(doc, blend)?;
    b.path.as_ref()?;
    let n = effective_steps(b);
    let p = b.path.take()?;
    if b.spacing.take().is_some() {
        b.steps = n.max(1);
    }
    Some(*p.obj)
}

/// Blend Along Full Path on or off. On, the start goes to the beginning of the path and the end
/// to its end; off, they stay where they are. False when nothing changed.
pub fn set_full_path(doc: &mut Document, blend: ObjId, on: bool) -> bool {
    let Some(b) = blend_mut(doc, blend) else { return false };
    if b.path.as_ref().is_none_or(|p| p.full == on) {
        return false;
    }
    let before = b.clone();
    if let Some(p) = &mut b.path {
        p.full = on;
    }
    settle(b, &before);
    true
}

/// Rotate All Objects on or off: the controls turn with the direction of the path where they
/// sit (or back), and so do the steps. False when nothing changed.
pub fn set_rotate_all(doc: &mut Document, blend: ObjId, on: bool) -> bool {
    let Some(b) = blend_mut(doc, blend) else { return false };
    if b.path.as_ref().is_none_or(|p| p.rotate == on) {
        return false;
    }
    turn_controls(b, if on { 1.0 } else { -1.0 });
    if let Some(p) = &mut b.path {
        p.rotate = on;
    }
    true
}

/// Fixed spacing (`Some(distance)`) or a number of steps (`None`). The spacing only applies on a
/// path. False when nothing changed.
pub fn set_spacing(doc: &mut Document, blend: ObjId, spacing: Option<f64>) -> bool {
    let Some(b) = blend_mut(doc, blend) else { return false };
    let spacing = spacing.map(|d| d.max(0.01));
    if b.spacing == spacing {
        return false;
    }
    if spacing.is_none() && b.path.is_some() {
        // back to a number of steps: as many as the spacing made
        b.steps = effective_steps(b).max(1);
    }
    b.spacing = spacing;
    true
}

/// New Start / New End: the object `obj` becomes the start (or end) of the blend, leaving the
/// page; the old start (or end) goes back on the page. On a path it goes onto the path. Gives the
/// old control's id.
pub fn replace_control(doc: &mut Document, blend: ObjId, obj: ObjId, end: bool) -> Result<ObjId, BlendError> {
    let bo = doc.find(blend).ok_or(BlendError::NotABlend)?;
    if !matches!(bo.kind, ObjKind::Blend(_)) {
        return Err(BlendError::NotABlend);
    }
    if obj == blend || bo.find(obj).is_some() {
        return Err(BlendError::OwnPart);
    }
    if !doc.find(obj).is_some_and(can_be_control) {
        return Err(BlendError::NotAControl);
    }
    let new = doc.remove(obj).ok_or(BlendError::NotAControl)?;
    let b = blend_mut(doc, blend).ok_or(BlendError::NotABlend)?;
    let slot = if end { &mut b.end } else { &mut b.start };
    let old = std::mem::replace(slot, Box::new(new));
    b.map = None;
    if let Some((st, ss)) = control_stations(b) {
        let rotate = b.path.as_ref().is_some_and(|p| p.rotate);
        let i = if end { ss.len() - 1 } else { 0 };
        let (q, a) = (st.point(ss[i]), st.angle(ss[i]));
        put(control_mut(b, i), q, if rotate { a } else { 0.0 });
    }
    let old_id = old.id;
    insert_above(doc, blend, *old);
    Ok(old_id)
}

/// Options ▸ Split: the step `slot` (1-based, in the order [`crate::object::blend_steps`] gives
/// them) becomes a split object the blend passes through; it can then be moved on its own.
/// Gives its id.
pub fn split(doc: &mut Document, blend: ObjId, slot: usize) -> Option<ObjId> {
    let id = doc.new_id();
    let b = blend_mut(doc, blend)?;
    let stations = control_stations(b);
    let places = slots(b, stations.as_ref());
    let k = slot.checked_sub(1)?;
    if !matches!(places.get(k)?, Slot::Step { .. }) {
        return None;
    }
    let mut o = steps(b, &mut || 0).into_iter().nth(k)?;
    o.id = id;
    o.name.clear();
    // the splits before it in the blend; every split keeps its place among the steps
    let pos = places[..k].iter().filter(|p| matches!(p, Slot::Split(_))).count();
    b.splits.insert(pos, BlendSplit { obj: Box::new(o), at: 0.0 });
    let n = places.len() + 1;
    let mut j = 0;
    for (i, p) in places.iter().enumerate() {
        if i == k || matches!(p, Slot::Split(_)) {
            if let Some(s) = b.splits.get_mut(j) {
                s.at = (i + 1) as f64 / n as f64;
            }
            j += 1;
        }
    }
    Some(id)
}

/// Which split Fuse Start / Fuse End joins back into the blend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fuse {
    /// Fuse Start: the split nearest the start.
    First,
    /// Fuse End: the split nearest the end.
    Last,
    At(usize),
}

/// Fuse a split back: the blend no longer passes through that object, which becomes a step
/// again. False when the blend has no such split.
pub fn fuse(doc: &mut Document, blend: ObjId, which: Fuse) -> bool {
    let Some(b) = blend_mut(doc, blend) else { return false };
    let n = b.splits.len();
    let i = match which {
        Fuse::First => 0,
        Fuse::Last => n.wrapping_sub(1),
        Fuse::At(i) => i,
    };
    if i >= n {
        return false;
    }
    b.splits.remove(i);
    true
}

/// Map Nodes: the closed subpath `si` of the start object is matched with the same subpath of the
/// end object from the points at the fractions `fa` and `fb` of their lengths.
pub fn map_nodes(doc: &mut Document, blend: ObjId, si: usize, fa: f64, fb: f64) -> bool {
    let Some(b) = blend_mut(doc, blend) else { return false };
    b.map = Some((si, fa.rem_euclid(1.0), fb.rem_euclid(1.0)));
    true
}

/// Delete a part of a blend: the start or the end takes the blend apart (the other control, the
/// path and the split objects stay on the page, the deleted one goes); the path detaches and goes;
/// a split is fused. Gives what to select afterwards.
pub fn delete_part(doc: &mut Document, blend: ObjId, part: BlendPart) -> Option<Vec<ObjId>> {
    match part {
        BlendPart::Path => {
            remove_path(doc, blend)?;
            Some(vec![blend])
        }
        BlendPart::Split(i) => fuse(doc, blend, Fuse::At(i)).then(|| vec![blend]),
        BlendPart::Start | BlendPart::End => {
            let b = blend_of(doc, blend)?.clone();
            let top = doc.top_level_of(blend)?;
            let (p, l, i) = doc.locate(top)?;
            let keep = if part == BlendPart::Start { *b.end } else { *b.start };
            let mut objs: Vec<Object> = vec![];
            if let Some(path) = b.path {
                objs.push(*path.obj);
            }
            objs.extend(b.splits.into_iter().map(|s| *s.obj));
            let keep_id = keep.id;
            objs.push(keep);
            if top == blend {
                let layer = &mut doc.pages[p].layers[l].objects;
                layer.remove(i);
                for (k, o) in objs.into_iter().enumerate() {
                    layer.insert(i + k, o);
                }
            } else {
                // a blend inside a group or PowerClip: the parts go on the page above that
                doc.remove(blend)?;
                let layer = &mut doc.pages[p].layers[l].objects;
                for (k, o) in objs.into_iter().enumerate() {
                    layer.insert((i + 1 + k).min(layer.len()), o);
                }
            }
            Some(vec![keep_id])
        }
    }
}

/// The step of the blend under `p` (1-based, as [`split`] takes it): the nearest one whose bounds
/// (grown by `tol`) hold `p`; split objects are not steps.
pub fn step_at(b: &BlendObj, p: Point, tol: f64) -> Option<usize> {
    let stations = control_stations(b);
    let places = slots(b, stations.as_ref());
    let objs = steps(b, &mut || 0);
    let mut best: Option<(f64, usize)> = None;
    for (k, (o, place)) in objs.iter().zip(places.iter()).enumerate() {
        if !matches!(place, Slot::Step { .. }) {
            continue;
        }
        let r = o.bounds().inflate(tol, tol);
        if r.contains(p) {
            let d = (r.center() - p).hypot();
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, k + 1));
            }
        }
    }
    best.map(|b| b.1)
}

/// The nodes of an object's outline that Map Nodes can match: (point, closed subpath, fraction of
/// its length). Open subpaths have no node to match.
pub fn map_candidates(o: &Object) -> Vec<(Point, usize, f64)> {
    let wp = o.world_path();
    let polys = geom::flatten_polylines(&wp, 0.2);
    let mut nodes: Vec<(Point, usize)> = vec![];
    let mut si: Option<usize> = None;
    for el in wp.elements() {
        let p = match *el {
            PathEl::MoveTo(p) => {
                si = Some(si.map_or(0, |s| s + 1));
                Some(p)
            }
            PathEl::LineTo(p) | PathEl::QuadTo(_, p) | PathEl::CurveTo(_, _, p) => Some(p),
            PathEl::ClosePath => None,
        };
        if let (Some(p), Some(s)) = (p, si) {
            if !nodes.iter().any(|(q, k)| *k == s && (*q - p).hypot() < 1e-6) {
                nodes.push((p, s));
            }
        }
    }
    nodes
        .into_iter()
        .filter_map(|(p, s)| {
            let (pts, closed) = polys.get(s)?.clone();
            if !closed {
                return None;
            }
            // as `resample_from` reads it: the flattened subpath, back to its first point
            let mut pts = pts;
            pts.push(pts[0]);
            let mut acc = vec![0.0];
            for w in pts.windows(2) {
                acc.push(acc[acc.len() - 1] + (w[1] - w[0]).hypot());
            }
            let total = acc[acc.len() - 1];
            if total <= 1e-9 {
                return None;
            }
            let mut best = (f64::INFINITY, 0.0);
            for (i, w) in pts.windows(2).enumerate() {
                let d = w[1] - w[0];
                let l2 = d.hypot2();
                let t = if l2 > 1e-24 { ((p - w[0]).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
                let dist = (p - (w[0] + d * t)).hypot2();
                if dist < best.0 {
                    best = (dist, acc[i] + (acc[i + 1] - acc[i]) * t);
                }
            }
            Some((p, s, (best.1 / total).rem_euclid(1.0)))
        })
        .collect()
}

/// The node of `o` nearest `p` within `tol` that Map Nodes can match: (closed subpath, fraction).
pub fn map_node_at(o: &Object, p: Point, tol: f64) -> Option<(usize, f64)> {
    map_candidates(o).into_iter().filter(|(q, _, _)| (*q - p).hypot() <= tol).min_by(|a, b| (a.0 - p).hypot().total_cmp(&(b.0 - p).hypot())).map(|(_, s, f)| (s, f))
}

/// Is `p` on the drawn blend `b` (a blend on a path: on a step, a control or the path)?
pub fn hit_steps(b: &BlendObj, p: Point, tol: f64) -> bool {
    steps(b, &mut || 0).iter().any(|o| o.bounds().inflate(tol, tol).contains(p))
}

/// The control of the blend `b` that a click picks (#149), where `hit(o, part)` tells whether
/// the pointer is on the part `part` (object `o`). The controls are the objects the user grabs, so
/// they win over the steps drawn on them; among themselves the one drawn on top wins: the end, the
/// split objects (drawn with the steps), the start, then the path under it all. Hidden parts are
/// left out. None when the pointer is on no control (on a step, or between them).
pub fn control_at(b: &BlendObj, hit: impl Fn(&Object, BlendPart) -> bool) -> Option<BlendPart> {
    let mut order = vec![BlendPart::End];
    order.extend((0..b.splits.len()).rev().map(BlendPart::Split));
    order.push(BlendPart::Start);
    order.push(BlendPart::Path);
    order.into_iter().find(|&part| b.part(part).is_some_and(|o| o.visible && hit(o, part)))
}
