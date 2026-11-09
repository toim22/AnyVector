//! Create a New Document: the "Document Settings" tab (page presets grid with search, sort
//! and size filter, custom presets, and the General / Dimensions / Layout / Colors settings)
//! and the "Templates" tab.

use super::window;
use crate::app::VectorApp;
use crate::icons::Tone;
use crate::theme;
use egui::{pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder};
use std::path::PathBuf;
use vector_core::document::{ColorSettings, RenderingIntent, DEFAULT_CMYK_PROFILE, DEFAULT_GRAY_PROFILE, DEFAULT_RGB_PROFILE};
use vector_core::Unit;

// ------------------------------------------------------------------------------------------
// presets
// ------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Custom,
    Print,
    Web,
    Devices,
    Social,
}

impl Kind {
    fn icon(self) -> &'static str {
        match self {
            Kind::Custom => "preset_custom",
            Kind::Print => "preset_print",
            Kind::Web => "preset_web",
            Kind::Devices => "preset_devices",
            Kind::Social => "preset_social",
        }
    }
}

/// "All Page Sizes" filter entries, in the order of the dropdown.
pub const SIZE_FILTERS: &[&str] = &["All Page Sizes", "Custom", "Print", "Web", "Devices", "Social"];
/// "Page Type" sort entries.
pub const SORTS: &[&str] = &["Page Type", "Name", "Most Recently Used", "Largest Page Size", "Smallest Page Size"];
/// Template categories, in the order of the dropdown.
pub const CATEGORIES: &[&str] = &[
    "All Categories",
    "Book Covers",
    "Brochures",
    "Business Cards",
    "Calendars",
    "Cards Invitations",
    "Catalog & Informational Booklets",
    "Certificates",
    "Collages",
    "Flyers",
    "Infographics",
    "Letterheads & Letters",
    "Letters & Stationary",
    "Logos",
    "Menus",
    "Merchandise",
    "Other",
    "Posters",
    "Presentations",
    "Resumes",
    "Social Media and Web",
];

const RESOLUTIONS: &[f64] = &[72.0, 96.0, 150.0, 200.0, 300.0, 400.0, 600.0, 1200.0];
const RGB_PROFILES: &[&str] = &[DEFAULT_RGB_PROFILE, "Adobe RGB (1998)", "Apple RGB", "ColorMatch RGB", "ProPhoto RGB", "Display P3"];
const CMYK_PROFILES: &[&str] = &[DEFAULT_CMYK_PROFILE, "U.S. Sheetfed Coated v2", "U.S. Web Uncoated v2", "Coated FOGRA39 (ISO 12647-2:2004)", "Uncoated FOGRA29 (ISO 12647-2:2004)", "Japan Color 2001 Coated", "Euroscale Coated v2"];
const GRAY_PROFILES: &[&str] = &[DEFAULT_GRAY_PROFILE, "Dot Gain 10%", "Dot Gain 15%", "Dot Gain 25%", "Dot Gain 30%", "Gray Gamma 1.8", "Gray Gamma 2.2"];

#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    /// Size in points.
    pub width: f64,
    pub height: f64,
    pub unit: Unit,
    pub dpi: f64,
    pub cmyk: bool,
    pub kind: Kind,
    /// Index into the custom presets (settings), for Kind::Custom.
    pub custom: Option<usize>,
}

impl Preset {
    fn new(name: &str, w: f64, h: f64, unit: Unit, kind: Kind) -> Preset {
        let dpi = if unit == Unit::Pixels { 72.0 } else { 300.0 };
        Preset { name: name.into(), width: unit.from_unit(w, dpi), height: unit.from_unit(h, dpi), unit, dpi, cmyk: kind == Kind::Print, kind, custom: None }
    }

    /// "8.5 × 11 in.", "182 × 257 mm", "1024 × 768 px".
    pub fn size_text(&self) -> String {
        let f = |v: f64| {
            let v = self.unit.to_unit(v, self.dpi);
            let s = format!("{:.4}", v);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        };
        let suffix = match self.unit {
            Unit::Inches => "in.",
            Unit::Millimeters => "mm",
            Unit::Centimeters => "cm",
            Unit::Pixels => "px",
            Unit::Points => "pt",
            u => u.suffix(),
        };
        format!("{} × {} {}", f(self.width), f(self.height), suffix)
    }

    fn area(&self) -> f64 {
        self.width * self.height
    }
}

