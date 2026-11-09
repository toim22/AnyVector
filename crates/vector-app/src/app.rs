//! Main application: window layout and global state.

use crate::commands::Cmd;
use crate::dialogs::Dialogs;
use crate::dockers::DockerState;
use crate::docview::DocView;
use crate::icons::Icons;
use crate::menus::BarSpot;
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
    /// The page the Welcome Screen shows.
    pub welcome_page: crate::welcome::Page,
    pub settings: Settings,
    pub icons: Icons,
    pub tool: ToolId,
    /// What each toolbox slot shows and which kind each tool was last used as.
    pub toolbox: crate::toolbox::ToolboxState,
    pub ts: ToolState,
    pub dockers: DockerState,
    pub dialogs: Dialogs,
    pub clip: Clip,
    pub pending: Vec<Cmd>,
    pub message: Option<Message>,
    pub untitled_counter: usize,
    pub fullscreen_preview: bool,
    /// View ▸ Preview Selected Only: the full-screen preview shows the selection only.
    pub preview_selected: bool,
    /// View ▸ Page ▸ Printable Area: the default printer's paper (landscape, paper).
    pub printable_paper: Option<(bool, crate::printing::Paper)>,
    pub quit_requested: bool,
    pub allow_close: bool,
    pub last_cmd: Option<Cmd>,
    /// Object ▸ Rollover ▸ Rollover Live Preview.
    pub rollover_live: bool,
    /// The rollover under the pointer during the live preview, and the state it shows.
    pub rollover_hot: Option<(vector_core::ObjId, u8)>,
    pub recording: Option<crate::scripting::Recorder>,
    pub temp_script: String,
    pub default_fill: Color,
    pub start_time: std::time::Instant,
    /// The graphics adapter the window renders with (Help > About > System Info).
    pub gpu: String,
    pub splash: Option<crate::splash::Splash>,
    pub palettes: crate::palette::PaletteState,
    pub status_hover: String,
    pub cursor_doc: Option<kurbo::Point>,
    pub ai: crate::ai::AiState,
    pub frame_count: u64,
    /// Work running on other threads (printing, uploads): each sends one result, shown as a
    /// status message (Ok) or a message box (Err).
    pub background: Vec<std::sync::mpsc::Receiver<Result<String, String>>>,
}

/// "NVIDIA GeForce RTX 3080 (Vulkan, NVIDIA 566.36)": the adapter, its API and driver.
fn gpu_name(i: &eframe::egui_wgpu::wgpu::AdapterInfo) -> String {
    let driver = [i.driver.trim(), i.driver_info.trim()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" ");
    if driver.is_empty() {
        format!("{} ({:?})", i.name, i.backend)
    } else {
        format!("{} ({:?}, {driver})", i.name, i.backend)
    }
}

