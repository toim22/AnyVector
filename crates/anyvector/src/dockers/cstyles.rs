//! Color Styles docker: the document's color styles and color harmonies, with the Harmony Editor
//! and the Color Editor of the selection.

use super::coled::{self, ColorEd};
use super::{drop_mark, icon_button, icon_button_resp, separator};
use crate::app::AnyVectorApp;
use crate::menus::item;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use vector_core::document::{ColorHarmony, Document};
use vector_core::{Color, NamedColor};

/// A color style: `(None, i)` in the styles, `(Some(h), i)` in harmony `h`.
pub type StyleRef = (Option<usize>, usize);

#[derive(Clone)]
pub struct CsUi {
    /// Selected styles, in selection order.
    pub sel: Vec<StyleRef>,
    /// Selected harmony folder.
    pub harmony: Option<usize>,
    pub large: bool,
    pub empty_arms: bool,
    pub hint: bool,
    pub harmony_open: bool,
    pub editor_open: bool,
    pub distribute: bool,
    pub ed: ColorEd,
    /// New Gradient: number of colors, similarity and shades (0 lighter, 1 darker, 2 both).
    pub grad: (u32, u32, u8),
}

impl Default for CsUi {
    fn default() -> Self {
        CsUi { sel: vec![], harmony: None, large: false, empty_arms: true, hint: false, harmony_open: true, editor_open: false, distribute: true, ed: ColorEd::default(), grad: (5, 50, 2) }
    }
}

pub const RULES: [&str; 7] = ["Custom", "Analogous - Accented", "Analogous", "Complementary", "Monochromatic", "Tetrad", "Triad"];
const RULE_TIPS: [&str; 7] = [
    "Custom starts a new harmony folder or removes a harmony rule.",
    "Analogous - Accented adds the complement of the middle hue to the analogous hues.",
    "Analogous uses hues next to each other on the color wheel.",
    "Complementary uses hues opposite each other on the color wheel.",
    "Monochromatic uses a single hue.",
    "Tetrad uses two pairs of complementary hues.",
    "Triad uses three hues evenly spaced on the color wheel.",
];

fn style(doc: &Document, r: StyleRef) -> Option<&NamedColor> {
    match r.0 {
        None => doc.color_styles.get(r.1),
        Some(h) => doc.color_harmonies.get(h)?.styles.get(r.1),
    }
}

fn style_mut(doc: &mut Document, r: StyleRef) -> Option<&mut NamedColor> {
    match r.0 {
        None => doc.color_styles.get_mut(r.1),
        Some(h) => doc.color_harmonies.get_mut(h)?.styles.get_mut(r.1),
    }
}

/// Recolors every object that uses `old` (the link between a style and its objects).
fn recolor(doc: &mut Document, map: &[(Color, Color)]) {
    for p in doc.pages.iter_mut() {
        for l in p.layers.iter_mut() {
            for o in l.objects.iter_mut() {
                o.walk_mut(&mut |x| {
                    for (old, new) in map {
                        if x.style.fill == vector_core::Fill::Solid(*old) {
                            x.style.fill = vector_core::Fill::Solid(*new);
                        }
                        if x.style.outline.color == Some(*old) {
                            x.style.outline.color = Some(*new);
                        }
                    }
                });
            }
        }
    }
}

/// Sets the colors of styles, recoloring their objects.
fn set_colors(app: &mut AnyVectorApp, label: &str, changes: Vec<(StyleRef, Color)>) {
    let Some(d) = app.doc_mut() else { return };
    d.modify(label, |doc, _| {
        let mut map = vec![];
        for (r, c) in &changes {
            if let Some(s) = style_mut(doc, *r) {
                if s.color != *c {
                    map.push((s.color, *c));
                    s.color = *c;
                }
            }
        }
        recolor(doc, &map);
    });
}

