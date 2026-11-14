//! Device output plugins: Rhai scripts that add an export format to File ▸ Export, for cutters,
//! plotters, engravers or embroidery machines whose file format AnyVector does not write.
//!
//! A plugin is a `.rhai` file in the `Devices` folder of the Scripts folder
//! (`%APPDATA%\AnyVector\Scripts\Devices`). At the top level it registers its formats, and for
//! each one names a function that turns the page into the file's bytes:
//!
//! ```rhai
//! register_export("ACME - Acme cutter (*.acm)", "acm", "write_acme");
//!
//! fn write_acme(paths, page) {
//!     let out = `SIZE ${to_mm(page.width)} ${to_mm(page.height)}` + "\n";
//!     for p in paths {
//!         out += `PEN ${p.pen} ${p.color} ${p.layer}` + "\n";
//!         for pt in p.points { out += `${to_mm(pt.x)},${to_mm(pt.y)}` + "\n"; }
//!         if p.closed { out += "CLOSE\n"; }
//!     }
//!     out   // a string (written as UTF-8) or a blob / array of bytes
//! }
//! ```
//!
//! `paths` holds the outlines of the visible, printable layers (or of the selection) already
//! flattened to straight segments, text included as glyph outlines: each is a map with
//! `points` (an array of `#{x, y}`), `closed`, `color` (`"#rrggbb"`), `spot` (the spot color
//! name, `"CutContour"` for cut lines, or `""`), `layer` and `pen` (one pen number per color,
//! from 1). Coordinates are in points (1/72 inch) with the origin at the bottom left of the page
//! and y up. `page` is `#{width, height, name}` in points. Objects with a fill but no outline are
//! included as their outline. `to_mm(v)` and `to_inch(v)` convert points.
//!
//! Only `register_export` and the conversion helpers exist while a plugin is loaded, so a
//! plugin cannot change the drawing; files are written only to the path chosen in Export.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use vector_core::document::Document;
use vector_core::ObjId;
use vector_io::cutter::{self, FillHandling};

/// An export format added by a plugin.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceFormat {
    /// The name shown in Save as type.
    pub name: String,
    /// The file extension, without the dot.
    pub ext: String,
    /// The script function that writes the file.
    pub func: String,
    pub script: PathBuf,
}

/// The curve resolution plugins get: 0.004 inch, like the HPGL export.
const TOLERANCE: f64 = 0.004 * 72.0;

/// The folder plugins are loaded from.
pub fn devices_dir() -> PathBuf {
    super::scripts_dir().join("Devices")
}

fn engine() -> rhai::Engine {
    let mut e = rhai::Engine::new();
    e.set_max_operations(200_000_000);
    e.set_max_call_levels(64);
    e.set_max_string_size(512 * 1024 * 1024);
    e.register_fn("to_mm", |v: f64| v * 25.4 / 72.0);
    e.register_fn("to_mm", |v: rhai::INT| v as f64 * 25.4 / 72.0);
    e.register_fn("to_inch", |v: f64| v / 72.0);
    e.register_fn("to_inch", |v: rhai::INT| v as f64 / 72.0);
    e
}

/// The formats a plugin script registers.
pub fn formats_in(source: &str, script: &Path) -> Result<Vec<DeviceFormat>, String> {
    let found: Arc<Mutex<Vec<(String, String, String)>>> = Arc::default();
    let mut e = engine();
    let f = found.clone();
    e.register_fn("register_export", move |name: &str, ext: &str, func: &str| {
        f.lock().unwrap().push((name.to_string(), ext.trim_start_matches('.').to_ascii_lowercase(), func.to_string()));
    });
    let ast = e.compile(source).map_err(|err| err.to_string())?;
    e.run_ast(&ast).map_err(|err| err.to_string())?;
    let list = std::mem::take(&mut *found.lock().unwrap());
    list.into_iter()
        .map(|(name, ext, func)| {
            if ext.is_empty() || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err(format!("{name}: \"{ext}\" is not a file extension"));
            }
            if !ast.iter_functions().any(|f| f.name == func && f.params.len() == 2) {
                return Err(format!("{name}: the script has no function {func}(paths, page)"));
            }
            Ok(DeviceFormat { name, ext, func, script: script.to_path_buf() })
        })
        .collect()
}

