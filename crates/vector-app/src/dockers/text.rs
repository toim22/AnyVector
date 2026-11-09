//! Font Sampler and Glyphs dockers.

use crate::app::VectorApp;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use vector_core::ObjKind;

/// A font list of `w` points; returns the font picked.
fn font_combo(ui: &mut Ui, id: &str, cur: &str, w: f32) -> Option<String> {
    let families: Vec<String> = vector_core::text::fonts().lock().families().to_vec();
    let mut out = None;
    egui::ComboBox::from_id_salt(id)
        .width(w)
        .height(420.0)
        .selected_text(cur)
        .show_ui(ui, |ui| {
            for f in &families {
                if ui.selectable_label(f == cur, f).clicked() {
                    out = Some(f.clone());
                }
            }
        })
        .response
        .on_hover_text("Choose a typeface for new or selected text.");
    out
}

/// The size controls: small A, a slider, big A.
fn zoom(app: &mut VectorApp, ui: &mut Ui, v: &mut f32, range: std::ops::RangeInclusive<f32>, step: f32, w: f32, tip: &str) {
    let (lo, hi) = (*range.start(), *range.end());
    if crate::widgets::icon_button(&mut app.icons, ui, "fs_smaller", tip, *v > lo, false).clicked() {
        *v = (*v - step).max(lo);
    }
    ui.spacing_mut().slider_width = w;
    ui.add(egui::Slider::new(v, range).show_value(false)).on_hover_text(tip);
    if crate::widgets::icon_button(&mut app.icons, ui, "fs_larger", tip, *v < hi, false).clicked() {
        *v = (*v + step).min(hi);
    }
}

// ------------------------------------------------------------------------------ Font Sampler

/// What the Font Sampler keeps between frames.
pub struct FsUi {
    /// The fonts of the samples, and the one picked.
    pub fonts: Vec<String>,
    pub sel: usize,
    pub text: String,
    /// 0 single line, 1 paragraph, 2 waterfall.
    pub view: u8,
    pub size: f32,
    /// The sample text is being typed.
    pub editing: bool,
}

impl Default for FsUi {
    fn default() -> Self {
        FsUi { fonts: vec!["Arial".into(), "Tahoma".into(), "Segoe UI".into()], sel: 0, text: "Lorem ipsum dolor sit amet.".into(), view: 0, size: 28.0, editing: false }
    }
}

