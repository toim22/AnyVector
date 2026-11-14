//! Edit ▸ Paste Special (choose the clipboard format to paste) and Edit ▸ Copy Properties From
//! (choose which properties to copy, then click the object to copy them from).

use super::{button, ok_button, window};
use crate::app::AnyVectorApp;
use crate::theme;
use egui::vec2;

/// The formats Paste Special can paste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteKind {
    /// Objects copied in AnyVector.
    Native,
    Svg,
    Text,
    /// An Enhanced Metafile (CF_ENHMETAFILE) from another program.
    Metafile,
    Bitmap,
}

impl PasteKind {
    pub fn name(self) -> &'static str {
        match self {
            PasteKind::Native => "AnyVector Graphic",
            PasteKind::Svg => "SVG Document",
            PasteKind::Text => "Text",
            PasteKind::Metafile => "Picture (Enhanced Metafile)",
            PasteKind::Bitmap => "Device Independent Bitmap",
        }
    }
    fn result(self) -> &'static str {
        match self {
            PasteKind::Native => "Inserts the contents of the clipboard into your document as AnyVector objects.",
            PasteKind::Svg => "Inserts the contents of the clipboard into your document as SVG objects.",
            PasteKind::Text => "Inserts the contents of the clipboard into your document as Text.",
            PasteKind::Metafile => "Inserts the contents of the clipboard into your document as Picture (Enhanced Metafile).",
            PasteKind::Bitmap => "Inserts the contents of the clipboard into your document as a bitmap.",
        }
    }
}

// ------------------------------------------------------------------------------------------
// copying to the system clipboard

/// The private clipboard format that marks a copy made in AnyVector (it holds the SVG).
pub const OWN_FORMAT: &str = "AnyVector Graphic";
/// The SVG picture format current Office versions paste as an editable SVG graphic.
pub const SVG_FORMAT: &str = "image/svg+xml";

/// A format put on the system clipboard by Copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipFormat {
    /// A format registered by name (RegisterClipboardFormat).
    Named(&'static str),
    /// CF_ENHMETAFILE: the bytes become a metafile handle.
    EnhMetafile,
    /// CF_DIB: a packed device independent bitmap.
    Dib,
}

/// What Copy puts on the system clipboard, in order of preference, all at once: AnyVector's own
/// marker (so Paste brings back the objects), the SVG (Office 365 pastes an editable SVG
/// picture), the EMF+ metafile (older Office and other programs: curves kept), and PNG / DIB
/// bitmaps. There is deliberately no text: Office would paste the SVG source as text.
pub fn copy_formats(svg: &str, emf: Vec<u8>, bitmap: Option<(u32, u32, &[u8])>) -> Vec<(ClipFormat, Vec<u8>)> {
    let mut v = vec![];
    let own = svg.replacen("<svg", "<svg data-vector-clip=\"1\"", 1);
    v.push((ClipFormat::Named(OWN_FORMAT), own.into_bytes()));
    v.push((ClipFormat::Named(SVG_FORMAT), svg.as_bytes().to_vec()));
    if !emf.is_empty() {
        v.push((ClipFormat::EnhMetafile, emf));
    }
    if let Some((w, h, rgba)) = bitmap {
        v.push((ClipFormat::Named("PNG"), vector_io::image_io::encode_png(w, h, rgba)));
        // the DIB is opaque (many programs ignore its alpha): composited on white
        v.push((ClipFormat::Dib, vector_io::emf::dib_from_rgba(w, h, rgba, false, Some(vector_core::Color::WHITE))));
    }
    v
}

#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn SetClipboardData(format: u32, mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}

