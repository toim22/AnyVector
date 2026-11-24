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

/// Whether the start window is see-through, for the splash's rounded corners. Not on X11: there it
/// is a window with an alpha channel (32 bits deep), and into those Mesa's software Vulkan
/// (lavapipe) draws nothing before about Mesa 25 (Ubuntu 22.04's and Debian 12's, in a virtual
/// machine or without a graphics driver). An X11 window can't lose its alpha channel later, so
/// the main window would stay black too.
pub fn transparent() -> bool {
    #[cfg(target_os = "linux")]
    return crate::platform::on_wayland();
    #[cfg(not(target_os = "linux"))]
    true
}

/// Asks for the maximized window again after these many seconds (see [`MaximizeAgain`]).
const MAXIMIZE_AGAIN: [f64; 3] = [0.2, 0.6, 1.2];

/// Linux (X11): GNOME (mutter 46) un-maximizes the main window while it adds the title bar that
/// the splash didn't have, some milliseconds after maximizing it. So AnyVector looks a few times
/// whether the window is still maximized and asks again when it isn't.
pub struct MaximizeAgain {
    since: f64,
    next: usize,
}

impl MaximizeAgain {
    /// Starts with the main window, at `now`.
    pub fn new(now: f64) -> MaximizeAgain {
        MaximizeAgain { since: now, next: 0 }
    }

    /// Whether to ask for the maximized window now.
    fn due(&mut self, now: f64, maximized: Option<bool>) -> bool {
        let before = self.next;
        while self.wait(now).is_some_and(|w| w <= 0.0) {
            self.next += 1;
        }
        self.next > before && maximized == Some(false)
    }

    /// The seconds until the next look, `None` after the last one.
    fn wait(&self, now: f64) -> Option<f64> {
        MAXIMIZE_AGAIN.get(self.next).map(|d| self.since + d - now)
    }
}

/// Maximizes the main window again when the desktop undid it (see [`MaximizeAgain`]).
pub fn maximize_again(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(again) = app.maximize_again.as_mut() else { return };
    let (now, maximized) = ctx.input(|i| (i.time, i.viewport().maximized));
    if again.due(now, maximized) {
        ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
    }
    match again.wait(now) {
        Some(w) => ctx.request_repaint_after(std::time::Duration::from_secs_f64(w.max(0.0))),
        None => app.maximize_again = None,
    }
}

/// The splash's color.
const CARD: Color32 = Color32::from_rgb(0x1c, 0x1f, 0x26);

pub struct Splash {
    /// The window is see-through: the corners are round. Otherwise the card fills the window.
    transparent: bool,
    tex: Option<egui::TextureHandle>,
    started: Option<f64>,
    switched: bool,
    status: usize,
}

impl Splash {
    pub fn new(transparent: bool) -> Splash {
        Splash { transparent, tex: None, started: None, switched: false, status: 0 }
    }

    /// The window's color behind the card.
    pub fn clear_color(&self) -> [f32; 4] {
        if self.transparent {
            [0.0; 4]
        } else {
            CARD.to_normalized_gamma_f32()
        }
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
            // an opaque splash is on X11
            if maximized && !sp.transparent {
                app.maximize_again = Some(MaximizeAgain::new(now));
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
    let transparent = sp.transparent;

    let full = ui.max_rect();
    let p = ui.painter();
    // card
    let card = full.shrink(if transparent { 1.0 } else { 0.0 });
    let round = if transparent { 10.0 } else { 0.0 };
    p.rect_filled(card, round, CARD);
    p.rect_stroke(card, round, egui::Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3f, 0x4a)), egui::StrokeKind::Inside);
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

    #[test]
    fn opaque_splash_has_the_cards_color_behind_it() {
        assert_eq!(Splash::new(true).clear_color(), [0.0; 4]);
        let c = Splash::new(false).clear_color();
        assert_eq!(c[3], 1.0);
        assert_eq!(c.map(|v| (v * 255.0).round() as u8), [0x1c, 0x1f, 0x26, 0xff]);
    }

    #[test]
    fn asks_again_only_while_not_maximized() {
        // undone by the desktop, then maximized
        let mut m = MaximizeAgain::new(10.0);
        assert!(!m.due(10.0, Some(false)), "not at once: the first request may still be on its way");
        assert!((m.wait(10.0).unwrap() - 0.2).abs() < 1e-9);
        assert!(m.due(10.25, Some(false)));
        assert!(!m.due(10.3, Some(false)), "once per look");
        assert!(!m.due(10.7, Some(true)), "it is maximized now");
        // undone later (a slow computer)
        assert!(m.due(11.3, Some(false)));
        assert_eq!(m.wait(11.3), None);
        // after the last look the window is the user's
        assert!(!m.due(20.0, Some(false)));
        // one request after a long frame, and none while the state isn't known
        let mut m = MaximizeAgain::new(0.0);
        assert!(m.due(5.0, Some(false)));
        assert_eq!(m.wait(5.0), None);
        assert!(!MaximizeAgain::new(0.0).due(5.0, None));
    }
}
