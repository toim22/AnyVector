//! User interface language: English (United States), the language of the source, or one of the other
//! 14 languages CorelDRAW offers (see `corel-i18n/docs/02-language-inventory.md`).
//!
//! The text is translated as it is shown: egui's translator (the AnyVector patch in
//! `vendor/egui`, see `egui::set_text_translator`) passes every label, button, menu item, tooltip
//! and painted caption through [`translator`]. Text that users type is never translated.
//!
//! The words are in `assets/i18n/<code>.tsv`, one `English<TAB>translation` pair per line. pt-BR is the
//! reference: `python -I xtask/i18n_check.py . <code>` checks another language against its keys.
//! A key may hold `{}` placeholders (format strings): `Page {} of {}` matches "Page 2 of 5" and
//! the translation receives the values in order (`{}`) or by position (`{0}`, `{1}`). Menu
//! captions may put `&` before their access key. `python -I xtask/i18n.py .` lists the strings
//! that are not translated yet.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

/// The user interface languages: CorelDRAW's 15 (its setup's "Product languages").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Lang {
    EnUs,
    PtBr,
    ZhCn,
    ZhTw,
    CsCz,
    NlNl,
    FrFr,
    DeDe,
    ItIt,
    JaJp,
    PlPl,
    RuRu,
    EsEs,
    SvSe,
    TrTr,
}

impl Lang {
    /// In the order of the language list: English, then the others by their English name, as CorelDRAW lists them.
    pub const ALL: [Lang; 15] = [
        Lang::EnUs,
        Lang::ZhCn,
        Lang::ZhTw,
        Lang::CsCz,
        Lang::NlNl,
        Lang::FrFr,
        Lang::DeDe,
        Lang::ItIt,
        Lang::JaJp,
        Lang::PlPl,
        Lang::PtBr,
        Lang::RuRu,
        Lang::EsEs,
        Lang::SvSe,
        Lang::TrTr,
    ];