/// The formats of every plugin in the Devices folder, by file name. Scripts that fail to load
/// are logged and left out.
pub fn scan() -> Vec<DeviceFormat> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(devices_dir()).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("rhai"))).collect()).unwrap_or_default();
    files.sort();
    let mut out = vec![];
    for p in files {
        match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|s| formats_in(&s, &p)) {
            Ok(v) => out.extend(v),
            Err(e) => log::warn!("device plugin {}: {e}", p.display()),
        }
    }
    out
}

/// The `paths` and `page` values a plugin function receives.
pub fn script_input(doc: &Document, page: usize, only: Option<&[ObjId]>) -> (rhai::Array, rhai::Map) {
    let lines = cutter::page_polylines(doc, page, only, FillHandling::FilledAsOutline, TOLERANCE);
    let pens = vector_io::hpgl::assign_pens(&lines, &[]);
    let to_dev = vector_io::hpgl::device_transform(doc, page, vector_io::hpgl::HpglOrigin::PageBottomLeft, &lines);
    let mut paths = rhai::Array::new();
    for l in &lines {
        let mut m = rhai::Map::new();
        let pts: rhai::Array = l
            .points
            .iter()
            .map(|p| {
                let (x, y) = to_dev(*p);
                let mut pm = rhai::Map::new();
                pm.insert("x".into(), ((x * 1e4).round() / 1e4).into());
                pm.insert("y".into(), ((y * 1e4).round() / 1e4).into());
                pm.into()
            })
            .collect();
        m.insert("points".into(), pts.into());
        m.insert("closed".into(), l.closed.into());
        m.insert("color".into(), format!("#{:02x}{:02x}{:02x}", l.color.r, l.color.g, l.color.b).into());
        let spot = if (l.color.r, l.color.g, l.color.b) == (cutter::CUT_CONTOUR.r, cutter::CUT_CONTOUR.g, cutter::CUT_CONTOUR.b) {
            cutter::CUT_CONTOUR_NAME.to_string()
        } else {
            doc.palette.iter().chain(doc.color_styles.iter()).find(|c| c.spot && c.color == l.color).map(|c| c.name.clone()).unwrap_or_default()
        };
        m.insert("spot".into(), spot.into());
        m.insert("layer".into(), l.layer.clone().into());
        let pen = pens.iter().find(|(c, _)| *c == l.color).map(|(_, p)| *p).unwrap_or(1);
        m.insert("pen".into(), (pen as rhai::INT).into());
        paths.push(m.into());
    }
    let mut pm = rhai::Map::new();
    let (w, h) = doc.pages.get(page).map(|p| (p.width, p.height)).unwrap_or_default();
    pm.insert("width".into(), w.into());
    pm.insert("height".into(), h.into());
    pm.insert("name".into(), doc.page_name(page).into());
    (paths, pm)
}

/// Run a plugin's writer on a page and return the file's bytes.
pub fn run(fmt: &DeviceFormat, source: &str, doc: &Document, page: usize, only: Option<&[ObjId]>) -> Result<Vec<u8>, String> {
    let mut e = engine();
    e.register_fn("register_export", |_: &str, _: &str, _: &str| {});
    let ast = e.compile(source).map_err(|err| err.to_string())?;
    let (paths, page_map) = script_input(doc, page, only);
    let r: rhai::Dynamic = e.call_fn(&mut rhai::Scope::new(), &ast, &fmt.func, (paths, page_map)).map_err(|err| err.to_string())?;
    if r.is_string() {
        return Ok(r.into_string().unwrap_or_default().into_bytes());
    }
    if r.is_blob() {
        return Ok(r.cast::<rhai::Blob>());
    }
    if r.is_array() {
        return r
            .cast::<rhai::Array>()
            .into_iter()
            .map(|v| v.as_int().ok().and_then(|i| u8::try_from(i).ok()).ok_or_else(|| "the array returned is not all bytes (0 to 255)".to_string()))
            .collect();
    }
    Err(format!("{} returned {}, not a string or bytes", fmt.func, r.type_name()))
}

