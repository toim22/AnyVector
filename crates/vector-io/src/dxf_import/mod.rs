//! DXF import (AutoCAD Drawing Interchange Format, ASCII, R12 to 2018).
//!
//! The model space of the drawing comes in at its real size, in the units the file says
//! (`$INSUNITS`, or inches/millimeters by `$MEASUREMENT`), with the page fitted around it. Every
//! DXF layer becomes a layer (hidden when it is off). Lines, polylines (with their arcs), circles,
//! arcs, ellipses, splines, solids, solid hatches, leaders, text, multiline text, block inserts
//! (with their attributes and arrays) and dimensions are read; colors come from the AutoCAD
//! Color Index or the true color, line weights from the entity or its layer. What is left out
//! (paper space, 3D solids, pattern hatches, images...) is listed in the import notes.

use crate::dxf::aci_rgb;
use crate::{IoError, Result};
use kurbo::{Affine, Arc, BezPath, PathEl, Point, Rect, Shape, SvgArc, Vec2};
use std::collections::{BTreeMap, HashMap};
use vector_core::color::Color;
use vector_core::document::{Document, Layer};
use vector_core::object::*;
use vector_core::path::NodePath;
use vector_core::style::*;
use vector_core::text::{self, CharFormat, TextObj};

/// One group code and its value.
type Pair = (i32, String);

/// An entity: its type and its group codes, in order.
#[derive(Clone, Debug)]
struct Ent {
    kind: String,
    codes: Vec<Pair>,
    /// The VERTEX entities of a POLYLINE, the ATTRIB entities of an INSERT.
    children: Vec<Ent>,
}

impl Ent {
    fn get(&self, code: i32) -> Option<&str> {
        self.codes.iter().find(|(c, _)| *c == code).map(|(_, v)| v.as_str())
    }
    fn f(&self, code: i32) -> Option<f64> {
        self.get(code).and_then(|v| v.trim().parse().ok())
    }
    fn f_or(&self, code: i32, d: f64) -> f64 {
        self.f(code).unwrap_or(d)
    }
    fn i(&self, code: i32) -> Option<i64> {
        self.get(code).and_then(|v| v.trim().parse::<f64>().ok()).map(|v| v as i64)
    }
    fn pt(&self, code: i32) -> Option<Point> {
        Some(Point::new(self.f(code)?, self.f(code + 10).unwrap_or(0.0)))
    }
    fn all(&self, code: i32) -> Vec<f64> {
        self.codes.iter().filter(|(c, _)| *c == code).filter_map(|(_, v)| v.trim().parse().ok()).collect()
    }
}

/// The DXF group code pairs of a file.
fn pairs(text: &str) -> Vec<Pair> {
    let mut lines = text.lines();
    let mut out = vec![];
    while let (Some(c), Some(v)) = (lines.next(), lines.next()) {
        let Ok(code) = c.trim().parse::<i32>() else { break };
        out.push((code, v.trim_end_matches('\r').to_string()));
    }
    out
}

/// Split pairs (from just after a SECTION's name to its ENDSEC) into entities, gathering the
/// VERTEX entities of POLYLINEs and the ATTRIBs of INSERTs.
fn entities(p: &[Pair]) -> Vec<Ent> {
    let mut flat: Vec<Ent> = vec![];
    for (c, v) in p {
        if *c == 0 {
            flat.push(Ent { kind: v.trim().to_uppercase(), codes: vec![], children: vec![] });
        } else if let Some(e) = flat.last_mut() {
            e.codes.push((*c, v.clone()));
        }
    }
    let mut out: Vec<Ent> = vec![];
    let mut open = false;
    for e in flat {
        match e.kind.as_str() {
            "VERTEX" | "ATTRIB" if open => out.last_mut().unwrap().children.push(e),
            "SEQEND" => open = false,
            _ => {
                open = e.kind == "POLYLINE" || (e.kind == "INSERT" && e.i(66) == Some(1));
                out.push(e);
            }
        }
    }
    out
}

/// The sections of a file: name -> pairs inside.
fn sections(p: &[Pair]) -> HashMap<String, Vec<Pair>> {
    let mut out = HashMap::new();
    let mut i = 0;
    while i < p.len() {
        if p[i] == (0, "SECTION".into()) && i + 1 < p.len() && p[i + 1].0 == 2 {
            let name = p[i + 1].1.trim().to_uppercase();
            let start = i + 2;
            let mut j = start;
            while j < p.len() && !(p[j].0 == 0 && p[j].1.trim() == "ENDSEC") {
                j += 1;
            }
            out.insert(name, p[start..j].to_vec());
            i = j;
        }
        i += 1;
    }
    out
}

#[derive(Clone, Debug)]
struct LayerDef {
    name: String,
    color: Color,
    on: bool,
    /// Line weight, hundredths of a millimeter (negative: default).
    weight: i64,
}

struct Block {
    base: Point,
    ents: Vec<Ent>,
}

/// What an entity inherits from the block insert it is drawn through.
#[derive(Clone)]
struct Ctx {
    xf: Affine,
    /// The layer of the insert (for entities on layer 0).
    layer: Option<String>,
    color: Color,
    weight: i64,
    depth: u32,
}

/// One shape, in drawing units (y up), before it is placed on the page.
enum Shape2 {
    Path { path: BezPath, fill: bool, even_odd: bool },
    Text { text: TextObj, xf: Affine },
}

struct Item {
    layer: String,
    color: Color,
    weight_mm: f64,
    shape: Shape2,
}

