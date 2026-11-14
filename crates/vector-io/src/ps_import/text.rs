//! Fonts and text. Text in Type 1, TrueType (Type 42) and fonts the file only names becomes
//! artistic text in the installed font that matches the font's name; the glyphs of Type 3
//! fonts are drawn by their own procedures and come in as curves.

use super::gfx::{ctm_scale, dvec, mat_read};
use super::interp::{op, Flow, Interp, OpFn, PsErr, R};
use super::obj::*;
use kurbo::{Affine, BezPath, Point, Vec2};
use std::collections::HashMap;
use vector_core::object::{ObjKind, Object};
use vector_core::style::{Fill, Outline, Style};
use vector_core::text::{CharFormat, TextObj};

pub fn ops() -> Vec<(&'static str, OpFn)> {
    vec![
        op("findfont", findfont),
        op("definefont", definefont),
        op("undefinefont", |i| i.pop().map(|_| ())),
        op("composefont", |i| {
            // CMap and descendant fonts: a composite font named by the key
            let _fonts = i.pop()?;
            let _cmap = i.pop()?;
            let key = i.pop()?;
            i.push(key);
            findfont(i)
        }),
        op("scalefont", |i| {
            let s = i.pop_num()?;
            let f = i.pop_dict()?;
            let nf = transform_font(i, &f, Affine::scale(s))?;
            i.push(Obj::Dict(nf));
            Ok(())
        }),
        op("makefont", |i| {
            let m = i.pop_arr()?;
            let m = mat_read(&m)?;
            let f = i.pop_dict()?;
            let nf = transform_font(i, &f, m)?;
            i.push(Obj::Dict(nf));
            Ok(())
        }),
        op("setfont", |i| {
            let f = i.pop_dict()?;
            i.gs.font = Some(f);
            Ok(())
        }),
        op("currentfont", |i| {
            let f = match i.gs.font.clone() {
                Some(f) => f,
                None => {
                    let n = i.lit("Helvetica");
                    i.push(n);
                    findfont(i)?;
                    i.pop_dict()?
                }
            };
            i.push(Obj::Dict(f));
            Ok(())
        }),
        op("rootfont", |i| {
            let f = i.gs.font.clone().map(Obj::Dict).unwrap_or(Obj::Null);
            i.push(f);
            Ok(())
        }),
        op("selectfont", |i| {
            let s = i.pop()?;
            findfont(i)?;
            let f = i.pop_dict()?;
            let m = match s {
                Obj::Array(a, _) => mat_read(&a)?,
                o => Affine::scale(o.num().ok_or(Flow::Err(PsErr::TypeCheck))?),
            };
            let nf = transform_font(i, &f, m)?;
            i.gs.font = Some(nf);
            Ok(())
        }),
        op("show", |i| {
            let s = i.pop_str()?;
            i.show(&s.bytes(), Extra::default())
        }),
        op("ashow", |i| {
            let s = i.pop_str()?;
            let v = i.nums(2)?;
            i.show(&s.bytes(), Extra { a: Vec2::new(v[0], v[1]), ..Default::default() })
        }),
        op("widthshow", |i| {
            let s = i.pop_str()?;
            let c = i.pop_int()?;
            let v = i.nums(2)?;
            i.show(&s.bytes(), Extra { w: Vec2::new(v[0], v[1]), wc: Some(c as u8), ..Default::default() })
        }),
        op("awidthshow", |i| {
            let s = i.pop_str()?;
            let a = i.nums(2)?;
            let c = i.pop_int()?;
            let w = i.nums(2)?;
            i.show(&s.bytes(), Extra { a: Vec2::new(a[0], a[1]), w: Vec2::new(w[0], w[1]), wc: Some(c as u8), ..Default::default() })
        }),
        op("xshow", |i| xyshow(i, 1)),
        op("yshow", |i| xyshow(i, 2)),
        op("xyshow", |i| xyshow(i, 3)),
        op("kshow", |i| {
            let s = i.pop_str()?;
            let p = i.pop_proc()?.with_exec(true);
            let b = s.bytes();
            for (k, c) in b.iter().enumerate() {
                i.show(&[*c], Extra::default())?;
                if let Some(n) = b.get(k + 1) {
                    i.push(Obj::Int(*c as i64));
                    i.push(Obj::Int(*n as i64));
                    match i.exec_value(p.clone()) {
                        Err(Flow::Exit) => break,
                        r => r?,
                    }
                }
            }
            Ok(())
        }),
        op("cshow", |i| {
            let s = i.pop_str()?;
            let p = i.pop_proc()?.with_exec(true);
            for c in s.bytes() {
                let w = i.string_width(&[c])?;
                i.push(Obj::Int(c as i64));
                i.push(Obj::Real(w.x));
                i.push(Obj::Real(w.y));
                match i.exec_value(p.clone()) {
                    Err(Flow::Exit) => break,
                    r => r?,
                }
            }
            Ok(())
        }),
        op("glyphshow", |i| {
            let g = i.pop()?;
            let name = i.text_of(&g).unwrap_or_default();
            let c = super::encodings::glyph_char(&name).unwrap_or('?');
            i.show_chars(&[c], &[name], Extra::default())
        }),
        op("stringwidth", |i| {
            let s = i.pop_str()?;
            let w = i.string_width(&s.bytes())?;
            i.push(Obj::Real(w.x));
            i.push(Obj::Real(w.y));
            Ok(())
        }),
        op("charpath", |i| {
            let _stroke = i.pop_bool()?;
            let s = i.pop_str()?;
            i.charpath(&s.bytes())
        }),
        op("setcachedevice", |i| {
            let v = i.nums(6)?;
            i.glyph_width = Some((v[0], v[1]));
            Ok(())
        }),
        op("setcachedevice2", |i| {
            let v = i.nums(10)?;
            i.glyph_width = Some((v[0], v[1]));
            Ok(())
        }),
        op("setcharwidth", |i| {
            let v = i.nums(2)?;
            i.glyph_width = Some((v[0], v[1]));
            Ok(())
        }),
    ]
}

