//! The advanced blend commands (#145): Path Properties (New Path, Show Path, Detach from Path),
//! Start and End Properties (New Start, Show Start, New End, Show End), and the Options of the
//! Blend property bar and docker (Map nodes, Split, Fuse start, Fuse end, Blend along full path,
//! Rotate all objects, fixed spacing).
//!
//! The items are property bar options ([`KEYS`]), so the property bar, the Effects ▸ Blend
//! flyouts, the right-click menus and the Blend docker share their enabled and checked states.
//! The commands that need an object clicked (New Path, New Start, New End, Split, Map nodes) set
//! a [`PickTarget::Blend`]; with the blend and that object both selected they apply at once.
//!
//! A part of a blend shown with Show Start, Show End or Show Path (or a split object) is selected
//! on its own: [`focus`] is that part while it is the whole selection. It can be moved, sized,
//! deleted and edited like any object; the blend follows it, and on a path the start, end and
//! splits stay on the path.
//!
//! The parts are picked on the canvas too (#149): with the blend selected, a click on its start,
//! end, path or a split object selects that part (the canvas outlines them and puts a grip on
//! each, [`grips`]), and Ctrl+click selects one straight away, as inside a group (#151).

use crate::app::AnyVectorApp;
use crate::docview::DocView;
use crate::theme;
use crate::tools::PickTarget;
use egui::{Painter, Stroke};
use kurbo::Point;
use vector_core::blend_path::{self as bp, BlendError, Fuse};
use vector_core::object::{BlendObj, BlendPart};
use vector_core::{Document, ObjId, ObjKind};

/// A blend command, by the reference id of its item.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlendCmd {
    NewPath,
    ShowPath,
    DetachPath,
    NewStart,
    ShowStart,
    NewEnd,
    ShowEnd,
    MapNodes,
    Split,
    FuseStart,
    FuseEnd,
    FullPath,
    RotateAll,
}

/// The items of the blend commands: the first 8 hex digits of their reference ids.
pub const KEYS: &[(&str, BlendCmd)] = &[
    ("42941bfb", BlendCmd::NewPath),
    ("050d45c6", BlendCmd::ShowPath),
    ("3726add2", BlendCmd::DetachPath),
    ("1e32e855", BlendCmd::NewStart),
    ("7341835a", BlendCmd::ShowStart),
    ("db7791fd", BlendCmd::NewEnd),
    ("1a1bed58", BlendCmd::ShowEnd),
    ("c5cd624e", BlendCmd::MapNodes),
    ("339589f8", BlendCmd::Split),
    ("e022c80a", BlendCmd::FuseStart),
    ("ba9b75cf", BlendCmd::FuseEnd),
    ("247d0e09", BlendCmd::FullPath),
    ("50c0c56f", BlendCmd::RotateAll),
];

/// The command of the item `key`.
pub fn cmd_of(key: &str) -> Option<BlendCmd> {
    KEYS.iter().find(|(k, _)| *k == key).map(|(_, c)| *c)
}

/// The key of the command `c` (the inverse of [`cmd_of`]).
pub fn key_of(c: BlendCmd) -> &'static str {
    KEYS.iter().find(|(_, x)| *x == c).map(|(k, _)| *k).unwrap_or("")
}

/// Whether `key` is one of the blend commands.
pub fn handles(key: &str) -> bool {
    cmd_of(key).is_some()
}

/// What the canvas waits for after a blend command.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Pick {
    /// New Path: the curve or shape the blend follows.
    Path { blend: ObjId },
    /// New Start / New End: the object that becomes the start (or end) of the blend.
    Control { blend: ObjId, end: bool },
    /// Split: the step the blend splits at.
    Split { blend: ObjId },
    /// Map nodes: a node of the start object, then the matching node of the end object.
    MapNodes { blend: ObjId, first: Option<(usize, f64)> },
}

/// The status bar hint of a pick in progress.
pub fn hint(p: Pick) -> &'static str {
    match p {
        Pick::Path { .. } => "Click the path the blend follows (Esc to cancel)",
        Pick::Control { end: false, .. } => "Click the new start object of the blend (Esc to cancel)",
        Pick::Control { end: true, .. } => "Click the new end object of the blend (Esc to cancel)",
        Pick::Split { .. } => "Click the step of the blend where it splits (Esc to cancel)",
        Pick::MapNodes { first: None, .. } => "Click a node of the start object (Esc to cancel)",
        Pick::MapNodes { first: Some(_), .. } => "Click the matching node of the end object (Esc to cancel)",
    }
}

