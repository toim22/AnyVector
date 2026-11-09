//! Text objects, the system font database and text layout (shaping with rustybuzz,
//! glyph outlines through ttf-parser).

use crate::color::Color;
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
    ForceJustify,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharFormat {
    pub font: String,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// None = use the object's fill.
    pub color: Option<Color>,
    /// Extra spacing between characters, % of space width.
    pub tracking: f64,
    /// Baseline shift in points (positive = up).
    #[serde(default)]
    pub baseline_shift: f64,
    #[serde(default)]
    pub caps: bool,
    /// Text ▸ Writing Tools ▸ Language: the language the text is tagged with (a BCP 47 tag; empty = the
    /// default language).
    #[serde(default)]
    pub lang: String,
}

impl Default for CharFormat {
    fn default() -> Self {
        CharFormat {
            font: default_font_family(),
            size: 24.0,
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            color: None,
            tracking: 0.0,
            baseline_shift: 0.0,
            caps: false,
            lang: String::new(),
        }
    }
}

pub fn default_font_family() -> String {
    "Arial".to_string()
}

/// A run of characters [start, end) (char indices) with its own format.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub format: CharFormat,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TextKind {
    /// Artistic text: anchored at the origin (baseline of the first line).
    Artistic,
    /// Paragraph text inside a frame of the given size (top-left at the origin).
    Paragraph { width: f64, height: f64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextObj {
    pub content: String,
    pub kind: TextKind,
    pub format: CharFormat,
    #[serde(default)]
    pub spans: Vec<Span>,
    pub align: TextAlign,
    /// Line spacing, % of character height.
    pub line_spacing: f64,
    /// Space after paragraph, % of character height.
    #[serde(default)]
    pub para_spacing: f64,
    /// Word spacing, %.
    #[serde(default = "hundred")]
    pub word_spacing: f64,
    #[serde(default)]
    pub first_indent: f64,
    #[serde(default)]
    pub left_indent: f64,
    #[serde(default)]
    pub right_indent: f64,
    #[serde(default)]
    pub vertical: bool,
    #[serde(default)]
    pub columns: u32,
    #[serde(default)]
    pub column_gap: f64,
    /// Text on a path: the path in object-local coordinates, plus offsets.
    #[serde(default)]
    pub on_path: Option<TextOnPath>,
    /// Layout ▸ Insert Page Number: the content is the page's number, kept up to date.
    #[serde(default)]
    pub page_number: bool,
    /// Text ▸ Tabs: the tab stops (None = one every half inch).
    #[serde(default)]
    pub tabs: Option<Vec<TabStop>>,
    /// Text ▸ Tabs ▸ Leader Options.
    #[serde(default)]
    pub leader: Leader,
    /// Text ▸ Columns with columns of their own widths: (width, gutter after) per column; empty = equal
    /// columns.
    #[serde(default)]
    pub column_widths: Vec<(f64, f64)>,
    /// Text ▸ Bullets and Numbering.
    #[serde(default)]
    pub list: Option<ListFormat>,
    /// Text ▸ Drop Cap.
    #[serde(default)]
    pub drop_cap: Option<DropCap>,
    /// Text ▸ Use Hyphenation.
    #[serde(default)]
    pub hyphenate: bool,
    #[serde(default)]
    pub hyphenation: Hyphenation,
    /// Text ▸ Paragraph Text Frame ▸ Link: the frame the text flows on to, and the one it comes from.
    #[serde(default)]
    pub link_next: Option<u64>,
    #[serde(default)]
    pub link_prev: Option<u64>,
    /// Where a linked frame's text starts in the chain (kept by `Document::update_text_flow`).
    #[serde(default)]
    pub flow: Flow,
    /// Text ▸ Align to Baseline Grid.
    #[serde(default)]
    pub align_grid: bool,
    /// The baseline grid while aligned to it: spacing and the first grid line below the frame top (kept by
    /// `Document::update_text_flow`).
    #[serde(default)]
    pub grid: Option<(f64, f64)>,
    /// Text ▸ Make Text Web-compatible.
    #[serde(default)]
    pub web: bool,
    /// A frame made from a shape (Paragraph Text Frame ▸ Create Empty Text Frame): the outline the lines
    /// fit in, in frame coordinates.
    #[serde(default)]
    pub frame_shape: Option<crate::path::NodePath>,
}

/// Formatting codes (Text ▸ Insert Formatting Code).
pub const COLUMN_BREAK: char = '\u{000C}';
pub const SOFT_HYPHEN: char = '\u{00AD}';
pub const NB_HYPHEN: char = '\u{2011}';
pub const NBSP: char = '\u{00A0}';
pub const EM_SPACE: char = '\u{2003}';
pub const EN_SPACE: char = '\u{2002}';
pub const QUARTER_EM_SPACE: char = '\u{2005}';
pub const EM_DASH: char = '\u{2014}';
pub const EN_DASH: char = '\u{2013}';

/// Paragraph ends: a new line or a column/frame break.
pub fn is_para_end(c: char) -> bool {
    c == '\n' || c == COLUMN_BREAK
}

/// The space every half inch that a tab moves to when no tab stops are set.
pub const DEFAULT_TAB: f64 = 36.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TabAlign {
    #[default]
    Left,
    Right,
    Center,
    Decimal,
}

impl TabAlign {
    pub const ALL: [TabAlign; 4] = [TabAlign::Left, TabAlign::Right, TabAlign::Center, TabAlign::Decimal];
    pub fn name(self) -> &'static str {
        match self {
            TabAlign::Left => "Left",
            TabAlign::Right => "Right",
            TabAlign::Center => "Center",
            TabAlign::Decimal => "Decimal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabStop {
    /// Distance from the left of the frame (points).
    pub pos: f64,
    #[serde(default)]
    pub align: TabAlign,
    #[serde(default)]
    pub leader: bool,
}

/// Tab leaders: the character repeated across the tab and the space between two of them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Leader {
    pub ch: char,
    /// Characters of space between two leader characters.
    pub spacing: u32,
}

impl Default for Leader {
    fn default() -> Self {
        Leader { ch: '.', spacing: 1 }
    }
}

/// Text ▸ Bullets and Numbering.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListFormat {
    /// Numbers instead of bullets.
    pub numbers: bool,
    /// List level, 1 to 9.
    pub level: u8,
    pub use_para_font: bool,
    pub font: String,
    pub glyph: char,
    /// Index into `NUMBER_STYLES`.
    pub style: u8,
    pub prefix: String,
    pub suffix: String,
    /// Bullet or number size when the paragraph font is not used (points).
    pub size: f64,
    pub baseline_shift: f64,
    /// From the bullet or number to the text (points).
    pub glyph_to_text: f64,
    /// From the frame to the bullet or number (points).
    pub frame_to_list: f64,
    /// Wrapped lines start under the text, not under the bullet.
    pub align_lines: bool,
}

impl Default for ListFormat {
    fn default() -> Self {
        ListFormat {
            numbers: false,
            level: 1,
            use_para_font: true,
            font: default_font_family(),
            glyph: '•',
            style: 0,
            prefix: String::new(),
            suffix: ".".into(),
            size: 12.0,
            baseline_shift: 0.0,
            glyph_to_text: 18.0,
            frame_to_list: 0.0,
            align_lines: true,
        }
    }
}

