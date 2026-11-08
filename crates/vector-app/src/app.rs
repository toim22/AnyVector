//! Main application: window layout and global state.

use crate::commands::Cmd;
use crate::dialogs::Dialogs;
use crate::dockers::{DockerId, DockerState};
use crate::docview::DocView;
use crate::icons::Icons;
use crate::settings::Settings;
use crate::theme;
use crate::tools::{ToolId, ToolState};
use egui::{Color32, Frame, Margin};
use vector_core::{Color, Object};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum Toolbar {
    MenuBar,
    StatusBar,
    Standard,
    PropertyBar,
    Toolbox,
    Zoom,
    Text,
    Layout,
    Transform,
    Macros,
    Internet,
    ProjectTimer,
    ParallelDrawing,
}

impl Toolbar {
    pub const EXTRA: [Toolbar; 8] = [Toolbar::Zoom, Toolbar::Text, Toolbar::Layout, Toolbar::Transform, Toolbar::Macros, Toolbar::Internet, Toolbar::ProjectTimer, Toolbar::ParallelDrawing];
    pub fn name(self) -> &'static str {
        match self {
            Toolbar::MenuBar => "Menu bar",
            Toolbar::StatusBar => "Status bar",
            Toolbar::Standard => "Standard",
            Toolbar::PropertyBar => "Property bar",
            Toolbar::Toolbox => "Toolbox",
            Toolbar::Zoom => "Zoom",
            Toolbar::Text => "Text",
            Toolbar::Layout => "Layout",
            Toolbar::Transform => "Transform",
            Toolbar::Macros => "Macros",
            Toolbar::Internet => "Internet",
            Toolbar::ProjectTimer => "Project timer",
            Toolbar::ParallelDrawing => "Parallel Drawing",
        }
    }
}

/// Clipboard contents (internal; also mirrored to the system clipboard as SVG/PNG when possible).
#[derive(Clone, Default)]
pub struct Clip {
    pub objects: Vec<Object>,
    pub images: std::collections::BTreeMap<u64, vector_core::document::ImageData>,
    pub symbols: Vec<vector_core::document::SymbolDef>,
}

pub struct Message {
    pub text: String,
    pub until: f64,
}

pub struct VectorApp {
    pub docs: Vec<DocView>,
    /// Index into `docs`; `None` = the Home (Welcome screen) tab.
    pub active: Option<usize>,
    pub settings: Settings,
    pub icons: Icons,
    pub tool: ToolId,
    /// Remembered tool per toolbox group (flyout selection).
    pub group_tool: Vec<ToolId>,
    pub ts: ToolState,
    pub dockers: DockerState,
    pub dialogs: Dialogs,
    pub clip: Clip,
    pub pending: Vec<Cmd>,
    pub message: Option<Message>,
    pub untitled_counter: usize,
    pub fullscreen_preview: bool,
    pub quit_requested: bool,
    pub allow_close: bool,
    pub last_cmd: Option<Cmd>,
    pub recording: Option<crate::scripting::Recorder>,
    pub temp_script: String,
    pub default_fill: Color,
    pub start_time: std::time::Instant,
    pub splash: Option<crate::splash::Splash>,
    pub thumbnails: std::collections::HashMap<std::path::PathBuf, Option<egui::TextureHandle>>,
    pub palettes: crate::palette::PaletteState,
    pub status_hover: String,
    pub cursor_doc: Option<kurbo::Point>,
    pub ai: crate::ai::AiState,
    pub frame_count: u64,
}

