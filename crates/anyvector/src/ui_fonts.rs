//! The interface's fonts where Windows' fonts aren't installed (Linux and the other free systems).
//!
//! On Windows `theme::install_fonts` reads Segoe UI, Consolas and Segoe UI Symbol from the fonts
//! folder. Elsewhere the same families are made from the installed fonts
//! (`vector_core::text::system_fonts`), each with fallbacks, so no family is ever empty, even
//! with no fonts installed at all:
//! - the interface face: Segoe UI when it is installed, else Selawik, bundled: Microsoft's open
//!   font (SIL Open Font License 1.1) made to Segoe UI's metrics, so labels, fields and dialogs
//!   measure as on Windows. Its bold and semilight faces make the "bold" and "light" families;
//! - an installed sans for what Selawik lacks (Cyrillic, Greek, №...): Noto Sans, DejaVu Sans...;
//! - egui's own fonts, as on Windows;
//! - every installed symbol font of a list, for the arrows, check marks and shapes Segoe UI
//!   Symbol has on Windows: DejaVu Sans, Noto Sans Symbols 2, Noto Sans Math...;
//! - monospace: Consolas when installed, else its look-alikes (Inconsolata, DejaVu Sans Mono...);
//! - Chinese and Japanese: the font of the interface language, as on Windows added by
//!   `i18n::add_fonts` (see `cjk_families`).
//!
//! Icons with text in them are drawn with Segoe UI and Times New Roman (`icons::usvg_opts`); here
//! those names stand for the installed fonts or their stand-ins (see `icon_fonts`).
//!
//! The Windows build has none of this (nor the bundled fonts); its tests build it, so the choices
//! are tested on Windows too.

use egui::FontFamily;
use std::sync::Arc;
use usvg::fontdb;
use vector_core::fontsub;

const REGULAR: u16 = 400;
const BOLD: u16 = 700;
/// Segoe UI Semilight, the "light" family on Windows.
const SEMILIGHT: u16 = 350;

/// Installed fonts for what Selawik lacks (Cyrillic, Greek, №...), best first: the first one
/// installed is used.
const TEXT: &[&str] = &["Noto Sans", "DejaVu Sans", "Liberation Sans", "Ubuntu", "Cantarell", "FreeSans"];

/// Installed fonts with the symbols the interface uses (arrows, check marks, shapes, ⟳ ⊕ ⏷ 📋 🔒),
/// which Segoe UI Symbol has on Windows: every one installed, in this order. "Noto Sans Symbols2"
/// is the older name of "Noto Sans Symbols 2" (Debian and Ubuntu still use it).
const SYMBOLS: &[&str] = &["Segoe UI Symbol", "DejaVu Sans", "Noto Sans Symbols 2", "Noto Sans Symbols2", "Noto Sans Symbols", "Noto Sans Math", "Symbola"];

/// Installed fonts with Chinese characters and kana, after the font of the interface language and
/// its equivalents (see `cjk_families`).
const CJK: &[&str] = &[
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Noto Sans CJK TC",
    "Noto Sans CJK HK",
    "Noto Sans CJK KR",
    "Source Han Sans SC",
    "Source Han Sans JP",
    "Source Han Sans TC",
    "Source Han Sans",
    "WenQuanYi Micro Hei",
    "WenQuanYi Zen Hei",
    "Droid Sans Fallback",
    "IPAexGothic",
    "IPAGothic",
    "IPAPGothic",
    "TakaoPGothic",
    "VL PGothic",
    "AR PL UMing CN",
    "AR PL UMing TW",
];

/// The bundled Selawik faces: the weights the interface uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Selawik {
    Regular,
    Bold,
    Semilight,
}