/// Export a page with a plugin format.
pub fn export(fmt: &DeviceFormat, doc: &Document, page: usize, only: Option<&[ObjId]>, path: &Path) -> Result<(), String> {
    let source = std::fs::read_to_string(&fmt.script).map_err(|e| format!("{}: {e}", fmt.script.display()))?;
    let bytes = run(fmt, &source, doc, page, only)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{Affine, Point};
    use vector_core::object::{ObjKind, Object};
    use vector_core::{Color, Fill, Style};

    const PLUGIN: &str = r#"
        register_export("TST - Test cutter (*.tst)", ".TST", "write_tst");
        register_export("BIN - Binary cutter (*.bin)", "bin", "write_bin");

        fn write_tst(paths, page) {
            let out = `SIZE ${to_inch(page.width)}x${to_inch(page.height)}` + "\n";
            for p in paths {
                out += `PEN ${p.pen} ${p.color} ${p.spot} ${p.layer}` + "\n";
                for pt in p.points { out += `${pt.x},${pt.y};`; }
                if p.closed { out += "CLOSE"; }
                out += "\n";
            }
            out
        }

        fn write_bin(paths, page) {
            let b = blob();
            b.push(paths.len());
            for p in paths { b.push(p.points.len()); }
            b
        }
    "#;

    fn doc() -> Document {
        let mut doc = Document::new(72.0, 72.0);
        let mut st = Style::default();
        st.fill = Fill::None;
        st.outline.color = Some(cutter::CUT_CONTOUR);
        let sq = vector_core::path::polyline(&[Point::new(0.0, 0.0), Point::new(36.0, 0.0), Point::new(36.0, 36.0)], true);
        let id = doc.new_id();
        doc.pages[0].layers[0].objects.push(Object::new(id, ObjKind::Path(sq), Affine::IDENTITY, st.clone()));
        st.outline.color = Some(Color::BLACK);
        let line = vector_core::path::polyline(&[Point::new(0.0, 72.0), Point::new(72.0, 72.0)], false);
        let id = doc.new_id();
        doc.pages[0].layers[0].objects.push(Object::new(id, ObjKind::Path(line), Affine::IDENTITY, st));
        doc
    }

    #[test]
    fn plugin_registers_formats_and_writes_bytes() {
        let fmts = formats_in(PLUGIN, Path::new("test.rhai")).unwrap();
        assert_eq!(fmts.len(), 2);
        assert_eq!((fmts[0].name.as_str(), fmts[0].ext.as_str(), fmts[0].func.as_str()), ("TST - Test cutter (*.tst)", "tst", "write_tst"));
        let d = doc();
        let layer = d.pages[0].layers[0].name.clone();
        let out = String::from_utf8(run(&fmts[0], PLUGIN, &d, 0, None).unwrap()).unwrap();
        assert_eq!(out, format!("SIZE 1.0x1.0\nPEN 1 #ec008c CutContour {layer}\n0.0,72.0;36.0,72.0;36.0,36.0;CLOSE\nPEN 2 #000000  {layer}\n0.0,0.0;72.0,0.0;\n"));
        assert_eq!(run(&fmts[1], PLUGIN, &d, 0, None).unwrap(), vec![2, 3, 2]);
        // the selection only
        let first = d.pages[0].layers[0].objects[1].id;
        assert_eq!(run(&fmts[1], PLUGIN, &d, 0, Some(&[first])).unwrap(), vec![1, 2]);
        // written to a file
        let path = std::env::temp_dir().join(format!("anyvector-device-{}.bin", std::process::id()));
        let script = std::env::temp_dir().join(format!("anyvector-device-{}.rhai", std::process::id()));
        std::fs::write(&script, PLUGIN).unwrap();
        let fmt = DeviceFormat { script: script.clone(), ..fmts[1].clone() };
        export(&fmt, &d, 0, None, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), vec![2, 3, 2]);
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(script);
    }

    #[test]
    fn plugins_cannot_draw_and_must_name_their_writer() {
        assert!(formats_in("rect(0, 0, 10, 10);", Path::new("x.rhai")).is_err());
        assert!(formats_in(r#"register_export("X", "x", "missing");"#, Path::new("x.rhai")).unwrap_err().contains("no function missing"));
        assert!(formats_in(r#"register_export("X", "a/b", "f"); fn f(a, b) { "" }"#, Path::new("x.rhai")).is_err());
    }
}
