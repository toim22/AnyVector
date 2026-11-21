//! Corel Presentation Exchange (CMX) export: the file to hand to someone who uses CorelDRAW.
//!
//! AnyVector's own `.cdr` is a container CorelDRAW does not read, and CorelDRAW's `.cdr` is an
//! undocumented format of its own. CMX is the exchange format CorelDRAW has read since version 6
//! (File ▸ Open and File ▸ Import, up to CorelDRAW 2026), so a CMX copy is what opens in
//! CorelDRAW, old versions included, with editable curves, fills, outlines, groups, layers and
//! pages.
//!
//! The file is the 32-bit variant of the format (CMX version 2: a 254000th of an inch per unit,
//! the origin at the center of the page, y up), laid out the way CorelDRAW 2026's own CMX export
//! lays it out:
//!
//! ```text
//! RIFF 'CMX1'
//!   cont          the header: byte order, coordinate size, units, index offset, drawing box
//!   page          one per page: begin page, then layers, groups and curves, end page
//!   rlst          one per page: the page's layers
//!   rotl rott rpen rdot rota rclr rscr      outlines, line styles, pens, dashes, arrowheads,
//!                                            colors, screens (each record a tagged field)
//!   LIST 'indx'   ixlr (layers of each page), ixtl (the layer indexes), ixpg (pages),
//!                 ixmr (the master index: table number to offset)
//!   LIST 'INFO'   keywords and comment
//! ```
//!
//! What a CMX file cannot hold is converted to what it can: text and dimension labels become
//! curves, effects (blends, contours, extrusions, block shadows, PowerClips, symbols, tables)
//! become the curves they draw, pattern and mesh fills their main color. Bitmaps, raster
//! effects (drop shadows, lenses, bevels) and transparency are left out; [`export`] says which
//! of these the drawing had, one line each, in English (keys of the app's translation file).

use crate::flat::{FlatOptions, Flattener, Paint, Prim, StrokeStyle};
use crate::{IoError, Result};
use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape, Vec2};
use std::path::Path;
use vector_core::color::Color;
use vector_core::document::Document;
use vector_core::geom;
use vector_core::object::{ObjKind, Object};
use vector_core::style::{Fill, Gradient, GradientKind, LineCap, LineJoin, Outline, OutlinePosition, Transparency};

/// File units per point (a 254000th of an inch).
pub const UNITS_PER_POINT: f64 = 254_000.0 / 72.0;

/// What to put in the file.
#[derive(Clone, Debug, Default)]
pub struct CmxOptions {
    /// The pages (0-based); empty: all of them.
    pub pages: Vec<usize>,
}

/// What a CMX copy changes or leaves out, one line each.
pub const NOTE_TEXT: &str = "Text was converted to curves.";
pub const NOTE_BITMAPS: &str = "Bitmaps were left out.";
pub const NOTE_RASTER_EFFECTS: &str = "Drop shadows, lenses and bevels were left out.";
pub const NOTE_TRANSPARENCY: &str = "Transparency was left out.";
pub const NOTE_FILLS: &str = "Pattern, texture and mesh fills were replaced by their main color.";
pub const NOTE_HIDDEN: &str = "Hidden layers and objects were left out.";

/// Write a CMX copy of `doc` to `path`; returns what the copy changes or leaves out (see the
/// notes above).
pub fn export(doc: &Document, path: &Path, opts: &CmxOptions) -> Result<Vec<String>> {
    let (bytes, notes) = to_bytes(doc, opts)?;
    std::fs::write(path, bytes)?;
    Ok(notes)
}

/// The CMX file of `doc`, and what it changes or leaves out.
pub fn to_bytes(doc: &Document, opts: &CmxOptions) -> Result<(Vec<u8>, Vec<String>)> {
    let pages: Vec<usize> = if opts.pages.is_empty() { (0..doc.pages.len()).collect() } else { opts.pages.iter().copied().filter(|p| *p < doc.pages.len()).collect() };
    if pages.is_empty() {
        return Err(IoError::Format("the drawing has no pages".into()));
    }
    let mut b = Builder { doc, tables: Tables::new(), notes: Notes::default(), to_file: Affine::IDENTITY };
    let built: Vec<PageItems> = pages.iter().map(|p| b.page(*p)).collect();
    let bytes = write_file(&built, &b.tables);
    let n = &b.notes;
    let notes = [(n.text, NOTE_TEXT), (n.bitmaps, NOTE_BITMAPS), (n.raster_effects, NOTE_RASTER_EFFECTS), (n.transparency, NOTE_TRANSPARENCY), (n.fills, NOTE_FILLS), (n.hidden, NOTE_HIDDEN)]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, s)| s.to_string())
        .collect();
    Ok((bytes, notes))
}

// ------------------------------------------------------------------------------------------
// The drawing as CMX items
// ------------------------------------------------------------------------------------------

/// A fill as CMX stores it.
#[derive(Clone, Debug, PartialEq)]
enum CmxFill {
    /// A color number (1-based).
    Solid(u16),
    /// A fountain fill: type (0 linear, 1 radial, 2 conical, 3 square), angle (degrees),
    /// the center's offset from the box's center (percent of its size), stops (color number,
    /// position in percent).
    Fountain { kind: u16, angle: f64, cx: i32, cy: i32, stops: Vec<(u16, u16)> },
}

/// One drawing instruction, in file units.
#[derive(Clone, Debug)]
enum Item {
    Curve { path: BezPath, fill: Option<CmxFill>, outline: Option<u16>, bbox: Rect },
    Group { items: Vec<Item>, bbox: Rect },
}

impl Item {
    fn bbox(&self) -> Rect {
        match self {
            Item::Curve { bbox, .. } | Item::Group { bbox, .. } => *bbox,
        }
    }
}

struct LayerItems {
    name: String,
    items: Vec<Item>,
}

