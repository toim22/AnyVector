//! Text of a CorelDRAW drawing: the font table, the style table and the text streams.

use super::build::{Ctx, Loda, ARG_STYLE, ARG_STYLE_JSON};
use super::color;
use super::riff::{Chunk, Rd};
use kurbo::{Affine, Point};
use std::collections::HashMap;
use vector_core::color::Color;
use vector_core::object::{ObjKind, Object};
use vector_core::style::{Fill, Outline, Style};
use vector_core::text::{CharFormat, ParaStyle, Span, TextAlign, TextObj};

/// Windows-1252, the encoding of one-byte text in drawings saved on Western systems.
pub(super) fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}', '\u{8f}',
        '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}', '\u{178}',
    ];
    match b {
        0x80..=0x9f => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// One-byte characters are kept as U+F000 + byte until the run's character set is known.
const RAW: u32 = 0xF000;

/// The text encoding of a Windows character set (the `lfCharSet` of a font).
fn encoding(charset: u16) -> Option<&'static encoding_rs::Encoding> {
    use encoding_rs::*;
    Some(match charset {
        0xee => WINDOWS_1250,
        0xcc => WINDOWS_1251,
        0xa1 => WINDOWS_1253,
        0xa2 => WINDOWS_1254,
        0xb1 => WINDOWS_1255,
        0xb2 => WINDOWS_1256,
        0xba => WINDOWS_1257,
        0xa3 => WINDOWS_1258,
        0xde => WINDOWS_874,
        0x80 => SHIFT_JIS,
        0x81 => EUC_KR,
        0x86 => GBK,
        0x88 => BIG5,
        _ => return None,
    })
}

/// Turn the one-byte characters of `text` into Unicode with the character set.
fn decode(text: &str, charset: Option<u16>) -> String {
    let enc = charset.and_then(encoding);
    let mut out = String::with_capacity(text.len());
    let mut bytes: Vec<u8> = vec![];
    let flush = |bytes: &mut Vec<u8>, out: &mut String| {
        if bytes.is_empty() {
            return;
        }
        match enc {
            Some(e) => out.push_str(&e.decode_without_bom_handling(bytes).0),
            None => out.extend(bytes.iter().map(|b| cp1252(*b))),
        }
        bytes.clear();
    };
    for c in text.chars() {
        let u = c as u32;
        if (RAW..RAW + 0x100).contains(&u) {
            bytes.push((u - RAW) as u8);
        } else {
            flush(&mut bytes, &mut out);
            out.push(c);
        }
    }
    flush(&mut bytes, &mut out);
    out
}

/// Font names that end in a script name ("Arial Cyr") pick that character set.
fn charset_of_name(name: &str) -> Option<(String, u16)> {
    const SUFFIXES: [(&str, u16); 11] = [
        (" CE", 0xee),
        (" Cyrillic", 0xcc),
        (" Cyr", 0xcc),
        (" CYR", 0xcc),
        (" Baltic", 0xba),
        (" Greek", 0xa1),
        (" Tur", 0xa2),
        (" TUR", 0xa2),
        (" Hebrew", 0xb1),
        (" Arabic", 0xb2),
        (" Thai", 0xde),
    ];
    SUFFIXES.iter().find(|(s, _)| name.ends_with(s)).map(|(s, c)| (name[..name.len() - s.len()].to_string(), *c))
}

/// What the style table says about text styles.
#[derive(Default)]
pub(super) struct Styles {
    /// Font records: record id to (font id, size in points, weight and slant flags, charset).
    fonts: HashMap<u32, (u32, f64, u32, u16)>,
    /// Alignment records.
    aligns: HashMap<u32, u32>,
    /// Spacing records.
    spacings: HashMap<u32, Spacing>,
    /// Fill records of the table: id to the `fild` id.
    fills: HashMap<u32, u32>,
    /// The styles, by id.
    styles: HashMap<u32, StyleRec>,
    first_font: Option<u32>,
}

/// One text style: its parent and the records it uses (0: inherited).
#[derive(Clone, Copy, Debug, Default)]
struct StyleRec {
    parent: u32,
    fill: u32,
    font: u32,
    align: u32,
    spacing: u32,
}

/// Spacing between characters, words, lines and paragraphs.
#[derive(Clone, Copy, Debug, Default)]
struct Spacing {
    /// Percent of the character height, when the record gives shares rather than lengths.
    line: Option<f64>,
    para: Option<f64>,
    /// Percent of a space's width.
    chars: f64,
    words: f64,
}

#[derive(Clone, Debug, Default)]
struct Look {
    font: Option<String>,
    size: Option<f64>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strike: Option<bool>,
    color: Option<Color>,
    align: Option<TextAlign>,
    /// Line spacing and extra paragraph spacing, percent of the character height.
    line: Option<f64>,
    para: Option<f64>,
    /// Windows character set of one-byte characters.
    charset: Option<u16>,
    /// Extra space between characters, percent of a space's width; word spacing, percent.
    tracking: Option<f64>,
    /// Shift up from the baseline, share of the character height.
    shift: Option<f64>,
    words: Option<f64>,
    /// Indents of the first line, the other lines and the right side, points from the frame.
    indents: [Option<f64>; 3],
    /// A paragraph frame's columns: (width, gutter after), in shares of the frame's width.
    columns: Option<Vec<(f64, f64)>>,
}

