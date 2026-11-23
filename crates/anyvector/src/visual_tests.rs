//! Headless renders of the main window for visual comparison with the design screenshots.
//! Run with `cargo test -p anyvector visual -- --ignored`; images go to `target/visual/`.
//! With `ANYVECTOR_VISUAL_THEME=Light` every render uses the Light theme instead and the images
//! go to `target/visual/light/`.

use crate::app::AnyVectorApp;
use crate::commands::Cmd;
use egui_kittest::Harness;
use kurbo::{Affine, Point};
use vector_core::object::{EllipseKind, EllipseShape, PolygonKind, PolygonShape, RectShape, CornerType};
use vector_core::{Color, Fill, ObjKind, Object, Style};

/// The theme named by `ANYVECTOR_VISUAL_THEME`, if it is not the default Dark.
fn visual_theme() -> Option<String> {
    std::env::var("ANYVECTOR_VISUAL_THEME").ok().filter(|t| !t.trim().is_empty() && !t.eq_ignore_ascii_case("dark"))
}

fn out_dir() -> std::path::PathBuf {
    let mut d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/visual");
    if let Some(t) = visual_theme() {
        d = d.join(t.to_lowercase());
    }
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn harness<'a>() -> Harness<'a, AnyVectorApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1920.0, 1051.0)).wgpu().build_eframe(|cc| {
        let mut app = AnyVectorApp::new(cc);
        app.splash = None;
        app.pending.clear();
        if let Some(t) = visual_theme() {
            app.settings.theme_name = t;
        }
        app
    });
    for _ in 0..3 {
        h.step();
    }
    h
}

fn save(h: &mut Harness<'_, AnyVectorApp>, name: &str) {
    for _ in 0..3 {
        h.step();
    }
    // The wgpu readback can time out on a busy GPU (e.g. right after another
    // harness was torn down); retry a few times before failing.
    let mut result = h.render();
    for _ in 0..4 {
        if result.is_ok() {
            break;
        }
        h.step();
        result = h.render();
    }
    let img = result.expect("render");
    img.save(out_dir().join(format!("{name}.png"))).unwrap();
}

#[test]
#[ignore]
fn visual_home_and_document() {
    let mut h = harness();
    save(&mut h, "01_home");
    h.state_mut().welcome_page = crate::welcome::Page::Workspace;
    save(&mut h, "01_home_workspace");
    h.state_mut().run(Cmd::WhatsNew);
    save(&mut h, "01_home_whats_new");
    h.state_mut().welcome_page = crate::welcome::Page::GetStarted;

    h.state_mut().run(Cmd::New);
    for _ in 0..3 {
        h.step();
    }
    // finish the New Document dialog if it is shown
    h.state_mut().dialogs.new_doc = None;
    if !h.state().has_doc() {
        h.state_mut().new_default_document();
    }
    {
        let app = h.state_mut();
        let d = app.doc_mut().unwrap();
        let mut objs = vec![];
        let mut add = |kind: ObjKind, at: (f64, f64), fill: Color| {
            let id = d.doc.new_id();
            let mut st = Style::default();
            st.fill = Fill::Solid(fill);
            st.outline.width = 1.0;
            st.outline.color = Some(Color::BLACK);
            objs.push(Object::new(id, kind, Affine::translate(at), st));
        };
        add(ObjKind::Rect(RectShape { w: 180.0, h: 120.0, radii: [12.0; 4], corner: CornerType::Round }), (72.0, 72.0), Color::rgb(0x1e, 0x90, 0xff));
        add(ObjKind::Ellipse(EllipseShape { w: 160.0, h: 160.0, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false }), (300.0, 90.0), Color::rgb(0xff, 0x60, 0x40));
        add(ObjKind::Polygon(PolygonShape { w: 150.0, h: 150.0, sides: 5, kind: PolygonKind::Star, sharpness: 53.0, nodes: Default::default() }), (200.0, 300.0), Color::rgb(0xff, 0xd0, 0x20));
        d.add_objects("test", objs);
        let first = d.selection[0];
        d.selection = vec![first];
        let _ = Point::ZERO;
    }
    save(&mut h, "02_document");

    h.state_mut().set_tool(crate::tools::ToolId::Shape);
    save(&mut h, "03_shape_tool");

    h.state_mut().set_tool(crate::tools::ToolId::Pick);
    h.state_mut().open_options(crate::commands::OptionsPage::App, None);
    save(&mut h, "04_options");
}

fn press(h: &mut Harness<'_, AnyVectorApp>, p: egui::Pos2, down: bool) {
    h.input_mut().events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: down, modifiers: egui::Modifiers::NONE });
}

fn move_to(h: &mut Harness<'_, AnyVectorApp>, p: egui::Pos2) {
    h.input_mut().events.push(egui::Event::PointerMoved(p));
}

fn drag(h: &mut Harness<'_, AnyVectorApp>, a: egui::Pos2, b: egui::Pos2) {
    move_to(h, a);
    h.step();
    press(h, a, true);
    h.step();
    for k in 1..=6 {
        move_to(h, a + (b - a) * (k as f32 / 6.0));
        h.step();
    }
    press(h, b, false);
    h.step();
}

fn click(h: &mut Harness<'_, AnyVectorApp>, p: egui::Pos2) {
    move_to(h, p);
    h.step();
    press(h, p, true);
    h.step();
    press(h, p, false);
    h.step();
}

/// Drive every tool with a drag and a few clicks; nothing may panic.
#[test]
#[ignore]
fn visual_every_tool_smoke() {
    use crate::tools::{ToolId, TOOLS};
    let mut h = harness();
    h.state_mut().new_default_document();
    h.state_mut().dialogs.new_doc = None;
    for _ in 0..3 {
        h.step();
    }
    let a = egui::pos2(800.0, 300.0);
    let b = egui::pos2(1000.0, 450.0);
    // a few objects to work on
    h.state_mut().set_tool(ToolId::Rectangle);
    drag(&mut h, a, b);
    h.state_mut().set_tool(ToolId::Ellipse);
    drag(&mut h, egui::pos2(850.0, 500.0), egui::pos2(1050.0, 650.0));
    let n0 = h.state().doc().unwrap().page_ref().layers[0].objects.len();
    assert!(n0 >= 2, "shapes were not created by dragging ({n0})");
    for t in TOOLS.iter().map(|t| t.id) {
        if t == ToolId::Pan || t == ToolId::Zoom {
            continue;
        }
        h.state_mut().set_tool(t);
        // select the rectangle first so selection-based tools have a target
        if let Some(d) = h.state_mut().doc_mut() {
            if let Some(o) = d.page_ref().layers[0].objects.first() {
                d.selection = vec![o.id];
            }
        }
        drag(&mut h, egui::pos2(900.0, 380.0), egui::pos2(960.0, 560.0));
        click(&mut h, egui::pos2(700.0, 700.0));
        click(&mut h, egui::pos2(720.0, 720.0));
        if t == ToolId::Text {
            h.input_mut().events.push(egui::Event::Text("Hello".into()));
            h.step();
        }
        h.state_mut().finish_tool();
        h.step();
    }
    let has_text = h.state().doc().unwrap().page_ref().all_objects().any(|o| matches!(&o.kind, ObjKind::Text(t) if t.content.contains("Hello")));
    assert!(has_text, "typing with the Text tool did not create text");
    h.state_mut().set_tool(ToolId::Pick);
    save(&mut h, "05_after_all_tools");
}

#[test]
#[ignore]
fn visual_new_document_and_bug_report() {
    let mut h = harness();
    h.state_mut().settings.show_new_doc_dialog = true;
    h.state_mut().run(Cmd::New);
    save(&mut h, "06_new_document");
    // scroll the preset grid to the B5 (JIS) row by searching
    if let Some(st) = h.state_mut().dialogs.new_doc.as_mut() {
        st.filter = 3; // Web
    }
    save(&mut h, "07_new_document_web");
    if let Some(st) = h.state_mut().dialogs.new_doc.as_mut() {
        st.tab = 1;
    }
    save(&mut h, "08_new_document_templates");
    h.state_mut().dialogs.new_doc = None;
    h.state_mut().run(Cmd::ReportBug);
    save(&mut h, "09_report_bug");
}

#[test]
#[ignore]
fn visual_about_and_system_info() {
    let mut h = harness();
    h.state_mut().new_default_document();
    h.state_mut().dialogs.about = true;
    save(&mut h, "10_about");
    let gpu = h.state().gpu.clone();
    h.state_mut().dialogs.system_info = Some(crate::about::SystemInfoState::new(&gpu));
    save(&mut h, "11_system_info");
    // the other categories (on Linux the monitors, CUPS' printers and the libraries loaded), and the
    // first in Brazilian Portuguese
    for i in 1..crate::sysinfo::CATEGORIES.len() {
        h.state_mut().dialogs.system_info = Some(crate::about::SystemInfoState { category: i, rows: crate::sysinfo::collect(i, &gpu) });
        save(&mut h, &format!("11_system_info_{i}"));
    }
    h.state_mut().dialogs.system_info = Some(crate::about::SystemInfoState::new(&gpu));
    h.state_mut().settings.language = "pt-BR".into();
    save(&mut h, "11_system_info_ptbr");
    h.state_mut().settings.language = "en-US".into();
    h.state_mut().dialogs.system_info = None;
    h.state_mut().dialogs.legal_notices = true;
    save(&mut h, "12_legal_notices");
}

#[test]
#[ignore]
fn visual_splash() {
    let mut h = Harness::builder().with_size(egui::vec2(620.0, 380.0)).wgpu().build_eframe(|cc| AnyVectorApp::new(cc));
    // the splash shows for 1.8 s of app time; render it right after it has faded in
    for _ in 0..2 {
        h.step();
    }
    let img = h.render().expect("render");
    img.save(out_dir().join("00_splash.png")).unwrap();
}

/// Open a menu of the menu bar. Waits first, so the menu closed before has finished fading out.
fn open_menu(h: &mut Harness<'_, AnyVectorApp>, label: &str) {
    use egui_kittest::kittest::Queryable;
    for _ in 0..30 {
        h.step();
    }
    h.get_by_label(label).click_accesskit();
}

fn close_menu(h: &mut Harness<'_, AnyVectorApp>) {
    h.key_press(egui::Key::Escape);
    h.step();
}

/// Open each menu of the menu bar, with no document and with a rectangle selected.
#[test]
#[ignore]
fn visual_menus() {
    let mut h = harness();
    for (i, m) in ["File", "Tools", "Window", "Help"].iter().enumerate() {
        open_menu(&mut h, m);
        save(&mut h, &format!("20_menu_nodoc_{:02}_{}", i + 1, m.to_lowercase()));
        close_menu(&mut h);
    }
    h.state_mut().new_default_document();
    {
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::rgb(0x1e, 0x90, 0xff));
        let o = Object::new(id, ObjKind::Rect(RectShape { w: 180.0, h: 120.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((72.0, 72.0)), st);
        d.add_objects("test", vec![o]);
    }
    for (i, m) in ["File", "Edit", "View", "Layout", "Object", "Effects", "Bitmaps", "Text", "Table", "Tools", "Window", "Help"].iter().enumerate() {
        open_menu(&mut h, m);
        save(&mut h, &format!("21_menu_{:02}_{}", i + 1, m.to_lowercase()));
        close_menu(&mut h);
    }
}

/// The interface in Brazilian Portuguese: Welcome Screen, a menu, a document and the Options dialog.
#[test]
#[ignore]
fn visual_pt_br() {
    let mut h = harness();
    h.state_mut().settings.language = "pt-BR".into();
    save(&mut h, "90_ptbr_home");
    h.state_mut().new_default_document();
    open_menu(&mut h, "Arquivo");
    save(&mut h, "90_ptbr_menu_file");
    close_menu(&mut h);
    open_menu(&mut h, "Objeto");
    save(&mut h, "90_ptbr_menu_object");
    close_menu(&mut h);
    // the Updates dialog alone, without a network check
    h.state_mut().update.dialog = true;
    save(&mut h, "90_ptbr_updates");
    h.state_mut().update.dialog = false;
    h.state_mut().open_options(crate::commands::OptionsPage::Global, None);
    save(&mut h, "90_ptbr_options");
    h.state_mut().settings.language = "en-US".into();
    h.step();
    assert_eq!(crate::i18n::lang(), crate::i18n::Lang::EnUs);
}

/// The other languages: the main window with a document, and the Options dialog, in a Latin, a Cyrillic and the
/// Chinese and Japanese scripts (their characters come from the CJK font `i18n::add_fonts` adds). Also the language
/// chooser, in English, which shows every language by its own name.
#[test]
#[ignore]
fn visual_languages() {
    let mut h = harness();
    h.state_mut().dialogs.ask_language = true;
    save(&mut h, "92_lang_chooser");
    h.state_mut().dialogs.ask_language = false;
    h.state_mut().new_default_document();
    for code in ["de-DE", "ru-RU", "ja-JP", "zh-CN", "zh-TW"] {
        h.state_mut().settings.language = code.into();
        h.step();
        assert_eq!(crate::i18n::lang().code(), code);
        save(&mut h, &format!("92_lang_{code}_document"));
        h.state_mut().open_options(crate::commands::OptionsPage::Global, None);
        save(&mut h, &format!("92_lang_{code}_options"));
        h.key_press(egui::Key::Escape);
        h.step();
    }
    h.state_mut().settings.language = "en-US".into();
    h.step();
    assert_eq!(crate::i18n::lang(), crate::i18n::Lang::EnUs);
}

/// The message shown at start when a new version is found, in English and in Brazilian Portuguese.
#[test]
#[ignore]
fn visual_update_prompt() {
    use crate::updater::{Release, Status};
    let mut h = harness();
    let r = Release {
        version: "0.2.0".into(),
        tag: "v0.2.0".into(),
        notes: String::new(),
        page: String::new(),
        asset_name: "AnyVector-0.2.0-setup.exe".into(),
        asset_url: String::new(),
        asset_size: 0,
        via_gh: false,
    };
    h.state_mut().update.status = Status::Available(r.clone());
    h.state_mut().update.prompt = true;
    save(&mut h, "91_update_prompt");
    h.state_mut().settings.language = "pt-BR".into();
    save(&mut h, "91_update_prompt_ptbr");
    // after OK, while the setup downloads
    h.state_mut().update.status = Status::Downloading(r, 0.4);
    h.state_mut().update.install_when_ready = true;
    save(&mut h, "91_update_prompt_ptbr_downloading");
    h.state_mut().settings.language = "en-US".into();
    h.state_mut().update.prompt = false;
    h.state_mut().update.install_when_ready = false;
    h.step();
    assert_eq!(crate::i18n::lang(), crate::i18n::Lang::EnUs);
}

/// Help > Updates with a new version found: the Install Now button must show with long release
/// notes, and while the setup downloads in the background (issue #33).
#[test]
#[ignore]
fn visual_updates_dialog() {
    use egui_kittest::kittest::Queryable;
    use crate::updater::{Release, Status};
    let mut h = harness();
    let notes = include_str!("../../../docs/RELEASE-NOTES-0.3.0.md").to_string();
    let r = Release {
        version: "9.9.9".into(),
        tag: "v9.9.9".into(),
        notes,
        page: String::new(),
        asset_name: "AnyVector-9.9.9-setup.exe".into(),
        asset_url: String::new(),
        asset_size: 0,
        via_gh: false,
    };
    h.state_mut().update.dialog = true;
    h.state_mut().update.status = Status::Available(r.clone());
    save(&mut h, "92_updates_available");
    // a Windows setup is never installed on Linux: the version is only announced (visual_update_announced)
    let offered = h.query_all_by_label("Install Now").next().is_some();
    assert_eq!(offered, cfg!(windows), "Install Now when a version is available (Windows only)");
    h.state_mut().update.status = Status::Downloading(r.clone(), 0.4);
    save(&mut h, "92_updates_downloading");
    assert!(h.query_all_by_label("Install Now").next().is_some(), "Install Now is missing while the setup downloads");
    // pressing it while the download runs: AnyVector installs as soon as it ends
    let at = h.get_by_label("Install Now").rect().center();
    click(&mut h, at);
    assert!(h.state().update.install_when_ready);
    save(&mut h, "92_updates_downloading_install");
    h.state_mut().update.install_when_ready = false;
    h.state_mut().settings.language = "pt-BR".into();
    save(&mut h, "92_updates_downloading_ptbr");
    h.state_mut().settings.language = "en-US".into();
    h.state_mut().update.status = Status::Ready(r, std::path::PathBuf::from("setup.exe"));
    save(&mut h, "92_updates_ready");
    assert!(h.query_all_by_label("Install Now").next().is_some(), "Install Now is missing when the setup is ready");
    h.state_mut().update.dialog = false;
    h.state_mut().update.status = Status::Idle;
    h.step();
}

/// A new version with nothing this system installs (always on Linux: AnyVector has no Linux
/// package to update yet): the message at start and Help > Updates announce it, with Open Release
/// Page instead of OK and Install Now, and nothing is downloaded.
#[test]
#[ignore]
fn visual_update_announced() {
    use egui_kittest::kittest::Queryable;
    use crate::updater::{Release, Status};
    let mut h = harness();
    let r = Release {
        version: "9.9.9".into(),
        tag: "v9.9.9".into(),
        notes: include_str!("../../../docs/RELEASE-NOTES-0.3.0.md").to_string(),
        page: "https://github.com/toim22/AnyVector/releases/tag/v9.9.9".into(),
        asset_name: String::new(),
        asset_url: String::new(),
        asset_size: 0,
        via_gh: false,
    };
    assert!(!r.installable());
    h.state_mut().update.status = Status::Available(r.clone());
    h.state_mut().update.prompt = true;
    save(&mut h, "92_updates_announced_prompt");
    assert!(h.query_all_by_label("Open Release Page").next().is_some(), "Open Release Page is missing in the message");
    assert!(h.query_all_by_label("OK").next().is_none(), "OK would install a setup this system doesn't have");
    h.state_mut().settings.language = "pt-BR".into();
    save(&mut h, "92_updates_announced_prompt_ptbr");
    h.state_mut().settings.language = "en-US".into();
    h.step();
    // Open Release Page takes the user to the page (no browser in tests) and closes the message
    click_label(&mut h, "Open Release Page");
    assert!(!h.state().update.prompt, "the message stays after Open Release Page");
    assert!(matches!(h.state().update.status, Status::Available(_)), "nothing is downloaded");
    h.state_mut().update.dialog = true;
    save(&mut h, "92_updates_announced_dialog");
    assert!(h.query_all_by_label("Open Release Page").next().is_some(), "Open Release Page is missing in Help > Updates");
    assert!(h.query_all_by_label("Install Now").next().is_none(), "Install Now would install a setup this system doesn't have");
    click_label(&mut h, "Open Release Page");
    assert!(matches!(h.state().update.status, Status::Available(_)), "nothing is downloaded");
    assert!(h.state().update.dialog, "Help > Updates stays open");
    h.state_mut().update.dialog = false;
    h.state_mut().update.status = Status::Idle;
    h.step();
    assert_eq!(crate::i18n::lang(), crate::i18n::Lang::EnUs);
}

fn right_click(h: &mut Harness<'_, AnyVectorApp>, p: egui::Pos2) {
    for _ in 0..30 {
        h.step();
    }
    move_to(h, p);
    h.step();
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
        h.step();
    }
}

/// Right-click menus: an object of each kind, the desktop, a ruler and a page tab.
#[test]
#[ignore]
fn visual_context_menus() {
    use egui_kittest::kittest::Queryable;
    use std::sync::Arc;
    use vector_core::document::ImageData;
    use vector_core::object::{ConnectorEnd, ConnectorKind, ConnectorObj, DimensionKind, DimensionObj, ImageObj, TableObj};
    use vector_core::text::{CharFormat, TextObj};
    let mut h = harness();
    h.state_mut().new_default_document();
    let (rect, ellipse, poly) = {
        let d = h.state_mut().doc_mut().unwrap();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::rgb(0x1e, 0x90, 0xff));
        let mut objs = vec![];
        let mut add = |d: &mut crate::docview::DocView, kind: ObjKind, x: f64, y: f64| {
            let o = Object::new(d.doc.new_id(), kind, Affine::translate((x, y)), st.clone());
            let id = o.id;
            objs.push(o);
            id
        };
        let rect = add(d, ObjKind::Rect(RectShape { w: 120.0, h: 80.0, radii: [0.0; 4], corner: CornerType::Round }), 40.0, 40.0);
        let ellipse = add(d, ObjKind::Ellipse(EllipseShape { w: 80.0, h: 80.0, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false }), 200.0, 40.0);
        let poly = add(d, ObjKind::Polygon(PolygonShape { w: 80.0, h: 80.0, sides: 5, kind: PolygonKind::Polygon, sharpness: 0.0, nodes: Default::default() }), 330.0, 40.0);
        let mut f = CharFormat::default();
        f.size = 48.0;
        add(d, ObjKind::Text(TextObj::artistic("Text", f.clone())), 40.0, 220.0);
        f.size = 24.0;
        add(d, ObjKind::Text(TextObj::paragraph("Paragraph text", 200.0, 60.0, f)), 200.0, 170.0);
        add(d, ObjKind::Table(TableObj::new(2, 2, 120.0, 60.0)), 440.0, 170.0);
        let image = d.doc.add_image(ImageData { width: 2, height: 2, rgba: Arc::new([0xff, 0xd0, 0x20, 0xff].repeat(4)), encoded: Arc::new(vec![]), format: "png".into(), name: "test.png".into() });
        add(d, ObjKind::Image(ImageObj { image, w: 100.0, h: 100.0, crop: [0.0, 0.0, 1.0, 1.0], adjust: Default::default() }), 40.0, 300.0);
        let small = |d: &mut crate::docview::DocView, x: f64, y: f64, w: f64| Object::new(d.doc.new_id(), ObjKind::Rect(RectShape { w, h: w, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((x, y)), st.clone());
        let g = vec![small(d, 200.0, 300.0, 50.0), small(d, 260.0, 330.0, 50.0)];
        add(d, ObjKind::Group(g), 0.0, 0.0);
        let frame = small(d, 360.0, 300.0, 100.0);
        let inside = Object::new(d.doc.new_id(), ObjKind::Ellipse(EllipseShape { w: 60.0, h: 60.0, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false }), Affine::translate((380.0, 320.0)), Style::default());
        add(d, ObjKind::PowerClip { frame: Box::new(frame), contents: vec![inside] }, 0.0, 0.0);
        let dim = DimensionObj {
            kind: DimensionKind::Parallel,
            points: vec![Point::new(40.0, 500.0), Point::new(200.0, 500.0), Point::new(120.0, 500.0)],
            text: String::new(),
            precision: 2,
            prefix: String::new(),
            suffix: String::new(),
            show_units: true,
            font_size: 10.0,
            unit: vector_core::units::Unit::Inches, ..Default::default()
        };
        add(d, ObjKind::Dimension(dim), 0.0, 0.0);
        let end = |x: f64, y: f64| ConnectorEnd { point: Point::new(x, y), attached: None, dir: None };
        add(d, ObjKind::Connector(ConnectorObj { kind: ConnectorKind::Straight, start: end(260.0, 500.0), end: end(400.0, 580.0), corner_radius: 0.0 }), 0.0, 0.0);
        d.add_objects("test", objs);
        d.selection = vec![poly];
        (rect, ellipse, poly)
    };
    let _ = poly;
    // the polygon becomes a curve
    h.state_mut().run(Cmd::ConvertToCurves);
    for _ in 0..5 {
        h.step();
    }
    let to_screen = |h: &Harness<'_, AnyVectorApp>, x: f64, y: f64| {
        let d = h.state().doc().unwrap();
        let p = d.to_screen(kurbo::Point::new(x, y));
        egui::pos2(p.x as f32, p.y as f32)
    };
    let cases: [(&str, f64, f64, Vec<u64>); 13] = [
        ("01_retangulo", 100.0, 80.0, vec![]),
        ("02_elipse", 240.0, 80.0, vec![]),
        ("03_curva", 370.0, 80.0, vec![]),
        ("04_texto-artistico", 75.0, 205.0, vec![]),
        ("05_texto-paragrafo", 230.0, 180.0, vec![]),
        ("06_bitmap", 90.0, 350.0, vec![]),
        ("07_grupo", 225.0, 325.0, vec![]),
        ("08_selecao-multipla", 100.0, 80.0, vec![rect, ellipse]),
        ("09_tabela", 470.0, 185.0, vec![]),
        ("11_powerclip", 410.0, 350.0, vec![]),
        ("12_linha-de-cota", 120.0, 500.0, vec![]),
        ("13_conector", 330.0, 540.0, vec![]),
        ("19_area-de-trabalho", -150.0, 300.0, vec![]),
    ];
    for (name, x, y, sel) in cases {
        h.state_mut().doc_mut().unwrap().selection = sel;
        let p = to_screen(&h, x, y);
        right_click(&mut h, p);
        save(&mut h, &format!("30_ctx_{name}"));
        close_menu(&mut h);
    }
    // the Shape tool on a node of the curve, and the Table tool on a cell
    let curve = h.state().doc().unwrap().doc.pages[0].layers[0].objects.iter().find(|o| matches!(o.kind, ObjKind::Path(_))).map(|o| o.id).unwrap();
    let table = h.state().doc().unwrap().doc.pages[0].layers[0].objects.iter().find(|o| matches!(o.kind, ObjKind::Table(_))).map(|o| o.id).unwrap();
    h.state_mut().set_tool(crate::tools::ToolId::Shape);
    {
        let d = h.state_mut().doc_mut().unwrap();
        d.selection = vec![curve];
        d.node_sel = vec![(curve, 0, 1)];
    }
    let p = to_screen(&h, 370.0, 80.0);
    right_click(&mut h, p);
    save(&mut h, "30_ctx_17_nos-da-curva");
    close_menu(&mut h);
    h.state_mut().set_tool(crate::tools::ToolId::Table);
    h.state_mut().ts.table_cell = Some((0, 0));
    h.state_mut().doc_mut().unwrap().selection = vec![table];
    let p = to_screen(&h, 470.0, 185.0);
    right_click(&mut h, p);
    save(&mut h, "30_ctx_15_tabela-celulas");
    close_menu(&mut h);
    h.state_mut().ts.table_cell = None;
    h.state_mut().set_tool(crate::tools::ToolId::Pick);
    // the rectangle becomes a symbol instance
    h.state_mut().doc_mut().unwrap().selection = vec![rect];
    h.state_mut().run(Cmd::NewSymbol);
    for _ in 0..3 {
        h.step();
    }
    h.state_mut().doc_mut().unwrap().selection.clear();
    let p = to_screen(&h, 100.0, 80.0);
    right_click(&mut h, p);
    save(&mut h, "30_ctx_10_simbolo");
    close_menu(&mut h);
    // the frame around the drawing: a ruler, the page tab and the command bars (with their
    // Customize submenu, which names the bar or item that was clicked)
    h.state_mut().doc_mut().unwrap().selection.clear();
    let frame = [
        ("20_regua", egui::pos2(300.0, 143.0), false),
        ("23_aba-da-pagina", egui::pos2(232.0, 985.0), false),
        ("25_barra-de-ferramentas", egui::pos2(1200.0, 43.0), true),
        ("26_barra-de-status", egui::pos2(1150.0, 1037.0), true),
        ("27_caixa-de-ferramentas", egui::pos2(16.0, 330.0), true),
    ];
    for (name, p, customize) in frame {
        right_click(&mut h, p);
        save(&mut h, &format!("30_ctx_{name}"));
        if customize {
            h.get_by_label("Customize").click_accesskit();
            save(&mut h, &format!("31_ctx_{name}_customize"));
        }
        close_menu(&mut h);
        close_menu(&mut h);
    }
}

/// The standard toolbar, and every optional toolbar of Window ▸ Toolbars turned on.
#[test]
#[ignore]
fn visual_toolbars() {
    let mut h = harness();
    h.state_mut().new_default_document();
    {
        let d = h.state_mut().doc_mut().unwrap();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::rgb(0x1e, 0x90, 0xff));
        let r = Object::new(d.doc.new_id(), ObjKind::Rect(RectShape { w: 180.0, h: 120.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((72.0, 72.0)), st);
        d.add_objects("test", vec![r]);
    }
    save(&mut h, "40_toolbar_standard");
    let names: Vec<String> = crate::app::Toolbar::EXTRA.iter().map(|t| t.name().to_string()).collect();
    h.state_mut().settings.extra_toolbars = names;
    for _ in 0..5 {
        h.step();
    }
    save(&mut h, "41_toolbars_all");
}

/// Every icon the reference interface data uses, with its name: `target/visual/50_icon_sheet.png`.
#[test]
#[ignore]
fn visual_icon_sheet() {
    let mut names: Vec<&str> = vec![];
    for b in crate::ui::bars() {
        for it in b.items {
            if !it.icon.is_empty() && !names.contains(&it.icon) {
                names.push(it.icon);
            }
        }
    }
    let (cols, cw, ch) = (8usize, 190u32, 60u32);
    let (w, h) = (cols as u32 * cw, names.len().div_ceil(cols) as u32 * ch);
    let light = visual_theme().is_some();
    let (bg, fg) = if light { ("#f0f0f0", "#3c3c3c") } else { ("#262626", "#c4c4c4") };
    let mut svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}"><rect width="{w}" height="{h}" fill="{bg}"/>"##);
    for (i, n) in names.iter().enumerate() {
        let (x, y) = ((i % cols) as u32 * cw, (i / cols) as u32 * ch);
        let body = crate::icons::themed_body(n, light).unwrap_or_default().replace("#c4c4c4", fg);
        svg += &format!(
            r##"<g transform="translate({} {}) scale(2)" fill="none" stroke="{fg}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">{body}</g><text x="{}" y="{}" font-family="Segoe UI" font-size="12" fill="{fg}">{n}</text>"##,
            x + 6,
            y + 6,
            x + 62,
            y + 34
        );
    }
    svg += "</svg>";
    let img = crate::icons::rasterize_svg(&svg, w, h).expect("sheet");
    image::RgbaImage::from_raw(w, h, img.as_raw().to_vec()).unwrap().save(out_dir().join("50_icon_sheet.png")).unwrap();
}

/// The toolbox with a document, its flyouts and Quick Customize.
#[test]
#[ignore]
fn visual_toolbox() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness();
    h.state_mut().new_default_document();
    save(&mut h, "42_toolbox");
    let click = |h: &mut Harness<'_, AnyVectorApp>, p: egui::Pos2| {
        move_to(h, p);
        h.step();
        press(h, p, true);
        h.step();
        press(h, p, false);
        h.step();
    };
    for (label, name) in [
        ("Pick tool", "01_pick"),
        ("Shape Tool (F10)", "02_shape"),
        ("Crop tool", "04_crop"),
        ("Zoom tool (Z)", "05_zoom"),
        ("Freehand tool (F5)", "07_freehand"),
        ("Painterly Brush tool (J)", "08_painterly"),
        ("Rectangle tool (F6)", "09_rectangle"),
        ("Ellipse tool (F7)", "10_ellipse"),
        ("Polygon tool (Y)", "11_polygon"),
        ("Text tool (F8)", "13_text"),
        ("Parallel Dimension tool", "15_dimension"),
        ("Connector tool", "16_connector"),
        ("Shadow tool", "18_shadow"),
        ("Color Eyedropper tool (E)", "21_eyedropper"),
        ("Interactive Fill tool (G)", "22_fill"),
    ] {
        let Some(r) = h.query_by_label(label).map(|n| n.rect()) else {
            eprintln!("no toolbox button {label:?}");
            continue;
        };
        click(&mut h, r.right_bottom() - egui::vec2(3.0, 3.0));
        save(&mut h, &format!("43_flyout_{name}"));
        close_menu(&mut h);
    }
    // the toolbox's own Quick Customize button (the property bar has one too)
    let r = h.query_all_by_label("Quick Customize").map(|n| n.rect()).min_by(|a, b| a.left().total_cmp(&b.left())).unwrap();
    click(&mut h, r.center());
    save(&mut h, "44_quick_customize");
}

/// The property bar for every selection of the Pick tool and for every tool, cropped to the bar
/// (`target/visual/pb/`), named like the reference captures they are compared with.
#[test]
#[ignore]
fn visual_property_bars() {
    use crate::tools::ToolId as T;
    use egui_kittest::kittest::Queryable;
    use std::sync::Arc;
    use vector_core::document::ImageData;
    use vector_core::object::{ConnectorEnd, ConnectorKind, ConnectorObj, DimensionKind, DimensionObj, ImageObj, TableObj};
    use vector_core::text::{CharFormat, TextObj};
    let dir = out_dir().join("pb");
    std::fs::create_dir_all(&dir).unwrap();
    *crate::cmdbar::MISSING.lock().unwrap() = Some(Default::default());
    let mut h = harness();
    h.state_mut().new_default_document();
    let mut ids = std::collections::HashMap::new();
    {
        let d = h.state_mut().doc_mut().unwrap();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::rgb(0x1e, 0x90, 0xff));
        st.outline.width = 0.5;
        let mut objs = vec![];
        let mut add = |d: &mut crate::docview::DocView, name: &'static str, kind: ObjKind, x: f64, y: f64| {
            let o = Object::new(d.doc.new_id(), kind, Affine::translate((x, y)), st.clone());
            ids.insert(name, o.id);
            objs.push(o);
        };
        let rect = |w: f64| ObjKind::Rect(RectShape { w, h: w * 2.0 / 3.0, radii: [0.0; 4], corner: CornerType::Round });
        add(d, "retangulo", rect(216.0), 40.0, 40.0);
        add(d, "elipse", ObjKind::Ellipse(EllipseShape { w: 80.0, h: 80.0, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false }), 300.0, 40.0);
        add(d, "curva", ObjKind::Polygon(PolygonShape { w: 80.0, h: 80.0, sides: 5, kind: PolygonKind::Polygon, sharpness: 0.0, nodes: Default::default() }), 420.0, 40.0);
        let mut f = CharFormat::default();
        f.size = 24.0;
        add(d, "texto-artistico", ObjKind::Text(TextObj::artistic("Text", f.clone())), 40.0, 220.0);
        add(d, "texto-paragrafo", ObjKind::Text(TextObj::paragraph("Paragraph text", 200.0, 60.0, f)), 200.0, 170.0);
        add(d, "tabela", ObjKind::Table(TableObj::new(3, 3, 360.0, 216.0)), 440.0, 170.0);
        let image = d.doc.add_image(ImageData { width: 2, height: 2, rgba: Arc::new([0xff, 0xd0, 0x20, 0xff].repeat(4)), encoded: Arc::new(vec![]), format: "png".into(), name: "test.png".into() });
        add(d, "bitmap", ObjKind::Image(ImageObj { image, w: 100.0, h: 100.0, crop: [0.0, 0.0, 1.0, 1.0], adjust: Default::default() }), 40.0, 300.0);
        let small = |d: &mut crate::docview::DocView, x: f64, y: f64, w: f64| Object::new(d.doc.new_id(), rect(w), Affine::translate((x, y)), Style::default());
        let g = vec![small(d, 200.0, 300.0, 50.0), small(d, 260.0, 330.0, 50.0)];
        add(d, "grupo", ObjKind::Group(g), 0.0, 0.0);
        let frame = Object::new(d.doc.new_id(), ObjKind::Ellipse(EllipseShape { w: 100.0, h: 100.0, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false }), Affine::translate((360.0, 300.0)), Style::default());
        let inside = small(d, 380.0, 320.0, 60.0);
        add(d, "powerclip", ObjKind::PowerClip { frame: Box::new(frame), contents: vec![inside] }, 0.0, 0.0);
        let dim = DimensionObj {
            kind: DimensionKind::Parallel,
            points: vec![Point::new(40.0, 500.0), Point::new(200.0, 500.0), Point::new(120.0, 500.0)],
            text: String::new(),
            precision: 2,
            prefix: String::new(),
            suffix: String::new(),
            show_units: true,
            font_size: 10.0,
            unit: vector_core::units::Unit::Inches, ..Default::default()
        };
        add(d, "linha-de-cota", ObjKind::Dimension(dim), 0.0, 0.0);
        let end = |x: f64, y: f64| ConnectorEnd { point: Point::new(x, y), attached: None, dir: None };
        add(d, "conector", ObjKind::Connector(ConnectorObj { kind: ConnectorKind::Straight, start: end(260.0, 500.0), end: end(400.0, 580.0), corner_radius: 0.0 }), 0.0, 0.0);
        add(d, "com-efeitos", rect(80.0), 600.0, 420.0);
        add(d, "simbolo", rect(60.0), 700.0, 40.0);
        d.add_objects("test", objs);
        let fx = ids["com-efeitos"];
        if let Some(o) = d.doc.find_mut(fx) {
            o.effects.drop_shadow = Some(crate::tools::default_shadow(false));
        }
    }
    // the pentagon becomes a curve, the small rectangle a symbol
    h.state_mut().doc_mut().unwrap().selection = vec![ids["curva"]];
    h.state_mut().run(Cmd::ConvertToCurves);
    for _ in 0..3 {
        h.step();
    }
    h.state_mut().doc_mut().unwrap().selection = vec![ids["simbolo"]];
    h.state_mut().run(Cmd::NewSymbol);
    for _ in 0..3 {
        h.step();
    }
    let symbol = h.state().doc().unwrap().selection.first().copied().unwrap_or(ids["simbolo"]);
    ids.insert("simbolo", symbol);
    // where the property bar is: the row of its Quick Customize button (the one further right)
    h.state_mut().doc_mut().unwrap().selection.clear();
    h.step();
    let plus = h.query_all_by_label("Quick Customize").map(|n| n.rect()).max_by(|a, b| a.left().total_cmp(&b.left())).unwrap();
    let y0 = (plus.center().y - 22.0).max(0.0) as u32;
    let shot = |h: &mut Harness<'_, AnyVectorApp>, name: &str| {
        for _ in 0..4 {
            h.step();
        }
        let img = h.render().expect("render");
        let bar = image::imageops::crop_imm(&img, 0, y0, img.width(), 44).to_image();
        bar.save(dir.join(format!("{name}.png"))).unwrap();
    };
    let select = |h: &mut Harness<'_, AnyVectorApp>, sel: Vec<vector_core::ObjId>| {
        let a = h.state_mut();
        a.ts.table_cell = None;
        a.doc_mut().unwrap().selection = sel;
    };
    // the Pick tool, by selection
    let contexts = [
        "doc-vazio", "retangulo", "elipse", "curva", "texto-artistico", "texto-paragrafo", "bitmap", "grupo", "selecao-multipla", "tabela", "tabela-celulas", "simbolo", "powerclip", "linha-de-cota", "conector", "varias-paginas", "rollover", "com-efeitos",
    ];
    for (n, ctx) in contexts.iter().enumerate() {
        h.state_mut().set_tool(T::Pick);
        let sel = match *ctx {
            "doc-vazio" | "varias-paginas" => vec![],
            "selecao-multipla" => vec![ids["retangulo"], ids["elipse"]],
            "tabela-celulas" => vec![ids["tabela"]],
            "rollover" => vec![ids["retangulo"]],
            c => vec![ids[c]],
        };
        select(&mut h, sel);
        if *ctx == "tabela-celulas" {
            h.state_mut().set_tool(T::Table);
            h.state_mut().ts.table_cell = Some((0, 0));
        }
        shot(&mut h, &format!("pick {:02} {ctx}", n + 1));
    }
    // every tool, with the selection of its capture
    let tools: &[(&str, T, &str)] = &[
        ("001 Pick", T::Pick, "retangulo"),
        ("002 Freehand Pick", T::FreehandPick, "retangulo"),
        ("003 Free Transform", T::FreeTransform, "retangulo"),
        ("004 Shape", T::Shape, "curva"),
        ("005 Smooth", T::Smooth, "curva"),
        ("006 Smear", T::Smear, "curva"),
        ("007 Twirl", T::Twirl, "curva"),
        ("008 Attract and Repel", T::AttractRepel, "curva"),
        ("009 Smudge", T::Smudge, "curva"),
        ("010 Roughen", T::Roughen, "curva"),
        ("011 Crop", T::Crop, "bitmap"),
        ("012 Knife", T::Knife, "retangulo"),
        ("013 Virtual Segment Delete", T::VirtualDelete, "retangulo"),
        ("014 Eraser", T::Eraser, "retangulo"),
        ("015 Zoom", T::Zoom, "retangulo"),
        ("016 Pan", T::Pan, "retangulo"),
        ("017 Freehand", T::Freehand, "retangulo"),
        ("018 2-Point Line", T::TwoPointLine, "retangulo"),
        ("019 Bezier", T::Bezier, "retangulo"),
        ("020 Pen", T::Pen, "retangulo"),
        ("021 B-Spline", T::BSpline, "retangulo"),
        ("022 Polyline", T::Polyline, "retangulo"),
        ("023 3-Point Curve", T::ThreePointCurve, "retangulo"),
        ("024 Painterly Brush", T::PainterlyBrush, "retangulo"),
        ("025 Artistic Media", T::ArtisticMedia, "retangulo"),
        ("026 LiveSketch", T::LiveSketch, "retangulo"),
        ("027 Smart Drawing", T::SmartDrawing, "retangulo"),
        ("028 Rectangle", T::Rectangle, "retangulo"),
        ("029 3-Point Rectangle", T::Rect3, "retangulo"),
        ("030 Ellipse", T::Ellipse, "retangulo"),
        ("031 3-Point Ellipse", T::Ellipse3, "retangulo"),
        ("032 Polygon", T::Polygon, "retangulo"),
        ("033 Star", T::Star, "retangulo"),
        ("034 Spiral", T::Spiral, "retangulo"),
        ("035 Common Shapes", T::CommonShapes, "retangulo"),
        ("037 Graph Paper", T::GraphPaper, "retangulo"),
        ("038 Text", T::Text, "texto-artistico"),
        ("039 Table", T::Table, "tabela"),
        ("040 Parallel Dimension", T::ParallelDim, "linha-de-cota"),
        ("041 Horizontal or Vertical Dimension", T::HvDim, "linha-de-cota"),
        ("042 Angular Dimension", T::AngularDim, "linha-de-cota"),
        ("043 Segment Dimension", T::SegmentDim, "linha-de-cota"),
        ("044 2-Leg Callout", T::Callout, "linha-de-cota"),
        ("045 Connector", T::StraightConnector, "conector"),
        ("046 Anchor Editing", T::EditAnchor, "conector"),
        ("047 Shadow", T::DropShadow, "retangulo"),
        ("048 Contour", T::Contour, "retangulo"),
        ("049 Blend", T::Blend, "retangulo"),
        ("050 Distort", T::Distort, "retangulo"),
        ("051 Envelope", T::Envelope, "retangulo"),
        ("052 Extrude", T::Extrude, "retangulo"),
        ("053 Block Shadow", T::BlockShadow, "retangulo"),
        ("054 Transparency", T::Transparency, "retangulo"),
        ("055 Color Eyedropper", T::ColorEyedropper, "retangulo"),
        ("056 Attributes Eyedropper", T::AttributesEyedropper, "retangulo"),
        ("057 Interactive Fill", T::InteractiveFill, "retangulo"),
        ("058 Smart Fill", T::SmartFill, "retangulo"),
        ("059 Mesh Fill", T::MeshFill, "retangulo"),
    ];
    for (name, t, ctx) in tools {
        select(&mut h, vec![ids[ctx]]);
        h.state_mut().set_tool(*t);
        select(&mut h, vec![ids[ctx]]);
        shot(&mut h, name);
    }
    // what the bars still show disabled: target/propbar-missing.txt
    let missing = crate::cmdbar::MISSING.lock().unwrap().take().unwrap_or_default();
    let text: String = missing.iter().map(|(b, i, k)| format!("{b}	{i}	{k}
")).collect();
    std::fs::write(out_dir().join("../propbar-missing.txt"), text).unwrap();
}

/// View menu toggles: multipage view, baseline grid, printable area, proof colors, simulated
/// overprints, alignment guides and Preview Selected Only.
#[test]
#[ignore]
fn visual_view_toggles() {
    let mut h = harness();
    h.state_mut().new_default_document();
    {
        let app = h.state_mut();
        let d = app.doc_mut().unwrap();
        let (w, hh) = (d.page_ref().width, d.page_ref().height);
        d.modify("pages", |doc, _| {
            doc.add_page(1, w, hh);
        });
        let mut objs = vec![];
        for (i, (x, c)) in [(72.0, Color::rgb(0x00, 0xae, 0xef)), (160.0, Color::rgb(0xff, 0xf2, 0x00))].into_iter().enumerate() {
            let id = d.doc.new_id();
            let mut st = Style::default();
            st.fill = Fill::Solid(c);
            st.overprint_fill = i == 1;
            objs.push(Object::new(id, ObjKind::Rect(RectShape { w: 200.0, h: 150.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((x, 100.0)), st));
        }
        d.add_objects("test", objs);
    }
    for c in [Cmd::ToggleMultipageView, Cmd::ToggleBaselineGrid, Cmd::TogglePrintableArea, Cmd::ToggleSimulateOverprints, Cmd::ToggleProofColors] {
        h.state_mut().run(c);
    }
    h.state_mut().run(Cmd::ZoomToAll);
    save(&mut h, "40_view_toggles");
    h.state_mut().run(Cmd::PreviewSelectedOnly);
    save(&mut h, "41_preview_selected_only");
}

/// File and Edit dialogs: Publish to PDF (and its settings), Export, bitmap export, Document
/// Properties, Copy Properties and Paste Special.
#[test]
#[ignore]
fn visual_file_edit_dialogs() {
    use crate::dialogs::{clipboard, export};
    let mut h = harness();
    h.state_mut().new_default_document();
    h.state_mut().run(Cmd::PublishPdf);
    save(&mut h, "50_publish_pdf");
    if let Some(st) = h.state_mut().dialogs.pdf.as_mut() {
        st.settings_open = true;
    }
    save(&mut h, "51_pdf_settings");
    h.state_mut().dialogs.pdf = None;
    let st = export::ExportState::new(h.state());
    h.state_mut().dialogs.export = Some(st);
    save(&mut h, "52_export");
    h.state_mut().dialogs.export = None;
    let st = export::BitmapState::new(h.state(), export::BitmapMode::Export, None, "png", vec![0], false);
    h.state_mut().dialogs.bitmap = Some(st);
    save(&mut h, "53_export_bitmap");
    h.state_mut().dialogs.bitmap = None;
    h.state_mut().run(Cmd::DocumentProperties);
    save(&mut h, "54_document_properties");
    h.state_mut().dialogs.doc_props = false;
    h.state_mut().dialogs.copy_props = Some(clipboard::CopyPropsState { fill: true, ..Default::default() });
    save(&mut h, "55_copy_properties");
    h.state_mut().dialogs.copy_props = None;
    h.state_mut().dialogs.paste_special = Some(clipboard::PasteSpecialState { source: "Unknown Source".into(), kinds: vec![clipboard::PasteKind::Text, clipboard::PasteKind::Bitmap], chosen: 0 });
    save(&mut h, "56_paste_special");
}

/// Layout dialogs: Insert, Duplicate, Rename, Delete and Go to Page, Page Number Settings and
/// Border and Grommet.
#[test]
#[ignore]
fn visual_layout_dialogs() {
    let mut h = harness();
    h.state_mut().new_default_document();
    h.state_mut().run(Cmd::InsertPage);
    save(&mut h, "60_insert_page");
    h.state_mut().dialogs.insert_page = None;
    h.state_mut().run(Cmd::DuplicatePage);
    save(&mut h, "61_duplicate_page");
    h.state_mut().dialogs.duplicate_page = None;
    h.state_mut().run(Cmd::RenamePage);
    save(&mut h, "62_rename_page");
    h.state_mut().dialogs.rename_page = None;
    h.state_mut().run(Cmd::InsertPageAfter);
    h.state_mut().run(Cmd::DeletePage);
    save(&mut h, "63_delete_page");
    h.state_mut().dialogs.delete_page = None;
    h.state_mut().run(Cmd::GoToPage);
    save(&mut h, "64_go_to_page");
    h.state_mut().dialogs.goto_page = None;
    h.state_mut().run(Cmd::PageNumberSettings);
    save(&mut h, "65_page_number_settings");
    h.state_mut().dialogs.page_numbers = None;
    h.state_mut().run(Cmd::BorderGrommet);
    save(&mut h, "66_border_grommet");
    if let Some(st) = h.state_mut().dialogs.border_grommet.as_mut() {
        st.opts.border = true;
        st.opts.grommets = true;
        st.opts.placement = [true, true, true, false, false];
    }
    save(&mut h, "67_border_grommet_on");
    h.state_mut().dialogs.border_grommet = None;
    h.state_mut().run(Cmd::InsertPageNumber(1));
    save(&mut h, "68_page_numbers");
}

/// Table dialogs and merged cells: Create New Table, a block of cells, Merge Cells, Insert
/// Rows, Split Cells, Convert Table to Text and Convert Text to Table.
#[test]
#[ignore]
fn visual_table_dialogs() {
    use crate::tools::ToolId;
    let mut h = harness();
    h.state_mut().new_default_document();
    h.state_mut().run(Cmd::CreateTable);
    save(&mut h, "70_create_table");
    // create it through the dialog's OK path
    {
        let a = h.state_mut();
        let st = a.dialogs.table.take().unwrap();
        crate::dialogs::table::apply_for_test(a, &st);
    }
    h.state_mut().set_tool(ToolId::Table);
    h.state_mut().ts.table_cell = Some((0, 0));
    h.state_mut().ts.table_cell_to = Some((1, 1));
    save(&mut h, "71_table_cells");
    h.state_mut().run(Cmd::TableMerge);
    save(&mut h, "72_table_merged");
    h.state_mut().run(Cmd::TableInsertMany(false));
    save(&mut h, "73_insert_rows");
    h.state_mut().dialogs.table = None;
    h.state_mut().run(Cmd::TableInsertMany(true));
    save(&mut h, "74_insert_columns");
    h.state_mut().dialogs.table = None;
    h.state_mut().run(Cmd::TableSplit(false));
    save(&mut h, "75_split_cells");
    if let Some(st) = h.state_mut().dialogs.table.take() {
        crate::dialogs::table::apply_for_test(h.state_mut(), &st);
    }
    save(&mut h, "76_table_split");
    h.state_mut().run(Cmd::TableToText);
    save(&mut h, "77_table_to_text");
    if let Some(st) = h.state_mut().dialogs.table.take() {
        crate::dialogs::table::apply_for_test(h.state_mut(), &st);
    }
    h.state_mut().run(Cmd::TextToTable);
    save(&mut h, "78_text_to_table");
}

/// Object menu: Create Arrowhead, Create Pattern, Barcode Wizard, QR Code, Edit Rollover and
/// Edit Master.
#[test]
#[ignore]
fn visual_object_dialogs() {
    use kurbo::{Affine, BezPath, Point, Rect, Shape};
    use vector_core::style::ArrowHead;
    use vector_core::{Color, Fill, ObjKind, Object, Style};
    let mut h = harness();
    h.state_mut().new_default_document();
    // a triangle pointing right, to become an arrowhead
    {
        let d = h.state_mut().doc_mut().unwrap();
        let mut bp = BezPath::new();
        bp.move_to((200.0, 200.0));
        bp.line_to((260.0, 225.0));
        bp.line_to((200.0, 250.0));
        bp.close_path();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::BLACK);
        let o = Object::new(d.doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(&bp)), Affine::IDENTITY, st);
        d.add_objects("Triangle", vec![o]);
    }
    h.state_mut().run(Cmd::CreateArrowhead);
    save(&mut h, "80_create_arrowhead");
    {
        let a = h.state_mut();
        let st = a.dialogs.arrowhead.take().unwrap();
        let ca = st.arrow();
        a.settings.custom_arrowheads.push(ca.clone());
        let d = a.doc_mut().unwrap();
        d.doc.arrowheads.push(ca.clone());
        let mut bp = BezPath::new();
        bp.move_to((150.0, 400.0));
        bp.line_to((450.0, 400.0));
        let mut stl = Style::default();
        stl.outline.width = 2.0;
        stl.outline.end_arrow = Some(ArrowHead::Custom(ca.id));
        let o = Object::new(d.doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(&bp)), Affine::IDENTITY, stl);
        d.add_objects("Line", vec![o]);
    }
    save(&mut h, "81_custom_arrowhead");
    h.state_mut().run(Cmd::CreatePattern);
    save(&mut h, "82_create_pattern");
    h.state_mut().dialogs.pattern = None;
    {
        let d = h.state_mut().doc_mut().unwrap();
        let page = d.page;
        let name = crate::dialogs::object::make_pattern(&mut d.doc, page, Rect::new(190.0, 190.0, 270.0, 260.0), 0, 1).unwrap();
        let fill = d.doc.patterns.iter().find(|p| p.name == name).unwrap().fill.clone();
        let mut st = Style::default();
        st.fill = Fill::Pattern(vector_core::style::PatternFill { tile: 30.0, ..fill });
        let r = Rect::new(300.0, 450.0, 520.0, 620.0).to_path(0.1);
        let o = Object::new(d.doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(&r)), Affine::IDENTITY, st);
        d.add_objects("Patterned", vec![o]);
    }
    save(&mut h, "83_pattern_fill");
    h.state_mut().run(Cmd::InsertBarcode);
    h.step();
    if let Some(b) = h.state_mut().dialogs.barcode.as_mut() {
        b.kind = 2;
        b.data = "400638133393".into();
    }
    save(&mut h, "84_barcode_wizard");
    {
        let a = h.state_mut();
        let st = a.dialogs.barcode.take().unwrap();
        let d = a.doc_mut().unwrap();
        let o = crate::dialogs::object::barcode_object(&mut d.doc, &st, Point::new(450.0, 250.0)).unwrap();
        d.add_objects("Insert Barcode", vec![o]);
    }
    h.state_mut().run(Cmd::InsertQrCode);
    save(&mut h, "85_qr_code");
    // a rollover of the QR code, in Edit Rollover
    h.state_mut().run(Cmd::RolloverCreate);
    h.state_mut().run(Cmd::RolloverEdit);
    save(&mut h, "86_edit_rollover");
    h.state_mut().run(Cmd::RolloverFinish);
    h.step();
    // a symbol of the barcode, in Edit Master
    {
        let a = h.state_mut();
        let d = a.doc_mut().unwrap();
        let id = d.doc.pages[0].layers.iter().flat_map(|l| l.objects.iter()).find(|o| matches!(o.special, Some(vector_core::codes::Special::Barcode { .. }))).map(|o| o.id).unwrap();
        d.selection = vec![id];
    }
    h.state_mut().run(Cmd::NewSymbol);
    h.step();
    h.state_mut().run(Cmd::SymbolEditMaster);
    save(&mut h, "87_edit_master");
    h.state_mut().run(Cmd::SymbolFinish);
    save(&mut h, "88_symbol_finished");
}

/// Text menu: Tabs, Leader Settings, Columns, Bullets and Numbering, Drop Cap, Statistics, Custom Optional
/// Hyphens, Writing Tools, Text Language, Change Case, Text Encoding, Edit Text, and text frames on the
/// page (non-printing characters, overflow, linked frames, spelling).
#[test]
#[ignore]
fn visual_text_dialogs() {
    use vector_core::text::{CharFormat, TextObj};
    let mut h = harness();
    h.state_mut().new_default_document();
    let (head, follower) = {
        let d = h.state_mut().doc_mut().unwrap();
        let f = CharFormat { size: 14.0, ..Default::default() };
        let body = "Teh quick brown fox jumps over the lazy dog. This is is a paragraph of text\twith a tab and an em\u{2003}space.\nA second paragraph flows into the next frame when the first one is full of words. ".repeat(3);
        let (a, b, c) = (d.doc.new_id(), d.doc.new_id(), d.doc.new_id());
        let mut t1 = TextObj::paragraph(&body, 220.0, 150.0, f.clone());
        t1.link_next = Some(b);
        let mut t2 = TextObj::paragraph("", 220.0, 180.0, f.clone());
        t2.link_prev = Some(a);
        let t3 = TextObj::paragraph(&"Overflowing text that does not fit in its frame. ".repeat(6), 180.0, 60.0, f.clone());
        let objs = vec![
            Object::new(a, ObjKind::Text(t1), Affine::translate((60.0, 80.0)), Style::text_default()),
            Object::new(b, ObjKind::Text(t2), Affine::translate((320.0, 80.0)), Style::text_default()),
            Object::new(c, ObjKind::Text(t3), Affine::translate((60.0, 400.0)), Style::text_default()),
        ];
        d.add_objects("Text", objs);
        d.selection = vec![a];
        (a, b)
    };
    h.state_mut().settings.show_nonprinting = true;
    h.state_mut().settings.spell_check = true;
    save(&mut h, "90_text_frames");
    h.state_mut().settings.show_nonprinting = false;
    h.state_mut().run(Cmd::TextTabs);
    save(&mut h, "90_tab_settings");
    if let Some(st) = h.state_mut().dialogs.tabs.as_mut() {
        st.leader_dlg = Some(st.leader);
    }
    save(&mut h, "90_leader_settings");
    h.state_mut().dialogs.tabs = None;
    h.state_mut().run(Cmd::TextColumns);
    save(&mut h, "91_column_settings");
    h.state_mut().dialogs.columns = None;
    h.state_mut().run(Cmd::TextBullets);
    h.step();
    if let Some(st) = h.state_mut().dialogs.bullets.as_mut() {
        st.on = true;
    }
    save(&mut h, "92_bullets_and_numbering");
    h.state_mut().dialogs.bullets = None;
    h.state_mut().run(Cmd::TextDropCap);
    save(&mut h, "93_drop_cap");
    h.state_mut().dialogs.drop_cap = None;
    h.state_mut().run(Cmd::TextStatistics);
    save(&mut h, "94_statistics");
    h.state_mut().dialogs.text_stats = None;
    h.state_mut().run(Cmd::CustomHyphens);
    save(&mut h, "95_custom_optional_hyphens");
    h.state_mut().dialogs.hyphens = None;
    h.state_mut().run(Cmd::WritingTools(0));
    save(&mut h, "96_spell_checker");
    h.state_mut().dialogs.writing = None;
    h.state_mut().run(Cmd::WritingTools(1));
    save(&mut h, "96_grammatik");
    h.state_mut().dialogs.writing = None;
    h.state_mut().run(Cmd::WritingTools(2));
    h.step();
    if let Some(st) = h.state_mut().dialogs.writing.as_mut() {
        st.word = "happy".into();
        st.results = crate::spell::synonyms("happy");
        st.looked = Some("happy".into());
    }
    save(&mut h, "96_thesaurus");
    h.state_mut().dialogs.writing = None;
    h.state_mut().run(Cmd::TextLanguage);
    save(&mut h, "97_text_language");
    h.state_mut().dialogs.text_lang = None;
    h.state_mut().run(Cmd::ChangeCaseDialog);
    save(&mut h, "98_change_case");
    h.state_mut().dialogs.change_case = None;
    h.state_mut().run(Cmd::EncodeText);
    save(&mut h, "99_text_encoding");
    h.state_mut().dialogs.encode = None;
    h.state_mut().run(Cmd::EditText);
    save(&mut h, "99_edit_text");
    h.state_mut().dialogs.edit_text = None;
    // two columns, bullets and a drop cap on the linked frames; unlink them
    {
        let a = h.state_mut();
        crate::dialogs::text::modify_texts(a, "Columns", false, |t| {
            t.columns = 2;
            t.column_gap = 12.0;
        });
        crate::dialogs::text::modify_texts(a, "Drop Cap", true, |t| t.drop_cap = Some(Default::default()));
    }
    h.state_mut().settings.show_nonprinting = true;
    save(&mut h, "99_columns_drop_cap");
    {
        let a = h.state_mut();
        a.doc_mut().unwrap().selection = vec![head, follower];
    }
    h.state_mut().run(Cmd::UnlinkFrames);
    save(&mut h, "99_unlinked");
}

#[test]
#[ignore]
fn visual_symmetry_perspective() {
    use kurbo::Rect;
    use vector_core::codes::Special;
    let mut h = harness();
    h.state_mut().new_default_document();
    // a petal to the left of the symmetry center
    let petal = {
        let d = h.state_mut().doc_mut().unwrap();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::rgb(0xe0, 0x50, 0x70));
        let e = EllipseShape { w: 90.0, h: 40.0, kind: EllipseKind::default(), start: 0.0, end: 0.0, clockwise: false };
        let id = d.doc.new_id();
        d.add_objects("Petal", vec![Object::new(id, ObjKind::Ellipse(e), Affine::translate((200.0, 280.0)), st)]);
        d.selection = vec![id];
        id
    };
    h.state_mut().run(Cmd::SymmetryCreate);
    h.step();
    if let Some(se) = h.state_mut().doc_mut().unwrap().symmetry_edit.as_mut() {
        se.lines = 4;
        se.center = Point::new(306.0, 300.0);
    }
    save(&mut h, "s1_symmetry_edit");
    h.state_mut().run(Cmd::SymmetryFinish);
    h.step();
    {
        let d = h.state().doc().unwrap();
        let [g] = d.selection.as_slice() else { panic!("the symmetry is selected") };
        let o = d.doc.find(*g).unwrap();
        assert!(matches!(o.special, Some(Special::Symmetry { lines: 4, primaries: 1, .. })));
        let ObjKind::Group(kids) = &o.kind else { panic!() };
        assert_eq!(kids.len(), 8, "the petal and 7 copies");
        assert_eq!(kids[0].id, petal);
    }
    save(&mut h, "s2_symmetry");
    // draw in perspective: a square on the left plane
    h.state_mut().run(Cmd::Deselect);
    h.state_mut().run(Cmd::PerspectiveDraw);
    h.step();
    if let Some(pe) = h.state_mut().doc_mut().unwrap().persp_edit.as_mut() {
        pe.kind = 1;
        pe.plane = 2;
    }
    {
        let d = h.state_mut().doc_mut().unwrap();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::rgb(0x40, 0x90, 0xd0));
        let r = RectShape { w: 200.0, h: 200.0, radii: [0.0; 4], corner: CornerType::default() };
        let id = d.doc.new_id();
        d.add_objects("Rectangle", vec![Object::new(id, ObjKind::Rect(r), Affine::translate((150.0, 450.0)), st)]);
    }
    save(&mut h, "s3_draw_in_perspective");
    h.state_mut().run(Cmd::PerspectiveFinish);
    h.step();
    {
        let d = h.state().doc().unwrap();
        let [g] = d.selection.as_slice() else { panic!("the perspective group is selected") };
        let o = d.doc.find(*g).unwrap();
        let Some(Special::Perspective { flats, .. }) = &o.special else { panic!("a perspective group") };
        assert_eq!(flats.len(), 1);
        assert_eq!(flats[0].0, 2, "on the left plane");
        // the square is no longer square on its plane
        let b: Rect = d.doc.bounds_of(o);
        assert!((b.width() - 200.0).abs() > 1.0 || (b.height() - 200.0).abs() > 1.0, "{b:?}");
    }
    save(&mut h, "s4_perspective_group");
    h.state_mut().run(Cmd::InsertObject);
    save(&mut h, "s5_insert_object");
}

/// Every docker, one at a time, with a rectangle selected (as in the reference captures):
/// `target/visual/dk_NNN.png`, numbered like `04-dockers/retangulo`.
#[test]
#[ignore]
fn visual_dockers() {
    use crate::dockers::{DockerId, ALL};
    let mut h = harness();
    h.state_mut().new_default_document();
    h.state_mut().dockers.set_width = Some(320.0);
    {
        let d = h.state_mut().doc_mut().unwrap();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::rgb(70, 130, 200));
        let r = RectShape { w: 216.0, h: 144.0, radii: [0.0; 4], corner: CornerType::default() };
        let id = d.doc.new_id();
        d.add_objects("Rectangle", vec![Object::new(id, ObjKind::Rect(r), Affine::translate((144.0, 288.0)), st)]);
        d.selection = vec![id];
    }
    let only: Option<String> = std::env::var("ANYVECTOR_DOCKER").ok();
    for (i, d) in ALL.iter().copied().enumerate() {
        if d == DockerId::Learn || only.as_ref().is_some_and(|o| format!("{d:?}") != *o) {
            continue;
        }
        {
            let app = h.state_mut();
            app.dockers.open.clear();
            app.dockers.show(d);
            app.dockers.collapsed = false;
            app.dockers.set_width = Some(320.0);
        }
        save(&mut h, &format!("dk_{:03}", i + 1));
        // the inner states: tabs and modes
        let states: &[fn(&mut AnyVectorApp)] = match d {
            DockerId::Transform => &[|a| a.dockers.ui.transform_tab = 1, |a| a.dockers.ui.transform_tab = 2, |a| a.dockers.ui.transform_tab = 3, |a| a.dockers.ui.transform_tab = 4],
            DockerId::Coordinates => &[|a| a.dockers.ui.co.tab = 1, |a| a.dockers.ui.co.tab = 2, |a| a.dockers.ui.co.tab = 3, |a| a.dockers.ui.co.tab = 4, |a| a.dockers.ui.co.tab = 5, |a| a.dockers.ui.co.tab = 6],
            DockerId::LiveGuides => &[|a| a.dockers.ui.lg_margins_open = true, |a| a.dockers.ui.lg_open = [false, true]],
            DockerId::Guidelines => &[|a| a.dockers.ui.guide_type = 2],
            DockerId::FitObjectsToPath => &[|a| a.dockers.ui.fit.style = 2, |a| a.dockers.ui.fit.style = 4],
            DockerId::StepRepeat => &[|a| a.dockers.ui.step_mode = [2, 2], |a| a.dockers.ui.step_open = [false, true], |a| a.dockers.ui.step_open = [true, false]],
            DockerId::Shape => &[|a| a.ts.shaping_op = 1, |a| a.ts.shaping_op = 2, |a| a.ts.shaping_op = 3, |a| a.ts.shaping_op = 4, |a| a.ts.shaping_op = 5, |a| a.ts.shaping_op = 6],
            DockerId::Corners => &[|a| a.dockers.ui.corner_kind = 1, |a| a.dockers.ui.corner_kind = 2],
            DockerId::JoinCurves => &[|a| a.dockers.ui.join_mode = 1, |a| a.dockers.ui.join_mode = 2, |a| a.dockers.ui.join_mode = 3],
            DockerId::Bevel => &[|a| a.dockers.ui.bevel.emboss = true, |a| { a.dockers.ui.bevel.emboss = false; a.dockers.ui.bevel.to_center = true }],
            DockerId::Blend => &[|a| a.dockers.ui.blend_dir = 1, |a| a.dockers.ui.blend_dir = 2, |a| a.dockers.ui.blend_more = true],
            DockerId::Contour => &[
                |a| contour_state(a, vector_core::object::ContourDir::ToCenter, 0, 0),
                |a| contour_state(a, vector_core::object::ContourDir::Outside, 0, 0),
                |a| contour_state(a, vector_core::object::ContourDir::Inside, 1, 0),
                |a| contour_state(a, vector_core::object::ContourDir::Inside, 0, 1),
                |a| contour_state(a, vector_core::object::ContourDir::Inside, 2, 0),
                |a| contour_state(a, vector_core::object::ContourDir::Inside, 0, 2),
            ],
            DockerId::Envelope => &[|a| a.dockers.ui.env_mode = 0, |a| a.dockers.ui.env_mode = 1, |a| a.dockers.ui.env_mode = 2],
            DockerId::Extrude => &[|a| a.dockers.ui.ex_tab = 1, |a| a.dockers.ui.ex_tab = 2, |a| a.dockers.ui.ex_tab = 3, |a| a.dockers.ui.ex_tab = 4],
            DockerId::Lens => &[
                |a| lens_state(a, 0),
                |a| lens_state(a, 1),
                |a| lens_state(a, 2),
                |a| lens_state(a, 3),
                |a| lens_state(a, 4),
                |a| lens_state(a, 5),
                |a| lens_state(a, 6),
                |a| lens_state(a, 7),
                |a| lens_state(a, 8),
                |a| lens_state(a, 9),
                |a| lens_state(a, 10),
                |a| lens_state(a, 11),
            ],
            DockerId::Color => &[|a| a.dockers.ui.color_ed.view = 1, |a| a.dockers.ui.color_ed.view = 2],
            DockerId::Properties => &[
                |a| a.dockers.ui.pr.tab = 6,
                |a| a.dockers.ui.pr.tabbed = false,
                |a| {
                    a.dockers.ui.pr.tabbed = true;
                    a.dockers.ui.pr.summary = false;
                    a.dockers.ui.pr.tab = 4;
                },
                |a| {
                    a.dockers.ui.pr.summary = true;
                    a.dockers.ui.pr.tab = 0;
                    a.dockers.ui.pr.open[0] = false;
                },
                |a| {
                    a.dockers.ui.pr.open[0] = true;
                    a.dockers.ui.pr.tab = 1;
                    pr_fill(a, 0);
                },
                |a| pr_fill(a, 2),
                |a| pr_fill(a, 3),
                |a| pr_fill(a, 4),
                |a| pr_fill(a, 1),
                |a| a.dockers.ui.pr.fill_ed.view = 1,
                |a| a.dockers.ui.pr.fill_ed.view = 2,
                |a| {
                    a.dockers.ui.pr.fill_ed.view = 0;
                    a.dockers.ui.pr.open[1] = false;
                },
                |a| {
                    a.dockers.ui.pr.open[1] = true;
                    a.dockers.ui.pr.tab = 2;
                    pr_transparency(a, 1);
                },
                |a| pr_transparency(a, 2),
                |a| pr_transparency(a, 3),
                |a| pr_transparency(a, 4),
                |a| pr_transparency(a, 5),
                |a| {
                    pr_transparency(a, 0);
                    a.dockers.ui.pr.tab = 3;
                },
                |a| a.dockers.ui.pr.tab = 5,
            ],
            DockerId::Comments => &[comments_state],
            DockerId::Objects => &[|a| a.settings.om_layers_view = true],
            DockerId::ObjectStyles => &[
                |a| a.dockers.ui.os.sel = crate::dockers::ostyles::OsSel::Styles,
                |a| a.dockers.ui.os.sel = crate::dockers::ostyles::OsSel::Sets,
                |a| {
                    a.dockers.ui.os.sel = crate::dockers::ostyles::OsSel::None;
                    a.dockers.ui.os.open[2] = true;
                },
                |a| a.dockers.ui.os.sel = crate::dockers::ostyles::OsSel::Default(1),
            ],
            DockerId::AiGenerate => &[|a| a.settings.ai_terms_accepted = true],
            DockerId::Export => &[|a| a.run(crate::commands::Cmd::ExportDockerAddPage)],
            DockerId::Assets => &[
                |a| {
                    // two drawings and two images in a sample folder (one in a category)
                    let dir = std::path::PathBuf::from("../../target/visual/assets");
                    let _ = std::fs::create_dir_all(dir.join("Food"));
                    for (n, c) in [("Food/PH00066.png", [200u8, 120, 60]), ("PH00453.png", [90, 160, 70])] {
                        let img = image::RgbImage::from_fn(96, 72, |x, y| image::Rgb([c[0].wrapping_add((x / 3) as u8), c[1], c[2].wrapping_add((y / 3) as u8)]));
                        let _ = img.save(dir.join(n));
                    }
                    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="60"><circle cx="30" cy="30" r="24" fill="#e33"/></svg>"##;
                    let _ = std::fs::write(dir.join("Food/CGS03556.svg"), svg);
                    let _ = std::fs::write(dir.join("CGS12877.svg"), svg.replace("e33", "4a4"));
                    a.settings.asset_folders = vec![std::fs::canonicalize(&dir).unwrap_or(dir).to_string_lossy().to_string()];
                },
                |a| a.dockers.ui.assets.rasters = false,
                |a| a.dockers.ui.assets.source = 2,
            ],
            DockerId::ColorProofing => &[|a| {
                a.settings.proof_colors = true;
                a.settings.proof_gamut = true;
            }],
            DockerId::ColorStyles => &[
                |a| {
                    let d = a.doc_mut().unwrap();
                    for (i, c) in [Color::rgb(230, 60, 40), Color::rgb(40, 110, 200), Color::rgb(250, 200, 30), Color::rgb(60, 160, 80)].into_iter().enumerate() {
                        d.doc.color_styles.push(vector_core::NamedColor::new(format!("Color Style {}", i + 1), c));
                    }
                    d.doc.color_harmonies.push(vector_core::document::ColorHarmony {
                        name: "Harmony 1".into(),
                        rule: 6,
                        styles: [Color::rgb(220, 40, 40), Color::rgb(40, 220, 40), Color::rgb(40, 40, 220)].into_iter().enumerate().map(|(i, c)| vector_core::NamedColor::new(format!("Color Style {}", i + 5), c)).collect(),
                    });
                    a.dockers.ui.cs.harmony = Some(0);
                    a.dockers.ui.cs.sel = vec![(Some(0), 0), (Some(0), 1), (Some(0), 2)];
                },
                |a| {
                    a.dockers.ui.cs.harmony = None;
                    a.dockers.ui.cs.sel = vec![(None, 1)];
                },
            ],
            DockerId::Palettes => &[
                |a| a.dockers.ui.pm.open.extend(["lib/process".to_string()]),
                |a| {
                    a.dockers.ui.pm.open.remove("lib/process");
                    a.dockers.ui.pm.open.insert("lib/spot".into());
                    a.dockers.ui.pm.sel = Some("lib/spot".into());
                },
            ],
            DockerId::LinksRollovers => &[
                |a| {
                    a.dockers.ui.link_url.clear();
                    crate::propbar::edit_sel(a, "Hyperlink", |o| o.url = Some("https://example.com".into()));
                },
                |a| crate::propbar::edit_sel(a, "Behavior", |o| {
                    o.url = None;
                    o.link.bookmark = Some("top".into());
                }),
            ],
            DockerId::Text => &[
                |a| {
                    a.dockers.ui.tx.more = [true; 3];
                },
                |a| {
                    a.dockers.ui.tx.more = [false; 3];
                    a.dockers.ui.tx.tab = 1;
                    a.dockers.ui.tx.jump = true;
                },
            ],
            DockerId::FontSampler => &[|a| a.dockers.ui.fs.view = 1, |a| a.dockers.ui.fs.view = 2],
            DockerId::FindReplace => &[
                |a| a.dockers.ui.fr.by_names = true,
                |a| {
                    a.dockers.ui.fr.by_names = false;
                    a.dockers.ui.fr.range_open = true;
                },
                |a| a.dockers.ui.fr.mode = 1,
                |a| a.dockers.ui.fr.mode = 2,
            ],
            DockerId::Pages => &[pages_state, |a| a.doc_mut().unwrap().doc.settings.multipage_view = true],
            DockerId::Symbols => &[symbols_state, |a| a.dockers.ui.sym_lib = 1, |a| a.dockers.ui.sym_local_open = true],
            DockerId::ObjectData => &[|a| a.dialogs.object_data_manager = true, |a| {
                a.dialogs.object_data_manager = false;
                a.dialogs.data_field_editor = Some(Default::default());
            }],
            DockerId::AlignDistribute => &[|a| a.dockers.ui.align_to = 1, |a| a.dockers.ui.align_to = 2, |a| a.dockers.ui.align_to = 3, |a| a.dockers.ui.align_to = 4, |a| a.dockers.ui.dist_to = 1, |a| a.dockers.ui.dist_to = 2],
            _ => &[],
        };
        for (k, f) in states.iter().enumerate() {
            f(h.state_mut());
            save(&mut h, &format!("dk_{:03}_{}", i + 1, k + 1));
        }
        let a = h.state_mut();
        a.dialogs.object_data_manager = false;
        a.dialogs.data_field_editor = None;
        if d == DockerId::Symbols {
            a.doc_mut().unwrap().doc.symbols.clear();
        }
        a.settings.om_layers_view = false;
        a.dockers.ui.fr = Default::default();
        a.dockers.ui.pm = Default::default();
        a.dockers.ui.cs = Default::default();
        a.dockers.ui.assets = Default::default();
        a.settings.asset_folders.clear();
        a.settings.proof_colors = false;
        a.settings.proof_gamut = false;
        if d == DockerId::ColorStyles {
            let dv = a.doc_mut().unwrap();
            dv.doc.color_styles.clear();
            dv.doc.color_harmonies.clear();
        }
        a.dockers.ui.tx = Default::default();
        a.dockers.ui.fs = Default::default();
        a.dockers.ui.export_items.clear();
        a.settings.ai_terms_accepted = false;
        if d == DockerId::LinksRollovers {
            crate::propbar::edit_sel(a, "Behavior", |o| o.link = Default::default());
            a.dockers.ui.link_url.clear();
        }
        if d == DockerId::Pages {
            let dv = a.doc_mut().unwrap();
            dv.doc.pages.truncate(1);
            dv.doc.settings.multipage_view = false;
            dv.page = 0;
        }
    }
}

/// A Pages docker state: three pages, the first current.
#[cfg(test)]
fn pages_state(a: &mut AnyVectorApp) {
    let d = a.doc_mut().unwrap();
    let (w, h) = (d.doc.pages[0].width, d.doc.pages[0].height);
    d.doc.add_page(1, w, h);
    d.doc.add_page(2, w, h);
    d.page = 0;
}

/// A Symbols docker state: a symbol made of the rectangle, selected.
#[cfg(test)]
fn symbols_state(a: &mut AnyVectorApp) {
    let d = a.doc_mut().unwrap();
    let objs: Vec<Object> = d.selection.iter().filter_map(|id| d.doc.find(*id).cloned()).collect();
    let id = d.doc.new_id();
    d.doc.symbols.push(vector_core::document::SymbolDef { id, name: "simbolo-teste".into(), objects: objs, link: None });
    a.dockers.ui.sym_sel = Some("simbolo-teste".into());
}

/// A Comments docker state: a note and a rectangle markup, the note selected.
#[cfg(test)]
fn comments_state(a: &mut AnyVectorApp) {
    use vector_core::document::Comment;
    let d = a.doc_mut().unwrap();
    let pid = d.doc.pages[0].id;
    let mut n = Comment::new(9001, pid, "Reviewer".into(), 0, &[kurbo::Point::new(380.0, 280.0)]);
    n.text = "Make the title larger".into();
    n.date = "2026-11-09".into();
    n.replies.push(("Designer".into(), "Done".into()));
    let mut r = Comment::new(9002, pid, "Reviewer".into(), 4, &[kurbo::Point::new(130.0, 275.0), kurbo::Point::new(375.0, 445.0)]);
    r.text = "Check this color".into();
    r.date = "2026-11-09".into();
    d.doc.comments = vec![n, r];
    a.dockers.ui.comment_sel = Some(9001);
}

/// A Contour docker state: the alignment, corners and color blend.
#[cfg(test)]
fn contour_state(a: &mut AnyVectorApp, dir: vector_core::object::ContourDir, corner: u8, color_dir: u8) {
    let c = &mut a.dockers.ui.contour;
    c.dir = dir;
    c.corner = corner;
    c.color_dir = color_dir;
}

/// A Lens docker state: the selection gets a lens of the `k`th type.
#[cfg(test)]
fn pr_fill(a: &mut AnyVectorApp, k: u8) {
    let f = crate::dockers::props::new_fill(a, k, Color::rgb(70, 130, 200));
    crate::propbar::edit_sel(a, "Fill", |o| o.style.fill = f.clone());
}

fn pr_transparency(a: &mut AnyVectorApp, k: u8) {
    let t = crate::dockers::props::new_transparency(a, k);
    crate::propbar::edit_sel(a, "Transparency", |o| o.style.transparency = t.clone());
}

fn lens_state(a: &mut AnyVectorApp, k: usize) {
    let l = vector_core::object::LensEffect::new(vector_core::object::LensKind::ALL[k]);
    crate::propbar::edit_sel(a, "Lens", |o| o.effects.lens = Some(l.clone()));
}

/// Every bitmap effect's page in the Properties docker, on a sample bitmap (`ANYVECTOR_FX=Id`
/// renders one).
#[test]
#[ignore]
fn visual_effect_pages() {
    let mut h = harness();
    sample_bitmap(&mut h);
    let only: Option<String> = std::env::var("ANYVECTOR_FX").ok();
    for (i, f) in crate::fx::FX.iter().enumerate() {
        if only.as_ref().is_some_and(|o| o != f.id) {
            continue;
        }
        {
            let app = h.state_mut();
            let d = app.doc_mut().unwrap();
            let id = d.selection[0];
            d.doc.find_mut(id).unwrap().effects.bitmap.clear();
            app.dockers.set_width = Some(300.0);
            app.run(Cmd::BitmapEffect(i as u8));
        }
        save(&mut h, &format!("fx_{:03}_{}", i + 1, f.id));
    }
}

/// A new document with a 240 × 160 bitmap (sky, sun and hills), selected.
fn sample_bitmap(h: &mut Harness<'_, AnyVectorApp>) {
    use vector_core::document::ImageData;
    use vector_core::object::ImageObj;
    h.state_mut().new_default_document();
    {
        let d = h.state_mut().doc_mut().unwrap();
        let (w, ht) = (240u32, 160u32);
        let mut rgba = Vec::with_capacity((w * ht * 4) as usize);
        for y in 0..ht {
            for x in 0..w {
                // sky, a yellow sun and green hills
                let (u, v) = (x as f32 / w as f32, y as f32 / ht as f32);
                let hill = 0.62 + 0.08 * (u * 9.0).sin();
                let sun = ((u - 0.72).powi(2) + (v - 0.28).powi(2)).sqrt() < 0.12;
                let c = if v > hill { [60 + (u * 60.0) as u8, 140 + (v * 60.0) as u8, 50] } else if sun { [250, 210, 60] } else { [70 + (v * 90.0) as u8, 140 + (v * 60.0) as u8, 230] };
                rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        let image = d.doc.add_image(ImageData { width: w, height: ht, rgba: std::sync::Arc::new(rgba), encoded: std::sync::Arc::new(vec![]), format: "png".into(), name: "sample.png".into() });
        let id = d.doc.new_id();
        d.add_objects("Bitmap", vec![Object::new(id, ObjKind::Image(ImageObj { image, w: 360.0, h: 240.0, crop: [0.0, 0.0, 1.0, 1.0], adjust: Default::default() }), Affine::translate((150.0, 250.0)), Style::default())]);
        d.selection = vec![id];
    }
}

/// The Bitmaps menu: Convert To Bitmap, Straighten Image, Image Adjustment Lab, Resample, Mode
/// (1 bit, Duotone, Paletted), Inflate Bitmap and PowerTRACE, then the commands without a dialog.
#[test]
#[ignore]
fn visual_bitmap_dialogs() {
    let mut h = harness();
    sample_bitmap(&mut h);
    let shots: [(Cmd, &str); 11] = [
        (Cmd::ConvertToBitmap, "bm_01_convert_to_bitmap"),
        (Cmd::StraightenImage, "bm_02_straighten_image"),
        (Cmd::ImageAdjustmentLab, "bm_03_image_adjustment_lab"),
        (Cmd::ResampleBitmap, "bm_05_resample"),
        (Cmd::BitmapMode(0), "bm_06_convert_to_1_bit"),
        (Cmd::BitmapMode(2), "bm_07_duotone"),
        (Cmd::BitmapMode(3), "bm_08_convert_to_paletted"),
        (Cmd::ManualInflateBitmap, "bm_09_inflate_bitmap"),
        (Cmd::CenterlineTrace(0), "bm_11_centerline_technical"),
        (Cmd::OutlineTrace(0), "bm_13_outline_line_art"),
        (Cmd::OutlineTrace(3), "bm_16_outline_clipart"),
    ];
    for (cmd, name) in shots {
        h.state_mut().run(cmd);
        for _ in 0..4 {
            h.step();
        }
        save(&mut h, name);
        h.state_mut().dialogs.bm = Default::default();
        h.step();
    }
    // Halftone in Convert to 1 bit and Tritone in Duotone
    h.state_mut().run(Cmd::BitmapMode(0));
    h.step();
    h.state_mut().dialogs.bm.one_bit.as_mut().unwrap().o.method = 5;
    save(&mut h, "bm_06b_halftone");
    h.state_mut().dialogs.bm = Default::default();
    h.state_mut().run(Cmd::BitmapMode(2));
    h.step();
    {
        let st = h.state_mut().dialogs.bm.duotone.as_mut().unwrap();
        st.d = crate::bitmaps::Duotone { kind: 2, inks: crate::bitmaps::Duotone::inks_for(2) };
    }
    save(&mut h, "bm_07b_tritone");
    h.state_mut().dialogs.bm = Default::default();
    // the commands that change the bitmap at once, and the trace itself
    for c in [Cmd::RemoveJpegArtifacts, Cmd::BitmapMode(1), Cmd::RemoveBackground, Cmd::BitmapMode(6), Cmd::QuickTrace] {
        h.state_mut().run(c);
        h.step();
    }
    save(&mut h, "bm_20_after_commands");
    let a = h.state_mut();
    let d = a.doc().unwrap();
    assert!(d.doc.pages[0].layers.iter().map(|l| l.objects.len()).sum::<usize>() >= 2, "Quick Trace added the traced group");
}

/// Window ▸ Tile Horizontally, Tile Vertically, Cascade and New Window, and the workspace
/// dialogs: New, Duplicate, Options ▸ Workspaces, Export, Import and Delete.
#[test]
#[ignore]
fn visual_window_workspaces() {
    use crate::workspaces as ws;
    let mut h = harness();
    for c in [Color::rgb(0x1e, 0x90, 0xff), Color::rgb(0xe5, 0x39, 0x35), Color::rgb(0x43, 0xa0, 0x47)] {
        h.state_mut().new_default_document();
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        let mut st = Style::default();
        st.fill = Fill::Solid(c);
        d.add_objects("test", vec![Object::new(id, ObjKind::Rect(RectShape { w: 300.0, h: 200.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((150.0, 200.0)), st)]);
    }
    for (cmd, name) in [(Cmd::TileH, "win_01_tile_horizontally"), (Cmd::TileV, "win_02_tile_vertically"), (Cmd::Cascade, "win_03_cascade")] {
        h.state_mut().run(cmd);
        for _ in 0..4 {
            h.step();
        }
        save(&mut h, name);
    }
    // a second window on the active document
    h.state_mut().run(Cmd::NewWindow);
    h.step();
    h.state_mut().run(Cmd::TileV);
    for _ in 0..4 {
        h.step();
    }
    save(&mut h, "win_04_new_window_tiled");
    {
        let a = h.state_mut();
        assert_eq!(a.docs.len(), 4);
        let names: Vec<String> = a.docs.iter().map(|d| d.display_name()).collect();
        assert!(names.iter().any(|n| n.contains(".cdr:2")) && names.iter().any(|n| n.contains(".cdr:1")), "{names:?}");
        // a change in one window shows in the other
        let d = a.doc_mut().unwrap();
        let id = d.doc.new_id();
        d.add_objects("test", vec![Object::new(id, ObjKind::Rect(RectShape { w: 80.0, h: 80.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((500.0, 500.0)), Style::default())]);
    }
    h.step();
    {
        let a = h.state_mut();
        let act = a.active.unwrap();
        let g = a.docs[act].group;
        let counts: Vec<usize> = a.docs.iter().filter(|d| d.group == g).map(|d| d.doc.pages[0].layers.iter().map(|l| l.objects.len()).sum()).collect();
        assert_eq!(counts.len(), 2);
        assert_eq!(counts[0], counts[1], "both windows show the same drawing");
    }
    h.state_mut().run(Cmd::CombineWindows);
    for _ in 0..3 {
        h.step();
    }
    save(&mut h, "win_05_combined");

    // Window ▸ Workspace ▸ New
    h.state_mut().run(Cmd::WorkspaceNew);
    h.step();
    save(&mut h, "ws_01_new_workspace");
    {
        let st = h.state_mut().dialogs.ws.new.as_mut().unwrap();
        st.name = "My Layout".into();
        st.base = "Page Layout".into();
        st.description = "Pages and object properties at hand.".into();
    }
    save(&mut h, "ws_01b_new_workspace_filled");
    {
        let a = h.state_mut();
        let st = a.dialogs.ws.new.take().unwrap();
        ws::create(a, &st.name, &st.base, &st.description, st.set_current);
        assert_eq!(a.settings.workspace, "My Layout");
        assert!(a.dockers.open.contains(&crate::dockers::DockerId::Pages));
        a.open_options(crate::commands::OptionsPage::Workspaces, None);
    }
    h.step();
    save(&mut h, "ws_02_options_workspaces");
    crate::dialogs::workspace::open_duplicate(h.state_mut(), "Default");
    h.step();
    save(&mut h, "ws_03_duplicate_workspace");
    h.state_mut().dialogs.ws.new = None;
    crate::dialogs::workspace::open_export(h.state_mut(), "My Layout");
    h.state_mut().dialogs.ws.export.as_mut().unwrap().tree.expanded = [0, 1, 2].into_iter().collect();
    h.step();
    save(&mut h, "ws_04_export_workspace");
    let file = std::env::temp_dir().join("vector_visual_ws.cdws");
    {
        let a = h.state_mut();
        let st = a.dialogs.ws.export.take().unwrap();
        let f = ws::WorkspaceFile { name: st.name.clone(), description: st.description.clone(), elements: ws::all_keys(&ws::elements(&st.layout)).into_iter().collect(), layout: st.layout.clone() };
        ws::write_file(&file, &f).unwrap();
        a.dialogs.options = Default::default();
        crate::dialogs::workspace::open_import(a, &file);
        a.dialogs.ws.import.as_mut().unwrap().name = "My Layout 2".into();
        a.dialogs.ws.import.as_mut().unwrap().tree.expanded = [0, 1].into_iter().collect();
    }
    h.step();
    save(&mut h, "ws_05_import_workspace");
    h.state_mut().dialogs.ws.import = None;
    let _ = std::fs::remove_file(&file);
    h.state_mut().run(Cmd::WorkspaceDelete);
    h.step();
    save(&mut h, "ws_06_delete_workspace");
    {
        let a = h.state_mut();
        assert_eq!(a.dialogs.ws.delete.as_deref(), Some("My Layout"));
        a.dialogs.ws.delete = None;
        ws::delete(a, "My Layout");
        assert_eq!(a.settings.workspace, "Default");
        assert!(a.settings.custom_workspaces.is_empty());
    }
}

/// Window ▸ Color Palettes ▸ Palette Editor, and its Select Color (Edit Color / Add Color).
#[test]
#[ignore]
fn visual_palette_editor() {
    use crate::dialogs::palette::{new_select, Purpose, Source};
    let mut h = harness();
    h.state_mut().new_default_document();
    h.state_mut().run(Cmd::PaletteEditor);
    h.step();
    {
        let st = h.state_mut().dialogs.palette_editor.as_mut().unwrap();
        assert!(st.edits[0].source == Source::Default);
        st.sel = Some(13);
    }
    save(&mut h, "pal_01_palette_editor");
    // Edit Color: the color in the CMYK palette
    {
        let a = h.state_mut();
        let mut st = a.dialogs.palette_editor.take().unwrap();
        let nc = st.edits[0].palette.colors[13].clone();
        st.select = Some(new_select(a, Purpose::Edit(13), nc.color, &nc.name));
        a.dialogs.palette_editor = Some(st);
    }
    save(&mut h, "pal_02_edit_color");
    {
        let sc = h.state_mut().dialogs.palette_editor.as_mut().unwrap().select.as_mut().unwrap();
        sc.palettes_mode = false;
        sc.color = Color::rgb(0x2e, 0x7d, 0x32);
    }
    save(&mut h, "pal_03_edit_color_viewers");
    // Add Color
    {
        let a = h.state_mut();
        let mut st = a.dialogs.palette_editor.take().unwrap();
        st.select = Some(new_select(a, Purpose::Add, Color::BLACK, "Black"));
        a.dialogs.palette_editor = Some(st);
    }
    save(&mut h, "pal_04_add_color");
    {
        let a = h.state_mut();
        a.dialogs.palette_editor.as_mut().unwrap().select = None;
        // a deleted color is gone only from the edit until OK
        let st = a.dialogs.palette_editor.as_mut().unwrap();
        let n = st.edits[0].palette.colors.len();
        st.edits[0].palette.colors.remove(0);
        assert_eq!(st.edits[0].original.colors.len(), n);
        a.dialogs.palette_editor = None;
    }
}

/// Properties ▸ Vector Pattern ▸ Edit fill opens the pattern as its own document in View ▸
/// Pattern Preview; closing it puts the changes into the pattern.
#[test]
#[ignore]
fn visual_pattern_preview() {
    use kurbo::Shape;
    let mut h = harness();
    h.state_mut().new_default_document();
    let fill = {
        let d = h.state_mut().doc_mut().unwrap();
        let mut objs = vec![];
        for (c, r) in [(Color::rgb(0xe5, 0x39, 0x35), kurbo::Rect::new(200.0, 200.0, 240.0, 240.0)), (Color::rgb(0x1e, 0x88, 0xe5), kurbo::Rect::new(230.0, 225.0, 270.0, 255.0))] {
            let mut st = Style::default();
            st.fill = Fill::Solid(c);
            let bp = kurbo::Ellipse::from_rect(r).to_path(0.1);
            objs.push(Object::new(d.doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(&bp)), Affine::IDENTITY, st));
        }
        d.add_objects("Shapes", objs);
        let page = d.page;
        let name = crate::dialogs::object::make_pattern(&mut d.doc, page, kurbo::Rect::new(190.0, 190.0, 280.0, 265.0), 0, 1).unwrap();
        let np = d.doc.patterns.iter().find(|p| p.name == name).unwrap().clone();
        let Some(vector_core::style::PatternSource::Tile(tile)) = np.fill.source else { panic!("a vector pattern keeps its objects in a tile") };
        assert_eq!(d.doc.pattern_tiles[&tile].objects.len(), 2, "the pattern keeps the objects it was made from");
        let mut st = Style::default();
        st.fill = Fill::Pattern(vector_core::style::PatternFill { tile: 30.0, ..np.fill.clone() });
        let r = kurbo::Rect::new(300.0, 450.0, 520.0, 620.0).to_path(0.1);
        let o = Object::new(d.doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(&r)), Affine::IDENTITY, st);
        d.add_objects("Patterned", vec![o]);
        np.fill.clone()
    };
    let image = fill.image.unwrap();
    let src_view = h.state_mut().active.unwrap();
    assert!(!crate::ui::bind::visible(h.state_mut(), "141c5d5e-1b88-9cac-4187-9ca1d4f7b9a9"), "Pattern Preview shows only in pattern documents");
    crate::dialogs::object::edit_pattern(h.state_mut(), &fill);
    for _ in 0..3 {
        h.step();
    }
    {
        let a = h.state_mut();
        let d = a.doc().unwrap();
        assert!(d.pattern.is_some() && d.view_mode == crate::docview::ViewMode::PatternPreview);
        assert_eq!(d.doc.pages[0].layers[0].objects.len(), 2);
        assert!(crate::ui::bind::visible(a, "141c5d5e-1b88-9cac-4187-9ca1d4f7b9a9"));
    }
    save(&mut h, "pat_01_pattern_preview");
    // a change, then closing the pattern document
    let old_ptr = {
        let a = h.state_mut();
        let ptr = std::sync::Arc::as_ptr(&a.docs[src_view].doc.images[&image].rgba) as usize;
        let d = a.doc_mut().unwrap();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::rgb(0x43, 0xa0, 0x47));
        let r = kurbo::Rect::new(5.0, 50.0, 85.0, 70.0).to_path(0.1);
        let o = Object::new(d.doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(&r)), Affine::IDENTITY, st);
        d.add_objects("Bar", vec![o]);
        ptr
    };
    save(&mut h, "pat_02_pattern_edited");
    {
        let a = h.state_mut();
        let i = a.active.unwrap();
        a.request_close_doc(i);
        assert_eq!(a.docs.len(), 1);
        let d = &a.docs[0];
        assert_ne!(std::sync::Arc::as_ptr(&d.doc.images[&image].rgba) as usize, old_ptr, "the tile was made again");
        let Some(vector_core::style::PatternSource::Tile(tile)) = fill.source else { unreachable!() };
        assert_eq!(d.doc.pattern_tiles[&tile].objects.len(), 3);
        assert_eq!(d.doc.pattern_tiles[&tile].revision, 1);
        assert!(d.history.is_modified());
    }
    save(&mut h, "pat_03_pattern_applied");
}

/// Issue #55: a guideline clicked with the Pick tool is selected (drawn red) and Delete removes it; Undo brings it
/// back. A click that doesn't move it adds no undo step.
#[test]
#[ignore]
fn visual_delete_guideline() {
    let mut h = harness();
    h.state_mut().new_default_document();
    h.step();
    let screen_y = {
        let d = h.state_mut().doc_mut().unwrap();
        let page = d.page;
        d.doc.pages[page].guides.push(vector_core::Guide {
            kind: vector_core::GuideKind::Horizontal,
            pos: kurbo::Point::new(0.0, 300.0),
            locked: false,
            color: vector_core::Color::rgb(0x00, 0x99, 0xff),
        });
        d.to_screen(kurbo::Point::new(0.0, 300.0)).y
    };
    let x = { let r = h.state_mut().doc().unwrap().canvas_rect; r.left() + 40.0 };
    let undo_before = h.state_mut().doc().unwrap().history.can_undo();
    click(&mut h, egui::pos2(x, screen_y));
    {
        let d = h.state_mut().doc().unwrap();
        assert_eq!(d.guide_sel, Some((d.page, 0)), "the click selects the guideline");
        assert_eq!(d.page_ref().guides.len(), 1);
        assert_eq!(d.history.can_undo(), undo_before, "a click adds no undo step");
    }
    save(&mut h, "93_guideline_selected");
    h.key_press(egui::Key::Delete);
    h.step();
    assert!(h.state_mut().doc().unwrap().page_ref().guides.is_empty(), "Delete removes the guideline");
    h.state_mut().run(Cmd::Undo);
    h.step();
    assert_eq!(h.state_mut().doc().unwrap().page_ref().guides.len(), 1, "Undo brings it back");
    // a click elsewhere drops the guideline's selection: Delete does nothing
    let mid = h.state_mut().doc().unwrap().canvas_rect.center();
    click(&mut h, egui::pos2(mid.x, screen_y + 120.0));
    h.key_press(egui::Key::Delete);
    h.step();
    assert_eq!(h.state_mut().doc().unwrap().page_ref().guides.len(), 1);
}

// ---- clipboard agent ----

/// A harness without a GPU, so these tests run with a plain `cargo test`; one selected rectangle.
fn clip_harness<'a>() -> Harness<'a, AnyVectorApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 900.0)).build_eframe(|cc| {
        let mut app = AnyVectorApp::new(cc);
        app.splash = None;
        app.pending.clear();
        app
    });
    h.state_mut().new_default_document();
    h.state_mut().dialogs.new_doc = None;
    for _ in 0..3 {
        h.step();
    }
    let d = h.state_mut().doc_mut().unwrap();
    let r = kurbo::Shape::to_path(&kurbo::Rect::new(10.0, 10.0, 90.0, 60.0), 0.1);
    let o = Object::new(d.doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(&r)), Affine::IDENTITY, Style::default());
    let id = o.id;
    d.add_objects("Rect", vec![o]);
    d.selection = vec![id];
    h
}

fn clip_count(h: &Harness<'_, AnyVectorApp>) -> usize {
    h.state().doc().unwrap().page_ref().all_objects().count()
}

/// Send events with the modifiers held, then a frame to run the commands.
fn clip_send(h: &mut Harness<'_, AnyVectorApp>, mods: egui::Modifiers, events: Vec<egui::Event>) {
    h.input_mut().events.push(egui::Event::ModifiersChanged(mods));
    h.input_mut().events.extend(events);
    h.step();
    h.input_mut().events.push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

const CLIP_CTRL: egui::Modifiers = egui::Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };

fn clip_key(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: CLIP_CTRL }
}

/// Ctrl+C / Ctrl+V / Ctrl+X as eframe delivers them on Windows: Event::Copy / Paste / PasteImage /
/// Cut, with no key press.
#[test]
fn clip_copy_paste_events() {
    let mut h = clip_harness();
    assert_eq!(clip_count(&h), 1);
    clip_send(&mut h, CLIP_CTRL, vec![egui::Event::Copy]);
    assert_eq!(h.state().clip.objects.len(), 1, "Ctrl+C (Event::Copy) did not copy");
    clip_send(&mut h, CLIP_CTRL, vec![egui::Event::Paste("<svg data-vector-clip=\"1\"></svg>".into())]);
    assert_eq!(clip_count(&h), 2, "Ctrl+V (Event::Paste) did not paste");
    // an image-only clipboard: eframe sends PasteImage
    clip_send(&mut h, CLIP_CTRL, vec![egui::Event::PasteImage(std::sync::Arc::new(egui::ColorImage::filled([1, 1], egui::Color32::RED)))]);
    assert_eq!(clip_count(&h), 3, "Ctrl+V (Event::PasteImage) did not paste");
    // Ctrl+X cuts the selection (the pasted copy)
    clip_send(&mut h, CLIP_CTRL, vec![egui::Event::Cut]);
    assert_eq!(clip_count(&h), 2, "Ctrl+X (Event::Cut) did not cut");
}

/// Plain key presses (kittest, other backends) still work, and a key press together with the
/// clipboard event in one frame pastes once.
#[test]
fn clip_copy_paste_keys() {
    let mut h = clip_harness();
    clip_send(&mut h, CLIP_CTRL, vec![clip_key(egui::Key::C, true), clip_key(egui::Key::C, false)]);
    assert_eq!(h.state().clip.objects.len(), 1, "Ctrl+C key did not copy");
    clip_send(&mut h, CLIP_CTRL, vec![clip_key(egui::Key::V, true), clip_key(egui::Key::V, false)]);
    assert_eq!(clip_count(&h), 2, "Ctrl+V key did not paste");
    clip_send(&mut h, CLIP_CTRL, vec![clip_key(egui::Key::V, true), egui::Event::Paste("x".into()), clip_key(egui::Key::V, false)]);
    assert_eq!(clip_count(&h), 3, "one Ctrl+V pasted more than once");
}

/// While text is typed in place the clipboard events belong to the text.
#[test]
fn clip_events_left_to_text_editing() {
    let mut h = clip_harness();
    h.state_mut().run(Cmd::Copy);
    h.step();
    assert_eq!(h.state().clip.objects.len(), 1);
    h.state_mut().ts.pending_text = Some((Point::new(100.0, 100.0), None));
    clip_send(&mut h, CLIP_CTRL, vec![egui::Event::Paste("typed".into())]);
    let paths = h.state().doc().unwrap().page_ref().all_objects().filter(|o| matches!(o.kind, ObjKind::Path(_))).count();
    assert_eq!(paths, 1, "Ctrl+V while typing pasted objects");
}

/// A 32-bit bottom-up BI_RGB DIB with an unused (zero) alpha byte, as browsers put on CF_DIB.
#[test]
fn clip_decode_dib_rgb32() {
    let mut dib = vec![];
    for v in [40u32, 2, 2] {
        dib.extend_from_slice(&v.to_le_bytes());
    }
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&32u16.to_le_bytes());
    dib.extend_from_slice(&[0; 24]);
    // bottom row first (BGRA): blue, green; then the top row: red, white
    dib.extend_from_slice(&[255, 0, 0, 0, 0, 255, 0, 0, 0, 0, 255, 0, 255, 255, 255, 0]);
    let img = crate::dialogs::clipboard::decode_dib(&dib).expect("decode");
    assert_eq!((img.width, img.height), (2, 2));
    assert_eq!(&img.rgba[0..4], &[255, 0, 0, 255], "top-left is red and opaque");
    assert_eq!(&img.rgba[4..8], &[255, 255, 255, 255]);
    assert_eq!(&img.rgba[8..12], &[0, 0, 255, 255]);
    assert_eq!(&img.rgba[12..16], &[0, 255, 0, 255]);
}

/// A top-down 24-bit DIB (rows padded to 4 bytes) and a BI_BITFIELDS one with its masks after
/// the 40-byte header.
#[test]
fn clip_decode_dib_24_and_bitfields() {
    let header = |w: i32, h: i32, bits: u16, comp: u32| {
        let mut v = vec![];
        v.extend_from_slice(&40u32.to_le_bytes());
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&bits.to_le_bytes());
        v.extend_from_slice(&comp.to_le_bytes());
        v.extend_from_slice(&[0; 20]);
        v
    };
    let mut dib = header(1, -2, 24, 0);
    dib.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);
    let img = crate::dialogs::clipboard::decode_dib(&dib).expect("24-bit");
    assert_eq!((img.width, img.height), (1, 2));
    assert_eq!(&img.rgba[..], &[255, 0, 0, 255, 0, 0, 255, 255]);
    let mut dib = header(1, 1, 32, 3);
    for m in [0x00ff_0000u32, 0x0000_ff00, 0x0000_00ff] {
        dib.extend_from_slice(&m.to_le_bytes());
    }
    dib.extend_from_slice(&[10, 20, 30, 0]);
    let img = crate::dialogs::clipboard::decode_dib(&dib).expect("bitfields");
    assert_eq!(&img.rgba[0..3], &[30, 20, 10]);
    assert!(crate::dialogs::clipboard::decode_dib(&[1, 2, 3]).is_none());
}

#[test]
fn clip_decode_png_and_links() {
    use crate::dialogs::clipboard::{decode_png, text_is_image_link};
    let mut png = vec![];
    image::RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 4])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let img = decode_png(&png).expect("png");
    assert_eq!((img.width, img.height, &img.rgba[0..4]), (3, 2, &[1u8, 2, 3, 4][..]));
    assert_eq!(img.png, png, "the PNG bytes are kept");
    assert!(text_is_image_link("https://example.com/a.png"));
    assert!(text_is_image_link("<img src=\"https://example.com/a.png\">"));
    assert!(!text_is_image_link("Hello world"));
    assert!(!text_is_image_link("<svg data-vector-clip=\"1\"></svg>"));
}

/// Put a bitmap on the real Windows clipboard the way a browser's "Copy image" does (bitmap +
/// "PNG" + the URL as text) and paste it with Ctrl+V; then a DIB-only bitmap; then Copy/Paste
/// inside the program. Run with `cargo test -p anyvector clip_real -- --ignored`; it overwrites
/// the clipboard.
#[cfg(windows)]
#[test]
#[ignore]
fn clip_real_browser_image() {
    let run = |s: &str| {
        let ok = std::process::Command::new("powershell").args(["-NoProfile", "-STA", "-Command", s]).status().map(|s| s.success()).unwrap_or(false);
        assert!(ok, "powershell could not set the clipboard");
    };
    run("Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$bmp = New-Object System.Drawing.Bitmap 120, 80
$g = [System.Drawing.Graphics]::FromImage($bmp); $g.Clear([System.Drawing.Color]::OrangeRed); $g.Dispose()
$png = New-Object System.IO.MemoryStream; $bmp.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)
$d = New-Object System.Windows.Forms.DataObject
$d.SetData([System.Windows.Forms.DataFormats]::Bitmap, $bmp)
$d.SetData('PNG', $png)
$d.SetText('https://example.com/picture.png')
[System.Windows.Forms.Clipboard]::SetDataObject($d, $true)");
    crate::dialogs::clipboard::SYSTEM_CLIPBOARD.with(|c| c.set(true));
    assert!(crate::dialogs::clipboard::system_has_content());
    let img = crate::dialogs::clipboard::system_image().expect("clipboard bitmap");
    assert_eq!((img.width, img.height), (120, 80));
    assert!(!img.png.is_empty(), "the PNG format was read");
    assert_eq!(&img.rgba[0..4], &[255, 69, 0, 255]);
    let mut h = clip_harness();
    let n = clip_count(&h);
    clip_send(&mut h, CLIP_CTRL, vec![egui::Event::Paste("https://example.com/picture.png".into())]);
    let widths: Vec<f64> = h.state().doc().unwrap().page_ref().all_objects().filter_map(|o| if let ObjKind::Image(i) = &o.kind { Some(i.w) } else { None }).collect();
    assert_eq!(clip_count(&h), n + 1);
    assert_eq!(widths.len(), 1, "the bitmap was not pasted as an image");
    // 96 dpi, as File ▸ Import
    assert!((widths[0] - 120.0 * 72.0 / 96.0).abs() < 1e-6);
    // only a bitmap (no "PNG", no text), as Paint does
    run("Add-Type -AssemblyName System.Windows.Forms, System.Drawing; $b = New-Object System.Drawing.Bitmap 30, 20; $g = [System.Drawing.Graphics]::FromImage($b); $g.Clear([System.Drawing.Color]::Blue); $g.Dispose(); [System.Windows.Forms.Clipboard]::SetImage($b)");
    let img = crate::dialogs::clipboard::system_image().expect("DIB bitmap");
    assert_eq!((img.width, img.height), (30, 20));
    assert_eq!(&img.rgba[0..4], &[0, 0, 255, 255]);
    clip_send(&mut h, CLIP_CTRL, vec![egui::Event::PasteImage(std::sync::Arc::new(egui::ColorImage::filled([30, 20], egui::Color32::BLUE)))]);
    assert_eq!(clip_count(&h), n + 2);
    // Copy then Paste inside the program restores the objects, not a bitmap
    let rect = h.state().doc().unwrap().page_ref().all_objects().find(|o| matches!(o.kind, ObjKind::Path(_))).unwrap().id;
    h.state_mut().doc_mut().unwrap().selection = vec![rect];
    clip_send(&mut h, CLIP_CTRL, vec![egui::Event::Copy]);
    let t = crate::dialogs::clipboard::system_own().unwrap_or_default();
    assert!(t.contains("data-vector-clip"));
    assert!(crate::dialogs::clipboard::system_text().is_none(), "no text for Office to paste as SVG source");
    // eframe finds the DIB and sends PasteImage
    clip_send(&mut h, CLIP_CTRL, vec![egui::Event::PasteImage(std::sync::Arc::new(egui::ColorImage::filled([1, 1], egui::Color32::BLUE)))]);
    assert_eq!(clip_count(&h), n + 3);
    let paths = h.state().doc().unwrap().page_ref().all_objects().filter(|o| matches!(o.kind, ObjKind::Path(_))).count();
    assert_eq!(paths, 2, "the copied rectangle came back as a rectangle");
    crate::dialogs::clipboard::SYSTEM_CLIPBOARD.with(|c| c.set(false));
}

/// What Copy puts on the clipboard, in order, without touching the real clipboard.
#[test]
fn clip_copy_formats() {
    use crate::dialogs::clipboard::{copy_formats, ClipFormat, OWN_FORMAT, SVG_FORMAT};
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10pt\" height=\"10pt\"></svg>";
    let px = [255u8, 0, 0, 128];
    let f = copy_formats(svg, vec![1, 2, 3, 4], Some((1, 1, &px)));
    let kinds: Vec<ClipFormat> = f.iter().map(|(k, _)| *k).collect();
    assert_eq!(kinds, vec![ClipFormat::Named(OWN_FORMAT), ClipFormat::Named(SVG_FORMAT), ClipFormat::EnhMetafile, ClipFormat::Named("PNG"), ClipFormat::Dib]);
    assert!(String::from_utf8_lossy(&f[0].1).contains("data-vector-clip"), "the private copy is marked");
    assert_eq!(f[1].1, svg.as_bytes(), "Office gets the plain SVG");
    assert_eq!(f[2].1, vec![1, 2, 3, 4]);
    assert_eq!(&f[3].1[1..4], b"PNG");
    assert_eq!(&f[4].1[40..44], &[127, 127, 255, 255], "the DIB is opaque, on white");
    // no metafile, no bitmap: only the SVG formats
    assert_eq!(copy_formats(svg, vec![], None).len(), 2);
}

/// Copy puts SVG, EMF, PNG and DIB on the real Windows clipboard together; the EMF reads back
/// and pastes as curves (Paste Special ▸ Picture (Enhanced Metafile)). Run with
/// `cargo test -p anyvector clip_real -- --ignored`; it overwrites the clipboard.
#[cfg(windows)]
#[test]
#[ignore]
fn clip_real_office_formats() {
    use crate::dialogs::clipboard::{self as cb, PasteKind};
    cb::SYSTEM_CLIPBOARD.with(|c| c.set(true));
    let mut h = clip_harness();
    clip_send(&mut h, CLIP_CTRL, vec![egui::Event::Copy]);
    {
        use clipboard_win::{formats, is_format_avail, register_format};
        for name in [cb::OWN_FORMAT, cb::SVG_FORMAT, "PNG"] {
            assert!(register_format(name).is_some_and(|f| is_format_avail(f.get())), "{name} is on the clipboard");
        }
        assert!(is_format_avail(formats::CF_ENHMETAFILE), "EMF");
        assert!(is_format_avail(formats::CF_DIB), "DIB");
    }
    let emf = cb::system_emf().expect("the metafile reads back");
    assert!(vector_io::emf_import::is_emf(&emf));
    let size = vector_io::emf_import::frame_size(&emf).unwrap();
    // the rectangle's visual bounds (its outline included)
    assert!((size.0 - 80.0).abs() < 3.0 && (size.1 - 50.0).abs() < 3.0, "the 80 x 50 pt rectangle: {size:?}");
    let svg = String::from_utf8_lossy(&cb::system_named(cb::SVG_FORMAT).unwrap()).to_string();
    assert!(svg.contains("<svg") && !svg.contains("data-vector-clip"));
    let n = clip_count(&h);
    // our own copy: Paste brings the objects back; the metafile is offered to other programs only
    let (kinds, source) = cb::clipboard_kinds(h.state());
    assert_eq!(kinds.first(), Some(&PasteKind::Native));
    assert_eq!(source, vector_core::APP_NAME);
    // a metafile from another program: put only the EMF back and paste it
    {
        let _ = cb::set_system(&[(cb::ClipFormat::EnhMetafile, emf)]);
    }
    assert!(cb::system_own().is_none());
    let (kinds, _) = cb::clipboard_kinds(h.state());
    assert_eq!(kinds, vec![PasteKind::Metafile]);
    h.state_mut().paste_as(None, None);
    h.step();
    assert_eq!(clip_count(&h), n + 1, "the metafile was pasted");
    let pasted = h.state().doc().unwrap().page_ref().all_objects().filter(|o| matches!(o.kind, ObjKind::Path(_))).count();
    assert_eq!(pasted, 2, "as a curve");
    cb::SYSTEM_CLIPBOARD.with(|c| c.set(false));
}

// ---- tools agent ----

fn key(h: &mut Harness<'_, AnyVectorApp>, k: egui::Key, down: bool) {
    h.input_mut().events.push(egui::Event::Key { key: k, physical_key: None, pressed: down, repeat: false, modifiers: egui::Modifiers::NONE });
}

fn doc_harness<'a>() -> Harness<'a, AnyVectorApp> {
    let mut h = harness();
    h.state_mut().new_default_document();
    h.state_mut().dialogs.new_doc = None;
    for _ in 0..3 {
        h.step();
    }
    h
}

/// Bézier tool: holding C while dragging a node's handles makes it a cusp; the C shortcut
/// (Align Centers Vertically) must not fire meanwhile.
#[test]
#[ignore]
fn visual_bezier_cusp_with_c() {
    use crate::tools::ToolId;
    use vector_core::path::NodeKind;
    let mut h = doc_harness();
    // a selected rectangle away from the page center: an Align would move it
    let rid = {
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        let o = Object::new(id, ObjKind::Rect(RectShape { w: 40.0, h: 30.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((20.0, 20.0)), Style::default());
        d.add_objects("test", vec![o]);
        d.selection = vec![id];
        id
    };
    let bounds0 = h.state().doc().unwrap().doc.find(rid).unwrap().bounds();
    h.state_mut().set_tool(ToolId::Bezier);
    let a = egui::pos2(800.0, 400.0);
    let b = egui::pos2(950.0, 400.0);
    click(&mut h, a);
    // second node: drag symmetric handles first, then hold C and swing the outgoing one
    move_to(&mut h, b);
    h.step();
    press(&mut h, b, true);
    h.step();
    for k in 1..=4 {
        move_to(&mut h, b + egui::vec2(15.0 * k as f32, 0.0));
        h.step();
    }
    key(&mut h, egui::Key::C, true);
    h.step();
    for k in 1..=4 {
        move_to(&mut h, b + egui::vec2(60.0, -20.0 * k as f32));
        h.step();
    }
    save(&mut h, "tools_01_bezier_cusp_drag");
    press(&mut h, b + egui::vec2(60.0, -80.0), false);
    h.step();
    key(&mut h, egui::Key::C, false);
    h.step();
    h.state_mut().finish_tool();
    h.step();
    let d = h.state().doc().unwrap();
    assert_eq!(d.doc.find(rid).unwrap().bounds(), bounds0, "the C shortcut fired while dragging");
    let np = d
        .page_ref()
        .all_objects()
        .find_map(|o| match &o.kind {
            ObjKind::Path(np) if np.node_count() == 2 => Some(np.clone()),
            _ => None,
        })
        .expect("the curve was not created");
    let n = &np.subpaths[0].nodes[1];
    assert_eq!(n.kind, NodeKind::Cusp);
    let (vi, vo) = (n.c_in - n.p, n.c_out - n.p);
    assert!(vi.hypot() > 1.0 && vo.hypot() > 1.0, "handles missing: {vi:?} {vo:?}");
    assert!((vi + vo).hypot() > 1.0, "handles are still symmetric: {vi:?} {vo:?}");
    save(&mut h, "tools_02_bezier_cusp");
}

/// Pick tool: double-clicking text edits it in place (no Edit Text dialog).
#[test]
#[ignore]
fn visual_double_click_text_in_place() {
    use crate::tools::ToolId;
    use vector_core::text::{CharFormat, TextObj};
    let mut h = doc_harness();
    let (id, at) = {
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::BLACK);
        let f = CharFormat { size: 48.0, ..CharFormat::default() };
        let o = Object::new(id, ObjKind::Text(TextObj::artistic("Hello world", f)), Affine::translate((100.0, 200.0)), st);
        d.add_objects("test", vec![o]);
        d.selection.clear();
        let c = d.doc.find(id).unwrap().bounds().center();
        (id, d.to_screen(c))
    };
    h.state_mut().set_tool(ToolId::Pick);
    h.step();
    move_to(&mut h, at);
    h.step();
    // a step is 0.25 s and a double-click must come within 0.3 s: one click per step
    for _ in 0..2 {
        press(&mut h, at, true);
        press(&mut h, at, false);
        h.step();
    }
    h.step();
    let a = h.state();
    assert_eq!(a.tool, ToolId::Text);
    assert!(a.dialogs.edit_text.is_none(), "the Edit Text dialog opened");
    let te = a.doc().unwrap().text_edit.clone().expect("not editing in place");
    assert_eq!(te.obj, id);
    assert!(te.caret > 0 && te.caret < 11, "caret not at the click: {}", te.caret);
    save(&mut h, "tools_03_text_in_place");
}

// ---- shortcuts agent ----

/// The reference's chords reach the right commands: C and E align centers on the two axes,
/// Shift+L distributes (not Align Left), Ctrl+Shift+D is Step and Repeat (not Duplicate),
/// Alt+F8 opens the Transform docker's Rotate page, C on a selected node toggles it.
#[test]
#[ignore]
fn visual_shortcuts_reference_chords() {
    use egui::{Key, Modifiers};
    use kurbo::Shape;
    let mut h = harness();
    h.state_mut().new_default_document();
    for _ in 0..3 {
        h.step();
    }
    let ids = {
        let d = h.state_mut().doc_mut().unwrap();
        let mut objs = vec![];
        for (x, y, w) in [(20.0, 20.0, 40.0), (100.0, 60.0, 20.0), (200.0, 30.0, 60.0)] {
            let r = kurbo::Rect::new(x, y, x + w, y + 30.0).to_path(0.1);
            objs.push(Object::new(d.doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(&r)), Affine::IDENTITY, Style::default()));
        }
        let ids: Vec<_> = objs.iter().map(|o| o.id).collect();
        d.add_objects("test", objs);
        d.selection = ids.clone();
        ids
    };
    let centers = |h: &Harness<'_, AnyVectorApp>| -> Vec<kurbo::Point> {
        let d = h.state().doc().unwrap();
        ids.iter().map(|i| d.doc.bounds_of(d.doc.find(*i).unwrap()).center()).collect()
    };
    let n = |h: &Harness<'_, AnyVectorApp>| h.state().doc().unwrap().doc.pages[0].layers.iter().map(|l| l.objects.len()).sum::<usize>();
    let before = centers(&h);
    h.key_press(Key::C);
    h.step();
    let c = centers(&h);
    assert!(c.iter().all(|p| (p.x - c[0].x).abs() < 1e-6), "C lines the centers up on a vertical line: {c:?}");
    assert!(c.iter().zip(&before).all(|(a, b)| (a.y - b.y).abs() < 1e-6), "C moves only horizontally");
    h.key_press(Key::E);
    h.step();
    let c = centers(&h);
    assert!(c.iter().all(|p| (p.y - c[0].y).abs() < 1e-6), "E lines the centers up on a horizontal line: {c:?}");
    // Shift+L must not run Align Left (L): the objects stay on one vertical center line
    h.key_press_modifiers(Modifiers::SHIFT, Key::L);
    h.step();
    let c = centers(&h);
    assert!(c.iter().all(|p| (p.x - c[0].x).abs() < 1e-6), "Shift+L did not align left: {c:?}");
    let count = n(&h);
    let was = h.state().dockers.active == Some(crate::dockers::DockerId::StepRepeat);
    h.key_press_modifiers(Modifiers { ctrl: true, command: true, shift: true, ..Default::default() }, Key::D);
    h.step();
    h.step();
    assert_eq!(n(&h), count, "Ctrl+Shift+D did not duplicate");
    assert_ne!(h.state().dockers.active == Some(crate::dockers::DockerId::StepRepeat), was, "Ctrl+Shift+D toggles Step and Repeat");
    h.key_press_modifiers(Modifiers::ALT, Key::F8);
    h.step();
    h.step();
    assert!(h.state().dockers.active == Some(crate::dockers::DockerId::Transform));
    assert_eq!(h.state().dockers.ui.transform_tab, 1);
    // NodeToggle: C on a cusp node makes it smooth, again makes it cusp
    {
        let d = h.state_mut().doc_mut().unwrap();
        d.selection = vec![ids[0]];
        d.node_sel = vec![(ids[0], 0, 1)];
    }
    let kind = |h: &Harness<'_, AnyVectorApp>| match &h.state().doc().unwrap().doc.find(ids[0]).unwrap().kind {
        ObjKind::Path(np) => np.subpaths[0].nodes[1].kind,
        _ => unreachable!(),
    };
    assert_eq!(kind(&h), vector_core::path::NodeKind::Cusp);
    h.state_mut().run(Cmd::NodeToggle(0));
    h.step();
    assert_eq!(kind(&h), vector_core::path::NodeKind::Smooth);
    h.state_mut().run(Cmd::NodeToggle(0));
    h.step();
    assert_eq!(kind(&h), vector_core::path::NodeKind::Cusp);
    save(&mut h, "shortcuts_01_reference_chords");
}

// ---- break apart ----

/// The property bar's Break Apart is on for a curve with several subpaths (it was always off for
/// curves) and off for a curve with one.
#[test]
fn break_apart_enabled_for_combined_curves() {
    let mut h = clip_harness();
    let on = |h: &Harness<'_, AnyVectorApp>| crate::ui::bind::enabled(h.state(), "d5669070-c0f3-4fc0-b524-e33aeea7925f");
    let d = h.state_mut().doc_mut().unwrap();
    let mut ids = vec![];
    for x in [0.0, 200.0] {
        let r = kurbo::Shape::to_path(&kurbo::Rect::new(x, 0.0, x + 100.0, 100.0), 0.1);
        let o = Object::new(d.doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(&r)), Affine::IDENTITY, Style::default());
        ids.push(o.id);
        d.add_objects("test", vec![o]);
    }
    d.selection = vec![ids[0]];
    assert_eq!(on(&h), Some(false));
    h.state_mut().doc_mut().unwrap().selection = ids;
    h.state_mut().run(Cmd::Combine);
    h.step();
    assert_eq!(h.state().doc().unwrap().selection.len(), 1);
    assert_eq!(on(&h), Some(true));
    h.state_mut().run(Cmd::BreakApart);
    h.step();
    assert_eq!(h.state().doc().unwrap().selection.len(), 2);
}

// ---- start-up document view (issue #63) ----

/// The page's screen rect is centred in the canvas and fits it (the 24-point fit margin on
/// the tighter side).
fn assert_page_fitted(h: &Harness<'_, AnyVectorApp>) {
    let d = h.state().doc().unwrap();
    let c = d.canvas_rect;
    let p = d.to_screen_rect(d.page_ref().rect());
    assert!((p.center().x - c.center().x).abs() < 1.0 && (p.center().y - c.center().y).abs() < 1.0, "page {p:?} not centred in canvas {c:?}");
    assert!(c.contains_rect(p), "page {p:?} does not fit canvas {c:?}");
    let slack = ((c.width() - p.width()) / 2.0).min((c.height() - p.height()) / 2.0);
    assert!((slack - 24.0).abs() < 1.0, "page {p:?} not zoomed to fit canvas {c:?}");
}

/// Start-up with "Create a new blank document": the document is made while the window still
/// has the splash size and only then grows to its real size (2560 × 1307 px at 125%); the
/// page must end up fitted and centred, with the paper size used last (A4).
#[test]
fn startup_document_fits_and_centres_page() {
    let a4 = vector_core::units::page_presets().into_iter().find(|p| p.0 == "A4").unwrap();
    let mut h = Harness::builder().with_size(egui::vec2(620.0, 380.0)).with_pixels_per_point(1.25).build_eframe(|cc| {
        let mut app = AnyVectorApp::new(cc);
        app.splash = None;
        app.pending.clear();
        app.docs.clear();
        app.active = None;
        app.settings.show_new_doc_dialog = false;
        app.settings.startup = crate::settings::StartupAction::NewDocument;
        app.settings.last_new_doc = Some(crate::settings::NewDocDefaults { preset: Some("A4".into()), width: a4.1, height: a4.2, unit: vector_core::Unit::Millimeters, ..Default::default() });
        // what AnyVectorApp::new queues for this start-up action
        app.pending.push(Cmd::New);
        app
    });
    for _ in 0..3 {
        h.step();
    }
    {
        let d = h.state().doc().expect("start-up document");
        assert!((d.page_ref().width - a4.1).abs() < 0.01 && (d.page_ref().height - a4.2).abs() < 0.01, "page is not A4");
        assert_eq!(d.doc.settings.unit, vector_core::Unit::Millimeters);
    }
    // the window becomes the main window: 2560 × 1307 pixels at 125%
    h.set_size(egui::vec2(2560.0 / 1.25, 1307.0 / 1.25));
    for _ in 0..3 {
        h.step();
    }
    assert!(h.state().doc().unwrap().canvas_rect.width() > 1500.0);
    assert_page_fitted(&h);

    // once the user zooms, a resize keeps the view where it is
    let (zoom, origin) = {
        let d = h.state_mut().doc_mut().unwrap();
        let c = d.canvas_rect.center();
        d.zoom_at(c, 2.0);
        (d.zoom, d.origin)
    };
    h.set_size(egui::vec2(1600.0, 900.0));
    for _ in 0..3 {
        h.step();
    }
    let d = h.state().doc().unwrap();
    assert_eq!((d.zoom, d.origin), (zoom, origin));
}

// ---- freehand-family auto-join (issue #65) ----

/// A document with snapping off (so drawn points land where the pointer is) and `tool` active.
fn join_harness<'a>(tool: crate::tools::ToolId) -> Harness<'a, AnyVectorApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 900.0)).build_eframe(|cc| {
        let mut app = AnyVectorApp::new(cc);
        app.splash = None;
        app.pending.clear();
        app
    });
    h.state_mut().new_default_document();
    h.state_mut().dialogs.new_doc = None;
    for _ in 0..3 {
        h.step();
    }
    h.state_mut().doc_mut().unwrap().doc.settings.snap_off = true;
    h.state_mut().set_tool(tool);
    h
}

/// Feed one frame of pointer input straight to the active tool (no egui frame in between).
fn tool_input(h: &mut Harness<'_, AnyVectorApp>, pos: egui::Pos2, pressed: bool, down: bool, released: bool) {
    let inp = crate::tools::Input { pos, hovered: true, pressed, released, down, double: false, secondary: false, secondary_down: false, secondary_released: false, mods: egui::Modifiers::NONE, key_c: false, time: 100.0, dt: 0.016 };
    crate::tools::handle_input(h.state_mut(), &inp);
}

/// Screen position of a document point.
fn scr(h: &Harness<'_, AnyVectorApp>, p: (f64, f64)) -> egui::Pos2 {
    h.state().doc().unwrap().to_screen(Point::new(p.0, p.1))
}

/// Drag through document points (a freehand stroke).
fn tool_stroke(h: &mut Harness<'_, AnyVectorApp>, pts: &[(f64, f64)]) {
    let s: Vec<egui::Pos2> = pts.iter().map(|p| scr(h, *p)).collect();
    tool_input(h, s[0], true, true, false);
    for w in s.windows(2) {
        for k in 1..=8 {
            tool_input(h, w[0] + (w[1] - w[0]) * (k as f32 / 8.0), false, true, false);
        }
    }
    tool_input(h, *s.last().unwrap(), false, false, true);
}

/// Click at a document point, with a screen offset.
fn tool_click(h: &mut Harness<'_, AnyVectorApp>, p: (f64, f64), off: egui::Vec2) {
    let s = scr(h, p) + off;
    tool_input(h, s, true, true, false);
    tool_input(h, s, false, false, true);
}

/// The curves on the page.
fn curves(h: &Harness<'_, AnyVectorApp>) -> Vec<vector_core::path::NodePath> {
    h.state()
        .doc()
        .unwrap()
        .page_ref()
        .all_objects()
        .filter_map(|o| match &o.kind {
            ObjKind::Path(np) => Some(np.clone()),
            _ => None,
        })
        .collect()
}

fn near(a: Point, b: (f64, f64)) -> bool {
    (a - Point::new(b.0, b.1)).hypot() < 0.5
}

/// Freehand: a stroke started on the end node of the curve just drawn continues that curve (one
/// object, one subpath), each stroke is one undo step, and ending on the start node closes it.
#[test]
fn freehand_strokes_continue_the_curve() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Freehand);
    let (a, b, c) = ((100.0, 100.0), (200.0, 120.0), (220.0, 220.0));
    tool_stroke(&mut h, &[a, (150.0, 80.0), b]);
    assert_eq!(curves(&h).len(), 1);
    let first_nodes = curves(&h)[0].node_count();
    let end = curves(&h)[0].subpaths[0].nodes.last().unwrap().p;
    // the second stroke starts 2 pixels off the end node: within the Auto-join distance
    let s = scr(&h, b) + egui::vec2(2.0, 1.0);
    let bs = h.state().doc().unwrap().to_doc(s);
    tool_stroke(&mut h, &[(bs.x, bs.y), (240.0, 160.0), c]);
    let cs = curves(&h);
    assert_eq!(cs.len(), 1, "the second stroke became a separate object");
    let sp = &cs[0].subpaths;
    assert_eq!(sp.len(), 1, "the strokes are not one continuous subpath");
    assert!(!sp[0].closed);
    assert!(near(sp[0].nodes[0].p, a) && near(sp[0].nodes.last().unwrap().p, c), "{:?}", sp[0].nodes);
    // the joint is the old end node itself
    assert!(sp[0].nodes.iter().any(|n| (n.p - end).hypot() < 1e-6), "{:?}", sp[0].nodes);
    // one undo step per stroke
    h.state_mut().run(Cmd::Undo);
    h.step();
    let cs = curves(&h);
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].node_count(), first_nodes);
    h.state_mut().run(Cmd::Redo);
    h.step();
    assert_eq!(curves(&h)[0].subpaths[0].nodes.len(), sp[0].nodes.len());
    // ending a stroke on the start node closes the curve
    tool_stroke(&mut h, &[c, (120.0, 200.0), (a.0 + 1.0, a.1 + 1.0)]);
    let cs = curves(&h);
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].subpaths.len(), 1);
    assert!(cs[0].subpaths[0].closed, "ending on the start node did not close the curve");
    assert!(near(cs[0].subpaths[0].nodes[0].p, a));
    // a stroke away from the ends of the (now closed) curve is a new object
    tool_stroke(&mut h, &[(400.0, 100.0), (450.0, 150.0)]);
    assert_eq!(curves(&h).len(), 2);
}

/// Freehand click-click lines: a line started on the end of the selected curve continues it;
/// started on its start node it goes in front of it; ending on the other end closes the curve.
#[test]
fn freehand_lines_continue_the_curve() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Freehand);
    let z = egui::Vec2::ZERO;
    let (a, b, c, d) = ((100.0, 100.0), (200.0, 100.0), (200.0, 200.0), (100.0, 200.0));
    tool_click(&mut h, a, z);
    tool_click(&mut h, b, z);
    assert_eq!(curves(&h).len(), 1);
    // start the next line on the end node (a pixel off), as a double-click there does
    tool_click(&mut h, b, egui::vec2(1.0, -1.0));
    tool_click(&mut h, c, z);
    let cs = curves(&h);
    assert_eq!(cs.len(), 1, "the second line became a separate object");
    let pts: Vec<Point> = cs[0].subpaths[0].nodes.iter().map(|n| n.p).collect();
    assert_eq!(pts.len(), 3, "{pts:?}");
    assert!(near(pts[0], a) && near(pts[1], b) && near(pts[2], c));
    // from the start node: the new line goes in front, the curve still runs a -> c
    tool_click(&mut h, a, z);
    tool_click(&mut h, d, z);
    let pts: Vec<Point> = curves(&h)[0].subpaths[0].nodes.iter().map(|n| n.p).collect();
    assert_eq!(pts.len(), 4, "{pts:?}");
    assert!(near(pts[0], d) && near(pts[1], a) && near(pts[3], c), "{pts:?}");
    // c -> d closes the square
    tool_click(&mut h, c, z);
    tool_click(&mut h, d, egui::vec2(2.0, 2.0));
    let cs = curves(&h);
    assert_eq!(cs.len(), 1);
    assert!(cs[0].subpaths[0].closed);
    assert_eq!(cs[0].subpaths[0].nodes.len(), 4);
}

/// Polyline, Bézier and Pen: a curve started on the end node of the selected curve continues
/// it, and ending on its start node closes it; 2-Point Line continues it too.
#[test]
fn polyline_bezier_and_line_continue_the_curve() {
    use crate::tools::ToolId;
    let z = egui::Vec2::ZERO;
    for tool in [ToolId::Polyline, ToolId::Bezier, ToolId::Pen] {
        let mut h = join_harness(ToolId::Freehand);
        tool_click(&mut h, (100.0, 100.0), z);
        tool_click(&mut h, (200.0, 100.0), z);
        h.state_mut().set_tool(tool);
        tool_click(&mut h, (200.0, 100.0), egui::vec2(1.0, 1.0));
        tool_click(&mut h, (200.0, 200.0), z);
        tool_click(&mut h, (300.0, 200.0), z);
        h.state_mut().finish_tool();
        let cs = curves(&h);
        assert_eq!(cs.len(), 1, "{tool:?}: a separate object");
        let pts: Vec<Point> = cs[0].subpaths[0].nodes.iter().map(|n| n.p).collect();
        assert_eq!(pts.len(), 4, "{tool:?}: {pts:?}");
        assert!(near(pts[0], (100.0, 100.0)) && near(pts[3], (300.0, 200.0)), "{tool:?}: {pts:?}");
        // and back to the start node closes it
        tool_click(&mut h, (300.0, 200.0), z);
        tool_click(&mut h, (100.0, 200.0), z);
        tool_click(&mut h, (100.0, 100.0), egui::vec2(-1.0, 1.0));
        h.state_mut().finish_tool();
        let cs = curves(&h);
        assert_eq!(cs.len(), 1, "{tool:?}: closing made a separate object");
        assert!(cs[0].subpaths[0].closed, "{tool:?}: not closed");
        assert_eq!(cs[0].subpaths[0].nodes.len(), 5, "{tool:?}");
    }
    let mut h = join_harness(ToolId::TwoPointLine);
    tool_stroke(&mut h, &[(100.0, 100.0), (200.0, 100.0)]);
    tool_stroke(&mut h, &[(200.0, 100.0), (200.0, 200.0)]);
    let cs = curves(&h);
    assert_eq!(cs.len(), 1, "2-Point Line: a separate object");
    assert_eq!(cs[0].subpaths[0].nodes.len(), 3);
    // away from the ends: a new line
    tool_stroke(&mut h, &[(400.0, 100.0), (500.0, 100.0)]);
    assert_eq!(curves(&h).len(), 2);
}

// ---- ungroup ----

/// Issue #56: ungrouping a CorelDRAW drawing selects every piece of it, thousands of objects.
/// Each frame looked every selected object up with a walk of the whole document (the selection
/// box, the toolbars' selection kind, the text buttons), so a frame took seconds and the window
/// went blank. The drawing stays whole and a frame stays quick.
#[test]
fn ungroup_of_a_large_drawing_stays_responsive() {
    const N: usize = 8000;
    let mut h = clip_harness();
    let gid = {
        let d = h.state_mut().doc_mut().unwrap();
        let mut kids = vec![];
        for i in 0..N {
            let (x, y) = ((i % 100) as f64 * 5.0, (i / 100) as f64 * 5.0);
            let r = kurbo::Shape::to_path(&kurbo::Rect::new(x, y, x + 4.0, y + 4.0), 0.1);
            kids.push(Object::new(d.doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(&r)), Affine::IDENTITY, Style::default()));
        }
        let g = Object::new(d.doc.new_id(), ObjKind::Group(kids), Affine::IDENTITY, Style::default());
        let id = g.id;
        d.add_objects("Group", vec![g]);
        d.selection = vec![id];
        id
    };
    h.step();
    let before = h.state().doc().unwrap().selection_bounds().unwrap();
    h.state_mut().run(Cmd::Ungroup);
    h.step();
    let d = h.state().doc().unwrap();
    assert!(d.doc.find(gid).is_none(), "the group is gone");
    assert_eq!(d.selection.len(), N, "every piece is selected");
    assert_eq!(d.page_ref().all_objects().count(), N + 1, "the pieces and the rectangle are on the page");
    assert_eq!(d.selection_bounds(), Some(before), "the pieces stay where they were");
    let t = std::time::Instant::now();
    for _ in 0..3 {
        h.step();
    }
    let frame = t.elapsed() / 3;
    // about 40 ms here; seconds when each selected object was looked up on its own
    assert!(frame < std::time::Duration::from_millis(500), "a frame with {N} selected objects took {frame:?}");
}

// ---- The dialogs added for the 0.4.x enhancements (PDF import, color management, cutters, AI
// providers, Options search). These tests run every time (no GPU); `visual_enhancement_dialogs`
// saves a picture of each with the other visual tests.

fn dialog_harness<'a>() -> Harness<'a, AnyVectorApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 900.0)).build_eframe(|cc| {
        let mut app = AnyVectorApp::new(cc);
        app.splash = None;
        app.pending.clear();
        app
    });
    h.state_mut().new_default_document();
    h.state_mut().dialogs.new_doc = None;
    for _ in 0..3 {
        h.step();
    }
    h
}

/// A blue rectangle on the page, selected.
fn add_selected_rect(h: &mut Harness<'_, AnyVectorApp>) {
    let d = h.state_mut().doc_mut().unwrap();
    let id = d.doc.new_id();
    let mut st = Style::default();
    st.fill = Fill::Solid(Color::rgb(0x1e, 0x90, 0xff));
    let o = Object::new(id, ObjKind::Rect(RectShape { w: 180.0, h: 120.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((72.0, 72.0)), st);
    d.add_objects("test", vec![o]);
    d.selection = vec![id];
}

fn object_count(h: &Harness<'_, AnyVectorApp>) -> usize {
    h.state().doc().unwrap().page_ref().all_objects().count()
}

fn click_label(h: &mut Harness<'_, AnyVectorApp>, label: &str) {
    use egui_kittest::kittest::Queryable;
    h.get_by_label(label).click_accesskit();
    for _ in 0..3 {
        h.step();
    }
}

/// Let queued commands run (`AnyVectorApp::run` only queues) and the window appear.
fn settle(h: &mut Harness<'_, AnyVectorApp>) {
    for _ in 0..4 {
        h.step();
    }
}

fn has_label(h: &Harness<'_, AnyVectorApp>, label: &str) -> bool {
    use egui_kittest::kittest::Queryable;
    h.query_by_label(label).is_some()
}

/// A file in a temp folder only this test uses.
fn temp_file(test: &str, name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("anyvector-test-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// A two-page PDF with a rectangle on each page.
fn two_page_pdf(test: &str) -> std::path::PathBuf {
    let mut doc = vector_core::Document::default();
    while doc.pages.len() < 2 {
        doc.pages.push(doc.pages[0].clone());
    }
    for p in 0..2 {
        let id = doc.new_id();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::rgb(0xd0, 0x20, 0x20));
        let shape = RectShape { w: 100.0, h: 50.0 + 20.0 * p as f64, radii: [0.0; 4], corner: CornerType::Round };
        let o = Object::new(id, ObjKind::Rect(shape), Affine::translate((50.0, 50.0)), st);
        doc.pages[p].layers.last_mut().unwrap().objects.push(o);
    }
    let path = temp_file(test, "two-pages.pdf");
    vector_io::pdf::export(&doc, &path, &vector_io::pdf::PdfOptions::default()).unwrap();
    path
}

/// File ▸ Import of a two-page PDF shows Import PDF; OK brings the first page's drawing in.
#[test]
fn pdf_import_dialog_imports_the_chosen_page() {
    let mut h = dialog_harness();
    let pdf = two_page_pdf("pdf_import");
    let before = object_count(&h);
    assert!(crate::dialogs::pdf_import::start(h.state_mut(), &pdf, false, None));
    settle(&mut h);
    assert!(h.state().dialogs.pdf_import.is_some() && has_label(&h, "OK"), "the dialog shows");
    click_label(&mut h, "OK");
    assert!(h.state().dialogs.pdf_import.is_none(), "OK closes it");
    assert!(object_count(&h) > before, "the page's drawing came in");
}

/// File ▸ Open of a PDF opens it as a new drawing with all its pages.
#[test]
fn pdf_open_dialog_makes_a_new_drawing() {
    let mut h = dialog_harness();
    let pdf = two_page_pdf("pdf_open");
    let docs = h.state().docs.len();
    assert!(crate::dialogs::pdf_import::start(h.state_mut(), &pdf, true, None));
    settle(&mut h);
    click_label(&mut h, "OK");
    assert_eq!(h.state().docs.len(), docs + 1, "a new drawing");
    assert_eq!(h.state().doc().unwrap().doc.pages.len(), 2, "with both pages");
}

/// A file that only looks like a PDF gives a message, not the dialog.
#[test]
fn pdf_import_of_a_broken_file_says_so() {
    let mut h = dialog_harness();
    let bad = temp_file("pdf_bad", "broken.pdf");
    std::fs::write(&bad, b"%PDF-1.4\nnot really a pdf").unwrap();
    assert!(!crate::dialogs::pdf_import::start(h.state_mut(), &bad, false, None));
    assert!(h.state().dialogs.pdf_import.is_none());
    assert!(h.state().dialogs.message_box.is_some(), "the user is told");
    h.step();
}

/// Tools ▸ Color Management opens, draws its tabs, and Cancel leaves the settings alone.
#[test]
fn color_management_dialog_opens_and_cancels() {
    let mut h = dialog_harness();
    let before = h.state().settings.color_management.clone();
    h.state_mut().run(Cmd::ColorManagement);
    settle(&mut h);
    assert!(h.state().dialogs.color_mgmt.is_some() && has_label(&h, "Cancel"));
    for tab in 0..3 {
        h.state_mut().dialogs.color_mgmt.as_mut().unwrap().tab = tab;
        h.step();
    }
    click_label(&mut h, "Cancel");
    assert!(h.state().dialogs.color_mgmt.is_none());
    assert!(h.state().settings.color_management == before, "Cancel changes nothing");
}

/// Object ▸ Create Cut Contour: OK adds a CutContour hairline and the CutContour spot color.
#[test]
fn cut_contour_dialog_adds_the_contour() {
    let mut h = dialog_harness();
    add_selected_rect(&mut h);
    let before = object_count(&h);
    h.state_mut().run(Cmd::CreateCutContour);
    settle(&mut h);
    assert!(h.state().dialogs.cutter.contour.is_some() && has_label(&h, "OK"));
    click_label(&mut h, "OK");
    assert_eq!(object_count(&h), before + 1);
    let d = h.state().doc().unwrap();
    assert!(d.page_ref().all_objects().any(|o| o.name == vector_core::color::CUT_CONTOUR_NAME));
    assert!(d.doc.palette.iter().any(|c| c.name == vector_core::color::CUT_CONTOUR_NAME && c.spot));
}

/// With nothing selected, Create Cut Contour does nothing.
#[test]
fn cut_contour_needs_a_selection() {
    let mut h = dialog_harness();
    h.state_mut().run(Cmd::CreateCutContour);
    settle(&mut h);
    assert!(h.state().dialogs.cutter.contour.is_none());
}

/// Export as PLT and as DXF: the export dialog, then OK writes the file.
#[test]
fn hpgl_and_dxf_export_dialogs_write_files() {
    use crate::dialogs::cutter;
    let mut h = dialog_harness();
    add_selected_rect(&mut h);
    let plt = temp_file("cutter_export", "drawing.plt");
    let target = cutter::Target::new(h.state(), plt.clone(), vec![0], false);
    cutter::start_hpgl(h.state_mut(), target, false);
    settle(&mut h);
    assert!(h.state().dialogs.cutter.hpgl.is_some() && has_label(&h, "OK"));
    click_label(&mut h, "OK");
    let hpgl = std::fs::read_to_string(&plt).expect("the PLT file is written");
    assert!(hpgl.starts_with("IN") && hpgl.contains("PD"), "HPGL with pen-down moves: {hpgl:.80}");

    let dxf = plt.with_extension("dxf");
    let target = cutter::Target::new(h.state(), dxf.clone(), vec![0], false);
    cutter::start_dxf(h.state_mut(), target, false);
    settle(&mut h);
    assert!(h.state().dialogs.cutter.dxf.is_some() && has_label(&h, "OK"));
    click_label(&mut h, "OK");
    let text = std::fs::read_to_string(&dxf).expect("the DXF file is written");
    assert!(text.contains("SECTION") && text.contains("ENTITIES") && text.trim_end().ends_with("EOF"));
}

/// Tools ▸ Options ▸ Global ▸ Privacy: each AI provider shows its own settings (the field
/// captions are painted, so the check is on the checkboxes each one has).
#[test]
fn ai_provider_options_follow_the_provider() {
    use crate::ai_provider::{ImageProvider, Provider};
    use egui_kittest::kittest::Queryable;
    let mut h = dialog_harness();
    h.state_mut().open_options(crate::commands::OptionsPage::Global, Some("Privacy"));
    // (provider, has "The model accepts images", has "Use Ollama's own API")
    let cases = [(Provider::Anthropic, false, false), (Provider::OpenAi, true, false), (Provider::Ollama, true, true), (Provider::LmStudio, true, false)];
    for (p, vision, ollama) in cases {
        h.state_mut().settings.ai_config.provider = p;
        settle(&mut h);
        assert_eq!(h.query_by_label_contains("accepts images").is_some(), vision, "{p:?}: the images checkbox");
        assert_eq!(h.query_by_label_contains("Ollama's own API").is_some(), ollama, "{p:?}: the Ollama API checkbox");
    }
    assert!(has_label(&h, "ComfyUI"), "the image provider choice");
    h.state_mut().settings.ai_config.image_provider = ImageProvider::ComfyUi;
    settle(&mut h);
}

/// Save Key's note in Options says where the key is kept: in the Windows account, on Linux in the
/// user's systemd environment file (~/.config/environment.d/anyvector.conf).
#[test]
fn ai_key_note_says_where_the_key_is_kept() {
    use egui_kittest::kittest::Queryable;
    let mut h = dialog_harness();
    h.state_mut().settings.language = "en-US".into();
    h.state_mut().settings.ai_config.provider = crate::ai_provider::Provider::Anthropic;
    h.state_mut().open_options(crate::commands::OptionsPage::Global, Some("Privacy"));
    settle(&mut h);
    let place = if cfg!(windows) { "environment variable of your Windows account" } else { "environment.d/anyvector.conf" };
    assert!(h.query_by_label_contains(place).is_some(), "Save Key's note names {place}");
}

/// The new dialogs, as pictures: Import PDF, Color Management (its three tabs), Create Cut
/// Contour, HPGL and DXF Export, the AI providers in Options, the Options search and Export
/// for Office.
#[test]
#[ignore]
fn visual_enhancement_dialogs() {
    use crate::dialogs::cutter;
    let mut h = harness();
    h.state_mut().new_default_document();
    h.state_mut().dialogs.new_doc = None;
    add_selected_rect(&mut h);
    let pdf = two_page_pdf("visual_enhancements");
    crate::dialogs::pdf_import::start(h.state_mut(), &pdf, false, None);
    save(&mut h, "95_pdf_import");
    h.state_mut().dialogs.pdf_import = None;
    h.state_mut().run(Cmd::ColorManagement);
    settle(&mut h);
    for tab in 0..3 {
        h.state_mut().dialogs.color_mgmt.as_mut().unwrap().tab = tab;
        save(&mut h, &format!("95_color_management_{tab}"));
    }
    h.state_mut().dialogs.color_mgmt = None;
    h.state_mut().run(Cmd::CreateCutContour);
    save(&mut h, "95_cut_contour");
    h.state_mut().dialogs.cutter.contour = None;
    let plt = temp_file("visual_enhancements_cut", "drawing.plt");
    let t = cutter::Target::new(h.state(), plt.clone(), vec![0], false);
    cutter::start_hpgl(h.state_mut(), t, false);
    save(&mut h, "95_hpgl_export");
    h.state_mut().dialogs.cutter.hpgl = None;
    let t = cutter::Target::new(h.state(), plt.with_extension("dxf"), vec![0], false);
    cutter::start_dxf(h.state_mut(), t, false);
    save(&mut h, "95_dxf_export");
    h.state_mut().dialogs.cutter.dxf = None;
    h.state_mut().open_options(crate::commands::OptionsPage::Global, Some("Privacy"));
    for p in crate::ai_provider::Provider::ALL {
        h.state_mut().settings.ai_config.provider = p;
        save(&mut h, &format!("95_ai_provider_{p:?}").to_lowercase());
    }
    h.state_mut().settings.ai_config.provider = crate::ai_provider::Provider::Anthropic;
    h.state_mut().dialogs.options.search.query = "crosshair".into();
    save(&mut h, "95_options_search");
    h.state_mut().dialogs.options = Default::default();
    h.state_mut().run(Cmd::ExportForOffice);
    save(&mut h, "95_export_for_office");
}

/// Issues 79, 82, 83 and 85: the rotation center moves, dialogs move by their title bar,
/// rectangle corners round by dragging their nodes, and one wheel notch zooms by the set rate.
#[test]
#[ignore]
fn visual_issue_fixes_round_rotate_move() {
    let mut h = harness();
    h.state_mut().new_default_document();
    for _ in 0..3 {
        h.step();
    }
    let rect = {
        let d = h.state_mut().doc_mut().unwrap();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::rgb(0x1e, 0x90, 0xff));
        let id = d.doc.new_id();
        d.add_objects("test", vec![Object::new(id, ObjKind::Rect(RectShape { w: 200.0, h: 120.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((100.0, 100.0)), st)]);
        d.selection = vec![id];
        id
    };
    let to_screen = |h: &Harness<'_, AnyVectorApp>, x: f64, y: f64| {
        let p = h.state().doc().unwrap().to_screen(Point::new(x, y));
        egui::pos2(p.x as f32, p.y as f32)
    };
    let radii = |h: &Harness<'_, AnyVectorApp>| match &h.state().doc().unwrap().doc.find(rect).unwrap().kind {
        ObjKind::Rect(r) => r.radii,
        _ => unreachable!(),
    };

    // 83: the Shape tool rounds a corner by dragging its node along a side
    h.state_mut().set_tool(crate::tools::ToolId::Shape);
    h.step();
    let (a, b) = (to_screen(&h, 100.0, 100.0), to_screen(&h, 130.0, 100.0));
    drag(&mut h, a, b);
    let r = radii(&h);
    assert!((r[0] - 30.0).abs() < 1.5, "the corner follows the node: {r:?}");
    assert!(r.iter().all(|x| (x - r[0]).abs() < 1e-9), "corners locked together by default: {r:?}");
    save(&mut h, "issues_01_rounded_by_dragging");
    // the node now sits where the rounding meets the side, and drags on from there
    let (a, b) = (to_screen(&h, 130.0, 100.0), to_screen(&h, 145.0, 100.0));
    drag(&mut h, a, b);
    let r = radii(&h);
    assert!((r[0] - 45.0).abs() < 1.5, "{r:?}");
    // and back towards the corner squares it again
    let (a, b) = (to_screen(&h, 145.0, 100.0), to_screen(&h, 80.0, 100.0));
    drag(&mut h, a, b);
    assert!(radii(&h)[0].abs() < 1e-9, "{:?}", radii(&h));

    // 79: in rotate mode the rotation center moves on its own; the object stays where it is
    h.state_mut().set_tool(crate::tools::ToolId::Pick);
    {
        let d = h.state_mut().doc_mut().unwrap();
        d.selection = vec![rect];
        d.rotate_mode = true;
    }
    h.step();
    let before = h.state().doc().unwrap().doc.find(rect).unwrap().transform;
    let (a, b) = (to_screen(&h, 200.0, 160.0), to_screen(&h, 300.0, 220.0));
    drag(&mut h, a, b);
    let d = h.state().doc().unwrap();
    assert_eq!(d.doc.find(rect).unwrap().transform, before, "dragging the center does not move the object");
    let c = d.rotation_center().unwrap();
    // (it snaps, like objects, so it lands near the pointer)
    assert!((c - Point::new(300.0, 220.0)).hypot() < 10.0, "{c:?}");
    assert!(d.rotate_mode);
    save(&mut h, "issues_02_rotation_center_moved");
    // the rotate handles now turn the rectangle around the new center
    let bb = h.state().doc().unwrap().selection_bounds().unwrap();
    let e = h.state().doc().unwrap().doc_len(6.0);
    let corner = to_screen(&h, bb.x0 - e, bb.y0 - e);
    drag(&mut h, corner, corner + egui::vec2(0.0, -80.0));
    let after = h.state().doc().unwrap().doc.find(rect).unwrap().transform;
    assert_ne!(after, before, "the rectangle turned");
    let moved = after * before.inverse();
    assert!(((moved * c) - c).hypot() < 1e-6, "it turned around the moved center");
    save(&mut h, "issues_03_rotated_around_moved_center");

    // 91: an angle typed on the property bar (or Rotate on the menu) also turns around the moved
    // center, which stays with the rectangle
    let c = h.state().doc().unwrap().rotation_center().unwrap();
    let before = h.state().doc().unwrap().doc.find(rect).unwrap().transform;
    h.state_mut().run(Cmd::Rotate(30.0));
    h.step();
    let d = h.state().doc().unwrap();
    let after = d.doc.find(rect).unwrap().transform;
    assert_ne!(after, before, "the rectangle turned");
    assert!(((after * before.inverse() * c) - c).hypot() < 1e-6, "Rotate turned around the moved center");
    assert!((d.rotation_center().unwrap() - c).hypot() < 1e-6, "the center stays where it was");
    h.state_mut().doc_mut().unwrap().transform_selection("Move", kurbo::Affine::translate((15.0, 5.0)));
    let c2 = h.state().doc().unwrap().rotation_center().unwrap();
    assert!((c2 - (c + kurbo::Vec2::new(15.0, 5.0))).hypot() < 1e-6, "the center moves with the rectangle");

    // 85: one wheel notch zooms by the Options zoom rate
    let z0 = h.state().doc().unwrap().zoom;
    let mid = to_screen(&h, 200.0, 400.0);
    move_to(&mut h, mid);
    h.step();
    h.input_mut().events.push(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, 1.0), modifiers: egui::Modifiers::NONE, phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Unknown });
    for _ in 0..40 {
        h.step();
    }
    let z1 = h.state().doc().unwrap().zoom;
    let want = 1.0 + h.state().settings.zoom_rate / 100.0;
    assert!((z1 / z0 - want).abs() < 0.01, "one notch zoomed {:.3}x, the rate is {want:.3}x", z1 / z0);

    // 82: a dialog moves by dragging its title bar
    h.state_mut().open_options(crate::commands::OptionsPage::App, None);
    for _ in 0..3 {
        h.step();
    }
    let area = egui::Id::unique(("dlg", "options"));
    let r0 = h.ctx.memory(|m| m.area_rect(area)).expect("the Options dialog is open");
    let grab = r0.left_top() + egui::vec2(120.0, 16.0);
    drag(&mut h, grab, grab + egui::vec2(-300.0, 120.0));
    for _ in 0..2 {
        h.step();
    }
    let r1 = h.ctx.memory(|m| m.area_rect(area)).unwrap();
    let moved = r1.min - r0.min;
    assert!((moved - egui::vec2(-300.0, 120.0)).length() < 3.0, "the dialog moved by {moved:?}");
    save(&mut h, "issues_04_dialog_moved");
}

/// A small DXF drawing in millimeters: a red rectangle on "Outline", a blue circle and a text
/// on "Notes", a half-round slot, a block inserted twice.
fn sample_dxf(test: &str) -> std::path::PathBuf {
    let p = temp_file(test, "plan.dxf");
    let pairs: &[(i32, &str)] = &[
        (0, "SECTION"), (2, "HEADER"), (9, "$INSUNITS"), (70, "4"), (0, "ENDSEC"),
        (0, "SECTION"), (2, "TABLES"), (0, "TABLE"), (2, "LAYER"),
        (0, "LAYER"), (2, "Outline"), (70, "0"), (62, "1"), (370, "35"),
        (0, "LAYER"), (2, "Notes"), (70, "0"), (62, "5"),
        (0, "ENDTAB"), (0, "ENDSEC"),
        (0, "SECTION"), (2, "BLOCKS"),
        (0, "BLOCK"), (2, "Bolt"), (10, "0"), (20, "0"),
        (0, "CIRCLE"), (8, "0"), (10, "0"), (20, "0"), (40, "4"),
        (0, "LINE"), (8, "0"), (10, "-6"), (20, "0"), (11, "6"), (21, "0"),
        (0, "LINE"), (8, "0"), (10, "0"), (20, "-6"), (11, "0"), (21, "6"),
        (0, "ENDBLK"), (0, "ENDSEC"),
        (0, "SECTION"), (2, "ENTITIES"),
        (0, "LWPOLYLINE"), (8, "Outline"), (90, "4"), (70, "1"), (10, "0"), (20, "0"), (10, "160"), (20, "0"), (10, "160"), (20, "100"), (10, "0"), (20, "100"),
        (0, "LWPOLYLINE"), (8, "Outline"), (90, "4"), (70, "1"), (10, "60"), (20, "40"), (10, "100"), (20, "40"), (42, "1"), (10, "100"), (20, "60"), (10, "60"), (20, "60"), (42, "1"),
        (0, "CIRCLE"), (8, "Notes"), (10, "130"), (20, "70"), (40, "15"),
        (0, "INSERT"), (8, "Outline"), (2, "Bolt"), (10, "15"), (20, "15"),
        (0, "INSERT"), (8, "Outline"), (2, "Bolt"), (10, "145"), (20, "15"), (50, "45"),
        (0, "TEXT"), (8, "Notes"), (10, "10"), (20, "80"), (40, "8"), (1, "PLATE %%c30"),
        (0, "ENDSEC"), (0, "EOF"),
    ];
    let s: String = pairs.iter().map(|(c, v)| format!("{c}\n{v}\n")).collect();
    std::fs::write(&p, s).unwrap();
    p
}

/// File ▸ Open of a DXF file (issue #89): a new drawing at the drawing's size, with its layers.
#[test]
fn dxf_open_makes_a_drawing_with_its_layers() {
    let mut h = dialog_harness();
    let docs = h.state().docs.len();
    let p = sample_dxf("dxf_open");
    assert!(h.state_mut().open_path(&p));
    settle(&mut h);
    assert_eq!(h.state().docs.len(), docs + 1, "a new drawing");
    let d = &h.state().doc().unwrap().doc;
    let names: Vec<&str> = d.pages[0].layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Outline", "Notes"]);
    let mm = 72.0 / 25.4;
    assert!((d.pages[0].width - 160.0 * mm).abs() < 0.01 && (d.pages[0].height - 100.0 * mm).abs() < 0.01, "the page fits the 160 x 100 mm plate");
    assert_eq!(d.pages[0].layers[0].objects.len(), 2 + 3 * 2, "the plate, the slot and two bolts of three parts");
}

/// File ▸ Import of a DXF file adds its drawing to the page as one group.
#[test]
fn dxf_import_adds_the_drawing() {
    let mut h = dialog_harness();
    let before = object_count(&h);
    let p = sample_dxf("dxf_import");
    h.state_mut().import_path(&p, None);
    settle(&mut h);
    assert!(object_count(&h) > before, "the drawing came in");
}

/// SVG Export (issue #88): "Export text: Text" writes the text as <text>, and is remembered.
#[test]
fn svg_export_dialog_keeps_text_as_text() {
    use vector_core::text::{CharFormat, TextObj};
    let mut h = dialog_harness();
    {
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        let mut st = Style::default();
        st.fill = Fill::Solid(Color::BLACK);
        let o = Object::new(id, ObjKind::Text(TextObj::artistic("Live text", CharFormat { size: 36.0, ..CharFormat::default() })), Affine::translate((100.0, 200.0)), st);
        d.add_objects("test", vec![o]);
    }
    let out = temp_file("svg_text", "out.svg");
    h.state_mut().dialogs.svg = Some(crate::dialogs::export::SvgState { path: out.clone(), pages: vec![0], selection: false, unit: 0, dpi: 300.0, text_as_text: true });
    settle(&mut h);
    assert!(has_label(&h, "Export text:"), "the option shows");
    click_label(&mut h, "OK");
    let svg = std::fs::read_to_string(&out).unwrap();
    assert!(svg.contains("<text") && svg.contains(">Live text</tspan>"), "{svg}");
    assert!(h.state().settings.svg_text_as_text, "the choice is remembered");
}

/// Renders: a DXF plan opened (issue #89) and the SVG Export dialog with its text option (#88).
#[test]
#[ignore]
fn visual_dxf_open_and_svg_text_option() {
    let mut h = harness();
    let p = sample_dxf("visual_dxf");
    assert!(h.state_mut().open_path(&p));
    for _ in 0..5 {
        h.step();
    }
    h.state_mut().run(Cmd::ZoomToPage);
    save(&mut h, "dxf_01_plan_opened");
    let out = temp_file("visual_svg_text", "out.svg");
    h.state_mut().dialogs.svg = Some(crate::dialogs::export::SvgState { path: out, pages: vec![0], selection: false, unit: 1, dpi: 300.0, text_as_text: true });
    save(&mut h, "svg_01_export_text_option");
}

// ---- bugs #95–#99 (0.5.3) ----

/// An unfilled rectangle on a fresh document, and the screen point at its center.
fn unfilled_rect(h: &mut Harness<'_, AnyVectorApp>) -> (u64, egui::Pos2) {
    let d = h.state_mut().doc_mut().unwrap();
    let id = d.doc.new_id();
    let r = RectShape { w: 300.0, h: 200.0, radii: [0.0; 4], corner: CornerType::default() };
    let mut st = Style::default();
    st.fill = Fill::None;
    st.outline.color = Some(Color::BLACK);
    st.outline.width = 1.0;
    d.add_objects("test", vec![Object::new(id, ObjKind::Rect(r), Affine::translate((100.0, 100.0)), st)]);
    d.selection.clear();
    let c = d.doc.find(id).unwrap().bounds().center();
    (id, d.to_screen(c))
}

/// #96/#116: Treat All Objects as Filled is on by default (as up to 0.5.2), so a click inside an
/// unfilled shape picks it, and the property bar button really turns that off and on again (a
/// second, document copy used to keep it on).
#[test]
fn treat_as_filled_button_controls_picking_inside() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().settings.treat_as_filled = crate::settings::Settings::default().treat_as_filled;
    assert!(h.state().settings.treat_as_filled, "#116: the default is on");
    let (id, at) = unfilled_rect(&mut h);
    h.state_mut().set_tool(ToolId::Pick);
    h.step();
    assert_eq!(crate::propbar::option_state(h.state(), "b73ed810"), Some((true, true)));
    click(&mut h, at);
    assert_eq!(h.state().doc().unwrap().selection, vec![id], "an unfilled shape wasn't picked from inside");
    crate::propbar::press(h.state_mut(), "b73ed810");
    assert_eq!(crate::propbar::option_state(h.state(), "b73ed810"), Some((true, false)));
    h.state_mut().doc_mut().unwrap().selection.clear();
    click(&mut h, at + egui::vec2(3.0, 3.0));
    assert!(h.state().doc().unwrap().selection.is_empty(), "turning the button off didn't stop picking inside");
    crate::propbar::press(h.state_mut(), "b73ed810");
    click(&mut h, at);
    assert_eq!(h.state().doc().unwrap().selection, vec![id], "turning the button on again didn't pick inside");
    // the pre-0.5.3 name isn't read back; the current name is
    let old: crate::settings::Settings = serde_json::from_str(r#"{"treat_as_filled": false}"#).unwrap();
    assert!(old.treat_as_filled);
    let new: crate::settings::Settings = serde_json::from_str(r#"{"pick_treat_as_filled": false}"#).unwrap();
    assert!(!new.treat_as_filled);
}

/// #116: settings saved by 0.5.3/0.5.4 have Treat All Objects as Filled off and no revision; they
/// are switched back on once, and a choice made after that sticks.
#[test]
fn treat_as_filled_off_from_0_5_4_is_migrated_once() {
    use crate::settings::{Settings, SETTINGS_REVISION};
    let mut s: Settings = serde_json::from_str(r#"{"pick_treat_as_filled": false}"#).unwrap();
    assert_eq!(s.settings_revision, 0, "a file without a revision is revision 0");
    assert!(!s.treat_as_filled);
    s.migrate();
    assert!(s.treat_as_filled, "0.5.4's off value wasn't reset");
    assert_eq!(s.settings_revision, SETTINGS_REVISION);
    // the user turns it off again; saving and loading keeps it off
    s.treat_as_filled = false;
    let mut back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    back.migrate();
    assert!(!back.treat_as_filled, "the migration ran twice");
    assert_eq!(back.settings_revision, SETTINGS_REVISION);
    // fresh settings are current and on
    let fresh = Settings::default();
    assert!(fresh.treat_as_filled);
    assert_eq!(fresh.settings_revision, SETTINGS_REVISION);
}

/// #117: Virtual Segment Delete works on a rectangle that was never converted to curves: a click
/// on a side turns it into curves without that side, as one undo step; a click on empty space
/// changes nothing.
#[test]
fn virtual_segment_delete_on_a_rectangle() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    let (id, _) = unfilled_rect(&mut h);
    let (side, empty) = {
        let d = h.state().doc().unwrap();
        let b = kurbo::Shape::bounding_box(&d.doc.find(id).unwrap().world_path());
        (d.to_screen(Point::new(b.center().x + 30.0, b.y0)), d.to_screen(Point::new(b.x1 + 150.0, b.y1 + 150.0)))
    };
    h.state_mut().set_tool(ToolId::VirtualDelete);
    h.step();
    click(&mut h, empty);
    assert!(matches!(h.state().doc().unwrap().doc.find(id).unwrap().kind, ObjKind::Rect(_)), "a click on empty space changed the rectangle");
    click(&mut h, side);
    let o = h.state().doc().unwrap().doc.find(id).expect("the rectangle is gone").clone();
    let ObjKind::Path(np) = &o.kind else { panic!("the rectangle wasn't converted to curves: {:?}", o.kind) };
    assert!(!np.is_closed(), "the side wasn't deleted");
    assert_eq!(np.subpaths.iter().map(|s| s.segment_count()).sum::<usize>(), 3);
    // conversion and deletion are one step
    h.state_mut().run(Cmd::Undo);
    h.step();
    assert!(matches!(h.state().doc().unwrap().doc.find(id).unwrap().kind, ObjKind::Rect(_)), "one undo didn't bring the rectangle back");
}

/// #118: Trim on the property bar keeps the trimming (source) object by default; the property
/// bar's Leave Original toggles change that, Trim is one undo step, and objects that don't overlap
/// are never deleted.
#[test]
fn trim_keeps_the_source_and_the_bar_toggles_it() {
    use crate::tools::ToolId;
    use egui_kittest::kittest::Queryable;
    use vector_core::ops::Shaping;
    let mut h = doc_harness();
    let sq = |d: &mut crate::docview::DocView, at: (f64, f64), w: f64| {
        let id = d.doc.new_id();
        let r = RectShape { w, h: w, radii: [0.0; 4], corner: CornerType::default() };
        let st = Style { fill: Fill::Solid(Color::rgb(200, 40, 40)), ..Style::default() };
        d.add_objects("test", vec![Object::new(id, ObjKind::Rect(r), Affine::translate(at), st)]);
        id
    };
    let (a, b, far) = {
        let d = h.state_mut().doc_mut().unwrap();
        (sq(d, (100.0, 100.0), 200.0), sq(d, (200.0, 200.0), 200.0), sq(d, (600.0, 600.0), 50.0))
    };
    let area = |h: &Harness<'_, AnyVectorApp>, id| h.state().doc().unwrap().doc.find(id).map(|o| kurbo::Shape::area(&o.world_path()).abs());
    let count = |h: &Harness<'_, AnyVectorApp>| h.state().doc().unwrap().page_ref().all_objects().count();
    let select = |h: &mut Harness<'_, AnyVectorApp>, ids: Vec<u64>| {
        h.state_mut().doc_mut().unwrap().selection = ids;
        h.step();
    };
    h.state_mut().set_tool(ToolId::Pick);
    select(&mut h, vec![a, b]);
    assert!(h.state().ts.leave_source && !h.state().ts.leave_target, "the source isn't kept by default");
    let toggle = |h: &Harness<'_, AnyVectorApp>, label: &str| h.query_by_label(label).map(|n| n.rect().center()).unwrap_or_else(|| panic!("no '{label}' toggle on the property bar"));
    toggle(&h, "Leave original target object");
    // default Trim: the target is cut, the source stays
    h.state_mut().run(Cmd::Shaping(Shaping::Trim));
    h.step();
    assert!(area(&h, a).is_some(), "Trim deleted the source");
    assert!((area(&h, b).unwrap() - 30000.0).abs() < 1.0, "the target wasn't trimmed");
    assert!(matches!(h.state().doc().unwrap().doc.find(b).unwrap().kind, ObjKind::Path(_)));
    h.state_mut().run(Cmd::Undo);
    h.step();
    assert!(matches!(h.state().doc().unwrap().doc.find(b).unwrap().kind, ObjKind::Rect(_)), "one undo didn't restore the target");
    assert_eq!(count(&h), 3);
    // the toggle on the property bar turns keeping the source off
    let at = toggle(&h, "Leave original source object");
    click(&mut h, at);
    assert!(!h.state().ts.leave_source, "the property bar toggle didn't change the option");
    select(&mut h, vec![a, b]);
    h.state_mut().run(Cmd::Shaping(Shaping::Trim));
    h.step();
    assert!(area(&h, a).is_none(), "with the toggle off the source is used up");
    h.state_mut().run(Cmd::Undo);
    h.step();
    assert_eq!(count(&h), 3);
    // objects that don't overlap: nothing is deleted or changed
    select(&mut h, vec![far, b]);
    h.state_mut().run(Cmd::Shaping(Shaping::Trim));
    h.step();
    assert_eq!(count(&h), 3, "a Trim that does nothing deleted an object");
    assert_eq!(h.state().message.as_ref().map(|m| m.text.as_str()), Some(crate::exec::NO_OVERLAP), "no status message");
    assert!(matches!(h.state().doc().unwrap().doc.find(b).unwrap().kind, ObjKind::Rect(_)));
    assert!(matches!(h.state().doc().unwrap().doc.find(far).unwrap().kind, ObjKind::Rect(_)));
    let at = toggle(&h, "Leave original source object");
    click(&mut h, at);
    assert!(h.state().ts.leave_source);
    // Weld from the property bar still consumes its pieces, as in CorelDRAW
    select(&mut h, vec![a, b]);
    h.state_mut().run(Cmd::Shaping(Shaping::Weld));
    h.step();
    assert_eq!(count(&h), 2, "Weld left its original pieces behind");
    assert!(area(&h, a).is_none() && area(&h, b).is_none());
    h.state_mut().run(Cmd::Undo);
    h.step();
    assert_eq!(count(&h), 3, "one undo didn't bring the welded pieces back");
}

/// #97: an ellipse converted to curves is still a closed shape and is picked from inside when
/// Treat All Objects as Filled is on.
#[test]
fn converted_ellipse_is_picked_inside() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().settings.treat_as_filled = true;
    let at = {
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        let e = EllipseShape { w: 300.0, h: 200.0, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false };
        d.add_objects("test", vec![Object::new(id, ObjKind::Ellipse(e), Affine::translate((100.0, 100.0)), Style::default())]);
        let c = d.doc.find(id).unwrap().bounds().center();
        d.to_screen(c)
    };
    h.state_mut().run(Cmd::ConvertToCurves);
    h.step();
    let cid = h.state().doc().unwrap().selection[0];
    let o = h.state().doc().unwrap().doc.find(cid).unwrap().clone();
    assert!(matches!(o.kind, ObjKind::Path(_)) && o.is_closed_shape(), "the converted ellipse is an open curve");
    h.state_mut().doc_mut().unwrap().selection.clear();
    h.state_mut().set_tool(ToolId::Pick);
    click(&mut h, at);
    assert_eq!(h.state().doc().unwrap().selection, vec![cid]);
}

/// #99: double-clicking the Rectangle tool draws a rectangle the size of the page, in one undo step.
#[test]
fn double_click_rectangle_tool_makes_a_page_rectangle() {
    use crate::tools::ToolId;
    use egui_kittest::kittest::Queryable;
    let mut h = doc_harness();
    let page = h.state().doc().unwrap().page_ref().rect();
    let before = h.state().doc().unwrap().page_ref().all_objects().count();
    // the toolbox button (left edge), not a property bar or menu item with the same tooltip
    let at = h.query_all_by_label_contains("Rectangle tool").map(|n| n.rect().center()).min_by(|a, b| a.x.total_cmp(&b.x)).expect("no Rectangle tool button");
    let at = egui::pos2(at.x - 3.0, at.y - 3.0);
    move_to(&mut h, at);
    h.step();
    // a step is 0.25 s and a double-click must come within 0.3 s: one click per step
    for _ in 0..2 {
        press(&mut h, at, true);
        press(&mut h, at, false);
        h.step();
    }
    h.step();
    let a = h.state();
    assert_eq!(a.tool, ToolId::Rectangle);
    let d = a.doc().unwrap();
    assert_eq!(d.page_ref().all_objects().count(), before + 1);
    let o = d.doc.find(d.selection[0]).unwrap();
    assert!(matches!(o.kind, ObjKind::Rect(_)));
    let b = kurbo::Shape::bounding_box(&o.world_path());
    for (x, y) in [(b.x0, page.x0), (b.y0, page.y0), (b.x1, page.x1), (b.y1, page.y1)] {
        assert!((x - y).abs() < 0.01, "{b:?} is not the page {page:?}");
    }
    h.state_mut().run(Cmd::Undo);
    h.step();
    assert_eq!(h.state().doc().unwrap().page_ref().all_objects().count(), before);
}

/// #98: Break Apart is on for text with more than one line, word or character, and keeps it text.
#[test]
fn break_apart_keeps_text_editable() {
    use vector_core::text::{CharFormat, TextObj};
    let mut h = clip_harness();
    let on = |h: &Harness<'_, AnyVectorApp>| crate::ui::bind::enabled(h.state(), "d5669070-c0f3-4fc0-b524-e33aeea7925f");
    let d = h.state_mut().doc_mut().unwrap();
    let id = d.doc.new_id();
    d.add_objects("test", vec![Object::new(id, ObjKind::Text(TextObj::artistic("Hello World", CharFormat::default())), Affine::translate((50.0, 100.0)), Style::default())]);
    d.selection = vec![id];
    assert_eq!(on(&h), Some(true));
    h.state_mut().run(Cmd::BreakApart);
    h.step();
    let d = h.state().doc().unwrap();
    let words: Vec<String> = d
        .selection
        .iter()
        .map(|i| match &d.doc.find(*i).unwrap().kind {
            ObjKind::Text(t) => t.content.clone(),
            k => panic!("became {k:?}"),
        })
        .collect();
    assert_eq!(words, ["Hello", "World"]);
    let d = h.state_mut().doc_mut().unwrap();
    let id = d.doc.new_id();
    d.add_objects("test", vec![Object::new(id, ObjKind::Text(TextObj::artistic("A", CharFormat::default())), Affine::translate((50.0, 200.0)), Style::default())]);
    d.selection = vec![id];
    assert_eq!(on(&h), Some(false), "a single character can't be broken apart");
}

/// #95: a spot color applied from an open palette (an Adobe or CorelDRAW color book) keeps its
/// name and stays a spot in the document palette, so PDF export separates it.
#[test]
fn spot_color_from_a_palette_keeps_its_name() {
    use vector_core::color::{NamedColor, Palette};
    let mut h = clip_harness();
    let ink = Color::from_cmyk(0.0, 95.0, 100.0, 0.0);
    h.state_mut().palettes.palettes.push(Palette { name: "Book".into(), colors: vec![NamedColor { name: "PANTONE 485 C".into(), color: ink, spot: true }] });
    let (id, _) = unfilled_rect(&mut h);
    h.state_mut().doc_mut().unwrap().selection = vec![id];
    crate::exec::apply_color(h.state_mut(), Some(ink), false);
    let d = h.state().doc().unwrap();
    assert_eq!(d.doc.find(id).unwrap().style.fill, Fill::Solid(ink));
    let named: Vec<_> = d.doc.palette.iter().filter(|n| n.color == ink).collect();
    assert_eq!(named.len(), 1, "{:?}", d.doc.palette);
    assert!(named[0].spot && named[0].name == "PANTONE 485 C");
    // applying it again adds nothing
    crate::exec::apply_color(h.state_mut(), Some(ink), false);
    assert_eq!(h.state().doc().unwrap().doc.palette.iter().filter(|n| n.color == ink).count(), 1);
}

/// A selected 200 x 120 blue rectangle at (100, 100) in a new document.
fn blue_rect(h: &mut Harness<'_, AnyVectorApp>) -> vector_core::ObjId {
    let d = h.state_mut().doc_mut().unwrap();
    let mut st = Style::default();
    st.fill = Fill::Solid(Color::rgb(0x1e, 0x90, 0xff));
    let id = d.doc.new_id();
    d.add_objects("test", vec![Object::new(id, ObjKind::Rect(RectShape { w: 200.0, h: 120.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((100.0, 100.0)), st)]);
    d.selection = vec![id];
    id
}

/// Press at `a`, move to `b` in steps (calling `during` after each one), and release.
fn drag_watching(h: &mut Harness<'_, AnyVectorApp>, a: egui::Pos2, b: egui::Pos2, mut during: impl FnMut(&mut Harness<'_, AnyVectorApp>)) {
    move_to(h, a);
    h.step();
    press(h, a, true);
    h.step();
    for k in 1..=6 {
        move_to(h, a + (b - a) * (k as f32 / 6.0));
        h.step();
        during(h);
    }
    press(h, b, false);
    h.step();
}

/// Issue 107: the Drop Shadow bar's opacity and feathering are sliders that work: dragging the
/// strip under the value or the slider button beside it changes the shadow while dragging, as one
/// undo step; typing a value still works; with the Drop Shadow tool they also give a shape
/// without a shadow the default one.
#[test]
#[ignore]
fn visual_issue_107_shadow_sliders() {
    let mut h = doc_harness();
    let rect = blue_rect(&mut h);
    h.state_mut().set_tool(crate::tools::ToolId::DropShadow);
    for _ in 0..3 {
        h.step();
    }
    let field = |h: &Harness<'_, AnyVectorApp>, tip: &str| h.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::unique(("slider_field", tip.to_string())))).unwrap_or_else(|| panic!("the {tip} slider is on the bar"));
    let shadow = |h: &Harness<'_, AnyVectorApp>| h.state().doc().unwrap().doc.find(rect).unwrap().effects.drop_shadow.clone();
    let steps = |h: &Harness<'_, AnyVectorApp>| h.state().doc().unwrap().history.undo.len();
    assert!(shadow(&h).is_none());

    // the opacity strip: no shadow yet, so the drag starts from the default one (50) and the
    // shadow follows the pointer while dragging
    let fr = field(&h, "Shadow opacity");
    let track = crate::propbar::fields::slider_track(fr);
    let at = |t: f32| egui::pos2(track.left() + t * track.width(), fr.bottom() - 3.0);
    let n0 = steps(&h);
    let mut seen = vec![];
    drag_watching(&mut h, at(0.5), at(0.8), |h| seen.push(shadow(h).map(|s| s.opacity)));
    let s = shadow(&h).expect("dragging the slider made the default shadow");
    assert_eq!(s.opacity, 80.0);
    assert_eq!(s.feather, crate::tools::default_shadow(false).feather);
    assert!(seen.iter().flatten().any(|o| *o > 50.0 && *o < 80.0), "the shadow changed while dragging: {seen:?}");
    assert_eq!(steps(&h), n0 + 1, "one undo step for the drag");
    save(&mut h, "issue_107_01_opacity_dragged");

    // the feathering button scrubs: SCRUB_PX to the right is the whole range
    let fr = field(&h, "Shadow feathering");
    let button = egui::pos2(fr.right() + 9.0, fr.center().y);
    let f0 = shadow(&h).unwrap().feather;
    let dx = (crate::propbar::fields::SCRUB_PX * 0.3) as f32;
    let mut seen = vec![];
    drag_watching(&mut h, button, button + egui::vec2(dx, 0.0), |h| seen.push(shadow(h).unwrap().feather));
    assert_eq!(shadow(&h).unwrap().feather, f0 + 30.0);
    assert!(seen.iter().any(|f| *f > f0 && *f < f0 + 30.0), "it changed while dragging: {seen:?}");
    assert_eq!(steps(&h), n0 + 2, "one more undo step");
    save(&mut h, "issue_107_02_feather_scrubbed");

    // a typed value still applies on Enter
    click(&mut h, egui::pos2(fr.center().x, fr.top() + 8.0));
    h.input_mut().events.push(egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.step();
    h.input_mut().events.push(egui::Event::Text("12".into()));
    h.step();
    key(&mut h, egui::Key::Enter, true);
    h.step();
    key(&mut h, egui::Key::Enter, false);
    for _ in 0..2 {
        h.step();
    }
    assert_eq!(shadow(&h).unwrap().feather, 12.0);
    assert_eq!(steps(&h), n0 + 3);

    // undo takes back one whole change at a time
    h.state_mut().run(Cmd::Undo);
    h.step();
    assert_eq!(shadow(&h).unwrap().feather, f0 + 30.0);
    h.state_mut().run(Cmd::Undo);
    h.step();
    assert_eq!(shadow(&h).unwrap().feather, f0);
    assert_eq!(shadow(&h).unwrap().opacity, 80.0);
    h.state_mut().run(Cmd::Undo);
    h.step();
    assert!(shadow(&h).is_none(), "the first drag made the shadow");
    save(&mut h, "issue_107_03_undone");
}

/// Issue 126: the rotation center snaps to the selected shape's own corners, and a moved center
/// goes along to the copies made by Duplicate and Paste (and comes back with Undo); Group makes a
/// new object, which turns around its middle again.
#[test]
#[ignore]
fn visual_issue_126_rotation_center_kept_and_snapped() {
    let mut h = doc_harness();
    let rect = blue_rect(&mut h);
    let to_screen = |h: &Harness<'_, AnyVectorApp>, x: f64, y: f64| {
        let p = h.state().doc().unwrap().to_screen(Point::new(x, y));
        egui::pos2(p.x as f32, p.y as f32)
    };
    h.state_mut().set_tool(crate::tools::ToolId::Pick);
    h.state_mut().doc_mut().unwrap().rotate_mode = true;
    h.step();

    // dragged near the rectangle's own top left corner, the center snaps onto it (and says so)
    let corner = Point::new(100.0, 100.0);
    let (a, b) = (to_screen(&h, 200.0, 160.0), to_screen(&h, 100.0, 100.0) + egui::vec2(5.0, 4.0));
    let mut mark = None;
    drag_watching(&mut h, a, b, |h| mark = h.state().ts.snap_mark);
    let d = h.state().doc().unwrap();
    assert_eq!(d.rotation_center(), Some(corner), "snapped onto its own corner");
    assert_eq!(mark, Some((corner, "Corner")), "the snap shows");
    save(&mut h, "issue_126_01_center_snapped_to_corner");
    // and its own midpoints too
    let (a, b) = (to_screen(&h, 100.0, 100.0), to_screen(&h, 300.0, 160.0) + egui::vec2(-4.0, 3.0));
    drag(&mut h, a, b);
    assert_eq!(h.state().doc().unwrap().rotation_center(), Some(Point::new(300.0, 160.0)));
    let (a, b) = (to_screen(&h, 300.0, 160.0), to_screen(&h, 100.0, 100.0) + egui::vec2(3.0, 3.0));
    drag(&mut h, a, b);
    assert_eq!(h.state().doc().unwrap().rotation_center(), Some(corner));

    // Duplicate: the copy keeps the center, moved with it
    let off = h.state().doc().unwrap().doc.settings.duplicate_offset;
    h.state_mut().run(Cmd::Duplicate);
    h.step();
    let d = h.state().doc().unwrap();
    let copy = d.selection.clone();
    assert_ne!(copy, vec![rect]);
    assert_eq!(d.rotation_center(), Some(corner + off));
    // it turns around there
    let before = d.doc.find(copy[0]).unwrap().transform;
    h.state_mut().run(Cmd::Rotate(30.0));
    h.step();
    let after = h.state().doc().unwrap().doc.find(copy[0]).unwrap().transform;
    let c = corner + off;
    assert!(((after * before.inverse() * c) - c).hypot() < 1e-6, "the copy turned around its moved center");
    save(&mut h, "issue_126_02_duplicate_turned_around_center");

    // Undo brings back the original and its center
    h.state_mut().run(Cmd::Undo);
    h.state_mut().run(Cmd::Undo);
    h.step();
    let d = h.state().doc().unwrap();
    assert_eq!(d.selection, vec![rect]);
    assert_eq!(d.rotation_center(), Some(corner));

    // Copy and Paste: the pasted copy has the center at the same place
    h.state_mut().run(Cmd::Copy);
    h.state_mut().run(Cmd::Paste);
    h.step();
    let d = h.state().doc().unwrap();
    assert_ne!(d.selection, vec![rect]);
    let bb = d.selection_bounds().unwrap();
    assert!((d.rotation_center().unwrap() - Point::new(bb.x0, bb.y0)).hypot() < 1e-6, "{:?} in {bb:?}", d.rotation_center());

    // Group makes a new object: it turns around its middle
    h.state_mut().doc_mut().unwrap().selection = vec![rect];
    h.state_mut().doc_mut().unwrap().rot_center = Some((vec![rect], corner));
    h.state_mut().run(Cmd::Group);
    h.step();
    let d = h.state().doc().unwrap();
    assert_eq!(d.rotation_center(), Some(d.selection_bounds().unwrap().center()));
    save(&mut h, "issue_126_03_group_turns_around_middle");
}

// ---- Shape tool: polygon and ellipse nodes, segments, + and - (#104, #108, #122, #123) ----

/// One frame of pointer input for the Shape tool (with a double-click flag and modifiers).
fn shape_in(h: &mut Harness<'_, AnyVectorApp>, pos: egui::Pos2, pressed: bool, down: bool, released: bool, double: bool, mods: egui::Modifiers) {
    let inp = crate::tools::Input { pos, hovered: true, pressed, released, down, double, secondary: false, secondary_down: false, secondary_released: false, mods, key_c: false, time: 100.0, dt: 0.016 };
    crate::tools::handle_input(h.state_mut(), &inp);
}

/// Drag with the Shape tool between document points.
fn shape_drag(h: &mut Harness<'_, AnyVectorApp>, a: (f64, f64), b: (f64, f64), mods: egui::Modifiers) {
    let (sa, sb) = (scr(h, a), scr(h, b));
    shape_in(h, sa, true, true, false, false, mods);
    for k in 1..=8 {
        shape_in(h, sa + (sb - sa) * (k as f32 / 8.0), false, true, false, false, mods);
    }
    shape_in(h, sb, false, false, true, false, mods);
}

/// A selected object on a Shape-tool document (snapping off).
fn shape_object<'a>(kind: ObjKind, at: (f64, f64)) -> (Harness<'a, AnyVectorApp>, u64) {
    let mut h = join_harness(crate::tools::ToolId::Shape);
    let d = h.state_mut().doc_mut().unwrap();
    let id = d.doc.new_id();
    let mut st = Style::default();
    st.outline.color = Some(Color::BLACK);
    d.add_objects("test", vec![Object::new(id, kind, Affine::translate(at), st)]);
    d.selection = vec![id];
    (h, id)
}

fn kind_of(h: &Harness<'_, AnyVectorApp>, id: u64) -> ObjKind {
    h.state().doc().unwrap().doc.find(id).unwrap().kind.clone()
}

fn undo_steps(h: &Harness<'_, AnyVectorApp>) -> usize {
    h.state().doc().unwrap().history.undo_labels().len()
}

fn undo(h: &mut Harness<'_, AnyVectorApp>) {
    h.state_mut().run(Cmd::Undo);
    h.step();
}

/// An open straight curve through the points.
fn line_curve(pts: &[(f64, f64)]) -> ObjKind {
    ObjKind::Path(vector_core::path::polyline(&pts.iter().map(|p| Point::new(p.0, p.1)).collect::<Vec<_>>(), false))
}

/// #104: dragging a polygon node with the Shape tool moves all its matching nodes, as in
/// CorelDRAW; the object stays a polygon; one undo step; a plain click changes nothing.
#[test]
fn shape_tool_drags_polygon_nodes_symmetrically() {
    let pg = PolygonShape { w: 100.0, h: 100.0, sides: 5, kind: PolygonKind::Polygon, sharpness: 0.0, nodes: Default::default() };
    let (mut h, id) = shape_object(ObjKind::Polygon(pg.clone()), (100.0, 100.0));
    let steps = undo_steps(&h);
    // a click on the top corner: nothing changes, no undo step
    tool_click(&mut h, (150.0, 100.0), egui::Vec2::ZERO);
    assert_eq!(kind_of(&h, id), ObjKind::Polygon(pg.clone()));
    assert_eq!(undo_steps(&h), steps);
    // dragged 20 up: every corner moves out the same way
    shape_drag(&mut h, (150.0, 100.0), (150.0, 80.0), egui::Modifiers::NONE);
    let ObjKind::Polygon(p) = kind_of(&h, id) else { panic!("no longer a polygon") };
    assert_eq!(p.sides, 5);
    assert!((p.nodes.outer_scale - 1.4).abs() < 0.02 && p.nodes.outer_turn.abs() < 1.0, "{:?}", p.nodes);
    assert!(p.nodes.inner_scale == 1.0 && p.nodes.inner_turn == 0.0);
    let c = Point::new(150.0, 150.0);
    for i in 0..5 {
        let q = Affine::translate((100.0, 100.0)) * p.node_point(false, i);
        assert!(((q - c).hypot() - 70.0).abs() < 1.0, "corner {i} at {q:?}");
    }
    // the middle of a side: the inner nodes follow it
    let mid = Affine::translate((100.0, 100.0)) * p.node_point(true, 0);
    shape_drag(&mut h, (mid.x, mid.y), (150.0 + (mid.x - 150.0) * 0.5, 150.0 + (mid.y - 150.0) * 0.5), egui::Modifiers::NONE);
    let ObjKind::Polygon(p2) = kind_of(&h, id) else { panic!() };
    assert!((p2.nodes.inner_scale - 0.5).abs() < 0.02, "{:?}", p2.nodes);
    assert_eq!(p2.nodes.outer_scale, p.nodes.outer_scale);
    // one undo step per drag
    assert_eq!(undo_steps(&h), steps + 2);
    undo(&mut h);
    assert_eq!(kind_of(&h, id), ObjKind::Polygon(p));
    undo(&mut h);
    assert_eq!(kind_of(&h, id), ObjKind::Polygon(pg));
}

/// #108: the ellipse node dragged inside makes a pie, outside an arc; it stays an ellipse; a pie
/// has two nodes; one undo step per drag.
#[test]
fn shape_tool_drags_ellipse_node_to_pie_and_arc() {
    let e = EllipseShape { w: 200.0, h: 100.0, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false };
    let (mut h, id) = shape_object(ObjKind::Ellipse(e.clone()), (100.0, 100.0));
    // the node is at the top; dragged clockwise, inside
    shape_drag(&mut h, (200.0, 100.0), (250.0, 120.0), egui::Modifiers::NONE);
    let ObjKind::Ellipse(pie) = kind_of(&h, id) else { panic!("not an ellipse any more") };
    assert_eq!(pie.kind, EllipseKind::Pie);
    assert_eq!(pie.start, 90.0);
    assert!(pie.end > 0.0 && pie.end < 90.0, "{pie:?}");
    assert_eq!(pie.nodes().len(), 2);
    // the end node dragged outside: an arc, the start stays
    let end = Affine::translate((100.0, 100.0)) * pie.angle_point(pie.end);
    shape_drag(&mut h, (end.x, end.y), (200.0, 230.0), egui::Modifiers::NONE);
    let ObjKind::Ellipse(arc) = kind_of(&h, id) else { panic!() };
    assert_eq!(arc.kind, EllipseKind::Arc);
    assert_eq!(arc.start, 90.0);
    assert!((arc.end - 270.0).abs() < 0.5, "{arc:?}");
    undo(&mut h);
    assert_eq!(kind_of(&h, id), ObjKind::Ellipse(pie));
    undo(&mut h);
    assert_eq!(kind_of(&h, id), ObjKind::Ellipse(e));
}

/// #122, #123: clicking a segment selects it (no node selected); arrows move it; To Curve, `+` and
/// Delete act on it; dragging a line segment moves it, dragging a curved one bends it.
#[test]
fn shape_tool_selects_drags_and_edits_segments() {
    use crate::tools::NodeOp;
    let (mut h, id) = shape_object(line_curve(&[(100.0, 100.0), (200.0, 100.0), (300.0, 100.0), (400.0, 100.0)]), (0.0, 0.0));
    let np = |h: &Harness<'_, AnyVectorApp>| match kind_of(h, id) {
        ObjKind::Path(np) => np,
        k => panic!("{k:?}"),
    };
    tool_click(&mut h, (150.0, 100.0), egui::Vec2::ZERO);
    let d = h.state().doc().unwrap();
    assert_eq!(d.seg_sel, Some((id, 0, 0)));
    assert!(d.node_sel.is_empty());
    assert_eq!(crate::tools::selected_segment(d), Some((id, 0, 0)));
    // arrow nudge moves both its nodes, one undo step
    h.state_mut().run(Cmd::Nudge(0.0, 10.0));
    h.step();
    let n = np(&h);
    assert!(near(n.subpaths[0].nodes[0].p, (100.0, 110.0)) && near(n.subpaths[0].nodes[1].p, (200.0, 110.0)) && near(n.subpaths[0].nodes[2].p, (300.0, 100.0)), "{:?}", n.subpaths[0].nodes);
    undo(&mut h);
    // dragging the line segment moves it
    tool_click(&mut h, (250.0, 100.0), egui::Vec2::ZERO);
    assert_eq!(h.state().doc().unwrap().seg_sel, Some((id, 0, 1)));
    shape_drag(&mut h, (250.0, 100.0), (250.0, 130.0), egui::Modifiers::NONE);
    let n = np(&h);
    assert!(near(n.subpaths[0].nodes[1].p, (200.0, 130.0)) && near(n.subpaths[0].nodes[2].p, (300.0, 130.0)), "{:?}", n.subpaths[0].nodes);
    undo(&mut h);
    // To Curve on the selected segment, then dragging it bends it: the nodes stay
    tool_click(&mut h, (250.0, 100.0), egui::Vec2::ZERO);
    h.state_mut().run(Cmd::Node(NodeOp::ToCurve));
    h.step();
    assert!(!np(&h).subpaths[0].nodes[1].line_out, "To Curve did not act on the segment");
    assert_eq!(h.state().doc().unwrap().seg_sel, Some((id, 0, 1)), "the segment stays selected");
    shape_drag(&mut h, (250.0, 100.0), (250.0, 140.0), egui::Modifiers::NONE);
    let n = np(&h);
    assert!(near(n.subpaths[0].nodes[1].p, (200.0, 100.0)) && near(n.subpaths[0].nodes[2].p, (300.0, 100.0)));
    let mid = kurbo::ParamCurve::eval(&n.subpaths[0].segment(1).unwrap().0, 0.5);
    assert!((mid.y - 140.0).abs() < 1.0, "the curve does not follow the pointer: {mid:?}");
    // with the curved segment selected, both its control points show
    let c = crate::tools::visible_controls(h.state().doc().unwrap());
    assert!(c.contains(&(id, 0, 1, false)) && c.contains(&(id, 0, 2, true)), "{c:?}");
    // + on the segment: one node in its middle, selected
    h.key_press(egui::Key::Plus);
    h.step();
    h.step();
    assert_eq!(np(&h).node_count(), 5);
    let d = h.state().doc().unwrap();
    assert_eq!(d.node_sel, vec![(id, 0, 2)]);
    assert!(d.seg_sel.is_none());
    // - deletes the selected node
    h.key_press(egui::Key::Minus);
    h.step();
    h.step();
    assert_eq!(np(&h).node_count(), 4);
    // Delete on a segment removes it: the curve splits in two
    tool_click(&mut h, (150.0, 100.0), egui::Vec2::ZERO);
    assert_eq!(h.state().doc().unwrap().seg_sel, Some((id, 0, 0)));
    h.state_mut().run(Cmd::Delete);
    h.step();
    let n = np(&h);
    assert_eq!(n.subpaths.len(), 1, "{n:?}");
    assert!(near(n.subpaths[0].nodes[0].p, (200.0, 100.0)));
}

/// #123: `+` (with or without Shift, as on a US keyboard; egui reports the keypad + as the same
/// key) adds one node between two selected neighbouring nodes, and again after the new node.
#[test]
fn plus_adds_a_node_between_two_selected_nodes() {
    let (mut h, id) = shape_object(line_curve(&[(100.0, 100.0), (200.0, 100.0), (300.0, 100.0), (400.0, 100.0)]), (0.0, 0.0));
    h.state_mut().doc_mut().unwrap().node_sel = vec![(id, 0, 1), (id, 0, 2)];
    h.key_press(egui::Key::Plus);
    h.step();
    h.step();
    let nodes = |h: &Harness<'_, AnyVectorApp>| match kind_of(h, id) {
        ObjKind::Path(np) => np.subpaths[0].nodes.iter().map(|n| n.p).collect::<Vec<_>>(),
        _ => unreachable!(),
    };
    let n = nodes(&h);
    assert_eq!(n.len(), 5, "{n:?}");
    assert!(near(n[2], (250.0, 100.0)));
    assert_eq!(h.state().doc().unwrap().node_sel, vec![(id, 0, 2)]);
    // Shift held (+ is Shift+= on a US keyboard): adds again, after the new node
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Plus);
    h.step();
    h.step();
    let n = nodes(&h);
    assert_eq!(n.len(), 6, "{n:?}");
    assert!(near(n[3], (275.0, 100.0)));
    // one undo step each
    undo(&mut h);
    assert_eq!(nodes(&h).len(), 5);
}

/// #122: double-clicking a node deletes it (one undo step); a two-node curve keeps its nodes;
/// double-clicking a segment still adds a node.
#[test]
fn double_click_on_a_node_deletes_it() {
    let count = |h: &Harness<'_, AnyVectorApp>, id: u64| match kind_of(h, id) {
        ObjKind::Path(np) => np.node_count(),
        _ => 0,
    };
    let dbl = |h: &mut Harness<'_, AnyVectorApp>, p: (f64, f64)| {
        let s = scr(h, p);
        tool_click(h, p, egui::Vec2::ZERO);
        shape_in(h, s, true, true, false, false, egui::Modifiers::NONE);
        shape_in(h, s, false, false, true, true, egui::Modifiers::NONE);
    };
    let (mut h, id) = shape_object(line_curve(&[(100.0, 100.0), (200.0, 100.0), (300.0, 100.0)]), (0.0, 0.0));
    dbl(&mut h, (200.0, 100.0));
    assert_eq!(count(&h, id), 2, "the node was not deleted");
    undo(&mut h);
    assert_eq!(count(&h, id), 3);
    // a segment: a node is added there
    dbl(&mut h, (150.0, 100.0));
    assert_eq!(count(&h, id), 4);
    // a curve of two nodes keeps them
    let (mut h, id) = shape_object(line_curve(&[(100.0, 100.0), (200.0, 100.0)]), (0.0, 0.0));
    dbl(&mut h, (100.0, 100.0));
    assert_eq!(count(&h, id), 2);
}

/// #122: selecting either node of a curved segment shows (and lets you drag) the control points
/// at both of its ends, as CorelDRAW does.
#[test]
fn curved_segment_shows_both_handles_from_either_node() {
    use crate::tools::NodeOp;
    let (mut h, id) = shape_object(line_curve(&[(100.0, 100.0), (250.0, 100.0), (400.0, 100.0)]), (0.0, 0.0));
    h.state_mut().doc_mut().unwrap().node_sel = vec![(id, 0, 0)];
    h.state_mut().run(Cmd::Node(NodeOp::ToCurve));
    h.step();
    for sel in [(id, 0, 0), (id, 0, 1)] {
        h.state_mut().doc_mut().unwrap().node_sel = vec![sel];
        let c = crate::tools::visible_controls(h.state().doc().unwrap());
        assert!(c.contains(&(id, 0, 0, false)) && c.contains(&(id, 0, 1, true)), "node {sel:?}: {c:?}");
        // the straight segment from node 1 shows none
        assert!(!c.contains(&(id, 0, 1, false)), "{c:?}");
    }
    // with node 0 selected, the far control point (node 1's) can be dragged
    h.state_mut().doc_mut().unwrap().node_sel = vec![(id, 0, 0)];
    let far = match kind_of(&h, id) {
        ObjKind::Path(np) => np.subpaths[0].nodes[1].c_in,
        _ => unreachable!(),
    };
    shape_drag(&mut h, (far.x, far.y), (far.x, far.y + 40.0), egui::Modifiers::NONE);
    let ObjKind::Path(np) = kind_of(&h, id) else { unreachable!() };
    assert!((np.subpaths[0].nodes[1].c_in.y - (far.y + 40.0)).abs() < 0.5, "{:?}", np.subpaths[0].nodes[1]);
    assert!(near(np.subpaths[0].nodes[1].p, (250.0, 100.0)));
}

/// #122: the Shape tool's cursor is not Pick's: a crosshair, a hand over what it can grab.
#[test]
fn shape_tool_cursor_differs_from_pick() {
    use egui::CursorIcon;
    let (mut h, _) = shape_object(line_curve(&[(100.0, 100.0), (200.0, 100.0), (300.0, 100.0)]), (0.0, 0.0));
    h.state_mut().settings.crosshair_cursor = false;
    let at = |h: &Harness<'_, AnyVectorApp>, p: (f64, f64)| crate::canvas::cursor_for(h.state(), scr(h, p));
    assert_eq!(at(&h, (150.0, 300.0)), CursorIcon::Crosshair);
    assert_eq!(at(&h, (200.0, 100.0)), CursorIcon::PointingHand, "over a node");
    assert_eq!(at(&h, (150.0, 101.0)), CursorIcon::PointingHand, "over a segment");
    h.state_mut().set_tool(crate::tools::ToolId::Pick);
    assert_ne!(at(&h, (150.0, 300.0)), CursorIcon::Crosshair);
}

/// What the Shape tool shows on a polygon, an ellipse turned into a pie, and a curve with a
/// curved segment selected. Images go to `target/visual/shape_tool_*.png`.
#[test]
#[ignore]
fn visual_shape_tool_nodes() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().doc_mut().unwrap().doc.settings.snap_off = true;
    let mut st = Style::default();
    st.fill = Fill::Solid(Color::rgb(0xff, 0xd0, 0x20));
    st.outline.color = Some(Color::BLACK);
    st.outline.width = 1.0;
    let (pid, eid, cid) = {
        let d = h.state_mut().doc_mut().unwrap();
        let ids = (d.doc.new_id(), d.doc.new_id(), d.doc.new_id());
        let mut np = vector_core::path::polyline(&[Point::new(450.0, 150.0), Point::new(550.0, 100.0), Point::new(650.0, 150.0)], false);
        np.set_segment_line((0, 0), false);
        d.add_objects(
            "test",
            vec![
                Object::new(ids.0, ObjKind::Polygon(PolygonShape { w: 150.0, h: 150.0, sides: 6, kind: PolygonKind::Polygon, sharpness: 0.0, nodes: Default::default() }), Affine::translate((60.0, 80.0)), st.clone()),
                Object::new(ids.1, ObjKind::Ellipse(EllipseShape { w: 160.0, h: 120.0, kind: EllipseKind::Ellipse, start: 90.0, end: 90.0, clockwise: false }), Affine::translate((260.0, 90.0)), st.clone()),
                Object::new(ids.2, ObjKind::Path(np), Affine::IDENTITY, Style::default()),
            ],
        );
        ids
    };
    h.state_mut().set_tool(ToolId::Shape);
    h.state_mut().doc_mut().unwrap().selection = vec![pid, eid, cid];
    save(&mut h, "shape_tool_01_nodes");
    // the polygon's top corner dragged out and turned, the ellipse node dragged into a pie
    shape_drag(&mut h, (135.0, 80.0), (160.0, 50.0), egui::Modifiers::NONE);
    shape_drag(&mut h, (340.0, 90.0), (380.0, 130.0), egui::Modifiers::NONE);
    // the curved segment clicked: it and both its control points show
    let mid = match kind_of(&h, cid) {
        ObjKind::Path(np) => kurbo::ParamCurve::eval(&np.subpaths[0].segment(0).unwrap().0, 0.5),
        _ => unreachable!(),
    };
    tool_click(&mut h, (mid.x, mid.y), egui::Vec2::ZERO);
    save(&mut h, "shape_tool_02_edited");
    assert!(matches!(kind_of(&h, pid), ObjKind::Polygon(p) if !p.nodes.is_regular()));
    assert!(matches!(kind_of(&h, eid), ObjKind::Ellipse(e) if e.kind == EllipseKind::Pie));
    assert_eq!(h.state().doc().unwrap().seg_sel, Some((cid, 0, 0)));
}

// ---- 0.6.0: nudging segments (#142), Freehand with Ctrl (#134), Rectangle around the selection
// (#135), text spacing handles (#110) ----

/// The point at `t` of segment `seg` of the first subpath of curve `id`, in document space.
fn seg_point(h: &Harness<'_, AnyVectorApp>, id: u64, seg: usize, t: f64) -> Point {
    let o = h.state().doc().unwrap().doc.find(id).unwrap();
    let ObjKind::Path(np) = &o.kind else { panic!("{:?}", o.kind) };
    o.transform * kurbo::ParamCurve::eval(&np.subpaths[0].segment(seg).unwrap().0, t)
}

fn last_undo(h: &Harness<'_, AnyVectorApp>) -> String {
    h.state().doc().unwrap().history.undo_labels().first().cloned().unwrap_or_default()
}

/// Press an arrow key: the nudge shortcut.
fn arrow(h: &mut Harness<'_, AnyVectorApp>, k: egui::Key) {
    h.key_press(k);
    h.step();
    h.step();
}

/// #142: the arrow keys on a selected segment, as in CorelDRAW: a curved segment bends, its
/// middle moving by the nudge while its nodes and the segments beside it stay; a straight one
/// moves with its nodes; one undo step per press, and the presses add up; selected nodes still
/// move as before.
#[test]
fn nudging_a_curved_segment_bends_it() {
    use crate::tools::NodeOp;
    let (mut h, id) = shape_object(line_curve(&[(100.0, 100.0), (200.0, 100.0), (300.0, 100.0), (400.0, 100.0)]), (0.0, 0.0));
    let np = |h: &Harness<'_, AnyVectorApp>| match kind_of(h, id) {
        ObjKind::Path(np) => np,
        k => panic!("{k:?}"),
    };
    let nudge = h.state().doc().unwrap().doc.settings.nudge;
    // the middle segment made a curve; it stays selected
    tool_click(&mut h, (250.0, 100.0), egui::Vec2::ZERO);
    h.state_mut().run(Cmd::Node(NodeOp::ToCurve));
    h.step();
    assert_eq!(h.state().doc().unwrap().seg_sel, Some((id, 0, 1)));
    let before = np(&h);
    let steps = undo_steps(&h);
    let mid0 = seg_point(&h, id, 1, 0.5);
    arrow(&mut h, egui::Key::ArrowUp);
    let n = np(&h);
    let nodes = &n.subpaths[0].nodes;
    for (k, p) in [(100.0, 100.0), (200.0, 100.0), (300.0, 100.0), (400.0, 100.0)].into_iter().enumerate() {
        assert!(near(nodes[k].p, p), "node {k} moved: {nodes:?}");
    }
    let mid = seg_point(&h, id, 1, 0.5);
    assert!((mid - (mid0 + kurbo::Vec2::new(0.0, -nudge))).hypot() < 1e-6, "the middle of the curve did not go up by the nudge: {mid:?}");
    assert_ne!(nodes[1].c_out, before.subpaths[0].nodes[1].c_out);
    assert_ne!(nodes[2].c_in, before.subpaths[0].nodes[2].c_in);
    // the segments beside it keep their nodes and stay straight
    assert_eq!(nodes[0], before.subpaths[0].nodes[0]);
    assert_eq!(nodes[3], before.subpaths[0].nodes[3]);
    assert!(nodes[0].line_out && nodes[2].line_out && !nodes[1].line_out);
    assert_eq!(undo_steps(&h), steps + 1);
    assert_eq!(last_undo(&h), "Edit Curve");
    assert_eq!(h.state().doc().unwrap().seg_sel, Some((id, 0, 1)), "the segment stays selected");
    // the presses add up, in any direction
    arrow(&mut h, egui::Key::ArrowUp);
    arrow(&mut h, egui::Key::ArrowRight);
    let mid = seg_point(&h, id, 1, 0.5);
    assert!((mid - (mid0 + kurbo::Vec2::new(nudge, -2.0 * nudge))).hypot() < 1e-6, "{mid:?}");
    assert_eq!(undo_steps(&h), steps + 3);
    for _ in 0..3 {
        undo(&mut h);
    }
    assert_eq!(np(&h), before, "undo did not bring back the control points");
    // a straight segment moves with both its nodes
    tool_click(&mut h, (150.0, 100.0), egui::Vec2::ZERO);
    assert_eq!(h.state().doc().unwrap().seg_sel, Some((id, 0, 0)));
    h.state_mut().run(Cmd::Nudge(0.0, -10.0));
    h.step();
    let n = np(&h);
    assert!(near(n.subpaths[0].nodes[0].p, (100.0, 90.0)) && near(n.subpaths[0].nodes[1].p, (200.0, 90.0)) && near(n.subpaths[0].nodes[2].p, (300.0, 100.0)), "{:?}", n.subpaths[0].nodes);
    assert_eq!(last_undo(&h), "Move Segment");
    undo(&mut h);
    // selected nodes: only they move, even next to the curved segment
    h.state_mut().doc_mut().unwrap().node_sel = vec![(id, 0, 2)];
    h.state_mut().run(Cmd::Nudge(10.0, 0.0));
    h.step();
    let n = np(&h);
    assert!(near(n.subpaths[0].nodes[2].p, (310.0, 100.0)) && near(n.subpaths[0].nodes[1].p, (200.0, 100.0)) && near(n.subpaths[0].nodes[3].p, (400.0, 100.0)));
    assert_eq!(last_undo(&h), "Move Nodes");
}

/// #142: a curve of one segment bends too, and the nudge goes the way the arrow points on the
/// page whatever the curve's own rotation.
#[test]
fn nudging_a_segment_of_a_rotated_curve() {
    use crate::tools::NodeOp;
    let (mut h, id) = shape_object(line_curve(&[(0.0, 0.0), (100.0, 0.0)]), (0.0, 0.0));
    h.state_mut().doc_mut().unwrap().doc.find_mut(id).unwrap().transform = Affine::translate((200.0, 200.0)) * Affine::rotate(std::f64::consts::FRAC_PI_2);
    // on the page it runs from (200, 200) down to (200, 300)
    tool_click(&mut h, (200.0, 250.0), egui::Vec2::ZERO);
    assert_eq!(h.state().doc().unwrap().seg_sel, Some((id, 0, 0)));
    h.state_mut().run(Cmd::Node(NodeOp::ToCurve));
    h.step();
    let mid0 = seg_point(&h, id, 0, 0.5);
    h.state_mut().run(Cmd::Nudge(10.0, 0.0));
    h.step();
    assert!((seg_point(&h, id, 0, 0.5) - (mid0 + kurbo::Vec2::new(10.0, 0.0))).hypot() < 1e-6, "{:?}", seg_point(&h, id, 0, 0.5));
    assert!((seg_point(&h, id, 0, 0.0) - Point::new(200.0, 200.0)).hypot() < 1e-6 && (seg_point(&h, id, 0, 1.0) - Point::new(200.0, 300.0)).hypot() < 1e-6, "the nodes moved");
}

/// #142: what a nudged curve segment looks like. Images go to `target/visual/issue_142_*.png`.
#[test]
#[ignore]
fn visual_issue_142_nudge_bends_a_curve_segment() {
    use crate::tools::{NodeOp, ToolId};
    let mut h = doc_harness();
    h.state_mut().doc_mut().unwrap().doc.settings.snap_off = true;
    let id = {
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        let mut st = Style::default();
        st.outline.color = Some(Color::BLACK);
        st.outline.width = 2.0;
        let np = vector_core::path::polyline(&[Point::new(150.0, 300.0), Point::new(300.0, 200.0), Point::new(500.0, 200.0), Point::new(650.0, 300.0)], false);
        d.add_objects("test", vec![Object::new(id, ObjKind::Path(np), Affine::IDENTITY, st)]);
        d.selection = vec![id];
        id
    };
    h.state_mut().set_tool(ToolId::Shape);
    tool_click(&mut h, (400.0, 200.0), egui::Vec2::ZERO);
    h.state_mut().run(Cmd::Node(NodeOp::ToCurve));
    h.step();
    save(&mut h, "issue_142_01_curve_segment_selected");
    for _ in 0..8 {
        arrow(&mut h, egui::Key::ArrowUp);
    }
    save(&mut h, "issue_142_02_nudged_up_8_times");
    let ObjKind::Path(np) = kind_of(&h, id) else { unreachable!() };
    assert!(near(np.subpaths[0].nodes[1].p, (300.0, 200.0)) && near(np.subpaths[0].nodes[2].p, (500.0, 200.0)), "the nodes moved");
    assert!(seg_point(&h, id, 1, 0.5).y < 200.0 - 50.0);
}

/// A drag through document points with the active tool, `mods` held while dragging and
/// `release` at the release.
fn stroke_with(h: &mut Harness<'_, AnyVectorApp>, pts: &[(f64, f64)], mods: egui::Modifiers, release: egui::Modifiers) {
    let s: Vec<egui::Pos2> = pts.iter().map(|p| scr(h, *p)).collect();
    shape_in(h, s[0], true, true, false, false, mods);
    for w in s.windows(2) {
        for k in 1..=8 {
            shape_in(h, w[0] + (w[1] - w[0]) * (k as f32 / 8.0), false, true, false, false, mods);
        }
    }
    shape_in(h, *s.last().unwrap(), false, false, true, false, release);
}

/// A shaky stroke from `o` that ends at `deg` degrees, 100 points away.
fn shaky(o: (f64, f64), deg: f64) -> Vec<(f64, f64)> {
    let at = |a: f64, r: f64| (o.0 + r * a.to_radians().cos(), o.1 + r * a.to_radians().sin());
    vec![o, at(deg + 12.0, 30.0), at(deg - 9.0, 55.0), at(deg + 6.0, 80.0), at(deg, 100.0)]
}

/// #134: Freehand, dragged with Ctrl held: a straight line at the nearest multiple of the
/// constrain angle (Options ▸ Freehand/Bézier tool), as the click-click line; one undo step;
/// without Ctrl at the release, or without Ctrl at all, the stroke stays freehand.
#[test]
fn freehand_ctrl_drag_draws_a_constrained_line() {
    use crate::tools::ToolId;
    let (ctrl, none) = (egui::Modifiers::CTRL, egui::Modifiers::NONE);
    let mut h = join_harness(ToolId::Freehand);
    assert_eq!(h.state().settings.constrain_angle, 15.0);
    let end = |o: (f64, f64), deg: f64| (o.0 + 100.0 * deg.to_radians().cos(), o.1 + 100.0 * deg.to_radians().sin());
    let last = |h: &Harness<'_, AnyVectorApp>| curves(h).last().unwrap().subpaths[0].clone();
    // 40° with 15° steps: 45°
    let steps = undo_steps(&h);
    stroke_with(&mut h, &shaky((100.0, 100.0), 40.0), ctrl, ctrl);
    assert_eq!(curves(&h).len(), 1);
    let sp = last(&h);
    assert_eq!(sp.nodes.len(), 2, "not a straight line: {:?}", sp.nodes);
    assert!(sp.nodes[0].line_out && !sp.closed);
    assert!(near(sp.nodes[0].p, (100.0, 100.0)) && near(sp.nodes[1].p, end((100.0, 100.0), 45.0)), "{:?}", sp.nodes);
    assert_eq!(undo_steps(&h), steps + 1);
    undo(&mut h);
    assert!(curves(&h).is_empty());
    // the constrain angle sets the steps: 30° makes it 30°
    h.state_mut().settings.constrain_angle = 30.0;
    stroke_with(&mut h, &shaky((100.0, 400.0), 40.0), ctrl, ctrl);
    let sp = last(&h);
    assert!(sp.nodes.len() == 2 && near(sp.nodes[1].p, end((100.0, 400.0), 30.0)), "{:?}", sp.nodes);
    h.state_mut().settings.constrain_angle = 15.0;
    // Ctrl let go before the release: the stroke as drawn
    stroke_with(&mut h, &shaky((500.0, 100.0), 40.0), ctrl, none);
    let sp = last(&h);
    assert!(near(sp.nodes.last().unwrap().p, end((500.0, 100.0), 40.0)), "{:?}", sp.nodes);
    // no Ctrl: freehand as before
    stroke_with(&mut h, &shaky((500.0, 400.0), 40.0), none, none);
    let sp = last(&h);
    assert!(near(sp.nodes.last().unwrap().p, end((500.0, 400.0), 40.0)), "{:?}", sp.nodes);
    assert_eq!(curves(&h).len(), 3);
    // click-click with Ctrl still keeps to the constrain angle
    let (a, b) = (scr(&h, (800.0, 100.0)), scr(&h, end((800.0, 100.0), 40.0)));
    for p in [a, b] {
        shape_in(&mut h, p, true, true, false, false, ctrl);
        shape_in(&mut h, p, false, false, true, false, ctrl);
    }
    let sp = last(&h);
    assert!(sp.nodes.len() == 2 && near(sp.nodes[1].p, end((800.0, 100.0), 45.0)), "{:?}", sp.nodes);
}

/// #134: a Ctrl+drag continuing the selected curve from its end node adds a straight segment to it.
#[test]
fn freehand_ctrl_drag_continues_the_curve() {
    use crate::tools::ToolId;
    let ctrl = egui::Modifiers::CTRL;
    let mut h = join_harness(ToolId::Freehand);
    stroke_with(&mut h, &shaky((100.0, 100.0), 0.0), ctrl, ctrl);
    assert_eq!(curves(&h).len(), 1);
    // from 2 pixels off its end: within the Auto-join distance
    let s = h.state().doc().unwrap().to_doc(scr(&h, (200.0, 100.0)) + egui::vec2(2.0, 1.0));
    let stroke = shaky((s.x, s.y), 88.0);
    stroke_with(&mut h, &stroke, ctrl, ctrl);
    let cs = curves(&h);
    assert_eq!(cs.len(), 1, "the line became a separate object");
    let nodes = &cs[0].subpaths[0].nodes;
    assert_eq!(nodes.len(), 3, "{nodes:?}");
    // from the end node, straight down (88° rounds to 90°), as long as the drag
    let e = stroke[stroke.len() - 1];
    let len = (Point::new(e.0, e.1) - Point::new(200.0, 100.0)).hypot();
    assert!(near(nodes[1].p, (200.0, 100.0)) && near(nodes[2].p, (200.0, 100.0 + len)), "{nodes:?}");
}

/// #134: the preview of a Ctrl+drag with Freehand is the straight line it makes, and the line
/// after one click follows the constrain angle too. Images go to `target/visual/issue_134_*.png`.
#[test]
#[ignore]
fn visual_issue_134_freehand_ctrl_constrains() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().doc_mut().unwrap().doc.settings.snap_off = true;
    h.state_mut().set_tool(ToolId::Freehand);
    h.step();
    let ctrl = egui::Modifiers::CTRL;
    let pts: Vec<egui::Pos2> = shaky((150.0, 150.0), 40.0).into_iter().map(|p| scr(&h, (p.0 * 2.0 - 150.0, p.1 * 2.0 - 150.0))).collect();
    h.input_mut().events.push(egui::Event::ModifiersChanged(ctrl));
    move_to(&mut h, pts[0]);
    h.step();
    h.input_mut().events.push(egui::Event::PointerButton { pos: pts[0], button: egui::PointerButton::Primary, pressed: true, modifiers: ctrl });
    h.step();
    for w in pts.windows(2) {
        for k in 1..=4 {
            move_to(&mut h, w[0] + (w[1] - w[0]) * (k as f32 / 4.0));
            h.step();
        }
    }
    assert!(matches!(h.state().ts.drag, crate::tools::Drag::Stroke { .. }), "not drawing");
    save(&mut h, "issue_134_01_ctrl_drag_preview");
    h.input_mut().events.push(egui::Event::PointerButton { pos: *pts.last().unwrap(), button: egui::PointerButton::Primary, pressed: false, modifiers: ctrl });
    h.step();
    h.input_mut().events.push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    let cs = curves(&h);
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].subpaths[0].nodes.len(), 2, "{:?}", cs[0].subpaths[0].nodes);
    let (a, b) = (cs[0].subpaths[0].nodes[0].p, cs[0].subpaths[0].nodes[1].p);
    assert!(((b.y - a.y).atan2(b.x - a.x).to_degrees() - 45.0).abs() < 1e-6);
    save(&mut h, "issue_134_02_ctrl_drag_line");
    // one click, then the pointer away with Ctrl: the line to it keeps to 15° steps
    h.state_mut().doc_mut().unwrap().selection.clear();
    let a = scr(&h, (150.0, 600.0));
    click(&mut h, a);
    h.input_mut().events.push(egui::Event::ModifiersChanged(ctrl));
    let b = scr(&h, (400.0, 520.0));
    move_to(&mut h, b);
    h.step();
    assert!(h.state().ts.line_start.is_some());
    save(&mut h, "issue_134_03_click_line_preview");
    h.input_mut().events.push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.state_mut().ts.cancel();
}

/// Double-click the Rectangle tool in the toolbox.
fn double_click_rectangle_tool(h: &mut Harness<'_, AnyVectorApp>) {
    use egui_kittest::kittest::Queryable;
    // the toolbox button (left edge), not a property bar or menu item with the same tooltip
    let at = h.query_all_by_label_contains("Rectangle tool").map(|n| n.rect().center()).min_by(|a, b| a.x.total_cmp(&b.x)).expect("no Rectangle tool button");
    let at = egui::pos2(at.x - 3.0, at.y - 3.0);
    move_to(h, at);
    h.step();
    // a step is 0.25 s and a double-click must come within 0.3 s: one click per step
    for _ in 0..2 {
        press(h, at, true);
        press(h, at, false);
        h.step();
    }
    h.step();
}

/// A rectangle and a turned ellipse, selected; returns their ids.
fn two_shapes(h: &mut Harness<'_, AnyVectorApp>) -> (u64, u64) {
    let d = h.state_mut().doc_mut().unwrap();
    let (a, b) = (d.doc.new_id(), d.doc.new_id());
    let st = Style { fill: Fill::Solid(Color::rgb(0x1e, 0x90, 0xff)), ..Style::default() };
    let e = EllipseShape { w: 160.0, h: 80.0, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false };
    d.add_objects(
        "test",
        vec![
            Object::new(a, ObjKind::Rect(RectShape { w: 200.0, h: 120.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((100.0, 100.0)), st.clone()),
            Object::new(b, ObjKind::Ellipse(e), Affine::translate((420.0, 260.0)) * Affine::rotate(30f64.to_radians()), st),
        ],
    );
    d.selection = vec![a, b];
    (a, b)
}

/// #135: with objects selected, double-clicking the Rectangle tool draws a rectangle around them
/// (the selection's bounding box, axis-aligned also for a turned object), selected in their
/// place, in one undo step; the objects stay as they were.
#[test]
fn double_click_rectangle_tool_wraps_the_selection() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    let (a, b) = two_shapes(&mut h);
    h.step();
    let (bb, before, oa, ob) = {
        let d = h.state().doc().unwrap();
        (d.selection_bounds().unwrap(), d.page_ref().all_objects().count(), d.doc.find(a).unwrap().clone(), d.doc.find(b).unwrap().clone())
    };
    let steps = undo_steps(&h);
    double_click_rectangle_tool(&mut h);
    let s = h.state();
    assert_eq!(s.tool, ToolId::Rectangle);
    let d = s.doc().unwrap();
    assert_eq!(d.page_ref().all_objects().count(), before + 1);
    assert_eq!(d.selection.len(), 1);
    assert!(d.selection[0] != a && d.selection[0] != b);
    let o = d.doc.find(d.selection[0]).unwrap();
    assert!(matches!(o.kind, ObjKind::Rect(_)));
    let r = kurbo::Shape::bounding_box(&o.world_path());
    for (x, y) in [(r.x0, bb.x0), (r.y0, bb.y0), (r.x1, bb.x1), (r.y1, bb.y1)] {
        assert!((x - y).abs() < 0.01, "{r:?} is not around the selection {bb:?}");
    }
    assert_eq!(d.doc.find(a).unwrap(), &oa);
    assert_eq!(d.doc.find(b).unwrap(), &ob);
    assert_eq!(undo_steps(&h), steps + 1);
    undo(&mut h);
    assert_eq!(h.state().doc().unwrap().page_ref().all_objects().count(), before);
    // a single object: around it
    h.state_mut().doc_mut().unwrap().selection = vec![b];
    crate::tools::rectangle_to_bounds(h.state_mut());
    let d = h.state().doc().unwrap();
    let r = kurbo::Shape::bounding_box(&d.doc.find(d.selection[0]).unwrap().world_path());
    let eb = d.doc.find(b).unwrap().bounds();
    assert!((r.x0 - eb.x0).abs() < 0.01 && (r.y1 - eb.y1).abs() < 0.01 && (r.width() - eb.width()).abs() < 0.01, "{r:?} vs {eb:?}");
    // nothing selected: the page, as before (#99)
    h.state_mut().doc_mut().unwrap().selection.clear();
    crate::tools::rectangle_to_bounds(h.state_mut());
    let d = h.state().doc().unwrap();
    let page = d.page_ref().rect();
    let r = kurbo::Shape::bounding_box(&d.doc.find(d.selection[0]).unwrap().world_path());
    assert!((r.x0 - page.x0).abs() < 0.01 && (r.y1 - page.y1).abs() < 0.01 && (r.width() - page.width()).abs() < 0.01, "{r:?} vs the page {page:?}");
}

/// #135: the rectangle drawn around two selected shapes by double-clicking the Rectangle tool.
/// Images go to `target/visual/issue_135_*.png`.
#[test]
#[ignore]
fn visual_issue_135_rectangle_around_the_selection() {
    let mut h = doc_harness();
    two_shapes(&mut h);
    h.state_mut().set_tool(crate::tools::ToolId::Pick);
    save(&mut h, "issue_135_01_selection");
    double_click_rectangle_tool(&mut h);
    let d = h.state().doc().unwrap();
    assert!(matches!(d.doc.find(d.selection[0]).unwrap().kind, ObjKind::Rect(_)));
    save(&mut h, "issue_135_02_rectangle_around_it");
}

/// A selected artistic text on a Shape-tool document.
fn spacing_text<'a>(s: &str, tr: Affine) -> (Harness<'a, AnyVectorApp>, u64) {
    use vector_core::text::{CharFormat, TextObj};
    let (mut h, id) = shape_object(ObjKind::Text(TextObj::artistic(s, CharFormat::default())), (0.0, 0.0));
    h.state_mut().doc_mut().unwrap().doc.find_mut(id).unwrap().transform = tr;
    h.state_mut().settings.crosshair_cursor = false;
    (h, id)
}

fn text_of(h: &Harness<'_, AnyVectorApp>, id: u64) -> vector_core::text::TextObj {
    match kind_of(h, id) {
        ObjKind::Text(t) => t,
        k => panic!("{k:?}"),
    }
}

/// The text's spacing handles: (horizontal, vertical), in document points.
fn spacing_handles(h: &Harness<'_, AnyVectorApp>, id: u64) -> Option<((f64, f64), (f64, f64))> {
    let d = h.state().doc().unwrap();
    crate::tools::text_spacing_handles(d, d.doc.find(id).unwrap()).map(|[(a, _), (b, _)]| ((a.x, a.y), (b.x, b.y)))
}

/// The width of the widest line and the baselines of a text.
fn text_lines(t: &vector_core::text::TextObj) -> (f64, Vec<f64>) {
    let l = vector_core::text::layout(t);
    (l.lines.iter().map(|x| x.x1 - x.x0).fold(0.0, f64::max), l.lines.iter().map(|x| x.baseline).collect())
}

/// #110: the Shape tool shows CorelDRAW's spacing handles on a selected text: dragging the one
/// past the lower right corner sets the character spacing (Shift: the word spacing), the widest
/// line following the pointer; the one below the lower left corner the line spacing (Shift: the
/// space after paragraphs), the last line following the pointer. One undo step per drag, named as
/// the Text docker's field; the docker's values are the text's.
#[test]
fn shape_tool_text_spacing_handles() {
    use egui::CursorIcon;
    let shift = egui::Modifiers::SHIFT;
    let none = egui::Modifiers::NONE;
    let (mut h, id) = spacing_text("Spacing handles\nsecond line", Affine::translate((100.0, 200.0)));
    let t0 = text_of(&h, id);
    let (w0, b0) = text_lines(&t0);
    assert_eq!(b0.len(), 2);
    let (hh, vh) = spacing_handles(&h, id).expect("no spacing handles on the text");
    let bb = h.state().doc().unwrap().doc.find(id).unwrap().bounds();
    assert!(hh.0 > bb.x1 && hh.1 > bb.center().y && hh.1 < vh.1, "{hh:?} {vh:?} {bb:?}");
    assert!(vh.1 > bb.y1 && (vh.0 - bb.x0).abs() < 3.0, "{vh:?} {bb:?}");
    let cursor = |h: &Harness<'_, AnyVectorApp>, p: (f64, f64)| crate::canvas::cursor_for(h.state(), scr(h, p));
    assert_eq!(cursor(&h, hh), CursorIcon::ResizeHorizontal);
    assert_eq!(cursor(&h, vh), CursorIcon::ResizeVertical);
    let steps = undo_steps(&h);
    // the horizontal handle 30 to the right: the character spacing grows
    shape_drag(&mut h, hh, (hh.0 + 30.0, hh.1 + 4.0), none);
    let t1 = text_of(&h, id);
    let (w1, _) = text_lines(&t1);
    assert!(t1.format.tracking > 0.0, "{}", t1.format.tracking);
    assert!((w1 - w0 - 30.0).abs() < 1.0, "the widest line grew {} for a drag of 30", w1 - w0);
    assert_eq!((t1.line_spacing, t1.word_spacing, t1.para_spacing), (100.0, 100.0, 0.0));
    assert_eq!(undo_steps(&h), steps + 1);
    assert_eq!(last_undo(&h), "Character Spacing");
    assert_eq!(crate::tools::TextSpacing::Char.value(&t1), t1.format.tracking);
    // the handle went along with the text
    let (hh1, vh1) = spacing_handles(&h, id).unwrap();
    assert!((hh1.0 - hh.0 - 30.0).abs() < 1.0, "{hh1:?}");
    // the vertical handle 20 down: the line spacing grows, the second line goes 20 down
    shape_drag(&mut h, vh1, (vh1.0 - 5.0, vh1.1 + 20.0), none);
    let t2 = text_of(&h, id);
    let (_, b2) = text_lines(&t2);
    assert!(t2.line_spacing > 100.0);
    assert!(((b2[1] - b2[0]) - (b0[1] - b0[0]) - 20.0).abs() < 0.5, "the second line moved {}", (b2[1] - b2[0]) - (b0[1] - b0[0]));
    assert_eq!(t2.format.tracking, t1.format.tracking);
    assert_eq!(last_undo(&h), "Line Spacing");
    // Shift: the word spacing
    let (hh2, vh2) = spacing_handles(&h, id).unwrap();
    shape_drag(&mut h, hh2, (hh2.0 + 20.0, hh2.1), shift);
    let t3 = text_of(&h, id);
    assert!(t3.word_spacing > 100.0 && t3.format.tracking == t2.format.tracking, "{} {}", t3.word_spacing, t3.format.tracking);
    assert!((text_lines(&t3).0 - text_lines(&t2).0 - 20.0).abs() < 1.5, "{}", text_lines(&t3).0 - text_lines(&t2).0);
    assert_eq!(last_undo(&h), "Word Spacing");
    // Shift: the space after paragraphs
    shape_drag(&mut h, vh2, (vh2.0, vh2.1 + 10.0), shift);
    let t4 = text_of(&h, id);
    let (_, b4) = text_lines(&t4);
    assert!(t4.para_spacing > 0.0 && t4.line_spacing == t2.line_spacing);
    assert!(((b4[1] - b4[0]) - (b2[1] - b2[0]) - 10.0).abs() < 0.5, "{}", (b4[1] - b4[0]) - (b2[1] - b2[0]));
    assert_eq!(last_undo(&h), "After Paragraph Spacing");
    // far to the left: no less than the Text docker allows
    let (hh4, _) = spacing_handles(&h, id).unwrap();
    shape_drag(&mut h, hh4, (hh4.0 - 3000.0, hh4.1), none);
    assert_eq!(text_of(&h, id).format.tracking, -100.0);
    // a click on a handle changes nothing
    let steps5 = undo_steps(&h);
    let (_, vh5) = spacing_handles(&h, id).unwrap();
    tool_click(&mut h, vh5, egui::Vec2::ZERO);
    assert_eq!(undo_steps(&h), steps5);
    assert_eq!(undo_steps(&h), steps + 5);
    for _ in 0..5 {
        undo(&mut h);
    }
    assert_eq!(text_of(&h, id), t0, "undo did not bring the spacing back");
}

/// #110: the spacing handles of a turned text work along its own lines; paragraph text has them
/// at its frame; the character spacing of every run changes alike; there are none on text being
/// edited, on a path or vertical.
#[test]
fn text_spacing_handles_follow_the_text() {
    use vector_core::text::{CharFormat, Span, TextKind};
    // turned a quarter: its lines run down the page
    let (mut h, id) = spacing_text("Turned", Affine::translate((300.0, 100.0)) * Affine::rotate(std::f64::consts::FRAC_PI_2));
    let t0 = text_of(&h, id);
    let (hh, vh) = spacing_handles(&h, id).unwrap();
    assert!(hh.1 > vh.1 && (hh.0 - 300.0).abs() < (vh.0 - 300.0).abs() + 1.0, "{hh:?} {vh:?}");
    shape_drag(&mut h, hh, (hh.0, hh.1 + 24.0), egui::Modifiers::NONE);
    let t1 = text_of(&h, id);
    assert!((text_lines(&t1).0 - text_lines(&t0).0 - 24.0).abs() < 1.0, "{}", text_lines(&t1).0 - text_lines(&t0).0);
    // runs of their own sizes and spacing: all change by the same amount
    let (mut h, id) = spacing_text("Big and small", Affine::translate((100.0, 300.0)));
    if let ObjKind::Text(t) = &mut h.state_mut().doc_mut().unwrap().doc.find_mut(id).unwrap().kind {
        t.spans = vec![Span { start: 0, end: 3, format: CharFormat { size: 48.0, tracking: 20.0, ..CharFormat::default() } }];
    }
    let t0 = text_of(&h, id);
    let (hh, _) = spacing_handles(&h, id).unwrap();
    shape_drag(&mut h, hh, (hh.0 + 40.0, hh.1), egui::Modifiers::NONE);
    let t1 = text_of(&h, id);
    let d = t1.format.tracking;
    assert!(d > 0.0 && (t1.spans[0].format.tracking - 20.0 - d).abs() < 0.11, "{} {}", d, t1.spans[0].format.tracking);
    assert!((text_lines(&t1).0 - text_lines(&t0).0 - 40.0).abs() < 1.0, "{}", text_lines(&t1).0 - text_lines(&t0).0);
    // paragraph text: at the lower corners of its frame
    if let ObjKind::Text(t) = &mut h.state_mut().doc_mut().unwrap().doc.find_mut(id).unwrap().kind {
        t.kind = TextKind::Paragraph { width: 300.0, height: 200.0 };
    }
    let gap = h.state().doc().unwrap().doc_len(12.0);
    let (hh, vh) = spacing_handles(&h, id).unwrap();
    assert!(near(Point::new(hh.0, hh.1), (400.0 + gap, 500.0)) && near(Point::new(vh.0, vh.1), (100.0, 500.0 + gap)), "{hh:?} {vh:?}");
    // none while the text is edited, on a path, or vertical
    h.state_mut().doc_mut().unwrap().text_edit = Some(crate::docview::TextEdit { obj: id, caret: 0, anchor: 0, blink: 0.0 });
    assert!(spacing_handles(&h, id).is_none());
    h.state_mut().doc_mut().unwrap().text_edit = None;
    if let ObjKind::Text(t) = &mut h.state_mut().doc_mut().unwrap().doc.find_mut(id).unwrap().kind {
        t.vertical = true;
    }
    assert!(spacing_handles(&h, id).is_none());
    if let ObjKind::Text(t) = &mut h.state_mut().doc_mut().unwrap().doc.find_mut(id).unwrap().kind {
        t.vertical = false;
        t.kind = TextKind::Artistic;
        t.on_path = Some(vector_core::text::TextOnPath { path: vector_core::path::polyline(&[Point::new(0.0, 0.0), Point::new(300.0, 0.0)], false), offset: 0.0, distance: 0.0, flip: false });
    }
    assert!(spacing_handles(&h, id).is_none());
    let p = scr(&h, (100.0, 300.0));
    assert!(!matches!(crate::tools::shape_hit(h.state().doc().unwrap(), h.state().doc().unwrap().to_doc(p), 5.0), Some(crate::tools::ShapeHit::TextSpacing { .. })));
}

/// #110: the spacing handles on a text with the Shape tool, and the text after dragging each.
/// Images go to `target/visual/issue_110_*.png`.
#[test]
#[ignore]
fn visual_issue_110_text_spacing_handles() {
    use vector_core::text::{CharFormat, TextObj};
    let mut h = doc_harness();
    h.state_mut().doc_mut().unwrap().doc.settings.snap_off = true;
    let id = {
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        let st = Style { fill: Fill::Solid(Color::BLACK), ..Style::default() };
        let t = TextObj::artistic("Character spacing\nand line spacing", CharFormat { size: 40.0, ..CharFormat::default() });
        d.add_objects("test", vec![Object::new(id, ObjKind::Text(t), Affine::translate((150.0, 250.0)), st)]);
        d.selection = vec![id];
        id
    };
    h.state_mut().set_tool(crate::tools::ToolId::Shape);
    save(&mut h, "issue_110_01_handles");
    let (hh, _) = spacing_handles(&h, id).unwrap();
    shape_drag(&mut h, hh, (hh.0 + 80.0, hh.1), egui::Modifiers::NONE);
    save(&mut h, "issue_110_02_character_spacing");
    let (_, vh) = spacing_handles(&h, id).unwrap();
    shape_drag(&mut h, vh, (vh.0, vh.1 + 40.0), egui::Modifiers::NONE);
    save(&mut h, "issue_110_03_line_spacing");
    let t = text_of(&h, id);
    assert!(t.format.tracking > 0.0 && t.line_spacing > 100.0);
}

// ---- PowerClip: Alt+click selects the contents, Ctrl+click enters, the floating PowerClip toolbar (#111, #112) ----

/// A PowerClip on a Pick-tool document: an ellipse frame (100,100)–(400,300) holding a red
/// rectangle (150,150)–(230,250) and a blue one (260,170)–(560,230) that runs past the frame.
/// Gives (PowerClip, red, blue, frame).
fn powerclip_doc<'a>() -> (Harness<'a, AnyVectorApp>, [u64; 4]) {
    let mut h = join_harness(crate::tools::ToolId::Pick);
    let ids = add_powerclip(&mut h);
    (h, ids)
}

fn add_powerclip(h: &mut Harness<'_, AnyVectorApp>) -> [u64; 4] {
    let d = h.state_mut().doc_mut().unwrap();
    let solid = |c: Color| Style { fill: Fill::Solid(c), ..Style::default() };
    let r = |w: f64, h: f64| ObjKind::Rect(RectShape { w, h, radii: [0.0; 4], corner: CornerType::default() });
    let (fid, aid, bid, pid) = (d.doc.new_id(), d.doc.new_id(), d.doc.new_id(), d.doc.new_id());
    let mut frame_style = Style::default();
    frame_style.outline.color = Some(Color::BLACK);
    let frame = Object::new(fid, ObjKind::Ellipse(EllipseShape { w: 300.0, h: 200.0, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false }), Affine::translate((100.0, 100.0)), frame_style);
    let a = Object::new(aid, r(80.0, 100.0), Affine::translate((150.0, 150.0)), solid(Color::rgb(220, 40, 40)));
    let b = Object::new(bid, r(300.0, 60.0), Affine::translate((260.0, 170.0)), solid(Color::rgb(40, 90, 220)));
    let mut pc = Object::new(pid, ObjKind::PowerClip { frame: Box::new(frame), contents: vec![a, b] }, Affine::IDENTITY, Style::default());
    pc.name = "PowerClip".into();
    d.add_objects("test", vec![pc]);
    d.selection.clear();
    [pid, aid, bid, fid]
}

/// A Pick tool click at a document point, with modifiers, fed straight to the tool.
fn pick_click(h: &mut Harness<'_, AnyVectorApp>, at: (f64, f64), mods: egui::Modifiers) {
    let s = scr(h, at);
    shape_in(h, s, true, true, false, false, mods);
    shape_in(h, s, false, false, true, false, mods);
    // the commands the click queued (Finish) run on the next frame
    h.step();
}

/// Run a command (commands run on the next frame).
fn run_cmd(h: &mut Harness<'_, AnyVectorApp>, c: Cmd) {
    h.state_mut().run(c);
    h.step();
}

/// A Pick tool double-click at a document point.
fn pick_double(h: &mut Harness<'_, AnyVectorApp>, at: (f64, f64)) {
    pick_click(h, at, egui::Modifiers::NONE);
    let s = scr(h, at);
    shape_in(h, s, true, true, false, false, egui::Modifiers::NONE);
    shape_in(h, s, false, false, true, true, egui::Modifiers::NONE);
    h.step();
}

fn sel(h: &Harness<'_, AnyVectorApp>) -> Vec<u64> {
    h.state().doc().unwrap().selection.clone()
}

fn editing(h: &Harness<'_, AnyVectorApp>) -> Option<u64> {
    h.state().doc().unwrap().editing_powerclip()
}

fn x0(h: &Harness<'_, AnyVectorApp>, id: u64) -> f64 {
    h.state().doc().unwrap().doc.find(id).unwrap().bounds().x0
}

const ALT: egui::Modifiers = egui::Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: false, command: false };
const CTRL: egui::Modifiers = egui::Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };

/// #111: a click selects the PowerClip; Alt+click selects the contents under the pointer without
/// entering the PowerClip, and they then drag on their own; Ctrl+click (and a double-click)
/// enters the PowerClip, where clicks and the marquee pick the contents, shown in full; Ctrl+click
/// off the contents, a double-click off them or Esc finishes and selects the PowerClip again.
#[test]
fn powerclip_alt_click_selects_contents_and_ctrl_click_enters() {
    let (mut h, [pc, a, b, frame]) = powerclip_doc();
    let none = egui::Modifiers::NONE;
    // a plain click picks the PowerClip
    pick_click(&mut h, (190.0, 200.0), none);
    assert_eq!(sel(&h), vec![pc]);
    // Alt+click: the content under the pointer, the PowerClip stays closed
    pick_click(&mut h, (190.0, 200.0), ALT);
    assert_eq!(sel(&h), vec![a], "Alt+click didn't select the contents");
    assert_eq!(editing(&h), None, "Alt+click entered the PowerClip");
    // with that content selected, Alt+click goes on down the stack (#139), and back to it
    pick_click(&mut h, (190.0, 200.0), ALT);
    assert_ne!(sel(&h), vec![a], "a second Alt+click stayed on the content");
    pick_click(&mut h, (190.0, 200.0), ALT);
    assert_eq!(sel(&h), vec![a], "Alt+click didn't come back to the contents");
    // Shift+Alt adds another content
    pick_click(&mut h, (300.0, 200.0), egui::Modifiers { shift: true, ..ALT });
    assert_eq!(sel(&h), vec![a, b]);
    pick_click(&mut h, (700.0, 500.0), none);
    pick_click(&mut h, (190.0, 200.0), ALT);
    // a content selected that way drags on its own (no Alt needed), as one undo step
    let (fx, bx) = (x0(&h, frame), x0(&h, b));
    let ax = x0(&h, a);
    shape_drag(&mut h, (190.0, 200.0), (210.0, 220.0), none);
    assert_eq!(sel(&h), vec![a]);
    assert!((x0(&h, a) - ax - 20.0).abs() < 0.01, "the content didn't move");
    assert_eq!((x0(&h, frame), x0(&h, b)), (fx, bx), "the frame or the other content moved");
    assert_eq!(h.state().doc().unwrap().doc.parent_of(a), Some(pc), "the content left the PowerClip");
    run_cmd(&mut h, Cmd::Undo);
    assert!((x0(&h, a) - ax).abs() < 0.01);
    // a click on the selected content keeps it (a second click is for rotating, as anywhere);
    // a click on the PowerClip off that content picks the PowerClip again
    pick_click(&mut h, (190.0, 200.0), none);
    assert_eq!(sel(&h), vec![a]);
    pick_click(&mut h, (250.0, 120.0), none);
    assert_eq!(sel(&h), vec![pc]);

    // Ctrl+click enters the PowerClip, the content clicked selected
    pick_click(&mut h, (300.0, 200.0), CTRL);
    assert_eq!(editing(&h), Some(pc), "Ctrl+click didn't enter the PowerClip");
    assert_eq!(sel(&h), vec![b]);
    // inside, a click picks a content and a drag moves only it
    pick_click(&mut h, (190.0, 200.0), none);
    assert_eq!(sel(&h), vec![a]);
    shape_drag(&mut h, (190.0, 200.0), (200.0, 200.0), none);
    assert!((x0(&h, a) - ax - 10.0).abs() < 0.01);
    assert_eq!(x0(&h, frame), fx);
    run_cmd(&mut h, Cmd::Undo);
    // the contents show in full: the part past the frame picks too
    pick_click(&mut h, (500.0, 200.0), none);
    assert_eq!(sel(&h), vec![b], "the contents past the frame can't be picked while editing");
    // empty space inside the frame: nothing selected, still editing
    pick_click(&mut h, (250.0, 120.0), none);
    assert!(sel(&h).is_empty());
    assert_eq!(editing(&h), Some(pc), "a click off the contents left the PowerClip");
    // the marquee selects among the contents
    shape_drag(&mut h, (140.0, 140.0), (240.0, 260.0), none);
    assert_eq!(sel(&h), vec![a]);
    // Select All: all the contents
    run_cmd(&mut h, Cmd::SelectAll);
    assert_eq!(sel(&h), vec![a, b]);
    // Ctrl+click off the contents finishes: the PowerClip is selected again
    pick_click(&mut h, (700.0, 500.0), CTRL);
    assert_eq!(editing(&h), None, "Ctrl+click outside didn't finish");
    assert_eq!(sel(&h), vec![pc]);

    // a double-click enters too
    pick_double(&mut h, (190.0, 200.0));
    assert_eq!(editing(&h), Some(pc), "a double-click didn't enter the PowerClip");
    assert_eq!(sel(&h), vec![a]);
    // and a double-click off the contents finishes
    pick_double(&mut h, (700.0, 500.0));
    assert_eq!(editing(&h), None);
    assert_eq!(sel(&h), vec![pc]);

    // Edit PowerClip, then Esc: the first deselects, the second finishes
    run_cmd(&mut h, Cmd::PowerClipEdit);
    assert_eq!(editing(&h), Some(pc));
    assert!(sel(&h).is_empty());
    pick_click(&mut h, (190.0, 200.0), none);
    assert_eq!(sel(&h), vec![a]);
    h.key_press(egui::Key::Escape);
    h.step();
    assert!(sel(&h).is_empty());
    assert_eq!(editing(&h), Some(pc), "Esc with a content selected left the PowerClip");
    h.key_press(egui::Key::Escape);
    h.step();
    assert_eq!(editing(&h), None, "Esc didn't finish editing");
    assert_eq!(sel(&h), vec![pc]);

    // with the Windows modifier keys Ctrl is Duplicate: a Ctrl+click doesn't enter
    h.state_mut().settings.windows_modifiers = true;
    pick_click(&mut h, (190.0, 200.0), CTRL);
    assert_eq!(editing(&h), None);
    assert_eq!(sel(&h), vec![pc]);
    h.state_mut().settings.windows_modifiers = false;
    // Alt+click off any PowerClip keeps its own meaning (nothing PowerClip-specific happens)
    pick_click(&mut h, (700.0, 500.0), ALT);
    assert!(sel(&h).is_empty());
}

/// #111 with a PowerClip inside the contents of another: Alt+click reaches the deepest content;
/// each Ctrl+click enters one level and each Finish goes back out one level.
#[test]
fn powerclip_nested_levels() {
    let mut h = join_harness(crate::tools::ToolId::Pick);
    let [outer, inner, deep] = {
        let d = h.state_mut().doc_mut().unwrap();
        let r = |id: u64, at: (f64, f64), w: f64, hh: f64, c: Color| {
            Object::new(id, ObjKind::Rect(RectShape { w, h: hh, radii: [0.0; 4], corner: CornerType::default() }), Affine::translate(at), Style { fill: Fill::Solid(c), ..Style::default() })
        };
        let ids: Vec<u64> = (0..5).map(|_| d.doc.new_id()).collect();
        let deep = r(ids[0], (160.0, 160.0), 60.0, 60.0, Color::rgb(220, 40, 40));
        let inner_frame = r(ids[1], (150.0, 150.0), 200.0, 100.0, Color::rgb(240, 240, 160));
        let inner = Object::new(ids[2], ObjKind::PowerClip { frame: Box::new(inner_frame), contents: vec![deep] }, Affine::IDENTITY, Style::default());
        let outer_frame = r(ids[3], (100.0, 100.0), 400.0, 300.0, Color::rgb(200, 220, 240));
        let outer = Object::new(ids[4], ObjKind::PowerClip { frame: Box::new(outer_frame), contents: vec![inner] }, Affine::IDENTITY, Style::default());
        d.add_objects("test", vec![outer]);
        [ids[4], ids[2], ids[0]]
    };
    pick_click(&mut h, (180.0, 180.0), ALT);
    assert_eq!(sel(&h), vec![deep], "Alt+click didn't dig to the deepest content");
    pick_click(&mut h, (300.0, 200.0), egui::Modifiers::NONE);
    assert_eq!(sel(&h), vec![outer]);
    pick_click(&mut h, (300.0, 200.0), CTRL);
    assert_eq!((editing(&h), sel(&h)), (Some(outer), vec![inner]), "Ctrl+click enters one level");
    pick_click(&mut h, (180.0, 180.0), CTRL);
    assert_eq!((editing(&h), sel(&h)), (Some(inner), vec![deep]), "Ctrl+click on a nested PowerClip enters it");
    run_cmd(&mut h, Cmd::PowerClipFinish);
    assert_eq!((editing(&h), sel(&h)), (Some(outer), vec![inner]), "Finish goes back to the PowerClip around");
    run_cmd(&mut h, Cmd::PowerClipFinish);
    assert_eq!((editing(&h), sel(&h)), (None, vec![outer]));
    // Extract of the nested PowerClip while editing the outer one keeps its contents inside
    run_cmd(&mut h, Cmd::PowerClipEdit);
    h.state_mut().doc_mut().unwrap().selection = vec![inner];
    run_cmd(&mut h, Cmd::PowerClipExtract);
    assert_eq!(sel(&h), vec![deep]);
    assert_eq!(h.state().doc().unwrap().doc.parent_of(deep), Some(outer));
}

/// The center of the widget with this accessible label.
fn labeled(h: &Harness<'_, AnyVectorApp>, label: &str) -> Option<egui::Pos2> {
    use egui_kittest::kittest::Queryable;
    h.query_by_label(label).map(|n| n.rect().center())
}

fn press_labeled(h: &mut Harness<'_, AnyVectorApp>, label: &str) {
    let at = labeled(h, label).unwrap_or_else(|| panic!("no '{label}' on screen"));
    click(h, at);
    h.step();
}

/// #112: a selected PowerClip shows the floating PowerClip toolbar under it (the property bar
/// stays the frame's, as in CorelDRAW); its Edit, Select Contents, Extract, Lock and placement
/// buttons work, and while editing it offers Finish (as does the Finish button of the drawing).
#[test]
fn powerclip_toolbar_shows_and_its_buttons_work() {
    let (mut h, [pc, a, b, frame]) = powerclip_doc();
    for _ in 0..2 {
        h.step();
    }
    assert!(labeled(&h, "Select Contents").is_none(), "the toolbar shows with nothing selected");
    h.state_mut().doc_mut().unwrap().selection = vec![pc];
    h.step();
    h.step();
    // under the PowerClip
    let target = crate::powerclip::target_rect(h.state().doc().unwrap()).unwrap();
    let at = labeled(&h, "Select Contents").expect("no floating PowerClip toolbar");
    assert!(at.y > target.bottom() && (at.x - target.center().x).abs() < 120.0, "the toolbar isn't under the PowerClip: {at:?} {target:?}");
    for l in ["Edit PowerClip...", "Extract Contents", "Lock Contents to PowerClip", crate::powerclip::ADJUST_CONTENTS] {
        assert!(labeled(&h, l).is_some(), "no '{l}' on the toolbar");
    }
    // the property bar is still the frame's (the ellipse bar), as in CorelDRAW
    let bar = crate::propbar::bar_for(h.state()).and_then(crate::ui::bar).unwrap();
    assert!(bar.id.starts_with("1fa965b7"), "the property bar of a PowerClip isn't its frame's: {}", bar.name);

    // Lock Contents to PowerClip toggles
    assert!(!h.state().doc().unwrap().doc.find(pc).unwrap().powerclip_unlocked);
    press_labeled(&mut h, "Lock Contents to PowerClip");
    assert!(h.state().doc().unwrap().doc.find(pc).unwrap().powerclip_unlocked, "Lock didn't toggle");
    press_labeled(&mut h, "Lock Contents to PowerClip");
    assert!(!h.state().doc().unwrap().doc.find(pc).unwrap().powerclip_unlocked);

    // the arrow: Fit Proportionally puts the contents in the middle of the frame, touching it
    press_labeled(&mut h, crate::powerclip::ADJUST_CONTENTS);
    press_labeled(&mut h, "Fit Proportionally");
    {
        let d = h.state().doc().unwrap();
        let fb = d.doc.find(frame).unwrap().bounds();
        let cb = d.doc.find(a).unwrap().bounds().union(d.doc.find(b).unwrap().bounds());
        assert!((cb.center() - fb.center()).hypot() < 0.5, "not centred: {cb:?} in {fb:?}");
        assert!((cb.width() - fb.width()).abs() < 0.5 || (cb.height() - fb.height()).abs() < 0.5, "doesn't fit: {cb:?} in {fb:?}");
        assert!(cb.width() <= fb.width() + 0.5 && cb.height() <= fb.height() + 0.5);
    }
    run_cmd(&mut h, Cmd::Undo);

    // Select Contents: the contents, the PowerClip stays closed; the toolbar goes away
    press_labeled(&mut h, "Select Contents");
    assert_eq!(sel(&h), vec![a, b]);
    assert_eq!(editing(&h), None);
    h.step();
    assert!(labeled(&h, "Select Contents").is_none());

    // Edit PowerClip: the toolbar offers Finish instead
    h.state_mut().doc_mut().unwrap().selection = vec![pc];
    h.step();
    h.step();
    press_labeled(&mut h, "Edit PowerClip...");
    assert_eq!(editing(&h), Some(pc), "Edit didn't enter the PowerClip");
    h.step();
    assert!(labeled(&h, "Edit PowerClip...").is_none());
    press_labeled(&mut h, "Finish Editing PowerClip");
    assert_eq!((editing(&h), sel(&h)), (None, vec![pc]), "Finish didn't leave the PowerClip");

    // Extract Contents: the frame stays, the contents are selected, one undo step
    h.step();
    press_labeled(&mut h, "Extract Contents");
    {
        let d = h.state().doc().unwrap();
        assert_eq!(d.selection, vec![a, b]);
        assert!(d.doc.find(pc).is_none());
        assert_eq!(d.doc.parent_of(a), None);
        assert!(matches!(d.doc.find(frame).unwrap().kind, ObjKind::Ellipse(_)));
    }
    run_cmd(&mut h, Cmd::Undo);
    assert!(h.state().doc().unwrap().doc.find(pc).is_some(), "one undo didn't put the contents back");

    // not with another tool, nor for a plain object
    h.state_mut().doc_mut().unwrap().selection = vec![pc];
    h.state_mut().set_tool(crate::tools::ToolId::Rectangle);
    h.step();
    h.step();
    assert!(labeled(&h, "Select Contents").is_none(), "the toolbar shows with the Rectangle tool");
    h.state_mut().set_tool(crate::tools::ToolId::Pick);
    h.state_mut().doc_mut().unwrap().selection = vec![frame];
    h.step();
    h.step();
    assert!(labeled(&h, "Select Contents").is_none());
}

/// #111/#112 on screen: the floating PowerClip toolbar under a selected PowerClip and its
/// placement menu; a content selected with Alt+click; editing, with the frame as a blue outline,
/// the contents shown in full and the drawing's Finish button.
#[test]
#[ignore]
fn visual_issue_111_112_powerclip() {
    let mut h = doc_harness();
    h.state_mut().doc_mut().unwrap().doc.settings.snap_off = true;
    h.state_mut().set_tool(crate::tools::ToolId::Pick);
    let [pc, a, b, _] = add_powerclip(&mut h);
    h.state_mut().doc_mut().unwrap().selection = vec![pc];
    save(&mut h, "issue_112_01_powerclip_toolbar");
    assert!(labeled(&h, "Select Contents").is_some());
    press_labeled(&mut h, crate::powerclip::ADJUST_CONTENTS);
    save(&mut h, "issue_112_02_adjust_contents_menu");
    h.key_press(egui::Key::Escape);
    h.step();
    h.state_mut().doc_mut().unwrap().selection = vec![pc];
    pick_click(&mut h, (190.0, 200.0), ALT);
    assert_eq!(sel(&h), vec![a]);
    save(&mut h, "issue_111_01_alt_click_content");
    pick_click(&mut h, (300.0, 200.0), CTRL);
    assert_eq!((editing(&h), sel(&h)), (Some(pc), vec![b]));
    save(&mut h, "issue_111_02_editing_powerclip");
    assert!(labeled(&h, "Finish Editing PowerClip").is_some());
    press_labeled(&mut h, "Finish Editing PowerClip");
    assert_eq!((editing(&h), sel(&h)), (None, vec![pc]));
    save(&mut h, "issue_111_03_finished");
}

// ---- 0.6.0: Alt+click through a stack (#139), the shadow clicked (#106), shadows broken off (#105) ----

/// A click with modifiers held (Alt, Shift+Alt...), as the real window sends it.
fn click_mods(h: &mut Harness<'_, AnyVectorApp>, p: egui::Pos2, mods: egui::Modifiers) {
    let button = |down| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: down, modifiers: mods };
    h.input_mut().events.push(egui::Event::ModifiersChanged(mods));
    move_to(h, p);
    h.step();
    h.input_mut().events.push(button(true));
    h.step();
    h.input_mut().events.push(button(false));
    h.step();
    h.input_mut().events.push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

/// A drag with modifiers held.
fn drag_mods(h: &mut Harness<'_, AnyVectorApp>, a: egui::Pos2, b: egui::Pos2, mods: egui::Modifiers) {
    let button = |p, down| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: down, modifiers: mods };
    h.input_mut().events.push(egui::Event::ModifiersChanged(mods));
    move_to(h, a);
    h.step();
    h.input_mut().events.push(button(a, true));
    h.step();
    for k in 1..=6 {
        move_to(h, a + (b - a) * (k as f32 / 6.0));
        h.step();
    }
    h.input_mut().events.push(button(b, false));
    h.step();
    h.input_mut().events.push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

/// A square of side `w` at `at` with a fill, on the active layer (not selected).
fn square_at(h: &mut Harness<'_, AnyVectorApp>, at: (f64, f64), w: f64, rgb: (u8, u8, u8)) -> vector_core::ObjId {
    let d = h.state_mut().doc_mut().unwrap();
    let id = d.doc.new_id();
    let r = RectShape { w, h: w, radii: [0.0; 4], corner: CornerType::default() };
    let mut st = Style::default();
    st.fill = Fill::Solid(Color::rgb(rgb.0, rgb.1, rgb.2));
    d.add_objects("test", vec![Object::new(id, ObjKind::Rect(r), Affine::translate(at), st)]);
    d.selection.clear();
    id
}

fn selection(h: &Harness<'_, AnyVectorApp>) -> Vec<vector_core::ObjId> {
    h.state().doc().unwrap().selection.clone()
}

/// The id prefix of the property bar shown.
fn bar_shown(h: &Harness<'_, AnyVectorApp>) -> String {
    crate::propbar::bar_for(h.state()).map(|b| crate::ui::bars()[b as usize].id[..8].to_string()).unwrap_or_default()
}

/// #139: what Alt+click picks among the objects at the pointer, topmost first.
#[test]
fn alt_pick_walks_down_and_round() {
    use crate::tools::alt_pick;
    let under = [3, 2, 1];
    assert_eq!(alt_pick(&under, &[], false), Some(2), "nothing selected: the one under the top one");
    assert_eq!(alt_pick(&under, &[3], false), Some(2));
    assert_eq!(alt_pick(&under, &[2], false), Some(1), "one further down at each click");
    assert_eq!(alt_pick(&under, &[1], false), Some(3), "round to the top after the bottom one");
    assert_eq!(alt_pick(&under, &[9], false), Some(2), "a selection elsewhere doesn't count");
    assert_eq!(alt_pick(&[7], &[], false), Some(7), "a single object is picked itself");
    assert_eq!(alt_pick(&[7], &[7], false), Some(7));
    assert_eq!(alt_pick(&[], &[], false), None);
    // Shift+Alt adds the next one down not selected yet
    assert_eq!(alt_pick(&under, &[3], true), Some(2));
    assert_eq!(alt_pick(&under, &[3, 2], true), Some(1));
    assert_eq!(alt_pick(&under, &[1, 2], true), Some(3));
    assert_eq!(alt_pick(&under, &[1, 2, 3], true), None, "all of them selected");
}

/// #139: with Treat All Objects as Filled on, Alt+click selects the object behind the top one,
/// one further down at each click and the top one again after the bottom one; Shift+Alt adds it;
/// a plain click still picks the top one, and Alt+drag on empty space is still the marquee that
/// selects what it touches. Alt+click and drag moves the object behind.
#[test]
fn alt_click_selects_the_object_behind() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().settings.treat_as_filled = true;
    h.state_mut().set_tool(ToolId::Pick);
    let a = square_at(&mut h, (100.0, 100.0), 200.0, (200, 40, 40));
    let b = square_at(&mut h, (150.0, 150.0), 200.0, (40, 160, 40));
    let c = square_at(&mut h, (200.0, 200.0), 200.0, (40, 40, 200));
    h.step();
    let p = scr(&h, (260.0, 260.0));
    click(&mut h, p);
    assert_eq!(selection(&h), vec![c], "a plain click picks the top one");
    for want in [b, a, c, b] {
        click_mods(&mut h, p, egui::Modifiers::ALT);
        assert_eq!(selection(&h), vec![want]);
    }
    assert!(!h.state().doc().unwrap().rotate_mode, "Alt+click on the selection isn't a second click");
    // Shift+Alt adds the one below
    click_mods(&mut h, p, egui::Modifiers::ALT | egui::Modifiers::SHIFT);
    assert_eq!(selection(&h), vec![b, a]);
    // only the objects under the pointer are walked
    let q1 = scr(&h, (600.0, 600.0));
    click(&mut h, q1);
    let q1 = scr(&h, (120.0, 120.0));
    click(&mut h, q1);
    assert_eq!(selection(&h), vec![a]);
    let q1 = scr(&h, (120.0, 120.0));
    click_mods(&mut h, q1, egui::Modifiers::ALT);
    assert_eq!(selection(&h), vec![a], "nothing under it: it stays selected");
    // Alt+click and drag moves the object behind, not the top one
    click(&mut h, p);
    let before = h.state().doc().unwrap().doc.find(b).unwrap().bounds();
    drag_mods(&mut h, p, p + egui::vec2(40.0, 0.0), egui::Modifiers::ALT);
    let d = h.state().doc().unwrap();
    assert_eq!(d.selection, vec![b]);
    assert!(d.doc.find(b).unwrap().bounds().x0 > before.x0 + 1.0, "the object behind moved");
    assert_eq!(d.doc.find(c).unwrap().bounds().x0, 200.0, "the top one stayed");
    h.state_mut().run(Cmd::Undo);
    h.step();
    // Alt+drag on empty space: the marquee selects what it touches
    let q1 = scr(&h, (600.0, 600.0));
    click(&mut h, q1);
    assert!(selection(&h).is_empty());
    let q1 = scr(&h, (50.0, 50.0));
    let q2 = scr(&h, (120.0, 120.0));
    drag_mods(&mut h, q1, q2, egui::Modifiers::ALT);
    assert_eq!(selection(&h), vec![a], "Alt+drag still selects the objects the marquee touches");
}

/// #139: Alt+click also walks unfilled objects with Treat All Objects as Filled off (where their
/// outlines are), and only the active layer's objects without Edit Across Layers.
#[test]
fn alt_click_respects_treat_as_filled_and_layers() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().settings.treat_as_filled = false;
    h.state_mut().set_tool(ToolId::Pick);
    let unfilled = |h: &mut Harness<'_, AnyVectorApp>, w: f64| {
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        let mut st = Style::default();
        st.fill = Fill::None;
        st.outline.color = Some(Color::BLACK);
        let r = RectShape { w, h: w, radii: [0.0; 4], corner: CornerType::default() };
        d.add_objects("test", vec![Object::new(id, ObjKind::Rect(r), Affine::translate((100.0, 100.0)), st)]);
        id
    };
    let a = unfilled(&mut h, 200.0);
    let b = unfilled(&mut h, 120.0);
    h.state_mut().doc_mut().unwrap().selection.clear();
    h.step();
    // the left side both have
    let side = scr(&h, (100.0, 150.0));
    click(&mut h, side);
    assert_eq!(selection(&h), vec![b]);
    click_mods(&mut h, side, egui::Modifiers::ALT);
    assert_eq!(selection(&h), vec![a]);
    // inside, off the outlines: nothing to walk, the selection goes as with a plain click
    let q1 = scr(&h, (150.0, 150.0));
    click_mods(&mut h, q1, egui::Modifiers::ALT);
    assert!(selection(&h).is_empty());
    // a second layer on top, made the active one, with Edit Across Layers off
    let top = {
        let d = h.state_mut().doc_mut().unwrap();
        let lid = d.doc.new_id();
        d.doc.pages[0].layers.push(vector_core::Layer::new(lid, "Top"));
        d.layer = d.doc.pages[0].layers.len() - 1;
        unfilled(&mut h, 160.0)
    };
    h.state_mut().settings.edit_across_layers = false;
    h.state_mut().doc_mut().unwrap().selection.clear();
    click(&mut h, side);
    assert_eq!(selection(&h), vec![top]);
    click_mods(&mut h, side, egui::Modifiers::ALT);
    assert_eq!(selection(&h), vec![top], "the other layer's objects aren't reached");
    h.state_mut().settings.edit_across_layers = true;
    click_mods(&mut h, side, egui::Modifiers::ALT);
    assert_eq!(selection(&h), vec![b]);
}

#[test]
#[ignore]
fn visual_issue_139_alt_click_selects_behind() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().settings.treat_as_filled = true;
    h.state_mut().set_tool(ToolId::Pick);
    let a = square_at(&mut h, (100.0, 100.0), 200.0, (200, 40, 40));
    let _b = square_at(&mut h, (180.0, 180.0), 200.0, (40, 40, 200));
    let p = scr(&h, (240.0, 240.0));
    click(&mut h, p);
    save(&mut h, "issue_139_01_click_picks_the_top_one");
    click_mods(&mut h, p, egui::Modifiers::ALT);
    assert_eq!(selection(&h), vec![a]);
    save(&mut h, "issue_139_02_alt_click_picks_the_one_behind");
}

/// A square at (100, 100) with a block shadow 40 pt to the lower right, and one at (500, 100)
/// with a drop shadow 30 pt down and to the right; neither selected.
fn shadowed_squares(h: &mut Harness<'_, AnyVectorApp>) -> (vector_core::ObjId, vector_core::ObjId) {
    let block = square_at(h, (100.0, 100.0), 150.0, (30, 144, 255));
    let drop = square_at(h, (500.0, 100.0), 150.0, (255, 160, 30));
    let d = h.state_mut().doc_mut().unwrap();
    d.doc.find_mut(block).unwrap().effects.block_shadow = Some(vector_core::object::BlockShadow { offset: kurbo::Vec2::new(40.0, 40.0), color: Color::rgb(60, 60, 60), ..Default::default() });
    d.doc.find_mut(drop).unwrap().effects.drop_shadow = Some(vector_core::object::DropShadow { offset: kurbo::Vec2::new(30.0, 30.0), feather: 10.0, ..Default::default() });
    d.selection.clear();
    (block, drop)
}

/// #106: clicking an object's shadow with the Pick tool selects the object with the shadow's
/// bar (block or drop shadow) and its fields on; the status bar says the shadow is selected; a
/// click on the object itself, Escape or another tool give the object's own bar back; Delete
/// with the shadow clicked removes only the shadow (one undo step); dragging the shadow moves
/// the object; an object above wins over a shadow under it.
#[test]
fn clicking_a_shadow_selects_it_with_its_bar() {
    use crate::tools::ToolId;
    use vector_core::object::ShadowPart;
    let mut h = doc_harness();
    h.state_mut().set_tool(ToolId::Pick);
    let (block, drop) = shadowed_squares(&mut h);
    h.step();
    let focus = |h: &Harness<'_, AnyVectorApp>| h.state().doc().unwrap().shadow_focus();
    // the object itself: its own bar
    let q1 = scr(&h, (150.0, 150.0));
    click(&mut h, q1);
    assert_eq!((selection(&h), focus(&h)), (vec![block], None));
    assert_eq!(bar_shown(&h), "b5286ff5", "the rectangle bar");
    // the block shadow
    let q1 = scr(&h, (280.0, 280.0));
    click(&mut h, q1);
    assert_eq!((selection(&h), focus(&h)), (vec![block], Some(ShadowPart::Block)));
    assert_eq!(bar_shown(&h), "aa88b391", "the Block Shadow bar");
    assert_eq!(crate::propbar::option_state(h.state(), "53bbc973"), Some((true, false)), "Clear block shadow is on");
    assert!(crate::statusbar::object_info(h.state()).starts_with("Block Shadow on "), "{}", crate::statusbar::object_info(h.state()));
    assert!(!h.state().doc().unwrap().rotate_mode, "the shadow clicked isn't a second click on the object");
    // back on the object
    let q1 = scr(&h, (150.0, 150.0));
    click(&mut h, q1);
    assert_eq!((selection(&h), focus(&h)), (vec![block], None));
    assert!(!h.state().doc().unwrap().rotate_mode);
    assert_eq!(bar_shown(&h), "b5286ff5");
    // the drop shadow, in its feathering past the side of the shifted square
    let q1 = scr(&h, (683.0, 200.0));
    click(&mut h, q1);
    assert_eq!((selection(&h), focus(&h)), (vec![drop], Some(ShadowPart::Drop)));
    assert_eq!(bar_shown(&h), "edcfb996", "the Drop Shadow bar");
    assert_eq!(crate::propbar::option_state(h.state(), "bacce2d7"), Some((true, false)), "Clear Shadow is on");
    assert_eq!(crate::propbar::option_state(h.state(), "e8ef757d"), Some((true, true)));
    // Escape: nothing selected
    key(&mut h, egui::Key::Escape, true);
    h.step();
    key(&mut h, egui::Key::Escape, false);
    h.step();
    assert_eq!((selection(&h), focus(&h)), (vec![], None));
    assert!(h.state().doc().unwrap().shadow_sel.is_none());
    // selected some other way, the object is the object
    h.state_mut().doc_mut().unwrap().selection = vec![drop];
    h.step();
    assert_eq!(bar_shown(&h), "b5286ff5");
    // another tool forgets the shadow
    let q1 = scr(&h, (683.0, 200.0));
    click(&mut h, q1);
    assert_eq!(focus(&h), Some(ShadowPart::Drop));
    h.state_mut().set_tool(ToolId::Rectangle);
    h.state_mut().set_tool(ToolId::Pick);
    h.step();
    assert_eq!(focus(&h), None);
    // Delete removes the shadow only, as one undo step
    let q1 = scr(&h, (683.0, 200.0));
    click(&mut h, q1);
    h.state_mut().run(Cmd::Delete);
    h.step();
    let d = h.state().doc().unwrap();
    assert!(d.doc.find(drop).is_some_and(|o| o.effects.drop_shadow.is_none()), "Delete removed the object, or left the shadow");
    assert_eq!(d.selection, vec![drop]);
    assert_eq!(bar_shown(&h), "b5286ff5");
    h.state_mut().run(Cmd::Undo);
    h.step();
    assert!(h.state().doc().unwrap().doc.find(drop).unwrap().effects.drop_shadow.is_some(), "one undo brings the shadow back");
    // dragging the shadow moves the object
    let a = scr(&h, (280.0, 280.0));
    drag(&mut h, a, a + egui::vec2(30.0, 0.0));
    let d = h.state().doc().unwrap();
    assert_eq!(d.selection, vec![block]);
    assert!(d.doc.find(block).unwrap().bounds().x0 > 101.0, "the object moved with its shadow");
    assert!(d.doc.find(block).unwrap().effects.block_shadow.is_some());
    // an object above wins over the shadow under it
    let over = square_at(&mut h, (650.0, 250.0), 40.0, (0, 0, 0));
    h.step();
    let q1 = scr(&h, (670.0, 270.0));
    click(&mut h, q1);
    assert_eq!((selection(&h), focus(&h)), (vec![over], None));
}

#[test]
#[ignore]
fn visual_issue_106_shadow_clicked_shows_its_bar() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().set_tool(ToolId::Pick);
    let (block, drop) = shadowed_squares(&mut h);
    let q1 = scr(&h, (280.0, 280.0));
    click(&mut h, q1);
    assert_eq!(selection(&h), vec![block]);
    save(&mut h, "issue_106_01_block_shadow_clicked");
    let q1 = scr(&h, (683.0, 200.0));
    click(&mut h, q1);
    assert_eq!(selection(&h), vec![drop]);
    save(&mut h, "issue_106_02_drop_shadow_clicked");
    let q1 = scr(&h, (575.0, 175.0));
    click(&mut h, q1);
    save(&mut h, "issue_106_03_object_clicked");
}

/// #105: Break Apart is on for a shadowed rectangle and separates the shadow into its own object
/// right behind it (a curve for a block shadow, a bitmap for a drop shadow), both selected, as
/// one undo step; the object keeps its fill and outline; the shadow moves on its own; text with
/// a shadow loses the shadow first and splits at the next Break Apart.
#[test]
fn break_apart_separates_the_shadow() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().set_tool(ToolId::Pick);
    let (block, drop) = shadowed_squares(&mut h);
    let on = |h: &Harness<'_, AnyVectorApp>| crate::ui::bind::enabled(h.state(), "d5669070-c0f3-4fc0-b524-e33aeea7925f");
    let count = |h: &Harness<'_, AnyVectorApp>| h.state().doc().unwrap().page_ref().all_objects().count();
    let index = |h: &Harness<'_, AnyVectorApp>, id| h.state().doc().unwrap().doc.locate(id).unwrap().2;
    let n0 = count(&h);
    // block shadow: a curve of the shadow's color
    h.state_mut().doc_mut().unwrap().selection = vec![block];
    h.step();
    assert_eq!(on(&h), Some(true), "Break Apart is on for a shadowed rectangle");
    let style = h.state().doc().unwrap().doc.find(block).unwrap().style.clone();
    h.state_mut().run(Cmd::BreakApart);
    h.step();
    let d = h.state().doc().unwrap();
    assert_eq!(count(&h), n0 + 1);
    assert_eq!(d.selection.len(), 2);
    assert_eq!(d.selection[1], block);
    let shadow = d.selection[0];
    let so = d.doc.find(shadow).unwrap();
    assert!(matches!(so.kind, ObjKind::Path(_)));
    assert_eq!(so.style.fill, Fill::Solid(Color::rgb(60, 60, 60)));
    assert!(!so.style.outline.is_visible());
    let sb = kurbo::Shape::bounding_box(&so.world_path());
    assert!((sb.x0 - 100.0).abs() < 0.5 && (sb.x1 - 290.0).abs() < 0.5 && (sb.y1 - 290.0).abs() < 0.5, "{sb:?}");
    assert_eq!(index(&h, shadow) + 1, index(&h, block), "right behind the object");
    let src = d.doc.find(block).unwrap();
    assert!(src.effects.block_shadow.is_none());
    assert_eq!(src.style, style, "the object keeps its fill and outline");
    // the shadow moves on its own
    h.state_mut().doc_mut().unwrap().selection = vec![shadow];
    h.state_mut().run(Cmd::Nudge(50.0, 0.0));
    h.step();
    assert_eq!(h.state().doc().unwrap().doc.find(block).unwrap().bounds().x0, 100.0);
    h.state_mut().run(Cmd::Undo);
    h.state_mut().run(Cmd::Undo);
    h.step();
    assert_eq!(count(&h), n0, "one undo step");
    assert!(h.state().doc().unwrap().doc.find(block).unwrap().effects.block_shadow.is_some());

    // drop shadow: a bitmap with the shadow's pixels where the shadow was drawn
    h.state_mut().doc_mut().unwrap().selection = vec![drop];
    h.step();
    assert_eq!(on(&h), Some(true));
    h.state_mut().run(Cmd::BreakApart);
    h.step();
    let d = h.state().doc().unwrap();
    assert_eq!(count(&h), n0 + 1);
    assert_eq!(d.selection.len(), 2);
    let img = d.doc.find(d.selection[0]).unwrap();
    let ObjKind::Image(io) = &img.kind else { panic!("the drop shadow isn't a bitmap: {:?}", img.kind) };
    let data = &d.doc.images[&io.image];
    assert!(data.rgba.chunks(4).any(|px| px[3] > 100), "the bitmap has the shadow in it");
    assert_eq!(img.style.merge, vector_core::style::MergeMode::Multiply);
    // the shadow square (530..680) and about its feathering around it
    let b = img.bounds();
    assert!(b.x0 > 500.0 && b.x0 < 530.0 && b.x1 > 680.0 && b.x1 < 710.0 && b.y1 > 280.0 && b.y1 < 310.0, "{b:?}");
    assert!(d.doc.find(drop).unwrap().effects.drop_shadow.is_none());
    h.state_mut().doc_mut().unwrap().selection = vec![drop];
    assert_eq!(on(&h), Some(false), "nothing more to break apart");

    // text with a shadow: the shadow first, the words next
    let t = {
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        let mut o = Object::new(id, ObjKind::Text(vector_core::text::TextObj::artistic("AB CD", vector_core::text::CharFormat::default())), Affine::translate((100.0, 500.0)), Style::default());
        o.effects.drop_shadow = Some(crate::tools::default_shadow(false));
        d.add_objects("test", vec![o]);
        id
    };
    h.state_mut().run(Cmd::BreakApart);
    h.step();
    let d = h.state().doc().unwrap();
    assert_eq!(d.selection.len(), 2);
    assert!(matches!(d.doc.find(t).unwrap().kind, ObjKind::Text(_)) && d.doc.find(t).unwrap().effects.drop_shadow.is_none());
    h.state_mut().doc_mut().unwrap().selection = vec![t];
    h.state_mut().run(Cmd::BreakApart);
    h.step();
    assert_eq!(selection(&h).len(), 2, "the next Break Apart splits the words");
}

#[test]
#[ignore]
fn visual_issue_105_shadow_broken_apart() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().set_tool(ToolId::Pick);
    let (block, drop) = shadowed_squares(&mut h);
    save(&mut h, "issue_105_01_shadowed");
    for id in [block, drop] {
        h.state_mut().doc_mut().unwrap().selection = vec![id];
        h.state_mut().run(Cmd::BreakApart);
        h.step();
        // the shadow pulled down, away from its object
        let shadow = selection(&h)[0];
        h.state_mut().doc_mut().unwrap().selection = vec![shadow];
        h.state_mut().run(Cmd::Nudge(0.0, 220.0));
        h.step();
    }
    save(&mut h, "issue_105_02_shadows_broken_off_and_moved");
}

// ---- Outlines and units: hairlines, the outline unit, Outline Pen, text units, inch rulers ----
// (#103, #109, #113, #127, #133, #137, #138)

/// A selected rectangle with this outline.
fn outlined_rect(h: &mut Harness<'_, AnyVectorApp>, ol: vector_core::style::Outline) -> vector_core::ObjId {
    let d = h.state_mut().doc_mut().unwrap();
    let id = d.doc.new_id();
    let st = Style { fill: Fill::Solid(Color::rgb(250, 220, 120)), outline: ol, ..Style::default() };
    d.add_objects("test", vec![Object::new(id, ObjKind::Rect(RectShape { w: 216.0, h: 144.0, radii: [0.0; 4], corner: CornerType::default() }), Affine::translate((144.0, 288.0)), st)]);
    d.selection = vec![id];
    h.step();
    id
}

/// What the status bar's outline field says: the color and the width.
fn status_outline(h: &Harness<'_, AnyVectorApp>) -> (Option<String>, String) {
    let (u, dpi) = crate::lengths::outline_unit(h.state());
    crate::statusbar::outline_texts(crate::statusbar::current_fill_outline(h.state()).1, u, dpi)
}

fn outline_of(h: &Harness<'_, AnyVectorApp>, id: vector_core::ObjId) -> vector_core::style::Outline {
    h.state().doc().unwrap().doc.find(id).unwrap().style.outline.clone()
}

/// #127/#133/#138: a hairline (width 0 with a color) is an outline: the status bar shows its
/// color and "Hairline", a palette color keeps it a hairline, and only an outline without a
/// color is "None".
#[test]
fn hairline_is_an_outline_everywhere() {
    use vector_core::style::Outline;
    let mut h = doc_harness();
    h.state_mut().settings.outline_unit = vector_core::Unit::Points;
    let id = outlined_rect(&mut h, Outline { width: 0.0, color: Some(Color::BLACK), ..Outline::default() });
    assert_eq!(status_outline(&h), (Some("R:0 G:0 B:0 (#000000)".to_string()), "Hairline".to_string()));
    // a palette color on the outline keeps the hairline
    crate::exec::apply_color(h.state_mut(), Some(Color::rgb(200, 0, 0)), true);
    h.step();
    let ol = outline_of(&h, id);
    assert_eq!((ol.color, ol.width), (Some(Color::rgb(200, 0, 0)), 0.0), "the hairline got a width");
    // the Outline Pen flyout: No Outline, then Hairline Outline gives it a color again
    h.state_mut().run(Cmd::OutlineWidth(None));
    h.step();
    assert!(!outline_of(&h, id).is_visible());
    assert_eq!(status_outline(&h), (None, "None".to_string()));
    h.state_mut().run(Cmd::OutlineWidth(Some(0.0)));
    h.step();
    let ol = outline_of(&h, id);
    assert!(ol.is_visible() && ol.width == 0.0, "{ol:?}");
    assert_eq!(status_outline(&h).1, "Hairline");
    // #138: the width is in the status bar, in the outline unit
    h.state_mut().run(Cmd::OutlineWidth(Some(2.0)));
    h.state_mut().settings.outline_unit = vector_core::Unit::Inches;
    h.step();
    assert_eq!(status_outline(&h).1, "0.0278 \"");
    // with nothing selected the status bar shows the outline of new objects
    h.state_mut().doc_mut().unwrap().selection.clear();
    h.state_mut().set_default_outline(Outline::default());
    h.step();
    assert_eq!(status_outline(&h).0, Some("R:0 G:0 B:0 (#000000)".to_string()));
}

/// #137: the outline width list follows the outline unit; a width typed with another unit is
/// converted, "hairline" and "none" are understood.
#[test]
fn outline_width_list_and_field_follow_the_outline_unit() {
    use crate::lengths::{parse_width, width_choices, width_text};
    use vector_core::Unit;
    let inch: Vec<String> = width_choices(Unit::Inches, 300.0).iter().map(|w| width_text(*w, Unit::Inches, 300.0)).collect();
    assert_eq!(inch[0], "Hairline");
    assert!(inch.iter().skip(1).all(|t| t.ends_with('"')), "{inch:?}");
    assert!(inch.contains(&"0.01 \"".to_string()) && inch.contains(&"0.5 \"".to_string()), "{inch:?}");
    let mm: Vec<String> = width_choices(Unit::Millimeters, 300.0).iter().map(|w| width_text(*w, Unit::Millimeters, 300.0)).collect();
    assert!(mm.iter().skip(1).all(|t| t.ends_with(" mm")), "{mm:?}");
    assert_eq!(parse_width("2 pt", Unit::Inches, 300.0), Some(Some(2.0)));
    assert_eq!(parse_width("Hairline", Unit::Inches, 300.0), Some(Some(0.0)));
    assert_eq!(parse_width("none", Unit::Inches, 300.0), Some(None));
    // the property bar sets the width of the selection, in points
    let mut h = doc_harness();
    let id = outlined_rect(&mut h, vector_core::style::Outline::none());
    crate::propbar::set_outline_width(h.state_mut(), parse_width("0.01\"", Unit::Points, 300.0).unwrap());
    h.step();
    let ol = outline_of(&h, id);
    assert!(ol.is_visible() && (ol.width - 0.72).abs() < 1e-9, "{ol:?}");
}

/// #103: F12 with nothing selected opens Outline Pen on the outline of new objects; OK makes
/// it the default (kept in the settings) and a new rectangle gets it. On a selection, OK changes
/// only what was changed, in one undo step.
#[test]
fn outline_pen_sets_the_default_outline_and_edits_the_selection() {
    use crate::tools::ToolId;
    use egui_kittest::kittest::Queryable;
    use vector_core::style::{LineJoin, Outline};
    let mut h = doc_harness();
    h.state_mut().set_default_outline(Outline::default());
    h.state_mut().settings.outline_unit = vector_core::Unit::Points;
    h.state_mut().doc_mut().unwrap().selection.clear();
    h.step();
    h.key_press(egui::Key::F12);
    h.step();
    let st = h.state_mut().dialogs.outline_pen.as_mut().expect("F12 didn't open Outline Pen");
    assert!(!st.selection);
    st.ol.width = 4.0;
    st.ol.color = Some(Color::rgb(0, 120, 0));
    st.ol.join = LineJoin::Round;
    h.step();
    assert!(h.query_by_label("Nothing is selected: these settings are the outline of new objects.").is_some(), "no note that the defaults are edited");
    save(&mut h, "outline_pen_01_defaults");
    h.get_by_label("OK").click();
    h.step();
    assert!(h.state().dialogs.outline_pen.is_none());
    for ol in [&h.state().settings.default_outline, &h.state().ts.default_outline] {
        assert_eq!((ol.width, ol.color, ol.join), (4.0, Some(Color::rgb(0, 120, 0)), LineJoin::Round));
    }
    // a new rectangle has it
    h.state_mut().set_tool(ToolId::Rectangle);
    h.step();
    let (a, b) = {
        let d = h.state().doc().unwrap();
        (d.to_screen(Point::new(100.0, 100.0)), d.to_screen(Point::new(300.0, 250.0)))
    };
    drag(&mut h, a, b);
    let id = *h.state().doc().unwrap().selection.first().expect("no rectangle drawn");
    let ol = outline_of(&h, id);
    assert_eq!((ol.width, ol.color, ol.join), (4.0, Some(Color::rgb(0, 120, 0)), LineJoin::Round));
    // on the selection: a hairline, only the width changes, as one undo step
    h.state_mut().set_tool(ToolId::Pick);
    h.state_mut().run(Cmd::OutlinePen);
    h.step();
    let st = h.state_mut().dialogs.outline_pen.as_mut().expect("Outline Pen didn't open");
    assert!(st.selection);
    st.ol.width = 0.0;
    h.step();
    save(&mut h, "outline_pen_02_selection");
    h.get_by_label("OK").click();
    h.step();
    let ol = outline_of(&h, id);
    assert_eq!((ol.width, ol.color, ol.join), (0.0, Some(Color::rgb(0, 120, 0)), LineJoin::Round));
    assert_eq!(status_outline(&h).1, "Hairline");
    h.state_mut().run(Cmd::Undo);
    h.step();
    assert_eq!(outline_of(&h, id).width, 4.0, "one undo didn't bring the width back");
    // the default stays as it was
    assert_eq!(h.state().ts.default_outline.width, 4.0);
    // a palette right-click with nothing selected sets the default outline color
    h.state_mut().doc_mut().unwrap().selection.clear();
    crate::exec::apply_color(h.state_mut(), Some(Color::rgb(0, 0, 255)), true);
    assert_eq!(h.state().ts.default_outline.color, Some(Color::rgb(0, 0, 255)));
    assert_eq!(h.state().ts.default_outline.width, 4.0);
}

/// #109: font sizes show in the text unit (Options ▸ Text ▸ Default text units); a size typed in
/// it, or with its own unit, is converted to points.
#[test]
fn font_size_uses_the_text_unit() {
    use crate::tools::ToolId;
    use egui_kittest::kittest::Queryable;
    let mut h = doc_harness();
    h.state_mut().settings.text_unit = vector_core::Unit::Inches;
    h.state_mut().ts.text_format.size = 12.0;
    h.state_mut().set_tool(ToolId::Text);
    h.step();
    h.step();
    // the property bar's field (the first one: the Text docker may show the size too)
    h.get_all_by_value("0.167 \"").next().expect("the font size isn't shown in inches").focus();
    h.step();
    h.get_all_by_value("0.167 \"").next().unwrap().set_text_value_accesskit("0.25");
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    assert!((h.state().ts.text_format.size - 18.0).abs() < 1e-9, "{}", h.state().ts.text_format.size);
    assert!(h.query_all_by_value("0.25 \"").next().is_some());
    // a size typed with its own unit
    h.get_all_by_value("0.25 \"").next().unwrap().focus();
    h.step();
    h.get_all_by_value("0.25 \"").next().unwrap().set_text_value_accesskit("24 pt");
    h.step();
    h.key_press(egui::Key::Enter);
    h.step();
    assert!((h.state().ts.text_format.size - 24.0).abs() < 1e-9, "{}", h.state().ts.text_format.size);
    // and in points
    h.state_mut().settings.text_unit = vector_core::Unit::Points;
    h.step();
    h.step();
    assert!(h.query_all_by_value("24 pt").next().is_some());
}

/// #127/#137/#138/#113: the hairline in the status bar and the Properties docker, the outline
/// width in inches, and inch rulers divided into fractions.
#[test]
#[ignore]
fn visual_outline_units_and_inch_rulers() {
    use crate::dockers::DockerId;
    use vector_core::style::Outline;
    let mut h = doc_harness();
    h.state_mut().settings.outline_unit = vector_core::Unit::Points;
    {
        let app = h.state_mut();
        app.dockers.open.clear();
        app.dockers.show(DockerId::Properties);
        app.dockers.collapsed = false;
        app.dockers.set_width = Some(320.0);
        app.dockers.ui.pr.tabbed = true;
        app.dockers.ui.pr.summary = false;
        app.dockers.ui.pr.tab = 0;
    }
    outlined_rect(&mut h, Outline { width: 0.0, color: Some(Color::BLACK), ..Outline::default() });
    save(&mut h, "outline_units_01_hairline");
    h.state_mut().run(Cmd::OutlineWidth(Some(2.0)));
    h.state_mut().settings.outline_unit = vector_core::Unit::Inches;
    h.state_mut().doc_mut().unwrap().doc.settings.unit = vector_core::Unit::Inches;
    save(&mut h, "outline_units_02_inches");
    // inch rulers, from far to close
    for (i, pct) in [25.0, 100.0, 400.0, 1600.0].into_iter().enumerate() {
        h.state_mut().doc_mut().unwrap().set_zoom_percent(pct);
        save(&mut h, &format!("outline_units_{:02}_inch_ruler_{pct}", i + 3));
    }
    h.state_mut().doc_mut().unwrap().doc.settings.unit = vector_core::Unit::Millimeters;
    h.state_mut().doc_mut().unwrap().set_zoom_percent(100.0);
    save(&mut h, "outline_units_07_mm_ruler");
}

// ---- Desktop, page border and window colours (#141), bitmaps in Wireframe (#129), Command Bars
// ---- (#136) and command bar customization (#124) ----

/// The darkest pixel (by luma) in the `rad` pixels around `p` along `dir`.
fn darkest(img: &image::RgbaImage, p: egui::Pos2, dir: egui::Vec2, rad: i32) -> f32 {
    (-rad..=rad)
        .filter_map(|k| {
            let q = p + dir * k as f32;
            let px = img.get_pixel_checked(q.x.round() as u32, q.y.round() as u32)?;
            Some(0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32)
        })
        .fold(f32::MAX, f32::min)
}

fn render(h: &mut Harness<'_, AnyVectorApp>) -> image::RgbaImage {
    for _ in 0..3 {
        h.step();
    }
    h.render().expect("render")
}

/// #141: a white page on a white desktop shows its border on all four sides (not just a shadow
/// at the right and bottom), View ▸ Page Border turns it off, and Options ▸ Appearance sets the
/// window border and the desktop colours with a Reset each.
#[test]
#[ignore]
fn visual_issue_141_page_border_and_window_colors() {
    use egui_kittest::kittest::Queryable;
    let mut h = doc_harness();
    h.state_mut().settings.desktop_color = [255, 255, 255];
    let img = render(&mut h);
    let d = h.state().doc().unwrap();
    let p = d.to_screen_rect(d.page_ref().rect());
    let edges = [(p.left_center(), egui::Vec2::X), (p.center_top(), egui::Vec2::Y), (p.right_center(), egui::Vec2::X), (p.center_bottom(), egui::Vec2::Y)];
    for (at, dir) in edges {
        assert!(darkest(&img, at, dir, 2) < 200.0, "no page border at {at:?} of {p:?}");
    }
    // the page and the desktop around it stay white
    assert!(darkest(&img, p.center(), egui::Vec2::X, 3) > 250.0);
    assert!(darkest(&img, p.left_center() - egui::vec2(12.0, 0.0), egui::Vec2::X, 2) > 250.0, "the border is wider than a line");
    save(&mut h, "issue_141_01_white_page_on_white_desktop");

    h.state_mut().run(Cmd::TogglePageBorder);
    let img = render(&mut h);
    assert!(!h.state().doc().unwrap().doc.settings.show_page_border);
    assert!(darkest(&img, p.left_center(), egui::Vec2::X, 2) > 250.0, "View ▸ Page Border off still shows the border");
    assert!(darkest(&img, p.center_top(), egui::Vec2::Y, 2) > 250.0);
    save(&mut h, "issue_141_02_page_border_off");
    h.state_mut().run(Cmd::TogglePageBorder);

    // Options ▸ Customization ▸ Appearance: the window border colour, kept, and its Reset
    h.state_mut().settings.window_border = Some([0x20, 0x60, 0xb0]);
    h.state_mut().open_options(crate::commands::OptionsPage::Customization, Some("Appearance"));
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.state().settings.window_border, Some([0x20, 0x60, 0xb0]), "the window border colour was thrown away");
    save(&mut h, "issue_141_03_appearance");
    // the Reset buttons of the Window border and the Desktop rows, in that order
    let resets = |h: &Harness<'_, AnyVectorApp>| {
        let mut v: Vec<egui::Rect> = h.query_all_by_label("Reset").map(|n| n.rect()).collect();
        v.sort_by(|a, b| a.top().total_cmp(&b.top()));
        assert_eq!(v.len(), 2, "a Reset for the window border and one for the desktop");
        v
    };
    let reset = resets(&h)[0];
    click(&mut h, reset.center());
    assert_eq!(h.state().settings.window_border, None, "Reset didn't give the window border back to the theme");
    assert_eq!(crate::theme::window_border(None), crate::theme::pal().bar_dark);
    // the desktop's Reset gives it the theme's desktop colour
    let reset = resets(&h)[1];
    click(&mut h, reset.center());
    assert_ne!(h.state().settings.desktop_color, [255, 255, 255], "Reset left the desktop white");
    save(&mut h, "issue_141_04_appearance_reset");
}

/// #129: in Wireframe view a bitmap shows as a light gray ghost of itself inside its frame,
/// not as an empty box.
#[test]
#[ignore]
fn visual_issue_129_wireframe_bitmap_ghost() {
    let mut h = harness();
    sample_bitmap(&mut h);
    h.state_mut().doc_mut().unwrap().selection.clear();
    h.state_mut().doc_mut().unwrap().view_mode = crate::docview::ViewMode::Wireframe;
    let img = render(&mut h);
    let d = h.state().doc().unwrap();
    let id = d.page_ref().all_objects().find(|o| matches!(o.kind, ObjKind::Image(_))).unwrap().id;
    let b = d.to_screen_rect(d.doc.find(id).unwrap().bounds());
    // a sample row across the picture: shades of gray, not all white, and no colour
    let (mut lo, mut hi) = (255u8, 0u8);
    for k in 1..20 {
        let x = b.left() + b.width() * k as f32 / 20.0;
        let px = img.get_pixel(x as u32, (b.top() + b.height() * 0.8) as u32);
        assert!(px[0].abs_diff(px[1]) < 6 && px[1].abs_diff(px[2]) < 6, "a coloured pixel {px:?} in Wireframe");
        lo = lo.min(px[0]);
        hi = hi.max(px[0]);
    }
    assert!(lo < 245, "the bitmap is an empty box in Wireframe ({lo}..{hi})");
    assert!(lo > 120, "the ghost is not light ({lo})");
    // the sun stands out from the sky
    let sun = img.get_pixel((b.left() + b.width() * 0.72) as u32, (b.top() + b.height() * 0.28) as u32)[0];
    let sky = img.get_pixel((b.left() + b.width() * 0.2) as u32, (b.top() + b.height() * 0.2) as u32)[0];
    assert!(sun.abs_diff(sky) > 6, "the picture can't be made out: sun {sun}, sky {sky}");
    save(&mut h, "issue_129_01_wireframe_bitmap_ghost");
    h.state_mut().doc_mut().unwrap().view_mode = crate::docview::ViewMode::Enhanced;
    save(&mut h, "issue_129_02_enhanced_bitmap");
}

/// #136: Options ▸ Customization ▸ Command Bars lists every bar on a line of its own, whole and
/// readable; New makes a bar that floats; Style is kept for the bar; Reset puts the bar back.
#[test]
#[ignore]
fn visual_issue_136_command_bars_page() {
    use crate::customize as cz;
    use egui_kittest::kittest::Queryable;
    let mut h = doc_harness();
    h.state_mut().run(Cmd::CommandBarsSetup);
    for _ in 0..4 {
        h.step();
    }
    save(&mut h, "issue_136_01_command_bars");
    let dlg = h.ctx.memory(|m| m.area_rect(egui::Id::unique(("dlg", "options")))).expect("the Options dialog is open");
    // the one widget labelled `label` on the dialog
    let on_dlg = |h: &Harness<'_, AnyVectorApp>, label: &str| h.query_all_by_label(label).map(|n| n.rect()).find(|r| dlg.intersects(*r)).unwrap_or_else(|| panic!("no {label:?} on the dialog"));
    let rows: Vec<egui::Rect> = ["Standard", "Toolbox", "Zoom", "Transform"].iter().map(|n| on_dlg(&h, n)).collect();
    let mut sorted = rows.clone();
    sorted.sort_by(|a, b| a.top().total_cmp(&b.top()));
    for w in sorted.windows(2) {
        assert!(w[1].top() >= w[0].bottom() - 0.5, "rows overlap: {w:?}");
        assert!((w[1].left() - w[0].left()).abs() < 1.0, "rows not one under the other: {w:?}");
    }
    for r in &rows {
        assert!(dlg.contains_rect(*r), "a row runs out of the dialog: {r:?}");
        assert!(r.width() > 300.0, "a row is cut short: {r:?}");
    }

    // New: "New Toolbar 1", open and floating, selected with its name editable
    let new = on_dlg(&h, "New");
    click(&mut h, new.center());
    for _ in 0..3 {
        h.step();
    }
    let s = &h.state().settings;
    let made: Vec<_> = cz::custom_bars(s).collect();
    assert_eq!(made.len(), 1);
    assert_eq!(made[0].name, "New Toolbar 1");
    assert!(made[0].shown);
    let key = made[0].bar.clone();
    assert_eq!(h.state().dialogs.options.cmd_bar_sel, key);
    assert!(cz::drawn_area(&h.ctx, &key).is_some(), "the new bar doesn't float");
    save(&mut h, "issue_136_02_new_toolbar");
    // Delete takes it away
    let del = on_dlg(&h, "Delete");
    click(&mut h, del.center());
    assert_eq!(cz::custom_bars(&h.state().settings).count(), 0);

    // the Standard bar: its Style kept, and Reset
    let std_key = crate::ui::bar_named("Standard").unwrap().1.id;
    click(&mut h, rows[0].center());
    assert_eq!(h.state().dialogs.options.cmd_bar_sel, std_key);
    // the boxes: Global button size, then Size and Style side by side
    let combo = h.query_all_by_role(egui::accesskit::Role::ComboBox).map(|n| n.rect()).filter(|r| dlg.contains_rect(*r)).max_by(|a, b| (a.top() + a.left() / 10_000.0).total_cmp(&(b.top() + b.left() / 10_000.0))).expect("the Style box");
    click(&mut h, combo.center());
    for _ in 0..2 {
        h.step();
    }
    let opt = h.query_all_by_label("Text only").map(|n| n.rect()).last().expect("Text only in the Style list");
    click(&mut h, opt.center());
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(cz::look(&h.state().settings, std_key).0, 2, "the Style wasn't kept");
    save(&mut h, "issue_136_03_standard_text_only");
    let reset = on_dlg(&h, "Reset");
    click(&mut h, reset.center());
    assert_eq!(cz::look(&h.state().settings, std_key).0, 0, "Reset left the style");
    assert!(cz::layout(&h.state().settings, std_key).is_none());
}

/// Press at `a` with `mods`, move to `b` in steps and let go there.
fn drag_with(h: &mut Harness<'_, AnyVectorApp>, a: egui::Pos2, b: egui::Pos2, mods: egui::Modifiers) {
    h.input_mut().events.push(egui::Event::ModifiersChanged(mods));
    move_to(h, a);
    h.step();
    h.input_mut().events.push(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: mods });
    h.step();
    for k in 1..=8 {
        move_to(h, a + (b - a) * (k as f32 / 8.0));
        h.step();
    }
    h.input_mut().events.push(egui::Event::PointerButton { pos: b, button: egui::PointerButton::Primary, pressed: false, modifiers: mods });
    h.step();
    h.input_mut().events.push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.step();
}

fn bar_ids(h: &Harness<'_, AnyVectorApp>, key: &str) -> Vec<&'static str> {
    crate::customize::items_of(&h.state().settings, key).iter().map(|i| i.id).collect()
}

/// Where the item at `index` of the bar `key` was drawn.
fn item_rect(h: &Harness<'_, AnyVectorApp>, key: &str, index: usize) -> egui::Rect {
    crate::customize::drawn_items(&h.ctx, key).into_iter().find(|(i, _)| *i == index).unwrap_or_else(|| panic!("item {index} of {key} isn't drawn")).1
}

/// #124: Alt+drag moves a button along the Standard toolbar and off it (without running it),
/// Ctrl+Alt copies it to another bar, and right-click ▸ Customize ▸ Reset puts the bar back.
#[test]
fn issue_124_alt_drag_moves_copies_and_removes_buttons() {
    use crate::customize as cz;
    let mut h = doc_harness();
    let key = crate::ui::bar_named("Standard").unwrap().1.id;
    let before = bar_ids(&h, key);
    let docs = h.state().docs.len();
    // New (the first button) dragged to after Save (the third)
    let (new, save_) = (item_rect(&h, key, 0), item_rect(&h, key, 2));
    drag_with(&mut h, new.center(), save_.right_center() - egui::vec2(2.0, 0.0), egui::Modifiers::ALT);
    let after = bar_ids(&h, key);
    assert_eq!(&after[..3], &[before[1], before[2], before[0]], "New didn't move after Save");
    assert_eq!(after.len(), before.len());
    assert_eq!(h.state().docs.len(), docs, "the New button ran while being dragged");
    assert!(h.state().dialogs.new_doc.is_none(), "the New button ran while being dragged");
    // an Alt+click without moving does nothing
    let r = item_rect(&h, key, 0);
    drag_with(&mut h, r.center(), r.center(), egui::Modifiers::ALT);
    assert_eq!(bar_ids(&h, key), after);
    // dragged off every bar (onto the page): gone
    let canvas = h.state().doc().unwrap().canvas_rect.center();
    let r = item_rect(&h, key, 2);
    drag_with(&mut h, r.center(), canvas, egui::Modifiers::ALT);
    assert!(!bar_ids(&h, key).contains(&before[0]), "New wasn't taken off the bar");
    assert_eq!(bar_ids(&h, key).len(), before.len() - 1);
    // Ctrl+Alt copies Open onto the property bar; the Standard bar keeps it
    let pb = crate::propbar::bar_for(h.state()).and_then(crate::ui::bar).expect("a property bar").id;
    let pb_before = bar_ids(&h, pb);
    let open = item_rect(&h, key, 0);
    let target = item_rect(&h, pb, 0);
    drag_with(&mut h, open.center(), target.left_center() + egui::vec2(2.0, 0.0), egui::Modifiers::ALT | egui::Modifiers::CTRL);
    assert_eq!(bar_ids(&h, pb).len(), pb_before.len() + 1, "Ctrl+Alt didn't copy onto the property bar");
    assert_eq!(bar_ids(&h, pb)[0], before[1]);
    assert_eq!(bar_ids(&h, key)[0], before[1], "the copy took Open off the Standard bar");
    // kept with the settings, as part of the workspace
    assert!(crate::workspaces::Layout::of(&h.state().settings).bar_layouts.iter().any(|l| l.bar == key));

    // right-click ▸ Customize: Add Separator after the item clicked, Delete it, then Reset
    let r = item_rect(&h, key, 0);
    right_click(&mut h, r.center());
    assert_eq!(cz::context_target(), Some((key.to_string(), Some(0))), "the right-click didn't find the bar and its item");
    h.input_mut().events.push(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    h.step();
    h.state_mut().run(Cmd::CustomizeBar(cz::BarOp::AddSeparator));
    h.step();
    assert_eq!(cz::items_of(&h.state().settings, key)[1].kind, crate::ui::Kind::Separator);
    h.state_mut().run(Cmd::CustomizeBar(cz::BarOp::DeleteItem));
    h.step();
    // Open (the item clicked) gone, the separator after it now first
    assert_eq!(&bar_ids(&h, key)[..2], &["", before[2]], "Delete didn't take the item clicked off");
    h.state_mut().run(Cmd::CustomizeBar(cz::BarOp::Reset));
    h.step();
    assert_eq!(bar_ids(&h, key), before, "Reset didn't put the Standard bar back");
    assert!(cz::layout(&h.state().settings, key).is_none());
}

/// #124: with Options ▸ Customization open, a command dragged from the Commands list lands on
/// the Standard toolbar where it is dropped, and the buttons on the bars drag without Alt.
#[test]
fn issue_124_commands_drag_onto_a_bar_while_customizing() {
    use egui_kittest::kittest::Queryable;
    let mut h = doc_harness();
    let key = crate::ui::bar_named("Standard").unwrap().1.id;
    let before = bar_ids(&h, key);
    h.state_mut().open_options(crate::commands::OptionsPage::Customization, Some("Commands"));
    for _ in 0..4 {
        h.step();
    }
    // a command of the first menu with a button of its own somewhere
    let entries = crate::menus::catalog(crate::menus::catalog_menus()[0].1);
    let e = entries
        .iter()
        .find(|e| crate::customize::item_for_cmd(e.cmd).is_some_and(|it| !before.contains(&it.id)) && h.query_all_by_label_contains(crate::widgets::plain(&e.label).trim()).count() == 1)
        .expect("a command to drag");
    let id = crate::customize::item_for_cmd(e.cmd).unwrap().id;
    let row = h.query_by_label_contains(crate::widgets::plain(&e.label).trim()).unwrap().rect();
    let at = item_rect(&h, key, 1);
    drag_with(&mut h, row.center(), at.left_center() + egui::vec2(2.0, 0.0), egui::Modifiers::NONE);
    let after = bar_ids(&h, key);
    assert_eq!(after.len(), before.len() + 1, "the command didn't land on the bar");
    assert_eq!(after[1], id, "the command landed at {:?}", after.iter().position(|i| *i == id));
    assert!(h.state().dialogs.options.open, "the drop closed Options");
    // the buttons drag without Alt now: the new one taken back off
    let r = item_rect(&h, key, 1);
    let canvas = h.state().doc().unwrap().canvas_rect;
    drag_with(&mut h, r.center(), egui::pos2(canvas.left() + 20.0, canvas.bottom() - 20.0), egui::Modifiers::NONE);
    assert_eq!(bar_ids(&h, key), before);
    // Cancel keeps what was done on the bars
    let mut moved = bar_ids(&h, key);
    moved.swap(0, 1);
    let r0 = item_rect(&h, key, 0);
    let r1 = item_rect(&h, key, 1);
    drag_with(&mut h, r0.center(), r1.right_center() - egui::vec2(2.0, 0.0), egui::Modifiers::NONE);
    assert_eq!(bar_ids(&h, key), moved);
    let cancel = h.query_all_by_label("Cancel").last().expect("a Cancel button").rect();
    click(&mut h, cancel.center());
    assert!(!h.state().dialogs.options.open);
    assert_eq!(bar_ids(&h, key), moved, "Cancel threw away the toolbar change");
}

/// #124: the toolbox's buttons Alt+drag into a new order, and the property bar's Quick
/// Customize hides and shows its items.
#[test]
#[ignore]
fn visual_issue_124_toolbox_and_property_bar_customized() {
    use crate::customize as cz;
    use egui_kittest::kittest::Queryable;
    let mut h = doc_harness();
    let tb = crate::ui::bar_named("Toolbox").unwrap().1.id;
    let before = bar_ids(&h, tb);
    let (first, third) = (item_rect(&h, tb, 0), item_rect(&h, tb, 2));
    // the Pick tool's slot under the third slot
    drag_with(&mut h, first.center(), third.center_bottom() - egui::vec2(0.0, 2.0), egui::Modifiers::ALT);
    let after = bar_ids(&h, tb);
    assert_eq!(&after[..3], &[before[1], before[2], before[0]], "the toolbox didn't take the new order");
    assert_eq!(h.state().tool, crate::tools::ToolId::Pick);
    save(&mut h, "issue_124_01_toolbox_reordered");
    // right-click ▸ Customize ▸ Reset
    let r = item_rect(&h, tb, 0);
    right_click(&mut h, r.center());
    h.input_mut().events.push(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    h.step();
    h.state_mut().run(Cmd::CustomizeBar(cz::BarOp::Reset));
    h.step();
    assert_eq!(bar_ids(&h, tb), before);

    // the property bar's Quick Customize: an item unchecked goes, checked again comes back
    blue_rect(&mut h);
    for _ in 0..3 {
        h.step();
    }
    let pb = crate::propbar::bar_for(h.state()).and_then(crate::ui::bar).expect("a property bar").id;
    let pb_before = bar_ids(&h, pb);
    let qc = h.query_all_by_label("Quick Customize").map(|n| n.rect()).max_by(|a, b| a.left().total_cmp(&b.left())).expect("the property bar's Quick Customize");
    click(&mut h, qc.center());
    for _ in 0..2 {
        h.step();
    }
    save(&mut h, "issue_124_02_property_bar_quick_customize");
    // the boxes of the list (the first is "Property bar"); the menu follows the button, which
    // moves as the bar gets shorter
    let boxes = |h: &Harness<'_, AnyVectorApp>| -> Vec<egui::Rect> { h.query_all_by_role(egui::accesskit::Role::CheckBox).map(|n| n.rect()).filter(|r| r.top() > 60.0).collect() };
    assert!(boxes(&h).len() > 3, "Quick Customize lists the bar's items ({})", boxes(&h).len());
    let first = boxes(&h)[1];
    click(&mut h, first.center());
    let fewer = bar_ids(&h, pb);
    assert_eq!(fewer.len(), pb_before.len() - 1, "unchecking didn't hide an item");
    save(&mut h, "issue_124_02b_property_bar_item_hidden");
    let first = boxes(&h)[1];
    click(&mut h, first.center());
    assert_eq!(bar_ids(&h, pb), pb_before, "checking again didn't put it back in place");
    assert!(cz::layout(&h.state().settings, pb).is_none(), "the bar is back as it was, without a layout");
}

/// #124: a bar the user made (Command Bars ▸ New) takes commands and Alt+dragged buttons.
#[test]
#[ignore]
fn visual_issue_124_custom_bar_takes_buttons() {
    use crate::customize as cz;
    let mut h = doc_harness();
    let key = cz::new_bar(&mut h.state_mut().settings, "My Bar");
    for _ in 0..4 {
        h.step();
    }
    save(&mut h, "issue_124_03_custom_bar_empty");
    let std_key = crate::ui::bar_named("Standard").unwrap().1.id;
    let area = crate::customize::drawn_area(&h.ctx, &key).expect("the new bar is drawn");
    let open = item_rect(&h, std_key, 1);
    drag_with(&mut h, open.center(), area.center(), egui::Modifiers::ALT | egui::Modifiers::CTRL);
    assert_eq!(bar_ids(&h, &key).len(), 1, "the empty bar didn't take the button");
    let save_ = item_rect(&h, std_key, 2);
    let first = item_rect(&h, &key, 0);
    drag_with(&mut h, save_.center(), first.right_center() + egui::vec2(-2.0, 0.0), egui::Modifiers::ALT | egui::Modifiers::CTRL);
    assert_eq!(bar_ids(&h, &key), vec![bar_ids(&h, std_key)[1], bar_ids(&h, std_key)[2]]);
    save(&mut h, "issue_124_04_custom_bar_with_buttons");
    assert!(cz::custom_bars(&h.state().settings).any(|l| l.name == "My Bar" && l.shown));
}

// ---- File compatibility: Save As copies, the .cdr note, opening an unknown .cdr (#132) ----

/// The drawing of the #132 tests, on two pages: a red rectangle, a blue ellipse and a line of
/// text on the first, a green square on the second. Returns the document's index.
fn compat_drawing(h: &mut Harness<'_, AnyVectorApp>) -> usize {
    let d = h.state_mut().doc_mut().unwrap();
    let style = |fill: Color, outline: bool| {
        let mut st = Style::default();
        st.fill = Fill::Solid(fill);
        if outline {
            st.outline.color = Some(Color::BLACK);
            st.outline.width = 1.0;
        }
        st
    };
    let (w, ht) = (d.doc.pages[0].width, d.doc.pages[0].height);
    let ids = (d.doc.new_id(), d.doc.new_id(), d.doc.new_id(), d.doc.new_id());
    let text = vector_core::text::TextObj::artistic("Hello CorelDRAW", vector_core::text::CharFormat { size: 36.0, ..Default::default() });
    d.add_objects(
        "test",
        vec![
            Object::new(ids.0, ObjKind::Rect(RectShape { w: 200.0, h: 140.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((90.0, 90.0)), style(Color::rgb(220, 30, 30), true)),
            Object::new(ids.1, ObjKind::Ellipse(EllipseShape { w: 180.0, h: 120.0, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false }), Affine::translate((340.0, 100.0)), style(Color::rgb(30, 90, 220), false)),
            Object::new(ids.2, ObjKind::Text(text), Affine::translate((90.0, 330.0)), style(Color::BLACK, false)),
        ],
    );
    let p = d.doc.add_page(1, w, ht);
    d.doc.pages[p].layers[0].objects.push(Object::new(ids.3, ObjKind::Rect(RectShape { w: 150.0, h: 150.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((100.0, 100.0)), style(Color::rgb(0, 160, 60), true)));
    let i = h.state().active.unwrap();
    // the one-time note starts on, whatever the settings of this computer say
    set_cdr_notice(h, true);
    i
}

fn cdr_notice_on(h: &Harness<'_, AnyVectorApp>) -> Option<bool> {
    h.state().settings.warnings.iter().find(|(t, _)| t == crate::settings::CDR_NOTICE_WARNING).map(|w| w.1)
}

fn set_cdr_notice(h: &mut Harness<'_, AnyVectorApp>, on: bool) {
    let w = &mut h.state_mut().settings.warnings;
    match w.iter_mut().find(|(t, _)| t == crate::settings::CDR_NOTICE_WARNING) {
        Some(e) => e.1 = on,
        None => w.push((crate::settings::CDR_NOTICE_WARNING.to_string(), on)),
    }
}

/// #132: Save As CMX, PDF, SVG or EMF writes a copy other programs (CorelDRAW among them) open,
/// with every page; the drawing keeps its own .cdr, stays modified, and the copies do not go into
/// the recent files. What a CMX copy cannot hold is said.
#[test]
fn save_as_other_formats_writes_copies_and_keeps_the_drawing() {
    let mut h = dialog_harness();
    let i = compat_drawing(&mut h);
    let cdr = temp_file("compat_copies", "drawing.cdr");
    let dir = cdr.parent().unwrap().to_path_buf();
    h.state_mut().docs[i].path = Some(cdr.clone());
    assert!(h.state_mut().save_doc(i, false), "the drawing saves");
    assert_eq!(vector_io::cdr::detect(&std::fs::read(&cdr).unwrap()), vector_io::cdr::CdrKind::AnyVector, "as AnyVector's own .cdr");
    h.state_mut().dialogs.message_box = None;
    // an edit, so the drawing is modified while the copies are written
    h.state_mut().run(Cmd::SelectAll);
    h.state_mut().run(Cmd::Duplicate);
    settle(&mut h);
    assert!(h.state().docs[i].history.is_modified());

    for ext in ["cmx", "pdf", "svg", "emf"] {
        let p = dir.join(format!("copy.{ext}"));
        h.state_mut().dialogs.message_box = None;
        assert!(!h.state_mut().save_copy(i, &p), "a copy is not the drawing saved");
        let files: Vec<std::path::PathBuf> = if matches!(ext, "svg" | "emf") { vec![dir.join(format!("copy-1.{ext}")), dir.join(format!("copy-2.{ext}"))] } else { vec![p.clone()] };
        for f in &files {
            let b = std::fs::read(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
            match ext {
                "cmx" => assert!(&b[0..4] == b"RIFF" && &b[8..12] == b"CMX1", "a Corel metafile"),
                "pdf" => assert!(b.starts_with(b"%PDF-")),
                "svg" => assert!(String::from_utf8_lossy(&b).contains("<svg")),
                _ => assert_eq!(&b[40..44], b" EMF", "an enhanced metafile"),
            }
        }
        let d = &h.state().docs[i];
        assert_eq!(d.path.as_deref(), Some(cdr.as_path()), "the drawing keeps its file");
        assert!(d.history.is_modified(), "and is still modified");
        assert!(!h.state().settings.recent_files.iter().any(|r| r.extension().is_some_and(|e| e == ext)), "copies are not recent drawings");
        if ext == "cmx" {
            let (_, text) = h.state().dialogs.message_box.clone().expect("the differences are said");
            assert!(text.contains("copy.cmx was saved with these differences") && text.contains("Text was converted to curves."), "{text}");
        } else {
            assert!(h.state().dialogs.message_box.is_none(), "{ext}: nothing to say");
        }
    }
    // the CMX copy reads back with both pages and the shapes where they were
    let back = vector_io::open(&dir.join("copy.cmx")).unwrap();
    assert_eq!(back.pages.len(), 2);
    let shapes: Vec<_> = back.pages[0].all_objects().filter(|o| !matches!(o.kind, ObjKind::Group(_))).collect();
    assert!(shapes.iter().any(|o| o.style.fill == Fill::Solid(Color::rgb(220, 30, 30))), "the red rectangle");
    // Save with the path of a copy type also writes a copy (what Save As does after the dialog)
    h.state_mut().docs[i].path = Some(dir.join("via_save.pdf"));
    assert!(!h.state_mut().save_doc(i, false));
    assert!(dir.join("via_save.pdf").exists());
    assert!(h.state().docs[i].history.is_modified());
    h.state_mut().docs[i].path = Some(cdr);
    h.step();
}

/// #132: the first .cdr save says, once, that CorelDRAW does not open AnyVector's .cdr and how to
/// get a file it opens; Options ▸ AnyVector ▸ Save turns the note back on.
#[test]
fn the_first_cdr_save_says_once_that_coreldraw_cannot_open_it() {
    let mut h = dialog_harness();
    let i = compat_drawing(&mut h);
    let cdr = temp_file("compat_notice", "first.cdr");
    h.state_mut().docs[i].path = Some(cdr.clone());
    assert_eq!(cdr_notice_on(&h), Some(true));
    assert!(h.state_mut().save_doc(i, false));
    let (title, text) = h.state().dialogs.message_box.clone().expect("the note shows");
    assert_eq!(title, "Save");
    assert!(text.contains("CorelDRAW cannot open this file") && text.contains("CMX (CorelDRAW-compatible), PDF, SVG or EMF"), "{text}");
    settle(&mut h);
    click_label(&mut h, "OK");
    assert!(h.state().dialogs.message_box.is_none());
    assert_eq!(cdr_notice_on(&h), Some(false), "acknowledged");
    // the next save is quiet
    assert!(h.state_mut().save_doc(i, false));
    assert!(h.state().dialogs.message_box.is_none(), "shown once");
    // a .vct save never shows it
    let vct = cdr.with_extension("vct");
    set_cdr_notice(&mut h, true);
    h.state_mut().docs[i].path = Some(vct);
    assert!(h.state_mut().save_doc(i, false));
    assert!(h.state().dialogs.message_box.is_none());
    set_cdr_notice(&mut h, false);
    h.state_mut().docs[i].path = Some(cdr);

    // Options ▸ AnyVector ▸ Save has the switch, the same as its Warnings entry
    h.state_mut().open_options(crate::commands::OptionsPage::App, Some("Save"));
    settle(&mut h);
    let label = "Remind me that CorelDRAW cannot open AnyVector .cdr files";
    assert!(has_label(&h, label), "the Save page shows the switch");
    click_label(&mut h, label);
    assert_eq!(cdr_notice_on(&h), Some(true));
    h.state_mut().dialogs.options.open = false;
    settle(&mut h);
    assert!(h.state_mut().save_doc(i, false));
    assert!(h.state().dialogs.message_box.is_some(), "on again, it shows again");
    h.state_mut().dialogs.message_box = None;

    // settings from an older version get the note (on) and the honest version name
    let mut s = crate::settings::Settings::default();
    s.warnings.retain(|(t, _)| t != crate::settings::CDR_NOTICE_WARNING);
    s.default_version = "0.1 (2026.0)".into();
    s.migrate();
    assert!(s.warnings.iter().any(|(t, on)| t == crate::settings::CDR_NOTICE_WARNING && *on));
    assert_eq!(s.default_version, crate::settings::FILE_VERSIONS[0]);
    // an acknowledged note stays off
    let n = s.warnings.len();
    s.warnings.iter_mut().for_each(|w| w.1 = false);
    s.migrate();
    assert_eq!(s.warnings.len(), n);
    assert!(s.warnings.iter().all(|w| !w.1));
}

/// #132: a .cdr that neither AnyVector nor CorelDRAW wrote is explained, not "Invalid file"; a
/// damaged CorelDRAW file still says why; AnyVector's CMX copy opens again.
#[test]
fn opening_a_cdr_no_one_recognises_explains_what_it_is() {
    let mut h = dialog_harness();
    let i = compat_drawing(&mut h);
    let other = temp_file("compat_open", "other.cdr");
    let dir = other.parent().unwrap().to_path_buf();
    std::fs::write(&other, b"made by some other program").unwrap();
    let docs = h.state().docs.len();
    assert!(!h.state_mut().open_path(&other));
    let (title, text) = h.state().dialogs.message_box.clone().expect("the user is told");
    assert_eq!(title, "Open");
    assert!(text.starts_with("other.cdr was not written by AnyVector or CorelDRAW, or it is damaged."), "{text}");
    assert!(text.contains("open a PDF, SVG or EPS copy") && !text.contains("Invalid file"), "{text}");
    settle(&mut h);
    click_label(&mut h, "OK");
    // a cut-off CorelDRAW file is damaged: it says so the usual way
    let damaged = dir.join("damaged.cdr");
    std::fs::write(&damaged, b"WL cut off").unwrap();
    assert!(!h.state_mut().open_path(&damaged));
    let (_, text) = h.state().dialogs.message_box.clone().unwrap();
    assert!(text.starts_with("Could not open"), "{text}");
    h.state_mut().dialogs.message_box = None;
    assert_eq!(h.state().docs.len(), docs);
    // the CMX copy opens as a new drawing with both pages
    let cmx = dir.join("copy.cmx");
    h.state_mut().save_copy(i, &cmx);
    h.state_mut().dialogs.message_box = None;
    assert!(h.state_mut().open_path(&cmx));
    assert_eq!(h.state().docs.len(), docs + 1);
    assert_eq!(h.state().doc().unwrap().doc.pages.len(), 2);
    h.step();
}

/// #132 in pictures: the note after the first .cdr save, the message for an unknown .cdr, the
/// switch on Options ▸ AnyVector ▸ Save, CMX in Export, and the CMX copy opened again.
#[test]
#[ignore]
fn visual_issue_132_file_compatibility() {
    let mut h = doc_harness();
    let i = compat_drawing(&mut h);
    let cdr = temp_file("visual_132", "Drawing.cdr");
    let dir = cdr.parent().unwrap().to_path_buf();
    h.state_mut().docs[i].path = Some(cdr);
    assert!(h.state_mut().save_doc(i, false));
    assert!(h.state().dialogs.message_box.is_some());
    save(&mut h, "issue_132_01_first_cdr_save_note");
    h.state_mut().dialogs.message_box = None;

    let other = dir.join("from another program.cdr");
    std::fs::write(&other, b"not a drawing").unwrap();
    h.state_mut().open_path(&other);
    save(&mut h, "issue_132_02_unknown_cdr");
    h.state_mut().dialogs.message_box = None;

    h.state_mut().open_options(crate::commands::OptionsPage::App, Some("Save"));
    save(&mut h, "issue_132_03_options_save_reminder");
    click_label(&mut h, "OK");
    assert!(!h.state().dialogs.options.open);

    h.state_mut().run(Cmd::Export);
    settle(&mut h);
    let cmx_type = vector_io::EXPORT_TYPES.iter().position(|t| t.extensions[0] == "cmx").unwrap();
    if let Some(st) = h.state_mut().dialogs.export.as_mut() {
        st.format = cmx_type;
        st.path = dir.join("Drawing.cmx").display().to_string();
    }
    save(&mut h, "issue_132_04_export_cmx");
    h.state_mut().dialogs.export = None;
    h.step();

    let cmx = dir.join("Drawing copy.cmx");
    h.state_mut().save_copy(i, &cmx);
    save(&mut h, "issue_132_05_cmx_copy_differences");
    h.state_mut().dialogs.message_box = None;
    assert!(h.state_mut().open_path(&cmx));
    save(&mut h, "issue_132_06_cmx_copy_reopened");
}

// ---- input and keyboard (#119, #130, #140, #121, #125) ----

/// A key press and release of `key` in one frame, with its physical key and modifiers.
fn key_tap(h: &mut Harness<'_, AnyVectorApp>, key: egui::Key, physical: Option<egui::Key>, mods: egui::Modifiers) {
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::Key { key, physical_key: physical, pressed, repeat: false, modifiers: mods });
    }
}

/// The numeric keypad's +: egui-winit reports it as Plus on the physical Plus key.
fn keypad_plus(h: &mut Harness<'_, AnyVectorApp>) {
    key_tap(h, egui::Key::Plus, Some(egui::Key::Plus), egui::Modifiers::NONE);
}

fn bounds_of(h: &Harness<'_, AnyVectorApp>, id: u64) -> kurbo::Rect {
    h.state().doc().unwrap().doc.find(id).unwrap().bounds()
}

fn same_rect(a: kurbo::Rect, b: kurbo::Rect) -> bool {
    (a.x0 - b.x0).abs() < 1e-3 && (a.y0 - b.y0).abs() < 1e-3 && (a.x1 - b.x1).abs() < 1e-3 && (a.y1 - b.y1).abs() < 1e-3
}

/// The ids of the objects on the page, in stacking order.
fn page_ids(h: &Harness<'_, AnyVectorApp>) -> Vec<u64> {
    h.state().doc().unwrap().page_ref().all_objects().map(|o| o.id).collect()
}

/// #119: the numeric keypad's + duplicates the selection in place (no offset), selects the copy
/// and is one undo step, as in CorelDRAW; the + of the main keys (Shift+= on a US keyboard) does
/// not; with the Shape tool and nodes selected + still adds a node.
#[test]
fn keypad_plus_duplicates_in_place() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let id = blue_rect(&mut h);
    h.step();
    let n0 = undo_steps(&h);
    let b0 = bounds_of(&h, id);
    keypad_plus(&mut h);
    for _ in 0..3 {
        h.step();
    }
    let ids = page_ids(&h);
    assert_eq!(ids.len(), 2, "the keypad + made no copy: {ids:?}");
    let sel = h.state().doc().unwrap().selection.clone();
    assert_eq!(sel.len(), 1);
    assert_ne!(sel[0], id, "the copy is selected");
    assert!(same_rect(bounds_of(&h, sel[0]), b0), "the copy is not exactly on top of the original");
    assert_eq!(*ids.last().unwrap(), sel[0], "the copy is in front");
    assert_eq!(undo_steps(&h), n0 + 1, "one undo step");
    undo(&mut h);
    assert_eq!(page_ids(&h).len(), 1);
    // the main keys' + (Shift+= on a US keyboard, its physical key is =): nothing
    h.state_mut().doc_mut().unwrap().selection = vec![id];
    key_tap(&mut h, egui::Key::Plus, Some(egui::Key::Equals), egui::Modifiers::SHIFT);
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(page_ids(&h).len(), 1, "the main keys' + duplicated");
    // the Shape tool with two nodes selected: the keypad + adds a node there, no copy
    let (mut h, cid) = shape_object(line_curve(&[(100.0, 100.0), (200.0, 100.0), (300.0, 100.0)]), (0.0, 0.0));
    h.state_mut().doc_mut().unwrap().node_sel = vec![(cid, 0, 0), (cid, 0, 1)];
    let count = page_ids(&h).len();
    keypad_plus(&mut h);
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(page_ids(&h).len(), count, "the Shape tool's + duplicated");
    assert!(matches!(kind_of(&h, cid), ObjKind::Path(np) if np.node_count() == 4), "no node was added");
}

/// #119: the keypad + while dragging an object leaves the original where it was (the drop is a
/// copy), and the pointer says so.
#[test]
fn keypad_plus_while_dragging_leaves_the_original() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let id = blue_rect(&mut h);
    h.step();
    let b0 = bounds_of(&h, id);
    let (a, b) = (scr(&h, (200.0, 160.0)), scr(&h, (500.0, 160.0)));
    let mut k = 0;
    let mut cursor = None;
    drag_watching(&mut h, a, b, |h| {
        k += 1;
        if k == 2 {
            keypad_plus(h);
        }
        if k == 5 {
            cursor = Some(crate::canvas::cursor_for(h.state(), b));
        }
    });
    h.step();
    assert_eq!(cursor, Some(egui::CursorIcon::Copy), "the pointer doesn't show a copy");
    let ids = page_ids(&h);
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert!(same_rect(bounds_of(&h, id), b0), "the original moved");
    let copy = h.state().doc().unwrap().selection[0];
    assert_ne!(copy, id);
    assert!(same_rect(bounds_of(&h, copy), b0 + kurbo::Vec2::new(300.0, 0.0)), "the copy isn't where it was dropped: {:?}", bounds_of(&h, copy));
}

/// #119: Virtual Segment Delete shows the piece a click deletes in red and a hand over it; away
/// from any shape there is nothing to show and the pointer is the crosshair.
#[test]
fn virtual_delete_shows_the_piece_under_the_pointer() {
    use crate::tools::ToolId;
    use egui::CursorIcon;
    let mut h = join_harness(ToolId::VirtualDelete);
    h.state_mut().settings.crosshair_cursor = false;
    let (id, _) = unfilled_rect(&mut h);
    let side = scr(&h, (280.0, 100.0));
    move_to(&mut h, side);
    h.step();
    h.step();
    let piece = h.state().ts.hover_cut.and_then(|(_, _, c)| c).expect("no piece under the pointer");
    // the top side of the rectangle, from corner to corner
    let (p0, p3) = (piece.p0, piece.p3);
    assert!((p0.y - 100.0).abs() < 1e-6 && (p3.y - 100.0).abs() < 1e-6, "{piece:?}");
    assert!((p0.x.min(p3.x) - 100.0).abs() < 1e-6 && (p0.x.max(p3.x) - 400.0).abs() < 1e-6, "{piece:?}");
    assert_eq!(crate::canvas::cursor_for(h.state(), side), CursorIcon::PointingHand);
    // the document changed: looked for again (the side is gone, nothing more to delete there)
    click(&mut h, side);
    h.step();
    let ObjKind::Path(np) = &h.state().doc().unwrap().doc.find(id).unwrap().kind else { panic!("not deleted") };
    assert_eq!(np.subpaths.iter().map(|s| s.segment_count()).sum::<usize>(), 3);
    assert!(h.state().ts.hover_cut.is_some_and(|(_, _, c)| c.is_none()), "the deleted side still shows");
    assert_ne!(crate::canvas::cursor_for(h.state(), side), CursorIcon::PointingHand);
    let empty = scr(&h, (700.0, 600.0));
    move_to(&mut h, empty);
    h.step();
    h.step();
    assert!(h.state().ts.hover_cut.is_some_and(|(_, _, c)| c.is_none()));
    assert_eq!(crate::canvas::cursor_for(h.state(), empty), CursorIcon::Crosshair);
}

/// Space pressed (a fresh press, or a key repeat of a held one).
fn space(h: &mut Harness<'_, AnyVectorApp>, repeat: bool) {
    h.input_mut().events.push(egui::Event::Key { key: egui::Key::Space, physical_key: Some(egui::Key::Space), pressed: true, repeat, modifiers: egui::Modifiers::NONE });
}

fn space_up(h: &mut Harness<'_, AnyVectorApp>) {
    h.input_mut().events.push(egui::Event::Key { key: egui::Key::Space, physical_key: Some(egui::Key::Space), pressed: false, repeat: false, modifiers: egui::Modifiers::NONE });
}

/// #130: Space while dragging with the Pick tool leaves a copy where the objects are at that
/// moment and the drag goes on, as in CorelDRAW; each copy is one undo step and a held Space
/// (key repeats) leaves no more. Space with nothing dragged still switches tools.
#[test]
fn space_while_dragging_leaves_copies() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let id = blue_rect(&mut h);
    h.step();
    let n0 = undo_steps(&h);
    let b0 = bounds_of(&h, id);
    let (a, b) = (scr(&h, (200.0, 160.0)), scr(&h, (500.0, 160.0)));
    let mut k = 0;
    let mut tools = vec![];
    drag_watching(&mut h, a, b, |h| {
        k += 1;
        match k {
            2 | 4 => space(h, false),
            3 => {
                space(h, true);
                space(h, true);
                space_up(h);
            }
            _ => space_up(h),
        }
        tools.push(h.state().tool);
    });
    h.step();
    assert!(tools.iter().all(|t| *t == ToolId::Pick), "Space switched tools during the drag: {tools:?}");
    let ids = page_ids(&h);
    assert_eq!(ids.len(), 3, "two copies (the key repeats made none): {ids:?}");
    // the original was dragged all the way; the copies stayed on the way, in order
    assert!(same_rect(bounds_of(&h, id), b0 + kurbo::Vec2::new(300.0, 0.0)), "{:?}", bounds_of(&h, id));
    let stamps: Vec<kurbo::Rect> = ids.iter().filter(|i| **i != id && bounds_of(&h, **i).width() > 150.0).map(|i| bounds_of(&h, *i)).collect();
    assert_eq!(stamps.len(), 2);
    for s in &stamps {
        assert!(s.x0 > b0.x0 + 1.0 && s.x0 < b0.x0 + 299.0, "a copy is not on the way: {s:?}");
        assert!((s.y0 - b0.y0).abs() < 1e-6 && (s.width() - b0.width()).abs() < 1e-6);
    }
    assert!(stamps[0].x0 < stamps[1].x0 - 1.0, "the second copy is farther: {stamps:?}");
    assert_eq!(h.state().doc().unwrap().selection, vec![id], "the dragged object stays selected");
    assert_eq!(undo_steps(&h), n0 + 3, "a step per copy and one for the move");
    undo(&mut h);
    assert!(same_rect(bounds_of(&h, id), b0));
    assert_eq!(page_ids(&h).len(), 3, "undoing the move keeps the copies");
    undo(&mut h);
    assert_eq!(page_ids(&h).len(), 2);
    // no drag: Space switches to the Pick tool and back
    h.state_mut().set_tool(ToolId::Rectangle);
    for _ in 0..2 {
        space(&mut h, false);
        h.step();
        space_up(&mut h);
        h.step();
    }
    assert_eq!(h.state().tool, ToolId::Rectangle, "Space and Space again is back to the Rectangle tool");
    space(&mut h, false);
    h.step();
    assert_eq!(h.state().tool, ToolId::Pick);
}

/// #130: Escape ends the drag with the original back where it was; the copies Space left stay.
/// A rotation leaves rotated copies.
#[test]
fn space_copies_kept_after_escape_and_rotated() {
    use crate::tools::{Drag, ToolId, Xform};
    let mut h = join_harness(ToolId::Pick);
    let id = blue_rect(&mut h);
    h.step();
    let b0 = bounds_of(&h, id);
    let (a, b) = (scr(&h, (200.0, 160.0)), scr(&h, (200.0, 400.0)));
    move_to(&mut h, a);
    h.step();
    press(&mut h, a, true);
    h.step();
    for k in 1..=4 {
        move_to(&mut h, a + (b - a) * (k as f32 / 4.0));
        h.step();
    }
    space(&mut h, false);
    h.step();
    space_up(&mut h);
    key(&mut h, egui::Key::Escape, true);
    h.step();
    key(&mut h, egui::Key::Escape, false);
    press(&mut h, b, false);
    h.step();
    assert!(same_rect(bounds_of(&h, id), b0), "Escape didn't put the original back");
    let ids = page_ids(&h);
    assert_eq!(ids.len(), 2, "the copy was lost: {ids:?}");
    let copy = *ids.iter().find(|i| **i != id && bounds_of(&h, **i).width() > 150.0).unwrap();
    assert!(same_rect(bounds_of(&h, copy), b0 + kurbo::Vec2::new(0.0, 240.0)), "{:?}", bounds_of(&h, copy));
    // a rotation by 90 degrees around the middle: the copy stands upright
    let bb = h.state().doc().unwrap().selection_bounds().unwrap();
    h.state_mut().ts.drag = Drag::Transform { kind: Xform::Rotate, start: Point::new(300.0, 160.0), bb, ids: vec![id], center: bb.center() };
    h.state_mut().ts.cur = Point::new(200.0, 260.0);
    assert!(crate::tools::drop::stamp_copy(h.state_mut()));
    let r = bounds_of(&h, *h.state().ts.stamps.last().unwrap());
    assert!((r.width() - b0.height()).abs() < 1e-6 && (r.height() - b0.width()).abs() < 1e-6, "{r:?}");
    assert!((r.center() - b0.center()).hypot() < 1e-6);
    h.state_mut().ts.cancel();
}

/// A right-button press, a drag in steps, and the release.
fn right_drag_to(h: &mut Harness<'_, AnyVectorApp>, a: egui::Pos2, b: egui::Pos2, mut during: impl FnMut(&mut Harness<'_, AnyVectorApp>)) {
    for _ in 0..30 {
        h.step();
    }
    move_to(h, a);
    h.step();
    h.input_mut().events.push(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Secondary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.step();
    for k in 1..=6 {
        move_to(h, a + (b - a) * (k as f32 / 6.0));
        h.step();
        during(h);
    }
    h.input_mut().events.push(egui::Event::PointerButton { pos: b, button: egui::PointerButton::Secondary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.step();
    h.step();
}

/// A blue rectangle (selected) and a red, outlined one to its right, not selected.
fn two_rects(h: &mut Harness<'_, AnyVectorApp>) -> (u64, u64) {
    let blue = blue_rect(h);
    let d = h.state_mut().doc_mut().unwrap();
    let id = d.doc.new_id();
    let mut st = Style::default();
    st.fill = Fill::Solid(Color::rgb(0xe0, 0x30, 0x30));
    st.outline.color = Some(Color::rgb(0, 0x80, 0));
    st.outline.width = 6.0;
    d.add_objects("test", vec![Object::new(id, ObjKind::Rect(RectShape { w: 160.0, h: 160.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((500.0, 100.0)), st)]);
    d.selection = vec![blue];
    (blue, id)
}

fn fill_of(h: &Harness<'_, AnyVectorApp>, id: u64) -> Fill {
    h.state().doc().unwrap().doc.find(id).unwrap().style.fill.clone()
}

/// #140: dragging the selection with the right button ends in a menu, as in CorelDRAW: over
/// another object Move Here, Copy Here, Copy Fill / Outline / All Properties and PowerClip
/// Inside; over empty space Move Here and Copy Here. Nothing changes until a row is chosen; each
/// choice is one undo step.
#[test]
fn right_drag_ends_in_the_drop_menu() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let (blue, red) = two_rects(&mut h);
    h.step();
    let b0 = bounds_of(&h, blue);
    let blue_fill = fill_of(&h, blue);
    let red_style = h.state().doc().unwrap().doc.find(red).unwrap().style.clone();
    let n0 = undo_steps(&h);
    let (a, over_red, empty) = (scr(&h, (200.0, 160.0)), scr(&h, (580.0, 180.0)), scr(&h, (400.0, 500.0)));
    let mut cursor = None;
    right_drag_to(&mut h, a, over_red, |h| cursor = Some(crate::canvas::cursor_for(h.state(), over_red)));
    assert_eq!(cursor, Some(egui::CursorIcon::ContextMenu));
    let drop = h.state().ts.right_drop.clone().expect("no drop menu");
    assert_eq!(drop.target, Some(red));
    for l in ["Move Here", "Copy Here", "Copy Fill Here", "Copy Outline Here", "Copy All Properties", "PowerClip Inside", "Cancel"] {
        assert!(has_label(&h, l), "the menu has no {l}");
    }
    assert!(same_rect(bounds_of(&h, blue), b0), "the drag moved the object before a choice");
    assert_eq!(undo_steps(&h), n0);
    // Copy Fill Here: the red one gets the blue fill and keeps its outline; the blue one stays
    click_label(&mut h, "Copy Fill Here");
    assert!(h.state().ts.right_drop.is_none(), "the menu stayed open");
    assert_eq!(fill_of(&h, red), blue_fill);
    assert_eq!(h.state().doc().unwrap().doc.find(red).unwrap().style.outline, red_style.outline);
    assert!(same_rect(bounds_of(&h, blue), b0));
    assert_eq!(h.state().doc().unwrap().selection, vec![blue]);
    assert_eq!(undo_steps(&h), n0 + 1);
    undo(&mut h);
    assert_eq!(fill_of(&h, red), red_style.fill);
    // Copy Outline Here
    let blue_outline = h.state().doc().unwrap().doc.find(blue).unwrap().style.outline.clone();
    right_drag_to(&mut h, a, over_red, |_| {});
    click_label(&mut h, "Copy Outline Here");
    let st = h.state().doc().unwrap().doc.find(red).unwrap().style.clone();
    assert_eq!(st.outline, blue_outline);
    assert_eq!(st.fill, red_style.fill, "Copy Outline changed the fill");
    undo(&mut h);
    // Copy Here over empty space: a copy there, the original stays; the menu has only Move and Copy
    right_drag_to(&mut h, a, empty, |_| {});
    let drop = h.state().ts.right_drop.clone().expect("no drop menu over empty space");
    assert_eq!(drop.target, None);
    assert!(has_label(&h, "Move Here") && has_label(&h, "Copy Here") && !has_label(&h, "Copy Fill Here") && !has_label(&h, "PowerClip Inside"));
    let delta = kurbo::Vec2::new(200.0, 340.0);
    click_label(&mut h, "Copy Here");
    assert_eq!(page_ids(&h).len(), 3);
    assert!(same_rect(bounds_of(&h, blue), b0));
    let copy = h.state().doc().unwrap().selection[0];
    assert_ne!(copy, blue);
    assert!(same_rect(bounds_of(&h, copy), b0 + delta), "{:?}", bounds_of(&h, copy));
    undo(&mut h);
    assert_eq!(page_ids(&h).len(), 2);
    // Move Here
    h.state_mut().doc_mut().unwrap().selection = vec![blue];
    right_drag_to(&mut h, a, empty, |_| {});
    click_label(&mut h, "Move Here");
    assert!(same_rect(bounds_of(&h, blue), b0 + delta));
    assert_eq!(undo_steps(&h), n0 + 1);
    undo(&mut h);
    assert!(same_rect(bounds_of(&h, blue), b0));
    // PowerClip Inside: the blue one goes into the red one where it was dropped
    h.state_mut().doc_mut().unwrap().selection = vec![blue];
    right_drag_to(&mut h, a, over_red, |_| {});
    click_label(&mut h, "PowerClip Inside");
    let d = h.state().doc().unwrap();
    assert!(d.doc.find(blue).is_some(), "the contents are gone");
    let clip = d.selection[0];
    assert!(matches!(&d.doc.find(clip).unwrap().kind, ObjKind::PowerClip { .. }), "no PowerClip");
    assert_eq!(page_ids(&h).len(), 1, "{:?}", page_ids(&h));
    undo(&mut h);
    assert_eq!(page_ids(&h).len(), 2);
    assert!(same_rect(bounds_of(&h, blue), b0));
    // Escape closes the menu and nothing changes
    h.state_mut().doc_mut().unwrap().selection = vec![blue];
    right_drag_to(&mut h, a, over_red, |_| {});
    assert!(h.state().ts.right_drop.is_some());
    let n = undo_steps(&h);
    key(&mut h, egui::Key::Escape, true);
    h.step();
    key(&mut h, egui::Key::Escape, false);
    h.step();
    h.step();
    assert!(h.state().ts.right_drop.is_none(), "Escape didn't close the menu");
    assert!(!has_label(&h, "Copy Fill Here"));
    assert_eq!(undo_steps(&h), n);
    assert!(same_rect(bounds_of(&h, blue), b0));
}

/// #140: a right-click that does not move still opens the right-click menu (no drop menu), and
/// Copy All Properties gives a group's objects the properties.
#[test]
fn right_click_still_opens_the_context_menu() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let (blue, red) = two_rects(&mut h);
    h.step();
    let at = scr(&h, (200.0, 160.0));
    right_click(&mut h, at);
    h.step();
    assert!(h.state().ts.right_drop.is_none() && h.state().ts.right_drag.is_none());
    assert!(!has_label(&h, "Move Here"), "a right-click opened the drop menu");
    assert!(egui::Popup::is_any_open(&h.ctx), "no right-click menu");
    key(&mut h, egui::Key::Escape, true);
    h.step();
    key(&mut h, egui::Key::Escape, false);
    h.step();
    // Copy All Properties onto a group: every object of it gets them; a locked one is left alone
    let src = h.state().doc().unwrap().doc.find(blue).unwrap().clone();
    let d = h.state_mut().doc_mut().unwrap();
    let o = d.doc.find(red).unwrap().clone();
    let mut kid = o.clone();
    kid.id = d.doc.new_id();
    let mut g = Object::new(d.doc.new_id(), ObjKind::Group(vec![o.clone(), kid]), Affine::IDENTITY, Style::default());
    g.id = red;
    let mut doc = d.doc.clone();
    *doc.find_mut(red).unwrap() = g;
    crate::tools::drop::copy_properties(&mut doc, &src, &[red], crate::tools::drop::Props::ALL);
    let ObjKind::Group(kids) = &doc.find(red).unwrap().kind else { unreachable!() };
    assert!(kids.iter().all(|k| k.style.fill == src.style.fill && k.style.outline == src.style.outline));
    doc.find_mut(red).unwrap().locked = true;
    crate::tools::drop::copy_properties(&mut doc, &o, &[red], crate::tools::drop::Props::FILL);
    let ObjKind::Group(kids) = &doc.find(red).unwrap().kind else { unreachable!() };
    assert!(kids.iter().all(|k| k.style.fill == src.style.fill), "a locked object was changed");
}

/// #121: Tab from one property bar field to the next selects the whole value, so typing replaces
/// it, as in CorelDRAW and Windows; a click in a field places the caret instead.
#[test]
fn tab_into_a_property_bar_field_selects_its_value() {
    use crate::tools::ToolId;
    use egui::text_edit::TextEditState;
    use egui_kittest::kittest::Queryable;
    let mut h = join_harness(ToolId::Pick);
    blue_rect(&mut h);
    for _ in 0..3 {
        h.step();
    }
    // the X field of the property bar: the top left text field
    let fields: Vec<egui::Rect> = h.query_all_by_role(egui::accesskit::Role::TextInput).map(|n| n.rect()).filter(|r| r.top() < 120.0).collect();
    let x = fields.iter().copied().min_by(|a, b| (a.top() + a.left()).total_cmp(&(b.top() + b.left()))).expect("no property bar fields");
    click(&mut h, x.center());
    h.step();
    let range = |h: &Harness<'_, AnyVectorApp>| {
        let id = h.ctx.memory(|m| m.focused()).expect("no field has the focus");
        let st = TextEditState::load(&h.ctx, id).unwrap_or_default();
        let r = st.cursor.char_range().expect("no cursor");
        (id, r.primary.index.0.min(r.secondary.index.0), r.primary.index.0.max(r.secondary.index.0))
    };
    let (clicked, s, e) = range(&h);
    assert_eq!(s, e, "a click selected the value instead of placing the caret");
    key_tap(&mut h, egui::Key::Tab, Some(egui::Key::Tab), egui::Modifiers::NONE);
    h.step();
    h.step();
    let (next, s, e) = range(&h);
    assert_ne!(next, clicked, "Tab didn't move to the next field");
    let text = h.query_all_by_role(egui::accesskit::Role::TextInput).find(|n| n.is_focused()).and_then(|n| n.value()).unwrap_or_default();
    assert!(!text.is_empty());
    assert_eq!((s, e), (0, text.chars().count()), "Tab didn't select all of {text:?}");
    // typing replaces it
    h.input_mut().events.push(egui::Event::Text("7".into()));
    h.step();
    h.step();
    let now = h.query_all_by_role(egui::accesskit::Role::TextInput).find(|n| n.is_focused()).and_then(|n| n.value()).unwrap_or_default();
    assert_eq!(now, "7", "typing didn't replace the value");
    // Shift+Tab back: selected too
    key_tap(&mut h, egui::Key::Tab, Some(egui::Key::Tab), egui::Modifiers::SHIFT);
    h.step();
    h.step();
    let (back, s, e) = range(&h);
    assert_ne!(back, next);
    assert!(e > 0 && s == 0, "Shift+Tab didn't select the value");
}

/// #125: Options ▸ Customization ▸ Commands lists the menus, the toolbox and the commands only a
/// key runs; a command gets the chord pressed in the Shortcut Keys tab, the conflict shows, and
/// Assign takes the chord from the command that had it.
#[test]
fn commands_page_assigns_a_pressed_chord() {
    use crate::commands::{parse_shortcut, OptionsPage};
    use crate::tools::ToolId;
    use vector_core::ops::Align;
    let mut h = join_harness(ToolId::Pick);
    let top = Cmd::Align(Align::Top);
    // two rectangles, the second lower: Align Top lines them up
    let a = blue_rect(&mut h);
    let b = {
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        d.add_objects("test", vec![Object::new(id, ObjKind::Rect(RectShape { w: 50.0, h: 50.0, radii: [0.0; 4], corner: CornerType::Round }), Affine::translate((400.0, 300.0)), Style::default())]);
        d.selection = vec![a, id];
        id
    };
    h.state_mut().open_options(OptionsPage::Customization, Some("Commands"));
    settle(&mut h);
    assert!(has_label(&h, "Shortcut Keys"));
    // the search looks through every category, Align Top among them
    h.state_mut().dialogs.options.commands_search = "align top".into();
    settle(&mut h);
    {
        use egui_kittest::kittest::Queryable;
        let at = h.query_all_by_label_contains("Align Top").next().map(|n| n.rect().center()).expect("search didn't find Align Top");
        click(&mut h, at);
    }
    assert_eq!(h.state().dialogs.options.commands_sel, Some(top));
    click_label(&mut h, "Shortcut Keys");
    assert!(has_label(&h, "T"), "the current chord T isn't shown");
    // press keys: Tab and Escape are not chords; Ctrl+G is taken by Group
    click_label(&mut h, "Click, then press keys");
    assert_eq!(h.state().dialogs.options.capture.as_deref(), Some(""));
    key_tap(&mut h, egui::Key::Tab, Some(egui::Key::Tab), egui::Modifiers::NONE);
    settle(&mut h);
    assert_eq!(h.state().dialogs.options.capture.as_deref(), Some(""), "Tab was taken as the chord");
    key_tap(&mut h, egui::Key::Escape, Some(egui::Key::Escape), egui::Modifiers::NONE);
    settle(&mut h);
    assert!(h.state().dialogs.options.open, "Escape closed the dialog instead of the capture");
    assert_eq!(h.state().dialogs.options.capture, None, "Escape didn't end the capture");
    click_label(&mut h, "Click, then press keys");
    key_tap(&mut h, egui::Key::G, Some(egui::Key::G), egui::Modifiers::CTRL | egui::Modifiers::COMMAND);
    settle(&mut h);
    assert_eq!(h.state().dialogs.options.capture.as_deref(), Some("Ctrl+G"));
    assert!(has_label(&h, "Conflicts with: Group"), "the conflict isn't shown");
    click_label(&mut h, "Assign");
    let ctrl_g = parse_shortcut("Ctrl+G").unwrap();
    assert!(crate::shortcuts::chords(h.state(), top).contains(&"Ctrl+G".to_string()));
    assert!(!crate::shortcuts::chords(h.state(), Cmd::Group).contains(&"Ctrl+G".to_string()), "Group kept the chord");
    assert_eq!(crate::shortcuts::chord_owners(h.state(), &ctrl_g, Cmd::Group), vec![top]);
    // and the menus show it
    assert_eq!(crate::shortcuts::shortcut_text(h.state(), top).as_deref(), Some("Ctrl+G"));
    click_label(&mut h, "OK");
    assert!(!h.state().dialogs.options.open);
    // the chord runs Align Top now, and not Group
    key_tap(&mut h, egui::Key::G, Some(egui::Key::G), egui::Modifiers::CTRL | egui::Modifiers::COMMAND);
    settle(&mut h);
    let d = h.state().doc().unwrap();
    assert!(d.doc.find(a).is_some() && d.doc.find(b).is_some(), "Ctrl+G still grouped");
    assert!((bounds_of(&h, a).y0 - bounds_of(&h, b).y0).abs() < 1e-6, "Ctrl+G didn't align the tops");
}

/// #125: assigning without Delete conflicts leaves both commands on the chord; Delete and the
/// keypad + survive the settings text; every command with a shortcut is on the Commands page.
#[test]
fn commands_page_assign_and_categories() {
    use crate::commands::parse_shortcut;
    use crate::dialogs::options::commands_page::{assign, categories};
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let app = h.state_mut();
    let f9 = parse_shortcut("Ctrl+Alt+F9").unwrap();
    assign(app, Cmd::Group, &f9, true);
    assign(app, Cmd::Ungroup, &f9, false);
    assert_eq!(crate::shortcuts::chord_owners(app, &f9, Cmd::Combine), vec![Cmd::Group, Cmd::Ungroup]);
    assert!(crate::shortcuts::chords(app, Cmd::Ungroup).contains(&"Ctrl+Alt+F9".to_string()));
    // with Delete conflicts both lose it; the one that had other chords keeps them
    assign(app, Cmd::Combine, &f9, true);
    assert_eq!(crate::shortcuts::chord_owners(app, &f9, Cmd::Combine), vec![]);
    // keypad +: Duplicate in Place's own chord
    assert_eq!(crate::shortcuts::chords(app, Cmd::DuplicateInPlace), vec!["Num +".to_string()]);
    let listed: Vec<Cmd> = categories().iter().flat_map(|(_, es)| es.iter().map(|e| e.cmd)).collect();
    for c in crate::commands::shortcut_commands() {
        assert!(listed.contains(&c), "{c:?} is not on the Commands page");
    }
    let other = &categories().iter().find(|(n, _)| n == "Other Commands").unwrap().1;
    assert!(other.iter().any(|e| e.cmd == Cmd::DuplicateInPlace && e.label == "Duplicate in Place"));
    // a caption, not the command's script name
    let bad: Vec<String> = other.iter().filter(|e| e.label.is_empty() || e.label == crate::scripting::cmd_to_text(&e.cmd) || e.label.contains('{') || e.label.contains('"')).map(|e| format!("{:?}", e.cmd)).collect();
    assert!(bad.is_empty(), "commands without a caption: {}", bad.join(", "));
}

/// Pictures for #119, #130, #140, #121 and #125: the piece Virtual Segment Delete would delete,
/// the copies Space leaves during a drag, the right-drag drop menu and the Commands page.
#[test]
#[ignore]
fn visual_issue_060_input_and_keyboard() {
    use crate::commands::OptionsPage;
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().doc_mut().unwrap().doc.settings.snap_off = true;
    // #119: Virtual Segment Delete over a side of an unfilled rectangle
    let (rid, _) = unfilled_rect(&mut h);
    h.state_mut().set_tool(ToolId::VirtualDelete);
    let side = scr(&h, (280.0, 100.0));
    move_to(&mut h, side);
    save(&mut h, "issue_119_virtual_delete_piece");
    h.state_mut().set_tool(ToolId::Pick);
    h.state_mut().doc_mut().unwrap().selection = vec![rid];
    h.state_mut().run(Cmd::Delete);
    h.step();
    // #130: two copies left on the way, the drag still going
    let id = blue_rect(&mut h);
    h.state_mut().set_tool(ToolId::Pick);
    h.step();
    let (a, b) = (scr(&h, (200.0, 160.0)), scr(&h, (560.0, 260.0)));
    move_to(&mut h, a);
    h.step();
    press(&mut h, a, true);
    h.step();
    for k in 1..=6 {
        move_to(&mut h, a + (b - a) * (k as f32 / 6.0));
        h.step();
        if k == 2 || k == 4 {
            space(&mut h, false);
            h.step();
            space_up(&mut h);
        }
    }
    save(&mut h, "issue_130_01_space_copies_while_dragging");
    press(&mut h, b, false);
    h.step();
    save(&mut h, "issue_130_02_dropped");
    for _ in 0..3 {
        undo(&mut h);
    }
    // #140: the drop menu over another object, and its Copy Fill Here
    let d = h.state_mut().doc_mut().unwrap();
    let red = d.doc.new_id();
    let mut st = Style::default();
    st.fill = Fill::Solid(Color::rgb(0xe0, 0x30, 0x30));
    d.add_objects("test", vec![Object::new(red, ObjKind::Ellipse(EllipseShape { w: 160.0, h: 160.0, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false }), Affine::translate((500.0, 100.0)), st)]);
    d.selection = vec![id];
    let (a, b) = (scr(&h, (200.0, 160.0)), scr(&h, (580.0, 180.0)));
    right_drag_to(&mut h, a, b, |_| {});
    save(&mut h, "issue_140_01_drop_menu");
    click_label(&mut h, "Copy Fill Here");
    save(&mut h, "issue_140_02_fill_copied");
    // #121: Tab into the property bar's Y field selects its value
    {
        use egui_kittest::kittest::Queryable;
        let x = h.query_all_by_role(egui::accesskit::Role::TextInput).map(|n| n.rect()).filter(|r| r.top() < 120.0).min_by(|a, b| (a.top() + a.left()).total_cmp(&(b.top() + b.left())));
        if let Some(x) = x {
            click(&mut h, x.center());
            key_tap(&mut h, egui::Key::Tab, Some(egui::Key::Tab), egui::Modifiers::NONE);
            h.step();
        }
    }
    save(&mut h, "issue_121_tab_selects_field");
    // #125: the Commands page, Align Top's Shortcut Keys with a conflicting chord pressed
    h.state_mut().open_options(OptionsPage::Customization, Some("Commands"));
    settle(&mut h);
    h.state_mut().dialogs.options.commands_menu = categories_len_minus_one();
    save(&mut h, "issue_125_01_other_commands");
    h.state_mut().dialogs.options.commands_menu = 0;
    h.state_mut().dialogs.options.commands_search = "align".into();
    h.state_mut().dialogs.options.commands_sel = Some(Cmd::Align(vector_core::ops::Align::Top));
    h.state_mut().dialogs.options.commands_tab = 1;
    h.state_mut().dialogs.options.capture = Some("Ctrl+G".into());
    save(&mut h, "issue_125_02_shortcut_keys_conflict");
}

fn categories_len_minus_one() -> usize {
    crate::dialogs::options::commands_page::categories().len() - 1
}

// #146: holding a palette swatch down shows its color variations (CorelDRAW's pop-up color
// picker); a click on one applies it as the fill, a right-click as the outline.

/// A document with a selected rectangle (a yellow fill, a black outline) and the palettes shown.
fn palette_harness<'a>() -> (Harness<'a, AnyVectorApp>, vector_core::ObjId) {
    let mut h = doc_harness();
    {
        let s = &mut h.state_mut().settings;
        s.default_palette_visible = true;
        s.doc_palette_visible = true;
        s.show_no_color_well = true;
        s.right_click_outline = true;
    }
    let id = outlined_rect(&mut h, vector_core::style::Outline { width: 1.0, color: Some(Color::BLACK), ..Default::default() });
    for _ in 0..3 {
        h.step();
    }
    (h, id)
}

/// The swatch of the default palette named `name`: its rectangle and color.
fn palette_swatch(h: &Harness<'_, AnyVectorApp>, name: &str) -> (egui::Rect, Color) {
    let a = h.state();
    let at = a.palettes.palettes[0].colors.iter().position(|c| c.name == name).expect("in the default palette");
    let k = at + a.settings.show_no_color_well as usize;
    let d = a.palettes.drawn.iter().find(|d| d.0 == crate::palette::Strip::Column(0) && d.1 == k).expect("the swatch is shown");
    (d.2, d.3.unwrap())
}

fn pointer_button(h: &mut Harness<'_, AnyVectorApp>, p: egui::Pos2, button: egui::PointerButton, pressed: bool) {
    h.input_mut().events.push(egui::Event::PointerButton { pos: p, button, pressed, modifiers: egui::Modifiers::NONE });
}

/// Press `button` on `p` and keep it down for `HOLD_DELAY` (each step is a quarter second).
fn hold_down(h: &mut Harness<'_, AnyVectorApp>, p: egui::Pos2, button: egui::PointerButton) {
    move_to(h, p);
    h.step();
    pointer_button(h, p, button, true);
    h.step();
    assert!(h.state().palettes.variations.is_none(), "not before the hold delay");
    for _ in 0..(crate::palette::HOLD_DELAY / 0.25).round() as usize {
        h.step();
    }
}

fn variations_open(h: &Harness<'_, AnyVectorApp>) -> bool {
    h.state().palettes.variations.is_some() && egui::Popup::is_id_open(&h.ctx, crate::palette::variations_popup_id())
}

/// The variation `i` of the open picker: where it is drawn and its color.
fn variation(h: &Harness<'_, AnyVectorApp>, i: usize) -> (egui::Pos2, Color) {
    let v = h.state().palettes.variations.as_ref().expect("the picker is open");
    (v.cells[i].center(), v.grid.colors[i])
}

/// Holding a swatch opens its variations without applying anything, also on release; a click on
/// a variation applies it as the fill, as one undo step, and closes the picker.
#[test]
fn palette_hold_opens_variations_and_a_click_applies_the_fill() {
    let (mut h, id) = palette_harness();
    let (r, red) = palette_swatch(&h, "Red");
    let fill = fill_of(&h, id);
    let undo = undo_steps(&h);
    hold_down(&mut h, r.center(), egui::PointerButton::Primary);
    assert!(h.state().palettes.variations.is_some(), "the hold opened the picker");
    h.step();
    assert!(variations_open(&h));
    {
        let v = h.state().palettes.variations.as_ref().unwrap();
        assert_eq!((v.base, v.grid.colors.len(), v.grid.colors[v.grid.base]), (red, 49, red));
        assert_eq!(v.held, Some(egui::PointerButton::Primary));
        assert!(v.grid.colors.iter().all(|c| c.is_cmyk()), "the CMYK palette's variations stay CMYK");
    }
    // releasing the hold on the swatch applies nothing
    pointer_button(&mut h, r.center(), egui::PointerButton::Primary, false);
    h.step();
    h.step();
    assert_eq!(fill_of(&h, id), fill, "the release of a hold is not a click");
    assert_eq!(undo_steps(&h), undo);
    assert!(variations_open(&h), "the picker stays open");
    assert_eq!(h.state().palettes.variations.as_ref().unwrap().held, None);
    // the darkest shade of red, below it
    let (p, shade) = variation(&h, 6 * 7 + 3);
    assert_eq!(shade.cmyk, Some([0, 100, 100, 75]));
    click(&mut h, p);
    assert_eq!(fill_of(&h, id), Fill::Solid(shade));
    assert_eq!(undo_steps(&h), undo + 1, "one undo step");
    h.step();
    assert!(!variations_open(&h), "a pick closes the picker");
    assert_eq!(outline_of(&h, id).color, Some(Color::BLACK));
}

/// A right-click on a variation applies it as the outline.
#[test]
fn palette_variation_right_click_sets_the_outline() {
    let (mut h, id) = palette_harness();
    let (r, _) = palette_swatch(&h, "Cyan");
    let fill = fill_of(&h, id);
    hold_down(&mut h, r.center(), egui::PointerButton::Primary);
    pointer_button(&mut h, r.center(), egui::PointerButton::Primary, false);
    h.step();
    h.step();
    let (p, tint) = variation(&h, 3);
    assert_eq!(tint.cmyk, Some([25, 0, 0, 0]));
    move_to(&mut h, p);
    h.step();
    for pressed in [true, false] {
        pointer_button(&mut h, p, egui::PointerButton::Secondary, pressed);
        h.step();
    }
    assert_eq!(outline_of(&h, id).color, Some(tint));
    assert_eq!(fill_of(&h, id), fill);
    h.step();
    assert!(!variations_open(&h));
}

/// Holding the right button opens the picker too, and releasing it over a variation sets the
/// outline; no context menu opens and the swatch color is not applied.
#[test]
fn palette_right_hold_released_over_a_variation_sets_the_outline() {
    for right_click_outline in [true, false] {
        let (mut h, id) = palette_harness();
        h.state_mut().settings.right_click_outline = right_click_outline;
        let (r, _) = palette_swatch(&h, "Green");
        let fill = fill_of(&h, id);
        let undo = undo_steps(&h);
        hold_down(&mut h, r.center(), egui::PointerButton::Secondary);
        h.step();
        assert!(variations_open(&h));
        assert_eq!(h.state().palettes.variations.as_ref().unwrap().held, Some(egui::PointerButton::Secondary));
        let (p, c) = variation(&h, 2 * 7 + 5);
        for k in 1..=4 {
            move_to(&mut h, r.center() + (p - r.center()) * (k as f32 / 4.0));
            h.step();
        }
        pointer_button(&mut h, p, egui::PointerButton::Secondary, false);
        h.step();
        assert_eq!(outline_of(&h, id).color, Some(c), "right_click_outline {right_click_outline}");
        assert_eq!(fill_of(&h, id), fill);
        assert_eq!(undo_steps(&h), undo + 1);
        h.step();
        assert!(!egui::Popup::is_any_open(&h.ctx), "no picker and no context menu left open");
    }
}

/// A quick click still applies the swatch at once, with no picker; a quick right-click sets the
/// outline.
#[test]
fn palette_quick_click_applies_at_once() {
    let (mut h, id) = palette_harness();
    let (r, red) = palette_swatch(&h, "Red");
    let undo = undo_steps(&h);
    move_to(&mut h, r.center());
    h.step();
    pointer_button(&mut h, r.center(), egui::PointerButton::Primary, true);
    h.step();
    pointer_button(&mut h, r.center(), egui::PointerButton::Primary, false);
    h.step();
    assert_eq!(fill_of(&h, id), Fill::Solid(red), "applied on the release, no waiting");
    assert_eq!(undo_steps(&h), undo + 1);
    let (r, blue) = palette_swatch(&h, "Blue");
    move_to(&mut h, r.center());
    h.step();
    for pressed in [true, false] {
        pointer_button(&mut h, r.center(), egui::PointerButton::Secondary, pressed);
        h.step();
    }
    assert_eq!(outline_of(&h, id).color, Some(blue));
    for _ in 0..4 {
        h.step();
    }
    assert!(h.state().palettes.variations.is_none(), "no picker for a click");
}

/// Dragging a swatch is not a hold: it still drops its color on the document palette.
#[test]
fn palette_drag_still_drops_on_the_document_palette() {
    let (mut h, id) = palette_harness();
    let fill = fill_of(&h, id);
    let first = Color::rgb(10, 20, 30);
    h.state_mut().doc_mut().unwrap().doc.palette.push(vector_core::NamedColor::new("First", first));
    for _ in 0..3 {
        h.step();
    }
    let at = h.state().palettes.drawn.iter().find(|d| d.0 == crate::palette::Strip::Document).expect("the document palette is shown").2;
    let (r, red) = palette_swatch(&h, "Red");
    drag(&mut h, r.center(), at.center() + egui::vec2(80.0, 0.0));
    h.step();
    assert!(h.state().doc().unwrap().doc.palette.iter().any(|n| n.color == red), "{:?}", h.state().doc().unwrap().doc.palette);
    assert!(h.state().palettes.variations.is_none(), "a drag opens no picker");
    assert_eq!(fill_of(&h, id), fill);
}

/// Esc or a click elsewhere closes the picker and applies nothing; Esc does not also clear the
/// selection.
#[test]
fn palette_variations_close_on_escape_and_click_outside() {
    let (mut h, id) = palette_harness();
    let (r, _) = palette_swatch(&h, "Orange");
    let fill = fill_of(&h, id);
    let undo = undo_steps(&h);
    for close in ["escape", "click"] {
        hold_down(&mut h, r.center(), egui::PointerButton::Primary);
        pointer_button(&mut h, r.center(), egui::PointerButton::Primary, false);
        h.step();
        h.step();
        assert!(variations_open(&h), "{close}");
        if close == "escape" {
            key(&mut h, egui::Key::Escape, true);
            h.step();
            key(&mut h, egui::Key::Escape, false);
            h.step();
        } else {
            // on the property bar, away from the picker and the selection
            click(&mut h, egui::pos2(900.0, 60.0));
        }
        h.step();
        assert!(!variations_open(&h), "closed by {close}");
        assert!(h.state().palettes.variations.is_none(), "{close}");
        assert_eq!(fill_of(&h, id), fill, "{close} applies nothing");
        assert_eq!(undo_steps(&h), undo);
        assert_eq!(h.state().doc().unwrap().selection, vec![id], "{close} keeps the selection");
    }
}

/// With right-click set to open the swatch menu, the menu offers the variations too.
#[test]
fn palette_context_menu_offers_color_variations() {
    let (mut h, id) = palette_harness();
    h.state_mut().settings.right_click_outline = false;
    h.step();
    let (r, purple) = palette_swatch(&h, "Purple");
    right_click(&mut h, r.center());
    h.step();
    assert!(has_label(&h, "Set Fill") && has_label(&h, "Color Variations"), "the swatch menu offers the variations");
    click_label(&mut h, "Color Variations");
    assert!(variations_open(&h));
    let v = h.state().palettes.variations.as_ref().unwrap();
    assert_eq!((v.base, v.held), (purple, None));
    let (p, c) = variation(&h, 3 * 7 + 4);
    click(&mut h, p);
    assert_eq!(fill_of(&h, id), Fill::Solid(c));
}

/// The document palette strip has the picker too; a spot color's variations are its tints.
#[test]
fn palette_document_strip_hold_shows_spot_tints() {
    let (mut h, id) = palette_harness();
    let spot = Color::from_cmyk(0.0, 60.0, 100.0, 0.0);
    h.state_mut().doc_mut().unwrap().doc.palette.push(vector_core::NamedColor { name: "PANTONE Orange".into(), color: spot, spot: true });
    for _ in 0..3 {
        h.step();
    }
    let r = h.state().palettes.drawn.iter().find(|d| d.0 == crate::palette::Strip::Document && d.1 == 0).expect("the swatch is shown").2;
    hold_down(&mut h, r.center(), egui::PointerButton::Primary);
    pointer_button(&mut h, r.center(), egui::PointerButton::Primary, false);
    h.step();
    h.step();
    assert!(variations_open(&h));
    {
        let v = h.state().palettes.variations.as_ref().unwrap();
        assert_eq!(v.grid.colors.len(), vector_core::color::SPOT_TINTS.len());
        assert!(v.spot && v.name == "PANTONE Orange");
        // the picker is above the strip, on screen
        assert!(v.cells.iter().all(|c| c.bottom() <= r.top() && c.top() >= 0.0), "{:?} {r:?}", v.cells);
    }
    let (p, half) = variation(&h, 5);
    assert_eq!(half.cmyk, Some([0, 30, 50, 0]));
    click(&mut h, p);
    assert_eq!(fill_of(&h, id), Fill::Solid(half));
}

/// The pictures: the picker of a CMYK color in the palette column, of a gray, and of a spot color
/// in the document palette.
#[test]
#[ignore]
fn visual_palette_variations() {
    let (mut h, _) = palette_harness();
    let open = |h: &mut Harness<'_, AnyVectorApp>, p: egui::Pos2| {
        hold_down(h, p, egui::PointerButton::Primary);
        pointer_button(h, p, egui::PointerButton::Primary, false);
        h.step();
        h.step();
        assert!(variations_open(h));
    };
    let (r, _) = palette_swatch(&h, "Red");
    open(&mut h, r.center());
    let (p, _) = variation(&h, 2 * 7 + 4);
    move_to(&mut h, p);
    save(&mut h, "pal_variations_01_red");
    let (r, _) = palette_swatch(&h, "50% Black");
    click(&mut h, egui::pos2(900.0, 60.0));
    open(&mut h, r.center());
    save(&mut h, "pal_variations_02_gray");
    click(&mut h, egui::pos2(900.0, 60.0));
    h.state_mut().doc_mut().unwrap().doc.palette.push(vector_core::NamedColor { name: "PANTONE Orange".into(), color: Color::from_cmyk(0.0, 60.0, 100.0, 0.0), spot: true });
    for _ in 0..3 {
        h.step();
    }
    let r = h.state().palettes.drawn.iter().find(|d| d.0 == crate::palette::Strip::Document && d.1 == 0).unwrap().2;
    open(&mut h, r.center());
    save(&mut h, "pal_variations_03_spot_tints");
}

// ---- #147: input methods (IME) typing on the canvas ----

fn ime_event(h: &mut Harness<'_, AnyVectorApp>, e: egui::ImeEvent) {
    h.input_mut().events.push(egui::Event::Ime(e));
}

/// The input method shows `s` as the text being composed (`active`: the clause being converted,
/// or an empty range for the caret inside the composition).
fn ime_preedit(h: &mut Harness<'_, AnyVectorApp>, s: &str, active: Option<std::ops::Range<usize>>) {
    ime_event(h, egui::ImeEvent::Preedit { text: s.into(), active_range_chars: active });
    h.step();
}

fn ime_commit(h: &mut Harness<'_, AnyVectorApp>, s: &str) {
    ime_event(h, egui::ImeEvent::Commit(s.into()));
    h.step();
}

/// A 36 pt text object `s` edited with the Text tool, the caret at `caret`.
fn ime_text<'a>(s: &str, caret: usize, tr: Affine) -> (Harness<'a, AnyVectorApp>, u64) {
    use vector_core::text::{CharFormat, TextObj};
    let mut h = join_harness(crate::tools::ToolId::Text);
    let d = h.state_mut().doc_mut().unwrap();
    let id = d.doc.new_id();
    let t = TextObj::artistic(s, CharFormat { size: 36.0, ..CharFormat::default() });
    d.add_objects("test", vec![Object::new(id, ObjKind::Text(t), tr, Style::text_default())]);
    d.selection = vec![id];
    d.text_edit = Some(crate::docview::TextEdit { obj: id, caret, anchor: caret, blink: 0.0 });
    // the next typing starts its own undo step
    h.state_mut().ts.text_last_edit = -10.0;
    h.step();
    h.step();
    (h, id)
}

fn ime_out(h: &Harness<'_, AnyVectorApp>) -> Option<egui::output::IMEOutput> {
    h.output().platform_output.ime
}

fn ime_caret(h: &Harness<'_, AnyVectorApp>) -> (usize, usize) {
    let te = h.state().doc().unwrap().text_edit.clone().expect("editing text");
    (te.caret, te.anchor)
}

fn text_count(h: &Harness<'_, AnyVectorApp>) -> usize {
    h.state().doc().unwrap().page_ref().all_objects().filter(|o| matches!(o.kind, ObjKind::Text(_))).count()
}

/// The screen point on the caret line before character `i` of the text `id` lays out in `doc`.
fn ime_caret_point(h: &Harness<'_, AnyVectorApp>, doc: &vector_core::Document, id: u64, i: usize) -> egui::Pos2 {
    let (t, tr) = crate::tools::ime::text_frame(doc, id, None).unwrap();
    let lay = vector_core::text::layout(&t);
    let (a, b) = crate::tools::ime::caret_line(&t, &lay.carets, tr, i).unwrap();
    h.state().doc().unwrap().to_screen(a.midpoint(b))
}

/// #147: the composition is shown but is not document text; its commit goes in once (also when
/// the integration repeats it as a text event), as one undo step.
#[test]
fn ime_commit_inserts_once() {
    let (mut h, id) = ime_text("Hello", 0, Affine::translate((100.0, 200.0)));
    let steps0 = undo_steps(&h);
    let rev0 = h.state().doc().unwrap().history.revision;
    ime_preedit(&mut h, "n", Some(1..1));
    ime_preedit(&mut h, "に", Some(1..1));
    assert!(h.state().ts.ime.composing());
    assert_eq!(text_of(&h, id).content, "Hello", "the composition is not document text");
    assert_eq!(h.state().doc().unwrap().history.revision, rev0, "nor a change");
    ime_event(&mut h, egui::ImeEvent::Preedit { text: "日本".into(), active_range_chars: Some(0..2) });
    ime_event(&mut h, egui::ImeEvent::Commit("日本".into()));
    h.input_mut().events.push(egui::Event::Text("日本".into()));
    h.step();
    assert_eq!(text_of(&h, id).content, "日本Hello");
    assert!(!h.state().ts.ime.composing());
    assert_eq!(ime_caret(&h), (2, 2), "the caret after the committed text");
    assert_eq!(undo_steps(&h), steps0 + 1, "one undo step");
    undo(&mut h);
    assert_eq!(text_of(&h, id).content, "Hello");
}

/// #147: the drawing shows the composition in place, from a copy of the document; dismissing it
/// leaves the text as it was.
#[test]
fn ime_preedit_is_drawn_in_place_not_stored() {
    let (mut h, id) = ime_text("Hello world", 5, Affine::translate((100.0, 200.0)));
    ime_preedit(&mut h, "に", None);
    ime_preedit(&mut h, "にほ", Some(2..2));
    {
        let app = h.state();
        let d = app.doc().unwrap();
        let pv = app.ts.ime.preview_for(d.uid).expect("the composition is drawn");
        let Some(ObjKind::Text(t)) = pv.doc.find(id).map(|o| &o.kind) else { panic!() };
        assert_eq!(t.content, "Helloにほ world");
        assert_eq!((pv.start, pv.len), (5, 2));
        assert_eq!(app.ts.ime.preedit().unwrap().active, Some(2..2));
        // the text after the caret moves along, as it will when committed
        let after = ime_caret_point(&h, &pv.doc, id, 8);
        let before = ime_caret_point(&h, &d.doc, id, 6);
        assert!(after.x > before.x + 5.0, "{after:?} {before:?}");
    }
    assert_eq!(text_of(&h, id).content, "Hello world");
    // the input method dismisses the composition (Backspace over its last character)
    ime_preedit(&mut h, "", None);
    assert!(!h.state().ts.ime.composing());
    assert!(h.state().ts.ime.preview_for(h.state().doc().unwrap().uid).is_none());
    assert_eq!(text_of(&h, id).content, "Hello world");
    assert_eq!(ime_caret(&h), (5, 5));
    // an empty commit only ends a composition
    ime_preedit(&mut h, "ほ", None);
    ime_commit(&mut h, "");
    assert!(!h.state().ts.ime.composing());
    assert_eq!(text_of(&h, id).content, "Hello world");
}

/// #147: at a Text tool click a cancelled composition leaves no text object; a committed one
/// makes it.
#[test]
fn ime_cancel_leaves_no_text_object() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Text);
    let n0 = text_count(&h);
    h.state_mut().ts.pending_text = Some((Point::new(100.0, 150.0), None));
    h.step();
    ime_preedit(&mut h, "にほん", None);
    assert!(h.state().ts.ime.composing());
    assert_eq!(text_count(&h), n0, "no object while composing");
    {
        let app = h.state();
        let pv = app.ts.ime.preview_for(app.doc().unwrap().uid).expect("drawn where the text goes");
        let Some(ObjKind::Text(t)) = pv.doc.find(pv.obj).map(|o| &o.kind) else { panic!() };
        assert_eq!(t.content, "にほん");
    }
    // Escape cancels the composition, not the typing
    key_tap(&mut h, egui::Key::Escape, Some(egui::Key::Escape), egui::Modifiers::NONE);
    h.step();
    assert!(!h.state().ts.ime.composing());
    assert!(h.state().ts.pending_text.is_some(), "still typing at the click");
    assert_eq!(text_count(&h), n0);
    // a second Escape ends the typing; nothing was made
    key_tap(&mut h, egui::Key::Escape, Some(egui::Key::Escape), egui::Modifiers::NONE);
    h.step();
    assert!(h.state().ts.pending_text.is_none());
    assert_eq!(text_count(&h), n0);
    // a commit makes the text object, edited after the committed text
    h.state_mut().ts.pending_text = Some((Point::new(100.0, 150.0), None));
    h.step();
    ime_preedit(&mut h, "にほん", Some(0..3));
    ime_commit(&mut h, "日本");
    assert_eq!(text_count(&h), n0 + 1);
    let d = h.state().doc().unwrap();
    let te = d.text_edit.clone().expect("the new text is edited");
    assert!(matches!(&d.doc.find(te.obj).unwrap().kind, ObjKind::Text(t) if t.content == "日本"));
    assert_eq!(te.caret, 2);
}

/// #147: while composing, keys belong to the input method: no shortcut (Undo, Zoom Out), no
/// editing key (Backspace, Select All), no typed or pasted text.
#[test]
fn ime_keys_do_not_run_shortcuts() {
    use crate::tools::ToolId;
    let (mut h, id) = ime_text("Hello", 5, Affine::translate((100.0, 200.0)));
    let steps0 = undo_steps(&h);
    let zoom0 = h.state().doc().unwrap().zoom;
    ime_preedit(&mut h, "に", Some(1..1));
    let ctrl = egui::Modifiers::CTRL | egui::Modifiers::COMMAND;
    key_tap(&mut h, egui::Key::Z, Some(egui::Key::Z), ctrl);
    key_tap(&mut h, egui::Key::F3, Some(egui::Key::F3), egui::Modifiers::NONE);
    key_tap(&mut h, egui::Key::Backspace, Some(egui::Key::Backspace), egui::Modifiers::NONE);
    key_tap(&mut h, egui::Key::A, Some(egui::Key::A), ctrl);
    h.input_mut().events.push(egui::Event::Text("e".into()));
    h.input_mut().events.push(egui::Event::Paste("pasted".into()));
    h.step();
    h.step();
    assert!(h.state().ts.ime.composing(), "the composition goes on");
    assert_eq!(text_of(&h, id).content, "Hello");
    assert_eq!(undo_steps(&h), steps0, "Ctrl+Z did not undo");
    assert_eq!(h.state().doc().unwrap().zoom, zoom0, "F3 did not zoom out");
    assert_eq!(h.state().tool, ToolId::Text);
    assert_eq!(ime_caret(&h), (5, 5), "Ctrl+A did not select");
    // committed: the keys work again
    ime_commit(&mut h, "に");
    assert_eq!(text_of(&h, id).content, "Helloに");
    key_tap(&mut h, egui::Key::Backspace, Some(egui::Key::Backspace), egui::Modifiers::NONE);
    h.step();
    assert_eq!(text_of(&h, id).content, "Hello");
    key_tap(&mut h, egui::Key::F3, Some(egui::Key::F3), egui::Modifiers::NONE);
    h.step();
    assert!(h.state().doc().unwrap().zoom < zoom0, "F3 zooms out again");
}

/// #147: the input method deleting around the caret counts characters, also during a
/// composition, which then goes on at the new caret.
#[test]
fn ime_delete_surrounding_counts_characters() {
    let (mut h, id) = ime_text("日本語abc", 3, Affine::translate((100.0, 200.0)));
    ime_event(&mut h, egui::ImeEvent::DeleteSurrounding { before_chars: 1, after_chars: 1 });
    h.step();
    assert_eq!(text_of(&h, id).content, "日本bc");
    assert_eq!(ime_caret(&h), (2, 2));
    ime_preedit(&mut h, "ご", None);
    ime_event(&mut h, egui::ImeEvent::DeleteSurrounding { before_chars: 1, after_chars: 0 });
    h.step();
    assert!(h.state().ts.ime.composing(), "the composition goes on");
    assert_eq!(text_of(&h, id).content, "日bc");
    ime_commit(&mut h, "語");
    assert_eq!(text_of(&h, id).content, "日語bc");
}

/// #147: the window's IME is on, with its candidates at the caret, only while typing goes to
/// the canvas (text, a Text tool click, a table cell), and off in other tools and while a text
/// field has the keyboard.
#[test]
fn ime_on_only_while_typing_on_canvas() {
    use crate::tools::ToolId;
    let (mut h, id) = ime_text("Hello", 5, Affine::translate((100.0, 200.0)));
    let out = ime_out(&h).expect("on while text is edited");
    let canvas = h.state().doc().unwrap().canvas_rect;
    assert!(canvas.contains(out.cursor_rect.center()), "{out:?} {canvas:?}");
    let doc = h.state().doc().unwrap().doc.clone();
    let p = ime_caret_point(&h, &doc, id, 5);
    assert!(out.cursor_rect.contains(p) && out.cursor_rect.width() < 6.0, "the caret: {out:?} {p:?}");
    assert!(out.rect.contains(out.cursor_rect.center()));
    assert!(!out.should_interrupt_composition);
    // the caret moved: so does the candidate window
    h.state_mut().doc_mut().unwrap().text_edit.as_mut().unwrap().caret = 0;
    h.state_mut().doc_mut().unwrap().text_edit.as_mut().unwrap().anchor = 0;
    h.step();
    let out0 = ime_out(&h).unwrap();
    assert!(out0.cursor_rect.center().x < out.cursor_rect.center().x - 20.0, "{out0:?} {out:?}");
    // another tool: off
    h.state_mut().set_tool(ToolId::Pick);
    h.step();
    assert!(ime_out(&h).is_none(), "off in the Pick tool");
    // a Text tool click: on, at the click
    h.state_mut().set_tool(ToolId::Text);
    h.state_mut().ts.pending_text = Some((Point::new(300.0, 300.0), None));
    h.step();
    let out = ime_out(&h).expect("on at a Text tool click");
    let at = h.state().doc().unwrap().to_screen(Point::new(300.0, 300.0));
    assert!((out.cursor_rect.center().x - at.x).abs() < 3.0 && out.cursor_rect.bottom() > at.y, "{out:?} {at:?}");
    // a property bar field has the keyboard: the IME is the field's, not the canvas's
    let field = {
        use egui_kittest::kittest::Queryable;
        h.query_all_by_role(egui::accesskit::Role::TextInput).map(|n| n.rect()).filter(|r| r.top() < 120.0).min_by(|a, b| a.left().total_cmp(&b.left())).expect("a property bar field")
    };
    click(&mut h, field.center());
    h.step();
    assert!(h.ctx.egui_wants_keyboard_input());
    let out = ime_out(&h).expect("the field's IME");
    assert!(field.expand(2.0).contains(out.cursor_rect.center()), "{out:?} {field:?}");
}

/// #147: the candidate window follows a turned text object's caret.
#[test]
fn ime_candidates_follow_rotated_text() {
    let tr = Affine::translate((300.0, 200.0)) * Affine::rotate(std::f64::consts::FRAC_PI_2);
    let (mut h, id) = ime_text("Hello", 2, tr);
    let out = ime_out(&h).unwrap();
    // turned a quarter: the caret line lies across the screen
    assert!(out.cursor_rect.width() > out.cursor_rect.height(), "{out:?}");
    let doc = h.state().doc().unwrap().doc.clone();
    let p = ime_caret_point(&h, &doc, id, 2);
    assert!(out.cursor_rect.contains(p), "{out:?} {p:?}");
    // composing: around the clause being converted
    ime_preedit(&mut h, "にほん", Some(0..3));
    let out = ime_out(&h).unwrap();
    let pv = h.state().ts.ime.preview_for(h.state().doc().unwrap().uid).unwrap().doc.clone();
    for i in [2, 5] {
        let p = ime_caret_point(&h, &pv, id, i);
        assert!(out.cursor_rect.expand(1.0).contains(p), "{i}: {out:?} {p:?}");
    }
}

/// #147: a composition left behind (the caret clicked elsewhere) is kept where it was typed and
/// the system's composition is interrupted once; one left behind by an undo is dropped.
#[test]
fn ime_composition_left_behind() {
    let (mut h, id) = ime_text("Hello", 5, Affine::translate((100.0, 200.0)));
    ime_preedit(&mut h, "にほん", None);
    {
        let te = h.state_mut().doc_mut().unwrap().text_edit.as_mut().unwrap();
        te.caret = 0;
        te.anchor = 0;
    }
    h.step();
    assert!(!h.state().ts.ime.composing());
    assert_eq!(text_of(&h, id).content, "Helloにほん", "kept where it was typed");
    assert_eq!(ime_caret(&h), (0, 0), "the caret where it was clicked");
    assert!(ime_out(&h).unwrap().should_interrupt_composition, "the system's composition ends");
    h.step();
    assert!(!ime_out(&h).unwrap().should_interrupt_composition, "only once");
    // an undo while composing: the undo applies, the composition goes
    ime_preedit(&mut h, "か", None);
    h.state_mut().run(Cmd::Undo);
    h.step();
    h.step();
    assert!(!h.state().ts.ime.composing());
    assert_eq!(text_of(&h, id).content, "Hello");
}

/// #147: the chosen table cell takes input method text like canvas text.
#[test]
fn ime_types_into_table_cells() {
    use crate::tools::ToolId;
    use vector_core::object::TableObj;
    let mut h = join_harness(ToolId::Table);
    let id = {
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        d.add_objects("test", vec![Object::new(id, ObjKind::Table(TableObj::new(2, 2, 240.0, 120.0)), Affine::translate((100.0, 100.0)), Style::default())]);
        d.selection = vec![id];
        id
    };
    h.state_mut().ts.table_cell = Some((0, 1));
    h.step();
    assert!(crate::tools::table_typing(h.state()));
    let cell = |h: &Harness<'_, AnyVectorApp>| match kind_of(h, id) {
        ObjKind::Table(t) => t.cell(0, 1).text.clone(),
        k => panic!("{k:?}"),
    };
    let out = ime_out(&h).expect("on for the cell");
    let cr = {
        let d = h.state().doc().unwrap();
        let ObjKind::Table(t) = kind_of(&h, id) else { panic!() };
        d.to_screen_rect(d.doc.find(id).unwrap().transform.transform_rect_bbox(t.cell_rect(0, 1)))
    };
    assert!(cr.contains(out.cursor_rect.center()), "in the cell: {out:?} {cr:?}");
    ime_preedit(&mut h, "ひょう", Some(0..3));
    assert_eq!(cell(&h), "", "not cell text yet");
    assert!(h.state().ts.ime.preview_for(h.state().doc().unwrap().uid).is_some());
    h.input_mut().events.push(egui::Event::Text("x".into()));
    h.step();
    assert_eq!(cell(&h), "");
    ime_commit(&mut h, "表");
    assert_eq!(cell(&h), "表");
    ime_event(&mut h, egui::ImeEvent::DeleteSurrounding { before_chars: 1, after_chars: 0 });
    h.step();
    assert_eq!(cell(&h), "");
}

/// Pictures for #147: a composition in the middle of a text (the clause being converted with the
/// heavier line), committed, at a Text tool click, and in a table cell.
#[test]
#[ignore]
fn visual_issue_147_ime() {
    use crate::tools::ToolId;
    use vector_core::object::TableObj;
    use vector_core::text::{CharFormat, TextObj};
    let mut h = doc_harness();
    h.state_mut().set_tool(ToolId::Text);
    let id = {
        let d = h.state_mut().doc_mut().unwrap();
        let id = d.doc.new_id();
        let t = TextObj::artistic("Hello world", CharFormat { size: 60.0, ..CharFormat::default() });
        d.add_objects("test", vec![Object::new(id, ObjKind::Text(t), Affine::translate((80.0, 160.0)), Style::text_default())]);
        d.selection = vec![id];
        d.text_edit = Some(crate::docview::TextEdit { obj: id, caret: 5, anchor: 5, blink: 0.0 });
        id
    };
    h.state_mut().ts.text_last_edit = -10.0;
    settle(&mut h);
    ime_preedit(&mut h, "にほんご", Some(0..2));
    assert!(ime_out(&h).is_some());
    save(&mut h, "issue_147_01_composing");
    ime_commit(&mut h, "日本語");
    assert_eq!(text_of(&h, id).content, "Hello日本語 world");
    save(&mut h, "issue_147_02_committed");
    h.state_mut().doc_mut().unwrap().text_edit = None;
    h.state_mut().ts.pending_text = Some((Point::new(80.0, 320.0), None));
    h.state_mut().ts.text_format.size = 48.0;
    h.step();
    ime_preedit(&mut h, "かんじ", Some(3..3));
    save(&mut h, "issue_147_03_new_text_composing");
    ime_commit(&mut h, "漢字");
    h.state_mut().set_tool(ToolId::Table);
    let tid = {
        let d = h.state_mut().doc_mut().unwrap();
        let tid = d.doc.new_id();
        let mut t = TableObj::new(2, 3, 420.0, 140.0);
        t.text_format.size = 28.0;
        d.add_objects("test", vec![Object::new(tid, ObjKind::Table(t), Affine::translate((80.0, 420.0)), Style::default())]);
        d.selection = vec![tid];
        tid
    };
    h.state_mut().ts.table_cell = Some((0, 1));
    h.step();
    ime_preedit(&mut h, "ひょう", Some(0..3));
    save(&mut h, "issue_147_04_table_cell_composing");
    ime_commit(&mut h, "表");
    assert!(matches!(kind_of(&h, tid), ObjKind::Table(t) if t.cell(0, 1).text == "表"));
}

/// #147: words in Chinese, Japanese, Korean or Thai script are not marked as misspelled by a
/// Western language's spell checker; other words still are.
#[test]
fn ime_text_is_not_spell_checked_as_english() {
    use crate::spell::unchecked_script;
    assert!(unchecked_script('日') && unchecked_script('に') && unchecked_script('한') && unchecked_script('ไ'));
    assert!(!unchecked_script('a') && !unchecked_script('é') && !unchecked_script('đ') && !unchecked_script('Ж'));
    if !crate::spell::supported("en-US") {
        return;
    }
    let text = "Hello 日本語 wrld 漢字";
    let flagged: Vec<String> = crate::spell::check(text, "en-US").iter().map(|e| text.chars().skip(e.start).take(e.len).collect()).collect();
    assert!(flagged.iter().all(|w| !w.chars().any(unchecked_script)), "{flagged:?}");
    assert!(flagged.iter().any(|w| w == "wrld"), "{flagged:?}");
}

// ---- Dockers that can be rearranged (#144) ----

/// A document with `dockers` open in one group on the right (as 0.6.0 had them), the first
/// one shown.
fn dock_harness<'a>(dockers: &[crate::dockers::DockerId]) -> Harness<'a, AnyVectorApp> {
    let mut h = doc_harness();
    {
        let app = h.state_mut();
        app.dockers.open.clear();
        for d in dockers {
            app.dockers.show(*d);
        }
        app.dockers.reset_layout();
        app.dockers.set_width = Some(300.0);
        app.dockers.show(dockers[0]);
    }
    for _ in 0..4 {
        h.step();
    }
    h
}

/// Where docker `d`'s tab was drawn.
fn dock_tab(h: &Harness<'_, AnyVectorApp>, d: crate::dockers::DockerId) -> egui::Rect {
    crate::dockers::dock::drawn_tab(h.state(), d).unwrap_or_else(|| panic!("the {d:?} tab isn't drawn"))
}

/// Where the group of docker `d` was drawn: the whole group and its title bar.
fn dock_group(h: &Harness<'_, AnyVectorApp>, d: crate::dockers::DockerId) -> (egui::Rect, egui::Rect) {
    crate::dockers::dock::drawn_group(h.state(), d).unwrap_or_else(|| panic!("the group of {d:?} isn't drawn"))
}

/// The settings written and read back, as across a restart: the dockers they bring back.
fn dock_restart(h: &mut Harness<'_, AnyVectorApp>) -> crate::settings::Settings {
    let app = h.state_mut();
    app.dockers.save(&mut app.settings);
    let json = serde_json::to_string(&app.settings).unwrap();
    serde_json::from_str(&json).unwrap()
}

fn middle_click(h: &mut Harness<'_, AnyVectorApp>, p: egui::Pos2) {
    move_to(h, p);
    h.step();
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Middle, pressed, modifiers: egui::Modifiers::NONE });
        h.step();
    }
}

/// #144: a tab dragged along the tab strip takes its new place, kept with the settings at
/// once; a click still shows a docker and a middle-click closes it; Escape calls a drag off.
#[test]
fn issue_144_tab_drag_reorders_and_persists() {
    use crate::dockers::DockerId::*;
    let mut h = dock_harness(&[Properties, Objects, Pages]);
    assert_eq!(h.state().dockers.open, vec![Properties, Objects, Pages]);
    // Pages dragged above Properties
    let (from, to) = (dock_tab(&h, Pages).center(), dock_tab(&h, Properties).center_top() + egui::vec2(0.0, 4.0));
    move_to(&mut h, from);
    h.step();
    press(&mut h, from, true);
    h.step();
    for k in 1..=6 {
        move_to(&mut h, from + (to - from) * (k as f32 / 6.0));
        h.step();
        if k == 4 {
            assert!(h.state().dockers.live.drag.is_some(), "the tab isn't being dragged");
            assert!(!crate::customize::dragging(&h.ctx), "the command bars think a button is being dragged");
        }
    }
    press(&mut h, to, false);
    h.step();
    h.step();
    assert_eq!(h.state().dockers.open, vec![Pages, Properties, Objects], "the tab didn't take its new place");
    assert!(h.state().dockers.live.drag.is_none());
    // kept with the settings at once, and there after a restart
    assert_eq!(h.state().settings.open_dockers, vec!["Pages", "Properties", "Objects"]);
    let back = dock_restart(&mut h);
    assert_eq!(crate::dockers::DockerState::from_settings(&back).open, vec![Pages, Properties, Objects]);
    // Properties to the end, after Objects
    let (a, b) = (dock_tab(&h, Properties).center(), dock_tab(&h, Objects).center_bottom() - egui::vec2(0.0, 4.0));
    drag(&mut h, a, b);
    assert_eq!(h.state().dockers.open, vec![Pages, Objects, Properties]);
    // the tabs drawn in their new places
    h.step();
    // a click shows a docker
    let t = dock_tab(&h, Objects).center();
    click(&mut h, t);
    assert_eq!(h.state().dockers.layout.groups[0].active, Some(Objects));
    // a drag called off with Escape changes nothing
    let (from, to) = (dock_tab(&h, Pages).center(), dock_tab(&h, Properties).center_bottom());
    move_to(&mut h, from);
    h.step();
    press(&mut h, from, true);
    h.step();
    for k in 1..=4 {
        move_to(&mut h, from + (to - from) * (k as f32 / 4.0));
        h.step();
    }
    h.input_mut().events.push(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    h.step();
    press(&mut h, to, false);
    h.step();
    assert_eq!(h.state().dockers.open, vec![Pages, Objects, Properties], "Escape didn't call the drag off");
    // a middle-click closes a docker; the last one closed takes the docker area away
    let canvas = h.state().doc().unwrap().canvas_rect;
    for d in [Objects, Pages, Properties] {
        let t = dock_tab(&h, d).center();
        middle_click(&mut h, t);
        assert!(!h.state().dockers.open.contains(&d), "middle-click didn't close {d:?}");
        h.step();
    }
    for _ in 0..3 {
        h.step();
    }
    assert!(crate::dockers::dock::drawn_group(h.state(), Properties).is_none());
    assert!(h.state().doc().unwrap().canvas_rect.right() > canvas.right() + 200.0, "the drawing window didn't take the room");
}

/// #144: a tab dragged by the left edge of the drawing window docks on the left; onto the
/// lower quarter of a group, a group stacked under it with a divider between them; onto a
/// group's tabs, into that group. A group's title bar drags the whole group; the fold button
/// folds every group of the side into one strip of tabs.
#[test]
#[ignore]
fn visual_issue_144_dock_left_stack_and_split() {
    use crate::dockers::DockerId::*;
    use crate::dockers::DockerSide::*;
    let mut h = dock_harness(&[Properties, Objects, Pages]);
    save(&mut h, "issue_144_01_one_group");
    let canvas = h.state().doc().unwrap().canvas_rect;
    let central = crate::dockers::dock::drawn_central(h.state());
    // Objects by the left edge of the drawing window: a docker area on the left
    let t = dock_tab(&h, Objects).center();
    drag(&mut h, t, egui::pos2(central.left() + 20.0, central.center().y));
    settle(&mut h);
    assert_eq!(h.state().dockers.side_of(Objects), Some(Left));
    assert_eq!(h.state().dockers.layout.side_groups(Left).len(), 1);
    assert!(!h.state().dockers.layout.groups[h.state().dockers.layout.group_of(Properties).unwrap()].dockers.contains(&Objects));
    assert!(h.state().doc().unwrap().canvas_rect.left() > canvas.left() + 200.0, "the drawing window didn't make room on the left");
    save(&mut h, "issue_144_02_docked_left");
    // Pages onto the lower quarter of the right group: two groups stacked on the right
    let (block, _) = dock_group(&h, Properties);
    let t = dock_tab(&h, Pages).center();
    drag(&mut h, t, egui::pos2(block.center().x - 30.0, block.bottom() - 40.0));
    settle(&mut h);
    let l = h.state().dockers.layout.clone();
    let gs = l.side_groups(Right);
    assert_eq!(gs.len(), 2, "Pages didn't stack under Properties");
    assert_eq!(l.groups[gs[0]].dockers, vec![Properties]);
    assert_eq!(l.groups[gs[1]].dockers, vec![Pages]);
    assert!((l.groups[gs[0]].share + l.groups[gs[1]].share - 1.0).abs() < 1e-3);
    save(&mut h, "issue_144_03_stacked_right");
    // the divider dragged down: the upper group grows, the lower one shrinks
    let (top, _) = dock_group(&h, Properties);
    let (bottom, _) = dock_group(&h, Pages);
    let y = (top.bottom() + bottom.top()) / 2.0;
    drag(&mut h, egui::pos2(top.center().x, y), egui::pos2(top.center().x, y + 120.0));
    settle(&mut h);
    assert!(dock_group(&h, Properties).0.height() > top.height() + 100.0, "the divider didn't move");
    assert!(dock_group(&h, Pages).0.height() < bottom.height() - 100.0);
    // Pages onto the left group's tabs: into that group, the right side back to one group
    let (a, b) = (dock_tab(&h, Pages).center(), dock_tab(&h, Objects).center_bottom() + egui::vec2(0.0, 6.0));
    drag(&mut h, a, b);
    settle(&mut h);
    let l = h.state().dockers.layout.clone();
    assert_eq!(l.groups[l.group_of(Objects).unwrap()].dockers, vec![Objects, Pages]);
    assert_eq!(l.side_groups(Right).len(), 1);
    save(&mut h, "issue_144_04_joined_left");
    // the left group's title bar onto the top of the right group: stacked above it, both dockers
    let (_, head) = dock_group(&h, Objects);
    let (block, _) = dock_group(&h, Properties);
    drag(&mut h, head.left_center() + egui::vec2(30.0, 0.0), egui::pos2(block.center().x - 30.0, block.top() + 70.0));
    settle(&mut h);
    let l = h.state().dockers.layout.clone();
    assert!(l.side_groups(Left).is_empty(), "the group didn't leave the left side");
    let gs = l.side_groups(Right);
    assert_eq!(gs.len(), 2);
    assert_eq!(l.groups[gs[0]].dockers, vec![Objects, Pages]);
    save(&mut h, "issue_144_05_group_stacked_on_top");
    // folded: one strip with the tabs of both groups; a tab clicked unfolds it
    let (_, head) = dock_group(&h, Objects);
    click(&mut h, egui::pos2(head.right() - 38.0, head.center().y));
    settle(&mut h);
    assert!(h.state().dockers.collapsed, "the fold button didn't fold the side");
    let (p, o) = (dock_tab(&h, Properties), dock_tab(&h, Objects));
    assert!(o.bottom() < p.top(), "the folded strip lost the order of the groups");
    save(&mut h, "issue_144_06_folded");
    click(&mut h, p.center());
    settle(&mut h);
    assert!(!h.state().dockers.collapsed);
    assert_eq!(h.state().dockers.layout.groups[h.state().dockers.layout.group_of(Properties).unwrap()].active, Some(Properties));
}

/// #144: a tab dragged onto the drawing window floats in a window of its own, which sizes by
/// its edges, moves by its title bar and docks again when let go on a group's tabs; right-click
/// ▸ Float floats it too, and a double-click on its tab docks it back.
#[test]
#[ignore]
fn visual_issue_144_float_and_redock() {
    use crate::dockers::DockerId::*;
    let mut h = dock_harness(&[Properties, Objects]);
    let central = crate::dockers::dock::drawn_central(h.state());
    let t = dock_tab(&h, Objects).center();
    drag(&mut h, t, central.center());
    settle(&mut h);
    let floating = |h: &Harness<'_, AnyVectorApp>, d| h.state().dockers.layout.group_of(d).map(|g| h.state().dockers.layout.groups[g].floating);
    assert_eq!(floating(&h, Objects), Some(true), "the tab didn't float");
    assert_eq!(h.state().dockers.layout.groups.len(), 2);
    let (fblock, _) = dock_group(&h, Objects);
    assert!(central.contains(fblock.center()));
    save(&mut h, "issue_144_07_floating");
    // wider by its right edge
    drag(&mut h, egui::pos2(fblock.right() - 2.0, fblock.center().y), egui::pos2(fblock.right() + 78.0, fblock.center().y));
    settle(&mut h);
    let (wider, head) = dock_group(&h, Objects);
    assert!((wider.width() - fblock.width() - 80.0).abs() < 4.0, "the window didn't size: {} -> {}", fblock.width(), wider.width());
    // moved by its title bar, still floating
    drag(&mut h, head.left_center() + egui::vec2(40.0, 0.0), head.left_center() + egui::vec2(-160.0, 120.0));
    settle(&mut h);
    let (moved, head) = dock_group(&h, Objects);
    assert!((moved.left() - (wider.left() - 200.0)).abs() < 4.0 && (moved.top() - (wider.top() + 120.0)).abs() < 4.0, "the window didn't move: {wider:?} -> {moved:?}");
    assert_eq!(floating(&h, Objects), Some(true));
    save(&mut h, "issue_144_08_floating_moved");
    // let go on the right group's tabs: docked into that group
    let t = dock_tab(&h, Properties).center_bottom() + egui::vec2(0.0, 6.0);
    drag(&mut h, head.left_center() + egui::vec2(40.0, 0.0), t);
    settle(&mut h);
    let l = h.state().dockers.layout.clone();
    assert_eq!(l.groups.len(), 1, "the window didn't dock");
    assert_eq!(l.groups[0].dockers, vec![Properties, Objects]);
    assert!(!l.groups[0].floating);
    save(&mut h, "issue_144_09_redocked");
    // right-click ▸ Float
    let t = dock_tab(&h, Objects).center();
    right_click(&mut h, t);
    // the menu's items are there from the next frames
    for _ in 0..3 {
        h.step();
    }
    click_label(&mut h, "Float");
    settle(&mut h);
    assert_eq!(floating(&h, Objects), Some(true), "right-click ▸ Float didn't float it");
    save(&mut h, "issue_144_10_floated_from_the_menu");
    // a double-click on its tab docks it back where it came from
    let t = dock_tab(&h, Objects).center();
    move_to(&mut h, t);
    h.step();
    for _ in 0..2 {
        for pressed in [true, false] {
            press(&mut h, t, pressed);
        }
        h.step();
    }
    settle(&mut h);
    assert_eq!(floating(&h, Objects), Some(false), "a double-click didn't dock it back");
    assert_eq!(h.state().dockers.side_of(Objects), Some(crate::dockers::DockerSide::Right));
}

/// #144: the arrangement (sides, stacked groups, a floating window) comes back after the
/// settings are written and read again, belongs to the workspace (switching away and back
/// brings it back) and travels in a workspace file. Window ▸ Dockers ▸ Reset Docker Layout puts
/// every docker back in one group on the right.
#[test]
#[ignore]
fn visual_issue_144_layout_survives_settings_and_workspaces() {
    use crate::dockers::dock::Act;
    use crate::dockers::layout::Target;
    use crate::dockers::DockerId::*;
    use crate::dockers::DockerSide::*;
    use crate::workspaces as ws;
    use egui_kittest::kittest::Queryable;
    let mut h = dock_harness(&[Properties, Objects, Pages, Color]);
    {
        let st = &mut h.state_mut().dockers;
        st.act(Act::Dock(Objects, Left));
        let g = st.layout.group_of(Properties).unwrap();
        st.layout.place(&[Pages], Target::Stack { group: g, after: true });
        st.act(Act::Float(Color));
    }
    settle(&mut h);
    let arranged = h.state().dockers.layout.clone();
    assert_eq!(arranged.side_groups(Left).len(), 1);
    assert_eq!(arranged.side_groups(Right).len(), 2);
    assert!(arranged.groups.iter().any(|g| g.floating && g.dockers == [Color]));
    save(&mut h, "issue_144_11_arranged");
    // as after a restart
    let back = dock_restart(&mut h);
    {
        let app = h.state_mut();
        app.settings = back;
        app.dockers = crate::dockers::DockerState::from_settings(&app.settings);
    }
    settle(&mut h);
    assert_eq!(h.state().dockers.layout, arranged, "the settings didn't bring the arrangement back");
    for d in [Properties, Objects, Pages, Color] {
        dock_tab(&h, d);
    }
    save(&mut h, "issue_144_12_after_restart");
    // a workspace of its own: switching away and back brings it back
    let cur = h.state().settings.workspace.clone();
    ws::create(h.state_mut(), "Docks 144", &cur, "", true);
    settle(&mut h);
    assert_eq!(h.state().dockers.layout, arranged);
    ws::switch(h.state_mut(), "Lite");
    settle(&mut h);
    assert_ne!(h.state().dockers.layout, arranged);
    ws::switch(h.state_mut(), "Docks 144");
    settle(&mut h);
    assert_eq!(h.state().dockers.layout, arranged, "the workspace didn't keep the arrangement");
    // in a workspace file
    let dir = std::env::temp_dir().join(format!("vector_ws144_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("Docks.cdws");
    let l = ws::capture(h.state_mut());
    ws::write_file(&p, &ws::WorkspaceFile { name: "Docks".into(), description: String::new(), elements: vec![], layout: l }).unwrap();
    let f = ws::read_file(&p).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    h.state_mut().run(Cmd::ResetDockerLayout);
    settle(&mut h);
    assert_eq!(h.state().dockers.layout.groups.len(), 1);
    ws::apply(h.state_mut(), &f.layout);
    settle(&mut h);
    assert_eq!(h.state().dockers.layout, arranged, "the workspace file didn't bring the arrangement back");
    // Window ▸ Dockers ▸ Reset Docker Layout
    open_menu(&mut h, "Window");
    for _ in 0..3 {
        h.step();
    }
    let dk = h.query_all_by_label("Dockers").map(|n| n.rect()).filter(|r| r.top() > 30.0).last().expect("Window ▸ Dockers");
    move_to(&mut h, dk.center());
    for _ in 0..4 {
        h.step();
    }
    save(&mut h, "issue_144_13_window_dockers_menu");
    click_label(&mut h, "Reset Docker Layout");
    settle(&mut h);
    let l = h.state().dockers.layout.clone();
    assert_eq!(l.groups.len(), 1, "Reset Docker Layout didn't put the dockers in one group");
    assert!(l.groups[0].on(Right));
    assert_eq!(l.groups[0].dockers.len(), 4);
    save(&mut h, "issue_144_14_reset");
    ws::delete(h.state_mut(), "Docks 144");
}

// ---- #145: blends along a path, their parts and their options ----

fn bp_circle(d: &mut crate::docview::DocView, c: (f64, f64), r: f64, fill: Fill) -> Object {
    let e = EllipseShape { w: 2.0 * r, h: 2.0 * r, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false };
    let mut st = Style::default();
    st.fill = fill;
    Object::new(d.doc.new_id(), ObjKind::Ellipse(e), Affine::translate((c.0 - r, c.1 - r)), st)
}

/// A blend of a red circle (radius 20, at the left of the circle below) to a blue one (radius
/// 30, at its right) in 8 steps, and an unfilled circle of radius 150 about (400, 400) to use as
/// its path. The blend is selected. Gives [blend, start, end, path].
fn bp_add(h: &mut Harness<'_, AnyVectorApp>) -> [u64; 4] {
    let d = h.state_mut().doc_mut().unwrap();
    let a = bp_circle(d, (250.0, 400.0), 20.0, Fill::Solid(Color::rgb(220, 40, 40)));
    let b = bp_circle(d, (550.0, 400.0), 30.0, Fill::Solid(Color::rgb(40, 80, 220)));
    let path = bp_circle(d, (400.0, 400.0), 150.0, Fill::None);
    let (ia, ib, ip) = (a.id, b.id, path.id);
    d.add_objects("test", vec![a, b, path]);
    let blend = d.modify("Blend", |doc, _| vector_core::ops::create_blend(doc, ia, ib, 8)).unwrap();
    d.selection = vec![blend];
    [blend, ia, ib, ip]
}

/// [`bp_add`] with the path already attached (the blend and the path selected, then New Path).
fn bp_on_path(h: &mut Harness<'_, AnyVectorApp>) -> [u64; 4] {
    let ids = bp_add(h);
    h.state_mut().doc_mut().unwrap().selection = vec![ids[0], ids[3]];
    run_cmd(h, Cmd::BarOption(0x42941bfb));
    assert!(bp_blend(h, ids[0]).path.is_some(), "New Path with the blend and the path selected");
    ids
}

fn bp_blend(h: &Harness<'_, AnyVectorApp>, id: u64) -> vector_core::object::BlendObj {
    vector_core::blend_path::blend_of(&h.state().doc().unwrap().doc, id).unwrap().clone()
}

fn bp_centres(b: &vector_core::object::BlendObj) -> Vec<Point> {
    vector_core::object::blend_steps(b, || 0).iter().map(|o| o.bounds().center()).collect()
}

/// How far `p` is off the path circle of [`bp_add`].
fn bp_off(p: Point) -> f64 {
    ((p - Point::new(400.0, 400.0)).hypot() - 150.0).abs()
}

fn bp_state(h: &Harness<'_, AnyVectorApp>, key: &str) -> Option<(bool, bool)> {
    crate::propbar::option_state(h.state(), key)
}

/// #145 (§5.4): every button, check box and radio button of the Blend property bar, of its
/// Path Properties, Options and Start and End Properties flyouts and of the Blend Control Editing
/// menu runs a command: none of them says "not available in this version yet". (The other bars
/// still have such items; they are listed, not checked, here.)
#[test]
fn blend_bar_items_all_run_a_command() {
    use crate::ui::Kind;
    let blend_bars = ["d16cdcfb", "1efc4f62", "428bed7d", "37492bbf", "8a5f6c77"];
    let runs = |it: &crate::ui::Item| !matches!(it.kind, Kind::Button | Kind::Check | Kind::Radio | Kind::ButtonFlyout) || it.id.is_empty() || crate::ui::bind::cmd(it.id).is_some();
    let mut seen = 0;
    let mut missing = vec![];
    let mut elsewhere = 0;
    for b in crate::ui::bars() {
        let ours = blend_bars.contains(&&b.id[..8.min(b.id.len())]);
        seen += ours as usize;
        for it in b.items.iter().filter(|it| !runs(it)) {
            if ours {
                missing.push(format!("{} ▸ {} ({})", b.name, crate::ui::plain(it.text), it.id));
            } else {
                elsewhere += 1;
            }
        }
    }
    assert_eq!(seen, blend_bars.len(), "a blend bar is missing from the reference data");
    assert!(missing.is_empty(), "blend items without a command: {missing:#?}");
    eprintln!("items without a command on the other bars: {elsewhere}");
    // and each blend command has a state of its own
    for (k, _) in crate::blendpath::KEYS {
        assert!(crate::ui::bind::cmd(k).is_some_and(|c| c == Cmd::BarOption(u32::from_str_radix(k, 16).unwrap())), "{k}");
    }
}

/// #145: New Path waits for a click on the path (the hint says so); the blend then follows the
/// circle (every step on it), the circle leaves the page and becomes part of the blend, and one
/// Undo puts it all back. Show Path, Detach and the path options are on only with a path.
#[test]
fn blend_new_path_by_click_follows_the_circle_and_undoes() {
    use crate::tools::{PickTarget, ToolId};
    let mut h = join_harness(ToolId::Blend);
    let [blend, start, end, path] = bp_add(&mut h);
    h.step();
    assert_eq!(bp_state(&h, "42941bfb"), Some((true, false)), "New Path");
    for k in ["050d45c6", "3726add2", "247d0e09", "50c0c56f"] {
        assert_eq!(bp_state(&h, k), Some((false, false)), "{k} without a path");
    }
    let undo0 = undo_steps(&h);
    run_cmd(&mut h, Cmd::BarOption(0x42941bfb));
    assert!(matches!(h.state().ts.pick_target, Some(PickTarget::Blend(crate::blendpath::Pick::Path { .. }))));
    assert!(h.state().ts.hint(ToolId::Blend).starts_with("Click the path the blend follows"));
    // the blend itself can't be its own path
    let s = bp_blend(&h, blend).start.bounds().center();
    pick_click(&mut h, (s.x, s.y), egui::Modifiers::NONE);
    assert!(bp_blend(&h, blend).path.is_none(), "the start became the path");
    assert_eq!(undo_steps(&h), undo0, "a refused click left an undo step");
    // the circle, clicked on its outline at the top
    run_cmd(&mut h, Cmd::BarOption(0x42941bfb));
    pick_click(&mut h, (400.0, 250.0), egui::Modifiers::NONE);
    let b = bp_blend(&h, blend);
    assert_eq!(b.path.as_ref().map(|p| p.obj.id), Some(path));
    assert!(h.state().ts.pick_target.is_none());
    let d = h.state().doc().unwrap();
    assert!(d.doc.locate(path).is_none(), "the path is still on the page of its own");
    assert_eq!(d.selection, vec![blend]);
    assert_eq!((b.start.id, b.end.id), (start, end));
    let c = bp_centres(&b);
    assert_eq!(c.len(), 8);
    for p in &c {
        assert!(bp_off(*p) < 1.0, "a step off the circle: {p:?}");
    }
    // the steps go round the circle, not straight across
    assert!(c.iter().any(|p| (p.y - 400.0).abs() > 100.0), "{c:?}");
    assert_eq!(undo_steps(&h), undo0 + 1);
    for k in ["050d45c6", "3726add2", "247d0e09", "50c0c56f"] {
        assert_eq!(bp_state(&h, k), Some((true, false)), "{k} with a path");
    }
    undo(&mut h);
    let d = h.state().doc().unwrap();
    assert!(bp_blend(&h, blend).path.is_none());
    assert!(d.doc.locate(path).is_some(), "Undo put the path back on the page");
    // with the blend and the curve both selected it applies at once (the Blend tool's way)
    h.state_mut().doc_mut().unwrap().selection = vec![path, blend];
    run_cmd(&mut h, Cmd::BarOption(0x42941bfb));
    assert!(h.state().ts.pick_target.is_none());
    assert_eq!(bp_blend(&h, blend).path.as_ref().map(|p| p.obj.id), Some(path));
    assert_eq!(sel(&h), vec![blend]);
}

/// #145: Show Start and Show End select the control on its own (the status bar names it, its
/// outline is drawn); dragging it slides it along the path; Escape lets go of it; Delete on the
/// end takes the blend apart (the start and the path stay), and Undo brings the blend back.
#[test]
fn blend_show_start_and_end_focus_drag_and_delete() {
    use crate::tools::ToolId;
    use vector_core::object::BlendPart;
    let mut h = join_harness(ToolId::Pick);
    let [blend, start, end, path] = bp_on_path(&mut h);
    h.state_mut().doc_mut().unwrap().selection = vec![blend];
    run_cmd(&mut h, Cmd::BarOption(0x7341835a));
    assert_eq!(sel(&h), vec![start]);
    assert_eq!(crate::blendpath::focus(h.state().doc().unwrap()), Some((blend, BlendPart::Start)));
    let info = crate::statusbar::object_info(h.state());
    assert!(info.starts_with("Blend start: ") && info.contains(" on "), "{info}");
    // the blend's commands still act on the blend
    assert_eq!(bp_state(&h, "1a1bed58"), Some((true, false)), "Show End with the start shown");
    // dragged up and away, it stays on the path
    let before = bp_blend(&h, blend).start.bounds().center();
    shape_drag(&mut h, (before.x, before.y), (before.x + 40.0, before.y - 90.0), egui::Modifiers::NONE);
    h.step();
    let b = bp_blend(&h, blend);
    let now = b.start.bounds().center();
    assert!((now - before).hypot() > 20.0, "the start didn't move: {before:?} → {now:?}");
    assert!(bp_off(now) < 1.0, "the start left the path: {now:?}");
    for p in bp_centres(&b) {
        assert!(bp_off(p) < 1.0, "a step off the circle: {p:?}");
    }
    assert_eq!(sel(&h), vec![start], "the start is still the one selected");
    h.key_press(egui::Key::Escape);
    h.step();
    assert_eq!(crate::blendpath::focus(h.state().doc().unwrap()), None);
    // Show End, then Delete: the blend comes apart
    h.state_mut().doc_mut().unwrap().selection = vec![blend];
    run_cmd(&mut h, Cmd::BarOption(0x1a1bed58));
    assert_eq!(sel(&h), vec![end]);
    assert!(crate::statusbar::object_info(h.state()).starts_with("Blend end: "));
    let undo0 = undo_steps(&h);
    run_cmd(&mut h, Cmd::Delete);
    let d = h.state().doc().unwrap();
    assert!(d.doc.find(blend).is_none() && d.doc.find(end).is_none(), "the blend or its end is left");
    assert!(d.doc.locate(start).is_some() && d.doc.locate(path).is_some(), "the start and the path are on the page");
    assert_eq!(d.selection, vec![start]);
    assert_eq!(undo_steps(&h), undo0 + 1);
    undo(&mut h);
    assert!(bp_blend(&h, blend).path.is_some(), "Undo brought the blend on its path back");
}

/// #145: Blend Along Full Path and Rotate All Objects, from the property bar's Options and from
/// the Blend docker, which shows them as the blend has them; Detach from Path puts the circle
/// back on the page and the blend goes straight again.
#[test]
fn blend_full_path_rotate_all_and_the_docker_follow_each_other() {
    use crate::dockers::DockerId;
    use crate::tools::ToolId;
    use egui_kittest::kittest::{NodeT, Queryable};
    let mut h = join_harness(ToolId::Blend);
    let [blend, _, _, path] = bp_on_path(&mut h);
    {
        let a = h.state_mut();
        a.dockers.open.clear();
        a.dockers.show(DockerId::Blend);
        a.dockers.collapsed = false;
        a.dockers.set_width = Some(320.0);
    }
    settle(&mut h);
    let checked = |h: &Harness<'_, AnyVectorApp>, label: &str| {
        h.query_all_by_label(label).find(|n| n.accesskit_node().role() == egui::accesskit::Role::CheckBox).map(|n| n.accesskit_node().toggled() == Some(egui::accesskit::Toggled::True))
    };
    assert_eq!(checked(&h, "Blend along full path"), Some(false));
    // from the bar: the docker follows
    run_cmd(&mut h, Cmd::BarOption(0x247d0e09));
    settle(&mut h);
    let b = bp_blend(&h, blend);
    assert!(b.path.as_ref().unwrap().full);
    assert_eq!(bp_state(&h, "247d0e09"), Some((true, true)));
    assert_eq!(checked(&h, "Blend along full path"), Some(true), "the docker didn't follow the bar");
    // on the whole circle the steps are evenly spread
    let c = bp_centres(&b);
    let gaps: Vec<f64> = c.windows(2).map(|w| (w[1] - w[0]).hypot()).collect();
    let (lo, hi) = gaps.iter().fold((f64::MAX, 0.0f64), |(lo, hi), g| (lo.min(*g), hi.max(*g)));
    assert!(hi - lo < 1.0, "uneven steps on the full circle: {gaps:?}");
    // from the docker: the bar follows
    let at = h.query_all_by_label("Rotate all objects").find(|n| n.accesskit_node().role() == egui::accesskit::Role::CheckBox).unwrap().rect().center();
    click(&mut h, at);
    settle(&mut h);
    assert!(bp_blend(&h, blend).path.as_ref().unwrap().rotate, "the docker's Rotate all objects did nothing");
    assert_eq!(bp_state(&h, "50c0c56f"), Some((true, true)));
    assert_eq!(checked(&h, "Rotate all objects"), Some(true));
    // Detach from Path
    run_cmd(&mut h, Cmd::BarOption(0x3726add2));
    settle(&mut h);
    let d = h.state().doc().unwrap();
    assert!(d.doc.locate(path).is_some(), "the path isn't back on the page");
    assert!(bp_blend(&h, blend).path.is_none());
    assert_eq!(checked(&h, "Blend along full path"), Some(false));
}

/// #145: Split (click a step: the blend passes through it, and it can be moved on its own),
/// Fuse end (the split goes back), Map nodes (a node of the start, then one of the end) and
/// fixed spacing (the Spacing radio button), each one undo step.
#[test]
fn blend_split_fuse_map_nodes_and_spacing() {
    use crate::tools::{PickTarget, ToolId};
    use vector_core::object::BlendPart;
    let mut h = join_harness(ToolId::Blend);
    let [blend, _, _, _] = bp_on_path(&mut h);
    assert_eq!(bp_state(&h, "e022c80a"), Some((false, false)), "Fuse start without a split");
    // Split at the fourth step
    let undo0 = undo_steps(&h);
    run_cmd(&mut h, Cmd::BarOption(0x339589f8));
    assert!(matches!(h.state().ts.pick_target, Some(PickTarget::Blend(crate::blendpath::Pick::Split { .. }))));
    let p = bp_centres(&bp_blend(&h, blend))[3];
    pick_click(&mut h, (p.x, p.y), egui::Modifiers::NONE);
    let b = bp_blend(&h, blend);
    assert_eq!(b.splits.len(), 1);
    assert_eq!(sel(&h), vec![b.splits[0].obj.id], "the split object is selected on its own");
    assert_eq!(crate::blendpath::focus(h.state().doc().unwrap()), Some((blend, BlendPart::Split(0))));
    assert!(crate::statusbar::object_info(h.state()).starts_with("Blend split: "));
    assert_eq!(undo_steps(&h), undo0 + 1);
    assert_eq!(bp_state(&h, "ba9b75cf"), Some((true, false)), "Fuse end with a split");
    run_cmd(&mut h, Cmd::BarOption(0xba9b75cf));
    assert!(bp_blend(&h, blend).splits.is_empty());
    assert_eq!(sel(&h), vec![blend]);
    // Map nodes: a node of the start, then the matching node of the end
    let b = bp_blend(&h, blend);
    let (na, nb) = (vector_core::blend_path::map_candidates(&b.start), vector_core::blend_path::map_candidates(&b.end));
    assert!(!na.is_empty() && !nb.is_empty());
    run_cmd(&mut h, Cmd::BarOption(0xc5cd624e));
    let q = na[na.len() / 2].0;
    pick_click(&mut h, (q.x, q.y), egui::Modifiers::NONE);
    assert!(matches!(h.state().ts.pick_target, Some(PickTarget::Blend(crate::blendpath::Pick::MapNodes { first: Some(_), .. }))));
    let q = nb[0].0;
    pick_click(&mut h, (q.x, q.y), egui::Modifiers::NONE);
    assert!(bp_blend(&h, blend).map.is_some(), "Map nodes did nothing");
    assert!(h.state().ts.pick_target.is_none());
    // fixed spacing on the path: the Spacing radio button, and back to steps
    assert_eq!(bp_state(&h, "46327bd4"), Some((true, false)));
    run_cmd(&mut h, Cmd::BarOption(0x46327bd4));
    let b = bp_blend(&h, blend);
    assert!(b.spacing.is_some_and(|s| s > 1.0), "{:?}", b.spacing);
    assert_eq!(bp_state(&h, "46327bd4"), Some((true, true)));
    assert_eq!(bp_state(&h, "5cefe9f1"), Some((true, false)));
    // the spacing the radio button chose keeps the blend as it was
    assert_eq!(vector_core::blend_path::effective_steps(&b), 8);
    run_cmd(&mut h, Cmd::BarOption(0x5cefe9f1));
    let b = bp_blend(&h, blend);
    assert!(b.spacing.is_none());
    assert_eq!(b.steps, 8);
    // Clear Blend: the start, the end and the path are objects of their own again
    run_cmd(&mut h, Cmd::BarOption(0xf1eb59d2));
    let d = h.state().doc().unwrap();
    assert!(d.doc.find(blend).is_none());
    assert!(d.doc.locate(b.path.as_ref().unwrap().obj.id).is_some(), "Clear Blend lost the path");
}

/// #145: blends along paths, as drawn: circles round a circle (as it is, along the full path,
/// with Rotate All Objects), the start shown on its own, Map nodes waiting for a node, the Blend
/// docker with its options, and a star to a square along an open wave with fixed spacing.
#[test]
#[ignore]
fn visual_issue_145_blend_along_a_path() {
    use crate::dockers::DockerId;
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().set_tool(ToolId::Blend);
    let [blend, _, _, _] = bp_on_path(&mut h);
    h.state_mut().doc_mut().unwrap().selection = vec![blend];
    save(&mut h, "issue_145_01_circles_on_a_circle");
    run_cmd(&mut h, Cmd::BarOption(0x247d0e09));
    save(&mut h, "issue_145_02_full_path");
    run_cmd(&mut h, Cmd::BarOption(0x50c0c56f));
    save(&mut h, "issue_145_03_rotate_all");
    h.state_mut().set_tool(ToolId::Pick);
    run_cmd(&mut h, Cmd::BarOption(0x7341835a));
    save(&mut h, "issue_145_04_show_start");
    h.state_mut().set_tool(ToolId::Blend);
    h.state_mut().doc_mut().unwrap().selection = vec![blend];
    run_cmd(&mut h, Cmd::BarOption(0xc5cd624e));
    save(&mut h, "issue_145_05_map_nodes_waits");
    h.key_press(egui::Key::Escape);
    h.step();
    {
        let a = h.state_mut();
        a.ts.pick_target = None;
        a.dockers.open.clear();
        a.dockers.show(DockerId::Blend);
        a.dockers.collapsed = false;
        a.dockers.set_width = Some(320.0);
        a.dockers.ui.blend_more = true;
        a.doc_mut().unwrap().selection = vec![blend];
    }
    save(&mut h, "issue_145_06_docker");
    // a star to a square along an open wave, 30 pt apart, turned along it
    let d = h.state_mut().doc_mut().unwrap();
    let mut st = Style::default();
    st.fill = Fill::Solid(Color::rgb(240, 180, 30));
    let star = Object::new(d.doc.new_id(), ObjKind::Polygon(PolygonShape { kind: PolygonKind::Star, sides: 5, sharpness: 50.0, w: 50.0, h: 50.0, nodes: Default::default() }), Affine::translate((95.0, 655.0)), st.clone());
    st.fill = Fill::Solid(Color::rgb(30, 160, 90));
    let square = Object::new(d.doc.new_id(), ObjKind::Rect(RectShape { w: 40.0, h: 40.0, radii: [0.0; 4], corner: CornerType::default() }), Affine::translate((480.0, 660.0)), st);
    let mut wave = kurbo::BezPath::new();
    wave.move_to((120.0, 680.0));
    wave.curve_to((220.0, 560.0), (300.0, 800.0), (380.0, 680.0));
    wave.curve_to((420.0, 620.0), (470.0, 640.0), (500.0, 680.0));
    let wave = Object::new(d.doc.new_id(), ObjKind::Path(vector_core::path::NodePath::from_bez(&wave)), Affine::IDENTITY, Style::default());
    let (a, b, w) = (star.id, square.id, wave.id);
    d.add_objects("test", vec![star, square, wave]);
    let second = d.modify("Blend", |doc, _| vector_core::ops::create_blend(doc, a, b, 6)).unwrap();
    d.selection = vec![second, w];
    run_cmd(&mut h, Cmd::BarOption(0x42941bfb));
    run_cmd(&mut h, Cmd::BarOption(0x247d0e09));
    run_cmd(&mut h, Cmd::BarOption(0x50c0c56f));
    run_cmd(&mut h, Cmd::BarOption(0x46327bd4));
    crate::blendpath::edit(h.state_mut(), "Blend", |b| b.spacing = Some(30.0));
    h.state_mut().dockers.open.clear();
    save(&mut h, "issue_145_07_star_to_square_on_a_wave");
    assert!(bp_blend(&h, second).path.as_ref().is_some_and(|p| p.full && p.rotate));
}

// ---- 0.6.2: Ctrl+click inside a group (#151), a blend's start and end picked and dragged (#149) ----

const CTRL_SHIFT: egui::Modifiers = egui::Modifiers { alt: false, ctrl: true, shift: true, mac_cmd: false, command: true };

fn si_rect(d: &mut crate::docview::DocView, at: (f64, f64), w: f64, rgb: (u8, u8, u8)) -> Object {
    let r = RectShape { w, h: w, radii: [0.0; 4], corner: CornerType::default() };
    let st = Style { fill: Fill::Solid(Color::rgb(rgb.0, rgb.1, rgb.2)), ..Style::default() };
    Object::new(d.doc.new_id(), ObjKind::Rect(r), Affine::translate(at), st)
}

fn si_group(d: &mut crate::docview::DocView, children: Vec<Object>) -> Object {
    Object::new(d.doc.new_id(), ObjKind::Group(children), Affine::IDENTITY, Style::default())
}

/// #151: a red rectangle R (100,100)–(200,200) and a blue circle E (250,100)–(350,200) grouped
/// (G); a group G2 of a green rectangle R2 (100,300)–(200,400) and a group G3 of two rectangles,
/// R3 (250,300)–(350,400) and R4 (400,300)–(500,400); and a square S (600,100)–(700,200) of its
/// own. Nothing selected. Gives [G, R, E, G2, R2, G3, R3, R4, S].
fn si_groups(h: &mut Harness<'_, AnyVectorApp>) -> [u64; 9] {
    let d = h.state_mut().doc_mut().unwrap();
    let r = si_rect(d, (100.0, 100.0), 100.0, (220, 40, 40));
    let circle = EllipseShape { w: 100.0, h: 100.0, kind: EllipseKind::Ellipse, start: 0.0, end: 0.0, clockwise: false };
    let e = Object::new(d.doc.new_id(), ObjKind::Ellipse(circle), Affine::translate((250.0, 100.0)), Style { fill: Fill::Solid(Color::rgb(40, 90, 220)), ..Style::default() });
    let r2 = si_rect(d, (100.0, 300.0), 100.0, (40, 160, 80));
    let r3 = si_rect(d, (250.0, 300.0), 100.0, (240, 180, 30));
    let r4 = si_rect(d, (400.0, 300.0), 100.0, (160, 60, 200));
    let s = si_rect(d, (600.0, 100.0), 100.0, (90, 90, 90));
    let ids = [r.id, e.id, r2.id, r3.id, r4.id, s.id];
    let g = si_group(d, vec![r, e]);
    let g3 = si_group(d, vec![r3, r4]);
    let (gid, g3id) = (g.id, g3.id);
    let g2 = si_group(d, vec![r2, g3]);
    let g2id = g2.id;
    d.add_objects("test", vec![g, g2, s]);
    d.selection.clear();
    [gid, ids[0], ids[1], g2id, ids[2], g3id, ids[3], ids[4], ids[5]]
}

/// #151: a plain click selects the group; Ctrl+click selects the object of the group under the
/// pointer, with its own handles (the selection box is the object's) and "Child Rectangle on
/// Layer 1" in the status bar; Ctrl+click on another object of the group picks that one; a press
/// on the object picked goes on with it; a plain click elsewhere on the group gives the group
/// back; Ctrl+click off a group is a plain click; a click on empty space selects nothing.
#[test]
fn issue_151_ctrl_click_selects_an_object_of_a_group() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let [g, r, e, _, _, _, _, _, s] = si_groups(&mut h);
    let none = egui::Modifiers::NONE;
    pick_click(&mut h, (150.0, 150.0), none);
    assert_eq!(sel(&h), vec![g], "a plain click selects the group");
    pick_click(&mut h, (150.0, 150.0), CTRL);
    assert_eq!(sel(&h), vec![r], "Ctrl+click didn't select the rectangle in the group");
    assert_eq!(h.state().doc().unwrap().selection_bounds(), Some(bounds_of(&h, r)), "the handles aren't the rectangle's");
    let info = crate::statusbar::object_info(h.state());
    assert!(info.starts_with("Child Rectangle on ") && !info.ends_with(" on "), "{info}");
    assert!(!h.state().doc().unwrap().rotate_mode);
    // another object of the same group
    pick_click(&mut h, (300.0, 150.0), CTRL);
    assert_eq!(sel(&h), vec![e]);
    assert!(crate::statusbar::object_info(h.state()).starts_with("Child Ellipse on "));
    // a plain press on the object picked keeps it (a second click is for rotating, as anywhere)
    pick_click(&mut h, (300.0, 150.0), none);
    assert_eq!(sel(&h), vec![e]);
    assert!(h.state().doc().unwrap().rotate_mode, "a second click on the object picked didn't turn on the rotate handles");
    pick_click(&mut h, (300.0, 150.0), none);
    // a plain click on the group off that object: the group again
    pick_click(&mut h, (150.0, 150.0), none);
    assert_eq!(sel(&h), vec![g]);
    // Ctrl+click on an object that is in no group: as a plain click
    pick_click(&mut h, (650.0, 150.0), CTRL);
    assert_eq!(sel(&h), vec![s]);
    assert!(crate::statusbar::object_info(h.state()).starts_with("Rectangle on "));
    // empty space: nothing, not the group
    pick_click(&mut h, (150.0, 150.0), CTRL);
    pick_click(&mut h, (225.0, 250.0), none);
    assert!(sel(&h).is_empty());
}

/// #151: in nested groups each Ctrl+click goes one level down (the inner group, then its
/// object); at the bottom it stays there; with an object of the inner group picked, a Ctrl+click
/// on the other one picks it; in another branch, the object there.
#[test]
fn issue_151_ctrl_click_goes_one_level_down_per_click() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let [_, _, _, g2, r2, g3, r3, r4, _] = si_groups(&mut h);
    pick_click(&mut h, (300.0, 350.0), egui::Modifiers::NONE);
    assert_eq!(sel(&h), vec![g2]);
    pick_click(&mut h, (300.0, 350.0), CTRL);
    assert_eq!(sel(&h), vec![g3], "the first Ctrl+click selects the inner group");
    let info = crate::statusbar::object_info(h.state());
    assert!(info.starts_with("Child Group of 2 objects on "), "{info}");
    pick_click(&mut h, (300.0, 350.0), CTRL);
    assert_eq!(sel(&h), vec![r3], "the second Ctrl+click selects the rectangle in it");
    pick_click(&mut h, (300.0, 350.0), CTRL);
    assert_eq!(sel(&h), vec![r3], "at the bottom it stays");
    pick_click(&mut h, (450.0, 350.0), CTRL);
    assert_eq!(sel(&h), vec![r4], "the other object of the inner group");
    pick_click(&mut h, (150.0, 350.0), CTRL);
    assert_eq!(sel(&h), vec![r2]);
    // from nothing selected, the inner group's object takes two
    pick_click(&mut h, (700.0, 700.0), egui::Modifiers::NONE);
    pick_click(&mut h, (450.0, 350.0), CTRL);
    assert_eq!(sel(&h), vec![g3]);
}

/// #151: Ctrl+Shift+click adds objects of groups to the selection (never their group with them)
/// and takes them off again; Shift+click on the group puts the group in place of its objects;
/// with the group selected, Ctrl+Shift+click puts the object in its place.
#[test]
fn issue_151_ctrl_shift_click_adds_and_removes_objects_of_groups() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let [g, r, e, _, _, g3, _, _, s] = si_groups(&mut h);
    let shift = egui::Modifiers { shift: true, ..egui::Modifiers::NONE };
    pick_click(&mut h, (150.0, 150.0), CTRL_SHIFT);
    assert_eq!(sel(&h), vec![r]);
    pick_click(&mut h, (300.0, 150.0), CTRL_SHIFT);
    assert_eq!(sel(&h), vec![r, e], "Ctrl+Shift+click didn't add the circle");
    assert!(crate::statusbar::object_info(h.state()).starts_with("2 objects selected on Layer"));
    // an object of another group, and one of no group
    pick_click(&mut h, (300.0, 350.0), CTRL_SHIFT);
    pick_click(&mut h, (650.0, 150.0), shift);
    assert_eq!(sel(&h), vec![r, e, g3, s]);
    // again on one: it is taken off (so does Shift+click on one, as on any object selected)
    pick_click(&mut h, (300.0, 150.0), CTRL_SHIFT);
    assert_eq!(sel(&h), vec![r, g3, s]);
    pick_click(&mut h, (150.0, 150.0), shift);
    assert_eq!(sel(&h), vec![g3, s]);
    // Shift+click on the group off its object selected: the group in place of that object
    pick_click(&mut h, (300.0, 150.0), CTRL_SHIFT);
    assert_eq!(sel(&h), vec![g3, s, e]);
    pick_click(&mut h, (150.0, 150.0), shift);
    assert_eq!(sel(&h), vec![g3, s, g]);
    // and Ctrl+Shift+click on the group selected: its object in place of the group
    pick_click(&mut h, (300.0, 150.0), CTRL_SHIFT);
    assert_eq!(sel(&h), vec![g3, s, e]);
}

/// #151: a Ctrl+click that drags moves only the object of the group (the group grows), as one
/// undo step that Undo takes back; the object picked then drags without Ctrl; Ctrl held during
/// the drag still keeps the move straight, as Ctrl always does.
#[test]
fn issue_151_dragging_an_object_of_a_group_moves_only_it() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let [g, r, e, _, _, _, _, _, _] = si_groups(&mut h);
    let (rb, eb, gb) = (bounds_of(&h, r), bounds_of(&h, e), bounds_of(&h, g));
    let undo0 = undo_steps(&h);
    shape_drag(&mut h, (150.0, 150.0), (150.0, 230.0), CTRL);
    assert_eq!(sel(&h), vec![r]);
    let now = bounds_of(&h, r);
    assert!((now.y0 - rb.y0 - 80.0).abs() < 0.01 && (now.x0 - rb.x0).abs() < 0.01, "the rectangle didn't move: {rb:?} → {now:?}");
    assert_eq!(bounds_of(&h, e), eb, "the circle moved with it");
    assert!((bounds_of(&h, g).y1 - gb.y1 - 80.0).abs() < 0.01, "the group didn't grow");
    assert_eq!(h.state().doc().unwrap().doc.parent_of(r), Some(g), "the rectangle left the group");
    assert_eq!(undo_steps(&h), undo0 + 1, "one undo step");
    undo(&mut h);
    assert_eq!(bounds_of(&h, r), rb);
    // picked, it drags without Ctrl; with Ctrl the move stays straight
    pick_click(&mut h, (150.0, 150.0), CTRL);
    shape_drag(&mut h, (150.0, 150.0), (190.0, 160.0), egui::Modifiers::NONE);
    let now = bounds_of(&h, r);
    assert!((now.x0 - rb.x0 - 40.0).abs() < 0.01 && (now.y0 - rb.y0 - 10.0).abs() < 0.01, "{now:?}");
    assert_eq!(bounds_of(&h, e), eb);
    shape_drag(&mut h, (170.0, 170.0), (230.0, 178.0), CTRL);
    let then = bounds_of(&h, r);
    assert!((then.x0 - now.x0 - 60.0).abs() < 0.01 && (then.y0 - now.y0).abs() < 0.01, "Ctrl didn't keep the move straight: {now:?} → {then:?}");
    assert_eq!(sel(&h), vec![r]);
}

/// #151: Delete on an object of a group deletes it alone (the group stays, one undo step); a
/// copy of it stays in the group; deleting every object of a group deletes the group, and Undo
/// brings it all back.
#[test]
fn issue_151_delete_and_duplicate_an_object_of_a_group() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let [g, r, e, _, _, _, _, _, _] = si_groups(&mut h);
    pick_click(&mut h, (150.0, 150.0), CTRL);
    let undo0 = undo_steps(&h);
    run_cmd(&mut h, Cmd::Delete);
    let d = h.state().doc().unwrap();
    assert!(d.doc.find(r).is_none(), "the rectangle is still there");
    assert!(d.doc.find(e).is_some() && d.doc.find(g).is_some(), "the group or the circle went too");
    assert_eq!(undo_steps(&h), undo0 + 1);
    undo(&mut h);
    assert_eq!(h.state().doc().unwrap().doc.parent_of(r), Some(g), "Undo didn't put the rectangle back in its group");
    // Duplicate: the copy is in the group, selected
    pick_click(&mut h, (150.0, 150.0), CTRL);
    run_cmd(&mut h, Cmd::Duplicate);
    let copy = sel(&h);
    assert_eq!(copy.len(), 1);
    assert_ne!(copy[0], r);
    assert_eq!(h.state().doc().unwrap().doc.parent_of(copy[0]), Some(g), "the copy left the group");
    undo(&mut h);
    // every object of the group: the group goes
    pick_click(&mut h, (150.0, 150.0), CTRL);
    pick_click(&mut h, (300.0, 150.0), CTRL_SHIFT);
    assert_eq!(sel(&h), vec![r, e]);
    run_cmd(&mut h, Cmd::Delete);
    assert!(h.state().doc().unwrap().doc.find(g).is_none(), "an empty group was left");
    undo(&mut h);
    let d = h.state().doc().unwrap();
    assert!(d.doc.find(g).is_some() && d.doc.parent_of(e) == Some(g));
}

/// #151: Ctrl's other meanings stay: with the Windows modifier keys (Ctrl is Duplicate) a
/// Ctrl+click selects the group; a double-click on a group is Focus Mode, where Ctrl+click picks
/// inside the group too; Alt+click still selects what is behind (#139), not inside; the marquee
/// selects whole groups; Ctrl+click on a PowerClip enters it (#111).
#[test]
fn issue_151_ctrl_click_keeps_its_other_meanings() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let [g, r, _, g2, _, _, _, _, _] = si_groups(&mut h);
    h.state_mut().settings.windows_modifiers = true;
    pick_click(&mut h, (150.0, 150.0), CTRL);
    assert_eq!(sel(&h), vec![g], "with Ctrl as Duplicate, Ctrl+click went inside the group");
    h.state_mut().settings.windows_modifiers = false;
    // Focus Mode by double-click, and Ctrl+click inside it
    h.state_mut().settings.dblclick_focus_mode = true;
    pick_click(&mut h, (700.0, 700.0), egui::Modifiers::NONE);
    pick_double(&mut h, (150.0, 150.0));
    assert_eq!(h.state().doc().unwrap().focus, vec![g], "the double-click didn't enter Focus Mode");
    pick_click(&mut h, (150.0, 150.0), CTRL);
    assert_eq!(sel(&h), vec![r]);
    h.state_mut().doc_mut().unwrap().focus.clear();
    // Alt+click: the whole object behind
    let behind = square_at(&mut h, (120.0, 120.0), 40.0, (0, 0, 0));
    h.state_mut().doc_mut().unwrap().doc.pages[0].layers.last_mut().unwrap().objects.rotate_right(1);
    pick_click(&mut h, (140.0, 140.0), egui::Modifiers::NONE);
    assert_eq!(sel(&h), vec![g]);
    pick_click(&mut h, (140.0, 140.0), ALT);
    assert_eq!(sel(&h), vec![behind], "Alt+click went inside the group instead of behind it");
    // the marquee: whole groups
    shape_drag(&mut h, (90.0, 280.0), (520.0, 420.0), egui::Modifiers::NONE);
    assert_eq!(sel(&h), vec![g2]);
    // a PowerClip: Ctrl+click enters it, as before
    let (mut h2, [pc, ..]) = powerclip_doc();
    pick_click(&mut h2, (300.0, 200.0), CTRL);
    assert_eq!(editing(&h2), Some(pc), "Ctrl+click no longer enters the PowerClip");
}

/// #149: a straight blend of a red circle at (200, 400) (radius 25) to a blue one at (500, 400)
/// (radius 35) in 6 steps, selected. Gives [blend, start, end].
fn si_blend(h: &mut Harness<'_, AnyVectorApp>) -> [u64; 3] {
    let d = h.state_mut().doc_mut().unwrap();
    let a = bp_circle(d, (200.0, 400.0), 25.0, Fill::Solid(Color::rgb(220, 40, 40)));
    let b = bp_circle(d, (500.0, 400.0), 35.0, Fill::Solid(Color::rgb(40, 80, 220)));
    let (ia, ib) = (a.id, b.id);
    d.add_objects("test", vec![a, b]);
    let blend = d.modify("Blend", |doc, _| vector_core::ops::create_blend(doc, ia, ib, 6)).unwrap();
    d.selection = vec![blend];
    [blend, ia, ib]
}

fn si_focus(h: &Harness<'_, AnyVectorApp>) -> Option<(u64, vector_core::object::BlendPart)> {
    crate::blendpath::focus(h.state().doc().unwrap())
}

/// #149: the blend selected shows a grip on its start and end; a click on its start selects the
/// start (the status bar names it) and one on its end the end; a click on a step selects the
/// blend again; with nothing selected a click selects the blend, the next one the control; the
/// grip picks too; Shift+click stays a plain multi-select; a locked blend gives no part.
#[test]
fn issue_149_click_selects_the_start_and_end_of_the_blend() {
    use crate::tools::ToolId;
    use vector_core::object::BlendPart;
    let mut h = join_harness(ToolId::Pick);
    let [blend, start, end] = si_blend(&mut h);
    let none = egui::Modifiers::NONE;
    let grips = crate::blendpath::grips(h.state().doc().unwrap());
    assert_eq!(grips.iter().map(|g| (g.0, g.1)).collect::<Vec<_>>(), vec![(BlendPart::Start, start), (BlendPart::End, end)]);
    assert!((grips[0].2 - scr(&h, (200.0, 400.0))).length() < 0.5, "the start's grip isn't in its middle");
    pick_click(&mut h, (200.0, 400.0), none);
    assert_eq!(sel(&h), vec![start], "a click on the start of the blend selected didn't select it");
    assert_eq!(si_focus(&h), Some((blend, BlendPart::Start)));
    let info = crate::statusbar::object_info(h.state());
    assert!(info.starts_with("Blend start: Ellipse on "), "{info}");
    assert!(!h.state().doc().unwrap().rotate_mode);
    // the start selected: the end keeps its grip
    let grips = crate::blendpath::grips(h.state().doc().unwrap());
    assert_eq!(grips.iter().map(|g| g.1).collect::<Vec<_>>(), vec![end]);
    pick_click(&mut h, (500.0, 400.0), none);
    assert_eq!(sel(&h), vec![end]);
    assert!(crate::statusbar::object_info(h.state()).starts_with("Blend end: "));
    // a step: the blend again
    let c = bp_centres(&bp_blend(&h, blend))[2];
    pick_click(&mut h, (c.x, c.y), none);
    assert_eq!(sel(&h), vec![blend], "a click on a step didn't select the blend again");
    assert_eq!(si_focus(&h), None);
    // nothing selected: a click on the start selects the blend, the next one the start
    pick_click(&mut h, (700.0, 700.0), none);
    pick_click(&mut h, (200.0, 400.0), none);
    assert_eq!(sel(&h), vec![blend]);
    pick_click(&mut h, (200.0, 400.0), none);
    assert_eq!(sel(&h), vec![start]);
    // the grip, on the end's middle
    h.state_mut().doc_mut().unwrap().selection = vec![blend];
    let at = crate::blendpath::grips(h.state().doc().unwrap())[1].2;
    assert_eq!(crate::blendpath::grip_at(h.state().doc().unwrap(), at + egui::vec2(3.0, -3.0)), Some((BlendPart::End, end)));
    assert_eq!(crate::blendpath::grip_at(h.state().doc().unwrap(), at + egui::vec2(30.0, 0.0)), None);
    // Shift+click is a plain multi-select: the blend comes off
    pick_click(&mut h, (200.0, 400.0), egui::Modifiers { shift: true, ..none });
    assert!(sel(&h).is_empty());
    // a locked blend: no grips, no part picked
    h.state_mut().doc_mut().unwrap().doc.find_mut(blend).unwrap().locked = true;
    h.state_mut().doc_mut().unwrap().selection = vec![blend];
    assert!(crate::blendpath::grips(h.state().doc().unwrap()).is_empty());
    pick_click(&mut h, (200.0, 400.0), none);
    assert_eq!(sel(&h), vec![blend], "a part of a locked blend was picked");
}

/// #149: dragging the start of a straight blend moves it freely and the steps follow, as one
/// undo step; the end drags the same way; on a blend along a path the end slides along the path
/// and the steps stay on it; the path itself is picked on its outline.
#[test]
fn issue_149_dragging_the_start_or_end_reshapes_the_blend() {
    use crate::tools::ToolId;
    let mut h = join_harness(ToolId::Pick);
    let [blend, start, end] = si_blend(&mut h);
    let none = egui::Modifiers::NONE;
    let before = bp_centres(&bp_blend(&h, blend));
    let undo0 = undo_steps(&h);
    // the blend selected: a drag from its start moves the start
    shape_drag(&mut h, (200.0, 400.0), (230.0, 300.0), none);
    assert_eq!(sel(&h), vec![start]);
    let b = bp_blend(&h, blend);
    assert!((b.start.bounds().center() - Point::new(230.0, 300.0)).hypot() < 0.01, "{:?}", b.start.bounds().center());
    assert!((b.end.bounds().center() - Point::new(500.0, 400.0)).hypot() < 0.01, "the end moved");
    let now = bp_centres(&b);
    assert_eq!(now.len(), before.len());
    for (p, q) in before.iter().zip(&now) {
        assert!(q.y < p.y - 1.0 && q.y > 300.0, "a step didn't follow: {p:?} → {q:?}");
    }
    assert_eq!(undo_steps(&h), undo0 + 1, "one undo step");
    undo(&mut h);
    assert!((bp_blend(&h, blend).start.bounds().center() - Point::new(200.0, 400.0)).hypot() < 0.01);
    // the end, picked then dragged
    h.state_mut().doc_mut().unwrap().selection = vec![blend];
    pick_click(&mut h, (500.0, 400.0), none);
    shape_drag(&mut h, (500.0, 400.0), (520.0, 480.0), none);
    let b = bp_blend(&h, blend);
    assert!((b.end.bounds().center() - Point::new(520.0, 480.0)).hypot() < 0.01);
    assert!(bp_centres(&b).iter().all(|p| p.y > 400.0 && p.y < 480.0), "the steps didn't follow the end");
    assert_eq!(sel(&h), vec![end]);

    // a blend along a path: the end slides along it
    let mut h = join_harness(ToolId::Pick);
    let [blend, _, end, path] = bp_on_path(&mut h);
    h.state_mut().doc_mut().unwrap().selection = vec![blend];
    let e = bp_blend(&h, blend).end.bounds().center();
    shape_drag(&mut h, (e.x, e.y), (e.x - 30.0, e.y + 100.0), none);
    assert_eq!(sel(&h), vec![end]);
    let b = bp_blend(&h, blend);
    let now = b.end.bounds().center();
    assert!((now - e).hypot() > 20.0, "the end didn't move: {e:?} → {now:?}");
    assert!(bp_off(now) < 1.0, "the end left the path: {now:?}");
    for p in bp_centres(&b) {
        assert!(bp_off(p) < 1.0, "a step off the circle: {p:?}");
    }
    // the path is a control too: a click on its outline where no step is selects it
    h.state_mut().doc_mut().unwrap().selection = vec![blend];
    let free = (0..72)
        .map(|k| {
            let a = k as f64 * std::f64::consts::TAU / 72.0;
            Point::new(400.0 + 150.0 * a.cos(), 400.0 + 150.0 * a.sin())
        })
        .find(|q| bp_centres(&b).iter().chain([b.start.bounds().center(), b.end.bounds().center()].iter()).all(|c| (*c - *q).hypot() > 45.0))
        .expect("a free spot on the path");
    pick_click(&mut h, (free.x, free.y), none);
    assert_eq!(sel(&h), vec![path], "a click on the path of the blend selected didn't select the path");
    assert!(crate::statusbar::object_info(h.state()).starts_with("Blend path: "));
}

/// #149: Ctrl+click goes straight to a control of a blend, even unselected, and Ctrl+Shift+click
/// adds another; Show Start still selects the start and Esc lets go of it; Delete on an end
/// picked by a click takes the blend apart (one undo step); a blend in a group takes two
/// Ctrl+clicks (the blend, then its control), and then a click on a step gives the blend back,
/// not the group.
#[test]
fn issue_149_ctrl_click_delete_and_show_start() {
    use crate::tools::ToolId;
    use vector_core::object::BlendPart;
    let mut h = join_harness(ToolId::Pick);
    let [blend, start, end] = si_blend(&mut h);
    let none = egui::Modifiers::NONE;
    h.state_mut().doc_mut().unwrap().selection.clear();
    pick_click(&mut h, (200.0, 400.0), CTRL);
    assert_eq!(sel(&h), vec![start], "Ctrl+click didn't select the start");
    pick_click(&mut h, (500.0, 400.0), CTRL_SHIFT);
    assert_eq!(sel(&h), vec![start, end]);
    // Show Start, Esc
    h.state_mut().doc_mut().unwrap().selection = vec![blend];
    run_cmd(&mut h, Cmd::BarOption(0x7341835a));
    assert_eq!(sel(&h), vec![start]);
    h.key_press(egui::Key::Escape);
    h.step();
    assert!(sel(&h).is_empty());
    // the end clicked, then Delete: the blend comes apart, the start stays on the page
    h.state_mut().doc_mut().unwrap().selection = vec![blend];
    pick_click(&mut h, (500.0, 400.0), none);
    assert_eq!(si_focus(&h), Some((blend, BlendPart::End)));
    let undo0 = undo_steps(&h);
    run_cmd(&mut h, Cmd::Delete);
    let d = h.state().doc().unwrap();
    assert!(d.doc.find(blend).is_none() && d.doc.find(end).is_none());
    assert!(d.doc.locate(start).is_some());
    assert_eq!(d.selection, vec![start]);
    assert_eq!(undo_steps(&h), undo0 + 1);
    undo(&mut h);
    assert!(h.state().doc().unwrap().doc.find(blend).is_some());
    // a blend in a group
    let other = square_at(&mut h, (100.0, 600.0), 50.0, (0, 0, 0));
    let g = h.state_mut().doc_mut().unwrap().modify("Group", |doc, _| vector_core::ops::group(doc, &[blend, other])).unwrap();
    h.state_mut().doc_mut().unwrap().selection.clear();
    pick_click(&mut h, (200.0, 400.0), none);
    assert_eq!(sel(&h), vec![g]);
    pick_click(&mut h, (200.0, 400.0), CTRL);
    assert_eq!(sel(&h), vec![blend]);
    pick_click(&mut h, (200.0, 400.0), CTRL);
    assert_eq!(sel(&h), vec![start]);
    let c = bp_centres(&bp_blend(&h, blend))[2];
    pick_click(&mut h, (c.x, c.y), none);
    assert_eq!(sel(&h), vec![blend], "a step of the blend in the group gave the group, not the blend");
    pick_click(&mut h, (500.0, 400.0), none);
    assert_eq!(sel(&h), vec![end], "the blend in a group: its end wasn't picked");
}

/// #151: a group, then an object of it picked with Ctrl+click (its own handles), two picked with
/// Ctrl+Shift+click, one level per Ctrl+click in nested groups, and the object dragged out of its
/// place (the group around it grows).
#[test]
#[ignore]
fn visual_issue_151_ctrl_click_inside_a_group() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().set_tool(ToolId::Pick);
    h.state_mut().doc_mut().unwrap().doc.settings.snap_off = true;
    let [g, r, e, _, _, g3, r3, _, _] = si_groups(&mut h);
    let q = scr(&h, (150.0, 150.0));
    click(&mut h, q);
    assert_eq!(selection(&h), vec![g]);
    save(&mut h, "issue_151_01_click_selects_the_group");
    let q = scr(&h, (150.0, 150.0));
    click_mods(&mut h, q, CTRL);
    assert_eq!(selection(&h), vec![r]);
    save(&mut h, "issue_151_02_ctrl_click_selects_the_rectangle");
    let q = scr(&h, (300.0, 150.0));
    click_mods(&mut h, q, CTRL_SHIFT);
    assert_eq!(selection(&h), vec![r, e]);
    save(&mut h, "issue_151_03_ctrl_shift_click_adds_the_circle");
    let q = scr(&h, (300.0, 350.0));
    click_mods(&mut h, q, CTRL);
    assert_eq!(selection(&h), vec![g3]);
    let q = scr(&h, (300.0, 350.0));
    click_mods(&mut h, q, CTRL);
    assert_eq!(selection(&h), vec![r3]);
    save(&mut h, "issue_151_04_nested_one_level_per_click");
    let q = scr(&h, (150.0, 150.0));
    click_mods(&mut h, q, CTRL);
    let (a, b) = (scr(&h, (150.0, 150.0)), scr(&h, (150.0, 220.0)));
    drag(&mut h, a, b);
    assert_eq!(selection(&h), vec![r]);
    assert!((bounds_of(&h, r).y0 - 170.0).abs() < 0.5, "{:?}", bounds_of(&h, r));
    save(&mut h, "issue_151_05_the_rectangle_dragged_out");
}

/// #149: a blend selected shows grips on its start and end; the start clicked is selected with
/// its handles; while it is dragged the steps follow it; dropped, the blend is reshaped; a blend
/// on a path shows its path dashed.
#[test]
#[ignore]
fn visual_issue_149_blend_start_and_end() {
    use crate::tools::ToolId;
    let mut h = doc_harness();
    h.state_mut().set_tool(ToolId::Pick);
    h.state_mut().doc_mut().unwrap().doc.settings.snap_off = true;
    let [blend, start, _] = si_blend(&mut h);
    save(&mut h, "issue_149_01_blend_selected_shows_grips");
    let q = scr(&h, (200.0, 400.0));
    click(&mut h, q);
    assert_eq!(selection(&h), vec![start]);
    save(&mut h, "issue_149_02_start_selected");
    // a drag in progress: the steps follow the start
    let (a, b) = (scr(&h, (200.0, 400.0)), scr(&h, (240.0, 280.0)));
    move_to(&mut h, a);
    h.step();
    press(&mut h, a, true);
    h.step();
    for k in 1..=6 {
        move_to(&mut h, a + (b - a) * (k as f32 / 6.0));
        h.step();
    }
    save(&mut h, "issue_149_03_dragging_the_start");
    press(&mut h, b, false);
    h.step();
    assert!((bp_blend(&h, blend).start.bounds().center() - Point::new(240.0, 280.0)).hypot() < 0.5);
    save(&mut h, "issue_149_04_start_dropped");
    // a blend on a path: its path dashed, grips on its start and end
    h.state_mut().doc_mut().unwrap().doc.pages[0].layers.last_mut().unwrap().objects.clear();
    let [blend2, ..] = bp_on_path(&mut h);
    h.state_mut().doc_mut().unwrap().selection = vec![blend2];
    save(&mut h, "issue_149_05_blend_on_a_path_selected");
}

// ---- #150: the Text Position options of dimensions ----

/// Text Above, Through and Below Dimension Line, Center Text Between Extension Lines, Force Text
/// Horizontal and Draw Box Around Text (the Text Position flyout of the dimension bars).
const DT_KEYS: [&str; 6] = ["5dad5787", "fe24f8fd", "3b48e7f1", "a57e1ff4", "f101e675", "5b07d790"];

fn dt_cmd(k: &str) -> Cmd {
    Cmd::BarOption(u32::from_str_radix(k, 16).unwrap())
}

fn dt_state(h: &Harness<'_, AnyVectorApp>, k: &str) -> (bool, bool) {
    crate::propbar::option_state(h.state(), k).unwrap()
}

/// Add a dimension (black, 12 pt text) and return its id.
fn dt_add(h: &mut Harness<'_, AnyVectorApp>, kind: vector_core::object::DimensionKind, pts: &[(f64, f64)], f: impl FnOnce(&mut vector_core::object::DimensionObj)) -> u64 {
    let d = h.state_mut().doc_mut().unwrap();
    let mut dim = vector_core::object::DimensionObj { kind, points: pts.iter().map(|p| Point::new(p.0, p.1)).collect(), font_size: 12.0, unit: vector_core::units::Unit::Millimeters, precision: 1, ..Default::default() };
    f(&mut dim);
    let mut st = Style::default();
    st.fill = Fill::Solid(Color::BLACK);
    st.outline.color = Some(Color::BLACK);
    let o = Object::new(d.doc.new_id(), ObjKind::Dimension(dim), Affine::IDENTITY, st);
    let id = o.id;
    d.add_objects("test", vec![o]);
    id
}

fn dt_dim(h: &Harness<'_, AnyVectorApp>, id: u64) -> vector_core::object::DimensionObj {
    match &h.state().doc().unwrap().doc.find(id).unwrap().kind {
        ObjKind::Dimension(d) => d.clone(),
        _ => panic!("not a dimension"),
    }
}

/// (placement, centred, horizontal, boxed)
fn dt_opts(d: &vector_core::object::DimensionObj) -> (vector_core::object::TextPlacement, bool, bool, bool) {
    (d.placement, d.text_centered, d.horizontal_text, d.boxed_text)
}

/// #150 (§5.4): every button, check box and radio button of the dimension property bars (a
/// selected dimension, the dimension tools) and of their Text Position flyout runs a command;
/// none says "not available in this version yet". The six Text Position items are the dimension
/// properties, not commands of their own.
#[test]
fn dimension_bar_items_all_run_a_command() {
    use crate::ui::Kind;
    let dim_bars = ["47015b40", "b1ae77c4", "b477e7c5"];
    let mut seen = 0;
    let mut missing = vec![];
    for b in crate::ui::bars() {
        if !dim_bars.contains(&&b.id[..8.min(b.id.len())]) {
            continue;
        }
        seen += 1;
        for it in b.items {
            let runs = match it.kind {
                Kind::Button | Kind::Check | Kind::Radio | Kind::ButtonFlyout => it.id.is_empty() || crate::ui::bind::cmd(it.id).is_some(),
                // the Text Position flyout opens its bar
                Kind::Flyout => crate::ui::bar(it.sub).is_some_and(|s| s.id.starts_with("b477e7c5")),
                _ => true,
            };
            if !runs {
                missing.push(format!("{} ▸ {} ({})", b.name, crate::ui::plain(it.text), it.id));
            }
        }
    }
    assert_eq!(seen, dim_bars.len(), "a dimension bar is missing from the reference data");
    assert!(missing.is_empty(), "dimension items without a command: {missing:#?}");
    for k in DT_KEYS {
        let id = crate::ui::bars().iter().flat_map(|b| b.items.iter()).find(|it| it.id.starts_with(k)).unwrap().id;
        assert_eq!(crate::ui::bind::cmd(id), Some(dt_cmd(k)), "{k}");
    }
}

/// #150: each Text Position option changes the selected dimension (horizontal, vertical, slanted
/// and angular) in one undo step, the flyout shows it, and Undo puts it back.
#[test]
fn dimension_text_position_applies_to_the_selection_with_undo() {
    use crate::tools::ToolId;
    use vector_core::object::{DimensionKind as K, TextPlacement as P};
    let mut h = join_harness(ToolId::Pick);
    let dims = [
        ("horizontal", dt_add(&mut h, K::Parallel, &[(100.0, 200.0), (300.0, 200.0), (150.0, 160.0)], |_| {})),
        ("vertical", dt_add(&mut h, K::HorizontalVertical, &[(400.0, 100.0), (420.0, 300.0), (250.0, 280.0)], |_| {})),
        ("slanted", dt_add(&mut h, K::Parallel, &[(100.0, 400.0), (273.2, 500.0), (150.0, 380.0)], |_| {})),
        ("angular", dt_add(&mut h, K::Angular, &[(400.0, 400.0), (550.0, 400.0), (400.0, 550.0), (490.0, 430.0)], |_| {})),
    ];
    for (name, id) in dims {
        h.state_mut().doc_mut().unwrap().selection = vec![id];
        h.step();
        let angular = name == "angular";
        assert_eq!(dt_opts(&dt_dim(&h, id)), (P::Above, true, false, false), "{name}: the defaults");
        let checked = |h: &Harness<'_, AnyVectorApp>| DT_KEYS.map(|k| dt_state(h, k).1);
        assert_eq!(checked(&h), [true, false, false, true, false, false], "{name}: the flyout shows Above and centred");
        for k in DT_KEYS {
            let (on, was) = dt_state(&h, k);
            if angular && k == "f101e675" {
                // an angular value is always level
                assert!(!on, "{name}: Force Text Horizontal is off");
                let n = undo_steps(&h);
                run_cmd(&mut h, dt_cmd(k));
                assert_eq!(undo_steps(&h), n, "{name}: a disabled option changed nothing");
                continue;
            }
            assert!(on, "{name} {k}: enabled");
            let before = dt_dim(&h, id);
            let n = undo_steps(&h);
            run_cmd(&mut h, dt_cmd(k));
            let after = dt_dim(&h, id);
            match k {
                "5dad5787" => {
                    // already above: a radio chosen again changes nothing
                    assert_eq!(after, before);
                    assert_eq!(undo_steps(&h), n, "{name}: no empty undo step");
                    continue;
                }
                "fe24f8fd" => assert_eq!(after.placement, P::Through),
                "3b48e7f1" => assert_eq!(after.placement, P::Below),
                "a57e1ff4" => assert!(!after.text_centered),
                "f101e675" => assert!(after.horizontal_text),
                _ => assert!(after.boxed_text),
            }
            assert_eq!(undo_steps(&h), n + 1, "{name} {k}: one undo step");
            assert_eq!((after.points.clone(), after.text.clone()), (before.points.clone(), before.text.clone()), "{name} {k}: only the option changes");
            let (_, now) = dt_state(&h, k);
            // a radio is now the one chosen, a check box flips
            let radio = matches!(k, "fe24f8fd" | "3b48e7f1");
            assert!(if radio { now } else { now == !was }, "{name} {k}: the flyout shows it");
            if k == "fe24f8fd" {
                assert!(!dt_state(&h, "5dad5787").1, "{name}: one radio at a time");
            }
            // the value moves, or the lines change
            let path = |d: &vector_core::object::DimensionObj| (vector_core::object::dimension_text(d).transform, vector_core::object::dimension_path(d).elements().len());
            // (a level line keeps its value level anyway)
            if !(name == "horizontal" && k == "f101e675") {
                assert_ne!(path(&after), path(&before), "{name} {k}: the drawing changes");
            }
            undo(&mut h);
            assert_eq!(dt_dim(&h, id), before, "{name} {k}: undone");
            run_cmd(&mut h, Cmd::Redo);
            assert_eq!(dt_dim(&h, id), after, "{name} {k}: redone");
        }
        // Above again: the radio goes back
        run_cmd(&mut h, dt_cmd("5dad5787"));
        assert_eq!(dt_dim(&h, id).placement, P::Above);
        assert!(dt_state(&h, "5dad5787").1 && !dt_state(&h, "3b48e7f1").1);
    }
    // two dimensions selected: both change, in one step
    let (a, b) = (dims[0].1, dims[2].1);
    h.state_mut().doc_mut().unwrap().selection = vec![a, b];
    let n = undo_steps(&h);
    let boxed = dt_state(&h, "5b07d790").1;
    run_cmd(&mut h, dt_cmd("5b07d790"));
    assert_eq!((dt_dim(&h, a).boxed_text, dt_dim(&h, b).boxed_text), (!boxed, !boxed));
    assert_eq!(undo_steps(&h), n + 1);
    // the tool defaults were left alone
    let t = &h.state().settings.topts;
    assert_eq!((t.dim_placement, t.dim_text_centered, t.dim_horizontal_text, t.dim_boxed_text), (P::Above, true, false, false));
}

/// #150: with no dimension selected the options are those of the dimensions drawn next: the
/// flyout shows them, no undo step is made, and the next dimension drawn uses them.
#[test]
fn dimension_text_position_sets_the_tool_defaults() {
    use crate::tools::ToolId;
    use vector_core::object::TextPlacement as P;
    let mut h = join_harness(ToolId::ParallelDim);
    h.state_mut().doc_mut().unwrap().selection.clear();
    let n = undo_steps(&h);
    assert_eq!(DT_KEYS.map(|k| dt_state(&h, k)), [(true, true), (true, false), (true, false), (true, true), (true, false), (true, false)]);
    for k in ["3b48e7f1", "a57e1ff4", "f101e675", "5b07d790"] {
        run_cmd(&mut h, dt_cmd(k));
    }
    assert_eq!(undo_steps(&h), n, "tool options make no undo step");
    let t = &h.state().settings.topts;
    assert_eq!((t.dim_placement, t.dim_text_centered, t.dim_horizontal_text, t.dim_boxed_text), (P::Below, false, true, true));
    assert_eq!(DT_KEYS.map(|k| dt_state(&h, k).1), [false, false, true, false, true, true], "the flyout shows the tool options");
    // draw a dimension: drag its ends, click the line's place
    tool_stroke(&mut h, &[(100.0, 300.0), (300.0, 300.0)]);
    tool_click(&mut h, (150.0, 260.0), egui::Vec2::ZERO);
    let id = h.state().doc().unwrap().selection.first().copied().or_else(|| {
        h.state().doc().unwrap().page_ref().all_objects().find(|o| matches!(o.kind, ObjKind::Dimension(_))).map(|o| o.id)
    });
    let d = dt_dim(&h, id.expect("a dimension was drawn"));
    assert_eq!(dt_opts(&d), (P::Below, false, true, true));
    // the label point is where the line was clicked, so the value sits there, below the line
    let t = vector_core::object::dimension_text(&d);
    let r = kurbo::Shape::bounding_box(&t.ink_outline().unwrap());
    assert!((r.center().x - 150.0).abs() < 1.0 && r.y0 > 260.0, "{r:?}");
}

/// #150: callouts have no dimension line, so the options are off for them, and a selection of
/// callouts doesn't change the tool options either.
#[test]
fn dimension_text_position_off_for_callouts() {
    use crate::tools::ToolId;
    use vector_core::object::DimensionKind as K;
    let mut h = join_harness(ToolId::Pick);
    let callout = dt_add(&mut h, K::Callout, &[(100.0, 100.0), (150.0, 60.0), (200.0, 60.0)], |d| d.text = "Note".into());
    h.state_mut().doc_mut().unwrap().selection = vec![callout];
    for k in DT_KEYS {
        assert!(!dt_state(&h, k).0, "{k} on a callout");
        assert_eq!(crate::ui::bind::enabled(h.state(), &format!("{k}-0000")), Some(false), "{k}: the flyout row is off");
    }
    let before = (dt_dim(&h, callout), undo_steps(&h), h.state().settings.topts.dim_boxed_text);
    run_cmd(&mut h, dt_cmd("5b07d790"));
    assert_eq!((dt_dim(&h, callout), undo_steps(&h), h.state().settings.topts.dim_boxed_text), before);
    // an angular dimension: everything but Force Text Horizontal
    let angular = dt_add(&mut h, K::Angular, &[(400.0, 400.0), (550.0, 400.0), (400.0, 550.0), (480.0, 480.0)], |_| {});
    h.state_mut().doc_mut().unwrap().selection = vec![angular];
    assert_eq!(DT_KEYS.map(|k| dt_state(&h, k).0), [true, true, true, true, false, true]);
}

/// #150: the six options on horizontal, vertical, slanted and angular dimensions, drawn
/// (`target/visual/issue_150_*.png`), and the same through the export flattener.
#[test]
#[ignore]
fn visual_dimension_text_positions() {
    use vector_core::object::{DimensionKind as K, TextPlacement as P};
    let mut h = harness();
    h.state_mut().new_default_document();
    h.state_mut().dialogs.new_doc = None;
    // columns: the defaults (Above), Through, Below, not centred, forced horizontal, boxed, boxed through
    let cols: [(&str, fn(&mut vector_core::object::DimensionObj)); 7] = [
        ("above", |_| {}),
        ("through", |d| d.placement = P::Through),
        ("below", |d| d.placement = P::Below),
        ("not-centred", |d| d.text_centered = false),
        ("horizontal", |d| d.horizontal_text = true),
        ("boxed", |d| d.boxed_text = true),
        ("boxed-through", |d| {
            d.boxed_text = true;
            d.placement = P::Through;
        }),
    ];
    let mut ids = vec![];
    for (c, (_, f)) in cols.iter().enumerate() {
        let x = 20.0 + c as f64 * 84.0;
        // horizontal; the label point (for "not centred") near the right end
        ids.push(dt_add(&mut h, K::Parallel, &[(x, 120.0), (x + 70.0, 120.0), (x + 58.0, 90.0)], |d| f(d)));
        // vertical
        ids.push(dt_add(&mut h, K::HorizontalVertical, &[(x + 45.0, 170.0), (x + 50.0, 300.0), (x + 20.0, 280.0)], |d| f(d)));
        // slanted (30°)
        ids.push(dt_add(&mut h, K::Parallel, &[(x, 420.0), (x + 69.3, 380.0), (x + 50.0, 360.0)], |d| f(d)));
        // angular (60°), the label point near the second ray
        ids.push(dt_add(&mut h, K::Angular, &[(x, 560.0), (x + 75.0, 560.0), (x + 37.5, 495.0), (x + 35.0, 520.0)], |d| f(d)));
    }
    h.state_mut().doc_mut().unwrap().selection.clear();
    run_cmd(&mut h, Cmd::ZoomToAll);
    save(&mut h, "issue_150_01_text_positions");
    // the same through the export flattener (PDF, SVG, EMF and CMX draw from it)
    let doc = h.state().doc().unwrap().doc.clone();
    let svg = vector_io::svg_export::page_to_svg(&doc, 0, &Default::default());
    std::fs::write(out_dir().join("issue_150_02_text_positions.svg"), &svg).unwrap();
    vector_io::pdf::export(&doc, &out_dir().join("issue_150_02_text_positions.pdf"), &vector_io::pdf::PdfOptions::default()).unwrap();
    // the dimension's bar with a dimension selected (its bounds hold the value)
    h.state_mut().doc_mut().unwrap().selection = vec![ids[4]];
    save(&mut h, "issue_150_03_selected");
    // the Parallel Dimension tool previews the options it will use: boxed, through the line
    h.state_mut().doc_mut().unwrap().selection.clear();
    h.state_mut().set_tool(crate::tools::ToolId::ParallelDim);
    h.state_mut().doc_mut().unwrap().doc.settings.snap_off = true;
    run_cmd(&mut h, dt_cmd("fe24f8fd"));
    run_cmd(&mut h, dt_cmd("5b07d790"));
    let (p0, p1, p2) = (scr(&h, (100.0, 700.0)), scr(&h, (400.0, 700.0)), scr(&h, (250.0, 660.0)));
    tool_input(&mut h, p0, true, true, false);
    tool_input(&mut h, p1, false, true, false);
    tool_input(&mut h, p1, false, false, true);
    tool_input(&mut h, p2, false, false, false);
    save(&mut h, "issue_150_04_tool_preview");
    for (i, (name, _)) in cols.iter().enumerate() {
        let d = dt_dim(&h, ids[i * 4]);
        let b = h.state().doc().unwrap().doc.bounds_of(h.state().doc().unwrap().doc.find(ids[i * 4]).unwrap());
        let t = kurbo::Shape::bounding_box(&vector_core::object::dimension_text(&d).ink_outline().unwrap());
        assert!(b.union(t) == b, "{name}: the bounds hold the value ({b:?}, {t:?})");
    }
}
