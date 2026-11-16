//! SVG export: text as text (`<text>`) or as curves (issue #88).

use kurbo::Affine;
use vector_core::style::*;
use vector_core::text::{CharFormat, TextObj};
use vector_core::{Color, Document, ObjKind, Object};
use vector_io::svg_export::{page_to_svg, SvgOptions};

fn doc_with(t: TextObj, style: Style, xf: Affine) -> Document {
    let mut d = Document::new(612.0, 792.0);
    let id = d.new_id();
    d.pages[0].layers[0].objects = vec![Object::new(id, ObjKind::Text(t), xf, style)];
    d
}

fn svg(d: &Document, text_as_text: bool) -> String {
    page_to_svg(d, 0, &SvgOptions { text_as_text, ..Default::default() })
}

fn red() -> Style {
    let mut s = Style::text_default();
    s.fill = Fill::Solid(Color::rgb(200, 10, 20));
    s
}

#[test]
fn text_is_written_as_text_when_asked() {
    let fmt = CharFormat { font: "Arial".into(), size: 24.0, bold: true, underline: true, ..Default::default() };
    let d = doc_with(TextObj::artistic("Hi & <you>", fmt), red(), Affine::translate((60.0, 100.0)));
    let s = svg(&d, true);
    assert!(s.contains("<text transform=\"matrix(1 0 0 1 60 100)\""), "{s}");
    assert!(s.contains(">Hi &amp; &lt;you&gt;</tspan>"), "the characters, escaped: {s}");
    assert!(s.contains("font-family=\"'Arial'\" font-size=\"24\" font-weight=\"bold\""), "{s}");
    assert!(s.contains("fill=\"#c80a14\""), "the object's fill: {s}");
    assert!(s.contains("text-decoration=\"underline\""), "{s}");
    // every character has its own x, from the layout
    let xs = s.split("<tspan x=\"").nth(1).unwrap().split('"').next().unwrap();
    assert_eq!(xs.split(' ').count(), "Hi & <you>".chars().count(), "{xs}");
    // and no glyph outlines
    assert!(!s.contains("<path"), "{s}");
}

#[test]
fn text_is_curves_by_default() {
    let d = doc_with(TextObj::artistic("Curves", CharFormat::default()), red(), Affine::translate((60.0, 100.0)));
    let s = svg(&d, false);
    assert!(!s.contains("<text"), "{s}");
    assert!(s.contains("<path"), "{s}");
}

#[test]
fn lines_and_colored_characters_are_separate_runs() {
    let mut t = TextObj::artistic("One\nTwo", CharFormat { size: 20.0, ..Default::default() });
    t.spans.push(vector_core::text::Span { start: 4, end: 7, format: CharFormat { size: 20.0, color: Some(Color::rgb(0, 0, 255)), ..Default::default() } });
    let d = doc_with(t, red(), Affine::IDENTITY);
    let s = svg(&d, true);
    assert_eq!(s.matches("<tspan").count(), 2, "{s}");
    assert!(s.contains(">One</tspan>") && s.contains(">Two</tspan>"), "{s}");
    assert!(s.contains("fill=\"#0000ff\""), "the second line keeps its own color: {s}");
    // the second line is below the first
    let ys: Vec<f64> = s.match_indices(" y=\"").map(|(i, _)| s[i + 4..].split('"').next().unwrap().parse().unwrap()).collect();
    assert!(ys[1] > ys[0], "{ys:?}");
}

#[test]
fn text_that_cannot_be_text_stays_curves() {
    // a gradient fill
    let mut st = Style::text_default();
    st.fill = Fill::Gradient(Gradient::linear(Color::BLACK, Color::WHITE));
    let d = doc_with(TextObj::artistic("Grad", CharFormat::default()), st, Affine::IDENTITY);
    let s = svg(&d, true);
    assert!(!s.contains("<text"), "{s}");
    // vertical text
    let mut t = TextObj::artistic("Up", CharFormat::default());
    t.vertical = true;
    let s = svg(&doc_with(t, red(), Affine::IDENTITY), true);
    assert!(!s.contains("<text"), "{s}");
}

#[test]
fn an_outline_goes_on_the_text() {
    let mut st = red();
    st.outline.color = Some(Color::BLACK);
    st.outline.width = 2.0;
    let d = doc_with(TextObj::artistic("Out", CharFormat::default()), st, Affine::scale(2.0));
    let s = svg(&d, true);
    // 2 points in the document is 1 unit in the text's own (doubled) space
    assert!(s.contains("stroke=\"#000000\" stroke-width=\"1\""), "{s}");
}