struct Reader {
    layers: Vec<LayerDef>,
    blocks: HashMap<String, Block>,
    /// Text style name -> font family.
    styles: HashMap<String, String>,
    items: Vec<Item>,
    skipped: BTreeMap<String, usize>,
    paper_space: usize,
}

const DEFAULT_WEIGHT_MM: f64 = 0.25;

fn aci(i: i64) -> Color {
    // white (7) is drawn black on paper, as CAD programs print it
    if i == 7 || i <= 0 || i > 255 {
        return Color::BLACK;
    }
    let [r, g, b] = aci_rgb(i as u8);
    Color::rgb(r, g, b)
}

fn true_color(v: i64) -> Color {
    Color::rgb(((v >> 16) & 255) as u8, ((v >> 8) & 255) as u8, (v & 255) as u8)
}

/// Points per drawing unit, from `$INSUNITS` (or `$MEASUREMENT` when it is unitless); None
/// when the file doesn't say.
fn unit_points(insunits: Option<i64>, measurement: Option<i64>) -> Option<f64> {
    let v = match insunits {
        Some(1) => 72.0,
        Some(2) => 864.0,
        Some(3) => 72.0 * 63360.0,
        Some(4) => 72.0 / 25.4,
        Some(5) => 720.0 / 25.4,
        Some(6) => 72000.0 / 25.4,
        Some(7) => 72_000_000.0 / 25.4,
        Some(8) => 72.0e-6,
        Some(9) => 0.072,
        Some(10) => 2592.0,
        Some(13) => 72.0e-3 / 25.4,
        Some(14) => 7.2 / 25.4,
        _ => match measurement {
            Some(0) => 72.0,
            Some(1) => 72.0 / 25.4,
            _ => return None,
        },
    };
    Some(v)
}

impl Reader {
    fn layer(&self, name: &str) -> Option<&LayerDef> {
        self.layers.iter().find(|l| l.name.eq_ignore_ascii_case(name))
    }

    fn note_skip(&mut self, kind: &str) {
        *self.skipped.entry(kind.to_string()).or_default() += 1;
    }

    /// Resolve the layer, color and line weight of an entity.
    fn style_of(&mut self, e: &Ent, ctx: &Ctx) -> (String, Color, f64) {
        let mut layer = e.get(8).unwrap_or("0").trim().to_string();
        if layer == "0" {
            if let Some(l) = &ctx.layer {
                layer = l.clone();
            }
        }
        if self.layer(&layer).is_none() {
            self.layers.push(LayerDef { name: layer.clone(), color: Color::BLACK, on: true, weight: -3 });
        }
        let ld = self.layer(&layer).cloned().unwrap();
        let color = if let Some(tc) = e.i(420) {
            true_color(tc)
        } else {
            match e.i(62).unwrap_or(256) {
                256 => ld.color,
                0 => ctx.color,
                i => aci(i.abs()),
            }
        };
        let w = match e.i(370).unwrap_or(-1) {
            -1 => ld.weight,
            -2 => ctx.weight,
            w => w,
        };
        let w = if w < 0 { DEFAULT_WEIGHT_MM } else { w as f64 / 100.0 };
        (layer, color, w)
    }

    fn push_path(&mut self, e: &Ent, ctx: &Ctx, path: BezPath, fill: bool, even_odd: bool) {
        if path.elements().is_empty() {
            return;
        }
        let (layer, color, weight_mm) = self.style_of(e, ctx);
        self.items.push(Item { layer, color, weight_mm, shape: Shape2::Path { path: ctx.xf * path, fill, even_odd } });
    }