/// Extra spacing of `ashow`, `widthshow` and `awidthshow` (user space).
#[derive(Default, Clone, Copy)]
pub struct Extra {
    pub a: Vec2,
    pub w: Vec2,
    pub wc: Option<u8>,
}

fn xyshow(i: &mut Interp, how: u8) -> R {
    let d = i.pop()?;
    let s = i.pop_str()?;
    let nums: Vec<f64> = match d {
        Obj::Array(a, _) => a.items().iter().filter_map(|o| o.num()).collect(),
        Obj::Str(st, _) => super::ops_base::decode_number_string(&st.bytes()),
        _ => return Err(PsErr::TypeCheck.into()),
    };
    let b = s.bytes();
    let mut total = Vec2::ZERO;
    let mut k = 0;
    for _ in 0..b.len() {
        let v = match how {
            1 => Vec2::new(nums.get(k).copied().unwrap_or(0.0), 0.0),
            2 => Vec2::new(0.0, nums.get(k).copied().unwrap_or(0.0)),
            _ => {
                let v = Vec2::new(nums.get(k).copied().unwrap_or(0.0), nums.get(k + 1).copied().unwrap_or(0.0));
                k += 1;
                v
            }
        };
        k += 1;
        total += v;
    }
    // one text object with the spacing that makes it as long as the positions say
    let start = i.gs.cur.ok_or(Flow::Err(PsErr::NoCurrentPoint))?;
    let natural = i.string_width(&b)?;
    let n = b.len().max(1) as f64;
    let extra = (total - natural) / n;
    i.show(&b, Extra { a: extra, ..Default::default() })?;
    i.gs.cur = Some(start + dvec(i.gs.ctm, total.x, total.y));
    Ok(())
}

/// A font dictionary with its matrix changed.
fn transform_font(i: &mut Interp, f: &DictRef, m: Affine) -> R<DictRef> {
    let nf = new_dict(f.borrow().cap);
    for (k, v) in f.borrow().map.iter() {
        nf.borrow_mut().map.insert(k.clone(), v.clone());
    }
    let fm = match i.dget(f, "FontMatrix") {
        Some(Obj::Array(a, _)) => mat_read(&a).unwrap_or(Affine::scale(0.001)),
        _ => Affine::scale(0.001),
    };
    i.dput(&nf, "FontMatrix", super::gfx::mat_obj(m * fm));
    let base = i.dget(f, "OrigFont").unwrap_or(Obj::Dict(f.clone()));
    i.dput(&nf, "OrigFont", base);
    Ok(nf)
}

