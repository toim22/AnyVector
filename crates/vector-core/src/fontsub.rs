//! Font substitution: which installed family shows text set in a family that isn't installed.
//!
//! The order is: the family itself (ignoring case) → where Windows' fonts aren't installed (Linux),
//! the open font made to the Windows font's metrics from [`EQUIVALENTS`] → a known look-alike
//! from [`ALIASES`] that is installed (or, off Windows, its equivalent) → an installed face of
//! the same width class (a "Narrow" name gets a condensed face) → the generic sans-serif face.
//! `FontDb::resolve` in `text.rs` runs it; this module holds the data and the name rules, so they
//! can be tested without fonts.

/// Fonts that old drawings name but Windows doesn't have, and the installed families that look
/// and measure most like them, best first. The first entry that is installed is used; when none
/// is, the width rules below choose (a name or a first substitute with "Narrow" in it asks for a
/// condensed face). Names are compared without case, spaces or punctuation, so "Switzerland
/// Narrow" and "SwitzerlandNarrow" are one entry. Only add substitutes here, never a family
/// that is the font itself under its own name: an installed font is always used as it is.
///
/// CorelDRAW 1 and 2 read their fonts from `.wfn` files; `corel::waldo` in vector-io maps the
/// file names to the CorelDRAW names below and takes the first substitute.
pub const ALIASES: &[(&str, &[&str])] = &[
    // --- CorelDRAW 1-5's own names for the classic typefaces (also the .wfn fonts of 1 and 2) ---
    // Prose Antique: CorelDRAW's Bookman-like oldstyle; Book Antiqua is the Palatino-width match.
    ("ProseAntique", &["Book Antiqua", "Palatino Linotype"]),
    // Avalon: CorelDRAW's ITC Avant Garde Gothic.
    ("Avalon", &["Century Gothic", "URW Gothic"]),
    // Bahamas (and its heavy and light weights): CorelDRAW's Bauhaus.
    ("BahamasHeavy", &["Bauhaus 93"]),
    ("BahamasLight", &["Bauhaus 93"]),
    ("Bahamas", &["Bauhaus 93"]),
    // Banff: CorelDRAW's Brush Script.
    ("Banff", &["Brush Script MT"]),
    // Bodnoff: CorelDRAW's Bodoni Poster.
    ("Bodnoff", &["Bodoni MT Black"]),
    // Brooklyn: CorelDRAW's ITC Bookman.
    ("Brooklyn", &["Bookman Old Style", "URW Bookman"]),
    // New Brunswick: CorelDRAW's New Century Schoolbook.
    ("NewBrunswick", &["Century Schoolbook", "C059"]),
    // Casablanca: CorelDRAW 1's text face, closest to Garamond on Windows.
    ("Casablanca", &["Garamond"]),
    // Centurion Old: CorelDRAW's Century Oldstyle.
    ("CenturionOld", &["Century"]),
    // Cupertino: CorelDRAW's Cooper Black.
    ("Cupertino", &["Cooper Black"]),
    // Erie (black, light): CorelDRAW's ITC Eras.
    ("Erie", &["Eras Medium ITC"]),
    ("ErieBlack", &["Eras Demi ITC"]),
    ("ErieLight", &["Eras Light ITC"]),
    // Frankfurt Gothic (heavy): CorelDRAW's Franklin Gothic.
    ("FrankfurtGothic", &["Franklin Gothic Book"]),
    ("FrankfurtGothicHeavy", &["Franklin Gothic Heavy"]),
    // Frankenstein: CorelDRAW's blackletter.
    ("Frankenstein", &["Old English Text MT"]),
    // Freeport: CorelDRAW's casual script.
    ("Freeport", &["Freestyle Script"]),
    // Fujiyama (light, extra bold): CorelDRAW's condensed Futura-like faces.
    ("Fujiyama", &["Tw Cen MT Condensed"]),
    ("Fujiyama2", &["Tw Cen MT"]),
    ("FujiyamaExtraBold", &["Tw Cen MT Condensed Extra Bold"]),
    ("FujiyamaLight", &["Tw Cen MT Condensed"]),
    // Gatineau: CorelDRAW 1's text face, closest to Garamond on Windows.
    ("Gatineau", &["Garamond"]),
    // Lincoln: CorelDRAW's Old English.
    ("Lincoln", &["Old English Text MT"]),
    // Memorandum and Monospaced: CorelDRAW's typewriter faces.
    ("Memorandum", &["Courier New", "Liberation Mono"]),
    ("Monospaced", &["Courier New", "Liberation Mono"]),
    // Mystical: CorelDRAW's Mistral-like script.
    ("Mystical", &["Mistral"]),
    // Nebraska: CorelDRAW's Baskerville.
    ("Nebraska", &["Baskerville Old Face"]),
    // Palm Springs: CorelDRAW's Palatino.
    ("PalmSprings", &["Palatino Linotype", "Book Antiqua"]),
    // Stamp: CorelDRAW's Stencil.
    ("Stamp", &["Stencil"]),
    // Switzerland family: CorelDRAW's Helvetica; Arial is metric-compatible with it.
    ("Switzerland", &["Arial", "Liberation Sans"]),
    ("SwitzerlandBlack", &["Arial Black"]),
    ("SwitzerlandCondBlack", &["Arial Narrow", "Liberation Sans Narrow"]),
    ("SwitzerlandCondensed", &["Arial Narrow", "Liberation Sans Narrow"]),
    ("SwitzerlandCondLight", &["Arial Narrow", "Liberation Sans Narrow"]),
    ("SwitzerlandInserat", &["Impact"]),
    ("SwitzerlandLight", &["Arial", "Liberation Sans"]),
    ("SwitzerlandNarrow", &["Arial Narrow", "Liberation Sans Narrow"]),
    // Greek/Math Symbols: CorelDRAW's Symbol.
    ("GreekMathSymbols", &["Symbol"]),
    // Toronto: CorelDRAW 1's Times.
    ("Toronto", &["Times New Roman", "Liberation Serif"]),
    // USA Black / USA Light: CorelDRAW's Helvetica black and light.
    ("USABlack", &["Arial Black"]),
    ("USALight", &["Arial", "Liberation Sans"]),
    // --- CorelDRAW 3-5's names (Bitstream fonts renamed by Corel) ---
    // Bedrock: a very condensed display face (the CorelDRAW 5 demo's "ON THE RIGHT TRACK!"); no
    // Windows twin, so the narrowest common sans keeps the text's width.
    ("Bedrock", &["Arial Narrow", "Liberation Sans Narrow", "Roboto Condensed"]),
    // Dutch: Bitstream's Times Roman.
    ("Dutch", &["Times New Roman", "Liberation Serif"]),
    // Zurich: Bitstream's Univers; Arial is the closest common width.
    ("Zurich", &["Arial", "Liberation Sans"]),
    // --- PostScript names that drawings carry from printers' font lists ---
    // Courier: the PostScript typewriter face (Windows has only bitmap "Courier").
    ("Courier", &["Courier New", "Liberation Mono"]),
    // Helvetica (narrow): Arial is metric-compatible.
    ("Helvetica", &["Arial", "Liberation Sans"]),
    ("HelveticaNarrow", &["Arial Narrow", "Liberation Sans Narrow"]),
    // Times: Times New Roman is metric-compatible.
    ("Times", &["Times New Roman", "Liberation Serif"]),
    // ITC Avant Garde Gothic, also as Bitstream's "AvantGarde Bk BT" / "Md BT" (the CorelDRAW 5
    // samples' 5EFFECTS, COLORBAR, SCREENS...).
    ("AvantGarde", &["Century Gothic", "URW Gothic"]),
    ("ITCAvantGardeGothic", &["Century Gothic", "URW Gothic"]),
    // ITC Bookman.
    ("Bookman", &["Bookman Old Style", "URW Bookman"]),
    ("ITCBookman", &["Bookman Old Style", "URW Bookman"]),
    // ITC Zapf Chancery.
    ("ZapfChancery", &["Monotype Corsiva", "URW Chancery L", "Z003"]),
    ("ITCZapfChancery", &["Monotype Corsiva", "URW Chancery L", "Z003"]),
    // Palatino.
    ("Palatino", &["Palatino Linotype", "Book Antiqua", "URW Palladio L"]),
    // New Century Schoolbook.
    ("NewCenturySchoolbook", &["Century Schoolbook", "C059"]),
];

