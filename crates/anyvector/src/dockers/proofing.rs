//! Color Proofing docker: soft proofing of the view (the proofing environment, the rendering
//! intent and the gamut warning), proof presets, and exporting or printing the soft proof.

use crate::app::AnyVectorApp;
use crate::settings::ProofPreset;
use crate::theme;
use egui::{vec2, Ui};
use vector_core::Color;

/// A proofing environment: its name and color space (0 CMYK, 1 RGB, 2 Grayscale).
#[derive(Clone, PartialEq)]
pub struct Env {
    pub name: String,
    pub kind: u8,
}

pub const INTENTS: [&str; 4] = ["Relative colorimetric", "Absolute colorimetric", "Perceptual", "Saturation"];

/// The installed color profiles (Windows and CorelDRAW color folders, see `vector_core::cms`):
/// description and color space.
pub fn installed_profiles() -> Vec<Env> {
    let mut v: Vec<Env> = vector_core::cms::installed_profiles()
        .iter()
        .filter(|p| matches!(&p.class, b"prtr" | b"mntr" | b"spac"))
        .map(|p| Env { name: p.description.clone(), kind: match p.space {
            "CMYK" => 0,
            "RGB" => 1,
            _ => 2,
        } })
        .collect();
    v.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    v.dedup();
    v
}

/// The rows of the Simulate environment list: (row, header?). The document's and the default
/// profiles come first.
fn env_rows(app: &AnyVectorApp) -> Vec<(Option<Env>, &'static str)> {
    let def = &app.settings.color_management.defaults;
    let doc = app.doc().map(|d| d.doc.settings.color.clone()).unwrap_or_else(|| def.clone());
    let mut v: Vec<(Option<Env>, &'static str)> = vec![
        (Some(Env { name: format!("(Document) {}", doc.cmyk_profile), kind: 0 }), ""),
        (Some(Env { name: format!("(Document) {}", doc.gray_profile), kind: 2 }), ""),
        (None, "Default profiles"),
        (Some(Env { name: format!("(Default) {}", def.rgb_profile), kind: 1 }), ""),
        (Some(Env { name: format!("(Default) {}", def.cmyk_profile), kind: 0 }), ""),
        (Some(Env { name: format!("(Default) {}", def.gray_profile), kind: 2 }), ""),
    ];
    let profiles = &app.dockers.ui.proof_profiles;
    for (k, head) in [(0u8, "CMYK output profiles"), (1, "RGB output profiles"), (2, "Grayscale output profiles")] {
        let list: Vec<&Env> = profiles.iter().filter(|e| e.kind == k).collect();
        if !list.is_empty() {
            v.push((None, head));
            v.extend(list.into_iter().map(|e| (Some(e.clone()), "")));
        }
    }
    v
}

/// The soft proof of the proofing environment in the settings, for a document's color
/// settings: its profile, through the Color Proofing rendering intent.
pub fn proof_lut(s: &crate::settings::Settings, doc: &vector_core::document::ColorSettings) -> Option<std::sync::Arc<vector_core::cms::ProofLut>> {
    use vector_core::document::RenderingIntent as I;
    let intent = match s.proof_intent {
        0 => I::RelativeColorimetric,
        1 => I::AbsoluteColorimetric,
        2 => I::Perceptual,
        _ => I::Saturation,
    };
    // "(Document) ...": the profile of the document shown, whichever it is
    let env = if s.proof_env.starts_with("(Document) ") {
        if s.proof_env_kind == 2 { doc.gray_profile.as_str() } else { doc.cmyk_profile.as_str() }
    } else {
        short(&s.proof_env)
    };
    let env = if env.trim().is_empty() { doc.cmyk_profile.as_str() } else { env };
    Some(vector_core::cms::engine(doc).proof(env, intent))
}

/// The environment's name without the (Document)/(Default) mark.
fn short(name: &str) -> &str {
    name.strip_prefix("(Document) ").or_else(|| name.strip_prefix("(Default) ")).unwrap_or(name)
}

