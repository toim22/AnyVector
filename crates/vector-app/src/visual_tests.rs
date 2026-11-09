//! Headless renders of the main window for visual comparison with the design screenshots.
//! Run with `cargo test -p vector-app visual -- --ignored`; images go to `target/visual/`.

use crate::app::VectorApp;
use crate::commands::Cmd;
use egui_kittest::Harness;
use kurbo::{Affine, Point};
use vector_core::object::{EllipseKind, EllipseShape, PolygonKind, PolygonShape, RectShape, CornerType};
use vector_core::{Color, Fill, ObjKind, Object, Style};

fn out_dir() -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/visual");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn harness<'a>() -> Harness<'a, VectorApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1920.0, 1051.0)).wgpu().build_eframe(|cc| {
        let mut app = VectorApp::new(cc);
        app.splash = None;
        app.pending.clear();
        app
    });
    for _ in 0..3 {
        h.step();
    }
    h
}

fn save(h: &mut Harness<'_, VectorApp>, name: &str) {
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
        add(ObjKind::Polygon(PolygonShape { w: 150.0, h: 150.0, sides: 5, kind: PolygonKind::Star, sharpness: 53.0 }), (200.0, 300.0), Color::rgb(0xff, 0xd0, 0x20));
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

fn press(h: &mut Harness<'_, VectorApp>, p: egui::Pos2, down: bool) {
    h.input_mut().events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: down, modifiers: egui::Modifiers::NONE });
}

fn move_to(h: &mut Harness<'_, VectorApp>, p: egui::Pos2) {
    h.input_mut().events.push(egui::Event::PointerMoved(p));
}

fn drag(h: &mut Harness<'_, VectorApp>, a: egui::Pos2, b: egui::Pos2) {
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

fn click(h: &mut Harness<'_, VectorApp>, p: egui::Pos2) {
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
    let mut h = Harness::builder().with_size(egui::vec2(620.0, 380.0)).wgpu().build_eframe(|cc| VectorApp::new(cc));
    // the splash shows for 1.8 s of app time; render it right after it has faded in
    for _ in 0..2 {
        h.step();
    }
    let img = h.render().expect("render");
    img.save(out_dir().join("00_splash.png")).unwrap();
}

/// Open a menu of the menu bar. Waits first, so the menu closed before has finished fading out.
fn open_menu(h: &mut Harness<'_, VectorApp>, label: &str) {
    use egui_kittest::kittest::Queryable;
    for _ in 0..30 {
        h.step();
    }
    h.get_by_label(label).click_accesskit();
}

fn close_menu(h: &mut Harness<'_, VectorApp>) {
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

fn right_click(h: &mut Harness<'_, VectorApp>, p: egui::Pos2) {
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
        let poly = add(d, ObjKind::Polygon(PolygonShape { w: 80.0, h: 80.0, sides: 5, kind: PolygonKind::Polygon, sharpness: 0.0 }), 330.0, 40.0);
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
    let to_screen = |h: &Harness<'_, VectorApp>, x: f64, y: f64| {
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
    let mut svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}"><rect width="{w}" height="{h}" fill="#262626"/>"##);
    for (i, n) in names.iter().enumerate() {
        let (x, y) = ((i % cols) as u32 * cw, (i / cols) as u32 * ch);
        let body = crate::icons::body(n).unwrap_or_default();
        svg += &format!(
            r##"<g transform="translate({} {}) scale(2)" fill="none" stroke="#c4c4c4" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">{body}</g><text x="{}" y="{}" font-family="Segoe UI" font-size="12" fill="#c4c4c4">{n}</text>"##,
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
    let click = |h: &mut Harness<'_, VectorApp>, p: egui::Pos2| {
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
        add(d, "curva", ObjKind::Polygon(PolygonShape { w: 80.0, h: 80.0, sides: 5, kind: PolygonKind::Polygon, sharpness: 0.0 }), 420.0, 40.0);
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
    let shot = |h: &mut Harness<'_, VectorApp>, name: &str| {
        for _ in 0..4 {
            h.step();
        }
        let img = h.render().expect("render");
        let bar = image::imageops::crop_imm(&img, 0, y0, img.width(), 44).to_image();
        bar.save(dir.join(format!("{name}.png"))).unwrap();
    };
    let select = |h: &mut Harness<'_, VectorApp>, sel: Vec<vector_core::ObjId>| {
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
    let only: Option<String> = std::env::var("VECTOR_DOCKER").ok();
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
        let states: &[fn(&mut VectorApp)] = match d {
            DockerId::Transform => &[|a| a.dockers.ui.transform_tab = 1, |a| a.dockers.ui.transform_tab = 2, |a| a.dockers.ui.transform_tab = 3, |a| a.dockers.ui.transform_tab = 4],
            DockerId::Coordinates => &[|a| a.dockers.ui.co.tab = 1, |a| a.dockers.ui.co.tab = 2, |a| a.dockers.ui.co.tab = 3, |a| a.dockers.ui.co.tab = 4, |a| a.dockers.ui.co.tab = 5, |a| a.dockers.ui.co.tab = 6],
            DockerId::LiveGuides => &[|a| a.dockers.ui.lg_margins_open = true, |a| a.dockers.ui.lg_open = [false, true]],
            DockerId::Guidelines => &[|a| a.dockers.ui.guide_type = 2],
            DockerId::FitObjectsToPath => &[|a| a.dockers.ui.fit.style = 2, |a| a.dockers.ui.fit.style = 4],
            DockerId::StepRepeat => &[|a| a.dockers.ui.step_mode = [2, 2], |a| a.dockers.ui.step_open = [false, true], |a| a.dockers.ui.step_open = [true, false]],
            DockerId::Shape => &[|a| a.ts.shaping_op = 1, |a| a.ts.shaping_op = 2, |a| a.ts.shaping_op = 3, |a| a.ts.shaping_op = 4, |a| a.ts.shaping_op = 5, |a| a.ts.shaping_op = 6],
            DockerId::Corners => &[|a| a.dockers.ui.corner_kind = 1, |a| a.dockers.ui.corner_kind = 2],
            DockerId::JoinCurves => &[|a| a.dockers.ui.join_mode = 1, |a| a.dockers.ui.join_mode = 2, |a| a.dockers.ui.join_mode = 3],
            DockerId::AlignDistribute => &[|a| a.dockers.ui.align_to = 1, |a| a.dockers.ui.align_to = 2, |a| a.dockers.ui.align_to = 3, |a| a.dockers.ui.align_to = 4, |a| a.dockers.ui.dist_to = 1, |a| a.dockers.ui.dist_to = 2],
            _ => &[],
        };
        for (k, f) in states.iter().enumerate() {
            f(h.state_mut());
            save(&mut h, &format!("dk_{:03}_{}", i + 1, k + 1));
        }
    }
}
