//! Dockers (Window > Dockers): the right-hand docking panel with a tab per open docker.

mod arrange;
mod assets;
mod color;
pub mod coled;
mod coords;
mod cstyles;
mod guides;
pub use guides::styled_line;
mod effects;
mod extrude;
/// The Extrude docker's pages, also the Extrude tool's drop-down panels.
pub(crate) mod extrude_pages {
    pub(crate) use super::extrude::{bevel_ui, color_ui, lighting_ui, rotation_ui};
}
mod find;
mod fx_dockers;
mod fxpage;
pub(crate) mod misc;
pub use misc::draw_hotspots;
pub use proofing::gamut_overlay;
mod objects;
pub(crate) mod palettes;
mod proofing;
pub mod ostyles;
pub mod props;
pub(crate) mod text;
mod textdk;

use crate::app::AnyVectorApp;
use crate::icons::Tone;
use crate::settings::Settings;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
pub use fx_dockers::fit_profile;

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
    ArtisticMedia,
    Bevel,
    BitmapMask,
    Blend,
    Contour,
    Envelope,
    Extrude,
    Lens,
    PhotoCocktail,
    Pointillizer,
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
    Properties, Objects, Pages, Comments, ObjectData, ObjectStyles, Symbols, FindReplace, Export, AiGenerate, Transform, Coordinates, LiveGuides, Guidelines, AlignDistribute, FitObjectsToPath, StepRepeat, Shape, Corners, JoinCurves, Text, FontSampler, Glyphs, ArtisticMedia, Bevel,
    BitmapMask, Blend, Contour, Envelope, Extrude, Lens, PhotoCocktail, Pointillizer, Color, Palettes, ColorStyles, ColorProofing, Assets, Tray, LinksRollovers, Sources, Views, History, Scripts, Learn,
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
        ArtisticMedia => "Artistic Me&dia",
        Bevel => "Be&vel",
        BitmapMask => "Bitmap &Mask",
        Blend => "&Blend",
        Contour => "&Contour",
        Envelope => "&Envelope",
        Extrude => "E&xtrude",
        Lens => "&Lens",
        PhotoCocktail => "PhotoCocktail",
        Pointillizer => "Pointillizer",
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
        ArtisticMedia => "artistic_media",
        Bevel => "bevel",
        BitmapMask => "bitmap_mask",
        Blend => "blend",
        Contour => "contour",
        Envelope => "envelope",
        Extrude => "extrude",
        Lens => "lens",
        PhotoCocktail => "photococktail",
        Pointillizer => "pointillizer",
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
    /// Only the tabs show (the docker area is folded to the right edge).
    pub collapsed: bool,
    /// A width the docker area takes on the next frame (then the user can resize it again).
    pub set_width: Option<f32>,
    /// Per-docker transient UI state.
    pub ui: DockerUi,
}

/// Transient values edited inside dockers (not saved with documents).
/// One asset in the Export docker: a page or an object with its own format and folder.
#[derive(Clone, Debug)]
pub struct ExportItem {
    pub page: Option<u64>,
    pub object: Option<vector_core::ObjId>,
    pub name: String,
    pub suffix: String,
    pub format: usize,
    pub folder: std::path::PathBuf,
    pub selected: bool,
    pub dpi: f64,
}

