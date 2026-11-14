//! Sign-making output: the HPGL Export and DXF Export dialogs that follow File ▸ Export, export
//! through device plugins (see [`crate::scripting::devices`]) and Object ▸ Create Cut Contour.

use super::{combo, note, row, tabs, window};
use crate::app::AnyVectorApp;
use crate::scripting::devices::DeviceFormat;
use egui::vec2;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use vector_core::color::NamedColor;
use vector_core::{Color, Document, ObjId};
use vector_io::cutter::FillHandling;
use vector_io::dxf::{DxfOptions, DxfUnits, DxfVersion};
use vector_io::hpgl::{HpglOptions, HpglOrigin};

/// The last HPGL, DXF and cut contour settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CutterSettings {
    pub hpgl_units: f64,
    /// Curve resolution in inches.
    pub hpgl_resolution: f64,
    /// Scale in percent.
    pub hpgl_scale: f64,
    pub hpgl_origin: HpglOrigin,
    pub hpgl_fills: FillHandling,
    pub hpgl_velocity: Option<u32>,
    /// Pen per color.
    pub hpgl_pens: Vec<(Color, u16)>,
    pub dxf_version: DxfVersion,
    pub dxf_units: DxfUnits,
    pub dxf_splines: bool,
    pub dxf_true_color: bool,
    pub dxf_fills: FillHandling,
    /// Create Cut Contour: the offset in millimeters and whether holes are dropped.
    pub contour_offset_mm: f64,
    pub contour_outer_only: bool,
}

impl Default for CutterSettings {
    fn default() -> Self {
        CutterSettings {
            hpgl_units: 1016.0,
            hpgl_resolution: 0.004,
            hpgl_scale: 100.0,
            hpgl_origin: HpglOrigin::PageBottomLeft,
            hpgl_fills: FillHandling::OutlinesOnly,
            hpgl_velocity: None,
            hpgl_pens: vec![],
            dxf_version: DxfVersion::R12,
            dxf_units: DxfUnits::Millimeters,
            dxf_splines: false,
            dxf_true_color: true,
            dxf_fills: FillHandling::FilledAsOutline,
            contour_offset_mm: 1.0,
            contour_outer_only: true,
        }
    }
}

impl CutterSettings {
    fn hpgl(&self, page: usize, only: Option<Vec<ObjId>>) -> HpglOptions {
        HpglOptions {
            page,
            only,
            units_per_inch: self.hpgl_units.max(1.0),
            tolerance: (self.hpgl_resolution * 72.0).max(0.001),
            scale: (self.hpgl_scale / 100.0).max(0.0001),
            origin: self.hpgl_origin,
            fills: self.hpgl_fills,
            pens: self.hpgl_pens.clone(),
            velocity: self.hpgl_velocity,
        }
    }
    fn dxf(&self, page: usize, only: Option<Vec<ObjId>>) -> DxfOptions {
        DxfOptions { page, only, version: self.dxf_version, units: self.dxf_units, splines: self.dxf_splines, tolerance: 0.1, true_color: self.dxf_true_color, fills: self.dxf_fills }
    }
}

/// The export of File ▸ Export: the file, the pages and the selection (top-level objects).
#[derive(Clone)]
pub struct Target {
    pub path: PathBuf,
    pub pages: Vec<usize>,
    pub only: Option<Vec<ObjId>>,
}

impl Target {
    /// The export target for the current document: the selection (on the current page) when
    /// `selection` is set, otherwise `pages`.
    pub fn new(app: &AnyVectorApp, path: PathBuf, pages: Vec<usize>, selection: bool) -> Target {
        let only = app.doc().filter(|_| selection).map(|d| {
            let mut v: Vec<ObjId> = d.selection.iter().filter_map(|id| d.doc.top_level_of(*id)).collect();
            v.dedup();
            v
        });
        let pages = match (&only, app.doc()) {
            (Some(_), Some(d)) => vec![d.page],
            _ => pages,
        };
        Target { path, pages, only }
    }

    /// The file for a page: numbered when several pages are exported.
    fn file(&self, page: usize) -> PathBuf {
        if self.pages.len() <= 1 {
            return self.path.clone();
        }
        let stem = self.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ext = self.path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
        self.path.with_file_name(format!("{stem}-{}.{ext}", page + 1))
    }
}

