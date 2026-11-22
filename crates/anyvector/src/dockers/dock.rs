//! The dockers drawn where their layout puts them (#144): a docker area on each side of the
//! drawing window with its groups stacked top to bottom (a divider between them), floating
//! docker windows, and dragging a tab or a group's title bar to rearrange them.
//!
//! As the command-bar customization does (`crate::customize`), a drag is a record the frame
//! keeps; each group notes where it was drawn, and after everything is drawn [`end_frame`]
//! shows where a drop would go and makes it when the button is let go:
//!
//! - on a group's title bar or tabs: into its tabs, where the line shows;
//! - on the top or bottom quarter of a docked group: a new group stacked above or below it;
//! - on the middle of a group: into its tabs, at the end;
//! - by the left or right edge of the drawing window: a new group on that side;
//! - anywhere else: a floating window there.
//!
//! A floating window dragged by its title bar moves, and docks when it is let go on one of the
//! places above. Double-clicking a tab floats it, or docks a floating one back.

use super::layout::{DockerSide, Target, MAX_WIDTH, MIN_FLOAT, MIN_SHARE, MIN_WIDTH};
use super::{docker_content, icon, title, DockerId, DockerState, ALL, TAB_STRIP};
use crate::app::AnyVectorApp;
use crate::icons::Tone;
use crate::theme;
use egui::{pos2, vec2, Color32, CursorIcon, Id, LayerId, Order, Pos2, Rect, Sense, Stroke, Ui};

/// Height of a group's title bar.
const HEADER: f32 = 28.0;
/// Height of the divider between groups stacked on a side.
const DIVIDER: f32 = 5.0;
/// How near the edge of the drawing window a drop docks on that side.
const EDGE: f32 = 48.0;

/// What is being dragged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Dragged {
    /// One docker, by its tab.
    Tab(DockerId),
    /// A docked group, by its title bar (any of its dockers names it).
    Group(DockerId),
    /// A floating window, by its title bar: it moves with the pointer.
    Window(DockerId),
}

