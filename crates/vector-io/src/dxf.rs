//! DXF export (AutoCAD R12 or 2000 ASCII) for CAD, laser and cutter software.
//!
//! Every AnyVector layer becomes a DXF layer. Paths are written either flattened (LINE and
//! POLYLINE/LWPOLYLINE entities) or, for AutoCAD 2000, with their curves kept as cubic SPLINE
//! entities. Colors are written as the nearest AutoCAD Color Index, plus the true color for
//! AutoCAD 2000. The y axis points up with the origin at the bottom left of the page, in the
//! chosen drawing units. Text is written as its glyph outlines.

use crate::cutter::{self, CutShape, FillHandling};
use crate::Result;
use kurbo::{BezPath, PathEl, Point};
use std::fmt::Write;
use std::path::Path;
use vector_core::color::Color;
use vector_core::document::Document;
use vector_core::ObjId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum DxfVersion {
    /// AutoCAD Release 12 (AC1009): the most widely read.
    #[default]
    R12,
    /// AutoCAD 2000 (AC1015): LWPOLYLINE, SPLINE and true colors.
    R2000,
}

impl DxfVersion {
    pub const ALL: [DxfVersion; 2] = [DxfVersion::R12, DxfVersion::R2000];
    pub fn name(self) -> &'static str {
        match self {
            DxfVersion::R12 => "AutoCAD R12 (DXF R12)",
            DxfVersion::R2000 => "AutoCAD 2000 (DXF R2000)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum DxfUnits {
    Inches,
    #[default]
    Millimeters,
    Centimeters,
    Meters,
    Points,
}

impl DxfUnits {
    pub const ALL: [DxfUnits; 5] = [DxfUnits::Inches, DxfUnits::Millimeters, DxfUnits::Centimeters, DxfUnits::Meters, DxfUnits::Points];
    pub fn name(self) -> &'static str {
        match self {
            DxfUnits::Inches => "Inches",
            DxfUnits::Millimeters => "Millimeters",
            DxfUnits::Centimeters => "Centimeters",
            DxfUnits::Meters => "Meters",
            DxfUnits::Points => "Points",
        }
    }
    /// Drawing units per point.
    pub fn per_point(self) -> f64 {
        match self {
            DxfUnits::Inches => 1.0 / 72.0,
            DxfUnits::Millimeters => 25.4 / 72.0,
            DxfUnits::Centimeters => 2.54 / 72.0,
            DxfUnits::Meters => 0.0254 / 72.0,
            DxfUnits::Points => 1.0,
        }
    }
    /// `$INSUNITS` code (AutoCAD 2000).
    fn insunits(self) -> i32 {
        match self {
            DxfUnits::Inches => 1,
            DxfUnits::Millimeters => 4,
            DxfUnits::Centimeters => 5,
            DxfUnits::Meters => 6,
            DxfUnits::Points => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DxfOptions {
    pub page: usize,
    /// Export only these top-level objects (the selection); `None` = the whole page.
    pub only: Option<Vec<ObjId>>,
    pub version: DxfVersion,
    pub units: DxfUnits,
    /// Keep curves as SPLINE entities (AutoCAD 2000 only); otherwise flatten them.
    pub splines: bool,
    /// Flattening tolerance in points.
    pub tolerance: f64,
    /// Also write true colors (AutoCAD 2000 only).
    pub true_color: bool,
    pub fills: FillHandling,
}

impl Default for DxfOptions {
    fn default() -> Self {
        DxfOptions { page: 0, only: None, version: DxfVersion::R12, units: DxfUnits::Millimeters, splines: false, tolerance: 0.1, true_color: true, fills: FillHandling::FilledAsOutline }
    }
}

// ------------------------------------------------------------------------------------------
// AutoCAD Color Index
// ------------------------------------------------------------------------------------------

/// The RGB value of an AutoCAD Color Index (1..=255).
pub fn aci_rgb(i: u8) -> [u8; 3] {
    match i {
        1 => [255, 0, 0],
        2 => [255, 255, 0],
        3 => [0, 255, 0],
        4 => [0, 255, 255],
        5 => [0, 0, 255],
        6 => [255, 0, 255],
        7 => [255, 255, 255],
        8 => [65, 65, 65],
        9 => [128, 128, 128],
        10..=249 => {
            let k = i - 10;
            let hue = (k / 10) as f64 * 15.0;
            let v = [255.0, 204.0, 153.0, 127.0, 76.0][((k % 10) / 2) as usize];
            let s = if k % 2 == 0 { 1.0 } else { 0.5 };
            hsv(hue, s, v)
        }
        250..=255 => {
            let g = [51, 80, 105, 130, 190, 255][(i - 250) as usize];
            [g, g, g]
        }
        _ => [0, 0, 0],
    }
}

fn hsv(h: f64, s: f64, v: f64) -> [u8; 3] {
    let c = v * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let q = |t: f64| (t + m).floor().clamp(0.0, 255.0) as u8;
    [q(r), q(g), q(b)]
}

/// The nearest AutoCAD Color Index for a color. Black and white both map to 7, which CAD
/// programs draw in the contrasting color of the background.
pub fn nearest_aci(c: Color) -> u8 {
    if (c.r, c.g, c.b) == (0, 0, 0) || (c.r, c.g, c.b) == (255, 255, 255) {
        return 7;
    }
    let d = |a: [u8; 3]| {
        let (dr, dg, db) = (a[0] as i32 - c.r as i32, a[1] as i32 - c.g as i32, a[2] as i32 - c.b as i32);
        2 * dr * dr + 4 * dg * dg + 3 * db * db
    };
    (1u8..=255).filter(|i| *i != 7).min_by_key(|i| d(aci_rgb(*i))).unwrap_or(7)
}

// ------------------------------------------------------------------------------------------
// Writer
// ------------------------------------------------------------------------------------------

/// Group code / value pairs.
struct Out {
    s: String,
    handle: u32,
}

impl Out {
    fn pair(&mut self, code: i32, value: impl std::fmt::Display) {
        writeln!(self.s, "{code:>3}\n{value}").unwrap();
    }
    fn num(&mut self, code: i32, v: f64) {
        let r = (v * 1e6).round() / 1e6;
        let r = if r == 0.0 { 0.0 } else { r };
        self.pair(code, format!("{r}"));
    }
    fn point(&mut self, base: i32, p: (f64, f64)) {
        self.num(base, p.0);
        self.num(base + 10, p.1);
        self.num(base + 20, 0.0);
    }
    fn next_handle(&mut self) -> String {
        self.handle += 1;
        format!("{:X}", self.handle)
    }
}

/// DXF layer names may not contain `<>/\":;?*|=',` and must not be empty.
pub fn layer_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if "<>/\\\":;?*|=',".contains(c) || c.is_control() { '_' } else { c }).collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "0".into()
    } else {
        s
    }
}

