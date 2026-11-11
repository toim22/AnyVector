//! Interface themes (Light and Dark, as in CorelDRAW 2026), fonts and text styles.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};
use std::cell::Cell;

/// The colors of one interface theme (Tools ▸ Options ▸ Customization ▸ Appearance ▸ Theme).
/// Canvas colors (selection handles, guides) are the same in every theme.
pub struct Palette {
    pub name: &'static str,
    pub light: bool,
    pub bg: Color32,
    pub bar: Color32,
    pub bar_dark: Color32,
    pub field: Color32,
    pub sep: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub disabled: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub accent: Color32,
    pub focus: Color32,
    pub menu_bg: Color32,
    pub desktop: Color32,
    pub ruler: Color32,
    pub canvas_tab: Color32,
    /// Strongest text: hovered and pressed widget labels, headings.
    pub strong: Color32,
    /// Push-button face.
    pub button: Color32,
    /// Border of a hovered widget.
    pub hover_stroke: Color32,
    /// Alternate-row and faint panel fill.
    pub faint: Color32,
    pub hyperlink: Color32,
    /// Fill of default (OK) and call-to-action buttons; their text is white in every theme.
    pub primary: Color32,
    /// Active docker tab.
    pub tab_active: Color32,
    pub shadow_alpha: u8,
}

pub const DARK: Palette = Palette {
    name: "Dark",
    light: false,
    bg: Color32::from_rgb(0x26, 0x26, 0x26),
    bar: Color32::from_rgb(0x2b, 0x2b, 0x2b),
    bar_dark: Color32::from_rgb(0x23, 0x23, 0x23),
    field: Color32::from_rgb(0x1f, 0x1f, 0x1f),
    sep: Color32::from_rgb(0x3c, 0x3c, 0x3c),
    text: Color32::from_rgb(0xd4, 0xd4, 0xd4),
    text_dim: Color32::from_rgb(0x9a, 0x9a, 0x9a),
    disabled: Color32::from_rgb(0x84, 0x84, 0x84),
    hover: Color32::from_rgb(0x3a, 0x3a, 0x3a),
    pressed: Color32::from_rgb(0x1f, 0x4e, 0x8c),
    accent: Color32::from_rgb(0x1d, 0x4a, 0x85),
    focus: Color32::from_rgb(0x0a, 0x6f, 0xd6),
    menu_bg: Color32::from_rgb(0x2b, 0x2b, 0x2b),
    desktop: Color32::from_rgb(0x26, 0x26, 0x26),
    ruler: Color32::from_rgb(0x2e, 0x2e, 0x2e),
    canvas_tab: Color32::from_rgb(0x33, 0x33, 0x33),
    strong: Color32::WHITE,
    button: Color32::from_rgb(0x33, 0x33, 0x33),
    hover_stroke: Color32::from_rgb(0x50, 0x50, 0x50),
    faint: Color32::from_rgb(0x30, 0x30, 0x30),
    hyperlink: Color32::from_rgb(0x4d, 0xa3, 0xff),
    primary: Color32::from_rgb(0x1d, 0x4a, 0x85),
    tab_active: Color32::from_rgb(29, 68, 112),
    shadow_alpha: 110,
};

/// Modelled on CorelDRAW 2026's Light theme: light-gray bars, white fields, near-black text,
/// pale-blue selection.
pub const LIGHT: Palette = Palette {
    name: "Light",
    light: true,
    bg: Color32::from_rgb(0xe9, 0xe9, 0xe9),
    bar: Color32::from_rgb(0xf0, 0xf0, 0xf0),
    bar_dark: Color32::from_rgb(0xe4, 0xe4, 0xe4),
    field: Color32::from_rgb(0xff, 0xff, 0xff),
    sep: Color32::from_rgb(0xc8, 0xc8, 0xc8),
    text: Color32::from_rgb(0x1e, 0x1e, 0x1e),
    text_dim: Color32::from_rgb(0x5c, 0x5c, 0x5c),
    disabled: Color32::from_rgb(0xa0, 0xa0, 0xa0),
    hover: Color32::from_rgb(0xdc, 0xdc, 0xdc),
    pressed: Color32::from_rgb(0xb4, 0xd2, 0xf2),
    accent: Color32::from_rgb(0xcc, 0xe2, 0xf8),
    focus: Color32::from_rgb(0x0a, 0x6f, 0xd6),
    menu_bg: Color32::from_rgb(0xf7, 0xf7, 0xf7),
    desktop: Color32::from_rgb(0xd9, 0xd9, 0xd9),
    ruler: Color32::from_rgb(0xf7, 0xf7, 0xf7),
    canvas_tab: Color32::from_rgb(0xdc, 0xdc, 0xdc),
    strong: Color32::BLACK,
    button: Color32::from_rgb(0xe1, 0xe1, 0xe1),
    hover_stroke: Color32::from_rgb(0xa8, 0xa8, 0xa8),
    faint: Color32::from_rgb(0xf4, 0xf4, 0xf4),
    hyperlink: Color32::from_rgb(0x00, 0x5f, 0xc4),
    primary: Color32::from_rgb(0x1a, 0x6c, 0xc8),
    tab_active: Color32::from_rgb(0xcc, 0xe2, 0xf8),
    shadow_alpha: 60,
};

