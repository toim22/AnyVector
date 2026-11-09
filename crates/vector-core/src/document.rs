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
    /// 0 a local layer; a master layer of the Master Page shown on 1 all pages, 2 odd pages,
    /// 3 even pages. Master layers are stored with the first page.
    #[serde(default)]
    pub master: u8,
}

impl Layer {
    pub fn new(id: u64, name: &str) -> Layer {
        Layer { id, name: name.to_string(), visible: true, printable: true, locked: false, color: Color::rgb(34, 31, 32), objects: vec![], master: 0 }
    }

    /// Whether this layer is drawn on page `page` (0-based): local layers on their own page,
    /// master layers on all, odd or even pages.
    pub fn shows_on(&self, page: usize) -> bool {
        match self.master {
            1 => true,
            2 => page % 2 == 0,
            3 => page % 2 == 1,
            _ => false,
        }
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

fn three() -> u32 {
    3
}

fn half_inch() -> f64 {
    36.0
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
    /// Multipage view settings: 0 grid, 1 vertical, 2 horizontal, 3 custom; the grid columns and
    /// the spacing between pages in points.
    #[serde(default)]
    pub multipage_arrangement: u8,
    #[serde(default = "three")]
    pub multipage_columns: u32,
    #[serde(default = "half_inch")]
    pub multipage_spacing: f64,
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
    /// Layout ▸ Page Number Settings: start at number, start on page (1-based) and style
    /// (0 = 1, 2, 3; 1 = I, II, III; 2 = i, ii, iii; 3 = A, B, C; 4 = a, b, c).
    #[serde(default = "one")]
    pub page_number_start: u32,
    #[serde(default = "one")]
    pub page_number_from: u32,
    #[serde(default)]
    pub page_number_style: u8,
}

fn one() -> u32 {
    1
}

fn roman(mut n: u32) -> String {
    const T: [(u32, &str); 13] = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut s = String::new();
    for (v, r) in T {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

fn letters(mut n: u32) -> String {
    let mut s = vec![];
    while n > 0 {
        n -= 1;
        s.push(b'A' + (n % 26) as u8);
        n /= 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// The page number shown on page `index` (0-based); empty before the "start on page" page.
pub fn page_number_label(index: usize, start: u32, from: u32, style: u8) -> String {
    let pos = index as u32 + 1;
    if pos < from.max(1) {
        return String::new();
    }
    let n = start.max(1) + (pos - from.max(1));
    match style {
        1 => roman(n),
        2 => roman(n).to_lowercase(),
        3 => letters(n),
        4 => letters(n).to_lowercase(),
        _ => n.to_string(),
    }
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
            multipage_arrangement: 0,
            multipage_columns: 3,
            multipage_spacing: 36.0,
            show_baseline_grid: false,
            baseline_spacing: default_baseline_spacing(),
            snap_to_baseline: false,
            snap_to_pixels: false,
            show_printable_area: false,
            page_number_start: 1,
            page_number_from: 1,
            page_number_style: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymbolDef {
    pub id: u64,
    pub name: String,
    pub objects: Vec<Object>,
    /// The symbol library file this symbol is linked to (Symbol ▸ Update from Link).
    #[serde(default)]
    pub link: Option<String>,
}

/// A pattern made with Object ▸ Create ▸ Pattern Fill.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedPattern {
    pub name: String,
    pub fill: crate::style::PatternFill,
    /// The objects a vector pattern was made from, with the tile's top-left corner at (0, 0):
    /// what the pattern document shows when the pattern is edited.
    #[serde(default)]
    pub source: Vec<crate::object::Object>,
}

/// A color harmony (Color Styles docker): its color styles and the harmony rule that relates
/// their hues (0 Custom, 1 Analogous - Accented, 2 Analogous, 3 Complementary, 4 Monochromatic,
/// 5 Tetrad, 6 Triad).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ColorHarmony {
    pub name: String,
    #[serde(default)]
    pub rule: u8,
    #[serde(default)]
    pub styles: Vec<NamedColor>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectStyle {
    pub name: String,
    pub style: Style,
    /// 0 a style set (outline, fill and transparency); a style of 1 the outline, 2 the fill,
    /// 3 the transparency.
    #[serde(default)]
    pub kind: u8,
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
    /// The mark: 0 note, 1 arrow, 2 highlighter, 3 freehand, 4 rectangle, 5 ellipse.
    #[serde(default)]
    pub mark: u8,
    /// The mark's points after `pos`: the arrow's end, the stroke, the opposite corner.
    #[serde(default)]
    pub pts: Vec<Point>,
    #[serde(default = "comment_color")]
    pub color: Color,
    #[serde(default = "comment_width")]
    pub width: f64,
    /// Fill of a rectangle or ellipse mark.
    #[serde(default)]
    pub fill: Option<Color>,
    #[serde(default)]
    pub date: String,
}

fn comment_color() -> Color {
    Color::rgb(0xe5, 0x2b, 0x2b)
}

fn comment_width() -> f64 {
    1.5
}

impl Comment {
    /// A comment of `mark` at `pts` (the first point is the position), in the mark's default style.
    pub fn new(id: u64, page: u64, author: String, mark: u8, pts: &[Point]) -> Comment {
        let (color, width) = if mark == 2 { (Color::rgb(0xff, 0xe6, 0x00), 12.0) } else { (comment_color(), comment_width()) };
        Comment { id, author, text: String::new(), pos: pts.first().copied().unwrap_or_default(), page, resolved: false, replies: vec![], mark, pts: pts.iter().skip(1).copied().collect(), color, width, fill: None, date: String::new() }
    }
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
    /// Color harmonies: folders of color styles that change together.
    #[serde(default)]
    pub color_harmonies: Vec<ColorHarmony>,
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
    /// Arrowheads made with Object ▸ Create ▸ Arrowhead that this document uses.
    #[serde(default)]
    pub arrowheads: Vec<crate::style::CustomArrow>,
    /// Patterns made with Object ▸ Create ▸ Pattern Fill.
    #[serde(default)]
    pub patterns: Vec<NamedPattern>,
    /// Externally linked images: image id → the source file (Sources docker).
    #[serde(default)]
    pub links: BTreeMap<u64, String>,
    /// The color mode of bitmaps converted by Bitmaps ▸ Mode (image id → mode index; RGB when
    /// absent).
    #[serde(default)]
    pub image_modes: BTreeMap<u64, u8>,
    /// Bitmaps ▸ Inflate Bitmap ▸ Auto Inflate Bitmap turned off: a bitmap's effects stay
    /// inside its edges.
    #[serde(default)]
    pub bitmap_no_inflate: bool,
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
            color_harmonies: vec![],
            object_styles: vec![],
            symbols: vec![],
            images: BTreeMap::new(),
            master_guides: vec![],
            comments: vec![],
            views: vec![],
            data_fields: vec!["Name".into(), "Cost".into(), "Comments".into(), "CDRStaticID".into()],
            print_merge: None,
            arrowheads: vec![],
            patterns: vec![],
            links: BTreeMap::new(),
            image_modes: BTreeMap::new(),
            bitmap_no_inflate: false,
            next_id: 1,
        };
        let pid = d.new_id();
        let lid = d.new_id();
        d.pages.push(Page::new(pid, lid, w, h));
        d
    }

    /// The id the next new object gets.
    pub fn next_id_value(&self) -> u64 {
        self.next_id
    }

    /// New ids start at `n` or later (ids taken over from another document stay unique).
    pub fn reserve_ids_below(&mut self, n: u64) {
        self.next_id = self.next_id.max(n);
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
            .map(|pg| pg.layers.iter().filter(|l| l.master == 0).map(|l| (l.name.clone(), l.visible, l.printable, l.locked)).collect())
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

    /// Refresh the page number text objects after pages were added, moved or deleted, or the
    /// Page Number Settings changed. Returns true when a number changed.
    /// The layers drawn on a page, bottom to top, with where each is stored (page, layer index):
    /// the page's own layers, then the master layers that show on it (master-page content sits on
    /// top).
    pub fn layers_on_page(&self, page: usize) -> Vec<(usize, usize, &Layer)> {
        let mut v: Vec<(usize, usize, &Layer)> = vec![];
        if let Some(pg) = self.pages.get(page) {
            v.extend(pg.layers.iter().enumerate().filter(|(_, l)| l.master == 0).map(|(i, l)| (page, i, l)));
        }
        if let Some(first) = self.pages.first() {
            v.extend(first.layers.iter().enumerate().filter(|(_, l)| l.shows_on(page)).map(|(i, l)| (0, i, l)));
        }
        v
    }

    /// The master layers (stored with the first page): (layer index, layer).
    pub fn master_layers(&self) -> Vec<(usize, &Layer)> {
        self.pages.first().map(|p| p.layers.iter().enumerate().filter(|(_, l)| l.master != 0).collect()).unwrap_or_default()
    }

    /// Remove the master layers from every page (before pages are deleted); put them back with
    /// [`Document::restore_master_layers`].
    pub fn take_master_layers(&mut self) -> Vec<Layer> {
        let mut v = vec![];
        for p in self.pages.iter_mut() {
            let (m, local): (Vec<Layer>, Vec<Layer>) = std::mem::take(&mut p.layers).into_iter().partition(|l| l.master != 0);
            p.layers = local;
            v.extend(m);
        }
        v
    }

    pub fn restore_master_layers(&mut self, layers: Vec<Layer>) {
        if let Some(p) = self.pages.first_mut() {
            p.layers.extend(layers);
        }
    }

    /// Keep the master layers with the first page after pages were reordered.
    pub fn fix_master_layers(&mut self) {
        if self.pages.iter().skip(1).any(|p| p.layers.iter().any(|l| l.master != 0)) {
            let m = self.take_master_layers();
            self.restore_master_layers(m);
        }
    }

    /// Add a master layer shown on 1 all, 2 odd or 3 even pages; returns its index on the first
    /// page.
    pub fn new_master_layer(&mut self, kind: u8) -> usize {
        let id = self.new_id();
        let n = self.pages.first().map(|p| p.layers.len()).unwrap_or(0) + 1;
        let which = match kind {
            2 => "odd pages",
            3 => "even pages",
            _ => "all pages",
        };
        let mut l = Layer::new(id, &format!("Layer {n} ({which})"));
        l.master = kind.clamp(1, 3);
        l.color = Color::rgb(0, 0, 255);
        let p = &mut self.pages[0];
        p.layers.push(l);
        p.layers.len() - 1
    }

    /// The page number text of page `page` (Layout ▸ Page Number Settings).
    pub fn page_number_text(&self, page: usize) -> String {
        page_number_label(page, self.settings.page_number_start, self.settings.page_number_from, self.settings.page_number_style)
    }

    /// A master-layer object as drawn on page `page`: page numbers show that page's number.
    pub fn object_on_page<'a>(&self, o: &'a Object, page: usize) -> std::borrow::Cow<'a, Object> {
        let mut has = false;
        o.walk(&mut |x| {
            if let crate::object::ObjKind::Text(t) = &x.kind {
                has |= t.page_number;
            }
        });
        if !has || page == 0 {
            return std::borrow::Cow::Borrowed(o);
        }
        let label = self.page_number_text(page);
        let mut c = o.clone();
        c.walk_mut(&mut |x| {
            if let crate::object::ObjKind::Text(t) = &mut x.kind {
                if t.page_number {
                    t.content = label.clone();
                    let n = t.content.chars().count();
                    for sp in &mut t.spans {
                        sp.start = sp.start.min(n);
                        sp.end = sp.end.min(n);
                    }
                    t.spans.retain(|sp| sp.start < sp.end);
                }
            }
        });
        std::borrow::Cow::Owned(c)
    }

    pub fn update_page_numbers(&mut self) -> bool {
        self.fix_master_layers();
        let (start, from, style) = (self.settings.page_number_start, self.settings.page_number_from, self.settings.page_number_style);
        let mut changed = false;
        for (i, pg) in self.pages.iter_mut().enumerate() {
            let label = page_number_label(i, start, from, style);
            for l in &mut pg.layers {
                for o in &mut l.objects {
                    o.walk_mut(&mut |x| {
                        if let crate::object::ObjKind::Text(t) = &mut x.kind {
                            if t.page_number && t.content != label {
                                t.content = label.clone();
                                let n = t.content.chars().count();
                                for sp in &mut t.spans {
                                    sp.start = sp.start.min(n);
                                    sp.end = sp.end.min(n);
                                }
                                t.spans.retain(|sp| sp.start < sp.end);
                                changed = true;
                            }
                        }
                    });
                }
            }
        }
        changed
    }

    /// Text ▸ Paragraph Text Frame ▸ Link: each frame after the first of a chain shows the text the frames
    /// before it cannot hold. Also places the baseline grid in frames aligned to it (Text ▸ Align to
    /// Baseline Grid).
    pub fn update_text_flow(&mut self) {
        self.update_text_wrap();
        self.update_text_flow_links();
    }

    /// Wrap Paragraph Text: give every paragraph text the outlines of the objects on its page that
    /// it flows around.
    pub fn update_text_wrap(&mut self) {
        use crate::text::TextKind;
        for pg in self.pages.iter_mut() {
            let mut walls: Vec<(ObjId, u8, kurbo::BezPath, Rect)> = vec![];
            for o in pg.all_objects() {
                let Some(w) = o.effects.wrap else { continue };
                if !o.visible {
                    continue;
                }
                let path = if w.style <= 2 {
                    let p = o.world_path();
                    if w.offset.abs() > 1e-6 {
                        crate::geom::offset_path(&p, w.offset, kurbo::Join::Round)
                    } else {
                        p
                    }
                } else {
                    crate::geom::rect_path(o.bounds().inflate(w.offset, w.offset))
                };
                let bb = path.bounding_box();
                walls.push((o.id, w.style, path, bb));
            }
            for l in pg.layers.iter_mut() {
                for o in l.objects.iter_mut() {
                    o.walk_mut(&mut |x| {
                        let id = x.id;
                        let tr = x.transform;
                        let ObjKind::Text(t) = &mut x.kind else { return };
                        let mut obs = vec![];
                        if let TextKind::Paragraph { width, height } = t.kind {
                            let frame = tr.transform_rect_bbox(Rect::new(0.0, 0.0, width, height));
                            let inv = tr.inverse();
                            for (wid, style, path, bb) in &walls {
                                if *wid == id || bb.intersect(frame).area() <= 0.0 {
                                    continue;
                                }
                                for (pts, _) in crate::geom::flatten_polylines(&(inv * path.clone()), 0.5) {
                                    if pts.len() >= 3 {
                                        obs.push((*style, pts));
                                    }
                                }
                            }
                        }
                        if t.wrap_obstacles != obs {
                            t.wrap_obstacles = obs;
                        }
                    });
                }
            }
        }
    }

    fn update_text_flow_links(&mut self) {
        use crate::text::{is_para_end, layout, Flow, Span, TextKind};
        let sp = self.settings.baseline_spacing.max(0.1);
        // links: (id, next, prev) of every text, to drop the ones whose other end is gone
        let mut links: Vec<(ObjId, Option<ObjId>, Option<ObjId>)> = vec![];
        for pg in &self.pages {
            for o in pg.all_objects() {
                o.walk(&mut |x| {
                    if let ObjKind::Text(t) = &x.kind {
                        links.push((x.id, t.link_next, t.link_prev));
                    }
                });
            }
        }
        let next_of = |id: ObjId| links.iter().find(|l| l.0 == id).map(|l| l.1);
        let prev_of = |id: ObjId| links.iter().find(|l| l.0 == id).map(|l| l.2);
        let mut heads = vec![];
        for pg in self.pages.iter_mut() {
            for l in pg.layers.iter_mut() {
                for o in l.objects.iter_mut() {
                    o.walk_mut(&mut |x| {
                        let c = x.transform.as_coeffs();
                        let id = x.id;
                        if let ObjKind::Text(t) = &mut x.kind {
                            if let Some(n) = t.link_next {
                                if prev_of(n) != Some(Some(id)) {
                                    t.link_next = None;
                                }
                            }
                            if let Some(p) = t.link_prev {
                                if next_of(p) != Some(Some(id)) {
                                    t.link_prev = None;
                                    t.flow = Flow::default();
                                }
                            }
                            if t.link_next.is_some() && t.link_prev.is_none() {
                                heads.push(id);
                            }
                            let g = if t.align_grid && matches!(t.kind, TextKind::Paragraph { .. }) {
                                let sy = c[3].abs().max(1e-6);
                                let ty = c[5];
                                let first = ((ty + 1e-6) / sp).ceil() * sp;
                                Some((sp / sy, (first - ty) / sy))
                            } else {
                                None
                            };
                            if t.grid != g {
                                t.grid = g;
                            }
                        }
                    });
                }
            }
        }
        for head in heads {
            let Some(ObjKind::Text(src)) = self.find(head).map(|o| o.kind.clone()) else { continue };
            let n = src.char_count();
            let chars: Vec<char> = src.content.chars().collect();
            let mut fit = layout(&src).fit_end;
            let mut seen = vec![head];
            let (mut next, mut prev) = (src.link_next, head);
            while let Some(id) = next {
                if seen.contains(&id) {
                    break;
                }
                seen.push(id);
                let Some(ObjKind::Text(t)) = self.find_mut(id).map(|o| &mut o.kind) else { break };
                let start = fit.min(n);
                let mut nt = src.clone();
                nt.content = chars[start..].iter().collect();
                nt.spans = src.spans.iter().filter(|s| s.end > start).map(|s| Span { start: s.start.saturating_sub(start), end: s.end - start, format: s.format.clone() }).collect();
                nt.flow = Flow { start, mid_para: start > 0 && !is_para_end(chars[start - 1]), para_index: chars[..start].iter().filter(|c| is_para_end(**c)).count() };
                nt.link_prev = Some(prev);
                // the frame keeps its own frame settings
                nt.kind = t.kind.clone();
                nt.vertical = t.vertical;
                nt.columns = t.columns;
                nt.column_gap = t.column_gap;
                nt.column_widths = t.column_widths.clone();
                nt.on_path = None;
                nt.page_number = false;
                nt.link_next = t.link_next;
                nt.align_grid = t.align_grid;
                nt.grid = t.grid;
                nt.frame_shape = t.frame_shape.clone();
                if *t != nt {
                    *t = nt;
                }
                fit = start + layout(t).fit_end;
                next = t.link_next;
                prev = id;
            }
        }
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
        for (_, _, l) in self.layers_on_page(page).into_iter().rev() {
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
        {
            for (_, _, l) in self.layers_on_page(page) {
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
        let mut dirs: Vec<(ObjId, Option<f64>, Option<f64>)> = vec![];
        let dir_of = |t: &crate::object::Object, anchor: Point| t.effects.anchors.iter().find(|a| (a.pos - anchor).hypot() < 1e-6).and_then(|a| a.dir);
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
                        let sd = c.start.attached.and_then(|(oid, a)| self.find(oid).and_then(|t| dir_of(t, a)));
                        let ed = c.end.attached.and_then(|(oid, a)| self.find(oid).and_then(|t| dir_of(t, a)));
                        if sd != c.start.dir || ed != c.end.dir {
                            dirs.push((x.id, sd, ed));
                        }
                    }
                });
            }
        }
        for (id, sd, ed) in dirs {
            if let Some(o) = self.find_mut(id) {
                if let ObjKind::Connector(c) = &mut o.kind {
                    c.start.dir = sd;
                    c.end.dir = ed;
                }
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
