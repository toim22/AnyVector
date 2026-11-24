//! Printing on Linux through CUPS, with its command-line programs only (no libcups): `lpstat` lists
//! the printers, `lpoptions -l` gives each printer's choices (paper, 2-sided, color, resolution),
//! the printer's PPD (served by the CUPS scheduler) gives the paper sizes and printable areas, and
//! `lp` takes the job as a PDF of page images.
//!
//! Linux printer drivers have no settings dialog of their own, so AnyVector's Printing Preferences
//! dialog (`dialogs::print`) chooses the options; they are kept as `key=value` lines, the bytes the
//! Print dialog stores where Windows keeps a DEVMODE, and go to `lp` as `-o key=value`.
//!
//! The parsing is plain text and builds everywhere, so the tests (and the visual tests) run on
//! Windows too; only the programs are run on Linux.
#![cfg_attr(windows, allow(dead_code))]

use super::Paper;
use kurbo::Rect;
use std::collections::HashMap;

/// The keys of the options the preferences dialog shows: PPD keywords and their IPP names.
pub const PAPER_KEYS: [&str; 2] = ["PageSize", "media"];
pub const DUPLEX_KEYS: [&str; 2] = ["Duplex", "sides"];
pub const COLOR_KEYS: [&str; 2] = ["ColorModel", "print-color-mode"];
pub const RESOLUTION_KEYS: [&str; 2] = ["Resolution", "printer-resolution"];

/// Margins (points) when the printer does not say where it can print: Paper::fallback's.
const FALLBACK_MARGIN: f64 = 18.0;
/// Resolution when the printer does not give one: Paper::fallback's.
const FALLBACK_DPI: f64 = 600.0;

// ------------------------------------------------------------------------------------------
// the printer's choices
// ------------------------------------------------------------------------------------------

/// One option of `lpoptions -l`: "PageSize/Media Size: Letter *A4 Legal".
#[derive(Clone, Debug, PartialEq)]
pub struct PrinterOption {
    pub key: String,
    /// The PPD's caption ("Media Size").
    pub text: String,
    pub choices: Vec<String>,
    /// The choice marked `*`: the printer's default, or the user's own (`lpoptions -o`).
    pub default: Option<usize>,
}

/// What a printer offers: its options and, from its PPD, the paper geometry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Caps {
    pub options: Vec<PrinterOption>,
    pub ppd: Ppd,
}

/// The paper geometry of a PPD, by PageSize choice, in points.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ppd {
    /// `*PaperDimension A4/A4: "595 842"`
    pub sizes: HashMap<String, (f64, f64)>,
    /// `*ImageableArea A4/A4: "18 36 577 806"` (left, bottom, right, top from the bottom-left corner)
    pub areas: HashMap<String, [f64; 4]>,
    /// `*PageSize Letter/US Letter: ...`: the choice captions.
    pub names: HashMap<String, String>,
}

/// The printer names of `lpstat -e` (one per line; instances are `printer/instance`). Messages such
/// as "lpstat: No destinations added." or "scheduler is not running" have spaces and are skipped:
/// CUPS names never do.
pub fn parse_lpstat_e(text: &str) -> Vec<String> {
    let mut v: Vec<String> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.contains(char::is_whitespace) && !l.ends_with(':')).map(String::from).collect();
    v.dedup();
    v
}

/// The default printer of `lpstat -d` ("system default destination: Office"). It already counts the
/// user's own default (`lpoptions -d`, `LPDEST`, `PRINTER`).
pub fn parse_lpstat_d(text: &str) -> Option<String> {
    text.lines().find_map(|l| l.strip_prefix("system default destination:")).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// The options of `lpoptions -p printer -l`.
pub fn parse_lpoptions(text: &str) -> Vec<PrinterOption> {
    let mut out = vec![];
    for line in text.lines() {
        // "Key/Caption: a *b c"; the caption may itself hold a colon, the key never does
        let Some((head, rest)) = line.split_once(": ") else { continue };
        let (key, caption) = head.split_once('/').unwrap_or((head, head));
        let key = key.trim();
        if key.is_empty() || key.contains(char::is_whitespace) {
            continue;
        }
        let mut choices = vec![];
        let mut default = None;
        for w in rest.split_whitespace() {
            match w.strip_prefix('*') {
                Some(c) => {
                    default = Some(choices.len());
                    choices.push(c.to_string());
                }
                None => choices.push(w.to_string()),
            }
        }
        if !choices.is_empty() {
            out.push(PrinterOption { key: key.to_string(), text: caption.trim().to_string(), choices, default });
        }
    }
    out
}

/// The quoted values of a PPD line `*Keyword Choice/Caption: "values"`.
fn ppd_entry<'a>(line: &'a str, keyword: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let rest = line.strip_prefix('*')?.strip_prefix(keyword)?;
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let (spec, value) = rest.trim_start().split_once(':')?;
    let (choice, caption) = spec.split_once('/').unwrap_or((spec, spec));
    Some((choice.trim(), caption.trim(), value.trim().trim_matches('"')))
}

