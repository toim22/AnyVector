//! Application settings (Tools > Options), persisted as JSON in %APPDATA%\AnyVector\settings.json.

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
    /// Bitmap effect presets saved with (+) on an effect's page: effect id → (name, settings).
    pub fx_presets: std::collections::BTreeMap<String, Vec<(String, std::collections::BTreeMap<String, vector_core::object::FxValue>)>>,
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
    /// Arrowheads made with Object ▸ Create ▸ Arrowhead, listed in the Arrowhead selectors.
    pub custom_arrowheads: Vec<vector_core::style::CustomArrow>,
    /// Line styles made with Edit Line Style (dash lengths, in outline widths).
    pub custom_dashes: Vec<Vec<f64>>,
    /// Properties ▸ Summary ▸ Wrap Connector Line.
    pub wrap_connector_lines: bool,
    /// Object Styles ▸ Set as New Document Defaults.
    pub default_object_styles: Vec<vector_core::document::ObjectStyle>,
    /// Color Styles ▸ Set as New Document Defaults: the color styles and harmonies of new documents.
    pub default_color_styles: Vec<vector_core::NamedColor>,
    pub default_color_harmonies: Vec<vector_core::document::ColorHarmony>,
    /// Objects docker ▸ Options.
    pub edit_across_layers: bool,
    pub om_expand_to_selection: bool,
    pub om_keep_desktop: bool,
    pub om_select_activates_layer: bool,
    pub om_show_properties: bool,
    pub om_show_master_on_pages: bool,
    /// View Layers and Objects (true) or View Pages, Layers, and Objects.
    pub om_layers_view: bool,
    /// Text ▸ Show Non-Printing Characters.
    pub show_nonprinting: bool,
    /// Text ▸ Insert Formatting Code ▸ Custom Optional Hyphens: (word, hyphenated word) and when they apply.
    pub hyphen_defs: Vec<(String, String)>,
    pub hyphen_when_typing: bool,
    pub hyphen_when_pasting: bool,
    /// QuickCorrect typographic quotes: open and close, single and double.
    pub quote_single: (String, String),
    pub quote_double: (String, String),
    pub straight_after_numbers: bool,
    /// Spelling options (Writing Tools ▸ Options).
    pub spell_auto_start: bool,
    pub spell_beep: bool,
    pub spell_recheck: bool,
    pub spell_numbers: bool,
    pub spell_duplicates: bool,
    pub spell_caps: bool,
    pub spell_prompt: bool,
    pub spell_phonetic: bool,
    pub spell_suggestions: u32,
    pub spell_add_to_quickcorrect: bool,
    /// Thesaurus options.
    pub thes_auto_lookup: bool,
    pub thes_spelling_assist: bool,
    /// Toolbox items shown or hidden with Quick Customize: (first 8 characters of the item id, shown).
    pub toolbox_items: Vec<(String, bool)>,
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
    /// Options of the tools' property bars.
    pub topts: ToolOpts,
    pub zoom_right_click_out: bool,
    pub zoom_relative_1_1: bool,
    pub zoom_center_mouse: bool,
    pub freehand_smoothing: f64,
    pub corner_threshold: f64,
    pub straight_threshold: f64,
    pub auto_join: f64,
    pub smart_draw_delay: f64,
    /// Smart Drawing: how readily strokes become shapes, 0 (None) to 5 (Highest).
    pub smart_recognition: u8,
    /// Smart Drawing: how much a stroke that stays a curve is smoothed, 0 (None) to 5 (Highest).
    pub smart_smoothing: u8,
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
    /// The Print dialog's last settings and printer.
    pub print: crate::printing::PrintSettings,
    pub printer: Option<String>,
    /// Publish to PDF: the last PDF settings and preset.
    pub pdf: Option<crate::dialogs::export::PdfSettings>,
    pub pdf_preset: Option<String>,
    /// File ▸ Export: last file type and bitmap resolution; SVG Export units.
    pub export_format: usize,
    pub export_dpi: Option<f64>,
    pub svg_unit: usize,
    /// Export For ▸ WordPress: the last site and user name (never the password).
    pub wordpress_site: Option<String>,
    pub wordpress_user: Option<String>,
    /// File ▸ Acquire Image ▸ Select WIA Source: the chosen device.
    pub wia_device: Option<String>,
    pub file_locations: Vec<(String, String)>,
    pub show_choose_palette: bool,
    pub user_name: String,
    pub user_company: String,
    pub usage_stats: bool,
    // Session
    pub recent_files: Vec<PathBuf>,
    /// Script folders loaded in the Scripts docker.
    pub script_folders: Vec<PathBuf>,
    pub last_dir: Option<PathBuf>,
    pub workspace: String,
    /// The user's own workspaces, listed after the built-in ones (Window ▸ Workspace).
    pub custom_workspaces: Vec<crate::workspaces::Custom>,
    /// The layout each workspace had when the user left it, by name.
    pub workspace_layouts: std::collections::BTreeMap<String, crate::workspaces::Layout>,
    pub open_dockers: Vec<String>,
    /// Artistic Media docker: the custom strokes (name, stroke).
    pub custom_strokes: Vec<(String, vector_core::style::WidthProfile)>,
    pub docker_width: f32,
    pub default_unit: Unit,
    pub show_tool_hints: u8,
    pub sync_folder: Option<PathBuf>,
    pub ai_provider_url: String,
    pub ai_api_key_env: String,
    /// AI Generate: the Generative AI terms were accepted.
    pub ai_terms_accepted: bool,
    /// Links and Rollovers: cross-hatch the Web links' hotspots on the page.
    pub show_hotspots: bool,
    /// Assets docker: the folders of local and network assets, the options, the favorite and
    /// recent assets, the thumbnail size and the view (thumbnails instead of the list).
    pub asset_folders: Vec<String>,
    pub asset_single_folder: bool,
    pub asset_recursive: bool,
    pub asset_favorites: Vec<String>,
    pub asset_recent: Vec<String>,
    pub asset_thumb: f32,
    pub asset_icon_view: bool,
    /// Color Proofing: the gamut warning, its color and transparency.
    pub proof_gamut: bool,
    pub proof_gamut_color: vector_core::Color,
    pub proof_gamut_transparency: u32,
    /// Color Proofing: the simulated environment, its color space (0 CMYK, 1 RGB, 2 Grayscale),
    /// Preserve numbers, the rendering intent, the presets and the current preset ("" Custom).
    pub proof_env: String,
    pub proof_env_kind: u8,
    pub proof_preserve: bool,
    pub proof_intent: u8,
    pub proof_presets: Vec<ProofPreset>,
    pub proof_preset: String,
    pub proof_show_desc: bool,
    pub window_maximized: bool,
    pub last_export_dir: Option<PathBuf>,
    // Live guides
    pub live_guides: bool,
    pub live_guides_edges: bool,
    pub live_guides_centers: bool,
    pub live_guides_page: bool,
    /// View ▸ Dynamic Guides.
    pub dynamic_guides: bool,
    /// Live Guides ▸ align to the individual objects in a group.
    pub live_guides_groups: bool,
    pub intelligent_spacing: bool,
    pub intelligent_dimensioning: bool,
    /// Live Guides ▸ line style (0 solid, 1 dashed, 2 dotted, 3 long dashes) and color of the alignment guides.
    pub align_guide_style: usize,
    pub align_guide_color: [u8; 3],
    /// Live Guides ▸ Margins: alignment guides offset or inset from the object edges.
    pub guide_margins: bool,
    pub guide_margin_style: usize,
    pub guide_margin_color: [u8; 3],
    /// Horizontal and vertical margin, in points.
    pub guide_margin: [f64; 2],
    /// 0 Offset, 1 Inset, 2 Offset and inset, 3 No margin.
    pub guide_margin_mode: [usize; 2],
    pub guide_margins_locked: bool,
    pub dyn_guide_style: usize,
    pub dyn_guide_color: [u8; 3],
    pub dyn_screen_tips: bool,
    pub dyn_extend_segment: bool,
    /// Snap to tick spacing along the dynamic guides.
    pub dyn_ticks: bool,
    pub dyn_tick_spacing: f64,
    /// The dynamic guide angles (degrees) and whether each is on.
    pub dyn_angles: Vec<(f64, bool)>,
    /// Guidelines ▸ line style and color of new guidelines.
    pub guide_line_style: usize,
    pub guide_color: [u8; 3],
    /// Guidelines ▸ Options ▸ Show at scale.
    pub guides_at_scale: bool,
    /// View ▸ Rasterize Complex Effects.
    pub rasterize_complex_effects: bool,
    /// View ▸ Document Navigator and View ▸ Scrollbars.
    pub show_doc_navigator: bool,
    pub show_scrollbars: bool,
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
    /// The setup's language choice already applied (its time stamp, see `i18n::installer_choice`).
    pub installer_lang_stamp: String,
    /// Allow product updates: 0 keep up to date, 1 check and let me choose, 2 never.
    pub update_mode: u8,
    /// Page presets saved with "+" in Create a New Document.
    pub custom_page_presets: Vec<CustomPagePreset>,
    /// Names of presets used most recently (newest first).
    pub recent_page_presets: Vec<String>,
    /// The settings of the last new document (Create a New Document, or the page size last
    /// set on the property bar): new documents, including the one made at start-up, start
    /// from them, as CorelDRAW remembers its last paper type. `None` until the first one.
    pub last_new_doc: Option<NewDocDefaults>,
    /// Save Drawing options (also under Options ▸ Document ▸ Save and the Advanced... button).
    pub save: SaveSettings,
}

