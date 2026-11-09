//! The pixel work of the Bitmaps menu: color modes (1-bit, grayscale, duotone, paletted),
//! Straighten Image, Image Adjustment Lab, Remove JPEG Artifacts, Remove Background, Crop
//! Bitmap and Inflate Bitmap. Every function takes straight (not premultiplied) RGBA pixels.

use std::sync::Arc;
use vector_core::document::ImageData;
use vector_core::object::BitmapEffect;
use vector_core::Color;

/// Pixels of a bitmap being worked on.
#[derive(Clone)]
pub struct Px {
    pub w: usize,
    pub h: usize,
    pub p: Vec<[u8; 4]>,
}

impl Px {
    pub fn of(img: &ImageData) -> Px {
        let p = img.rgba.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
        Px { w: img.width as usize, h: img.height as usize, p }
    }

    pub fn new(w: usize, h: usize) -> Px {
        Px { w, h, p: vec![[0; 4]; w * h] }
    }

    /// The image data of these pixels, named and typed as `like`.
    pub fn image(&self, like: &ImageData) -> ImageData {
        let rgba: Vec<u8> = self.p.iter().flat_map(|c| *c).collect();
        ImageData { width: self.w as u32, height: self.h as u32, rgba: Arc::new(rgba), encoded: Arc::new(vec![]), format: "png".into(), name: like.name.clone() }
    }

    /// A smaller copy whose longer side is at most `max` (for previews).
    pub fn fit(&self, max: usize) -> Px {
        let s = (max as f32 / self.w.max(self.h) as f32).min(1.0);
        if s >= 1.0 {
            return self.clone();
        }
        let (w, h) = (((self.w as f32 * s) as usize).max(1), ((self.h as f32 * s) as usize).max(1));
        let mut out = Px::new(w, h);
        for y in 0..h {
            for x in 0..w {
                out.p[y * w + x] = self.sample((x as f32 + 0.5) / s - 0.5, (y as f32 + 0.5) / s - 0.5);
            }
        }
        out
    }

    /// Bilinear sample (transparent outside).
    pub fn sample(&self, x: f32, y: f32) -> [u8; 4] {
        if x < -0.5 || y < -0.5 || x > self.w as f32 - 0.5 || y > self.h as f32 - 0.5 {
            return [0; 4];
        }
        let x = x.clamp(0.0, self.w as f32 - 1.0);
        let y = y.clamp(0.0, self.h as f32 - 1.0);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let g = |xx: usize, yy: usize| self.p[yy * self.w + xx];
        let (a, b, c, d) = (g(x0, y0), g(x1, y0), g(x0, y1), g(x1, y1));
        let mut out = [0u8; 4];
        // alpha-weighted so transparent pixels do not darken the edges
        let wa = [(1.0 - fx) * (1.0 - fy), fx * (1.0 - fy), (1.0 - fx) * fy, fx * fy];
        let al: f32 = [a, b, c, d].iter().zip(wa).map(|(p, w)| p[3] as f32 * w).sum();
        for k in 0..3 {
            let v: f32 = [a, b, c, d].iter().zip(wa).map(|(p, w)| p[k] as f32 * p[3] as f32 * w).sum();
            out[k] = if al > 0.0 { (v / al).round().clamp(0.0, 255.0) as u8 } else { 0 };
        }
        out[3] = al.round().clamp(0.0, 255.0) as u8;
        out
    }

    pub fn to_pixmap(&self) -> Option<tiny_skia::Pixmap> {
        let mut pm = tiny_skia::Pixmap::new(self.w as u32, self.h as u32)?;
        for (d, s) in pm.pixels_mut().iter_mut().zip(&self.p) {
            *d = tiny_skia::ColorU8::from_rgba(s[0], s[1], s[2], s[3]).premultiply();
        }
        Some(pm)
    }

    pub fn from_pixmap(pm: &tiny_skia::Pixmap) -> Px {
        let p = pm.pixels().iter().map(|c| {
            let c = c.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        });
        Px { w: pm.width() as usize, h: pm.height() as usize, p: p.collect() }
    }