/// The numbering styles of Bullets and Numbering, in the order of its Style list.
pub const NUMBER_STYLES: &[&str] = &[
    "1, 2, 3, ...",
    "1, 1.1, 1.1.1, ...",
    "i, ii, iii, ...",
    "I, II, III, ...",
    "a, b, c, ...",
    "A, B, C, ...",
    "१, २, ३, ...",
    "一, 二, 三, ...",
    "壱, 弐, 参, ...",
    "壹, 貳, 參, ...",
    "甲, 乙, 丙, ...",
    "子, 丑, 寅, ...",
    "ア, イ, ウ, ...",
    "イ, ロ, ハ, ...",
    "01, 02, 03, ...",
    "가, 나, 다, ...",
];

fn roman(mut n: usize) -> String {
    const R: [(usize, &str); 13] = [(1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"), (50, "l"), (40, "xl"), (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i")];
    let mut s = String::new();
    for (v, r) in R {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

/// 1 -> a, 26 -> z, 27 -> aa (with the given letters).
fn alpha(n: usize, set: &[char]) -> String {
    let k = set.len();
    let mut n = n;
    let mut s = vec![];
    while n > 0 {
        n -= 1;
        s.push(set[n % k]);
        n /= k;
    }
    s.iter().rev().collect()
}

fn cjk_number(n: usize, digits: &[char], ten: char) -> String {
    if n == 0 {
        return String::new();
    }
    if n < 10 {
        return digits[n - 1].to_string();
    }
    if n < 100 {
        let (t, u) = (n / 10, n % 10);
        let mut s = String::new();
        if t > 1 {
            s.push(digits[t - 1]);
        }
        s.push(ten);
        if u > 0 {
            s.push(digits[u - 1]);
        }
        return s;
    }
    n.to_string()
}

/// The label of item `n` (from 1) in a numbering style; `level` repeats the parent numbers for the
/// "1, 1.1, 1.1.1" style.
pub fn number_label(style: u8, n: usize, level: u8) -> String {
    let n = n.max(1);
    let chars = |s: &str| s.chars().collect::<Vec<_>>();
    match style {
        1 => {
            let mut s = String::new();
            for _ in 1..level.max(1) {
                s.push_str("1.");
            }
            s + &n.to_string()
        }
        2 => roman(n),
        3 => roman(n).to_uppercase(),
        4 => alpha(n, &chars("abcdefghijklmnopqrstuvwxyz")),
        5 => alpha(n, &chars("ABCDEFGHIJKLMNOPQRSTUVWXYZ")),
        6 => n.to_string().chars().map(|c| char::from_u32(0x0966 + c.to_digit(10).unwrap_or(0)).unwrap_or(c)).collect(),
        7 => cjk_number(n, &chars("一二三四五六七八九"), '十'),
        8 => cjk_number(n, &chars("壱弐参四伍六七八九"), '拾'),
        9 => cjk_number(n, &chars("壹貳參肆伍陸柒捌玖"), '拾'),
        10 => alpha(n, &chars("甲乙丙丁戊己庚辛壬癸")),
        11 => alpha(n, &chars("子丑寅卯辰巳午未申酉戌亥")),
        12 => alpha(n, &chars("アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン")),
        13 => alpha(n, &chars("イロハニホヘトチリヌルヲワカヨタレソツネナラムウヰノオクヤマケフコエテアサキユメミシヱヒモセス")),
        14 => format!("{n:02}"),
        15 => alpha(n, &chars("가나다라마바사아자차카타파하")),
        _ => n.to_string(),
    }
}

/// Text ▸ Drop Cap.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DropCap {
    /// Number of lines dropped.
    pub lines: u32,
    /// Space after the drop cap (points).
    pub space: f64,
    /// The whole paragraph is indented beside the drop cap.
    pub hanging: bool,
}

impl Default for DropCap {
    fn default() -> Self {
        DropCap { lines: 3, space: 0.0, hanging: false }
    }
}

/// Hyphenation settings of paragraph text.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hyphenation {
    pub break_caps: bool,
    pub break_all_caps: bool,
    pub min_word: u32,
    pub min_before: u32,
    pub min_after: u32,
    /// Hot zone: words starting this close to the right edge move to the next line whole (points).
    pub hot_zone: f64,
}

impl Default for Hyphenation {
    fn default() -> Self {
        Hyphenation { break_caps: true, break_all_caps: false, min_word: 5, min_before: 2, min_after: 2, hot_zone: 36.0 }
    }
}

/// Where a linked frame's text starts in the text of its chain.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Flow {
    /// Character index in the first frame's text.
    pub start: usize,
    /// The frame starts in the middle of a paragraph (no bullet or drop cap for it).
    pub mid_para: bool,
    /// Paragraphs before this frame (list numbers go on from there).
    pub para_index: usize,
}

fn hundred() -> f64 {
    100.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextOnPath {
    pub path: crate::path::NodePath,
    pub offset: f64,
    pub distance: f64,
    pub flip: bool,
}

impl TextObj {
    pub fn artistic(s: &str, format: CharFormat) -> TextObj {
        TextObj {
            content: s.to_string(),
            kind: TextKind::Artistic,
            format,
            spans: vec![],
            align: TextAlign::Left,
            line_spacing: 100.0,
            para_spacing: 0.0,
            word_spacing: 100.0,
            first_indent: 0.0,
            left_indent: 0.0,
            right_indent: 0.0,
            vertical: false,
            columns: 1,
            column_gap: 12.0,
            on_path: None,
            page_number: false,
            tabs: None,
            leader: Leader::default(),
            column_widths: vec![],
            list: None,
            drop_cap: None,
            hyphenate: false,
            hyphenation: Hyphenation::default(),
            link_next: None,
            link_prev: None,
            flow: Flow::default(),
            align_grid: false,
            grid: None,
            web: false,
            frame_shape: None,
        }
    }
    pub fn paragraph(s: &str, w: f64, h: f64, format: CharFormat) -> TextObj {
        let mut t = TextObj::artistic(s, format);
        t.kind = TextKind::Paragraph { width: w, height: h };
        t
    }

    pub fn char_count(&self) -> usize {
        self.content.chars().count()
    }

    /// Format for character `i` (spans override the default).
    pub fn format_at(&self, i: usize) -> &CharFormat {
        for s in self.spans.iter().rev() {
            if i >= s.start && i < s.end {
                return &s.format;
            }
        }
        &self.format
    }

    /// Apply a format change to a char range.
    pub fn apply_format(&mut self, start: usize, end: usize, f: impl Fn(&mut CharFormat)) {
        if start >= end {
            return;
        }
        if start == 0 && end >= self.char_count() {
            f(&mut self.format);
            for s in self.spans.iter_mut() {
                f(&mut s.format);
            }
            return;
        }
        // split into per-char formats and rebuild spans
        let n = self.char_count();
        let mut per: Vec<CharFormat> = (0..n).map(|i| self.format_at(i).clone()).collect();
        for fmt in per.iter_mut().take(end.min(n)).skip(start) {
            f(fmt);
        }
        self.rebuild_spans(per);
    }

    fn rebuild_spans(&mut self, per: Vec<CharFormat>) {
        self.spans.clear();
        let mut i = 0;
        while i < per.len() {
            let mut j = i + 1;
            while j < per.len() && per[j] == per[i] {
                j += 1;
            }
            if per[i] != self.format {
                self.spans.push(Span { start: i, end: j, format: per[i].clone() });
            }
            i = j;
        }
    }

    /// Insert text at char index, keeping span positions consistent.
    pub fn insert(&mut self, at: usize, s: &str) {
        let byte = char_to_byte(&self.content, at);
        self.content.insert_str(byte, s);
        let k = s.chars().count();
        for sp in self.spans.iter_mut() {
            if sp.start >= at && at != 0 || sp.start > at {
                sp.start += k;
                sp.end += k;
            } else if sp.end >= at {
                sp.end += k;
            }
        }
    }

    pub fn delete(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let b0 = char_to_byte(&self.content, start);
        let b1 = char_to_byte(&self.content, end);
        self.content.replace_range(b0..b1, "");
        let k = end - start;
        for sp in self.spans.iter_mut() {
            let adj = |x: usize| if x <= start { x } else if x >= end { x - k } else { start };
            sp.start = adj(sp.start);
            sp.end = adj(sp.end);
        }
        self.spans.retain(|s| s.end > s.start);
    }
}

pub fn char_to_byte(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

// ---------------------------------------------------------------------------------------------
// Font database
// ---------------------------------------------------------------------------------------------

pub struct FontDb {
    pub db: fontdb::Database,
    families: Vec<String>,
    face_cache: HashMap<fontdb::ID, Arc<FaceData>>,
    glyph_cache: HashMap<(fontdb::ID, u16), Arc<BezPath>>,
}

pub struct FaceData {
    pub data: Arc<Vec<u8>>,
    pub index: u32,
    pub units_per_em: f64,
    pub ascender: f64,
    pub descender: f64,
    pub line_gap: f64,
    pub underline_pos: f64,
    pub underline_thick: f64,
    pub cap_height: f64,
}

static FONTS: OnceLock<Mutex<FontDb>> = OnceLock::new();

pub fn fonts() -> &'static Mutex<FontDb> {
    FONTS.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let mut fam: Vec<String> = db
            .faces()
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        fam.sort_by_key(|s| s.to_lowercase());
        fam.dedup();
        Mutex::new(FontDb { db, families: fam, face_cache: HashMap::new(), glyph_cache: HashMap::new() })
    })
}

