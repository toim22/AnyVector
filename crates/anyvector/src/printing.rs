//! Printing (File ▸ Print, Print Preview, Print Merge): the print settings and print styles,
//! where each page lands on the paper (Layout tab), the printers' marks (Prepress tab), the
//! preflight checks, and the print job itself, sent to a Windows printer through GDI, or on Linux
//! to CUPS as a PDF (`cups`).
//!
//! Pages are rendered by AnyVector's own renderer at the printer resolution (capped at 600 dpi, or
//! at the "Print as bitmap" resolution) and sent to the printer in bands, so any printer driver
//! works, including "Microsoft Print to PDF". On Linux the same bands make the page images of the
//! PDF, so what prints (marks, tiles, mirror, invert, grayscale) is the same on both systems.

use kurbo::{Affine, BezPath, Point, Rect, Shape};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use vector_core::{Color, Document, ObjId, ObjKind, Object, Style};

// ------------------------------------------------------------------------------------------
// settings
// ------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum Orientation {
    /// Each page prints in its own orientation.
    #[default]
    Match,
    Portrait,
    Landscape,
}

impl Orientation {
    pub const ALL: [Orientation; 3] = [Orientation::Match, Orientation::Portrait, Orientation::Landscape];
    pub fn name(self) -> &'static str {
        match self {
            Orientation::Match => "Match orientation",
            Orientation::Portrait => "Portrait",
            Orientation::Landscape => "Landscape",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum Range {
    #[default]
    CurrentDocument,
    Documents,
    CurrentPage,
    Selection,
    Pages,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum Parity {
    #[default]
    Both,
    Even,
    Odd,
}

impl Parity {
    pub const ALL: [Parity; 3] = [Parity::Both, Parity::Even, Parity::Odd];
    pub fn name(self) -> &'static str {
        match self {
            Parity::Both => "Even & Odd",
            Parity::Even => "Even",
            Parity::Odd => "Odd",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum ToFile {
    #[default]
    Single,
    PerPage,
}

impl ToFile {
    pub const ALL: [ToFile; 2] = [ToFile::Single, ToFile::PerPage];
    pub fn name(self) -> &'static str {
        match self {
            ToFile::Single => "Single file",
            ToFile::PerPage => "Pages to separate files",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum Placement {
    #[default]
    AsInDocument,
    FitToPage,
    Reposition,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum OutputColor {
    #[default]
    Rgb,
    Grayscale,
}

impl OutputColor {
    pub const ALL: [OutputColor; 2] = [OutputColor::Rgb, OutputColor::Grayscale];
    pub fn name(self) -> &'static str {
        match self {
            OutputColor::Rgb => "RGB",
            OutputColor::Grayscale => "Grayscale",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum Overprints {
    #[default]
    Ignore,
    Simulate,
}

impl Overprints {
    pub const ALL: [Overprints; 2] = [Overprints::Ignore, Overprints::Simulate];
    pub fn name(self) -> &'static str {
        match self {
            Overprints::Ignore => "Ignore",
            Overprints::Simulate => "Simulate",
        }
    }
}

/// Where Reposition puts the artwork on the printable area (3 × 3, row by row).
pub const ANCHORS: [&str; 9] = ["Top left", "Top center", "Top right", "Middle left", "Center", "Middle right", "Bottom left", "Bottom center", "Bottom right"];

/// Everything the Print dialog sets, except the printer itself. A print style is this, saved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrintSettings {
    // General
    pub orientation: Orientation,
    pub copies: u32,
    pub collate: bool,
    pub range: Range,
    pub pages: String,
    pub parity: Parity,
    pub as_bitmap: bool,
    pub bitmap_dpi: u32,
    pub to_file: bool,
    pub file_mode: ToFile,
    // Layout
    pub placement: Placement,
    pub anchor: usize,
    /// Reposition: offset of the artwork from the anchor, in points.
    pub offset: (f64, f64),
    /// Reposition: scale in percent.
    pub scale: f64,
    pub tiled: bool,
    /// Tile overlap in points.
    pub tile_overlap: f64,
    pub tiling_marks: bool,
    /// How far artwork may run past the page edge, in points.
    pub bleed_limit: f64,
    // Prepress
    pub crop_marks: bool,
    pub exterior_only: bool,
    pub registration_marks: bool,
    pub calibration_bar: bool,
    pub densitometer: bool,
    pub page_numbers: bool,
    pub file_info: bool,
    pub marks_to_objects: bool,
    pub invert: bool,
    pub mirror: bool,
    // Color
    pub output: OutputColor,
    pub color_proof: bool,
    pub intent: vector_core::document::RenderingIntent,
    // Composite
    pub overprints: Overprints,
}

impl Default for PrintSettings {
    fn default() -> Self {
        PrintSettings {
            orientation: Orientation::Match,
            copies: 1,
            collate: true,
            range: Range::CurrentDocument,
            pages: String::new(),
            parity: Parity::Both,
            as_bitmap: false,
            bitmap_dpi: 300,
            to_file: false,
            file_mode: ToFile::Single,
            placement: Placement::AsInDocument,
            anchor: 4,
            offset: (0.0, 0.0),
            scale: 100.0,
            tiled: false,
            tile_overlap: 0.0,
            tiling_marks: true,
            bleed_limit: 9.0,
            crop_marks: false,
            exterior_only: false,
            registration_marks: false,
            calibration_bar: false,
            densitometer: false,
            page_numbers: false,
            file_info: false,
            marks_to_objects: false,
            invert: false,
            mirror: false,
            output: OutputColor::Rgb,
            color_proof: false,
            intent: Default::default(),
            overprints: Overprints::Ignore,
        }
    }
}

impl PrintSettings {
    /// Any mark printed outside the page, which needs room on the paper.
    pub fn has_marks(&self) -> bool {
        self.crop_marks || self.registration_marks || self.calibration_bar || self.densitometer || self.page_numbers || self.file_info
    }
}

/// The name of the built-in print style.
pub const DEFAULT_STYLE: &str = "Default";

pub fn styles_dir() -> PathBuf {
    crate::settings::config_dir().join("Print Styles")
}

/// The saved print styles, by name.
pub fn style_names() -> Vec<String> {
    style_names_in(&styles_dir())
}

/// The print styles (`.prs` files, the extension in any case) of `dir`, by name.
fn style_names_in(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| vector_io::paths::has_extension(p, &["prs"])).filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().to_string())).collect())
        .unwrap_or_default();
    v.sort();
    v.dedup();
    v
}

pub fn load_style(path: &std::path::Path) -> Option<PrintSettings> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

pub fn style_path(name: &str) -> PathBuf {
    style_path_in(&styles_dir(), name)
}

/// The file of the print style `name` in `dir`: `name.prs`, else the style file whose name
/// differs only in case, as Windows finds it (Linux tells `Poster.PRS` from `Poster.prs`): one
/// with the same name first. A new style is `name.prs`.
fn style_path_in(dir: &std::path::Path, name: &str) -> PathBuf {
    let exact = dir.join(format!("{name}.prs"));
    if exact.exists() {
        return exact;
    }
    let stem = |p: &PathBuf| p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut hits: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| vector_io::paths::has_extension(p, &["prs"]) && stem(p).to_lowercase() == name.to_lowercase()).collect())
        .unwrap_or_default();
    hits.sort_by_key(|p| (stem(p) != name, p.clone()));
    hits.into_iter().next().unwrap_or(exact)
}

pub fn save_style(path: &std::path::Path, st: &PrintSettings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(st).unwrap_or_default())
}

// ------------------------------------------------------------------------------------------
// paper and layout
// ------------------------------------------------------------------------------------------

/// A sheet of the printer's paper, in points, with the area the printer can print on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper {
    pub width: f64,
    pub height: f64,
    pub printable: Rect,
    /// Printer resolution (dots per inch).
    pub dpi: (f64, f64),
}

