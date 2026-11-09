//! The bitmap effects of the Effects menu (the Bitmap Effects list of the Properties docker):
//! each effect works on the object rendered to pixels, with the settings of the reference.
//!
//! Sizes in the settings are in effect pixels; `FxCtx::s` is the number of device pixels per
//! effect pixel, so an effect looks the same at every zoom. Centers and frames are relative to the
//! object's bounds (`FxCtx::frame`), not to the visible part of it.

use crate::bitfx_art as art;
use tiny_skia::Pixmap;
use vector_core::object::BitmapEffect;
use vector_core::Color;

/// Where an effect runs: device pixels per effect pixel, the object's bounds in the layer
/// (x0, y0, x1, y1) and a seed for the random parts (the object's id).
#[derive(Clone, Copy, Debug)]
pub struct FxCtx {
    pub s: f32,
    pub frame: [f32; 4],
    pub seed: u32,
}

impl FxCtx {
    pub fn px(&self, v: impl Into<f64>) -> f32 {
        let v: f64 = v.into();
        (v as f32 * self.s).max(0.0)
    }
    pub fn fw(&self) -> f32 {
        (self.frame[2] - self.frame[0]).max(1.0)
    }
    pub fn fh(&self) -> f32 {
        (self.frame[3] - self.frame[1]).max(1.0)
    }
    /// A point given in 0..1 of the object's bounds, in layer pixels.
    pub fn at(&self, u: f64, v: f64) -> (f32, f32) {
        (self.frame[0] + self.fw() * u as f32, self.frame[1] + self.fh() * v as f32)
    }
    /// The center an effect turns or zooms around (CenterX/CenterY, 0..1, default the middle).
    pub fn center(&self, e: &BitmapEffect) -> (f32, f32) {
        self.at(e.num("CenterX", 0.5), e.num("CenterY", 0.5))
    }
}

/// An image in straight (not premultiplied) RGBA, 0..255 floats.
#[derive(Clone)]
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub p: Vec<[f32; 4]>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Edge {
    /// Outside is transparent.
    Clear,
    /// Outside repeats the nearest edge pixel.
    Clamp,
    /// Outside wraps around.
    Wrap,
}

impl Img {
    pub fn from(pm: &Pixmap) -> Img {
        let (w, h) = (pm.width() as usize, pm.height() as usize);
        let p = pm
            .data()
            .chunks_exact(4)
            .map(|px| {
                let a = px[3] as f32;
                if a <= 0.0 {
                    [0.0, 0.0, 0.0, 0.0]
                } else {
                    let f = 255.0 / a;
                    [px[0] as f32 * f, px[1] as f32 * f, px[2] as f32 * f, a]
                }
            })
            .collect();
        Img { w, h, p }
    }
    pub fn new(w: usize, h: usize) -> Img {
        Img { w, h, p: vec![[0.0; 4]; w * h] }
    }
    pub fn write(&self, pm: &mut Pixmap) {
        for (d, s) in pm.data_mut().chunks_exact_mut(4).zip(self.p.iter()) {
            let a = s[3].clamp(0.0, 255.0);
            let f = a / 255.0;
            d[0] = (s[0].clamp(0.0, 255.0) * f).round() as u8;
            d[1] = (s[1].clamp(0.0, 255.0) * f).round() as u8;
            d[2] = (s[2].clamp(0.0, 255.0) * f).round() as u8;
            d[3] = a.round() as u8;
        }
    }
    #[inline]
    pub fn at(&self, x: usize, y: usize) -> [f32; 4] {
        self.p[y * self.w + x]
    }
    #[inline]
    pub fn get(&self, x: i64, y: i64, edge: Edge) -> [f32; 4] {
        let (w, h) = (self.w as i64, self.h as i64);
        let (x, y) = match edge {
            Edge::Clear => {
                if x < 0 || y < 0 || x >= w || y >= h {
                    return [0.0; 4];
                }
                (x, y)
            }
            Edge::Clamp => (x.clamp(0, w - 1), y.clamp(0, h - 1)),
            Edge::Wrap => (x.rem_euclid(w), y.rem_euclid(h)),
        };
        self.p[(y * w + x) as usize]
    }
    /// Bilinear sample at a pixel position (pixel centers at +0.5).
    pub fn sample(&self, x: f32, y: f32, edge: Edge) -> [f32; 4] {
        let x = x - 0.5;
        let y = y - 0.5;
        let x0 = x.floor();
        let y0 = y.floor();
        let fx = x - x0;
        let fy = y - y0;
        let (x0, y0) = (x0 as i64, y0 as i64);
        let a = self.get(x0, y0, edge);
        let b = self.get(x0 + 1, y0, edge);
        let c = self.get(x0, y0 + 1, edge);
        let d = self.get(x0 + 1, y0 + 1, edge);
        // weight colors by alpha so transparent pixels do not darken the edges
        let mut out = [0.0f32; 4];
        let ws = [(1.0 - fx) * (1.0 - fy), fx * (1.0 - fy), (1.0 - fx) * fy, fx * fy];
        let mut aw = 0.0;
        for (px, wgt) in [a, b, c, d].iter().zip(ws) {
            let k = wgt * px[3];
            out[0] += px[0] * k;
            out[1] += px[1] * k;
            out[2] += px[2] * k;
            out[3] += px[3] * wgt;
            aw += k;
        }
        if aw > 0.0 {
            out[0] /= aw;
            out[1] /= aw;
            out[2] /= aw;
        }
        out
    }
    /// A new image where each pixel takes the source at `f(x, y)` (pixel centers).
    pub fn remap(&self, edge: Edge, f: impl Fn(f32, f32) -> (f32, f32)) -> Img {
        let mut out = Img::new(self.w, self.h);
        for y in 0..self.h {
            for x in 0..self.w {
                let (sx, sy) = f(x as f32 + 0.5, y as f32 + 0.5);
                out.p[y * self.w + x] = self.sample(sx, sy, edge);
            }
        }
        out
    }
    pub fn map(&mut self, mut f: impl FnMut([f32; 3]) -> [f32; 3]) {
        for px in self.p.iter_mut() {
            if px[3] <= 0.0 {
                continue;
            }
            let o = f([px[0], px[1], px[2]]);
            px[0] = o[0].clamp(0.0, 255.0);
            px[1] = o[1].clamp(0.0, 255.0);
            px[2] = o[2].clamp(0.0, 255.0);
        }
    }
    pub fn luma(&self) -> Vec<f32> {
        self.p.iter().map(|p| luma(p)).collect()
    }
    /// Mix `other` over this image by `t` (0 keeps this image).
    pub fn mix(&mut self, other: &Img, t: f32) {
        let t = t.clamp(0.0, 1.0);
        for (a, b) in self.p.iter_mut().zip(other.p.iter()) {
            for c in 0..4 {
                a[c] = a[c] * (1.0 - t) + b[c] * t;
            }
        }
    }
}

#[inline]
pub fn luma(p: &[f32; 4]) -> f32 {
    0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2]
}

pub fn clamp255(v: f32) -> f32 {
    v.clamp(0.0, 255.0)
}

