//! The search box at the top of the Tools ▸ Options page list: typing lists the pages whose
//! settings match (in every category, not only the one shown), choosing one opens the page, scrolls
//! to the first match and outlines every matching control. Esc clears the search.
//!
//! The labels are not listed by hand. The first time something is typed, every page (and every
//! tab of a tabbed page) is laid out once in an off-screen egui context while the text translator
//! records each caption the page shows, in the user interface language; the shapes of that pass
//! give where each caption sits, so a result can scroll its page to it.

use super::{page_rect, page_ui, pages, title_of, CATEGORIES};
use crate::app::AnyVectorApp;
use crate::commands::OptionsPage;
use crate::theme;
use egui::{pos2, vec2, Align2, Color32, FontId, Id, Rect, Sense, Shape, Stroke, Ui, UiBuilder};
use std::cell::RefCell;

/// One caption of a page.
#[derive(Clone, Debug)]
pub struct Entry {
    pub cat: OptionsPage,
    pub page: &'static str,
    pub tab: usize,
    pub text: String,
    /// Where the caption sits: its distance from the top of the scrolled area that holds it, and
    /// whether that area is the body of a tab (`None` when it was not drawn, e.g. far down a list).
    pub at: Option<(f32, bool)>,
}

#[derive(Default)]
pub struct SearchState {
    pub query: String,
    pub index: Option<Vec<Entry>>,
    /// The result just chosen: its page and where to scroll it.
    pub jump: Option<(OptionsPage, &'static str, f32, bool)>,
}

/// A page with matches: the tab of its first match and its matching captions.
pub struct Hit<'a> {
    pub cat: OptionsPage,
    pub page: &'static str,
    pub tab: usize,
    pub entries: Vec<&'a Entry>,
}

const JUMP_PAGE: &str = "options_search_jump_page";
const JUMP_TAB: &str = "options_search_jump_tab";
const TAB_TOP: &str = "options_search_tab_top";

fn terms(query: &str) -> Vec<String> {
    query.split_whitespace().map(|t| t.to_lowercase()).collect()
}

fn text_matches(text: &str, terms: &[String]) -> bool {
    if terms.is_empty() {
        return false;
    }
    let t = text.to_lowercase();
    terms.iter().all(|w| t.contains(w.as_str()))
}

/// For a list on a page: whether a line shows while the search is in progress (every line
/// when nothing is searched for).
pub fn filter(st: &SearchState) -> impl Fn(&str) -> bool {
    let terms = terms(&st.query);
    move |text: &str| terms.is_empty() || text_matches(text, &terms)
}

/// The pages with captions matching every word of `query`, in page-list order.
pub fn hits<'a>(index: &'a [Entry], query: &str) -> Vec<Hit<'a>> {
    let terms = terms(query);
    let mut out: Vec<Hit<'a>> = vec![];
    for e in index.iter().filter(|e| text_matches(&e.text, &terms)) {
        match out.iter_mut().find(|h| h.cat == e.cat && h.page == e.page) {
            Some(h) => h.entries.push(e),
            None => out.push(Hit { cat: e.cat, page: e.page, tab: e.tab, entries: vec![e] }),
        }
    }
    out
}

// ------------------------------------------------------------------------------------------
// index
// ------------------------------------------------------------------------------------------

