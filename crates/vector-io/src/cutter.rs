//! Shared geometry for sign-making and CAD output (HPGL plotter files, DXF and device plugins):
//! the paths a cutter or plotter would follow, each with its color and layer.
//!
//! Everything is taken from the visible, printable layers of one page, in document points with
//! the y axis pointing down (page coordinates). Text is converted to curves (glyph outlines),
//! groups, blends, symbols and PowerClips are opened up (PowerClip contents are not clipped),
//! contour effects add their steps, and bitmaps are skipped since they cannot be cut.

use kurbo::{BezPath, Point};
use vector_core::color::{Color, NamedColor};
use vector_core::document::Document;
use vector_core::geom;
use vector_core::object::{contour_rings, ObjKind, Object};
use vector_core::ObjId;

/// One object's geometry and colors.
#[derive(Clone, Debug)]
pub struct CutShape {
    /// Page coordinates (points, y down).
    pub path: BezPath,
    /// The outline color, when the outline is visible.
    pub outline: Option<Color>,
    /// The outline width in points (0 = hairline).
    pub outline_width: f64,
    /// The main fill color, when the object is filled.
    pub fill: Option<Color>,
    /// The name of the layer the object is on.
    pub layer: String,
}

/// What to do with objects that have a fill but no outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum FillHandling {
    /// Export outlines only: objects without an outline are left out.
    #[default]
    OutlinesOnly,
    /// Objects without an outline but with a fill are exported as their outline, in the fill
    /// color.
    FilledAsOutline,
}

/// A flattened path for a cutter or plotter: straight segments only.
#[derive(Clone, Debug, PartialEq)]
pub struct CutPolyline {
    /// Page coordinates (points, y down). A closed polyline does not repeat its first point.
    pub points: Vec<Point>,
    pub closed: bool,
    /// The color the path is drawn or cut with (outline color, or fill color for filled
    /// objects exported as outlines).
    pub color: Color,
    pub layer: String,
}

pub use vector_core::color::{CUT_CONTOUR, CUT_CONTOUR_NAME};

/// The CutContour spot color (see [`NamedColor::cut_contour`]).
pub fn cut_contour() -> NamedColor {
    NamedColor::cut_contour()
}

/// The geometry of a page (all visible, printable layers), or only of the top-level objects in
/// `only` when it is given.
pub fn collect(doc: &Document, page: usize, only: Option<&[ObjId]>) -> Vec<CutShape> {
    let mut out = vec![];
    for (_, _, l) in doc.layers_on_page(page) {
        if !l.visible || !l.printable {
            continue;
        }
        for o in &l.objects {
            if only.is_some_and(|ids| !ids.contains(&o.id)) {
                continue;
            }
            let o = doc.object_on_page(o, page);
            shapes(doc, &o, &l.name, &mut out, false);
        }
    }
    out
}

/// The shapes of an object; with `images`, a bitmap gives its frame (filled white).
fn shapes(doc: &Document, o: &Object, layer: &str, out: &mut Vec<CutShape>, images: bool) {
    if !o.visible || !o.printable {
        return;
    }
    if let Some(c) = &o.effects.contour {
        for (path, fill, outline) in contour_rings(o, c) {
            let outline = if o.style.outline.is_visible() { outline } else { None };
            out.push(CutShape { path, outline, outline_width: o.style.outline.width, fill: fill.main_color(), layer: layer.into() });
        }
    }
    match &o.kind {
        ObjKind::Group(children) => children.iter().for_each(|c| shapes(doc, c, layer, out, images)),
        ObjKind::PowerClip { frame, contents } => {
            shapes(doc, frame, layer, out, images);
            contents.iter().for_each(|c| shapes(doc, c, layer, out, images));
        }
        ObjKind::Blend(b) => {
            shapes(doc, &b.start, layer, out, images);
            for step in vector_core::object::blend_steps(b, || 0) {
                shapes(doc, &step, layer, out, images);
            }
            shapes(doc, &b.end, layer, out, images);
        }
        ObjKind::Symbol(sid) => {
            if let Some(def) = doc.symbols.iter().find(|s| s.id == *sid) {
                for c in &def.objects {
                    let mut c2 = c.clone();
                    c2.apply_transform(o.transform);
                    shapes(doc, &c2, layer, out, images);
                }
            }
        }
        ObjKind::Image(_) => {
            if images {
                out.push(CutShape { path: o.world_path(), outline: None, outline_width: 0.0, fill: Some(Color::WHITE), layer: layer.into() });
            }
        }
        _ => {
            let path = o.world_path();
            if path.elements().is_empty() {
                return;
            }
            let outline = o.style.outline.color.filter(|c| c.a > 0);
            let fill = o.style.fill.main_color().filter(|c| c.a > 0);
            out.push(CutShape { path, outline, outline_width: o.style.outline.width, fill, layer: layer.into() });
        }
    }
}

