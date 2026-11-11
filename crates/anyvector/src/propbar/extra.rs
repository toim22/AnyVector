//! The property bar items of the tools' options (Knife, Free Transform, eyedroppers, brushes,
//! interactive effects, presets, dimensions, anchors, tables, Painterly Brush, Variable Outline
//! and Impact), as on the reference bars.

use super::*;
use crate::settings::ToolOpts;
use crate::tools::PickTarget;
use vector_core::object::{DimensionObj, ObjAnchor, TextWrap};
use vector_core::style::{ArrowHead, MeshFill, PaintOpts, VarWidth, WidthProfile};

// ----------------------------------------------------------------------------- catalogs

/// The Painterly Brush styles: one per category of the reference brush picker (name, bristles,
/// jitter, kind of mark: 0 bristles, 1 airbrush droplets, 2 particles, 3 solid ink).
pub const BRUSHES: &[(&str, u32, f64, u8)] = &[
    ("Acrylics", 18, 0.3, 0),
    ("Airbrushes", 1, 0.0, 1),
    ("Charcoal", 10, 0.9, 0),
    ("Digital Effects", 6, 0.2, 2),
    ("Fluid", 1, 0.0, 3),
    ("Glazing", 20, 0.15, 0),
    ("Gouache", 16, 0.25, 0),
    ("Inks", 1, 0.0, 3),
    ("Markers", 8, 0.05, 0),
    ("Oils", 22, 0.45, 0),
    ("Particles", 1, 0.0, 2),
    ("Pastels & Chalks", 12, 1.0, 0),
    ("Pencils", 4, 0.6, 0),
    ("Sponges", 1, 0.0, 1),
    ("Watercolor", 24, 0.5, 0),
];

/// The Painterly Brush textures.
const TEXTURES: &[&str] = &["Basic Paper"];

/// Wrap Paragraph Text styles (see `TextWrap::style`).
const WRAP_STYLES: &[(&str, &str)] = &[
    ("Contour - Follow", "Text flow left"),
    ("Contour - Follow", "Text flow right"),
    ("Contour - Follow", "Straddle text"),
    ("Square", "Text flow left"),
    ("Square", "Text flow right"),
    ("Square", "Straddle text"),
    ("Square", "Above/below"),
];

/// Interactive effects with presets: (name of the preset list, add button, delete button).
const PRESET_LISTS: &[(&str, &str, &str, &str)] = &[
    ("b61b23b8", "2742f968", "4933ce7e", "Blend"),
    ("5d0f070b", "65a24647", "3b31083e", "Contour"),
    ("6af525e1", "e7e3e939", "bb990f92", "Distortion"),
    ("dd7baf42", "cb92c441", "b3e06fa1", "Drop Shadow"),
    ("b43a901b", "f08777f1", "7a004a12", "Envelope"),
    ("60c9692d", "6238a70e", "3d684904", "Extrude"),
];

/// The buttons handled here (first 8 characters of the ids).
pub const OPTIONS: &[&str] = &[
    // copy fill, transparency, mesh, blend, contour, distortion, shadow, envelope, extrude,
    // variable outline; create envelope from; edit fill
    "2a8b936e", "47c44ef5", "a540f78f", "692bc9d1", "9a9dead0", "c3f54953", "2c99de2c", "1f8f2496", "1d271da3", "70e110e8", "1a21e45c", "3d9ac113",
    // anchors: adjust direction, relative to object, auto anchor, delete
    "5e1fc86c", "5024706a", "8a929185", "f6d38abb",
    // dimensions: dynamic, leading zero, show units, successive
    "fdaf006a", "0cf00463", "91a0ffc5", "d3154914",
    // 2-point line modes; bounding box; parallel drawing
    "cf3843d7", "5e89d7ab", "7ec5ed5a", "3b408305", "a1df5fa2",
    // painterly: recent brushes, clear brushstroke, glow, reset, simulate pressure
    "53ad9d10", "95fe5c94", "eac788fd", "19fd40bd", "7d4894f7", "3860fe09", "c15728bb",
    // artistic media: scale stroke with object
    "227bb320",
    // attract, repel, pen pressure; smear smooth/pointy; twirl counterclockwise/clockwise
    "fb587552", "4e560b2f", "a45bce92", "d5b480c4", "a937dead", "90069312", "80bbe208",
    // block shadow: from outline, overprint, remove holes, simplify
    "64cc85b2", "100b6bed", "e6025a48", "e8a6d2d0",
    // shape tool: align, reflect H/V, scale, rotate and skew
    "04cdfb63", "b5180999", "952282d5", "94e2c33b", "8f8f5213",
    // eraser: bearing, pressure, tilt
    "75bcd7d6", "756973e2", "4e713b0f",
    // eyedroppers: 1x1, 2x2, 5x5, select/apply color, desktop, select/apply attributes
    "a13af0fa", "7772fd02", "fddc92f7", "8b50d9ca", "abf0e5e9", "e09ae203", "636e2e05", "b8d60a54",
    // free transform: rotate, reflect, scale, skew, apply to duplicate, relative to object
    "462130ab", "d703bd6d", "3f78578f", "abb745ee", "e7cec14f", "249bae0e",
    // blend: direct, clockwise, counterclockwise, loop, size acceleration
    "7e180fc2", "c6994e2e", "7531fe58", "add7ce56", "2c88e5ab",
    // presets: add and delete (blend, contour, distortion, shadow, envelope, extrude)
    "2742f968", "4933ce7e", "65a24647", "3b31083e", "e7e3e939", "bb990f92", "cb92c441", "b3e06fa1", "f08777f1", "7a004a12", "6238a70e", "3d684904",
    // distortion: center, add new
    "88dd3b61", "0381788e",
    // envelope: straight, single arc, double arc, unconstrained, keep lines, add new
    "a14006c6", "a63b08d2", "0b55facb", "180a4571", "a428b4e7", "68a0b923",
    // extrude: VP object/page
    "45ed8127",
    // mesh: add intersection, sample color, smooth color
    "89b0fceb", "0d1dd68e", "180f15f7",
    // knife: 2-point, freehand, Bézier, auto-close
    "39d9acd4", "68943942", "7b87d66c", "a9a56d5c",
    // livesketch: single curve, include curves, preview
    "0230228e", "b3914d92", "61575354",
    // pen: auto add-delete, preview; polyline: auto-close
    "4b0a1855", "921e4f4e", "4d4cf95b",
    // roughen: pressure, use tilt; smudge: pressure, use bearing, use tilt
    "8534142a", "16e06dc5", "01c8c86b", "034d74d6", "726cb4c9",
    // text: bullets, numbers, drop cap, increase/decrease indent, interactive OpenType
    "fa26e7e3", "e624647c", "6123599f", "a7d5c6be", "3720fa77", "5e8bb83c",
    // zoom to all pages
    "019453ed",
    // variable outline: add node, delete node, lock ratio, scale with stroke, clear nodes
    "d561296d", "1e55a1b0", "16bb3f06", "8edb8dc0", "da692f6b",
    // impact: inner and outer boundary, randomize width, randomize spacing
    "8bbef079", "572c7ba4", "d7a05774", "63df27f5",
];

// ----------------------------------------------------------------------------- helpers

fn to(app: &AnyVectorApp) -> &ToolOpts {
    &app.settings.topts
}

fn has<T>(app: &AnyVectorApp, f: impl Fn(&Object) -> Option<T>) -> bool {
    first(app, f).flatten().is_some()
}

fn copy_from(app: &mut AnyVectorApp, kind: u8) {
    if app.has_selection() {
        app.ts.pick_target = Some(PickTarget::CopyEffect { kind, clone: false });
    }
}

fn blend_now(app: &AnyVectorApp) -> Option<vector_core::object::BlendObj> {
    selected_kind(app, |k| if let ObjKind::Blend(b) = k { Some(b.clone()) } else { None })
}

fn edit_blend(app: &mut AnyVectorApp, f: impl Fn(&mut vector_core::object::BlendObj)) {
    edit_sel(app, "Blend", |o| {
        if let ObjKind::Blend(b) = &mut o.kind {
            f(b);
        }
    });
}

fn mesh_now(app: &AnyVectorApp) -> Option<MeshFill> {
    first(app, |o| if let Fill::Mesh(m) = &o.style.fill { Some(m.clone()) } else { None }).flatten()
}

/// The nodes of the selected patch of a mesh (all of them when none is selected).
fn mesh_nodes(app: &AnyVectorApp, m: &MeshFill) -> Vec<usize> {
    let n = m.cols + 1;
    match app.ts.table_cell.filter(|(r, c)| *r < m.rows && *c < m.cols) {
        Some((r, c)) => vec![r * n + c, r * n + c + 1, (r + 1) * n + c, (r + 1) * n + c + 1],
        None => (0..m.colors.len()).collect(),
    }
}

/// A palette color with the Mesh Fill tool goes to the selected patch (or every node) of the
/// selected mesh. False when the selection has no mesh fill.
pub fn mesh_palette_color(app: &mut AnyVectorApp, c: vector_core::Color) -> bool {
    if app.tool != ToolId::MeshFill || mesh_now(app).is_none() {
        return false;
    }
    edit_mesh(app, "Mesh Fill Color", |m, nodes| {
        for &i in nodes {
            if let Some(x) = m.colors.get_mut(i) {
                *x = c;
            }
        }
    });
    true
}

fn edit_mesh(app: &mut AnyVectorApp, label: &str, f: impl Fn(&mut MeshFill, &[usize])) {
    let Some(m) = mesh_now(app) else { return };
    let nodes = mesh_nodes(app, &m);
    edit_sel(app, label, |o| {
        if let Fill::Mesh(m) = &mut o.style.fill {
            f(m, &nodes);
        }
    });
}

fn dim_flag(app: &AnyVectorApp, get: fn(&DimensionObj) -> bool, dflt: fn(&ToolOpts) -> bool) -> bool {
    dimension_now(app).map(|d| get(&d)).unwrap_or_else(|| dflt(to(app)))
}

/// Set an option of the selected dimensions, or of the dimensions drawn next.
fn set_dim(app: &mut AnyVectorApp, f: impl Fn(&mut DimensionObj), t: impl FnOnce(&mut ToolOpts)) {
    if dimension_now(app).is_some() {
        edit_sel(app, "Dimension", |o| {
            if let ObjKind::Dimension(d) = &mut o.kind {
                f(d);
            }
        });
    } else {
        t(&mut app.settings.topts);
    }
}

/// The selected anchor: object, index and anchor.
fn anchor_now(app: &AnyVectorApp) -> Option<(vector_core::ObjId, usize, ObjAnchor)> {
    let (id, i) = app.ts.anchor_sel?;
    let o = app.doc()?.doc.find(id)?;
    Some((id, i, *o.effects.anchor_list().get(i)?))
}

/// Change the anchors of an object (its geometric anchors become editable ones first).
pub fn edit_anchors(app: &mut AnyVectorApp, label: &str, id: vector_core::ObjId, f: impl FnOnce(&mut Vec<ObjAnchor>)) {
    let Some(d) = app.doc_mut() else { return };
    d.modify(label, |doc, _| {
        if let Some(o) = doc.find_mut(id) {
            if o.effects.anchors.is_empty() {
                o.effects.anchors = o.effects.anchor_list();
            }
            f(&mut o.effects.anchors);
        }
        doc.update_connectors();
    });
}

fn painterly_now(app: &AnyVectorApp) -> bool {
    has(app, |o| matches!(o.style.outline.profile, Some(WidthProfile::Painterly { .. })).then_some(()))
}

/// The settings of the Painterly Brush from the property bar.
pub fn paint_opts(t: &ToolOpts) -> PaintOpts {
    let b = t.paint_brush.min(BRUSHES.len() - 1);
    PaintOpts { brush: b as u32, alpha: t.paint_alpha, glow: t.paint_glow, taper: t.paint_taper, density: t.paint_density, tilt: t.paint_tilt, tilt_dir: t.paint_tilt_dir, texture_strength: t.paint_texture_strength, mark: BRUSHES[b].3 }
}

/// The Painterly profile of a new stroke.
pub fn painterly_profile(t: &ToolOpts, seed: u32) -> WidthProfile {
    let b = BRUSHES[t.paint_brush.min(BRUSHES.len() - 1)];
    WidthProfile::Painterly { bristles: b.1, jitter: b.2, seed, opts: paint_opts(t) }
}