struct PageItems {
    /// Points.
    width: f64,
    height: f64,
    layers: Vec<LayerItems>,
}

fn union(items: &[Item]) -> Rect {
    items.iter().map(|i| i.bbox()).reduce(|a, b| a.union(b)).unwrap_or(Rect::ZERO)
}

/// A color as the color table stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorRec {
    Rgb([u8; 3]),
    /// Inks in percent.
    Cmyk([u8; 4]),
}

impl ColorRec {
    fn of(c: Color) -> ColorRec {
        match c.cmyk {
            Some([c, m, y, k]) => ColorRec::Cmyk([c.min(100), m.min(100), y.min(100), k.min(100)]),
            None => ColorRec::Rgb([c.r, c.g, c.b]),
        }
    }
}

/// The description tables; every number is 1-based, as CMX counts them.
struct Tables {
    colors: Vec<ColorRec>,
    /// Width (file units), aspect (percent), angle (millionths of a degree).
    pens: Vec<(i32, u16, i32)>,
    /// Spec (0x02 solid, 0x04 dashed, 0x10 behind the fill, 0x20 scaled with the object), caps
    /// (low nibble) and joins (high nibble).
    line_styles: Vec<(u8, u8)>,
    /// Dash patterns in multiples of the line width; the first is solid.
    dashes: Vec<Vec<u16>>,
    /// Line style, screen, color, arrowheads, pen, dashes.
    outlines: Vec<[u16; 6]>,
}

impl Tables {
    fn new() -> Tables {
        Tables { colors: vec![], pens: vec![], line_styles: vec![], dashes: vec![vec![]], outlines: vec![] }
    }

    fn index<T: PartialEq>(list: &mut Vec<T>, v: T) -> u16 {
        let i = match list.iter().position(|x| *x == v) {
            Some(i) => i,
            None => {
                list.push(v);
                list.len() - 1
            }
        };
        (i + 1).min(u16::MAX as usize) as u16
    }

    fn color(&mut self, c: Color) -> u16 {
        Self::index(&mut self.colors, ColorRec::of(c))
    }

    /// The outline record of a pen: `width` in points, `dash` in multiples of the width, `nib`
    /// the calligraphic stretch (percent) and angle (degrees).
    #[allow(clippy::too_many_arguments)]
    fn outline(&mut self, color: Color, width: f64, cap: LineCap, join: LineJoin, dash: &[f64], behind: bool, scaled: bool, nib: (f64, f64)) -> u16 {
        // CorelDRAW's thinnest outline (a hairline) is about a fifth of a point
        let w = (width.max(0.216) * UNITS_PER_POINT).round().min(i32::MAX as f64) as i32;
        let (stretch, angle) = nib;
        let (aspect, angle) = if stretch > 0.0 && stretch < 99.5 { (stretch.round() as u16, (angle * 1e6).round() as i32) } else { (100, 0) };
        let pen = Self::index(&mut self.pens, (w, aspect, angle));
        let dash: Vec<u16> = dash.iter().map(|d| d.round().clamp(1.0, 1000.0) as u16).collect();
        let mut spec = if dash.is_empty() { 0x02 } else { 0x04 };
        if behind {
            spec |= 0x10;
        }
        if scaled {
            spec |= 0x20;
        }
        let caps = match cap {
            LineCap::Butt => 0,
            LineCap::Round => 1,
            LineCap::Square => 2,
        };
        let joins = match join {
            LineJoin::Miter => 0,
            LineJoin::Round => 1,
            LineJoin::Bevel => 2,
        };
        let style = Self::index(&mut self.line_styles, (spec, caps | (joins << 4)));
        let dash = if dash.is_empty() { 1 } else { Self::index(&mut self.dashes, dash) };
        let color = self.color(color);
        Self::index(&mut self.outlines, [style, 1, color, 1, pen, dash])
    }
}

/// What the drawing had that the copy changes or leaves out.
#[derive(Default)]
struct Notes {
    text: bool,
    bitmaps: bool,
    raster_effects: bool,
    transparency: bool,
    fills: bool,
    hidden: bool,
}

struct Builder<'a> {
    doc: &'a Document,
    tables: Tables,
    notes: Notes,
    /// Page points to file units, for the page being built.
    to_file: Affine,
}

/// The most points one curve instruction holds (its point list is a tagged field of at most
/// 65535 bytes, 9 bytes a point).
const MAX_POINTS: usize = 7000;

impl<'a> Builder<'a> {
    fn page(&mut self, page: usize) -> PageItems {
        let doc = self.doc;
        let pg = &doc.pages[page];
        let (w, h) = (pg.width, pg.height);
        let k = UNITS_PER_POINT;
        self.to_file = Affine::new([k, 0.0, 0.0, -k, -w / 2.0 * k, h / 2.0 * k]);
        let mut layers = vec![];
        for (_, _, l) in doc.layers_on_page(page) {
            if !l.visible {
                self.notes.hidden |= !l.objects.is_empty();
                continue;
            }
            let mut items = vec![];
            if layers.is_empty() {
                if let Some(bg) = pg.background {
                    // the page's background color: a rectangle under everything
                    let fill = Some(CmxFill::Solid(self.tables.color(bg)));
                    items.extend(self.curves(self.to_file * pg.rect().to_path(0.1), fill, None, 0.0));
                }
            }
            for o in &l.objects {
                let o = doc.object_on_page(o, page);
                self.object(&o, &mut items);
            }
            layers.push(LayerItems { name: l.name.clone(), items });
        }
        if layers.is_empty() {
            layers.push(LayerItems { name: "Layer 1".into(), items: vec![] });
        }
        PageItems { width: w, height: h, layers }
    }