fn describe(app: &mut AnyVectorApp, resp: &egui::Response, text: &'static str) {
    if resp.hovered() {
        app.dockers.ui.proof_desc = Some(text);
    }
}

pub fn proofing(app: &mut AnyVectorApp, ui: &mut Ui) {
    if app.dockers.ui.proof_profiles.is_empty() {
        app.dockers.ui.proof_profiles = installed_profiles();
        if app.dockers.ui.proof_profiles.is_empty() {
            app.dockers.ui.proof_profiles.push(Env { name: String::new(), kind: 9 });
        }
    }
    let w = ui.available_width() - 6.0;
    app.dockers.ui.proof_desc = None;
    let s = &app.settings;
    let mut proof = s.proof_colors;
    let r = ui.checkbox(&mut proof, "Proof colors");
    if r.changed() {
        app.settings.proof_colors = proof;
    }
    describe(app, &r, "Show the colors of the document as they will look in the proofing environment.");
    // Gamut warning
    ui.horizontal(|ui| {
        ui.label("Gamut warning");
        let rr = ui.available_rect_before_wrap();
        ui.painter().line_segment([egui::pos2(rr.left() + 4.0, rr.center().y), egui::pos2(rr.right() - 4.0, rr.center().y)], egui::Stroke::new(1.0, theme::SEP()));
    });
    let mut gamut = app.settings.proof_gamut;
    let r = ui.checkbox(&mut gamut, "Out-of-gamut colors");
    if r.changed() {
        app.settings.proof_gamut = gamut;
    }
    describe(app, &r, "Highlight the colors that the proofing environment cannot reproduce.");
    ui.horizontal(|ui| {
        let mut c = Some(app.settings.proof_gamut_color);
        let r = ui.scope(|ui| super::color_pick_w(ui, "proof_gamut_color", &mut c, false, true, 30.0));
        if r.inner {
            if let Some(c) = c {
                app.settings.proof_gamut_color = c;
            }
        }
        describe(app, &r.response, "Choose the color of the out-of-gamut overlay.");
        ui.add_space(6.0);
        if let Some(t) = app.icons.get(ui.ctx(), "transparency", 16.0, crate::icons::Tone::Normal) {
            ui.add(egui::Image::new(t).fit_to_exact_size(vec2(16.0, 16.0)));
        }
        let mut a = app.settings.proof_gamut_transparency;
        let r = ui.scope(|ui| super::spin(ui, &mut a, 0..=100, 1.0, 0, "", 46.0));
        if r.inner {
            app.settings.proof_gamut_transparency = a;
        }
        ui.label("%");
        describe(app, &r.response, "Set the transparency of the out-of-gamut overlay.");
    });
    ui.add_space(4.0);
    // Proof preset
    ui.label("Proof preset:");
    ui.horizontal(|ui| {
        let presets = app.settings.proof_presets.clone();
        let cur = app.settings.proof_preset.clone();
        let label = if cur.is_empty() { "Custom".to_string() } else { cur.clone() };
        let r = egui::ComboBox::from_id_salt("proof_preset").width(w - 64.0).selected_text(label).show_ui(ui, |ui| {
            if ui.add(egui::Button::selectable(cur.is_empty(), "Custom")).clicked() {
                app.settings.proof_preset.clear();
            }
            for p in &presets {
                if ui.add(egui::Button::selectable(cur == p.name, &p.name)).clicked() {
                    app.settings.proof_preset = p.name.clone();
                    app.settings.proof_env = p.env.clone();
                    app.settings.proof_env_kind = p.kind;
                    app.settings.proof_preserve = p.preserve;
                    app.settings.proof_intent = p.intent;
                }
            }
        });
        describe(app, &r.response, "Choose a proof preset.");
        let save = crate::widgets::icon_button(&mut app.icons, ui, "save", "Save all current proof settings as a proof preset. Out-of-gamut settings are not included in the proof preset.", true, false);
        describe(app, &save, "Save all current proof settings as a proof preset. Out-of-gamut settings are not included in the proof preset.");
        if save.clicked() {
            app.dockers.ui.proof_new_preset = Some(format!("Proof preset {}", app.settings.proof_presets.len() + 1));
        }
        let del_on = !cur.is_empty();
        let del = crate::widgets::icon_button(&mut app.icons, ui, "delete", "Delete the selected proof preset.", del_on, false);
        describe(app, &del, "Delete the selected proof preset.");
        if del.clicked() && del_on {
            app.settings.proof_presets.retain(|p| p.name != cur);
            app.settings.proof_preset.clear();
        }
    });
    if let Some(mut name) = app.dockers.ui.proof_new_preset.clone() {
        let mut done = None;
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(w - 64.0));
            r.request_focus();
            if r.lost_focus() {
                done = Some(ui.input(|i| i.key_pressed(egui::Key::Enter)));
            }
        });
        app.dockers.ui.proof_new_preset = Some(name.clone());
        if let Some(ok) = done {
            if ok && !name.trim().is_empty() {
                let s = &mut app.settings;
                let p = ProofPreset { name: name.trim().to_string(), env: s.proof_env.clone(), kind: s.proof_env_kind, preserve: s.proof_preserve, intent: s.proof_intent };
                s.proof_presets.retain(|x| x.name != p.name);
                s.proof_preset = p.name.clone();
                s.proof_presets.push(p);
            }
            app.dockers.ui.proof_new_preset = None;
        }
    }
    let custom = |app: &mut AnyVectorApp| app.settings.proof_preset.clear();
    // Simulate environment
    ui.add_space(4.0);
    ui.label("Simulate environment:");
    let rows = env_rows(app);
    let cur_env = app.settings.proof_env.clone();
    let r = egui::ComboBox::from_id_salt("proof_env").width(w).height(520.0).selected_text(short(&cur_env).to_string()).show_ui(ui, |ui| {
        for (row, head) in &rows {
            match row {
                None => {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(*head).color(theme::FOCUS()));
                        let rr = ui.available_rect_before_wrap();
                        ui.painter().line_segment([egui::pos2(rr.left() + 4.0, rr.center().y), egui::pos2(rr.right(), rr.center().y)], egui::Stroke::new(1.0, theme::SEP()));
                    });
                }
                Some(e) if e.kind <= 2 => {
                    if ui.add(egui::Button::selectable(cur_env == e.name, &e.name)).clicked() {
                        app.settings.proof_env = e.name.clone();
                        app.settings.proof_env_kind = e.kind;
                        custom(app);
                    }
                }
                _ => {}
            }
        }
    });
    describe(app, &r.response, "Choose the printer, press or display whose colors the soft proof simulates.");
    let kind = app.settings.proof_env_kind;
    let mut keep = app.settings.proof_preserve;
    let what = ["CMYK", "RGB", "Grayscale"][kind.min(2) as usize];
    let r = ui.checkbox(&mut keep, format!("Preserve {what} numbers"));
    if r.changed() {
        app.settings.proof_preserve = keep;
        custom(app);
    }
    describe(
        app,
        &r,
        match kind {
            1 => "Keep the RGB color values of the document in the soft proof. RGB colors will not be converted from the document color profile to the color profile of the proofing environment.",
            2 => "Keep the Grayscale color values of the document in the soft proof. Grayscale colors will not be converted from the document color profile to the color profile of the proofing environment.",
            _ => "Keep the CMYK color values of the document in the soft proof. CMYK colors will not be converted from the document color profile to the color profile of the proofing environment.",
        },
    );
    // Rendering intent
    ui.add_space(4.0);
    ui.label("Rendering intent:");
    let mut intent = app.settings.proof_intent as usize;
    let r = egui::ComboBox::from_id_salt("proof_intent").width(w).selected_text(INTENTS[intent.min(3)]).show_ui(ui, |ui| {
        for (k, t) in INTENTS.iter().enumerate() {
            if ui.add(egui::Button::selectable(k == intent, *t)).clicked() {
                intent = k;
            }
        }
    });
    if intent as u8 != app.settings.proof_intent {
        app.settings.proof_intent = intent as u8;
        custom(app);
    }
    describe(app, &r.response, "The rendering intent determines how out-of-gamut colors are interpreted in the soft proof.");
    // Export and print
    ui.add_space(8.0);
    let on = app.settings.proof_colors && app.has_doc();
    let r = ui.add_enabled(on, egui::Button::new("Export soft proof...").min_size(vec2(126.0, 22.0)));
    describe(app, &r, "Save the soft proof as a JPG, TIFF, Adobe Portable Document Format (PDF), or Corel PHOTO-PAINT (CPT ) file. Note that the colors of the saved proof will be different from the colors of the active document.");
    if r.clicked() {
        export_proof(app);
    }
    let r = ui.add_enabled(on, egui::Button::new("Print proof...").min_size(vec2(126.0, 22.0)));
    describe(app, &r, "Print the document with the colors of the proofing environment.");
    if r.clicked() {
        let mut st = crate::dialogs::print::PrintState::new(app, false);
        st.settings.color_proof = true;
        app.dialogs.print = Some(st);
    }
    // Description
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label("Description");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let on = app.settings.proof_show_desc;
            if crate::widgets::icon_button(&mut app.icons, ui, "help", "Show or hide the description.", true, false).clicked() {
                app.settings.proof_show_desc = !on;
            }
            let rr = ui.available_rect_before_wrap();
            ui.painter().line_segment([egui::pos2(rr.left() + 4.0, rr.center().y), egui::pos2(rr.right() - 4.0, rr.center().y)], egui::Stroke::new(1.0, theme::SEP()));
        });
    });
    if app.settings.proof_show_desc {
        let t = app.dockers.ui.proof_desc.or(app.dockers.ui.proof_last_desc).unwrap_or("Point to a control to learn how it affects colors in the file or when you should use it.");
        if app.dockers.ui.proof_desc.is_some() {
            app.dockers.ui.proof_last_desc = app.dockers.ui.proof_desc;
        }
        ui.add(egui::Label::new(egui::RichText::new(t).color(theme::TEXT())).wrap());
    }
}