impl Look {
    fn over(&self, base: &Look) -> Look {
        Look {
            font: self.font.clone().or_else(|| base.font.clone()),
            size: self.size.or(base.size),
            bold: self.bold.or(base.bold),
            italic: self.italic.or(base.italic),
            underline: self.underline.or(base.underline),
            strike: self.strike.or(base.strike),
            color: self.color.or(base.color),
            align: self.align.or(base.align),
            line: self.line.or(base.line),
            para: self.para.or(base.para),
            charset: self.charset.or(base.charset),
            tracking: self.tracking.or(base.tracking),
            shift: self.shift.or(base.shift),
            words: self.words.or(base.words),
            indents: std::array::from_fn(|i| self.indents[i].or(base.indents[i])),
            columns: self.columns.clone().or_else(|| base.columns.clone()),
        }
    }
    fn format(&self) -> CharFormat {
        let mut f = CharFormat { size: 12.0, ..Default::default() };
        if let Some(n) = &self.font {
            f.font = n.clone();
        }
        if let Some(s) = self.size {
            f.size = s;
        }
        f.bold = self.bold.unwrap_or(false);
        f.italic = self.italic.unwrap_or(false);
        f.underline = self.underline.unwrap_or(false);
        f.strike = self.strike.unwrap_or(false);
        f.tracking = self.tracking.unwrap_or(0.0);
        f.baseline_shift = self.shift.unwrap_or(0.0) * f.size;
        f
    }
}

/// A run of characters with one look.
struct Run {
    text: String,
    look: Look,
}

struct Para {
    runs: Vec<Run>,
    align: Option<TextAlign>,
    indents: [Option<f64>; 3],
}

// ---------------------------------------------------------------- tables

/// A `font` record: id, encoding, then the family name.
pub(super) fn font(cx: &mut Ctx, b: &[u8]) {
    let mut r = Rd::new(b);
    let id = r.u16() as u32;
    // CorelDRAW 4 has the name right after the id
    let mut charset = 0;
    if cx.ver >= 500 {
        charset = r.u16();
        r.skip(14);
    }
    // version 5 gave the East Asian sets for fonts whose text is not stored that way
    if cx.ver < 600 && matches!(charset, 0x80 | 0x81 | 0x86 | 0x88) {
        charset = 0;
    }
    let mut name = cx.string(r.bytes(r.left()));
    let mut named = false;
    if let Some((base, c)) = charset_of_name(&name) {
        name = base;
        charset = c;
        named = true;
    }
    if name.is_empty() {
        return;
    }
    // A font is listed once per script it is used with. The first entry names the font; its
    // character set holds only when the entries agree or the name gives it.
    match cx.fonts.get(&id) {
        None => {
            cx.fonts.insert(id, name);
            cx.charsets.insert(id, charset);
            if named {
                cx.named_charsets.insert(id);
            }
        }
        Some(_) if !cx.named_charsets.contains(&id) && cx.charsets.get(&id) != Some(&charset) => {
            cx.charsets.insert(id, 0);
        }
        _ => {}
    }
}

/// The style table (versions 7 to X5): the records styles are made of, then the styles.
pub(super) fn stlt(cx: &mut Ctx, b: &[u8]) {
    if cx.ver < 700 {
        stlt5(cx, b);
        return;
    }
    if cx.ver >= 1600 {
        return;
    }
    // version 8 files come in two layouts; the later one has one more table before the styles
    let read = |extra: bool| stlt_tables(cx, b, extra);
    let styles = match read(cx.ver > 800) {
        Some(t) => t,
        None if cx.ver == 800 => match read(true) {
            Some(t) => t,
            None => return,
        },
        None => return,
    };
    cx.styles = styles;
}

/// Versions 5 and 6: a list of styles, each an argument table like an object's (0xcd: the
/// fill id, 0xd2: the outline id).
fn stlt5(cx: &mut Ctx, b: &[u8]) {
    let mut chunks = vec![];
    super::riff::parse(b, 0, b.len(), None, None, 0, &mut chunks);
    for styl in chunks.iter().filter(|c| c.is_list(b"styl")) {
        let Some(d) = styl.child(b"styd") else { continue };
        let mut r = Rd::new(&d.data);
        let id = cx.uint(&mut r);
        let Some(rest) = d.data.get(r.p..) else { continue };
        let l = cx.loda(rest);
        let arg = |t: u32| l.arg(t).map(|a| Rd::new(a).u32()).unwrap_or(0);
        let ids = (arg(0xcd), arg(0xd2));
        cx.graphic_styles.insert(id, ids);
    }
}

