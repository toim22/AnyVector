//! Assets docker: the drawings and images of local and network folders (Local and network
//! assets) and the assets linked to the document (Linked assets), with search, sorting,
//! categories (the subfolders), thumbnails and importing.

use super::{drop_mark, icon_button, icon_button_resp, separator};
use crate::app::AnyVectorApp;
use crate::menus::item;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct AsUi {
    /// 0 Local and network assets, 2 Linked assets (1, the cloud, is not available).
    pub source: u8,
    pub search: String,
    /// 0 Favorites, 1 Recent, 2 Name, 3 Date Created, 4 Date Modified.
    pub sort: usize,
    /// The category shown: None all, Some("") uncategorized.
    pub category: Option<String>,
    pub vectors: bool,
    pub rasters: bool,
    /// The folder tree is shown.
    pub tree: bool,
    /// The folder browsed (one of the added folders or below).
    pub folder: Option<PathBuf>,
    pub sel: Option<PathBuf>,
    /// The thumbnails, by path (None: no preview).
    pub thumbs: HashMap<PathBuf, Option<egui::TextureHandle>>,
}

impl Default for AsUi {
    fn default() -> Self {
        AsUi { source: 0, search: String::new(), sort: 2, category: None, vectors: true, rasters: true, tree: false, folder: None, sel: None, thumbs: HashMap::new() }
    }
}

const SORTS: [&str; 5] = ["Favorites", "Recent", "Name", "Date Created", "Date Modified"];
const VECTOR_EXTS: [&str; 7] = ["cdr", "cdt", "vct", "svg", "svgz", "pdf", "ai"];

fn is_vector(p: &Path) -> bool {
    VECTOR_EXTS.contains(&vector_io::extension(p).as_str())
}

fn is_raster(p: &Path) -> bool {
    vector_io::image_io::is_image_ext(&vector_io::extension(p))
}

/// The default folder of local assets.
pub fn default_folder() -> PathBuf {
    let d = crate::settings::config_dir().join("Assets");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn folders(app: &AnyVectorApp) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = app.settings.asset_folders.iter().map(PathBuf::from).collect();
    if v.is_empty() {
        v.push(default_folder());
    }
    v
}

/// The assets under `dir` (recursively when asked), with their category (the subfolder they are
/// in, "" at the top).
fn scan(dir: &Path, recursive: bool, out: &mut Vec<(PathBuf, String)>, cat: &str, depth: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if recursive && depth < 6 {
                let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                let c = if cat.is_empty() { name } else { cat.to_string() };
                scan(&p, recursive, out, &c, depth + 1);
            }
        } else if is_vector(&p) || is_raster(&p) {
            out.push((p, cat.to_string()));
        }
    }
}

fn thumbnail(ctx: &egui::Context, p: &Path) -> Option<egui::TextureHandle> {
    let ext = vector_io::extension(p);
    let (w, h, rgba) = if ["cdr", "cdt", "vct"].contains(&ext.as_str()) {
        vector_io::read_thumbnail(p)?
    } else if is_raster(p) {
        let img = image::open(p).ok()?.thumbnail(160, 160).to_rgba8();
        (img.width(), img.height(), img.into_raw())
    } else if ext == "svg" || ext == "svgz" {
        let doc = vector_io::open(p).ok()?;
        let page = doc.pages.first()?;
        let dpi = (160.0 * 72.0 / page.width.max(page.height).max(1.0)).clamp(4.0, 300.0);
        let pm = vector_render::render_page_image(&doc, 0, dpi, Some(vector_core::Color::WHITE), None, true)?;
        let img = image::RgbaImage::from_raw(pm.width(), pm.height(), pm.data().to_vec())?;
        (img.width(), img.height(), img.into_raw())
    } else {
        return None;
    };
    Some(ctx.load_texture(format!("asset:{}", p.display()), egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba), egui::TextureOptions::LINEAR))
}

fn created(p: &Path) -> std::time::SystemTime {
    std::fs::metadata(p).and_then(|m| m.created()).unwrap_or(std::time::UNIX_EPOCH)
}

fn modified(p: &Path) -> std::time::SystemTime {
    std::fs::metadata(p).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH)
}