/// A font file used by a document, for embedding when saving.
#[derive(Clone, Debug)]
pub struct FontFile {
    /// File name of the font (e.g. "arial.ttf").
    pub name: String,
    pub data: Arc<Vec<u8>>,
    /// False when the font's license forbids embedding (OS/2 fsType "restricted").
    pub embeddable: bool,
}

/// The font files behind the given (family, bold, italic) faces, without duplicates.
pub fn font_files(faces: &[(String, bool, bool)]) -> Vec<FontFile> {
    let db = fonts().lock();
    let mut out: Vec<FontFile> = Vec::new();
    for (family, bold, italic) in faces {
        let Some(id) = db.query(family, *bold, *italic) else { continue };
        let Some(face) = db.db.face(id) else { continue };
        let name = match &face.source {
            fontdb::Source::File(p) => p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            _ => format!("{}.ttf", family),
        };
        if out.iter().any(|f| f.name.eq_ignore_ascii_case(&name)) {
            continue;
        }
        let file = db.db.with_face_data(id, |data, index| {
            let embeddable = ttf_parser::Face::parse(data, index).ok().and_then(|f| f.permissions()).map(|p| p != ttf_parser::Permissions::Restricted).unwrap_or(true);
            (Arc::new(data.to_vec()), embeddable)
        });
        if let Some((data, embeddable)) = file {
            out.push(FontFile { name, data, embeddable });
        }
    }
    out
}

/// Start loading the font database on a background thread.
pub fn preload_fonts() {
    std::thread::spawn(|| {
        let _ = fonts();
    });
}

impl FontDb {
    pub fn families(&self) -> &[String] {
        &self.families
    }

    pub fn query(&self, family: &str, bold: bool, italic: bool) -> Option<fontdb::ID> {
        let fams = [fontdb::Family::Name(family), fontdb::Family::SansSerif];
        let q = fontdb::Query {
            families: &fams,
            weight: if bold { fontdb::Weight::BOLD } else { fontdb::Weight::NORMAL },
            style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
            stretch: fontdb::Stretch::Normal,
        };
        self.db.query(&q).or_else(|| self.db.faces().next().map(|f| f.id))
    }

    pub fn face(&mut self, id: fontdb::ID) -> Option<Arc<FaceData>> {
        if let Some(f) = self.face_cache.get(&id) {
            return Some(f.clone());
        }
        let fd = self.db.with_face_data(id, |data, index| {
            let face = ttf_parser::Face::parse(data, index).ok()?;
            let upem = face.units_per_em() as f64;
            let ul = face.underline_metrics();
            Some(FaceData {
                data: Arc::new(data.to_vec()),
                index,
                units_per_em: upem,
                ascender: face.ascender() as f64,
                descender: face.descender() as f64,
                line_gap: face.line_gap() as f64,
                underline_pos: ul.map(|u| u.position as f64).unwrap_or(-upem * 0.1),
                underline_thick: ul.map(|u| u.thickness as f64).unwrap_or(upem * 0.05),
                cap_height: face.capital_height().map(|h| h as f64).filter(|h| *h > 0.0).unwrap_or(upem * 0.7),
            })
        })??;
        let fd = Arc::new(fd);
        self.face_cache.insert(id, fd.clone());
        Some(fd)
    }

    fn glyph(&mut self, id: fontdb::ID, face: &FaceData, gid: u16) -> Arc<BezPath> {
        if let Some(g) = self.glyph_cache.get(&(id, gid)) {
            return g.clone();
        }
        let mut b = OutlineBuilder(BezPath::new());
        if let Ok(f) = ttf_parser::Face::parse(&face.data, face.index) {
            f.outline_glyph(ttf_parser::GlyphId(gid), &mut b);
        }
        let g = Arc::new(b.0);
        self.glyph_cache.insert((id, gid), g.clone());
        g
    }
}

struct OutlineBuilder(BezPath);

impl ttf_parser::OutlineBuilder for OutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((x as f64, -y as f64));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((x as f64, -y as f64));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.quad_to((x1 as f64, -y1 as f64), (x as f64, -y as f64));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.curve_to((x1 as f64, -y1 as f64), (x2 as f64, -y2 as f64), (x as f64, -y as f64));
    }
    fn close(&mut self) {
        self.0.close_path();
    }
}

// ---------------------------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------------------------

/// One laid-out glyph.
#[derive(Clone, Debug)]
pub struct LaidGlyph {
    /// Index of the source character.
    pub char_index: usize,
    /// Pen position (baseline origin) in object-local coordinates.
    pub origin: Point,
    pub advance: f64,
    pub size: f64,
    /// Glyph outline already positioned in local coordinates.
    pub path: Arc<BezPath>,
    pub transform: Affine,
    pub color: Option<Color>,
    pub line: usize,
}

#[derive(Clone, Debug, Default)]
pub struct LaidLine {
    pub baseline: f64,
    pub ascent: f64,
    pub descent: f64,
    pub x0: f64,
    pub x1: f64,
    pub first_char: usize,
    pub end_char: usize,
    /// The line is past the bottom of its frame (overflow text): it is not drawn.
    pub hidden: bool,
    pub column: usize,
}

