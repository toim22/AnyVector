//! Dark theme matching the reference screenshots.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};

pub const BG: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);
pub const BAR: Color32 = Color32::from_rgb(0x2b, 0x2b, 0x2b);
pub const BAR_DARK: Color32 = Color32::from_rgb(0x23, 0x23, 0x23);
pub const FIELD: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x1f);
pub const SEP: Color32 = Color32::from_rgb(0x3c, 0x3c, 0x3c);
pub const TEXT: Color32 = Color32::from_rgb(0xd4, 0xd4, 0xd4);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x9a, 0x9a, 0x9a);
pub const DISABLED: Color32 = Color32::from_rgb(0x6e, 0x6e, 0x6e);
pub const HOVER: Color32 = Color32::from_rgb(0x3a, 0x3a, 0x3a);
pub const PRESSED: Color32 = Color32::from_rgb(0x1f, 0x4e, 0x8c);
pub const ACCENT: Color32 = Color32::from_rgb(0x1d, 0x4a, 0x85);
pub const FOCUS: Color32 = Color32::from_rgb(0x0a, 0x6f, 0xd6);
pub const MENU_BG: Color32 = Color32::from_rgb(0x2b, 0x2b, 0x2b);
pub const DESKTOP: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);
pub const RULER: Color32 = Color32::from_rgb(0x2e, 0x2e, 0x2e);
pub const SELECTION: Color32 = Color32::from_rgb(0x1e, 0x90, 0xff);
pub const NODE_BLUE: Color32 = Color32::from_rgb(0x00, 0x66, 0xff);
pub const NODE_RED: Color32 = Color32::from_rgb(0xff, 0x33, 0x33);
pub const GUIDE: Color32 = Color32::from_rgb(0x00, 0x99, 0xff);
pub const BG_CANVAS_TAB: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    let mut v = Visuals::dark();
    v.panel_fill = BAR;
    v.window_fill = MENU_BG;
    v.extreme_bg_color = FIELD;
    v.faint_bg_color = Color32::from_rgb(0x30, 0x30, 0x30);
    v.override_text_color = Some(TEXT);
    v.window_stroke = Stroke::new(1.0, SEP);
    v.window_corner_radius = CornerRadius::same(2);
    v.menu_corner_radius = CornerRadius::same(0);
    v.window_shadow = egui::epaint::Shadow { offset: [2, 3], blur: 8, spread: 0, color: Color32::from_black_alpha(110) };
    v.popup_shadow = egui::epaint::Shadow { offset: [2, 3], blur: 6, spread: 0, color: Color32::from_black_alpha(110) };
    v.selection.bg_fill = FOCUS;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.hyperlink_color = Color32::from_rgb(0x4d, 0xa3, 0xff);
    let r = CornerRadius::same(2);
    v.widgets.noninteractive.bg_fill = BAR;
    v.widgets.noninteractive.weak_bg_fill = BAR;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, SEP);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.bg_fill = FIELD;
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x33, 0x33, 0x33);
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.hovered.weak_bg_fill = HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x50, 0x50, 0x50));
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.hovered.corner_radius = r;
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.bg_fill = PRESSED;
    v.widgets.active.weak_bg_fill = PRESSED;
    v.widgets.active.bg_stroke = Stroke::new(1.0, FOCUS);
    v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.active.corner_radius = r;
    v.widgets.active.expansion = 0.0;
    v.widgets.open.bg_fill = HOVER;
    v.widgets.open.weak_bg_fill = HOVER;
    v.widgets.open.bg_stroke = Stroke::NONE;
    v.widgets.open.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.open.corner_radius = r;
    v.striped = false;
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 3.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
        s.spacing.interact_size = egui::vec2(24.0, 20.0);
        s.spacing.menu_margin = egui::Margin::same(2);
        s.spacing.window_margin = egui::Margin::same(8);
        s.spacing.icon_width = 13.0;
        s.spacing.icon_spacing = 6.0;
        s.spacing.combo_height = 400.0;
        s.spacing.scroll = egui::style::ScrollStyle::thin();
        s.interaction.tooltip_delay = 0.6;
        s.interaction.selectable_labels = false;
        s.text_styles = [
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(12.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(12.5, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(15.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
        ]
        .into();
    });
}

fn install_fonts(ctx: &egui::Context) {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let fonts = std::path::Path::new(&windir).join("Fonts");
    let mut defs = egui::FontDefinitions::default();
    let mut add = |name: &str, file: &str, family: FontFamily, front: bool| {
        if let Ok(bytes) = std::fs::read(fonts.join(file)) {
            defs.font_data.insert(name.to_string(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            let list = defs.families.entry(family).or_default();
            if front {
                list.insert(0, name.to_string());
            } else {
                list.push(name.to_string());
            }
            true
        } else {
            false
        }
    };
    add("segoeui", "segoeui.ttf", FontFamily::Proportional, true);
    add("consolas", "consola.ttf", FontFamily::Monospace, true);
    add("segoeuisymbol", "seguisym.ttf", FontFamily::Proportional, false);
    if let Ok(bytes) = std::fs::read(fonts.join("segoeuib.ttf")) {
        defs.font_data.insert("segoeuib".into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        defs.families.insert(FontFamily::Name("bold".into()), vec!["segoeuib".into(), "segoeui".into()]);
    } else {
        let p = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
        defs.families.insert(FontFamily::Name("bold".into()), p);
    }
    if let Ok(bytes) = std::fs::read(fonts.join("segoeuisl.ttf")) {
        defs.font_data.insert("segoeuisl".into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        defs.families.insert(FontFamily::Name("light".into()), vec!["segoeuisl".into(), "segoeui".into()]);
    } else {
        let p = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
        defs.families.insert(FontFamily::Name("light".into()), p);
    }
    ctx.set_fonts(defs);
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn light(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("light".into()))
}

pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn to_c32(c: vector_core::Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

pub fn from_c32(c: Color32) -> vector_core::Color {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    vector_core::Color::rgba(r, g, b, a)
}