    fn entity(&mut self, e: &Ent, ctx: &Ctx) {
        if e.i(67) == Some(1) {
            self.paper_space += 1;
            return;
        }
        // an object's own extrusion direction pointing down mirrors it (2D drawings made upside down)
        let ocs = matches!(e.kind.as_str(), "LWPOLYLINE" | "CIRCLE" | "ARC" | "TEXT" | "MTEXT" | "INSERT" | "SOLID" | "TRACE" | "HATCH" | "DIMENSION");
        let mirror = if ocs && e.f(230).is_some_and(|z| z < 0.0) { Affine::new([-1.0, 0.0, 0.0, 1.0, 0.0, 0.0]) } else { Affine::IDENTITY };
        let c = Ctx { xf: ctx.xf * mirror, ..ctx.clone() };
        let ctx = &c;
        match e.kind.as_str() {
            "LINE" => {
                let (Some(a), Some(b)) = (e.pt(10), e.pt(11)) else { return };
                let mut p = BezPath::new();
                p.move_to(a);
                p.line_to(b);
                self.push_path(e, ctx, p, false, false);
            }
            "LWPOLYLINE" => {
                // vertices: 10/20 pairs, each optionally followed by its bulge (42)
                let mut verts: Vec<(Point, f64)> = vec![];
                let mut x = None;
                for (code, v) in &e.codes {
                    let v: f64 = v.trim().parse().unwrap_or(0.0);
                    match code {
                        10 => x = Some(v),
                        20 => {
                            if let Some(x) = x.take() {
                                verts.push((Point::new(x, v), 0.0));
                            }
                        }
                        42 => {
                            if let Some(last) = verts.last_mut() {
                                last.1 = v;
                            }
                        }
                        _ => {}
                    }
                }
                let closed = e.i(70).unwrap_or(0) & 1 == 1;
                self.push_path(e, ctx, bulge_path(&verts, closed), false, false);
            }
            "POLYLINE" => {
                let flags = e.i(70).unwrap_or(0);
                if flags & (16 | 64) != 0 {
                    self.note_skip("POLYLINE mesh");
                    return;
                }
                let verts: Vec<(Point, f64)> = e
                    .children
                    .iter()
                    .filter(|v| v.i(70).unwrap_or(0) & 16 == 0)
                    .filter_map(|v| Some((v.pt(10)?, v.f_or(42, 0.0))))
                    .collect();
                self.push_path(e, ctx, bulge_path(&verts, flags & 1 == 1), false, false);
            }
            "CIRCLE" => {
                let (Some(c), Some(r)) = (e.pt(10), e.f(40)) else { return };
                self.push_path(e, ctx, kurbo::Circle::new(c, r.abs()).to_path(0.01), false, false);
            }
            "ARC" => {
                let (Some(c), Some(r)) = (e.pt(10), e.f(40)) else { return };
                let a0 = e.f_or(50, 0.0).to_radians();
                let a1 = e.f_or(51, 360.0).to_radians();
                let arc = Arc { center: c, radii: Vec2::new(r, r), start_angle: a0, sweep_angle: ccw_sweep(a0, a1), x_rotation: 0.0 };
                self.push_path(e, ctx, open_arc(&arc), false, false);
            }
            "ELLIPSE" => {
                let (Some(c), Some(m)) = (e.pt(10), e.pt(11)) else { return };
                let ratio = e.f_or(40, 1.0);
                let major = m.to_vec2().hypot();
                let (t0, t1) = (e.f_or(41, 0.0), e.f_or(42, std::f64::consts::TAU));
                let arc = Arc { center: c, radii: Vec2::new(major, major * ratio), start_angle: t0, sweep_angle: ccw_sweep(t0, t1), x_rotation: m.y.atan2(m.x) };
                let mut p = open_arc(&arc);
                if (arc.sweep_angle - std::f64::consts::TAU).abs() < 1e-9 {
                    p.close_path();
                }
                self.push_path(e, ctx, p, false, false);
            }
            "SPLINE" => {
                let p = spline_path(e);
                self.push_path(e, ctx, p, false, false);
            }
            "SOLID" | "TRACE" | "3DFACE" => {
                // corners in the order 1, 2, 4, 3 (a SOLID's third and fourth points are crossed)
                let pts: Vec<Point> = [10, 11, 12, 13].iter().filter_map(|c| e.pt(*c)).collect();
                if pts.len() < 3 {
                    return;
                }
                let order: Vec<Point> = if e.kind == "3DFACE" { pts.clone() } else if pts.len() == 4 { vec![pts[0], pts[1], pts[3], pts[2]] } else { pts.clone() };
                let mut p = BezPath::new();
                p.move_to(order[0]);
                for q in &order[1..] {
                    p.line_to(*q);
                }
                p.close_path();
                let fill = e.kind != "3DFACE";
                self.push_path(e, ctx, p, fill, false);
            }
            "LEADER" => {
                let pts: Vec<(Point, f64)> = e.all(10).iter().zip(e.all(20)).map(|(x, y)| (Point::new(*x, y), 0.0)).collect();
                self.push_path(e, ctx, bulge_path(&pts, false), false, false);
            }
            "HATCH" => match hatch_path(e) {
                Some(p) if e.i(70) == Some(1) => self.push_path(e, ctx, p, true, true),
                _ => self.note_skip("HATCH pattern"),
            },
            "TEXT" | "ATTRIB" => self.text(e, ctx, false),
            "MTEXT" => self.text(e, ctx, true),
            "INSERT" => self.insert(e, ctx),
            "DIMENSION" => {
                // a dimension is drawn by its (anonymous) block, already in place
                if let Some(name) = e.get(2) {
                    let (layer, color, w) = self.style_of(e, ctx);
                    let sub = Ctx { xf: ctx.xf, layer: Some(layer), color, weight: (w * 100.0) as i64, depth: ctx.depth + 1 };
                    self.block(name.trim(), &sub, Affine::IDENTITY);
                }
            }
            "POINT" | "ATTDEF" | "VIEWPORT" | "SEQEND" | "ENDBLK" | "BLOCK" => {}
            k => self.note_skip(k),
        }
    }

    fn block(&mut self, name: &str, ctx: &Ctx, place: Affine) {
        if ctx.depth > 16 {
            return;
        }
        let Some(b) = self.blocks.get(name) else { return };
        let ents = b.ents.clone();
        let xf = ctx.xf * place * Affine::translate(-b.base.to_vec2());
        let sub = Ctx { xf, ..ctx.clone() };
        for e in &ents {
            self.entity(e, &sub);
        }
    }

    fn insert(&mut self, e: &Ent, ctx: &Ctx) {
        let Some(name) = e.get(2).map(|n| n.trim().to_string()) else { return };
        let at = e.pt(10).unwrap_or(Point::ZERO);
        let (sx, sy) = (e.f_or(41, 1.0), e.f_or(42, 1.0));
        let rot = e.f_or(50, 0.0).to_radians();
        let (cols, rows) = (e.i(70).unwrap_or(1).clamp(1, 1000), e.i(71).unwrap_or(1).clamp(1, 1000));
        let (dx, dy) = (e.f_or(44, 0.0), e.f_or(45, 0.0));
        let (layer, color, w) = self.style_of(e, ctx);
        let sub = Ctx { xf: ctx.xf, layer: Some(layer), color, weight: (w * 100.0) as i64, depth: ctx.depth + 1 };
        for r in 0..rows {
            for c in 0..cols {
                let off = Affine::rotate(rot) * Point::new(c as f64 * dx, r as f64 * dy);
                let place = Affine::translate(at.to_vec2() + off.to_vec2()) * Affine::rotate(rot) * Affine::scale_non_uniform(sx, sy);
                self.block(&name, &sub, place);
            }
        }
        // attributes are placed in the drawing already
        for a in &e.children {
            self.entity(a, ctx);
        }
    }