/// Theme names as offered in Options (and stored in `Settings::theme_name`), in CorelDRAW's order.
pub const THEMES: [&Palette; 2] = [&LIGHT, &DARK];

thread_local! {
    /// Index in `THEMES` of the active theme. Per thread: the interface is drawn on one thread,
    /// and headless tests running side by side each keep their own theme.
    static CURRENT: Cell<u8> = const { Cell::new(1) };
}

/// The active palette.
pub fn pal() -> &'static Palette {
    THEMES[(CURRENT.with(Cell::get) as usize).min(THEMES.len() - 1)]
}

/// The active theme's index in `THEMES`; changes whenever the theme does (icon caches key on it).
pub fn index() -> u8 {
    CURRENT.with(Cell::get)
}

pub fn is_light() -> bool {
    pal().light
}

/// Switches to the theme called `name` (unknown names fall back to Dark) and restyles `ctx`.
pub fn select(ctx: &egui::Context, name: &str) {
    let i = THEMES.iter().position(|p| p.name.eq_ignore_ascii_case(name)).unwrap_or(1);
    CURRENT.with(|c| c.set(i as u8));
    apply_visuals(ctx);
}

/// Switches to `name` when it is another known theme (called every frame, like the language);
/// returns the palette it switched from.
pub fn sync(ctx: &egui::Context, name: &str) -> Option<&'static Palette> {
    let old = pal();
    if !old.name.eq_ignore_ascii_case(name) && THEMES.iter().any(|p| p.name.eq_ignore_ascii_case(name)) {
        select(ctx, name);
        return Some(old);
    }
    None
}

/// A desktop color left at `from`'s default follows the active theme's.
pub fn follow_desktop(color: &mut [u8; 3], from: &Palette) {
    let rgb = |c: Color32| [c.r(), c.g(), c.b()];
    if *color == rgb(from.desktop) {
        *color = rgb(pal().desktop);
    }
}

macro_rules! colors {
    ($($name:ident => $field:ident),* $(,)?) => {
        $(
            #[allow(non_snake_case, dead_code)]
            #[inline]
            pub fn $name() -> Color32 {
                pal().$field
            }
        )*
    };
}

colors! {
    BG => bg, BAR => bar, BAR_DARK => bar_dark, FIELD => field, SEP => sep, TEXT => text,
    TEXT_DIM => text_dim, DISABLED => disabled, HOVER => hover, PRESSED => pressed,
    ACCENT => accent, FOCUS => focus, MENU_BG => menu_bg, DESKTOP => desktop, RULER => ruler,
    BG_CANVAS_TAB => canvas_tab, STRONG => strong, BUTTON => button, HOVER_STROKE => hover_stroke,
    FAINT => faint, PRIMARY => primary, TAB_ACTIVE => tab_active,
}

/// A gray that was picked for the Dark theme, carried over to the active theme: unchanged in
/// Dark, mirrored in Light (dark fills turn light, light text turns dark) so it keeps its role.
pub fn gray(v: u8) -> Color32 {
    if !is_light() {
        return Color32::from_gray(v);
    }
    // dark-theme gray -> light-theme gray, through the palette's own pairs
    const MAP: [(f32, f32); 10] = [
        (0.0, 255.0),
        (38.0, 240.0),
        (51.0, 225.0),
        (58.0, 216.0),
        (80.0, 176.0),
        (96.0, 160.0),
        (154.0, 92.0),
        (212.0, 30.0),
        (240.0, 16.0),
        (255.0, 0.0),
    ];
    let x = v as f32;
    let mut out = 0.0;
    for w in MAP.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x <= x1 {
            out = y0 + (y1 - y0) * (x - x0) / (x1 - x0);
            break;
        }
    }
    Color32::from_gray(out.round().clamp(0.0, 255.0) as u8)
}

