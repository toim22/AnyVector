//! Palettes docker (the palette manager): the Document and Color Styles palettes, My Palettes (the
//! user's palette files) and the Palette Libraries, each with a check box that opens or closes it
//! as a palette on the right edge.

use super::{drop_mark, icon_button, icon_button_resp};
use crate::app::AnyVectorApp;
use crate::commands::Cmd;
use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Sense, Stroke, Ui};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use vector_core::color::{NamedColor, Palette};
use vector_core::Color;

#[derive(Clone)]
pub struct PmUi {
    /// Expanded folders, by row key.
    pub open: HashSet<String>,
    /// The selected row.
    pub sel: Option<String>,
    /// Cut or copied palette (file path or library key), and whether it was cut.
    pub clip: Option<(String, bool)>,
}

impl Default for PmUi {
    fn default() -> Self {
        PmUi { open: ["lib".to_string()].into_iter().collect(), sel: None, clip: None }
    }
}

/// A row of the palette tree.
enum Node {
    /// A palette the library generates.
    Lib(&'static str, &'static str),
    /// A library palette AnyVector does not include (licensed color books, themed collections).
    Missing(&'static str),
    /// A library folder.
    Folder(&'static str, &'static str, Vec<Node>),
    /// A folder AnyVector does not include.
    MissingFolder(&'static str),
}

fn library() -> Vec<Node> {
    use Node::*;
    vec![
        Folder(
            "lib/process",
            "Process",
            vec![
                Folder("lib/process/cmyk", "CMYK", vec![MissingFolder("Misc"), MissingFolder("Nature"), MissingFolder("People"), MissingFolder("Things")]),
                MissingFolder("PANTONE®"),
                Folder("lib/process/rgb", "RGB", vec![MissingFolder("256 Colors"), MissingFolder("Misc"), MissingFolder("Nature"), MissingFolder("People"), MissingFolder("Things"), Lib("websafe", "Web Safe Colors")]),
                Lib("lab", "Lab Colors"),
                Lib("svg", "SVG Colors"),
                Missing("TRUMATCH Colors"),
                Lib("uniform", "Uniform Colors"),
                Missing("Universal Laser Systems"),
            ],
        ),
        Folder(
            "lib/spot",
            "Spot",
            vec![
                MissingFolder("HKS"),
                MissingFolder("ONYX Graphics"),
                MissingFolder("PANTONE®"),
                MissingFolder("Roland"),
                Missing("DIC Colors"),
                Missing("FOCOLTONE Colors"),
                Missing("SpectraMaster® Colors"),
                Missing("TOYO COLOR FINDER"),
            ],
        ),
        Lib("gray256", "256 shades of Gray"),
        Lib("cmyk", "CMYK Palette"),
        Lib("default", "Default Palette"),
        Lib("percent", "Percent Gray"),
        Lib("rgb", "RGB Palette"),
    ]
}

/// The palettes the library generates, by key.
pub fn library_palette(key: &str) -> Option<Palette> {
    let named = |name: &str, colors: Vec<NamedColor>| Some(Palette { name: name.into(), colors });
    match key {
        "default" => Some(Palette::default_palette()),
        "websafe" => {
            let mut p = Palette::rgb_palette();
            p.name = "Web Safe Colors".into();
            Some(p)
        }
        "rgb" => {
            let mut p = Palette::rgb_palette();
            p.name = "RGB Palette".into();
            Some(p)
        }
        "gray256" => named("256 shades of Gray", (0..=255u8).rev().map(|v| NamedColor::new(format!("R{v} G{v} B{v}"), Color::rgb(v, v, v))).collect()),
        "percent" => named("Percent Gray", (0..=100).map(|k| NamedColor::new(format!("{k}% Black"), Color::from_cmyk(0.0, 0.0, 0.0, k as f64))).collect()),
        "cmyk" => {
            let mut v = vec![];
            for k in (0..=100).step_by(10) {
                v.push(NamedColor::new(format!("C0 M0 Y0 K{k}"), Color::from_cmyk(0.0, 0.0, 0.0, k as f64)));
            }
            let mixes: [(f64, f64, f64); 6] = [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), (0.0, 1.0, 1.0), (1.0, 0.0, 1.0), (1.0, 1.0, 0.0)];
            for (c, m, y) in mixes {
                for t in (20..=100).step_by(20) {
                    let t = t as f64;
                    let (c, m, y) = (c * t, m * t, y * t);
                    v.push(NamedColor::new(format!("C{c:.0} M{m:.0} Y{y:.0} K0"), Color::from_cmyk(c, m, y, 0.0)));
                }
            }
            named("CMYK Palette", v)
        }
        "lab" => {
            let mut v = vec![];
            for l in [25.0, 45.0, 65.0, 85.0] {
                for a in [-60.0, -30.0, 0.0, 30.0, 60.0] {
                    for b in [-60.0, -30.0, 0.0, 30.0, 60.0] {
                        v.push(NamedColor::new(format!("L{l:.0} a{a:.0} b{b:.0}"), lab(l, a, b)));
                    }
                }
            }
            named("Lab Colors", v)
        }
        "uniform" => {
            let mut v = vec![];
            for h in (0..360).step_by(15) {
                for (s, b) in [(35.0, 100.0), (70.0, 100.0), (100.0, 100.0), (100.0, 70.0), (100.0, 45.0)] {
                    v.push(NamedColor::new(format!("H{h} S{s:.0} B{b:.0}"), Color::from_hsb(h as f64, s, b)));
                }
            }
            named("Uniform Colors", v)
        }
        "svg" => named(
            "SVG Colors",
            SVG_COLORS
                .split(' ')
                .filter_map(|e| {
                    let (n, h) = e.split_once(':')?;
                    Some(NamedColor::new(n, Color::from_hex(&format!("#{h}"))?))
                })
                .collect(),
        ),
        _ => None,
    }
}

/// CIE L*a*b* (D65) to sRGB.
fn lab(l: f64, a: f64, b: f64) -> Color {
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let f = |t: f64| if t > 6.0 / 29.0 { t * t * t } else { 3.0 * (6.0f64 / 29.0).powi(2) * (t - 4.0 / 29.0) };
    let (x, y, z) = (0.95047 * f(fx), f(fy), 1.08883 * f(fz));
    let r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
    let g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
    let bl = 0.0557 * x - 0.2040 * y + 1.0570 * z;
    let gam = |c: f64| {
        let c = if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
        (c.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    Color::rgb(gam(r), gam(g), gam(bl))
}

/// The SVG 1.1 color keywords.
const SVG_COLORS: &str = "aliceblue:f0f8ff antiquewhite:faebd7 aqua:00ffff aquamarine:7fffd4 azure:f0ffff beige:f5f5dc bisque:ffe4c4 black:000000 blanchedalmond:ffebcd blue:0000ff blueviolet:8a2be2 brown:a52a2a burlywood:deb887 cadetblue:5f9ea0 chartreuse:7fff00 chocolate:d2691e coral:ff7f50 cornflowerblue:6495ed cornsilk:fff8dc crimson:dc143c cyan:00ffff darkblue:00008b darkcyan:008b8b darkgoldenrod:b8860b darkgray:a9a9a9 darkgreen:006400 darkgrey:a9a9a9 darkkhaki:bdb76b darkmagenta:8b008b darkolivegreen:556b2f darkorange:ff8c00 darkorchid:9932cc darkred:8b0000 darksalmon:e9967a darkseagreen:8fbc8f darkslateblue:483d8b darkslategray:2f4f4f darkslategrey:2f4f4f darkturquoise:00ced1 darkviolet:9400d3 deeppink:ff1493 deepskyblue:00bfff dimgray:696969 dimgrey:696969 dodgerblue:1e90ff firebrick:b22222 floralwhite:fffaf0 forestgreen:228b22 fuchsia:ff00ff gainsboro:dcdcdc ghostwhite:f8f8ff gold:ffd700 goldenrod:daa520 gray:808080 grey:808080 green:008000 greenyellow:adff2f honeydew:f0fff0 hotpink:ff69b4 indianred:cd5c5c indigo:4b0082 ivory:fffff0 khaki:f0e68c lavender:e6e6fa lavenderblush:fff0f5 lawngreen:7cfc00 lemonchiffon:fffacd lightblue:add8e6 lightcoral:f08080 lightcyan:e0ffff lightgoldenrodyellow:fafad2 lightgray:d3d3d3 lightgreen:90ee90 lightgrey:d3d3d3 lightpink:ffb6c1 lightsalmon:ffa07a lightseagreen:20b2aa lightskyblue:87cefa lightslategray:778899 lightslategrey:778899 lightsteelblue:b0c4de lightyellow:ffffe0 lime:00ff00 limegreen:32cd32 linen:faf0e6 magenta:ff00ff maroon:800000 mediumaquamarine:66cdaa mediumblue:0000cd mediumorchid:ba55d3 mediumpurple:9370db mediumseagreen:3cb371 mediumslateblue:7b68ee mediumspringgreen:00fa9a mediumturquoise:48d1cc mediumvioletred:c71585 midnightblue:191970 mintcream:f5fffa mistyrose:ffe4e1 moccasin:ffe4b5 navajowhite:ffdead navy:000080 oldlace:fdf5e6 olive:808000 olivedrab:6b8e23 orange:ffa500 orangered:ff4500 orchid:da70d6 palegoldenrod:eee8aa palegreen:98fb98 paleturquoise:afeeee palevioletred:db7093 papayawhip:ffefd5 peachpuff:ffdab9 peru:cd853f pink:ffc0cb plum:dda0dd powderblue:b0e0e6 purple:800080 red:ff0000 rosybrown:bc8f8f royalblue:4169e1 saddlebrown:8b4513 salmon:fa8072 sandybrown:f4a460 seagreen:2e8b57 seashell:fff5ee sienna:a0522d silver:c0c0c0 skyblue:87ceeb slateblue:6a5acd slategray:708090 slategrey:708090 snow:fffafa springgreen:00ff7f steelblue:4682b4 tan:d2b48c teal:008080 thistle:d8bfd8 tomato:ff6347 turquoise:40e0d0 violet:ee82ee wheat:f5deb3 white:ffffff whitesmoke:f5f5f5 yellow:ffff00 yellowgreen:9acd32";

/// My Palettes: the folder of the user's palette files.
pub fn my_palettes_dir() -> PathBuf {
    let d = crate::settings::config_dir().join("Palettes");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn read_palette(p: &Path) -> Option<Palette> {
    let t = std::fs::read_to_string(p).ok()?;
    let name = p.file_stem()?.to_string_lossy().to_string();
    Some(Palette::parse(&name, &t))
}

/// Whether a palette is showing on the right edge (the first one only while the default palette
/// column is on).
fn is_open(app: &AnyVectorApp, name: &str) -> bool {
    app.palettes.palettes.iter().position(|p| p.name == name).is_some_and(|i| i > 0 || app.settings.default_palette_visible)
}

fn set_open(app: &mut AnyVectorApp, p: Option<Palette>, name: &str, on: bool, path: Option<&Path>) {
    let at = app.palettes.palettes.iter().position(|x| x.name == name);
    match (on, at) {
        (true, Some(0)) => app.settings.default_palette_visible = true,
        (true, Some(_)) => {}
        (true, None) => {
            if let Some(p) = p {
                if app.palettes.palettes.is_empty() {
                    app.settings.default_palette_visible = true;
                }
                app.palettes.palettes.push(p);
            }
        }
        (false, Some(0)) => app.settings.default_palette_visible = false,
        (false, Some(i)) => {
            app.palettes.palettes.remove(i);
        }
        (false, None) => {}
    }
    if let Some(path) = path {
        let s = path.to_string_lossy().to_string();
        app.settings.open_palettes.retain(|x| *x != s);
        if on {
            app.settings.open_palettes.push(s);
        }
    }
}

/// What a row of the tree does when its check box or its row is used.
enum Act {
    Toggle(bool),
    Select(String),
    Expand(String),
}

/// Paints one tree row: chevron (folders), check box (`Some(Some(on))`, `Some(None)` for some),
/// and the label. Gives the clicked part.
#[allow(clippy::too_many_arguments)]
fn row(ui: &mut Ui, key: &str, depth: usize, folder: Option<bool>, check: Option<Option<bool>>, label: &str, enabled: bool, selected: bool) -> Option<Act> {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(r, 0.0, theme::ACCENT);
    } else if resp.hovered() && enabled {
        p.rect_filled(r, 0.0, theme::HOVER);
    }
    let x = r.left() + 7.0 + depth as f32 * 19.0;
    let mut act = None;
    let chev = Rect::from_center_size(pos2(x + 6.0, r.center().y), vec2(14.0, 23.0));
    if let Some(open) = folder {
        let c = if enabled { theme::TEXT_DIM } else { theme::DISABLED };
        if open {
            crate::widgets::chevron(p, chev.center(), 3.2, c);
        } else {
            crate::widgets::chevron_right(p, chev.center(), 3.2, c);
        }
    }
    let cb = Rect::from_center_size(pos2(x + 21.0, r.center().y), vec2(13.0, 13.0));
    if let Some(state) = check {
        p.rect_filled(cb, 2.0, if enabled { Color32::from_gray(64) } else { Color32::from_gray(48) });
        p.rect_stroke(cb, 2.0, Stroke::new(1.0, Color32::from_gray(if enabled { 96 } else { 70 })), egui::StrokeKind::Inside);
        let ink = if enabled { theme::TEXT } else { theme::DISABLED };
        match state {
            Some(true) => {
                p.line_segment([pos2(cb.left() + 3.0, cb.center().y), pos2(cb.left() + 5.5, cb.bottom() - 3.5)], Stroke::new(1.5, ink));
                p.line_segment([pos2(cb.left() + 5.5, cb.bottom() - 3.5), pos2(cb.right() - 2.5, cb.top() + 3.0)], Stroke::new(1.5, ink));
            }
            None => {
                p.line_segment([pos2(cb.left() + 3.0, cb.center().y), pos2(cb.right() - 3.0, cb.center().y)], Stroke::new(1.5, ink));
            }
            Some(false) => {}
        }
    }
    let tx = if check.is_some() { cb.right() + 6.0 } else { x + 14.0 };
    p.text(pos2(tx, r.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), if enabled { theme::TEXT } else { theme::DISABLED });
    let resp = if enabled { resp } else { resp.on_hover_text("Not included in AnyVector") };
    if !enabled {
        return None;
    }
    if resp.clicked() {
        let at = resp.interact_pointer_pos().unwrap_or_default();
        if folder.is_some() && chev.expand2(vec2(2.0, 0.0)).contains(at) {
            act = Some(Act::Expand(key.to_string()));
        } else if check.is_some() && cb.expand(3.0).contains(at) {
            act = Some(Act::Toggle(state_on(check)));
        } else {
            act = Some(Act::Select(key.to_string()));
        }
    }
    if resp.double_clicked() && folder.is_some() {
        act = Some(Act::Expand(key.to_string()));
    }
    act
}

fn state_on(check: Option<Option<bool>>) -> bool {
    !matches!(check, Some(Some(true)))
}

/// Library palettes under a node (for a folder's check box).
fn lib_keys(n: &Node, out: &mut Vec<(&'static str, &'static str)>) {
    match n {
        Node::Lib(k, name) => out.push((k, name)),
        Node::Folder(_, _, kids) => kids.iter().for_each(|k| lib_keys(k, out)),
        _ => {}
    }
}

fn tri(app: &AnyVectorApp, names: &[&str]) -> Option<Option<bool>> {
    let n = names.iter().filter(|x| is_open(app, x)).count();
    Some(if n == 0 {
        Some(false)
    } else if n == names.len() {
        Some(true)
    } else {
        None
    })
}

/// The user's palette files and folders, sorted folders first.
fn user_entries(dir: &Path) -> Vec<(PathBuf, bool)> {
    let mut v: Vec<(PathBuf, bool)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    let is_dir = p.is_dir();
                    let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
                    (is_dir || ["xml", "vpal", "gpl", "txt"].contains(&ext.as_str())).then_some((p, is_dir))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.file_name().cmp(&b.0.file_name())));
    v
}

pub fn palettes(app: &mut AnyVectorApp, ui: &mut Ui) {
    let sel = app.dockers.ui.pm.sel.clone();
    let user_sel = sel.as_deref().and_then(|s| s.strip_prefix("my:")).map(PathBuf::from).filter(|p| *p != my_palettes_dir());
    let palette_sel = sel.as_deref().is_some_and(|s| s.starts_with("lib:") || s.starts_with("my:") && !Path::new(&s[3..]).is_dir() || s == "doc" || s == "styles");
    let mut cmd: Option<&str> = None;
    // the gear menu
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let gear = icon_button_resp(app, ui, "gear", "Options", true);
            drop_mark(ui, gear.rect);
            egui::Popup::menu(&gear).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                crate::menus::menu_style(ui);
                ui.set_min_width(150.0);
                let items: [(&str, &str, bool); 8] = [
                    ("New Folder", "folder", true),
                    ("New Blank Palette", "blank", true),
                    ("Open Palette...", "open", true),
                    ("Edit Palette...", "edit", palette_sel),
                    ("Cut", "cut", user_sel.is_some()),
                    ("Copy", "copy", palette_sel),
                    ("Paste", "paste", app.dockers.ui.pm.clip.is_some()),
                    ("Delete", "delete", user_sel.is_some()),
                ];
                for (i, (t, k, on)) in items.into_iter().enumerate() {
                    if [1, 4, 7].contains(&i) {
                        ui.separator();
                    }
                    if ui.add_enabled(on, egui::Button::new(t)).clicked() {
                        cmd = Some(k);
                        ui.close();
                    }
                }
            });
        });
    });
    // the tree
    let h = ui.available_height() - 30.0;
    egui::Frame::new().stroke(Stroke::new(1.0, theme::SEP)).inner_margin(egui::Margin::same(1)).show(ui, |ui| {
        ui.set_min_height(h.max(60.0));
        egui::ScrollArea::vertical().id_salt("pm_tree").max_height(h.max(60.0)).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(3.0);
            let mut acts: Vec<(Act, Option<Palette>, String, Option<PathBuf>)> = vec![];
            // Document and Color Styles palettes
            let doc_on = app.settings.doc_palette_visible;
            if let Some(a) = row(ui, "doc", 0, None, Some(Some(doc_on)), "Document Palette", true, sel.as_deref() == Some("doc")) {
                acts.push((a, None, "doc".into(), None));
            }
            let st_on = app.settings.color_styles_palette_visible;
            if let Some(a) = row(ui, "styles", 0, None, Some(Some(st_on)), "Color Styles Palette", true, sel.as_deref() == Some("styles")) {
                acts.push((a, None, "styles".into(), None));
            }
            // My Palettes
            let my = my_palettes_dir();
            user_tree(app, ui, &my, "My Palettes", 0, &sel, &mut acts);
            // Palette Libraries
            let lib = library();
            let mut all = vec![];
            lib.iter().for_each(|n| lib_keys(n, &mut all));
            let names: Vec<&str> = all.iter().map(|x| x.1).collect();
            let open = app.dockers.ui.pm.open.contains("lib");
            if let Some(a) = row(ui, "lib", 0, Some(open), tri(app, &names), "Palette Libraries", true, sel.as_deref() == Some("lib")) {
                acts.push((a, None, "lib".into(), None));
            }
            if open {
                for n in &lib {
                    lib_tree(app, ui, n, 1, &sel, &mut acts);
                }
            }
            ui.add_space(3.0);
            for (a, pal, key, path) in acts {
                match a {
                    Act::Expand(k) => {
                        if !app.dockers.ui.pm.open.remove(&k) {
                            app.dockers.ui.pm.open.insert(k);
                        }
                    }
                    Act::Select(k) => app.dockers.ui.pm.sel = Some(k),
                    Act::Toggle(on) => match key.as_str() {
                        "doc" => app.settings.doc_palette_visible = on,
                        "styles" => app.settings.color_styles_palette_visible = on,
                        "lib" => {
                            for (k, name) in &all {
                                set_open(app, library_palette(k), name, on, None);
                            }
                        }
                        k if k.starts_with("libdir:") => {
                            let mut keys = vec![];
                            if let Some(n) = find_folder(&lib, &k[7..]) {
                                lib_keys(n, &mut keys);
                            }
                            for (k, name) in keys {
                                set_open(app, library_palette(k), name, on, None);
                            }
                        }
                        k if k.starts_with("mydir:") => {
                            for (p, dir) in user_entries(Path::new(&k[6..])) {
                                if !dir {
                                    if let Some(pal) = read_palette(&p) {
                                        let name = pal.name.clone();
                                        set_open(app, Some(pal), &name, on, Some(&p));
                                    }
                                }
                            }
                        }
                        _ => {
                            if let Some(pal) = pal {
                                let name = pal.name.clone();
                                set_open(app, Some(pal), &name, on, path.as_deref());
                            }
                        }
                    },
                }
            }
        });
    });
    // the bottom bar
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let add = icon_button_resp(app, ui, "add_circle", "New palette", true);
        drop_mark(ui, add.rect);
        egui::Popup::menu(&add).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            crate::menus::menu_style(ui);
            ui.set_min_width(200.0);
            if ui.button("New empty Color Palette").clicked() {
                cmd = Some("blank");
                ui.close();
            }
            if ui.add_enabled(app.has_selection(), egui::Button::new("New Palette from Selected objects")).clicked() {
                cmd = Some("from_sel");
                ui.close();
            }
            if ui.add_enabled(app.has_doc(), egui::Button::new("New Palette from the Document")).clicked() {
                cmd = Some("from_doc");
                ui.close();
            }
        });
        if icon_button(app, ui, "open", "Open Palette...", true) {
            cmd = Some("open");
        }
        if icon_button(app, ui, "pm_new_folder", "New Folder", true) {
            cmd = Some("folder");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(app, ui, "delete", "Delete", user_sel.is_some()) {
                cmd = Some("delete");
            }
            if icon_button(app, ui, "pm_edit", "Edit Palette...", palette_sel) {
                cmd = Some("edit");
            }
        });
    });
    if let Some(c) = cmd {
        run(app, c, sel.as_deref(), user_sel);
    }
}

