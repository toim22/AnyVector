//! The Welcome Screen tab, laid out as in the reference: a column of green page links on the
//! left (Get Started, Workspace, What's New), the chosen page beside it, and round shortcuts
//! below a rule (Ideas and feedback, User Guide). The reference's training, store, trial and
//! support entries are Corel services and are left out.

use crate::app::VectorApp;
use crate::commands::Cmd;
use crate::icons::Tone;
use crate::theme;
use egui::{pos2, vec2, Align2, Color32, CursorIcon, FontId, Rect, Sense, Stroke, Ui};

/// The pages of the left column.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Page {
    #[default]
    GetStarted,
    Workspace,
    WhatsNew,
}

impl Page {
    const ALL: [Page; 3] = [Page::GetStarted, Page::Workspace, Page::WhatsNew];

    fn name(self) -> &'static str {
        match self {
            Page::GetStarted => "Get Started",
            Page::Workspace => "Workspace",
            Page::WhatsNew => "What's New",
        }
    }
}

/// The green of the page links.
const LINK: Color32 = Color32::from_rgb(0x8c, 0xc6, 0x3f);
/// Width of the whole block (links, page, shortcuts) and of the link column.
const BLOCK_W: f32 = 1280.0;
const NAV_W: f32 = 240.0;
/// Distance from the top of the area to the shortcuts rule.
const RULE_Y: f32 = 568.0;

/// What Vector adds in this version, shown on the What's New page.
pub const WHATS_NEW: &[&str] = &[
    "Menus, right-click menus, toolbars, the toolbox and every property bar follow the reference layout",
    "The Welcome Screen with Get Started, Workspace and What's New",
    "CDR drawing format (.cdr); the older .vct format still opens",
    "PDF, SVG and bitmap export",
    "PowerTRACE bitmap tracing",
    "Scripting with recording",
    "AI Generate",
];