/// A stable random number 0..1 for an integer position.
#[inline]
pub fn hash(x: i64, y: i64, seed: u32) -> f32 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ (seed as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    h = h.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    h ^= h >> 33;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// Smooth value noise 0..1 at a position.
pub fn noise(x: f32, y: f32, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (ix, iy) = (x0 as i64, y0 as i64);
    let a = hash(ix, iy, seed);
    let b = hash(ix + 1, iy, seed);
    let c = hash(ix, iy + 1, seed);
    let d = hash(ix + 1, iy + 1, seed);
    let top = a + (b - a) * sx;
    let bot = c + (d - c) * sx;
    top + (bot - top) * sy
}

/// Fractal noise 0..1 with `oct` octaves.
pub fn fbm(x: f32, y: f32, oct: u32, seed: u32) -> f32 {
    let (mut amp, mut freq, mut sum, mut norm) = (0.5, 1.0, 0.0, 0.0);
    for i in 0..oct {
        sum += noise(x * freq, y * freq, seed.wrapping_add(i * 101)) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm.max(1e-6)
}

pub fn rgb_to_hsl(c: [f32; 3]) -> (f32, f32, f32) {
    let (r, g, b) = (c[0] / 255.0, c[1] / 255.0, c[2] / 255.0);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let l = (mx + mn) / 2.0;
    if mx - mn < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [(r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0]
}

pub fn col(c: Color) -> [f32; 3] {
    [c.r as f32, c.g as f32, c.b as f32]
}

// ------------------------------------------------------------------------------ filters

/// Gaussian blur (three box passes) on premultiplied colors, so transparent pixels do not bleed.
pub fn gauss(im: &Img, radius: f32) -> Img {
    if radius < 0.3 {
        return im.clone();
    }
    let (w, h) = (im.w, im.h);
    let mut buf: Vec<[f32; 4]> = im.p.iter().map(|p| {
        let a = p[3] / 255.0;
        [p[0] * a, p[1] * a, p[2] * a, p[3]]
    }).collect();
    let sigma = (radius / 2.0).max(0.4);
    let n = 3.0f32;
    let w_ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let m = ((12.0 * sigma * sigma - n * (wl * wl) as f32 - 4.0 * n * wl as f32 - 3.0 * n) / (-4.0 * wl as f32 - 4.0)).round() as i32;
    let mut tmp = buf.clone();
    for i in 0..3 {
        let bw = if i < m { wl.max(1) } else { wu.max(1) };
        let r = ((bw - 1) / 2).max(0) as usize;
        box_h(&buf, &mut tmp, w, h, r);
        box_v(&tmp, &mut buf, w, h, r);
    }
    Img {
        w,
        h,
        p: buf
            .into_iter()
            .map(|p| if p[3] <= 0.01 { [0.0; 4] } else { let f = 255.0 / p[3]; [p[0] * f, p[1] * f, p[2] * f, p[3]] })
            .collect(),
    }
}

/// A box blur of radius `rx` × `ry` (premultiplied).
pub fn box_blur(im: &Img, rx: usize, ry: usize) -> Img {
    let (w, h) = (im.w, im.h);
    let mut buf: Vec<[f32; 4]> = im.p.iter().map(|p| { let a = p[3] / 255.0; [p[0] * a, p[1] * a, p[2] * a, p[3]] }).collect();
    let mut tmp = buf.clone();
    box_h(&buf, &mut tmp, w, h, rx);
    box_v(&tmp, &mut buf, w, h, ry);
    Img { w, h, p: buf.into_iter().map(|p| if p[3] <= 0.01 { [0.0; 4] } else { let f = 255.0 / p[3]; [p[0] * f, p[1] * f, p[2] * f, p[3]] }).collect() }
}

fn box_h(src: &[[f32; 4]], dst: &mut [[f32; 4]], w: usize, h: usize, r: usize) {
    if r == 0 || w == 0 {
        dst.copy_from_slice(src);
        return;
    }
    let k = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let get = |x: isize| row[x.clamp(0, w as isize - 1) as usize];
        let mut acc = [0.0f32; 4];
        for x in -(r as isize)..=(r as isize) {
            let p = get(x);
            for c in 0..4 {
                acc[c] += p[c];
            }
        }
        for x in 0..w {
            for c in 0..4 {
                dst[y * w + x][c] = acc[c] * k;
            }
            let add = get(x as isize + r as isize + 1);
            let sub = get(x as isize - r as isize);
            for c in 0..4 {
                acc[c] += add[c] - sub[c];
            }
        }
    }
}

fn box_v(src: &[[f32; 4]], dst: &mut [[f32; 4]], w: usize, h: usize, r: usize) {
    if r == 0 || h == 0 {
        dst.copy_from_slice(src);
        return;
    }
    let k = 1.0 / (2 * r + 1) as f32;
    for x in 0..w {
        let get = |y: isize| src[y.clamp(0, h as isize - 1) as usize * w + x];
        let mut acc = [0.0f32; 4];
        for y in -(r as isize)..=(r as isize) {
            let p = get(y);
            for c in 0..4 {
                acc[c] += p[c];
            }
        }
        for y in 0..h {
            for c in 0..4 {
                dst[y * w + x][c] = acc[c] * k;
            }
            let add = get(y as isize + r as isize + 1);
            let sub = get(y as isize - r as isize);
            for c in 0..4 {
                acc[c] += add[c] - sub[c];
            }
        }
    }
}

/// A 3×3 (or any odd square) convolution of the colors; alpha is kept.
pub fn convolve(im: &Img, k: &[f32], n: usize, div: f32, off: f32) -> Img {
    let r = (n / 2) as i64;
    let mut out = im.clone();
    for y in 0..im.h {
        for x in 0..im.w {
            if im.at(x, y)[3] <= 0.0 {
                continue;
            }
            let mut acc = [0.0f32; 3];
            for j in 0..n {
                for i in 0..n {
                    let kv = k[j * n + i];
                    if kv == 0.0 {
                        continue;
                    }
                    let p = im.get(x as i64 + i as i64 - r, y as i64 + j as i64 - r, Edge::Clamp);
                    for c in 0..3 {
                        acc[c] += p[c] * kv;
                    }
                }
            }
            let o = &mut out.p[y * im.w + x];
            for c in 0..3 {
                o[c] = clamp255(acc[c] / div + off);
            }
        }
    }
    out
}

/// The horizontal and vertical luminance gradients (Sobel).
pub fn sobel(im: &Img) -> (Vec<f32>, Vec<f32>) {
    let l = im.luma();
    let (w, h) = (im.w as i64, im.h as i64);
    let g = |x: i64, y: i64| l[(y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize];
    let mut gx = vec![0.0; l.len()];
    let mut gy = vec![0.0; l.len()];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            gx[i] = (g(x + 1, y - 1) + 2.0 * g(x + 1, y) + g(x + 1, y + 1)) - (g(x - 1, y - 1) + 2.0 * g(x - 1, y) + g(x - 1, y + 1));
            gy[i] = (g(x - 1, y + 1) + 2.0 * g(x, y + 1) + g(x + 1, y + 1)) - (g(x - 1, y - 1) + 2.0 * g(x, y - 1) + g(x + 1, y - 1));
        }
    }
    (gx, gy)
}

/// Lighting of a height map: 1 is flat, more is lit, less is in shadow. `dir` is the direction
/// the light comes from (degrees, counterclockwise from the right), `depth` the relief strength.
pub fn shade(hm: &[f32], w: usize, h: usize, dir: f32, depth: f32) -> Vec<f32> {
    let (lx, ly) = (dir.to_radians().cos(), -dir.to_radians().sin());
    let (wi, hi) = (w as i64, h as i64);
    let g = |x: i64, y: i64| hm[(y.clamp(0, hi - 1) * wi + x.clamp(0, wi - 1)) as usize];
    let mut out = vec![1.0; hm.len()];
    for y in 0..hi {
        for x in 0..wi {
            let dx = (g(x + 1, y) - g(x - 1, y)) * 0.5;
            let dy = (g(x, y + 1) - g(x, y - 1)) * 0.5;
            // the slope facing the light is lit
            out[(y * wi + x) as usize] = 1.0 - (dx * lx + dy * ly) * depth;
        }
    }
    out
}

/// Max / min / median of a square neighborhood, per channel.
fn rank(im: &Img, r: usize, which: u8) -> Img {
    if r == 0 {
        return im.clone();
    }
    let ri = r as i64;
    let mut out = im.clone();
    let mut buf: Vec<f32> = Vec::with_capacity((2 * r + 1) * (2 * r + 1));
    for y in 0..im.h {
        for x in 0..im.w {
            if im.at(x, y)[3] <= 0.0 {
                continue;
            }
            for c in 0..3 {
                buf.clear();
                for j in -ri..=ri {
                    for i in -ri..=ri {
                        let p = im.get(x as i64 + i, y as i64 + j, Edge::Clamp);
                        if p[3] > 0.0 {
                            buf.push(p[c]);
                        }
                    }
                }
                if buf.is_empty() {
                    continue;
                }
                let v = match which {
                    0 => buf.iter().cloned().fold(f32::MIN, f32::max),
                    1 => buf.iter().cloned().fold(f32::MAX, f32::min),
                    _ => {
                        let mid = buf.len() / 2;
                        let (_, m, _) = buf.select_nth_unstable_by(mid, |a, b| a.partial_cmp(b).unwrap());
                        *m
                    }
                };
                out.p[y * im.w + x][c] = v;
            }
        }
    }
    out
}

/// A lookup table applied to the colors (`ch`: 0 all, 1 red, 2 green, 3 blue).
fn lut(im: &mut Img, t: &[f32; 256], ch: usize) {
    let f = |v: f32| {
        let v = v.clamp(0.0, 255.0);
        let i = v.floor() as usize;
        let j = (i + 1).min(255);
        let k = v - i as f32;
        t[i] * (1.0 - k) + t[j] * k
    };
    im.map(|c| match ch {
        1 => [f(c[0]), c[1], c[2]],
        2 => [c[0], f(c[1]), c[2]],
        3 => [c[0], c[1], f(c[2])],
        _ => [f(c[0]), f(c[1]), f(c[2])],
    });
}

/// A curve through points (x, y in 0..255) as a lookup table (monotone cubic).
pub fn curve_lut(pts: &[(f32, f32)]) -> [f32; 256] {
    let mut p: Vec<(f32, f32)> = pts.to_vec();
    p.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    p.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-3);
    let mut t = [0.0f32; 256];
    if p.len() < 2 {
        for (i, v) in t.iter_mut().enumerate() {
            *v = i as f32;
        }
        return t;
    }
    let n = p.len();
    let mut m = vec![0.0f32; n];
    let d: Vec<f32> = (0..n - 1).map(|i| (p[i + 1].1 - p[i].1) / (p[i + 1].0 - p[i].0).max(1e-3)).collect();
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        m[i] = if d[i - 1] * d[i] <= 0.0 { 0.0 } else { (d[i - 1] + d[i]) / 2.0 };
    }
    for (i, v) in t.iter_mut().enumerate() {
        let x = i as f32;
        let y = if x <= p[0].0 {
            p[0].1
        } else if x >= p[n - 1].0 {
            p[n - 1].1
        } else {
            let k = p.windows(2).position(|w| x >= w[0].0 && x <= w[1].0).unwrap_or(0);
            let (x0, y0) = p[k];
            let (x1, y1) = p[k + 1];
            let hh = (x1 - x0).max(1e-3);
            let s = (x - x0) / hh;
            let h00 = 2.0 * s * s * s - 3.0 * s * s + 1.0;
            let h10 = s * s * s - 2.0 * s * s + s;
            let h01 = -2.0 * s * s * s + 3.0 * s * s;
            let h11 = s * s * s - s * s;
            h00 * y0 + h10 * hh * m[k] + h01 * y1 + h11 * hh * m[k + 1]
        };
        *v = y.clamp(0.0, 255.0);
    }
    t
}

