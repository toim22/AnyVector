//! Font substitution: which installed family shows text set in a family that isn't installed.
//!
//! The order is: the family itself (ignoring case) → a known look-alike from [`ALIASES`] that is
//! installed → an installed face of the same width class (a "Narrow" name gets a condensed face)
//! → the generic sans-serif face. `FontDb::resolve` in `text.rs` runs it; this module holds the
//! data and the name rules, so they can be tested without fonts.

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

/// Vendor and weight words that trail a font name and don't change which typeface it is, so
/// "AvantGarde Bk BT" finds the "AvantGarde" entry.
const TRAILING: &[&str] = &["bt", "mt", "itc", "bk", "md", "book", "medium", "roman", "regular", "normal", "light", "lt", "demi", "bold", "italic"];

/// The name without case, spaces or punctuation.
pub fn squeeze(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// The look-alikes of a font that isn't installed, best first (empty when it has none).
pub fn aliases(name: &str) -> &'static [&'static str] {
    let find = |key: &str| ALIASES.iter().find(|(n, _)| squeeze(n) == key).map(|(_, s)| *s);
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
        let mut keys: Vec<String> = ALIASES.iter().map(|(n, _)| squeeze(n)).collect();
        let n = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), n);
        assert!(ALIASES.iter().all(|(_, s)| !s.is_empty()));
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
        let mut fdb = fontdb::Database::new();
        fdb.load_system_fonts();
        let a = crate::text::FontDb::new(fdb.clone());
        let b = crate::text::FontDb::new(fdb);
        for name in ["Bedrock", "Dutch", "AvantGarde Md BT", "Imaginary Sans Condensed", "Wide Extended", "No Such Font 12345", "Arial"] {
            let ra = a.resolve(name);
            assert_eq!(ra, b.resolve(name), "{name}");
            assert_eq!(ra, a.resolve(name), "{name}");
        }
    }
}