/// The selected strokes take the brush settings (with nothing selected they are the defaults).
pub(crate) fn paint_apply(app: &mut AnyVectorApp) {
    if !painterly_now(app) {
        return;
    }
    let t = app.settings.topts.clone();
    edit_sel(app, "Painterly Brush", |o| {
        if let Some(WidthProfile::Painterly { seed, .. }) = &o.style.outline.profile {
            o.style.outline.profile = Some(painterly_profile(&t, *seed));
        }
    });
}

/// Choose a brush style: it becomes the first of the recent ones and goes to the selection.
pub(crate) fn choose_brush(app: &mut AnyVectorApp, b: usize) {
    let t = &mut app.settings.topts;
    t.paint_brush = b;
    t.paint_recent.retain(|x| *x != b);
    t.paint_recent.insert(0, b);
    t.paint_recent.truncate(3);
    let t = app.settings.topts.clone();
    if app.has_selection() {
        let seed = (app.time() * 1000.0) as u32;
        edit_sel(app, "Painterly Brush", |o| {
            if o.is_container() {
                return;
            }
            let seed = match &o.style.outline.profile {
                Some(WidthProfile::Painterly { seed, .. }) => *seed,
                _ => seed,
            };
            o.style.outline.profile = Some(painterly_profile(&t, seed));
            if o.style.outline.color.is_none() {
                o.style.outline.color = Some(Color::BLACK);
            }
            o.style.outline.width = o.style.outline.width.max(7.2);
        });
    }
}

/// The variable outline of the first selected object: nodes and "scale with stroke".
fn vo_now(app: &AnyVectorApp) -> Option<(Vec<VarWidth>, bool)> {
    first(app, |o| if let Some(WidthProfile::Variable { nodes, scale }) = &o.style.outline.profile { Some((nodes.clone(), *scale)) } else { None }).flatten()
}

fn edit_vo(app: &mut AnyVectorApp, label: &str, mut f: impl FnMut(&mut Vec<VarWidth>, &mut bool)) {
    let dflt = app.ts.default_outline_width.max(0.5);
    edit_sel(app, label, |o| {
        if o.is_container() {
            return;
        }
        let w = o.style.outline.width.max(dflt) / 2.0;
        let (mut nodes, mut scale) = match &o.style.outline.profile {
            Some(WidthProfile::Variable { nodes, scale }) => (nodes.clone(), *scale),
            _ => (vec![VarWidth { t: 0.0, side1: w, side2: w }, VarWidth { t: 1.0, side1: w, side2: w }], false),
        };
        f(&mut nodes, &mut scale);
        nodes.sort_by(|a, b| a.t.total_cmp(&b.t));
        o.style.outline.profile = Some(WidthProfile::Variable { nodes, scale });
        if o.style.outline.color.is_none() {
            o.style.outline.color = Some(Color::BLACK);
        }
    });
}

/// Rows of check boxes in a drop-down panel.
fn checks(ui: &mut Ui, names: &[&str], vals: &mut [bool]) -> bool {
    let mut ch = false;
    for (n, v) in names.iter().zip(vals.iter_mut()) {
        ch |= ui.checkbox(v, *n).changed();
    }
    ch
}

/// The name of a selected list entry.
fn preset_names(app: &AnyVectorApp, effect: &str) -> Vec<String> {
    to(app).presets.get(effect).map(|v| v.iter().map(|p| p.0.clone()).collect()).unwrap_or_default()
}

/// The effect of the selection as JSON, for a preset.
fn effect_json(app: &AnyVectorApp, effect: &str) -> Option<String> {
    let v = match effect {
        "Blend" => {
            let b = blend_now(app)?;
            serde_json::json!({ "steps": b.steps, "rotation": b.rotation, "acceleration": b.acceleration, "color_dir": b.color_dir, "looped": b.looped, "color_accel": b.color_accel, "link_accel": b.link_accel, "size_accel": b.size_accel })
        }
        "Contour" => serde_json::to_value(first(app, |o| o.effects.contour.clone()).flatten()?).ok()?,
        "Distortion" => serde_json::to_value(first(app, |o| o.effects.distort.clone()).flatten()?).ok()?,
        "Drop Shadow" => serde_json::to_value(first(app, |o| o.effects.drop_shadow.clone()).flatten()?).ok()?,
        "Envelope" => serde_json::to_value(first(app, |o| o.effects.envelope.clone()).flatten()?).ok()?,
        "Extrude" => serde_json::to_value(first(app, |o| o.effects.extrude.clone()).flatten()?).ok()?,
        _ => return None,
    };
    Some(v.to_string())
}

/// Apply a preset to the selection.
fn apply_preset(app: &mut AnyVectorApp, effect: &str, json: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return };
    match effect {
        "Blend" => {
            let g = |k: &str| v.get(k).and_then(|x| x.as_f64());
            let gb = |k: &str| v.get(k).and_then(|x| x.as_bool());
            edit_blend(app, |b| {
                if let Some(s) = g("steps") {
                    b.steps = s as u32;
                }
                b.rotation = g("rotation").unwrap_or(b.rotation);
                b.acceleration = g("acceleration").unwrap_or(b.acceleration);
                b.color_dir = g("color_dir").map(|x| x as u8).unwrap_or(b.color_dir);
                b.looped = gb("looped").unwrap_or(b.looped);
                b.color_accel = g("color_accel").unwrap_or(b.color_accel);
                b.link_accel = gb("link_accel").unwrap_or(b.link_accel);
                b.size_accel = gb("size_accel").unwrap_or(b.size_accel);
            });
        }
        "Contour" => {
            if let Ok(e) = serde_json::from_value::<vector_core::object::ContourEffect>(v) {
                edit_sel(app, "Contour", |o| o.effects.contour = Some(e.clone()));
            }
        }
        "Distortion" => {
            if let Ok(e) = serde_json::from_value::<vector_core::object::DistortEffect>(v) {
                edit_sel(app, "Distortion", |o| o.effects.distort = Some(e.clone()));
            }
        }
        "Drop Shadow" => {
            if let Ok(e) = serde_json::from_value::<vector_core::object::DropShadow>(v) {
                edit_sel(app, "Drop Shadow", |o| o.effects.drop_shadow = Some(e.clone()));
            }
        }
        "Envelope" => {
            if let Ok(e) = serde_json::from_value::<vector_core::object::EnvelopeEffect>(v) {
                edit_sel(app, "Envelope", |o| o.effects.envelope = Some(e.clone()));
            }
        }
        "Extrude" => {
            if let Ok(e) = serde_json::from_value::<vector_core::object::ExtrudeEffect>(v) {
                edit_sel(app, "Extrude", |o| o.effects.extrude = Some(e.clone()));
            }
        }
        _ => {}
    }
}

/// Bake the envelope and distortion of the selection into its curves (Add New).
fn bake_warps(app: &mut AnyVectorApp, label: &str, fresh_envelope: bool) {
    let t = to(app);
    let fresh = fresh_envelope.then(|| vector_core::object::EnvelopeEffect { mode: t.env_mode, mapping: t.env_mapping, keep_lines: t.env_keep_lines, ..Default::default() });
    bake_warps_with(app, label, fresh);
}

/// Bake the envelopes and distortions of the selection into its curves, then give them `fresh`
/// (Add New: a new envelope over the warped shapes).
pub(crate) fn bake_warps_with(app: &mut AnyVectorApp, label: &str, fresh: Option<vector_core::object::EnvelopeEffect>) {
    let Some(d) = app.doc_mut() else { return };
    let sel = d.selection.clone();
    d.modify(label, |doc, s| {
        let others: Vec<_> = sel.iter().copied().filter(|id| doc.find(*id).is_some_and(|o| !matches!(o.kind, ObjKind::Path(_)) && !o.is_container())).collect();
        let mut ids: Vec<_> = sel.iter().copied().filter(|id| !others.contains(id)).collect();
        // text and shapes become curves, with their warps
        ids.extend(vector_core::ops::convert_to_curves(doc, &others));
        for id in &ids {
            let Some(o) = doc.find_mut(*id) else { continue };
            if !matches!(o.kind, ObjKind::Path(_)) {
                continue;
            }
            let wp = o.world_path();
            o.kind = ObjKind::Path(vector_core::path::NodePath::from_bez(&wp));
            o.transform = Affine::IDENTITY;
            o.effects.distort = None;
            o.effects.envelope = fresh.clone();
        }
        *s = ids;
    });
}

// ----------------------------------------------------------------------------- buttons