/// Put the formats on the system clipboard together (one open, so none replaces another).
/// False when the clipboard could not be opened.
pub fn set_system(formats: &[(ClipFormat, Vec<u8>)]) -> bool {
    if !system_enabled() {
        return false;
    }
    #[cfg(windows)]
    {
        use clipboard_win::{formats as cf, raw, register_format, Clipboard};
        let Ok(_open) = Clipboard::new_attempts(10) else { return false };
        if raw::empty().is_err() {
            return false;
        }
        for (f, data) in formats {
            match f {
                ClipFormat::Named(name) => {
                    if let Some(id) = register_format(name) {
                        let _ = raw::set_without_clear(id.get(), data);
                    }
                }
                ClipFormat::Dib => {
                    let _ = raw::set_without_clear(cf::CF_DIB, data);
                }
                ClipFormat::EnhMetafile => unsafe {
                    let h = windows::Win32::Graphics::Gdi::SetEnhMetaFileBits(data);
                    if !h.is_invalid() && SetClipboardData(cf::CF_ENHMETAFILE, h.0).is_null() {
                        // not taken by the clipboard: still ours to free
                        let _ = windows::Win32::Graphics::Gdi::DeleteEnhMetaFile(Some(h));
                    }
                },
            }
        }
        true
    }
    #[cfg(not(windows))]
    {
        // elsewhere: the SVG (with the marker) as text, which Paste recognizes
        let Ok(mut cb) = arboard::Clipboard::new() else { return false };
        match formats.iter().find(|(f, _)| *f == ClipFormat::Named(OWN_FORMAT)) {
            Some((_, svg)) => cb.set_text(String::from_utf8_lossy(svg).into_owned()).is_ok(),
            None => false,
        }
    }
}

/// Bytes of a format registered by name on the system clipboard.
pub fn system_named(name: &str) -> Option<Vec<u8>> {
    if !system_enabled() {
        return None;
    }
    #[cfg(windows)]
    {
        use clipboard_win::{raw, register_format, Clipboard};
        let f = register_format(name).filter(|f| raw::is_format_avail(f.get()))?;
        let _open = Clipboard::new_attempts(10).ok()?;
        let mut v = vec![];
        raw::get_vec(f.get(), &mut v).ok()?;
        (!v.is_empty()).then_some(v)
    }
    #[cfg(not(windows))]
    {
        let _ = name;
        None
    }
}

/// The SVG of a copy made in AnyVector (with the data-vector-clip marker), if the clipboard
/// holds one.
pub fn system_own() -> Option<String> {
    system_named(OWN_FORMAT).map(|v| String::from_utf8_lossy(&v).trim_end_matches('\0').to_string())
}

/// An SVG picture another program put on the clipboard ("image/svg+xml").
pub fn system_svg() -> Option<String> {
    system_named(SVG_FORMAT).map(|v| String::from_utf8_lossy(&v).trim_end_matches('\0').to_string()).filter(|s| s.contains("<svg"))
}

/// Whether the clipboard holds an Enhanced Metafile (cheap: no data is read).
pub fn system_has_emf() -> bool {
    if !system_enabled() {
        return false;
    }
    #[cfg(windows)]
    return clipboard_win::is_format_avail(clipboard_win::formats::CF_ENHMETAFILE);
    #[cfg(not(windows))]
    false
}

/// The Enhanced Metafile on the clipboard, as bytes.
pub fn system_emf() -> Option<Vec<u8>> {
    if !system_has_emf() {
        return None;
    }
    #[cfg(windows)]
    {
        use clipboard_win::{formats as cf, raw, Clipboard};
        use windows::Win32::Graphics::Gdi::{GetEnhMetaFileBits, HENHMETAFILE};
        let _open = Clipboard::new_attempts(10).ok()?;
        let h = HENHMETAFILE(raw::get_clipboard_data(cf::CF_ENHMETAFILE).ok()?.as_ptr());
        // the handle belongs to the clipboard: read it, never delete it
        let n = unsafe { GetEnhMetaFileBits(h, None) } as usize;
        if n == 0 {
            return None;
        }
        let mut v = vec![0u8; n];
        let got = unsafe { GetEnhMetaFileBits(h, Some(&mut v)) } as usize;
        v.truncate(got);
        (got > 0).then_some(v)
    }
    #[cfg(not(windows))]
    None
}

pub struct PasteSpecialState {
    pub source: String,
    pub kinds: Vec<PasteKind>,
    pub chosen: usize,
}

// ------------------------------------------------------------------------------------------
// system clipboard