pub fn assets(app: &mut AnyVectorApp, ui: &mut Ui) {
    let w = ui.available_width() - 4.0;
    let mut run: Option<&str> = None;
    // search, Get More and the options
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.dockers.ui.assets.search).hint_text("Search").desired_width(w - 58.0));
        icon_button(app, ui, "as_store", "Get More", false);
        let gear = icon_button_resp(app, ui, "gear", "Options", true);
        drop_mark(ui, gear.rect);
        egui::Popup::menu(&gear).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            crate::menus::menu_style(ui);
            ui.set_min_width(190.0);
            let s = &mut app.settings;
            if ui.add(egui::Button::selectable(s.asset_single_folder, "Single-Folder Selection Mode")).clicked() {
                s.asset_single_folder = !s.asset_single_folder;
            }
            if ui.add(egui::Button::selectable(s.asset_recursive, "Browse Recursively")).clicked() {
                s.asset_recursive = !s.asset_recursive;
                app.dockers.ui.assets.thumbs.clear();
            }
            separator(ui);
            if item(app, ui, "Add Folder", true, false) {
                run = Some("add");
            }
            if item(app, ui, "Browse", true, false) {
                run = Some("browse");
            }
            let folder = app.dockers.ui.assets.folder.is_some();
            if item(app, ui, "Reindex Folder", folder, false) {
                app.dockers.ui.assets.thumbs.clear();
            }
            if folder && app.dockers.ui.assets.folder.as_ref().is_some_and(|f| app.settings.asset_folders.iter().any(|x| Path::new(x) == f)) && item(app, ui, "Remove from List", true, false) {
                run = Some("remove");
            }
            separator(ui);
            if item(app, ui, "Help", true, false) {
                app.run(crate::commands::Cmd::ProductHelp);
            }
        });
    });
    // the sources
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let src = app.dockers.ui.assets.source;
        for (k, icon, tip, on) in [(0u8, "as_local", "Local and network assets", true), (1, "as_cloud", "Cloud assets", false), (2, "as_linked", "Linked assets", true)] {
            if crate::widgets::icon_button(&mut app.icons, ui, icon, tip, on, src == k).clicked() && on {
                app.dockers.ui.assets.source = k;
            }
        }
        if src == 2 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(app, ui, "refresh", "Refresh List", true) {
                    app.dockers.ui.assets.thumbs.clear();
                }
            });
        }
    });
    if app.dockers.ui.assets.source == 2 {
        // Linked assets: the files the document links to
        let links: Vec<String> = app.doc().map(|d| d.doc.links.values().cloned().collect()).unwrap_or_default();
        let mut links: Vec<String> = links.into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        links.retain(|l| l.to_lowercase().contains(&app.dockers.ui.assets.search.to_lowercase()));
        sort_combo(app, ui, w);
        if links.is_empty() {
            egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).show(ui, |ui| {
                ui.set_min_size(vec2(w, (ui.available_height() - 6.0).max(80.0)));
                ui.vertical_centered(|ui| {
                    ui.add_space((ui.available_height() / 2.0 - 60.0).max(10.0));
                    ui.label(egui::RichText::new("No symbol libraries are linked to this document.").strong());
                    ui.add_space(14.0);
                    ui.add(egui::Label::new("To link a symbol library, click the Local and network assets button or the Cloud assets button, right-click a symbol library, and click Link to document.").wrap());
                });
            });
        } else {
            let items: Vec<(PathBuf, String)> = links.iter().map(|l| (PathBuf::from(l), String::new())).collect();
            asset_list(app, ui, &items, &mut run);
        }
    } else {
        sort_combo(app, ui, w);
        // the category and the content filters
        let roots = folders(app);
        let recursive = app.settings.asset_recursive;
        let st = &app.dockers.ui.assets;
        let browse: Vec<PathBuf> = match &st.folder {
            Some(f) if app.settings.asset_single_folder => vec![f.clone()],
            _ => roots.clone(),
        };
        let mut all = vec![];
        for r in &browse {
            scan(r, recursive, &mut all, "", 0);
        }
        let mut cats: Vec<String> = all.iter().map(|x| x.1.clone()).filter(|c| !c.is_empty()).collect();
        cats.sort();
        cats.dedup();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let cur = app.dockers.ui.assets.category.clone();
            let text = match &cur {
                None => "All categories".to_string(),
                Some(c) if c.is_empty() => "Uncategorized".to_string(),
                Some(c) => c.clone(),
            };
            egui::ComboBox::from_id_salt("as_cat").width(w - 84.0).selected_text(text).show_ui(ui, |ui| {
                if ui.add(egui::Button::selectable(cur.is_none(), "All categories")).clicked() {
                    app.dockers.ui.assets.category = None;
                }
                ui.separator();
                for c in &cats {
                    if ui.add(egui::Button::selectable(cur.as_deref() == Some(c.as_str()), c)).clicked() {
                        app.dockers.ui.assets.category = Some(c.clone());
                    }
                }
                ui.separator();
                if ui.add(egui::Button::selectable(cur.as_deref() == Some(""), "Uncategorized")).clicked() {
                    app.dockers.ui.assets.category = Some(String::new());
                }
            })
            .response
            .on_hover_text("Filter content");
            let st = &mut app.dockers.ui.assets;
            let (v, r, t) = (st.vectors, st.rasters, st.tree);
            if crate::widgets::icon_button(&mut app.icons, ui, "as_vectors", "Show vector images", true, v).clicked() {
                app.dockers.ui.assets.vectors = !v;
            }
            if crate::widgets::icon_button(&mut app.icons, ui, "as_rasters", "Show raster images", true, r).clicked() {
                app.dockers.ui.assets.rasters = !r;
            }
            if crate::widgets::icon_button(&mut app.icons, ui, "open", "Browse folders to choose a content source.", true, t).clicked() {
                app.dockers.ui.assets.tree = !t;
            }
        });
        // the folder tree
        if app.dockers.ui.assets.tree {
            egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).show(ui, |ui| {
                ui.set_width(w);
                egui::ScrollArea::vertical().id_salt("as_tree").max_height(120.0).auto_shrink([false, true]).show(ui, |ui| {
                    for r in &roots {
                        folder_row(app, ui, r, 0);
                    }
                });
            });
        }
        let st = &app.dockers.ui.assets;
        let q = st.search.trim().to_lowercase();
        let mut items: Vec<(PathBuf, String)> = all
            .into_iter()
            .filter(|(p, c)| {
                let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                (q.is_empty() || name.contains(&q) || c.to_lowercase().contains(&q))
                    && (st.vectors || !is_vector(p))
                    && (st.rasters || !is_raster(p))
                    && st.category.as_ref().is_none_or(|k| k == c)
            })
            .collect();
        let fav = &app.settings.asset_favorites;
        let recent = &app.settings.asset_recent;
        match st.sort {
            0 => items.retain(|(p, _)| fav.iter().any(|f| Path::new(f) == p)),
            1 => {
                items.retain(|(p, _)| recent.iter().any(|f| Path::new(f) == p));
                items.sort_by_key(|(p, _)| recent.iter().position(|f| Path::new(f) == p));
            }
            3 => items.sort_by_key(|(p, _)| std::cmp::Reverse(created(p))),
            4 => items.sort_by_key(|(p, _)| std::cmp::Reverse(modified(p))),
            _ => items.sort_by_key(|(p, _)| p.file_name().unwrap_or_default().to_string_lossy().to_lowercase()),
        }
        asset_list(app, ui, &items, &mut run);
    }
    if let Some(r) = run {
        act(app, r);
    }
}