/// The built-in presets, in "Page Type" order.
pub fn builtin_presets() -> Vec<Preset> {
    use Kind::*;
    let mut v = Vec::new();
    let mut inch = |n: &str, w: f64, h: f64| v.push(Preset::new(n, w, h, Unit::Inches, Print));
    // United States and architectural sizes
    inch("Letter", 8.5, 11.0);
    inch("Legal", 8.5, 14.0);
    inch("Tabloid", 11.0, 17.0);
    inch("Ledger", 17.0, 11.0);
    inch("Executive", 7.25, 10.5);
    inch("Statement", 5.5, 8.5);
    inch("Folio", 8.5, 13.0);
    inch("Tabloid Extra", 12.0, 18.0);
    inch("Super B", 13.0, 19.0);
    inch("Arch A", 9.0, 12.0);
    inch("Arch B", 12.0, 18.0);
    inch("Arch C", 18.0, 24.0);
    inch("Arch D", 24.0, 36.0);
    inch("Arch E", 36.0, 48.0);
    let mut mm = |n: &str, w: f64, h: f64| v.push(Preset::new(n, w, h, Unit::Millimeters, Print));
    // ISO A and B series
    for (n, w, h) in [("A0", 841.0, 1189.0), ("A1", 594.0, 841.0), ("A2", 420.0, 594.0), ("A3", 297.0, 420.0), ("A4", 210.0, 297.0), ("A5", 148.0, 210.0), ("A6", 105.0, 148.0), ("A7", 74.0, 105.0), ("A8", 52.0, 74.0), ("A9", 37.0, 52.0), ("A10", 26.0, 37.0)] {
        mm(n, w, h);
    }
    for (n, w, h) in [("B0 (ISO)", 1000.0, 1414.0), ("B1 (ISO)", 707.0, 1000.0), ("B2 (ISO)", 500.0, 707.0), ("B3 (ISO)", 353.0, 500.0), ("B4 (ISO)", 250.0, 353.0), ("B5 (ISO)", 176.0, 250.0), ("B6 (ISO)", 125.0, 176.0), ("B7 (ISO)", 88.0, 125.0), ("B8 (ISO)", 62.0, 88.0), ("B9 (ISO)", 44.0, 62.0), ("B10 (ISO)", 31.0, 44.0)] {
        mm(n, w, h);
    }
    // envelopes
    for (n, w, h) in [("Envelope C4", 229.0, 324.0), ("Envelope C5", 162.0, 229.0), ("Envelope C6", 114.0, 162.0), ("Envelope DL", 110.0, 220.0)] {
        mm(n, w, h);
    }
    let mut inch = |n: &str, w: f64, h: f64| v.push(Preset::new(n, w, h, Unit::Inches, Print));
    inch("Envelope #9", 3.875, 8.875);
    inch("Envelope #10", 4.125, 9.5);
    inch("Envelope #11", 4.5, 10.375);
    inch("Envelope #12", 4.75, 11.0);
    inch("Envelope #14", 5.0, 11.5);
    inch("Envelope Monarch", 3.875, 7.5);
    inch("Business Card", 3.5, 2.0);
    inch("Index Card 3 × 5", 5.0, 3.0);
    inch("Index Card 4 × 6", 6.0, 4.0);
    inch("Index Card 5 × 8", 8.0, 5.0);
    let mut mm = |n: &str, w: f64, h: f64| v.push(Preset::new(n, w, h, Unit::Millimeters, Print));
    // JIS B series
    for (n, w, h) in [("B0 (JIS)", 1030.0, 1456.0), ("B1 (JIS)", 728.0, 1030.0), ("B2 (JIS)", 515.0, 728.0), ("B3 (JIS)", 364.0, 515.0), ("B4 (JIS)", 257.0, 364.0), ("B5 (JIS)", 182.0, 257.0)] {
        mm(n, w, h);
    }
    mm("F4", 210.0, 330.0);
    // ---- from here on the order follows the reference screenshots exactly
    let mut rgb = Preset::new("Default RGB", 8.5, 11.0, Unit::Inches, Web);
    rgb.dpi = 300.0;
    v.push(rgb);
    let px = |v: &mut Vec<Preset>, kind: Kind, items: &[(&str, f64, f64)]| {
        for (n, w, h) in items {
            v.push(Preset::new(n, *w, *h, Unit::Pixels, kind));
        }
    };
    px(
        &mut v,
        Web,
        &[
            ("Default Web", 1024.0, 768.0),
            ("Full Screen", 1024.0, 768.0),
            ("Web Page Square", 960.0, 960.0),
            ("Web Page 1", 600.0, 300.0),
            ("Web Page 2", 760.0, 420.0),
            ("Web Banner 1", 120.0, 60.0),
            ("Web Banner 2", 120.0, 90.0),
            ("Web Banner 3", 120.0, 600.0),
            ("Web Banner 4", 125.0, 125.0),
            ("Web Banner 5", 160.0, 600.0),
            ("Web Banner 6", 250.0, 250.0),
            ("Web Banner 7", 300.0, 250.0),
            ("Web Banner 8", 336.0, 280.0),
            ("Web Banner 9", 468.0, 60.0),
            ("Web Banner 10", 728.0, 90.0),
            ("Leaderboard Ad", 728.0, 90.0),
            ("Medium Rectangle Ad", 300.0, 250.0),
            ("Wide Skyscraper Ad", 160.0, 600.0),
            ("Blog Cover", 560.0, 315.0),
            ("Blog Graphic", 800.0, 1200.0),
            ("Website - Small", 1024.0, 768.0),
            ("Website - Normal", 1280.0, 800.0),
            ("Website - Medium", 1366.0, 768.0),
            ("Website - Large", 1440.0, 900.0),
            ("Website - Extra Large", 1280.0, 1024.0),
            ("Website - Huge", 1920.0, 1280.0),
            ("Full Website", 1440.0, 3072.0),
        ],
    );
    px(
        &mut v,
        Devices,
        &[
            ("MacBook Pro 13\"", 2560.0, 1600.0),
            ("MacBook Pro 15\"", 2880.0, 1800.0),
            ("MacBook Pro 14\"", 3024.0, 1964.0),
            ("MacBook Pro 16\"", 3456.0, 2244.0),
            ("iMac27", 2560.0, 1440.0),
            ("iPhone 12/ 12 Pro", 1170.0, 2532.0),
            ("iPhone 12 mini/ 13 mini", 1080.0, 2340.0),
            ("iPhone 12 Pro Max", 1284.0, 2778.0),
            ("iPad Pro 4th gen 12.9\"", 2048.0, 2732.0),
            ("iPad Pro 4th gen 11\"", 1668.0, 2388.0),
            ("iPad Air (4th gen)", 1640.0, 2360.0),
            ("iPad 7th gen/8th gen", 1620.0, 2160.0),
            ("iPhone X", 375.0, 812.0),
            ("iPhone XR", 828.0, 1792.0),
            ("iPhone Xs", 1242.0, 2688.0),
            ("iPhone 6/7/8 Plus", 414.0, 736.0),
            ("iPhone 6/7/8", 375.0, 667.0),
            ("iPhone 5/SE", 320.0, 568.0),
            ("Google Pixel 2/XL", 411.0, 731.0),
            ("Android mobile", 360.0, 640.0),
            ("Apple Watch 38 mm", 272.0, 340.0),
            ("Apple Watch 42 mm", 312.0, 390.0),
            ("Apple Watch 4 40 mm", 324.0, 394.0),
            ("Apple Watch 4 44 mm", 368.0, 448.0),
            ("iPad 3/4/Air/Mini", 768.0, 1024.0),
            ("iPad Pro 12.9/4th gen", 1024.0, 1366.0),
            ("Nexus 7", 600.0, 960.0),
            ("Nexus 9", 1024.0, 768.0),
            ("Nexus 10", 1280.0, 800.0),
            ("Surface Pro 3", 1440.0, 960.0),
            ("Surface Pro 4", 1368.0, 912.0),
        ],
    );
    px(
        &mut v,
        Social,
        &[
            ("Facebook Cover", 851.0, 315.0),
            ("Facebook Post", 940.0, 788.0),
            ("Facebook App", 810.0, 450.0),
            ("Facebook Ad", 1200.0, 627.0),
            ("Instagram Square", 1080.0, 1080.0),
            ("Instagram Story", 1080.0, 1920.0),
            ("Twitter Cover", 1500.0, 500.0),
            ("Twitter Post", 1024.0, 512.0),
            ("YouTube Cover", 2560.0, 1440.0),
            ("Google Business Profile", 250.0, 250.0),
            ("Google Business Cover", 1080.0, 608.0),
            ("Google Business Post", 497.0, 373.0),
            ("LinkedIn Cover", 1440.0, 425.0),
            ("LinkedIn Banner", 646.0, 220.0),
            ("Dribbble Shot small", 400.0, 300.0),
            ("Dribbble Shot large", 800.0, 600.0),
            ("Twitch Cover", 900.0, 480.0),
            ("Twitch Video", 1920.0, 1080.0),
            ("Pinterest Pin", 735.0, 600.0),
            ("Tumblr Graphic", 540.0, 810.0),
        ],
    );
    v.push(Preset::new("Poster", 18.0, 24.0, Unit::Inches, Print));
    v.push(Preset::new("International Business Card", 85.0, 55.0, Unit::Millimeters, Print));
    v.push(Preset::new("Postcard", 6.0, 4.0, Unit::Inches, Print));
    px(&mut v, Web, &[("Presentation", 1920.0, 1080.0), ("Desktop Web Page", 2048.0, 3840.0), ("Mobile Web Page", 1296.0, 2304.0), ("Mobile Leaderboard", 320.0, 50.0)]);
    px(
        &mut v,
        Social,
        &[
            ("Instagram Profile", 320.0, 320.0),
            ("Instagram Landscape", 1080.0, 566.0),
            ("Instagram Portrait", 1080.0, 1350.0),
            ("Facebook Profile", 170.0, 170.0),
            ("Facebook Square", 1200.0, 1200.0),
            ("Facebook Landscape", 1200.0, 630.0),
            ("Facebook Portrait", 630.0, 1200.0),
            ("Facebook Story", 1080.0, 1920.0),
            ("YouTube Profile", 800.0, 800.0),
            ("YouTube Banner", 2048.0, 1152.0),
            ("YouTube Thumbnail", 3840.0, 1440.0),
        ],
    );
    v
}

