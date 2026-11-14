//! Tools ▸ Color Management: the Color Management Settings dialog of CorelDRAW 2026, with its
//! Document tab (the active document's color profiles, assigned or converted to, its primary
//! color mode and rendering intent) and its Default tab (the presets, the default profiles, the
//! color engine and conversion options, the open and import policies and the spot color
//! definition). The "Simulate Color Management Off" preset (color engine None) shows colors
//! exactly by their numbers.

use super::{ok_cancel_row, window};
use crate::app::AnyVectorApp;
use crate::theme;
use egui::{vec2, Ui};
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use vector_core::cms::{self, CmsOptions, ColorEngine};
use vector_core::document::{ColorSettings, RenderingIntent, DEFAULT_CMYK_PROFILE, DEFAULT_GRAY_PROFILE, DEFAULT_RGB_PROFILE};
use vector_core::Document;

/// What to do with a color profile when opening or importing a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PolicyAction {
    #[default]
    UseEmbedded,
    ConvertToDocument,
    AssignDocument,
}

impl PolicyAction {
    pub const ALL: [PolicyAction; 3] = [PolicyAction::UseEmbedded, PolicyAction::ConvertToDocument, PolicyAction::AssignDocument];

    pub fn name(self) -> &'static str {
        match self {
            PolicyAction::UseEmbedded => "Use embedded color profile",
            PolicyAction::ConvertToDocument => "Convert to document color profile",
            PolicyAction::AssignDocument => "Assign document color profile",
        }
    }
}

/// The color management policy for opening, or for importing and pasting.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Policy {
    pub rgb: PolicyAction,
    pub cmyk: PolicyAction,
    pub gray: PolicyAction,
    pub warn_mismatch: bool,
    pub warn_missing: bool,
}

impl Policy {
    fn all(a: PolicyAction) -> Policy {
        Policy { rgb: a, cmyk: a, gray: a, warn_mismatch: false, warn_missing: false }
    }
}

/// How spot colors are defined for conversions (Spot color definition).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SpotDefinition {
    #[default]
    Lab,
    Cmyk,
    Rgb,
}

impl SpotDefinition {
    pub const ALL: [SpotDefinition; 3] = [SpotDefinition::Lab, SpotDefinition::Cmyk, SpotDefinition::Rgb];

    pub fn name(self) -> &'static str {
        match self {
            SpotDefinition::Lab => "Lab values",
            SpotDefinition::Cmyk => "CMYK values",
            SpotDefinition::Rgb => "RGB values",
        }
    }
}

/// A set of default color management settings, as a preset stores it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CmSet {
    pub name: String,
    pub defaults: ColorSettings,
    pub options: CmsOptions,
    pub open: Policy,
    pub import: Policy,
    pub spot: SpotDefinition,
}

impl Default for CmSet {
    fn default() -> Self {
        preset("North America General Purpose").unwrap()
    }
}

/// The Default tab, persisted in the settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CmPrefs {
    /// The preset the settings come from ("" when changed: Custom).
    pub preset: String,
    /// The default color profiles, primary color mode and rendering intent (new documents).
    pub defaults: ColorSettings,
    /// Color engine, Preserve pure black, Map gray to CMYK black.
    pub options: CmsOptions,
    pub open: Policy,
    pub import: Policy,
    pub spot: SpotDefinition,
    /// Presets the user saved.
    pub user_presets: Vec<CmSet>,
}

impl Default for CmPrefs {
    fn default() -> Self {
        let s = CmSet::default();
        CmPrefs { preset: s.name, defaults: s.defaults, options: s.options, open: s.open, import: s.import, spot: s.spot, user_presets: vec![] }
    }
}

impl CmPrefs {
    fn set(&self, name: &str) -> CmSet {
        CmSet { name: name.into(), defaults: self.defaults.clone(), options: self.options.clone(), open: self.open.clone(), import: self.import.clone(), spot: self.spot }
    }

    fn apply(&mut self, s: &CmSet) {
        self.preset = s.name.clone();
        self.defaults = s.defaults.clone();
        self.options = s.options.clone();
        self.open = s.open.clone();
        self.import = s.import.clone();
        self.spot = s.spot;
    }
}