pub fn welcome(app: &mut VectorApp, ui: &mut Ui) {
    let area = ui.available_rect_before_wrap();
    let bg = ui.interact(area, ui.scope_id().with("home_area"), Sense::click());
    let p = ui.painter().clone();

    let w = BLOCK_W.min(area.width() - 40.0).max(560.0);
    let block = Rect::from_min_size(pos2(area.center().x - w / 2.0, area.top()), vec2(w, area.height()));
    let rule_y = (area.top() + RULE_Y).min(area.bottom() - 150.0).max(area.top() + 360.0);

    // the link column and its divider
    let divider_x = block.left() + NAV_W;
    p.line_segment([pos2(divider_x, area.top() + 38.0), pos2(divider_x, rule_y - 146.0)], Stroke::new(1.0, theme::SEP));
    for (i, page) in Page::ALL.into_iter().enumerate() {
        let font = FontId::proportional(18.0);
        let g = p.layout_no_wrap(page.name().to_string(), font.clone(), LINK);
        let at = pos2(divider_x - 21.0, area.top() + 50.0 + i as f32 * 51.0);
        let r = Rect::from_min_size(pos2(at.x - g.size().x, at.y - g.size().y / 2.0), g.size());
        let resp = ui.interact(r.expand(3.0), ui.scope_id().with(("welcome_page", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let current = app.welcome_page == page;
        let color = if current { Color32::WHITE } else { LINK };
        p.text(r.left_center(), Align2::LEFT_CENTER, page.name(), font, color);
        if resp.hovered() {
            p.line_segment([r.left_bottom() + vec2(0.0, 1.0), r.right_bottom() + vec2(0.0, 1.0)], Stroke::new(1.0, color));
        }
        if resp.clicked() {
            app.welcome_page = page;
        }
    }

    // the page
    let page_rect = Rect::from_min_max(pos2(divider_x + 40.0, area.top() + 38.0), pos2(block.right(), rule_y - 30.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(page_rect));
    child.set_clip_rect(page_rect.intersect(ui.clip_rect()));
    match app.welcome_page {
        Page::GetStarted => get_started(app, &mut child),
        Page::Workspace => workspace(app, &mut child),
        Page::WhatsNew => whats_new(&mut child),
    }

    // the shortcuts
    p.line_segment([pos2(block.left(), rule_y), pos2(block.right(), rule_y)], Stroke::new(1.0, theme::SEP));
    let shortcuts: [(&str, &str, Cmd); 2] = [("feedback", "Ideas and feedback", Cmd::Feedback), ("book", "User Guide", Cmd::UserGuide)];
    let step = w / 5.0;
    let first = block.center().x - step * (shortcuts.len() as f32 - 1.0) / 2.0;
    for (i, (icon, label, cmd)) in shortcuts.into_iter().enumerate() {
        let c = pos2(first + i as f32 * step, rule_y + 61.0);
        let hit = Rect::from_center_size(c + vec2(0.0, 22.0), vec2(150.0, 120.0));
        let resp = ui.interact(hit, ui.scope_id().with(("welcome_link", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let fill = if resp.hovered() { Color32::from_gray(0xe0) } else { Color32::from_gray(0xc4) };
        p.circle_filled(c, 36.0, fill);
        if let Some(t) = app.icons.get(ui.ctx(), icon, 44.0, Tone::Dark) {
            p.image(t.id, Rect::from_center_size(c, vec2(44.0, 44.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        p.text(c + vec2(0.0, 57.0), Align2::CENTER_CENTER, label, theme::body(13.0), if resp.hovered() { theme::TEXT } else { theme::TEXT_DIM });
        if resp.clicked() {
            app.run(cmd);
        }
    }

    if bg.double_clicked() {
        app.run(Cmd::New);
    }
    // dropped-file hint while dragging files over the window
    if ui.ctx().input(|i| !i.raw.hovered_files.is_empty()) {
        p.rect_stroke(area.shrink(12.0), 6.0, Stroke::new(2.0, theme::FOCUS), egui::StrokeKind::Inside);
        p.text(area.center(), Align2::CENTER_CENTER, "Drop files to open them", theme::body(16.0), theme::TEXT);
    }
}

fn heading(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).font(theme::light(20.0)).color(theme::TEXT));
    ui.add_space(10.0);
}

/// A large button: an icon over a caption.
fn tile(app: &mut VectorApp, ui: &mut Ui, icon: &str, caption: &str, size: egui::Vec2) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    p.rect_filled(r, 4.0, if resp.hovered() { theme::HOVER } else { theme::BAR });
    p.rect_stroke(r, 4.0, Stroke::new(1.0, if resp.hovered() { theme::FOCUS } else { theme::SEP }), egui::StrokeKind::Inside);
    if let Some(t) = app.icons.get(ui.ctx(), icon, 40.0, Tone::Normal) {
        p.image(t.id, Rect::from_center_size(pos2(r.center().x, r.top() + 40.0), vec2(40.0, 40.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    p.text(pos2(r.center().x, r.bottom() - 22.0), Align2::CENTER_CENTER, caption, theme::body(13.0), theme::TEXT);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn get_started(app: &mut VectorApp, ui: &mut Ui) {
    heading(ui, "Get Started");
    let mut run = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        for (icon, caption, cmd) in [("new", "New Document", Cmd::New), ("new_template", "New from Template", Cmd::NewFromTemplate), ("open", "Open Document", Cmd::Open)] {
            if tile(app, ui, icon, caption, vec2(150.0, 108.0)).clicked() {
                run = Some(cmd);
            }
        }
    });
    ui.add_space(22.0);
    ui.label(egui::RichText::new("Recent Documents").font(theme::bold(13.0)).color(theme::TEXT));
    ui.add_space(4.0);
    let files = app.settings.recent_files.clone();
    if files.is_empty() {
        ui.label(egui::RichText::new("Documents you open or save are listed here.").color(theme::TEXT_DIM));
    }
    for (i, path) in files.iter().enumerate().take(8) {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(720.0), 40.0), Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(r, 3.0, theme::HOVER);
        }
        if let Some(t) = app.icons.get(ui.ctx(), "doc_props", 24.0, Tone::Normal) {
            p.image(t.id, Rect::from_center_size(pos2(r.left() + 20.0, r.center().y), vec2(24.0, 24.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let folder = path.parent().map(|d| d.display().to_string()).unwrap_or_default();
        p.text(pos2(r.left() + 42.0, r.top() + 12.0), Align2::LEFT_CENTER, name, theme::body(13.0), theme::TEXT);
        p.text(pos2(r.left() + 42.0, r.top() + 29.0), Align2::LEFT_CENTER, folder, theme::body(11.5), theme::TEXT_DIM);
        if let Some(when) = modified(path) {
            p.text(pos2(r.right() - 10.0, r.center().y), Align2::RIGHT_CENTER, when, theme::body(11.5), theme::TEXT_DIM);
        }
        if resp.on_hover_text(path.display().to_string()).clicked() {
            run = Some(Cmd::OpenRecent(i));
        }
    }
    if let Some(c) = run {
        app.run(c);
    }
}

/// When a file was last changed.
fn modified(path: &std::path::Path) -> Option<String> {
    Some(crate::settings::format_time(std::fs::metadata(path).ok()?.modified().ok()?))
}

fn workspace(app: &mut VectorApp, ui: &mut Ui) {
    heading(ui, "Workspace");
    ui.label(egui::RichText::new("Choose the layout of tools, toolbars and dockers that fits your work. You can change it later in Window ▸ Workspace.").color(theme::TEXT_DIM));
    ui.add_space(12.0);
    let mut pick = None;
    let cols = 3;
    let gap = 12.0;
    let tw = ((ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32).clamp(160.0, 300.0);
    for (row, chunk) in crate::dialogs::options::WORKSPACES.chunks(cols).enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (j, (name, desc)) in chunk.iter().enumerate() {
                let i = row * cols + j;
                let (r, resp) = ui.allocate_exact_size(vec2(tw, 92.0), Sense::click());
                let current = app.settings.workspace == *name;
                let p = ui.painter();
                p.rect_filled(r, 4.0, if resp.hovered() { theme::HOVER } else { theme::BAR });
                p.rect_stroke(r, 4.0, Stroke::new(if current { 2.0 } else { 1.0 }, if current || resp.hovered() { theme::FOCUS } else { theme::SEP }), egui::StrokeKind::Inside);
                p.text(r.left_top() + vec2(12.0, 12.0), Align2::LEFT_TOP, *name, theme::bold(14.0), theme::TEXT);
                if current {
                    p.text(r.right_top() + vec2(-12.0, 13.0), Align2::RIGHT_TOP, "Current", theme::body(11.5), LINK);
                }
                let g = p.layout(desc.to_string(), theme::body(12.0), theme::TEXT_DIM, tw - 24.0);
                p.galley(r.left_top() + vec2(12.0, 36.0), g, theme::TEXT_DIM);
                if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    pick = Some(i as u8);
                }
            }
        });
        ui.add_space(gap);
    }
    if let Some(i) = pick {
        app.run(Cmd::Workspace(i));
    }
}

fn whats_new(ui: &mut Ui) {
    heading(ui, &format!("What's New in Vector {}", vector_core::APP_VERSION));
    for line in WHATS_NEW {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("•").color(LINK));
            ui.label(egui::RichText::new(*line).color(theme::TEXT));
        });
        ui.add_space(4.0);
    }
}
