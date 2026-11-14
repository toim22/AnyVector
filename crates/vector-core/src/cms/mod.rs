//! Color management: converting colors between RGB, CMYK and grayscale with the document's ICC
//! color profiles and rendering intent, showing them on screen, and soft proofing.
//!
//! The engine uses the ICC profile files installed on the computer (the Windows color folder,
//! CorelDRAW's color folder, or folders added with [`add_profile_dir`]), found by the profile's
//! description as the dialogs name it (e.g. "U.S. Web Coated (SWOP) v2") or by file name.
//! sRGB, Adobe RGB (1998), ProPhoto RGB and Display P3 are built in. A CMYK profile that is not
//! installed falls back to how CorelDRAW 2026 shows that profile on screen (sampled grids, see
//! [`grid`]); a grayscale "Dot Gain N%" profile that is not installed is modeled by its dot gain.
//!
//! With the color engine set to [`ColorEngine::None`] ("Simulate Color Management Off") colors
//! are shown and converted exactly by their numbers, with the plain device formulas.
//!
//! The monitor is taken to be sRGB.

pub mod grid;

use crate::color::Color;
use crate::document::{ColorSettings, RenderingIntent};
use moxcms::{ColorProfile, DataColorSpace, Layout, TransformF64Executor, TransformOptions};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

/// The color engine (Color Management Settings ▸ Default ▸ Color engine).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ColorEngine {
    /// No color management: colors are shown and converted by their numbers.
    None,
    /// The built-in ICC color engine.
    #[default]
    Builtin,
}

impl ColorEngine {
    pub const ALL: [ColorEngine; 2] = [ColorEngine::None, ColorEngine::Builtin];

    pub fn name(self) -> &'static str {
        match self {
            ColorEngine::None => "None",
            ColorEngine::Builtin => "AnyVector Color Engine",
        }
    }
}

/// Color conversion settings shared by all documents.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct CmsOptions {
    pub engine: ColorEngine,
    /// RGB black (and pure K) converts to 100% black ink only.
    pub preserve_pure_black: bool,
    /// Neutral grays convert to the black ink only.
    pub map_gray_to_black: bool,
}

impl Default for CmsOptions {
    fn default() -> Self {
        // CorelDRAW's factory settings (CRLCLR GetFactoryColorSettings: preserve pure black off,
        // map gray to CMYK black on)
        CmsOptions { engine: ColorEngine::Builtin, preserve_pure_black: false, map_gray_to_black: true }
    }
}

impl CmsOptions {
    /// Color management off: colors by their numbers.
    pub fn off(&self) -> bool {
        self.engine == ColorEngine::None
    }
}

fn intent_of(i: RenderingIntent) -> moxcms::RenderingIntent {
    match i {
        RenderingIntent::Perceptual => moxcms::RenderingIntent::Perceptual,
        RenderingIntent::RelativeColorimetric => moxcms::RenderingIntent::RelativeColorimetric,
        RenderingIntent::Saturation => moxcms::RenderingIntent::Saturation,
        RenderingIntent::AbsoluteColorimetric => moxcms::RenderingIntent::AbsoluteColorimetric,
    }
}

// ------------------------------------------------------------------ installed profiles

/// An ICC profile file found on the computer.
#[derive(Clone, Debug)]
pub struct InstalledProfile {
    pub path: PathBuf,
    /// The description stored in the profile ('desc' tag), the name the dialogs show.
    pub description: String,
    /// The color space: "RGB", "CMYK" or "Gray".
    pub space: &'static str,
    /// Device class: monitor ("mntr"), output ("prtr"), color space ("spac") or other.
    pub class: [u8; 4],
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

/// The description ('desc' tag) of an ICC profile.
pub fn icc_description(b: &[u8]) -> Option<String> {
    if b.len() < 132 || b.get(36..40)? != b"acsp" {
        return None;
    }
    let count = be32(b, 128)? as usize;
    for i in 0..count.min(200) {
        let e = 132 + i * 12;
        if b.get(e..e + 4)? != b"desc" {
            continue;
        }
        let off = be32(b, e + 4)? as usize;
        let len = be32(b, e + 8)? as usize;
        let tag = b.get(off..off + len)?;
        return match tag.get(0..4)? {
            b"desc" => {
                let n = be32(tag, 8)? as usize;
                let s = tag.get(12..12 + n)?;
                Some(String::from_utf8_lossy(s).trim_end_matches('\0').trim().to_string())
            }
            b"mluc" => {
                let rec_len = be32(tag, 20)? as usize;
                let rec_off = be32(tag, 24)? as usize;
                let s = tag.get(rec_off..rec_off + rec_len)?;
                let u: Vec<u16> = s.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                Some(String::from_utf16_lossy(&u).trim_end_matches('\0').trim().to_string())
            }
            _ => None,
        };
    }
    None
}

static EXTRA_DIRS: RwLock<Vec<PathBuf>> = RwLock::new(Vec::new());
static INSTALLED: RwLock<Option<Arc<Vec<InstalledProfile>>>> = RwLock::new(None);

/// Also look for profiles in `dir` (rescans the folders).
pub fn add_profile_dir(dir: PathBuf) {
    let mut d = EXTRA_DIRS.write();
    if !d.contains(&dir) {
        d.push(dir);
    }
    drop(d);
    *INSTALLED.write() = None;
    LOADED.lock().clear();
}

/// The folders searched for ICC profiles.
pub fn profile_dirs() -> Vec<PathBuf> {
    let mut v = vec![];
    if cfg!(windows) {
        let win = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        v.push(win.join("System32").join("spool").join("drivers").join("color"));
        // CorelDRAW's own profiles: <Program Files>\Corel\<suite>\<version>\Color
        for pf in [std::env::var_os("ProgramFiles"), std::env::var_os("ProgramFiles(x86)")].into_iter().flatten() {
            let corel = PathBuf::from(pf).join("Corel");
            for suite in std::fs::read_dir(&corel).into_iter().flatten().flatten() {
                for ver in std::fs::read_dir(suite.path()).into_iter().flatten().flatten() {
                    let c = ver.path().join("Color");
                    if c.is_dir() {
                        v.push(c);
                    }
                }
            }
        }
    } else {
        v.extend(["/usr/share/color/icc", "/usr/local/share/color/icc", "/Library/ColorSync/Profiles", "/System/Library/ColorSync/Profiles"].map(PathBuf::from));
        if let Some(h) = std::env::var_os("HOME") {
            v.push(PathBuf::from(h).join(".local/share/icc"));
        }
    }
    v.extend(EXTRA_DIRS.read().iter().cloned());
    v
}

/// Read the header and the description of a profile file without reading all of it.
fn probe(path: &Path) -> Option<InstalledProfile> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = vec![0u8; 132];
    f.read_exact(&mut head).ok()?;
    if &head[36..40] != b"acsp" {
        return None;
    }
    let count = (be32(&head, 128)? as usize).min(200);
    let mut table = vec![0u8; count * 12];
    f.read_exact(&mut table).ok()?;
    let space = match &head[16..20] {
        b"CMYK" => "CMYK",
        b"RGB " => "RGB",
        b"GRAY" => "Gray",
        _ => return None,
    };
    let mut class = [0u8; 4];
    class.copy_from_slice(&head[12..16]);
    let mut description = None;
    for i in 0..count {
        let e = i * 12;
        if &table[e..e + 4] != b"desc" {
            continue;
        }
        let off = be32(&table, e + 4)? as usize;
        let len = (be32(&table, e + 8)? as usize).min(1 << 16);
        f.seek(SeekFrom::Start(off as u64)).ok()?;
        let mut tag = vec![0u8; len];
        f.read_exact(&mut tag).ok()?;
        // reuse the parser: a fake profile with just this tag
        let mut fake = head.clone();
        fake[128..132].copy_from_slice(&1u32.to_be_bytes());
        fake.extend_from_slice(b"desc");
        fake.extend_from_slice(&144u32.to_be_bytes());
        fake.extend_from_slice(&(len as u32).to_be_bytes());
        fake.extend_from_slice(&tag);
        description = icc_description(&fake);
        break;
    }
    let description = description.filter(|d| !d.is_empty()).unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().to_string());
    Some(InstalledProfile { path: path.to_path_buf(), description, space, class })
}

