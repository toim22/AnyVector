//! Writing tools backed by Windows: the spell checker (Windows Spell Checking API) and the system code
//! pages used by Text ▸ Encode.

/// A misspelled or repeated word: char range in the checked text, and what Windows suggests doing.
#[derive(Clone, Debug, PartialEq)]
pub struct SpellError {
    pub start: usize,
    pub len: usize,
    /// The word is repeated ("the the"): delete it.
    pub delete: bool,
    /// Windows knows the replacement (an auto-correct pair).
    pub replacement: Option<String>,
}

/// The default spell checking language.
pub const DEFAULT_LANG: &str = "en-US";

#[cfg(windows)]
mod imp {
    use super::SpellError;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use windows::core::{HSTRING, PCWSTR, PWSTR};
    use windows::Win32::Globalization::*;
    use windows::Win32::System::Com::*;

    thread_local! {
        static CHECKERS: RefCell<HashMap<String, Option<ISpellChecker>>> = RefCell::new(HashMap::new());
    }

    fn com() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
    }

    fn checker(lang: &str) -> Option<ISpellChecker> {
        let lang = if lang.is_empty() { super::DEFAULT_LANG } else { lang };
        CHECKERS.with(|c| {
            c.borrow_mut()
                .entry(lang.to_string())
                .or_insert_with(|| unsafe {
                    com();
                    let f: ISpellCheckerFactory = CoCreateInstance(&SpellCheckerFactory, None, CLSCTX_INPROC_SERVER).ok()?;
                    let tag = HSTRING::from(lang);
                    if !f.IsSupported(&tag).ok()?.as_bool() {
                        return None;
                    }
                    f.CreateSpellChecker(&tag).ok()
                })
                .clone()
        })
    }

    fn take(p: PWSTR) -> String {
        if p.is_null() {
            return String::new();
        }
        unsafe {
            let s = p.to_string().unwrap_or_default();
            CoTaskMemFree(Some(p.0 as *const _));
            s
        }
    }

    pub fn supported(lang: &str) -> bool {
        checker(lang).is_some()
    }

    pub fn check(text: &str, lang: &str) -> Vec<SpellError> {
        let Some(c) = checker(lang) else { return vec![] };
        let wide: Vec<u16> = text.encode_utf16().collect();
        // utf-16 index -> char index
        let mut to_char = Vec::with_capacity(wide.len() + 1);
        for (ci, ch) in text.chars().enumerate() {
            for _ in 0..ch.len_utf16() {
                to_char.push(ci);
            }
        }
        to_char.push(text.chars().count());
        let mut out = vec![];
        unsafe {
            let h = HSTRING::from(text);
            let Ok(errors) = c.Check(&h) else { return out };
            loop {
                let mut e: Option<ISpellingError> = None;
                let hr = errors.Next(&mut e);
                if hr.is_err() || hr.0 == 1 {
                    break;
                }
                let Some(e) = e else { break };
                let (Ok(s), Ok(l)) = (e.StartIndex(), e.Length()) else { continue };
                let (s, l) = (s as usize, l as usize);
                let a = to_char.get(s).copied().unwrap_or(0);
                let b = to_char.get(s + l).copied().unwrap_or(a);
                let action = e.CorrectiveAction().unwrap_or(CORRECTIVE_ACTION_NONE);
                let replacement = if action == CORRECTIVE_ACTION_REPLACE { e.Replacement().ok().map(take) } else { None };
                out.push(SpellError { start: a, len: b - a, delete: action == CORRECTIVE_ACTION_DELETE, replacement });
            }
        }
        out
    }

    pub fn suggest(word: &str, lang: &str) -> Vec<String> {
        let Some(c) = checker(lang) else { return vec![] };
        let mut out = vec![];
        unsafe {
            let Ok(list) = c.Suggest(&HSTRING::from(word)) else { return out };
            loop {
                let mut item = [PWSTR::null()];
                let mut got = 0u32;
                let hr = list.Next(&mut item, Some(&mut got));
                if hr.is_err() || got == 0 {
                    break;
                }
                out.push(take(item[0]));
            }
        }
        out
    }

    pub fn add(word: &str, lang: &str) {
        if let Some(c) = checker(lang) {
            unsafe {
                let _ = c.Add(&HSTRING::from(word));
            }
        }
    }

    pub fn ignore(word: &str, lang: &str) {
        if let Some(c) = checker(lang) {
            unsafe {
                let _ = c.Ignore(&HSTRING::from(word));
            }
        }
    }

    pub fn auto_correct(from: &str, to: &str, lang: &str) {
        if let Some(c) = checker(lang) {
            unsafe {
                let _ = c.AutoCorrect(&HSTRING::from(from), &HSTRING::from(to));
            }
        }
    }

    thread_local! {
        static PAGES: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }

    unsafe extern "system" fn enum_page(s: PCWSTR) -> windows::core::BOOL {
        let n = unsafe { s.to_string() }.ok().and_then(|s| s.trim().parse::<u32>().ok());
        if let Some(n) = n {
            PAGES.with(|p| p.borrow_mut().push(n));
        }
        true.into()
    }

    /// Installed code pages as (number, name) — the name as Windows gives it, e.g. "1252  (ANSI - Latin I)".
    pub fn code_pages() -> Vec<(u32, String)> {
        PAGES.with(|p| p.borrow_mut().clear());
        unsafe {
            let _ = EnumSystemCodePagesW(Some(enum_page), CP_INSTALLED);
        }
        let pages = PAGES.with(|p| p.borrow().clone());
        let mut out: Vec<(u32, String)> = pages
            .into_iter()
            .filter_map(|cp| {
                let mut info = CPINFOEXW::default();
                unsafe { GetCPInfoExW(cp, 0, &mut info) }.ok()?;
                let end = info.CodePageName.iter().position(|c| *c == 0).unwrap_or(info.CodePageName.len());
                let name = String::from_utf16_lossy(&info.CodePageName[..end]);
                Some((cp, if name.is_empty() { cp.to_string() } else { name }))
            })
            .filter(|(cp, _)| !matches!(cp, 1200 | 1201 | 12000 | 12001 | 65000 | 65001))
            .collect();
        out.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));
        out.dedup_by_key(|p| p.0);
        out
    }

    /// The bytes of `text` in a code page (characters it lacks become "?").
    pub fn encode(text: &str, cp: u32) -> Vec<u8> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        if wide.is_empty() {
            return vec![];
        }
        unsafe {
            let n = WideCharToMultiByte(cp, 0, &wide, None, windows::core::PCSTR::null(), None);
            if n <= 0 {
                return text.bytes().collect();
            }
            let mut buf = vec![0u8; n as usize];
            WideCharToMultiByte(cp, 0, &wide, Some(&mut buf), windows::core::PCSTR::null(), None);
            buf
        }
    }

    /// Bytes read in a code page.
    pub fn decode(bytes: &[u8], cp: u32) -> String {
        if bytes.is_empty() {
            return String::new();
        }
        unsafe {
            let n = MultiByteToWideChar(cp, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None);
            if n <= 0 {
                return String::from_utf8_lossy(bytes).to_string();
            }
            let mut buf = vec![0u16; n as usize];
            MultiByteToWideChar(cp, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, Some(&mut buf));
            String::from_utf16_lossy(&buf)
        }
    }

    /// The code page Windows uses for text that is not Unicode.
    pub fn ansi_code_page() -> u32 {
        unsafe { GetACP() }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::SpellError;
    pub fn supported(_lang: &str) -> bool {
        false
    }
    pub fn check(_text: &str, _lang: &str) -> Vec<SpellError> {
        vec![]
    }
    pub fn suggest(_word: &str, _lang: &str) -> Vec<String> {
        vec![]
    }
    pub fn add(_word: &str, _lang: &str) {}
    pub fn ignore(_word: &str, _lang: &str) {}
    pub fn auto_correct(_from: &str, _to: &str, _lang: &str) {}
    pub fn code_pages() -> Vec<(u32, String)> {
        vec![(1252, "1252  (ANSI - Latin I)".into())]
    }
    pub fn encode(text: &str, _cp: u32) -> Vec<u8> {
        text.bytes().collect()
    }
    pub fn decode(bytes: &[u8], _cp: u32) -> String {
        String::from_utf8_lossy(bytes).to_string()
    }
    pub fn ansi_code_page() -> u32 {
        1252
    }
}