    fn object(&mut self, o: &Object, out: &mut Vec<Item>) {
        if !o.visible {
            self.notes.hidden = true;
            return;
        }
        if !matches!(o.style.transparency, Transparency::None) && !matches!(o.style.transparency, Transparency::Uniform(t) if t <= 0.0) {
            self.notes.transparency = true;
        }
        let o = self.vector_only(o);
        let e = &o.effects;
        let drawn_by_effect = e.block_shadow.is_some() || e.extrude.is_some() || e.contour.is_some() || e.warps_geometry();
        if let (ObjKind::Group(kids), false) = (&o.kind, drawn_by_effect) {
            let mut items = vec![];
            for k in kids {
                self.object(k, &mut items);
            }
            push_group(out, items);
            return;
        }
        if let Some(items) = self.direct(&o) {
            out.extend(items);
            return;
        }
        // everything else as the curves it draws
        let opts = FlatOptions { raster_dpi: 150.0, conical_gradients: true, repeat_gradients: true, gradient_alpha: true, printing: false, text_as_text: false };
        let mut f = Flattener::new(self.doc, opts);
        f.object(&o);
        let mut text = false;
        o.walk(&mut |x| text |= matches!(x.kind, ObjKind::Text(_) | ObjKind::Dimension(_) | ObjKind::Table(_)));
        self.notes.text |= text;
        let items = self.prims(f.out);
        if items.len() == 1 {
            out.extend(items);
        } else {
            push_group(out, items);
        }
    }

    /// The object without what a CMX file cannot hold: raster effects, transparency, and
    /// fills other than uniform and fountain fills (replaced by their main color).
    fn vector_only(&mut self, o: &Object) -> Object {
        let mut c = o.clone();
        let (mut raster, mut fills, mut bitmaps, mut hidden) = (false, false, false, false);
        c.walk_mut(&mut |x| {
            let e = &mut x.effects;
            raster |= e.drop_shadow.is_some() || e.inner_shadow.is_some() || e.lens.is_some() || !e.bitmap.is_empty() || e.bevel.is_some();
            e.drop_shadow = None;
            e.inner_shadow = None;
            e.lens = None;
            e.bitmap.clear();
            e.bevel = None;
            x.style.transparency = Transparency::None;
            match &x.style.fill {
                Fill::Pattern(_) | Fill::Mesh(_) => {
                    fills = true;
                    x.style.fill = match x.style.fill.main_color() {
                        Some(c) => Fill::Solid(c),
                        None => Fill::None,
                    };
                }
                Fill::Gradient(g) if g.steps > 0 || g.accel != 0.0 || g.blend != 0 || g.smooth => {
                    // as the plain fountain fill: smooth, even, straight between the colors
                    let mut g = g.clone();
                    g.steps = 0;
                    g.accel = 0.0;
                    g.blend = 0;
                    g.smooth = false;
                    x.style.fill = Fill::Gradient(g);
                }
                _ => {}
            }
            bitmaps |= matches!(x.kind, ObjKind::Image(_));
            hidden |= !x.visible;
        });
        self.notes.raster_effects |= raster;
        self.notes.fills |= fills;
        self.notes.bitmaps |= bitmaps;
        self.notes.hidden |= hidden;
        c
    }

    /// A shape written as one curve with its own fill and outline; `None` when it needs more
    /// than that (text, effects, arrowheads, outlines inside or outside the shape...).
    fn direct(&mut self, o: &Object) -> Option<Vec<Item>> {
        if !matches!(o.kind, ObjKind::Rect(_) | ObjKind::Ellipse(_) | ObjKind::Polygon(_) | ObjKind::Spiral(_) | ObjKind::Path(_) | ObjKind::Connector(_)) {
            return None;
        }
        let e = &o.effects;
        if e.block_shadow.is_some() || e.extrude.is_some() || e.contour.is_some() {
            return None;
        }
        let ol = &o.style.outline;
        let visible = ol.is_visible();
        if visible && (ol.profile.is_some() || ol.start_arrow.is_some() || ol.end_arrow.is_some() || (ol.position != OutlinePosition::Center && o.is_closed_shape())) {
            return None;
        }
        let path = o.world_path();
        if path.elements().is_empty() {
            return Some(vec![]);
        }
        let fill_ok = o.is_closed_shape() || matches!(o.kind, ObjKind::Path(_));
        let space = vector_render::Renderer::fill_space(o, &path);
        let file_path = self.to_file * path;
        let fill = if fill_ok { self.fill(&o.style.fill, space, &file_path) } else { None };
        let outline = visible.then(|| self.outline_of(ol));
        Some(self.curves(file_path, fill, outline, if visible { ol.width } else { 0.0 }))
    }

    fn outline_of(&mut self, ol: &Outline) -> u16 {
        let color = ol.color.unwrap_or(Color::BLACK);
        self.tables.outline(color, ol.width, ol.cap, ol.join, &ol.dash, ol.behind_fill, ol.scale_with_object, (ol.stretch, ol.nib_angle))
    }

    /// The primitives an object draws, as curves; clipped primitives are cut to their clip.
    fn prims(&mut self, prims: Vec<Prim>) -> Vec<Item> {
        let mut out = vec![];
        let mut clips: Vec<BezPath> = vec![];
        for p in prims {
            match p {
                Prim::Fill { path, paint, even_odd } => {
                    let file_path = self.to_file * clip(path, &clips, even_odd);
                    let fill = match paint {
                        Paint::Solid(c) => Some(CmxFill::Solid(self.tables.color(c))),
                        Paint::Gradient(g, space) => self.fill(&Fill::Gradient(g), space, &file_path),
                    };
                    out.extend(self.curves(file_path, fill, None, 0.0));
                }
                Prim::Stroke { path, color, style } => {
                    if clips.is_empty() {
                        let outline = self.tables.outline(color, style.width, style.cap, style.join, &dash_multiples(&style), false, false, (100.0, 0.0));
                        out.extend(self.curves(self.to_file * path, None, Some(outline), style.width));
                    } else {
                        // a clipped outline: the part of its area inside the clip
                        let area = geom::stroke_to_path(&path, style.width, join(style.join), cap(style.cap), &dash_multiples(&style));
                        let fill = Some(CmxFill::Solid(self.tables.color(color)));
                        out.extend(self.curves(self.to_file * clip(area, &clips, false), fill, None, 0.0));
                    }
                }
                Prim::BeginClip { path } => clips.push(path),
                Prim::EndClip => {
                    clips.pop();
                }
                Prim::Image { .. } | Prim::Raster { .. } => self.notes.bitmaps = true,
                Prim::BeginGroup { opacity, .. } => self.notes.transparency |= opacity < 0.999,
                Prim::Text { .. } => self.notes.text = true,
                Prim::EndGroup | Prim::Link { .. } => {}
            }
        }
        out
    }