/// A bitmap read from the system clipboard (RGBA, plus the PNG bytes when it came as PNG).
pub struct ClipImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub png: Vec<u8>,
}

#[cfg(test)]
thread_local! {
    /// Tests run without touching the user's clipboard unless they turn it on.
    pub static SYSTEM_CLIPBOARD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the system clipboard is used (always, outside tests).
pub fn system_enabled() -> bool {
    #[cfg(test)]
    return SYSTEM_CLIPBOARD.with(|c| c.get());
    #[cfg(not(test))]
    true
}

/// Whether the system clipboard holds text or a bitmap (cheap: no data is read on Windows).
pub fn system_has_content() -> bool {
    if !system_enabled() {
        return false;
    }
    #[cfg(windows)]
    {
        use clipboard_win::{formats, is_format_avail, register_format};
        let named = ["PNG", "image/png", OWN_FORMAT, SVG_FORMAT].iter().any(|n| register_format(n).is_some_and(|f| is_format_avail(f.get())));
        named || [formats::CF_UNICODETEXT, formats::CF_DIB, formats::CF_DIBV5, formats::CF_BITMAP, formats::CF_ENHMETAFILE].iter().any(|f| is_format_avail(*f))
    }
    #[cfg(not(windows))]
    {
        arboard::Clipboard::new().map(|mut cb| cb.get_text().is_ok_and(|t| !t.is_empty()) || cb.get_image().is_ok()).unwrap_or(false)
    }
}

/// The text on the system clipboard (None when there is none or it is empty).
pub fn system_text() -> Option<String> {
    if !system_enabled() {
        return None;
    }
    arboard::Clipboard::new().ok()?.get_text().ok().filter(|t| !t.is_empty())
}

/// The bitmap on the system clipboard: the "PNG" format first (browsers, Office), then
/// CF_DIBV5 / CF_DIB (Windows synthesizes these from CF_BITMAP too).
pub fn system_image() -> Option<ClipImage> {
    if !system_enabled() {
        return None;
    }
    #[cfg(windows)]
    {
        use clipboard_win::{formats, raw, register_format, Clipboard};
        let read = || -> Option<ClipImage> {
            let _open = Clipboard::new_attempts(10).ok()?;
            for name in ["PNG", "image/png"] {
                if let Some(f) = register_format(name).filter(|f| raw::is_format_avail(f.get())) {
                    let mut v = vec![];
                    if raw::get_vec(f.get(), &mut v).is_ok() {
                        if let Some(img) = decode_png(&v) {
                            return Some(img);
                        }
                    }
                }
            }
            for f in [formats::CF_DIBV5, formats::CF_DIB] {
                if raw::is_format_avail(f) {
                    let mut v = vec![];
                    if raw::get_vec(f, &mut v).is_ok() {
                        if let Some(img) = decode_dib(&v) {
                            return Some(img);
                        }
                    }
                }
            }
            None
        };
        if let Some(img) = read() {
            return Some(img);
        }
    }
    let img = arboard::Clipboard::new().ok()?.get_image().ok()?;
    let mut rgba = img.bytes.into_owned();
    fix_zero_alpha(&mut rgba);
    Some(ClipImage { width: img.width as u32, height: img.height as u32, rgba, png: vec![] })
}

/// Decode PNG bytes from the clipboard.
pub fn decode_png(data: &[u8]) -> Option<ClipImage> {
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Png).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    (width > 0 && height > 0).then(|| ClipImage { width, height, rgba: img.into_raw(), png: data.to_vec() })
}