fn find_folder<'a>(nodes: &'a [Node], key: &str) -> Option<&'a Node> {
    for n in nodes {
        if let Node::Folder(k, _, kids) = n {
            if *k == key {
                return Some(n);
            }
            if let Some(f) = find_folder(kids, key) {
                return Some(f);
            }
        }
    }
    None
}

type Acts = Vec<(Act, Option<Palette>, String, Option<PathBuf>)>;

fn lib_tree(app: &AnyVectorApp, ui: &mut Ui, n: &Node, depth: usize, sel: &Option<String>, acts: &mut Acts) {
    match n {
        Node::Lib(k, name) => {
            let key = format!("lib:{k}");
            if let Some(a) = row(ui, &key, depth, None, Some(Some(is_open(app, name))), name, true, sel.as_deref() == Some(key.as_str())) {
                acts.push((a, library_palette(k), key, None));
            }
        }
        Node::Missing(name) => {
            row(ui, name, depth, None, Some(Some(false)), name, false, false);
        }
        Node::MissingFolder(name) => {
            row(ui, name, depth, Some(false), Some(Some(false)), name, false, false);
        }
        Node::Folder(k, name, kids) => {
            let mut keys = vec![];
            lib_keys(n, &mut keys);
            let names: Vec<&str> = keys.iter().map(|x| x.1).collect();
            let open = app.dockers.ui.pm.open.contains(*k);
            let key = format!("libdir:{k}");
            if let Some(a) = row(ui, k, depth, Some(open), tri(app, &names), name, true, sel.as_deref() == Some(*k)) {
                acts.push((a, None, key, None));
            }
            if open {
                for c in kids {
                    lib_tree(app, ui, c, depth + 1, sel, acts);
                }
            }
        }
    }
}