fn numbers(s: &str) -> Vec<f64> {
    s.split_whitespace().filter_map(|w| w.parse().ok()).collect()
}

/// The paper sizes, printable areas and captions of a PPD.
pub fn parse_ppd(text: &str) -> Ppd {
    let mut p = Ppd::default();
    for line in text.lines() {
        if let Some((c, _, v)) = ppd_entry(line, "PaperDimension") {
            if let [w, h] = numbers(v)[..] {
                p.sizes.insert(c.to_string(), (w, h));
            }
        } else if let Some((c, _, v)) = ppd_entry(line, "ImageableArea") {
            if let [a, b, cc, d] = numbers(v)[..] {
                p.areas.insert(c.to_string(), [a, b, cc, d]);
            }
        } else if let Some((c, caption, _)) = ppd_entry(line, "PageSize") {
            if caption != c {
                p.names.insert(c.to_string(), caption.to_string());
            }
        }
    }
    p
}

impl Caps {
    /// The first option with one of these keys.
    pub fn option(&self, keys: &[&str]) -> Option<&PrinterOption> {
        keys.iter().find_map(|k| self.options.iter().find(|o| o.key == *k))
    }

    /// The choice of the option with one of these keys: the one in `opts`, else the default.
    pub fn chosen(&self, keys: &[&str], opts: &Options) -> Option<(String, String)> {
        let o = self.option(keys)?;
        let c = opts.get(&o.key).map(String::from).or_else(|| o.default.and_then(|i| o.choices.get(i).cloned()))?;
        Some((o.key.clone(), c))
    }

    /// The paper the printer prints on with these options, in this orientation.
    pub fn paper(&self, opts: &Options, landscape: bool) -> Paper {
        let dpi = self.chosen(&RESOLUTION_KEYS, opts).and_then(|(_, r)| parse_resolution(&r)).unwrap_or((FALLBACK_DPI, FALLBACK_DPI));
        let Some((_, name)) = self.chosen(&PAPER_KEYS, opts) else {
            let mut p = Paper::fallback(landscape);
            p.dpi = dpi;
            return p;
        };
        let Some((w, h)) = self.ppd.sizes.get(&name).copied().or_else(|| paper_size(&name)) else {
            let mut p = Paper::fallback(landscape);
            p.dpi = dpi;
            return p;
        };
        // printable area from the top-left corner, as GDI gives it
        let area = match self.ppd.areas.get(&name) {
            Some(&[l, b, r, t]) => Rect::new(l.max(0.0), (h - t).max(0.0), r.min(w), (h - b).min(h)),
            None => Rect::new(FALLBACK_MARGIN, FALLBACK_MARGIN, w - FALLBACK_MARGIN, h - FALLBACK_MARGIN),
        };
        let area = if area.width() > 1.0 && area.height() > 1.0 { area } else { Rect::new(0.0, 0.0, w, h) };
        if !landscape {
            return Paper { width: w, height: h, printable: area, dpi };
        }
        // landscape turns the paper a quarter turn counter-clockwise (CUPS' orientation-requested=4):
        // the portrait left margin is at the top, its bottom margin at the left
        let printable = Rect::new(h - area.y1, area.x0, h - area.y0, area.x1);
        Paper { width: h, height: w, printable, dpi: (dpi.1, dpi.0) }
    }
}

/// "600dpi", "300x600dpi", "118dpcm" → dots per inch (horizontal, vertical).
pub fn parse_resolution(s: &str) -> Option<(f64, f64)> {
    let s = s.trim().to_ascii_lowercase();
    let (num, k) = if let Some(n) = s.strip_suffix("dpi") {
        (n, 1.0)
    } else if let Some(n) = s.strip_suffix("dpcm") {
        (n, 2.54)
    } else {
        return None;
    };
    let (x, y) = match num.split_once('x') {
        Some((x, y)) => (x.parse::<f64>().ok()?, y.parse::<f64>().ok()?),
        None => {
            let v = num.parse::<f64>().ok()?;
            (v, v)
        }
    };
    (x > 0.0 && y > 0.0).then_some((x * k, y * k))
}

const MM: f64 = 72.0 / 25.4;