impl Paper {
    /// Letter, with a quarter-inch margin: used when no printer can be asked.
    pub fn fallback(landscape: bool) -> Paper {
        let (w, h) = if landscape { (792.0, 612.0) } else { (612.0, 792.0) };
        Paper { width: w, height: h, printable: Rect::new(18.0, 18.0, w - 18.0, h - 18.0), dpi: (600.0, 600.0) }
    }
    pub fn rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width, self.height)
    }
}

/// One printed sheet: a page (or one tile of it) placed on the paper.
#[derive(Clone, Debug)]
pub struct Sheet {
    /// Index into the job's documents.
    pub doc: usize,
    pub page: usize,
    pub landscape: bool,
    /// Document points → paper points.
    pub place: Affine,
    /// What may print, in paper points (the page plus the bleed limit).
    pub clip: Rect,
    /// The page rectangle on the paper.
    pub page_rect: Rect,
    /// Tile (column, row) when the page is tiled.
    pub tile: Option<(usize, usize)>,
}

/// Room left around the page for the printers' marks.
pub const MARK_ROOM: f64 = 36.0;

fn landscape_for(st: &PrintSettings, w: f64, h: f64) -> bool {
    match st.orientation {
        Orientation::Match => w > h,
        Orientation::Portrait => false,
        Orientation::Landscape => true,
    }
}

/// The sheets for these pages: `pages` lists (document, page, page size, area), where `area`
/// is the part of the page that prints (the selection bounds when printing a selection).
pub fn layout(st: &PrintSettings, pages: &[(usize, usize, f64, f64, Rect)], paper: &dyn Fn(bool) -> Paper) -> Vec<(Sheet, Paper)> {
    let mut out = vec![];
    for &(doc, page, w, h, area) in pages {
        let landscape = landscape_for(st, w, h);
        let pp = paper(landscape);
        let pr = pp.printable;
        let room = if st.has_marks() && !st.marks_to_objects { MARK_ROOM } else { 0.0 };
        let avail = pr.inset(-room);
        // the part of the document that is placed: the page, or the selection with marks to objects
        let art = if st.marks_to_objects { area } else { Rect::new(0.0, 0.0, w, h) };
        let (scale, origin) = match st.placement {
            Placement::AsInDocument => (1.0, Point::new((pp.width - art.width()) / 2.0, (pp.height - art.height()) / 2.0)),
            Placement::FitToPage => {
                let k = (avail.width() / art.width().max(1e-6)).min(avail.height() / art.height().max(1e-6)).max(0.01);
                (k, Point::new(avail.center().x - art.width() * k / 2.0, avail.center().y - art.height() * k / 2.0))
            }
            Placement::Reposition => {
                let k = (st.scale / 100.0).max(0.01);
                let (aw, ah) = (art.width() * k, art.height() * k);
                let col = st.anchor % 3;
                let row = st.anchor / 3;
                let x = match col {
                    0 => avail.x0,
                    1 => avail.center().x - aw / 2.0,
                    _ => avail.x1 - aw,
                };
                let y = match row {
                    0 => avail.y0,
                    1 => avail.center().y - ah / 2.0,
                    _ => avail.y1 - ah,
                };
                (k, Point::new(x + st.offset.0, y + st.offset.1))
            }
        };
        let place = Affine::translate(origin.to_vec2()) * Affine::scale(scale) * Affine::translate(-art.origin().to_vec2());
        let page_rect = place.transform_rect_bbox(Rect::new(0.0, 0.0, w, h));
        let clip = page_rect.inflate(st.bleed_limit, st.bleed_limit);
        let placed = place.transform_rect_bbox(art);
        if st.tiled && (placed.width() > pr.width() + 0.5 || placed.height() > pr.height() + 0.5) {
            // tiles the size of the printable area, overlapping by the tile overlap
            let step_x = (pr.width() - st.tile_overlap).max(36.0);
            let step_y = (pr.height() - st.tile_overlap).max(36.0);
            let cols = ((placed.width() - st.tile_overlap) / step_x).ceil().max(1.0) as usize;
            let rows = ((placed.height() - st.tile_overlap) / step_y).ceil().max(1.0) as usize;
            for r in 0..rows {
                for c in 0..cols {
                    let shift = Affine::translate((pr.x0 - placed.x0 - c as f64 * step_x, pr.y0 - placed.y0 - r as f64 * step_y));
                    out.push((Sheet { doc, page, landscape, place: shift * place, clip: shift.transform_rect_bbox(clip), page_rect: shift.transform_rect_bbox(page_rect), tile: Some((c, r)) }, pp));
                }
            }
        } else {
            out.push((Sheet { doc, page, landscape, place, clip, page_rect, tile: None }, pp));
        }
    }
    out
}

// ------------------------------------------------------------------------------------------
// printers' marks
// ------------------------------------------------------------------------------------------

fn stroke_path(bp: BezPath, width: f64, color: Color) -> Object {
    let mut style = Style::default();
    style.outline.width = width;
    style.outline.color = Some(color);
    Object::new(0, ObjKind::Path(vector_core::path::NodePath::from_bez(&bp)), Affine::IDENTITY, style)
}

fn fill_rect(r: Rect, color: Color) -> Object {
    let mut bp = BezPath::new();
    bp.move_to((r.x0, r.y0));
    bp.line_to((r.x1, r.y0));
    bp.line_to((r.x1, r.y1));
    bp.line_to((r.x0, r.y1));
    bp.close_path();
    Object::new(0, ObjKind::Path(vector_core::path::NodePath::from_bez(&bp)), Affine::IDENTITY, Style::filled(color))
}

fn label(text: &str, at: Point, size: f64) -> Object {
    let fmt = vector_core::text::CharFormat { size, ..Default::default() };
    Object::new(0, ObjKind::Text(vector_core::text::TextObj::artistic(text, fmt)), Affine::translate(at.to_vec2()), Style::text_default())
}