/// Print sizes offered by Layout ▸ Paper Type.
fn paper_types(all: &[Preset]) -> Vec<&Preset> {
    all.iter().filter(|p| p.kind == Kind::Print).collect()
}

// ------------------------------------------------------------------------------------------
// state
// ------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct TemplateInfo {
    pub path: PathBuf,
    pub name: String,
    pub category: String,
    pub modified: String,
}

#[derive(Clone)]
pub struct NewDocState {
    /// 0 = Document Settings, 1 = Templates.
    pub tab: usize,
    pub search: String,
    pub sort: usize,
    pub filter: usize,
    /// Name of the selected preset.
    pub selected: Option<String>,
    /// Name being typed for a new custom preset ("+").
    pub new_preset_name: Option<String>,

    pub name: String,
    pub pages: usize,
    pub multipage: bool,
    pub width: f64,
    pub height: f64,
    pub unit: Unit,
    pub dpi: f64,
    pub bleed_on: bool,
    pub bleed: f64,
    pub color: ColorSettings,
    pub layout_open: bool,
    pub colors_open: bool,
    pub dont_show: bool,

    pub category: usize,
    pub template_search: String,
    pub templates: Option<Vec<TemplateInfo>>,
    pub template_sel: Option<usize>,
    pub thumbs: std::collections::HashMap<PathBuf, Option<egui::TextureHandle>>,
}

impl NewDocState {
    pub fn new(n: usize, unit: Unit) -> NewDocState {
        NewDocState {
            tab: 0,
            search: String::new(),
            sort: 0,
            filter: 0,
            selected: Some("Letter".into()),
            new_preset_name: None,
            name: format!("Untitled-{n}"),
            pages: 1,
            multipage: false,
            width: 612.0,
            height: 792.0,
            unit,
            dpi: 300.0,
            bleed_on: false,
            bleed: 9.0,
            color: ColorSettings::default(),
            layout_open: true,
            colors_open: true,
            dont_show: false,
            category: 0,
            template_search: String::new(),
            templates: None,
            template_sel: None,
            thumbs: Default::default(),
        }
    }

    /// The template chosen on the Templates tab, if that tab is active.
    pub fn template(&self) -> Option<&TemplateInfo> {
        if self.tab != 1 {
            return None;
        }
        self.templates.as_ref().and_then(|t| self.template_sel.and_then(|i| t.get(i)))
    }

    fn apply(&mut self, p: &Preset) {
        self.selected = Some(p.name.clone());
        self.width = p.width;
        self.height = p.height;
        self.unit = p.unit;
        self.dpi = p.dpi;
        self.color.cmyk = p.cmyk;
    }
}

fn all_presets(app: &VectorApp) -> Vec<Preset> {
    let mut v: Vec<Preset> = app
        .settings
        .custom_page_presets
        .iter()
        .enumerate()
        .map(|(i, c)| Preset { name: c.name.clone(), width: c.width, height: c.height, unit: c.unit, dpi: c.dpi, cmyk: c.cmyk, kind: Kind::Custom, custom: Some(i) })
        .collect();
    v.extend(builtin_presets());
    v
}