/// The luminance histogram (256 bins) of the visible pixels, for the docker.
pub fn histogram(pm: &Pixmap) -> [[u32; 256]; 4] {
    let mut h = [[0u32; 256]; 4];
    for px in pm.data().chunks_exact(4) {
        let a = px[3] as f32;
        if a < 8.0 {
            continue;
        }
        let f = 255.0 / a;
        let (r, g, b) = ((px[0] as f32 * f).min(255.0), (px[1] as f32 * f).min(255.0), (px[2] as f32 * f).min(255.0));
        h[0][(0.299 * r + 0.587 * g + 0.114 * b).round().min(255.0) as usize] += 1;
        h[1][r as usize] += 1;
        h[2][g as usize] += 1;
        h[3][b as usize] += 1;
    }
    h
}

// ------------------------------------------------------------------------------ dispatch

/// Applies one effect to the layer.
pub fn apply(e: &BitmapEffect, pm: &mut Pixmap, cx: &FxCtx) {
    if !e.enabled || pm.width() == 0 || pm.height() == 0 {
        return;
    }
    let mut im = Img::from(pm);
    let done = adjust(e, &mut im, cx) || blur_fx(e, &mut im, cx) || sharpen_fx(e, &mut im, cx) || noise_fx(e, &mut im, cx) || art::apply(e, &mut im, cx);
    if done {
        im.write(pm);
    }
}

fn pct(e: &BitmapEffect, k: &str, d: f64) -> f32 {
    (e.num(k, d) / 100.0) as f32
}

