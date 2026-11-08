//! Text, Font Sampler, Glyphs and Find and Replace dockers.

use super::{heading, hint, no_doc, primary};
use crate::app::VectorApp;
use crate::propbar::edit_sel;
use egui::Ui;
use vector_core::text::TextAlign;
use vector_core::ObjKind;

fn sel_text(app: &VectorApp) -> Option<vector_core::text::TextObj> {
    let d = app.doc()?;
    let o = d.doc.find(*d.selection.first()?)?;
    if let ObjKind::Text(t) = &o.kind {
        Some(t.clone())
    } else {
        None
    }
}

pub fn text(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    heading(ui, "Character");
    ui.horizontal_wrapped(|ui| crate::propbar::text_controls(app, ui));
    let Some(t) = sel_text(app) else {
        hint(ui, "Select a text object to edit its character and paragraph formatting. New text uses the settings above.");
        return;
    };
    let mut f = t.format.clone();
    let mut ch = false;
    ui.horizontal(|ui| {
        ch |= ui.checkbox(&mut f.strike, "Strikethrough").changed();
        ch |= ui.checkbox(&mut f.caps, "All caps").changed();
    });
    ch |= ui.add(egui::Slider::new(&mut f.tracking, -100.0..=500.0).text("Character spacing %")).changed();
    ch |= ui.add(egui::Slider::new(&mut f.baseline_shift, -100.0..=100.0).text("Baseline shift (pt)")).changed();
    if ch {
        edit_sel(app, "Character Format", |o| {
            if let ObjKind::Text(tt) = &mut o.kind {
                let n = tt.char_count();
                tt.format.strike = f.strike;
                tt.format.caps = f.caps;
                tt.format.tracking = f.tracking;
                tt.format.baseline_shift = f.baseline_shift;
                let (s, c, tr, b) = (f.strike, f.caps, f.tracking, f.baseline_shift);
                if !tt.spans.is_empty() {
                    tt.apply_format(0, n, |cf| {
                        cf.strike = s;
                        cf.caps = c;
                        cf.tracking = tr;
                        cf.baseline_shift = b;
                    });
                }
            }
        });
    }
    heading(ui, "Paragraph");
    let mut tt = t.clone();
    let mut ch = false;
    ui.horizontal(|ui| {
        for (n, a) in [("Left", TextAlign::Left), ("Center", TextAlign::Center), ("Right", TextAlign::Right), ("Justify", TextAlign::Justify), ("Force", TextAlign::ForceJustify)] {
            ch |= ui.selectable_value(&mut tt.align, a, n).changed();
        }
    });
    ch |= ui.add(egui::Slider::new(&mut tt.line_spacing, 50.0..=400.0).text("Line spacing %")).changed();
    ch |= ui.add(egui::Slider::new(&mut tt.para_spacing, 0.0..=400.0).text("Paragraph spacing %")).changed();
    ch |= ui.add(egui::Slider::new(&mut tt.word_spacing, 0.0..=500.0).text("Word spacing %")).changed();
    if matches!(tt.kind, vector_core::text::TextKind::Paragraph { .. }) {
        heading(ui, "Frame");
        ch |= ui.add(egui::Slider::new(&mut tt.first_indent, 0.0..=144.0).text("First line indent (pt)")).changed();
        ch |= ui.add(egui::Slider::new(&mut tt.left_indent, 0.0..=144.0).text("Left indent (pt)")).changed();
        ch |= ui.add(egui::Slider::new(&mut tt.right_indent, 0.0..=144.0).text("Right indent (pt)")).changed();
        ch |= ui.add(egui::Slider::new(&mut tt.columns, 1..=8).text("Columns")).changed();
        ch |= ui.add(egui::Slider::new(&mut tt.column_gap, 0.0..=72.0).text("Gutter (pt)")).changed();
    }
    if ch {
        edit_sel(app, "Paragraph Format", |o| {
            if let ObjKind::Text(x) = &mut o.kind {
                x.align = tt.align;
                x.line_spacing = tt.line_spacing;
                x.para_spacing = tt.para_spacing;
                x.word_spacing = tt.word_spacing;
                x.first_indent = tt.first_indent;
                x.left_indent = tt.left_indent;
                x.right_indent = tt.right_indent;
                x.columns = tt.columns;
                x.column_gap = tt.column_gap;
            }
        });
    }
    ui.horizontal(|ui| {
        if ui.button("Convert to Curves").clicked() {
            app.run(crate::commands::Cmd::ConvertToCurves);
        }
        if ui.button("Convert Artistic/Paragraph").clicked() {
            app.run(crate::commands::Cmd::ConvertText);
        }
    });
}

