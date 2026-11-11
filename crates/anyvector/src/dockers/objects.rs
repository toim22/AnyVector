//! Objects docker (the tree of pages, layers and objects with the Master Page) and Pages docker,
//! as the reference shows them.

use super::{drop_mark, icon_button, icon_button_resp, no_doc};
use crate::app::AnyVectorApp;
use crate::commands::Cmd;
use crate::icons::Tone;
use crate::menus::{item, separator, submenu};
use crate::theme;
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, Ui};
use std::collections::HashSet;
use vector_core::{Layer, ObjId, ObjKind, Object};

/// The bands of the pages and the Master Page, the row lines and the selected row.
fn band_gray() -> Color32 {
    theme::gray(60)
}
fn line_gray() -> Color32 {
    theme::gray(82)
}
/// Selected row (the active docker tab blue).
fn sel_fill() -> Color32 {
    crate::theme::TAB_ACTIVE()
}
/// The fold key of the Master Page.
const MASTER: u64 = u64::MAX;
/// Special rows: the guides of a page, of all pages, the desktop and the document grid.
const GUIDES: usize = usize::MAX;
const MASTER_GUIDES: usize = usize::MAX - 1;
const DESKTOP: usize = usize::MAX - 2;
const DOC_GRID: usize = usize::MAX - 3;

/// What the Objects docker keeps between frames.
#[derive(Default)]
pub struct ObjUi {
    /// Folded pages (page id), layers (layer id) and the Master Page.
    pub folded: HashSet<u64>,
    /// Unfolded groups (object id).
    pub open_groups: HashSet<u64>,
    /// The selected layer row (page, layer), or (page, special row).
    pub row_sel: Option<(usize, usize)>,
    /// Resize list: 0 the smallest rows.
    pub size: f32,
    /// Move to Layer (false) or Copy to Layer (true), waiting for a click on the destination.
    pub pending: Option<bool>,
    pub last_sel: Vec<ObjId>,
    pub rename_layer: Option<((usize, usize), String)>,
}

/// What the Pages docker keeps between frames.
#[derive(Default)]
pub struct PagesUi {
    /// The list view (true) or the thumbnails.
    pub list: bool,
    /// Resize: 0 the smallest thumbnails.
    pub size: f32,
    pub sel: Option<usize>,
    pub rename: Option<(usize, String)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    PageBand(usize),
    MasterBand,
    /// A special row: (page, GUIDES / MASTER_GUIDES / DESKTOP / DOC_GRID).
    Special(usize, usize),
    Layer(usize, usize),
    Obj(ObjId),
}

struct Row {
    kind: Kind,
    /// Indent of object rows (group levels).
    depth: usize,
    name: String,
    fold: Option<bool>,
    line_below: bool,
    bar: Option<Color32>,
    icon: &'static str,
    /// Objects: their fill and outline.
    fill: Option<vector_core::Color>,
    outline: Option<vector_core::Color>,
    /// Shown (eye), locked, prints; None where the row has no such switch.
    flags: [Option<bool>; 3],
    selected: bool,
    bold: bool,
    scroll: bool,
}

impl Row {
    fn new(kind: Kind, name: String) -> Row {
        Row { kind, depth: 0, name, fold: None, line_below: false, bar: None, icon: "", fill: None, outline: None, flags: [None; 3], selected: false, bold: false, scroll: false }
    }
}

enum Act {
    Fold(u64),
    FoldGroup(u64),
    GoPage(usize),
    ClickLayer(usize, usize),
    ClickSpecial(usize, usize),
    ClickObj(ObjId, bool),
    RenameObj(ObjId, String),
    RenameLayer(usize, usize, String),
    Toggle(Kind, usize),
}

pub fn objects(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let mut st = std::mem::take(&mut app.dockers.ui.om);
    objects_body(app, ui, &mut st);
    app.dockers.ui.om = st;
}

fn objects_body(app: &mut AnyVectorApp, ui: &mut Ui, st: &mut ObjUi) {
    follow_selection(app, st);
    ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.objects_filter).hint_text("Search").desired_width(f32::INFINITY));
    // the views, where the selection is (or the page selector) and the options
    let layers_view = app.settings.om_layers_view;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if crate::widgets::icon_button(&mut app.icons, ui, "om_view_all", "View Pages, Layers, and Objects", true, !layers_view).clicked() {
            app.settings.om_layers_view = false;
        }
        if crate::widgets::icon_button(&mut app.icons, ui, "layer", "View Layers and Objects", true, layers_view).clicked() {
            app.settings.om_layers_view = true;
        }
        let Some(d) = app.doc() else { return };
        let (pi, li) = d.active_loc();
        let layer = d.doc.pages[pi].layers.get(li).map(|l| l.name.clone()).unwrap_or_default();
        if layers_view {
            ui.add_space(4.0);
            let names: Vec<String> = (0..d.doc.pages.len()).map(|i| d.doc.page_name(i)).collect();
            let mut cur = d.page;
            let before = cur;
            egui::ComboBox::from_id_salt("om_page_sel").width(125.0).selected_text(names[cur].clone()).show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut cur, i, n);
                }
            });
            if cur != before {
                go_page(app, cur);
            }
        } else {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(d.doc.page_name(d.page)).font(theme::bold(12.5)));
            ui.add_space(8.0);
            ui.label(layer);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let gear = icon_button_resp(app, ui, "gear", "Options", true);
            drop_mark(ui, gear.rect);
            egui::Popup::menu(&gear).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                crate::menus::menu_style(ui);
                options_menu(app, ui, st);
            });
        });
    });
    if let Some(copy) = st.pending {
        super::hint(ui, if copy { "Click the layer to copy the selected objects to." } else { "Click the layer to move the selected objects to." });
    }
    // the tree, then the buttons at the bottom
    let rows = build_rows(app, st);
    let rh = 26.0 + st.size * 30.0;
    let tree_h = (ui.clip_rect().bottom() - ui.cursor().top() - 36.0).max(60.0);
    let mut acts = vec![];
    egui::ScrollArea::vertical().id_salt("om_tree").max_height(tree_h).min_scrolled_height(tree_h).auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, line_gray()));
        for row in &rows {
            draw_row(app, ui, st, row, rh, &mut acts);
        }
    });
    apply(app, st, acts);
    bottom_bar(app, ui, st);
}