/// The printers' marks around `target` (the page, or the objects with Marks to objects), in
/// paper points.
pub fn marks(st: &PrintSettings, target: Rect, name: &str, page_no: usize) -> Vec<Object> {
    let mut v = vec![];
    let k = Color::BLACK;
    let gap = 9.0f64.max(st.bleed_limit.min(18.0));
    if st.crop_marks {
        let len = 18.0;
        let mut bp = BezPath::new();
        for (x, y, dx, dy) in [(target.x0, target.y0, -1.0, -1.0), (target.x1, target.y0, 1.0, -1.0), (target.x0, target.y1, -1.0, 1.0), (target.x1, target.y1, 1.0, 1.0)] {
            bp.move_to((x + dx * gap, y));
            bp.line_to((x + dx * (gap + len), y));
            bp.move_to((x, y + dy * gap));
            bp.line_to((x, y + dy * (gap + len)));
        }
        v.push(stroke_path(bp, 0.5, k));
    }
    if st.registration_marks {
        let off = gap + 12.0;
        for c in [Point::new(target.center().x, target.y0 - off), Point::new(target.center().x, target.y1 + off), Point::new(target.x0 - off, target.center().y), Point::new(target.x1 + off, target.center().y)] {
            let mut bp = kurbo::Circle::new(c, 5.0).to_path(0.05);
            bp.move_to((c.x - 9.0, c.y));
            bp.line_to((c.x + 9.0, c.y));
            bp.move_to((c.x, c.y - 9.0));
            bp.line_to((c.x, c.y + 9.0));
            v.push(stroke_path(bp, 0.5, k));
        }
    }
    if st.calibration_bar {
        let colors = [Color::rgb(0, 174, 239), Color::rgb(236, 0, 140), Color::rgb(255, 242, 0), Color::rgb(237, 28, 36), Color::rgb(0, 166, 81), Color::rgb(46, 49, 146), Color::BLACK];
        let s = 10.0;
        let x0 = target.x0 + 36.0;
        let y0 = target.y0 - gap - 4.0 - s;
        for (i, c) in colors.iter().enumerate() {
            v.push(fill_rect(Rect::new(x0 + i as f64 * s, y0, x0 + (i + 1) as f64 * s, y0 + s), *c));
        }
    }
    if st.densitometer {
        let s = 10.0;
        let x0 = target.x0 - gap - 4.0 - s;
        let y0 = target.y0 + 36.0;
        for i in 0..=10 {
            let g = (255.0 * (1.0 - i as f64 / 10.0)).round() as u8;
            v.push(fill_rect(Rect::new(x0, y0 + i as f64 * s, x0 + s, y0 + (i + 1) as f64 * s), Color::rgb(g, g, g)));
        }
    }
    if st.page_numbers {
        v.push(label(&format!("Page {page_no}"), Point::new(target.center().x - 14.0, target.y1 + gap + 16.0), 7.0));
    }
    if st.file_info {
        let when = crate::settings::format_time(std::time::SystemTime::now());
        v.push(label(&format!("{name}   {when}   Page {page_no}"), Point::new(target.x0, target.y0 - gap - 20.0), 7.0));
    }
    v
}

/// Tiling marks: short lines where tiles meet, at the printable area edges.
fn tiling_marks(paper: &Paper) -> Vec<Object> {
    let pr = paper.printable;
    let mut bp = BezPath::new();
    for (x, y, dx, dy) in [(pr.x0, pr.y0, 1.0, 1.0), (pr.x1, pr.y0, -1.0, 1.0), (pr.x0, pr.y1, 1.0, -1.0), (pr.x1, pr.y1, -1.0, -1.0)] {
        bp.move_to((x, y));
        bp.line_to((x + dx * 12.0, y));
        bp.move_to((x, y));
        bp.line_to((x, y + dy * 12.0));
    }
    vec![stroke_path(bp, 0.5, Color::BLACK)]
}

// ------------------------------------------------------------------------------------------
// rendering a sheet
// ------------------------------------------------------------------------------------------

/// What a job prints from one document.
#[derive(Clone)]
pub struct JobDoc {
    pub doc: Document,
    pub name: String,
    /// Print only these objects (File ▸ Print ▸ Selection).
    pub only: Option<Vec<ObjId>>,
}

/// The top-level objects of a page that are not in `keep` (to hide them).
fn others(doc: &Document, page: usize, keep: &[ObjId]) -> Vec<ObjId> {
    let mut v = vec![];
    if let Some(pg) = doc.pages.get(page) {
        for l in &pg.layers {
            for o in &l.objects {
                if !keep.contains(&o.id) {
                    v.push(o.id);
                }
            }
        }
    }
    v
}

/// Render rows `y0..y0 + h` of a sheet at `dpi` into a white pixmap (paper pixels).
pub fn render_band(renderer: &mut vector_render::Renderer, jd: &JobDoc, sheet: &Sheet, paper: &Paper, st: &PrintSettings, page_no: usize, dpi: f64, y0: u32, h: u32) -> Option<tiny_skia::Pixmap> {
    let k = dpi / 72.0;
    let w = (paper.width * k).round().max(1.0) as u32;
    let mut pm = tiny_skia::Pixmap::new(w, h.max(1))?;
    pm.fill(tiny_skia::Color::WHITE);
    let mut paper_to_px = Affine::translate((0.0, -(y0 as f64))) * Affine::scale(k);
    if st.mirror {
        paper_to_px = Affine::translate((0.0, -(y0 as f64))) * Affine::translate((w as f64, 0.0)) * Affine::scale_non_uniform(-k, k);
    }
    // the artwork, clipped to the page and bleed limit
    let clip_px = paper_to_px.transform_rect_bbox(sheet.clip.intersect(paper.rect()));
    let cx0 = clip_px.x0.max(0.0).floor() as i32;
    let cy0 = clip_px.y0.max(0.0).floor() as i32;
    let cx1 = clip_px.x1.min(w as f64).ceil() as i32;
    let cy1 = clip_px.y1.min(h as f64).ceil() as i32;
    if cx1 > cx0 && cy1 > cy0 {
        if let Some(mut art) = tiny_skia::Pixmap::new((cx1 - cx0) as u32, (cy1 - cy0) as u32) {
            let view = Affine::translate((-(cx0 as f64), -(cy0 as f64))) * paper_to_px * sheet.place;
            let hidden = jd.only.as_ref().map(|keep| others(&jd.doc, sheet.page, keep)).unwrap_or_default();
            if let Some(bg) = jd.doc.pages.get(sheet.page).and_then(|p| p.background) {
                art.fill(vector_render::sk_color(bg));
            }
            let opts = vector_render::RenderOptions { draw_page: false, printing: true, hidden, antialias: true, ..Default::default() };
            renderer.render_page(&jd.doc, sheet.page, view, &mut art, &opts);
            pm.draw_pixmap(cx0, cy0, art.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
        }
    }
    // marks
    let mut extra = vec![];
    if st.has_marks() {
        let target = if st.marks_to_objects {
            jd.only.as_ref().and_then(|ids| jd.doc.selection_bounds(ids)).map(|b| sheet.place.transform_rect_bbox(b)).unwrap_or(sheet.page_rect)
        } else {
            sheet.page_rect
        };
        extra.extend(marks(st, target, &jd.name, page_no));
    }
    if sheet.tile.is_some() && st.tiling_marks {
        extra.extend(tiling_marks(paper));
    }
    if !extra.is_empty() {
        let opts = vector_render::RenderOptions { draw_page: false, ..Default::default() };
        renderer.render_objects(&jd.doc, &extra, paper_to_px, &mut pm, &opts);
    }
    // color output
    if st.output == OutputColor::Grayscale || st.invert {
        for px in pm.pixels_mut() {
            let c = px.demultiply();
            let (mut r, mut g, mut b) = (c.red(), c.green(), c.blue());
            if st.output == OutputColor::Grayscale {
                let y = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32).round() as u8;
                (r, g, b) = (y, y, y);
            }
            if st.invert {
                (r, g, b) = (255 - r, 255 - g, 255 - b);
            }
            *px = tiny_skia::ColorU8::from_rgba(r, g, b, 255).premultiply();
        }
    }
    Some(pm)
}

