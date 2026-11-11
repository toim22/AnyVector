//! File ▸ Document Properties: language, title, subject, author, copyright, tags, notes and
//! rating, then the document statistics in titled sections (File, Document, Color, Graphic
//! Objects, Text Statistics, Bitmap Objects, Styles, Effects, Fills, Outlines, Print Merge
//! Fields), as in the reference dialog.

use super::{ok_cancel_row, window};
use crate::app::AnyVectorApp;
use crate::theme;
use egui::{vec2, Ui};
use std::collections::BTreeMap;
use vector_core::style::Fill;
use vector_core::{Document, ObjKind};

/// The languages offered by the Language list.
pub const LANGUAGES: [&str; 15] = [
    "Chinese (Simplified)",
    "Chinese (Traditional)",
    "Czech",
    "Dutch (The Netherlands)",
    "English (United States)",
    "French (France)",
    "German (Germany)",
    "Italian",
    "Japanese",
    "Polish",
    "Portuguese (Brazil)",
    "Russian (Russia)",
    "Spanish (Spain)",
    "Swedish (Sweden)",
    "Turkish",
];

const RATINGS: [&str; 6] = ["None", "1 star", "2 stars", "3 stars", "4 stars", "5 stars"];

/// The statistics shown under the fields: (section, [(label, value)]).
pub fn sections(doc: &Document, name: &str) -> Vec<(&'static str, Vec<(String, String)>)> {
    let mut v = vec![];
    v.push(("File", vec![("Name and location:".into(), name.to_string())]));
    let p0 = &doc.pages[0];
    let orientation = if p0.width > p0.height { "Landscape" } else { "Portrait" };
    v.push((
        "Document",
        vec![
            ("Pages:".into(), doc.pages.len().to_string()),
            ("Layers:".into(), doc.pages.iter().map(|p| p.layers.len()).max().unwrap_or(0).to_string()),
            ("Page size:".into(), page_size_name(p0.width, p0.height)),
            ("Page orientation:".into(), orientation.into()),
            ("Resolution (dpi):".into(), format!("{}", doc.settings.dpi.round())),
        ],
    ));
    let c = &doc.settings.color;
    v.push((
        "Color",
        vec![
            ("RGB Profile:".into(), c.rgb_profile.clone()),
            ("CMYK Profile:".into(), c.cmyk_profile.clone()),
            ("Grayscale Profile:".into(), c.gray_profile.clone()),
            ("Primary color mode:".into(), if c.cmyk { "CMYK" } else { "RGB" }.into()),
            ("Rendering intent:".into(), title_case(c.intent.name())),
        ],
    ));
    // walk every object once
    let mut kinds: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut objects = 0;
    let mut nodes = 0;
    let (mut texts, mut paragraphs, mut words, mut chars) = (0, 0, 0, 0);
    let mut fonts: Vec<String> = vec![];
    let mut bitmaps = 0;
    let mut effects: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut fills: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut outlines: BTreeMap<&'static str, usize> = BTreeMap::new();
    for p in &doc.pages {
        for l in &p.layers {
            for o in &l.objects {
                o.walk(&mut |x| {
                    objects += 1;
                    *kinds.entry(x.kind.type_name()).or_default() += 1;
                    match &x.kind {
                        ObjKind::Path(np) => nodes += np.subpaths.iter().map(|s| s.nodes.len()).sum::<usize>(),
                        ObjKind::Text(t) => {
                            texts += 1;
                            paragraphs += t.content.split('\n').count();
                            words += t.content.split_whitespace().count();
                            chars += t.content.chars().filter(|c| *c != '\n').count();
                            for f in std::iter::once(&t.format.font).chain(t.spans.iter().map(|s| &s.format.font)) {
                                if !fonts.contains(f) {
                                    fonts.push(f.clone());
                                }
                            }
                        }
                        ObjKind::Image(_) => bitmaps += 1,
                        _ => {}
                    }
                    let e = &x.effects;
                    for (on, n) in [
                        (e.drop_shadow.is_some() || e.inner_shadow.is_some() || e.block_shadow.is_some(), "Shadows:"),
                        (e.contour.is_some(), "Contours:"),
                        (e.extrude.is_some(), "Extrusions:"),
                        (e.distort.is_some(), "Distortions:"),
                        (e.envelope.is_some(), "Envelopes:"),
                        (e.lens.is_some(), "Lenses:"),
                        (e.bevel.is_some(), "Bevels:"),
                        (!e.bitmap.is_empty(), "Bitmap effects:"),
                        (matches!(x.kind, ObjKind::Blend(_)), "Blends:"),
                    ] {
                        if on {
                            *effects.entry(n).or_default() += 1;
                        }
                    }
                    if !matches!(x.kind, ObjKind::Group(_) | ObjKind::Image(_)) {
                        let f = match &x.style.fill {
                            Fill::None => "No fill:",
                            Fill::Solid(_) => "Uniform fills:",
                            Fill::Gradient(_) => "Fountain fills:",
                            Fill::Pattern(_) => "Pattern fills:",
                            Fill::Mesh(_) => "Mesh fills:",
                        };
                        *fills.entry(f).or_default() += 1;
                        let ol = &x.style.outline;
                        let k = if ol.color.is_none() {
                            "No outline:"
                        } else if ol.width <= 0.0 {
                            "Hairline outlines:"
                        } else {
                            "Outlines:"
                        };
                        *outlines.entry(k).or_default() += 1;
                    }
                });
            }
        }
    }
    let mut g: Vec<(String, String)> = vec![("Objects:".into(), objects.to_string()), ("Points in curves:".into(), nodes.to_string())];
    for (k, n) in &kinds {
        g.push((format!("{}:", plural(k)), n.to_string()));
    }
    v.push(("Graphic Objects", g));
    let mut t = vec![
        ("Text objects:".into(), texts.to_string()),
        ("Paragraphs:".into(), paragraphs.to_string()),
        ("Words:".into(), words.to_string()),
        ("Characters:".into(), chars.to_string()),
    ];
    if !fonts.is_empty() {
        t.push(("Fonts used:".into(), fonts.join(", ")));
    }
    v.push(("Text Statistics", t));
    v.push(("Bitmap Objects", vec![("Bitmaps:".into(), bitmaps.to_string()), ("Images in document:".into(), doc.images.len().to_string())]));
    v.push(("Styles", vec![("Object styles:".into(), doc.object_styles.len().to_string()), ("Color styles:".into(), doc.color_styles.len().to_string())]));
    let list = |m: BTreeMap<&'static str, usize>| -> Vec<(String, String)> {
        if m.is_empty() {
            vec![("None".into(), String::new())]
        } else {
            m.into_iter().map(|(k, n)| (k.to_string(), n.to_string())).collect()
        }
    };
    v.push(("Effects", list(effects)));
    v.push(("Fills", list(fills)));
    v.push(("Outlines", list(outlines)));
    let merge: Vec<(String, String)> = match &doc.print_merge {
        Some(pm) if !pm.fields.is_empty() => pm.fields.iter().map(|f| (f.name.clone(), format!("{:?}", f.kind))).collect(),
        _ => vec![("None".into(), String::new())],
    };
    v.push(("Print Merge Fields", merge));
    v
}

