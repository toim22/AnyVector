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
        let mut best = 0;
        let mut bd = f64::INFINITY;
        // choose line by baseline proximity first
        let mut line = 0;
        let mut ld = f64::INFINITY;
        for (i, l) in self.lines.iter().enumerate() {
            let mid = l.baseline - (l.ascent - l.descent) / 2.0 + l.descent;
            let top = l.baseline - l.ascent;
            let bot = l.baseline + l.descent;
            let d = if p.y >= top && p.y <= bot { 0.0 } else { (p.y - mid).abs() };
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
    ci: usize,
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

pub fn layout_with(db: &mut FontDb, t: &TextObj) -> TextLayout {
    let chars: Vec<char> = t.content.chars().collect();
    let mut out = TextLayout::default();
    // --- shape per run of identical format (within paragraphs) ---
    let mut shaped: Vec<ShapedChar> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let fmt = t.format_at(i).clone();
        let mut j = i + 1;
        while j < chars.len() && t.format_at(j) == &fmt && chars[j] != '\n' && chars[i] != '\n' {
            j += 1;
        }
        let Some(fid) = db.query(&fmt.font, fmt.bold, fmt.italic) else {
            i = j;
            continue;
        };
        let Some(face) = db.face(fid) else {
            i = j;
            continue;
        };
        let scale = fmt.size / face.units_per_em;
        let run: String = chars[i..j]
            .iter()
            .map(|c| if fmt.caps { c.to_uppercase().next().unwrap_or(*c) } else { *c })
            .collect();
        let mut per_char: Vec<Vec<(u16, f64, f64, f64)>> = vec![vec![]; j - i];
        if let Some(rb) = rustybuzz::Face::from_slice(&face.data, face.index) {
            let mut buf = rustybuzz::UnicodeBuffer::new();
            buf.push_str(&run);
            let gb = rustybuzz::shape(&rb, &[], buf);
            // cluster = byte offset in run
            let byte_to_char: Vec<usize> = {
                let mut v = vec![0; run.len() + 1];
                for (ci, (b, _)) in run.char_indices().enumerate() {
                    v[b] = ci;
                }
                v
            };
            for (info, pos) in gb.glyph_infos().iter().zip(gb.glyph_positions()) {
                let ci = byte_to_char.get(info.cluster as usize).copied().unwrap_or(0).min(j - i - 1);
                per_char[ci].push((
                    info.glyph_id as u16,
                    pos.x_advance as f64 * scale,
                    pos.x_offset as f64 * scale,
                    pos.y_offset as f64 * scale,
                ));
            }
        }
        for (k, ci) in (i..j).enumerate() {
            let space_w = fmt.size * 0.25;
            let mut adv: f64 = per_char[k].iter().map(|g| g.1).sum();
            if chars[ci] == ' ' {
                adv *= t.word_spacing / 100.0;
            }
            adv += fmt.tracking / 100.0 * space_w;
            // a tab is blank space (the font has no glyph for it), half an inch wide
            let glyphs = if chars[ci] == '\t' {
                adv = 36.0;
                vec![]
            } else {
                per_char[k].clone()
            };
            shaped.push(ShapedChar { ci, ch: chars[ci], glyphs, advance: adv, face_id: fid, face: face.clone(), fmt: fmt.clone() });
        }
        i = j;
    }

    // --- line breaking ---
    let frame_w = match t.kind {
        TextKind::Paragraph { width, .. } => {
            let cols = t.columns.max(1) as f64;
            Some(((width - t.column_gap * (cols - 1.0)) / cols - t.left_indent - t.right_indent).max(1.0))
        }
        TextKind::Artistic => None,
    };
    let mut lines: Vec<(usize, usize, bool)> = vec![]; // [start,end) in shaped, is_para_end
    {
        let mut start = 0;
        let mut k = 0;
        let mut x = 0.0;
        let mut last_space: Option<usize> = None;
        let mut first_in_para = true;
        while k < shaped.len() {
            let sc = &shaped[k];
            if sc.ch == '\n' {
                lines.push((start, k, true));
                k += 1;
                start = k;
                x = 0.0;
                last_space = None;
                first_in_para = true;
                continue;
            }
            let indent = if first_in_para { t.first_indent } else { 0.0 };
            if let Some(w) = frame_w {
                if x + sc.advance > w - indent && k > start && sc.ch != ' ' {
                    let brk = match last_space {
                        Some(s) if s > start => s + 1,
                        _ => k,
                    };
                    lines.push((start, brk, false));
                    start = brk;
                    k = brk;
                    x = 0.0;
                    last_space = None;
                    first_in_para = false;
                    continue;
                }
            }
            if sc.ch == ' ' {
                last_space = Some(k);
            }
            x += sc.advance;
            k += 1;
        }
        lines.push((start, shaped.len(), true));
    }

    // --- vertical metrics & placement ---
    let base_fmt = &t.format;
    let mut y = 0.0;
    let paragraph = frame_w.is_some();
    let (frame_width, frame_height) = match t.kind {
        TextKind::Paragraph { width, height } => (width, height),
        _ => (0.0, 0.0),
    };
    let cols = t.columns.max(1) as usize;
    let col_w = frame_w.unwrap_or(0.0) + t.left_indent + t.right_indent;
    let mut col = 0usize;
    let mut carets: Vec<(f64, f64, usize)> = vec![(0.0, 0.0, 0); chars.len() + 1];
    let mut have_caret = vec![false; chars.len() + 1];
    for (li, &(s, e, para_end)) in lines.iter().enumerate() {
        let (mut asc, mut desc, mut size) = (0.0f64, 0.0f64, 0.0f64);
        for sc in &shaped[s..e] {
            let sc_scale = sc.fmt.size / sc.face.units_per_em;
            asc = asc.max(sc.face.ascender * sc_scale);
            desc = desc.max(-sc.face.descender * sc_scale);
            size = size.max(sc.fmt.size);
        }
        if s == e {
            // empty line: use format of the preceding char or default
            let f = if s > 0 && s - 1 < shaped.len() { shaped[s - 1].fmt.clone() } else { base_fmt.clone() };
            size = f.size;
            asc = f.size * 0.9;
            desc = f.size * 0.25;
        }
        let line_h = (asc + desc) * t.line_spacing / 100.0;
        if li == 0 {
            y = if paragraph { asc } else { 0.0 };
        } else {
            y += line_h;
        }
        if paragraph && cols > 1 && y + desc > frame_height && col + 1 < cols {
            col += 1;
            y = asc;
        }
        if paragraph && y + desc > frame_height + 0.5 {
            out.overflow = true;
        }
        let col_x = col as f64 * (col_w + t.column_gap);
        // width of content (trim trailing spaces)
        let mut e_trim = e;
        while e_trim > s && shaped[e_trim - 1].ch == ' ' {
            e_trim -= 1;
        }
        let content_w: f64 = shaped[s..e_trim].iter().map(|c| c.advance).sum();
        let indent = if li == 0 || lines[li - 1].2 { t.first_indent } else { 0.0 } + t.left_indent;
        let avail = frame_w.unwrap_or(content_w);
        let (mut x, mut extra_per_space) = (0.0, 0.0);
        match t.align {
            TextAlign::Left => {}
            TextAlign::Center => x = (avail - content_w) / 2.0,
            TextAlign::Right => x = avail - content_w,
            TextAlign::Justify | TextAlign::ForceJustify => {
                let spaces = shaped[s..e_trim].iter().filter(|c| c.ch == ' ').count();
                let do_it = paragraph && spaces > 0 && (!para_end || t.align == TextAlign::ForceJustify);
                if do_it {
                    extra_per_space = (avail - content_w) / spaces as f64;
                }
            }
        }
        if !paragraph {
            // artistic: alignment relative to origin
            match t.align {
                TextAlign::Center => x = -content_w / 2.0,
                TextAlign::Right => x = -content_w,
                _ => x = 0.0,
            }
        }
        x += indent + col_x;
        let line_x0 = x;
        for sc in &shaped[s..e] {
            carets[sc.ci] = (x, y, li);
            have_caret[sc.ci] = true;
            let mut gx = x;
            let shift = sc.fmt.baseline_shift;
            for &(gid, adv, xo, yo) in &sc.glyphs {
                let gpath = db.glyph(sc.face_id, &sc.face, gid);
                let sc_scale = sc.fmt.size / sc.face.units_per_em;
                let mut tr = Affine::translate((gx + xo, y - yo - shift)) * Affine::scale(sc_scale);
                if sc.fmt.italic && !is_italic_face(db, sc.face_id) {
                    tr = Affine::translate((gx + xo, y - yo - shift)) * Affine::new([1.0, 0.0, -0.2, 1.0, 0.0, 0.0]) * Affine::scale(sc_scale);
                }
                if !gpath.elements().is_empty() {
                    out.glyphs.push(LaidGlyph {
                        char_index: sc.ci,
                        origin: Point::new(gx, y),
                        advance: adv,
                        size: sc.fmt.size,
                        path: gpath,
                        transform: tr,
                        color: sc.fmt.color,
                        line: li,
                    });
                }
                gx += adv;
            }
            let adv = sc.advance + if sc.ch == ' ' { extra_per_space } else { 0.0 };
            // decorations
            if sc.fmt.underline || sc.fmt.strike {
                let scale = sc.fmt.size / sc.face.units_per_em;
                if sc.fmt.underline {
                    let uy = y - sc.face.underline_pos * scale;
                    let th = (sc.face.underline_thick * scale).max(0.5);
                    out.decorations.push((Rect::new(x, uy, x + adv, uy + th), sc.fmt.color));
                }
                if sc.fmt.strike {
                    let sy = y - sc.fmt.size * 0.28;
                    let th = (sc.face.underline_thick * scale).max(0.5);
                    out.decorations.push((Rect::new(x, sy, x + adv, sy + th), sc.fmt.color));
                }
            }
            x += adv;
        }
        // caret at line end
        let end_ci = if e < shaped.len() { shaped[e].ci } else { chars.len() };
        if !have_caret[end_ci] || para_end {
            carets[end_ci] = (x, y, li);
            have_caret[end_ci] = true;
        }
        out.lines.push(LaidLine {
            baseline: y,
            ascent: asc,
            descent: desc,
            x0: line_x0,
            x1: x,
            first_char: if s < shaped.len() { shaped[s].ci } else { chars.len() },
            end_char: end_ci,
        });
        if para_end {
            y += size * t.para_spacing / 100.0;
        }
        let _ = size;
    }
    // fill missing carets (e.g. newline chars)
    for k in 0..carets.len() {
        if !have_caret[k] && k > 0 {
            carets[k] = carets[k - 1];
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
        TextKind::Paragraph { .. } => Rect::new(0.0, 0.0, frame_width, frame_height),
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
