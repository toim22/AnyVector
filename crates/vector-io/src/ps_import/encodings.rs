//! PostScript's standard encodings and the Unicode characters of glyph names.

/// StandardEncoding: (code, glyph) for the codes that have a glyph.
const STANDARD_HIGH: &[(u8, &str)] = &[
    (161, "exclamdown"),
    (162, "cent"),
    (163, "sterling"),
    (164, "fraction"),
    (165, "yen"),
    (166, "florin"),
    (167, "section"),
    (168, "currency"),
    (169, "quotesingle"),
    (170, "quotedblleft"),
    (171, "guillemotleft"),
    (172, "guilsinglleft"),
    (173, "guilsinglright"),
    (174, "fi"),
    (175, "fl"),
    (177, "endash"),
    (178, "dagger"),
    (179, "daggerdbl"),
    (180, "periodcentered"),
    (182, "paragraph"),
    (183, "bullet"),
    (184, "quotesinglbase"),
    (185, "quotedblbase"),
    (186, "quotedblright"),
    (187, "guillemotright"),
    (188, "ellipsis"),
    (189, "perthousand"),
    (191, "questiondown"),
    (193, "grave"),
    (194, "acute"),
    (195, "circumflex"),
    (196, "tilde"),
    (197, "macron"),
    (198, "breve"),
    (199, "dotaccent"),
    (200, "dieresis"),
    (202, "ring"),
    (203, "cedilla"),
    (205, "hungarumlaut"),
    (206, "ogonek"),
    (207, "caron"),
    (208, "emdash"),
    (225, "AE"),
    (227, "ordfeminine"),
    (232, "Lslash"),
    (233, "Oslash"),
    (234, "OE"),
    (235, "ordmasculine"),
    (241, "ae"),
    (245, "dotlessi"),
    (248, "lslash"),
    (249, "oslash"),
    (250, "oe"),
    (251, "germandbls"),
];

/// The printable ASCII glyph names, from 32.
const ASCII: [&str; 95] = [
    "space", "exclam", "quotedbl", "numbersign", "dollar", "percent", "ampersand", "quoteright", "parenleft", "parenright", "asterisk", "plus", "comma", "hyphen", "period", "slash", "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "colon", "semicolon", "less", "equal", "greater", "question", "at", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "bracketleft", "backslash", "bracketright", "asciicircum", "underscore", "quoteleft", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z", "braceleft", "bar", "braceright", "asciitilde",
];

/// Latin-1 glyph names from 160 (ISOLatin1Encoding's upper half).
const LATIN1: [&str; 96] = [
    "space", "exclamdown", "cent", "sterling", "currency", "yen", "brokenbar", "section", "dieresis", "copyright", "ordfeminine", "guillemotleft", "logicalnot", "hyphen", "registered", "macron", "degree", "plusminus", "twosuperior", "threesuperior", "acute", "mu", "paragraph", "periodcentered", "cedilla", "onesuperior", "ordmasculine", "guillemotright", "onequarter", "onehalf", "threequarters", "questiondown", "Agrave", "Aacute", "Acircumflex", "Atilde", "Adieresis", "Aring", "AE", "Ccedilla", "Egrave", "Eacute", "Ecircumflex", "Edieresis", "Igrave", "Iacute", "Icircumflex", "Idieresis", "Eth", "Ntilde", "Ograve", "Oacute", "Ocircumflex", "Otilde", "Odieresis", "multiply", "Oslash", "Ugrave", "Uacute", "Ucircumflex", "Udieresis", "Yacute", "Thorn", "germandbls", "agrave", "aacute", "acircumflex", "atilde", "adieresis", "aring", "ae", "ccedilla", "egrave", "eacute", "ecircumflex", "edieresis", "igrave", "iacute", "icircumflex", "idieresis", "eth", "ntilde", "ograve", "oacute", "ocircumflex", "otilde", "odieresis", "divide", "oslash", "ugrave", "uacute", "ucircumflex", "udieresis", "yacute", "thorn", "ydieresis",
];

