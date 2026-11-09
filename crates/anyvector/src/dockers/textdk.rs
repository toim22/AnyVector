//! The Text docker: the Character, Paragraph and Frame sections of the selected text (else of
//! new text), with a toolbar that jumps to a section, shows all sections or one at a time, and
//! shows or hides the style indicators (the small squares that revert a property to its style).

use super::{separator, spin};
use crate::app::AnyVectorApp;
use crate::commands::Cmd;
use crate::propbar::edit_sel;
use crate::propbar::text as pt;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use vector_core::style::Fill;
use vector_core::text::{CharFormat, TextAlign, TextKind, TextObj};
use vector_core::{Color, ObjKind, Object};

/// What the Text docker keeps between frames.
pub struct TextUi {
    /// The section jumped to (0 character, 1 paragraph, 2 frame).
    pub tab: u8,
    /// All sections in a column (else only the one jumped to).
    pub all: bool,
    /// The style indicators are shown.
    pub indicators: bool,
    /// The more-options fold of each section.
    pub more: [bool; 3],
    pub folded: [bool; 3],
    pub jump: bool,
    /// Vertical spacing units: 0 % of char. height, 1 points, 2 % of pt. size.
    pub spacing_unit: usize,
}

impl Default for TextUi {
    fn default() -> Self {
        TextUi { tab: 0, all: true, indicators: true, more: [false; 3], folded: [false; 3], jump: false, spacing_unit: 0 }
    }
}

const ROW: f32 = 26.0;

fn sel_obj(app: &AnyVectorApp) -> Option<Object> {
    let d = app.doc()?;
    let o = d.doc.find(*d.selection.first()?)?;
    matches!(o.kind, ObjKind::Text(_)).then(|| o.clone())
}

fn text_of(o: &Option<Object>) -> Option<&TextObj> {
    match o.as_ref().map(|o| &o.kind) {
        Some(ObjKind::Text(t)) => Some(t),
        _ => None,
    }
}

/// Change the selected text objects.
fn edit_text(app: &mut AnyVectorApp, label: &str, f: impl Fn(&mut TextObj)) {
    edit_sel(app, label, |o| {
        if let ObjKind::Text(t) = &mut o.kind {
            f(t);
        }
    });
}

/// A style indicator: a small square; clicked, it offers Revert to Style.
fn ind(app: &mut AnyVectorApp, ui: &mut Ui, on: bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(14.0, 18.0), if on { Sense::click() } else { Sense::hover() });
    if !on {
        return;
    }
    let c = if resp.hovered() { theme::TEXT } else { Color32::from_gray(96) };
    ui.painter().rect_stroke(Rect::from_center_size(r.center(), vec2(7.0, 7.0)), 0.0, Stroke::new(1.0, c), egui::StrokeKind::Inside);
    egui::Popup::menu(&resp).show(|ui| {
        crate::menus::menu_style(ui);
        let linked = super::ostyles::linked_style(app).is_some();
        if crate::menus::item(app, ui, "&Revert to style", linked, false) {
            app.run(Cmd::RevertToStyle);
        }
    });
}

/// The indicator at the right end of a row.
fn end_ind(app: &mut AnyVectorApp, ui: &mut Ui, on: bool) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ind(app, ui, on));
}

/// An icon (not a button) leading a field.
fn lead(app: &mut AnyVectorApp, ui: &mut Ui, icon: &str, tip: &str, enabled: bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(20.0, 22.0), Sense::hover());
    let _ = enabled;
    if let Some(t) = app.icons.get(ui.ctx(), icon, 16.0, crate::icons::Tone::Normal) {
        ui.painter().image(t.id, Rect::from_center_size(r.center(), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
}

fn tbtn(app: &mut AnyVectorApp, ui: &mut Ui, icon: &str, tip: &str, enabled: bool, on: bool) -> egui::Response {
    crate::widgets::icon_button(&mut app.icons, ui, icon, tip, enabled, on)
}

/// The "..." button of a row.
fn more_btn(ui: &mut Ui, tip: &str, enabled: bool) -> bool {
    ui.add_enabled_ui(enabled, |ui| ui.add_sized(vec2(22.0, 22.0), egui::Button::new("•••").frame(false)).on_hover_text(tip)).inner.clicked()
}

/// The fold arrow under a section's first controls.
fn fold(ui: &mut Ui, open: &mut bool) {
    let w = ui.available_width() - 14.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 14.0), Sense::click());
    let c = if resp.hovered() { Color32::WHITE } else { theme::TEXT_DIM };
    if *open {
        crate::widgets::chevron_up(ui.painter(), r.center(), 3.5, c);
    } else {
        crate::widgets::chevron(ui.painter(), r.center(), 3.5, c);
    }
    if resp.clicked() {
        *open = !*open;
    }
}