fn visible_presets(app: &VectorApp, st: &NewDocState) -> Vec<Preset> {
    let q = st.search.trim().to_lowercase();
    let kind = match st.filter {
        1 => Some(Kind::Custom),
        2 => Some(Kind::Print),
        3 => Some(Kind::Web),
        4 => Some(Kind::Devices),
        5 => Some(Kind::Social),
        _ => None,
    };
    let mut v: Vec<Preset> = all_presets(app).into_iter().filter(|p| kind.is_none_or(|k| p.kind == k) && (q.is_empty() || p.name.to_lowercase().contains(&q) || p.size_text().to_lowercase().contains(&q))).collect();
    match st.sort {
        1 => v.sort_by_key(|p| p.name.to_lowercase()),
        2 => {
            let rank = |p: &Preset| app.settings.recent_page_presets.iter().position(|n| n == &p.name).unwrap_or(usize::MAX);
            v.sort_by_key(rank);
        }
        3 => v.sort_by(|a, b| b.area().total_cmp(&a.area())),
        4 => v.sort_by(|a, b| a.area().total_cmp(&b.area())),
        _ => {}
    }
    v
}

fn load_templates() -> Vec<TemplateInfo> {
    let root = crate::settings::config_dir().join("Templates");
    let _ = std::fs::create_dir_all(&root);
    let mut out = Vec::new();
    let mut stack = vec![(root.clone(), "Other".to_string())];
    while let Some((dir, cat)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                // a folder named like a category files its templates under that category
                let c = CATEGORIES.iter().find(|c| c.eq_ignore_ascii_case(&name)).map(|c| c.to_string()).unwrap_or_else(|| cat.clone());
                stack.push((p, c));
            } else if matches!(vector_io::extension(&p).as_str(), "cdr" | "cdt" | "cdrt" | "vct") {
                let modified = e.metadata().ok().and_then(|m| m.modified().ok()).map(crate::settings::format_time).unwrap_or_default();
                out.push(TemplateInfo { name: p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), path: p, category: cat.clone(), modified });
            }
        }
    }
    out.sort_by_key(|t| t.name.to_lowercase());
    out
}

// ------------------------------------------------------------------------------------------
// drawing helpers
// ------------------------------------------------------------------------------------------

const LABEL_W: f32 = 140.0;

/// A settings row: right-aligned label then the field.
fn row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, _) = ui.allocate_exact_size(vec2(LABEL_W, 24.0), Sense::hover());
        ui.painter().text(pos2(r.right(), r.center().y), Align2::RIGHT_CENTER, label, FontId::proportional(12.5), theme::TEXT);
        add(ui)
    })
    .inner
}

fn section_title(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(30.0);
        ui.label(RichText::new(text).strong().size(13.0).color(Color32::WHITE));
    });
    ui.add_space(2.0);
}

/// Collapsible section header with a ^/v chevron ("^ Layout").
fn collapsible(ui: &mut Ui, text: &str, open: &mut bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
    let c = pos2(r.left() + 10.0, r.center().y);
    if *open {
        crate::widgets::chevron_up(ui.painter(), c, 3.5, theme::TEXT);
    } else {
        crate::widgets::chevron(ui.painter(), c, 3.5, theme::TEXT);
    }
    ui.painter().text(pos2(r.left() + 20.0, r.center().y), Align2::LEFT_CENTER, text, FontId::proportional(13.0), Color32::WHITE);
    if resp.clicked() {
        *open = !*open;
    }
}

fn hline(ui: &mut Ui) {
    ui.add_space(6.0);
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.left()..=r.right() - 6.0, r.top(), Stroke::new(1.0, theme::SEP));
    ui.add_space(8.0);
}

/// Integer spin field (text plus up/down arrows).
fn spin(ui: &mut Ui, id: &str, value: &mut usize, range: std::ops::RangeInclusive<usize>, width: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let mut t = value.to_string();
        let r = ui.add(egui::TextEdit::singleline(&mut t).id_salt(id).desired_width(width - 14.0).margin(vec2(4.0, 3.0)));
        if r.changed() {
            if let Ok(v) = t.trim().parse::<usize>() {
                *value = v.clamp(*range.start(), *range.end());
            }
        }
        let (rect, _) = ui.allocate_exact_size(vec2(14.0, 22.0), Sense::hover());
        let up = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.center().y));
        let dn = Rect::from_min_max(pos2(rect.min.x, rect.center().y), rect.max);
        let ru = ui.interact(up, egui::Id::unique((id, "up")), Sense::click());
        let rd = ui.interact(dn, egui::Id::unique((id, "dn")), Sense::click());
        crate::widgets::chevron_up(ui.painter(), up.center(), 3.0, if ru.hovered() { Color32::WHITE } else { theme::TEXT_DIM });
        crate::widgets::chevron(ui.painter(), dn.center(), 3.0, if rd.hovered() { Color32::WHITE } else { theme::TEXT_DIM });
        if ru.clicked() && *value < *range.end() {
            *value += 1;
        }
        if rd.clicked() && *value > *range.start() {
            *value -= 1;
        }
    });
}

/// Icon toggle button (Page view, Orientation).
fn icon_toggle(app: &mut VectorApp, ui: &mut Ui, icon: &str, tip: &str, on: bool) -> bool {
    crate::widgets::icon_button_sized(&mut app.icons, ui, icon, tip, true, on, 26.0, 18.0).clicked()
}

fn combo_strs(ui: &mut Ui, id: &str, cur: &mut String, items: &[&str], default: &str, width: f32) {
    let shown = |s: &str| if s == default { format!("(Default) {s}") } else { s.to_string() };
    egui::ComboBox::from_id_salt(id).selected_text(shown(cur)).width(width).show_ui(ui, |ui| {
        for it in items {
            if ui.selectable_label(cur == it, shown(it)).clicked() {
                *cur = it.to_string();
            }
        }
    });
}

/// The preset outline: the page proportions drawn as a rounded (screen sizes) or square
/// (print sizes) rectangle, fitted into `area`.
fn draw_thumb(p: &egui::Painter, area: Rect, w: f64, h: f64, rounded: bool) {
    let max = vec2(area.width(), area.height());
    let s = (max.x / w as f32).min(max.y / h as f32);
    let sz = vec2((w as f32 * s).max(12.0), (h as f32 * s).max(12.0));
    let r = Rect::from_center_size(area.center(), sz);
    let radius = if rounded { (sz.min_elem() * 0.2).clamp(3.0, 7.0) } else { 0.0 };
    p.rect_stroke(r, radius, Stroke::new(1.0, Color32::from_rgb(0xd8, 0xd8, 0xd8)), StrokeKind::Inside);
}