/// Adjust, Color Transform, Transform and Contour effects.
fn adjust(e: &BitmapEffect, im: &mut Img, cx: &FxCtx) -> bool {
    match e.id.as_str() {
        "Invert" => im.map(|c| [255.0 - c[0], 255.0 - c[1], 255.0 - c[2]]),
        "Desaturate" => im.map(|c| {
            let l = luma(&[c[0], c[1], c[2], 255.0]);
            [l, l, l]
        }),
        "Equalize" => {
            // Levels: input black/gamma/white and output black/white, for all channels (0) and
            // then for each of red, green and blue
            for ch in 0..4usize {
                let k = |n: &str, d: f64| e.num(&format!("{n}{ch}"), d) as f32;
                let (il, g, ih) = (k("InputLow", 0.0), k("Gamma", 1.0).max(0.01), k("InputHigh", 255.0));
                let (ol, oh) = (k("OutputLow", 0.0), k("OutputHigh", 255.0));
                if il == 0.0 && g == 1.0 && ih == 255.0 && ol == 0.0 && oh == 255.0 {
                    continue;
                }
                let mut t = [0.0f32; 256];
                for (i, v) in t.iter_mut().enumerate() {
                    let x = ((i as f32 - il) / (ih - il).max(1.0)).clamp(0.0, 1.0).powf(1.0 / g);
                    *v = ol + x * (oh - ol);
                }
                lut(im, &t, ch);
            }
        }
        "HistogramEqualize" => {
            let mut hist = [0u32; 256];
            let mut n = 0u32;
            for p in &im.p {
                if p[3] > 0.0 {
                    hist[luma(p).round().clamp(0.0, 255.0) as usize] += 1;
                    n += 1;
                }
            }
            let mut cdf = [0.0f32; 256];
            let mut acc = 0u32;
            for i in 0..256 {
                acc += hist[i];
                cdf[i] = acc as f32 / n.max(1) as f32 * 255.0;
            }
            let preserve = e.flag("PreserveColor", false);
            im.map(|c| {
                if preserve {
                    let l = luma(&[c[0], c[1], c[2], 255.0]);
                    let k = cdf[l.round().clamp(0.0, 255.0) as usize] / l.max(1.0);
                    [c[0] * k, c[1] * k, c[2] * k]
                } else {
                    let f = |v: f32| cdf[v.round().clamp(0.0, 255.0) as usize];
                    [f(c[0]), f(c[1]), f(c[2])]
                }
            });
        }
        "SampleTargetBalance" => {
            // map each sample color to its target color, piecewise per channel
            let mut pts: Vec<[(f32, f32); 3]> = vec![[(0.0, 0.0); 3], [(255.0, 255.0); 3]];
            for (use_k, src, dst) in [("UseLow", "LowSourceItem", "LowTargetItem"), ("UseMid", "MidSourceItem", "MidTargetItem"), ("UseHigh", "HighSourceItem", "HighTargetItem")] {
                if e.flag(use_k, false) {
                    let (s, t) = (col(e.color(src, Color::BLACK)), col(e.color(dst, Color::BLACK)));
                    pts.push([(s[0], t[0]), (s[1], t[1]), (s[2], t[2])]);
                }
            }
            let all = e.flag("AdjustAll", true);
            let tabs: Vec<[f32; 256]> = (0..3).map(|c| curve_lut(&pts.iter().map(|p| p[c]).collect::<Vec<_>>())).collect();
            let ch = e.num("Channel", 0.0) as usize;
            im.map(|c| {
                let mut o = c;
                for k in 0..3 {
                    if all || ch == 0 || ch == k + 1 {
                        o[k] = tabs[k][c[k].round().clamp(0.0, 255.0) as usize];
                    }
                }
                o
            });
        }
        "ToneCurve" => {
            for ch in 0..4usize {
                if let Some(l) = e.list(&format!("Curve{ch}")) {
                    let pts: Vec<(f32, f32)> = l.chunks_exact(2).map(|p| (p[0] as f32, p[1] as f32)).collect();
                    let t = curve_lut(&pts);
                    lut(im, &t, ch);
                }
            }
        }
        "SMH" | "BCI" => {
            let (b, c, i) = (pct(e, "Brightness", 0.0), pct(e, "Contrast", 0.0), pct(e, "Intensity", 0.0));
            let (hi, sh, mi) = (pct(e, "Highlights", 0.0), pct(e, "Shadows", 0.0), pct(e, "Midtones", 0.0));
            let mut t = [0.0f32; 256];
            for (k, v) in t.iter_mut().enumerate() {
                let mut x = k as f32 / 255.0;
                x += b * 0.5;
                x = (x - 0.5) * (1.0 + c).max(0.0) + 0.5;
                // intensity brightens the light parts more than the dark ones
                x += i * 0.4 * x;
                let s = (1.0 - x).clamp(0.0, 1.0).powi(2);
                let hh = x.clamp(0.0, 1.0).powi(2);
                let m = 4.0 * x.clamp(0.0, 1.0) * (1.0 - x.clamp(0.0, 1.0));
                x += sh * 0.4 * s + hi * 0.4 * hh + mi * 0.3 * m;
                *v = (x * 255.0).clamp(0.0, 255.0);
            }
            lut(im, &t, 0);
        }
        "ColorBalance" => {
            let three = e.num("Mode", 1.0) >= 1.0;
            let keep = e.flag("Luminance", true);
            let get = |r: &str| [pct(e, &format!("CR_{r}"), 0.0), pct(e, &format!("MG_{r}"), 0.0), pct(e, &format!("YB_{r}"), 0.0)];
            let (m, s, mi, h) = (get("Master"), get("Shadows"), get("Midtones"), get("Highlights"));
            im.map(|c| {
                let l = luma(&[c[0], c[1], c[2], 255.0]) / 255.0;
                let ws = (1.0 - l * 2.0).clamp(0.0, 1.0);
                let wh = (l * 2.0 - 1.0).clamp(0.0, 1.0);
                let wm = 1.0 - ws - wh;
                let mut d = [0.0f32; 3];
                for k in 0..3 {
                    d[k] = if three { s[k] * ws + mi[k] * wm + h[k] * wh } else { m[k] };
                }
                // cyan-red moves red, magenta-green green, yellow-blue blue
                let mut o = [c[0] + d[0] * 80.0, c[1] + d[1] * 80.0, c[2] + d[2] * 80.0];
                if keep {
                    let l2 = luma(&[o[0], o[1], o[2], 255.0]).max(1.0);
                    let k = l * 255.0 / l2;
                    o = [o[0] * k, o[1] * k, o[2] * k];
                }
                o
            });
        }
        "Gamma" => {
            let g = e.num("Level", 1.0).max(0.01) as f32;
            let mut t = [0.0f32; 256];
            for (i, v) in t.iter_mut().enumerate() {
                *v = (i as f32 / 255.0).powf(1.0 / g) * 255.0;
            }
            lut(im, &t, 0);
        }
        "TintTemp" => {
            // warmer below 5000 K? The reference's slider runs from warm (left) to cool (right)
            let t = ((e.num("Temperature", 5000.0) as f32 - 5000.0) / 3000.0).clamp(-1.0, 1.0);
            let tint = pct(e, "Tint", 0.0);
            im.map(|c| [c[0] * (1.0 - t * 0.25), c[1] * (1.0 + tint * 0.2), c[2] * (1.0 + t * 0.25)]);
        }
        "HueSaturationLightness" => {
            // channel 0 is Master, 1..6 a hue range (red, yellow, green, cyan, blue, magenta), 7 Grayscale
            let vals: Vec<(f32, f32, f32)> = (0..8).map(|i| (e.num(&format!("Hue{i}"), 0.0) as f32, pct(e, &format!("Saturation{i}"), 0.0), pct(e, &format!("Lightness{i}"), 0.0))).collect();
            im.map(|c| {
                let (mut h, mut s, mut l) = rgb_to_hsl(c);
                let gray = s < 0.05;
                let mut dh = vals[0].0;
                let mut ds = vals[0].1;
                let mut dl = vals[0].2;
                if gray {
                    dl += vals[7].2;
                } else {
                    for k in 1..7 {
                        let center = (k - 1) as f32 * 60.0;
                        let d = ((h - center + 180.0).rem_euclid(360.0) - 180.0).abs();
                        let wgt = (1.0 - d / 60.0).clamp(0.0, 1.0);
                        dh += vals[k].0 * wgt;
                        ds += vals[k].1 * wgt;
                        dl += vals[k].2 * wgt;
                    }
                }
                h += dh;
                s = if ds >= 0.0 { s + (1.0 - s) * ds } else { s * (1.0 + ds) };
                l = if dl >= 0.0 { l + (1.0 - l) * dl } else { l * (1.0 + dl) };
                hsl_to_rgb(h, s, l)
            });
        }
        "Grayscale" => {
            // Black & White: each hue range adds its weight (%) to the gray
            let wts: Vec<f32> = ["Reds", "Yellows", "Greens", "Cyans", "Blues", "Magentas"].iter().zip([40.0, 60.0, 40.0, 60.0, 20.0, 80.0]).map(|(k, d)| pct(e, k, d)).collect();
            let split = e.flag("SplitToneTintEnabled", false);
            let (shh, shs) = (e.num("ShadowsHue", 42.0) as f32, pct(e, "ShadowsSaturation", 0.0));
            let (hih, his) = (e.num("HighlightsHue", 42.0) as f32, pct(e, "HighlightsSaturation", 0.0));
            im.map(|c| {
                let mn = c[0].min(c[1]).min(c[2]);
                let (h, _, _) = rgb_to_hsl(c);
                let chroma = c[0].max(c[1]).max(c[2]) - mn;
                // two neighbouring hue ranges share the chroma
                let f = h / 60.0;
                let i = f.floor() as usize % 6;
                let j = (i + 1) % 6;
                let t = f - f.floor();
                let g = mn + chroma * (wts[i] * (1.0 - t) + wts[j] * t);
                if split {
                    let l = (g / 255.0).clamp(0.0, 1.0);
                    let (hh, ss) = if l < 0.5 { (shh, shs * (1.0 - l * 2.0)) } else { (hih, his * (l * 2.0 - 1.0)) };
                    return hsl_to_rgb(hh, ss, l);
                }
                [g, g, g]
            });
        }
        "Vibrance" => {
            let (v, s) = (pct(e, "Vibrance", 0.0), pct(e, "Saturation", 0.0));
            im.map(|c| {
                let (h, sat, l) = rgb_to_hsl(c);
                // vibrance boosts the dull colors more than the saturated ones
                let mut sat2 = sat + v * (1.0 - sat) * (1.0 - sat);
                sat2 = if s >= 0.0 { sat2 + (1.0 - sat2) * s } else { sat2 * (1.0 + s) };
                hsl_to_rgb(h, sat2, l)
            });
        }
        "SelectiveClr" => {
            let abs = e.num("Mode", 0.0) >= 1.0;
            let adj: Vec<[f32; 4]> = (0..9).map(|i| [pct(e, &format!("Cyan{i}"), 0.0), pct(e, &format!("Magenta{i}"), 0.0), pct(e, &format!("Yellow{i}"), 0.0), pct(e, &format!("Black{i}"), 0.0)]).collect();
            im.map(|c| {
                let (h, s, l) = rgb_to_hsl(c);
                let mut o = c;
                for (i, a) in adj.iter().enumerate() {
                    if a.iter().all(|v| *v == 0.0) {
                        continue;
                    }
                    let wgt = if i < 6 {
                        let center = i as f32 * 60.0;
                        let d = ((h - center + 180.0).rem_euclid(360.0) - 180.0).abs();
                        (1.0 - d / 60.0).clamp(0.0, 1.0) * s
                    } else {
                        // shadows, midtones, highlights
                        match i {
                            6 => (1.0 - l * 2.0).clamp(0.0, 1.0),
                            8 => (l * 2.0 - 1.0).clamp(0.0, 1.0),
                            _ => 1.0 - (l * 2.0 - 1.0).abs(),
                        }
                    };
                    for k in 0..3 {
                        // more cyan is less red, and so on; black darkens all
                        let ink = 1.0 - o[k] / 255.0;
                        let d = if abs { a[k] } else { a[k] * ink };
                        let dk = if abs { a[3] } else { a[3] * ink };
                        o[k] -= (d + dk) * 255.0 * wgt;
                    }
                }
                o
            });
        }
        "ReplaceColorsV2" | "ReplaceColors" => {
            let from = col(e.color("ColorItem", Color::rgb(255, 0, 0)));
            let to = col(e.color("ReplaceColorItem", Color::rgb(0, 0, 255)));
            let (fh, fs, _) = rgb_to_hsl(from);
            let (th, ts, _) = rgb_to_hsl(to);
            let smooth = pct(e, "Smoothness", 50.0).max(0.05);
            let (dh, ds, dl) = (e.num("Hue", 0.0) as f32, pct(e, "Saturation", 0.0), pct(e, "Lightness", 0.0));
            let single = e.flag("SingleColor", false);
            if e.params.get("ColorItem").is_none() && dh == 0.0 && ds == 0.0 && dl == 0.0 {
                return true;
            }
            im.map(|c| {
                let (h, s, l) = rgb_to_hsl(c);
                let d = ((h - fh + 180.0).rem_euclid(360.0) - 180.0).abs() / 180.0 + (s - fs).abs() * 0.5;
                let wgt = (1.0 - d / (smooth * 0.6)).clamp(0.0, 1.0);
                if wgt <= 0.0 {
                    return c;
                }
                let (nh, ns, nl) = if single { rgb_to_hsl(to) } else { (h + (th - fh) + dh, (s + (ts - fs) + ds).clamp(0.0, 1.0), (l + dl).clamp(0.0, 1.0)) };
                let n = hsl_to_rgb(nh, ns, nl);
                [c[0] + (n[0] - c[0]) * wgt, c[1] + (n[1] - c[1]) * wgt, c[2] + (n[2] - c[2]) * wgt]
            });
        }
        "ChannelMixer" => {
            let mono = e.num("OutputAsType", 0.0) >= 1.0;
            // each output channel mixes the input channels (0 red, 1 green, 2 blue)
            let row = |o: usize| {
                let d = |i: usize| if i == o { 100.0 } else { 0.0 };
                [pct(e, &format!("RedChannel{o}"), d(0)), pct(e, &format!("GreenChannel{o}"), d(1)), pct(e, &format!("BlueChannel{o}"), d(2)), pct(e, &format!("Constant{o}"), 0.0)]
            };
            let (rr, gg, bb) = (row(0), row(1), row(2));
            im.map(|c| {
                let f = |m: [f32; 4]| c[0] * m[0] + c[1] * m[1] + c[2] * m[2] + m[3] * 255.0;
                if mono {
                    let v = f(rr);
                    [v, v, v]
                } else {
                    [f(rr), f(gg), f(bb)]
                }
            });
        }
        "Posterize" => {
            let n = e.num("Level", 4.0).max(2.0) as f32;
            im.map(|c| {
                let q = |v: f32| ((v / 255.0 * (n - 1.0)).round() / (n - 1.0)) * 255.0;
                [q(c[0]), q(c[1]), q(c[2])]
            });
        }
        "Threshold" => {
            let ch = e.num("Channel", 0.0) as usize;
            let mid = e.num("Mid", 127.0) as f32;
            let lo = e.num("Low", 0.0) as f32;
            let hi = e.num("High", 255.0) as f32;
            let mode = e.num("Level", 0.0) as u8;
            im.map(|c| {
                let v = match ch {
                    1 => c[0],
                    2 => c[1],
                    3 => c[2],
                    _ => luma(&[c[0], c[1], c[2], 255.0]),
                };
                let o = match mode {
                    // to black: below the threshold turns black, the rest keeps its color
                    0 => {
                        if v < mid {
                            return [0.0, 0.0, 0.0];
                        } else {
                            return c;
                        }
                    }
                    1 => {
                        if v > mid {
                            return [255.0, 255.0, 255.0];
                        } else {
                            return c;
                        }
                    }
                    _ => {
                        if v < lo.max(0.0).min(mid) || v < mid && lo <= 0.0 {
                            0.0
                        } else if v > hi.min(255.0) {
                            255.0
                        } else if v < mid {
                            0.0
                        } else {
                            255.0
                        }
                    }
                };
                [o, o, o]
            });
        }
        "DeInterlace" => {
            let odd = e.num("ScanLine", 0.0) >= 1.0;
            let interp = e.num("Replacement", 1.0) >= 1.0;
            // one scan line of the original image is about one effect pixel
            let line = cx.s.max(1.0);
            let src = im.clone();
            for y in 0..im.h {
                let k = ((y as f32 - cx.frame[1]) / line).floor() as i64;
                if (k.rem_euclid(2) == 1) != odd {
                    continue;
                }
                let up = (y as f32 - line).max(0.0) as usize;
                let dn = ((y as f32 + line) as usize).min(im.h - 1);
                for x in 0..im.w {
                    let a = src.at(x, up);
                    im.p[y * im.w + x] = if interp {
                        let b = src.at(x, dn);
                        [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0, src.at(x, y)[3]]
                    } else {
                        [a[0], a[1], a[2], src.at(x, y)[3]]
                    };
                }
            }
        }
        "BitPlanes" => {
            let all = e.flag("AllPlanes", true);
            let r = e.num("Red", 0.0) as u32;
            let (g, b) = if all { (r, r) } else { (e.num("Green", 0.0) as u32, e.num("Blue", 0.0) as u32) };
            im.map(|c| {
                let bit = |v: f32, k: u32| if (v as u32 >> k.min(7)) & 1 == 1 { 255.0 } else { 0.0 };
                [bit(c[0], r), bit(c[1], g), bit(c[2], b)]
            });
        }
        "Psychedelic" => {
            let lv = e.num("Level", 127.0) as f32;
            im.map(|c| {
                let f = |v: f32| ((v * 2.0 + lv * 3.0) % 512.0 - 256.0).abs().min(255.0);
                [f(c[0]), f(c[1] + 85.0), f(c[2] + 170.0)]
            });
        }
        "Solarize" => {
            let lv = e.num("Level", 127.0) as f32;
            im.map(|c| {
                let f = |v: f32| if v > lv { 255.0 - v } else { v };
                [f(c[0]), f(c[1]), f(c[2])]
            });
        }
        "EdgeDetect" => {
            let bg = match e.num("ColorMode", 0.0) as u8 {
                0 => [255.0, 255.0, 255.0],
                1 => [0.0, 0.0, 0.0],
                _ => col(e.color("BackgroundColorItem", Color::BLACK)),
            };
            let sens = e.num("Sensitivity", 1.0) as f32;
            let (gx, gy) = sobel(im);
            let src = im.clone();
            for (i, p) in im.p.iter_mut().enumerate() {
                if p[3] <= 0.0 {
                    continue;
                }
                let m = ((gx[i] * gx[i] + gy[i] * gy[i]).sqrt() / 255.0 * (0.5 + sens * 0.5)).clamp(0.0, 1.0);
                let s = src.p[i];
                for c in 0..3 {
                    p[c] = bg[c] * (1.0 - m) + s[c] * m;
                }
            }
        }
        "FindEdges" => {
            let solid = e.num("EdgeType", 0.0) >= 1.0;
            let lv = pct(e, "Level", 50.0);
            let (gx, gy) = sobel(im);
            let src = im.clone();
            for (i, p) in im.p.iter_mut().enumerate() {
                if p[3] <= 0.0 {
                    continue;
                }
                let mut m = ((gx[i] * gx[i] + gy[i] * gy[i]).sqrt() / 255.0 * (0.3 + lv * 2.0)).clamp(0.0, 1.0);
                if solid {
                    m = if m > 0.25 { 1.0 } else { 0.0 };
                }
                let s = src.p[i];
                for c in 0..3 {
                    // the edges keep their color (darkened), the rest turns white
                    p[c] = 255.0 * (1.0 - m) + s[c] * 0.6 * m;
                }
            }
        }
        "TraceContour" => {
            let lv = e.num("Level", 127.0) as f32;
            let upper = e.num("EdgeType", 0.0) >= 1.0;
            let l = im.luma();
            let src = im.clone();
            let (w, h) = (im.w as i64, im.h as i64);
            for y in 0..h {
                for x in 0..w {
                    let i = (y * w + x) as usize;
                    if src.p[i][3] <= 0.0 {
                        continue;
                    }
                    let here = if upper { l[i] >= lv } else { l[i] < lv };
                    let mut edge = false;
                    for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
                        let (nx, ny) = ((x + dx).clamp(0, w - 1), (y + dy).clamp(0, h - 1));
                        let n = l[(ny * w + nx) as usize];
                        if here && (if upper { n < lv } else { n >= lv }) {
                            edge = true;
                        }
                    }
                    let p = &mut im.p[i];
                    if edge {
                        // the contour in a strong version of the color
                        for c in 0..3 {
                            p[c] = if src.p[i][c] > 127.0 { 255.0 } else { 0.0 };
                        }
                    } else {
                        p[0] = 255.0;
                        p[1] = 255.0;
                        p[2] = 255.0;
                    }
                }
            }
        }
        "LocalEqualize" => {
            let (rw, rh) = ((cx.px(e.num("Width", 50.0)) / 2.0).max(1.0) as usize, (cx.px(e.num("Height", 50.0)) / 2.0).max(1.0) as usize);
            let mean = box_blur(im, rw, rh);
            let mut sq = im.clone();
            for p in sq.p.iter_mut() {
                for c in 0..3 {
                    p[c] = p[c] * p[c] / 255.0;
                }
            }
            let sqm = box_blur(&sq, rw, rh);
            for (i, p) in im.p.iter_mut().enumerate() {
                if p[3] <= 0.0 {
                    continue;
                }
                for c in 0..3 {
                    let m = mean.p[i][c];
                    let var = (sqm.p[i][c] * 255.0 - m * m).max(1.0);
                    let sd = var.sqrt();
                    p[c] = clamp255(128.0 + (p[c] - m) / sd * 50.0);
                }
            }
        }
        "DustScratch" => {
            let r = cx.px(e.num("Radius", 2.0)).round().max(1.0) as usize;
            let th = e.num("Threshold", 64.0) as f32;
            let med = rank(im, r.min(12), 2);
            for (p, m) in im.p.iter_mut().zip(med.p.iter()) {
                if (luma(p) - luma(m)).abs() > th {
                    p[0] = m[0];
                    p[1] = m[1];
                    p[2] = m[2];
                }
            }
        }
        _ => return false,
    }
    true
}