/// Decode a packed DIB (CF_DIB / CF_DIBV5: a BITMAPINFOHEADER/V4/V5 header, optional masks or
/// palette, then the pixels) by giving it a BMP file header.
pub fn decode_dib(dib: &[u8]) -> Option<ClipImage> {
    let u32_at = |o: usize| dib.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let u16_at = |o: usize| dib.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let hsize = u32_at(0)? as usize;
    if !(12..=dib.len()).contains(&hsize) {
        return None;
    }
    let (bits, compression, clr_used) = if hsize == 12 { (u16_at(10)?, 0, 0) } else { (u16_at(14)?, u32_at(16)?, u32_at(32)?) };
    // BI_BITFIELDS (3) / BI_ALPHABITFIELDS (6) after a 40-byte header: the masks follow it
    let masks = match (hsize, compression) {
        (40, 3) => 12,
        (40, 6) => 16,
        _ => 0,
    };
    let entry = if hsize == 12 { 3 } else { 4 };
    let colors = if clr_used > 0 { clr_used as usize } else if bits <= 8 { 1usize << bits } else { 0 };
    let offset = 14 + hsize + masks + colors * entry;
    let mut file = Vec::with_capacity(14 + dib.len());
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&((14 + dib.len()) as u32).to_le_bytes());
    file.extend_from_slice(&[0; 4]);
    file.extend_from_slice(&(offset as u32).to_le_bytes());
    file.extend_from_slice(dib);
    let img = image::load_from_memory_with_format(&file, image::ImageFormat::Bmp).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    let mut rgba = img.into_raw();
    fix_zero_alpha(&mut rgba);
    (width > 0 && height > 0).then_some(ClipImage { width, height, rgba, png: vec![] })
}

/// 32-bit DIBs often carry an unused (all zero) alpha byte: treat such a bitmap as opaque.
fn fix_zero_alpha(rgba: &mut [u8]) {
    if !rgba.is_empty() && rgba.chunks_exact(4).all(|p| p[3] == 0) {
        for p in rgba.chunks_exact_mut(4) {
            p[3] = 255;
        }
    }
}

/// Text that only names the copied bitmap (a browser's "Copy image" may add the image URL or
/// an `<img>` tag as text): the bitmap is pasted instead of the text.
pub fn text_is_image_link(t: &str) -> bool {
    let t = t.trim();
    if t.is_empty() {
        return true;
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("<img") && !t.contains('\n') {
        return true;
    }
    !t.contains(char::is_whitespace) && ["http://", "https://", "file:", "data:image/", "blob:"].iter().any(|p| lower.starts_with(p))
}

/// Where pasted outside content goes: the center of the view (the page center before the view
/// has been laid out).
pub fn paste_center(d: &crate::docview::DocView) -> kurbo::Point {
    if d.canvas_rect.is_positive() {
        d.to_doc(d.canvas_rect.center())
    } else {
        d.page_ref().rect().center()
    }
}

/// What the clipboard holds, best format first.
pub fn clipboard_kinds(app: &AnyVectorApp) -> (Vec<PasteKind>, String) {
    let mut kinds = vec![];
    let text = system_text();
    let image = system_image().is_some();
    let own = system_own().is_some();
    let ours = own || text.as_deref().is_some_and(|t| t.contains("data-vector-clip"));
    let svg = own || system_svg().is_some();
    let emf = !ours && system_has_emf();
    if (ours || (text.is_none() && !image && !svg && !emf)) && !app.clip.objects.is_empty() {
        kinds.push(PasteKind::Native);
    }
    if svg {
        kinds.push(PasteKind::Svg);
    }
    // a bitmap with only its URL as text: the bitmap comes first
    let link = text.as_deref().is_some_and(text_is_image_link);
    if image && link {
        kinds.push(PasteKind::Bitmap);
    }
    if let Some(t) = &text {
        let t = t.trim();
        if (t.starts_with("<svg") || t.starts_with("<?xml")) && !svg {
            kinds.push(PasteKind::Svg);
        }
        if !t.is_empty() && !ours {
            kinds.push(PasteKind::Text);
        }
    }
    if emf {
        kinds.push(PasteKind::Metafile);
    }
    if image && !link {
        kinds.push(PasteKind::Bitmap);
    }
    let source = if ours || kinds.first() == Some(&PasteKind::Native) { vector_core::APP_NAME.to_string() } else { "Unknown Source".to_string() };
    (kinds, source)
}

pub fn paste_special(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.paste_special.take() else { return };
    let mut ok = false;
    let mut cancel = false;
    let ((), dismissed) = window(ctx, "paste_special", "Paste Special", vec2(456.0, 267.0), |ui| {
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(330.0);
                ui.horizontal(|ui| {
                    ui.label("Source:");
                    ui.label(&st.source);
                });
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(90.0);
                        ui.add_space(26.0);
                        let _ = ui.radio(true, "Paste");
                        ui.add_space(14.0);
                        ui.add_enabled(false, egui::RadioButton::new(false, "Paste Link"));
                    });
                    ui.vertical(|ui| {
                        ui.label("As:");
                        egui::Frame::new().fill(theme::FIELD()).stroke(egui::Stroke::new(1.0, theme::SEP())).inner_margin(2.0).show(ui, |ui| {
                            ui.set_min_size(vec2(226.0, 80.0));
                            egui::ScrollArea::vertical().max_height(80.0).show(ui, |ui| {
                                for (i, k) in st.kinds.iter().enumerate() {
                                    if ui.selectable_label(st.chosen == i, k.name()).clicked() {
                                        st.chosen = i;
                                    }
                                }
                            });
                        });
                    });
                });
                ui.add_space(6.0);
                super::group(ui, "Result", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("📋").size(22.0));
                        let text = st.kinds.get(st.chosen).map(|k| k.result()).unwrap_or("The clipboard is empty.");
                        ui.add(egui::Label::new(text).wrap());
                    });
                });
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ok = ok_button(ui, "OK", !st.kinds.is_empty());
                cancel = button(ui, "Cancel", true);
                ui.add_space(50.0);
                ui.add_enabled(false, egui::Checkbox::new(&mut false, "Display As Icon"));
            });
        });
    });
    if ok {
        let kind = st.kinds.get(st.chosen).copied();
        app.paste_as(kind, None);
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.paste_special = Some(st);
    }
}

