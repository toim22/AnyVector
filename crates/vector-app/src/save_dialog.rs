//! The "Save Drawing" dialog: the Windows save dialog (IFileSaveDialog) extended with the
//! drawing options of the reference screenshot:
//!
//! - Title / Subject / Rating (shell property fields, stored in the drawing's metadata)
//! - "Embed color profiles (size)" check box and the "Color profiles:" summary
//! - "Embed fonts (size)" check box
//! - "Version:" combo box
//! - "Advanced..." button (optimize for speed/size, compression, thumbnail, presentation
//!   attributes)

use std::path::PathBuf;

/// The options behind the "Advanced..." button (also on Options ▸ Document ▸ Save).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Advanced {
    pub optimize_for_size: bool,
    pub presentation_attributes: bool,
    pub bitmap_compression: bool,
    pub object_compression: bool,
    pub thumbnail: bool,
}

pub struct SaveRequest {
    pub dir: Option<PathBuf>,
    pub file_name: String,
    /// (display name, extension) pairs; the first is the default.
    pub types: Vec<(String, String)>,
    pub type_index: usize,
    pub title: String,
    pub subject: String,
    /// 0 (none) to 5 stars.
    pub rating: u8,
    pub embed_profiles: bool,
    pub profiles_size: u64,
    pub profiles_summary: String,
    pub embed_fonts: bool,
    pub fonts_size: u64,
    pub versions: Vec<String>,
    pub version: usize,
    pub advanced: Advanced,
}

#[derive(Debug)]
pub struct SaveResult {
    pub path: PathBuf,
    pub type_index: usize,
    pub title: String,
    pub subject: String,
    pub rating: u8,
    pub embed_profiles: bool,
    pub embed_fonts: bool,
    pub version: usize,
    pub advanced: Advanced,
}