/// A whole sheet as one image (Print Preview).
pub fn render_sheet(jd: &JobDoc, sheet: &Sheet, paper: &Paper, st: &PrintSettings, page_no: usize, dpi: f64) -> Option<tiny_skia::Pixmap> {
    let h = (paper.height * dpi / 72.0).round().max(1.0) as u32;
    let mut r = vector_render::Renderer::new();
    render_band(&mut r, jd, sheet, paper, st, page_no, dpi, 0, h)
}

// ------------------------------------------------------------------------------------------
// preflight
// ------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub title: String,
    pub detail: String,
}

/// The preflight checks for printing: problems that are likely to spoil the printout.
pub fn preflight(doc: &Document, pages: &[usize], st: &PrintSettings, paper: &Paper) -> Vec<Issue> {
    let mut out = vec![];
    let mut off_page = 0;
    let mut low_res = 0;
    let mut empty_text = 0;
    let mut hairlines = 0;
    let mut objects = 0;
    for &p in pages {
        let Some(pg) = doc.pages.get(p) else { continue };
        let pr = pg.rect();
        for l in pg.layers.iter().filter(|l| l.visible && l.printable) {
            for o in &l.objects {
                objects += 1;
                let b = o.visual_bounds();
                if b.intersect(pr).area() <= 0.0 && b.area() > 0.0 {
                    off_page += 1;
                }
                o.walk(&mut |x| {
                    match &x.kind {
                        ObjKind::Image(i) => {
                            if let Some(img) = doc.images.get(&i.image) {
                                let wb = x.visual_bounds();
                                let dpi = img.width as f64 / (wb.width() / 72.0).max(1e-6);
                                if dpi < 150.0 {
                                    low_res += 1;
                                }
                            }
                        }
                        ObjKind::Text(t) if t.content.trim().is_empty() => empty_text += 1,
                        _ => {}
                    }
                    if x.style.outline.color.is_some() && x.style.outline.width > 0.0 && x.style.outline.width < 0.25 {
                        hairlines += 1;
                    }
                });
            }
        }
    }
    if objects == 0 {
        out.push(Issue { title: "There is nothing to print.".into(), detail: "The pages being printed have no printable objects.".into() });
    }
    if off_page > 0 {
        out.push(Issue { title: format!("{off_page} object(s) lie completely outside the page."), detail: "Objects outside the page do not print. Move them onto the page or remove them.".into() });
    }
    if low_res > 0 {
        out.push(Issue { title: format!("{low_res} bitmap(s) have a resolution lower than 150 dpi."), detail: "Low-resolution bitmaps print blurred or pixelated. Use higher-resolution images or print them smaller.".into() });
    }
    if empty_text > 0 {
        out.push(Issue { title: format!("{empty_text} empty text object(s)."), detail: "Empty text objects print nothing and can be deleted.".into() });
    }
    if hairlines > 0 {
        out.push(Issue { title: format!("{hairlines} object(s) have outlines thinner than 0.25 pt."), detail: "Very thin outlines may not print, or print unevenly. Use at least 0.25 pt.".into() });
    }
    if st.has_marks() && !st.marks_to_objects && st.placement != Placement::FitToPage {
        for &p in pages {
            if let Some(pg) = doc.pages.get(p) {
                if pg.width + 2.0 * MARK_ROOM > paper.width + 0.5 || pg.height + 2.0 * MARK_ROOM > paper.height + 0.5 {
                    out.push(Issue { title: "Printers' marks do not fit on the paper.".into(), detail: "The paper must be larger than the page to hold crop marks, registration marks and the other printers' marks. Choose a larger paper, or Fit to page on the Layout tab.".into() });
                    break;
                }
            }
        }
    }
    out
}

// ------------------------------------------------------------------------------------------
// the job
// ------------------------------------------------------------------------------------------

pub struct Job {
    pub printer: String,
    /// The printer's settings from Printer preferences (a DEVMODE), if changed.
    pub devmode: Option<Vec<u8>>,
    pub docs: Vec<JobDoc>,
    /// (document, page) in print order.
    pub pages: Vec<(usize, usize)>,
    pub settings: PrintSettings,
    /// Print to file: the file (numbered per page with Pages to separate files).
    pub file: Option<PathBuf>,
    pub title: String,
}

impl Job {
    /// The (document, page, width, height, printed area) list `layout` takes.
    pub fn page_list(&self) -> Vec<(usize, usize, f64, f64, Rect)> {
        self.pages
            .iter()
            .filter_map(|&(d, p)| {
                let jd = self.docs.get(d)?;
                let pg = jd.doc.pages.get(p)?;
                let area = jd.only.as_ref().and_then(|ids| jd.doc.selection_bounds(ids)).unwrap_or(pg.rect());
                Some((d, p, pg.width, pg.height, area))
            })
            .collect()
    }
}

/// The page numbers (0-based) a page-range text like "1-3, 5" names, filtered by parity.
pub fn page_range(s: &str, n: usize, parity: Parity) -> Vec<usize> {
    let mut v = crate::dialogs::forms::parse_page_range(s, n);
    if s.trim().is_empty() {
        v = (0..n).collect();
    }
    v.retain(|p| match parity {
        Parity::Both => true,
        Parity::Even => (p + 1) % 2 == 0,
        Parity::Odd => (p + 1) % 2 == 1,
    });
    v
}

#[cfg(windows)]
pub use win::{default_printer, open_preferences, paper_for, printers, run_job};

pub mod cups;

#[cfg(not(windows))]
pub fn printers() -> Vec<String> {
    cups::printers()
}
#[cfg(not(windows))]
pub fn default_printer() -> Option<String> {
    cups::default_printer()
}
/// The paper the printer uses in this orientation, with the options of Printing Preferences.
#[cfg(not(windows))]
pub fn paper_for(printer: &str, options: Option<&[u8]>, landscape: bool) -> Paper {
    cups::caps(printer).paper(&cups::Options::from_bytes(options), landscape)
}

