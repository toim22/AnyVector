//! The system clipboard on Linux, by MIME type.
//!
//! - Wayland: the data-control protocol (KDE Plasma, Sway, Hyprland and the other wlroots
//!   compositors) through `wl-clipboard-rs`. It needs no keyboard focus or input serial.
//! - X11: the CLIPBOARD selection through `x11rb`, served by a thread of our own (large data in
//!   INCR chunks), handed to the desktop's clipboard manager when the program ends. This is also
//!   the way under a Wayland compositor without data-control (GNOME, Weston): XWayland passes the
//!   selection on to the Wayland programs.
//!
//! Both crates were already in the program through eframe (arboard and winit use them); arboard
//! itself only offers text, HTML, PNG and file lists, not formats of our own.

use std::io::Read;
use std::os::fd::AsFd;
use std::time::{Duration, Instant};

/// How long another program may take to say what it offers (asked about once a second while the
/// Edit menu or the standard toolbar shows Paste).
const TYPES_TIMEOUT: Duration = Duration::from_millis(500);
/// How long another program may take to hand over (or between two chunks of) its data: making a
/// large bitmap can take a while.
const DATA_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Wayland,
    X11,
}

/// Which clipboard to use: Wayland's data-control protocol when the compositor has it, else X11
/// (an X11 session, or XWayland under a Wayland compositor without data-control). None: neither
/// (copy and paste then work inside the program only).
pub fn choose(wayland: bool, data_control: impl FnOnce() -> bool, x11: bool) -> Option<Backend> {
    if wayland && data_control() {
        Some(Backend::Wayland)
    } else if x11 {
        Some(Backend::X11)
    } else {
        None
    }
}

static BACKEND: std::sync::Mutex<Option<Option<Backend>>> = std::sync::Mutex::new(None);

/// The clipboard in use (decided once).
pub fn backend() -> Option<Backend> {
    let mut b = BACKEND.lock().unwrap_or_else(|e| e.into_inner());
    *b.get_or_insert_with(|| choose(crate::platform::on_wayland(), wayland::available, crate::platform::env_set("DISPLAY")))
}

/// Tests: decide again (after the displays were changed) and drop the X11 connection.
#[cfg(test)]
pub fn reset() {
    *BACKEND.lock().unwrap_or_else(|e| e.into_inner()) = None;
    x11::reset();
}

/// Tests: offer formats on the X11 clipboard from a connection of their own, as another program
/// does (kept while the result lives).
#[cfg(test)]
pub fn other_x11_program(offers: &[(&str, &[u8])]) -> Option<std::sync::Arc<x11::Owner>> {
    let o = x11::Owner::new()?;
    o.set(offers).then_some(o)
}

/// Tests: an X11 clipboard manager waiting to be handed the clipboard (see `x11::fake_manager`).
#[cfg(test)]
pub fn fake_clipboard_manager() -> Option<std::thread::JoinHandle<Vec<(String, Vec<u8>)>>> {
    x11::fake_manager()
}

/// Offer the formats on the clipboard, all together. False when there is no clipboard.
pub fn set(offers: &[(&str, &[u8])]) -> bool {
    if offers.is_empty() {
        return false;
    }
    match backend() {
        Some(Backend::Wayland) => wayland::set(offers),
        Some(Backend::X11) => x11::set(offers),
        None => false,
    }
}

/// The formats the clipboard offers (MIME types; X11 also lists names such as UTF8_STRING).
pub fn types() -> Vec<String> {
    match backend() {
        Some(Backend::Wayland) => wayland::types(),
        Some(Backend::X11) => x11::types(),
        None => vec![],
    }
}

/// The clipboard's data in one format.
pub fn get(mime: &str) -> Option<Vec<u8>> {
    match backend() {
        Some(Backend::Wayland) => wayland::get(mime),
        Some(Backend::X11) => x11::get(mime),
        None => None,
    }
}