/// ISOLatin1Encoding's accents from 144.
const LATIN1_ACCENTS: [(u8, &str); 15] = [
    (144, "dotlessi"),
    (145, "grave"),
    (146, "acute"),
    (147, "circumflex"),
    (148, "tilde"),
    (149, "macron"),
    (150, "breve"),
    (151, "dotaccent"),
    (152, "dieresis"),
    (154, "ring"),
    (155, "cedilla"),
    (157, "hungarumlaut"),
    (158, "ogonek"),
    (159, "caron"),
    (39, "quoteright"),
];

pub fn standard() -> Vec<&'static str> {
    let mut v = vec![".notdef"; 256];
    for (k, n) in ASCII.iter().enumerate() {
        v[32 + k] = n;
    }
    for (c, n) in STANDARD_HIGH {
        v[*c as usize] = n;
    }
    v
}

pub fn iso_latin1() -> Vec<&'static str> {
    let mut v = vec![".notdef"; 256];
    for (k, n) in ASCII.iter().enumerate() {
        v[32 + k] = n;
    }
    v[45] = "minus";
    for (k, n) in LATIN1.iter().enumerate() {
        v[160 + k] = n;
    }
    for (c, n) in LATIN1_ACCENTS {
        v[c as usize] = n;
    }
    v
}

/// Names whose characters are not their Latin-1 or ASCII position.
const SPECIAL: &[(&str, char)] = &[
    ("quoteright", '\u{2019}'),
    ("quoteleft", '\u{2018}'),
    ("quotesingle", '\''),
    ("fraction", '\u{2044}'),
    ("florin", '\u{0192}'),
    ("quotedblleft", '\u{201C}'),
    ("quotedblright", '\u{201D}'),
    ("guilsinglleft", '\u{2039}'),
    ("guilsinglright", '\u{203A}'),
    ("fi", '\u{FB01}'),
    ("fl", '\u{FB02}'),
    ("endash", '\u{2013}'),
    ("emdash", '\u{2014}'),
    ("dagger", '\u{2020}'),
    ("daggerdbl", '\u{2021}'),
    ("bullet", '\u{2022}'),
    ("quotesinglbase", '\u{201A}'),
    ("quotedblbase", '\u{201E}'),
    ("ellipsis", '\u{2026}'),
    ("perthousand", '\u{2030}'),
    ("grave", '`'),
    ("circumflex", '\u{02C6}'),
    ("tilde", '\u{02DC}'),
    ("breve", '\u{02D8}'),
    ("dotaccent", '\u{02D9}'),
    ("ring", '\u{02DA}'),
    ("hungarumlaut", '\u{02DD}'),
    ("ogonek", '\u{02DB}'),
    ("caron", '\u{02C7}'),
    ("Lslash", '\u{0141}'),
    ("lslash", '\u{0142}'),
    ("OE", '\u{0152}'),
    ("oe", '\u{0153}'),
    ("dotlessi", '\u{0131}'),
    ("Scaron", '\u{0160}'),
    ("scaron", '\u{0161}'),
    ("Zcaron", '\u{017D}'),
    ("zcaron", '\u{017E}'),
    ("Ydieresis", '\u{0178}'),
    ("Euro", '\u{20AC}'),
    ("trademark", '\u{2122}'),
    ("minus", '\u{2212}'),
    ("hyphen", '-'),
    ("nbspace", '\u{00A0}'),
    ("sfthyphen", '\u{00AD}'),
    ("periodcentered", '\u{00B7}'),
    ("middot", '\u{00B7}'),
    ("Delta", '\u{2206}'),
    ("Omega", '\u{2126}'),
    ("mu", '\u{00B5}'),
    ("pi", '\u{03C0}'),
    ("lessequal", '\u{2264}'),
    ("greaterequal", '\u{2265}'),
    ("notequal", '\u{2260}'),
    ("infinity", '\u{221E}'),
    ("partialdiff", '\u{2202}'),
    ("summation", '\u{2211}'),
    ("product", '\u{220F}'),
    ("integral", '\u{222B}'),
    ("radical", '\u{221A}'),
    ("approxequal", '\u{2248}'),
    ("lozenge", '\u{25CA}'),
    ("Gbreve", '\u{011E}'),
    ("gbreve", '\u{011F}'),
    ("Idotaccent", '\u{0130}'),
    ("Scedilla", '\u{015E}'),
    ("scedilla", '\u{015F}'),
    ("Cacute", '\u{0106}'),
    ("cacute", '\u{0107}'),
    ("Ccaron", '\u{010C}'),
    ("ccaron", '\u{010D}'),
    ("Dcroat", '\u{0110}'),
    ("dcroat", '\u{0111}'),
    ("Ecaron", '\u{011A}'),
    ("ecaron", '\u{011B}'),
    ("Eogonek", '\u{0118}'),
    ("eogonek", '\u{0119}'),
    ("Aogonek", '\u{0104}'),
    ("aogonek", '\u{0105}'),
    ("Nacute", '\u{0143}'),
    ("nacute", '\u{0144}'),
    ("Ncaron", '\u{0147}'),
    ("ncaron", '\u{0148}'),
    ("Rcaron", '\u{0158}'),
    ("rcaron", '\u{0159}'),
    ("Sacute", '\u{015A}'),
    ("sacute", '\u{015B}'),
    ("Tcaron", '\u{0164}'),
    ("tcaron", '\u{0165}'),
    ("Uring", '\u{016E}'),
    ("uring", '\u{016F}'),
    ("Zacute", '\u{0179}'),
    ("zacute", '\u{017A}'),
    ("Zdotaccent", '\u{017B}'),
    ("zdotaccent", '\u{017C}'),
    ("Ohungarumlaut", '\u{0150}'),
    ("ohungarumlaut", '\u{0151}'),
    ("Uhungarumlaut", '\u{0170}'),
    ("uhungarumlaut", '\u{0171}'),
    ("Lcaron", '\u{013D}'),
    ("lcaron", '\u{013E}'),
    ("Lacute", '\u{0139}'),
    ("lacute", '\u{013A}'),
    ("Racute", '\u{0154}'),
    ("racute", '\u{0155}'),
    ("Dcaron", '\u{010E}'),
    ("dcaron", '\u{010F}'),
    ("Abreve", '\u{0102}'),
    ("abreve", '\u{0103}'),
    ("Tcommaaccent", '\u{0162}'),
    ("tcommaaccent", '\u{0163}'),
];