    /// An egui image of these pixels.
    pub fn color_image(&self) -> egui::ColorImage {
        let rgba: Vec<u8> = self.p.iter().flat_map(|c| *c).collect();
        egui::ColorImage::from_rgba_unmultiplied([self.w, self.h], &rgba)
    }
}

fn luma(c: [u8; 4]) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

/// Runs bitmap effects over the pixels.
pub fn apply_effects(px: &Px, effects: &[BitmapEffect]) -> Px {
    let Some(mut pm) = px.to_pixmap() else { return px.clone() };
    let cx = vector_render::bitfx::FxCtx { s: 1.0, frame: [0.0, 0.0, px.w as f32, px.h as f32], seed: 1 };
    for e in effects {
        vector_render::bitfx::apply(e, &mut pm, &cx);
    }
    Px::from_pixmap(&pm)
}

// ------------------------------------------------------------------------------ modes

/// The color modes of Bitmaps ▸ Mode, in menu order.
pub const MODES: [&str; 7] = ["Black and White (1-bit)", "Grayscale (8-bit)", "Duotone (8-bit)", "Paletted (8-bit)", "RGB Color (24-bit)", "Lab Color (24-bit)", "CMYK Color (32-bit)"];
/// The mode of a bitmap that has none recorded.
pub const MODE_RGB: u8 = 4;

pub fn grayscale(px: &Px) -> Px {
    let mut out = px.clone();
    for c in out.p.iter_mut() {
        let l = luma(*c).round() as u8;
        *c = [l, l, l, c[3]];
    }
    out
}

/// The conversion methods of Convert to 1 bit.
pub const ONE_BIT_METHODS: [&str; 7] = ["Line Art", "Ordered", "Jarvis", "Stucki", "Floyd-Steinberg", "Halftone", "Cardinality-Distribution"];
pub const SCREENS: [&str; 6] = ["Square", "Round", "Line", "Cross", "Fixed 4x4", "Fixed 8x8"];

#[derive(Clone, PartialEq)]
pub struct OneBit {
    pub method: usize,
    /// 0..100: how much becomes white.
    pub intensity: f64,
    pub screen: usize,
    pub angle: f64,
    /// Screen frequency, lines per inch.
    pub lpi: f64,
    /// Pixels per inch of the bitmap (for the screen frequency).
    pub ppi: f64,
}

impl Default for OneBit {
    fn default() -> Self {
        OneBit { method: 5, intensity: 100.0, screen: 0, angle: 45.0, lpi: 75.0, ppi: 150.0 }
    }
}

const BAYER8: [[u8; 8]; 8] = [
    [0, 48, 12, 60, 3, 51, 15, 63],
    [32, 16, 44, 28, 35, 19, 47, 31],
    [8, 56, 4, 52, 11, 59, 7, 55],
    [40, 24, 36, 20, 43, 27, 39, 23],
    [2, 50, 14, 62, 1, 49, 13, 61],
    [34, 18, 46, 30, 33, 17, 45, 29],
    [10, 58, 6, 54, 9, 57, 5, 53],
    [42, 26, 38, 22, 41, 25, 37, 21],
];

/// Error diffusion: the neighbors (dx, dy, weight) and the divisor.
fn diffusion(method: usize) -> (&'static [(i32, i32, f32)], f32) {
    match method {
        2 => (&[(1, 0, 7.0), (2, 0, 5.0), (-2, 1, 3.0), (-1, 1, 5.0), (0, 1, 7.0), (1, 1, 5.0), (2, 1, 3.0), (-2, 2, 1.0), (-1, 2, 3.0), (0, 2, 5.0), (1, 2, 3.0), (2, 2, 1.0)], 48.0),
        3 => (&[(1, 0, 8.0), (2, 0, 4.0), (-2, 1, 2.0), (-1, 1, 4.0), (0, 1, 8.0), (1, 1, 4.0), (2, 1, 2.0), (-2, 2, 1.0), (-1, 2, 2.0), (0, 2, 4.0), (1, 2, 2.0), (2, 2, 1.0)], 42.0),
        _ => (&[(1, 0, 7.0), (-1, 1, 3.0), (0, 1, 5.0), (1, 1, 1.0)], 16.0),
    }
}

