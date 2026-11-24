//! The Color Eyedropper's "Select from Desktop" on Linux: the color of a pixel anywhere on
//! the screen, as Windows reads it with GDI.
//!
//! - X11: the pixel under the pointer, read from the root window (`GetImage`) when the left
//!   button goes down outside AnyVector's window, the way the Windows version polls the button.
//! - Wayland: a program cannot read other windows, so the desktop's own color picker does it
//!   (xdg-desktop-portal, `Screenshot.PickColor`): it covers the screen, the user clicks, and the
//!   color comes back.
//!
//! The way is chosen as the clipboard chooses (`dialogs::clipboard::linux::choose`): the portal
//! under Wayland when the desktop has one, else X11 (an X11 session, or XWayland, which sees only
//! X11 programs' windows), else none.
//!
//! The pixel conversion and the portal's answer are plain functions, tested on every system.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use vector_core::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    Portal,
    X11,
}

/// How to reach the screen: the portal under Wayland when there is one, else X11, else nothing.
pub fn choose(wayland: bool, portal: bool, x11: bool) -> Option<Way> {
    if wayland && portal {
        Some(Way::Portal)
    } else if x11 {
        Some(Way::X11)
    } else {
        None
    }
}

/// When neither way exists (a Wayland desktop without the portal and without XWayland).
pub const NO_PICKER: &str = "AnyVector cannot see the screen outside its window: the desktop's color picker (xdg-desktop-portal) was not found. Install xdg-desktop-portal and your desktop's portal (for example xdg-desktop-portal-gnome or xdg-desktop-portal-kde), or sample colors inside the drawing window.";

/// Shown once when Wayland has no portal and X11 (XWayland) is used instead.
pub const ONLY_X11: &str = "Without the desktop's color picker (xdg-desktop-portal), only the windows of X11 programs can be sampled.";

// ------------------------------------------------------------------------------ X11 pixels

/// The pixel value at the start of `data`, an image row in a `bits_per_pixel` format; `msb_first`
/// is the server's image byte order.
pub fn pixel_value(data: &[u8], bits_per_pixel: u8, msb_first: bool) -> Option<u32> {
    let n = match bits_per_pixel {
        8 => 1,
        16 => 2,
        24 => 3,
        32 => 4,
        _ => return None,
    };
    let b = data.get(..n)?;
    let mut v = 0u32;
    for i in 0..n {
        let byte = if msb_first { b[i] } else { b[n - 1 - i] };
        v = (v << 8) | byte as u32;
    }
    Some(v)
}

/// A true-color pixel to RGB by the visual's channel masks (8, 10, 5-6-5 bits, ...), each channel
/// scaled to 0..255.
pub fn rgb_from_masks(pixel: u32, masks: [u32; 3]) -> Color {
    let channel = |mask: u32| -> u8 {
        if mask == 0 {
            return 0;
        }
        let shift = mask.trailing_zeros();
        let max = (mask >> shift) as u64;
        let v = ((pixel & mask) >> shift) as u64;
        ((v * 255 + max / 2) / max) as u8
    };
    Color::rgb(channel(masks[0]), channel(masks[1]), channel(masks[2]))
}

// ------------------------------------------------------------------------------ portal answers

/// What the desktop's color picker answered.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Color(Color),
    Cancelled,
    /// No portal, or one without PickColor: try X11.
    NoPortal,
    Failed(String),
}

/// The `Response` signal of a PickColor request: code 0 with the `color` result (three doubles
/// from 0 to 1), 1 when the user cancelled, 2 when it failed.
pub fn answer(code: u32, color: Option<(f64, f64, f64)>) -> Answer {
    let unit = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    match (code, color) {
        (0, Some((r, g, b))) if [r, g, b].iter().all(|v| v.is_finite()) => Answer::Color(Color::rgb(unit(r), unit(g), unit(b))),
        (1, _) => Answer::Cancelled,
        // the portal gives no reason
        _ => Answer::Failed(String::new()),
    }
}

/// The object path of the portal request a `handle_token` makes (subscribed to before the call,
/// so its answer cannot be missed): the caller's unique bus name without `:` and with `_` for `.`.
pub fn request_path(unique_name: &str, token: &str) -> String {
    format!("/org/freedesktop/portal/desktop/request/{}/{token}", unique_name.trim_start_matches(':').replace('.', "_"))
}

/// Whether a D-Bus error name means "no such portal" (rather than a portal that failed).
pub fn means_no_portal(error_name: &str) -> bool {
    ["ServiceUnknown", "UnknownMethod", "UnknownInterface", "UnknownObject", "NameHasNoOwner", "NotSupported"].iter().any(|n| error_name.ends_with(n))
}