/// A user-defined page preset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomPagePreset {
    pub name: String,
    /// Size in points.
    pub width: f64,
    pub height: f64,
    pub unit: vector_core::Unit,
    pub dpi: f64,
    pub cmyk: bool,
}

/// The remembered Create a New Document settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NewDocDefaults {
    /// The page preset chosen (None for a custom size).
    pub preset: Option<String>,
    /// Page size in points (the orientation is width > height).
    pub width: f64,
    pub height: f64,
    pub unit: vector_core::Unit,
    pub dpi: f64,
    pub pages: usize,
    pub multipage: bool,
    pub bleed_on: bool,
    /// Bleed in points.
    pub bleed: f64,
    pub color: vector_core::document::ColorSettings,
}

impl Default for NewDocDefaults {
    fn default() -> Self {
        NewDocDefaults {
            preset: Some("Letter".into()),
            width: 612.0,
            height: 792.0,
            unit: vector_core::Unit::Inches,
            dpi: 300.0,
            pages: 1,
            multipage: false,
            bleed_on: false,
            bleed: 9.0,
            color: Default::default(),
        }
    }
}

/// Options used when saving drawings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SaveSettings {
    pub embed_color_profiles: bool,
    /// Optimize for: false = speed, true = size.
    pub optimize_for_size: bool,
    pub bitmap_compression: bool,
    pub object_compression: bool,
    pub save_thumbnail: bool,
    pub save_presentation_attributes: bool,
}