    fn text(&mut self, e: &Ent, ctx: &Ctx, mtext: bool) {
        let raw = if mtext {
            let mut s: String = e.codes.iter().filter(|(c, _)| *c == 3).map(|(_, v)| v.as_str()).collect();
            s.push_str(e.get(1).unwrap_or(""));
            mtext_plain(&s)
        } else {
            special_chars(e.get(1).unwrap_or(""))
        };
        if raw.trim().is_empty() {
            return;
        }
        let height = e.f_or(40, 1.0).abs().max(1e-6);
        let font = e.get(7).and_then(|s| self.styles.get(&s.trim().to_lowercase())).cloned().unwrap_or_else(|| "Arial".into());
        // DXF heights are cap heights; a font's size is about 1.4 times its capitals
        let fmt = CharFormat { font, size: height / 0.7, ..Default::default() };
        let t = TextObj::artistic(&raw, fmt);
        let lay = text::layout(&t);
        let b = lay.bounds;
        let first_ascent = lay.lines.first().map(|l| l.ascent).unwrap_or(height);
        // where the insertion point is on the text: (0..1 across, baseline/top/middle/bottom)
        let (at, rot, hx, vy) = if mtext {
            let at = e.pt(10).unwrap_or(Point::ZERO);
            let rot = match e.pt(11) {
                Some(d) if d.to_vec2().hypot() > 1e-12 => d.y.atan2(d.x),
                _ => e.f_or(50, 0.0).to_radians(),
            };
            let k = e.i(71).unwrap_or(1).clamp(1, 9) - 1;
            (at, rot, (k % 3) as f64 / 2.0, ["top", "middle", "bottom"][(k / 3) as usize])
        } else {
            let h = e.i(72).unwrap_or(0);
            let v = e.i(73).unwrap_or(0);
            let aligned = h != 0 || v != 0;
            let at = if aligned && !matches!(h, 3 | 5) { e.pt(11).or(e.pt(10)) } else { e.pt(10) }.unwrap_or(Point::ZERO);
            let hx = match h {
                1 | 4 => 0.5,
                2 => 1.0,
                _ => 0.0,
            };
            let vy = match (h, v) {
                (4, _) | (_, 2) => "middle",
                (_, 1) => "bottom",
                (_, 3) => "top",
                _ => "baseline",
            };
            (at, e.f_or(50, 0.0).to_radians(), hx, vy)
        };
        // in the text's own space (y down, origin on the first baseline), the point that goes on `at`
        let ax = b.x0 + b.width() * hx;
        let ay = match vy {
            "top" => -first_ascent,
            "middle" => (b.y0 + b.y1) / 2.0,
            "bottom" => b.y1,
            _ => 0.0,
        };
        // text space is y down; the drawing is y up
        let xf = ctx.xf * Affine::translate(at.to_vec2()) * Affine::rotate(rot) * Affine::scale_non_uniform(1.0, -1.0) * Affine::translate((-ax, -ay));
        let (layer, color, weight_mm) = self.style_of(e, ctx);
        self.items.push(Item { layer, color, weight_mm, shape: Shape2::Text { text: t, xf } });
    }
}

/// The counterclockwise sweep from `a0` to `a1` (radians), in (0, 2π].
fn ccw_sweep(a0: f64, a1: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    let s = (a1 - a0).rem_euclid(tau);
    if s < 1e-12 {
        tau
    } else {
        s
    }
}

fn open_arc(a: &Arc) -> BezPath {
    let mut p = BezPath::new();
    let start = a.center + (Affine::rotate(a.x_rotation) * Point::new(a.radii.x * a.start_angle.cos(), a.radii.y * a.start_angle.sin())).to_vec2();
    p.move_to(start);
    a.to_cubic_beziers(0.001, |p1, p2, p3| p.curve_to(p1, p2, p3));
    p
}

/// A polyline through vertices; a vertex's bulge makes the segment after it an arc (the tangent
/// of a quarter of the arc's angle, positive counterclockwise).
fn bulge_path(v: &[(Point, f64)], closed: bool) -> BezPath {
    let mut p = BezPath::new();
    if v.is_empty() {
        return p;
    }
    p.move_to(v[0].0);
    let n = v.len();
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let (a, b) = (v[i].0, v[(i + 1) % n].0);
        let bulge = v[i].1;
        if bulge.abs() < 1e-12 || (a - b).hypot() < 1e-12 {
            if !(closed && i == n - 1) {
                p.line_to(b);
            }
            continue;
        }
        let d = (b - a).hypot();
        let r = d * (1.0 + bulge * bulge) / (4.0 * bulge.abs());
        let arc = SvgArc { from: a, to: b, radii: Vec2::new(r, r), x_rotation: 0.0, large_arc: bulge.abs() > 1.0, sweep: bulge > 0.0 };
        match Arc::from_svg_arc(&arc) {
            Some(a) => a.to_cubic_beziers(0.001, |p1, p2, p3| p.curve_to(p1, p2, p3)),
            None => p.line_to(b),
        }
    }
    if closed {
        p.close_path();
    }
    p
}

