//! Color palette column (right edge) and the document palette strip (bottom).

use crate::app::AnyVectorApp;
use crate::settings::Settings;
use crate::theme;
use crate::widgets::paint_well;
use egui::{pos2, vec2, Color32, PointerButton, Pos2, Rect, RectAlign, Sense, Stroke, Ui};
use vector_core::color::{ColorVariations, NamedColor, Palette, SPOT_TINTS};
use vector_core::Color;

/// Drag-and-drop payload for colors.
#[derive(Clone, Debug)]
pub struct ColorDrag(pub Option<Color>);

/// How long a palette swatch is held down before its pop-up color picker opens, in seconds.
pub const HOLD_DELAY: f64 = 0.5;

/// A strip of palette swatches: an open palette docked on the right (its index in
/// [`PaletteState::palettes`]) or the document palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strip {
    Column(usize),
    Document,
}

/// The pop-up color picker of a palette swatch, as in CorelDRAW: holding a swatch down shows the
/// variations of its color (tints, shades and nearby hues; the tints of a spot color), and a
/// click on one applies it as the fill, a right-click as the outline.
pub struct Variations {
    pub strip: Strip,
    /// The swatch it hangs from: its index in the strip.
    pub index: usize,
    pub base: Color,
    /// The palette name of the color (the spot ink's name for its tints).
    pub name: String,
    pub spot: bool,
    pub grid: ColorVariations,
    /// The button whose press opened the picker, while it is still down: releasing it over a
    /// variation applies that one (the fill for the primary button, the outline for the
    /// secondary), and its release on the swatch is not a click.
    pub held: Option<PointerButton>,
    /// Where the variations were drawn last, in the order of `grid.colors`.
    pub cells: Vec<Rect>,
    /// Not shown yet: the next show opens the popup.
    fresh: bool,
}

pub struct PaletteState {
    pub palettes: Vec<Palette>,
    /// The first shown swatch of each open palette.
    pub scroll: Vec<usize>,
    pub recent: Vec<String>,
    /// Color being sampled with the palette eyedropper.
    pub eyedropper: bool,
    /// The open pop-up color picker of a swatch.
    pub variations: Option<Variations>,
    /// The swatches drawn last: their strip, index, rectangle and color.
    pub drawn: Vec<(Strip, usize, Rect, Option<Color>)>,
}

impl PaletteState {
    pub fn new(s: &Settings) -> PaletteState {
        let edited = std::fs::read_to_string(crate::dialogs::palette::default_palette_file()).ok().map(|t| Palette::parse("Default Palette", &t)).filter(|p| !p.colors.is_empty());
        let mut palettes = vec![edited.unwrap_or_else(Palette::default_palette)];
        for p in &s.open_palettes {
            if let Ok(t) = std::fs::read_to_string(p) {
                let name = std::path::Path::new(p).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                palettes.push(Palette::parse(&name, &t));
            }
        }
        PaletteState { palettes, scroll: vec![], recent: vec![], eyedropper: false, variations: None, drawn: vec![] }
    }

    /// Open the pop-up color picker of the swatch `index` of `strip`, showing `nc`; `held`: the
    /// button of the press that opened it, still down.
    pub fn open_variations(&mut self, strip: Strip, index: usize, nc: &NamedColor, held: Option<PointerButton>) {
        let grid = if nc.spot { nc.color.spot_tints() } else { nc.color.variations() };
        self.variations = Some(Variations { strip, index, base: nc.color, name: nc.name.clone(), spot: nc.spot, grid, held, cells: vec![], fresh: true });
    }

    /// The press that opened the color picker of this swatch is still down (or released this
    /// frame): it is a hold, not a click on the swatch.
    fn opened_by_press(&self, strip: Strip, index: usize) -> bool {
        self.variations.as_ref().is_some_and(|v| v.strip == strip && v.index == index && v.held.is_some())
    }