fn message(e: BlendError) -> &'static str {
    match e {
        BlendError::NotABlend => "Select a blend first",
        BlendError::OwnPart => "Click an object that is not part of the blend",
        BlendError::NotAPath => "Click a curve or a shape to use as the path",
        BlendError::TooShort => "The path has no length",
        BlendError::NotAControl => "Groups, bitmaps and blends can't start or end a blend",
    }
}

// ------------------------------------------------------------------------------ the target

/// The part of a blend selected on its own (Show Start, Show End, Show Path, a split object):
/// (blend, part), while it is the whole selection.
pub fn focus(d: &DocView) -> Option<(ObjId, BlendPart)> {
    let [id] = d.selection.as_slice() else { return None };
    bp::part_of(&d.doc, *id)
}

/// The blend the commands act on: the blend selected, the blend of the part selected, or the
/// blend of a selected pair (a blend and the object to use with it).
pub fn target_in(d: &DocView) -> Option<ObjId> {
    match d.selection.as_slice() {
        [id] => bp::blend_of(&d.doc, *id).map(|_| *id).or_else(|| focus(d).map(|(b, _)| b)),
        [a, b] => [*a, *b].into_iter().find(|id| bp::blend_of(&d.doc, *id).is_some()),
        _ => None,
    }
}

/// The other object of a selected pair (a blend and the object to use with it).
fn other_in(d: &DocView, blend: ObjId) -> Option<ObjId> {
    match d.selection.as_slice() {
        [a, b] if *a == blend => Some(*b),
        [a, b] if *b == blend => Some(*a),
        _ => None,
    }
}

/// The blend the commands act on, and its settings.
pub fn target(app: &AnyVectorApp) -> Option<(ObjId, BlendObj)> {
    let d = app.doc()?;
    let id = target_in(d)?;
    Some((id, bp::blend_of(&d.doc, id)?.clone()))
}

/// Change the target blend as one undoable step (a property bar slider drag is one step).
pub fn edit(app: &mut AnyVectorApp, label: &str, f: impl FnOnce(&mut BlendObj)) {
    let live = app.ts.live_slider.take();
    let Some(d) = app.doc_mut() else { return };
    let Some(id) = target_in(d) else { return };
    let ed = move |doc: &mut Document, _: &mut Vec<ObjId>| {
        if let Some(ObjKind::Blend(b)) = doc.find_mut(id).map(|o| &mut o.kind) {
            f(b);
        }
    };
    match live {
        Some(key) => d.modify_live(key, label, ed),
        None => d.modify(label, ed),
    }
}