/// Black and white: each pixel black or white by the method.
pub fn one_bit(px: &Px, o: &OneBit) -> Px {
    let (w, h) = (px.w, px.h);
    // intensity moves the threshold: 100 keeps it in the middle, lower makes the result darker
    let bias = (o.intensity as f32 - 100.0) * 1.27;
    let mut lum: Vec<f32> = px.p.iter().map(|c| luma(*c) + bias).collect();
    let mut out = px.clone();
    let set = |out: &mut Px, i: usize, white: bool| {
        let v = if white { 255 } else { 0 };
        out.p[i] = [v, v, v, out.p[i][3]];
    };
    match o.method {
        0 => {
            for (i, l) in lum.iter().enumerate() {
                set(&mut out, i, *l >= 128.0);
            }
        }
        1 => {
            for y in 0..h {
                for x in 0..w {
                    let t = (BAYER8[y % 8][x % 8] as f32 + 0.5) * 4.0;
                    set(&mut out, y * w + x, lum[y * w + x] >= t);
                }
            }
        }
        2..=4 => {
            let (k, div) = diffusion(o.method);
            for y in 0..h {
                for x in 0..w {
                    let i = y * w + x;
                    let old = lum[i];
                    let white = old >= 128.0;
                    set(&mut out, i, white);
                    let err = old - if white { 255.0 } else { 0.0 };
                    for (dx, dy, wt) in k {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                            lum[ny as usize * w + nx as usize] += err * wt / div;
                        }
                    }
                }
            }
        }
        5 => {
            // the halftone screen: cell size from the frequency, turned by the angle
            let cell = (o.ppi / o.lpi.max(1.0)).max(2.0) as f32;
            let (s, c) = (o.angle as f32).to_radians().sin_cos();
            for y in 0..h {
                for x in 0..w {
                    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                    let (u, v) = ((fx * c + fy * s) / cell, (-fx * s + fy * c) / cell);
                    let (du, dv) = (u - u.floor() - 0.5, v - v.floor() - 0.5);
                    let t = match o.screen {
                        1 => 1.0 - (du * du + dv * dv).sqrt() * 1.414,
                        2 => 1.0 - dv.abs() * 2.0,
                        3 => 1.0 - du.abs().min(dv.abs()) * 2.0,
                        4 => (BAYER8[y % 4][x % 4] as f32 + 0.5) / 64.0 * 4.0 % 1.0,
                        5 => (BAYER8[y % 8][x % 8] as f32 + 0.5) / 64.0,
                        _ => 1.0 - du.abs().max(dv.abs()) * 2.0,
                    };
                    // dark pixels get big black dots
                    let dark = 1.0 - (lum[y * w + x] / 255.0).clamp(0.0, 1.0);
                    set(&mut out, y * w + x, t.clamp(0.0, 1.0) >= dark);
                }
            }
        }
        _ => {
            // cardinality-distribution: a scattered threshold
            for y in 0..h {
                for x in 0..w {
                    let t = (vector_render::bitfx::hash(x as i64, y as i64, 77) * 255.0).clamp(1.0, 254.0);
                    set(&mut out, y * w + x, lum[y * w + x] >= t);
                }
            }
        }
    }
    out
}

/// Duotone: the gray levels printed with one to four inks, each through its curve.
#[derive(Clone, PartialEq)]
pub struct Duotone {
    /// 0 monotone, 1 duotone, 2 tritone, 3 quadtone.
    pub kind: usize,
    /// The inks: name, color and curve (points 0..255 of ink amount for each gray).
    pub inks: Vec<(String, Color, Vec<(f32, f32)>)>,
}

impl Default for Duotone {
    fn default() -> Self {
        Duotone { kind: 0, inks: Duotone::inks_for(0) }
    }
}