    /// One curve instruction for a path (file units), or several in a group when it has more
    /// points than one instruction holds; `width` is the outline's width in points.
    fn curves(&mut self, path: BezPath, fill: Option<CmxFill>, outline: Option<u16>, width: f64) -> Vec<Item> {
        if path.elements().len() < 2 || (fill.is_none() && outline.is_none()) {
            return vec![];
        }
        let pad = if outline.is_some() { width.max(0.216) * UNITS_PER_POINT / 2.0 } else { 0.0 };
        let mk = |p: BezPath| {
            let bbox = p.bounding_box().inflate(pad, pad);
            Item::Curve { path: p, fill: fill.clone(), outline, bbox }
        };
        if points(&path).0.len() <= MAX_POINTS {
            return vec![mk(path)];
        }
        // split at subpaths, and very long subpaths into pieces
        let mut parts = vec![];
        let mut cur = BezPath::new();
        let mut count = 0;
        for el in path.elements() {
            let n = match el {
                PathEl::CurveTo(..) | PathEl::QuadTo(..) => 3,
                _ => 1,
            };
            if count + n > MAX_POINTS - 1 && cur.elements().len() > 1 {
                let last = cur.elements().last().and_then(|e| e.end_point());
                parts.push(std::mem::take(&mut cur));
                count = 0;
                if let (false, Some(p)) = (matches!(el, PathEl::MoveTo(_)), last) {
                    cur.move_to(p);
                    count = 1;
                }
            }
            cur.push(*el);
            count += n;
        }
        parts.push(cur);
        let items: Vec<Item> = parts.into_iter().filter(|p| p.elements().len() > 1).map(mk).collect();
        let bbox = union(&items);
        vec![Item::Group { items, bbox }]
    }

    /// A fill for a path (`file_path`, file units) whose fill space (unit square to page
    /// points) is `space`.
    fn fill(&mut self, f: &Fill, space: Affine, file_path: &BezPath) -> Option<CmxFill> {
        match f {
            Fill::None => None,
            Fill::Solid(c) => Some(CmxFill::Solid(self.tables.color(*c))),
            Fill::Gradient(g) => Some(self.fountain(g, self.to_file * space, file_path.bounding_box())),
            Fill::Pattern(_) | Fill::Mesh(_) => f.main_color().map(|c| CmxFill::Solid(self.tables.color(c))),
        }
    }

    /// A fountain fill from a gradient: `space` takes its unit square to file units, `bounds`
    /// is the box of the filled shape. CorelDRAW spans a linear fill across the box in its
    /// direction, and a radial or square fill from the box's edge (`max(w, h) / 2` from its
    /// center) to its center, so the stops move to where the gradient puts its colors.
    fn fountain(&mut self, g: &Gradient, space: Affine, bounds: Rect) -> CmxFill {
        let stops = g.sorted_stops();
        let (w, h) = (bounds.width().max(1.0), bounds.height().max(1.0));
        let c = bounds.center();
        let start = space * g.start;
        let v = space * g.end - start;
        let len = v.hypot();
        let d = if len > 1e-9 { v / len } else { Vec2::new(1.0, 0.0) };
        let (kind, angle, center, ts): (u16, f64, Point, Vec<f64>) = match g.kind {
            GradientKind::Linear => {
                let half = ((w * d.x.abs() + h * d.y.abs()) / 2.0).max(1e-9);
                let a = (start - c).dot(d) + half;
                (0, d.y.atan2(d.x).to_degrees(), c, stops.iter().map(|s| (a + s.pos * len) / (2.0 * half)).collect())
            }
            // CorelDRAW's conical fill starts opposite its angle
            GradientKind::Conical => (2, (-d.y).atan2(-d.x).to_degrees(), start, stops.iter().map(|s| s.pos).collect()),
            GradientKind::Elliptical | GradientKind::Rectangular => {
                let reach = w.max(h) / 2.0;
                // from the edge (0) to the center (1)
                let ts = stops.iter().map(|s| 1.0 - s.pos * len / reach).collect();
                (if g.kind == GradientKind::Elliptical { 1 } else { 3 }, d.y.atan2(d.x).to_degrees(), start, ts)
            }
        };
        let mut placed: Vec<(f64, Color)> = ts.into_iter().zip(stops.iter().map(|s| s.color)).collect();
        placed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let stops = fit_stops(&placed).into_iter().map(|(t, col)| (self.tables.color(col), (t * 100.0).round().clamp(0.0, 100.0) as u16)).collect();
        let pct = |v: f64| (v * 100.0).round().clamp(-100.0, 100.0) as i32;
        let angle = if angle.abs() < 1e-9 { 0.0 } else { angle };
        CmxFill::Fountain { kind, angle, cx: pct((center.x - c.x) / w), cy: pct((center.y - c.y) / h), stops }
    }
}

