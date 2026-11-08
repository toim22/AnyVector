//! Application settings (Tools > Options), persisted as JSON in %APPDATA%\Vector\settings.json.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use vector_core::Unit;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartupAction {
    WelcomeScreen,
    NewDocument,
    OpenDocument,
    OpenLastEdited,
    Nothing,
}

impl StartupAction {
    pub const ALL: [StartupAction; 5] = [StartupAction::WelcomeScreen, StartupAction::NewDocument, StartupAction::OpenDocument, StartupAction::OpenLastEdited, StartupAction::Nothing];
    pub fn name(self) -> &'static str {
        match self {
            StartupAction::WelcomeScreen => "Welcome Screen",
            StartupAction::NewDocument => "New document",
            StartupAction::OpenDocument => "Open document",
            StartupAction::OpenLastEdited => "Open last edited",
            StartupAction::Nothing => "Nothing",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WheelAction {
    Zoom,
    Scroll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuBarMode {
    Normal,
    File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonSize {
    Small,
    Medium,
    Large,
}

impl ButtonSize {
    pub fn icon(self) -> f32 {
        match self {
            ButtonSize::Small => 18.0,
            ButtonSize::Medium => 22.0,
            ButtonSize::Large => 28.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeSize {
    Small,
    Medium,
    Large,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // General
    pub startup: StartupAction,
    pub show_new_doc_dialog: bool,
    pub undo_levels: usize,
    // Tablet mode
    pub auto_tablet: bool,
    // Display
    pub proof_colors: bool,
    pub interruptible_refresh: bool,
    pub manual_refresh: bool,
    pub auto_panning: bool,
    pub show_tooltips: bool,
    pub antialias_bitmaps: bool,
    pub hide_bbox_curve_tools: bool,
    pub highlight_selected: bool,
    pub fullscreen_enhanced: bool,
    pub fullscreen_page_border: bool,
    pub wheel_action: WheelAction,
    pub zoom_rate: f64,
    pub alt_zoom_rate: f64,
    // Edit
    pub constrain_angle: f64,
    pub drawing_precision: u32,
    pub inactive_visibility: f64,
    pub dblclick_focus_mode: bool,
    // Nodes & handles
    pub node_size: NodeSize,
    pub show_curve_direction: bool,
    pub node_tracking: bool,
    pub node_main_color: [u8; 3],
    pub node_secondary_color: [u8; 3],
    pub show_node_highlight: bool,
    pub node_types_colors: bool,
    // PowerClip
    pub powerclip_drag_mode: u8,
    pub powerclip_autocenter: u8,
    pub powerclip_empty_lines: bool,
    pub powerclip_lines_print: bool,
    // PowerTRACE
    pub quick_trace_method: String,
    // Snapping
    pub snap_to_objects: bool,
    pub snap_self: bool,
    pub snap_to_page: bool,
    pub snap_radius: f64,
    pub show_snap_marks: bool,
    pub show_snap_tip: bool,
    pub snap_modes: [bool; 9],
    // Save
    pub backup_on_save: bool,
    pub auto_backup: bool,
    pub auto_backup_minutes: u32,
    pub auto_backup_dir: Option<PathBuf>,
    pub embed_fonts: bool,
    // Text
    pub text_show_handles: bool,
    pub text_increment: f64,
    pub show_text_frames: bool,
    pub show_text_links: bool,
    pub recent_fonts: usize,
    pub spell_check: bool,
    pub quickcorrect_capitalize: bool,
    pub quickcorrect_two_caps: bool,
    pub quickcorrect_days: bool,
    pub quickcorrect_links: bool,
    pub typographic_quotes: bool,
    pub replace_while_typing: bool,
    pub replacements: Vec<(String, String)>,
    // Warnings
    pub warnings: Vec<(String, bool)>,
    // Scripts
    pub trust_scripts: bool,
    pub delay_load_scripts: bool,
    pub plugin_folders: Vec<(PathBuf, bool)>,
    // Customization
    pub center_dialogs: bool,
    pub ui_scale: f64,
    pub desktop_color: [u8; 3],
    pub button_size: ButtonSize,
    pub show_floating_title: bool,
    pub lock_toolbars: bool,
    pub menu_bar_mode: MenuBarMode,
    pub show_menu_bar: bool,
    pub show_status_bar: bool,
    pub show_standard: bool,
    pub show_property_bar: bool,
    pub show_toolbox: bool,
    pub extra_toolbars: Vec<String>,
    pub hidden_tools: Vec<crate::tools::ToolId>,
    pub auto_update_doc_palette: bool,
    pub palette_rows: u32,
    pub wide_borders: bool,
    pub large_swatches: bool,
    pub show_no_color_well: bool,
    pub right_click_outline: bool,
    pub default_palette_visible: bool,
    pub doc_palette_visible: bool,
    pub color_styles_palette_visible: bool,
    pub open_palettes: Vec<String>,
    pub shortcuts: Vec<(String, String)>,
    // Tools
    pub crosshair_cursor: bool,
    pub treat_as_filled: bool,
    pub windows_modifiers: bool,
    pub eraser_thickness: f64,
    pub eraser_reduce_nodes: bool,
    pub zoom_right_click_out: bool,
    pub zoom_relative_1_1: bool,
    pub zoom_center_mouse: bool,
    pub freehand_smoothing: f64,
    pub corner_threshold: f64,
    pub straight_threshold: f64,
    pub auto_join: f64,
    pub smart_draw_delay: f64,
    pub rect_corner: u8,
    pub rect_radii: [f64; 4],
    pub rect_scale_corners: bool,
    pub ellipse_kind: u8,
    pub ellipse_start: f64,
    pub ellipse_end: f64,
    pub ellipse_clockwise: bool,
    pub graph_cols: u32,
    pub graph_rows: u32,
    pub spiral_log: bool,
    pub spiral_revs: u32,
    pub spiral_expansion: f64,
    pub polygon_sides: u32,
    pub star_points: u32,
    pub star_sharpness: f64,
    pub table_rows: u32,
    pub table_cols: u32,
    pub table_tab_moves: bool,
    pub dim_style: u8,
    pub dim_precision: u32,
    pub dim_prefix: String,
    pub dim_suffix: String,
    pub angular_precision: u32,
    pub connector_geometric_anchors: bool,
    pub connector_route_distance: f64,
    pub mesh_cols: u32,
    pub mesh_rows: u32,
    // Global
    pub background_tasks: bool,
    pub language: String,
    pub hardware_accel: bool,
    pub print_only_current: bool,
    pub file_locations: Vec<(String, String)>,
    pub show_choose_palette: bool,
    pub user_name: String,
    pub user_company: String,
    pub usage_stats: bool,
    // Session
    pub recent_files: Vec<PathBuf>,
    pub last_dir: Option<PathBuf>,
    pub workspace: String,
    pub open_dockers: Vec<String>,
    pub docker_width: f32,
    pub default_unit: Unit,
    pub show_tool_hints: u8,
    pub sync_folder: Option<PathBuf>,
    pub ai_provider_url: String,
    pub ai_api_key_env: String,
    pub window_maximized: bool,
    pub last_export_dir: Option<PathBuf>,
    // Live guides
    pub live_guides: bool,
    pub live_guides_edges: bool,
    pub live_guides_centers: bool,
    pub live_guides_page: bool,
    pub tablet_desktop_ws: String,
    pub tablet_touch_ws: String,
    pub dblclick_bitmap_editor: bool,
    pub ask_save_previous: bool,
    pub keep_appearance: bool,
    pub default_version: String,
    pub backup_dir: Option<PathBuf>,
    pub expand_paragraph_frames: bool,
    pub spell_errors_all: bool,
    pub theme_name: String,
    pub ui_language_ask: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            startup: StartupAction::WelcomeScreen,
            show_new_doc_dialog: true,
            undo_levels: 150,
            auto_tablet: true,
            proof_colors: false,
            interruptible_refresh: true,
            manual_refresh: false,
            auto_panning: true,
            show_tooltips: true,
            antialias_bitmaps: true,
            hide_bbox_curve_tools: false,
            highlight_selected: false,
            fullscreen_enhanced: true,
            fullscreen_page_border: true,
            wheel_action: WheelAction::Zoom,
            zoom_rate: 15.0,
            alt_zoom_rate: 100.0,
            constrain_angle: 15.0,
            drawing_precision: 3,
            inactive_visibility: 20.0,
            dblclick_focus_mode: true,
            node_size: NodeSize::Small,
            show_curve_direction: true,
            node_tracking: false,
            node_main_color: [0, 102, 255],
            node_secondary_color: [255, 51, 51],
            show_node_highlight: true,
            node_types_colors: false,
            powerclip_drag_mode: 0,
            powerclip_autocenter: 0,
            powerclip_empty_lines: true,
            powerclip_lines_print: true,
            quick_trace_method: "Clipart".into(),
            snap_to_objects: true,
            snap_self: true,
            snap_to_page: false,
            snap_radius: 10.0,
            show_snap_marks: true,
            show_snap_tip: true,
            snap_modes: [true; 9],
            backup_on_save: true,
            auto_backup: true,
            auto_backup_minutes: 20,
            auto_backup_dir: None,
            embed_fonts: true,
            text_show_handles: true,
            text_increment: 1.0,
            show_text_frames: true,
            show_text_links: true,
            recent_fonts: 5,
            spell_check: true,
            quickcorrect_capitalize: false,
            quickcorrect_two_caps: true,
            quickcorrect_days: true,
            quickcorrect_links: true,
            typographic_quotes: true,
            replace_while_typing: true,
            replacements: vec![
                ("(c".into(), "©".into()),
                ("(c)".into(), "©".into()),
                ("(r".into(), "®".into()),
                ("(r)".into(), "®".into()),
                ("--".into(), "–".into()),
                ("---".into(), "—".into()),
            ],
            warnings: crate::dialogs::options::default_warnings(),
            trust_scripts: true,
            delay_load_scripts: true,
            plugin_folders: vec![],
            center_dialogs: true,
            ui_scale: 100.0,
            desktop_color: [0x26, 0x26, 0x26],
            button_size: ButtonSize::Small,
            show_floating_title: true,
            lock_toolbars: true,
            menu_bar_mode: MenuBarMode::File,
            show_menu_bar: true,
            show_status_bar: true,
            show_standard: true,
            show_property_bar: true,
            show_toolbox: true,
            extra_toolbars: vec![],
            hidden_tools: vec![],
            auto_update_doc_palette: true,
            palette_rows: 1,
            wide_borders: false,
            large_swatches: false,
            show_no_color_well: true,
            right_click_outline: true,
            default_palette_visible: true,
            doc_palette_visible: true,
            color_styles_palette_visible: false,
            open_palettes: vec![],
            shortcuts: vec![],
            crosshair_cursor: false,
            treat_as_filled: true,
            windows_modifiers: false,
            eraser_thickness: 18.0,
            eraser_reduce_nodes: true,
            zoom_right_click_out: true,
            zoom_relative_1_1: true,
            zoom_center_mouse: false,
            freehand_smoothing: 50.0,
            corner_threshold: 5.0,
            straight_threshold: 5.0,
            auto_join: 5.0,
            smart_draw_delay: 0.0,
            rect_corner: 0,
            rect_radii: [0.0; 4],
            rect_scale_corners: false,
            ellipse_kind: 0,
            ellipse_start: 90.0,
            ellipse_end: 90.0,
            ellipse_clockwise: false,
            graph_cols: 1,
            graph_rows: 1,
            spiral_log: false,
            spiral_revs: 4,
            spiral_expansion: 100.0,
            polygon_sides: 7,
            star_points: 5,
            star_sharpness: 53.0,
            table_rows: 3,
            table_cols: 4,
            table_tab_moves: true,
            dim_style: 0,
            dim_precision: 2,
            dim_prefix: String::new(),
            dim_suffix: String::new(),
            angular_precision: 2,
            connector_geometric_anchors: false,
            connector_route_distance: 0.0,
            mesh_cols: 2,
            mesh_rows: 2,
            background_tasks: true,
            language: "English-US".into(),
            hardware_accel: true,
            print_only_current: false,
            file_locations: vec![],
            show_choose_palette: true,
            user_name: std::env::var("USERNAME").unwrap_or_default(),
            user_company: String::new(),
            usage_stats: false,
            recent_files: vec![],
            last_dir: None,
            workspace: "Default".into(),
            open_dockers: vec![],
            docker_width: 280.0,
            default_unit: Unit::Inches,
            show_tool_hints: 0,
            sync_folder: None,
            ai_provider_url: String::new(),
            ai_api_key_env: "ANTHROPIC_API_KEY".into(),
            window_maximized: true,
            last_export_dir: None,
            live_guides: true,
            live_guides_edges: true,
            live_guides_centers: true,
            live_guides_page: true,
            tablet_desktop_ws: "Default".into(),
            tablet_touch_ws: "Touch".into(),
            dblclick_bitmap_editor: false,
            ask_save_previous: true,
            keep_appearance: true,
            default_version: "1.0".into(),
            backup_dir: None,
            expand_paragraph_frames: false,
            spell_errors_all: true,
            theme_name: "Dark".into(),
            ui_language_ask: false,
        }
    }
}

pub fn config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let d = base.join("Vector");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn documents_dir() -> PathBuf {
    dirs::document_dir().unwrap_or_else(|| PathBuf::from("."))
}

impl Settings {
    pub fn path() -> PathBuf {
        config_dir().join("settings.json")
    }

    pub fn load() -> Settings {
        std::fs::read(Self::path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(b) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(Self::path(), b);
        }
    }

    pub fn add_recent(&mut self, p: &std::path::Path) {
        self.recent_files.retain(|x| x != p);
        self.recent_files.insert(0, p.to_path_buf());
        self.recent_files.truncate(12);
    }

    /// Wheel zoom factor from the zoom rate percentage.
    pub fn zoom_factor(&self) -> f64 {
        1.0 + (self.zoom_rate / 100.0).max(0.01) * (1.25 - 1.0) / 0.15
    }
}