pub struct HpglState {
    pub target: Target,
    pub s: CutterSettings,
    pub tab: usize,
    /// The colors of the export with their pens, and the fill handling they were listed for.
    pub pens: Vec<(Color, u16)>,
    pub pens_fills: FillHandling,
}

pub struct DxfState {
    pub target: Target,
    pub s: CutterSettings,
}

#[derive(Default)]
pub struct CutterDialogs {
    pub hpgl: Option<HpglState>,
    pub dxf: Option<DxfState>,
    /// Create Cut Contour: (offset in mm, outer edge only).
    pub contour: Option<(f64, bool)>,
}

impl CutterDialogs {
    pub fn any_open(&self) -> bool {
        self.hpgl.is_some() || self.dxf.is_some() || self.contour.is_some()
    }
}

pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    if app.dialogs.cutter.hpgl.is_some() {
        show_hpgl(app, ctx);
    }
    if app.dialogs.cutter.dxf.is_some() {
        show_dxf(app, ctx);
    }
    if app.dialogs.cutter.contour.is_some() {
        show_contour(app, ctx);
    }
}

fn report(app: &mut AnyVectorApp, res: Result<(), String>, path: &Path) {
    match res {
        Ok(()) => app.flash(format!("Exported {}", path.display())),
        Err(e) => app.dialogs.message_box = Some(("Export".into(), format!("Export failed:\n{e}"))),
    }
}

// ------------------------------------------------------------------------------------------
// HPGL Export
// ------------------------------------------------------------------------------------------

/// Continue File ▸ Export to a `.plt` file: the HPGL Export dialog, or the export itself.
pub fn start_hpgl(app: &mut AnyVectorApp, target: Target, no_dialog: bool) {
    let s = app.settings.cutter.clone();
    if no_dialog {
        run_hpgl(app, &target, &s);
        return;
    }
    let pens = app.doc().map(|d| vector_io::hpgl::used_pens(&d.doc, &s.hpgl(target.pages[0], target.only.clone()))).unwrap_or_default();
    let pens_fills = s.hpgl_fills;
    app.dialogs.cutter.hpgl = Some(HpglState { target, s, tab: 0, pens, pens_fills });
}

fn run_hpgl(app: &mut AnyVectorApp, t: &Target, s: &CutterSettings) {
    let Some(d) = app.doc() else { return };
    let res = t.pages.iter().try_for_each(|p| vector_io::hpgl::export(&d.doc, &t.file(*p), &s.hpgl(*p, t.only.clone())).map_err(|e| e.to_string()));
    report(app, res, &t.path);
}