/// File formats offered by the Export docker.
pub const EXPORT_DOCKER_FORMATS: [&str; 8] = ["PDF", "JPG", "PNG", "SVG", "TIF", "GIF", "BMP", "WEBP"];

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
    pub t_center: [f64; 2],
    pub t_rel_center: bool,
    pub t_mirror: [bool; 2],
    pub t_size_portion: bool,
    pub t_portion: f64,
    pub t_use_anchor: bool,
    /// What the Transform fields were last filled from (selection, origin, relative settings).
    pub t_key: u64,
    pub t_copies: u32,
    /// Align to: 0 Active objects, 1 Page edge, 2 Page center, 3 Grid, 4 Specified point.
    pub align_to: usize,
    pub use_outline: bool,
    /// The specified point (from the ruler origin).
    pub align_xy: [f64; 2],
    /// Distribute to: 0 Selection extent, 1 Page extent, 2 Spacing; the spacing H and V.
    pub dist_to: usize,
    pub dist_gap: [f64; 2],
    /// Text: 0 First line baseline, 1 Last line baseline, 2 Bounding box.
    pub text_source: usize,
    pub step_copies: u32,
    pub step_offset: [f64; 2],
    /// Step and Repeat, horizontal and vertical: 0 No offset, 1 Offset, 2 Spacing between objects.
    pub step_mode: [usize; 2],
    /// Direction of the spacing: horizontal 0 Left / 1 Right, vertical 0 Up / 1 Down.
    pub step_dir: [usize; 2],
    pub step_open: [bool; 2],
    /// Shape: leave the original source / target objects, for Weld, Trim and Intersect.
    pub shape_leave: [[bool; 2]; 3],
    /// Shape ▸ Boundary: place behind the selected objects; leave the original objects.
    pub boundary_behind: bool,
    pub boundary_leave: bool,
    pub fr: find::FrUi,
    /// Palettes docker: the expanded folders, the selected row and the clipboard.
    pub pm: palettes::PmUi,
    /// Color Styles docker: the selection, the view options and the editors.
    pub cs: cstyles::CsUi,
    /// Assets docker: the source, search, sorting, filters, folder, selection and thumbnails.
    pub assets: assets::AsUi,
    pub tx: textdk::TextUi,
    pub fs: text::FsUi,
    pub gl: text::GlUi,
    /// Live Guides: the section picked by the top buttons, one section at a time, open sections, Margins open.
    pub lg_section: usize,
    pub lg_collapse: bool,
    pub lg_open: [bool; 2],
    pub lg_margins_open: bool,
    pub lg_angle_sel: usize,
    pub lg_add_angle: f64,
    /// Guidelines: 0 Horizontal, 1 Vertical, 2 Angled; x/y; angle; the selected guideline.
    pub guide_type: usize,
    pub guide_xy: [f64; 2],
    pub guide_angle: f64,
    pub guide_sel: Option<usize>,
    pub join_tolerance: f64,
    /// Join Curves: 0 Extend, 1 Chamfer, 2 Fillet, 3 Bézier curve; the fillet radius.
    pub join_mode: usize,
    pub join_radius: f64,
    pub corner_radius: f64,
    pub corner_kind: usize,
    /// Corners ▸ Chamfer distance A and B, and the lock that keeps them equal.
    pub chamfer: [f64; 2],
    pub chamfer_lock: bool,
    /// Envelope docker: edit mode (0 straight, 1 single arc, 2 double arc, 3 unconstrained),
    /// mapping mode (0 horizontal, 1 original, 2 putty, 3 vertical), keep lines, the preset
    /// chosen, and the object they were read from.
    pub env_mode: u8,
    pub env_mapping: usize,
    pub env_keep: bool,
    pub env_preset: Option<usize>,
    pub env_for: Option<vector_core::ObjId>,
    /// Extrude docker: the page shown, the light selected, and the vanishing point measured from
    /// the object's center (else the page origin).
    pub ex_tab: usize,
    pub ex_light: usize,
    pub ex_from_center: bool,
    /// Contour docker settings, and the object they were read from.
    pub contour: vector_core::object::ContourEffect,
    pub contour_for: Option<vector_core::ObjId>,
    pub blend_steps: u32,
    /// Blend docker: rotation, loop, accelerations, link, apply to sizing, color direction
    /// (0 direct, 1 clockwise, 2 counterclockwise), more options open; the blend they came from.
    pub blend_rot: f64,
    pub blend_loop: bool,
    pub blend_accel: f64,
    pub blend_caccel: f64,
    pub blend_link: bool,
    pub blend_size: bool,
    pub blend_dir: u8,
    pub blend_more: bool,
    pub blend_spacing: f64,
    pub blend_for: Option<vector_core::ObjId>,
    /// Bevel docker settings, and the object they were read from.
    pub bevel: vector_core::object::BevelEffect,
    pub bevel_for: Option<vector_core::ObjId>,
    /// Color docker: its editor, the new color, auto-apply and its target (outline).
    pub color_ed: coled::ColorEd,
    pub color_edit: Option<vector_core::Color>,
    pub color_auto: bool,
    pub color_auto_outline: bool,
    /// The color editor whose eyedropper is sampling.
    pub coled_owner: Option<String>,
    /// Properties docker.
    pub pr: props::PropsUi,
    /// Comments docker: Show Annotations, Show Resolved, the search text, the selected comment,
    /// the comment being typed (id, text, a new one), the reply being typed.
    pub comment_show: bool,
    pub comment_resolved: bool,
    pub comment_search: String,
    pub comment_sel: Option<u64>,
    pub comment_edit: Option<(u64, String, bool)>,
    pub comment_reply: String,
    pub export_format: usize,
    /// Export docker items (pages first, then objects).
    pub export_items: Vec<ExportItem>,
    pub script_error: Option<String>,
    pub link_url: String,
    /// Links and Rollovers: the hotspot cross-hatch and background colors.
    pub hotspot_colors: (vector_core::Color, Option<vector_core::Color>),
    /// Fit Objects to Path options.
    pub fit: vector_core::ops::FitPathOptions,
    pub objects_filter: String,
    pub pages_filter: String,
    pub rename: Option<(vector_core::ObjId, String)>,
    pub om: objects::ObjUi,
    pub os: ostyles::OsUi,
    pub pg: objects::PagesUi,
    /// Tray docker: the trays, the one shown, its new name while renaming, the last item id.
    pub trays: Vec<misc::TrayDef>,
    pub tray_sel: usize,
    pub tray_rename: Option<String>,
    pub tray_next: u64,
    /// Sources docker: the selected link (image id).
    pub src_sel: Option<u64>,
    /// Views docker: the selected view and the one being renamed.
    pub view_sel: Option<usize>,
    pub view_rename: Option<(usize, String)>,
    /// Scripts docker: the selected node, the folded nodes, Simple mode and a script being renamed.
    pub script_sel: Option<String>,
    pub script_closed: Vec<String>,
    pub script_simple: bool,
    pub script_rename: Option<(std::path::PathBuf, String)>,
    /// Color Proofing: the installed color profiles, the description of the control under the
    /// pointer (and the last one shown), and the name of a preset being saved.
    pub proof_profiles: Vec<proofing::Env>,
    pub proof_desc: Option<&'static str>,
    pub proof_last_desc: Option<&'static str>,
    pub proof_new_preset: Option<String>,
    /// Symbols docker: the library shown (0 the document, 1 linked symbols, 2 local symbols,
    /// 3 network symbols, 4 a local library file), the file, Local Symbols unfolded, the selected
    /// symbol (by name), the search text, Scale to world units, the symbol being renamed, and the
    /// symbols of the library file read last.
    pub sym_lib: u8,
    pub sym_file: Option<std::path::PathBuf>,
    pub sym_local_open: bool,
    pub sym_sel: Option<String>,
    pub sym_search: String,
    pub sym_world: bool,
    pub sym_rename: Option<(u64, String)>,
    pub sym_cache: Option<(std::path::PathBuf, Vec<vector_core::document::SymbolDef>)>,
    pub fx: fx_dockers::FxUi,
    pub co: coords::CoUi,
    /// Which docker field a point or length picked on the canvas goes to (1 = Transform ▸ Scale portion).
    pub pick_slot: u8,
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
            t_center: [0.0, 0.0],
            t_rel_center: false,
            t_mirror: [false, false],
            t_size_portion: false,
            t_portion: 72.0,
            t_use_anchor: false,
            t_key: 0,
            t_copies: 0,
            align_to: 0,
            use_outline: false,
            align_xy: [306.0, 396.0],
            dist_to: 0,
            dist_gap: [14.4, 14.4],
            text_source: 2,
            step_copies: 2,
            step_offset: [18.0, 18.0],
            step_mode: [1, 1],
            step_dir: [1, 0],
            step_open: [true, true],
            shape_leave: [[false, false], [true, false], [true, true]],
            boundary_behind: false,
            boundary_leave: true,
            fr: Default::default(),
            pm: Default::default(),
            cs: Default::default(),
            assets: Default::default(),
            tx: Default::default(),
            fs: Default::default(),
            gl: Default::default(),
            lg_section: 0,
            lg_collapse: false,
            lg_open: [true, true],
            lg_margins_open: true,
            lg_angle_sel: 0,
            lg_add_angle: 10.0,
            guide_type: 0,
            guide_xy: [0.0, 0.0],
            guide_angle: 0.0,
            guide_sel: None,
            join_tolerance: 7.2,
            join_mode: 0,
            join_radius: 36.0,
            corner_radius: 36.0,
            corner_kind: 0,
            chamfer: [36.0, 36.0],
            chamfer_lock: true,
            contour: vector_core::object::ContourEffect { dir: vector_core::object::ContourDir::Inside, steps: 1, offset: 7.2, ..Default::default() },
            contour_for: None,
            ex_tab: 0,
            ex_light: 0,
            ex_from_center: false,
            env_mode: 3,
            env_mapping: 2,
            env_keep: false,
            env_preset: None,
            env_for: None,
            blend_steps: 20,
            blend_rot: 0.0,
            blend_loop: false,
            blend_accel: 0.0,
            blend_caccel: 0.0,
            blend_link: true,
            blend_size: false,
            blend_dir: 0,
            blend_more: false,
            blend_spacing: 14.4,
            blend_for: None,
            bevel: vector_core::object::BevelEffect::default(),
            bevel_for: None,
            color_ed: coled::ColorEd::default(),
            pr: props::PropsUi::default(),
            color_edit: None,
            color_auto: false,
            color_auto_outline: false,
            coled_owner: None,
            comment_show: true,
            comment_resolved: false,
            comment_search: String::new(),
            comment_sel: None,
            comment_edit: None,
            comment_reply: String::new(),
            export_format: 1,
            export_items: vec![],
            script_error: None,
            link_url: String::new(),
            hotspot_colors: (vector_core::Color::rgb(230, 30, 40), None),
            fit: vector_core::ops::FitPathOptions::default(),
            objects_filter: String::new(),
            pages_filter: String::new(),
            rename: None,
            om: Default::default(),
            os: Default::default(),
            pg: Default::default(),
            trays: vec![misc::TrayDef::new("Tray 1")],
            tray_sel: 0,
            tray_rename: None,
            tray_next: 0,
            src_sel: None,
            view_sel: None,
            view_rename: None,
            script_sel: Some("rhai".into()),
            script_closed: vec![],
            script_simple: false,
            script_rename: None,
            proof_profiles: vec![],
            proof_desc: None,
            proof_last_desc: None,
            proof_new_preset: None,
            sym_lib: 0,
            sym_file: None,
            sym_local_open: false,
            sym_sel: None,
            sym_search: String::new(),
            sym_world: true,
            sym_rename: None,
            sym_cache: None,
            fx: fx_dockers::FxUi::default(),
            co: coords::CoUi::default(),
            pick_slot: 0,
        }
    }
}