/// CorelDRAW 2026's factory presets (Color\Settings\ColorSettings.xml), by name: RGB, CMYK and
/// grayscale profiles, primary color mode CMYK?, rendering intent, color engine off?
const PRESETS: [(&str, &str, &str, &str, bool, RenderingIntent, bool); 12] = [
    ("Europe General Purpose", "sRGB IEC61966-2.1", "ISO Coated v2 (ECI)", "Dot Gain 15%", true, RenderingIntent::RelativeColorimetric, false),
    ("Europe Prepress", "Adobe RGB (1998)", "ISO Coated v2 (ECI)", "Dot Gain 15%", true, RenderingIntent::RelativeColorimetric, false),
    ("Europe Web", "sRGB IEC61966-2.1", "ISO Coated v2 (ECI)", "Gray Gamma 2.2", false, RenderingIntent::RelativeColorimetric, false),
    ("Japan General Purpose", "sRGB IEC61966-2.1", "Japan Color 2001 Coated", "Dot Gain 15%", true, RenderingIntent::RelativeColorimetric, false),
    ("Japan Prepress", "Adobe RGB (1998)", "Japan Color 2001 Coated", "Dot Gain 15%", true, RenderingIntent::RelativeColorimetric, false),
    ("Japan Web", "sRGB IEC61966-2.1", "Japan Color 2001 Coated", "Gray Gamma 2.2", false, RenderingIntent::RelativeColorimetric, false),
    ("Minimal Color Management", "sRGB IEC61966-2.1", DEFAULT_CMYK_PROFILE, "Gray Gamma 2.2", false, RenderingIntent::RelativeColorimetric, false),
    ("North America General Purpose", DEFAULT_RGB_PROFILE, DEFAULT_CMYK_PROFILE, DEFAULT_GRAY_PROFILE, true, RenderingIntent::RelativeColorimetric, false),
    ("North America Prepress", "Adobe RGB (1998)", DEFAULT_CMYK_PROFILE, DEFAULT_GRAY_PROFILE, true, RenderingIntent::RelativeColorimetric, false),
    ("North America Web", "sRGB IEC61966-2.1", DEFAULT_CMYK_PROFILE, "Gray Gamma 2.2", false, RenderingIntent::RelativeColorimetric, false),
    ("Simulate Color Management Off", "sRGB IEC61966-2.1", "Wide Gamut CMYK Simulation", "Gray Gamma 2.2", false, RenderingIntent::RelativeColorimetric, true),
    ("Simulate CorelDRAW Graphics Suite X4", "sRGB IEC61966-2.1", "SWOP Proofer CMYK - Coated Stock", "Gray Gamma 2.2", false, RenderingIntent::Perceptual, false),
];

/// The descriptions of the presets.
fn preset_description(name: &str) -> &'static str {
    match name {
        n if n.ends_with("Web") => "Use this preset for Web designs.",
        n if n.ends_with("Prepress") || n.ends_with("General Purpose") => "Use this preset for designs that will be printed by print service providers.",
        "Minimal Color Management" => "Use this preset to preserve the original RGB, CMYK, and Grayscale color values when opening, importing, or pasting documents.",
        "Simulate Color Management Off" => "Simulate the color-conversion results of the Color Management Off preset that is available in earlier versions of CorelDRAW Graphics Suite: colors are shown exactly by their numbers.",
        "Simulate CorelDRAW Graphics Suite X4" => "Simulate the color conversions of CorelDRAW Graphics Suite X4.",
        _ => "",
    }
}

/// A factory preset by name.
pub fn preset(name: &str) -> Option<CmSet> {
    let (n, rgb, cmyk, gray, cmyk_mode, intent, off) = PRESETS.iter().find(|p| p.0 == name)?;
    // the presets that keep color values assign profiles on open and import
    let minimal = name.starts_with("Minimal") || name.starts_with("Simulate");
    Some(CmSet {
        name: n.to_string(),
        defaults: ColorSettings { cmyk: *cmyk_mode, rgb_profile: rgb.to_string(), cmyk_profile: cmyk.to_string(), gray_profile: gray.to_string(), intent: *intent },
        options: CmsOptions { engine: if *off { ColorEngine::None } else { ColorEngine::Builtin }, preserve_pure_black: false, map_gray_to_black: true },
        open: if minimal { Policy::all(PolicyAction::AssignDocument) } else { Policy::all(PolicyAction::UseEmbedded) },
        import: if minimal {
            Policy::all(PolicyAction::AssignDocument)
        } else {
            Policy { rgb: PolicyAction::ConvertToDocument, cmyk: PolicyAction::AssignDocument, gray: PolicyAction::ConvertToDocument, warn_mismatch: false, warn_missing: false }
        },
        spot: if name.ends_with("X4") { SpotDefinition::Rgb } else { SpotDefinition::Lab },
    })
}