/// Run `f` on a copy of the document and keep it, as one undoable step, only if it succeeds:
/// a refused command leaves no empty undo step behind.
fn change(d: &mut DocView, label: &str, f: impl FnOnce(&mut Document, &mut Vec<ObjId>) -> Result<(), &'static str>) -> Result<(), &'static str> {
    let mut doc = d.doc.clone();
    let mut sel = d.selection.clone();
    f(&mut doc, &mut sel)?;
    d.checkpoint(label);
    d.doc = doc;
    d.selection = sel;
    d.after_change();
    Ok(())
}

// ------------------------------------------------------------------------- states and runs

/// Enabled and checked, for the item `key` (None when it isn't a blend command).
pub fn option_state(app: &AnyVectorApp, key: &str) -> Option<(bool, bool)> {
    let c = cmd_of(key)?;
    let Some((_, b)) = target(app) else { return Some((false, false)) };
    let path = b.path.as_ref();
    Some(match c {
        BlendCmd::NewPath | BlendCmd::NewStart | BlendCmd::NewEnd | BlendCmd::ShowStart | BlendCmd::ShowEnd => (true, false),
        BlendCmd::ShowPath | BlendCmd::DetachPath => (path.is_some(), false),
        BlendCmd::MapNodes => (!bp::map_candidates(&b.start).is_empty() && !bp::map_candidates(&b.end).is_empty(), false),
        BlendCmd::Split => (bp::effective_steps(&b) > 0, false),
        BlendCmd::FuseStart | BlendCmd::FuseEnd => (!b.splits.is_empty(), false),
        BlendCmd::FullPath => (path.is_some(), path.is_some_and(|p| p.full)),
        BlendCmd::RotateAll => (path.is_some(), path.is_some_and(|p| p.rotate)),
    })
}

/// Press the item `key`.
pub fn press(app: &mut AnyVectorApp, key: &str) {
    if let Some(c) = cmd_of(key) {
        run(app, c);
    }
}

/// Run the blend command `c` on the target blend.
pub fn run(app: &mut AnyVectorApp, c: BlendCmd) {
    let Some((blend, b)) = target(app) else {
        app.flash(message(BlendError::NotABlend));
        return;
    };
    if option_state(app, key_of(c)).is_some_and(|(on, _)| !on) {
        return;
    }
    let Some(d) = app.doc_mut() else { return };
    let other = other_in(d, blend);
    let mut out: Result<(), &'static str> = Ok(());
    let wait = |app: &mut AnyVectorApp, p: Pick| app.ts.pick_target = Some(PickTarget::Blend(p));
    match c {
        BlendCmd::NewPath | BlendCmd::NewStart | BlendCmd::NewEnd => {
            let p = match c {
                BlendCmd::NewPath => Pick::Path { blend },
                _ => Pick::Control { blend, end: c == BlendCmd::NewEnd },
            };
            match other {
                Some(o) => out = apply(d, p, o),
                None => wait(app, p),
            }
        }
        BlendCmd::ShowPath | BlendCmd::ShowStart | BlendCmd::ShowEnd => {
            let part = match c {
                BlendCmd::ShowPath => BlendPart::Path,
                BlendCmd::ShowStart => BlendPart::Start,
                _ => BlendPart::End,
            };
            if let Some(o) = b.part(part) {
                d.selection = vec![o.id];
                d.rotate_mode = false;
                d.shadow_sel = None;
                d.node_sel.clear();
            }
        }
        BlendCmd::DetachPath => {
            out = change(d, "Detach from Path", |doc, sel| {
                bp::detach_path(doc, blend).ok_or(message(BlendError::NotAPath))?;
                *sel = vec![blend];
                Ok(())
            })
        }
        BlendCmd::MapNodes => wait(app, Pick::MapNodes { blend, first: None }),
        BlendCmd::Split => wait(app, Pick::Split { blend }),
        BlendCmd::FuseStart | BlendCmd::FuseEnd => {
            let (label, which) = if c == BlendCmd::FuseStart { ("Fuse start", Fuse::First) } else { ("Fuse end", Fuse::Last) };
            out = change(d, label, |doc, sel| {
                if !bp::fuse(doc, blend, which) {
                    return Err("The blend has no split to fuse");
                }
                *sel = vec![blend];
                Ok(())
            })
        }
        BlendCmd::FullPath => {
            let on = !b.path.as_ref().is_some_and(|p| p.full);
            out = change(d, "Blend along full path", |doc, _| if bp::set_full_path(doc, blend, on) { Ok(()) } else { Err("") });
        }
        BlendCmd::RotateAll => {
            let on = !b.path.as_ref().is_some_and(|p| p.rotate);
            out = change(d, "Rotate all objects", |doc, _| if bp::set_rotate_all(doc, blend, on) { Ok(()) } else { Err("") });
        }
    }
    if let Err(m) = out {
        if !m.is_empty() {
            app.flash(m);
        }
    }
}

/// Use the object `obj` for the pick `p` (New Path, New Start, New End).
fn apply(d: &mut DocView, p: Pick, obj: ObjId) -> Result<(), &'static str> {
    match p {
        Pick::Path { blend } => change(d, "New Path", |doc, sel| {
            bp::set_path(doc, blend, obj).map_err(message)?;
            *sel = vec![blend];
            Ok(())
        }),
        Pick::Control { blend, end } => change(d, if end { "New End" } else { "New Start" }, |doc, sel| {
            bp::replace_control(doc, blend, obj, end).map_err(message)?;
            *sel = vec![blend];
            Ok(())
        }),
        Pick::Split { .. } | Pick::MapNodes { .. } => Ok(()),
    }
}