pub fn findfont(i: &mut Interp) -> R {
    let k = i.pop()?;
    let key = i.to_key(&k)?;
    let found = i.fontdir.borrow().map.get(&key).cloned();
    if let Some(f) = found {
        i.push(f);
        return Ok(());
    }
    let name = i.text_of(&k).unwrap_or_else(|| "Helvetica".into());
    // Illustrator's reencoded fonts: "_Helvetica" is Helvetica
    let base = name.trim_start_matches('_');
    if base != name {
        let bk = i.key(base);
        let found = i.fontdir.borrow().map.get(&bk).cloned();
        if let Some(f) = found {
            i.push(f);
            return Ok(());
        }
    }
    let f = synth_font(i, base);
    i.fontdir.borrow_mut().map.insert(key, Obj::Dict(f.clone()));
    i.push(Obj::Dict(f));
    Ok(())
}

/// A font the file names but doesn't hold: its name, standard encoding, the usual matrix.
fn synth_font(i: &mut Interp, name: &str) -> DictRef {
    let f = new_dict(16);
    let n = i.lit(name);
    i.dput(&f, "FontName", n);
    i.dput(&f, "FontType", Obj::Int(1));
    i.dput(&f, "PaintType", Obj::Int(0));
    i.dput(&f, "FontMatrix", super::gfx::mat_obj(Affine::scale(0.001)));
    i.dput(&f, "FontBBox", Obj::arr(vec![Obj::Int(-200), Obj::Int(-250), Obj::Int(1200), Obj::Int(950)]));
    let enc_name = if name == "Symbol" || name.starts_with("ZapfDingbats") { "AVSymbolEncoding" } else { "StandardEncoding" };
    let enc = i.dget(&i.systemdict.clone(), enc_name).or_else(|| i.dget(&i.systemdict.clone(), "StandardEncoding")).unwrap_or(Obj::Null);
    i.dput(&f, "Encoding", enc);
    let cs = new_dict(4);
    i.dput(&cs, ".notdef", Obj::Int(0));
    i.dput(&f, "CharStrings", Obj::Dict(cs));
    i.dput(&f, "Private", Obj::Dict(new_dict(4)));
    let info = new_dict(4);
    let fam = Obj::str(name.as_bytes().to_vec());
    i.dput(&info, "FamilyName", fam);
    i.dput(&f, "FontInfo", Obj::Dict(info));
    i.next_fid += 1;
    i.dput(&f, "FID", Obj::FontId(i.next_fid));
    i.dput(&f, "AVSynthetic", Obj::Bool(true));
    f
}

pub fn definefont(i: &mut Interp) -> R {
    let f = i.pop()?;
    let k = i.pop()?;
    let Obj::Dict(d) = &f else { return Err(PsErr::TypeCheck.into()) };
    if i.dget(d, "FID").is_none() {
        i.next_fid += 1;
        i.dput(d, "FID", Obj::FontId(i.next_fid));
    }
    if i.dget(d, "FontMatrix").is_none() {
        i.dput(d, "FontMatrix", super::gfx::mat_obj(Affine::scale(0.001)));
    }
    let key = i.to_key(&k)?;
    i.fontdir.borrow_mut().map.insert(key, f.clone());
    i.push(f);
    Ok(())
}