impl Selawik {
    /// The font file (from Selawik 1.01, https://github.com/microsoft/Selawik).
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Selawik::Regular => include_bytes!("../assets/fonts/selawk.ttf"),
            Selawik::Bold => include_bytes!("../assets/fonts/selawkb.ttf"),
            Selawik::Semilight => include_bytes!("../assets/fonts/selawksl.ttf"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Selawik::Regular => "Selawik",
            Selawik::Bold => "Selawik Bold",
            Selawik::Semilight => "Selawik Semilight",
        }
    }
}

/// Where a font of the interface comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face<K> {
    Bundled(Selawik),
    /// An installed face.
    Installed(K),
}

/// The interface's fonts: each by its name in the definitions, and each family's fonts by those
/// names, best first (egui's own fonts by theirs).
#[derive(Debug, PartialEq)]
pub struct Plan<K> {
    pub fonts: Vec<(String, Face<K>)>,
    pub families: Vec<(FontFamily, Vec<String>)>,
}

/// The name of `face`, adding it under `name` the first time.
fn add<K: PartialEq>(fonts: &mut Vec<(String, Face<K>)>, name: String, face: Face<K>) -> String {
    match fonts.iter().find(|(_, f)| *f == face) {
        Some((n, _)) => n.clone(),
        None => {
            fonts.push((name.clone(), face));
            name
        }
    }
}

/// The interface's fonts (see the module notes). `find(family, weight)` is the installed upright
/// face of `family` of about that weight, if there is one.
pub fn plan<K: Copy + PartialEq>(find: impl Fn(&str, u16) -> Option<K>) -> Plan<K> {
    let mut fonts = vec![];
    let installed = |fonts: &mut Vec<(String, Face<K>)>, family: &str, weight: u16| find(family, weight).map(|k| add(fonts, format!("{family} {weight}"), Face::Installed(k)));
    let bundled = |fonts: &mut Vec<(String, Face<K>)>, s: Selawik| add(fonts, s.name().into(), Face::Bundled(s));
    // the interface face, with its bold and semilight faces (a missing one is the regular face,
    // as on Windows)
    let (regular, bold, light) = match installed(&mut fonts, "Segoe UI", REGULAR) {
        Some(r) => (r, installed(&mut fonts, "Segoe UI", BOLD), installed(&mut fonts, "Segoe UI", SEMILIGHT)),
        None => (bundled(&mut fonts, Selawik::Regular), Some(bundled(&mut fonts, Selawik::Bold)), Some(bundled(&mut fonts, Selawik::Semilight))),
    };
    let text = TEXT.iter().find_map(|&f| installed(&mut fonts, f, REGULAR).map(|n| (f, n)));
    let text_bold = text.as_ref().and_then(|&(f, _)| installed(&mut fonts, f, BOLD));
    let text = text.map(|(_, n)| n);
    let symbols: Vec<String> = SYMBOLS.iter().filter_map(|&f| installed(&mut fonts, f, REGULAR)).collect();
    let mono = std::iter::once("Consolas").chain(fontsub::equivalents("Consolas").iter().copied()).chain(["Noto Sans Mono", "Ubuntu Mono"]).find_map(|f| installed(&mut fonts, f, REGULAR));

    // egui's own fonts, then the symbols
    let tail: Vec<String> = ["Ubuntu-Light", "egui-icons"].into_iter().map(String::from).chain(symbols).collect();
    let family = |head: Vec<Option<String>>, tail: &[String]| {
        let mut out: Vec<String> = vec![];
        for n in head.into_iter().flatten().chain(tail.iter().cloned()) {
            if !out.contains(&n) {
                out.push(n);
            }
        }
        out
    };
    let families = vec![
        (FontFamily::Proportional, family(vec![Some(regular.clone()), text.clone()], &tail)),
        // egui's Hack, and the interface face last, so monospace is never empty
        (FontFamily::Monospace, family(vec![mono, Some("Hack".into())], &[tail.clone(), vec![regular.clone()]].concat())),
        (FontFamily::Name("bold".into()), family(vec![bold, Some(regular.clone()), text_bold, text.clone()], &tail)),
        (FontFamily::Name("light".into()), family(vec![light, Some(regular), text], &tail)),
    ];
    Plan { fonts, families }
}