#[derive(Clone, Debug, Default)]
pub struct TextLayout {
    pub glyphs: Vec<LaidGlyph>,
    pub lines: Vec<LaidLine>,
    /// Decorations (underline/strike) as filled rectangles with their colors.
    pub decorations: Vec<(Rect, Option<Color>)>,
    pub bounds: Rect,
    /// Char index -> caret position (x, baseline, line).
    pub carets: Vec<(f64, f64, usize)>,
    pub overflow: bool,
    /// The first character that does not fit in the frame (the text length when all of it fits).
    pub fit_end: usize,
}

impl TextLayout {
    /// The whole text as a single path (glyphs + decorations) for one color group.
    pub fn to_path(&self) -> BezPath {
        let mut bp = BezPath::new();
        for g in &self.glyphs {
            bp.extend((g.transform * (*g.path).clone()).elements().iter().copied());
        }
        for (r, _) in &self.decorations {
            bp.extend(r.to_path(0.1).elements().iter().copied());
        }
        bp
    }

    /// Paths grouped by color override (None = object fill).
    pub fn colored_paths(&self) -> Vec<(Option<Color>, BezPath)> {
        let mut groups: Vec<(Option<Color>, BezPath)> = vec![];
        let mut push = |c: Option<Color>, els: &BezPath| {
            if let Some(g) = groups.iter_mut().find(|g| g.0 == c) {
                g.1.extend(els.elements().iter().copied());
            } else {
                groups.push((c, els.clone()));
            }
        };
        for g in &self.glyphs {
            push(g.color, &(g.transform * (*g.path).clone()));
        }
        for (r, c) in &self.decorations {
            push(*c, &r.to_path(0.1));
        }
        groups
    }

    /// Char index nearest to a local point.
    pub fn hit(&self, p: Point) -> usize {
        if self.carets.is_empty() {
            return 0;
        }
        let any_visible = self.lines.iter().any(|l| !l.hidden);
        let mut best = 0;
        let mut bd = f64::INFINITY;
        // choose line by baseline proximity first
        let mut line = 0;
        let mut ld = f64::INFINITY;
        for (i, l) in self.lines.iter().enumerate() {
            if l.hidden && any_visible {
                continue;
            }
            let mid = l.baseline - (l.ascent - l.descent) / 2.0 + l.descent;
            let top = l.baseline - l.ascent;
            let bot = l.baseline + l.descent;
            let inside_x = p.x >= l.x0 - 4.0 && p.x <= l.x1.max(l.x0 + 8.0) + 4.0;
            let d = if p.y >= top && p.y <= bot { if inside_x { 0.0 } else { 0.5 + (p.x - (l.x0 + l.x1) / 2.0).abs() * 1e-3 } } else { (p.y - mid).abs() + 1.0 };
            if d < ld {
                ld = d;
                line = i;
            }
        }
        for (i, c) in self.carets.iter().enumerate() {
            if c.2 != line {
                continue;
            }
            let d = (c.0 - p.x).abs();
            if d < bd {
                bd = d;
                best = i;
            }
        }
        best
    }
}

struct ShapedChar {
    ch: char,
    glyphs: Vec<(u16, f64, f64, f64)>, // gid, x_advance, x_offset, y_offset (pt)
    advance: f64,
    face_id: fontdb::ID,
    face: Arc<FaceData>,
    fmt: CharFormat,
}

pub fn layout(t: &TextObj) -> TextLayout {
    let mut db = fonts().lock();
    layout_with(&mut db, t)
}

/// Shape one run of text in one face: glyphs per character.
fn shape_run(face: &FaceData, run: &str, scale: f64, n: usize) -> Vec<Vec<(u16, f64, f64, f64)>> {
    let mut per_char: Vec<Vec<(u16, f64, f64, f64)>> = vec![vec![]; n];
    if n == 0 {
        return per_char;
    }
    if let Some(rb) = rustybuzz::Face::from_slice(&face.data, face.index) {
        let mut buf = rustybuzz::UnicodeBuffer::new();
        buf.push_str(run);
        let gb = rustybuzz::shape(&rb, &[], buf);
        // cluster = byte offset in run
        let mut byte_to_char = vec![0; run.len() + 1];
        for (ci, (b, _)) in run.char_indices().enumerate() {
            byte_to_char[b] = ci;
        }
        for (info, pos) in gb.glyph_infos().iter().zip(gb.glyph_positions()) {
            let ci = byte_to_char.get(info.cluster as usize).copied().unwrap_or(0).min(n - 1);
            per_char[ci].push((info.glyph_id as u16, pos.x_advance as f64 * scale, pos.x_offset as f64 * scale, pos.y_offset as f64 * scale));
        }
    }
    per_char
}

/// Characters shaped on their own (no glyph of the font, or a width of their own).
fn is_special(c: char) -> bool {
    matches!(c, '\n' | '\t' | COLUMN_BREAK | SOFT_HYPHEN | EM_SPACE | EN_SPACE | QUARTER_EM_SPACE)
}

fn shape_text(db: &mut FontDb, t: &TextObj, chars: &[char], fallback: (fontdb::ID, Arc<FaceData>)) -> Vec<ShapedChar> {
    let mut shaped: Vec<ShapedChar> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let fmt = t.format_at(i).clone();
        let mut j = i + 1;
        if !is_special(chars[i]) {
            while j < chars.len() && t.format_at(j) == &fmt && !is_special(chars[j]) {
                j += 1;
            }
        }
        let (fid, face) = match db.query(&fmt.font, fmt.bold, fmt.italic).and_then(|id| db.face(id).map(|f| (id, f))) {
            Some(x) => x,
            None => fallback.clone(),
        };
        let scale = fmt.size / face.units_per_em;
        let run: String = chars[i..j]
            .iter()
            .map(|c| {
                let c = if *c == NB_HYPHEN { '-' } else { *c };
                if fmt.caps {
                    c.to_uppercase().next().unwrap_or(c)
                } else {
                    c
                }
            })
            .collect();
        let per_char = shape_run(&face, &run, scale, j - i);
        for (k, ci) in (i..j).enumerate() {
            let c = chars[ci];
            let space_w = fmt.size * 0.25;
            let mut adv: f64 = per_char[k].iter().map(|g| g.1).sum();
            let mut glyphs = per_char[k].clone();
            match c {
                ' ' | NBSP => adv *= t.word_spacing / 100.0,
                '\n' | '\t' | COLUMN_BREAK | SOFT_HYPHEN => {
                    adv = 0.0;
                    glyphs.clear();
                }
                EM_SPACE => {
                    adv = fmt.size;
                    glyphs.clear();
                }
                EN_SPACE => {
                    adv = fmt.size / 2.0;
                    glyphs.clear();
                }
                QUARTER_EM_SPACE => {
                    adv = fmt.size / 4.0;
                    glyphs.clear();
                }
                _ => {}
            }
            if !matches!(c, '\n' | '\t' | COLUMN_BREAK | SOFT_HYPHEN) {
                adv += fmt.tracking / 100.0 * space_w;
            }
            shaped.push(ShapedChar { ch: c, glyphs, advance: adv, face_id: fid, face: face.clone(), fmt: fmt.clone() });
        }
        i = j;
    }
    shaped
}

/// A string shaped in one format (bullets, numbers, hyphens, leaders).
struct ShapedStr {
    glyphs: Vec<(u16, f64, f64, f64)>,
    width: f64,
    face_id: fontdb::ID,
    face: Arc<FaceData>,
    size: f64,
}