/// The family (and bold, italic) to show a PostScript font name in.
pub fn family_for(name: &str) -> (String, bool, bool, Option<String>) {
    // a subset prefix ("ABCDEF+"), Illustrator's "_", Corel's and others' encoding suffixes
    let mut n = name.trim_start_matches('_').to_string();
    if n.len() > 7 && n.as_bytes()[6] == b'+' && n[..6].bytes().all(|b| b.is_ascii_uppercase()) {
        n = n[7..].to_string();
    }
    for suffix in ["-ISOLatin1", "-ISO", "-Identity-H", "-Identity-V", "-WinCharSetFFFF", "-Encoded", "-Reencoded"] {
        if let Some(s) = n.strip_suffix(suffix) {
            n = s.to_string();
        }
    }
    let lower = n.to_lowercase();
    let bold = ["bold", "black", "heavy", "semibold", "demi", "extrabold", "ultra"].iter().any(|w| lower.contains(w));
    let italic = ["italic", "oblique", "ital", "kursiv", "slanted"].iter().any(|w| lower.contains(w)) || lower.ends_with("-it") || lower.ends_with("-bi");
    let db = vector_core::text::fonts().lock();
    let installed = |q: &str| {
        let r = db.resolve(q);
        (r.how == vector_core::fontsub::How::Installed).then(|| {
            let fam = if r.name != r.family && !r.name.contains('-') && !r.name.contains(',') && r.name.to_lowercase().starts_with(&r.family.to_lowercase()) && !r.name.to_lowercase().contains("bold") && !r.name.to_lowercase().contains("italic") { r.name.clone() } else { r.family.clone() };
            let heavy = r.weight.is_some_and(|w| w.0 >= 600);
            (fam, heavy)
        })
    };
    if let Some((f, heavy)) = installed(&n) {
        return (f, bold || heavy, italic, None);
    }
    // the family part of the name, in words
    let base = n.split(['-', ',']).next().unwrap_or(&n).to_string();
    let base = base.strip_suffix("PSMT").or_else(|| base.strip_suffix("MT")).or_else(|| base.strip_suffix("PS")).unwrap_or(&base).to_string();
    let words = camel_words(&base);
    for q in [&base, &words] {
        if let Some((f, _)) = installed(q) {
            return (f, bold, italic, None);
        }
    }
    // the PostScript standard fonts, as Windows has them
    let narrow = lower.contains("narrow");
    let std = match base.as_str() {
        "Helvetica" | "Swiss" | "ArialMT" | "Arial" if narrow => Some("Arial Narrow"),
        "Helvetica" | "Swiss" => Some("Arial"),
        "Times" | "TimesNewRoman" | "Dutch" => Some("Times New Roman"),
        "Courier" | "CourierNew" => Some("Courier New"),
        "Symbol" => Some("Symbol"),
        "ZapfDingbats" | "Dingbats" => Some("Wingdings"),
        "AvantGarde" | "ITCAvantGarde" => Some("Century Gothic"),
        "Bookman" | "ITCBookman" => Some("Bookman Old Style"),
        "NewCenturySchlbk" | "CenturySchoolbook" => Some("Century Schoolbook"),
        "Palatino" => Some("Palatino Linotype"),
        "ZapfChancery" | "ITCZapfChancery" => Some("Monotype Corsiva"),
        "DejaVuSans" => Some("DejaVu Sans"),
        "CMR10" | "CMR12" | "CMR8" | "CMR7" | "CMR5" => Some("Times New Roman"),
        _ => None,
    };
    if let Some(s) = std {
        if let Some((f, _)) = installed(s) {
            return (f, bold, italic, None);
        }
    }
    // not installed: keep the name; the drawing shows it in a substitute
    let family = if words.is_empty() { base.clone() } else { words };
    let used = db.resolve(&family).name;
    (family.clone(), bold, italic, Some(used))
}

/// "TimesNewRoman" -> "Times New Roman".
fn camel_words(s: &str) -> String {
    let mut out = String::new();
    let cs: Vec<char> = s.chars().collect();
    for (k, c) in cs.iter().enumerate() {
        if k > 0 && c.is_uppercase() && (cs[k - 1].is_lowercase() || cs.get(k + 1).is_some_and(|n| n.is_lowercase()) && cs[k - 1].is_uppercase()) {
            out.push(' ');
        }
        out.push(*c);
    }
    out
}

thread_local! {
    static WIDTHS: std::cell::RefCell<HashMap<(String, bool, bool, char), f64>> = std::cell::RefCell::new(HashMap::new());
    static FAMILIES: std::cell::RefCell<HashMap<String, (String, bool, bool, Option<String>)>> = std::cell::RefCell::new(HashMap::new());
}

fn cached_family(name: &str) -> (String, bool, bool, Option<String>) {
    if let Some(v) = FAMILIES.with(|f| f.borrow().get(name).cloned()) {
        return v;
    }
    let v = family_for(name);
    FAMILIES.with(|f| f.borrow_mut().insert(name.to_string(), v.clone()));
    v
}