pub fn font_sampler(app: &mut VectorApp, ui: &mut Ui) {
    let mut st = std::mem::take(&mut app.dockers.ui.fs);
    st.sel = st.sel.min(st.fonts.len().saturating_sub(1));
    ui.add_space(2.0);
    let w = ui.available_width();
    let cur = st.fonts.get(st.sel).cloned().unwrap_or_default();
    if let Some(f) = font_combo(ui, "fs_font", &cur, w - 8.0) {
        if let Some(x) = st.fonts.get_mut(st.sel) {
            *x = f;
        }
    }
    ui.add_space(4.0);
    super::separator(ui);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (k, icon, tip) in [(0u8, "fs_single", "Show a single line of text."), (1, "fs_multi", "Show a paragraph of text."), (2, "fs_waterfall", "Show single line of text in increasing sizes.")] {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, st.view == k).clicked() {
                st.view = k;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // right to left: big A, slider, small A
            let tip = "Change the size of the sample text.";
            let (lo, hi) = (10.0f32, 96.0f32);
            if crate::widgets::icon_button(&mut app.icons, ui, "fs_larger", tip, st.size < hi, false).clicked() {
                st.size = (st.size + 2.0).min(hi);
            }
            ui.spacing_mut().slider_width = 70.0;
            ui.add(egui::Slider::new(&mut st.size, lo..=hi).show_value(false)).on_hover_text(tip);
            if crate::widgets::icon_button(&mut app.icons, ui, "fs_smaller", tip, st.size > lo, false).clicked() {
                st.size = (st.size - 2.0).max(lo);
            }
        });
    });
    if st.editing {
        let r = ui.add(egui::TextEdit::singleline(&mut st.text).desired_width(f32::INFINITY));
        if r.lost_focus() {
            st.editing = false;
        }
    }
    ui.add_space(4.0);
    let mut remove = None;
    let mut copy = None;
    egui::ScrollArea::vertical().id_salt("fs_list").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let bw = ui.available_width() - 4.0;
        let samples: Vec<(usize, f32)> = if st.view == 2 {
            [0.35f32, 0.42, 0.5, 0.6, 0.75, 1.0, 1.5, 2.0].iter().map(|k| (st.sel, st.size * k)).collect()
        } else {
            (0..st.fonts.len()).map(|i| (i, st.size)).collect()
        };
        let n = samples.len();
        for (k, (i, size)) in samples.into_iter().enumerate() {
            let font = st.fonts[i].clone();
            let wrap = (st.view == 1).then_some((bw - 16.0) as f64);
            let tex = sample_texture_wrapped(app, ui.ctx(), &font, &st.text, size, wrap, Color32::WHITE);
            let ppp = ui.ctx().pixels_per_point();
            let th = tex.as_ref().map(|t| t.size_vec2().y / ppp).unwrap_or(size);
            // the name under each sample, or once under the waterfall
            let named = st.view != 2 || k + 1 == n;
            let h = th + 12.0 + if named { 16.0 } else { 0.0 };
            let (r, resp) = ui.allocate_exact_size(vec2(bw, h), Sense::click());
            let p = ui.painter_at(r);
            p.rect_filled(r, 0.0, Color32::BLACK);
            if let Some(t) = &tex {
                let s = t.size_vec2() / ppp;
                let room = r.width() - 16.0;
                let ir = Rect::from_min_size(pos2(r.left() + 6.0, r.top() + 6.0), s);
                if s.x > room && st.view != 1 {
                    // a single line too long: cut, then "..."
                    let dots = egui::FontId::proportional(size.max(9.0) * 0.95);
                    let dw = 3.0 * size * 0.28;
                    let keep = (room - dw).max(10.0);
                    let clip = Rect::from_min_size(ir.min, vec2(keep, s.y));
                    ui.painter_at(clip).image(t.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    p.text(pos2(clip.right() + 1.0, ir.bottom() - s.y * 0.12), egui::Align2::LEFT_BOTTOM, "...", dots, Color32::WHITE);
                } else {
                    p.image(t.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
            }
            if named {
                p.text(pos2(r.left() + 6.0, r.bottom() - 5.0), egui::Align2::LEFT_BOTTOM, format!("{font} Normal"), egui::FontId::proportional(11.0), crate::theme::TEXT_DIM);
            }
            if resp.clicked() {
                st.sel = i;
            }
            if resp.double_clicked() {
                st.sel = i;
                st.editing = true;
            }
            resp.context_menu(|ui| {
                crate::menus::menu_style(ui);
                if ui.button("Copy").clicked() {
                    copy = Some(i);
                    ui.close();
                }
                if ui.add_enabled(st.fonts.len() > 1, egui::Button::new("Remove")).clicked() {
                    remove = Some(i);
                    ui.close();
                }
            });
        }
        if st.view != 2 {
            ui.add_space(2.0);
            let link = ui.add(egui::Label::new(egui::RichText::new("Add another sample").color(egui::Color32::from_rgb(0x3d, 0x9b, 0xff)).size(11.5)).sense(Sense::click()));
            if link.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                let f = st.fonts.get(st.sel).cloned().unwrap_or_else(|| "Arial".into());
                st.fonts.push(f);
                st.sel = st.fonts.len() - 1;
            }
        }
    });
    if let Some(i) = remove {
        st.fonts.remove(i);
        st.sel = st.sel.min(st.fonts.len() - 1);
    }
    if let Some(i) = copy {
        copy_sample(app, &st.fonts[i], &st.text, st.size);
    }
    app.dockers.ui.fs = st;
}

