//! Object Styles docker: Styles, Style Sets and Default Object Properties, the style editor, and
//! the Object ▸ Object Styles commands.

use super::{drop_mark, icon_button_resp, no_doc};
use crate::app::AnyVectorApp;
use crate::icons::Tone;
use crate::menus::{item, separator};
use crate::propbar::edit_sel;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use vector_core::document::ObjectStyle;
use vector_core::{Fill, Style, Transparency};

const SEL: Color32 = Color32::from_rgb(29, 68, 112);
const GREEN: Color32 = Color32::from_rgb(0x36, 0xb2, 0x4a);

/// The kinds of styles: a style set holds the outline, the fill and the transparency.
pub const KINDS: [&str; 4] = ["Style Set", "Outline", "Fill", "Transparency"];
/// The object types of Default Object Properties.
pub const DEFAULTS: [&str; 6] = ["Artistic Media", "Artistic Text", "Callout", "Dimension", "Graphic", "Paragraph Text"];

#[derive(Clone, Copy, PartialEq, Default)]
pub enum OsSel {
    #[default]
    None,
    Styles,
    Sets,
    Defaults,
    /// A style or style set (index in the document's styles).
    Style(usize),
    Default(usize),
}

/// What the Object Styles docker keeps between frames.
pub struct OsUi {
    pub sel: OsSel,
    /// Styles, Style Sets and Default Object Properties unfolded.
    pub open: [bool; 3],
    /// The height of the list (dragged at the grip under it).
    pub list_h: f32,
    pub rename: Option<(usize, String)>,
    /// The editor sections unfolded: Outline, Fill, Transparency.
    pub sections: [bool; 3],
}

impl Default for OsUi {
    fn default() -> Self {
        OsUi { sel: OsSel::None, open: [true, true, false], list_h: 72.0, rename: None, sections: [true; 3] }
    }
}

fn kind_icon(kind: u8) -> &'static str {
    match kind {
        1 => "outline_icon",
        2 => "fill_icon",
        3 => "transparency",
        _ => "os_style",
    }
}

/// A style or style set made from an object's properties.
pub fn style_from(name: String, kind: u8, s: &Style) -> ObjectStyle {
    ObjectStyle { name, style: s.clone(), kind }
}

/// Apply a style to an object's style: a style set applies all its properties, a style its own.
pub fn apply_style(to: &mut Style, s: &ObjectStyle) {
    match s.kind {
        1 => to.outline = s.style.outline.clone(),
        2 => to.fill = s.style.fill.clone(),
        3 => to.transparency = s.style.transparency.clone(),
        _ => {
            to.outline = s.style.outline.clone();
            to.fill = s.style.fill.clone();
            to.transparency = s.style.transparency.clone();
        }
    }
}

fn unique_name(doc: &vector_core::Document, base: &str) -> String {
    (1..).map(|i| format!("{base} {i}")).find(|n| !doc.object_styles.iter().any(|s| &s.name == n)).unwrap_or_default()
}

/// New Style (Outline / Fill / Transparency) or New Style Set: from the selection when there is
/// one, else from the default properties of graphics.
pub fn new_style(app: &mut AnyVectorApp, kind: u8) -> Option<usize> {
    let base = app.doc().and_then(|d| d.selection.first().and_then(|id| d.doc.find(*id)).map(|o| o.style.clone())).unwrap_or_else(|| app.ts.new_style());
    let d = app.doc_mut()?;
    let name = unique_name(&d.doc, KINDS[kind as usize]);
    let i = d.modify(if kind == 0 { "New Style Set" } else { "New Style" }, |doc, _| {
        doc.object_styles.push(style_from(name, kind, &base));
        doc.object_styles.len() - 1
    });
    app.dockers.ui.os.sel = OsSel::Style(i);
    app.dockers.ui.os.open[if kind == 0 { 1 } else { 0 }] = true;
    Some(i)
}

/// Apply a style to the selection; the objects keep a link to it.
pub fn apply_to_selection(app: &mut AnyVectorApp, i: usize) {
    let Some(s) = app.doc().and_then(|d| d.doc.object_styles.get(i).cloned()) else { return };
    edit_sel(app, "Apply Style", |o| {
        apply_style(&mut o.style, &s);
        o.object_style = Some(s.name.clone());
    });
}