/// A numeric field that shows nothing when there is no value (`None`).
fn opt_spin(ui: &mut Ui, v: Option<f64>, range: std::ops::RangeInclusive<f64>, step: f64, decimals: usize, suffix: &str, w: f32) -> Option<f64> {
    match v {
        Some(mut x) => {
            let before = x;
            spin(ui, &mut x, range, step, decimals, suffix, w);
            ((x - before).abs() > 1e-9).then_some(x)
        }
        None => {
            ui.add_enabled_ui(false, |ui| {
                let mut s = String::new();
                ui.add_sized(vec2(w, 22.0), egui::TextEdit::singleline(&mut s));
            });
            None
        }
    }
}

/// A combo of names, `w` wide; returns the picked index.
fn combo(ui: &mut Ui, id: &str, cur: usize, items: &[&str], w: f32, enabled: &dyn Fn(usize) -> bool, tip: &str) -> Option<usize> {
    let mut out = None;
    let r = egui::ComboBox::from_id_salt(id).width(w).selected_text(items.get(cur).copied().unwrap_or("")).show_ui(ui, |ui| {
        for (i, t) in items.iter().enumerate() {
            if ui.add_enabled(enabled(i), egui::Button::selectable(i == cur, *t)).clicked() {
                out = Some(i);
            }
        }
    });
    if !tip.is_empty() {
        r.response.on_hover_text(tip);
    }
    out
}

pub fn text(app: &mut AnyVectorApp, ui: &mut Ui) {
    let mut st = std::mem::take(&mut app.dockers.ui.tx);
    toolbar(app, ui, &mut st);
    if !app.has_doc() {
        app.dockers.ui.tx = st;
        return super::no_doc(ui);
    }
    let o = sel_obj(app);
    let names = ["Character", "Paragraph", "Frame"];
    let sections: Vec<u8> = if st.all { vec![0, 1, 2] } else { vec![st.tab] };
    for (k, s) in sections.into_iter().enumerate() {
        if k > 0 {
            ui.add_space(8.0);
        }
        // the header: fold, title, the gear of Paragraph, the indicator
        let open = ui
            .horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 44.0, 20.0), Sense::click());
                let c = pos2(r.left() + 7.0, r.center().y);
                if st.folded[s as usize] {
                    crate::widgets::chevron(ui.painter(), c, 3.0, theme::TEXT);
                } else {
                    crate::widgets::chevron_up(ui.painter(), c, 3.0, theme::TEXT);
                }
                ui.painter().text(pos2(r.left() + 16.0, r.center().y), egui::Align2::LEFT_CENTER, names[s as usize], theme::bold(12.5), Color32::WHITE);
                if resp.clicked() {
                    st.folded[s as usize] = !st.folded[s as usize];
                }
                if st.jump && s == st.tab {
                    resp.scroll_to_me(Some(egui::Align::TOP));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ind(app, ui, st.indicators);
                    if s == 1 {
                        gear(app, ui, text_of(&o).is_some_and(|t| matches!(t.kind, TextKind::Paragraph { .. })));
                    }
                });
                !st.folded[s as usize]
            })
            .inner;
        if !open {
            continue;
        }
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.push_id(("tx_section", s), |ui| match s {
            0 => character(app, ui, &o, &mut st),
            1 => paragraph(app, ui, &o, &mut st),
            _ => frame(app, ui, &o, &mut st),
        });
    }
    st.jump = false;
    app.dockers.ui.tx = st;
}

fn toolbar(app: &mut AnyVectorApp, ui: &mut Ui, st: &mut TextUi) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        for (i, icon, tip) in [(0u8, "tx_char", "Jump to the character properties."), (1, "tx_para", "Jump to the paragraph properties."), (2, "tx_frame", "Jump to the frame properties.")] {
            if tbtn(app, ui, icon, tip, true, st.tab == i).clicked() {
                st.tab = i;
                st.jump = true;
                st.folded[i as usize] = false;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if tbtn(app, ui, if st.all { "pr_all" } else { "pr_tabbed" }, "Show or hide all properties.", true, st.all).clicked() {
                st.all = !st.all;
            }
            if tbtn(app, ui, "tx_styleind", "Display or hide the style indicators.", true, !st.indicators).clicked() {
                st.indicators = !st.indicators;
            }
        });
    });
    ui.add_space(4.0);
    separator(ui);
    ui.add_space(6.0);
}