/// (enabled, checked) of a button of [`OPTIONS`].
pub fn option_state(app: &AnyVectorApp, key: &str) -> Option<(bool, bool)> {
    let doc = app.has_doc();
    let sel = app.has_selection();
    let t = to(app);
    let ts = &app.ts;
    Some(match key {
        "2a8b936e" | "47c44ef5" | "1a21e45c" | "3d9ac113" => (sel, false),
        "a540f78f" => (sel && mesh_now(app).is_some(), false),
        "692bc9d1" => (blend_now(app).is_some(), false),
        "9a9dead0" | "c3f54953" | "2c99de2c" | "1f8f2496" | "1d271da3" | "70e110e8" => (sel, false),
        "5e1fc86c" => match anchor_now(app) {
            Some((_, _, a)) => (true, a.dir.is_some()),
            None => (false, false),
        },
        "5024706a" => (doc, t.anchor_relative),
        "8a929185" => match anchor_now(app) {
            Some((_, _, a)) => (true, a.auto),
            None => (false, false),
        },
        "f6d38abb" => (anchor_now(app).is_some(), false),
        "fdaf006a" => (doc, dim_flag(app, |d| d.dynamic, |t| t.dim_dynamic)),
        "0cf00463" => (doc, dim_flag(app, |d| d.leading_zero, |t| t.dim_leading_zero)),
        "91a0ffc5" => (doc, dim_flag(app, |d| d.show_units, |t| t.dim_units)),
        "d3154914" => (doc, t.dim_successive),
        "cf3843d7" | "5e89d7ab" | "7ec5ed5a" => (doc, t.line2_mode == line2_of(key)),
        "3b408305" => (doc, !app.settings.hide_bbox_curve_tools),
        "a1df5fa2" => (doc, ts.parallel_drawing),
        "53ad9d10" | "95fe5c94" | "eac788fd" => {
            let i = ["53ad9d10", "95fe5c94", "eac788fd"].iter().position(|k| *k == key).unwrap_or(0);
            match t.paint_recent.get(i) {
                Some(b) => (doc, *b == t.paint_brush),
                None => (false, false),
            }
        }
        "19fd40bd" => (painterly_now(app), false),
        "7d4894f7" => (doc, t.paint_glow),
        "3860fe09" => (doc, false),
        "c15728bb" => (doc, t.paint_taper),
        "227bb320" => (doc, t.media_scale),
        "fb587552" => (doc, !t.repel),
        "4e560b2f" => (doc, t.repel),
        "a45bce92" => (doc, t.pen_pressure),
        "d5b480c4" => (doc, t.smear_smooth),
        "a937dead" => (doc, !t.smear_smooth),
        "90069312" => (doc, t.twirl_ccw),
        "80bbe208" => (doc, !t.twirl_ccw),
        "64cc85b2" | "100b6bed" | "e6025a48" | "e8a6d2d0" => match first(app, |o| o.effects.block_shadow.clone()).flatten() {
            Some(b) => (true, match key {
                "64cc85b2" => b.from_outline,
                "100b6bed" => b.overprint,
                "e6025a48" => b.remove_holes,
                _ => b.simplify,
            }),
            None => (false, false),
        },
        "04cdfb63" => (app.doc().is_some_and(|d| d.node_sel.len() >= 2), false),
        "b5180999" => (doc, t.reflect_h),
        "952282d5" => (doc, t.reflect_v),
        "94e2c33b" => (doc, ts.node_mode == 1),
        "8f8f5213" => (doc, ts.node_mode == 2),
        "75bcd7d6" => (doc, t.eraser_use_bearing),
        "756973e2" => (doc, t.eraser_pressure),
        "4e713b0f" => (doc, t.eraser_use_tilt),
        "a13af0fa" | "7772fd02" | "fddc92f7" => (doc, t.eye_area == eye_area_of(key)),
        "8b50d9ca" => (doc, !ts.eye_apply && !ts.desktop_pick),
        "abf0e5e9" => (ts.eyedrop_color.is_some(), ts.eye_apply),
        "e09ae203" => (doc, ts.desktop_pick),
        "636e2e05" => (doc, !ts.attr_apply),
        "b8d60a54" => (ts.eyedrop_attrs.is_some(), ts.attr_apply),
        "462130ab" | "d703bd6d" | "3f78578f" | "abb745ee" => (doc, t.ft_mode == ft_of(key)),
        "e7cec14f" => (doc, t.ft_duplicate),
        "249bae0e" => (doc, t.ft_relative),
        "7e180fc2" | "c6994e2e" | "7531fe58" => match blend_now(app) {
            Some(b) => (true, b.color_dir == blend_dir_of(key)),
            None => (false, false),
        },
        "add7ce56" => match blend_now(app) {
            Some(b) => (b.rotation.abs() > 1e-9, b.looped),
            None => (false, false),
        },
        "2c88e5ab" => match blend_now(app) {
            Some(b) => (true, b.size_accel),
            None => (false, false),
        },
        "2742f968" | "65a24647" | "e7e3e939" | "cb92c441" | "f08777f1" | "6238a70e" => {
            let e = PRESET_LISTS.iter().find(|p| p.1 == key).map(|p| p.3).unwrap_or("");
            (effect_json(app, e).is_some(), false)
        }
        "4933ce7e" | "3b31083e" | "bb990f92" | "b3e06fa1" | "7a004a12" | "3d684904" => {
            let e = PRESET_LISTS.iter().find(|p| p.2 == key).map(|p| p.3).unwrap_or("");
            (t.preset_sel.get(e).is_some_and(|n| preset_names(app, e).contains(n)), false)
        }
        "88dd3b61" | "0381788e" => (has(app, |o| o.effects.distort.as_ref().map(|_| ())), false),
        "a14006c6" | "a63b08d2" | "0b55facb" | "180a4571" => (doc, env_mode(app) == env_mode_of(key)),
        "a428b4e7" => (doc, first(app, |o| o.effects.envelope.as_ref().map(|e| e.keep_lines)).flatten().unwrap_or(t.env_keep_lines)),
        "68a0b923" => (has(app, |o| o.effects.envelope.as_ref().map(|_| ())), false),
        "45ed8127" => match first(app, |o| o.effects.extrude.as_ref().map(|e| e.vp_page)).flatten() {
            Some(p) => (true, p),
            None => (false, false),
        },
        "89b0fceb" | "0d1dd68e" => (mesh_now(app).is_some(), false),
        "180f15f7" => match mesh_now(app) {
            Some(m) => (true, m.smooth),
            None => (false, false),
        },
        "39d9acd4" | "68943942" | "7b87d66c" => (doc, t.knife_mode == knife_of(key)),
        "a9a56d5c" => (doc, t.knife_auto_close),
        "0230228e" => (doc, t.ls_single),
        "b3914d92" => (doc, t.ls_include),
        "61575354" => (doc, t.ls_preview),
        "4b0a1855" => (doc, t.pen_auto_add),
        "921e4f4e" => (doc, t.pen_preview),
        "4d4cf95b" => (doc, t.polyline_auto_close),
        "8534142a" => (doc, t.rough_pressure),
        "16e06dc5" => (doc, t.rough_use_tilt),
        "01c8c86b" => (doc, t.smudge_pressure),
        "034d74d6" => (doc, t.smudge_use_bearing),
        "726cb4c9" => (doc, t.smudge_use_tilt),
        "fa26e7e3" | "e624647c" => {
            let tx = crate::dialogs::text::first_text(app);
            (doc, tx.list.as_ref().is_some_and(|l| l.numbers == (key == "e624647c")))
        }
        "6123599f" => (doc, crate::dialogs::text::first_text(app).drop_cap.is_some()),
        "a7d5c6be" => (doc, false),
        "3720fa77" => (doc && crate::dialogs::text::first_text(app).left_indent > 0.0, false),
        "5e8bb83c" => (doc, t.text_opentype),
        "019453ed" => (doc, false),
        "d561296d" => (sel, false),
        "1e55a1b0" => (vo_now(app).is_some_and(|(n, _)| app.ts.vo_node.is_some_and(|i| i < n.len()) && n.len() > 1), false),
        "16bb3f06" => (doc, t.vo_lock),
        "8edb8dc0" => (doc, vo_now(app).map(|v| v.1).unwrap_or(t.vo_scale)),
        "da692f6b" => (vo_now(app).is_some(), false),
        "8bbef079" => (doc, ts.impact_inner.is_some()),
        "572c7ba4" => (doc, ts.impact_outer.is_some()),
        "d7a05774" => (doc, t.impact_width_random),
        "63df27f5" => (doc, t.impact_spacing_random),
        _ => return None,
    })
}

fn line2_of(key: &str) -> u8 {
    match key {
        "5e89d7ab" => 1,
        "7ec5ed5a" => 2,
        _ => 0,
    }
}

fn eye_area_of(key: &str) -> u8 {
    match key {
        "7772fd02" => 1,
        "fddc92f7" => 2,
        _ => 0,
    }
}

fn ft_of(key: &str) -> u8 {
    match key {
        "462130ab" => 0,
        "d703bd6d" => 1,
        "abb745ee" => 3,
        _ => 2,
    }
}

fn blend_dir_of(key: &str) -> u8 {
    match key {
        "c6994e2e" => 1,
        "7531fe58" => 2,
        _ => 0,
    }
}

fn env_mode_of(key: &str) -> u8 {
    match key {
        "a14006c6" => 0,
        "a63b08d2" => 1,
        "0b55facb" => 2,
        _ => 3,
    }
}

fn env_mode(app: &AnyVectorApp) -> u8 {
    first(app, |o| o.effects.envelope.as_ref().map(|e| e.mode)).flatten().unwrap_or(to(app).env_mode)
}

fn knife_of(key: &str) -> u8 {
    match key {
        "68943942" => 1,
        "7b87d66c" => 2,
        _ => 0,
    }
}