pub fn font_sampler(app: &mut VectorApp, ui: &mut Ui) {
    ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.sampler_text).desired_width(f32::INFINITY));
    ui.add(egui::Slider::new(&mut app.dockers.ui.sampler_size, 8.0..=72.0).text("Size"));
    let mut fams: Vec<String> = app.ts.recent_fonts.clone();
    for f in ["Arial", "Segoe UI", "Times New Roman", "Georgia", "Verdana", "Courier New", "Calibri", "Cambria", "Consolas", "Tahoma"] {
        if !fams.iter().any(|x| x == f) {
            fams.push(f.to_string());
        }
    }
    let size = app.dockers.ui.sampler_size;
    let text = app.dockers.ui.sampler_text.clone();
    for fam in fams {
        let Some(tex) = sample_texture(app, ui.ctx(), &fam, &text, size) else { continue };
        ui.label(egui::RichText::new(&fam).small().color(crate::theme::TEXT_DIM));
        let r = ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(tex.id(), tex.size_vec2())).sense(egui::Sense::click()));
        if r.on_hover_text("Click to apply this font").clicked() {
            let f = fam.clone();
            if sel_text(app).is_some() {
                edit_sel(app, "Font", |o| {
                    if let ObjKind::Text(t) = &mut o.kind {
                        t.format.font = f.clone();
                        let n = t.char_count();
                        if !t.spans.is_empty() {
                            t.apply_format(0, n, |c| c.font = f.clone());
                        }
                    }
                });
            } else {
                app.ts.text_format.font = f;
            }
        }
    }
}

/// Render a text sample with the document text engine (so it shows the real font).
fn sample_texture(app: &mut VectorApp, ctx: &egui::Context, family: &str, text: &str, size: f32) -> Option<egui::TextureHandle> {
    let key = format!("sample:{family}:{text}:{size}");
    if let Some(t) = app.ts.texture_cache.get(&key) {
        return Some(t.clone());
    }
    let mut fmt = vector_core::text::CharFormat::default();
    fmt.font = family.to_string();
    fmt.size = size as f64 * 0.75;
    let t = vector_core::text::TextObj::artistic(text, fmt);
    let lay = vector_core::text::layout(&t);
    let bb = lay.bounds;
    if bb.width() <= 0.0 {
        return None;
    }
    let scale = 1.3333 * ctx.pixels_per_point() as f64;
    let w = (bb.width() * scale).ceil() as u32 + 4;
    let h = (bb.height() * scale).ceil() as u32 + 4;
    let mut pm = tiny_skia::Pixmap::new(w.min(2000), h.min(400))?;
    let path = lay.to_path();
    let xf = kurbo::Affine::translate((2.0, 2.0)) * kurbo::Affine::scale(scale) * kurbo::Affine::translate((-bb.x0, -bb.y0));
    let sk = vector_render::to_sk_path(&(xf * path))?;
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(0xd4, 0xd4, 0xd4, 255);
    paint.anti_alias = true;
    pm.fill_path(&sk, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
    let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
    let tex = ctx.load_texture(key.clone(), img, egui::TextureOptions::LINEAR);
    app.ts.texture_cache.insert(key, tex.clone());
    Some(tex)
}

pub fn glyphs(app: &mut VectorApp, ui: &mut Ui) {
    let fams: Vec<String> = vector_core::text::fonts().lock().families().to_vec();
    let cur = app.dockers.ui.glyph_font.clone();
    egui::ComboBox::from_id_salt("glyph_font").selected_text(&cur).width(220.0).height(400.0).show_ui(ui, |ui| {
        for f in &fams {
            ui.selectable_value(&mut app.dockers.ui.glyph_font, f.clone(), f);
        }
    });
    ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.glyph_filter).hint_text("Filter (character or U+hex)"));
    let chars = vector_core::text::font_chars(&app.dockers.ui.glyph_font, false, false, 4000);
    let filter = app.dockers.ui.glyph_filter.trim().to_lowercase();
    let font = app.dockers.ui.glyph_font.clone();
    let mut insert: Option<char> = None;
    let cols = ((ui.available_width() / 34.0).floor() as usize).max(4);
    egui::Grid::new("glyph_grid").spacing(egui::vec2(2.0, 2.0)).show(ui, |ui| {
        let mut k = 0;
        for c in chars {
            if c.is_control() {
                continue;
            }
            let code = format!("u+{:04x}", c as u32);
            if !filter.is_empty() && !(c.to_string() == filter || code.contains(&filter)) {
                continue;
            }
            let Some(tex) = sample_texture(app, ui.ctx(), &font, &c.to_string(), 22.0) else { continue };
            let (r, resp) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, 2.0, crate::theme::HOVER);
            }
            let s = tex.size_vec2().min(egui::vec2(28.0, 28.0));
            ui.painter().image(tex.id(), egui::Rect::from_center_size(r.center(), s), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
            if resp.on_hover_text(format!("{} (U+{:04X})", c, c as u32)).clicked() {
                insert = Some(c);
            }
            k += 1;
            if k % cols == 0 {
                ui.end_row();
            }
            if k > 1500 {
                break;
            }
        }
    });
    if let Some(c) = insert {
        crate::tools::insert_text(app, &c.to_string(), Some(&font));
    }
}