/// The DXF text for a page.
pub fn to_dxf(doc: &Document, opts: &DxfOptions) -> String {
    let shapes = cutter::collect(doc, opts.page, opts.only.as_deref());
    let h = doc.pages.get(opts.page).map(|p| p.height).unwrap_or(0.0);
    let (w_pg, k) = (doc.pages.get(opts.page).map(|p| p.width).unwrap_or(0.0), opts.units.per_point());
    let map = move |p: Point| (p.x * k, (h - p.y) * k);
    let r2000 = opts.version == DxfVersion::R2000;
    let splines = r2000 && opts.splines;
    let true_color = r2000 && opts.true_color;

    // the layers, in document order (the page's, then master layers), only those exported
    let mut layers: Vec<String> = vec![];
    for (_, _, l) in doc.layers_on_page(opts.page) {
        if l.visible && l.printable {
            let n = layer_name(&l.name);
            if !layers.contains(&n) {
                layers.push(n);
            }
        }
    }
    if !layers.iter().any(|l| l == "0") {
        layers.insert(0, "0".into());
    }

    // R2000 handles: fixed ones for the tables and blocks below 0x30, then the layers, then the
    // entities (written first, to know the handle seed)
    let mut o = Out { s: String::new(), handle: 0x2F };
    let mut ents = Out { s: String::new(), handle: 0x40 + layers.len() as u32 };
    let model = "1F";
    for s in &shapes {
        let Some(color) = cutter::shape_color(s, opts.fills) else { continue };
        entities(&mut ents, s, color, &map, opts, splines, true_color, r2000, model);
    }

    // HEADER
    o.pair(0, "SECTION");
    o.pair(2, "HEADER");
    o.pair(9, "$ACADVER");
    o.pair(1, if r2000 { "AC1015" } else { "AC1009" });
    if r2000 {
        o.pair(9, "$HANDSEED");
        o.pair(5, format!("{:X}", ents.handle + 1));
        o.pair(9, "$INSUNITS");
        o.pair(70, opts.units.insunits());
        o.pair(9, "$MEASUREMENT");
        o.pair(70, if opts.units == DxfUnits::Inches { 0 } else { 1 });
    }
    o.pair(9, "$INSBASE");
    o.point(10, (0.0, 0.0));
    o.pair(9, "$EXTMIN");
    o.point(10, (0.0, 0.0));
    o.pair(9, "$EXTMAX");
    o.point(10, (w_pg * k, h * k));
    o.pair(9, "$LIMMIN");
    o.num(10, 0.0);
    o.num(20, 0.0);
    o.pair(9, "$LIMMAX");
    o.num(10, w_pg * k);
    o.num(20, h * k);
    o.pair(0, "ENDSEC");
    if r2000 {
        o.pair(0, "SECTION");
        o.pair(2, "CLASSES");
        o.pair(0, "ENDSEC");
    }

    // TABLES
    o.pair(0, "SECTION");
    o.pair(2, "TABLES");
    if r2000 {
        tables_r2000(&mut o, &layers, model);
    } else {
        tables_r12(&mut o, &layers);
    }
    o.pair(0, "ENDSEC");

    // BLOCKS
    o.pair(0, "SECTION");
    o.pair(2, "BLOCKS");
    if r2000 {
        for (name, rec, blk, end) in [("*Model_Space", model, "20", "21"), ("*Paper_Space", "1E", "22", "23")] {
            o.pair(0, "BLOCK");
            o.pair(5, blk);
            o.pair(330, rec);
            o.pair(100, "AcDbEntity");
            if name == "*Paper_Space" {
                o.pair(67, 1);
            }
            o.pair(8, "0");
            o.pair(100, "AcDbBlockBegin");
            o.pair(2, name);
            o.pair(70, 0);
            o.point(10, (0.0, 0.0));
            o.pair(3, name);
            o.pair(1, "");
            o.pair(0, "ENDBLK");
            o.pair(5, end);
            o.pair(330, rec);
            o.pair(100, "AcDbEntity");
            if name == "*Paper_Space" {
                o.pair(67, 1);
            }
            o.pair(8, "0");
            o.pair(100, "AcDbBlockEnd");
        }
    }
    o.pair(0, "ENDSEC");

    // ENTITIES
    o.pair(0, "SECTION");
    o.pair(2, "ENTITIES");
    o.s.push_str(&ents.s);
    o.pair(0, "ENDSEC");

    if r2000 {
        // the named object dictionary with the (empty) group dictionary
        o.pair(0, "SECTION");
        o.pair(2, "OBJECTS");
        o.pair(0, "DICTIONARY");
        o.pair(5, "C");
        o.pair(330, 0);
        o.pair(100, "AcDbDictionary");
        o.pair(281, 1);
        o.pair(3, "ACAD_GROUP");
        o.pair(350, "D");
        o.pair(0, "DICTIONARY");
        o.pair(5, "D");
        o.pair(330, "C");
        o.pair(100, "AcDbDictionary");
        o.pair(281, 1);
        o.pair(0, "ENDSEC");
    }
    o.pair(0, "EOF");
    o.s
}