/// Run a button of [`OPTIONS`].
pub fn press(app: &mut AnyVectorApp, key: &str) {
    if option_state(app, key).is_some_and(|(on, _)| !on) {
        return;
    }
    let t = &mut app.settings.topts;
    match key {
        "2a8b936e" => copy_from(app, 8),
        "47c44ef5" => copy_from(app, 9),
        "a540f78f" => copy_from(app, 11),
        "692bc9d1" => copy_from(app, 2),
        "9a9dead0" => copy_from(app, 4),
        "c3f54953" => copy_from(app, 7),
        "2c99de2c" => copy_from(app, 6),
        "1f8f2496" => copy_from(app, 0),
        "1d271da3" => copy_from(app, 3),
        "70e110e8" => copy_from(app, 10),
        "1a21e45c" => app.ts.pick_target = Some(PickTarget::EnvelopeFrom),
        "3d9ac113" => app.run(Cmd::ObjectProperties),
        "5e1fc86c" => {
            if let Some((id, i, a)) = anchor_now(app) {
                let dir = if a.dir.is_some() { None } else { Some(0.0) };
                edit_anchors(app, "Anchor Direction", id, |v| v[i].dir = dir);
            }
        }
        "5024706a" => t.anchor_relative = !t.anchor_relative,
        "8a929185" => {
            if let Some((id, i, a)) = anchor_now(app) {
                edit_anchors(app, "Auto Anchor", id, |v| v[i].auto = !a.auto);
            }
        }
        "f6d38abb" => {
            if let Some((id, i, _)) = anchor_now(app) {
                edit_anchors(app, "Delete Anchor", id, |v| {
                    v.remove(i);
                });
                app.ts.anchor_sel = None;
            }
        }
        "fdaf006a" => {
            let v = !dim_flag(app, |d| d.dynamic, |t| t.dim_dynamic);
            set_dim(
                app,
                |d| {
                    d.dynamic = v;
                    // a static dimension keeps the value it shows now
                    d.text = if v { String::new() } else { vector_core::object::dimension_label(d).0 };
                },
                |t| t.dim_dynamic = v,
            );
        }
        "0cf00463" => {
            let v = !dim_flag(app, |d| d.leading_zero, |t| t.dim_leading_zero);
            set_dim(app, |d| d.leading_zero = v, |t| t.dim_leading_zero = v);
        }
        "91a0ffc5" => {
            let v = !dim_flag(app, |d| d.show_units, |t| t.dim_units);
            set_dim(app, |d| d.show_units = v, |t| t.dim_units = v);
        }
        "d3154914" => t.dim_successive = !t.dim_successive,
        "cf3843d7" | "5e89d7ab" | "7ec5ed5a" => t.line2_mode = line2_of(key),
        "3b408305" => app.settings.hide_bbox_curve_tools = !app.settings.hide_bbox_curve_tools,
        "a1df5fa2" => app.ts.parallel_drawing = !app.ts.parallel_drawing,
        "53ad9d10" | "95fe5c94" | "eac788fd" => {
            let i = ["53ad9d10", "95fe5c94", "eac788fd"].iter().position(|k| *k == key).unwrap_or(0);
            if let Some(b) = t.paint_recent.get(i).copied() {
                choose_brush(app, b);
            }
        }
        "19fd40bd" => edit_sel(app, "Clear Brushstroke", |o| {
            if matches!(o.style.outline.profile, Some(WidthProfile::Painterly { .. })) {
                let c = o.style.outline.color;
                o.style.outline = vector_core::Outline::default();
                o.style.outline.color = c;
            }
        }),
        "7d4894f7" => {
            t.paint_glow = !t.paint_glow;
            paint_apply(app);
        }
        "3860fe09" => {
            let d = ToolOpts::default();
            t.paint_alpha = d.paint_alpha;
            t.paint_smoothing = d.paint_smoothing;
            t.paint_texture = d.paint_texture;
            t.paint_texture_strength = d.paint_texture_strength;
            t.paint_density = d.paint_density;
            t.paint_tilt = d.paint_tilt;
            t.paint_tilt_dir = d.paint_tilt_dir;
            t.paint_glow = d.paint_glow;
            t.paint_taper = d.paint_taper;
            paint_apply(app);
        }
        "c15728bb" => {
            t.paint_taper = !t.paint_taper;
            paint_apply(app);
        }
        "227bb320" => {
            t.media_scale = !t.media_scale;
            let v = t.media_scale;
            if has(app, |o| o.style.outline.profile.as_ref().map(|_| ())) {
                edit_sel(app, "Scale Stroke With Object", |o| o.style.outline.scale_with_object = v);
            }
        }
        "fb587552" => t.repel = false,
        "4e560b2f" => t.repel = true,
        "a45bce92" => t.pen_pressure = !t.pen_pressure,
        "d5b480c4" => t.smear_smooth = true,
        "a937dead" => t.smear_smooth = false,
        "90069312" => t.twirl_ccw = true,
        "80bbe208" => t.twirl_ccw = false,
        "64cc85b2" | "100b6bed" | "e6025a48" | "e8a6d2d0" => {
            let k = key.to_string();
            edit_sel(app, "Block Shadow", |o| {
                if let Some(b) = &mut o.effects.block_shadow {
                    match k.as_str() {
                        "64cc85b2" => b.from_outline = !b.from_outline,
                        "100b6bed" => b.overprint = !b.overprint,
                        "e6025a48" => b.remove_holes = !b.remove_holes,
                        _ => b.simplify = !b.simplify,
                    }
                }
            });
        }
        "04cdfb63" => app.dialogs.node_align = Some([true, true, false]),
        "b5180999" => t.reflect_h = !t.reflect_h,
        "952282d5" => t.reflect_v = !t.reflect_v,
        "94e2c33b" => app.ts.node_mode = if app.ts.node_mode == 1 { 0 } else { 1 },
        "8f8f5213" => app.ts.node_mode = if app.ts.node_mode == 2 { 0 } else { 2 },
        "75bcd7d6" => t.eraser_use_bearing = !t.eraser_use_bearing,
        "756973e2" => t.eraser_pressure = !t.eraser_pressure,
        "4e713b0f" => t.eraser_use_tilt = !t.eraser_use_tilt,
        "a13af0fa" | "7772fd02" | "fddc92f7" => t.eye_area = eye_area_of(key),
        "8b50d9ca" => {
            app.ts.eye_apply = false;
            app.ts.desktop_pick = false;
        }
        "abf0e5e9" => {
            app.ts.eye_apply = true;
            app.ts.desktop_pick = false;
        }
        "e09ae203" => {
            app.ts.desktop_pick = !app.ts.desktop_pick;
            app.ts.eye_apply = false;
        }
        "636e2e05" => app.ts.attr_apply = false,
        "b8d60a54" => app.ts.attr_apply = true,
        "462130ab" | "d703bd6d" | "3f78578f" | "abb745ee" => t.ft_mode = ft_of(key),
        "e7cec14f" => t.ft_duplicate = !t.ft_duplicate,
        "249bae0e" => t.ft_relative = !t.ft_relative,
        "7e180fc2" | "c6994e2e" | "7531fe58" => {
            let d = blend_dir_of(key);
            edit_blend(app, |b| b.color_dir = d);
        }
        "add7ce56" => edit_blend(app, |b| b.looped = !b.looped),
        "2c88e5ab" => edit_blend(app, |b| b.size_accel = !b.size_accel),
        "2742f968" | "65a24647" | "e7e3e939" | "cb92c441" | "f08777f1" | "6238a70e" => {
            let Some(&(_, _, _, e)) = PRESET_LISTS.iter().find(|p| p.1 == key) else { return };
            let Some(json) = effect_json(app, e) else { return };
            let list = app.settings.topts.presets.entry(e.to_string()).or_default();
            let mut n = list.len() + 1;
            while list.iter().any(|p| p.0 == format!("Preset {n}")) {
                n += 1;
            }
            let name = format!("Preset {n}");
            list.push((name.clone(), json));
            app.settings.topts.preset_sel.insert(e.to_string(), name);
        }
        "4933ce7e" | "3b31083e" | "bb990f92" | "b3e06fa1" | "7a004a12" | "3d684904" => {
            let Some(&(_, _, _, e)) = PRESET_LISTS.iter().find(|p| p.2 == key) else { return };
            if let Some(name) = t.preset_sel.remove(e) {
                if let Some(list) = t.presets.get_mut(e) {
                    list.retain(|p| p.0 != name);
                }
            }
        }
        "88dd3b61" => edit_sel(app, "Center Distortion", |o| {
            if let Some(d) = &mut o.effects.distort {
                d.center = Point::new(0.5, 0.5);
            }
        }),
        "0381788e" => bake_warps(app, "Add New Distortion", false),
        "a14006c6" | "a63b08d2" | "0b55facb" | "180a4571" => {
            let m = env_mode_of(key);
            t.env_mode = m;
            edit_sel(app, "Envelope", |o| {
                if let Some(e) = &mut o.effects.envelope {
                    e.mode = m;
                }
            });
        }
        "a428b4e7" => {
            let v = !option_state(app, key).map(|s| s.1).unwrap_or(false);
            app.settings.topts.env_keep_lines = v;
            edit_sel(app, "Envelope", |o| {
                if let Some(e) = &mut o.effects.envelope {
                    e.keep_lines = v;
                }
            });
        }
        "68a0b923" => bake_warps(app, "Add New Envelope", true),
        "45ed8127" => edit_sel(app, "Extrude", |o| {
            if let Some(e) = &mut o.effects.extrude {
                e.vp_page = !e.vp_page;
            }
        }),
        "89b0fceb" => {
            let Some(m) = mesh_now(app) else { return };
            let (u, v) = match app.ts.table_cell {
                Some((r, c)) => ((c as f64 + 0.5) / m.cols.max(1) as f64, (r as f64 + 0.5) / m.rows.max(1) as f64),
                None => (0.5, 0.5),
            };
            edit_mesh(app, "Add Intersection", |m, _| m.add_intersection(u, v));
            app.ts.table_cell = None;
        }
        "0d1dd68e" => {
            // the next click samples the color for the selected nodes
            app.ts.eye_apply = false;
            app.ts.desktop_pick = false;
            app.ts.mesh_sample = true;
        }
        "180f15f7" => edit_mesh(app, "Smooth Mesh Color", |m, _| m.smooth = !m.smooth),
        "39d9acd4" | "68943942" | "7b87d66c" => {
            t.knife_mode = knife_of(key);
            app.ts.stage.clear();
        }
        "a9a56d5c" => t.knife_auto_close = !t.knife_auto_close,
        "0230228e" => t.ls_single = !t.ls_single,
        "b3914d92" => t.ls_include = !t.ls_include,
        "61575354" => t.ls_preview = !t.ls_preview,
        "4b0a1855" => t.pen_auto_add = !t.pen_auto_add,
        "921e4f4e" => t.pen_preview = !t.pen_preview,
        "4d4cf95b" => t.polyline_auto_close = !t.polyline_auto_close,
        "8534142a" => t.rough_pressure = !t.rough_pressure,
        "16e06dc5" => t.rough_use_tilt = !t.rough_use_tilt,
        "01c8c86b" => t.smudge_pressure = !t.smudge_pressure,
        "034d74d6" => t.smudge_use_bearing = !t.smudge_use_bearing,
        "726cb4c9" => t.smudge_use_tilt = !t.smudge_use_tilt,
        "fa26e7e3" | "e624647c" => {
            let numbers = key == "e624647c";
            let on = option_state(app, key).map(|s| s.1).unwrap_or(false);
            crate::dialogs::text::modify_texts(app, if numbers { "Numbers" } else { "Bullets" }, true, move |tx| {
                tx.list = if on {
                    None
                } else {
                    Some(vector_core::text::ListFormat { numbers, ..tx.list.clone().unwrap_or_default() })
                };
            });
        }
        "6123599f" => crate::dialogs::text::toggle_para(app, "Drop Cap", |t| t.drop_cap.is_some(), |t, v| t.drop_cap = v.then(Default::default)),
        "a7d5c6be" | "3720fa77" => {
            // one tab stop (half an inch) per step
            let step = if key == "a7d5c6be" { 36.0 } else { -36.0 };
            crate::dialogs::text::modify_texts(app, if step > 0.0 { "Increase Indent" } else { "Decrease Indent" }, true, move |tx| {
                tx.clear_paragraph_styles();
                let first_rel = tx.first_indent - tx.left_indent;
                tx.left_indent = (tx.left_indent + step).max(0.0);
                tx.first_indent = (tx.left_indent + first_rel).max(0.0);
            });
        }
        "5e8bb83c" => t.text_opentype = !t.text_opentype,
        "019453ed" => {
            if let Some(d) = app.doc_mut() {
                let r = if d.doc.settings.multipage_view && d.doc.pages.len() > 1 {
                    crate::canvas::page_offsets(&d.doc, d.page).into_iter().map(|(i, dv)| d.doc.pages[i].rect() + dv).reduce(|a, b| a.union(b))
                } else {
                    Some(d.doc.pages[d.page].rect())
                };
                if let Some(r) = r {
                    d.zoom_to_rect(r, 24.0);
                }
            }
        }
        "d561296d" => {
            let sel = app.ts.vo_node;
            let mut added = None;
            let now = vo_now(app);
            edit_vo(app, "Add Variable Node", |nodes, _| {
                let at = match sel.filter(|i| *i + 1 < nodes.len()) {
                    Some(i) => (nodes[i].t + nodes[i + 1].t) / 2.0,
                    None => 0.5,
                };
                let (a, b) = vector_core::style::var_width_at(nodes, at);
                nodes.push(VarWidth { t: at, side1: a, side2: b });
                added = Some(at);
            });
            let _ = now;
            if let Some(at) = added {
                app.ts.vo_node = vo_now(app).and_then(|(n, _)| n.iter().position(|w| (w.t - at).abs() < 1e-9));
            }
        }
        "1e55a1b0" => {
            if let Some(i) = app.ts.vo_node {
                edit_vo(app, "Delete Variable Node", |nodes, _| {
                    if i < nodes.len() && nodes.len() > 1 {
                        nodes.remove(i);
                    }
                });
                app.ts.vo_node = None;
            }
        }
        "16bb3f06" => t.vo_lock = !t.vo_lock,
        "8edb8dc0" => {
            let v = !option_state(app, key).map(|s| s.1).unwrap_or(false);
            app.settings.topts.vo_scale = v;
            if vo_now(app).is_some() {
                edit_vo(app, "Scale Variable Outline", |_, s| *s = v);
            }
        }
        "da692f6b" => {
            edit_sel(app, "Clear Nodes", |o| {
                if matches!(o.style.outline.profile, Some(WidthProfile::Variable { .. })) {
                    o.style.outline.profile = None;
                }
            });
            app.ts.vo_node = None;
        }
        "8bbef079" | "572c7ba4" => {
            // Select inner / outer boundary: click a reference object; pressed again, it is removed
            let outer = key == "572c7ba4";
            let set = if outer { app.ts.impact_outer.is_some() } else { app.ts.impact_inner.is_some() };
            if set {
                if outer {
                    app.ts.impact_outer = None;
                } else {
                    app.ts.impact_inner = None;
                }
                crate::tools::regen_impacts(app);
            } else {
                app.ts.pick_target = Some(PickTarget::ImpactBoundary { outer });
            }
        }
        "d7a05774" => {
            t.impact_width_random = !t.impact_width_random;
            crate::tools::regen_impacts(app);
        }
        "63df27f5" => {
            t.impact_spacing_random = !t.impact_spacing_random;
            crate::tools::regen_impacts(app);
        }
        _ => {}
    }
}

// ----------------------------------------------------------------------------- fields

/// Draw the field `it` if it is one of the tools' options handled here.
/// The Impact settings that shape an impact effect.
fn impact_key(t: &ToolOpts) -> String {
    format!(
        "{:?}",
        (t.impact_parallel, t.impact_angle, t.impact_ends, t.impact_width, t.impact_width_steps, t.impact_spacing, t.impact_spacing_steps, t.impact_style, t.impact_widest)
    )
}

pub fn control(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item, k: &str) -> bool {
    // changing an Impact setting changes the selected impact effects
    let impact = matches!(k, "99a3bfe4" | "352a8cf2" | "bddb7369" | "aa85aae1" | "ad61ddb5" | "df313a7e" | "4bb88774" | "c393853c" | "a6027411");
    let before = impact.then(|| impact_key(to(app)));
    let r = control_inner(app, ui, it, k);
    if let Some(b) = before {
        if impact_key(to(app)) != b {
            crate::tools::regen_impacts(app);
        }
    }
    r
}