impl DockerState {
    pub fn from_settings(s: &Settings) -> DockerState {
        let open: Vec<DockerId> = s.open_dockers.iter().filter_map(|n| ALL.iter().copied().find(|d| format!("{d:?}") == *n)).collect();
        DockerState { active: open.last().copied(), open, collapsed: false, set_width: None, ui: DockerUi::default() }
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

/// Width of the strip of docker tabs at the right of the docker area.
pub const TAB_STRIP: f32 = 28.0;
const TAB_BLUE: Color32 = Color32::from_rgb(29, 68, 112);

/// A docker tab: its icon at the top and its name written downward.
fn docker_tab(app: &mut AnyVectorApp, ui: &mut Ui, p: &egui::Painter, x: f32, y: f32, d: DockerId, active: bool) -> (egui::Response, f32) {
    let galley = ui.painter().layout_no_wrap(title(d), egui::FontId::proportional(12.0), theme::TEXT);
    let h = 26.0 + galley.size().x + 8.0;
    let r = Rect::from_min_size(pos2(x, y), vec2(TAB_STRIP - 4.0, h));
    let resp = ui.interact(r, ui.scope_id().with(("dk_tab", d)), Sense::click()).on_hover_text(title(d));
    let fill = if active {
        TAB_BLUE
    } else if resp.hovered() {
        theme::HOVER
    } else {
        theme::BAR
    };
    p.rect_filled(r, egui::CornerRadius { nw: 0, sw: 0, ne: 4, se: 4 }, fill);
    if let Some(t) = app.icons.get(ui.ctx(), icon(d), 16.0, if active { Tone::Bright } else { Tone::Normal }) {
        p.image(t.id, Rect::from_center_size(pos2(r.center().x, r.top() + 13.0), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    // written downward: the text turns a quarter clockwise about its top-left corner
    let gh = galley.size().y;
    let at = pos2(r.center().x + gh / 2.0, r.top() + 26.0);
    p.add(egui::epaint::TextShape::new(at, galley, if active { Color32::WHITE } else { theme::TEXT }).with_angle(std::f32::consts::FRAC_PI_2));
    (resp, h)
}

pub fn dockers_panel(app: &mut AnyVectorApp, ui: &mut Ui) {
    let full = ui.max_rect();
    // the panel keeps the width it is given (a panel shrinks to what its contents use)
    ui.expand_to_include_rect(full);
    let p = ui.painter().clone();
    p.rect_filled(full, 0.0, theme::BG);
    // left border
    p.line_segment([full.left_top(), full.left_bottom()], Stroke::new(1.0, theme::SEP));
    let Some(active) = app.dockers.active.or(app.dockers.open.first().copied()) else { return };
    let collapsed = app.dockers.collapsed;
    // header: title, then expand/collapse and close
    let header = Rect::from_min_size(full.min, vec2(full.width(), 28.0));
    let close = Rect::from_center_size(pos2(header.right() - 14.0, header.center().y), vec2(20.0, 20.0));
    let fold = Rect::from_center_size(pos2(header.right() - 38.0, header.center().y), vec2(20.0, 20.0));
    if !collapsed {
        let clip = Rect::from_min_max(header.min, pos2(fold.left() - 4.0, header.bottom()));
        p.with_clip_rect(clip).text(pos2(header.left() + 8.0, header.center().y), egui::Align2::LEFT_CENTER, title(active), theme::bold(13.0), theme::TEXT);
        let cr = ui.interact(close, ui.scope_id().with("dk_close"), Sense::click()).on_hover_text("Close");
        if cr.hovered() {
            p.rect_filled(close, 2.0, theme::HOVER);
        }
        let s = 4.5;
        p.line_segment([close.center() + vec2(-s, -s), close.center() + vec2(s, s)], Stroke::new(1.3, theme::TEXT));
        p.line_segment([close.center() + vec2(s, -s), close.center() + vec2(-s, s)], Stroke::new(1.3, theme::TEXT));
        if cr.clicked() {
            app.dockers.close(active);
            return;
        }
    }
    let fold = if collapsed { Rect::from_center_size(pos2(full.right() - TAB_STRIP / 2.0, header.center().y), vec2(20.0, 20.0)) } else { fold };
    let fr = ui.interact(fold, ui.scope_id().with("dk_fold"), Sense::click()).on_hover_text(if collapsed { "Expand Dockers" } else { "Collapse Dockers" });
    if fr.hovered() {
        p.rect_filled(fold, 2.0, theme::HOVER);
    }
    // two arrowheads, pointing right to collapse and left to expand
    for dx in [-3.5f32, 2.5] {
        let c = fold.center() + vec2(dx, 0.0);
        let pts = if collapsed { vec![c + vec2(2.5, -4.0), c + vec2(-2.5, 0.0), c + vec2(2.5, 4.0)] } else { vec![c + vec2(-2.5, -4.0), c + vec2(2.5, 0.0), c + vec2(-2.5, 4.0)] };
        p.add(egui::Shape::convex_polygon(pts, theme::TEXT, Stroke::NONE));
    }
    if fr.clicked() {
        app.dockers.collapsed = !collapsed;
    }
    // the tabs of the open dockers, down the right edge, apart from the docker
    let strip_left = full.right() - TAB_STRIP;
    if !collapsed {
        p.line_segment([pos2(strip_left - 2.5, header.bottom()), pos2(strip_left - 2.5, full.bottom())], Stroke::new(1.0, theme::SEP));
    }
    let mut y = header.bottom() + 4.0;
    let open = app.dockers.open.clone();
    for d in open {
        let (resp, h) = docker_tab(app, ui, &p, strip_left, y, d, d == active && !collapsed);
        if resp.clicked() {
            if d == active && !collapsed {
                app.dockers.collapsed = true;
            } else {
                app.dockers.active = Some(d);
                app.dockers.collapsed = false;
            }
        }
        if resp.middle_clicked() {
            app.dockers.close(d);
        }
        y += h + 2.0;
    }
    // + : open another docker
    let plus = Rect::from_center_size(pos2(strip_left + (TAB_STRIP - 4.0) / 2.0, y + 14.0), vec2(22.0, 22.0));
    let pr = ui.interact(plus, ui.scope_id().with("dk_plus"), Sense::click()).on_hover_text("Add Docker");
    if pr.hovered() {
        p.rect_filled(plus, 2.0, theme::HOVER);
    }
    p.line_segment([plus.center() - vec2(6.0, 0.0), plus.center() + vec2(6.0, 0.0)], Stroke::new(1.6, theme::TEXT));
    p.line_segment([plus.center() - vec2(0.0, 6.0), plus.center() + vec2(0.0, 6.0)], Stroke::new(1.6, theme::TEXT));
    egui::Popup::menu(&pr).show(|ui| {
        ui.set_min_width(200.0);
        for &d in ALL.iter().filter(|d| **d != Learn) {
            let on = app.dockers.open.contains(&d);
            if ui.add(egui::Button::new(title(d)).selected(on)).clicked() {
                app.dockers.show(d);
                app.dockers.collapsed = false;
                ui.close();
            }
        }
    });
    if collapsed {
        return;
    }
    let content = Rect::from_min_max(pos2(full.left() + 1.0, header.bottom()), pos2(strip_left - 2.0, full.bottom()));
    let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(content.shrink2(vec2(6.0, 4.0))).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::ScrollArea::vertical().id_salt(("dk_scroll", active)).auto_shrink([false, false]).show(&mut cui, |ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        docker_content(app, ui, active);
    });
}

fn docker_content(app: &mut AnyVectorApp, ui: &mut Ui, d: DockerId) {
    match d {
        Properties => props::properties(app, ui),
        Objects => objects::objects(app, ui),
        Pages => objects::pages(app, ui),
        Comments => misc::comments(app, ui),
        ObjectData => misc::object_data(app, ui),
        ObjectStyles => ostyles::object_styles(app, ui),
        Symbols => misc::symbols(app, ui),
        FindReplace => find::find_replace(app, ui),
        Export => misc::export(app, ui),
        AiGenerate => crate::ai::panel(app, ui),
        Transform => arrange::transform(app, ui),
        Coordinates => coords::coordinates(app, ui),
        LiveGuides => guides::live_guides(app, ui),
        Guidelines => guides::guidelines(app, ui),
        AlignDistribute => arrange::align(app, ui),
        FitObjectsToPath => arrange::fit_to_path(app, ui),
        StepRepeat => arrange::step_repeat(app, ui),
        Shape => arrange::shaping(app, ui),
        Corners => arrange::corners(app, ui),
        JoinCurves => arrange::join_curves(app, ui),
        Text => textdk::text(app, ui),
        FontSampler => text::font_sampler(app, ui),
        Glyphs => text::glyphs(app, ui),
        ArtisticMedia => fx_dockers::artistic_media(app, ui),
        Bevel => effects::bevel(app, ui),
        BitmapMask => fx_dockers::bitmap_mask(app, ui),
        Blend => effects::blend(app, ui),
        Contour => effects::contour(app, ui),
        Envelope => effects::envelope(app, ui),
        Extrude => extrude::extrude(app, ui),
        Lens => effects::lens(app, ui),
        PhotoCocktail => fx_dockers::photococktail(app, ui),
        Pointillizer => fx_dockers::pointillizer(app, ui),
        Color => color::color(app, ui),
        Palettes => palettes::palettes(app, ui),
        ColorStyles => cstyles::color_styles(app, ui),
        ColorProofing => proofing::proofing(app, ui),
        Assets => assets::assets(app, ui),
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

// ----------------------------------------------------------------------------- docker forms
// The reference dockers lay their controls out as labelled rows (the label right-aligned in a
// column), sections with a bold title under a separator, and a full-width Apply bar.

/// A labelled row: `label` right-aligned in a column `lw` points wide, then the controls.
pub(crate) fn form_row<R>(ui: &mut Ui, lw: f32, label: &str, enabled: bool, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(lw, 22.0), Sense::hover());
        ui.painter().text(pos2(r.right() - 6.0, r.center().y), egui::Align2::RIGHT_CENTER, label, egui::FontId::proportional(13.0), if enabled { theme::TEXT } else { theme::DISABLED });
        ui.add_enabled_ui(enabled, f).inner
    })
    .inner
}

/// A section: a separator line, then its bold title.
pub(crate) fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    separator(ui);
    if !title.is_empty() {
        ui.add_space(6.0);
        bold_title(ui, title);
        ui.add_space(4.0);
    }
}

/// A bold title in a docker.
pub(crate) fn bold_title(ui: &mut Ui, title: &str) {
    ui.label(egui::RichText::new(title).font(theme::bold(13.0)).color(theme::TEXT));
}

/// A section title that folds its section: an arrow (up when open), then the bold title.
pub(crate) fn fold_header(ui: &mut Ui, title: &str, open: &mut bool) {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
    let a = pos2(r.left() + 6.0, r.center().y);
    if *open {
        crate::widgets::chevron_up(ui.painter(), a, 3.5, theme::TEXT);
    } else {
        crate::widgets::chevron(ui.painter(), a, 3.5, theme::TEXT);
    }
    ui.painter().text(pos2(r.left() + 16.0, r.center().y), egui::Align2::LEFT_CENTER, title, theme::bold(12.5), theme::TEXT);
    if resp.clicked() {
        *open = !*open;
    }
}

/// A radio button with its label.
pub(crate) fn radio(ui: &mut Ui, on: bool, label: &str, enabled: bool) -> bool {
    let galley_w = ui.fonts_mut(|f| f.layout_no_wrap(crate::i18n::t(label), egui::FontId::proportional(12.5), theme::TEXT).size().x);
    let (r, resp) = ui.allocate_exact_size(vec2(galley_w + 22.0, 22.0), if enabled { Sense::click() } else { Sense::hover() });
    let c = pos2(r.left() + 7.0, r.center().y);
    let p = ui.painter();
    p.circle_stroke(c, 5.5, Stroke::new(1.0, if enabled { theme::TEXT_DIM } else { theme::SEP }));
    if on {
        p.circle_filled(c, 3.0, if enabled { theme::TEXT } else { theme::DISABLED });
    }
    p.text(pos2(r.left() + 18.0, r.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), if enabled { theme::TEXT } else { theme::DISABLED });
    resp.clicked() && enabled
}

/// The slider of the effect dockers: the label above, a track with ticks (blue up to the knob)
/// and the value with arrows at its right, the ends of the range under it.
pub(crate) fn tick_slider(ui: &mut Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, enabled: bool) -> bool {
    let full_w = ui.available_width();
    let (lo, hi) = (*range.start(), *range.end());
    let (text, dim) = (if enabled { theme::TEXT } else { theme::DISABLED }, if enabled { theme::TEXT_DIM } else { theme::SEP });
    let blue = if enabled { Color32::from_rgb(0x1e, 0x88, 0xd8) } else { Color32::from_gray(90) };
    ui.label(egui::RichText::new(label).color(text));
    let mut changed = false;
    let tw = full_w - 64.0;
    ui.horizontal(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(tw, 22.0), if enabled { Sense::click_and_drag() } else { Sense::hover() });
        let (x0, x1, y) = (r.left() + 8.0, r.right() - 8.0, r.center().y - 3.0);
        if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
            let t = ((pos.x - x0) / (x1 - x0)).clamp(0.0, 1.0) as f64;
            *v = (lo + t * (hi - lo)).round();
            changed = true;
        }
        let t = ((*v - lo) / (hi - lo).max(1e-9)).clamp(0.0, 1.0) as f32;
        let kx = x0 + t * (x1 - x0);
        let p = ui.painter();
        p.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(3.0, Color32::from_gray(80)));
        p.line_segment([pos2(x0, y), pos2(kx, y)], Stroke::new(3.0, blue));
        for k in 0..=10 {
            let tx = x0 + (x1 - x0) * k as f32 / 10.0;
            let th = if k % 5 == 0 { 3.5 } else { 2.0 };
            p.line_segment([pos2(tx, y + 6.0), pos2(tx, y + 6.0 + th)], Stroke::new(1.0, dim));
        }
        p.circle_filled(pos2(kx, y), 5.5, theme::BG);
        p.circle_stroke(pos2(kx, y), 5.5, Stroke::new(1.5, blue));
        ui.add_space(8.0);
        ui.add_enabled_ui(enabled, |ui| {
            if spin(ui, v, range.clone(), 1.0, 0, "", 48.0) {
                changed = true;
            }
        });
    });
    let (r, _) = ui.allocate_exact_size(vec2(tw, 12.0), Sense::hover());
    let font = egui::FontId::proportional(11.0);
    ui.painter().text(pos2(r.left() + 2.0, r.center().y), egui::Align2::LEFT_CENTER, format!("{lo}"), font.clone(), text);
    ui.painter().text(pos2(r.right() - 2.0, r.center().y), egui::Align2::RIGHT_CENTER, format!("{hi}"), font, text);
    changed
}