/// The style the first selected object is linked to.
pub fn linked_style(app: &AnyVectorApp) -> Option<usize> {
    let d = app.doc()?;
    let o = d.selection.first().and_then(|id| d.doc.find(*id))?;
    let n = o.object_style.as_ref()?;
    d.doc.object_styles.iter().position(|s| &s.name == n)
}

/// Object ▸ Object Styles ▸ Apply to Style: the style takes the first selected object's
/// properties.
pub fn apply_to_style(app: &mut AnyVectorApp) {
    let Some(i) = linked_style(app) else { return };
    let Some(d) = app.doc_mut() else { return };
    let Some(src) = d.selection.first().and_then(|id| d.doc.find(*id)).map(|o| o.style.clone()) else { return };
    d.modify("Apply to Style", |doc, _| {
        let s = &mut doc.object_styles[i];
        let mut st = s.style.clone();
        apply_style(&mut st, &style_from(String::new(), s.kind, &src));
        s.style = st;
        // the other objects linked to the style follow
        let s = s.clone();
        for p in doc.pages.iter_mut() {
            for l in p.layers.iter_mut() {
                for o in l.objects.iter_mut() {
                    o.walk_mut(&mut |x| {
                        if x.object_style.as_deref() == Some(s.name.as_str()) {
                            apply_style(&mut x.style, &s);
                        }
                    });
                }
            }
        }
    });
}

/// Object ▸ Object Styles ▸ Revert to Style.
pub fn revert_to_style(app: &mut AnyVectorApp) {
    let Some(i) = linked_style(app) else { return };
    let Some(s) = app.doc().map(|d| d.doc.object_styles[i].clone()) else { return };
    edit_sel(app, "Revert to Style", |o| {
        if o.object_style.as_deref() == Some(s.name.as_str()) {
            apply_style(&mut o.style, &s);
        }
    });
}

/// Object ▸ Object Styles ▸ Break Link to Style.
pub fn break_link(app: &mut AnyVectorApp) {
    edit_sel(app, "Break Link to Style", |o| o.object_style = None);
}

pub fn object_styles(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let mut st = std::mem::take(&mut app.dockers.ui.os);
    body(app, ui, &mut st);
    app.dockers.ui.os = st;
}