/// The data of the first format of `wanted` (best first) that the clipboard has, with the format
/// as it was offered.
pub fn get_first(wanted: &[&str]) -> Option<(String, Vec<u8>)> {
    get_first_in(&types(), wanted)
}

/// [`get_first`] when the formats on offer are known already.
pub fn get_first_in(types: &[String], wanted: &[&str]) -> Option<(String, Vec<u8>)> {
    for w in wanted {
        if let Some(m) = super::mime::pick(types, &[*w]) {
            if let Some(v) = get(m).filter(|v| !v.is_empty()) {
                return Some((m.to_string(), v));
            }
        }
    }
    None
}

/// When the program ends: let the clipboard manager keep what Copy offered (X11; on Wayland the
/// managers copy each new clipboard themselves).
pub fn persist() {
    if BACKEND.lock().unwrap_or_else(|e| e.into_inner()).flatten() == Some(Backend::X11) {
        x11::persist();
    }
}

fn timespec(d: Duration) -> rustix::event::Timespec {
    rustix::event::Timespec { tv_sec: d.as_secs() as _, tv_nsec: d.subsec_nanos() as _ }
}

/// Wait until `fd` can be read or the deadline passes (false).
fn wait_readable(fd: impl AsFd, deadline: Instant) -> bool {
    loop {
        let Some(left) = deadline.checked_duration_since(Instant::now()).filter(|d| !d.is_zero()) else { return false };
        let mut fds = [rustix::event::PollFd::new(&fd, rustix::event::PollFlags::IN)];
        match rustix::event::poll(&mut fds, Some(&timespec(left))) {
            Ok(0) => return false,
            Ok(_) => return true,
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return false,
        }
    }
}