/// The Blur effects.
fn blur_fx(e: &BitmapEffect, im: &mut Img, cx: &FxCtx) -> bool {
    match e.id.as_str() {
        "GaussianBlur" => *im = gauss(im, cx.px(e.num("Radius", 5.0)) * 2.0),
        "Glow" => {
            let mut g = gauss(im, cx.px(e.num("Radius", 4.0)) * 2.0);
            let c = col(e.color("Color", Color::rgb(255, 230, 120)));
            for p in g.p.iter_mut() {
                p[0] = c[0];
                p[1] = c[1];
                p[2] = c[2];
            }
            // the object over its glow
            for (gp, sp) in g.p.iter_mut().zip(im.p.iter()) {
                let a = sp[3] / 255.0;
                for k in 0..3 {
                    gp[k] = gp[k] * (1.0 - a) + sp[k] * a;
                }
                gp[3] = sp[3] + gp[3] * (1.0 - a);
            }
            *im = g;
        }
        "Smooth" | "Soften" | "LowPass" => {
            let p = pct(e, "Percentage", if e.id == "LowPass" { 100.0 } else { 50.0 });
            let r = match e.id.as_str() {
                "Smooth" => cx.px(2.0),
                "Soften" => cx.px(1.0),
                _ => cx.px(e.num("Radius", 1.0)) * 1.5,
            };
            let b = gauss(im, r.max(1.0));
            im.mix(&b, p);
        }
        "DirectionalSmooth" | "SmartBlur" => {
            // smooths the flat areas and keeps the edges
            let p = pct(e, if e.id == "SmartBlur" { "Detail" } else { "Percentage" }, 50.0);
            let b = gauss(im, cx.px(2.0).max(1.0) * (1.0 + p * 2.0));
            let (gx, gy) = sobel(im);
            for (i, px) in im.p.iter_mut().enumerate() {
                let m = ((gx[i] * gx[i] + gy[i] * gy[i]).sqrt() / 120.0).clamp(0.0, 1.0);
                let t = p * (1.0 - m);
                for c in 0..3 {
                    px[c] = px[c] * (1.0 - t) + b.p[i][c] * t;
                }
            }
        }
        "JaggyDespeckle" => {
            let (rw, rh) = ((cx.px(e.num("Width", 3.0)) / 3.0).round() as usize, (cx.px(e.num("Height", 3.0)) / 3.0).round() as usize);
            *im = box_blur(im, rw.max(1), rh.max(1));
        }
        "Feather" => {
            // fades the edges of the object to transparent over `Width`
            let wd = cx.px(e.num("Width", 20.0)).max(1.0);
            let mode = e.num("Mode", 0.0) as u8;
            let mut a = im.clone();
            for p in a.p.iter_mut() {
                let v = if p[3] > 127.0 { 255.0 } else { 0.0 };
                *p = [v, v, v, 255.0];
            }
            let b = gauss(&a, wd);
            for (p, m) in im.p.iter_mut().zip(b.p.iter()) {
                // inside distance from the edge, 0..1
                let t = ((m[0] / 255.0 - 0.5) * 2.0).clamp(0.0, 1.0);
                let f = match mode {
                    1 => t,
                    2 => 1.0 - (-(t * 2.5).powi(2)).exp(),
                    _ => (t * std::f32::consts::FRAC_PI_2).sin(),
                };
                p[3] *= f;
            }
        }
        "MotionBlur" => {
            let d = cx.px(e.num("Distance", 5.0));
            let ang = (e.num("Direction", 0.0) as f32).to_radians();
            let edge = match e.num("Sampling", 0.0) as u8 {
                2 => Edge::Clamp,
                _ => Edge::Clear,
            };
            let n = (d.ceil() as usize).clamp(1, 120);
            let (dx, dy) = (ang.cos() * d / n as f32, -ang.sin() * d / n as f32);
            let src = im.clone();
            *im = streak(&src, n, |k| (dx * (k as f32 - n as f32 / 2.0), dy * (k as f32 - n as f32 / 2.0)), edge);
        }
        "RadialBlur" => {
            let (cxp, cyp) = cx.center(e);
            let amt = pct(e, "Amount", 5.0) * 0.6;
            let src = im.clone();
            let n = 24usize;
            *im = Img::new(src.w, src.h);
            for y in 0..src.h {
                for x in 0..src.w {
                    let (px, py) = (x as f32 + 0.5 - cxp, y as f32 + 0.5 - cyp);
                    let mut acc = [0.0f32; 4];
                    let mut aw = 0.0;
                    for k in 0..n {
                        let a = (k as f32 / (n - 1) as f32 - 0.5) * amt;
                        let (s, c) = a.sin_cos();
                        let p = src.sample(cxp + px * c - py * s, cyp + px * s + py * c, Edge::Clear);
                        for ch in 0..3 {
                            acc[ch] += p[ch] * p[3];
                        }
                        acc[3] += p[3];
                        aw += p[3];
                    }
                    let o = &mut im.p[y * src.w + x];
                    if aw > 0.0 {
                        *o = [acc[0] / aw, acc[1] / aw, acc[2] / aw, acc[3] / n as f32];
                    }
                }
            }
        }
        "Zoom" => {
            let (cxp, cyp) = cx.center(e);
            let amt = pct(e, "Amount", 5.0) * 0.5;
            let src = im.clone();
            let n = 20usize;
            for y in 0..src.h {
                for x in 0..src.w {
                    let (px, py) = (x as f32 + 0.5 - cxp, y as f32 + 0.5 - cyp);
                    let mut acc = [0.0f32; 4];
                    let mut aw = 0.0;
                    for k in 0..n {
                        let f = 1.0 - amt * k as f32 / n as f32;
                        let p = src.sample(cxp + px * f, cyp + py * f, Edge::Clear);
                        for ch in 0..3 {
                            acc[ch] += p[ch] * p[3];
                        }
                        acc[3] += p[3];
                        aw += p[3];
                    }
                    if aw > 0.0 {
                        im.p[y * src.w + x] = [acc[0] / aw, acc[1] / aw, acc[2] / aw, acc[3] / n as f32];
                    }
                }
            }
        }
        "TuneBlur" => {
            let step = pct(e, "Step", 10.0);
            for op in e.list("Ops").unwrap_or(&[]).iter() {
                let b = match *op as u8 {
                    0 => gauss(im, cx.px(1.0 + step * 10.0)),
                    1 => box_blur(im, cx.px(1.0 + step * 5.0) as usize, cx.px(1.0 + step * 5.0) as usize),
                    2 => box_blur(im, cx.px(1.0 + step * 8.0) as usize, 1),
                    _ => gauss(im, cx.px(0.5 + step * 4.0)),
                };
                *im = b;
            }
        }
        _ => return false,
    }
    true
}