/// Options: the reference menu.
fn options_menu(app: &mut AnyVectorApp, ui: &mut Ui, st: &mut ObjUi) {
    let s = &app.settings;
    let (ea, ex, kd, sa, sp) = (s.edit_across_layers, s.om_expand_to_selection, s.om_keep_desktop, s.om_select_activates_layer, s.om_show_properties);
    if item(app, ui, "&Edit Across Layers", true, ea) {
        app.settings.edit_across_layers = !ea;
    }
    if item(app, ui, "E&xpand to Show Selection", true, ex) {
        app.settings.om_expand_to_selection = !ex;
    }
    if item(app, ui, "&Keep Desktop Objects on Layer", true, kd) {
        app.settings.om_keep_desktop = !kd;
    }
    if item(app, ui, "&Select Object to Activate Layer", true, sa) {
        app.settings.om_select_activates_layer = !sa;
    }
    separator(ui);
    if item(app, ui, "Show Object Properties", true, sp) {
        app.settings.om_show_properties = !sp;
    }
    separator(ui);
    let Some(d) = app.doc() else { return };
    let odd = d.page % 2 == 0;
    let (pi, li) = st.row_sel.filter(|(_, l)| *l < DOC_GRID).unwrap_or(d.active_loc());
    let can_delete = layer_deletable(&d.doc, pi, li) && d.selection.is_empty();
    let empty = d.doc.pages.iter().any(|p| p.layers.iter().filter(|l| l.master == 0).count() > 1 && p.layers.iter().any(|l| l.objects.is_empty()));
    let has_sel = !d.selection.is_empty();
    submenu(app, ui, "&Layer", true, |app, ui| {
        if item(app, ui, "&New Layer", true, false) {
            new_layer(app);
        }
        for (k, text, on) in [(1u8, "New Master Layer (&all pages)", true), (2, "New Master Layer (&odd pages)", odd), (3, "New Master Layer (&even pages)", !odd)] {
            if item(app, ui, text, on, false) {
                new_master_layer(app, k);
            }
        }
        if item(app, ui, "&Delete Layer", can_delete, false) {
            delete_layer(app, pi, li);
        }
        if item(app, ui, "Delete E&mpty Layers", empty, false) {
            delete_empty_layers(app);
        }
    });
    if item(app, ui, "&Move to Layer...", has_sel, false) {
        st.pending = Some(false);
    }
    if item(app, ui, "&Copy to Layer...", has_sel, false) {
        st.pending = Some(true);
    }
    separator(ui);
    submenu(app, ui, "&Page", true, |app, ui| page_items(app, ui, None));
    let sm = app.settings.om_show_master_on_pages;
    if item(app, ui, "Show Master Layers on Pages", true, sm) {
        app.settings.om_show_master_on_pages = !sm;
    }
    separator(ui);
    let lv = app.settings.om_layers_view;
    if item(app, ui, "&View Pages, Layers, and Objects", true, !lv) {
        app.settings.om_layers_view = false;
    }
    if item(app, ui, "&View Layers and Objects", true, lv) {
        app.settings.om_layers_view = true;
    }
}

/// New Page, Insert Page…, Duplicate Page…, Delete Page (of the Objects ▸ Page and the Pages
/// docker options); `sel` is the page chosen in the Pages docker.
fn page_items(app: &mut AnyVectorApp, ui: &mut Ui, sel: Option<Option<usize>>) {
    let n = app.doc().map(|d| d.doc.pages.len()).unwrap_or(1);
    if item(app, ui, "&New Page", true, false) {
        app.run(Cmd::InsertPageAfter);
    }
    if item(app, ui, "&Insert Page...", true, false) {
        app.run(Cmd::InsertPage);
    }
    if item(app, ui, "&Duplicate Page...", true, false) {
        app.run(Cmd::DuplicatePage);
    }
    let can = n > 1 && sel.map(|s| s.is_some()).unwrap_or(true);
    if item(app, ui, "D&elete Page", can, false) {
        if let (Some(Some(p)), Some(d)) = (sel, app.doc_mut()) {
            d.page = p;
        }
        app.run(Cmd::DeletePageNow);
    }
}

fn layer_deletable(doc: &vector_core::Document, pi: usize, li: usize) -> bool {
    let Some(l) = doc.pages.get(pi).and_then(|p| p.layers.get(li)) else { return false };
    l.master != 0 || doc.pages[pi].layers.iter().filter(|l| l.master == 0).count() > 1
}

fn new_layer(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    let pi = d.page;
    let li = d.modify("New Layer", |doc, _| {
        let id = doc.new_id();
        let n = doc.pages[pi].layers.iter().filter(|l| l.master == 0).count() + 1;
        let p = &mut doc.pages[pi];
        // above the local layers, below the master layers of the first page
        let at = p.layers.iter().position(|l| l.master != 0).unwrap_or(p.layers.len());
        p.layers.insert(at, Layer::new(id, &format!("Layer {n}")));
        at
    });
    d.layer = li;
    d.on_master = false;
}

fn new_master_layer(app: &mut AnyVectorApp, kind: u8) {
    let Some(d) = app.doc_mut() else { return };
    let li = d.modify("New Master Layer", |doc, _| doc.new_master_layer(kind));
    d.layer = li;
    d.on_master = true;
}

