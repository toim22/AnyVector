//! The text fields of the property bar (font, size, style, alignment), also used by the Text
//! docker.

use super::{edit_sel, fields, selected_kind};
use crate::app::AnyVectorApp;
use egui::Ui;
use vector_core::text::{CharFormat, TextAlign, TextObj};
use vector_core::ObjKind;

/// The selected text object, the selected character range when editing, and the format the
/// fields show (the selection's, else the format for new text).
fn target(app: &AnyVectorApp) -> (Option<TextObj>, Option<(usize, usize)>, CharFormat) {
    let text_obj = selected_kind(app, |k| if let ObjKind::Text(t) = k { Some(t.clone()) } else { None });
    let range = app.doc().and_then(|d| d.text_edit.as_ref()).filter(|te| te.caret != te.anchor).map(|te| (te.caret.min(te.anchor), te.caret.max(te.anchor)));
    let fmt = match (&text_obj, range) {
        (Some(t), Some((s, _))) => t.format_at(s).clone(),
        (Some(t), None) => t.format.clone(),
        _ => app.ts.text_format.clone(),
    };
    (text_obj, range, fmt)
}

/// The character format the fields show.
pub fn format(app: &AnyVectorApp) -> CharFormat {
    target(app).2
}

/// Change the character format of the selected text (the selected characters while editing),
/// or the format of new text when no text is selected.
pub fn apply(app: &mut AnyVectorApp, label: &str, f: &dyn Fn(&mut CharFormat)) {
    let (text_obj, range, _) = target(app);
    if text_obj.is_none() {
        f(&mut app.ts.text_format);
        return;
    }
    edit_sel(app, label, |o| {
        if let ObjKind::Text(t) = &mut o.kind {
            match range {
                Some((s, e)) => t.apply_format(s, e, f),
                None => {
                    f(&mut t.format);
                    let n = t.char_count();
                    if !t.spans.is_empty() {
                        t.apply_format(0, n, f);
                    }
                }
            }
        }
    });
}

/// The paragraph alignment of the selected text, else of new text.
pub fn align(app: &AnyVectorApp) -> TextAlign {
    selected_kind(app, |k| if let ObjKind::Text(t) = k { Some(t.align) } else { None }).unwrap_or(app.ts.text_align)
}

pub fn set_align(app: &mut AnyVectorApp, a: TextAlign) {
    if selected_kind(app, |k| matches!(k, ObjKind::Text(_)).then_some(())).is_some() {
        edit_sel(app, "Alignment", |o| {
            if let ObjKind::Text(t) = &mut o.kind {
                t.align = a;
                t.clear_paragraph_styles();
            }
        });
    } else {
        app.ts.text_align = a;
    }
}

/// The font list.
pub fn font_list(app: &mut AnyVectorApp, ui: &mut Ui, lead: fields::Lead, w: f32, tip: &str) {
    let fmt = format(app);
    let families: Vec<String> = vector_core::text::fonts().lock().families().to_vec();
    let recent = app.ts.recent_fonts.clone();
    let mut chosen: Option<String> = None;
    fields::combo(app, ui, lead, &fmt.font, w, true, tip, |_, ui| {
        for f in &recent {
            if ui.selectable_label(*f == fmt.font, egui::RichText::new(f).strong()).clicked() {
                chosen = Some(f.clone());
            }
        }
        if !recent.is_empty() {
            ui.separator();
        }
        for f in &families {
            if ui.selectable_label(*f == fmt.font, f).clicked() {
                chosen = Some(f.clone());
            }
        }
    });
    if let Some(f) = chosen {
        app.ts.recent_fonts.retain(|x| *x != f);
        app.ts.recent_fonts.insert(0, f.clone());
        app.ts.recent_fonts.truncate(app.settings.recent_fonts.max(1));
        apply(app, "Font", &move |c: &mut CharFormat| c.font = f.clone());
    }
}

/// The font size: a value the user can type, and the list of common sizes.
pub fn font_size(app: &mut AnyVectorApp, ui: &mut Ui, lead: fields::Lead, tip: &str) {
    let size = format(app).size;
    let id = ui.make_persistent_id("font_size");
    let list: Vec<(String, f64)> = [6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 24.0, 36.0, 48.0, 72.0, 100.0, 150.0, 200.0].iter().map(|s| (format!("{} pt", trim_num(*s)), *s)).collect();
    let typed = fields::edit_list(app, ui, id, lead, Some(size), fields::Fmt::Trim(" pt"), 84.0, true, tip, &list);
    if let Some(s) = typed.map(|v| v.clamp(0.5, 3000.0)).filter(|s| (s - size).abs() > 1e-9) {
        apply(app, "Font Size", &move |c: &mut CharFormat| c.size = s);
    }
}

fn trim_num(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Variable Fonts: a slider for each design axis of the font (off for fonts without axes).
pub fn variable_axes(app: &mut AnyVectorApp, ui: &mut Ui, it: &crate::ui::Item) {
    let f = format(app);
    let axes = vector_core::text::fonts().lock().variation_axes(&f.font, f.bold, f.italic);
    let tip = if it.tip.is_empty() { crate::ui::plain(it.text) } else { it.tip.to_string() };
    let lead = if it.icon.is_empty() { fields::Lead::None } else { fields::Lead::Icon(it.icon) };
    let on = app.has_doc() && !axes.is_empty();
    let mut out: Option<Vec<(String, f32)>> = None;
    fields::panel(app, ui, lead, "", 16.0, on, &tip, |_, ui| {
        let mut vals = f.variations.clone();
        let mut ch = false;
        for (tag, name, lo, def, hi) in &axes {
            let mut v = vals.iter().find(|(t, _)| t == tag).map(|x| x.1).unwrap_or(*def);
            ui.label(name);
            if ui.add(egui::Slider::new(&mut v, *lo..=*hi)).changed() {
                match vals.iter_mut().find(|(t, _)| t == tag) {
                    Some(x) => x.1 = v,
                    None => vals.push((tag.clone(), v)),
                }
                ch = true;
            }
        }
        ui.separator();
        if ui.button("Reset").clicked() {
            vals.clear();
            ch = true;
        }
        if ch {
            out = Some(vals);
        }
    });
    if let Some(v) = out {
        apply(app, "Variable Fonts", &move |c| c.variations = v.clone());
    }
}