/// Paragraph ▸ the gear: Justification, Tabs, Bullets and Numbering, Drop Cap, Hyphenation
/// Settings.
fn gear(app: &mut AnyVectorApp, ui: &mut Ui, para: bool) {
    let r = tbtn(app, ui, "gear", "Options", true, false);
    egui::Popup::menu(&r).show(|ui| {
        crate::menus::menu_style(ui);
        let _ = crate::menus::item(app, ui, "&Justification...", false, false);
        if crate::menus::item(app, ui, "&Tabs...", para, false) {
            app.run(Cmd::TextTabs);
        }
        if crate::menus::item(app, ui, "&Bullets and Numbering...", para, false) {
            app.run(Cmd::TextBullets);
        }
        if crate::menus::item(app, ui, "&Drop Cap...", para, false) {
            app.run(Cmd::TextDropCap);
        }
        if crate::menus::item(app, ui, "&Hyphenation Settings...", para, false) {
            app.run(Cmd::CustomHyphens);
        }
    });
}

// ------------------------------------------------------------------------------- Character

const STYLES: [&str; 4] = ["Normal", "Normal-Italic", "Bold", "Bold-Italic"];
const FILL_TYPES: [&str; 8] = ["No Fill", "Uniform Fill", "Fountain Fill", "2 Color Pattern", "Vector Pattern", "Bitmap Pattern", "Texture Fill", "PostScript Fill"];
const LINE_TYPES: [&str; 7] = ["(none)", "Single Thin", "Single Thin Word", "Single Thick", "Single Thick Word", "Double Thin", "Double Thin Word"];
const WIDTHS: [f64; 9] = [0.0, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 8.0];