#[cfg(target_os = "linux")]
pub use session::{poll, stop, Polled};

#[cfg(target_os = "linux")]
pub mod session {
    use super::portal::Step;
    use super::{Answer, Way};
    use std::sync::mpsc::{Receiver, TryRecvError};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use vector_core::Color;

    /// What one frame's look gave.
    #[derive(Debug, PartialEq)]
    pub enum Polled {
        Waiting,
        Color(Color),
        Cancelled,
        /// A note to show while picking goes on.
        Note(String),
        /// Picking is not possible: why.
        Message(String),
    }

    /// How long the portal may take to open its color picker, in all: then it counts as missing.
    const OPEN_LIMIT: Duration = Duration::from_secs(8);

    /// The desktop's color picker, asked from a thread of its own: the window only ever looks
    /// whether something has arrived. The thread is not waited for when AnyVector ends (the end of
    /// the program closes the connection, and the portal then closes its picker).
    pub struct PortalPick {
        rx: Receiver<Step>,
        since: Instant,
        /// The connection of the picker on the screen.
        open: Option<zbus::blocking::Connection>,
    }

    impl PortalPick {
        pub fn new(rx: Receiver<Step>) -> PortalPick {
            PortalPick { rx, since: Instant::now(), open: None }
        }

        fn start() -> PortalPick {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || super::portal::run(super::portal::session_bus(), tx));
            PortalPick::new(rx)
        }

        /// The answer, if it is there by `now`. A portal that has not opened its picker in time
        /// counts as none; an open picker waits for the user as long as it takes.
        pub fn poll(&mut self, now: Instant) -> Option<Answer> {
            loop {
                match self.rx.try_recv() {
                    Ok(Step::Open(c)) => self.open = Some(c),
                    Ok(Step::Done(a)) => return Some(a),
                    Err(TryRecvError::Disconnected) => return Some(Answer::NoPortal),
                    Err(TryRecvError::Empty) => return (self.open.is_none() && now.duration_since(self.since) > OPEN_LIMIT).then_some(Answer::NoPortal),
                }
            }
        }
    }

    impl Drop for PortalPick {
        /// Given up: closing the connection makes the portal close its picker and ends the thread.
        fn drop(&mut self) {
            let unseen = self.rx.try_iter().find_map(|s| if let Step::Open(c) = s { Some(c) } else { None });
            if let Some(c) = self.open.take().or(unseen) {
                std::thread::spawn(move || c.close());
            }
        }
    }

    enum Session {
        X11 { screen: super::x11::Screen, armed: bool },
        Portal(PortalPick),
    }

    static SESSION: Mutex<Option<Session>> = Mutex::new(None);

    fn env_set(name: &str) -> bool {
        std::env::var_os(name).is_some_and(|v| !v.is_empty())
    }

    /// Begin; `portal` is off once the portal has not answered.
    fn start(portal: bool) -> Result<(Session, Option<String>), String> {
        let wayland = env_set("WAYLAND_DISPLAY") || env_set("WAYLAND_SOCKET");
        match super::choose(wayland, portal, env_set("DISPLAY")) {
            Some(Way::Portal) => Ok((Session::Portal(PortalPick::start()), None)),
            Some(Way::X11) => {
                let screen = super::x11::Screen::open().ok_or_else(|| super::NO_PICKER.to_string())?;
                Ok((Session::X11 { screen, armed: false }, wayland.then(|| super::ONLY_X11.to_string())))
            }
            None => Err(super::NO_PICKER.into()),
        }
    }

    /// One frame of picking: `now` samples the pixel under the pointer at once (the eyedropper
    /// let go outside the window); `inside` says the pointer is over AnyVector's window, whose
    /// clicks the drawing's own eyedropper takes.
    pub fn poll(now: bool, inside: bool) -> Polled {
        let mut s = SESSION.lock().unwrap_or_else(|e| e.into_inner());
        let mut note = None;
        if s.is_none() {
            match start(true) {
                Ok((session, n)) => {
                    *s = Some(session);
                    note = n;
                }
                Err(m) => return Polled::Message(m),
            }
        }
        let got = match s.as_mut() {
            Some(Session::X11 { screen, armed }) => match screen.pointer() {
                None => Polled::Cancelled,
                Some((x, y, _)) if now => screen.pixel(x, y).map_or(Polled::Cancelled, Polled::Color),
                Some((_, _, false)) => {
                    *armed = true;
                    Polled::Waiting
                }
                // a click: taken when the button goes down after having been up
                Some((x, y, true)) => {
                    if std::mem::replace(armed, false) && !inside {
                        screen.pixel(x, y).map_or(Polled::Cancelled, Polled::Color)
                    } else {
                        Polled::Waiting
                    }
                }
            },
            Some(Session::Portal(pick)) => match pick.poll(Instant::now()) {
                None => Polled::Waiting,
                Some(Answer::Color(c)) => Polled::Color(c),
                Some(Answer::Cancelled) => Polled::Cancelled,
                Some(Answer::Failed(e)) if e.is_empty() => Polled::Message("The desktop's color picker failed.".into()),
                Some(Answer::Failed(e)) => Polled::Message(format!("The desktop's color picker failed:\n{e}")),
                // no portal after all: X11, when there is one
                Some(Answer::NoPortal) => match start(false) {
                    Ok((session, n)) => {
                        *s = Some(session);
                        return n.map_or(Polled::Waiting, Polled::Note);
                    }
                    Err(m) => Polled::Message(m),
                },
            },
            None => Polled::Waiting,
        };
        if got != Polled::Waiting {
            *s = None;
            return got;
        }
        note.map_or(Polled::Waiting, Polled::Note)
    }

    /// Picking ended (the button was turned off): drop the X11 connection, or close the portal's
    /// picker.
    pub fn stop() {
        let mut s = SESSION.lock().unwrap_or_else(|e| e.into_inner());
        if s.is_some() {
            *s = None;
        }
    }
}