pub const SELECTION: Color32 = Color32::from_rgb(0x1e, 0x90, 0xff);
pub const GUIDE: Color32 = Color32::from_rgb(0x00, 0x99, 0xff);

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    apply_visuals(ctx);
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
        // lists end with a hard edge, as in the reference (no faded last row)
        s.spacing.scroll.fade.strength = 0.0;
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

fn apply_visuals(ctx: &egui::Context) {
    let p = pal();
    let mut v = if p.light { Visuals::light() } else { Visuals::dark() };
    v.panel_fill = p.bar;
    v.window_fill = p.menu_bg;
    v.extreme_bg_color = p.field;
    v.faint_bg_color = p.faint;
    v.override_text_color = Some(p.text);
    v.window_stroke = Stroke::new(1.0, p.sep);
    v.window_corner_radius = CornerRadius::same(2);
    v.menu_corner_radius = CornerRadius::same(0);
    v.window_shadow = egui::epaint::Shadow { offset: [2, 3], blur: 8, spread: 0, color: Color32::from_black_alpha(p.shadow_alpha) };
    v.popup_shadow = egui::epaint::Shadow { offset: [2, 3], blur: 6, spread: 0, color: Color32::from_black_alpha(p.shadow_alpha) };
    v.selection.bg_fill = p.focus;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.hyperlink_color = p.hyperlink;
    let r = CornerRadius::same(2);
    v.widgets.noninteractive.bg_fill = p.bar;
    v.widgets.noninteractive.weak_bg_fill = p.bar;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.sep);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.bg_fill = p.field;
    v.widgets.inactive.weak_bg_fill = p.button;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.hover_stroke);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.strong);
    v.widgets.hovered.corner_radius = r;
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.bg_fill = p.pressed;
    v.widgets.active.weak_bg_fill = p.pressed;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.focus);
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.strong);
    v.widgets.active.corner_radius = r;
    v.widgets.active.expansion = 0.0;
    v.widgets.open.bg_fill = p.hover;
    v.widgets.open.weak_bg_fill = p.hover;
    v.widgets.open.bg_stroke = Stroke::NONE;
    v.widgets.open.fg_stroke = Stroke::new(1.0, p.strong);
    v.widgets.open.corner_radius = r;
    v.striped = false;
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.set_theme(if p.light { egui::ThemePreference::Light } else { egui::ThemePreference::Dark });
}

pub(crate) fn install_fonts(ctx: &egui::Context) {
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
    crate::i18n::add_fonts(&mut defs, &fonts);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_themes_changes_the_palette() {
        let ctx = egui::Context::default();
        select(&ctx, "Light");
        assert!(is_light());
        assert_eq!(BAR(), LIGHT.bar);
        assert_eq!(ctx.theme(), egui::Theme::Light);
        // a known name switches and reports the old palette; the same or an unknown one does not
        assert!(sync(&ctx, "Light").is_none());
        assert!(sync(&ctx, "Purple").is_none());
        assert_eq!(sync(&ctx, "dark").map(|p| p.name), Some("Light"));
        assert!(!is_light());
        assert_eq!(BAR(), DARK.bar);
    }

    #[test]
    fn desktop_follows_only_when_left_at_the_default() {
        let ctx = egui::Context::default();
        select(&ctx, "Light");
        let mut c = [0x26, 0x26, 0x26];
        follow_desktop(&mut c, &DARK);
        assert_eq!(c, [LIGHT.desktop.r(); 3]);
        let mut custom = [0x40, 0x10, 0x10];
        follow_desktop(&mut custom, &DARK);
        assert_eq!(custom, [0x40, 0x10, 0x10]);
        select(&ctx, "Dark");
    }

    #[test]
    fn grays_mirror_in_the_light_theme() {
        let ctx = egui::Context::default();
        select(&ctx, "Dark");
        assert_eq!(gray(0x50), Color32::from_gray(0x50));
        select(&ctx, "Light");
        // dark fills turn light, light text turns dark, and the order is kept
        assert!(gray(0x26).r() > 0xe0);
        assert!(gray(0xd4).r() < 0x40);
        assert!(gray(0x40).r() > gray(0x60).r());
        select(&ctx, "Dark");
    }

    #[test]
    fn theme_names_have_their_own_pt_br_words() {
        let tsv = include_str!("../assets/i18n/pt-BR.tsv");
        for p in THEMES {
            let key = format!("{}|theme\t", p.name);
            assert!(tsv.lines().any(|l| l.starts_with(&key)), "{key}");
        }
    }
}