fn character(app: &mut AnyVectorApp, ui: &mut Ui, o: &Option<Object>, st: &mut TextUi) {
    let f: CharFormat = pt::format(app);
    let has = o.is_some();
    let editing_range = app.doc().and_then(|d| d.text_edit.as_ref()).is_some_and(|te| te.caret != te.anchor);
    let i = st.indicators;
    let w = ui.available_width();
    let wide = (w - 134.0).max(120.0);
    // font and size
    ui.horizontal(|ui| {
        font_combo(app, ui, &f.font, wide);
        ind(app, ui, i);
        let mut size = f.size;
        if spin(ui, &mut size, 0.5..=3000.0, 1.0, 1, " pt", 64.0) {
            pt::apply(app, "Font Size", &move |c| c.size = size);
        }
        end_ind(app, ui, i);
    });
    // style and variable fonts
    ui.horizontal(|ui| {
        let cur = (f.bold as usize) * 2 + f.italic as usize;
        if let Some(k) = combo(ui, "tx_style", cur, &STYLES, wide, &|_| true, "Choose a font style for the text.") {
            pt::apply(app, "Font Style", &move |c| {
                c.bold = k >= 2;
                c.italic = k % 2 == 1;
            });
        }
        ind(app, ui, i);
        let axes = !vector_core::text::fonts().lock().variation_axes(&f.font, f.bold, f.italic).is_empty();
        ui.add_space((ui.available_width() - 44.0).max(0.0));
        let vr = tbtn(app, ui, "variable_font", "Adjust variable font properties.", axes, false);
        if axes {
            egui::Popup::menu(&vr).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| variable_axes(app, ui, &f));
        }
        end_ind(app, ui, i);
    });
    ui.add_space(8.0);
    // range kerning and underline
    ui.horizontal(|ui| {
        lead(app, ui, "tx_kern", "Range Kerning", editing_range);
        let v = editing_range.then_some(f.tracking);
        if let Some(x) = opt_spin(ui, v, -100.0..=500.0, 1.0, 0, " %", wide - 22.0) {
            pt::apply(app, "Range Kerning", &move |c| c.tracking = x);
        }
        ind(app, ui, i);
        ui.add_space((ui.available_width() - 44.0).max(0.0));
        let u = tbtn(app, ui, "underline", "Underline the text.", true, f.underline);
        crate::dockers::drop_mark(ui, u.rect);
        egui::Popup::menu(&u).show(|ui| {
            crate::menus::menu_style(ui);
            for (k, t) in LINE_TYPES.iter().enumerate() {
                let on = if k == 0 { !f.underline } else { k == 1 && f.underline };
                if crate::menus::item(app, ui, t, k <= 1, on) {
                    pt::apply(app, "Underline", &move |c| c.underline = k == 1);
                }
            }
        });
        end_ind(app, ui, i);
    });
    // fill, background, outline
    let fill_kind = o.as_ref().map(|o| super::props::fill_kind(&o.style.fill) as usize).unwrap_or(1);
    let fill_color = match o.as_ref().map(|o| &o.style.fill) {
        Some(Fill::Solid(c)) => Some(*c),
        Some(_) => None,
        None => Some(f.color.unwrap_or(Color::BLACK)),
    };
    ui.horizontal(|ui| {
        lead(app, ui, "tx_fill", "Fill", true);
        if let Some(k) = combo(ui, "tx_fill_type", fill_kind.min(7), &FILL_TYPES, wide - 30.0, &|k| has || k <= 1, "Choose the type of fill to apply to the characters.") {
            if has {
                let base = fill_color.unwrap_or(Color::BLACK);
                let nf = super::props::new_fill(app, k as u8, base);
                edit_sel(app, "Text Fill", |o| o.style.fill = nf.clone());
            }
        }
        let mut c = fill_color;
        if super::color_pick_w(ui, "tx_fill_color", &mut c, false, fill_kind == 1 || !has, 48.0) {
            if let Some(c) = c {
                if has {
                    edit_sel(app, "Text Color", |o| o.style.fill = Fill::Solid(c));
                } else {
                    app.ts.text_format.color = Some(c);
                }
            }
        }
        if more_btn(ui, "Fill", has) {
            open_props(app, 1);
        }
        end_ind(app, ui, i);
    });
    ui.horizontal(|ui| {
        lead(app, ui, "tx_bg", "Background Fill", false);
        ui.add_enabled_ui(false, |ui| {
            combo(ui, "tx_bg_type", 0, &FILL_TYPES, wide - 30.0, &|_| true, "Choose the type of background fill for the characters.");
            let mut c = Some(Color::BLACK);
            super::color_pick_w(ui, "tx_bg_color", &mut c, false, false, 48.0);
        });
        more_btn(ui, "Background Fill", false);
        end_ind(app, ui, i);
    });
    let ol = o.as_ref().map(|o| o.style.outline.clone());
    ui.horizontal(|ui| {
        lead(app, ui, "tx_outline", "Outline Width", has);
        let wnow = ol.as_ref().filter(|x| x.color.is_some()).map(|x| x.width);
        let names: Vec<String> = std::iter::once("None".to_string()).chain(WIDTHS.iter().skip(1).map(|w| format!("{} pt", trim(*w)))).collect();
        let cur = wnow.and_then(|w| WIDTHS.iter().position(|x| (x - w).abs() < 1e-6)).unwrap_or(0);
        let label = match wnow {
            Some(w) if cur == 0 => format!("{} pt", trim(w)),
            _ => names[cur].clone(),
        };
        ui.add_enabled_ui(has, |ui| {
            egui::ComboBox::from_id_salt("tx_ol_w").width(wide - 30.0).selected_text(label).show_ui(ui, |ui| {
                for (k, n) in names.iter().enumerate() {
                    if ui.selectable_label(k == cur && wnow.is_some() || k == 0 && wnow.is_none(), n).clicked() {
                        let w = WIDTHS[k];
                        edit_sel(app, "Outline Width", |o| {
                            if w == 0.0 {
                                o.style.outline.color = None;
                            } else {
                                o.style.outline.width = w;
                                if o.style.outline.color.is_none() {
                                    o.style.outline.color = Some(Color::BLACK);
                                }
                            }
                        });
                    }
                }
            })
            .response
            .on_hover_text("Set an outline width for the characters.");
        });
        let mut c = ol.as_ref().and_then(|x| x.color);
        if super::color_pick_w(ui, "tx_ol_color", &mut c, true, has, 48.0) {
            edit_sel(app, "Outline Color", |o| o.style.outline.color = c);
        }
        if more_btn(ui, "Outline", has) {
            open_props(app, 0);
        }
        end_ind(app, ui, i);
    });
    ui.add_space(4.0);
    fold(ui, &mut st.more[0]);
    if !st.more[0] {
        return;
    }
    ui.add_space(6.0);
    // position, caps, capital spacing, alternate annotation forms
    ui.horizontal(|ui| {
        let pos = tbtn(app, ui, if f.baseline_shift < 0.0 { "subscript" } else { "superscript" }, "Change the position of a selected character relative to the surrounding characters.", true, f.baseline_shift != 0.0);
        crate::dockers::drop_mark(ui, pos.rect);
        egui::Popup::menu(&pos).show(|ui| {
            crate::menus::menu_style(ui);
            let size = f.size;
            for (t, v) in [("None", 0.0), ("Superscript (auto)", size * 0.33), ("Subscript (auto)", -size * 0.2)] {
                let on = (f.baseline_shift - v).abs() < 1e-6;
                if crate::menus::item(app, ui, t, true, on) {
                    pt::apply(app, "Position", &move |c| c.baseline_shift = v);
                }
            }
            let _ = crate::menus::item(app, ui, "Superscript (synthesized)", false, false);
            let _ = crate::menus::item(app, ui, "Subscript (synthesized)", false, false);
        });
        ind(app, ui, i);
        let caps = tbtn(app, ui, "all_caps", "Change text capitalization.", true, f.caps);
        crate::dockers::drop_mark(ui, caps.rect);
        egui::Popup::menu(&caps).show(|ui| {
            crate::menus::menu_style(ui);
            if crate::menus::item(app, ui, "None", true, !f.caps) {
                pt::apply(app, "Caps", &|c| c.caps = false);
            }
            if crate::menus::item(app, ui, "All Caps", true, f.caps) {
                pt::apply(app, "Caps", &|c| c.caps = true);
            }
            for t in ["Titling Caps", "Small Caps (synthesized)", "All Small Caps", "Small Caps from Caps", "Petite Caps", "All Petite Caps", "Unicase"] {
                let _ = crate::menus::item(app, ui, t, false, false);
            }
        });
        ind(app, ui, i);
        tbtn(app, ui, "tx_capspace", "Capital Spacing", false, false);
        ind(app, ui, i);
        tbtn(app, ui, "tx_annot", "Alternate Annotation Forms", false, false);
        ind(app, ui, i);
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        lead(app, ui, "tx_voff", "Vertical Character Offset", true);
        let mut v = f.baseline_shift;
        if spin(ui, &mut v, -1000.0..=1000.0, 1.0, 1, " pt", 90.0) {
            pt::apply(app, "Vertical Character Offset", &move |c| c.baseline_shift = v);
        }
        ind(app, ui, i);
        lead(app, ui, "tx_strike", "Character Strikethrough", true);
        if let Some(k) = combo(ui, "tx_strike_t", f.strike as usize, &LINE_TYPES, 86.0, &|k| k <= 1, "Apply a line through the selected text.") {
            pt::apply(app, "Strikethrough", &move |c| c.strike = k == 1);
        }
        end_ind(app, ui, i);
    });
    ui.horizontal(|ui| {
        lead(app, ui, "tx_hoff", "Horizontal Character Offset", false);
        opt_spin(ui, None, 0.0..=1.0, 1.0, 0, "", 90.0);
        ind(app, ui, i);
        lead(app, ui, "tx_overline", "Character Overline", false);
        ui.add_enabled_ui(false, |ui| combo(ui, "tx_over_t", 0, &LINE_TYPES, 86.0, &|_| true, "Apply a line above the selected text."));
        end_ind(app, ui, i);
    });
    ui.horizontal(|ui| {
        lead(app, ui, "tx_angle", "Character Angle", false);
        opt_spin(ui, None, 0.0..=1.0, 1.0, 0, "", 90.0);
        ind(app, ui, i);
    });
    let (op_ol, op_fill) = o.as_ref().map(|o| (o.style.overprint_outline, o.style.overprint_fill)).unwrap_or((false, false));
    ui.horizontal(|ui| {
        let mut v = op_ol;
        if ui.add_enabled(has, egui::Checkbox::new(&mut v, "Overprint outline")).on_hover_text("Let the outline print on top of the underlying colors.").changed() {
            edit_sel(app, "Overprint Outline", |o| o.style.overprint_outline = v);
        }
        end_ind(app, ui, i);
    });
    ui.horizontal(|ui| {
        let mut v = op_fill;
        if ui.add_enabled(has, egui::Checkbox::new(&mut v, "Overprint fill")).on_hover_text("Let the fill print on top of the underlying colors.").changed() {
            edit_sel(app, "Overprint Fill", |o| o.style.overprint_fill = v);
        }
        end_ind(app, ui, i);
    });
    ui.add_space(6.0);
    separator(ui);
    ui.add_space(6.0);
    // OpenType features: none of them is available for the fonts
    for row in [[("tx_num", "Number Styles"), ("tx_frac", "Fraction")], [("tx_ord", "Ordinals"), ("tx_salt", "Stylistic Alternates")]] {
        ui.horizontal(|ui| {
            for (icon, tip) in row {
                tbtn(app, ui, icon, tip, false, false);
                ind(app, ui, i);
            }
        });
    }
}