/// Condensed families to try, best first, when a condensed name has no installed alias.
pub const CONDENSED: &[&str] = &["Arial Narrow", "Liberation Sans Narrow", "Roboto Condensed", "DejaVu Sans Condensed", "Noto Sans Condensed"];

/// Fonts that come with Windows (or Office) and that drawings name, and the open fonts that take
/// their place where they aren't installed, as on Linux, best first. The fonts made to the same
/// metrics come first: every character as wide as in the Windows font, so text keeps its line
/// breaks, widths and layout (Liberation Sans for Arial, Carlito for Calibri, Selawik for Segoe
/// UI...). Then, for fonts that have no such twin, the common look-alike nearest in width. Names
/// are compared as in [`ALIASES`]. Only used where Windows' fonts aren't the installed fonts (see
/// `FontDb::with_equivalents`); a Windows font that is installed is always used as it is.
pub const EQUIVALENTS: &[(&str, &[&str])] = &[
    // --- the same metrics (open "metric-compatible" fonts) ---
    // Liberation and the ChromeOS core fonts (Arimo, Tinos, Cousine) are made to Arial's, Times
    // New Roman's and Courier New's widths; URW's Nimbus to Helvetica's, Times' and Courier's,
    // which are the same.
    ("Arial", &["Liberation Sans", "Arimo", "Nimbus Sans", "Nimbus Sans L"]),
    ("Arial Narrow", &["Liberation Sans Narrow", "Nimbus Sans Narrow"]),
    ("Times New Roman", &["Liberation Serif", "Tinos", "Nimbus Roman", "Nimbus Roman No9 L"]),
    ("Courier New", &["Liberation Mono", "Cousine", "Nimbus Mono PS", "Nimbus Mono L"]),
    // Google's Crosextra fonts, made to the Office fonts' metrics.
    ("Calibri", &["Carlito"]),
    ("Cambria", &["Caladea"]),
    ("Georgia", &["Gelasio"]),
    // Microsoft's open fallback for Segoe UI, made to its metrics, in Segoe UI's weights.
    ("Segoe UI", &["Selawik"]),
    ("Segoe UI Light", &["Selawik Light"]),
    ("Segoe UI Semilight", &["Selawik Semilight"]),
    ("Segoe UI Semibold", &["Selawik Semibold"]),
    ("Comic Sans MS", &["Comic Relief", "Comic Neue"]),
    // Century Gothic and Book Antiqua were made to ITC Avant Garde's and Palatino's metrics, like
    // URW's Gothic and P052 (URW Palladio).
    ("Century Gothic", &["URW Gothic", "URW Gothic L", "TeX Gyre Adventor"]),
    ("Book Antiqua", &["P052", "URW Palladio L", "TeX Gyre Pagella"]),
    ("Palatino Linotype", &["P052", "URW Palladio L", "TeX Gyre Pagella"]),
    ("Bookman Old Style", &["URW Bookman", "URW Bookman L", "TeX Gyre Bonum"]),
    ("Century Schoolbook", &["C059", "Century Schoolbook L", "TeX Gyre Schola"]),
    ("Monotype Corsiva", &["Z003", "URW Chancery L", "TeX Gyre Chorus"]),
    // Lucida Console is as wide as DejaVu Sans Mono (0.6 em).
    ("Lucida Console", &["DejaVu Sans Mono", "Liberation Mono"]),
    // IPA's fonts are made to MS Gothic's and MS Mincho's widths.
    ("MS Gothic", &["IPAGothic", "Noto Sans Mono CJK JP", "Noto Sans CJK JP"]),
    ("MS PGothic", &["IPAPGothic", "Noto Sans CJK JP", "IPAGothic"]),
    ("MS Mincho", &["IPAMincho", "Noto Serif CJK JP", "IPAexMincho"]),
    ("MS PMincho", &["IPAPMincho", "Noto Serif CJK JP", "IPAMincho"]),
    // --- no open twin: the common look-alike nearest in width ---
    // Verdana and Tahoma are Matthew Carter's wide and narrow humanist sans; DejaVu Sans (from
    // Bitstream Vera) is the wide one's look-alike, its condensed face the narrow one's.
    ("Verdana", &["DejaVu Sans", "Bitstream Vera Sans", "Noto Sans"]),
    ("Tahoma", &["DejaVu Sans Condensed", "DejaVu Sans", "Noto Sans"]),
    ("Microsoft Sans Serif", &["Liberation Sans", "Arimo", "DejaVu Sans"]),
    ("Trebuchet MS", &["Fira Sans", "Noto Sans", "DejaVu Sans"]),
    ("Consolas", &["Inconsolata", "DejaVu Sans Mono", "Liberation Mono"]),
    ("Candara", &["Noto Sans", "DejaVu Sans"]),
    ("Corbel", &["Noto Sans", "DejaVu Sans"]),
    ("Constantia", &["Noto Serif", "DejaVu Serif"]),
    ("Garamond", &["EB Garamond", "Cormorant Garamond", "Noto Serif"]),
    ("Segoe UI Symbol", &["Noto Sans Symbols 2", "Noto Sans Symbols2", "DejaVu Sans", "Symbola"]),
    // outline faces only: a colour bitmap face (Noto Color Emoji) has no outlines to draw
    ("Segoe UI Emoji", &["Noto Emoji", "Symbola", "Noto Sans Symbols 2", "Noto Sans Symbols2"]),
    // East Asian interface fonts
    ("Yu Gothic", &["Noto Sans CJK JP", "Noto Sans JP", "Source Han Sans JP", "IPAexGothic"]),
    ("Yu Gothic UI", &["Noto Sans CJK JP", "Noto Sans JP", "Source Han Sans JP", "IPAexGothic"]),
    ("Meiryo", &["Noto Sans CJK JP", "Noto Sans JP", "Source Han Sans JP", "IPAexGothic"]),
    ("Yu Mincho", &["Noto Serif CJK JP", "Noto Serif JP", "IPAexMincho"]),
    ("Microsoft YaHei", &["Noto Sans CJK SC", "Noto Sans SC", "Source Han Sans SC", "WenQuanYi Micro Hei"]),
    ("Microsoft YaHei UI", &["Noto Sans CJK SC", "Noto Sans SC", "Source Han Sans SC", "WenQuanYi Micro Hei"]),
    ("SimHei", &["Noto Sans CJK SC", "WenQuanYi Zen Hei", "WenQuanYi Micro Hei"]),
    ("SimSun", &["Noto Serif CJK SC", "AR PL UMing CN", "AR PL SungtiL GB"]),
    ("NSimSun", &["Noto Serif CJK SC", "AR PL UMing CN"]),
    ("Microsoft JhengHei", &["Noto Sans CJK TC", "Noto Sans TC", "Source Han Sans TC", "WenQuanYi Zen Hei"]),
    ("Microsoft JhengHei UI", &["Noto Sans CJK TC", "Noto Sans TC", "Source Han Sans TC", "WenQuanYi Zen Hei"]),
    ("PMingLiU", &["Noto Serif CJK TC", "AR PL UMing TW", "AR PL New Sung"]),
    ("MingLiU", &["Noto Serif CJK TC", "AR PL UMing TW", "AR PL New Sung"]),
    ("Malgun Gothic", &["Noto Sans CJK KR", "Noto Sans KR", "NanumGothic", "UnDotum"]),
    ("Gulim", &["Noto Sans CJK KR", "NanumGothic", "UnDotum"]),
    ("Batang", &["Noto Serif CJK KR", "NanumMyeongjo", "UnBatang"]),
];