/// A color swatch with an arrow that drops a color picker; `None` is no color (white, crossed in red).
pub(crate) fn color_pick(ui: &mut Ui, id: &str, c: &mut Option<vector_core::Color>, allow_none: bool, enabled: bool) -> bool {
    color_pick_w(ui, id, c, allow_none, enabled, 44.0)
}

/// [`color_pick`], `w` points wide (the swatch fills it up to the arrow).
pub(crate) fn color_pick_w(ui: &mut Ui, id: &str, c: &mut Option<vector_core::Color>, allow_none: bool, enabled: bool, w: f32) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 24.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    p.rect_filled(r, 2.0, Color32::from_rgb(0x33, 0x33, 0x33));
    let sw = Rect::from_min_size(r.min + vec2(4.0, 5.0), vec2(w - 22.0, 14.0));
    match c {
        Some(col) => {
            p.rect_filled(sw, 1.0, Color32::from_rgb(col.r, col.g, col.b));
        }
        None => {
            p.rect_filled(sw, 1.0, Color32::WHITE);
            p.line_segment([sw.left_bottom(), sw.right_top()], Stroke::new(1.2, Color32::from_rgb(220, 40, 40)));
        }
    }
    p.rect_stroke(sw, 1.0, Stroke::new(1.0, theme::SEP), egui::StrokeKind::Outside);
    crate::widgets::chevron(p, pos2(r.right() - 9.0, r.center().y), 3.0, if enabled { theme::TEXT } else { theme::DISABLED });
    let mut changed = false;
    let pid = egui::Id::unique(("color_pick", id));
    let open = egui::Popup::menu(&resp).id(pid).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        changed = coled::picker(ui, id, c, allow_none);
    });
    if open.is_none() {
        coled::picker_closed(ui.ctx(), id);
    }
    changed
}