/// The color a shape is output with, or `None` when it is left out.
pub fn shape_color(s: &CutShape, fills: FillHandling) -> Option<Color> {
    match (s.outline, fills) {
        (Some(c), _) => Some(c),
        (None, FillHandling::FilledAsOutline) => s.fill,
        (None, FillHandling::OutlinesOnly) => None,
    }
}

/// Flatten a path into polylines of at least two distinct points: (points, closed). A subpath
/// that ends where it starts counts as closed, and a closed polyline does not repeat its first
/// point.
pub fn polylines(path: &BezPath, tolerance: f64) -> Vec<(Vec<Point>, bool)> {
    let same = |a: &Point, b: &Point| (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6;
    let mut out = vec![];
    for (mut points, mut closed) in geom::flatten_polylines(path, tolerance.max(0.001)) {
        points.dedup_by(|a, b| same(a, b));
        if points.len() > 2 && same(&points[0], &points[points.len() - 1]) {
            points.pop();
            closed = true;
        }
        if points.len() >= 2 {
            let closed = closed && points.len() > 2;
            out.push((points, closed));
        }
    }
    out
}

/// Flatten shapes into polylines whose segments stay within `tolerance` points of the curves.
pub fn flatten(shapes: &[CutShape], fills: FillHandling, tolerance: f64) -> Vec<CutPolyline> {
    let mut out = vec![];
    for s in shapes {
        let Some(color) = shape_color(s, fills) else { continue };
        for (points, closed) in polylines(&s.path, tolerance) {
            out.push(CutPolyline { points, closed, color, layer: s.layer.clone() });
        }
    }
    out
}

/// The polylines of a page (or of the top-level objects in `only`), ready for a cutter.
pub fn page_polylines(doc: &Document, page: usize, only: Option<&[ObjId]>, fills: FillHandling, tolerance: f64) -> Vec<CutPolyline> {
    flatten(&collect(doc, page, only), fills, tolerance)
}

/// The cut line around objects for print-and-cut: the outside edge of everything they cover
/// (fills, outlines, text and bitmap frames), grown by `offset` points. With `outer_only`, holes
/// are dropped so the cutter follows the outside only.
pub fn contour_for_cutting(doc: &Document, objects: &[&Object], offset: f64, outer_only: bool) -> BezPath {
    let mut found = vec![];
    for o in objects {
        shapes(doc, o, "", &mut found, true);
    }
    let mut areas = vec![];
    for s in &found {
        let closed = s.path.elements().iter().any(|e| matches!(e, kurbo::PathEl::ClosePath)) || polylines(&s.path, 0.5).iter().all(|(_, c)| *c);
        if closed && s.fill.is_some() {
            areas.push(s.path.clone());
        }
        if s.outline.is_some() {
            areas.push(geom::stroke_to_path(&s.path, s.outline_width.max(0.25), kurbo::Join::Round, kurbo::Cap::Round, &[]));
        }
    }
    if areas.is_empty() {
        return BezPath::new();
    }
    let all: BezPath = areas.iter().flat_map(|p| p.elements().iter().copied()).collect();
    let mut edge = geom::simplify_self(&all);
    if offset.abs() > 1e-9 {
        edge = geom::offset_path(&edge, offset, kurbo::Join::Round);
    }
    if !outer_only {
        return edge;
    }
    let parts = geom::subpaths(&edge);
    let mut out = BezPath::new();
    for (i, p) in parts.iter().enumerate() {
        let Some(start) = p.elements().first().and_then(|e| match e {
            kurbo::PathEl::MoveTo(a) => Some(*a),
            _ => None,
        }) else {
            continue;
        };
        let inside_other = parts.iter().enumerate().any(|(j, q)| j != i && geom::path_area(q).abs() > geom::path_area(p).abs() && geom::contains(q, start, false));
        if !inside_other {
            out.extend(p.elements().iter().copied());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Affine;
    use vector_core::object::{EllipseKind, EllipseShape};
    use vector_core::{Fill, Style};

    fn circle(doc: &mut Document, r: f64) -> Object {
        let e = EllipseShape { w: 2.0 * r, h: 2.0 * r, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false };
        Object::new(doc.new_id(), ObjKind::Ellipse(e), Affine::translate((100.0, 100.0)), Style::default())
    }

    #[test]
    fn flattening_stays_within_tolerance() {
        let mut doc = Document::new(400.0, 400.0);
        let r = 72.0;
        let o = circle(&mut doc, r);
        doc.pages[0].layers[0].objects.push(o);
        let shapes = collect(&doc, 0, None);
        let center = Point::new(100.0 + r, 100.0 + r);
        let mut counts = vec![];
        for tol in [1.0, 0.25, 0.01] {
            let lines = flatten(&shapes, FillHandling::OutlinesOnly, tol);
            assert_eq!(lines.len(), 1);
            let pl = &lines[0];
            assert!(pl.closed);
            let n = pl.points.len();
            counts.push(n);
            for i in 0..n {
                let (a, b) = (pl.points[i], pl.points[(i + 1) % n]);
                // vertices lie on the circle (the ellipse is a cubic approximation, so allow a
                // hair) and chord midpoints are no farther than the tolerance from it
                assert!(((a - center).hypot() - r).abs() < 0.05, "vertex off the curve");
                let mid = a.midpoint(b);
                assert!(r - (mid - center).hypot() <= tol + 0.05, "segment deviates more than {tol}");
            }
        }
        assert!(counts[0] < counts[1] && counts[1] < counts[2], "finer tolerance gives more points: {counts:?}");
    }

    #[test]
    fn fill_handling() {
        let mut doc = Document::new(400.0, 400.0);
        let mut o = circle(&mut doc, 10.0);
        o.style.outline.color = None;
        o.style.fill = Fill::Solid(Color::RED);
        doc.pages[0].layers[0].objects.push(o);
        assert!(page_polylines(&doc, 0, None, FillHandling::OutlinesOnly, 0.1).is_empty());
        let v = page_polylines(&doc, 0, None, FillHandling::FilledAsOutline, 0.1);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].color, Color::RED);
        assert_eq!(v[0].layer, doc.pages[0].layers[0].name);
    }

    #[test]
    fn contour_for_cutting_surrounds_the_artwork() {
        use kurbo::Shape;
        let mut doc = Document::new(400.0, 400.0);
        // a ring (outline only, 4 pt wide) and a filled square apart from it
        let mut ring = circle(&mut doc, 20.0);
        ring.style.outline.width = 4.0;
        ring.style.fill = Fill::None;
        let sq = vector_core::path::polyline(&[Point::new(200.0, 100.0), Point::new(240.0, 100.0), Point::new(240.0, 140.0), Point::new(200.0, 140.0)], true);
        let mut st = Style::filled(Color::RED);
        st.outline.color = None;
        let sq = Object::new(doc.new_id(), ObjKind::Path(sq), Affine::IDENTITY, st);
        let p = contour_for_cutting(&doc, &[&ring, &sq], 3.0, true);
        let parts = geom::subpaths(&p);
        assert_eq!(parts.len(), 2, "one cut line around each, the ring's hole dropped");
        let bb = p.bounding_box();
        // the ring reaches 22 pt from its center (half the outline), plus the 3 pt offset
        assert!((bb.x0 - (120.0 - 25.0)).abs() < 0.5, "{bb:?}");
        assert!((bb.x1 - 243.0).abs() < 0.5, "{bb:?}");
        let with_holes = contour_for_cutting(&doc, &[&ring, &sq], 3.0, false);
        assert_eq!(geom::subpaths(&with_holes).len(), 3);
    }

    #[test]
    fn hidden_and_non_printing_layers_are_skipped() {
        let mut doc = Document::new(400.0, 400.0);
        let o = circle(&mut doc, 10.0);
        doc.pages[0].layers[0].objects.push(o);
        doc.pages[0].layers[0].printable = false;
        assert!(collect(&doc, 0, None).is_empty());
    }
}
