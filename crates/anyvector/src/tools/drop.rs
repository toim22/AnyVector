//! Copies made while dragging with the Pick tool, as in CorelDRAW. Space leaves a copy where the
//! dragged objects are now and the drag goes on (#130). A drag with the right button ends in a
//! menu: move or copy the objects there, give the object under the pointer their fill, outline or
//! all their properties, or place them inside it (#140).

use super::interact::xform_affine;
use super::Drag;
use crate::app::AnyVectorApp;
use kurbo::{Affine, Vec2};
use vector_core::{ops, Document, ObjId, ObjKind, Object};

/// How far (screen pixels) the right button must move before a press is a drag and not a click
/// (egui's own `max_click_dist`, so a right-click still opens the right-click menu).
pub const RIGHT_DRAG_DIST: f32 = 6.0;

/// A right-button drag in progress.
#[derive(Clone, Copy, Debug)]
pub struct RightGrab {
    /// The object the selection was grabbed by: the one whose properties are copied.
    pub obj: ObjId,
    /// Where the button went down (screen).
    pub press: egui::Pos2,
    /// The pointer went farther than [`RIGHT_DRAG_DIST`].
    pub far: bool,
}

/// A right-button drag that ended: what the menu works on.
#[derive(Clone, Debug)]
pub struct RightDrop {
    /// The dragged objects (the selection) and the one they were grabbed by.
    pub ids: Vec<ObjId>,
    pub source: ObjId,
    /// Where the drag puts them.
    pub affine: Affine,
    /// The object under the pointer at the release, other than the dragged ones.
    pub target: Option<ObjId>,
    /// Where the menu opens (screen).
    pub at: egui::Pos2,
    /// The menu was opened (set by [`drop_menu`]).
    pub shown: bool,
}

/// One row of the menu at the end of a right-button drag.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DropAction {
    Move,
    Copy,
    CopyFill,
    CopyOutline,
    CopyAll,
    PowerClip,
}

impl DropAction {
    /// The reference's caption.
    pub fn caption(self) -> &'static str {
        match self {
            DropAction::Move => "&Move Here",
            DropAction::Copy => "&Copy Here",
            DropAction::CopyFill => "Copy &Fill Here",
            DropAction::CopyOutline => "Copy &Outline Here",
            DropAction::CopyAll => "Copy &All Properties",
            DropAction::PowerClip => "PowerClip &Inside",
        }
    }

    /// The menu's rows for a drop: Move and Copy anywhere, the property rows and PowerClip over
    /// another object (Cancel follows them).
    pub fn rows(drop: &RightDrop) -> &'static [DropAction] {
        use DropAction::*;
        if drop.target.is_some() {
            &[Move, Copy, CopyFill, CopyOutline, CopyAll, PowerClip]
        } else {
            &[Move, Copy]
        }
    }
}

/// Which properties Copy Properties gives the targets.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Props {
    /// The outline pen (width, style, corners, arrows…) without its color.
    pub pen: bool,
    /// The outline color.
    pub color: bool,
    pub fill: bool,
    /// The character and paragraph formatting (text to text only).
    pub text: bool,
}

impl Props {
    pub const FILL: Props = Props { pen: false, color: false, fill: true, text: false };
    pub const OUTLINE: Props = Props { pen: true, color: true, fill: false, text: false };
    pub const ALL: Props = Props { pen: true, color: true, fill: true, text: true };
}

/// The object whose style stands for `o`: a group's first object, a PowerClip's frame, a blend's
/// start.
fn style_source(o: &Object) -> &Object {
    match &o.kind {
        ObjKind::Group(c) => c.first().map(style_source).unwrap_or(o),
        ObjKind::PowerClip { frame, .. } => style_source(frame),
        ObjKind::Blend(b) => style_source(&b.start),
        _ => o,
    }
}

/// Run `f` on the objects of `o` that carry a style: the objects of a group, a PowerClip's frame,
/// the ends of a blend, or `o` itself.
fn each_styled(o: &mut Object, f: &mut dyn FnMut(&mut Object)) {
    match &mut o.kind {
        ObjKind::Group(c) => c.iter_mut().for_each(|x| each_styled(x, f)),
        ObjKind::PowerClip { frame, .. } => each_styled(frame, f),
        ObjKind::Blend(b) => {
            each_styled(&mut b.start, f);
            each_styled(&mut b.end, f);
        }
        _ => f(o),
    }
}

/// Give `targets` the properties `what` of `src` (Edit ▸ Copy Properties From, and the right-drag
/// menu's Copy Fill / Outline / All Properties Here).
pub fn copy_properties(doc: &mut Document, src: &Object, targets: &[ObjId], what: Props) {
    let src = style_source(src);
    for id in targets {
        let Some(o) = doc.find_mut(*id) else { continue };
        if o.locked {
            continue;
        }
        each_styled(o, &mut |o| {
            if what.pen {
                let c = o.style.outline.color;
                o.style.outline = src.style.outline.clone();
                o.style.outline.color = c;
            }
            if what.color {
                o.style.outline.color = src.style.outline.color;
            }
            if what.fill {
                o.style.fill = src.style.fill.clone();
            }
            if what.text {
                if let (ObjKind::Text(dst), ObjKind::Text(from)) = (&mut o.kind, &src.kind) {
                    dst.format = from.format.clone();
                    dst.spans.clear();
                    dst.align = from.align.clone();
                    dst.paragraphs = from.paragraphs.clone();
                    dst.line_spacing = from.line_spacing;
                }
            }
        });
    }
}