/// Vendor and weight words that trail a font name and don't change which typeface it is, so
/// "AvantGarde Bk BT" finds the "AvantGarde" entry.
const TRAILING: &[&str] = &["bt", "mt", "itc", "bk", "md", "book", "medium", "roman", "regular", "normal", "light", "lt", "demi", "bold", "italic"];

/// The name without case, spaces or punctuation.
pub fn squeeze(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// A font name as Windows lists it: without the spaces or NULs a file may pad it with, and
/// without the "@" that names the vertical form of an East Asian font ("@MS Gothic").
pub fn undecorated(name: &str) -> &str {
    fn t(s: &str) -> &str {
        s.trim_matches(|c: char| c.is_whitespace() || c == '\0')
    }
    let s = t(name);
    s.strip_prefix('@').map(t).unwrap_or(s)
}

/// A font name without the script that old font lists add to it ("Arial Cyr", "Times New Roman
/// CE": Windows 3.1 and 9x listed a font once per character set); the name itself when it ends
/// in none.
pub fn without_script(name: &str) -> &str {
    const SCRIPTS: [&str; 10] = ["cyr", "cyrillic", "ce", "baltic", "greek", "tur", "turkish", "hebrew", "arabic", "thai"];
    match name.rsplit_once(' ') {
        Some((base, last)) if !base.trim().is_empty() && SCRIPTS.contains(&last.to_lowercase().as_str()) => base.trim_end(),
        _ => name,
    }
}

/// The look-alikes of a font that isn't installed, best first (empty when it has none).
pub fn aliases(name: &str) -> &'static [&'static str] {
    lookup(ALIASES, name)
}