fn delete_layer(app: &mut AnyVectorApp, pi: usize, li: usize) {
    let Some(d) = app.doc_mut() else { return };
    if !layer_deletable(&d.doc, pi, li) {
        return;
    }
    d.modify("Delete Layer", |doc, sel| {
        doc.pages[pi].layers.remove(li);
        sel.clear();
    });
    d.on_master = false;
    d.layer = d.doc.pages[d.page].layers.iter().rposition(|l| l.master == 0).unwrap_or(0);
}

fn delete_empty_layers(app: &mut AnyVectorApp) {
    let Some(d) = app.doc_mut() else { return };
    d.modify("Delete Empty Layers", |doc, _| {
        for p in doc.pages.iter_mut() {
            let mut locals = p.layers.iter().filter(|l| l.master == 0).count();
            p.layers.retain(|l| {
                let keep = !l.objects.is_empty() || (l.master == 0 && locals == 1);
                if !keep && l.master == 0 {
                    locals -= 1;
                }
                keep
            });
        }
    });
    d.on_master = false;
    d.layer = d.doc.pages[d.page].layers.iter().rposition(|l| l.master == 0).unwrap_or(0);
}

/// Select Object to Activate Layer and Expand to Show Selection, when the selection changes.
fn follow_selection(app: &mut AnyVectorApp, st: &mut ObjUi) {
    let (activate, expand) = (app.settings.om_select_activates_layer, app.settings.om_expand_to_selection);
    let Some(d) = app.doc_mut() else { return };
    if d.selection == st.last_sel {
        return;
    }
    st.last_sel = d.selection.clone();
    let Some(&id) = d.selection.first() else { return };
    st.row_sel = None;
    let Some((p, l, _)) = d.doc.locate(id) else { return };
    let master = d.doc.pages[p].layers[l].master != 0;
    if activate && (master || p == d.page) {
        d.layer = l;
        d.on_master = master;
    }
    if expand {
        st.folded.remove(&d.doc.pages[p].id);
        st.folded.remove(&d.doc.pages[p].layers[l].id);
        if master {
            st.folded.remove(&MASTER);
        }
        let mut cur = d.doc.parent_of(id);
        while let Some(g) = cur {
            st.open_groups.insert(g);
            cur = d.doc.parent_of(g);
        }
    }
}

fn build_rows(app: &AnyVectorApp, st: &ObjUi) -> Vec<Row> {
    let Some(d) = app.doc() else { return vec![] };
    let s = &app.settings;
    let filter = app.dockers.ui.objects_filter.trim().to_lowercase();
    let first_sel = d.selection.first().copied().filter(|id| st.last_sel.first() == Some(id));
    let mut rows = vec![];
    let doc = &d.doc;
    let active = d.active_loc();
    let pages: Vec<usize> = if s.om_layers_view { vec![d.page] } else { (0..doc.pages.len()).collect() };
    let guides_bar = Color32::from_rgb(0, 0, 255);
    for pi in pages {
        if !s.om_layers_view {
            let id = doc.pages[pi].id;
            let mut r = Row::new(Kind::PageBand(pi), doc.page_name(pi));
            r.fold = Some(!st.folded.contains(&id));
            r.bold = true;
            rows.push(r);
            if st.folded.contains(&id) {
                continue;
            }
        }
        let mut g = Row::new(Kind::Special(pi, GUIDES), "Guides".into());
        g.icon = "om_guides";
        g.flags = [Some(doc.settings.show_guides), None, Some(false)];
        g.bar = Some(guides_bar);
        g.line_below = true;
        g.selected = st.row_sel == Some((pi, GUIDES));
        rows.push(g);
        let mut layers: Vec<(usize, usize, &Layer)> = vec![];
        if s.om_show_master_on_pages {
            layers.extend(doc.layers_on_page(pi).into_iter().filter(|(_, _, l)| l.master != 0).rev());
        }
        layers.extend(doc.pages[pi].layers.iter().enumerate().filter(|(_, l)| l.master == 0).rev().map(|(i, l)| (pi, i, l)));
        for (p, li, l) in layers {
            layer_rows(&mut rows, st, d, p, li, l, active, &filter, first_sel);
        }
    }
    if !s.om_layers_view {
        let mut r = Row::new(Kind::MasterBand, "Master Page".into());
        r.fold = Some(!st.folded.contains(&MASTER));
        rows.push(r);
        if !st.folded.contains(&MASTER) {
            for (li, l) in doc.master_layers().into_iter().rev() {
                layer_rows(&mut rows, st, d, 0, li, l, active, &filter, first_sel);
            }
            let specials = [
                (MASTER_GUIDES, "Guides (all pages)", "om_guides", [Some(doc.settings.show_guides), None, Some(false)], guides_bar),
                (DESKTOP, "Desktop", "om_desktop", [None, None, Some(false)], Color32::from_rgb(34, 31, 32)),
                (DOC_GRID, "Document Grid", "om_grid", [Some(doc.settings.show_grid), Some(true), Some(false)], Color32::from_rgb(194, 194, 193)),
            ];
            for (k, name, icon, flags, bar) in specials {
                let mut r = Row::new(Kind::Special(0, k), name.into());
                r.icon = icon;
                r.flags = flags;
                r.bar = Some(bar);
                r.line_below = true;
                r.selected = st.row_sel.is_some_and(|(_, x)| x == k);
                rows.push(r);
            }
        }
    }
    rows
}

#[allow(clippy::too_many_arguments)]
fn layer_rows(rows: &mut Vec<Row>, st: &ObjUi, d: &crate::docview::DocView, pi: usize, li: usize, l: &Layer, active: (usize, usize), filter: &str, first_sel: Option<ObjId>) {
    let mut r = Row::new(Kind::Layer(pi, li), l.name.clone());
    r.icon = "layer";
    r.bold = true;
    r.fold = (!l.objects.is_empty()).then_some(!st.folded.contains(&l.id));
    r.flags = [Some(l.visible), Some(l.locked), Some(l.printable)];
    r.bar = Some(theme::to_c32(l.color));
    r.selected = st.row_sel == Some((pi, li));
    let _ = active;
    rows.push(r);
    if !st.folded.contains(&l.id) {
        for o in l.objects.iter().rev() {
            object_rows(rows, st, d, o, 0, filter, first_sel);
        }
    }
    if let Some(last) = rows.last_mut() {
        last.line_below = true;
    }
}