impl Dragged {
    fn docker(self) -> DockerId {
        match self {
            Dragged::Tab(d) | Dragged::Group(d) | Dragged::Window(d) => d,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DockDrag {
    pub what: Dragged,
}

/// Where a group was drawn this frame.
#[derive(Clone, Debug)]
pub(crate) struct Hit {
    /// A docker of the group (its first): names the group even when the layout changes.
    pub key: DockerId,
    pub group: usize,
    pub floating: bool,
    /// The tabs of a group on a folded side: a drop there only joins the group.
    pub folded: bool,
    pub layer: LayerId,
    /// The whole group, its title bar and its tab strip.
    pub block: Rect,
    pub head: Rect,
    pub strip: Rect,
    /// The tabs, top to bottom.
    pub tabs: Vec<(DockerId, Rect)>,
}

/// Something clicked on a docker's tab, title bar or menu, carried out after the frame is drawn
/// (so the groups being drawn don't change under it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Act {
    /// A tab clicked: shows its docker, or folds the side when it is the one shown.
    Click(DockerId),
    Fold(DockerSide, bool),
    Close(DockerId),
    /// Closes the other dockers of its group.
    CloseOthers(DockerId),
    /// The + menu: opens a docker in the group of the other one.
    OpenWith(DockerId, DockerId),
    Dock(DockerId, DockerSide),
    DockGroup(DockerId, DockerSide),
    Float(DockerId),
    FloatGroup(DockerId),
    /// A tab double-clicked: floats it, or docks a floating one back.
    ToggleFloat(DockerId),
    /// A title bar double-clicked: the same for the whole group.
    ToggleFloatGroup(DockerId),
    Reset,
}

/// What a docker area keeps from one frame to the next (not saved).
pub(crate) struct Live {
    pub drag: Option<DockDrag>,
    /// The groups drawn this frame, and those drawn last frame.
    pub hits: Vec<Hit>,
    pub drawn: Vec<Hit>,
    /// The drawing window last frame.
    pub central: Rect,
    /// A width each side (left, right) takes on the next frame.
    pub width: [Option<f32>; 2],
    pub acts: Vec<Act>,
}

impl Default for Live {
    fn default() -> Live {
        Live { drag: None, hits: vec![], drawn: vec![], central: Rect::NOTHING, width: [None; 2], acts: vec![] }
    }
}

/// Saves the layout at once (a crash then can't lose it).
fn commit(app: &mut AnyVectorApp) {
    app.dockers.save(&mut app.settings);
    app.settings.save();
}

// ------------------------------------------------------------------------------------------
// The side areas
// ------------------------------------------------------------------------------------------

/// The docker area on one side of the drawing window, when dockers are docked there. The left
/// one is made after the toolbox and the right one after the color palettes, so each sits
/// between them and the drawing window.
pub fn side_panel(app: &mut AnyVectorApp, ui: &mut Ui, side: DockerSide) {
    app.dockers.sync();
    if app.dockers.layout.side_groups(side).is_empty() {
        return;
    }
    let frame = egui::Frame::new().fill(theme::BG()).inner_margin(egui::Margin::same(0));
    let (open_id, folded_id) = match side {
        DockerSide::Left => ("dockers_left", "dockers_left_folded"),
        DockerSide::Right => ("dockers", "dockers_folded"),
    };
    let panel = |id: &str| match side {
        DockerSide::Left => egui::Panel::left(id),
        DockerSide::Right => egui::Panel::right(id),
    };
    if app.dockers.side_collapsed(side) {
        panel(folded_id).frame(frame).exact_size(TAB_STRIP + 2.0).show(ui, |ui| folded(app, ui, side));
        return;
    }
    // with both sides open, each leaves the drawing window some room
    let both = DockerSide::BOTH.iter().all(|s| !app.dockers.layout.side_groups(*s).is_empty() && !app.dockers.side_collapsed(*s));
    let room = (ui.ctx().content_rect().width() - 420.0) / if both { 2.0 } else { 1.0 };
    let max = room.clamp(MIN_WIDTH, MAX_WIDTH);
    let w = app.dockers.layout.width(side).min(max);
    let mut p = panel(open_id).frame(frame).resizable(true).default_size(w).size_range(MIN_WIDTH..=max);
    if let Some(w) = app.dockers.take_width(side) {
        p = p.exact_size(w.clamp(MIN_WIDTH, max));
    }
    let r = p.show(ui, |ui| expanded(app, ui, side));
    *app.dockers.layout.width_mut(side) = r.response.rect.width().clamp(MIN_WIDTH, MAX_WIDTH);
}

impl DockerState {
    /// A width asked for the side: by a workspace, or `set_width` for the side of the docker
    /// brought forward last.
    fn take_width(&mut self, side: DockerSide) -> Option<f32> {
        let mine = self.active.and_then(|a| self.side_of(a)).unwrap_or_default() == side;
        let asked = if mine { self.set_width.take() } else { None };
        let workspace = self.live.width[side.index()].take();
        asked.or(workspace)
    }
}

/// The keys (first dockers) of a side's groups, top to bottom.
fn side_keys(app: &AnyVectorApp, side: DockerSide) -> Vec<DockerId> {
    let l = &app.dockers.layout;
    l.side_groups(side).into_iter().map(|g| l.groups[g].dockers[0]).collect()
}

/// A side unfolded: its groups top to bottom, as much of the height each as its share says, a
/// divider between two of them.
fn expanded(app: &mut AnyVectorApp, ui: &mut Ui, side: DockerSide) {
    let full = ui.max_rect();
    // the panel keeps the width it is given (a panel shrinks to what its contents use)
    ui.expand_to_include_rect(full);
    let keys = side_keys(app, side);
    let n = keys.len();
    let avail = (full.height() - DIVIDER * (n.saturating_sub(1)) as f32).max(1.0);
    let mut rects = Vec::with_capacity(n);
    let mut y = full.top();
    for (k, key) in keys.iter().enumerate() {
        let share = app.dockers.layout.group_of(*key).map(|g| app.dockers.layout.groups[g].share).unwrap_or(1.0);
        let h = if k + 1 == n { full.bottom() - y } else { (avail * share).round() };
        rects.push(Rect::from_min_max(pos2(full.left(), y), pos2(full.right(), y + h)));
        y += h + DIVIDER;
    }
    for (k, key) in keys.iter().enumerate() {
        block(app, ui, *key, rects[k], Spot { side, floating: false, top: k == 0 });
    }
    // the dividers: dragged, the groups on either side trade height
    for k in 1..n {
        let r = Rect::from_min_max(pos2(full.left(), rects[k - 1].bottom()), pos2(full.right(), rects[k].top()));
        let resp = ui.interact(r, Id::unique(("dk_divider", side, k)), Sense::drag()).on_hover_cursor(CursorIcon::ResizeVertical);
        let p = ui.painter();
        p.rect_filled(r, 0.0, if resp.hovered() || resp.dragged() { theme::HOVER() } else { theme::BAR() });
        p.line_segment([pos2(r.center().x - 12.0, r.center().y), pos2(r.center().x + 12.0, r.center().y)], Stroke::new(1.0, theme::TEXT_DIM()));
        if resp.dragged() {
            let ds = resp.drag_delta().y / avail;
            let l = &mut app.dockers.layout;
            if let (Some(a), Some(b)) = (l.group_of(keys[k - 1]), l.group_of(keys[k])) {
                let (sa, sb) = (l.groups[a].share, l.groups[b].share);
                let ds = ds.clamp(MIN_SHARE - sa, sb - MIN_SHARE);
                l.groups[a].share = sa + ds;
                l.groups[b].share = sb - ds;
            }
        }
        if resp.drag_stopped() {
            commit(app);
        }
    }
}

/// A side folded to its tabs: the tabs of each of its groups down the edge, one group under
/// the other.
fn folded(app: &mut AnyVectorApp, ui: &mut Ui, side: DockerSide) {
    let full = ui.max_rect();
    ui.expand_to_include_rect(full);
    let p = ui.painter().clone();
    p.rect_filled(full, 0.0, theme::BG());
    let inner = match side {
        DockerSide::Right => full.left(),
        DockerSide::Left => full.right(),
    };
    p.line_segment([pos2(inner, full.top()), pos2(inner, full.bottom())], Stroke::new(1.0, theme::SEP()));
    let header = Rect::from_min_size(full.min, vec2(full.width(), HEADER));
    let x = strip_x(full, side, false);
    let fold = Rect::from_center_size(pos2(x + (TAB_STRIP - 4.0) / 2.0, header.center().y), vec2(20.0, 20.0));
    if fold_button(ui, &p, fold, side, true).clicked() {
        app.dockers.live.acts.push(Act::Fold(side, false));
    }
    let mut y = header.bottom() + 4.0;
    let keys = side_keys(app, side);
    for key in keys.iter().copied() {
        let Some(g) = app.dockers.layout.group_of(key) else { continue };
        let dockers = app.dockers.layout.groups[g].dockers.clone();
        let top = y;
        let mut tabs = vec![];
        for d in dockers {
            let (resp, r) = tab(app, ui, &p, x, y, d, false, side);
            tab_input(app, &resp, d);
            tabs.push((d, r));
            y = r.bottom() + 2.0;
        }
        let block = Rect::from_min_max(pos2(full.left(), top - 2.0), pos2(full.right(), y + 2.0));
        app.dockers.live.hits.push(Hit { key, group: g, floating: false, folded: true, layer: ui.layer_id(), block, head: block, strip: block, tabs });
        y += 8.0;
    }
    // + : opens a docker in the side's last group
    if let Some(&last) = keys.last() {
        plus_button(app, ui, &p, x, y - 8.0, last);
    }
}

/// The left edge of the tab strip: at the outer edge of the side (a floating window's at its
/// right).
fn strip_x(full: Rect, side: DockerSide, floating: bool) -> f32 {
    if side == DockerSide::Left && !floating {
        full.left() + 4.0
    } else {
        full.right() - TAB_STRIP
    }
}

/// The fold button: two arrowheads pointing to the edge (fold) or away from it (unfold).
fn fold_button(ui: &mut Ui, p: &egui::Painter, r: Rect, side: DockerSide, folded: bool) -> egui::Response {
    let tip = if folded { "Expand Dockers" } else { "Collapse Dockers" };
    let resp = ui.interact(r, Id::unique(("dk_fold", side)), Sense::click()).on_hover_text(tip);
    if resp.hovered() {
        p.rect_filled(r, 2.0, theme::HOVER());
    }
    // to the right: fold a right side, unfold a left one
    let right = folded == (side == DockerSide::Left);
    for dx in [-3.5f32, 2.5] {
        let c = r.center() + vec2(dx, 0.0);
        let pts = if right { vec![c + vec2(-2.5, -4.0), c + vec2(2.5, 0.0), c + vec2(-2.5, 4.0)] } else { vec![c + vec2(2.5, -4.0), c + vec2(-2.5, 0.0), c + vec2(2.5, 4.0)] };
        p.add(egui::Shape::convex_polygon(pts, theme::TEXT(), Stroke::NONE));
    }
    resp
}

/// Where a group is drawn.
#[derive(Clone, Copy)]
struct Spot {
    side: DockerSide,
    floating: bool,
    /// The first group of its side: its title bar has the fold button.
    top: bool,
}

/// A group: its title bar (the docker shown, then fold and close), the tabs of its dockers down
/// the edge and the docker shown.
fn block(app: &mut AnyVectorApp, ui: &mut Ui, key: DockerId, full: Rect, spot: Spot) {
    let Some(g) = app.dockers.layout.group_of(key) else { return };
    let grp = app.dockers.layout.groups[g].clone();
    let Some(active) = grp.active.or(grp.dockers.first().copied()) else { return };
    let p = ui.painter().with_clip_rect(full.intersect(ui.clip_rect()));
    p.rect_filled(full, 0.0, theme::BG());
    if !spot.floating {
        // the border toward the drawing window
        let x = if spot.side == DockerSide::Left { full.right() } else { full.left() };
        p.line_segment([pos2(x, full.top()), pos2(x, full.bottom())], Stroke::new(1.0, theme::SEP()));
    }
    let header = Rect::from_min_size(full.min, vec2(full.width(), HEADER));
    let close = Rect::from_center_size(pos2(header.right() - 14.0, header.center().y), vec2(20.0, 20.0));
    let fold = Rect::from_center_size(pos2(header.right() - 38.0, header.center().y), vec2(20.0, 20.0));
    let has_fold = spot.top && !spot.floating;
    let buttons_left = if has_fold { fold.left() } else { close.left() };
    // the title bar: dragged, the group moves (a floating window with it)
    let bar = Rect::from_min_max(header.min, pos2(buttons_left - 2.0, header.bottom()));
    let hr = ui.interact(bar, Id::unique(("dk_head", key)), Sense::click_and_drag());
    if spot.floating {
        p.rect_filled(header, 0.0, theme::BAR());
    }
    p.with_clip_rect(bar).text(pos2(header.left() + 8.0, header.center().y), egui::Align2::LEFT_CENTER, title(active), theme::bold(13.0), theme::TEXT());
    head_input(app, &hr, key, spot.floating);
    let cr = ui.interact(close, Id::unique(("dk_close", key)), Sense::click()).on_hover_text("Close");
    if cr.hovered() {
        p.rect_filled(close, 2.0, theme::HOVER());
    }
    let s = 4.5;
    p.line_segment([close.center() + vec2(-s, -s), close.center() + vec2(s, s)], Stroke::new(1.3, theme::TEXT()));
    p.line_segment([close.center() + vec2(s, -s), close.center() + vec2(-s, s)], Stroke::new(1.3, theme::TEXT()));
    if cr.clicked() {
        app.dockers.live.acts.push(Act::Close(active));
    }
    if has_fold && fold_button(ui, &p, fold, spot.side, false).clicked() {
        app.dockers.live.acts.push(Act::Fold(spot.side, true));
    }
    // the tabs, down the edge, apart from the docker
    let x = strip_x(full, spot.side, spot.floating);
    let sep_x = if x < full.center().x { x + TAB_STRIP - 1.5 } else { x - 2.5 };
    p.line_segment([pos2(sep_x, header.bottom()), pos2(sep_x, full.bottom())], Stroke::new(1.0, theme::SEP()));
    let mut y = header.bottom() + 4.0;
    let mut tabs = vec![];
    let dragged = app.dockers.live.drag.map(|d| d.what);
    for d in grp.dockers.iter().copied() {
        let (resp, r) = tab(app, ui, &p, x, y, d, d == active, if spot.floating { DockerSide::Right } else { spot.side });
        if dragged == Some(Dragged::Tab(d)) {
            p.rect_filled(r, 2.0, theme::BG().gamma_multiply(0.6));
        }
        tab_input(app, &resp, d);
        tabs.push((d, r));
        y = r.bottom() + 2.0;
    }
    plus_button(app, ui, &p, x, y, key);
    let strip = Rect::from_min_max(pos2(x - 2.0, header.bottom()), pos2(x + TAB_STRIP - 2.0, full.bottom()));
    app.dockers.live.hits.push(Hit { key, group: g, floating: spot.floating, folded: false, layer: ui.layer_id(), block: full, head: header, strip, tabs });
    // the docker
    let content = if x < full.center().x { Rect::from_min_max(pos2(x + TAB_STRIP + 1.0, header.bottom()), pos2(full.right() - 1.0, full.bottom())) } else { Rect::from_min_max(pos2(full.left() + 1.0, header.bottom()), pos2(x - 2.0, full.bottom())) };
    if content.width() < 8.0 || content.height() < 8.0 {
        return;
    }
    let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(content.shrink2(vec2(6.0, 4.0))).layout(egui::Layout::top_down(egui::Align::Min)));
    cui.set_clip_rect(content.intersect(ui.clip_rect()));
    egui::ScrollArea::vertical().id_salt(("dk_scroll", active)).auto_shrink([false, false]).show(&mut cui, |ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        docker_content(app, ui, active);
    });
}

