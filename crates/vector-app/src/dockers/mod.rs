//! Dockers (Window > Dockers): the right-hand docking panel with a tab per open docker.

mod arrange;
mod color;
mod effects;
mod misc;
mod objects;
pub mod props;
mod text;

use crate::app::VectorApp;
use crate::icons::Tone;
use crate::settings::Settings;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum DockerId {
    Properties,
    Objects,
    Pages,
    Comments,
    ObjectData,
    ObjectStyles,
    Symbols,
    FindReplace,
    Export,
    AiGenerate,
    Transform,
    Coordinates,
    LiveGuides,
    Guidelines,
    AlignDistribute,
    FitObjectsToPath,
    StepRepeat,
    Shape,
    Corners,
    JoinCurves,
    Text,
    FontSampler,
    Glyphs,
    Contour,
    Blend,
    Extrude,
    Bevel,
    Lens,
    Envelope,
    Distort,
    Shadow,
    BitmapEffects,
    Color,
    Palettes,
    ColorStyles,
    ColorProofing,
    Assets,
    Tray,
    LinksRollovers,
    Sources,
    Views,
    History,
    Scripts,
    Learn,
}

use DockerId::*;

pub const ALL: &[DockerId] = &[
    Properties, Objects, Pages, Comments, ObjectData, ObjectStyles, Symbols, FindReplace, Export, AiGenerate, Transform, Coordinates, LiveGuides, Guidelines, AlignDistribute, FitObjectsToPath, StepRepeat, Shape, Corners, JoinCurves, Text, FontSampler, Glyphs, Contour, Blend, Extrude, Bevel,
    Lens, Envelope, Distort, Shadow, BitmapEffects, Color, Palettes, ColorStyles, ColorProofing, Assets, Tray, LinksRollovers, Sources, Views, History, Scripts, Learn,
];

pub fn shortcut(d: DockerId) -> Option<&'static str> {
    Some(match d {
        Properties => "Alt+Enter",
        ObjectStyles => "Ctrl+F5",
        Symbols => "Ctrl+F3",
        FindReplace => "Ctrl+F",
        Transform => "Alt+F7",
        AlignDistribute => "Ctrl+Shift+A",
        StepRepeat => "Ctrl+Shift+D",
        Text => "Ctrl+T",
        Glyphs => "Ctrl+F11",
        ColorStyles => "Ctrl+F6",
        Views => "Ctrl+F2",
        Scripts => "Alt+Shift+F11",
        _ => return None,
    })
}

/// Menu label with `&` mnemonic.
pub fn menu_label(d: DockerId) -> &'static str {
    match d {
        Properties => "P&roperties",
        Objects => "&Objects",
        Pages => "&Pages",
        Comments => "Co&mments",
        ObjectData => "Object &Data",
        ObjectStyles => "Object St&yles",
        Symbols => "&Symbols",
        FindReplace => "&Find and Replace",
        Export => "E&xport",
        AiGenerate => "&AI Generate",
        Transform => "&Transform",
        Coordinates => "Coord&inates",
        LiveGuides => "&Live Guides",
        Guidelines => "&Guidelines",
        AlignDistribute => "Align and Dis&tribute",
        FitObjectsToPath => "Fit Objects to Pat&h",
        StepRepeat => "St&ep and Repeat",
        Shape => "S&hape",
        Corners => "Cor&ners",
        JoinCurves => "&Join Curves",
        Text => "Te&xt",
        FontSampler => "Font Sa&mpler",
        Glyphs => "&Glyphs",
        Contour => "&Contour",
        Blend => "&Blend",
        Extrude => "E&xtrude",
        Bevel => "Be&vel",
        Lens => "&Lens",
        Envelope => "&Envelope",
        Distort => "&Distort",
        Shadow => "&Shadow",
        BitmapEffects => "Bitmap E&ffects",
        Color => "&Color",
        Palettes => "&Palettes",
        ColorStyles => "Color St&yles",
        ColorProofing => "Color Pr&oofing",
        Assets => "&Assets",
        Tray => "T&ray",
        LinksRollovers => "&Links and Rollovers",
        Sources => "So&urces",
        Views => "&Views",
        History => "&History",
        Scripts => "Scr&ipts",
        Learn => "Lear&n",
    }
}

pub fn title(d: DockerId) -> String {
    crate::widgets::plain(menu_label(d))
}