/// The open fonts that take the place of a Windows font that isn't installed, best first, from
/// [`EQUIVALENTS`] (empty when it has none). The name is found as Windows lists it too: padded,
/// with an old font list's script ("Arial Cyr"), or with vendor and weight words after it.
pub fn equivalents(name: &str) -> &'static [&'static str] {
    let plain = undecorated(name);
    [plain, without_script(plain)].into_iter().map(|n| lookup(EQUIVALENTS, n)).find(|s| !s.is_empty()).unwrap_or(&[])
}

/// The entry of `table` for `name`: by the name without case, spaces or punctuation, then with
/// trailing vendor and weight words dropped one at a time.
fn lookup(table: &'static [(&'static str, &'static [&'static str])], name: &str) -> &'static [&'static str] {
    let find = |key: &str| table.iter().find(|(n, _)| squeeze(n) == key).map(|(_, s)| *s);
    if let Some(s) = find(&squeeze(name)) {
        return s;
    }
    // drop trailing vendor and weight words one at a time
    let mut words: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    while words.len() > 1 && TRAILING.contains(&words[words.len() - 1].to_lowercase().as_str()) {
        words.pop();
        if let Some(s) = find(&squeeze(&words.concat())) {
            return s;
        }
    }
    &[]
}

/// How wide a font's name says it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    Condensed,
    Normal,
    Expanded,
}