/// A SPLINE: its control points as Bézier pieces (by knot insertion), or a curve through its
/// fit points when it has none.
fn spline_path(e: &Ent) -> BezPath {
    let deg = e.i(71).unwrap_or(3).clamp(1, 10) as usize;
    let knots = e.all(40);
    let cps: Vec<Point> = e.all(10).iter().zip(e.all(20)).map(|(x, y)| Point::new(*x, y)).collect();
    let weights = e.all(41);
    let closed = e.i(70).unwrap_or(0) & 1 == 1;
    if cps.len() < 2 {
        let fit: Vec<Point> = e.all(11).iter().zip(e.all(21)).map(|(x, y)| Point::new(*x, y)).collect();
        return through_points(&fit, closed);
    }
    let rational = weights.len() == cps.len() && weights.iter().any(|w| (w - 1.0).abs() > 1e-9);
    let mut p = BezPath::new();
    if knots.len() != cps.len() + deg + 1 || rational || deg > 3 || !is_clamped(deg, &knots) {
        // sample the curve
        let n = (cps.len() * 16).clamp(16, 4000);
        let k = if knots.len() == cps.len() + deg + 1 { knots.clone() } else { clamped_knots(cps.len(), deg) };
        let (t0, t1) = (k[deg], k[k.len() - 1 - deg]);
        for i in 0..=n {
            let t = t0 + (t1 - t0) * i as f64 / n as f64;
            let q = de_boor(deg, &k, &cps, if rational { Some(&weights) } else { None }, t);
            if i == 0 {
                p.move_to(q);
            } else {
                p.line_to(q);
            }
        }
    } else {
        for seg in bezier_pieces(deg, &knots, &cps) {
            if p.elements().is_empty() {
                p.move_to(seg[0]);
            }
            match deg {
                1 => p.line_to(seg[1]),
                2 => p.quad_to(seg[1], seg[2]),
                _ => p.curve_to(seg[1], seg[2], seg[3]),
            }
        }
    }
    if closed {
        p.close_path();
    }
    p
}

fn clamped_knots(n: usize, deg: usize) -> Vec<f64> {
    let mut k = vec![0.0; deg + 1];
    let inner = n.saturating_sub(deg + 1);
    for i in 1..=inner {
        k.push(i as f64 / (inner + 1) as f64);
    }
    k.extend(std::iter::repeat(1.0).take(deg + 1));
    k
}

fn de_boor(deg: usize, k: &[f64], cps: &[Point], w: Option<&Vec<f64>>, t: f64) -> Point {
    let n = cps.len();
    let mut s = deg;
    while s < n - 1 && t >= k[s + 1] {
        s += 1;
    }
    // homogeneous coordinates for rational curves
    let mut d: Vec<(f64, f64, f64)> = (0..=deg)
        .map(|j| {
            let i = (j + s - deg).min(n - 1);
            let wt = w.map(|w| w[i]).unwrap_or(1.0);
            (cps[i].x * wt, cps[i].y * wt, wt)
        })
        .collect();
    for r in 1..=deg {
        for j in (r..=deg).rev() {
            let i = j + s - deg;
            let den = k[i + deg + 1 - r] - k[i];
            let a = if den.abs() < 1e-14 { 0.0 } else { (t - k[i]) / den };
            d[j] = ((1.0 - a) * d[j - 1].0 + a * d[j].0, (1.0 - a) * d[j - 1].1 + a * d[j].1, (1.0 - a) * d[j - 1].2 + a * d[j].2);
        }
    }
    let (x, y, wt) = d[deg];
    Point::new(x / wt, y / wt)
}

/// The Bézier pieces of a clamped, non-rational B-spline (degree 1 to 3): insert every inner
/// knot until it has full multiplicity; then every `deg` control points make one piece.
fn bezier_pieces(deg: usize, knots: &[f64], cps: &[Point]) -> Vec<Vec<Point>> {
    let mut k = knots.to_vec();
    let mut c = cps.to_vec();
    let (t0, t1) = (k[deg], k[k.len() - 1 - deg]);
    let mut inner: Vec<f64> = k.iter().copied().filter(|x| *x > t0 + 1e-12 && *x < t1 - 1e-12).collect();
    inner.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    for t in inner {
        while k.iter().filter(|x| (**x - t).abs() < 1e-12).count() < deg {
            insert_knot(deg, &mut k, &mut c, t);
        }
    }
    (0..(c.len() - 1) / deg).map(|i| c[i * deg..=(i + 1) * deg].to_vec()).collect()
}

fn is_clamped(deg: usize, k: &[f64]) -> bool {
    let n = k.len();
    n >= 2 * (deg + 1) && k[..=deg].iter().all(|x| (x - k[0]).abs() < 1e-12) && k[n - 1 - deg..].iter().all(|x| (x - k[n - 1]).abs() < 1e-12)
}

/// Boehm's knot insertion.
fn insert_knot(deg: usize, k: &mut Vec<f64>, c: &mut Vec<Point>, t: f64) {
    let n = c.len();
    let mut s = deg;
    while s < n - 1 && t >= k[s + 1] {
        s += 1;
    }
    let mut out = Vec::with_capacity(n + 1);
    for i in 0..=n {
        if i + deg <= s {
            out.push(c[i]);
        } else if i > s {
            out.push(c[i - 1]);
        } else {
            let den = k[i + deg] - k[i];
            let a = if den.abs() < 1e-14 { 0.0 } else { (t - k[i]) / den };
            out.push(c[i - 1].lerp(c[i], a));
        }
    }
    k.insert(s + 1, t);
    *c = out;
}