pub use imp::{add, ansi_code_page, auto_correct, check, code_pages, decode, encode, ignore, suggest, supported};

/// Text ▸ Writing Tools ▸ Language: the languages text can be tagged with, as the reference lists them
/// (name, code shown, BCP 47 tag).
pub const LANGUAGES: &[(&str, &str, &str)] = &[
    ("Afrikaans", "AFK", "af-ZA"),
    ("Albanian", "SQI", "sq-AL"),
    ("Arabic (Algeria)", "ARG", "ar-DZ"),
    ("Arabic (Bahrain)", "ARH", "ar-BH"),
    ("Arabic (Egypt)", "ARE", "ar-EG"),
    ("Arabic (Iraq)", "ARI", "ar-IQ"),
    ("Arabic (Jordan)", "ARJ", "ar-JO"),
    ("Arabic (Kuwait)", "ARK", "ar-KW"),
    ("Arabic (Lebanon)", "ARB", "ar-LB"),
    ("Arabic (Libya)", "ARL", "ar-LY"),
    ("Arabic (Morocco)", "ARM", "ar-MA"),
    ("Arabic (Oman)", "ARO", "ar-OM"),
    ("Arabic (Qatar)", "ARQ", "ar-QA"),
    ("Arabic (Saudi Arabia)", "ARA", "ar-SA"),
    ("Arabic (Syria)", "ARS", "ar-SY"),
    ("Arabic (Tunisia)", "ART", "ar-TN"),
    ("Arabic (United Arab Emirates)", "ARA", "ar-AE"),
    ("Arabic (Yemen)", "ARY", "ar-YE"),
    ("Azeri (Cyrillic)", "AZC", "az-Cyrl-AZ"),
    ("Azeri (Latin)", "AZL", "az-Latn-AZ"),
    ("Basque", "EUQ", "eu-ES"),
    ("Belarusian", "BEL", "be-BY"),
    ("Bulgarian", "BGR", "bg-BG"),
    ("Catalan", "CAT", "ca-ES"),
    ("Chinese (China)", "CHC", "zh-CN"),
    ("Chinese (Hong Kong SAR)", "ZHH", "zh-HK"),
    ("Chinese (Macau SAR)", "ZHM", "zh-MO"),
    ("Chinese (Simplified)", "CHS", "zh-Hans"),
    ("Chinese (Singapore)", "ZHI", "zh-SG"),
    ("Chinese (Taiwan)", "CHW", "zh-TW"),
    ("Chinese (Traditional)", "CHT", "zh-Hant"),
    ("Croatian", "HRV", "hr-HR"),
    ("Czech", "CSY", "cs-CZ"),
    ("Danish", "DAN", "da-DK"),
    ("Dutch (Belgium)", "NLB", "nl-BE"),
    ("Dutch (The Netherlands)", "NLD", "nl-NL"),
    ("English (Australia)", "ENA", "en-AU"),
    ("English (Belize)", "ENB", "en-BZ"),
    ("English (Canada)", "ENC", "en-CA"),
    ("English (Caribbean)", "ENN", "en-029"),
    ("English (Ireland)", "ENI", "en-IE"),
    ("English (Jamaica)", "ENJ", "en-JM"),
    ("English (New Zealand)", "ENZ", "en-NZ"),
    ("English (Philippines)", "ENP", "en-PH"),
    ("English (South Africa)", "ENS", "en-ZA"),
    ("English (Trinidad and Tobago)", "ENT", "en-TT"),
    ("English (United Kingdom)", "ENG", "en-GB"),
    ("English (United States)", "ENU", "en-US"),
    ("English (Zimbabwe)", "ENW", "en-ZW"),
    ("Estonian", "ETI", "et-EE"),
    ("Faroese", "FOS", "fo-FO"),
    ("Farsi", "FAI", "fa-IR"),
    ("Finnish", "FIN", "fi-FI"),
    ("French (Belgium)", "FRB", "fr-BE"),
    ("French (Canada)", "FRC", "fr-CA"),
    ("French (France)", "FRA", "fr-FR"),
    ("French (Luxembourg)", "FRL", "fr-LU"),
    ("French (Monaco)", "FRM", "fr-MC"),
    ("French (Switzerland)", "FRS", "fr-CH"),
    ("Galician", "GLC", "gl-ES"),
    ("German (Austria)", "DEA", "de-AT"),
    ("German (Germany)", "DEU", "de-DE"),
    ("German (Liechtenstein)", "DEI", "de-LI"),
    ("German (Luxembourg)", "DEL", "de-LU"),
    ("German (Switzerland)", "DES", "de-CH"),
    ("Greek", "ELL", "el-GR"),
    ("Hebrew", "HEB", "he-IL"),
    ("Hungarian", "HUN", "hu-HU"),
    ("Icelandic", "ISL", "is-IS"),
    ("Indonesian", "IND", "id-ID"),
    ("Italian", "ITA", "it-IT"),
    ("Italian (Switzerland)", "ITS", "it-CH"),
    ("Japanese", "JPN", "ja-JP"),
    ("Kazakh", "KKZ", "kk-KZ"),
    ("Korean", "KOR", "ko-KR"),
    ("Kyrgyz", "KYR", "ky-KG"),
    ("Latvian", "LVI", "lv-LV"),
    ("Lithuanian", "LTH", "lt-LT"),
    ("Macedonian", "MKI", "mk-MK"),
    ("Malay (Brunei)", "MSB", "ms-BN"),
    ("Malay (Malaysia)", "MSL", "ms-MY"),
    ("Mongolian", "MON", "mn-MN"),
    ("Norwegian (Bokmel)", "NOR", "nb-NO"),
    ("Norwegian (Nynorsk)", "NON", "nn-NO"),
    ("Polish", "PLK", "pl-PL"),
    ("Portuguese (Brazil)", "PTB", "pt-BR"),
    ("Portuguese (Portugal)", "PTG", "pt-PT"),
    ("Romanian (Romania)", "RON", "ro-RO"),
    ("Russian (Russia)", "RUS", "ru-RU"),
    ("Serbian (Cyrillic)", "SRB", "sr-Cyrl-RS"),
    ("Serbian (Latin)", "SRL", "sr-Latn-RS"),
    ("Slovak", "SKY", "sk-SK"),
    ("Slovenian", "SLV", "sl-SI"),
    ("Spanish (Argentina)", "ESS", "es-AR"),
    ("Spanish (Bolivia)", "ESB", "es-BO"),
    ("Spanish (Chile)", "ESL", "es-CL"),
    ("Spanish (Colombia)", "ESO", "es-CO"),
    ("Spanish (Costa Rica)", "ESC", "es-CR"),
    ("Spanish (Dominican Republic)", "ESD", "es-DO"),
    ("Spanish (Ecuador)", "ESF", "es-EC"),
    ("Spanish (El Salvador)", "ESE", "es-SV"),
    ("Spanish (Guatemala)", "ESG", "es-GT"),
    ("Spanish (Honduras)", "ESH", "es-HN"),
    ("Spanish (Mexico)", "ESM", "es-MX"),
    ("Spanish (Nicaragua)", "ESI", "es-NI"),
    ("Spanish (Panama)", "ESA", "es-PA"),
    ("Spanish (Paraguay)", "ESZ", "es-PY"),
    ("Spanish (Peru)", "ESR", "es-PE"),
    ("Spanish (Puerto Rico)", "ESU", "es-PR"),
    ("Spanish (Spain)", "ESN", "es-ES"),
    ("Spanish (Uruguay)", "ESY", "es-UY"),
    ("Spanish (Venezuela)", "ESV", "es-VE"),
    ("Swahili", "SWK", "sw-KE"),
    ("Swedish (Finland)", "SVF", "sv-FI"),
    ("Swedish (Sweden)", "SVE", "sv-SE"),
    ("Tatar", "TTT", "tt-RU"),
    ("Thai", "THA", "th-TH"),
    ("Turkish", "TRK", "tr-TR"),
    ("Ukrainian", "UKR", "uk-UA"),
    ("Urdu", "URD", "ur-PK"),
    ("Uzbek (Cyrillic)", "UZC", "uz-Cyrl-UZ"),
    ("Uzbek (Latin)", "UZL", "uz-Latn-UZ"),
    ("Vietnamese", "VIT", "vi-VN"),
];