    /// The code kept in the settings ("en-US", "pt-BR", ...).
    pub fn code(self) -> &'static str {
        match self {
            Lang::EnUs => "en-US",
            Lang::PtBr => "pt-BR",
            Lang::ZhCn => "zh-CN",
            Lang::ZhTw => "zh-TW",
            Lang::CsCz => "cs-CZ",
            Lang::NlNl => "nl-NL",
            Lang::FrFr => "fr-FR",
            Lang::DeDe => "de-DE",
            Lang::ItIt => "it-IT",
            Lang::JaJp => "ja-JP",
            Lang::PlPl => "pl-PL",
            Lang::RuRu => "ru-RU",
            Lang::EsEs => "es-ES",
            Lang::SvSe => "sv-SE",
            Lang::TrTr => "tr-TR",
        }
    }

    /// The name in the language list, in the language itself as CorelDRAW writes it (never translated).
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::EnUs => "English (United States)",
            Lang::PtBr => "Português Brasileiro",
            Lang::ZhCn => "中文(简体)",
            Lang::ZhTw => "中文 (繁體)",
            Lang::CsCz => "Čeština",
            Lang::NlNl => "Nederlands",
            Lang::FrFr => "Français",
            Lang::DeDe => "Deutsch",
            Lang::ItIt => "Italiano",
            Lang::JaJp => "日本語",
            Lang::PlPl => "Polski",
            Lang::RuRu => "Русский",
            Lang::EsEs => "Español",
            Lang::SvSe => "Svenska",
            Lang::TrTr => "Türkçe",
        }
    }

    /// The translations (`assets/i18n/<code>.tsv`); empty for English.
    fn tsv(self) -> &'static str {
        match self {
            Lang::EnUs => "",
            Lang::PtBr => include_str!("../assets/i18n/pt-BR.tsv"),
            Lang::ZhCn => include_str!("../assets/i18n/zh-CN.tsv"),
            Lang::ZhTw => include_str!("../assets/i18n/zh-TW.tsv"),
            Lang::CsCz => include_str!("../assets/i18n/cs-CZ.tsv"),
            Lang::NlNl => include_str!("../assets/i18n/nl-NL.tsv"),
            Lang::FrFr => include_str!("../assets/i18n/fr-FR.tsv"),
            Lang::DeDe => include_str!("../assets/i18n/de-DE.tsv"),
            Lang::ItIt => include_str!("../assets/i18n/it-IT.tsv"),
            Lang::JaJp => include_str!("../assets/i18n/ja-JP.tsv"),
            Lang::PlPl => include_str!("../assets/i18n/pl-PL.tsv"),
            Lang::RuRu => include_str!("../assets/i18n/ru-RU.tsv"),
            Lang::EsEs => include_str!("../assets/i18n/es-ES.tsv"),
            Lang::SvSe => include_str!("../assets/i18n/sv-SE.tsv"),
            Lang::TrTr => include_str!("../assets/i18n/tr-TR.tsv"),
        }
    }

    /// A settings value: a code ("de-DE", or just "de"), the native name, or the names earlier versions stored.
    pub fn from_setting(s: &str) -> Option<Lang> {
        let s = s.trim().to_lowercase();
        if s.is_empty() {
            return None;
        }
        match s.as_str() {
            "en" | "english-us" | "english" => return Some(Lang::EnUs),
            "pt" | "portuguese (brazilian)" => return Some(Lang::PtBr),
            "zh" | "zh-hans" | "zh-sg" => return Some(Lang::ZhCn),
            "zh-hant" | "zh-hk" | "zh-mo" => return Some(Lang::ZhTw),
            _ => {}
        }
        Lang::ALL
            .into_iter()
            .find(|l| l.code().to_lowercase() == s || l.native_name().to_lowercase() == s)
            .or_else(|| {
                // a language code without its region ("de", "fr"), except Chinese (two scripts)
                let primary = s.split(['-', '_']).next().unwrap_or("");
                (primary != "zh").then(|| Lang::ALL.into_iter().find(|l| l.code()[..2].eq_ignore_ascii_case(primary))).flatten()
            })
    }

    /// The language of a settings value; an empty value follows Windows' display language.
    pub fn resolve(setting: &str) -> Lang {
        Lang::from_setting(setting).unwrap_or_else(system_lang)
    }

    /// The language of a Windows language id (LANGID), English when AnyVector doesn't have it.
    pub fn from_langid(id: u16) -> Lang {
        let sub = id >> 10;
        match id & 0x3ff {
            // Chinese: Taiwan, Hong Kong and Macao write traditional characters
            0x04 => {
                if matches!(sub, 0x01 | 0x03 | 0x05) {
                    Lang::ZhTw
                } else {
                    Lang::ZhCn
                }
            }
            0x05 => Lang::CsCz,
            0x07 => Lang::DeDe,
            0x0a => Lang::EsEs,
            0x0c => Lang::FrFr,
            0x10 => Lang::ItIt,
            0x11 => Lang::JaJp,
            0x13 => Lang::NlNl,
            0x15 => Lang::PlPl,
            0x16 => Lang::PtBr,
            0x19 => Lang::RuRu,
            0x1d => Lang::SvSe,
            0x1f => Lang::TrTr,
            _ => Lang::EnUs,
        }
    }

    /// The Windows font for Chinese and Japanese text in this language (file, face in the collection). Other
    /// languages use Microsoft YaHei, so the language list can show 中文 and 日本語.
    /// The faces are the collections' second, the one made for user interfaces (Yu Gothic UI, Microsoft JhengHei UI,
    /// Microsoft YaHei UI).
    fn cjk_font(self) -> (&'static str, &'static str) {
        match self {
            Lang::JaJp => ("cjk-ja", "YuGothM.ttc"),
            Lang::ZhTw => ("cjk-tw", "msjh.ttc"),
            _ => ("cjk-cn", "msyh.ttc"),
        }
    }
}