impl Duotone {
    /// The reference's starting inks for each type.
    pub fn inks_for(kind: usize) -> Vec<(String, Color, Vec<(f32, f32)>)> {
        let all = [("PANTONE Black 7 C", Color::rgb(0x3d, 0x39, 0x35)), ("PANTONE 1505 C", Color::rgb(0xff, 0x6c, 0x0c)), ("PANTONE 3115 C", Color::rgb(0x00, 0xc1, 0xd5)), ("PANTONE 2607 C", Color::rgb(0x50, 0x0f, 0x8a))];
        all.iter().take(kind + 1).map(|(n, c)| (n.to_string(), *c, vec![(0.0, 0.0), (255.0, 255.0)])).collect()
    }
}

pub fn duotone(px: &Px, d: &Duotone) -> Px {
    let luts: Vec<[f32; 256]> = d.inks.iter().map(|i| vector_render::bitfx::curve_lut(&i.2)).collect();
    let mut out = px.clone();
    for c in out.p.iter_mut() {
        // ink amount grows with darkness
        let dark = (255.0 - luma(*c)).clamp(0.0, 255.0) as usize;
        let mut rgb = [1.0f32; 3];
        for (ink, lut) in d.inks.iter().zip(&luts) {
            let a = lut[dark] / 255.0;
            let ic = [ink.1.r as f32 / 255.0, ink.1.g as f32 / 255.0, ink.1.b as f32 / 255.0];
            for k in 0..3 {
                rgb[k] *= 1.0 - a * (1.0 - ic[k]);
            }
        }
        *c = [(rgb[0] * 255.0) as u8, (rgb[1] * 255.0) as u8, (rgb[2] * 255.0) as u8, c[3]];
    }
    out
}

/// The palettes of Convert to Paletted, in list order.
pub const PALETTES: [&str; 9] = ["Document Palette", "Color Styles Palette", "Adaptive", "Optimized", "Grayscale", "System", "Uniform Colors", "Standard VGA", "More Palettes..."];
pub const DITHERS: [&str; 5] = ["None", "Ordered", "Jarvis", "Stucki", "Floyd-Steinberg"];

#[derive(Clone, PartialEq)]
pub struct Paletted {
    pub smoothing: f64,
    pub palette: usize,
    pub dither: usize,
    pub intensity: f64,
    pub colors: usize,
}

impl Default for Paletted {
    fn default() -> Self {
        Paletted { smoothing: 0.0, palette: 6, dither: 0, intensity: 100.0, colors: 256 }
    }
}

/// The colors of a palette type (`doc`: the document palette, `styles`: the color styles).
pub fn palette_colors(px: &Px, o: &Paletted, doc: &[Color], styles: &[Color]) -> Vec<[u8; 3]> {
    let c3 = |c: &Color| [c.r, c.g, c.b];
    match o.palette {
        0 if !doc.is_empty() => doc.iter().map(c3).collect(),
        1 if !styles.is_empty() => styles.iter().map(c3).collect(),
        2 | 3 => {
            let samples: Vec<[u8; 4]> = px.p.iter().step_by((px.p.len() / 20000).max(1)).copied().filter(|c| c[3] > 0).collect();
            kmeans(&samples, o.colors.clamp(2, 256), if o.palette == 3 { 8 } else { 3 })
        }
        4 => (0..o.colors.clamp(2, 256)).map(|i| {
            let v = (i * 255 / (o.colors.clamp(2, 256) - 1)) as u8;
            [v, v, v]
        }).collect(),
        5 => vec![[0, 0, 0], [128, 0, 0], [0, 128, 0], [128, 128, 0], [0, 0, 128], [128, 0, 128], [0, 128, 128], [192, 192, 192], [192, 220, 192], [166, 202, 240], [255, 251, 240], [160, 160, 164], [128, 128, 128], [255, 0, 0], [0, 255, 0], [255, 255, 0], [0, 0, 255], [255, 0, 255], [0, 255, 255], [255, 255, 255]],
        7 => vec![[0, 0, 0], [128, 0, 0], [0, 128, 0], [128, 128, 0], [0, 0, 128], [128, 0, 128], [0, 128, 128], [192, 192, 192], [128, 128, 128], [255, 0, 0], [0, 255, 0], [255, 255, 0], [0, 0, 255], [255, 0, 255], [0, 255, 255], [255, 255, 255]],
        _ => {
            // uniform: 6 reds × 7 greens × 6 blues
            let mut v = vec![];
            for r in 0..6 {
                for g in 0..7 {
                    for b in 0..6 {
                        v.push([(r * 51) as u8, (g * 255 / 6) as u8, (b * 51) as u8]);
                    }
                }
            }
            v
        }
    }
}