fn sort_combo(app: &mut AnyVectorApp, ui: &mut Ui, w: f32) {
    let cur = app.dockers.ui.assets.sort;
    egui::ComboBox::from_id_salt("as_sort").width(w).selected_text(SORTS[cur]).show_ui(ui, |ui| {
        for (k, t) in SORTS.iter().enumerate() {
            if k == 2 {
                ui.separator();
            }
            if ui.add(egui::Button::selectable(k == cur, *t)).clicked() {
                app.dockers.ui.assets.sort = k;
            }
        }
    })
    .response
    .on_hover_text("Sort assets by name or date.");
}

fn folder_row(app: &mut AnyVectorApp, ui: &mut Ui, dir: &Path, depth: usize) {
    let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| dir.display().to_string());
    let sel = app.dockers.ui.assets.folder.as_deref() == Some(dir);
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
    if sel {
        ui.painter().rect_filled(r, 0.0, theme::ACCENT);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 0.0, theme::HOVER);
    }
    let x = r.left() + 6.0 + depth as f32 * 14.0;
    if let Some(t) = app.icons.get(ui.ctx(), "open", 14.0, crate::icons::Tone::Normal) {
        ui.painter().image(t.id, Rect::from_center_size(pos2(x + 7.0, r.center().y), vec2(14.0, 14.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    ui.painter().text(pos2(x + 18.0, r.center().y), egui::Align2::LEFT_CENTER, &name, egui::FontId::proportional(12.5), theme::TEXT);
    if resp.on_hover_text(dir.display().to_string()).clicked() {
        app.dockers.ui.assets.folder = if sel { None } else { Some(dir.to_path_buf()) };
        app.dockers.ui.assets.category = None;
    }
    if depth < 3 {
        let mut subs: Vec<PathBuf> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
        subs.sort();
        for s in subs {
            folder_row(app, ui, &s, depth + 1);
        }
    }
}

fn asset_list(app: &mut AnyVectorApp, ui: &mut Ui, items: &[(PathBuf, String)], run: &mut Option<&'static str>) {
    let size = app.settings.asset_thumb.clamp(32.0, 160.0);
    let list = !app.settings.asset_icon_view;
    let h = ui.available_height() - 30.0;
    let ctx = ui.ctx().clone();
    let mut loaded = 0;
    egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).show(ui, |ui| {
        ui.set_min_height(h.max(60.0));
        egui::ScrollArea::vertical().id_salt("as_list").max_height(h.max(60.0)).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let full = ui.available_width();
            let per = if list { 1 } else { ((full / (size + 14.0)).floor() as usize).max(1) };
            for row in items.chunks(per) {
                ui.horizontal(|ui| {
                    for (p, _) in row {
                        let cell = if list { vec2(full, size + 12.0) } else { vec2(full / per as f32, size + 30.0) };
                        let (r, resp) = ui.allocate_exact_size(cell, Sense::click_and_drag());
                        let visible = ui.is_rect_visible(r);
                        let sel = app.dockers.ui.assets.sel.as_deref() == Some(p.as_path());
                        let pt = ui.painter();
                        if sel {
                            pt.rect_filled(r, 0.0, theme::ACCENT);
                        } else if resp.hovered() {
                            pt.rect_filled(r, 0.0, theme::HOVER);
                        }
                        if list {
                            pt.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, theme::SEP));
                        }
                        let tr = if list { Rect::from_min_size(pos2(r.left() + 16.0, r.top() + 6.0), vec2(size, size)) } else { Rect::from_min_size(pos2(r.center().x - size / 2.0, r.top() + 6.0), vec2(size, size)) };
                        if visible && !app.dockers.ui.assets.thumbs.contains_key(p) && loaded < 3 {
                            loaded += 1;
                            let t = thumbnail(&ctx, p);
                            app.dockers.ui.assets.thumbs.insert(p.clone(), t);
                        }
                        if let Some(Some(t)) = app.dockers.ui.assets.thumbs.get(p) {
                            let ts = t.size_vec2();
                            let k = (size / ts.x).min(size / ts.y);
                            let ir = Rect::from_center_size(tr.center(), ts * k);
                            ui.painter().image(t.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                        }
                        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let fav = app.settings.asset_favorites.iter().any(|f| Path::new(f) == p);
                        let label = if fav { format!("★ {name}") } else { name.clone() };
                        if list {
                            ui.painter().text(pos2(tr.right() + 24.0, r.center().y), egui::Align2::LEFT_CENTER, &label, egui::FontId::proportional(12.5), theme::TEXT);
                        } else {
                            ui.painter().text(pos2(r.center().x, r.bottom() - 10.0), egui::Align2::CENTER_CENTER, &label, egui::FontId::proportional(11.5), theme::TEXT);
                        }
                        let resp = resp.on_hover_text(p.display().to_string());
                        if resp.clicked() {
                            app.dockers.ui.assets.sel = Some(p.clone());
                        }
                        if resp.double_clicked() {
                            app.dockers.ui.assets.sel = Some(p.clone());
                            *run = Some("import");
                        }
                        resp.context_menu(|ui| {
                            crate::menus::menu_style(ui);
                            app.dockers.ui.assets.sel = Some(p.clone());
                            if item(app, ui, "Import", true, false) {
                                *run = Some("import");
                            }
                            if item(app, ui, "Open", true, false) {
                                *run = Some("open");
                            }
                            if item(app, ui, if fav { "Remove from Favorites" } else { "Add to Favorites" }, true, false) {
                                *run = Some("fav");
                            }
                            if item(app, ui, "Open Folder Location", true, false) {
                                *run = Some("location");
                            }
                        });
                    }
                });
            }
        });
    });
    // the bottom bar
    let has = app.dockers.ui.assets.sel.is_some();
    let raster = app.dockers.ui.assets.sel.as_deref().is_some_and(is_raster);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let imp = icon_button_resp(app, ui, "as_import", "Import", has);
        drop_mark(ui, imp.rect);
        if has {
            egui::Popup::menu(&imp).show(|ui| {
                crate::menus::menu_style(ui);
                if item(app, ui, "Link to document", raster, false) {
                    *run = Some("link");
                }
                separator(ui);
                if item(app, ui, "Import and link", raster, false) {
                    *run = Some("link");
                }
                if item(app, ui, "Import at original position", true, false) {
                    *run = Some("import_orig");
                }
                if item(app, ui, "Import", true, false) {
                    *run = Some("import");
                }
            });
        }
        if icon_button(app, ui, "as_properties", "Properties...", has) {
            *run = Some("properties");
        }
        if icon_button(app, ui, "pm_edit", "Edit...", has) {
            *run = Some("edit");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut v = app.settings.asset_thumb;
            if ui.add(egui::Slider::new(&mut v, 32.0..=160.0).show_value(false)).on_hover_text("Thumbnail size").changed() {
                app.settings.asset_thumb = v;
            }
            let icon = app.settings.asset_icon_view;
            if crate::widgets::icon_button(&mut app.icons, ui, "as_view", "Switch between list view and thumbnail preview.", true, false).clicked() {
                app.settings.asset_icon_view = !icon;
            }
        });
    });
}