/// Windows' display language, English when AnyVector doesn't have it (never in tests).
pub fn system_lang() -> Lang {
    if cfg!(test) {
        return Lang::EnUs;
    }
    #[cfg(windows)]
    {
        let id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
        return Lang::from_langid(id);
    }
    #[allow(unreachable_code)]
    Lang::EnUs
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn lang() -> Lang {
    let v = LANG.load(Ordering::Relaxed);
    Lang::ALL.into_iter().find(|l| *l as u8 == v).unwrap_or(Lang::EnUs)
}

pub fn set_lang(l: Lang) {
    if lang() != l {
        LANG.store(l as u8, Ordering::Relaxed);
        cache().lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

/// Install the translator in egui and choose the language of the settings.
pub fn init(setting: &str) {
    egui::set_text_translator(Some(translator));
    set_lang(Lang::resolve(setting));
}

/// Follow the settings (Tools ▸ Options changes the language at once; Cancel brings it back). True when the
/// fonts must be installed again (`theme::install_fonts`): Chinese and Japanese need another font.
pub fn sync(setting: &str) -> bool {
    set_lang(Lang::resolve(setting));
    let font = lang().cjk_font().0;
    FONT.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|f| f != font)
}

/// The Chinese/Japanese font the installed fonts have.
static FONT: Mutex<Option<&'static str>> = Mutex::new(None);

/// Adds the Chinese or Japanese font of the user interface language to the end of every font family, so
/// those characters show (Segoe UI has none). Called by `theme::install_fonts`.
pub fn add_fonts(defs: &mut egui::FontDefinitions, fonts: &std::path::Path) {
    let (name, file) = lang().cjk_font();
    *FONT.lock().unwrap_or_else(|e| e.into_inner()) = Some(name);
    let Ok(bytes) = std::fs::read(fonts.join(file)) else { return };
    let mut data = egui::FontData::from_owned(bytes);
    data.index = 1;
    defs.font_data.insert(name.to_string(), std::sync::Arc::new(data));
    for list in defs.families.values_mut() {
        list.push(name.to_string());
    }
}

struct Template {
    /// The literal text around the placeholders.
    parts: Vec<String>,
    out: String,
    /// Letters in the literal parts (longer templates are tried first).
    weight: usize,
    /// Templates with fewer than three letters of their own ("{} of {}") only take numbers,
    /// or they would match almost anything.
    numbers_only: bool,
}

/// A placeholder value that is a number, like "12", "3.5", "-2", "100%".
fn is_number(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_digit()) && s.chars().all(|c| c.is_ascii_digit() || ".,-+% ".contains(c))
}

#[derive(Default)]
struct Dict {
    exact: HashMap<String, String>,
    templates: Vec<Template>,
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn parse(tsv: &str) -> Dict {
    let mut d = Dict::default();
    for line in tsv.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('\t') else { continue };
        let (k, v) = (unescape(k), unescape(v));
        if k.is_empty() || v.is_empty() {
            continue;
        }
        if k.contains("{}") {
            let parts: Vec<String> = k.split("{}").map(str::to_string).collect();
            let weight = parts.iter().map(|p| p.chars().filter(|c| c.is_alphabetic()).count()).sum();
            // a template needs some words of its own, or it would match anything
            let numbers_only = weight < 3;
            if !numbers_only || parts.iter().any(|p| !p.trim().is_empty()) {
                d.templates.push(Template { parts, out: v.clone(), weight, numbers_only });
            }
        }
        d.exact.insert(k, v);
    }
    d.templates.sort_by(|a, b| b.weight.cmp(&a.weight));
    d
}

/// The words of the user interface language.
fn dict() -> &'static Dict {
    static D: [OnceLock<Dict>; Lang::ALL.len()] = [const { OnceLock::new() }; Lang::ALL.len()];
    let l = lang();
    D[l as usize].get_or_init(|| parse(l.tsv()))
}

fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static C: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// A translation without its access-key marker.
fn plain_value(v: &str) -> String {
    if !v.contains('&') {
        return v.to_string();
    }
    let mut v = v.replace("&&", "\u{1}");
    // Chinese and Japanese put the access key in parentheses after the word ("文件(&F)"): away from a menu the
    // parentheses go too
    let chars: Vec<char> = v.chars().collect();
    if let Some(i) = (0..chars.len().saturating_sub(3)).find(|&i| chars[i] == '(' && chars[i + 1] == '&' && chars[i + 3] == ')') {
        let mut s: String = chars[..i].iter().collect::<String>().trim_end().to_string();
        s.extend(&chars[i + 4..]);
        v = s;
    }
    v.replace('&', "").replace('\u{1}', "&")
}

fn exact(d: &Dict, s: &str) -> Option<String> {
    d.exact.get(s).map(|v| plain_value(v))
}