fn kmeans(samples: &[[u8; 4]], k: usize, rounds: usize) -> Vec<[u8; 3]> {
    if samples.is_empty() {
        return vec![[0, 0, 0], [255, 255, 255]];
    }
    let mut centers: Vec<[f32; 3]> = (0..k).map(|i| {
        let c = samples[i * samples.len() / k];
        [c[0] as f32, c[1] as f32, c[2] as f32]
    }).collect();
    for _ in 0..rounds {
        let mut sum = vec![[0f32; 4]; k];
        for s in samples {
            let i = nearest3(&centers, [s[0] as f32, s[1] as f32, s[2] as f32]);
            for j in 0..3 {
                sum[i][j] += s[j] as f32;
            }
            sum[i][3] += 1.0;
        }
        for (c, s) in centers.iter_mut().zip(&sum) {
            if s[3] > 0.0 {
                *c = [s[0] / s[3], s[1] / s[3], s[2] / s[3]];
            }
        }
    }
    let mut out: Vec<[u8; 3]> = centers.iter().map(|c| [c[0] as u8, c[1] as u8, c[2] as u8]).collect();
    out.dedup();
    out
}

fn nearest3(pal: &[[f32; 3]], c: [f32; 3]) -> usize {
    let mut best = (0, f32::MAX);
    for (i, p) in pal.iter().enumerate() {
        let d = (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2);
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

/// Paletted: each pixel the nearest palette color, dithered.
pub fn paletted(px: &Px, o: &Paletted, pal: &[[u8; 3]]) -> Px {
    let palf: Vec<[f32; 3]> = pal.iter().map(|c| [c[0] as f32, c[1] as f32, c[2] as f32]).collect();
    let src = if o.smoothing > 0.0 {
        let e = BitmapEffect::new("GaussianBlur").with("Radius", o.smoothing / 25.0);
        apply_effects(px, &[e])
    } else {
        px.clone()
    };
    let (w, h) = (px.w, px.h);
    let mut work: Vec<[f32; 3]> = src.p.iter().map(|c| [c[0] as f32, c[1] as f32, c[2] as f32]).collect();
    let mut out = px.clone();
    let k = o.intensity as f32 / 100.0;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut c = work[i];
            if o.dither == 1 {
                let t = (BAYER8[y % 8][x % 8] as f32 / 64.0 - 0.5) * 48.0 * k;
                c = [c[0] + t, c[1] + t, c[2] + t];
            }
            let n = nearest3(&palf, c);
            out.p[i] = [pal[n][0], pal[n][1], pal[n][2], px.p[i][3]];
            if o.dither >= 2 {
                let (kern, div) = diffusion(o.dither);
                let err = [(work[i][0] - palf[n][0]) * k, (work[i][1] - palf[n][1]) * k, (work[i][2] - palf[n][2]) * k];
                for (dx, dy, wt) in kern {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                        let j = ny as usize * w + nx as usize;
                        for c in 0..3 {
                            work[j][c] += err[c] * wt / div;
                        }
                    }
                }
            }
        }
    }
    out
}

// ------------------------------------------------------------------------------ straighten

#[derive(Clone, PartialEq)]
pub struct Straighten {
    /// -100..100: barrel (negative) or pincushion (positive).
    pub lens: f64,
    /// Degrees, -15..15.
    pub rotate: f64,
    /// -100..100.
    pub vertical: f64,
    pub horizontal: f64,
    pub crop: bool,
    pub crop_resample: bool,
}