impl Default for SaveSettings {
    fn default() -> Self {
        SaveSettings { embed_color_profiles: true, optimize_for_size: false, bitmap_compression: true, object_compression: true, save_thumbnail: true, save_presentation_attributes: true }
    }
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
            fx_presets: Default::default(),
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
            custom_arrowheads: vec![],
            custom_dashes: vec![],
            wrap_connector_lines: true,
            default_object_styles: vec![],
            default_color_styles: vec![],
            default_color_harmonies: vec![],
            edit_across_layers: true,
            om_expand_to_selection: true,
            om_keep_desktop: true,
            om_select_activates_layer: true,
            om_show_properties: false,
            om_show_master_on_pages: false,
            om_layers_view: false,
            show_nonprinting: false,
            hyphen_defs: vec![],
            hyphen_when_typing: true,
            hyphen_when_pasting: true,
            quote_single: ("‘".into(), "’".into()),
            quote_double: ("“".into(), "”".into()),
            straight_after_numbers: true,
            spell_auto_start: true,
            spell_beep: false,
            spell_recheck: false,
            spell_numbers: false,
            spell_duplicates: true,
            spell_caps: true,
            spell_prompt: false,
            spell_phonetic: false,
            spell_suggestions: 5,
            spell_add_to_quickcorrect: false,
            thes_auto_lookup: true,
            thes_spelling_assist: true,
            toolbox_items: vec![],
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
            topts: ToolOpts::default(),
            zoom_right_click_out: true,
            zoom_relative_1_1: true,
            zoom_center_mouse: false,
            freehand_smoothing: 50.0,
            corner_threshold: 5.0,
            straight_threshold: 5.0,
            auto_join: 5.0,
            smart_draw_delay: 0.0,
            smart_recognition: 3,
            smart_smoothing: 3,
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
            // empty: Windows' display language (see i18n::Lang::resolve)
            language: String::new(),
            hardware_accel: true,
            print_only_current: false,
            print: Default::default(),
            printer: None,
            pdf: None,
            pdf_preset: None,
            export_format: 2,
            export_dpi: None,
            svg_unit: 0,
            wordpress_site: None,
            wordpress_user: None,
            wia_device: None,
            file_locations: vec![],
            show_choose_palette: true,
            user_name: std::env::var("USERNAME").unwrap_or_default(),
            user_company: String::new(),
            usage_stats: false,
            recent_files: vec![],
            script_folders: vec![],
            last_dir: None,
            workspace: "Default".into(),
            custom_workspaces: vec![],
            workspace_layouts: Default::default(),
            open_dockers: vec![],
            custom_strokes: vec![],
            docker_width: 280.0,
            default_unit: Unit::Inches,
            show_tool_hints: 0,
            sync_folder: None,
            ai_provider_url: String::new(),
            ai_api_key_env: "ANTHROPIC_API_KEY".into(),
            ai_terms_accepted: false,
            show_hotspots: true,
            asset_folders: vec![],
            asset_single_folder: true,
            asset_recursive: true,
            asset_favorites: vec![],
            asset_recent: vec![],
            asset_thumb: 48.0,
            asset_icon_view: false,
            proof_gamut: false,
            proof_gamut_color: vector_core::Color::rgb(0, 255, 0),
            proof_gamut_transparency: 0,
            proof_env: "U.S. Web Coated (SWOP) v2".into(),
            proof_env_kind: 0,
            proof_preserve: false,
            proof_intent: 0,
            proof_presets: vec![],
            proof_preset: String::new(),
            proof_show_desc: true,
            window_maximized: true,
            last_export_dir: None,
            live_guides: true,
            live_guides_edges: true,
            live_guides_centers: true,
            live_guides_page: true,
            dynamic_guides: false,
            live_guides_groups: false,
            intelligent_spacing: true,
            intelligent_dimensioning: true,
            align_guide_style: 1,
            align_guide_color: [220, 215, 255],
            guide_margins: false,
            guide_margin_style: 1,
            guide_margin_color: [255, 180, 100],
            guide_margin: [14.4, 14.4],
            guide_margin_mode: [0, 0],
            guide_margins_locked: true,
            dyn_guide_style: 1,
            dyn_guide_color: [100, 150, 255],
            dyn_screen_tips: true,
            dyn_extend_segment: false,
            dyn_ticks: false,
            dyn_tick_spacing: 9.0,
            dyn_angles: vec![(0.0, true), (30.0, false), (45.0, true), (60.0, false), (90.0, true), (120.0, false), (135.0, true), (150.0, false)],
            guide_line_style: 2,
            guide_color: [0, 0, 255],
            guides_at_scale: false,
            rasterize_complex_effects: false,
            show_doc_navigator: true,
            show_scrollbars: true,
            tablet_desktop_ws: "Default".into(),
            tablet_touch_ws: "Touch".into(),
            dblclick_bitmap_editor: false,
            ask_save_previous: true,
            keep_appearance: true,
            default_version: crate::settings::FILE_VERSIONS[0].into(),
            backup_dir: None,
            expand_paragraph_frames: false,
            spell_errors_all: true,
            theme_name: "Dark".into(),
            ui_language_ask: false,
            installer_lang_stamp: String::new(),
            update_mode: crate::updater::KEEP_UP_TO_DATE,
            custom_page_presets: Vec::new(),
            recent_page_presets: Vec::new(),
            last_new_doc: None,
            save: SaveSettings::default(),
        }
    }
}