/// The advance of a character in an installed font, in ems.
fn char_width(family: &str, bold: bool, italic: bool, c: char) -> f64 {
    let key = (family.to_string(), bold, italic, c);
    if let Some(w) = WIDTHS.with(|m| m.borrow().get(&key).copied()) {
        return w;
    }
    let fmt = CharFormat { font: family.to_string(), size: 1000.0, bold, italic, ..CharFormat::default() };
    let t = TextObj::artistic(&c.to_string(), fmt);
    let l = vector_core::text::layout(&t);
    let w = l.carets.last().map(|c| c.0).filter(|w| *w > 0.0).unwrap_or_else(|| l.bounds.width().max(0.0)) / 1000.0;
    let w = if c == ' ' && w <= 0.0 { 0.25 } else { w };
    WIDTHS.with(|m| m.borrow_mut().insert(key, w));
    w
}

/// How a font shows: what the text object needs.
struct FontInfo {
    kind: i64,
    family: String,
    bold: bool,
    italic: bool,
    /// The font matrix scaled to ems (glyph space of Type 1 fonts is 1000 to the em).
    em: Affine,
    encoding: Option<PsArr>,
}

impl Interp {
    fn font_info(&mut self) -> R<(DictRef, FontInfo)> {
        let f = self.gs.font.clone().ok_or(Flow::Err(PsErr::InvalidFont))?;
        let kind = self.dnum(&f, "FontType").unwrap_or(1.0) as i64;
        let fm = match self.dget(&f, "FontMatrix") {
            Some(Obj::Array(a, _)) => mat_read(&a).unwrap_or(Affine::scale(0.001)),
            _ => Affine::scale(0.001),
        };
        let units = if kind == 42 { 1.0 } else { 1000.0 };
        let name = match self.dget(&f, "FontName") {
            Some(o) => self.text_of(&o).unwrap_or_default(),
            None => String::new(),
        };
        let name = if name.is_empty() { "Helvetica".to_string() } else { name };
        let (family, bold, italic, sub) = if kind == 3 { (String::new(), false, false, None) } else { cached_family(&name) };
        if let Some(used) = sub {
            let shown = name.trim_start_matches('_').to_string();
            self.counts.fonts.entry(shown).or_insert(used);
        }
        let encoding = match self.dget(&f, "Encoding") {
            Some(Obj::Array(a, _)) => Some(a),
            _ => None,
        };
        Ok((f, FontInfo { kind, family, bold, italic, em: fm * Affine::scale(units), encoding }))
    }

    /// Glyph names and characters of a string in the current font's encoding.
    fn decode(&self, info: &FontInfo, bytes: &[u8]) -> (Vec<char>, Vec<String>) {
        let mut chars = vec![];
        let mut names = vec![];
        for &b in bytes {
            let name = info.encoding.as_ref().and_then(|e| e.get(b as usize)).and_then(|o| self.text_of(&o)).unwrap_or_default();
            let c = super::encodings::glyph_char(&name).unwrap_or(if (32..127).contains(&b) || b >= 160 { b as char } else { ' ' });
            chars.push(c);
            names.push(name);
        }
        (chars, names)
    }

    pub fn show(&mut self, bytes: &[u8], extra: Extra) -> R {
        let (f, info) = self.font_info()?;
        if self.gs.cur.is_none() {
            return Err(PsErr::NoCurrentPoint.into());
        }
        match info.kind {
            3 => self.show_type3(&f, &info, bytes, extra),
            0 | 9 | 10 | 11 | 32 => {
                self.counts.composite_text += 1;
                Ok(())
            }
            _ => {
                let (chars, names) = self.decode(&info, bytes);
                let extra = Extra { wc: extra.wc.and_then(|c| bytes.iter().position(|b| *b == c).map(|k| if chars[k] == ' ' { b' ' } else { c })), ..extra };
                self.emit_text(&info, &chars, &names, extra)
            }
        }
    }

    pub fn show_chars(&mut self, chars: &[char], names: &[String], extra: Extra) -> R {
        let (_, info) = self.font_info()?;
        if self.gs.cur.is_none() {
            return Err(PsErr::NoCurrentPoint.into());
        }
        if info.kind == 3 || info.kind == 0 {
            return Ok(());
        }
        self.emit_text(&info, chars, names, extra)
    }