fn object_rows(rows: &mut Vec<Row>, st: &ObjUi, d: &crate::docview::DocView, o: &Object, depth: usize, filter: &str, first_sel: Option<ObjId>) {
    let name = o.display_name();
    let kids = o.children();
    let open = st.open_groups.contains(&o.id);
    if filter.is_empty() || name.to_lowercase().contains(filter) {
        let mut r = Row::new(Kind::Obj(o.id), name);
        r.depth = depth;
        r.fold = (!kids.is_empty()).then_some(open);
        r.icon = match &o.kind {
            ObjKind::Text(_) => "text",
            ObjKind::Image(_) => "bitmap",
            ObjKind::Group(_) => "group",
            ObjKind::PowerClip { .. } => "powerclip",
            ObjKind::Symbol(_) => "symbols",
            _ => "",
        };
        r.fill = o.style.fill.main_color();
        r.outline = o.style.outline.color;
        r.flags = [(!o.visible).then_some(false), o.locked.then_some(true), None];
        r.selected = d.selection.contains(&o.id);
        r.scroll = first_sel == Some(o.id);
        rows.push(r);
    }
    if open || !filter.is_empty() {
        for c in kids.into_iter().rev() {
            object_rows(rows, st, d, c, depth + 1, filter, first_sel);
        }
    }
}

fn row_key(k: Kind) -> (u8, u64, u64) {
    match k {
        Kind::PageBand(p) => (0, p as u64, 0),
        Kind::MasterBand => (1, 0, 0),
        Kind::Special(p, s) => (2, p as u64, s as u64),
        Kind::Layer(p, l) => (3, p as u64, l as u64),
        Kind::Obj(id) => (4, id, 0),
    }
}