impl Default for Straighten {
    fn default() -> Self {
        Straighten { lens: 0.0, rotate: 0.0, vertical: 0.0, horizontal: 0.0, crop: true, crop_resample: false }
    }
}

impl Straighten {
    /// Where the output point (centered, -0.5..0.5 of the size) comes from in the source.
    fn source(&self, u: f32, v: f32, aspect: f32) -> (f32, f32) {
        // undo the rotation (in a square space so the angle stays true)
        let (s, c) = (-(self.rotate as f32).to_radians()).sin_cos();
        let (x, y) = (u * aspect, v);
        let (x, y) = (x * c - y * s, x * s + y * c);
        let (mut u, mut v) = (x / aspect, y);
        // perspective: one edge narrower than the other
        let kv = self.vertical as f32 / 250.0;
        let kh = self.horizontal as f32 / 250.0;
        u /= 1.0 + kv * v * 2.0;
        v /= 1.0 + kh * u * 2.0;
        // lens distortion
        let r2 = u * u + v * v;
        let k = 1.0 + self.lens as f32 / 120.0 * r2;
        (u * k, v * k)
    }

    /// The share of the size (0..1) of the largest centered crop holding only image.
    fn crop_share(&self, w: usize, h: usize) -> f32 {
        let aspect = w as f32 / h as f32;
        let inside = |s: f32| {
            (0..=20).all(|i| {
                let t = i as f32 / 20.0 - 0.5;
                [(t, -0.5), (t, 0.5), (-0.5, t), (0.5, t)].iter().all(|(a, b)| {
                    let (x, y) = self.source(a * s, b * s, aspect);
                    x.abs() <= 0.5 && y.abs() <= 0.5
                })
            })
        };
        let (mut lo, mut hi) = (0.05f32, 1.0f32);
        if inside(1.0) {
            return 1.0;
        }
        for _ in 0..18 {
            let m = (lo + hi) / 2.0;
            if inside(m) {
                lo = m;
            } else {
                hi = m;
            }
        }
        lo
    }
}

pub fn straighten(px: &Px, o: &Straighten) -> Px {
    let (w, h) = (px.w, px.h);
    let aspect = w as f32 / h as f32;
    let share = if o.crop || o.crop_resample { o.crop_share(w, h) } else { 1.0 };
    let (ow, oh) = if o.crop_resample || !o.crop { (w, h) } else { (((w as f32 * share).round() as usize).max(1), ((h as f32 * share).round() as usize).max(1)) };
    let mut out = Px::new(ow, oh);
    for y in 0..oh {
        for x in 0..ow {
            let u = ((x as f32 + 0.5) / ow as f32 - 0.5) * share;
            let v = ((y as f32 + 0.5) / oh as f32 - 0.5) * share;
            let (su, sv) = o.source(u, v, aspect);
            out.p[y * ow + x] = px.sample((su + 0.5) * w as f32 - 0.5, (sv + 0.5) * h as f32 - 0.5);
        }
    }
    out
}

// ------------------------------------------------------------------------------ adjustment lab

#[derive(Clone, PartialEq)]
pub struct AdjustLab {
    pub temperature: f64,
    pub tint: f64,
    pub saturation: f64,
    pub brightness: f64,
    pub contrast: f64,
    pub highlights: f64,
    pub shadows: f64,
    pub midtones: f64,
}

impl Default for AdjustLab {
    fn default() -> Self {
        AdjustLab { temperature: 5000.0, tint: 0.0, saturation: 0.0, brightness: 0.0, contrast: 0.0, highlights: 0.0, shadows: 0.0, midtones: 0.0 }
    }
}

impl AdjustLab {
    pub fn effects(&self) -> Vec<BitmapEffect> {
        
        vec![
            BitmapEffect::new("TintTemp").with("Temperature", self.temperature).with("Tint", self.tint),
            BitmapEffect::new("Vibrance").with("Saturation", self.saturation),
            BitmapEffect::new("SMH").with("Brightness", self.brightness).with("Contrast", self.contrast).with("Highlights", self.highlights).with("Shadows", self.shadows).with("Midtones", self.midtones),
        ]
    }

