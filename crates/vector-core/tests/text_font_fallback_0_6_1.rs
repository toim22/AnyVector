//! #147: characters a text's font lacks (Japanese typed through an input method into Arial)
//! show in an installed face that has them, instead of empty boxes; the text keeps its font.

use vector_core::text::{self as vt, fonts, CharFormat, TextObj};

fn fmt(font: &str, size: f64) -> CharFormat {
    CharFormat { font: font.into(), size, ..Default::default() }
}

/// The face that shows `c` when its font lacks it, if this computer has one.
fn fallback(c: char) -> Option<fontdb::ID> {
    fonts().lock().fallback_face(c, false, false)
}

#[test]
fn missing_characters_show_in_a_face_that_has_them() {
    let Some(id) = fallback('日') else {
        eprintln!("no font with Japanese characters on this computer; nothing to check");
        return;
    };
    // the same answer again (cached), and none for a character no font has
    assert_eq!(fallback('日'), Some(id));
    assert_eq!(fallback('\u{10FFFD}'), None, "a private-use character is in no font");
    let t = TextObj::artistic("A日本B", fmt("Arial", 100.0));
    let lay = vt::layout(&t);
    for ci in 0..4 {
        assert!(lay.glyphs.iter().any(|g| g.char_index == ci && !g.path.elements().is_empty()), "character {ci} has a glyph");
    }
    // full-width characters, one em each (an empty box of Arial is narrower)
    let w = |ci: usize| lay.carets[ci + 1].0 - lay.carets[ci].0;
    assert!((w(1) - 100.0).abs() < 8.0 && (w(2) - 100.0).abs() < 8.0, "{} {}", w(1), w(2));
    // the Latin letters keep Arial's widths
    let arial = vt::layout(&TextObj::artistic("AB", fmt("Arial", 100.0)));
    assert!((w(0) - (arial.carets[1].0 - arial.carets[0].0)).abs() < 1e-6);
    assert!((w(3) - (arial.carets[2].0 - arial.carets[1].0)).abs() < 1e-6);
    assert_eq!(t.format.font, "Arial", "the text keeps its font");
}

/// Symbols Arial lacks (⏷ 📋 ⟳, which the interface uses too) show in a symbol face: Segoe UI
/// Symbol on Windows; on Linux Noto's symbol faces, which are tried there after the Windows ones.
#[test]
fn symbols_show_in_a_symbol_face() {
    let noto = fonts().lock().families().iter().any(|f| f == "Noto Sans Symbols2" || f == "Noto Sans Symbols 2");
    for c in ['⏷', '📋', '⟳'] {
        if fallback(c).is_none() {
            assert!(cfg!(windows) || !noto, "{c:?} is in Noto Sans Symbols 2, which is installed");
            eprintln!("no font with {c:?} on this computer; skipped");
            continue;
        }
        let lay = vt::layout(&TextObj::artistic(&format!("A{c}B"), fmt("Arial", 100.0)));
        assert!(lay.glyphs.iter().any(|g| g.char_index == 1 && !g.path.elements().is_empty()), "{c:?} has a glyph");
    }
}

#[test]
fn covered_text_is_shaped_as_before() {
    // characters the font has never look for another face: spaces, accents, Vietnamese
    let t = TextObj::artistic("Tiếng Việt đẹp", fmt("Arial", 24.0));
    let lay = vt::layout(&t);
    let n = t.content.chars().filter(|c| !c.is_whitespace()).count();
    assert_eq!(lay.glyphs.iter().filter(|g| !g.path.elements().is_empty()).map(|g| g.char_index).collect::<std::collections::BTreeSet<_>>().len(), n);
}
