//! Document operations (arrange, shaping, alignment…). All operate on top-level objects of a page.

use crate::document::Document;
use crate::geom::{self, BoolOp};
use crate::object::{BlendObj, ObjId, ObjKind, Object};
use crate::path::NodePath;
use kurbo::{Affine, BezPath, Point, Rect, Shape, Vec2};

/// Where the given objects live: (page, layer, index), only top-level ones.
fn locs(doc: &Document, ids: &[ObjId]) -> Vec<(usize, usize, usize, ObjId)> {
    let mut v: Vec<(usize, usize, usize, ObjId)> = ids.iter().filter_map(|id| doc.locate(*id).map(|(p, l, i)| (p, l, i, *id))).collect();
    v.sort();
    v
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Order {
    ToFront,
    ToBack,
    ForwardOne,
    BackOne,
}

pub fn reorder(doc: &mut Document, ids: &[ObjId], order: Order) {
    let ls = locs(doc, ids);
    match order {
        Order::ToFront => {
            for (p, l, _, id) in ls.iter() {
                let layer = &mut doc.pages[*p].layers[*l];
                if let Some(i) = layer.objects.iter().position(|o| o.id == *id) {
                    let o = layer.objects.remove(i);
                    layer.objects.push(o);
                }
            }
        }
        Order::ToBack => {
            for (p, l, _, id) in ls.iter().rev() {
                let layer = &mut doc.pages[*p].layers[*l];
                if let Some(i) = layer.objects.iter().position(|o| o.id == *id) {
                    let o = layer.objects.remove(i);
                    layer.objects.insert(0, o);
                }
            }
        }
        Order::ForwardOne => {
            for (p, l, _, id) in ls.iter().rev() {
                let layer = &mut doc.pages[*p].layers[*l];
                if let Some(i) = layer.objects.iter().position(|o| o.id == *id) {
                    if i + 1 < layer.objects.len() && !ids.contains(&layer.objects[i + 1].id) {
                        layer.objects.swap(i, i + 1);
                    }
                }
            }
        }
        Order::BackOne => {
            for (p, l, _, id) in ls.iter() {
                let layer = &mut doc.pages[*p].layers[*l];
                if let Some(i) = layer.objects.iter().position(|o| o.id == *id) {
                    if i > 0 && !ids.contains(&layer.objects[i - 1].id) {
                        layer.objects.swap(i, i - 1);
                    }
                }
            }
        }
    }
}

/// Reverse stacking order of the selection.
pub fn reverse_order(doc: &mut Document, ids: &[ObjId]) {
    let ls = locs(doc, ids);
    let mut by_layer: std::collections::BTreeMap<(usize, usize), Vec<usize>> = Default::default();
    for (p, l, i, _) in ls {
        by_layer.entry((p, l)).or_default().push(i);
    }
    for ((p, l), idx) in by_layer {
        let layer = &mut doc.pages[p].layers[l];
        let objs: Vec<Object> = idx.iter().map(|i| layer.objects[*i].clone()).collect();
        for (k, i) in idx.iter().enumerate() {
            layer.objects[*i] = objs[objs.len() - 1 - k].clone();
        }
    }
}

/// Place `ids` directly in front of / behind `target`.
pub fn place_relative(doc: &mut Document, ids: &[ObjId], target: ObjId, in_front: bool) {
    let objs: Vec<Object> = ids.iter().filter(|i| **i != target).filter_map(|i| doc.remove(*i)).collect();
    if let Some((p, l, i)) = doc.locate(target) {
        let at = if in_front { i + 1 } else { i };
        let layer = &mut doc.pages[p].layers[l];
        for (k, o) in objs.into_iter().enumerate() {
            layer.objects.insert(at + k, o);
        }
    }
}

pub fn group(doc: &mut Document, ids: &[ObjId]) -> Option<ObjId> {
    let ls = locs(doc, ids);
    if ls.len() < 1 {
        return None;
    }
    let (p, l, at, _) = *ls.last().unwrap();
    let mut children = vec![];
    for (_, _, _, id) in &ls {
        if let Some(o) = doc.remove(*id) {
            children.push(o);
        }
    }
    let at = at + 1 - children.len().min(at + 1);
    let gid = doc.new_id();
    let g = Object::new(gid, ObjKind::Group(children), Affine::IDENTITY, Default::default());
    let layer = &mut doc.pages[p].layers[l];
    let at = at.min(layer.objects.len());
    layer.objects.insert(at, g);
    Some(gid)
}

pub fn ungroup(doc: &mut Document, ids: &[ObjId], all: bool) -> Vec<ObjId> {
    let mut out = vec![];
    for id in ids {
        let Some((p, l, i)) = doc.locate(*id) else { continue };
        let o = doc.pages[p].layers[l].objects[i].clone();
        if let ObjKind::Group(children) = o.kind {
            doc.pages[p].layers[l].objects.remove(i);
            let mut insert = vec![];
            for c in children {
                if all {
                    insert.extend(flatten_groups(c));
                } else {
                    insert.push(c);
                }
            }
            for (k, c) in insert.into_iter().enumerate() {
                out.push(c.id);
                doc.pages[p].layers[l].objects.insert(i + k, c);
            }
        } else {
            out.push(*id);
        }
    }
    out
}

fn flatten_groups(o: Object) -> Vec<Object> {
    match o.kind {
        ObjKind::Group(c) => c.into_iter().flat_map(flatten_groups).collect(),
        _ => vec![o],
    }
}

/// Combine selected objects into one curve (keeps the last-selected object's style, like the original).
pub fn combine(doc: &mut Document, ids: &[ObjId]) -> Option<ObjId> {
    let ls = locs(doc, ids);
    if ls.len() < 2 {
        return None;
    }
    let style_src = ids.last().and_then(|i| doc.find(*i)).map(|o| o.style.clone()).unwrap_or_default();
    let mut np = NodePath::new();
    for (_, _, _, id) in &ls {
        if let Some(o) = doc.find(*id) {
            for leaf in leaves(o) {
                if let Some(c) = leaf.to_curve() {
                    np.subpaths.extend(c.subpaths);
                }
            }
        }
    }
    let (p, l, at, _) = ls[0];
    for (_, _, _, id) in &ls {
        doc.remove(*id);
    }
    let nid = doc.new_id();
    let mut style = style_src;
    style.even_odd = true;
    let o = Object::new(nid, ObjKind::Path(np), Affine::IDENTITY, style);
    let layer = &mut doc.pages[p].layers[l];
    layer.objects.insert(at.min(layer.objects.len()), o);
    Some(nid)
}

pub fn leaves(o: &Object) -> Vec<&Object> {
    match &o.kind {
        ObjKind::Group(c) => c.iter().flat_map(leaves).collect(),
        _ => vec![o],
    }
}

/// Break a combined curve into one object per subpath.
pub fn break_apart(doc: &mut Document, ids: &[ObjId]) -> Vec<ObjId> {
    let mut out = vec![];
    for id in ids {
        let Some((p, l, i)) = doc.locate(*id) else { continue };
        let o = doc.pages[p].layers[l].objects[i].clone();
        let np = match &o.kind {
            ObjKind::Path(np) => np.clone(),
            ObjKind::Text(_) => match o.to_curve() {
                Some(c) => c,
                None => continue,
            },
            _ => continue,
        };
        if np.subpaths.len() < 2 {
            out.push(*id);
            continue;
        }
        doc.pages[p].layers[l].objects.remove(i);
        for (k, sp) in np.subpaths.into_iter().enumerate() {
            let nid = doc.new_id();
            let mut no = Object::new(nid, ObjKind::Path(NodePath { subpaths: vec![sp] }), Affine::IDENTITY, o.style.clone());
            no.effects = o.effects.clone();
            doc.pages[p].layers[l].objects.insert(i + k, no);
            out.push(nid);
        }
    }
    out
}

pub fn convert_to_curves(doc: &mut Document, ids: &[ObjId]) -> Vec<ObjId> {
    let mut out = vec![];
    for id in ids {
        let Some(o) = doc.find(*id).cloned() else { continue };
        match &o.kind {
            ObjKind::Group(_) | ObjKind::PowerClip { .. } => {
                let child_ids: Vec<ObjId> = o.children().iter().map(|c| c.id).collect();
                convert_to_curves(doc, &child_ids);
                out.push(*id);
                continue;
            }
            ObjKind::Path(_) | ObjKind::Image(_) | ObjKind::Symbol(_) => {
                out.push(*id);
                continue;
            }
            ObjKind::Text(t) => {
                // text: one curve per color group, grouped if multicolor
                let layout = crate::text::layout(t);
                let groups = layout.colored_paths();
                let target = doc.find_mut(*id).unwrap();
                if groups.len() <= 1 {
                    let mut warped = target.apply_warps(target.transform * layout.to_path());
                    if warped.elements().is_empty() {
                        warped = BezPath::new();
                    }
                    target.kind = ObjKind::Path(NodePath::from_bez(&warped));
                    target.transform = Affine::IDENTITY;
                    target.effects.envelope = None;
                    target.effects.distort = None;
                } else {
                    let mut children = vec![];
                    let base_style = target.style.clone();
                    let tr = target.transform;
                    let mut ids_new = vec![];
                    for _ in 0..groups.len() {
                        ids_new.push(0);
                    }
                    let _ = ids_new;
                    for (c, path) in groups {
                        let mut st = base_style.clone();
                        if let Some(c) = c {
                            st.fill = crate::style::Fill::Solid(c);
                        }
                        children.push((st, tr * path));
                    }
                    let mut objs = vec![];
                    for (st, p) in children {
                        let nid = doc.new_id();
                        objs.push(Object::new(nid, ObjKind::Path(NodePath::from_bez(&p)), Affine::IDENTITY, st));
                    }
                    let target = doc.find_mut(*id).unwrap();
                    target.kind = ObjKind::Group(objs);
                    target.transform = Affine::IDENTITY;
                    target.style = Default::default();
                }
                out.push(*id);
                continue;
            }
            _ => {}
        }
        if let Some(c) = o.to_curve() {
            let target = doc.find_mut(*id).unwrap();
            target.kind = ObjKind::Path(c);
            target.transform = Affine::IDENTITY;
            target.effects.envelope = None;
            target.effects.distort = None;
            out.push(*id);
        }
    }
    out
}

/// Shaping (Weld / Trim / Intersect / Simplify / Front minus back / Back minus front / Boundary).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Shaping {
    Weld,
    Trim,
    Intersect,
    Simplify,
    FrontMinusBack,
    BackMinusFront,
    Boundary,
}