    pub fn open_by_name(&mut self, name: &str) {
        let p = match name {
            "Default Palette" => Palette::default_palette(),
            "Grayscale" => Palette::grayscale(),
            "RGB" => Palette::rgb_palette(),
            "Web" => Palette::web_palette(),
            _ => return,
        };
        if !self.palettes.iter().any(|x| x.name == p.name) {
            self.palettes.push(p);
        }
    }
}

pub fn column_width(s: &Settings) -> f32 {
    let w = if s.large_swatches { 26.0 } else { 18.0 };
    w * s.palette_rows.max(1) as f32 + 4.0
}

fn swatch_h(s: &Settings) -> f32 {
    if s.large_swatches {
        22.0
    } else {
        16.0
    }
}

/// The `idx`-th open palette, docked on the right edge.
pub fn palette_column(app: &mut AnyVectorApp, ui: &mut Ui, idx: usize) {
    if app.palettes.scroll.len() <= idx {
        app.palettes.scroll.resize(idx + 1, 0);
    }
    let full = ui.available_rect_before_wrap();
    let cols = app.settings.palette_rows.max(1) as usize;
    let sw = (full.width() - 2.0) / cols as f32;
    let sh = swatch_h(&app.settings);
    let mut y = full.top();
    let x0 = full.left() + 1.0;
    let p = ui.painter().clone();
    // dock arrow ›
    let r = Rect::from_min_size(pos2(x0, y), vec2(full.width() - 2.0, 14.0));
    let resp = ui.interact(r, ui.scope_id().with("pal_menu"), Sense::click()).on_hover_text("Palette options");
    crate::widgets::chevron_right(&p, r.center(), 3.0, if resp.hovered() { theme::STRONG() } else { theme::TEXT_DIM() });
    egui::Popup::menu(&resp).show(|ui| palette_options_menu(app, ui));
    y += 16.0;
    // eyedropper
    let r = Rect::from_min_size(pos2(x0, y), vec2(full.width() - 2.0, 18.0));
    let resp = ui.interact(r, ui.scope_id().with("pal_eye"), Sense::click()).on_hover_text("Eyedropper: sample a color and apply it to the selection");
    if resp.hovered() || app.palettes.eyedropper {
        p.rect_filled(r, 2.0, if app.palettes.eyedropper { theme::PRESSED() } else { theme::HOVER() });
    }
    if let Some(t) = app.icons.get(ui.ctx(), "eyedropper", 14.0, crate::icons::Tone::Normal) {
        p.image(t.id, Rect::from_center_size(r.center(), vec2(14.0, 14.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    if resp.clicked() && app.has_doc() {
        app.palettes.eyedropper = !app.palettes.eyedropper;
    }
    y += 19.0;
    // scroll up
    let r = Rect::from_min_size(pos2(x0, y), vec2(full.width() - 2.0, 12.0));
    let up = ui.interact(r, ui.scope_id().with("pal_up"), Sense::click());
    crate::widgets::chevron_up(&p, r.center(), 3.0, if app.palettes.scroll[idx] > 0 { theme::TEXT() } else { theme::DISABLED() });
    if up.clicked() && app.palettes.scroll[idx] > 0 {
        app.palettes.scroll[idx] -= cols;
    }
    y += 13.0;
    let bottom_reserved = 30.0;
    let avail = full.bottom() - bottom_reserved - y;
    let rows_fit = ((avail / sh).floor() as usize).max(1);
    let mut colors: Vec<Option<NamedColor>> = vec![];
    if app.settings.show_no_color_well {
        colors.push(None);
    }
    colors.extend(app.palettes.palettes.get(idx).map(|p| p.colors.clone()).unwrap_or_default().into_iter().map(Some));
    let total_rows = colors.len().div_ceil(cols);
    let max_scroll = total_rows.saturating_sub(rows_fit) * cols;
    app.palettes.scroll[idx] = app.palettes.scroll[idx].min(max_scroll);
    // wheel scroll over the palette
    if ui.rect_contains_pointer(Rect::from_min_max(pos2(full.left(), y), pos2(full.right(), full.bottom() - bottom_reserved))) {
        let d = ui.input(|i| i.smooth_scroll_delta.y);
        if d < -1.0 {
            app.palettes.scroll[idx] = (app.palettes.scroll[idx] + cols).min(max_scroll);
        } else if d > 1.0 {
            app.palettes.scroll[idx] = app.palettes.scroll[idx].saturating_sub(cols);
        }
    }
    let start = app.palettes.scroll[idx];
    let strip = Strip::Column(idx);
    app.palettes.drawn.retain(|d| d.0 != strip);
    let mut clicked: Option<(Option<Color>, bool)> = None;
    let mut variations: Option<(usize, Option<PointerButton>)> = None;
    for (k, c) in colors.iter().enumerate().skip(start).take(rows_fit * cols) {
        let i = k - start;
        let row = i / cols;
        let col = i % cols;
        let r = Rect::from_min_size(pos2(x0 + col as f32 * sw, y + row as f32 * sh), vec2(sw - if app.settings.wide_borders { 2.0 } else { 0.0 }, sh - 1.0));
        let id = ui.scope_id().with(("sw", k));
        let resp = ui.interact(r, id, Sense::click_and_drag());
        let color = c.as_ref().map(|nc| nc.color);
        paint_well(&p, r, color);
        if resp.hovered() {
            p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
        }
        app.palettes.drawn.push((strip, k, r, color));
        let name = c.as_ref().map(|nc| nc.name.clone()).unwrap_or_else(|| "No Color".into());
        let tip = match color {
            Some(cc) => format!("{name}\n{}", cc.describe()),
            None => name,
        };
        let resp = resp.on_hover_text(tip);
        resp.dnd_set_drag_payload(ColorDrag(color));
        // the release of a hold that opened the color picker applies nothing
        if !app.palettes.opened_by_press(strip, k) {
            if resp.clicked() {
                clicked = Some((color, false));
            }
            if resp.secondary_clicked() {
                if app.settings.right_click_outline {
                    clicked = Some((color, true));
                }
            }
            if !app.settings.right_click_outline {
                resp.context_menu(|ui| {
                    if ui.button("Set Fill").clicked() {
                        clicked = Some((color, false));
                    }
                    if ui.button("Set Outline").clicked() {
                        clicked = Some((color, true));
                    }
                    if color.is_some() && ui.button("Color Variations").clicked() {
                        variations = Some((k, None));
                    }
                });
            }
        }
        // holding a color down opens its pop-up color picker ("No Color" has no variations)
        if color.is_some() && !app.palettes.eyedropper {
            if let Some(b) = crate::widgets::press_and_hold(ui, &resp, HOLD_DELAY) {
                variations = Some((k, Some(b)));
            }
        }
    }
    if let Some((k, held)) = variations {
        if let Some(Some(nc)) = colors.get(k) {
            app.palettes.open_variations(strip, k, nc, held);
        }
    }
    if let Some((c, outline)) = clicked {
        crate::exec::apply_color(app, c, outline);
    }
    // to the left of the column, flipped to stay on screen
    show_variations(app, ui, strip, RectAlign::LEFT_START, &[RectAlign::LEFT_END, RectAlign::RIGHT_START, RectAlign::RIGHT_END]);
    // scroll down and expand
    let yb = full.bottom() - bottom_reserved + 2.0;
    let r = Rect::from_min_size(pos2(x0, yb), vec2(full.width() - 2.0, 12.0));
    let dn = ui.interact(r, ui.scope_id().with("pal_dn"), Sense::click());
    crate::widgets::chevron(&p, r.center(), 3.0, if app.palettes.scroll[idx] < max_scroll { theme::TEXT() } else { theme::DISABLED() });
    if dn.clicked() {
        app.palettes.scroll[idx] = (app.palettes.scroll[idx] + cols).min(max_scroll);
    }
    let r = Rect::from_min_size(pos2(x0, yb + 13.0), vec2(full.width() - 2.0, 12.0));
    let ex = ui.interact(r, ui.scope_id().with("pal_ex"), Sense::click()).on_hover_text("Expand palette");
    p.text(r.center(), egui::Align2::CENTER_CENTER, "»", egui::FontId::proportional(11.0), if ex.hovered() { theme::STRONG() } else { theme::TEXT_DIM() });
    if ex.clicked() {
        app.settings.palette_rows = if app.settings.palette_rows >= 3 { 1 } else { app.settings.palette_rows + 1 };
    }
}

fn palette_options_menu(app: &mut AnyVectorApp, ui: &mut Ui) {
    ui.set_min_width(200.0);
    for name in ["Default Palette", "Grayscale", "RGB", "Web"] {
        let active = app.palettes.palettes.first().map(|p| p.name == name).unwrap_or(false);
        if ui.selectable_label(active, name).clicked() {
            let p = match name {
                "Grayscale" => Palette::grayscale(),
                "RGB" => Palette::rgb_palette(),
                "Web" => Palette::web_palette(),
                _ => Palette::default_palette(),
            };
            app.palettes.palettes.retain(|x| x.name != p.name);
            app.palettes.palettes.insert(0, p);
            app.palettes.scroll.iter_mut().for_each(|s| *s = 0);
            if !app.palettes.recent.contains(&name.to_string()) {
                app.palettes.recent.push(name.to_string());
            }
            ui.close();
        }
    }
    ui.separator();
    if ui.button("Open Palette...").clicked() {
        app.run(crate::commands::Cmd::OpenPalette);
        ui.close();
    }
    if ui.button("Palette Editor...").clicked() {
        app.run(crate::commands::Cmd::PaletteEditor);
        ui.close();
    }
    ui.separator();
    ui.checkbox(&mut app.settings.large_swatches, "Large swatches");
    ui.checkbox(&mut app.settings.wide_borders, "Wide borders");
    ui.checkbox(&mut app.settings.show_no_color_well, "Show \"No Color\" well");
}

pub fn document_palette(app: &mut AnyVectorApp, ui: &mut Ui) {
    let full = ui.available_rect_before_wrap();
    let p = ui.painter().clone();
    // drop zone for colors (and objects: their colors are added)
    let resp = ui.interact(full, ui.scope_id().with("docpal"), Sense::hover());
    let drop = resp.dnd_release_payload::<ColorDrag>();
    if let Some(c) = drop {
        if let (Some(color), Some(d)) = (c.0, app.doc_mut()) {
            if !d.doc.palette.iter().any(|n| n.color == color) {
                d.checkpoint("Add Color to Document Palette");
                d.doc.palette.push(NamedColor::new(color.to_hex(), color));
            }
        }
    }
    let colors: Vec<NamedColor> = app.doc().map(|d| d.doc.palette.clone()).unwrap_or_default();
    app.palettes.drawn.retain(|d| d.0 != Strip::Document);
    let right_w = 40.0;
    if colors.is_empty() {
        let hovering_drop = resp.dnd_hover_payload::<ColorDrag>().is_some();
        // cut in a narrow window, with the whole text when pointed at
        let room = Rect::from_min_max(full.min + vec2(4.0, 0.0), pos2((full.right() - right_w - 2.0).max(full.left() + 4.0), full.bottom()));
        let font = egui::FontId::new(12.0, egui::FontFamily::Proportional);
        let hint = crate::i18n::t("Drag colors (or objects) here to store these colors with your document");
        let shown = crate::statusbar::fit(&p, &hint, &font, room.width());
        if shown != hint {
            ui.interact(room, ui.scope_id().with("docpal_hint"), Sense::hover()).on_hover_text(hint.to_string());
        }
        // centered in the strip without the buttons, as far as there is room
        p.text(room.center(), egui::Align2::CENTER_CENTER, shown, font, if hovering_drop { theme::STRONG() } else { theme::TEXT_DIM() });
        // italic look: draw via galley with italics is unavailable for system fonts; dim text matches.
    } else {
        let sw = 18.0;
        let mut clicked = None;
        let mut variations: Option<(usize, PointerButton)> = None;
        for (i, nc) in colors.iter().enumerate() {
            let r = Rect::from_min_size(pos2(full.left() + 4.0 + i as f32 * (sw + 1.0), full.top() + 4.0), vec2(sw, full.height() - 8.0));
            if r.right() > full.right() - right_w {
                break;
            }
            let rr = ui.interact(r, ui.scope_id().with(("dps", i)), Sense::click_and_drag());
            paint_well(&p, r, Some(nc.color));
            if rr.hovered() {
                p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
            }
            app.palettes.drawn.push((Strip::Document, i, r, Some(nc.color)));
            rr.dnd_set_drag_payload(ColorDrag(Some(nc.color)));
            let rr = rr.on_hover_text(format!("{}\n{}", nc.name, nc.color.describe()));
            // the release of a hold that opened the color picker applies nothing
            if !app.palettes.opened_by_press(Strip::Document, i) {
                if rr.clicked() {
                    clicked = Some((Some(nc.color), false));
                }
                if rr.secondary_clicked() {
                    clicked = Some((Some(nc.color), true));
                }
            }
            if !app.palettes.eyedropper {
                if let Some(b) = crate::widgets::press_and_hold(ui, &rr, HOLD_DELAY) {
                    variations = Some((i, b));
                }
            }
        }
        if let Some((i, held)) = variations {
            app.palettes.open_variations(Strip::Document, i, &colors[i], Some(held));
        }
        if let Some((c, o)) = clicked {
            crate::exec::apply_color(app, c, o);
        }
    }
    // above the strip, flipped to stay on screen
    show_variations(app, ui, Strip::Document, RectAlign::TOP_START, &[RectAlign::TOP_END, RectAlign::BOTTOM_START, RectAlign::BOTTOM_END]);
    // › and » buttons at the right
    let r1 = Rect::from_center_size(pos2(full.right() - 30.0, full.center().y), vec2(12.0, 14.0));
    let r2 = Rect::from_center_size(pos2(full.right() - 13.0, full.center().y), vec2(12.0, 14.0));
    let b1 = ui.interact(r1, ui.scope_id().with("dp_menu"), Sense::click()).on_hover_text("Document palette options");
    let b2 = ui.interact(r2, ui.scope_id().with("dp_more"), Sense::click()).on_hover_text("Show more");
    crate::widgets::chevron_right(&p, r1.center(), 3.0, if b1.hovered() { theme::STRONG() } else { theme::TEXT_DIM() });
    p.text(r2.center(), egui::Align2::CENTER_CENTER, "»", egui::FontId::proportional(11.0), if b2.hovered() { theme::STRONG() } else { theme::TEXT_DIM() });
    egui::Popup::menu(&b1).show(|ui| {
        ui.set_min_width(220.0);
        if ui.button("Add Colors from Selection").clicked() {
            app.run(crate::commands::Cmd::AddColorsFromSelection);
        }
        if ui.button("Add Colors from Document").clicked() {
            app.run(crate::commands::Cmd::AddColorsFromDocument);
        }
        ui.separator();
        ui.checkbox(&mut app.settings.auto_update_doc_palette, "Automatically update");
        if ui.button("Clear Document Palette").clicked() {
            if let Some(d) = app.doc_mut() {
                d.checkpoint("Clear Document Palette");
                d.doc.palette.clear();
            }
        }
    });
    if b2.clicked() {
        app.run(crate::commands::Cmd::Docker(crate::dockers::DockerId::Palettes));
    }
}

/// The id of the pop-up color picker (one at a time, as for menus).
pub fn variations_popup_id() -> egui::Id {
    egui::Id::unique("palette_variations")
}

/// The pop-up color picker of a swatch of `strip`, if it has one open: placed by `align` beside
/// the swatch (or one of `alternatives` where that does not fit on screen). It is an egui popup,
/// so the keyboard shortcuts wait while it is open, Esc or a click elsewhere closes it, and it
/// closes when its swatch is no longer shown (the palette hidden, scrolled or changed).
fn show_variations(app: &mut AnyVectorApp, ui: &Ui, strip: Strip, align: RectAlign, alternatives: &'static [RectAlign]) {
    let id = variations_popup_id();
    let ctx = ui.ctx().clone();
    let pal = &mut app.palettes;
    let Some(v) = pal.variations.as_mut().filter(|v| v.strip == strip) else { return };
    // the swatch it hangs from, still showing the same color
    let Some(anchor) = pal.drawn.iter().find(|d| d.0 == strip && d.1 == v.index && d.3 == Some(v.base)).map(|d| d.2) else {
        egui::Popup::close_id(&ctx, id);
        pal.variations = None;
        return;
    };
    let open = std::mem::take(&mut v.fresh).then_some(egui::SetOpenCommand::Bool(true));
    let (pointer, released) = ctx.input(|i| (i.pointer.latest_pos(), v.held.filter(|b| i.pointer.button_released(*b))));
    let mut cells = vec![];
    let mut pick: Option<(Color, bool)> = None;
    let shown = egui::Popup::new(id, ctx.clone(), anchor, ui.layer_id())
        .kind(egui::PopupKind::Popup)
        .align(align)
        .align_alternatives(alternatives)
        .gap(2.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .open_memory(open)
        .show(|ui| variations_grid(ui, v, pointer, released, &mut cells, &mut pick));
    if shown.is_none() || !egui::Popup::is_id_open(&ctx, id) {
        pal.variations = None;
        return;
    }
    v.cells = cells;
    if v.held.is_some_and(|b| released.is_some() || !ctx.input(|i| i.pointer.button_down(b))) {
        v.held = None;
    }
    if let Some((c, outline)) = pick {
        egui::Popup::close_id(&ctx, id);
        pal.variations = None;
        // a release over a variation is a pick, not a drop of the swatch it was dragged from
        egui::DragAndDrop::clear_payload(&ctx);
        crate::exec::apply_color(app, Some(c), outline);
    }
}

/// The variations of the color picker: a click applies one as the fill, a right-click as the
/// outline; releasing the button of the hold that opened the picker over one applies it too.
fn variations_grid(ui: &mut Ui, v: &Variations, pointer: Option<Pos2>, released: Option<PointerButton>, cells: &mut Vec<Rect>, pick: &mut Option<(Color, bool)>) {
    const CELL: f32 = 16.0;
    const GAP: f32 = 2.0;
    let cols = v.grid.cols.max(1);
    let size = vec2(cols as f32 * (CELL + GAP) - GAP, v.grid.rows() as f32 * (CELL + GAP) - GAP);
    let (area, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter().clone();
    for (i, c) in v.grid.colors.iter().enumerate() {
        let r = Rect::from_min_size(area.min + vec2((i % cols) as f32 * (CELL + GAP), (i / cols) as f32 * (CELL + GAP)), vec2(CELL, CELL));
        cells.push(r);
        let resp = ui.interact(r, ui.scope_id().with(("variation", i)), Sense::click());
        paint_well(&p, r, Some(*c));
        // a thin edge keeps white wells visible on a light popup
        p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_black_alpha(50)), egui::StrokeKind::Inside);
        // the color itself: a black and white ring, visible on any color
        if i == v.grid.base {
            p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Inside);
            p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Outside);
        }
        // the pointer may be over it while the swatch is still held (egui does not hover then)
        let over = pointer.is_some_and(|q| r.contains(q));
        if over {
            p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
        }
        let tip = match SPOT_TINTS.get(i).filter(|_| v.spot) {
            Some(t) => format!("{} {t}%\n{}", v.name, c.describe()),
            None if c.is_cmyk() => c.describe(),
            None => format!("{}\n{}", c.to_hex(), c.describe()),
        };
        let resp = resp.on_hover_text(tip);
        if resp.clicked() {
            *pick = Some((*c, false));
        } else if resp.secondary_clicked() {
            *pick = Some((*c, true));
        } else if let (true, Some(b)) = (over, released) {
            *pick = Some((*c, b == PointerButton::Secondary));
        }
    }
}