/// The dragged objects that can move (not locked).
fn movable(doc: &Document, ids: &[ObjId]) -> Vec<ObjId> {
    ids.iter().copied().filter(|id| doc.find(*id).is_some_and(|o| !o.locked)).collect()
}

/// Space during a Pick drag (move, stretch, rotate, skew, Free Transform): a copy of the dragged
/// objects where the drag has them now, in one undo step; the drag goes on with the originals.
/// Gives whether a copy was made.
pub fn stamp_copy(app: &mut AnyVectorApp) -> bool {
    let Drag::Transform { kind, start, bb, ids, center } = &app.ts.drag else { return false };
    let a = xform_affine(*kind, *start, app.ts.cur, *bb, *center, app.ts.mods, app.settings.constrain_angle);
    let ids = ids.clone();
    let Some(d) = app.active.and_then(|i| app.docs.get_mut(i)) else { return false };
    let ids = movable(&d.doc, &ids);
    if ids.is_empty() {
        return false;
    }
    let copies = d.modify("Duplicate", |doc, _| {
        let c = ops::duplicate(doc, &ids, Vec2::ZERO);
        doc.transform_objects(&c, a);
        c
    });
    app.ts.stamps.extend(copies);
    true
}

/// Carry out a choice of the right-drag menu, as one undo step.
pub fn apply(app: &mut AnyVectorApp, drop: &RightDrop, action: DropAction) {
    let Some(d) = app.active.and_then(|i| app.docs.get_mut(i)) else { return };
    let ids = movable(&d.doc, &drop.ids);
    let a = drop.affine;
    match action {
        DropAction::Move | DropAction::Copy => {
            if ids.is_empty() {
                return;
            }
            let copy = action == DropAction::Copy;
            d.modify(if copy { "Copy" } else { "Move" }, |doc, sel| {
                let targets = if copy { ops::duplicate(doc, &ids, Vec2::ZERO) } else { ids.clone() };
                doc.transform_objects(&targets, a);
                *sel = targets;
            });
            // a moved rotation center goes along with its objects (or their copies)
            if let Some((cids, c)) = d.rot_center.take() {
                if cids == ids {
                    d.rot_center = Some((d.selection.clone(), a * c));
                }
            }
        }
        DropAction::CopyFill | DropAction::CopyOutline | DropAction::CopyAll => {
            let (Some(t), Some(src)) = (drop.target, d.doc.find(drop.source).cloned()) else { return };
            let what = match action {
                DropAction::CopyFill => Props::FILL,
                DropAction::CopyOutline => Props::OUTLINE,
                _ => Props::ALL,
            };
            // the dragged objects stay where they were; the selection does not change
            d.modify("Copy Properties", |doc, _| copy_properties(doc, &src, &[t], what));
        }
        DropAction::PowerClip => {
            let Some(t) = drop.target else { return };
            if ids.is_empty() {
                return;
            }
            // the contents go in where they were dropped
            d.modify("PowerClip Inside", |doc, sel| {
                doc.transform_objects(&ids, a);
                if let Some(id) = ops::powerclip(doc, &ids, t) {
                    *sel = vec![id];
                }
            });
        }
    }
}

/// The menu at the end of a right-button drag, at the pointer, while `ts.right_drop` is set.
/// Escape, Cancel or a click elsewhere closes it and nothing changes.
pub fn drop_menu(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(drop) = app.ts.right_drop.clone() else { return };
    let id = egui::Id::unique("right_drag_drop_menu");
    let mut chosen = None;
    egui::Popup::new(id, ctx.clone(), egui::PopupAnchor::Position(drop.at), egui::LayerId::new(egui::Order::Foreground, id.with("layer")))
        .kind(egui::PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .style(egui::containers::menu::menu_style)
        .open_memory((!drop.shown).then_some(egui::SetOpenCommand::Bool(true)))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            crate::menus::menu_style(ui);
            ui.set_min_width(200.0);
            for a in DropAction::rows(&drop) {
                if crate::menus::item(app, ui, a.caption(), true, false) {
                    chosen = Some(*a);
                    ui.close();
                }
            }
            crate::menus::separator(ui);
            if crate::menus::item(app, ui, "&Cancel", true, false) {
                ui.close();
            }
        });
    if let Some(a) = chosen {
        apply(app, &drop, a);
    }
    if chosen.is_some() || (drop.shown && !egui::Popup::is_id_open(ctx, id)) {
        app.ts.right_drop = None;
    } else if let Some(d) = app.ts.right_drop.as_mut() {
        d.shown = true;
    }
}