/// A thin horizontal line across the docker.
pub(crate) fn separator(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, theme::SEP));
}

/// The full-width Apply bar at the bottom of an effect docker.
pub(crate) fn apply_bar(ui: &mut Ui, text: &str, enabled: bool) -> bool {
    ui.add_space(4.0);
    let w = ui.available_width();
    ui.add_enabled_ui(enabled, |ui| ui.add_sized(vec2(w, 24.0), egui::Button::new(text).fill(Color32::from_rgb(0x33, 0x33, 0x33)))).inner.clicked()
}

/// A number field with up/down arrows that change it by `step`, `w` points wide.
pub(crate) fn spin<N: egui::emath::Numeric>(ui: &mut Ui, v: &mut N, range: std::ops::RangeInclusive<N>, step: f64, decimals: usize, suffix: &str, w: f32) -> bool {
    let enabled = ui.is_enabled();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let (lo, hi) = (range.start().to_f64(), range.end().to_f64());
        let mut changed = ui.add_sized(vec2(w - 14.0, 22.0), egui::DragValue::new(v).range(range).speed(step * 0.5).max_decimals(decimals).min_decimals(decimals).suffix(suffix)).changed();
        let (rect, _) = ui.allocate_exact_size(vec2(14.0, 20.0), Sense::hover());
        let up = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.center().y));
        let down = Rect::from_min_max(pos2(rect.min.x, rect.center().y), rect.max);
        let id = ui.next_auto_id();
        let ru = ui.interact(up, id.with("up"), if enabled { Sense::click() } else { Sense::hover() });
        let rd = ui.interact(down, id.with("dn"), if enabled { Sense::click() } else { Sense::hover() });
        let col = |r: &egui::Response| if !enabled { theme::DISABLED } else if r.hovered() { Color32::WHITE } else { theme::TEXT_DIM };
        crate::widgets::chevron_up(ui.painter(), up.center(), 3.0, col(&ru));
        crate::widgets::chevron(ui.painter(), down.center(), 3.0, col(&rd));
        for (r, d) in [(ru, step), (rd, -step)] {
            if r.clicked() {
                *v = N::from_f64((v.to_f64() + d).clamp(lo, hi));
                changed = true;
            }
        }
        changed
    })
    .inner
}