fn user_tree(app: &AnyVectorApp, ui: &mut Ui, dir: &Path, label: &str, depth: usize, sel: &Option<String>, acts: &mut Acts) {
    let entries = user_entries(dir);
    let pals: Vec<(PathBuf, Palette)> = entries.iter().filter(|e| !e.1).filter_map(|(p, _)| Some((p.clone(), read_palette(p)?))).collect();
    let names: Vec<&str> = pals.iter().map(|x| x.1.name.as_str()).collect();
    let key = format!("my:{}", dir.display());
    let open = app.dockers.ui.pm.open.contains(&key);
    let check = if names.is_empty() { Some(Some(false)) } else { tri(app, &names) };
    let folder = (!entries.is_empty()).then_some(open);
    if let Some(a) = row(ui, &key, depth, folder, check, label, true, sel.as_deref() == Some(key.as_str())) {
        acts.push((a, None, format!("mydir:{}", dir.display()), None));
    }
    if !open {
        return;
    }
    for (p, is_dir) in &entries {
        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if *is_dir {
            user_tree(app, ui, p, &name, depth + 1, sel, acts);
        } else if let Some((_, pal)) = pals.iter().find(|x| &x.0 == p) {
            let key = format!("my:{}", p.display());
            if let Some(a) = row(ui, &key, depth + 1, None, Some(Some(is_open(app, &pal.name))), &pal.name, true, sel.as_deref() == Some(key.as_str())) {
                acts.push((a, Some(pal.clone()), key, Some(p.clone())));
            }
        }
    }
}