/// Averages `n` samples displaced by `off(k)` (motion streaks).
pub fn streak(src: &Img, n: usize, off: impl Fn(usize) -> (f32, f32), edge: Edge) -> Img {
    let mut out = Img::new(src.w, src.h);
    let offs: Vec<(f32, f32)> = (0..n).map(&off).collect();
    for y in 0..src.h {
        for x in 0..src.w {
            let mut acc = [0.0f32; 4];
            let mut aw = 0.0;
            for (dx, dy) in &offs {
                let p = src.sample(x as f32 + 0.5 + dx, y as f32 + 0.5 + dy, edge);
                for c in 0..3 {
                    acc[c] += p[c] * p[3];
                }
                acc[3] += p[3];
                aw += p[3];
            }
            if aw > 0.0 {
                out.p[y * src.w + x] = [acc[0] / aw, acc[1] / aw, acc[2] / aw, acc[3] / n as f32];
            }
        }
    }
    out
}

/// The Sharpen effects (and Correction ▸ Tune Sharpen).
fn sharpen_fx(e: &BitmapEffect, im: &mut Img, cx: &FxCtx) -> bool {
    let unsharp = |im: &mut Img, amount: f32, radius: f32, th: f32, colors: bool| {
        let b = gauss(im, radius.max(0.5));
        for (p, q) in im.p.iter_mut().zip(b.p.iter()) {
            if p[3] <= 0.0 {
                continue;
            }
            if colors {
                // sharpen the lightness only
                let d = luma(p) - luma(q);
                if d.abs() >= th {
                    for c in 0..3 {
                        p[c] = clamp255(p[c] + d * amount);
                    }
                }
            } else {
                for c in 0..3 {
                    let d = p[c] - q[c];
                    if d.abs() >= th {
                        p[c] = clamp255(p[c] + d * amount);
                    }
                }
            }
        }
    };
    match e.id.as_str() {
        "UnsharpMask" => unsharp(im, pct(e, "Percentage", 100.0), cx.px(e.num("Radius", 1.0)) * 2.0, e.num("Threshold", 0.0) as f32, false),
        "Sharpen" => unsharp(im, pct(e, "EdgeLevel", 25.0) * 2.0, cx.px(1.0) * 1.5, e.num("Threshold", 0.0) as f32, e.flag("PreserveColors", false)),
        "AdaptiveUnsharp" => {
            // stronger on the edges, gentle in flat areas
            let amt = pct(e, "Level", 50.0) * 1.5;
            let (gx, gy) = sobel(im);
            let b = gauss(im, cx.px(1.0) * 2.0);
            for (i, p) in im.p.iter_mut().enumerate() {
                let m = ((gx[i] * gx[i] + gy[i] * gy[i]).sqrt() / 200.0).clamp(0.0, 1.0);
                for c in 0..3 {
                    p[c] = clamp255(p[c] + (p[c] - b.p[i][c]) * amt * m);
                }
            }
        }
        "DirectionalSharpen" => unsharp(im, pct(e, "Percentage", 50.0) * 1.5, cx.px(1.0) * 1.5, 4.0, false),
        "HighPass" => {
            let p = pct(e, "Percentage", 100.0);
            let b = gauss(im, cx.px(e.num("Radius", 1.0)) * 2.0);
            for (px, q) in im.p.iter_mut().zip(b.p.iter()) {
                for c in 0..3 {
                    let hp = clamp255(128.0 + px[c] - q[c]);
                    px[c] = px[c] * (1.0 - p) + hp * p;
                }
            }
        }
        "TuneSharpness" => {
            let step = pct(e, "Percentage", 10.0);
            for op in e.list("Ops").unwrap_or(&[]).iter() {
                unsharp(im, step * 4.0 * if *op as u8 == 2 { 0.6 } else { 1.0 }, cx.px(1.0 + (*op as u8 % 2) as f32) * 2.0, 0.0, false);
            }
        }
        "BandPass" => {
            // keeps the detail between two blur sizes
            let a = gauss(im, cx.px(1.0));
            let b = gauss(im, cx.px(4.0));
            let (fi, mi) = (pct(e, "InWeight", 100.0), pct(e, "MidWeight", 100.0));
            for (i, p) in im.p.iter_mut().enumerate() {
                for c in 0..3 {
                    let band = a.p[i][c] - b.p[i][c];
                    p[c] = clamp255(b.p[i][c] * mi + band * (1.0 + fi) + p[c] * (1.0 - mi) * 0.0 + (1.0 - mi) * 128.0);
                }
            }
        }
        "UserDefined" => {
            let mut k = [0.0f32; 25];
            let mut sum = 0.0;
            for j in 0..5 {
                for i in 0..5 {
                    let def = match (j, i) {
                        (2, 2) => 5.0,
                        (1, 2) | (2, 1) | (2, 3) | (3, 2) => -1.0,
                        _ => 0.0,
                    };
                    k[j * 5 + i] = e.num(&format!("Kernel_{j}{i}"), def) as f32;
                    sum += k[j * 5 + i];
                }
            }
            let div = if e.flag("AutoDivisor", true) { if sum.abs() < 1e-3 { 1.0 } else { sum } } else { (e.num("Divisor", 1.0) as f32).max(1e-3) };
            let off = e.num("Offset", 0.0) as f32;
            let out = convolve(im, &k, 5, div, off);
            if e.flag("PreserveColors", false) {
                for (p, q) in im.p.iter_mut().zip(out.p.iter()) {
                    let d = luma(q) - luma(p);
                    for c in 0..3 {
                        p[c] = clamp255(p[c] + d);
                    }
                }
            } else {
                *im = out;
            }
        }
        _ => return false,
    }
    true
}

