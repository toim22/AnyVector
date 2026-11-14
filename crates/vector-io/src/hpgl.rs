//! HPGL plotter file export (`.plt`, `.hgl`) for vinyl cutters and pen plotters.
//!
//! Curves are flattened to polylines within the curve resolution, text is output as its glyph
//! outlines, and each color is drawn with its own pen (`SP`). Coordinates are in plotter units
//! (1016 per inch by default) with the origin at the bottom left and y pointing up, the way
//! plotters count. Only outlines are exported unless filled objects are asked for, which are
//! then cut along their edge.

use crate::cutter::{self, CutPolyline, FillHandling};
use crate::Result;
use kurbo::{Point, Rect};
use std::fmt::Write;
use std::path::Path;
use vector_core::color::Color;
use vector_core::document::Document;
use vector_core::ObjId;

/// Where the plotter origin (0,0) is put.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum HpglOrigin {
    /// The bottom left corner of the page.
    #[default]
    PageBottomLeft,
    /// The center of the page.
    PageCenter,
    /// The bottom left corner of the exported objects (no material wasted on a cutter).
    ObjectsBottomLeft,
}

impl HpglOrigin {
    pub const ALL: [HpglOrigin; 3] = [HpglOrigin::PageBottomLeft, HpglOrigin::PageCenter, HpglOrigin::ObjectsBottomLeft];
    pub fn name(self) -> &'static str {
        match self {
            HpglOrigin::PageBottomLeft => "Bottom left of page",
            HpglOrigin::PageCenter => "Page center",
            HpglOrigin::ObjectsBottomLeft => "Bottom left of objects",
        }
    }
}

#[derive(Clone, Debug)]
pub struct HpglOptions {
    pub page: usize,
    /// Export only these top-level objects (the selection); `None` = the whole page.
    pub only: Option<Vec<ObjId>>,
    /// Plotter units per inch (1016 for HP-GL).
    pub units_per_inch: f64,
    /// Curve resolution: the largest distance, in points, between a curve and the straight
    /// segments that replace it.
    pub tolerance: f64,
    /// Scale factor (1 = 100%).
    pub scale: f64,
    pub origin: HpglOrigin,
    pub fills: FillHandling,
    /// Pen per color. Colors not listed get the next free pen in the order they first appear.
    pub pens: Vec<(Color, u16)>,
    /// Pen velocity (`VS`, cm/s); `None` leaves the plotter's own.
    pub velocity: Option<u32>,
}

impl Default for HpglOptions {
    fn default() -> Self {
        HpglOptions {
            page: 0,
            only: None,
            units_per_inch: 1016.0,
            // 0.004 inch, CorelDRAW's default curve resolution
            tolerance: 0.004 * 72.0,
            scale: 1.0,
            origin: HpglOrigin::PageBottomLeft,
            fills: FillHandling::OutlinesOnly,
            pens: vec![],
            velocity: None,
        }
    }
}

/// The distinct colors of the polylines in order of appearance, each with its pen: the pen in
/// `pens` when the color is listed there, otherwise the lowest pen number not yet used.
pub fn assign_pens(lines: &[CutPolyline], pens: &[(Color, u16)]) -> Vec<(Color, u16)> {
    let mut out: Vec<(Color, u16)> = vec![];
    for l in lines {
        if out.iter().any(|(c, _)| *c == l.color) {
            continue;
        }
        let pen = match pens.iter().find(|(c, _)| *c == l.color) {
            Some((_, p)) => *p,
            None => (1u16..).find(|p| !out.iter().any(|(_, q)| q == p) && !pens.iter().any(|(_, q)| q == p)).unwrap_or(1),
        };
        out.push((l.color, pen));
    }
    out
}

/// The colors (with their pens) that an export with these options would use.
pub fn used_pens(doc: &Document, opts: &HpglOptions) -> Vec<(Color, u16)> {
    assign_pens(&lines(doc, opts), &opts.pens)
}

fn lines(doc: &Document, opts: &HpglOptions) -> Vec<CutPolyline> {
    // the tolerance is on the output: shrink it when the drawing is scaled up
    let tol = opts.tolerance / opts.scale.abs().max(1e-6);
    cutter::page_polylines(doc, opts.page, opts.only.as_deref(), opts.fills, tol)
}

/// Map page points (y down) to device coordinates (y up) for the chosen origin and scale.
pub fn device_transform(doc: &Document, page: usize, origin: HpglOrigin, lines: &[CutPolyline]) -> impl Fn(Point) -> (f64, f64) {
    let (w, h) = doc.pages.get(page).map(|p| (p.width, p.height)).unwrap_or((0.0, 0.0));
    let (ox, oy) = match origin {
        HpglOrigin::PageBottomLeft => (0.0, h),
        HpglOrigin::PageCenter => (w / 2.0, h / 2.0),
        HpglOrigin::ObjectsBottomLeft => {
            let bb = lines.iter().flat_map(|l| l.points.iter()).fold(Rect::new(f64::MAX, f64::MAX, f64::MIN, f64::MIN), |r, p| Rect::new(r.x0.min(p.x), r.y0.min(p.y), r.x1.max(p.x), r.y1.max(p.y)));
            if bb.x0 <= bb.x1 {
                (bb.x0, bb.y1)
            } else {
                (0.0, h)
            }
        }
    };
    move |p: Point| (p.x - ox, oy - p.y)
}