/// The font list: the recent fonts, then all of them.
fn font_combo(app: &mut AnyVectorApp, ui: &mut Ui, cur: &str, w: f32) {
    let families: Vec<String> = vector_core::text::fonts().lock().families().to_vec();
    let recent = app.ts.recent_fonts.clone();
    let mut chosen: Option<String> = None;
    egui::ComboBox::from_id_salt("tx_font")
        .width(w)
        .height(420.0)
        .selected_text(cur)
        .show_ui(ui, |ui| {
            for f in &recent {
                if ui.selectable_label(f == cur, egui::RichText::new(f).strong()).clicked() {
                    chosen = Some(f.clone());
                }
            }
            if !recent.is_empty() {
                ui.separator();
            }
            for f in &families {
                if ui.selectable_label(f == cur, f).clicked() {
                    chosen = Some(f.clone());
                }
            }
        })
        .response
        .on_hover_text("Choose a typeface for new or selected text.");
    if let Some(f) = chosen {
        app.ts.recent_fonts.retain(|x| *x != f);
        app.ts.recent_fonts.insert(0, f.clone());
        app.ts.recent_fonts.truncate(app.settings.recent_fonts.max(1));
        pt::apply(app, "Font", &move |c| c.font = f.clone());
    }
}

/// Variable Fonts: a slider for each design axis of the font.
fn variable_axes(app: &mut AnyVectorApp, ui: &mut Ui, f: &CharFormat) {
    let axes = vector_core::text::fonts().lock().variation_axes(&f.font, f.bold, f.italic);
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
        pt::apply(app, "Variable Fonts", &move |c| c.variations = vals.clone());
    }
}