fn stlt_tables(cx: &Ctx, b: &[u8], set11: bool) -> Option<Styles> {
    let mut out = Styles::default();
    let mut r = Rd::new(b);
    let records = r.u32() as usize;
    let count = |r: &mut Rd, size: usize| (r.u32() as usize).min(r.left() / size.max(1));
    for _ in 0..count(&mut r, 12) {
        let id = r.u32();
        r.skip(4);
        out.fills.insert(id, r.u32());
        if cx.ver >= 1300 {
            r.skip(48);
        }
    }
    let outlines = count(&mut r, 12);
    r.skip(outlines * 12);
    let coord = if cx.wide() { 4 } else { 2 };
    for _ in 0..count(&mut r, 4 + 4 + 8 + coord + if cx.ver < 1000 { 24 } else { 40 }) {
        let id = r.u32();
        r.skip(if cx.ver < 1000 { 12 } else { 20 });
        let font = r.u16() as u32;
        let charset = r.u16();
        r.skip(8);
        let size = cx.coord(&mut r) * cx.k;
        if cx.ver >= 1000 {
            r.skip(8); // the sizes for right-to-left and East Asian text
        }
        let flags = r.u32();
        r.skip(8);
        if size > 0.1 && size < 5000.0 {
            out.fonts.insert(id, (font, size, flags, charset));
            out.first_font.get_or_insert(id);
        }
    }
    for _ in 0..count(&mut r, 12) {
        let id = r.u32();
        r.skip(4);
        out.aligns.insert(id, r.u32());
    }
    for _ in 0..count(&mut r, 52) {
        let id = r.u32();
        r.skip(8);
        let chars = r.i32() as f64;
        r.skip(4);
        let words = r.i32() as f64;
        let line = r.i32() as f64;
        let before = r.i32() as f64;
        let after = r.i32() as f64;
        r.skip(12);
        // shares of the character height (1 000 000: 100%), or lengths when the unit is set
        let shares = r.u32() <= 1 && line > 0.0 && before >= 0.0 && after >= 0.0;
        out.spacings.insert(
            id,
            Spacing {
                line: shares.then_some(line / 10_000.0),
                // space before is a share of the line (100%: none extra), space after is added
                para: shares.then_some((before + after) / 10_000.0 - 100.0),
                chars: chars / 10_000.0,
                words: if words >= 0.0 { words / 10_000.0 } else { 100.0 },
            },
        );
    }
    let set5 = count(&mut r, 152);
    r.skip(set5 * 152);
    let tabs = count(&mut r, 784);
    r.skip(tabs * 784);
    let bullets = count(&mut r, 16);
    for _ in 0..bullets {
        r.skip(40);
        if cx.ver > 1300 {
            r.skip(4);
        }
        if cx.ver >= 1300 {
            let more = r.u32() != 0;
            r.skip(if more { 68 } else { 12 });
        } else {
            r.skip(if cx.ver >= 1000 { 28 } else { 20 });
            if r.u32() != 0 {
                r.skip(8);
            }
            r.skip(8);
        }
    }
    let indents = count(&mut r, 16 + 3 * coord);
    r.skip(indents * (16 + 3 * coord));
    let hyphens = count(&mut r, 36);
    r.skip(hyphens * if cx.ver >= 1300 { 36 } else { 32 });
    let dropcaps = count(&mut r, 28);
    r.skip(dropcaps * 28);
    if set11 {
        let n = count(&mut r, 12);
        r.skip(n * 12);
    }
    let name_unit = if cx.ver >= 1200 { 2 } else { 1 };
    for _ in 0..records.min(100_000) {
        if r.left() < 32 {
            break;
        }
        let num = r.u32();
        let id = r.u32();
        let parent = r.u32();
        r.skip(8);
        let name_len = r.u32() as usize;
        if num == 0 || num > 3 || name_len > 1024 {
            return None;
        }
        r.skip(name_len * name_unit);
        let mut st = StyleRec { parent, fill: r.u32(), ..Default::default() };
        let _outline = r.u32();
        if num > 1 {
            st.font = r.u32();
            st.align = r.u32();
            st.spacing = r.u32();
            r.skip(if set11 { 8 } else { 4 });
        }
        if num > 2 {
            r.skip(20);
        }
        if r.eof() && r.p > b.len() {
            return None;
        }
        out.styles.insert(id, st);
    }
    // a table read with the wrong layout points its styles at records it does not have
    let bad = out.styles.values().any(|s| (s.font != 0 && !out.fonts.contains_key(&s.font)) || (s.parent != 0 && !out.styles.contains_key(&s.parent)));
    if bad || r.p > b.len() {
        return None;
    }
    Some(out)
}

impl Styles {
    /// A style with what it inherits from its parents filled in.
    /// A style of the table, or any style when the drawing has no table.
    fn known(&self, style: u32) -> bool {
        self.styles.is_empty() || self.styles.contains_key(&style)
    }

    fn resolve(&self, style: u32) -> StyleRec {
        let mut out = StyleRec::default();
        let mut id = style;
        for _ in 0..32 {
            let Some(s) = self.styles.get(&id) else { break };
            let take = |a: &mut u32, b: u32| {
                if *a == 0 {
                    *a = b;
                }
            };
            take(&mut out.fill, s.fill);
            take(&mut out.font, s.font);
            take(&mut out.align, s.align);
            take(&mut out.spacing, s.spacing);
            if s.parent == 0 || s.parent == id {
                break;
            }
            id = s.parent;
        }
        out
    }

    fn look(&self, style: u32, cx: &Ctx) -> Look {
        let st = self.resolve(style);
        let rec = if st.font != 0 { Some(st.font) } else { self.first_font };
        let mut l = match rec.and_then(|id| self.fonts.get(&id)) {
            Some((font, size, flags, charset)) => {
                let (bold, italic) = weight(*flags);
                let (underline, strike) = decorations(*flags);
                let charset = Some(*charset).filter(|c| *c != 0).or_else(|| cx.charsets.get(font).copied());
                Look {
                    font: cx.fonts.get(font).cloned(),
                    size: Some(*size),
                    bold: Some(bold),
                    italic: Some(italic),
                    underline: Some(underline),
                    strike: Some(strike),
                    charset,
                    ..Default::default()
                }
            }
            None => Look::default(),
        };
        l.align = self.aligns.get(&st.align).and_then(|v| align(*v as f64));
        if let Some(sp) = self.spacings.get(&st.spacing) {
            l.line = sp.line.filter(|v| *v > 1.0 && *v < 2000.0);
            l.para = sp.para.filter(|v| *v > 0.01 && *v < 2000.0);
            l.tracking = Some(sp.chars).filter(|v| v.abs() > 0.01);
            l.words = Some(sp.words).filter(|v| (*v - 100.0).abs() > 0.01);
        }
        if let Some(super::build::RawFill::Solid(c)) = self.fills.get(&st.fill).and_then(|f| cx.fills.get(f)) {
            l.color = Some(*c);
        }
        l
    }
}

// ---------------------------------------------------------------- styles stored as JSON (X6 and later)