fn control_inner(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item, k: &str) -> bool {
    let doc = app.has_doc();
    let id = pid(ui, k);
    match k {
        // ---- anchors
        "a2555029" => {
            let cur = anchor_now(app).and_then(|(oid, _, a)| {
                let o = app.doc()?.doc.find(oid)?;
                let b = app.doc()?.doc.bounds_of(o);
                let abs = Point::new(b.x0 + a.pos.x * b.width(), b.y0 + a.pos.y * b.height());
                Some((oid, b, abs))
            });
            let rel = to(app).anchor_relative;
            let shown = cur.map(|(_, b, p)| if rel { (p.x - b.x0, p.y - b.y0) } else { (p.x, p.y) });
            let f = len(app);
            let r = fields::stacked(app, ui, id, [Lead::Text("X:"), Lead::Text("Y:")], [shown.map(|s| s.0), shown.map(|s| s.1)], f, 84.0, "", true, true, cur.is_some(), &tip(it));
            if let (Some((oid, b, p)), Some(i)) = (cur, app.ts.anchor_sel.map(|s| s.1)) {
                if r[0].is_some() || r[1].is_some() {
                    let off = if rel { Vec2::new(b.x0, b.y0) } else { Vec2::ZERO };
                    let x = r[0].map(|x| x + off.x).unwrap_or(p.x);
                    let y = r[1].map(|y| y + off.y).unwrap_or(p.y);
                    let n = Point::new(((x - b.x0) / b.width().max(1e-9)).clamp(0.0, 1.0), ((y - b.y0) / b.height().max(1e-9)).clamp(0.0, 1.0));
                    edit_anchors(app, "Anchor Position", oid, |v| v[i].pos = n);
                }
            }
        }
        "024be2af" => {
            let cur = anchor_now(app);
            let dir = cur.and_then(|(_, _, a)| a.dir);
            if let Some(v) = fields::spin(app, ui, id, lead(it), dir, Fmt::Num(1, ""), 52.0, "°", dir.is_some(), &tip(it)) {
                if let Some((oid, i, _)) = cur {
                    edit_anchors(app, "Anchor Direction", oid, |a| a[i].dir = Some(v.rem_euclid(360.0)));
                }
            }
        }
        // ---- tables
        "f62e2a7e" | "420cf24d" => table_color(app, ui, it, k),
        "a31335f5" => table_options(app, ui, it),
        "1b2339bc" => table_margins(app, ui, it),
        "82f9896c" => table_cell_size(app, ui, it),
        // ---- wrap paragraph text
        "574feecc" => wrap_text(app, ui, it),
        // ---- dimensions and callouts
        "34c73207" => dim_arrow(app, ui, it),
        "81eeaff4" => dim_ext(app, ui, it),
        "7b7a86f3" => {
            let cur = dimension_now(app).map(|d| d.callout_gap).unwrap_or(to(app).callout_gap);
            let f = len(app);
            if let Some(v) = fields::spin(app, ui, id, lead(it), Some(cur), f, 64.0, "", doc, &tip(it)) {
                let v = v.max(0.0);
                set_dim(app, |d| d.callout_gap = v, |t| t.callout_gap = v);
            }
        }
        "aedec877" => {
            let names = ["None", "Rectangle", "Rounded Rectangle", "Ellipse"];
            let cur = dimension_now(app).map(|d| d.callout_shape).unwrap_or(to(app).callout_shape).min(3) as usize;
            let mut pick = None;
            fields::combo(app, ui, Lead::None, names[cur], 110.0, doc, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i as u8);
                    }
                }
            });
            if let Some(s) = pick {
                set_dim(app, |d| d.callout_shape = s, |t| t.callout_shape = s);
            }
        }
        // ---- Painterly Brush
        "b9d3739a" => brush_picker(app, ui, it),
        "4f957d7b" => {
            let cur = to(app).paint_texture.min(TEXTURES.len() - 1);
            let mut pick = None;
            fields::combo(app, ui, lead(it), TEXTURES[cur], 96.0, doc, &tip(it), |_, ui| {
                for (i, n) in TEXTURES.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i);
                    }
                }
            });
            if let Some(i) = pick {
                app.settings.topts.paint_texture = i;
                paint_apply(app);
            }
        }
        "5ba8fb1c" | "8e6898f8" | "9920f482" | "2b5a49e1" => {
            let t = to(app);
            let v = match k {
                "5ba8fb1c" => t.paint_alpha,
                "8e6898f8" => t.paint_smoothing,
                "9920f482" => t.paint_density,
                _ => t.paint_texture_strength,
            };
            if let Some(n) = fields::slider(app, ui, id, lead(it), Some(v), 0.0..=100.0, Fmt::Num(0, ""), 36.0, doc, &tip(it)) {
                let n = n.clamp(0.0, 100.0);
                let t = &mut app.settings.topts;
                match k {
                    "5ba8fb1c" => t.paint_alpha = n,
                    "8e6898f8" => t.paint_smoothing = n,
                    "9920f482" => t.paint_density = n,
                    _ => t.paint_texture_strength = n,
                }
                paint_apply(app);
            }
        }
        "242398aa" | "46917a68" => {
            let tilt = k == "242398aa";
            let v = if tilt { to(app).paint_tilt } else { to(app).paint_tilt_dir };
            if let Some(n) = fields::spin(app, ui, id, lead(it), Some(v), Fmt::Num(0, ""), 44.0, "°", doc, &tip(it)) {
                if tilt {
                    app.settings.topts.paint_tilt = n.clamp(0.0, 90.0);
                } else {
                    app.settings.topts.paint_tilt_dir = n.rem_euclid(360.0);
                }
                paint_apply(app);
            }
        }
        // ---- block shadow
        "46e0780d" => {
            let cur = first(app, |o| o.effects.block_shadow.as_ref().map(|b| b.expand)).flatten();
            let f = len(app);
            if let Some(v) = fields::spin(app, ui, id, lead(it), cur, f, 64.0, "", cur.is_some(), &tip(it)) {
                edit_sel(app, "Block Shadow", |o| {
                    if let Some(b) = &mut o.effects.block_shadow {
                        b.expand = v;
                    }
                });
            }
        }
        // ---- crop
        "e0c8e0e5" => {
            let on = app.ts.crop_rect.is_some();
            if let Some(v) = fields::spin(app, ui, id, lead(it), on.then_some(app.ts.crop_angle), Fmt::Num(1, ""), 52.0, "°", on, &tip(it)) {
                app.ts.crop_angle = v.rem_euclid(360.0);
            }
        }
        // ---- curve smoothness: the Shape tool's nodes, the mesh's colors
        "15b32d9e" => {
            if app.tool == ToolId::MeshFill {
                let cur = mesh_now(app).map(|m| m.smoothness);
                if let Some(v) = fields::slider(app, ui, id, lead(it), cur, 0.0..=100.0, Fmt::Num(0, ""), 36.0, cur.is_some(), &tip(it)) {
                    let v = v.clamp(0.0, 100.0);
                    edit_mesh(app, "Curve Smoothness", |m, _| m.smoothness = v);
                }
            } else {
                let on = app.doc().is_some_and(|d| !d.node_sel.is_empty());
                if let Some(v) = fields::slider(app, ui, id, lead(it), Some(0.0), 0.0..=100.0, Fmt::Num(0, ""), 36.0, on, &tip(it)) {
                    smooth_nodes(app, v.clamp(0.0, 100.0));
                }
            }
        }
        // ---- eraser and virtual segment delete
        "13d2ba1f" => fields::caption(ui, "Shape:", doc),
        "a0eb3705" | "e83bd0a8" => {
            let tilt = k == "a0eb3705";
            let t = to(app);
            let (v, on) = if tilt { (t.eraser_tilt, t.eraser_use_tilt) } else { (t.eraser_bearing, t.eraser_use_bearing) };
            if let Some(n) = fields::spin(app, ui, id, lead(it), Some(v), Fmt::Num(0, ""), 44.0, "°", doc && on, &tip(it)) {
                if tilt {
                    app.settings.topts.eraser_tilt = n.clamp(0.0, 90.0);
                } else {
                    app.settings.topts.eraser_bearing = n.rem_euclid(360.0);
                }
            }
        }
        // ---- eyedroppers
        "be78ee55" | "4342a7e3" | "10d46324" => eye_lists(app, ui, it, k),
        "ed7a60e8" => {
            let c = app.ts.eyedrop_color;
            if let Some(n) = fields::color_combo(ui, id, c, 44.0, doc, &tip(it)) {
                app.ts.eyedrop_color = n;
                if n.is_some() {
                    app.ts.eye_apply = true;
                }
            }
        }
        // ---- free transform
        "067af1c1" => {
            let bb = app.doc().and_then(|d| d.selection_bounds());
            let c = app.ts.ft_center.or(bb.map(|b| b.center()));
            let f = len(app);
            let r = fields::stacked(app, ui, id, [Lead::Icon("rotate"), Lead::Icon("rotate")], [c.map(|c| c.x), c.map(|c| c.y)], f, 72.0, "", true, true, bb.is_some(), &tip(it));
            if let Some(c) = c {
                if r[0].is_some() || r[1].is_some() {
                    app.ts.ft_center = Some(Point::new(r[0].unwrap_or(c.x), r[1].unwrap_or(c.y)));
                }
            }
        }
        "6655110e" => {
            let on = app.has_selection();
            let r = fields::stacked(app, ui, id, [Lead::Icon("skew"), Lead::Icon("skew")], [on.then_some(0.0), on.then_some(0.0)], Fmt::Num(1, ""), 52.0, "°", false, true, on, &tip(it));
            let (h, v) = (r[0].unwrap_or(0.0), r[1].unwrap_or(0.0));
            if on && (h.abs() > 1e-9 || v.abs() > 1e-9) {
                if let Some(bb) = app.doc().and_then(|d| d.selection_bounds()) {
                    let c = app.ts.ft_center.unwrap_or(bb.center());
                    let m = Affine::translate(c.to_vec2()) * Affine::skew((-h.clamp(-89.0, 89.0)).to_radians().tan(), (-v.clamp(-89.0, 89.0)).to_radians().tan()) * Affine::translate(-c.to_vec2());
                    transform_sel(app, "Skew", m);
                }
            }
        }
        // ---- blend and contour accelerations
        "18e65c3f" => {
            let Some(b) = blend_now(app) else {
                fields::empty(app, ui, it);
                return true;
            };
            let mut out = None;
            fields::panel(app, ui, lead(it), "", 16.0, true, &tip(it), |_, ui| {
                let (mut a, mut c, mut link) = (b.acceleration, b.color_accel, b.link_accel);
                ui.label("Object acceleration");
                let ra = ui.add(egui::Slider::new(&mut a, -100.0..=100.0).show_value(true));
                ui.label("Color acceleration");
                let rc = ui.add_enabled(!link, egui::Slider::new(&mut c, -100.0..=100.0).show_value(true));
                let rl = ui.checkbox(&mut link, "Link accelerations");
                if ra.changed() || rc.changed() || rl.changed() {
                    out = Some((a, if link { a } else { c }, link));
                }
            });
            if let Some((a, c, l)) = out {
                edit_blend(app, |b| {
                    b.acceleration = a;
                    b.color_accel = c;
                    b.link_accel = l;
                });
            }
        }
        "eac9ad1a" => {
            let Some(cc) = first(app, |o| o.effects.contour.clone()).flatten() else {
                fields::empty(app, ui, it);
                return true;
            };
            let mut out = None;
            fields::panel(app, ui, lead(it), "", 16.0, true, &tip(it), |_, ui| {
                let (mut a, mut c, mut link) = (cc.accel, cc.color_accel, cc.link_accel);
                ui.label("Object acceleration");
                let ra = ui.add(egui::Slider::new(&mut a, -100.0..=100.0).show_value(true));
                ui.label("Color acceleration");
                let rc = ui.add_enabled(!link, egui::Slider::new(&mut c, -100.0..=100.0).show_value(true));
                let rl = ui.checkbox(&mut link, "Link accelerations");
                if ra.changed() || rc.changed() || rl.changed() {
                    out = Some((a, if link { a } else { c }, link));
                }
            });
            if let Some((a, c, l)) = out {
                edit_sel(app, "Contour", |o| {
                    if let Some(e) = &mut o.effects.contour {
                        e.accel = a;
                        e.color_accel = c;
                        e.link_accel = l;
                    }
                });
            }
        }
        "9fd89348" => {
            let cur = first(app, |o| o.effects.contour.as_ref().and_then(|c| c.last_fill)).flatten();
            let on = has(app, |o| o.effects.contour.as_ref().map(|_| ()));
            if let Some(n) = fields::color_combo(ui, id, cur, 44.0, on, &tip(it)) {
                edit_sel(app, "Contour", |o| {
                    if let Some(e) = &mut o.effects.contour {
                        e.last_fill = n;
                    }
                });
            }
        }
        // ---- presets
        "b61b23b8" | "5d0f070b" | "6af525e1" | "dd7baf42" | "b43a901b" | "60c9692d" => preset_list(app, ui, it, k),
        // ---- drop shadow
        "12f3637d" | "7d76ecf3" | "dcda4d7d" => {
            let s = first(app, |o| o.effects.drop_shadow.clone()).flatten();
            let persp = s.as_ref().is_some_and(|s| s.perspective);
            let (v, range, suffix) = match k {
                "12f3637d" => (s.as_ref().map(|s| s.angle), -360.0..=360.0, "°"),
                "7d76ecf3" => (s.as_ref().map(|s| s.fade), 0.0..=100.0, ""),
                _ => (s.as_ref().map(|s| s.stretch), 0.0..=100.0, ""),
            };
            let on = s.is_some() && (persp || k == "7d76ecf3");
            let n = if k == "12f3637d" { fields::spin(app, ui, id, lead(it), v, Fmt::Num(0, ""), 44.0, suffix, on, &tip(it)) } else { fields::slider(app, ui, id, lead(it), v, range.clone(), Fmt::Num(0, ""), 36.0, on, &tip(it)) };
            if let Some(n) = n {
                let n = n.clamp(*range.start(), *range.end());
                let k = k.to_string();
                edit_sel(app, "Drop Shadow", |o| {
                    if let Some(s) = &mut o.effects.drop_shadow {
                        match k.as_str() {
                            "12f3637d" => s.angle = n,
                            "7d76ecf3" => s.fade = n,
                            _ => s.stretch = n,
                        }
                    }
                });
            }
        }
        "cecc8360" => {
            let cur = first(app, |o| o.effects.drop_shadow.as_ref().map(|s| s.merge)).flatten();
            let mut pick = None;
            fields::combo(app, ui, Lead::None, cur.map(|m| m.name()).unwrap_or(MergeMode::Multiply.name()), 96.0, cur.is_some(), &tip(it), |_, ui| {
                for m in MergeMode::ALL {
                    if ui.selectable_label(Some(m) == cur, m.name()).clicked() {
                        pick = Some(m);
                    }
                }
            });
            if let Some(m) = pick {
                edit_sel(app, "Drop Shadow", |o| {
                    if let Some(s) = &mut o.effects.drop_shadow {
                        s.merge = m;
                    }
                });
            }
        }
        // ---- envelope
        "a4910957" => {
            let names = ["Horizontal", "Original", "Putty", "Vertical"];
            let cur = first(app, |o| o.effects.envelope.as_ref().map(|e| e.mapping)).flatten().unwrap_or(to(app).env_mapping).min(3) as usize;
            let mut pick = None;
            fields::combo(app, ui, Lead::None, names[cur], 86.0, doc, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i as u8);
                    }
                }
            });
            if let Some(m) = pick {
                app.settings.topts.env_mapping = m;
                edit_sel(app, "Envelope", |o| {
                    if let Some(e) = &mut o.effects.envelope {
                        e.mapping = m;
                    }
                });
            }
        }
        "032aabda" | "9f2ccc11" => {
            let names = ["Rectangular", "Freehand"];
            let mesh = k == "9f2ccc11";
            let cur = if mesh { to(app).mesh_sel_mode } else { to(app).env_sel_mode }.min(1) as usize;
            let mut pick = None;
            fields::combo(app, ui, Lead::None, names[cur], 86.0, doc, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i as u8);
                    }
                }
            });
            if let Some(m) = pick {
                if mesh {
                    app.settings.topts.mesh_sel_mode = m;
                } else {
                    app.settings.topts.env_sel_mode = m;
                }
            }
        }
        // ---- extrude
        "f43e12b3" => {
            let names = ["Small Back", "Small Front", "Big Back", "Big Front", "Back Parallel", "Front Parallel"];
            let cur = first(app, |o| o.effects.extrude.as_ref().map(|e| e.kind)).flatten();
            let mut pick = None;
            fields::combo(app, ui, Lead::None, names[cur.unwrap_or(0).min(5) as usize], 96.0, cur.is_some(), &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(Some(i as u8) == cur, *n).clicked() {
                        pick = Some(i as u8);
                    }
                }
            });
            if let Some(kd) = pick {
                edit_sel(app, "Extrude", |o| {
                    if let Some(e) = &mut o.effects.extrude {
                        e.kind = kd;
                    }
                });
            }
        }
        "7942470e" => {
            let e = first(app, |o| o.effects.extrude.clone().map(|e| (e, o.bounds()))).flatten();
            let f = len(app);
            let vp = e.as_ref().map(|(e, b)| b.center() + e.depth);
            let r = fields::stacked(app, ui, id, [Lead::Text("x:"), Lead::Text("y:")], [vp.map(|p| p.x), vp.map(|p| p.y)], f, 72.0, "", false, true, e.is_some(), &tip(it));
            if let (Some((e, b)), Some(p)) = (e, vp) {
                if r[0].is_some() || r[1].is_some() {
                    let q = Point::new(r[0].unwrap_or(p.x), r[1].unwrap_or(p.y));
                    let _ = e;
                    let d = q - b.center();
                    edit_sel(app, "Extrude", |o| {
                        if let Some(e) = &mut o.effects.extrude {
                            e.depth = d;
                        }
                    });
                }
            }
        }
        "3b3c408c" => {
            let names = ["VP Locked to Object", "VP Locked to Page"];
            let cur = first(app, |o| o.effects.extrude.as_ref().map(|e| e.vp_page as usize)).flatten();
            let mut pick = None;
            fields::combo(app, ui, Lead::None, names[cur.unwrap_or(0)], 120.0, cur.is_some(), &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(Some(i) == cur, *n).clicked() {
                        pick = Some(i == 1);
                    }
                }
            });
            if let Some(p) = pick {
                edit_sel(app, "Extrude", |o| {
                    if let Some(e) = &mut o.effects.extrude {
                        e.vp_page = p;
                    }
                });
            }
        }
        "6fefe159" => extrude_rotation(app, ui, it),
        "a36ae227" => extrude_color(app, ui, it),
        "89d1a93d" => extrude_bevel(app, ui, it),
        "9b222aa8" => extrude_lights(app, ui, it),
        // ---- mesh
        "0fce601b" => {
            let m = mesh_now(app);
            let c = m.as_ref().and_then(|m| mesh_nodes(app, m).first().and_then(|i| m.colors.get(*i)).copied());
            if let Some(Some(n)) = fields::color_combo(ui, id, c, 44.0, m.is_some(), &tip(it)) {
                edit_mesh(app, "Mesh Fill Color", |m, nodes| {
                    for i in nodes {
                        if let Some(c) = m.colors.get_mut(*i) {
                            *c = n;
                        }
                    }
                });
            }
        }
        "8123419d" => {
            let m = mesh_now(app);
            let a = m.as_ref().map(|m| mesh_nodes(app, m).first().and_then(|i| m.alpha.get(*i)).copied().unwrap_or(0.0));
            if let Some(v) = fields::slider(app, ui, id, lead(it), a, 0.0..=100.0, Fmt::Num(0, ""), 36.0, m.is_some(), &tip(it)) {
                let v = v.clamp(0.0, 100.0);
                edit_mesh(app, "Mesh Transparency", |m, nodes| {
                    if m.alpha.len() != m.colors.len() {
                        m.alpha = vec![0.0; m.colors.len()];
                    }
                    for i in nodes {
                        m.alpha[*i] = v;
                    }
                });
            }
        }
        // ---- knife
        "fc13a9e4" => {
            let names = ["None", "Create Gap", "Overlap"];
            let cur = to(app).knife_span.min(2) as usize;
            let mut pick = None;
            fields::combo(app, ui, lead(it), names[cur], 90.0, doc, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i as u8);
                    }
                }
            });
            if let Some(i) = pick {
                app.settings.topts.knife_span = i;
            }
        }
        "498b531c" => {
            let f = len(app);
            let on = doc && to(app).knife_span != 0;
            if let Some(v) = fields::spin(app, ui, id, lead(it), Some(to(app).knife_width), f, 64.0, "", on, &tip(it)) {
                app.settings.topts.knife_width = v.max(0.0);
            }
        }
        "5a186fb6" => {
            let names = ["Automatic", "Convert Outlines to Objects", "Keep Outlines"];
            let cur = to(app).knife_outline.min(2) as usize;
            let mut pick = None;
            fields::combo(app, ui, lead(it), names[cur], 90.0, doc, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i as u8);
                    }
                }
            });
            if let Some(i) = pick {
                app.settings.topts.knife_outline = i;
            }
        }
        // ---- LiveSketch
        "0b45a1ec" => {
            if let Some(v) = fields::slider(app, ui, id, lead(it), Some(to(app).ls_timer * 1000.0), 0.0..=5000.0, Fmt::Num(0, " ms"), 52.0, doc, &tip(it)) {
                app.settings.topts.ls_timer = v.clamp(0.0, 5000.0) / 1000.0;
            }
        }
        "01e560ce" => {
            let f = len(app);
            let on = doc && to(app).ls_include;
            if let Some(v) = fields::spin(app, ui, id, lead(it), Some(to(app).ls_distance), f, 64.0, "", on, &tip(it)) {
                app.settings.topts.ls_distance = v.max(0.0);
            }
        }
        "be4b89ef" => {
            if let Some(v) = fields::slider(app, ui, id, lead(it), Some(to(app).ls_smoothing), 0.0..=100.0, Fmt::Num(0, ""), 36.0, doc, &tip(it)) {
                app.settings.topts.ls_smoothing = v.clamp(0.0, 100.0);
            }
        }
        // ---- No Selection: autofit page
        "41c7ed55" => {
            let mut fit = false;
            let mut margin = app.settings.topts.autofit_margin;
            fields::panel(app, ui, lead(it), "Autofit page", 96.0, doc, &tip(it), |_, ui| {
                ui.horizontal(|ui| {
                    ui.label("Margins:");
                    ui.add(egui::DragValue::new(&mut margin).range(0.0..=720.0).suffix(" pt"));
                });
                if ui.button("Autofit page").clicked() {
                    fit = true;
                    ui.close();
                }
            });
            app.settings.topts.autofit_margin = margin;
            if fit {
                autofit_page(app, margin);
            }
        }
        // ---- roughen and smudge
        "b6023a11" | "840eef8f" | "5c7f4049" => {
            let t = to(app);
            let (v, range) = match k {
                "b6023a11" => (t.rough_freq, 1.0..=10.0),
                "840eef8f" => (t.rough_dryout, -10.0..=10.0),
                _ => (t.smudge_dryout, -10.0..=10.0),
            };
            if let Some(n) = fields::spin(app, ui, id, lead(it), Some(v), Fmt::Num(0, ""), 40.0, "", doc, &tip(it)) {
                let n = n.clamp(*range.start(), *range.end());
                let t = &mut app.settings.topts;
                match k {
                    "b6023a11" => t.rough_freq = n,
                    "840eef8f" => t.rough_dryout = n,
                    _ => t.smudge_dryout = n,
                }
            }
        }
        "ab030d14" | "12ef64aa" | "c151c016" | "9b372a37" => {
            let t = to(app);
            let (v, on) = match k {
                "ab030d14" => (t.rough_tilt, t.rough_use_tilt),
                "12ef64aa" => (t.smudge_tilt, t.smudge_use_tilt),
                "c151c016" => (t.rough_bearing, t.rough_fixed),
                _ => (t.smudge_bearing, t.smudge_use_bearing),
            };
            if let Some(n) = fields::spin(app, ui, id, lead(it), Some(v), Fmt::Num(0, ""), 44.0, "°", doc && on, &tip(it)) {
                let t = &mut app.settings.topts;
                match k {
                    "ab030d14" => t.rough_tilt = n.clamp(15.0, 90.0),
                    "12ef64aa" => t.smudge_tilt = n.clamp(15.0, 90.0),
                    "c151c016" => t.rough_bearing = n.rem_euclid(360.0),
                    _ => t.smudge_bearing = n.rem_euclid(360.0),
                }
            }
        }
        "0b5da902" => {
            let names = ["Auto", "Fixed Direction"];
            let cur = to(app).rough_fixed as usize;
            let mut pick = None;
            fields::combo(app, ui, lead(it), names[cur], 76.0, doc, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i == 1);
                    }
                }
            });
            if let Some(f) = pick {
                app.settings.topts.rough_fixed = f;
            }
        }
        // ---- Smart Fill
        "b0d50c80" | "b07f479a" => {
            let names = ["Use Default", "Specify", "No Fill"];
            let names = if k == "b07f479a" { ["Use Default", "Specify", "No Outline"] } else { names };
            let fill = k == "b0d50c80";
            let cur = if fill { to(app).sf_fill_src } else { to(app).sf_outline_src }.min(2) as usize;
            let mut pick = None;
            fields::combo(app, ui, lead(it), names[cur], 90.0, doc, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i as u8);
                    }
                }
            });
            if let Some(i) = pick {
                if fill {
                    app.settings.topts.sf_fill_src = i;
                } else {
                    app.settings.topts.sf_outline_src = i;
                }
            }
        }
        "b0760dc1" | "b0cb30bf" => {
            let fill = k == "b0760dc1";
            let t = to(app);
            let (c, on) = if fill { (t.sf_fill, t.sf_fill_src == 1) } else { (t.sf_outline, t.sf_outline_src == 1) };
            if let Some(Some(n)) = fields::color_combo(ui, id, Some(c), 44.0, doc && on, &tip(it)) {
                if fill {
                    app.settings.topts.sf_fill = n;
                } else {
                    app.settings.topts.sf_outline = n;
                }
            }
        }
        "b01ffa70" => {
            let f = len(app);
            let on = doc && to(app).sf_outline_src == 1;
            if let Some(v) = fields::spin(app, ui, id, lead(it), Some(to(app).sf_outline_w), f, 64.0, "", on, &tip(it)) {
                app.settings.topts.sf_outline_w = v.max(0.0);
            }
        }
        // ---- text: variable fonts
        "59055958" => super::text::variable_axes(app, ui, it),
        // ---- Variable Outline
        "e6087c68" => vo_sides(app, ui, it),
        "a536db14" => {
            let v = vo_now(app);
            let node = v.as_ref().and_then(|(n, _)| app.ts.vo_node.and_then(|i| n.get(i)).copied());
            let total = v.as_ref().map(|(n, _)| node.map(|w| w.side1 + w.side2).unwrap_or_else(|| n.iter().map(|w| w.side1 + w.side2).fold(0.0, f64::max)));
            let f = len(app);
            if let Some(nv) = fields::spin(app, ui, id, lead(it), Some(total.unwrap_or(0.0)), f, 64.0, "", app.has_selection(), &tip(it)) {
                let nv = nv.max(0.0);
                let sel = app.ts.vo_node;
                edit_vo(app, "Variable Outline", |nodes, _| {
                    for (i, w) in nodes.iter_mut().enumerate() {
                        if sel.is_some_and(|s| s != i) {
                            continue;
                        }
                        let tot = w.side1 + w.side2;
                        let (a, b) = if tot > 1e-9 { (w.side1 / tot, w.side2 / tot) } else { (0.5, 0.5) };
                        w.side1 = nv * a;
                        w.side2 = nv * b;
                    }
                });
            }
        }
        // ---- Impact
        "99a3bfe4" => {
            let names = ["Radial", "Parallel"];
            let cur = to(app).impact_parallel as usize;
            let mut pick = None;
            fields::combo(app, ui, Lead::None, names[cur], 110.0, doc, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i == 1);
                    }
                }
            });
            if let Some(p) = pick {
                app.settings.topts.impact_parallel = p;
            }
        }
        "352a8cf2" => {
            if let Some(v) = fields::spin(app, ui, id, lead(it), Some(to(app).impact_angle), Fmt::Num(3, ""), 72.0, "°", doc, &tip(it)) {
                app.settings.topts.impact_angle = v.rem_euclid(360.0);
            }
        }
        "bddb7369" => {
            let names = ["Aligned Start and End", "Random Start", "Random End", "Random Start and End"];
            let cur = to(app).impact_ends.min(3) as usize;
            let mut pick = None;
            fields::combo(app, ui, lead(it), "", 16.0, doc, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i as u8);
                    }
                }
            });
            if let Some(i) = pick {
                app.settings.topts.impact_ends = i;
            }
        }
        "aa85aae1" | "ad61ddb5" => {
            let width = k == "aa85aae1";
            let v = if width { to(app).impact_width } else { to(app).impact_spacing };
            let f = len(app);
            let r = fields::stacked(app, ui, id, [Lead::Text("Min:"), Lead::Text("Max:")], [Some(v[0]), Some(v[1])], f, 76.0, "", true, true, doc, &tip(it));
            let t = &mut app.settings.topts;
            let target = if width { &mut t.impact_width } else { &mut t.impact_spacing };
            if let Some(a) = r[0] {
                target[0] = a.max(0.01);
                target[1] = target[1].max(target[0]);
            }
            if let Some(b) = r[1] {
                target[1] = b.max(0.01);
                target[0] = target[0].min(target[1]);
            }
        }
        "df313a7e" | "4bb88774" => {
            let width = k == "df313a7e";
            let v = if width { to(app).impact_width_steps } else { to(app).impact_spacing_steps };
            if let Some(n) = fields::spin(app, ui, id, lead(it), Some(v as f64), Fmt::Num(0, ""), 56.0, "", doc, &tip(it)) {
                let n = n.clamp(0.0, 100.0) as u32;
                if width {
                    app.settings.topts.impact_width_steps = n;
                } else {
                    app.settings.topts.impact_spacing_steps = n;
                }
            }
        }
        "c393853c" => {
            let names = ["Wedge", "Tapered Both Ends", "Tapered Start", "Straight"];
            let cur = to(app).impact_style.min(3) as usize;
            let mut pick = None;
            fields::combo(app, ui, Lead::None, names[cur], 110.0, doc, &tip(it), |_, ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(i == cur, *n).clicked() {
                        pick = Some(i as u8);
                    }
                }
            });
            if let Some(i) = pick {
                app.settings.topts.impact_style = i;
            }
        }
        "a6027411" => {
            if let Some(v) = fields::slider(app, ui, id, lead(it), Some(to(app).impact_widest), 0.0..=100.0, Fmt::Num(0, " %"), 44.0, doc, &tip(it)) {
                app.settings.topts.impact_widest = v.clamp(0.0, 100.0);
            }
        }
        _ => return false,
    }
    true
}