/// The + under the tabs: a menu of every docker, opening the one chosen in this group.
fn plus_button(app: &mut AnyVectorApp, ui: &mut Ui, p: &egui::Painter, x: f32, y: f32, key: DockerId) {
    let plus = Rect::from_center_size(pos2(x + (TAB_STRIP - 4.0) / 2.0, y + 14.0), vec2(22.0, 22.0));
    let pr = ui.interact(plus, Id::unique(("dk_plus", key)), Sense::click()).on_hover_text("Add Docker");
    if pr.hovered() {
        p.rect_filled(plus, 2.0, theme::HOVER());
    }
    p.line_segment([plus.center() - vec2(6.0, 0.0), plus.center() + vec2(6.0, 0.0)], Stroke::new(1.6, theme::TEXT()));
    p.line_segment([plus.center() - vec2(0.0, 6.0), plus.center() + vec2(0.0, 6.0)], Stroke::new(1.6, theme::TEXT()));
    egui::Popup::menu(&pr).show(|ui| {
        ui.set_min_width(200.0);
        for &d in ALL.iter().filter(|d| **d != DockerId::Learn) {
            let on = app.dockers.open.contains(&d);
            if ui.add(egui::Button::new(title(d)).selected(on)).clicked() {
                app.dockers.live.acts.push(Act::OpenWith(d, key));
                ui.close();
            }
        }
    });
}

