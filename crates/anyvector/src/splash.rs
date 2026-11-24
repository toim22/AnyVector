//! Start-up splash screen (the logo), shown in the undecorated start window.

use crate::app::AnyVectorApp;
use egui::{pos2, vec2, Color32, Rect, Ui, Vec2, ViewportCommand};

const MIN_SECONDS: f64 = 1.8;

/// The size of the splash window.
pub const SPLASH_SIZE: Vec2 = vec2(620.0, 380.0);
/// The smallest the main window can be made.
pub const MIN_WINDOW_SIZE: Vec2 = vec2(480.0, 320.0);
/// The main window's size before it is maximized.
const MAIN_SIZE: Vec2 = vec2(1400.0, 860.0);

/// The window commands that turn the splash window into the main window: a title bar and frame,
/// resizable, opaque, `MAIN_SIZE` centred on the monitor (when its size is known) and maximized
/// when `maximized`.
pub fn main_window_commands(monitor_size: Option<Vec2>, maximized: bool) -> Vec<ViewportCommand> {
    let mut cmds = vec![ViewportCommand::Decorations(true), ViewportCommand::Resizable(true)];
    // Linux: winit 0.30 on Wayland makes a window that isn't resizable keep its size as its
    // minimum and maximum, and still has them once it is resizable again, so the main window
    // stayed as small as the splash and couldn't be maximized (GNOME, KDE, sway): set them again
    #[cfg(target_os = "linux")]
    cmds.extend([ViewportCommand::MinInnerSize(MIN_WINDOW_SIZE), ViewportCommand::MaxInnerSize(Vec2::INFINITY)]);
    cmds.push(ViewportCommand::Transparent(false));
    cmds.push(ViewportCommand::InnerSize(MAIN_SIZE));
    if let Some(mon) = monitor_size {
        cmds.push(ViewportCommand::OuterPosition(pos2(((mon.x - 1400.0) / 2.0).max(0.0), ((mon.y - 900.0) / 2.0).max(0.0))));
    }
    if maximized {
        cmds.push(ViewportCommand::Maximized(true));
    }
    cmds
}

pub struct Splash {
    tex: Option<egui::TextureHandle>,
    started: Option<f64>,
    switched: bool,
    status: usize,
}

impl Splash {
    pub fn new() -> Splash {
        Splash { tex: None, started: None, switched: false, status: 0 }
    }
}

const STEPS: [&str; 5] = ["Initializing...", "Loading fonts...", "Loading color palettes...", "Loading workspace...", "Starting AnyVector..."];