/// The palette a row stands for.
fn palette_of(app: &AnyVectorApp, key: &str) -> Option<(Palette, Option<PathBuf>)> {
    if let Some(k) = key.strip_prefix("lib:") {
        return library_palette(k).map(|p| (p, None));
    }
    if let Some(p) = key.strip_prefix("my:") {
        let p = PathBuf::from(p);
        return read_palette(&p).map(|x| (x, Some(p)));
    }
    let d = app.doc()?;
    match key {
        "doc" => Some((Palette { name: "Document Palette".into(), colors: d.doc.palette.clone() }, None)),
        "styles" => Some((Palette { name: "Color Styles Palette".into(), colors: d.doc.color_styles.clone() }, None)),
        _ => None,
    }
}

/// The folder new palettes and folders go in: the selected user folder, else My Palettes.
fn target_dir(user_sel: &Option<PathBuf>) -> PathBuf {
    match user_sel {
        Some(p) if p.is_dir() => p.clone(),
        Some(p) => p.parent().map(Path::to_path_buf).unwrap_or_else(my_palettes_dir),
        None => my_palettes_dir(),
    }
}

fn unique(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut k = 1;
    loop {
        let name = if k == 1 { format!("{stem}{ext}") } else { format!("{stem} {k}{ext}") };
        let p = dir.join(name);
        if !p.exists() {
            return p;
        }
        k += 1;
    }
}