/// "2026-11-08 14:05" (UTC) for a file time.
pub fn format_time(t: std::time::SystemTime) -> String {
    let secs = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", rem / 3600, (rem % 3600) / 60)
}

/// File-format versions offered in Save Drawing ▸ Version and Options ▸ Save.
pub const FILE_VERSIONS: &[&str] = &["0.1 (2026.0)"];

/// A Color Proofing preset: the environment, its color space, Preserve numbers and the intent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProofPreset {
    pub name: String,
    pub env: String,
    pub kind: u8,
    pub preserve: bool,
    pub intent: u8,
}

pub fn config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let d = base.join("AnyVector");
    // the program was called Vector before 0.1.0: its settings folder moves to the new name
    if !cfg!(test) && !d.exists() {
        let old = base.join("Vector");
        if old.is_dir() {
            let _ = std::fs::rename(&old, &d);
        }
    }
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
        // tests never write over the user's settings
        if cfg!(test) {
            return;
        }
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

/// The options set on the tools' property bars (kept between sessions, as in the original).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolOpts {
    /// 2-Point Line: 0 line, 1 perpendicular, 2 tangent.
    pub line2_mode: u8,
    /// Knife: 0 2-point line, 1 freehand, 2 Bézier.
    pub knife_mode: u8,
    pub knife_auto_close: bool,
    /// Cut span: 0 none, 1 create gap, 2 overlap.
    pub knife_span: u8,
    pub knife_width: f64,
    /// Outline options: 0 automatic, 1 convert outlines to objects, 2 keep outlines.
    pub knife_outline: u8,
    /// Eyedropper sample size: 0 1x1, 1 2x2, 2 5x5.
    pub eye_area: u8,
    /// Attributes eyedropper: properties (outline, fill, text), transformations (size, rotation,
    /// position) and effects (perspective, envelope, blend, extrude, contour, lens, PowerClip,
    /// drop shadow, distortion) it copies.
    pub eye_props: [bool; 3],
    pub eye_xforms: [bool; 3],
    pub eye_effects: [bool; 9],
    /// Free Transform: 0 rotate, 1 reflect, 2 scale, 3 skew.
    pub ft_mode: u8,
    pub ft_duplicate: bool,
    pub ft_relative: bool,
    /// Envelope: 0 straight line, 1 single arc, 2 double arc, 3 unconstrained.
    pub env_mode: u8,
    /// Node selection: 0 rectangular, 1 freehand.
    pub env_sel_mode: u8,
    /// Mapping for new envelopes: 0 horizontal, 1 original, 2 putty, 3 vertical.
    pub env_mapping: u8,
    pub env_keep_lines: bool,
    pub mesh_sel_mode: u8,
    pub twirl_ccw: bool,
    pub smear_smooth: bool,
    pub repel: bool,
    /// Pen pressure for Smooth, Smear, Twirl and Attract and Repel.
    pub pen_pressure: bool,
    pub rough_pressure: bool,
    pub rough_freq: f64,
    pub rough_dryout: f64,
    pub rough_use_tilt: bool,
    pub rough_tilt: f64,
    /// Spike direction: false automatic, true fixed (at `rough_bearing`).
    pub rough_fixed: bool,
    pub rough_bearing: f64,
    pub smudge_pressure: bool,
    pub smudge_dryout: f64,
    pub smudge_use_tilt: bool,
    pub smudge_tilt: f64,
    pub smudge_use_bearing: bool,
    pub smudge_bearing: f64,
    pub eraser_square: bool,
    pub eraser_pressure: bool,
    pub eraser_use_tilt: bool,
    pub eraser_tilt: f64,
    pub eraser_use_bearing: bool,
    pub eraser_bearing: f64,
    pub pen_preview: bool,
    pub pen_auto_add: bool,
    pub polyline_auto_close: bool,
    pub reflect_h: bool,
    pub reflect_v: bool,
    /// LiveSketch.
    pub ls_timer: f64,
    pub ls_include: bool,
    pub ls_distance: f64,
    pub ls_single: bool,
    pub ls_smoothing: f64,
    pub ls_preview: bool,
    /// Painterly Brush.
    pub paint_brush: usize,
    pub paint_recent: Vec<usize>,
    pub paint_alpha: f64,
    pub paint_smoothing: f64,
    pub paint_texture: usize,
    pub paint_texture_strength: f64,
    pub paint_density: f64,
    pub paint_tilt: f64,
    pub paint_tilt_dir: f64,
    pub paint_glow: bool,
    pub paint_taper: bool,
    /// Artistic Media: scale the stroke with the object.
    pub media_scale: bool,
    pub dim_successive: bool,
    /// Smart Fill: source 0 default, 1 specify, 2 none.
    pub sf_fill_src: u8,
    pub sf_fill: vector_core::Color,
    pub sf_outline_src: u8,
    pub sf_outline_w: f64,
    pub sf_outline: vector_core::Color,
    pub text_opentype: bool,
    pub anchor_auto: bool,
    /// Effect presets: effect name -> (preset name, settings as JSON).
    pub presets: std::collections::BTreeMap<String, Vec<(String, String)>>,
    /// The preset chosen in each list.
    pub preset_sel: std::collections::BTreeMap<String, String>,
    /// Impact tool.
    pub impact_parallel: bool,
    pub impact_outer: bool,
    pub impact_angle: f64,
    /// Start and end points: 0 aligned, 1 random start, 2 random end, 3 random start and end.
    pub impact_ends: u8,
    pub impact_width: [f64; 2],
    pub impact_width_steps: u32,
    pub impact_width_random: bool,
    pub impact_spacing: [f64; 2],
    pub impact_spacing_steps: u32,
    pub impact_spacing_random: bool,
    /// Line style: 0 wedge, 1 tapered both ends, 2 tapered start, 3 straight.
    pub impact_style: u8,
    pub impact_widest: f64,
    /// Variable Outline: lock the ratio of the sides; scale the widths with the object.
    pub vo_lock: bool,
    pub vo_scale: bool,
    /// Dimensions drawn next: show units, leading zero, dynamic, arrowheads, extension lines.
    pub dim_units: bool,
    pub dim_leading_zero: bool,
    pub dim_dynamic: bool,
    pub dim_arrow: Option<vector_core::style::ArrowHead>,
    pub dim_ext_gap: f64,
    pub dim_ext_len: Option<f64>,
    /// Callouts drawn next: shape around the text and gap.
    pub callout_shape: u8,
    pub callout_gap: f64,
    /// Anchor Editing: the anchor position is shown relative to the object.
    pub anchor_relative: bool,
    /// Painterly Brush: favorite brush styles.
    pub paint_favorites: Vec<usize>,
    /// Autofit page: the margin around the objects.
    pub autofit_margin: f64,
}