fn json_num(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::String(s) => s.trim().parse().ok(),
        serde_json::Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

/// A color written as "RGB255,USER,r,g,b,…" or "CMYK,USER,c,m,y,k,…".
fn json_color(s: &str) -> Option<Color> {
    let parts: Vec<&str> = s.split(',').collect();
    let n = |i: usize| parts.get(i).and_then(|p| p.trim().parse::<f64>().ok());
    match parts.first()?.trim() {
        "RGB255" => Some(Color::rgb(n(2)? as u8, n(3)? as u8, n(4)? as u8)),
        "RGB" => Some(Color::rgb((n(2)? * 2.55) as u8, (n(3)? * 2.55) as u8, (n(4)? * 2.55) as u8)),
        "CMYK" => Some(color::cmyk(n(2)? / 100.0, n(3)? / 100.0, n(4)? / 100.0, n(5)? / 100.0)),
        "CMYK255" => Some(color::cmyk(n(2)? / 255.0, n(3)? / 255.0, n(4)? / 255.0, n(5)? / 255.0)),
        "GRAY255" => Some(Color::rgb(n(2)? as u8, n(2)? as u8, n(2)? as u8)),
        "GRAY" => {
            let g = (n(2)? * 2.55) as u8;
            Some(Color::rgb(g, g, g))
        }
        _ => None,
    }
}

fn json_look(v: &serde_json::Value, k: f64) -> Look {
    let mut l = Look::default();
    // "width,gutter|width,gutter": only the proportions count, CorelDRAW fits them to the frame
    l.columns = v.get("frame").and_then(|f| f.get("columns")).and_then(|c| c.as_str()).and_then(|c| {
        let cols: Vec<(f64, f64)> = c
            .split('|')
            .map(|col| {
                let mut n = col.split(',').map(|x| x.trim().parse::<f64>().unwrap_or(0.0));
                (n.next().unwrap_or(0.0).max(0.0), n.next().unwrap_or(0.0).max(0.0))
            })
            .collect();
        let total: f64 = cols.iter().map(|(w, g)| w + g).sum::<f64>() - cols.last().map_or(0.0, |c| c.1);
        (cols.len() > 1 && total > 0.0).then(|| cols.iter().map(|(w, g)| (w / total, g / total)).collect())
    });
    let Some(c) = v.get("character") else { return l };
    if let Some(latin) = c.get("latin") {
        l.font = latin.get("font").and_then(|f| f.as_str()).filter(|s| !s.is_empty()).map(str::to_string);
        l.size = latin.get("size").and_then(json_num).map(|s| s * k).filter(|s| *s > 0.1);
        l.italic = latin.get("italic").and_then(json_num).map(|x| x != 0.0);
        // weight is on the OS/2 scale divided by 6.25: 64 regular, 112 bold
        l.bold = latin.get("weight").and_then(json_num).map(|x| x >= 96.0);
        l.underline = latin.get("underline").and_then(json_num).map(|x| x != 0.0);
        l.strike = latin.get("strikeout").and_then(json_num).map(|x| x != 0.0);
    }
    if let Some(fill) = c.get("fill") {
        l.color = fill.get("primaryColor").and_then(|p| p.as_str()).and_then(json_color);
    }
    if let Some(p) = v.get("paragraph") {
        l.align = p.get("justify").and_then(json_num).and_then(align);
        l.indents = ["firstLineIndent", "leftMargin", "rightMargin"].map(|key| p.get(key).and_then(json_num).map(|v| v * k));
        // spacing in millionths of the character height when the method is 0
        if p.get("spacingMethod").and_then(json_num).unwrap_or(0.0) == 0.0 {
            l.line = p.get("interLineSpacing").and_then(json_num).map(|v| v / 10_000.0).filter(|v| *v > 1.0 && *v < 2000.0);
            let before = p.get("beforeParaSpacing").and_then(json_num).map(|v| v / 10_000.0);
            let after = p.get("afterParaSpacing").and_then(json_num).map(|v| v / 10_000.0);
            if before.is_some() || after.is_some() {
                l.para = Some(((before.unwrap_or(100.0) - 100.0) + after.unwrap_or(0.0)).clamp(0.0, 2000.0));
            }
        }
    }
    l
}

/// Underline and strikethrough from the high half of a font's flags: three bits each, from bit 18
/// and from bit 24 (1 single, 3 single thick, 5 double); overline (from bit 21) has no equal here.
fn decorations(flags: u32) -> (bool, bool) {
    ((flags >> 18) & 7 != 0, (flags >> 24) & 7 != 0)
}

/// Weight and slant flags: one bit per face, upright on even bits and italic on odd ones;
/// 0x40 regular, 0x80 italic, 0x1000 bold, 0x2000 bold italic.
fn weight(flags: u32) -> (bool, bool) {
    let flags = flags & 0xffff;
    if flags == 0 {
        return (false, false);
    }
    let bit = flags.trailing_zeros();
    (bit >= 12, bit % 2 == 1)
}

fn align(v: f64) -> Option<TextAlign> {
    match v as i64 {
        1 => Some(TextAlign::Left),
        2 => Some(TextAlign::Center),
        3 => Some(TextAlign::Right),
        4 | 5 => Some(TextAlign::Justify),
        _ => None,
    }
}

/// The next length-prefixed JSON object at or after `from`: (start of the text, bytes).
fn find_json(b: &[u8], from: usize, wide: bool) -> Option<(usize, usize)> {
    let unit = if wide { 2 } else { 1 };
    let mut p = from;
    while p + 4 + unit <= b.len() {
        if b[p + 4] == b'{' && (!wide || b[p + 5] == 0) {
            let n = u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]]) as usize;
            let len = n.saturating_mul(unit);
            if n >= 2 && p + 4 + len <= b.len() {
                let last = p + 4 + len - unit;
                if b[last] == b'}' && (!wide || b[last + 1] == 0) {
                    return Some((p + 4, len));
                }
            }
        }
        p += 1;
    }
    None
}