/// ISO and JIS series in millimetres, index = the number (A0, A1, ...).
const ISO_A: [(f64, f64); 11] = [(841.0, 1189.0), (594.0, 841.0), (420.0, 594.0), (297.0, 420.0), (210.0, 297.0), (148.0, 210.0), (105.0, 148.0), (74.0, 105.0), (52.0, 74.0), (37.0, 52.0), (26.0, 37.0)];
const ISO_B: [(f64, f64); 7] = [(1000.0, 1414.0), (707.0, 1000.0), (500.0, 707.0), (353.0, 500.0), (250.0, 353.0), (176.0, 250.0), (125.0, 176.0)];
const ISO_C: [(f64, f64); 7] = [(917.0, 1297.0), (648.0, 917.0), (458.0, 648.0), (324.0, 458.0), (229.0, 324.0), (162.0, 229.0), (114.0, 162.0)];
const JIS_B: [(f64, f64); 7] = [(1030.0, 1456.0), (728.0, 1030.0), (515.0, 728.0), (364.0, 515.0), (257.0, 364.0), (182.0, 257.0), (128.0, 182.0)];
const RA: [(f64, f64); 5] = [(860.0, 1220.0), (610.0, 860.0), (430.0, 610.0), (305.0, 430.0), (215.0, 305.0)];
const SRA: [(f64, f64); 5] = [(900.0, 1280.0), (640.0, 900.0), (450.0, 640.0), (320.0, 450.0), (225.0, 320.0)];

/// The PPD's standard names (Adobe PPD 4.3, appendix B) that are not a series, in points.
const NAMED: [(&str, f64, f64); 23] = [
    ("letter", 612.0, 792.0),
    ("legal", 612.0, 1008.0),
    ("tabloid", 792.0, 1224.0),
    ("ledger", 1224.0, 792.0),
    ("executive", 522.0, 756.0),
    ("statement", 396.0, 612.0),
    ("folio", 612.0, 936.0),
    ("quarto", 610.0, 780.0),
    ("tabloidextra", 864.0, 1296.0),
    ("ansia", 612.0, 792.0),
    ("ansib", 792.0, 1224.0),
    ("ansic", 1224.0, 1584.0),
    ("ansid", 1584.0, 2448.0),
    ("ansie", 2448.0, 3168.0),
    ("archa", 648.0, 864.0),
    ("archb", 864.0, 1296.0),
    ("archc", 1296.0, 1728.0),
    ("archd", 1728.0, 2592.0),
    ("arche", 2592.0, 3456.0),
    ("env10", 297.0, 684.0),
    ("envdl", 312.0, 624.0),
    ("envmonarch", 279.0, 540.0),
    ("postcard", 284.0, 419.0),
];

fn series(table: &[(f64, f64)], n: &str) -> Option<(f64, f64)> {
    let i: usize = n.parse().ok()?;
    table.get(i).map(|(w, h)| (w * MM, h * MM))
}

/// A paper size from its name, in points: PWG names ("na_letter_8.5x11in", "iso_a4_210x297mm"),
/// the PPD's ("Letter", "A4", "Env10", "4x6", "w288h432", "A4.Fullbleed", "A4Small") and
/// "Custom.8.5x11in". None for names it does not know (and the `Custom.WIDTHxHEIGHT` template).
pub fn paper_size(name: &str) -> Option<(f64, f64)> {
    // PWG self-describing names end with "_<w>x<h><in|mm>"
    if name.contains('_') {
        let dims = name.rsplit('_').next()?;
        return dims_with_unit(dims);
    }
    if let Some(d) = name.strip_prefix("Custom.") {
        return dims_with_unit(d).or_else(|| {
            // Custom.WxH in points
            let (w, h) = d.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        });
    }
    // variants of a size: borderless, with larger margins, or turned
    let mut n = name.to_ascii_lowercase();
    let mut turned = false;
    for suffix in [".fullbleed", ".borderless", ".duplex"] {
        if let Some(s) = n.strip_suffix(suffix) {
            n = s.to_string();
        }
    }
    for suffix in [".transverse", "rotated", ".rotated"] {
        if let Some(s) = n.strip_suffix(suffix) {
            n = s.to_string();
            turned = true;
        }
    }
    if let Some(s) = n.strip_suffix("small") {
        n = s.to_string();
    }
    let size = base_size(&n)?;
    Some(if turned { (size.1, size.0) } else { size })
}