fn show_hpgl(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.cutter.hpgl.take() else { return };
    let mut result = (false, false);
    let ((), dismissed) = window(ctx, "hpgl_export", "HPGL Export", vec2(470.0, 400.0), |ui| {
        ui.add_space(6.0);
        tabs(ui, &mut st.tab, &["Pen".to_string(), "Page".to_string(), "Advanced".to_string()]);
        let s = &mut st.s;
        match st.tab {
            0 => {
                note(ui, "Pen selection: each outline color is plotted or cut with its own pen.");
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    if st.pens.is_empty() {
                        note(ui, "Nothing to export: no object has an outline.");
                    }
                    for (c, pen) in st.pens.iter_mut() {
                        ui.horizontal(|ui| {
                            crate::widgets::color_well(ui, Some(*c), vec2(36.0, 18.0));
                            ui.add_sized(vec2(150.0, 20.0), egui::Label::new(c.describe()).halign(egui::Align::Min));
                            ui.label("Pen:");
                            ui.add(egui::DragValue::new(pen).range(1..=256));
                        });
                    }
                });
                ui.add_space(6.0);
                row(ui, "Pen velocity:", 120.0, |ui| {
                    let mut on = s.hpgl_velocity.is_some();
                    if ui.checkbox(&mut on, "").changed() {
                        s.hpgl_velocity = on.then_some(20);
                    }
                    if let Some(v) = s.hpgl_velocity.as_mut() {
                        ui.add(egui::DragValue::new(v).range(1..=128).suffix(" cm/s"));
                    } else {
                        ui.add_enabled(false, egui::Label::new("Plotter default"));
                    }
                });
            }
            1 => {
                let mut o = HpglOrigin::ALL.iter().position(|x| *x == s.hpgl_origin).unwrap_or(0);
                row(ui, "Plotter origin:", 120.0, |ui| {
                    let names: Vec<&str> = HpglOrigin::ALL.iter().map(|x| x.name()).collect();
                    if combo(ui, "hpgl_origin", &mut o, &names, 200.0) {
                        s.hpgl_origin = HpglOrigin::ALL[o];
                    }
                });
                row(ui, "Scale:", 120.0, |ui| ui.add(egui::DragValue::new(&mut s.hpgl_scale).range(1.0..=10000.0).speed(1.0).suffix(" %")));
                note(ui, "Plotters count from the bottom left with y up; the drawing is not mirrored.");
            }
            _ => {
                row(ui, "Plotter units:", 120.0, |ui| ui.add(egui::DragValue::new(&mut s.hpgl_units).range(1.0..=10000.0).speed(1.0).suffix(" per inch")));
                row(ui, "Curve resolution:", 120.0, |ui| ui.add(egui::DragValue::new(&mut s.hpgl_resolution).range(0.0005..=0.1).speed(0.0005).max_decimals(4).suffix(" in")));
                let mut f = (s.hpgl_fills == FillHandling::FilledAsOutline) as usize;
                row(ui, "Fill type:", 120.0, |ui| {
                    if combo(ui, "hpgl_fill", &mut f, &["Outlines only", "Filled objects as outlines"], 200.0) {
                        s.hpgl_fills = if f == 1 { FillHandling::FilledAsOutline } else { FillHandling::OutlinesOnly };
                    }
                });
                note(ui, "Text is exported as curves. Bitmaps are not exported.");
            }
        }
        result = super::ok_cancel_row(ui, "OK", true);
    });
    // the pen table follows the fill choice (filled objects bring their fill colors)
    if st.s.hpgl_fills != st.pens_fills {
        if let Some(d) = app.doc() {
            let mut s = st.s.clone();
            s.hpgl_pens = st.pens.clone();
            st.pens = vector_io::hpgl::used_pens(&d.doc, &s.hpgl(st.target.pages[0], st.target.only.clone()));
        }
        st.pens_fills = st.s.hpgl_fills;
    }
    if result.0 {
        // the pens of these colors, then earlier choices for other colors
        let mut pens = st.pens.clone();
        for (c, p) in &app.settings.cutter.hpgl_pens {
            if !pens.iter().any(|x| x.0 == *c) {
                pens.push((*c, *p));
            }
        }
        pens.truncate(256);
        st.s.hpgl_pens = pens;
        app.settings.cutter = st.s.clone();
        run_hpgl(app, &st.target, &st.s);
    } else if !(result.1 || dismissed) {
        app.dialogs.cutter.hpgl = Some(st);
    }
}

// ------------------------------------------------------------------------------------------
// DXF Export
// ------------------------------------------------------------------------------------------

/// Continue File ▸ Export to a `.dxf` file: the DXF Export dialog, or the export itself.
pub fn start_dxf(app: &mut AnyVectorApp, target: Target, no_dialog: bool) {
    let s = app.settings.cutter.clone();
    if no_dialog {
        run_dxf(app, &target, &s);
    } else {
        app.dialogs.cutter.dxf = Some(DxfState { target, s });
    }
}

fn run_dxf(app: &mut AnyVectorApp, t: &Target, s: &CutterSettings) {
    let Some(d) = app.doc() else { return };
    let res = t.pages.iter().try_for_each(|p| vector_io::dxf::export(&d.doc, &t.file(*p), &s.dxf(*p, t.only.clone())).map_err(|e| e.to_string()));
    report(app, res, &t.path);
}