fn act(app: &mut AnyVectorApp, r: &str) {
    let sel = app.dockers.ui.assets.sel.clone();
    let remember = |app: &mut AnyVectorApp, p: &Path| {
        let s = p.to_string_lossy().to_string();
        app.settings.asset_recent.retain(|x| *x != s);
        app.settings.asset_recent.insert(0, s);
        app.settings.asset_recent.truncate(30);
    };
    match r {
        "add" | "browse" => {
            if let Some(d) = rfd::FileDialog::new().set_title(if r == "add" { "Add Folder" } else { "Browse" }).pick_folder() {
                let s = d.to_string_lossy().to_string();
                if r == "add" {
                    if app.settings.asset_folders.is_empty() {
                        app.settings.asset_folders.push(default_folder().to_string_lossy().to_string());
                    }
                    if !app.settings.asset_folders.contains(&s) {
                        app.settings.asset_folders.push(s);
                    }
                }
                app.dockers.ui.assets.folder = Some(d);
                app.dockers.ui.assets.tree = true;
            }
        }
        "remove" => {
            if let Some(f) = app.dockers.ui.assets.folder.take() {
                app.settings.asset_folders.retain(|x| Path::new(x) != f);
            }
        }
        "import" | "import_orig" | "link" | "open" => {
            let Some(p) = sel else { return };
            remember(app, &p);
            match r {
                "open" => {
                    app.open_path(&p);
                }
                "import_orig" => {
                    if !app.has_doc() {
                        app.new_default_document();
                    }
                    if let Some(d) = app.doc_mut() {
                        match vector_io::import(&mut d.doc, &p) {
                            Ok(objs) if !objs.is_empty() => d.add_objects("Import", objs),
                            Ok(_) => {}
                            Err(e) => app.dialogs.message_box = Some(("Import".into(), format!("Could not import {}:\n{e}", p.display()))),
                        }
                    }
                }
                "link" => {
                    let before: Vec<u64> = app.doc().map(|d| d.doc.images.keys().copied().collect()).unwrap_or_default();
                    app.import_path(&p, None);
                    if let Some(d) = app.doc_mut() {
                        let new: Vec<u64> = d.doc.images.keys().filter(|k| !before.contains(k)).copied().collect();
                        for k in new {
                            d.doc.links.insert(k, p.to_string_lossy().to_string());
                        }
                    }
                }
                _ => app.import_path(&p, None),
            }
        }
        "fav" => {
            let Some(p) = sel else { return };
            let s = p.to_string_lossy().to_string();
            if app.settings.asset_favorites.contains(&s) {
                app.settings.asset_favorites.retain(|x| *x != s);
            } else {
                app.settings.asset_favorites.push(s);
            }
        }
        "location" => {
            if let Some(p) = sel {
                let _ = std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn();
            }
        }
        "properties" => {
            if let Some(p) = sel {
                let m = std::fs::metadata(&p).ok();
                let size = m.as_ref().map(|m| m.len()).unwrap_or(0);
                let when = crate::settings::format_time;
                let text = format!(
                    "Name: {}\nFolder: {}\nSize: {} KB\nCreated: {}\nModified: {}",
                    p.file_name().unwrap_or_default().to_string_lossy(),
                    p.parent().map(|x| x.display().to_string()).unwrap_or_default(),
                    size.div_ceil(1024),
                    when(created(&p)),
                    when(modified(&p)),
                );
                app.dialogs.message_box = Some(("Properties".into(), text));
            }
        }
        "edit" => {
            if let Some(p) = sel {
                if is_raster(&p) {
                    let _ = std::process::Command::new("cmd").args(["/C", "start", "", &p.to_string_lossy()]).spawn();
                } else {
                    app.open_path(&p);
                }
            }
        }
        _ => {}
    }
}