fn base_size(n: &str) -> Option<(f64, f64)> {
    if let Some(&(_, w, h)) = NAMED.iter().find(|(k, _, _)| *k == n) {
        return Some((w, h));
    }
    // "w288h432": points
    if let Some(rest) = n.strip_prefix('w') {
        if let Some((w, h)) = rest.split_once('h') {
            if let (Ok(w), Ok(h)) = (w.parse::<f64>(), h.parse::<f64>()) {
                return Some((w, h));
            }
        }
    }
    // "4x6", "8x10", "11x17": inches
    if let Some((w, h)) = n.split_once('x') {
        if let (Ok(w), Ok(h)) = (w.parse::<f64>(), h.parse::<f64>()) {
            return Some((w * 72.0, h * 72.0));
        }
    }
    for (prefix, table) in [("sra", &SRA[..]), ("ra", &RA[..]), ("isob", &ISO_B[..]), ("jisb", &JIS_B[..]), ("envc", &ISO_C[..]), ("a", &ISO_A[..]), ("c", &ISO_C[..])] {
        if let Some(num) = n.strip_prefix(prefix) {
            if let Some(s) = series(table, num) {
                return Some(s);
            }
        }
    }
    // the PPD's plain "B4"/"B5" are JIS sizes
    n.strip_prefix('b').and_then(|num| series(&JIS_B, num))
}

/// "8.5x11in", "210x297mm" in points.
fn dims_with_unit(d: &str) -> Option<(f64, f64)> {
    let (num, k) = if let Some(n) = d.strip_suffix("in") {
        (n, 72.0)
    } else if let Some(n) = d.strip_suffix("mm") {
        (n, MM)
    } else {
        return None;
    };
    let (w, h) = num.split_once('x')?;
    let (w, h) = (w.parse::<f64>().ok()?, h.parse::<f64>().ok()?);
    (w > 0.0 && h > 0.0).then_some((w * k, h * k))
}

/// The caption of a paper size choice: the PPD's ("US Letter") or one made from a PWG name, with
/// the size ("A4 (210 × 297 mm)", "US Letter (8.5 × 11 in)").
pub fn paper_label(caps: &Caps, name: &str) -> String {
    let base = caps.ppd.names.get(name).cloned().unwrap_or_else(|| pwg_caption(name));
    let Some((w, h)) = caps.ppd.sizes.get(name).copied().or_else(|| paper_size(name)) else { return base };
    let num = |v: f64| {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    if is_metric(name) {
        format!("{base} ({} × {} mm)", (w / MM).round(), (h / MM).round())
    } else {
        format!("{base} ({} × {} in)", num(w / 72.0), num(h / 72.0))
    }
}

/// Whether a paper size is a metric one (shown in millimetres): PWG names in mm, and the ISO, JIS
/// and RA series and DL envelopes of the PPD.
fn is_metric(name: &str) -> bool {
    if name.contains('_') || name.starts_with("Custom.") {
        return name.ends_with("mm");
    }
    let n = name.to_ascii_lowercase();
    let n = n.split('.').next().unwrap_or(&n).trim_end_matches("small").trim_end_matches("rotated");
    n == "envdl" || ["sra", "ra", "isob", "jisb", "envc", "a", "b", "c"].iter().any(|p| n.strip_prefix(p).is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())))
}

/// "na_letter_8.5x11in" → "Letter", "iso_a4_210x297mm" → "A4", other names as they are.
fn pwg_caption(name: &str) -> String {
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() < 3 {
        return name.to_string();
    }
    let n = parts[1..parts.len() - 1].join(" ");
    if parts[0] == "iso" || parts[0] == "jis" || n.len() <= 3 {
        return n.to_uppercase();
    }
    let mut c = n.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or(n)
}

/// The caption of a choice of the 2-sided, color or resolution option, in the dialog's words.
pub fn choice_label(key: &str, choice: &str) -> String {
    let lower = choice.to_ascii_lowercase();
    if DUPLEX_KEYS.contains(&key) {
        return match lower.as_str() {
            "none" | "one-sided" | "simplex" | "off" => "Off".into(),
            "duplexnotumble" | "two-sided-long-edge" | "longedge" => "Flip on long edge".into(),
            "duplextumble" | "two-sided-short-edge" | "shortedge" => "Flip on short edge".into(),
            _ => choice.into(),
        };
    }
    if COLOR_KEYS.contains(&key) {
        if lower == "auto" {
            return "Automatic".into();
        }
        if ["gray", "grey", "kgray", "black", "mono", "monochrome", "process-monochrome", "bi-level", "w"].contains(&lower.as_str()) {
            return "Grayscale".into();
        }
        if ["rgb", "cmyk", "cmy", "kcmy", "rgbw", "rgbk", "color", "colour"].contains(&lower.as_str()) {
            return "Color".into();
        }
        return choice.into();
    }
    if RESOLUTION_KEYS.contains(&key) {
        if let Some((x, y)) = parse_resolution(choice).filter(|_| lower.ends_with("dpi")) {
            return if x == y { format!("{x} dpi") } else { format!("{x} × {y} dpi") };
        }
    }
    choice.into()
}