/// The language of a text format ("" = English (United States)).
pub fn lang_of(tag: &str) -> &'static str {
    let tag = if tag.is_empty() { DEFAULT_LANG } else { tag };
    LANGUAGES.iter().find(|l| l.2.eq_ignore_ascii_case(tag)).map(|l| l.2).unwrap_or(DEFAULT_LANG)
}

/// Grammatik: rules checked beside spelling. Returns (start, len, rule, explanation, replacements).
pub fn grammar(text: &str, lang: &str) -> Vec<(usize, usize, &'static str, String, Vec<String>)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<(usize, usize, &'static str, String, Vec<String>)> = vec![];
    // words with their char ranges
    let mut words: Vec<(usize, usize)> = vec![];
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_alphanumeric() {
            let s = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '\'' || chars[i] == '’') {
                i += 1;
            }
            words.push((s, i));
        } else {
            i += 1;
        }
    }
    let word = |w: (usize, usize)| chars[w.0..w.1].iter().collect::<String>();
    let spelling = check(text, lang);
    for e in &spelling {
        let w: String = chars[e.start..e.start + e.len].iter().collect();
        if e.delete {
            out.push((e.start, e.len, "Duplicate word", format!("`{w}` is repeated."), vec![String::new()]));
        } else {
            let mut sug = suggest(&w, lang);
            if let Some(r) = &e.replacement {
                sug.insert(0, r.clone());
            }
            out.push((e.start, e.len, "Spelling", format!("`{w}` is not in the spelling dictionary."), sug));
        }
    }
    let english = lang.is_empty() || lang.starts_with("en");
    for (k, &w) in words.iter().enumerate() {
        let s = word(w);
        // the first word of a sentence starts with a capital
        let before: String = chars[..w.0].iter().rev().take_while(|c| !c.is_alphanumeric()).collect();
        let sentence_start = (k == 0 && chars[..w.0].iter().all(|c| !c.is_alphanumeric())) || before.contains(['.', '!', '?']) || before.contains('\n');
        if sentence_start && s.chars().next().is_some_and(|c| c.is_lowercase()) && !out.iter().any(|o| o.0 == w.0) {
            let fixed = {
                let mut c = s.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            };
            out.push((w.0, w.1 - w.0, "Capitalization", "The first word of a sentence starts with a capital letter.".into(), vec![fixed]));
        }
        // a / an
        if english && (s == "a" || s == "A" || s == "an" || s == "An") {
            if let Some(&nw) = words.get(k + 1) {
                let next = word(nw).to_lowercase();
                let vowel = next.starts_with(['a', 'e', 'i', 'o', 'u']) && !next.starts_with("eu") && !next.starts_with("uni") && !next.starts_with("one");
                let want = if vowel { "an" } else { "a" };
                if !s.eq_ignore_ascii_case(want) {
                    let rep = if s.starts_with('A') { format!("{}{}", want[..1].to_uppercase(), &want[1..]) } else { want.to_string() };
                    out.push((w.0, w.1 - w.0, "Article", format!("Use `{want}` before `{next}`."), vec![rep]));
                }
            }
        }
    }
    // two spaces between words
    for (i, w) in chars.windows(2).enumerate() {
        if w[0] == ' ' && w[1] == ' ' && i > 0 && chars[i - 1] != ' ' {
            out.push((i, 2, "Spacing", "There are two spaces between words.".into(), vec![" ".into()]));
        }
    }
    // a space before punctuation
    for (i, w) in chars.windows(2).enumerate() {
        if w[0] == ' ' && matches!(w[1], ',' | '.' | ';' | ':' | '!' | '?') && i > 0 && chars[i - 1].is_alphanumeric() {
            out.push((i, 2, "Punctuation", "There is a space before the punctuation mark.".into(), vec![w[1].to_string()]));
        }
    }
    out.sort_by_key(|o| o.0);
    out
}