/// A smooth curve through points (Catmull-Rom), for splines given by their fit points.
fn through_points(pts: &[Point], closed: bool) -> BezPath {
    let mut p = BezPath::new();
    let n = pts.len();
    if n < 2 {
        return p;
    }
    p.move_to(pts[0]);
    let get = |i: isize| -> Point {
        if closed {
            pts[i.rem_euclid(n as isize) as usize]
        } else {
            pts[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs as isize {
        let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
        p.curve_to(p1 + (p2 - p0) / 6.0, p2 - (p3 - p1) / 6.0, p2);
    }
    if closed {
        p.close_path();
    }
    p
}

/// The boundary of a HATCH: polyline loops and edge loops (lines, arcs, ellipses, splines).
fn hatch_path(e: &Ent) -> Option<BezPath> {
    let c = &e.codes;
    let mut i = c.iter().position(|(code, _)| *code == 91)?;
    let loops = c[i].1.trim().parse::<usize>().ok()?;
    i += 1;
    let num = |v: &str| v.trim().parse::<f64>().unwrap_or(0.0);
    let mut out = BezPath::new();
    let next = |code: i32, i: &mut usize| -> Option<f64> {
        while *i < c.len() && c[*i].0 != code {
            *i += 1;
        }
        let v = c.get(*i).map(|x| num(&x.1));
        *i += 1;
        v
    };
    for _ in 0..loops {
        let kind = next(92, &mut i)? as i64;
        if kind & 2 != 0 {
            let bulges = next(72, &mut i)? != 0.0;
            let closed = next(73, &mut i)? != 0.0;
            let n = next(93, &mut i)? as usize;
            let mut v = vec![];
            for _ in 0..n {
                let x = next(10, &mut i)?;
                let y = next(20, &mut i)?;
                let b = if bulges && c.get(i).is_some_and(|x| x.0 == 42) { next(42, &mut i)? } else { 0.0 };
                v.push((Point::new(x, y), b));
            }
            let _ = closed;
            out.extend(bulge_path(&v, true).elements().iter().copied());
        } else {
            let n = next(93, &mut i)? as usize;
            let mut lp = BezPath::new();
            for _ in 0..n {
                let et = next(72, &mut i)? as i64;
                let seg: BezPath = match et {
                    1 => {
                        let (x0, y0, x1, y1) = (next(10, &mut i)?, next(20, &mut i)?, next(11, &mut i)?, next(21, &mut i)?);
                        let mut s = BezPath::new();
                        s.move_to((x0, y0));
                        s.line_to((x1, y1));
                        s
                    }
                    2 => {
                        let (x, y, r, a0, a1, ccw) = (next(10, &mut i)?, next(20, &mut i)?, next(40, &mut i)?, next(50, &mut i)?, next(51, &mut i)?, next(73, &mut i)?);
                        let (a0, a1) = (a0.to_radians(), a1.to_radians());
                        let arc = if ccw != 0.0 {
                            Arc { center: Point::new(x, y), radii: Vec2::new(r, r), start_angle: a0, sweep_angle: ccw_sweep(a0, a1), x_rotation: 0.0 }
                        } else {
                            Arc { center: Point::new(x, y), radii: Vec2::new(r, r), start_angle: -a0, sweep_angle: -ccw_sweep(a0, a1), x_rotation: 0.0 }
                        };
                        open_arc(&arc)
                    }
                    3 => {
                        let (x, y, mx, my, ratio, a0, a1, ccw) = (next(10, &mut i)?, next(20, &mut i)?, next(11, &mut i)?, next(21, &mut i)?, next(40, &mut i)?, next(50, &mut i)?, next(51, &mut i)?, next(73, &mut i)?);
                        let major = Vec2::new(mx, my).hypot();
                        let (a0, a1) = (a0.to_radians(), a1.to_radians());
                        let (s, sw) = if ccw != 0.0 { (a0, ccw_sweep(a0, a1)) } else { (-a0, -ccw_sweep(a0, a1)) };
                        open_arc(&Arc { center: Point::new(x, y), radii: Vec2::new(major, major * ratio), start_angle: s, sweep_angle: sw, x_rotation: my.atan2(mx) })
                    }
                    4 => {
                        let deg = next(94, &mut i)?;
                        let _rational = next(73, &mut i)?;
                        let _periodic = next(74, &mut i)?;
                        let nk = next(95, &mut i)? as usize;
                        let nc = next(96, &mut i)? as usize;
                        let mut codes = vec![(71, (deg as i64).to_string())];
                        for _ in 0..nk {
                            codes.push((40, next(40, &mut i)?.to_string()));
                        }
                        for _ in 0..nc {
                            codes.push((10, next(10, &mut i)?.to_string()));
                            codes.push((20, next(20, &mut i)?.to_string()));
                        }
                        spline_path(&Ent { kind: "SPLINE".into(), codes, children: vec![] })
                    }
                    _ => return None,
                };
                // chain the edges into one loop
                for (k, el) in seg.elements().iter().enumerate() {
                    match el {
                        PathEl::MoveTo(q) if k == 0 => {
                            if lp.elements().is_empty() {
                                lp.move_to(*q);
                            } else {
                                lp.line_to(*q);
                            }
                        }
                        el => lp.push(*el),
                    }
                }
            }
            lp.close_path();
            out.extend(lp.elements().iter().copied());
        }
        // skip the loop's source-object references
        if let Some(n) = c.get(i).filter(|x| x.0 == 97).map(|x| num(&x.1) as usize) {
            i += 1 + n;
        }
    }
    Some(out)
}

/// TEXT special codes: %%d degree, %%p plus/minus, %%c diameter, %%% percent; %%u/%%o (under/
/// overline toggles) are dropped.
fn special_chars(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '%' && it.peek() == Some(&'%') {
            it.next();
            match it.next().map(|c| c.to_ascii_lowercase()) {
                Some('d') => out.push('°'),
                Some('p') => out.push('±'),
                Some('c') => out.push('Ø'),
                Some('%') => out.push('%'),
                Some('u') | Some('o') | Some('k') => {}
                Some(d) if d.is_ascii_digit() => {
                    let mut num = d.to_string();
                    while let Some(n) = it.peek().filter(|c| c.is_ascii_digit()) {
                        num.push(*n);
                        it.next();
                    }
                    if let Some(ch) = num.parse::<u32>().ok().and_then(char::from_u32) {
                        out.push(ch);
                    }
                }
                Some(o) => {
                    out.push_str("%%");
                    out.push(o);
                }
                None => out.push_str("%%"),
            }
        } else {
            out.push(c);
        }
    }
    unicode_escapes(&out)
}

/// `\U+XXXX` escapes.
fn unicode_escapes(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 6 < chars.len() + 1 && chars.get(i + 1).is_some_and(|c| c.eq_ignore_ascii_case(&'u')) && chars.get(i + 2) == Some(&'+') {
            let hex: String = chars[i + 3..(i + 7).min(chars.len())].iter().collect();
            if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                out.push(ch);
                i += 7;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// The plain text of MTEXT: paragraphs (\P) become new lines, formatting codes are dropped,
/// stacked fractions (\Sa^b;) read a/b.
fn mtext_plain(s: &str) -> String {
    let s = unicode_escapes(&special_chars(s));
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '{' | '}' => i += 1,
            '\\' if i + 1 < chars.len() => {
                let k = chars[i + 1];
                match k {
                    'P' | 'X' => {
                        out.push('\n');
                        i += 2;
                    }
                    '~' => {
                        out.push(' ');
                        i += 2;
                    }
                    '\\' | '{' | '}' => {
                        out.push(k);
                        i += 2;
                    }
                    'L' | 'l' | 'O' | 'o' | 'K' | 'k' | 'N' => i += 2,
                    'S' => {
                        // \Snum^den; or \Snum/den; or \Snum#den;
                        let mut j = i + 2;
                        let mut frac = String::new();
                        while j < chars.len() && chars[j] != ';' {
                            frac.push(if matches!(chars[j], '^' | '#') { '/' } else { chars[j] });
                            j += 1;
                        }
                        out.push_str(frac.trim());
                        i = j + 1;
                    }
                    _ => {
                        // \f...; \H...; \C...; \W...; \Q...; \T...; \A...; \c...;
                        let mut j = i + 2;
                        while j < chars.len() && chars[j] != ';' {
                            j += 1;
                        }
                        i = j + 1;
                    }
                }
            }
            '^' if i + 1 < chars.len() && chars[i + 1] == 'I' => {
                out.push('\t');
                i += 2;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Read a DXF file into a new document, with notes on what was left out. A drawing that
/// doesn't say its units (AutoCAD R12 files and older) is taken to be in millimeters.
pub fn load_with_notes(bytes: &[u8]) -> Result<(Document, Vec<String>)> {
    load_with_units(bytes, 72.0 / 25.4)
}

/// Like [`load_with_notes`], with the size of a drawing unit (in points) for drawings that don't
/// say their units.
pub fn load_with_units(bytes: &[u8], unknown_unit_points: f64) -> Result<(Document, Vec<String>)> {
    if bytes.starts_with(b"AutoCAD Binary DXF") {
        return Err(IoError::Unsupported("binary DXF (save it as ASCII DXF)".into()));
    }
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        // older files are in the Windows code page
        Err(_) => bytes.iter().map(|b| if *b < 0x80 { *b as char } else { encoding_1252(*b) }).collect(),
    };
    let p = pairs(&text);
    let secs = sections(&p);
    if !secs.contains_key("ENTITIES") {
        return Err(IoError::Format("not a DXF file (no ENTITIES section)".into()));
    }
    // header variables
    let mut insunits = None;
    let mut measurement = None;
    if let Some(h) = secs.get("HEADER") {
        let mut var = String::new();
        for (c, v) in h {
            match c {
                9 => var = v.trim().to_uppercase(),
                70 if var == "$INSUNITS" => insunits = v.trim().parse().ok(),
                70 if var == "$MEASUREMENT" => measurement = v.trim().parse().ok(),
                _ => {}
            }
        }
    }
    let mut r = Reader { layers: vec![], blocks: HashMap::new(), styles: HashMap::new(), items: vec![], skipped: BTreeMap::new(), paper_space: 0 };
    // tables: layers and text styles
    if let Some(t) = secs.get("TABLES") {
        for e in entities(t) {
            match e.kind.as_str() {
                "LAYER" => {
                    let Some(name) = e.get(2) else { continue };
                    let ci = e.i(62).unwrap_or(7);
                    let color = e.i(420).map(true_color).unwrap_or_else(|| aci(ci.abs()));
                    let off = ci < 0 || e.i(70).unwrap_or(0) & 1 == 1;
                    r.layers.push(LayerDef { name: name.trim().to_string(), color, on: !off, weight: e.i(370).unwrap_or(-3) });
                }
                "STYLE" => {
                    let (Some(name), Some(file)) = (e.get(2), e.get(3)) else { continue };
                    // the font file can be a full Windows path ("C:\WINDOWS\Fonts\ARIALBD.TTF"),
                    // which Linux would not take apart at the backslashes
                    let file_name = crate::paths::foreign_file_name(file);
                    let stem = std::path::Path::new(file_name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    let ttf = file.to_lowercase().ends_with(".ttf") || file.to_lowercase().ends_with(".otf");
                    let family = e.get(1000).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).or_else(|| ttf.then(|| font_family_of(&stem))).unwrap_or_else(|| "Arial".into());
                    r.styles.insert(name.trim().to_lowercase(), family);
                }
                _ => {}
            }
        }
    }
    // blocks
    if let Some(b) = secs.get("BLOCKS") {
        let ents = entities(b);
        let mut cur: Option<(String, Block)> = None;
        for e in ents {
            match e.kind.as_str() {
                "BLOCK" => {
                    let name = e.get(2).unwrap_or("").trim().to_string();
                    cur = Some((name, Block { base: e.pt(10).unwrap_or(Point::ZERO), ents: vec![] }));
                }
                "ENDBLK" => {
                    if let Some((n, b)) = cur.take() {
                        r.blocks.insert(n, b);
                    }
                }
                _ => {
                    if let Some((_, b)) = cur.as_mut() {
                        b.ents.push(e);
                    }
                }
            }
        }
    }
    let ctx = Ctx { xf: Affine::IDENTITY, layer: None, color: Color::BLACK, weight: -3, depth: 0 };
    for e in entities(&secs["ENTITIES"]) {
        r.entity(&e, &ctx);
    }
    let mut notes = vec![];
    let pts = match unit_points(insunits, measurement) {
        Some(p) => p,
        None => {
            if (unknown_unit_points - 72.0 / 25.4).abs() < 1e-9 {
                notes.push("The drawing doesn't say its units; it was read in millimeters".to_string());
            }
            unknown_unit_points
        }
    };
    if r.paper_space > 0 {
        notes.push(format!("Left out: {} object(s) in paper space (layouts); the model space was imported", r.paper_space));
    }
    for (k, n) in &r.skipped {
        notes.push(format!("Left out: {n} {k} entities"));
    }
    Ok((build_document(r, pts), notes))
}

fn encoding_1252(b: u8) -> char {
    const HI: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    if (0x80..0xa0).contains(&b) {
        HI[(b - 0x80) as usize]
    } else {
        b as char
    }
}

/// A font family from a font file name ("arialbd" -> "Arial", "times" -> "Times New Roman").
fn font_family_of(stem: &str) -> String {
    let s = stem.to_lowercase();
    let known = [("arial", "Arial"), ("times", "Times New Roman"), ("cour", "Courier New"), ("calibri", "Calibri"), ("verdana", "Verdana"), ("tahoma", "Tahoma"), ("segoe", "Segoe UI"), ("isocp", "Arial"), ("romans", "Arial"), ("simplex", "Arial")];
    for (k, f) in known {
        if s.starts_with(k) {
            return f.to_string();
        }
    }
    let mut c = stem.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "Arial".into(),
    }
}

/// Place the shapes on a page fitted around them: drawing units to points, y up to y down.
fn build_document(r: Reader, pts: f64) -> Document {
    let to_doc = |b: Rect| Affine::new([pts, 0.0, 0.0, -pts, -b.x0 * pts, b.y1 * pts]);
    // the drawing's extents
    let mut bb: Option<Rect> = None;
    for it in &r.items {
        let rb = match &it.shape {
            Shape2::Path { path, .. } => path.bounding_box(),
            Shape2::Text { text, xf } => xf.transform_rect_bbox(text::layout(text).bounds),
        };
        if rb.x0.is_finite() && rb.y0.is_finite() && rb.x1.is_finite() && rb.y1.is_finite() {
            bb = Some(bb.map_or(rb, |b| b.union(rb)));
        }
    }
    let bb = bb.unwrap_or(Rect::new(0.0, 0.0, 210.0 / 25.4 * 72.0 / pts, 297.0 / 25.4 * 72.0 / pts));
    let (w, h) = ((bb.width() * pts).max(1.0), (bb.height() * pts).max(1.0));
    let mut doc = Document::new(w, h);
    let place = to_doc(bb);
    // layers, in the order of the layer table, that have something on them
    let used: Vec<&LayerDef> = r.layers.iter().filter(|l| r.items.iter().any(|it| it.layer.eq_ignore_ascii_case(&l.name))).collect();
    let mut layers: Vec<Layer> = vec![];
    for l in &used {
        let id = doc.new_id();
        let mut layer = Layer::new(id, &l.name);
        layer.visible = l.on;
        layer.printable = l.on;
        layers.push(layer);
    }
    for it in r.items {
        let li = used.iter().position(|l| l.name.eq_ignore_ascii_case(&it.layer)).unwrap_or(0);
        let id = doc.new_id();
        let o = match it.shape {
            Shape2::Path { path, fill, even_odd } => {
                let mut st = Style::default();
                st.fill = if fill { Fill::Solid(it.color) } else { Fill::None };
                st.even_odd = even_odd;
                st.outline = if fill {
                    Outline::none()
                } else {
                    Outline { width: it.weight_mm * 72.0 / 25.4, color: Some(it.color), ..Outline::default() }
                };
                Object::new(id, ObjKind::Path(NodePath::from_bez(&(place * path))), Affine::IDENTITY, st)
            }
            Shape2::Text { mut text, xf } => {
                text.format.color = None;
                let mut st = Style::text_default();
                st.fill = Fill::Solid(it.color);
                Object::new(id, ObjKind::Text(text), place * xf, st)
            }
        };
        if let Some(l) = layers.get_mut(li) {
            l.objects.push(o);
        }
    }
    if !layers.is_empty() {
        doc.pages[0].layers = layers;
    }
    doc
}

#[cfg(test)]
mod tests;