/// A drop-down list of `items`, `w` points wide; true when the choice changed.
pub(crate) fn choice(ui: &mut Ui, id: &str, cur: &mut usize, items: &[&str], w: f32) -> bool {
    let before = *cur;
    egui::ComboBox::from_id_salt(("dkc", id)).width(w).selected_text(items.get(*cur).copied().unwrap_or("")).show_ui(ui, |ui| {
        for (i, n) in items.iter().enumerate() {
            ui.selectable_value(cur, i, *n);
        }
    });
    before != *cur
}

/// A row of a docker list or tree, 24 points high: `depth` levels in, then a fold chevron
/// (`fold`: None on a flat list; Some(None) on a tree leaf, which keeps the chevron's room;
/// Some(Some(open)) on a node with children), an icon and the label, on `fill` when given.
/// Gives the row's response and whether the chevron was clicked.
pub(crate) fn list_row(app: &mut AnyVectorApp, ui: &mut Ui, depth: usize, fold: Option<Option<bool>>, icon: &str, label: impl Into<egui::WidgetText>, fill: Option<Color32>) -> (egui::Response, bool) {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    let p = ui.painter().clone();
    if let Some(c) = fill {
        p.rect_filled(r, 0.0, c);
    } else if resp.hovered() {
        p.rect_filled(r, 0.0, theme::HOVER);
    }
    let mut x = r.left() + 6.0 + depth as f32 * 19.0;
    let mut chevron = false;
    if let Some(f) = fold {
        if let Some(open) = f {
            let c = pos2(x + 5.0, r.center().y);
            let pts = if open { vec![c + vec2(-4.0, -2.0), c + vec2(0.0, 2.0), c + vec2(4.0, -2.0)] } else { vec![c + vec2(-2.0, -4.0), c + vec2(2.0, 0.0), c + vec2(-2.0, 4.0)] };
            p.add(egui::Shape::line(pts, Stroke::new(1.2, theme::TEXT)));
            let cr = Rect::from_min_max(pos2(x - 3.0, r.top()), pos2(x + 13.0, r.bottom()));
            chevron = resp.clicked() && resp.interact_pointer_pos().is_some_and(|q| cr.contains(q));
        }
        x += 14.0;
    }
    if !icon.is_empty() {
        if let Some(t) = app.icons.get(ui.ctx(), icon, 16.0, Tone::Normal) {
            p.image(t.id, Rect::from_min_size(pos2(x, r.center().y - 8.0), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        x += 22.0;
    }
    let g = label.into().into_galley(ui, Some(egui::TextWrapMode::Truncate), (r.right() - x - 4.0).max(10.0), egui::TextStyle::Body);
    p.galley(pos2(x, r.center().y - g.size().y / 2.0), g, theme::TEXT);
    (resp, chevron)
}

/// The small triangle in the corner of a button that drops a menu.
pub(crate) fn drop_mark(ui: &Ui, r: Rect) {
    let c = r.right_bottom() - vec2(3.0, 3.0);
    ui.painter().add(egui::Shape::convex_polygon(vec![c, c - vec2(4.0, 0.0), c - vec2(0.0, 4.0)], theme::TEXT, Stroke::NONE));
}

/// A table header of a docker list: the column titles centered over columns ending at the
/// given right edges (relative to the left).
pub(crate) fn table_header(ui: &mut Ui, cols: &[(&str, f32)]) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 0.0, Color32::from_rgb(0x33, 0x33, 0x33));
    let mut x0 = r.left();
    for (i, (t, right)) in cols.iter().enumerate() {
        let x1 = if i + 1 == cols.len() { r.right() } else { r.left() + right };
        if i > 0 {
            p.vline(x0, r.y_range(), Stroke::new(1.0, theme::SEP));
        }
        p.text(pos2((x0 + x1) / 2.0, r.center().y), egui::Align2::CENTER_CENTER, *t, egui::FontId::proportional(12.5), theme::TEXT);
        x0 = x1;
    }
}

/// Today's date, as the comments show it (year-month-day).
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    // days since 1970-01-01 to a civil date (Howard Hinnant's algorithm)
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year}-{month:02}-{day:02}")
}

/// A small icon button of the docker toolbars.
pub(crate) fn icon_button(app: &mut AnyVectorApp, ui: &mut Ui, icon: &str, tip: &str, enabled: bool) -> bool {
    icon_button_resp(app, ui, icon, tip, enabled).clicked() && enabled
}

/// [`icon_button`], giving its response (for a menu that drops from it).
pub(crate) fn icon_button_resp(app: &mut AnyVectorApp, ui: &mut Ui, icon: &str, tip: &str, enabled: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), if enabled { Sense::click() } else { Sense::hover() });
    let resp = resp.on_hover_text(tip);
    if enabled && resp.hovered() {
        ui.painter().rect_filled(r, 2.0, theme::HOVER);
    }
    if let Some(t) = app.icons.get(ui.ctx(), icon, 16.0, if enabled { Tone::Normal } else { Tone::Disabled }) {
        ui.painter().image(t.id, Rect::from_center_size(r.center(), vec2(16.0, 16.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    resp
}