/// The Noise effects.
fn noise_fx(e: &BitmapEffect, im: &mut Img, cx: &FxCtx) -> bool {
    match e.id.as_str() {
        "AddNoise" => {
            let lv = pct(e, "Level", 50.0) * 160.0;
            let dens = pct(e, "Density", 50.0);
            let mode = e.num("NoiseMode", 0.0) as u8;
            let cmode = e.num("ColorMode", 0.0) as u8;
            let single = col(e.color("ColorItem", Color::BLACK));
            let s = cx.s.max(1.0);
            for y in 0..im.h {
                for x in 0..im.w {
                    let (ex, ey) = (((x as f32 - cx.frame[0]) / s) as i64, ((y as f32 - cx.frame[1]) / s) as i64);
                    if hash(ex, ey, cx.seed ^ 77) > dens {
                        continue;
                    }
                    let r = |k: u32| {
                        let u = hash(ex, ey, cx.seed.wrapping_add(k));
                        match mode {
                            // gaussian: the sum of uniforms
                            0 => (u + hash(ex, ey, cx.seed.wrapping_add(k + 9)) + hash(ex, ey, cx.seed.wrapping_add(k + 19)) - 1.5) * 1.15,
                            1 => {
                                if u > 0.9 {
                                    1.0
                                } else if u < 0.1 {
                                    -1.0
                                } else {
                                    0.0
                                }
                            }
                            _ => u - 0.5,
                        }
                    };
                    let p = &mut im.p[y * im.w + x];
                    if p[3] <= 0.0 {
                        continue;
                    }
                    match cmode {
                        0 => {
                            let n = r(1) * lv;
                            for c in 0..3 {
                                p[c] = clamp255(p[c] + n);
                            }
                        }
                        1 => {
                            for c in 0..3 {
                                p[c] = clamp255(p[c] + r(c as u32 + 3) * lv);
                            }
                        }
                        _ => {
                            let t = (r(1).abs() * lv / 160.0).clamp(0.0, 1.0);
                            for c in 0..3 {
                                p[c] = p[c] * (1.0 - t) + single[c] * t;
                            }
                        }
                    }
                }
            }
        }
        "Maximum" | "Minimum" | "Median" => {
            let r = cx.px(e.num("Radius", 1.0)).round().clamp(1.0, 12.0) as usize;
            let which = match e.id.as_str() {
                "Maximum" => 0,
                "Minimum" => 1,
                _ => 2,
            };
            let out = rank(im, r, which);
            let p = if which == 2 { 1.0 } else { pct(e, "Percentage", 100.0) };
            im.mix(&out, p);
        }
        "RemoveMoire" => {
            let b = gauss(im, cx.px(0.5 + e.num("Amount", 6.0) * 0.25));
            im.mix(&b, 0.8);
        }
        "RemoveNoise" => {
            let th = if e.flag("Auto", true) { 24.0 } else { (255.0 - e.num("Threshold", 127.0) as f32) / 4.0 };
            let med = rank(im, cx.px(1.0).round().max(1.0) as usize, 2);
            for (p, m) in im.p.iter_mut().zip(med.p.iter()) {
                if (luma(p) - luma(m)).abs() > th {
                    p[0] = m[0];
                    p[1] = m[1];
                    p[2] = m[2];
                }
            }
        }
        "TuneNoise" => {
            let lv = pct(e, "Level", 50.0);
            for op in e.list("Ops").unwrap_or(&[]).iter() {
                let op = *op as u8;
                let mut n = vector_core::object::BitmapEffect::new(match op {
                    0..=2 => "AddNoise",
                    3 => "Diffuse",
                    4 => "Minimum",
                    5 => "Median",
                    6 => "Maximum",
                    7 => "JaggyDespeckle",
                    _ => "RemoveNoise",
                });
                n = n.with("NoiseMode", match op {
                    0 => 1.0,
                    1 => 0.0,
                    _ => 2.0,
                }).with("Level", (lv * 30.0) as f64).with("Density", e.num("Density", 50.0)).with("Percentage", (lv * 100.0) as f64);
                let mut cx2 = *cx;
                cx2.seed = cx.seed.wrapping_add(op as u32 * 31);
                let _ = noise_fx(&n, im, &cx2) || blur_fx(&n, im, &cx2) || art::apply(&n, im, &cx2);
            }
        }
        "ThreeDStereoNoise" => {
            // a random-dot stereogram whose depth is the lightness of the image
            let depth = e.num("DepthValue", 3.0) as f32;
            let period = (cx.px(40.0)).max(8.0) as usize;
            let l = im.luma();
            let s = cx.s.max(1.0);
            for y in 0..im.h {
                let mut row = vec![0.0f32; im.w];
                for x in 0..im.w {
                    let shift = (l[y * im.w + x] / 255.0 * depth * cx.s * 2.0) as usize;
                    row[x] = if x >= period { row[x + shift.min(period - 1) - period] } else { hash((x as f32 / s) as i64, (y as f32 / s) as i64, cx.seed) * 255.0 };
                }
                for x in 0..im.w {
                    let p = &mut im.p[y * im.w + x];
                    if p[3] > 0.0 {
                        p[0] = row[x];
                        p[1] = row[x];
                        p[2] = row[x];
                    }
                }
            }
            if e.flag("ShowDots", true) {
                // two guide dots above the image
                for k in [-1.0f32, 1.0] {
                    let (dx, dy) = (cx.frame[0] + cx.fw() / 2.0 + k * period as f32 / 2.0, cx.frame[1] + cx.px(6.0));
                    let r = cx.px(3.0).max(2.0);
                    for y in (dy - r).max(0.0) as usize..((dy + r) as usize).min(im.h) {
                        for x in (dx - r).max(0.0) as usize..((dx + r) as usize).min(im.w) {
                            if ((x as f32 - dx).powi(2) + (y as f32 - dy).powi(2)).sqrt() <= r {
                                im.p[y * im.w + x] = [0.0, 0.0, 0.0, 255.0];
                            }
                        }
                    }
                }
            }
        }
        _ => return false,
    }
    true
}