fn elide(ui: &Ui, text: &str, max_w: f32, font: &FontId) -> String {
    let width = |s: &str| ui.fonts_mut(|f| f.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x);
    if width(text) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut n = chars.len();
    while n > 1 {
        n -= 1;
        let s: String = chars[..n].iter().collect::<String>() + "...";
        if width(&s) <= max_w {
            return s;
        }
    }
    "...".into()
}

// ------------------------------------------------------------------------------------------
// dialog
// ------------------------------------------------------------------------------------------

pub fn show(app: &mut VectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.new_doc.take() else { return };
    let mut ok = false;
    let mut cancel = false;
    let size = vec2(924.0, 685.0);
    let ((), dismissed) = window(ctx, "new_doc", "Create a New Document", size, |ui| {
        let body = ui.max_rect();
        let x0 = body.left();
        let y0 = body.top();
        let w = body.width();
        // ---- tabs
        let tabs = Rect::from_min_size(pos2(x0, y0 + 8.0), vec2(300.0, 25.0));
        ui.scope_builder(UiBuilder::new().max_rect(tabs), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for (i, t) in ["Document Settings", "Templates"].iter().enumerate() {
                    let sel = st.tab == i;
                    let b = egui::Button::new(RichText::new(*t).color(if sel { Color32::WHITE } else { theme::TEXT }))
                        .fill(if sel { theme::ACCENT } else { Color32::from_rgb(0x3a, 0x3a, 0x3a) })
                        .corner_radius(2.0)
                        .min_size(vec2(0.0, 24.0));
                    if ui.add(b).clicked() {
                        st.tab = i;
                    }
                }
            });
        });
        ui.painter().hline(x0..=x0 + w, y0 + 41.0, Stroke::new(1.0, theme::SEP));
        let grid_rect = Rect::from_min_max(pos2(x0, y0 + 92.0), pos2(x0 + 479.0, y0 + 562.0));
        let right_rect = Rect::from_min_max(pos2(x0 + 492.0, y0 + 92.0), pos2(x0 + w, y0 + 562.0));
        if st.tab == 0 {
            settings_tab(app, ui, &mut st, Rect::from_min_max(pos2(x0, y0 + 50.0), pos2(x0 + w, y0 + 74.0)), grid_rect, right_rect, &mut ok);
        } else {
            templates_tab(app, ui, &mut st, Rect::from_min_max(pos2(x0, y0 + 50.0), pos2(x0 + w, y0 + 74.0)), grid_rect, right_rect, &mut ok);
        }
        // ---- footer
        let fy = y0 + 612.0;
        ui.painter().hline(x0 - 10.0..=x0 + w + 10.0, fy - 8.0, Stroke::new(1.0, theme::SEP));
        let footer = Rect::from_min_max(pos2(x0, fy), pos2(x0 + w, fy + 28.0));
        ui.scope_builder(UiBuilder::new().max_rect(footer), |ui| {
            ui.horizontal_centered(|ui| {
                if ui.add(egui::Button::new("?").min_size(vec2(24.0, 24.0))).on_hover_text("Help").clicked() {
                    app.run(crate::commands::Cmd::ProductHelp);
                }
                ui.add_space(4.0);
                ui.checkbox(&mut st.dont_show, "Do not show this dialog again");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new("Cancel").min_size(vec2(80.0, 24.0))).clicked() {
                        cancel = true;
                    }
                    let can = if st.tab == 0 { st.width > 0.0 && st.height > 0.0 && !st.name.trim().is_empty() } else { st.template().is_some() };
                    if super::ok_button(ui, "OK", can) {
                        ok = true;
                    }
                });
            });
        });
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) && st.new_preset_name.is_none() && !egui::Popup::is_any_open(ui.ctx()) {
            ok = true;
        }
    });
    if ok {
        if st.dont_show {
            app.settings.show_new_doc_dialog = false;
        }
        if let Some(t) = st.template().cloned() {
            if app.open_path(&t.path) {
                if let Some(d) = app.doc_mut() {
                    d.path = None;
                    d.title = format!("{}.cdr", st.name.trim());
                    d.history.touch();
                }
            }
            return;
        }
        if let Some(n) = st.selected.clone() {
            app.settings.recent_page_presets.retain(|x| x != &n);
            app.settings.recent_page_presets.insert(0, n);
            app.settings.recent_page_presets.truncate(20);
        }
        app.create_document(&st);
    } else if !(cancel || dismissed) {
        app.dialogs.new_doc = Some(st);
    }
}

fn search_box(ui: &mut Ui, text: &mut String, hint: &str, width: f32) {
    ui.add(egui::TextEdit::singleline(text).hint_text(hint).desired_width(width).margin(vec2(6.0, 4.0)));
}