fn run(app: &mut AnyVectorApp, c: &str, sel: Option<&str>, user_sel: Option<PathBuf>) {
    let user_sel = &user_sel;
    let expand = |app: &mut AnyVectorApp, dir: &Path| {
        app.dockers.ui.pm.open.insert(format!("my:{}", dir.display()));
    };
    match c {
        "folder" => {
            let dir = target_dir(user_sel);
            let p = unique(&dir, "New Folder", "");
            if std::fs::create_dir_all(&p).is_ok() {
                expand(app, &dir);
                app.dockers.ui.pm.sel = Some(format!("my:{}", p.display()));
            }
        }
        "blank" => {
            let dir = target_dir(user_sel);
            let p = unique(&dir, "Untitled Palette", ".xml");
            let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let pal = Palette { name: name.clone(), colors: vec![] };
            if std::fs::write(&p, pal.to_xml()).is_ok() {
                expand(app, &dir);
                set_open(app, Some(pal), &name, true, Some(&p));
                app.dockers.ui.pm.sel = Some(format!("my:{}", p.display()));
            }
        }
        "open" => app.run(Cmd::OpenPalette),
        "from_sel" => app.run(Cmd::CreatePaletteFromSelection),
        "from_doc" => app.run(Cmd::CreatePaletteFromDocument),
        "edit" => {
            if let Some((palette, path)) = sel.and_then(|k| palette_of(app, k)) {
                use crate::dialogs::palette::Source;
                let _ = palette;
                let s = match (sel, path) {
                    (_, Some(p)) => Source::File(p),
                    (Some("doc"), None) => Source::Document,
                    (Some("styles"), None) => Source::ColorStyles,
                    _ => Source::Default,
                };
                crate::dialogs::palette::open_editor_for(app, s);
            }
        }
        "cut" | "copy" => {
            if let Some(k) = sel {
                app.dockers.ui.pm.clip = Some((k.to_string(), c == "cut"));
            }
        }
        "paste" => {
            let Some((k, cut)) = app.dockers.ui.pm.clip.clone() else { return };
            let dir = target_dir(user_sel);
            if let Some(src) = k.strip_prefix("my:").map(PathBuf::from) {
                let stem = src.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let ext = src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                let to = unique(&dir, &stem, &ext);
                let ok = if cut { std::fs::rename(&src, &to).is_ok() } else { std::fs::copy(&src, &to).is_ok() };
                if ok && cut {
                    app.dockers.ui.pm.clip = None;
                }
            } else if let Some((pal, _)) = palette_of(app, &k) {
                let to = unique(&dir, &pal.name, ".xml");
                let _ = std::fs::write(&to, pal.to_xml());
            }
            expand(app, &dir);
        }
        "delete" => {
            if let Some(p) = user_sel {
                if p.is_dir() {
                    let _ = std::fs::remove_dir_all(p);
                } else {
                    if let Some(pal) = read_palette(p) {
                        set_open(app, None, &pal.name, false, Some(p));
                    }
                    let _ = std::fs::remove_file(p);
                }
                app.dockers.ui.pm.sel = None;
            }
        }
        _ => {}
    }
}