pub fn find_replace(app: &mut VectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    ui.label("Find text:");
    ui.text_edit_singleline(&mut app.dockers.ui.find_text);
    ui.label("Replace with:");
    ui.text_edit_singleline(&mut app.dockers.ui.replace_text);
    ui.checkbox(&mut app.dockers.ui.find_case, "Match case");
    let needle = app.dockers.ui.find_text.clone();
    let case = app.dockers.ui.find_case;
    let found = |s: &str| if case { s.contains(&needle) } else { s.to_lowercase().contains(&needle.to_lowercase()) };
    ui.horizontal(|ui| {
        if primary(ui, "Find All", !needle.is_empty()) {
            if let Some(d) = app.doc_mut() {
                let mut ids = vec![];
                for (pi, p) in d.doc.pages.iter().enumerate() {
                    for l in &p.layers {
                        for o in &l.objects {
                            o.walk(&mut |x| {
                                let hit = match &x.kind {
                                    ObjKind::Text(t) => found(&t.content),
                                    _ => found(&x.name),
                                };
                                if hit {
                                    ids.push((pi, x.id));
                                }
                            });
                        }
                    }
                }
                if let Some((pi, _)) = ids.first() {
                    d.page = *pi;
                    let pg = *pi;
                    d.selection = ids.iter().filter(|(p, _)| *p == pg).map(|(_, i)| *i).collect();
                }
                let n = ids.len();
                app.flash(format!("{n} matching objects found"));
            }
        }
        if ui.add_enabled(!needle.is_empty(), egui::Button::new("Replace All")).clicked() {
            let rep = app.dockers.ui.replace_text.clone();
            let mut count = 0;
            if let Some(d) = app.doc_mut() {
                d.modify("Replace Text", |doc, _| {
                    for p in doc.pages.iter_mut() {
                        for l in p.layers.iter_mut() {
                            for o in l.objects.iter_mut() {
                                o.walk_mut(&mut |x| {
                                    if let ObjKind::Text(t) = &mut x.kind {
                                        let new = if case { t.content.replace(&needle, &rep) } else { replace_ci(&t.content, &needle, &rep) };
                                        if new != t.content {
                                            count += 1;
                                            t.spans.clear();
                                            t.content = new;
                                        }
                                    }
                                });
                            }
                        }
                    }
                });
            }
            app.flash(format!("Replaced text in {count} objects"));
        }
    });
    hint(ui, "Searches text objects and object names on all pages.");
}

fn replace_ci(s: &str, needle: &str, rep: &str) -> String {
    let lower = s.to_lowercase();
    let nl = needle.to_lowercase();
    if nl.is_empty() || lower.len() != s.len() {
        return s.replace(needle, rep);
    }
    let mut out = String::new();
    let mut i = 0;
    while let Some(p) = lower[i..].find(&nl) {
        out.push_str(&s[i..i + p]);
        out.push_str(rep);
        i += p + nl.len();
    }
    out.push_str(&s[i..]);
    out
}