impl Template {
    /// The values in the placeholders, if `s` has this template's shape.
    fn matches<'a>(&self, s: &'a str) -> Option<Vec<&'a str>> {
        let first = &self.parts[0];
        let last = self.parts.last().unwrap();
        if !s.starts_with(first.as_str()) || !s.ends_with(last.as_str()) || s.len() < first.len() + last.len() {
            return None;
        }
        let body = &s[first.len()..s.len() - last.len()];
        let mut args = Vec::with_capacity(self.parts.len() - 1);
        let mut rest = body;
        for p in &self.parts[1..self.parts.len() - 1] {
            let i = rest.find(p.as_str())?;
            if i == 0 {
                return None;
            }
            args.push(&rest[..i]);
            rest = &rest[i + p.len()..];
        }
        if rest.is_empty() {
            return None;
        }
        args.push(rest);
        if self.numbers_only && !args.iter().all(|a| is_number(a)) {
            return None;
        }
        Some(args)
    }

    fn fill(&self, d: &Dict, args: &[&str]) -> String {
        let args: Vec<String> = args.iter().map(|a| exact(d, a).unwrap_or_else(|| a.to_string())).collect();
        let mut out = String::with_capacity(self.out.len() + 16);
        let mut next = 0;
        let mut it = self.out.char_indices().peekable();
        while let Some((i, c)) = it.next() {
            if c == '{' {
                let tail = &self.out[i + 1..];
                if let Some(end) = tail.find('}') {
                    let inner = &tail[..end];
                    let idx = if inner.is_empty() {
                        next += 1;
                        Some(next - 1)
                    } else {
                        inner.parse::<usize>().ok()
                    };
                    if let Some(a) = idx.and_then(|k| args.get(k)) {
                        out.push_str(a);
                        for _ in 0..=end {
                            it.next();
                        }
                        continue;
                    }
                }
            }
            out.push(c);
        }
        plain_value(&out)
    }
}

const SUFFIXES: [&str; 6] = ["...", "…", ":", " *", ".", "?"];

fn lookup(d: &Dict, s: &str) -> Option<String> {
    if let Some(v) = exact(d, s) {
        return Some(v);
    }
    // surrounding spaces and line breaks
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if t.len() != s.len() {
        let lead = &s[..s.len() - s.trim_start().len()];
        let trail = &s[s.trim_end().len()..];
        return lookup(d, t).map(|v| format!("{lead}{v}{trail}"));
    }
    // "Name:" / "Open..." when only "Name" / "Open" is in the dictionary, and the other way round
    for suf in SUFFIXES {
        if let Some(base) = s.strip_suffix(suf) {
            if let Some(v) = exact(d, base) {
                return Some(format!("{v}{suf}"));
            }
        }
        if let Some(v) = d.exact.get(&format!("{s}{suf}")).map(|v| plain_value(v)) {
            if let Some(v) = v.strip_suffix(suf) {
                return Some(v.to_string());
            }
        }
    }
    // a symbol in front: "✦ Generate", "• Item"
    let mut chars = s.chars();
    if let (Some(c), Some(' ')) = (chars.next(), chars.next()) {
        if !c.is_alphanumeric() {
            let rest = &s[c.len_utf8() + 1..];
            if let Some(v) = lookup(d, rest) {
                return Some(format!("{c} {v}"));
            }
        }
    }
    // several lines: each line on its own
    if s.contains('\n') {
        let mut changed = false;
        let lines: Vec<String> = s
            .split('\n')
            .map(|l| match lookup(d, l) {
                Some(v) => {
                    changed = true;
                    v
                }
                None => l.to_string(),
            })
            .collect();
        return changed.then(|| lines.join("\n"));
    }
    for t in &d.templates {
        if let Some(args) = t.matches(s) {
            return Some(t.fill(d, &args));
        }
    }
    None
}