/// A docker tab: its icon at the top and its name written downward, rounded on the side away
/// from the docker.
#[allow(clippy::too_many_arguments)]
fn tab(app: &mut AnyVectorApp, ui: &mut Ui, p: &egui::Painter, x: f32, y: f32, d: DockerId, active: bool, side: DockerSide) -> (egui::Response, Rect) {
    let galley = ui.painter().layout_no_wrap(title(d), egui::FontId::proportional(12.0), theme::TEXT());
    let h = 26.0 + galley.size().x + 8.0;
    let r = Rect::from_min_size(pos2(x, y), vec2(TAB_STRIP - 4.0, h));
    let resp = ui.interact(r, Id::unique(("dk_tab", d)), Sense::click_and_drag()).on_hover_text(title(d));
    let fill = if active {
        theme::TAB_ACTIVE()
    } else if resp.hovered() {
        theme::HOVER()
    } else {
        theme::BAR()
    };
    let round = match side {
        DockerSide::Right => egui::CornerRadius { nw: 0, sw: 0, ne: 4, se: 4 },
        DockerSide::Left => egui::CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 },
    };
    p.rect_filled(r, round, fill);
    if let Some(t) = app.icons.get(ui.ctx(), icon(d), 16.0, if active { Tone::Bright } else { Tone::Normal }) {
        p.image(t.id, Rect::from_center_size(pos2(r.center().x, r.top() + 13.0), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    // written downward: the text turns a quarter clockwise about its top-left corner
    let gh = galley.size().y;
    let at = pos2(r.center().x + gh / 2.0, r.top() + 26.0);
    p.add(egui::epaint::TextShape::new(at, galley, if active { theme::STRONG() } else { theme::TEXT() }).with_angle(std::f32::consts::FRAC_PI_2));
    (resp, r)
}

/// What a tab does: click shows it (or folds the side), middle-click closes it, double-click
/// floats or docks it, a drag moves it, right-click has its menu.
fn tab_input(app: &mut AnyVectorApp, resp: &egui::Response, d: DockerId) {
    if resp.drag_started() {
        app.dockers.live.drag = Some(DockDrag { what: Dragged::Tab(d) });
    }
    if resp.double_clicked() {
        app.dockers.live.acts.push(Act::ToggleFloat(d));
    } else if resp.clicked() {
        app.dockers.live.acts.push(Act::Click(d));
    }
    if resp.middle_clicked() {
        app.dockers.live.acts.push(Act::Close(d));
    }
    resp.context_menu(|ui| tab_menu(app, ui, d, false));
}

/// What a title bar does: a drag moves the group (a floating window follows the pointer),
/// double-click floats or docks the group, right-click has the group's menu.
fn head_input(app: &mut AnyVectorApp, resp: &egui::Response, key: DockerId, floating: bool) {
    if resp.drag_started() {
        app.dockers.live.drag = Some(DockDrag { what: if floating { Dragged::Window(key) } else { Dragged::Group(key) } });
    }
    if floating && resp.dragged() {
        let dv = resp.drag_delta();
        if let Some(g) = app.dockers.layout.group_of(key) {
            let r = &mut app.dockers.layout.groups[g].rect;
            r[0] += dv.x;
            r[1] += dv.y;
        }
    }
    if floating && resp.hovered() {
        resp.ctx.set_cursor_icon(CursorIcon::Move);
    }
    if resp.double_clicked() {
        app.dockers.live.acts.push(Act::ToggleFloatGroup(key));
    }
    resp.context_menu(|ui| tab_menu(app, ui, key, true));
}

/// The right-click menu of a tab (`group`: of a title bar, for the whole group).
fn tab_menu(app: &mut AnyVectorApp, ui: &mut Ui, d: DockerId, group: bool) {
    crate::menus::menu_style(ui);
    ui.set_min_width(200.0);
    let l = &app.dockers.layout;
    let Some(g) = l.group_of(d) else { return };
    let grp = l.groups[g].clone();
    let shown = grp.active.unwrap_or(d);
    let mut act = None;
    for (text, side) in [("Dock &Left", DockerSide::Left), ("Dock Ri&ght", DockerSide::Right)] {
        if crate::menus::item(app, ui, text, !grp.on(side), false) {
            act = Some(if group { Act::DockGroup(d, side) } else { Act::Dock(d, side) });
        }
    }
    if crate::menus::item(app, ui, "&Float", !grp.floating, false) {
        act = Some(if group { Act::FloatGroup(d) } else { Act::Float(d) });
    }
    crate::menus::separator(ui);
    if crate::menus::item(app, ui, "&Close", true, false) {
        act = Some(Act::Close(if group { shown } else { d }));
    }
    if crate::menus::item(app, ui, "Close &Others", grp.dockers.len() > 1, false) {
        act = Some(Act::CloseOthers(if group { shown } else { d }));
    }
    crate::menus::separator(ui);
    if crate::menus::item(app, ui, "&Reset Docker Layout", true, false) {
        act = Some(Act::Reset);
    }
    if let Some(a) = act {
        app.dockers.live.acts.push(a);
        ui.close();
    }
}

// ------------------------------------------------------------------------------------------
// Floating windows
// ------------------------------------------------------------------------------------------

/// The floating docker windows, over the drawing window. A window kept off screen (a monitor
/// that is gone) comes back into view.
pub fn floats(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let screen = ctx.content_rect();
    let keys: Vec<DockerId> = app.dockers.layout.groups.iter().filter(|g| g.floating).map(|g| g.dockers[0]).collect();
    for key in keys {
        let Some(g) = app.dockers.layout.group_of(key) else { continue };
        let r = clamp_float(app.dockers.layout.groups[g].rect, screen);
        app.dockers.layout.groups[g].rect = r;
        let rect = Rect::from_min_size(pos2(r[0], r[1]), vec2(r[2], r[3]));
        egui::Area::new(Id::unique(("dk_float", key))).order(Order::Middle).fixed_pos(rect.min).constrain(false).show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
            let p = ui.painter();
            p.add(egui::epaint::Shadow { offset: [2, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(110) }.as_shape(rect, 3.0));
            p.rect_filled(rect, 3.0, theme::BG());
            block(app, ui, key, rect.shrink(1.0), Spot { side: DockerSide::Right, floating: true, top: false });
            ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, theme::SEP()), egui::StrokeKind::Inside);
            resize_handles(app, ui, key, rect);
        });
    }
}

