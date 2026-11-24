//! Code pages without Windows (Text ▸ Encode on Linux): the pages Windows lists that `encoding_rs`
//! can read and write, under the names Windows gives them, and the ANSI code page of a locale.
//! Built on every system so the Windows tests can compare it with Windows' own conversions.

#![cfg_attr(windows, allow(dead_code))]

use encoding_rs::Encoding;

/// The code pages Text ▸ Encode lists, with the names `GetCPInfoEx` gives them on an English
/// Windows 11. Windows' other installed pages (OEM pages but 866, EBCDIC, ISCII, the other Mac
/// pages...) have no converter here, so they are left out.
const PAGES: &[(u32, &str)] = &[
    (866, "OEM - Russian"),
    (874, "ANSI/OEM - Thai"),
    (932, "ANSI/OEM - Japanese Shift-JIS"),
    (936, "ANSI/OEM - Simplified Chinese GBK"),
    (949, "ANSI/OEM - Korean"),
    (950, "ANSI/OEM - Traditional Chinese Big5"),
    (1250, "ANSI - Central Europe"),
    (1251, "ANSI - Cyrillic"),
    (1252, "ANSI - Latin I"),
    (1253, "ANSI - Greek"),
    (1254, "ANSI - Turkish"),
    (1255, "ANSI - Hebrew"),
    (1256, "ANSI - Arabic"),
    (1257, "ANSI - Baltic"),
    (1258, "ANSI/OEM - Viet Nam"),
    (10000, "MAC - Roman"),
    (10017, "MAC - Ukraine"),
    (20127, "US-ASCII"),
    (20866, "Russian - KOI8"),
    (20932, "JIS X 0208-1990 & 0212-1990"),
    (21866, "Ukrainian - KOI8-U"),
    (28591, "ISO 8859-1 Latin I"),
    (28592, "ISO 8859-2 Central Europe"),
    (28593, "ISO 8859-3 Latin 3"),
    (28594, "ISO 8859-4 Baltic"),
    (28595, "ISO 8859-5 Cyrillic"),
    (28596, "ISO 8859-6 Arabic"),
    (28597, "ISO 8859-7 Greek"),
    (28598, "ISO 8859-8 Hebrew: Visual Ordering"),
    (28599, "ISO 8859-9 Latin 5"),
    (28603, "ISO 8859-13 Latin 7"),
    (28605, "ISO 8859-15 Latin 9"),
    (38598, "ISO 8859-8 Hebrew: Logical Ordering"),
    (50220, "ISO-2022 Japanese with no halfwidth Katakana"),
    (54936, "GB18030 Simplified Chinese"),
];

/// How a code page is converted.
enum Codec {
    Rs(&'static Encoding),
    /// An ISO 8859 page that the web (and so `encoding_rs`) reads as its Windows page, which
    /// differs only in 0x80-0x9F: there ISO has the C1 control characters, as Windows' ISO pages do.
    IsoOf(&'static Encoding),
    Ascii,
}

fn codec(cp: u32) -> Option<Codec> {
    use encoding_rs::*;
    let e = match cp {
        866 => IBM866,
        874 => WINDOWS_874,
        932 => SHIFT_JIS,
        936 => GBK,
        949 => EUC_KR,
        950 => BIG5,
        1250 => WINDOWS_1250,
        1251 => WINDOWS_1251,
        1252 => WINDOWS_1252,
        1253 => WINDOWS_1253,
        1254 => WINDOWS_1254,
        1255 => WINDOWS_1255,
        1256 => WINDOWS_1256,
        1257 => WINDOWS_1257,
        1258 => WINDOWS_1258,
        10000 => MACINTOSH,
        // the web's x-mac-cyrillic is the later Mac Cyrillic, Windows' 10017
        10017 => X_MAC_CYRILLIC,
        20127 => return Some(Codec::Ascii),
        20866 => KOI8_R,
        20932 => EUC_JP,
        21866 => KOI8_U,
        28591 => return Some(Codec::IsoOf(WINDOWS_1252)),
        28592 => ISO_8859_2,
        28593 => ISO_8859_3,
        28594 => ISO_8859_4,
        28595 => ISO_8859_5,
        28596 => ISO_8859_6,
        28597 => ISO_8859_7,
        28598 => ISO_8859_8,
        28599 => return Some(Codec::IsoOf(WINDOWS_1254)),
        28603 => ISO_8859_13,
        28605 => ISO_8859_15,
        38598 => ISO_8859_8_I,
        50220 => ISO_2022_JP,
        54936 => GB18030,
        65001 => UTF_8,
        _ => return None,
    };
    Some(Codec::Rs(e))
}

/// The code pages as (number, name), the name as Windows gives it ("1252  (ANSI - Latin I)"),
/// in Windows' order (by the number as text).
pub fn code_pages() -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = PAGES.iter().map(|(cp, name)| (*cp, format!("{cp:<5} ({name})"))).collect();
    out.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));
    out
}