// ------------------------------------------------------------------ in use

/// Every frame: the color options in force, and the active document's color settings.
pub fn sync(app: &mut AnyVectorApp) {
    cms::set_options(&app.settings.color_management.options);
    match app.doc() {
        Some(d) => cms::activate(&d.doc.settings.color),
        None => cms::deactivate(),
    }
}

/// What the drawing window's colors depend on, for its render cache.
pub fn hash_view(s: &crate::settings::Settings, doc: &ColorSettings, h: &mut impl std::hash::Hasher) {
    s.color_management.options.hash(h);
    (&doc.rgb_profile, &doc.cmyk_profile, &doc.gray_profile, doc.intent as u8).hash(h);
    (&s.proof_env, s.proof_intent).hash(h);
}

/// The profile names offered for a color space: the built-in ones, the installed ones and (CMYK)
/// those CorelDRAW's look is known for.
fn profiles(space: &str) -> Vec<String> {
    let mut v: Vec<String> = match space {
        "RGB" => ["sRGB IEC61966-2.1", "Adobe RGB (1998)", "ProPhoto RGB", "Display P3"].map(String::from).to_vec(),
        "CMYK" => {
            let mut v = vec![DEFAULT_CMYK_PROFILE.to_string()];
            v.extend(cms::grid::profile_names());
            v
        }
        _ => ["Dot Gain 10%", "Dot Gain 15%", "Dot Gain 20%", "Dot Gain 25%", "Dot Gain 30%", "Gray Gamma 1.8", "Gray Gamma 2.2"].map(String::from).to_vec(),
    };
    v.extend(cms::installed_profiles().iter().filter(|p| p.space == space).map(|p| p.description.clone()));
    v.sort_by_key(|a| a.to_lowercase());
    v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    v
}

/// The dialog's state while it is open.
pub struct CmState {
    pub tab: usize,
    /// The document's color settings being edited (Document tab), and the ones it had.
    pub doc: Option<ColorSettings>,
    pub doc_before: Option<ColorSettings>,
    /// Convert (preserve color appearance) instead of Assign new (preserve color values).
    pub convert: bool,
    pub prefs: CmPrefs,
    /// Name being typed for a new preset.
    pub new_preset: Option<String>,
    lists: [Vec<String>; 3],
}

/// Tools ▸ Color Management.
pub fn open(app: &mut AnyVectorApp) {
    let doc = app.doc().map(|d| d.doc.settings.color.clone());
    app.dialogs.color_mgmt = Some(CmState {
        tab: if doc.is_some() { 0 } else { 1 },
        doc_before: doc.clone(),
        doc,
        convert: false,
        prefs: app.settings.color_management.clone(),
        new_preset: None,
        lists: [profiles("RGB"), profiles("CMYK"), profiles("Gray")],
    });
}

const LABEL_W: f32 = 132.0;
const FIELD_W: f32 = 300.0;

/// A section title with a line to its right.
fn section(ui: &mut Ui, title: &str) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(title);
        let r = ui.available_rect_before_wrap();
        ui.painter().hline(r.left() + 6.0..=r.right() - 4.0, r.center().y, egui::Stroke::new(1.0, theme::SEP()));
    });
    ui.add_space(6.0);
}

fn label_row<R>(ui: &mut Ui, label: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(vec2(LABEL_W, 22.0), egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(label));
        ui.add_space(6.0);
        body(ui)
    })
    .inner
}

/// A profile list: "(Default) " marks the default profile. Returns true when changed.
fn profile_combo(ui: &mut Ui, id: &str, cur: &mut String, list: &[String], default: &str) -> bool {
    let shown = |s: &str| if s.eq_ignore_ascii_case(default) { format!("(Default) {s}") } else { s.to_string() };
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(shown(cur)).width(FIELD_W).height(360.0).show_ui(ui, |ui| {
        for it in list {
            if ui.selectable_label(cur.eq_ignore_ascii_case(it), shown(it)).clicked() && cur != it {
                *cur = it.clone();
                changed = true;
            }
        }
    });
    changed
}

