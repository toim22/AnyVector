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
    assert!(h.query_all_by_label("Install Now").next().is_some(), "Install Now is missing when a version is available");
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
    let inp = crate::tools::Input { pos, hovered: true, pressed, released, down, double: false, secondary: false, mods: egui::Modifiers::NONE, key_c: false, time: 100.0, dt: 0.016 };
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
    let inp = crate::tools::Input { pos, hovered: true, pressed, released, down, double, secondary: false, mods, key_c: false, time: 100.0, dt: 0.016 };
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