// ----------------------------------------------------------------------------- field bodies

fn table_now(app: &AnyVectorApp) -> Option<vector_core::object::TableObj> {
    selected_kind(app, |k| if let ObjKind::Table(t) = k { Some(t.clone()) } else { None })
}

fn edit_table(app: &mut AnyVectorApp, label: &str, f: impl Fn(&mut vector_core::object::TableObj, Option<vector_core::object::CellRange>)) {
    let range = table_now(app).and_then(|t| app.ts.table_range(&t));
    edit_sel(app, label, |o| {
        if let ObjKind::Table(t) = &mut o.kind {
            f(t, range);
        }
    });
}

fn table_color(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item, k: &str) {
    let t = table_now(app);
    let fill = k == "f62e2a7e";
    let range = t.as_ref().and_then(|t| app.ts.table_range(t));
    let cur = t.as_ref().map(|t| match (fill, range) {
        (true, Some(r)) => t.cell(r[0], r[1]).fill,
        (true, None) => first(app, |o| o.style.fill.main_color()).flatten(),
        (false, _) => Some(t.border),
    });
    if let Some(n) = fields::color_combo(ui, pid(ui, k), cur.flatten(), 44.0, t.is_some(), &tip(it)) {
        if fill && range.is_none() {
            edit_sel(app, "Table Fill", |o| o.style.fill = n.map(Fill::Solid).unwrap_or(Fill::None));
            return;
        }
        edit_table(app, if fill { "Cell Fill" } else { "Table Border" }, |t, r| {
            if fill {
                if let Some(r) = r {
                    for row in r[0]..=r[2] {
                        for col in r[1]..=r[3] {
                            t.cell_mut(row, col).fill = n;
                        }
                    }
                }
            } else if let Some(c) = n {
                t.border = c;
            }
        });
    }
}