fn shape_str(db: &mut FontDb, font: &str, bold: bool, italic: bool, size: f64, s: &str) -> Option<ShapedStr> {
    let fid = db.query(font, bold, italic)?;
    let face = db.face(fid)?;
    let n = s.chars().count();
    let glyphs: Vec<(u16, f64, f64, f64)> = shape_run(&face, s, size / face.units_per_em, n).into_iter().flatten().collect();
    let width = glyphs.iter().map(|g| g.1).sum();
    Some(ShapedStr { glyphs, width, face_id: fid, face, size })
}

impl FontDb {
    /// Glyphs of a shaped string placed with their pen at (x, y), scaled by `k`.
    fn emit(&mut self, out: &mut Vec<LaidGlyph>, s: &ShapedStr, x: f64, y: f64, k: f64, char_index: usize, color: Option<Color>, line: usize) {
        let scale = s.size / s.face.units_per_em * k;
        let mut gx = x;
        for &(gid, adv, xo, yo) in &s.glyphs {
            let path = self.glyph(s.face_id, &s.face, gid);
            if !path.elements().is_empty() {
                out.push(LaidGlyph {
                    char_index,
                    origin: Point::new(gx, y),
                    advance: adv * k,
                    size: s.size * k,
                    path,
                    transform: Affine::translate((gx + xo * k, y - yo * k)) * Affine::scale(scale),
                    color,
                    line,
                });
            }
            gx += adv * k;
        }
    }
}

static HYPHENATOR: OnceLock<Option<hyphenation::Standard>> = OnceLock::new();

/// The hyphenation dictionary (English, United States).
fn hyphenator() -> Option<&'static hyphenation::Standard> {
    HYPHENATOR
        .get_or_init(|| {
            use hyphenation::Load;
            hyphenation::Standard::from_embedded(hyphenation::Language::EnglishUS).ok()
        })
        .as_ref()
}

/// The places a word can be hyphenated (character counts from its start).
pub fn hyphenation_points(word: &str) -> Vec<usize> {
    use hyphenation::Hyphenator;
    let Some(dict) = hyphenator() else { return vec![] };
    let w = dict.hyphenate(word);
    w.breaks.iter().map(|b| word[..*b].chars().count()).collect()
}

/// The next tab stop after `at` (points from the left of the frame).
pub fn next_tab_stop(t: &TextObj, at: f64) -> TabStop {
    let mut last = 0.0f64;
    if let Some(tabs) = &t.tabs {
        let mut v: Vec<TabStop> = tabs.clone();
        v.sort_by(|a, b| a.pos.total_cmp(&b.pos));
        for s in &v {
            if s.pos > at + 0.01 {
                return *s;
            }
            last = last.max(s.pos);
        }
    }
    let base = at.max(last);
    TabStop { pos: ((base + 0.01) / DEFAULT_TAB).floor() * DEFAULT_TAB + DEFAULT_TAB, align: TabAlign::Left, leader: false }
}

/// The tab stops shown by Text ▸ Tabs: the ones set, or one every half inch up to `width`.
pub fn tab_list(t: &TextObj, width: f64) -> Vec<TabStop> {
    match &t.tabs {
        Some(v) => {
            let mut v = v.clone();
            v.sort_by(|a, b| a.pos.total_cmp(&b.pos));
            v
        }
        None => {
            let n = ((width / DEFAULT_TAB).floor() as usize).max(1);
            (1..=n).map(|i| TabStop { pos: i as f64 * DEFAULT_TAB, align: TabAlign::Left, leader: false }).collect()
        }
    }
}

/// Width of a tab at `at` (from the frame left), and whether it has a leader.
fn tab_advance(t: &TextObj, chars: &[char], adv: &[f64], k: usize, at: f64) -> (f64, bool) {
    let stop = next_tab_stop(t, at);
    let mut seg = 0.0;
    let mut dec: Option<f64> = None;
    let mut j = k + 1;
    while j < chars.len() && chars[j] != '\t' && !is_para_end(chars[j]) {
        if dec.is_none() && (chars[j] == '.' || chars[j] == ',') {
            dec = Some(seg);
        }
        seg += adv[j];
        j += 1;
    }
    let w = match stop.align {
        TabAlign::Left => stop.pos - at,
        TabAlign::Right => stop.pos - at - seg,
        TabAlign::Center => stop.pos - at - seg / 2.0,
        TabAlign::Decimal => stop.pos - at - dec.unwrap_or(seg),
    };
    (w.max(0.0), stop.leader)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum BreakKind {
    ParaEnd,
    ColBreak,
    Wrap,
    Hyphen,
}

struct LineBreak {
    /// Characters [start, end) are on the line.
    end: usize,
    /// First character of the next line.
    next: usize,
    kind: BreakKind,
}

fn is_break_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | EM_SPACE | EN_SPACE | QUARTER_EM_SPACE)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == SOFT_HYPHEN || c == '\'' || c == '’'
}

/// Hyphenate the word starting at `ws` (at `wx` on the line) so that its first part fits in `avail`.
fn auto_hyphen(chars: &[char], shaped: &[ShapedChar], adv: &[f64], t: &TextObj, ws: usize, wx: f64, avail: f64) -> Option<usize> {
    let h = &t.hyphenation;
    if wx > avail - h.hot_zone {
        return None;
    }
    let mut we = ws;
    while we < chars.len() && chars[we].is_alphabetic() {
        we += 1;
    }
    let len = we - ws;
    if len < h.min_word.max(2) as usize {
        return None;
    }
    let all_upper = chars[ws..we].iter().all(|c| !c.is_lowercase());
    if all_upper && !h.break_all_caps {
        return None;
    }
    if chars[ws].is_uppercase() && !all_upper && !h.break_caps {
        return None;
    }
    let word: String = chars[ws..we].iter().collect();
    let hyphen_w = shaped[ws].fmt.size * 0.33;
    let mut best = None;
    for p in hyphenation_points(&word) {
        if p < h.min_before as usize || len - p < h.min_after as usize {
            continue;
        }
        let w: f64 = adv[ws..ws + p].iter().sum();
        if wx + w + hyphen_w <= avail {
            best = Some(ws + p);
        }
    }
    best
}