thread_local! {
    static RECORDED: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// The user interface translator, also keeping every caption it is given (as shown) while an
/// index is being made on this thread. It is the translator installed in egui (`i18n::init`), so
/// laying out other windows at the same time cannot switch the recording off.
pub(crate) fn recording_translator(text: &str) -> Option<String> {
    let t = crate::i18n::translator(text);
    RECORDED.with(|r| {
        if let Some(v) = r.borrow_mut().as_mut() {
            v.push(t.clone().unwrap_or_else(|| text.to_string()));
        }
    });
    t
}

/// Whether a recorded string is a caption worth finding (not a number, a unit or a symbol).
fn is_caption(s: &str) -> bool {
    s.chars().filter(|c| c.is_alphabetic()).count() >= 2
}

fn collect_texts(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| collect_texts(s, out)),
        Shape::Text(t) => out.push((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
        _ => {}
    }
}

/// Lay out every page of every category once, off screen, and keep its captions. `panel` is
/// the page panel of the dialog (its size decides where the captions fall).
pub fn build_index(app: &mut AnyVectorApp, style: std::sync::Arc<egui::Style>, panel: Rect) -> Vec<Entry> {
    let off = egui::Context::default();
    off.set_global_style(style);
    // textures belong to the context that made them: the off-screen pass gets its own icons
    let icons = std::mem::take(&mut app.icons);
    let saved = (app.dialogs.options.tab, app.dialogs.options.doc_edit.clone());
    // lists narrowed by the search show everything while the index is made
    let query = std::mem::take(&mut app.dialogs.options.search.query);
    egui::set_text_translator(Some(recording_translator));
    let mut index = vec![];
    for (cat, _, _) in CATEGORIES {
        if cat == OptionsPage::Document && app.dialogs.options.doc_edit.is_none() {
            continue;
        }
        for (page, _) in pages(cat) {
            let inner = page_rect(panel, cat, page);
            let size = vec2(inner.width(), 30_000.0);
            let mut seen: Vec<String> = vec![];
            let mut previous: Option<Vec<String>> = None;
            for tab in 0..8 {
                app.dialogs.options.tab = tab;
                RECORDED.with(|r| *r.borrow_mut() = Some(vec![]));
                off.data_mut(|d| d.remove::<f32>(Id::unique(TAB_TOP)));
                let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size + vec2(40.0, 40.0))), ..Default::default() };
                let mut tab_top = None;
                let mut out = off.run_ui(input, |ui| {
                    let mut child = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(pos2(0.0, 0.0), size)).layout(egui::Layout::top_down(egui::Align::Min)));
                    child.spacing_mut().item_spacing.y = 7.0;
                    page_ui(app, &mut child, cat, page);
                    tab_top = ui.ctx().data(|d| d.get_temp::<f32>(Id::unique(TAB_TOP)));
                });
                // nothing is painted: the font atlas and icon uploads are dropped
                out.textures_delta.clear();
                let texts: Vec<String> = RECORDED.with(|r| r.borrow_mut().take()).unwrap_or_default().into_iter().filter(|s| is_caption(s)).collect();
                // the page clamped the tab (no such tab), or shows the same as the last tab: done
                if app.dialogs.options.tab != tab || previous.as_ref() == Some(&texts) {
                    break;
                }
                let mut placed = vec![];
                for s in &out.shapes {
                    collect_texts(&s.shape, &mut placed);
                }
                for t in &texts {
                    if seen.contains(t) {
                        continue;
                    }
                    seen.push(t.clone());
                    let at = placed.iter().find(|(s, _)| s == t).map(|(_, r)| match tab_top {
                        Some(top) if r.top() >= top => (r.top() - top, true),
                        _ => (r.top(), false),
                    });
                    index.push(Entry { cat, page, tab, text: t.clone(), at });
                }
                previous = Some(texts);
            }
        }
    }
    RECORDED.with(|r| *r.borrow_mut() = None);
    (app.dialogs.options.tab, app.dialogs.options.doc_edit) = saved;
    app.dialogs.options.search.query = query;
    app.icons = icons;
    index
}

// ------------------------------------------------------------------------------------------
// dialog parts
// ------------------------------------------------------------------------------------------

/// Esc clears a search instead of closing the dialog.
pub fn escape(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let st = &mut app.dialogs.options.search;
    if !st.query.is_empty() && !egui::Popup::is_any_open(ctx) && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        st.query.clear();
    }
}

/// The search box over the page list; true while there is something to search for.
pub fn search_box(app: &mut AnyVectorApp, ui: &mut Ui, rect: Rect, panel: Rect) -> bool {
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.add(egui::TextEdit::singleline(&mut app.dialogs.options.search.query).hint_text("Search...").desired_width(rect.width()));
    let active = !app.dialogs.options.search.query.trim().is_empty();
    if active && app.dialogs.options.search.index.is_none() {
        let style = ui.ctx().global_style();
        app.dialogs.options.search.index = Some(build_index(app, style, panel));
    }
    active
}