/// Stops placed anywhere (sorted by position), cut to 0..1 with a stop at each end.
fn fit_stops(placed: &[(f64, Color)]) -> Vec<(f64, Color)> {
    let at = |t: f64| -> Color {
        if t <= placed[0].0 {
            return placed[0].1;
        }
        for w in placed.windows(2) {
            if t <= w[1].0 {
                let span = (w[1].0 - w[0].0).max(1e-9);
                return Color::lerp(w[0].1, w[1].1, ((t - w[0].0) / span).clamp(0.0, 1.0));
            }
        }
        placed[placed.len() - 1].1
    };
    let mut out = vec![(0.0, at(0.0))];
    // stops are written in whole percents: one that rounds onto an end is that end
    out.extend(placed.iter().filter(|(t, _)| *t >= 0.005 && *t < 0.995).copied());
    out.push((1.0, at(1.0)));
    out
}

fn push_group(out: &mut Vec<Item>, items: Vec<Item>) {
    if !items.is_empty() {
        let bbox = union(&items);
        out.push(Item::Group { items, bbox });
    }
}

fn clip(path: BezPath, clips: &[BezPath], even_odd: bool) -> BezPath {
    clips.iter().fold(path, |p, c| geom::boolean(&p, c, geom::BoolOp::Intersect, even_odd))
}

/// A stroke's dashes (points) in multiples of its width, as the flattener made them.
fn dash_multiples(s: &StrokeStyle) -> Vec<f64> {
    let w = s.width.max(0.5);
    s.dash.iter().map(|d| d / w).collect()
}

fn join(j: LineJoin) -> kurbo::Join {
    match j {
        LineJoin::Miter => kurbo::Join::Miter,
        LineJoin::Round => kurbo::Join::Round,
        LineJoin::Bevel => kurbo::Join::Bevel,
    }
}

fn cap(c: LineCap) -> kurbo::Cap {
    match c {
        LineCap::Butt => kurbo::Cap::Butt,
        LineCap::Round => kurbo::Cap::Round,
        LineCap::Square => kurbo::Cap::Square,
    }
}

// ------------------------------------------------------------------------------------------
// Curves as CMX points
// ------------------------------------------------------------------------------------------

/// Point types: the two high bits say what the point is (00 the start of a subpath, 01 a line's
/// end, 10 a curve's end, 11 a control point); 0x08 marks the first and the last node of a
/// closed subpath, 0x10 a smooth node, 0x20 a symmetrical one. CorelDRAW sets 0x04 on every
/// node but the last of a closed subpath.
const PT_MOVE: u8 = 0x00;
const PT_LINE: u8 = 0x40;
const PT_CURVE: u8 = 0x80;
const PT_CONTROL: u8 = 0xc0;
const PT_CLOSED: u8 = 0x08;
const PT_NODE: u8 = 0x04;
const PT_SMOOTH: u8 = 0x10;
const PT_SYMMETRIC: u8 = 0x20;

/// The points (file units, rounded) and point types of a path. Subpaths of a single point are
/// left out; a closed subpath ends on its first point, as CorelDRAW writes it.
fn points(path: &BezPath) -> (Vec<(i32, i32)>, Vec<u8>) {
    let mut pts: Vec<Point> = vec![];
    let mut kinds: Vec<u8> = vec![];
    // the subpath being read
    let mut sub: Vec<Point> = vec![];
    let mut sub_kinds: Vec<u8> = vec![];
    let mut last = Point::ZERO;
    let mut finish = |sub: &mut Vec<Point>, sub_kinds: &mut Vec<u8>, closed: bool| {
        if sub.len() >= 2 {
            if closed {
                let first = sub[0];
                if (*sub.last().unwrap() - first).hypot() > 0.5 {
                    sub.push(first);
                    sub_kinds.push(PT_LINE | PT_NODE);
                }
                let n = sub_kinds.len() - 1;
                sub_kinds[n] = (sub_kinds[n] & !PT_NODE) | PT_CLOSED;
                sub_kinds[0] |= PT_CLOSED;
            }
            pts.append(sub);
            kinds.append(sub_kinds);
        }
        sub.clear();
        sub_kinds.clear();
    };
    for el in path.elements() {
        if !matches!(el, PathEl::MoveTo(_) | PathEl::ClosePath) && sub.is_empty() {
            sub.push(last);
            sub_kinds.push(PT_MOVE | PT_NODE);
        }
        match *el {
            PathEl::MoveTo(p) => {
                finish(&mut sub, &mut sub_kinds, false);
                sub.push(p);
                sub_kinds.push(PT_MOVE | PT_NODE);
                last = p;
            }
            PathEl::LineTo(p) => {
                sub.push(p);
                sub_kinds.push(PT_LINE | PT_NODE);
                last = p;
            }
            PathEl::QuadTo(q, p) => {
                let c1 = last + (q - last) * (2.0 / 3.0);
                let c2 = p + (q - p) * (2.0 / 3.0);
                sub.extend([c1, c2, p]);
                sub_kinds.extend([PT_CONTROL, PT_CONTROL, PT_CURVE | PT_NODE]);
                last = p;
            }
            PathEl::CurveTo(c1, c2, p) => {
                sub.extend([c1, c2, p]);
                sub_kinds.extend([PT_CONTROL, PT_CONTROL, PT_CURVE | PT_NODE]);
                last = p;
            }
            PathEl::ClosePath => {
                if let Some(first) = sub.first().copied() {
                    finish(&mut sub, &mut sub_kinds, true);
                    last = first;
                }
            }
        }
    }
    finish(&mut sub, &mut sub_kinds, false);
    smooth_nodes(&pts, &mut kinds);
    let r = |v: f64| v.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32;
    (pts.iter().map(|p| (r(p.x), r(p.y))).collect(), kinds)
}