fn trim(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The "..." of Fill and Outline: the Properties docker at that section.
fn open_props(app: &mut AnyVectorApp, tab: u8) {
    app.dockers.ui.pr.tab = tab;
    app.dockers.ui.pr.jump = true;
    app.dockers.show(super::DockerId::Properties);
}

// ------------------------------------------------------------------------------- Paragraph

fn paragraph(app: &mut AnyVectorApp, ui: &mut Ui, o: &Option<Object>, st: &mut TextUi) {
    let t = text_of(o).cloned();
    let has = t.is_some();
    let para = t.as_ref().is_some_and(|t| matches!(t.kind, TextKind::Paragraph { .. }));
    let i = st.indicators;
    let unit = app.doc().map(|d| d.doc.settings.unit).unwrap_or_default();
    let dpi = app.doc().map(|d| d.doc.settings.dpi).unwrap_or(96.0);
    let cur = pt::align(app);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (icon, tip, a) in [
            ("align_text_none", "Do not align text with the text frame.", None),
            ("align_text_left", "Align text with the left side of the text frame.", Some(TextAlign::Left)),
            ("align_text_center", "Center text between the left and right sides of the text frame.", Some(TextAlign::Center)),
            ("align_text_right", "Align text with the right side of the text frame.", Some(TextAlign::Right)),
            ("align_text_justify", "Align text, excluding the last line, with the left and right sides of the text frame.", Some(TextAlign::Justify)),
            ("align_text_force", "Align text with both left and right sides of the text frame.", Some(TextAlign::ForceJustify)),
        ] {
            let on = match a {
                None => false,
                Some(a) => a == cur,
            };
            if tbtn(app, ui, icon, tip, a.is_some(), on).clicked() {
                if let Some(a) = a {
                    pt::set_align(app, a);
                }
            }
        }
        end_ind(app, ui, i);
    });
    ui.add_space(6.0);
    let half = ((ui.available_width() - 60.0) / 2.0 - 24.0).max(60.0);
    ui.horizontal(|ui| {
        lead(app, ui, "tx_linesp", "Line Spacing", true);
        let mut v = t.as_ref().map(|t| t.line_spacing).unwrap_or(100.0);
        ui.add_enabled_ui(has, |ui| {
            if spin(ui, &mut v, 0.0..=2000.0, 1.0, 1, " %", half) {
                edit_text(app, "Line Spacing", |t| t.line_spacing = v);
            }
        });
        ind(app, ui, i);
        if let Some(k) = combo(ui, "tx_sp_unit", st.spacing_unit, &["% of Char. height", "Points", "% of Pt. size"], half, &|k| k == 0, "Set the unit of measurement for spacing text.") {
            st.spacing_unit = k;
        }
        end_ind(app, ui, i);
    });
    // indents and paragraph spacing
    let field = |app: &mut AnyVectorApp, ui: &mut Ui, id: &str, icon: &str, tip: &str, v: Option<f64>, set: fn(&mut TextObj, f64)| {
        lead(app, ui, icon, tip, v.is_some());
        match v {
            Some(mut x) => {
                if crate::widgets::unit_field(ui, ui.make_persistent_id(id), &mut x, unit, dpi, half, true) {
                    edit_text(app, tip, move |t| set(t, x));
                }
            }
            None => {
                opt_spin(ui, None, 0.0..=1.0, 1.0, 0, "", half);
            }
        }
        ind(app, ui, i);
    };
    let pv = |f: fn(&TextObj) -> f64| t.as_ref().filter(|_| para).map(f);
    ui.horizontal(|ui| {
        field(app, ui, "tx_li", "tx_lindent", "Left indent", pv(|t| t.left_indent), |t, v| t.left_indent = v);
        lead(app, ui, "tx_before", "Before Paragraph Spacing", false);
        opt_spin(ui, None, 0.0..=1.0, 1.0, 0, "", half);
        end_ind(app, ui, i);
    });
    ui.horizontal(|ui| {
        field(app, ui, "tx_fi", "tx_findent", "First Line", pv(|t| t.first_indent), |t, v| t.first_indent = v);
        lead(app, ui, "tx_after", "After Paragraph Spacing", has);
        if let Some(x) = opt_spin(ui, t.as_ref().map(|t| t.para_spacing), 0.0..=2000.0, 1.0, 1, " %", half) {
            edit_text(app, "After Paragraph Spacing", |t| t.para_spacing = x);
        }
        end_ind(app, ui, i);
    });
    ui.horizontal(|ui| {
        field(app, ui, "tx_ri", "tx_rindent", "Right indent", pv(|t| t.right_indent), |t, v| t.right_indent = v);
    });
    ui.add_space(4.0);
    fold(ui, &mut st.more[1]);
    if !st.more[1] {
        return;
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        lead(app, ui, "tx_charsp", "Character Spacing", has);
        if let Some(x) = opt_spin(ui, t.as_ref().map(|t| t.format.tracking), -100.0..=2000.0, 1.0, 1, " %", half) {
            edit_text(app, "Character Spacing", |t| {
                t.format.tracking = x;
                for s in t.spans.iter_mut() {
                    s.format.tracking = x;
                }
            });
        }
        ind(app, ui, i);
        lead(app, ui, "tx_langsp", "Language", false);
        opt_spin(ui, None, 0.0..=1.0, 1.0, 0, "", half);
        end_ind(app, ui, i);
    });
    ui.horizontal(|ui| {
        lead(app, ui, "tx_wordsp", "Word", has);
        if let Some(x) = opt_spin(ui, t.as_ref().map(|t| t.word_spacing), 0.0..=2000.0, 1.0, 1, " %", half) {
            edit_text(app, "Word Spacing", |t| t.word_spacing = x);
        }
        ind(app, ui, i);
    });
    ui.add_space(4.0);
    let list = t.as_ref().and_then(|t| t.list.clone());
    ui.horizontal(|ui| {
        if tbtn(app, ui, "bullets", "Add or remove bulleted-list formatting.", para, list.as_ref().is_some_and(|l| !l.numbers)).clicked() {
            let on = !list.as_ref().is_some_and(|l| !l.numbers);
            edit_text(app, "Bullets", move |t| t.list = on.then(Default::default));
        }
        ind(app, ui, i);
        if tbtn(app, ui, "numbered_list", "Add or remove numbered-list formatting.", para, list.as_ref().is_some_and(|l| l.numbers)).clicked() {
            let on = !list.as_ref().is_some_and(|l| l.numbers);
            edit_text(app, "Numbers", move |t| {
                t.list = on.then(|| vector_core::text::ListFormat { numbers: true, ..Default::default() });
            });
        }
        ind(app, ui, i);
        let lvl = list.as_ref().map(|l| l.level).unwrap_or(1);
        if tbtn(app, ui, "indent_increase", "Move the list item to the right.", list.is_some() && lvl < 9, false).clicked() {
            edit_text(app, "Increase Indent", |t| {
                if let Some(l) = t.list.as_mut() {
                    l.level = (l.level + 1).min(9);
                }
            });
        }
        ind(app, ui, i);
        if tbtn(app, ui, "indent_decrease", "Move the list item to the left.", list.is_some() && lvl > 1, false).clicked() {
            edit_text(app, "Decrease Indent", |t| {
                if let Some(l) = t.list.as_mut() {
                    l.level = l.level.saturating_sub(1).max(1);
                }
            });
        }
        ind(app, ui, i);
        let dc = t.as_ref().is_some_and(|t| t.drop_cap.is_some());
        if tbtn(app, ui, "drop_cap", "Add or remove a drop cap.", para, dc).clicked() {
            edit_text(app, "Drop Cap", move |t| t.drop_cap = (!dc).then(Default::default));
        }
        ind(app, ui, i);
    });
    ui.horizontal(|ui| {
        if ui.add_enabled(para, egui::Button::new("Tabs...")).on_hover_text("Modify tab settings.").clicked() {
            app.run(Cmd::TextTabs);
        }
        end_ind(app, ui, i);
    });
    ui.horizontal(|ui| {
        let mut h = t.as_ref().is_some_and(|t| t.hyphenate);
        if ui.add_enabled(para, egui::Checkbox::new(&mut h, "Use hyphenation")).changed() {
            edit_text(app, "Hyphenation", move |t| t.hyphenate = h);
        }
        end_ind(app, ui, i);
    });
    ui.horizontal(|ui| {
        ui.label("Text Direction:");
        tbtn(app, ui, "tx_ltr", "Make text flow from left to right", false, true);
        tbtn(app, ui, "tx_rtl", "Make text flow from right to left", false, false);
        end_ind(app, ui, i);
    });
}