    /// Auto adjust: stretch the levels and neutralize the color cast.
    pub fn auto(px: &Px) -> AdjustLab {
        let n = px.p.len().max(1) as f64;
        let (mut r, mut g, mut b, mut lo, mut hi) = (0.0, 0.0, 0.0, 255.0f64, 0.0f64);
        for c in &px.p {
            r += c[0] as f64;
            g += c[1] as f64;
            b += c[2] as f64;
            let l = luma(*c) as f64;
            lo = lo.min(l);
            hi = hi.max(l);
        }
        let (r, b) = (r / n, b / n);
        let _ = g;
        let warm = ((b - r) / 255.0 * 3000.0).clamp(-2000.0, 2000.0);
        let range = (hi - lo).max(1.0);
        AdjustLab { temperature: (5000.0 - warm).round(), contrast: ((255.0 / range - 1.0) * 50.0).clamp(0.0, 60.0).round(), ..Default::default() }
    }
}

// ------------------------------------------------------------------------------ cleanup

/// Remove JPEG Artifacts: smooths the 8 × 8 block edges and the ringing near edges while keeping
/// the edges of the picture.
pub fn remove_jpeg_artifacts(px: &Px) -> Px {
    let (w, h) = (px.w, px.h);
    let mut out = px.clone();
    for y in 0..h {
        for x in 0..w {
            let c = px.p[y * w + x];
            let mut acc = [0f32; 3];
            let mut wsum = 0f32;
            let near_block = x % 8 == 0 || x % 8 == 7 || y % 8 == 0 || y % 8 == 7;
            let sigma = if near_block { 26.0f32 } else { 14.0 };
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let (nx, ny) = ((x as i32 + dx).clamp(0, w as i32 - 1) as usize, (y as i32 + dy).clamp(0, h as i32 - 1) as usize);
                    let n = px.p[ny * w + nx];
                    let d = ((n[0] as f32 - c[0] as f32).powi(2) + (n[1] as f32 - c[1] as f32).powi(2) + (n[2] as f32 - c[2] as f32).powi(2)).sqrt();
                    let wt = (-(d / sigma).powi(2)).exp() * if dx == 0 && dy == 0 { 1.0 } else { 0.7 };
                    for k in 0..3 {
                        acc[k] += n[k] as f32 * wt;
                    }
                    wsum += wt;
                }
            }
            out.p[y * w + x] = [(acc[0] / wsum) as u8, (acc[1] / wsum) as u8, (acc[2] / wsum) as u8, c[3]];
        }
    }
    out
}

/// Remove Background: the area joined to the border with the border's color becomes transparent
/// (with a soft edge).
pub fn remove_background(px: &Px) -> Px {
    let (w, h) = (px.w, px.h);
    if w < 3 || h < 3 {
        return px.clone();
    }
    // the background color: the median of the border
    let mut border: Vec<[u8; 4]> = vec![];
    for x in 0..w {
        border.push(px.p[x]);
        border.push(px.p[(h - 1) * w + x]);
    }
    for y in 0..h {
        border.push(px.p[y * w]);
        border.push(px.p[y * w + w - 1]);
    }
    let med = |k: usize| {
        let mut v: Vec<u8> = border.iter().map(|c| c[k]).collect();
        v.sort_unstable();
        v[v.len() / 2] as f32
    };
    let bg = [med(0), med(1), med(2)];
    let dist = |c: [u8; 4]| ((c[0] as f32 - bg[0]).powi(2) + (c[1] as f32 - bg[1]).powi(2) + (c[2] as f32 - bg[2]).powi(2)).sqrt();
    let tol = 60.0;
    let mut gone = vec![false; w * h];
    let mut stack: Vec<usize> = (0..w).chain((0..w).map(|x| (h - 1) * w + x)).chain((0..h).map(|y| y * w)).chain((0..h).map(|y| y * w + w - 1)).filter(|i| dist(px.p[*i]) < tol).collect();
    while let Some(i) = stack.pop() {
        if gone[i] {
            continue;
        }
        gone[i] = true;
        let (x, y) = (i % w, i / w);
        for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
            if nx < w && ny < h {
                let j = ny * w + nx;
                if !gone[j] && dist(px.p[j]) < tol {
                    stack.push(j);
                }
            }
        }
    }
    let mut out = px.clone();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if gone[i] {
                out.p[i][3] = 0;
            } else {
                // soften the pixels next to the removed area
                let n = [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)].iter().filter(|(a, b)| *a < w && *b < h && gone[b * w + a]).count();
                if n > 0 {
                    out.p[i][3] = (out.p[i][3] as f32 * (1.0 - n as f32 * 0.12)) as u8;
                }
            }
        }
    }
    out
}

