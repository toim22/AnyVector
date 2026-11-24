//! Code pages without Windows (Text ▸ Encode on Linux): the pages Windows lists, under the names
//! Windows gives them, converted by `encoding_rs` or a table, and the ANSI code page of a locale.
//! Built on every system so the Windows tests can compare it with Windows' own conversions.

#![cfg_attr(windows, allow(dead_code))]

use super::upper::UPPER;
use encoding_rs::Encoding;

/// The code pages Text ▸ Encode lists, with the names `GetCPInfoEx` gives them on an English
/// Windows 11. Left out of the pages Windows installs: EBCDIC, ISCII, the 7-bit ones (IA5, GSM,
/// ISO-2022 but Japanese), Johab and the old Taiwanese and Mac double-byte ones.
const PAGES: &[(u32, &str)] = &[
    (437, "OEM - United States"),
    (708, "Arabic - ASMO"),
    (720, "Arabic - Transparent ASMO"),
    (737, "OEM - Greek 437G"),
    (775, "OEM - Baltic"),
    (850, "OEM - Multilingual Latin I"),
    (852, "OEM - Latin II"),
    (855, "OEM - Cyrillic"),
    (857, "OEM - Turkish"),
    (858, "OEM - Multilingual Latin I + Euro"),
    (860, "OEM - Portuguese"),
    (861, "OEM - Icelandic"),
    (862, "OEM - Hebrew"),
    (863, "OEM - Canadian French"),
    (864, "OEM - Arabic"),
    (865, "OEM - Nordic"),
    (866, "OEM - Russian"),
    (869, "OEM - Modern Greek"),
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
    (10004, "MAC - Arabic"),
    (10005, "MAC - Hebrew"),
    (10006, "MAC - Greek I"),
    (10007, "MAC - Cyrillic"),
    (10010, "MAC - Romania"),
    (10017, "MAC - Ukraine"),
    (10029, "MAC - Latin II"),
    (10079, "MAC - Icelandic"),
    (10081, "MAC - Turkish"),
    (10082, "MAC - Croatia"),
    (20127, "US-ASCII"),
    (20866, "Russian - KOI8"),
    (20932, "JIS X 0208-1990 & 0212-1990"),
    (20936, "Simplified Chinese GB2312"),
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
    (50221, "ISO-2022 Japanese with halfwidth Katakana"),
    (51949, "EUC-Korean"),
    (54936, "GB18030 Simplified Chinese"),
];

/// The `encoding_rs` encoding of a code page with more than one byte a character.
fn multi_byte(cp: u32) -> Option<&'static Encoding> {
    use encoding_rs::*;
    Some(match cp {
        932 => SHIFT_JIS,
        936 | 20936 => GBK,
        949 | 51949 => EUC_KR,
        950 => BIG5,
        20932 => EUC_JP,
        50220 | 50221 => ISO_2022_JP,
        54936 => GB18030,
        65001 => UTF_8,
        _ => return None,
    })
}