/// The out-of-gamut overlay: the gamut color with the docker's transparency.
pub fn gamut_overlay(app: &AnyVectorApp) -> Option<[u8; 4]> {
    if !app.settings.proof_gamut || app.settings.proof_env_kind != 0 {
        return None;
    }
    let c: Color = app.settings.proof_gamut_color;
    let a = ((100 - app.settings.proof_gamut_transparency.min(100)) as f32 * 2.55).round() as u8;
    Some([c.r, c.g, c.b, a])
}

/// Export soft proof: the page as PNG, JPG or TIFF in the colors of the proofing environment.
fn export_proof(app: &mut AnyVectorApp) {
    let Some(d) = app.doc() else { return };
    let Some(path) = rfd::FileDialog::new().set_title("Export Soft Proof").add_filter("PNG", &["png"]).add_filter("JPG", &["jpg", "jpeg"]).add_filter("TIFF", &["tif", "tiff"]).set_file_name(format!("{} proof.png", d.display_name())).save_file() else { return };
    let Some(mut pm) = vector_render::render_page_image(&d.doc, d.page, 150.0, Some(Color::WHITE), None, true) else { return };
    let lut = proof_lut(&app.settings, &d.doc.settings.color);
    crate::canvas::proof_pixmap(&mut pm, lut.as_deref(), true, None, [0, 0, 0, 0]);
    let img = image::RgbaImage::from_raw(pm.width(), pm.height(), pm.data().to_vec());
    let ok = img.map(|i| image::DynamicImage::ImageRgba8(i).to_rgb8().save(&path).is_ok()).unwrap_or(false);
    if !ok {
        app.flash("The soft proof could not be saved.");
    }
}