// ------------------------------------------------------------------------------------------
// the chosen options (the bytes the Print dialog keeps)
// ------------------------------------------------------------------------------------------

/// The options chosen in Printing Preferences, `key=value` in order; sent as `lp -o key=value`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options(pub Vec<(String, String)>);

impl Options {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        match self.0.iter_mut().find(|(k, _)| k == key) {
            Some(kv) => kv.1 = value.to_string(),
            None => self.0.push((key.to_string(), value.to_string())),
        }
    }

    /// One `key=value` per line. Keys and values have no spaces, `=` or line breaks (CUPS names).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.iter().map(|(k, v)| format!("{k}={v}\n")).collect::<String>().into_bytes()
    }

    /// The options in `bytes`; lines that are not `key=value` are skipped.
    pub fn from_bytes(bytes: Option<&[u8]>) -> Options {
        let text = bytes.map(String::from_utf8_lossy).unwrap_or_default();
        Options(
            text.lines()
                .filter_map(|l| {
                    let (k, v) = l.split_once('=')?;
                    let ok = |s: &str| !s.is_empty() && !s.contains(char::is_whitespace);
                    (ok(k) && ok(v)).then(|| (k.to_string(), v.to_string()))
                })
                .collect(),
        )
    }
}

/// The arguments of `lp` for a job: printer, copies, title, collation, then the options.
pub fn lp_args(printer: &str, copies: u32, collate: bool, title: &str, opts: &Options, file: &str) -> Vec<String> {
    let mut a: Vec<String> = vec!["-d".into(), printer.into(), "-n".into(), copies.max(1).to_string()];
    if !title.trim().is_empty() {
        a.extend(["-t".into(), title.into()]);
    }
    if copies > 1 {
        a.extend(["-o".into(), format!("collate={collate}")]);
    }
    for (k, v) in &opts.0 {
        a.extend(["-o".into(), format!("{k}={v}")]);
    }
    a.push(file.into());
    a
}

/// The job of `lp`'s answer: "request id is Office-42 (1 file(s))" → "Office-42".
pub fn parse_lp_job(text: &str) -> Option<String> {
    text.lines().find_map(|l| l.trim().strip_prefix("request id is ")).and_then(|r| r.split_whitespace().next()).map(String::from)
}

// ------------------------------------------------------------------------------------------
// running CUPS' programs (Linux)
// ------------------------------------------------------------------------------------------

/// What a program printed and whether it succeeded, run with the C locale (so it answers in
/// English); `None` when it is not installed or does not finish in time (a scheduler that does not
/// answer). `timeout` None waits as long as it takes (`lp` sending a large job).
#[cfg(not(windows))]
fn run(program: &str, args: &[String], timeout: Option<std::time::Duration>) -> Option<(bool, String, String)> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut child = Command::new(program).args(args).env("LC_ALL", "C").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let (mut out, mut err) = (child.stdout.take()?, child.stderr.take()?);
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        s
    });
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = err.read_to_string(&mut s);
        s
    });
    let deadline = timeout.map(|t| std::time::Instant::now() + t);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some((status.success(), reader.join().ok()?, err_reader.join().ok()?)),
            Ok(None) if deadline.is_none_or(|d| std::time::Instant::now() < d) => std::thread::sleep(std::time::Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(not(windows))]
const ASK: Option<std::time::Duration> = Some(std::time::Duration::from_secs(3));

/// The printers CUPS knows (none without CUPS or when its scheduler is not running).
#[cfg(not(windows))]
pub fn printers() -> Vec<String> {
    run("lpstat", &["-e".into()], ASK).map(|(_, out, _)| parse_lpstat_e(&out)).unwrap_or_default()
}

#[cfg(not(windows))]
pub fn default_printer() -> Option<String> {
    run("lpstat", &["-d".into()], ASK).and_then(|(_, out, _)| parse_lpstat_d(&out))
}

/// The printer's options and paper geometry.
#[cfg(not(windows))]
pub fn caps(printer: &str) -> Caps {
    let options = run("lpoptions", &["-p".into(), printer.into(), "-l".into()], ASK).map(|(_, out, _)| parse_lpoptions(&out)).unwrap_or_default();
    let ppd = fetch_ppd(printer).map(|t| parse_ppd(&t)).unwrap_or_default();
    Caps { options, ppd }
}