#[cfg(target_os = "linux")]
pub mod x11 {
    use vector_core::Color;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, ImageOrder, KeyButMask, VisualClass};
    use x11rb::rust_connection::RustConnection;

    /// A connection to the X11 display in `DISPLAY`.
    pub struct Screen {
        conn: RustConnection,
        screen: usize,
    }

    impl Screen {
        pub fn open() -> Option<Screen> {
            let (conn, screen) = RustConnection::connect(None).ok()?;
            Some(Screen { conn, screen })
        }

        /// The pointer on the screen and whether the left button is down.
        pub fn pointer(&self) -> Option<(i16, i16, bool)> {
            let root = self.conn.setup().roots.get(self.screen)?.root;
            let r = self.conn.query_pointer(root).ok()?.reply().ok()?;
            Some((r.root_x, r.root_y, r.mask.contains(KeyButMask::BUTTON1)))
        }

        /// The color of the screen pixel at (x, y).
        pub fn pixel(&self, x: i16, y: i16) -> Option<Color> {
            let setup = self.conn.setup();
            let scr = setup.roots.get(self.screen)?;
            let img = self.conn.get_image(ImageFormat::Z_PIXMAP, scr.root, x, y, 1, 1, !0).ok()?.reply().ok()?;
            let bpp = setup.pixmap_formats.iter().find(|f| f.depth == img.depth)?.bits_per_pixel;
            let px = super::pixel_value(&img.data, bpp, setup.image_byte_order == ImageOrder::MSB_FIRST)?;
            let visual = scr.allowed_depths.iter().flat_map(|d| d.visuals.iter()).find(|v| v.visual_id == img.visual);
            match visual {
                Some(v) if v.class == VisualClass::TRUE_COLOR || v.class == VisualClass::DIRECT_COLOR => Some(super::rgb_from_masks(px, [v.red_mask, v.green_mask, v.blue_mask])),
                // indexed colors: the colormap says what the pixel is
                _ => {
                    let c = self.conn.query_colors(scr.default_colormap, &[px]).ok()?.reply().ok()?;
                    let c = c.colors.first()?;
                    Some(Color::rgb((c.red >> 8) as u8, (c.green >> 8) as u8, (c.blue >> 8) as u8))
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub mod portal {
    use super::Answer;
    use std::collections::HashMap;
    use std::sync::mpsc::Sender;
    use std::time::Duration;
    use zbus::blocking::{connection::Builder, Connection};
    use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

    const DEST: &str = "org.freedesktop.portal.Desktop";
    const PATH: &str = "/org/freedesktop/portal/desktop";
    const IFACE: &str = "org.freedesktop.portal.Screenshot";

    /// How long the portal may take for each call it answers at once (all but the user's click,
    /// which comes as a signal).
    pub const CALL_LIMIT: Duration = Duration::from_secs(5);

    /// What the thread asking the portal reports.
    pub enum Step {
        /// The picker is open on this connection; the rest is up to the user.
        Open(Connection),
        Done(Answer),
    }

    /// The session bus, with a limit on every call.
    pub fn session_bus() -> zbus::Result<Connection> {
        limited(Builder::session()?, CALL_LIMIT)
    }

    pub fn limited(b: Builder<'_>, limit: Duration) -> zbus::Result<Connection> {
        b.method_timeout(limit).build()
    }

    /// The thread: ask over `bus`, reporting to `tx` until the answer or until no one listens.
    pub fn run(bus: zbus::Result<Connection>, tx: Sender<Step>) {
        let answer = match bus {
            Ok(c) => pick(&c, || tx.send(Step::Open(c.clone())).is_ok()),
            Err(_) => Answer::NoPortal,
        };
        let _ = tx.send(Step::Done(answer));
    }

    fn error_answer(e: zbus::Error) -> Answer {
        match &e {
            // no answer in time
            zbus::Error::InputOutput(io) if io.kind() == std::io::ErrorKind::TimedOut => Answer::NoPortal,
            zbus::Error::MethodError(name, _, _) if super::means_no_portal(name.as_str()) => Answer::NoPortal,
            zbus::Error::MethodError(_, Some(text), _) => Answer::Failed(text.clone()),
            _ => Answer::Failed(e.to_string()),
        }
    }

    /// The version of the portal's Screenshot interface (PickColor came with version 2).
    fn version(conn: &Connection) -> Result<u32, zbus::Error> {
        let r = conn.call_method(Some(DEST), PATH, Some("org.freedesktop.DBus.Properties"), "Get", &(IFACE, "version"))?;
        let v: OwnedValue = r.body().deserialize()?;
        u32::try_from(v).map_err(zbus::Error::from)
    }

    /// The `color` of a Response's results.
    pub fn color_of(results: &HashMap<String, OwnedValue>) -> Option<(f64, f64, f64)> {
        let v: Value = results.get("color")?.try_clone().ok()?.into();
        <(f64, f64, f64)>::try_from(v).ok()
    }

    fn signals(conn: &Connection, path: &str) -> Result<zbus::blocking::MessageIterator, zbus::Error> {
        let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).interface("org.freedesktop.portal.Request")?.member("Response")?.path(path)?.build();
        zbus::blocking::MessageIterator::for_match_rule(rule, conn, Some(4))
    }

    /// Ask the desktop's color picker for a color. `opened` is called once the picker is on the
    /// screen and says whether to go on; then this waits until the user clicks or cancels, or the
    /// connection is closed.
    pub fn pick(conn: &Connection, opened: impl FnOnce() -> bool) -> Answer {
        match version(conn) {
            Ok(v) if v >= 2 => {}
            Ok(_) => return Answer::NoPortal,
            Err(_) => return Answer::NoPortal,
        }
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let token = format!("anyvector{}_{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        let Some(unique) = conn.unique_name().map(|n| n.to_string()) else { return Answer::NoPortal };
        let path = super::request_path(&unique, &token);
        let mut it = match signals(conn, &path) {
            Ok(it) => it,
            Err(e) => return error_answer(e),
        };
        let mut opts: HashMap<&str, Value> = HashMap::new();
        opts.insert("handle_token", Value::from(token.as_str()));
        let handle = match conn.call_method(Some(DEST), PATH, Some(IFACE), "PickColor", &("", opts)).and_then(|r| r.body().deserialize::<OwnedObjectPath>()) {
            Ok(h) => h,
            Err(e) => return error_answer(e),
        };
        // portals older than 0.9 answer on a path of their own
        if handle.as_str() != path {
            it = match signals(conn, handle.as_str()) {
                Ok(it) => it,
                Err(e) => return error_answer(e),
            };
        }
        if !opened() {
            return Answer::Cancelled;
        }
        if let Some(Ok(m)) = it.next() {
            return match m.body().deserialize::<(u32, HashMap<String, OwnedValue>)>() {
                Ok((code, results)) => super::answer(code, color_of(&results)),
                Err(e) => Answer::Failed(e.to_string()),
            };
        }
        // the connection was closed
        Answer::Failed(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_portal_comes_first_under_wayland_as_the_clipboard_chooses() {
        assert_eq!(choose(true, true, true), Some(Way::Portal));
        assert_eq!(choose(true, false, true), Some(Way::X11), "XWayland when the desktop has no portal");
        assert_eq!(choose(true, false, false), None);
        assert_eq!(choose(false, true, true), Some(Way::X11), "an X11 session reads the screen itself");
        assert_eq!(choose(false, false, false), None);
    }

    #[test]
    fn pixels_in_the_x11_formats() {
        // 32 bits, little-endian BGRX (the usual 24-bit depth)
        let rgb888 = [0xff_0000, 0x00_ff00, 0x00_00ff];
        let px = pixel_value(&[0x20, 0x40, 0xff, 0x00], 32, false).unwrap();
        assert_eq!(px, 0x00ff_4020);
        assert_eq!(rgb_from_masks(px, rgb888), Color::rgb(255, 64, 32));
        // the same pixel from a big-endian server
        let px = pixel_value(&[0x00, 0xff, 0x40, 0x20], 32, true).unwrap();
        assert_eq!(rgb_from_masks(px, rgb888), Color::rgb(255, 64, 32));
        // packed 24 bits
        let px = pixel_value(&[0x10, 0x80, 0xc0], 24, false).unwrap();
        assert_eq!(rgb_from_masks(px, rgb888), Color::rgb(0xc0, 0x80, 0x10));
        // 16 bits 5-6-5: full red, half green, no blue
        let rgb565 = [0xf800, 0x07e0, 0x001f];
        let px = pixel_value(&[0x00, 0xf8 | 0x04], 16, false).unwrap();
        assert_eq!(px, 0xfc00);
        let c = rgb_from_masks(px, rgb565);
        assert_eq!((c.r, c.g, c.b), (255, 130, 0));
        assert_eq!(rgb_from_masks(0xffff, rgb565), Color::WHITE);
        // 30-bit depth (10 bits a channel)
        let rgb101010 = [0x3ff0_0000, 0x000f_fc00, 0x0000_03ff];
        assert_eq!(rgb_from_masks(0x3ff0_0000 | 0x200, rgb101010), Color::rgb(255, 0, 128));
        // 8 bits 3-3-2
        assert_eq!(rgb_from_masks(0b111_000_11, [0xe0, 0x1c, 0x03]), Color::rgb(255, 0, 255));
        // a visual without a channel, and formats that are not read
        assert_eq!(rgb_from_masks(0xffffff, [0xff0000, 0, 0xff]), Color::rgb(255, 0, 255));
        assert_eq!(pixel_value(&[0xff], 32, false), None, "too few bytes");
        assert_eq!(pixel_value(&[0xff; 4], 4, false), None);
        assert_eq!(pixel_value(&[0x7f], 8, true), Some(0x7f));
    }

    #[test]
    fn the_portal_answer() {
        assert_eq!(answer(0, Some((1.0, 0.5, 0.0))), Answer::Color(Color::rgb(255, 128, 0)));
        // out of range or rounding: still a color
        assert_eq!(answer(0, Some((1.2, -0.1, 0.2))), Answer::Color(Color::rgb(255, 0, 51)));
        assert_eq!(answer(1, None), Answer::Cancelled);
        assert!(matches!(answer(2, None), Answer::Failed(_)));
        assert!(matches!(answer(0, None), Answer::Failed(_)), "success without a color");
        assert!(matches!(answer(0, Some((f64::NAN, 0.0, 0.0))), Answer::Failed(_)));
        assert_eq!(request_path(":1.42", "anyvector7_0"), "/org/freedesktop/portal/desktop/request/1_42/anyvector7_0");
        assert!(means_no_portal("org.freedesktop.DBus.Error.ServiceUnknown"));
        assert!(means_no_portal("org.freedesktop.DBus.Error.UnknownMethod"));
        assert!(!means_no_portal("org.freedesktop.portal.Error.Failed"));
    }

    /// The window's side of a portal that never answers: it only looks, and gives up.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_portal_that_never_opens_its_picker_is_given_up() {
        use std::time::{Duration, Instant};
        let (tx, rx) = std::sync::mpsc::channel();
        let mut p = session::PortalPick::new(rx);
        let t = Instant::now();
        assert_eq!(p.poll(t), None, "waits");
        assert_eq!(p.poll(t + Duration::from_secs(60)), Some(Answer::NoPortal));
        assert!(t.elapsed() < Duration::from_secs(1), "looking never waits");
        tx.send(portal::Step::Done(Answer::Cancelled)).unwrap();
        assert_eq!(p.poll(t), Some(Answer::Cancelled));
        // the thread ended without a word
        drop(tx);
        assert_eq!(p.poll(t), Some(Answer::NoPortal));
    }

    /// The results dictionary of a real Response signal, as the portal sends it.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_color_in_the_portal_results() {
        use std::collections::HashMap;
        use zbus::zvariant::{OwnedValue, Value};
        let mut results: HashMap<String, OwnedValue> = HashMap::new();
        assert_eq!(portal::color_of(&results), None);
        results.insert("color".into(), OwnedValue::try_from(Value::from((0.25f64, 0.5f64, 1.0f64))).unwrap());
        assert_eq!(portal::color_of(&results), Some((0.25, 0.5, 1.0)));
        results.insert("color".into(), OwnedValue::from(7u32));
        assert_eq!(portal::color_of(&results), None, "not three doubles");
    }
}