/// The hues of a harmony rule around `base` (degrees), for `n` colors.
fn rule_hues(rule: u8, base: f64, n: usize) -> Vec<f64> {
    let offs: &[f64] = match rule {
        1 => &[0.0, -30.0, 30.0, 180.0],
        2 => &[0.0, -30.0, 30.0, -60.0, 60.0],
        3 => &[0.0, 180.0],
        4 => &[0.0],
        5 => &[0.0, 90.0, 180.0, 270.0],
        6 => &[0.0, 120.0, 240.0],
        _ => return vec![],
    };
    (0..n).map(|i| (base + offs[i % offs.len()]).rem_euclid(360.0)).collect()
}

/// Applies a harmony rule: the hues of its styles follow the first style's hue.
fn apply_rule(app: &mut AnyVectorApp, h: usize, rule: u8) {
    let Some(d) = app.doc() else { return };
    let Some(hm) = d.doc.color_harmonies.get(h) else { return };
    let mut changes = vec![];
    if let Some(first) = hm.styles.first() {
        let base = first.color.to_hsb()[0];
        let hues = rule_hues(rule, base, hm.styles.len());
        for (i, s) in hm.styles.iter().enumerate() {
            if let Some(hue) = hues.get(i) {
                let hsb = s.color.to_hsb();
                changes.push(((Some(h), i), Color::from_hsb(*hue, hsb[1], hsb[2]).with_alpha(s.color.a)));
            }
        }
    }
    if let Some(d) = app.doc_mut() {
        d.modify("Harmony Rule", |doc, _| doc.color_harmonies[h].rule = rule);
    }
    set_colors(app, "Harmony Rule", changes);
}

fn selection_colors(app: &AnyVectorApp) -> Vec<Color> {
    let Some(d) = app.doc() else { return vec![] };
    let mut v: Vec<Color> = vec![];
    for id in &d.selection {
        if let Some(o) = d.doc.find(*id) {
            o.walk(&mut |x| {
                for c in [x.style.fill.main_color(), x.style.outline.color].into_iter().flatten() {
                    if !v.contains(&c) {
                        v.push(c);
                    }
                }
            });
        }
    }
    v
}

fn add_styles(app: &mut AnyVectorApp, label: &str, colors: Vec<Color>, harmony: bool) {
    let Some(d) = app.doc_mut() else { return };
    let mut new = vec![];
    d.modify(label, |doc, _| {
        let mut list = vec![];
        for c in colors {
            let exists = doc.color_styles.iter().chain(doc.color_harmonies.iter().flat_map(|h| h.styles.iter())).any(|s| s.color == c);
            if !exists || harmony {
                let n = doc.color_styles.len() + doc.color_harmonies.iter().map(|h| h.styles.len()).sum::<usize>() + list.len() + 1;
                list.push(NamedColor::new(format!("Color Style {n}"), c));
            }
        }
        if harmony {
            let n = doc.color_harmonies.len() + 1;
            let len = list.len();
            doc.color_harmonies.push(ColorHarmony { name: format!("Harmony {n}"), rule: 0, styles: list });
            new = (0..len).map(|i| (Some(doc.color_harmonies.len() - 1), i)).collect();
        } else {
            let at = doc.color_styles.len();
            new = (at..at + list.len()).map(|i| (None, i)).collect();
            doc.color_styles.extend(list);
        }
    });
    app.dockers.ui.cs.sel = new;
}