/// egui's translator: the text in the user interface language, or `None` to keep it.
pub fn translator(text: &str) -> Option<String> {
    if lang() == Lang::EnUs || text.len() < 2 || !text.bytes().any(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let mut c = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(r) = c.get(text) {
        return r.clone();
    }
    let r = lookup(dict(), text);
    if c.len() > 50_000 {
        c.clear();
    }
    c.insert(text.to_string(), r.clone());
    r
}

/// The text in the user interface language.
pub fn t(text: &str) -> String {
    translator(text).unwrap_or_else(|| text.to_string())
}

/// A menu caption with its `&` access key, in the user interface language. A translation
/// without `&` gets one on the original key's letter (or its first letter).
pub fn caption(text: &str) -> String {
    if lang() == Lang::EnUs {
        return text.to_string();
    }
    let plain = text.replace("&&", "\u{1}").replace('&', "").replace('\u{1}', "&");
    if let Some(v) = dict().exact.get(&plain) {
        if v.contains('&') {
            return v.clone();
        }
        return with_access_key(v, access_key(text));
    }
    match translator(&plain) {
        Some(v) => with_access_key(&v.replace('&', "&&"), access_key(text)),
        None => text.to_string(),
    }
}

/// The letter after a single `&`.
fn access_key(text: &str) -> Option<char> {
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        if c == '&' {
            match it.next() {
                Some('&') => continue,
                other => return other,
            }
        }
    }
    None
}

fn with_access_key(v: &str, key: Option<char>) -> String {
    let Some(key) = key else { return v.to_string() };
    let lower = key.to_lowercase().next().unwrap_or(key);
    let pos = v.char_indices().find(|(_, c)| c.to_lowercase().next() == Some(lower)).or_else(|| v.char_indices().find(|(_, c)| c.is_alphabetic()));
    match pos {
        Some((i, _)) => format!("{}&{}", &v[..i], &v[i..]),
        None => v.to_string(),
    }
}

/// The language chosen in the setup (Software\AnyVector: Language, LanguageStamp), for the user
/// or for all users.
pub fn installer_choice() -> Option<(Lang, String)> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            let lang = reg_string(root, r"Software\AnyVector", "Language");
            let stamp = reg_string(root, r"Software\AnyVector", "LanguageStamp");
            if let (Some(l), Some(s)) = (lang.as_deref().and_then(Lang::from_setting), stamp) {
                return Some((l, s));
            }
        }
    }
    None
}