/// The 256 characters of a single-byte code page, the same as Windows'.
fn single_byte(cp: u32) -> Option<[char; 256]> {
    use encoding_rs::*;
    let mut t = ['\0'; 256];
    for (b, c) in t.iter_mut().enumerate() {
        *c = b as u8 as char;
    }
    if let Some((_, upper)) = UPPER.iter().find(|u| u.0 == cp) {
        for (c, u) in t[128..].iter_mut().zip(upper.chars()) {
            *c = u;
        }
        return Some(t);
    }
    // the encoding, the bytes Windows reads otherwise, and the private-use character Windows gives
    // the first byte the page leaves undefined (the next ones get the characters after it)
    let (e, differ, undefined): (&Encoding, &[(u8, char)], u32) = match cp {
        // ISO 8859-1 is the first 256 characters of Unicode
        28591 => return Some(t),
        20127 => {
            // Windows drops the eighth bit
            for b in 128..256 {
                t[b] = t[b - 128];
            }
            return Some(t);
        }
        866 => (IBM866, &[], 0),
        874 => (WINDOWS_874, &[], 0xF8C1),
        1250 => (WINDOWS_1250, &[], 0),
        1251 => (WINDOWS_1251, &[], 0),
        1252 => (WINDOWS_1252, &[], 0),
        1253 => (WINDOWS_1253, &[], 0xF8F9),
        1254 => (WINDOWS_1254, &[], 0),
        1255 => (WINDOWS_1255, &[], 0xF88D),
        1256 => (WINDOWS_1256, &[], 0),
        1257 => (WINDOWS_1257, &[], 0xF8FC),
        1258 => (WINDOWS_1258, &[], 0),
        // the ohm sign, not the capital omega
        10000 => (MACINTOSH, &[(0xBD, '\u{2126}')], 0),
        // the web's x-mac-cyrillic is Windows' 10017, but for the euro sign
        10017 => (X_MAC_CYRILLIC, &[(0xFF, '¤')], 0),
        20866 => (KOI8_R, &[], 0),
        21866 => (KOI8_U, &[], 0),
        28592 => (ISO_8859_2, &[], 0),
        28593 => (ISO_8859_3, &[], 0xF7F5),
        28594 => (ISO_8859_4, &[], 0),
        28595 => (ISO_8859_5, &[], 0),
        28596 => (ISO_8859_6, &[], 0xF7C8),
        // Windows has the 1987 edition
        28597 => (ISO_8859_7, &[(0xA1, '\u{2BD}'), (0xA2, '\u{2BC}'), (0xA4, '\u{F7C2}'), (0xA5, '\u{F7C3}'), (0xAA, '\u{F7C4}')], 0xF7C5),
        28598 | 38598 => (ISO_8859_8, &[(0xAF, '\u{203E}'), (0xFD, '\u{F7BF}'), (0xFE, '\u{F7C0}'), (0xFF, '\u{F7C1}')], 0xF79C),
        // the web reads ISO 8859-9 as 1254, which differs in 0x80-0x9F (done below)
        28599 => (WINDOWS_1254, &[], 0),
        28603 => (ISO_8859_13, &[], 0),
        28605 => (ISO_8859_15, &[], 0),
        _ => return None,
    };
    let mut undefined = undefined;
    for b in 128..256usize {
        // ISO pages have the control characters there, as `t` already does
        if cp == 28599 && b < 0xA0 {
            continue;
        }
        let byte = b as u8;
        if let Some((_, c)) = differ.iter().find(|d| d.0 == byte) {
            t[b] = *c;
            continue;
        }
        let one = [byte];
        let (s, _) = e.decode_without_bom_handling(&one);
        t[b] = match s.chars().next() {
            Some(c) if c != '\u{FFFD}' => c,
            _ => {
                let c = char::from_u32(undefined).unwrap_or('\u{FFFD}');
                undefined += 1;
                c
            }
        };
    }
    Some(t)
}

/// The code pages as (number, name), the name as Windows gives it ("1252  (ANSI - Latin I)"),
/// in Windows' order (by the number as text).
pub fn code_pages() -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = PAGES.iter().map(|(cp, name)| (*cp, format!("{cp:<5} ({name})"))).collect();
    out.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));
    out
}

/// Whether a page has the character (as one with nothing else written before it).
fn has(cp: u32, encoding: &'static Encoding, c: char) -> bool {
    let mut buf = [0u8; 4];
    let (bytes, _, missing) = encoding.encode(c.encode_utf8(&mut buf));
    // GB2312 is the part of GBK with both bytes above 0xA0
    !missing && (cp != 20936 || c.is_ascii() || bytes.iter().all(|b| *b > 0xA0))
}