fn world_curve(o: &Object) -> BezPath {
    let mut bp = BezPath::new();
    for leaf in leaves(o) {
        let p = leaf.world_path();
        bp.extend(p.elements().iter().copied());
    }
    bp
}

/// `ids` in selection order: the last one is the "target" for Weld/Trim/Intersect.
pub fn shaping(doc: &mut Document, ids: &[ObjId], op: Shaping, leave_source: bool, leave_target: bool) -> Vec<ObjId> {
    if ids.len() < 2 && op != Shaping::Boundary {
        return vec![];
    }
    // stacking order (back to front)
    let ordered: Vec<ObjId> = locs(doc, ids).into_iter().map(|x| x.3).collect();
    let target_id = *ids.last().unwrap();
    let sources: Vec<ObjId> = ids[..ids.len() - 1].to_vec();
    let get = |doc: &Document, id: ObjId| doc.find(id).map(world_curve).unwrap_or_default();
    let mut created = vec![];
    match op {
        Shaping::Weld | Shaping::Boundary => {
            let paths: Vec<BezPath> = ordered.iter().map(|i| get(doc, *i)).collect();
            let u = geom::union_all(&paths);
            let style = doc.find(target_id).map(|o| o.style.clone()).unwrap_or_default();
            let (p, l, at) = doc.locate(ordered[ordered.len() - 1]).unwrap_or((0, 0, 0));
            if op == Shaping::Weld {
                for id in &ordered {
                    let keep = (*id == target_id && leave_target) || (*id != target_id && leave_source);
                    if !keep {
                        doc.remove(*id);
                    }
                }
            }
            let nid = doc.new_id();
            let mut st = style;
            if op == Shaping::Boundary {
                st = crate::style::Style::default();
            }
            let o = Object::new(nid, ObjKind::Path(NodePath::from_bez(&u)), Affine::IDENTITY, st);
            let layer = &mut doc.pages[p].layers[l];
            let at = if op == Shaping::Boundary { (at + 1).min(layer.objects.len()) } else { at.min(layer.objects.len()) };
            layer.objects.insert(at, o);
            created.push(nid);
        }
        Shaping::Trim => {
            let mut tp = get(doc, target_id);
            for s in &sources {
                let sp = get(doc, *s);
                tp = geom::boolean(&tp, &sp, BoolOp::Difference, false);
            }
            replace_with_curve(doc, target_id, tp, leave_target, &mut created);
            if !leave_source {
                for s in &sources {
                    doc.remove(*s);
                }
            }
        }
        Shaping::Intersect => {
            let tp = get(doc, target_id);
            let style = doc.find(target_id).map(|o| o.style.clone()).unwrap_or_default();
            let mut results = vec![];
            for s in &sources {
                let sp = get(doc, *s);
                results.push(geom::boolean(&tp, &sp, BoolOp::Intersect, false));
            }
            let (p, l, at) = doc.locate(target_id).unwrap_or((0, 0, 0));
            for r in results {
                if r.elements().is_empty() {
                    continue;
                }
                let nid = doc.new_id();
                let o = Object::new(nid, ObjKind::Path(NodePath::from_bez(&r)), Affine::IDENTITY, style.clone());
                let layer = &mut doc.pages[p].layers[l];
                layer.objects.insert((at + 1).min(layer.objects.len()), o);
                created.push(nid);
            }
            if !leave_target {
                doc.remove(target_id);
            }
            if !leave_source {
                for s in &sources {
                    doc.remove(*s);
                }
            }
        }
        Shaping::Simplify => {
            // each object minus all objects in front of it
            for (k, id) in ordered.iter().enumerate() {
                let mut p = get(doc, *id);
                for front in &ordered[k + 1..] {
                    p = geom::boolean(&p, &get(doc, *front), BoolOp::Difference, false);
                }
                replace_with_curve(doc, *id, p, false, &mut created);
            }
        }
        Shaping::FrontMinusBack => {
            let front = *ordered.last().unwrap();
            let mut p = get(doc, front);
            for b in &ordered[..ordered.len() - 1] {
                p = geom::boolean(&p, &get(doc, *b), BoolOp::Difference, false);
            }
            replace_with_curve(doc, front, p, false, &mut created);
            for b in &ordered[..ordered.len() - 1] {
                doc.remove(*b);
            }
        }
        Shaping::BackMinusFront => {
            let back = ordered[0];
            let mut p = get(doc, back);
            for f in &ordered[1..] {
                p = geom::boolean(&p, &get(doc, *f), BoolOp::Difference, false);
            }
            replace_with_curve(doc, back, p, false, &mut created);
            for f in &ordered[1..] {
                doc.remove(*f);
            }
        }
    }
    created
}