#[allow(clippy::too_many_arguments)]
fn settings_tab(app: &mut VectorApp, ui: &mut Ui, st: &mut NewDocState, top: Rect, grid_rect: Rect, right_rect: Rect, ok: &mut bool) {
    // ---- search / sort / filter row
    ui.scope_builder(UiBuilder::new().max_rect(top), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            search_box(ui, &mut st.search, "Search presets", 460.0);
            ui.add_space(4.0);
            ui.spacing_mut().interact_size.y = 24.0;
            egui::ComboBox::from_id_salt("nd_sort").selected_text(SORTS[st.sort]).width(206.0).show_ui(ui, |ui| {
                for (i, s) in SORTS.iter().enumerate() {
                    ui.selectable_value(&mut st.sort, i, *s);
                }
            });
            egui::ComboBox::from_id_salt("nd_filter").selected_text(SIZE_FILTERS[st.filter]).width(196.0).show_ui(ui, |ui| {
                for (i, s) in SIZE_FILTERS.iter().enumerate() {
                    ui.selectable_value(&mut st.filter, i, *s);
                }
            });
        });
    });
    // ---- presets grid
    let presets = visible_presets(app, st);
    ui.painter().rect(grid_rect, 0.0, Color32::from_rgb(0x22, 0x22, 0x22), Stroke::new(1.0, theme::SEP), StrokeKind::Inside);
    let inner = grid_rect.shrink(2.0);
    ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
        egui::ScrollArea::vertical().id_salt("nd_presets").auto_shrink([false, false]).show(ui, |ui| {
            let cols = 4;
            let cell = vec2((inner.width() - 14.0) / cols as f32, 106.0);
            let rows = presets.len().div_ceil(cols);
            let (area, _) = ui.allocate_exact_size(vec2(cell.x * cols as f32, cell.y * rows as f32), Sense::hover());
            let font = FontId::proportional(12.0);
            for (i, p) in presets.iter().enumerate() {
                let r = Rect::from_min_size(area.min + vec2((i % cols) as f32 * cell.x, (i / cols) as f32 * cell.y), cell);
                if !ui.is_rect_visible(r) {
                    continue;
                }
                let resp = ui.interact(r, egui::Id::unique(("nd_preset", i)), Sense::click());
                let selected = st.selected.as_deref() == Some(p.name.as_str());
                if selected {
                    ui.painter().rect_filled(r.shrink(2.0), 3.0, theme::ACCENT);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r.shrink(2.0), 3.0, theme::HOVER);
                }
                let thumb = Rect::from_center_size(pos2(r.center().x, r.top() + 42.0), vec2(60.0, 58.0));
                draw_thumb(ui.painter(), thumb, p.width, p.height, p.kind != Kind::Print);
                if let Some(t) = app.icons.get(ui.ctx(), p.kind.icon(), 20.0, Tone::Bright) {
                    let ir = Rect::from_center_size(pos2(r.right() - 22.0, r.top() + 14.0), vec2(20.0, 20.0));
                    egui::Image::from_texture(t).paint_at(ui, ir);
                }
                let name = elide(ui, &p.name, cell.x - 8.0, &font);
                ui.painter().text(pos2(r.center().x, r.top() + 82.0), Align2::CENTER_CENTER, name, font.clone(), theme::TEXT);
                ui.painter().text(pos2(r.center().x, r.top() + 96.0), Align2::CENTER_CENTER, p.size_text(), font.clone(), theme::TEXT);
                let resp = resp.on_hover_text(format!("{}\n{}", p.name, p.size_text()));
                if resp.clicked() {
                    st.apply(p);
                }
                if resp.double_clicked() {
                    st.apply(p);
                    *ok = true;
                }
            }
            if presets.is_empty() {
                ui.label(RichText::new("No presets match.").color(theme::TEXT_DIM));
            }
        });
    });
    // ---- + / − (save / delete custom preset)
    let pm = Rect::from_min_size(pos2(grid_rect.left() + 2.0, grid_rect.bottom() + 6.0), vec2(grid_rect.width(), 26.0));
    ui.scope_builder(UiBuilder::new().max_rect(pm), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let sel_custom = st.selected.as_ref().and_then(|n| app.settings.custom_page_presets.iter().position(|c| &c.name == n));
            if crate::widgets::icon_button_sized(&mut app.icons, ui, "add_circle", "Save preset", true, false, 24.0, 20.0).clicked() && st.new_preset_name.is_none() {
                st.new_preset_name = Some(format!("{} × {}", st.unit.format(st.width, st.dpi), st.unit.format(st.height, st.dpi)));
            }
            if crate::widgets::icon_button_sized(&mut app.icons, ui, "remove_circle", "Delete preset", sel_custom.is_some(), false, 24.0, 20.0).clicked() {
                if let Some(i) = sel_custom {
                    app.settings.custom_page_presets.remove(i);
                    st.selected = None;
                }
            }
            if let Some(name) = st.new_preset_name.as_mut() {
                ui.add_space(6.0);
                ui.label("Preset name:");
                let r = ui.add(egui::TextEdit::singleline(name).desired_width(170.0));
                r.request_focus();
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let save = ui.button("Save").clicked() || enter;
                let cancel_new = ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
                if save && !name.trim().is_empty() {
                    let n = name.trim().to_string();
                    app.settings.custom_page_presets.retain(|c| c.name != n);
                    app.settings.custom_page_presets.push(crate::settings::CustomPagePreset { name: n.clone(), width: st.width, height: st.height, unit: st.unit, dpi: st.dpi, cmyk: st.color.cmyk });
                    st.selected = Some(n);
                    st.new_preset_name = None;
                } else if cancel_new {
                    st.new_preset_name = None;
                }
            }
        });
    });
    // ---- Document Settings
    let all = all_presets(app);
    ui.scope_builder(UiBuilder::new().max_rect(right_rect), |ui| {
        egui::ScrollArea::vertical().id_salt("nd_settings").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(right_rect.width() - 16.0);
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.spacing_mut().interact_size.y = 24.0;
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("Document Settings").strong().size(13.0).color(Color32::WHITE));
            });
            ui.add_space(4.0);
            section_title(ui, "General");
            row(ui, "Name:", |ui| ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(170.0).margin(vec2(4.0, 3.0))));
            row(ui, "Number of pages:", |ui| spin(ui, "nd_pages", &mut st.pages, 1..=999, 50.0));
            row(ui, "Page view:", |ui| {
                if icon_toggle(app, ui, "page_single", "Single Page View", !st.multipage) {
                    st.multipage = false;
                }
                if icon_toggle(app, ui, "page_multi", "Multipage View", st.multipage) {
                    st.multipage = true;
                }
            });
            row(ui, "Primary color mode:", |ui| {
                ui.radio_value(&mut st.color.cmyk, true, "CMYK");
                ui.radio_value(&mut st.color.cmyk, false, "RGB");
            });
            ui.add_space(10.0);
            section_title(ui, "Dimensions");
            let before = (st.width, st.height);
            row(ui, "Width:", |ui| {
                crate::widgets::unit_field(ui, egui::Id::unique("nd_w"), &mut st.width, st.unit, st.dpi, 70.0, true);
                ui.add_space(6.0);
                egui::ComboBox::from_id_salt("nd_unit").selected_text(st.unit.name()).width(98.0).show_ui(ui, |ui| {
                    for u in Unit::ALL {
                        ui.selectable_value(&mut st.unit, u, u.name());
                    }
                });
            });
            row(ui, "Height:", |ui| crate::widgets::unit_field(ui, egui::Id::unique("nd_h"), &mut st.height, st.unit, st.dpi, 70.0, true));
            let landscape = st.width > st.height;
            row(ui, "Orientation:", |ui| {
                if icon_toggle(app, ui, "portrait", "Portrait", !landscape) && landscape {
                    std::mem::swap(&mut st.width, &mut st.height);
                }
                if icon_toggle(app, ui, "landscape", "Landscape", landscape) && !landscape {
                    std::mem::swap(&mut st.width, &mut st.height);
                }
            });
            row(ui, "Resolution:", |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let id = egui::Id::unique("nd_dpi_text");
                let mut t = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| format!("{}", st.dpi));
                let r = ui.add(egui::TextEdit::singleline(&mut t).id(id.with("te")).desired_width(56.0).margin(vec2(4.0, 3.0)));
                if r.changed() {
                    if let Ok(v) = t.trim().parse::<f64>() {
                        if (1.0..=10000.0).contains(&v) {
                            st.dpi = v;
                        }
                    }
                }
                if r.has_focus() {
                    ui.data_mut(|d| d.insert_temp(id, t));
                } else {
                    ui.data_mut(|d| d.remove::<String>(id));
                }
                egui::ComboBox::from_id_salt("nd_dpi").selected_text("").width(18.0).show_ui(ui, |ui| {
                    for d in RESOLUTIONS {
                        ui.selectable_value(&mut st.dpi, *d, format!("{d}"));
                    }
                });
                ui.add_space(8.0);
                ui.label("dpi");
            });
            if (st.width, st.height) != before {
                // the size no longer matches the chosen preset
                if let Some(p) = st.selected.as_ref().and_then(|n| all.iter().find(|p| &p.name == n)) {
                    let same = |a: f64, b: f64| (a - b).abs() < 0.01;
                    if !(same(p.width, st.width) && same(p.height, st.height)) && !(same(p.width, st.height) && same(p.height, st.width)) {
                        st.selected = None;
                    }
                }
            }
            hline(ui);
            collapsible(ui, "Layout", &mut st.layout_open);
            if st.layout_open {
                let papers = paper_types(&all);
                let same = |a: f64, b: f64| (a - b).abs() < 0.5;
                let cur = papers.iter().position(|p| (same(p.width, st.width) && same(p.height, st.height)) || (same(p.width, st.height) && same(p.height, st.width)));
                row(ui, "Paper Type:", |ui| {
                    let text = cur.map(|i| papers[i].name.clone()).unwrap_or_else(|| "Custom".into());
                    egui::ComboBox::from_id_salt("nd_paper").selected_text(text).width(170.0).height(320.0).show_ui(ui, |ui| {
                        for (i, p) in papers.iter().enumerate() {
                            if ui.selectable_label(cur == Some(i), p.name.as_str()).clicked() {
                                let land = st.width > st.height;
                                let (w, h) = if land == (p.width > p.height) { (p.width, p.height) } else { (p.height, p.width) };
                                st.width = w;
                                st.height = h;
                                st.selected = Some(p.name.clone());
                            }
                        }
                        let _ = ui.selectable_label(cur.is_none(), "Custom");
                    });
                });
                row(ui, "", |ui| {
                    ui.checkbox(&mut st.bleed_on, "Bleed:");
                    crate::widgets::unit_field(ui, egui::Id::unique("nd_bleed"), &mut st.bleed, st.unit, st.dpi, 60.0, st.bleed_on);
                    st.bleed = st.bleed.max(0.0);
                });
            }
            hline(ui);
            collapsible(ui, "Colors", &mut st.colors_open);
            if st.colors_open {
                row(ui, "RGB profile:", |ui| combo_strs(ui, "nd_rgb", &mut st.color.rgb_profile, RGB_PROFILES, DEFAULT_RGB_PROFILE, 220.0));
                row(ui, "CMYK profile:", |ui| combo_strs(ui, "nd_cmyk", &mut st.color.cmyk_profile, CMYK_PROFILES, DEFAULT_CMYK_PROFILE, 220.0));
                row(ui, "Grayscale profile:", |ui| combo_strs(ui, "nd_gray", &mut st.color.gray_profile, GRAY_PROFILES, DEFAULT_GRAY_PROFILE, 220.0));
                row(ui, "Rendering intent:", |ui| {
                    egui::ComboBox::from_id_salt("nd_intent").selected_text(st.color.intent.name()).width(220.0).show_ui(ui, |ui| {
                        for it in RenderingIntent::ALL {
                            ui.selectable_value(&mut st.color.intent, it, it.name());
                        }
                    });
                });
            }
            ui.add_space(10.0);
        });
    });
    // vertical splitter dots between the two panes
    let c = pos2(grid_rect.right() + 6.0, grid_rect.center().y);
    for dy in [-4.0, 0.0, 4.0] {
        ui.painter().circle_filled(c + vec2(0.0, dy), 1.0, theme::TEXT_DIM);
    }
}