fn body(app: &mut AnyVectorApp, ui: &mut Ui, st: &mut OsUi) {
    let Some(d) = app.doc() else { return };
    let styles: Vec<(usize, String, u8)> = d.doc.object_styles.iter().enumerate().map(|(i, s)| (i, s.name.clone(), s.kind)).collect();
    if let OsSel::Style(i) = st.sel {
        if i >= styles.len() {
            st.sel = OsSel::None;
        }
    }
    let has_sel = !d.selection.is_empty();
    // the list: Styles, Style Sets and Default Object Properties
    let list_h = st.list_h;
    let mut acts: Vec<(OsSel, u8)> = vec![];
    egui::ScrollArea::vertical().id_salt("os_list").max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (root, title, k) in [(OsSel::Styles, "Styles", 0usize), (OsSel::Sets, "Style Sets", 1), (OsSel::Defaults, "Default Object Properties", 2)] {
            let kids: Vec<(OsSel, String, &'static str)> = match k {
                0 => styles.iter().filter(|s| s.2 != 0).map(|s| (OsSel::Style(s.0), s.1.clone(), kind_icon(s.2))).collect(),
                1 => styles.iter().filter(|s| s.2 == 0).map(|s| (OsSel::Style(s.0), s.1.clone(), "os_style")).collect(),
                _ => DEFAULTS.iter().enumerate().map(|(i, n)| (OsSel::Default(i), n.to_string(), "os_style")).collect(),
            };
            let fold = (!kids.is_empty()).then_some(st.open[k]);
            let right = if k == 2 { "os_revert" } else { "" };
            if let Some(a) = row(app, ui, st, root, 0, fold, "os_styles", title, true, right) {
                acts.push((root, a));
            }
            if st.open[k] {
                for (sel, name, icon) in kids {
                    if let Some(a) = row(app, ui, st, sel, 1, None, icon, &name, false, "") {
                        acts.push((sel, a));
                    }
                }
            }
        }
    });
    // the grip that sizes the list
    let (gr, gresp) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::drag());
    let gresp = gresp.on_hover_cursor(egui::CursorIcon::ResizeVertical);
    for k in -1..=1 {
        ui.painter().circle_filled(pos2(gr.center().x + k as f32 * 4.0, gr.center().y - 2.0), 1.1, theme::TEXT);
    }
    if gresp.dragged() {
        st.list_h = (st.list_h + gresp.drag_delta().y).clamp(48.0, 600.0);
    }
    for (sel, a) in acts {
        match a {
            // a click on the row
            0 => {
                st.sel = sel;
                st.rename = None;
            }
            // the fold arrow
            1 => {
                let k = match sel {
                    OsSel::Styles => 0,
                    OsSel::Sets => 1,
                    _ => 2,
                };
                st.open[k] = !st.open[k];
            }
            // the button at the right: New Style Set; Revert All to New Document Default
            2 => match sel {
                OsSel::Sets => {
                    new_style(app, 0);
                }
                OsSel::Defaults => revert_defaults(app),
                _ => {}
            },
            _ => {}
        }
    }
    // import, export or save defaults; From Selection; Apply to Selection
    let selected_style = match st.sel {
        OsSel::Style(i) => Some(i),
        _ => None,
    };
    ui.horizontal(|ui| {
        let io = icon_button_resp(app, ui, "os_io", "Import, export, or save defaults", true);
        drop_mark(ui, io.rect);
        egui::Popup::menu(&io).show(|ui| {
            crate::menus::menu_style(ui);
            if item(app, ui, "&Import Style Sheet...", true, false) {
                import_styles(app);
            }
            if item(app, ui, "&Export Style Sheet...", true, false) {
                export_styles(app);
            }
            separator(ui);
            if item(app, ui, "&Set as New Document Defaults...", true, false) {
                if let Some(d) = app.doc() {
                    app.settings.default_object_styles = d.doc.object_styles.clone();
                    app.flash("The styles are the defaults of new documents.");
                }
            }
        });
        let w = (ui.available_width() - 8.0) / 2.0;
        if ui.add_enabled(has_sel, egui::Button::new("From Selection").min_size(vec2(w, 24.0))).clicked() {
            let kind = match st.sel {
                OsSel::Styles => 2,
                OsSel::Style(i) => styles.get(i).map(|s| s.2).unwrap_or(0),
                _ => 0,
            };
            new_style(app, kind);
            st.sel = app.dockers.ui.os.sel;
            st.open[if kind == 0 { 1 } else { 0 }] = true;
        }
        if ui.add_enabled(has_sel && selected_style.is_some(), egui::Button::new("Apply to Selection").min_size(vec2(w, 24.0))).clicked() {
            if let Some(i) = selected_style {
                apply_to_selection(app, i);
            }
        }
    });
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, theme::SEP));
    // the editor of the selected style or default properties
    match st.sel {
        OsSel::Style(i) => {
            let Some(s) = app.doc().and_then(|d| d.doc.object_styles.get(i).cloned()) else { return };
            let mut style = s.style.clone();
            let parts: Vec<usize> = match s.kind {
                1 => vec![0],
                2 => vec![1],
                3 => vec![2],
                _ => vec![0, 1, 2],
            };
            if editor(ui, st, &mut style, &parts, true) {
                if let Some(d) = app.doc_mut() {
                    d.modify("Edit Style", |doc, _| {
                        doc.object_styles[i].style = style;
                        let s = doc.object_styles[i].clone();
                        for p in doc.pages.iter_mut() {
                            for l in p.layers.iter_mut() {
                                for o in l.objects.iter_mut() {
                                    o.walk_mut(&mut |x| {
                                        if x.object_style.as_deref() == Some(s.name.as_str()) {
                                            apply_style(&mut x.style, &s);
                                        }
                                    });
                                }
                            }
                        }
                    });
                }
            }
        }
        OsSel::Default(k) => {
            // graphics take the defaults of new objects; text keeps its own
            let graphic = DEFAULTS[k] == "Graphic" || DEFAULTS[k] == "Artistic Media";
            let mut style = if graphic { app.ts.new_style() } else { Style::text_default() };
            if editor(ui, st, &mut style, &[0, 1, 2], graphic) && graphic {
                app.ts.new_fill = style.fill.main_color();
                app.ts.default_outline_width = style.outline.width;
            }
        }
        _ => {}
    }
}