pub fn icon(d: DockerId) -> &'static str {
    match d {
        Properties => "properties",
        Objects => "objects",
        Pages => "pages",
        Comments => "comments",
        ObjectData => "object_data",
        ObjectStyles => "object_styles",
        Symbols => "symbols",
        FindReplace => "find",
        Export => "export",
        AiGenerate => "ai",
        Transform => "transform",
        Coordinates => "coordinates",
        LiveGuides => "live_guides",
        Guidelines => "guidelines",
        AlignDistribute => "align",
        FitObjectsToPath => "fit_path",
        StepRepeat => "step_repeat",
        Shape => "weld",
        Corners => "corners",
        JoinCurves => "join_curves",
        Text => "text_docker",
        FontSampler => "font_sampler",
        Glyphs => "glyphs",
        Contour => "contour",
        Blend => "blend",
        Extrude => "extrude",
        Bevel => "bevel",
        Lens => "lens",
        Envelope => "envelope",
        Distort => "distort",
        Shadow => "drop_shadow",
        BitmapEffects => "bitmap",
        Color => "color",
        Palettes => "palettes",
        ColorStyles => "color_styles",
        ColorProofing => "color_proofing",
        Assets => "assets",
        Tray => "tray",
        LinksRollovers => "links",
        Sources => "sources",
        Views => "views",
        History => "history",
        Scripts => "script",
        Learn => "learn",
    }
}

pub struct DockerState {
    /// Open dockers in tab order.
    pub open: Vec<DockerId>,
    pub active: Option<DockerId>,
    /// Per-docker transient UI state.
    pub ui: DockerUi,
}

/// Transient values edited inside dockers (not saved with documents).
pub struct DockerUi {
    pub transform_tab: usize,
    pub t_pos: [f64; 2],
    pub t_relative: bool,
    pub t_rot: f64,
    pub t_scale: [f64; 2],
    pub t_proportional: bool,
    pub t_size: [f64; 2],
    pub t_skew: [f64; 2],
    pub t_anchor: usize,
    pub t_copies: u32,
    pub align_to: usize,
    pub use_outline: bool,
    pub step_copies: u32,
    pub step_offset: [f64; 2],
    pub find_text: String,
    pub replace_text: String,
    pub find_case: bool,
    pub glyph_font: String,
    pub glyph_filter: String,
    pub sampler_text: String,
    pub sampler_size: f32,
    pub guide_pos: f64,
    pub guide_vertical: bool,
    pub join_tolerance: f64,
    pub corner_radius: f64,
    pub corner_kind: usize,
    pub contour_steps: u32,
    pub contour_offset: f64,
    pub contour_dir: usize,
    pub blend_steps: u32,
    pub bevel_depth: f64,
    pub bevel_angle: f64,
    pub lens_kind: usize,
    pub lens_amount: f64,
    pub bitmap_effect: usize,
    pub bitmap_amount: f64,
    pub color_model: usize,
    pub color_hsv: [f32; 3],
    pub color_edit: Option<vector_core::Color>,
    pub hex: String,
    pub data_name: String,
    pub data_value: String,
    pub comment: String,
    pub view_name: String,
    pub style_name: String,
    pub export_format: usize,
    pub export_dpi: f64,
    pub script_text: String,
    pub script_error: Option<String>,
    pub link_url: String,
    pub fit_offset: f64,
    pub fit_rotate: bool,
    pub objects_filter: String,
    pub rename: Option<(vector_core::ObjId, String)>,
    pub layer_rename: Option<(usize, String)>,
    pub tray: Vec<vector_core::Object>,
    pub proof_gray: bool,
    pub sym_name: String,
}