/// Mark the nodes between two curves whose handles line up as smooth, or symmetrical when the
/// handles are as long as each other, as CorelDRAW does.
fn smooth_nodes(pts: &[Point], kinds: &mut [u8]) {
    for i in 1..pts.len().saturating_sub(1) {
        let k = kinds[i];
        if k & 0xc0 != PT_CURVE || kinds[i - 1] & 0xc0 != PT_CONTROL || kinds[i + 1] & 0xc0 != PT_CONTROL {
            continue;
        }
        let (a, b) = (pts[i] - pts[i - 1], pts[i + 1] - pts[i]);
        let (la, lb) = (a.hypot(), b.hypot());
        if la < 1.0 || lb < 1.0 {
            continue;
        }
        let cross = (a.x * b.y - a.y * b.x) / (la * lb);
        if cross.abs() < 1e-3 && a.dot(b) > 0.0 {
            kinds[i] |= if (la - lb).abs() <= 0.01 * la.max(lb) { PT_SYMMETRIC } else { PT_SMOOTH };
        }
    }
}

// ------------------------------------------------------------------------------------------
// Writing the file
// ------------------------------------------------------------------------------------------

const BEGIN_PAGE: i16 = 9;
const END_PAGE: i16 = 10;
const BEGIN_LAYER: i16 = 11;
const END_LAYER: i16 = 12;
const BEGIN_GROUP: i16 = 13;
const END_GROUP: i16 = 14;
const POLYCURVE: i16 = 67;
const TAG_END: u8 = 0xff;

/// Little-endian bytes with tagged fields and size patches.
#[derive(Default)]
struct W {
    b: Vec<u8>,
}