#[allow(clippy::too_many_arguments)]
fn break_line(chars: &[char], shaped: &[ShapedChar], adv: &mut [f64], leader_at: &mut [bool], t: &TextObj, start: usize, avail: f64, left: f64, limit: bool) -> LineBreak {
    let n = chars.len();
    let mut x = 0.0;
    let mut k = start;
    let mut last: Option<(usize, bool)> = None;
    loop {
        if k >= n {
            return LineBreak { end: n, next: n + 1, kind: BreakKind::ParaEnd };
        }
        let c = chars[k];
        if c == '\n' {
            return LineBreak { end: k, next: k + 1, kind: BreakKind::ParaEnd };
        }
        if c == COLUMN_BREAK {
            return LineBreak { end: k, next: k + 1, kind: BreakKind::ColBreak };
        }
        if c == '\t' {
            let (w, lead) = tab_advance(t, chars, adv, k, left + x);
            adv[k] = w;
            leader_at[k] = lead;
        }
        let w = adv[k];
        if limit && x + w > avail && k > start && !is_break_space(c) && c != SOFT_HYPHEN {
            let mut ws = k;
            while ws > start && is_word_char(chars[ws - 1]) {
                ws -= 1;
            }
            // a soft hyphen inside this word
            if let Some((b, true)) = last {
                if b > ws {
                    return LineBreak { end: b, next: b, kind: BreakKind::Hyphen };
                }
            }
            if t.hyphenate && is_word_char(c) {
                let wx = x - adv[ws..k].iter().sum::<f64>();
                if let Some(b) = auto_hyphen(chars, shaped, adv, t, ws, wx, avail) {
                    if b > start {
                        return LineBreak { end: b, next: b, kind: BreakKind::Hyphen };
                    }
                }
            }
            return match last {
                Some((b, hy)) if b > start => LineBreak { end: b, next: b, kind: if hy { BreakKind::Hyphen } else { BreakKind::Wrap } },
                _ => LineBreak { end: k, next: k, kind: BreakKind::Wrap },
            };
        }
        match c {
            ' ' | '\t' | EM_SPACE | EN_SPACE | QUARTER_EM_SPACE | '-' | EN_DASH | EM_DASH => last = Some((k + 1, false)),
            SOFT_HYPHEN if !limit || x + shaped[k].fmt.size * 0.33 <= avail => last = Some((k + 1, true)),
            _ => {}
        }
        x += w;
        k += 1;
    }
}

/// The columns of a frame: (left, width).
pub fn column_boxes(t: &TextObj, frame_w: f64) -> Vec<(f64, f64)> {
    if !t.column_widths.is_empty() && matches!(t.kind, TextKind::Paragraph { .. }) {
        let mut x = 0.0;
        t.column_widths
            .iter()
            .map(|&(w, g)| {
                let b = (x, w.max(1.0));
                x += w + g;
                b
            })
            .collect()
    } else {
        let cols = t.columns.max(1) as usize;
        let w = ((frame_w - t.column_gap * (cols as f64 - 1.0)) / cols as f64).max(1.0);
        (0..cols).map(|i| (i as f64 * (w + t.column_gap), w)).collect()
    }
}

fn flatten_poly(bp: &BezPath) -> Vec<Vec<Point>> {
    let mut polys: Vec<Vec<Point>> = vec![];
    kurbo::flatten(bp.elements().iter().copied(), 0.25, |el| match el {
        kurbo::PathEl::MoveTo(p) => polys.push(vec![p]),
        kurbo::PathEl::LineTo(p) => {
            if let Some(v) = polys.last_mut() {
                v.push(p);
            }
        }
        kurbo::PathEl::ClosePath => {
            if let Some(v) = polys.last_mut() {
                if let Some(f) = v.first().copied() {
                    v.push(f);
                }
            }
        }
        _ => {}
    });
    polys
}

/// Inside intervals of the polygons on a horizontal line (even-odd).
fn scan(polys: &[Vec<Point>], y: f64) -> Vec<(f64, f64)> {
    let mut xs = vec![];
    for poly in polys {
        let n = poly.len();
        for i in 0..n {
            let a = poly[i];
            let b = poly[(i + 1) % n];
            if (a.y <= y && b.y > y) || (b.y <= y && a.y > y) {
                xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
            }
        }
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    xs.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).collect()
}

/// The widest stretch inside the shape over the band [y0, y1].
fn shape_span(polys: &[Vec<Point>], y0: f64, y1: f64) -> Option<(f64, f64)> {
    let mut cur = scan(polys, y0);
    for y in [(y0 + y1) / 2.0, y1] {
        let other = scan(polys, y);
        let mut next = vec![];
        for a in &cur {
            for b in &other {
                let (l, r) = (a.0.max(b.0), a.1.min(b.1));
                if r > l {
                    next.push((l, r));
                }
            }
        }
        cur = next;
    }
    cur.into_iter().filter(|s| s.1 - s.0 > 1.0).max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))
}

/// Line metrics (ascent, descent, size) of the characters [s, e), or of the format around an empty line.
fn line_metrics(shaped: &[ShapedChar], s: usize, e: usize, base: &FaceData, base_fmt: &CharFormat) -> (f64, f64, f64) {
    let (mut asc, mut desc, mut size) = (0.0f64, 0.0f64, 0.0f64);
    for sc in &shaped[s..e.min(shaped.len())] {
        if matches!(sc.ch, '\n' | COLUMN_BREAK) {
            continue;
        }
        let k = sc.fmt.size / sc.face.units_per_em;
        asc = asc.max(sc.face.ascender * k);
        desc = desc.max(-sc.face.descender * k);
        size = size.max(sc.fmt.size);
    }
    if size == 0.0 {
        // empty line: the format of the paragraph mark, or of the character before
        let (face, size) = if s < shaped.len() {
            (shaped[s].face.as_ref(), shaped[s].fmt.size)
        } else if s > 0 && s - 1 < shaped.len() {
            (shaped[s - 1].face.as_ref(), shaped[s - 1].fmt.size)
        } else {
            (base, base_fmt.size)
        };
        let k = size / face.units_per_em;
        return (face.ascender * k, -face.descender * k, size);
    }
    (asc, desc, size)
}

/// A paragraph's bullet or number.
struct Marker {
    shaped: ShapedStr,
    x: f64,
    text_x: f64,
    shift: f64,
    color: Option<Color>,
}

/// A paragraph's drop cap.
struct Drop {
    ci: usize,
    k: f64,
    width: f64,
}