impl Default for ToolOpts {
    fn default() -> Self {
        ToolOpts {
            line2_mode: 0,
            knife_mode: 0,
            knife_auto_close: true,
            knife_span: 0,
            knife_width: 0.0,
            knife_outline: 0,
            eye_area: 0,
            eye_props: [true; 3],
            eye_xforms: [false; 3],
            eye_effects: [true; 9],
            ft_mode: 2,
            ft_duplicate: false,
            ft_relative: false,
            env_mode: 3,
            env_sel_mode: 0,
            env_mapping: 1,
            env_keep_lines: false,
            mesh_sel_mode: 0,
            twirl_ccw: false,
            smear_smooth: true,
            repel: false,
            pen_pressure: false,
            rough_pressure: false,
            rough_freq: 1.0,
            rough_dryout: 0.0,
            rough_use_tilt: false,
            rough_tilt: 45.0,
            rough_fixed: false,
            rough_bearing: 0.0,
            smudge_pressure: false,
            smudge_dryout: 0.0,
            smudge_use_tilt: false,
            smudge_tilt: 45.0,
            smudge_use_bearing: false,
            smudge_bearing: 0.0,
            eraser_square: false,
            eraser_pressure: true,
            eraser_use_tilt: true,
            eraser_tilt: 90.0,
            eraser_use_bearing: true,
            eraser_bearing: 0.0,
            pen_preview: true,
            pen_auto_add: true,
            polyline_auto_close: false,
            reflect_h: false,
            reflect_v: false,
            ls_timer: 1.0,
            ls_include: true,
            ls_distance: 5.0,
            ls_single: false,
            ls_smoothing: 50.0,
            ls_preview: true,
            paint_brush: 0,
            paint_recent: vec![],
            paint_alpha: 0.0,
            paint_smoothing: 0.0,
            paint_texture: 0,
            paint_texture_strength: 50.0,
            paint_density: 50.0,
            paint_tilt: 90.0,
            paint_tilt_dir: 0.0,
            paint_glow: false,
            paint_taper: false,
            media_scale: false,
            dim_successive: false,
            sf_fill_src: 0,
            sf_fill: vector_core::Color::rgb(0x00, 0x00, 0x00),
            sf_outline_src: 0,
            sf_outline_w: 0.5669,
            sf_outline: vector_core::Color::BLACK,
            text_opentype: true,
            anchor_auto: true,
            presets: Default::default(),
            preset_sel: Default::default(),
            impact_parallel: false,
            impact_outer: false,
            impact_angle: 0.0,
            impact_ends: 0,
            impact_width: [0.5, 0.5],
            impact_width_steps: 0,
            impact_width_random: false,
            impact_spacing: [5.0, 5.0],
            impact_spacing_steps: 0,
            impact_spacing_random: false,
            impact_style: 0,
            impact_widest: 85.0,
            vo_lock: false,
            vo_scale: false,
            dim_units: true,
            dim_leading_zero: true,
            dim_dynamic: true,
            dim_arrow: Some(vector_core::style::ArrowHead::OpenTriangle),
            dim_ext_gap: 0.0,
            dim_ext_len: None,
            callout_shape: 0,
            callout_gap: 2.0,
            anchor_relative: false,
            paint_favorites: vec![],
            autofit_margin: 0.0,
        }
    }
}