fn replace_with_curve(doc: &mut Document, id: ObjId, path: BezPath, keep_original: bool, created: &mut Vec<ObjId>) {
    if keep_original {
        let style = doc.find(id).map(|o| o.style.clone()).unwrap_or_default();
        if let Some((p, l, at)) = doc.locate(id) {
            let nid = doc.new_id();
            let o = Object::new(nid, ObjKind::Path(NodePath::from_bez(&path)), Affine::IDENTITY, style);
            doc.pages[p].layers[l].objects.insert(at + 1, o);
            created.push(nid);
        }
    } else if let Some(o) = doc.find_mut(id) {
        o.kind = ObjKind::Path(NodePath::from_bez(&path));
        o.transform = Affine::IDENTITY;
        o.effects.envelope = None;
        o.effects.distort = None;
        created.push(id);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Align {
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignTo {
    /// Last selected object (the original's default).
    LastSelected,
    FirstSelected,
    Selection,
    Page,
    PageEdge,
    Point(i64, i64),
}

pub fn align(doc: &mut Document, ids: &[ObjId], how: Align, to: AlignTo, page: usize, use_outline: bool) {
    if ids.is_empty() {
        return;
    }
    let b = |doc: &Document, id: ObjId| -> Rect {
        doc.find(id)
            .map(|o| if use_outline { o.visual_bounds() } else { doc.bounds_of(o) })
            .unwrap_or(Rect::ZERO)
    };
    let reference = match to {
        AlignTo::LastSelected => b(doc, *ids.last().unwrap()),
        AlignTo::FirstSelected => b(doc, ids[0]),
        AlignTo::Selection => doc.selection_bounds(ids).unwrap_or(Rect::ZERO),
        AlignTo::Page | AlignTo::PageEdge => doc.pages.get(page).map(|p| p.rect()).unwrap_or(Rect::ZERO),
        AlignTo::Point(x, y) => Rect::new(x as f64, y as f64, x as f64, y as f64),
    };
    let movers: Vec<ObjId> = match to {
        AlignTo::LastSelected => ids[..ids.len() - 1].to_vec(),
        AlignTo::FirstSelected => ids[1..].to_vec(),
        _ => ids.to_vec(),
    };
    for id in movers {
        let r = b(doc, id);
        let d = match how {
            Align::Left => Vec2::new(reference.x0 - r.x0, 0.0),
            Align::Right => Vec2::new(reference.x1 - r.x1, 0.0),
            Align::CenterH => Vec2::new(reference.center().x - r.center().x, 0.0),
            Align::Top => Vec2::new(0.0, reference.y0 - r.y0),
            Align::Bottom => Vec2::new(0.0, reference.y1 - r.y1),
            Align::CenterV => Vec2::new(0.0, reference.center().y - r.center().y),
        };
        doc.transform_objects(&[id], Affine::translate(d));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Distribute {
    Left,
    CenterH,
    SpacingH,
    Right,
    Top,
    CenterV,
    SpacingV,
    Bottom,
}

pub fn distribute(doc: &mut Document, ids: &[ObjId], how: Distribute, extent: Option<Rect>) {
    if ids.len() < 2 {
        return;
    }
    let mut items: Vec<(ObjId, Rect)> = ids.iter().filter_map(|i| doc.find(*i).map(|o| (*i, doc.bounds_of(o)))).collect();
    let horizontal = matches!(how, Distribute::Left | Distribute::CenterH | Distribute::SpacingH | Distribute::Right);
    let key = |r: &Rect| match how {
        Distribute::Left => r.x0,
        Distribute::CenterH | Distribute::SpacingH => r.center().x,
        Distribute::Right => r.x1,
        Distribute::Top => r.y0,
        Distribute::CenterV | Distribute::SpacingV => r.center().y,
        Distribute::Bottom => r.y1,
    };
    items.sort_by(|a, b| key(&a.1).partial_cmp(&key(&b.1)).unwrap());
    let n = items.len();
    let all = extent.unwrap_or_else(|| items.iter().fold(items[0].1, |a, b| a.union(b.1)));
    match how {
        Distribute::SpacingH | Distribute::SpacingV => {
            let total: f64 = items.iter().map(|(_, r)| if horizontal { r.width() } else { r.height() }).sum();
            let span = if horizontal { all.width() } else { all.height() };
            let gap = (span - total) / (n - 1) as f64;
            let mut pos = if horizontal { all.x0 } else { all.y0 };
            for (id, r) in items {
                let d = if horizontal { Vec2::new(pos - r.x0, 0.0) } else { Vec2::new(0.0, pos - r.y0) };
                doc.transform_objects(&[id], Affine::translate(d));
                pos += (if horizontal { r.width() } else { r.height() }) + gap;
            }
        }
        _ => {
            let first = key(&items[0].1);
            let last = key(&items[n - 1].1);
            let (first, last) = if extent.is_some() {
                match how {
                    Distribute::Left => (all.x0, all.x1 - items[n - 1].1.width()),
                    Distribute::Right => (all.x0 + items[0].1.width(), all.x1),
                    Distribute::CenterH => (all.x0 + items[0].1.width() / 2.0, all.x1 - items[n - 1].1.width() / 2.0),
                    Distribute::Top => (all.y0, all.y1 - items[n - 1].1.height()),
                    Distribute::Bottom => (all.y0 + items[0].1.height(), all.y1),
                    _ => (all.y0 + items[0].1.height() / 2.0, all.y1 - items[n - 1].1.height() / 2.0),
                }
            } else {
                (first, last)
            };
            for (k, (id, r)) in items.into_iter().enumerate() {
                let target = first + (last - first) * k as f64 / (n - 1) as f64;
                let cur = key(&r);
                let d = if horizontal { Vec2::new(target - cur, 0.0) } else { Vec2::new(0.0, target - cur) };
                doc.transform_objects(&[id], Affine::translate(d));
            }
        }
    }
}

/// Duplicate objects with an offset; returns new ids (placed above originals).
pub fn duplicate(doc: &mut Document, ids: &[ObjId], offset: Vec2) -> Vec<ObjId> {
    let mut out = vec![];
    for (p, l, _, id) in locs(doc, ids) {
        let Some(mut o) = doc.find(id).cloned() else { continue };
        doc.reassign_ids(&mut o);
        o.apply_transform(Affine::translate(offset));
        let nid = o.id;
        let layer = &mut doc.pages[p].layers[l];
        layer.objects.push(o);
        out.push(nid);
    }
    out
}

/// Step and repeat.
pub fn step_and_repeat(doc: &mut Document, ids: &[ObjId], copies: u32, offset: Vec2) -> Vec<ObjId> {
    let mut cur = ids.to_vec();
    let mut all = vec![];
    for _ in 0..copies {
        cur = duplicate(doc, &cur, offset);
        all.extend(cur.iter().copied());
    }
    all
}

pub fn create_blend(doc: &mut Document, a: ObjId, b: ObjId, steps: u32) -> Option<ObjId> {
    let (p, l, i) = doc.locate(b).or_else(|| doc.locate(a))?;
    let oa = doc.remove(a)?;
    let ob = doc.remove(b)?;
    let nid = doc.new_id();
    let bl = Object::new(
        nid,
        ObjKind::Blend(BlendObj { start: Box::new(oa), end: Box::new(ob), steps, rotation: 0.0, acceleration: 0.0 }),
        Affine::IDENTITY,
        Default::default(),
    );
    let layer = &mut doc.pages[p].layers[l];
    layer.objects.insert(i.min(layer.objects.len()), bl);
    Some(nid)
}

/// Break a blend/effect group apart into separate objects.
pub fn break_blend(doc: &mut Document, id: ObjId) -> Vec<ObjId> {
    let Some((p, l, i)) = doc.locate(id) else { return vec![] };
    let o = doc.pages[p].layers[l].objects[i].clone();
    let ObjKind::Blend(b) = &o.kind else { return vec![] };
    let mut objs = vec![(*b.start).clone()];
    for k in 1..=b.steps {
        let t = k as f64 / (b.steps + 1) as f64;
        let nid = doc.new_id();
        objs.push(crate::object::blend_step(&b.start, &b.end, t, nid, b.rotation));
    }
    objs.push((*b.end).clone());
    doc.pages[p].layers[l].objects.remove(i);
    let mut out = vec![];
    let gid = doc.new_id();
    let mut group = Object::new(gid, ObjKind::Group(vec![]), Affine::IDENTITY, Default::default());
    if let ObjKind::Group(c) = &mut group.kind {
        for o in objs {
            out.push(o.id);
            c.push(o);
        }
    }
    doc.pages[p].layers[l].objects.insert(i, group);
    vec![gid]
}

/// PowerClip: place `contents` inside `frame`.
pub fn powerclip(doc: &mut Document, contents: &[ObjId], frame: ObjId) -> Option<ObjId> {
    let (p, l, i) = doc.locate(frame)?;
    let mut items = vec![];
    for c in contents {
        if *c == frame {
            continue;
        }
        if let Some(o) = doc.remove(*c) {
            items.push(o);
        }
    }
    let (p, l, i) = doc.locate(frame).unwrap_or((p, l, i));
    let fo = doc.pages[p].layers[l].objects[i].clone();
    let new = match fo.kind {
        ObjKind::PowerClip { frame: f, contents: mut cs } => {
            cs.extend(items);
            ObjKind::PowerClip { frame: f, contents: cs }
        }
        _ => ObjKind::PowerClip { frame: Box::new(fo.clone()), contents: items },
    };
    let nid = doc.new_id();
    let mut o = Object::new(nid, new, Affine::IDENTITY, Default::default());
    o.name = "PowerClip".into();
    doc.pages[p].layers[l].objects[i] = o;
    Some(nid)
}

pub fn extract_powerclip(doc: &mut Document, id: ObjId) -> Vec<ObjId> {
    let Some((p, l, i)) = doc.locate(id) else { return vec![] };
    let o = doc.pages[p].layers[l].objects[i].clone();
    let ObjKind::PowerClip { frame, contents } = o.kind else { return vec![] };
    let mut out = vec![frame.id];
    doc.pages[p].layers[l].objects[i] = *frame;
    for (k, c) in contents.into_iter().enumerate() {
        out.push(c.id);
        doc.pages[p].layers[l].objects.insert(i + 1 + k, c);
    }
    out
}

/// Create an outline-to-object conversion (stroke becomes a filled curve).
pub fn outline_to_object(doc: &mut Document, ids: &[ObjId]) -> Vec<ObjId> {
    let mut out = vec![];
    for id in ids {
        let Some(o) = doc.find(*id).cloned() else { continue };
        let Some(color) = o.style.outline.color else { continue };
        let path = o.world_path();
        let join = match o.style.outline.join {
            crate::style::LineJoin::Miter => kurbo::Join::Miter,
            crate::style::LineJoin::Round => kurbo::Join::Round,
            crate::style::LineJoin::Bevel => kurbo::Join::Bevel,
        };
        let cap = match o.style.outline.cap {
            crate::style::LineCap::Butt => kurbo::Cap::Butt,
            crate::style::LineCap::Round => kurbo::Cap::Round,
            crate::style::LineCap::Square => kurbo::Cap::Square,
        };
        let stroke = geom::stroke_to_path(&path, o.style.outline.width.max(0.25), join, cap, &o.style.outline.dash);
        let stroke = geom::simplify_self(&stroke);
        let Some((p, l, i)) = doc.locate(*id) else { continue };
        let nid = doc.new_id();
        let mut st = crate::style::Style::filled(color);
        st.even_odd = false;
        let so = Object::new(nid, ObjKind::Path(NodePath::from_bez(&stroke)), Affine::IDENTITY, st);
        doc.pages[p].layers[l].objects.insert(i + 1, so);
        if let Some(orig) = doc.find_mut(*id) {
            orig.style.outline.color = None;
        }
        out.push(nid);
    }
    out
}

/// Bounding box of the (top-level) objects intersecting a point region; used by Smart Fill.
pub fn smart_fill_region(doc: &Document, page: usize, p: Point) -> Option<BezPath> {
    let pg = doc.pages.get(page)?;
    let mut containing = vec![];
    let mut others = vec![];
    for l in &pg.layers {
        if !l.visible {
            continue;
        }
        for o in &l.objects {
            for leaf in leaves(o) {
                let wp = leaf.world_path();
                if wp.elements().is_empty() {
                    continue;
                }
                // closed version of the path
                let mut np = NodePath::from_bez(&wp);
                np.close_all();
                let closed = np.to_bez();
                if geom::contains(&closed, p, leaf.style.even_odd) {
                    containing.push(closed);
                } else {
                    others.push(closed);
                }
            }
        }
    }
    if containing.is_empty() {
        return None;
    }
    let mut region = containing[0].clone();
    for c in &containing[1..] {
        region = geom::boolean(&region, c, BoolOp::Intersect, false);
    }
    for o in &others {
        if o.bounding_box().intersect(region.bounding_box()).area() > 0.0 {
            region = geom::boolean(&region, o, BoolOp::Difference, false);
        }
    }
    // keep only the piece containing the point
    let np = NodePath::from_bez(&region);
    let mut pieces: Vec<BezPath> = vec![];
    for sp in np.subpaths {
        pieces.push(NodePath { subpaths: vec![sp] }.to_bez());
    }
    let mut best: Option<BezPath> = None;
    for pc in &pieces {
        if geom::contains(pc, p, false) {
            // add holes that lie inside it
            let mut acc = pc.clone();
            for h in &pieces {
                if !std::ptr::eq(h, pc) && pc.bounding_box().contains(h.bounding_box().center()) && !geom::contains(h, p, false) && h.area().abs() < pc.area().abs() {
                    acc.extend(h.elements().iter().copied());
                }
            }
            let area = pc.area().abs();
            if best.as_ref().map_or(true, |b| area < b.area().abs()) {
                best = Some(acc);
            }
        }
    }
    best.or(Some(region))
}

/// Cut objects with a knife polyline (straight segments). Returns ids of the resulting pieces.
pub fn knife(doc: &mut Document, page: usize, a: Point, b: Point) -> Vec<ObjId> {
    let mut out = vec![];
    let ids: Vec<ObjId> = doc.pages.get(page).map(|p| p.all_objects().map(|o| o.id).collect()).unwrap_or_default();
    let dir = (b - a).normalize();
    if !dir.x.is_finite() {
        return out;
    }
    let normal = Vec2::new(-dir.y, dir.x);
    let big = 1.0e5;
    let half_a = {
        let mut bp = BezPath::new();
        bp.move_to(a - dir * big);
        bp.line_to(b + dir * big);
        bp.line_to(b + dir * big + normal * big);
        bp.line_to(a - dir * big + normal * big);
        bp.close_path();
        bp
    };
    let half_b = {
        let mut bp = BezPath::new();
        bp.move_to(a - dir * big);
        bp.line_to(b + dir * big);
        bp.line_to(b + dir * big - normal * big);
        bp.line_to(a - dir * big - normal * big);
        bp.close_path();
        bp
    };
    for id in ids {
        let Some(o) = doc.find(id).cloned() else { continue };
        if o.locked || o.is_container() || matches!(o.kind, ObjKind::Image(_) | ObjKind::Text(_)) {
            continue;
        }
        let wp = o.world_path();
        // does the cutting line cross the object?
        let crosses = {
            let mut hit = 0;
            for (pts, closed) in geom::flatten_polylines(&wp, 0.2) {
                let n = pts.len();
                let m = if closed { n } else { n - 1 };
                for k in 0..m {
                    if geom::seg_intersect(a, b, pts[k], pts[(k + 1) % n]).is_some() {
                        hit += 1;
                    }
                }
            }
            hit >= 2 || (!o.is_closed_shape() && hit >= 1)
        };
        if !crosses {
            continue;
        }
        if o.is_closed_shape() {
            let pa = geom::boolean(&wp, &half_a, BoolOp::Intersect, false);
            let pb = geom::boolean(&wp, &half_b, BoolOp::Intersect, false);
            let Some((p, l, i)) = doc.locate(id) else { continue };
            doc.pages[p].layers[l].objects.remove(i);
            for (k, piece) in [pa, pb].into_iter().enumerate() {
                if piece.elements().is_empty() {
                    continue;
                }
                let nid = doc.new_id();
                let mut no = Object::new(nid, ObjKind::Path(NodePath::from_bez(&piece)), Affine::IDENTITY, o.style.clone());
                no.effects = Default::default();
                doc.pages[p].layers[l].objects.insert(i + k, no);
                out.push(nid);
            }
        } else if let ObjKind::Path(np) = &o.kind {
            // open path: split at the first intersection
            let mut np = np.clone();
            if let Some((si, seg, t)) = first_intersection(&np, a, b) {
                if let Some(r) = np.insert_node(si, seg, t) {
                    np.break_at(r);
                }
                let parts = np.subpaths.clone();
                let Some((p, l, i)) = doc.locate(id) else { continue };
                doc.pages[p].layers[l].objects.remove(i);
                for (k, sp) in parts.into_iter().enumerate() {
                    let nid = doc.new_id();
                    let no = Object::new(nid, ObjKind::Path(NodePath { subpaths: vec![sp] }), Affine::IDENTITY, o.style.clone());
                    doc.pages[p].layers[l].objects.insert(i + k, no);
                    out.push(nid);
                }
            }
        }
    }
    out
}

fn first_intersection(np: &NodePath, a: Point, b: Point) -> Option<(usize, usize, f64)> {
    use kurbo::ParamCurve;
    for (si, sp) in np.subpaths.iter().enumerate() {
        for seg in 0..sp.segment_count() {
            let (c, _) = sp.segment(seg)?;
            let steps = 32;
            let mut prev = c.eval(0.0);
            for k in 1..=steps {
                let t = k as f64 / steps as f64;
                let cur = c.eval(t);
                if let Some((_, u, _)) = geom::seg_intersect(prev, cur, a, b) {
                    let t0 = (k - 1) as f64 / steps as f64;
                    return Some((si, seg, t0 + u * (1.0 / steps as f64)));
                }
                prev = cur;
            }
        }
    }
    None
}

/// Erase: subtract a brush stroke (polyline with given width) from closed shapes; open curves are cut.
pub fn erase(doc: &mut Document, page: usize, stroke: &[Point], width: f64) -> Vec<ObjId> {
    if stroke.is_empty() {
        return vec![];
    }
    let mut sp = BezPath::new();
    sp.move_to(stroke[0]);
    for p in &stroke[1..] {
        sp.line_to(*p);
    }
    if stroke.len() == 1 {
        sp.line_to(stroke[0] + Vec2::new(0.01, 0.0));
    }
    let brush = geom::stroke_to_path(&sp, width, kurbo::Join::Round, kurbo::Cap::Round, &[]);
    let brush_bb = brush.bounding_box();
    let mut out = vec![];
    let ids: Vec<ObjId> = doc.pages.get(page).map(|p| p.all_objects().map(|o| o.id).collect()).unwrap_or_default();
    for id in ids {
        let Some(o) = doc.find(id).cloned() else { continue };
        if o.locked || o.is_container() || matches!(o.kind, ObjKind::Image(_)) {
            continue;
        }
        let wp = o.world_path();
        if wp.bounding_box().intersect(brush_bb).area() <= 0.0 && wp.bounding_box().width() > 0.0 && wp.bounding_box().height() > 0.0 {
            continue;
        }
        if o.is_closed_shape() {
            let res = geom::boolean(&wp, &brush, BoolOp::Difference, o.style.even_odd);
            if let Some(t) = doc.find_mut(id) {
                t.kind = ObjKind::Path(NodePath::from_bez(&res));
                t.transform = Affine::IDENTITY;
                t.effects.envelope = None;
                t.effects.distort = None;
            }
            out.push(id);
        } else {
            // open path: drop flattened points inside the brush, keep remaining runs
            let polys = geom::flatten_polylines(&wp, 0.25);
            let mut np = NodePath::new();
            for (pts, _) in polys {
                let mut run: Vec<Point> = vec![];
                for p in pts {
                    if geom::contains(&brush, p, false) {
                        if run.len() >= 2 {
                            np.subpaths.extend(crate::path::fit_freehand(&run, 10.0, false).subpaths);
                        }
                        run.clear();
                    } else {
                        run.push(p);
                    }
                }
                if run.len() >= 2 {
                    np.subpaths.extend(crate::path::fit_freehand(&run, 10.0, false).subpaths);
                }
            }
            if let Some(t) = doc.find_mut(id) {
                t.kind = ObjKind::Path(np);
                t.transform = Affine::IDENTITY;
            }
            out.push(id);
        }
    }
    out
}

/// Crop all (or selected) objects on a page to a rectangle.
pub fn crop(doc: &mut Document, page: usize, ids: &[ObjId], r: Rect) {
    let rp = r.to_path(0.1);
    let targets: Vec<ObjId> = if ids.is_empty() {
        doc.pages.get(page).map(|p| p.all_objects().map(|o| o.id).collect()).unwrap_or_default()
    } else {
        ids.to_vec()
    };
    for id in targets {
        let Some(o) = doc.find(id).cloned() else { continue };
        let b = doc.bounds_of(&o);
        if r.contains(Point::new(b.x0, b.y0)) && r.contains(Point::new(b.x1, b.y1)) {
            continue;
        }
        if b.intersect(r).area() <= 0.0 {
            doc.remove(id);
            continue;
        }
        match &o.kind {
            ObjKind::Image(img) => {
                // crop image (axis-aligned transforms)
                let inv = o.transform.inverse();
                let lr = inv.transform_rect_bbox(r);
                let x0 = (lr.x0 / img.w).clamp(0.0, 1.0);
                let y0 = (lr.y0 / img.h).clamp(0.0, 1.0);
                let x1 = (lr.x1 / img.w).clamp(0.0, 1.0);
                let y1 = (lr.y1 / img.h).clamp(0.0, 1.0);
                if let Some(t) = doc.find_mut(id) {
                    if let ObjKind::Image(im) = &mut t.kind {
                        let c = im.crop;
                        let cw = c[2] - c[0];
                        let ch = c[3] - c[1];
                        im.crop = [c[0] + x0 * cw, c[1] + y0 * ch, c[0] + x1 * cw, c[1] + y1 * ch];
                        let (ow, oh) = (im.w, im.h);
                        im.w = (x1 - x0) * ow;
                        im.h = (y1 - y0) * oh;
                        t.transform = t.transform * Affine::translate((x0 * ow, y0 * oh));
                    }
                }
            }
            ObjKind::Group(_) | ObjKind::PowerClip { .. } | ObjKind::Blend(_) | ObjKind::Symbol(_) => {
                // wrap in a powerclip with a rectangle frame
                let fid = doc.new_id();
                let frame = Object::new(
                    fid,
                    ObjKind::Rect(crate::object::RectShape { w: r.width(), h: r.height(), radii: [0.0; 4], corner: Default::default() }),
                    Affine::translate(r.origin().to_vec2()),
                    crate::style::Style { outline: crate::style::Outline::none(), ..Default::default() },
                );
                if let Some((p, l, i)) = doc.locate(id) {
                    let obj = doc.pages[p].layers[l].objects.remove(i);
                    let nid = doc.new_id();
                    let pc = Object::new(nid, ObjKind::PowerClip { frame: Box::new(frame), contents: vec![obj] }, Affine::IDENTITY, Default::default());
                    doc.pages[p].layers[l].objects.insert(i, pc);
                }
            }
            _ => {
                let wp = o.world_path();
                let res = if o.is_closed_shape() {
                    geom::boolean(&wp, &rp, BoolOp::Intersect, o.style.even_odd)
                } else {
                    // clip open path by dropping outside points
                    let mut np = NodePath::new();
                    for (pts, _) in geom::flatten_polylines(&wp, 0.25) {
                        let mut run = vec![];
                        for p in pts {
                            if r.contains(p) {
                                run.push(p);
                            } else if run.len() >= 2 {
                                np.subpaths.extend(crate::path::polyline(&run, false).subpaths);
                                run.clear();
                            } else {
                                run.clear();
                            }
                        }
                        if run.len() >= 2 {
                            np.subpaths.extend(crate::path::polyline(&run, false).subpaths);
                        }
                    }
                    np.to_bez()
                };
                if let Some(t) = doc.find_mut(id) {
                    t.kind = ObjKind::Path(NodePath::from_bez(&res));
                    t.transform = Affine::IDENTITY;
                }
            }
        }
    }
}

/// Fit text to path: attach a text object to a curve.
pub fn fit_text_to_path(doc: &mut Document, text_id: ObjId, path_id: ObjId) -> bool {
    let Some(po) = doc.find(path_id).cloned() else { return false };
    let wp = po.world_path();
    let Some(t) = doc.find_mut(text_id) else { return false };
    let inv = t.transform.inverse();
    let local = NodePath::from_bez(&(inv * wp));
    if let ObjKind::Text(tx) = &mut t.kind {
        tx.on_path = Some(crate::text::TextOnPath { path: local, offset: 0.0, distance: 0.0, flip: false });
        tx.kind = crate::text::TextKind::Artistic;
        return true;
    }
    false
}

/// Join curves: merge open subpaths whose ends are within `tolerance`.
pub fn join_curves(doc: &mut Document, ids: &[ObjId], tolerance: f64) -> Option<ObjId> {
    let combined = if ids.len() > 1 { combine(doc, ids)? } else { *ids.first()? };
    let o = doc.find_mut(combined)?;
    if let ObjKind::Path(np) = &mut o.kind {
        loop {
            let mut merged = false;
            let n = np.subpaths.len();
            'outer: for a in 0..n {
                if np.subpaths[a].closed {
                    continue;
                }
                let alen = np.subpaths[a].nodes.len();
                // close itself?
                if alen > 2 {
                    let first = np.subpaths[a].nodes[0].p;
                    let last = np.subpaths[a].nodes[alen - 1].p;
                    if (first - last).hypot() <= tolerance {
                        np.join_nodes((a, 0), (a, alen - 1));
                        merged = true;
                        break 'outer;
                    }
                }
                for b in 0..n {
                    if a == b || np.subpaths[b].closed {
                        continue;
                    }
                    let blen = np.subpaths[b].nodes.len();
                    for &ea in &[0, alen - 1] {
                        for &eb in &[0, blen - 1] {
                            let pa = np.subpaths[a].nodes[ea].p;
                            let pb = np.subpaths[b].nodes[eb].p;
                            if (pa - pb).hypot() <= tolerance {
                                np.join_nodes((a, ea), (b, eb));
                                merged = true;
                                break 'outer;
                            }
                        }
                    }
                }
            }
            if !merged {
                break;
            }
        }
    }
    Some(combined)
}

/// Mirror selection about its center.
pub fn mirror(doc: &mut Document, ids: &[ObjId], horizontal: bool) {
    if let Some(b) = doc.selection_bounds(ids) {
        let c = b.center();
        let a = Affine::translate(c.to_vec2()) * if horizontal { Affine::scale_non_uniform(-1.0, 1.0) } else { Affine::scale_non_uniform(1.0, -1.0) } * Affine::translate(-c.to_vec2());
        doc.transform_objects(ids, a);
    }
}