fn tables_r12(o: &mut Out, layers: &[String]) {
    o.pair(0, "TABLE");
    o.pair(2, "LTYPE");
    o.pair(70, 1);
    o.pair(0, "LTYPE");
    o.pair(2, "CONTINUOUS");
    o.pair(70, 0);
    o.pair(3, "Solid line");
    o.pair(72, 65);
    o.pair(73, 0);
    o.num(40, 0.0);
    o.pair(0, "ENDTAB");
    o.pair(0, "TABLE");
    o.pair(2, "LAYER");
    o.pair(70, layers.len());
    for l in layers {
        o.pair(0, "LAYER");
        o.pair(2, l);
        o.pair(70, 0);
        o.pair(62, 7);
        o.pair(6, "CONTINUOUS");
    }
    o.pair(0, "ENDTAB");
}

/// The symbol tables AutoCAD 2000 requires, with fixed handles below 0x100.
fn tables_r2000(o: &mut Out, layers: &[String], model: &str) {
    let table = |o: &mut Out, name: &str, handle: &str, count: usize| {
        o.pair(0, "TABLE");
        o.pair(2, name);
        o.pair(5, handle);
        o.pair(330, 0);
        o.pair(100, "AcDbSymbolTable");
        o.pair(70, count);
    };
    let record = |o: &mut Out, kind: &str, handle: String, owner: &str, sub: &str, name: &str| {
        o.pair(0, kind);
        o.pair(5, handle);
        o.pair(330, owner);
        o.pair(100, "AcDbSymbolTableRecord");
        o.pair(100, sub);
        o.pair(2, name);
        o.pair(70, 0);
    };
    // VPORT
    table(o, "VPORT", "8", 0);
    o.pair(0, "ENDTAB");
    // LTYPE
    table(o, "LTYPE", "5", 3);
    for (h, name, desc) in [("14", "ByBlock", ""), ("15", "ByLayer", ""), ("16", "Continuous", "Solid line")] {
        record(o, "LTYPE", h.into(), "5", "AcDbLinetypeTableRecord", name);
        o.pair(3, desc);
        o.pair(72, 65);
        o.pair(73, 0);
        o.num(40, 0.0);
    }
    o.pair(0, "ENDTAB");
    // LAYER
    table(o, "LAYER", "2", layers.len());
    for l in layers {
        let h = o.next_handle();
        record(o, "LAYER", h, "2", "AcDbLayerTableRecord", l);
        o.pair(62, 7);
        o.pair(6, "Continuous");
        o.pair(370, -3);
        o.pair(390, "F");
    }
    o.pair(0, "ENDTAB");
    // STYLE
    table(o, "STYLE", "3", 1);
    record(o, "STYLE", "11".into(), "3", "AcDbTextStyleTableRecord", "Standard");
    o.num(40, 0.0);
    o.num(41, 1.0);
    o.num(50, 0.0);
    o.pair(71, 0);
    o.num(42, 2.5);
    o.pair(3, "txt");
    o.pair(4, "");
    o.pair(0, "ENDTAB");
    // VIEW, UCS
    table(o, "VIEW", "6", 0);
    o.pair(0, "ENDTAB");
    table(o, "UCS", "7", 0);
    o.pair(0, "ENDTAB");
    // APPID
    table(o, "APPID", "9", 1);
    record(o, "APPID", "12".into(), "9", "AcDbRegAppTableRecord", "ACAD");
    o.pair(0, "ENDTAB");
    // DIMSTYLE
    o.pair(0, "TABLE");
    o.pair(2, "DIMSTYLE");
    o.pair(5, "A");
    o.pair(330, 0);
    o.pair(100, "AcDbSymbolTable");
    o.pair(70, 1);
    o.pair(100, "AcDbDimStyleTable");
    o.pair(0, "DIMSTYLE");
    o.pair(105, "27");
    o.pair(330, "A");
    o.pair(100, "AcDbSymbolTableRecord");
    o.pair(100, "AcDbDimStyleTableRecord");
    o.pair(2, "Standard");
    o.pair(70, 0);
    o.pair(0, "ENDTAB");
    // BLOCK_RECORD
    table(o, "BLOCK_RECORD", "1", 2);
    record(o, "BLOCK_RECORD", model.into(), "1", "AcDbBlockTableRecord", "*Model_Space");
    record(o, "BLOCK_RECORD", "1E".into(), "1", "AcDbBlockTableRecord", "*Paper_Space");
    o.pair(0, "ENDTAB");
}

