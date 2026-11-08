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