/// The definitions of `plan`: egui's own fonts, and the plan's fonts as `load` reads them. A
/// font that can't be read, and one of egui's that isn't there, are left out of the families.
pub fn build<K>(plan: &Plan<K>, load: impl Fn(&Face<K>) -> Option<Arc<egui::FontData>>) -> egui::FontDefinitions {
    let mut defs = egui::FontDefinitions::default();
    for (name, face) in &plan.fonts {
        if let Some(data) = load(face) {
            defs.font_data.insert(name.clone(), data);
        }
    }
    for (family, names) in &plan.families {
        let names = names.iter().filter(|n| defs.font_data.contains_key(*n)).cloned().collect();
        defs.families.insert(family.clone(), names);
    }
    defs
}

/// The installed fonts with Chinese characters and kana for the interface in `lang`, best first:
/// the font Windows uses (Yu Gothic UI for Japanese, Microsoft JhengHei UI for traditional
/// Chinese, else Microsoft YaHei UI, so the language list shows 中文 and 日本語), its open
/// equivalents (`fontsub::EQUIVALENTS`: Noto Sans CJK...), then any other.
pub fn cjk_families(lang: crate::i18n::Lang) -> Vec<&'static str> {
    use crate::i18n::Lang;
    let windows = match lang {
        Lang::JaJp => "Yu Gothic UI",
        Lang::ZhTw => "Microsoft JhengHei UI",
        _ => "Microsoft YaHei UI",
    };
    let mut out = vec![windows];
    for f in fontsub::equivalents(windows).iter().chain(CJK) {
        if !out.contains(f) {
            out.push(f);
        }
    }
    out
}

/// The installed upright, normal-width face of `family` (by any of its names, ignoring case)
/// nearest to `weight`, within 100 (the heavier of two as near).
fn find(db: &fontdb::Database, family: &str, weight: u16) -> Option<fontdb::ID> {
    db.faces()
        .filter(|f| f.style == fontdb::Style::Normal && f.stretch == fontdb::Stretch::Normal)
        .filter(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(family)))
        .map(|f| ((f.weight.0 as i32 - weight as i32).abs(), std::cmp::Reverse(f.weight.0), f.id))
        .filter(|(d, ..)| *d <= 100)
        .min()
        .map(|(.., id)| id)
}

/// The font data of a face. Installed faces are read once: the families are made again when the
/// language changes.
fn font_data(db: &fontdb::Database, face: &Face<fontdb::ID>) -> Option<Arc<egui::FontData>> {
    static READ: std::sync::Mutex<std::collections::BTreeMap<fontdb::ID, Arc<egui::FontData>>> = std::sync::Mutex::new(std::collections::BTreeMap::new());
    let id = match *face {
        Face::Bundled(s) => return Some(Arc::new(egui::FontData::from_static(s.bytes()))),
        Face::Installed(id) => id,
    };
    let mut read = READ.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(d) = read.get(&id) {
        return Some(d.clone());
    }
    let (source, index) = db.face_source(id)?;
    let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = match source {
        fontdb::Source::Binary(b) | fontdb::Source::SharedFile(_, b) => b,
        fontdb::Source::File(p) => Arc::new(std::fs::read(p).ok()?),
    };
    let d = Arc::new(egui::FontData::from_blob(bytes, index));
    read.insert(id, d.clone());
    Some(d)
}

/// The interface's fonts from the installed ones (see the module notes), with the Chinese or
/// Japanese font of the interface language (`i18n::add_fonts`).
pub fn definitions() -> egui::FontDefinitions {
    let db = vector_core::text::system_fonts();
    let mut defs = build(&plan(|family, weight| find(db, family, weight)), |face| font_data(db, face));
    crate::i18n::add_fonts(&mut defs, |lang, _| {
        let id = cjk_families(lang).into_iter().find_map(|f| find(db, f, REGULAR))?;
        font_data(db, &Face::Installed(id)).map(|d| (*d).clone())
    });
    defs
}