/// Thesaurus: the synonyms of a word (WordNet).
pub fn synonyms(word: &str) -> Vec<String> {
    use std::sync::OnceLock;
    static DICT: OnceLock<String> = OnceLock::new();
    let dict = DICT.get_or_init(thesaurus_wordnet::uncompress);
    let w = word.trim().to_lowercase();
    if w.is_empty() {
        return vec![];
    }
    let key = format!("{w}|");
    let mut out: Vec<String> = vec![];
    for line in dict.lines() {
        if let Some(rest) = line.strip_prefix(&key) {
            for s in rest.split('|') {
                if !s.is_empty() && s != w && !out.iter().any(|o| o == s) {
                    out.push(s.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_tools() {
        assert_eq!(synonyms("happy").is_empty(), false);
        assert!(synonyms("happy").iter().any(|s| s == "glad"));
        let g = grammar("this is is a apple", "en-US");
        assert!(g.iter().any(|e| e.2 == "Capitalization"));
        assert!(g.iter().any(|e| e.2 == "Article"));
        #[cfg(windows)]
        {
            let pages = code_pages();
            assert!(pages.iter().any(|p| p.0 == 1252), "{pages:?}");
            let bytes = encode("Olá", 1252);
            assert_eq!(decode(&bytes, 1252), "Olá");
            if supported("en-US") {
                let e = check("Texto artistico", "en-US");
                assert!(!e.is_empty());
                assert!(!suggest("Texto", "en-US").is_empty());
                assert!(check("is is", "en-US").iter().any(|e| e.delete));
            }
        }
    }
}