fn decode_json(b: &[u8], wide: bool) -> Option<serde_json::Value> {
    let s = if wide {
        let u: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&u)
    } else {
        String::from_utf8_lossy(b).to_string()
    };
    serde_json::from_str(&s).ok()
}

/// The characters of one paragraph block: count, one descriptor per character, byte count, bytes.
/// Returns the text of each style index run and where the block ends.
fn characters(b: &[u8], at: usize, descr: usize, counted: bool) -> Option<(Vec<(usize, String)>, usize)> {
    let mut r = Rd::at(b, at);
    let n = r.u32() as usize;
    if n > b.len() {
        return None;
    }
    let d = r.bytes(n * descr);
    if d.len() != n * descr {
        return None;
    }
    let wide_chars = d.chunks_exact(descr).filter(|c| c[0] & 1 != 0).count();
    let bytes = if counted { r.u32() as usize } else { n + wide_chars };
    if bytes != n + wide_chars {
        return None;
    }
    let t = r.bytes(bytes);
    if t.len() != bytes {
        return None;
    }
    let mut runs: Vec<(usize, String)> = vec![];
    let mut tr = Rd::new(t);
    for c in d.chunks_exact(descr) {
        let ch = if c[0] & 1 != 0 { char::from_u32(tr.u16() as u32).unwrap_or('\u{fffd}') } else {
            // ASCII is the same in every character set; the rest waits for the run's set
            match tr.u8() {
                c @ 0..=0x7f => c as char,
                c => char::from_u32(RAW + c as u32).unwrap_or('?'),
            }
        };
        let ch = match ch {
            '\r' => '\n',
            '\0' => continue,
            c => c,
        };
        let style = if descr >= 4 { (u16::from_le_bytes([c[2], c[3]]) >> 1) as usize } else { 0 };
        match runs.last_mut() {
            Some((s, text)) if *s == style => text.push(ch),
            _ => runs.push((style, ch.to_string())),
        }
    }
    Some((runs, r.p))
}

fn paragraphs_json(cx: &Ctx, b: &[u8]) -> Vec<Para> {
    let wide = cx.ver < 1700;
    let mut out = vec![];
    let mut pos = 0;
    while let Some((s, len)) = find_json(b, pos, wide) {
        pos = s + len;
        let Some(v) = decode_json(&b[s..s + len], wide) else { continue };
        if v.get("character").is_none() {
            continue;
        }
        let base = json_look(&v, cx.k);
        let count = Rd::at(b, pos).u32() as usize;
        if count == 0 || count > 4096 {
            continue;
        }
        let mut p = pos + 4;
        let mut looks = vec![];
        for _ in 0..count {
            match find_json(b, p, wide) {
                Some((s2, l2)) if s2 - p <= 256 => {
                    let o = decode_json(&b[s2..s2 + l2], wide).map(|v| json_look(&v, cx.k)).unwrap_or_default();
                    looks.push(o.over(&base));
                    p = s2 + l2;
                }
                _ => break,
            }
        }
        if looks.len() != count {
            continue;
        }
        let Some((runs, end)) = characters(b, p, 8, true) else { continue };
        let runs = runs
            .into_iter()
            .map(|(i, text)| {
                let look = looks.get(i).cloned().unwrap_or_else(|| base.clone());
                Run { text: decode(&text, look.charset), look }
            })
            .collect();
        // a paragraph's style lists every setting but a left alignment, the default
        let left = v.get("paragraph").is_some_and(|p| p.get("justify").is_none());
        let align = base.align.or(left.then_some(TextAlign::Left));
        out.push(Para { runs, align, indents: base.indents });
        pos = end;
    }
    out
}

/// Text streams of versions 7 to X5: each paragraph is a style id, a list of style overrides
/// and the characters. The header before the first paragraph changed often, so the first
/// paragraph is found by trying each offset until the whole stream reads cleanly.
/// `placed`: text whose characters can be followed by the place of each one (text fitted to a path,
/// characters moved one by one).
fn paragraphs_binary(cx: &Ctx, b: &[u8], artistic: bool, placed: bool) -> Vec<Para> {
    let descr = if cx.ver >= 1200 { 8 } else { 4 };
    let counted = cx.ver >= 1200;
    // bytes between the style id and the override count, and after the characters
    let gap = if cx.ver >= 1300 && artistic { 2 } else { 1 };
    // one paragraph at `p`, where the next one starts and whether its style is in the table
    let one = |p: usize, tail: usize| -> Option<(Para, usize, bool)> {
        {
            let mut r = Rd::at(b, p);
            let style = r.u32();
            r.skip(gap);
            let n = r.u32() as usize;
            if n > 512 {
                return None;
            }
            let base = cx.styles.look(style, cx);
            let mut looks = vec![];
            for _ in 0..n {
                looks.push(override_look(cx, &mut r).over(&base));
                if r.p > b.len() {
                    return None;
                }
            }
            let (runs, end) = characters(b, r.p, descr, counted)?;
            if runs.is_empty() && n == 0 {
                return None;
            }
            let runs = runs
                .into_iter()
                .map(|(i, text)| {
                    let look = looks.get(i).cloned().unwrap_or_else(|| base.clone());
                    Run { text: decode(&text, look.charset), look }
                })
                .collect();
            Some((Para { runs, align: base.align, indents: base.indents }, end + tail, cx.styles.known(style)))
        }
    };
    // the paragraphs from `start`, and whether all their styles are in the table
    let read_all = |(start, tail): (usize, usize), lenient: bool| -> Option<(Vec<Para>, bool)> {
        let mut out = vec![];
        let mut known = true;
        let mut p = start;
        while p + 12 <= b.len() {
            match one(p, tail) {
                Some((para, next, k)) => {
                    out.push(para);
                    known &= k;
                    p = next;
                }
                None if lenient && out.iter().any(|p: &Para| !p.runs.is_empty()) => return Some((out, known)),
                None => return None,
            }
        }
        // nothing but padding may remain
        if out.is_empty() || b[p.min(b.len())..].iter().any(|x| *x != 0) {
            return None;
        }
        Some((out, known))
    };
    let tails: &[usize] = if cx.ver >= 1300 { &[1] } else { &[0, 1] };
    let starts = || (0x20..b.len().saturating_sub(12)).flat_map(|s| tails.iter().map(move |t| (s, *t)));
    let chars = |ps: &Vec<Para>| ps.iter().flat_map(|p| &p.runs).map(|r| r.text.chars().count()).sum::<usize>();
    // a start a byte early can read just as cleanly, with a style id that is not in the table:
    // the first reading whose styles are all known, else the first one
    let mut first = None;
    for (ps, known) in starts().filter_map(|st| read_all(st, false)).filter(|(ps, _)| chars(ps) > 0) {
        if known {
            return ps;
        }
        first.get_or_insert(ps);
    }
    if let Some(ps) = first {
        return ps;
    }
    if placed {
        // the places of the characters follow them, so a wrong start can read a few of their
        // bytes as text: take the reading with the most characters, known styles first
        return starts().filter_map(|st| read_all(st, true)).max_by_key(|(ps, known)| (*known, chars(ps))).map(|(ps, _)| ps).unwrap_or_default();
    }
    vec![]
}