pub fn color_styles(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return super::no_doc(ui);
    }
    let doc = app.doc().unwrap().doc.clone();
    let st = app.dockers.ui.cs.clone();
    let sel: Vec<StyleRef> = st.sel.iter().copied().filter(|r| style(&doc, *r).is_some()).collect();
    let harmony = st.harmony.filter(|h| *h < doc.color_harmonies.len());
    let has_sel = app.has_selection();
    let mut run: Option<&str> = None;
    // the name, View Options and Convert
    ui.horizontal(|ui| {
        let one = (sel.len() == 1).then(|| sel[0]);
        let mut name = match (one, harmony) {
            (Some(r), _) => style(&doc, r).map(|s| s.name.clone()).unwrap_or_default(),
            (None, Some(h)) => doc.color_harmonies[h].name.clone(),
            _ => String::new(),
        };
        let on = one.is_some() || harmony.is_some();
        let r = ui.add_enabled(on, egui::TextEdit::singleline(&mut name).desired_width(ui.available_width() - 60.0)).on_hover_text("Specify a new name for the color style.");
        if r.lost_focus() && on {
            if let Some(d) = app.doc_mut() {
                d.modify("Rename Color Style", |doc, _| match (one, harmony) {
                    (Some(r), _) => {
                        if let Some(s) = style_mut(doc, r) {
                            s.name = name.clone();
                        }
                    }
                    (None, Some(h)) => doc.color_harmonies[h].name = name.clone(),
                    _ => {}
                });
            }
        }
        let view = icon_button_resp(app, ui, "cs_view", "View Options", true);
        drop_mark(ui, view.rect);
        egui::Popup::menu(&view).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            crate::menus::menu_style(ui);
            let cs = &mut app.dockers.ui.cs;
            for (t, v) in [("Use large swatches", &mut cs.large), ("Show empty arms", &mut cs.empty_arms), ("Hint View", &mut cs.hint)] {
                if ui.add(egui::Button::selectable(*v, t)).on_hover_text(match t {
                    "Use large swatches" => "Increase the size of color swatches.",
                    "Show empty arms" => "Show arms that do not contain colors.",
                    _ => "Mark used color styles and objects associated with color styles.",
                }).clicked() {
                    *v = !*v;
                    ui.close();
                }
            }
        });
        let conv = icon_button_resp(app, ui, "cs_convert", "Convert the selected color styles to a different color mode or to spot colors.", !sel.is_empty());
        drop_mark(ui, conv.rect);
        egui::Popup::menu(&conv).show(|ui| {
            crate::menus::menu_style(ui);
            for (t, k) in [("Convert to &RGB", "rgb"), ("Convert to &CMYK", "cmyk"), ("Convert to &Grayscale", "gray"), ("Convert to &Spot", "spot")] {
                if item(app, ui, t, !sel.is_empty(), false) {
                    run = Some(k);
                }
            }
        });
    });
    // the styles and harmonies
    let sw = if st.large { 30.0 } else { 20.0 };
    let area_h = 150.0;
    let frame = egui::Frame::new().fill(Color32::from_gray(128)).stroke(Stroke::new(1.0, Color32::from_gray(200))).inner_margin(egui::Margin::same(4));
    let mut click: Option<(Option<StyleRef>, Option<usize>, bool)> = None;
    let mut dropped: Option<(Color, bool)> = None;
    frame.show(ui, |ui| {
        ui.set_min_size(vec2(ui.available_width(), area_h));
        egui::ScrollArea::vertical().id_salt("cs_grid").max_height(area_h).auto_shrink([false, false]).show(ui, |ui| {
            let full = ui.available_width();
            let ink = Color32::from_gray(245);
            let italic = |t: &str| egui::RichText::new(t).italics().color(ink).size(12.5);
            // the styles
            let zone_h: f32 = if doc.color_styles.is_empty() { 44.0 } else { 0.0 };
            let (zr, zresp) = ui.allocate_exact_size(vec2(full, zone_h.max(1.0)), Sense::click());
            if doc.color_styles.is_empty() {
                let p = ui.painter();
                if let Some(t) = app.icons.get(ui.ctx(), "cs_drop_style", 20.0, crate::icons::Tone::Normal) {
                    p.image(t.id, Rect::from_center_size(pos2(zr.left() + 34.0, zr.center().y), vec2(20.0, 20.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                ui.put(Rect::from_min_max(pos2(zr.left() + 56.0, zr.top()), zr.max), egui::Label::new(italic("Drag here to add color styles.")));
                if zresp.dnd_hover_payload::<crate::palette::ColorDrag>().is_some() {
                    ui.painter().rect_stroke(zr, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
                }
                if let Some(c) = zresp.dnd_release_payload::<crate::palette::ColorDrag>().and_then(|p| p.0) {
                    dropped = Some((c, false));
                }
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
                    for (i, s) in doc.color_styles.iter().enumerate() {
                        let r = (None, i);
                        let resp = swatch(ui, s, sw, sel.contains(&r), st.hint && doc.used_colors().contains(&s.color));
                        if resp.clicked() {
                            click = Some((Some(r), None, ui.input(|i| i.modifiers.ctrl || i.modifiers.shift)));
                        }
                    }
                });
                if let Some(c) = ui.response().dnd_release_payload::<crate::palette::ColorDrag>().and_then(|p| p.0) {
                    dropped = Some((c, false));
                }
            }
            let _ = zr;
            ui.painter().line_segment([pos2(ui.min_rect().left(), ui.cursor().top() + 2.0), pos2(ui.min_rect().left() + full, ui.cursor().top() + 2.0)], Stroke::new(1.0, Color32::from_gray(225)));
            ui.add_space(5.0);
            // the harmonies
            for (h, hm) in doc.color_harmonies.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
                    let (fr, fresp) = ui.allocate_exact_size(vec2(sw + 6.0, sw), Sense::click());
                    let p = ui.painter();
                    if harmony == Some(h) {
                        p.rect_filled(fr.expand(1.0), 2.0, theme::ACCENT());
                    }
                    if let Some(t) = app.icons.get(ui.ctx(), "cs_harmony_folder", sw - 2.0, crate::icons::Tone::Normal) {
                        p.image(t.id, Rect::from_center_size(fr.center(), vec2(sw - 2.0, sw - 2.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    let fresp = fresp.on_hover_text(format!("{}\n{}", hm.name, RULES[hm.rule as usize % RULES.len()]));
                    if fresp.clicked() {
                        click = Some((None, Some(h), false));
                    }
                    for (i, s) in hm.styles.iter().enumerate() {
                        let r = (Some(h), i);
                        let resp = swatch(ui, s, sw, sel.contains(&r), st.hint && doc.used_colors().contains(&s.color));
                        if resp.clicked() {
                            click = Some((Some(r), None, ui.input(|i| i.modifiers.ctrl || i.modifiers.shift)));
                        }
                    }
                });
                ui.add_space(3.0);
            }
            let rest = ui.available_height().max(60.0);
            let (hr, hresp) = ui.allocate_exact_size(vec2(full, rest), Sense::click());
            if doc.color_harmonies.is_empty() {
                let p = ui.painter();
                if let Some(t) = app.icons.get(ui.ctx(), "cs_drop_harmony", 20.0, crate::icons::Tone::Normal) {
                    p.image(t.id, Rect::from_center_size(pos2(hr.left() + 30.0, hr.top() + 34.0), vec2(20.0, 20.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                let tr = Rect::from_min_max(pos2(hr.left() + 40.0, hr.top() + 14.0), pos2(hr.right() - 4.0, hr.top() + 54.0));
                ui.put(tr, egui::Label::new(italic("Drag here to add color styles and\ngenerate a harmony.")).halign(egui::Align::Center));
            }
            if hresp.clicked() {
                click = Some((None, None, false));
            }
            if let Some(c) = hresp.dnd_release_payload::<crate::palette::ColorDrag>().and_then(|p| p.0) {
                dropped = Some((c, true));
            }
        });
    });
    // the splitter mark
    ui.vertical_centered(|ui| ui.label(egui::RichText::new("•••").size(7.0).color(theme::TEXT_DIM())));
    if let Some((r, h, add)) = click {
        let cs = &mut app.dockers.ui.cs;
        cs.harmony = h;
        match r {
            Some(r) if add => {
                if let Some(i) = cs.sel.iter().position(|x| *x == r) {
                    cs.sel.remove(i);
                } else {
                    cs.sel.push(r);
                }
            }
            Some(r) => cs.sel = vec![r],
            None => cs.sel.clear(),
        }
        if let Some(h) = h {
            cs.sel = (0..doc.color_harmonies[h].styles.len()).map(|i| (Some(h), i)).collect();
        }
    }
    if let Some((c, harmony)) = dropped {
        add_styles(app, "New Color Style", vec![c], harmony);
    }
    // the toolbar
    let one_style = sel.len() == 1;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let new = icon_button_resp(app, ui, "cs_new_style", "Create a color style.", true);
        drop_mark(ui, new.rect);
        egui::Popup::menu(&new).show(|ui| {
            crate::menus::menu_style(ui);
            if item(app, ui, "&New Color Style", true, false) {
                run = Some("new");
            }
            if item(app, ui, "&New from Selected...", has_sel, false) {
                run = Some("from_sel");
            }
            if item(app, ui, "New from &Document...", true, false) {
                run = Some("from_doc");
            }
        });
        let hm = icon_button_resp(app, ui, "cs_new_harmony", "Create a color harmony.", true);
        drop_mark(ui, hm.rect);
        egui::Popup::menu(&hm).show(|ui| {
            crate::menus::menu_style(ui);
            if item(app, ui, "New Color &Harmony", true, false) {
                run = Some("harmony");
            }
            if item(app, ui, "&Duplicate Harmony", harmony.is_some(), false) {
                run = Some("dup_harmony");
            }
            if item(app, ui, "New &Gradient", one_style, false) {
                run = Some("gradient");
            }
        });
        let (sr, _) = ui.allocate_exact_size(vec2(7.0, 22.0), Sense::hover());
        ui.painter().line_segment([pos2(sr.center().x, sr.top() + 3.0), pos2(sr.center().x, sr.bottom() - 3.0)], Stroke::new(1.0, theme::SEP()));
        if icon_button(app, ui, "cs_merge", "Merge the selected color styles with the last selected color style.", sel.len() > 1) {
            run = Some("merge");
        }
        if icon_button(app, ui, "cs_swap", "Swaps two selected color styles.", sel.len() == 2) {
            run = Some("swap");
        }
        let any = !doc.color_styles.is_empty() || !doc.color_harmonies.is_empty();
        if icon_button(app, ui, "cs_select_unused", "Select all color styles that are not in use by the current document.", any) {
            run = Some("unused");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(app, ui, "delete", "Delete the selected color style or color harmony.", !sel.is_empty() || harmony.is_some()) {
                run = Some("delete");
            }
            let io = icon_button_resp(app, ui, "cs_io", "Import or export style sheets, or save current styles as the defaults for new documents.", true);
            drop_mark(ui, io.rect);
            egui::Popup::menu(&io).show(|ui| {
                crate::menus::menu_style(ui);
                if item(app, ui, "&Import Style Sheet...", true, false) {
                    run = Some("import");
                }
                if item(app, ui, "&Export Style Sheet...", true, false) {
                    run = Some("export");
                }
                item(app, ui, "&Add Linked Style Sheet...", false, false);
                separator(ui);
                if item(app, ui, "&Set as New Document Defaults...", true, false) {
                    run = Some("defaults");
                }
            });
        });
    });
    egui::ScrollArea::vertical().id_salt("cs_edit").auto_shrink([false, false]).show(ui, |ui| {
        // the harmony rule (no harmony: a new harmony folder)
        let rule_of = harmony.map(|h| doc.color_harmonies[h].rule).unwrap_or(0) as usize;
        if harmony.is_none() {
            ui.add_space(4.0);
            if let Some(k) = rule_combo(ui, "cs_rule", rule_of, ui.available_width() - 6.0) {
                if k > 0 && !sel.is_empty() {
                    run = Some(["", "r1", "r2", "r3", "r4", "r5", "r6"][k]);
                }
            }
        } else if let Some(h) = harmony {
            let mut open = app.dockers.ui.cs.harmony_open;
            super::fold_header(ui, "Harmony Editor", &mut open);
            app.dockers.ui.cs.harmony_open = open;
            if open {
                ui.horizontal(|ui| {
                    if let Some(k) = rule_combo(ui, "cs_rule_h", rule_of, ui.available_width() - 34.0) {
                        apply_rule(app, h, k as u8);
                    }
                    let d = app.dockers.ui.cs.distribute;
                    if crate::widgets::icon_button(&mut app.icons, ui, "cs_distribute", "Distribute colors evenly when changing harmony rule.", true, d).clicked() {
                        app.dockers.ui.cs.distribute = !d;
                    }
                });
                // the harmony's colors on a hue wheel
                let n = doc.color_harmonies[h].styles.len();
                let size = (ui.available_width() - 12.0).min(180.0);
                let (wr, _) = ui.allocate_exact_size(vec2(ui.available_width(), size), Sense::hover());
                let c = wr.center();
                let rad = size / 2.0 - 6.0;
                let p = ui.painter();
                for k in 0..72 {
                    let a0 = k as f32 * 5.0;
                    let col = Color::from_hsb(a0 as f64, 100.0, 100.0);
                    let at = |a: f32, r: f32| pos2(c.x + r * (a - 90.0).to_radians().cos(), c.y + r * (a - 90.0).to_radians().sin());
                    p.line_segment([at(a0, rad - 8.0), at(a0, rad)], Stroke::new(rad * 0.09, theme::to_c32(col)));
                }
                for s in &doc.color_harmonies[h].styles {
                    let hsb = s.color.to_hsb();
                    let a = (hsb[0] as f32 - 90.0).to_radians();
                    let r = (rad - 14.0) * (hsb[1] as f32 / 100.0).max(0.15);
                    let pt = pos2(c.x + r * a.cos(), c.y + r * a.sin());
                    p.circle_filled(pt, 6.0, theme::to_c32(s.color));
                    p.circle_stroke(pt, 6.0, Stroke::new(1.5, Color32::WHITE));
                }
                if n == 0 {
                    p.text(c, egui::Align2::CENTER_CENTER, "Empty", egui::FontId::proportional(12.0), theme::TEXT_DIM());
                }
            }
        }
        // the color of the selected styles
        if !sel.is_empty() {
            let first = style(&doc, sel[0]).map(|s| s.color).unwrap_or(Color::rgb(0, 0, 0));
            let mut open = app.dockers.ui.cs.editor_open;
            super::fold_header(ui, "Color Editor", &mut open);
            app.dockers.ui.cs.editor_open = open;
            let mut c = first;
            let changed = if open {
                let mut ed = app.dockers.ui.cs.ed.clone();
                let ch = coled::editor(ui, &mut ed, &mut c, true);
                app.dockers.ui.cs.ed = ed;
                ch
            } else {
                // the components on one row
                let comps = coled::components(1, c);
                let mut vals: Vec<f64> = comps.iter().map(|x| x.1).collect();
                let mut ch = false;
                ui.horizontal(|ui| {
                    let w = ((ui.available_width() - 4.0 * 16.0) / 4.0).max(30.0);
                    for (k, (n, _, lo, hi)) in comps.iter().enumerate() {
                        ui.label(*n);
                        if super::spin(ui, &mut vals[k], *lo..=*hi, 1.0, 0, "", w) {
                            ch = true;
                        }
                    }
                });
                if ch {
                    c = coled::from_components(1, &vals, c);
                }
                ch
            };
            if changed {
                // every selected style moves by the change of the first one's components
                let changes = if sel.len() == 1 { vec![(sel[0], c)] } else { sel.iter().map(|r| (*r, c)).collect() };
                set_colors(app, "Edit Color Style", changes);
            }
        }
    });
    if let Some(r) = run {
        act(app, r, &doc, &sel, harmony);
    }
}

fn rule_combo(ui: &mut Ui, id: &str, cur: usize, w: f32) -> Option<usize> {
    let mut out = None;
    egui::ComboBox::from_id_salt(id).width(w).selected_text(RULES[cur.min(6)]).show_ui(ui, |ui| {
        for (k, t) in RULES.iter().enumerate() {
            if ui.add(egui::Button::selectable(k == cur, *t)).on_hover_text(RULE_TIPS[k]).clicked() {
                out = Some(k);
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(RULE_TIPS[cur.min(6)]).size(11.5));
        });
    })
    .response
    .on_hover_text("Choose or remove a harmony rule to create or edit a color harmony.");
    out
}

fn swatch(ui: &mut Ui, s: &NamedColor, sw: f32, selected: bool, used: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(sw, sw), Sense::click());
    let p = ui.painter();
    p.rect_filled(r, 0.0, theme::to_c32(s.color));
    if s.spot {
        p.circle_filled(pos2(r.right() - 4.0, r.bottom() - 4.0), 2.5, Color32::WHITE);
    }
    if used {
        p.circle_filled(pos2(r.left() + 4.0, r.top() + 4.0), 2.0, Color32::BLACK);
    }
    if selected {
        p.rect_stroke(r.expand(1.5), 0.0, Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Outside);
    } else if resp.hovered() {
        p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
    }
    resp.on_hover_text(format!("{}\n{}", s.name, s.color.describe()))
}

fn act(app: &mut AnyVectorApp, r: &str, doc: &Document, sel: &[StyleRef], harmony: Option<usize>) {
    let colors = |rs: &[StyleRef]| rs.iter().filter_map(|r| style(doc, *r).map(|s| s.color)).collect::<Vec<_>>();
    match r {
        "rgb" | "cmyk" | "gray" | "spot" => {
            let to_spot = r == "spot";
            let changes: Vec<(StyleRef, Color)> = sel
                .iter()
                .filter_map(|x| {
                    let c = style(doc, *x)?.color;
                    Some((*x, if r == "gray" { let l = c.luminance().round().clamp(0.0, 255.0) as u8; Color::rgb(l, l, l).with_alpha(c.a) } else { c }))
                })
                .collect();
            if let Some(d) = app.doc_mut() {
                let sel = sel.to_vec();
                d.modify("Convert Color Styles", |doc, _| {
                    for x in &sel {
                        if let Some(s) = style_mut(doc, *x) {
                            s.spot = to_spot;
                        }
                    }
                });
            }
            set_colors(app, "Convert Color Styles", changes);
        }
        "new" => add_styles(app, "New Color Style", vec![Color::rgb(0, 0, 0)], false),
        "from_sel" => {
            let c = selection_colors(app);
            add_styles(app, "New Color Styles", c, false);
        }
        "from_doc" => {
            let c = doc.used_colors();
            add_styles(app, "New Color Styles", c, false);
        }
        "harmony" => {
            // the selected styles move into the new harmony
            let picked: Vec<NamedColor> = sel.iter().filter_map(|r| style(doc, *r).cloned()).collect();
            let sel = sel.to_vec();
            if let Some(d) = app.doc_mut() {
                d.modify("New Color Harmony", |doc, _| {
                    remove(doc, &sel);
                    let n = doc.color_harmonies.len() + 1;
                    doc.color_harmonies.push(ColorHarmony { name: format!("Harmony {n}"), rule: 0, styles: picked.clone() });
                });
                let h = d.doc.color_harmonies.len() - 1;
                app.dockers.ui.cs.harmony = Some(h);
                app.dockers.ui.cs.sel = (0..picked.len()).map(|i| (Some(h), i)).collect();
            }
        }
        "dup_harmony" => {
            if let (Some(h), Some(d)) = (harmony, app.doc_mut()) {
                d.modify("Duplicate Harmony", |doc, _| {
                    let mut c = doc.color_harmonies[h].clone();
                    c.name = format!("{} (copy)", c.name);
                    doc.color_harmonies.push(c);
                });
            }
        }
        "gradient" => {
            let Some(base) = sel.first().and_then(|r| style(doc, *r)).map(|s| s.color) else { return };
            let (n, sim, kind) = app.dockers.ui.cs.grad;
            let hsb = base.to_hsb();
            let spread = (100.0 - sim as f64).max(5.0);
            let mut list = vec![];
            for i in 0..n {
                let t = if n > 1 { i as f64 / (n - 1) as f64 } else { 0.0 };
                let delta = match kind {
                    0 => t * spread,
                    1 => -t * spread,
                    _ => (t - 0.5) * spread,
                };
                let b = (hsb[2] + delta).clamp(0.0, 100.0);
                let s = (hsb[1] - delta * 0.5).clamp(0.0, 100.0);
                list.push(Color::from_hsb(hsb[0], s, b));
            }
            add_styles(app, "New Gradient", list, true);
            if let Some(d) = app.doc() {
                app.dockers.ui.cs.harmony = Some(d.doc.color_harmonies.len() - 1);
            }
        }
        "merge" => {
            let Some(last) = sel.last().copied() else { return };
            let Some(target) = style(doc, last).map(|s| s.color) else { return };
            let others: Vec<StyleRef> = sel[..sel.len() - 1].to_vec();
            let map: Vec<(Color, Color)> = colors(&others).into_iter().map(|c| (c, target)).collect();
            if let Some(d) = app.doc_mut() {
                d.modify("Merge Color Styles", |doc, _| {
                    recolor(doc, &map);
                    remove(doc, &others);
                });
            }
            app.dockers.ui.cs.sel.clear();
        }
        "swap" => {
            if let [a, b] = sel {
                let (ca, cb) = (style(doc, *a).map(|s| s.color), style(doc, *b).map(|s| s.color));
                if let (Some(ca), Some(cb)) = (ca, cb) {
                    // through a temporary color, so the two sets of objects trade colors
                    let tmp = Color::rgb(1, 2, 3).with_alpha(7);
                    let (a, b) = (*a, *b);
                    if let Some(d) = app.doc_mut() {
                        d.modify("Swap Color Styles", |doc, _| {
                            recolor(doc, &[(ca, tmp)]);
                            recolor(doc, &[(cb, ca)]);
                            recolor(doc, &[(tmp, cb)]);
                            if let Some(s) = style_mut(doc, a) {
                                s.color = cb;
                            }
                            if let Some(s) = style_mut(doc, b) {
                                s.color = ca;
                            }
                        });
                    }
                }
            }
        }
        "unused" => {
            let used = doc.used_colors();
            let mut v = vec![];
            for (i, s) in doc.color_styles.iter().enumerate() {
                if !used.contains(&s.color) {
                    v.push((None, i));
                }
            }
            for (h, hm) in doc.color_harmonies.iter().enumerate() {
                for (i, s) in hm.styles.iter().enumerate() {
                    if !used.contains(&s.color) {
                        v.push((Some(h), i));
                    }
                }
            }
            app.dockers.ui.cs.sel = v;
            app.dockers.ui.cs.harmony = None;
        }
        "delete" => {
            let sel = sel.to_vec();
            if let Some(d) = app.doc_mut() {
                d.modify("Delete Color Styles", |doc, _| {
                    remove(doc, &sel);
                    if sel.is_empty() {
                        if let Some(h) = harmony {
                            doc.color_harmonies.remove(h);
                        }
                    }
                });
            }
            app.dockers.ui.cs.sel.clear();
            app.dockers.ui.cs.harmony = None;
        }
        "import" => crate::dockers::ostyles::import_styles(app),
        "export" => crate::dockers::ostyles::export_styles(app),
        "defaults" => {
            app.settings.default_color_styles = doc.color_styles.clone();
            app.settings.default_color_harmonies = doc.color_harmonies.clone();
            app.flash("The color styles are the defaults of new documents.");
        }
        r if r.starts_with('r') => {
            // a harmony rule with styles selected: a new harmony of them
            let k: u8 = r[1..].parse().unwrap_or(0);
            act(app, "harmony", doc, sel, harmony);
            if let Some(h) = app.dockers.ui.cs.harmony {
                apply_rule(app, h, k);
            }
        }
        _ => {}
    }
}

/// Removes styles (from the end, so the indices hold).
fn remove(doc: &mut Document, rs: &[StyleRef]) {
    let mut rs = rs.to_vec();
    rs.sort_by(|a, b| b.cmp(a));
    for r in rs {
        match r.0 {
            None if r.1 < doc.color_styles.len() => {
                doc.color_styles.remove(r.1);
            }
            Some(h) if h < doc.color_harmonies.len() && r.1 < doc.color_harmonies[h].styles.len() => {
                doc.color_harmonies[h].styles.remove(r.1);
            }
            _ => {}
        }
    }
}