/// What a click on the canvas did to a blend pick.
#[derive(Debug, PartialEq)]
pub enum Picked {
    Done,
    /// The pick goes on (Map nodes: the end object's node is next).
    Again(Pick),
    /// The click was refused, and why.
    Flash(&'static str),
}

/// A click at `p` for the pick `pick`; `hit` is the object under the pointer.
pub fn pick_click(d: &mut DocView, pick: Pick, p: Point, hit: Option<ObjId>, tol: f64) -> Picked {
    let done = |r: Result<(), &'static str>| match r {
        Ok(()) => Picked::Done,
        Err(m) => Picked::Flash(m),
    };
    match pick {
        Pick::Path { .. } | Pick::Control { .. } => match hit {
            Some(t) => done(apply(d, pick, t)),
            None => Picked::Flash("No object was clicked"),
        },
        Pick::Split { blend } => {
            let Some(k) = bp::blend_of(&d.doc, blend).and_then(|b| bp::step_at(b, p, tol)) else {
                return Picked::Flash("Click a step of the blend");
            };
            done(change(d, "Split", |doc, sel| {
                let id = bp::split(doc, blend, k).ok_or("Click a step of the blend")?;
                *sel = vec![id];
                Ok(())
            }))
        }
        Pick::MapNodes { blend, first } => {
            let Some(b) = bp::blend_of(&d.doc, blend) else { return Picked::Done };
            match first {
                None => match bp::map_node_at(&b.start, p, tol * 1.5) {
                    Some(n) => Picked::Again(Pick::MapNodes { blend, first: Some(n) }),
                    None => Picked::Flash("Click a node of the start object"),
                },
                Some((si, fa)) => match bp::map_node_at(&b.end, p, tol * 1.5) {
                    Some((sj, fb)) if sj == si => done(change(d, "Map nodes", |doc, _| if bp::map_nodes(doc, blend, si, fa, fb) { Ok(()) } else { Err("") })),
                    _ => Picked::Flash("Click the matching node of the end object"),
                },
            }
        }
    }
}

/// Delete the part of a blend selected on its own: the start or end takes the blend apart, the
/// path leaves the blend, a split is fused back.
pub fn delete(app: &mut AnyVectorApp, blend: ObjId, part: BlendPart) {
    let Some(d) = app.doc_mut() else { return };
    let r = change(d, "Delete", |doc, sel| {
        *sel = bp::delete_part(doc, blend, part).ok_or("")?;
        Ok(())
    });
    if let Err(m) = r {
        if !m.is_empty() {
            app.flash(m);
        }
    }
}

/// A fixed spacing that keeps the blend as it is: the distance between its steps now.
pub fn spacing_now(b: &BlendObj) -> Option<f64> {
    if let Some(s) = b.spacing {
        return Some(s);
    }
    let (st, ss) = bp::control_stations(b)?;
    let span = (ss[ss.len() - 1] - ss[0]).abs();
    let span = if span > 1e-6 { span } else { st.total() };
    Some(span / (b.steps.max(1) + 1) as f64)
}

/// The status bar text of a part selected on its own: "Blend start: Ellipse".
pub fn part_info(d: &DocView) -> Option<String> {
    let (blend, part) = focus(d)?;
    let o = bp::blend_of(&d.doc, blend)?.part(part)?;
    let layer = d.doc.top_level_of(blend).and_then(|t| d.doc.locate(t)).map(|(p, l, _)| d.doc.pages[p].layers[l].name.clone()).unwrap_or_default();
    let name = o.display_name();
    Some(match part {
        BlendPart::Start => format!("Blend start: {name} on {layer}"),
        BlendPart::End => format!("Blend end: {name} on {layer}"),
        BlendPart::Path => format!("Blend path: {name} on {layer}"),
        BlendPart::Split(_) => format!("Blend split: {name} on {layer}"),
    })
}

// ------------------------------------------------------------------------------ the canvas

/// The blend whose controls a click picks (#149): the blend selected on its own, or the blend of
/// the part selected; none when it is locked (or what it is in is), or its layer is locked or
/// hidden.
pub fn context(d: &DocView) -> Option<ObjId> {
    let [id] = d.selection.as_slice() else { return None };
    let blend = if bp::blend_of(&d.doc, *id).is_some() { *id } else { focus(d)?.0 };
    if d.doc.find(blend)?.locked || !d.doc.locked_within(&[blend]).is_empty() {
        return None;
    }
    let (p, l, _) = d.doc.locate(d.doc.top_level_of(blend)?)?;
    let layer = &d.doc.pages[p].layers[l];
    (layer.visible && !layer.locked).then_some(blend)
}

/// The side of a control's grip on the canvas, in screen pixels.
pub const GRIP: f32 = 7.0;

/// The controls of the blend in [`context`] that show a grip, the selected one left out: the
/// start, the end and the split objects, each with its grip (the middle of its box, on screen).
/// A click on a control or on its grip selects it (#149).
pub fn grips(d: &DocView) -> Vec<(BlendPart, ObjId, egui::Pos2)> {
    let Some(b) = context(d).and_then(|blend| bp::blend_of(&d.doc, blend)) else { return vec![] };
    let mut parts = vec![BlendPart::Start, BlendPart::End];
    parts.extend((0..b.splits.len()).map(BlendPart::Split));
    parts
        .into_iter()
        .filter_map(|part| {
            let o = b.part(part)?;
            (o.visible && !d.selection.contains(&o.id)).then(|| (part, o.id, d.to_screen(d.doc.bounds_of(o).center())))
        })
        .collect()
}