/// One style override inside a text stream: flag bytes say which fields follow.
fn override_look(cx: &Ctx, r: &mut Rd) -> Look {
    let mut l = Look::default();
    let _chars = r.u16();
    let fl2 = r.u8();
    let fl3 = if cx.ver >= 800 { r.u8() } else { 0 };
    if fl2 & 0x01 != 0 {
        let font = r.u16() as u32;
        let charset = r.u16();
        l.font = cx.fonts.get(&font).cloned();
        l.charset = Some(charset).filter(|c| *c != 0).or_else(|| cx.charsets.get(&font).copied());
    }
    if fl2 & 0x02 != 0 {
        // weight and slant, and the lines
        let flags = r.u32();
        let (bold, italic) = weight(flags);
        let (underline, strike) = decorations(flags);
        l.bold = Some(bold);
        l.italic = Some(italic);
        l.underline = Some(underline);
        l.strike = Some(strike);
    }
    if fl2 & 0x04 != 0 {
        let s = cx.coord(r) * cx.k;
        if s > 0.1 {
            l.size = Some(s);
        }
    }
    if fl2 & 0x08 != 0 {
        r.skip(4);
    }
    if fl2 & 0x10 != 0 {
        r.skip(4);
    }
    if fl2 & 0x20 != 0 {
        // shift up from the baseline, in hundredths of the character height
        l.shift = Some(r.i32() as f64 / 100.0);
    }
    if fl2 & 0x40 != 0 {
        let fill = r.u32();
        if let Some(super::build::RawFill::Solid(c)) = cx.fills.get(&fill) {
            l.color = Some(*c);
        }
        // and the fill's matrix, six doubles
        if cx.ver >= 1300 {
            r.skip(48);
        }
    }
    if fl2 & 0x80 != 0 {
        r.skip(4);
    }
    if fl3 & 0x08 != 0 {
        if cx.ver >= 1300 {
            let n = r.u32() as usize;
            r.skip(n.min(4096) * 2);
        } else {
            r.skip(4);
        }
    }
    if fl3 & 0x20 != 0 {
        // a zero byte alone, else a fill and its 48-byte settings
        if r.u8() != 0 {
            r.skip(52);
        }
    }
    l
}

// ---------------------------------------------------------------- objects

/// The text stream of a text object: inside the object, or shared in the document by id.
fn stream<'a>(obj: &'a Chunk) -> Option<&'a [u8]> {
    fn find<'a>(c: &'a Chunk) -> Option<&'a Chunk> {
        if c.is(b"txsm") {
            return Some(c);
        }
        c.children.iter().find_map(find)
    }
    find(obj).map(|c| &c.data[..])
}

/// Artistic text whose characters have places of their own: fitted to a path, or moved one by one.
const ARG_PLACED: u32 = 0x3e8f;