fn show_dxf(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.cutter.dxf.take() else { return };
    let mut result = (false, false);
    let ((), dismissed) = window(ctx, "dxf_export", "DXF Export", vec2(440.0, 290.0), |ui| {
        ui.add_space(10.0);
        let s = &mut st.s;
        let mut v = DxfVersion::ALL.iter().position(|x| *x == s.dxf_version).unwrap_or(0);
        row(ui, "Version:", 120.0, |ui| {
            let names: Vec<&str> = DxfVersion::ALL.iter().map(|x| x.name()).collect();
            if combo(ui, "dxf_version", &mut v, &names, 220.0) {
                s.dxf_version = DxfVersion::ALL[v];
            }
        });
        let mut u = DxfUnits::ALL.iter().position(|x| *x == s.dxf_units).unwrap_or(1);
        row(ui, "Export units:", 120.0, |ui| {
            let names: Vec<&str> = DxfUnits::ALL.iter().map(|x| x.name()).collect();
            if combo(ui, "dxf_units", &mut u, &names, 220.0) {
                s.dxf_units = DxfUnits::ALL[u];
            }
        });
        let r2000 = s.dxf_version == DxfVersion::R2000;
        let mut c = (r2000 && s.dxf_splines) as usize;
        row(ui, "Export curves as:", 120.0, |ui| {
            ui.add_enabled_ui(r2000, |ui| {
                if combo(ui, "dxf_curves", &mut c, &["Polylines", "Splines"], 220.0) {
                    s.dxf_splines = c == 1;
                }
            });
        });
        let mut f = (s.dxf_fills == FillHandling::FilledAsOutline) as usize;
        row(ui, "Fill type:", 120.0, |ui| {
            if combo(ui, "dxf_fill", &mut f, &["Outlines only", "Filled objects as outlines"], 220.0) {
                s.dxf_fills = if f == 1 { FillHandling::FilledAsOutline } else { FillHandling::OutlinesOnly };
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(124.0);
            ui.add_enabled(r2000, egui::Checkbox::new(&mut s.dxf_true_color, "True color"));
        });
        note(ui, "Layers are kept. Colors map to the AutoCAD Color Index (and true color in AutoCAD 2000). Text is exported as curves.");
        result = super::ok_cancel_row(ui, "OK", true);
    });
    if result.0 {
        app.settings.cutter = st.s.clone();
        run_dxf(app, &st.target, &st.s);
    } else if !(result.1 || dismissed) {
        app.dialogs.cutter.dxf = Some(st);
    }
}

// ------------------------------------------------------------------------------------------
// Device plugins
// ------------------------------------------------------------------------------------------

/// Export with a format a device plugin registered.
pub fn run_device(app: &mut AnyVectorApp, fmt: &DeviceFormat, target: &Target) {
    let Some(d) = app.doc() else { return };
    let res = target.pages.iter().try_for_each(|p| crate::scripting::devices::export(fmt, &d.doc, *p, target.only.as_deref(), &target.file(*p)));
    report(app, res, &target.path);
}

// ------------------------------------------------------------------------------------------
// Object ▸ Create Cut Contour
// ------------------------------------------------------------------------------------------

/// Open Create Cut Contour for the selection.
pub fn start_contour(app: &mut AnyVectorApp) {
    if !app.has_selection() {
        return;
    }
    let s = &app.settings.cutter;
    app.dialogs.cutter.contour = Some((s.contour_offset_mm, s.contour_outer_only));
}

fn show_contour(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some((mut mm, mut outer)) = app.dialogs.cutter.contour.take() else { return };
    let mut result = (false, false);
    let ((), dismissed) = window(ctx, "cut_contour", "Create Cut Contour", vec2(400.0, 210.0), |ui| {
        ui.add_space(10.0);
        row(ui, "Offset:", 110.0, |ui| ui.add(egui::DragValue::new(&mut mm).range(-50.0..=100.0).speed(0.1).max_decimals(2).suffix(" mm")));
        ui.horizontal(|ui| {
            ui.add_space(114.0);
            ui.checkbox(&mut outer, "Outer edge only (no holes)");
        });
        ui.add_space(4.0);
        note(ui, "Adds a hairline in the CutContour spot color around the selected objects, for print-and-cut. It prints as a separation named CutContour in PDF files.");
        result = super::ok_cancel_row(ui, "OK", app.has_selection());
    });
    if result.0 {
        app.settings.cutter.contour_offset_mm = mm;
        app.settings.cutter.contour_outer_only = outer;
        if !create_cut_contour(app, mm * 72.0 / 25.4, outer) {
            app.dialogs.message_box = Some(("Create Cut Contour".into(), "The selected objects have no outline or fill to cut around.".into()));
        }
    } else if !(result.1 || dismissed) {
        app.dialogs.cutter.contour = Some((mm, outer));
    }
}

/// Add the cut contour of the selection above it and select it; false when there is nothing
/// to cut around.
pub fn create_cut_contour(app: &mut AnyVectorApp, offset: f64, outer_only: bool) -> bool {
    let Some(d) = app.doc_mut() else { return false };
    let objs: Vec<vector_core::Object> = d.selection.iter().filter_map(|id| d.doc.find(*id).cloned()).collect();
    let path = vector_io::cutter::contour_for_cutting(&d.doc, &objs.iter().collect::<Vec<_>>(), offset, outer_only);
    if path.elements().is_empty() {
        return false;
    }
    d.modify("Create Cut Contour", |doc, sel| add_cut_contour(doc, sel, &path));
    true
}

/// Put a CutContour hairline with this path above the topmost selected object and select it.
fn add_cut_contour(doc: &mut Document, sel: &mut Vec<ObjId>, path: &kurbo::BezPath) {
    use vector_core::style::{LineJoin, Outline};
    let at = sel.iter().filter_map(|id| doc.top_level_of(*id)).filter_map(|id| doc.locate(id)).max_by_key(|(p, l, i)| (*p, *l, *i));
    let Some((p, l, i)) = at else { return };
    let mut style = vector_core::Style { fill: vector_core::Fill::None, ..Default::default() };
    style.outline = Outline { width: 0.0, color: Some(vector_core::color::CUT_CONTOUR), join: LineJoin::Round, ..Default::default() };
    let id = doc.new_id();
    let mut o = vector_core::Object::new(id, vector_core::ObjKind::Path(vector_core::path::NodePath::from_bez(path)), kurbo::Affine::IDENTITY, style);
    o.name = vector_core::color::CUT_CONTOUR_NAME.into();
    doc.pages[p].layers[l].objects.insert(i + 1, o);
    if !doc.palette.iter().any(|c| c.name == vector_core::color::CUT_CONTOUR_NAME) {
        doc.palette.push(NamedColor::cut_contour());
    }
    *sel = vec![id];
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{Affine, Point};
    use vector_core::object::{ObjKind, Object};
    use vector_core::Style;

    #[test]
    fn cut_contour_goes_above_the_selection_in_the_spot_color() {
        let mut doc = Document::new(200.0, 200.0);
        let a = doc.new_id();
        let sq = vector_core::path::polyline(&[Point::new(10.0, 10.0), Point::new(50.0, 10.0), Point::new(50.0, 50.0), Point::new(10.0, 50.0)], true);
        doc.pages[0].layers[0].objects.push(Object::new(a, ObjKind::Path(sq), Affine::IDENTITY, Style::filled(Color::RED)));
        let b = doc.new_id();
        let line = vector_core::path::polyline(&[Point::new(100.0, 100.0), Point::new(150.0, 100.0)], false);
        doc.pages[0].layers[0].objects.push(Object::new(b, ObjKind::Path(line), Affine::IDENTITY, Style::default()));
        let objs = [doc.find(a).unwrap().clone()];
        let path = vector_io::cutter::contour_for_cutting(&doc, &objs.iter().collect::<Vec<_>>(), 2.0, true);
        let mut sel = vec![a];
        add_cut_contour(&mut doc, &mut sel, &path);
        let objs = &doc.pages[0].layers[0].objects;
        assert_eq!(objs.len(), 3);
        assert_eq!(objs[1].id, sel[0], "right above the selected object");
        assert_eq!(objs[1].name, "CutContour");
        assert_eq!(objs[1].style.outline.color, Some(vector_core::color::CUT_CONTOUR));
        assert_eq!(objs[1].style.outline.width, 0.0, "hairline");
        assert!(objs[1].style.fill.is_none());
        let bb = kurbo::Shape::bounding_box(&objs[1].world_path());
        assert!((bb.x0 - 8.0).abs() < 0.2 && (bb.x1 - 52.0).abs() < 0.2, "{bb:?}");
        assert!(doc.palette.iter().any(|c| c.name == "CutContour" && c.spot));
    }
}