/// The HPGL commands for a page.
pub fn to_hpgl(doc: &Document, opts: &HpglOptions) -> String {
    let lines = lines(doc, opts);
    let pens = assign_pens(&lines, &opts.pens);
    let to_dev = device_transform(doc, opts.page, opts.origin, &lines);
    let k = opts.units_per_inch / 72.0 * opts.scale;
    let pt = |p: Point| {
        let (x, y) = to_dev(p);
        ((x * k).round() as i64, (y * k).round() as i64)
    };
    let mut s = String::from("IN;\nPA;\n");
    let mut order: Vec<u16> = pens.iter().map(|(_, p)| *p).collect();
    order.sort_unstable();
    order.dedup();
    for pen in order {
        writeln!(s, "SP{pen};").unwrap();
        if let Some(v) = opts.velocity {
            writeln!(s, "VS{v};").unwrap();
        }
        for l in lines.iter().filter(|l| pens.iter().any(|(c, p)| *c == l.color && *p == pen)) {
            let mut pts: Vec<(i64, i64)> = l.points.iter().map(|p| pt(*p)).collect();
            if l.closed {
                pts.push(pts[0]);
            }
            pts.dedup();
            if pts.len() < 2 {
                continue;
            }
            writeln!(s, "PU{},{};", pts[0].0, pts[0].1).unwrap();
            let rest: Vec<String> = pts[1..].iter().map(|(x, y)| format!("{x},{y}")).collect();
            writeln!(s, "PD{};", rest.join(",")).unwrap();
        }
    }
    s.push_str("PU;\nSP0;\n");
    s
}

pub fn export(doc: &Document, path: &Path, opts: &HpglOptions) -> Result<()> {
    std::fs::write(path, to_hpgl(doc, opts))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Affine;
    use vector_core::object::{ObjKind, Object};
    use vector_core::{Fill, Style};

    fn poly(doc: &mut Document, pts: &[(f64, f64)], closed: bool, color: Color) -> Object {
        let pts: Vec<Point> = pts.iter().map(|p| Point::new(p.0, p.1)).collect();
        let mut st = Style::default();
        st.outline.color = Some(color);
        st.fill = Fill::None;
        Object::new(doc.new_id(), ObjKind::Path(vector_core::path::polyline(&pts, closed)), Affine::IDENTITY, st)
    }

    #[test]
    fn square_and_line_give_exact_commands() {
        // a 1 x 0.5 inch rectangle at the top left of a 2 x 2 inch page, plus a red line
        let mut doc = Document::new(144.0, 144.0);
        let r = poly(&mut doc, &[(0.0, 0.0), (72.0, 0.0), (72.0, 36.0), (0.0, 36.0)], true, Color::BLACK);
        let l = poly(&mut doc, &[(36.0, 72.0), (108.0, 144.0)], false, Color::rgb(255, 0, 0));
        doc.pages[0].layers[0].objects.extend([r, l]);
        let out = to_hpgl(&doc, &HpglOptions::default());
        assert_eq!(out, "IN;\nPA;\nSP1;\nPU0,2032;\nPD1016,2032,1016,1524,0,1524,0,2032;\nSP2;\nPU508,1016;\nPD1524,0;\nPU;\nSP0;\n");
    }

    #[test]
    fn pens_origin_scale_and_velocity() {
        let mut doc = Document::new(144.0, 144.0);
        let l = poly(&mut doc, &[(72.0, 72.0), (144.0, 72.0)], false, Color::rgb(0, 0, 255));
        doc.pages[0].layers[0].objects.push(l);
        let opts = HpglOptions { origin: HpglOrigin::PageCenter, scale: 0.5, pens: vec![(Color::rgb(0, 0, 255), 4)], velocity: Some(20), ..Default::default() };
        assert_eq!(to_hpgl(&doc, &opts), "IN;\nPA;\nSP4;\nVS20;\nPU0,0;\nPD508,0;\nPU;\nSP0;\n");
        let opts = HpglOptions { origin: HpglOrigin::ObjectsBottomLeft, units_per_inch: 72.0, ..Default::default() };
        assert_eq!(to_hpgl(&doc, &opts), "IN;\nPA;\nSP1;\nPU0,0;\nPD72,0;\nPU;\nSP0;\n");
    }

    #[test]
    fn auto_pens_skip_assigned_numbers() {
        let mk = |c: Color| CutPolyline { points: vec![Point::ZERO, Point::new(1.0, 0.0)], closed: false, color: c, layer: String::new() };
        let a = Color::rgb(1, 0, 0);
        let b = Color::rgb(2, 0, 0);
        let c = Color::rgb(3, 0, 0);
        let pens = assign_pens(&[mk(a), mk(b), mk(a), mk(c)], &[(b, 1)]);
        assert_eq!(pens, vec![(a, 2), (b, 1), (c, 3)]);
    }

    #[test]
    fn curves_are_flattened_with_the_curve_resolution() {
        use vector_core::object::{EllipseKind, EllipseShape};
        let mut doc = Document::new(288.0, 288.0);
        let e = EllipseShape { w: 144.0, h: 144.0, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false };
        let o = Object::new(doc.new_id(), ObjKind::Ellipse(e), Affine::translate((72.0, 72.0)), Style::default());
        doc.pages[0].layers[0].objects.push(o);
        let count = |tol: f64| to_hpgl(&doc, &HpglOptions { tolerance: tol, ..Default::default() }).matches(',').count();
        assert!(count(0.01) > count(0.5), "a finer curve resolution gives more points");
        // every vertex is on the circle (radius 1 inch = 1016 units around 2032,2032)
        let out = to_hpgl(&doc, &HpglOptions::default());
        let pd = out.lines().find(|l| l.starts_with("PD")).unwrap();
        let nums: Vec<f64> = pd[2..pd.len() - 1].split(',').map(|v| v.parse().unwrap()).collect();
        for xy in nums.chunks(2) {
            let r = ((xy[0] - 2032.0).powi(2) + (xy[1] - 2032.0).powi(2)).sqrt();
            assert!((r - 1016.0).abs() < 2.0, "vertex {xy:?} is {r} from the center");
        }
    }
}