/// A floating window's rectangle kept in view: its title bar on screen, no bigger than it.
fn clamp_float(r: [f32; 4], screen: Rect) -> [f32; 4] {
    let w = r[2].clamp(MIN_FLOAT[0], screen.width().max(MIN_FLOAT[0]));
    let h = r[3].clamp(MIN_FLOAT[1], screen.height().max(MIN_FLOAT[1]));
    let x = r[0].clamp(screen.left() - w + 80.0, (screen.right() - 80.0).max(screen.left()));
    let y = r[1].clamp(screen.top(), (screen.bottom() - HEADER).max(screen.top()));
    [x, y, w, h]
}

/// The edges and lower corners of a floating window, dragged to size it.
fn resize_handles(app: &mut AnyVectorApp, ui: &mut Ui, key: DockerId, r: Rect) {
    let m = 5.0;
    // which of left, right, bottom each one moves
    let zones = [
        (Rect::from_min_max(pos2(r.left(), r.top() + HEADER), pos2(r.left() + m, r.bottom() - m)), [true, false, false], CursorIcon::ResizeHorizontal),
        (Rect::from_min_max(pos2(r.right() - m, r.top() + HEADER), pos2(r.right(), r.bottom() - m)), [false, true, false], CursorIcon::ResizeHorizontal),
        (Rect::from_min_max(pos2(r.left() + m, r.bottom() - m), pos2(r.right() - m, r.bottom())), [false, false, true], CursorIcon::ResizeVertical),
        (Rect::from_min_max(pos2(r.left(), r.bottom() - m), pos2(r.left() + m, r.bottom())), [true, false, true], CursorIcon::ResizeNeSw),
        (Rect::from_min_max(pos2(r.right() - m, r.bottom() - m), r.max), [false, true, true], CursorIcon::ResizeNwSe),
    ];
    for (i, (zr, [left, right, bottom], cursor)) in zones.into_iter().enumerate() {
        let resp = ui.interact(zr, Id::unique(("dk_float_size", key, i)), Sense::drag()).on_hover_cursor(cursor);
        if resp.dragged() {
            let dv = resp.drag_delta();
            if let Some(g) = app.dockers.layout.group_of(key) {
                let f = &mut app.dockers.layout.groups[g].rect;
                if left {
                    let dx = dv.x.min(f[2] - MIN_FLOAT[0]);
                    f[0] += dx;
                    f[2] -= dx;
                }
                if right {
                    f[2] = (f[2] + dv.x).max(MIN_FLOAT[0]);
                }
                if bottom {
                    f[3] = (f[3] + dv.y).max(MIN_FLOAT[1]);
                }
            }
        }
        if resp.drag_stopped() {
            commit(app);
        }
    }
}

// ------------------------------------------------------------------------------------------
// Dropping
// ------------------------------------------------------------------------------------------

/// What a drop does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Drop {
    To(Target),
    /// A floating window let go away from a docking place: it stays where it was moved.
    Stay,
    Nothing,
}

/// What a drop at `p` would do, the area it fills and the line that shows a place among tabs.
/// `top` is the layer under the pointer; `size` the size of a window the drop floats.
pub(crate) fn drop_at(hits: &[Hit], central: Rect, p: Pos2, what: Dragged, layout: &super::DockerLayout, top: Option<LayerId>, size: egui::Vec2) -> (Drop, Option<Rect>, Option<[Pos2; 2]>) {
    let src = layout.group_of(what.docker());
    let whole = !matches!(what, Dragged::Tab(_));
    // a docked group let go on itself stays
    if matches!(what, Dragged::Group(_)) && hits.iter().any(|h| Some(h.group) == src && h.block.contains(p)) {
        return (Drop::Nothing, None, None);
    }
    let hits: Vec<&Hit> = hits.iter().filter(|h| h.block.contains(p) && !(whole && Some(h.group) == src)).collect();
    // the group on top: a floating window over the docker areas
    let hit = hits.iter().find(|h| Some(h.layer) == top).or_else(|| hits.iter().rev().find(|h| h.floating)).or_else(|| hits.first()).copied();
    let tab_at = |h: &Hit, at: usize| match what {
        Dragged::Tab(d) if Some(h.group) == src => h.tabs.iter().position(|(t, _)| *t == d).is_some_and(|i| at == i || at == i + 1),
        _ => false,
    };
    if let Some(h) = hit {
        if h.folded || h.head.contains(p) || h.strip.contains(p) {
            let at = h.tabs.iter().filter(|(_, r)| r.center().y < p.y).count();
            if tab_at(h, at) {
                return (Drop::Nothing, None, None);
            }
            let (x0, x1) = h.tabs.first().map(|(_, r)| (r.left(), r.right())).unwrap_or((h.strip.left(), h.strip.right()));
            let y = match h.tabs.get(at) {
                Some((_, r)) => r.top() - 1.0,
                None => h.tabs.last().map(|(_, r)| r.bottom() + 1.0).unwrap_or(h.strip.top() + 4.0),
            };
            return (Drop::To(Target::Join { group: h.group, at }), Some(h.block), Some([pos2(x0, y), pos2(x1, y)]));
        }
        let alone = matches!(what, Dragged::Tab(_)) && Some(h.group) == src && h.tabs.len() == 1;
        if !h.floating {
            let t = (p.y - h.block.top()) / h.block.height().max(1.0);
            let half = h.block.height() / 2.0;
            if t < 0.25 && !alone {
                return (Drop::To(Target::Stack { group: h.group, after: false }), Some(Rect::from_min_size(h.block.min, vec2(h.block.width(), half))), None);
            }
            if t > 0.75 && !alone {
                return (Drop::To(Target::Stack { group: h.group, after: true }), Some(Rect::from_min_max(pos2(h.block.left(), h.block.bottom() - half), h.block.max)), None);
            }
        }
        if matches!(what, Dragged::Window(_)) {
            return (Drop::Stay, None, None);
        }
        let at = h.tabs.len();
        if tab_at(h, at) || alone {
            return (Drop::Nothing, None, None);
        }
        return (Drop::To(Target::Join { group: h.group, at }), Some(h.block), None);
    }
    // by an edge of the drawing window: a new group on that side
    if central.contains(p) && central.width() > 2.0 * EDGE {
        let side = if p.x < central.left() + EDGE {
            Some(DockerSide::Left)
        } else if p.x > central.right() - EDGE {
            Some(DockerSide::Right)
        } else {
            None
        };
        if let Some(side) = side {
            let w = layout.width(side).min(central.width() / 2.0);
            let area = match side {
                DockerSide::Left => Rect::from_min_size(central.min, vec2(w, central.height())),
                DockerSide::Right => Rect::from_min_max(pos2(central.right() - w, central.top()), central.max),
            };
            return (Drop::To(Target::Side(side)), Some(area), None);
        }
    }
    if matches!(what, Dragged::Window(_)) {
        return (Drop::Stay, None, None);
    }
    // anywhere else: a floating window, its title bar under the pointer
    let r = Rect::from_min_size(p - vec2(40.0, 12.0), size);
    (Drop::To(Target::Float([r.left(), r.top(), r.width(), r.height()])), Some(r), None)
}