/// The ICC profiles installed on the computer (scanned once).
pub fn installed_profiles() -> Arc<Vec<InstalledProfile>> {
    if let Some(v) = INSTALLED.read().as_ref() {
        return v.clone();
    }
    let mut v: Vec<InstalledProfile> = vec![];
    for dir in profile_dirs() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if ext != "icc" && ext != "icm" {
                continue;
            }
            if let Some(ip) = probe(&p) {
                if !v.iter().any(|x| x.description.eq_ignore_ascii_case(&ip.description) && x.space == ip.space) {
                    v.push(ip);
                }
            }
        }
    }
    v.sort_by(|a, b| a.space.cmp(b.space).then_with(|| a.description.to_lowercase().cmp(&b.description.to_lowercase())));
    let v = Arc::new(v);
    *INSTALLED.write() = Some(v.clone());
    v
}

/// The installed profile file for a profile name: its description, its file name, or a path.
pub fn find_profile(name: &str) -> Option<PathBuf> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let p = Path::new(name);
    if p.is_absolute() && p.is_file() {
        return Some(p.to_path_buf());
    }
    let list = installed_profiles();
    list.iter()
        .find(|x| x.description.eq_ignore_ascii_case(name))
        .or_else(|| list.iter().find(|x| x.path.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name))))
        .map(|x| x.path.clone())
}

type Loaded = Option<(Arc<ColorProfile>, Arc<Vec<u8>>)>;
static LOADED: Mutex<Vec<(String, Loaded)>> = Mutex::new(Vec::new());

