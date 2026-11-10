//! Text of a CorelDRAW drawing: the font table, the style table and the text streams.

use super::build::{Ctx, Loda, ARG_STYLE, ARG_STYLE_JSON};
use super::color;
use super::riff::{Chunk, Rd};
use kurbo::Affine;
use std::collections::HashMap;
use vector_core::color::Color;
use vector_core::object::{ObjKind, Object};
use vector_core::style::{Fill, Outline, Style};
use vector_core::text::{CharFormat, Span, TextAlign, TextObj};

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
        let flags = r.u32() & 0xffff;
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
                let charset = Some(*charset).filter(|c| *c != 0).or_else(|| cx.charsets.get(font).copied());
                Look { font: cx.fonts.get(font).cloned(), size: Some(*size), bold: Some(bold), italic: Some(italic), charset, ..Default::default() }
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

/// Weight and slant flags: one bit per face, upright on even bits and italic on odd ones;
/// 0x40 regular, 0x80 italic, 0x1000 bold, 0x2000 bold italic.
fn weight(flags: u32) -> (bool, bool) {
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
        out.push(Para { runs, align: base.align });
        pos = end;
    }
    out
}

/// Text streams of versions 7 to X5: each paragraph is a style id, a list of style overrides
/// and the characters. The header before the first paragraph changed often, so the first
/// paragraph is found by trying each offset until the whole stream reads cleanly.
fn paragraphs_binary(cx: &Ctx, b: &[u8], artistic: bool) -> Vec<Para> {
    let descr = if cx.ver >= 1200 { 8 } else { 4 };
    let counted = cx.ver >= 1200;
    // bytes between the style id and the override count, and after the characters
    let gap = if cx.ver >= 1300 && artistic { 2 } else { 1 };
    let read_all = |(start, tail): (usize, usize)| -> Option<Vec<Para>> {
        let mut out = vec![];
        let mut p = start;
        while p + 12 <= b.len() {
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
            out.push(Para { runs, align: base.align });
            p = end + tail;
        }
        // nothing but padding may remain
        if out.is_empty() || b[p.min(b.len())..].iter().any(|x| *x != 0) {
            return None;
        }
        Some(out)
    };
    let tails: &[usize] = if cx.ver >= 1300 { &[1] } else { &[0, 1] };
    (0x20..b.len().saturating_sub(12)).flat_map(|s| tails.iter().map(move |t| (s, *t))).find_map(read_all).unwrap_or_default()
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
        // weight and slant
        let (bold, italic) = weight(r.u32());
        l.bold = Some(bold);
        l.italic = Some(italic);
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
        if cx.ver >= 1500 {
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

pub(super) fn object(cx: &mut Ctx, obj: &Chunk, l: &Loda, world: Affine) -> Option<Object> {
    let paragraph = l.kind == 6;
    let k = cx.k;
    let data = stream(obj)?;
    let mut paras = if cx.ver >= 1600 { paragraphs_json(cx, data) } else if cx.ver >= 700 { paragraphs_binary(cx, data, !paragraph) } else { vec![] };
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
    let mut st = Style::text_default();
    if let Fill::Solid(c) = cx.style_of(l, kurbo::Rect::new(0.0, 0.0, 1.0, 1.0), Affine::IDENTITY).fill {
        st.fill = Fill::Solid(c);
    }
    st.outline = Outline::none();
    if let Some(c) = first.color {
        st.fill = Fill::Solid(c);
    }
    Some(Object::new(cx.doc.new_id(), ObjKind::Text(t), xf, st))
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