#[allow(clippy::too_many_arguments)]
fn entities(o: &mut Out, s: &CutShape, color: Color, map: &dyn Fn(Point) -> (f64, f64), opts: &DxfOptions, splines: bool, true_color: bool, r2000: bool, model: &str) {
    let layer = layer_name(&s.layer);
    let aci = nearest_aci(color);
    let head = |o: &mut Out, kind: &str, sub: &str| {
        o.pair(0, kind);
        if r2000 {
            let h = o.next_handle();
            o.pair(5, h);
            o.pair(330, model);
            o.pair(100, "AcDbEntity");
        }
        o.pair(8, &layer);
        o.pair(62, aci);
        if true_color {
            o.pair(420, ((color.r as u32) << 16) | ((color.g as u32) << 8) | color.b as u32);
        }
        if r2000 {
            o.pair(100, sub);
        }
    };
    for sub in vector_core::geom::subpaths(&s.path) {
        let has_curves = sub.elements().iter().any(|e| matches!(e, PathEl::CurveTo(..) | PathEl::QuadTo(..)));
        if splines && has_curves {
            let (ctrl, closed) = bezier_controls(&sub);
            if ctrl.len() < 4 {
                continue;
            }
            let n = (ctrl.len() - 1) / 3;
            head(o, "SPLINE", "AcDbSpline");
            o.num(210, 0.0);
            o.num(220, 0.0);
            o.num(230, 1.0);
            o.pair(70, if closed { 8 | 1 } else { 8 });
            o.pair(71, 3);
            o.pair(72, ctrl.len() + 4);
            o.pair(73, ctrl.len());
            o.pair(74, 0);
            // a clamped knot vector with triple interior knots: each segment is one Bézier
            let mut knots = vec![0.0; 4];
            for i in 1..n {
                knots.extend([i as f64; 3]);
            }
            knots.extend([n as f64; 4]);
            for kv in knots {
                o.num(40, kv);
            }
            for p in ctrl {
                o.point(10, map(p));
            }
            continue;
        }
        for (pts, closed) in cutter::polylines(&sub, opts.tolerance) {
            let pts: Vec<(f64, f64)> = pts.into_iter().map(map).collect();
            if pts.len() == 2 && !closed {
                head(o, "LINE", "AcDbLine");
                o.point(10, pts[0]);
                o.point(11, pts[1]);
            } else if r2000 {
                head(o, "LWPOLYLINE", "AcDbPolyline");
                o.pair(90, pts.len());
                o.pair(70, if closed { 1 } else { 0 });
                for p in pts {
                    o.num(10, p.0);
                    o.num(20, p.1);
                }
            } else {
                head(o, "POLYLINE", "");
                o.pair(66, 1);
                o.point(10, (0.0, 0.0));
                o.pair(70, if closed { 1 } else { 0 });
                for p in pts {
                    o.pair(0, "VERTEX");
                    o.pair(8, &layer);
                    o.point(10, p);
                }
                o.pair(0, "SEQEND");
                o.pair(8, &layer);
            }
        }
    }
}