thread_local! {
    static NO_FILES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run `f` as if no ICC profile file were installed (the built-in and sampled profiles only),
/// for results that do not depend on the computer.
pub fn without_installed_profiles<T>(f: impl FnOnce() -> T) -> T {
    let before = NO_FILES.with(|n| n.replace(true));
    let out = f();
    NO_FILES.with(|n| n.set(before));
    out
}

/// The ICC profile of a profile name, parsed, and its file data (cached).
pub fn load_profile(name: &str) -> Loaded {
    if NO_FILES.with(|n| n.get()) {
        return None;
    }
    if let Some((_, l)) = LOADED.lock().iter().find(|(n, _)| n == name) {
        return l.clone();
    }
    let l = find_profile(name).and_then(|p| std::fs::read(p).ok()).and_then(|b| ColorProfile::new_from_slice(&b).ok().map(|p| (Arc::new(p), Arc::new(b))));
    LOADED.lock().push((name.to_string(), l.clone()));
    l
}

/// The ICC data of a profile name, if it is installed.
pub fn profile_data(name: &str) -> Option<Arc<Vec<u8>>> {
    load_profile(name).map(|(_, b)| b)
}

// ------------------------------------------------------------------ profiles as the engine sees them

/// Where the engine took a profile from.
#[derive(Clone, Debug, PartialEq)]
pub enum ProfileSource {
    /// An ICC file on the computer.
    File(PathBuf),
    /// Built into the engine (sRGB, Adobe RGB (1998), ...).
    Builtin,
    /// The way CorelDRAW shows this CMYK profile, sampled (the ICC file is not installed).
    Sampled,
    /// Modeled from the profile's name (Dot Gain N%, Gray Gamma N).
    Modeled,
    /// Not found: the default profile of its color space is used instead.
    Substituted,
}

enum Prof {
    Icc(Arc<ColorProfile>),
    Grid(&'static [u8]),
    /// Grayscale ink with this dot gain at 50% (0.2 for "Dot Gain 20%").
    DotGain(f64),
}

impl Prof {
    fn channels(&self) -> usize {
        match self {
            Prof::Icc(p) => match p.color_space {
                DataColorSpace::Cmyk => 4,
                DataColorSpace::Gray => 1,
                _ => 3,
            },
            Prof::Grid(_) => 4,
            Prof::DotGain(_) => 1,
        }
    }
}

fn layout(channels: usize) -> Layout {
    match channels {
        1 => Layout::Gray,
        4 => Layout::Rgba,
        _ => Layout::Rgb,
    }
}

fn srgb_profile() -> Arc<ColorProfile> {
    static P: OnceLock<Arc<ColorProfile>> = OnceLock::new();
    P.get_or_init(|| Arc::new(ColorProfile::new_srgb())).clone()
}

fn builtin_rgb(name: &str) -> Option<ColorProfile> {
    let n = name.to_lowercase();
    if n.contains("srgb") {
        Some(ColorProfile::new_srgb())
    } else if n.starts_with("adobe rgb") || n.starts_with("adobergb") {
        Some(ColorProfile::new_adobe_rgb())
    } else if n.starts_with("prophoto") || n.contains("romm") {
        Some(ColorProfile::new_pro_photo_rgb())
    } else if n.contains("display p3") {
        Some(ColorProfile::new_display_p3())
    } else {
        None
    }
}

/// The number in a name like "Dot Gain 20%" or "Gray Gamma 2.2".
fn number_in(name: &str) -> Option<f64> {
    let s: String = name.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    s.parse().ok()
}

fn resolve_rgb(name: &str) -> (Prof, ProfileSource, bool) {
    if let Some(p) = builtin_rgb(name) {
        let srgb = name.to_lowercase().contains("srgb");
        return (Prof::Icc(if srgb { srgb_profile() } else { Arc::new(p) }), ProfileSource::Builtin, srgb);
    }
    if let Some((p, _)) = load_profile(name).filter(|(p, _)| p.color_space == DataColorSpace::Rgb) {
        return (Prof::Icc(p), ProfileSource::File(find_profile(name).unwrap_or_default()), false);
    }
    (Prof::Icc(srgb_profile()), ProfileSource::Substituted, true)
}

fn resolve_cmyk(name: &str) -> (Prof, ProfileSource) {
    if let Some((p, _)) = load_profile(name).filter(|(p, _)| p.color_space == DataColorSpace::Cmyk) {
        return (Prof::Icc(p), ProfileSource::File(find_profile(name).unwrap_or_default()));
    }
    match grid::profile_grid(name) {
        Some(g) => (Prof::Grid(g), ProfileSource::Sampled),
        None => (Prof::Grid(grid::DEFAULT_GRID), ProfileSource::Substituted),
    }
}

fn resolve_gray(name: &str) -> (Prof, ProfileSource) {
    if let Some((p, _)) = load_profile(name).filter(|(p, _)| p.color_space == DataColorSpace::Gray) {
        return (Prof::Icc(p), ProfileSource::File(find_profile(name).unwrap_or_default()));
    }
    let lower = name.to_lowercase();
    if lower.starts_with("gray gamma") {
        let g = number_in(name).map(|g| if g >= 10.0 { g / 10.0 } else { g }).filter(|g| *g > 0.1).unwrap_or(2.2);
        return (Prof::Icc(Arc::new(ColorProfile::new_gray_with_gamma(g as f32))), ProfileSource::Modeled);
    }
    if lower.starts_with("dot gain") {
        let d = number_in(name).unwrap_or(20.0) / 100.0;
        return (Prof::DotGain(d.clamp(0.0, 0.5)), ProfileSource::Modeled);
    }
    (Prof::DotGain(0.2), ProfileSource::Substituted)
}

/// Dot gain: gray level (1 white) to the L* it prints with.
fn dot_gain_to_l(d: f64, gray: f64) -> f64 {
    let t = (1.0 - gray).clamp(0.0, 1.0);
    let printed = (t + 4.0 * d * t * (1.0 - t)).clamp(0.0, 1.0);
    let y: f64 = 1.0 - printed;
    if y > 216.0 / 24389.0 {
        116.0 * y.cbrt() - 16.0
    } else {
        y * 24389.0 / 27.0
    }
}

fn dot_gain_from_l(d: f64, l: f64) -> f64 {
    let fy = (l.clamp(0.0, 100.0) + 16.0) / 116.0;
    let y = if fy * fy * fy > 216.0 / 24389.0 { fy * fy * fy } else { l.clamp(0.0, 100.0) * 27.0 / 24389.0 };
    let printed = 1.0 - y;
    let t = if d <= 1e-9 {
        printed
    } else {
        let a = 4.0 * d;
        ((1.0 + a) - ((1.0 + a) * (1.0 + a) - 4.0 * a * printed).max(0.0).sqrt()) / (2.0 * a)
    };
    1.0 - t.clamp(0.0, 1.0)
}

// ------------------------------------------------------------------ the engine

type Xf = Arc<TransformF64Executor>;

/// Converts colors for one set of document color settings and conversion options.
pub struct Engine {
    pub settings: ColorSettings,
    pub options: CmsOptions,
    rgb: Prof,
    cmyk: Prof,
    gray: Prof,
    srgb: Prof,
    rgb_is_srgb: bool,
    sources: [ProfileSource; 3],
    xforms: Mutex<HashMap<(u8, u8, u8), Option<Xf>>>,
    memo_cmyk_rgb: Mutex<HashMap<[u16; 4], [u8; 3]>>,
    memo_cmyk_disp: Mutex<HashMap<[u16; 4], [u8; 3]>>,
    memo_rgb_cmyk: Mutex<HashMap<[u8; 3], [f64; 4]>>,
    memo_rgb_disp: Mutex<HashMap<[u8; 3], [u8; 3]>>,
    k_ramp: OnceLock<Vec<f64>>,
    proofs: Mutex<Vec<(String, u8, Arc<ProofLut>)>>,
}

const RGB: u8 = 0;
const CMYK: u8 = 1;
const GRAY: u8 = 2;
const SRGB: u8 = 3;

fn q16(v: f64) -> u16 {
    (v.clamp(0.0, 100.0) * 100.0).round() as u16
}

fn naive_cmyk_to_rgb(c: [f64; 4]) -> [u8; 3] {
    let [c, m, y, k] = c.map(|v| (v / 100.0).clamp(0.0, 1.0));
    let f = |x: f64| ((1.0 - x) * (1.0 - k) * 255.0).round().clamp(0.0, 255.0) as u8;
    [f(c), f(m), f(y)]
}

/// Plain (device) CMYK of an RGB color, values 0..100.
pub fn naive_rgb_to_cmyk(c: Color) -> [f64; 4] {
    let r = c.r as f64 / 255.0;
    let g = c.g as f64 / 255.0;
    let b = c.b as f64 / 255.0;
    let k = 1.0 - r.max(g).max(b);
    if k >= 0.9999 {
        return [0.0, 0.0, 0.0, 100.0];
    }
    [(1.0 - r - k) / (1.0 - k) * 100.0, (1.0 - g - k) / (1.0 - k) * 100.0, (1.0 - b - k) / (1.0 - k) * 100.0, k * 100.0]
}

fn to_u8(v: [f64; 3]) -> [u8; 3] {
    v.map(|x| (x * 255.0).round().clamp(0.0, 255.0) as u8)
}

impl Engine {
    pub fn new(settings: &ColorSettings, options: &CmsOptions) -> Engine {
        let (rgb, rs, rgb_is_srgb) = resolve_rgb(&settings.rgb_profile);
        let (cmyk, cs) = resolve_cmyk(&settings.cmyk_profile);
        let (gray, gs) = resolve_gray(&settings.gray_profile);
        Engine {
            settings: settings.clone(),
            options: options.clone(),
            rgb,
            cmyk,
            gray,
            srgb: Prof::Icc(srgb_profile()),
            rgb_is_srgb,
            sources: [rs, cs, gs],
            xforms: Mutex::new(HashMap::new()),
            memo_cmyk_rgb: Mutex::new(HashMap::new()),
            memo_cmyk_disp: Mutex::new(HashMap::new()),
            memo_rgb_cmyk: Mutex::new(HashMap::new()),
            memo_rgb_disp: Mutex::new(HashMap::new()),
            k_ramp: OnceLock::new(),
            proofs: Mutex::new(vec![]),
        }
    }

    /// Color management is off: numbers as they are.
    pub fn off(&self) -> bool {
        self.options.off()
    }

    /// Where the RGB, CMYK and grayscale profiles came from.
    pub fn sources(&self) -> &[ProfileSource; 3] {
        &self.sources
    }

    /// The ICC data of the document's CMYK profile, when it is an installed file.
    pub fn cmyk_icc(&self) -> Option<Arc<Vec<u8>>> {
        match self.sources[1] {
            ProfileSource::File(_) => profile_data(&self.settings.cmyk_profile),
            _ => None,
        }
    }

    fn prof(&self, id: u8) -> &Prof {
        match id {
            RGB => &self.rgb,
            CMYK => &self.cmyk,
            GRAY => &self.gray,
            _ => &self.srgb,
        }
    }

    fn intent(&self) -> RenderingIntent {
        self.settings.intent
    }

    fn xform(&self, src: u8, dst: u8, intent: RenderingIntent) -> Option<Xf> {
        let key = (src, dst, intent as u8);
        if let Some(x) = self.xforms.lock().get(&key) {
            return x.clone();
        }
        let x = match (self.prof(src), self.prof(dst)) {
            (Prof::Icc(a), Prof::Icc(b)) => icc_xform(a, b, intent),
            _ => None,
        };
        self.xforms.lock().insert(key, x.clone());
        x
    }

    /// Device values of `src` (each 0..1) to sRGB (each 0..1).
    fn to_srgb(&self, src: u8, v: &[f64], intent: RenderingIntent) -> [f64; 3] {
        if src == SRGB {
            return [v[0], v[1], v[2]];
        }
        match self.prof(src) {
            Prof::Grid(g) => grid::sample(g, [v[0], v[1], v[2], v[3]]).map(|x| x / 255.0),
            Prof::DotGain(d) => grid::lab_to_srgb([dot_gain_to_l(*d, v[0]), 0.0, 0.0]),
            Prof::Icc(_) => {
                let out = run(self.xform(src, SRGB, intent).as_ref(), v, 3);
                [out[0], out[1], out[2]]
            }
        }
    }

    /// sRGB (each 0..1) to device values of `dst`.
    fn from_srgb(&self, dst: u8, rgb: [f64; 3], intent: RenderingIntent) -> Vec<f64> {
        if dst == SRGB {
            return rgb.to_vec();
        }
        match self.prof(dst) {
            Prof::Grid(g) => grid::invert(g, rgb).to_vec(),
            Prof::DotGain(d) => vec![dot_gain_from_l(*d, grid::srgb_to_lab(rgb)[0])],
            Prof::Icc(_) => run(self.xform(SRGB, dst, intent).as_ref(), &rgb, self.prof(dst).channels()),
        }
    }

    /// Device values of `src` to device values of `dst` (each 0..1).
    fn convert(&self, src: u8, v: &[f64], dst: u8, intent: RenderingIntent) -> Vec<f64> {
        if src == dst {
            return v.to_vec();
        }
        if let (Prof::Icc(_), Prof::Icc(_)) = (self.prof(src), self.prof(dst)) {
            if let Some(x) = self.xform(src, dst, intent) {
                return run(Some(&x), v, self.prof(dst).channels());
            }
        }
        let s = self.to_srgb(src, v, intent);
        self.from_srgb(dst, s, intent)
    }

    /// The RGB (in the document RGB profile) of CMYK inks (each 0..100).
    pub fn cmyk_to_rgb(&self, cmyk: [f64; 4]) -> Color {
        if self.off() {
            let [r, g, b] = naive_cmyk_to_rgb(cmyk);
            return Color::rgb(r, g, b);
        }
        let key = cmyk.map(q16);
        if let Some(c) = self.memo_cmyk_rgb.lock().get(&key) {
            return Color::rgb(c[0], c[1], c[2]);
        }
        let v = self.convert(CMYK, &cmyk.map(|x| (x / 100.0).clamp(0.0, 1.0)), RGB, self.intent());
        let c = to_u8([v[0], v[1], v[2]]);
        let mut m = self.memo_cmyk_rgb.lock();
        if m.len() > 65536 {
            m.clear();
        }
        m.insert(key, c);
        Color::rgb(c[0], c[1], c[2])
    }

    /// How CMYK inks (each 0..100) look on screen.
    pub fn cmyk_to_display(&self, cmyk: [f64; 4]) -> Color {
        if self.off() {
            let [r, g, b] = naive_cmyk_to_rgb(cmyk);
            return Color::rgb(r, g, b);
        }
        let key = cmyk.map(q16);
        if let Some(c) = self.memo_cmyk_disp.lock().get(&key) {
            return Color::rgb(c[0], c[1], c[2]);
        }
        let c = to_u8(self.to_srgb(CMYK, &cmyk.map(|x| (x / 100.0).clamp(0.0, 1.0)), self.intent()));
        let mut m = self.memo_cmyk_disp.lock();
        if m.len() > 65536 {
            m.clear();
        }
        m.insert(key, c);
        Color::rgb(c[0], c[1], c[2])
    }

    /// How an RGB color (in the document RGB profile) looks on screen.
    pub fn rgb_to_display(&self, c: Color) -> Color {
        if self.off() || self.rgb_is_srgb {
            return Color::rgba(c.r, c.g, c.b, c.a);
        }
        let key = [c.r, c.g, c.b];
        if let Some(d) = self.memo_rgb_disp.lock().get(&key) {
            return Color::rgba(d[0], d[1], d[2], c.a);
        }
        let d = to_u8(self.to_srgb(RGB, &key.map(|v| v as f64 / 255.0), self.intent()));
        self.memo_rgb_disp.lock().insert(key, d);
        Color::rgba(d[0], d[1], d[2], c.a)
    }

    /// The document RGB of a screen (sRGB) color.
    pub fn display_to_rgb(&self, c: Color) -> Color {
        if self.off() || self.rgb_is_srgb {
            return Color::rgba(c.r, c.g, c.b, c.a);
        }
        let v = self.from_srgb(RGB, [c.r, c.g, c.b].map(|v| v as f64 / 255.0), self.intent());
        let d = to_u8([v[0], v[1], v[2]]);
        Color::rgba(d[0], d[1], d[2], c.a)
    }

    /// Whether RGB colors need converting to be shown (the document RGB profile is not sRGB).
    pub fn rgb_needs_display_conversion(&self) -> bool {
        !self.off() && !self.rgb_is_srgb
    }

    /// The black ink (0..100) alone that looks like `l` (L*), through the CMYK profile.
    fn black_for_l(&self, l: f64) -> f64 {
        let ramp = self.k_ramp.get_or_init(|| (0..=100).map(|k| grid::srgb_to_lab(self.to_srgb(CMYK, &[0.0, 0.0, 0.0, k as f64 / 100.0], self.intent()))[0]).collect());
        let (white, black) = (ramp[0], ramp[100]);
        if l >= white {
            return 0.0;
        }
        if l <= black {
            return 100.0;
        }
        for k in 0..100 {
            let (a, b) = (ramp[k], ramp[k + 1]);
            if l <= a && l >= b {
                let t = if (a - b).abs() < 1e-9 { 0.0 } else { (a - l) / (a - b) };
                return k as f64 + t;
            }
        }
        100.0
    }

    /// The CMYK inks (each 0..100) of an RGB color (in the document RGB profile).
    pub fn rgb_to_cmyk(&self, c: Color) -> [f64; 4] {
        if self.off() {
            return naive_rgb_to_cmyk(c);
        }
        let key = [c.r, c.g, c.b];
        if key == [255, 255, 255] {
            return [0.0; 4];
        }
        if self.options.preserve_pure_black && key == [0, 0, 0] {
            return [0.0, 0.0, 0.0, 100.0];
        }
        if let Some(v) = self.memo_rgb_cmyk.lock().get(&key) {
            return *v;
        }
        let out = if self.options.map_gray_to_black && c.r == c.g && c.g == c.b {
            let s = self.to_srgb(RGB, &key.map(|v| v as f64 / 255.0), self.intent());
            [0.0, 0.0, 0.0, self.black_for_l(grid::srgb_to_lab(s)[0])]
        } else {
            let v = self.convert(RGB, &key.map(|v| v as f64 / 255.0), CMYK, self.intent());
            [v[0], v[1], v[2], v[3]].map(|x| (x * 100.0).clamp(0.0, 100.0))
        };
        let mut m = self.memo_rgb_cmyk.lock();
        if m.len() > 65536 {
            m.clear();
        }
        m.insert(key, out);
        out
    }

    /// The RGB of a gray level (0 black .. 255 white in the grayscale profile).
    pub fn gray_to_rgb(&self, level: f64) -> Color {
        let g = (level / 255.0).clamp(0.0, 1.0);
        if self.off() {
            let v = (g * 255.0).round() as u8;
            return Color::rgb(v, v, v);
        }
        let v = self.convert(GRAY, &[g], RGB, self.intent());
        let c = to_u8([v[0], v[1], v[2]]);
        Color::rgb(c[0], c[1], c[2])
    }

    /// Bitmap pixels (RGB, 3 bytes each, document RGB profile) to CMYK (4 bytes each, 0..255),
    /// converted in bulk.
    pub fn rgb_pixels_to_cmyk(&self, rgb: &[u8]) -> Vec<u8> {
        let n = rgb.len() / 3;
        let mut out = Vec::with_capacity(n * 4);
        let to8 = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
        if !self.off() {
            if let Some(x) = self.xform(RGB, CMYK, self.intent()) {
                const CHUNK: usize = 16384;
                let mut dst = vec![0.0f64; CHUNK * 4];
                for px in rgb[..n * 3].chunks(CHUNK * 3) {
                    let src: Vec<f64> = px.iter().map(|v| *v as f64 / 255.0).collect();
                    let d = &mut dst[..px.len() / 3 * 4];
                    if x.transform(&src, d).is_err() {
                        out.clear();
                        break;
                    }
                    out.extend(d.iter().map(|v| to8(*v)));
                }
                if out.len() == n * 4 {
                    return out;
                }
                out.clear();
            }
        }
        // no ICC transform: color by color (remembered)
        let mut seen: HashMap<[u8; 3], [u8; 4]> = HashMap::new();
        for px in rgb[..n * 3].chunks_exact(3) {
            let key = [px[0], px[1], px[2]];
            let v = *seen.entry(key).or_insert_with(|| {
                let c = if self.off() { naive_rgb_to_cmyk(Color::rgb(px[0], px[1], px[2])) } else { self.rgb_to_cmyk_plain(Color::rgb(px[0], px[1], px[2])) };
                c.map(|v| to8(v / 100.0))
            });
            out.extend_from_slice(&v);
        }
        out
    }

    /// RGB to CMYK without the pure black and gray rules (bitmaps).
    fn rgb_to_cmyk_plain(&self, c: Color) -> [f64; 4] {
        let v = self.convert(RGB, &[c.r, c.g, c.b].map(|v| v as f64 / 255.0), CMYK, self.intent());
        [v[0], v[1], v[2], v[3]].map(|x| (x * 100.0).clamp(0.0, 100.0))
    }

    /// The gray level (0..255 in the grayscale profile) of an RGB color.
    pub fn rgb_to_gray(&self, c: Color) -> f64 {
        if self.off() {
            return 0.299 * c.r as f64 + 0.587 * c.g as f64 + 0.114 * c.b as f64;
        }
        let v = self.convert(RGB, &[c.r, c.g, c.b].map(|v| v as f64 / 255.0), GRAY, self.intent());
        (v[0] * 255.0).clamp(0.0, 255.0)
    }

    /// The gray level (0..255) of CMYK inks.
    pub fn cmyk_to_gray(&self, cmyk: [f64; 4]) -> f64 {
        if self.off() {
            let [r, g, b] = naive_cmyk_to_rgb(cmyk);
            return 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
        }
        let v = self.convert(CMYK, &cmyk.map(|x| (x / 100.0).clamp(0.0, 1.0)), GRAY, self.intent());
        (v[0] * 255.0).clamp(0.0, 255.0)
    }

    /// CMYK inks of this engine's CMYK profile to those of `other`'s, keeping the look
    /// (Convert, preserve color appearance).
    pub fn cmyk_to_other(&self, cmyk: [f64; 4], other: &Engine) -> [f64; 4] {
        if self.off() || other.off() || self.settings.cmyk_profile == other.settings.cmyk_profile {
            return cmyk;
        }
        if self.options.preserve_pure_black && cmyk[0] == 0.0 && cmyk[1] == 0.0 && cmyk[2] == 0.0 {
            return cmyk;
        }
        let v = cmyk.map(|x| (x / 100.0).clamp(0.0, 1.0));
        let out = match (&self.cmyk, &other.cmyk) {
            (Prof::Icc(a), Prof::Icc(b)) => icc_xform(a, b, other.intent()).map(|x| run(Some(&x), &v, 4)),
            _ => None,
        }
        .unwrap_or_else(|| other.from_srgb(CMYK, self.to_srgb(CMYK, &v, self.intent()), other.intent()));
        [out[0], out[1], out[2], out[3]].map(|x| (x * 100.0).clamp(0.0, 100.0))
    }

    /// An RGB color of this engine's RGB profile in `other`'s RGB profile, keeping the look.
    pub fn rgb_to_other(&self, c: Color, other: &Engine) -> Color {
        if self.off() || other.off() || self.settings.rgb_profile == other.settings.rgb_profile {
            return c;
        }
        let v = [c.r, c.g, c.b].map(|v| v as f64 / 255.0);
        let out = match (&self.rgb, &other.rgb) {
            (Prof::Icc(a), Prof::Icc(b)) => icc_xform(a, b, other.intent()).map(|x| run(Some(&x), &v, 3)),
            _ => None,
        }
        .unwrap_or_else(|| other.from_srgb(RGB, self.to_srgb(RGB, &v, self.intent()), other.intent()));
        let d = to_u8([out[0], out[1], out[2]]);
        Color::rgba(d[0], d[1], d[2], c.a)
    }

    /// The soft proof of screen colors through the named profile (an installed profile's
    /// description, or the document's or default profile names), with a rendering intent.
    pub fn proof(&self, env: &str, intent: RenderingIntent) -> Arc<ProofLut> {
        let key = (env.to_string(), intent as u8);
        if let Some((_, _, l)) = self.proofs.lock().iter().find(|(n, i, _)| *n == key.0 && *i == key.1) {
            return l.clone();
        }
        let lut = Arc::new(ProofLut::build(self, env, intent));
        let mut p = self.proofs.lock();
        if p.len() > 8 {
            p.remove(0);
        }
        p.push((key.0, key.1, lut.clone()));
        lut
    }
}

fn icc_xform(a: &ColorProfile, b: &ColorProfile, intent: RenderingIntent) -> Option<Xf> {
    let ch = |p: &ColorProfile| match p.color_space {
        DataColorSpace::Cmyk => 4,
        DataColorSpace::Gray => 1,
        _ => 3,
    };
    let opts = TransformOptions { rendering_intent: intent_of(intent), ..Default::default() };
    // a profile without the intent's tables falls back to its default one
    a.create_transform_f64(layout(ch(a)), b, layout(ch(b)), opts)
        .or_else(|_| a.create_transform_f64(layout(ch(a)), b, layout(ch(b)), TransformOptions { rendering_intent: moxcms::RenderingIntent::RelativeColorimetric, ..Default::default() }))
        .or_else(|_| a.create_transform_f64(layout(ch(a)), b, layout(ch(b)), TransformOptions { rendering_intent: moxcms::RenderingIntent::Perceptual, ..Default::default() }))
        .ok()
}

/// Run a transform over one pixel's values (each 0..1), clamping the result; without a
/// transform the values pass through as far as they fit.
fn run(x: Option<&Xf>, src: &[f64], out_channels: usize) -> Vec<f64> {
    let mut dst = vec![0.0f64; out_channels];
    match x {
        Some(x) if x.transform(src, &mut dst).is_ok() => dst.iter().map(|v| v.clamp(0.0, 1.0)).collect(),
        _ => (0..out_channels).map(|i| src.get(i).copied().unwrap_or(0.0)).collect(),
    }
}

/// Batch transform of many pixels.
fn run_batch(x: Option<&Xf>, src: &[f64], out: &mut [f64]) -> bool {
    match x {
        Some(x) => x.transform(src, out).is_ok(),
        None => false,
    }
}

// ------------------------------------------------------------------ soft proofing

/// Screen colors through a proofing environment and back to the screen, sampled on a grid.
pub struct ProofLut {
    n: usize,
    /// Proofed sRGB (0..255) of each sample.
    rgb: Vec<[f32; 3]>,
    /// Color difference (ΔE) between each sample and what the environment can print.
    delta: Vec<f32>,
}

impl ProofLut {
    const N: usize = 17;

    fn build(e: &Engine, env: &str, intent: RenderingIntent) -> ProofLut {
        let n = Self::N;
        let pts: Vec<[f64; 3]> = (0..n * n * n).map(|i| [i / (n * n), (i / n) % n, i % n].map(|v| v as f64 / (n - 1) as f64)).collect();
        let mut rgb = vec![[0f32; 3]; pts.len()];
        let mut delta = vec![0f32; pts.len()];
        if e.off() {
            for (i, p) in pts.iter().enumerate() {
                rgb[i] = p.map(|v| (v * 255.0) as f32);
            }
            return ProofLut { n, rgb, delta };
        }
        // the environment: an installed profile, a sampled CMYK profile, or a modeled gray one
        let target: Prof = match load_profile(env) {
            Some((p, _)) => Prof::Icc(p),
            None => {
                if let Some(g) = grid::profile_grid(env) {
                    Prof::Grid(g)
                } else if let Some(p) = builtin_rgb(env) {
                    Prof::Icc(Arc::new(p))
                } else {
                    match resolve_gray(env) {
                        (p, ProfileSource::Modeled) => p,
                        _ => Prof::Grid(grid::DEFAULT_GRID),
                    }
                }
            }
        };
        let flat: Vec<f64> = pts.iter().flat_map(|p| p.iter().copied()).collect();
        let ch = target.channels();
        // screen -> environment
        let dev: Vec<f64> = match &target {
            Prof::Icc(p) => {
                let mut out = vec![0.0; pts.len() * ch];
                if run_batch(icc_xform(&srgb_profile(), p, intent).as_ref(), &flat, &mut out) {
                    out
                } else {
                    pts.iter().flat_map(|p| p.iter().copied().take(ch).chain(std::iter::repeat(0.0)).take(ch)).collect()
                }
            }
            Prof::Grid(g) => pts.iter().flat_map(|p| grid::invert(g, *p)).collect(),
            Prof::DotGain(d) => pts.iter().map(|p| dot_gain_from_l(*d, grid::srgb_to_lab(*p)[0])).collect(),
        };
        // environment -> screen: absolute colorimetric shows the paper white
        let back_intent = if intent == RenderingIntent::AbsoluteColorimetric { RenderingIntent::AbsoluteColorimetric } else { RenderingIntent::RelativeColorimetric };
        let back: Vec<f64> = match &target {
            Prof::Icc(p) => {
                let mut out = vec![0.0; pts.len() * 3];
                if run_batch(icc_xform(p, &srgb_profile(), back_intent).as_ref(), &dev, &mut out) {
                    out
                } else {
                    flat.clone()
                }
            }
            Prof::Grid(g) => dev.chunks(4).flat_map(|c| grid::sample(g, [c[0], c[1], c[2], c[3]]).map(|v| v / 255.0)).collect(),
            Prof::DotGain(d) => dev.iter().flat_map(|g| grid::lab_to_srgb([dot_gain_to_l(*d, *g), 0.0, 0.0])).collect(),
        };
        // the gamut: what relative colorimetric printing loses
        let rel: Vec<f64> = match &target {
            Prof::Icc(p) if intent != RenderingIntent::RelativeColorimetric => {
                let mut d1 = vec![0.0; pts.len() * ch];
                let mut out = vec![0.0; pts.len() * 3];
                if run_batch(icc_xform(&srgb_profile(), p, RenderingIntent::RelativeColorimetric).as_ref(), &flat, &mut d1) && run_batch(icc_xform(p, &srgb_profile(), RenderingIntent::RelativeColorimetric).as_ref(), &d1, &mut out) {
                    out
                } else {
                    back.clone()
                }
            }
            _ => back.clone(),
        };
        for (i, p) in pts.iter().enumerate() {
            let b = [back[i * 3], back[i * 3 + 1], back[i * 3 + 2]].map(|v| v.clamp(0.0, 1.0));
            rgb[i] = b.map(|v| (v * 255.0) as f32);
            let r = [rel[i * 3], rel[i * 3 + 1], rel[i * 3 + 2]].map(|v| v.clamp(0.0, 1.0));
            let (l1, l2) = (grid::srgb_to_lab(*p), grid::srgb_to_lab(r));
            delta[i] = ((l1[0] - l2[0]).powi(2) + (l1[1] - l2[1]).powi(2) + (l1[2] - l2[2]).powi(2)).sqrt() as f32;
        }
        ProofLut { n, rgb, delta }
    }

    /// The proofed screen color (each 0..255) of a screen color, and how far it is from what
    /// the environment can print (ΔE; out of gamut above about 5).
    pub fn apply(&self, rgb: [f32; 3]) -> ([f32; 3], f32) {
        let n = self.n;
        let s = (n - 1) as f32;
        let p = rgb.map(|v| (v / 255.0).clamp(0.0, 1.0) * s);
        let lo = p.map(|v| (v.floor() as usize).min(n - 2));
        let t = [0, 1, 2].map(|i| p[i] - lo[i] as f32);
        let mut out = [0f32; 3];
        let mut d = 0f32;
        for corner in 0..8usize {
            let mut w = 1.0;
            let mut idx = 0usize;
            for i in 0..3 {
                let hi = (corner >> (2 - i)) & 1;
                w *= if hi == 1 { t[i] } else { 1.0 - t[i] };
                idx = idx * n + lo[i] + hi;
            }
            if w > 0.0 {
                for (o, v) in out.iter_mut().zip(self.rgb[idx]) {
                    *o += w * v;
                }
                d += w * self.delta[idx];
            }
        }
        (out, d)
    }
}

// ------------------------------------------------------------------ the engine in use

static OPTIONS: RwLock<Option<CmsOptions>> = RwLock::new(None);
static ENGINES: Mutex<Vec<Arc<Engine>>> = Mutex::new(Vec::new());
static ACTIVE: RwLock<Option<Arc<Engine>>> = RwLock::new(None);
/// An engine is active: CMYK colors are shown through it.
static SHOWING: AtomicBool = AtomicBool::new(false);
/// RGB colors are converted to be shown too.
static SHOWING_RGB: AtomicBool = AtomicBool::new(false);

thread_local! {
    static OVERRIDE: RefCell<Option<Arc<Engine>>> = const { RefCell::new(None) };
}

/// The color conversion options in force (Color Management Settings).
pub fn options() -> CmsOptions {
    OPTIONS.read().clone().unwrap_or_default()
}

/// Set the color conversion options (the active engine follows on the next [`activate`]).
pub fn set_options(o: &CmsOptions) {
    let changed = OPTIONS.read().as_ref() != Some(o);
    if changed {
        *OPTIONS.write() = Some(o.clone());
        let active = ACTIVE.read().as_ref().map(|e| e.settings.clone());
        if let Some(cs) = active {
            activate(&cs);
        }
    }
}

/// The engine for document color settings with the current options (cached).
pub fn engine(cs: &ColorSettings) -> Arc<Engine> {
    engine_with(cs, &options())
}

/// The engine for document color settings and options (cached).
pub fn engine_with(cs: &ColorSettings, o: &CmsOptions) -> Arc<Engine> {
    if NO_FILES.with(|n| n.get()) {
        return Arc::new(Engine::new(cs, o));
    }
    let mut list = ENGINES.lock();
    if let Some(e) = list.iter().find(|e| e.settings == *cs && e.options == *o) {
        return e.clone();
    }
    let e = Arc::new(Engine::new(cs, o));
    if list.len() >= 8 {
        list.remove(0);
    }
    list.push(e.clone());
    e
}

/// Make the color settings of the document being worked on the ones colors are shown and
/// converted with (cheap when nothing changed).
pub fn activate(cs: &ColorSettings) {
    let o = options();
    if ACTIVE.read().as_ref().is_some_and(|e| e.settings == *cs && e.options == o) {
        return;
    }
    let e = engine_with(cs, &o);
    SHOWING_RGB.store(e.rgb_needs_display_conversion(), Ordering::Relaxed);
    *ACTIVE.write() = Some(e);
    SHOWING.store(true, Ordering::Relaxed);
}

/// Stop showing colors through an engine (no document).
pub fn deactivate() {
    *ACTIVE.write() = None;
    SHOWING.store(false, Ordering::Relaxed);
    SHOWING_RGB.store(false, Ordering::Relaxed);
}

/// The engine in force: the one of [`with_settings`] on this thread, the active one, or the
/// one of the default color settings.
pub fn current() -> Arc<Engine> {
    if let Some(e) = OVERRIDE.with(|o| o.borrow().clone()) {
        return e;
    }
    if let Some(e) = ACTIVE.read().as_ref() {
        return e.clone();
    }
    engine(&ColorSettings::default())
}

/// Run `f` with colors converted through `cs` on this thread (export of a document).
pub fn with_settings<T>(cs: &ColorSettings, f: impl FnOnce() -> T) -> T {
    let e = engine(cs);
    let before = OVERRIDE.with(|o| o.replace(Some(e)));
    let out = f();
    OVERRIDE.with(|o| *o.borrow_mut() = before);
    out
}

/// How a color looks on screen: CMYK inks through the CMYK profile (or by their numbers with
/// color management off), RGB through the RGB profile. Without an active engine (no document,
/// tests) colors show as stored.
pub fn display(c: Color) -> Color {
    let overridden = OVERRIDE.with(|o| o.borrow().is_some());
    if !overridden && !SHOWING.load(Ordering::Relaxed) {
        return c;
    }
    match c.cmyk {
        Some(k) => {
            let d = current().cmyk_to_display(k.map(|v| v as f64));
            Color::rgba(d.r, d.g, d.b, c.a)
        }
        None if overridden || SHOWING_RGB.load(Ordering::Relaxed) => current().rgb_to_display(c),
        None => c,
    }
}

/// Registered print characterizations (ICC registry) of well-known CMYK profiles, for PDF/X
/// output intents: (OutputConditionIdentifier, OutputCondition).
pub fn output_condition(profile: &str) -> Option<(&'static str, &'static str)> {
    let p = profile.to_lowercase();
    let table: [(&str, &str, &str); 14] = [
        ("u.s. web coated (swop) v2", "CGATS TR 001", "SWOP (Publication) printing in North America"),
        ("swop proofer cmyk", "CGATS TR 001", "SWOP (Publication) printing in North America"),
        ("web coated swop 2006 grade 3", "CGATS TR 003", "Web offset, grade 3 paper"),
        ("web coated swop 2006 grade 5", "CGATS TR 005", "Web offset, grade 5 paper"),
        ("coated gracol 2006", "CGATS TR 006", "Sheetfed offset, grade 1 paper"),
        ("coated fogra39", "FOGRA39", "Offset commercial and specialty printing, paper type 1 or 2"),
        ("iso coated v2 (eci)", "FOGRA39", "Offset commercial and specialty printing, paper type 1 or 2"),
        ("iso coated v2 300%", "FOGRA39", "Offset commercial and specialty printing, paper type 1 or 2"),
        ("coated fogra27", "FOGRA27", "Offset commercial and specialty printing, paper type 1 or 2"),
        ("uncoated fogra29", "FOGRA29", "Offset commercial and specialty printing, paper type 4"),
        ("web coated fogra28", "FOGRA28", "Offset commercial and specialty printing, paper type 3"),
        ("japan color 2001 coated", "JC200103", "Japan Color 2001 Coated"),
        ("japan color 2001 uncoated", "JC200104", "Japan Color 2001 Uncoated"),
        ("japan color 2003 web coated", "JCW2003", "Japan Color 2003 Web Coated"),
    ];
    table.iter().find(|(n, _, _)| p.starts_with(n)).map(|(_, id, c)| (*id, *c))
}

#[cfg(test)]
mod tests;