/// The grip under the screen position `pos` (see [`grips`]): the control's part and id.
pub fn grip_at(d: &DocView, pos: egui::Pos2) -> Option<(BlendPart, ObjId)> {
    let reach = GRIP / 2.0 + 2.0;
    grips(d).into_iter().rev().find(|(_, _, c)| (c.x - pos.x).abs() <= reach && (c.y - pos.y).abs() <= reach).map(|(part, id, _)| (part, id))
}

/// While controls of blends are dragged (`ids`, moved by `a`): each blend as it will be, its
/// steps outlined in `color`, so the blend is seen to follow (#149). A blend on a path keeps its
/// controls on the path, as the drop will.
pub fn draw_drag_preview(painter: &Painter, d: &DocView, ids: &[ObjId], a: kurbo::Affine, color: egui::Color32) {
    if ids.len() > 8 {
        return;
    }
    let ids = d.doc.outermost(ids);
    let mut moved: Vec<(ObjId, BlendObj, BlendObj)> = vec![];
    for id in &ids {
        let Some((blend, part)) = bp::part_of(&d.doc, *id) else { continue };
        if ids.contains(&blend) {
            continue;
        }
        if !moved.iter().any(|(b, _, _)| *b == blend) {
            let Some(b) = bp::blend_of(&d.doc, blend) else { continue };
            moved.push((blend, b.clone(), b.clone()));
        }
        if let Some((_, _, b)) = moved.iter_mut().find(|(b, _, _)| *b == blend) {
            if let Some(o) = b.part_mut(part) {
                o.apply_transform(a);
            }
        }
    }
    for (_, before, mut b) in moved {
        if b.path.is_some() {
            bp::settle(&mut b, &before);
        }
        for s in bp::steps(&b, &mut || 0) {
            crate::tools::overlay::stroke_bez(painter, d, &s.world_path(), Stroke::new(1.0, color));
        }
    }
}

/// The part shown, outlined; the controls of the blend selected, outlined with a grip each, and
/// its path dashed (#149), with `show_grips` (the Pick tool, no drag going on); while Map nodes waits
/// for a click, the nodes it can match.
pub fn draw_overlay(painter: &Painter, d: &DocView, pick: Option<PickTarget>, show_grips: bool) {
    if let Some((blend, part)) = focus(d) {
        if let Some(o) = bp::blend_of(&d.doc, blend).and_then(|b| b.part(part)) {
            crate::tools::overlay::stroke_bez(painter, d, &o.world_path(), Stroke::new(1.0, theme::SELECTION));
        }
    }
    if let Some(b) = context(d).filter(|_| show_grips).and_then(|blend| bp::blend_of(&d.doc, blend)) {
        let dim = theme::SELECTION.gamma_multiply(0.6);
        if let Some(path) = b.path.as_ref().filter(|x| x.obj.visible && !d.selection.contains(&x.obj.id)) {
            crate::tools::overlay::dashed_bez(painter, d, &path.obj.world_path(), dim);
        }
        for (part, _, c) in grips(d) {
            if let Some(o) = b.part(part) {
                crate::tools::overlay::stroke_bez(painter, d, &o.world_path(), Stroke::new(1.0, dim));
            }
            let r = egui::Rect::from_center_size(c, egui::vec2(GRIP, GRIP));
            painter.rect_filled(r, 0.0, egui::Color32::WHITE);
            painter.rect_stroke(r, 0.0, Stroke::new(1.5, theme::SELECTION), egui::StrokeKind::Inside);
        }
    }
    let Some(PickTarget::Blend(Pick::MapNodes { blend, first })) = pick else { return };
    let Some(b) = bp::blend_of(&d.doc, blend) else { return };
    let node = |o: &vector_core::Object, only: Option<(usize, f64)>, filled: bool| {
        for (q, si, f) in bp::map_candidates(o) {
            let chosen = only.is_some_and(|(s, g)| s == si && (g - f).abs() < 1e-9);
            let c = d.to_screen(q);
            let r = egui::Rect::from_center_size(c, egui::vec2(7.0, 7.0));
            painter.rect_filled(r, 0.0, if chosen || filled { theme::SELECTION } else { egui::Color32::WHITE });
            painter.rect_stroke(r, 0.0, Stroke::new(1.0, theme::SELECTION), egui::StrokeKind::Inside);
        }
    };
    node(&b.start, first, false);
    if first.is_some() {
        node(&b.end, None, false);
    }
}