/// The printer's PPD from the CUPS scheduler (`/printers/<name>.ppd`): the copy in /etc/cups/ppd
/// is readable by root and the lp group only.
#[cfg(not(windows))]
fn fetch_ppd(printer: &str) -> Option<String> {
    let queue = printer.split('/').next().unwrap_or(printer);
    let url = format!("http://{}/printers/{}.ppd", cups_server(std::env::var("CUPS_SERVER").ok().as_deref()), percent(queue));
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(std::time::Duration::from_secs(3))).build().into();
    let mut r = agent.get(&url).call().ok()?;
    if r.status().as_u16() != 200 {
        return None;
    }
    // PPDs are ISO Latin 1 or UTF-8; the parts read here are ASCII
    let bytes = r.body_mut().with_config().limit(16 << 20).read_to_vec().ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The scheduler's host and port: `CUPS_SERVER` when it names a host, else the local one. A socket
/// path (`/run/cups/cups.sock`) means the local scheduler.
pub fn cups_server(var: Option<&str>) -> String {
    match var.map(str::trim).filter(|s| !s.is_empty() && !s.starts_with('/')) {
        Some(s) if s.starts_with('[') || s.matches(':').count() == 1 => s.to_string(),
        Some(s) if s.contains(':') => format!("[{s}]:631"),
        Some(s) => format!("{s}:631"),
        None => "localhost:631".into(),
    }
}