impl W {
    fn u8(&mut self, v: u8) {
        self.b.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.b.extend(v.to_le_bytes());
    }
    fn i16(&mut self, v: i16) {
        self.b.extend(v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.b.extend(v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.b.extend(v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.b.extend(v.to_le_bytes());
    }
    fn put_u32(&mut self, at: usize, v: u32) {
        self.b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn pos(&self) -> usize {
        self.b.len()
    }
    /// A fixed-size text field, padded with zeros.
    fn text(&mut self, s: &str, n: usize) {
        let mut v = s.as_bytes().to_vec();
        v.resize(n, 0);
        self.b.extend(&v[..n]);
    }
    /// Windows code page 1252 characters after their 16-bit count.
    fn string(&mut self, s: &str) {
        let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(s);
        let bytes = &bytes[..bytes.len().min(255)];
        self.u16(bytes.len() as u16);
        self.b.extend_from_slice(bytes);
    }
    /// UTF-16 characters after their 32-bit count.
    fn wide_string(&mut self, s: &str) {
        let units: Vec<u16> = s.encode_utf16().take(255).collect();
        self.u32(units.len() as u32);
        for u in units {
            self.u16(u);
        }
    }
    /// A box: left, top, right, bottom (y up).
    fn bbox(&mut self, r: Rect) {
        for v in [r.x0, r.y1, r.x1, r.y0] {
            self.i32(v.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32);
        }
    }
    /// A tagged field: its number, its size (these 3 bytes included), then `f`'s bytes.
    fn tag(&mut self, id: u8, f: impl FnOnce(&mut W)) {
        let at = self.pos();
        self.u8(id);
        self.u16(0);
        f(self);
        let n = (self.pos() - at).min(u16::MAX as usize) as u16;
        self.b[at + 1..at + 3].copy_from_slice(&n.to_le_bytes());
    }
    /// The identity matrix (matrix type 1).
    fn identity(&mut self) {
        self.u16(1);
    }
    /// Start a RIFF chunk; returns where its size goes.
    fn chunk(&mut self, tag: &[u8; 4]) -> usize {
        self.b.extend_from_slice(tag);
        self.u32(0);
        self.pos() - 4
    }
    fn end_chunk(&mut self, at: usize) {
        let n = self.pos() - at - 4;
        self.put_u32(at, n as u32);
        if n % 2 == 1 {
            self.b.push(0);
        }
    }
    /// A table of tagged records: their count, then each record's fields and an end tag.
    fn records(&mut self, n: usize, mut f: impl FnMut(&mut W, usize)) {
        self.u16(n as u16);
        for i in 0..n {
            f(self, i);
            self.u8(TAG_END);
        }
    }
    /// An instruction: its size, its number and `f`'s bytes; returns where the bytes start.
    /// Sizes past 32767 bytes take the long form (-1, then a 32-bit size). CorelDRAW stops
    /// reading a page at the first instruction of odd size, so every one is padded to even.
    fn instr(&mut self, code: i16, f: impl FnOnce(&mut W)) -> usize {
        let mut body = W::default();
        f(&mut body);
        if body.b.len() % 2 == 1 {
            body.u8(0);
        }
        if body.b.len() + 4 <= i16::MAX as usize {
            self.i16((body.b.len() + 4) as i16);
            self.i16(code);
        } else {
            self.i16(-1);
            self.i32((body.b.len() + 8) as i32);
            self.i16(code);
        }
        let at = self.pos();
        self.b.extend_from_slice(&body.b);
        at
    }
}

fn write_file(pages: &[PageItems], t: &Tables) -> Vec<u8> {
    let mut w = W::default();
    w.b.extend_from_slice(b"RIFF");
    w.u32(0);
    w.b.extend_from_slice(b"CMX1");
    let page_box = |p: &PageItems| {
        let (hw, hh) = (p.width / 2.0 * UNITS_PER_POINT, p.height / 2.0 * UNITS_PER_POINT);
        Rect::new(-hw, -hh, hw, hh)
    };
    // the header
    let cont = w.chunk(b"cont");
    w.text("{Corel Binary Meta File}", 32);
    w.text("Windows 3.1", 16);
    w.text("2", 4); // little-endian
    w.text("4", 2); // 32-bit coordinates
    w.text("2", 4); // version 2.0
    w.text("0", 4);
    w.u16(35); // units: metres...
    w.f64(1e-7); // ...times 1e-7: a 254000th of an inch
    w.u32(0); // options
    w.u32(0); // foreign key
    w.u32(0); // capability
    let index_at = w.pos();
    w.u32(0);
    let info_at = w.pos();
    w.u32(0);
    w.u32(u32::MAX); // no thumbnail
    w.bbox(page_box(&pages[0]));
    let tally_at = w.pos();
    w.u32(0);
    w.b.extend([0u8; 64]);
    w.end_chunk(cont);

    // the pages, then their layer lists
    let mut page_at = vec![];
    let mut layer_at: Vec<Vec<usize>> = vec![];
    let mut tally = 0u32;
    for (n, p) in pages.iter().enumerate() {
        page_at.push(w.pos());
        let (count, layers) = write_page(&mut w, p, n as u16 + 1, page_box(p));
        tally += count;
        layer_at.push(layers);
    }
    let mut rlst_at = vec![];
    for p in pages {
        rlst_at.push(w.pos());
        let c = w.chunk(b"rlst");
        w.u16(p.layers.len() as u16);
        for l in 1..=p.layers.len() {
            w.u16(2);
            w.u16(9);
            w.u16(l as u16);
        }
        w.end_chunk(c);
    }

    // the tables: (table number, offset) for the master index
    let mut sections: Vec<(u16, usize)> = vec![];
    sections.push((15, w.pos()));
    let c = w.chunk(b"rotl");
    w.records(t.outlines.len(), |w, i| {
        w.tag(1, |w| {
            for v in t.outlines[i] {
                w.u16(v);
            }
        })
    });
    w.end_chunk(c);
    sections.push((16, w.pos()));
    let c = w.chunk(b"rott");
    let styles = if t.line_styles.is_empty() { vec![(0x02, 0)] } else { t.line_styles.clone() };
    w.records(styles.len(), |w, i| {
        w.tag(1, |w| {
            w.u8(styles[i].0);
            w.u8(styles[i].1);
        })
    });
    w.end_chunk(c);
    sections.push((19, w.pos()));
    let c = w.chunk(b"rpen");
    let pens = if t.pens.is_empty() { vec![(3528, 100, 0)] } else { t.pens.clone() };
    w.records(pens.len(), |w, i| {
        w.tag(1, |w| {
            let (width, aspect, angle) = pens[i];
            w.i32(width);
            w.u16(aspect);
            w.i32(angle);
            w.identity();
        })
    });
    w.end_chunk(c);
    sections.push((20, w.pos()));
    let c = w.chunk(b"rdot");
    w.records(t.dashes.len(), |w, i| {
        w.tag(1, |w| {
            w.u16(t.dashes[i].len() as u16);
            for d in &t.dashes[i] {
                w.u16(*d);
            }
        })
    });
    w.end_chunk(c);
    // arrowheads: one record, no arrowhead
    sections.push((17, w.pos()));
    let c = w.chunk(b"rota");
    w.u16(1);
    w.u32(0);
    w.end_chunk(c);
    sections.push((21, w.pos()));
    let c = w.chunk(b"rclr");
    let colors = if t.colors.is_empty() { vec![ColorRec::Rgb([0, 0, 0])] } else { t.colors.clone() };
    w.records(colors.len(), |w, i| {
        // color model (5 RGB, 2 CMYK) and palette, then the value
        let (model, value): (u8, Vec<u8>) = match colors[i] {
            ColorRec::Rgb(v) => (5, v.to_vec()),
            ColorRec::Cmyk(v) => (2, v.to_vec()),
        };
        w.tag(1, |w| {
            w.u8(model);
            w.u8(5);
        });
        w.tag(2, |w| w.b.extend(value));
    });
    w.end_chunk(c);
    // screens: the default halftone, 60 lines per inch at 45 degrees
    sections.push((18, w.pos()));
    let c = w.chunk(b"rscr");
    w.records(1, |w, _| {
        w.tag(1, |w| {
            w.u16(0);
            w.u16(60);
            w.u32(0);
            w.i32(45_000_000);
            w.u8(0);
        })
    });
    w.end_chunk(c);

    // the index
    let list = w.chunk(b"LIST");
    w.b.extend_from_slice(b"indx");
    let mut ixlr_at = vec![];
    for (n, p) in pages.iter().enumerate() {
        ixlr_at.push(w.pos());
        let c = w.chunk(b"ixlr");
        w.u16(p.layers.len() as u16);
        w.u16(n as u16 + 1);
        for (l, at) in p.layers.iter().zip(&layer_at[n]) {
            // a record's size leaves out its last 2 bytes, as CorelDRAW counts it
            let mut r = W::default();
            r.u32(*at as u32);
            r.string(&l.name);
            r.u32(u32::MAX);
            w.u16((r.b.len() - 2) as u16);
            w.b.extend(r.b);
        }
        w.end_chunk(c);
    }
    sections.push((3, w.pos()));
    let c = w.chunk(b"ixtl");
    w.u16(pages.len() as u16);
    w.u16(4); // record size
    w.u16(3); // type: layer indexes
    for at in &ixlr_at {
        w.u32(*at as u32);
    }
    w.end_chunk(c);
    sections.push((2, w.pos()));
    let c = w.chunk(b"ixpg");
    w.u16(pages.len() as u16);
    for n in 0..pages.len() {
        // page, layer index, thumbnail (none), layer list
        w.u16(16);
        w.u32(page_at[n] as u32);
        w.u32(ixlr_at[n] as u32);
        w.u32(u32::MAX);
        w.u32(rlst_at[n] as u32);
    }
    w.end_chunk(c);
    let ixmr = w.pos();
    sections.push((1, ixmr));
    sections.sort_by_key(|s| s.0);
    let c = w.chunk(b"ixmr");
    w.u16(1);
    w.u16(0x19);
    w.u16(sections.len() as u16);
    for (id, off) in &sections {
        w.u16(*id);
        w.u32(*off as u32);
    }
    w.end_chunk(c);
    w.end_chunk(list);

    // keywords and comment, empty
    let info = w.pos();
    let list = w.chunk(b"LIST");
    w.b.extend_from_slice(b"INFO");
    for tag in [b"IKEY", b"ICMT"] {
        let c = w.chunk(tag);
        w.b.extend([0u8; 32]);
        w.end_chunk(c);
    }
    w.end_chunk(list);

    w.put_u32(index_at, ixmr as u32);
    w.put_u32(info_at, info as u32);
    w.put_u32(tally_at, tally);
    let n = (w.pos() - 8) as u32;
    w.put_u32(4, n);
    w.b
}

/// Write a page chunk; returns its number of instructions and where its layers begin.
fn write_page(w: &mut W, p: &PageItems, number: u16, page_box: Rect) -> (u32, Vec<usize>) {
    let chunk = w.chunk(b"page");
    let (mut end_at, mut tally_at) = (0, 0);
    let body = w.instr(BEGIN_PAGE, |b| {
        b.tag(1, |b| {
            b.u16(number);
            b.u32(0); // flags
            b.bbox(page_box);
            end_at = b.pos();
            b.u32(0);
            b.u16(p.layers.len() as u16);
            tally_at = b.pos();
            b.u32(0);
        });
        b.tag(2, |b| b.identity());
        b.tag(3, |b| b.u8(0));
        // the page's size
        b.tag(4, |b| {
            b.u32((p.width * UNITS_PER_POINT).round() as u32);
            b.u32((p.height * UNITS_PER_POINT).round() as u32);
        });
        b.u8(TAG_END);
    });
    let mut tally = 1u32;
    let mut layers = vec![];
    for (n, l) in p.layers.iter().enumerate() {
        layers.push(w.pos());
        tally += write_layer(w, l, number, n as u16 + 1);
    }
    let end = w.pos();
    w.instr(END_PAGE, |_| {});
    tally += 1;
    w.put_u32(body + end_at, end as u32);
    w.put_u32(body + tally_at, tally);
    w.end_chunk(chunk);
    (tally, layers)
}

fn write_layer(w: &mut W, l: &LayerItems, page: u16, number: u16) -> u32 {
    let mut tally_at = 0;
    let body = w.instr(BEGIN_LAYER, |b| {
        b.tag(1, |b| {
            b.u16(page);
            b.u16(number);
            b.u32(0); // flags
            tally_at = b.pos();
            b.u32(0);
            b.string(&l.name);
        });
        b.tag(2, |b| b.identity());
        b.tag(3, |b| b.u8(0));
        b.tag(4, |b| b.wide_string(&l.name));
        b.u8(TAG_END);
    });
    let mut tally = 1;
    for it in &l.items {
        tally += write_item(w, it);
    }
    w.instr(END_LAYER, |_| {});
    tally += 1;
    w.put_u32(body + tally_at, tally);
    tally
}

/// Write an item; returns its number of instructions.
fn write_item(w: &mut W, it: &Item) -> u32 {
    match it {
        Item::Curve { path, fill, outline, bbox } => {
            let (pts, kinds) = points(path);
            if pts.len() < 2 {
                return 0;
            }
            w.instr(POLYCURVE, |b| {
                b.tag(1, |b| attributes(b, fill.as_ref(), *outline));
                b.tag(2, |b| {
                    b.u16(pts.len() as u16);
                    for (x, y) in &pts {
                        b.i32(*x);
                        b.i32(*y);
                    }
                    b.b.extend(&kinds);
                });
                b.tag(3, |b| b.bbox(*bbox));
                b.tag(4, |_| {});
                b.u8(TAG_END);
            });
            1
        }
        Item::Group { items, bbox } => {
            let (mut tally_at, mut end_at) = (0, 0);
            let groups = items.iter().filter(|i| matches!(i, Item::Group { .. })).count();
            let body = w.instr(BEGIN_GROUP, |b| {
                b.tag(1, |b| {
                    b.bbox(*bbox);
                    b.u16(groups as u16);
                    tally_at = b.pos();
                    b.u32(0);
                    end_at = b.pos();
                    b.u32(0);
                });
                b.u8(TAG_END);
            });
            let mut tally = 1;
            for i in items {
                tally += write_item(w, i);
            }
            w.instr(END_GROUP, |_| {});
            tally += 1;
            let end = w.pos();
            w.put_u32(body + tally_at, tally);
            w.put_u32(body + end_at, end as u32);
            tally
        }
    }
}

/// Rendering attributes: which parts follow (0x01 fill, 0x02 outline), then each part's
/// tagged fields.
fn attributes(b: &mut W, fill: Option<&CmxFill>, outline: Option<u16>) {
    b.u8(fill.map_or(0, |_| 0x01) | outline.map_or(0, |_| 0x02));
    if let Some(f) = fill {
        b.tag(1, |b| match f {
            CmxFill::Solid(color) => {
                b.u16(1);
                b.tag(1, |b| {
                    b.u16(*color);
                    b.u16(1); // screen
                });
                b.u8(TAG_END);
            }
            CmxFill::Fountain { kind, angle, cx, cy, stops } => {
                b.u16(2);
                b.tag(1, |b| {
                    b.u16(*kind);
                    b.u16(1); // screen
                    b.u16(0); // edge pad
                    b.i32((angle * 1e6).round() as i32);
                    b.i32(*cx);
                    b.i32(*cy);
                    b.u16(0); // steps: as many as needed
                    // two colors blend straight; more need the custom mode, or CorelDRAW
                    // keeps only the first and the last
                    b.u16(if stops.len() > 2 { 3 } else { 0 });
                    b.u16(0); // rate method
                    b.u16(50); // midpoint
                });
                b.tag(2, |b| {
                    b.u16(stops.len() as u16);
                    for (color, at) in stops {
                        b.u16(*color);
                        b.u16(*at);
                    }
                });
                b.u8(TAG_END);
            }
        });
        b.u8(TAG_END);
    }
    if let Some(o) = outline {
        b.tag(1, |b| b.u16(o));
        b.u8(TAG_END);
    }
}

#[cfg(test)]
#[path = "cmx_export_tests.rs"]
mod tests;