fn draw_row(app: &mut AnyVectorApp, ui: &mut Ui, st: &mut ObjUi, row: &Row, rh: f32, acts: &mut Vec<Act>) {
    let band = matches!(row.kind, Kind::PageBand(_) | Kind::MasterBand);
    let h = if band { 26.0 } else { rh };
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    if row.scroll {
        resp.scroll_to_me(None);
    }
    let p = ui.painter().clone();
    if band {
        p.rect_filled(r, 0.0, band_gray());
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, line_gray()));
    } else if row.selected {
        p.rect_filled(r, 0.0, sel_fill());
    } else if resp.hovered() {
        p.rect_filled(r, 0.0, theme::HOVER());
    }
    if row.line_below {
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, line_gray()));
    }
    let cy = r.center().y;
    let pointer = resp.interact_pointer_pos();
    // the fold triangle
    let tri_x = r.left() + 6.0 + if let Kind::Obj(_) = row.kind { 20.0 + row.depth as f32 * 14.0 } else { 0.0 };
    let mut chevron = false;
    if let Some(open) = row.fold {
        let c = pos2(tri_x, cy);
        let pts = if open { vec![c + vec2(-4.5, -2.5), c + vec2(4.5, -2.5), c + vec2(0.0, 3.0)] } else { vec![c + vec2(-2.5, -4.5), c + vec2(3.0, 0.0), c + vec2(-2.5, 4.5)] };
        p.add(egui::Shape::convex_polygon(pts, theme::TEXT(), Stroke::NONE));
        chevron = resp.clicked() && pointer.is_some_and(|q| (q.x - tri_x).abs() < 8.0);
    }
    // the switches and the color bar at the right
    let toggles = matches!(row.kind, Kind::Layer(..) | Kind::Obj(_)) || matches!(row.kind, Kind::Special(_, k) if k == GUIDES || k == MASTER_GUIDES || k == DOC_GRID);
    let mut right = r.right();
    if let Some(c) = row.bar {
        let br = Rect::from_center_size(pos2(r.right() - 7.0, cy), vec2(4.0, 16.0));
        p.rect_filled(br, 0.0, c);
        right = br.left() - 2.0;
    }
    // shown, locked and printing: the states that are not the default always show; all three
    // show (as switches) while the pointer is on the row
    let mut toggled = None;
    let icons = [("eye", "eye_off", "Show or hide"), ("lock", "unlock", "Lock or unlock"), ("printer_small", "no_print", "Enable or disable printing and exporting")];
    let on_row = resp.contains_pointer();
    for (k, slot) in [2usize, 1, 0].into_iter().enumerate() {
        let Some(on) = row.flags[slot] else { continue };
        let default = if slot == 1 { !on } else { on };
        let c = pos2(right - 9.0 - k as f32 * 16.0, cy);
        let sr = Rect::from_center_size(c, vec2(15.0, 15.0));
        let sw = toggles.then(|| ui.interact(sr, ui.scope_id().with(("om_sw", row_key(row.kind), slot)), Sense::click()).on_hover_text(icons[slot].2));
        if default && !(on_row && toggles) {
            continue;
        }
        let (on_icon, off_icon, _) = icons[slot];
        let name = if on { on_icon } else { off_icon };
        if let Some(t) = app.icons.get(ui.ctx(), name, 14.0, if default { Tone::Disabled } else { Tone::Normal }) {
            p.image(t.id, Rect::from_center_size(c, vec2(14.0, 14.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        if sw.is_some_and(|s| s.clicked()) {
            toggled = Some(slot);
        }
    }
    // icon or swatch, then the name
    let x;
    match row.kind {
        Kind::PageBand(_) | Kind::MasterBand => x = r.left() + 18.0,
        Kind::Obj(_) => {
            let sx = tri_x + 8.0;
            let size = vec2(16.0, 10.0) * (h / 26.0).min(2.0);
            let sw = Rect::from_min_size(pos2(sx, cy - size.y / 2.0), size);
            if !row.icon.is_empty() {
                if let Some(t) = app.icons.get(ui.ctx(), row.icon, 16.0, Tone::Normal) {
                    p.image(t.id, Rect::from_center_size(sw.center(), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
            } else if let Some(f) = row.fill {
                p.rect_filled(sw, 0.0, theme::to_c32(f));
            } else {
                p.rect_stroke(sw.shrink(0.5), 0.0, Stroke::new(1.0, row.outline.map(theme::to_c32).unwrap_or(theme::TEXT_DIM())), egui::StrokeKind::Inside);
            }
            x = sw.right() + 7.0;
        }
        _ => {
            if let Some(t) = app.icons.get(ui.ctx(), row.icon, 16.0, Tone::Normal) {
                p.image(t.id, Rect::from_center_size(pos2(r.left() + 25.0, cy), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            x = r.left() + 40.0;
        }
    }
    // Show Object Properties: the fill and the outline
    if app.settings.om_show_properties {
        if let Kind::Obj(_) = row.kind {
            for (k, c) in [row.outline, row.fill].into_iter().enumerate() {
                let wr = Rect::from_center_size(pos2(right - 60.0 - k as f32 * 16.0, cy), vec2(11.0, 11.0));
                crate::widgets::paint_well(&p, wr, c);
            }
            right -= 90.0;
        }
    }
    let renaming = match row.kind {
        Kind::Obj(id) => app.dockers.ui.rename.as_ref().is_some_and(|(x, _)| *x == id),
        Kind::Layer(pi, li) => st.rename_layer.as_ref().is_some_and(|(x, _)| *x == (pi, li)),
        _ => false,
    };
    if renaming {
        let er = Rect::from_min_max(pos2(x - 2.0, cy - 10.0), pos2(right - 40.0, cy + 10.0));
        let text = match row.kind {
            Kind::Obj(_) => &mut app.dockers.ui.rename.as_mut().unwrap().1,
            _ => &mut st.rename_layer.as_mut().unwrap().1,
        };
        let te = ui.put(er, egui::TextEdit::singleline(text));
        if te.lost_focus() {
            let n = text.clone();
            match row.kind {
                Kind::Obj(id) => {
                    app.dockers.ui.rename = None;
                    acts.push(Act::RenameObj(id, n));
                }
                Kind::Layer(pi, li) => {
                    st.rename_layer = None;
                    acts.push(Act::RenameLayer(pi, li, n));
                }
                _ => {}
            }
        } else {
            te.request_focus();
        }
    } else {
        let font = if row.bold { theme::bold(12.5) } else { theme::body(12.5) };
        let g = p.layout_no_wrap(row.name.clone(), font, theme::TEXT());
        p.with_clip_rect(Rect::from_min_max(r.min, pos2(right - 4.0, r.max.y)).intersect(ui.clip_rect())).galley(pos2(x, cy - g.size().y / 2.0), g, theme::TEXT());
    }
    // what the clicks do
    let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.shift);
    if chevron {
        match row.kind {
            Kind::PageBand(pi) => {
                if let Some(d) = app.doc() {
                    acts.push(Act::Fold(d.doc.pages[pi].id));
                }
            }
            Kind::MasterBand => acts.push(Act::Fold(MASTER)),
            Kind::Layer(pi, li) => {
                if let Some(d) = app.doc() {
                    acts.push(Act::Fold(d.doc.pages[pi].layers[li].id));
                }
            }
            Kind::Obj(id) => acts.push(Act::FoldGroup(id)),
            _ => {}
        }
    } else if let Some(slot) = toggled {
        acts.push(Act::Toggle(row.kind, slot));
    } else if resp.double_clicked() {
        match row.kind {
            Kind::Obj(id) => app.dockers.ui.rename = Some((id, row.name.clone())),
            Kind::Layer(pi, li) => st.rename_layer = Some(((pi, li), row.name.clone())),
            _ => {}
        }
    } else if resp.clicked() {
        match row.kind {
            Kind::PageBand(pi) => acts.push(Act::GoPage(pi)),
            Kind::Layer(pi, li) => acts.push(Act::ClickLayer(pi, li)),
            Kind::Special(pi, k) => acts.push(Act::ClickSpecial(pi, k)),
            Kind::Obj(id) => acts.push(Act::ClickObj(id, ctrl)),
            Kind::MasterBand => {}
        }
    }
    // right-click menus
    match row.kind {
        Kind::Obj(id) => {
            resp.context_menu(|ui| {
                if let Some(d) = app.doc_mut() {
                    if !d.selection.contains(&id) {
                        d.selection = vec![id];
                    }
                }
                if let Some(name) = crate::menus::canvas_menu(app) {
                    crate::menus::context_menu(app, ui, name);
                }
            });
        }
        Kind::Layer(pi, li) => {
            resp.context_menu(|ui| {
                crate::menus::menu_style(ui);
                layer_menu(app, ui, st, pi, li);
            });
        }
        _ => {}
    }
}

/// The right-click menu of a layer.
fn layer_menu(app: &mut AnyVectorApp, ui: &mut Ui, st: &mut ObjUi, pi: usize, li: usize) {
    let Some(d) = app.doc() else { return };
    let Some(l) = d.doc.pages.get(pi).and_then(|p| p.layers.get(li)) else { return };
    let (name, locked, master) = (l.name.clone(), l.locked, l.master);
    let deletable = layer_deletable(&d.doc, pi, li);
    let odd = d.page % 2 == 0;
    if item(app, ui, "&Rename", true, false) {
        st.rename_layer = Some(((pi, li), name));
    }
    if item(app, ui, "&Delete", deletable, false) {
        delete_layer(app, pi, li);
    }
    separator(ui);
    if item(app, ui, if locked { "Un&lock" } else { "&Lock" }, true, false) {
        set_layer(app, pi, li, |l| l.locked = !l.locked);
    }
    submenu(app, ui, "&Change Layer To", true, |app, ui| {
        for (k, text, on) in [(0u8, "&Local Layer", master != 0), (1, "Master Layer (&all pages)", master != 1), (2, "Master Layer (&odd pages)", master != 2 && odd), (3, "Master Layer (&even pages)", master != 3 && !odd)] {
            if item(app, ui, text, on, master == k) {
                change_layer(app, pi, li, k);
            }
        }
    });
}

/// Change Layer To: a local layer of the current page, or a master layer of a kind.
fn change_layer(app: &mut AnyVectorApp, pi: usize, li: usize, kind: u8) {
    let Some(d) = app.doc_mut() else { return };
    let cur = d.page;
    let loc = d.modify("Change Layer To", |doc, _| {
        let mut l = doc.pages[pi].layers.remove(li);
        let suffix = |k: u8| match k {
            1 => " (all pages)",
            2 => " (odd pages)",
            3 => " (even pages)",
            _ => "",
        };
        if let Some(base) = l.name.strip_suffix(suffix(l.master)).filter(|_| l.master != 0) {
            l.name = base.to_string();
        }
        l.name.push_str(suffix(kind));
        l.master = kind;
        if kind == 0 {
            let p = &mut doc.pages[cur];
            let at = p.layers.iter().position(|x| x.master != 0).unwrap_or(p.layers.len());
            p.layers.insert(at, l);
            (cur, at)
        } else {
            doc.pages[0].layers.push(l);
            (0, doc.pages[0].layers.len() - 1)
        }
    });
    d.layer = loc.1;
    d.on_master = kind != 0;
}

fn set_layer(app: &mut AnyVectorApp, pi: usize, li: usize, f: impl Fn(&mut Layer)) {
    if let Some(d) = app.doc_mut() {
        d.modify("Layer Properties", |doc, _| {
            if let Some(l) = doc.pages.get_mut(pi).and_then(|p| p.layers.get_mut(li)) {
                f(l);
            }
        });
    }
}

fn set_obj(app: &mut AnyVectorApp, id: ObjId, label: &str, f: impl Fn(&mut Object)) {
    if let Some(d) = app.doc_mut() {
        d.modify(label, |doc, _| {
            if let Some(o) = doc.find_mut(id) {
                f(o);
            }
        });
    }
}

fn go_page(app: &mut AnyVectorApp, pi: usize) {
    if let Some(d) = app.doc_mut() {
        if d.page != pi {
            d.page = pi;
            d.on_master = false;
            d.layer = d.doc.pages[pi].layers.iter().rposition(|l| l.master == 0).unwrap_or(0);
            d.selection.clear();
            d.node_sel.clear();
            d.needs_fit = true;
        }
    }
}

fn apply(app: &mut AnyVectorApp, st: &mut ObjUi, acts: Vec<Act>) {
    for a in acts {
        match a {
            Act::Fold(k) => {
                if !st.folded.remove(&k) {
                    st.folded.insert(k);
                }
            }
            Act::FoldGroup(id) => {
                if !st.open_groups.remove(&id) {
                    st.open_groups.insert(id);
                }
            }
            Act::GoPage(pi) => go_page(app, pi),
            Act::ClickLayer(pi, li) => {
                if let Some(copy) = st.pending.take() {
                    move_to_layer(app, pi, li, copy);
                    continue;
                }
                let Some(d) = app.doc_mut() else { continue };
                let master = d.doc.pages[pi].layers[li].master != 0;
                if !master && pi != d.page {
                    go_page(app, pi);
                }
                let Some(d) = app.doc_mut() else { continue };
                d.layer = li;
                d.on_master = master;
                d.selection.clear();
                st.row_sel = Some((pi, li));
                st.last_sel.clear();
            }
            Act::ClickSpecial(pi, k) => {
                st.pending = None;
                if k == GUIDES {
                    go_page(app, pi);
                }
                st.row_sel = Some((pi, k));
            }
            Act::ClickObj(id, add) => {
                st.pending = None;
                st.row_sel = None;
                let activate = app.settings.om_select_activates_layer;
                let Some(d) = app.doc_mut() else { continue };
                if let Some((p, l, _)) = d.doc.locate(id) {
                    let master = d.doc.pages[p].layers[l].master != 0;
                    if !master && p != d.page {
                        go_page(app, p);
                    }
                    let Some(d) = app.doc_mut() else { continue };
                    if activate {
                        d.layer = l;
                        d.on_master = master;
                    }
                }
                let Some(d) = app.doc_mut() else { continue };
                if add {
                    if let Some(i) = d.selection.iter().position(|x| *x == id) {
                        d.selection.remove(i);
                    } else {
                        d.selection.push(id);
                    }
                } else {
                    d.selection = vec![id];
                }
                st.last_sel = d.selection.clone();
            }
            Act::RenameObj(id, n) => set_obj(app, id, "Rename", move |o| o.name = n.clone()),
            Act::RenameLayer(pi, li, n) => set_layer(app, pi, li, move |l| l.name = n.clone()),
            Act::Toggle(kind, slot) => match kind {
                Kind::Layer(pi, li) => set_layer(app, pi, li, |l| match slot {
                    0 => l.visible = !l.visible,
                    1 => l.locked = !l.locked,
                    _ => l.printable = !l.printable,
                }),
                Kind::Obj(id) => set_obj(app, id, if slot == 0 { "Visibility" } else { "Lock" }, |o| match slot {
                    0 => o.visible = !o.visible,
                    _ => o.locked = !o.locked,
                }),
                Kind::Special(_, k) if slot == 0 => {
                    if let Some(d) = app.doc_mut() {
                        if k == DOC_GRID {
                            d.doc.settings.show_grid = !d.doc.settings.show_grid;
                        } else {
                            d.doc.settings.show_guides = !d.doc.settings.show_guides;
                        }
                        d.cache.key = 0;
                    }
                }
                _ => {}
            },
        }
    }
}

/// Move to Layer / Copy to Layer: the selected objects to the clicked layer.
fn move_to_layer(app: &mut AnyVectorApp, pi: usize, li: usize, copy: bool) {
    let Some(d) = app.doc_mut() else { return };
    let ids = d.selection.clone();
    d.modify(if copy { "Copy to Layer" } else { "Move to Layer" }, |doc, sel| {
        let mut objs = vec![];
        for id in &ids {
            if copy {
                if let Some(mut o) = doc.find(*id).cloned() {
                    doc.reassign_ids(&mut o);
                    objs.push(o);
                }
            } else if doc.top_level_of(*id) == Some(*id) {
                if let Some(o) = doc.remove(*id) {
                    objs.push(o);
                }
            }
        }
        *sel = objs.iter().map(|o| o.id).collect();
        if let Some(l) = doc.pages.get_mut(pi).and_then(|p| p.layers.get_mut(li)) {
            l.objects.extend(objs);
        }
    });
}

fn bottom_bar(app: &mut AnyVectorApp, ui: &mut Ui, st: &mut ObjUi) {
    let Some(d) = app.doc() else { return };
    let odd = d.page % 2 == 0;
    let has_sel = !d.selection.is_empty();
    let layer_sel = st.row_sel.filter(|(_, l)| *l < DOC_GRID);
    let deletable = has_sel || layer_sel.is_some_and(|(p, l)| layer_deletable(&d.doc, p, l));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if icon_button(app, ui, "layer_new", "New Layer", true) {
            new_layer(app);
        }
        let ml = icon_button_resp(app, ui, "om_master_new", "New Master Layer", true);
        drop_mark(ui, ml.rect);
        egui::Popup::menu(&ml).show(|ui| {
            crate::menus::menu_style(ui);
            for (k, text, on) in [(1u8, "New Master Layer (&all pages)", true), (2, "New Master Layer (&odd pages)", odd), (3, "New Master Layer (&even pages)", !odd)] {
                if item(app, ui, text, on, false) {
                    new_master_layer(app, k);
                }
            }
        });
        let fx = icon_button_resp(app, ui, "fx_add", "Add Effects", has_sel);
        egui::Popup::menu(&fx).show(|ui| {
            crate::menus::menu_style(ui);
            if let Some((i, _)) = crate::ui::bar_named("Effects") {
                crate::menus::show_bar(app, ui, i);
            }
        });
        if icon_button(app, ui, "delete", "Delete", deletable) {
            if has_sel {
                app.run(Cmd::Delete);
            } else if let Some((p, l)) = layer_sel {
                delete_layer(app, p, l);
                st.row_sel = None;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            list_slider(ui, &mut st.size, "Resize list", 2);
        });
    });
}

/// The resize slider at the bottom of the Objects and Pages dockers: a round knob on a line,
/// with `ticks` tick marks.
fn list_slider(ui: &mut Ui, v: &mut f32, tip: &str, ticks: usize) {
    let (r, resp) = ui.allocate_exact_size(vec2(110.0, 20.0), Sense::click_and_drag());
    let resp = resp.on_hover_text(tip);
    let (x0, x1) = (r.left() + 6.0, r.right() - 6.0);
    if let Some(q) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() {
            *v = ((q.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
        }
    }
    let p = ui.painter();
    let cy = r.center().y;
    let kx = x0 + *v * (x1 - x0);
    p.hline(x0..=kx, cy, Stroke::new(3.0, theme::FOCUS()));
    p.hline(kx..=x1, cy, Stroke::new(3.0, theme::gray(0x55)));
    for k in 1..=ticks {
        let tx = x0 + (x1 - x0) * k as f32 / (ticks + 1) as f32 + 6.0;
        p.vline(tx, (cy + 5.0)..=(cy + 8.0), Stroke::new(1.5, theme::TEXT()));
    }
    p.circle(pos2(kx, cy), 5.5, theme::BG(), Stroke::new(1.5, theme::FOCUS()));
}

// ------------------------------------------------------------------------------- Pages docker

pub fn pages(app: &mut AnyVectorApp, ui: &mut Ui) {
    if !app.has_doc() {
        return no_doc(ui);
    }
    let mut st = std::mem::take(&mut app.dockers.ui.pg);
    pages_body(app, ui, &mut st);
    app.dockers.ui.pg = st;
}

fn pages_body(app: &mut AnyVectorApp, ui: &mut Ui, st: &mut PagesUi) {
    ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.pages_filter).hint_text("Search").desired_width(f32::INFINITY));
    let multi = app.doc().map(|d| d.doc.settings.multipage_view).unwrap_or(false);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (m, icon, tip) in [(false, "pg_single", "Single page view"), (true, "pg_multi", "Multipage view")] {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, multi == m).clicked() && multi != m {
                if let Some(d) = app.doc_mut() {
                    d.doc.settings.multipage_view = m;
                    d.needs_fit = true;
                    d.cache.key = 0;
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let gear = icon_button_resp(app, ui, "gear", "Options", true);
            drop_mark(ui, gear.rect);
            egui::Popup::menu(&gear).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                crate::menus::menu_style(ui);
                page_items(app, ui, Some(st.sel));
            });
            let mv = icon_button_resp(app, ui, "pg_grid", "Multipage view settings", multi);
            drop_mark(ui, mv.rect);
            if multi {
                egui::Popup::menu(&mv).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                    crate::menus::menu_style(ui);
                    multipage_settings(app, ui);
                });
            }
        });
    });
    // the pages
    let Some(d) = app.doc() else { return };
    let q = app.dockers.ui.pages_filter.trim().to_lowercase();
    let n = d.doc.pages.len();
    let (cur, rev, uid) = (d.page, d.history.revision, d.uid);
    let names: Vec<String> = (0..n).map(|i| d.doc.page_name(i)).collect();
    let th = if st.list { 0.0 } else { 58.0 + st.size * 90.0 };
    let rh = if st.list { 26.0 } else { th + 10.0 };
    let tree_h = (ui.clip_rect().bottom() - ui.cursor().top() - 36.0).max(60.0);
    let mut go = None;
    let mut menu_page = None;
    egui::ScrollArea::vertical().id_salt("pg_list").max_height(tree_h).min_scrolled_height(tree_h).auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, line_gray()));
        for (i, name) in names.iter().enumerate() {
            if !q.is_empty() && !name.to_lowercase().contains(&q) {
                continue;
            }
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), rh), Sense::click());
            let p = ui.painter().clone();
            if st.sel == Some(i) {
                p.rect_filled(r, 0.0, sel_fill());
            } else if resp.hovered() {
                p.rect_filled(r, 0.0, theme::HOVER());
            }
            p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, line_gray()));
            let mut x = r.left() + 14.0;
            if !st.list {
                let key = format!("pgdk:{uid}:{i}:{rev}:{}", th as u32);
                if !app.ts.texture_cache.contains_key(&key) {
                    app.ts.texture_cache.retain(|k, _| !k.starts_with(&format!("pgdk:{uid}:{i}:")));
                    let d = app.doc().unwrap();
                    let pg = &d.doc.pages[i];
                    let dpi = (th as f64 * 2.0 * 72.0 / pg.width.max(pg.height).max(1.0)).max(1.0);
                    if let Some(pm) = vector_render::render_page_image(&d.doc, i, dpi, Some(vector_core::Color::WHITE), None, true) {
                        let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
                        let tex = ui.ctx().load_texture(key.clone(), img, egui::TextureOptions::LINEAR);
                        app.ts.texture_cache.insert(key.clone(), tex);
                    }
                }
                if let Some(tex) = app.ts.texture_cache.get(&key) {
                    let s = tex.size_vec2();
                    let k = (th / s.y).min((th * 1.3) / s.x);
                    let ir = Rect::from_min_size(pos2(r.left() + 14.0, r.center().y - s.y * k / 2.0), s * k);
                    p.image(tex.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    x = ir.right() + 13.0;
                } else {
                    x += th * 0.8 + 13.0;
                }
            }
            if st.rename.as_ref().is_some_and(|(k, _)| *k == i) {
                let text = &mut st.rename.as_mut().unwrap().1;
                let te = ui.put(Rect::from_min_max(pos2(x - 2.0, r.center().y - 10.0), pos2(r.right() - 8.0, r.center().y + 10.0)), egui::TextEdit::singleline(text));
                if te.lost_focus() {
                    let n = text.clone();
                    st.rename = None;
                    if let Some(d) = app.doc_mut() {
                        d.modify("Rename Page", |doc, _| doc.pages[i].name = n);
                    }
                } else {
                    te.request_focus();
                }
            } else {
                let font = if i == cur { theme::bold(12.5) } else { theme::body(12.5) };
                p.text(pos2(x, r.center().y), Align2::LEFT_CENTER, name, font, theme::TEXT());
            }
            if resp.double_clicked() {
                st.rename = Some((i, name.clone()));
            } else if resp.clicked() {
                st.sel = Some(i);
                go = Some(i);
            }
            resp.context_menu(|ui| {
                crate::menus::menu_style(ui);
                menu_page = Some(i);
                if item(app, ui, "&Rename", true, false) {
                    st.rename = Some((i, names[i].clone()));
                }
                if item(app, ui, "&Delete", n > 1, false) {
                    if let Some(d) = app.doc_mut() {
                        d.page = i;
                    }
                    app.run(Cmd::DeletePageNow);
                    st.sel = None;
                }
            });
        }
    });
    let _ = menu_page;
    if let Some(i) = go {
        go_page(app, i);
    }
    // New Page, Delete, the list view and the size of the thumbnails
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if icon_button(app, ui, "pg_new", "New Page", true) {
            app.run(Cmd::InsertPageAfter);
        }
        if icon_button(app, ui, "delete", "Delete Page", st.sel.is_some() && n > 1) {
            if let (Some(p), Some(d)) = (st.sel, app.doc_mut()) {
                d.page = p;
            }
            app.run(Cmd::DeletePageNow);
            st.sel = None;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            list_slider(ui, &mut st.size, "Resize list", 2);
            if crate::widgets::icon_button(&mut app.icons, ui, "pg_list", "List view", true, st.list).clicked() {
                st.list = !st.list;
            }
        });
    });
}