/// The pages with matches, in place of the page list. Choosing one opens it at its first match.
pub fn results(app: &mut AnyVectorApp, ui: &mut Ui, list: Rect, cat: OptionsPage, page: &str) {
    ui.painter().rect_stroke(list, 0.0, Stroke::new(1.0, theme::SEP()), egui::StrokeKind::Inside);
    let st = &app.dialogs.options.search;
    let index = st.index.as_deref().unwrap_or_default();
    let found = hits(index, &st.query);
    if found.is_empty() {
        ui.painter().text(pos2(list.left() + 8.0, list.top() + 14.0), Align2::LEFT_CENTER, "No results found.", FontId::proportional(13.0), theme::TEXT_DIM());
        return;
    }
    let mut chosen: Option<(OptionsPage, &'static str, usize, Option<(f32, bool)>)> = None;
    let mut child = ui.new_child(UiBuilder::new().max_rect(list.shrink(1.0)).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(list.shrink(1.0));
    egui::ScrollArea::vertical().id_salt("options_search_results").auto_shrink([false, false]).show(&mut child, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let w = ui.available_width();
        let mut last_cat = None;
        for h in &found {
            if last_cat != Some(h.cat) {
                last_cat = Some(h.cat);
                let (r, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
                ui.painter().text(pos2(r.left() + 3.0, r.center().y), Align2::LEFT_CENTER, title_of(h.cat), theme::bold(12.0), theme::TEXT_DIM());
            }
            let (row, resp) = ui.allocate_exact_size(vec2(w, 25.0), Sense::click());
            if h.cat == cat && h.page == page {
                ui.painter().rect_filled(row, 0.0, theme::HOVER());
            } else if resp.hovered() {
                ui.painter().rect_filled(row, 0.0, theme::gray(0x31));
            }
            ui.painter().text(pos2(row.left() + 15.0, row.center().y), Align2::LEFT_CENTER, h.page, FontId::proportional(13.0), theme::TEXT());
            let shown: Vec<&str> = h.entries.iter().take(8).map(|e| e.text.as_str()).collect();
            let resp = resp.on_hover_text(shown.join("\n"));
            if resp.clicked() {
                chosen = Some((h.cat, h.page, h.tab, h.entries[0].at));
            }
        }
    });
    if let Some((c, p, tab, at)) = chosen {
        let o = &mut app.dialogs.options;
        o.category = Some(c);
        o.page = Some(p.to_string());
        o.tab = tab;
        o.search.jump = at.map(|(y, in_tab)| (c, p, y, in_tab));
    }
}

/// Once the chosen page is shown, hand its scroll position to the scrolled area holding the match
/// (read by [`take_jump`]).
pub fn arm_jump(app: &mut AnyVectorApp, ui: &Ui, cat: OptionsPage, page: &str) {
    // whatever the last frame did not use (the match was not in a scrolled area)
    ui.ctx().data_mut(|d| {
        d.remove::<f32>(Id::unique(JUMP_PAGE));
        d.remove::<f32>(Id::unique(JUMP_TAB));
    });
    let st = &mut app.dialogs.options.search;
    if st.jump.is_some_and(|j| j.0 == cat && j.1 == page) {
        let (_, _, y, in_tab) = st.jump.take().unwrap_or((cat, "", 0.0, false));
        ui.ctx().data_mut(|d| d.insert_temp(Id::unique(if in_tab { JUMP_TAB } else { JUMP_PAGE }), y));
    }
}

/// The scroll offset that brings a chosen result into view, once (`tab_body` is true for the
/// scrolled body of a tab).
pub fn take_jump(ui: &Ui, tab_body: bool) -> Option<f32> {
    let id = Id::unique(if tab_body { JUMP_TAB } else { JUMP_PAGE });
    ui.ctx().data_mut(|d| d.remove_temp::<f32>(id)).map(|y| (y - 60.0).max(0.0))
}

/// Remember where the body of a tab starts (read by the off-screen pass of [`build_index`]).
pub fn mark_tab_top(ui: &Ui, top: f32) {
    ui.ctx().data_mut(|d| d.insert_temp(Id::unique(TAB_TOP), top));
}

/// Outline the captions in `area` that match the search.
pub fn highlight(app: &AnyVectorApp, ui: &Ui, area: Rect) {
    let terms = terms(&app.dialogs.options.search.query);
    if terms.is_empty() {
        return;
    }
    let layer = ui.layer_id();
    let found: Vec<Rect> = ui.ctx().graphics(|g| {
        let mut v = vec![];
        if let Some(list) = g.get(layer) {
            for cs in list.all_entries() {
                let mut texts = vec![];
                collect_texts(&cs.shape, &mut texts);
                for (t, r) in texts {
                    let vis = r.intersect(cs.clip_rect).intersect(area);
                    if vis.is_positive() && text_matches(&t, &terms) {
                        v.push(vis);
                    }
                }
            }
        }
        v
    });
    let p = ui.painter().with_clip_rect(area);
    let accent = theme::ACCENT();
    for r in found {
        p.rect(r.expand(2.0), 2.0, Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 48), Stroke::new(1.5, accent), egui::StrokeKind::Outside);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(cat: OptionsPage, page: &'static str, tab: usize, text: &str) -> Entry {
        Entry { cat, page, tab, text: text.into(), at: None }
    }

    #[test]
    fn hits_group_by_page_and_need_every_word() {
        let index = vec![
            entry(OptionsPage::Document, "Grid", 0, "Show grid"),
            entry(OptionsPage::Document, "Grid", 0, "Snap to grid"),
            entry(OptionsPage::App, "Text", 2, "Show font sample"),
            entry(OptionsPage::App, "Snapping", 0, "Snap to objects"),
            entry(OptionsPage::Global, "Printing", 1, "Driver compatibility"),
        ];
        let h = hits(&index, "grid");
        assert_eq!(h.len(), 1);
        assert_eq!((h[0].cat, h[0].page, h[0].entries.len()), (OptionsPage::Document, "Grid", 2));
        let h = hits(&index, "SNAP");
        assert_eq!(h.iter().map(|h| h.page).collect::<Vec<_>>(), ["Grid", "Snapping"]);
        let h = hits(&index, "show  sample");
        assert_eq!((h.len(), h[0].page, h[0].tab), (1, "Text", 2));
        assert!(hits(&index, "show nothing").is_empty());
        assert!(hits(&index, "   ").is_empty());
    }

    /// The index made by laying the real pages out off screen (no GPU needed).
    #[test]
    fn index_finds_labels_on_the_real_pages() {
        let mut h = egui_kittest::Harness::builder().with_size(vec2(1600.0, 900.0)).build_eframe(|cc| {
            let mut app = AnyVectorApp::new(cc);
            app.splash = None;
            app.pending.clear();
            app
        });
        h.state_mut().new_default_document();
        h.state_mut().dialogs.new_doc = None;
        h.step();
        let app = h.state_mut();
        app.dialogs.options.doc_edit = super::super::DocEdit::from_app(app);
        app.dialogs.options.search.query = "kept".into();
        // the page panel of the 806 × 681 dialog
        let panel = Rect::from_min_size(pos2(200.0, 80.0), vec2(598.0, 566.0));
        let index = build_index(app, std::sync::Arc::new(egui::Style::default()), panel);
        assert_eq!(app.dialogs.options.search.query, "kept");
        assert_eq!(app.dialogs.options.tab, 0);
        // the captions are recorded as shown, in the user interface language
        let found = |label: &str| hits(&index, &crate::i18n::t(label)).iter().map(|h| (h.cat, h.page, h.tab)).collect::<Vec<_>>();
        assert!(found("Show grid").contains(&(OptionsPage::Document, "Grid", 0)));
        assert!(found("Undo levels").contains(&(OptionsPage::App, "General", 0)));
        assert!(found("Zoom rate:").contains(&(OptionsPage::App, "Display", 0)));
        assert!(found("Crosshair cursor").contains(&(OptionsPage::Tools, "Pick", 0)));
        assert!(found("Enable background tasks").contains(&(OptionsPage::Global, "General", 0)));
        // tabs other than the first, and a line far down a scrolled list
        assert!(found("Enable remote fonts").contains(&(OptionsPage::App, "Text", 2)));
        assert!(found("Perform automatic spell checking").contains(&(OptionsPage::App, "Text", 3)));
        assert!(found("Print only current page").contains(&(OptionsPage::Global, "Printing", 0)));
        assert!(found("Welding objects with different outlines").contains(&(OptionsPage::App, "Warnings", 0)));
        // where a caption sits on a scrolled page, and inside a tab's body
        let at = |label: &str| index.iter().find(|e| e.text == crate::i18n::t(label)).and_then(|e| e.at);
        assert!(at("Alternate Zoom (Ctrl+Shift) rate:").is_some_and(|(y, tab)| y > 100.0 && !tab));
        assert!(at("Perform automatic spell checking").is_some_and(|(y, tab)| y >= 0.0 && tab));
        assert!(index.len() > 300, "{}", index.len());
    }
}