/// The control points of a subpath as a chain of cubic Béziers (lines and quadratics are
/// raised to cubics), and whether it was closed.
fn bezier_controls(p: &BezPath) -> (Vec<Point>, bool) {
    let mut v: Vec<Point> = vec![];
    let mut start = Point::ZERO;
    let mut closed = false;
    for el in p.elements() {
        let last = v.last().copied().unwrap_or(start);
        match *el {
            PathEl::MoveTo(a) => {
                start = a;
                v.push(a);
            }
            PathEl::LineTo(a) => v.extend([last.lerp(a, 1.0 / 3.0), last.lerp(a, 2.0 / 3.0), a]),
            PathEl::QuadTo(c, a) => v.extend([last.lerp(c, 2.0 / 3.0), a.lerp(c, 2.0 / 3.0), a]),
            PathEl::CurveTo(a, b, c) => v.extend([a, b, c]),
            PathEl::ClosePath => {
                closed = true;
                if last.distance(start) > 1e-9 {
                    v.extend([last.lerp(start, 1.0 / 3.0), last.lerp(start, 2.0 / 3.0), start]);
                }
            }
        }
    }
    (v, closed)
}

pub fn export(doc: &Document, path: &Path, opts: &DxfOptions) -> Result<()> {
    std::fs::write(path, to_dxf(doc, opts))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Affine;
    use vector_core::object::{EllipseKind, EllipseShape, ObjKind, Object};
    use vector_core::{Fill, Style};

    /// A minimal DXF reader: (code, value) pairs, checked for the group structure.
    fn pairs(s: &str) -> Vec<(i32, String)> {
        let lines: Vec<&str> = s.lines().collect();
        assert!(lines.len() % 2 == 0, "odd number of lines");
        lines.chunks(2).map(|c| (c[0].trim().parse::<i32>().expect("group code"), c[1].to_string())).collect()
    }

    /// The entities of the ENTITIES section: (type, its pairs).
    fn sections(p: &[(i32, String)]) -> Vec<(String, Vec<(i32, String)>)> {
        let mut out: Vec<(String, Vec<(i32, String)>)> = vec![];
        let mut i = 0;
        let mut depth = 0;
        while i < p.len() {
            match (p[i].0, p[i].1.as_str()) {
                (0, "SECTION") => {
                    depth += 1;
                    assert_eq!(p[i + 1].0, 2);
                    out.push((p[i + 1].1.clone(), vec![]));
                    i += 2;
                    continue;
                }
                (0, "ENDSEC") => depth -= 1,
                (0, "EOF") => {
                    assert_eq!(i, p.len() - 1, "EOF is last");
                }
                _ => out.last_mut().expect("pair outside a section").1.push(p[i].clone()),
            }
            assert!((0..=1).contains(&depth), "sections are not nested");
            i += 1;
        }
        assert_eq!(depth, 0);
        assert_eq!(p.last().map(|x| x.1.as_str()), Some("EOF"));
        out
    }

    fn entities_of(p: &[(i32, String)]) -> Vec<(String, Vec<(i32, String)>)> {
        let secs = sections(p);
        let ents = &secs.iter().find(|s| s.0 == "ENTITIES").expect("ENTITIES").1;
        let mut out: Vec<(String, Vec<(i32, String)>)> = vec![];
        for (c, v) in ents {
            if *c == 0 {
                out.push((v.clone(), vec![]));
            } else {
                out.last_mut().unwrap().1.push((*c, v.clone()));
            }
        }
        out
    }

    fn vals(e: &[(i32, String)], code: i32) -> Vec<f64> {
        e.iter().filter(|(c, _)| *c == code).map(|(_, v)| v.parse().unwrap()).collect()
    }

    fn doc_with_shapes() -> Document {
        let mut doc = Document::new(72.0, 72.0);
        doc.pages[0].layers[0].name = "Cut: lines".into();
        let mut st = Style::default();
        st.fill = Fill::None;
        st.outline.color = Some(Color::rgb(255, 0, 0));
        let sq = vector_core::path::polyline(&[Point::new(0.0, 0.0), Point::new(36.0, 0.0), Point::new(36.0, 36.0), Point::new(0.0, 36.0)], true);
        let id = doc.new_id();
        doc.pages[0].layers[0].objects.push(Object::new(id, ObjKind::Path(sq), Affine::IDENTITY, st.clone()));
        let e = EllipseShape { w: 36.0, h: 36.0, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false };
        st.outline.color = Some(Color::rgb(0, 0, 255));
        let id = doc.new_id();
        let lid = doc.new_id();
        let mut l2 = vector_core::Layer::new(lid, "Circles");
        l2.objects.push(Object::new(id, ObjKind::Ellipse(e), Affine::translate((36.0, 36.0)), st));
        doc.pages[0].layers.push(l2);
        doc
    }

    #[test]
    fn r12_parses_back() {
        let doc = doc_with_shapes();
        let s = to_dxf(&doc, &DxfOptions { units: DxfUnits::Inches, ..Default::default() });
        let p = pairs(&s);
        let secs = sections(&p);
        let names: Vec<&str> = secs.iter().map(|s| s.0.as_str()).collect();
        assert_eq!(names, ["HEADER", "TABLES", "BLOCKS", "ENTITIES"]);
        assert!(secs[0].1.iter().any(|(c, v)| *c == 1 && v == "AC1009"));
        // the layers, with the name sanitized
        let layers: Vec<&str> = secs[1].1.windows(2).filter(|w| w[0] == (0, "LAYER".into())).map(|w| w[1].1.as_str()).collect();
        assert_eq!(layers, ["0", "Cut_ lines", "Circles"]);
        let ents = entities_of(&p);
        // the square: a closed POLYLINE with four vertices, red (ACI 1), in inches, y up
        assert_eq!(ents[0].0, "POLYLINE");
        assert!(ents[0].1.contains(&(8, "Cut_ lines".into())));
        assert!(ents[0].1.contains(&(62, "1".into())));
        assert!(ents[0].1.contains(&(70, "1".into())));
        let verts: Vec<_> = ents.iter().skip(1).take_while(|e| e.0 == "VERTEX").collect();
        assert_eq!(verts.len(), 4);
        let xy: Vec<(f64, f64)> = verts.iter().map(|v| (vals(&v.1, 10)[0], vals(&v.1, 20)[0])).collect();
        assert_eq!(xy, [(0.0, 1.0), (0.5, 1.0), (0.5, 0.5), (0.0, 0.5)]);
        assert_eq!(ents[5].0, "SEQEND");
        // the circle: flattened, blue (ACI 5), on its layer
        assert_eq!(ents[6].0, "POLYLINE");
        assert!(ents[6].1.contains(&(8, "Circles".into())));
        assert!(ents[6].1.contains(&(62, "5".into())));
        assert!(!s.contains("SPLINE") && !s.contains("LWPOLYLINE"));
    }

    #[test]
    fn r2000_keeps_curves_as_splines() {
        let doc = doc_with_shapes();
        let s = to_dxf(&doc, &DxfOptions { version: DxfVersion::R2000, splines: true, units: DxfUnits::Millimeters, ..Default::default() });
        let p = pairs(&s);
        let secs = sections(&p);
        let names: Vec<&str> = secs.iter().map(|s| s.0.as_str()).collect();
        assert_eq!(names, ["HEADER", "CLASSES", "TABLES", "BLOCKS", "ENTITIES", "OBJECTS"]);
        assert!(secs[0].1.iter().any(|(c, v)| *c == 1 && v == "AC1015"));
        let ents = entities_of(&p);
        let kinds: Vec<&str> = ents.iter().map(|e| e.0.as_str()).collect();
        assert_eq!(kinds, ["LWPOLYLINE", "SPLINE"]);
        // the square in millimeters, closed
        let lw = &ents[0].1;
        assert_eq!(vals(lw, 90), [4.0]);
        assert_eq!(vals(lw, 10), [0.0, 12.7, 12.7, 0.0]);
        assert!(lw.contains(&(420, 0xff0000u32.to_string())));
        // the circle: degree 3, knots = controls + 4, every control on or near the circle box
        let sp = &ents[1].1;
        assert_eq!(vals(sp, 71), [3.0]);
        let nk = vals(sp, 72)[0] as usize;
        let nc = vals(sp, 73)[0] as usize;
        assert_eq!(nk, nc + 4);
        assert_eq!(vals(sp, 40).len(), nk);
        assert_eq!(vals(sp, 10).len(), nc);
        assert_eq!((nc - 1) % 3, 0);
        // every entity has a unique handle below the seed
        let seed = u32::from_str_radix(&secs[0].1.iter().skip_while(|x| x.1 != "$HANDSEED").nth(1).unwrap().1, 16).unwrap();
        let mut handles: Vec<u32> = secs.iter().skip(1).flat_map(|s| s.1.iter()).filter(|(c, _)| *c == 5).map(|(_, v)| u32::from_str_radix(v, 16).unwrap()).collect();
        assert!(handles.iter().all(|h| *h < seed));
        let n = handles.len();
        handles.sort_unstable();
        handles.dedup();
        assert_eq!(handles.len(), n, "handles are unique");
    }

    #[test]
    fn aci_table() {
        assert_eq!(aci_rgb(10), [255, 0, 0]);
        assert_eq!(aci_rgb(11), [255, 127, 127]);
        assert_eq!(aci_rgb(13), [204, 102, 102]);
        assert_eq!(aci_rgb(20), [255, 63, 0]);
        assert_eq!(nearest_aci(Color::rgb(255, 0, 0)), 1);
        assert_eq!(nearest_aci(Color::BLACK), 7);
        assert_eq!(nearest_aci(Color::rgb(0, 0, 250)), 5);
    }
}