/// Print the job through CUPS: the sheets become a PDF of page images in the private temporary
/// folder, which `lp` sends with the copies and the options of Printing Preferences. Print to file
/// writes that PDF (with the copies, as Windows writes them) instead. Returns a short report for
/// the status message.
#[cfg(not(windows))]
pub fn run_job(job: Job) -> Result<String, String> {
    if !cups::printers().contains(&job.printer) {
        return Err(format!("The printer \"{}\" is not available.", job.printer));
    }
    let caps = cups::caps(&job.printer);
    let opts = cups::Options::from_bytes(job.devmode.as_deref());
    let (portrait, landscape) = (caps.paper(&opts, false), caps.paper(&opts, true));
    let sheets = layout(&job.settings, &job.page_list(), &|l| if l { landscape } else { portrait });
    if sheets.is_empty() {
        return Err("There are no pages to print.".into());
    }
    let n = sheets.len();
    let copies = job.settings.copies.max(1) as usize;
    if let Some(path) = &job.file {
        let mut order = vec![];
        if job.settings.collate {
            for _ in 0..copies {
                order.extend(0..n);
            }
        } else {
            for i in 0..n {
                order.extend(std::iter::repeat_n(i, copies));
            }
        }
        let mut total = 0;
        if job.settings.file_mode == ToFile::PerPage && n > 1 {
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let ext = path.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "pdf".into());
            for i in 0..n {
                let one: Vec<usize> = order.iter().copied().filter(|x| *x == i).collect();
                total += write_pdf_file(&job, &sheets, &one, &path.with_file_name(format!("{stem}_{}.{ext}", i + 1)))?;
            }
        } else {
            total += write_pdf_file(&job, &sheets, &order, path)?;
        }
        return Ok(format!("Printed {total} page(s) to {}", path.display()));
    }
    // one set of sheets; CUPS makes the copies, collated or not
    static JOBS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = JOBS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = crate::platform::temp_dir().join(format!("print-{}-{k}.pdf", std::process::id()));
    // left by a crashed AnyVector with the same process number
    let _ = std::fs::remove_file(&tmp);
    let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(|e| format!("Could not write the print file: {e}"))?;
    let sent = write_pdf_to(&job, &sheets, &(0..n).collect::<Vec<_>>(), file).and_then(|_| cups::submit(&job.printer, copies as u32, job.settings.collate, &job.title, &opts, &tmp));
    // lp has handed the whole file to the scheduler when it returns
    let _ = std::fs::remove_file(&tmp);
    let id = sent?;
    let total = n * copies;
    Ok(if id.is_empty() { format!("Sent {total} page(s) to {}", job.printer) } else { format!("Sent {total} page(s) to {} (job {id})", job.printer) })
}

/// `write_pdf` into a file.
#[cfg_attr(windows, allow(dead_code))]
fn write_pdf_file(job: &Job, sheets: &[(Sheet, Paper)], order: &[usize], path: &std::path::Path) -> Result<usize, String> {
    let f = std::fs::File::create(path).map_err(|e| format!("Could not write the print file: {e}"))?;
    write_pdf_to(job, sheets, order, f)
}

#[cfg_attr(windows, allow(dead_code))]
fn write_pdf_to(job: &Job, sheets: &[(Sheet, Paper)], order: &[usize], f: std::fs::File) -> Result<usize, String> {
    let fail = |e: std::io::Error| match e.to_string() {
        e if e == NOT_RENDERED => e,
        e => format!("Could not write the print file: {e}"),
    };
    let mut w = std::io::BufWriter::new(f);
    let n = write_pdf(job, sheets, order, &mut w).map_err(fail)?;
    std::io::Write::flush(&mut w).map_err(fail)?;
    Ok(n)
}

const NOT_RENDERED: &str = "The page could not be rendered.";

/// A writer that counts the bytes written (the offsets of a PDF's objects).
struct Counted<W> {
    w: W,
    n: u64,
}

impl<W: std::io::Write> std::io::Write for Counted<W> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let k = self.w.write(b)?;
        self.n += k as u64;
        Ok(k)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.w.flush()
    }
}