fn table_options(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item) {
    let t = table_now(app);
    let on = t.is_some();
    let mut out = None;
    fields::panel(app, ui, Lead::None, "Options", 70.0, on, &tip(it), |_, ui| {
        let Some(t) = &t else { return };
        let mut auto = t.auto_resize;
        let mut sep = t.spacing.is_some();
        let (mut h, mut v) = t.spacing.unwrap_or((2.0, 2.0));
        let mut ch = ui.checkbox(&mut auto, "Automatically resize cells while typing").changed();
        ch |= ui.checkbox(&mut sep, "Separated cell borders").changed();
        ui.add_enabled_ui(sep, |ui| {
            ui.horizontal(|ui| {
                ui.label("Horizontal cell spacing:");
                ch |= ui.add(egui::DragValue::new(&mut h).range(0.0..=72.0).suffix(" pt")).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Vertical cell spacing:");
                ch |= ui.add(egui::DragValue::new(&mut v).range(0.0..=72.0).suffix(" pt")).changed();
            });
        });
        if ch {
            out = Some((auto, sep.then_some((h, v))));
        }
    });
    if let Some((auto, sp)) = out {
        edit_table(app, "Table Options", |t, _| {
            t.auto_resize = auto;
            t.spacing = sp;
            if auto {
                t.fit_rows();
            }
        });
    }
}

fn table_margins(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item) {
    let t = table_now(app);
    let m = t.as_ref().map(|t| t.margins);
    let (u, dpi) = unit_dpi(app);
    let text = m.map(|m| u.format(m[0], dpi)).unwrap_or_default();
    let mut out = None;
    fields::panel(app, ui, Lead::Text("Margins"), &text, 70.0, t.is_some(), &tip(it), |_, ui| {
        let Some(mut m) = m else { return };
        let mut lock = m.iter().all(|x| (x - m[0]).abs() < 1e-9);
        let mut ch = false;
        for (i, n) in ["Top:", "Bottom:", "Left:", "Right:"].iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(*n);
                if ui.add(egui::DragValue::new(&mut m[i]).range(0.0..=144.0).suffix(" pt")).changed() {
                    ch = true;
                    if lock {
                        let v = m[i];
                        m = [v; 4];
                    }
                }
            });
        }
        ch |= ui.checkbox(&mut lock, "Lock margins").changed();
        if ch {
            out = Some(m);
        }
    });
    if let Some(m) = out {
        edit_table(app, "Cell Margins", |t, _| {
            t.margins = m;
            if t.auto_resize {
                t.fit_rows();
            }
        });
    }
}

fn table_cell_size(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item) {
    let t = table_now(app);
    let range = t.as_ref().and_then(|t| app.ts.table_range(t));
    let size = t.as_ref().zip(range).map(|(t, r)| (t.col_widths[r[1]], t.row_heights[r[0]]));
    let f = len(app);
    let r = fields::stacked(app, ui, pid(ui, "82f9896c"), [Lead::Icon("size_w"), Lead::Icon("size_h")], [size.map(|s| s.0), size.map(|s| s.1)], f, 76.0, "", true, true, size.is_some(), &tip(it));
    if size.is_some() && (r[0].is_some() || r[1].is_some()) {
        edit_table(app, "Cell Size", |t, rg| {
            let Some(rg) = rg else { return };
            if let Some(w) = r[0] {
                for c in rg[1]..=rg[3] {
                    t.col_widths[c] = w.max(1.0);
                }
            }
            if let Some(h) = r[1] {
                for rr in rg[0]..=rg[2] {
                    t.row_heights[rr] = h.max(1.0);
                }
            }
        });
    }
}

fn wrap_text(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item) {
    let on = app.has_selection();
    let cur = first(app, |o| o.effects.wrap).flatten();
    let mut out: Option<Option<TextWrap>> = None;
    fields::panel(app, ui, Lead::Icon(if it.icon.is_empty() { "wrap_text" } else { it.icon }), "", 16.0, on, &tip(it), |_, ui| {
        if ui.selectable_label(cur.is_none(), "None").clicked() {
            out = Some(None);
        }
        let mut group = "";
        for (i, (g, n)) in WRAP_STYLES.iter().enumerate() {
            if *g != group {
                ui.separator();
                ui.label(egui::RichText::new(*g).color(crate::theme::TEXT_DIM()));
                group = g;
            }
            if ui.selectable_label(cur.is_some_and(|w| w.style == i as u8), *n).clicked() {
                out = Some(Some(TextWrap { style: i as u8, offset: cur.map(|w| w.offset).unwrap_or(8.0) }));
            }
        }
        ui.separator();
        ui.add_enabled_ui(cur.is_some(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Text wrap offset:");
                let mut off = cur.map(|w| w.offset).unwrap_or(8.0);
                if ui.add(egui::DragValue::new(&mut off).range(0.0..=720.0).suffix(" pt")).changed() {
                    if let Some(w) = cur {
                        out = Some(Some(TextWrap { offset: off, ..w }));
                    }
                }
            });
        });
    });
    if let Some(w) = out {
        edit_sel(app, "Wrap Paragraph Text", |o| o.effects.wrap = w);
        if let Some(d) = app.doc_mut() {
            d.doc.update_text_flow();
        }
    }
}

fn dim_arrow(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item) {
    let cur = dimension_now(app).map(|d| d.arrow).unwrap_or(to(app).dim_arrow);
    let text = match cur {
        None => "—".to_string(),
        Some(a) => crate::dialogs::object::arrow_name(app, a),
    };
    let choices = crate::dialogs::object::arrow_choices(app);
    let mut pick: Option<Option<ArrowHead>> = None;
    fields::combo(app, ui, Lead::None, &text, 48.0, app.has_doc(), &tip(it), |_, ui| {
        if ui.selectable_label(cur.is_none(), "None").clicked() {
            pick = Some(None);
        }
        for (a, name) in &choices {
            if ui.selectable_label(cur == Some(*a), name).clicked() {
                pick = Some(Some(*a));
            }
        }
    });
    if let Some(a) = pick {
        crate::dialogs::object::ensure_arrow(app, a);
        set_dim(app, |d| d.arrow = a, |t| t.dim_arrow = a);
    }
}

fn dim_ext(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item) {
    let d = dimension_now(app);
    let (gap, ext) = d.as_ref().map(|d| (d.ext_gap, d.ext_len)).unwrap_or((to(app).dim_ext_gap, to(app).dim_ext_len));
    let mut out = None;
    fields::panel(app, ui, lead(it), "", 16.0, app.has_doc(), &tip(it), |_, ui| {
        let mut use_gap = gap > 0.0;
        let mut g = if gap > 0.0 { gap } else { 2.0 };
        let mut use_ext = ext.is_some();
        let mut e = ext.unwrap_or(2.0);
        let mut ch = false;
        ui.horizontal(|ui| {
            ch |= ui.checkbox(&mut use_gap, "Gap to object:").changed();
            ch |= ui.add_enabled(use_gap, egui::DragValue::new(&mut g).range(0.0..=144.0).suffix(" pt")).changed();
        });
        ui.horizontal(|ui| {
            ch |= ui.checkbox(&mut use_ext, "Extend beyond dimension line:").changed();
            ch |= ui.add_enabled(use_ext, egui::DragValue::new(&mut e).range(0.0..=144.0).suffix(" pt")).changed();
        });
        if ch {
            out = Some((if use_gap { g } else { 0.0 }, use_ext.then_some(e)));
        }
    });
    if let Some((g, e)) = out {
        set_dim(
            app,
            |d| {
                d.ext_gap = g;
                d.ext_len = e;
            },
            |t| {
                t.dim_ext_gap = g;
                t.dim_ext_len = e;
            },
        );
    }
}