fn enum_combo<T: Copy + PartialEq>(ui: &mut Ui, id: &str, cur: &mut T, all: &[T], name: impl Fn(T) -> &'static str) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(name(*cur)).width(FIELD_W).show_ui(ui, |ui| {
        for it in all {
            if ui.selectable_label(*cur == *it, name(*it)).clicked() && *cur != *it {
                *cur = *it;
                changed = true;
            }
        }
    });
    changed
}

/// Where the engine finds a profile, in a few words.
fn source_note(src: &cms::ProfileSource) -> String {
    match src {
        cms::ProfileSource::File(p) => format!("ICC profile: {}", p.display()),
        cms::ProfileSource::Builtin => "Built into the color engine".into(),
        cms::ProfileSource::Sampled => "Profile file not installed: colors as CorelDRAW shows them with this profile".into(),
        cms::ProfileSource::Modeled => "Profile file not installed: modeled from its name".into(),
        cms::ProfileSource::Substituted => "Profile not found: the default profile is used".into(),
    }
}

fn color_settings_rows(ui: &mut Ui, id: &str, c: &mut ColorSettings, lists: &[Vec<String>; 3], defaults: &ColorSettings) -> bool {
    let mut ch = false;
    ch |= label_row(ui, "RGB profile:", |ui| profile_combo(ui, &format!("{id}_rgb"), &mut c.rgb_profile, &lists[0], &defaults.rgb_profile));
    ch |= label_row(ui, "CMYK profile:", |ui| profile_combo(ui, &format!("{id}_cmyk"), &mut c.cmyk_profile, &lists[1], &defaults.cmyk_profile));
    ch |= label_row(ui, "Grayscale profile:", |ui| profile_combo(ui, &format!("{id}_gray"), &mut c.gray_profile, &lists[2], &defaults.gray_profile));
    ui.add_space(10.0);
    let mut mode = c.cmyk;
    ch |= label_row(ui, "Primary color mode:", |ui| enum_combo(ui, &format!("{id}_mode"), &mut mode, &[true, false], |m| if m { "CMYK" } else { "RGB" }));
    c.cmyk = mode;
    ch |= label_row(ui, "Rendering intent:", |ui| enum_combo(ui, &format!("{id}_intent"), &mut c.intent, &RenderingIntent::ALL, |i| i.name()));
    ch
}

fn document_tab(ui: &mut Ui, st: &mut CmState) {
    let defaults = st.prefs.defaults.clone();
    let (Some(cur), Some(edit)) = (st.doc_before.clone(), st.doc.as_mut()) else {
        ui.add_space(20.0);
        super::note(ui, "Open or create a document to change its color settings.");
        return;
    };
    section(ui, "Current Profiles");
    let e = cms::engine(&cur);
    for (i, (label, name)) in [("RGB:", &cur.rgb_profile), ("CMYK:", &cur.cmyk_profile), ("Grayscale:", &cur.gray_profile)].into_iter().enumerate() {
        label_row(ui, label, |ui| ui.label(name)).on_hover_text(source_note(&e.sources()[i]));
    }
    if !matches!(e.sources()[1], cms::ProfileSource::File(_)) {
        ui.horizontal(|ui| {
            ui.add_space(LABEL_W + 10.0);
            super::note(ui, &source_note(&e.sources()[1]));
        });
    }
    super::separator(ui);
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + 10.0);
        ui.vertical(|ui| {
            ui.radio_value(&mut st.convert, false, "Assign new (preserve color values)");
            ui.radio_value(&mut st.convert, true, "Convert (preserve color appearance)");
        });
    });
    ui.add_space(6.0);
    let lists = st.lists.clone();
    let mut c = edit.clone();
    label_row(ui, "RGB:", |ui| profile_combo(ui, "cm_doc_rgb", &mut c.rgb_profile, &lists[0], &defaults.rgb_profile));
    label_row(ui, "CMYK:", |ui| profile_combo(ui, "cm_doc_cmyk", &mut c.cmyk_profile, &lists[1], &defaults.cmyk_profile));
    label_row(ui, "Grayscale:", |ui| profile_combo(ui, "cm_doc_gray", &mut c.gray_profile, &lists[2], &defaults.gray_profile));
    super::separator(ui);
    let mut mode = c.cmyk;
    label_row(ui, "Primary color mode:", |ui| enum_combo(ui, "cm_doc_mode", &mut mode, &[true, false], |m| if m { "CMYK" } else { "RGB" }));
    c.cmyk = mode;
    label_row(ui, "Rendering intent:", |ui| enum_combo(ui, "cm_doc_intent", &mut c.intent, &RenderingIntent::ALL, |i| i.name()));
    *edit = c;
}