/// Copy: the sample as artistic text, on the clipboard (Paste puts it into the drawing).
fn copy_sample(app: &mut VectorApp, font: &str, text: &str, size: f32) {
    let mut fmt = vector_core::text::CharFormat::default();
    fmt.font = font.to_string();
    fmt.size = size as f64 * 0.75;
    let t = vector_core::text::TextObj::artistic(text, fmt);
    let mut style = vector_core::Style::default();
    style.fill = vector_core::style::Fill::Solid(vector_core::Color::BLACK);
    style.outline.color = None;
    let o = vector_core::Object::new(1, ObjKind::Text(t), kurbo::Affine::IDENTITY, style);
    let doc = app.doc().map(|d| d.doc.clone()).unwrap_or_else(|| vector_core::Document::new(612.0, 792.0));
    let svg = vector_io::svg_export::objects_to_svg(&doc, &[o]);
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let _ = cb.set_text(svg.replacen("<svg", "<svg data-vector-clip=\"1\"", 1));
    }
}

/// Render a text sample with the document text engine (so it shows the real font).
pub(crate) fn sample_texture(app: &mut VectorApp, ctx: &egui::Context, family: &str, text: &str, size: f32) -> Option<egui::TextureHandle> {
    sample_texture_wrapped(app, ctx, family, text, size, None, Color32::from_gray(0xd4))
}