fn brush_picker(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item) {
    let cur = to(app).paint_brush.min(BRUSHES.len() - 1);
    let favs = to(app).paint_favorites.clone();
    let mut pick = None;
    let mut fav = None;
    fields::panel(app, ui, Lead::None, BRUSHES[cur].0, 120.0, app.has_doc(), &tip(it), |_, ui| {
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            if !favs.is_empty() {
                ui.label(egui::RichText::new("Favorites").color(crate::theme::TEXT_DIM()));
                for &b in &favs {
                    if let Some(br) = BRUSHES.get(b) {
                        if ui.selectable_label(b == cur, br.0).clicked() {
                            pick = Some(b);
                        }
                    }
                }
                ui.separator();
            }
            for (i, b) in BRUSHES.iter().enumerate() {
                ui.horizontal(|ui| {
                    let star = if favs.contains(&i) { "★" } else { "☆" };
                    if ui.small_button(star).on_hover_text(if favs.contains(&i) { "Remove from Favorites" } else { "Add to Favorites" }).clicked() {
                        fav = Some(i);
                    }
                    if ui.selectable_label(i == cur, b.0).clicked() {
                        pick = Some(i);
                    }
                });
            }
        });
    });
    if let Some(i) = fav {
        let f = &mut app.settings.topts.paint_favorites;
        if let Some(p) = f.iter().position(|x| *x == i) {
            f.remove(p);
        } else {
            f.push(i);
        }
    }
    if let Some(b) = pick {
        choose_brush(app, b);
    }
}

fn eye_lists(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item, k: &str) {
    let (title, names): (&str, &[&str]) = match k {
        "be78ee55" => ("Properties", &["Outline", "Fill", "Text"]),
        "4342a7e3" => ("Transformations", &["Size", "Rotation", "Position"]),
        _ => ("Effects", &["Perspective", "Envelope", "Blend", "Extrude", "Contour", "Lens", "PowerClip", "Drop Shadow", "Distortion"]),
    };
    let mut vals: Vec<bool> = match k {
        "be78ee55" => to(app).eye_props.to_vec(),
        "4342a7e3" => to(app).eye_xforms.to_vec(),
        _ => to(app).eye_effects.to_vec(),
    };
    let mut changed = false;
    fields::panel(app, ui, Lead::None, title, 96.0, app.has_doc(), &tip(it), |_, ui| {
        changed = checks(ui, names, &mut vals);
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Select All").clicked() {
                vals.iter_mut().for_each(|v| *v = true);
                changed = true;
            }
            if ui.button("Clear All").clicked() {
                vals.iter_mut().for_each(|v| *v = false);
                changed = true;
            }
        });
    });
    if changed {
        let t = &mut app.settings.topts;
        match k {
            "be78ee55" => t.eye_props.copy_from_slice(&vals),
            "4342a7e3" => t.eye_xforms.copy_from_slice(&vals),
            _ => t.eye_effects.copy_from_slice(&vals),
        }
    }
}

fn preset_list(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item, k: &str) {
    let Some(&(_, _, _, effect)) = PRESET_LISTS.iter().find(|p| p.0 == k) else { return };
    let names = preset_names(app, effect);
    let cur = to(app).preset_sel.get(effect).filter(|n| names.contains(n)).cloned();
    let mut pick = None;
    fields::combo(app, ui, Lead::None, cur.as_deref().unwrap_or("Presets..."), 110.0, app.has_doc(), &tip(it), |_, ui| {
        if names.is_empty() {
            ui.label(egui::RichText::new("No presets saved").color(crate::theme::TEXT_DIM()));
        }
        for n in &names {
            if ui.selectable_label(cur.as_ref() == Some(n), n).clicked() {
                pick = Some(n.clone());
            }
        }
    });
    if let Some(n) = pick {
        let json = to(app).presets.get(effect).and_then(|l| l.iter().find(|p| p.0 == n)).map(|p| p.1.clone());
        app.settings.topts.preset_sel.insert(effect.to_string(), n);
        if let Some(j) = json {
            apply_preset(app, effect, &j);
        }
    }
}

/// The Extrude tool's drop-down panels: the pages of the Extrude docker.
fn extrude_panel(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item, label: &str, page: impl Fn(&mut AnyVectorApp, &mut Ui, &mut vector_core::object::ExtrudeEffect) -> bool) {
    let e = first(app, |o| o.effects.extrude.clone()).flatten();
    let mut out = None;
    fields::panel(app, ui, lead(it), "", 16.0, e.is_some(), &tip(it), |app, ui| {
        let Some(mut e) = e.clone() else { return };
        ui.set_width(220.0);
        if page(app, ui, &mut e) {
            out = Some(e);
        }
    });
    if let Some(e) = out {
        edit_sel(app, label, |o| {
            if o.effects.extrude.is_some() {
                o.effects.extrude = Some(e.clone());
            }
        });
    }
}

fn extrude_rotation(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item) {
    extrude_panel(app, ui, it, "Extrude Rotation", |app, ui, e| crate::dockers::extrude_pages::rotation_ui(&mut app.icons, ui, e, true));
}

fn extrude_color(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item) {
    extrude_panel(app, ui, it, "Extrude Color", |_, ui, e| crate::dockers::extrude_pages::color_ui(ui, e, true));
}

fn extrude_bevel(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item) {
    extrude_panel(app, ui, it, "Extrude Bevel", |app, ui, e| {
        let (un, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
        crate::dockers::extrude_pages::bevel_ui(&mut app.icons, ui, e, true, un, dpi)
    });
}

fn extrude_lights(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item) {
    extrude_panel(app, ui, it, "Extrude Lighting", |app, ui, e| {
        let mut sel = app.dockers.ui.ex_light;
        let c = crate::dockers::extrude_pages::lighting_ui(ui, e, true, &mut sel);
        app.dockers.ui.ex_light = sel;
        c
    });
}

fn vo_sides(app: &mut AnyVectorApp, ui: &mut Ui, it: &Item) {
    let v = vo_now(app);
    let node = v.as_ref().and_then(|(n, _)| app.ts.vo_node.and_then(|i| n.get(i)).copied());
    let shown = node.map(|w| (w.side1, w.side2)).or_else(|| v.as_ref().and_then(|(n, _)| n.first().map(|w| (w.side1, w.side2))));
    let f = len(app);
    let r = fields::stacked(app, ui, pid(ui, "e6087c68"), [Lead::Text("Side 1:"), Lead::Text("Side 2:")], [Some(shown.map(|s| s.0).unwrap_or(0.0)), Some(shown.map(|s| s.1).unwrap_or(0.0))], f, 60.0, "", false, true, app.has_selection(), &tip(it));
    if r[0].is_none() && r[1].is_none() {
        return;
    }
    let lock = to(app).vo_lock;
    let sel = app.ts.vo_node;
    edit_vo(app, "Variable Outline", |nodes, _| {
        for (i, w) in nodes.iter_mut().enumerate() {
            if sel.is_some_and(|s| s != i) {
                continue;
            }
            let ratio = if w.side1 > 1e-9 { w.side2 / w.side1 } else { 1.0 };
            if let Some(a) = r[0] {
                w.side1 = a.max(0.0);
                if lock {
                    w.side2 = w.side1 * ratio;
                }
            }
            if let Some(b) = r[1] {
                let inv = if w.side2 > 1e-9 { w.side1 / w.side2 } else { 1.0 };
                w.side2 = b.max(0.0);
                if lock {
                    w.side1 = w.side2 * inv;
                }
            }
        }
    });
}

/// Curve Smoothness of the Shape tool: fewer nodes on the selected curves.
fn smooth_nodes(app: &mut AnyVectorApp, v: f64) {
    if v <= 0.0 {
        return;
    }
    let Some(d) = app.doc_mut() else { return };
    let ids: Vec<_> = d.node_sel.iter().map(|n| n.0).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    d.modify("Curve Smoothness", |doc, _| {
        for id in &ids {
            let Some(o) = doc.find_mut(*id) else { continue };
            let ObjKind::Path(np) = &o.kind else { continue };
            let bez = np.to_bez();
            use kurbo::Shape;
            let diag = bez.bounding_box().size().to_vec2().hypot().max(1.0);
            let simple = vector_core::geom::simplify_path(&bez, diag * 0.002 * v);
            o.kind = ObjKind::Path(vector_core::path::NodePath::from_bez(&simple));
        }
    });
    if let Some(d) = app.doc_mut() {
        d.node_sel.clear();
    }
}

/// Autofit page: the page takes the size of the objects on it, plus the margins.
fn autofit_page(app: &mut AnyVectorApp, margin: f64) {
    let Some(d) = app.doc_mut() else { return };
    let page = d.page;
    let bb = vector_core::object::union_bounds(d.doc.pages[page].all_objects().map(|o| d.doc.bounds_of(o)));
    if bb.width() <= 0.0 && bb.height() <= 0.0 {
        return;
    }
    let r = bb.inflate(margin, margin);
    d.modify("Autofit Page", |doc, _| {
        let shift = Vec2::new(-r.x0, -r.y0);
        let ids: Vec<_> = doc.pages[page].layers.iter().flat_map(|l| l.objects.iter().map(|o| o.id)).collect();
        doc.transform_objects(&ids, Affine::translate(shift));
        let pg = &mut doc.pages[page];
        pg.width = r.width();
        pg.height = r.height();
        for g in pg.guides.iter_mut() {
            g.pos += shift;
        }
    });
    if let Some(d) = app.doc_mut() {
        d.zoom_page();
    }
}

// ----------------------------------------------------------------------------- dialogs

/// Shape tool ▸ Align: line up the selected nodes horizontally, vertically or both.
pub fn node_align_dialog(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut v) = app.dialogs.node_align else { return };
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = crate::dialogs::window(ctx, "node_align", "Node Align", egui::vec2(260.0, 170.0), |ui| {
        ui.add_space(10.0);
        ui.checkbox(&mut v[0], "Align horizontal");
        ui.checkbox(&mut v[1], "Align vertical");
        ui.add_enabled(v[0] && v[1], egui::Checkbox::new(&mut v[2], "Align control points"));
        let (o, c) = crate::dialogs::ok_cancel_plain(ui, v[0] || v[1]);
        ok = o;
        cancel = c;
    });
    if cancel || dismissed {
        app.dialogs.node_align = None;
        return;
    }
    if !ok {
        app.dialogs.node_align = Some(v);
        return;
    }
    app.dialogs.node_align = None;
    let Some(d) = app.doc_mut() else { return };
    let nodes = d.node_sel.clone();
    if nodes.len() < 2 {
        return;
    }
    // the nodes line up on the last selected one (as in the original); with both directions,
    // they all meet at its position
    let Some(&(lid, ls, ln)) = nodes.last() else { return };
    let anchor = d.doc.find(lid).and_then(|o| if let ObjKind::Path(np) = &o.kind { np.subpaths.get(ls).and_then(|s| s.nodes.get(ln)).map(|n| o.transform * n.p) } else { None });
    let Some(target) = anchor else { return };
    d.modify("Align Nodes", |doc, _| {
        for (id, si, ni) in &nodes {
            let Some(o) = doc.find_mut(*id) else { continue };
            let tr = o.transform;
            let inv = tr.inverse();
            let ObjKind::Path(np) = &mut o.kind else { continue };
            let Some(n) = np.subpaths.get_mut(*si).and_then(|s| s.nodes.get_mut(*ni)) else { continue };
            let wp = tr * n.p;
            // horizontal alignment puts the nodes on one horizontal line (same y)
            let q = Point::new(if v[1] { target.x } else { wp.x }, if v[0] { target.y } else { wp.y });
            let dv = inv * q - n.p;
            n.p += dv;
            if v[2] {
                n.c_in = n.p;
                n.c_out = n.p;
            } else {
                n.c_in += dv;
                n.c_out += dv;
            }
        }
    });
}