/// A number in a PDF: at most 3 decimals, no trailing zeros.
fn pdf_num(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The sheets in `order` as a PDF, one page per sheet, the size of its paper, holding the sheet as
/// one image rendered band by band at the job's resolution (the printer's, capped at 600 dpi or at
/// "Print as bitmap", as on Windows). The bands are compressed as they are made, so a page never
/// sits in memory uncompressed. Grayscale output is a gray image. Returns the pages written.
#[cfg_attr(windows, allow(dead_code))]
pub fn write_pdf(job: &Job, sheets: &[(Sheet, Paper)], order: &[usize], out: impl std::io::Write) -> std::io::Result<usize> {
    use std::io::Write;
    let mut f = Counted { w: out, n: 0 };
    // objects: 1 catalog, 2 pages, 3 info, then 4 per page (page, contents, image, image length)
    let mut offsets = vec![0u64; 4 + 4 * order.len()];
    f.write_all(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")?;
    let gray = job.settings.output == OutputColor::Grayscale;
    let mut renderer = vector_render::Renderer::new();
    let mut kids = String::new();
    for (i, &si) in order.iter().enumerate() {
        let (sheet, paper) = &sheets[si];
        let jd = &job.docs[sheet.doc];
        let cap = if job.settings.as_bitmap { job.settings.bitmap_dpi as f64 } else { 600.0 };
        let dpi = paper.dpi.0.min(cap).max(72.0);
        let k = dpi / 72.0;
        let wpx = (paper.width * k).round().max(1.0) as u32;
        let hpx = (paper.height * k).round().max(1.0) as u32;
        let base = 4 + 4 * i;
        kids.push_str(&format!("{base} 0 R "));
        offsets[base] = f.n;
        write!(f, "{base} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /XObject << /Im0 {} 0 R >> >> /Contents {} 0 R >>\nendobj\n", pdf_num(paper.width), pdf_num(paper.height), base + 2, base + 1)?;
        let content = format!("q {} 0 0 {} 0 0 cm /Im0 Do Q\n", pdf_num(paper.width), pdf_num(paper.height));
        offsets[base + 1] = f.n;
        write!(f, "{} 0 obj\n<< /Length {} >>\nstream\n{content}endstream\nendobj\n", base + 1, content.len())?;
        offsets[base + 2] = f.n;
        let space = if gray { "DeviceGray" } else { "DeviceRGB" };
        write!(f, "{} 0 obj\n<< /Type /XObject /Subtype /Image /Width {wpx} /Height {hpx} /ColorSpace /{space} /BitsPerComponent 8 /Filter /FlateDecode /Length {} 0 R >>\nstream\n", base + 2, base + 3)?;
        let start = f.n;
        {
            let mut z = flate2::write::ZlibEncoder::new(&mut f, flate2::Compression::default());
            let band = ((48u64 << 20) / (wpx as u64 * 4)).clamp(16, 4096) as u32;
            let mut row = Vec::new();
            let mut y = 0;
            while y < hpx {
                let h = band.min(hpx - y);
                let pm = render_band(&mut renderer, jd, sheet, paper, &job.settings, sheet.page + 1, dpi, y, h).ok_or_else(|| std::io::Error::other(NOT_RENDERED))?;
                row.clear();
                for px in pm.pixels() {
                    let c = px.demultiply();
                    if gray {
                        row.push(c.red());
                    } else {
                        row.extend_from_slice(&[c.red(), c.green(), c.blue()]);
                    }
                }
                z.write_all(&row)?;
                y += h;
            }
            z.finish()?;
        }
        let len = f.n - start;
        f.write_all(b"\nendstream\nendobj\n")?;
        offsets[base + 3] = f.n;
        write!(f, "{} 0 obj\n{len}\nendobj\n", base + 3)?;
    }
    offsets[2] = f.n;
    write!(f, "2 0 obj\n<< /Type /Pages /Count {} /Kids [{}] >>\nendobj\n", order.len(), kids.trim_end())?;
    // the title as UTF-16 (any language)
    let title: String = std::iter::once(0xfeffu16).chain(job.title.encode_utf16()).map(|u| format!("{u:04X}")).collect();
    offsets[3] = f.n;
    write!(f, "3 0 obj\n<< /Title <{title}> /Producer ({}) >>\nendobj\n", vector_core::APP_NAME)?;
    offsets[1] = f.n;
    f.write_all(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")?;
    let xref = f.n;
    write!(f, "xref\n0 {}\n0000000000 65535 f \n", offsets.len())?;
    for o in &offsets[1..] {
        write!(f, "{o:010} 00000 n \n")?;
    }
    write!(f, "trailer\n<< /Size {} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len())?;
    f.flush()?;
    Ok(order.len())
}

#[cfg(windows)]
mod win {
    use super::{Job, Paper};
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::Graphics::Printing::*;
    use windows::Win32::Storage::Xps::{EndDoc, EndPage, StartDocW, StartPage, AbortDoc, DOCINFOW};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The printers Windows knows (local and network connections).
    pub fn printers() -> Vec<String> {
        let mut out = vec![];
        unsafe {
            let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
            let mut needed = 0u32;
            let mut count = 0u32;
            let _ = EnumPrintersW(flags, PCWSTR::null(), 4, None, &mut needed, &mut count);
            if needed == 0 {
                return out;
            }
            let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
            let bytes = std::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, needed as usize);
            if EnumPrintersW(flags, PCWSTR::null(), 4, Some(bytes), &mut needed, &mut count).is_ok() {
                let infos = std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_4W, count as usize);
                for i in infos {
                    if let Ok(s) = i.pPrinterName.to_string() {
                        out.push(s);
                    }
                }
            }
        }
        out
    }

    pub fn default_printer() -> Option<String> {
        unsafe {
            let mut n = 0u32;
            let _ = GetDefaultPrinterW(None, &mut n);
            if n == 0 {
                return None;
            }
            let mut buf = vec![0u16; n as usize];
            if GetDefaultPrinterW(Some(PWSTR(buf.as_mut_ptr())), &mut n).as_bool() {
                let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
                return Some(String::from_utf16_lossy(&buf[..end]));
            }
        }
        None
    }

    /// The printer's DEVMODE: the one given, or the driver's defaults.
    fn devmode(printer: &str, given: Option<&[u8]>) -> Option<Vec<u64>> {
        unsafe {
            let name = wide(printer);
            let mut h = PRINTER_HANDLE::default();
            OpenPrinterW(PCWSTR(name.as_ptr()), &mut h, None).ok()?;
            let size = DocumentPropertiesW(None, h, PCWSTR(name.as_ptr()), None, None, 0);
            if size <= 0 {
                let _ = ClosePrinter(h);
                return None;
            }
            let mut buf = vec![0u64; (size as usize).div_ceil(8) + 1];
            let out = buf.as_mut_ptr() as *mut DEVMODEW;
            let ok = match given {
                Some(g) if g.len() >= std::mem::size_of::<DEVMODEW>() => {
                    let mut inb = vec![0u64; g.len().div_ceil(8)];
                    std::ptr::copy_nonoverlapping(g.as_ptr(), inb.as_mut_ptr() as *mut u8, g.len());
                    DocumentPropertiesW(None, h, PCWSTR(name.as_ptr()), Some(out), Some(inb.as_ptr() as *const DEVMODEW), (DM_OUT_BUFFER.0 | DM_IN_BUFFER.0) as u32)
                }
                _ => DocumentPropertiesW(None, h, PCWSTR(name.as_ptr()), Some(out), None, DM_OUT_BUFFER.0 as u32),
            };
            let _ = ClosePrinter(h);
            (ok >= 0).then_some(buf)
        }
    }

    fn devmode_bytes(buf: &[u64]) -> Vec<u8> {
        let dm = buf.as_ptr() as *const DEVMODEW;
        let len = unsafe { (*dm).dmSize as usize + (*dm).dmDriverExtra as usize };
        let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, len.min(buf.len() * 8)) };
        bytes.to_vec()
    }

    fn set_orientation(buf: &mut [u64], landscape: bool) {
        unsafe {
            let dm = buf.as_mut_ptr() as *mut DEVMODEW;
            (*dm).dmFields |= DM_ORIENTATION;
            (*dm).Anonymous1.Anonymous1.dmOrientation = if landscape { 2 } else { 1 };
            // copies are made by AnyVector (so collating works with every driver)
            (*dm).dmFields |= DM_COPIES;
            (*dm).Anonymous1.Anonymous1.dmCopies = 1;
        }
    }

    /// Printer ▸ Preferences: the driver's own settings dialog. Returns the new settings, or
    /// None when cancelled.
    pub fn open_preferences(printer: &str, given: Option<&[u8]>) -> Option<Vec<u8>> {
        unsafe {
            let mut buf = devmode(printer, given)?;
            let name = wide(printer);
            let mut h = PRINTER_HANDLE::default();
            OpenPrinterW(PCWSTR(name.as_ptr()), &mut h, None).ok()?;
            let owner = windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow();
            let inb = buf.clone();
            let r = DocumentPropertiesW(Some(owner), h, PCWSTR(name.as_ptr()), Some(buf.as_mut_ptr() as *mut DEVMODEW), Some(inb.as_ptr() as *const DEVMODEW), (DM_OUT_BUFFER.0 | DM_IN_BUFFER.0 | DM_IN_PROMPT.0) as u32);
            let _ = ClosePrinter(h);
            (r == 1).then(|| devmode_bytes(&buf))
        }
    }

    fn paper_of(hdc: HDC) -> Paper {
        unsafe {
            let dx = GetDeviceCaps(Some(hdc), LOGPIXELSX).max(1) as f64;
            let dy = GetDeviceCaps(Some(hdc), LOGPIXELSY).max(1) as f64;
            let pw = GetDeviceCaps(Some(hdc), PHYSICALWIDTH) as f64 * 72.0 / dx;
            let ph = GetDeviceCaps(Some(hdc), PHYSICALHEIGHT) as f64 * 72.0 / dy;
            let ox = GetDeviceCaps(Some(hdc), PHYSICALOFFSETX) as f64 * 72.0 / dx;
            let oy = GetDeviceCaps(Some(hdc), PHYSICALOFFSETY) as f64 * 72.0 / dy;
            let hw = GetDeviceCaps(Some(hdc), HORZRES) as f64 * 72.0 / dx;
            let hh = GetDeviceCaps(Some(hdc), VERTRES) as f64 * 72.0 / dy;
            Paper { width: pw, height: ph, printable: kurbo::Rect::new(ox, oy, ox + hw, oy + hh), dpi: (dx, dy) }
        }
    }

    /// The paper the printer uses in this orientation.
    pub fn paper_for(printer: &str, given: Option<&[u8]>, landscape: bool) -> Paper {
        let Some(mut buf) = devmode(printer, given) else { return Paper::fallback(landscape) };
        set_orientation(&mut buf, landscape);
        unsafe {
            let name = wide(printer);
            let hdc = CreateICW(PCWSTR::null(), PCWSTR(name.as_ptr()), PCWSTR::null(), Some(buf.as_ptr() as *const DEVMODEW));
            if hdc.is_invalid() {
                return Paper::fallback(landscape);
            }
            let p = paper_of(hdc);
            let _ = DeleteDC(hdc);
            if p.width <= 0.0 || p.height <= 0.0 {
                return Paper::fallback(landscape);
            }
            p
        }
    }

    struct Dc(HDC);
    impl Drop for Dc {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteDC(self.0);
            }
        }
    }

    fn send_band(hdc: HDC, pm: &tiny_skia::Pixmap, x: i32, y: i32, dw: i32, dh: i32) {
        let (w, h) = (pm.width() as i32, pm.height() as i32);
        let mut bgr = Vec::with_capacity((w * h * 4) as usize);
        for px in pm.pixels() {
            let c = px.demultiply();
            bgr.extend_from_slice(&[c.blue(), c.green(), c.red(), 0]);
        }
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER { biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32, biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() },
            ..Default::default()
        };
        unsafe {
            SetStretchBltMode(hdc, HALFTONE);
            StretchDIBits(hdc, x, y, dw, dh, 0, 0, w, h, Some(bgr.as_ptr() as *const _), &bmi, DIB_RGB_COLORS, SRCCOPY);
        }
    }

    fn print_sheets(job: &Job, sheets: &[(super::Sheet, Paper)], order: &[usize], file: Option<&std::path::Path>, base: &[u64]) -> Result<usize, String> {
        unsafe {
            let name = wide(&job.printer);
            let mut dm = base.to_vec();
            let first_landscape = order.first().map(|i| sheets[*i].0.landscape).unwrap_or(false);
            set_orientation(&mut dm, first_landscape);
            let hdc = CreateDCW(PCWSTR::null(), PCWSTR(name.as_ptr()), PCWSTR::null(), Some(dm.as_ptr() as *const DEVMODEW));
            if hdc.is_invalid() {
                return Err(format!("Could not open the printer \"{}\".", job.printer));
            }
            let dc = Dc(hdc);
            let title = wide(&job.title);
            let out = file.map(|p| wide(&p.display().to_string()));
            let di = DOCINFOW {
                cbSize: std::mem::size_of::<DOCINFOW>() as i32,
                lpszDocName: PCWSTR(title.as_ptr()),
                lpszOutput: out.as_ref().map(|o| PCWSTR(o.as_ptr())).unwrap_or(PCWSTR::null()),
                ..Default::default()
            };
            if StartDocW(dc.0, &di) <= 0 {
                return Err("The printer did not accept the job.".into());
            }
            let mut renderer = vector_render::Renderer::new();
            let mut printed = 0;
            let mut current_landscape = first_landscape;
            for &i in order {
                let (sheet, _) = &sheets[i];
                if sheet.landscape != current_landscape {
                    set_orientation(&mut dm, sheet.landscape);
                    ResetDCW(dc.0, dm.as_ptr() as *const DEVMODEW);
                    current_landscape = sheet.landscape;
                }
                let paper = paper_of(dc.0);
                if StartPage(dc.0) <= 0 {
                    let _ = AbortDoc(dc.0);
                    return Err("The printer stopped the job.".into());
                }
                let jd = &job.docs[sheet.doc];
                let cap = if job.settings.as_bitmap { job.settings.bitmap_dpi as f64 } else { 600.0 };
                let dpi = paper.dpi.0.min(cap).max(72.0);
                let k = dpi / 72.0;
                let wpx = (paper.width * k).round() as u32;
                let hpx = (paper.height * k).round() as u32;
                let band = ((48u64 << 20) / (wpx.max(1) as u64 * 4)).clamp(16, 4096) as u32;
                // device pixels per rendered pixel; the device origin is the printable area
                let sx = paper.dpi.0 / dpi;
                let sy = paper.dpi.1 / dpi;
                let off_x = (paper.printable.x0 * paper.dpi.0 / 72.0).round() as i32;
                let off_y = (paper.printable.y0 * paper.dpi.1 / 72.0).round() as i32;
                let mut y = 0;
                while y < hpx {
                    let h = band.min(hpx - y);
                    if let Some(pm) = super::render_band(&mut renderer, jd, sheet, &paper, &job.settings, sheet.page + 1, dpi, y, h) {
                        let dy0 = (y as f64 * sy).round() as i32;
                        let dy1 = ((y + h) as f64 * sy).round() as i32;
                        send_band(dc.0, &pm, -off_x, dy0 - off_y, (wpx as f64 * sx).round() as i32, dy1 - dy0);
                    }
                    y += h;
                }
                if EndPage(dc.0) <= 0 {
                    let _ = AbortDoc(dc.0);
                    return Err("The printer stopped the job.".into());
                }
                printed += 1;
            }
            if EndDoc(dc.0) <= 0 {
                return Err("The printer could not finish the job.".into());
            }
            Ok(printed)
        }
    }

    /// Print the job. Returns a short report for the status message.
    pub fn run_job(job: Job) -> Result<String, String> {
        let base = devmode(&job.printer, job.devmode.as_deref()).ok_or_else(|| format!("The printer \"{}\" is not available.", job.printer))?;
        let bytes = devmode_bytes(&base);
        let list = job.page_list();
        let sheets = super::layout(&job.settings, &list, &|landscape| paper_for(&job.printer, Some(&bytes), landscape));
        if sheets.is_empty() {
            return Err("There are no pages to print.".into());
        }
        let n = sheets.len();
        let copies = job.settings.copies.max(1) as usize;
        let mut order = vec![];
        if job.settings.collate {
            for _ in 0..copies {
                order.extend(0..n);
            }
        } else {
            for i in 0..n {
                order.extend(std::iter::repeat_n(i, copies));
            }
        }
        let mut total = 0;
        match (&job.file, job.settings.file_mode) {
            (Some(path), super::ToFile::PerPage) if n > 1 => {
                let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let ext = path.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "prn".into());
                for i in 0..n {
                    let p = path.with_file_name(format!("{stem}_{}.{ext}", i + 1));
                    let one: Vec<usize> = order.iter().copied().filter(|x| *x == i).collect();
                    total += print_sheets(&job, &sheets, &one, Some(&p), &base)?;
                }
            }
            (file, _) => total += print_sheets(&job, &sheets, &order, file.as_deref(), &base)?,
        }
        Ok(match &job.file {
            Some(p) => format!("Printed {total} page(s) to {}", p.display()),
            None => format!("Sent {total} page(s) to {}", job.printer),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_to_page_fits_the_printable_area() {
        let st = PrintSettings { placement: Placement::FitToPage, ..Default::default() };
        let paper = Paper::fallback(false);
        let sheets = layout(&st, &[(0, 0, 1224.0, 1584.0, Rect::new(0.0, 0.0, 1224.0, 1584.0))], &|_| paper);
        assert_eq!(sheets.len(), 1);
        let r = sheets[0].0.page_rect;
        assert!(r.x0 >= paper.printable.x0 - 0.01 && r.x1 <= paper.printable.x1 + 0.01);
        assert!(r.y0 >= paper.printable.y0 - 0.01 && r.y1 <= paper.printable.y1 + 0.01);
    }

    #[test]
    fn tiling_splits_a_large_page() {
        let st = PrintSettings { tiled: true, ..Default::default() };
        let paper = Paper::fallback(false);
        let sheets = layout(&st, &[(0, 0, 1224.0, 1584.0, Rect::new(0.0, 0.0, 1224.0, 1584.0))], &|_| paper);
        assert!(sheets.len() >= 4, "{} tiles", sheets.len());
    }

    #[test]
    fn match_orientation_follows_the_page() {
        let st = PrintSettings::default();
        let sheets = layout(&st, &[(0, 0, 792.0, 612.0, Rect::new(0.0, 0.0, 792.0, 612.0))], &|l| Paper::fallback(l));
        assert!(sheets[0].0.landscape);
    }

    #[test]
    fn page_ranges_and_parity() {
        assert_eq!(page_range("1-4", 5, Parity::Even), vec![1, 3]);
        assert_eq!(page_range("", 3, Parity::Odd), vec![0, 2]);
    }

    #[test]
    fn rendered_sheet_is_paper_sized() {
        let doc = Document::new(612.0, 792.0);
        let jd = JobDoc { doc, name: "t".into(), only: None };
        let st = PrintSettings { crop_marks: true, page_numbers: true, ..Default::default() };
        let paper = Paper::fallback(false);
        let sheets = layout(&st, &[(0, 0, 612.0, 792.0, Rect::new(0.0, 0.0, 612.0, 792.0))], &|_| paper);
        let pm = render_sheet(&jd, &sheets[0].0, &paper, &st, 1, 36.0).unwrap();
        assert_eq!((pm.width(), pm.height()), (306, 396));
    }

    /// A two-page job (a portrait and a landscape page) with a red square on page 1.
    fn two_page_job(st: PrintSettings) -> (Job, Vec<(Sheet, Paper)>) {
        let mut doc = Document::new(612.0, 792.0);
        doc.add_page(1, 792.0, 612.0);
        let mut sq = fill_rect(Rect::new(72.0, 72.0, 216.0, 216.0), Color::rgb(255, 0, 0));
        sq.id = doc.new_id();
        doc.pages[0].layers[0].objects.push(sq);
        let job = Job { printer: "PDF".into(), devmode: None, docs: vec![JobDoc { doc, name: "t".into(), only: None }], pages: vec![(0, 0), (0, 1)], settings: st, file: None, title: "Logo – Prova".into() };
        // a low printer resolution keeps the test quick
        let paper = |l: bool| Paper { dpi: (36.0, 36.0), ..Paper::fallback(l) };
        let sheets = layout(&job.settings, &job.page_list(), &paper);
        (job, sheets)
    }

    /// The PDF a Linux print job sends to CUPS: a page per sheet in the order given (copies
    /// included), each page the size of its paper (the landscape sheet turned), each a page image
    /// at the job's resolution that the PDF importer reads back, with the artwork in place.
    #[test]
    fn print_job_pdf_pages_and_sizes() {
        let (job, sheets) = two_page_job(PrintSettings::default());
        assert_eq!(sheets.len(), 2);
        let mut pdf = vec![];
        assert_eq!(write_pdf(&job, &sheets, &[0, 1, 0], &mut pdf).unwrap(), 3);
        assert_eq!(vector_io::pdf_import::page_sizes(&pdf).unwrap(), [(612.0, 792.0), (792.0, 612.0), (612.0, 792.0)]);
        let (doc, _) = vector_io::pdf_import::load_with_notes(&pdf, &Default::default()).unwrap();
        assert_eq!(doc.pages.len(), 3);
        // 36 dpi is below the 72 dpi floor: the images are 1 pixel per point
        let mut sizes: Vec<(u32, u32)> = doc.images.values().map(|i| (i.width, i.height)).collect();
        sizes.sort();
        sizes.dedup();
        assert_eq!(sizes, [(612, 792), (792, 612)]);
        // the red square of page 1 is 1 inch from the top-left of the centred page
        let first = doc.images.values().find(|i| i.width == 612 && !i.rgba.is_empty()).unwrap();
        let px = |x: usize, y: usize| &first.rgba[(y * 612 + x) * 4..(y * 612 + x) * 4 + 3];
        assert_eq!(px(144, 144), [255, 0, 0]);
        assert_eq!(px(300, 500), [255, 255, 255]);
        assert!(String::from_utf8_lossy(&pdf).ends_with("\n%%EOF\n"));
    }

    /// Grayscale output makes gray page images; Print as bitmap sets their resolution.
    #[test]
    fn print_job_pdf_grayscale_and_bitmap_resolution() {
        let st = PrintSettings { output: OutputColor::Grayscale, as_bitmap: true, bitmap_dpi: 144, ..Default::default() };
        let (job, mut sheets) = two_page_job(st);
        for s in &mut sheets {
            s.1.dpi = (600.0, 600.0);
        }
        let mut pdf = vec![];
        write_pdf(&job, &sheets, &[0], &mut pdf).unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Width 1224 /Height 1584 /ColorSpace /DeviceGray"), "{}", &text[..400.min(text.len())]);
        let (doc, _) = vector_io::pdf_import::load_with_notes(&pdf, &Default::default()).unwrap();
        let img = doc.images.values().next().unwrap();
        let i = (288 * 1224 + 288) * 4;
        let g = img.rgba[i];
        assert!(g == img.rgba[i + 1] && g == img.rgba[i + 2] && g < 128, "red prints dark gray");
    }

    /// Print to file on Linux writes the PDF; with Pages to separate files one file per sheet,
    /// numbered, each with its copies.
    #[cfg(not(windows))]
    #[test]
    fn print_to_file_writes_pdfs() {
        let dir = std::env::temp_dir().join(format!("anyvector-print-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (job, sheets) = two_page_job(PrintSettings { copies: 2, collate: false, ..Default::default() });
        let one = dir.join("Logo.pdf");
        assert_eq!(write_pdf_file(&job, &sheets, &[0, 0, 1, 1], &one).unwrap(), 4);
        assert_eq!(vector_io::pdf_import::page_sizes(&std::fs::read(&one).unwrap()).unwrap().len(), 4);
        assert!(write_pdf_file(&job, &sheets, &[0], &dir.join("missing").join("x.pdf")).unwrap_err().starts_with("Could not write the print file:"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A real job through CUPS to the cups-pdf printer "PDF" (Ubuntu: `apt install cups
    /// printer-driver-cups-pdf`); the PDF lands in ~/PDF. Run with
    /// `cargo test -p anyvector print_job_to_cups_pdf -- --ignored`.
    #[cfg(not(windows))]
    #[test]
    #[ignore]
    fn print_job_to_cups_pdf() {
        assert!(printers().contains(&"PDF".to_string()), "no cups-pdf printer: {:?}", printers());
        let (mut job, _) = two_page_job(PrintSettings { copies: 2, ..Default::default() });
        job.title = format!("anyvector-test-{}", std::process::id());
        let report = run_job(job).unwrap();
        assert!(report.starts_with("Sent 4 page(s) to PDF (job PDF-"), "{report}");
    }

    /// Print styles whose extension is in capitals (copied from another computer) are listed and
    /// opened on Linux as on Windows.
    #[test]
    fn print_styles_with_capital_extensions() {
        let dir = std::env::temp_dir().join(format!("anyvector-prs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let st = PrintSettings { copies: 3, ..Default::default() };
        save_style(&dir.join("Poster.PRS"), &st).unwrap();
        save_style(&dir.join("Proof.prs"), &PrintSettings::default()).unwrap();
        std::fs::write(dir.join("notes.txt"), "not a style").unwrap();
        assert_eq!(style_names_in(&dir), ["Poster", "Proof"]);
        let p = style_path_in(&dir, "Poster");
        assert!(p.is_file(), "{p:?}");
        assert_eq!(load_style(&p).map(|s| s.copies), Some(3));
        assert!(style_path_in(&dir, "Proof").is_file());
        // a style saved under a new name is `name.prs`
        assert_eq!(style_path_in(&dir, "New"), dir.join("New.prs"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