/// The faces of a bundled font.
fn bundled_faces(s: Selawik) -> Vec<fontdb::FaceInfo> {
    let mut db = fontdb::Database::new();
    db.load_font_source(fontdb::Source::Binary(Arc::new(s.bytes())));
    db.faces().cloned().collect()
}

/// The fonts icons are drawn with (`icons::usvg_opts`), into `db`: Segoe UI (regular and bold)
/// and Times New Roman, by those names, as Windows has them: the installed ones, else Selawik and
/// the installed serif made to Times New Roman's metrics (Liberation Serif), else any installed
/// serif. Sans-serif is Segoe UI and serif Times New Roman, as on Windows (where a name that isn't
/// one of them, as the icons' Arial, is drawn in Times New Roman).
pub fn icon_fonts(db: &mut fontdb::Database) {
    let sys = vector_core::text::system_fonts();
    let mut add = |face: fontdb::FaceInfo, family: &str| {
        let mut face = face;
        face.families = vec![(family.to_string(), fontdb::Language::English_UnitedStates)];
        db.push_face_info(face);
    };
    if find(sys, "Segoe UI", REGULAR).is_some() {
        for w in [REGULAR, BOLD] {
            if let Some(f) = find(sys, "Segoe UI", w).and_then(|id| sys.face(id)) {
                add(f.clone(), "Segoe UI");
            }
        }
    } else {
        for f in [Selawik::Regular, Selawik::Bold].into_iter().flat_map(bundled_faces) {
            add(f, "Segoe UI");
        }
    }
    let serif = std::iter::once("Times New Roman").chain(fontsub::equivalents("Times New Roman").iter().copied()).chain([sys.family_name(&fontdb::Family::Serif)]).find_map(|f| find(sys, f, REGULAR));
    match serif.and_then(|id| sys.face(id)) {
        Some(f) => add(f.clone(), "Times New Roman"),
        // no font installed at all: text in icons still shows
        None => bundled_faces(Selawik::Regular).into_iter().for_each(|f| add(f, "Times New Roman")),
    }
    db.set_sans_serif_family("Segoe UI");
    db.set_serif_family("Times New Roman");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    /// A made-up set of installed fonts: (family, weights).
    fn installed(list: &'static [(&'static str, &'static [u16])]) -> impl Fn(&str, u16) -> Option<(&'static str, u16)> {
        move |family, weight| {
            let (name, weights) = list.iter().find(|(n, _)| n.eq_ignore_ascii_case(family))?;
            weights.iter().map(|w| ((*w as i32 - weight as i32).abs(), std::cmp::Reverse(*w))).filter(|(d, _)| *d <= 100).min().map(|(_, w)| (*name, w.0))
        }
    }

    fn family<'a, K>(p: &'a Plan<K>, f: &FontFamily) -> Vec<&'a str> {
        p.families.iter().find(|(g, _)| g == f).map(|(_, n)| n.iter().map(String::as_str).collect()).unwrap_or_default()
    }

    fn bold() -> FontFamily {
        FontFamily::Name("bold".into())
    }

    fn light() -> FontFamily {
        FontFamily::Name("light".into())
    }

    #[test]
    fn with_no_fonts_installed_selawik_and_eguis_fonts_make_every_family() {
        let p = plan(installed(&[]));
        assert_eq!(family(&p, &FontFamily::Proportional), ["Selawik", "Ubuntu-Light", "egui-icons"]);
        assert_eq!(family(&p, &bold()), ["Selawik Bold", "Selawik", "Ubuntu-Light", "egui-icons"]);
        assert_eq!(family(&p, &light()), ["Selawik Semilight", "Selawik", "Ubuntu-Light", "egui-icons"]);
        assert_eq!(family(&p, &FontFamily::Monospace), ["Hack", "Ubuntu-Light", "egui-icons", "Selawik"]);
        assert!(p.fonts.iter().all(|(_, f)| matches!(f, Face::Bundled(_))));
        // read: every family has fonts, and every font of a family is there
        let defs = build(&p, |f| match f {
            Face::Bundled(s) => Some(Arc::new(egui::FontData::from_static(s.bytes()))),
            Face::Installed(_) => None,
        });
        for f in [FontFamily::Proportional, FontFamily::Monospace, bold(), light()] {
            let names = &defs.families[&f];
            assert!(!names.is_empty(), "{f:?}");
            assert!(names.iter().all(|n| defs.font_data.contains_key(n)), "{f:?}: {names:?}");
        }
    }

    #[test]
    fn segoe_ui_is_the_interface_face_when_installed() {
        let p = plan(installed(&[("Segoe UI", &[300, 350, 400, 600, 700]), ("Segoe UI Symbol", &[400]), ("Consolas", &[400, 700])]));
        assert_eq!(family(&p, &FontFamily::Proportional), ["Segoe UI 400", "Ubuntu-Light", "egui-icons", "Segoe UI Symbol 400"]);
        assert_eq!(family(&p, &bold())[0], "Segoe UI 700");
        assert_eq!(family(&p, &light())[0], "Segoe UI 350");
        assert_eq!(family(&p, &FontFamily::Monospace)[..2], ["Consolas 400", "Hack"]);
        assert!(p.fonts.iter().all(|(_, f)| !matches!(f, Face::Bundled(_))), "Selawik isn't needed");
        // without its bold and semilight faces, those families start with the regular face, as
        // on Windows
        let p = plan(installed(&[("Segoe UI", &[400])]));
        assert_eq!(family(&p, &bold())[0], "Segoe UI 400");
        assert_eq!(family(&p, &light())[0], "Segoe UI 400");
    }

    #[test]
    fn installed_fonts_fill_in_what_selawik_lacks() {
        // a Linux desktop: Noto, DejaVu, Liberation
        let p = plan(installed(&[
            ("Noto Sans", &[400, 700]),
            ("DejaVu Sans", &[400, 700]),
            ("DejaVu Sans Mono", &[400, 700]),
            ("Liberation Sans", &[400, 700]),
            ("Noto Sans Symbols2", &[400]),
            ("Noto Sans Symbols", &[400, 700]),
            ("Noto Sans Math", &[400]),
        ]));
        let symbols = ["DejaVu Sans 400", "Noto Sans Symbols2 400", "Noto Sans Symbols 400", "Noto Sans Math 400"];
        let with = |head: &[&'static str], tail: &[&'static str]| [head, &["Ubuntu-Light", "egui-icons"], &symbols, tail].concat();
        assert_eq!(family(&p, &FontFamily::Proportional), with(&["Selawik", "Noto Sans 400"], &[]));
        assert_eq!(family(&p, &bold()), with(&["Selawik Bold", "Selawik", "Noto Sans 700", "Noto Sans 400"], &[]));
        assert_eq!(family(&p, &light()), with(&["Selawik Semilight", "Selawik", "Noto Sans 400"], &[]));
        assert_eq!(family(&p, &FontFamily::Monospace), with(&["DejaVu Sans Mono 400", "Hack"], &["Selawik"]));
        // a font in two roles is added once
        assert_eq!(p.fonts.iter().filter(|(n, _)| n == "DejaVu Sans 400").count(), 1);
        // DejaVu alone: it is the text font and the symbol font; no bold face, so bold text it
        // has to show is in its regular face
        let p = plan(installed(&[("DejaVu Sans", &[400])]));
        assert_eq!(family(&p, &FontFamily::Proportional), ["Selawik", "DejaVu Sans 400", "Ubuntu-Light", "egui-icons"]);
        assert_eq!(family(&p, &bold()), ["Selawik Bold", "Selawik", "DejaVu Sans 400", "Ubuntu-Light", "egui-icons"]);
        // the older and newer names of Noto Sans Symbols 2
        for name in ["Noto Sans Symbols 2", "Noto Sans Symbols2"] {
            let p = plan(|f: &str, w| (f == name && w == REGULAR).then_some(()));
            assert!(family(&p, &FontFamily::Proportional).contains(&format!("{name} 400").as_str()), "{name}");
        }
    }

    #[test]
    fn monospace_is_consolas_or_its_look_alike() {
        let mono = |list: &'static [(&'static str, &'static [u16])]| family(&plan(installed(list)), &FontFamily::Monospace)[0].to_string();
        assert_eq!(mono(&[("Consolas", &[400]), ("DejaVu Sans Mono", &[400])]), "Consolas 400");
        assert_eq!(mono(&[("Inconsolata", &[400]), ("DejaVu Sans Mono", &[400])]), "Inconsolata 400");
        assert_eq!(mono(&[("Liberation Mono", &[400]), ("Noto Sans Mono", &[400])]), "Liberation Mono 400");
        assert_eq!(mono(&[("Noto Sans Mono", &[400])]), "Noto Sans Mono 400");
        assert_eq!(mono(&[]), "Hack");
    }

    #[test]
    fn chinese_and_japanese_follow_the_language() {
        let ja = cjk_families(Lang::JaJp);
        assert_eq!(ja[..2], ["Yu Gothic UI", "Noto Sans CJK JP"]);
        let tw = cjk_families(Lang::ZhTw);
        assert_eq!(tw[..2], ["Microsoft JhengHei UI", "Noto Sans CJK TC"]);
        for lang in [Lang::ZhCn, Lang::EnUs, Lang::PtBr, Lang::RuRu] {
            assert_eq!(cjk_families(lang)[..2], ["Microsoft YaHei UI", "Noto Sans CJK SC"]);
        }
        // whichever is installed is found, in every language, once
        for lang in [Lang::JaJp, Lang::ZhTw, Lang::ZhCn] {
            let l = cjk_families(lang);
            assert!(CJK.iter().all(|f| l.contains(f)), "{lang:?}");
            let mut d = l.clone();
            d.sort();
            d.dedup();
            assert_eq!(d.len(), l.len());
        }
    }

    /// The installed fonts as the program reads them (on Windows Segoe UI is installed, so the
    /// families start with it): every family has fonts, and every font is there.
    #[test]
    fn the_installed_fonts_make_every_family() {
        let defs = definitions();
        for f in [FontFamily::Proportional, FontFamily::Monospace, bold(), light()] {
            let names = &defs.families[&f];
            assert!(!names.is_empty(), "{f:?}");
            assert!(names.iter().all(|n| defs.font_data.contains_key(n)), "{f:?}: {names:?}");
        }
        if cfg!(windows) {
            assert_eq!(defs.families[&FontFamily::Proportional][0], "Segoe UI 400");
        }
    }

    /// The interface's text, symbols, Cyrillic and Greek all have glyphs, in every family, where
    /// the usual fonts are installed (Segoe UI Symbol on Windows; DejaVu Sans and Noto Sans
    /// Symbols 2 on Linux).
    #[test]
    fn the_interface_characters_have_glyphs() {
        let db = vector_core::text::system_fonts();
        let has = |f: &str| find(db, f, REGULAR).is_some();
        if !has("Segoe UI Symbol") && !(has("DejaVu Sans") && (has("Noto Sans Symbols2") || has("Noto Sans Symbols 2"))) {
            eprintln!("neither Segoe UI Symbol nor DejaVu Sans and Noto Sans Symbols 2 are installed; skipped");
            return;
        }
        let ctx = egui::Context::default();
        ctx.set_fonts(definitions());
        ctx.set_missing_glyph_policy(egui::MissingGlyphPolicy::Panic);
        // the symbols the interface's labels use (see visual_interface_fonts), Latin, Cyrillic, Greek
        let text = "Àçãõ ªº № ✓✔ ▸▶◀◂△□○◯◇◦★☆⚠✦ →−⊕⊣⊥⌄⏷⟳ ℹ 📋🔒 Ёжик Ωμέγα";
        let mut out = ctx.run_ui(Default::default(), |ui| {
            for f in [FontFamily::Proportional, FontFamily::Monospace, bold(), light()] {
                ui.label(egui::RichText::new(text).family(f));
            }
        });
        // no renderer takes the font atlas
        out.textures_delta.clear();
    }

    /// Selawik measures as Segoe UI: text set in it is as wide, so the interface lays out as on
    /// Windows. Compared where Segoe UI is installed (Windows).
    #[test]
    fn selawik_is_as_wide_as_segoe_ui() {
        let db = vector_core::text::system_fonts();
        let pairs = [(Selawik::Regular, REGULAR), (Selawik::Bold, BOLD), (Selawik::Semilight, SEMILIGHT)];
        let text = "AnyVector: Object Properties, Align and Distribute (Ctrl+Shift+A) 0123456789 %";
        for (s, w) in pairs {
            let Some(id) = find(db, "Segoe UI", w).filter(|id| db.face(*id).is_some_and(|f| f.weight.0 == w)) else {
                eprintln!("Segoe UI {w} isn't installed; skipped");
                continue;
            };
            let segoe = font_data(db, &Face::Installed(id)).unwrap();
            let width = |data: Arc<egui::FontData>| {
                let mut defs = egui::FontDefinitions::empty();
                defs.font_data.insert("f".into(), data);
                defs.families.insert(FontFamily::Proportional, vec!["f".into()]);
                let ctx = egui::Context::default();
                ctx.set_fonts(defs);
                let mut x = 0.0;
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    x = ui.fonts_mut(|f| f.layout_no_wrap(text.into(), egui::FontId::proportional(12.5), egui::Color32::WHITE).size().x);
                });
                out.textures_delta.clear();
                x
            };
            let (a, b) = (width(Arc::new(egui::FontData::from_static(s.bytes()))), width(segoe));
            assert!((a - b).abs() <= b * 0.01, "{s:?}: {a} against Segoe UI's {b}");
        }
    }

    /// Icon text: Segoe UI and Times New Roman are there by those names (the installed fonts or
    /// their stand-ins), and an icon with text draws it.
    #[test]
    fn icons_find_their_fonts() {
        let mut o = usvg::Options::default();
        icon_fonts(o.fontdb_mut());
        let db = o.fontdb.clone();
        for (name, generic) in [("Segoe UI", fontdb::Family::SansSerif), ("Times New Roman", fontdb::Family::Serif)] {
            assert!(db.query(&fontdb::Query { families: &[fontdb::Family::Name(name)], ..Default::default() }).is_some(), "{name}");
            assert_eq!(db.family_name(&generic), name);
        }
        let bold = db.query(&fontdb::Query { families: &[fontdb::Family::Name("Segoe UI")], weight: fontdb::Weight::BOLD, ..Default::default() }).and_then(|id| db.face(id)).map(|f| f.weight);
        assert_eq!(bold, Some(fontdb::Weight::BOLD));
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><text x="2" y="18" font-family="Segoe UI" font-size="16" fill="#fff">Ab</text></svg>"##;
        let tree = usvg::Tree::from_str(svg, &o).unwrap();
        let mut pm = tiny_skia::Pixmap::new(24, 24).unwrap();
        resvg::render(&tree, tiny_skia::Transform::identity(), &mut pm.as_mut());
        assert!(pm.data().chunks(4).any(|p| p[3] > 0), "the text is drawn");
    }
}