/// The character a glyph name stands for.
pub fn glyph_char(name: &str) -> Option<char> {
    if let Some(&(_, c)) = SPECIAL.iter().find(|(n, _)| *n == name) {
        return Some(c);
    }
    if let Some(k) = ASCII.iter().position(|n| *n == name) {
        return char::from_u32(32 + k as u32);
    }
    if let Some(k) = LATIN1.iter().position(|n| *n == name) {
        return char::from_u32(160 + k as u32);
    }
    // uniXXXX, uXXXX[XX]
    if let Some(h) = name.strip_prefix("uni") {
        if h.len() >= 4 {
            return u32::from_str_radix(&h[..4], 16).ok().and_then(char::from_u32);
        }
    }
    if let Some(h) = name.strip_prefix('u') {
        if (4..=6).contains(&h.len()) && h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return u32::from_str_radix(h, 16).ok().and_then(char::from_u32);
        }
    }
    // a suffix after a period ("a.sc", "one.oldstyle")
    if let Some((base, _)) = name.split_once('.') {
        if !base.is_empty() {
            return glyph_char(base);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings() {
        let s = standard();
        assert_eq!(s[65], "A");
        assert_eq!(s[39], "quoteright");
        assert_eq!(s[225], "AE");
        let l = iso_latin1();
        assert_eq!(l[233], "eacute");
        assert_eq!(l[45], "minus");
        assert_eq!(glyph_char("eacute"), Some('é'));
        assert_eq!(glyph_char("quoteright"), Some('\u{2019}'));
        assert_eq!(glyph_char("uni20AC"), Some('€'));
        assert_eq!(glyph_char("a.sc"), Some('a'));
        assert_eq!(glyph_char("zero"), Some('0'));
    }
}