impl Default for DockerUi {
    fn default() -> Self {
        DockerUi {
            transform_tab: 0,
            t_pos: [0.0, 0.0],
            t_relative: true,
            t_rot: 0.0,
            t_scale: [100.0, 100.0],
            t_proportional: true,
            t_size: [72.0, 72.0],
            t_skew: [0.0, 0.0],
            t_anchor: 4,
            t_copies: 0,
            align_to: 0,
            use_outline: false,
            step_copies: 3,
            step_offset: [18.0, 18.0],
            find_text: String::new(),
            replace_text: String::new(),
            find_case: false,
            glyph_font: vector_core::text::default_font_family(),
            glyph_filter: String::new(),
            sampler_text: "The quick brown fox jumps over the lazy dog".into(),
            sampler_size: 20.0,
            guide_pos: 72.0,
            guide_vertical: false,
            join_tolerance: 3.6,
            corner_radius: 9.0,
            corner_kind: 0,
            contour_steps: 3,
            contour_offset: 6.0,
            contour_dir: 2,
            blend_steps: 20,
            bevel_depth: 6.0,
            bevel_angle: 135.0,
            lens_kind: 0,
            lens_amount: 2.0,
            bitmap_effect: 0,
            bitmap_amount: 4.0,
            color_model: 0,
            color_hsv: [0.6, 0.7, 0.9],
            color_edit: None,
            hex: String::new(),
            data_name: String::new(),
            data_value: String::new(),
            comment: String::new(),
            view_name: String::new(),
            style_name: String::new(),
            export_format: 0,
            export_dpi: 300.0,
            script_text: String::new(),
            script_error: None,
            link_url: String::new(),
            fit_offset: 0.0,
            fit_rotate: true,
            objects_filter: String::new(),
            rename: None,
            layer_rename: None,
            tray: vec![],
            proof_gray: false,
            sym_name: String::new(),
        }
    }
}

impl DockerState {
    pub fn from_settings(s: &Settings) -> DockerState {
        let open: Vec<DockerId> = s.open_dockers.iter().filter_map(|n| ALL.iter().copied().find(|d| format!("{d:?}") == *n)).collect();
        DockerState { active: open.last().copied(), open, ui: DockerUi::default() }
    }
    pub fn visible(&self) -> bool {
        !self.open.is_empty()
    }
    pub fn toggle(&mut self, d: DockerId) {
        if self.active == Some(d) {
            self.close(d);
        } else {
            self.show(d);
        }
    }
    pub fn show(&mut self, d: DockerId) {
        if !self.open.contains(&d) {
            self.open.push(d);
        }
        self.active = Some(d);
    }
    pub fn close(&mut self, d: DockerId) {
        self.open.retain(|x| *x != d);
        if self.active == Some(d) {
            self.active = self.open.last().copied();
        }
    }
    pub fn save(&self, s: &mut Settings) {
        s.open_dockers = self.open.iter().map(|d| format!("{d:?}")).collect();
    }
}