/// A printer name in a URL path.
fn percent(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Send a PDF to the printer with `lp`; the job's name in CUPS ("Office-42").
#[cfg(not(windows))]
pub fn submit(printer: &str, copies: u32, collate: bool, title: &str, opts: &Options, file: &std::path::Path) -> Result<String, String> {
    let args = lp_args(printer, copies, collate, title, opts, &file.to_string_lossy());
    match run("lp", &args, None) {
        None => Err("Could not run lp, the CUPS print command. Install the CUPS client (the cups-client package).".into()),
        Some((true, out, _)) => Ok(parse_lp_job(&out).unwrap_or_default()),
        Some((false, _, err)) => {
            let why = err.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
            let why = why.strip_prefix("lp: ").unwrap_or(&why).to_string();
            Err(format!("The printer did not accept the job: {why}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `lpstat -e` and `-d` of CUPS 2.4 with the cups-pdf printer, with no printers and with the
    /// scheduler stopped (stdout is empty then, the message goes to stderr).
    #[test]
    fn printers_from_lpstat() {
        assert_eq!(parse_lpstat_e("PDF\nOffice_Laser\nOffice_Laser/draft\n"), ["PDF", "Office_Laser", "Office_Laser/draft"]);
        assert!(parse_lpstat_e("").is_empty());
        assert!(parse_lpstat_e("lpstat: No destinations added.\n").is_empty());
        assert!(parse_lpstat_e("scheduler is not running\n").is_empty());
        assert_eq!(parse_lpstat_d("system default destination: PDF\n").as_deref(), Some("PDF"));
        assert_eq!(parse_lpstat_d("no system default destination\n"), None);
        assert_eq!(parse_lpstat_d(""), None);
    }

    /// CUPS not installed: the programs cannot start, so there are no printers (not an error).
    #[cfg(not(windows))]
    #[test]
    fn missing_programs_mean_no_printers() {
        assert!(run("anyvector-no-such-lpstat", &["-e".into()], ASK).is_none());
    }

    const CUPS_PDF: &str = "PageSize/Page Size: Custom.WIDTHxHEIGHT 11x14 11x17 A3 *A4 A5 Env10 EnvDL Executive ISOB5 JISB5 Ledger Legal Letter SRA3 Tabloid\n\
Resolution/Output Resolution: 150dpi *300dpi 600dpi 1200dpi\n\
PDFVer/PDF version: 1.1 *1.2 1.3 1.4 1.5\n\
Truncate/Truncate output filename to: 8 16 32 *64\n";

    const LASER: &str = "PageSize/Media Size: *Letter Legal A4 Env10\n\
Duplex/2-Sided Printing: None *DuplexNoTumble DuplexTumble\n\
ColorModel/Color Mode: Gray *RGB\n\
Resolution/Resolution: 600dpi *1200x600dpi\n";

    /// driverless (IPP Everywhere) queue: IPP names
    const IPP: &str = "media/Media: *na_letter_8.5x11in iso_a4_210x297mm na_index-4x6_4x6in\n\
sides/2-Sided Printing: *one-sided two-sided-long-edge two-sided-short-edge\n\
print-color-mode/Print Color Mode: auto color *monochrome\n\
printer-resolution/Print Resolution: *300dpi 600dpi\n";

    #[test]
    fn options_from_lpoptions() {
        let o = parse_lpoptions(CUPS_PDF);
        assert_eq!(o.len(), 4);
        assert_eq!(o[0].key, "PageSize");
        assert_eq!(o[0].text, "Page Size");
        assert_eq!(o[0].default.map(|i| o[0].choices[i].as_str()), Some("A4"));
        assert_eq!(o[1].choices, ["150dpi", "300dpi", "600dpi", "1200dpi"]);
        let caps = Caps { options: parse_lpoptions(LASER), ..Default::default() };
        assert_eq!(caps.chosen(&DUPLEX_KEYS, &Options::default()), Some(("Duplex".into(), "DuplexNoTumble".into())));
        let caps = Caps { options: parse_lpoptions(IPP), ..Default::default() };
        assert_eq!(caps.chosen(&COLOR_KEYS, &Options::default()), Some(("print-color-mode".into(), "monochrome".into())));
        assert_eq!(caps.chosen(&PAPER_KEYS, &Options(vec![("media".into(), "iso_a4_210x297mm".into())])).map(|c| c.1).as_deref(), Some("iso_a4_210x297mm"));
        // a printer that is gone: lpoptions says so (and exits 0), nothing to parse
        assert!(parse_lpoptions("lpoptions: Unable to get PPD file for Nope: No such file or directory\n").is_empty());
        assert!(parse_lpoptions("").is_empty());
    }

    #[test]
    fn paper_sizes_from_names() {
        let near = |a: Option<(f64, f64)>, w: f64, h: f64| a.is_some_and(|(x, y)| (x - w).abs() < 0.6 && (y - h).abs() < 0.6);
        assert!(near(paper_size("Letter"), 612.0, 792.0));
        assert!(near(paper_size("A4"), 595.3, 841.9));
        assert!(near(paper_size("A4.Fullbleed"), 595.3, 841.9));
        assert!(near(paper_size("A4Small"), 595.3, 841.9));
        assert!(near(paper_size("A4.Transverse"), 841.9, 595.3));
        assert!(near(paper_size("A3"), 841.9, 1190.6));
        assert!(near(paper_size("Legal"), 612.0, 1008.0));
        assert!(near(paper_size("Env10"), 297.0, 684.0));
        assert!(near(paper_size("EnvC5"), 459.2, 649.1));
        assert!(near(paper_size("ISOB5"), 498.9, 708.7));
        assert!(near(paper_size("JISB5"), 515.9, 728.5));
        assert!(near(paper_size("B5"), 515.9, 728.5));
        assert!(near(paper_size("SRA3"), 907.1, 1275.6));
        assert!(near(paper_size("4x6"), 288.0, 432.0));
        assert!(near(paper_size("w288h432"), 288.0, 432.0));
        assert!(near(paper_size("na_letter_8.5x11in"), 612.0, 792.0));
        assert!(near(paper_size("iso_a4_210x297mm"), 595.3, 841.9));
        assert!(near(paper_size("na_index-4x6_4x6in"), 288.0, 432.0));
        assert!(near(paper_size("Custom.8.5x11in"), 612.0, 792.0));
        assert_eq!(paper_size("Custom.WIDTHxHEIGHT"), None);
        assert_eq!(paper_size("Banner"), None);
        assert_eq!(parse_resolution("1200x600dpi"), Some((1200.0, 600.0)));
        assert_eq!(parse_resolution("300dpi"), Some((300.0, 300.0)));
        assert_eq!(parse_resolution("Normal"), None);
    }

    const PPD: &str = "*PPD-Adobe: \"4.3\"\n\
*OpenUI *PageSize/Media Size: PickOne\n\
*DefaultPageSize: Letter\n\
*PageSize Letter/US Letter: \"<</PageSize[612 792]>>setpagedevice\"\n\
*PageSize A4/A4: \"<</PageSize[595 842]>>setpagedevice\"\n\
*ImageableArea Letter/US Letter: \"18 36 594 774\"\n\
*ImageableArea A4/A4: \"12.24 12.06 583.76 829.94\"\n\
*PaperDimension Letter/US Letter: \"612 792\"\n\
*PaperDimension A4/A4: \"595 842\"\n";

    /// The PPD's paper size and printable area; the user's choice wins over the default; landscape
    /// turns the margins with the paper; without a PPD the size comes from the name.
    #[test]
    fn paper_from_the_ppd_and_options() {
        let caps = Caps { options: parse_lpoptions(LASER), ppd: parse_ppd(PPD) };
        assert_eq!(caps.ppd.names.get("Letter").map(String::as_str), Some("US Letter"));
        let p = caps.paper(&Options::default(), false);
        assert_eq!((p.width, p.height), (612.0, 792.0));
        assert_eq!(p.printable, Rect::new(18.0, 18.0, 594.0, 756.0));
        assert_eq!(p.dpi, (1200.0, 600.0));
        let l = caps.paper(&Options::default(), true);
        assert_eq!((l.width, l.height), (792.0, 612.0));
        // left 18 → top 18, bottom 36 → left 36, right 18 → bottom 18, top 18 → right 18
        assert_eq!(l.printable, Rect::new(36.0, 18.0, 774.0, 594.0));
        assert_eq!(l.dpi, (600.0, 1200.0));
        let a4 = Options(vec![("PageSize".into(), "A4".into()), ("Resolution".into(), "600dpi".into())]);
        let p = caps.paper(&a4, false);
        assert_eq!((p.width, p.height, p.dpi), (595.0, 842.0, (600.0, 600.0)));
        assert!((p.printable.x0 - 12.24).abs() < 1e-9 && (p.printable.y0 - 12.06).abs() < 1e-9);
        // no PPD (driverless queue): the size from the PWG name, quarter-inch margins
        let caps = Caps { options: parse_lpoptions(IPP), ..Default::default() };
        let p = caps.paper(&Options::default(), false);
        assert_eq!((p.width, p.height), (612.0, 792.0));
        assert_eq!(p.printable, Rect::new(18.0, 18.0, 594.0, 774.0));
        assert_eq!(p.dpi, (300.0, 300.0));
        // nothing known: Paper::fallback
        assert_eq!(Caps::default().paper(&Options::default(), true), Paper::fallback(true));
    }

    #[test]
    fn option_bytes_round_trip() {
        let o = Options(vec![("PageSize".into(), "A4".into()), ("Duplex".into(), "DuplexTumble".into()), ("print-color-mode".into(), "monochrome".into())]);
        let b = o.to_bytes();
        assert_eq!(String::from_utf8_lossy(&b), "PageSize=A4\nDuplex=DuplexTumble\nprint-color-mode=monochrome\n");
        assert_eq!(Options::from_bytes(Some(&b)), o);
        assert_eq!(Options::from_bytes(None), Options::default());
        // a Windows DEVMODE or junk gives no options
        assert_eq!(Options::from_bytes(Some(&[0u8, 1, 2, 255, 10, 61, 61])).0.len(), 0);
        let mut o = o;
        o.set("PageSize", "Letter");
        o.set("Resolution", "600dpi");
        assert_eq!(o.get("PageSize"), Some("Letter"));
        assert_eq!(o.0.len(), 4);
    }

    #[test]
    fn lp_command_line() {
        let o = Options(vec![("PageSize".into(), "A4".into())]);
        assert_eq!(lp_args("PDF", 2, false, "Logo", &o, "/run/user/1000/AnyVector/print.pdf"), ["-d", "PDF", "-n", "2", "-t", "Logo", "-o", "collate=false", "-o", "PageSize=A4", "/run/user/1000/AnyVector/print.pdf"]);
        assert_eq!(lp_args("PDF", 1, true, "", &Options::default(), "/tmp/a.pdf"), ["-d", "PDF", "-n", "1", "/tmp/a.pdf"]);
        assert_eq!(parse_lp_job("request id is PDF-3 (1 file(s))\n").as_deref(), Some("PDF-3"));
        assert_eq!(parse_lp_job(""), None);
    }

    #[test]
    fn captions() {
        let caps = Caps { ppd: parse_ppd(PPD), ..Default::default() };
        assert_eq!(paper_label(&caps, "Letter"), "US Letter (8.5 × 11 in)");
        assert_eq!(paper_label(&caps, "A4"), "A4 (210 × 297 mm)");
        assert_eq!(paper_label(&Caps::default(), "iso_a4_210x297mm"), "A4 (210 × 297 mm)");
        assert_eq!(paper_label(&Caps::default(), "na_letter_8.5x11in"), "Letter (8.5 × 11 in)");
        assert_eq!(paper_label(&Caps::default(), "Env10"), "Env10 (4.13 × 9.5 in)");
        assert_eq!(choice_label("Duplex", "DuplexNoTumble"), "Flip on long edge");
        assert_eq!(choice_label("sides", "one-sided"), "Off");
        assert_eq!(choice_label("ColorModel", "Gray"), "Grayscale");
        assert_eq!(choice_label("print-color-mode", "color"), "Color");
        assert_eq!(choice_label("Resolution", "1200x600dpi"), "1200 × 600 dpi");
        assert_eq!(choice_label("Resolution", "600dpi"), "600 dpi");
        assert_eq!(cups_server(None), "localhost:631");
        assert_eq!(cups_server(Some("/run/cups/cups.sock")), "localhost:631");
        assert_eq!(cups_server(Some("printhost")), "printhost:631");
        assert_eq!(cups_server(Some("printhost:8631")), "printhost:8631");
        assert_eq!(percent("Office Laser/x"), "Office%20Laser%2Fx");
    }
}