/// Whether `encoding` has the character (as one with nothing else written before it).
fn has(encoding: &'static Encoding, c: char) -> bool {
    let mut buf = [0u8; 4];
    !encoding.encode(c.encode_utf8(&mut buf)).2
}

/// The bytes of `text` in a code page; characters it lacks become "?", as Windows writes them. A
/// code page there is no converter for gives UTF-8, as on Windows when the page isn't installed.
pub fn encode(text: &str, cp: u32) -> Vec<u8> {
    match codec(cp) {
        None => text.bytes().collect(),
        Some(Codec::Ascii) => text.chars().map(|c| if c.is_ascii() { c as u8 } else { b'?' }).collect(),
        Some(Codec::IsoOf(e)) => text
            .chars()
            .map(|c| match c as u32 {
                n @ 0x80..=0x9F => n as u8,
                _ => {
                    let mut buf = [0u8; 4];
                    let (b, _, bad) = e.encode(c.encode_utf8(&mut buf));
                    // what the Windows page has in 0x80-0x9F the ISO page lacks
                    if bad || b.len() != 1 || (0x80..=0x9F).contains(&b[0]) {
                        b'?'
                    } else {
                        b[0]
                    }
                }
            })
            .collect(),
        Some(Codec::Rs(e)) => {
            // encoding_rs writes a missing character as an HTML reference ("&#322;"), so those are
            // replaced first
            let clean: String = text.chars().map(|c| if has(e, c) { c } else { '?' }).collect();
            e.encode(&clean).0.into_owned()
        }
    }
}

/// Bytes read in a code page (a page there is no converter for reads them as UTF-8).
pub fn decode(bytes: &[u8], cp: u32) -> String {
    match codec(cp) {
        None => String::from_utf8_lossy(bytes).to_string(),
        // Windows drops the eighth bit
        Some(Codec::Ascii) => bytes.iter().map(|b| (b & 0x7F) as char).collect(),
        Some(Codec::IsoOf(e)) => bytes
            .iter()
            .map(|&b| if (0x80..=0x9F).contains(&b) { b as char } else { e.decode_without_bom_handling(&[b]).0.chars().next().unwrap_or('\u{FFFD}') })
            .collect(),
        Some(Codec::Rs(e)) => e.decode_without_bom_handling(bytes).0.into_owned(),
    }
}

/// The ANSI code page Windows gives a system locale, for a POSIX locale name (`pt_BR.UTF-8`,
/// `sr_RS@latin`): the one of its language (and script), 1252 for C, POSIX and the languages that
/// use the Latin alphabet of Western Europe.
pub fn ansi_code_page_for(locale: &str) -> u32 {
    let Some((lang, territory)) = crate::i18n::posix_locale_parts(locale) else { return 1252 };
    let modifier = locale.split_once('@').map(|m| m.1.to_ascii_lowercase()).unwrap_or_default();
    match lang.as_str() {
        "th" => 874,
        "ja" => 932,
        "zh" if matches!(territory.as_str(), "TW" | "HK" | "MO") => 950,
        "zh" => 936,
        "ko" => 949,
        // Serbian is Cyrillic unless the locale says Latin; Bosnian the other way round
        "sr" if modifier == "latin" => 1250,
        "bs" if modifier == "cyrillic" => 1251,
        "cs" | "pl" | "hu" | "sk" | "sl" | "hr" | "ro" | "sq" | "bs" | "hsb" | "dsb" | "tk" => 1250,
        // Azeri and Uzbek are Latin unless the locale says Cyrillic
        "az" | "uz" if modifier == "cyrillic" => 1251,
        "ru" | "uk" | "be" | "bg" | "mk" | "sr" | "kk" | "ky" | "mn" | "tt" | "ba" | "sah" | "tg" | "cv" => 1251,
        "el" => 1253,
        "tr" | "az" | "uz" => 1254,
        "he" | "yi" => 1255,
        "ar" | "fa" | "ur" | "ps" | "ug" | "sd" | "ckb" => 1256,
        "et" | "lv" | "lt" | "ltg" => 1257,
        "vi" => 1258,
        _ => 1252,
    }
}