#[cfg(windows)]
fn reg_string(root: windows::Win32::System::Registry::HKEY, sub: &str, name: &str) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{RegGetValueW, RRF_RT_REG_SZ};
    let (sub, value) = (HSTRING::from(sub), HSTRING::from(name));
    let mut size = 0u32;
    unsafe { RegGetValueW(root, &sub, &value, RRF_RT_REG_SZ, None, None, Some(&mut size)) }.ok().ok()?;
    let mut buf = vec![0u16; size as usize / 2 + 1];
    let mut size = (buf.len() * 2) as u32;
    unsafe { RegGetValueW(root, &sub, &value, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut size)) }.ok().ok()?;
    let n = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..n]).trim().to_string()).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d() -> Dict {
        parse("# comment\nFile\t&Arquivo\nOpen\tAbrir\nName\tNome\nPage {} of {}\tPágina {} de {}\n{} objects selected\t{} objetos selecionados\nMove {} to {}\tMover para {1}: {0}\nLayer\tCamada\nLine one\\nLine two\tLinha um\\nLinha dois\nGenerate\tGerar\n{} of {}\t{} de {}\n")
    }

    #[test]
    fn exact_suffix_and_spaces() {
        let d = d();
        assert_eq!(lookup(&d, "File").as_deref(), Some("Arquivo"));
        assert_eq!(lookup(&d, "Open...").as_deref(), Some("Abrir..."));
        assert_eq!(lookup(&d, "Name:").as_deref(), Some("Nome:"));
        assert_eq!(lookup(&d, "  Name ").as_deref(), Some("  Nome "));
        assert_eq!(lookup(&d, "✦ Generate").as_deref(), Some("✦ Gerar"));
        assert_eq!(lookup(&d, "Line one\nLine two").as_deref(), Some("Linha um\nLinha dois"));
        assert_eq!(lookup(&d, "Open\nUnknown").as_deref(), Some("Abrir\nUnknown"));
        assert_eq!(lookup(&d, "Unknown"), None);
    }

    #[test]
    fn parenthesized_access_keys_leave_plain_text() {
        assert_eq!(plain_value("文件(&F)"), "文件");
        assert_eq!(plain_value("打开(&O)..."), "打开...");
        assert_eq!(plain_value("&Arquivo"), "Arquivo");
        assert_eq!(plain_value("Preto && branco"), "Preto & branco");
    }

    #[test]
    fn templates_fill_in_order_and_by_position() {
        let d = d();
        assert_eq!(lookup(&d, "Page 2 of 5").as_deref(), Some("Página 2 de 5"));
        assert_eq!(lookup(&d, "12 objects selected").as_deref(), Some("12 objetos selecionados"));
        assert_eq!(lookup(&d, "Move Star to Layer").as_deref(), Some("Mover para Camada: Star"));
        assert_eq!(lookup(&d, "Page  of 5"), None);
        // a template with almost no words takes numbers only
        assert_eq!(lookup(&d, "1 of 12").as_deref(), Some("1 de 12"));
        assert_eq!(lookup(&d, "Set of tools"), None);
    }

    #[test]
    fn access_keys() {
        assert_eq!(with_access_key("Exibir", Some('V')), "&Exibir");
        assert_eq!(with_access_key("Editar", Some('E')), "&Editar");
        assert_eq!(with_access_key("Objeto", Some('j')), "Ob&jeto");
        assert_eq!(access_key("Lay&out"), Some('o'));
        assert_eq!(access_key("Black && White"), None);
    }

    #[test]
    fn settings_values() {
        assert_eq!(Lang::from_setting("English-US"), Some(Lang::EnUs));
        assert_eq!(Lang::from_setting("pt-BR"), Some(Lang::PtBr));
        assert_eq!(Lang::from_setting("Português Brasileiro"), Some(Lang::PtBr));
        assert_eq!(Lang::from_setting("de-DE"), Some(Lang::DeDe));
        assert_eq!(Lang::from_setting("fr"), Some(Lang::FrFr));
        assert_eq!(Lang::from_setting("日本語"), Some(Lang::JaJp));
        assert_eq!(Lang::from_setting("zh-TW"), Some(Lang::ZhTw));
        assert_eq!(Lang::from_setting("zh"), Some(Lang::ZhCn));
        assert_eq!(Lang::from_setting("ko-KR"), None);
        assert_eq!(Lang::from_setting(""), None);
        assert_eq!(Lang::resolve(""), Lang::EnUs);
        for l in Lang::ALL {
            assert_eq!(Lang::from_setting(l.code()), Some(l));
            assert_eq!(Lang::from_setting(l.native_name()), Some(l));
        }
    }

    #[test]
    fn windows_language_ids() {
        assert_eq!(Lang::from_langid(0x0416), Lang::PtBr);
        assert_eq!(Lang::from_langid(0x0816), Lang::PtBr);
        assert_eq!(Lang::from_langid(0x0407), Lang::DeDe);
        assert_eq!(Lang::from_langid(0x0c0a), Lang::EsEs);
        assert_eq!(Lang::from_langid(0x0804), Lang::ZhCn);
        assert_eq!(Lang::from_langid(0x0404), Lang::ZhTw);
        assert_eq!(Lang::from_langid(0x0c04), Lang::ZhTw);
        assert_eq!(Lang::from_langid(0x0411), Lang::JaJp);
        assert_eq!(Lang::from_langid(0x0409), Lang::EnUs);
        assert_eq!(Lang::from_langid(0x0412), Lang::EnUs);
    }

    #[test]
    fn every_language_is_listed_once() {
        for (i, l) in Lang::ALL.into_iter().enumerate() {
            assert!(!Lang::ALL[..i].contains(&l));
        }
        assert_eq!(Lang::ALL[0], Lang::EnUs);
    }

    /// Every language translates every pt-BR key, with the same placeholders and line breaks.
    #[test]
    fn dictionaries_parse() {
        let reference = parse(Lang::PtBr.tsv());
        assert!(reference.exact.len() > 5000);
        for l in Lang::ALL.into_iter().filter(|l| *l != Lang::EnUs) {
            let d = parse(l.tsv());
            for k in reference.exact.keys() {
                assert!(d.exact.contains_key(k), "{}: {k:?} is not translated", l.code());
            }
            for (k, v) in &d.exact {
                assert!(!k.is_empty() && !v.is_empty());
                let ph = |s: &str| s.matches("{}").count() + (0..10).filter(|i| s.contains(&format!("{{{i}}}"))).count();
                assert_eq!(ph(k), ph(v), "{}: placeholders differ: {k} -> {v}", l.code());
                assert_eq!(k.matches('\n').count(), v.matches('\n').count(), "{}: line breaks differ: {k} -> {v}", l.code());
            }
        }
    }
}