/// The bytes of `text` in a code page; characters it lacks become "?", as Windows writes them. A
/// code page there is no converter for gives UTF-8, as on Windows when the page isn't installed.
pub fn encode(text: &str, cp: u32) -> Vec<u8> {
    if let Some(t) = single_byte(cp) {
        return text.chars().map(|c| t.iter().position(|x| *x == c).map_or(b'?', |b| b as u8)).collect();
    }
    match multi_byte(cp) {
        None => text.bytes().collect(),
        Some(e) => {
            // encoding_rs writes a missing character as an HTML reference ("&#322;"), so those are
            // replaced first
            let clean: String = text.chars().map(|c| if has(cp, e, c) { c } else { '?' }).collect();
            e.encode(&clean).0.into_owned()
        }
    }
}

/// Bytes read in a code page (a page there is no converter for reads them as UTF-8).
pub fn decode(bytes: &[u8], cp: u32) -> String {
    if let Some(t) = single_byte(cp) {
        return bytes.iter().map(|b| t[*b as usize]).collect();
    }
    match multi_byte(cp) {
        None => String::from_utf8_lossy(bytes).to_string(),
        Some(e) => e.decode_without_bom_handling(bytes).0.into_owned(),
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
        assert!(pages.iter().all(|p| single_byte(p.0).is_some() || multi_byte(p.0).is_some()));
        assert_eq!(label(850).as_deref(), Some("850   (OEM - Multilingual Latin I)"));
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
            (10007, "Привет"),
            (437, "Olá ░▒▓"),
            (850, "ação ©"),
            (860, "ação"),
            (858, "5 €"),
            (20936, "简体中文"),
            (51949, "한국어"),
            (50221, "日本語"),
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
        // bytes a page leaves undefined get the private-use characters Windows gives them
        assert_eq!(decode(b"\xAA\xD2", 1253), "\u{F8F9}\u{F8FA}");
        // GB2312 lacks what GBK added
        assert_eq!(encode("語", 20936), b"?");
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

    /// The names and the conversions are Windows' own (GetCPInfoEx, MultiByteToWideChar,
    /// WideCharToMultiByte).
    #[cfg(windows)]
    #[test]
    fn code_pages_match_windows() {
        use super::super::imp;
        // the spell checker's DLLs first: loading them while this thread loads the DLLs of the
        // ISO-2022 and GB18030 pages (another test may) locks Windows up
        imp::suggest("wrld", "en-US");
        let windows = imp::code_pages();
        for (cp, label) in code_pages() {
            // Windows may not have every page installed, nor its names in English
            let Some((_, name)) = windows.iter().find(|p| p.0 == cp) else { continue };
            if windows.iter().any(|p| p.1 == "1252  (ANSI - Latin I)") {
                assert_eq!(*name, label);
            }
            if single_byte(cp).is_some() {
                for b in 0u8..=255 {
                    let c = imp::decode(&[b], cp);
                    assert_eq!(decode(&[b], cp), c, "code page {cp}, byte {b:#04x}");
                    // back to the same byte, or another one the page has for the character
                    // (US-ASCII's upper half, Mac Hebrew's)
                    let back = encode(&c, cp);
                    assert!(back == [b] || imp::decode(&back, cp) == c, "code page {cp}, byte {b:#04x}: {back:?}");
                }
                continue;
            }
            let text = match cp {
                949 | 51949 => "Text 한국어 漢字",
                950 => "Text 繁體中文",
                936 | 20936 => "Text 简体中文",
                54936 => "Text 简体中文 한국어 ﾊ Ž",
                50220 => "Text 日本語のテキスト",
                _ => "Text 日本語のテキスト",
            };
            assert_eq!(encode(text, cp), imp::encode(text, cp), "code page {cp}");
            assert_eq!(decode(&imp::encode(text, cp), cp), text, "code page {cp}");
            if cp != 54936 {
                // "?" for what the page lacks
                assert_eq!(encode("a\u{E000F}b", cp).iter().filter(|b| **b == b'?').count(), 1, "code page {cp}");
            }
        }
        assert_eq!(encode("漢語", 20936), imp::encode("漢語", 20936));
    }
}