// ------------------------------------------------------------------------------- Frame

fn frame(app: &mut AnyVectorApp, ui: &mut Ui, o: &Option<Object>, st: &mut TextUi) {
    let t = text_of(o).cloned();
    let para = t.as_ref().is_some_and(|t| matches!(t.kind, TextKind::Paragraph { .. }));
    let i = st.indicators;
    ui.horizontal(|ui| {
        lead(app, ui, "tx_framebg", "Background Color", false);
        let mut c = None;
        super::color_pick_w(ui, "tx_frame_bg", &mut c, true, false, 48.0);
        ind(app, ui, i);
        let g = t.as_ref().is_some_and(|t| t.align_grid);
        if tbtn(app, ui, "baseline_grid", "Align the text inside the text frame to the document's baseline grid.", para, g).clicked() {
            edit_text(app, "Align to Baseline Grid", move |t| t.align_grid = !g);
        }
        ind(app, ui, i);
        let va = tbtn(app, ui, "valign_top", "Choose a method of aligning text vertically.", false, false);
        crate::dockers::drop_mark(ui, va.rect);
        ind(app, ui, i);
    });
    ui.horizontal(|ui| {
        lead(app, ui, "columns", "Number Of Columns", para);
        let mut n = t.as_ref().map(|t| t.columns.max(1)).unwrap_or(1);
        ui.add_enabled_ui(para, |ui| {
            if spin(ui, &mut n, 1..=64, 1.0, 0, "", 50.0) {
                edit_text(app, "Columns", move |t| t.columns = n);
            }
        });
        ind(app, ui, i);
        let eq = t.as_ref().is_some_and(|t| t.column_widths.is_empty());
        if tbtn(app, ui, "tx_eqcols", "Adjust the width of text frame columns equally.", para && n > 1, eq).clicked() {
            edit_text(app, "Equal Column Width", |t| t.column_widths.clear());
        }
        ind(app, ui, i);
        ui.add_space((ui.available_width() - 44.0).max(0.0));
        if more_btn(ui, "Modify column settings.", para) {
            app.run(Cmd::TextColumns);
        }
        end_ind(app, ui, i);
    });
    ui.add_space(4.0);
    fold(ui, &mut st.more[2]);
    if !st.more[2] {
        return;
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        tbtn(app, ui, "tx_rtlcols", "Apply right-to-left text flow to the columns.", false, false);
        end_ind(app, ui, i);
    });
    ui.horizontal(|ui| {
        ui.label("Text Orientation:");
        let v = t.as_ref().is_some_and(|t| t.vertical);
        let has = t.is_some();
        for (icon, tip, val) in [("text_horizontal", "Changes the text to horizontal direction", false), ("text_vertical", "Changes the text to vertical", true)] {
            if tbtn(app, ui, icon, tip, has, has && v == val).clicked() {
                edit_text(app, "Text Orientation", move |t| t.vertical = val);
            }
        }
        end_ind(app, ui, i);
    });
    let _ = ROW;
}