/// The ANSI code page of the locale environment: the character type locale (the first set of
/// `LC_ALL`, `LC_CTYPE` and `LANG`), as Windows' comes from the system locale.
pub fn ansi_code_page() -> u32 {
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"].into_iter().find_map(|n| std::env::var(n).ok().filter(|v| !v.trim().is_empty()));
    ansi_code_page_for(&locale.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_page_list_has_windows_names_and_order() {
        let pages = code_pages();
        let label = |cp: u32| pages.iter().find(|p| p.0 == cp).map(|p| p.1.clone());
        assert_eq!(label(1252).as_deref(), Some("1252  (ANSI - Latin I)"));
        assert_eq!(label(874).as_deref(), Some("874   (ANSI/OEM - Thai)"));
        assert_eq!(label(10000).as_deref(), Some("10000 (MAC - Roman)"));
        assert_eq!(label(28591).as_deref(), Some("28591 (ISO 8859-1 Latin I)"));
        // Windows leaves the Unicode pages out of the list
        assert!(label(65001).is_none() && label(1200).is_none());
        // sorted as text, as Windows sorts them: 10000 before 1250, 874 after 54936
        let order: Vec<u32> = pages.iter().map(|p| p.0).collect();
        assert_eq!(order[0], 10000);
        assert!(order.iter().position(|&c| c == 1250) < order.iter().position(|&c| c == 866));
        assert_eq!(*order.last().unwrap(), 950);
        // every listed page converts
        assert!(pages.iter().all(|p| codec(p.0).is_some()));
    }

    #[test]
    fn code_pages_round_trip() {
        for (cp, text) in [
            (1252, "Olá, ação — 5 €"),
            (1250, "Žluťoučký kůň, łódź"),
            (1251, "Привет, мир"),
            (1253, "Καλημέρα"),
            (1254, "Günaydın, İstanbul"),
            (1257, "Ąžuolas, ūdens"),
            (866, "Привет"),
            (20866, "Привет"),
            (21866, "Привіт, ґанок"),
            (932, "日本語のテキスト"),
            (20932, "日本語"),
            (50220, "日本語"),
            (936, "简体中文"),
            (54936, "简体中文"),
            (950, "繁體中文"),
            (949, "한국어"),
            (874, "ภาษาไทย"),
            (28591, "Olá"),
            (28592, "Łódź"),
            (28605, "5 € œ"),
            (28599, "İstanbul"),
            (10000, "Olá"),
            (65001, "Olá 日本"),
        ] {
            let bytes = encode(text, cp);
            assert_eq!(decode(&bytes, cp), text, "code page {cp}");
        }
        // the bytes themselves
        assert_eq!(encode("Olá €", 1252), b"Ol\xE1 \x80");
        assert_eq!(encode("é", 28591), b"\xE9");
        assert_eq!(encode("Ж", 1251), b"\xC6");
        assert_eq!(encode("日", 932), b"\x93\xFA");
        // a character the page lacks becomes "?"
        assert_eq!(encode("a日b", 1252), b"a?b");
        assert_eq!(encode("€", 28591), b"?");
        assert_eq!(encode("Ł", 1252), b"?");
        assert_eq!(encode("é", 20127), b"?");
        // ISO pages keep the C1 controls where the Windows pages have letters
        assert_eq!(decode(b"\x80", 28591), "\u{80}");
        assert_eq!(decode(b"\x80", 1252), "€");
        assert_eq!(decode(b"\xE9", 20127), "i");
        // a page there is no converter for: UTF-8, as Windows without the page
        assert_eq!(encode("Olá", 37), "Olá".as_bytes());
        assert_eq!(decode("Olá".as_bytes(), 37), "Olá");
        assert!(encode("", 1252).is_empty() && decode(b"", 932).is_empty());
    }

    #[test]
    fn ansi_code_page_follows_the_locale_language() {
        for (locale, cp) in [
            ("", 1252),
            ("C", 1252),
            ("C.UTF-8", 1252),
            ("POSIX", 1252),
            ("en_US.UTF-8", 1252),
            ("pt_BR.UTF-8", 1252),
            ("de_DE@euro", 1252),
            ("fr_FR", 1252),
            ("es_ES.UTF-8", 1252),
            ("it_IT", 1252),
            ("nl_NL", 1252),
            ("sv_SE", 1252),
            ("cs_CZ.UTF-8", 1250),
            ("pl_PL", 1250),
            ("hu_HU", 1250),
            ("ru_RU.UTF-8", 1251),
            ("uk_UA", 1251),
            ("sr_RS", 1251),
            ("sr_RS@latin", 1250),
            ("el_GR", 1253),
            ("tr_TR.UTF-8", 1254),
            ("uz_UZ", 1254),
            ("uz_UZ@cyrillic", 1251),
            ("he_IL", 1255),
            ("ar_EG", 1256),
            ("lt_LT", 1257),
            ("vi_VN", 1258),
            ("th_TH", 874),
            ("ja_JP.UTF-8", 932),
            ("zh_CN.UTF-8", 936),
            ("zh_SG", 936),
            ("zh_TW.UTF-8", 950),
            ("zh_HK", 950),
            ("ko_KR.UTF-8", 949),
        ] {
            assert_eq!(ansi_code_page_for(locale), cp, "{locale:?}");
        }
    }

    /// The conversions agree with Windows' own (MultiByteToWideChar, WideCharToMultiByte).
    #[cfg(windows)]
    #[test]
    fn code_pages_match_windows() {
        let windows_pages: Vec<u32> = super::super::imp::code_pages().into_iter().map(|p| p.0).collect();
        let windows_names = super::super::imp::code_pages();
        for (cp, label) in code_pages() {
            // the same name as Windows gives the page, where it is installed
            if let Some(w) = windows_names.iter().find(|p| p.0 == cp) {
                assert_eq!(w.1, label, "name of {cp}");
            }
            if !windows_pages.contains(&cp) {
                continue;
            }
            let single = !matches!(cp, 932 | 936 | 949 | 950 | 20932 | 50220 | 54936);
            if single {
                for b in 0u8..=255 {
                    let win = super::super::imp::decode(&[b], cp);
                    let ours = decode(&[b], cp);
                    // Windows gives undefined bytes private-use characters; encoding_rs U+FFFD
                    let undefined = win.chars().all(|c| ('\u{E000}'..='\u{F8FF}').contains(&c)) || ours == "\u{FFFD}";
                    if !undefined {
                        assert_eq!(ours, win, "code page {cp}, byte {b:#04x}");
                        // and back: the same byte
                        assert_eq!(encode(&ours, cp), super::super::imp::encode(&win, cp), "code page {cp}, U+{:04X}", ours.chars().next().map(|c| c as u32).unwrap_or(0));
                    }
                }
            } else {
                let text = "Olá 日本語 简体 繁體 한국어 ascii";
                let ours = decode(&encode(text, cp), cp);
                let win = super::super::imp::decode(&super::super::imp::encode(text, cp), cp);
                // the characters the page has come back the same
                for (a, b) in ours.chars().zip(win.chars()) {
                    if a != '?' && b != '?' && a != '・' && b != '・' {
                        assert_eq!(a, b, "code page {cp}: {ours} / {win}");
                    }
                }
            }
        }
    }
}