/// The width a font name asks for: "Narrow", "Condensed", "Compressed" (or the short "Cond",
/// "Cn") make it condensed; "Extended", "Expanded" or "Wide" make it expanded.
pub fn width_hint(name: &str) -> Width {
    let s = squeeze(name);
    let words: Vec<String> = name.split(|c: char| !c.is_alphanumeric()).map(|w| w.to_lowercase()).collect();
    if ["narrow", "condensed", "compressed", "condblack", "condlight"].iter().any(|w| s.contains(w)) || words.iter().any(|w| matches!(w.as_str(), "cond" | "cn" | "bkcn" | "comp")) {
        Width::Condensed
    } else if ["extended", "expanded", "wide"].iter().any(|w| s.contains(w)) || words.iter().any(|w| matches!(w.as_str(), "ext" | "ex")) {
        Width::Expanded
    } else {
        Width::Normal
    }
}

/// How a family was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    /// It is installed (possibly with different capitals).
    Installed,
    /// A look-alike from [`ALIASES`].
    Alias,
    /// An installed face of the width its name asks for.
    Width,
    /// The generic sans-serif face.
    Fallback,
}

/// The installed family that shows a font, and how it was chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The family to ask the font database for.
    pub family: String,
    /// The font's name as the user knows it ("Arial Narrow", where `family` is "Arial").
    pub name: String,
    /// The width of the family to use (condensed for "Arial Narrow").
    pub stretch: fontdb::Stretch,
    /// The weight, for a font listed under a weight of its own ("Segoe UI Semibold").
    pub weight: Option<fontdb::Weight>,
    pub how: How,
}