/// Multipage view settings: the arrangement of the pages (Grid, Vertical, Horizontal, Custom),
/// the columns of the grid and the spacing between pages.
fn multipage_settings(app: &mut AnyVectorApp, ui: &mut Ui) {
    let unit = app.doc().map(|d| d.doc.settings.unit).unwrap_or_default();
    let dpi = app.doc().map(|d| d.doc.settings.dpi).unwrap_or(96.0);
    let Some(d) = app.doc_mut() else { return };
    let mut s = d.doc.settings.clone();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        for (k, icon, tip) in [(0u8, "pg_grid", "Grid"), (1, "dist_vspace", "Vertical"), (2, "dist_hspace", "Horizontal"), (3, "arrangement", "Custom")] {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, true, s.multipage_arrangement == k).clicked() {
                s.multipage_arrangement = k;
            }
        }
    });
    egui::Grid::new("mp_settings").num_columns(2).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
        ui.add_enabled(s.multipage_arrangement == 0, egui::Label::new("Columns:"));
        ui.add_enabled_ui(s.multipage_arrangement == 0, |ui| {
            super::spin(ui, &mut s.multipage_columns, 1..=99, 1.0, 0, "", 70.0);
        });
        ui.end_row();
        ui.label("Spacing:");
        crate::widgets::unit_field(ui, egui::Id::unique("mp_spacing"), &mut s.multipage_spacing, unit, dpi, 90.0, true);
        ui.end_row();
    });
    let Some(d) = app.doc_mut() else { return };
    if s != d.doc.settings {
        d.doc.settings = s;
        d.cache.key = 0;
    }
}