impl VectorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> VectorApp {
        theme::apply(&cc.egui_ctx);
        egui_extras_install(&cc.egui_ctx);
        let settings = Settings::load();
        cc.egui_ctx.set_zoom_factor((settings.ui_scale / 100.0) as f32);
        // warm the font database in the background (first text layout is otherwise slow)
        std::thread::spawn(vector_core::text::preload_fonts);
        let group_tool = crate::tools::GROUPS.iter().map(|(_, t)| t[0]).collect();
        let mut app = VectorApp {
            docs: vec![],
            active: None,
            dockers: DockerState::from_settings(&settings),
            palettes: crate::palette::PaletteState::new(&settings),
            settings,
            icons: Icons::default(),
            tool: ToolId::Pick,
            group_tool,
            ts: ToolState::default(),
            dialogs: Dialogs::default(),
            clip: Clip::default(),
            pending: vec![],
            message: None,
            untitled_counter: 0,
            fullscreen_preview: false,
            quit_requested: false,
            allow_close: false,
            last_cmd: None,
            recording: None,
            temp_script: String::new(),
            default_fill: Color::BLACK,
            start_time: std::time::Instant::now(),
            splash: Some(crate::splash::Splash::new()),
            thumbnails: Default::default(),
            status_hover: String::new(),
            cursor_doc: None,
            ai: Default::default(),
            frame_count: 0,
        };
        // command line: open files
        for a in std::env::args().skip(1) {
            let p = std::path::PathBuf::from(&a);
            if p.exists() {
                app.open_path(&p);
            }
        }
        if app.docs.is_empty() {
            match app.settings.startup {
                crate::settings::StartupAction::NewDocument => app.pending.push(Cmd::New),
                crate::settings::StartupAction::OpenDocument => app.pending.push(Cmd::Open),
                crate::settings::StartupAction::OpenLastEdited => {
                    if let Some(p) = app.settings.recent_files.first().cloned() {
                        app.open_path(&p);
                    }
                }
                _ => {}
            }
        }
        app
    }

    pub fn doc(&self) -> Option<&DocView> {
        self.active.and_then(|i| self.docs.get(i))
    }

    pub fn doc_mut(&mut self) -> Option<&mut DocView> {
        self.active.and_then(move |i| self.docs.get_mut(i))
    }

    pub fn has_doc(&self) -> bool {
        self.doc().is_some()
    }

    pub fn has_selection(&self) -> bool {
        self.doc().map(|d| !d.selection.is_empty()).unwrap_or(false)
    }

    pub fn time(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    pub fn flash(&mut self, text: impl Into<String>) {
        let until = self.time() + 4.0;
        self.message = Some(Message { text: text.into(), until });
    }

    pub fn run(&mut self, cmd: Cmd) {
        self.pending.push(cmd);
    }

    pub fn set_tool(&mut self, t: ToolId) {
        if self.tool != t {
            self.finish_tool();
        }
        self.tool = t;
        let g = crate::tools::group_of(t);
        self.group_tool[g] = t;
        if let Some(d) = self.doc_mut() {
            if t != ToolId::Shape {
                d.node_sel.clear();
            }
            if t != ToolId::Text {
                d.text_edit = None;
            }
        }
    }

    /// Finish any in-progress tool operation (e.g. a Bézier curve being drawn).
    pub fn finish_tool(&mut self) {
        let ts = std::mem::take(&mut self.ts);
        let mut ts = ts;
        if let Some(i) = self.active {
            if let Some(d) = self.docs.get_mut(i) {
                crate::tools::finish(&mut ts, d, &self.settings);
            }
        }
        self.ts = ts;
    }

    fn ui_impl(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.frame_count += 1;
        if self.splash.is_some() {
            if crate::splash::show(self, ui) {
                return;
            }
        }
        self.handle_dropped_files(&ctx);
        crate::shortcuts::handle(self, &ctx);
        self.process_pending(&ctx);
        crate::ai::poll(self);

        if self.fullscreen_preview {
            crate::canvas::fullscreen_preview(self, ui);
            return;
        }

        let bar = Frame::new().fill(theme::BAR).inner_margin(Margin::symmetric(2, 0));
        if self.settings.show_menu_bar {
            egui::Panel::top("menubar").frame(Frame::new().fill(theme::BAR_DARK).inner_margin(Margin::symmetric(4, 0))).exact_size(20.0).show(ui, |ui| {
                crate::menus::menu_bar(self, ui);
            });
        }
        if self.settings.show_standard {
            egui::Panel::top("standard").frame(bar.stroke(egui::Stroke::NONE)).exact_size(34.0).show(ui, |ui| {
                crate::toolbar::standard(self, ui);
            });
        }
        for tb in Toolbar::EXTRA {
            if self.settings.extra_toolbars.iter().any(|s| s == tb.name()) {
                egui::Panel::top(format!("tb_{}", tb.name())).frame(bar).exact_size(32.0).show(ui, |ui| {
                    crate::toolbar::extra(self, ui, tb);
                });
            }
        }
        if self.settings.show_property_bar {
            egui::Panel::top("propbar").frame(bar).exact_size(44.0).show(ui, |ui| {
                crate::propbar::property_bar(self, ui);
            });
        }
        if self.settings.show_status_bar {
            egui::Panel::bottom("status").frame(Frame::new().fill(theme::BAR).inner_margin(Margin::symmetric(4, 0))).exact_size(30.0).show(ui, |ui| {
                crate::statusbar::status_bar(self, ui);
            });
        }
        if self.settings.show_toolbox {
            egui::Panel::left("toolbox").frame(Frame::new().fill(theme::BAR).inner_margin(Margin { left: 2, right: 0, top: 4, bottom: 4 })).exact_size(33.0).show(ui, |ui| {
                crate::toolbox::toolbox(self, ui);
            });
        }
        if self.settings.default_palette_visible {
            egui::Panel::right("palette").frame(Frame::new().fill(theme::BAR).inner_margin(Margin::symmetric(1, 2))).exact_size(crate::palette::column_width(&self.settings)).show(ui, |ui| {
                crate::palette::palette_column(self, ui);
            });
        }
        if self.dockers.visible() {
            let w = self.settings.docker_width;
            let resp = egui::Panel::right("dockers").frame(Frame::new().fill(theme::BAR).inner_margin(Margin::same(0))).resizable(true).default_size(w).size_range(220.0..=600.0).show(ui, |ui| {
                crate::dockers::dockers_panel(self, ui);
            });
            self.settings.docker_width = resp.response.rect.width();
        }
        if self.settings.doc_palette_visible {
            egui::Panel::bottom("docpalette").frame(Frame::new().fill(theme::BG).inner_margin(Margin::symmetric(2, 0))).exact_size(24.0).show(ui, |ui| {
                crate::palette::document_palette(self, ui);
            });
        }
        egui::CentralPanel::default().frame(Frame::new().fill(theme::BG)).show(ui, |ui| {
            crate::tabs::tab_strip(self, ui);
            if self.active.is_some() {
                crate::canvas::drawing_window(self, ui);
            } else {
                crate::welcome::welcome(self, ui);
            }
        });
        crate::dialogs::show(self, &ctx);
        self.show_message(&ctx);
        self.process_pending(&ctx);
        self.autosave_tick();
        // window title
        let title = match self.doc() {
            Some(d) => format!("{} - [{}]", vector_core::APP_NAME, d.display_name()),
            None => vector_core::APP_NAME.to_string(),
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        // close request
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.docs.iter().any(|d| d.history.is_modified()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.dialogs.confirm_quit = true;
            } else {
                self.settings.save();
            }
        }
        if self.quit_requested {
            self.quit_requested = false;
            self.allow_close = true;
            self.dockers.save(&mut self.settings);
            self.settings.save();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn show_message(&mut self, ctx: &egui::Context) {
        let now = self.time();
        if let Some(m) = &self.message {
            if now > m.until {
                self.message = None;
                return;
            }
            let text = m.text.clone();
            egui::Area::new(egui::Id::unique("flash")).anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -70.0)).order(egui::Order::Tooltip).interactable(false).show(ctx, |ui| {
                Frame::new().fill(Color32::from_rgb(0x33, 0x33, 0x33)).stroke(egui::Stroke::new(1.0, theme::SEP)).corner_radius(3.0).inner_margin(Margin::symmetric(14, 8)).show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(text).color(theme::TEXT)).extend());
                });
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        for f in files {
            let p = f.path().to_path_buf();
            if !p.as_os_str().is_empty() {
                let ext = vector_io::extension(&p);
                if self.has_doc() && ext != "vct" {
                    self.import_path(&p, None);
                } else {
                    self.open_path(&p);
                }
            }
        }
    }

    pub fn process_pending(&mut self, ctx: &egui::Context) {
        let mut guard = 0;
        while !self.pending.is_empty() && guard < 64 {
            guard += 1;
            let cmds = std::mem::take(&mut self.pending);
            for c in cmds {
                self.execute(ctx, c);
            }
        }
    }

    fn autosave_tick(&mut self) {
        if !self.settings.auto_backup {
            return;
        }
        let now = self.time();
        let interval = self.settings.auto_backup_minutes.max(1) as f64 * 60.0;
        let dir = self.settings.auto_backup_dir.clone().unwrap_or_else(|| std::env::temp_dir().join("Vector Backups"));
        for d in self.docs.iter_mut() {
            if d.history.is_modified() && now - d.last_saved_backup > interval {
                d.last_saved_backup = now;
                let _ = std::fs::create_dir_all(&dir);
                let name = format!("AutoBackup_of_{}.vct", d.display_name().trim_end_matches('*').trim_end_matches(".vct"));
                let _ = vector_io::native::save(&d.doc, &dir.join(name));
            }
        }
    }
}

fn egui_extras_install(_ctx: &egui::Context) {}

impl eframe::App for VectorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui_impl(ui);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.splash.is_some() {
            return [0.0, 0.0, 0.0, 0.0];
        }
        [0.149, 0.149, 0.149, 1.0]
    }

    fn on_exit(&mut self) {
        self.dockers.save(&mut self.settings);
        self.settings.save();
    }
}

/// Whether a docker id is currently open.
pub fn docker_open(app: &VectorApp, id: DockerId) -> bool {
    app.dockers.open.contains(&id)
}
