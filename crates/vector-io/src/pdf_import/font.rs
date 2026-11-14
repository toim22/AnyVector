//! PDF fonts: how a shown string splits into codes, what each code means (Unicode), how wide
//! it is, and its outline when the font program is embedded (TrueType, OpenType and CFF
//! through ttf-parser; Type 1 programs are not read). Fonts that are not embedded are matched
//! to installed fonts by name.

use super::{encodings, lexer, Pdf};
use kurbo::{Affine, BezPath};
use lopdf::{Dictionary, Object};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// A glyph name's Unicode text (Adobe Glyph List subset, `uniXXXX`, `uXXXX[XX]`).
pub fn glyph_unicode(name: &str) -> Option<String> {
    static LIST: OnceLock<HashMap<&'static str, char>> = OnceLock::new();
    let list = LIST.get_or_init(|| {
        include_str!("glyphlist.txt")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let (n, c) = l.split_once(' ')?;
                Some((n, char::from_u32(u32::from_str_radix(c, 16).ok()?)?))
            })
            .collect()
    });
    let base = name.split('.').next().unwrap_or(name);
    if let Some(c) = list.get(base) {
        // ligatures as their letters, so the text stays editable and spells right
        let lig = match c {
            '\u{FB00}' => "ff",
            '\u{FB01}' => "fi",
            '\u{FB02}' => "fl",
            '\u{FB03}' => "ffi",
            '\u{FB04}' => "ffl",
            '\u{FB05}' | '\u{FB06}' => "st",
            _ => return Some(c.to_string()),
        };
        return Some(lig.to_string());
    }
    if let Some(hex) = base.strip_prefix("uni") {
        if hex.len() >= 4 && hex.len() % 4 == 0 {
            let s: String = (0..hex.len() / 4).filter_map(|i| u32::from_str_radix(&hex[i * 4..i * 4 + 4], 16).ok().and_then(char::from_u32)).collect();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    if let Some(hex) = base.strip_prefix('u') {
        if (4..=6).contains(&hex.len()) {
            if let Some(c) = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                return Some(c.to_string());
            }
        }
    }
    // ligatures written as a_b
    if base.contains('_') {
        let s: Option<String> = base.split('_').map(glyph_unicode).collect();
        return s;
    }
    None
}

/// The glyph name of a character in the Adobe Glyph List subset (for looking glyphs up by name).
fn unicode_glyph_name(c: char) -> Option<&'static str> {
    static REV: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
    let rev = REV.get_or_init(|| {
        let mut m = HashMap::new();
        for l in include_str!("glyphlist.txt").lines().filter(|l| !l.starts_with('#')) {
            if let Some((n, c)) = l.split_once(' ') {
                if let Some(c) = u32::from_str_radix(c, 16).ok().and_then(char::from_u32) {
                    m.entry(c).or_insert(n);
                }
            }
        }
        m
    });
    rev.get(&c).copied()
}

/// How the bytes of a Type 0 font's strings split into codes and map to CIDs.
#[derive(Debug, Default)]
pub struct CMap {
    /// Codespace ranges: (byte length, low, high).
    pub space: Vec<(usize, u32, u32)>,
    /// CID ranges: (low code, high code, first CID).
    pub cids: Vec<(u32, u32, u32)>,
    /// The code is the CID (Identity-H/V and unknown CMaps).
    pub identity: bool,
    /// The code is Unicode (UCS-2/UTF-16 CMaps).
    pub unicode: bool,
}

impl CMap {
    fn identity() -> CMap {
        CMap { space: vec![(2, 0, 0xFFFF)], identity: true, ..Default::default() }
    }

    /// The next code in `s` from byte `i`: (code, its length).
    fn next(&self, s: &[u8], i: usize) -> (u32, usize) {
        let mut code = 0u32;
        for n in 1..=4usize {
            if i + n > s.len() {
                break;
            }
            code = (code << 8) | s[i + n - 1] as u32;
            if self.space.iter().any(|(len, lo, hi)| *len == n && code >= *lo && code <= *hi) {
                return (code, n);
            }
        }
        // not in any range: take the shortest length the CMap uses
        let n = self.space.iter().map(|s| s.0).min().unwrap_or(2).min(s.len() - i).max(1);
        let mut code = 0u32;
        for k in 0..n {
            code = (code << 8) | s[i + k] as u32;
        }
        (code, n)
    }