pub(super) fn object(cx: &mut Ctx, obj: &Chunk, l: &Loda, world: Affine) -> Option<Object> {
    let paragraph = l.kind == 6;
    let k = cx.k;
    let data = stream(obj)?;
    let mut paras = if cx.ver >= 1600 { paragraphs_json(cx, data) } else if cx.ver >= 700 { paragraphs_binary(cx, data, !paragraph, l.arg(ARG_PLACED).is_some()) } else { vec![] };
    if paras.is_empty() {
        return None;
    }
    // the object's own style: the JSON properties (X6 and later) or the style table
    let own = match l.arg(ARG_STYLE_JSON) {
        Some(a) if a.len() > 8 => {
            let wide = cx.ver < 1700;
            find_json(a, 0, wide).and_then(|(s, n)| decode_json(&a[s..s + n], wide)).map(|v| json_look(&v, k)).unwrap_or_default()
        }
        _ => match l.arg(ARG_STYLE) {
            Some(a) => cx.styles.look(Rd::new(a).u32(), cx),
            None => Look::default(),
        },
    };
    let first = paras[0].runs.first().map(|r| r.look.over(&own)).unwrap_or_else(|| own.clone());
    let format = first.format();
    let mut content = String::new();
    let mut spans = vec![];
    let last = paras.len() - 1;
    for (i, p) in paras.iter_mut().enumerate() {
        for run in &p.runs {
            let look = run.look.over(&own);
            let start = content.chars().count();
            content.push_str(&run.text);
            let end = content.chars().count();
            let mut f = look.format();
            // a run colored unlike the object's text
            f.color = look.color.filter(|c| Some(*c) != first.color);
            if f != format && end > start {
                spans.push(Span { start, end, format: f });
            }
        }
        if i != last {
            content.push('\n');
        }
    }
    while content.ends_with('\n') {
        content.pop();
    }
    if content.trim().is_empty() {
        return None;
    }
    let mut t;
    let xf;
    if paragraph {
        // the frame: left, width, height from its top-left corner
        let a = l.arg(0x1e).unwrap_or(&[]);
        let mut r = Rd::new(a);
        r.skip(if cx.wide() { 4 } else { 2 });
        let w = cx.coord(&mut r).abs() * k;
        let h = cx.coord(&mut r).abs() * k;
        t = TextObj::paragraph(&content, w.max(1.0), h.max(1.0), format);
        let binary = if cx.ver < 1600 { binary_columns(data, w / k) } else { None };
        if let Some(cols) = binary.as_ref().or(own.columns.as_ref()) {
            let w = w.max(1.0);
            t.columns = cols.len() as u32;
            let n = cols.len() - 1;
            let equal = cols.iter().all(|c| (c.0 - cols[0].0).abs() < 1e-3) && cols[..n].iter().all(|c| (c.1 - cols[0].1).abs() < 1e-3);
            if equal {
                t.column_gap = cols[0].1 * w;
            } else {
                t.column_widths = cols.iter().map(|(cw, g)| (cw * w, g * w)).collect();
            }
        }
        xf = world * Affine::scale_non_uniform(1.0 / k, -1.0 / k);
    } else {
        t = TextObj::artistic(&content, format);
        xf = world * Affine::scale_non_uniform(1.0 / k, -1.0 / k);
    }
    t.spans = spans;
    if let Some(v) = first.line {
        t.line_spacing = v;
    }
    if let Some(v) = first.para {
        t.para_spacing = v;
    }
    if let Some(v) = first.words {
        t.word_spacing = v;
    }
    if let Some(a) = paras[0].align.or(first.align).or(own.align) {
        t.align = a;
    }
    // indents: CorelDRAW measures the first line's from the frame, AnyVector from the left indent
    let indents = |p: &Para| {
        let [first, left, right]: [Option<f64>; 3] = std::array::from_fn(|i| p.indents[i].or(own.indents[i]));
        let left = left.unwrap_or(0.0);
        (first.map(|f| f - left), left, right.unwrap_or(0.0))
    };
    if paragraph {
        let (f0, l0, r0) = indents(&paras[0]);
        t.first_indent = f0.unwrap_or(0.0);
        t.left_indent = l0;
        t.right_indent = r0;
        // the paragraphs that differ from the first
        let styles: Vec<ParaStyle> = paras
            .iter()
            .map(|p| {
                let (f, l, r) = indents(p);
                // a paragraph without an alignment of its own is left aligned
                let a = p.align.or(own.align).or(Some(TextAlign::Left));
                ParaStyle {
                    align: a.filter(|a| *a != t.align),
                    first_indent: Some(f.unwrap_or(0.0)).filter(|v| (v - t.first_indent).abs() > 0.01),
                    left_indent: Some(l).filter(|v| (v - t.left_indent).abs() > 0.01),
                    right_indent: Some(r).filter(|v| (v - t.right_indent).abs() > 0.01),
                }
            })
            .collect();
        if styles.iter().any(|s| *s != ParaStyle::default()) {
            t.paragraphs = styles;
        }
    }
    let mut st = Style::text_default();
    if let Fill::Solid(c) = cx.style_of(l, kurbo::Rect::new(0.0, 0.0, 1.0, 1.0), Affine::IDENTITY).fill {
        st.fill = Fill::Solid(c);
    }
    st.outline = Outline::none();
    if let Some(c) = first.color {
        st.fill = Fill::Solid(c);
    }
    let id = cx.doc.new_id();
    if l.arg(ARG_PLACED).is_some() {
        if let Some(place) = path_place(data, k) {
            cx.path_places.insert(id, place);
        }
    }
    Some(Object::new(id, ObjKind::Text(t), xf, st))
}

/// Where text fitted to a path sits on it.
#[derive(Clone, Copy, Debug)]
pub(super) struct PathPlace {
    /// Where the text starts, as a share of the path's length from its first node.
    pub start: f64,
    /// Distance from the path in points.
    pub distance: f64,
    /// On the other side of the path, running from its end.
    pub flip: bool,
    /// The placement: 1 to 3 on an open path (left, right, center), and on an ellipse the side the
    /// text is centered on (1 top, 2 left, 3 bottom, 4 right).
    pub placement: i32,
}