#[derive(Clone, Default)]
pub struct CopyPropsState {
    pub pen: bool,
    pub color: bool,
    pub fill: bool,
    pub text: bool,
}

pub fn copy_properties(app: &mut AnyVectorApp, ctx: &egui::Context) {
    let Some(mut st) = app.dialogs.copy_props.take() else { return };
    // Text Properties needs text in the selection
    let has_text = app.doc().is_some_and(|d| d.selection.iter().any(|id| d.doc.find(*id).is_some_and(|o| matches!(o.kind, vector_core::ObjKind::Text(_)))));
    if !has_text {
        st.text = false;
    }
    let (mut ok, mut cancel) = (false, false);
    let ((), dismissed) = window(ctx, "copy_props", "Copy Properties", vec2(424.0, 253.0), |ui| {
        ui.add_space(6.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(170.0);
                ui.checkbox(&mut st.pen, "Outline Pen");
                ui.checkbox(&mut st.color, "Outline Color");
                ui.checkbox(&mut st.fill, "Fill");
                ui.add_enabled(has_text, egui::Checkbox::new(&mut st.text, "Text Properties"));
            });
            ui.vertical(|ui| {
                egui::Frame::new().stroke(egui::Stroke::new(1.0, theme::SEP())).inner_margin(10.0).show(ui, |ui| {
                    ui.set_width(196.0);
                    ui.set_min_height(86.0);
                    ui.add(egui::Label::new("Hint: You may copy properties by dragging one object onto another using the right mouse button.").wrap());
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let any = st.pen || st.color || st.fill || st.text;
                    ok = ok_button(ui, "OK", any);
                    ui.add_space(8.0);
                    cancel = button(ui, "Cancel", true);
                });
            });
        });
        ui.add_space(10.0);
        ui.separator();
        ui.label(egui::RichText::new("After pressing OK, choose the object to copy from.").color(crate::theme::gray(0xc8)));
    });
    if ok {
        app.set_tool(crate::tools::ToolId::Pick);
        app.ts.pick_target = Some(crate::tools::PickTarget::CopyProperties { pen: st.pen, color: st.color, fill: st.fill, text: st.text });
        app.flash("Click the object to copy properties from.");
        return;
    }
    if !(cancel || dismissed) {
        app.dialogs.copy_props = Some(st);
    }
}