    fn cid(&self, code: u32) -> u32 {
        if self.identity || self.unicode {
            return code;
        }
        for (lo, hi, cid) in &self.cids {
            if code >= *lo && code <= *hi {
                return cid + (code - lo);
            }
        }
        0
    }

    /// An embedded CMap stream.
    fn parse(data: &[u8]) -> CMap {
        let mut m = CMap::default();
        for op in lexer::parse(data) {
            match op.op.as_slice() {
                b"endcodespacerange" => {
                    for p in op.args.chunks(2) {
                        if let (Ok(lo), Ok(hi)) = (p[0].as_str(), p.get(1).map_or(Err(lopdf::Error::CharacterEncoding), |h| h.as_str())) {
                            m.space.push((lo.len(), be(lo), be(hi)));
                        }
                    }
                }
                b"endcidrange" => {
                    for p in op.args.chunks(3) {
                        if p.len() == 3 {
                            if let (Ok(lo), Ok(hi), Ok(c)) = (p[0].as_str(), p[1].as_str(), p[2].as_i64()) {
                                m.cids.push((be(lo), be(hi), c.max(0) as u32));
                            }
                        }
                    }
                }
                b"endcidchar" => {
                    for p in op.args.chunks(2) {
                        if p.len() == 2 {
                            if let (Ok(c), Ok(cid)) = (p[0].as_str(), p[1].as_i64()) {
                                let c = be(c);
                                m.cids.push((c, c, cid.max(0) as u32));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if m.space.is_empty() {
            m.space.push((2, 0, 0xFFFF));
        }
        m
    }
}

fn be(b: &[u8]) -> u32 {
    b.iter().take(4).fold(0u32, |a, c| (a << 8) | *c as u32)
}

fn utf16(b: &[u8]) -> String {
    let units: Vec<u16> = b.chunks(2).map(|c| ((c[0] as u16) << 8) | *c.get(1).unwrap_or(&0) as u16).collect();
    String::from_utf16_lossy(&units)
}

/// A ToUnicode CMap: code → text.
fn parse_to_unicode(data: &[u8]) -> HashMap<u32, String> {
    let mut m = HashMap::new();
    for op in lexer::parse(data) {
        match op.op.as_slice() {
            b"endbfchar" => {
                for p in op.args.chunks(2) {
                    if p.len() == 2 {
                        if let (Ok(src), Ok(dst)) = (p[0].as_str(), p[1].as_str()) {
                            m.insert(be(src), utf16(dst));
                        } else if let (Ok(src), Ok(name)) = (p[0].as_str(), p[1].as_name()) {
                            if let Some(u) = glyph_unicode(&String::from_utf8_lossy(name)) {
                                m.insert(be(src), u);
                            }
                        }
                    }
                }
            }
            b"endbfrange" => {
                for p in op.args.chunks(3) {
                    if p.len() < 3 {
                        continue;
                    }
                    let (Ok(lo), Ok(hi)) = (p[0].as_str(), p[1].as_str()) else { continue };
                    let (lo, hi) = (be(lo), be(hi));
                    if hi < lo || hi - lo > 65535 {
                        continue;
                    }
                    match &p[2] {
                        Object::String(dst, _) => {
                            let mut units: Vec<u16> = dst.chunks(2).map(|c| ((c[0] as u16) << 8) | *c.get(1).unwrap_or(&0) as u16).collect();
                            for code in lo..=hi {
                                m.insert(code, String::from_utf16_lossy(&units));
                                if let Some(last) = units.last_mut() {
                                    *last = last.wrapping_add(1);
                                }
                            }
                        }
                        Object::Array(a) => {
                            for (k, d) in a.iter().enumerate() {
                                if let Ok(d) = d.as_str() {
                                    m.insert(lo + k as u32, utf16(d));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    m
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramKind {
    /// TrueType or OpenType (sfnt) data.
    Sfnt,
    /// A bare CFF font (FontFile3 /Type1C or /CIDFontType0C).
    Cff,
}

pub struct Program {
    pub kind: ProgramKind,
    pub data: Vec<u8>,
}

/// A Type 3 font: glyphs are content streams.
pub struct Type3<'a> {
    pub procs: &'a Dictionary,
    pub resources: Option<&'a Dictionary>,
    pub matrix: Affine,
}

pub struct Font<'a> {
    /// The font's PostScript name, without a subset prefix.
    pub name: String,
    /// The installed family that shows it, its weight and slant.
    pub family: String,
    pub bold: bool,
    pub italic: bool,
    /// The family is installed (or is the standard stand-in for one of the 14 standard fonts).
    pub installed: bool,
    /// Codes are split by this CMap (Type 0 fonts); simple fonts use one byte per code.
    pub cmap: Option<CMap>,
    pub vertical: bool,
    /// Glyph widths in text space (1 = the font size), by code (simple) or CID (Type 0).
    widths: HashMap<u32, f64>,
    default_width: f64,
    to_unicode: HashMap<u32, String>,
    /// Simple fonts: the glyph name of each code.
    names: Vec<Option<String>>,
    pub program: Option<Program>,
    /// Type 0 TrueType: CID → glyph id (None = identity).
    cid_to_gid: Option<Vec<u16>>,
    pub symbolic: bool,
    pub type3: Option<Type3<'a>>,
    glyphs: RefCell<HashMap<u32, Option<Arc<BezPath>>>>,
    texts: RefCell<HashMap<u32, String>>,
    /// The embedded program's cmap read backwards: glyph id → character.
    reverse: OnceLock<HashMap<u16, char>>,
    /// The installed face used to measure and draw glyphs when the font isn't embedded.
    installed_face: OnceLock<Option<(Arc<Vec<u8>>, u32)>>,
    /// The font uses a CMap AnyVector doesn't read (codes are taken as two bytes).
    pub unknown_cmap: bool,
}

/// One code of a shown string.
pub struct Glyph {
    pub code: u32,
    /// The code is a single byte 32 (word spacing applies).
    pub space: bool,
    pub cid: u32,
}

const STANDARD14: &[(&str, &str, bool, bool)] = &[
    ("Helvetica", "Arial", false, false),
    ("Helvetica-Bold", "Arial", true, false),
    ("Helvetica-Oblique", "Arial", false, true),
    ("Helvetica-BoldOblique", "Arial", true, true),
    ("Times-Roman", "Times New Roman", false, false),
    ("Times-Bold", "Times New Roman", true, false),
    ("Times-Italic", "Times New Roman", false, true),
    ("Times-BoldItalic", "Times New Roman", true, true),
    ("Courier", "Courier New", false, false),
    ("Courier-Bold", "Courier New", true, false),
    ("Courier-Oblique", "Courier New", false, true),
    ("Courier-BoldOblique", "Courier New", true, true),
    ("Symbol", "Symbol", false, false),
    ("ZapfDingbats", "Wingdings", false, false),
];

/// "MyriadPro" → "Myriad Pro", "TimesNewRomanPSMT" → "Times New Roman".
fn spaced(name: &str) -> String {
    let name = name.trim_end_matches("MT").trim_end_matches("PS");
    let mut out = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && c.is_uppercase() && (chars[i - 1].is_lowercase() || chars.get(i + 1).is_some_and(|n| n.is_lowercase()) && chars[i - 1].is_uppercase()) {
            out.push(' ');
        }
        out.push(*c);
    }
    out
}

/// The installed family for a PDF font name: (family, bold, italic, installed).
pub fn match_font(ps: &str, flags: i64, weight: Option<f64>) -> (String, bool, bool, bool) {
    if let Some((_, fam, b, i)) = STANDARD14.iter().find(|(n, ..)| *n == ps) {
        return (fam.to_string(), *b, *i, true);
    }
    let (fam_part, style) = match ps.find([',', '-']) {
        Some(k) => (&ps[..k], &ps[k + 1..]),
        None => (ps, ""),
    };
    let st = style.to_lowercase();
    let mut bold = ["bold", "black", "heavy", "semibold", "demi", "extrabold", "ultra"].iter().any(|w| st.contains(w)) || flags & (1 << 18) != 0 || weight.is_some_and(|w| w >= 600.0);
    let italic = st.contains("italic") || st.contains("oblique") || flags & 64 != 0;
    let db = vector_core::text::fonts().lock();
    let candidates = [ps.to_string(), fam_part.to_string(), spaced(fam_part)];
    for c in candidates.iter() {
        if c.is_empty() {
            continue;
        }
        let r = db.resolve(c);
        if !r.substituted() {
            if let Some(w) = r.weight {
                bold |= w.0 >= 600;
            }
            return (r.name, bold, italic, true);
        }
    }
    // the standard fonts under other names (Arial,Bold for Helvetica-Bold and the like)
    let lower = fam_part.to_lowercase();
    for (n, fam, ..) in STANDARD14 {
        if n.to_lowercase() == lower {
            return (fam.to_string(), bold, italic, !db.resolve(fam).substituted());
        }
    }
    (spaced(fam_part), bold, italic, false)
}

fn strip_subset(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() > 7 && b[6] == b'+' && b[..6].iter().all(|c| c.is_ascii_uppercase()) {
        &s[7..]
    } else {
        s
    }
}

impl<'a> Font<'a> {
    pub fn load(pdf: &Pdf<'a>, d: &'a Dictionary) -> Font<'a> {
        let subtype = pdf.name(d, b"Subtype").unwrap_or_default();
        let base = pdf.name(d, b"BaseFont").unwrap_or_default();
        let name = strip_subset(&base).to_string();
        let type0 = subtype == "Type0";
        // the dictionary with the widths and the descriptor
        let cid_font = if type0 { pdf.array(d, b"DescendantFonts").first().and_then(|f| pdf.dict(pdf.deref(f))) } else { None };
        let desc = pdf.get_dict(cid_font.unwrap_or(d), b"FontDescriptor");
        let flags = desc.and_then(|x| pdf.int(x, b"Flags")).unwrap_or(0);
        let weight = desc.and_then(|x| pdf.num(x, b"FontWeight"));
        let symbolic = flags & 4 != 0 && flags & 32 == 0;
        let display_name = if name.is_empty() { cid_font.and_then(|c| pdf.name(c, b"BaseFont")).map(|n| strip_subset(&n).to_string()).unwrap_or_default() } else { name.clone() };
        let (family, bold, italic, installed) = if subtype == "Type3" { (String::new(), false, false, false) } else { match_font(&display_name, flags, weight) };

        // font program
        let mut program = None;
        if let Some(desc) = desc {
            if let Some(s) = pdf.get(desc, b"FontFile2") {
                program = pdf.stream_data(s).map(|data| Program { kind: ProgramKind::Sfnt, data });
            } else if let Some(s) = pdf.get(desc, b"FontFile3") {
                let sub = pdf.dict(s).and_then(|sd| pdf.name(sd, b"Subtype")).unwrap_or_default();
                let kind = if sub == "OpenType" { ProgramKind::Sfnt } else { ProgramKind::Cff };
                program = pdf.stream_data(s).map(|data| Program { kind, data });
            }
        }
        // a program that doesn't parse is no program
        if let Some(p) = &program {
            let ok = match p.kind {
                ProgramKind::Sfnt => ttf_parser::Face::parse(&p.data, 0).is_ok(),
                ProgramKind::Cff => ttf_parser::cff::Table::parse(&p.data).is_some(),
            };
            if !ok {
                program = None;
            }
        }

        let to_unicode = pdf.get(d, b"ToUnicode").and_then(|s| pdf.stream_data(s)).map(|data| parse_to_unicode(&data)).unwrap_or_default();

        let mut f = Font {
            name: display_name,
            family,
            bold,
            italic,
            installed,
            cmap: None,
            vertical: false,
            widths: HashMap::new(),
            default_width: 0.0,
            to_unicode,
            names: vec![],
            program,
            cid_to_gid: None,
            symbolic,
            type3: None,
            glyphs: RefCell::new(HashMap::new()),
            texts: RefCell::new(HashMap::new()),
            reverse: OnceLock::new(),
            installed_face: OnceLock::new(),
            unknown_cmap: false,
        };

        if type0 {
            let enc = d.get(b"Encoding").ok().map(|e| pdf.deref(e));
            f.cmap = Some(match enc {
                Some(Object::Name(n)) => {
                    let n = String::from_utf8_lossy(n).into_owned();
                    f.vertical = n.ends_with("-V");
                    if n.starts_with("Identity") {
                        CMap::identity()
                    } else if n.contains("UCS2") || n.contains("UTF16") {
                        CMap { space: vec![(2, 0, 0xFFFF)], unicode: true, ..Default::default() }
                    } else {
                        f.unknown_cmap = true;
                        CMap::identity()
                    }
                }
                Some(s @ Object::Stream(_)) => {
                    f.vertical = pdf.dict(s).and_then(|sd| pdf.int(sd, b"WMode")).unwrap_or(0) == 1;
                    pdf.stream_data(s).map(|data| CMap::parse(&data)).unwrap_or_else(CMap::identity)
                }
                _ => CMap::identity(),
            });
            if let Some(cf) = cid_font {
                f.default_width = pdf.num(cf, b"DW").unwrap_or(1000.0) / 1000.0;
                let w = pdf.array(cf, b"W");
                let mut i = 0;
                while i < w.len() {
                    let first = pdf.deref(&w[i]).as_float().ok().map(|v| v as u32);
                    let Some(first) = first else { break };
                    match w.get(i + 1).map(|x| pdf.deref(x)) {
                        Some(Object::Array(ws)) => {
                            for (k, x) in ws.iter().enumerate() {
                                if let Ok(v) = pdf.deref(x).as_float() {
                                    f.widths.insert(first + k as u32, v as f64 / 1000.0);
                                }
                            }
                            i += 2;
                        }
                        Some(last) => {
                            let last = last.as_float().unwrap_or(0.0) as u32;
                            let v = w.get(i + 2).and_then(|x| pdf.deref(x).as_float().ok()).unwrap_or(0.0) as f64 / 1000.0;
                            if last >= first && last - first < 65536 {
                                for c in first..=last {
                                    f.widths.insert(c, v);
                                }
                            }
                            i += 3;
                        }
                        None => break,
                    }
                }
                match cf.get(b"CIDToGIDMap").ok().map(|m| pdf.deref(m)) {
                    Some(s @ Object::Stream(_)) => {
                        if let Some(data) = pdf.stream_data(s) {
                            f.cid_to_gid = Some(data.chunks(2).map(|c| ((c[0] as u16) << 8) | *c.get(1).unwrap_or(&0) as u16).collect());
                        }
                    }
                    _ => {}
                }
            }
        } else {
            // simple font: widths and glyph names
            let first = pdf.int(d, b"FirstChar").unwrap_or(0).max(0) as u32;
            let scale = if subtype == "Type3" { pdf.nums(d, b"FontMatrix").first().copied().unwrap_or(0.001) } else { 0.001 };
            for (k, x) in pdf.array(d, b"Widths").iter().enumerate() {
                if let Ok(v) = pdf.deref(x).as_float() {
                    f.widths.insert(first + k as u32, v as f64 * scale);
                }
            }
            f.default_width = desc.and_then(|x| pdf.num(x, b"MissingWidth")).filter(|w| *w > 0.0).map(|w| w * scale).unwrap_or(0.0);
            let mut names: Vec<Option<String>> = vec![None; 256];
            let base_table = |n: &str| -> Option<&'static [&'static str; 256]> {
                match n {
                    "WinAnsiEncoding" => Some(&encodings::WIN_ANSI),
                    "MacRomanEncoding" => Some(&encodings::MAC_ROMAN),
                    "StandardEncoding" => Some(&encodings::STANDARD),
                    "PDFDocEncoding" => Some(&encodings::PDF_DOC),
                    _ => None,
                }
            };
            let default_table: Option<&[&str; 256]> = if name == "Symbol" {
                Some(&encodings::SYMBOL)
            } else if name == "ZapfDingbats" || (symbolic && f.program.is_some()) || subtype == "Type3" {
                None
            } else {
                Some(&encodings::STANDARD)
            };
            let mut table = default_table;
            let mut diffs: Vec<(usize, String)> = vec![];
            match d.get(b"Encoding").ok().map(|e| pdf.deref(e)) {
                Some(Object::Name(n)) => {
                    if let Some(t) = base_table(&String::from_utf8_lossy(n)) {
                        table = Some(t);
                    }
                }
                Some(e) => {
                    if let Some(ed) = pdf.dict(e) {
                        if let Some(t) = pdf.name(ed, b"BaseEncoding").and_then(|n| base_table(&n)) {
                            table = Some(t);
                        }
                        let mut code = 0usize;
                        for x in pdf.array(ed, b"Differences") {
                            match pdf.deref(x) {
                                Object::Integer(c) => code = (*c).clamp(0, 255) as usize,
                                Object::Real(c) => code = (*c as i64).clamp(0, 255) as usize,
                                Object::Name(n) => {
                                    if code < 256 {
                                        diffs.push((code, String::from_utf8_lossy(n).into_owned()));
                                    }
                                    code += 1;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                None => {}
            }
            if let Some(t) = table {
                for (i, n) in t.iter().enumerate() {
                    if !n.is_empty() {
                        names[i] = Some(n.to_string());
                    }
                }
            }
            for (c, n) in diffs {
                names[c] = Some(n);
            }
            f.names = names;
            if subtype == "Type3" {
                let m = pdf.nums(d, b"FontMatrix");
                let matrix = if m.len() == 6 { Affine::new([m[0], m[1], m[2], m[3], m[4], m[5]]) } else { Affine::scale(0.001) };
                if let Some(procs) = pdf.get_dict(d, b"CharProcs") {
                    f.type3 = Some(Type3 { procs, resources: pdf.get_dict(d, b"Resources"), matrix });
                }
            }
        }
        f
    }

    /// The codes of a shown string.
    pub fn decode(&self, s: &[u8]) -> Vec<Glyph> {
        let mut out = vec![];
        match &self.cmap {
            None => {
                for b in s {
                    out.push(Glyph { code: *b as u32, space: *b == 32, cid: *b as u32 });
                }
            }
            Some(m) => {
                let mut i = 0;
                while i < s.len() {
                    let (code, n) = m.next(s, i);
                    out.push(Glyph { code, space: n == 1 && code == 32, cid: m.cid(code) });
                    i += n;
                }
            }
        }
        out
    }

    /// The glyph name of a simple font's code.
    pub fn glyph_name(&self, code: u32) -> Option<&str> {
        self.names.get(code as usize).and_then(|n| n.as_deref())
    }

    /// The text a code stands for.
    pub fn unicode(&self, g: &Glyph) -> String {
        if let Some(t) = self.texts.borrow().get(&g.code) {
            return t.clone();
        }
        let t = self.unicode_uncached(g);
        self.texts.borrow_mut().insert(g.code, t.clone());
        t
    }

    fn unicode_uncached(&self, g: &Glyph) -> String {
        if let Some(u) = self.to_unicode.get(&g.code) {
            return u.chars().filter(|c| *c != '\0').collect();
        }
        if let Some(m) = &self.cmap {
            if m.unicode {
                return char::from_u32(g.code).map(|c| c.to_string()).unwrap_or_default();
            }
            // from the embedded font's own character map
            if let Some(c) = self.reverse_cmap(self.gid(g)) {
                return c.to_string();
            }
            return String::new();
        }
        if let Some(u) = self.glyph_name(g.code).and_then(glyph_unicode) {
            return u;
        }
        if self.symbolic {
            if let Some(c) = self.program.as_ref().and_then(|_| self.reverse_cmap(self.gid(g))) {
                return c.to_string();
            }
        }
        match g.code {
            32..=126 | 160..=255 => char::from_u32(g.code).map(|c| c.to_string()).unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// The width of a glyph in text space (1 = the font size).
    pub fn width(&self, g: &Glyph) -> f64 {
        let key = if self.cmap.is_some() { g.cid } else { g.code };
        if let Some(w) = self.widths.get(&key) {
            return *w;
        }
        if self.cmap.is_some() {
            return self.default_width;
        }
        if self.default_width > 0.0 {
            return self.default_width;
        }
        // not listed (the standard fonts): measure the glyph in the installed font
        let u = self.unicode(g);
        u.chars().next().and_then(|c| self.installed_advance(c)).unwrap_or(0.5)
    }

    fn installed_face(&self) -> Option<(Arc<Vec<u8>>, u32)> {
        self.installed_face
            .get_or_init(|| {
                let mut db = vector_core::text::fonts().lock();
                let id = db.query(&self.family, self.bold, self.italic)?;
                let face = db.face(id)?;
                Some((face.data.clone(), face.index))
            })
            .clone()
    }

    fn installed_advance(&self, c: char) -> Option<f64> {
        let (data, index) = self.installed_face()?;
        let face = ttf_parser::Face::parse(&data, index).ok()?;
        let gid = face.glyph_index(c)?;
        Some(face.glyph_hor_advance(gid)? as f64 / face.units_per_em() as f64)
    }

    /// The outline of a character in the installed font that stands in for this one, in
    /// glyph space (1 = the font size), when the font program isn't embedded.
    pub fn installed_outline(&self, text: &str) -> Option<BezPath> {
        let c = text.chars().next()?;
        let (data, index) = self.installed_face()?;
        let face = ttf_parser::Face::parse(&data, index).ok()?;
        let gid = face.glyph_index(c)?;
        let mut b = Builder(BezPath::new());
        face.outline_glyph(gid, &mut b)?;
        Some(Affine::scale(1.0 / face.units_per_em() as f64) * b.0)
    }

    /// The glyph id of a code in the embedded program.
    fn gid(&self, g: &Glyph) -> u16 {
        let Some(p) = &self.program else { return 0 };
        match p.kind {
            ProgramKind::Sfnt => {
                if self.cmap.is_some() {
                    return match &self.cid_to_gid {
                        Some(map) => map.get(g.cid as usize).copied().unwrap_or(0),
                        None => g.cid as u16,
                    };
                }
                let Ok(face) = ttf_parser::Face::parse(&p.data, 0) else { return 0 };
                let Some(cmap) = face.tables().cmap else {
                    return self.glyph_name(g.code).and_then(|n| face.glyph_index_by_name(n)).map(|x| x.0).unwrap_or(g.code as u16);
                };
                let sub = |pid: ttf_parser::PlatformId, eid: u16| cmap.subtables.into_iter().find(|s| s.platform_id == pid && s.encoding_id == eid);
                // symbolic fonts: the (3,0) table at F0xx, else the Mac table by code
                if let Some(s) = sub(ttf_parser::PlatformId::Windows, 0) {
                    for c in [g.code, 0xF000 | g.code, 0xF100 | g.code, 0xF200 | g.code] {
                        if let Some(gid) = s.glyph_index(c) {
                            return gid.0;
                        }
                    }
                }
                if !self.symbolic || sub(ttf_parser::PlatformId::Macintosh, 0).is_none() {
                    if let Some(s) = sub(ttf_parser::PlatformId::Windows, 1).or_else(|| sub(ttf_parser::PlatformId::Unicode, 3)) {
                        let u = self.glyph_name(g.code).and_then(glyph_unicode).or_else(|| self.to_unicode.get(&g.code).cloned());
                        if let Some(c) = u.and_then(|u| u.chars().next()) {
                            if let Some(gid) = s.glyph_index(c as u32) {
                                return gid.0;
                            }
                        }
                    }
                }
                if let Some(s) = sub(ttf_parser::PlatformId::Macintosh, 0) {
                    if let Some(gid) = s.glyph_index(g.code) {
                        return gid.0;
                    }
                }
                if let Some(gid) = self.glyph_name(g.code).and_then(|n| face.glyph_index_by_name(n)) {
                    return gid.0;
                }
                g.code as u16
            }
            ProgramKind::Cff => {
                let Some(t) = ttf_parser::cff::Table::parse(&p.data) else { return 0 };
                if self.cmap.is_some() {
                    // CID-keyed: find the glyph whose CID this is
                    if t.glyph_cid(ttf_parser::GlyphId(0)).is_some() {
                        for gid in 0..t.number_of_glyphs() {
                            if t.glyph_cid(ttf_parser::GlyphId(gid)) == Some(g.cid as u16) {
                                return gid;
                            }
                        }
                        return 0;
                    }
                    return g.cid as u16;
                }
                if let Some(gid) = self.glyph_name(g.code).and_then(|n| t.glyph_index_by_name(n)) {
                    return gid.0;
                }
                // a name from the text, when the encoding gives none
                if let Some(n) = self.to_unicode.get(&g.code).and_then(|u| u.chars().next()).and_then(unicode_glyph_name) {
                    if let Some(gid) = t.glyph_index_by_name(n) {
                        return gid.0;
                    }
                }
                t.glyph_index(g.code as u8).map(|x| x.0).unwrap_or(0)
            }
        }
    }

    /// The character of a glyph in the embedded program's own cmap.
    fn reverse_cmap(&self, gid: u16) -> Option<char> {
        let map = self.reverse.get_or_init(|| {
            let mut m = HashMap::new();
            let Some(p) = self.program.as_ref().filter(|p| p.kind == ProgramKind::Sfnt) else { return m };
            let Ok(face) = ttf_parser::Face::parse(&p.data, 0) else { return m };
            let Some(cmap) = face.tables().cmap else { return m };
            for s in cmap.subtables {
                if !s.is_unicode() {
                    continue;
                }
                s.codepoints(|cp| {
                    if let (Some(g), Some(c)) = (s.glyph_index(cp), char::from_u32(cp)) {
                        m.entry(g.0).or_insert(c);
                    }
                });
            }
            m
        });
        map.get(&gid).copied()
    }

    /// The outline of a glyph from the embedded program, in glyph space (1 = the font size).
    pub fn outline(&self, g: &Glyph) -> Option<Arc<BezPath>> {
        let p = self.program.as_ref()?;
        let key = if self.cmap.is_some() { g.cid } else { g.code };
        if let Some(o) = self.glyphs.borrow().get(&key) {
            return o.clone();
        }
        let gid = self.gid(g);
        let mut b = Builder(BezPath::new());
        let out = match p.kind {
            ProgramKind::Sfnt => ttf_parser::Face::parse(&p.data, 0).ok().and_then(|face| {
                face.outline_glyph(ttf_parser::GlyphId(gid), &mut b)?;
                Some(Affine::scale(1.0 / face.units_per_em() as f64) * std::mem::take(&mut b.0))
            }),
            ProgramKind::Cff => ttf_parser::cff::Table::parse(&p.data).and_then(|t| {
                t.outline(ttf_parser::GlyphId(gid), &mut b).ok()?;
                let m = t.matrix();
                Some(Affine::new([m.sx as f64, m.ky as f64, m.kx as f64, m.sy as f64, m.tx as f64, m.ty as f64]) * std::mem::take(&mut b.0))
            }),
        };
        // an empty glyph (a space) is still a glyph
        let out = out.map(Arc::new).or_else(|| Some(Arc::new(BezPath::new())));
        self.glyphs.borrow_mut().insert(key, out.clone());
        out
    }
}

struct Builder(BezPath);

impl ttf_parser::OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((x as f64, y as f64));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((x as f64, y as f64));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.quad_to((x1 as f64, y1 as f64), (x as f64, y as f64));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.curve_to((x1 as f64, y1 as f64), (x2 as f64, y2 as f64), (x as f64, y as f64));
    }
    fn close(&mut self) {
        self.0.close_path();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_names_and_cmaps() {
        assert_eq!(glyph_unicode("A").as_deref(), Some("A"));
        assert_eq!(glyph_unicode("Euro").as_deref(), Some("\u{20AC}"));
        assert_eq!(glyph_unicode("uni00E9").as_deref(), Some("é"));
        assert_eq!(glyph_unicode("f_i").as_deref(), Some("fi"));
        assert_eq!(glyph_unicode("a.sc").as_deref(), Some("a"));
        let tu = parse_to_unicode(b"2 beginbfchar <0003> <0020> <0024> <0041> endbfchar 1 beginbfrange <0010> <0012> <0061> endbfrange");
        assert_eq!(tu.get(&0x24).map(|s| s.as_str()), Some("A"));
        assert_eq!(tu.get(&0x12).map(|s| s.as_str()), Some("c"));
        let cm = CMap::parse(b"1 begincodespacerange <00> <80> <8140> <FFFF> endcodespacerange 1 begincidrange <8140> <8142> 100 endcidrange");
        assert_eq!(cm.next(&[0x41, 0x81, 0x41], 0), (0x41, 1));
        assert_eq!(cm.next(&[0x41, 0x81, 0x41], 1), (0x8141, 2));
        assert_eq!(cm.cid(0x8141), 101);
        assert_eq!(spaced("MyriadPro"), "Myriad Pro");
        assert_eq!(spaced("TimesNewRomanPSMT"), "Times New Roman");
        assert_eq!(strip_subset("ABCDEF+Calibri-Bold"), "Calibri-Bold");
    }
}