/// The pixels inside a crop rectangle (0..1 of the size).
pub fn crop(px: &Px, c: [f64; 4]) -> Px {
    let x0 = (c[0] * px.w as f64).floor().clamp(0.0, px.w as f64 - 1.0) as usize;
    let y0 = (c[1] * px.h as f64).floor().clamp(0.0, px.h as f64 - 1.0) as usize;
    let x1 = ((c[2] * px.w as f64).ceil() as usize).clamp(x0 + 1, px.w);
    let y1 = ((c[3] * px.h as f64).ceil() as usize).clamp(y0 + 1, px.h);
    let mut out = Px::new(x1 - x0, y1 - y0);
    for y in y0..y1 {
        for x in x0..x1 {
            out.p[(y - y0) * out.w + (x - x0)] = px.p[y * px.w + x];
        }
    }
    out
}

/// A larger canvas with the pixels in its middle (transparent around).
pub fn inflate(px: &Px, w: usize, h: usize) -> Px {
    let (w, h) = (w.max(px.w), h.max(px.h));
    let (ox, oy) = ((w - px.w) / 2, (h - px.h) / 2);
    let mut out = Px::new(w, h);
    for y in 0..px.h {
        for x in 0..px.w {
            out.p[(y + oy) * w + x + ox] = px.p[y * px.w + x];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Px {
        let mut p = Px::new(32, 24);
        for y in 0..24 {
            for x in 0..32 {
                let inside = (x as i32 - 16).pow(2) + (y as i32 - 12).pow(2) < 64;
                p.p[y * 32 + x] = if inside { [240, 200, 40, 255] } else { [70, 130, 200, 255] };
            }
        }
        p
    }

    #[test]
    fn one_bit_makes_only_black_and_white() {
        for m in 0..ONE_BIT_METHODS.len() {
            let out = one_bit(&sample(), &OneBit { method: m, ..Default::default() });
            assert!(out.p.iter().all(|c| (c[0] == 0 || c[0] == 255) && c[0] == c[1] && c[1] == c[2]), "{m}");
        }
    }

    #[test]
    fn paletted_uses_only_palette_colors() {
        let px = sample();
        let o = Paletted { palette: 7, dither: 4, ..Default::default() };
        let pal = palette_colors(&px, &o, &[], &[]);
        let out = paletted(&px, &o, &pal);
        assert!(out.p.iter().all(|c| pal.contains(&[c[0], c[1], c[2]])));
    }

    #[test]
    fn background_goes_and_the_subject_stays() {
        let out = remove_background(&sample());
        assert_eq!(out.p[0][3], 0);
        assert_eq!(out.p[12 * 32 + 16][3], 255);
    }

    #[test]
    fn straighten_and_inflate_sizes() {
        let px = sample();
        let s = straighten(&px, &Straighten { rotate: 10.0, ..Default::default() });
        assert!(s.w < 32 && s.h < 24 && s.w > 10);
        let r = straighten(&px, &Straighten { rotate: 10.0, crop_resample: true, ..Default::default() });
        assert_eq!((r.w, r.h), (32, 24));
        let i = inflate(&px, 40, 30);
        assert_eq!((i.w, i.h), (40, 30));
        assert_eq!(i.p[0][3], 0);
        let c = crop(&px, [0.25, 0.25, 0.75, 0.75]);
        assert_eq!((c.w, c.h), (16, 12));
    }
}