/// After everything is drawn: the clicks of the frame are carried out; a tab or a group being
/// dragged shows where it would go, and goes there when the button is let go.
pub fn end_frame(app: &mut AnyVectorApp, ctx: &egui::Context, central: Rect) {
    let mut hits = std::mem::take(&mut app.dockers.live.hits);
    app.dockers.live.central = central;
    let acts = std::mem::take(&mut app.dockers.live.acts);
    if !acts.is_empty() {
        let before = (app.dockers.layout.clone(), app.dockers.open.clone());
        for a in acts {
            app.dockers.act(a);
        }
        if (app.dockers.layout.clone(), app.dockers.open.clone()) != before {
            commit(app);
        }
    }
    // the groups as the layout has them now
    hits.retain_mut(|h| match app.dockers.layout.group_of(h.key) {
        Some(g) => {
            h.group = g;
            true
        }
        None => false,
    });
    app.dockers.live.drawn = hits.clone();
    let Some(drag) = app.dockers.live.drag else { return };
    let (pos, down, esc) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.primary_down(), i.key_pressed(egui::Key::Escape)));
    let key = drag.what.docker();
    let Some(src) = app.dockers.layout.group_of(key) else {
        app.dockers.live.drag = None;
        return;
    };
    if esc {
        app.dockers.live.drag = None;
        if matches!(drag.what, Dragged::Window(_)) {
            commit(app);
        }
        return;
    }
    let Some(pos) = pos else { return };
    let size = {
        let g = &app.dockers.layout.groups[src];
        if g.floating {
            vec2(g.rect[2], g.rect[3])
        } else {
            let h = hits.iter().find(|h| h.group == src && !h.folded).map(|h| h.block.height()).unwrap_or(420.0);
            vec2(app.dockers.layout.width(g.side), h.clamp(MIN_FLOAT[1], 480.0))
        }
    };
    let (drop, area, line) = drop_at(&hits, central, pos, drag.what, &app.dockers.layout, ctx.layer_id_at(pos), size);
    if !down {
        app.dockers.live.drag = None;
        app.dockers.drop(drag.what, drop);
        commit(app);
        ctx.request_repaint();
        return;
    }
    let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::unique("dk_drop")));
    if let Some(a) = area {
        painter.rect_filled(a, 2.0, theme::FOCUS().gamma_multiply(if line.is_some() { 0.08 } else { 0.18 }));
        painter.rect_stroke(a, 2.0, Stroke::new(2.0, theme::FOCUS()), egui::StrokeKind::Inside);
    }
    if let Some(l) = line {
        painter.line_segment(l, Stroke::new(2.0, theme::FOCUS()));
    }
    if !matches!(drag.what, Dragged::Window(_)) {
        // the docker at the pointer
        let shown = app.dockers.layout.groups[src].active.filter(|_| matches!(drag.what, Dragged::Group(_))).unwrap_or(key);
        let galley = painter.layout_no_wrap(title(shown), egui::FontId::proportional(12.5), theme::TEXT());
        let ghost = Rect::from_min_size(pos + vec2(12.0, 14.0), vec2(galley.size().x + 34.0, 24.0));
        painter.rect_filled(ghost, 2.0, theme::BUTTON().gamma_multiply(0.92));
        painter.rect_stroke(ghost, 2.0, Stroke::new(1.0, theme::FOCUS()), egui::StrokeKind::Inside);
        if let Some(t) = app.icons.get(ctx, icon(shown), 16.0, Tone::Normal) {
            painter.image(t.id, Rect::from_center_size(ghost.left_center() + vec2(13.0, 0.0), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        painter.galley(pos2(ghost.left() + 26.0, ghost.center().y - galley.size().y / 2.0), galley, theme::TEXT());
    }
    ctx.set_cursor_icon(if drop == Drop::Nothing { CursorIcon::NoDrop } else { CursorIcon::Grabbing });
    ctx.request_repaint();
}

impl DockerState {
    /// A window for a docker floated from a menu: at the right of the drawing window.
    fn float_rect(&self, d: DockerId) -> [f32; 4] {
        let c = self.live.central;
        let h = self.live.drawn.iter().find(|h| h.tabs.iter().any(|(t, _)| *t == d) && !h.folded).map(|h| h.block.height()).unwrap_or(420.0).clamp(MIN_FLOAT[1], 480.0);
        let w = 300.0;
        if c.width() > w + 60.0 {
            [c.right() - w - 30.0, c.top() + 40.0, w, h.min(c.height() - 60.0).max(MIN_FLOAT[1])]
        } else {
            [240.0, 160.0, w, h]
        }
    }

    /// Carries out a drop.
    pub(crate) fn drop(&mut self, what: Dragged, drop: Drop) {
        let Drop::To(t) = drop else { return };
        match what {
            Dragged::Tab(d) => {
                self.layout.place(&[d], t);
                self.bring_forward(d);
            }
            Dragged::Group(d) | Dragged::Window(d) => {
                if let Some(g) = self.layout.group_of(d) {
                    self.layout.place_group(g, t);
                }
                if let Some(a) = self.layout.group_of(d).and_then(|g| self.layout.groups[g].active) {
                    self.bring_forward(a);
                }
            }
        }
    }

    /// Carries out a click on a tab, a title bar or their menus.
    pub(crate) fn act(&mut self, a: Act) {
        self.sync();
        let group = |s: &DockerState, d: DockerId| s.layout.group_of(d);
        match a {
            Act::Click(d) => {
                let Some(g) = group(self, d) else { return };
                let grp = &self.layout.groups[g];
                if !grp.floating && grp.active == Some(d) && !self.side_collapsed(grp.side) {
                    self.set_collapsed(grp.side, true);
                } else {
                    self.bring_forward(d);
                }
            }
            Act::Fold(side, on) => self.set_collapsed(side, on),
            Act::Close(d) => self.close(d),
            Act::CloseOthers(d) => {
                let Some(g) = group(self, d) else { return };
                for o in self.layout.groups[g].dockers.clone() {
                    if o != d {
                        self.close(o);
                    }
                }
                self.bring_forward(d);
            }
            Act::OpenWith(d, key) => {
                if !self.layout.contains(d) {
                    if let Some(g) = group(self, key) {
                        let at = self.layout.groups[g].dockers.len();
                        self.layout.place(&[d], Target::Join { group: g, at });
                    }
                }
                self.show(d);
            }
            Act::Dock(d, side) => {
                self.layout.dock(d, side);
                self.bring_forward(d);
            }
            Act::DockGroup(d, side) => {
                if let Some(g) = group(self, d) {
                    let shown = self.layout.groups[g].active;
                    self.layout.dock_group(g, side);
                    if let Some(s) = shown {
                        self.bring_forward(s);
                    }
                }
            }
            Act::Float(d) => {
                let r = self.float_rect(d);
                self.layout.place(&[d], Target::Float(r));
                self.bring_forward(d);
            }
            Act::FloatGroup(d) => {
                if let Some(g) = group(self, d) {
                    let r = self.float_rect(d);
                    self.layout.place_group(g, Target::Float(r));
                }
            }
            Act::ToggleFloat(d) => {
                let Some(g) = group(self, d) else { return };
                let (floating, side) = (self.layout.groups[g].floating, self.layout.groups[g].side);
                if floating {
                    self.layout.dock(d, side);
                    self.bring_forward(d);
                } else {
                    // the first click of the two folded the side when the tab was the one shown
                    self.set_collapsed(side, false);
                    self.act(Act::Float(d));
                }
            }
            Act::ToggleFloatGroup(d) => {
                let Some(g) = group(self, d) else { return };
                let (floating, side) = (self.layout.groups[g].floating, self.layout.groups[g].side);
                self.act(if floating { Act::DockGroup(d, side) } else { Act::FloatGroup(d) });
            }
            Act::Reset => self.reset_layout(),
        }
        self.sync();
    }

    /// Window ▸ Dockers ▸ Reset Docker Layout: the open dockers in one group on the right, as
    /// before dockers could be rearranged.
    pub fn reset_layout(&mut self) {
        self.sync();
        self.layout.reset(self.active);
        self.collapsed = false;
        self.collapsed_left = false;
        self.live.drag = None;
        self.sync();
    }
}

/// Where docker `d`'s tab was drawn last frame.
#[cfg(test)]
pub(crate) fn drawn_tab(app: &AnyVectorApp, d: DockerId) -> Option<Rect> {
    app.dockers.live.drawn.iter().flat_map(|h| h.tabs.iter()).find(|(t, _)| *t == d).map(|(_, r)| *r)
}

/// Where the group of docker `d` was drawn last frame: the whole group and its title bar.
#[cfg(test)]
pub(crate) fn drawn_group(app: &AnyVectorApp, d: DockerId) -> Option<(Rect, Rect)> {
    app.dockers.live.drawn.iter().find(|h| h.tabs.iter().any(|(t, _)| *t == d)).map(|h| (h.block, h.head))
}

/// The drawing window last frame.
#[cfg(test)]
pub(crate) fn drawn_central(app: &AnyVectorApp) -> Rect {
    app.dockers.live.central
}

#[cfg(test)]
mod tests {
    use super::super::layout::DockerLayout;
    use super::*;
    use DockerId::*;

    fn hit(key: DockerId, group: usize, block: Rect, tabs: &[DockerId], floating: bool) -> Hit {
        let head = Rect::from_min_size(block.min, vec2(block.width(), HEADER));
        let x = block.right() - TAB_STRIP;
        let strip = Rect::from_min_max(pos2(x - 2.0, head.bottom()), pos2(block.right(), block.bottom()));
        let tabs = tabs.iter().enumerate().map(|(i, d)| (*d, Rect::from_min_size(pos2(x, head.bottom() + 4.0 + i as f32 * 90.0), vec2(24.0, 88.0)))).collect();
        Hit { key, group, floating, folded: false, layer: LayerId::background(), block, head, strip, tabs }
    }

    /// Settings saved by 0.6.0 (only the list of open dockers) open as one group on the right;
    /// settings saved now keep that list for 0.6.0 and add the arrangement.
    #[test]
    fn settings_from_and_for_0_6_0() {
        let old = r#"{"open_dockers":["Properties","Objects","NoSuchDocker"],"docker_width":333.0,"workspace":"Default"}"#;
        let s: crate::settings::Settings = serde_json::from_str(old).unwrap();
        let mut st = DockerState::from_settings(&s);
        assert_eq!(st.open, vec![Properties, Objects]);
        assert_eq!(st.layout.groups.len(), 1);
        assert!(st.layout.groups[0].on(DockerSide::Right));
        assert_eq!(st.active, Some(Objects));
        assert_eq!(st.layout.right_width, 333.0);
        // rearranged, saved and read back
        st.layout.dock(Objects, DockerSide::Left);
        st.layout.place(&[Pages], Target::Float([50.0, 60.0, 320.0, 400.0]));
        st.open = st.layout.dockers().collect();
        let mut s2 = crate::settings::Settings::default();
        st.save(&mut s2);
        let json = serde_json::to_string(&s2).unwrap();
        let back: crate::settings::Settings = serde_json::from_str(&json).unwrap();
        let st2 = DockerState::from_settings(&back);
        assert_eq!(st2.layout, st.layout);
        assert_eq!(st2.open, st.open);
        // what 0.6.0 reads: the open dockers and the width
        #[derive(serde::Deserialize)]
        struct Old {
            open_dockers: Vec<String>,
            docker_width: f32,
        }
        let o: Old = serde_json::from_str(&json).unwrap();
        assert_eq!(o.open_dockers, vec!["Properties", "Objects", "Pages"]);
        assert_eq!(o.docker_width, 333.0);
        // a layout that can't be read leaves the rest of the settings alone
        let bad = r#"{"open_dockers":["Pages"],"docker_layout":{"groups":"nonsense"},"workspace":"Lite"}"#;
        let s: crate::settings::Settings = serde_json::from_str(bad).unwrap();
        assert_eq!(s.workspace, "Lite");
        assert_eq!(DockerState::from_settings(&s).open, vec![Pages]);
    }

    /// Code that adds or takes dockers in `open` directly (as older code and tests do) opens and
    /// closes them in the layout; menus open a docker in the group of the one shown last.
    #[test]
    fn open_list_and_show_follow_the_layout() {
        let mut st = DockerState::from_settings(&crate::settings::Settings::default());
        st.show(Properties);
        st.show(Objects);
        assert_eq!(st.layout.groups.len(), 1);
        st.layout.dock(Objects, DockerSide::Left);
        st.active = Some(Objects);
        st.show(Pages);
        assert_eq!(st.side_of(Pages), Some(DockerSide::Left), "opened next to the docker shown last");
        st.open.retain(|d| *d != Objects);
        st.open.push(Color);
        st.sync();
        assert!(!st.layout.contains(Objects));
        assert!(st.layout.contains(Color));
        assert_eq!(st.open.iter().filter(|d| **d == Color).count(), 1);
        st.open.clear();
        st.show(Text);
        assert_eq!(st.open, vec![Text]);
        assert_eq!(st.layout.groups.len(), 1);
        assert!(st.layout.groups[0].on(DockerSide::Right));
        // toggle closes the docker shown last; close shows its neighbour
        st.show(Glyphs);
        st.show(Pages);
        st.act(Act::Click(Glyphs));
        st.toggle(Glyphs);
        assert!(!st.open.contains(&Glyphs));
        assert!(st.active.is_some());
        // clicking the docker shown folds its side, clicking again unfolds it
        let shown = st.layout.groups[0].active.unwrap();
        st.act(Act::Click(shown));
        assert!(st.collapsed);
        st.act(Act::Click(shown));
        assert!(!st.collapsed);
        // a docker opened folds the side open again
        st.collapsed = true;
        st.show(Color);
        assert!(!st.collapsed);
        // close others, then reset
        st.act(Act::CloseOthers(Color));
        assert_eq!(st.open, vec![Color]);
        st.act(Act::Float(Color));
        assert!(st.layout.groups[0].floating);
        st.act(Act::ToggleFloat(Color));
        assert!(st.layout.groups[0].on(DockerSide::Right));
        st.show(Text);
        st.act(Act::Dock(Text, DockerSide::Left));
        st.reset_layout();
        assert_eq!(st.layout.groups.len(), 1);
        assert_eq!(st.layout.groups[0].dockers, vec![Color, Text]);
    }

    #[test]
    fn drop_places() {
        let mut l = DockerLayout::one_group(vec![Properties, Objects, Pages], 300.0);
        l.place(&[Color], Target::Float([100.0, 100.0, 300.0, 300.0]));
        let block = Rect::from_min_max(pos2(1600.0, 100.0), pos2(1900.0, 1000.0));
        let fblock = Rect::from_min_size(pos2(100.0, 100.0), vec2(300.0, 300.0));
        let hits = vec![hit(Properties, 0, block, &[Properties, Objects, Pages], false), hit(Color, 1, fblock, &[Color], true)];
        let central = Rect::from_min_max(pos2(40.0, 100.0), pos2(1590.0, 1000.0));
        let size = vec2(300.0, 400.0);
        let at = |p: Pos2, what: Dragged| drop_at(&hits, central, p, what, &l, None, size).0;
        let tab = |i: usize| hits[0].tabs[i].1;
        // Pages above Properties: into the tabs at 0
        assert_eq!(at(tab(0).center_top() + vec2(0.0, 5.0), Dragged::Tab(Pages)), Drop::To(Target::Join { group: 0, at: 0 }));
        // Properties dropped on itself: nothing
        assert_eq!(at(tab(0).center(), Dragged::Tab(Properties)), Drop::Nothing);
        assert_eq!(at(tab(1).center_top() + vec2(0.0, 2.0), Dragged::Tab(Properties)), Drop::Nothing);
        // the top and bottom quarters of the group: stacked
        assert_eq!(at(pos2(1700.0, 150.0), Dragged::Tab(Pages)), Drop::To(Target::Stack { group: 0, after: false }));
        assert_eq!(at(pos2(1700.0, 950.0), Dragged::Tab(Pages)), Drop::To(Target::Stack { group: 0, after: true }));
        // its middle: at the end of its tabs
        assert_eq!(at(pos2(1700.0, 550.0), Dragged::Tab(Color)), Drop::To(Target::Join { group: 0, at: 3 }));
        // by the left edge of the drawing window: the left side
        assert_eq!(at(pos2(60.0, 500.0), Dragged::Tab(Objects)), Drop::To(Target::Side(DockerSide::Left)));
        assert_eq!(at(pos2(1570.0, 500.0), Dragged::Tab(Objects)), Drop::To(Target::Side(DockerSide::Right)));
        // the middle of the drawing window: a floating window there
        assert_eq!(at(pos2(800.0, 500.0), Dragged::Tab(Objects)), Drop::To(Target::Float([760.0, 488.0, 300.0, 400.0])));
        // over the floating window: into its tabs
        assert_eq!(at(pos2(200.0, 250.0), Dragged::Tab(Objects)), Drop::To(Target::Join { group: 1, at: 1 }));
        // the floating window moved over the middle of a group stays floating; by an edge it docks
        assert_eq!(at(pos2(1700.0, 550.0), Dragged::Window(Color)), Drop::Stay);
        assert_eq!(at(pos2(800.0, 500.0), Dragged::Window(Color)), Drop::Stay);
        assert_eq!(at(pos2(200.0, 250.0), Dragged::Window(Color)), Drop::Stay, "a window doesn't dock into itself");
        assert_eq!(at(pos2(1700.0, 115.0), Dragged::Window(Color)), Drop::To(Target::Join { group: 0, at: 0 }));
        assert_eq!(at(pos2(60.0, 500.0), Dragged::Window(Color)), Drop::To(Target::Side(DockerSide::Left)));
        // a docked group dragged off: floats
        assert!(matches!(at(pos2(800.0, 500.0), Dragged::Group(Objects)), Drop::To(Target::Float(_))));
    }
}