impl Resolved {
    /// The family stands in for another one.
    pub fn substituted(&self) -> bool {
        self.how != How::Installed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_without_spaces_case_or_vendor_words() {
        assert_eq!(aliases("Switzerland Narrow")[0], "Arial Narrow");
        assert_eq!(aliases("SWITZERLANDNARROW")[0], "Arial Narrow");
        assert_eq!(aliases("AvantGarde Bk BT")[0], "Century Gothic");
        assert_eq!(aliases("AvantGarde Md BT")[0], "Century Gothic");
        assert_eq!(aliases("Bedrock")[0], "Arial Narrow");
        assert!(aliases("Some Font Nobody Has").is_empty());
        // a trailing word only goes when what is left is a known name
        assert!(aliases("Bold").is_empty());
    }

    #[test]
    fn the_table_has_no_duplicates() {
        for table in [ALIASES, EQUIVALENTS] {
            let mut keys: Vec<String> = table.iter().map(|(n, _)| squeeze(n)).collect();
            let n = keys.len();
            keys.sort();
            keys.dedup();
            assert_eq!(keys.len(), n);
            assert!(table.iter().all(|(_, s)| !s.is_empty()));
            // a font never stands in for itself
            assert!(table.iter().all(|(n, s)| !s.iter().any(|x| squeeze(x) == squeeze(n))), "{table:?}");
        }
        // a name is either a Windows font with open equivalents or a font Windows doesn't have
        for (n, _) in EQUIVALENTS {
            assert!(aliases(n).is_empty(), "{n} is in both tables");
        }
        for (n, _) in ALIASES {
            assert!(equivalents(n).is_empty(), "{n} is in both tables");
        }
    }

    #[test]
    fn windows_fonts_have_their_metric_twins_first() {
        assert_eq!(equivalents("Arial")[0], "Liberation Sans");
        assert_eq!(equivalents("Times New Roman")[0], "Liberation Serif");
        assert_eq!(equivalents("Courier New")[0], "Liberation Mono");
        assert_eq!(equivalents("Arial Narrow")[0], "Liberation Sans Narrow");
        assert_eq!(equivalents("Calibri"), &["Carlito"]);
        assert_eq!(equivalents("Cambria"), &["Caladea"]);
        assert_eq!(equivalents("Segoe UI"), &["Selawik"]);
        assert_eq!(equivalents("Segoe UI Semibold"), &["Selawik Semibold"]);
        assert_eq!(equivalents("Verdana")[0], "DejaVu Sans");
        assert_eq!(equivalents("Tahoma")[0], "DejaVu Sans Condensed");
        // found as Windows lists them: capitals, padding, an old list's script, vendor words
        assert_eq!(equivalents("ARIAL")[0], "Liberation Sans");
        assert_eq!(equivalents(" Arial\0"), equivalents("Arial"));
        assert_eq!(equivalents("Arial Cyr"), equivalents("Arial"));
        assert_eq!(equivalents("Times New Roman CE"), equivalents("Times New Roman"));
        assert_eq!(equivalents("@MS Gothic")[0], "IPAGothic");
        assert_eq!(equivalents("Calibri Bold"), &["Carlito"]);
        // fonts with no entry, and fonts Windows doesn't have
        assert!(equivalents("Some Font Nobody Has").is_empty());
        assert!(equivalents("Switzerland").is_empty());
    }

    /// Selawik, bundled with the app: a real font to give made-up family names.
    const FONT: &[u8] = include_bytes!("../../anyvector/assets/fonts/selawk.ttf");

    /// Fonts that go by `families` (each is Selawik underneath), with `sans` the sans-serif family.
    fn made_up(families: &[&str], sans: &str, equivalents: bool) -> crate::text::FontDb {
        let mut db = fontdb::Database::new();
        let data: std::sync::Arc<dyn AsRef<[u8]> + Send + Sync> = std::sync::Arc::new(FONT.to_vec());
        for name in families {
            for id in db.load_font_source(fontdb::Source::Binary(data.clone())) {
                let mut face = db.face(id).unwrap().clone();
                db.remove_face(id);
                face.families = vec![(name.to_string(), fontdb::Language::English_UnitedStates)];
                face.post_script_name = squeeze(name);
                db.push_face_info(face);
            }
        }
        db.set_sans_serif_family(sans);
        crate::text::FontDb::with_equivalents(db, equivalents)
    }

    #[test]
    fn windows_fonts_show_in_their_open_equivalents_off_windows() {
        let fonts = ["Liberation Sans", "Liberation Serif", "Liberation Mono", "Carlito", "DejaVu Sans", "DejaVu Sans Condensed"];
        let db = made_up(&fonts, "DejaVu Sans", true);
        let shown = |name: &str| {
            let r = db.resolve(name);
            (r.family, r.how)
        };
        for (name, used) in [
            ("Arial", "Liberation Sans"),
            ("Times New Roman", "Liberation Serif"),
            ("Courier New", "Liberation Mono"),
            ("Calibri", "Carlito"),
            ("Verdana", "DejaVu Sans"),
            ("Tahoma", "DejaVu Sans Condensed"),
            // as Windows lists them
            ("arial", "Liberation Sans"),
            ("Arial Cyr", "Liberation Sans"),
            ("Times New Roman CE", "Liberation Serif"),
            // an old CorelDRAW name whose look-alike is a Windows font
            ("Dutch", "Liberation Serif"),
            ("Monospaced", "Liberation Mono"),
        ] {
            assert_eq!(shown(name), (used.to_string(), How::Alias), "{name}");
            assert_eq!(db.substitute(name).as_deref(), Some(used), "{name}");
        }
        // no equivalent installed (Caladea for Cambria): the sans-serif family
        assert_eq!(shown("Cambria"), ("DejaVu Sans".to_string(), How::Fallback));
        // installed fonts are themselves
        assert_eq!(shown("Liberation Sans"), ("Liberation Sans".to_string(), How::Installed));
        assert!(db.query("Arial", true, false).is_some());
    }

    #[test]
    fn windows_keeps_its_substitutions_without_equivalents() {
        // as on Windows: Arial missing (it never is there) falls back to the sans-serif family;
        // the look-alikes of ALIASES are the same either way
        let db = made_up(&["Liberation Sans", "Liberation Serif", "DejaVu Sans"], "DejaVu Sans", false);
        assert_eq!(db.resolve("Arial").family, "DejaVu Sans");
        assert_eq!(db.resolve("Arial").how, How::Fallback);
        assert_eq!(db.resolve("Times New Roman").how, How::Fallback);
        assert_eq!((db.resolve("Dutch").family.as_str(), db.resolve("Dutch").how), ("Liberation Serif", How::Alias));
        assert_eq!((db.resolve("Switzerland").family.as_str(), db.resolve("Switzerland").how), ("Liberation Sans", How::Alias));
    }

    /// The installed fonts: on Linux, with Liberation (most distributions install it), Arial,
    /// Times New Roman and Courier New show in it; Calibri and Cambria in Carlito and Caladea
    /// where those are installed. On Windows they are all installed.
    #[test]
    fn missing_windows_fonts_show_in_installed_equivalents() {
        let db = crate::text::fonts().lock();
        for (name, used) in [("Arial", "Liberation Sans"), ("Times New Roman", "Liberation Serif"), ("Courier New", "Liberation Mono"), ("Calibri", "Carlito"), ("Cambria", "Caladea")] {
            if db.resolve(name).how == How::Installed || db.resolve(used).how != How::Installed {
                eprintln!("{name} is installed or {used} isn't; skipped");
                continue;
            }
            assert_eq!(db.substitute(name).as_deref(), Some(used));
        }
        // a font with no stand-in shows in an installed face, never in whichever came first
        let r = db.resolve("No Such Font 12345");
        if r.how == How::Fallback && !db.families().is_empty() {
            assert!(db.families().contains(&r.family), "{r:?}");
            assert_eq!(r.family, db.db.family_name(&fontdb::Family::SansSerif));
        }
    }

    #[test]
    fn width_from_the_name() {
        assert_eq!(width_hint("Arial Narrow"), Width::Condensed);
        assert_eq!(width_hint("SwitzerlandCondensed"), Width::Condensed);
        assert_eq!(width_hint("Univers 57 Cond"), Width::Condensed);
        assert_eq!(width_hint("Futura Md Cn BT"), Width::Condensed);
        assert_eq!(width_hint("Helvetica Extended"), Width::Expanded);
        assert_eq!(width_hint("Bedrock"), Width::Normal);
        assert_eq!(width_hint("Second"), Width::Normal);
        assert_eq!(width_hint("Arial"), Width::Normal);
    }

    /// Whether the installed fonts have a condensed upright face.
    fn has_condensed(db: &crate::text::FontDb) -> bool {
        db.db.faces().any(|f| f.stretch < fontdb::Stretch::Normal && f.style == fontdb::Style::Normal)
    }

    /// Whether a resolved font is condensed: by its width, or by its name when the font's
    /// own width class says normal (Roboto Condensed's does).
    fn is_condensed(r: &Resolved) -> bool {
        r.stretch < fontdb::Stretch::Normal || width_hint(&r.name) == Width::Condensed
    }

    #[test]
    fn installed_fonts_are_never_substituted() {
        let db = crate::text::fonts().lock();
        for family in db.families().iter().take(200) {
            let r = db.resolve(family);
            assert_eq!((r.family.as_str(), r.how), (family.as_str(), How::Installed));
            assert_eq!(db.substitute(family), None);
            // other capitals are the same font
            assert_eq!(db.resolve(&family.to_uppercase()).how, How::Installed, "{family}");
        }
    }

    #[test]
    fn names_lose_padding_vertical_marks_and_scripts() {
        assert_eq!(undecorated(" Arial\0\0"), "Arial");
        assert_eq!(undecorated("@MS Gothic"), "MS Gothic");
        assert_eq!(undecorated("Futura PT Book"), "Futura PT Book");
        assert_eq!(without_script("Arial Cyr"), "Arial");
        assert_eq!(without_script("Times New Roman CYR"), "Times New Roman");
        assert_eq!(without_script("Arial CE"), "Arial");
        assert_eq!(without_script("Courier New Baltic"), "Courier New");
        assert_eq!(without_script("Arial"), "Arial");
        assert_eq!(without_script("Cyr"), "Cyr");
        assert_eq!(without_script("Futura PT Book"), "Futura PT Book");
    }

    /// The family names (name id 1, every language) and full names (id 4) of a face.
    fn names_of(db: &crate::text::FontDb, id: fontdb::ID) -> Vec<(u16, String)> {
        db.db
            .with_face_data(id, |data, index| {
                let face = ttf_parser::Face::parse(data, index).ok()?;
                Some(face.names().into_iter().filter(|n| n.is_unicode() && matches!(n.name_id, 1 | 4)).filter_map(|n| Some((n.name_id, n.to_string()?))).collect::<Vec<_>>())
            })
            .flatten()
            .unwrap_or_default()
    }

    /// Issue #64: CorelDRAW writes the family name Windows lists (name id 1), which for many
    /// fonts isn't the family fontdb files them under (the typographic family, id 16):
    /// "Futura PT Book" is "Futura PT", "Sitka Text" is "Sitka", and a font can have a name in
    /// the user's language. Every such name of an installed face is that font, never a stand-in.
    #[test]
    fn every_name_an_installed_face_goes_by_is_installed() {
        let db = crate::text::fonts().lock();
        for f in db.db.faces() {
            for (id, name) in names_of(&db, f.id) {
                let name = undecorated(&name);
                if name.is_empty() {
                    continue;
                }
                assert_eq!(db.substitute(name), None, "{name} (name id {id}) of {:?}", f.families);
                // a family name of older programs shows one of the faces that go by it
                let r = db.resolve(name);
                if id == 1 && r.name != r.family {
                    let shown = db.query(name, false, false).unwrap();
                    assert!(names_of(&db, shown).iter().any(|(i, n)| *i == 1 && undecorated(n).eq_ignore_ascii_case(name)), "{name} -> {:?}", db.db.face(shown).map(|f| &f.post_script_name));
                }
            }
        }
    }

    #[test]
    fn odd_names_of_installed_fonts_are_found() {
        let db = crate::text::fonts().lock();
        let ps = |name: &str, bold: bool, italic: bool| db.query(name, bold, italic).and_then(|id| db.db.face(id)).map(|f| f.post_script_name.clone()).unwrap_or_default();
        if db.resolve("Arial").how == How::Installed {
            for name in ["Arial Cyr", "Arial CE", "@Arial", "Arial\0", " Arial ", "Arial Bold", "Arial-BoldMT"] {
                assert_eq!(db.substitute(name), None, "{name}");
            }
            assert_eq!(ps("Arial Cyr", false, false), ps("Arial", false, false));
            // a face's own name shows that face, and its slanted form when italic
            assert_eq!(ps("Arial Bold", false, false), ps("Arial", true, false));
            assert_eq!(ps("Arial Bold", false, true), ps("Arial", true, true));
        }
        // Windows 11's Sitka: fontdb calls it "Sitka", Windows and CorelDRAW "Sitka Text"
        if db.families().iter().any(|f| f == "Sitka") && db.db.faces().any(|f| names_of(&db, f.id).contains(&(1, "Sitka Text".to_string()))) {
            assert_eq!(db.substitute("Sitka Text"), None);
            assert!(ps("Sitka Text", false, false).starts_with("SitkaText"));
            assert!(ps("Sitka Text", false, true).contains("Italic"));
        }
        // bold of a name listed under a weight of its own is the family's bold
        if db.families().iter().any(|f| f == "Segoe UI") && db.resolve("Segoe UI Semibold").how == How::Installed {
            assert_eq!(ps("Segoe UI Semibold", true, false), ps("Segoe UI", true, false));
        }
    }

    #[test]
    fn a_condensed_missing_font_gets_a_condensed_substitute() {
        let db = crate::text::fonts().lock();
        if !has_condensed(&db) {
            eprintln!("no condensed font installed; skipped");
            return;
        }
        for name in ["Bedrock", "Imaginary Sans Condensed", "Nonexistent Narrow"] {
            if db.resolve(name).how == How::Installed {
                continue;
            }
            let r = db.resolve(name);
            assert!(r.substituted() && is_condensed(&r), "{name} -> {r:?}");
        }
        // Arial Narrow is the first choice wherever it is installed
        if db.resolve("Arial Narrow").how == How::Installed {
            assert_eq!(db.substitute("Bedrock").as_deref(), Some("Arial Narrow"));
            assert_eq!(db.substitute("Switzerland Narrow").as_deref(), Some("Arial Narrow"));
        }
    }

    #[test]
    fn aliases_are_used_when_installed_and_skipped_when_not() {
        let db = crate::text::fonts().lock();
        if db.families().iter().any(|f| f == "Times New Roman") {
            let r = db.resolve("Dutch");
            assert_eq!((r.family.as_str(), r.how), ("Times New Roman", How::Alias));
        }
        // a missing font of normal width with no alias falls back to the sans-serif face
        let r = db.resolve("No Such Font 12345");
        assert_eq!(r.how, How::Fallback);
        assert!(db.query("No Such Font 12345", false, false).is_some());
    }

    #[test]
    fn a_width_of_a_family_is_found_by_its_windows_name() {
        // fontdb files Arial Narrow under "Arial", condensed; drawings call it "Arial Narrow"
        let db = crate::text::fonts().lock();
        let r = db.resolve("Arial Narrow");
        if r.how != How::Installed {
            eprintln!("Arial Narrow isn't installed; skipped");
            return;
        }
        assert!(r.stretch < fontdb::Stretch::Normal, "{r:?}");
        for bold in [false, true] {
            let face = db.query("Arial Narrow", bold, false).and_then(|id| db.db.face(id)).unwrap();
            assert!(face.stretch < fontdb::Stretch::Normal);
            assert_eq!(face.weight.0 >= 700, bold);
        }
        // plain Arial stays the regular width
        let face = db.query("Arial", false, false).and_then(|id| db.db.face(id)).unwrap();
        assert_eq!(face.stretch, fontdb::Stretch::Normal);
    }

    #[test]
    fn the_resolver_is_deterministic() {
        // a database of its own, so nothing is cached yet
        let fdb = crate::text::system_fonts().clone();
        let a = crate::text::FontDb::new(fdb.clone());
        let b = crate::text::FontDb::new(fdb);
        for name in ["Bedrock", "Dutch", "AvantGarde Md BT", "Imaginary Sans Condensed", "Wide Extended", "No Such Font 12345", "Arial"] {
            let ra = a.resolve(name);
            assert_eq!(ra, b.resolve(name), "{name}");
            assert_eq!(ra, a.resolve(name), "{name}");
        }
    }
}