fn policy_rows(ui: &mut Ui, id: &str, p: &mut Policy) -> bool {
    let mut ch = false;
    for (label, v, k) in [("RGB:", &mut p.rgb, "r"), ("CMYK:", &mut p.cmyk, "c"), ("Grayscale:", &mut p.gray, "g")] {
        ch |= label_row(ui, label, |ui| enum_combo(ui, &format!("{id}_{k}"), v, &PolicyAction::ALL, |a| a.name()));
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + 10.0);
        ui.vertical(|ui| {
            ch |= ui.checkbox(&mut p.warn_mismatch, "Warn on color profile mismatch").changed();
            ch |= ui.checkbox(&mut p.warn_missing, "Warn on missing color profile").changed();
        });
    });
    ch
}

fn default_tab(app: &mut AnyVectorApp, ui: &mut Ui, st: &mut CmState) {
    let mut changed = false;
    // presets: the factory ones, then the user's
    let mut names: Vec<String> = PRESETS.iter().map(|p| p.0.to_string()).collect();
    names.extend(st.prefs.user_presets.iter().map(|p| p.name.clone()));
    label_row(ui, "Preset:", |ui| {
        let shown = if st.prefs.preset.is_empty() { "Custom".to_string() } else { st.prefs.preset.clone() };
        egui::ComboBox::from_id_salt("cm_preset").selected_text(shown).width(FIELD_W - 52.0).height(400.0).show_ui(ui, |ui| {
            for n in &names {
                let r = ui.selectable_label(st.prefs.preset == *n, n);
                let r = match preset_description(n) {
                    "" => r,
                    d => r.on_hover_text(d),
                };
                if r.clicked() {
                    let set = preset(n).or_else(|| st.prefs.user_presets.iter().find(|p| p.name == *n).cloned());
                    if let Some(set) = set {
                        st.prefs.apply(&set);
                    }
                }
            }
        });
        if crate::widgets::icon_button(&mut app.icons, ui, "save", "Save the settings as a preset.", true, false).clicked() {
            st.new_preset = Some(format!("Color management preset {}", st.prefs.user_presets.len() + 1));
        }
        let user = st.prefs.user_presets.iter().any(|p| p.name == st.prefs.preset);
        if crate::widgets::icon_button(&mut app.icons, ui, "delete", "Delete the preset.", user, false).clicked() && user {
            let cur = st.prefs.preset.clone();
            st.prefs.user_presets.retain(|p| p.name != cur);
            st.prefs.preset.clear();
        }
    });
    if let Some(mut name) = st.new_preset.take() {
        let mut keep = true;
        ui.horizontal(|ui| {
            ui.add_space(LABEL_W + 10.0);
            let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(FIELD_W - 52.0));
            r.request_focus();
            if r.lost_focus() {
                keep = false;
                let name = name.trim().to_string();
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.is_empty() && !PRESETS.iter().any(|p| p.0 == name) {
                    let set = st.prefs.set(&name);
                    st.prefs.user_presets.retain(|p| p.name != name);
                    st.prefs.user_presets.push(set);
                    st.prefs.preset = name;
                }
            }
        });
        if keep {
            st.new_preset = Some(name);
        }
    }
    if let Some(d) = preset_description(&st.prefs.preset).chars().next().map(|_| preset_description(&st.prefs.preset)) {
        ui.horizontal(|ui| {
            ui.add_space(LABEL_W + 10.0);
            ui.add_sized(vec2(FIELD_W, 0.0), egui::Label::new(egui::RichText::new(d).color(theme::TEXT_DIM()).size(11.5)).wrap());
        });
    }
    super::separator(ui);
    let lists = st.lists.clone();
    let factory = ColorSettings::default();
    changed |= color_settings_rows(ui, "cm_def", &mut st.prefs.defaults, &lists, &factory);
    section(ui, "Color conversion");
    changed |= label_row(ui, "Color engine:", |ui| enum_combo(ui, "cm_engine", &mut st.prefs.options.engine, &ColorEngine::ALL, |e| e.name()));
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W + 10.0);
        ui.vertical(|ui| {
            changed |= ui.checkbox(&mut st.prefs.options.preserve_pure_black, "Preserve pure black").on_hover_text("Convert black to 100% black ink only, and keep pure black when converting between CMYK profiles.").changed();
            changed |= ui.checkbox(&mut st.prefs.options.map_gray_to_black, "Map gray to CMYK black").on_hover_text("Convert neutral grays to the black ink only.").changed();
        });
    });
    section(ui, "Open policy");
    changed |= policy_rows(ui, "cm_open", &mut st.prefs.open);
    section(ui, "Import and Paste policy");
    changed |= policy_rows(ui, "cm_import", &mut st.prefs.import);
    super::separator(ui);
    changed |= label_row(ui, "Spot color definition:", |ui| enum_combo(ui, "cm_spot", &mut st.prefs.spot, &SpotDefinition::ALL, |s| s.name()));
    if changed {
        st.prefs.preset.clear();
    }
}