pub fn layout_with(db: &mut FontDb, t: &TextObj) -> TextLayout {
    let chars: Vec<char> = t.content.chars().collect();
    let n = chars.len();
    let mut out = TextLayout { fit_end: n, ..Default::default() };
    let Some(base) = db.query(&t.format.font, t.format.bold, t.format.italic).and_then(|id| db.face(id).map(|f| (id, f))) else {
        out.carets = vec![(0.0, 0.0, 0); n + 1];
        out.bounds = Rect::new(0.0, -t.format.size * 0.8, 1.0, t.format.size * 0.2);
        return out;
    };
    let base_face = base.1.clone();
    let shaped = shape_text(db, t, &chars, base.clone());
    let mut adv: Vec<f64> = shaped.iter().map(|s| s.advance).collect();
    let mut leader_at = vec![false; n];

    let paragraph = matches!(t.kind, TextKind::Paragraph { .. });
    let (frame_w, frame_h) = match t.kind {
        TextKind::Paragraph { width, height } => (width, height),
        TextKind::Artistic => (0.0, 0.0),
    };
    let bk = t.format.size / base_face.units_per_em;
    let (base_asc, base_desc) = (base_face.ascender * bk, -base_face.descender * bk);
    let base_line_h = ((base_asc + base_desc) * t.line_spacing / 100.0).max(1.0);
    let columns = column_boxes(t, frame_w);
    let poly = t.frame_shape.as_ref().filter(|_| paragraph).map(|p| flatten_poly(&p.to_bez()));
    let drop_cap = t.drop_cap.filter(|d| paragraph && d.lines >= 1);

    let mut carets: Vec<(f64, f64, usize)> = vec![(0.0, 0.0, 0); n + 1];
    let mut have = vec![false; n + 1];
    let mut k = 0usize;
    let mut li = 0usize;
    let mut col = 0usize;
    let mut y = 0.0f64;
    let mut first_in_col = true;
    let mut para_line = 0usize;
    let mut para_no = t.flow.para_index;
    let mut mid_para = t.flow.mid_para;
    let mut pending_space = 0.0f64;
    let mut hidden = false;
    let mut marker: Option<Marker> = None;
    let mut drop: Option<Drop> = None;

    while k <= n {
        // --- a new paragraph: its bullet or number and its drop cap
        if para_line == 0 {
            marker = None;
            drop = None;
            let fmt = if k < n { shaped[k].fmt.clone() } else if k > 0 { shaped[k - 1].fmt.clone() } else { t.format.clone() };
            if let Some(l) = t.list.as_ref().filter(|_| !mid_para) {
                let label = if l.numbers { format!("{}{}{}", l.prefix, number_label(l.style, para_no + 1, l.level), l.suffix) } else { l.glyph.to_string() };
                let (font, size, bold, italic) = if l.use_para_font { (fmt.font.clone(), fmt.size, fmt.bold, fmt.italic) } else { (l.font.clone(), l.size, false, false) };
                if let Some(ss) = shape_str(db, &font, bold, italic, size, &label) {
                    let x = t.left_indent + l.frame_to_list + (l.level.max(1) - 1) as f64 * 18.0;
                    let text_x = x + l.glyph_to_text.max(ss.width + size * 0.25);
                    marker = Some(Marker { shaped: ss, x, text_x, shift: l.baseline_shift, color: fmt.color });
                }
            }
            if let Some(dc) = drop_cap.filter(|_| !mid_para && k < n && !is_para_end(chars[k])) {
                let sc = &shaped[k];
                let cap = sc.face.cap_height / sc.face.units_per_em;
                let target = (dc.lines - 1) as f64 * base_line_h + sc.fmt.size * cap;
                let kk = (target / cap / sc.fmt.size).max(1.0);
                let width = sc.advance * kk;
                adv[k] = 0.0;
                drop = Some(Drop { ci: k, k: kk, width });
            }
        }

        // --- where the line goes
        let (col_x, col_w) = columns[col.min(columns.len() - 1)];
        let est_y = if first_in_col { if paragraph { base_asc } else { 0.0 } } else { y + base_line_h + pending_space };
        let (mut span_x, mut span_w, mut push_down) = (col_x, col_w, 0.0);
        if let Some(polys) = &poly {
            let mut yy = est_y;
            let mut found = None;
            while yy <= frame_h + base_line_h {
                if let Some(s) = shape_span(polys, yy - base_asc, yy + base_desc) {
                    found = Some(s);
                    break;
                }
                yy += base_line_h;
            }
            match found {
                Some((a, b)) => {
                    span_x = a;
                    span_w = b - a;
                    push_down = yy - est_y;
                }
                None => {
                    span_x = 0.0;
                    span_w = frame_w;
                    push_down = frame_h + base_line_h - est_y;
                }
            }
        }
        let dc_indent = match (&drop, drop_cap) {
            (Some(d), Some(dc)) if para_line < dc.lines as usize || dc.hanging => d.width + dc.space,
            _ => 0.0,
        };
        let left = match &marker {
            Some(m) if para_line == 0 || t.list.as_ref().is_some_and(|l| l.align_lines) => m.text_x,
            Some(m) => m.x,
            None => t.left_indent + if para_line == 0 && !mid_para { t.first_indent } else { 0.0 },
        } + dc_indent;
        let avail = if paragraph { (span_w - left - t.right_indent).max(1.0) } else { f64::INFINITY };

        let br = break_line(&chars, &shaped, &mut adv, &mut leader_at, t, k, avail, left, paragraph);
        let (asc, desc, size) = line_metrics(&shaped, k, br.end, &base_face, &t.format);
        let line_h = (asc + desc) * t.line_spacing / 100.0;
        let mut by = if first_in_col { if paragraph { asc } else { 0.0 } } else { y + line_h + pending_space } + push_down;
        if paragraph && t.align_grid {
            if let Some((sp, off)) = t.grid.filter(|g| g.0 > 0.1) {
                by = off + ((by - off) / sp - 1e-6).ceil() * sp;
            }
        }
        if paragraph && !hidden && by + desc > frame_h + 0.5 {
            if col + 1 < columns.len() && poly.is_none() && !first_in_col {
                col += 1;
                first_in_col = true;
                pending_space = 0.0;
                continue;
            }
            hidden = true;
        }
        if hidden {
            out.overflow = true;
            out.fit_end = out.fit_end.min(k);
        }

        // --- horizontal placement
        let e = br.end;
        let mut e_trim = e;
        while e_trim > k && matches!(chars[e_trim - 1], ' ' | EM_SPACE | EN_SPACE | QUARTER_EM_SPACE) {
            e_trim -= 1;
        }
        let hyphen = if br.kind == BreakKind::Hyphen && e > 0 {
            let f = &shaped[e - 1].fmt;
            shape_str(db, &f.font, f.bold, f.italic, f.size, "-").map(|s| (s, f.color))
        } else {
            None
        };
        let hyph_w = hyphen.as_ref().map(|h| h.0.width).unwrap_or(0.0);
        let content_w: f64 = adv[k..e_trim].iter().sum::<f64>() + hyph_w;
        let (mut x, mut extra_per_space) = (0.0, 0.0);
        if paragraph {
            match t.align {
                TextAlign::Left => {}
                TextAlign::Center => x = (avail - content_w) / 2.0,
                TextAlign::Right => x = avail - content_w,
                TextAlign::Justify | TextAlign::ForceJustify => {
                    let spaces = chars[k..e_trim].iter().filter(|c| **c == ' ').count();
                    let ends_para = matches!(br.kind, BreakKind::ParaEnd | BreakKind::ColBreak);
                    if spaces > 0 && (!ends_para || t.align == TextAlign::ForceJustify) && !chars[k..e_trim].contains(&'\t') {
                        extra_per_space = ((avail - content_w) / spaces as f64).max(0.0);
                    }
                }
            }
            x += span_x + left;
        } else {
            x = match t.align {
                TextAlign::Center => -content_w / 2.0,
                TextAlign::Right => -content_w,
                _ => 0.0,
            } + left;
        }
        let line_x0 = x;
        // bullet or number
        if para_line == 0 && !hidden {
            if let Some(m) = &marker {
                let mx = if paragraph { span_x + m.x } else { m.x };
                db.emit(&mut out.glyphs, &m.shaped, mx, by - m.shift, 1.0, k.min(n.saturating_sub(1)), m.color, li);
            }
        }
        for ci in k..e {
            carets[ci] = (x, by, li);
            have[ci] = true;
            let sc = &shaped[ci];
            if let Some(d) = drop.as_ref().filter(|d| d.ci == ci && para_line == 0) {
                // the drop cap, in the space before the first lines
                let dx = span_x + left - dc_indent;
                carets[ci] = (dx, by, li);
                if !hidden {
                    let base_y = by + (drop_cap.map(|d| d.lines).unwrap_or(1) - 1) as f64 * line_h;
                    let ss = ShapedStr { glyphs: sc.glyphs.clone(), width: d.width, face_id: sc.face_id, face: sc.face.clone(), size: sc.fmt.size };
                    db.emit(&mut out.glyphs, &ss, dx, base_y, d.k, ci, sc.fmt.color, li);
                }
                continue;
            }
            let a = adv[ci] + if sc.ch == ' ' { extra_per_space } else { 0.0 };
            if !hidden {
                let mut gx = x;
                let shift = sc.fmt.baseline_shift;
                for &(gid, gadv, xo, yo) in &sc.glyphs {
                    let gpath = db.glyph(sc.face_id, &sc.face, gid);
                    let sc_scale = sc.fmt.size / sc.face.units_per_em;
                    let mut tr = Affine::translate((gx + xo, by - yo - shift)) * Affine::scale(sc_scale);
                    if sc.fmt.italic && !is_italic_face(db, sc.face_id) {
                        tr = Affine::translate((gx + xo, by - yo - shift)) * Affine::new([1.0, 0.0, -0.2, 1.0, 0.0, 0.0]) * Affine::scale(sc_scale);
                    }
                    if !gpath.elements().is_empty() {
                        out.glyphs.push(LaidGlyph { char_index: ci, origin: Point::new(gx, by), advance: gadv, size: sc.fmt.size, path: gpath, transform: tr, color: sc.fmt.color, line: li });
                    }
                    gx += gadv;
                }
                // tab leader
                if sc.ch == '\t' && leader_at[ci] && a > 0.0 {
                    if let Some(ls) = shape_str(db, &sc.fmt.font, sc.fmt.bold, sc.fmt.italic, sc.fmt.size, &t.leader.ch.to_string()) {
                        let step = ls.width.max(0.5) * (1.0 + t.leader.spacing as f64);
                        let mut lx = ((x + ls.width * 0.5) / step).ceil() * step;
                        while lx + ls.width <= x + a - ls.width * 0.5 {
                            db.emit(&mut out.glyphs, &ls, lx, by, 1.0, ci, sc.fmt.color, li);
                            lx += step;
                        }
                    }
                }
                // decorations
                if (sc.fmt.underline || sc.fmt.strike) && !matches!(sc.ch, SOFT_HYPHEN) {
                    let scale = sc.fmt.size / sc.face.units_per_em;
                    let th = (sc.face.underline_thick * scale).max(0.5);
                    if sc.fmt.underline {
                        let uy = by - sc.face.underline_pos * scale;
                        out.decorations.push((Rect::new(x, uy, x + a, uy + th), sc.fmt.color));
                    }
                    if sc.fmt.strike {
                        let sy = by - sc.fmt.size * 0.28;
                        out.decorations.push((Rect::new(x, sy, x + a, sy + th), sc.fmt.color));
                    }
                }
            }
            x += a;
        }
        if let Some((h, color)) = &hyphen {
            if !hidden {
                db.emit(&mut out.glyphs, h, x, by, 1.0, e.saturating_sub(1), *color, li);
            }
            x += h.width;
        }
        // caret at line end
        let end_ci = e.min(n);
        if !have[end_ci] || matches!(br.kind, BreakKind::ParaEnd | BreakKind::ColBreak) {
            carets[end_ci] = (x, by, li);
            have[end_ci] = true;
        }
        out.lines.push(LaidLine { baseline: by, ascent: asc, descent: desc, x0: line_x0, x1: x, first_char: k.min(n), end_char: end_ci, hidden, column: col });

        first_in_col = false;
        y = by;
        pending_space = 0.0;
        li += 1;
        match br.kind {
            BreakKind::ParaEnd | BreakKind::ColBreak => {
                pending_space = size * t.para_spacing / 100.0;
                para_line = 0;
                para_no += 1;
                mid_para = false;
            }
            BreakKind::Wrap | BreakKind::Hyphen => para_line += 1,
        }
        if br.kind == BreakKind::ColBreak && paragraph && !hidden {
            if col + 1 < columns.len() && poly.is_none() {
                col += 1;
                first_in_col = true;
                pending_space = 0.0;
            } else {
                hidden = true;
                if br.next <= n {
                    out.overflow = true;
                    out.fit_end = out.fit_end.min(br.next);
                }
            }
        }
        k = br.next;
    }
    // fill missing carets (e.g. newline chars)
    for i in 1..carets.len() {
        if !have[i] {
            carets[i] = carets[i - 1];
        }
    }
    out.carets = carets;

    // text on path: remap glyphs along the path
    if let Some(top) = &t.on_path {
        let bp = top.path.to_bez();
        let total = crate::path::bez_length(&bp);
        if total > 0.0 {
            let mut remapped = vec![];
            for g in out.glyphs.drain(..) {
                let mid = g.origin.x + g.advance / 2.0 + top.offset;
                let s = if top.flip { total - mid } else { mid };
                if s < 0.0 || s > total {
                    continue;
                }
                if let Some((p, d)) = crate::path::point_at_length(&bp, s) {
                    let mut ang = d.y.atan2(d.x);
                    if top.flip {
                        ang += std::f64::consts::PI;
                    }
                    let local = Affine::translate((-g.origin.x - g.advance / 2.0, -g.origin.y));
                    let normal = kurbo::Vec2::new(-ang.sin(), ang.cos());
                    let place = Affine::translate(p.to_vec2() - normal * top.distance) * Affine::rotate(ang);
                    let mut g2 = g.clone();
                    g2.transform = place * local * g.transform;
                    remapped.push(g2);
                }
            }
            out.glyphs = remapped;
            out.decorations.clear();
        }
    }

    // bounds
    let mut b: Option<Rect> = None;
    for g in &out.glyphs {
        let r = (g.transform * (*g.path).clone()).bounding_box();
        b = Some(b.map_or(r, |x| x.union(r)));
    }
    for (r, _) in &out.decorations {
        b = Some(b.map_or(*r, |x| x.union(*r)));
    }
    out.bounds = match t.kind {
        TextKind::Paragraph { .. } => Rect::new(0.0, 0.0, frame_w, frame_h),
        TextKind::Artistic => {
            let line_box = out.lines.iter().fold(None::<Rect>, |acc, l| {
                let r = Rect::new(l.x0, l.baseline - l.ascent, l.x1.max(l.x0 + 1.0), l.baseline + l.descent);
                Some(acc.map_or(r, |a| a.union(r)))
            });
            match (b, line_box) {
                (Some(a), Some(l)) => {
                    if t.on_path.is_some() {
                        a
                    } else {
                        a.union(l)
                    }
                }
                (Some(a), None) => a,
                (None, Some(l)) => l,
                (None, None) => Rect::new(0.0, -t.format.size * 0.8, 1.0, t.format.size * 0.2),
            }
        }
    };
    out
}

fn is_italic_face(db: &FontDb, id: fontdb::ID) -> bool {
    db.db.face(id).map(|f| f.style != fontdb::Style::Normal).unwrap_or(false)
}

/// Glyph coverage for the Glyphs docker: list of (char, gid) available in a font.
pub fn font_chars(family: &str, bold: bool, italic: bool, max: usize) -> Vec<char> {
    let mut db = fonts().lock();
    let Some(id) = db.query(family, bold, italic) else { return vec![] };
    let Some(face) = db.face(id) else { return vec![] };
    let Ok(f) = ttf_parser::Face::parse(&face.data, face.index) else { return vec![] };
    let mut out = vec![];
    if let Some(cmap) = f.tables().cmap {
        for sub in cmap.subtables {
            if !sub.is_unicode() {
                continue;
            }
            let mut cps = vec![];
            sub.codepoints(|cp| cps.push(cp));
            cps.sort();
            for cp in cps {
                if let Some(c) = char::from_u32(cp) {
                    if !c.is_control() && !out.contains(&c) {
                        out.push(c);
                        if out.len() >= max {
                            return out;
                        }
                    }
                }
            }
        }
    }
    out
}