/// Draws the splash. Returns `true` while the splash is still showing (the main UI is skipped).
pub fn show(app: &mut AnyVectorApp, ui: &mut Ui) -> bool {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let maximized = app.settings.window_maximized;
    let Some(sp) = app.splash.as_mut() else { return false };
    let t0 = *sp.started.get_or_insert(now);
    let t = now - t0;
    if t >= MIN_SECONDS {
        if !sp.switched {
            sp.switched = true;
            // turn the splash window into the main window
            let monitor_size = ctx.input(|i| i.viewport().monitor_size);
            for cmd in main_window_commands(monitor_size, maximized) {
                ctx.send_viewport_cmd(cmd);
            }
        }
        app.splash = None;
        ctx.request_repaint();
        return false;
    }
    sp.status = ((t / MIN_SECONDS) * STEPS.len() as f64) as usize;
    let tex = sp
        .tex
        .get_or_insert_with(|| {
            let img = image::load_from_memory(crate::LOGO_PNG).map(|i| i.to_rgba8()).unwrap_or_else(|_| image::RgbaImage::new(1, 1));
            let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
            ctx.load_texture("splash", ci, egui::TextureOptions::LINEAR)
        })
        .clone();
    let status = STEPS[sp.status.min(STEPS.len() - 1)];

    let full = ui.max_rect();
    let p = ui.painter();
    // card
    let card = full.shrink(1.0);
    p.rect_filled(card, 10.0, Color32::from_rgb(0x1c, 0x1f, 0x26));
    p.rect_stroke(card, 10.0, egui::Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3f, 0x4a)), egui::StrokeKind::Inside);
    // subtle glow behind the airship
    let c = pos2(card.center().x, card.top() + card.height() * 0.40);
    for i in 0..18 {
        let r = 230.0 - i as f32 * 11.0;
        let a = (2 + i) as u8;
        p.add(egui::Shape::ellipse_filled(c, vec2(r * 1.25, r * 0.55), Color32::from_rgba_unmultiplied(0x2a, 0x7d, 0xe1, a)));
    }
    // airship (fade + gentle float)
    let fade = (t / 0.35).clamp(0.0, 1.0) as f32;
    let bob = ((t * 2.2).sin() * 3.0) as f32;
    let tw = card.width() * 0.56;
    let size = tex.size_vec2();
    let th = tw * size.y / size.x;
    let r = Rect::from_center_size(c + vec2(0.0, bob), vec2(tw, th));
    p.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::from_white_alpha((fade * 255.0) as u8));
    // name and version
    let ty = card.top() + card.height() * 0.73;
    p.text(pos2(card.center().x, ty), egui::Align2::CENTER_CENTER, vector_core::APP_NAME, crate::theme::light(34.0), Color32::from_gray(0xf2));
    p.text(pos2(card.center().x, ty + 30.0), egui::Align2::CENTER_CENTER, "Vector graphics editor", crate::theme::body(12.5), Color32::from_rgb(0x9a, 0xa3, 0xb2));
    // progress bar in logo colours
    let bar = Rect::from_min_size(pos2(card.left() + 40.0, card.bottom() - 34.0), vec2(card.width() - 80.0, 3.0));
    p.rect_filled(bar, 1.5, Color32::from_rgb(0x2c, 0x31, 0x3b));
    let k = (t / MIN_SECONDS).clamp(0.0, 1.0) as f32;
    let cols = [(0x22, 0x96, 0xf3), (0x02, 0x93, 0x44), (0x3d, 0xcf, 0x50), (0xc8, 0xdc, 0x10), (0xff, 0x7a, 0x00), (0xad, 0x1d, 0x56)];
    let filled = bar.width() * k;
    let seg = bar.width() / cols.len() as f32;
    for (i, (r_, g, b)) in cols.iter().enumerate() {
        let x0 = bar.left() + i as f32 * seg;
        let x1 = (x0 + seg).min(bar.left() + filled);
        if x1 > x0 {
            p.rect_filled(Rect::from_min_max(pos2(x0, bar.top()), pos2(x1, bar.bottom())), 0.0, Color32::from_rgb(*r_, *g, *b));
        }
    }
    p.text(pos2(bar.left(), bar.top() - 9.0), egui::Align2::LEFT_CENTER, status, crate::theme::body(11.0), Color32::from_gray(0x8c));
    p.text(pos2(bar.right(), bar.top() - 9.0), egui::Align2::RIGHT_CENTER, format!("Version {}", vector_core::APP_VERSION), crate::theme::body(11.0), Color32::from_gray(0x8c));
    ctx.request_repaint();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::ViewportCommand as C;

    #[test]
    fn splash_becomes_a_main_window_that_can_grow() {
        let cmds = main_window_commands(Some(vec2(1920.0, 1080.0)), true);
        let at = |c: &C| cmds.iter().position(|x| x == c);
        let resizable = at(&C::Resizable(true)).expect("resizable");
        if cfg!(target_os = "linux") {
            // the splash's fixed size stops being the limit, after Resizable(true) brought it back
            assert!(at(&C::MinInnerSize(MIN_WINDOW_SIZE)).expect("minimum") > resizable);
            assert!(at(&C::MaxInnerSize(Vec2::INFINITY)).expect("no maximum") > resizable);
            assert!(at(&C::InnerSize(MAIN_SIZE)).unwrap() > at(&C::MaxInnerSize(Vec2::INFINITY)).unwrap());
        } else {
            // Windows: the commands sent before AnyVector ran on Linux, unchanged
            let before = [C::Decorations(true), C::Resizable(true), C::Transparent(false), C::InnerSize(vec2(1400.0, 860.0)), C::OuterPosition(pos2(260.0, 90.0)), C::Maximized(true)];
            assert_eq!(cmds, before);
        }
        assert_eq!(at(&C::Decorations(true)), Some(0));
        assert!(cmds.contains(&C::Transparent(false)));
        // centred: 1400 × 860 plus the title bar
        assert!(cmds.contains(&C::OuterPosition(pos2(260.0, 90.0))));
        assert_eq!(cmds.last(), Some(&C::Maximized(true)));
        // monitor unknown, not maximized last time: no position, no maximize
        let plain = main_window_commands(None, false);
        assert!(!plain.iter().any(|c| matches!(c, C::OuterPosition(_) | C::Maximized(_))), "{plain:?}");
        assert!(plain.contains(&C::InnerSize(MAIN_SIZE)));
    }
}