/// [`sample_texture`], wrapped to `wrap` points when given, in `color`.
fn sample_texture_wrapped(app: &mut VectorApp, ctx: &egui::Context, family: &str, text: &str, size: f32, wrap: Option<f64>, color: Color32) -> Option<egui::TextureHandle> {
    let key = format!("sample:{family}:{text}:{size}:{wrap:?}:{color:?}");
    if let Some(t) = app.ts.texture_cache.get(&key) {
        return Some(t.clone());
    }
    let mut fmt = vector_core::text::CharFormat::default();
    fmt.font = family.to_string();
    fmt.size = size as f64 * 0.75;
    let t = match wrap {
        Some(w) => vector_core::text::TextObj::paragraph(text, w * 0.75, 100000.0, fmt),
        None => vector_core::text::TextObj::artistic(text, fmt),
    };
    let lay = vector_core::text::layout(&t);
    let path = lay.to_path();
    // the line box across, the ink down (a paragraph's frame is far taller than its lines)
    let ink = kurbo::Shape::bounding_box(&path);
    let bb = if wrap.is_some() { kurbo::Rect::new(lay.bounds.x0.min(ink.x0), ink.y0 - 2.0, lay.bounds.x1.max(ink.x1), ink.y1 + 2.0) } else { lay.bounds };
    if bb.width() <= 0.0 || bb.height() <= 0.0 {
        return None;
    }
    let scale = 1.3333 * ctx.pixels_per_point() as f64;
    let w = (bb.width() * scale).ceil() as u32 + 4;
    let h = (bb.height() * scale).ceil() as u32 + 4;
    let mut pm = tiny_skia::Pixmap::new(w.min(4000), h.min(2000))?;
    let xf = kurbo::Affine::translate((2.0, 2.0)) * kurbo::Affine::scale(scale) * kurbo::Affine::translate((-bb.x0, -bb.y0));
    let sk = vector_render::to_sk_path(&(xf * path))?;
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(color.r(), color.g(), color.b(), 255);
    paint.anti_alias = true;
    pm.fill_path(&sk, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
    let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
    let tex = ctx.load_texture(key.clone(), img, egui::TextureOptions::LINEAR);
    app.ts.texture_cache.insert(key, tex.clone());
    Some(tex)
}

// ------------------------------------------------------------------------------ Glyphs

/// The character filter: Entire Font, the common categories, then the scripts.
const COMMON: [&str; 7] = ["Arrows", "Currency", "Mathematical Symbols", "Marks", "Numbers", "Punctuation & Separators", "Symbols"];
const SCRIPTS: [&str; 6] = ["Latin", "Greek", "Cyrillic", "Hebrew", "Arabic", "Thai"];

/// What the Glyphs docker keeps between frames.
pub struct GlUi {
    pub font: String,
    /// The categories checked (none: the entire font).
    pub filter: Vec<&'static str>,
    pub sel: Option<char>,
    pub recent: Vec<(String, char)>,
    pub size: f32,
    pub info: bool,
    pub variations: bool,
}

impl Default for GlUi {
    fn default() -> Self {
        GlUi { font: vector_core::text::default_font_family(), filter: vec![], sel: None, recent: vec![], size: 38.0, info: false, variations: false }
    }
}

fn in_ranges(c: char, r: &[(u32, u32)]) -> bool {
    let u = c as u32;
    r.iter().any(|(a, b)| u >= *a && u <= *b)
}

fn category(c: char, name: &str) -> bool {
    match name {
        "Arrows" => in_ranges(c, &[(0x2190, 0x21FF), (0x27F0, 0x27FF), (0x2900, 0x297F), (0x2B00, 0x2BFF)]),
        "Currency" => in_ranges(c, &[(0x20A0, 0x20CF)]) || "$¢£¤¥".contains(c),
        "Mathematical Symbols" => in_ranges(c, &[(0x2200, 0x22FF), (0x27C0, 0x27EF), (0x2980, 0x2AFF)]) || "+<=>|~¬±×÷".contains(c),
        "Marks" => in_ranges(c, &[(0x0300, 0x036F), (0x1AB0, 0x1AFF), (0x20D0, 0x20FF)]),
        "Numbers" => c.is_numeric(),
        "Punctuation & Separators" => (c.is_ascii_punctuation() && !"$+<=>|~".contains(c)) || c.is_whitespace() || in_ranges(c, &[(0x2000, 0x206F), (0x3000, 0x303F)]) || "¡§«¶·»¿".contains(c),
        "Symbols" => in_ranges(c, &[(0x2100, 0x214F), (0x2300, 0x23FF), (0x25A0, 0x25FF), (0x2600, 0x26FF), (0x2700, 0x27BF)]) || "©®°¦¨¯´¸".contains(c),
        "Latin" => c.is_alphabetic() && in_ranges(c, &[(0x0041, 0x024F), (0x1E00, 0x1EFF)]),
        "Greek" => in_ranges(c, &[(0x0370, 0x03FF), (0x1F00, 0x1FFF)]),
        "Cyrillic" => in_ranges(c, &[(0x0400, 0x052F)]),
        "Hebrew" => in_ranges(c, &[(0x0590, 0x05FF)]),
        "Arabic" => in_ranges(c, &[(0x0600, 0x06FF), (0x0750, 0x077F)]),
        "Thai" => in_ranges(c, &[(0x0E00, 0x0E7F)]),
        _ => true,
    }
}

/// Insert a glyph into the text being edited, else as new artistic text; remember it.
fn use_glyph(app: &mut VectorApp, st: &mut GlUi, font: &str, c: char) {
    crate::tools::insert_text(app, &c.to_string(), Some(font));
    st.recent.retain(|x| !(x.0 == font && x.1 == c));
    st.recent.insert(0, (font.to_string(), c));
    st.recent.truncate(40);
}

pub fn glyphs(app: &mut VectorApp, ui: &mut Ui) {
    let mut st = std::mem::take(&mut app.dockers.ui.gl);
    ui.add_space(2.0);
    ui.label("Recent glyphs:");
    // the recent glyphs
    let w = ui.available_width() - 6.0;
    let (rr, _) = ui.allocate_exact_size(vec2(w, 72.0), Sense::hover());
    ui.painter().rect_stroke(rr, 0.0, Stroke::new(1.0, crate::theme::SEP), egui::StrokeKind::Inside);
    let mut use_recent = None;
    if st.recent.is_empty() {
        ui.painter().text(pos2(rr.center().x, rr.top() + 20.0), egui::Align2::CENTER_CENTER, "Shows the most recently used glyphs", egui::FontId::proportional(11.5), crate::theme::TEXT_DIM);
    } else {
        let cell = 30.0;
        let cols = ((rr.width() - 4.0) / cell).floor().max(1.0) as usize;
        for (k, (font, c)) in st.recent.clone().iter().enumerate().take(cols * 2) {
            let cr = Rect::from_min_size(rr.min + vec2(2.0 + (k % cols) as f32 * cell, 4.0 + (k / cols) as f32 * cell), vec2(cell, cell));
            let resp = ui.interact(cr, ui.scope_id().with(("gl_recent", k)), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(cr, 2.0, crate::theme::HOVER);
            }
            if let Some(t) = sample_texture(app, ui.ctx(), font, &c.to_string(), 20.0) {
                let s = (t.size_vec2() / ui.ctx().pixels_per_point()).min(vec2(cell - 6.0, cell - 6.0));
                ui.painter().image(t.id(), Rect::from_center_size(cr.center(), s), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            if resp.on_hover_text(format!("{font}: U+{:04X}", *c as u32)).double_clicked() {
                use_recent = Some((font.clone(), *c));
            }
        }
    }
    if let Some((f, c)) = use_recent {
        use_glyph(app, &mut st, &f, c);
    }
    ui.add_space(6.0);
    if let Some(f) = font_combo(ui, "gl_font", &st.font.clone(), w - 4.0) {
        st.font = f;
        st.sel = None;
    }
    // the character filter
    let label = if st.filter.is_empty() { "Entire Font".to_string() } else { st.filter.join(", ") };
    let mut filter_open = false;
    egui::ComboBox::from_id_salt("gl_filter")
        .width(w - 4.0)
        .selected_text(label)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            filter_open = true;
            let mut all = st.filter.is_empty();
            if ui.checkbox(&mut all, "Entire Font").changed() && all {
                st.filter.clear();
            }
            for (group, names) in [("Common", &COMMON[..]), ("Scripts", &SCRIPTS[..])] {
                ui.label(egui::RichText::new(group).color(egui::Color32::from_rgb(0x3d, 0x9b, 0xff)));
                for n in names {
                    let mut on = st.filter.contains(n);
                    if ui.checkbox(&mut on, *n).changed() {
                        if on {
                            st.filter.push(n);
                        } else {
                            st.filter.retain(|x| x != n);
                        }
                    }
                }
            }
            if ui.add_sized(vec2(ui.available_width(), 22.0), egui::Button::new("Close")).clicked() {
                ui.close();
            }
        })
        .response
        .on_hover_text("Find characters and glyphs for specific OpenType features, languages, categories, and more.");
    let _ = filter_open;
    ui.add_space(4.0);
    // the glyphs, the glyph info, the bottom bar
    let chars: Vec<char> = vector_core::text::font_chars(&st.font, false, false, 6000).into_iter().filter(|c| st.filter.is_empty() || st.filter.iter().any(|n| category(*c, n))).collect();
    let info_h = if st.info { 120.0 } else { 0.0 };
    let grid_h = (ui.clip_rect().bottom() - ui.cursor().top() - 54.0 - info_h).max(80.0);
    let cell = st.size + 6.0;
    let font = st.font.clone();
    let mut insert = None;
    egui::ScrollArea::vertical().id_salt("gl_grid").max_height(grid_h).min_scrolled_height(grid_h).auto_shrink([false, false]).show(ui, |ui| {
        let cols = ((ui.available_width() / cell).floor() as usize).max(3);
        let rows = chars.len().div_ceil(cols);
        let (area, _) = ui.allocate_exact_size(vec2(cols as f32 * cell, rows as f32 * cell), Sense::hover());
        let vis = ui.clip_rect();
        for (k, c) in chars.iter().enumerate() {
            let cr = Rect::from_min_size(area.min + vec2((k % cols) as f32 * cell, (k / cols) as f32 * cell), vec2(cell, cell));
            if !vis.intersects(cr) {
                continue;
            }
            let resp = ui.interact(cr, ui.scope_id().with(("gl", k)), Sense::click());
            if st.sel == Some(*c) {
                ui.painter().rect_filled(cr.shrink(1.0), 2.0, crate::theme::ACCENT);
            } else if resp.hovered() {
                ui.painter().rect_filled(cr.shrink(1.0), 2.0, crate::theme::HOVER);
            }
            if let Some(t) = sample_texture_wrapped(app, ui.ctx(), &font, &c.to_string(), st.size * 0.75, None, Color32::WHITE) {
                let s = (t.size_vec2() / ui.ctx().pixels_per_point()).min(vec2(cell - 4.0, cell - 4.0));
                ui.painter().image(t.id(), Rect::from_center_size(cr.center(), s), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            let resp = resp.on_hover_text(format!("U+{:04X}", *c as u32));
            if resp.clicked() {
                st.sel = Some(*c);
            }
            if resp.double_clicked() {
                insert = Some(*c);
            }
        }
    });
    if let Some(c) = insert {
        use_glyph(app, &mut st, &font, c);
    }
    // the fold of the glyph info
    let (fr, fresp) = ui.allocate_exact_size(vec2(ui.available_width() - 6.0, 12.0), Sense::click());
    let col = if fresp.hovered() { Color32::WHITE } else { crate::theme::TEXT_DIM };
    if st.info {
        crate::widgets::chevron_up(ui.painter(), fr.center(), 3.5, col);
    } else {
        crate::widgets::chevron(ui.painter(), fr.center(), 3.5, col);
    }
    if fresp.clicked() {
        st.info = !st.info;
    }
    if st.info {
        let gi = st.sel.and_then(|c| vector_core::text::glyph_info(&font, c).map(|g| (c, g)));
        egui::Grid::new("gl_info").num_columns(2).spacing(vec2(8.0, 2.0)).show(ui, |ui| {
            let v = |x: Option<String>| x.unwrap_or_default();
            let rows = [
                ("Name:", v(gi.as_ref().and_then(|(_, g)| g.1.clone()))),
                ("Glyph ID:", v(gi.as_ref().map(|(_, g)| g.0.to_string()))),
                ("Unicode:", v(gi.as_ref().map(|(c, _)| format!("U+{:04X}", *c as u32)))),
                ("Keystroke:", v(gi.as_ref().filter(|(c, _)| (0x80..0x100).contains(&(*c as u32))).map(|(c, _)| format!("Alt+0{}", *c as u32)))),
                ("OpenType:", String::new()),
                ("Language:", String::new()),
            ];
            for (k, val) in rows {
                ui.label(egui::RichText::new(k).color(crate::theme::TEXT_DIM));
                ui.label(val);
                ui.end_row();
            }
        });
    }
    // the bottom: all variations, the glyph size
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if crate::widgets::icon_button(&mut app.icons, ui, "gl_variations", "Show all glyph variations.", false, st.variations).clicked() {
            st.variations = !st.variations;
        }
        ui.add_space(6.0);
        let mut s = st.size;
        zoom(app, ui, &mut s, 20.0..=72.0, 4.0, (ui.available_width() - 60.0).max(60.0), "Change the glyph size in the Glyph Preview area.");
        st.size = s;
    });
    // Copy: the picked glyph on the clipboard
    if ui.ui_contains_pointer() && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::C)) {
        if let (Some(c), Ok(mut cb)) = (st.sel, arboard::Clipboard::new()) {
            let _ = cb.set_text(c.to_string());
        }
    }
    app.dockers.ui.gl = st;
}