pub fn dockers_panel(app: &mut VectorApp, ui: &mut Ui) {
    let full = ui.max_rect();
    let p = ui.painter().clone();
    // left border
    p.line_segment([full.left_top(), full.left_bottom()], Stroke::new(1.0, theme::SEP));
    let Some(active) = app.dockers.active.or(app.dockers.open.first().copied()) else { return };
    // header: title + close
    let header = Rect::from_min_size(full.min, vec2(full.width(), 30.0));
    p.rect_filled(header, 0.0, theme::BAR_DARK);
    p.text(pos2(header.left() + 10.0, header.center().y), egui::Align2::LEFT_CENTER, title(active), theme::bold(13.0), theme::TEXT);
    let close = Rect::from_center_size(pos2(header.right() - 14.0, header.center().y), vec2(18.0, 18.0));
    let cr = ui.interact(close, ui.scope_id().with("dk_close"), Sense::click()).on_hover_text("Close");
    if cr.hovered() {
        p.rect_filled(close, 2.0, theme::HOVER);
    }
    let s = 4.0;
    p.line_segment([close.center() + vec2(-s, -s), close.center() + vec2(s, s)], Stroke::new(1.2, theme::TEXT));
    p.line_segment([close.center() + vec2(s, -s), close.center() + vec2(-s, s)], Stroke::new(1.2, theme::TEXT));
    if cr.clicked() {
        app.dockers.close(active);
        return;
    }
    // tab strip of open dockers (icons)
    let mut y = header.bottom();
    if app.dockers.open.len() > 1 {
        let tabs = Rect::from_min_size(pos2(full.left(), y), vec2(full.width(), 28.0));
        p.rect_filled(tabs, 0.0, theme::BAR);
        let mut x = tabs.left() + 4.0;
        let open = app.dockers.open.clone();
        for d in open {
            let r = Rect::from_min_size(pos2(x, tabs.top() + 2.0), vec2(26.0, 24.0));
            if r.right() > tabs.right() {
                break;
            }
            let resp = ui.interact(r, ui.scope_id().with(("dk_tab", d)), Sense::click()).on_hover_text(title(d));
            if d == active {
                p.rect_filled(r, 2.0, theme::PRESSED);
            } else if resp.hovered() {
                p.rect_filled(r, 2.0, theme::HOVER);
            }
            if let Some(t) = app.icons.get(ui.ctx(), icon(d), 16.0, if d == active { Tone::Bright } else { Tone::Normal }) {
                p.image(t.id, Rect::from_center_size(r.center(), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            if resp.clicked() {
                app.dockers.active = Some(d);
            }
            if resp.middle_clicked() {
                app.dockers.close(d);
            }
            x += 28.0;
        }
        y = tabs.bottom();
    }
    p.line_segment([pos2(full.left(), y), pos2(full.right(), y)], Stroke::new(1.0, theme::SEP));
    let content = Rect::from_min_max(pos2(full.left() + 1.0, y + 1.0), full.max);
    let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(content.shrink2(vec2(8.0, 6.0))).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::ScrollArea::vertical().id_salt(("dk_scroll", active)).auto_shrink([false, false]).show(&mut cui, |ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        docker_content(app, ui, active);
    });
}

fn docker_content(app: &mut VectorApp, ui: &mut Ui, d: DockerId) {
    match d {
        Properties => props::properties(app, ui),
        Objects => objects::objects(app, ui),
        Pages => objects::pages(app, ui),
        Comments => misc::comments(app, ui),
        ObjectData => misc::object_data(app, ui),
        ObjectStyles => misc::object_styles(app, ui),
        Symbols => misc::symbols(app, ui),
        FindReplace => text::find_replace(app, ui),
        Export => misc::export(app, ui),
        AiGenerate => crate::ai::panel(app, ui),
        Transform => arrange::transform(app, ui),
        Coordinates => arrange::coordinates(app, ui),
        LiveGuides => arrange::live_guides(app, ui),
        Guidelines => arrange::guidelines(app, ui),
        AlignDistribute => arrange::align(app, ui),
        FitObjectsToPath => arrange::fit_to_path(app, ui),
        StepRepeat => arrange::step_repeat(app, ui),
        Shape => arrange::shaping(app, ui),
        Corners => arrange::corners(app, ui),
        JoinCurves => arrange::join_curves(app, ui),
        Text => text::text(app, ui),
        FontSampler => text::font_sampler(app, ui),
        Glyphs => text::glyphs(app, ui),
        Contour => effects::contour(app, ui),
        Blend => effects::blend(app, ui),
        Extrude => effects::extrude(app, ui),
        Bevel => effects::bevel(app, ui),
        Lens => effects::lens(app, ui),
        Envelope => effects::envelope(app, ui),
        Distort => effects::distort(app, ui),
        Shadow => effects::shadow(app, ui),
        BitmapEffects => effects::bitmap_effects(app, ui),
        Color => color::color(app, ui),
        Palettes => color::palettes(app, ui),
        ColorStyles => color::color_styles(app, ui),
        ColorProofing => color::proofing(app, ui),
        Assets => misc::assets(app, ui),
        Tray => misc::tray(app, ui),
        LinksRollovers => misc::links(app, ui),
        Sources => misc::sources(app, ui),
        Views => misc::views(app, ui),
        History => misc::history(app, ui),
        Scripts => misc::scripts(app, ui),
        Learn => misc::learn(app, ui),
    }
}

// ----------------------------------------------------------------------------- shared helpers

pub(crate) fn no_doc(ui: &mut Ui) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new("Open or create a document to use this docker.").color(theme::TEXT_DIM));
}

pub(crate) fn hint(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(theme::TEXT_DIM));
}

pub(crate) fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text).strong().color(theme::TEXT));
    let r = ui.available_rect_before_wrap();
    ui.painter().line_segment([pos2(r.left(), r.top()), pos2(r.right(), r.top())], Stroke::new(1.0, theme::SEP));
    ui.add_space(2.0);
}

pub(crate) fn primary(ui: &mut Ui, text: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(egui::RichText::new(text).color(Color32::WHITE)).fill(theme::ACCENT).min_size(vec2(90.0, 24.0))).clicked()
}

/// Unit-aware length field for dockers (wider than the property bar).
pub(crate) fn len_field(app: &VectorApp, ui: &mut Ui, id: &str, v: &mut f64) -> bool {
    let (u, dpi) = app.doc().map(|d| (d.doc.settings.unit, d.doc.settings.dpi)).unwrap_or((app.settings.default_unit, 300.0));
    crate::widgets::unit_field(ui, egui::Id::unique(("dk", id)), v, u, dpi, 90.0, true)
}