    /// Artistic text at the current point; the current point moves past it.
    fn emit_text(&mut self, info: &FontInfo, chars: &[char], _names: &[String], extra: Extra) -> R {
        let cur = self.gs.cur.unwrap();
        // em space to device space
        let lin = Affine::new({
            let c = self.gs.ctm.as_coeffs();
            [c[0], c[1], c[2], c[3], 0.0, 0.0]
        });
        let e = lin * info.em;
        let size = e.determinant().abs().sqrt();
        let width_em: f64 = chars.iter().map(|c| char_width(&info.family, info.bold, info.italic, *c)).sum();
        // extra spacing, in user space, per character and per space
        let spaces = chars.iter().filter(|c| **c == ' ').count() as f64;
        let n = chars.len() as f64;
        let user_extra = extra.a * n + if extra.wc.is_some() { extra.w * spaces } else { Vec2::ZERO };
        let adv = dvec(e, width_em, 0.0) + dvec(lin, user_extra.x, user_extra.y);
        let content: String = chars.iter().collect();
        if size > 1e-6 && !self.gs.null_device && !content.trim().is_empty() {
            let r = Affine::new({
                let c = e.as_coeffs();
                [c[0] / size, c[1] / size, c[2] / size, c[3] / size, 0.0, 0.0]
            });
            let t = self.out.to_doc * Affine::translate(cur.to_vec2()) * r * Affine::scale_non_uniform(1.0, -1.0);
            let mut fmt = CharFormat { font: info.family.clone(), size, bold: info.bold, italic: info.italic, ..CharFormat::default() };
            // the extra spacing as the text's own spacing
            let space_w = char_width(&info.family, info.bold, info.italic, ' ').max(0.05) * size;
            let along = |v: Vec2| {
                let d = dvec(lin, v.x, v.y);
                let dir = dvec(r, 1.0, 0.0);
                d.dot(dir)
            };
            if extra.a != Vec2::ZERO {
                fmt.tracking = along(extra.a) / space_w * 100.0;
            }
            let mut t_obj = TextObj::artistic(&content, fmt);
            if extra.wc.is_some() && extra.w != Vec2::ZERO {
                t_obj.word_spacing = 100.0 + along(extra.w) / space_w * 100.0;
            }
            let pc = self.gs.color.clone();
            let c = self.paint_color(&pc);
            let style = Style { fill: Fill::Solid(c), outline: Outline::none(), overprint_fill: self.gs.overprint, ..Style::default() };
            let id = self.out.doc.new_id();
            let o = Object::new(id, ObjKind::Text(t_obj), t, style);
            let clip = self.gs.clip.clone();
            self.out.add_object(o, &clip);
        }
        self.gs.cur = Some(cur + adv);
        Ok(())
    }

    fn show_type3(&mut self, f: &DictRef, info: &FontInfo, bytes: &[u8], extra: Extra) -> R {
        let build_glyph = self.dget(f, "BuildGlyph");
        let build_char = self.dget(f, "BuildChar");
        if build_glyph.is_none() && build_char.is_none() {
            return Err(PsErr::InvalidFont.into());
        }
        // glyph space is the font matrix itself for Type 3 fonts
        let fm = info.em * Affine::scale(1.0 / 1000.0);
        self.out.glyphs += 1;
        let mut r = Ok(());
        for &b in bytes {
            let cur = self.gs.cur.unwrap_or(Point::ZERO);
            let lin = Affine::new({
                let c = self.gs.ctm.as_coeffs();
                [c[0], c[1], c[2], c[3], 0.0, 0.0]
            });
            self.gsave();
            self.gs.ctm = Affine::translate(cur.to_vec2()) * lin * fm;
            self.gs.newpath();
            self.glyph_width = None;
            self.push(Obj::Dict(f.clone()));
            let res = match &build_glyph {
                Some(p) => {
                    let name = info.encoding.as_ref().and_then(|e| e.get(b as usize)).unwrap_or(Obj::Null);
                    let name = match name {
                        Obj::Name(n, _) => Obj::Name(n, false),
                        o => o,
                    };
                    self.push(name);
                    self.exec_value(p.clone().with_exec(true))
                }
                None => {
                    self.push(Obj::Int(b as i64));
                    self.exec_value(build_char.clone().unwrap().with_exec(true))
                }
            };
            self.grestore();
            if let Err(e) = res {
                match e {
                    Flow::Err(_) | Flow::Exit => {}
                    other => {
                        r = Err(other);
                        break;
                    }
                }
            }
            let (wx, wy) = self.glyph_width.take().unwrap_or((0.0, 0.0));
            let mut adv = dvec(lin * fm, wx, wy);
            adv += dvec(lin, extra.a.x, extra.a.y);
            if extra.wc == Some(b) {
                adv += dvec(lin, extra.w.x, extra.w.y);
            }
            self.gs.cur = Some(cur + adv);
        }
        self.out.glyphs -= 1;
        r
    }