pub fn show(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.color_mgmt.take() else { return };
    let mut result = (false, false);
    let ((), dismissed) = window(ctx, "color_mgmt", "Color Management Settings", vec2(573.0, 540.0), |ui| {
        ui.add_space(6.0);
        super::tabs(ui, &mut st.tab, &["Document".to_string(), "Default".to_string()]);
        let h = ui.available_height() - 40.0;
        egui::ScrollArea::vertical().id_salt(("cm_scroll", st.tab)).max_height(h).auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width() - 12.0);
            if st.tab == 0 {
                document_tab(ui, &mut st);
            } else {
                default_tab(app, ui, &mut st);
            }
            ui.add_space(8.0);
        });
        result = ok_cancel_row(ui, "OK", true);
    });
    let (ok, cancel) = result;
    if ok {
        apply(app, &st);
    }
    if !(ok || cancel || dismissed) {
        app.dialogs.color_mgmt = Some(st);
    }
}

fn apply(app: &mut AnyVectorApp, st: &CmState) {
    if app.settings.color_management != st.prefs {
        app.settings.color_management = st.prefs.clone();
        // new documents start from the default color settings
        if let Some(l) = app.settings.last_new_doc.as_mut() {
            l.color = st.prefs.defaults.clone();
        }
        app.settings.save();
    }
    cms::set_options(&st.prefs.options);
    if let (Some(before), Some(after)) = (&st.doc_before, &st.doc) {
        if before != after {
            let convert = st.convert;
            let (before, after) = (before.clone(), after.clone());
            if let Some(d) = app.doc_mut() {
                d.modify(if convert { "Convert Color Profiles" } else { "Assign Color Profiles" }, |doc, _| {
                    if convert {
                        convert_colors(doc, &before, &after);
                    }
                    doc.settings.color = after.clone();
                });
            }
        }
    }
}

/// Convert every color of the document from one set of color profiles to another, keeping
/// their look: CMYK inks from the old CMYK profile to the new one, RGB values from the old RGB
/// profile to the new one.
pub fn convert_colors(doc: &mut Document, from: &ColorSettings, to: &ColorSettings) {
    let (a, b) = (cms::engine(from), cms::engine(to));
    // bitmaps are not serialized: set them aside
    let images = std::mem::take(&mut doc.images);
    if let Ok(mut v) = serde_json::to_value(&*doc) {
        walk(&mut v, &mut |m| {
            let get = |k: &str| m.get(k).and_then(|x| x.as_u64()).map(|x| x.min(255) as u8);
            let (Some(r), Some(g), Some(bl), Some(al)) = (get("r"), get("g"), get("b"), get("a")) else { return };
            let ink = m.get("cmyk").and_then(|k| serde_json::from_value::<[u8; 4]>(k.clone()).ok());
            let c = match ink {
                Some(k) => {
                    let inks = a.cmyk_to_other(k.map(|x| x as f64), &b);
                    b.cmyk_to_rgb(inks).with_cmyk(inks).with_alpha(al)
                }
                None => a.rgb_to_other(vector_core::Color::rgba(r, g, bl, al), &b),
            };
            if let Ok(serde_json::Value::Object(n)) = serde_json::to_value(c) {
                *m = n;
            }
        });
        if let Ok(d) = serde_json::from_value::<Document>(v) {
            *doc = d;
        }
    }
    doc.images = images;
}