impl VectorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> VectorApp {
        theme::apply(&cc.egui_ctx);
        egui_extras_install(&cc.egui_ctx);
        let settings = Settings::load();
        cc.egui_ctx.set_zoom_factor((settings.ui_scale / 100.0) as f32);
        // warm the font database in the background (first text layout is otherwise slow)
        std::thread::spawn(vector_core::text::preload_fonts);
        let mut app = VectorApp {
            docs: vec![],
            active: None,
            welcome_page: Default::default(),
            dockers: DockerState::from_settings(&settings),
            palettes: crate::palette::PaletteState::new(&settings),
            settings,
            icons: Icons::default(),
            tool: ToolId::Pick,
            toolbox: Default::default(),
            ts: ToolState::default(),
            dialogs: Dialogs::default(),
            clip: Clip::default(),
            pending: vec![],
            message: None,
            untitled_counter: 0,
            fullscreen_preview: false,
            preview_selected: false,
            printable_paper: None,
            quit_requested: false,
            allow_close: false,
            last_cmd: None,
            rollover_live: false,
            rollover_hot: None,
            recording: None,
            temp_script: String::new(),
            default_fill: Color::BLACK,
            start_time: std::time::Instant::now(),
            splash: Some(crate::splash::Splash::new()),
            status_hover: String::new(),
            cursor_doc: None,
            ai: Default::default(),
            frame_count: 0,
            background: vec![],
            gpu: cc.wgpu_render_state.as_ref().map(|rs| gpu_name(&rs.adapter.get_info())).unwrap_or_default(),
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
        self.toolbox.remember(t);
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
        crate::dockers::coled::publish(self, &ctx);
        crate::ai::poll(self);
        self.poll_background(&ctx);
        crate::dialogs::bitmaps::poll_edits(self, &ctx);

        if self.fullscreen_preview {
            crate::canvas::fullscreen_preview(self, ui);
            return;
        }

        let bar = Frame::new().fill(theme::BAR).inner_margin(Margin::symmetric(2, 0));
        if self.settings.show_menu_bar {
            let r = egui::Panel::top("menubar").frame(Frame::new().fill(theme::BAR_DARK).inner_margin(Margin::symmetric(4, 0))).exact_size(26.0).show(ui, |ui| {
                crate::menus::menu_bar(self, ui);
            });
            crate::menus::command_bar_context(self, &r.response, BarSpot::MenuBar);
        }
        if self.settings.show_standard {
            let r = egui::Panel::top("standard").frame(bar.stroke(egui::Stroke::NONE)).exact_size(34.0).show(ui, |ui| {
                crate::toolbar::standard(self, ui);
            });
            crate::menus::command_bar_context(self, &r.response, BarSpot::Toolbar("Standard"));
        }
        // the Project Timer docks under the standard toolbar; the other optional toolbars float
        if self.settings.extra_toolbars.iter().any(|s| s == Toolbar::ProjectTimer.name()) {
            let r = egui::Panel::top("tb_project_timer").frame(bar).exact_size(32.0).show(ui, |ui| {
                crate::toolbar::extra(self, ui, Toolbar::ProjectTimer);
            });
            crate::menus::command_bar_context(self, &r.response, BarSpot::Toolbar("Project timer"));
        }
        self.floating_toolbars(ui.ctx());
        if self.settings.show_property_bar {
            let r = egui::Panel::top("propbar").frame(bar).exact_size(44.0).show(ui, |ui| {
                crate::propbar::property_bar(self, ui);
            });
            crate::menus::command_bar_context(self, &r.response, BarSpot::Toolbar("Property Bar"));
        }
        if self.settings.show_status_bar {
            let r = egui::Panel::bottom("status").frame(Frame::new().fill(theme::BAR).inner_margin(Margin::symmetric(4, 0))).exact_size(30.0).show(ui, |ui| {
                crate::statusbar::status_bar(self, ui);
            });
            crate::menus::command_bar_context(self, &r.response, BarSpot::StatusBar);
        }
        if self.settings.show_toolbox {
            let r = egui::Panel::left("toolbox").frame(Frame::new().fill(theme::BAR).inner_margin(Margin { left: 2, right: 0, top: 4, bottom: 4 })).exact_size(33.0).show(ui, |ui| {
                crate::toolbox::toolbox(self, ui);
            });
            crate::menus::command_bar_context(self, &r.response, BarSpot::Toolbar("Toolbox"));
        }
        // the open palettes: the default one at the edge, the others docked to its left
        for i in 0..self.palettes.palettes.len() {
            if i == 0 && !self.settings.default_palette_visible {
                continue;
            }
            let id = if i == 0 { "palette".to_string() } else { format!("palette{i}") };
            egui::Panel::right(id.as_str()).frame(Frame::new().fill(theme::BAR).inner_margin(Margin::symmetric(1, 2))).exact_size(crate::palette::column_width(&self.settings)).show(ui, |ui| {
                crate::palette::palette_column(self, ui, i);
            });
        }
        if self.dockers.visible() {
            let w = self.settings.docker_width;
            if self.dockers.collapsed {
                egui::Panel::right("dockers_folded").frame(Frame::new().fill(theme::BG).inner_margin(Margin::same(0))).exact_size(crate::dockers::TAB_STRIP + 2.0).show(ui, |ui| {
                    crate::dockers::dockers_panel(self, ui);
                });
            } else {
                let mut panel = egui::Panel::right("dockers").frame(Frame::new().fill(theme::BG).inner_margin(Margin::same(0))).resizable(true).default_size(w).size_range(220.0..=600.0);
                if let Some(w) = self.dockers.set_width.take() {
                    panel = panel.exact_size(w);
                }
                let resp = panel.show(ui, |ui| {
                    crate::dockers::dockers_panel(self, ui);
                });
                self.settings.docker_width = resp.response.rect.width();
            }
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

    fn poll_background(&mut self, ctx: &egui::Context) {
        if self.background.is_empty() {
            return;
        }
        let mut done = vec![];
        self.background.retain(|rx| match rx.try_recv() {
            Ok(r) => {
                done.push(r);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(_) => false,
        });
        for r in done {
            match r {
                // File ▸ Acquire Image: the scanned image to import, or the chosen WIA device
                Ok(m) if m.starts_with("\u{1}import:") => {
                    let p = std::path::PathBuf::from(&m["\u{1}import:".len()..]);
                    self.import_path(&p, None);
                    let _ = std::fs::remove_file(&p);
                }
                Ok(m) if m.starts_with("\u{1}wia:") => {
                    self.settings.wia_device = Some(m["\u{1}wia:".len()..].to_string());
                    self.flash("Scanner selected.");
                }
                Ok(m) if m.is_empty() => {}
                Ok(m) => self.flash(m),
                Err(e) => self.dialogs.message_box = Some((vector_core::APP_NAME.to_string(), e)),
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
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
                if self.has_doc() && !matches!(ext.as_str(), "cdr" | "vct") {
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
                let name = format!("AutoBackup_of_{}.cdr", d.stem());
                let _ = vector_io::cdr::save(&d.doc, &dir.join(name), &Default::default());
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
impl VectorApp {
    /// The optional toolbars that float (Window ▸ Toolbars): a small window each, with its name
    /// and a close button. Toolbars opened together start stacked one under the other.
    fn floating_toolbars(&mut self, ctx: &egui::Context) {
        let mut next = egui::pos2(300.0, 170.0);
        for tb in Toolbar::EXTRA.into_iter().filter(|t| *t != Toolbar::ProjectTimer) {
            if !self.settings.extra_toolbars.iter().any(|s| s == tb.name()) {
                continue;
            }
            let mut open = true;
            let r = egui::Window::new(tb.name())
                .id(egui::Id::unique(("floating_toolbar", tb.name())))
                .title_bar(false)
                .resizable(false)
                .default_pos(next)
                .frame(Frame::window(&ctx.global_style()).fill(theme::BAR).inner_margin(Margin::symmetric(6, 3)).corner_radius(2))
                .show(ctx, |ui| {
                    ui.set_max_width(860.0);
                    // a small title row: the name, and × at the right end once the width is known
                    let (title, _) = ui.allocate_exact_size(egui::vec2(60.0, 18.0), egui::Sense::hover());
                    crate::toolbar::extra(self, ui, tb);
                    let right = ui.min_rect().right();
                    ui.painter().text(title.left_center(), egui::Align2::LEFT_CENTER, tb.name(), egui::FontId::proportional(12.0), theme::TEXT);
                    let x = egui::Rect::from_center_size(egui::pos2(right - 7.0, title.center().y), egui::vec2(16.0, 16.0));
                    let xr = ui.interact(x, ui.scope_id().with("close"), egui::Sense::click());
                    if xr.hovered() {
                        ui.painter().rect_filled(x, 2.0, theme::HOVER);
                    }
                    let c = x.center();
                    for d in [egui::vec2(4.0, 4.0), egui::vec2(4.0, -4.0)] {
                        ui.painter().line_segment([c - d, c + d], egui::Stroke::new(1.2, theme::TEXT));
                    }
                    if xr.clicked() {
                        open = false;
                    }
                });
            if let Some(r) = r {
                next = egui::pos2(next.x, r.response.rect.bottom() + 6.0);
                crate::menus::command_bar_context(self, &r.response, BarSpot::Toolbar(tb.name()));
            }
            if !open {
                self.run(Cmd::ToggleToolbar(tb));
            }
        }
    }
}