    /// `stringwidth`: the advance in user space.
    pub fn string_width(&mut self, bytes: &[u8]) -> R<Vec2> {
        let (f, info) = self.font_info()?;
        match info.kind {
            3 => {
                // run the glyph procedures with nothing painted
                let saved_cur = self.gs.cur;
                let saved_path = self.gs.path.clone();
                self.gsave();
                self.gs.null_device = true;
                self.gs.cur = Some(Point::ZERO);
                let lin_inv = {
                    let c = self.gs.ctm.as_coeffs();
                    Affine::new([c[0], c[1], c[2], c[3], 0.0, 0.0])
                };
                let r = self.show_type3(&f, &info, bytes, Extra::default());
                let end = self.gs.cur.unwrap_or(Point::ZERO);
                self.grestore();
                self.gs.cur = saved_cur;
                self.gs.path = saved_path;
                r?;
                let inv = if lin_inv.determinant().abs() > 1e-300 { lin_inv.inverse() } else { Affine::IDENTITY };
                let u = inv * end;
                Ok(Vec2::new(u.x, u.y))
            }
            0 => Ok(Vec2::ZERO),
            _ => {
                let (chars, _) = self.decode(&info, bytes);
                let w: f64 = chars.iter().map(|c| char_width(&info.family, info.bold, info.italic, *c)).sum();
                Ok(dvec(info.em, w, 0.0))
            }
        }
    }

    /// `charpath`: the glyph outlines join the current path.
    pub fn charpath(&mut self, bytes: &[u8]) -> R {
        let (f, info) = self.font_info()?;
        let cur = self.gs.cur.ok_or(Flow::Err(PsErr::NoCurrentPoint))?;
        if info.kind == 3 {
            // the glyph procedures' fills, collected as a path
            let before = self.charpath_capture.replace(BezPath::new());
            let saved = self.gs.path.clone();
            self.gsave();
            let r = self.show_type3(&f, &info, bytes, Extra::default());
            let end = self.gs.cur;
            self.grestore();
            let got = std::mem::replace(&mut self.charpath_capture, before).unwrap_or_default();
            self.gs.path = saved;
            self.gs.path.extend(got.elements().iter().copied());
            self.gs.cur = end;
            return r;
        }
        if info.kind == 0 {
            return Ok(());
        }
        let (chars, _) = self.decode(&info, bytes);
        let lin = Affine::new({
            let c = self.gs.ctm.as_coeffs();
            [c[0], c[1], c[2], c[3], 0.0, 0.0]
        });
        let e = lin * info.em;
        let size = e.determinant().abs().sqrt();
        let content: String = chars.iter().collect();
        if size > 1e-6 {
            let r = Affine::new({
                let c = e.as_coeffs();
                [c[0] / size, c[1] / size, c[2] / size, c[3] / size, 0.0, 0.0]
            });
            let fmt = CharFormat { font: info.family.clone(), size, bold: info.bold, italic: info.italic, ..CharFormat::default() };
            let t = TextObj::artistic(&content, fmt);
            let l = vector_core::text::layout(&t);
            let path = Affine::translate(cur.to_vec2()) * r * Affine::scale_non_uniform(1.0, -1.0) * l.to_path();
            self.gs.path.extend(path.elements().iter().copied());
        }
        let w: f64 = chars.iter().map(|c| char_width(&info.family, info.bold, info.italic, *c)).sum();
        let adv = dvec(e, w, 0.0);
        self.gs.cur = Some(cur + adv);
        let _ = ctm_scale;
        Ok(())
    }
}