/// Call `f` on every JSON object that looks like a color (r, g, b and a, and maybe cmyk).
fn walk(v: &mut serde_json::Value, f: &mut impl FnMut(&mut serde_json::Map<String, serde_json::Value>)) {
    match v {
        serde_json::Value::Object(m) => {
            let is_color = m.len() <= 5 && ["r", "g", "b", "a"].iter().all(|k| m.get(*k).is_some_and(|x| x.is_u64())) && m.keys().all(|k| matches!(k.as_str(), "r" | "g" | "b" | "a" | "cmyk"));
            if is_color {
                f(m);
            } else {
                for x in m.values_mut() {
                    walk(x, f);
                }
            }
        }
        serde_json::Value::Array(a) => {
            for x in a.iter_mut() {
                walk(x, f);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_core::{Fill, ObjKind, Object, Style};

    #[test]
    fn settings_round_trip_and_old_files_load() {
        let mut s = crate::settings::Settings::default();
        assert_eq!(s.color_management.preset, "North America General Purpose");
        s.color_management.apply(&preset("Simulate Color Management Off").unwrap());
        s.color_management.options.preserve_pure_black = true;
        s.color_management.user_presets.push(s.color_management.set("Mine"));
        let j = serde_json::to_string(&s).unwrap();
        let back: crate::settings::Settings = serde_json::from_str(&j).unwrap();
        assert_eq!(back.color_management, s.color_management);
        assert_eq!(back.color_management.options.engine, ColorEngine::None);
        // a settings file from before color management loads with the factory defaults
        let mut v: serde_json::Value = serde_json::from_str(&j).unwrap();
        v.as_object_mut().unwrap().remove("color_management");
        let old: crate::settings::Settings = serde_json::from_value(v).unwrap();
        assert_eq!(old.color_management, CmPrefs::default());
        // and a partial one too
        let partial: CmPrefs = serde_json::from_str(r#"{"options":{"engine":"None"}}"#).unwrap();
        assert_eq!(partial.options.engine, ColorEngine::None);
        assert!(partial.options.map_gray_to_black);
    }

    #[test]
    fn presets_follow_coreldraw() {
        assert_eq!(PRESETS.len(), 12);
        let off = preset("Simulate Color Management Off").unwrap();
        assert!(off.options.off());
        assert_eq!(off.defaults.cmyk_profile, "Wide Gamut CMYK Simulation");
        let eu = preset("Europe Prepress").unwrap();
        assert_eq!((eu.defaults.rgb_profile.as_str(), eu.defaults.cmyk_profile.as_str(), eu.defaults.cmyk), ("Adobe RGB (1998)", "ISO Coated v2 (ECI)", true));
        assert_eq!(eu.import.cmyk, PolicyAction::AssignDocument);
        assert_eq!(CmSet::default().defaults, ColorSettings::default());
    }

    #[test]
    fn converting_keeps_the_look_and_assigning_keeps_the_numbers() {
        cms::without_installed_profiles(|| {
            let from = ColorSettings::default();
            let to = ColorSettings { cmyk_profile: "Japan Color 2001 Coated".into(), rgb_profile: "Adobe RGB (1998)".into(), ..ColorSettings::default() };
            let mut doc = Document::new(100.0, 100.0);
            let ink = cms::with_settings(&from, || vector_core::Color::from_cmyk(20.0, 60.0, 10.0, 5.0));
            for c in [ink, vector_core::Color::rgb(0, 200, 0)] {
                let id = doc.new_id();
                let o = Object::new(id, ObjKind::Rect(vector_core::object::RectShape { w: 10.0, h: 10.0, radii: [0.0; 4], corner: Default::default() }), Default::default(), Style::filled(c));
                doc.pages[0].layers[0].objects.push(o);
            }
            convert_colors(&mut doc, &from, &to);
            let fills: Vec<vector_core::Color> = doc.pages[0].layers[0].objects.iter().filter_map(|o| if let Fill::Solid(c) = o.style.fill { Some(c) } else { None }).collect();
            assert_eq!(fills.len(), 2);
            assert!(fills[0].cmyk.is_some() && fills[0].cmyk != ink.cmyk, "{:?}", fills[0]);
            let (old, new) = (cms::engine(&from).cmyk_to_display(ink.to_cmyk()), cms::engine(&to).cmyk_to_display(fills[0].to_cmyk()));
            assert!(old.r.abs_diff(new.r) <= 8 && old.g.abs_diff(new.g) <= 8 && old.b.abs_diff(new.b) <= 8, "{old:?} {new:?}");
            // sRGB green in Adobe RGB
            assert!(fills[1].cmyk.is_none() && fills[1].r > 60 && fills[1].g < 200, "{:?}", fills[1]);
        });
    }
}