/// Read a pipe to its end, giving up when the writer stalls for `timeout`.
fn read_pipe(mut r: impl Read + AsFd, timeout: Duration) -> Option<Vec<u8>> {
    let mut out = vec![];
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if !wait_readable(r.as_fd(), Instant::now() + timeout) {
            return None;
        }
        match r.read(&mut buf) {
            Ok(0) => return Some(out),
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
}

// ------------------------------------------------------------------------------------------
// Wayland: data-control

mod wayland {
    use wl_clipboard_rs::{copy, paste};

    fn mime_types() -> Result<Vec<String>, paste::Error> {
        paste::get_mime_types_ordered(paste::ClipboardType::Regular, paste::Seat::Unspecified)
    }

    /// Whether the compositor can be reached and has the data-control protocol (and a seat).
    pub fn available() -> bool {
        matches!(mime_types(), Ok(_) | Err(paste::Error::ClipboardEmpty | paste::Error::NoMimeType))
    }

    pub fn set(offers: &[(&str, &[u8])]) -> bool {
        let sources = offers
            .iter()
            .map(|(m, d)| copy::MimeSource { source: copy::Source::Bytes(Box::from(*d)), mime_type: copy::MimeType::Specific(m.to_string()) })
            .collect();
        let mut o = copy::Options::new();
        // the SVG counts as text for wl-clipboard-rs: no text/plain copy of it (as on Windows,
        // a program that takes text only would paste the SVG source)
        o.omit_additional_text_mime_types(true);
        // a thread of wl-clipboard-rs serves the data until another program copies
        o.copy_multi(sources).is_ok()
    }

    pub fn types() -> Vec<String> {
        mime_types().unwrap_or_default()
    }

    pub fn get(mime: &str) -> Option<Vec<u8>> {
        let (pipe, _) = paste::get_contents(paste::ClipboardType::Regular, paste::Seat::Unspecified, paste::MimeType::Specific(mime)).ok()?;
        super::read_pipe(pipe, super::DATA_TIMEOUT)
    }
}

// ------------------------------------------------------------------------------------------
// X11: the CLIPBOARD selection

pub(super) mod x11 {
    use super::{DATA_TIMEOUT, TYPES_TIMEOUT};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::{Duration, Instant};
    use x11rb::connection::{Connection, RequestConnection};
    use x11rb::protocol::xproto::{
        Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, Property, SelectionNotifyEvent, SelectionRequestEvent, Window, WindowClass,
        SELECTION_NOTIFY_EVENT,
    };
    use x11rb::protocol::Event;
    use x11rb::rust_connection::RustConnection;
    use x11rb::wrapper::ConnectionExt as _;
    use x11rb::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, CURRENT_TIME, NONE};

    x11rb::atom_manager! {
        pub Atoms: AtomsCookie {
            CLIPBOARD,
            TARGETS,
            INCR,
            CLIPBOARD_MANAGER,
            SAVE_TARGETS,
            // the property conversions to our windows land in
            SELECTION_PROPERTY: b"ANYVECTOR_SELECTION",
            SAVE_PROPERTY: b"ANYVECTOR_SAVE_TARGETS",
        }
    }

    /// Data larger than this goes in INCR chunks of this size (ICCCM 2.7.2).
    const CHUNK: usize = 256 * 1024;
    /// INCR transfers kept going at once (a requestor that vanished leaves one behind).
    const MAX_TRANSFERS: usize = 16;

    /// One format we offer.
    #[derive(Clone)]
    struct Item {
        mime: String,
        atom: Atom,
        data: Arc<[u8]>,
    }

    #[derive(Default)]
    struct Shared {
        /// What we offer; empty when another program owns the clipboard.
        items: Mutex<Vec<Item>>,
        /// The clipboard manager answered our SAVE_TARGETS request.
        saved: Mutex<bool>,
        saved_cv: Condvar,
    }

    /// Our end of the clipboard: a hidden window that owns the selection, its connection and the
    /// thread that answers the other programs' requests.
    pub struct Owner {
        conn: Arc<RustConnection>,
        win: Window,
        atoms: Atoms,
        shared: Arc<Shared>,
        thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    }

    static OWNER: Mutex<Option<Arc<Owner>>> = Mutex::new(None);

    /// A hidden 1×1 window on `conn` that gets the property events of conversions.
    fn hidden_window(conn: &RustConnection, screen: usize, events: EventMask) -> Option<Window> {
        let root = conn.setup().roots.get(screen)?.root;
        let win = conn.generate_id().ok()?;
        conn.create_window(COPY_DEPTH_FROM_PARENT, win, root, 0, 0, 1, 1, 0, WindowClass::INPUT_OUTPUT, COPY_FROM_PARENT, &CreateWindowAux::new().event_mask(events)).ok()?;
        Some(win)
    }

    impl Owner {
        /// Connect to the display in `DISPLAY` and start the thread that serves our data.
        pub fn new() -> Option<Arc<Owner>> {
            let (conn, screen) = RustConnection::connect(None).ok()?;
            let win = hidden_window(&conn, screen, EventMask::PROPERTY_CHANGE | EventMask::STRUCTURE_NOTIFY)?;
            let atoms = Atoms::new(&conn).ok()?.reply().ok()?;
            conn.flush().ok()?;
            let owner = Arc::new(Owner { conn: Arc::new(conn), win, atoms, shared: Arc::default(), thread: Mutex::new(None) });
            let (conn, shared) = (owner.conn.clone(), owner.shared.clone());
            let handle = std::thread::Builder::new().name("x11-clipboard".into()).spawn(move || serve(&conn, win, atoms, &shared)).ok()?;
            *owner.thread.lock().unwrap() = Some(handle);
            Some(owner)
        }

        fn owns_clipboard(&self) -> bool {
            self.conn.get_selection_owner(self.atoms.CLIPBOARD).ok().and_then(|c| c.reply().ok()).is_some_and(|r| r.owner == self.win)
        }

        /// What we offer, when we own the clipboard.
        fn own_items(&self) -> Option<Vec<Item>> {
            if !self.owns_clipboard() {
                return None;
            }
            Some(self.shared.items.lock().unwrap().clone())
        }

        /// Take the clipboard with these formats. False when another program kept it.
        pub fn set(&self, offers: &[(&str, &[u8])]) -> bool {
            let cookies: Vec<_> = offers.iter().map(|(m, _)| self.conn.intern_atom(false, m.as_bytes())).collect();
            let mut items = vec![];
            for ((mime, data), cookie) in offers.iter().zip(cookies) {
                let Some(atom) = cookie.ok().and_then(|c| c.reply().ok()).map(|r| r.atom) else { return false };
                items.push(Item { mime: mime.to_string(), atom, data: Arc::from(*data) });
            }
            *self.shared.items.lock().unwrap() = items;
            if self.conn.set_selection_owner(self.win, self.atoms.CLIPBOARD, CURRENT_TIME).is_err() || self.conn.flush().is_err() {
                return false;
            }
            self.owns_clipboard()
        }

        /// Ask the clipboard manager (when one runs) to copy our data, and wait a little for it.
        fn persist(&self) {
            let Some(items) = self.own_items().filter(|i| !i.is_empty()) else { return };
            let manager = self.conn.get_selection_owner(self.atoms.CLIPBOARD_MANAGER).ok().and_then(|c| c.reply().ok()).map(|r| r.owner);
            if manager.is_none_or(|m| m == NONE) {
                return;
            }
            let targets: Vec<Atom> = items.iter().map(|i| i.atom).collect();
            *self.shared.saved.lock().unwrap() = false;
            let sent = self.conn.change_property32(PropMode::REPLACE, self.win, self.atoms.SAVE_PROPERTY, AtomEnum::ATOM, &targets).is_ok()
                && self.conn.convert_selection(self.win, self.atoms.CLIPBOARD_MANAGER, self.atoms.SAVE_TARGETS, self.atoms.SAVE_PROPERTY, CURRENT_TIME).is_ok()
                && self.conn.flush().is_ok();
            if sent {
                // the serving thread answers the manager's requests meanwhile
                let done = self.shared.saved.lock().unwrap();
                let _ = self.shared.saved_cv.wait_timeout_while(done, Duration::from_secs(2), |d| !*d);
            }
        }
    }

    fn owner() -> Option<Arc<Owner>> {
        let mut o = OWNER.lock().unwrap_or_else(|e| e.into_inner());
        if o.is_none() {
            *o = Owner::new();
        }
        o.clone()
    }

    pub fn set(offers: &[(&str, &[u8])]) -> bool {
        owner().is_some_and(|o| o.set(offers))
    }

    pub fn types() -> Vec<String> {
        let Some(o) = owner() else { return vec![] };
        if let Some(items) = o.own_items() {
            return items.into_iter().map(|i| i.mime).collect();
        }
        let Some(r) = Reader::new() else { return vec![] };
        let Some(reply) = r.convert(&o.atoms, o.atoms.TARGETS, TYPES_TIMEOUT) else { return vec![] };
        let atoms: Vec<Atom> = reply.chunks_exact(4).map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]])).filter(|a| *a != NONE).collect();
        let cookies: Vec<_> = atoms.iter().map(|a| r.conn.get_atom_name(*a)).collect();
        cookies.into_iter().filter_map(|c| c.ok()?.reply().ok()).map(|n| String::from_utf8_lossy(&n.name).into_owned()).collect()
    }

    pub fn get(mime: &str) -> Option<Vec<u8>> {
        let o = owner()?;
        if let Some(items) = o.own_items() {
            return items.into_iter().find(|i| i.mime == mime).map(|i| i.data.to_vec());
        }
        let atom = o.conn.intern_atom(false, mime.as_bytes()).ok()?.reply().ok()?.atom;
        Reader::new()?.convert(&o.atoms, atom, DATA_TIMEOUT)
    }

    pub fn persist() {
        let o = OWNER.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(o) = o {
            o.persist();
        }
    }

    /// Tests: drop the connection (it is made again for the display now in `DISPLAY`).
    #[cfg(test)]
    pub fn reset() {
        let o = OWNER.lock().unwrap_or_else(|e| e.into_inner()).take();
        drop(o);
    }

    /// Tests: a clipboard manager as the desktops run one (ICCCM CLIPBOARD_MANAGER). Asked to
    /// SAVE_TARGETS (within 10 s), it reads each format the program lists, then says it is
    /// done; the thread gives back what it read, by format.
    #[cfg(test)]
    pub fn fake_manager() -> Option<std::thread::JoinHandle<Vec<(String, Vec<u8>)>>> {
        let (conn, screen) = RustConnection::connect(None).ok()?;
        let win = hidden_window(&conn, screen, EventMask::NO_EVENT)?;
        let atoms = Atoms::new(&conn).ok()?.reply().ok()?;
        conn.set_selection_owner(win, atoms.CLIPBOARD_MANAGER, CURRENT_TIME).ok()?;
        let owner = conn.get_selection_owner(atoms.CLIPBOARD_MANAGER).ok()?.reply().ok()?.owner;
        if owner != win {
            return None;
        }
        Some(std::thread::spawn(move || {
            let mut saved = vec![];
            let deadline = Instant::now() + Duration::from_secs(10);
            let request = loop {
                match conn.poll_for_event() {
                    Ok(Some(Event::SelectionRequest(e))) if e.selection == atoms.CLIPBOARD_MANAGER && e.target == atoms.SAVE_TARGETS => break e,
                    Ok(Some(_)) => {}
                    Ok(None) if Instant::now() < deadline => {
                        let _ = super::wait_readable(conn.stream(), Instant::now() + Duration::from_millis(100));
                    }
                    _ => return saved,
                }
            };
            let targets: Vec<Atom> = conn
                .get_property(false, request.requestor, request.property, AtomEnum::ATOM, 0, 1024)
                .ok()
                .and_then(|c| c.reply().ok())
                .and_then(|r| r.value32().map(|v| v.collect()))
                .unwrap_or_default();
            if let Some(r) = Reader::new() {
                for t in targets {
                    let name = conn.get_atom_name(t).ok().and_then(|c| c.reply().ok()).map(|n| String::from_utf8_lossy(&n.name).into_owned()).unwrap_or_default();
                    if let Some(v) = r.convert(&atoms, t, DATA_TIMEOUT) {
                        saved.push((name, v));
                    }
                }
            }
            let done = SelectionNotifyEvent {
                response_type: SELECTION_NOTIFY_EVENT,
                sequence: 0,
                time: request.time,
                requestor: request.requestor,
                selection: request.selection,
                target: request.target,
                property: request.property,
            };
            let _ = conn.send_event(false, request.requestor, EventMask::NO_EVENT, done);
            let _ = conn.flush();
            saved
        }))
    }

    impl Drop for Owner {
        /// The serving thread ends with the window.
        fn drop(&mut self) {
            let _ = self.conn.destroy_window(self.win);
            let _ = self.conn.flush();
            if let Some(t) = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = t.join();
            }
        }
    }

    // ---- serving our data

    /// An INCR transfer under way: the next chunk goes when the requestor deletes the property.
    struct Transfer {
        requestor: Window,
        property: Atom,
        target: Atom,
        data: Arc<[u8]>,
        sent: usize,
    }

    fn serve(conn: &RustConnection, win: Window, atoms: Atoms, shared: &Shared) {
        let mut transfers: Vec<Transfer> = vec![];
        let chunk = CHUNK.min(conn.maximum_request_bytes().saturating_sub(1024)).max(4096);
        while let Ok(ev) = conn.wait_for_event() {
            match ev {
                Event::SelectionRequest(e) => {
                    let _ = answer(conn, &atoms, shared, &e, &mut transfers, chunk);
                }
                Event::SelectionClear(e) if e.selection == atoms.CLIPBOARD => {
                    // another program copied (unless we took the clipboard back meanwhile)
                    let owner = conn.get_selection_owner(atoms.CLIPBOARD).ok().and_then(|c| c.reply().ok()).map(|r| r.owner);
                    if owner != Some(win) {
                        shared.items.lock().unwrap().clear();
                    }
                }
                Event::PropertyNotify(e) if e.state == Property::DELETE => {
                    if let Some(i) = transfers.iter().position(|t| t.requestor == e.window && t.property == e.atom) {
                        if !send_chunk(conn, &mut transfers[i], chunk) {
                            let t = transfers.remove(i);
                            stop_watching(conn, &transfers, t.requestor);
                        }
                    }
                }
                Event::SelectionNotify(e) if e.selection == atoms.CLIPBOARD_MANAGER => {
                    *shared.saved.lock().unwrap() = true;
                    shared.saved_cv.notify_all();
                }
                Event::DestroyNotify(e) if e.window == win => return,
                Event::DestroyNotify(e) => transfers.retain(|t| t.requestor != e.window),
                _ => {}
            }
        }
    }

    /// Answer a request for our data (ICCCM 2.2): the list of formats, or the data of one, in
    /// INCR chunks when it is large.
    fn answer(conn: &RustConnection, atoms: &Atoms, shared: &Shared, e: &SelectionRequestEvent, transfers: &mut Vec<Transfer>, chunk: usize) -> Result<(), Box<dyn std::error::Error>> {
        // obsolete requestors leave the property out: the target names it then
        let property = if e.property == NONE { e.target } else { e.property };
        let mut ok = false;
        if e.selection == atoms.CLIPBOARD {
            let items = shared.items.lock().unwrap().clone();
            if items.is_empty() {
                // nothing to give (another program owns the clipboard now)
            } else if e.target == atoms.TARGETS {
                let mut list: Vec<Atom> = items.iter().map(|i| i.atom).collect();
                list.extend([atoms.TARGETS, atoms.SAVE_TARGETS]);
                conn.change_property32(PropMode::REPLACE, e.requestor, property, AtomEnum::ATOM, &list)?;
                ok = true;
            } else if let Some(item) = items.iter().find(|i| i.atom == e.target) {
                if item.data.len() <= chunk {
                    conn.change_property8(PropMode::REPLACE, e.requestor, property, e.target, &item.data)?;
                } else {
                    // INCR: announce the size; the chunks follow as the requestor reads them
                    conn.change_window_attributes(e.requestor, &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE | EventMask::STRUCTURE_NOTIFY))?;
                    conn.change_property32(PropMode::REPLACE, e.requestor, property, atoms.INCR, &[item.data.len() as u32])?;
                    transfers.retain(|t| !(t.requestor == e.requestor && t.property == property));
                    if transfers.len() >= MAX_TRANSFERS {
                        transfers.remove(0);
                    }
                    transfers.push(Transfer { requestor: e.requestor, property, target: e.target, data: item.data.clone(), sent: 0 });
                }
                ok = true;
            }
        }
        let reply = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: e.time,
            requestor: e.requestor,
            selection: e.selection,
            target: e.target,
            property: if ok { property } else { NONE },
        };
        conn.send_event(false, e.requestor, EventMask::NO_EVENT, reply)?;
        conn.flush()?;
        Ok(())
    }

    /// The next INCR chunk; after the last one an empty property ends the transfer (false: done).
    fn send_chunk(conn: &RustConnection, t: &mut Transfer, chunk: usize) -> bool {
        let end = (t.sent + chunk).min(t.data.len());
        let more = t.sent < t.data.len();
        let _ = conn.change_property8(PropMode::REPLACE, t.requestor, t.property, t.target, &t.data[t.sent..end]);
        let _ = conn.flush();
        t.sent = end;
        more
    }

    /// No more property events from a requestor's window once its transfers are done.
    fn stop_watching(conn: &RustConnection, transfers: &[Transfer], requestor: Window) {
        if !transfers.iter().any(|t| t.requestor == requestor) {
            let _ = conn.change_window_attributes(requestor, &ChangeWindowAttributesAux::new().event_mask(EventMask::NO_EVENT));
            let _ = conn.flush();
        }
    }

    // ---- reading another program's data

    /// A connection of its own for one read, so its events don't mix with the serving thread's.
    pub struct Reader {
        conn: RustConnection,
        win: Window,
    }

    impl Reader {
        pub fn new() -> Option<Reader> {
            let (conn, screen) = RustConnection::connect(None).ok()?;
            let win = hidden_window(&conn, screen, EventMask::PROPERTY_CHANGE)?;
            conn.flush().ok()?;
            Some(Reader { conn, win })
        }

        /// The next event, or None once the deadline has passed.
        fn next_event(&self, deadline: Instant) -> Option<Event> {
            loop {
                if let Some(ev) = self.conn.poll_for_event().ok()? {
                    return Some(ev);
                }
                if !super::wait_readable(self.conn.stream(), deadline) {
                    return None;
                }
            }
        }

        /// Ask the owner of the clipboard for `target` and wait for the data (INCR transfers
        /// included). The bytes as they came (format 32 data in this connection's byte order).
        pub fn convert(&self, atoms: &Atoms, target: Atom, timeout: Duration) -> Option<Vec<u8>> {
            let c = &self.conn;
            c.delete_property(self.win, atoms.SELECTION_PROPERTY).ok()?;
            c.convert_selection(self.win, atoms.CLIPBOARD, target, atoms.SELECTION_PROPERTY, CURRENT_TIME).ok()?;
            c.flush().ok()?;
            let mut deadline = Instant::now() + timeout;
            let mut incr: Option<Vec<u8>> = None;
            let read = || c.get_property(true, self.win, atoms.SELECTION_PROPERTY, AtomEnum::ANY, 0, u32::MAX / 4).ok()?.reply().ok();
            loop {
                match self.next_event(deadline)? {
                    Event::SelectionNotify(e) if incr.is_none() && e.requestor == self.win && e.selection == atoms.CLIPBOARD && e.target == target => {
                        if e.property == NONE {
                            return None;
                        }
                        let r = read()?;
                        if r.type_ != atoms.INCR {
                            return Some(r.value);
                        }
                        // deleting the INCR property (done by reading it) starts the transfer
                        incr = Some(Vec::with_capacity(r.value32().and_then(|mut v| v.next()).unwrap_or(0).min(1 << 28) as usize));
                        deadline = Instant::now() + timeout;
                    }
                    Event::PropertyNotify(e) if incr.is_some() && e.window == self.win && e.atom == atoms.SELECTION_PROPERTY && e.state == Property::NEW_VALUE => {
                        let r = read()?;
                        if r.value.is_empty() {
                            return incr.take();
                        }
                        if let Some(buf) = incr.as_mut() {
                            buf.extend_from_slice(&r.value);
                        }
                        deadline = Instant::now() + timeout;
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Data-control first; X11 (or XWayland) when the compositor lacks it; nothing without a
    /// display. The Wayland check is not even tried outside Wayland.
    #[test]
    fn clipboard_backend_choice() {
        assert_eq!(choose(true, || true, true), Some(Backend::Wayland));
        assert_eq!(choose(true, || false, true), Some(Backend::X11), "GNOME: XWayland");
        assert_eq!(choose(true, || false, false), None);
        assert_eq!(choose(false, || panic!("asked Wayland in an X11 session"), true), Some(Backend::X11));
        assert_eq!(choose(false, || true, false), None);
    }

    /// A pipe that never ends gives up after the timeout; one that ends gives its data.
    #[test]
    fn pipe_reads_time_out() {
        let (r, mut w) = std::io::pipe().unwrap();
        std::io::Write::write_all(&mut w, b"partial").unwrap();
        let t = Instant::now();
        assert_eq!(read_pipe(r, Duration::from_millis(200)), None);
        assert!(t.elapsed() < Duration::from_secs(2));
        let (r, mut w) = std::io::pipe().unwrap();
        std::io::Write::write_all(&mut w, b"whole").unwrap();
        drop(w);
        assert_eq!(read_pipe(r, Duration::from_millis(200)).as_deref(), Some(&b"whole"[..]));
    }
}