fn plural(type_name: &str) -> String {
    match type_name {
        "Ellipse" => "Ellipses".into(),
        "Polygon" => "Polygons".into(),
        "Text" => "Text objects".into(),
        "Bitmap" => "Bitmaps".into(),
        "Group" => "Groups".into(),
        "Curve" => "Curves".into(),
        "Rectangle" => "Rectangles".into(),
        n if n.ends_with('s') => n.to_string(),
        n => format!("{n}s"),
    }
}

fn title_case(s: &str) -> String {
    s.split(' ').map(|w| {
        let mut c = w.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    }).collect::<Vec<_>>().join(" ")
}

/// "Letter (8.5 × 11 in.)" for a known size, otherwise the size in inches.
pub fn page_size_name(w: f64, h: f64) -> String {
    for p in super::new_doc::builtin_presets() {
        if ((p.width - w).abs() < 0.5 && (p.height - h).abs() < 0.5) || ((p.width - h).abs() < 0.5 && (p.height - w).abs() < 0.5) {
            return format!("{} ({})", p.name, p.size_text());
        }
    }
    let f = |v: f64| format!("{:.3}", v / 72.0).trim_end_matches('0').trim_end_matches('.').to_string();
    format!("Custom ({} × {} in.)", f(w), f(h))
}

pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(d) = app.doc() else {
        app.dialogs.doc_props = false;
        return;
    };
    let mut info = d.doc.info.clone();
    let name = d.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| d.title.trim_end_matches('*').to_string());
    let secs = sections(&d.doc, &name);
    let mut result = (false, false);
    let ((), dismissed) = window(ctx, "doc_props", "Document Properties", vec2(358.0, 760.0), |ui| {
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing.y = 10.0;
        let label_w = 76.0;
        let field_w = ui.available_width() - label_w - 4.0;
        let row = |ui: &mut Ui, label: &str, body: &mut dyn FnMut(&mut Ui)| {
            ui.horizontal(|ui| {
                ui.add_sized(vec2(label_w, 22.0), egui::Label::new(label).halign(egui::Align::Min));
                body(ui);
            });
        };
        row(ui, "Language:", &mut |ui| {
            egui::ComboBox::from_id_salt("dp_lang").selected_text(&info.language).width(field_w - 8.0).show_ui(ui, |ui| {
                for l in LANGUAGES {
                    ui.selectable_value(&mut info.language, l.to_string(), l);
                }
            });
        });
        for (label, field) in [("Title:", &mut info.title), ("Subject:", &mut info.subject), ("Author:", &mut info.author), ("Copyright:", &mut info.copyright), ("Tags:", &mut info.keywords)] {
            row(ui, label, &mut |ui| {
                ui.add(egui::TextEdit::singleline(field).desired_width(field_w));
            });
        }
        ui.horizontal_top(|ui| {
            ui.add_sized(vec2(label_w, 22.0), egui::Label::new("Notes:").halign(egui::Align::Min));
            ui.add(egui::TextEdit::multiline(&mut info.notes).desired_width(field_w).desired_rows(4));
        });
        row(ui, "Rating:", &mut |ui| {
            let mut r = info.rating.min(5) as usize;
            if super::combo(ui, "dp_rating", &mut r, &RATINGS, field_w - 8.0) {
                info.rating = r as u8;
            }
        });
        egui::Frame::new().fill(theme::FIELD()).stroke(egui::Stroke::new(1.0, theme::SEP())).inner_margin(6.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().id_salt("dp_stats").max_height(370.0).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                for (title, rows) in &secs {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(*title).color(egui::Color32::from_rgb(0x4d, 0x9c, 0xe8)));
                        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width() - 6.0, 10.0), egui::Sense::hover());
                        ui.painter().hline(r.x_range(), r.center().y, egui::Stroke::new(1.0, egui::Color32::from_rgb(0x2f, 0x7b, 0xd0)));
                    });
                    egui::Grid::new(("dp_sec", *title)).num_columns(2).spacing(vec2(16.0, 8.0)).min_col_width(140.0).show(ui, |ui| {
                        for (k, v) in rows {
                            ui.label(k);
                            ui.add(egui::Label::new(v).truncate()).on_hover_text(v);
                            ui.end_row();
                        }
                    });
                }
            });
        });
        result = ok_cancel_row(ui, "OK", true);
    });
    let (ok, cancel) = result;
    if ok {
        if let Some(d) = app.doc_mut() {
            if d.doc.info != info {
                d.modify("Document Properties", |doc, _| doc.info = info.clone());
            }
        }
        app.dialogs.doc_props = false;
    } else if cancel || dismissed {
        app.dialogs.doc_props = false;
    } else if let Some(d) = app.doc_mut() {
        // keep edits between frames without an undo step
        d.doc.info = info;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letter_and_sections() {
        assert_eq!(page_size_name(612.0, 792.0), "Letter (8.5 × 11 in.)");
        let doc = Document::new(612.0, 792.0);
        let s = sections(&doc, "Untitled-1");
        let titles: Vec<&str> = s.iter().map(|(t, _)| *t).collect();
        assert_eq!(titles, ["File", "Document", "Color", "Graphic Objects", "Text Statistics", "Bitmap Objects", "Styles", "Effects", "Fills", "Outlines", "Print Merge Fields"]);
        assert_eq!(s[1].1[3].1, "Portrait");
    }
}
