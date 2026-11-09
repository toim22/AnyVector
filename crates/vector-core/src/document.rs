//! Document → Pages → Layers → Objects, plus document-level resources.

use crate::color::{Color, NamedColor, Palette};
use crate::object::{ObjId, ObjKind, Object};
use crate::style::Style;
use crate::units::Unit;
use kurbo::{Affine, Point, Rect, Shape, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub visible: bool,
    pub printable: bool,
    pub locked: bool,
    pub color: Color,
    pub objects: Vec<Object>,
}

impl Layer {
    pub fn new(id: u64, name: &str) -> Layer {
        Layer { id, name: name.to_string(), visible: true, printable: true, locked: false, color: Color::rgb(0, 102, 204), objects: vec![] }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum GuideKind {
    Horizontal,
    Vertical,
    Angled(f64),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Guide {
    pub kind: GuideKind,
    /// Position: y for horizontal, x for vertical, a point on the line for angled.
    pub pos: Point,
    pub locked: bool,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub id: u64,
    pub name: String,
    pub width: f64,
    pub height: f64,
    pub layers: Vec<Layer>,
    pub background: Option<Color>,
    #[serde(default)]
    pub guides: Vec<Guide>,
}

impl Page {
    pub fn new(id: u64, layer_id: u64, w: f64, h: f64) -> Page {
        Page { id, name: String::new(), width: w, height: h, layers: vec![Layer::new(layer_id, "Layer 1")], background: None, guides: vec![] }
    }
    pub fn rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width, self.height)
    }
    pub fn is_landscape(&self) -> bool {
        self.width > self.height
    }
    pub fn all_objects(&self) -> impl Iterator<Item = &Object> {
        self.layers.iter().flat_map(|l| l.objects.iter())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GridKind {
    #[default]
    Lines,
    Dots,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocSettings {
    pub unit: Unit,
    pub dpi: f64,
    /// Nudge distance in points.
    pub nudge: f64,
    pub super_nudge_mult: f64,
    pub micro_nudge_div: f64,
    pub duplicate_offset: Vec2,
    pub grid_spacing: Vec2,
    pub grid_kind: GridKind,
    pub grid_color: Color,
    pub grid_opacity: f64,
    pub show_grid: bool,
    pub show_rulers: bool,
    pub show_guides: bool,
    pub show_page_border: bool,
    pub show_bleed: bool,
    pub bleed: f64,
    pub snap_to_grid: bool,
    pub snap_to_guides: bool,
    pub snap_to_objects: bool,
    pub snap_to_page: bool,
    pub snap_off: bool,
    /// Ruler origin in page coordinates.
    pub ruler_origin: Point,
    pub drawing_scale: f64,
    pub treat_as_filled: bool,
    pub apply_to_all_pages: bool,
    pub pixel_grid: bool,
    pub facing_pages: bool,
    /// Color management settings chosen in Create a New Document.
    #[serde(default)]
    pub color: ColorSettings,
    /// Page view: false = single page, true = multipage.
    #[serde(default)]
    pub multipage_view: bool,
    /// View ▸ Grid ▸ Baseline Grid, and the baseline spacing in points.
    #[serde(default)]
    pub show_baseline_grid: bool,
    #[serde(default = "default_baseline_spacing")]
    pub baseline_spacing: f64,
    /// View ▸ Snap To ▸ Baseline Grid and ▸ Pixels.
    #[serde(default)]
    pub snap_to_baseline: bool,
    #[serde(default)]
    pub snap_to_pixels: bool,
    /// View ▸ Page ▸ Printable Area.
    #[serde(default)]
    pub show_printable_area: bool,
}

fn default_baseline_spacing() -> f64 {
    14.0
}

/// Rendering intent used when converting between color spaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RenderingIntent {
    Perceptual,
    #[default]
    RelativeColorimetric,
    Saturation,
    AbsoluteColorimetric,
}

impl RenderingIntent {
    pub const ALL: [RenderingIntent; 4] = [RenderingIntent::Perceptual, RenderingIntent::RelativeColorimetric, RenderingIntent::Saturation, RenderingIntent::AbsoluteColorimetric];

    pub fn name(self) -> &'static str {
        match self {
            RenderingIntent::Perceptual => "Perceptual",
            RenderingIntent::RelativeColorimetric => "Relative colorimetric",
            RenderingIntent::Saturation => "Saturation",
            RenderingIntent::AbsoluteColorimetric => "Absolute colorimetric",
        }
    }
}

pub const DEFAULT_RGB_PROFILE: &str = "sRGB IEC61966-2.1";
pub const DEFAULT_CMYK_PROFILE: &str = "U.S. Web Coated (SWOP) v2";
pub const DEFAULT_GRAY_PROFILE: &str = "Dot Gain 20%";

/// Document color settings (primary color mode and the color profiles).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorSettings {
    /// Primary color mode: true = CMYK, false = RGB.
    pub cmyk: bool,
    pub rgb_profile: String,
    pub cmyk_profile: String,
    pub gray_profile: String,
    pub intent: RenderingIntent,
}

impl Default for ColorSettings {
    fn default() -> Self {
        ColorSettings {
            cmyk: true,
            rgb_profile: DEFAULT_RGB_PROFILE.into(),
            cmyk_profile: DEFAULT_CMYK_PROFILE.into(),
            gray_profile: DEFAULT_GRAY_PROFILE.into(),
            intent: RenderingIntent::RelativeColorimetric,
        }
    }
}

impl ColorSettings {
    /// "sRGB IEC61966-2.1; U.S. Web Coated (SWOP) v2; Dot Gain 20%".
    pub fn summary(&self) -> String {
        format!("{}; {}; {}", self.rgb_profile, self.cmyk_profile, self.gray_profile)
    }
}

impl Default for DocSettings {
    fn default() -> Self {
        DocSettings {
            unit: Unit::Inches,
            dpi: 300.0,
            nudge: 7.2,
            super_nudge_mult: 2.0,
            micro_nudge_div: 2.0,
            duplicate_offset: Vec2::new(18.0, 18.0),
            grid_spacing: Vec2::new(18.0, 18.0),
            grid_kind: GridKind::Lines,
            grid_color: Color::rgb(160, 160, 160),
            grid_opacity: 40.0,
            show_grid: false,
            show_rulers: true,
            show_guides: true,
            show_page_border: true,
            show_bleed: false,
            bleed: 0.0,
            snap_to_grid: false,
            snap_to_guides: true,
            snap_to_objects: true,
            snap_to_page: false,
            snap_off: false,
            ruler_origin: Point::ZERO,
            drawing_scale: 1.0,
            treat_as_filled: true,
            apply_to_all_pages: true,
            pixel_grid: false,
            facing_pages: false,
            color: ColorSettings::default(),
            multipage_view: false,
            show_baseline_grid: false,
            baseline_spacing: default_baseline_spacing(),
            snap_to_baseline: false,
            snap_to_pixels: false,
            show_printable_area: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymbolDef {
    pub id: u64,
    pub name: String,
    pub objects: Vec<Object>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectStyle {
    pub name: String,
    pub style: Style,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocInfo {
    pub title: String,
    pub subject: String,
    pub author: String,
    pub copyright: String,
    pub keywords: String,
    pub notes: String,
    pub rating: u8,
    /// Document language (File ▸ Document Properties ▸ Language).
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "English (United States)".into()
}

impl Default for DocInfo {
    fn default() -> Self {
        DocInfo { title: String::new(), subject: String::new(), author: String::new(), copyright: String::new(), keywords: String::new(), notes: String::new(), rating: 0, language: default_language() }
    }
}

/// The kind of a print merge column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MergeKind {
    #[default]
    Text,
    Numeric,
    /// A path to an image file.
    Path,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MergeField {
    pub name: String,
    pub kind: MergeKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MergeRecord {
    pub selected: bool,
    pub values: Vec<String>,
}

/// Print merge data (File ▸ Print Merge): the columns and records of the data source. Text
/// in the document marks a field with its column name in guillemets: «Name».
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct PrintMerge {
    pub fields: Vec<MergeField>,
    pub records: Vec<MergeRecord>,
    /// The file the data was imported from, if any.
    #[serde(default)]
    pub source: Option<String>,
}

impl PrintMerge {
    /// The field marker put in text: «Name».
    pub fn marker(name: &str) -> String {
        format!("\u{ab}{name}\u{bb}")
    }

    /// Text with each «field» replaced by the record's value.
    pub fn fill(&self, text: &str, record: &MergeRecord) -> String {
        let mut out = text.to_string();
        for (i, f) in self.fields.iter().enumerate() {
            out = out.replace(&Self::marker(&f.name), record.values.get(i).map(|s| s.as_str()).unwrap_or(""));
        }
        out
    }

    /// Read a data source: CSV (a header row, comma or tab separated) or the text format
    /// (line 1 = number of fields, line 2 = `\Field1\\Field2\`, one record per line in the same
    /// form). A field name starting with `@` is an image path column.
    pub fn parse(text: &str) -> PrintMerge {
        let text = text.trim_start_matches('\u{feff}');
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let mut pm = PrintMerge::default();
        let Some(first) = lines.next() else { return pm };
        let backslash = first.trim().parse::<usize>().is_ok();
        let split = |l: &str| -> Vec<String> {
            if backslash {
                l.trim().trim_matches('\\').split("\\\\").map(|s| s.to_string()).collect()
            } else {
                let sep = if l.contains('\t') { '\t' } else { ',' };
                split_csv(l, sep)
            }
        };
        let header = if backslash { lines.next().map(split).unwrap_or_default() } else { split(first) };
        for h in header {
            let (name, kind) = match h.strip_prefix('@') {
                Some(n) => (n.to_string(), MergeKind::Path),
                None => (h.clone(), MergeKind::Text),
            };
            pm.fields.push(MergeField { name: name.trim().to_string(), kind });
        }
        for l in lines {
            let mut values = split(l);
            values.resize(pm.fields.len(), String::new());
            pm.records.push(MergeRecord { selected: true, values });
        }
        pm
    }

    /// The data as CSV, with a header row and a byte order mark.
    pub fn to_csv(&self, selected_only: bool) -> String {
        let q = |s: &str| if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
        let mut out = String::from("\u{feff}");
        let head: Vec<String> = self.fields.iter().map(|f| q(&if f.kind == MergeKind::Path { format!("@{}", f.name) } else { f.name.clone() })).collect();
        out.push_str(&head.join(","));
        out.push_str("\r\n");
        for r in self.records.iter().filter(|r| r.selected || !selected_only) {
            let v: Vec<String> = r.values.iter().map(|s| q(s)).collect();
            out.push_str(&v.join(","));
            out.push_str("\r\n");
        }
        out
    }
}

fn split_csv(line: &str, sep: char) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    #[test]
    fn csv_and_text_sources() {
        let pm = PrintMerge::parse("Name,@Photo\r\n\"Smith, J\",a.png\r\nLee,b.png\r\n");
        assert_eq!(pm.fields.len(), 2);
        assert_eq!(pm.fields[1].kind, MergeKind::Path);
        assert_eq!(pm.records[0].values[0], "Smith, J");
        let t = PrintMerge::parse("2\n\\Name\\\\City\\\n\\Ann\\\\Rome\\\n");
        assert_eq!(t.fields[1].name, "City");
        assert_eq!(t.records[0].values, vec!["Ann", "Rome"]);
        assert_eq!(t.fill("Hi «Name» from «City»", &t.records[0]), "Hi Ann from Rome");
        let back = PrintMerge::parse(&pm.to_csv(false));
        assert_eq!(back.records, pm.records);
    }
}

/// Raw image data stored once per document (RGBA8, premultiplied = false).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    #[serde(skip)]
    pub rgba: Arc<Vec<u8>>,
    /// Original encoded bytes (PNG/JPEG), kept for lossless saving.
    #[serde(skip)]
    pub encoded: Arc<Vec<u8>>,
    pub format: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub author: String,
    pub text: String,
    pub pos: Point,
    pub page: u64,
    pub resolved: bool,
    pub replies: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedView {
    pub name: String,
    pub page: usize,
    pub zoom: f64,
    pub center: Point,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    pub pages: Vec<Page>,
    pub settings: DocSettings,
    pub info: DocInfo,
    /// Document palette (colors stored with the document).
    pub palette: Vec<NamedColor>,
    pub color_styles: Vec<NamedColor>,
    pub object_styles: Vec<ObjectStyle>,
    pub symbols: Vec<SymbolDef>,
    /// Image store; pixel data is saved separately by the file format.
    pub images: BTreeMap<u64, ImageData>,
    pub master_guides: Vec<Guide>,
    pub comments: Vec<Comment>,
    pub views: Vec<SavedView>,
    /// Object data field names (Object Data docker).
    pub data_fields: Vec<String>,
    /// Print merge data (File ▸ Print Merge).
    #[serde(default)]
    pub print_merge: Option<PrintMerge>,
    next_id: u64,
}

impl Default for Document {
    fn default() -> Self {
        Document::new(612.0, 792.0)
    }
}

impl Document {
    pub const FORMAT_VERSION: u32 = 1;

    pub fn new(w: f64, h: f64) -> Document {
        let mut d = Document {
            version: Self::FORMAT_VERSION,
            pages: vec![],
            settings: DocSettings::default(),
            info: DocInfo::default(),
            palette: vec![],
            color_styles: vec![],
            object_styles: vec![],
            symbols: vec![],
            images: BTreeMap::new(),
            master_guides: vec![],
            comments: vec![],
            views: vec![],
            data_fields: vec!["Name".into(), "Cost".into(), "Comments".into(), "CDRStaticID".into()],
            print_merge: None,
            next_id: 1,
        };
        let pid = d.new_id();
        let lid = d.new_id();
        d.pages.push(Page::new(pid, lid, w, h));
        d
    }

    pub fn new_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn ensure_next_id(&mut self) {
        let mut max = self.next_id;
        for p in &self.pages {
            max = max.max(p.id + 1);
            for l in &p.layers {
                max = max.max(l.id + 1);
                for o in &l.objects {
                    o.walk(&mut |x| max = max.max(x.id + 1));
                }
            }
        }
        for s in &self.symbols {
            max = max.max(s.id + 1);
            for o in &s.objects {
                o.walk(&mut |x| max = max.max(x.id + 1));
            }
        }
        for k in self.images.keys() {
            max = max.max(k + 1);
        }
        for c in &self.comments {
            max = max.max(c.id + 1);
        }
        self.next_id = max;
    }

    /// Give fresh ids to an object tree (used for paste/duplicate).
    pub fn reassign_ids(&mut self, o: &mut Object) {
        let mut ids = vec![];
        o.walk(&mut |_| ids.push(()));
        o.walk_mut(&mut |x| {
            x.id = 0;
        });
        let mut next = self.next_id;
        o.walk_mut(&mut |x| {
            x.id = next;
            next += 1;
        });
        self.next_id = next;
    }

    pub fn add_page(&mut self, at: usize, w: f64, h: f64) -> usize {
        let pid = self.new_id();
        let mut p = Page::new(pid, 0, w, h);
        // copy layer structure from page 0
        p.layers.clear();
        let names: Vec<(String, bool, bool, bool)> = self
            .pages
            .first()
            .map(|pg| pg.layers.iter().map(|l| (l.name.clone(), l.visible, l.printable, l.locked)).collect())
            .unwrap_or_else(|| vec![("Layer 1".into(), true, true, false)]);
        for (n, v, pr, lk) in names {
            let lid = self.new_id();
            let mut l = Layer::new(lid, &n);
            l.visible = v;
            l.printable = pr;
            l.locked = lk;
            p.layers.push(l);
        }
        let at = at.min(self.pages.len());
        self.pages.insert(at, p);
        at
    }

    pub fn page_name(&self, i: usize) -> String {
        let p = &self.pages[i];
        if p.name.is_empty() {
            format!("Page {}", i + 1)
        } else {
            p.name.clone()
        }
    }

    pub fn find(&self, id: ObjId) -> Option<&Object> {
        for p in &self.pages {
            for l in &p.layers {
                for o in &l.objects {
                    if let Some(x) = o.find(id) {
                        return Some(x);
                    }
                }
            }
        }
        None
    }

    pub fn find_mut(&mut self, id: ObjId) -> Option<&mut Object> {
        for p in self.pages.iter_mut() {
            for l in p.layers.iter_mut() {
                for o in l.objects.iter_mut() {
                    if let Some(x) = o.find_mut(id) {
                        return Some(x);
                    }
                }
            }
        }
        None
    }

    /// Location of a top-level object: (page, layer, index).
    pub fn locate(&self, id: ObjId) -> Option<(usize, usize, usize)> {
        for (pi, p) in self.pages.iter().enumerate() {
            for (li, l) in p.layers.iter().enumerate() {
                for (oi, o) in l.objects.iter().enumerate() {
                    if o.id == id {
                        return Some((pi, li, oi));
                    }
                }
            }
        }
        None
    }

    /// Path of indices to an object possibly nested in groups: top-level location + child chain.
    pub fn parent_of(&self, id: ObjId) -> Option<ObjId> {
        fn rec(o: &Object, id: ObjId) -> Option<ObjId> {
            for c in o.children() {
                if c.id == id {
                    return Some(o.id);
                }
                if let Some(x) = rec(c, id) {
                    return Some(x);
                }
            }
            None
        }
        for p in &self.pages {
            for l in &p.layers {
                for o in &l.objects {
                    if o.id == id {
                        return None;
                    }
                    if let Some(x) = rec(o, id) {
                        return Some(x);
                    }
                }
            }
        }
        None
    }

    /// Top-level ancestor of an object.
    pub fn top_level_of(&self, id: ObjId) -> Option<ObjId> {
        let mut cur = id;
        if self.find(id).is_none() {
            return None;
        }
        while let Some(p) = self.parent_of(cur) {
            cur = p;
        }
        Some(cur)
    }

    /// Remove an object anywhere in the tree; returns it.
    pub fn remove(&mut self, id: ObjId) -> Option<Object> {
        fn rec(list: &mut Vec<Object>, id: ObjId) -> Option<Object> {
            if let Some(i) = list.iter().position(|o| o.id == id) {
                return Some(list.remove(i));
            }
            for o in list.iter_mut() {
                match &mut o.kind {
                    ObjKind::Group(c) => {
                        if let Some(x) = rec(c, id) {
                            return Some(x);
                        }
                    }
                    ObjKind::PowerClip { contents, .. } => {
                        if let Some(x) = rec(contents, id) {
                            return Some(x);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        for p in self.pages.iter_mut() {
            for l in p.layers.iter_mut() {
                if let Some(x) = rec(&mut l.objects, id) {
                    return Some(x);
                }
            }
        }
        None
    }

    pub fn symbol_bounds(&self, id: u64) -> Option<Rect> {
        let s = self.symbols.iter().find(|s| s.id == id)?;
        Some(crate::object::union_bounds(s.objects.iter().map(|o| o.bounds())))
    }

    pub fn bounds_of(&self, o: &Object) -> Rect {
        o.bounds_with(&|id| self.symbol_bounds(id))
    }

    pub fn selection_bounds(&self, ids: &[ObjId]) -> Option<Rect> {
        let mut acc: Option<Rect> = None;
        for id in ids {
            if let Some(o) = self.find(*id) {
                let b = self.bounds_of(o);
                acc = Some(acc.map_or(b, |a| a.union(b)));
            }
        }
        acc
    }

    pub fn add_image(&mut self, data: ImageData) -> u64 {
        let id = self.new_id();
        self.images.insert(id, data);
        id
    }

    pub fn default_palette() -> Palette {
        Palette::default_palette()
    }

    /// Collect all colors used in the document (for "Add Colors from Document").
    pub fn used_colors(&self) -> Vec<Color> {
        let mut v: Vec<Color> = vec![];
        let mut push = |c: Color| {
            if !v.contains(&c) {
                v.push(c)
            }
        };
        for p in &self.pages {
            for o in p.all_objects() {
                o.walk(&mut |x| {
                    if let Some(c) = x.style.fill.main_color() {
                        push(c);
                    }
                    if let Some(c) = x.style.outline.color {
                        push(c);
                    }
                });
            }
        }
        v
    }

    /// Hit-test the topmost object at a point on a page. Returns the top-level object id.
    pub fn hit_test(&self, page: usize, p: Point, tolerance: f64, treat_as_filled: bool) -> Option<ObjId> {
        let pg = self.pages.get(page)?;
        for l in pg.layers.iter().rev() {
            if !l.visible || l.locked {
                continue;
            }
            for o in l.objects.iter().rev() {
                if !o.visible {
                    continue;
                }
                if self.hit_object(o, p, tolerance, treat_as_filled) {
                    return Some(o.id);
                }
            }
        }
        None
    }

    pub fn hit_object(&self, o: &Object, p: Point, tol: f64, treat_as_filled: bool) -> bool {
        match &o.kind {
            ObjKind::Group(c) => c.iter().any(|x| self.hit_object(x, p, tol, treat_as_filled)),
            ObjKind::PowerClip { frame, contents } => {
                self.hit_object(frame, p, tol, true) || {
                    let fp = frame.world_path();
                    crate::geom::contains(&fp, p, false) && contents.iter().any(|x| self.hit_object(x, p, tol, treat_as_filled))
                }
            }
            ObjKind::Blend(b) => {
                self.bounds_of(o).inflate(tol, tol).contains(p)
                    || self.hit_object(&b.start, p, tol, treat_as_filled)
                    || self.hit_object(&b.end, p, tol, treat_as_filled)
            }
            ObjKind::Symbol(sid) => {
                if let Some(s) = self.symbols.iter().find(|s| s.id == *sid) {
                    let inv = o.transform.inverse();
                    let lp = inv * p;
                    s.objects.iter().any(|x| self.hit_object(x, lp, tol, treat_as_filled))
                } else {
                    false
                }
            }
            ObjKind::Text(_) | ObjKind::Image(_) | ObjKind::Table(_) => {
                let b = self.bounds_of(o);
                if let ObjKind::Text(t) = &o.kind {
                    // use the transformed layout box for rotated text
                    let l = crate::text::layout(t);
                    let inv = o.transform.inverse();
                    let lp = inv * p;
                    return l.bounds.inflate(tol, tol).contains(lp);
                }
                let inv = o.transform.inverse();
                let lp = inv * p;
                let local = o.local_path().bounding_box();
                local.inflate(tol, tol).contains(lp) || b.width() == 0.0 && false
            }
            _ => {
                let path = o.world_path();
                let filled = !o.style.fill.is_none() || treat_as_filled;
                if filled && o.is_closed_shape() && crate::geom::contains(&path, p, o.style.even_odd) {
                    return true;
                }
                let w = if o.style.outline.is_visible() { o.style.outline.width / 2.0 } else { 0.0 };
                crate::geom::distance_to_outline(&path, p) <= tol + w
            }
        }
    }

    /// All top-level object ids on a page (visible, unlocked layers) whose bounds fall inside `r`.
    pub fn objects_in_rect(&self, page: usize, r: Rect, touching: bool) -> Vec<ObjId> {
        let mut v = vec![];
        if let Some(pg) = self.pages.get(page) {
            for l in &pg.layers {
                if !l.visible || l.locked {
                    continue;
                }
                for o in &l.objects {
                    if !o.visible || o.locked {
                        continue;
                    }
                    let b = self.bounds_of(o);
                    let inside = if touching { b.intersect(r).area() > 0.0 || r.contains(b.center()) } else { r.contains(Point::new(b.x0, b.y0)) && r.contains(Point::new(b.x1, b.y1)) };
                    if inside {
                        v.push(o.id);
                    }
                }
            }
        }
        v
    }

    /// Transform objects by an affine in document space.
    pub fn transform_objects(&mut self, ids: &[ObjId], a: Affine) {
        for id in ids {
            if let Some(o) = self.find_mut(*id) {
                o.apply_transform(a);
            }
        }
        self.update_connectors();
    }

    /// Recompute connector endpoints attached to objects.
    pub fn update_connectors(&mut self) {
        let mut updates: Vec<(ObjId, Option<Point>, Option<Point>)> = vec![];
        for p in &self.pages {
            for o in p.all_objects() {
                o.walk(&mut |x| {
                    if let ObjKind::Connector(c) = &x.kind {
                        let mut sp = None;
                        let mut ep = None;
                        if let Some((oid, anchor)) = c.start.attached {
                            if let Some(t) = self.find(oid) {
                                let b = self.bounds_of(t);
                                sp = Some(Point::new(b.x0 + anchor.x * b.width(), b.y0 + anchor.y * b.height()));
                            }
                        }
                        if let Some((oid, anchor)) = c.end.attached {
                            if let Some(t) = self.find(oid) {
                                let b = self.bounds_of(t);
                                ep = Some(Point::new(b.x0 + anchor.x * b.width(), b.y0 + anchor.y * b.height()));
                            }
                        }
                        if sp.is_some() || ep.is_some() {
                            updates.push((x.id, sp, ep));
                        }
                    }
                });
            }
        }
        for (id, sp, ep) in updates {
            if let Some(o) = self.find_mut(id) {
                if let ObjKind::Connector(c) = &mut o.kind {
                    if let Some(p) = sp {
                        c.start.point = p;
                    }
                    if let Some(p) = ep {
                        c.end.point = p;
                    }
                }
            }
        }
    }

    /// The (family, bold, italic) faces used by text objects, without duplicates.
    pub fn used_fonts(&self) -> Vec<(String, bool, bool)> {
        let mut v: Vec<(String, bool, bool)> = vec![];
        let mut add = |f: &crate::text::CharFormat| {
            let k = (f.font.clone(), f.bold, f.italic);
            if !v.contains(&k) {
                v.push(k);
            }
        };
        for p in &self.pages {
            for o in p.all_objects() {
                o.walk(&mut |x| {
                    if let ObjKind::Text(t) = &x.kind {
                        add(&t.format);
                        for s in &t.spans {
                            add(&s.format);
                        }
                    }
                });
            }
        }
        v
    }

    /// Images actually referenced by objects (for saving).
    pub fn referenced_images(&self) -> Vec<u64> {
        let mut v = vec![];
        let mut visit = |o: &Object| {
            o.walk(&mut |x| {
                if let ObjKind::Image(i) = &x.kind {
                    if !v.contains(&i.image) {
                        v.push(i.image);
                    }
                }
                if let crate::style::Fill::Pattern(p) = &x.style.fill {
                    if let Some(img) = p.image {
                        if !v.contains(&img) {
                            v.push(img);
                        }
                    }
                }
            })
        };
        for p in &self.pages {
            for o in p.all_objects() {
                visit(o);
            }
        }
        for s in &self.symbols {
            for o in &s.objects {
                visit(o);
            }
        }
        v
    }
}