/// A row of the list: (0 clicked, 1 fold arrow, 2 button at the right).
#[allow(clippy::too_many_arguments)]
fn row(app: &mut AnyVectorApp, ui: &mut Ui, st: &mut OsUi, sel: OsSel, depth: usize, fold: Option<bool>, icon: &str, name: &str, bold: bool, right: &str) -> Option<u8> {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
    let p = ui.painter().clone();
    if st.sel == sel {
        p.rect_filled(r, 0.0, SEL);
    } else if resp.hovered() {
        p.rect_filled(r, 0.0, theme::HOVER);
    }
    let cy = r.center().y;
    let x0 = r.left() + 8.0 + depth as f32 * 19.0;
    let mut out = None;
    if let Some(open) = fold {
        let c = pos2(x0, cy);
        let pts = if open { vec![c + vec2(-3.5, -1.5), c + vec2(0.0, 2.0), c + vec2(3.5, -1.5)] } else { vec![c + vec2(-1.5, -3.5), c + vec2(2.0, 0.0), c + vec2(-1.5, 3.5)] };
        p.add(egui::Shape::line(pts, Stroke::new(1.2, theme::TEXT)));
        if resp.clicked() && resp.interact_pointer_pos().is_some_and(|q| (q.x - x0).abs() < 7.0) {
            out = Some(1);
        }
    }
    if let Some(t) = app.icons.get(ui.ctx(), icon, 16.0, Tone::Normal) {
        p.image(t.id, Rect::from_center_size(pos2(x0 + 16.0, cy), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    let x = x0 + 26.0;
    if let (OsSel::Style(i), Some((ri, text))) = (sel, st.rename.as_mut()) {
        if *ri == i {
            let te = ui.put(Rect::from_min_max(pos2(x - 2.0, cy - 10.0), pos2(r.right() - 30.0, cy + 10.0)), egui::TextEdit::singleline(text));
            if te.lost_focus() {
                let n = text.trim().to_string();
                st.rename = None;
                rename_style(app, i, n);
            } else {
                te.request_focus();
            }
            return out;
        }
    }
    p.text(pos2(x, cy), egui::Align2::LEFT_CENTER, name, if bold { theme::bold(12.5) } else { theme::body(12.5) }, theme::TEXT);
    // the buttons at the right: New Style (a menu), New Style Set, Revert All
    let br = Rect::from_center_size(pos2(r.right() - 12.0, cy), vec2(18.0, 18.0));
    if matches!(sel, OsSel::Styles | OsSel::Sets) {
        p.rect_filled(br.shrink(2.0), 0.0, GREEN);
        p.hline((br.center().x - 4.5)..=(br.center().x + 4.5), br.center().y, Stroke::new(1.8, Color32::WHITE));
        p.vline(br.center().x, (br.center().y - 4.5)..=(br.center().y + 4.5), Stroke::new(1.8, Color32::WHITE));
        let tip = if sel == OsSel::Styles { "New Style" } else { "New Style Set" };
        let b = ui.interact(br, ui.scope_id().with(("os_new", depth, name)), Sense::click()).on_hover_text(tip);
        if sel == OsSel::Styles {
            egui::Popup::menu(&b).show(|ui| {
                crate::menus::menu_style(ui);
                for (k, text) in [(1u8, "&Outline"), (2, "&Fill"), (3, "&Transparency")] {
                    if item(app, ui, text, true, false) {
                        new_style(app, k);
                    }
                }
                for text in ["&Character", "&Paragraph", "F&rame"] {
                    item(app, ui, text, false, false);
                }
            });
        } else if b.clicked() {
            out = Some(2);
        }
    } else if !right.is_empty() {
        if let Some(t) = app.icons.get(ui.ctx(), right, 16.0, Tone::Normal) {
            p.image(t.id, Rect::from_center_size(br.center(), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        if ui.interact(br, ui.scope_id().with(("os_right", name)), Sense::click()).on_hover_text("Revert All to New Document Default").clicked() {
            out = Some(2);
        }
    }
    if out.is_none() && resp.clicked() {
        out = Some(0);
    }
    if let OsSel::Style(i) = sel {
        if resp.double_clicked() {
            st.rename = Some((i, name.to_string()));
        }
        resp.context_menu(|ui| {
            crate::menus::menu_style(ui);
            let has_sel = app.has_selection();
            if item(app, ui, "&Apply Style", has_sel, false) {
                apply_to_selection(app, i);
            }
            separator(ui);
            if item(app, ui, "&Rename", true, false) {
                st.rename = Some((i, name.to_string()));
            }
            if item(app, ui, "D&uplicate", true, false) {
                duplicate_style(app, i);
            }
            if item(app, ui, "&Delete", true, false) {
                delete_style(app, i);
                st.sel = OsSel::None;
            }
            separator(ui);
            if item(app, ui, "&Select Objects Using Style", true, false) {
                select_using(app, i);
            }
        });
    }
    out
}

/// The editor of a style: its outline, fill and transparency (`parts` of 0, 1, 2).
fn editor(ui: &mut Ui, st: &mut OsUi, s: &mut Style, parts: &[usize], enabled: bool) -> bool {
    let mut changed = false;
    ui.add_enabled_ui(enabled, |ui| {
        for &k in parts {
            super::fold_header(ui, ["Outline", "Fill", "Transparency"][k], &mut st.sections[k]);
            if !st.sections[k] {
                continue;
            }
            match k {
                0 => {
                    ui.horizontal(|ui| {
                        changed |= super::color_pick_w(ui, "os_ol_color", &mut s.outline.color, true, true, 120.0);
                        changed |= super::spin(ui, &mut s.outline.width, 0.0..=1000.0, 0.1, 3, " pt", 90.0);
                    });
                }
                1 => {
                    let mut kind = if s.fill.is_none() { 0 } else { 1 };
                    ui.horizontal(|ui| {
                        if super::choice(ui, "os_fill_kind", &mut kind, &["No Fill", "Uniform Fill"], 120.0) {
                            s.fill = if kind == 0 { Fill::None } else { Fill::Solid(s.fill.main_color().unwrap_or(vector_core::Color::BLACK)) };
                            changed = true;
                        }
                        if kind == 1 {
                            let mut c = s.fill.main_color();
                            if super::color_pick_w(ui, "os_fill_color", &mut c, false, true, 90.0) {
                                s.fill = c.map(Fill::Solid).unwrap_or(Fill::None);
                                changed = true;
                            }
                        }
                    });
                }
                _ => {
                    let mut amount = match s.transparency {
                        Transparency::Uniform(v) => Some(v),
                        Transparency::None => None,
                        _ => Some(50.0),
                    };
                    let mut kind = amount.is_some() as usize;
                    ui.horizontal(|ui| {
                        if super::choice(ui, "os_tr_kind", &mut kind, &["No Transparency", "Uniform Transparency"], 160.0) {
                            amount = (kind == 1).then_some(50.0);
                            changed = true;
                        }
                        if let Some(v) = amount.as_mut() {
                            changed |= super::spin(ui, v, 0.0..=100.0, 1.0, 0, " %", 70.0);
                        }
                    });
                    if changed {
                        s.transparency = amount.map(Transparency::Uniform).unwrap_or(Transparency::None);
                    }
                }
            }
        }
    });
    changed
}

fn rename_style(app: &mut AnyVectorApp, i: usize, n: String) {
    let Some(d) = app.doc_mut() else { return };
    if n.is_empty() || d.doc.object_styles.iter().any(|s| s.name == n) {
        return;
    }
    d.modify("Rename Style", |doc, _| {
        let old = std::mem::replace(&mut doc.object_styles[i].name, n.clone());
        for p in doc.pages.iter_mut() {
            for l in p.layers.iter_mut() {
                for o in l.objects.iter_mut() {
                    o.walk_mut(&mut |x| {
                        if x.object_style.as_deref() == Some(old.as_str()) {
                            x.object_style = Some(n.clone());
                        }
                    });
                }
            }
        }
    });
}

fn duplicate_style(app: &mut AnyVectorApp, i: usize) {
    let Some(d) = app.doc_mut() else { return };
    d.modify("Duplicate Style", |doc, _| {
        let mut s = doc.object_styles[i].clone();
        s.name = (1..).map(|k| format!("{} ({k})", s.name)).find(|n| !doc.object_styles.iter().any(|x| &x.name == n)).unwrap_or_default();
        doc.object_styles.insert(i + 1, s);
    });
}

fn delete_style(app: &mut AnyVectorApp, i: usize) {
    let Some(d) = app.doc_mut() else { return };
    d.modify("Delete Style", |doc, _| {
        let s = doc.object_styles.remove(i);
        for p in doc.pages.iter_mut() {
            for l in p.layers.iter_mut() {
                for o in l.objects.iter_mut() {
                    o.walk_mut(&mut |x| {
                        if x.object_style.as_deref() == Some(s.name.as_str()) {
                            x.object_style = None;
                        }
                    });
                }
            }
        }
    });
}

fn select_using(app: &mut AnyVectorApp, i: usize) {
    let Some(d) = app.doc_mut() else { return };
    let name = d.doc.object_styles[i].name.clone();
    let page = d.page;
    let ids: Vec<_> = d.doc.layers_on_page(page).into_iter().flat_map(|(_, _, l)| l.objects.iter()).filter(|o| o.object_style.as_deref() == Some(name.as_str())).map(|o| o.id).collect();
    d.selection = ids;
}

/// Revert All to New Document Default: the defaults of new graphics.
fn revert_defaults(app: &mut AnyVectorApp) {
    app.ts.new_fill = None;
    app.ts.default_outline_width = crate::tools::ToolState::default().default_outline_width;
}

/// The style sheet file: the document's object styles, color styles and color harmonies as JSON
/// (a plain list of object styles is read too).
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct StyleSheet {
    #[serde(default)]
    object_styles: Vec<ObjectStyle>,
    #[serde(default)]
    color_styles: Vec<vector_core::NamedColor>,
    #[serde(default)]
    color_harmonies: Vec<vector_core::document::ColorHarmony>,
}

pub(crate) fn import_styles(app: &mut AnyVectorApp) {
    let Some(p) = rfd::FileDialog::new().set_title("Import Style Sheet").add_filter("AnyVector Style Sheet (*.vstyles)", &["vstyles"]).pick_file() else { return };
    let text = std::fs::read_to_string(&p).unwrap_or_default();
    let sheet: Option<StyleSheet> = serde_json::from_str(&text).ok().or_else(|| serde_json::from_str::<Vec<ObjectStyle>>(&text).ok().map(|object_styles| StyleSheet { object_styles, ..Default::default() }));
    let Some(sheet) = sheet else {
        app.flash("The style sheet could not be read.");
        return;
    };
    let Some(d) = app.doc_mut() else { return };
    d.modify("Import Style Sheet", |doc, _| {
        for s in sheet.object_styles {
            doc.object_styles.retain(|x| x.name != s.name);
            doc.object_styles.push(s);
        }
        for s in sheet.color_styles {
            doc.color_styles.retain(|x| x.name != s.name);
            doc.color_styles.push(s);
        }
        for h in sheet.color_harmonies {
            doc.color_harmonies.retain(|x| x.name != h.name);
            doc.color_harmonies.push(h);
        }
    });
}

pub(crate) fn export_styles(app: &mut AnyVectorApp) {
    let Some(d) = app.doc() else { return };
    let sheet = StyleSheet { object_styles: d.doc.object_styles.clone(), color_styles: d.doc.color_styles.clone(), color_harmonies: d.doc.color_harmonies.clone() };
    let json = serde_json::to_string_pretty(&sheet).unwrap_or_default();
    let Some(p) = rfd::FileDialog::new().set_title("Export Style Sheet").add_filter("AnyVector Style Sheet (*.vstyles)", &["vstyles"]).set_file_name("Styles.vstyles").save_file() else { return };
    if std::fs::write(&p, json).is_err() {
        app.flash("The style sheet could not be saved.");
    }
}

/// The run-time rows of Object ▸ Object Styles ▸ Apply Style (the styles), Edit Style (the
/// styles the selection is linked to) and New Style From (the kinds).
pub fn menu_styles(app: &AnyVectorApp, which: u8) -> Vec<(String, usize, bool)> {
    let Some(d) = app.doc() else { return vec![] };
    let has = !d.selection.is_empty();
    match which {
        0 => d.doc.object_styles.iter().enumerate().map(|(i, s)| (s.name.replace('&', "&&"), i, has)).collect(),
        1 => linked_style(app).map(|i| vec![(d.doc.object_styles[i].name.replace('&', "&&"), i, true)]).unwrap_or_default(),
        _ => vec![("&Outline".into(), 1, has), ("&Fill".into(), 2, has), ("&Transparency".into(), 3, has)],
    }
}

/// Edit Style: the docker with the style selected.
pub fn edit_style(app: &mut AnyVectorApp, i: usize) {
    app.dockers.show(super::DockerId::ObjectStyles);
    app.dockers.ui.os.sel = OsSel::Style(i);
}