/// "548 KB", "0 bytes", "12.4 MB".
pub fn size_text(bytes: u64) -> String {
    if bytes == 0 {
        "0 bytes".into()
    } else if bytes < 1024 {
        format!("{bytes} bytes")
    } else if bytes < 1024 * 1024 {
        format!("{} KB", bytes.div_ceil(1024))
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Shell rating (System.Rating is 1..99) <-> stars.
pub fn stars_to_rating(stars: u8) -> u32 {
    match stars {
        0 => 0,
        1 => 1,
        2 => 25,
        3 => 50,
        4 => 75,
        _ => 99,
    }
}

pub fn rating_to_stars(r: u32) -> u8 {
    match r {
        0 => 0,
        1..=12 => 1,
        13..=37 => 2,
        38..=62 => 3,
        63..=87 => 4,
        _ => 5,
    }
}

#[cfg(windows)]
pub use win::show;

#[cfg(not(windows))]
pub fn show(_req: SaveRequest) -> Option<SaveResult> {
    None
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::cell::Cell;
    use windows::core::{implement, w, Interface, Ref, BOOL, HSTRING, PCWSTR};
    #[allow(unused_imports)]
    use windows::Win32::UI::Shell::IFileDialogControlEvents as _;
    use windows::Win32::Foundation::{E_NOTIMPL, HWND, LPARAM, WPARAM};
    use windows::Win32::Storage::EnhancedStorage::{PKEY_Rating, PKEY_Subject, PKEY_Title};
    use windows::Win32::System::Com::StructuredStorage::{PropVariantToStringAlloc, PropVariantToUInt32, PROPVARIANT};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Ole::IOleWindow;
    use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyDescriptionList, IPropertyStore, PSCreateMemoryPropertyStore, PSGetPropertyDescriptionListFromString};
    use windows::Win32::UI::Shell::{
        FileSaveDialog, IFileDialog, IFileDialogControlEvents, IFileDialogControlEvents_Impl, IFileDialogCustomize, IFileDialogEvents, IFileDialogEvents_Impl, IFileSaveDialog, IShellItem, SHCreateItemFromParsingName, FDE_OVERWRITE_RESPONSE,
        FDE_SHAREVIOLATION_RESPONSE, FOS_OVERWRITEPROMPT, FOS_STRICTFILETYPES, SIGDN_FILESYSPATH,
    };
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        DialogBoxIndirectParamW, EndDialog, GetDlgItem, GetWindow, GetWindowLongPtrW, GetWindowRect, SendMessageW, SetWindowLongPtrW, SetWindowPos, BM_GETCHECK, BM_SETCHECK, DLGTEMPLATE, GWLP_USERDATA, GW_OWNER, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, WM_COMMAND, WM_INITDIALOG,
    };

    const ID_EMBED_PROFILES: u32 = 100;
    const ID_PROFILES_GROUP: u32 = 101;
    const ID_PROFILES_TEXT: u32 = 102;
    const ID_EMBED_FONTS: u32 = 103;
    const ID_VERSION_GROUP: u32 = 104;
    const ID_VERSION: u32 = 105;
    const ID_ADVANCED: u32 = 106;

    #[implement(IFileDialogEvents, IFileDialogControlEvents)]
    struct Events {
        advanced: Cell<Advanced>,
    }

    impl IFileDialogEvents_Impl for Events_Impl {
        fn OnFileOk(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> {
            Ok(())
        }
        fn OnFolderChanging(&self, _: Ref<IFileDialog>, _: Ref<IShellItem>) -> windows::core::Result<()> {
            Ok(())
        }
        fn OnFolderChange(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> {
            Ok(())
        }
        fn OnSelectionChange(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> {
            Ok(())
        }
        fn OnShareViolation(&self, _: Ref<IFileDialog>, _: Ref<IShellItem>) -> windows::core::Result<FDE_SHAREVIOLATION_RESPONSE> {
            Err(E_NOTIMPL.into())
        }
        fn OnTypeChange(&self, _: Ref<IFileDialog>) -> windows::core::Result<()> {
            Ok(())
        }
        fn OnOverwrite(&self, _: Ref<IFileDialog>, _: Ref<IShellItem>) -> windows::core::Result<FDE_OVERWRITE_RESPONSE> {
            Err(E_NOTIMPL.into())
        }
    }

    impl IFileDialogControlEvents_Impl for Events_Impl {
        fn OnItemSelected(&self, _: Ref<IFileDialogCustomize>, _: u32, _: u32) -> windows::core::Result<()> {
            Ok(())
        }
        fn OnButtonClicked(&self, pfdc: Ref<IFileDialogCustomize>, id: u32) -> windows::core::Result<()> {
            if id == ID_ADVANCED {
                let owner = pfdc.ok().ok().and_then(|c| c.cast::<IOleWindow>().ok()).and_then(|w| unsafe { w.GetWindow() }.ok()).unwrap_or_default();
                let mut a = self.advanced.get();
                if advanced_dialog(owner, &mut a) {
                    self.advanced.set(a);
                }
            }
            Ok(())
        }
        fn OnCheckButtonToggled(&self, _: Ref<IFileDialogCustomize>, _: u32, _: BOOL) -> windows::core::Result<()> {
            Ok(())
        }
        fn OnControlActivating(&self, _: Ref<IFileDialogCustomize>, _: u32) -> windows::core::Result<()> {
            Ok(())
        }
    }

    pub fn show(req: SaveRequest) -> Option<SaveResult> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            run(req).ok().flatten()
        }
    }

    unsafe fn run(req: SaveRequest) -> windows::core::Result<Option<SaveResult>> {
        let dlg: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
        dlg.SetTitle(w!("Save Drawing"))?;
        // file types
        let names: Vec<HSTRING> = req.types.iter().map(|(n, _)| HSTRING::from(n.as_str())).collect();
        let specs_s: Vec<HSTRING> = req.types.iter().map(|(_, e)| HSTRING::from(format!("*.{e}"))).collect();
        let specs: Vec<COMDLG_FILTERSPEC> = names.iter().zip(specs_s.iter()).map(|(n, s)| COMDLG_FILTERSPEC { pszName: PCWSTR(n.as_ptr()), pszSpec: PCWSTR(s.as_ptr()) }).collect();
        dlg.SetFileTypes(&specs)?;
        dlg.SetFileTypeIndex(req.type_index as u32 + 1)?;
        if let Some((_, e)) = req.types.get(req.type_index) {
            dlg.SetDefaultExtension(&HSTRING::from(e.as_str()))?;
        }
        dlg.SetFileName(&HSTRING::from(req.file_name.as_str()))?;
        if let Some(d) = &req.dir {
            if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(d.as_os_str()), None) {
                let _ = dlg.SetFolder(&item);
            }
        }
        let opts = dlg.GetOptions()?;
        dlg.SetOptions(opts | FOS_OVERWRITEPROMPT | FOS_STRICTFILETYPES)?;

        // Title / Subject / Rating
        let mut list: Option<IPropertyDescriptionList> = None;
        if PSGetPropertyDescriptionListFromString(w!("prop:System.Title;System.Subject;System.Rating"), &IPropertyDescriptionList::IID, &mut list as *mut _ as *mut _).is_ok() {
            if let Some(list) = &list {
                let _ = dlg.SetCollectedProperties(list, false);
            }
        }
        let mut store: Option<IPropertyStore> = None;
        if PSCreateMemoryPropertyStore(&IPropertyStore::IID, &mut store as *mut _ as *mut _).is_ok() {
            if let Some(store) = &store {
                if !req.title.is_empty() {
                    let _ = store.SetValue(&PKEY_Title, &PROPVARIANT::from(req.title.as_str()));
                }
                if !req.subject.is_empty() {
                    let _ = store.SetValue(&PKEY_Subject, &PROPVARIANT::from(req.subject.as_str()));
                }
                if req.rating > 0 {
                    let _ = store.SetValue(&PKEY_Rating, &PROPVARIANT::from(stars_to_rating(req.rating)));
                }
                let _ = dlg.SetProperties(store);
            }
        }

        // custom controls
        let c: IFileDialogCustomize = dlg.cast()?;
        c.AddCheckButton(ID_EMBED_PROFILES, &HSTRING::from(format!("Embed color profiles ({})", size_text(req.profiles_size))), req.embed_profiles)?;
        c.StartVisualGroup(ID_PROFILES_GROUP, w!("Color profiles:"))?;
        c.AddText(ID_PROFILES_TEXT, &HSTRING::from(req.profiles_summary.as_str()))?;
        c.EndVisualGroup()?;
        c.AddCheckButton(ID_EMBED_FONTS, &HSTRING::from(format!("Embed fonts ({})", size_text(req.fonts_size))), req.embed_fonts)?;
        c.StartVisualGroup(ID_VERSION_GROUP, w!("Ver&sion:"))?;
        c.AddComboBox(ID_VERSION)?;
        for (i, v) in req.versions.iter().enumerate() {
            c.AddControlItem(ID_VERSION, i as u32, &HSTRING::from(v.as_str()))?;
        }
        c.SetSelectedControlItem(ID_VERSION, req.version.min(req.versions.len().saturating_sub(1)) as u32)?;
        c.EndVisualGroup()?;
        c.AddPushButton(ID_ADVANCED, w!("&Advanced..."))?;
        // next to the Save button, as in the reference dialog
        let _ = c.MakeProminent(ID_ADVANCED);

        let events: IFileDialogEvents = Events { advanced: Cell::new(req.advanced) }.into();
        let cookie = dlg.Advise(&events)?;
        let owner = GetActiveWindow();
        let shown = dlg.Show(if owner.is_invalid() { None } else { Some(owner) });
        let _ = dlg.Unadvise(cookie);
        if shown.is_err() {
            return Ok(None); // cancelled
        }
        let item = dlg.GetResult()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = PathBuf::from(p.to_string().unwrap_or_default());
        CoTaskMemFree(Some(p.0 as *const _));
        let type_index = dlg.GetFileTypeIndex().map(|i| i.saturating_sub(1) as usize).unwrap_or(req.type_index);
        let (mut title, mut subject, mut rating) = (req.title.clone(), req.subject.clone(), req.rating);
        if let Ok(ps) = dlg.GetProperties() {
            let read = |k| -> Option<String> {
                let v = ps.GetValue(k).ok()?;
                let s = PropVariantToStringAlloc(&v).ok()?;
                let out = s.to_string().ok();
                CoTaskMemFree(Some(s.0 as *const _));
                out
            };
            title = read(&PKEY_Title).unwrap_or_default();
            subject = read(&PKEY_Subject).unwrap_or_default();
            rating = ps.GetValue(&PKEY_Rating).ok().and_then(|v| PropVariantToUInt32(&v).ok()).map(rating_to_stars).unwrap_or(0);
        }
        let embed_profiles = c.GetCheckButtonState(ID_EMBED_PROFILES).map(|b| b.as_bool()).unwrap_or(req.embed_profiles);
        let embed_fonts = c.GetCheckButtonState(ID_EMBED_FONTS).map(|b| b.as_bool()).unwrap_or(req.embed_fonts);
        let version = c.GetSelectedControlItem(ID_VERSION).map(|i| i as usize).unwrap_or(req.version);
        let advanced = windows::core::AsImpl::<Events>::as_impl(&events).advanced.get();
        Ok(Some(SaveResult { path, type_index, title, subject, rating, embed_profiles, embed_fonts, version, advanced }))
    }

    // ---------------------------------------------------------------------------------------
    // "Advanced Save Options" dialog, built from an in-memory dialog template
    // ---------------------------------------------------------------------------------------

    const IDC_SPEED: u16 = 201;
    const IDC_SIZE: u16 = 202;
    const IDC_PRESENTATION: u16 = 203;
    const IDC_BITMAP: u16 = 204;
    const IDC_OBJECTS: u16 = 205;
    const IDC_THUMB: u16 = 206;

    // styles
    const WS_CHILD: u32 = 0x4000_0000;
    const WS_VISIBLE: u32 = 0x1000_0000;
    const WS_TABSTOP: u32 = 0x0001_0000;
    const WS_GROUP: u32 = 0x0002_0000;
    const WS_POPUP: u32 = 0x8000_0000;
    const WS_CAPTION: u32 = 0x00C0_0000;
    const WS_SYSMENU: u32 = 0x0008_0000;
    const DS_SETFONT: u32 = 0x40;
    const DS_MODALFRAME: u32 = 0x80;
    const DS_CENTER: u32 = 0x0800;
    const BS_DEFPUSHBUTTON: u32 = 1;
    const BS_AUTOCHECKBOX: u32 = 3;
    const BS_GROUPBOX: u32 = 7;
    const BS_AUTORADIOBUTTON: u32 = 9;
    const BUTTON: u16 = 0x0080;

    struct Template(Vec<u16>);

    impl Template {
        fn u32(&mut self, v: u32) {
            self.0.push(v as u16);
            self.0.push((v >> 16) as u16);
        }
        fn str(&mut self, s: &str) {
            self.0.extend(s.encode_utf16());
            self.0.push(0);
        }
        fn align(&mut self) {
            if self.0.len() % 2 == 1 {
                self.0.push(0);
            }
        }
        #[allow(clippy::too_many_arguments)]
        fn item(&mut self, style: u32, x: i16, y: i16, cx: i16, cy: i16, id: u16, class: u16, text: &str) {
            self.align();
            self.u32(style | WS_CHILD | WS_VISIBLE);
            self.u32(0);
            for v in [x, y, cx, cy] {
                self.0.push(v as u16);
            }
            self.0.push(id);
            self.0.push(0xFFFF);
            self.0.push(class);
            self.str(text);
            self.0.push(0);
        }
    }

    fn template() -> Template {
        let mut t = Template(Vec::new());
        let items = 9u16;
        t.u32(WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME | DS_SETFONT | DS_CENTER);
        t.u32(0);
        t.0.push(items);
        for v in [0i16, 0, 236, 142] {
            t.0.push(v as u16);
        }
        t.0.push(0); // menu
        t.0.push(0); // class
        t.str("Advanced Save Options");
        t.0.push(9); // point size
        t.str("Segoe UI");
        t.item(BS_GROUPBOX, 7, 6, 222, 34, 0xFFFF, BUTTON, "Optimize for");
        t.item(BS_AUTORADIOBUTTON | WS_GROUP | WS_TABSTOP, 16, 21, 90, 10, IDC_SPEED, BUTTON, "S&peed");
        t.item(BS_AUTORADIOBUTTON, 112, 21, 90, 10, IDC_SIZE, BUTTON, "Si&ze");
        t.item(BS_AUTOCHECKBOX | WS_GROUP | WS_TABSTOP, 10, 50, 215, 10, IDC_PRESENTATION, BUTTON, "Save &presentation attributes (views)");
        t.item(BS_AUTOCHECKBOX | WS_TABSTOP, 10, 64, 215, 10, IDC_BITMAP, BUTTON, "Use &bitmap compression");
        t.item(BS_AUTOCHECKBOX | WS_TABSTOP, 10, 78, 215, 10, IDC_OBJECTS, BUTTON, "Use &graphic object compression");
        t.item(BS_AUTOCHECKBOX | WS_TABSTOP, 10, 92, 215, 10, IDC_THUMB, BUTTON, "Save &thumbnail");
        t.item(BS_DEFPUSHBUTTON | WS_GROUP | WS_TABSTOP, 124, 120, 50, 14, 1, BUTTON, "OK");
        t.item(WS_TABSTOP, 179, 120, 50, 14, 2, BUTTON, "Cancel");
        t
    }

    fn set_check(h: HWND, id: u16, on: bool) {
        unsafe {
            if let Ok(c) = GetDlgItem(Some(h), id as i32) {
                SendMessageW(c, BM_SETCHECK, Some(WPARAM(on as usize)), None);
            }
        }
    }

    fn get_check(h: HWND, id: u16) -> bool {
        unsafe { GetDlgItem(Some(h), id as i32).map(|c| SendMessageW(c, BM_GETCHECK, None, None).0 == 1).unwrap_or(false) }
    }

    unsafe extern "system" fn proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> isize {
        match msg {
            WM_INITDIALOG => {
                SetWindowLongPtrW(h, GWLP_USERDATA, lp.0);
                let a = &*(lp.0 as *const Advanced);
                set_check(h, IDC_SPEED, !a.optimize_for_size);
                set_check(h, IDC_SIZE, a.optimize_for_size);
                set_check(h, IDC_PRESENTATION, a.presentation_attributes);
                set_check(h, IDC_BITMAP, a.bitmap_compression);
                set_check(h, IDC_OBJECTS, a.object_compression);
                set_check(h, IDC_THUMB, a.thumbnail);
                // center over the Save dialog (DS_CENTER centers on the monitor)
                if let Ok(owner) = GetWindow(h, GW_OWNER) {
                    let (mut o, mut me) = (RECT::default(), RECT::default());
                    if GetWindowRect(owner, &mut o).is_ok() && GetWindowRect(h, &mut me).is_ok() {
                        let (w, hh) = (me.right - me.left, me.bottom - me.top);
                        let x = o.left + ((o.right - o.left) - w) / 2;
                        let y = o.top + ((o.bottom - o.top) - hh) / 2;
                        let _ = SetWindowPos(h, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
                    }
                }
                1
            }
            WM_COMMAND => {
                let id = (wp.0 & 0xFFFF) as i32;
                if id == 1 {
                    let a = &mut *(GetWindowLongPtrW(h, GWLP_USERDATA) as *mut Advanced);
                    a.optimize_for_size = get_check(h, IDC_SIZE);
                    a.presentation_attributes = get_check(h, IDC_PRESENTATION);
                    a.bitmap_compression = get_check(h, IDC_BITMAP);
                    a.object_compression = get_check(h, IDC_OBJECTS);
                    a.thumbnail = get_check(h, IDC_THUMB);
                    let _ = EndDialog(h, 1);
                    1
                } else if id == 2 {
                    let _ = EndDialog(h, 0);
                    1
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    /// Show the Advanced Save Options dialog; true if OK was pressed (then `a` is updated).
    pub fn advanced_dialog(owner: HWND, a: &mut Advanced) -> bool {
        let t = template();
        // DLGTEMPLATE must be DWORD aligned; a Vec<u16> only guarantees 2-byte alignment
        let mut buf = vec![0u32; t.0.len().div_ceil(2)];
        for (i, w) in t.0.iter().enumerate() {
            buf[i / 2] |= (*w as u32) << (16 * (i % 2));
        }
        let mut copy = *a;
        let r = unsafe {
            let inst = GetModuleHandleW(None).unwrap_or_default();
            DialogBoxIndirectParamW(Some(inst.into()), buf.as_ptr() as *const DLGTEMPLATE, if owner.is_invalid() { None } else { Some(owner) }, Some(proc), LPARAM(&mut copy as *mut Advanced as isize))
        };
        if r == -1 {
            log::warn!("Advanced Save Options dialog failed: {:?}", windows::core::Error::from_thread());
        }
        if r == 1 {
            *a = copy;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_ratings() {
        assert_eq!(size_text(0), "0 bytes");
        assert_eq!(size_text(561_000), "548 KB");
        assert_eq!(size_text(3 * 1024 * 1024), "3.0 MB");
        for s in 0..=5 {
            assert_eq!(rating_to_stars(stars_to_rating(s)), s);
        }
    }
}

/// Opens the Advanced Save Options dialog alone (manual check).
#[cfg(all(test, windows))]
#[test]
#[ignore]
fn advanced_dialog_manual() {
    let mut a = Advanced { optimize_for_size: true, presentation_attributes: true, bitmap_compression: false, object_compression: true, thumbnail: true };
    let ok = win::advanced_dialog(Default::default(), &mut a);
    println!("ADVANCED ok={ok} {a:?} err={:?}", windows::core::Error::from_thread());
}

/// Opens the real dialog (manual check): `cargo test -p vector-app save_dialog_manual -- --ignored`.
#[cfg(all(test, windows))]
#[test]
#[ignore]
fn save_dialog_manual() {
    let r = show(SaveRequest {
        dir: dirs::document_dir(),
        file_name: "Untitled-1.cdr".into(),
        types: vec![("CDR - CorelDRAW (*.cdr)".into(), "cdr".into()), ("VCT - AnyVector Drawing (*.vct)".into(), "vct".into())],
        type_index: 0,
        title: String::new(),
        subject: String::new(),
        rating: 0,
        embed_profiles: true,
        profiles_size: 3144,
        profiles_summary: "sRGB IEC61966-2.1; U.S. Web Coated (SWOP) v2; Dot Gain 20%".into(),
        embed_fonts: true,
        fonts_size: 0,
        versions: vec!["0.1 (2026.0)".into()],
        version: 0,
        advanced: Advanced { optimize_for_size: false, presentation_attributes: true, bitmap_compression: true, object_compression: true, thumbnail: true },
    });
    println!("{r:?}");
}