/// Put text that `fit_text_to_path` fitted to a path where CorelDRAW has it. `ellipse`: the path
/// is a whole ellipse, its width and height and the map from its space to the text's.
pub(super) fn place_on_path(t: &mut TextObj, place: PathPlace, ellipse: Option<(f64, f64, Affine)>) {
    let Some(tp) = &mut t.on_path else { return };
    tp.distance = place.distance;
    tp.flip = place.flip;
    if let Some((w, h, m)) = ellipse.filter(|_| (1..=4).contains(&place.placement)) {
        // CorelDRAW centers the text on one side of an ellipse (top, left, bottom, right) and goes
        // round it clockwise: start across from that side and center the text half-way round
        let (c, r) = (Point::new(w / 2.0, h / 2.0), kurbo::Vec2::new(w / 2.0, h / 2.0));
        let at = |a: f64| m * Point::new(c.x + r.x * a.cos(), c.y + r.y * a.sin());
        let key = |p: Point| match place.placement {
            1 => p.y,
            2 => p.x,
            3 => -p.y,
            _ => -p.x,
        };
        let start = (0..720).map(|i| i as f64 * std::f64::consts::PI / 360.0).max_by(|a, b| key(at(*a)).total_cmp(&key(at(*b)))).unwrap_or(0.0);
        // clockwise on the page (y down) is a growing angle, unless the map mirrors
        let sweep = if m.determinant() < 0.0 { -std::f64::consts::TAU } else { std::f64::consts::TAU };
        let mut bp = kurbo::BezPath::new();
        bp.move_to(at(start));
        kurbo::Arc::new(c, r, start, sweep, 0.0).to_cubic_beziers(0.01, |p1, p2, p3| bp.curve_to(m * p1, m * p2, m * p3));
        tp.path = vector_core::path::NodePath::from_bez(&bp);
        t.align = TextAlign::Center;
        tp.offset = vector_core::path::bez_length(&bp) / 2.0;
        return;
    }
    let len = vector_core::path::bez_length(&tp.path.to_bez());
    tp.offset = if place.flip { (1.0 - place.start) * len } else { place.start * len };
}


/// The stream starts with the text's matrix (six doubles), then: a number, the other-side flag, two
/// numbers, the start on the path in ten-thousandths of its length, the offset, a number, the
/// distance from the path and the placement (1 to 4). The start already takes the offset and the
/// placement into account. Recent versions put the matrix 4 bytes further.
/// The columns of a paragraph frame in the text stream of X5 and older: a count, then each
/// column's width and the gutter after it, in file units, near the stream's start (the count
/// at 0x71 in X5, then a 16-bit word, then the pairs). Found by their sum, which is the frame's
/// width (`width`, file units). One column gives
/// `None`. Returned as shares of the width, as the JSON of later versions has them.
fn binary_columns(b: &[u8], width: f64) -> Option<Vec<(f64, f64)>> {
    if width < 1.0 {
        return None;
    }
    let head = &b[..b.len().min(0x200)];
    (0..head.len().saturating_sub(4)).flat_map(|at| [(at, 6), (at, 4)]).find_map(|(at, gap)| {
        let n = Rd::at(head, at).u32() as usize;
        if !(2..=16).contains(&n) || at + gap + 8 * n > head.len() {
            return None;
        }
        let mut r = Rd::at(head, at + gap);
        let cols: Vec<(f64, f64)> = (0..n).map(|_| (r.i32() as f64, r.i32() as f64)).collect();
        let sum: f64 = cols.iter().map(|c| c.0 + c.1).sum();
        let sane = cols.iter().all(|c| c.0 > 0.0 && c.1 >= 0.0) && cols[n - 1].1 == 0.0;
        (sane && (sum - width).abs() <= width * 0.01).then(|| cols.iter().map(|(cw, g)| (cw / width, g / width)).collect())
    })
}

fn path_place(b: &[u8], k: f64) -> Option<PathPlace> {
    [0x31, 0x2d].into_iter().find_map(|m| {
        let at = m + 48;
        if at + 36 > b.len() {
            return None;
        }
        let f: Vec<i32> = (0..9).map(|i| Rd::at(b, at + 4 * i).i32()).collect();
        let ok = (0..=1).contains(&f[1]) && (0..=10_000).contains(&f[4]) && (1..=4).contains(&f[8]);
        ok.then(|| PathPlace { start: f[4] as f64 / 10_000.0, distance: f[7] as f64 * k, flip: f[1] == 1, placement: f[8] })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_byte_text_takes_the_fonts_character_set() {
        let raw: String = [0xcf, 0xf0, 0xe8].iter().map(|b| char::from_u32(RAW + *b as u32).unwrap()).collect();
        assert_eq!(decode(&format!("A{raw}"), Some(0xcc)), "A\u{41f}\u{440}\u{438}");
        assert_eq!(decode(&raw, None), "\u{cf}\u{f0}\u{e8}");
        assert_eq!(charset_of_name("Arial Cyr"), Some(("Arial".to_string(), 0xcc)));
    }

    #[test]
    fn finds_length_prefixed_json() {
        let mut b = vec![9u8, 9, 9];
        let s = br#"{"a":{"b":"1"}}"#;
        b.extend((s.len() as u32).to_le_bytes());
        b.extend_from_slice(s);
        b.extend([0, 0]);
        let (at, len) = find_json(&b, 0, false).unwrap();
        assert_eq!(&b[at..at + len], s);
        assert!(decode_json(&b[at..at + len], false).is_some());
    }

    #[test]
    fn character_block_with_runs() {
        // 3 characters: "A" in style 0, "é" (two bytes) and "b" in style 1
        let mut b = 3u32.to_le_bytes().to_vec();
        b.extend([0, 0, 0, 0, 0, 0, 0, 0]);
        b.extend([1, 0, 2, 0, 0, 0, 0, 0]);
        b.extend([0, 0, 2, 0, 0, 0, 0, 0]);
        b.extend(4u32.to_le_bytes());
        b.extend([b'A', 0xe9, 0x00, b'b']);
        let (runs, end) = characters(&b, 0, 8, true).unwrap();
        assert_eq!(runs, vec![(0, "A".to_string()), (1, "\u{e9}b".to_string())]);
        assert_eq!(end, b.len());
        assert_eq!(cp1252(0xe7), '\u{e7}');
        assert_eq!(cp1252(0x80), '\u{20ac}');
    }

    #[test]
    fn colors_in_json() {
        assert_eq!(json_color("RGB255,USER,220,0,0,100,0000"), Some(Color::rgb(220, 0, 0)));
        assert_eq!(json_color("CMYK,USER,0,100,100,0,100,0000"), Some(Color::rgb(237, 50, 55)));
    }
}