#[allow(clippy::too_many_arguments)]
fn templates_tab(_app: &mut VectorApp, ui: &mut Ui, st: &mut NewDocState, top: Rect, grid_rect: Rect, right_rect: Rect, ok: &mut bool) {
    if st.templates.is_none() {
        st.templates = Some(load_templates());
    }
    ui.scope_builder(UiBuilder::new().max_rect(top), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            search_box(ui, &mut st.template_search, "Search templates", 460.0);
            ui.add_space(4.0);
            ui.spacing_mut().interact_size.y = 24.0;
            egui::ComboBox::from_id_salt("nd_cat").selected_text(CATEGORIES[st.category]).width(410.0).height(560.0).show_ui(ui, |ui| {
                for (i, c) in CATEGORIES.iter().enumerate() {
                    ui.selectable_value(&mut st.category, i, *c);
                }
            });
        });
    });
    let q = st.template_search.trim().to_lowercase();
    let list: Vec<(usize, TemplateInfo)> = st
        .templates
        .as_ref()
        .map(|t| t.iter().cloned().enumerate().filter(|(_, t)| (st.category == 0 || t.category == CATEGORIES[st.category]) && (q.is_empty() || t.name.to_lowercase().contains(&q))).collect())
        .unwrap_or_default();
    ui.painter().rect(grid_rect, 0.0, Color32::from_rgb(0x22, 0x22, 0x22), Stroke::new(1.0, theme::SEP), StrokeKind::Inside);
    let inner = grid_rect.shrink(2.0);
    ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
        egui::ScrollArea::vertical().id_salt("nd_templates").auto_shrink([false, false]).show(ui, |ui| {
            if list.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("No templates found.").color(theme::TEXT));
                    ui.add_space(4.0);
                    ui.label(RichText::new("Use File ▸ Save as Template to add your own, or copy\ntemplates (.cdr, .cdt, .vct) into the Templates folder.").color(theme::TEXT_DIM));
                    ui.add_space(8.0);
                    if ui.button("Open Templates Folder").clicked() {
                        let dir = crate::settings::config_dir().join("Templates");
                        let _ = std::fs::create_dir_all(&dir);
                        let _ = std::process::Command::new("explorer").arg(dir).spawn();
                    }
                });
                return;
            }
            let cols = 4;
            let cell = vec2((inner.width() - 14.0) / cols as f32, 130.0);
            let rows = list.len().div_ceil(cols);
            let (area, _) = ui.allocate_exact_size(vec2(cell.x * cols as f32, cell.y * rows as f32), Sense::hover());
            let font = FontId::proportional(12.0);
            for (k, (i, t)) in list.iter().enumerate() {
                let r = Rect::from_min_size(area.min + vec2((k % cols) as f32 * cell.x, (k / cols) as f32 * cell.y), cell);
                if !ui.is_rect_visible(r) {
                    continue;
                }
                let resp = ui.interact(r, egui::Id::unique(("nd_tpl", *i)), Sense::click());
                if st.template_sel == Some(*i) {
                    ui.painter().rect_filled(r.shrink(2.0), 3.0, theme::ACCENT);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r.shrink(2.0), 3.0, theme::HOVER);
                }
                let thumb_area = Rect::from_center_size(pos2(r.center().x, r.top() + 52.0), vec2(cell.x - 20.0, 86.0));
                let tex = st
                    .thumbs
                    .entry(t.path.clone())
                    .or_insert_with(|| {
                        vector_io::read_thumbnail(&t.path).map(|(w, h, rgba)| ui.ctx().load_texture(format!("tpl:{}", t.path.display()), egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba), egui::TextureOptions::LINEAR))
                    })
                    .clone();
                match tex {
                    Some(tex) => {
                        let s = tex.size_vec2();
                        let f = (thumb_area.width() / s.x).min(thumb_area.height() / s.y);
                        let ir = Rect::from_center_size(thumb_area.center(), s * f);
                        ui.painter().rect_filled(ir, 0.0, Color32::WHITE);
                        egui::Image::from_texture(egui::load::SizedTexture::new(tex.id(), s * f)).paint_at(ui, ir);
                    }
                    None => draw_thumb(ui.painter(), thumb_area, 612.0, 792.0, false),
                }
                let name = elide(ui, &t.name, cell.x - 8.0, &font);
                ui.painter().text(pos2(r.center().x, r.top() + 112.0), Align2::CENTER_CENTER, name, font.clone(), theme::TEXT);
                if resp.clicked() {
                    st.template_sel = Some(*i);
                }
                if resp.double_clicked() {
                    st.template_sel = Some(*i);
                    *ok = true;
                }
            }
        });
    });
    ui.scope_builder(UiBuilder::new().max_rect(right_rect), |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(RichText::new("Template Details").strong().size(13.0).color(Color32::WHITE));
        });
        ui.add_space(8.0);
        let sel = st.templates.as_ref().and_then(|t| st.template_sel.and_then(|i| t.get(i))).cloned();
        match sel {
            Some(t) => {
                row(ui, "Name:", |ui| ui.label(t.name.as_str()));
                row(ui, "Category:", |ui| ui.label(t.category.as_str()));
                row(ui, "Modified:", |ui| ui.label(t.modified.as_str()));
                row(ui, "File:", |ui| ui.label(RichText::new(t.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()).color(theme::TEXT_DIM)));
                ui.add_space(6.0);
                row(ui, "Document name:", |ui| ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(170.0)));
            }
            None => {
                ui.horizontal(|ui| {
                    ui.add_space(30.0);
                    ui.label(RichText::new("Select a template to see its details.").color(theme::TEXT_DIM));
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_match_reference() {
        let v = builtin_presets();
        let find = |n: &str| v.iter().find(|p| p.name == n).unwrap_or_else(|| panic!("{n}"));
        assert_eq!(find("B5 (JIS)").size_text(), "182 × 257 mm");
        assert_eq!(find("Default RGB").size_text(), "8.5 × 11 in.");
        assert_eq!(find("Web Banner 4").size_text(), "125 × 125 px");
        assert_eq!(find("Poster").size_text(), "18 × 24 in.");
        assert_eq!(find("Postcard").size_text(), "6 × 4 in.");
        assert_eq!(find("International Business Card").size_text(), "85 × 55 mm");
        assert_eq!(find("YouTube Thumbnail").size_text(), "3840 × 1440 px");
        // order after F4 follows the screenshots
        let i = v.iter().position(|p| p.name == "F4").unwrap();
        assert_eq!(v[i - 1].name, "B5 (JIS)");
        assert_eq!(v[i + 1].name, "Default RGB");
        assert_eq!(v.last().unwrap().name, "YouTube Thumbnail");
        assert_eq!(find("Surface Pro 4").kind, Kind::Devices);
        assert_eq!(find("Facebook Cover").kind, Kind::Social);
        // names are unique (selection is by name)
        let mut names: Vec<&str> = v.iter().map(|p| p.name.as_str()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), v.len());
    }
}
